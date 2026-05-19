use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::task::JoinSet;
use tokio::time::timeout;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct DerpMap {
    pub regions: HashMap<String, DerpRegion>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct DerpRegion {
    pub region_id: u16,
    pub region_code: String,
    pub region_name: String,
    pub nodes: Vec<DerpNode>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct DerpNode {
    pub name: String,
    pub region_id: u16,
    pub host_name: String,
    pub ipv4: String,
    pub ipv6: Option<String>,
    pub can_port_80: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct RegionPing {
    pub region_id: u16,
    pub ping: u128, // milliseconds
}

pub async fn ping_region(region: &DerpRegion) -> Option<u128> {
    let mut best_ping = None;
    const MAX_NODES_PER_REGION: usize = 3;

    for node in region.nodes.iter().take(MAX_NODES_PER_REGION) {
        let start = Instant::now();
        let addr = format!("{}:443", node.ipv4);

        // Try to connect with a 2-second timeout
        match timeout(Duration::from_secs(2), TcpStream::connect(&addr)).await {
            Ok(Ok(_)) => {
                let duration = start.elapsed().as_millis();
                if best_ping.is_none() || duration < best_ping.unwrap() {
                    best_ping = Some(duration);
                }
            }
            _ => {
                // Try port 80 if 443 fails and it's supported
                if node.can_port_80.unwrap_or(false) {
                    let start = Instant::now();
                    let addr = format!("{}:80", node.ipv4);
                    match timeout(Duration::from_secs(2), TcpStream::connect(&addr)).await {
                        Ok(Ok(_)) => {
                            let duration = start.elapsed().as_millis();
                            if best_ping.is_none() || duration < best_ping.unwrap() {
                                best_ping = Some(duration);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    best_ping
}

pub async fn rank_regions(derp_map: &DerpMap) -> Vec<RegionPing> {
    const MAX_REGIONS: usize = 8;
    const GLOBAL_TIMEOUT_SECS: u64 = 8;

    let mut results = Vec::new();
    let mut ping_tasks = JoinSet::new();

    for region in derp_map.regions.values().take(MAX_REGIONS).cloned() {
        ping_tasks.spawn(async move {
            ping_region(&region).await.map(|ping| RegionPing {
                region_id: region.region_id,
                ping,
            })
        });
    }

    let collect = async {
        while let Some(joined) = ping_tasks.join_next().await {
            if let Ok(Some(region_ping)) = joined {
                results.push(region_ping);
            }
        }
    };

    let _ = timeout(Duration::from_secs(GLOBAL_TIMEOUT_SECS), collect).await;

    results.sort_by_key(|r| r.ping);
    results
}

pub fn get_negotiation_region() -> Result<DerpRegion, serde_json::Error> {
    serde_json::from_str(r#"{
      "RegionID": 14,
      "RegionCode": "ams",
      "RegionName": "Amsterdam",
      "Latitude": 52.372778,
      "Longitude": 4.893611,
      "Nodes": [
        {
          "Name": "14b",
          "RegionID": 14,
          "HostName": "derp14b.tailscale.com",
          "IPv4": "176.58.93.248",
          "IPv6": "2a00:dd80:3c::807",
          "CanPort80": true
        },
        {
          "Name": "14c",
          "RegionID": 14,
          "HostName": "derp14c.tailscale.com",
          "IPv4": "176.58.93.147",
          "IPv6": "2a00:dd80:3c::b09",
          "CanPort80": true
        },
        {
          "Name": "14d",
          "RegionID": 14,
          "HostName": "derp14d.tailscale.com",
          "IPv4": "176.58.93.154",
          "IPv6": "2a00:dd80:3c::3d5",
          "CanPort80": true
        }
      ]
    }"#)
}
