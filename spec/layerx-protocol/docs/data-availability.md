# LayerX Data Availability

Normative specification of the LayerX data-availability (DA) layer: what must be
published alongside every batch, how possession of that data is committed to and
attested, how finalisation depends on it, how agents retrieve and independently
replay finalised history, and what the system does when data goes missing.

MUST, MUST NOT, SHOULD, SHOULD NOT and MAY are normative requirements on a
conforming C17 implementation. Companion documents, all under
`spec/layerx-protocol/docs/`: `wire-format.md` (canonical encoding),
`state-machine.md` (transition function), `checkpointing.md` (checkpoint
lifecycle), `guarantors.md` (quorum rules) and `economics.md` (bonds, slashing
amounts, reserve reconciliation).

## 1. Why a state root is not enough

A `resulting_state_root` in a `BatchHeader` is a commitment, not a witness. It
proves that *someone* claims a state exists; it proves nothing about whether
anyone can construct that state, prove a balance inside it, or show that the
transition into it was legal. If the sequencer publishes a root and withholds
the underlying batch:

1. **No agent can compute an exit proof.** Paxeer settlement verifies Merkle
   membership against a finalised state root. Without the state-diff material an
   agent cannot build the path for its own account, so custody assets become
   unreachable even though the checkpoint is "final".
2. **No replica can rebuild state.** Missing activity bytes are an unrecoverable
   gap: the append-only log is the authority and a hole in it cannot be filled
   from any rebuildable SQLite index.
3. **No fraud is detectable.** A guarantor cannot prove a root wrong without the
   inputs that produced it. Withheld data converts an economically-backed
   attestation into an unfalsifiable assertion.
4. **Receipts and oracle inputs become unverifiable.** A counterparty cannot
   check an inclusion proof against an activity set it cannot fetch, and every
   perps mark, funding accrual and liquidation is only auditable relative to the
   exact signed oracle payload that entered history.

Therefore **a checkpoint MUST NOT be finalisable on the strength of roots
alone**: finality is gated on attested possession of a defined availability set
by a bonded quorum. Threshold attestations are not a validity proof, and this
document specifies exactly what they do and do not buy.

## 2. The availability set

For every batch `B` the availability set `DA(B)` is exactly five sections, each
a byte string produced by the canonical codec. No section may be omitted,
reordered, or replaced by a derived summary.

| ID | Section | Content |
|----|---------|---------|
| 1 | `ACTIVITIES` | Every accepted activity envelope in `B`, in global-sequence order, canonically encoded, including `payload` and `signature` bytes verbatim as received |
| 2 | `RECEIPTS` | Every `ActivityReceipt` produced by `B`, in global-sequence order, canonically encoded, including `effects` and `fee_charged` |
| 3 | `ORACLE` | Every signed oracle activity consumed by `B`, plus the resolved oracle view: market symbol, price, `snapshot_id`, `stats_seq`, `source_timestamp_ms`, publisher identity and signature |
| 4 | `STATE_DIFF` | For every state key touched by `B`: the key, the pre-image value, the post-image value, and the module that wrote it — sorted by canonical key byte order |
| 5 | `RECOVERY` | Batch manifest, `previous_state_root`, `resulting_state_root`, parameter-set hash, account-sequence advances, subaccount creations and closures, idempotency-key insertions, withdrawal nullifier updates, and the snapshot descriptor if `B` closes a snapshot boundary |

Sections 1 and 2 are the *authority*. Sections 3, 4 and 5 are derivable from
section 1 plus the pre-state, and are published anyway because deriving them
requires already possessing the history a joining node does not have. Section 4
MUST carry both pre-image and post-image: post-image alone prevents checking the
transition, pre-image alone prevents incremental state sync. Sections 1, 4 and 5
together MUST suffice to reconstruct the state at `resulting_state_root` from
`previous_state_root` with no other node's help. Empty sections are legal and
MUST still appear in the manifest with `byte_length = 0` and the empty-tree root.

## 3. Chunking and section commitments

Each section is split into fixed-size chunks so possession can be probed and
challenged at fine granularity.

```c
#define LX_DA_CHUNK_SIZE      65536u   /* 64 KiB, final chunk may be shorter */
#define LX_DA_SECTION_COUNT   5u
#define LX_DA_PROBE_COUNT     16u

enum lx_da_section_id {
    LX_DA_SECTION_ACTIVITIES = 1,
    LX_DA_SECTION_RECEIPTS   = 2,
    LX_DA_SECTION_ORACLE     = 3,
    LX_DA_SECTION_STATE_DIFF = 4,
    LX_DA_SECTION_RECOVERY   = 5
};

struct lx_da_section_desc {
    uint8_t  section_id;
    uint8_t  reserved[3];
    uint32_t chunk_count;      /* ceil(byte_length / LX_DA_CHUNK_SIZE) */
    uint64_t byte_length;
    uint8_t  content_hash[32]; /* over the whole section, one shot */
    uint8_t  chunk_root[32];   /* Merkle root over chunk leaves */
};

struct lx_da_manifest {
    uint16_t protocol_version;
    uint16_t section_count;    /* MUST equal LX_DA_SECTION_COUNT */
    uint32_t network_id;
    uint64_t epoch;
    uint64_t batch_number;
    uint64_t first_sequence;
    uint64_t last_sequence;
    uint64_t total_bytes;
    struct lx_da_section_desc sections[LX_DA_SECTION_COUNT];
};
```

Commitment rules, all with domain-separated SHA-256:

```
chunk_leaf   = H("LXP:DA:CHUNK:v1"    || section_id || u32be(index) || chunk_bytes)
node         = H("LXP:DA:NODE:v1"     || left || right)
content_hash = H("LXP:DA:SECTION:v1"  || section_id || u64be(len) || section_bytes)
da_root      = H("LXP:DA:MANIFEST:v1" || canonical_encode(manifest))
```

The tree is binary and built bottom-up in index order. An odd node at any level
is **promoted unchanged**, never duplicated, and `chunk_count` is committed in
the descriptor, so tree shape is fixed and shape-collision attacks are
impossible. The empty section root is `H("LXP:DA:EMPTY:v1")`. `da_root` is the
value written into `BatchHeader.data_availability_root`, and it transitively
binds every byte of `DA(B)`. All commitments are over **uncompressed canonical
bytes**; compression at rest or on the wire MUST NOT change any hash input.

A batch header whose `data_availability_root` does not equal the `da_root`
recomputed from the published sections is invalid; a guarantor that attests to it
has attested to an unavailable set and is slashable on that evidence
(`docs/guarantors.md`, section 7).

## 4. Guarantor possession attestation

A guarantor signs only after it has, in this order, downloaded every section,
verified every activity signature, replayed every transition, recomputed all six
roots in the header, written the full `DA(B)` to durable local storage, and
fsynced it.

```c
struct lx_da_attestation {
    uint16_t protocol_version;
    uint16_t reserved;
    uint32_t network_id;
    uint64_t epoch;
    uint64_t batch_number;
    uint8_t  da_root[32];
    uint8_t  resulting_state_root[32];
    uint8_t  guarantor_id[32];
    uint64_t stored_bytes;         /* MUST equal manifest.total_bytes */
    uint64_t attested_at_ms;
    uint8_t  possession_digest[32];
    uint8_t  signature[65];        /* secp256k1 recoverable, Paxeer-facing */
};
```

The `possession_digest` is a self-probe that makes a blind signature useless:

```
seed  = H("LXP:DA:PROBE:v1" || da_root || guarantor_id || u64be(epoch))
idx_i = be64(H(seed || u32be(i))) mod manifest.total_chunk_count   /* i = 0..15 */
possession_digest = H("LXP:DA:POSSESS:v1" || da_root || guarantor_id ||
                      chunk_bytes(idx_0) || ... || chunk_bytes(idx_15))
```

Chunk indices address a flattened space over all sections in section-ID order;
duplicate draws are redrawn with an incremented counter. Because the seed binds
`da_root` and the guarantor's own identity, each guarantor probes a different,
unpredictable 1 MiB sample it cannot fabricate. The signature payload is
`H("LXP:DA:ATTEST:v1" || canonical_encode(attestation_without_signature))`, and
signing for data the guarantor later cannot serve is the offence section 7.2
punishes.

## 5. The finalisation gate

A checkpoint covering batches `[b_lo, b_hi]` is finalisable only when all of the
following hold, evaluated as a conjunction with no configurable bypass.

1. Every batch in the range has a published manifest whose recomputed `da_root`
   equals the header field.
2. For every batch, at least `T` distinct bonded guarantors have produced valid
   `lx_da_attestation` records over the same `(da_root, resulting_state_root)`
   pair. `T` is the guarantor threshold of `docs/guarantors.md`
   (`T >= floor(2N/3) + 1`, `T >= 3`, `N >= 4`; initially `N = 7`, `T = 5`), and
   `LX_DA_THRESHOLD` is an alias for it — there is no separate DA quorum.
3. No attesting guarantor is unbonding or slashed at the checkpoint height.
4. No guarantor has a conflicting attestation for the same
   `(epoch, batch_number)` with a different `da_root` or state root.
5. No DA challenge (section 7.2) against the range is unresolved.
6. The node's own reserve reconciliation for the range succeeds
   (`docs/economics.md`, section 10).

The Paxeer checkpoint contract verifies conditions 2, 3 and 4 from the guarantor
signature set it is given, and condition 5 from its own challenge registry. It
does not, and must not, understand section contents.

**A state root with fewer than `T` possession attestations is not
finalisable, however many guarantors signed the state root itself.** Availability
and validity ride on one signature precisely so they cannot be decoupled.

## 6. Retrieval interface

Any agent, replica or auditor retrieves `DA(B)` over the canonical binary
transport. Four message pairs are defined; the optional JSON/HTTP gateway MAY
mirror them but never defines behaviour.

| Message | Request fields | Response fields |
|---------|----------------|-----------------|
| `LX_MSG_DA_MANIFEST` | `epoch`, `batch_number` | `lx_da_manifest`, header signature |
| `LX_MSG_DA_CHUNK` | `batch_number`, `section_id`, `chunk_index` | chunk bytes, Merkle path to `chunk_root`, `chunk_count` |
| `LX_MSG_DA_SECTION` | `batch_number`, `section_id`, `offset`, `length` | byte range, `content_hash` |
| `LX_MSG_DA_RANGE` | `first_batch`, `last_batch`, section bitmap | streamed manifests and sections |

A chunk response carries the header fields, then the chunk bytes, then
`path_len = ceil(log2(chunk_count))` sibling digests of 32 bytes in leaf-to-root
order. Guarantors MUST serve any chunk inside their retention window (section 8);
failure to serve is evidence for a challenge, not a QoS event. Every response
MUST be verifiable standalone, so a client holding only `da_root` from Paxeer can
verify 64 KiB without downloading anything else. `LX_MSG_DA_RANGE` MAY be rate
limited per peer, but never for a request citing an open challenge identifier.
Error codes: `LX_DA_ERR_PRUNED`, `LX_DA_ERR_UNKNOWN_BATCH`,
`LX_DA_ERR_RATE_LIMITED`, `LX_DA_ERR_NOT_FINALISED`.

### Independent replay

`cmd/layerx-verify` implements the reference procedure. An agent trusting
nothing but Paxeer runs:

1. Read the finalised checkpoint certificate from Paxeer — `epoch`, batch range,
   `resulting_state_root`, per-batch `da_root`, guarantor signature set — and
   verify those signatures against the bonded set registered on Paxeer.
2. Fetch each manifest and all five sections; recompute `da_root`, every
   `content_hash` and every `chunk_root`; compare.
3. Verify every activity signature and authority in section 1.
4. Load the state at `previous_state_root` (verified snapshot, or replay from
   genesis) and apply the versioned transition function in sequence order.
5. Recompute `activity_merkle_root`, `receipt_merkle_root`, `event_merkle_root`,
   `oracle_root` and `resulting_state_root`; compare against the header; compare
   locally produced receipts byte-for-byte against section 2.

Exit status 0 means finalised history is reproducible bit-for-bit on the
verifier's own machine; any nonzero status names the first divergent
`(batch_number, global_sequence, field)` triple.

## 7. Sampling and challenge

### 7.1 Sampling

Every replica samples continuously in the background; it is cheap, and it is the
early-warning system feeding the challenge game. On each newly attested batch a
replica draws `LX_DA_SAMPLE_COUNT = 8` chunk indices from
`H("LXP:DA:SAMPLE:v1" || da_root || replica_id || nonce)` and requests them from
a randomly chosen attesting guarantor. A response that is missing, late
(`> LX_DA_SAMPLE_TIMEOUT_MS`, default 5000) or fails Merkle verification
increments that guarantor's fault counter; `LX_DA_SAMPLE_FAULTS_MAX = 3` faults
against distinct chunks of one batch escalates to a formal challenge.

Detection probability: withholding a fraction `f` of chunks survives `S`
independent samples with probability `(1 - f)^S`. With 8 samples per replica and
10 sampling replicas, withholding 5% of a batch is caught with probability
`1 - 0.95^80 = 98.4%`. Sampling supplements, and never replaces, the guarantor's
own full download.

### 7.2 Challenge

Sampling produces suspicion; the challenge game produces settlement. It runs on
Paxeer because that is where the bonds live.

1. **Open.** Any party posts `openDataChallenge(epoch, batch_number, section_id,
   chunk_index, guarantor_id)` with a bond of `LX_DA_CHALLENGE_BOND` (see
   `docs/economics.md`), recorded against the registered `da_root`.
2. **Respond.** The named guarantor has `LX_DA_CHALLENGE_WINDOW_SEC` (default
   3600) to submit the chunk bytes and Merkle path. The contract verifies the
   path against `chunk_root` and `chunk_root` against `da_root` from the
   manifest committed at checkpoint registration. It never parses the chunk.
3. **Resolve.** A valid response closes the challenge and forfeits the
   challenger's bond to the responder, making griefing costly. No response, or an
   invalid path, slashes the guarantor per the DA schedule, returns the
   challenger's bond plus its share of the slash, and marks the batch
   `DA_FAULTED`, which fails finalisation condition 5 for every checkpoint
   containing it. If unresolved batches push clean attestations below
   `T`, the chain enters `DA_STALL` (section 9).

Responses are cheap on Paxeer — one 64 KiB blob plus at most 17 hashes for an
8 GiB batch — so an honest guarantor always wins, and one that attested without
possessing cannot respond at any price.

## 8. Retention and pruning

| Section | Sequencer / archive | Guarantor | Replica |
|---------|---------------------|-----------|---------|
| `ACTIVITIES`, `RECEIPTS`, `ORACLE` | forever | finality + 90 days | finality + 7 days |
| `STATE_DIFF` | forever | finality + 30 days | finality + 2 days |
| `RECOVERY` | forever | finality + 90 days | until superseded by a verified snapshot + 2 epochs |

Retention is governance-settable within bounds declared in `docs/economics.md`.
The guarantor `ACTIVITIES` minimum MUST NOT fall below
`LX_DA_CHALLENGE_WINDOW_SEC * 24`, and no retention may be reduced retroactively
for already-attested batches.

A node MAY prune a `(batch, section)` only when **all** of the following hold:

1. The containing checkpoint is finalised on Paxeer.
2. The challenge window for that checkpoint has elapsed with no open challenge.
3. The section's retention period for this node's role has elapsed.
4. At least `LX_DA_ARCHIVE_MIN = 3` distinct archive nodes have published a
   possession attestation covering the batch, verifiable by the pruning node.
5. No dispute, emergency-exit claim or reserve-reconciliation halt references
   the batch range.

Pruning MUST be journalled — the node permanently records `(batch_number,
section_id, content_hash, pruned_at)` — so a later inability to serve is
attributable to a legal prune rather than withholding. A guarantor that prunes
in violation of these conditions and is then challenged is slashed exactly as if
it had withheld: the journal is not a defence.

## 9. When data becomes unavailable

The DA subsystem is a three-state machine, evaluated independently and
deterministically by every node from finalised evidence.

| State | Entry condition | Effect |
|-------|-----------------|--------|
| `DA_OK` | attestation quorum current, no faults | normal operation |
| `DA_DEGRADED` | any batch with attestations `>= threshold` but at least one sampling fault escalated, or clean attestations within 1 of threshold | new batches still sealed; checkpoint submission paused; alarms raised; sampling rate raised to `LX_DA_SAMPLE_COUNT * 4` |
| `DA_STALL` | any unfinalised batch below `T` clean attestations for `LX_DA_STALL_EPOCHS = 2`, or any `DA_FAULTED` batch inside the finalisation frontier | finalisation halts |

`DA_STALL` behaviour is mandatory and fail-closed. The sequencer MUST stop
sealing once the unfinalised backlog exceeds `LX_DA_MAX_UNFINALISED_BATCHES`
(default 256) rather than extend the chain on top of unavailable data;
guarantors MUST refuse to attest any batch built on a `DA_FAULTED` parent; no
checkpoint is submitted to Paxeer, so withdrawals sitting in
`system:paxeer-withdrawals` are not settled (settlement needs a finalised root)
while already-finalised deposits remain creditable once the stall clears; and
perps markets go `REDUCE_ONLY` at `DA_DEGRADED` and `PAUSED` at `DA_STALL`,
because liquidation correctness depends on oracle inputs whose availability is
in question.

**Emergency exit.** If `DA_STALL` persists for `LX_DA_EXIT_TRIGGER_EPOCHS`
(default 8), anyone may arm the Paxeer contract's emergency-exit mode once the
condition is provable from the contract's own challenge registry and checkpoint
timestamps. In that mode agents withdraw directly against the **last finalised**
state root by submitting a Merkle membership proof for `agent:<did>:main` and
each subaccount they own, plus a withdrawal nullifier. Those proofs are always
constructible: that root passed the finalisation gate, so `T` guarantors
attested possession, and archive nodes plus the exit-proof generator
in `cmd/layerx-verify` rebuild any account path from the retained `STATE_DIFF`
and `RECOVERY` sections. Escrow, budget, stream and margin subaccounts exit to
their owner of record; open perps positions settle at the last finalised mark;
the liquidity and insurance pools exit last and absorb any residual — a fixed
order, so exit is deterministic. The window is open-ended, and ordinary operation
cannot resume without a governance action that first restores DA for the stalled
range. That is the point of the design: withholding data cannot steal assets, it
can only stop the system, and stopping the system releases assets to their owners
against the last root everyone could verify.

## 10. Storage-cost analysis

Per-activity DA cost, canonical encodings with 32-byte identifiers:

| Section | Bytes per activity (typical) | Notes |
|---------|------------------------------|-------|
| `ACTIVITIES` | 300 | 172 envelope header + 64 signature + ~64 payload median |
| `RECEIPTS` | 220 | roots and balances are fixed-width |
| `STATE_DIFF` | 240 | ~3 touched keys at 80 bytes (key + pre + post) |
| `ORACLE` + `RECOVERY` | 40 | amortised oracle record, sequence advance, idempotency insert |
| **Total** | **800** | design budget; implementations SHOULD track the real figure |

At commodity prices of $0.02/GB-month hot and $0.004/GB-month archive:

| Load | Per day | Per year | Guarantor 90d set | Guarantor hot cost | Archive yr-1 avg | Ingress |
|------|---------|----------|-------------------|--------------------|------------------|---------|
| 10/s | 0.69 GB | 252 GB | 62 GB | $1.24/mo | $0.50/mo | 0.064 Mbit/s |
| 100/s | 6.91 GB | 2.52 TB | 622 GB | $12.44/mo | $5.04/mo | 0.64 Mbit/s |
| 1000/s | 69.1 GB | 25.2 TB | 6.22 TB | $124.42/mo | $50.46/mo | 6.4 Mbit/s |

DA storage is never the binding constraint on guarantor economics at v1 loads —
the bond dominates it by three orders of magnitude, which is deliberate: the
bond, not the disk, is what makes the attestation meaningful. Sequencer egress is
`N * 800 B` per activity, so at `N = 7` and 100 activities/s it is 4.5 Mbit/s
sustained and no erasure-coded sampling scheme is needed in v1; section 3's
chunking stays compatible with adding one later without changing `BatchHeader`.
`STATE_DIFF` is 30% of volume with the shortest retention, so storing it in a
separate per-batch file makes pruning a single `unlink` and holds the retained
corpus at ~70% of raw. zstd level 3 yields a further 2.5x to 3x.

## 11. Conformance requirements

| # | Test | Pass condition |
|---|------|----------------|
| 1 | Manifest determinism | x86-64 and AArch64 produce byte-identical sections and one `da_root` |
| 2 | Odd-node promotion | chunk counts 1, 2, 3, 5, 7, 1024 match published roots; no tree of `n` chunks collides with one of `m != n` |
| 3 | Possession probe | deleting one chunk makes `possession_digest` unproducible for every epoch that draws it |
| 4 | Gate refusal | `T - 1` attestations refused, with an error distinct from every other refusal cause |
| 5 | Challenge round trip | honest guarantor answers first, last and a random middle chunk of each section; a missing chunk fails and slashes exactly once |
| 6 | Prune safety | each of the five preconditions, violated alone, blocks the prune and names itself |
| 7 | Stall and exit | withholding drives `DA_OK -> DA_DEGRADED -> DA_STALL`, halts finalisation, and after `LX_DA_EXIT_TRIGGER_EPOCHS` permits exits totalling custody |
| 8 | Independent replay | `cmd/layerx-verify` reproduces a 10^6-activity history from DA alone on a machine that never ran the sequencer |

## 12. Implementation map

| Path | Responsibility |
|------|----------------|
| `include/layerx/lx_da.h` | manifest, section, attestation and message structs |
| `src/storage/da_store.c`, `da_merkle.c` | chunked section store with fsync discipline and prune journal; chunk leaves, promotion tree, path generation and verification |
| `src/sequencer/da_publish.c` | section assembly, manifest build, `data_availability_root` |
| `src/guarantor/da_attest.c`, `da_serve.c` | download, verify, store, probe, sign; retrieval handlers and rate limits |
| `src/replica/da_sample.c` | background sampling and fault escalation |
| `src/paxeer/da_challenge.c` | challenge submission and response construction |
| `src/protocol/da_gate.c` | finalisation gate, DA state machine, stall and exit arming |
| `cmd/layerx-verify/`, `tests/da/` | independent replay and exit-proof generation; commitment vectors and fault injection |
