## Engineering ground rules

These apply to **every** task in this plan. They are not per-task acceptance
criteria to be negotiated; they are the standing conditions under which any task
may reach `done`. A task whose output violates one of these is not done, however
green its `verify_cmd` looks.

- **C17 only.** The protocol runtime is C17. No C++, Go, Rust, C# or JavaScript
  anywhere in `src/`, `include/layerx/` or `cmd/`. Tooling that never links into
  the runtime may use other languages; consensus code may not.
- **No floating point.** No `float`, `double`, `long double`, no libm, no
  floating literals in consensus-critical paths. Prices, funding rates,
  interest, fees and stream rates are integer or fixed-point with an explicitly
  specified scale and an explicitly specified rounding direction.
- **Checked integer arithmetic.** Every add, subtract, multiply and shift on a
  value that can influence state uses the checked fixed-width helpers, including
  the 128/256-bit implementation. Overflow is a defined, testable failure that
  rejects the activity — never a wrap, never undefined behaviour, never a
  saturating silent clamp.
- **Canonical encoding.** Exactly one valid byte encoding per value. Encoders
  emit it; decoders reject every non-canonical alternative rather than
  normalising it. Map and set iteration order is stable and specified. Anything
  that is hashed, signed or committed to is encoded canonically first.
- **Single deterministic state writer.** One thread, one code path, writes
  state. Worker threads exist only for signature verification, networking,
  indexing and other non-consensus work, and they never mutate state. SQLite
  indexes are rebuildable projections of the append-only activity log, and the
  log — not the index — is the authority.
- **No clock or network access inside execution.** Transition functions may not
  call `time()`, `gettimeofday`, `rand`, any HTTP client, any DNS resolver, or
  read environment or filesystem state. Time is the deterministic timestamp
  supplied by the batch. External observation, including every Crossverse price,
  enters only as a signed oracle activity whose exact payload becomes part of
  replayable history.
- **Every monetary effect through 402LXP only.** There is exactly one balance
  mutation primitive, `lxp_apply_transfer`, plus its atomic multi-leg form
  `lxp_apply_transfer_set`. No module — not `escrow`, not `perps`, not
  `governance`, not `bridge` — writes a balance directly, and no direct database
  balance update is permitted. If a feature implies value movement, it compiles
  into authenticated transfers or it does not happen.
- **Sanitizer-clean builds.** The test suite passes clean under
  AddressSanitizer, UndefinedBehaviorSanitizer and, where the task touches
  threading, ThreadSanitizer. A sanitizer report is a failure, not a warning to
  triage later.
- **Fuzz targets for every parser.** Every decoder, envelope reader, signature
  parser and wire-format entry point ships with a fuzz target under `fuzz/` in
  the same task that introduces it. A parser without a fuzz target is an
  unfinished parser.
- **Deterministic replay tests.** Replaying the same activity history must
  produce byte-identical state roots, receipts and event roots, across runs,
  machines and architectures. Any task that changes execution adds or extends a
  replay test proving it.
- **The legacy Go implementation is a read-only behavioural reference.**
  The external legacy implementation stays untouched. It may be consulted to
  understand proven behaviour — DID-native accounts, escrow-bounded spending,
  fully reserved assets, signed receipts, idempotent execution, crash recovery,
  deterministic perps arithmetic and fail-closed market data. Never translate
  it file by file. Its PostgreSQL schema is not the protocol definition, its
  HTTP endpoints are not the canonical wire protocol, its in-memory auth
  challenges are not authority state, and its background timing is not
  consensus behaviour.

## Locked decisions

These supersede anything ambiguous in `docs/00-source-brief.md`.

- **Implementation language:** C17. No C++, Go, Rust, C# or JavaScript in the
  protocol runtime.
- **Implementation repository root:** this repository. The brief proposed a
  separate implementation directory; that is superseded so the spec and the
  code live in one Codify-indexed graph. Every file path in this plan is
  relative to the repository root — for example
  `src/codec/lxp_codec.c`, never an absolute path.
- **Wire format:** a canonical binary protocol is the consensus format. A
  JSON/HTTP gateway is optional, is a convenience surface only, and never
  defines consensus behaviour.
- **Sequencing and attestation:** one active sequencer initially, plus a bonded
  quorum of independent Paxeer guarantors that each download the full batch,
  verify every signature, replay every transition, recompute every root and
  store the required availability data before attesting. Threshold attestation
  is an economic guarantee backed by bonds and slashing for equivocation — it is
  described honestly as such, and is not equivalent to a validity proof.
- **Authority of record:** the append-only activity log is authoritative. SQLite
  indexes and materialised state are rebuildable projections and may be dropped
  and reconstructed at any time.
- **Paxeer scope:** custody, deposits and withdrawals, checkpoint registration,
  guarantor bonds, attestations, slashing, emergency exits, dispute resolution
  and final settlement. Paxeer contracts understand no LayerX business logic —
  not perps orders, not service agreements, not ordinary transfers. An ordinary
  LayerX action never requires a Paxeer transaction.
- **v1 activity vocabulary:** the **complete agent work lifecycle**, not only
  economically meaningful actions. This resolves the open scope question at the
  end of the brief. Task commitments, tool execution attestations, deliveries,
  acceptances and disputes are first-class ordered and attested activities in
  the `service` module. They carry **no direct monetary effect**: any value
  movement they imply still executes through 402LXP transfers, under the same
  authorization, atomicity and conservation rules as every other transfer.
