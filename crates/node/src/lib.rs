//! Lumen node implementation
//!
//! This crate provides the core node functionality for Lumen, including:
//! - Payload builder implementation
//! - Node configuration
//! - RPC interfaces

/// Builder module for payload construction and related utilities.
pub mod builder;
/// Configuration types and validation for the Rollkit payload builder
pub mod config;

// Re-export public types
pub use builder::{create_payload_builder_service, RollkitPayloadBuilder};
pub use config::{ConfigError, RollkitPayloadBuilderConfig};

/// Configuration for transaction forwarding to sequencer
#[derive(Debug, Clone)]
pub struct ForwardingConfig {
    /// Optional sequencer HTTP endpoint
    pub sequencer_http: Option<String>,
    /// Optional Basic-Auth header
    pub sequencer_auth: Option<String>,
    /// Disable transaction pool gossip
    pub disable_tx_pool_gossip: bool,
    /// Maximum number of in-flight requests
    pub queue_size: usize,
    /// Maximum requests per second to sequencer
    pub rate_limit_per_sec: u32,
}

impl Default for ForwardingConfig {
    fn default() -> Self {
        Self {
            sequencer_http: None,
            sequencer_auth: None,
            disable_tx_pool_gossip: false,
            queue_size: 64,
            rate_limit_per_sec: 1_000,
        }
    }
}
