use alloy_primitives::{hex::encode as hex_encode, Address};
use alloy_rlp::encode as rlp_encode;
use alloy_rpc_types_txpool::TxpoolContent;
use async_trait::async_trait;
use jsonrpsee::{core::RpcResult, proc_macros::rpc, RpcModule};
use reth_transaction_pool::{TransactionPool, ValidPoolTransaction};
use std::collections::BTreeMap;

/// Rollkit txpool RPC API trait
#[rpc(server, namespace = "txpoolExt")]
pub trait RollkitTxpoolApi {
    /// Get transactions from the pool up to the configured max_bytes limit
    #[method(name = "getTxs")]
    async fn get_txs(&self) -> RpcResult<TxpoolContent<String>>;
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
pub fn create_rollkit_txpool_module<Pool>(pool: Pool, max_bytes: u64) -> RollkitTxpoolApiImpl<Pool>
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
    /// Returns a Geth-style TxpoolContent with raw RLP hex strings.
    async fn get_txs(&self) -> RpcResult<TxpoolContent<String>> {
        //------------------------------------------------------------------//
        // 1. Iterate pending txs and stop once we hit the byte cap         //
        //------------------------------------------------------------------//
        let mut total = 0u64;
        let mut pending_map: BTreeMap<Address, BTreeMap<String, String>> = BTreeMap::new();

        for arc_tx in self.pool.pending_transactions() {
            // deref Arc<ValidPoolTransaction<_>>
            let pooled: &ValidPoolTransaction<_> = &arc_tx;

            let sz = pooled.encoded_length() as u64;
            if total + sz > self.max_bytes {
                break;
            }

            // sender / nonce helpers come from PoolTransaction
            let sender = pooled.sender();
            let nonce = pooled.nonce().to_string();

            // inside the loop
            let tx = pooled.to_consensus();
            let rlp_bytes = rlp_encode(&tx); // Vec<u8>
            let rlp_hex = format!("0x{}", hex_encode(&rlp_bytes));

            pending_map
                .entry(sender)
                .or_default()
                .insert(nonce, rlp_hex);

            total += sz;
        }

        let content = TxpoolContent {
            pending: pending_map,
            queued: BTreeMap::new(), // not collected for now
        };

        Ok(content)
    }
}
