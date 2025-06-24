//! Integration tests for Lumen rollkit
//!
//! This crate contains integration tests for the Lumen rollkit implementation,
//! including payload builder tests, engine API tests, and common test utilities.

pub mod common;

#[cfg(test)]
mod engine_api_tests;
#[cfg(test)]
mod integration_tests;
#[cfg(test)]
mod payload_builder_tests;
#[cfg(test)]
mod test_rollkit_engine_api;

// Re-export common test utilities
pub use common::*;
