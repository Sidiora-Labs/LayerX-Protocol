# Generated SDK guarantees

This file is generated from `agent/schema/agent-api`. Do not hand-edit.

| Restriction | Enforcement | Exact statement |
|---|---|---|
| `ProtocolBudget` | `protocol_enforced` | Enforced by the LayerX protocol state machine. |
| `DaemonLimit` | `daemon_enforced` | Bypassing the daemon bypasses this limit. It is not equivalent to a protocol budget. |

Every authoritative read carries the full verification-level lattice and freshness coordinates. `Unknown` remains a first-class submission state. Mutations retain caller-supplied idempotency keys, and protocol result codes retain their exact signed integer value.
