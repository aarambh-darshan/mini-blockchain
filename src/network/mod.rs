//! P2P Networking module
//!
//! Provides peer-to-peer networking for distributed blockchain consensus.
//!
//! # Features
//! - TCP-based peer connections with SHA-256 checksums
//! - Block and transaction gossip
//! - Chain synchronization (parallel download supported)
//! - Peer discovery (DNS seeds, Addr/GetAddr)
//! - Protocol versioning
//! - Peer scoring and banning
//! - Rate limiting (DOS protection)
//! - Address manager (new/tried tables)
//! - NAT traversal (UPnP)

pub mod addrman;
pub mod discovery;
pub mod message;
pub mod node;
pub mod parallel_sync;
pub mod peer;
pub mod server;
pub mod sync;
pub mod upnp;

pub use addrman::{AddrEntry, AddrManager};
pub use discovery::{DiscoveryStats, PeerDiscovery, DEFAULT_DNS_SEEDS};
pub use message::{
    BlockHeader as NetworkBlockHeader, CompactBlock, Handshake, InvItem, InvType, Message, NetAddr,
    RejectCode, RejectMessage, ServiceFlags, VersionMessage, HEADER_SIZE, MAGIC,
    MAX_ADDR_PER_MESSAGE, MAX_MESSAGE_SIZE, MIN_PROTOCOL_VERSION, PROTOCOL_VERSION,
};
pub use node::{Node, NodeConfig, NodeStatus};
pub use parallel_sync::{ParallelSync, ParallelSyncStats, SyncError};
pub use peer::{
    BanEntry, Misbehavior, PeerError, PeerHandle, PeerInfo, PeerManager, PeerManagerStats,
    PeerState, RateLimitStats, RateLimiter, BAN_SCORE, DEFAULT_BAN_DURATION, DEFAULT_PEER_SCORE,
    DISCONNECT_SCORE, MAX_INBOUND, MAX_OUTBOUND, MAX_PEERS,
};
pub use server::{connect_to_peer, MessageCodec, Server};
pub use sync::ChainSync;
pub use upnp::{UpnpError, UpnpManager, UpnpStatus};
