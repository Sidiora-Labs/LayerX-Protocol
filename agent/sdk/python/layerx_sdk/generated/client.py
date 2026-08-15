# Generated from the LayerX Agent API schema. Do not hand-edit.

from dataclasses import dataclass
from enum import IntEnum
from types import MappingProxyType
from typing import Generic, Literal, Mapping, Protocol, TypeAlias, TypeVar

_PACKAGE_METADATA: Mapping[str, str | int] = MappingProxyType({
    "name": "layerx-sdk",
    "version": "0.1.0",
    "contract_major": 1,
})

def layerx_sdk_py_package() -> Mapping[str, str | int]:
    return _PACKAGE_METADATA

Amount = int

def parse_amount(value: str) -> Amount:
    if not value or (value != "0" and value.startswith("0")) or not value.isascii() or not value.isdigit():
        raise ValueError("invalid Amount")
    parsed = int(value)
    if parsed > 340282366920938463463374607431768211455:
        raise OverflowError("Amount out of range")
    return parsed

BudgetLimit = int

def parse_budget_limit(value: str) -> BudgetLimit:
    if not value or (value != "0" and value.startswith("0")) or not value.isascii() or not value.isdigit():
        raise ValueError("invalid BudgetLimit")
    parsed = int(value)
    if parsed > 340282366920938463463374607431768211455:
        raise OverflowError("BudgetLimit out of range")
    return parsed

Sequence = int

def parse_sequence(value: str) -> Sequence:
    if not value or (value != "0" and value.startswith("0")) or not value.isascii() or not value.isdigit():
        raise ValueError("invalid Sequence")
    parsed = int(value)
    if parsed > 18446744073709551615:
        raise OverflowError("Sequence out of range")
    return parsed

TimestampSeconds = int

def parse_timestamp_seconds(value: str) -> TimestampSeconds:
    if not value or (value != "0" and value.startswith("0")) or not value.isascii() or not value.isdigit():
        raise ValueError("invalid TimestampSeconds")
    parsed = int(value)
    if parsed > 18446744073709551615:
        raise OverflowError("TimestampSeconds out of range")
    return parsed

class VerificationLevel(IntEnum):
    UNVERIFIED = 0
    SEQUENCER_SIGNED = 1
    BATCH_INCLUDED = 2
    STATE_PROVEN = 3
    CHECKPOINT_FINALISED = 4
    SETTLEMENT_ANCHORED = 5

ErrorClass = Literal["TransportFailure", "Deadline", "ProtocolIncompatibility", "UnavailableCapability", "CoreRejection", "VerificationFailure", "PolicyRefusal", "CapabilityRefusal", "BudgetRefusal", "RateLimit", "IdempotencyConflict", "InternalFault"]
Operation = Literal["agent.register", "availability.fetch", "budget.create", "budget.fund", "budget.list", "budget.reconciliation", "budget.revoke", "capability.attenuate", "capability.create", "capability.list", "capability.revoke", "export.offline", "prepare", "project", "read.account", "read.balance", "read.batch", "read.checkpoint", "read.history", "read.module_state", "read.proof_bundle", "session.close", "session.list", "session.open", "session.refresh", "sign", "submit", "subscription.acknowledge", "subscription.create", "subscription.delete", "subscription.health", "subscription.list", "subscription.pause", "subscription.resume", "track", "wait"]

T = TypeVar("T")
R = TypeVar("R")

@dataclass(frozen=True)
class VerifiedRead(Generic[R]):
    value: R
    achieved_verification_level: VerificationLevel
    chain_head: int
    latest_batch: str
    latest_checkpoint: str
    value_sequence: int

def require_verified(requested: VerificationLevel, read: VerifiedRead[R]) -> VerifiedRead[R]:
    if read.achieved_verification_level == VerificationLevel.UNVERIFIED:
        raise ValueError("unverified_read")
    if read.achieved_verification_level < requested:
        raise ValueError(
            f"verification_below_requested:{requested.value}:{read.achieved_verification_level.value}"
        )
    return read

@dataclass(frozen=True)
class SubmissionUnknown:
    kind: Literal["Unknown"] = "Unknown"

@dataclass(frozen=True)
class SubmissionExecuted:
    receipt_ref: str
    kind: Literal["Executed"] = "Executed"

@dataclass(frozen=True)
class SubmissionFailed:
    protocol_result_code: int
    kind: Literal["Failed"] = "Failed"

@dataclass(frozen=True)
class SubmissionPending:
    stage: str
    kind: Literal["Pending"] = "Pending"

SubmissionState: TypeAlias = (
    SubmissionUnknown | SubmissionExecuted | SubmissionFailed | SubmissionPending
)

@dataclass(frozen=True)
class IdempotentMutation(Generic[T]):
    request_id: int
    key: bytes
    body_digest: bytes
    operation: T

@dataclass(frozen=True)
class ApiError(Exception):
    error_class: ErrorClass
    protocol_result_code: int | None
    retriable: bool
    request_id: int
    reason: str

    def __str__(self) -> str:
        return f"{self.error_class}:{self.reason}"

class Transport(Protocol):
    def call(self, operation: Operation, request: object) -> object: ...

class Client:
    def __init__(self, transport: Transport) -> None:
        self._transport = transport

    def call(self, operation: Operation, request: object) -> object:
        return self._transport.call(operation, request)
