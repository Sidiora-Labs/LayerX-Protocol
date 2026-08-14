# LayerX Protocol — Source Brief

> This is the architectural source brief from which the `layerx-protocol` spec
> was compiled. Local workspace locations have been replaced with repository-
> relative language for publication. It is provenance, not generated output.
> The normative artifacts are `spec/layerx-protocol/spec.kvx` and the documents
> in this `docs/` folder.

The clean framing is:

> LayerX is the canonical activity, execution, and accounting layer for autonomous agents. Paxeer provides custody, checkpoint finality, economic guarantees, and dispute settlement—but does not process ordinary agent activity.

## The new division of responsibility

### LayerX handles

- Agent identities and delegated authority.
- Activity ordering.
- Payments and balances.
- Holds, escrow, budgets, subscriptions and streaming payments.
- Service agreements, deliveries and attestations.
- Trading, positions, funding and liquidation.
- Deterministic state execution.
- Receipts and inclusion proofs.
- Data availability.
- Replay and state reconstruction.
- Sequencer or committee operation.
- Fees and resource metering.

### Paxeer handles

- Asset custody.
- Deposits and withdrawals.
- LayerX checkpoint registration.
- Guarantor bonds.
- Checkpoint attestations.
- Slashing for conflicting attestations.
- Emergency exits.
- Dispute resolution.
- Final settlement between LayerX and external assets.

An ordinary LayerX action should never require a Paxeer transaction. Thousands or millions of LayerX activities collapse into one periodic Paxeer checkpoint.

## Core protocol model

Every action becomes a canonical activity envelope:

```text
Activity {
    protocol_version
    network_id
    activity_type
    actor_did
    authority
    account_sequence
    timestamp_bound
    idempotency_key
    fee_limit
    payload_hash
    payload
    signature
}
```

LayerX deterministically processes it into:

```text
ActivityReceipt {
    activity_id
    global_sequence
    previous_state_root
    resulting_state_root
    activity_root
    result_code
    effects
    fee_charged
    batch_id
    sequencer_signature
}
```

There are no floating-point values, platform-dependent encodings or ambiguous JSON numbers in consensus-critical execution.

## Protocol layers

### 1. Identity and authority

Agent DIDs remain the native account identity.

The new protocol should add:

- Primary agent keys.
- Rotatable session keys.
- Capability grants.
- Delegated spending and trading limits.
- Revocation.
- Expiration.
- Per-action authority scopes.
- Account sequence numbers.
- Recovery and key rotation.
- Optional EVM payout-address bindings.

Authorization becomes part of the state machine, not HTTP middleware.

### 2. Activity kernel

The kernel should only understand universal concepts:

- Identities.
- Accounts.
- Assets.
- Authority.
- Nonces and sequences.
- Fees.
- State transitions.
- Events.
- Receipts.
- Checkpoints.
- Modules.

Payments, markets and service coordination should be protocol modules built on that kernel.

### 3. Native modules

Initial protocol modules should include:

- `asset`: balances, deposits, withdrawals and transfers.
- `escrow`: holds, capture, release, timeout and disputes.
- `budget`: recurring limits, allowances and delegated spending.
- `stream`: metered and time-based payments.
- `service`: offer, accept, work commitment, delivery and acceptance.
- `perps`: markets, orders, positions, funding and liquidation.
- `governance`: protocol parameters and emergency controls.
- `bridge`: Paxeer deposits, exits and checkpoint settlement.

Crossverse becomes an oracle/data adapter consumed by the perps module. It is not embedded into the LayerX kernel.

### 4. Deterministic execution

A LayerX node must produce identical results from identical activity history.

That requires:

- Canonical binary encoding.
- Integer-only arithmetic.
- Explicit overflow behavior.
- Stable map and set ordering.
- Fixed rounding rules.
- Versioned transition functions.
- Deterministic timestamps supplied by batches.
- No operating-system state inside execution.
- No external HTTP calls during state transition evaluation.

External observations such as Crossverse prices enter as signed oracle activities. Once accepted, their exact payload becomes part of the replayable history.

### 5. Activity batches

The sequencer assembles accepted activities into batches:

```text
BatchHeader {
    protocol_version
    network_id
    epoch
    batch_number
    first_sequence
    last_sequence
    previous_state_root
    resulting_state_root
    activity_merkle_root
    receipt_merkle_root
    event_merkle_root
    data_availability_root
    oracle_root
    timestamp
    sequencer_id
}
```

The full batch is distributed to LayerX replicas before it becomes checkpoint-eligible.

### 6. Paxeer guarantors

My recommended initial model is one active LayerX sequencer with multiple independent guarantors.

Each guarantor:

1. Downloads the complete batch.
2. Verifies every signature.
3. Replays every transition.
4. Recomputes all roots.
5. Stores the required availability data.
6. Signs the checkpoint only if everything matches.

Paxeer accepts a checkpoint after a defined guarantor threshold signs it.

This gives Paxeer an economic guarantee role without putting individual activities on Paxeer.

The guarantee must be described honestly: threshold attestations are not equivalent to a validity proof. Guarantors can be slashed for equivocation and must be sufficiently bonded. A later version can add validity proofs without changing the activity protocol.

### 7. Data availability

A state root without available activity data is insufficient.

A checkpoint should only be finalizable when guarantors attest that they possess:

- The complete activity batch.
- Receipts.
- Oracle inputs.
- State-diff material.
- Recovery metadata.

Agents must be able to retrieve and independently replay finalized history.

### 8. Settlement and exits

Paxeer contracts should hold assets but understand as little LayerX business logic as possible.

They should verify:

- A finalized checkpoint certificate.
- Membership or balance proofs against its state root.
- Withdrawal nullifiers.
- Guarantor signatures.
- Challenge windows.
- Emergency-exit eligibility.

The contract should not understand perps orders, service agreements or ordinary transfers.

## Pure-C implementation

I recommend C17 for the first reference implementation, with no C++, C#, Go, Rust or JavaScript in the protocol runtime.

Proposed structure:

```text
layerx-protocol/
├── spec/
│   ├── protocol.kvx
│   ├── threat-model.md
│   ├── wire-format.md
│   ├── state-machine.md
│   ├── activity-types.md
│   ├── checkpointing.md
│   ├── guarantors.md
│   ├── data-availability.md
│   ├── economics.md
│   └── migration.md
├── include/layerx/
├── src/
│   ├── protocol/
│   ├── codec/
│   ├── crypto/
│   ├── state/
│   ├── storage/
│   ├── network/
│   ├── sequencer/
│   ├── replica/
│   ├── guarantor/
│   ├── paxeer/
│   └── modules/
│       ├── asset/
│       ├── escrow/
│       ├── budget/
│       ├── stream/
│       ├── service/
│       └── perps/
├── cmd/
│   ├── layerxd/
│   ├── layerxctl/
│   ├── layerx-verify/
│   └── layerx-genesis/
├── contracts/
├── migrations/
├── tests/
├── fuzz/
└── tools/
```

The runtime should use:

- Canonical CBOR or a purpose-built canonical binary codec.
- Ed25519 for agent identities.
- secp256k1 for Paxeer-facing certificates and transactions.
- Domain-separated SHA-256 or BLAKE3 commitments.
- A canonical append-only activity log.
- SQLite, which is itself C, for rebuildable indexes and materialized state.
- A single deterministic state writer.
- Worker threads only for signature verification, networking and non-consensus work.
- Checked fixed-width arithmetic with a tested 128/256-bit C implementation.
- Sanitizers, fuzzing, deterministic replay tests and fault injection.

JSON/HTTP can remain as an optional gateway, but it must not define consensus behavior.

## What should not be ported

The existing Go implementation should be treated as a behavioral reference, not translated file by file.

We should discard:

- PostgreSQL structures as the protocol definition.
- HTTP endpoints as the canonical wire protocol.
- In-memory authentication challenges as authority state.
- Direct Crossverse access inside execution.
- Development settlement fallbacks.
- Process-local SSE as the authoritative event mechanism.
- Implicit background timing as consensus behavior.
- Documentation that mixes historical plans with current behavior.

We should preserve only proven concepts:

- DID-native accounts.
- Escrow-bounded spending.
- Fully reserved assets.
- Signed receipts.
- Merkle commitments.
- Idempotent execution.
- Crash recovery.
- Deterministic perps arithmetic.
- Fail-closed market-data handling.
- Staged rollout and emergency modes.
- Paxeer custody and escape guarantees.

## Migration posture

The new protocol should start from an explicit genesis manifest, not silently inherit the old database.

A genesis import must separately account for:

- USDX balances.
- Vault reserves.
- Open holds.
- Queued withdrawals.
- Liquidity and insurance pools.
- Open perps positions.
- Pending orders.
- Funding state.
- DID-to-EVM bindings.
- Outstanding receipts and anchored roots.

Every imported value must reconcile against Paxeer custody or be rejected. The old LayerX remains read-only during shadow replay until the new C implementation reproduces its accepted outcomes.

## Required qualification

Before calling the protocol ready:

- Byte-identical replay across machines and architectures.
- Deterministic state roots from millions of activities.
- Crash recovery at every write boundary.
- Network partition and sequencer-loss tests.
- Malformed activity and signature fuzzing.
- Overflow and rounding proofs.
- Guarantor disagreement and equivocation tests.
- Data-unavailability simulations.
- Emergency-exit execution on Paxeer.
- Full reserve reconciliation.
- Shadow comparison against the existing LayerX.
- Canary with real agents.
- Independent C API and wire-protocol conformance suites.

## Decisions that must be locked first

My recommended defaults are:

- New standalone implementation repository.
- Existing legacy implementation remains untouched as an external reference.
- C17 reference implementation.
- Canonical binary protocol with an optional JSON gateway.
- One active sequencer initially.
- Bonded Paxeer guarantor quorum independently replays batches.
- Append-only activity log as authority; SQLite indexes are rebuildable.
- Payments, escrow, budgets, services and perps as protocol modules.
- Paxeer used only for custody, checkpoints, guarantees, disputes and exits.

The remaining scope question is what "agent activity" must cover in version 1: only economically meaningful actions, or the complete lifecycle of agent work—including task commitments, tool execution attestations, deliverables and disputes. That choice changes the protocol's core event vocabulary.

---

# 402LXP — the single financial transaction gateway

Exactly. That makes `402LXP` the single financial transaction gateway for LayerX.

Every feature may have richer metadata and rules, but every monetary effect must compile into one or more authenticated balance transfers. No module may directly edit a balance.

## The fundamental state transition

For a transfer of amount `q` from account `x` to account `z`:

```text
Before:
    balance[x] = bx
    balance[z] = bz

After:
    balance[x] = bx - q
    balance[z] = bz + q
```

Subject to:

```text
q > 0
balance[x] >= q
authorization controls x
nonce is exactly next_nonce[x]
asset[x] == asset[z]
no integer overflow
sum(all balance changes) == 0
```

The protocol computes the new balances. Clients must never be trusted to submit authoritative "new balance" values.

## The two public operations

### `SEND`

A payer pushes value to a recipient.

```text
SEND {
    from
    to
    asset
    amount
    sequence
    idempotency_key
    expires_at
    context_hash
    conditions
    authorization
}
```

The authorization may be:

- The owner's direct signature.
- A session key.
- A delegated capability.
- A budget allowance.
- An escrow authority.
- A protocol-module capability.

### `RECEIVE`

A recipient pulls value using authorization previously issued by the payer.

```text
RECEIVE {
    from
    to
    asset
    amount
    grant_id
    receiver_sequence
    idempotency_key
    context_hash
    receiver_authorization
    payer_grant
}
```

`RECEIVE` must not let a recipient debit arbitrary accounts. It requires a signed payer grant specifying:

- Authorized recipient.
- Asset.
- Maximum amount.
- Total or recurring allowance.
- Expiration.
- Permitted purpose.
- Optional service or invoice identifier.
- Revocation sequence.

Both operations execute the same internal transfer function. The difference is who initiates it and which authorization proves permission to debit `from`.

## One internal C primitive

There should be exactly one balance mutation function:

```c
int lxp_apply_transfer(
    struct lxp_state *state,
    const struct lxp_transfer *transfer,
    struct lxp_receipt *receipt
);
```

And an atomic multi-leg form:

```c
int lxp_apply_transfer_set(
    struct lxp_state *state,
    const struct lxp_transfer_set *set,
    struct lxp_receipt *receipt
);
```

Everything else must call this kernel. Direct database balance updates are forbidden.

## How every feature becomes transfers

| System action | 402LXP transfer |
|---|---|
| Agent payment | Agent A → Agent B |
| Service purchase | Buyer → Provider |
| Open escrow | Owner → Escrow subaccount |
| Capture escrow | Escrow subaccount → Provider |
| Release escrow | Escrow subaccount → Owner |
| Create budget | Owner → Budget subaccount |
| Spend budget | Budget subaccount → Recipient |
| Stream payment | Payer stream account → Recipient |
| Protocol fee | Actor → Fee treasury |
| Deposit | Paxeer reserve mirror → Agent |
| Withdrawal | Agent → Paxeer withdrawal account |
| Perps margin | Agent → Position margin account |
| Release margin | Position margin account → Agent |
| Trading loss | Position margin → Liquidity pool |
| Trading profit | Liquidity pool → Agent |
| Funding payment | Long funding account → Short funding account |
| Liquidation fee | Position margin → Liquidator/insurance |
| Insurance payout | Insurance pool → Deficit account |
| Refund | Merchant/escrow → Buyer |

Orders, service agreements and positions still have non-monetary state, but none of them can create financial effects except through authenticated 402LXP transfers.

## Accounts and subaccounts

The protocol should represent locked funds as real accounts rather than hidden balance columns:

```text
agent:<did>:main
agent:<did>:budget:<id>
agent:<did>:escrow:<id>
agent:<did>:margin:<position>
system:liquidity:<market>
system:insurance
system:fees
system:paxeer-reserve
system:paxeer-withdrawals
```

For example, opening a position does not merely set `reserved_margin = 100`. It performs:

```text
agent:alice:main
    → agent:alice:margin:position-42
    100 USDX
```

That makes every unit traceable through the same ledger.

## Atomic transfer sets

Complex operations need multiple balance legs that either all succeed or none do.

Example liquidation:

```text
1. position margin → liquidity pool
2. position margin → liquidation fee account
3. insurance pool → liquidity pool deficit
4. remaining position margin → agent main account
```

The transfer set has one authorization context, one execution sequence and one receipt. If any leg violates an invariant, the entire set rolls back.

The conservation rule is:

```text
For every asset:

    Σ debits == Σ credits
```

Deposits and withdrawals still preserve this rule because they move value between agent accounts and the Paxeer reserve mirror. Ordinary modules never mint or burn.

## The HTTP 402 flow

For paid services:

```text
Agent requests resource
        │
        ▼
Service returns HTTP 402 + LXP payment requirement
        │
        ▼
Agent signs 402LXP SEND
        │
        ▼
LayerX executes authenticated transfer
        │
        ▼
LayerX returns signed receipt
        │
        ▼
Service verifies receipt and delivers resource
```

The payment requirement should contain:

```text
network_id
recipient
asset
amount
invoice_id
purpose_hash
expiry
acceptable_conditions
service_signature
```

The client returns either the LayerX receipt or a payment authorization that the service can submit through `RECEIVE`.

This supports both models:

- `SEND`: client pays first and presents the receipt.
- `RECEIVE`: service claims payment from a signed client authorization.

## Receipt

Every successful operation returns:

```text
402LXPReceipt {
    protocol_version
    transaction_id
    operation
    global_sequence
    asset
    amount

    from
    from_balance_before
    from_balance_after
    from_sequence

    to
    to_balance_before
    to_balance_after

    transfer_set_root
    authorization_hash
    context_hash

    previous_state_root
    resulting_state_root
    batch_id
    timestamp
    sequencer_signature
}
```

The before-and-after balances are evidence, not client-controlled inputs.

Once checkpointed, the receipt gains:

```text
activity_inclusion_proof
state_inclusion_proof
checkpoint_id
guarantor_certificate
paxeer_settlement_reference
```

## Core invariants

The protocol should make these non-negotiable:

1. Every monetary mutation is a 402LXP transfer.
2. Every debit has explicit authority.
3. `RECEIVE` requires payer authorization.
4. No account can become negative.
5. Every normal transfer conserves supply.
6. Transfer sets are completely atomic.
7. Every successful transfer has one durable receipt.
8. Every idempotency key produces at most one economic result.
9. Every account sequence is consumed exactly once.
10. No module can write balances directly.
11. External oracle data cannot directly alter balances.
12. Paxeer deposits cannot credit LayerX without finalized proof.
13. Paxeer withdrawals cannot pay twice.
14. Replaying history must produce identical balances and roots.

So the hierarchy becomes:

```text
402LXP
    └── SEND / RECEIVE
            └── authenticated transfer set
                    └── deterministic balance transitions
                            └── activity receipt
                                    └── LayerX checkpoint
                                            └── Paxeer guarantee
```

That is a much cleaner protocol than the current LayerX service: one financial doorway, one execution law and one auditable balance-transition model.
