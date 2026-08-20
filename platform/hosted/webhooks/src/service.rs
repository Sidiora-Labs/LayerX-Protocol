//! The hosted webhook service: endpoint registration and secret rotation,
//! publication with replay protection, ordered at-least-once delivery with
//! bounded backoff, the dead-letter path, and cursor-based redelivery.

use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Instant;

use serde::Serialize;

use crate::cursor;
use crate::deliveries::{AttemptRecord, Delivery, DeliveryRecord, DeliveryState, FailureKind};
use crate::encoding::digest;
use crate::endpoints::{validate_url, Endpoint, EndpointHealth, RetryPolicy};
use crate::error::WebhookError;
use crate::events::{DeliveryId, EndpointId, EventKind, Principal, ProtocolEvent, Verification};
use crate::scheme::{self, EndpointKey};
use crate::state::{self, State};
use crate::transport::{Attempt, Transport};

/// Largest page the redelivery surface returns.
pub const MAXIMUM_PAGE: usize = 200;
/// Largest number of endpoints one principal may register.
pub const MAXIMUM_ENDPOINTS: usize = 32;
/// How long a superseded signing key keeps signing after a rotation is
/// announced, giving a receiver time to install the announced public key.
pub const DEFAULT_KEY_OVERLAP_SECONDS: u64 = 86_400;

/// A published signing key. Only the public half leaves the service, so this
/// record is safe to render, store and paste into a receiver's configuration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Registration {
    /// The endpoint the key belongs to.
    pub endpoint: String,
    /// The key identifier delivered in `layerx-webhook-key-id`.
    pub key_id: String,
    /// The public half, base64 as the shipped consumers accept it.
    pub public_key: String,
    /// The exact value to place in `LAYERX_WEBHOOK_PUBLIC_KEYS_JSON`.
    pub public_keys_json: String,
    /// When this key starts signing, on an announced rotation.
    pub activates_at: Option<u64>,
    /// The obligation the scheme places on the receiver.
    pub receiver_obligation: String,
}

impl Registration {
    fn issued(endpoint: &EndpointId, key: &EndpointKey, activates_at: Option<u64>) -> Self {
        let key_id = key.id().to_owned();
        let public_key = key.public_key_base64();
        let public_keys_json = format!("{{\"{key_id}\":\"{public_key}\"}}");
        Self {
            endpoint: endpoint.as_str().to_owned(),
            key_id,
            public_key,
            public_keys_json,
            activates_at,
            receiver_obligation: scheme::RECEIVER_OBLIGATION.to_owned(),
        }
    }
}

/// The registration a developer asks for.
#[derive(Clone, Copy, Debug)]
pub struct EndpointRequest<'a> {
    /// Owning developer principal.
    pub principal: &'a Principal,
    /// Destination, transport-secure or loopback.
    pub url: &'a str,
    /// Subscribed families. An empty list subscribes to all of them.
    pub kinds: &'a [EventKind],
    /// Weakest level the endpoint is willing to receive.
    pub minimum_verification: Verification,
}

/// What publication did with one event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PublishOutcome {
    /// Durable log position of the event.
    pub position: u64,
    /// True when the identical event had already been published.
    pub duplicate: bool,
    /// Deliveries queued for this event.
    pub queued: Vec<String>,
}

/// What one dispatch pass did.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct DispatchReport {
    /// Attempts made in this pass.
    pub attempted: u32,
    /// Deliveries accepted in this pass.
    pub delivered: u32,
    /// Deliveries rescheduled in this pass.
    pub retrying: u32,
    /// Deliveries dead-lettered in this pass.
    pub dead_lettered: u32,
    /// Subject queues that were not due, suspended, or over budget.
    pub blocked: u32,
    /// When the next scheduled attempt becomes due.
    pub next_attempt_at: Option<u64>,
}

/// One page of the redelivery surface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EventPage {
    /// Events the endpoint is eligible to receive.
    pub events: Vec<ProtocolEvent>,
    /// Cursor to resume from.
    pub next_cursor: String,
    /// Whether more events remain after this page.
    pub has_more: bool,
}

/// What a redelivery request queued.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RedeliveryOutcome {
    /// Deliveries queued by this request.
    pub queued: Vec<String>,
    /// Cursor to resume from.
    pub next_cursor: String,
    /// Whether more events remain after this page.
    pub has_more: bool,
}

#[derive(Serialize)]
struct Envelope<'a> {
    scheme: &'a str,
    endpoint_id: &'a str,
    receiver_obligation: &'a str,
    event: &'a ProtocolEvent,
}

struct Prepared {
    queue: String,
    delivery: String,
    endpoint: String,
    url: String,
    headers: Vec<(String, String)>,
    payload: Vec<u8>,
    attempt: u32,
}

fn envelope_bytes(endpoint: &EndpointId, event: &ProtocolEvent) -> Result<Vec<u8>, WebhookError> {
    serde_json::to_vec(&Envelope {
        scheme: scheme::SCHEME_VERSION,
        endpoint_id: endpoint.as_str(),
        receiver_obligation: scheme::RECEIVER_OBLIGATION,
        event,
    })
    .map_err(|_| WebhookError::CorruptStore)
}

enum Ready {
    Prepared(Prepared),
    Blocked,
    Skip,
}

#[derive(Clone, Copy)]
enum Landing {
    Accepted(u16),
    Failed {
        failure: FailureKind,
        status: Option<u16>,
    },
}

/// The hosted webhook service over one durable root.
pub struct Service<T> {
    root: PathBuf,
    state: Mutex<State>,
    transport: T,
    policy: RetryPolicy,
    key_overlap_seconds: u64,
}

impl<T: Transport> Service<T> {
    /// Opens or creates the durable webhook store.
    ///
    /// # Errors
    /// Returns [`WebhookError::InvalidRequest`] for an invalid retry contract,
    /// [`WebhookError::CorruptStore`] for an undecodable store, and
    /// [`WebhookError::Io`] when the root cannot be created or read.
    pub fn open(
        root: impl AsRef<Path>,
        transport: T,
        policy: RetryPolicy,
    ) -> Result<Self, WebhookError> {
        let policy = policy.validate()?;
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;
        let loaded = state::load(&root.join(state::STATE_FILE))?;
        Ok(Self {
            root,
            state: Mutex::new(loaded),
            transport,
            policy,
            key_overlap_seconds: DEFAULT_KEY_OVERLAP_SECONDS,
        })
    }

    /// Overrides how long a superseded signing key keeps signing after a
    /// rotation is announced.
    #[must_use]
    pub fn with_key_overlap(mut self, seconds: u64) -> Self {
        self.key_overlap_seconds = seconds;
        self
    }

    /// Returns the retry contract in force.
    #[must_use]
    pub const fn policy(&self) -> RetryPolicy {
        self.policy
    }

    fn lock(&self) -> Result<MutexGuard<'_, State>, WebhookError> {
        self.state.lock().map_err(|_| WebhookError::Unavailable)
    }

    /// Registers one endpoint and publishes the public half of the key its
    /// deliveries are signed under.
    ///
    /// # Errors
    /// Returns [`WebhookError::InvalidRequest`] for an unusable destination or
    /// once the principal holds the maximum number of endpoints, and
    /// [`WebhookError::Entropy`] when secure generation is unavailable.
    pub fn register(
        &self,
        request: &EndpointRequest<'_>,
        now: u64,
    ) -> Result<Registration, WebhookError> {
        validate_url(request.url)?;
        let id = EndpointId::generate()?;
        let key = EndpointKey::generate()?;
        let registration = Registration::issued(&id, &key, None);
        let mut kinds = request.kinds.to_vec();
        kinds.sort_unstable();
        kinds.dedup();
        let endpoint = Endpoint {
            id: id.clone(),
            principal: request.principal.clone(),
            url: request.url.to_owned(),
            kinds,
            minimum_verification: request.minimum_verification,
            key,
            pending_key: None,
            pending_key_activates_at: 0,
            created_at: now,
            key_rotated_at: now,
            suspended: false,
            suspended_reason: None,
            consecutive_dead_letters: 0,
            delivered_total: 0,
            dead_lettered_total: 0,
            last_delivery_at: None,
            last_failure: None,
            last_failure_at: None,
        };
        let mut guard = self.lock()?;
        let held = guard
            .endpoints
            .values()
            .filter(|record| &record.principal == request.principal)
            .count();
        if held >= MAXIMUM_ENDPOINTS {
            return Err(WebhookError::InvalidRequest);
        }
        guard.endpoints.insert(id.as_str().to_owned(), endpoint);
        state::persist(&self.root, &guard)?;
        Ok(registration)
    }

    /// Announces a new signing key. The superseded key keeps signing until the
    /// returned activation time, so a receiver can install the announced public
    /// key before any delivery is signed with it.
    ///
    /// # Errors
    /// Returns [`WebhookError::UnknownEndpoint`] when the principal does not own
    /// the endpoint and [`WebhookError::Entropy`] when generation is unavailable.
    pub fn rotate_key(
        &self,
        principal: &Principal,
        endpoint: &EndpointId,
        now: u64,
    ) -> Result<Registration, WebhookError> {
        let key = EndpointKey::generate()?;
        let activates_at = now.saturating_add(self.key_overlap_seconds);
        let registration = Registration::issued(endpoint, &key, Some(activates_at));
        let mut guard = self.lock()?;
        let record = guard
            .endpoints
            .get_mut(endpoint.as_str())
            .filter(|record| &record.principal == principal)
            .ok_or(WebhookError::UnknownEndpoint)?;
        record.promote_due_key(now);
        record.pending_key = Some(key);
        record.pending_key_activates_at = activates_at;
        record.key_rotated_at = now;
        state::persist(&self.root, &guard)?;
        Ok(registration)
    }

    /// Returns the published signing keys for one endpoint: the key signing now
    /// and any announced successor.
    ///
    /// # Errors
    /// Returns [`WebhookError::UnknownEndpoint`] when the principal does not own
    /// the endpoint.
    pub fn signing_keys(
        &self,
        principal: &Principal,
        endpoint: &EndpointId,
        now: u64,
    ) -> Result<Vec<Registration>, WebhookError> {
        let guard = self.lock()?;
        let record = guard.endpoint_of(principal, endpoint)?;
        let mut published = vec![Registration::issued(
            endpoint,
            record.signing_key(now),
            None,
        )];
        if let Some(pending) = record.pending_key() {
            if pending.id() != record.signing_key(now).id() {
                published.push(Registration::issued(
                    endpoint,
                    pending,
                    Some(record.pending_key_activates_at()),
                ));
            }
        }
        Ok(published)
    }

    /// Suspends delivery to one endpoint, leaving queued work pending.
    ///
    /// # Errors
    /// Returns [`WebhookError::UnknownEndpoint`] when the principal does not own
    /// the endpoint and [`WebhookError::InvalidRequest`] for an unusable reason.
    pub fn suspend(
        &self,
        principal: &Principal,
        endpoint: &EndpointId,
        reason: &str,
        now: u64,
    ) -> Result<(), WebhookError> {
        let mut guard = self.lock()?;
        let record = guard
            .endpoints
            .get_mut(endpoint.as_str())
            .filter(|record| &record.principal == principal)
            .ok_or(WebhookError::UnknownEndpoint)?;
        record.suspended = true;
        record.set_reason(reason)?;
        record.last_failure_at = Some(now);
        state::persist(&self.root, &guard)
    }

    /// Resumes delivery to one endpoint and clears its dead-letter streak.
    ///
    /// # Errors
    /// Returns [`WebhookError::UnknownEndpoint`] when the principal does not own
    /// the endpoint.
    pub fn resume(&self, principal: &Principal, endpoint: &EndpointId) -> Result<(), WebhookError> {
        let mut guard = self.lock()?;
        let record = guard
            .endpoints
            .get_mut(endpoint.as_str())
            .filter(|record| &record.principal == principal)
            .ok_or(WebhookError::UnknownEndpoint)?;
        record.suspended = false;
        record.suspended_reason = None;
        record.consecutive_dead_letters = 0;
        state::persist(&self.root, &guard)
    }

    /// Publishes one protocol event. Republishing the identical event is a
    /// no-op that reports the existing position; republishing a different event
    /// under the same identifier is refused, and a subject sequence that does
    /// not advance is refused.
    ///
    /// # Errors
    /// Returns [`WebhookError::VerificationRequired`] when a fact claims more
    /// than its evidence supports, [`WebhookError::EventConflict`] on a reused
    /// identifier, and [`WebhookError::OrderViolation`] on a stale sequence.
    pub fn publish(&self, event: &ProtocolEvent, now: u64) -> Result<PublishOutcome, WebhookError> {
        event.validate()?;
        let mut guard = self.lock()?;
        if let Some(position) = guard.positions.get(event.id().as_str()).copied() {
            let existing = guard.event_at(position).ok_or(WebhookError::CorruptStore)?;
            if existing != event {
                return Err(WebhookError::EventConflict);
            }
            let queued = guard
                .deliveries
                .values()
                .filter(|delivery| delivery.event.as_str() == event.id().as_str())
                .map(|delivery| delivery.id.as_str().to_owned())
                .collect();
            return Ok(PublishOutcome {
                position,
                duplicate: true,
                queued,
            });
        }
        let subject = state::subject_key(event.principal(), event.subject());
        let reached = guard.high_water.get(&subject).copied().unwrap_or(0);
        if event.subject_sequence() <= reached {
            return Err(WebhookError::OrderViolation);
        }
        let position = guard.next_position();
        guard.events.push(event.clone());
        guard
            .positions
            .insert(event.id().as_str().to_owned(), position);
        guard.high_water.insert(subject, event.subject_sequence());
        let targets: Vec<EndpointId> = guard
            .endpoints
            .values()
            .filter(|record| record.principal == *event.principal())
            .filter(|record| record.accepts(event.kind(), event.verification()))
            .map(|record| record.id.clone())
            .collect();
        let mut queued = Vec::with_capacity(targets.len());
        for endpoint in targets {
            queued.push(enqueue(&mut guard, event, position, now, endpoint, None)?);
        }
        state::persist(&self.root, &guard)?;
        Ok(PublishOutcome {
            position,
            duplicate: false,
            queued,
        })
    }

    /// Runs one bounded dispatch pass over every subject queue. At most one
    /// delivery per subject is ever outstanding, so per-subject order is held.
    ///
    /// # Errors
    /// Returns [`WebhookError::Unavailable`] when the durable state lock is
    /// poisoned and [`WebhookError::Io`] when the store cannot be written.
    pub fn dispatch(&self, now: u64, budget: u32) -> Result<DispatchReport, WebhookError> {
        let budget = usize::try_from(budget).unwrap_or(0);
        let mut report = DispatchReport::default();
        let prepared = {
            let mut guard = self.lock()?;
            let (prepared, blocked) = self.prepare(&mut guard, now, budget);
            report.blocked = blocked;
            if !prepared.is_empty() {
                state::persist(&self.root, &guard)?;
            }
            prepared
        };
        let mut landings = Vec::with_capacity(prepared.len());
        for item in &prepared {
            let started = Instant::now();
            let outcome = self.transport.post(&Attempt {
                url: &item.url,
                headers: &item.headers,
                payload: &item.payload,
            });
            let latency = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            landings.push((classify(outcome), latency));
        }
        let mut guard = self.lock()?;
        for (item, (landing, latency)) in prepared.iter().zip(landings) {
            report.attempted = report.attempted.saturating_add(1);
            self.settle(&mut guard, item, landing, latency, now, &mut report);
        }
        report.next_attempt_at = next_attempt_at(&guard, self.policy);
        if report.attempted > 0 {
            state::persist(&self.root, &guard)?;
        }
        Ok(report)
    }

    fn prepare(&self, guard: &mut State, now: u64, budget: usize) -> (Vec<Prepared>, u32) {
        let mut prepared = Vec::new();
        let mut blocked = 0_u32;
        let keys: Vec<String> = guard.queues.keys().cloned().collect();
        for key in keys {
            retire_terminal(guard, &key);
            let Some(head) = guard.queues.get(&key).and_then(VecDeque::front).cloned() else {
                guard.queues.remove(&key);
                continue;
            };
            if prepared.len() >= budget {
                blocked = blocked.saturating_add(1);
                continue;
            }
            match self.prepare_one(guard, &key, &head, now) {
                Ready::Prepared(item) => prepared.push(item),
                Ready::Blocked => blocked = blocked.saturating_add(1),
                Ready::Skip => {}
            }
        }
        (prepared, blocked)
    }

    fn prepare_one(&self, guard: &mut State, queue: &str, id: &str, now: u64) -> Ready {
        let Some(delivery) = guard.deliveries.get(id) else {
            return Ready::Skip;
        };
        let Some(due) = delivery.due_at(self.policy.in_flight_timeout_seconds) else {
            return Ready::Skip;
        };
        if due > now {
            return Ready::Blocked;
        }
        let attempt = delivery.state.attempts().saturating_add(1);
        let position = delivery.log_position;
        let endpoint_id = delivery.endpoint.clone();
        let subject = delivery.subject.clone();
        let subject_sequence = delivery.subject_sequence;
        let event_id = delivery.event.clone();
        let Some(endpoint) = guard.endpoints.get_mut(endpoint_id.as_str()) else {
            return Ready::Skip;
        };
        if endpoint.suspended {
            return Ready::Blocked;
        }
        endpoint.promote_due_key(now);
        let url = endpoint.url.clone();
        let key = endpoint.signing_key(now).clone();
        let Some(event) = guard.event_at(position) else {
            return Ready::Skip;
        };
        let kind = event.kind();
        let Ok(payload) = envelope_bytes(&endpoint_id, event) else {
            return Ready::Skip;
        };
        let signature = scheme::sign(&key, event_id.as_str(), now, &payload);
        let headers = vec![
            ("Content-Type".to_owned(), "application/json".to_owned()),
            (scheme::ID_HEADER.to_owned(), event_id.as_str().to_owned()),
            (scheme::TIMESTAMP_HEADER.to_owned(), now.to_string()),
            (scheme::KEY_HEADER.to_owned(), key.id().to_owned()),
            (scheme::SIGNATURE_HEADER.to_owned(), signature),
            (scheme::DELIVERY_HEADER.to_owned(), id.to_owned()),
            (scheme::KIND_HEADER.to_owned(), kind.as_str().to_owned()),
            (
                scheme::SUBJECT_HEADER.to_owned(),
                subject.as_str().to_owned(),
            ),
            (
                scheme::SEQUENCE_HEADER.to_owned(),
                subject_sequence.to_string(),
            ),
            (scheme::ATTEMPT_HEADER.to_owned(), attempt.to_string()),
            (
                scheme::ENDPOINT_HEADER.to_owned(),
                endpoint_id.as_str().to_owned(),
            ),
        ];
        if let Some(record) = guard.deliveries.get_mut(id) {
            record.state = DeliveryState::InFlight {
                attempt,
                started_at: now,
            };
        }
        Ready::Prepared(Prepared {
            queue: queue.to_owned(),
            delivery: id.to_owned(),
            endpoint: endpoint_id.as_str().to_owned(),
            url,
            headers,
            payload,
            attempt,
        })
    }

    fn settle(
        &self,
        guard: &mut State,
        item: &Prepared,
        landing: Landing,
        latency_ms: u64,
        now: u64,
        report: &mut DispatchReport,
    ) {
        let (status, failure) = match landing {
            Landing::Accepted(status) => (Some(status), None),
            Landing::Failed { failure, status } => (status, Some(failure)),
        };
        if let Some(delivery) = guard.deliveries.get_mut(&item.delivery) {
            delivery.attempts.push(AttemptRecord {
                attempt: item.attempt,
                at: now,
                status,
                failure,
                latency_ms,
            });
        }
        match landing {
            Landing::Accepted(status) => {
                if let Some(delivery) = guard.deliveries.get_mut(&item.delivery) {
                    delivery.state = DeliveryState::Delivered {
                        attempt: item.attempt,
                        at: now,
                        status,
                    };
                }
                if let Some(endpoint) = guard.endpoints.get_mut(&item.endpoint) {
                    endpoint.delivered_total = endpoint.delivered_total.saturating_add(1);
                    endpoint.consecutive_dead_letters = 0;
                    endpoint.last_delivery_at = Some(now);
                }
                retire(guard, &item.queue, &item.delivery);
                report.delivered = report.delivered.saturating_add(1);
            }
            Landing::Failed { failure, status } => {
                self.settle_failure(guard, item, failure, status, now, report);
            }
        }
    }

    fn settle_failure(
        &self,
        guard: &mut State,
        item: &Prepared,
        failure: FailureKind,
        status: Option<u16>,
        now: u64,
        report: &mut DispatchReport,
    ) {
        let exhausted = failure.permanent() || item.attempt >= self.policy.maximum_attempts;
        let text = failure_text(failure, status);
        if exhausted {
            if let Some(delivery) = guard.deliveries.get_mut(&item.delivery) {
                delivery.state = DeliveryState::DeadLettered {
                    attempts: item.attempt,
                    at: now,
                    failure,
                    status,
                };
            }
            guard.dead_letters.push(item.delivery.clone());
            let suspend_after = self.policy.suspend_after_dead_letters;
            if let Some(endpoint) = guard.endpoints.get_mut(&item.endpoint) {
                endpoint.dead_lettered_total = endpoint.dead_lettered_total.saturating_add(1);
                endpoint.consecutive_dead_letters =
                    endpoint.consecutive_dead_letters.saturating_add(1);
                endpoint.last_failure = Some(text);
                endpoint.last_failure_at = Some(now);
                if endpoint.consecutive_dead_letters >= suspend_after {
                    endpoint.suspended = true;
                    endpoint.suspended_reason =
                        Some("consecutive dead letters reached the suspension bound".to_owned());
                }
            }
            retire(guard, &item.queue, &item.delivery);
            report.dead_lettered = report.dead_lettered.saturating_add(1);
            return;
        }
        let delay = self
            .policy
            .backoff_seconds(item.attempt, &digest(item.delivery.as_bytes()));
        if let Some(delivery) = guard.deliveries.get_mut(&item.delivery) {
            delivery.state = DeliveryState::Retrying {
                attempt: item.attempt,
                next_attempt_at: now.saturating_add(delay),
                failure,
                status,
            };
        }
        if let Some(endpoint) = guard.endpoints.get_mut(&item.endpoint) {
            endpoint.last_failure = Some(text);
            endpoint.last_failure_at = Some(now);
        }
        report.retrying = report.retrying.saturating_add(1);
    }

    /// Returns the events an endpoint is eligible to receive after a cursor.
    ///
    /// # Errors
    /// Returns [`WebhookError::UnknownEndpoint`], [`WebhookError::InvalidCursor`]
    /// for a cursor issued elsewhere, and [`WebhookError::CursorExpired`] when
    /// retention has already released the named position.
    pub fn events_since(
        &self,
        principal: &Principal,
        endpoint: &EndpointId,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<EventPage, WebhookError> {
        let guard = self.lock()?;
        let record = guard.endpoint_of(principal, endpoint)?;
        let from = resume_from(&guard, endpoint, cursor)?;
        let page = scan(
            &guard,
            principal,
            record,
            from,
            limit.clamp(1, MAXIMUM_PAGE),
        );
        let events = page
            .positions
            .iter()
            .filter_map(|position| guard.event_at(*position).cloned())
            .collect();
        Ok(EventPage {
            events,
            next_cursor: cursor::encode(endpoint, page.cursor_position),
            has_more: page.has_more,
        })
    }

    /// Queues fresh deliveries for the missed events after a cursor. The new
    /// deliveries carry new delivery identifiers and the original event
    /// identifiers, so receivers deduplicate them exactly as they deduplicate
    /// any other at-least-once repeat.
    ///
    /// # Errors
    /// Returns [`WebhookError::UnknownEndpoint`], [`WebhookError::InvalidCursor`],
    /// [`WebhookError::CursorExpired`] and [`WebhookError::Entropy`].
    pub fn redeliver(
        &self,
        principal: &Principal,
        endpoint: &EndpointId,
        cursor: Option<&str>,
        limit: usize,
        now: u64,
    ) -> Result<RedeliveryOutcome, WebhookError> {
        let mut guard = self.lock()?;
        let record = guard.endpoint_of(principal, endpoint)?;
        let from = resume_from(&guard, endpoint, cursor)?;
        let page = scan(
            &guard,
            principal,
            record,
            from,
            limit.clamp(1, MAXIMUM_PAGE),
        );
        let selected: Vec<(u64, ProtocolEvent)> = page
            .positions
            .iter()
            .filter_map(|position| {
                guard
                    .event_at(*position)
                    .map(|event| (*position, event.clone()))
            })
            .collect();
        let mut queued = Vec::with_capacity(selected.len());
        for (position, event) in selected {
            queued.push(enqueue(
                &mut guard,
                &event,
                position,
                now,
                endpoint.clone(),
                None,
            )?);
        }
        state::persist(&self.root, &guard)?;
        Ok(RedeliveryOutcome {
            queued,
            next_cursor: cursor::encode(endpoint, page.cursor_position),
            has_more: page.has_more,
        })
    }

    /// Queues one fresh delivery for a dead-lettered delivery's event.
    ///
    /// # Errors
    /// Returns [`WebhookError::UnknownDelivery`] when the principal does not own
    /// it, [`WebhookError::NotDeadLettered`] when it is still live, and
    /// [`WebhookError::CursorExpired`] when its event has been released.
    pub fn replay_dead_letter(
        &self,
        principal: &Principal,
        delivery: &DeliveryId,
        now: u64,
    ) -> Result<String, WebhookError> {
        let mut guard = self.lock()?;
        let record = guard
            .deliveries
            .get(delivery.as_str())
            .ok_or(WebhookError::UnknownDelivery)?;
        if !matches!(record.state, DeliveryState::DeadLettered { .. }) {
            return Err(WebhookError::NotDeadLettered);
        }
        let endpoint = record.endpoint.clone();
        let position = record.log_position;
        let owned = guard
            .endpoints
            .get(endpoint.as_str())
            .is_some_and(|record| &record.principal == principal);
        if !owned {
            return Err(WebhookError::UnknownDelivery);
        }
        let event = guard
            .event_at(position)
            .cloned()
            .ok_or(WebhookError::CursorExpired)?;
        let queued = enqueue(
            &mut guard,
            &event,
            position,
            now,
            endpoint,
            Some(delivery.clone()),
        )?;
        state::persist(&self.root, &guard)?;
        Ok(queued)
    }

    /// Releases the oldest events once every delivery of them was accepted,
    /// keeping at least the requested number of events addressable by cursor.
    ///
    /// # Errors
    /// Returns [`WebhookError::Unavailable`] when the lock is poisoned and
    /// [`WebhookError::Io`] when the store cannot be written.
    pub fn prune(&self, keep_events: usize) -> Result<usize, WebhookError> {
        let mut guard = self.lock()?;
        let mut released = 0_usize;
        while guard.events.len() > keep_events {
            let position = guard.first_position;
            let clear = guard
                .deliveries
                .values()
                .filter(|delivery| delivery.log_position == position)
                .all(|delivery| matches!(delivery.state, DeliveryState::Delivered { .. }));
            if !clear {
                break;
            }
            let retired: Vec<String> = guard
                .deliveries
                .values()
                .filter(|delivery| delivery.log_position == position)
                .map(|delivery| delivery.id.as_str().to_owned())
                .collect();
            for id in retired {
                guard.deliveries.remove(&id);
            }
            let event = guard.events.remove(0);
            guard.positions.remove(event.id().as_str());
            guard.first_position = guard.first_position.saturating_add(1);
            released = released.saturating_add(1);
        }
        if released > 0 {
            state::persist(&self.root, &guard)?;
        }
        Ok(released)
    }

    /// Returns delivery health for every endpoint the principal owns.
    ///
    /// # Errors
    /// Returns [`WebhookError::Unavailable`] when the lock is poisoned.
    pub fn health(
        &self,
        principal: &Principal,
        now: u64,
    ) -> Result<Vec<EndpointHealth>, WebhookError> {
        let guard = self.lock()?;
        Ok(state::endpoint_health(&guard, principal, now, self.policy))
    }

    /// Returns the most recent deliveries the principal owns.
    ///
    /// # Errors
    /// Returns [`WebhookError::Unavailable`] when the lock is poisoned.
    pub fn deliveries(
        &self,
        principal: &Principal,
        endpoint: Option<&EndpointId>,
        limit: usize,
    ) -> Result<Vec<DeliveryRecord>, WebhookError> {
        let guard = self.lock()?;
        Ok(state::delivery_records(
            &guard,
            principal,
            endpoint,
            limit.min(MAXIMUM_PAGE),
        ))
    }

    /// Returns the dead-letter path for the principal, newest first.
    ///
    /// # Errors
    /// Returns [`WebhookError::Unavailable`] when the lock is poisoned.
    pub fn dead_letters(
        &self,
        principal: &Principal,
        limit: usize,
    ) -> Result<Vec<DeliveryRecord>, WebhookError> {
        let guard = self.lock()?;
        Ok(state::dead_letter_records(
            &guard,
            principal,
            limit.min(MAXIMUM_PAGE),
        ))
    }

    /// Returns the retained protocol events the principal owns, newest first.
    ///
    /// # Errors
    /// Returns [`WebhookError::Unavailable`] when the lock is poisoned.
    pub fn events(
        &self,
        principal: &Principal,
        kind: Option<EventKind>,
        limit: usize,
    ) -> Result<Vec<ProtocolEvent>, WebhookError> {
        let guard = self.lock()?;
        Ok(state::event_records(
            &guard,
            principal,
            kind,
            limit.min(MAXIMUM_PAGE),
        ))
    }
}

struct Page {
    positions: Vec<u64>,
    cursor_position: u64,
    has_more: bool,
}

fn resume_from(
    guard: &State,
    endpoint: &EndpointId,
    cursor: Option<&str>,
) -> Result<u64, WebhookError> {
    let position = match cursor {
        Some(value) => cursor::decode(endpoint, value)?,
        None => 0,
    };
    let from = position.saturating_add(1);
    if from < guard.first_position {
        return Err(WebhookError::CursorExpired);
    }
    Ok(from)
}

fn scan(
    guard: &State,
    principal: &Principal,
    endpoint: &Endpoint,
    from: u64,
    limit: usize,
) -> Page {
    let mut positions = Vec::new();
    let mut cursor_position = from.saturating_sub(1);
    let mut has_more = false;
    let mut current = from.max(guard.first_position);
    while current < guard.next_position() {
        let Some(event) = guard.event_at(current) else {
            break;
        };
        let eligible =
            event.principal() == principal && endpoint.accepts(event.kind(), event.verification());
        if eligible {
            if positions.len() >= limit {
                has_more = true;
                break;
            }
            positions.push(current);
        }
        cursor_position = current;
        current = current.saturating_add(1);
    }
    Page {
        positions,
        cursor_position,
        has_more,
    }
}

fn enqueue(
    guard: &mut State,
    event: &ProtocolEvent,
    position: u64,
    now: u64,
    endpoint: EndpointId,
    replay_of: Option<DeliveryId>,
) -> Result<String, WebhookError> {
    let id = DeliveryId::generate()?;
    let key = state::queue_key(&endpoint, event.subject());
    let delivery = Delivery {
        id: id.clone(),
        endpoint,
        event: event.id().clone(),
        subject: event.subject().clone(),
        subject_sequence: event.subject_sequence(),
        log_position: position,
        created_at: now,
        state: DeliveryState::Pending,
        attempts: Vec::new(),
        replay_of,
    };
    let wire = id.as_str().to_owned();
    guard.deliveries.insert(wire.clone(), delivery);
    guard.queues.entry(key).or_default().push_back(wire.clone());
    Ok(wire)
}

fn retire_terminal(guard: &mut State, key: &str) {
    while let Some(front) = guard.queues.get(key).and_then(VecDeque::front).cloned() {
        let finished = guard
            .deliveries
            .get(&front)
            .is_none_or(|delivery| delivery.state.terminal());
        if !finished {
            break;
        }
        if let Some(queue) = guard.queues.get_mut(key) {
            queue.pop_front();
        }
    }
}

fn retire(guard: &mut State, key: &str, id: &str) {
    if let Some(queue) = guard.queues.get_mut(key) {
        if queue.front().is_some_and(|front| front == id) {
            queue.pop_front();
        }
    }
}

fn next_attempt_at(guard: &State, policy: RetryPolicy) -> Option<u64> {
    let mut soonest: Option<u64> = None;
    for queue in guard.queues.values() {
        let Some(front) = queue.front() else {
            continue;
        };
        let Some(delivery) = guard.deliveries.get(front) else {
            continue;
        };
        if let Some(due) = delivery.due_at(policy.in_flight_timeout_seconds) {
            soonest = Some(soonest.map_or(due, |current: u64| current.min(due)));
        }
    }
    soonest
}

fn classify(outcome: Result<u16, FailureKind>) -> Landing {
    match outcome {
        Ok(status) if (200..300).contains(&status) => Landing::Accepted(status),
        Ok(410) => Landing::Failed {
            failure: FailureKind::Gone,
            status: Some(410),
        },
        Ok(status) => Landing::Failed {
            failure: FailureKind::Refused,
            status: Some(status),
        },
        Err(failure) => Landing::Failed {
            failure,
            status: None,
        },
    }
}

fn failure_text(failure: FailureKind, status: Option<u16>) -> String {
    status.map_or_else(
        || failure.as_str().to_owned(),
        |code| format!("{} ({code})", failure.as_str()),
    )
}
