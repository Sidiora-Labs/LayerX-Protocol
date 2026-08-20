use layerx_ramp_toolkit::{platform_ramp_toolkit, EXTERNAL_CUSTODY_LABEL};

fn main() {
    println!("{}", platform_ramp_toolkit());
    println!("{EXTERNAL_CUSTODY_LABEL}");
    println!("Configure a production OrdinaryPrincipalPlane to submit 402LXP transfers and Paxeer rebalancing for the operator account.");
}

#[must_use]
pub const fn platform_reference_ramp() -> &'static str {
    "receipt-backed-reference-market-maker"
}
