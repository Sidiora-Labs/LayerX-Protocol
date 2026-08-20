//! Authentication-free queries over protocol-public explorer state.

use crate::verify::{PastedInclusion, VerificationReport, Verifier, VerifyError};
use crate::{
    AccountActivityRecord, BatchRecord, CheckpointRecord, Freshness, Indexed, Indexer,
    PublicRecord, RecordId,
};

const MAXIMUM_PAGE_SIZE: usize = 100;

/// One bounded newest-first public page and its exclusive continuation point.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_before: Option<u64>,
}

/// Typed refusal for invalid or silently incomplete public queries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryError {
    InvalidPageSize,
    AccountIndexIncomplete { batch: u64 },
}

/// Every public refusal carries the same live freshness disclosure as success.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueryFailure {
    pub error: QueryError,
    pub freshness: Freshness,
}

/// Proof refusal paired with live public-index freshness. The error is boxed
/// so the response result remains inexpensive to move at the HTTP boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationFailure {
    pub error: Box<VerifyError>,
    pub freshness: Freshness,
}

/// Public explorer surface. Construction and every method deliberately omit
/// principal, session, profile and notification inputs.
#[derive(Clone, Copy)]
pub struct PublicExplorer<'a> {
    index: &'a Indexer,
    verifier: &'a Verifier,
}

impl Indexer {
    /// Opens the authentication-free protocol-public query surface.
    #[must_use]
    pub const fn public<'a>(&'a self, verifier: &'a Verifier) -> PublicExplorer<'a> {
        PublicExplorer {
            index: self,
            verifier,
        }
    }
}

impl PublicExplorer<'_> {
    /// Browses finalised checkpoints newest-first.
    ///
    /// # Errors
    ///
    /// Refuses a zero or over-limit page size.
    pub fn checkpoints(
        &self,
        before_batch: Option<u64>,
        limit: usize,
    ) -> Result<Indexed<Page<CheckpointRecord>>, QueryFailure> {
        validate_limit(limit).map_err(|error| self.failure(error))?;
        let mut records = self
            .index
            .checkpoints_by_batch
            .iter()
            .rev()
            .filter(|(batch, _)| before_batch.is_none_or(|before| **batch < before))
            .filter_map(|(_, identifier)| self.index.checkpoints.get(identifier).cloned())
            .take(limit.saturating_add(1))
            .collect::<Vec<_>>();
        Ok(Indexed {
            value: page(&mut records, limit, |record| record.batch_number),
            freshness: self.index.freshness(),
        })
    }

    /// Browses complete availability batches newest-first.
    ///
    /// # Errors
    ///
    /// Refuses a zero or over-limit page size.
    pub fn batches(
        &self,
        before_batch: Option<u64>,
        limit: usize,
    ) -> Result<Indexed<Page<BatchRecord>>, QueryFailure> {
        validate_limit(limit).map_err(|error| self.failure(error))?;
        let mut records = self
            .index
            .batches
            .iter()
            .rev()
            .filter(|(batch, _)| before_batch.is_none_or(|before| **batch < before))
            .map(|(_, record)| record.clone())
            .take(limit.saturating_add(1))
            .collect::<Vec<_>>();
        Ok(Indexed {
            value: page(&mut records, limit, |record| record.batch_number),
            freshness: self.index.freshness(),
        })
    }

    /// Looks up one protocol-public receipt by its content identifier.
    #[must_use]
    pub fn receipt(&self, identifier: RecordId) -> Indexed<Option<PublicRecord>> {
        self.index.receipt(identifier)
    }

    /// Looks up a receipt using either its protocol receipt digest or activity
    /// identifier, not the explorer's content-addressed row key.
    #[must_use]
    pub fn receipt_by_id(&self, identifier: [u8; 32]) -> Indexed<Option<PublicRecord>> {
        Indexed {
            value: self
                .index
                .receipts_by_protocol_id
                .get(&identifier)
                .and_then(|identifier| self.index.receipts.get(identifier))
                .cloned(),
            freshness: self.index.freshness(),
        }
    }

    /// Lists receipt-verified activity for one public protocol account hash.
    ///
    /// # Errors
    ///
    /// Refuses invalid bounds and any view for which an indexed batch has not
    /// completed independent receipt-authority verification.
    pub fn account_activity(
        &self,
        account: [u8; 32],
        before_sequence: Option<u64>,
        limit: usize,
    ) -> Result<Indexed<Page<AccountActivityRecord>>, QueryFailure> {
        validate_limit(limit).map_err(|error| self.failure(error))?;
        if let Some(batch) = self
            .index
            .batches
            .keys()
            .find(|batch| !self.index.receipt_authority_batches.contains(batch))
        {
            return Err(self.failure(QueryError::AccountIndexIncomplete { batch: *batch }));
        }
        let mut records = self
            .index
            .account_activities
            .values()
            .filter(|record| record.from == account || record.to == account)
            .filter(|record| before_sequence.is_none_or(|before| record.global_sequence < before))
            .cloned()
            .collect::<Vec<_>>();
        records.sort_by(|left, right| {
            right
                .global_sequence
                .cmp(&left.global_sequence)
                .then_with(|| right.receipt_id.cmp(&left.receipt_id))
        });
        records.truncate(limit.saturating_add(1));
        Ok(Indexed {
            value: page(&mut records, limit, |record| record.global_sequence),
            freshness: self.index.freshness(),
        })
    }

    /// Verifies pasted receipt bytes independently, pairing the proof result
    /// with current explorer freshness without consulting indexed receipt state.
    ///
    /// # Errors
    ///
    /// Returns the proof failure and current freshness together.
    pub fn verify_receipt(
        &self,
        pasted_receipt: &[u8],
    ) -> Result<Indexed<VerificationReport>, VerificationFailure> {
        Ok(Indexed {
            value: self
                .verifier
                .receipt(pasted_receipt)
                .map_err(|error| VerificationFailure {
                    error: Box::new(error),
                    freshness: self.index.freshness(),
                })?,
            freshness: self.index.freshness(),
        })
    }

    /// Verifies a pasted inclusion proof independently and pairs the result
    /// with current explorer freshness.
    ///
    /// # Errors
    ///
    /// Returns the proof failure and current freshness together.
    pub fn verify_inclusion(
        &self,
        pasted: &PastedInclusion<'_>,
    ) -> Result<Indexed<VerificationReport>, VerificationFailure> {
        Ok(Indexed {
            value: self
                .verifier
                .inclusion(pasted)
                .map_err(|error| VerificationFailure {
                    error: Box::new(error),
                    freshness: self.index.freshness(),
                })?,
            freshness: self.index.freshness(),
        })
    }

    fn failure(&self, error: QueryError) -> QueryFailure {
        QueryFailure {
            error,
            freshness: self.index.freshness(),
        }
    }
}

fn validate_limit(limit: usize) -> Result<(), QueryError> {
    if limit == 0 || limit > MAXIMUM_PAGE_SIZE {
        Err(QueryError::InvalidPageSize)
    } else {
        Ok(())
    }
}

fn page<T>(records: &mut Vec<T>, limit: usize, coordinate: impl Fn(&T) -> u64) -> Page<T> {
    let has_more = records.len() > limit;
    records.truncate(limit);
    let next_before = has_more.then(|| records.last().map(&coordinate)).flatten();
    Page {
        items: std::mem::take(records),
        next_before,
    }
}
