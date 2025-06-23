//! Lumen node implementation
//!
//! This crate provides the core node functionality for Lumen, including:
//! - Payload builder implementation
//! - Node configuration
//! - RPC interfaces

pub mod builder;
pub mod config;

// Re-export public types
pub use builder::{create_payload_builder_service, RollkitPayloadBuilder};
pub use config::{ConfigError, RollkitPayloadBuilderConfig};
