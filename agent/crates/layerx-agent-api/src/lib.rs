//! Stable contract shared by agent-facing servers and SDKs.

pub mod generated;
#[path = "identity.rs"]
mod identity_contract;

/// Identity and session contract namespace.
pub mod identity {
    pub use crate::identity_contract::*;
}

/// Capability contract namespace.
pub mod capability {
    pub use crate::identity_contract::{
        AmountCeiling, CapabilityAttenuate, CapabilityCreate, CapabilityDimensions,
        CapabilityId, CapabilityList, CapabilityRevoke, ExplicitSet, RateCeiling,
    };
}

/// Budget contract namespace.
pub mod budget {
    pub use crate::identity_contract::{
        BudgetCreate, BudgetEnforcement, BudgetFund, BudgetId, BudgetList, BudgetTarget,
    };
}

pub use generated::{
    agent_api_compat_gate, agent_api_schema_v1, Amount, BudgetLimit, ContractSchema,
    ContractVersion, Sequence, TimestampSeconds, VersionRequest, VersionResponse,
    AGENT_API_V1_SOURCE,
};
