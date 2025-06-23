//! End-to-end tests for Lumen
//!
//! This crate contains comprehensive integration and e2e tests for the Lumen node.

#[cfg(test)]
mod common;
#[cfg(test)]
mod engine_api_tests;
#[cfg(test)]
mod integration_tests;
#[cfg(test)]
mod payload_builder_tests;
#[cfg(test)]
mod test_rollkit_engine_api;