//! The durable webhook store and the read-only projections the developer
//! dashboards render from it.

use std::collections::{BTreeMap, VecDeque};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::deliveries::{Delivery, DeliveryRecord, DeliveryState};
use crate::endpoints::{Endpoint, EndpointHealth, RetryPolicy};
use crate::error::WebhookError;
use crate::events::{EndpointId, EventKind, Principal, ProtocolEvent, SubjectId};
use crate::scheme::EndpointKey;

/// Name of the durable state file inside the configured root.
pub const STATE_FILE: &str = "webhook-state.json";

const TEMPORARY_FILE: &str = "webhook-state.json.tmp";

#[derive(Serialize, Deserialize)]
pub(crate) struct State {
    pub(crate) first_position: u64,
    pub(crate) endpoints: BTreeMap<String, Endpoint>,
    pub(crate) events: Vec<ProtocolEvent>,
    pub(crate) positions: BTreeMap<String, u64>,
    pub(crate) deliveries: BTreeMap<String, Delivery>,
    pub(crate) queues: BTreeMap<String, VecDeque<String>>,
    pub(crate) dead_letters: Vec<String>,
    pub(crate) high_water: BTreeMap<String, u64>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            first_position: 1,
            endpoints: BTreeMap::new(),
            events: Vec::new(),
            positions: BTreeMap::new(),
            deliveries: BTreeMap::new(),
            queues: BTreeMap::new(),
            dead_letters: Vec::new(),
            high_water: BTreeMap::new(),
        }
    }
}

impl State {
    pub(crate) fn next_position(&self) -> u64 {
        self.first_position
            .saturating_add(self.events.len().try_into().unwrap_or(u64::MAX))
    }

    pub(crate) fn event_at(&self, position: u64) -> Option<&ProtocolEvent> {
        let offset = position.checked_sub(self.first_position)?;
        self.events.get(usize::try_from(offset).ok()?)
    }

    pub(crate) fn endpoint_of(
        &self,
        principal: &Principal,
        endpoint: &EndpointId,
    ) -> Result<&Endpoint, WebhookError> {
        self.endpoints
            .get(endpoint.as_str())
            .filter(|record| &record.principal == principal)
            .ok_or(WebhookError::UnknownEndpoint)
    }
}

pub(crate) fn queue_key(endpoint: &EndpointId, subject: &SubjectId) -> String {
    format!("{}|{}", endpoint.as_str(), subject.as_str())
}

pub(crate) fn subject_key(principal: &Principal, subject: &SubjectId) -> String {
    format!("{}|{}", principal.as_str(), subject.as_str())
}

pub(crate) fn load(path: &Path) -> Result<State, WebhookError> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| WebhookError::CorruptStore),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(State::default()),
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn persist(root: &Path, state: &State) -> Result<(), WebhookError> {
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;

    let bytes = serde_json::to_vec(state).map_err(|_| WebhookError::CorruptStore)?;
    let temporary = root.join(TEMPORARY_FILE);
    let target = root.join(STATE_FILE);
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&temporary)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    fs::rename(temporary, target)?;
    Ok(())
}

fn record_for(state: &State, delivery: &Delivery) -> Option<DeliveryRecord> {
    let event = state.event_at(delivery.log_position)?;
    Some(DeliveryRecord {
        delivery: delivery.id.as_str().to_owned(),
        endpoint: delivery.endpoint.as_str().to_owned(),
        event: delivery.event.as_str().to_owned(),
        kind: event.kind(),
        subject: delivery.subject.as_str().to_owned(),
        subject_sequence: delivery.subject_sequence,
        log_position: delivery.log_position,
        created_at: delivery.created_at,
        state: delivery.state,
        attempts: delivery.attempts.clone(),
        verification: event.verification(),
        receipt_digest: event.receipt_digest().map(str::to_owned),
        replay_of: delivery.replay_of.as_ref().map(|id| id.as_str().to_owned()),
    })
}

fn owned_by(state: &State, delivery: &Delivery, principal: &Principal) -> bool {
    state
        .endpoints
        .get(delivery.endpoint.as_str())
        .is_some_and(|endpoint| &endpoint.principal == principal)
}

pub(crate) fn delivery_records(
    state: &State,
    principal: &Principal,
    endpoint: Option<&EndpointId>,
    limit: usize,
) -> Vec<DeliveryRecord> {
    let mut selected: Vec<&Delivery> = state
        .deliveries
        .values()
        .filter(|delivery| owned_by(state, delivery, principal))
        .filter(|delivery| endpoint.is_none_or(|wanted| &delivery.endpoint == wanted))
        .collect();
    selected.sort_by(|left, right| {
        right
            .log_position
            .cmp(&left.log_position)
            .then_with(|| right.created_at.cmp(&left.created_at))
            .then_with(|| left.id.as_str().cmp(right.id.as_str()))
    });
    selected
        .into_iter()
        .take(limit)
        .filter_map(|delivery| record_for(state, delivery))
        .collect()
}

pub(crate) fn dead_letter_records(
    state: &State,
    principal: &Principal,
    limit: usize,
) -> Vec<DeliveryRecord> {
    state
        .dead_letters
        .iter()
        .rev()
        .filter_map(|id| state.deliveries.get(id))
        .filter(|delivery| owned_by(state, delivery, principal))
        .take(limit)
        .filter_map(|delivery| record_for(state, delivery))
        .collect()
}

pub(crate) fn event_records(
    state: &State,
    principal: &Principal,
    kind: Option<EventKind>,
    limit: usize,
) -> Vec<ProtocolEvent> {
    state
        .events
        .iter()
        .rev()
        .filter(|event| event.principal() == principal)
        .filter(|event| kind.is_none_or(|wanted| event.kind() == wanted))
        .take(limit)
        .cloned()
        .collect()
}

#[derive(Default)]
struct Counters {
    pending: u64,
    in_flight: u64,
    retrying: u64,
    oldest_undelivered: Option<u64>,
    next_attempt_at: Option<u64>,
}

fn counters(state: &State, endpoint: &EndpointId, policy: RetryPolicy) -> Counters {
    let mut counters = Counters::default();
    for delivery in state
        .deliveries
        .values()
        .filter(|delivery| &delivery.endpoint == endpoint)
    {
        match delivery.state {
            DeliveryState::Pending => counters.pending = counters.pending.saturating_add(1),
            DeliveryState::InFlight { .. } => {
                counters.in_flight = counters.in_flight.saturating_add(1);
            }
            DeliveryState::Retrying { .. } => {
                counters.retrying = counters.retrying.saturating_add(1);
            }
            DeliveryState::Delivered { .. } | DeliveryState::DeadLettered { .. } => continue,
        }
        counters.oldest_undelivered = Some(
            counters
                .oldest_undelivered
                .map_or(delivery.created_at, |current: u64| {
                    current.min(delivery.created_at)
                }),
        );
        if let Some(due) = delivery.due_at(policy.in_flight_timeout_seconds) {
            counters.next_attempt_at = Some(
                counters
                    .next_attempt_at
                    .map_or(due, |current: u64| current.min(due)),
            );
        }
    }
    counters
}

pub(crate) fn endpoint_health(
    state: &State,
    principal: &Principal,
    now: u64,
    policy: RetryPolicy,
) -> Vec<EndpointHealth> {
    state
        .endpoints
        .values()
        .filter(|endpoint| &endpoint.principal == principal)
        .map(|endpoint| {
            let counters = counters(state, &endpoint.id, policy);
            EndpointHealth {
                endpoint: endpoint.id.as_str().to_owned(),
                url: endpoint.url.clone(),
                kinds: endpoint.kinds.clone(),
                minimum_verification: endpoint.minimum_verification,
                suspended: endpoint.suspended,
                suspended_reason: endpoint.suspended_reason.clone(),
                pending: counters.pending,
                in_flight: counters.in_flight,
                retrying: counters.retrying,
                delivered_total: endpoint.delivered_total,
                dead_lettered_total: endpoint.dead_lettered_total,
                consecutive_dead_letters: endpoint.consecutive_dead_letters,
                oldest_undelivered_seconds: counters
                    .oldest_undelivered
                    .map_or(0, |created| now.saturating_sub(created)),
                next_attempt_at: counters.next_attempt_at,
                last_delivery_at: endpoint.last_delivery_at,
                last_failure: endpoint.last_failure.clone(),
                last_failure_at: endpoint.last_failure_at,
                key_id: endpoint.key.id().to_owned(),
                public_key: endpoint.key.public_key_base64(),
                key_rotated_at: endpoint.key_rotated_at,
                pending_key_id: endpoint
                    .pending_key()
                    .map(|pending| pending.id().to_owned()),
                pending_public_key: endpoint.pending_key().map(EndpointKey::public_key_base64),
                pending_key_activates_at: endpoint
                    .pending_key()
                    .map(|_| endpoint.pending_key_activates_at()),
            }
        })
        .collect()
}

/// A read-only reader over the durable webhook store, for processes that render
/// delivery health without ever dispatching.
pub struct Ledger {
    path: PathBuf,
    policy: RetryPolicy,
}

impl Ledger {
    /// Opens the durable store for reading.
    ///
    /// # Errors
    /// Returns [`WebhookError::InvalidRequest`] when the root is not a directory
    /// and [`WebhookError::InvalidRequest`] when the retry contract is invalid.
    pub fn open(root: impl AsRef<Path>, policy: RetryPolicy) -> Result<Self, WebhookError> {
        let root = root.as_ref();
        if !root.is_dir() {
            return Err(WebhookError::InvalidRequest);
        }
        Ok(Self {
            path: root.join(STATE_FILE),
            policy: policy.validate()?,
        })
    }

    /// Reads delivery health for every endpoint the principal owns.
    ///
    /// # Errors
    /// Returns [`WebhookError::CorruptStore`] or [`WebhookError::Io`] when the
    /// durable store cannot be read.
    pub fn health(
        &self,
        principal: &Principal,
        now: u64,
    ) -> Result<Vec<EndpointHealth>, WebhookError> {
        let state = load(&self.path)?;
        Ok(endpoint_health(&state, principal, now, self.policy))
    }

    /// Reads the most recent deliveries the principal owns.
    ///
    /// # Errors
    /// Returns [`WebhookError::CorruptStore`] or [`WebhookError::Io`] when the
    /// durable store cannot be read.
    pub fn deliveries(
        &self,
        principal: &Principal,
        endpoint: Option<&EndpointId>,
        limit: usize,
    ) -> Result<Vec<DeliveryRecord>, WebhookError> {
        let state = load(&self.path)?;
        Ok(delivery_records(&state, principal, endpoint, limit))
    }

    /// Reads the dead-letter path for the principal, newest first.
    ///
    /// # Errors
    /// Returns [`WebhookError::CorruptStore`] or [`WebhookError::Io`] when the
    /// durable store cannot be read.
    pub fn dead_letters(
        &self,
        principal: &Principal,
        limit: usize,
    ) -> Result<Vec<DeliveryRecord>, WebhookError> {
        let state = load(&self.path)?;
        Ok(dead_letter_records(&state, principal, limit))
    }

    /// Reads the retained protocol events the principal owns, newest first,
    /// each carrying the verification status of every fact it displays.
    ///
    /// # Errors
    /// Returns [`WebhookError::CorruptStore`] or [`WebhookError::Io`] when the
    /// durable store cannot be read.
    pub fn events(
        &self,
        principal: &Principal,
        kind: Option<EventKind>,
        limit: usize,
    ) -> Result<Vec<ProtocolEvent>, WebhookError> {
        let state = load(&self.path)?;
        Ok(event_records(&state, principal, kind, limit))
    }
}
