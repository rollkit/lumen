use crate::{
    config::RollkitConfig, rpc::selection::select_transactions, types::WeightedTransaction,
};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
};
use reth_transaction_pool::TransactionPool;
use std::sync::Arc;
use tracing::debug;

/// Rollkit txpool RPC API trait
#[rpc(server, namespace = "txpool")]
pub trait RollkitTxpoolApi {
    /// Get transactions from the pool up to the configured max_bytes limit
    #[method(name = "getTxs")]
    async fn get_txs(&self) -> RpcResult<Vec<WeightedTransaction>>;
}

/// Implementation of the Rollkit txpool RPC API
#[derive(Debug)]
pub struct RollkitTxpoolApiImpl<Pool> {
    /// Transaction pool
    pool: Pool,
    /// Rollkit configuration
    config: Arc<RollkitConfig>,
}

impl<Pool> RollkitTxpoolApiImpl<Pool> {
    /// Creates a new instance of the txpool API
    pub fn new(pool: Pool, config: Arc<RollkitConfig>) -> Self {
        Self { pool, config }
    }
}

#[async_trait]
impl<Pool> RollkitTxpoolApiServer for RollkitTxpoolApiImpl<Pool>
where
    Pool: TransactionPool + 'static,
    Pool::Transaction: alloy_eips::eip2718::Encodable2718,
{
    async fn get_txs(&self) -> RpcResult<Vec<WeightedTransaction>> {
        let selected_txs = select_transactions(&self.pool, self.config.max_txpool_bytes);

        Ok(selected_txs)
    }
}

/// Creates a new Rollkit txpool RPC module
pub fn create_rollkit_txpool_module<Pool>(
    pool: Pool,
    config: Arc<RollkitConfig>,
) -> RollkitTxpoolApiImpl<Pool>
where
    Pool: TransactionPool + 'static,
    Pool::Transaction: alloy_eips::eip2718::Encodable2718,
{
    RollkitTxpoolApiImpl::new(pool, config)
}
