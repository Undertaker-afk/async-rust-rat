//! Kademlia DHT Implementation
//!
//! This module provides a privacy-enhanced Kademlia DHT for peer discovery
//! in the WRAITH protocol. Key features include:
//!
//! - 256-bit node identifiers derived from public keys using BLAKE3
//! - XOR distance metric for efficient routing
//! - K-bucket routing table with LRU eviction (k=20)
//! - Encrypted DHT messages using wraith-crypto AEAD
//! - Iterative lookup with alpha parallelism (α=3)
//! - Bootstrap mechanism for network join
//! - S/Kademlia Sybil resistance with crypto puzzles (SEC-001)
//! - Privacy-enhanced key derivation with group secrets (SEC-002)
//!
//! # Example Usage
//!
//! ```rust,no_run
//! use wraith_discovery::dht::{DhtNode, NodeId, BootstrapConfig};
//! use std::time::Duration;
//!
//! // Create a DHT node
//! let id = NodeId::random();
//! let addr = "127.0.0.1:8000".parse().unwrap();
//! let mut node = DhtNode::new(id, addr);
//!
//! // Store a value
//! let key = [42u8; 32];
//! let value = vec![1, 2, 3];
//! node.store(key, value, Duration::from_secs(3600));
//!
//! // Retrieve a value
//! if let Some(data) = node.get(&key) {
//!     println!("Found value: {:?}", data);
//! }
//! ```

use zeroize::{Zeroize, ZeroizeOnDrop};

// Module declarations
pub mod bootstrap;
pub mod messages;
pub mod node;
pub mod node_id;
pub mod operations;
pub mod routing;

// Re-exports for convenience
pub use bootstrap::{Bootstrap, BootstrapConfig, BootstrapError, BootstrapNode};
pub use messages::{
    CompactPeer, DhtMessage, FindNodeRequest, FindValueRequest, FoundNodesResponse,
    FoundValueResponse, MessageError, PingRequest, PongResponse, StoreAckResponse, StoreRequest,
};
pub use node::{DhtNode, NodeState, StoredValue};
pub use node_id::{NodeId, SybilResistance};
pub use operations::{ALPHA, DhtOperations, OperationError};
pub use routing::{DhtError, DhtPeer, K, KBucket, NUM_BUCKETS, RoutingTable};

// SEC-002: Privacy exports (DhtPrivacy and GroupSecret are defined below in this file)

// ============================================================================
// SEC-002: DHT Privacy Enhancement
// ============================================================================

/// Group secret for privacy-preserving DHT lookups
///
/// A 32-byte secret shared among group members that enables privacy-enhanced
/// DHT operations. The secret is zeroized on drop to prevent memory disclosure.
///
/// # Security
///
/// - Stored in memory with zeroization on drop
/// - Used to derive info hashes that hide real file hashes
/// - Should be generated with a cryptographically secure RNG
/// - Should be rotated periodically
///
/// # Examples
///
/// ```
/// use wraith_discovery::dht::GroupSecret;
///
/// let secret = GroupSecret::new([42u8; 32]);
/// assert_eq!(secret.as_bytes().len(), 32);
/// ```
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct GroupSecret([u8; 32]);

impl GroupSecret {
    /// Create a new group secret from bytes
    ///
    /// # Arguments
    ///
    /// * `bytes` - 32-byte secret (should be cryptographically random)
    ///
    /// # Examples
    ///
    /// ```
    /// use wraith_discovery::dht::GroupSecret;
    ///
    /// let secret = GroupSecret::new([42u8; 32]);
    /// ```
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Generate a random group secret
    ///
    /// # Examples
    ///
    /// ```
    /// use wraith_discovery::dht::GroupSecret;
    ///
    /// let secret = GroupSecret::random();
    /// assert_eq!(secret.as_bytes().len(), 32);
    /// ```
    #[must_use]
    pub fn random() -> Self {
        let mut bytes = [0u8; 32];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut bytes);
        Self(bytes)
    }

    /// Get the secret bytes
    ///
    /// # Security
    ///
    /// The returned slice is valid only as long as the GroupSecret exists.
    /// Do not store or copy these bytes without proper zeroization.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for GroupSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GroupSecret")
            .field("bytes", &"[REDACTED]")
            .finish()
    }
}

/// Privacy-enhanced DHT operations
///
/// Provides methods for privacy-preserving key derivation using group secrets.
/// Real file hashes are never exposed in DHT lookups.
pub struct DhtPrivacy;

impl DhtPrivacy {
    /// Derive a privacy-enhanced info hash
    ///
    /// Uses BLAKE3 keyed hashing to derive an info hash that hides the real
    /// content hash. Only participants with the group secret can derive the
    /// lookup key.
    ///
    /// # Arguments
    ///
    /// * `group_secret` - Shared secret known to group members
    /// * `content_hash` - Real hash of the content (32 bytes)
    ///
    /// # Returns
    ///
    /// A 32-byte info hash for DHT storage/lookup
    ///
    /// # Security
    ///
    /// - Uses BLAKE3 keyed hash (group_secret as key)
    /// - Different group secrets produce different info hashes
    /// - Observers cannot derive content_hash from info_hash
    /// - Provides unlinkability between lookups
    ///
    /// # Examples
    ///
    /// ```
    /// use wraith_discovery::dht::{DhtPrivacy, GroupSecret};
    ///
    /// let group_secret = GroupSecret::new([42u8; 32]);
    /// let content_hash = [1u8; 32];
    ///
    /// let info_hash = DhtPrivacy::derive_info_hash(&group_secret, &content_hash);
    /// assert_eq!(info_hash.len(), 32);
    /// ```
    #[must_use]
    pub fn derive_info_hash(group_secret: &GroupSecret, content_hash: &[u8; 32]) -> [u8; 32] {
        // Use BLAKE3 keyed hash with group_secret as the key
        let hash = blake3::keyed_hash(group_secret.as_bytes(), content_hash);
        *hash.as_bytes()
    }

    /// Verify that an info hash matches a content hash
    ///
    /// Used to verify that a received info hash was derived from a specific
    /// content hash using the group secret.
    ///
    /// # Arguments
    ///
    /// * `group_secret` - Group secret
    /// * `content_hash` - Real content hash
    /// * `info_hash` - Claimed info hash
    ///
    /// # Returns
    ///
    /// `true` if info_hash was derived from content_hash using group_secret
    ///
    /// # Examples
    ///
    /// ```
    /// use wraith_discovery::dht::{DhtPrivacy, GroupSecret};
    ///
    /// let group_secret = GroupSecret::new([42u8; 32]);
    /// let content_hash = [1u8; 32];
    /// let info_hash = DhtPrivacy::derive_info_hash(&group_secret, &content_hash);
    ///
    /// assert!(DhtPrivacy::verify_info_hash(&group_secret, &content_hash, &info_hash));
    /// ```
    #[must_use]
    pub fn verify_info_hash(
        group_secret: &GroupSecret,
        content_hash: &[u8; 32],
        info_hash: &[u8; 32],
    ) -> bool {
        let expected = Self::derive_info_hash(group_secret, content_hash);
        expected == *info_hash
    }
}

/// DHT key derivation for announcements (legacy)
///
/// Derives a 160-bit (20-byte) announcement key from group secret and file hash
/// using BLAKE3 hashing with domain separation.
///
/// **Note:** For new code, prefer `DhtPrivacy::derive_info_hash()` which provides
/// stronger privacy guarantees using BLAKE3 keyed hashing.
///
/// This function is used to generate privacy-enhanced DHT keys for announcing
/// file availability without revealing file contents or group membership.
///
/// # Arguments
///
/// * `group_secret` - Shared secret known to group members
/// * `file_hash` - Hash of the file being announced
///
/// # Returns
///
/// A 20-byte announcement key suitable for DHT storage
///
/// # Security
///
/// The announcement key is derived using BLAKE3 with domain separation
/// to prevent key reuse attacks. The key reveals nothing about the
/// file contents or group membership to observers.
///
/// # Examples
///
/// ```
/// use wraith_discovery::dht::derive_announce_key;
///
/// let group_secret = b"shared-secret";
/// let file_hash = b"file-hash-value";
///
/// let announce_key = derive_announce_key(group_secret, file_hash);
/// assert_eq!(announce_key.len(), 20);
/// ```
#[must_use]
pub fn derive_announce_key(group_secret: &[u8], file_hash: &[u8]) -> [u8; 20] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(group_secret);
    hasher.update(file_hash);
    hasher.update(b"wraith-dht-announce"); // Domain separation

    let hash = hasher.finalize();
    let mut key = [0u8; 20];
    key.copy_from_slice(&hash.as_bytes()[..20]);
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_announce_key() {
        let group_secret = b"test-group-secret";
        let file_hash = b"test-file-hash";

        let key1 = derive_announce_key(group_secret, file_hash);
        let key2 = derive_announce_key(group_secret, file_hash);

        // Same inputs produce same key
        assert_eq!(key1, key2);
        assert_eq!(key1.len(), 20);
    }

    #[test]
    fn test_derive_announce_key_different_inputs() {
        let group_secret = b"test-group-secret";
        let file_hash1 = b"file-hash-1";
        let file_hash2 = b"file-hash-2";

        let key1 = derive_announce_key(group_secret, file_hash1);
        let key2 = derive_announce_key(group_secret, file_hash2);

        // Different inputs produce different keys
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_module_exports() {
        // Test that all re-exports are available
        let _id = NodeId::random();
        let _config = BootstrapConfig::new();
        let _bootstrap = Bootstrap::with_defaults();

        // Test constants
        assert_eq!(K, 20);
        assert_eq!(NUM_BUCKETS, 256);
        assert_eq!(ALPHA, 3);
    }

    // SEC-002: DHT Privacy Enhancement Tests
    #[test]
    fn test_group_secret_creation() {
        let secret = GroupSecret::new([42u8; 32]);
        assert_eq!(secret.as_bytes().len(), 32);
        assert_eq!(secret.as_bytes()[0], 42);
    }

    #[test]
    fn test_group_secret_random() {
        let secret1 = GroupSecret::random();
        let secret2 = GroupSecret::random();
        assert_eq!(secret1.as_bytes().len(), 32);
        assert_eq!(secret2.as_bytes().len(), 32);
        // Random secrets should be different
        assert_ne!(secret1.as_bytes(), secret2.as_bytes());
    }

    #[test]
    fn test_group_secret_debug() {
        let secret = GroupSecret::new([42u8; 32]);
        let debug_str = format!("{:?}", secret);
        assert!(debug_str.contains("REDACTED"));
        assert!(!debug_str.contains("42"));
    }

    #[test]
    fn test_dht_privacy_derive_info_hash() {
        let group_secret = GroupSecret::new([42u8; 32]);
        let content_hash = [1u8; 32];

        let info_hash = DhtPrivacy::derive_info_hash(&group_secret, &content_hash);
        assert_eq!(info_hash.len(), 32);

        // Same inputs should produce same hash
        let info_hash2 = DhtPrivacy::derive_info_hash(&group_secret, &content_hash);
        assert_eq!(info_hash, info_hash2);
    }

    #[test]
    fn test_dht_privacy_different_secrets() {
        let secret1 = GroupSecret::new([1u8; 32]);
        let secret2 = GroupSecret::new([2u8; 32]);
        let content_hash = [42u8; 32];

        let hash1 = DhtPrivacy::derive_info_hash(&secret1, &content_hash);
        let hash2 = DhtPrivacy::derive_info_hash(&secret2, &content_hash);

        // Different secrets should produce different hashes
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_dht_privacy_different_content() {
        let group_secret = GroupSecret::new([42u8; 32]);
        let content1 = [1u8; 32];
        let content2 = [2u8; 32];

        let hash1 = DhtPrivacy::derive_info_hash(&group_secret, &content1);
        let hash2 = DhtPrivacy::derive_info_hash(&group_secret, &content2);

        // Different content should produce different hashes
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_dht_privacy_verify_info_hash() {
        let group_secret = GroupSecret::new([42u8; 32]);
        let content_hash = [1u8; 32];
        let info_hash = DhtPrivacy::derive_info_hash(&group_secret, &content_hash);

        // Correct verification should pass
        assert!(DhtPrivacy::verify_info_hash(
            &group_secret,
            &content_hash,
            &info_hash
        ));

        // Wrong secret should fail
        let wrong_secret = GroupSecret::new([99u8; 32]);
        assert!(!DhtPrivacy::verify_info_hash(
            &wrong_secret,
            &content_hash,
            &info_hash
        ));

        // Wrong content should fail
        let wrong_content = [99u8; 32];
        assert!(!DhtPrivacy::verify_info_hash(
            &group_secret,
            &wrong_content,
            &info_hash
        ));

        // Wrong info hash should fail
        let wrong_info_hash = [99u8; 32];
        assert!(!DhtPrivacy::verify_info_hash(
            &group_secret,
            &content_hash,
            &wrong_info_hash
        ));
    }

    #[test]
    fn test_dht_privacy_unlinkability() {
        let group_secret = GroupSecret::new([42u8; 32]);
        let content_hash = [1u8; 32];
        let info_hash = DhtPrivacy::derive_info_hash(&group_secret, &content_hash);

        // Info hash should not reveal content hash
        // (statistical test - they should be uncorrelated)
        assert_ne!(info_hash, content_hash);

        // Even with all zeros, should produce non-zero hash
        let zero_content = [0u8; 32];
        let zero_info_hash = DhtPrivacy::derive_info_hash(&group_secret, &zero_content);
        assert_ne!(zero_info_hash, zero_content);
    }
}
