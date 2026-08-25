# Storage

Paxeer's storage layer is split into multiple subsystems optimized for different access patterns. The architecture is defined under `paxeer-network/storage/` and implements the PaxDB design, which separates active state from historical data.

## Architecture Overview

PaxDB divides storage into three layers:

1. **State Commitment (SC)** — in-memory Merkle tree for active state and proofs
2. **State Store (SS)** — versioned key-value store for historical queries
3. **Ledger DB** — block, transaction, and receipt storage

This separation is based on the [Cosmos StoreV2 ADR](https://docs.cosmos.network/main/build/architecture/adr-065-store-v2), which recognizes that a single database cannot efficiently serve both consensus proofs and historical queries.

### Key Benefits

From `storage/README.md`:

- Reduces active chain state size by 60%
- Reduces historical data growth rate by ~90%
- Improves state sync times by 1200% and block sync by 2x
- 287x improvement in block commit times
- 2x overall TPS improvement
- Archive nodes achieve the same performance as full nodes

### Trade-offs

- No historical proofs for all historical blocks (only recent, unpruned heights)
- Historical data lacks integrity validation (must trust the source)

## State Commitment (SC) Layer

**Path:** `storage/state_db/sc/`

The SC layer provides:

- Root app hash for each block
- Data access for transaction execution
- State import/export for state sync
- Historical proofs for unpruned heights

### MemIAVL Implementation

PaxDB forks [MemIAVL](https://github.com/crypto-org-chain/cronos/tree/main/memiavl) as its SC implementation. MemIAVL uses the same Merkelized AVL tree structure as Cosmos SDK but represents it in memory-mapped flat files instead of persisting nodes as key-value pairs in a database.

This design:

- **Avoids database overhead** — no encoding/decoding, no read amplification from node traversal
- **Memory-mapped I/O** — the OS manages page cache, reducing application memory pressure
- **Fast commits** — writing a new snapshot file is sequential I/O
- **Incremental snapshots** — only changed nodes are written

The SC tree is rebuilt from disk on node restart using the memory-mapped files.

### Pruning

SC stores are pruned to a configurable retention window (e.g., keep last 1000 blocks). Beyond that, only the SS layer retains historical data, and proofs are unavailable.

Pruning configuration is in `storage/config/sc_config.go`.

## State Store (SS) Layer

**Path:** `storage/state_db/ss/`

The SS layer stores versioned raw key-value pairs for historical queries. It does NOT store Merkle proofs or intermediate tree nodes, only the application-level key-value data.

### Responsibilities

- Fast versioned queries (key at height H)
- Versioned iteration over key ranges
- Batching for bulk writes
- Pruning of old versions

### DB Backend

PaxDB uses **PebbleDB** as the recommended backend for SS. Extensive benchmarking (documented in `storage/README.md`) showed PebbleDB outperforms LevelDB, RocksDB, and SQLite for Paxeer's workload:

- Random writes
- Random reads
- Forward/backward iteration

The SS implementation wraps PebbleDB with versioning logic, encoding each key as `{prefix}{version}{original_key}`.

### Write Modes

SS supports multiple write modes (`storage/config/write_mode.go`):

- **Synchronous** — block until write is flushed to disk
- **Asynchronous** — return after write is in OS buffer
- **Batch** — accumulate writes and flush in larger batches

Mode selection balances durability vs throughput. Validators typically use synchronous mode; archive nodes use async for higher throughput.

### Configuration

SS config is in `storage/config/ss_config.go`:

- Database path
- Cache size
- Pruning strategy (keep last N versions)
- Compaction settings

## Ledger DB

**Path:** `storage/ledger_db/`

The ledger DB stores blockchain-level data outside the application state tree. It is organized into subdirectories:

### Block Store

**Path:** `storage/ledger_db/block/`

Stores full block data (headers, transactions, evidence, commits). Implemented in `block_db.go`:

```go
type BlockDB interface {
    SaveBlock(height int64, block *types.Block, commit *types.Commit) error
    LoadBlock(height int64) (*types.Block, error)
    LoadBlockMeta(height int64) (*types.BlockMeta, error)
    DeleteBlock(height int64) error
}
```

An in-memory implementation (`mem_block_db/`) is available for testing. Production uses on-disk storage.

Pruning is independent of SC/SS pruning and configured separately.

### Transaction Store

**Path:** `storage/ledger_db/transaction/`

Currently a placeholder. Transaction indexing is handled by the consensus layer's transaction indexer (`consensus/internal/state/indexer/`).

### Receipt Store

**Path:** `storage/ledger_db/receipt/`

Stores EVM transaction receipts. Implemented in `receipt_store.go`:

```go
type ReceiptStore interface {
    SaveReceipt(ctx sdk.Context, hash common.Hash, receipt *ethtypes.Receipt) error
    GetReceipt(ctx sdk.Context, hash common.Hash) (*ethtypes.Receipt, error)
    GetReceiptByIndex(ctx sdk.Context, height uint64, idx uint64) (*ethtypes.Receipt, common.Hash, error)
}
```

Receipts are indexed by:

- **Transaction hash** — primary lookup
- **Block height + index** — for block-level queries

A transaction hash index (`tx_hash_index.go`) maps EVM tx hashes to block height and index. This is a write-through cache with on-disk backing.

Receipts are cached in memory (`receipt_cache.go`) with LRU eviction to avoid repeated disk reads.

### Event Store

**Path:** `storage/ledger_db/event/`

Currently a placeholder. Event indexing is handled by the consensus event sinks (`consensus/internal/state/indexer/sink/`).

## WAL (Write-Ahead Log)

**Path:** `storage/wal/`

The WAL records state changes before they are committed to the SC/SS layers. This enables:

- **Crash recovery** — replay uncommitted changes after node restart
- **Atomic commits** — ensure all layers are consistent
- **Debugging** — inspect the sequence of state changes

WAL entries are written sequentially and flushed to disk before acknowledgment. After a successful commit, WAL entries are garbage-collected.

WAL configuration includes:
- File rotation size
- Retention policy
- Sync mode (fsync vs OS buffer)

## DB Engine

**Path:** `storage/db_engine/`

The db_engine package provides a unified interface for database backends:

```go
type DB interface {
    Get(key []byte) ([]byte, error)
    Set(key []byte, value []byte) error
    Delete(key []byte) error
    Iterator(start, end []byte) Iterator
    ReverseIterator(start, end []byte) Iterator
    NewBatch() Batch
    Close() error
}
```

This abstraction allows swapping between PebbleDB, LevelDB, RocksDB, or other backends without changing application code.

The engine implements:

- **Batch writes** — accumulate multiple writes and commit atomically
- **Iterators** — range queries with forward/reverse traversal
- **Prefix scans** — iterate all keys with a common prefix

Backend selection is configured at node startup.

## State Reconstruction

A node can reconstruct its state from:

1. **Genesis file** — initial state at height 0
2. **Block replay** — apply all blocks sequentially
3. **State sync** — download a recent snapshot and sync forward

State sync uses the SC layer's export/import functionality to transfer the active state tree. After import, the node syncs blocks from the snapshot height to the chain tip.

## Persistence Guarantees

Different layers have different durability:

| Layer      | Durable After        | Loss Impact                |
|------------|----------------------|----------------------------|
| SC         | Commit + sync        | Lose uncommitted state     |
| SS         | Async flush (config) | Lose recent historical data|
| Ledger DB  | Block write          | Lose recent blocks         |
| WAL        | Fsync                | Lose uncommitted writes    |

Validators use synchronous writes for all layers to prevent any data loss. Full nodes and archive nodes may use async writes for higher throughput at the cost of potential data loss on crash.

## Configuration Files

Storage configuration is loaded from `config.toml` and split into:

- **SC config** (`storage/config/sc_config.go`) — pruning, cache, snapshot intervals
- **SS config** (`storage/config/ss_config.go`) — backend, write mode, compaction
- **Receipt config** (`storage/config/receipt_config.go`) — cache size, indexing

Per-module configuration allows independent tuning of each layer.

## Observability

Storage metrics exposed via Prometheus:

- SC commit latency and size
- SS write/read latency
- Ledger DB block save time
- Receipt store cache hit rate
- Database compaction activity
- Disk usage per layer

Logs include:

- Pruning operations (which heights were pruned)
- Snapshot creation/restoration
- Database compaction runs
- WAL replay on startup

## Tools

The `storage/tools/` directory provides utilities:

- **paxdb** (`storage/tools/cmd/paxdb/`) — CLI for inspecting and manipulating PaxDB databases
  - Dump SC/SS state
  - Benchmark read/write performance
  - Import/export between formats
  - Compute state size
  - Replay changelogs
- **rpc_bench** (`storage/tools/rpc_bench/`) — benchmarks RPC query performance
- **cryptosim** (`storage/state_db/bench/cryptosim/`) — synthetic workload generator for storage testing

These tools are used for debugging, performance tuning, and migration testing.
