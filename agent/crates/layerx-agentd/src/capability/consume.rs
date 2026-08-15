//! Serialised capability-ceiling reservations and receipt-only consumption.

use std::collections::BTreeMap;
use std::sync::Mutex;

/// One held amount awaiting a verified terminal receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reservation {
    pub id: [u8; 32],
    pub amount: u128,
    pub expiry_sequence: u64,
    pub unknown: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct State {
    consumed: u128,
    reservations: BTreeMap<[u8; 32], Reservation>,
    reconciled: bool,
}

/// Thread-safe capability ceiling.
#[derive(Debug)]
pub struct Ceiling {
    maximum: u128,
    state: Mutex<State>,
}

impl Ceiling {
    #[must_use]
    pub fn new(maximum: u128) -> Self {
        Self {
            maximum,
            state: Mutex::new(State {
                consumed: 0,
                reservations: BTreeMap::new(),
                reconciled: true,
            }),
        }
    }

    /// Rebuilds consumed value from persisted verified receipts only.
    pub fn rebuild(maximum: u128, receipts: &[VerifiedReceipt]) -> Result<Self, CeilingError> {
        let mut consumed = 0_u128;
        for receipt in receipts {
            if !receipt.verified {
                return Err(CeilingError::UnverifiedReceipt);
            }
            if let ReceiptOutcome::Executed(amount) = receipt.outcome {
                consumed = consumed.checked_add(amount).ok_or(CeilingError::Overflow)?;
            }
        }
        if consumed > maximum {
            return Err(CeilingError::Exceeded);
        }
        Ok(Self {
            maximum,
            state: Mutex::new(State {
                consumed,
                reservations: BTreeMap::new(),
                reconciled: true,
            }),
        })
    }

    /// Applies only a verified terminal receipt; failure consumes nothing.
    pub fn apply_receipt(&self, receipt: &VerifiedReceipt) -> Result<(), CeilingError> {
        if !receipt.verified {
            return Err(CeilingError::UnverifiedReceipt);
        }
        let mut state = self.state.lock().map_err(|_| CeilingError::Poisoned)?;
        let reservation = state
            .reservations
            .remove(&receipt.reservation_id)
            .ok_or(CeilingError::MissingReservation)?;
        match receipt.outcome {
            ReceiptOutcome::Executed(amount) => {
                if amount != reservation.amount {
                    state.reservations.insert(reservation.id, reservation);
                    return Err(CeilingError::AmountMismatch);
                }
                state.consumed = state
                    .consumed
                    .checked_add(amount)
                    .ok_or(CeilingError::Overflow)?;
            }
            ReceiptOutcome::Failed => {}
        }
        Ok(())
    }

    /// Marks an indeterminate outcome; it remains held across expiry.
    pub fn mark_unknown(&self, id: [u8; 32]) -> Result<(), CeilingError> {
        let mut state = self.state.lock().map_err(|_| CeilingError::Poisoned)?;
        let reservation = state
            .reservations
            .get_mut(&id)
            .ok_or(CeilingError::MissingReservation)?;
        reservation.unknown = true;
        Ok(())
    }

    /// Releases only expired reservations whose outcome is not unknown.
    pub fn release_expired(&self, current_sequence: u64) -> Result<usize, CeilingError> {
        let mut state = self.state.lock().map_err(|_| CeilingError::Poisoned)?;
        let before = state.reservations.len();
        state
            .reservations
            .retain(|_, value| value.unknown || current_sequence < value.expiry_sequence);
        Ok(before - state.reservations.len())
    }

    pub fn snapshot(&self) -> Result<CeilingSnapshot, CeilingError> {
        let state = self.state.lock().map_err(|_| CeilingError::Poisoned)?;
        let held = state
            .reservations
            .values()
            .try_fold(0_u128, |total, value| total.checked_add(value.amount))
            .ok_or(CeilingError::Overflow)?;
        Ok(CeilingSnapshot {
            maximum: self.maximum,
            consumed: state.consumed,
            held,
            reservations: state.reservations.len(),
            reconciled: state.reconciled,
        })
    }
}

/// Persisted receipt outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiptOutcome {
    Executed(u128),
    Failed,
}

/// Receipt record with independent verification status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedReceipt {
    pub reservation_id: [u8; 32],
    pub outcome: ReceiptOutcome,
    pub verified: bool,
}

/// Atomic ceiling snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CeilingSnapshot {
    pub maximum: u128,
    pub consumed: u128,
    pub held: u128,
    pub reservations: usize,
    pub reconciled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CeilingError {
    ZeroAmount,
    Expired,
    Duplicate,
    Exceeded,
    Unreconciled,
    UnverifiedReceipt,
    MissingReservation,
    AmountMismatch,
    Overflow,
    Poisoned,
}

pub(crate) fn reserve(
    ceiling: &Ceiling,
    reservation_id: [u8; 32],
    amount: u128,
    expiry_sequence: u64,
    current_sequence: u64,
) -> Result<Reservation, CeilingError> {
    if amount == 0 {
        return Err(CeilingError::ZeroAmount);
    }
    if expiry_sequence <= current_sequence {
        return Err(CeilingError::Expired);
    }
    let mut state = ceiling.state.lock().map_err(|_| CeilingError::Poisoned)?;
    if !state.reconciled {
        return Err(CeilingError::Unreconciled);
    }
    if state.reservations.contains_key(&reservation_id) {
        return Err(CeilingError::Duplicate);
    }
    let held = state
        .reservations
        .values()
        .try_fold(0_u128, |total, value| total.checked_add(value.amount))
        .ok_or(CeilingError::Overflow)?;
    let projected = state
        .consumed
        .checked_add(held)
        .and_then(|value| value.checked_add(amount))
        .ok_or(CeilingError::Overflow)?;
    if projected > ceiling.maximum {
        return Err(CeilingError::Exceeded);
    }
    let reservation = Reservation {
        id: reservation_id,
        amount,
        expiry_sequence,
        unknown: false,
    };
    state.reservations.insert(reservation_id, reservation.clone());
    Ok(reservation)
}
