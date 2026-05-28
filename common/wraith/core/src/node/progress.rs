//! File transfer progress tracking
//!
//! Provides detailed progress information for file transfers, including speed,
//! ETA, and per-chunk status.

use crate::node::identity::TransferId;
use std::time::Duration;

/// Transfer status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferStatus {
    /// Transfer is initializing
    Initializing,
    /// Transfer is in progress
    Transferring,
    /// Transfer is verifying file integrity
    Verifying,
    /// Transfer completed successfully
    Complete,
    /// Transfer failed with error
    Failed,
    /// Transfer paused
    Paused,
}

impl std::fmt::Display for TransferStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Initializing => write!(f, "Initializing"),
            Self::Transferring => write!(f, "Transferring"),
            Self::Verifying => write!(f, "Verifying"),
            Self::Complete => write!(f, "Complete"),
            Self::Failed => write!(f, "Failed"),
            Self::Paused => write!(f, "Paused"),
        }
    }
}

/// Detailed transfer progress information
#[derive(Debug, Clone)]
pub struct TransferProgress {
    /// Transfer ID
    pub transfer_id: TransferId,

    /// Current status
    pub status: TransferStatus,

    /// Bytes transferred so far
    pub bytes_sent: u64,

    /// Total bytes to transfer
    pub bytes_total: u64,

    /// Number of chunks sent
    pub chunks_sent: usize,

    /// Total number of chunks
    pub chunks_total: usize,

    /// Current transfer speed in bytes/second
    pub speed_bytes_per_sec: f64,

    /// Estimated time remaining
    pub eta: Option<Duration>,

    /// Progress percentage (0.0 to 100.0)
    pub progress_percent: f64,
}

impl TransferProgress {
    /// Create a new transfer progress instance
    pub fn new(transfer_id: TransferId, bytes_total: u64, chunks_total: usize) -> Self {
        Self {
            transfer_id,
            status: TransferStatus::Initializing,
            bytes_sent: 0,
            bytes_total,
            chunks_sent: 0,
            chunks_total,
            speed_bytes_per_sec: 0.0,
            eta: None,
            progress_percent: 0.0,
        }
    }

    /// Update progress with current transfer state
    pub fn update(&mut self, bytes_sent: u64, chunks_sent: usize, speed_bytes_per_sec: f64) {
        self.bytes_sent = bytes_sent;
        self.chunks_sent = chunks_sent;
        self.speed_bytes_per_sec = speed_bytes_per_sec;

        // Calculate progress percentage
        if self.bytes_total > 0 {
            self.progress_percent = (bytes_sent as f64 / self.bytes_total as f64) * 100.0;
        }

        // Calculate ETA
        if speed_bytes_per_sec > 0.0 && bytes_sent < self.bytes_total {
            let remaining_bytes = self.bytes_total - bytes_sent;
            let seconds_remaining = remaining_bytes as f64 / speed_bytes_per_sec;
            self.eta = Some(Duration::from_secs_f64(seconds_remaining));
        } else {
            self.eta = None;
        }

        // Update status based on progress
        if self.bytes_sent >= self.bytes_total && self.chunks_sent >= self.chunks_total {
            self.status = TransferStatus::Complete;
        } else if self.bytes_sent > 0 {
            self.status = TransferStatus::Transferring;
        }
    }

    /// Check if transfer is complete
    pub fn is_complete(&self) -> bool {
        matches!(self.status, TransferStatus::Complete)
    }

    /// Check if transfer has failed
    pub fn is_failed(&self) -> bool {
        matches!(self.status, TransferStatus::Failed)
    }

    /// Get human-readable ETA string
    pub fn eta_string(&self) -> String {
        match self.eta {
            Some(duration) => {
                let seconds = duration.as_secs();
                if seconds < 60 {
                    format!("{seconds}s")
                } else if seconds < 3600 {
                    format!("{}m {}s", seconds / 60, seconds % 60)
                } else {
                    format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
                }
            }
            None => {
                if self.is_complete() {
                    "Complete".to_string()
                } else {
                    "Calculating...".to_string()
                }
            }
        }
    }

    /// Get human-readable speed string
    pub fn speed_string(&self) -> String {
        if self.speed_bytes_per_sec < 1024.0 {
            format!("{:.2} B/s", self.speed_bytes_per_sec)
        } else if self.speed_bytes_per_sec < 1024.0 * 1024.0 {
            format!("{:.2} KiB/s", self.speed_bytes_per_sec / 1024.0)
        } else if self.speed_bytes_per_sec < 1024.0 * 1024.0 * 1024.0 {
            format!("{:.2} MiB/s", self.speed_bytes_per_sec / (1024.0 * 1024.0))
        } else {
            format!(
                "{:.2} GiB/s",
                self.speed_bytes_per_sec / (1024.0 * 1024.0 * 1024.0)
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transfer_progress_new() {
        let transfer_id = [0u8; 32];
        let progress = TransferProgress::new(transfer_id, 1000, 10);

        assert_eq!(progress.transfer_id, transfer_id);
        assert_eq!(progress.status, TransferStatus::Initializing);
        assert_eq!(progress.bytes_sent, 0);
        assert_eq!(progress.bytes_total, 1000);
        assert_eq!(progress.chunks_sent, 0);
        assert_eq!(progress.chunks_total, 10);
        assert_eq!(progress.speed_bytes_per_sec, 0.0);
        assert_eq!(progress.progress_percent, 0.0);
    }

    #[test]
    fn test_transfer_progress_update() {
        let transfer_id = [0u8; 32];
        let mut progress = TransferProgress::new(transfer_id, 1000, 10);

        progress.update(500, 5, 100.0);

        assert_eq!(progress.bytes_sent, 500);
        assert_eq!(progress.chunks_sent, 5);
        assert_eq!(progress.speed_bytes_per_sec, 100.0);
        assert_eq!(progress.progress_percent, 50.0);
        assert_eq!(progress.status, TransferStatus::Transferring);

        // Check ETA is calculated
        assert!(progress.eta.is_some());
        let eta = progress.eta.unwrap();
        assert_eq!(eta.as_secs(), 5); // (1000 - 500) / 100 = 5 seconds
    }

    #[test]
    fn test_transfer_progress_complete() {
        let transfer_id = [0u8; 32];
        let mut progress = TransferProgress::new(transfer_id, 1000, 10);

        progress.update(1000, 10, 100.0);

        assert!(progress.is_complete());
        assert_eq!(progress.progress_percent, 100.0);
        assert_eq!(progress.status, TransferStatus::Complete);
        assert!(progress.eta.is_none());
    }

    #[test]
    fn test_eta_string() {
        let transfer_id = [0u8; 32];
        let mut progress = TransferProgress::new(transfer_id, 1000, 10);

        // Test seconds
        progress.eta = Some(Duration::from_secs(30));
        assert_eq!(progress.eta_string(), "30s");

        // Test minutes
        progress.eta = Some(Duration::from_secs(90));
        assert_eq!(progress.eta_string(), "1m 30s");

        // Test hours
        progress.eta = Some(Duration::from_secs(3700));
        assert_eq!(progress.eta_string(), "1h 1m");

        // Test complete
        progress.status = TransferStatus::Complete;
        progress.eta = None;
        assert_eq!(progress.eta_string(), "Complete");
    }

    #[test]
    fn test_speed_string() {
        let transfer_id = [0u8; 32];
        let mut progress = TransferProgress::new(transfer_id, 1000, 10);

        // Test bytes/sec
        progress.speed_bytes_per_sec = 500.0;
        assert_eq!(progress.speed_string(), "500.00 B/s");

        // Test KiB/s
        progress.speed_bytes_per_sec = 1024.0 * 50.0;
        assert_eq!(progress.speed_string(), "50.00 KiB/s");

        // Test MiB/s
        progress.speed_bytes_per_sec = 1024.0 * 1024.0 * 2.5;
        assert_eq!(progress.speed_string(), "2.50 MiB/s");

        // Test GiB/s
        progress.speed_bytes_per_sec = 1024.0 * 1024.0 * 1024.0 * 1.5;
        assert_eq!(progress.speed_string(), "1.50 GiB/s");
    }
}
