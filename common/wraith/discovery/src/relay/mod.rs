//! # DERP-Style Relay Infrastructure
//!
//! Provides relay infrastructure for NAT traversal when direct connections fail.
//! Inspired by Tailscale's DERP (Designated Encrypted Relay for Packets).
//!
//! ## Features
//!
//! - Client registration with relay servers
//! - Encrypted packet forwarding between peers
//! - Geographic and latency-based relay selection
//! - Automatic failover to backup relays
//! - End-to-end encryption (relay cannot decrypt)
//!
//! ## Architecture
//!
//! ```text
//!                    ┌─────────────────┐
//!                    │   Relay Server  │
//!                    │  (Public IP)    │
//!                    └────────┬────────┘
//!                             │
//!              ┌──────────────┴──────────────┐
//!              │                              │
//!              ▼                              ▼
//!       ┌─────────────┐                ┌─────────────┐
//!       │   Peer A    │                │   Peer B    │
//!       │  (NAT'd)    │                │  (NAT'd)    │
//!       └─────────────┘                └─────────────┘
//! ```
//!
//! ## Example
//!
//! ```rust,no_run
//! use wraith_discovery::relay::{RelayClient, RelaySelector, RelayInfo};
//! use std::net::SocketAddr;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Select best relay
//! let mut selector = RelaySelector::new();
//! selector.add_relay(RelayInfo {
//!     addr: "relay.example.com:443".parse()?,
//!     region: "us-west".to_string(),
//!     load: 0.3,
//!     priority: 100,
//! });
//!
//! let relay_info = selector.select_best().unwrap();
//!
//! // Connect to relay
//! let node_id = [1u8; 32]; // Your node ID
//! let mut client = RelayClient::connect(relay_info.addr, node_id).await?;
//!
//! let public_key = [2u8; 32];
//! client.register(&public_key).await?;
//!
//! // Send packet through relay
//! let dest_id = [3u8; 32];
//! client.send_to_peer(dest_id, b"hello").await?;
//!
//! // Receive forwarded packets
//! let (from, data) = client.recv_from_peer().await?;
//! # Ok(())
//! # }
//! ```

pub mod client;
pub mod protocol;
pub mod selection;
pub mod server;

pub use client::RelayClient;
pub use protocol::{RelayError, RelayErrorCode, RelayMessage};
pub use selection::{RelayInfo, RelaySelector, SelectionStrategy};
pub use server::{RelayServer, RelayServerConfig};

/// Default relay port (HTTPS)
pub const DEFAULT_RELAY_PORT: u16 = 443;

/// Maximum relay packet size (64 KB)
pub const MAX_RELAY_PACKET_SIZE: usize = 65536;

/// Relay keepalive interval (30 seconds)
pub const RELAY_KEEPALIVE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);

/// Relay connection timeout (10 seconds)
pub const RELAY_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
