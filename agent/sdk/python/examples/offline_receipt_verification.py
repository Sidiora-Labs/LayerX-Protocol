from dataclasses import dataclass

from layerx_sdk import VerificationLevel, VerifiedRead, require_verified


@dataclass(frozen=True)
class OfflineReceipt:
    canonical_bytes: bytes
    receipt_digest: bytes


def offline_receipt_verification(read: VerifiedRead[OfflineReceipt]) -> OfflineReceipt:
    return require_verified(VerificationLevel.SEQUENCER_SIGNED, read).value
