use serde::{Deserialize, Serialize};

/// Default maximum bytes for txpool transactions (1.85 MiB)
pub const DEFAULT_MAX_TXPOOL_BYTES: u64 = (1.85 * 1024.0 * 1024.0).round() as u64; // 1.85 MiB = 1,939,866 bytes

/// Configuration for Rollkit-specific functionality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollkitConfig {
    /// Maximum bytes of transactions to return from the txpool
    pub max_txpool_bytes: u64,
}

impl Default for RollkitConfig {
    fn default() -> Self {
        Self {
            max_txpool_bytes: DEFAULT_MAX_TXPOOL_BYTES,
        }
    }
}

impl RollkitConfig {
    /// Creates a new `RollkitConfig` with the given max txpool bytes
    pub const fn new(max_txpool_bytes: u64) -> Self {
        Self { max_txpool_bytes }
    }
}
