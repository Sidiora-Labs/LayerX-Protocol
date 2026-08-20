## How to execute this plan

This plan builds the complete LayerX Platform. Waves 1 through 8 are the Human
Control Plane, carried verbatim — task ids, statuses and dependencies intact —
from the human-interface specification this plan supersedes: the custody
service, the intent compiler, the approval seam in the agent contract, the
Paxeer boundary client, the explorer index, the human-api contract, and one web
application with two native shells. Waves 9 through 13 add the four platform
pillars: the developer platform, LayerX Programs, the interoperability gateway
and the multichain surface, closed out by an all-up platform qualification.

**Two rules govern every task below.** The surface exposes exactly five ideas —
log in, add money, move money, manage agents, see what happened — and "Done" is
rendered only when a verified LayerX receipt or a Paxeer finality proof backs
it. A task whose output would let a screen claim something the protocol did not
do is not done, however green its `verify_cmd` looks.

**The plane never grows a second write path.** Every mutation is a typed intent
compiled by `layerx-intents`, disclosed, signed by the custody service and
submitted through the agent layer's existing pipeline. The only agent-contract
change this feature makes is the additive `approval.*` module, built in wave 1.

**Work is driven by `cg spec next`.** Run it; it resolves the wave graph and
returns the next eligible task. Exactly one task is `in_progress` at a time.
A task is `done` only when its `verify_cmd` genuinely passes on real code paths:
journey suites run against a real node, a real `layerx-agentd` and a Paxeer
test network — a test that passes against a fake proves nothing here.

**Waves express real dependencies.** The approval seam and the workspace gates
come first; intents and the contract before custody; custody and the Paxeer
boundary before journeys; journeys before surfaces; everything before
qualification. Tasks within a wave are independent and may be worked in any
order.

**The new pillars respect the plane's law.** The SDKs, middleware, hosted
surfaces and adapters hold no authority and render nothing beyond their backing
evidence; Programs arrive as a module with every monetary effect forced through
402LXP; mirrors are archives and custody stays on Paxeer alone. A pillar task
that would bend one of these to ship faster is a spec defect, not a shortcut.

## The thirteen waves

| Wave | Delivers |
|---|---|
| 1 | Foundations: the `human/` workspace, dependency and boundary policy, the copy catalog and its lint, design tokens and the UI rule gates, the test and state-matrix harness — and the `approval.*` module in the agent contract with regenerated SDKs. |
| 2 | `layerx-intents` — the typed intent vocabulary, compilation to canonical bytes, golden vectors, the disclosure round-trip gate, the single-payload-authority gate — and the `human-api` schema with its generated TypeScript client. |
| 3 | Custody: the principal-scoped store, passkey auth and step-up, the KMS keystore and custody signer, onboarding orchestration, session-key provisioning, the wallet-binding journey, the audit chain — and `layerx-paxeer-client` for finality, deposits, claims and exits. |
| 4 | Journeys: the route resolver, the durable journey engine, deposit, withdrawal and claim, emergency exit, move-money — and the managed-agent journeys plus approvals and notifications in the service. |
| 5 | Evidence and reads: the rebuildable explorer index, the unified activity feed, entry detail, statement and evidence exports, explorer lookups and the verifier. |
| 6 | The application foundation: the two planes, SSR-safe shell selection, the component kit in both shells, the state-matrix machinery and error surfaces, the accessibility and theme foundation, the performance machinery. |
| 7 | The application surfaces: onboarding, home and move-money, the custody journeys, agents, approvals and notifications, activity — plus settings, security, wallet binding, support and the public explorer plane. |
| 8 | Qualification: every journey against the real stack in both shells, the hostile-plane no-fabrication suite, the journey fault-injection matrix, the UI, copy and accessibility gates, performance and soak, the usability gate, and the release-gating qualification report. |
| 9 | The developer platform: the `platform/` workspace and release pipeline, seven schema-generated SDKs with the cross-language parity suite, buyer/seller/merchant/agent middleware, the framework and mobile integrations, the CLI, the real-transition-function emulator, the hosted testnet, faucet, gateway, webhooks and dashboards, one-command MCP and A2A installation, executable docs, the reference applications, and the ten-line and five-minute benchmark gates. |
| 10 | LayerX Programs: the `programs/` workspace and deterministic WASM engine, metering, the capability ABI and namespaced storage, the programs module registered in the core, determinism and fuzz proofs, permissionless deploy/upgrade/migration, the registry with source verification, deprecation rules, the 402LXP monetary law for guest code, composition and reentrancy rules, the hostile-program gauntlet, the Rust program SDK plus two more languages, and the EVM/Solana/CosmWasm porting kits. |
| 11 | The interoperability gateway: the `interop/` workspace and edge-only gateway core, x402 v2 buyer/seller/facilitator across HTTP, MCP and A2A, AP2 mandates, UCP commerce, Visa Trusted Agent credentials, Ethereum and Solana migration tooling, the card/bank/RTP adapter interfaces, and portable two-way receipt and mandate verification. |
| 12 | The multichain surface: the batch mirror publisher to Ethereum and Solana, mirror-only verification in the explorer and SDKs, the domain-tagged claim vocabulary with Paxeer as the sole valid domain, the market-maker ramp toolkit with its reference ramp, and the external-custody labelling gates. |
| 13 | Platform qualification: the adoption benchmark gates, the programs security qualification, the interop conformance matrix, the multichain verification gates, and the release-gating platform report. |

Waves 1 through 3 are this feature's trust spine: the seam that lets humans veto
agent spending, the compiler that makes disclosures provable, and the custody
that keeps signing power where it belongs. Everything above them either moves
money or claims something about money, and both depend on the spine being real.
The pillar waves stand on that spine: the developer platform packages the
plane's honest paths, Programs extend the protocol without touching the kernel,
the gateway translates the outside world into typed intents, and the multichain
surface makes the evidence portable while custody never leaves Paxeer.
