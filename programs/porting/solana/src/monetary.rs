//! Solana lamport and token flow translated onto the 402LXP monetary law.
//!
//! On Solana every account holds lamports, and a program that owns an account
//! may debit it by writing the balance field directly. A program-derived
//! address gives a program its own signing authority, so a program can also
//! pay out of an account it controls with `invoke_signed`.
//!
//! `LayerX` gives a program neither. A program holds no balance and writes no
//! balance; it can only request an authenticated 402LXP transfer that debits
//! the invoking principal, and the kernel applies the whole requested set
//! atomically or none of it.
//!
//! So a `system_program::transfer` signed by the payer carries over exactly,
//! and a lamport write, an `invoke_signed` payout and a `close = recipient`
//! rent sweep do not. Each of those is refused by name here rather than
//! emulated with a shadow balance, because a shadow balance would be a second,
//! unauthenticated money supply.

use layerx_programs_runtime::Capability;

use crate::error::PortRefusal;

/// One translated 402LXP transfer request: the asset, the recipient account
/// and the exact amount the invoking principal pays.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Transfer402Plan {
    asset: [u8; 32],
    to: [u8; 32],
    amount: u128,
}

impl Transfer402Plan {
    /// Builds one transfer leg.
    ///
    /// # Errors
    ///
    /// Refuses the reserved zero asset, the reserved zero recipient and a zero
    /// amount, all of which the capability ABI rejects as invalid grants.
    pub fn new(asset: [u8; 32], to: [u8; 32], amount: u128) -> Result<Self, PortRefusal> {
        if asset == [0u8; 32] || to == [0u8; 32] || amount == 0 {
            return Err(PortRefusal::OutOfRange);
        }
        Ok(Self { asset, to, amount })
    }

    /// Returns the asset the leg moves.
    #[must_use]
    pub const fn asset(&self) -> [u8; 32] {
        self.asset
    }

    /// Returns the account credited by the leg.
    #[must_use]
    pub const fn to(&self) -> [u8; 32] {
        self.to
    }

    /// Returns the exact amount debited from the invoking principal.
    #[must_use]
    pub const fn amount(&self) -> u128 {
        self.amount
    }

    /// Returns the grant an activity must carry for the leg to be admitted.
    #[must_use]
    pub const fn capability(&self) -> Capability {
        Capability::Transfer402 {
            asset: self.asset,
            to: self.to,
            maximum_amount: self.amount,
        }
    }

    /// Returns the amount split into the two `i64` halves the `transfer_402`
    /// host function takes, high limb first. The ABI is integer-only and has
    /// no 128-bit value type, so the amount crosses the boundary as two limbs
    /// the host reassembles as `high << 64 | low`.
    #[must_use]
    pub fn amount_limbs(&self) -> (i64, i64) {
        let high = u64::try_from(self.amount >> 64).unwrap_or(u64::MAX);
        let low = u64::try_from(self.amount & u128::from(u64::MAX)).unwrap_or(u64::MAX);
        (
            i64::from_be_bytes(high.to_be_bytes()),
            i64::from_be_bytes(low.to_be_bytes()),
        )
    }
}

/// How a Solana instruction moves value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueFlow {
    /// `system_program::transfer` whose `from` account is a `Signer` on the
    /// instruction, which is the pattern every price guard uses.
    SignerFunded {
        /// The account the signer's lamports are credited to.
        recipient: [u8; 32],
        /// The exact amount the signer pays.
        amount: u128,
    },
    /// A direct lamport write on a program-owned account, as in
    /// `**account.try_borrow_mut_lamports()? -= amount`.
    LamportWrite {
        /// The account the write would credit.
        recipient: [u8; 32],
        /// The amount the write would move.
        amount: u128,
    },
    /// An `invoke_signed` transfer whose source is a program-derived address,
    /// paying out of a balance the program itself controls.
    ProgramAuthorityFunded {
        /// The program-derived address that would sign the payout.
        authority: [u8; 32],
        /// The account the payout would credit.
        recipient: [u8; 32],
        /// The amount the payout would move.
        amount: u128,
    },
    /// An SPL token `transfer` whose `authority` account signs the debit.
    TokenTransfer {
        /// The account that signs the debit.
        authority: [u8; 32],
        /// The token account credited by the transfer.
        recipient: [u8; 32],
        /// The amount the transfer moves.
        amount: u128,
    },
    /// A `#[account(mut, close = recipient)]` constraint sweeping an account's
    /// rent-exempt lamports on close.
    RentSweep {
        /// The account the sweep would credit.
        recipient: [u8; 32],
    },
}

impl ValueFlow {
    /// Translates one Solana value flow into a 402LXP transfer leg paid by the
    /// invoking principal.
    ///
    /// # Errors
    ///
    /// Refuses [`PortRefusal::LamportMutation`] for a direct balance write, an
    /// `invoke_signed` payout and a rent sweep, because no program holds or
    /// writes balance, and [`PortRefusal::DelegatedSpend`] for a token debit
    /// authorised by anyone other than the invoking principal, because
    /// delegated spending on `LayerX` is an explicit capability the payer
    /// grants at invocation and never an authority a program holds over
    /// somebody else's account.
    pub fn translate(
        &self,
        asset: [u8; 32],
        principal: [u8; 32],
    ) -> Result<Transfer402Plan, PortRefusal> {
        match self {
            Self::SignerFunded { recipient, amount } => {
                Transfer402Plan::new(asset, *recipient, *amount)
            }
            Self::LamportWrite { .. }
            | Self::ProgramAuthorityFunded { .. }
            | Self::RentSweep { .. } => Err(PortRefusal::LamportMutation),
            Self::TokenTransfer {
                authority,
                recipient,
                amount,
            } => {
                if authority == &principal {
                    Transfer402Plan::new(asset, *recipient, *amount)
                } else {
                    Err(PortRefusal::DelegatedSpend)
                }
            }
        }
    }

    /// Returns whether the flow can be carried over at all, without building
    /// the leg. A porting tool uses this to report every unportable statement
    /// in one pass instead of stopping at the first.
    #[must_use]
    pub const fn portable(&self) -> bool {
        matches!(self, Self::SignerFunded { .. } | Self::TokenTransfer { .. })
    }
}

/// Translates a whole instruction handler's value flow, keeping declaration
/// order so the emitted transfer set matches the order the Anchor source pays
/// in.
///
/// # Errors
///
/// Returns the first refusal encountered, naming the construct that cannot be
/// carried over.
pub fn translate_all(
    flows: &[ValueFlow],
    asset: [u8; 32],
    principal: [u8; 32],
) -> Result<Vec<Transfer402Plan>, PortRefusal> {
    let mut plans = Vec::with_capacity(flows.len());
    for flow in flows {
        plans.push(flow.translate(asset, principal)?);
    }
    Ok(plans)
}
