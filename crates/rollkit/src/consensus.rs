//! Rollkit custom consensus implementation that allows same timestamps across blocks.

use reth_chainspec::ChainSpec;
use reth_consensus::{Consensus, ConsensusError, FullConsensus, HeaderValidator};
use reth_consensus_common::validation::validate_body_against_header;
use reth_ethereum::node::builder::{components::ConsensusBuilder, BuilderContext};
use reth_ethereum_consensus::EthBeaconConsensus;
use reth_ethereum_primitives::{Block, BlockBody, EthPrimitives, Receipt};
use reth_execution_types::BlockExecutionResult;
use reth_node_api::{FullNodeTypes, NodeTypes};
use reth_primitives::{GotExpected, GotExpectedBoxed, RecoveredBlock, SealedBlock, SealedHeader};
use std::sync::Arc;

/// Builder for `RollkitConsensus`
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct RollkitConsensusBuilder;

impl RollkitConsensusBuilder {
    /// Create a new `RollkitConsensusBuilder`
    pub const fn new() -> Self {
        Self
    }

    /// Build the consensus implementation
    pub fn build(chain_spec: Arc<ChainSpec>) -> Arc<RollkitConsensus> {
        Arc::new(RollkitConsensus::new(chain_spec))
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

/// Rollkit consensus implementation that allows blocks with the same timestamp.
///
/// This consensus implementation wraps the standard Ethereum beacon consensus
/// but modifies the timestamp validation to allow multiple blocks to have the
/// same timestamp, which is required for Rollkit's operation.
#[derive(Debug, Clone)]
pub struct RollkitConsensus {
    /// Inner Ethereum beacon consensus for standard validation
    inner: EthBeaconConsensus<ChainSpec>,
}

impl RollkitConsensus {
    /// Create a new Rollkit consensus instance
    pub const fn new(chain_spec: Arc<ChainSpec>) -> Self {
        let inner = EthBeaconConsensus::new(chain_spec);
        Self { inner }
    }
}

impl HeaderValidator for RollkitConsensus {
    fn validate_header(&self, header: &SealedHeader) -> Result<(), ConsensusError> {
        // Use inner consensus for basic header validation
        self.inner.validate_header(header)
    }

    fn validate_header_against_parent(
        &self,
        header: &SealedHeader,
        parent: &SealedHeader,
    ) -> Result<(), ConsensusError> {
        // Custom validation that allows same timestamps
        // This is the key difference from standard Ethereum consensus

        // First validate parent hash and number
        if header.parent_hash != parent.hash() {
            return Err(ConsensusError::ParentHashMismatch(GotExpectedBoxed(
                Box::new(GotExpected {
                    got: header.parent_hash,
                    expected: parent.hash(),
                }),
            )));
        }

        if header.number != parent.number + 1 {
            return Err(ConsensusError::ParentBlockNumberMismatch {
                parent_block_number: parent.number,
                block_number: header.number,
            });
        }

        // ROLLKIT MODIFICATION: Allow same timestamp
        // Standard Ethereum requires: header.timestamp > parent.timestamp
        // Rollkit allows: header.timestamp >= parent.timestamp
        if header.timestamp < parent.timestamp {
            return Err(ConsensusError::TimestampIsInPast {
                parent_timestamp: parent.timestamp,
                timestamp: header.timestamp,
            });
        }
        // NOTE: We explicitly do NOT check for header.timestamp == parent.timestamp
        // as an error, which is the main change for Rollkit

        // For all other validations, delegate to the inner consensus
        // but skip it when timestamps are equal since the inner consensus
        // would reject this case
        if header.timestamp == parent.timestamp {
            // Timestamps are equal, which we explicitly allow for Rollkit
            // Skip the inner consensus validation that would reject this
            Ok(())
        } else {
            // Timestamps are different, so we can safely delegate to inner consensus
            self.inner.validate_header_against_parent(header, parent)
        }
    }
}

impl Consensus<Block> for RollkitConsensus {
    type Error = ConsensusError;

    fn validate_body_against_header(
        &self,
        body: &BlockBody,
        header: &SealedHeader,
    ) -> Result<(), Self::Error> {
        validate_body_against_header(body, header.header())
    }

    fn validate_block_pre_execution(&self, block: &SealedBlock) -> Result<(), Self::Error> {
        // Use inner consensus for pre-execution validation
        self.inner.validate_block_pre_execution(block)
    }
}

impl FullConsensus<EthPrimitives> for RollkitConsensus {
    fn validate_block_post_execution(
        &self,
        block: &RecoveredBlock<Block>,
        result: &BlockExecutionResult<Receipt>,
    ) -> Result<(), ConsensusError> {
        <EthBeaconConsensus<ChainSpec> as FullConsensus<EthPrimitives>>::validate_block_post_execution(&self.inner, block, result)
    }
}
