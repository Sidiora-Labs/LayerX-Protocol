//! The 402LXP transfer binding.
//!
//! A program cannot write a balance. It can only request an authenticated
//! transfer the kernel applies atomically after the whole execution succeeds,
//! and only inside the ceiling its capability grant fixed. Amounts are exact
//! protocol integers, refused at construction when zero.

use crate::amount::Amount;
use crate::error::{Field, ProgramError, Reason};
use crate::ids::{AccountId, AssetId};

#[cfg(target_arch = "wasm32")]
use crate::host;

/// One authenticated 402LXP transfer the kernel will apply.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Payment {
    asset: AssetId,
    to: AccountId,
    amount: Amount,
}

impl Payment {
    /// Builds a payment of an exact nonzero integer amount.
    ///
    /// # Errors
    ///
    /// Refuses the zero amount the runtime's monetary law rejects.
    pub const fn new(asset: AssetId, to: AccountId, amount: Amount) -> Result<Self, ProgramError> {
        if amount.is_zero() {
            return Err(ProgramError::value(Field::Amount, Reason::Zero));
        }
        Ok(Self { asset, to, amount })
    }

    /// Returns the asset this payment moves.
    #[must_use]
    pub const fn asset(self) -> AssetId {
        self.asset
    }

    /// Returns the account this payment credits.
    #[must_use]
    pub const fn to(self) -> AccountId {
        self.to
    }

    /// Returns the exact integer amount.
    #[must_use]
    pub const fn amount(self) -> Amount {
        self.amount
    }
}

/// Requests one authenticated 402LXP transfer.
///
/// # Errors
///
/// Refuses missing transfer authority, an amount above the granted ceiling,
/// and every meter refusal.
#[cfg(target_arch = "wasm32")]
pub fn pay(payment: Payment) -> Result<(), ProgramError> {
    let (amount_high, amount_low) = payment.amount().split();
    let asset = payment.asset().bytes();
    let recipient = payment.to().bytes();
    host::transfer_402(amount_high, amount_low, &asset, &recipient)?;
    Ok(())
}
