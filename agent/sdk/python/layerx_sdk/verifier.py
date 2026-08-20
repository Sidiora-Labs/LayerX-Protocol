from __future__ import annotations

from dataclasses import dataclass
from hashlib import sha256
from typing import Literal, Protocol

from .production import PlatformSdkError, SdkErrorCode

_MERKLE_LEAF_DOMAIN = b"LXP/v1/merkle-leaf\0"
_MERKLE_INTERNAL_DOMAIN = b"LXP/v1/merkle-internal\0"
_BATCH_HEADER_DOMAIN = b"LXP/v1/batch-header\0"
_CHECKPOINT_DOMAIN = b"LXP/v1/checkpoint-certificate\0"
_BATCH_HEADER_BYTES = 354
_ALL_AVAILABILITY_CLASSES = 0x1F


def _failure() -> None:
    raise PlatformSdkError(SdkErrorCode.VERIFICATION_FAILURE, "never")


def _exact(value: bytes, length: int) -> bytes:
    if len(value) != length:
        _failure()
    return value


def _equal(left: bytes, right: bytes) -> bool:
    if len(left) != len(right):
        return False
    difference = 0
    for left_byte, right_byte in zip(left, right, strict=True):
        difference |= left_byte ^ right_byte
    return difference == 0


def _digest(domain: bytes, *values: bytes) -> bytes:
    digest = sha256()
    digest.update(domain)
    for value in values:
        digest.update(value)
    return digest.digest()


@dataclass(frozen=True)
class MerkleProof:
    leaf_index: int
    leaf_count: int
    siblings: tuple[bytes, ...]


def _proof_depth(leaf_count: int) -> int:
    count = leaf_count
    depth = 0
    while count > 1:
        count = (count + 1) // 2
        depth += 1
    return depth


def verify_merkle_inclusion(canonical_leaf: bytes, proof: MerkleProof, expected_root: bytes) -> None:
    if (
        proof.leaf_count <= 0
        or proof.leaf_count > 0xFFFF_FFFF
        or proof.leaf_index < 0
        or proof.leaf_index >= proof.leaf_count
        or len(proof.siblings) > 32
        or len(proof.siblings) != _proof_depth(proof.leaf_count)
    ):
        _failure()
    current = _digest(_MERKLE_LEAF_DOMAIN, canonical_leaf)
    index = proof.leaf_index
    level_count = proof.leaf_count
    for sibling_value in proof.siblings:
        sibling = _exact(sibling_value, 32)
        if (index ^ 1) >= level_count and not _equal(sibling, current):
            _failure()
        current = (
            _digest(_MERKLE_INTERNAL_DOMAIN, current, sibling)
            if index % 2 == 0
            else _digest(_MERKLE_INTERNAL_DOMAIN, sibling, current)
        )
        index //= 2
        level_count = (level_count + 1) // 2
    if not _equal(current, _exact(expected_root, 32)):
        _failure()


class _Decoder:
    def __init__(self, value: bytes) -> None:
        self._value = value
        self._offset = 0

    def fixed(self, length: int) -> bytes:
        end = self._offset + length
        if length < 0 or end > len(self._value):
            _failure()
        result = self._value[self._offset:end]
        self._offset = end
        return result

    def integer(self, length: int) -> int:
        return int.from_bytes(self.fixed(length), "big", signed=False)

    def u8(self) -> int:
        return self.integer(1)

    def u16(self) -> int:
        return self.integer(2)

    def u32(self) -> int:
        return self.integer(4)

    def u64(self) -> int:
        return self.integer(8)

    def bounded(self, length: int) -> bytes:
        if self.u32() != length:
            _failure()
        return self.fixed(length)

    def finish(self) -> None:
        if self._offset != len(self._value):
            _failure()


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


def _field(decoder: _Decoder, expected: int) -> None:
    if decoder.u8() != expected:
        _failure()


def decode_batch_header(canonical_header: bytes) -> BatchHeader:
    if len(canonical_header) != _BATCH_HEADER_BYTES:
        _failure()
    decoder = _Decoder(canonical_header)
    if decoder.u16() != 1 or decoder.u16() != 0x1701 or decoder.u8() != 15:
        _failure()
    _field(decoder, 1)
    protocol_version = decoder.u16()
    _field(decoder, 2)
    network_id = decoder.u32()
    _field(decoder, 3)
    epoch = decoder.u64()
    _field(decoder, 4)
    batch_number = decoder.u64()
    _field(decoder, 5)
    first_sequence = decoder.u64()
    _field(decoder, 6)
    last_sequence = decoder.u64()
    _field(decoder, 7)
    previous_state_root = decoder.bounded(32)
    _field(decoder, 8)
    resulting_state_root = decoder.bounded(32)
    _field(decoder, 9)
    activity_merkle_root = decoder.bounded(32)
    _field(decoder, 10)
    receipt_merkle_root = decoder.bounded(32)
    _field(decoder, 11)
    event_merkle_root = decoder.bounded(32)
    _field(decoder, 12)
    data_availability_root = decoder.bounded(32)
    _field(decoder, 13)
    oracle_root = decoder.bounded(32)
    _field(decoder, 14)
    timestamp_ms = decoder.u64()
    _field(decoder, 15)
    sequencer_id = decoder.bounded(32)
    decoder.finish()
    return BatchHeader(
        protocol_version=protocol_version,
        network_id=network_id,
        epoch=epoch,
        batch_number=batch_number,
        first_sequence=first_sequence,
        last_sequence=last_sequence,
        previous_state_root=previous_state_root,
        resulting_state_root=resulting_state_root,
        activity_merkle_root=activity_merkle_root,
        receipt_merkle_root=receipt_merkle_root,
        event_merkle_root=event_merkle_root,
        data_availability_root=data_availability_root,
        oracle_root=oracle_root,
        timestamp_ms=timestamp_ms,
        sequencer_id=sequencer_id,
    )


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


def verify_batch_inclusion(
    kind: InclusionKind,
    canonical_leaf: bytes,
    proof: MerkleProof,
    canonical_header: bytes,
    header_signature: bytes,
    authorization: SequencerAuthorization,
    signatures: LocalSignatureVerifier,
) -> InclusionVerification:
    header = decode_batch_header(canonical_header)
    if (
        header.batch_number < authorization.first_batch_number
        or header.batch_number > authorization.last_batch_number
        or not _equal(header.sequencer_id, _exact(authorization.sequencer_id, 32))
    ):
        _failure()
    digest = _digest(_BATCH_HEADER_DOMAIN, canonical_header)
    if not signatures.verify_ed25519(
        _exact(authorization.public_key, 32),
        _exact(header_signature, 64),
        digest,
    ):
        _failure()
    roots = {
        "activity": header.activity_merkle_root,
        "receipt": header.receipt_merkle_root,
        "event": header.event_merkle_root,
        "state": header.resulting_state_root,
    }
    root = roots[kind]
    verify_merkle_inclusion(canonical_leaf, proof, root)
    return InclusionVerification(
        level="state-proven" if kind == "state" else "batch-included",
        header=header,
        header_digest=digest,
        root=root,
    )


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
    settlement_reference: bytes | None = None


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


def _u32(value: int) -> bytes:
    if value < 0 or value > 0xFFFF_FFFF:
        _failure()
    return value.to_bytes(4, "big", signed=False)


def _u64(value: int) -> bytes:
    if value < 0 or value > 0xFFFF_FFFF_FFFF_FFFF:
        _failure()
    return value.to_bytes(8, "big", signed=False)


def _attestation_message(attestation: CheckpointAttestation) -> bytes:
    return b"".join((
        _exact(attestation.checkpoint_id, 32),
        _exact(attestation.checkpoint_hash, 32),
        _exact(attestation.guarantor_id, 32),
        _u64(attestation.batch_number),
        _exact(attestation.data_availability_root, 32),
        bytes((
            1 if attestation.replayed else 0,
            1 if attestation.data_possessed else 0,
            attestation.availability_class_mask,
        )),
        _u64(attestation.attested_at_ms),
    ))


def verify_checkpoint(
    verification: CheckpointVerificationInput,
    signatures: LocalSignatureVerifier,
) -> CheckpointVerification:
    certificate = verification.certificate
    if not verification.availability_obtained or len(certificate.validity_proof) > 0xFFFF_FFFF:
        _failure()
    header = decode_batch_header(certificate.canonical_header)
    checkpoint_id = _digest(
        _CHECKPOINT_DOMAIN,
        certificate.canonical_header,
        _u32(len(certificate.validity_proof)),
        certificate.validity_proof,
    )
    if not _equal(checkpoint_id, _exact(verification.registered_checkpoint_id, 32)):
        _failure()
    if certificate.threshold <= 0:
        _failure()
    seen: set[bytes] = set()
    achieved = 0
    for attestation in certificate.attestations:
        guarantor_id = _exact(attestation.guarantor_id, 32)
        if (
            guarantor_id in seen
            or not _equal(attestation.checkpoint_id, checkpoint_id)
            or not _equal(attestation.checkpoint_hash, checkpoint_id)
            or attestation.batch_number != header.batch_number
            or not _equal(attestation.data_availability_root, header.data_availability_root)
            or not attestation.replayed
            or not attestation.data_possessed
            or attestation.availability_class_mask != _ALL_AVAILABILITY_CLASSES
            or attestation.attested_at_ms <= 0
        ):
            _failure()
        seen.add(guarantor_id)
        member = next(
            (
                candidate
                for candidate in verification.bonded_set
                if candidate.bonded and _equal(candidate.guarantor_id, guarantor_id)
            ),
            None,
        )
        if member is None:
            _failure()
        digest = _digest(_CHECKPOINT_DOMAIN, _attestation_message(attestation))
        if not signatures.verify_secp256k1(
            _exact(member.public_key, 33),
            _exact(attestation.signature, 64),
            digest,
        ):
            _failure()
        achieved += 1
    if achieved < certificate.threshold:
        _failure()
    settlement = certificate.settlement_reference
    if settlement is not None and (
        not settlement
        or verification.registered_settlement_reference is None
        or not _equal(settlement, verification.registered_settlement_reference)
    ):
        _failure()
    return CheckpointVerification(
        level="checkpoint-finalised" if settlement is None else "settlement-anchored",
        checkpoint_id=checkpoint_id,
        achieved=achieved,
        required=certificate.threshold,
        header=header,
    )
