# LayerX Paxeer Guarantors

Normative. This document specifies the guarantor role: duties, machine
requirements, bonding, the attestation message and its signing pre-image,
threshold selection, equivocation and slashing, unavailability and rotation,
onboarding and exit, and an honest account of what threshold attestation does
not prove. Key words MUST, MUST NOT, SHALL, SHOULD and MAY follow RFC 2119. The
implementation is C17 under `src/guarantor/` and `src/paxeer/`; paths are
repository-root-relative.

## 1. Position in the system

LayerX runs one active sequencer. Guarantors are independent operators who
re-derive the sequencer's work from first principles and stake bonded capital on
the result. Paxeer holds custody and accepts a checkpoint only when a threshold
of guarantors has signed it.

A guarantor is **not** a validator in a BFT consensus. It proposes nothing,
orders nothing and votes on no fork. It answers exactly one question per
checkpoint: *does this state root follow deterministically from data I hold in
full?* It may answer yes, answer no, or stay silent — and each of those has
distinct, specified consequences.

A guarantor MUST be operationally independent of the sequencer and of every
other guarantor: separate legal entity, separate keys, separate infrastructure
provider, separate network path. Co-located guarantors defeat the entire
security argument and MUST be rejected at onboarding.

## 2. The six duties

| # | Duty | Requirement |
|---|---|---|
| 1 | Download the complete batch | Fetch all five DA sections — `ACTIVITIES`, `RECEIPTS`, `ORACLE`, `STATE_DIFF`, `RECOVERY` — and recompute every `chunk_root` and the manifest root. Header-only following is forbidden. |
| 2 | Verify every signature | Every activity envelope signature, every authority proof in the delegation chain, every oracle publisher signature, and the sequencer's batch signature. No sampling, no trusted-cache shortcuts across batches. |
| 3 | Replay every transition | Execute all activities in sequence order through the same deterministic state machine, at the `protocol_version` named in the header, using only `BatchHeader.timestamp` as the clock. |
| 4 | Recompute all roots | Independently derive `resulting_state_root`, `activity_merkle_root`, `receipt_merkle_root`, `event_merkle_root`, `oracle_root` and `data_availability_root`, and compare to the header. |
| 5 | Store the availability data | Persist the sections for their guarantor retention periods — finality plus 90 days for `ACTIVITIES`, `RECEIPTS`, `ORACLE` and `RECOVERY`, plus 30 days for `STATE_DIFF` — and serve any chunk on request. A guarantor that cannot serve it MUST NOT attest that it holds it. |
| 6 | Sign only on full agreement | Attest only if duties 1-5 succeeded for **every** batch in the checkpoint range. Any mismatch means refuse, publish the disagreement and escalate. |

Duty 6 is absolute. There is no "close enough", no majority-following and no
signing on another guarantor's word. A guarantor that signs a root it did not
itself compute has committed the only fraud this design cannot survive.

## 3. Machine and bandwidth expectations

Sized for the v1 target of 4000 activities per second sustained, 250 ms batches,
2400-batch checkpoints, average canonical activity 512 bytes.

| Quantity | Derivation | Value |
|---|---|---|
| Activity ingest | 4000/s x 512 B | ~2.0 MB/s |
| DA object rate | ingest + receipts + events + diffs, ~2.2x | ~4.5 MB/s |
| Sustained inbound | DA + gossip overhead | 50 Mbit/s minimum |
| Provisioned link | serve DA to peers and agents | 1 Gbit/s recommended |
| Signature verification | 4000 Ed25519/s plus authority chains, ~12000 verifies/s | 8 dedicated cores |
| Storage growth | 4.5 MB/s x 86400 | ~390 GB/day |
| 90-day retention | above x 90 | ~35 TB usable NVMe |
| Replay headroom | must replay faster than production | >= 2x real time |

Minimum: 16 physical cores, 64 GB ECC RAM, NVMe with `fsync` durability and
power-loss protection, 35 TB usable, 50 Mbit/s committed. Recommended: 32 cores,
128 GB ECC, 1 Gbit/s, plus an archive tier.

A guarantor MUST run the reference C17 implementation or an independently
written conformant one; running a byte-copy of the sequencer's binary with the
same bug is worth far less than the diversity the bond is paying for. Operators
SHOULD disable turbo-frequency-dependent code paths, MUST build without fast
math, and MUST verify against `tests/vectors/` before activation.

## 4. Bonding and the economic security argument

Each guarantor locks a bond `B` on Paxeer before activation. The bond is
slashable for equivocation and for attesting to data it does not hold.

The attack that matters: a colluding sequencer plus `T` guarantors sign a
checkpoint with a fabricated state root that credits attacker accounts, wait out
the challenge window, and withdraw. The honest minority can prove nothing on
Paxeer in v1, because Paxeer verifies signatures and thresholds, not execution.

Naive bound: `T * B >= TVL`. With a 50M USDX TVL, `N = 7` and `T = 5`, that is
10M per guarantor — capital-inefficient to the point of being unusable.

The protocol therefore bounds the *extractable* amount per window instead of the
total custody:

```text
maximum extraction per challenge window
    = min( TVL , W )                       W = per-asset withdrawal rate limit
requirement:  T * B  >=  SAFETY_MARGIN * W        SAFETY_MARGIN >= 2
```

`W` is `GOV_REGISTER_ASSET.withdraw_rate_limit`, enforced by the LayerX state
machine at `BRIDGE_WITHDRAW_REQUEST` and independently re-enforced by the Paxeer
contract per settled checkpoint. Because a fabricated root cannot pay out faster
than `W`, and because equivocation evidence or a stalled honest minority halts
the chain within one or two checkpoints, the loss from a successful collusion is
bounded by `W` per window rather than by TVL.

Worked example: `W = 2M USDX/day`, `SAFETY_MARGIN = 2`, `T = 5` gives
`B >= 800k USDX` per guarantor — a viable number that still makes collusion
strictly loss-making for any attacker who cannot also break the rate limit.

Rules: `B` MUST be denominated in an asset Paxeer custodies, not in a token the
guarantors themselves control; a bond that falls below `B_min` after a slash
jails the member immediately; and `SAFETY_MARGIN`, `W` and `B_min` are
governance parameters that MUST be re-derived whenever TVL or throughput changes
materially. Honest disclosure: this is a bounded-loss argument, not a
no-loss-possible argument.

## 5. The attestation message and its pre-image

A guarantor signs one short statement per checkpoint, exactly as fixed by
`wire-format.md`:

```text
T = "LXP1/guarantor-attest"
B = checkpoint_id || u8 attest_flags
P = u8(len(T)) || T || B          /* the signing pre-image, 55 bytes */

attest_flags bit 0 REPLAYED : replayed every transition and matched every root
attest_flags bit 1 DA_HELD  : holds the complete activity, receipt, oracle,
                              state-diff and recovery data
```

Both bits MUST be set for a signature to count toward the threshold. An
attestation with `DA_HELD` clear is not a weaker attestation, it is an invalid
one: a valid root over unavailable data is precisely the failure this design
exists to prevent.

Nothing else needs to be signed, because `checkpoint_id` already commits to
`network_id`, `epoch`, `checkpoint_number`, `start_state_root`,
`end_state_root`, `batch_merkle_root`, `data_availability_root` and
`guarantor_set_id`. Signing a digest of a digest keeps the Paxeer contract cheap
and keeps the guarantor's exposure to exactly one unambiguous claim.

Two digests exist over the identical pre-image bytes `P`:

```text
LayerX side : H("LXP1/guarantor-attest", B) = SHA-256(P)
              used for gossip, deduplication and evidence indexing
Paxeer side : keccak256(P)
              signed with secp256k1; the contract recomputes P from the
              checkpointId and flags it already holds and calls ecrecover
signature   : 65 bytes r || s || v, low-s enforced, v in {27, 28}
```

Only the Paxeer-side signature enters a certificate, so one attestation has
exactly one on-chain encoding and malleability cannot manufacture a second.
Guarantor keys are used for this tag and no other; because every LayerX
structure is hashed under a different `LXP1/` tag, a guarantor key can never be
tricked into signing a batch header, an activity envelope or a grant.

Operator metadata such as the local wall-clock time of attestation is
deliberately outside `B`. If it were inside, one guarantor could produce two
valid signatures over the same checkpoint, and equivocation detection would have
to distinguish them from an actual conflict.

## 6. Threshold selection

`T` of `N`, with `T >= floor(2N/3) + 1`, `T >= LX_MIN_THRESHOLD` (3),
`N >= LX_MIN_GUARANTORS` (4), `N <= LX_MAX_GUARANTORS` (256).

| N | T | Colluders needed | Failures tolerated |
|---|---|---|---|
| 4 | 3 | 3 | 1 |
| 7 | 5 | 5 | 2 |
| 10 | 7 | 7 | 3 |
| 16 | 11 | 11 | 5 |

The trade-off is explicit. Raising `T` raises the collusion cost (`T * B`) and
the number of independent replays behind each root, and it lowers liveness: any
`N - T + 1` simultaneous outages halt checkpointing. Lowering `T` improves
uptime and cheapens the attack in the same step. Raising `N` improves both but
increases coordination latency, DA bandwidth fan-out and onboarding overhead.

Initial parameters: `N = 7`, `T = 5`. Two guarantors may be down without halting
checkpoints; five independent operators must collude to settle a false root; the
bonded collusion cost is `5B`.

The threshold is not a majority vote on truth. It is a bonded-cost floor. Four
honest guarantors that all disagree with the sequencer cannot force a correct
root to settle — they can only withhold and halt. Halting is the intended
behaviour.

## 7. Equivocation detection and slashing evidence

Equivocation is one guarantor key signing attestations over two different
`checkpoint_id`s whose bodies share `(network_id, epoch, checkpoint_number)`.
Because the signed statement is only `checkpoint_id || flags`, the evidence must
carry both **bodies**, from which the contract recomputes both ids itself.

Anyone may submit evidence. Guarantors MUST gossip every attestation they see
and MUST run a detector over the union.

```c
struct lxp_slash_evidence {
    uint8_t    kind;              /* 1 equivocation, 2 unavailability, 3 invalid-root */
    uint32_t   guarantor_id;
    struct lxp_checkpoint body_a, body_b;   /* kind 1: the conflicting pair */
    uint8_t    flags_a, flags_b;
    lxp_secp_sig_t sig_a, sig_b;
    lxp_h256_t challenged_da_root;          /* kind 2: unanswered probe        */
    uint32_t   section_id, chunk_index, challenge_deadline;
    lxp_h256_t disputed_batch_id;           /* kind 3: governance-adjudicated  */
};
```

Paxeer verification for kind 1 is entirely mechanical and contains no LayerX
business logic: recompute `checkpoint_id` for each body; require the two ids to
differ; require `network_id`, `epoch` and `checkpoint_number` to be equal;
recompute each pre-image `P` and `ecrecover` each signature; require both
recovered addresses to equal the address registered for `guarantor_id` in
`guarantor_set_id`; require both flag bytes to have both bits set; require the
evidence within `PARAM_SLASH_WINDOW_MS` (2592000000, 30 days). The contract
parses no activity, no receipt and no module state.

Consequences: the full bond is slashed, `SLASH_REPORTER_BPS` (1000) goes to the
submitter, the remainder to `system:insurance` via the Paxeer reserve; the
member is ejected immediately; every checkpoint that depended on that signature
to reach `T` is marked contested; if any contested checkpoint is still inside its
challenge window, settlement is frozen and governance must resolve it.

Kind 2 (unavailability) is proven by an unanswered DA challenge, per
`data-availability.md`: a challenger names `(batch, section_id, chunk_index)`
against a manifest root the guarantor attested to, and the guarantor must
publish the chunk bytes plus its Merkle path to `chunk_root` before
`challenge_deadline` (`LX_DA_CHALLENGE_WINDOW_SEC`, 3600). The contract verifies
the path and never parses the chunk. Silence is the offence, and a legal prune
is not a defence. Slash is partial — `UNAVAIL_SLASH_BPS` (500) — escalating to
ejection on repeat within an epoch. Kind 3 is not mechanically verifiable in v1:
it is an invalid-root claim adjudicated by governance under a timelock, and it
exists so that this failure mode is recorded rather than pretended away.

## 8. Unavailability, jailing and rotation

Missing an attestation is not equivocation. It is a liveness fault, tracked per
epoch as `missed / expected`.

| Condition | Consequence |
|---|---|
| Miss ratio > 5% in an epoch | warning event, published |
| Miss ratio > 20% in an epoch | jailed: excluded from `N` for threshold purposes, no rewards |
| Jailed twice in a rolling 30 days | forced exit through the normal unbonding queue |
| Bond below `B_min` | immediate jail until topped up |

A jailed member's signatures are not counted toward `T`, and `N` for threshold
computation is the count of active, non-jailed, fully bonded members. Jailing
therefore raises the relative burden on the rest; if active `N` would fall below
`LX_MIN_GUARANTORS`, checkpointing halts rather than continuing at a weakened
threshold. This is
the same safety-over-liveness rule as section 10 of `checkpointing.md`.

Rotation, set changes and threshold changes go through `GOV_SET_GUARANTOR_SET`
(activity `0x0707`), and take effect on LayerX only at an epoch boundary and
only after the identical set is registered on Paxeer. If the two registries
disagree, checkpoint acceptance halts. There is no partial or racing rotation.

## 9. Onboarding and exit

Onboarding:

1. Register a secp256k1 attestation key and an operator identity on Paxeer, and
   post the bond. The bond is locked from this moment.
2. Sync from genesis or from a settled checkpoint, then replay forward and
   reproduce every root exactly. Publish the reproduced roots for the last
   `PARAM_ONBOARD_PROOF_BATCHES` (10000) batches.
3. Serve a live DA sampling challenge round without a miss.
4. Governance enacts `GOV_SET_GUARANTOR_SET` including the member.
5. Activation occurs at the next epoch boundary, never mid-epoch, and never
   before `PARAM_ACTIVATION_DELAY_MS` (86400000) has elapsed since step 4.

Exit:

1. Signal exit on Paxeer. The member keeps attesting until its removal epoch.
2. Governance removes it at an epoch boundary; the set and the threshold are
   recomputed and re-registered.
3. Unbonding runs for `PARAM_UNBOND_MS` (2764800000, 32 days), chosen to
   strictly exceed `PARAM_CHALLENGE_WINDOW_MS` (1 day) plus
   `PARAM_SLASH_WINDOW_MS` (30 days) measured from the member's last
   attestation, so a departing member's bond is still slashable for everything
   it signed.
4. DA retention obligations survive exit for the full retention period; the bond
   is not released while an unanswered DA challenge is outstanding.

An exit that would drop active `N` below `LX_MIN_GUARANTORS`, or below what
`T` requires, is queued, not executed. Custody safety outranks an operator's
schedule.

## 10. What threshold attestation does not prove

Stated plainly, because the design depends on nobody being confused about it:

- **It is not a validity proof.** `T` signatures mean `T` bonded operators claim
  they replayed the batch and got the same root. If they collude, or run the
  same buggy binary, or all trust one compromised dependency, a wrong root
  settles and Paxeer accepts it. There is no cryptographic object in v1 that
  makes an invalid state transition unrepresentable.
- **It is not a fraud proof.** An honest minority cannot force a rollback on
  Paxeer. It can withhold signatures, halt the chain, publish its own roots and
  escalate to governance. Those are social and economic remedies with a bounded
  window, not automatic ones.
- **It does not bound loss to zero.** It bounds loss to roughly the withdrawal
  rate limit inside a challenge window, backed by `T * B`.
- **It says nothing about censorship.** Guarantors verify what was included.
  They cannot prove what was excluded. Emergency exit is the only real answer.
- **It does not certify off-chain facts.** Oracle observations and
  tool-execution attestations are final as recorded data, never as truth about
  the world.
- **Correlated failure is the real risk.** One cloud region, one distro image,
  one libc bug, one popular ops playbook: independence must be audited at
  onboarding and re-audited, or `T` independent signatures are one signature
  wearing five hats.

## 11. Upgrade path to validity proofs

The activity protocol does not change. Determinism, canonical encoding,
integer-only arithmetic, batch-supplied timestamps and the fixed Merkle
construction already give the transition function the properties a prover needs.
The migration is confined to the checkpoint acceptance path:

1. **Prover as an optional service.** A prover produces a succinct proof that
   `previous_state_root -> resulting_state_root` is the correct execution of the
   committed `activity_merkle_root` under `protocol_version`. Nothing in
   `activity-types.md` moves.
2. **Certificate extension.** `lxp_checkpoint_certificate` gains an optional
   `validity_proof` field. Old certificates stay valid; the field is additive
   and version-gated.
3. **Dual acceptance.** Paxeer accepts a certificate carrying a verified proof
   *or* one carrying `T` attestations. Guarantors keep attesting during this
   phase and the proof is checked against their conclusion, which is how prover
   bugs get caught before anyone relies on them.
4. **Proof-primary.** Once proofs are reliable, a valid proof alone suffices for
   the state root, `T` drops toward a data-availability-only threshold, and the
   challenge window shortens because validity no longer depends on the window.
5. **Guarantors persist for DA.** Even with proofs, someone must hold and serve
   the data. Duties 1 and 5 of section 2 remain, with a smaller bond sized to
   availability failure rather than to custody theft.

Every step is a Paxeer-contract and certificate change. No activity type, no
receipt field, no state-machine rule and no agent-facing signature changes. That
is the whole point of specifying the checkpoint interface separately from the
activity protocol.

## 12. Constants and conformance

| Constant | Value |
|---|---|
| `LX_MAX_GUARANTORS` | 256 |
| `LX_MIN_GUARANTORS` | 4 |
| `LX_MIN_THRESHOLD` | 3 |
| `SAFETY_MARGIN` | 2 |
| `SLASH_REPORTER_BPS` | 1000 |
| `UNAVAIL_SLASH_BPS` | 500 |
| `PARAM_SLASH_WINDOW_MS` | 2592000000 |
| `PARAM_UNBOND_MS` | 2764800000 |
| `PARAM_ACTIVATION_DELAY_MS` | 86400000 |
| `PARAM_ONBOARD_PROOF_BATCHES` | 10000 |
| `PARAM_DA_RETENTION_DAYS` | 90 |

Under `tests/guarantor/` an implementation MUST demonstrate: independent replay
producing byte-identical roots on a different architecture from the sequencer;
refusal to attest on an injected root mismatch; equivocation detection and a
Paxeer slash executed from generated evidence; a DA sampling challenge answered
and, in a fault case, missed and slashed; a jail-and-rotate cycle across an epoch
boundary; a halt when active `N` falls below the minimum; and attestation digest
vectors that the C17 signer and the Paxeer contract agree on byte for byte.
