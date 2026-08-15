//! Durable tenant- and scope-bound subscription state.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Display, Formatter};

use layerx_agent_api::identity::{
    ActivityType, AgentDid, Asset, CapabilityId, ExplicitSet, TenantId as ApiTenantId,
};
use layerx_agent_api::read::{AccountRef, ModuleRef};
use layerx_agent_api::subscription::{
    Cursor as ApiCursor, DeliveryTarget, SubscriptionCreate, SubscriptionFilter, SubscriptionId,
    SubscriptionRecord, SubscriptionScope, SubscriptionTarget, TenantObject,
};
use layerx_agent_api::{subscription::CursorAcknowledgement, Sequence};
use layerx_types::result::ResultCode;

use crate::store::{
    ObjectKind, StorageClass, Store as DurableStore, StoreError, TenantId, TenantKey,
};

const RECORD_MAGIC: &[u8; 4] = b"LXSB";

/// Durable subscription position. The value is the next stream cursor after
/// acknowledged delivery, not a locally inferred protocol sequence.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Cursor(pub u64);

/// Durable continuity state that prevents delivery from crossing an
/// unresolved gap or an expired retention window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Continuity {
    Healthy,
    GapBlocked {
        missing_first: u64,
        missing_last: u64,
        backfill_attempted: bool,
        recovered_through: Option<u64>,
    },
    Truncated {
        requested_from: u64,
        oldest_available: u64,
        lost_through: u64,
    },
}

impl From<ApiCursor> for Cursor {
    fn from(value: ApiCursor) -> Self {
        Self(value.0 .0)
    }
}

impl From<Cursor> for ApiCursor {
    fn from(value: Cursor) -> Self {
        Self(Sequence(value.0))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DurableRecord {
    public: SubscriptionRecord,
    delivered_unacknowledged: BTreeSet<Cursor>,
    current_delivery: Cursor,
    continuity: Continuity,
}

/// Durable subscription store failures.
#[derive(Debug)]
pub enum SubscriptionError {
    InvalidScope,
    InvalidFilter,
    Duplicate,
    NotFound,
    Paused,
    CursorOutOfOrder { expected: Cursor, received: Cursor },
    CursorNeverDelivered { cursor: Cursor },
    CursorRegressed { current: Cursor, received: Cursor },
    SequenceExhausted,
    Corrupt,
    Durable(StoreError),
}

impl Display for SubscriptionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidScope => formatter.write_str("subscription scope is invalid"),
            Self::InvalidFilter => formatter.write_str("subscription filter exceeds its scope"),
            Self::Duplicate => formatter.write_str("subscription already exists"),
            Self::NotFound => formatter.write_str("subscription not found"),
            Self::Paused => formatter.write_str("subscription is paused"),
            Self::CursorOutOfOrder { expected, received } => write!(
                formatter,
                "delivery cursor {} arrived out of order; expected {}",
                received.0, expected.0
            ),
            Self::CursorNeverDelivered { cursor } => {
                write!(formatter, "cursor {} was never delivered", cursor.0)
            }
            Self::CursorRegressed { current, received } => write!(
                formatter,
                "cursor {} precedes acknowledged cursor {}",
                received.0, current.0
            ),
            Self::SequenceExhausted => formatter.write_str("subscription cursor is exhausted"),
            Self::Corrupt => formatter.write_str("durable subscription record is corrupt"),
            Self::Durable(error) => Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for SubscriptionError {}

impl From<StoreError> for SubscriptionError {
    fn from(value: StoreError) -> Self {
        Self::Durable(value)
    }
}

/// Tenant-owned durable subscription collection.
#[derive(Debug)]
pub struct Store {
    durable: DurableStore,
    tenant: TenantId,
    records: BTreeMap<String, DurableRecord>,
}

impl Store {
    /// Restores every subscription owned by `tenant` from local durable state.
    pub fn open(durable: DurableStore, tenant: TenantId) -> Result<Self, SubscriptionError> {
        let mut records = BTreeMap::new();
        for object_id in durable.list_object_ids(&tenant, ObjectKind::Subscription) {
            let key = TenantKey::new(tenant.clone(), ObjectKind::Subscription, object_id.clone())?;
            let stored = durable.get(&key).ok_or(SubscriptionError::Corrupt)?;
            if stored.class() != StorageClass::LocalOnly {
                return Err(SubscriptionError::Corrupt);
            }
            let record = decode_record(stored.bytes())?;
            let id = record.public.subscription_id.as_str().to_owned();
            if object_id != id.as_bytes()
                || record.public.scope.tenant.as_str() != tenant.as_str()
                || records.insert(id, record).is_some()
            {
                return Err(SubscriptionError::Corrupt);
            }
        }
        Ok(Self {
            durable,
            tenant,
            records,
        })
    }

    /// Creates one durable subscription after applying scope restrictions to
    /// every filter dimension.
    pub fn create(
        &mut self,
        subscription_id: SubscriptionId,
        request: SubscriptionCreate,
    ) -> Result<SubscriptionRecord, SubscriptionError> {
        self.validate_scope(&request.scope)?;
        validate_filter(&request.scope, &request.filter)?;
        let request = request
            .validate()
            .map_err(|_| SubscriptionError::InvalidFilter)?;
        let id = subscription_id.as_str().to_owned();
        if self.records.contains_key(&id) {
            return Err(SubscriptionError::Duplicate);
        }
        let record = DurableRecord {
            public: SubscriptionRecord {
                subscription_id,
                scope: request.scope,
                filter: request.filter,
                start: request.start,
                last_acknowledged: request.start,
                delivery_target: request.delivery_target,
                paused: false,
            },
            delivered_unacknowledged: BTreeSet::new(),
            current_delivery: Cursor::from(request.start),
            continuity: Continuity::Healthy,
        };
        self.persist(&record)?;
        self.records.insert(id, record.clone());
        Ok(record.public)
    }

    /// Lists only records owned by the exact tenant, agent and capability scope.
    pub fn list(&self, scope: &SubscriptionScope) -> Vec<SubscriptionRecord> {
        if self.validate_scope(scope).is_err() {
            return Vec::new();
        }
        self.records
            .values()
            .filter(|record| record.public.scope == *scope)
            .map(|record| record.public.clone())
            .collect()
    }

    /// Reads one subscription without revealing cross-scope existence.
    pub fn get(
        &self,
        target: &SubscriptionTarget,
    ) -> Result<SubscriptionRecord, SubscriptionError> {
        Ok(self.target(target)?.public.clone())
    }

    /// Records one strictly consecutive delivered cursor durably so a later
    /// acknowledgement cannot claim an unseen position.
    pub fn mark_delivered(
        &mut self,
        target: &SubscriptionTarget,
        cursor: Cursor,
    ) -> Result<(), SubscriptionError> {
        let id = target.subscription_id.as_str().to_owned();
        let record = self.target(target)?;
        if record.public.paused {
            return Err(SubscriptionError::Paused);
        }
        if cursor <= record.current_delivery {
            let expected = Cursor(
                record
                    .current_delivery
                    .0
                    .checked_add(1)
                    .ok_or(SubscriptionError::SequenceExhausted)?,
            );
            return Err(SubscriptionError::CursorOutOfOrder {
                expected,
                received: cursor,
            });
        }
        let mut updated = record.clone();
        updated.current_delivery = cursor;
        if updated.delivered_unacknowledged.insert(cursor) {
            self.persist(&updated)?;
        }
        self.records.insert(id, updated);
        Ok(())
    }

    /// Advances the durable acknowledged cursor only through positions that
    /// were previously delivered in strict order.
    pub fn acknowledge(
        &mut self,
        acknowledgement: &CursorAcknowledgement,
    ) -> Result<SubscriptionRecord, SubscriptionError> {
        let target = SubscriptionTarget {
            scope: acknowledgement.scope.clone(),
            subscription_id: acknowledgement.subscription_id.clone(),
        };
        let id = target.subscription_id.as_str().to_owned();
        let record = self.target(&target)?;
        let current = Cursor::from(record.public.last_acknowledged);
        let received = Cursor::from(acknowledgement.cursor);
        if received < current {
            return Err(SubscriptionError::CursorRegressed { current, received });
        }
        if received == current {
            return Ok(record.public.clone());
        }
        if !record.delivered_unacknowledged.contains(&received) {
            return Err(SubscriptionError::CursorNeverDelivered { cursor: received });
        }
        let mut updated = record.clone();
        updated.public.last_acknowledged = acknowledgement.cursor;
        updated
            .delivered_unacknowledged
            .retain(|delivered| *delivered > received);
        self.persist(&updated)?;
        self.records.insert(id, updated.clone());
        Ok(updated.public)
    }

    /// Pauses delivery durably.
    pub fn pause(
        &mut self,
        target: &SubscriptionTarget,
    ) -> Result<SubscriptionRecord, SubscriptionError> {
        self.set_paused(target, true)
    }

    /// Resumes delivery durably from the last acknowledged cursor.
    pub fn resume(
        &mut self,
        target: &SubscriptionTarget,
    ) -> Result<SubscriptionRecord, SubscriptionError> {
        self.set_paused(target, false)
    }

    /// Removes one exact-scope subscription durably.
    pub fn delete(&mut self, target: &SubscriptionTarget) -> Result<(), SubscriptionError> {
        self.target(target)?;
        let key = subscription_key(self.tenant.clone(), &target.subscription_id)?;
        if !self.durable.remove_local(&key)? {
            return Err(SubscriptionError::NotFound);
        }
        self.records.remove(target.subscription_id.as_str());
        Ok(())
    }

    /// Returns the cursor from which restart delivery must resume.
    pub fn resume_cursor(&self, target: &SubscriptionTarget) -> Result<Cursor, SubscriptionError> {
        Ok(Cursor::from(self.target(target)?.public.last_acknowledged))
    }

    /// Returns the durable continuity state for the exact subscription scope.
    pub fn continuity(&self, target: &SubscriptionTarget) -> Result<Continuity, SubscriptionError> {
        Ok(self.target(target)?.continuity)
    }

    /// Blocks delivery at an explicit missing global-sequence range.
    pub fn block_gap(
        &mut self,
        target: &SubscriptionTarget,
        missing_first: u64,
        missing_last: u64,
    ) -> Result<(), SubscriptionError> {
        let id = target.subscription_id.as_str().to_owned();
        let mut updated = self.target(target)?.clone();
        updated.continuity = Continuity::GapBlocked {
            missing_first,
            missing_last,
            backfill_attempted: false,
            recovered_through: None,
        };
        self.persist(&updated)?;
        self.records.insert(id, updated);
        Ok(())
    }

    /// Records exact backfill progress while leaving the gap blocked.
    pub fn record_backfill(
        &mut self,
        target: &SubscriptionTarget,
        recovered_through: Option<u64>,
    ) -> Result<(), SubscriptionError> {
        let id = target.subscription_id.as_str().to_owned();
        let mut updated = self.target(target)?.clone();
        let Continuity::GapBlocked {
            missing_first,
            missing_last,
            ..
        } = updated.continuity
        else {
            return Err(SubscriptionError::Corrupt);
        };
        updated.continuity = Continuity::GapBlocked {
            missing_first,
            missing_last,
            backfill_attempted: true,
            recovered_through,
        };
        self.persist(&updated)?;
        self.records.insert(id, updated);
        Ok(())
    }

    /// Clears a gap only after complete contiguous backfill was verified.
    pub fn clear_gap(&mut self, target: &SubscriptionTarget) -> Result<(), SubscriptionError> {
        let id = target.subscription_id.as_str().to_owned();
        let mut updated = self.target(target)?.clone();
        if !matches!(updated.continuity, Continuity::GapBlocked { .. }) {
            return Err(SubscriptionError::Corrupt);
        }
        updated.continuity = Continuity::Healthy;
        self.persist(&updated)?;
        self.records.insert(id, updated);
        Ok(())
    }

    /// Marks retention loss durably; truncated state is terminal.
    pub fn mark_truncated(
        &mut self,
        target: &SubscriptionTarget,
        requested_from: u64,
        oldest_available: u64,
        lost_through: u64,
    ) -> Result<(), SubscriptionError> {
        let id = target.subscription_id.as_str().to_owned();
        let mut updated = self.target(target)?.clone();
        updated.continuity = Continuity::Truncated {
            requested_from,
            oldest_available,
            lost_through,
        };
        self.persist(&updated)?;
        self.records.insert(id, updated);
        Ok(())
    }

    /// Consumes the subscription collection and returns the underlying store.
    #[must_use]
    pub fn into_durable(self) -> DurableStore {
        self.durable
    }

    /// Borrows the tenant-scoped durable state for event-history delivery.
    #[must_use]
    pub const fn durable(&self) -> &DurableStore {
        &self.durable
    }

    fn set_paused(
        &mut self,
        target: &SubscriptionTarget,
        paused: bool,
    ) -> Result<SubscriptionRecord, SubscriptionError> {
        let id = target.subscription_id.as_str().to_owned();
        let mut updated = self.target(target)?.clone();
        updated.public.paused = paused;
        self.persist(&updated)?;
        self.records.insert(id, updated.clone());
        Ok(updated.public)
    }

    fn target(&self, target: &SubscriptionTarget) -> Result<&DurableRecord, SubscriptionError> {
        if self.validate_scope(&target.scope).is_err() {
            return Err(SubscriptionError::NotFound);
        }
        self.records
            .get(target.subscription_id.as_str())
            .filter(|record| record.public.scope == target.scope)
            .ok_or(SubscriptionError::NotFound)
    }

    fn validate_scope(&self, scope: &SubscriptionScope) -> Result<(), SubscriptionError> {
        if scope.tenant.as_str() != self.tenant.as_str() {
            Err(SubscriptionError::InvalidScope)
        } else {
            Ok(())
        }
    }

    fn persist(&mut self, record: &DurableRecord) -> Result<(), SubscriptionError> {
        let key = subscription_key(self.tenant.clone(), &record.public.subscription_id)?;
        self.durable.put_local(key, encode_record(record)?)?;
        Ok(())
    }
}

fn validate_filter(
    scope: &SubscriptionScope,
    filter: &SubscriptionFilter,
) -> Result<(), SubscriptionError> {
    let tenant = scope.tenant.as_str();
    if filter
        .agents
        .values()
        .iter()
        .any(|item| item.tenant.as_str() != tenant || item.value != scope.agent)
        || filter
            .accounts
            .values()
            .iter()
            .any(|item| item.tenant.as_str() != tenant)
        || filter
            .modules
            .values()
            .iter()
            .any(|item| item.tenant.as_str() != tenant)
        || filter
            .assets
            .values()
            .iter()
            .any(|item| item.tenant.as_str() != tenant)
        || filter
            .counterparties
            .values()
            .iter()
            .any(|item| item.tenant.as_str() != tenant)
    {
        return Err(SubscriptionError::InvalidFilter);
    }
    Ok(())
}

fn subscription_key(tenant: TenantId, id: &SubscriptionId) -> Result<TenantKey, StoreError> {
    TenantKey::new(
        tenant,
        ObjectKind::Subscription,
        id.as_str().as_bytes().to_vec(),
    )
}

fn encode_record(record: &DurableRecord) -> Result<Vec<u8>, SubscriptionError> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(RECORD_MAGIC);
    push_string(&mut bytes, record.public.subscription_id.as_str())?;
    push_string(&mut bytes, record.public.scope.tenant.as_str())?;
    push_string(&mut bytes, record.public.scope.agent.as_str())?;
    push_string(&mut bytes, record.public.scope.capability.as_str())?;
    push_string(&mut bytes, record.public.delivery_target.as_str())?;
    bytes.extend_from_slice(&record.public.start.0 .0.to_be_bytes());
    bytes.extend_from_slice(&record.public.last_acknowledged.0 .0.to_be_bytes());
    push_len(&mut bytes, record.delivered_unacknowledged.len())?;
    for cursor in &record.delivered_unacknowledged {
        bytes.extend_from_slice(&cursor.0.to_be_bytes());
    }
    bytes.push(u8::from(record.public.paused));
    encode_continuity(&mut bytes, record.continuity);
    encode_filter(&mut bytes, &record.public.filter)?;
    Ok(bytes)
}

fn encode_filter(
    bytes: &mut Vec<u8>,
    filter: &SubscriptionFilter,
) -> Result<(), SubscriptionError> {
    push_len(bytes, filter.agents.values().len())?;
    for item in filter.agents.values() {
        push_string(bytes, item.tenant.as_str())?;
        push_string(bytes, item.value.as_str())?;
    }
    push_len(bytes, filter.accounts.values().len())?;
    for item in filter.accounts.values() {
        push_string(bytes, item.tenant.as_str())?;
        push_string(bytes, item.value.as_str())?;
    }
    push_len(bytes, filter.activity_types.values().len())?;
    for value in filter.activity_types.values() {
        bytes.extend_from_slice(&value.0.to_be_bytes());
    }
    push_len(bytes, filter.modules.values().len())?;
    for item in filter.modules.values() {
        push_string(bytes, item.tenant.as_str())?;
        push_string(bytes, item.value.as_str())?;
    }
    push_len(bytes, filter.assets.values().len())?;
    for item in filter.assets.values() {
        push_string(bytes, item.tenant.as_str())?;
        push_string(bytes, item.value.as_str())?;
    }
    push_len(bytes, filter.counterparties.values().len())?;
    for item in filter.counterparties.values() {
        push_string(bytes, item.tenant.as_str())?;
        push_string(bytes, item.value.as_str())?;
    }
    push_len(bytes, filter.result_classes.values().len())?;
    for value in filter.result_classes.values() {
        bytes.extend_from_slice(&value.raw().to_be_bytes());
    }
    Ok(())
}

fn decode_record(bytes: &[u8]) -> Result<DurableRecord, SubscriptionError> {
    let mut decoder = Decoder::new(bytes);
    if decoder.take(4)? != RECORD_MAGIC {
        return Err(SubscriptionError::Corrupt);
    }
    let subscription_id =
        SubscriptionId::new(decoder.string()?).map_err(|_| SubscriptionError::Corrupt)?;
    let tenant = ApiTenantId::new(decoder.string()?).map_err(|_| SubscriptionError::Corrupt)?;
    let agent = AgentDid::new(decoder.string()?).map_err(|_| SubscriptionError::Corrupt)?;
    let capability =
        CapabilityId::new(decoder.string()?).map_err(|_| SubscriptionError::Corrupt)?;
    let delivery_target =
        DeliveryTarget::new(decoder.string()?).map_err(|_| SubscriptionError::Corrupt)?;
    let start = ApiCursor(Sequence(decoder.u64()?));
    let last_acknowledged = ApiCursor(Sequence(decoder.u64()?));
    let delivered_count = decoder.len()?;
    let mut delivered_unacknowledged = BTreeSet::new();
    for _ in 0..delivered_count {
        if !delivered_unacknowledged.insert(Cursor(decoder.u64()?)) {
            return Err(SubscriptionError::Corrupt);
        }
    }
    let paused = decoder.byte()?;
    if paused > 1 {
        return Err(SubscriptionError::Corrupt);
    }
    let continuity = decode_continuity(&mut decoder)?;
    let scope = SubscriptionScope {
        tenant,
        agent,
        capability,
    };
    let filter = decode_filter(&mut decoder)?;
    if !decoder.is_empty()
        || validate_filter(&scope, &filter).is_err()
        || Cursor::from(last_acknowledged) < Cursor::from(start)
    {
        return Err(SubscriptionError::Corrupt);
    }
    if delivered_unacknowledged
        .iter()
        .any(|delivered| *delivered <= Cursor::from(last_acknowledged))
    {
        return Err(SubscriptionError::Corrupt);
    }
    Ok(DurableRecord {
        public: SubscriptionRecord {
            subscription_id,
            scope,
            filter,
            start,
            last_acknowledged,
            delivery_target,
            paused: paused == 1,
        },
        delivered_unacknowledged,
        current_delivery: Cursor::from(last_acknowledged),
        continuity,
    })
}

fn encode_continuity(bytes: &mut Vec<u8>, continuity: Continuity) {
    match continuity {
        Continuity::Healthy => bytes.push(0),
        Continuity::GapBlocked {
            missing_first,
            missing_last,
            backfill_attempted,
            recovered_through,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&missing_first.to_be_bytes());
            bytes.extend_from_slice(&missing_last.to_be_bytes());
            bytes.push(u8::from(backfill_attempted));
            bytes.push(u8::from(recovered_through.is_some()));
            bytes.extend_from_slice(&recovered_through.unwrap_or(0).to_be_bytes());
        }
        Continuity::Truncated {
            requested_from,
            oldest_available,
            lost_through,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&requested_from.to_be_bytes());
            bytes.extend_from_slice(&oldest_available.to_be_bytes());
            bytes.extend_from_slice(&lost_through.to_be_bytes());
        }
    }
}

fn decode_continuity(decoder: &mut Decoder<'_>) -> Result<Continuity, SubscriptionError> {
    match decoder.byte()? {
        0 => Ok(Continuity::Healthy),
        1 => {
            let missing_first = decoder.u64()?;
            let missing_last = decoder.u64()?;
            let attempted = decoder.byte()?;
            let recovered_present = decoder.byte()?;
            let recovered = decoder.u64()?;
            if missing_last < missing_first || attempted > 1 || recovered_present > 1 {
                return Err(SubscriptionError::Corrupt);
            }
            let recovered_through = (recovered_present == 1).then_some(recovered);
            if recovered_through.is_some_and(|value| value < missing_first || value > missing_last)
                || (attempted == 0 && recovered_through.is_some())
            {
                return Err(SubscriptionError::Corrupt);
            }
            Ok(Continuity::GapBlocked {
                missing_first,
                missing_last,
                backfill_attempted: attempted == 1,
                recovered_through,
            })
        }
        2 => {
            let requested_from = decoder.u64()?;
            let oldest_available = decoder.u64()?;
            let lost_through = decoder.u64()?;
            if oldest_available <= requested_from
                || lost_through.saturating_add(1) != oldest_available
            {
                return Err(SubscriptionError::Corrupt);
            }
            Ok(Continuity::Truncated {
                requested_from,
                oldest_available,
                lost_through,
            })
        }
        _ => Err(SubscriptionError::Corrupt),
    }
}

fn decode_filter(decoder: &mut Decoder<'_>) -> Result<SubscriptionFilter, SubscriptionError> {
    let mut agents = Vec::new();
    for _ in 0..decoder.len()? {
        agents.push(TenantObject {
            tenant: api_tenant(decoder.string()?)?,
            value: AgentDid::new(decoder.string()?).map_err(|_| SubscriptionError::Corrupt)?,
        });
    }
    let mut accounts = Vec::new();
    for _ in 0..decoder.len()? {
        accounts.push(TenantObject {
            tenant: api_tenant(decoder.string()?)?,
            value: AccountRef::new(decoder.string()?).map_err(|_| SubscriptionError::Corrupt)?,
        });
    }
    let mut activity_types = Vec::new();
    for _ in 0..decoder.len()? {
        activity_types.push(ActivityType(decoder.u16()?));
    }
    let mut modules = Vec::new();
    for _ in 0..decoder.len()? {
        modules.push(TenantObject {
            tenant: api_tenant(decoder.string()?)?,
            value: ModuleRef::new(decoder.string()?).map_err(|_| SubscriptionError::Corrupt)?,
        });
    }
    let mut assets = Vec::new();
    for _ in 0..decoder.len()? {
        assets.push(TenantObject {
            tenant: api_tenant(decoder.string()?)?,
            value: Asset::new(decoder.string()?).map_err(|_| SubscriptionError::Corrupt)?,
        });
    }
    let mut counterparties = Vec::new();
    for _ in 0..decoder.len()? {
        counterparties.push(TenantObject {
            tenant: api_tenant(decoder.string()?)?,
            value: layerx_agent_api::identity::Counterparty::new(decoder.string()?)
                .map_err(|_| SubscriptionError::Corrupt)?,
        });
    }
    let mut result_classes = Vec::new();
    for _ in 0..decoder.len()? {
        result_classes.push(ResultCode::from_raw(decoder.i32()?));
    }
    Ok(SubscriptionFilter {
        agents: ExplicitSet::allow(agents),
        accounts: ExplicitSet::allow(accounts),
        activity_types: ExplicitSet::allow(activity_types),
        modules: ExplicitSet::allow(modules),
        assets: ExplicitSet::allow(assets),
        counterparties: ExplicitSet::allow(counterparties),
        result_classes: ExplicitSet::allow(result_classes),
    })
}

fn api_tenant(value: String) -> Result<ApiTenantId, SubscriptionError> {
    ApiTenantId::new(value).map_err(|_| SubscriptionError::Corrupt)
}

fn push_len(bytes: &mut Vec<u8>, value: usize) -> Result<(), SubscriptionError> {
    let value = u32::try_from(value).map_err(|_| SubscriptionError::Corrupt)?;
    bytes.extend_from_slice(&value.to_be_bytes());
    Ok(())
}

fn push_string(bytes: &mut Vec<u8>, value: &str) -> Result<(), SubscriptionError> {
    push_len(bytes, value.len())?;
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Decoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], SubscriptionError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(SubscriptionError::Corrupt)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(SubscriptionError::Corrupt)?;
        self.offset = end;
        Ok(value)
    }

    fn byte(&mut self) -> Result<u8, SubscriptionError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, SubscriptionError> {
        let mut value = [0_u8; 2];
        value.copy_from_slice(self.take(2)?);
        Ok(u16::from_be_bytes(value))
    }

    fn u32(&mut self) -> Result<u32, SubscriptionError> {
        let mut value = [0_u8; 4];
        value.copy_from_slice(self.take(4)?);
        Ok(u32::from_be_bytes(value))
    }

    fn i32(&mut self) -> Result<i32, SubscriptionError> {
        let mut value = [0_u8; 4];
        value.copy_from_slice(self.take(4)?);
        Ok(i32::from_be_bytes(value))
    }

    fn u64(&mut self) -> Result<u64, SubscriptionError> {
        let mut value = [0_u8; 8];
        value.copy_from_slice(self.take(8)?);
        Ok(u64::from_be_bytes(value))
    }

    fn len(&mut self) -> Result<usize, SubscriptionError> {
        usize::try_from(self.u32()?).map_err(|_| SubscriptionError::Corrupt)
    }

    fn string(&mut self) -> Result<String, SubscriptionError> {
        let length = self.len()?;
        let value =
            std::str::from_utf8(self.take(length)?).map_err(|_| SubscriptionError::Corrupt)?;
        Ok(value.to_owned())
    }

    const fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}
