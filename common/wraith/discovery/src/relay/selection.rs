//! Relay server selection algorithm.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

/// Relay server information
#[derive(Debug, Clone)]
pub struct RelayInfo {
    /// Relay server address
    pub addr: SocketAddr,
    /// Geographic region (e.g., "us-west", "eu-central")
    pub region: String,
    /// Current server load (0.0 = empty, 1.0 = full)
    pub load: f32,
    /// Priority (higher = more preferred)
    pub priority: u32,
}

impl RelayInfo {
    /// Create a new relay info
    #[must_use]
    pub fn new(addr: SocketAddr, region: String) -> Self {
        Self {
            addr,
            region,
            load: 0.0,
            priority: 100,
        }
    }

    /// Set load value
    #[must_use]
    pub fn with_load(mut self, load: f32) -> Self {
        self.load = load.clamp(0.0, 1.0);
        self
    }

    /// Set priority
    #[must_use]
    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }
}

/// Relay selection strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionStrategy {
    /// Select relay with lowest latency
    LowestLatency,
    /// Select relay with lowest load
    LowestLoad,
    /// Select relay with highest priority
    HighestPriority,
    /// Balanced selection (weighted combination)
    Balanced,
}

/// Relay selector for choosing the best relay server
pub struct RelaySelector {
    /// Available relay servers
    relays: Vec<RelayInfo>,
    /// Measured latencies (relay addr -> latency)
    latencies: HashMap<SocketAddr, Duration>,
    /// Selection strategy
    strategy: SelectionStrategy,
    /// Last latency measurement time
    last_measurement: HashMap<SocketAddr, Instant>,
}

impl RelaySelector {
    /// Create a new relay selector
    #[must_use]
    pub fn new() -> Self {
        Self {
            relays: Vec::new(),
            latencies: HashMap::new(),
            strategy: SelectionStrategy::Balanced,
            last_measurement: HashMap::new(),
        }
    }

    /// Create a new relay selector with specific strategy
    #[must_use]
    pub fn with_strategy(strategy: SelectionStrategy) -> Self {
        Self {
            relays: Vec::new(),
            latencies: HashMap::new(),
            strategy,
            last_measurement: HashMap::new(),
        }
    }

    /// Add a relay server to the selection pool
    pub fn add_relay(&mut self, relay: RelayInfo) {
        self.relays.push(relay);
    }

    /// Remove a relay server by address
    pub fn remove_relay(&mut self, addr: &SocketAddr) {
        self.relays.retain(|r| r.addr != *addr);
        self.latencies.remove(addr);
        self.last_measurement.remove(addr);
    }

    /// Set selection strategy
    pub fn set_strategy(&mut self, strategy: SelectionStrategy) {
        self.strategy = strategy;
    }

    /// Select the best relay based on current strategy
    ///
    /// Returns `None` if no relays are available.
    #[must_use]
    pub fn select_best(&self) -> Option<&RelayInfo> {
        if self.relays.is_empty() {
            return None;
        }

        match self.strategy {
            SelectionStrategy::LowestLatency => self.select_lowest_latency(),
            SelectionStrategy::LowestLoad => self.select_lowest_load(),
            SelectionStrategy::HighestPriority => self.select_highest_priority(),
            SelectionStrategy::Balanced => self.select_balanced(),
        }
    }

    /// Select relay with lowest latency
    fn select_lowest_latency(&self) -> Option<&RelayInfo> {
        self.relays
            .iter()
            .min_by_key(|relay| self.latencies.get(&relay.addr).copied())
    }

    /// Select relay with lowest load
    fn select_lowest_load(&self) -> Option<&RelayInfo> {
        self.relays
            .iter()
            .min_by(|a, b| a.load.partial_cmp(&b.load).unwrap())
    }

    /// Select relay with highest priority
    fn select_highest_priority(&self) -> Option<&RelayInfo> {
        self.relays.iter().max_by_key(|relay| relay.priority)
    }

    /// Select relay using balanced algorithm
    ///
    /// Scoring: priority * 0.4 - load * 0.3 - latency_score * 0.3
    fn select_balanced(&self) -> Option<&RelayInfo> {
        if self.relays.is_empty() {
            return None;
        }

        // Calculate score for each relay
        let mut best_relay: Option<&RelayInfo> = None;
        let mut best_score = f64::NEG_INFINITY;

        for relay in &self.relays {
            let priority_score = f64::from(relay.priority) * 0.4;
            let load_score = -f64::from(relay.load) * 30.0; // Negative because lower is better

            let latency_score = if let Some(latency) = self.latencies.get(&relay.addr) {
                -(latency.as_millis() as f64) * 0.003 // Negative, normalized
            } else {
                -10.0 // Penalty for unknown latency
            };

            let total_score = priority_score + load_score + latency_score;

            if total_score > best_score {
                best_score = total_score;
                best_relay = Some(relay);
            }
        }

        best_relay
    }

    /// Select multiple fallback relays
    ///
    /// Returns up to `count` relays, ordered by preference.
    #[must_use]
    pub fn select_fallbacks(&self, count: usize) -> Vec<&RelayInfo> {
        let mut relays: Vec<&RelayInfo> = self.relays.iter().collect();

        // Sort by balanced score
        relays.sort_by(|a, b| {
            let score_a = self.calculate_score(a);
            let score_b = self.calculate_score(b);
            score_b.partial_cmp(&score_a).unwrap()
        });

        relays.into_iter().take(count).collect()
    }

    /// Calculate score for a relay (used for sorting)
    fn calculate_score(&self, relay: &RelayInfo) -> f64 {
        let priority_score = f64::from(relay.priority) * 0.4;
        let load_score = -f64::from(relay.load) * 30.0;

        let latency_score = if let Some(latency) = self.latencies.get(&relay.addr) {
            -(latency.as_millis() as f64) * 0.003
        } else {
            -10.0
        };

        priority_score + load_score + latency_score
    }

    /// Measure latency to a relay server
    ///
    /// This is a placeholder for actual network measurement.
    /// In production, this would send PING and measure RTT.
    pub async fn measure_latency(&mut self, addr: SocketAddr) {
        // Placeholder implementation
        // In production, send ICMP ping or UDP probe
        let latency = Duration::from_millis(50); // Simulated latency

        self.latencies.insert(addr, latency);
        self.last_measurement.insert(addr, Instant::now());
    }

    /// Update latency measurement for a relay
    pub fn update_latency(&mut self, addr: SocketAddr, latency: Duration) {
        self.latencies.insert(addr, latency);
        self.last_measurement.insert(addr, Instant::now());
    }

    /// Update load information for a relay
    pub fn update_load(&mut self, addr: SocketAddr, load: f32) {
        if let Some(relay) = self.relays.iter_mut().find(|r| r.addr == addr) {
            relay.load = load.clamp(0.0, 1.0);
        }
    }

    /// Get latency for a relay
    #[must_use]
    pub fn get_latency(&self, addr: &SocketAddr) -> Option<Duration> {
        self.latencies.get(addr).copied()
    }

    /// Get all available relays
    #[must_use]
    pub fn relays(&self) -> &[RelayInfo] {
        &self.relays
    }

    /// Get number of available relays
    #[must_use]
    pub fn relay_count(&self) -> usize {
        self.relays.len()
    }

    /// Check if latency measurement is stale
    #[must_use]
    pub fn is_measurement_stale(&self, addr: &SocketAddr, threshold: Duration) -> bool {
        if let Some(last) = self.last_measurement.get(addr) {
            last.elapsed() > threshold
        } else {
            true
        }
    }

    /// Find relays in a specific region
    #[must_use]
    pub fn find_by_region(&self, region: &str) -> Vec<&RelayInfo> {
        self.relays.iter().filter(|r| r.region == region).collect()
    }
}

impl Default for RelaySelector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relay_info_creation() {
        let addr = "127.0.0.1:443".parse().unwrap();
        let info = RelayInfo::new(addr, "us-west".to_string());

        assert_eq!(info.addr, addr);
        assert_eq!(info.region, "us-west");
        assert_eq!(info.load, 0.0);
        assert_eq!(info.priority, 100);
    }

    #[test]
    fn test_relay_info_builder() {
        let addr = "127.0.0.1:443".parse().unwrap();
        let info = RelayInfo::new(addr, "eu-central".to_string())
            .with_load(0.5)
            .with_priority(200);

        assert_eq!(info.load, 0.5);
        assert_eq!(info.priority, 200);
    }

    #[test]
    fn test_relay_info_load_clamping() {
        let addr = "127.0.0.1:443".parse().unwrap();
        let info1 = RelayInfo::new(addr, "region".to_string()).with_load(1.5);
        let info2 = RelayInfo::new(addr, "region".to_string()).with_load(-0.5);

        assert_eq!(info1.load, 1.0);
        assert_eq!(info2.load, 0.0);
    }

    #[test]
    fn test_relay_selector_creation() {
        let selector = RelaySelector::new();
        assert_eq!(selector.relay_count(), 0);
        assert_eq!(selector.strategy, SelectionStrategy::Balanced);
    }

    #[test]
    fn test_relay_selector_with_strategy() {
        let selector = RelaySelector::with_strategy(SelectionStrategy::LowestLatency);
        assert_eq!(selector.strategy, SelectionStrategy::LowestLatency);
    }

    #[test]
    fn test_relay_selector_add_remove() {
        let mut selector = RelaySelector::new();
        let addr = "127.0.0.1:443".parse().unwrap();
        let relay = RelayInfo::new(addr, "region".to_string());

        selector.add_relay(relay);
        assert_eq!(selector.relay_count(), 1);

        selector.remove_relay(&addr);
        assert_eq!(selector.relay_count(), 0);
    }

    #[test]
    fn test_select_best_empty() {
        let selector = RelaySelector::new();
        assert!(selector.select_best().is_none());
    }

    #[test]
    fn test_select_best_single() {
        let mut selector = RelaySelector::new();
        let addr = "127.0.0.1:443".parse().unwrap();
        let relay = RelayInfo::new(addr, "region".to_string());

        selector.add_relay(relay);
        let best = selector.select_best();
        assert!(best.is_some());
        assert_eq!(best.unwrap().addr, addr);
    }

    #[test]
    fn test_select_lowest_load() {
        let mut selector = RelaySelector::with_strategy(SelectionStrategy::LowestLoad);

        let addr1 = "127.0.0.1:443".parse().unwrap();
        let addr2 = "127.0.0.1:444".parse().unwrap();

        selector.add_relay(RelayInfo::new(addr1, "region".to_string()).with_load(0.8));
        selector.add_relay(RelayInfo::new(addr2, "region".to_string()).with_load(0.3));

        let best = selector.select_best().unwrap();
        assert_eq!(best.addr, addr2);
    }

    #[test]
    fn test_select_highest_priority() {
        let mut selector = RelaySelector::with_strategy(SelectionStrategy::HighestPriority);

        let addr1 = "127.0.0.1:443".parse().unwrap();
        let addr2 = "127.0.0.1:444".parse().unwrap();

        selector.add_relay(RelayInfo::new(addr1, "region".to_string()).with_priority(100));
        selector.add_relay(RelayInfo::new(addr2, "region".to_string()).with_priority(200));

        let best = selector.select_best().unwrap();
        assert_eq!(best.addr, addr2);
    }

    #[test]
    fn test_select_fallbacks() {
        let mut selector = RelaySelector::new();

        for i in 0..5 {
            let addr = format!("127.0.0.1:{}", 443 + i).parse().unwrap();
            selector.add_relay(RelayInfo::new(addr, "region".to_string()).with_priority(i));
        }

        let fallbacks = selector.select_fallbacks(3);
        assert_eq!(fallbacks.len(), 3);
    }

    #[test]
    fn test_update_latency() {
        let mut selector = RelaySelector::new();
        let addr = "127.0.0.1:443".parse().unwrap();

        selector.update_latency(addr, Duration::from_millis(25));
        assert_eq!(selector.get_latency(&addr), Some(Duration::from_millis(25)));
    }

    #[test]
    fn test_update_load() {
        let mut selector = RelaySelector::new();
        let addr = "127.0.0.1:443".parse().unwrap();
        selector.add_relay(RelayInfo::new(addr, "region".to_string()));

        selector.update_load(addr, 0.75);

        let relay = selector.relays().iter().find(|r| r.addr == addr).unwrap();
        assert_eq!(relay.load, 0.75);
    }

    #[test]
    fn test_find_by_region() {
        let mut selector = RelaySelector::new();

        let addr1 = "127.0.0.1:443".parse().unwrap();
        let addr2 = "127.0.0.1:444".parse().unwrap();
        let addr3 = "127.0.0.1:445".parse().unwrap();

        selector.add_relay(RelayInfo::new(addr1, "us-west".to_string()));
        selector.add_relay(RelayInfo::new(addr2, "eu-central".to_string()));
        selector.add_relay(RelayInfo::new(addr3, "us-west".to_string()));

        let us_west = selector.find_by_region("us-west");
        assert_eq!(us_west.len(), 2);

        let eu = selector.find_by_region("eu-central");
        assert_eq!(eu.len(), 1);
    }

    #[test]
    fn test_is_measurement_stale() {
        let mut selector = RelaySelector::new();
        let addr = "127.0.0.1:443".parse().unwrap();

        assert!(selector.is_measurement_stale(&addr, Duration::from_secs(60)));

        selector.update_latency(addr, Duration::from_millis(50));
        assert!(!selector.is_measurement_stale(&addr, Duration::from_secs(60)));
    }
}
