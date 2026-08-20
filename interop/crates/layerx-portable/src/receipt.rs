use std::fmt::{Display, Formatter};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use layerx_proof::receipt::{
    verify_outcome, AuthorizedBatch, VerificationFailure, VerifiedReceipt,
};
use serde::{Deserialize, Serialize};

/// Stable media payload identifier shared with every interop adapter.
pub const PORTABLE_RECEIPT_FORMAT: &str = "layerx-receipt-proof-v1";
const VERIFICATION_LEVEL: &str = "sequencer-signed";
const MAX_RECEIPT_BYTES: usize = 1_048_576;
const MAX_JSON_BYTES: usize = 1_500_000;

/// Self-contained, language-neutral receipt proof.
///
/// Every byte string is unpadded base64url. The batch facts are deliberately
/// repeated outside the receipt so the verifier can compare them with its
/// independently trusted [`AuthorizedBatch`]. Carrying those fields does not
/// itself make them authoritative.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PortableReceipt {
    format: String,
    verification_level: String,
    canonical_receipt: String,
    receipt_digest: String,
    batch_id: String,
    asset: String,
    previous_state_root: String,
    resulting_state_root: String,
    sequencer_public_key: String,
}

impl PortableReceipt {
    /// Exports one receipt only after its exact canonical encoding, state-root
    /// chain, balance invariants and sequencer signature verify locally.
    /// Rejected protocol outcomes remain exportable as rejected outcomes;
    /// this constructor never upgrades them to success.
    ///
    /// # Errors
    ///
    /// Returns the exact receipt verification stage that failed.
    pub fn export(
        canonical_receipt: &[u8],
        authorised: &AuthorizedBatch,
    ) -> Result<Self, PortableReceiptError> {
        if canonical_receipt.is_empty() || canonical_receipt.len() > MAX_RECEIPT_BYTES {
            return Err(PortableReceiptError::ReceiptBounds);
        }
        let verified =
            verify_outcome(canonical_receipt, authorised).map_err(PortableReceiptError::Receipt)?;
        let digest = verified
            .evidence()
            .receipt_digest()
            .ok_or(PortableReceiptError::MissingReceiptDigest)?;
        Ok(Self {
            format: PORTABLE_RECEIPT_FORMAT.to_owned(),
            verification_level: VERIFICATION_LEVEL.to_owned(),
            canonical_receipt: URL_SAFE_NO_PAD.encode(canonical_receipt),
            receipt_digest: URL_SAFE_NO_PAD.encode(digest),
            batch_id: URL_SAFE_NO_PAD.encode(authorised.batch_id()),
            asset: URL_SAFE_NO_PAD.encode(authorised.asset()),
            previous_state_root: URL_SAFE_NO_PAD.encode(authorised.previous_state_root()),
            resulting_state_root: URL_SAFE_NO_PAD.encode(authorised.resulting_state_root()),
            sequencer_public_key: URL_SAFE_NO_PAD.encode(authorised.sequencer_public_key()),
        })
    }

    /// Parses the bounded JSON wire form. Parsing alone establishes no trust;
    /// callers must use [`Self::verify`] before consuming any receipt fact.
    ///
    /// # Errors
    ///
    /// Refuses oversize, malformed, extended, or non-canonical JSON fields.
    pub fn from_json(bytes: &[u8]) -> Result<Self, PortableReceiptError> {
        if bytes.is_empty() || bytes.len() > MAX_JSON_BYTES {
            return Err(PortableReceiptError::JsonBounds);
        }
        let portable: Self =
            serde_json::from_slice(bytes).map_err(|_| PortableReceiptError::JsonShape)?;
        portable.decode_fields()?;
        Ok(portable)
    }

    /// Parses and verifies one JSON proof without exposing an intermediate
    /// unverified receipt value.
    ///
    /// # Errors
    ///
    /// Returns the first wire-shape, receipt, digest or signature refusal.
    pub fn verify_json(
        bytes: &[u8],
        trusted_batch: &AuthorizedBatch,
    ) -> Result<PortableVerifiedReceipt, PortableReceiptError> {
        Self::from_json(bytes)?.verify(trusted_batch)
    }

    /// Encodes the stable JSON object. Struct field order is stable for
    /// reproducible vectors, but consumers must treat JSON member order as
    /// insignificant.
    ///
    /// # Errors
    ///
    /// Returns an encoding error if the serializer refuses the object.
    pub fn to_json(&self) -> Result<Vec<u8>, PortableReceiptError> {
        self.decode_fields()?;
        serde_json::to_vec(self).map_err(|_| PortableReceiptError::JsonShape)
    }

    /// Verifies this proof against independently trusted batch authorization
    /// using the standalone `LayerX` proof library. No gateway or live
    /// infrastructure is consulted.
    ///
    /// # Errors
    ///
    /// Refuses a format/version mismatch, non-canonical base64url, mismatched
    /// receipt digest, or any canonical receipt/signature/invariant failure.
    pub fn verify(
        &self,
        trusted_batch: &AuthorizedBatch,
    ) -> Result<PortableVerifiedReceipt, PortableReceiptError> {
        let decoded = self.decode_fields()?;
        if decoded.batch_id != trusted_batch.batch_id()
            || decoded.asset != trusted_batch.asset()
            || decoded.previous_state_root != trusted_batch.previous_state_root()
            || decoded.resulting_state_root != trusted_batch.resulting_state_root()
            || decoded.sequencer_public_key != trusted_batch.sequencer_public_key()
        {
            return Err(PortableReceiptError::BatchAuthorizationMismatch);
        }
        let verified = verify_outcome(&decoded.canonical_receipt, trusted_batch)
            .map_err(PortableReceiptError::Receipt)?;
        let actual = verified
            .evidence()
            .receipt_digest()
            .ok_or(PortableReceiptError::MissingReceiptDigest)?;
        if actual != decoded.receipt_digest {
            return Err(PortableReceiptError::ReceiptDigestMismatch);
        }
        Ok(PortableVerifiedReceipt {
            receipt: verified,
            receipt_digest: actual,
            authorised_batch: *trusted_batch,
        })
    }

    /// Returns the wire-format identifier.
    #[must_use]
    pub fn format(&self) -> &str {
        &self.format
    }

    /// Returns the encoded exact canonical receipt.
    #[must_use]
    pub fn canonical_receipt(&self) -> &str {
        &self.canonical_receipt
    }

    /// Returns the encoded receipt-signature digest.
    #[must_use]
    pub fn receipt_digest(&self) -> &str {
        &self.receipt_digest
    }

    fn decode_fields(&self) -> Result<DecodedReceipt, PortableReceiptError> {
        if self.format != PORTABLE_RECEIPT_FORMAT {
            return Err(PortableReceiptError::UnsupportedFormat);
        }
        if self.verification_level != VERIFICATION_LEVEL {
            return Err(PortableReceiptError::UnsupportedVerificationLevel);
        }
        let canonical_receipt = decode_bounded(
            "canonicalReceipt",
            &self.canonical_receipt,
            1,
            MAX_RECEIPT_BYTES,
        )?;
        Ok(DecodedReceipt {
            canonical_receipt,
            receipt_digest: decode_fixed("receiptDigest", &self.receipt_digest)?,
            batch_id: decode_fixed("batchId", &self.batch_id)?,
            asset: decode_fixed("asset", &self.asset)?,
            previous_state_root: decode_fixed("previousStateRoot", &self.previous_state_root)?,
            resulting_state_root: decode_fixed("resultingStateRoot", &self.resulting_state_root)?,
            sequencer_public_key: decode_fixed("sequencerPublicKey", &self.sequencer_public_key)?,
        })
    }
}

struct DecodedReceipt {
    canonical_receipt: Vec<u8>,
    receipt_digest: [u8; 32],
    batch_id: [u8; 32],
    asset: [u8; 32],
    previous_state_root: [u8; 32],
    resulting_state_root: [u8; 32],
    sequencer_public_key: [u8; 32],
}

/// A receipt value produced only after the complete portable proof verifies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortableVerifiedReceipt {
    receipt: VerifiedReceipt,
    receipt_digest: [u8; 32],
    authorised_batch: AuthorizedBatch,
}

impl PortableVerifiedReceipt {
    /// Returns the verified canonical protocol receipt.
    #[must_use]
    pub const fn receipt(&self) -> &VerifiedReceipt {
        &self.receipt
    }

    /// Returns the verified receipt-signature digest.
    #[must_use]
    pub const fn receipt_digest(&self) -> [u8; 32] {
        self.receipt_digest
    }

    /// Returns the independently supplied batch authorization facts.
    #[must_use]
    pub const fn authorised_batch(&self) -> &AuthorizedBatch {
        &self.authorised_batch
    }
}

/// Typed portable-receipt refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortableReceiptError {
    JsonBounds,
    JsonShape,
    ReceiptBounds,
    UnsupportedFormat,
    UnsupportedVerificationLevel,
    InvalidBase64(&'static str),
    InvalidLength(&'static str),
    Receipt(VerificationFailure),
    BatchAuthorizationMismatch,
    MissingReceiptDigest,
    ReceiptDigestMismatch,
}

impl Display for PortableReceiptError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::JsonBounds => formatter.write_str("portable receipt JSON is outside its bounds"),
            Self::JsonShape => formatter.write_str("portable receipt JSON shape is invalid"),
            Self::ReceiptBounds => formatter.write_str("canonical receipt is outside its bounds"),
            Self::UnsupportedFormat => {
                formatter.write_str("portable receipt format is unsupported")
            }
            Self::UnsupportedVerificationLevel => {
                formatter.write_str("portable receipt verification level is unsupported")
            }
            Self::InvalidBase64(field) => write!(formatter, "{field} is not canonical base64url"),
            Self::InvalidLength(field) => write!(formatter, "{field} has an invalid length"),
            Self::Receipt(VerificationFailure { check }) => {
                write!(formatter, "receipt verification failed at {check:?}")
            }
            Self::BatchAuthorizationMismatch => {
                formatter.write_str("portable receipt does not match the trusted batch")
            }
            Self::MissingReceiptDigest => {
                formatter.write_str("verified receipt did not produce a receipt digest")
            }
            Self::ReceiptDigestMismatch => {
                formatter.write_str("portable receipt digest does not match the receipt")
            }
        }
    }
}

impl std::error::Error for PortableReceiptError {}

fn decode_fixed<const N: usize>(
    field: &'static str,
    encoded: &str,
) -> Result<[u8; N], PortableReceiptError> {
    let decoded = decode_bounded(field, encoded, N, N)?;
    decoded
        .try_into()
        .map_err(|_| PortableReceiptError::InvalidLength(field))
}

fn decode_bounded(
    field: &'static str,
    encoded: &str,
    minimum: usize,
    maximum: usize,
) -> Result<Vec<u8>, PortableReceiptError> {
    if encoded.is_empty() || encoded.contains('=') {
        return Err(PortableReceiptError::InvalidBase64(field));
    }
    let decoded = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| PortableReceiptError::InvalidBase64(field))?;
    if decoded.len() < minimum || decoded.len() > maximum {
        return Err(PortableReceiptError::InvalidLength(field));
    }
    if URL_SAFE_NO_PAD.encode(&decoded) != encoded {
        return Err(PortableReceiptError::InvalidBase64(field));
    }
    Ok(decoded)
}
