use reth_transaction_pool::TransactionPool;
use tracing::debug;

/// Select transactions from the pool according to the specified strategy
pub fn select_transactions<Pool>(
    pool: &Pool,
    max_bytes: u64,
) -> Vec<<Pool as TransactionPool>::Transaction>
where
    Pool: TransactionPool + Clone + 'static,
{
    let pending = pool.pending_transactions();
    let transactions: Vec<_> = pending.into_iter().collect();

    // Select transactions up to max_bytes
    let mut selected = Vec::new();
    let mut total_bytes = 0u64;

    for tx in transactions {
        let tx_size: u64 = tx.encoded_length() as u64;

        if total_bytes + tx_size > max_bytes {
            debug!(
                "Stopping selection: total_bytes({}) + tx_size({}) > max_bytes({})",
                total_bytes, tx_size, max_bytes
            );
            break;
        }

        // Extract the inner transaction from the pooled transaction

        let signed_tx = tx.transaction.clone();
        selected.push(signed_tx);

        total_bytes += tx_size;
    }

    debug!(
        "Selected {} transactions, total bytes: {}",
        selected.len(),
        total_bytes
    );

    selected
}
