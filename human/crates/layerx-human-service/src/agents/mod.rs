//! Managed-agent lifecycle orchestration.

mod archive;
mod controls;
mod create;
mod reclaim;

pub use archive::{
    AgentBalance, ArchiveAgentContract, ArchiveBoundary, ArchiveError, ArchiveJourney,
    ArchiveRequest, ArchiveStage, ArchiveStatus, ArchivedHistoryEntry, FundsDispositionEvidence,
    SessionArchiveAdapter, ARCHIVE_ACTION_LABEL, ARCHIVE_CONFIRMATION_TONE,
    ARCHIVE_IRREVERSIBILITY_NOTICE,
};

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
