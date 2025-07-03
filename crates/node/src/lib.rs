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
/// RPC wrapper that forwards transactions to the sequencer
pub mod eth_api_forwarder;
/// Transaction forwarder that ships raw txs to the rollup sequencer
pub mod forwarder;

pub use forwarder::{ForwardError, TxForwarder};

// Re-export public types
pub use builder::{create_payload_builder_service, RollkitPayloadBuilder};
pub use config::{ConfigError, RollkitPayloadBuilderConfig};

// Re-export the forwarder type
pub use eth_api_forwarder::EthApiForwarder;

/// Configuration for transaction forwarding to sequencer
#[derive(Debug, Clone, Default)]
pub struct ForwardingConfig {
    /// Optional sequencer HTTP endpoint
    pub sequencer_http: Option<String>,
    /// Optional Basic-Auth header
    pub sequencer_auth: Option<String>,
    /// Disable transaction pool gossip
    pub disable_tx_pool_gossip: bool,
}
