//! Immediate session invalidation from core-produced authority events.

use layerx_types::ids::Did;

use crate::identity::ProtocolAuthority;
use crate::store::Store;

use super::{persist_record, SessionError, SessionId, SessionRegistry};

/// Core event that invalidates one authority or every authority under a DID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevocationEvent {
    pub did: Did,
    pub authority: Option<ProtocolAuthority>,
    pub reason: InvalidationReason,
    pub observed_sequence: u64,
}

/// Exact reason reported to affected clients.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidationReason {
    IdentityFrozen,
    PrimaryKeyRotated,
    SessionKeyRevoked,
    CapabilityGrantRevoked,
    AccountRecovered,
}

/// Local lifecycle state relevant to revocation handling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationState {
    Prepared,
    Signed,
    Queued,
    Unknown,
    Executed,
    Failed,
}

/// One activity whose local handling may be changed by invalidation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingActivity {
    pub session_id: SessionId,
    pub state: PreparationState,
    pub cancelled: bool,
    pub resolution_continues: bool,
}

/// Observable invalidation result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvalidationReport {
    pub reason: InvalidationReason,
    pub observed_sequence: u64,
    pub invalidated_sessions: Vec<SessionId>,
    pub cancelled_preparations: usize,
    pub unresolved_left_for_resolution: usize,
    pub executed_untouched: usize,
}

pub(crate) fn apply_revocation(
    store: &mut Store,
    registry: &mut SessionRegistry,
    activities: &mut [PendingActivity],
    event: &RevocationEvent,
) -> Result<InvalidationReport, SessionError> {
    let mut invalidated_sessions = Vec::new();
    for (session_id, record) in registry.records_mut() {
        if !record.open || record.request.agent != event.did {
            continue;
        }
        if let Some(authority) = &event.authority {
            if &record.request.authority != authority {
                continue;
            }
        }
        let mut invalidated = record.clone();
        invalidated.open = false;
        persist_record(store, &invalidated)?;
        *record = invalidated;
        invalidated_sessions.push(*session_id);
    }

    let mut cancelled_preparations = 0_usize;
    let mut unresolved_left_for_resolution = 0_usize;
    let mut executed_untouched = 0_usize;
    for activity in activities {
        if !invalidated_sessions.contains(&activity.session_id) {
            continue;
        }
        match activity.state {
            PreparationState::Prepared | PreparationState::Signed => {
                activity.cancelled = true;
                cancelled_preparations += 1;
            }
            PreparationState::Queued | PreparationState::Unknown => {
                activity.resolution_continues = true;
                unresolved_left_for_resolution += 1;
            }
            PreparationState::Executed | PreparationState::Failed => {
                executed_untouched += 1;
            }
        }
    }
    Ok(InvalidationReport {
        reason: event.reason,
        observed_sequence: event.observed_sequence,
        invalidated_sessions,
        cancelled_preparations,
        unresolved_left_for_resolution,
        executed_untouched,
    })
}
