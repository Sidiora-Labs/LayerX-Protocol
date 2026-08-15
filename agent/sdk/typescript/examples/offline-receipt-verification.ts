import { VerificationLevel, requireVerified, type VerifiedRead } from "../src/index.js";

export interface OfflineReceipt { readonly canonicalBytes: Uint8Array; readonly receiptDigest: Uint8Array }

export function offlineReceiptVerification(read: VerifiedRead<OfflineReceipt>): OfflineReceipt {
  return requireVerified(VerificationLevel.SequencerSigned, read).value;
}
