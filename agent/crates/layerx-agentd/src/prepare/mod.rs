//! Construction of complete unsigned activities from verified core state.

use layerx_types::activity::{
    ActivityBuildError, Authority, EnvelopeBuilder, TimestampBound, UnsignedEnvelope,
};
use layerx_types::amount::Amount;
use layerx_types::ids::{Did, IdempotencyKey};
use layerx_types::payload::{ActivityType, ModuleRegistry, Payload, PayloadError};
use layerx_wire::activity::encode_unsigned_envelope;
use layerx_wire::hash::payload_hash_for;
use layerx_wire::sign::preimage_unsigned;
use layerx_wire::WireError;

/// Core-produced inputs required to prepare one activity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorePreparationState {
    pub network_id: u32,
    pub account_sequence: u64,
    pub protocol_timestamp: u64,
    pub observed_head_sequence: u64,
    pub module_registry: ModuleRegistry,
}

/// Boundary that obtains preparation state from core without local guessing.
pub trait CorePreparationBoundary {
    fn preparation_state(&mut self, actor: &Did) -> Result<CorePreparationState, CoreStateError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreStateError {
    Unavailable,
    Unverified,
}

/// Explicit configured defaults and hard preparation bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparationDefaults {
    pub timestamp_span: u64,
    pub fee_limit: Amount,
    pub maximum_payload_bytes: usize,
}

/// Complete caller input. Every omitted bounded field has one explicit default.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareRequest {
    pub actor: Did,
    pub authority: Authority,
    pub activity_type: ActivityType,
    pub expected_account_sequence: Option<u64>,
    pub timestamp_bound: Option<TimestampBound>,
    pub fee_limit: Option<Amount>,
    pub idempotency_key: IdempotencyKey,
    pub payload: Vec<u8>,
    pub declared_payload_limit: usize,
}

/// Exact typed envelope plus the only canonical bytes and signer preimage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Prepared {
    pub envelope: UnsignedEnvelope,
    pub canonical_bytes: Vec<u8>,
    pub signing_preimage: [u8; 32],
    pub observed_head_sequence: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrepareError {
    Core(CoreStateError),
    StaleSequence { expected: u64, core: u64 },
    InvalidDefaults,
    TimestampDoesNotContainCore,
    TimestampBoundWidened,
    PayloadLimitExceeded { actual: usize, maximum: usize },
    Payload(PayloadError),
    Activity(ActivityBuildError),
    Wire(WireError),
}

/// Constructs and canonically encodes an unsigned activity from core state.
pub fn prepare_activity(
    boundary: &mut dyn CorePreparationBoundary,
    defaults: PreparationDefaults,
    request: PrepareRequest,
) -> Result<Prepared, PrepareError> {
    if defaults.timestamp_span == 0 || defaults.maximum_payload_bytes == 0 {
        return Err(PrepareError::InvalidDefaults);
    }
    let state = boundary
        .preparation_state(&request.actor)
        .map_err(PrepareError::Core)?;
    if let Some(expected) = request.expected_account_sequence {
        if expected != state.account_sequence {
            return Err(PrepareError::StaleSequence {
                expected,
                core: state.account_sequence,
            });
        }
    }
    let timestamp_bound = match request.timestamp_bound {
        Some(bound) => {
            if bound.not_before() > state.protocol_timestamp
                || bound.not_after() < state.protocol_timestamp
            {
                return Err(PrepareError::TimestampDoesNotContainCore);
            }
            if bound.not_after().saturating_sub(bound.not_before()) > defaults.timestamp_span {
                return Err(PrepareError::TimestampBoundWidened);
            }
            bound
        }
        None => TimestampBound::new(
            state.protocol_timestamp,
            state
                .protocol_timestamp
                .saturating_add(defaults.timestamp_span),
        )
        .map_err(PrepareError::Activity)?,
    };
    let maximum_payload = defaults
        .maximum_payload_bytes
        .min(request.declared_payload_limit);
    if request.payload.len() > maximum_payload {
        return Err(PrepareError::PayloadLimitExceeded {
            actual: request.payload.len(),
            maximum: maximum_payload,
        });
    }
    let payload = Payload::new(
        &state.module_registry,
        request.activity_type,
        &request.payload,
    )
    .map_err(PrepareError::Payload)?;
    let payload_hash = payload_hash_for(&payload).map_err(PrepareError::Wire)?;
    let mut builder = EnvelopeBuilder::new();
    builder
        .protocol_version(1)
        .and_then(|builder| builder.network_id(state.network_id))
        .and_then(|builder| builder.activity_type(request.activity_type))
        .and_then(|builder| builder.actor_did(request.actor))
        .and_then(|builder| builder.authority(request.authority))
        .and_then(|builder| builder.account_sequence(state.account_sequence))
        .and_then(|builder| builder.timestamp_bound(timestamp_bound))
        .and_then(|builder| builder.idempotency_key(request.idempotency_key))
        .and_then(|builder| builder.fee_limit(request.fee_limit.unwrap_or(defaults.fee_limit)))
        .and_then(|builder| builder.payload_hash(payload_hash))
        .and_then(|builder| builder.payload(payload))
        .map_err(PrepareError::Activity)?;
    let envelope = builder.build().map_err(PrepareError::Activity)?;
    let canonical_bytes = encode_unsigned_envelope(&envelope).map_err(PrepareError::Wire)?;
    let signing_preimage = *preimage_unsigned(&envelope)
        .map_err(PrepareError::Wire)?
        .as_bytes();
    Ok(Prepared {
        envelope,
        canonical_bytes,
        signing_preimage,
        observed_head_sequence: state.observed_head_sequence,
    })
}
