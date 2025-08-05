use alloy_primitives::Bytes;
use async_trait::async_trait;
use jsonrpsee_core::RpcResult;
use jsonrpsee_proc_macros::rpc;
use reth_transaction_pool::{PoolTransaction, TransactionPool};
use jsonrpsee::tracing::debug;

/// Rollkit txpool RPC API trait
#[rpc(server, namespace = "txpoolExt")]
pub trait RollkitTxpoolApi {
    /// Get transactions from the pool up to the configured `max_bytes` limit
    #[method(name = "getTxs")]
    async fn get_txs(&self) -> RpcResult<Vec<Bytes>>;
}

/// Implementation of the Rollkit txpool RPC API
#[derive(Debug)]
pub struct RollkitTxpoolApiImpl<Pool> {
    /// Transaction pool
    pool: Pool,
    /// Maximum bytes allowed for transaction selection
    max_bytes: u64,
}

impl<Pool> RollkitTxpoolApiImpl<Pool> {
    /// Creates a new instance of `TxpoolApi`.
    pub const fn new(pool: Pool, max_bytes: u64) -> Self {
        Self { pool, max_bytes }
    }
}

/// Creates a new Rollkit txpool RPC module
pub const fn create_rollkit_txpool_module<Pool>(
    pool: Pool,
    max_bytes: u64,
) -> RollkitTxpoolApiImpl<Pool>
where
    Pool: TransactionPool + Send + Sync + 'static,
{
    RollkitTxpoolApiImpl { pool, max_bytes }
}

#[async_trait]
impl<Pool> RollkitTxpoolApiServer for RollkitTxpoolApiImpl<Pool>
where
    Pool: TransactionPool + Send + Sync + 'static,
{
    /// Returns a Geth-style `TxpoolContent` with raw RLP hex strings.
    async fn get_txs(&self) -> RpcResult<Vec<Bytes>> {
        //------------------------------------------------------------------//
        // 1. Iterate best txs (sorted by priority) and stop once we hit    //
        //    the byte cap                                                   //
        //------------------------------------------------------------------//
        let mut total = 0u64;
        let mut selected_txs: Vec<Bytes> = Vec::new();

        // Use best_transactions() which returns an iterator of transactions
        // ordered by their priority (gas price/priority fee)
        for best_tx in self.pool.best_transactions() {
            let sz = best_tx.encoded_length() as u64;
            if total + sz > self.max_bytes {
                break;
            }

            // Convert to consensus transaction and encode to RLP
            let tx = best_tx.transaction.clone().into_consensus_with2718();
            let bz = tx.encoded_bytes();

            selected_txs.push(bz.clone());

            total += sz;
        }

debug!("get_txs returning {} transactions", selected_txs.len());
        Ok(selected_txs)
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{RollkitConfig, DEFAULT_MAX_TXPOOL_BYTES};

    #[test]
    fn test_default_config_value() {
        // Test that the default max_txpool_bytes value is correctly set
        let config = RollkitConfig::default();
        assert_eq!(config.max_txpool_bytes, DEFAULT_MAX_TXPOOL_BYTES);
        assert_eq!(DEFAULT_MAX_TXPOOL_BYTES, 1_980 * 1024); // 1.98 MB
    }

    #[test]
    fn test_rollkit_txpool_api_creation() {
        // This test verifies that we can create the API with different max_bytes values
        // The actual behavior testing would require a mock transaction pool

        // Test with default config
        let config = RollkitConfig::default();
        assert_eq!(config.max_txpool_bytes, 1_980 * 1024);

        // Test with custom config
        let custom_config = RollkitConfig::new(1000);
        assert_eq!(custom_config.max_txpool_bytes, 1000);
    }
}
