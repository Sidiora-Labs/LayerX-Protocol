#![forbid(unsafe_code)]

mod client;
mod finality;
mod json;
mod rpc;
mod status;

pub use client::{
    BlockRef, ClientConfigError, EndpointError, ExecutionOutcome, PaxeerClient, TransactionHash,
    TransactionHashError, TransactionInclusion, TransactionView,
};
pub use finality::{
    ChainSignal, ConfirmationProgress, EndpointSignal, FinalityReport, FinalityStage,
    FinalityTracker, TrackerConfig, TrackerConfigError,
};
pub use json::{parse as parse_json, Json, JsonError, JsonErrorReason};
pub use rpc::{raw_call, EndpointConfig, EndpointFailure, EndpointFault};
pub use status::{
    BoundaryHealth, BoundaryStatus, ChainStatus, ContractStatus, DelayExpectation, EndpointStatus,
};

/// Stable identity of the Paxeer custody-boundary client.
pub const CRATE_IDENTITY: &str = "layerx-paxeer-client";
