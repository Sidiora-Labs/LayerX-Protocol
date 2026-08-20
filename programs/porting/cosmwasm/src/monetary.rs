//! `CosmWasm` value flow translated onto the 402LXP monetary law.
//!
//! A `CosmWasm` contract has a bank account. Funds arrive attached to a message
//! as `info.funds`, sit in the contract's balance, and leave later as a
//! `BankMsg` the chain dispatches after the handler returns.
//!
//! A `LayerX` program has no account at all. It holds no balance and mutates
//! none; it can only request an authenticated 402LXP transfer that debits the
//! invoking principal, and the kernel applies the whole requested set
//! atomically or none of it.
//!
//! So funds a caller attaches and the handler forwards in the same invocation
//! carry over exactly, and a payout from an accumulated balance does not. The
//! latter is refused by name here rather than emulated with a shadow ledger,
//! because a shadow ledger would be a second, unauthenticated money supply.
//! A denom does not carry over either: a program is paid in an authenticated
//! 402LXP asset, and a port names which asset stands in for the denom.

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

/// How a `CosmWasm` handler moves value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueFlow {
    /// Funds the caller attached as `info.funds` and the handler forwards to a
    /// named recipient in the same invocation.
    SentFunds {
        /// The credited account.
        recipient: [u8; 32],
        /// The exact amount the caller pays.
        amount: u128,
    },
    /// `BankMsg::Send` paid out of the contract's own balance.
    BankSend {
        /// The credited account.
        recipient: [u8; 32],
        /// The amount the contract would have paid from its balance.
        amount: u128,
    },
    /// `BankMsg::Burn`, which destroys balance the contract holds.
    BankBurn {
        /// The amount the burn would destroy.
        amount: u128,
    },
    /// `WasmMsg::Execute { funds, .. }` attaching funds to a sub-message out
    /// of the contract's own balance.
    SubMessageFunds {
        /// The callee the sub-message would pay.
        recipient: [u8; 32],
        /// The amount the sub-message would attach.
        amount: u128,
    },
    /// A `cw20` `TransferFrom` drawn against an allowance the contract stored
    /// for a third party.
    AllowanceSpend {
        /// The account whose balance the allowance would debit.
        owner: [u8; 32],
        /// The credited account.
        recipient: [u8; 32],
        /// The amount the allowance would move.
        amount: u128,
    },
    /// `IbcMsg::Transfer`, which moves value to another chain.
    IbcTransfer {
        /// The account on the far chain the transfer would credit.
        recipient: [u8; 32],
        /// The amount the transfer would move.
        amount: u128,
    },
}

impl ValueFlow {
    /// Translates one `CosmWasm` value flow into a 402LXP transfer leg paid by
    /// the invoking principal.
    ///
    /// # Errors
    ///
    /// Refuses [`PortRefusal::ContractHeldBalance`] for any payout, burn or
    /// sub-message funded from a balance the contract accumulated, because no
    /// program holds balance; [`PortRefusal::DelegatedSpend`] for an allowance
    /// drawn on an account other than the invoking principal, because
    /// delegated spending on `LayerX` is an explicit capability the payer
    /// grants at invocation and never state a contract stores about a third
    /// party; and [`PortRefusal::ChainQuery`] for an inter-chain transfer,
    /// which has no equivalent inside a deterministic execution.
    pub fn translate(
        &self,
        asset: [u8; 32],
        principal: [u8; 32],
    ) -> Result<Transfer402Plan, PortRefusal> {
        match self {
            Self::SentFunds { recipient, amount } => {
                Transfer402Plan::new(asset, *recipient, *amount)
            }
            Self::BankSend { .. } | Self::BankBurn { .. } | Self::SubMessageFunds { .. } => {
                Err(PortRefusal::ContractHeldBalance)
            }
            Self::AllowanceSpend {
                owner,
                recipient,
                amount,
            } => {
                if owner == &principal {
                    Transfer402Plan::new(asset, *recipient, *amount)
                } else {
                    Err(PortRefusal::DelegatedSpend)
                }
            }
            Self::IbcTransfer { .. } => Err(PortRefusal::ChainQuery),
        }
    }

    /// Returns whether the flow can be carried over at all, without building
    /// the leg. A porting tool uses this to report every unportable statement
    /// in one pass instead of stopping at the first.
    #[must_use]
    pub const fn portable(&self) -> bool {
        matches!(self, Self::SentFunds { .. } | Self::AllowanceSpend { .. })
    }
}

/// Translates a whole handler's value flow, keeping declaration order so the
/// emitted transfer set matches the order the `Response` carries messages in.
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
