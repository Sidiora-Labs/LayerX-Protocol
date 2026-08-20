//! Lowercase hexadecimal encoding shared by registry identifiers and digests.

use crate::RegistryError;

const DIGITS: &[u8; 16] = b"0123456789abcdef";

/// Encodes bytes as lowercase hexadecimal.
#[must_use]
pub fn encode(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}

/// Decodes exactly thirty-two hexadecimal-encoded bytes.
///
/// # Errors
///
/// Refuses wrong lengths and non-hexadecimal characters.
pub fn decode_digest(text: &str) -> Result<[u8; 32], RegistryError> {
    if text.len() != 64 {
        return Err(RegistryError::InvalidDigestEncoding);
    }
    let mut digest = [0_u8; 32];
    for (slot, pair) in digest.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
        let high = nibble(pair[0]).ok_or(RegistryError::InvalidDigestEncoding)?;
        let low = nibble(pair[1]).ok_or(RegistryError::InvalidDigestEncoding)?;
        *slot = (high << 4) | low;
    }
    Ok(digest)
}

/// Decodes an even-length hexadecimal string.
///
/// # Errors
///
/// Refuses odd lengths and non-hexadecimal characters.
pub fn decode(text: &str) -> Result<Vec<u8>, RegistryError> {
    if !text.len().is_multiple_of(2) {
        return Err(RegistryError::InvalidDigestEncoding);
    }
    let mut bytes = Vec::with_capacity(text.len() / 2);
    for pair in text.as_bytes().chunks_exact(2) {
        let high = nibble(pair[0]).ok_or(RegistryError::InvalidDigestEncoding)?;
        let low = nibble(pair[1]).ok_or(RegistryError::InvalidDigestEncoding)?;
        bytes.push((high << 4) | low);
    }
    Ok(bytes)
}

fn nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
