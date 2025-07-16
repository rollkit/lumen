use alloy_primitives::{hex::encode as hex_encode, Address};
use alloy_rlp::Encodable;
use alloy_rpc_types_txpool::TxpoolContent;
use async_trait::async_trait;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use reth_transaction_pool::{TransactionPool, ValidPoolTransaction};
use std::collections::BTreeMap;

/// Rollkit txpool RPC API trait
#[rpc(server, namespace = "txpoolExt")]
pub trait RollkitTxpoolApi {
    /// Get transactions from the pool up to the configured `max_bytes` limit
    #[method(name = "getTxs")]
    async fn get_txs(&self) -> RpcResult<Vec<String>>;
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
    async fn get_txs(&self) -> RpcResult<Vec<String>> {
        //------------------------------------------------------------------//
        // 1. Iterate pending txs and stop once we hit the byte cap         //
        //------------------------------------------------------------------//
        let mut total = 0u64;
        let mut pending_map: Vec<String> = Vec::new();

        for arc_tx in self.pool.pending_transactions() {
            // deref Arc<ValidPoolTransaction<_>>
            let pooled: &ValidPoolTransaction<_> = &arc_tx;

            let sz = pooled.encoded_length() as u64;
            if total + sz > self.max_bytes {
                break;
            }

            // inside the loop
            let tx = pooled.to_consensus();
            let mut rlp_bytes = Vec::new();
            tx.encode(&mut rlp_bytes); // encode into Vec<u8>
            let rlp_hex = format!("0x{}", hex_encode(&rlp_bytes));

            pending_map.push(rlp_hex);

            total += sz;
        }

        Ok(pending_map)
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
