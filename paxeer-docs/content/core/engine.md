# Engine

The Paxeer engine executes transactions within blocks, bridging consensus output to EVM state transitions. The executor package (`paxeer-network/engine/executor/`) provides the core transaction execution interface.

## Executor

The executor wraps a go-ethereum EVM instance and manages transaction execution lifecycle. It is defined in `engine/executor/executor.go`:

```go
type Executor struct {
    evm *vm.EVM
}
```

### Executor Variants

Paxeer supports two execution backends:

**1. Evmone Executor** — uses the evmone C++ interpreter via EVMC bindings for performance:

```go
func NewEvmoneExecutor(
    evmoneVM *evmc.VM,
    blockCtx vm.BlockContext,
    stateDB vm.StateDB,
    chainConfig *params.ChainConfig,
    config vm.Config,
    customPrecompiles map[common.Address]vm.PrecompiledContract,
) *Executor
```

The evmone path pre-computes host context configuration from chain config to avoid per-SSTORE overhead. The evmone VM is wrapped in `internal/HostContext` which adapts EVMC callbacks to geth's StateDB interface.

**2. Geth Executor** — pure Go implementation using go-ethereum's interpreter:

```go
func NewGethExecutor(
    blockCtx vm.BlockContext,
    stateDB vm.StateDB,
    chainConfig *params.ChainConfig,
    config vm.Config,
    customPrecompiles map[common.Address]vm.PrecompiledContract,
) *Executor
```

Both executors expose the same execution interface, allowing runtime selection based on build configuration.

## Transaction Execution

The executor provides two execution modes:

### Standard Execution

`ExecuteTransaction` performs full gas metering including fee deduction:

```go
func (e *Executor) ExecuteTransaction(
    tx *types.Transaction,
    sender common.Address,
    baseFee *big.Int,
    gasPool *core.GasPool,
) (*core.ExecutionResult, error)
```

This path:
1. Converts the Ethereum transaction to a geth `Message`
2. Applies gas fee purchase from sender balance
3. Executes the EVM state transition
4. Refunds unused gas to sender
5. Returns execution result with gas used, logs, and return data

### Fee-Charged Execution

`ExecuteTransactionFeeCharged` executes transactions where fees have been charged separately (e.g., via Cosmos ante handler):

```go
func (e *Executor) ExecuteTransactionFeeCharged(
    tx *types.Transaction,
    sender common.Address,
    baseFee *big.Int,
    gasPool *core.GasPool,
) (*core.ExecutionResult, error)
```

This matches the behavior of the EVM module's msg_server path where the ante handler charges fees before execution. The executor:
- Sets `feeAlreadyCharged=true` to skip gas purchase/refund
- Sets `shouldIncrementNonce=true` to increment nonce during execution
- Calls `core.NewStateTransition(...).Execute()` directly

This is defined in `engine/executor/executor.go`:

```go
e.evm.SetTxContext(core.NewEVMTxContext(message))
return core.NewStateTransition(e.evm, message, gasPool, true, true).Execute()
```

## StateDB Bridge

The executor operates on a `vm.StateDB` interface implemented by the EVM module's state package (`modules/evm/state/`). This bridges Cosmos SDK key-value stores to Ethereum's account-based model.

The StateDB implementation (`modules/evm/state/statedb.go`) provides:

- Balance management with 6-decimal µhpx to 18-decimal wei conversion
- Contract code and storage access
- Nonce tracking
- Snapshot/revert for failed transactions
- Journal for tracking all state mutations

All state changes are isolated within a `CacheMultiStore` during execution and only committed if the transaction succeeds.

## Block Context

The executor requires a `vm.BlockContext` providing block-level parameters:

```go
type BlockContext struct {
    Coinbase     common.Address   // fee collector for this tx
    GasLimit     uint64            // block gas limit
    BlockNumber  *big.Int          // current height
    Time         uint64            // block timestamp
    Difficulty   *big.Int          // always 0 (no PoW)
    BaseFee      *big.Int          // EIP-1559 base fee
    Random       common.Hash       // PREVRANDAO (from consensus)
}
```

The block context is constructed by the EVM keeper before execution using consensus block metadata.

## Gas Metering

Gas conversion between Cosmos SDK (integer gas units) and EVM (uint64 gas units) is handled by the EVM keeper. The conversion factor is configurable but typically 1:1.

The executor receives a `core.GasPool` limiting total gas available for the transaction:

```go
gasPool := new(core.GasPool).AddGas(gasLimit)
result, err := executor.ExecuteTransaction(tx, sender, baseFee, gasPool)
```

After execution, the remaining gas in the pool indicates unused gas.

## Custom Precompiles

Paxeer extends Ethereum's precompile set with custom contracts exposing Cosmos functionality. These are passed to the executor during construction:

```go
customPrecompiles := map[common.Address]vm.PrecompiledContract{
    common.HexToAddress("0x0000000000000000000000000000000000001001"): bankPrecompile,
    common.HexToAddress("0x0000000000000000000000000000000000001002"): stakingPrecompile,
    // ...
}
```

The executor registers these in the EVM config, making them callable at their designated addresses.

## Execution Isolation

Each transaction executes in isolation with its own:

- Gas meter (from the gas pool)
- Snapshot of state (revertible)
- Log accumulator (cleared on revert)
- Transient storage (EIP-1153, cleared between transactions)
- Access list (EIP-2930, reset per transaction)

Failed transactions revert all state changes but still consume gas and increment the sender's nonce (if the transaction passed ante validation).

## Testing and Replay

The engine includes test harnesses in `engine/tests/`:

- `state_test.go` — Ethereum state test JSON compatibility
- `giga_test.go` — large-scale stress tests
- `harness/` — test builder for constructing block execution scenarios

The EVM keeper also supports Ethereum replay mode for validating historical Ethereum blocks against the executor, useful for verifying compatibility.

## Parallelization Hooks

The `engine/` package defines interfaces for future optimistic concurrency control (OCC) and parallel execution. Actual parallelization implementation lives in `paxeer-network/parallelization/` but is not yet enabled in production.

Planned parallel execution will use:
- Read/write set tracking during execution
- Conflict detection across concurrent transactions
- Deterministic re-execution on conflicts

Current executor API is designed to be compatible with parallel execution once enabled.

## Dependencies

The engine depends on:

- **xevm** (`engine/deps/xevm/`) — EVM keeper types and state access
- **xbank** (`engine/deps/xbank/`) — deferred bank operations for gas refunds
- **testutil** (`engine/deps/testutil/`) — test fixtures and block processing utilities

These dependencies are isolated under `engine/deps/` to maintain clear boundaries between executor and higher-level chain logic.
