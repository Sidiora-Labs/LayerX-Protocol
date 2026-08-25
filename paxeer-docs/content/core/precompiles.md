# Precompiles

Paxeer extends Ethereum's precompiled contracts with custom contracts that expose Cosmos SDK functionality to EVM callers. These are registered at fixed addresses and callable from any EVM transaction or contract.

Precompile addresses and registration are defined in `paxeer-network/precompiles/setup.go`. All precompiles are versioned by upgrade name to support historical queries and tracing.

## Precompile Addresses

All Paxeer precompiles are registered in the reserved address space below `0x100`:

| Precompile | Address | Description |
|------------|---------|-------------|
| [Bank](#bank) | `0x0000000000000000000000000000000000001001` | Native token transfers and queries |
| [Wasmd](#wasmd) | `0x0000000000000000000000000000000000001002` | CosmWasm contract instantiation and execution |
| [JSON](#json) | `0x0000000000000000000000000000000000001003` | JSON parsing utilities |
| [Addr](#addr) | `0x0000000000000000000000000000000000001004` | Address association and lookup |
| [Staking](#staking) | `0x0000000000000000000000000000000000001005` | Validator delegation and staking operations |
| [Gov](#gov) | `0x0000000000000000000000000000000000001006` | Governance proposal submission and voting |
| [Distribution](#distribution) | `0x0000000000000000000000000000000000001007` | Staking rewards withdrawal |
| [Oracle](#oracle) | `0x0000000000000000000000000000000000001008` | Exchange rate queries (retired) |
| [IBC](#ibc) | `0x0000000000000000000000000000000000001009` | IBC token transfers |
| [PointerView](#pointerview) | `0x000000000000000000000000000000000000100A` | Query pointer contract addresses (read-only) |
| [Pointer](#pointer) | `0x000000000000000000000000000000000000100B` | Create pointer contracts for CW/EVM interop |
| [Solo](#solo) | `0x000000000000000000000000000000000000100C` | Solo machine integration (permissioned) |
| [P256](#p256) | `0x0000000000000000000000000000000000001011` | P256 signature verification |

Precompiles are initialized in `setup.go`:

```go
func GetCustomPrecompiles(
    latestUpgrade string,
    keepers utils.Keepers,
) map[ecommon.Address]utils.VersionedPrecompiles {
    return map[ecommon.Address]utils.VersionedPrecompiles{
        ecommon.HexToAddress(bank.BankAddress):               bank.GetVersioned(...),
        ecommon.HexToAddress(wasmd.WasmdAddress):             wasmd.GetVersioned(...),
        // ... all precompiles registered
    }
}
```

## Bank

**Address:** `0x0000000000000000000000000000000000001001`  
**Source:** `precompiles/bank/bank.go`

The bank precompile exposes Cosmos SDK bank module functionality for native token operations.

### Methods

- **`send(string toAddress, string denom, uint256 amount)`** — send native tokens to a Pax address
- **`sendNative(address recipient, string denom, uint256 amount)`** — send native tokens to an EVM address
- **`balance(address account, string denom) returns (uint256)`** — query balance of a denom
- **`all_balances(address account) returns (Coin[])`** — query all balances for an account
- **`name(string denom) returns (string)`** — query denom display name
- **`symbol(string denom) returns (string)`** — query denom symbol
- **`decimals(string denom) returns (uint8)`** — query denom decimals
- **`supply(string denom) returns (uint256)`** — query total supply of a denom

Tokens are specified using native Cosmos denoms (e.g., `uhpx`, `factory/pax1.../subdenom`). Amounts are 18-decimal wei values, converted internally to 6-decimal µhpx.

## Wasmd

**Address:** `0x0000000000000000000000000000000000001002`  
**Source:** `precompiles/wasmd/wasmd.go`

The wasmd precompile enables EVM contracts to instantiate and execute CosmWasm contracts.

### Methods

- **`instantiate(uint64 codeID, string admin, bytes msg, string label) returns (string)`** — instantiate a CW contract, returns contract address
- **`execute(string contractAddress, bytes msg) payable`** — execute a CW contract with optional native token transfer
- **`execute_batch(string[] contractAddresses, bytes[] msgs, Coin[][] coins)`** — batch execute multiple CW contracts
- **`query(string contractAddress, bytes request) returns (bytes)`** — static query to CW contract

The `msg` and `request` parameters are JSON-encoded CosmWasm messages. The precompile handles conversion between EVM and Cosmos types.

### Payable Behavior

The `execute` method is payable. Value sent to the contract is converted to native Cosmos coins and passed to the CW contract as funds.

## JSON

**Address:** `0x0000000000000000000000000000000000001003`  
**Source:** `precompiles/json/json.go`

The JSON precompile provides utilities for parsing JSON data within EVM contracts. Useful for working with CosmWasm query responses.

### Methods

- **`extractAsBytes(bytes json, string key) returns (bytes)`** — extract a field as bytes
- **`extractAsBytesList(bytes json, string key) returns (bytes[])`** — extract an array field
- **`extractAsUint256(bytes json, string key) returns (uint256)`** — extract a numeric field
- **`extractAsBytesFromArray(bytes json, string key, uint256 index) returns (bytes)`** — extract from array by index

Gas cost scales with input size (100 gas per byte).

## Addr

**Address:** `0x0000000000000000000000000000000000001004`  
**Source:** `precompiles/addr/addr.go`

The addr precompile manages bidirectional address association between Pax (bech32) and EVM (hex) addresses.

### Methods

- **`getPaxAddr(address evmAddr) returns (string)`** — get Pax address for an EVM address
- **`getEvmAddr(string paxAddr) returns (address)`** — get EVM address for a Pax address
- **`associate(string paxAddr, bytes pubkey)`** — associate addresses using a public key
- **`associatePubKey(string paxAddr, bytes pubkey)`** — alternative association method

Association is typically automatic when signing transactions, but these methods allow explicit linking.

## Staking

**Address:** `0x0000000000000000000000000000000000001005`  
**Source:** `precompiles/staking/staking.go`

The staking precompile exposes validator delegation, undelegation, and redelegation functionality to EVM contracts.

### Methods

#### State-Changing

- **`delegate(string validator, uint256 amount)`** — delegate tokens to a validator
- **`redelegate(string srcValidator, string dstValidator, uint256 amount)`** — redelegate between validators
- **`undelegate(string validator, uint256 amount)`** — begin unbonding tokens from a validator
- **`createValidator(ValidatorParams params)`** — create a new validator (requires validator pubkey)
- **`editValidator(ValidatorParams params)`** — update validator metadata

#### Queries

- **`delegation(address delegator, string validator) returns (uint256)`** — query delegation amount
- **`validators() returns (Validator[])`** — list all validators
- **`validator(string valAddr) returns (Validator)`** — get validator details
- **`validatorDelegations(string valAddr) returns (Delegation[])`** — all delegations to a validator
- **`validatorUnbondingDelegations(string valAddr) returns (UnbondingDelegation[])`** — unbonding delegations for validator
- **`unbondingDelegation(address delegator, string validator) returns (UnbondingDelegation)`** — query unbonding progress
- **`delegatorDelegations(address delegator) returns (Delegation[])`** — all delegations by a delegator
- **`delegatorValidator(address delegator, string validator) returns (Validator)`** — validator delegated to
- **`delegatorUnbondingDelegations(address delegator) returns (UnbondingDelegation[])`** — all unbonding for delegator
- **`redelegations(address delegator, string srcVal, string dstVal) returns (Redelegation[])`** — query redelegations
- **`delegatorValidators(address delegator) returns (Validator[])`** — validators a delegator has staked with
- **`historicalInfo(uint64 height) returns (HistoricalInfo)`** — historical validator set at height
- **`pool() returns (uint256 bonded, uint256 notBonded)`** — total staked vs unbonded tokens
- **`params() returns (StakingParams)`** — staking module parameters

Validator addresses are bech32-encoded (e.g., `paxvaloper1...`). Amounts are in wei (18 decimals), converted to µhpx internally.

## Gov

**Address:** `0x0000000000000000000000000000000000001006`  
**Source:** `precompiles/gov/gov.go`

The gov precompile allows EVM contracts to submit governance proposals and vote.

### Methods

- **`submitProposal(bytes content, string proposalType) returns (uint64)`** — submit a governance proposal, returns proposal ID
- **`vote(uint64 proposalId, uint32 option)`** — vote on a proposal (Yes=1, No=3, NoWithVeto=4, Abstain=2)
- **`deposit(uint64 proposalId, uint256 amount)`** — deposit tokens to a proposal
- **`proposal(uint64 proposalId) returns (Proposal)`** — query proposal details
- **`proposals(uint32 status) returns (Proposal[])`** — list proposals by status

The `content` parameter is JSON-encoded proposal content. Voting options are integers matching Cosmos SDK vote options.

## Distribution

**Address:** `0x0000000000000000000000000000000000001007`  
**Source:** `precompiles/distribution/distribution.go`

The distribution precompile handles staking rewards withdrawal.

### Methods

- **`setWithdrawAddress(address withdrawAddr)`** — set address to receive rewards
- **`withdrawDelegationRewards(string validator)`** — withdraw rewards from one validator
- **`withdrawMultipleDelegationRewards(string[] validators)`** — withdraw from multiple validators in one tx
- **`withdrawValidatorCommission(string validator)`** — withdraw validator commission (validator only)
- **`rewards(address delegator, string validator) returns (Coin[])`** — query pending rewards

Rewards accumulate per block and can be withdrawn at any time. Multiple withdrawals can be batched for gas efficiency.

## Oracle

**Address:** `0x0000000000000000000000000000000000001008`  
**Source:** `precompiles/oracle/oracle.go`

**Status:** Retired. Oracle data queries are disabled.

The oracle precompile previously provided exchange rate data from validator voting. It now returns an error (`ErrOraclePrecompileRetired`) on all calls.

Historical methods (now disabled):

- `getExchangeRates() returns (ExchangeRate[])`
- `getOracleTwaps(string[] denoms, uint64 lookback) returns (OracleTwap[])`

## IBC

**Address:** `0x0000000000000000000000000000000000001009`  
**Source:** `precompiles/ibc/ibc.go`

The IBC precompile enables cross-chain token transfers via IBC.

### Methods

- **`transfer(string sourcePort, string sourceChannel, string denom, uint256 amount, address sender, string receiver, string timeoutHeight, uint64 timeoutTimestamp)`** — IBC transfer with explicit timeout
- **`transferWithDefaultTimeout(string sourcePort, string sourceChannel, string denom, uint256 amount, address sender, string receiver)`** — IBC transfer with default 10-minute timeout

Transfers send tokens to another IBC-connected chain. The `receiver` is a bech32 address on the destination chain.

Typical usage:

```solidity
ibc.transferWithDefaultTimeout(
    "transfer",           // source port
    "channel-0",          // source channel
    "uhpx",               // denom
    1000000000000000000,  // amount (1 HPX in wei)
    msg.sender,           // sender
    "osmo1..."            // receiver on destination
);
```

## PointerView

**Address:** `0x000000000000000000000000000000000000100A`  
**Source:** `precompiles/pointerview/pointerview.go`

The pointerview precompile queries existing pointer contract addresses (read-only). It does NOT create pointers; use the `pointer` precompile for creation.

### Methods

- **`getNativePointer(string denom) returns (address)`** — get ERC20 pointer for a native denom
- **`getCW20Pointer(string cwAddr) returns (address)`** — get ERC20 pointer for a CW20 token
- **`getCW721Pointer(string cwAddr) returns (address)`** — get ERC721 pointer for a CW721 NFT
- **`getCW1155Pointer(string cwAddr) returns (address)`** — get ERC1155 pointer for a CW1155 token

Returns `address(0)` if no pointer exists.

## Pointer

**Address:** `0x000000000000000000000000000000000000100B`  
**Source:** `precompiles/pointer/pointer.go`

The pointer precompile creates EVM pointer contracts for CosmWasm and native tokens, enabling cross-VM token access.

### Methods

- **`addNativePointer(string denom) returns (address)`** — create ERC20 pointer for a native denom
- **`addCW20Pointer(string cwAddr) returns (address)`** — create ERC20 pointer for a CW20 token
- **`addCW721Pointer(string cwAddr) returns (address)`** — create ERC721 pointer for a CW721 NFT
- **`addCW1155Pointer(string cwAddr) returns (address)`** — create ERC1155 pointer for a CW1155 token

Pointer creation is idempotent; calling again returns the existing pointer address. Pointers are deployed as minimal proxy contracts with embedded logic.

See [EVM Module - Pointer Contracts](evm.md#pointer-contracts) for architectural details.

## Solo

**Address:** `0x000000000000000000000000000000000000100C`  
**Source:** `precompiles/solo/solo.go`

The solo precompile integrates with solo machine workflows for permissioned claim operations.

### Methods

- **`claim()`** — claim assets from solo machine
- **`claimSpecific(address token, uint256 amount)`** — claim specific token and amount

This precompile is restricted to authorized addresses and used for internal infrastructure. General users should not call it directly.

## P256

**Address:** `0x0000000000000000000000000000000000001011`  
**Source:** `precompiles/p256/p256.go`

The P256 precompile verifies P256 (secp256r1) ECDSA signatures. This is the signature scheme used by Apple Secure Enclave and many hardware security modules.

### Methods

- **`verify(bytes32 hash, bytes signature, bytes publicKey) returns (bool)`** — verify a P256 signature

Gas cost is 300 per byte of input. This enables passkey-based authentication and hardware wallet integration.

## Versioning

Precompiles are versioned per upgrade to support historical queries and tracing. The `GetVersioned` function in each precompile package returns a map of upgrade names to precompile implementations.

Versioning is managed in `precompiles/setup.go`:

```go
var PrecompileLastUpgrade = map[string]int64{
    bank.BankAddress: 1,
    // ...
}
```

When querying historical state or replaying transactions, the correct precompile version is loaded based on the block height's upgrade.

## ABI Files

All precompiles embed their ABI JSON in the binary via `go:embed abi.json`. ABIs are accessible via:

```go
precompiles.GetPrecompileInfo("bank").ABI
```

This allows external tools to generate contract interfaces without distributing separate ABI files.
