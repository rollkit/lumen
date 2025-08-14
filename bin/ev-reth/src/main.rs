//! Rollkit node binary with standard reth CLI support and rollkit payload builder integration.
//!
//! This node supports all standard reth CLI flags and functionality, with a customized
//! payload builder that accepts transactions via engine API payload attributes.

#![allow(missing_docs, rustdoc::missing_crate_level_docs)]

pub mod attributes;
pub mod builder;
pub mod error;
pub mod validator;

#[cfg(test)]
mod signal_tests;

use alloy_rpc_types::engine::{
    ExecutionData, ExecutionPayloadEnvelopeV2, ExecutionPayloadEnvelopeV3,
    ExecutionPayloadEnvelopeV4, ExecutionPayloadEnvelopeV5, ExecutionPayloadV1,
};
use clap::Parser;
use evolve_ev_reth::{
    config::RollkitConfig,
    consensus::RollkitConsensusBuilder,
    rpc::txpool::{RollkitTxpoolApiImpl, RollkitTxpoolApiServer},
};
use reth_ethereum::{
    chainspec::ChainSpec,
    node::{
        api::{EngineTypes, FullNodeTypes, NodeTypes, PayloadTypes},
        builder::{
            components::{BasicPayloadServiceBuilder, ComponentsBuilder},
            rpc::RpcAddOns,
            Node, NodeAdapter, NodeComponentsBuilder,
        },
        node::{EthereumExecutorBuilder, EthereumNetworkBuilder, EthereumPoolBuilder},
        EthereumEthApiBuilder,
    },
    primitives::SealedBlock,
};
use reth_ethereum_cli::{chainspec::EthereumChainSpecParser, Cli};
use reth_payload_builder::EthBuiltPayload;
use reth_trie_db::MerklePatriciaTrie;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::{signal, time::timeout};
use tracing::info;

use crate::{
    attributes::{RollkitEnginePayloadAttributes, RollkitEnginePayloadBuilderAttributes},
    builder::{RollkitArgs, RollkitPayloadBuilderBuilder},
    validator::RollkitEngineValidatorBuilder,
};

#[global_allocator]
static ALLOC: reth_cli_util::allocator::Allocator = reth_cli_util::allocator::new_allocator();

/// Centralized configuration for EV-RETH node operation
#[derive(Debug, Clone)]
struct NodeConfig {
    shutdown_timeout: Duration,
    status_check_interval: u64,
    enable_fallback_status_checks: bool,
    max_fallback_checks: u64,
}

impl NodeConfig {
    /// Minimum allowed shutdown timeout in seconds
    const MIN_SHUTDOWN_TIMEOUT_SECS: u64 = 1;

    /// Default shutdown timeout in seconds (optimized for containers)
    const DEFAULT_SHUTDOWN_TIMEOUT_SECS: u64 = 15;

    /// Maximum allowed shutdown timeout in seconds (5 minutes)
    const MAX_SHUTDOWN_TIMEOUT_SECS: u64 = 300;

    /// Minimum allowed status check interval in seconds
    const MIN_STATUS_CHECK_INTERVAL_SECS: u64 = 1;

    /// Default status check interval in seconds (1 hour)
    const DEFAULT_STATUS_CHECK_INTERVAL_SECS: u64 = 3600;

    /// Maximum allowed status check interval in seconds (6 hours)
    const MAX_STATUS_CHECK_INTERVAL_SECS: u64 = 21600;

    /// Default maximum number of fallback status checks
    const DEFAULT_MAX_FALLBACK_CHECKS: u64 = 24;

    /// Load configuration from environment variables with validation
    fn from_env() -> Self {
        let shutdown_timeout = Self::parse_shutdown_timeout();
        let status_check_interval = Self::parse_status_check_interval();
        let enable_fallback_status_checks = std::env::var("EV_RETH_ENABLE_FALLBACK_STATUS_CHECKS")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);
        let max_fallback_checks = std::env::var("EV_RETH_MAX_FALLBACK_CHECKS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(Self::DEFAULT_MAX_FALLBACK_CHECKS);

        Self {
            shutdown_timeout,
            status_check_interval,
            enable_fallback_status_checks,
            max_fallback_checks,
        }
    }

    fn parse_shutdown_timeout() -> Duration {
        match std::env::var("EV_RETH_SHUTDOWN_TIMEOUT") {
            Ok(val) => match val.parse::<u64>() {
                Ok(secs) if secs >= Self::MIN_SHUTDOWN_TIMEOUT_SECS && secs <= Self::MAX_SHUTDOWN_TIMEOUT_SECS => {
                    tracing::info!("Using custom shutdown timeout of {}s from environment", secs);
                    Duration::from_secs(secs)
                }
                Ok(secs) => {
                    tracing::warn!(
                        "Shutdown timeout {}s is out of bounds ({}-{}), using default {}s",
                        secs, Self::MIN_SHUTDOWN_TIMEOUT_SECS, Self::MAX_SHUTDOWN_TIMEOUT_SECS, Self::DEFAULT_SHUTDOWN_TIMEOUT_SECS
                    );
                    Duration::from_secs(Self::DEFAULT_SHUTDOWN_TIMEOUT_SECS)
                }
                Err(_) => {
                    tracing::warn!(
                        "Invalid EV_RETH_SHUTDOWN_TIMEOUT value '{}', using default {}s",
                        val, Self::DEFAULT_SHUTDOWN_TIMEOUT_SECS
                    );
                    Duration::from_secs(Self::DEFAULT_SHUTDOWN_TIMEOUT_SECS)
                }
            },
            Err(_) => {
                tracing::info!("Using default shutdown timeout of {}s", Self::DEFAULT_SHUTDOWN_TIMEOUT_SECS);
                Duration::from_secs(Self::DEFAULT_SHUTDOWN_TIMEOUT_SECS)
            }
        }
    }

    fn parse_status_check_interval() -> u64 {
        match std::env::var("EV_RETH_STATUS_CHECK_INTERVAL") {
            Ok(val) => match val.parse::<u64>() {
                Ok(secs) if secs >= Self::MIN_STATUS_CHECK_INTERVAL_SECS && secs <= Self::MAX_STATUS_CHECK_INTERVAL_SECS => {
                    tracing::info!("Using custom status check interval of {}s from environment", secs);
                    secs
                }
                Ok(secs) => {
                    tracing::warn!(
                        "Status check interval {}s is out of bounds ({}-{}), using default {}s",
                        secs, Self::MIN_STATUS_CHECK_INTERVAL_SECS, Self::MAX_STATUS_CHECK_INTERVAL_SECS, Self::DEFAULT_STATUS_CHECK_INTERVAL_SECS
                    );
                    Self::DEFAULT_STATUS_CHECK_INTERVAL_SECS
                }
                Err(_) => {
                    tracing::warn!(
                        "Invalid EV_RETH_STATUS_CHECK_INTERVAL value '{}', using default {}s",
                        val, Self::DEFAULT_STATUS_CHECK_INTERVAL_SECS
                    );
                    Self::DEFAULT_STATUS_CHECK_INTERVAL_SECS
                }
            },
            Err(_) => Self::DEFAULT_STATUS_CHECK_INTERVAL_SECS,
        }
    }
}

/// Fallback mechanism for when signal handling fails completely
async fn signal_fallback_mechanism(config: &NodeConfig) {
    if config.enable_fallback_status_checks {
        tracing::info!("Fallback status checks enabled");

        let mut check_count = 0;
        while check_count < config.max_fallback_checks {
            tokio::time::sleep(Duration::from_secs(config.status_check_interval)).await;
            check_count += 1;
            tracing::info!("Periodic status check #{} - node still running", check_count);
        }

        tracing::info!("Maximum fallback status checks ({}) reached, switching to efficient wait", config.max_fallback_checks);
    } else {
        tracing::info!("Using efficient fallback - waiting indefinitely for natural node exit");
        tracing::info!("Set EV_RETH_ENABLE_FALLBACK_STATUS_CHECKS=true to enable periodic status logging");
    }

    // Use std::future::pending() for the most efficient "wait forever" approach
    std::future::pending::<()>().await;
}

/// Handle shutdown signals with optimized, non-redundant signal handling
async fn handle_shutdown_signals() {
    #[cfg(unix)]
    {
        // On Unix systems, handle SIGTERM and SIGINT separately to avoid redundancy
        // SIGTERM is sent by process managers, SIGINT is sent by Ctrl+C
        match (
            signal::unix::signal(signal::unix::SignalKind::terminate()),
            signal::unix::signal(signal::unix::SignalKind::interrupt())
        ) {
            (Ok(mut sigterm), Ok(mut sigint)) => {
                tokio::select! {
                    _ = sigterm.recv() => {
                        tracing::info!("Received SIGTERM, initiating graceful shutdown");
                    }
                    _ = sigint.recv() => {
                        tracing::info!("Received SIGINT, initiating graceful shutdown");
                    }
                }
            }
            (Ok(mut sigterm), Err(sigint_err)) => {
                tracing::warn!("Failed to install SIGINT handler: {}, using SIGTERM only", sigint_err);
                if sigterm.recv().await.is_some() {
                    tracing::info!("Received SIGTERM, initiating graceful shutdown");
                }
            }
            (Err(sigterm_err), Ok(mut sigint)) => {
                tracing::warn!("Failed to install SIGTERM handler: {}, using SIGINT only", sigterm_err);
                if sigint.recv().await.is_some() {
                    tracing::info!("Received SIGINT, initiating graceful shutdown");
                }
            }
            (Err(sigterm_err), Err(sigint_err)) => {
                tracing::error!("Failed to install both SIGTERM and SIGINT handlers: SIGTERM={}, SIGINT={}", sigterm_err, sigint_err);
                tracing::warn!("No signal handling available, shutdown will only occur on natural node exit");
                let config = NodeConfig::from_env();
                signal_fallback_mechanism(&config).await;
            }
        }
    }

    #[cfg(not(unix))]
    {
        // On non-Unix systems, only handle Ctrl+C (SIGINT)
        if let Err(err) = signal::ctrl_c().await {
            tracing::error!("Failed to wait for SIGINT: {}", err);
            tracing::warn!("No signal handling available, shutdown will only occur on natural node exit");
            let config = NodeConfig::from_env();
            signal_fallback_mechanism(&config).await;
        } else {
            tracing::info!("Received SIGINT/Ctrl+C, initiating graceful shutdown");
        }
    }
}

/// Rollkit engine types - uses custom payload attributes that support transactions
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[non_exhaustive]
pub struct RollkitEngineTypes;

impl PayloadTypes for RollkitEngineTypes {
    type ExecutionData = ExecutionData;
    type BuiltPayload = EthBuiltPayload;
    type PayloadAttributes = RollkitEnginePayloadAttributes;
    type PayloadBuilderAttributes = RollkitEnginePayloadBuilderAttributes;

    fn block_to_payload(
        block: SealedBlock<
            <<Self::BuiltPayload as reth_ethereum::node::api::BuiltPayload>::Primitives as reth_ethereum::node::api::NodePrimitives>::Block,
        >,
    ) -> ExecutionData {
        let (payload, sidecar) =
            reth_ethereum::rpc::types::engine::ExecutionPayload::from_block_unchecked(
                block.hash(),
                &block.into_block(),
            );
        ExecutionData { payload, sidecar }
    }
}

impl EngineTypes for RollkitEngineTypes {
    type ExecutionPayloadEnvelopeV1 = ExecutionPayloadV1;
    type ExecutionPayloadEnvelopeV2 = ExecutionPayloadEnvelopeV2;
    type ExecutionPayloadEnvelopeV3 = ExecutionPayloadEnvelopeV3;
    type ExecutionPayloadEnvelopeV4 = ExecutionPayloadEnvelopeV4;
    type ExecutionPayloadEnvelopeV5 = ExecutionPayloadEnvelopeV5;
}

/// Rollkit node type
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct RollkitNode {
    /// Rollkit-specific arguments
    pub args: RollkitArgs,
}

impl RollkitNode {
    /// Create a new rollkit node with the given arguments
    pub const fn new(args: RollkitArgs) -> Self {
        Self { args }
    }
}

impl NodeTypes for RollkitNode {
    type Primitives = reth_ethereum::EthPrimitives;
    type ChainSpec = ChainSpec;
    type StateCommitment = MerklePatriciaTrie;
    type Storage = reth_ethereum::provider::EthStorage;
    type Payload = RollkitEngineTypes;
}

/// Rollkit node addons configuring RPC types with custom engine validator
pub type RollkitNodeAddOns<N> = RpcAddOns<N, EthereumEthApiBuilder, RollkitEngineValidatorBuilder>;

impl<N> Node<N> for RollkitNode
where
    N: FullNodeTypes<
        Types: NodeTypes<
            Payload = RollkitEngineTypes,
            ChainSpec = ChainSpec,
            Primitives = reth_ethereum::EthPrimitives,
            Storage = reth_ethereum::provider::EthStorage,
        >,
    >,
{
    type ComponentsBuilder = ComponentsBuilder<
        N,
        EthereumPoolBuilder,
        BasicPayloadServiceBuilder<RollkitPayloadBuilderBuilder>,
        EthereumNetworkBuilder,
        EthereumExecutorBuilder,
        RollkitConsensusBuilder,
    >;
    type AddOns = RollkitNodeAddOns<
        NodeAdapter<N, <Self::ComponentsBuilder as NodeComponentsBuilder<N>>::Components>,
    >;

    fn components_builder(&self) -> Self::ComponentsBuilder {
        ComponentsBuilder::default()
            .node_types::<N>()
            .pool(EthereumPoolBuilder::default())
            .executor(EthereumExecutorBuilder::default())
            .payload(BasicPayloadServiceBuilder::new(
                RollkitPayloadBuilderBuilder::new(&self.args),
            ))
            .network(EthereumNetworkBuilder::default())
            .consensus(RollkitConsensusBuilder::default())
    }

    fn add_ons(&self) -> Self::AddOns {
        RollkitNodeAddOns::default()
    }
}

fn main() {
    tracing::info!("=== EV-RETH NODE STARTING ===");

    reth_cli_util::sigsegv_handler::install();

    // Enable backtraces unless a RUST_BACKTRACE value has already been explicitly provided.
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    // Note: Signal handlers are installed in the async context after node launch
    // This provides the most reliable signal handling without creating unused handlers

    if let Err(err) = Cli::<EthereumChainSpecParser, RollkitArgs>::parse().run(
        async move |builder, rollkit_args| {
            tracing::info!("=== EV-RETH: Starting with args: {:?} ===", rollkit_args);
            tracing::info!("=== EV-RETH: EV-node mode enabled ===");
            tracing::info!("=== EV-RETH: Using custom payload builder with transaction support ===");

            let mut handle = builder
                .node(RollkitNode::new(rollkit_args))
                .extend_rpc_modules(move |ctx| {
                    // Build custom txpool RPC
                    let rollkit_txpool = RollkitTxpoolApiImpl::new(
                        ctx.pool().clone(),
                        RollkitConfig::default().max_txpool_bytes,
                    );

                    // Merge into all enabled transports (HTTP / WS)
                    ctx.modules.merge_configured(rollkit_txpool.into_rpc())?;
                    Ok(())
                })
                .launch()
                .await?;

            tracing::info!("=== EV-RETH: Node launched successfully with ev-reth payload builder ===");

            // Load configuration once at startup
            let config = NodeConfig::from_env();

            // Wait for either the node to exit naturally or a shutdown signal
            tokio::select! {
                result = &mut handle.node_exit_future => {
                    tracing::info!("Node exited naturally");
                    result
                }
                _ = handle_shutdown_signals() => {
                    tracing::info!("Shutdown signal received, initiating graceful shutdown");

                    // Structured shutdown phases for better observability (informational only)
                    // Note: These phases are logged for monitoring purposes but don't implement
                    // specific connection stopping or request draining - the underlying reth node
                    // handles the actual shutdown logic when the handle is dropped
                    tracing::info!("Phase 1 - Initiating shutdown sequence");
                    tracing::info!("Phase 2 - Waiting for graceful node termination");

                    // Wait for the node to actually exit with a timeout
                    let shutdown_result = timeout(config.shutdown_timeout, &mut handle.node_exit_future).await;

                    tracing::info!("Phase 3 - Shutdown sequence completed");

                    match shutdown_result {
                        Ok(result) => {
                            tracing::info!("Node shutdown completed gracefully");
                            result
                        }
                        Err(_) => {
                            tracing::error!("Node shutdown timed out after {:?}", config.shutdown_timeout);
                            tracing::error!("Forcing application exit - this may indicate a shutdown issue");
                            // Return an error to indicate that shutdown didn't complete gracefully
                            Err(Box::new(std::io::Error::new(
                                std::io::ErrorKind::TimedOut,
                                format!("Graceful shutdown timed out after {}s", config.shutdown_timeout.as_secs())
                            )) as Box<dyn std::error::Error + Send + Sync>)
                        }
                    }
                }
            }
        },
    ) {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
}
