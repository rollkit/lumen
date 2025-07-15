use crate::{config::RollkitConfig, rpc::selection::select_transactions};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
};
use reth_transaction_pool::TransactionPool;
use std::sync::Arc;


/// Rollkit txpool RPC API trait
#[rpc(server, namespace = "txpoolExt")]
pub trait RollkitTxpoolApi {
    /// Get transactions from the pool up to the configured max_bytes limit
    #[method(name = "getTxs")]
    async fn get_txs(&self) -> RpcResult<Vec<<Pool as TransactionPool>::Transaction>>>;
}

/// Implementation of the Rollkit txpool RPC API
#[derive(Debug)]
pub struct RollkitTxpoolApiImpl<Pool> {
    /// Transaction pool
    pool: Pool,
    /// Rollkit configuration
    config: Arc<RollkitConfig>,
}

#[async_trait]
impl<Pool> RollkitTxpoolApiServer for RollkitTxpoolApiImpl<Pool>
where
    Pool: TransactionPool + Clone + 'static,
{
    async fn get_txs(&self) -> RpcResult<Vec<<Pool as TransactionPool>::Transaction>> {
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
    Pool: TransactionPool<Transaction = reth_transaction_pool::EthPooledTransaction> + 'static,
{
    RollkitTxpoolApiImpl { pool, config }
}
