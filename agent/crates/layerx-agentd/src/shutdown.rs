//! Graceful daemon shutdown with durable in-flight and audit draining.

use std::collections::BTreeMap;

use crate::outbox::Outbox;
use crate::store::{ObjectKind, Store, StoreError, TenantId, TenantKey};

#[derive(Default)]
pub struct DaemonLifecycle {
    accepting_work: bool,
    in_flight: BTreeMap<[u8; 32], Vec<u8>>,
    pending_audit: Vec<Vec<u8>>,
}

impl DaemonLifecycle {
    #[must_use]
    pub fn running() -> Self {
        Self {
            accepting_work: true,
            in_flight: BTreeMap::new(),
            pending_audit: Vec::new(),
        }
    }

    pub fn begin_work(
        &mut self,
        submission_id: [u8; 32],
        durable_state: Vec<u8>,
    ) -> Result<(), ShutdownError> {
        if !self.accepting_work {
            return Err(ShutdownError::NotAccepting);
        }
        self.in_flight.insert(submission_id, durable_state);
        Ok(())
    }

    pub fn append_audit(&mut self, record: Vec<u8>) -> Result<(), ShutdownError> {
        if record.is_empty() {
            return Err(ShutdownError::EmptyAudit);
        }
        self.pending_audit.push(record);
        Ok(())
    }

    #[must_use]
    pub const fn accepting_work(&self) -> bool {
        self.accepting_work
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShutdownReport {
    pub in_flight_recorded: usize,
    pub audit_entries_flushed: usize,
    pub accepting_work: bool,
}

#[derive(Debug)]
pub enum ShutdownError {
    NotAccepting,
    EmptyAudit,
    MissingOutboxRecord([u8; 32]),
    Store(StoreError),
    Arithmetic,
}

impl From<StoreError> for ShutdownError {
    fn from(value: StoreError) -> Self {
        Self::Store(value)
    }
}

/// Stops admission, durably records all in-flight work, and flushes audit entries.
pub fn graceful(
    store: &mut Store,
    tenant: TenantId,
    outbox: &Outbox,
    lifecycle: &mut DaemonLifecycle,
) -> Result<ShutdownReport, ShutdownError> {
    lifecycle.accepting_work = false;
    for (submission_id, durable_state) in &lifecycle.in_flight {
        if outbox.status(*submission_id).is_none() {
            return Err(ShutdownError::MissingOutboxRecord(*submission_id));
        }
        store.put_local(
            prefixed_key(
                tenant.clone(),
                ObjectKind::Configuration,
                b"shutdown-inflight:",
                submission_id,
            )?,
            durable_state.clone(),
        )?;
    }
    for (index, audit) in lifecycle.pending_audit.iter().enumerate() {
        let index = u64::try_from(index).map_err(|_| ShutdownError::Arithmetic)?;
        store.put_local(
            prefixed_key(
                tenant.clone(),
                ObjectKind::Audit,
                b"shutdown-audit:",
                &index.to_be_bytes(),
            )?,
            audit.clone(),
        )?;
    }
    store.put_local(
        TenantKey::new(
            tenant,
            ObjectKind::Configuration,
            b"shutdown-complete".to_vec(),
        )?,
        b"durable".to_vec(),
    )?;
    Ok(ShutdownReport {
        in_flight_recorded: lifecycle.in_flight.len(),
        audit_entries_flushed: lifecycle.pending_audit.len(),
        accepting_work: false,
    })
}

fn prefixed_key(
    tenant: TenantId,
    kind: ObjectKind,
    prefix: &[u8],
    suffix: &[u8],
) -> Result<TenantKey, StoreError> {
    let mut object_id = prefix.to_vec();
    object_id.extend_from_slice(suffix);
    TenantKey::new(tenant, kind, object_id)
}
