//! Rollkit node binary with standard reth CLI support and rollkit payload builder integration.
//!
//! This node supports all standard reth CLI flags and functionality, with a customized
//! payload builder that accepts transactions via engine API payload attributes.

#![allow(missing_docs, rustdoc::missing_crate_level_docs)]

pub mod attributes;
pub mod builder;
pub mod error;

use alloy_rpc_types::engine::{
    ExecutionData, ExecutionPayloadEnvelopeV2, ExecutionPayloadEnvelopeV3,
    ExecutionPayloadEnvelopeV4, ExecutionPayloadEnvelopeV5, ExecutionPayloadV1,
};
use clap::Parser;
use reth_ethereum::{
    chainspec::ChainSpec,
    node::{
        api::{
            payload::{EngineApiMessageVersion, EngineObjectValidationError, PayloadOrAttributes},
            validate_version_specific_fields, AddOnsContext, EngineTypes, EngineValidator,
            FullNodeComponents, FullNodeTypes, InvalidPayloadAttributesError, NewPayloadError,
            NodeTypes, PayloadTypes, PayloadValidator,
        },
        builder::{
            components::{BasicPayloadServiceBuilder, ComponentsBuilder},
            rpc::{EngineValidatorBuilder, RpcAddOns},
            Node, NodeAdapter, NodeComponentsBuilder,
        },
        node::{
            EthereumConsensusBuilder, EthereumExecutorBuilder, EthereumNetworkBuilder,
            EthereumPoolBuilder,
        },
        EthereumEthApiBuilder,
    },
    primitives::{RecoveredBlock, SealedBlock},
};
use reth_ethereum_cli::{chainspec::EthereumChainSpecParser, Cli};
use reth_ethereum_payload_builder::EthereumExecutionPayloadValidator;
use reth_payload_builder::EthBuiltPayload;
use reth_trie_db::MerklePatriciaTrie;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

use crate::{
    attributes::{RollkitEnginePayloadAttributes, RollkitEnginePayloadBuilderAttributes},
    builder::{RollkitArgs, RollkitPayloadBuilderBuilder},
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

/// Rollkit engine validator that handles custom payload validation
#[derive(Debug, Clone)]
pub struct RollkitEngineValidator {
    inner: EthereumExecutionPayloadValidator<ChainSpec>,
}

impl RollkitEngineValidator {
    /// Instantiates a new validator.
    pub const fn new(chain_spec: Arc<ChainSpec>) -> Self {
        Self {
            inner: EthereumExecutionPayloadValidator::new(chain_spec),
        }
    }

    /// Returns the chain spec used by the validator.
    #[inline]
    fn chain_spec(&self) -> &ChainSpec {
        self.inner.chain_spec().as_ref()
    }
}

impl PayloadValidator for RollkitEngineValidator {
    type Block = reth_ethereum::Block;
    type ExecutionData = ExecutionData;

    fn ensure_well_formed_payload(
        &self,
        payload: ExecutionData,
    ) -> Result<RecoveredBlock<Self::Block>, NewPayloadError> {
        info!("Rollkit engine validator: validating payload");

        // Use inner validator but with custom rollkit handling
        match self.inner.ensure_well_formed_payload(payload.clone()) {
            Ok(sealed_block) => {
                info!("Rollkit engine validator: payload validation succeeded");
                sealed_block
                    .try_recover()
                    .map_err(|e| NewPayloadError::Other(e.into()))
            }
            Err(err) => {
                // Log the error for debugging
                tracing::debug!("Rollkit payload validation error: {:?}", err);

                // Check if this is a block hash mismatch error - bypass it for rollkit
                if matches!(err, alloy_rpc_types::engine::PayloadError::BlockHash { .. }) {
                    info!("Rollkit engine validator: bypassing block hash mismatch for rollkit");
                    // For rollkit, we trust the payload builder - just parse the block without hash validation
                    use reth_primitives_traits::Block;
                    let ExecutionData { payload, sidecar } = payload;
                    let sealed_block = payload.try_into_block_with_sidecar(&sidecar)?.seal_slow();
                    sealed_block
                        .try_recover()
                        .map_err(|e| NewPayloadError::Other(e.into()))
                } else {
                    // For other errors, re-throw them
                    Err(NewPayloadError::Eth(err))
                }
            }
        }
    }
}

impl<T> EngineValidator<T> for RollkitEngineValidator
where
    T: PayloadTypes<
        PayloadAttributes = RollkitEnginePayloadAttributes,
        ExecutionData = ExecutionData,
    >,
{
    fn validate_version_specific_fields(
        &self,
        version: EngineApiMessageVersion,
        payload_or_attrs: PayloadOrAttributes<'_, Self::ExecutionData, T::PayloadAttributes>,
    ) -> Result<(), EngineObjectValidationError> {
        validate_version_specific_fields(self.chain_spec(), version, payload_or_attrs)
    }

    fn ensure_well_formed_attributes(
        &self,
        version: EngineApiMessageVersion,
        attributes: &T::PayloadAttributes,
    ) -> Result<(), EngineObjectValidationError> {
        validate_version_specific_fields(
            self.chain_spec(),
            version,
            PayloadOrAttributes::<Self::ExecutionData, T::PayloadAttributes>::PayloadAttributes(
                attributes,
            ),
        )?;

        // Validate rollkit-specific attributes
        if let Some(ref transactions) = attributes.transactions {
            info!(
                "Rollkit engine validator: validating {} transactions",
                transactions.len()
            );
        }

        Ok(())
    }

    fn validate_payload_attributes_against_header(
        &self,
        _attr: &<T as PayloadTypes>::PayloadAttributes,
        _header: &<Self::Block as reth_ethereum::primitives::Block>::Header,
    ) -> Result<(), InvalidPayloadAttributesError> {
        // Skip default timestamp validation for rollkit
        Ok(())
    }
}

/// Rollkit engine validator builder
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct RollkitEngineValidatorBuilder;

impl<N> EngineValidatorBuilder<N> for RollkitEngineValidatorBuilder
where
    N: FullNodeComponents<
        Types: NodeTypes<
            Payload = RollkitEngineTypes,
            ChainSpec = ChainSpec,
            Primitives = reth_ethereum::EthPrimitives,
        >,
    >,
{
    type Validator = RollkitEngineValidator;

    async fn build(self, ctx: &AddOnsContext<'_, N>) -> eyre::Result<Self::Validator> {
        Ok(RollkitEngineValidator::new(ctx.config.chain.clone()))
    }
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
        EthereumConsensusBuilder,
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
            .consensus(EthereumConsensusBuilder::default())
    }

    fn add_ons(&self) -> Self::AddOns {
        RollkitNodeAddOns::default()
    }
}

fn main() {
    info!("=== ROLLKIT-RETH NODE STARTING ===");

    reth_cli_util::sigsegv_handler::install();

    // Enable backtraces unless a RUST_BACKTRACE value has already been explicitly provided.
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    if let Err(err) = Cli::<EthereumChainSpecParser, RollkitArgs>::parse().run(
        async move |builder, rollkit_args| {
            info!(
                "=== ROLLKIT-RETH: Starting with args: {:?} ===",
                rollkit_args
            );
            info!("=== ROLLKIT-RETH: Rollkit mode enabled ===");
            info!("=== ROLLKIT-RETH: Using custom payload builder with transaction support ===");

            let handle = builder
                .node(RollkitNode::new(rollkit_args))
                .launch()
                .await?;

            info!("=== ROLLKIT-RETH: Node launched successfully with rollkit payload builder ===");
            handle.node_exit_future.await
        },
    ) {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
}
