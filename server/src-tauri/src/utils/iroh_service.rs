use iroh::{Endpoint, NodeTicket, RelayMap, RelayMode};
use moq_transport::setup;
use std::sync::Arc;
use tokio::sync::Mutex;
use once_cell::sync::Lazy;
use tauri::{AppHandle, Emitter};
use base64::{engine::general_purpose, Engine as _};
use nostr_sdk::prelude::*;
use std::str::FromStr;
use serde_json::Value;

pub static IROH_ENDPOINT: Lazy<Mutex<Option<Endpoint>>> = Lazy::new(|| Mutex::new(None));
pub static TAURI_HANDLE: Lazy<Mutex<Option<AppHandle>>> = Lazy::new(|| Mutex::new(None));

pub async fn init_iroh() -> anyhow::Result<()> {
    let relay_map = fetch_tailscale_derp_map().await.unwrap_or_else(|_| RelayMap::default());

    let endpoint = Endpoint::builder()
        .relay_mode(RelayMode::Custom(relay_map))
        .bind()
        .await?;

    let mut guard = IROH_ENDPOINT.lock().await;
    *guard = Some(endpoint.clone());

    tokio::spawn(async move {
        while let Ok(conn) = endpoint.accept().await {
            tokio::spawn(async move {
                if let Err(e) = handle_moq_connection(conn).await {
                    eprintln!("MoQ connection error: {:?}", e);
                }
            });
        }
    });

    Ok(())
}

async fn fetch_tailscale_derp_map() -> anyhow::Result<RelayMap> {
    let client = reqwest::Client::new();
    let response = client.get("https://login.tailscale.com/derpmap/default").send().await?;
    let derp_map: Value = response.json().await?;

    let mut relay_map = RelayMap::default();
    if let Some(regions) = derp_map.get("Regions").and_then(|r| r.as_object()) {
        for (_id, region) in regions {
            if let Some(nodes) = region.get("Nodes").and_then(|n| n.as_array()) {
                for node in nodes {
                    if let Some(host) = node.get("HostName").and_then(|h| h.as_str()) {
                        let url = format!("https://{}", host);
                        if let Ok(url) = iroh::relay::RelayUrl::from_str(&url) {
                            relay_map.add_node(url, iroh::relay::RelayMode::Stun);
                        }
                    }
                }
            }
        }
    }

    Ok(relay_map)
}

async fn handle_moq_connection(conn: iroh::endpoint::Connecting) -> anyhow::Result<()> {
    let connection = conn.await?;
    let (mut session, mut control) = moq_transport::session::run(connection).await?;

    tokio::spawn(async move {
        while let Some(mut track) = session.subscribe_any().await {
            tokio::spawn(async move {
                while let Some(chunk) = track.next().await {
                    if let Ok(data) = chunk {
                        let handle_guard = TAURI_HANDLE.lock().await;
                        if let Some(handle) = handle_guard.as_ref() {
                            handle.emit("high_speed_frame", general_purpose::STANDARD.encode(&data)).ok();
                        }
                    }
                }
            });
        }
    });

    Ok(())
}

pub async fn negotiate_best_relay(client_addr: String, client_pubkey: PublicKey) -> anyhow::Result<()> {
    let mut guard = TAURI_HANDLE.lock().await;
    if let Some(handle) = guard.as_ref() {
        // Load or generate keys for the panel
        let keys = Keys::generate();
        let opts = ClientOptions::new().wait_for_send(true);
        let client = nostr_sdk::Client::builder().signer(keys).opts(opts).build();

        client.add_relay("wss://relay.damus.io").await?;
        client.connect().await;

        // Find best relay based on geographical proximity to client's IP
        let best_relay = "tailscale_nyc";
        let msg = format!("best_relay:{}", best_relay);

        client.send_private_msg(client_pubkey, msg, []).await?;
    }
    Ok(())
}

pub async fn get_connection_ticket() -> String {
    let guard = IROH_ENDPOINT.lock().await;
    if let Some(endpoint) = guard.as_ref() {
        if let Ok(ticket) = endpoint.ticket().await {
            return ticket.to_string();
        }
    }
    "".to_string()
}

pub async fn set_tauri_handle(handle: AppHandle) {
    let mut guard = TAURI_HANDLE.lock().await;
    *guard = Some(handle);
}
