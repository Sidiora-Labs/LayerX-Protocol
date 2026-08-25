# Paxeer Core Documentation

Technical documentation for Paxeer's core protocol implementation, written from the source code in `paxeer-network/`.

## Pages

- [Consensus](consensus.md) — Byzantine Fault Tolerant consensus, block proposal, voting, ABCI integration
- [Engine](engine.md) — Transaction execution, executor, StateDB bridge, gas metering
- [EVM](evm.md) — Ethereum Virtual Machine integration on chain ID 125, address association, receipts, pointer contracts
- [Modules](modules.md) — Paxeer-specific modules (epoch, mint, oracle, tokenfactory)
- [Storage](storage.md) — PaxDB architecture (SC, SS, ledger DB, WAL)
- [Precompiles](precompiles.md) — Custom EVM precompiles exposing Cosmos functionality
- [WASM](wasm.md) — CosmWasm integration, wasmbinding, runtime

## Source

All documentation extracted from `paxeer-network/` (consensus, engine, modules, storage, precompiles, wasm, wasmbinding).
