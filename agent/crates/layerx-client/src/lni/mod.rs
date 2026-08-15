//! The sole versioned protocol boundary between clients and the `LayerX` core.

pub mod capabilities;
pub mod framing;
pub mod handshake;
pub mod schema;
pub mod transport;

pub use capabilities::Capabilities;
pub use handshake::NodeInfo;
