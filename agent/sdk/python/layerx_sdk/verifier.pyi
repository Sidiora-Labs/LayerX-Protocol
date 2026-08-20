from dataclasses import dataclass
from typing import Literal, Protocol

@dataclass(frozen=True)
class MerkleProof:
    leaf_index: int
    leaf_count: int
    siblings: tuple[bytes, ...]

@dataclass(frozen=True)
class BatchHeader:
    protocol_version: int
    network_id: int
    epoch: int
    batch_number: int
    first_sequence: int
    last_sequence: int
    previous_state_root: bytes
    resulting_state_root: bytes
    activity_merkle_root: bytes
    receipt_merkle_root: bytes
    event_merkle_root: bytes
    data_availability_root: bytes
    oracle_root: bytes
    timestamp_ms: int
    sequencer_id: bytes

class LocalSignatureVerifier(Protocol):
    def verify_ed25519(self, public_key: bytes, signature: bytes, digest: bytes) -> bool: ...
    def verify_secp256k1(self, public_key: bytes, signature: bytes, digest: bytes) -> bool: ...

@dataclass(frozen=True)
class SequencerAuthorization:
    sequencer_id: bytes
    public_key: bytes
    first_batch_number: int
    last_batch_number: int

InclusionKind = Literal["activity", "receipt", "event", "state"]
@dataclass(frozen=True)
class InclusionVerification:
    level: Literal["batch-included", "state-proven"]
    header: BatchHeader
    header_digest: bytes
    root: bytes

@dataclass(frozen=True)
class CheckpointAttestation:
    checkpoint_id: bytes
    checkpoint_hash: bytes
    guarantor_id: bytes
    batch_number: int
    data_availability_root: bytes
    replayed: bool
    data_possessed: bool
    availability_class_mask: int
    attested_at_ms: int
    signature: bytes

@dataclass(frozen=True)
class GuarantorKey:
    guarantor_id: bytes
    public_key: bytes
    bonded: bool

@dataclass(frozen=True)
class CheckpointCertificate:
    canonical_header: bytes
    validity_proof: bytes
    attestations: tuple[CheckpointAttestation, ...]
    threshold: int
    settlement_reference: bytes | None = ...

@dataclass(frozen=True)
class CheckpointVerificationInput:
    certificate: CheckpointCertificate
    bonded_set: tuple[GuarantorKey, ...]
    registered_checkpoint_id: bytes
    registered_settlement_reference: bytes | None
    availability_obtained: bool

@dataclass(frozen=True)
class CheckpointVerification:
    level: Literal["checkpoint-finalised", "settlement-anchored"]
    checkpoint_id: bytes
    achieved: int
    required: int
    header: BatchHeader

def verify_merkle_inclusion(canonical_leaf: bytes, proof: MerkleProof, expected_root: bytes) -> None: ...
def decode_batch_header(canonical_header: bytes) -> BatchHeader: ...
def verify_batch_inclusion(kind: InclusionKind, canonical_leaf: bytes, proof: MerkleProof, canonical_header: bytes, header_signature: bytes, authorization: SequencerAuthorization, signatures: LocalSignatureVerifier) -> InclusionVerification: ...
def verify_checkpoint(verification: CheckpointVerificationInput, signatures: LocalSignatureVerifier) -> CheckpointVerification: ...
