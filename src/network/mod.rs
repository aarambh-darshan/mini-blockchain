//! P2P Networking module
//!
//! Provides peer-to-peer networking for distributed blockchain consensus.
//!
//! # Features
//! - TCP-based peer connections
//! - Block and transaction gossip
//! - Chain synchronization
//! - Peer discovery

pub mod message;
pub mod node;
pub mod peer;
pub mod server;
pub mod sync;

pub use message::{Handshake, Message, PROTOCOL_VERSION};
pub use node::{Node, NodeConfig, NodeStatus};
pub use peer::{PeerError, PeerHandle, PeerInfo, PeerManager, PeerState, MAX_PEERS};
pub use server::{connect_to_peer, Server};
pub use sync::ChainSync;
