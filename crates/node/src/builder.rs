use alloy_consensus::transaction::Transaction;
use lumen_rollkit::RollkitPayloadAttributes;
use reth_errors::RethError;
use reth_evm::{
    execute::{BlockBuilder, BlockBuilderOutcome},
    ConfigureEvm, NextBlockEnvAttributes,
};
use reth_evm_ethereum::EthEvmConfig;
use reth_payload_builder_primitives::PayloadBuilderError;
use reth_primitives::{transaction::SignedTransaction, Header, SealedBlock, SealedHeader};
use reth_provider::{HeaderProvider, StateProviderFactory};
use reth_revm::{database::StateProviderDatabase, State};
use std::sync::Arc;

/// Payload builder for Rollkit Reth node
#[derive(Debug)]
pub struct RollkitPayloadBuilder<Client> {
    /// The client for state access
    pub client: Arc<Client>,
    /// EVM configuration
    pub evm_config: EthEvmConfig,
}

impl<Client> RollkitPayloadBuilder<Client>
where
    Client: StateProviderFactory + HeaderProvider<Header = Header> + Send + Sync + 'static,
{
    /// Creates a new instance of `RollkitPayloadBuilder`
    pub const fn new(client: Arc<Client>, evm_config: EthEvmConfig) -> Self {
        Self { client, evm_config }
    }

    /// Builds a payload using the provided attributes
    pub async fn build_payload(
        &self,
        attributes: RollkitPayloadAttributes,
    ) -> Result<SealedBlock, PayloadBuilderError> {
        // Validate attributes
        attributes
            .validate()
            .map_err(|e| PayloadBuilderError::Internal(RethError::Other(Box::new(e))))?;

        // Get the latest state provider
        let state_provider = self.client.latest().map_err(PayloadBuilderError::other)?;

        // Create a database from the state provider
        let db = StateProviderDatabase::new(&state_provider);
        let mut state_db = State::builder()
            .with_database(db)
            .with_bundle_update()
            .build();

        // Get parent header using the client's HeaderProvider trait
        let parent_header = self
            .client
            .header(&attributes.parent_hash)
            .map_err(PayloadBuilderError::other)?
            .ok_or_else(|| {
                PayloadBuilderError::Internal(RethError::Other("Parent header not found".into()))
            })?;
        let sealed_parent = SealedHeader::new(parent_header, attributes.parent_hash);

        // Create next block environment attributes
        let gas_limit = attributes.gas_limit.ok_or_else(|| {
            PayloadBuilderError::Internal(RethError::Other(
                "Gas limit is required for rollkit payloads".into(),
            ))
        })?;

        let next_block_attrs = NextBlockEnvAttributes {
            timestamp: attributes.timestamp,
            suggested_fee_recipient: attributes.suggested_fee_recipient,
            prev_randao: attributes.prev_randao,
            gas_limit,
            parent_beacon_block_root: Some(alloy_primitives::B256::ZERO), // Set to zero for rollkit blocks
            withdrawals: None,
        };

        // Create block builder using the EVM config
        let mut builder = self
            .evm_config
            .builder_for_next_block(&mut state_db, &sealed_parent, next_block_attrs)
            .map_err(PayloadBuilderError::other)?;

        // Apply pre-execution changes
        builder
            .apply_pre_execution_changes()
            .map_err(|err| PayloadBuilderError::Internal(err.into()))?;

        // Execute transactions
        tracing::info!(
            transaction_count = attributes.transactions.len(),
            "Rollkit payload builder: executing transactions"
        );
        for (i, tx) in attributes.transactions.iter().enumerate() {
            tracing::debug!(
            index = i,
            hash = ?tx.hash(),
            nonce = tx.nonce(),
            gas_price = ?tx.gas_price(),
            gas_limit = tx.gas_limit(),
            "Processing transaction"
            );

            // Convert to recovered transaction for execution
            let recovered_tx = tx.try_clone_into_recovered().map_err(|_| {
                PayloadBuilderError::Internal(RethError::Other(
                    "Failed to recover transaction".into(),
                ))
            })?;

            // Execute the transaction
            match builder.execute_transaction(recovered_tx) {
                Ok(gas_used) => {
                    tracing::debug!(index = i, gas_used, "Transaction executed successfully");
                }
                Err(err) => {
                    // Log the error but continue with other transactions
                    tracing::warn!(index = i, error = ?err, "Transaction execution failed");
                }
            }
        }

        // Finish building the block - this calculates the proper state root
        let BlockBuilderOutcome {
            execution_result: _,
            hashed_state: _,
            trie_updates: _,
            block,
        } = builder
            .finish(&state_provider)
            .map_err(PayloadBuilderError::other)?;

        let sealed_block = block.sealed_block().clone();
        tracing::info!(
                    block_number = sealed_block.number,
                    block_hash = ?sealed_block.hash(),
                    transaction_count = sealed_block.transaction_count(),
                    gas_used = sealed_block.gas_used,
                    "Rollkit payload builder: built block"
        );

        // Return the sealed block
        Ok(sealed_block)
    }
}

/// Creates a new payload builder service
pub const fn create_payload_builder_service<Client>(
    client: Arc<Client>,
    evm_config: EthEvmConfig,
) -> Option<RollkitPayloadBuilder<Client>>
where
    Client: StateProviderFactory + HeaderProvider<Header = Header> + Send + Sync + 'static,
{
    Some(RollkitPayloadBuilder::new(client, evm_config))
}
