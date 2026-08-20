//! Tenant-bound session lifecycle and daemon-only authentication tokens.

use std::collections::{BTreeMap, BTreeSet};

use layerx_types::ids::Did;

use crate::identity::{IdentityRecord, ProtocolAuthority};
use crate::store::{ObjectKind, Store, StoreError, TenantId, TenantKey};

#[path = "session_revocation.rs"]
mod revocation;

pub use revocation::{
    InvalidationReason, InvalidationReport, PendingActivity, PreparationState, RevocationEvent,
};

/// Stable session identifier supplied by the daemon's secure identifier source.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SessionId(pub [u8; 32]);

/// Complete request required to open a session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenRequest {
    pub session_id: SessionId,
    pub token_id: [u8; 32],
    pub tenant: TenantId,
    pub agent: Did,
    pub authority: ProtocolAuthority,
    pub permitted_activity_types: BTreeSet<u16>,
    pub scopes: BTreeSet<String>,
    pub expiry_sequence: u64,
    pub opening_client: String,
    pub policy_version: String,
}

/// A daemon authenticator. It is never accepted as protocol authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Token {
    id: [u8; 32],
    session_id: SessionId,
    tenant: TenantId,
    agent: Did,
    scopes: BTreeSet<String>,
    expiry_sequence: u64,
}

impl Token {
    /// Checks tenant, agent, scope and core-relative expiry.
    ///
    /// # Errors
    ///
    /// Returns `WrongPrincipal` for a mismatched tenant or agent, `Expired` once the core sequence
    /// reaches the token's expiry, and `ScopeDenied` for a scope the token does not carry.
    pub fn authorize(
        &self,
        tenant: &TenantId,
        agent: &Did,
        scope: &str,
        core_sequence: u64,
    ) -> Result<SessionId, SessionError> {
        if &self.tenant != tenant || &self.agent != agent {
            return Err(SessionError::WrongPrincipal);
        }
        if core_sequence >= self.expiry_sequence {
            return Err(SessionError::Expired);
        }
        if !self.scopes.contains(scope) {
            return Err(SessionError::ScopeDenied);
        }
        Ok(self.session_id)
    }

    /// Returns the daemon token identifier for audit correlation.
    #[must_use]
    pub const fn token_id(&self) -> [u8; 32] {
        self.id
    }

    pub(crate) const fn tenant(&self) -> &TenantId {
        &self.tenant
    }

    pub(crate) const fn agent(&self) -> &Did {
        &self.agent
    }
}

/// Durable session record, including the protocol authority actually used.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionRecord {
    pub request: OpenRequest,
    pub open: bool,
    pub sequence: u64,
    pub budget_reserved: u128,
    pub subscription_cursor: u64,
}

/// In-memory index backed by the tenant-scoped durable store.
#[derive(Default)]
pub struct SessionRegistry {
    records: BTreeMap<SessionId, SessionRecord>,
}

impl SessionRegistry {
    #[must_use]
    pub fn get(&self, id: SessionId) -> Option<&SessionRecord> {
        self.records.get(&id)
    }

    #[must_use]
    pub fn open_count(&self) -> usize {
        self.records.values().filter(|record| record.open).count()
    }

    pub(crate) fn records_mut(&mut self) -> &mut BTreeMap<SessionId, SessionRecord> {
        &mut self.records
    }
}

/// Session refusal taxonomy suitable for audit recording.
#[derive(Debug)]
pub enum SessionError {
    MissingField(&'static str),
    IdentityMismatch,
    AuthorityMissing,
    Expired,
    WrongPrincipal,
    ScopeDenied,
    NotFound,
    AlreadyClosed,
    Store(StoreError),
}

impl PartialEq for SessionError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::MissingField(left), Self::MissingField(right)) => left == right,
            (Self::Store(_), Self::Store(_)) => true,
            _ => std::mem::discriminant(self) == std::mem::discriminant(other),
        }
    }
}

impl Eq for SessionError {}

impl From<StoreError> for SessionError {
    fn from(value: StoreError) -> Self {
        Self::Store(value)
    }
}

/// Opens and durably records one independently scoped session.
///
/// # Errors
///
/// Returns `IdentityMismatch`, `MissingField`, `Expired` or `AuthorityMissing` from request
/// validation, or `Store` when the session record cannot be encoded or durably written; the
/// registry is left untouched unless the record persisted.
pub fn open(
    store: &mut Store,
    registry: &mut SessionRegistry,
    identity: &IdentityRecord,
    request: OpenRequest,
    core_sequence: u64,
) -> Result<Token, SessionError> {
    validate_request(identity, &request, core_sequence)?;
    let token = Token {
        id: request.token_id,
        session_id: request.session_id,
        tenant: request.tenant.clone(),
        agent: request.agent.clone(),
        scopes: request.scopes.clone(),
        expiry_sequence: request.expiry_sequence,
    };
    let record = SessionRecord {
        request,
        open: true,
        sequence: 0,
        budget_reserved: 0,
        subscription_cursor: 0,
    };
    persist_record(store, &record)?;
    registry.records.insert(record.request.session_id, record);
    Ok(token)
}

/// Closes exactly one session without disturbing any sibling state.
///
/// # Errors
///
/// Returns `NotFound` for a session the registry never held, `AlreadyClosed` for one already
/// closed, or `Store` when the closed record cannot be persisted.
pub fn close(
    store: &mut Store,
    registry: &mut SessionRegistry,
    session_id: SessionId,
) -> Result<(), SessionError> {
    let existing = registry
        .records
        .get(&session_id)
        .cloned()
        .ok_or(SessionError::NotFound)?;
    if !existing.open {
        return Err(SessionError::AlreadyClosed);
    }
    let mut closed = existing;
    closed.open = false;
    persist_record(store, &closed)?;
    registry.records.insert(session_id, closed);
    Ok(())
}

/// Applies a core revocation event to sessions and unsubmitted preparations.
///
/// # Errors
///
/// Returns `Store` when a revoked session's closed record cannot be persisted, or `MissingField`
/// when its opening client or policy version exceeds the `u16` length prefix.
pub fn invalidate_on_revocation(
    store: &mut Store,
    registry: &mut SessionRegistry,
    activities: &mut [PendingActivity],
    event: &RevocationEvent,
) -> Result<InvalidationReport, SessionError> {
    revocation::apply_revocation(store, registry, activities, event)
}

fn validate_request(
    identity: &IdentityRecord,
    request: &OpenRequest,
    core_sequence: u64,
) -> Result<(), SessionError> {
    if identity.tenant() != &request.tenant || identity.did() != &request.agent {
        return Err(SessionError::IdentityMismatch);
    }
    if request.permitted_activity_types.is_empty() {
        return Err(SessionError::MissingField("permitted_activity_types"));
    }
    if request.scopes.is_empty() {
        return Err(SessionError::MissingField("scopes"));
    }
    if request.opening_client.is_empty() {
        return Err(SessionError::MissingField("opening_client"));
    }
    if request.policy_version.is_empty() {
        return Err(SessionError::MissingField("policy_version"));
    }
    if request.expiry_sequence <= core_sequence {
        return Err(SessionError::Expired);
    }
    if !identity.authorities().contains(&request.authority) {
        return Err(SessionError::AuthorityMissing);
    }
    Ok(())
}

pub(crate) fn persist_record(
    store: &mut Store,
    record: &SessionRecord,
) -> Result<(), SessionError> {
    let key = TenantKey::new(
        record.request.tenant.clone(),
        ObjectKind::Session,
        record.request.session_id.0.to_vec(),
    )?;
    let encoded = encode(record)?;
    store.put_local(key, encoded)?;
    Ok(())
}

fn encode(record: &SessionRecord) -> Result<Vec<u8>, SessionError> {
    let client_len = u16::try_from(record.request.opening_client.len())
        .map_err(|_| SessionError::MissingField("opening_client"))?;
    let policy_len = u16::try_from(record.request.policy_version.len())
        .map_err(|_| SessionError::MissingField("policy_version"))?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&record.request.session_id.0);
    bytes.extend_from_slice(&record.request.token_id);
    bytes.extend_from_slice(&record.request.expiry_sequence.to_be_bytes());
    bytes.push(u8::from(record.open));
    bytes.extend_from_slice(&record.sequence.to_be_bytes());
    bytes.extend_from_slice(&record.budget_reserved.to_be_bytes());
    bytes.extend_from_slice(&record.subscription_cursor.to_be_bytes());
    bytes.extend_from_slice(&client_len.to_be_bytes());
    bytes.extend_from_slice(record.request.opening_client.as_bytes());
    bytes.extend_from_slice(&policy_len.to_be_bytes());
    bytes.extend_from_slice(record.request.policy_version.as_bytes());
    Ok(bytes)
}
