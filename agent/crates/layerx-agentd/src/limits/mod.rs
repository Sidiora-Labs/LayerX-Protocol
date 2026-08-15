//! Layered tenant admission limits and bounded work controls.

pub mod admission;
pub mod deadline;
pub mod quota;

pub use quota::Quota;

/// Cancels caller-owned work or transfers an indeterminate submission to its resolver.
pub fn cancel(
    tracker: &mut deadline::RequestTracker,
    request_id: u64,
    observed_at_ms: u64,
) -> Result<deadline::DisconnectOutcome, deadline::DeadlineError> {
    deadline::disconnect_request(tracker, request_id, observed_at_ms)
}

/// Records a client activity and durably sheds only that client when pathological.
pub fn shed(
    quota: &mut quota::Quota,
    store: &mut crate::store::Store,
    activity: quota::ClientActivity,
) -> Result<Option<quota::SheddingDecision>, quota::QuotaError> {
    quota.observe_activity(store, activity)
}

#[path = "rate.rs"]
mod rate_limit;

pub use rate_limit::{
    Admission, CounterLedger, LimitConfig, LimitId, LimitScope, RateLimiter, RateRequest, Refusal,
    Utilization, Window, COUNTER_CONSISTENCY_MODEL,
};
