//! Hosted webhook delivery for `LayerX` journey, payment, approval and program
//! events.
//!
//! Deliveries are signed with Ed25519 under the published scheme in
//! [`scheme`], which is the exact scheme the shipped middleware and framework
//! integrations verify, so an unmodified integration consumes hosted deliveries.
//! They are ordered per subject, retried with bounded deterministic backoff,
//! dead-lettered when attempts are exhausted, and replayable from stable
//! cursors. Delivery state is honest: nothing is reported as delivered without
//! an accepting status observed from the developer's own endpoint, and no fact
//! is presented above `unverified` without the receipt digest that established
//! it.

pub mod cursor;
pub mod deliveries;
pub mod encoding;
pub mod endpoints;
pub mod error;
pub mod events;
pub mod http;
pub mod scheme;
pub mod service;
pub mod state;
pub mod transport;

pub use deliveries::{AttemptRecord, Delivery, DeliveryRecord, DeliveryState, FailureKind};
pub use endpoints::{Endpoint, EndpointHealth, RetryPolicy};
pub use error::WebhookError;
pub use events::{
    settled_payment, DeliveryId, EndpointId, EventDraft, EventId, EventKind, PaymentDraft,
    Principal, ProtocolEvent, ProtocolFact, SubjectId, Verification,
};
pub use scheme::{EndpointKey, Presentation, ReplayGuard, Verified};
pub use service::{
    DispatchReport, EndpointRequest, EventPage, PublishOutcome, RedeliveryOutcome, Registration,
    Service,
};
pub use state::Ledger;
pub use transport::{Attempt, HttpTransport, Transport};

/// Names the delivery contract this crate implements.
#[must_use]
pub fn platform_webhooks() -> &'static str {
    "signed-ordered-at-least-once-webhook-delivery-with-dead-letter-and-redelivery"
}
