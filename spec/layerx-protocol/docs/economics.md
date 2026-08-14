# LayerX Economics

Normative specification of every value flow in LayerX: fee metering and
computation, the `fee_limit` ceiling, the fee treasury, sequencer compensation,
guarantor bonds, rewards and slashing, the insurance pool, perps liquidity
accounting, funding mechanics, liquidation incentives, and the reserve accounting
that proves one-to-one Paxeer backing. MUST, MUST NOT, SHOULD and MAY are
normative for a conforming C17 implementation; companion documents under
`spec/layerx-protocol/docs/` are `state-machine.md`, `wire-format.md`,
`checkpointing.md`, `guarantors.md`, `data-availability.md` and `migration.md`.

**Governing rule.** 402LXP is the single financial doorway: every quantity here
moves as legs of an authenticated transfer set through `lxp_apply_transfer` or
`lxp_apply_transfer_set`. No module, fee collector, reward distributor or
liquidation engine writes a balance directly.

## 1. Units, types and rounding

| Quantity | Scale | C type | Intermediate |
|---|---|---|---|
| Asset amount | micro-units, 1 USDX = 1e6 µUSDX | `int64_t` (always `>= 0` in state) | `lx_i128` |
| Price | cents, 1e2 | `int64_t` | `lx_i128` |
| Rate (funding) | ppb, 1e9 | `int64_t` | `lx_i128` |
| Ratio (margin, fee) | bps, 1e4 | `int64_t` | `lx_i128` |
| Contracts | whole contracts | `int64_t` | — |

There are no floating-point values anywhere in execution. All arithmetic is
checked; on overflow the operation returns `LX_ERR_OVERFLOW` and the enclosing
transfer set rolls back in full.

```c
enum lx_round { LX_ROUND_FLOOR, LX_ROUND_CEIL, LX_ROUND_TRUNC };
int lx_checked_add(int64_t a, int64_t b, int64_t *out);   /* _sub, _mul alike */
int lx_mul_div(int64_t a, int64_t b, int64_t c, enum lx_round r, int64_t *out);
```

`lx_mul_div` holds the product in a 128-bit intermediate, never in `int64_t`.
`LX_ROUND_TRUNC` rounds toward zero, `LX_ROUND_FLOOR` toward negative infinity;
they coincide for non-negative operands and differ only for signed PnL and
funding.

**The rounding law**, applied without exception:

1. Amounts the protocol **collects** (fees, margin requirements, liquidation
   fees) round `LX_ROUND_CEIL`.
2. Amounts the protocol **pays out** (refunds, rewards, pro-rata credits,
   available-withdrawal figures) round `LX_ROUND_FLOOR`.
3. Signed PnL and funding accruals round `LX_ROUND_TRUNC`.
4. Every residue produced by rule 2 is accumulated exactly and credited to
   `system:insurance` — never dropped, never left in a floating remainder, never
   silently absorbed by the last recipient in a loop.

Rule 4 is what makes conservation exact: flooring `n` recipients leaves a residue
of at most `n - 1` µUSDX, and that residue is a real leg with a real destination.

## 2. Resource metering

Metering depends only on the activity and the pre-state. Wall-clock time, thread
counts and I/O latency are never inputs.

| Dimension | Unit | Default weight (µUSDX) |
|---|---|---|
| `LX_METER_BYTES` | canonical encoded byte of the activity | 4 |
| `LX_METER_SIGVERIFY` | one Ed25519 or secp256k1 verification | 2 000 |
| `LX_METER_STATE_READ` | one state key read | 100 |
| `LX_METER_STATE_WRITE` | one state key written | 800 |
| `LX_METER_HASH` | one 32-byte hash block | 5 |
| `LX_METER_TRANSFER_LEG` | one 402LXP leg in a transfer set | 1 500 |
| `LX_METER_MODULE_STEP` | one module-declared unit of work | 50 |

The meter (`struct lx_meter { int64_t units[LX_METER_DIM_COUNT]; int64_t
charged; }`) is advanced by the kernel at the point work is performed, never
estimated afterwards. A module iterating over `k` items MUST charge `k`
`MODULE_STEP` units before the loop body, so an activity cannot outrun its own
fee ceiling.

## 3. Fee computation

```
fee_raw   = LX_FEE_BASE + Σ_d ( weight[d] * units[d] )    /* exact, no division */
fee_final = ceil( fee_raw * congestion / LX_CONGESTION_DEN )
```

`LX_FEE_BASE` defaults to 5 000 µUSDX. `congestion` is an integer state variable
with denominator `LX_CONGESTION_DEN = 1 000 000`, recomputed once per epoch from
the previous epoch only:

```
delta       = trunc( congestion * (used - target) / (target * LX_CONGESTION_ADJ) )
congestion' = clamp( congestion + delta, LX_CONGESTION_MIN, LX_CONGESTION_MAX )
```

with `used` the epoch's total metered units, `target` a governance parameter,
`LX_CONGESTION_ADJ = 8`, `MIN = 1 000 000` (1.0x) and `MAX = 64 000 000` (64x).
`delta` uses truncating signed division, so the update is bit-identical on every
architecture. Congestion never depends on mempool depth, which is not state.

### 3.1 The `fee_limit` ceiling and the fee escrow

`fee_limit` is the actor's maximum acceptable charge, enforced through real
accounts rather than a bookkeeping flag.

**Admission (pre-consensus, sequencer only).** An activity is *not included* —
no state effect, no sequence consumed, no fee — when `fee_limit < LX_FEE_BASE`,
when `balance(actor:main) < fee_limit`, or when the envelope is malformed,
mis-signed, expired or has a non-next `account_sequence`. These are reported over
the gateway with an `LX_ERR_*` code and never enter history; censorship is
handled by force-inclusion (`docs/checkpointing.md`), not here.

**Execution (consensus).** For every included activity the kernel executes, in
order:

1. `FEE_SET` leg 1, always committed: `agent:<did>:main -> system:fees:escrow`
   for exactly `fee_limit`. It cannot fail — admission proved the balance and
   this leg runs before any module code.
2. The module body runs against a nested journal (`EFFECT_SET`) with the meter
   live; if `fee_final` would exceed `fee_limit` at any point, execution aborts
   with `LX_RESULT_FEE_LIMIT_EXCEEDED`.
3. `FEE_SET` legs 2 and 3, always committed: `system:fees:escrow -> system:fees`
   for `fee_charged`, and `system:fees:escrow -> agent:<did>:main` for the
   unused remainder.

| Outcome | `EFFECT_SET` | `fee_charged` | Sequence | Receipt |
|---|---|---|---|---|
| Success | committed | `fee_final` | consumed | `LX_RESULT_OK` |
| Module error | rolled back | metered-so-far, ceil | consumed | module result code |
| `fee_limit` exhausted | rolled back | `fee_limit` | consumed | `LX_RESULT_FEE_LIMIT_EXCEEDED` |
| Already-liquidated keeper race | rolled back | `LX_FEE_BASE` only | consumed | `LX_RESULT_ALREADY_LIQUIDATED` |

A failed activity still costs the actor, still consumes exactly one account
sequence, still produces exactly one durable receipt, and still yields exactly one
economic result per idempotency key. Balances can never go negative because the
escrow leg precedes all module work, and `system:fees:escrow` holds zero at every
commit boundary.

## 4. Fee treasury and distribution

`system:fees` accrues every `fee_charged` and is emptied at each epoch close by
one atomic transfer set, in canonical account-id byte order:

| Recipient | Share (bps) |
|---|---|
| Sequencer compensation account | 3 000 |
| Guarantor reward pot | 3 000 |
| `system:insurance` | 2 000 |
| `system:treasury` (governance-controlled) | 2 000 |

Each share is `floor(total * share_bps / 10 000)`. The guarantor pot splits
`floor(pot / n_eligible)` equally among active, non-jailed members whose epoch
miss ratio is at most 20% — equal rather than bond-weighted, to avoid rewarding
capital concentration. Residues from both levels are summed and credited to
`system:insurance` in the same set. Shares sum to exactly 10 000 bps; a
governance change breaking that is rejected at parameter validation.

## 5. Sequencer compensation

The single active sequencer posts `LX_SEQ_BOND_MIN` (default 250 000 USDX) into
Paxeer custody before it may seal, and per epoch receives the 3 000 bps fee
share less liveness penalties.

| Condition | Effect |
|---|---|
| Seals every batch within `LX_BATCH_MAX_GAP_MS` (default 2 000) while the mempool is non-empty | full share |
| Misses a batch slot | share reduced by `LX_SEQ_MISS_BPS = 100` per missed slot, floor zero |
| No batch for `LX_SEQ_FAILOVER_EPOCHS` (default 4) | share zero; governance may promote a standby sequencer |
| Publishes two distinct batches with the same `batch_number` | 100% of bond slashed |
| Seals a batch whose replay diverges from its header roots | 100% of bond slashed |

Sequencer rewards accrue to a LayerX account under the same `PARAM_UNBOND_MS`
withdrawal delay as guarantor rewards, so misbehaviour discovered late is still
economically reachable.

## 6. Guarantor bonds, rewards and slashing

Bonds live in Paxeer custody, not LayerX, because the slashing authority is the
Paxeer contract. `docs/guarantors.md` is authoritative for set membership,
threshold `T` of `N` and slash evidence; this section states only the amounts and
the destinations. `LX_GUARANTOR_BOND_MIN` (`B_min`) defaults to 100 000 USDX,
the initial set is `N = 7`, `T = 5`, and unbonding runs for `PARAM_UNBOND_MS`
(32 days) throughout which the bond stays fully slashable.

| Offence | Proof | Slash | Destination |
|---|---|---|---|
| Equivocation: two valid signatures over conflicting attestations at the same `(epoch, batch_number)` | the signature pair, on Paxeer | full bond, plus ejection | `SLASH_REPORTER_BPS` (1 000) to the submitter, remainder to `system:insurance` |
| Invalid-root claim: a root disproved by verified replay | governance adjudication under timelock, not mechanically verifiable in v1 | up to full bond, plus ejection | as above |
| DA unavailability: an unanswered or invalidly answered challenge | challenge expiry on Paxeer | `UNAVAIL_SLASH_BPS` (500), escalating to ejection on repeat within an epoch | as above |
| Liveness: miss ratio above 20% in an epoch | epoch accounting | no slash — jailed, excluded from `N` for threshold purposes, epoch reward forfeited | forfeited reward stays in `system:fees` for the next epoch |

Slashed value destined for insurance re-enters LayerX only through the bridge
module, against a finalised Paxeer event with a nullifier. There is no
administrative credit path. `LX_DA_CHALLENGE_BOND` defaults to 250 USDX: enough
to deter griefing, trivial against the slash it can trigger.

Stated honestly, this is an economic guarantee and not a validity proof. A
colluding super-majority of bonded guarantors can finalise an invalid root; the
protocol's answer is bond size plus the emergency-exit path, not cryptographic
impossibility. Adding validity proofs later changes none of the activity
protocol.

## 7. Insurance pool

`system:insurance` is a real account with a non-negative balance.

**Funded by** the 2 000 bps fee share, all rounding residues (section 1 rule 4),
the insurance share of every slash and of every liquidation fee, and voluntary
deposits, which are ordinary transfers with no special privilege.

**Drained by** liquidation deficits (section 9), exit shortfalls during a
data-availability emergency, and oracle-failure remediation — the latter two
governance-gated. Every drain is a transfer set leg appearing in the same receipt
stream as any other payment.

**Floor and gating.** Below `LX_INSURANCE_FLOOR` every perps market becomes
`REDUCE_ONLY`; at zero balance, `PAUSED`. A drain exceeding the balance is capped
at it, the uncovered remainder becomes an explicit bad-debt state object, and the
affected market halts. The pool never goes negative and bad debt is never hidden
inside a balance.

## 8. Perps liquidity pool and capacity

`system:liquidity:<market>` is the counterparty to every position in that market,
funded only by ordinary transfers from agent accounts. There is no mint.

| Event | Transfer legs |
|---|---|
| Open position | `agent:<did>:main -> agent:<did>:margin:<position>` |
| Trading fee | `agent:<did>:margin:<position> -> system:fees` |
| Trader loss realised | `agent:<did>:margin:<position> -> system:liquidity:<market>` |
| Trader profit realised | `system:liquidity:<market> -> agent:<did>:margin:<position>` |
| Close position | `agent:<did>:margin:<position> -> agent:<did>:main` |

Capacity gating, all integer, evaluated per market before activation and before
any size increase:

```
usable       = liquidity + insurance - insurance_floor
                         - committed_profit_reserve - pending_withdrawals
required     = ceil(configured_max_oi * stress_loss_bps     / 10000)
             + ceil(configured_max_oi * liquidation_fee_bps / 10000)
capacity_oi  = floor(usable * 10000 / (stress_loss_bps + liquidation_fee_bps))
effective_oi_cap = min(configured_max_oi, capacity_oi)
activation_allowed = (usable >= required)
```

The effective cap is a minimum against the configured cap, so capacity can only
lower advertised exposure, never silently raise it. `committed_profit_reserve` is
the unrealised profit the pool would owe if every open position closed at the
current mark, recomputed from state and never cached across batches.

## 9. Funding and liquidation

### 9.1 Funding rate

Per market, once per `LX_FUNDING_INTERVAL_MS`:

```
skew_ppb    = trunc( net_notional * LX_MAX_SKEW_FUNDING_PPB / effective_oi_cap )
applied_ppb = clamp( external_ppb + skew_ppb, -LX_MAX_FUNDING_PPB, +LX_MAX_FUNDING_PPB )
raw_p       = trunc( notional_p * applied_ppb * elapsed_ms
                     / (1e9 * LX_FUNDING_INTERVAL_MS) )
```

`external_ppb` arrives only as a signed oracle activity already in history, never
read live. Positive `applied_ppb` means longs pay shorts.

Truncation makes naive per-position sums unequal, so conservation is restored
explicitly, in this order:

1. Compute `raw_p` for every open position on the paying side.
2. Cap each payer at its available margin; a capped payer records a funding
   deficit and is flagged for liquidation.
3. `total_debit = Σ capped amounts`, computed after capping.
4. `credit_r = floor(total_debit * notional_r / Σ notional_receivers)` per
   receiver, then `residue = total_debit - Σ credit_r`, necessarily `>= 0`.
5. Emit one transfer set: payer margin accounts to `system:funding:<market>`,
   that account to receiver margin accounts, and `residue` to
   `system:insurance`. The funding account holds zero at the commit boundary.

Σ debits equals Σ credits exactly, by construction, for any rounding.

### 9.2 Liquidation

```
notional     = abs(contracts) * LX_CONTRACT_NOTIONAL_MICRO
upnl         = trunc(side_sign * notional * (mark_cents - entry_cents) / entry_cents)
equity       = margin + upnl - funding_debit + funding_credit
maintenance  = ceil(notional * maintenance_margin_bps / 10000)
liq_fee      = ceil(notional * liquidation_fee_bps    / 10000)
eligible     = (equity <= maintenance + liq_fee)
```

Eligibility always uses the latest oracle mark in history, never a displayed
trigger price. Liquidation is permissionless: the first valid liquidation
activity in sequence order wins, and race losers pay only `LX_FEE_BASE`
(section 3.1) so keeper competition stays cheap. The waterfall is a single atomic
transfer set, legs in this fixed order:

1. `margin -> system:liquidity:<market>` — realised loss, capped at margin.
2. `margin -> keeper:main` and `margin -> system:insurance` — `liq_fee` split
   `LX_LIQ_KEEPER_BPS = 5 000` to the keeper, remainder to insurance.
3. `system:insurance -> system:liquidity:<market>` — any deficit margin did not
   cover, capped at the insurance balance.
4. `margin -> agent:<did>:main` — whatever margin remains.

If step 3 cannot cover the deficit, the shortfall is recorded as bad debt and the
market is `PAUSED`. If any leg violates an invariant the whole set rolls back and
the position stays open — safe, because it is simply eligible again at the next
mark.

## 10. Reserve accounting

### 10.1 The supply invariant

Let `A` be all accounts, agent subaccounts and `system:*` included, and let
`custody(t)` be the Paxeer vault balance backing LayerX at the last bridge event
processed at or before `t`.

`INVARIANT R: Σ_{a ∈ A} balance(a) == custody(t)` at every commit boundary. The
proof is by induction over transfer sets.

- **Base.** Genesis (`docs/migration.md`) mints exactly `custody(t_0)` into
  `system:paxeer-reserve` against an attested custody reading, then distributes it
  by one atomic transfer set. R holds at `t_0`.
- **Ordinary step.** Every non-bridge transfer set satisfies
  `Σ debits == Σ credits` per asset, checked by `lxp_apply_transfer_set` before
  commit, so Σ over `A` is unchanged and R is preserved.
- **Bridge step.** Only two operations change Σ over `A`, and only the bridge may
  emit them: `MINT_MIRROR(q)` credits `system:paxeer-reserve` against a finalised
  Paxeer deposit of exactly `q` with an unused nullifier, `BURN_MIRROR(q)` debits
  it against a finalised payout of exactly `q`. Each moves Σ and `custody` by the
  same `q` in the same direction, so R is preserved.

Therefore every asset unit inside LayerX is backed one-to-one by Paxeer custody.
Ordinary flows are compositions of the above: a deposit is `MINT_MIRROR(q)` then
`system:paxeer-reserve -> agent:<did>:main`; a withdrawal is
`agent:<did>:main -> system:paxeer-withdrawals`, then after the Paxeer payout
finalises `system:paxeer-withdrawals -> system:paxeer-reserve` and
`BURN_MIRROR(q)`. Nullifiers make double-credit and double-payout impossible,
satisfying invariants 12 and 13.

### 10.2 Reconciliation buckets

`Σ balance(a)` is computed by summing these thirteen disjoint buckets, in this
order, from state alone:

| # | Bucket | Accounts |
|---|---|---|
| 1 | Agent spendable | `agent:*:main` |
| 2 | Escrow and holds | `agent:*:escrow:*` (order margin reservations are `agent:*:escrow:order:*`) |
| 3 | Budgets and streams | `agent:*:budget:*`, `agent:*:stream:*` |
| 4 | Position margin | `agent:*:margin:*` |
| 5 | Liquidity pools | `system:liquidity:*` |
| 6 | Insurance and fee treasury | `system:insurance`, `system:fees` |
| 7 | Pass-through, zero at commit boundaries | `system:fees:escrow`, `system:funding:*` |
| 8 | Custody-facing | `system:paxeer-withdrawals`, `system:paxeer-reserve` |

### 10.3 Reconciliation procedure

Per batch, cheap: assert `Σ debits == Σ credits` for every non-bridge set, and
assert bucket 7 is zero at the commit boundary. Per checkpoint, full:

1. Compute the bucket sums from the state at `resulting_state_root`.
2. Compute `custody(t)` from the bridge module's finalised event log, Σ minted
   minus Σ burned — state, not a live RPC call; no external I/O enters execution.
3. Compare steps 1 and 2 with **tolerance zero**, and out of band have an auditor
   check step 2 against the Paxeer vault balance at the referenced L1 block.

Any mismatch raises `LX_HALT_RESERVE_MISMATCH`: no further batches are sealed,
guarantors refuse to attest, no checkpoint is submitted, and emergency exit arms
exactly as in a DA stall. A mismatch is never reconciled by adjusting a balance;
it is diagnosed by replaying the range to find the first set whose legs did not
sum to zero.

## 11. Parameters

| Parameter | Default | Governance bounds |
|---|---|---|
| `LX_FEE_BASE` | 5 000 µUSDX | 1 000 – 100 000 |
| `LX_CONGESTION_MIN` / `MAX` | 1e6 / 64e6 | `MIN` fixed; `MAX` ≤ 256e6 |
| Fee split (seq/guar/ins/treasury) | 3000/3000/2000/2000 bps | must sum to 10 000 |
| `LX_SEQ_BOND_MIN`, `LX_GUARANTOR_BOND_MIN` | 250 000 / 100 000 USDX | ≥ 100 000 / ≥ 50 000 |
| `PARAM_UNBOND_MS`, `SLASH_REPORTER_BPS`, `UNAVAIL_SLASH_BPS` | 32 days, 1 000, 500 | set in `docs/guarantors.md` |
| `LX_DA_CHALLENGE_BOND`, `LX_LIQ_KEEPER_BPS` | 250 USDX, 5 000 bps | 50 – 5 000, 2 000 – 8 000 |
| `LX_INSURANCE_FLOOR` | per-market at listing | ≥ stress loss at `effective_oi_cap` |
| `LX_MAX_FUNDING_PPB` | market-locked | reduce only |

Parameter changes are governance activities, ordered and attested like any other,
taking effect only at an epoch boundary so every replica applies them at the same
sequence.

## 12. Implementation map

| Path | Responsibility |
|---|---|
| `include/layerx/lx_math.h`, `src/protocol/lx_math.c` | checked arithmetic, `lx_mul_div`, 128-bit intermediates |
| `src/protocol/lxp_transfer.c` | `lxp_apply_transfer`, `lxp_apply_transfer_set`, conservation check |
| `src/protocol/fee.c`, `treasury.c` | meter, `fee_raw`, congestion update, fee escrow legs, epoch distribution and residue sink |
| `src/modules/perps/funding.c`, `liquidation.c`, `capacity.c` | sections 9.1, 9.2 and 8 verbatim |
| `src/paxeer/bridge.c` | `MINT_MIRROR`, `BURN_MIRROR`, nullifiers, slash ingestion |
| `src/state/reserve_audit.c` | bucket sums, per-batch and per-checkpoint reconciliation |
| `tests/economics/` | rounding-law vectors, conservation fuzzing, funding residue proofs |
