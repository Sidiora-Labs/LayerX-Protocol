# Consensus

Paxeer's consensus layer implements a Byzantine Fault Tolerant state machine based on Tendermint BFT. The implementation lives in `paxeer-network/consensus/` and coordinates block proposal, voting, and finality across the validator set.

## Architecture

The consensus engine operates as a state machine progressing through distinct round steps for each block height. The core state machine is implemented in `consensus/internal/consensus/state.go`, which handles:

- Proposal reception and validation
- Prevote and precommit voting rounds
- Block locking and commit finalization
- Vote aggregation and supermajority detection
- Timeout management for each consensus step

### Round Steps

Each consensus round progresses through these steps (`consensus/internal/consensus/types/round_state.go`):

1. **NewHeight** — height increment, validator set loaded
2. **NewRound** — round counter incremented, proposer selected
3. **Propose** — designated proposer broadcasts block proposal
4. **Prevote** — validators cast prevote for proposed block or nil
5. **PrevoteWait** — wait for +2/3 prevotes
6. **Precommit** — validators lock on block with polka (supermajority prevotes) and precommit
7. **PrecommitWait** — wait for +2/3 precommits
8. **Commit** — block executed and finalized

The round step type is defined as:

```go
type RoundStepType uint8

const (
    RoundStepNewHeight     RoundStepType = 0x01
    RoundStepNewRound      RoundStepType = 0x02
    RoundStepPropose       RoundStepType = 0x03
    RoundStepPrevote       RoundStepType = 0x04
    RoundStepPrevoteWait   RoundStepType = 0x05
    RoundStepPrecommit     RoundStepType = 0x06
    RoundStepPrecommitWait RoundStepType = 0x07
    RoundStepCommit        RoundStepType = 0x08
)
```

### Voting and Locking

When +2/3 of voting power prevotes for a block, the consensus forms a polka (proof of lock). Validators then lock on that block and issue precommits. The locking mechanism (`enterPrecommit` in `consensus/internal/consensus/state.go`) ensures:

- Once locked, validators will only prevote/precommit for the locked block or nil
- A lock can be updated only with a polka from a later round
- This prevents safety violations while allowing liveness through round progression

The precommit logic validates:

```go
// From consensus/internal/consensus/state.go
if cs.roundState.ProposalBlock().HashesTo(blockID.Hash) {
    if err := cs.blockExec.ValidateBlock(ctx, cs.state, cs.roundState.ProposalBlock()); err != nil {
        panic(fmt.Sprintf("precommit step: +2/3 prevoted for an invalid block %v", err))
    }
    cs.roundState.SetLockedRound(round)
    cs.roundState.SetLockedBlock(cs.roundState.ProposalBlock())
    cs.signAddVote(ctx, tmproto.PrecommitType, blockID.Hash, blockID.PartSetHeader)
}
```

### Block Execution and Commit

When +2/3 precommits arrive for a block, the consensus engine calls `finalizeCommit` which:

1. Executes the block via ABCI `FinalizeBlock` (app state transition)
2. Persists the block to `store.BlockStore` (`consensus/internal/store/store.go`)
3. Updates the validator set for the next height
4. Advances to `NewHeight` step

The block store (`consensus/internal/store/store.go`) provides:

- `SaveBlock` — writes block and commit to disk
- `LoadBlock` — retrieves block by height
- Pruning of old blocks based on retention config

## ABCI Integration

The Application Blockchain Interface connects consensus to the application layer. Paxeer's ABCI types are defined in `consensus/abci/types/` and include:

- `CheckTx` — mempool admission (gas validation, signature verification)
- `FinalizeBlock` — execute all transactions in a committed block
- `Commit` — finalize state and return app hash

The `consensus/internal/proxy/` package wraps the ABCI client and provides:

```go
// consensus/internal/proxy/app_conn.go
type AppConnConsensus interface {
    FinalizeBlock(context.Context, *abci.FinalizeBlockRequest) (*abci.FinalizeBlockResponse, error)
    Commit(context.Context, *abci.CommitRequest) (*abci.CommitResponse, error)
}
```

State synchronization between consensus and application state is managed by `consensus/internal/state/` which persists validator sets, consensus parameters, and the last committed height.

## Node Initialization

Consensus nodes are constructed in `consensus/node/node.go`. The `makeNode` function initializes:

- Block and state databases (`initDBs`)
- Genesis document loading and validation
- Event bus for consensus events
- Mempool for transaction ordering
- Evidence pool for Byzantine fault detection
- P2P router for validator communication
- RPC endpoints

A node's consensus policy (defined in `consensus/types/consensus_policy.go`) determines whether it participates in proposals, voting, and commit.

## Mempool

The mempool (`consensus/internal/mempool/`) orders transactions before they enter a block. Transactions are:

1. Received via `CheckTx` ABCI call
2. Validated for signatures, nonces, gas limits
3. Stored in a priority queue (currently FIFO)
4. Broadcast to peers via `mempool/reactor/`
5. Removed after inclusion in a committed block

Mempool configuration (`consensus/config/config.go`) controls max transaction size, cache size, and broadcast behavior.

## Light Client Verification

Light clients can verify block headers without full block data by checking validator signatures on commits. The light client protocol is implemented in `consensus/light/` and verifies:

- Validator set continuity (unbonding period constraints)
- 2/3+ voting power signed the commit
- Block header hash matches the commit

Light clients track validator set changes and can detect forks through evidence of conflicting commits.

## Failure Handling

The consensus engine handles failures through:

- **Timeouts** — each step has a timeout triggering progression to next step
- **Evidence** — conflicting votes are reported to the evidence pool (`consensus/internal/evidence/`)
- **Peer gossip** — missing proposals or votes are requested from peers
- **Round advancement** — if proposal fails, advance to next round with new proposer

Byzantine validators submitting conflicting votes are detected via `ReportConflictingVotes` and subject to slashing.

## Configuration

Consensus behavior is configured in `consensus/config/config.go`:

- `TimeoutPropose`, `TimeoutPrevote`, `TimeoutPrecommit` — step timeouts
- `SkipTimeoutCommit` — skip commit timeout for faster blocks
- `CreateEmptyBlocks` — produce blocks even without transactions
- `PeerGossipSleepDuration` — P2P broadcast throttling

The config is loaded from `config.toml` and can be overridden per-node.

## Observability

Consensus metrics are exposed via Prometheus (`consensus/internal/consensus/metrics.go`):

- Block height and round progression
- Vote collection latency
- Proposal broadcast time
- Peer connectivity status
- Evidence pool size

Events are published to the event bus (`consensus/internal/eventbus/`) for external subscribers.
