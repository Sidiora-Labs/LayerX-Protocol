from __future__ import annotations

from dataclasses import dataclass
from typing import Literal, Mapping, cast

from .production import IdempotencyKey, ProductionClient
from .verifier import AuthorizedReceiptBatch, LocalSignatureVerifier, ReceiptVerification, verify_receipt_outcome

_MAX_CALLDATA = 1_048_576
_MAX_CAPABILITIES = 256

@dataclass(frozen=True)
class ProgramCall:
    program_id: str
    version: int
    code_hash: str
    abi_version: int
    entrypoint: str
    calldata: bytes
    fuel: int
    fee_limit: int
    capabilities: tuple[bytes, ...]
    signed_activity: bytes

@dataclass(frozen=True)
class ProgramExecution:
    verification: ReceiptVerification
    outcome: Mapping[str, object]
    terminal_attachments: tuple[bytes, ...]

ProgramSubmissionState = Literal["refused", "unknown", "executed"]

def _validate(call: ProgramCall) -> None:
    if len(call.program_id) != 64 or len(call.code_hash) != 64 or any(c not in "0123456789abcdef" for c in call.program_id + call.code_hash):
        raise ValueError("program and code hash must be canonical bytes32 hex")
    if call.version <= 0 or call.abi_version <= 0 or not call.entrypoint or len(call.entrypoint.encode()) > 255:
        raise ValueError("invalid program identity")
    if len(call.calldata) > _MAX_CALLDATA or len(call.capabilities) > _MAX_CAPABILITIES or any(not item or len(item) > 4096 for item in call.capabilities):
        raise ValueError("program call exceeds contract bounds")

def _verified(raw: Mapping[str, object], call: ProgramCall, signatures: LocalSignatureVerifier) -> ProgramExecution:
    receipt = cast(bytes, raw["receipt"])
    authority = cast(AuthorizedReceiptBatch, raw["authority"])
    activity_id = cast(bytes, raw["activity_id"])
    verification = verify_receipt_outcome(receipt, authority, signatures)
    protocol = verification.receipt
    if protocol.module_id != 9 or protocol.operation != 3 or protocol.module_version != call.abi_version or protocol.activity_id != activity_id:
        raise ValueError("program receipt binding failed")
    return ProgramExecution(verification, cast(Mapping[str, object], raw["outcome"]), tuple(cast(tuple[bytes, ...], raw["terminal_attachments"])))

class ProgramOperations:
    def __init__(self, client: ProductionClient, signatures: LocalSignatureVerifier) -> None:
        self._client = client
        self._signatures = signatures

    def discover(self, program_id: str) -> Mapping[str, object]:
        return self._client.agent("program.discover", {"program_id": program_id, "requested_verification_level": "sequencer-signed"})

    def interface(self, program_id: str, version: int) -> Mapping[str, object]:
        return self._client.agent("program.interface", {"program_id": program_id, "version": version, "requested_verification_level": "sequencer-signed"})

    def simulate(self, call: ProgramCall) -> ProgramExecution:
        _validate(call)
        return _verified(self._client.agent("program.simulate", call), call, self._signatures)

    def submit(self, call: ProgramCall, idempotency_key: IdempotencyKey) -> Mapping[str, object] | ProgramExecution:
        _validate(call)
        result = cast(Mapping[str, object], self._client.agent("program.call", call, idempotency_key=idempotency_key))
        return _verified(cast(Mapping[str, object], result["evidence"]), call, self._signatures) if result.get("state") == "executed" else result

    def receipt(self, idempotency_key: str, expected_activity_id: str) -> Mapping[str, object]:
        return self._client.agent("program.receipt", {"idempotency_key": idempotency_key, "expected_activity_id": expected_activity_id, "requested_verification_level": "sequencer-signed"})

    def activity(self, activity_id: str) -> Mapping[str, object]:
        return self._client.agent("program.activity", {"activity_id": activity_id, "requested_verification_level": "sequencer-signed"})

def platform_sdk_programs() -> str:
    return "receipt-verified-program-operations-v1"
