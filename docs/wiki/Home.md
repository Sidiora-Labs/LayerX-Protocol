# LayerX Ecosystem

Welcome to the LayerX Ecosystem documentation. This repository is the canonical Sidiora Labs monorepo for LayerX and the Paxeer Network.

## What is LayerX?

LayerX is a deterministic execution and accounting network built for autonomous agents. It provides infrastructure for agent identity, delegated authority, payments, budgets, escrow, streams, trading, and settlement as one coherent system.

Ordinary agent activity is executed and ordered inside LayerX. Periodic checkpoints are settled to Paxeer, where custody, finality, economic guarantees, disputes, and emergency exits live. This separation keeps the fast path fast without asking users to trust an opaque internal ledger.

## What is Paxeer?

Paxeer is the settlement layer that handles custody, checkpoint registration, guarantor bonds, challenges, withdrawals, and emergency exits for LayerX. While LayerX handles thousands or millions of activities, Paxeer registers periodic checkpoints and provides the custody guarantee.

The Paxeer Network node source is co-located in this repository under `paxeer-network/` as part of the LayerX ecosystem monorepo. Co-location keeps the protocol, settlement network, and their automation auditable in one place while preserving separate build, release, and trust boundaries.

## What is LayerX Programs?

Programs is a programmable surface within LayerX that allows developers to deploy economic applications with capability-based kernel APIs, namespaced storage, explicit metering, and versioned ABIs. Programs is not a separate module ID—it is a runtime surface where guest code runs inside the `programs` module's namespace, with every monetary effect forced through 402LXP and no program ever holding direct balance-writing authority.

## Development status

LayerX is currently in **limited beta** (opening September 7, 2026). The protocol is source-available for inspection and security review under a temporary license that does not grant deployment rights.

- Source code is open for inspection
- Limited beta opens September 7, 2026
- No public RPC endpoint yet
- Inspection-only license until qualification completes

Sidiora Labs intends to publish LayerX under an open-source license after protocol development and release qualification are complete.

## Key properties

Three rules sit at the center of LayerX:

1. **One canonical history.** Every accepted or failed activity receives a global sequence. State roots are chained per activity, not only per batch.
2. **One financial doorway.** `402LXP` is the only component allowed to write balances. Protocol modules produce validated transfer sets rather than mutating funds themselves.
3. **One reproducible result.** Consensus-critical execution excludes floating point, local clock decisions, database iteration order, and other sources of nondeterminism.

## What LayerX supports

| Area | Responsibilities |
| --- | --- |
| Identity and authority | Agent DIDs, primary and session keys, scoped capability grants, rotation, recovery, revocation, and expiry |
| Money movement | Authenticated sends and receives, asset accounts, deposits, withdrawals, and reserve accounting |
| Spending controls | Holds, escrow, recurring budgets, delegated limits, approvals, and metered streams |
| Agent commerce | Offers, commitments, tool-execution attestations, delivery, acceptance, and disputes |
| Markets | Oracle intake, order books, positions, funding, margin, liquidation, and insurance accounting |
| Network operation | Sequencing, replicas, batch construction, data availability, replay, fees, and metering |
| Settlement | Guarantor attestations, checkpoint registration, custody reconciliation, claims, and emergency exits |

## Fees

LayerX charges a base fee of approximately ½¢ per 5,000 µUSDX of activity volume for network operation, sequencing, and data availability.

## Repository structure

This monorepo contains:

- **LayerX protocol** (`src/`, `include/`, `agent/`, `human/`, `platform/`, `programs/`, `interop/`, `contracts/`, `spec/`)
- **Paxeer Network** (`paxeer-network/`) with independent build, release tags (`paxeer-network/vX.Y.Z`), and trust boundaries

Co-location keeps the ecosystem auditable while preserving separate deployment authority. See `docs/MONOREPO.md` for details.

## Resources

- [Protocol design](https://github.com/Sidiora-Labs/LayerX-Protocol/blob/main/spec/layerx-protocol/design.md)
- [Contributing guide](https://github.com/Sidiora-Labs/LayerX-Protocol/blob/main/CONTRIBUTING.md)
- [Security policy](https://github.com/Sidiora-Labs/LayerX-Protocol/blob/main/SECURITY.md)
- [Qualification documentation](https://github.com/Sidiora-Labs/LayerX-Protocol/blob/main/docs/QUALIFICATION.md)
- [Monorepo layout](https://github.com/Sidiora-Labs/LayerX-Protocol/blob/main/docs/MONOREPO.md)

---

LayerX is developed by [Sidiora Labs](https://github.com/Sidiora-Labs).
