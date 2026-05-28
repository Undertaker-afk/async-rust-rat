//! Error types for the WRAITH core protocol.

use thiserror::Error;

/// Core protocol errors
#[derive(Debug, Error)]
pub enum Error {
    /// Frame parsing error
    #[error("frame error: {0}")]
    Frame(#[from] FrameError),

    /// Session error
    #[error("session error: {0}")]
    Session(#[from] SessionError),

    /// Cryptographic error
    #[error("crypto error: {0}")]
    Crypto(#[from] wraith_crypto::CryptoError),
}

/// Frame-level errors
#[derive(Debug, Error)]
pub enum FrameError {
    /// Frame too short to parse
    #[error("frame too short: expected at least {expected}, got {actual}")]
    TooShort {
        /// Expected minimum size
        expected: usize,
        /// Actual size received
        actual: usize,
    },

    /// Invalid frame type byte
    #[error("invalid frame type: 0x{0:02X}")]
    InvalidFrameType(u8),

    /// Reserved frame type used
    #[error("reserved frame type used")]
    ReservedFrameType,

    /// Payload length exceeds packet size
    #[error("payload length exceeds packet size")]
    PayloadOverflow,

    /// Invalid padding
    #[error("invalid padding")]
    InvalidPadding,

    /// Reserved stream ID (1-15)
    #[error("reserved stream ID: {0} (1-15 are reserved)")]
    ReservedStreamId(u32),

    /// Invalid offset (exceeds max file size)
    #[error("invalid offset: {offset} exceeds max {max}")]
    InvalidOffset {
        /// Provided offset
        offset: u64,
        /// Maximum allowed offset
        max: u64,
    },

    /// Sequence number anomaly detected
    #[error("sequence anomaly: expected ~{expected}, got {received} (delta: {delta})")]
    SequenceAnomaly {
        /// Expected sequence number
        expected: u32,
        /// Received sequence number
        received: u32,
        /// Delta from expected
        delta: u32,
    },

    /// Payload too large
    #[error("payload too large: {size} exceeds max {max}")]
    PayloadTooLarge {
        /// Payload size
        size: usize,
        /// Maximum allowed
        max: usize,
    },
}

/// Session-level errors
#[derive(Debug, Error)]
pub enum SessionError {
    /// Invalid state for the requested operation
    #[error("invalid state for operation")]
    InvalidState,

    /// No handshake in progress
    #[error("no handshake in progress")]
    NoHandshake,

    /// No session keys available
    #[error("no session keys available")]
    NoKeys,

    /// Too many concurrent streams
    #[error("too many concurrent streams")]
    TooManyStreams,

    /// Unknown stream ID
    #[error("unknown stream: {0}")]
    UnknownStream(u16),

    /// Connection timeout
    #[error("connection timeout")]
    Timeout,

    /// Connection closed by peer
    #[error("connection closed: {0}")]
    Closed(String),
}
