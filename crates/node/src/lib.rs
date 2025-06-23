//! Lumen node implementation
//!
//! This crate provides the core node functionality for Lumen, including:
//! - Payload builder implementation
//! - Node configuration
//! - RPC interfaces

/// Builder module for payload construction and related utilities.
pub mod builder;
/// Rollkit payload builder implementation
pub mod config;

// Re-export public types
pub use builder::{create_payload_builder_service, RollkitPayloadBuilder};
pub use config::{ConfigError, RollkitPayloadBuilderConfig};
