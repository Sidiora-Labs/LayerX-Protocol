//! Verified receipt facts.
//!
//! A program never sees kernel state. It reads only the facts the core has
//! already verified for a digest the invoking activity named in an explicit
//! grant, decoded from the frozen 116-byte view.

use crate::abi::RECEIPT_ENCODING_BYTES;
use crate::amount::Amount;
use crate::error::{Field, ProgramError, Reason};

#[cfg(target_arch = "wasm32")]
use crate::error::HostRefusal;
#[cfg(target_arch = "wasm32")]
use crate::host;
#[cfg(target_arch = "wasm32")]
use crate::ids::ReceiptDigest;

const DIGEST_OFFSET: usize = 0;
const RESULT_CODE_OFFSET: usize = 32;
const ASSET_OFFSET: usize = 36;
const AMOUNT_OFFSET: usize = 68;
const STATE_ROOT_OFFSET: usize = 84;

/// Verified receipt facts exposed without raw kernel state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Receipt {
    /// Digest naming the verified receipt.
    pub digest: [u8; 32],
    /// Result code the receipt records.
    pub result_code: i32,
    /// Asset the receipt settles.
    pub asset: [u8; 32],
    /// Exact integer amount the receipt settles.
    pub amount: Amount,
    /// State root the receipt commits to.
    pub state_root: [u8; 32],
}

impl Receipt {
    /// Decodes the frozen receipt view the host writes.
    ///
    /// # Errors
    ///
    /// Refuses any encoding other than the exact host format.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProgramError> {
        if bytes.len() != RECEIPT_ENCODING_BYTES {
            return Err(ProgramError::value(Field::Receipt, Reason::Malformed));
        }
        Ok(Self {
            digest: fixed::<32>(bytes, DIGEST_OFFSET)?,
            result_code: i32::from_be_bytes(fixed::<4>(bytes, RESULT_CODE_OFFSET)?),
            asset: fixed::<32>(bytes, ASSET_OFFSET)?,
            amount: Amount::from_be_bytes(fixed::<16>(bytes, AMOUNT_OFFSET)?),
            state_root: fixed::<32>(bytes, STATE_ROOT_OFFSET)?,
        })
    }
}

fn fixed<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], ProgramError> {
    let end = offset
        .checked_add(N)
        .ok_or_else(|| ProgramError::value(Field::Receipt, Reason::Malformed))?;
    let slice = bytes
        .get(offset..end)
        .ok_or_else(|| ProgramError::value(Field::Receipt, Reason::Malformed))?;
    let mut output = [0u8; N];
    output.copy_from_slice(slice);
    Ok(output)
}

/// Reads the verified facts of one receipt named by an explicit grant.
///
/// # Errors
///
/// Refuses missing digest authority, absent or mismatched evidence, and a
/// host encoding outside the frozen view.
#[cfg(target_arch = "wasm32")]
pub fn read(receipt_digest: ReceiptDigest) -> Result<Receipt, ProgramError> {
    let digest = receipt_digest.bytes();
    let mut output = [0u8; RECEIPT_ENCODING_BYTES];
    let status = host::receipt_read(&digest, &mut output)?;
    let written = usize::try_from(status)
        .map_err(|_| ProgramError::value(Field::Receipt, Reason::Malformed))?;
    if written != RECEIPT_ENCODING_BYTES {
        return Err(ProgramError::value(Field::Receipt, Reason::Malformed));
    }
    let receipt = Receipt::decode(&output)?;
    if receipt.digest != digest {
        return Err(ProgramError::Host(HostRefusal::Evidence));
    }
    Ok(receipt)
}
