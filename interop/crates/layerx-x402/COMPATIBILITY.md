# x402 compatibility matrix

The adapter is pinned to x402 v2 revision `7d5363a6d51750dc246041f2b0ed5819dd46a0d7`. Buyer, seller and facilitator core values are transported without semantic changes across HTTP, MCP and A2A. HTTP uses the normative `PAYMENT-REQUIRED`, `PAYMENT-SIGNATURE` and `PAYMENT-RESPONSE` headers for payment signals; facilitator endpoint messages are JSON bodies on all transports.

| Transport | Buyer | Seller | Facilitator | Local matrix |
|---|---:|---:|---:|---:|
| HTTP | yes | yes | yes | `tests/transports.rs` |
| MCP | yes | yes | yes | `tests/transports.rs` |
| A2A | yes | yes | yes | `tests/transports.rs` |

The local matrix proves strict encode/decode parity, validation and transport-independent settlement identity. It does not claim upstream reference-implementation conformance or live-service settlement. Those results may be added only from the pinned upstream vector corpus and an independently operated service; neither is embedded or simulated here.
