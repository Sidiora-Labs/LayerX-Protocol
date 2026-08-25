# Modules

Paxeer-specific chain modules live under `paxeer-network/modules/`. These extend the Cosmos SDK framework with Paxeer-native functionality. Framework-provided modules remain under `sdk/x/`; interchain applications live under `interchain/modules/`.

## EVM Module

The `evm` module integrates the Ethereum Virtual Machine. See the dedicated [EVM](evm.md) page for complete documentation.

Summary:
- Native EVM execution on chain ID 125
- Address association between Pax and EVM
- Receipts and logs
- Pointer contracts for CW↔EVM interoperability
- EIP-1559 dynamic fees

## Epoch Module

**Path:** `modules/epoch/`

The epoch module manages time-based epochs that trigger periodic chain actions. Epochs are defined by a fixed duration and advance automatically when the block time exceeds the current epoch's start time plus duration.

### Lifecycle

Epochs progress in `BeginBlock` (`modules/epoch/keeper/abci.go`):

```go
func (k Keeper) BeginBlock(ctx sdk.Context) {
    lastEpoch := k.GetEpoch(ctx)
    if ctx.BlockTime().Sub(lastEpoch.CurrentEpochStartTime) > lastEpoch.EpochDuration {
        k.AfterEpochEnd(ctx, lastEpoch)
        
        newEpoch := types.Epoch{
            GenesisTime:           lastEpoch.GenesisTime,
            EpochDuration:         lastEpoch.EpochDuration,
            CurrentEpoch:          lastEpoch.CurrentEpoch + 1,
            CurrentEpochStartTime: ctx.BlockTime(),
            CurrentEpochHeight:    ctx.BlockHeight(),
        }
        k.SetEpoch(ctx, newEpoch)
        k.BeforeEpochStart(ctx, newEpoch)
    }
}
```

When an epoch boundary is crossed:
1. `AfterEpochEnd` hooks fire (previous epoch cleanup)
2. Epoch counter increments
3. `BeforeEpochStart` hooks fire (new epoch initialization)
4. `EventTypeNewEpoch` event is emitted

### Hooks

Other modules register epoch hooks to run logic at epoch boundaries. Hooks are defined in `modules/epoch/types/hooks.go`:

```go
type EpochHooks interface {
    BeforeEpochStart(ctx sdk.Context, epoch Epoch)
    AfterEpochEnd(ctx sdk.Context, epoch Epoch)
}
```

The mint module uses `BeforeEpochStart` to issue inflation rewards. The oracle module uses `AfterEpochEnd` to tally exchange rate votes.

### State

The epoch keeper stores:

```go
type Epoch struct {
    GenesisTime           time.Time
    EpochDuration         time.Duration
    CurrentEpoch          uint64
    CurrentEpochStartTime time.Time
    CurrentEpochHeight    int64
}
```

Epochs are persisted in `modules/epoch/keeper/epoch.go` and queryable via gRPC.

### Messages and Queries

The module does not expose user-facing messages (epochs advance automatically). Available queries:

- `Epoch` — returns current epoch number, start time, and height
- `Params` — returns epoch duration

### Configuration

Epoch duration is set at genesis and governable via params. Typical production epochs are 1 day.

## Mint Module

**Path:** `modules/mint/`

The mint module issues new native tokens according to an inflation schedule. It runs during epoch transitions to mint and distribute rewards.

### ABCI

Minting occurs in `BeginBlock` via the epoch hook:

```go
func (k Keeper) BeforeEpochStart(ctx sdk.Context, epoch epochtypes.Epoch) {
    // Calculate inflation amount based on params
    mintedCoin := k.CalculateInflation(ctx, epoch)
    
    // Mint to module account
    k.MintCoins(ctx, mintedCoin)
    
    // Distribute to staking rewards pool
    k.DistributeInflation(ctx, mintedCoin)
}
```

The minted tokens flow to the distribution module, which allocates them to validators and delegators as staking rewards.

### Inflation Schedule

Inflation parameters define:

- `InflationRate` — annual inflation percentage
- `MintDenom` — the native token denom (e.g., `uhpx`)
- `BlocksPerYear` — expected blocks per year for rate calculation

Inflation can be constant or dynamic based on staking ratio (bonded vs total supply). The schedule is implemented in the keeper and configurable via governance.

### State

The mint module maintains:
- Current inflation rate
- Annual provisions (expected yearly mint amount)
- Last mint block

These are queryable via gRPC.

### Messages and Queries

No user-facing messages (minting is automatic). Queries:

- `InflationRate` — current annual inflation percentage
- `AnnualProvisions` — expected yearly mint amount
- `Params` — mint parameters

## Oracle Module

**Path:** `modules/oracle/`

The oracle module aggregates exchange rate data from validators via a vote-reveal scheme. It provides price feeds for other modules (e.g., for fee conversion or stablecoin operations).

### Architecture

The oracle keeper (`modules/oracle/keeper/keeper.go`) maintains:

```go
type Keeper struct {
    cdc                         codec.BinaryCodec
    storeKey                    sdk.StoreKey
    accountKeeper               types.AccountKeeper
    bankKeeper                  types.BankKeeper
    distrKeeper                 types.DistributionKeeper
    StakingKeeper               types.StakingKeeper
    spamPreventionCounterMtxMap *datastructures.TypedSyncMap[string, *sync.Mutex]
    distrName                   string
}
```

### Exchange Rate Voting

Validators submit exchange rates via a commit-reveal scheme:

1. **Prevote** — validator submits hash of their exchange rate (concealed)
2. **Vote** — validator reveals actual exchange rate
3. **Tally** — at epoch end, median is computed from all reveals weighted by voting power

Exchange rates are stored in `modules/oracle/keeper/keeper.go`:

```go
type OracleExchangeRate struct {
    ExchangeRate         sdk.Dec
    LastUpdate           sdk.Int   // block height of last update
    LastUpdateTimestamp  int64     // unix timestamp of last update
}

func (k Keeper) SetBaseExchangeRate(ctx sdk.Context, denom string, exchangeRate sdk.Dec)
func (k Keeper) GetBaseExchangeRate(ctx sdk.Context, denom string) (sdk.Dec, sdk.Int, int64, error)
```

### Rewards and Penalties

Validators who participate in voting earn oracle rewards (distributed from a reward pool). Validators who miss votes or submit outliers are penalized via slashing.

Participation is tracked per validator and used to compute reward allocation.

### TWAP (Time-Weighted Average Price)

The oracle maintains TWAP windows for smoothing price volatility. TWAP data is computed by averaging exchange rates over a sliding window.

### State

The oracle keeper stores:
- Exchange rates per denom
- Validator prevotes and votes (ephemeral, cleared each epoch)
- Miss counters per validator
- TWAP history

### Messages

- `MsgAggregateExchangeRatePrevote` — commit hash of exchange rates
- `MsgAggregateExchangeRateVote` — reveal actual exchange rates
- `MsgDelegateFeedConsent` — delegate voting rights to a feeder address

### Queries

- `ExchangeRates` — all current exchange rates
- `OracleTwaps` — TWAP data for specified denoms and lookback
- `VotePenaltyCounter` — miss count for a validator

### Configuration

Oracle params:
- `VotePeriod` — blocks per vote window
- `VoteThreshold` — minimum voting power required for valid rate
- `RewardBand` — acceptable deviation from median (outside = penalty)
- `SlashFraction` — penalty for missing votes
- `MinValidPerWindow` — minimum valid votes in slash window

## TokenFactory Module

**Path:** `modules/tokenfactory/`

The tokenfactory module allows permissioned creation and management of native token denominations. Tokens created via tokenfactory are indistinguishable from genesis denoms and fully integrated with the bank module.

### Architecture

The keeper (`modules/tokenfactory/keeper/keeper.go`) manages:

```go
type Keeper struct {
    storeKey      sdk.StoreKey
    paramSpace    paramtypes.Subspace
    accountKeeper types.AccountKeeper
    bankKeeper    types.BankKeeper
    distrKeeper   types.DistrKeeper
}
```

### Denom Creation

Users create denoms via `MsgCreateDenom`:

```go
// modules/tokenfactory/keeper/createdenom.go
func (k Keeper) CreateDenom(ctx sdk.Context, creatorAddr string, subdenom string) (string, error) {
    denom := fmt.Sprintf("factory/%s/%s", creatorAddr, subdenom)
    // ... store denom metadata
    return denom, nil
}
```

The resulting denom format is `factory/{creator_address}/{subdenom}`. The creator becomes the denom admin.

### Admin Actions

The denom admin can:

- **Mint** — create new tokens of the denom
- **Burn** — destroy tokens from their own balance
- **ChangeAdmin** — transfer admin rights to another address
- **SetDenomMetadata** — update display name, symbol, decimals

Admin actions are implemented in `modules/tokenfactory/keeper/bankactions.go`:

```go
func (k Keeper) MintTo(ctx sdk.Context, admin string, denom string, amount sdk.Coin, recipient string) error
func (k Keeper) BurnFrom(ctx sdk.Context, admin string, denom string, amount sdk.Coin, sender string) error
```

Minting/burning is executed via the bank module after admin verification.

### Allow Lists (Optional)

Denoms can optionally restrict minting/burning to an allow list of addresses:

```go
func (k Keeper) SetDenomAllowList(ctx sdk.Context, denom string, allowList []string) error
```

When an allow list is set, only addresses on the list can mint or burn that denom.

### State

The keeper stores:
- Creator → denoms mapping (`GetCreatorPrefixStore`)
- Denom → admin mapping
- Denom → allow list mapping
- List of all creators (`GetCreatorsPrefixStore`)

### Messages

- `MsgCreateDenom` — create a new tokenfactory denom
- `MsgMint` — mint tokens (admin only)
- `MsgBurn` — burn tokens from sender
- `MsgChangeAdmin` — transfer admin rights
- `MsgSetDenomMetadata` — update metadata
- `MsgUpdateAllowList` — modify allow list

### Queries

- `DenomAuthorityMetadata` — admin and creation metadata for a denom
- `DenomsFromCreator` — all denoms created by an address
- `Params` — tokenfactory parameters

### Configuration

Tokenfactory params:
- `DenomCreationFee` — cost to create a new denom (paid to community pool)
- `DenomAllowListMaxSize` — maximum allow list size

### Use Cases

Tokenfactory enables:
- **Stablecoins** — admin mints/burns to maintain peg
- **Wrapped assets** — bridged tokens from other chains
- **Protocol tokens** — governance or utility tokens for dApps
- **Gaming tokens** — in-game currencies

## Store Module

**Path:** `modules/store/`

The store module is a thin helper providing module-level store integration. It does not implement business logic but offers utilities for accessing stores from other modules.

This module is primarily internal scaffolding and not directly exposed to users or external developers.
