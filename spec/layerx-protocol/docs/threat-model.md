# LayerX Threat Model

Status: normative. Version 1. Protocol tag `LXP1`. Binding on the C17 reference
implementation in this repository.

This document defines what the protocol protects, whom it protects it from, and what it does
**not** protect. Every mitigation names an enforcement point; a mitigation with no enforcement
point is a release blocker.

Framing: LayerX is the canonical activity, execution and accounting layer for autonomous agents.
Paxeer provides custody, checkpoint finality, economic guarantees and dispute settlement, and
never processes ordinary agent activity. 402LXP is the single financial doorway — every monetary
effect compiles into one or more authenticated balance transfers, and no module writes a balance
directly.

## 1. Assets under protection

| ID | Asset | Loss condition |
|---|---|---|
| A1 | Custodied value in Paxeer contracts | Value leaves custody without a finalized checkpoint and an unspent nullifier |
| A2 | LayerX balances and subaccount ledger | A balance changes outside `lxp_apply_transfer_set`, or conservation breaks |
| A3 | Activity ordering (append-only log) | Two conflicting histories are accepted for one sequence range |
| A4 | Authority state (keys, session keys, grants, revocations) | A debit executes without live authority over the debited account |
| A5 | Sequences and idempotency records | A sequence is consumed twice, or one idempotency key yields two economic results |
| A6 | Data availability of batches, receipts, oracle inputs, state diffs | An agent cannot replay history or prove a balance for exit |
| A7 | Deterministic replay | Two honest nodes on identical history compute different state roots |
| A8 | Service-lifecycle records | Commitments, tool attestations, deliveries, acceptances or disputes are forged, reordered or lost |
| A9 | Liveness of admission and checkpointing | Agents cannot get activities sequenced, or cannot get funds out |
| A10 | Oracle inputs consumed by `perps` | Prices are manipulated, stale, or replayed across markets |

A8 is in scope because v1 covers the **complete agent work lifecycle**, not only economically
meaningful actions. Lifecycle activities carry no direct monetary effect — any value they imply
still moves through 402LXP transfers — but they are ordered, attested and checkpointed, so forging
one attacks the record even when no balance moves.

## 2. Trust boundaries

```
   agent process            LayerX node                      Paxeer chain
 ┌───────────────┐   B1  ┌──────────────────────────┐  B3  ┌───────────────┐
 │ keys, payload │──────▶│ admission → deterministic│─────▶│ custody,      │
 │ construction  │       │ execution → log → roots  │      │ checkpoints,  │
 └───────────────┘       └──────────────────────────┘      │ slashing,     │
         │  B2 (gateway)       ▲ B4        │ B5            │ exits         │
         └────────────────────▶│    guarantors ◀── oracle  └───────────────┘
```

- **B1 — untrusted input.** Everything crossing it is hostile bytes until the codec and signature
  verifier accept it (`src/codec/`, `src/crypto/`).
- **B2 — optional JSON/HTTP gateway.** Convenience only; it MUST NOT define consensus behaviour.
  It re-encodes to canonical binary, and the binary form is what is signed, hashed, logged and
  replayed. A gateway compromise MUST NOT be able to change an outcome the agent authorized.
- **B3 — settlement.** Only checkpoint certificates, membership and balance proofs, nullifiers and
  exit claims cross it. Paxeer contracts MUST NOT parse perps orders, service agreements or
  ordinary transfers.
- **B4 — replay.** Guarantors re-derive everything from batch bytes and trust no
  sequencer-supplied result.
- **B5 — external data.** Oracle observations enter as signed activities and become replayable
  history; execution performs no network I/O.

## 3. Actors: trusted for, and not trusted for

### 3.1 Agent (account owner)
- **Trusted for:** custody of its own primary key, choice of counterparties, and the meaning of
  its own payloads.
- **Not trusted for:** encoding correctness, sequence monotonicity, well-formed text, bounded
  payload sizes, uniqueness of idempotency keys, or the truth of any balance, timestamp or
  "resulting state" it asserts.
- Clients MUST never supply authoritative new-balance values; the protocol computes them and
  reports them as evidence in the receipt.

### 3.2 Sequencer (one active, initially)
- **Trusted for:** proposing an order, assembling batches, supplying the batch timestamp within
  protocol bounds, distributing batch data to replicas.
- **Not trusted for:** validity of transitions, honesty of results, fair inclusion, or
  non-equivocation.
- It MUST NOT be able to mint value, alter authority, forge a receipt for an activity the agent
  did not sign, or reorder a finalized range. A lying sequencer produces a batch no guarantor
  attests; an equivocating one is slashable at B3.

### 3.3 Guarantor (bonded Paxeer quorum)
- **Trusted for:** independently downloading the complete batch, verifying every signature,
  replaying every transition, recomputing all roots, storing the required availability data, and
  signing only on full agreement.
- **Not trusted for:** individual honesty, mutual independence, or liveness.
- A guarantor MUST NOT sign a checkpoint it did not fully replay. It can only attest or withhold —
  never alter state. Threshold attestation is an economic guarantee, not a validity proof (§5.1).

### 3.4 Oracle signer
- **Trusted for:** authenticity of its own observations under its registered key at the timestamp
  it signs.
- **Not trusted for:** truth of the value, timeliness, or non-collusion.
- Oracle data MUST NOT alter balances directly. It feeds `perps`, whose monetary effects still
  compile to 402LXP transfers.

### 3.5 Service (counterparty issuing HTTP 402 requirements)
- **Trusted for:** deciding whether to deliver its own resource, and signing its own payment
  requirements and delivery attestations.
- **Not trusted for:** debiting anyone.
- `RECEIVE` MUST carry a signed payer grant. A service MUST NOT pull from an account that never
  granted it, beyond the grant's cap, after expiry, or after the grant's revocation sequence.

### 3.6 Paxeer contract
- **Trusted for:** custody, deposit finality, checkpoint registration, bond accounting, slashing,
  challenge windows and emergency exits.
- **Not trusted for:** understanding LayerX business logic. It verifies a certificate, a proof
  against a state root, a nullifier and a window — nothing else.
- Its correctness is a root assumption; a bug there is unmitigated by anything in this document
  (§5.4).

## 4. Attack catalogue

### 4.1 Sequencer censorship
- **Attack.** The single active sequencer drops or delays a target's activities.
- **Impact.** Loss of A9; targeted denial of settlement.
- **Mitigation.** Agents MAY submit to any replica, which gossips into the mempool and timestamps
  receipt of submission. An agent MAY escalate by posting the signed envelope to Paxeer as a
  **forced-inclusion request**, which the sequencer MUST satisfy with inclusion or a deterministic
  rejection receipt within `FORCED_INCLUSION_EPOCHS`; otherwise the checkpoint is invalid and
  guarantors MUST refuse to attest. Unresolved censorship escalates to emergency exit (§6).
- **Enforcement.** `src/sequencer/inclusion.c`, `src/guarantor/verify_batch.c`.
- **Residual.** Latency up to the forced-inclusion window.

### 4.2 Sequencer equivocation
- **Attack.** Two different batches signed for one `(epoch, batch_number)`, or two histories from
  one `previous_state_root`.
- **Impact.** Split view; double spend against off-chain observers.
- **Mitigation.** Headers are chained by `previous_state_root` and signed under tag
  `LXP1/batch-sign`. Any party holding two conflicting signed headers for the same `(network_id,
  epoch, batch_number)` submits both to Paxeer, which verifies the signatures and slashes the
  bond. Finality for external parties derives from the **checkpoint**, never from a sequencer
  signature alone.
- **Enforcement.** `contracts/`, `src/paxeer/equivocation.c`.
- **Residual.** Pre-checkpoint receipts are provisional by construction and MUST be presented to
  agents that way.

### 4.3 Invalid state transition
- **Attack.** The sequencer computes results the transition function does not imply.
- **Impact.** Theft, minting, unauthorized authority change.
- **Mitigation.** Every guarantor recomputes `resulting_state_root`, `activity_merkle_root`,
  `receipt_merkle_root`, `event_merkle_root` and `oracle_root` from batch bytes, and MUST refuse
  to sign on any mismatch; checkpoints require the guarantor threshold. Kernel invariants are
  asserted per activity: conservation per asset, no negative balance, single balance-mutation
  path.
- **Enforcement.** `src/state/transition.c`, `src/modules/asset/lxp_transfer.c`,
  `src/guarantor/verify_batch.c`.
- **Residual.** A transition function that is itself wrong is reproduced and attested by every
  honest node (§5.2).

### 4.4 Guarantor collusion
- **Attack.** A threshold of guarantors jointly attests an invalid checkpoint.
- **Impact.** Total loss of A1 for the affected range.
- **Mitigation.** Independence requirements (distinct operators, keys, and where possible
  implementations); bonds sized so aggregate slashable stake exceeds the value transferable in one
  challenge window; a challenge window in which **any** party may submit a re-execution
  disagreement witness against a registered checkpoint before withdrawals finalize; governance
  halt of exits on proof of quorum failure.
- **Enforcement.** `contracts/`, `src/paxeer/challenge.c`.
- **Residual.** This is the primary residual risk in the entire system (§5.1).

### 4.5 Guarantor equivocation
- **Attack.** One guarantor signs two different certificates for one `(epoch, checkpoint_number)`.
- **Impact.** Ambiguous finality; attempted double settlement.
- **Mitigation.** Attestations are signed under `LXP1/guarantor-attest` over a preimage binding
  `(network_id, epoch, checkpoint_number)`. Two valid signatures with the same binding and
  different digests form a self-contained slashing proof that requires no replay to verify.
- **Enforcement.** `contracts/` slashing path.
- **Residual.** None beyond bond sizing; detection is trivial and permissionless.

### 4.6 Data withholding
- **Attack.** Roots are published but the underlying batch data is not.
- **Impact.** Loss of A6 — agents cannot replay, prove balances, or exit correctly.
- **Mitigation.** A checkpoint is finalizable only when the guarantor threshold attests possession
  of the complete activity batch, receipts, oracle inputs, state-diff material and recovery
  metadata, with `data_availability_root` inside the attestation preimage. Guarantors MUST serve
  any chunk under that root; sustained failure is a bondable fault. Agents MUST be able to
  retrieve and independently replay finalized history.
- **Enforcement.** `src/guarantor/da_attest.c`, `src/network/da_serve.c`.
- **Residual.** A guarantor may attest truthfully and still refuse one specific requester; the
  backstop is emergency exit against the last checkpoint whose data the agent already holds.

### 4.7 Oracle manipulation and staleness
- **Attack.** A signer reports a false value, or a true-but-old value is replayed into a later
  batch or a different market.
- **Impact.** Unjust liquidation, funding theft, insurance drain via `perps`.
- **Mitigation.** Observations are signed under `LXP1/oracle-obs` binding `(network_id, market_id,
  observation_time, sequence)` — the market binding blocks cross-market reuse, the sequence blocks
  replay. The module consumes a median over at least `ORACLE_MIN_SIGNERS` distinct registered
  signers and MUST **fail closed** when the quorum is unmet. An observation with `batch_timestamp
  - observation_time > ORACLE_MAX_AGE_MS` is rejected. Per-batch deviation clamps bound a single
  update's influence. Accepted payloads become permanent replayable evidence.
- **Enforcement.** `src/modules/perps/oracle_gate.c`.
- **Residual.** Collusion of a majority of registered signers within one batch. Fail-closed also
  means an oracle outage suspends liquidations — a liveness cost accepted deliberately.

### 4.8 Replay and double spend
- **Attack.** A captured signed activity is resubmitted, possibly on another network or protocol
  version.
- **Impact.** Duplicate debits.
- **Mitigation.** The signing preimage binds `protocol_version`, `network_id`, `activity_type`,
  `actor_did`, `account_sequence`, `timestamp_bound` and `payload_hash` under a domain tag; the
  sequence must equal the actor's `next_sequence` exactly; `not_after` bounds the replay horizon;
  identity is `H("LXP1/activity-id", envelope)`. A replay fails the sequence check and never
  reaches execution.
- **Enforcement.** `src/state/authority.c`, `src/state/sequence.c`.
- **Residual.** None for exact replay; near-replay is covered by 4.9 and 4.10.

### 4.9 Sequence reuse
- **Attack.** Two distinct activities are produced for one `(actor_did, account_sequence)`,
  typically by a duplicated agent process.
- **Impact.** One is silently dropped while the agent believes both executed.
- **Mitigation.** Sequence consumption is a state write inside the transition function, not a
  mempool heuristic. The check is `seq == next_sequence[actor]`, and consumption sets
  `next_sequence = seq + 1` exactly once, before any module effect can fail. The loser
  deterministically yields `LX_ERR_BAD_SEQUENCE` at admission and is never sequenced. Agents MUST
  treat a receipt, not a submission, as evidence.
- **Enforcement.** `src/state/sequence.c`.
- **Residual.** An agent running two writers gets nondeterministic which-one-wins; that is its own
  key-management problem.

### 4.10 Idempotency abuse
- **Attack.** An idempotency key is reused with different contents, or distinct keys are flooded
  to bloat state.
- **Impact.** A second economic result under one key, or unbounded state growth.
- **Mitigation.** The idempotency tree maps `(actor_did, idempotency_key)` to `(activity_id,
  result_digest)`. A different `activity_id` presenting an existing key is admitted, consumes its
  sequence, pays its fee, and terminates with `LX_ERR_IDEMPOTENCY_CONFLICT` and **zero module
  effects** — never a second economic result. Records expire after `IDEM_RETENTION_MS`, which MUST
  exceed the maximum `not_after` horizon so expiry can never resurrect a spendable key. Growth is
  paid for by fees.
- **Enforcement.** `src/state/idempotency.c`.
- **Residual.** A client retrying after retention is protected by the sequence rule, not by
  idempotency.

### 4.11 Grant replay and over-draw after revocation
- **Attack.** A service holding a signed payer grant calls `RECEIVE` repeatedly, past the cap,
  past expiry, or after revocation.
- **Impact.** Unauthorized debits of the payer.
- **Mitigation.** `RECEIVE` MUST NOT let a recipient debit arbitrary accounts. The grant is
  **state**, not a bearer document: `grant_id` indexes a record holding authorized recipient,
  asset, per-draw maximum, total or recurring allowance, window, permitted purpose, optional
  invoice identifier and `revocation_sequence`. Each draw decrements the remaining allowance
  atomically inside the same transfer set. A grant is live only if its revocation epoch is
  unchanged and `batch_timestamp` is inside its window; revocation is an ordinary activity that
  raises the epoch, after which later draws deterministically fail.
- **Enforcement.** `src/state/grants.c`, `src/modules/asset/receive.c`.
- **Residual.** Draws already ordered before the revocation activity are final. The payer's
  exposure is bounded by the grant cap and the inclusion window, and MUST be surfaced to agents
  that way.

### 4.12 Double withdrawal
- **Attack.** One withdrawal is proven twice, or proven against two checkpoints.
- **Impact.** Direct loss of A1.
- **Mitigation.** A withdrawal moves value `agent → system:paxeer-withdrawals` inside LayerX and
  produces `H("LXP1/withdrawal-nullifier", network_id || account || asset || amount ||
  global_sequence)`. The Paxeer contract marks the nullifier spent atomically with payout and MUST
  reject repeats regardless of which checkpoint proves them. Amount and asset are bound in, so
  partial re-proof is impossible.
- **Enforcement.** `contracts/`, `src/modules/bridge/withdraw.c`.
- **Residual.** None, conditional on contract correctness.

### 4.13 Forged deposit
- **Attack.** A deposit activity claims funds that were never custodied.
- **Impact.** Minting; conservation break.
- **Mitigation.** Paxeer deposits cannot credit LayerX without finalized proof. A deposit is
  admitted only with a deposit-event proof against a finalized Paxeer block, and credits as a
  transfer `system:paxeer-reserve → agent`, never as a mint; each deposit consumes a nullifier in
  the bridge tree. Reserve reconciliation (`Σ mirror ≥ Σ agent balances`, per asset) is asserted
  at every checkpoint.
- **Enforcement.** `src/modules/bridge/deposit.c`.
- **Residual.** Dependence on the configured finality depth of the source chain.

### 4.14 Key compromise and session-key abuse
- **Attack.** A session key is stolen, or used outside its granted scope.
- **Impact.** Unauthorized activity within the compromised authority.
- **Mitigation.** Authorization is part of the state machine, not HTTP middleware. Session keys
  are registered records with explicit scopes (permitted `activity_type` set, per-asset spend
  ceilings, expiry), bound under `LXP1/session-key-bind` to the primary key. Every activity
  resolves authority against live state before execution; out-of-scope use is
  `LX_ERR_AUTHORITY_SCOPE`. The primary key may rotate or revoke instantly, and a recovery record
  permits rotation after a delay window in which the old key can veto.
- **Enforcement.** `src/state/authority.c`.
- **Residual.** Primary-key compromise is unrecoverable except through the recovery record and its
  delay. Agents MUST keep the primary key offline and operate through session keys.

### 4.15 Integer overflow
- **Attack.** Crafted amounts, fees, allowances or funding rates near type bounds.
- **Impact.** Wrapped balances, conservation break, effectively minted value.
- **Mitigation.** All consensus arithmetic is integer-only on unsigned fixed-width types, with
  **no bare `+`, `-` or `*` on consensus values**. Everything goes through checked helpers
  (`lx_u128_add`, `lx_u128_sub`, `lx_u128_mul_div_floor`, …) in `src/protocol/int128.c` that
  return a status and never wrap silently. Signed overflow is undefined behaviour in C and is
  structurally excluded by using unsigned types throughout. Rounding is fixed and explicit (floor,
  with the remainder assigned to a named party).
- **Enforcement.** Unit proofs, `fuzz/`, `-fsanitize=integer` in CI.
- **Residual.** A bug in a helper — addressed by exhaustive edge cases at 0, 1, MAX-1 and MAX, and
  differential tests against a reference big-integer implementation.

### 4.16 Non-canonical encoding
- **Attack.** Two byte strings decode to one logical activity, or a malleable field changes
  `activity_id` without changing meaning.
- **Impact.** Hash-identity confusion, duplicate execution, divergent roots.
- **Mitigation.** Exactly one valid encoding per value (`wire-format.md`): fixed-width big-endian
  scalars, minimal-length varints, closed enum sets, strictly ascending map and set keys with
  duplicates rejected, no trailing bytes, and a decoder that validates and never normalizes. The
  implementation MUST assert `re-encode(decode(b)) == b` for every accepted activity in debug and
  fuzz builds. Ed25519 verification MUST enforce canonical `S` and reject small-order keys;
  secp256k1 MUST reject high-`s`.
- **Enforcement.** `src/codec/lxp_codec.c`, `src/crypto/`.
- **Residual.** None known; this is a primary fuzzing target.

### 4.17 Denial of service and resource exhaustion
- **Attack.** Floods of expensive activities, oversized payloads, deep nesting, or state-bloating
  writes.
- **Impact.** Loss of A9; unbounded storage; node crashes.
- **Mitigation.** Hard size limits checked at the codec boundary **before** allocation; bounded
  nesting depth and no unbounded recursion in decode or Merkle paths; `fee_limit` on every
  activity with metered resource units, so state growth is paid for; signature verification — the
  expensive step — runs on worker threads outside the deterministic writer, with results collected
  in canonical order; per-connection admission rate limits that are local policy and MUST NOT
  affect consensus outcomes; allocation failure is fail-stop, never a divergent code path.
- **Enforcement.** `src/network/`, `src/codec/`, `src/state/fees.c`.
- **Residual.** A well-funded attacker raises fees for everyone — the intended economic response,
  not a failure.

## 5. Residual risks, stated honestly

**5.1 Threshold attestation is not a validity proof.** This is the most important limitation in
the system. A checkpoint accepted by Paxeer means *a threshold of bonded guarantors claims to have
replayed this batch and stored its data*. It does not mean the transition was valid. If a
threshold colludes — or if they all run the same buggy binary and the bug is triggered — an
invalid state root can be finalized. The protections are economic (bonds, slashing for
equivocation), procedural (challenge window, permissionless fraud submission) and structural
(client independence), not cryptographic. A later version may add validity proofs **without
changing the activity protocol**; the wire format and transition function are specified so that
substitution is possible. Until then, no document, UI or SDK may describe checkpoint finality as
"proven".

**5.2 A correct implementation of a wrong rule.** Determinism guarantees agreement, not
correctness. An economic error in the transition function is reproduced by every honest node and
attested by every honest guarantor. The mitigations are specification-level: versioned transition
functions, shadow replay against the external legacy implementation before cutover, reserve
reconciliation at every checkpoint, and governance emergency controls.

**5.3 Single active sequencer.** v1 accepts one sequencer, so liveness has a single point of
failure and short-horizon ordering is a trusted role. Forced inclusion, equivocation slashing and
emergency exit bound the damage; none of them restore low latency during an outage.

**5.4 Trust in the Paxeer contracts.** Contract bugs are outside LayerX's ability to mitigate. The
only countermeasure is minimality: the contracts verify certificates, proofs, nullifiers and
windows, and understand no business logic.

**5.5 Agent-side key custody.** The protocol cannot distinguish an agent from an attacker holding
its key. Session-key scoping and revocation bound the damage; they do not prevent it.

**5.6 Off-chain semantics of lifecycle activities.** Ordered, attested deliveries and acceptances
prove what was claimed, when, and by whom. They do not prove that a deliverable was good or that a
tool actually ran. Disputes are settled by evidence and Paxeer arbitration, never by the state
machine asserting truth about the world.

## 6. Emergency exit: the ultimate backstop

Every mitigation above eventually terminates here. The escape hatch exists so that no combination
of sequencer misbehaviour, guarantor unavailability or data withholding can trap custodied value
indefinitely.

Triggers, any one of which the Paxeer contract evaluates independently of LayerX:

1. No checkpoint registered for `EXIT_STALL_EPOCHS`.
2. A forced-inclusion request unanswered past its window.
3. A proven equivocation by the sequencer or by a threshold-relevant guarantor set.
4. A governance declaration of emergency mode.

Procedure:

1. The agent takes the **last finalized checkpoint** and its `state_root`.
2. It produces a balance-membership proof for `agent:<did>:main`, and for each named subaccount it
   claims, against that root — from data it already holds or retrieves under
   `data_availability_root`.
3. It submits an exit claim; the contract verifies the certificate, the proof and an unspent exit
   nullifier.
4. A challenge window opens in which anyone may present a later finalized checkpoint showing the
   balance already spent or withdrawn.
5. On expiry, custody pays out and the nullifier is marked spent.

Properties the implementation MUST preserve:

- Exit MUST depend only on a finalized checkpoint, an inclusion proof and a nullifier — never on
  sequencer cooperation, guarantor cooperation, or LayerX liveness.
- Exit MUST be possible from data an agent can hold locally. Any design that makes exit require a
  live LayerX node is invalid.
- Exit MUST NOT require the contract to interpret escrow, budget, stream, service or perps
  semantics. Module positions resolve to account balances in the state tree first; the contract
  only ever proves an account balance.
- Emergency mode MUST halt new deposits and ordinary withdrawals, so the same reserve cannot be
  paid twice while exits drain it.

Any change that weakens those four properties is rejected regardless of its other benefits.
