use serde::{Deserialize, Serialize};

/// Default maximum bytes for txpool transactions (1.98 MB)
pub const DEFAULT_MAX_TXPOOL_BYTES: u64 = 1_980 * 1024; // 1.98 MB = 2,027,520 bytes

/// Configuration for Rollkit-specific functionality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollkitConfig {
    /// Maximum bytes of transactions to return from the txpool
    #[serde(default = "default_max_txpool_bytes")]
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
    /// Creates a new RollkitConfig with the given max txpool bytes
    pub const fn new(max_txpool_bytes: u64) -> Self {
        Self { max_txpool_bytes }
    }
}

fn default_max_txpool_bytes() -> u64 {
    DEFAULT_MAX_TXPOOL_BYTES
}
