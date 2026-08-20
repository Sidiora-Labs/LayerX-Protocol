//! Durable, receipt-gated human journeys and their deterministic routing.

mod engine;
mod resolver;

pub use engine::{
    AgentBoundary, AgentBoundaryError, AgentObservation, AgentPreparation, JourneyEngine,
    JourneyError, JourneyLeg, JourneyPhase, JourneyPlan, JourneyProgress, JourneyState,
    JourneyStatus, ReceiptLookup, ReceiptMaterial,
};

pub use resolver::{
    BudgetCreation, BudgetRoute, ChangeSurface, CustodyRoute, Endpoint, EndpointKind, LimitRefusal,
    LimitRefusalError, LimitSource, Mechanism, MovementTerm, PayerGrantRoute, Relationship, Route,
    RouteError, RouteLeg, RouteRequest, RouteResolver, SendRoute,
};
