use iroh::{Endpoint, NodeAddr, RelayMap, RelayMode, RelayUrl};
use tokio::sync::Mutex;
use once_cell::sync::Lazy;
use tauri::AppHandle;
use nostr_sdk::{prelude::*, Options};
use std::str::FromStr;
use serde_json::Value;
use iroh_base::ticket::NodeTicket;

pub static IROH_ENDPOINT: Lazy<Mutex<Option<Endpoint>>> = Lazy::new(|| Mutex::new(None));
pub static TAURI_HANDLE: Lazy<Mutex<Option<AppHandle>>> = Lazy::new(|| Mutex::new(None));

pub async fn init_iroh() -> anyhow::Result<()> {
    let relay_map = fetch_tailscale_derp_map().await.unwrap_or_else(|_| RelayMap::empty());

    let endpoint = Endpoint::builder()
        .relay_mode(RelayMode::Custom(relay_map))
        .bind()
        .await?;

    let mut guard = IROH_ENDPOINT.lock().await;
    *guard = Some(endpoint.clone());

    tokio::spawn(async move {
        while let Some(incoming) = endpoint.accept().await {
            match incoming.accept() {
                Ok(connecting) => {
                    tokio::spawn(async move {
                        if let Err(e) = handle_incoming_connection(connecting).await {
                            eprintln!("Incoming connection error: {:?}", e);
                        }
                    });
                }
                Err(err) => {
                    eprintln!("Incoming connection accept error: {:?}", err);
                }
            }
        }
    });

    Ok(())
}

async fn fetch_tailscale_derp_map() -> anyhow::Result<RelayMap> {
    let client = reqwest::Client::new();
    let response = client.get("https://login.tailscale.com/derpmap/default").send().await?;
    let derp_map: Value = response.json().await?;

    if let Some(regions) = derp_map.get("Regions").and_then(|r| r.as_object()) {
        for region in regions.values() {
            if let Some(nodes) = region.get("Nodes").and_then(|n| n.as_array()) {
                for node in nodes {
                    if let Some(host) = node.get("HostName").and_then(|h| h.as_str()) {
                        let url = format!("https://{}", host);
                        if let Ok(relay_url) = RelayUrl::from_str(&url) {
                            return Ok(RelayMap::default_from_node(relay_url, 0));
                        }
                    }
                }
            }
        }
    }

    Ok(RelayMap::empty())
}

async fn handle_incoming_connection(conn: iroh::endpoint::Connecting) -> anyhow::Result<()> {
    let _connection = conn.await?;
    Ok(())
}

pub async fn negotiate_best_relay(_client_addr: String, client_pubkey: PublicKey) -> anyhow::Result<()> {
    let guard = TAURI_HANDLE.lock().await;
    if guard.as_ref().is_some() {
        let keys = Keys::generate();
        let opts = Options::new().wait_for_send(true);
        let client = nostr_sdk::Client::builder().signer(keys).opts(opts).build();

        client.add_relay("wss://relay.damus.io").await?;
        client.connect().await;

        let best_relay = "tailscale_nyc";
        let msg = format!("best_relay:{}", best_relay);

        client.send_private_msg(client_pubkey, msg, None).await?;
    }
    Ok(())
}

pub async fn get_connection_ticket() -> String {
    let guard = IROH_ENDPOINT.lock().await;
    if let Some(endpoint) = guard.as_ref() {
        let node_addr = NodeAddr::new(endpoint.node_id());
        let ticket = NodeTicket::new(node_addr);
        return ticket.to_string();
    }
    "".to_string()
}

pub async fn set_tauri_handle(handle: AppHandle) {
    let mut guard = TAURI_HANDLE.lock().await;
    *guard = Some(handle);
}
