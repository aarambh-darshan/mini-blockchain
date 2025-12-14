//! Peer management for P2P networking
//!
//! Handles peer connections, tracking, and message routing.

use crate::network::message::{Handshake, Message};
use std::collections::HashMap;
use std::net::SocketAddr;
use thiserror::Error;
use tokio::sync::{mpsc, RwLock};

/// Maximum number of connected peers
pub const MAX_PEERS: usize = 8;

/// Peer connection errors
#[derive(Error, Debug)]
pub enum PeerError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("Peer disconnected")]
    Disconnected,
    #[error("Max peers reached")]
    MaxPeersReached,
    #[error("Invalid handshake")]
    InvalidHandshake,
}

/// Peer connection state
#[derive(Debug, Clone, PartialEq)]
pub enum PeerState {
    Connecting,
    Connected,
    Disconnected,
}

/// Information about a connected peer
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// Peer's address
    pub addr: SocketAddr,
    /// Connection state
    pub state: PeerState,
    /// Peer's chain height
    pub height: u64,
    /// Peer's best block hash
    pub best_hash: String,
    /// Peer's user agent
    pub user_agent: String,
    /// Whether this is an outbound connection
    pub outbound: bool,
}

impl PeerInfo {
    pub fn new(addr: SocketAddr, outbound: bool) -> Self {
        Self {
            addr,
            state: PeerState::Connecting,
            height: 0,
            best_hash: String::new(),
            user_agent: String::new(),
            outbound,
        }
    }

    pub fn update_from_handshake(&mut self, handshake: &Handshake) {
        self.height = handshake.height;
        self.best_hash = handshake.best_hash.clone();
        self.user_agent = handshake.user_agent.clone();
        self.state = PeerState::Connected;
    }
}

/// Handle for sending messages to a peer
#[derive(Clone)]
pub struct PeerHandle {
    pub addr: SocketAddr,
    pub tx: mpsc::Sender<Message>,
}

impl PeerHandle {
    pub async fn send(&self, msg: Message) -> Result<(), PeerError> {
        self.tx.send(msg).await.map_err(|_| PeerError::Disconnected)
    }
}

/// Manages all peer connections
pub struct PeerManager {
    /// Connected peers info
    peers: RwLock<HashMap<SocketAddr, PeerInfo>>,
    /// Peer message senders
    handles: RwLock<HashMap<SocketAddr, PeerHandle>>,
    /// Known peer addresses (for discovery)
    known_peers: RwLock<Vec<String>>,
    /// Our listening port
    #[allow(dead_code)]
    listen_port: u16,
}

impl PeerManager {
    pub fn new(listen_port: u16) -> Self {
        Self {
            peers: RwLock::new(HashMap::new()),
            handles: RwLock::new(HashMap::new()),
            known_peers: RwLock::new(Vec::new()),
            listen_port,
        }
    }

    /// Add a new peer
    pub async fn add_peer(
        &self,
        addr: SocketAddr,
        handle: PeerHandle,
        outbound: bool,
    ) -> Result<(), PeerError> {
        let mut peers = self.peers.write().await;

        if peers.len() >= MAX_PEERS {
            return Err(PeerError::MaxPeersReached);
        }

        peers.insert(addr, PeerInfo::new(addr, outbound));

        let mut handles = self.handles.write().await;
        handles.insert(addr, handle);

        // Add to known peers
        let mut known = self.known_peers.write().await;
        let addr_str = addr.to_string();
        if !known.contains(&addr_str) {
            known.push(addr_str);
        }

        log::info!("Added peer: {} (outbound: {})", addr, outbound);
        Ok(())
    }

    /// Remove a peer
    pub async fn remove_peer(&self, addr: &SocketAddr) {
        let mut peers = self.peers.write().await;
        peers.remove(addr);

        let mut handles = self.handles.write().await;
        handles.remove(addr);

        log::info!("Removed peer: {}", addr);
    }

    /// Update peer info after handshake
    pub async fn update_peer(&self, addr: &SocketAddr, handshake: &Handshake) {
        let mut peers = self.peers.write().await;
        if let Some(peer) = peers.get_mut(addr) {
            peer.update_from_handshake(handshake);
            log::info!(
                "Peer {} connected: height={}, agent={}",
                addr,
                handshake.height,
                handshake.user_agent
            );
        }
    }

    /// Get all connected peer addresses
    pub async fn get_peers(&self) -> Vec<SocketAddr> {
        let peers = self.peers.read().await;
        peers.keys().cloned().collect()
    }

    /// Get peer info
    pub async fn get_peer_info(&self, addr: &SocketAddr) -> Option<PeerInfo> {
        let peers = self.peers.read().await;
        peers.get(addr).cloned()
    }

    /// Get all peer infos
    pub async fn get_all_peer_info(&self) -> Vec<PeerInfo> {
        let peers = self.peers.read().await;
        peers.values().cloned().collect()
    }

    /// Get peer count
    pub async fn peer_count(&self) -> usize {
        let peers = self.peers.read().await;
        peers.len()
    }

    /// Get known peer addresses (as strings)
    pub async fn get_known_peers(&self) -> Vec<String> {
        let known = self.known_peers.read().await;
        known.clone()
    }

    /// Add known peer addresses
    pub async fn add_known_peers(&self, addrs: Vec<String>) {
        let mut known = self.known_peers.write().await;
        for addr in addrs {
            if !known.contains(&addr) {
                known.push(addr);
            }
        }
    }

    /// Broadcast a message to all peers
    pub async fn broadcast(&self, msg: Message) {
        let handles = self.handles.read().await;
        for (addr, handle) in handles.iter() {
            if let Err(e) = handle.send(msg.clone()).await {
                log::warn!("Failed to send to {}: {}", addr, e);
            }
        }
    }

    /// Broadcast a message to all peers except one
    pub async fn broadcast_except(&self, msg: Message, except: &SocketAddr) {
        let handles = self.handles.read().await;
        for (addr, handle) in handles.iter() {
            if addr != except {
                if let Err(e) = handle.send(msg.clone()).await {
                    log::warn!("Failed to send to {}: {}", addr, e);
                }
            }
        }
    }

    /// Send a message to a specific peer
    pub async fn send_to(&self, addr: &SocketAddr, msg: Message) -> Result<(), PeerError> {
        let handles = self.handles.read().await;
        if let Some(handle) = handles.get(addr) {
            handle.send(msg).await
        } else {
            Err(PeerError::Disconnected)
        }
    }

    /// Get the peer with the highest chain
    pub async fn get_best_peer(&self) -> Option<(SocketAddr, u64)> {
        let peers = self.peers.read().await;
        peers
            .iter()
            .filter(|(_, p)| p.state == PeerState::Connected)
            .max_by_key(|(_, p)| p.height)
            .map(|(addr, p)| (*addr, p.height))
    }
}
