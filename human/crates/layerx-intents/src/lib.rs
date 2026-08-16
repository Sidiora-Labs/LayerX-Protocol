#![forbid(unsafe_code)]

#[path = "compile.rs"]
mod compiler;
mod vocabulary;

pub use compiler::{compile, CompileError, CompileErrorReason, CompileField, CompiledIntent};

pub use vocabulary::{
    BridgeDepositCredit, BridgeWithdrawRequest, BudgetCreate, BudgetDefund, BudgetFund,
    DidRegistration, EvmPayoutBinding, Intent, IntentError, IntentErrorReason, IntentField,
    IntentKind, IntentVersion, KeyRotation, LxpReceive, LxpSend, PayerGrantRegistration,
    RecoveryRegistration,
};

/// Stable identity of the sole human-plane payload authority.
pub const CRATE_IDENTITY: &str = "layerx-intents";
