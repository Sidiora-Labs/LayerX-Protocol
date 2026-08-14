# LayerX State Machine

Status: normative. Version: 1. Protocol tag: `LXP1`.

This document defines the state LayerX maintains, how it is committed, and the exact deterministic
procedure that turns one canonical activity into one receipt. Binding on the C17 reference implementation
in this repository; the kernel is `src/state/`, with public state interfaces in `include/layerx/lxp_state.h`.

Two rules govern everything below, because every other rule depends on them: **a LayerX node must produce
identical results from identical activity history**, on any machine, architecture and build, forever; and
**402LXP is the single financial doorway** — every monetary effect compiles into one or more
authenticated balance transfers executed by `lxp_apply_transfer_set`, and no module writes a balance
directly.

## 1. State trees

State is a fixed vector of sparse Merkle trees (SMTs), each with a stable `tree_id` that is part of every
key preimage, so a key in one tree can never be confused with a key in another.

| id | Tree | Key preimage | Record |
|---|---|---|---|
| `01` | identity | `did_ref` | primary key, key scheme, recovery record, rotation state, EVM payout binding |
| `02` | account | `account_id \|\| asset_id` | `u128 balance`, account kind, owner `did_ref` |
| `03` | sequence | `did_ref` | `u64 next_sequence` |
| `04` | idempotency | `H("LXP1/idempotency", did_ref \|\| idem_key)` | `activity_id`, `result_digest`, `expires_at_ms` |
| `05` | authority | `key_id` or `grant_id` | session-key scope; grant terms, remaining allowances, `revocation_epoch` |
| `06` | escrow | `escrow_id` | parties, subaccount, timeout, dispute state |
| `07` | budget | `budget_id` | period, per-period cap, consumed, subaccount |
| `08` | stream | `stream_id` | rate, last settled time, subaccount |
| `09` | service | `agreement_id` | offer, acceptance, commitments, tool attestations, deliveries, acceptances, disputes |
| `0A` | perps | `market_id` / `position_id` | market params, positions, funding accumulators, order book commitments |
| `0B` | oracle | `market_id \|\| signer_key_id` | last accepted observation, sequence, timestamp |
| `0C` | bridge | nullifier | deposit and withdrawal nullifiers, reserve mirror accounting |
| `0D` | governance | parameter name | protocol parameters, active version table, emergency flags |
| `0E` | asset | `asset_id` | symbol, decimals, custody binding, circulating mirror |
| `0F` | metering | `epoch \|\| resource_id` | resource accumulators, fee accounting |

Locked funds are **real accounts**, never hidden columns. Canonical labels (`agent:<did>:main`,
`agent:<did>:budget:<id>`, `agent:<did>:escrow:<id>`, `agent:<did>:margin:<position>`,
`system:liquidity:<market>`, `system:insurance`, `system:fees`, `system:paxeer-reserve`,
`system:paxeer-withdrawals`) exist for humans; consensus uses the 32-byte `account_id` from
`wire-format.md` §3. The append-only activity log is the authority and every tree above is a
**projection** rebuildable from genesis plus the log; SQLite indexes are rebuildable projections of those
projections, never consulted by the transition function.

### 1.1 SMT construction

Each tree is a binary SMT of depth 256 over `path = H("LXP1/state-key", u8(tree_id) || key_bytes)`,
traversed most-significant-bit first.

```
value_digest = H("LXP1/state-value", encoded_record)      /* canonical LXC/1 */
leaf         = H("LXP1/state-leaf",  path || value_digest)
node(l,r)    = H("LXP1/state-node",  l || r)
E[256]       = H("LXP1/merkle-empty", "")
E[d]         = node(E[d+1], E[d+1])                        /* precomputed once */
```

Absent keys hash to `E[d]`, so proofs are non-membership proofs for free; the `E[]` table is computed at
startup and MUST match `tests/vectors/state/empty_subtrees.hex`. Writes recompute only the dirty path
(≤ 256 node hashes), iteratively and never recursively — see §7.

### 1.2 Global state root

```
state_root = H("LXP1/state-root",
               u32(layout_version) || root[01] || root[02] || ... || root[0F])
```

Tree order is fixed by `tree_id` ascending and is part of the layout: adding a tree changes
`layout_version`, which changes every subsequent `state_root` by construction, so an upgrade can never be
mistaken for a continuation. Because `layout_version`, the governance parameters and the active
transition version table all live inside committed state, a replayer needs **only** the genesis manifest
and the log — no external configuration file may influence execution, and a node where one can is not
conformant.

### 1.3 Commitment points

- After **each activity**: dirty paths are recomputed and `state_root` materialized; the receipt carries
  `previous_state_root` and `resulting_state_root`, so every activity is individually provable.
- After **each batch**: the header's `resulting_state_root` MUST equal the last activity's, and its
  `previous_state_root` the first activity's. An empty batch has the two equal.
- After **each checkpoint**: guarantors recompute all of the above from batch bytes before attesting.

## 2. Execution context

The transition function reads no clock, environment, socket or file. Everything time-like or node-like
arrives in an immutable context supplied by the batch:

```c
struct lx_ctx {
    uint16_t protocol_version;   /* selects decoder + transition version   */
    uint32_t network_id;
    uint64_t epoch;
    uint64_t batch_number;
    uint64_t global_sequence;    /* of this activity within all history    */
    uint64_t timestamp_ms;       /* batch timestamp; the ONLY notion of now */
    uint8_t  sequencer_id[32];
    const struct lx_params *params;  /* snapshot of tree 0D at batch start */
};
```

`timestamp_ms` is deterministic input, not a reading: it MUST be monotonically non-decreasing across
batches and within `PARAM_MAX_CLOCK_DRIFT_MS` of the previous one, and a batch violating that is invalid.

## 3. Transition function

```c
typedef enum {
    LX_OK     = 0,  /* admitted, executed, effects committed              */
    LX_FAIL   = 1,  /* admitted, executed, effects rolled back, fee taken */
    LX_REJECT = 2,  /* not admissible: no log entry, no state change      */
    LX_FATAL  = 3   /* implementation fault: halt, never continue         */
} lx_class_t;

lx_class_t lx_apply_activity(struct lx_state      *state,
                             const struct lx_ctx  *ctx,
                             const uint8_t        *bytes,
                             size_t                len,
                             struct lx_journal    *journal,
                             struct lx_receipt    *out_receipt,
                             struct lx_event_buf  *out_events);

lx_class_t lx_apply_batch(struct lx_state          *state,
                          const struct lx_batch    *batch,
                          struct lx_batch_result   *out);
```

The 402LXP kernel exposes exactly two balance-mutating entry points, and they are the only functions in
the codebase permitted to write tree `02`:

```c
int lxp_apply_transfer(struct lxp_state *state,
                       const struct lxp_transfer *transfer,
                       struct lxp_receipt *receipt);

int lxp_apply_transfer_set(struct lxp_state *state,
                           const struct lxp_transfer_set *set,
                           struct lxp_receipt *receipt);
```

`lxp_apply_transfer` is a one-leg call into `lxp_apply_transfer_set`, so there is one code path in
practice. Modules receive a `struct lxp_transfer_set *` to fill and never a writable handle to tree `02`.
This is structural: the balance mutator is `static` inside `src/modules/asset/lxp_transfer.c`, reachable
only through those two symbols, and CI greps for any other unit touching the account tree.

Result classes:

- **`LX_REJECT`** — the activity never enters the log: no sequence consumed, no fee, no receipt. A batch
  containing one is an **invalid batch**; the sequencer should not have ordered it.
- **`LX_FAIL`** — the activity is real history. Sequence consumed, fee charged, module and transfer
  effects fully rolled back, receipt emitted with a non-zero `result_code`.
- **`LX_FATAL`** — an invariant the code believed impossible was violated (allocation failure, journal
  corruption, a root mismatching its recomputation). The node MUST stop: never skip the activity, never
  continue degraded, never return an error a caller could read as a transaction failure. Divergence is
  worse than downtime.

## 4. Evaluation order of a single activity

Phases execute in exactly this order; reordering any two changes observable behaviour, so the order is
normative, not advisory.

**P0 — Journal savepoint S0.** Push an undo savepoint before touching anything.

**P1 — Decode.** Run the LXC/1 decoder (`wire-format.md`): magic, version, structure, limits, minimal
varints, closed enums, map/set ordering, no trailing bytes, `payload_hash` equality. Any failure →
`LX_REJECT`, with nothing yet read from state.

**P2 — Context admission.** `protocol_version` accepted by the active version table, `network_id ==
ctx->network_id`, `not_before_ms <= ctx->timestamp_ms <= not_after_ms`, and span within
`PARAM_MAX_VALIDITY_MS`. Failure → `LX_REJECT`.

**P3 — Signature verification.** Reconstruct the `wire-format.md` §4.1 preimage and verify under
`sig_scheme`, with malleability checks. In production, worker threads do this *before* the activity
reaches the deterministic writer; the writer consumes a verified flag but MUST re-verify in replay, audit
and guarantor modes. Failure → `LX_REJECT`.

**P4 — Actor resolution.** Load the identity record for `actor_did` from tree `01`. Absent identity →
`LX_REJECT`: an unknown account has no sequence to consume and no balance to charge.

**P5 — Authority resolution.** Resolve `authority.kind`:

- `00` primary: the signature key must equal the identity's live primary key.
- `01` session key: load `key_id` from tree `05`; it must be bound to `actor_did`, unexpired at
  `ctx->timestamp_ms`, unrevoked, and its scope must admit this `activity_type` and this asset.
- `02` grant: load `grant_id` from tree `05`; the *state record* is authoritative, not the presented
  bytes. It must be live, in window, unrevoked (`revocation_epoch` unchanged), and the draw must fit
  `max_per_draw` and the remaining total/period allowance.
- `03` module capability: the capability must have been minted by a module in an earlier committed
  activity and must not be consumed.

Authorization is state, never HTTP middleware. Failure is `LX_REJECT` when the authority does not exist
or is not bound to the actor, and `LX_FAIL` when it exists and is bound but is exhausted, expired or out
of scope — the second case is the account's own spendable history and must be recorded and charged.

**P6 — Sequence consumption.** Read `next_sequence[actor_did]` from tree `03` and require
`account_sequence == next_sequence` exactly — not `>=`, not "within a window". Mismatch → `LX_REJECT`; on
match write `next_sequence + 1`. Every sequence is consumed exactly once, and **before** any effect can
fail, so no failure path leaves a gap or a reusable slot.

**P7 — Idempotency check.** If `idem_present`, look up `H("LXP1/idempotency", actor_did || idem_key)` in
tree `04`.

- Absent → insert `(activity_id, 0, ctx->timestamp_ms + PARAM_IDEM_RETENTION_MS)`; finalized in P12.
- Present with the same `activity_id` → impossible, since P6 already rejected the replay; `LX_FATAL`.
- Present with a different `activity_id` → `LX_FAIL` with `LX_ERR_IDEMPOTENCY_CONFLICT`: sequence stays
  consumed, fee is charged, **zero** module effects execute. One idempotency key therefore produces at
  most one economic result by construction, not by convention.

**P8 — Fee reservation.** Compute `fee_estimate` from metered resource units and tree `0D` parameters.
Require `fee_estimate <= fee_limit` (else `LX_FAIL`, `LX_ERR_FEE_LIMIT`) and `balance[actor_main,
fee_asset] >= fee_estimate` (else `LX_ERR_INSUFFICIENT_FEE`). Reserve by transferring `agent:<did>:main →
system:fees` through `lxp_apply_transfer_set`: the fee is not an exception to 402LXP.

**P9 — Journal savepoint S1.** Everything after this point is rollback-eligible; everything before it
(sequence, idempotency insert, fee) survives failure.

**P10 — Module execution.** Dispatch on `activity_type >> 8` to the module and on `activity_type & 0xFF`
to the action. The module:

- reads and writes only its own tree plus records it owns; performs integer-only arithmetic through
  checked helpers; emits events into `out_events`; returns a module result code;
- emits a `struct lxp_transfer_set` describing every monetary leg — it never writes a balance itself.

`service` actions — task commitment, tool execution attestation, delivery, acceptance, dispute
open/respond/resolve — are first-class ordered and attested activities that write only tree `09` and
produce an **empty** transfer set. Where they imply value movement (releasing escrow on acceptance,
refunding on a dispute resolution) that movement is a separate authorized activity, or a
module-capability-authorized transfer set that still executes in P11. A module that mutates a balance
outside P11 is a defect of the highest severity.

**P11 — Transfer application.** `lxp_apply_transfer_set` validates the whole set atomically before
writing anything:

- every leg has `amount > 0`, and `asset[from] == asset[to] == leg.asset`;
- the debited account is covered by the resolved authority from P5;
- `balance[from] >= amount` with debits applied in leg order, and no addition overflows `u128`;
- per asset, `Σ debits == Σ credits` across the entire set.

Deposits and withdrawals conserve because they move value between agent accounts and
`system:paxeer-reserve` / `system:paxeer-withdrawals`; ordinary modules never mint or burn. If any leg
violates any invariant, **no leg is written** and the phase fails.

**P12 — Outcome fixing.** On success compute `result_digest = H("LXP1/receipt", receipt core)` and
finalize the P7 idempotency record with it. On failure, roll back to S1 (§5), set `result_code`, and
finalize with the failure digest — a failure is still exactly one outcome for that key.

**P13 — Receipt and event emission.** Build the receipt:

```
activity_id, global_sequence, previous_state_root, resulting_state_root,
activity_root, result_code, effects, fee_charged, batch_id, sequencer_signature
```

plus, for 402LXP operations, `operation`, `asset`, `amount`, `from`, `from_balance_before`,
`from_balance_after`, `from_sequence`, `to`, `to_balance_before`, `to_balance_after`,
`transfer_set_root`, `authorization_hash`, `context_hash`. Before/after balances are **evidence read out
of state**, never client inputs. Events append in emission order; their Merkle root is order-dependent
and part of the batch header.

**P14 — Root update.** Recompute the dirty paths of every touched tree, then `state_root` per §1.2.
Write `resulting_state_root` into the receipt. Pop savepoint S0. The activity is now history.

Ordering consequence: a `LX_FAIL` activity still changes the state root, because P6 and P8 committed.
That is intended — failure is a real, paid-for, provable event, not a no-op.

## 5. Rollback semantics

State writes go through one accessor:

```c
int lx_state_put(struct lx_state *s, uint8_t tree_id, const uint8_t *key, size_t key_len,
                 const uint8_t *val, size_t val_len, struct lx_journal *j);
```

which appends an undo entry `{tree_id, key, prev_present, prev_value}` before mutating. Properties the
implementation MUST hold:

1. **Savepoints are a stack** with a compile-time bound (`LX_MAX_SAVEPOINTS 8`): S0 wraps the activity,
   S1 the effect region, and a module may push at most one more. Exceeding the bound is `LX_FATAL`.
2. **Rollback is exact.** Unwinding restores every value in reverse order, absence included — deleting a
   key that was absent restores absence, not an empty record, because those hash differently.
3. **Rollback is total or it is fatal.** A partially unwound journal is unrecoverable; the node halts.
4. **No observation of rolled-back state.** Roots are recomputed only in P14, after the outcome is
   fixed, so an intermediate root is never signed, stored or shown.
5. **A transfer set is all-or-nothing.** All legs validate before the first write; there is no
   apply-then-undo path, because undo-based atomicity would depend on undo being bug-free at exactly the
   moment the code is already in an unexpected state.
6. **Durability boundary.** Log record, journal and resulting root become durable as one unit. On
   restart a partially applied activity is replayed from the log, and recovery is idempotent because
   replay from `previous_state_root` is deterministic. Tested at every write boundary.

## 6. Versioning of transition functions

```c
struct lx_transition_entry {
    uint16_t   version;
    uint64_t   activation_epoch;
    lx_apply_fn fn;
};
```

Rules, all mandatory:

- The active table lives in tree `0D`, so it is committed in the state root: a node cannot be
  configured into a different rule set out of band.
- Replay dispatches on the version recorded for the batch's epoch, **never** on the newest version the
  binary implements. Historical batches re-execute under the code that first executed them, forever.
- A transition function is **immutable once activated**. Bugs are never fixed in place; a new version is
  added and activated at a future epoch, and old versions stay in the binary permanently — deleting one
  makes history unverifiable.
- Activation is a governance activity effective at an epoch boundary, so every node switches at the same
  deterministic point.
- Any change to encoding, hashing, tree layout, evaluation order, rounding, consensus-effective
  parameters or error classification is a version change. "It only affects an error path" is not an
  exemption: error classification moves the state root through fees and sequences.
- Every version boundary MUST be covered by a replay test that runs the same historical batches under
  old and new binaries and compares roots byte for byte.

## 7. Determinism obligations and the C hazards that violate them

The obligations — canonical binary encoding, integer-only arithmetic, explicit overflow behaviour,
stable ordering, fixed rounding, versioned transition functions, batch-supplied timestamps, no OS state,
no network I/O in execution — translate into specific C17 rules. Each hazard below has produced consensus
forks in real systems.

- **Undefined and implementation-defined arithmetic.** Signed overflow is UB, so consensus values are
  `uint*_t` only. No bare `+`, `-`, `*` on them — use `lx_u128_add/sub/mul_div_floor` in
  `src/protocol/int128.c`, which return status. `x << n` with `n >= width` is UB, so bound every shift by
  an explicit constant. Integer promotion turns `uint8_t * uint8_t` into `int`: cast before operating.
- **No floating point in execution.** x87 excess precision, FMA contraction, differing `libm` and
  `-ffast-math` all make identical source produce different bits. Build with `-ffp-contract=off` and fail
  CI on any `float` or `double` in a consensus translation unit.
- **Struct padding.** Never hash, sign, compare or persist a struct by `memcpy`/`memcmp` over its bytes;
  padding is uninitialized and implementation-defined. Everything hashed is emitted field by field.
- **Alignment and aliasing.** Read multi-byte values out of buffers with `memcpy` into a local, never a
  pointer cast: unaligned access is UB on some targets and punning breaks strict aliasing. Build with
  `-fno-strict-aliasing` as belt and braces, not as a licence to alias.
- **Char signedness and locale.** `char` signedness is implementation-defined, so byte data is `uint8_t`.
  Never use `strcmp`, `strcoll`, `toupper`, `isalnum`, `atoi`, `strtod`, `sprintf("%f")` or anything else
  locale-sensitive; ordering is `memcmp` over encoded bytes only.
- **Sorting.** `qsort` is unspecified for equal elements and `qsort_r` differs across platforms.
  Consensus ordering is a total order over encoded bytes with duplicates rejected at decode, so no
  stability question arises; where a sort is needed, use the in-tree deterministic merge sort.
- **Pointers and addresses.** No pointer value may influence output: never hash a pointer, order by
  address, or use an address-keyed hash map in a path that affects state. Consensus iteration is over
  ordered arrays or the SMT, never a hash table.
- **Uninitialized memory.** Every buffer that is hashed or encoded is fully initialized; MSan runs in CI
  and an MSan finding in consensus code is a release blocker.
- **Type widths.** `size_t`, `long` and `int` differ across targets. Wire and state values are `uintN_t`
  exclusively with `static_assert` on every serialized size; `size_t` appears only in buffer arithmetic,
  where length checks are `len > remaining` to avoid overflowing `offset + len`.
- **Enums and bitfields.** Enum underlying type is implementation-defined and bitfield layout is
  unspecified, so neither may appear in a serialized or hashed structure; wire enums are explicit
  `u8`/`u16` over closed sets.
- **Ambient state.** `time()`, `clock()`, `gettimeofday()`, `getpid()`, `getenv()`, `rand()`,
  `/dev/urandom`, filesystem state and any network call are forbidden in the transition function. The
  only clock is `ctx->timestamp_ms`; the only randomness is what the log contains.
- **Threads.** Exactly one deterministic state writer. Worker threads do signature verification, decode
  of not-yet-admitted activities, networking, indexing and metrics, and their results cross into the
  writer in canonical order — completion order, atomics, lock ordering and scheduling MUST NOT be
  observable in the state root.
- **Allocation.** Allocation failure is fail-stop (`LX_FATAL`), never a recoverable path, because a
  machine with less memory would otherwise produce different history. Execution uses bounded arenas sized
  from the limits in `wire-format.md` §9, so steady state allocates nothing unbounded.
- **Recursion.** No unbounded recursion in decoding, Merkle recomputation or module execution. Stack
  depth is a machine property and a stack overflow on one node but not another is a fork; Merkle paths
  and decode nesting are iterative with compile-time bounds.
- **Compiler settings.** Consensus objects build at a pinned standard (`-std=c17 -pedantic`),
  warnings-as-errors, `-ffp-contract=off`, no `-Ofast`, no `-march=native`. Codegen differences are
  acceptable only where the C abstract machine guarantees identical results — exactly why UB is excluded
  rather than "handled".

### 7.1 Qualification

Not ready until: byte-identical replay across machines and architectures; deterministic roots over
millions of activities; crash recovery verified at every write boundary; malformed-activity and signature
fuzzing; overflow and rounding proofs; guarantor disagreement and equivocation tests;
data-unavailability simulation; emergency-exit execution on Paxeer; full reserve reconciliation; and
shadow comparison against the external legacy implementation, which stays a read-only reference and
is never translated file by file.

## 8. Invariants asserted every activity

Checked in debug and guarantor builds; a violation is `LX_FATAL`.

1. Every monetary mutation passed through `lxp_apply_transfer_set`.
2. Every debit had explicit resolved authority.
3. `RECEIVE` consumed a live payer grant.
4. No balance is negative — structurally impossible: balances are `u128`, debits checked before write.
5. For every asset in the set, `Σ debits == Σ credits`.
6. Transfer sets are atomic.
7. Exactly one durable receipt per successful transfer.
8. At most one economic result per idempotency key.
9. Every account sequence consumed exactly once.
10. No module wrote tree `02`.
11. No oracle input altered a balance without a module-authorized transfer set.
12. No deposit credited without a finalized Paxeer proof and an unspent nullifier.
13. No withdrawal nullifier spent twice.
14. Replaying this activity from `previous_state_root` reproduces `resulting_state_root` exactly.
