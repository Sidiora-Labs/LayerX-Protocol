# Interop

The interop plane translates at the edge. Adapters speak someone else's protocol on one side and hand typed, verified results to LayerX on the other - and they are structurally prevented from constructing LayerX payload bytes themselves. The plane's payload authorities are the only constructors, which is why an adapter bug cannot become a monetary bug.

## Adapters

| Adapter | Speaks | Direction |
|---|---|---|
| `layerx-x402` | x402 v2, pinned to an exact upstream spec commit and its SHA-256 | Both - buyer, seller and facilitator |
| `layerx-ap2` | AP2 mandates over JOSE | Inbound mandate verification, outbound signed evidence |
| `layerx-ucp` | UCP shopping checkout and order, pinned to a dated spec revision | Inbound |
| `layerx-visa-tap` | Visa Trusted Agent Protocol HTTP message signatures | Inbound |
| `layerx-fiat` | Certified fiat rails via opaque provider tokens | Both |
| `layerx-migrate` | Ethereum and Solana migration boundaries | Inbound |
| `layerx-mirror` | Ethereum and Solana publication of retrievable batch archives | Outbound |

Every adapter carries an `AdapterDescriptor` naming its id, its pinned specification and its conformance suite. Pinning is exact: `layerx-x402` names the upstream commit hash and the SHA-256 of the specification document it was written against. An upstream revision that has not been reviewed does not silently become the behaviour.

## Both directions are typed

Evidence leaves LayerX as a `PortableReceipt` - exact receipt bytes plus batch claims - which a counterparty verifies against an independently trusted batch authorisation. No node, gateway, daemon, database, clock or network connection is required to check it.

Evidence enters through `verify_external_evidence`. The adapter keeps ownership of its own protocol's cryptography and constraints; `layerx-portable` binds the adapter's typed result to the exact presentation and pinned specification that were verified. The result is that "this AP2 mandate was verified" is a statement about a specific presentation under a specific spec version, not a boolean.

## The gateway

`layerx-interop-gateway` is the one contract through which the interop plane reaches LayerX. It carries the principal, the trace and the translation request; it classifies translation status; and it redacts. Adapters do not reach past it.

## x402

x402 is the case most people meet first, because it is what the seller and buyer middleware speak over HTTP. The three headers are `PAYMENT-REQUIRED` on the `402`, `PAYMENT-SIGNATURE` on the retry, and `PAYMENT-RESPONSE` on the settled reply. The Rust crate exposes the same three roles - `Buyer`, `Seller`, `Facilitator` - for services that are not Node.

The relationship between the layers is worth stating plainly: x402 is the transport for the offer and the proof. It is not the settlement. The settlement is the LayerX move, and the thing that makes the settlement true is the receipt carried in the payment's extensions.

## Agent transports: MCP and A2A

Two more edges exist for the case where the counterparty is a model rather than a service. Both are served by the CLI and both expose exactly the same five tools.

```
layerx install mcp --environment testnet --key <key-id> --host claude
layerx mcp serve --environment testnet --key <key-id>

layerx install a2a --environment testnet --key <key-id> --listen 127.0.0.1:9433
layerx a2a serve --environment testnet --key <key-id> --listen 127.0.0.1:9433
```

`mcp serve` speaks the model context protocol over standard input and output. `a2a serve` publishes an agent card and a task interface on a loopback endpoint. `install` writes the host configuration and registers the transport in one command, so an agent host gains payment capability without anyone hand-editing a JSON config.

| Tool | Kind | Does |
|---|---|---|
| `balance.get` | read | Reads account balance material |
| `receipt.get` | read | Fetches exact receipt material for one receipt id |
| `activity.prepare` | write | Quotes a payment from source, destination, currency and amount |
| `activity.submit` | write | Commits a quote under a caller-supplied idempotency key |
| `activity.track` | read | Reads the current state of one committed journey |

`--read-only` serves only the read tools. It is not a prompt instruction or a policy note - the write tools are not in the surface at all, so a model that decides to spend money finds nothing to call. Every served tool declares its required scope, whether it mutates, and what evidence it produces, and every one of them passes the daemon's policy, capability, budget and audit gates.

Note the shape of `activity.submit`: `quote_id` and `idempotency_key`, both required. A model cannot commit a payment it did not quote, and cannot commit one without a key.

## Enforced by

| Capability | Layer | What that means here |
|---|---|---|
| Interop adapters hold no protocol authority | `protocol` | Adapters translate at the edge; payload authorities are the sole constructors of LayerX payload bytes. |
| Offline receipt verification | `protocol` | Portable receipts verify against an independently trusted batch authorisation. |
| Atomic settlement | `protocol` | A translated payment settles whole or not at all. |
| Replay refusal | `protocol` | Idempotency domains are separated per adapter and bound into the request digest. |
| Quote then commit | `service` | An x402 offer is accepted before it is paid. |
| Idempotent money moves | `service` | Fiat and migration boundaries derive keys from the request, not the attempt. |
| Capability attenuation | `agent-layer` | `--read-only` removes the write tools from the served surface entirely. |
| Agent tenancy isolation | `agent-layer` | A served transport is bound to one environment and one key. |
