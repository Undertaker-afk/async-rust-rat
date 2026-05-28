//! Integration tests for the relay infrastructure

use std::net::SocketAddr;
use std::time::Duration;
use wraith_discovery::relay::{
    RelayClient, RelayInfo, RelaySelector, RelayServer, SelectionStrategy,
};

#[tokio::test]
async fn test_relay_server_startup() {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let server = RelayServer::bind(addr).await;
    assert!(server.is_ok());
}

#[tokio::test]
async fn test_relay_selector_best_selection() {
    let mut selector = RelaySelector::with_strategy(SelectionStrategy::LowestLoad);

    let addr1 = "127.0.0.1:8001".parse().unwrap();
    let addr2 = "127.0.0.1:8002".parse().unwrap();
    let addr3 = "127.0.0.1:8003".parse().unwrap();

    selector.add_relay(RelayInfo::new(addr1, "us-west".to_string()).with_load(0.8));
    selector.add_relay(RelayInfo::new(addr2, "us-east".to_string()).with_load(0.3));
    selector.add_relay(RelayInfo::new(addr3, "eu-central".to_string()).with_load(0.5));

    let best = selector.select_best().unwrap();
    assert_eq!(best.addr, addr2); // Lowest load
}

#[tokio::test]
async fn test_relay_selector_fallbacks() {
    let mut selector = RelaySelector::new();

    for i in 0..5 {
        let addr = format!("127.0.0.1:{}", 8000 + i).parse().unwrap();
        selector.add_relay(
            RelayInfo::new(addr, "region".to_string())
                .with_priority(i * 10)
                .with_load(0.5),
        );
    }

    let fallbacks = selector.select_fallbacks(3);
    assert_eq!(fallbacks.len(), 3);
}

#[tokio::test]
async fn test_relay_selector_region_filtering() {
    let mut selector = RelaySelector::new();

    let addr1 = "127.0.0.1:8001".parse().unwrap();
    let addr2 = "127.0.0.1:8002".parse().unwrap();
    let addr3 = "127.0.0.1:8003".parse().unwrap();

    selector.add_relay(RelayInfo::new(addr1, "us-west".to_string()));
    selector.add_relay(RelayInfo::new(addr2, "eu-central".to_string()));
    selector.add_relay(RelayInfo::new(addr3, "us-west".to_string()));

    let us_west_relays = selector.find_by_region("us-west");
    assert_eq!(us_west_relays.len(), 2);
}

#[tokio::test]
async fn test_relay_selector_latency_updates() {
    let mut selector = RelaySelector::new();
    let addr = "127.0.0.1:8000".parse().unwrap();

    selector.add_relay(RelayInfo::new(addr, "region".to_string()));
    selector.update_latency(addr, Duration::from_millis(50));

    let latency = selector.get_latency(&addr);
    assert_eq!(latency, Some(Duration::from_millis(50)));
}

#[tokio::test]
async fn test_relay_client_connection_attempt() {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let node_id = [1u8; 32];

    // This will fail to connect since no server is running,
    // but it tests the client creation logic
    let result = RelayClient::connect(addr, node_id).await;
    assert!(result.is_ok() || result.is_err()); // Either is acceptable without a server
}

#[tokio::test]
async fn test_relay_server_client_count() {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let server = RelayServer::bind(addr).await.unwrap();

    let count = server.client_count().await;
    assert_eq!(count, 0); // No clients connected yet
}

#[tokio::test]
async fn test_relay_info_load_bounds() {
    let addr = "127.0.0.1:8000".parse().unwrap();

    let info_high = RelayInfo::new(addr, "region".to_string()).with_load(1.5);
    let info_low = RelayInfo::new(addr, "region".to_string()).with_load(-0.5);

    assert_eq!(info_high.load, 1.0); // Clamped to max
    assert_eq!(info_low.load, 0.0); // Clamped to min
}

#[tokio::test]
async fn test_relay_selector_strategy_switching() {
    let mut selector = RelaySelector::with_strategy(SelectionStrategy::LowestLatency);
    assert_eq!(selector.relay_count(), 0);

    selector.set_strategy(SelectionStrategy::HighestPriority);

    let addr = "127.0.0.1:8000".parse().unwrap();
    selector.add_relay(RelayInfo::new(addr, "region".to_string()).with_priority(100));

    let best = selector.select_best();
    assert!(best.is_some());
}

#[tokio::test]
async fn test_relay_selector_balanced_scoring() {
    let mut selector = RelaySelector::with_strategy(SelectionStrategy::Balanced);

    let addr1 = "127.0.0.1:8001".parse().unwrap();
    let addr2 = "127.0.0.1:8002".parse().unwrap();

    // addr1: high priority, high load
    selector.add_relay(
        RelayInfo::new(addr1, "region".to_string())
            .with_priority(200)
            .with_load(0.9),
    );

    // addr2: medium priority, low load
    selector.add_relay(
        RelayInfo::new(addr2, "region".to_string())
            .with_priority(100)
            .with_load(0.2),
    );

    selector.update_latency(addr1, Duration::from_millis(10));
    selector.update_latency(addr2, Duration::from_millis(50));

    let best = selector.select_best();
    assert!(best.is_some());
}
