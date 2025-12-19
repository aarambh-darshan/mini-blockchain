//! Parallel Block Download for Enhanced Synchronization
//!
//! Production-grade sync features:
//! - Parallel block download from multiple peers
//! - Download scheduler with window management
//! - Stale tip detection
//! - Request timeout handling

use crate::core::{Block, Blockchain};
use crate::network::message::Message;
use crate::network::peer::{PeerManager, PeerState};
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

// =============================================================================
// Constants
// =============================================================================

/// Maximum parallel block downloads per peer
const MAX_BLOCKS_PER_PEER: usize = 16;

/// Maximum total in-flight block requests
const MAX_IN_FLIGHT_BLOCKS: usize = 128;

/// Block request timeout
const BLOCK_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Stale tip detection threshold
const STALE_TIP_THRESHOLD: Duration = Duration::from_secs(30 * 60);

/// Maximum block height difference for parallel download
const PARALLEL_DOWNLOAD_THRESHOLD: u64 = 144; // ~1 day of blocks

/// Batch size for block requests
const BATCH_SIZE: u32 = 16;

// =============================================================================
// Download Request
// =============================================================================

/// A pending block download request
#[derive(Debug, Clone)]
struct BlockRequest {
    /// Block height
    height: u64,
    /// Block hash (if known from headers)
    hash: Option<String>,
    /// Peer we requested from
    peer: SocketAddr,
    /// When we sent the request
    requested_at: Instant,
    /// Number of retries
    retries: u32,
}

impl BlockRequest {
    fn is_timed_out(&self) -> bool {
        self.requested_at.elapsed() > BLOCK_REQUEST_TIMEOUT
    }
}

// =============================================================================
// Peer Download State
// =============================================================================

/// Download state for a single peer
#[derive(Debug)]
struct PeerDownloadState {
    /// Blocks currently being downloaded from this peer
    in_flight: HashSet<u64>,
    /// Number of blocks successfully received
    blocks_received: u64,
    /// Number of failed/timed out requests
    failures: u32,
    /// Average block download latency
    avg_latency_ms: f64,
    /// Last successful download time
    last_success: Option<Instant>,
}

impl PeerDownloadState {
    fn new() -> Self {
        Self {
            in_flight: HashSet::new(),
            blocks_received: 0,
            failures: 0,
            avg_latency_ms: 0.0,
            last_success: None,
        }
    }

    fn available_slots(&self) -> usize {
        MAX_BLOCKS_PER_PEER.saturating_sub(self.in_flight.len())
    }

    fn record_success(&mut self, latency_ms: f64) {
        self.blocks_received += 1;
        self.last_success = Some(Instant::now());
        
        // Exponential moving average
        self.avg_latency_ms = if self.blocks_received == 1 {
            latency_ms
        } else {
            self.avg_latency_ms * 0.9 + latency_ms * 0.1
        };
    }

    fn record_failure(&mut self) {
        self.failures += 1;
    }

    /// Score for peer selection (higher = better)
    fn score(&self) -> f64 {
        if self.blocks_received == 0 {
            return 1.0;
        }
        
        let success_rate = self.blocks_received as f64 / (self.blocks_received as f64 + self.failures as f64);
        let speed_factor = 1000.0 / (self.avg_latency_ms + 100.0);
        
        success_rate * speed_factor
    }
}

// =============================================================================
// Parallel Sync Manager
// =============================================================================

/// Manages parallel block download from multiple peers
pub struct ParallelSync {
    /// Blockchain reference
    blockchain: Arc<RwLock<Blockchain>>,
    /// Peer manager reference
    peer_manager: Arc<PeerManager>,
    /// Download queue (blocks to download)
    queue: RwLock<VecDeque<u64>>,
    /// In-flight requests
    in_flight: RwLock<HashMap<u64, BlockRequest>>,
    /// Peer download states
    peer_states: RwLock<HashMap<SocketAddr, PeerDownloadState>>,
    /// Received blocks waiting to be processed
    block_buffer: RwLock<HashMap<u64, Block>>,
    /// Next block height to process
    next_height: RwLock<u64>,
    /// Is sync in progress
    syncing: RwLock<bool>,
    /// Last tip update time
    last_tip_update: RwLock<Instant>,
    /// Sync target height
    target_height: RwLock<u64>,
}

impl ParallelSync {
    /// Create a new parallel sync manager
    pub fn new(blockchain: Arc<RwLock<Blockchain>>, peer_manager: Arc<PeerManager>) -> Self {
        Self {
            blockchain,
            peer_manager,
            queue: RwLock::new(VecDeque::new()),
            in_flight: RwLock::new(HashMap::new()),
            peer_states: RwLock::new(HashMap::new()),
            block_buffer: RwLock::new(HashMap::new()),
            next_height: RwLock::new(0),
            syncing: RwLock::new(false),
            last_tip_update: RwLock::new(Instant::now()),
            target_height: RwLock::new(0),
        }
    }

    /// Start syncing to a target height
    pub async fn start_sync(&self, target: u64) -> Result<(), SyncError> {
        let current_height = {
            let chain = self.blockchain.read().await;
            chain.height()
        };

        if target <= current_height {
            return Ok(());
        }

        log::info!(
            "Starting parallel sync from {} to {} ({} blocks)",
            current_height,
            target,
            target - current_height
        );

        // Initialize state
        *self.next_height.write().await = current_height + 1;
        *self.target_height.write().await = target;
        *self.syncing.write().await = true;

        // Build initial queue
        let mut queue = self.queue.write().await;
        queue.clear();
        for height in (current_height + 1)..=target {
            queue.push_back(height);
        }

        // Start downloading
        drop(queue);
        self.schedule_downloads().await;

        Ok(())
    }

    /// Check if we have a stale tip
    pub async fn is_stale_tip(&self) -> bool {
        let last_update = self.last_tip_update.read().await;
        last_update.elapsed() > STALE_TIP_THRESHOLD
    }

    /// Record a new tip update
    pub async fn record_tip_update(&self) {
        *self.last_tip_update.write().await = Instant::now();
    }

    /// Is sync in progress
    pub async fn is_syncing(&self) -> bool {
        *self.syncing.read().await
    }

    /// Schedule block downloads to available peers
    async fn schedule_downloads(&self) {
        let in_flight = self.in_flight.read().await;
        if in_flight.len() >= MAX_IN_FLIGHT_BLOCKS {
            return;
        }
        drop(in_flight);

        // Get available peers
        let peers = self.peer_manager.get_all_peer_info().await;
        let connected_peers: Vec<_> = peers
            .into_iter()
            .filter(|p| p.state == PeerState::Connected)
            .collect();

        if connected_peers.is_empty() {
            return;
        }

        // Update peer states
        let mut peer_states = self.peer_states.write().await;
        for peer in &connected_peers {
            peer_states.entry(peer.addr).or_insert_with(PeerDownloadState::new);
        }

        // Calculate slots per peer
        let mut peer_slots: Vec<_> = connected_peers
            .iter()
            .filter_map(|p| {
                let state = peer_states.get(&p.addr)?;
                let slots = state.available_slots();
                if slots > 0 {
                    Some((p.addr, slots, state.score()))
                } else {
                    None
                }
            })
            .collect();

        // Sort by score (descending)
        peer_slots.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

        drop(peer_states);

        // Assign downloads
        let mut queue = self.queue.write().await;
        let mut in_flight = self.in_flight.write().await;
        let mut peer_states = self.peer_states.write().await;

        for (peer_addr, slots, _) in peer_slots {
            for _ in 0..slots {
                if in_flight.len() >= MAX_IN_FLIGHT_BLOCKS {
                    break;
                }

                if let Some(height) = queue.pop_front() {
                    let request = BlockRequest {
                        height,
                        hash: None,
                        peer: peer_addr,
                        requested_at: Instant::now(),
                        retries: 0,
                    };

                    in_flight.insert(height, request);

                    if let Some(state) = peer_states.get_mut(&peer_addr) {
                        state.in_flight.insert(height);
                    }

                    // Send request
                    let msg = Message::GetBlocks {
                        start_height: height,
                        count: BATCH_SIZE.min((self.target_height.read().await.clone() - height + 1) as u32),
                    };

                    if let Err(e) = self.peer_manager.send_to(&peer_addr, msg).await {
                        log::debug!("Failed to send block request to {}: {}", peer_addr, e);
                    }
                } else {
                    break;
                }
            }
        }
    }

    /// Handle received blocks
    pub async fn handle_blocks(&self, blocks: Vec<Block>, from: SocketAddr) -> Result<(), SyncError> {
        if blocks.is_empty() {
            return Ok(());
        }

        let now = Instant::now();

        // Record received blocks
        let mut in_flight = self.in_flight.write().await;
        let mut peer_states = self.peer_states.write().await;
        let mut block_buffer = self.block_buffer.write().await;

        for block in blocks {
            let height = block.index;

            // Remove from in-flight
            if let Some(request) = in_flight.remove(&height) {
                let latency_ms = request.requested_at.elapsed().as_millis() as f64;

                if let Some(state) = peer_states.get_mut(&from) {
                    state.in_flight.remove(&height);
                    state.record_success(latency_ms);
                }
            }

            // Add to buffer
            block_buffer.insert(height, block);
        }

        drop(in_flight);
        drop(peer_states);
        drop(block_buffer);

        // Try to process buffered blocks in order
        self.process_buffer().await?;

        // Schedule more downloads
        self.schedule_downloads().await;

        // Check if sync is complete
        self.check_complete().await;

        self.record_tip_update().await;

        Ok(())
    }

    /// Process blocks in order from the buffer
    async fn process_buffer(&self) -> Result<(), SyncError> {
        let mut next_height = self.next_height.write().await;
        let mut block_buffer = self.block_buffer.write().await;
        let mut blockchain = self.blockchain.write().await;

        while let Some(block) = block_buffer.remove(&*next_height) {
            match blockchain.process_block(block) {
                Ok(_) => {
                    *next_height += 1;
                }
                Err(e) => {
                    log::warn!("Failed to process block {}: {:?}", *next_height, e);
                    // Don't fail the entire sync for one bad block
                    *next_height += 1;
                }
            }
        }

        Ok(())
    }

    /// Handle request timeout
    pub async fn handle_timeout(&self, height: u64) {
        let mut in_flight = self.in_flight.write().await;
        
        if let Some(mut request) = in_flight.remove(&height) {
            // Record failure
            let mut peer_states = self.peer_states.write().await;
            if let Some(state) = peer_states.get_mut(&request.peer) {
                state.in_flight.remove(&height);
                state.record_failure();
            }
            drop(peer_states);

            // Retry if not too many attempts
            if request.retries < 3 {
                request.retries += 1;
                let mut queue = self.queue.write().await;
                queue.push_front(height);
            }
        }

        drop(in_flight);
        self.schedule_downloads().await;
    }

    /// Check and handle timed out requests
    pub async fn check_timeouts(&self) {
        let in_flight = self.in_flight.read().await;
        let timed_out: Vec<_> = in_flight
            .iter()
            .filter(|(_, r)| r.is_timed_out())
            .map(|(h, _)| *h)
            .collect();
        drop(in_flight);

        for height in timed_out {
            self.handle_timeout(height).await;
        }
    }

    /// Check if sync is complete
    async fn check_complete(&self) {
        let target = *self.target_height.read().await;
        let current = {
            let chain = self.blockchain.read().await;
            chain.height()
        };

        if current >= target {
            *self.syncing.write().await = false;
            log::info!("Parallel sync complete at height {}", current);
        }
    }

    /// Get sync statistics
    pub async fn stats(&self) -> ParallelSyncStats {
        let in_flight = self.in_flight.read().await;
        let queue = self.queue.read().await;
        let block_buffer = self.block_buffer.read().await;
        let peer_states = self.peer_states.read().await;

        ParallelSyncStats {
            in_flight: in_flight.len(),
            queued: queue.len(),
            buffered: block_buffer.len(),
            active_peers: peer_states.len(),
            syncing: *self.syncing.read().await,
            target_height: *self.target_height.read().await,
        }
    }
}

// =============================================================================
// Types
// =============================================================================

#[derive(Debug)]
pub enum SyncError {
    NoPeers,
    BlockchainError(String),
}

#[derive(Debug, Clone)]
pub struct ParallelSyncStats {
    pub in_flight: usize,
    pub queued: usize,
    pub buffered: usize,
    pub active_peers: usize,
    pub syncing: bool,
    pub target_height: u64,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_download_state() {
        let mut state = PeerDownloadState::new();
        assert_eq!(state.available_slots(), MAX_BLOCKS_PER_PEER);
        
        state.in_flight.insert(1);
        assert_eq!(state.available_slots(), MAX_BLOCKS_PER_PEER - 1);
    }

    #[test]
    fn test_peer_score() {
        let mut state = PeerDownloadState::new();
        assert_eq!(state.score(), 1.0);
        
        state.record_success(100.0);
        assert!(state.score() > 0.0);
        
        state.record_failure();
        let score1 = state.score();
        
        state.record_failure();
        let score2 = state.score();
        
        assert!(score2 < score1);
    }

    #[test]
    fn test_block_request_timeout() {
        let request = BlockRequest {
            height: 1,
            hash: None,
            peer: "127.0.0.1:8333".parse().unwrap(),
            requested_at: Instant::now() - BLOCK_REQUEST_TIMEOUT - Duration::from_secs(1),
            retries: 0,
        };
        
        assert!(request.is_timed_out());
    }
}
