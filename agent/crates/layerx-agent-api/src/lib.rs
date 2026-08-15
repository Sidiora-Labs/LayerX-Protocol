//! Stable contract shared by agent-facing servers and SDKs.

pub mod generated;
#[path = "identity.rs"]
mod identity_contract;
#[path = "write.rs"]
mod write_contract;

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

/// Preparation contract namespace.
pub mod prepare {
    pub use crate::write_contract::{
        CanonicalBytes, Disclosure, IdempotencyRef, PayloadBytes, PrepareRequest, Prepared,
        PreparationRef, SigningPreimage, TimestampBound,
    };
}

/// Submission contract namespace.
pub mod submit {
    pub use crate::write_contract::{PreparationRef, SignRequest, SignatureBytes, SubmitRequest};
}

/// Tracking contract namespace.
pub mod track {
    pub use crate::write_contract::{
        EvidenceRef, ReceiptRef, SubmissionRef, SubmissionState, TrackRequest, TrackedSubmission,
        Transition, WaitRequest, WaitResult,
    };
}

pub use write_contract::SubmissionState;

pub use generated::{
    agent_api_compat_gate, agent_api_schema_v1, Amount, BudgetLimit, ContractSchema,
    ContractVersion, Sequence, TimestampSeconds, VersionRequest, VersionResponse,
    AGENT_API_V1_SOURCE,
};
