use iroh::{Endpoint, NodeTicket};
use moq_transport::setup;
use std::sync::Arc;
use tokio::sync::Mutex;
use once_cell::sync::Lazy;
use tauri::{AppHandle, Emitter};
use base64::{engine::general_purpose, Engine as _};

pub static IROH_ENDPOINT: Lazy<Mutex<Option<Endpoint>>> = Lazy::new(|| Mutex::new(None));
pub static TAURI_HANDLE: Lazy<Mutex<Option<AppHandle>>> = Lazy::new(|| Mutex::new(None));

pub async fn init_iroh() -> anyhow::Result<()> {
    let secret = std::env::var("IROH_SERVICES_API_SECRET").unwrap_or_else(|_| "servicesaaqk4lc7mvaksxfm6yy6gyh57b4d65r7l77buth5xhklerbvk3ep5dqgcq5rfbcfdstm7arptsaypjntiqix26rdtwiawhdx4ws3q6oejyaa".to_string());

    let endpoint = Endpoint::builder()
        .discovery_n0()
        .bind()
        .await?;

    let mut guard = IROH_ENDPOINT.lock().await;
    *guard = Some(endpoint.clone());

    // Start listening for MoQ connections
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

async fn handle_moq_connection(conn: iroh::endpoint::Connecting) -> anyhow::Result<()> {
    let connection = conn.await?;
    let (mut session, mut control) = moq_transport::session::run(connection).await?;

    // Subscribe to the video track
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
