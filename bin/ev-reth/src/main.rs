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
    info!("=== EV-RETH NODE STARTING ===");

    reth_cli_util::sigsegv_handler::install();

    // Enable backtraces unless a RUST_BACKTRACE value has already been explicitly provided.
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    if let Err(err) = Cli::<EthereumChainSpecParser, RollkitArgs>::parse().run(
        async move |builder, rollkit_args| {
            info!("=== EV-RETH: Starting with args: {:?} ===", rollkit_args);
            info!("=== EV-RETH: EV-node mode enabled ===");
            info!("=== EV-RETH: Using custom payload builder with transaction support ===");

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

            info!("=== EV-RETH: Node launched successfully with ev-reth payload builder ===");

            // Set up graceful shutdown handling
            let shutdown_signal = async {
                #[cfg(unix)]
                {
                    // Set up SIGTERM handler with proper error handling
                    // Note: We only handle SIGTERM explicitly; ctrl_c() handles SIGINT automatically
                    match signal::unix::signal(signal::unix::SignalKind::terminate()) {
                        Ok(mut sigterm) => {
                            tokio::select! {
                                _ = sigterm.recv() => {
                                    info!("=== EV-RETH: Received SIGTERM, initiating graceful shutdown ===");
                                }
                                _ = signal::ctrl_c() => {
                                    info!("=== EV-RETH: Received SIGINT/Ctrl+C, initiating graceful shutdown ===");
                                }
                            }
                        }
                        Err(err) => {
                            tracing::warn!("Failed to install SIGTERM handler: {}, falling back to SIGINT only", err);
                            // Fall back to just handling SIGINT/Ctrl+C
                            signal::ctrl_c().await.unwrap_or_else(|ctrl_c_err| {
                                tracing::error!("Failed to wait for Ctrl+C: {}", ctrl_c_err);
                            });
                            info!("=== EV-RETH: Received SIGINT/Ctrl+C, initiating graceful shutdown ===");
                        }
                    }
                }

                #[cfg(not(unix))]
                {
                    // On non-Unix systems, only handle Ctrl+C
                    signal::ctrl_c().await.unwrap_or_else(|err| {
                        tracing::error!("Failed to wait for Ctrl+C: {}", err);
                    });
                    info!("=== EV-RETH: Received SIGINT/Ctrl+C, initiating graceful shutdown ===");
                }
            };

            // Wait for either the node to exit naturally or a shutdown signal
            tokio::select! {
                result = &mut handle.node_exit_future => {
                    info!("=== EV-RETH: Node exited naturally ===");
                    result
                }
                _ = shutdown_signal => {
                    info!("=== EV-RETH: Shutdown signal received, stopping node ===");

                    // Initiate graceful shutdown with timeout
                    let shutdown_timeout = Duration::from_secs(30);

                    // Wait for the node to finish shutting down with a timeout
                    // The handle will be dropped automatically, triggering shutdown
                    let shutdown_result = timeout(shutdown_timeout, async {
                        // Drop the handle to trigger shutdown
                        drop(handle);
                        // Give the node time to clean up
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        Ok(())
                    }).await;

                    match shutdown_result {
                        Ok(result) => {
                            info!("=== EV-RETH: Node shutdown completed gracefully ===");
                            result
                        }
                        Err(_) => {
                            tracing::warn!("=== EV-RETH: Node shutdown timed out after {:?} ===", shutdown_timeout);
                            info!("=== EV-RETH: Node shutdown process completed ===");
                            Ok(())
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
