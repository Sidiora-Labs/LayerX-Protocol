# LayerX Human Interface

The human control plane reports protocol state only from verified LayerX receipts or verified Paxeer finality evidence. A movement or mutation is never presented as Done from a local assertion, and an unknown outcome remains still checking until receipt lookup resolves it.

The product exposes five ideas: log in, add money, move money, manage agents, and see what happened. Protocol mechanisms stay behind those ideas, and the application never asks a person to choose or understand an internal route.

The Rust workspace contains the human service, the sole typed-intent payload authority, the Paxeer custody-boundary client, and the rebuildable explorer index. They consume the agent-layer crates through path dependencies and reach the LayerX core only through the agent contract.
