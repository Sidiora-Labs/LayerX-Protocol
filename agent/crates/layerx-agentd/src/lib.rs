//! Non-authoritative `LayerX` agent daemon.

pub mod authority;
pub mod audit;
pub mod budget;
pub mod cache;
pub mod capability;
pub mod events;
pub mod export;
pub mod finality;
pub mod idempotency;
pub mod identity;
pub mod limits;
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
