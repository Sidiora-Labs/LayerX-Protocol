# WASM (CosmWasm)

Paxeer integrates [CosmWasm](https://cosmwasm.com/), a smart contract platform for the Cosmos ecosystem. CosmWasm contracts run in a sandboxed WebAssembly (WASM) VM and can interact with the Cosmos SDK and EVM through custom bindings.

The WASM implementation is split across three directories:

- `paxeer-network/wasm/` — CosmWasm app module and Cosmos SDK integration
- `paxeer-network/wasm-runtime/` — WASM VM runtime (libwasmvm wrapper)
- `paxeer-network/wasmbinding/` — Paxeer-specific query and message bindings

## Architecture

CosmWasm contracts execute in the Wasmer runtime, which compiles WASM bytecode to native machine code. The runtime is embedded in the node via CGO bindings to the Rust `libwasmvm` library.

The CosmWasm `x/wasm` module (`wasm/x/wasm/`) integrates into the Cosmos SDK app as a standard module providing:

- Contract upload (store compiled WASM)
- Contract instantiation (create contract instances)
- Contract execution (call contract entry points)
- Contract queries (read contract state)
- Contract migration (upgrade contract code)

## App Module

**Path:** `wasm/x/wasm/`

The WASM app module is defined in `wasm/x/wasm/module.go`:

```go
type AppModule struct {
    keeper Keeper
    // ...
}

func (am AppModule) RegisterServices(cfg module.Configurator) {
    types.RegisterMsgServer(cfg.MsgServer(), keeper.NewMsgServerImpl(am.keeper))
    types.RegisterQueryServer(cfg.QueryServer(), keeper.Querier{Keeper: am.keeper})
}
```

The module exposes:

- **Msg server** — state-changing operations (upload, instantiate, execute, migrate)
- **Query server** — read-only contract queries and metadata
- **Genesis** — export/import contract state
- **ABCI hooks** — BeginBlock/EndBlock for contract lifecycle

## Keeper

**Path:** `wasm/x/wasm/keeper/keeper.go`

The WASM keeper manages contract lifecycle and state:

```go
type Keeper struct {
    storeKey          sdk.StoreKey
    cdc               codec.Codec
    wasmVM            types.WasmerEngine  // WASM runtime
    accountKeeper     types.AccountKeeper
    bankKeeper        types.BankKeeper
    // ...
}
```

### Key Operations

**Upload:**

```go
func (k Keeper) Create(ctx sdk.Context, creator sdk.AccAddress, wasmCode []byte) (codeID uint64, err error)
```

Stores compiled WASM bytecode and assigns a code ID. The code is validated and instantiated once; multiple contracts can be instantiated from the same code.

**Instantiate:**

```go
func (k Keeper) Instantiate(
    ctx sdk.Context,
    codeID uint64,
    creator, admin sdk.AccAddress,
    initMsg []byte,
    label string,
    deposit sdk.Coins,
) (sdk.AccAddress, []byte, error)
```

Creates a contract instance with:

- Unique contract address (derived from code ID and instance count)
- Optional admin (can migrate the contract)
- Initialization message (JSON-encoded)
- Label for identification

**Execute:**

```go
func (k Keeper) Execute(
    ctx sdk.Context,
    contractAddress sdk.AccAddress,
    caller sdk.AccAddress,
    msg []byte,
    coins sdk.Coins,
) (*sdk.Result, error)
```

Calls a contract's `execute` entry point with funds. The contract can:

- Mutate its state
- Send messages to other modules
- Query other contracts or modules
- Return events and data

**Query:**

```go
func (k Keeper) QuerySmart(
    ctx sdk.Context,
    contractAddress sdk.AccAddress,
    req []byte,
) ([]byte, error)
```

Calls a contract's `query` entry point (read-only). Queries cannot modify state or consume significant gas.

## WASM Runtime

**Path:** `wasm-runtime/`

The WASM runtime wraps libwasmvm, the Rust library that executes WASM bytecode. Paxeer supports two build modes:

### CGO Mode (Default)

Links against the C-exported libwasmvm library:

```go
// #cgo LDFLAGS: -lwasmvm
import "C"

func (vm *VM) Instantiate(...) { /* CGO calls */ }
```

This mode requires:

- Rust toolchain at build time
- `libwasmvm.so` (Linux) or `libwasmvm.dylib` (macOS) in library path

CGO mode provides the best performance (Wasmer JIT compilation).

### No-CGO Mode

Uses a pure Go WASM interpreter (slower but avoids CGO):

```go
// +build !cgo

func (vm *VM) Instantiate(...) { /* Go interpreter */ }
```

Enabled with `CGO_ENABLED=0` or when `libwasmvm` is unavailable. Useful for cross-compilation or environments without Rust.

### Gas Metering

The runtime injects gas metering into WASM bytecode. Every instruction consumes gas; when the gas limit is reached, execution panics with `OutOfGasError`.

Gas costs are defined in `wasm/x/wasm/keeper/gas_register.go`:

- Memory allocation
- CPU cycles (per opcode)
- Storage reads/writes
- External calls

## Wasmbinding

**Path:** `wasmbinding/`

Wasmbinding provides Paxeer-specific queries and messages for CosmWasm contracts. This is how CW contracts access Paxeer modules like EVM, oracle, epoch, and tokenfactory.

### Custom Queries

**Path:** `wasmbinding/query_plugin.go`

The query plugin handles read-only queries from CW contracts to Paxeer modules:

```go
type QueryPlugin struct {
    oracleHandler       oraclewasm.OracleWasmQueryHandler
    epochHandler        epochwasm.EpochWasmQueryHandler
    tokenfactoryHandler tokenfactorywasm.TokenFactoryWasmQueryHandler
    evmHandler          evmwasm.EVMQueryHandler
    stakingKeeper       stakingkeeper.Keeper
}
```

Contracts send queries via the `PaxQuery` custom query type defined in `wasmbinding/bindings/`:

```rust
// From CosmWasm contract
let response: ExchangeRatesResponse = deps.querier.query(&QueryRequest::Custom(
    PaxQuery::ExchangeRates {}
))?;
```

The query plugin (`HandleOracleQuery`, `HandleEpochQuery`, etc.) unmarshals the query, calls the appropriate keeper, and returns JSON-encoded results.

#### Supported Queries

**Oracle** (`wasmbinding/query_plugin.go`):

- `ExchangeRates` — all current exchange rates
- `OracleTwaps` — time-weighted average prices

**Epoch**:

- `Epoch` — current epoch number and timing

**TokenFactory**:

- `DenomAuthorityMetadata` — admin and creator of a tokenfactory denom
- `DenomsFromCreator` — all denoms created by an address

**EVM**:

- Static EVM calls
- ERC20/721/1155 token queries
- Address association lookups
- Pointer contract addresses

### Custom Messages

**Path:** `wasmbinding/message_plugin.go`

The message plugin handles state-changing operations from CW contracts:

```go
type MessagePlugin struct {
    wrapped msg.Handler
}

func (mp MessagePlugin) DispatchMsg(ctx sdk.Context, contractAddr sdk.AccAddress, contractIBCPortID string, msg wasmvmtypes.CosmosMsg) ([]sdk.Event, [][]byte, error)
```

Contracts send messages via the `PaxMsg` custom message type:

```rust
// From CosmWasm contract
let msg = CosmosMsg::Custom(PaxMsg::CallEvm {
    to: evm_contract_address,
    data: encoded_call_data,
    value: Uint128::zero(),
});
```

The message plugin routes to the appropriate module (EVM, tokenfactory, oracle, etc.).

#### Supported Messages

**EVM**:

- `CallEvm` — call an EVM contract from CW
- `DelegateCallEvm` — delegate call (restricted to pointers)

**TokenFactory**:

- `CreateDenom`
- `MintTokens`
- `BurnTokens`
- `ChangeAdmin`

**Oracle** (if enabled):

- `AggregateExchangeRatePrevote`
- `AggregateExchangeRateVote`

### Bindings

**Path:** `wasmbinding/bindings/`

Type definitions for Paxeer-specific queries and messages are in `wasmbinding/bindings/`:

- `msg.go` — message types (`PaxMsg`)
- `queries.go` — query types (`PaxQuery`)
- `errors.go` — error types

These are imported by CosmWasm contracts to interact with Paxeer:

```rust
use paxeer_bindings::{PaxQuery, PaxMsg, ExchangeRatesResponse};
```

## Contract-to-EVM Interaction

CosmWasm contracts can call EVM contracts via the `evmHandler` in wasmbinding. This enables:

- CW contracts invoking ERC20 `transfer`, `approve`
- CW contracts calling custom EVM logic
- Pointer contracts forwarding operations between VMs

### Call Flow

1. CW contract emits `PaxMsg::CallEvm`
2. Message plugin receives the message
3. Keeper creates an internal EVM call
4. EVM executes the call with CW contract as caller
5. Return data is passed back to CW contract

EVM calls from CW are subject to:

- Gas limits (shared gas meter)
- Read-only enforcement (for queries)
- Reentrancy restrictions (no CW→EVM→CW→EVM)

## EVM-to-WASM Interaction

EVM contracts can call CosmWasm contracts via:

1. **Wasmd precompile** (`0x1002`) — direct instantiation and execution from EVM
2. **Pointer contracts** — EVM pointers for CW20/721/1155 tokens

### Call Flow

1. EVM contract calls wasmd precompile or pointer
2. Precompile converts EVM call to CW message
3. WASM keeper executes the CW contract
4. Return data is converted back to EVM format
5. EVM contract receives the result

EVM→CW calls support:

- Passing native tokens (EVM `value` → CW `funds`)
- JSON message encoding
- Query and execute entry points

## Gas Accounting

Gas is shared between Cosmos SDK, EVM, and WASM. The keeper tracks gas usage and converts between units:

- **Cosmos gas** — integer, consumed by SDK modules
- **EVM gas** — uint64, consumed by EVM execution
- **WASM gas** — uint64, consumed by WASM execution

Conversion factors are configurable. When a CW contract calls the EVM (or vice versa), gas is deducted from the shared pool.

## Contract Permissions

CosmWasm supports permission policies via the `authz_policy` mechanism (`wasm/x/wasm/keeper/authz_policy.go`). Contracts can be restricted by:

- Who can instantiate from a code ID
- Who can execute a contract
- Who can migrate a contract

Paxeer uses this to restrict sensitive operations (e.g., only whitelisted contracts can delegate-call the EVM).

## Contract Lifecycle

### Upload

Developer compiles Rust code to WASM, then uploads via:

```bash
paxd tx wasm store contract.wasm --from creator
```

The keeper validates the WASM (no floating point, no start function, valid exports) and stores it with a code ID.

### Instantiate

User instantiates a contract from code ID:

```bash
paxd tx wasm instantiate 1 '{"count": 0}' --from creator --label "counter"
```

The contract receives a unique address and runs its `instantiate` entry point.

### Execute

User sends a message to the contract:

```bash
paxd tx wasm execute pax14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9s4hmalr '{"increment":{}}' --from user
```

The contract's `execute` entry point runs and can mutate state.

### Migrate

Admin upgrades the contract to new code:

```bash
paxd tx wasm migrate pax14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9s4hmalr 2 '{}' --from admin
```

The new code's `migrate` entry point runs to upgrade state schema if needed.

## Events and Attributes

CosmWasm contracts emit events via:

```rust
let event = Event::new("action")
    .add_attribute("sender", info.sender)
    .add_attribute("amount", amount.to_string());
```

These are converted to Cosmos SDK events and indexed for queries. Contracts can also emit synthetic EVM logs (via pointer contracts) that appear in EVM receipts.

## Testing

**Path:** `wasm/x/wasm/keeper/`

The keeper includes test helpers (`test_common.go`) for:

- Mock WASM VM (no CGO required)
- Contract upload/instantiate in tests
- Simulated execution
- State inspection

Tests load example contracts from `wasm/x/wasm/keeper/testdata/`.

## Observability

WASM metrics exposed via Prometheus:

- Contract instantiations per code ID
- Execution gas usage
- Query latency
- Upload size

Events:

- `wasm-store_code` — code uploaded
- `wasm-instantiate` — contract instantiated
- `wasm` — contract execution (includes contract events)

## Limitations

- **No floating point** — WASM contracts cannot use `f32`/`f64` (disabled for determinism)
- **No dynamic linking** — contracts must be fully self-contained
- **Gas limits** — queries have low gas limits to prevent DoS
- **Reentrancy** — CW→EVM→CW call chains are restricted (max 1 hop with writes)

## Contract Security

Best practices for CosmWasm on Paxeer:

1. **Validate inputs** — contracts are public APIs, all inputs are untrusted
2. **Check funds** — ensure sent funds match expected amounts
3. **Avoid reentrancy** — do not call untrusted contracts during state updates
4. **Limit queries** — external queries can be expensive or manipulated
5. **Use pointer contracts** — for CW↔EVM interop, prefer established patterns

See [CosmWasm security documentation](https://docs.cosmwasm.com/docs/1.0/security/) for general guidance.
