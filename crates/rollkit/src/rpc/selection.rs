use crate::types::WeightedTransaction;
use reth_transaction_pool::TransactionPool;
use tracing::debug;

/// Select transactions from the pool according to the specified strategy
pub fn select_transactions<Pool>(pool: &Pool, max_bytes: u64) -> Vec<WeightedTransaction>
where
    Pool: TransactionPool,
    Pool::Transaction: alloy_eips::eip2718::Encodable2718,
{
    let pending = pool.pending_transactions();
    let transactions: Vec<_> = pending.into_iter().collect();

    // Select transactions up to max_bytes
    let mut selected = Vec::new();
    let mut total_bytes = 0u64;

    for tx in transactions {
        let signed_tx = &tx.transaction;
        use alloy_eips::eip2718::Encodable2718;
        let mut buf = Vec::new();
        signed_tx.encode_2718(&mut buf);
        let encoded = buf;
        let tx_size = encoded.len() as u64;

        if total_bytes + tx_size > max_bytes {
            debug!(
                "Stopping selection: total_bytes({}) + tx_size({}) > max_bytes({})",
                total_bytes, tx_size, max_bytes
            );
            break;
        }

        selected.push(WeightedTransaction {
            tx: encoded.into(),
            weight: tx_size as i64,
        });

        total_bytes += tx_size;
    }

    debug!(
        "Selected {} transactions, total bytes: {}",
        selected.len(),
        total_bytes
    );

    selected
}
