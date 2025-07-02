#![allow(clippy::needless_lifetimes, clippy::type_complexity)]

use std::sync::Arc;

use async_trait::async_trait;
use delegate::delegate;
use jsonrpsee::{http_client::HttpClient, types::RpcResult};

use reth_primitives::H256;
use reth_rpc_api::servers::eth::EthApiServer;
use reth_rpc_eth_types::{
    Account, AccountInfo, BlockId, BlockNumberOrTag, EIP1186AccountProofResponse, FeeHistory,
    Index, SimulationTrace,
};

use alloy_eips::eip2930::AccessListWithGasUsed;
use alloy_primitives::B256;

use alloy_primitives::{Address, Bytes, U256, U64};

/// Thin wrapper that adds selective forwarding on top of an existing
/// `EthApiServer` implementation.
#[derive(Clone, Debug)]
pub struct EthApiForwarder<I> {
    /// The implementation we keep for read‑only paths (usually `EthApi`).
    pub inner: I,
    /// Remote endpoint we forward write‑heavy calls to (e.g. sequencer, L1 EL).
    pub remote: Arc<HttpClient>,
}

impl<I> EthApiForwarder<I> {
    pub fn new(inner: I, remote: HttpClient) -> Self {
        Self {
            inner,
            remote: Arc::new(remote),
        }
    }
}

// -----------------------------------------------------------------------------
//  Full trait impl — we override only the endpoints we **care** about. For every
//  other RPC the `delegate!` macro generates a `self.inner.<method>(..)` call
//  with the correct signature.
// -----------------------------------------------------------------------------
#[async_trait]
impl<I, TxReq, T, B, R> EthApiServer<TxReq, T, B, R> for EthApiForwarder<I>
where
    I: EthApiServer<TxReq, T, B, R> + Send + Sync,
    TxReq: RpcObject + Send + Sync + 'static,
    T: RpcObject + Send + Sync + 'static,
    B: RpcObject + Send + Sync + 'static,
    R: RpcObject + Send + Sync + 'static,
{
    // ---------- CUSTOM OVERRIDES ------------------------------------------------
    async fn send_raw_transaction(&self, raw_tx: Bytes) -> RpcResult<B256> {
        // Forward writes to the remote node (e.g. L2 sequencer).
        // Uses the **same** error semantics as jsonrpsee’s auto‑gen client.
        self.remote
            .request("eth_sendRawTransaction", vec![raw_tx])
            .await
    }

    async fn send_transaction(&self, raw_tx: Bytes) -> RpcResult<B256> {
        // Forward writes to the remote node (e.g. L2 sequencer).
        // Uses the **same** error semantics as jsonrpsee’s auto‑gen client.
        self.remote
            .request("eth_sendTransaction", vec![raw_tx])
            .await
    }

    async fn send_raw_transaction_sync(&self, raw_tx: Bytes) -> RpcResult<B256> {
        // Blocking variant until transaction enters canonical chain. Adjust the
        // method name if your remote uses a different extension.
        self.remote
            .request("eth_sendRawTransactionSync", vec![raw_tx])
            .await
    }

    // ---------- ALL OTHER METHODS: AUTO‑DELEGATED -------------------------------
    // The `delegate!` macro expands each item below into a normal function body
    // that forwards to `self.inner` (awaiting when necessary). Keep it in sync
    // with upstream Reth when new RPCs are added.
    delegate! {
        to self.inner {
            // ── meta / chain ──────────────────────────────────────────────────
            async fn protocol_version(&self) -> RpcResult<U64>;
            fn syncing(&self) -> RpcResult<alloy_rpc_types::SyncStatus>;
            async fn author(&self) -> RpcResult<Address>;
            fn accounts(&self) -> RpcResult<Vec<Address>>;
            fn block_number(&self) -> RpcResult<U256>;
            async fn chain_id(&self) -> RpcResult<Option<U64>>;

            // ── blocks ────────────────────────────────────────────────────────
            async fn block_by_hash(&self, hash: B256, full: bool) -> RpcResult<Option<B>>;
            async fn block_by_number(&self, number: BlockNumberOrTag, full: bool) -> RpcResult<Option<B>>;
            async fn block_transaction_count_by_hash(&self, hash: B256) -> RpcResult<Option<U256>>;
            async fn block_transaction_count_by_number(&self, number: BlockNumberOrTag) -> RpcResult<Option<U256>>;
            async fn block_uncles_count_by_hash(&self, hash: B256) -> RpcResult<Option<U256>>;
            async fn block_uncles_count_by_number(&self, number: BlockNumberOrTag) -> RpcResult<Option<U256>>;
            async fn block_receipts(&self, id: BlockId) -> RpcResult<Option<Vec<R>>>;
            async fn uncle_by_block_hash_and_index(&self, hash: B256, idx: Index) -> RpcResult<Option<B>>;
            async fn uncle_by_block_number_and_index(&self, number: BlockNumberOrTag, idx: Index) -> RpcResult<Option<B>>;

            // ── transactions (fetch) ──────────────────────────────────────────
            async fn raw_transaction_by_hash(&self, hash: B256) -> RpcResult<Option<Bytes>>;
            async fn transaction_by_hash(&self, hash: B256) -> RpcResult<Option<T>>;
            async fn raw_transaction_by_block_hash_and_index(&self, hash: B256, idx: Index) -> RpcResult<Option<Bytes>>;
            async fn transaction_by_block_hash_and_index(&self, hash: B256, idx: Index) -> RpcResult<Option<T>>;
            async fn raw_transaction_by_block_number_and_index(&self, number: BlockNumberOrTag, idx: Index) -> RpcResult<Option<Bytes>>;
            async fn transaction_by_block_number_and_index(&self, number: BlockNumberOrTag, idx: Index) -> RpcResult<Option<T>>;
            async fn transaction_by_sender_and_nonce(&self, sender: Address, nonce: U256) -> RpcResult<Option<T>>;
            async fn transaction_receipt(&self, hash: B256) -> RpcResult<Option<R>>;

            // ── state & accounts ──────────────────────────────────────────────
            async fn balance(&self, addr: Address, at: Option<BlockId>) -> RpcResult<U256>;
            async fn storage_at(&self, addr: Address, slot: B256, at: Option<BlockId>) -> RpcResult<B256>;
            async fn transaction_count(&self, addr: Address, at: Option<BlockId>) -> RpcResult<U256>;
            async fn get_code(&self, addr: Address, at: Option<BlockId>) -> RpcResult<Bytes>;
            async fn header_by_number(&self, number: BlockNumberOrTag) -> RpcResult<Option<H>>;
            async fn header_by_hash(&self, hash: B256) -> RpcResult<Option<H>>;

            // ── execution helpers ─────────────────────────────────────────────
            async fn simulate_v1(&self, reqs: Vec<TxReq>) -> RpcResult<Vec<SimulationTrace>>;
            async fn call(&self, req: TxReq, block: Option<BlockId>) -> RpcResult<Bytes>;
            async fn call_many(&self, batch: Vec<(TxReq, Option<BlockId>)>) -> RpcResult<Vec<Bytes>>;
            async fn create_access_list(&self, tx: TxReq, block: Option<BlockId>) -> RpcResult<AccessListWithGasUsed>;
            async fn estimate_gas(&self, tx: TxReq, block: Option<BlockId>) -> RpcResult<U256>;
            async fn gas_price(&self) -> RpcResult<U256>;
            async fn get_account(&self, addr: Address, block: Option<BlockId>) -> RpcResult<Account>;
            async fn max_priority_fee_per_gas(&self) -> RpcResult<U256>;
            async fn blob_base_fee(&self) -> RpcResult<Option<U256>>;
            async fn fee_history(&self, block_count: U256, newest: BlockNumberOrTag, reward_percentiles: Option<Vec<f64>>) -> RpcResult<FeeHistory>;

            // ── mining / hashrate ─────────────────────────────────────────────
            fn is_mining(&self) -> RpcResult<bool>;
            fn hashrate(&self) -> RpcResult<U256>;
            fn get_work(&self) -> RpcResult<[B256; 3]>;
            fn submit_hashrate(&self, rate: U256, id: B256) -> RpcResult<bool>;
            fn submit_work(&self, nonce: B256, pow_hash: B256, mix_digest: B256) -> RpcResult<bool>;

            // ── signing ───────────────────────────────────────────────────────
            async fn sign(&self, addr: Address, data: Bytes) -> RpcResult<Bytes>;
            async fn sign_transaction(&self, tx: TxReq) -> RpcResult<Bytes>;
            async fn sign_typed_data(&self, addr: Address, payload: Bytes) -> RpcResult<Bytes>;

            // ── proofs ────────────────────────────────────────────────────────
            async fn get_proof(&self, addr: Address, slots: Vec<B256>, at: Option<BlockId>) -> RpcResult<EIP1186AccountProofResponse>;
            async fn get_account_info(&self, addr: Address, at: Option<BlockId>) -> RpcResult<Option<AccountInfo>>;
        }
    }
}
