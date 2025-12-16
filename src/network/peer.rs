//! Peer management for P2P networking
//!
//! Production-grade peer management with:
//! - Peer scoring and reputation
//! - Misbehavior tracking and banning
//! - Rate limiting (DOS protection)
//! - Connection management

use crate::network::message::{
    Handshake, Message, ServiceFlags, VersionMessage, MIN_PROTOCOL_VERSION,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::{mpsc, RwLock};

// =============================================================================
// Constants
// =============================================================================

/// Maximum number of connected peers
pub const MAX_PEERS: usize = 125;

/// Maximum outbound connections
pub const MAX_OUTBOUND: usize = 8;

/// Maximum inbound connections
pub const MAX_INBOUND: usize = 117;

/// Default peer score (starts positive)
pub const DEFAULT_PEER_SCORE: i32 = 100;

/// Score threshold for disconnection
pub const DISCONNECT_SCORE: i32 = 0;

/// Score threshold for banning
pub const BAN_SCORE: i32 = -100;

/// Default ban duration (24 hours)
pub const DEFAULT_BAN_DURATION: Duration = Duration::from_secs(24 * 60 * 60);

/// Rate limit window (seconds)
pub const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);

/// Maximum messages per window (general)
pub const MAX_MESSAGES_PER_WINDOW: u32 = 1000;

/// Maximum blocks per window
pub const MAX_BLOCKS_PER_WINDOW: u32 = 100;

/// Maximum transactions per window
pub const MAX_TRANSACTIONS_PER_WINDOW: u32 = 5000;

// =============================================================================
// Error Types
// =============================================================================

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
    #[error("Peer banned until {0:?}")]
    Banned(Instant),
    #[error("Incompatible protocol version: {0}")]
    IncompatibleVersion(u32),
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    #[error("Peer misbehaving: {0}")]
    Misbehaving(String),
}

// =============================================================================
// Peer State
// =============================================================================

/// Peer connection state
#[derive(Debug, Clone, PartialEq)]
pub enum PeerState {
    Connecting,
    VersionSent,
    Connected,
    Disconnecting,
    Disconnected,
}

// =============================================================================
// Misbehavior Types
// =============================================================================

/// Types of peer misbehavior
#[derive(Debug, Clone, Copy)]
pub enum Misbehavior {
    /// Sent invalid message
    InvalidMessage,
    /// Sent invalid block
    InvalidBlock,
    /// Sent invalid transaction
    InvalidTransaction,
    /// Protocol violation
    ProtocolViolation,
    /// Excessive traffic
    ExcessiveTraffic,
    /// Invalid proof of work
    InvalidPoW,
    /// Unrequested data
    UnrequestedData,
    /// Spam
    Spam,
}

impl Misbehavior {
    /// Get penalty score for this misbehavior
    pub fn penalty(&self) -> i32 {
        match self {
            Misbehavior::InvalidMessage => 10,
            Misbehavior::InvalidBlock => 100, // Immediate ban
            Misbehavior::InvalidTransaction => 10,
            Misbehavior::ProtocolViolation => 50,
            Misbehavior::ExcessiveTraffic => 20,
            Misbehavior::InvalidPoW => 100, // Immediate ban
            Misbehavior::UnrequestedData => 20,
            Misbehavior::Spam => 30,
        }
    }
}

// =============================================================================
// Rate Limiter
// =============================================================================

/// Rate limiter for DOS protection
#[derive(Debug, Clone)]
pub struct RateLimiter {
    /// Window start time
    window_start: Instant,
    /// Message counts by type
    message_count: u32,
    block_count: u32,
    tx_count: u32,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            window_start: Instant::now(),
            message_count: 0,
            block_count: 0,
            tx_count: 0,
        }
    }

    /// Reset if window expired
    fn maybe_reset(&mut self) {
        if self.window_start.elapsed() > RATE_LIMIT_WINDOW {
            self.window_start = Instant::now();
            self.message_count = 0;
            self.block_count = 0;
            self.tx_count = 0;
        }
    }

    /// Check and record a message
    pub fn check_message(&mut self) -> bool {
        self.maybe_reset();
        self.message_count += 1;
        self.message_count <= MAX_MESSAGES_PER_WINDOW
    }

    /// Check and record a block
    pub fn check_block(&mut self) -> bool {
        self.maybe_reset();
        self.block_count += 1;
        self.block_count <= MAX_BLOCKS_PER_WINDOW
    }

    /// Check and record a transaction
    pub fn check_transaction(&mut self) -> bool {
        self.maybe_reset();
        self.tx_count += 1;
        self.tx_count <= MAX_TRANSACTIONS_PER_WINDOW
    }

    /// Get current stats
    pub fn stats(&self) -> RateLimitStats {
        RateLimitStats {
            messages: self.message_count,
            blocks: self.block_count,
            transactions: self.tx_count,
            window_remaining: RATE_LIMIT_WINDOW.saturating_sub(self.window_start.elapsed()),
        }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// Rate limit statistics
#[derive(Debug, Clone)]
pub struct RateLimitStats {
    pub messages: u32,
    pub blocks: u32,
    pub transactions: u32,
    pub window_remaining: Duration,
}

// =============================================================================
// Peer Info
// =============================================================================

/// Information about a connected peer
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// Peer's address
    pub addr: SocketAddr,
    /// Connection state
    pub state: PeerState,
    /// Protocol version
    pub version: u32,
    /// Services offered
    pub services: ServiceFlags,
    /// Peer's chain height
    pub height: u64,
    /// Peer's best block hash
    pub best_hash: String,
    /// Peer's user agent
    pub user_agent: String,
    /// Whether this is an outbound connection
    pub outbound: bool,
    /// Connection time
    pub connected_at: Instant,
    /// Last message received time
    pub last_recv: Instant,
    /// Last message sent time
    pub last_send: Instant,
    /// Peer score (reputation)
    pub score: i32,
    /// Rate limiter
    pub rate_limiter: RateLimiter,
    /// Ping latency (last measurement)
    pub ping_latency: Option<Duration>,
    /// Nonce of last ping sent
    pub last_ping_nonce: Option<u64>,
    /// Time of last ping sent
    pub last_ping_time: Option<Instant>,
}

impl PeerInfo {
    pub fn new(addr: SocketAddr, outbound: bool) -> Self {
        let now = Instant::now();
        Self {
            addr,
            state: PeerState::Connecting,
            version: 0,
            services: ServiceFlags::default(),
            height: 0,
            best_hash: String::new(),
            user_agent: String::new(),
            outbound,
            connected_at: now,
            last_recv: now,
            last_send: now,
            score: DEFAULT_PEER_SCORE,
            rate_limiter: RateLimiter::new(),
            ping_latency: None,
            last_ping_nonce: None,
            last_ping_time: None,
        }
    }

    /// Update from version message
    pub fn update_from_version(&mut self, version: &VersionMessage) {
        self.version = version.version;
        self.services = version.services;
        self.height = version.start_height;
        self.user_agent = version.user_agent.clone();
        self.state = PeerState::Connected;
    }

    /// Update from legacy handshake
    pub fn update_from_handshake(&mut self, handshake: &Handshake) {
        self.version = handshake.version;
        self.height = handshake.height;
        self.best_hash = handshake.best_hash.clone();
        self.user_agent = handshake.user_agent.clone();
        self.state = PeerState::Connected;
    }

    /// Check if protocol version is compatible
    pub fn is_compatible(&self) -> bool {
        self.version >= MIN_PROTOCOL_VERSION
    }

    /// Record message received
    pub fn record_recv(&mut self) {
        self.last_recv = Instant::now();
    }

    /// Record message sent
    pub fn record_send(&mut self) {
        self.last_send = Instant::now();
    }

    /// Apply penalty for misbehavior
    pub fn penalize(&mut self, behavior: Misbehavior) -> i32 {
        let penalty = behavior.penalty();
        self.score -= penalty;
        self.score
    }

    /// Add positive score (good behavior)
    pub fn reward(&mut self, amount: i32) {
        self.score = (self.score + amount).min(DEFAULT_PEER_SCORE * 2);
    }

    /// Check if peer should be disconnected
    pub fn should_disconnect(&self) -> bool {
        self.score <= DISCONNECT_SCORE
    }

    /// Check if peer should be banned
    pub fn should_ban(&self) -> bool {
        self.score <= BAN_SCORE
    }

    /// Record ping sent
    pub fn record_ping(&mut self, nonce: u64) {
        self.last_ping_nonce = Some(nonce);
        self.last_ping_time = Some(Instant::now());
    }

    /// Record pong received, returns latency if valid
    pub fn record_pong(&mut self, nonce: u64) -> Option<Duration> {
        if self.last_ping_nonce == Some(nonce) {
            if let Some(ping_time) = self.last_ping_time {
                let latency = ping_time.elapsed();
                self.ping_latency = Some(latency);
                self.last_ping_nonce = None;
                self.last_ping_time = None;
                return Some(latency);
            }
        }
        None
    }
}

// =============================================================================
// Ban Entry
// =============================================================================

/// Information about a banned peer
#[derive(Debug, Clone)]
pub struct BanEntry {
    /// Banned address
    pub addr: SocketAddr,
    /// Ban start time
    pub banned_at: Instant,
    /// Ban duration
    pub duration: Duration,
    /// Reason for ban
    pub reason: String,
}

impl BanEntry {
    pub fn new(addr: SocketAddr, duration: Duration, reason: String) -> Self {
        Self {
            addr,
            banned_at: Instant::now(),
            duration,
            reason,
        }
    }

    /// Check if ban has expired
    pub fn is_expired(&self) -> bool {
        self.banned_at.elapsed() > self.duration
    }

    /// Get remaining ban time
    pub fn remaining(&self) -> Duration {
        self.duration.saturating_sub(self.banned_at.elapsed())
    }
}

// =============================================================================
// Peer Handle
// =============================================================================

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

// =============================================================================
// Peer Manager
// =============================================================================

/// Manages all peer connections with scoring and banning
pub struct PeerManager {
    /// Connected peers info
    peers: RwLock<HashMap<SocketAddr, PeerInfo>>,
    /// Peer message senders
    handles: RwLock<HashMap<SocketAddr, PeerHandle>>,
    /// Known peer addresses (for discovery)
    known_peers: RwLock<Vec<String>>,
    /// Banned peers
    banned: RwLock<HashMap<SocketAddr, BanEntry>>,
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
            banned: RwLock::new(HashMap::new()),
            listen_port,
        }
    }

    /// Check if address is banned
    pub async fn is_banned(&self, addr: &SocketAddr) -> bool {
        let banned = self.banned.read().await;
        if let Some(entry) = banned.get(addr) {
            !entry.is_expired()
        } else {
            false
        }
    }

    /// Ban a peer
    pub async fn ban_peer(&self, addr: &SocketAddr, duration: Duration, reason: &str) {
        // Remove from connected peers
        self.remove_peer(addr).await;

        // Add to ban list
        let mut banned = self.banned.write().await;
        banned.insert(*addr, BanEntry::new(*addr, duration, reason.to_string()));

        log::warn!("Banned peer {} for {:?}: {}", addr, duration, reason);
    }

    /// Unban a peer
    pub async fn unban_peer(&self, addr: &SocketAddr) {
        let mut banned = self.banned.write().await;
        banned.remove(addr);
    }

    /// Clean up expired bans
    pub async fn cleanup_bans(&self) {
        let mut banned = self.banned.write().await;
        banned.retain(|_, entry| !entry.is_expired());
    }

    /// Get list of banned peers
    pub async fn get_banned(&self) -> Vec<BanEntry> {
        let banned = self.banned.read().await;
        banned.values().cloned().collect()
    }

    /// Add a new peer
    pub async fn add_peer(
        &self,
        addr: SocketAddr,
        handle: PeerHandle,
        outbound: bool,
    ) -> Result<(), PeerError> {
        // Check if banned
        if self.is_banned(&addr).await {
            let banned = self.banned.read().await;
            if let Some(entry) = banned.get(&addr) {
                return Err(PeerError::Banned(entry.banned_at + entry.duration));
            }
        }

        let mut peers = self.peers.write().await;

        // Check connection limits
        let outbound_count = peers.values().filter(|p| p.outbound).count();
        let inbound_count = peers.values().filter(|p| !p.outbound).count();

        if outbound && outbound_count >= MAX_OUTBOUND {
            return Err(PeerError::MaxPeersReached);
        }
        if !outbound && inbound_count >= MAX_INBOUND {
            return Err(PeerError::MaxPeersReached);
        }
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

    /// Report misbehavior and potentially ban
    pub async fn report_misbehavior(
        &self,
        addr: &SocketAddr,
        behavior: Misbehavior,
    ) -> Result<(), PeerError> {
        let mut peers = self.peers.write().await;
        if let Some(peer) = peers.get_mut(addr) {
            let new_score = peer.penalize(behavior);
            log::warn!(
                "Peer {} misbehaved ({:?}), score: {}",
                addr,
                behavior,
                new_score
            );

            if peer.should_ban() {
                drop(peers); // Release lock before banning
                self.ban_peer(addr, DEFAULT_BAN_DURATION, &format!("{:?}", behavior))
                    .await;
                return Err(PeerError::Misbehaving(format!("{:?}", behavior)));
            } else if peer.should_disconnect() {
                drop(peers);
                self.remove_peer(addr).await;
                return Err(PeerError::Misbehaving(format!("{:?}", behavior)));
            }
        }
        Ok(())
    }

    /// Check rate limit for a message type
    pub async fn check_rate_limit(
        &self,
        addr: &SocketAddr,
        msg: &Message,
    ) -> Result<(), PeerError> {
        let mut peers = self.peers.write().await;
        if let Some(peer) = peers.get_mut(addr) {
            let allowed = match msg {
                Message::NewBlock(_) | Message::Blocks(_) => peer.rate_limiter.check_block(),
                Message::NewTransaction(_) => peer.rate_limiter.check_transaction(),
                _ => peer.rate_limiter.check_message(),
            };

            if !allowed {
                log::warn!("Rate limit exceeded for peer {}", addr);
                drop(peers);
                self.report_misbehavior(addr, Misbehavior::ExcessiveTraffic)
                    .await?;
                return Err(PeerError::RateLimitExceeded);
            }
        }
        Ok(())
    }

    /// Update peer from version message
    pub async fn update_peer_version(
        &self,
        addr: &SocketAddr,
        version: &VersionMessage,
    ) -> Result<(), PeerError> {
        if version.version < MIN_PROTOCOL_VERSION {
            return Err(PeerError::IncompatibleVersion(version.version));
        }

        let mut peers = self.peers.write().await;
        if let Some(peer) = peers.get_mut(addr) {
            peer.update_from_version(version);
            log::info!(
                "Peer {} version: {}, services: {:?}, height: {}, agent: {}",
                addr,
                version.version,
                version.services,
                version.start_height,
                version.user_agent
            );
        }
        Ok(())
    }

    /// Update peer info after handshake (legacy)
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

    /// Get statistics
    pub async fn stats(&self) -> PeerManagerStats {
        let peers = self.peers.read().await;
        let banned = self.banned.read().await;

        let connected = peers
            .values()
            .filter(|p| p.state == PeerState::Connected)
            .count();
        let outbound = peers.values().filter(|p| p.outbound).count();
        let avg_score = if !peers.is_empty() {
            peers.values().map(|p| p.score as f64).sum::<f64>() / peers.len() as f64
        } else {
            0.0
        };

        PeerManagerStats {
            total_peers: peers.len(),
            connected_peers: connected,
            outbound_peers: outbound,
            inbound_peers: peers.len() - outbound,
            banned_count: banned.len(),
            average_score: avg_score,
        }
    }
}

/// Peer manager statistics
#[derive(Debug, Clone)]
pub struct PeerManagerStats {
    pub total_peers: usize,
    pub connected_peers: usize,
    pub outbound_peers: usize,
    pub inbound_peers: usize,
    pub banned_count: usize,
    pub average_score: f64,
}
