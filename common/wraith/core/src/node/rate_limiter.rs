//! Rate limiting and DoS protection
//!
//! Implements token bucket algorithm for:
//! - Connection rate limiting (max connections per IP per minute)
//! - Packet rate limiting (max packets per session per second)
//! - Bandwidth limiting (max bytes per session per second)
//! - Global session limit (max concurrent sessions)

use dashmap::DashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Rate limiter configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum connections per IP per minute
    pub max_connections_per_ip_per_minute: u32,

    /// Maximum packets per session per second
    pub max_packets_per_session_per_second: u32,

    /// Maximum bytes per session per second (bandwidth limit)
    pub max_bytes_per_session_per_second: u64,

    /// Maximum concurrent sessions globally
    pub max_concurrent_sessions: usize,

    /// Token bucket refill interval
    pub refill_interval: Duration,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_connections_per_ip_per_minute: 10,
            max_packets_per_session_per_second: 1000,
            max_bytes_per_session_per_second: 10 * 1024 * 1024, // 10 MB/s
            max_concurrent_sessions: 1000,
            refill_interval: Duration::from_millis(100),
        }
    }
}

/// Token bucket for rate limiting
#[derive(Debug, Clone)]
struct TokenBucket {
    /// Current number of tokens
    tokens: f64,

    /// Maximum tokens (capacity)
    max_tokens: f64,

    /// Tokens added per refill
    refill_rate: f64,

    /// Last refill time
    last_refill: Instant,

    /// Refill interval
    refill_interval: Duration,
}

impl TokenBucket {
    /// Create a new token bucket
    fn new(capacity: f64, refill_rate: f64, refill_interval: Duration) -> Self {
        Self {
            tokens: capacity,
            max_tokens: capacity,
            refill_rate,
            last_refill: Instant::now(),
            refill_interval,
        }
    }

    /// Refill tokens based on elapsed time
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill);

        if elapsed >= self.refill_interval {
            let refills = elapsed.as_secs_f64() / self.refill_interval.as_secs_f64();
            let tokens_to_add = self.refill_rate * refills;
            self.tokens = (self.tokens + tokens_to_add).min(self.max_tokens);
            self.last_refill = now;
        }
    }

    /// Try to consume tokens
    fn try_consume(&mut self, amount: f64) -> bool {
        self.refill();

        if self.tokens >= amount {
            self.tokens -= amount;
            true
        } else {
            false
        }
    }

    /// Get current token count
    #[cfg(test)]
    fn available(&self) -> f64 {
        self.tokens
    }
}

/// Rate limiter using token bucket algorithm
pub struct RateLimiter {
    /// Configuration
    config: RateLimitConfig,

    /// Per-IP connection rate limiting
    ip_buckets: Arc<DashMap<IpAddr, TokenBucket>>,

    /// Per-session packet rate limiting
    session_packet_buckets: Arc<DashMap<[u8; 32], TokenBucket>>,

    /// Per-session bandwidth limiting
    session_bandwidth_buckets: Arc<DashMap<[u8; 32], TokenBucket>>,

    /// Current session count
    current_sessions: Arc<AtomicUsize>,

    /// Metrics (using atomic counters for lock-free updates)
    connections_blocked: Arc<AtomicU64>,
    packets_blocked: Arc<AtomicU64>,
    bytes_blocked: Arc<AtomicU64>,
    session_limit_hits: Arc<AtomicU64>,
    connections_allowed: Arc<AtomicU64>,
    packets_allowed: Arc<AtomicU64>,
    bytes_allowed: Arc<AtomicU64>,
}

/// Rate limiting metrics
#[derive(Debug, Default, Clone)]
pub struct RateLimitMetrics {
    /// Total connection attempts blocked
    pub connections_blocked: u64,

    /// Total packets blocked
    pub packets_blocked: u64,

    /// Total bytes blocked
    pub bytes_blocked: u64,

    /// Session limit hits
    pub session_limit_hits: u64,

    /// Total connection attempts allowed
    pub connections_allowed: u64,

    /// Total packets allowed
    pub packets_allowed: u64,

    /// Total bytes allowed
    pub bytes_allowed: u64,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            ip_buckets: Arc::new(DashMap::new()),
            session_packet_buckets: Arc::new(DashMap::new()),
            session_bandwidth_buckets: Arc::new(DashMap::new()),
            current_sessions: Arc::new(AtomicUsize::new(0)),
            connections_blocked: Arc::new(AtomicU64::new(0)),
            packets_blocked: Arc::new(AtomicU64::new(0)),
            bytes_blocked: Arc::new(AtomicU64::new(0)),
            session_limit_hits: Arc::new(AtomicU64::new(0)),
            connections_allowed: Arc::new(AtomicU64::new(0)),
            packets_allowed: Arc::new(AtomicU64::new(0)),
            bytes_allowed: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Check if a connection from the given IP is allowed
    pub fn check_connection(&self, ip: IpAddr) -> bool {
        let mut entry = self.ip_buckets.entry(ip).or_insert_with(|| {
            TokenBucket::new(
                self.config.max_connections_per_ip_per_minute as f64,
                self.config.max_connections_per_ip_per_minute as f64 / 60.0, // Per second
                self.config.refill_interval,
            )
        });

        if entry.try_consume(1.0) {
            self.connections_allowed.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            self.connections_blocked.fetch_add(1, Ordering::Relaxed);
            false
        }
    }

    /// Check if a packet from the given session is allowed
    pub fn check_packet(&self, session_id: &[u8; 32]) -> bool {
        let mut entry = self
            .session_packet_buckets
            .entry(*session_id)
            .or_insert_with(|| {
                TokenBucket::new(
                    self.config.max_packets_per_session_per_second as f64,
                    self.config.max_packets_per_session_per_second as f64,
                    self.config.refill_interval,
                )
            });

        if entry.try_consume(1.0) {
            self.packets_allowed.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            self.packets_blocked.fetch_add(1, Ordering::Relaxed);
            false
        }
    }

    /// Check if bandwidth usage is allowed for the given session
    pub fn check_bandwidth(&self, session_id: &[u8; 32], bytes: u64) -> bool {
        let mut entry = self
            .session_bandwidth_buckets
            .entry(*session_id)
            .or_insert_with(|| {
                TokenBucket::new(
                    self.config.max_bytes_per_session_per_second as f64,
                    self.config.max_bytes_per_session_per_second as f64,
                    self.config.refill_interval,
                )
            });

        if entry.try_consume(bytes as f64) {
            self.bytes_allowed.fetch_add(bytes, Ordering::Relaxed);
            true
        } else {
            self.bytes_blocked.fetch_add(bytes, Ordering::Relaxed);
            false
        }
    }

    /// Check if a new session can be created (global limit)
    pub fn check_session_limit(&self) -> bool {
        let current = self.current_sessions.load(Ordering::Relaxed);

        if current < self.config.max_concurrent_sessions {
            true
        } else {
            self.session_limit_hits.fetch_add(1, Ordering::Relaxed);
            false
        }
    }

    /// Increment session count
    pub fn increment_sessions(&self) {
        self.current_sessions.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement session count
    pub fn decrement_sessions(&self) {
        // Use fetch_sub with wrapping, but check first to avoid underflow
        let current = self.current_sessions.load(Ordering::Relaxed);
        if current > 0 {
            self.current_sessions.fetch_sub(1, Ordering::Relaxed);
        }
    }

    /// Remove session from rate limiting
    pub fn remove_session(&self, session_id: &[u8; 32]) {
        self.session_packet_buckets.remove(session_id);
        self.session_bandwidth_buckets.remove(session_id);
        self.decrement_sessions();
    }

    /// Clean up stale IP buckets (no activity in last hour)
    pub fn cleanup_stale_buckets(&self) {
        let now = Instant::now();
        self.ip_buckets
            .retain(|_, bucket| now.duration_since(bucket.last_refill) < Duration::from_secs(3600));
    }

    /// Get current metrics
    pub fn metrics(&self) -> RateLimitMetrics {
        RateLimitMetrics {
            connections_blocked: self.connections_blocked.load(Ordering::Relaxed),
            packets_blocked: self.packets_blocked.load(Ordering::Relaxed),
            bytes_blocked: self.bytes_blocked.load(Ordering::Relaxed),
            session_limit_hits: self.session_limit_hits.load(Ordering::Relaxed),
            connections_allowed: self.connections_allowed.load(Ordering::Relaxed),
            packets_allowed: self.packets_allowed.load(Ordering::Relaxed),
            bytes_allowed: self.bytes_allowed.load(Ordering::Relaxed),
        }
    }

    /// Get current session count
    pub fn current_session_count(&self) -> usize {
        self.current_sessions.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_bucket_basic() {
        let mut bucket = TokenBucket::new(10.0, 10.0, Duration::from_millis(100));

        // Should be able to consume up to capacity
        assert!(bucket.try_consume(5.0));
        assert_eq!(bucket.available(), 5.0);

        assert!(bucket.try_consume(5.0));
        assert_eq!(bucket.available(), 0.0);

        // Should fail when empty
        assert!(!bucket.try_consume(1.0));
    }

    #[tokio::test]
    async fn test_token_bucket_refill() {
        let mut bucket = TokenBucket::new(10.0, 10.0, Duration::from_millis(100));

        // Consume all tokens
        assert!(bucket.try_consume(10.0));
        assert_eq!(bucket.available(), 0.0);

        // Wait for refill
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should have refilled
        assert!(bucket.try_consume(1.0));
    }

    #[test]
    fn test_rate_limiter_connection() {
        let config = RateLimitConfig {
            max_connections_per_ip_per_minute: 5,
            ..Default::default()
        };
        let limiter = RateLimiter::new(config);
        let ip = "127.0.0.1".parse().unwrap();

        // Should allow first 5 connections
        for _ in 0..5 {
            assert!(limiter.check_connection(ip));
        }

        // Should block 6th connection
        assert!(!limiter.check_connection(ip));

        // Check metrics
        let metrics = limiter.metrics();
        assert_eq!(metrics.connections_allowed, 5);
        assert_eq!(metrics.connections_blocked, 1);
    }

    #[test]
    fn test_rate_limiter_packet() {
        let config = RateLimitConfig {
            max_packets_per_session_per_second: 10,
            ..Default::default()
        };
        let limiter = RateLimiter::new(config);
        let session_id = [1u8; 32];

        // Should allow first 10 packets
        for _ in 0..10 {
            assert!(limiter.check_packet(&session_id));
        }

        // Should block 11th packet
        assert!(!limiter.check_packet(&session_id));

        // Check metrics
        let metrics = limiter.metrics();
        assert_eq!(metrics.packets_allowed, 10);
        assert_eq!(metrics.packets_blocked, 1);
    }

    #[test]
    fn test_rate_limiter_bandwidth() {
        let config = RateLimitConfig {
            max_bytes_per_session_per_second: 1000,
            ..Default::default()
        };
        let limiter = RateLimiter::new(config);
        let session_id = [2u8; 32];

        // Should allow first 1000 bytes
        assert!(limiter.check_bandwidth(&session_id, 500));
        assert!(limiter.check_bandwidth(&session_id, 500));

        // Should block additional bytes
        assert!(!limiter.check_bandwidth(&session_id, 1));

        // Check metrics
        let metrics = limiter.metrics();
        assert_eq!(metrics.bytes_allowed, 1000);
        assert_eq!(metrics.bytes_blocked, 1);
    }

    #[test]
    fn test_rate_limiter_session_limit() {
        let config = RateLimitConfig {
            max_concurrent_sessions: 3,
            ..Default::default()
        };
        let limiter = RateLimiter::new(config);

        // Should allow first 3 sessions
        for _ in 0..3 {
            assert!(limiter.check_session_limit());
            limiter.increment_sessions();
        }

        // Should block 4th session
        assert!(!limiter.check_session_limit());

        // Decrement and try again
        limiter.decrement_sessions();
        assert!(limiter.check_session_limit());
    }

    #[test]
    fn test_rate_limiter_remove_session() {
        let limiter = RateLimiter::new(RateLimitConfig::default());
        let session_id = [3u8; 32];

        // Add session
        limiter.increment_sessions();
        assert!(limiter.check_packet(&session_id));

        // Remove session
        limiter.remove_session(&session_id);

        // Session count should be decremented
        assert_eq!(limiter.current_session_count(), 0);
    }

    #[test]
    fn test_rate_limiter_cleanup() {
        let limiter = RateLimiter::new(RateLimitConfig::default());
        let ip = "192.168.1.1".parse().unwrap();

        // Create bucket
        assert!(limiter.check_connection(ip));

        // Cleanup should not remove recent bucket
        limiter.cleanup_stale_buckets();
        assert_eq!(limiter.ip_buckets.len(), 1);
    }
}
