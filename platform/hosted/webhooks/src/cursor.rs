//! Stable redelivery cursors.
//!
//! A cursor is the durable log position an endpoint has already been offered,
//! bound to that endpoint by a keyless checksum so a cursor issued for one
//! endpoint can never be replayed against another. Cursors survive restarts and
//! stay meaningful for the whole retention window of the event log.

use crate::encoding::{digest, hex_encode};
use crate::error::WebhookError;
use crate::events::EndpointId;

/// Wire prefix of every issued cursor.
pub const CURSOR_PREFIX: &str = "lxwc1_";

const CURSOR_DOMAIN: &[u8] = b"LayerX/webhooks/cursor/v1\0";
const POSITION_HEX: usize = 16;
const CHECKSUM_HEX: usize = 16;

fn checksum(endpoint: &EndpointId, position: u64) -> String {
    let mut message = Vec::with_capacity(CURSOR_DOMAIN.len() + endpoint.as_str().len() + 9);
    message.extend_from_slice(CURSOR_DOMAIN);
    message.extend_from_slice(endpoint.as_str().as_bytes());
    message.push(0);
    message.extend_from_slice(&position.to_be_bytes());
    hex_encode(&digest(&message)[..8])
}

/// Issues the cursor for one already-offered log position.
#[must_use]
pub fn encode(endpoint: &EndpointId, position: u64) -> String {
    format!(
        "{CURSOR_PREFIX}{position:016x}{}",
        checksum(endpoint, position)
    )
}

/// Returns the cursor that starts a stream before its first event.
#[must_use]
pub fn start(endpoint: &EndpointId) -> String {
    encode(endpoint, 0)
}

/// Recovers the log position a cursor names.
///
/// # Errors
/// Returns [`WebhookError::InvalidCursor`] when the prefix, length or checksum
/// does not match a cursor issued for this endpoint.
pub fn decode(endpoint: &EndpointId, cursor: &str) -> Result<u64, WebhookError> {
    let body = cursor
        .strip_prefix(CURSOR_PREFIX)
        .ok_or(WebhookError::InvalidCursor)?;
    if body.len() != POSITION_HEX + CHECKSUM_HEX {
        return Err(WebhookError::InvalidCursor);
    }
    let (encoded_position, presented) = body.split_at(POSITION_HEX);
    let position =
        u64::from_str_radix(encoded_position, 16).map_err(|_| WebhookError::InvalidCursor)?;
    if checksum(endpoint, position) != presented {
        return Err(WebhookError::InvalidCursor);
    }
    Ok(position)
}
