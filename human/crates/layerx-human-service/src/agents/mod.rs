//! Managed-agent lifecycle orchestration.

mod controls;
mod create;
mod reclaim;

pub use controls::{
    AgentControlAgent, AgentControlContract, AgentControlError, AgentControlProfile,
    AgentControlState, AgentControlView, AgentControls, AppLimitEvidence, AppLimitRequest,
    LimitBacking, LimitChangeRequest, LimitEnforcement, PresentedLimit, SessionControlAdapter,
    APP_LIMIT_COPY_KEY, APP_LIMIT_EXPLANATION, PAUSE_CONSEQUENCE, PAUSE_CONSEQUENCE_COPY_KEY,
    PROTOCOL_LIMIT_COPY_KEY,
};
pub use create::{
    AgentCreationContract, AgentCreationError, AgentEvidence, AgentFailure, CapabilityProvision,
    CreateAgentRequest, CreationContext, CreationJourney, CreationStage, CreationState,
    CreationStatus, ProtocolAction, ProtocolEvidence, PurposePresetCatalog, SessionProvision,
    StageState,
};
pub use reclaim::{
    Reclaim, ReclaimAgentBoundary, ReclaimAgentContext, ReclaimError, ReclaimMechanism,
    ReclaimRequest, ReclaimResult, ReclaimStage, ReclaimStatus,
};
