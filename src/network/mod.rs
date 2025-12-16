//! P2P Networking module
//!
//! Provides peer-to-peer networking for distributed blockchain consensus.
//!
//! # Features
//! - TCP-based peer connections
//! - Block and transaction gossip
//! - Chain synchronization
//! - Peer discovery
//! - Protocol versioning
//! - Peer scoring and banning
//! - Rate limiting (DOS protection)

pub mod message;
pub mod node;
pub mod peer;
pub mod server;
pub mod sync;

pub use message::{
    BlockHeader as NetworkBlockHeader, CompactBlock, Handshake, InvItem, InvType, Message,
    RejectCode, RejectMessage, ServiceFlags, VersionMessage, MAGIC, MAX_MESSAGE_SIZE,
    MIN_PROTOCOL_VERSION, PROTOCOL_VERSION,
};
pub use node::{Node, NodeConfig, NodeStatus};
pub use peer::{
    BanEntry, Misbehavior, PeerError, PeerHandle, PeerInfo, PeerManager, PeerManagerStats,
    PeerState, RateLimitStats, RateLimiter, BAN_SCORE, DEFAULT_BAN_DURATION, DEFAULT_PEER_SCORE,
    DISCONNECT_SCORE, MAX_INBOUND, MAX_OUTBOUND, MAX_PEERS,
};
pub use server::{connect_to_peer, Server};
pub use sync::ChainSync;
