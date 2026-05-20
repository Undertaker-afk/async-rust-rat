use iroh::{Endpoint, RelayMode, SecretKey, RelayConfig, RelayUrl, EndpointAddr};
use iroh::endpoint::{presets, QuicTransportConfig, VarInt, Connection};
use std::sync::Arc;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;
use tokio::sync::mpsc;
use tokio::task;
use chacha20poly1305::{ChaCha20Poly1305, aead::Aead, aead::NewAead};
use rand::RngCore;

#[derive(Debug, Deserialize)]
struct TailscaleDerpMap {
    #[serde(rename = "Regions")]
    region_map: HashMap<u32, Region>,
}

#[derive(Debug, Deserialize)]
struct Region {
    #[serde(rename = "Nodes")]
    nodes: Vec<Node>,
}

#[derive(Debug, Deserialize)]
struct Node {
    #[serde(rename = "HostName")]
    host_name: String,
    #[serde(rename = "DERPPort")]
    derp_port: Option<u16>,
}

#[derive(Clone)]
pub struct IrohManager {
    endpoint: Endpoint,
}

impl IrohManager {
    pub async fn new(secret_key: SecretKey) -> Result<Self> {
        let relay_map = RelayMode::Default.relay_map();

        // Optional: Fetch latest DERP map to ensure best reachability
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .use_rustls_tls()
            .build();

        if let Ok(client) = client {
            if let Ok(response) = client.get("https://login.tailscale.com/derpmap/default").send().await {
                if let Ok(derp_map) = response.json::<TailscaleDerpMap>().await {
                    for region in derp_map.region_map.values() {
                        for node in &region.nodes {
                            let port = node.derp_port.unwrap_or(443);
                            let url_str = if port == 443 {
                                format!("https://{}", node.host_name)
                            } else {
                                format!("https://{}:{}", node.host_name, port)
                            };

                            if let Ok(url) = url_str.parse::<RelayUrl>() {
                                relay_map.insert(url.clone(), Arc::new(RelayConfig::from(url)));
                            }
                        }
                    }
                }
            }
        }

        let transport_config = QuicTransportConfig::builder()
            .max_concurrent_uni_streams(VarInt::from_u32(2048))
            .stream_receive_window(VarInt::from_u32(1024 * 1024 * 32))
            .receive_window(VarInt::from_u32(1024 * 1024 * 128))
            .max_idle_timeout(Some(std::time::Duration::from_secs(60).try_into()?))
            .build();

        let endpoint = Endpoint::builder(presets::N0)
            .secret_key(secret_key)
            .relay_mode(RelayMode::Custom(relay_map))
            .transport_config(transport_config)
            .alpns(vec![b"bloodin-p2p/0.1".to_vec()])
            .bind()
            .await?;

        Ok(Self { endpoint })
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    pub fn addr(&self) -> EndpointAddr {
        self.endpoint.addr()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum P2PMessageType {
    Stream = 0x01,
    Blob = 0x02,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptedFrame {
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
}

#[derive(Clone)]
pub struct P2PChannel {
    key: [u8; 32],
}

impl P2PChannel {
    pub fn new(key: [u8; 32]) -> Self {
        Self { key }
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedFrame> {
        let cipher = ChaCha20Poly1305::new(&self.key.into());
        let mut nonce = [0u8; 12];
        rand::rng().fill_bytes(&mut nonce);
        let ciphertext = cipher
            .encrypt(&nonce.into(), plaintext)
            .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;

        Ok(EncryptedFrame { nonce, ciphertext })
    }

    pub fn decrypt(&self, frame: &EncryptedFrame) -> Result<Vec<u8>> {
        let cipher = ChaCha20Poly1305::new(&self.key.into());
        cipher
            .decrypt(&frame.nonce.into(), frame.ciphertext.as_ref())
            .map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))
    }
}

pub struct P2PDispatcher {
    connection: Connection,
    stream_tx: mpsc::Sender<Vec<u8>>,
    blob_tx: mpsc::Sender<Vec<u8>>,
    crypto: P2PChannel,
}

impl P2PDispatcher {
    pub fn new(
        connection: Connection,
        crypto: P2PChannel,
    ) -> (Arc<Self>, mpsc::Receiver<Vec<u8>>, mpsc::Receiver<Vec<u8>>) {
        let (stream_tx, stream_rx) = mpsc::channel(1000);
        let (blob_tx, blob_rx) = mpsc::channel(100);

        let dispatcher = Arc::new(Self {
            connection,
            stream_tx,
            blob_tx,
            crypto,
        });

        let d_clone = dispatcher.clone();
        tokio::spawn(async move {
            if let Err(e) = d_clone.run_loop().await {
                debug!("P2P dispatcher loop stopped: {}", e);
            }
        });

        (dispatcher, stream_rx, blob_rx)
    }

    async fn run_loop(&self) -> Result<()> {
        let conn = self.connection.clone();

        loop {
            let mut recv_stream = conn.accept_uni().await?;
            let blob_dispatcher = self.blob_tx.clone();
            let stream_dispatcher = self.stream_tx.clone();
            let crypto = self.crypto.clone();

            tokio::spawn(async move {
                const MAX_STREAM_SIZE: usize = 100 * 1024 * 1024;
                match recv_stream.read_to_end(MAX_STREAM_SIZE).await {
                    Ok(payload) => {
                        if payload.len() < 14 { return; }
                        let header = payload[0];

                        let result = task::spawn_blocking(move || -> Result<(u8, Vec<u8>)> {
                            let mut nonce = [0u8; 12];
                            nonce.copy_from_slice(&payload[1..13]);
                            let ciphertext = &payload[13..];

                            let encrypted = EncryptedFrame { nonce, ciphertext: ciphertext.to_vec() };
                            let decrypted = crypto.decrypt(&encrypted)?;
                            let decompressed = zstd::decode_all(&decrypted[..])?;
                            Ok((header, decompressed))
                        }).await;

                        match result {
                            Ok(Ok((0x01, data))) => { let _ = stream_dispatcher.send(data).await; }
                            Ok(Ok((0x02, data))) => { let _ = blob_dispatcher.send(data).await; }
                            _ => {}
                        }
                    }
                    Err(_) => {}
                }
            });
        }
    }

    pub async fn send(&self, msg_type: P2PMessageType, data: Vec<u8>) -> Result<()> {
        let crypto = self.crypto.clone();
        let payload = task::spawn_blocking(move || -> Result<Vec<u8>> {
            let compressed = zstd::encode_all(&data[..], 3)?;
            let encrypted = crypto.encrypt(&compressed)?;

            let mut packet = Vec::with_capacity(1 + 12 + encrypted.ciphertext.len());
            packet.push(msg_type as u8);
            packet.extend_from_slice(&encrypted.nonce);
            packet.extend_from_slice(&encrypted.ciphertext);
            Ok(packet)
        }).await??;

        let mut send_stream = self.connection.open_uni().await?;
        send_stream.write_all(&payload).await?;
        send_stream.finish()?;
        Ok(())
    }
}
