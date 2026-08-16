//! Domain types used by human-plane intents before canonical compilation.

use crate::limits::IDENTIFIER_BYTES;

macro_rules! exact_identifier {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; IDENTIFIER_BYTES]);

        impl $name {
            #[must_use]
            pub const fn new(bytes: [u8; IDENTIFIER_BYTES]) -> Self {
                Self(bytes)
            }

            #[must_use]
            pub const fn bytes(self) -> [u8; IDENTIFIER_BYTES] {
                self.0
            }

            #[must_use]
            pub fn is_zero(self) -> bool {
                self.0.iter().all(|byte| *byte == 0)
            }
        }
    };
}

exact_identifier!(PublicKey, "A protocol public-key identifier.");
exact_identifier!(RecoveryRoot, "A recovery-policy commitment.");
exact_identifier!(PayerGrantId, "A payer-grant identifier.");
exact_identifier!(BudgetId, "A protocol budget identifier.");
exact_identifier!(DepositProofId, "An external deposit-proof identifier.");
exact_identifier!(WithdrawalId, "A bridge withdrawal identifier.");
exact_identifier!(PurposeHash, "A protocol purpose commitment.");
exact_identifier!(ContextHash, "A transaction context commitment.");

/// Exact 20-byte EVM payout address.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EvmAddress([u8; 20]);

impl EvmAddress {
    #[must_use]
    pub const fn new(bytes: [u8; 20]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn bytes(self) -> [u8; 20] {
        self.0
    }
}

/// Protocol account sequence with no signed or floating-point representation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Sequence(u64);

impl Sequence {
    #[must_use]
    pub const fn from_u64(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Protocol timestamp expressed as exact whole seconds.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TimestampSeconds(u64);

impl TimestampSeconds {
    #[must_use]
    pub const fn from_u64(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Non-zero protocol network identifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NetworkId(u32);

impl NetworkId {
    /// Constructs a network identifier.
    ///
    /// # Errors
    ///
    /// Refuses the reserved zero network.
    pub const fn new(value: u32) -> Result<Self, IntentDomainError> {
        if value == 0 {
            Err(IntentDomainError::Zero("network_id"))
        } else {
            Ok(Self(value))
        }
    }

    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }
}

/// Non-zero recovery approval threshold.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ApprovalThreshold(u16);

impl ApprovalThreshold {
    /// Constructs a recovery threshold.
    ///
    /// # Errors
    ///
    /// Refuses zero approvals.
    pub const fn new(value: u16) -> Result<Self, IntentDomainError> {
        if value == 0 {
            Err(IntentDomainError::Zero("approval_threshold"))
        } else {
            Ok(Self(value))
        }
    }

    #[must_use]
    pub const fn value(self) -> u16 {
        self.0
    }
}

/// Non-zero recurring or budget period measured in exact whole seconds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PeriodLength(u64);

impl PeriodLength {
    /// Constructs a period length.
    ///
    /// # Errors
    ///
    /// Refuses a zero-length period.
    pub const fn new(value: u64) -> Result<Self, IntentDomainError> {
        if value == 0 {
            Err(IntentDomainError::Zero("period_length"))
        } else {
            Ok(Self(value))
        }
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Payer-grant draw schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrantSchedule {
    SingleUse,
    Recurring(PeriodLength),
}

/// Protocol budget rollover policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RolloverPolicy {
    None,
    Capped,
}

/// Construction failure for an intent-domain scalar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntentDomainError {
    Zero(&'static str),
}
