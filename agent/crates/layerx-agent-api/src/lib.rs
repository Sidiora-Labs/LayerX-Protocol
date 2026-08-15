//! Stable contract shared by agent-facing servers and SDKs.

pub mod generated;

pub use generated::{
    agent_api_compat_gate, agent_api_schema_v1, Amount, BudgetLimit, ContractSchema,
    ContractVersion, Sequence, TimestampSeconds, VersionRequest, VersionResponse,
    AGENT_API_V1_SOURCE,
};
