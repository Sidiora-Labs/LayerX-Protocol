## How to execute this plan

This plan is the build order for the LayerX protocol rewrite: the canonical
activity, execution and accounting layer for autonomous agents. Paxeer supplies
custody, checkpoint finality, economic guarantees and dispute settlement, and
never processes ordinary agent activity. 402LXP is the single financial doorway
— every monetary effect in the entire system compiles into one or more
authenticated balance transfers, and no module writes a balance directly.

**Work is driven by `cg spec next`.** Do not pick tasks by reading down the list
and choosing what looks interesting. Run `cg spec next`; it resolves the wave
graph, filters out anything whose dependencies are unmet, and returns the next
eligible task. That is the task you work. If `cg spec next` returns nothing, the
plan is either complete or blocked — diagnose the block, do not invent work
outside the plan.

**Exactly one task is `in_progress` at a time.** Before starting, set that
task's status to `in_progress` in `spec.kvx`. Before starting anything else,
that task must reach `done` or be returned to `pending` with an honest note
about why. Two tasks in flight means neither has a trustworthy state, and the
wave graph stops describing reality.

**A task is `done` only when its `verify_cmd` genuinely passes.** The
`verify_cmd` is the definition of completion, not a formality appended
afterwards. Run it, watch it pass, on the real code path, with real types. A
`verify_cmd` that passes because the thing it exercises was stubbed out is a
failure that has been dressed as a success. If the command cannot pass yet, the
task is not done — say so, leave it `pending`, and report the honest partial.

**No task may be marked done on the strength of a stub or a fake.** This is the
hardest rule in the plan and it has no exceptions. Do not write placeholder
implementations, mock doubles, or short-circuit branches to turn a check green.
Do not hardcode an expected root, a signature, or a balance to satisfy an
assertion. For a consensus-critical protocol, a fake that reaches `done` is
strictly worse than an unfinished task: the plan then contains a lie that later
waves will build on top of, and the divergence surfaces during replay, when it
is expensive.

**Waves express real dependencies, not scheduling preference.** A task sits in
wave N because it genuinely cannot be built correctly before wave N-1 exists.
The codec must be canonical before anything hashes; commitments must be stable
before state roots mean anything; the transfer kernel must exist before any
module can move value. Tasks within one wave are independent of each other and
may be worked in any order. Do not pull work forward across a wave boundary to
"unblock" yourself — that is the signal that a dependency is real.

## The sixteen waves

| Wave | Delivers |
|---|---|
| 1 | Foundations: C17 build, sanitizer and fuzz harnesses, error model, checked fixed-width 128/256-bit arithmetic, arena allocation. |
| 2 | Canonical binary codec: one unambiguous byte encoding per type, round-trip and non-canonical-rejection tests, parser fuzz targets. |
| 3 | Crypto and commitments: Ed25519 agent signatures, secp256k1 Paxeer-facing certificates, domain-separated hashing, Merkle trees. |
| 4 | Identity and authority: DID accounts, primary and session keys, capability grants, delegated limits, revocation, expiry, sequences, rotation. |
| 5 | State, storage and log: account and state trees, the append-only activity log as authority, rebuildable SQLite projections, crash recovery. |
| 6 | Activity kernel: the activity envelope, admission, global ordering, fee metering, versioned transition dispatch, events and receipts. |
| 7 | 402LXP transfer kernel: `lxp_apply_transfer`, atomic transfer sets, conservation, non-negativity, the single deterministic balance writer. |
| 8 | `SEND` and `RECEIVE`: the two public financial operations, signed payer grants, idempotency keys, expiry and context binding. |
| 9 | `asset` module: the account and subaccount namespace, asset registry, transfers, deposit and withdrawal accounting against the reserve mirror. |
| 10 | `escrow`, `budget` and `stream`: holds, capture, release, timeout, recurring allowances, delegated spending and metered payments. |
| 11 | `service` module: the complete agent work lifecycle — offers, acceptances, task commitments, tool execution attestations, deliveries, acceptances and disputes. |
| 12 | `perps` and oracle intake: markets, orders, positions, funding, liquidation, and Crossverse prices entering only as signed oracle activities. |
| 13 | `governance`, fees and emergency modes: protocol parameters, staged rollout, fee treasury routing, emergency controls. |
| 14 | Sequencer, batches, replicas and data availability: batch assembly, all roots, distribution to replicas, DA commitments and retrieval. |
| 15 | Guarantors, checkpoints and `bridge`: independent full replay, threshold attestation, equivocation evidence, Paxeer settlement and exits. |
| 16 | Genesis, migration and qualification: the explicit genesis manifest, reserve reconciliation, shadow replay against the legacy system, cross-architecture byte-identical replay, conformance suites, optional JSON gateway. |

Waves 1 through 8 are the protocol spine. Nothing above wave 8 is meaningful
until the transfer kernel and its two public operations are real, verified and
fuzzed, because every later wave ultimately expresses itself as transfers
through them.
