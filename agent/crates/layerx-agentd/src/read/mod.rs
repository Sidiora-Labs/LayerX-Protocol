//! Proof-gated daemon read surfaces.

#[path = "balance.rs"]
mod balances;

pub use balances::{balance, BalanceRead, Freshness};
