//! Exact protocol session-key grant issuance.

use std::collections::BTreeSet;
use std::fmt;

use layerx_types::activity::Authority;
use layerx_types::payload::ActivityType;
use layerx_wire::encode::Encoder;
use layerx_wire::hash::Domain;
use sha2::{Digest as _, Sha256};

use crate::ct;

const GRANT_WIRE_TAG: u16 = 0x2001;
const GRANT_VERSION: u8 = 1;
const SESSION_KEY_AUTHORITY: u8 = 2;
const MAX_GRANT_BYTES: usize = 1024;

/// Explicit operator request for one protocol-enforced session authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionKeyRequest {
    /// Protocol identity delegating authority.
    pub grantor: [u8; 32],
    /// Public key that will exercise the delegated authority.
    pub session_public_key: [u8; 32],
    /// Inclusive lower validity bound.
    pub not_before: u64,
    /// Required inclusive upper validity bound.
    pub expires_at: Option<u64>,
    /// Exact activity types the session may submit.
    pub permitted_activity_types: Vec<ActivityType>,
    /// Required identity revocation sequence captured in protocol state.
    pub revocation_sequence: Option<u64>,
}

/// Protocol bytes and authority representation produced by issuance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssuedSessionKey {
    /// Canonical `lxp_authority_grant` bytes for an ordinary registration activity.
    pub registration_payload: Vec<u8>,
    /// Exact protocol session-key authority representation, not a local record.
    pub authority: Authority,
    /// Core-compatible authority-hash identifier of the grant payload.
    pub grant_id: [u8; 32],
    /// Exact activity set represented by the protocol scope.
    pub permitted_activity_types: Vec<ActivityType>,
    /// Required protocol expiry.
    pub expires_at: u64,
    /// Required protocol revocation sequence.
    pub revocation_sequence: u64,
}

/// Typed refusal for an unsafe or unrepresentable session-key request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionIssueError {
    /// No expiry was supplied.
    MissingExpiry,
    /// No permitted activity type was supplied.
    EmptyActivitySet,
    /// No positive revocation sequence was supplied.
    MissingRevocationSequence,
    /// Expiry does not follow the lower validity bound.
    InvalidExpiry,
    /// Grantor or session public key is the all-zero invalid value.
    InvalidIdentityOrKey,
    /// The exact set would be widened by core's module/range representation.
    NonRepresentableActivitySet,
    /// Canonical protocol encoding failed.
    Encoding,
}

impl fmt::Display for SessionIssueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MissingExpiry => "session key expiry is required",
            Self::EmptyActivitySet => "session key permitted activity set is required",
            Self::MissingRevocationSequence => {
                "session key revocation sequence is required and must be positive"
            }
            Self::InvalidExpiry => "session key expiry must follow not_before",
            Self::InvalidIdentityOrKey => "session key grantor and public key must be non-zero",
            Self::NonRepresentableActivitySet => {
                "session activity set cannot be represented by protocol scope without widening"
            }
            Self::Encoding => "session key protocol grant could not be encoded",
        })
    }
}

impl std::error::Error for SessionIssueError {}

fn exact_scope(
    activity_types: &[ActivityType],
) -> Result<(u64, u16, u16, Vec<ActivityType>), SessionIssueError> {
    if activity_types.is_empty() {
        return Err(SessionIssueError::EmptyActivitySet);
    }
    let unique: BTreeSet<_> = activity_types.iter().copied().collect();
    if unique.len() != activity_types.len() {
        return Err(SessionIssueError::NonRepresentableActivitySet);
    }
    let modules: BTreeSet<_> = unique.iter().map(|kind| kind.module() as u16).collect();
    let minimum = unique
        .iter()
        .map(|kind| kind.ordinal())
        .min()
        .ok_or(SessionIssueError::EmptyActivitySet)?;
    let maximum = unique
        .iter()
        .map(|kind| kind.ordinal())
        .max()
        .ok_or(SessionIssueError::EmptyActivitySet)?;
    let range_length = usize::from(maximum - minimum) + 1;
    let expected_length = modules
        .len()
        .checked_mul(range_length)
        .ok_or(SessionIssueError::NonRepresentableActivitySet)?;
    if unique.len() != expected_length
        || modules.iter().any(|module| {
            (minimum..=maximum).any(|ordinal| {
                !unique
                    .iter()
                    .any(|kind| kind.module() as u16 == *module && kind.ordinal() == ordinal)
            })
        })
    {
        return Err(SessionIssueError::NonRepresentableActivitySet);
    }
    let mut module_mask = 0_u64;
    for module in modules {
        module_mask |= 1_u64 << module;
    }
    Ok((module_mask, minimum, maximum, unique.into_iter().collect()))
}

fn encode_grant(
    request: &SessionKeyRequest,
    module_mask: u64,
    ordinal_min: u16,
    ordinal_max: u16,
    expires_at: u64,
    revocation_sequence: u64,
) -> Result<Vec<u8>, SessionIssueError> {
    let mut encoder = Encoder::new(MAX_GRANT_BYTES);
    macro_rules! write {
        ($expression:expr) => {
            $expression.map_err(|_| SessionIssueError::Encoding)?
        };
    }
    write!(encoder.structure_header(GRANT_WIRE_TAG));
    write!(encoder.u8(GRANT_VERSION));
    write!(encoder.bytes(&request.grantor, 32));
    write!(encoder.bytes(&request.grantor, 32));
    write!(encoder.u8(SESSION_KEY_AUTHORITY));
    write!(encoder.bytes(&request.session_public_key, 32));
    write!(encoder.u64(module_mask));
    write!(encoder.u16(ordinal_min));
    write!(encoder.u16(ordinal_max));
    write!(encoder.bytes(&[0_u8; 32], 32));
    write!(encoder.u128(0));
    write!(encoder.u128(0));
    write!(encoder.u128(0));
    write!(encoder.u64(0));
    write!(encoder.u128(0));
    write!(encoder.u128(0));
    write!(encoder.u64(0));
    write!(encoder.bytes(&[0_u8; 32], 32));
    write!(encoder.u64(request.not_before));
    write!(encoder.u64(expires_at));
    write!(encoder.u64(revocation_sequence));
    write!(encoder.u8(0));
    write!(encoder.u64(0));
    write!(encoder.bytes(&[0_u8; 64], 64));
    Ok(encoder.finish())
}

/// Issues a bounded authority as exact protocol grant bytes.
///
/// # Errors
///
/// Refuses every missing bound, zero identity/key, duplicate or unrepresentable
/// scope, and any canonical encoding failure.
pub fn issue_session_key(
    request: &SessionKeyRequest,
) -> Result<IssuedSessionKey, SessionIssueError> {
    let expires_at = request.expires_at.ok_or(SessionIssueError::MissingExpiry)?;
    if expires_at == 0 || expires_at <= request.not_before {
        return Err(SessionIssueError::InvalidExpiry);
    }
    let revocation_sequence = request
        .revocation_sequence
        .filter(|value| *value > 0)
        .ok_or(SessionIssueError::MissingRevocationSequence)?;
    if ct::eq_fixed(&request.grantor, &[0_u8; 32])
        || ct::eq_fixed(&request.session_public_key, &[0_u8; 32])
    {
        return Err(SessionIssueError::InvalidIdentityOrKey);
    }
    let (module_mask, ordinal_min, ordinal_max, permitted_activity_types) =
        exact_scope(&request.permitted_activity_types)?;
    let registration_payload = encode_grant(
        request,
        module_mask,
        ordinal_min,
        ordinal_max,
        expires_at,
        revocation_sequence,
    )?;
    let authority =
        Authority::session_key(&registration_payload).map_err(|_| SessionIssueError::Encoding)?;
    let mut hasher = Sha256::new();
    hasher.update(Domain::AuthorityHash.tag());
    hasher.update(&registration_payload);
    let grant_id = hasher.finalize().into();
    Ok(IssuedSessionKey {
        registration_payload,
        authority,
        grant_id,
        permitted_activity_types,
        expires_at,
        revocation_sequence,
    })
}
