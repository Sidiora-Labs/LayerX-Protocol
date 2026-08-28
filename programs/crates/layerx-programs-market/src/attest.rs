//! Canonical admission of inputs produced outside deterministic execution.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Display, Formatter};

use layerx_programs_runtime::{verify_ed25519, SignatureRefusal};
use sha2::{Digest, Sha256};

const STATEMENT_DOMAIN: &[u8] = b"LXP/off-platform-input/v1\0";
const ATTESTATION_DOMAIN: &[u8] = b"LXP/off-platform-attestation/v1\0";
const ATTESTER_SET_DOMAIN: &[u8] = b"LXP/attester-set/v1\0";
const COMMITMENT_SET_DOMAIN: &[u8] = b"LXP/lease-inputs/v1\0";
const ED25519_KEY_BYTES: usize = 32;
const ED25519_SIGNATURE_BYTES: usize = 64;
const MAX_ATTESTERS: usize = 256;
const MAX_ATTESTER_NAME_BYTES: usize = 64;
const MAX_INPUTS_PER_LEASE: usize = 1_024;

/// Canonical protocol identifier for a compute lease.
pub type LeaseId = [u8; 32];

/// A bounded, printable protocol-state name for an attester.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AttesterName(String);

impl AttesterName {
    /// Constructs a canonical lowercase attester name.
    pub fn new(value: impl Into<String>) -> Result<Self, AttestationRefusal> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_ATTESTER_NAME_BYTES
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'.'))
        {
            return Err(AttestationRefusal::InvalidAttesterName);
        }
        Ok(Self(value))
    }

    /// Returns the canonical name bytes.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A named Ed25519 attester admitted by program protocol state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attester {
    /// Stable protocol name included in every signed statement.
    pub name: AttesterName,
    /// Ed25519 public key used by the Programs signature primitive.
    pub public_key: [u8; ED25519_KEY_BYTES],
}

/// Versioned program policy naming every accepted off-platform attester.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttesterSet {
    revision: u64,
    entries: BTreeMap<AttesterName, [u8; ED25519_KEY_BYTES]>,
}

impl AttesterSet {
    /// Builds canonical protocol state, refusing duplicate names and oversized sets.
    pub fn new(revision: u64, attesters: impl IntoIterator<Item = Attester>) -> Result<Self, AttestationRefusal> {
        let mut entries = BTreeMap::new();
        for attester in attesters {
            if entries.len() == MAX_ATTESTERS {
                return Err(AttestationRefusal::TooManyAttesters);
            }
            if entries.insert(attester.name, attester.public_key).is_some() {
                return Err(AttestationRefusal::DuplicateAttester);
            }
        }
        if entries.is_empty() {
            return Err(AttestationRefusal::EmptyAttesterSet);
        }
        Ok(Self { revision, entries })
    }

    /// Returns the policy revision committed by the program.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Commits the full named policy in deterministic key order.
    #[must_use]
    pub fn state_root(&self) -> [u8; 32] {
        let mut encoded = Vec::new();
        encoded.extend_from_slice(ATTESTER_SET_DOMAIN);
        encoded.extend_from_slice(&self.revision.to_be_bytes());
        encoded.extend_from_slice(&usize_u32(self.entries.len()).to_be_bytes());
        for (name, key) in &self.entries {
            push_bytes(&mut encoded, name.as_str().as_bytes());
            encoded.extend_from_slice(key);
        }
        sha256(&encoded)
    }

    fn key(&self, name: &AttesterName) -> Result<&[u8; ED25519_KEY_BYTES], AttestationRefusal> {
        self.entries
            .get(name)
            .ok_or(AttestationRefusal::UnnamedAttester)
    }
}

/// Real-world class of the source observed by an attester.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ExternalInputSource {
    /// Response from a named HTTPS API whose response body is committed.
    HttpsApi = 1,
    /// Output emitted by a named hardware device or sensor.
    HardwareSensor = 2,
    /// Result from a confidential-compute enclave, distinct from an enclave proof.
    ConfidentialCompute = 3,
    /// Signed decision by an identified human operator.
    HumanOperator = 4,
}

/// Honest evidence label carried with every admitted result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceClass {
    /// A trusted external claim whose signature was verified on-platform.
    Attested,
    /// Execution reproduced by the deterministic Programs runtime.
    VerifiedExecution,
}

/// Exact external input that must be committed before lease execution starts.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct InputCommitment {
    /// Lease-local stable identifier for this input.
    pub input_id: [u8; 32],
    /// Digest of the exact bytes supplied to work and adjudication.
    pub payload_digest: [u8; 32],
    /// Exact byte length, preventing digest-only framing ambiguity.
    pub payload_len: u64,
}

/// Signed claim about one committed external input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attestation {
    /// Lease whose precommitted input is being supplied.
    pub lease_id: LeaseId,
    /// Named signer selected from the program's current attester set.
    pub attester: AttesterName,
    /// Policy revision and root fixed when the input was committed.
    pub attester_set_revision: u64,
    /// Exact root of the accepted attester set.
    pub attester_set_root: [u8; 32],
    /// Exact committed input.
    pub input: InputCommitment,
    /// Kind of real-world source observed.
    pub source: ExternalInputSource,
    /// Digest of a canonical source locator, such as URL, device ID, enclave ID, or operator ID.
    pub source_locator_digest: [u8; 32],
    /// Batch height at which the attester says it made the observation.
    pub observed_at_batch: u64,
    /// Ed25519 signature over [`Self::statement_digest`].
    pub signature: Vec<u8>,
}

impl Attestation {
    /// Returns the deterministic digest signed through the Programs Ed25519 primitive.
    #[must_use]
    pub fn statement_digest(&self) -> [u8; 32] {
        let mut encoded = Vec::new();
        encoded.extend_from_slice(STATEMENT_DOMAIN);
        encoded.extend_from_slice(&self.lease_id);
        push_bytes(&mut encoded, self.attester.as_str().as_bytes());
        encoded.extend_from_slice(&self.attester_set_revision.to_be_bytes());
        encoded.extend_from_slice(&self.attester_set_root);
        encoded.extend_from_slice(&self.input.input_id);
        encoded.extend_from_slice(&self.input.payload_digest);
        encoded.extend_from_slice(&self.input.payload_len.to_be_bytes());
        encoded.push(self.source as u8);
        encoded.extend_from_slice(&self.source_locator_digest);
        encoded.extend_from_slice(&self.observed_at_batch.to_be_bytes());
        sha256(&encoded)
    }

    /// Returns the statement identity consumed exactly once, independent of signature encoding.
    #[must_use]
    pub fn replay_id(&self) -> [u8; 32] {
        let mut encoded = Vec::new();
        encoded.extend_from_slice(ATTESTATION_DOMAIN);
        encoded.extend_from_slice(&self.statement_digest());
        sha256(&encoded)
    }
}

/// Admitted external input, deliberately labeled as trusted rather than proven execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttestedInput {
    /// Lease-local input identity.
    pub input: InputCommitment,
    /// Signature-bound source class.
    pub source: ExternalInputSource,
    /// Signer named by protocol state.
    pub attester: AttesterName,
    /// Evidence label that interfaces must render without promotion.
    pub evidence_class: EvidenceClass,
    /// Deterministic identity consumed by replay protection.
    pub attestation_id: [u8; 32],
}

/// Protocol state that fixes inputs before execution and consumes attestations once.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InputCommitmentBook {
    leases: BTreeMap<LeaseId, LeaseInputs>,
    consumed_attestations: BTreeSet<[u8; 32]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LeaseInputs {
    committed_at_batch: u64,
    attester_set_revision: u64,
    attester_set_root: [u8; 32],
    inputs: BTreeSet<InputCommitment>,
    execution_started_at_batch: Option<u64>,
}

impl InputCommitmentBook {
    /// Commits the complete external-input set while the lease is not executable.
    pub fn commit_lease_inputs(
        &mut self,
        lease_id: LeaseId,
        committed_at_batch: u64,
        attester_set: &AttesterSet,
        inputs: impl IntoIterator<Item = InputCommitment>,
    ) -> Result<[u8; 32], AttestationRefusal> {
        if self.leases.contains_key(&lease_id) {
            return Err(AttestationRefusal::LeaseAlreadyCommitted);
        }
        let mut committed_inputs = BTreeSet::new();
        for input in inputs {
            if !committed_inputs.insert(input) {
                return Err(AttestationRefusal::DuplicateInput);
            }
        }
        let inputs = committed_inputs;
        if inputs.is_empty() {
            return Err(AttestationRefusal::EmptyInputSet);
        }
        if inputs.len() > MAX_INPUTS_PER_LEASE {
            return Err(AttestationRefusal::TooManyInputs);
        }
        let set_root = attester_set.state_root();
        let root = commitment_root(lease_id, committed_at_batch, set_root, &inputs);
        self.leases.insert(
            lease_id,
            LeaseInputs {
                committed_at_batch,
                attester_set_revision: attester_set.revision(),
                attester_set_root: set_root,
                inputs,
                execution_started_at_batch: None,
            },
        );
        Ok(root)
    }

    /// Seals the committed inputs before any off-platform execution may begin.
    pub fn start_execution(&mut self, lease_id: LeaseId, batch: u64) -> Result<(), AttestationRefusal> {
        let lease = self
            .leases
            .get_mut(&lease_id)
            .ok_or(AttestationRefusal::UncommittedLease)?;
        if lease.execution_started_at_batch.is_some() {
            return Err(AttestationRefusal::ExecutionAlreadyStarted);
        }
        if batch < lease.committed_at_batch {
            return Err(AttestationRefusal::InputCommittedAfterExecution);
        }
        lease.execution_started_at_batch = Some(batch);
        Ok(())
    }

    /// Verifies and consumes one signed external input deterministically.
    pub fn admit(
        &mut self,
        attestation: &Attestation,
        accepted_attesters: &AttesterSet,
    ) -> Result<AttestedInput, AttestationRefusal> {
        let lease = self
            .leases
            .get(&attestation.lease_id)
            .ok_or(AttestationRefusal::UncommittedLease)?;
        let started = lease
            .execution_started_at_batch
            .ok_or(AttestationRefusal::ExecutionNotStarted)?;
        if lease.committed_at_batch > started {
            return Err(AttestationRefusal::InputCommittedAfterExecution);
        }
        if attestation.observed_at_batch < started {
            return Err(AttestationRefusal::AttestationPredatesExecution);
        }
        if attestation.attester_set_revision != lease.attester_set_revision
            || attestation.attester_set_revision != accepted_attesters.revision()
            || attestation.attester_set_root != lease.attester_set_root
            || attestation.attester_set_root != accepted_attesters.state_root()
        {
            return Err(AttestationRefusal::AttesterPolicyMismatch);
        }
        if !lease.inputs.contains(&attestation.input) {
            return Err(AttestationRefusal::InputNotCommitted);
        }
        let key = accepted_attesters.key(&attestation.attester)?;
        if attestation.signature.len() != ED25519_SIGNATURE_BYTES {
            return Err(AttestationRefusal::MalformedAttestation);
        }
        let replay_id = attestation.replay_id();
        if self.consumed_attestations.contains(&replay_id) {
            return Err(AttestationRefusal::Replay);
        }
        verify_ed25519(&attestation.statement_digest(), key, &attestation.signature)
            .map_err(AttestationRefusal::Signature)?;
        self.consumed_attestations.insert(replay_id);
        Ok(AttestedInput {
            input: attestation.input.clone(),
            source: attestation.source,
            attester: attestation.attester.clone(),
            evidence_class: EvidenceClass::Attested,
            attestation_id: replay_id,
        })
    }

    /// Commits every lease input, execution seal, and consumed replay identity.
    #[must_use]
    pub fn state_root(&self) -> [u8; 32] {
        let mut encoded = Vec::new();
        encoded.extend_from_slice(b"LXP/attested-input-state/v1\0");
        encoded.extend_from_slice(&usize_u32(self.leases.len()).to_be_bytes());
        for (lease_id, lease) in &self.leases {
            encoded.extend_from_slice(lease_id);
            encoded.extend_from_slice(&lease.committed_at_batch.to_be_bytes());
            encoded.extend_from_slice(&lease.attester_set_revision.to_be_bytes());
            encoded.extend_from_slice(&lease.attester_set_root);
            match lease.execution_started_at_batch {
                Some(batch) => {
                    encoded.push(1);
                    encoded.extend_from_slice(&batch.to_be_bytes());
                }
                None => encoded.push(0),
            }
            encoded.extend_from_slice(&usize_u32(lease.inputs.len()).to_be_bytes());
            for input in &lease.inputs {
                encoded.extend_from_slice(&input.input_id);
                encoded.extend_from_slice(&input.payload_digest);
                encoded.extend_from_slice(&input.payload_len.to_be_bytes());
            }
        }
        encoded.extend_from_slice(&usize_u32(self.consumed_attestations.len()).to_be_bytes());
        for replay_id in &self.consumed_attestations {
            encoded.extend_from_slice(replay_id);
        }
        sha256(&encoded)
    }
}

/// Typed refusal for attester policy, commitment, signature, and replay failures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttestationRefusal {
    InvalidAttesterName,
    EmptyAttesterSet,
    TooManyAttesters,
    DuplicateAttester,
    UnnamedAttester,
    EmptyInputSet,
    TooManyInputs,
    DuplicateInput,
    LeaseAlreadyCommitted,
    UncommittedLease,
    ExecutionNotStarted,
    ExecutionAlreadyStarted,
    InputCommittedAfterExecution,
    AttestationPredatesExecution,
    InputNotCommitted,
    AttesterPolicyMismatch,
    MalformedAttestation,
    Replay,
    Signature(SignatureRefusal),
}

impl Display for AttestationRefusal {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidAttesterName => write!(formatter, "invalid attester name"),
            Self::EmptyAttesterSet => write!(formatter, "attester set is empty"),
            Self::TooManyAttesters => write!(formatter, "attester set exceeds protocol limit"),
            Self::DuplicateAttester => write!(formatter, "attester name is duplicated"),
            Self::UnnamedAttester => write!(formatter, "attester is not accepted by program state"),
            Self::EmptyInputSet => write!(formatter, "lease input commitment is empty"),
            Self::TooManyInputs => write!(formatter, "lease input commitment exceeds protocol limit"),
            Self::DuplicateInput => write!(formatter, "lease input commitment contains a duplicate"),
            Self::LeaseAlreadyCommitted => write!(formatter, "lease inputs are already committed"),
            Self::UncommittedLease => write!(formatter, "lease inputs were not committed"),
            Self::ExecutionNotStarted => write!(formatter, "lease execution was not sealed"),
            Self::ExecutionAlreadyStarted => write!(formatter, "lease execution already started"),
            Self::InputCommittedAfterExecution => write!(formatter, "input was not committed before execution"),
            Self::AttestationPredatesExecution => write!(formatter, "attestation predates lease execution"),
            Self::InputNotCommitted => write!(formatter, "attested input is absent from the lease commitment"),
            Self::AttesterPolicyMismatch => write!(formatter, "attester policy differs from committed protocol state"),
            Self::MalformedAttestation => write!(formatter, "attestation has malformed framing"),
            Self::Replay => write!(formatter, "attestation was already consumed"),
            Self::Signature(error) => write!(formatter, "attestation signature refused: {error}"),
        }
    }
}

impl std::error::Error for AttestationRefusal {}

fn commitment_root(
    lease_id: LeaseId,
    committed_at_batch: u64,
    attester_set_root: [u8; 32],
    inputs: &BTreeSet<InputCommitment>,
) -> [u8; 32] {
    let mut encoded = Vec::new();
    encoded.extend_from_slice(COMMITMENT_SET_DOMAIN);
    encoded.extend_from_slice(&lease_id);
    encoded.extend_from_slice(&committed_at_batch.to_be_bytes());
    encoded.extend_from_slice(&attester_set_root);
    encoded.extend_from_slice(&usize_u32(inputs.len()).to_be_bytes());
    for input in inputs {
        encoded.extend_from_slice(&input.input_id);
        encoded.extend_from_slice(&input.payload_digest);
        encoded.extend_from_slice(&input.payload_len.to_be_bytes());
    }
    sha256(&encoded)
}

fn push_bytes(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&usize_u32(value.len()).to_be_bytes());
    output.extend_from_slice(value);
}

fn usize_u32(value: usize) -> u32 {
    let Ok(value) = u32::try_from(value) else {
        unreachable!("protocol collection limits fit in u32")
    };
    value
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer as _, SigningKey};

    use super::*;

    const LEASE: LeaseId = [7; 32];

    fn input(payload: &[u8]) -> InputCommitment {
        InputCommitment {
            input_id: sha256(b"weather-api:berlin:2026-08-28T12:00Z"),
            payload_digest: sha256(payload),
            payload_len: payload.len() as u64,
        }
    }

    fn policy(signing_key: &SigningKey, name: &str) -> AttesterSet {
        AttesterSet::new(
            4,
            [Attester {
                name: AttesterName::new(name).expect("canonical fixture name"),
                public_key: signing_key.verifying_key().to_bytes(),
            }],
        )
        .expect("non-empty fixture policy")
    }

    fn signed_attestation(signing_key: &SigningKey, policy: &AttesterSet, name: &str) -> Attestation {
        let mut attestation = Attestation {
            lease_id: LEASE,
            attester: AttesterName::new(name).expect("canonical fixture name"),
            attester_set_revision: policy.revision(),
            attester_set_root: policy.state_root(),
            input: input(br#"{\"temperature_celsius\":21}"#),
            source: ExternalInputSource::HttpsApi,
            source_locator_digest: sha256(b"https://weather.example/v1/berlin"),
            observed_at_batch: 12_004,
            signature: Vec::new(),
        };
        attestation.signature = signing_key.sign(&attestation.statement_digest()).to_bytes().to_vec();
        attestation
    }

    fn committed_book(policy: &AttesterSet, committed: InputCommitment) -> InputCommitmentBook {
        let mut book = InputCommitmentBook::default();
        book.commit_lease_inputs(LEASE, 12_000, policy, [committed])
            .expect("fixture input commitment");
        book.start_execution(LEASE, 12_001).expect("fixture execution seal");
        book
    }

    #[test]
    fn accepts_real_https_source_as_attested_not_verified_execution() {
        let signing_key = SigningKey::from_bytes(&[11; 32]);
        let policy = policy(&signing_key, "weather-oracle.eu");
        let attestation = signed_attestation(&signing_key, &policy, "weather-oracle.eu");
        let mut book = committed_book(&policy, attestation.input.clone());

        let admitted = book.admit(&attestation, &policy).expect("authentic committed input");

        assert_eq!(admitted.evidence_class, EvidenceClass::Attested);
        assert_ne!(admitted.evidence_class, EvidenceClass::VerifiedExecution);
        assert_eq!(admitted.source, ExternalInputSource::HttpsApi);
    }

    #[test]
    fn refuses_a_valid_signature_from_an_unnamed_attester() {
        let accepted_key = SigningKey::from_bytes(&[12; 32]);
        let foreign_key = SigningKey::from_bytes(&[13; 32]);
        let policy = policy(&accepted_key, "accepted-sensor");
        let foreign_policy = policy(&foreign_key, "foreign-sensor");
        let mut attestation = signed_attestation(&foreign_key, &foreign_policy, "foreign-sensor");
        attestation.attester_set_revision = policy.revision();
        attestation.attester_set_root = policy.state_root();
        attestation.signature = foreign_key.sign(&attestation.statement_digest()).to_bytes().to_vec();
        let mut book = committed_book(&policy, attestation.input.clone());

        assert_eq!(book.admit(&attestation, &policy), Err(AttestationRefusal::UnnamedAttester));
    }

    #[test]
    fn refuses_a_malformed_hardware_sensor_attestation() {
        let signing_key = SigningKey::from_bytes(&[14; 32]);
        let policy = policy(&signing_key, "factory-meter-17");
        let mut attestation = signed_attestation(&signing_key, &policy, "factory-meter-17");
        attestation.source = ExternalInputSource::HardwareSensor;
        attestation.signature.truncate(63);
        let mut book = committed_book(&policy, attestation.input.clone());

        assert_eq!(book.admit(&attestation, &policy), Err(AttestationRefusal::MalformedAttestation));
    }

    #[test]
    fn refuses_replay_after_first_protocol_state_consumption() {
        let signing_key = SigningKey::from_bytes(&[15; 32]);
        let policy = policy(&signing_key, "enclave-cluster-3");
        let mut attestation = signed_attestation(&signing_key, &policy, "enclave-cluster-3");
        attestation.source = ExternalInputSource::ConfidentialCompute;
        attestation.signature = signing_key.sign(&attestation.statement_digest()).to_bytes().to_vec();
        let mut book = committed_book(&policy, attestation.input.clone());

        assert!(book.admit(&attestation, &policy).is_ok());
        assert_eq!(book.admit(&attestation, &policy), Err(AttestationRefusal::Replay));
    }

    #[test]
    fn refuses_input_that_was_not_committed_before_execution() {
        let signing_key = SigningKey::from_bytes(&[16; 32]);
        let policy = policy(&signing_key, "operator-review-board");
        let attestation = signed_attestation(&signing_key, &policy, "operator-review-board");
        let mut different = attestation.input.clone();
        different.payload_digest = sha256(b"different committed payload");
        let mut book = committed_book(&policy, different);

        assert_eq!(book.admit(&attestation, &policy), Err(AttestationRefusal::InputNotCommitted));
    }
}
