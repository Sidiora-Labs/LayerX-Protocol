# LayerX Batches, Checkpoints and Finality

Normative. This document specifies how accepted activities become batches, how
batches become checkpoints, what Paxeer stores and verifies, what finality does
and does not guarantee, why LayerX cannot reorganise, and how the network
recovers from sequencer loss. Key words MUST, MUST NOT, SHALL, SHOULD and MAY
follow RFC 2119. The implementation is C17; paths are repository-root-relative.

## 1. Roles and objects

One **sequencer** is active at a time. Any number of **replicas** replay history
and serve data. A bonded quorum of **guarantors** independently downloads,
verifies, replays and attests, and their attestations are what Paxeer accepts.
Paxeer holds custody and never sees an individual activity.

```text
activity -> receipt -> batch -> checkpoint -> Paxeer settlement
```

A **batch** is a contiguous, gapless range of global sequences with one signed
header. A **checkpoint** is a contiguous range of batches plus a threshold of
guarantor attestations. Finality is a property of checkpoints, never of
individual activities in isolation.

All commitments use the domain-separated hash of `wire-format.md`,
`H(T, B) = SHA-256( u8(len(T)) || T || B )`, with an ASCII tag beginning `LXP1/`.
Domain separation is mandatory: no two structures may ever share a pre-image.

## 2. Global sequence assignment

`global_sequence` is a `uint64_t` that starts at 1 in the genesis batch and
increases by exactly 1 for every sequenced activity, forever, across epochs and
across sequencer handovers. It never resets, never skips and never repeats.

Assignment rules the sequencer MUST obey:

1. Admission first. An activity failing an *admission* check (see the result
   code table in `activity-types.md`: bad version, network, unknown type,
   payload hash or size, invalid signature, unknown actor, wrong account
   sequence, timestamp bound outside the batch) is dropped before assignment.
   It gets no sequence, no receipt and no fee, and it never appears in history.
2. Everything admitted is assigned. An activity that fails during *execution*
   still receives a sequence, a receipt with a non-zero `result_code`, a
   consumed account sequence and a charged fee. Failures are history.
3. Assignment is single-threaded. Exactly one writer assigns sequences, applies
   transitions and updates the state root, in `src/sequencer/`. Worker threads
   may verify signatures and decode payloads ahead of time; they may not touch
   state.
4. Order inside a batch is the assignment order. There is no reordering pass,
   no priority queue effect on the log, and no fee auction in v1.
5. Per-account ordering is enforced by `account_sequence`. An activity whose
   account sequence is ahead of the expected value MAY be buffered in the
   mempool but MUST NOT be assigned out of order.

A replica or guarantor rejects a batch whose sequences are not contiguous with
the previous batch's `last_sequence + 1`.

## 3. Deterministic batch timestamps

Execution has exactly one clock: `BatchHeader.timestamp`, in milliseconds since
the Unix epoch. Nothing inside a state transition may read the operating system
clock, and any call to `time()`, `clock_gettime()` or equivalent inside
`src/state/` or `src/modules/` is a conformance failure.

Constraints, all checked by guarantors:

| Rule | Constraint |
|---|---|
| Monotonic | `timestamp >= previous_batch.timestamp` |
| Bounded advance | `timestamp - previous_batch.timestamp <= PARAM_MAX_CLOCK_DRIFT_MS` (60000) |
| Not far future | `timestamp <= guarantor_local_clock + PARAM_CLOCK_SKEW_MS` (2000) |
| Covers members | every activity satisfies `not_before_ms <= timestamp <= not_after_ms` |
| Heartbeat | an empty batch MUST be sealed if `PARAM_HEARTBEAT_MS` (10000) elapses |

Equal timestamps across consecutive batches are legal; time may stall but never
runs backwards. Every time-dependent rule in the protocol — expiries, budget
windows, stream accrual, escrow deadlines, oracle staleness, funding intervals,
acceptance windows — reads this field and nothing else.

## 4. The BatchHeader

```c
struct lxp_batch_header {
    uint16_t   protocol_version;
    uint32_t   network_id;
    uint64_t   epoch;
    uint64_t   batch_number;
    uint64_t   first_sequence;
    uint64_t   last_sequence;
    lxp_h256_t previous_state_root;
    lxp_h256_t resulting_state_root;
    lxp_h256_t activity_merkle_root;
    lxp_h256_t receipt_merkle_root;
    lxp_h256_t event_merkle_root;
    lxp_h256_t data_availability_root;
    lxp_h256_t oracle_root;
    uint64_t   timestamp;
    lxp_h256_t sequencer_id;
};
```

| Field | Meaning and constraint |
|---|---|
| `protocol_version` | Active transition-function version. Changes only at a batch named by `GOV_SET_TRANSITION_VERSION`. |
| `network_id` | Chain domain separator. A mismatch is fatal, never a warning. |
| `epoch` | Sequencer term. Increments only on handover (section 11). Guarantors check it against the Paxeer sequencer registry. |
| `batch_number` | Increments by exactly 1 forever, across epochs. Genesis is batch 0. |
| `first_sequence` | Sequence of the first activity, `= previous.last_sequence + 1`. |
| `last_sequence` | Sequence of the last activity. `activity_count = last_sequence - first_sequence + 1`. An empty heartbeat batch sets `first_sequence = last_sequence + 1`, giving count 0. |
| `previous_state_root` | MUST equal the previous batch's `resulting_state_root`. This is the chain link. |
| `resulting_state_root` | Root of the authenticated state trie after applying every activity in order. |
| `activity_merkle_root` | Merkle root over canonical activity envelope bytes, in sequence order. |
| `receipt_merkle_root` | Merkle root over canonical receipts, in sequence order. Receipt *i* corresponds to activity *i*. |
| `event_merkle_root` | Merkle root over emitted events in emission order, including every transfer leg. |
| `data_availability_root` | Root over the DA chunk digests for this batch (section 6). |
| `oracle_root` | Merkle root over the accepted oracle observation payloads in this batch, verbatim. Empty root when none. |
| `timestamp` | Section 3. Named `timestamp_ms` in the encoding. |
| `sequencer_id` | `H("LXP1/sequencer", ed25519_pk \|\| secp256k1_pk)`. Binds both keys of the active sequencer. |

Merkle construction is fixed by `wire-format.md`:
`leaf = H("LXP1/merkle-leaf", item_bytes)`,
`node = H("LXP1/merkle-node", left || right)`, RFC-6962 splitting at the largest
power of two strictly below the leaf count. The last leaf is **never duplicated**
to pad, because that admits the classic two-trees-one-root collision. The empty
root is `H("LXP1/merkle-empty", "")` and a single-leaf root is the leaf hash
itself. Implementations MUST NOT substitute any other convention.

`batch_id = H("LXP1/batch-header", B)` over the field order above; the sequencer
signs the same body under the tag `LXP1/batch-sign` with Ed25519, and the
signature travels beside the header, not inside it.

## 5. Sealing

A batch seals when the first of these fires: `LX_MAX_BATCH_ACTIVITIES` (65536),
`LX_MAX_BATCH_BYTES` (64 MiB), `PARAM_BATCH_INTERVAL_MS` (250) since the last
seal, or `PARAM_HEARTBEAT_MS` with an empty mempool.

The seal procedure is ordered and crash-safe:

1. Freeze the member list; no activity may be added after this point.
2. Apply every activity in order through the single state writer, collecting
   receipts and events.
3. Compute `resulting_state_root`, then the four content roots and `oracle_root`.
4. Build the DA chunk set and `data_availability_root`.
5. Fill the header, compute `batch_id`, sign it.
6. `fsync` the activity log segment, then the receipt segment, then the header
   record, in that order.
7. Only after step 6 returns may any receipt be published to a client, and only
   marked at level L1 (section 8).

Crash recovery replays from the last fully written header record. A partially
written trailing segment is truncated: since no receipt for it was published, no
client saw a value that recovery invalidates. This ordering is the reason the
header is written last.

## 6. Distribution requirement

A batch is **not checkpoint-eligible** until its complete data is provably in
other hands. Per `data-availability.md`, the DA object for a batch is a manifest
over five sections — `ACTIVITIES`, `RECEIPTS`, `ORACLE`, `STATE_DIFF`,
`RECOVERY` — each split into `LX_DA_CHUNK_SIZE` (64 KiB) chunks with a chunk
Merkle root. `BatchHeader.data_availability_root` is exactly the manifest root,
and a header whose recomputed manifest root differs is an invalid header.
Sections 1, 4 and 5 together MUST suffice to advance `previous_state_root` to
`resulting_state_root` with no other node's help.

Eligibility rule: a batch becomes checkpoint-eligible only when at least the
attestation threshold `T` of guarantors have each fetched every section,
recomputed the manifest root, and stored it under their retention policy. A
guarantor MUST NOT attest to possession it does not have; that assertion is bit 1
of `attest_flags` and is slashable (section 7 and `guarantors.md`).

A state root without retrievable activity data is worthless: any agent must be
able to fetch the DA object for any finalized batch and independently reproduce
every root. Guarantors retain the activity, receipt, oracle and recovery sections
for at least 90 days past finality; archive nodes retain everything forever.

## 7. Checkpoint certificate

A checkpoint covers a contiguous run of batches, sealed and distributed, up to
`PARAM_CHECKPOINT_BATCHES` (2400, about 10 minutes at the 250 ms target) or
`PARAM_CHECKPOINT_MAX_MS` (600000), whichever comes first.

```c
struct lxp_checkpoint {                /* body, tag "LXP1/checkpoint" */
    uint16_t   protocol_version;
    uint32_t   network_id;
    uint64_t   epoch;
    uint64_t   checkpoint_number;
    uint64_t   first_batch, last_batch;
    lxp_h256_t start_state_root;       /* = first_batch.previous_state_root  */
    lxp_h256_t end_state_root;         /* = last_batch.resulting_state_root  */
    lxp_h256_t batch_merkle_root;      /* Merkle over batch_id, in order     */
    lxp_h256_t data_availability_root; /* Merkle over per-batch manifest roots */
    uint64_t   timestamp_ms;           /* = last_batch.timestamp             */
    uint32_t   guarantor_set_id;       /* registry version that must attest  */
};
```

`checkpoint_id = H("LXP1/checkpoint", B)`. The body is deliberately lean: the
per-batch activity, receipt, event and oracle roots and the `sequencer_id` are
all committed inside the batch headers, which are committed by
`batch_merkle_root`, so a proof against any of them is a two-step proof and the
Paxeer contract never needs to parse one. There is no `previous_checkpoint_id`
field because `start_state_root` is the chain link (section 10).

The **certificate** submitted to Paxeer is the body plus an ordered signer set:

```c
struct lxp_checkpoint_certificate {
    struct lxp_checkpoint cp;
    uint16_t   signer_count;
    struct { uint32_t guarantor_id; uint8_t attest_flags;
             lxp_secp_sig_t sig; } signers[LX_MAX_GUARANTORS]; /* 65-byte sigs */
};
```

`guarantor_id` values are strictly ascending, so duplicates are structurally
impossible. Each signature is secp256k1 over the `LXP1/guarantor-attest`
pre-image, which binds `checkpoint_id` and `attest_flags`; the exact digest is in
`guarantors.md` and is identical byte for byte on both sides.

**Threshold rule.** Let `N` be the number of active, non-jailed, fully bonded
members of `guarantor_set_id` and `T` its threshold. Paxeer accepts a certificate
only when `signer_count >= T`, `T >= floor(2*N/3) + 1`, `T >= LX_MIN_THRESHOLD`
(3), `N >= LX_MIN_GUARANTORS` (4), every `guarantor_id` resolves to an active
member, and every `attest_flags` has both bits set. Initial parameters: `N = 7`,
`T = 5`.

### 7.1 What Paxeer stores and verifies

Paxeer holds assets and understands as little LayerX business logic as possible.
It stores, per network: the head `checkpoint_number`, `checkpoint_id`,
`end_state_root`, `data_availability_root` and `timestamp_ms`; the guarantor
registry (`guarantor_set_id`, member addresses, bonds, jail state, `T`); the
sequencer registry (`epoch`, `sequencer_id`, resume point); the spent withdrawal
nullifier set; per-asset withdrawal counters for the rate limit; and open
challenges.

It verifies, and nothing more: the body decodes and `checkpoint_id` recomputes;
`checkpoint_number == head + 1`; `start_state_root == stored end_state_root`;
`epoch` matches the sequencer registry; every signature recovers to a distinct
active member with both `attest_flags` bits set; `signer_count >= T`; the
withdrawal rate limit is not exceeded; and, for a payout, a membership or
balance proof against `end_state_root` plus an unspent nullifier.

It MUST NOT parse an activity, a receipt, a perps order, a service agreement or
a transfer. Everything it checks is a hash, a signature, a counter or a Merkle
path. That is what keeps millions of agent activities off the settlement layer.

## 8. Finality ladder

| Level | Name | Reached when | What a holder may rely on |
|---|---|---|---|
| L0 | accepted | sequencer admitted it | nothing; a courtesy acknowledgement |
| L1 | sealed | inside a signed, fsynced batch | ordering is fixed and the sequencer is cryptographically accountable |
| L2 | distributed | DA object held by `T` guarantors | anyone can replay and reproduce it; survives sequencer loss |
| L3 | attested | `T` guarantor signatures exist | bonded capital stands behind the state root |
| L4 | settled | certificate accepted on Paxeer and the challenge window elapsed | withdrawals and exits against this root are payable |

Clients MUST be told the level of every receipt. Anything at or below L1 is
revocable by sequencer loss (section 11). Nothing at L2 or above is.

`PARAM_CHALLENGE_WINDOW_MS` (86400000, one day) separates L3 from L4 on Paxeer.

## 9. What finality guarantees, and what it does not

Guarantees at L4: the activity sequence is immutable; the state root is the one
`T` bonded guarantors independently recomputed; the DA object exists and is
retrievable; withdrawals covered by the root cannot be paid twice; every receipt
under the root has an inclusion proof against `activity_root`, `receipt_root`
and `resulting_state_root`.

It does **not** guarantee:

- **Validity.** A threshold attestation is a bonded assertion by a quorum, not a
  proof. If more than `N - T` guarantors collude with the sequencer, an invalid
  state root can settle. See `guarantors.md`.
- **Censorship resistance.** A single sequencer can refuse to admit an activity.
  Finality says nothing about what was excluded. The mitigation is the Paxeer
  emergency exit, not the checkpoint.
- **Truth of inputs.** An oracle observation is final as *data*, not as *price*.
  A tool-execution attestation is final as a *signed claim*, never as evidence
  that the tool ran.
- **Liveness.** Finality of past checkpoints implies nothing about future ones.
- **Anything below L2.** A sealed but undistributed batch may be discarded
  during recovery.

State this honestly in every client SDK. A receipt is evidence of what LayerX
recorded, not a warrant of external fact.

## 10. Reorg impossibility versus halt

LayerX has **no fork-choice rule**, by design. There is exactly one chain of
state roots because each header pins `previous_state_root` and each checkpoint
pins `previous_checkpoint_id`. Paxeer accepts a certificate only when
`previous_checkpoint_id` equals the stored head and `previous_state_root` equals
the stored `resulting_state_root`. A second, conflicting certificate at the same
`checkpoint_number` is therefore not a competing branch that could win — it is
rejected on arrival and it is *evidence*.

The consequences are deliberate:

- Two signed batches with the same `batch_number` and different `batch_id`
  constitute sequencer equivocation. Guarantors MUST stop attesting, submit both
  headers as evidence, and halt.
- Two attestations by one guarantor over conflicting checkpoints at the same
  number are guarantor equivocation and are slashable on Paxeer.
- A guarantor whose independent replay disagrees with the sequencer MUST refuse
  to sign and publish its own root. If the threshold cannot be reached, the
  chain stops advancing. This is the correct outcome.

LayerX prefers safety to liveness at every such fork: **ambiguity halts the
chain, it never resolves it.** A halted chain is recoverable by governance and,
in the worst case, by emergency exit on Paxeer. A silently reorganised chain
would destroy the accounting guarantee that the whole design exists to provide.

## 11. Recovery after sequencer loss

Trigger: no new batch for `PARAM_LIVENESS_TIMEOUT_MS` (30000), or proven
equivocation, or a governance halt.

1. **Freeze.** Guarantors stop attesting. Replicas stop accepting new headers
   from the failed `sequencer_id`.
2. **Establish the resume point.** Every guarantor publishes its highest batch
   that it verified *and* whose DA object it fully holds. The resume point is the
   highest batch number at level L2 or above, i.e. held by at least `T`
   guarantors, together with its `resulting_state_root`.
3. **Discard the tail.** Batches above the resume point are discarded. They were
   at most L1, so no L2+ receipt is invalidated. Discarded activities return to
   the mempool and may be resubmitted; their account sequences were never
   consumed from any surviving state, so resubmission is safe. Clients holding
   L1 receipts for discarded batches MUST treat them as void.
4. **Select the successor.** Governance enacts a sequencer change and Paxeer
   records `(epoch + 1, new_sequencer_id, resume_batch, resume_state_root)` in
   the sequencer registry. Only a pre-registered standby that has proven it is
   synced to at least the resume point is eligible.
5. **Verify before producing.** The successor replays from the last settled (L4)
   checkpoint forward to the resume point and MUST reproduce every root exactly.
   A mismatch aborts the handover; the chain stays halted.
6. **Resume.** The first new batch carries `epoch = old_epoch + 1`,
   `batch_number = resume_batch + 1`, `previous_state_root = resume_state_root`,
   and a `timestamp` at least the resume batch's. `batch_number` and
   `global_sequence` never reset. Guarantors reject any batch whose
   `(epoch, sequencer_id)` disagrees with the Paxeer registry, which is what
   makes a returning old sequencer harmless.
7. **Catch up.** The successor checkpoints the resumed range on the normal
   schedule. The first post-handover certificate MUST cover the resume point.
8. **Escape hatch.** If no successor is registered within
   `PARAM_EXIT_INACTIVITY_MS` (604800000, seven days) of the last settled
   checkpoint, Paxeer enters emergency-exit mode and agents withdraw directly
   against the last settled `end_state_root` using balance proofs. That
   path needs no LayerX liveness at all, which is the ultimate backstop and the
   reason DA retention is mandatory rather than best-effort.

## 12. Constants

| Constant | Value |
|---|---|
| `PARAM_BATCH_INTERVAL_MS` | 250 |
| `PARAM_HEARTBEAT_MS` | 10000 |
| `LX_MAX_BATCH_ACTIVITIES` | 65536 |
| `LX_MAX_BATCH_BYTES` | 67108864 |
| `PARAM_MAX_CLOCK_DRIFT_MS` | 60000 |
| `PARAM_CLOCK_SKEW_MS` | 2000 |
| `PARAM_CHECKPOINT_BATCHES` | 2400 |
| `PARAM_CHECKPOINT_MAX_MS` | 600000 |
| `PARAM_CHALLENGE_WINDOW_MS` | 86400000 |
| `PARAM_LIVENESS_TIMEOUT_MS` | 30000 |
| `PARAM_EXIT_INACTIVITY_MS` | 604800000 |
| `PARAM_DA_RETENTION_DAYS` | 90 |
| `LX_MAX_GUARANTORS` | 256 |
| `LX_MIN_GUARANTORS` | 4 |
| `LX_MIN_THRESHOLD` | 3 |

## 13. Conformance

Under `tests/checkpoint/` an implementation MUST demonstrate: byte-identical
headers and roots for the same history on at least two architectures; crash
injection at every `fsync` boundary in section 5 with no published receipt ever
invalidated; rejection vectors for non-contiguous sequences, backwards
timestamps, wrong `previous_state_root`, wrong epoch and sub-threshold
certificates; a sequencer-loss drill executing section 11 end to end; and an
emergency-exit drill proving withdrawal against a settled root with LayerX
offline.
