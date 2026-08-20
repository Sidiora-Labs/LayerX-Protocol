//! Durable, receipt-gated human journeys and their deterministic routing.

mod deposit;
mod engine;
mod resolver;

pub use deposit::{
    DepositActivity, DepositAgentBoundary, DepositAgentPlan, DepositBoundaryError,
    DepositFailureKind, DepositJourney, DepositJourneyError, DepositNotification, DepositPlan,
    DepositRuntime, DepositStage, DepositStatus, FinalityDelay, WalletCustodyOutcome,
    WalletCustodyRequest,
};

pub use engine::{
    AgentBoundary, AgentBoundaryError, AgentObservation, AgentPreparation, JourneyEngine,
    JourneyError, JourneyLeg, JourneyPhase, JourneyPlan, JourneyProgress, JourneyState,
    JourneyStatus, ReceiptLookup, ReceiptMaterial, VerifiedLegEvidence,
};

pub use resolver::{
    BudgetCreation, BudgetRoute, ChangeSurface, CustodyRoute, Endpoint, EndpointKind, LimitRefusal,
    LimitRefusalError, LimitSource, Mechanism, MovementTerm, PayerGrantRoute, Relationship, Route,
    RouteError, RouteLeg, RouteRequest, RouteResolver, SendRoute,
};
