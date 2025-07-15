# Custom RPC Implementation Notes

## Current Status

The Lumen node currently uses the default node add-ons without custom RPC modules or engine validators. The custom implementations have been disabled due to complex trait bounds.

## What We Want to Implement

1. **Custom Engine Validator** (`RollkitEngineValidator`): Already implemented but not integrated
   - Bypasses block hash validation for Rollkit compatibility
   - Validates Rollkit-specific payload attributes

2. **Custom RPC Module** (`txpool_getTxs`): Already implemented but not integrated
   - Returns weighted transactions from the txpool
   - Respects byte size limits configured via CLI args

## Integration Challenges

### NodeAddOns Trait
The `NodeAddOns` trait requires:
```rust
type Handle: Send + Sync + Clone;
async fn launch_add_ons(self, ctx: AddOnsContext<'_, N>) -> eyre::Result<Self::Handle>;
```

### RethRpcAddOns Trait
For proper RPC integration, the add-ons must implement `RethRpcAddOns<N>` which requires:
- `NodeAddOns<N, Handle = RpcHandle<N, Self::EthApi>>`
- Complex trait bounds on `N: FullNodeComponents`
- Proper EVM configuration traits

### Current Blockers

1. **Trait Bounds**: The generic bounds required for custom implementations are complex:
   - `N: FullNodeComponents` with specific associated types
   - EVM configuration requiring `From<NextBlockEnvAttributes>` implementations
   - Error type conversions for `EthApiError`

2. **Private Fields**: `EthereumAddOns` has private fields, making it hard to customize

3. **Type Complexity**: The full type signature for custom add-ons is verbose and requires many imports

## Potential Solutions

1. **Use RpcHooks**: The proper way seems to be using `RpcHooks` with `extend_rpc_modules`
2. **Custom RethRpcAddOns Implementation**: Create a complete custom implementation
3. **Fork/Patch Reth**: Modify reth to expose necessary APIs

## Next Steps

1. Study examples in reth repository (e.g., custom-engine-types example)
2. Consider using execution extensions (ExEx) for custom functionality
3. Implement a minimal `RethRpcAddOns` that satisfies all trait bounds
4. File an issue with reth for better extensibility APIs