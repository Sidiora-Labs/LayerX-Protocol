# Generated from the LayerX Agent API schema. Do not hand-edit.

from dataclasses import dataclass
from enum import IntEnum
from typing import Generic, Literal, Protocol, TypeVar

{{SCALARS}}

class VerificationLevel(IntEnum):
{{LEVELS}}

ErrorClass = Literal[{{ERRORS}}]
Operation = Literal[{{OPERATIONS}}]

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

@dataclass(frozen=True)
class IdempotentMutation(Generic[T]):
    request_id: int
    key: bytes
    body_digest: bytes
    operation: T

class Transport(Protocol):
    def call(self, operation: Operation, request: object) -> object: ...

class Client:
    def __init__(self, transport: Transport) -> None:
        self._transport = transport

    def call(self, operation: Operation, request: object) -> object:
        return self._transport.call(operation, request)
