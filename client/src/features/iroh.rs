use iroh::{Endpoint, SecretKey, NodeAddr};
use iroh_blobs::protocol::BlobsProtocol;
use iroh_blobs::store::mem::Store;
use iroh::protocol::Router;
use common::derp::{DerpMap, rank_regions, get_negotiation_region};
use std::sync::Arc;
use tokio::sync::Mutex;
use once_cell::sync::Lazy;
use common::packets::{IrohBlobInfo, BlobContext};
use common::client_info::IrohInfo;
use std::time::Duration;
use iroh::net::relay::{RelayMode, RelayMap, RelayNode};
use std::net::SocketAddr;
use std::str::FromStr;
use tokio::io::AsyncReadExt;

pub struct IrohNode {
    pub endpoint: Endpoint,
    pub store: Store,
    pub router: Router,
    pub info: IrohInfo,
    pub server_node_id: Option<String>,
    pub derp_map: Option<DerpMap>,
}

static IROH_NODE: Lazy<Mutex<Option<Arc<Mutex<IrohNode>>>>> = Lazy::new(|| Mutex::new(None));

pub async fn init_iroh() -> Result<IrohInfo, Box<dyn std::error::Error + Send + Sync>> {
    let secret_key = SecretKey::generate();
    let node_id = secret_key.public_key().to_string();

    let derp_map_url = "https://login.tailscale.com/derpmap/default";
    let client = reqwest::Client::new();

    let mut pings = Vec::new();
    let mut derp_map_opt = None;
    if let Ok(resp) = client.get(derp_map_url).send().await {
        if let Ok(derp_map) = resp.json::<DerpMap>().await {
            pings = rank_regions(&derp_map).await;
            derp_map_opt = Some(derp_map);
        }
    }

    let info = IrohInfo {
        node_id,
        derp_pings: pings,
    };

    let endpoint = Endpoint::builder()
        .secret_key(secret_key)
        .bind()
        .await?;

    let store = Store::memory();
    let blobs = BlobsProtocol::new(store.clone());

    let router = Router::builder(endpoint.clone())
        .accept(iroh_blobs::ALPN, blobs)
        .spawn()
        .await?;

    let node = Arc::new(Mutex::new(IrohNode {
        endpoint,
        store,
        router,
        info: info.clone(),
        server_node_id: None,
        derp_map: derp_map_opt,
    }));

    let mut lock = IROH_NODE.lock().await;
    *lock = Some(node);

    Ok(info)
}

pub async fn get_iroh_node() -> Option<Arc<Mutex<IrohNode>>> {
    IROH_NODE.lock().await.clone()
}

pub async fn add_blob(data: Vec<u8>, name: String, context: BlobContext) -> Option<IrohBlobInfo> {
    let node_arc = get_iroh_node().await?;
    let node = node_arc.lock().await;
    let size = data.len() as u64;

    // In iroh-blobs 0.29, import_bytes returns a TempTag which protects the blob from GC.
    // We'll let it drop immediately since we don't have a background GC running,
    // but we'll manually delete it later.
    let hash = node.store.import_bytes(data.into(), iroh_blobs::format::BlobFormat::Raw).await.ok()?.hash();

    let hash_str = hash.to_string();
    let store_clone = node.store.clone();

    // Auto-cleanup blob after 10 minutes to prevent memory leak
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(600)).await;
        let _ = iroh_blobs::store::Store::delete(&store_clone, vec![hash]).await;
    });

    Some(IrohBlobInfo {
        hash: hash_str,
        name,
        size,
        context,
    })
}

pub async fn delete_blob(hash_str: String) -> bool {
    if let Some(node_arc) = get_iroh_node().await {
        let node = node_arc.lock().await;
        if let Ok(hash) = iroh_blobs::Hash::from_hex(&hash_str) {
            return iroh_blobs::store::Store::delete(&node.store, vec![hash]).await.is_ok();
        }
    }
    false
}

pub async fn set_iroh_config(server_node_id: String, region_id: u16) {
    if let Some(node_arc) = get_iroh_node().await {
        let mut node = node_arc.lock().await;
        node.server_node_id = Some(server_node_id);

        if let Some(derp_map) = &node.derp_map {
            if let Some(region) = derp_map.regions.values().find(|r| r.region_id == region_id) {
                let mut nodes = Vec::new();
                for n in &region.nodes {
                    if let Ok(url) = iroh::net::util::Url::from_str(&format!("https://{}", n.host_name)) {
                        nodes.push(RelayNode {
                            url,
                            stun_only: false,
                            stun_port: 3478,
                        });
                    }
                }

                if !nodes.is_empty() {
                    let relay_map = RelayMap::from_nodes(nodes).unwrap();
                    let _ = node.endpoint.set_relay_mode(RelayMode::Custom(relay_map));
                    println!("Switched Iroh relay to region {} ({})", region.region_id, region.region_code);
                }
            }
        }
    }
}

pub async fn download_blob(peer_node_id: String, blob_info: IrohBlobInfo) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let node_arc = get_iroh_node().await.ok_or("Iroh node not initialized")?;
    let node = node_arc.lock().await;
    let peer_public_key: iroh::PublicKey = peer_node_id.parse()?;
    let addr = NodeAddr::new(peer_public_key);

    let hash = iroh_blobs::Hash::from_hex(&blob_info.hash)?;
    let mut stream = iroh_blobs::get::blobs::get_to_reader(&node.endpoint, addr, hash).await?;
    let mut buffer = Vec::with_capacity(blob_info.size as usize);
    stream.read_to_end(&mut buffer).await?;

    Ok(buffer)
}
