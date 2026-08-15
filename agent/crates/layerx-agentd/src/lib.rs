//! Non-authoritative `LayerX` agent daemon.

pub mod audit;
pub mod authority;
pub mod budget;
pub mod cache;
pub mod capability;
pub mod config;
pub mod events;
pub mod export;
pub mod finality;
pub mod idempotency;
pub mod identity;
pub mod limits;
pub mod obs;
pub mod outbox;
pub mod policy;
pub mod prepare;
pub mod read;
pub mod receipt;
pub mod session;
pub mod shutdown;
pub mod sign;
pub mod store;
pub mod tenant;
