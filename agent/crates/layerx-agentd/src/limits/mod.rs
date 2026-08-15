//! Layered tenant admission limits and bounded work controls.

#[path = "rate.rs"]
mod rate_limit;

pub use rate_limit::{
    Admission, CounterLedger, LimitConfig, LimitId, LimitScope, RateLimiter, RateRequest, Refusal,
    Utilization, Window, COUNTER_CONSISTENCY_MODEL,
};
