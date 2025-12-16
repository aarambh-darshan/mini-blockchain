//! Network message types for P2P communication
//!
//! Defines message types for the gossip protocol with:
//! - Protocol versioning
//! - Service flags (like Bitcoin)
//! - Version negotiation
//! - Reject messages for misbehavior

use crate::core::{Block, Transaction};
use serde::{Deserialize, Serialize};

// =============================================================================
// Protocol Constants
// =============================================================================

/// Current protocol version (bumped for new features)
pub const PROTOCOL_VERSION: u32 = 70001;

/// Minimum supported protocol version
pub const MIN_PROTOCOL_VERSION: u32 = 70000;

/// Magic bytes for message framing (network identification)
pub const MAGIC_MAINNET: [u8; 4] = [0x4D, 0x49, 0x4E, 0x49]; // "MINI"
pub const MAGIC_TESTNET: [u8; 4] = [0x54, 0x45, 0x53, 0x54]; // "TEST"

/// Default magic (mainnet)
pub const MAGIC: [u8; 4] = MAGIC_MAINNET;

/// Maximum message size (16 MB)
pub const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

/// Maximum blocks per GetBlocks request
pub const MAX_BLOCKS_PER_REQUEST: u32 = 500;

/// Maximum headers per GetHeaders request  
pub const MAX_HEADERS_PER_REQUEST: u32 = 2000;

// =============================================================================
// Service Flags (Bitcoin-style)
// =============================================================================

/// Service flags advertised by nodes (represented as u64 for serde compatibility)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ServiceFlags(pub u64);

impl ServiceFlags {
    /// Node can serve full blocks
    pub const NODE_NETWORK: ServiceFlags = ServiceFlags(1 << 0);
    /// Node supports bloom filters (BIP 37)
    pub const NODE_BLOOM: ServiceFlags = ServiceFlags(1 << 2);
    /// Node supports witness data (SegWit)
    pub const NODE_WITNESS: ServiceFlags = ServiceFlags(1 << 3);
    /// Node supports compact block relay (BIP 152)
    pub const NODE_COMPACT_FILTERS: ServiceFlags = ServiceFlags(1 << 6);
    /// Node serves historical blocks (not pruned)
    pub const NODE_NETWORK_LIMITED: ServiceFlags = ServiceFlags(1 << 10);
    /// Empty flags
    pub const NONE: ServiceFlags = ServiceFlags(0);

    /// Check if flag is set
    pub fn contains(&self, flag: ServiceFlags) -> bool {
        (self.0 & flag.0) == flag.0
    }

    /// Add a flag
    pub fn insert(&mut self, flag: ServiceFlags) {
        self.0 |= flag.0;
    }

    /// Remove a flag
    pub fn remove(&mut self, flag: ServiceFlags) {
        self.0 &= !flag.0;
    }
}

impl std::ops::BitOr for ServiceFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        ServiceFlags(self.0 | rhs.0)
    }
}

// =============================================================================
// Message Types
// =============================================================================

/// Network message types  
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Version handshake (first message)
    Version(VersionMessage),
    /// Version acknowledged
    VerAck,
    /// Initial handshake (legacy, kept for compatibility)
    Handshake(Handshake),

    /// Announce a newly mined block
    NewBlock(Block),
    /// Announce a new transaction
    NewTransaction(Transaction),

    /// Request blocks by height range
    GetBlocks { start_height: u64, count: u32 },
    /// Response with blocks
    Blocks(Vec<Block>),

    /// Request block headers only
    GetHeaders { start_height: u64, count: u32 },
    /// Response with headers
    Headers(Vec<BlockHeader>),

    /// Request list of known peers
    GetPeers,
    /// Response with peer addresses
    Peers(Vec<String>),

    /// Keep-alive ping
    Ping(u64),
    /// Keep-alive pong response  
    Pong(u64),

    /// Request current chain height
    GetHeight,
    /// Response with chain height
    Height(u64),

    /// Inventory announcement (have these items)
    Inv(Vec<InvItem>),
    /// Request data for inventory items
    GetData(Vec<InvItem>),
    /// Item not found
    NotFound(Vec<InvItem>),

    /// Transaction rejected
    Reject(RejectMessage),

    /// Compact block relay (BIP 152 style)
    CompactBlock(CompactBlock),
    /// Request missing transactions for compact block
    GetBlockTxn {
        block_hash: String,
        indexes: Vec<u32>,
    },
    /// Missing transactions for compact block
    BlockTxn {
        block_hash: String,
        transactions: Vec<Transaction>,
    },

    /// Fee filter (minimum fee to relay)
    FeeFilter(u64),

    /// Send compact blocks preference
    SendCmpct { enable: bool, version: u64 },
}

// =============================================================================
// Version Message (Bitcoin-style handshake)
// =============================================================================

/// Version message for protocol negotiation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionMessage {
    /// Protocol version
    pub version: u32,
    /// Services offered by this node
    pub services: ServiceFlags,
    /// Unix timestamp
    pub timestamp: i64,
    /// Recipient's address
    pub addr_recv: String,
    /// Sender's address
    pub addr_from: String,
    /// Random nonce for connection identification
    pub nonce: u64,
    /// User agent string
    pub user_agent: String,
    /// Best block height
    pub start_height: u64,
    /// Whether to relay transactions
    pub relay: bool,
}

impl VersionMessage {
    pub fn new(services: ServiceFlags, height: u64, addr_recv: String, addr_from: String) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            services,
            timestamp: chrono::Utc::now().timestamp(),
            addr_recv,
            addr_from,
            nonce: rand::random(),
            user_agent: format!("mini-blockchain/{}", env!("CARGO_PKG_VERSION")),
            start_height: height,
            relay: true,
        }
    }

    /// Check if version is compatible
    pub fn is_compatible(&self) -> bool {
        self.version >= MIN_PROTOCOL_VERSION
    }
}

// =============================================================================
// Legacy Handshake (for backward compatibility)
// =============================================================================

/// Legacy handshake message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Handshake {
    pub version: u32,
    pub height: u64,
    pub best_hash: String,
    pub listen_port: u16,
    pub user_agent: String,
}

impl Handshake {
    pub fn new(height: u64, best_hash: String, listen_port: u16) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            height,
            best_hash,
            listen_port,
            user_agent: format!("mini-blockchain/{}", env!("CARGO_PKG_VERSION")),
        }
    }
}

// =============================================================================
// Inventory System
// =============================================================================

/// Inventory item type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvType {
    Error = 0,
    Transaction = 1,
    Block = 2,
    CompactBlock = 4,
}

/// Inventory item (reference to tx or block)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvItem {
    pub inv_type: InvType,
    pub hash: String,
}

impl InvItem {
    pub fn transaction(hash: String) -> Self {
        Self {
            inv_type: InvType::Transaction,
            hash,
        }
    }

    pub fn block(hash: String) -> Self {
        Self {
            inv_type: InvType::Block,
            hash,
        }
    }
}

// =============================================================================
// Block Header (for headers-first sync)
// =============================================================================

/// Lightweight block header for sync
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    pub version: u32,
    pub previous_hash: String,
    pub merkle_root: String,
    pub timestamp: i64,
    pub difficulty: u32,
    pub nonce: u64,
    pub hash: String,
}

// =============================================================================
// Compact Blocks (BIP 152 style)
// =============================================================================

/// Compact block for efficient relay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactBlock {
    /// Block header
    pub header: BlockHeader,
    /// Nonce for short ID calculation
    pub nonce: u64,
    /// Short transaction IDs (first 6 bytes of txid)
    pub short_ids: Vec<u64>,
    /// Prefilled transactions (always includes coinbase)
    pub prefilled_txn: Vec<PrefilledTransaction>,
}

/// Prefilled transaction in compact block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefilledTransaction {
    pub index: u32,
    pub tx: Transaction,
}

// =============================================================================
// Reject Message
// =============================================================================

/// Reject codes (Bitcoin-style)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RejectCode {
    Malformed = 0x01,
    Invalid = 0x10,
    Obsolete = 0x11,
    Duplicate = 0x12,
    NonStandard = 0x40,
    Dust = 0x41,
    InsufficientFee = 0x42,
    Checkpoint = 0x43,
}

/// Reject message for misbehavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectMessage {
    /// Rejected message type
    pub message: String,
    /// Reject code
    pub code: RejectCode,
    /// Human-readable reason
    pub reason: String,
    /// Data hash (for tx/block rejects)
    pub data: Option<String>,
}

impl RejectMessage {
    pub fn new(message: &str, code: RejectCode, reason: &str, data: Option<String>) -> Self {
        Self {
            message: message.to_string(),
            code,
            reason: reason.to_string(),
            data,
        }
    }
}

// =============================================================================
// Message Implementation
// =============================================================================

impl Message {
    /// Serialize message to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Deserialize message from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(data)
    }

    /// Get message type name for logging
    pub fn type_name(&self) -> &'static str {
        match self {
            Message::Version(_) => "Version",
            Message::VerAck => "VerAck",
            Message::Handshake(_) => "Handshake",
            Message::NewBlock(_) => "NewBlock",
            Message::NewTransaction(_) => "NewTransaction",
            Message::GetBlocks { .. } => "GetBlocks",
            Message::Blocks(_) => "Blocks",
            Message::GetHeaders { .. } => "GetHeaders",
            Message::Headers(_) => "Headers",
            Message::GetPeers => "GetPeers",
            Message::Peers(_) => "Peers",
            Message::Ping(_) => "Ping",
            Message::Pong(_) => "Pong",
            Message::GetHeight => "GetHeight",
            Message::Height(_) => "Height",
            Message::Inv(_) => "Inv",
            Message::GetData(_) => "GetData",
            Message::NotFound(_) => "NotFound",
            Message::Reject(_) => "Reject",
            Message::CompactBlock(_) => "CompactBlock",
            Message::GetBlockTxn { .. } => "GetBlockTxn",
            Message::BlockTxn { .. } => "BlockTxn",
            Message::FeeFilter(_) => "FeeFilter",
            Message::SendCmpct { .. } => "SendCmpct",
        }
    }

    /// Check if this is a high-bandwidth message
    pub fn is_high_bandwidth(&self) -> bool {
        matches!(
            self,
            Message::NewBlock(_) | Message::Blocks(_) | Message::CompactBlock(_)
        )
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        let msg = Message::Ping(12345);
        let bytes = msg.to_bytes().unwrap();
        let decoded = Message::from_bytes(&bytes).unwrap();

        if let Message::Ping(nonce) = decoded {
            assert_eq!(nonce, 12345);
        } else {
            panic!("Wrong message type");
        }
    }

    #[test]
    fn test_handshake() {
        let handshake = Handshake::new(100, "abc123".to_string(), 8333);
        assert_eq!(handshake.version, PROTOCOL_VERSION);
        assert_eq!(handshake.height, 100);
    }

    #[test]
    fn test_version_message() {
        let version = VersionMessage::new(
            ServiceFlags::NODE_NETWORK,
            100,
            "127.0.0.1:8333".to_string(),
            "127.0.0.1:8334".to_string(),
        );
        assert!(version.is_compatible());
        assert_eq!(version.version, PROTOCOL_VERSION);
    }

    #[test]
    fn test_service_flags() {
        let flags = ServiceFlags::NODE_NETWORK | ServiceFlags::NODE_BLOOM;
        assert!(flags.contains(ServiceFlags::NODE_NETWORK));
        assert!(flags.contains(ServiceFlags::NODE_BLOOM));
        assert!(!flags.contains(ServiceFlags::NODE_WITNESS));
    }

    #[test]
    fn test_reject_message() {
        let reject = RejectMessage::new(
            "tx",
            RejectCode::InsufficientFee,
            "Fee too low",
            Some("abc123".to_string()),
        );
        assert_eq!(reject.code, RejectCode::InsufficientFee);
    }
}
