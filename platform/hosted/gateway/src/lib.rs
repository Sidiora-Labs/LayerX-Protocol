//! Hosted gateway policy boundary. The gateway authenticates developer keys,
//! enforces durable quotas and idempotency, and only returns operations backed
//! by verification evidence produced by the protocol service.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use zeroize::Zeroize;

const KEY_PREFIX: &str = "lxp_live_";
const KEY_BYTES: usize = 32;
const HASH_DOMAIN: &[u8] = b"LayerX/gateway/api-key/v1\0";
const REQUEST_DOMAIN: &[u8] = b"LayerX/gateway/request/v1\0";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PrincipalId(String);

impl PrincipalId {
    /// Creates a path-safe principal identifier.
    ///
    /// # Errors
    /// Returns [`GatewayError::InvalidRequest`] for an invalid identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, GatewayError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        {
            return Err(GatewayError::InvalidRequest);
        }
        Ok(Self(value))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Quota {
    pub requests: u64,
    pub window_seconds: u64,
}

impl Quota {
    /// Creates a non-zero fixed-window quota.
    ///
    /// # Errors
    /// Returns [`GatewayError::InvalidRequest`] when either bound is zero.
    pub fn new(requests: u64, window_seconds: u64) -> Result<Self, GatewayError> {
        if requests == 0 || window_seconds == 0 {
            return Err(GatewayError::InvalidRequest);
        }
        Ok(Self {
            requests,
            window_seconds,
        })
    }
}

#[derive(Debug)]
pub struct IssuedKey {
    pub id: String,
    pub secret: String,
}

impl Drop for IssuedKey {
    fn drop(&mut self) {
        self.secret.zeroize();
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VerifiedOperation {
    pub response: Vec<u8>,
    pub receipt: Vec<u8>,
    pub verification_level: String,
}

impl VerifiedOperation {
    fn validate(&self) -> Result<(), GatewayError> {
        if self.receipt.is_empty() || self.verification_level != "receipt-verified" {
            return Err(GatewayError::VerificationRequired);
        }
        Ok(())
    }
}

pub trait ProtocolGateway: Send + Sync {
    /// Executes an already-authenticated request against the protocol service.
    ///
    /// # Errors
    /// Returns a typed gateway error when transport or protocol verification fails.
    fn execute(
        &self,
        principal: &PrincipalId,
        operation: &str,
        signed_request: &[u8],
    ) -> Result<VerifiedOperation, GatewayError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GatewayResponse {
    Completed(VerifiedOperation),
    RateLimited { retry_after_seconds: u64 },
}

#[derive(Serialize, Deserialize)]
struct KeyRecord {
    principal: PrincipalId,
    salt: [u8; 16],
    digest: [u8; 32],
    quota: Quota,
    disabled: bool,
}

#[derive(Serialize, Deserialize)]
struct Usage {
    window_started: u64,
    used: u64,
}

#[derive(Serialize, Deserialize)]
struct IdempotencyRecord {
    request_digest: [u8; 32],
    result: VerifiedOperation,
}

#[derive(Serialize, Deserialize, Default)]
struct State {
    keys: BTreeMap<String, KeyRecord>,
    usage: BTreeMap<String, Usage>,
    idempotency: BTreeMap<String, IdempotencyRecord>,
    audit: Vec<AuditRecord>,
}

#[derive(Serialize, Deserialize)]
struct AuditRecord {
    at: u64,
    principal_digest: [u8; 32],
    operation_digest: [u8; 32],
    outcome: AuditOutcome,
}

#[derive(Serialize, Deserialize)]
enum AuditOutcome {
    Completed,
    RateLimited,
    Refused,
}

pub struct HostedGateway<P> {
    root: PathBuf,
    state: Mutex<State>,
    protocol: P,
}

impl<P: ProtocolGateway> HostedGateway<P> {
    /// Opens or creates the durable gateway state.
    ///
    /// # Errors
    /// Returns an error when the store is missing integrity or cannot be read.
    pub fn open(root: impl AsRef<Path>, protocol: P) -> Result<Self, GatewayError> {
        fs::create_dir_all(root.as_ref())?;
        let path = root.as_ref().join("gateway-state.json");
        let state = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| GatewayError::CorruptStore)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => State::default(),
            Err(error) => return Err(error.into()),
        };
        Ok(Self {
            root: root.as_ref().to_path_buf(),
            state: Mutex::new(state),
            protocol,
        })
    }

    /// Issues a new secret, returning its plaintext exactly once.
    ///
    /// # Errors
    /// Returns an error when entropy or durable storage is unavailable.
    pub fn issue_key(
        &self,
        principal: PrincipalId,
        quota: Quota,
    ) -> Result<IssuedKey, GatewayError> {
        let (issued, record) = generate_key(principal, quota)?;
        let mut state = self.lock()?;
        state.keys.insert(issued.id.clone(), record);
        self.persist(&state)?;
        Ok(issued)
    }

    /// Atomically replaces an authenticated key and disables its predecessor.
    ///
    /// # Errors
    /// Returns an authentication, entropy, or durable-storage error.
    pub fn rotate_key(
        &self,
        old_id: &str,
        secret: &str,
        quota: Quota,
    ) -> Result<IssuedKey, GatewayError> {
        let principal = self.authenticate(old_id, secret)?;
        let (issued, record) = generate_key(principal, quota)?;
        let mut state = self.lock()?;
        let old = state
            .keys
            .get_mut(old_id)
            .ok_or(GatewayError::Unauthenticated)?;
        old.disabled = true;
        state.keys.insert(issued.id.clone(), record);
        self.persist(&state)?;
        Ok(issued)
    }

    /// Authenticates, quota-checks and durably executes a production RPC request.
    ///
    /// # Errors
    /// Returns typed authentication, idempotency, verification and storage failures.
    pub fn execute(
        &self,
        key_id: &str,
        secret: &str,
        idempotency_key: &str,
        operation: &str,
        signed_request: &[u8],
        now: u64,
    ) -> Result<GatewayResponse, GatewayError> {
        if idempotency_key.is_empty()
            || idempotency_key.len() > 128
            || !production_route(operation)
            || signed_request.is_empty()
        {
            return Err(GatewayError::InvalidRequest);
        }
        let principal = self.authenticate(key_id, secret)?;
        let request_digest = request_digest(operation, signed_request);
        let idem_key = scoped_idempotency(&principal, idempotency_key);
        {
            let mut state = self.lock()?;
            if let Some(record) = state.idempotency.get(&idem_key) {
                if record.request_digest != request_digest {
                    return Err(GatewayError::IdempotencyConflict);
                }
                return Ok(GatewayResponse::Completed(record.result.clone()));
            }
            let key = state
                .keys
                .get(key_id)
                .ok_or(GatewayError::Unauthenticated)?;
            let quota = key.quota;
            let usage = state.usage.entry(key_id.to_owned()).or_insert(Usage {
                window_started: now,
                used: 0,
            });
            let elapsed = now.saturating_sub(usage.window_started);
            if elapsed >= quota.window_seconds {
                usage.window_started = now;
                usage.used = 0;
            }
            if usage.used >= quota.requests {
                let retry = quota
                    .window_seconds
                    .saturating_sub(now.saturating_sub(usage.window_started))
                    .max(1);
                append_audit(
                    &mut state,
                    now,
                    &principal,
                    operation,
                    AuditOutcome::RateLimited,
                );
                self.persist(&state)?;
                return Ok(GatewayResponse::RateLimited {
                    retry_after_seconds: retry,
                });
            }
            usage.used = usage.used.saturating_add(1);
            self.persist(&state)?;
        }
        let result = self
            .protocol
            .execute(&principal, operation, signed_request)?;
        result.validate()?;
        let mut state = self.lock()?;
        state.idempotency.insert(
            idem_key,
            IdempotencyRecord {
                request_digest,
                result: result.clone(),
            },
        );
        append_audit(
            &mut state,
            now,
            &principal,
            operation,
            AuditOutcome::Completed,
        );
        self.persist(&state)?;
        Ok(GatewayResponse::Completed(result))
    }

    fn authenticate(&self, key_id: &str, secret: &str) -> Result<PrincipalId, GatewayError> {
        let state = self.lock()?;
        let record = state
            .keys
            .get(key_id)
            .ok_or(GatewayError::Unauthenticated)?;
        let candidate = key_digest(&record.salt, secret.as_bytes());
        if record.disabled || !constant_time_eq(&candidate, &record.digest) {
            return Err(GatewayError::Unauthenticated);
        }
        Ok(record.principal.clone())
    }

    fn lock(&self) -> Result<MutexGuard<'_, State>, GatewayError> {
        self.state.lock().map_err(|_| GatewayError::Unavailable)
    }

    fn persist(&self, state: &State) -> Result<(), GatewayError> {
        let bytes = serde_json::to_vec(state).map_err(|_| GatewayError::CorruptStore)?;
        let temporary = self.root.join("gateway-state.json.tmp");
        let target = self.root.join("gateway-state.json");
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(temporary, target)?;
        Ok(())
    }
}

#[must_use]
pub fn platform_gateway() -> &'static str {
    "authenticated-receipt-backed-hosted-gateway"
}

fn append_audit(
    state: &mut State,
    at: u64,
    principal: &PrincipalId,
    operation: &str,
    outcome: AuditOutcome,
) {
    state.audit.push(AuditRecord {
        at,
        principal_digest: hash(principal.0.as_bytes()),
        operation_digest: hash(operation.as_bytes()),
        outcome,
    });
}
fn scoped_idempotency(principal: &PrincipalId, key: &str) -> String {
    hex(&hash(
        &[principal.0.as_bytes(), b"\0", key.as_bytes()].concat(),
    ))
}
fn request_digest(operation: &str, request: &[u8]) -> [u8; 32] {
    hash(&[REQUEST_DOMAIN, operation.as_bytes(), b"\0", request].concat())
}
fn production_route(operation: &str) -> bool {
    matches!(operation, "POST /v1/activities" | "GET /v1/state")
        || operation
            .strip_prefix("GET /v1/receipts/")
            .is_some_and(|id| {
                !id.is_empty()
                    && id.len() <= 128
                    && id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            })
}
fn key_digest(salt: &[u8; 16], secret: &[u8]) -> [u8; 32] {
    hash(&[HASH_DOMAIN, salt, secret].concat())
}
fn generate_key(
    principal: PrincipalId,
    quota: Quota,
) -> Result<(IssuedKey, KeyRecord), GatewayError> {
    let mut random = [0_u8; KEY_BYTES + 16];
    getrandom::fill(&mut random).map_err(|_| GatewayError::Entropy)?;
    let id = hex(&hash(&random)[..12]);
    let secret = format!("{KEY_PREFIX}{}", hex(&random[..KEY_BYTES]));
    let mut salt = [0_u8; 16];
    salt.copy_from_slice(&random[KEY_BYTES..]);
    let digest = key_digest(&salt, secret.as_bytes());
    random.zeroize();
    let issued = IssuedKey { id, secret };
    let record = KeyRecord {
        principal,
        salt,
        digest,
        quota,
        disabled: false,
    };
    Ok((issued, record))
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
fn constant_time_eq(left: &[u8; 32], right: &[u8; 32]) -> bool {
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(char::from(DIGITS[usize::from(byte >> 4)]));
        value.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    value
}

#[derive(Debug)]
pub enum GatewayError {
    Unauthenticated,
    InvalidRequest,
    IdempotencyConflict,
    VerificationRequired,
    CorruptStore,
    Entropy,
    Unavailable,
    Io(io::Error),
}
impl Display for GatewayError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Unauthenticated => "gateway authentication refused",
            Self::InvalidRequest => "gateway request is invalid",
            Self::IdempotencyConflict => "idempotency key conflicts with an earlier request",
            Self::VerificationRequired => "protocol response lacks verified receipt evidence",
            Self::CorruptStore => "gateway durable store is corrupt",
            Self::Entropy => "secure key generation unavailable",
            Self::Unavailable => "gateway state unavailable",
            Self::Io(_) => "gateway durable store unavailable",
        })
    }
}
impl std::error::Error for GatewayError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Self::Io(error) = self {
            Some(error)
        } else {
            None
        }
    }
}
impl From<io::Error> for GatewayError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
