//! KMS-backed custody for human and managed-agent signing keys.

mod sessions;
mod signer;

pub use sessions::{
    AgentContractError, AgentSessionContract, AgentSessionProvision, AgentSessionSecret,
    ManagedAgentState, PlainTime, ProtocolIdentitySnapshot, ProvisionEvidence, RenewalOutcome,
    RevocationEvidence, RevocationOutcome, RevocationReason, RotationEvidence, RotationJourney,
    RotationJourneyState, RotationObservation, RotationSubject, RotationSubmission,
    SessionEntropySource, SessionKeyEntropy, SessionKeyError, SessionKeyProvisioner, SessionLease,
    SessionLeaseState, SessionPolicy, SessionTarget, SuspensionEvidence,
};

pub use signer::{
    CustodySigner, Operation, SignAuthorization, SignRequest, SignatureGrant, SigningLimits,
    StepUpEvidence,
};

use std::fmt::{Display, Formatter};
use std::fs;
use std::io;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use layerx_crypto::keystore::{Keystore as KeyEnvelope, KeystoreEntropy, KeystoreError};
use layerx_crypto::local::LocalSigner;
use layerx_crypto::redact::Secret;
use layerx_crypto::signer::{SignError, Signer as _};
use zeroize::Zeroizing;

use crate::audit::AuditError;
use crate::store::{PrincipalId, StoreError};

const IDENTITY_DOMAIN: &[u8] = b"layerx-human-custody/v1";
const RECORD_MAGIC: &[u8; 4] = b"LXCK";
const RECORD_VERSION: u8 = 1;
const PRINCIPALS_DIR: &str = "principals";
const KEY_FILE_SUFFIX: &str = ".key";
const TEMP_FILE_SUFFIX: &str = ".key.tmp";
const IDENTIFIER_LIMIT: usize = 128;
const KEY_REFERENCE_LIMIT: usize = 256;
const KMS_SECRET_MINIMUM: usize = 32;
const KMS_SECRET_MAXIMUM: usize = 4096;

pub(crate) fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= IDENTIFIER_LIMIT
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_'
        })
}

/// A validated custody key identifier, the only handle that reaches sealed
/// key records and the sole source of one on-disk file name.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct KeyId(String);

impl KeyId {
    /// Creates a bounded identifier limited to `a-z`, `0-9`, `-` and `_` so no
    /// identifier can traverse or alias another principal's key subtree.
    ///
    /// # Errors
    ///
    /// Rejects empty, oversize and out-of-charset identifiers.
    pub fn new(value: impl Into<String>) -> Result<Self, CustodyError> {
        let value = value.into();
        if valid_identifier(&value) {
            Ok(Self(value))
        } else {
            Err(CustodyError::InvalidKeyId)
        }
    }

    /// Returns the identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The principal class a custody key signs for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyClass {
    HumanPrimary,
    AgentPrimary,
}

impl KeyClass {
    const fn code(self) -> u8 {
        match self {
            Self::HumanPrimary => 1,
            Self::AgentPrimary => 2,
        }
    }

    fn from_code(value: u8) -> Result<Self, CustodyError> {
        match value {
            1 => Ok(Self::HumanPrimary),
            2 => Ok(Self::AgentPrimary),
            _ => Err(CustodyError::CorruptRecord("unknown key class")),
        }
    }

    /// Returns the stable audit label for this class.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::HumanPrimary => "human-primary",
            Self::AgentPrimary => "agent-primary",
        }
    }
}

/// Caller-supplied entropy for one generated key: the private seed and the
/// envelope entropy, zeroized on release and never rendered.
pub struct KeyEntropy {
    seed: Zeroizing<[u8; 32]>,
    envelope: KeystoreEntropy,
}

impl KeyEntropy {
    /// Accepts independently generated seed, salt and nonce bytes.
    ///
    /// # Errors
    ///
    /// Refuses any all-zero value, which is never valid generated entropy.
    pub fn new(seed: [u8; 32], salt: [u8; 16], nonce: [u8; 24]) -> Result<Self, CustodyError> {
        let envelope =
            KeystoreEntropy::new(salt, nonce).map_err(|_| CustodyError::InvalidEntropy)?;
        let seed = Zeroizing::new(seed);
        if layerx_crypto::ct::eq_fixed(&seed, &[0_u8; 32]) {
            return Err(CustodyError::InvalidEntropy);
        }
        Ok(Self { seed, envelope })
    }
}

impl std::fmt::Debug for KeyEntropy {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("KeyEntropy([redacted])")
    }
}

/// Typed refusal taxonomy of the key-management boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KmsError {
    /// The KMS root material cannot be reached.
    Unavailable,
    /// The KMS was reached and refused the operation.
    Refused,
    /// The sealed envelope bytes are malformed.
    InvalidEnvelope,
}

impl Display for KmsError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Unavailable => "key-management service is unavailable",
            Self::Refused => "key-management service refused the operation",
            Self::InvalidEnvelope => "sealed key envelope is malformed",
        })
    }
}

impl std::error::Error for KmsError {}

/// The boundary inside which private key material exists in the clear. Every
/// implementation seals seeds into ciphertext envelopes and unseals them only
/// into zeroizing memory, and none exposes key material on any surface.
trait KmsKeyProvider: std::fmt::Debug + Send + Sync {
    /// Returns the non-secret root key reference this provider seals under.
    fn key_reference(&self) -> &str;

    /// Reports whether the provider can currently reach its root material.
    ///
    /// # Errors
    ///
    /// Returns the typed unavailability or refusal the next operation would hit.
    fn probe(&self) -> Result<(), KmsError>;

    /// Seals one private seed into a ciphertext envelope bound to the exact
    /// identity and network.
    ///
    /// # Errors
    ///
    /// Returns a typed refusal and never a partially sealed envelope.
    fn seal(
        &self,
        identity: &[u8],
        network_id: u32,
        seed: &[u8; 32],
        entropy: KeystoreEntropy,
    ) -> Result<Vec<u8>, KmsError>;

    /// Unseals one envelope into zeroizing memory under the exact identity and
    /// network it was sealed for.
    ///
    /// # Errors
    ///
    /// Refuses foreign identities, foreign networks and tampered envelopes.
    fn unseal(
        &self,
        identity: &[u8],
        network_id: u32,
        envelope: &[u8],
    ) -> Result<Secret<[u8; 32]>, KmsError>;
}

/// Working KMS provider performing authenticated envelope encryption under a
/// root secret the operator mounts at a declared path. The root secret is read
/// per operation into zeroizing memory and never cached, so removing the mount
/// makes the provider honestly unavailable rather than silently degraded.
pub struct EnvelopeKms {
    key_reference: String,
    secret_path: PathBuf,
}

impl EnvelopeKms {
    /// Configures the provider with its root key reference and secret mount.
    ///
    /// # Errors
    ///
    /// Rejects empty, oversize or NUL-bearing key references.
    pub fn new(
        key_reference: impl Into<String>,
        secret_path: impl Into<PathBuf>,
    ) -> Result<Self, CustodyError> {
        let key_reference = key_reference.into();
        if key_reference.is_empty()
            || key_reference.len() > KEY_REFERENCE_LIMIT
            || key_reference.as_bytes().contains(&0)
        {
            return Err(CustodyError::InvalidKeyReference);
        }
        Ok(Self {
            key_reference,
            secret_path: secret_path.into(),
        })
    }

    fn root_secret(&self) -> Result<Zeroizing<Vec<u8>>, KmsError> {
        let file = fs::File::open(&self.secret_path).map_err(|_| KmsError::Unavailable)?;
        let mut secret = Zeroizing::new(Vec::with_capacity(KMS_SECRET_MINIMUM));
        file.take(
            u64::try_from(KMS_SECRET_MAXIMUM.saturating_add(1)).map_err(|_| KmsError::Refused)?,
        )
        .read_to_end(&mut secret)
        .map_err(|_| KmsError::Unavailable)?;
        if !(KMS_SECRET_MINIMUM..=KMS_SECRET_MAXIMUM).contains(&secret.len()) {
            return Err(KmsError::Refused);
        }
        Ok(secret)
    }
}

impl std::fmt::Debug for EnvelopeKms {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EnvelopeKms")
            .field("key_reference", &self.key_reference)
            .finish_non_exhaustive()
    }
}

fn seal_refusal(error: KeystoreError) -> KmsError {
    match error {
        KeystoreError::MalformedStorage => KmsError::InvalidEnvelope,
        KeystoreError::InvalidInput
        | KeystoreError::IdentityMismatch
        | KeystoreError::NetworkMismatch
        | KeystoreError::KeyDerivation
        | KeystoreError::AuthenticationFailed => KmsError::Refused,
    }
}

impl KmsKeyProvider for EnvelopeKms {
    fn key_reference(&self) -> &str {
        &self.key_reference
    }

    fn probe(&self) -> Result<(), KmsError> {
        self.root_secret().map(|_| ())
    }

    fn seal(
        &self,
        identity: &[u8],
        network_id: u32,
        seed: &[u8; 32],
        entropy: KeystoreEntropy,
    ) -> Result<Vec<u8>, KmsError> {
        let root = self.root_secret()?;
        let envelope =
            KeyEnvelope::seal(seed, &root, identity, network_id, entropy).map_err(seal_refusal)?;
        envelope.to_bytes().map_err(seal_refusal)
    }

    fn unseal(
        &self,
        identity: &[u8],
        network_id: u32,
        envelope: &[u8],
    ) -> Result<Secret<[u8; 32]>, KmsError> {
        let root = self.root_secret()?;
        let envelope = KeyEnvelope::from_bytes(envelope).map_err(seal_refusal)?;
        envelope
            .open(&root, identity, network_id)
            .map_err(seal_refusal)
    }
}

/// Errors from the custody keystore and signer.
#[derive(Debug)]
pub enum CustodyError {
    InvalidKeyId,
    InvalidKeyReference,
    InvalidNetwork,
    InvalidEntropy,
    InvalidLimits,
    InvalidEvidence,
    CorruptState(&'static str),
    KeyExists,
    KeyNotFound,
    CorruptRecord(&'static str),
    Io(io::Error),
    Kms(KmsError),
    Sign(SignError),
    StepUpRequired,
    StepUpOperationMismatch,
    StepUpMismatch,
    StepUpNotYetValid,
    StepUpExpired,
    StepUpReplayed,
    NonMonotonicTime,
    ThroughputExceeded { retry_at: u64 },
    CoordinationUnavailable,
    Audit(AuditError),
    Store(StoreError),
}

impl CustodyError {
    /// Returns the stable machine code this refusal is audited under.
    #[must_use]
    pub const fn refusal_code(&self) -> &'static str {
        match self {
            Self::InvalidKeyId => "invalid-key-id",
            Self::InvalidKeyReference => "invalid-key-reference",
            Self::InvalidNetwork => "invalid-network",
            Self::InvalidEntropy => "invalid-entropy",
            Self::InvalidLimits => "invalid-limits",
            Self::InvalidEvidence => "invalid-evidence",
            Self::CorruptState(_) => "corrupt-state",
            Self::KeyExists => "key-exists",
            Self::KeyNotFound => "key-not-found",
            Self::CorruptRecord(_) => "corrupt-record",
            Self::Io(_) => "storage-io",
            Self::Kms(KmsError::Unavailable) => "kms-unavailable",
            Self::Kms(KmsError::Refused) => "kms-refused",
            Self::Kms(KmsError::InvalidEnvelope) => "kms-invalid-envelope",
            Self::Sign(SignError::DisclosureMismatch(_)) => "disclosure-mismatch",
            Self::Sign(SignError::InvalidDisclosure) => "disclosure-invalid",
            Self::Sign(_) => "signer-refused",
            Self::StepUpRequired => "step-up-required",
            Self::StepUpOperationMismatch => "step-up-operation-mismatch",
            Self::StepUpMismatch => "step-up-mismatch",
            Self::StepUpNotYetValid => "step-up-not-yet-valid",
            Self::StepUpExpired => "step-up-expired",
            Self::StepUpReplayed => "step-up-replayed",
            Self::NonMonotonicTime => "non-monotonic-time",
            Self::ThroughputExceeded { .. } => "throughput-exceeded",
            Self::CoordinationUnavailable => "coordination-unavailable",
            Self::Audit(_) => "audit-append-failed",
            Self::Store(_) => "store-refused",
        }
    }
}

impl Display for CustodyError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidKeyId => formatter.write_str("invalid custody key identifier"),
            Self::InvalidKeyReference => formatter.write_str("invalid KMS key reference"),
            Self::InvalidNetwork => formatter.write_str("network zero is never a custody scope"),
            Self::InvalidEntropy => formatter.write_str("key entropy is invalid"),
            Self::InvalidLimits => formatter.write_str("signing limits must be non-zero"),
            Self::InvalidEvidence => formatter.write_str("step-up evidence is invalid"),
            Self::CorruptState(reason) => write!(formatter, "corrupt custody state: {reason}"),
            Self::KeyExists => formatter.write_str("custody key already exists"),
            Self::KeyNotFound => formatter.write_str("custody key does not exist"),
            Self::CorruptRecord(reason) => write!(formatter, "corrupt key record: {reason}"),
            Self::Io(error) => write!(formatter, "custody storage failure: {error}"),
            Self::Kms(error) => write!(formatter, "{error}"),
            Self::Sign(error) => write!(formatter, "{error}"),
            Self::StepUpRequired => {
                formatter.write_str("operation requires step-up evidence bound to the disclosure")
            }
            Self::StepUpOperationMismatch => {
                formatter.write_str("step-up evidence is bound to another operation")
            }
            Self::StepUpMismatch => {
                formatter.write_str("step-up evidence is bound to another disclosure")
            }
            Self::StepUpNotYetValid => formatter.write_str("step-up evidence is not valid yet"),
            Self::StepUpExpired => formatter.write_str("step-up evidence is outside its validity"),
            Self::StepUpReplayed => formatter.write_str("step-up evidence was already consumed"),
            Self::NonMonotonicTime => {
                formatter.write_str("signing time moved behind the active throughput window")
            }
            Self::ThroughputExceeded { retry_at } => write!(
                formatter,
                "per-principal signing throughput exceeded until {retry_at}"
            ),
            Self::CoordinationUnavailable => {
                formatter.write_str("custody signing coordination is unavailable")
            }
            Self::Audit(error) => {
                write!(formatter, "signing decision could not be audited: {error}")
            }
            Self::Store(error) => write!(formatter, "principal store refused: {error}"),
        }
    }
}

impl std::error::Error for CustodyError {}

impl From<io::Error> for CustodyError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<KmsError> for CustodyError {
    fn from(value: KmsError) -> Self {
        Self::Kms(value)
    }
}

impl From<SignError> for CustodyError {
    fn from(value: SignError) -> Self {
        Self::Sign(value)
    }
}

/// Public description of one held key: everything the keystore ever returns.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyDescriptor {
    /// The class the key signs for.
    pub class: KeyClass,
    /// The public verification key.
    pub public_key: [u8; 32],
}

/// Availability of one custody dependency on the status surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Availability {
    Available,
    Unavailable,
}

/// Degradation state surfaced for the custody service.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CustodyStatus {
    /// Whether the KMS boundary can be reached.
    pub kms: Availability,
    /// Whether the sealed-record storage can be reached.
    pub storage: Availability,
}

impl CustodyStatus {
    /// Returns whether any custody dependency is degraded.
    #[must_use]
    pub const fn degraded(self) -> bool {
        !matches!(
            (self.kms, self.storage),
            (Availability::Available, Availability::Available)
        )
    }
}

struct KeyRecord {
    class: KeyClass,
    public_key: [u8; 32],
    envelope: Vec<u8>,
}

/// KMS-backed keystore holding every human and managed-agent private key as a
/// sealed envelope under one principal subtree. Key material at rest is
/// ciphertext only; unsealed seeds live exclusively in zeroizing memory and no
/// method returns private material.
#[derive(Debug)]
pub struct Keystore {
    root: PathBuf,
    network_id: u32,
    provider: EnvelopeKms,
}

impl Keystore {
    /// Opens the keystore at `root` for one network under one KMS provider.
    ///
    /// # Errors
    ///
    /// Refuses network zero, foreign entries in the principals tree, and
    /// storage failures.
    pub fn open(
        root: impl AsRef<Path>,
        network_id: u32,
        provider: EnvelopeKms,
    ) -> Result<Self, CustodyError> {
        if network_id == 0 {
            return Err(CustodyError::InvalidNetwork);
        }
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;
        require_directory(&root, "custody root is not a directory")?;
        verify_custody_tree(&root)?;
        Ok(Self {
            root,
            network_id,
            provider,
        })
    }

    /// Returns the network every held key is bound to.
    #[must_use]
    pub const fn network_id(&self) -> u32 {
        self.network_id
    }

    /// Returns the non-secret KMS root key reference in use.
    #[must_use]
    pub fn key_reference(&self) -> &str {
        self.provider.key_reference()
    }

    /// Generates one key inside the KMS boundary and returns only its public
    /// verification key. The supplied entropy is zeroized on release.
    ///
    /// # Errors
    ///
    /// Refuses to overwrite an existing key and returns typed KMS and storage
    /// refusals with nothing persisted.
    pub fn generate(
        &self,
        principal: &PrincipalId,
        key: &KeyId,
        class: KeyClass,
        entropy: KeyEntropy,
    ) -> Result<[u8; 32], CustodyError> {
        let directory = self.create_principal_directory(principal)?;
        let path = directory.join(format!("{}{KEY_FILE_SUFFIX}", key.as_str()));
        if fs::symlink_metadata(&path).is_ok() {
            return Err(CustodyError::KeyExists);
        }
        let identity = identity_bytes(principal, key, class);
        let KeyEntropy { seed, envelope } = entropy;
        let public_key = LocalSigner::new(*seed).public_key();
        let sealed = self
            .provider
            .seal(&identity, self.network_id, &seed, envelope)?;
        drop(seed);
        let record = encode_record(class, &public_key, &sealed)?;
        write_atomic(
            &directory,
            &format!("{}{KEY_FILE_SUFFIX}", key.as_str()),
            &format!("{}{TEMP_FILE_SUFFIX}", key.as_str()),
            &record,
        )?;
        Ok(public_key)
    }

    /// Describes one held key without touching the KMS boundary.
    ///
    /// # Errors
    ///
    /// Returns a typed refusal for unknown keys and corrupt records.
    pub fn describe(
        &self,
        principal: &PrincipalId,
        key: &KeyId,
    ) -> Result<KeyDescriptor, CustodyError> {
        let record = self.read_record(principal, key)?;
        Ok(KeyDescriptor {
            class: record.class,
            public_key: record.public_key,
        })
    }

    /// Lists one principal's key identifiers in deterministic order.
    ///
    /// # Errors
    ///
    /// Returns storage failures and refuses foreign entries in the subtree.
    pub fn keys(&self, principal: &PrincipalId) -> Result<Vec<KeyId>, CustodyError> {
        let directory = self.principal_directory(principal);
        if !directory.try_exists()? {
            return Ok(Vec::new());
        }
        require_directory(&directory, "principal key path is not a directory")?;
        let mut keys = Vec::new();
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                return Err(CustodyError::CorruptRecord("foreign key entry"));
            }
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                return Err(CustodyError::CorruptRecord("foreign key entry"));
            };
            if name.ends_with(TEMP_FILE_SUFFIX) {
                continue;
            }
            let Some(stem) = name.strip_suffix(KEY_FILE_SUFFIX) else {
                return Err(CustodyError::CorruptRecord("foreign key entry"));
            };
            keys.push(
                KeyId::new(stem).map_err(|_| CustodyError::CorruptRecord("foreign key entry"))?,
            );
        }
        keys.sort();
        Ok(keys)
    }

    /// Reports KMS and storage availability for the status surface.
    #[must_use]
    pub fn status(&self) -> CustodyStatus {
        let kms = match self.provider.probe() {
            Ok(()) => Availability::Available,
            Err(_) => Availability::Unavailable,
        };
        let storage = if self.root.is_dir() {
            Availability::Available
        } else {
            Availability::Unavailable
        };
        CustodyStatus { kms, storage }
    }

    pub(crate) fn unseal_signer(
        &self,
        principal: &PrincipalId,
        key: &KeyId,
    ) -> Result<(LocalSigner, KeyClass), CustodyError> {
        let record = self.read_record(principal, key)?;
        let identity = identity_bytes(principal, key, record.class);
        let seed = self
            .provider
            .unseal(&identity, self.network_id, &record.envelope)?;
        let signer = LocalSigner::from_secret(seed);
        if signer.public_key() != record.public_key {
            return Err(CustodyError::CorruptRecord(
                "sealed key does not match its recorded public key",
            ));
        }
        Ok((signer, record.class))
    }

    fn principal_directory(&self, principal: &PrincipalId) -> PathBuf {
        self.root.join(PRINCIPALS_DIR).join(principal.as_str())
    }

    fn key_path(&self, principal: &PrincipalId, key: &KeyId) -> PathBuf {
        self.principal_directory(principal)
            .join(format!("{}{KEY_FILE_SUFFIX}", key.as_str()))
    }

    fn read_record(&self, principal: &PrincipalId, key: &KeyId) -> Result<KeyRecord, CustodyError> {
        let path = self.key_path(principal, key);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_file() => metadata,
            Ok(_) => return Err(CustodyError::CorruptRecord("foreign key entry")),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(CustodyError::KeyNotFound);
            }
            Err(error) => return Err(CustodyError::Io(error)),
        };
        if metadata.len() == 0 {
            return Err(CustodyError::CorruptRecord("empty key record"));
        }
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(CustodyError::KeyNotFound)
            }
            Err(error) => return Err(CustodyError::Io(error)),
        };
        decode_record(&bytes)
    }

    fn create_principal_directory(&self, principal: &PrincipalId) -> Result<PathBuf, CustodyError> {
        let principals = self.root.join(PRINCIPALS_DIR);
        match fs::create_dir(&principals) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(CustodyError::Io(error)),
        }
        require_directory(&principals, "principals path is not a directory")?;
        let directory = principals.join(principal.as_str());
        match fs::create_dir(&directory) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(CustodyError::Io(error)),
        }
        require_directory(&directory, "principal key path is not a directory")?;
        Ok(directory)
    }
}

fn identity_bytes(principal: &PrincipalId, key: &KeyId, class: KeyClass) -> Vec<u8> {
    let principal = principal.as_str().as_bytes();
    let key = key.as_str().as_bytes();
    let class = class.label().as_bytes();
    let mut identity =
        Vec::with_capacity(IDENTITY_DOMAIN.len() + principal.len() + key.len() + class.len() + 3);
    identity.extend_from_slice(IDENTITY_DOMAIN);
    identity.push(0);
    identity.extend_from_slice(principal);
    identity.push(0);
    identity.extend_from_slice(key);
    identity.push(0);
    identity.extend_from_slice(class);
    identity
}

fn encode_record(
    class: KeyClass,
    public_key: &[u8; 32],
    envelope: &[u8],
) -> Result<Vec<u8>, CustodyError> {
    let envelope_length = u32::try_from(envelope.len())
        .map_err(|_| CustodyError::CorruptRecord("envelope exceeds encoding bounds"))?;
    let mut output = Vec::with_capacity(4 + 1 + 1 + 32 + 4 + envelope.len());
    output.extend_from_slice(RECORD_MAGIC);
    output.push(RECORD_VERSION);
    output.push(class.code());
    output.extend_from_slice(public_key);
    output.extend_from_slice(&envelope_length.to_be_bytes());
    output.extend_from_slice(envelope);
    Ok(output)
}

fn decode_record(bytes: &[u8]) -> Result<KeyRecord, CustodyError> {
    let mut reader = RecordReader::new(bytes);
    if reader.take(4)? != RECORD_MAGIC {
        return Err(CustodyError::CorruptRecord("invalid record header"));
    }
    if reader.byte()? != RECORD_VERSION {
        return Err(CustodyError::CorruptRecord("unknown record version"));
    }
    let class = KeyClass::from_code(reader.byte()?)?;
    let public_key = reader
        .take(32)?
        .try_into()
        .map_err(|_| CustodyError::CorruptRecord("truncated public key"))?;
    let envelope_length = reader.length()?;
    let envelope = reader.take(envelope_length)?.to_vec();
    reader.finish()?;
    Ok(KeyRecord {
        class,
        public_key,
        envelope,
    })
}

struct RecordReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> RecordReader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], CustodyError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(CustodyError::CorruptRecord("record length overflow"))?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(CustodyError::CorruptRecord("truncated record"))?;
        self.offset = end;
        Ok(value)
    }

    fn byte(&mut self) -> Result<u8, CustodyError> {
        Ok(self.take(1)?[0])
    }

    fn length(&mut self) -> Result<usize, CustodyError> {
        let bytes: [u8; 4] = self
            .take(4)?
            .try_into()
            .map_err(|_| CustodyError::CorruptRecord("truncated record"))?;
        usize::try_from(u32::from_be_bytes(bytes))
            .map_err(|_| CustodyError::CorruptRecord("record length overflow"))
    }

    fn finish(self) -> Result<(), CustodyError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(CustodyError::CorruptRecord("trailing record bytes"))
        }
    }
}

fn verify_custody_tree(root: &Path) -> Result<(), CustodyError> {
    let principals = root.join(PRINCIPALS_DIR);
    if !principals.try_exists()? {
        return Ok(());
    }
    require_directory(&principals, "principals path is not a directory")?;
    for entry in fs::read_dir(principals)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            return Err(CustodyError::CorruptRecord("foreign principals entry"));
        };
        if PrincipalId::new(name).is_err() || !entry.file_type()?.is_dir() {
            return Err(CustodyError::CorruptRecord("foreign principals entry"));
        }
        for key_entry in fs::read_dir(entry.path())? {
            let key_entry = key_entry?;
            if !key_entry.file_type()?.is_file() {
                return Err(CustodyError::CorruptRecord("foreign key entry"));
            }
            let key_name = key_entry.file_name();
            let Some(key_name) = key_name.to_str() else {
                return Err(CustodyError::CorruptRecord("foreign key entry"));
            };
            let stem = key_name
                .strip_suffix(TEMP_FILE_SUFFIX)
                .or_else(|| key_name.strip_suffix(KEY_FILE_SUFFIX));
            let valid = stem.is_some_and(valid_identifier);
            if !valid {
                return Err(CustodyError::CorruptRecord("foreign key entry"));
            }
        }
    }
    Ok(())
}

fn require_directory(path: &Path, reason: &'static str) -> Result<(), CustodyError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_dir() {
        Ok(())
    } else {
        Err(CustodyError::CorruptRecord(reason))
    }
}

fn write_atomic(
    directory: &Path,
    final_name: &str,
    temp_name: &str,
    bytes: &[u8],
) -> Result<(), CustodyError> {
    use std::io::Write as _;

    let temp_path = directory.join(temp_name);
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp_path)?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(&temp_path);
        return Err(CustodyError::Io(error));
    }
    drop(file);
    let final_path = directory.join(final_name);
    if let Err(error) = fs::hard_link(&temp_path, &final_path) {
        let _ = fs::remove_file(&temp_path);
        return if error.kind() == io::ErrorKind::AlreadyExists {
            Err(CustodyError::KeyExists)
        } else {
            Err(CustodyError::Io(error))
        };
    }
    fs::remove_file(temp_path)?;
    fs::File::open(directory)?.sync_all()?;
    Ok(())
}
