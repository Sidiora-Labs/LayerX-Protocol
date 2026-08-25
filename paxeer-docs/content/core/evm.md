# EVM Module

The EVM module (`paxeer-network/modules/evm/`) integrates a full Ethereum Virtual Machine into Paxeer as chain ID **125**. It enables native EVM transaction execution, bidirectional address mapping between Pax and Ethereum, and interoperability between CosmWasm and EVM via pointer contracts.

## Chain ID

Paxeer operates as Ethereum chain ID **125**. All EVM transactions must sign with this chain ID to be accepted. This is enforced during signature recovery in the ante handler.

## Dual Address Model

Every account can have both a Pax (bech32) address and an EVM (hex) address. The module maintains bidirectional mappings in `modules/evm/keeper/address.go`:

### Explicit Association

Users link addresses via:

1. **Associate transaction** — a dedicated Paxeer transaction type that binds addresses
2. **Implicit derivation** — signing any Cosmos or EVM transaction automatically derives and associates the EVM address from the secp256k1 public key

The keeper stores mappings:

```go
// modules/evm/keeper/address.go
func (k *Keeper) SetAddressMapping(ctx sdk.Context, paxAddr sdk.AccAddress, evmAddr common.Address)
func (k *Keeper) GetPaxAddress(ctx sdk.Context, evmAddr common.Address) (sdk.AccAddress, bool)
func (k *Keeper) GetEVMAddress(ctx sdk.Context, paxAddr sdk.AccAddress) (common.Address, bool)
```

### Cast Addresses

When no explicit association exists, the module falls back to deterministic byte-cast between address formats. Cast addresses have limitations:

- **Two views of the same account** — balances and state appear different from Pax and EVM perspectives
- **Limited interoperability** — pointer contracts and cross-VM calls require explicit association

Cast addresses are used only as a fallback and are not recommended for production use.

## Transaction Execution

EVM transactions arrive as `MsgEVMTransaction` and are unpacked into native Ethereum transaction types:

- **Type 0** — Legacy transactions
- **Type 1** — EIP-2930 access list transactions
- **Type 2** — EIP-1559 dynamic fee transactions
- **Type 4** — EIP-7702 set-code transactions (code delegation)

The execution flow (`modules/evm/keeper/keeper.go`):

1. Ante handler validates signature, nonce, gas, fees
2. EVM keeper constructs block context from consensus data
3. Executor (from `engine/executor/`) runs the transaction
4. State changes are committed to the Cosmos store
5. Receipt is written to receipt store (`storage/ledger_db/receipt/`)

Gas is charged in Cosmos SDK units and converted to EVM units via a configurable normalizer (default 1:1).

## Ante Handler

EVM transactions bypass the standard Cosmos ante chain and use a dedicated EVM ante handler (`modules/evm/keeper/ante.go`). The pipeline enforces:

1. **Cosmos field rejection** — EVM txs must not use memo, timeout, or Cosmos-style fees
2. **Preprocessing** — unpack inner Ethereum tx, recover sender via ECDSA
3. **Address derivation** — for Cosmos-signed txs, derive EVM address from pubkey
4. **Basic validation** — init code size limits, non-negative value, intrinsic gas calculation
5. **Signature/nonce verification** — validate chain ID and nonce ordering
6. **Fee validation** — check base fee and minimum fee, execute gas purchase
7. **Gas metering** — set Cosmos gas meter to converted EVM gas limit

Nonce handling differs between CheckTx and DeliverTx:

- **CheckTx** — uses pending nonce logic for mempool ordering (allows gaps)
- **DeliverTx** — requires exact nonce match (must be `account.nonce + 1`)

## StateDB Implementation

The `state` package (`modules/evm/state/statedb.go`) implements go-ethereum's `vm.StateDB` interface on top of Cosmos SDK stores. This is the core bridge enabling EVM execution within the Cosmos framework.

### Balance Representation

Paxeer uses 6-decimal **µhpx** as the native unit, while EVM expects 18-decimal **wei**. The StateDB handles conversion:

- EVM operations work in wei (18 decimals)
- Cosmos stores balances in µhpx (6 decimals)
- Sub-µhpx remainder (the lower 12 decimal places) is tracked separately in a `wei` store

When an EVM transaction sends `1.5 * 10^12 wei`, the StateDB:
1. Converts to µhpx: `1.5 * 10^12 / 10^12 = 1.5 µhpx` → stores `1 µhpx`
2. Stores remainder: `0.5 * 10^12 wei` in the `wei` store

This ensures sub-µhpx precision is preserved across EVM operations without modifying the bank module.

### Snapshots and Reverts

The StateDB uses `CacheMultiStore` for snapshotting:

```go
func (s *StateDB) Snapshot() int {
    id := s.snapCounter
    s.snapshots[id] = s.ctx.MultiStore().CacheMultiStore()
    s.snapCounter++
    return id
}

func (s *StateDB) RevertToSnapshot(snapID int) {
    cachedStore := s.snapshots[snapID]
    s.ctx = s.ctx.WithMultiStore(cachedStore)
    s.journal.Revert(snapID)
}
```

A journal records every state mutation (balance change, storage write, code update) so it can be rolled back on revert. The journal is cleared at transaction boundaries.

### Transient State

The following state is held in memory per-transaction and not persisted:

- **Logs** — EVM event logs
- **Transient storage** — EIP-1153 TLOAD/TSTORE
- **Access lists** — EIP-2929/2930 accessed addresses and storage keys
- **Gas refunds** — gas refund counter

These are finalized only when the transaction commits successfully.

## Receipts and Logs

### Receipt Structure

Receipts are built after execution and contain:

- Transaction hash
- Block number and transaction index
- Gas used
- Cumulative gas used in block
- Contract address (if contract creation)
- Status (1 = success, 0 = failure)
- Logs (event emissions)
- Logs bloom filter

Receipts are stored in `storage/ledger_db/receipt/receipt_store.go` indexed by transaction hash and block height.

### Synthetic Receipts

Paxeer creates synthetic receipts for cross-VM interactions:

**CW→EVM** — if a Cosmos transaction calls an EVM contract, a synthetic receipt carries EVM-side logs

**CW Pointee** — if a Cosmos transaction calls a CosmWasm contract with an EVM pointer, synthetic logs are emitted to represent activity from the EVM perspective

**EVM→CW** — if an EVM transaction calls a CosmWasm contract via pointer, synthetic logs are added to the EVM transaction's receipt

### Failure Receipts

Unlike Ethereum, EVM transactions on Paxeer can fail before reaching the EVM:

- **Nonce mismatch** — no receipt (no state change)
- **Other ante failures** — status-0 receipt (nonce incremented, gas consumed)

This ensures receipts exist for all state-changing failures.

### Block Bloom Filters

The EVM keeper aggregates bloom filters from all receipts in a block to create:

- **Block bloom** — includes all logs (EVM + CW synthetic)
- **EVM-only bloom** — excludes CW-originated logs

These are stored per-height for efficient log filtering.

## Deferred Processing

Rather than finalizing during execution, some work is deferred to EndBlock (`modules/evm/keeper/keeper.go`):

1. **Fee surplus collection** — the difference between paid and required fees is swept to the fee collector
2. **Failed receipts** — written for transactions that failed during execution
3. **Block bloom aggregation** — composed from all transaction blooms

Deferred info is stored in transient storage during the block and processed in `EndBlock`.

### Coinbase Addresses

Each transaction receives a deterministic coinbase address for collecting fee surplus. At EndBlock, all coinbase balances are swept to the module's fee collector account.

## Dynamic Base Fee (EIP-1559)

The module implements EIP-1559 dynamic fee adjustment (`modules/evm/keeper/fee.go`):

```go
func (k *Keeper) EndBlock(ctx sdk.Context) {
    // ... other end-block logic
    k.UpdateBaseFeePerGas(ctx)
}
```

After each block:
- If gas usage > target, base fee increases (up to max cap)
- If gas usage < target, base fee decreases (down to min floor)

Fee bounds are configurable via module params. The new base fee applies to the next block.

## Pointer Contracts

Pointer contracts enable tokens on one VM to be accessed from the other. This is the primary interoperability mechanism between CosmWasm and EVM.

### EVM Pointers for CW/Native Tokens

- **Native Cosmos denoms** → ERC20 pointer
- **CW20 tokens** → ERC20 pointer
- **CW721 NFTs** → ERC721 pointer
- **CW1155 multi-tokens** → ERC1155 pointer

### CW Wrappers for ERC Tokens

- **ERC20 tokens** → CW20 wrapper
- **ERC721 NFTs** → CW721 wrapper
- **ERC1155 multi-tokens** → CW1155 wrapper

Pointers are created via the `pointer` precompile (address `0x100b`, see [Precompiles](precompiles.md)). Precompiled bytecode for pointer contracts is embedded in the binary under `artifacts/`.

A reverse registry allows looking up the original token from its pointer address. Pointers are versioned and can be upgraded.

## Precompiled Contracts

The module supports custom precompiled contracts exposing Cosmos functionality to EVM callers. Precompiles are registered at fixed addresses (see [Precompiles](precompiles.md)) and are versioned by block height.

Payable precompiles suppress transfer events to avoid double-counting when value is forwarded.

## WASM Integration

CosmWasm contracts interact with the EVM through:

### Queries (Read-Only)

- Static EVM calls
- ERC20/721/1155 token queries
- Address association lookups

### Messages (State-Changing)

- `MsgInternalEVMCall` — regular EVM calls from CW contracts
- `MsgInternalEVMDelegateCall` — delegate calls (restricted to whitelisted pointer contracts)

These are implemented in `modules/evm/types/` and processed by the EVM keeper.

## ABCI Lifecycle

### BeginBlock

```go
func (k Keeper) BeginBlock(ctx sdk.Context) {
    k.txResults = nil
    k.msgs = nil
}
```

Resets per-block transaction tracking.

### EndBlock

```go
func (k Keeper) EndBlock(ctx sdk.Context) {
    k.CleanTxHashIndex(ctx)
    k.MigrateLegacyReceipts(ctx)
    k.PruneZeroValueStorageSlots(ctx)
    k.UpdateBaseFeePerGas(ctx)
    k.CollectCoinbaseSurplus(ctx)
    k.ProcessDeferredInfo(ctx)
}
```

1. Cleans old transaction hash indices
2. Migrates legacy receipts (one-time migration)
3. Prunes zero-value storage slots (resumable, batched)
4. Adjusts dynamic base fee for next block
5. Sweeps coinbase balances to fee collector
6. Aggregates deferred info (surplus, receipts, blooms)

## Configuration

Module params (`modules/evm/types/params.go`):

- `MinimumFeePerGas` — floor for base fee
- `BaseFeePerGas` — current base fee (updated each block)
- `PriorityNormalizer` — scales priority fee for mempool ordering
- `BaseFeeChangeDenominator` — EIP-1559 adjustment rate
- `ElasticityMultiplier` — gas target vs limit ratio

Configuration is stored in module state and governable via governance proposals.

## Observability

Metrics exposed via Prometheus:

- Transaction throughput (EVM vs total)
- Gas used per block
- Base fee evolution
- Receipt storage latency
- Pointer contract invocations

Events emitted to event bus:

- `EventTypeEthereumTx` — EVM transaction executed
- `EventTypeEVMCallFailed` — execution reverted
- `EventTypeAddressAssociation` — new address link
