//! Network message types for P2P communication
//!
//! Defines all message types used in the gossip protocol.

use crate::core::{Block, Transaction};
use serde::{Deserialize, Serialize};

/// Protocol version
pub const PROTOCOL_VERSION: u32 = 1;

/// Magic bytes for message framing
pub const MAGIC: [u8; 4] = [0x4D, 0x49, 0x4E, 0x49]; // "MINI"

/// Network message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Initial handshake when connecting
    Handshake(Handshake),

    /// Announce a newly mined block
    NewBlock(Block),

    /// Announce a new transaction
    NewTransaction(Transaction),

    /// Request blocks starting from a height
    GetBlocks { start_height: u64, count: u32 },

    /// Response with requested blocks
    Blocks(Vec<Block>),

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
}

/// Handshake message for initial connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Handshake {
    /// Protocol version
    pub version: u32,
    /// Node's chain height
    pub height: u64,
    /// Node's best block hash
    pub best_hash: String,
    /// Node's listening port (for incoming connections)
    pub listen_port: u16,
    /// Node's user agent string
    pub user_agent: String,
}

impl Handshake {
    /// Create a new handshake message
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
            Message::Handshake(_) => "Handshake",
            Message::NewBlock(_) => "NewBlock",
            Message::NewTransaction(_) => "NewTransaction",
            Message::GetBlocks { .. } => "GetBlocks",
            Message::Blocks(_) => "Blocks",
            Message::GetPeers => "GetPeers",
            Message::Peers(_) => "Peers",
            Message::Ping(_) => "Ping",
            Message::Pong(_) => "Pong",
            Message::GetHeight => "GetHeight",
            Message::Height(_) => "Height",
        }
    }
}

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
}
