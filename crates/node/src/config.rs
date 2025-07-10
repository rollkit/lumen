use serde::{Deserialize, Serialize};

/// Configuration for the Rollkit payload builder
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollkitPayloadBuilderConfig {
    /// Minimum gas price for transactions
    pub min_gas_price: u64,
    /// Maximum size in bytes for transactions in a payload (best effort)
    pub max_payload_bytes: usize,
}

impl Default for RollkitPayloadBuilderConfig {
    fn default() -> Self {
        Self {
            min_gas_price: 1_000_000_000, // 1 Gwei
            max_payload_bytes: 2_097_152, // 2MB default
        }
    }
}

impl RollkitPayloadBuilderConfig {
    /// Creates a new instance of `RollkitPayloadBuilderConfig`
    pub const fn new(min_gas_price: u64, max_payload_bytes: usize) -> Self {
        Self {
            min_gas_price,
            max_payload_bytes,
        }
    }

    /// Validates the configuration
    pub const fn validate(&self) -> Result<(), ConfigError> {
        if self.min_gas_price == 0 {
            return Err(ConfigError::InvalidMinGasPrice);
        }

        if self.max_payload_bytes == 0 {
            return Err(ConfigError::InvalidMaxPayloadBytes);
        }

        Ok(())
    }
}

/// Errors that can occur during configuration validation
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid min gas price value")]
    /// Invalid minimum gas price value.
    InvalidMinGasPrice,
    #[error("Invalid max payload bytes value")]
    /// Invalid maximum payload bytes value.
    InvalidMaxPayloadBytes,
}
