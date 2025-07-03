// src/eth_api_forwarder.rs — explicit pass‑through wrapper
// -----------------------------------------------------------------------------
// A *no‑macro* implementation of `EthApiServer` that forwards every method to
// an inner implementation, except for the handful you override manually
// (here: `send_raw_transaction` and `send_raw_transaction_sync`).
// Works with **reth‑rpc‑api v1.5.x** + Alloy 0.7.
// -----------------------------------------------------------------------------

#![allow(
    clippy::needless_lifetimes,
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::single_match
)]

use std::sync::Arc;

use async_trait::async_trait;
use jsonrpsee::{
    core::{client::ClientT, RpcResult},
    http_client::HttpClient,
};

use alloy_eips::{BlockId, BlockNumberOrTag};
use alloy_json_rpc::RpcObject;
use alloy_primitives::{Address, Bytes, B256, B64, U256, U64};
use alloy_rpc_types::{
    simulate::{SimulatePayload, SimulatedBlock},
    state::StateOverride,
    AccessListResult, Account, AccountInfo, BlockOverrides, Bundle, EIP1186AccountProofResponse,
    EthCallResponse, FeeHistory, Index, StateContext, SyncStatus, TransactionRequest, Work,
};
use alloy_serde::JsonStorageKey;

use reth_rpc_api::servers::eth::EthApiServer;
use reth_rpc_eth_api::{helpers::AddDevSigners, EthApiTypes, RpcNodeCore};

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
    /// Create a new `EthApiForwarder` instance.
    pub fn new(inner: I, remote: HttpClient) -> Self {
        Self {
            inner,
            remote: Arc::new(remote),
        }
    }
}

// Implement required traits for EthApiForwarder
impl<I> EthApiTypes for EthApiForwarder<I>
where
    I: EthApiTypes,
{
    type Error = I::Error;
    type NetworkTypes = I::NetworkTypes;
    type RpcConvert = I::RpcConvert;

    fn tx_resp_builder(&self) -> &Self::RpcConvert {
        self.inner.tx_resp_builder()
    }
}

// Implement RpcNodeCore for EthApiForwarder by delegating to inner
impl<I> RpcNodeCore for EthApiForwarder<I>
where
    I: RpcNodeCore,
{
    type Primitives = I::Primitives;
    type Provider = I::Provider;
    type Pool = I::Pool;
    type Evm = I::Evm;
    type Network = I::Network;
    type PayloadBuilder = I::PayloadBuilder;

    fn provider(&self) -> &Self::Provider {
        self.inner.provider()
    }

    fn pool(&self) -> &Self::Pool {
        self.inner.pool()
    }

    fn evm_config(&self) -> &Self::Evm {
        self.inner.evm_config()
    }

    fn network(&self) -> &Self::Network {
        self.inner.network()
    }

    fn payload_builder(&self) -> &Self::PayloadBuilder {
        self.inner.payload_builder()
    }
}

// Implement AddDevSigners for EthApiForwarder
impl<I> AddDevSigners for EthApiForwarder<I>
where
    I: AddDevSigners,
{
    fn with_dev_accounts(&self) {
        // Delegate to inner implementation
        self.inner.with_dev_accounts()
    }
}

// -----------------------------------------------------------------------------
//  Trait impl — *all* methods spelled out explicitly. Most just forward to
//  `self.inner`. The two tx‑submission helpers delegate to `self.remote`.
// -----------------------------------------------------------------------------
#[async_trait]
impl<I, T, B, R, H> EthApiServer<TransactionRequest, T, B, R, H> for EthApiForwarder<I>
where
    I: EthApiServer<TransactionRequest, T, B, R, H> + Send + Sync,
    T: RpcObject + Send + Sync + 'static,
    B: RpcObject + Send + Sync + 'static,
    R: RpcObject + Send + Sync + 'static,
    H: RpcObject + Send + Sync + 'static,
{
    // ---------- custom overrides ------------------------------------------------
    async fn send_transaction(&self, request: TransactionRequest) -> RpcResult<B256> {
        // For send_transaction, we need to sign the transaction locally first
        // then forward it as a raw transaction. This delegates to the inner
        // implementation which handles signing.
        self.inner.send_transaction(request).await
    }

    async fn send_raw_transaction(&self, raw_tx: Bytes) -> RpcResult<B256> {
        self.remote
            .request("eth_sendRawTransaction", vec![raw_tx])
            .await
            .map_err(|e| {
                jsonrpsee::types::error::ErrorObject::owned(
                    jsonrpsee::types::error::INTERNAL_ERROR_CODE,
                    format!("Failed to forward transaction: {e}"),
                    None::<String>,
                )
            })
    }

    async fn send_raw_transaction_sync(&self, raw_tx: Bytes) -> RpcResult<R> {
        // Note: This returns a receipt (R), not just a hash (B256)
        // We need to forward and wait for the receipt
        self.remote
            .request("eth_sendRawTransactionSync", vec![raw_tx])
            .await
            .map_err(|e| {
                jsonrpsee::types::error::ErrorObject::owned(
                    jsonrpsee::types::error::INTERNAL_ERROR_CODE,
                    format!("Failed to forward transaction sync: {e}"),
                    None::<String>,
                )
            })
    }

    // ---------- meta / chain ----------------------------------------------------
    async fn protocol_version(&self) -> RpcResult<U64> {
        self.inner.protocol_version().await
    }
    fn syncing(&self) -> RpcResult<SyncStatus> {
        self.inner.syncing()
    }
    async fn author(&self) -> RpcResult<Address> {
        self.inner.author().await
    }
    fn accounts(&self) -> RpcResult<Vec<Address>> {
        self.inner.accounts()
    }
    fn block_number(&self) -> RpcResult<U256> {
        self.inner.block_number()
    }
    async fn chain_id(&self) -> RpcResult<Option<U64>> {
        self.inner.chain_id().await
    }

    // ---------- blocks ----------------------------------------------------------
    async fn block_by_hash(&self, hash: B256, full: bool) -> RpcResult<Option<B>> {
        self.inner.block_by_hash(hash, full).await
    }
    async fn block_by_number(&self, number: BlockNumberOrTag, full: bool) -> RpcResult<Option<B>> {
        self.inner.block_by_number(number, full).await
    }
    async fn block_transaction_count_by_hash(&self, hash: B256) -> RpcResult<Option<U256>> {
        self.inner.block_transaction_count_by_hash(hash).await
    }
    async fn block_transaction_count_by_number(
        &self,
        number: BlockNumberOrTag,
    ) -> RpcResult<Option<U256>> {
        self.inner.block_transaction_count_by_number(number).await
    }
    async fn block_uncles_count_by_hash(&self, hash: B256) -> RpcResult<Option<U256>> {
        self.inner.block_uncles_count_by_hash(hash).await
    }
    async fn block_uncles_count_by_number(
        &self,
        number: BlockNumberOrTag,
    ) -> RpcResult<Option<U256>> {
        self.inner.block_uncles_count_by_number(number).await
    }
    async fn block_receipts(&self, id: BlockId) -> RpcResult<Option<Vec<R>>> {
        self.inner.block_receipts(id).await
    }
    async fn uncle_by_block_hash_and_index(&self, hash: B256, idx: Index) -> RpcResult<Option<B>> {
        self.inner.uncle_by_block_hash_and_index(hash, idx).await
    }
    async fn uncle_by_block_number_and_index(
        &self,
        number: BlockNumberOrTag,
        idx: Index,
    ) -> RpcResult<Option<B>> {
        self.inner
            .uncle_by_block_number_and_index(number, idx)
            .await
    }

    // ---------- transaction fetch ----------------------------------------------
    async fn raw_transaction_by_hash(&self, hash: B256) -> RpcResult<Option<Bytes>> {
        self.inner.raw_transaction_by_hash(hash).await
    }
    async fn transaction_by_hash(&self, hash: B256) -> RpcResult<Option<T>> {
        self.inner.transaction_by_hash(hash).await
    }
    async fn raw_transaction_by_block_hash_and_index(
        &self,
        hash: B256,
        idx: Index,
    ) -> RpcResult<Option<Bytes>> {
        self.inner
            .raw_transaction_by_block_hash_and_index(hash, idx)
            .await
    }
    async fn transaction_by_block_hash_and_index(
        &self,
        hash: B256,
        idx: Index,
    ) -> RpcResult<Option<T>> {
        self.inner
            .transaction_by_block_hash_and_index(hash, idx)
            .await
    }
    async fn raw_transaction_by_block_number_and_index(
        &self,
        number: BlockNumberOrTag,
        idx: Index,
    ) -> RpcResult<Option<Bytes>> {
        self.inner
            .raw_transaction_by_block_number_and_index(number, idx)
            .await
    }
    async fn transaction_by_block_number_and_index(
        &self,
        number: BlockNumberOrTag,
        idx: Index,
    ) -> RpcResult<Option<T>> {
        self.inner
            .transaction_by_block_number_and_index(number, idx)
            .await
    }
    async fn transaction_by_sender_and_nonce(
        &self,
        sender: Address,
        nonce: U64,
    ) -> RpcResult<Option<T>> {
        self.inner
            .transaction_by_sender_and_nonce(sender, nonce)
            .await
    }
    async fn transaction_receipt(&self, hash: B256) -> RpcResult<Option<R>> {
        self.inner.transaction_receipt(hash).await
    }

    // ---------- state & accounts ------------------------------------------------
    async fn balance(&self, addr: Address, at: Option<BlockId>) -> RpcResult<U256> {
        self.inner.balance(addr, at).await
    }
    async fn storage_at(
        &self,
        addr: Address,
        slot: JsonStorageKey,
        at: Option<BlockId>,
    ) -> RpcResult<B256> {
        self.inner.storage_at(addr, slot, at).await
    }
    async fn transaction_count(&self, addr: Address, at: Option<BlockId>) -> RpcResult<U256> {
        self.inner.transaction_count(addr, at).await
    }
    async fn get_code(&self, addr: Address, at: Option<BlockId>) -> RpcResult<Bytes> {
        self.inner.get_code(addr, at).await
    }
    async fn header_by_number(&self, number: BlockNumberOrTag) -> RpcResult<Option<H>> {
        self.inner.header_by_number(number).await
    }
    async fn header_by_hash(&self, hash: B256) -> RpcResult<Option<H>> {
        self.inner.header_by_hash(hash).await
    }

    // ---------- execution helpers ---------------------------------------------
    async fn simulate_v1(
        &self,
        payload: SimulatePayload,
        at: Option<BlockId>,
    ) -> RpcResult<Vec<SimulatedBlock<B>>> {
        self.inner.simulate_v1(payload, at).await
    }
    async fn call(
        &self,
        request: TransactionRequest,
        block_number: Option<BlockId>,
        state_overrides: Option<StateOverride>,
        block_overrides: Option<Box<BlockOverrides>>,
    ) -> RpcResult<Bytes> {
        self.inner
            .call(request, block_number, state_overrides, block_overrides)
            .await
    }
    async fn call_many(
        &self,
        bundles: Vec<Bundle>,
        state_context: Option<StateContext>,
        state_override: Option<StateOverride>,
    ) -> RpcResult<Vec<Vec<EthCallResponse>>> {
        self.inner
            .call_many(bundles, state_context, state_override)
            .await
    }
    async fn create_access_list(
        &self,
        request: TransactionRequest,
        block_number: Option<BlockId>,
        state_override: Option<StateOverride>,
    ) -> RpcResult<AccessListResult> {
        self.inner
            .create_access_list(request, block_number, state_override)
            .await
    }
    async fn estimate_gas(
        &self,
        request: TransactionRequest,
        block_number: Option<BlockId>,
        state_override: Option<StateOverride>,
    ) -> RpcResult<U256> {
        self.inner
            .estimate_gas(request, block_number, state_override)
            .await
    }
    async fn gas_price(&self) -> RpcResult<U256> {
        self.inner.gas_price().await
    }
    async fn get_account(&self, addr: Address, block: BlockId) -> RpcResult<Option<Account>> {
        self.inner.get_account(addr, block).await
    }
    async fn max_priority_fee_per_gas(&self) -> RpcResult<U256> {
        self.inner.max_priority_fee_per_gas().await
    }
    async fn blob_base_fee(&self) -> RpcResult<U256> {
        self.inner.blob_base_fee().await
    }
    async fn fee_history(
        &self,
        block_count: U64,
        newest: BlockNumberOrTag,
        reward_percentiles: Option<Vec<f64>>,
    ) -> RpcResult<FeeHistory> {
        self.inner
            .fee_history(block_count, newest, reward_percentiles)
            .await
    }

    // ---------- mining / hashrate --------------------------------------------
    async fn is_mining(&self) -> RpcResult<bool> {
        self.inner.is_mining().await
    }
    async fn hashrate(&self) -> RpcResult<U256> {
        self.inner.hashrate().await
    }
    async fn get_work(&self) -> RpcResult<Work> {
        self.inner.get_work().await
    }
    async fn submit_hashrate(&self, hashrate: U256, id: B256) -> RpcResult<bool> {
        self.inner.submit_hashrate(hashrate, id).await
    }
    async fn submit_work(&self, nonce: B64, pow_hash: B256, mix_digest: B256) -> RpcResult<bool> {
        self.inner.submit_work(nonce, pow_hash, mix_digest).await
    }

    // ---------- signing -------------------------------------------------------
    async fn sign(&self, addr: Address, data: Bytes) -> RpcResult<Bytes> {
        self.inner.sign(addr, data).await
    }
    async fn sign_transaction(&self, transaction: TransactionRequest) -> RpcResult<Bytes> {
        self.inner.sign_transaction(transaction).await
    }
    async fn sign_typed_data(
        &self,
        addr: Address,
        data: alloy_dyn_abi::TypedData,
    ) -> RpcResult<Bytes> {
        self.inner.sign_typed_data(addr, data).await
    }

    // ---------- proofs --------------------------------------------------------
    async fn get_proof(
        &self,
        addr: Address,
        keys: Vec<JsonStorageKey>,
        at: Option<BlockId>,
    ) -> RpcResult<EIP1186AccountProofResponse> {
        self.inner.get_proof(addr, keys, at).await
    }
    async fn get_account_info(&self, addr: Address, at: BlockId) -> RpcResult<AccountInfo> {
        self.inner.get_account_info(addr, at).await
    }
}
