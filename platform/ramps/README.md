# LayerX market-maker ramp kit

This kit is for independent market makers acting as ordinary LayerX principals. A ramp receives no protocol role, reserved vocabulary, custody guarantee, minting power, settlement authority, or privileged API. It uses an operator-owned agent account, ordinary 402LXP transfers, optional payer grants supplied by customers, and the existing Paxeer deposit and withdrawal boundary for the operator's own inventory.

Every integration must display: **External custody: this independent market maker controls the off-platform funds and payout.** LayerX guarantees neither the provider-side funds nor the market maker's liquidity, price, slippage, payout, reversal handling, recovery, or availability. Paxeer remains LayerX's sole custody and guaranteed-withdrawal boundary; a ramp cannot describe any other rail as LayerX settlement.

The on-ramp path accepts only a `FiatJourneyState::Credited` produced by the production fiat adapter before requesting the operator-to-customer 402LXP transfer. The off-ramp path verifies the customer-to-operator receipt before exposing external payout state. Pending, reversed, and unknown provider states remain explicit and are never converted into LayerX facts.

Run the reference configuration check with `cargo run -p layerx-reference-ramp`. A production deployment must implement `OrdinaryPrincipalPlane` using its LayerX SDK client and Paxeer boundary credentials. Provider sandbox and testnet endpoints are deployment inputs; this repository contains no provider simulator, embedded secret, or invented settlement response.
