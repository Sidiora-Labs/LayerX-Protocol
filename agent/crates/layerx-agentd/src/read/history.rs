//! Strictly ordered, byte-preserving history with durable bounded cursors.

use layerx_client::lni::transport::FrameTransport;
use layerx_client::read::{self, HistoryCursor, HistoryItem, ReadContext, ReadError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Cursor {
    pub next_sequence: u64,
    pub end_sequence: u64,
    pub observed_head_sequence: u64,
    pub observed_checkpoint: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HistoryLimits {
    pub maximum_items: u16,
    pub maximum_response_bytes: usize,
    pub oldest_available_sequence: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryPage {
    pub items: Vec<HistoryItem>,
    pub cursor: Option<Cursor>,
    pub response_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HistoryReadError {
    Read(ReadError),
    InvalidLimits,
    CursorMismatch,
    PrunedRange { requested: u64, oldest: u64 },
    ItemTooLarge { sequence: u64, bytes: usize },
    Arithmetic,
}

/// Serves one bounded page while preserving exact core bytes and cursor continuity.
pub fn history(
    transport: &mut dyn FrameTransport,
    start_sequence: u64,
    end_sequence: u64,
    cursor: Option<Cursor>,
    limits: HistoryLimits,
    context: ReadContext,
) -> Result<HistoryPage, HistoryReadError> {
    if limits.maximum_items == 0 || limits.maximum_response_bytes == 0 {
        return Err(HistoryReadError::InvalidLimits);
    }
    let requested_start = cursor.map_or(start_sequence, |value| value.next_sequence);
    if requested_start < limits.oldest_available_sequence {
        return Err(HistoryReadError::PrunedRange {
            requested: requested_start,
            oldest: limits.oldest_available_sequence,
        });
    }
    if cursor.is_some_and(|value| {
        value.end_sequence != end_sequence
            || value.observed_head_sequence != context.head.chain_sequence
            || value.observed_checkpoint != context.head.finalised_checkpoint
            || value.next_sequence < start_sequence
    }) {
        return Err(HistoryReadError::CursorMismatch);
    }
    let client_cursor = cursor.map(|value| {
        HistoryCursor::from_coordinates(
            value.next_sequence,
            value.end_sequence,
            value.observed_head_sequence,
            value.observed_checkpoint,
        )
    });
    let page = read::history(
        transport,
        start_sequence,
        end_sequence,
        limits.maximum_items,
        client_cursor,
        context,
    )
    .map_err(HistoryReadError::Read)?;

    let mut bytes = 0_usize;
    let mut items = Vec::new();
    let mut truncated_at = None;
    for item in page.items {
        let item_bytes = item.canonical_bytes().len();
        if item_bytes > limits.maximum_response_bytes {
            return Err(HistoryReadError::ItemTooLarge {
                sequence: item.global_sequence,
                bytes: item_bytes,
            });
        }
        let projected = bytes
            .checked_add(item_bytes)
            .ok_or(HistoryReadError::Arithmetic)?;
        if projected > limits.maximum_response_bytes {
            truncated_at = Some(item.global_sequence);
            break;
        }
        bytes = projected;
        items.push(item);
    }
    let cursor = if let Some(next_sequence) = truncated_at {
        Some(Cursor {
            next_sequence,
            end_sequence,
            observed_head_sequence: context.head.chain_sequence,
            observed_checkpoint: context.head.finalised_checkpoint,
        })
    } else {
        page.cursor.map(|value| Cursor {
            next_sequence: value.next_sequence(),
            end_sequence,
            observed_head_sequence: context.head.chain_sequence,
            observed_checkpoint: context.head.finalised_checkpoint,
        })
    };
    Ok(HistoryPage {
        items,
        cursor,
        response_bytes: bytes,
    })
}
