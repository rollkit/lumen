//! Rollkit consensus builder implementation

use lumen_rollkit::consensus::RollkitConsensus;
use reth_chainspec::ChainSpec;
use reth_consensus::{ConsensusError, FullConsensus};
use reth_ethereum::node::builder::{components::ConsensusBuilder, BuilderContext};
use reth_ethereum_primitives::EthPrimitives;
use reth_node_api::{FullNodeTypes, NodeTypes};
use std::sync::Arc;

/// Builder for `RollkitConsensus` that implements the `ConsensusBuilder` trait
#[derive(Debug, Default, Clone)]
pub struct RollkitConsensusBuilder;

impl RollkitConsensusBuilder {
    /// Create a new instance
    pub const fn new() -> Self {
        Self
    }
}

impl<Node> ConsensusBuilder<Node> for RollkitConsensusBuilder
where
    Node: FullNodeTypes,
    Node::Types: NodeTypes<ChainSpec = ChainSpec, Primitives = EthPrimitives>,
{
    type Consensus = Arc<dyn FullConsensus<EthPrimitives, Error = ConsensusError>>;

    async fn build_consensus(self, ctx: &BuilderContext<Node>) -> eyre::Result<Self::Consensus> {
        Ok(Arc::new(RollkitConsensus::new(ctx.chain_spec())) as Self::Consensus)
    }
}
