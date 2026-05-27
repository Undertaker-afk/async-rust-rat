//! File transfer layer.
//!
//! Provides high-level file transfer session management, progress tracking,
//! and multi-peer coordination.

pub mod session;

pub use session::{Direction, TransferSession, TransferState};
