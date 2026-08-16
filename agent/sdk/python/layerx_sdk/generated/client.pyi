# Generated from the LayerX Agent API schema. Do not hand-edit.

from enum import IntEnum
from typing import Generic, Literal, Mapping, Protocol, TypeAlias, TypeVar

def layerx_sdk_py_package() -> Mapping[str, str | int]: ...

Amount: TypeAlias = int
def parse_amount(value: str) -> Amount: ...

BudgetLimit: TypeAlias = int
def parse_budget_limit(value: str) -> BudgetLimit: ...

Sequence: TypeAlias = int
def parse_sequence(value: str) -> Sequence: ...

TimestampSeconds: TypeAlias = int
def parse_timestamp_seconds(value: str) -> TimestampSeconds: ...

class VerificationLevel(IntEnum):
    UNVERIFIED = 0
    SEQUENCER_SIGNED = 1
    BATCH_INCLUDED = 2
    STATE_PROVEN = 3
    CHECKPOINT_FINALISED = 4
    SETTLEMENT_ANCHORED = 5

ErrorClass: TypeAlias = Literal["TransportFailure", "Deadline", "ProtocolIncompatibility", "UnavailableCapability", "CoreRejection", "VerificationFailure", "PolicyRefusal", "CapabilityRefusal", "BudgetRefusal", "RateLimit", "IdempotencyConflict", "InternalFault"]
Operation: TypeAlias = Literal["agent.register", "approval.approve", "approval.get", "approval.list", "approval.reject", "availability.fetch", "budget.create", "budget.fund", "budget.list", "budget.reconciliation", "budget.revoke", "capability.attenuate", "capability.create", "capability.list", "capability.revoke", "export.offline", "prepare", "project", "read.account", "read.balance", "read.batch", "read.checkpoint", "read.history", "read.module_state", "read.proof_bundle", "session.close", "session.list", "session.open", "session.refresh", "sign", "submit", "subscription.acknowledge", "subscription.create", "subscription.delete", "subscription.health", "subscription.list", "subscription.pause", "subscription.resume", "track", "wait"]

T = TypeVar("T")
R = TypeVar("R")

class VerifiedRead(Generic[R]):
    value: R
    achieved_verification_level: VerificationLevel
    chain_head: int
    latest_batch: str
    latest_checkpoint: str
    value_sequence: int
    def __init__(
        self,
        value: R,
        achieved_verification_level: VerificationLevel,
        chain_head: int,
        latest_batch: str,
        latest_checkpoint: str,
        value_sequence: int,
    ) -> None: ...

def require_verified(requested: VerificationLevel, read: VerifiedRead[R]) -> VerifiedRead[R]: ...

class SubmissionUnknown:
    kind: Literal["Unknown"]
    def __init__(self, kind: Literal["Unknown"] = "Unknown") -> None: ...

class SubmissionExecuted:
    receipt_ref: str
    kind: Literal["Executed"]
    def __init__(self, receipt_ref: str, kind: Literal["Executed"] = "Executed") -> None: ...

class SubmissionFailed:
    protocol_result_code: int
    kind: Literal["Failed"]
    def __init__(self, protocol_result_code: int, kind: Literal["Failed"] = "Failed") -> None: ...

class SubmissionPending:
    stage: str
    kind: Literal["Pending"]
    def __init__(self, stage: str, kind: Literal["Pending"] = "Pending") -> None: ...

SubmissionState: TypeAlias = SubmissionUnknown | SubmissionExecuted | SubmissionFailed | SubmissionPending

class IdempotentMutation(Generic[T]):
    request_id: int
    key: bytes
    body_digest: bytes
    operation: T
    def __init__(self, request_id: int, key: bytes, body_digest: bytes, operation: T) -> None: ...

class ApiError(Exception):
    error_class: ErrorClass
    protocol_result_code: int | None
    retriable: bool
    request_id: int
    reason: str
    def __init__(
        self,
        error_class: ErrorClass,
        protocol_result_code: int | None,
        retriable: bool,
        request_id: int,
        reason: str,
    ) -> None: ...

class Transport(Protocol):
    def call(self, operation: Operation, request: object) -> object: ...

class Client:
    def __init__(self, transport: Transport) -> None: ...
    def call(self, operation: Operation, request: object) -> object: ...
