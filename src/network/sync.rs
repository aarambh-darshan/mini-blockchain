//! Chain synchronization with peers
//!
//! Handles syncing the blockchain with connected peers.

use crate::core::{Block, Blockchain, BlockchainError};
use crate::network::message::Message;
use crate::network::peer::PeerManager;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Blocks to request per batch
const SYNC_BATCH_SIZE: u32 = 50;

/// Chain synchronization manager
pub struct ChainSync {
    blockchain: Arc<RwLock<Blockchain>>,
    peer_manager: Arc<PeerManager>,
    syncing: RwLock<bool>,
}

impl ChainSync {
    pub fn new(blockchain: Arc<RwLock<Blockchain>>, peer_manager: Arc<PeerManager>) -> Self {
        Self {
            blockchain,
            peer_manager,
            syncing: RwLock::new(false),
        }
    }

    /// Check if we need to sync and start sync if needed
    pub async fn check_sync(&self) -> bool {
        let our_height = {
            let chain = self.blockchain.read().await;
            chain.height()
        };

        // Find peer with higher chain
        if let Some((peer_addr, peer_height)) = self.peer_manager.get_best_peer().await {
            if peer_height > our_height {
                log::info!(
                    "Peer {} has higher chain ({} vs {}), starting sync",
                    peer_addr,
                    peer_height,
                    our_height
                );
                self.start_sync(peer_addr, our_height + 1).await;
                return true;
            }
        }

        false
    }

    /// Start syncing from a specific peer
    async fn start_sync(&self, peer: SocketAddr, start_height: u64) {
        let mut syncing = self.syncing.write().await;
        if *syncing {
            return;
        }
        *syncing = true;
        drop(syncing);

        // Request blocks
        let msg = Message::GetBlocks {
            start_height,
            count: SYNC_BATCH_SIZE,
        };

        if let Err(e) = self.peer_manager.send_to(&peer, msg).await {
            log::warn!("Failed to request blocks from {}: {}", peer, e);
        }
    }

    /// Handle received blocks
    pub async fn handle_blocks(
        &self,
        blocks: Vec<Block>,
        from: SocketAddr,
    ) -> Result<usize, BlockchainError> {
        if blocks.is_empty() {
            let mut syncing = self.syncing.write().await;
            *syncing = false;
            return Ok(0);
        }

        let mut chain = self.blockchain.write().await;
        let mut added = 0;

        for block in blocks {
            // Verify block connects to our chain
            if block.index != chain.height() + 1 {
                log::warn!(
                    "Block {} doesn't connect to chain height {}",
                    block.index,
                    chain.height()
                );
                continue;
            }

            match chain.add_block(block) {
                Ok(()) => {
                    added += 1;
                }
                Err(e) => {
                    log::warn!("Failed to add synced block: {}", e);
                    break;
                }
            }
        }

        let current_height = chain.height();
        drop(chain);

        log::info!("Synced {} blocks, height now {}", added, current_height);

        // Continue syncing if we added a full batch
        if added == SYNC_BATCH_SIZE as usize {
            let msg = Message::GetBlocks {
                start_height: current_height + 1,
                count: SYNC_BATCH_SIZE,
            };
            if let Err(e) = self.peer_manager.send_to(&from, msg).await {
                log::warn!("Failed to continue sync: {}", e);
            }
        } else {
            let mut syncing = self.syncing.write().await;
            *syncing = false;
            log::info!("Sync complete");
        }

        Ok(added)
    }

    /// Handle a new block announcement
    pub async fn handle_new_block(
        &self,
        block: Block,
        from: SocketAddr,
    ) -> Result<bool, BlockchainError> {
        let mut chain = self.blockchain.write().await;

        // Check if block connects to our chain
        if block.index == chain.height() + 1 {
            if block.header.previous_hash == chain.latest_block().hash {
                chain.add_block(block.clone())?;
                log::info!("Added new block {} from peer", block.index);

                // Relay to other peers
                drop(chain);
                self.peer_manager
                    .broadcast_except(Message::NewBlock(block), &from)
                    .await;

                return Ok(true);
            }
        } else if block.index > chain.height() + 1 {
            // We're behind, need to sync
            drop(chain);
            self.check_sync().await;
        }

        Ok(false)
    }

    /// Get blocks for a peer request
    pub async fn get_blocks(&self, start_height: u64, count: u32) -> Vec<Block> {
        let chain = self.blockchain.read().await;
        let mut blocks = Vec::new();

        for i in 0..count {
            let height = start_height + i as u64;
            if let Some(block) = chain.get_block(height) {
                blocks.push(block.clone());
            } else {
                break;
            }
        }

        blocks
    }

    /// Check if currently syncing
    pub async fn is_syncing(&self) -> bool {
        *self.syncing.read().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_chain_sync_creation() {
        let blockchain = Arc::new(RwLock::new(Blockchain::with_difficulty(4)));
        let peer_manager = Arc::new(PeerManager::new(8333));
        let sync = ChainSync::new(blockchain, peer_manager);

        assert!(!sync.is_syncing().await);
    }
}
