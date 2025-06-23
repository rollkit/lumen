//! Rollkit-specific types and integration
//!
//! This crate provides Rollkit-specific functionality including:
//! - Custom payload attributes for Rollkit
//! - Rollkit-specific types and traits

/// Rollkit-specific types and related definitions.
pub mod types;

// Re-export public types
pub use types::{PayloadAttributesError, RollkitPayloadAttributes};
