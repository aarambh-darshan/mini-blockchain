//! Chain State Management
//!
//! Handles fork resolution, orphan blocks, chain tips, and undo data for reorganizations.
//! This is a critical component for making the blockchain behave like Bitcoin.

use crate::core::block::Block;
use crate::core::transaction::TransactionOutput;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Maximum number of orphan blocks to keep in memory
pub const MAX_ORPHAN_BLOCKS: usize = 100;

/// Maximum time (in seconds) an orphan block can stay in the pool
pub const ORPHAN_BLOCK_EXPIRE_TIME: u64 = 3600; // 1 hour

/// Number of blocks to use for Median Time Past calculation (Bitcoin uses 11)
pub const MTP_BLOCK_COUNT: usize = 11;

/// Maximum allowed time drift into the future (2 hours in seconds)
pub const MAX_FUTURE_BLOCK_TIME: i64 = 7200;

/// Represents a chain tip (end of a chain branch)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainTip {
    /// Hash of the tip block
    pub block_hash: String,
    /// Height of the tip
    pub height: u64,
    /// Total cumulative work on this chain
    pub chain_work: u128,
    /// Whether this is the active (best) chain
    pub is_active: bool,
}

impl ChainTip {
    pub fn new(block_hash: String, height: u64, chain_work: u128, is_active: bool) -> Self {
        Self {
            block_hash,
            height,
            chain_work,
            is_active,
        }
    }
}

/// An orphan block waiting for its parent
#[derive(Debug, Clone)]
pub struct OrphanBlock {
    /// The block itself
    pub block: Block,
    /// Hash of the parent block we're waiting for
    pub parent_hash: String,
    /// Timestamp when this orphan was received
    pub received_at: u64,
}

impl OrphanBlock {
    pub fn new(block: Block, received_at: u64) -> Self {
        let parent_hash = block.header.previous_hash.clone();
        Self {
            block,
            parent_hash,
            received_at,
        }
    }

    /// Check if this orphan has expired
    pub fn is_expired(&self, current_time: u64) -> bool {
        current_time - self.received_at > ORPHAN_BLOCK_EXPIRE_TIME
    }
}

/// Data required to undo a block during reorganization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoData {
    /// Block hash this undo data belongs to
    pub block_hash: String,
    /// Spent outputs that need to be restored (tx_id:vout -> TxOut)
    pub spent_outputs: Vec<(String, TransactionOutput)>,
    /// Transactions that were added by this block (need to remove their outputs)
    pub added_tx_ids: Vec<String>,
}

impl UndoData {
    pub fn new(block_hash: String) -> Self {
        Self {
            block_hash,
            spent_outputs: Vec::new(),
            added_tx_ids: Vec::new(),
        }
    }

    /// Record a spent output for potential restoration
    pub fn record_spent(&mut self, outpoint: String, output: TransactionOutput) {
        self.spent_outputs.push((outpoint, output));
    }

    /// Record a transaction added by this block
    pub fn record_added_tx(&mut self, tx_id: String) {
        self.added_tx_ids.push(tx_id);
    }
}

/// Manages chain state including orphans, tips, and undo data
#[derive(Debug, Clone, Default)]
pub struct ChainStateManager {
    /// Orphan blocks waiting for their parent
    pub orphan_pool: HashMap<String, OrphanBlock>,
    /// Map from parent hash to orphan hashes (for quick lookup when parent arrives)
    pub orphans_by_parent: HashMap<String, Vec<String>>,
    /// All known chain tips
    pub chain_tips: Vec<ChainTip>,
    /// Undo data for recent blocks (for reorganization)
    pub undo_data: HashMap<String, UndoData>,
    /// Block hash to height mapping for quick lookups
    pub block_index: HashMap<String, u64>,
    /// Height to block hash mapping
    pub height_index: HashMap<u64, String>,
}

impl ChainStateManager {
    pub fn new() -> Self {
        Self {
            orphan_pool: HashMap::new(),
            orphans_by_parent: HashMap::new(),
            chain_tips: Vec::new(),
            undo_data: HashMap::new(),
            block_index: HashMap::new(),
            height_index: HashMap::new(),
        }
    }

    /// Add an orphan block to the pool
    pub fn add_orphan(&mut self, block: Block, current_time: u64) -> bool {
        // Check if we already have this orphan or if pool is full
        if self.orphan_pool.len() >= MAX_ORPHAN_BLOCKS {
            self.prune_orphans(current_time);
        }

        let block_hash = block.hash.clone();
        let parent_hash = block.header.previous_hash.clone();

        if self.orphan_pool.contains_key(&block_hash) {
            return false;
        }

        let orphan = OrphanBlock::new(block, current_time);
        self.orphan_pool.insert(block_hash.clone(), orphan);

        // Index by parent hash
        self.orphans_by_parent
            .entry(parent_hash)
            .or_default()
            .push(block_hash);

        true
    }

    /// Get orphan blocks that depend on the given parent hash
    pub fn get_orphans_by_parent(&self, parent_hash: &str) -> Vec<Block> {
        self.orphans_by_parent
            .get(parent_hash)
            .map(|hashes| {
                hashes
                    .iter()
                    .filter_map(|h| self.orphan_pool.get(h))
                    .map(|o| o.block.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Remove an orphan block (when it gets connected)
    pub fn remove_orphan(&mut self, block_hash: &str) {
        if let Some(orphan) = self.orphan_pool.remove(block_hash) {
            // Remove from parent index
            if let Some(siblings) = self.orphans_by_parent.get_mut(&orphan.parent_hash) {
                siblings.retain(|h| h != block_hash);
                if siblings.is_empty() {
                    self.orphans_by_parent.remove(&orphan.parent_hash);
                }
            }
        }
    }

    /// Remove expired orphans
    pub fn prune_orphans(&mut self, current_time: u64) {
        let expired: Vec<String> = self
            .orphan_pool
            .iter()
            .filter(|(_, orphan)| orphan.is_expired(current_time))
            .map(|(hash, _)| hash.clone())
            .collect();

        for hash in expired {
            self.remove_orphan(&hash);
        }
    }

    /// Register a new block in the index
    pub fn index_block(&mut self, block_hash: String, height: u64) {
        self.block_index.insert(block_hash.clone(), height);
        self.height_index.insert(height, block_hash);
    }

    /// Store undo data for a block
    pub fn store_undo_data(&mut self, undo: UndoData) {
        self.undo_data.insert(undo.block_hash.clone(), undo);
    }

    /// Get undo data for a block
    pub fn get_undo_data(&self, block_hash: &str) -> Option<&UndoData> {
        self.undo_data.get(block_hash)
    }

    /// Update the active chain tip
    pub fn set_active_tip(&mut self, block_hash: &str, height: u64, chain_work: u128) {
        // Deactivate all tips
        for tip in &mut self.chain_tips {
            tip.is_active = false;
        }

        // Check if this tip already exists
        if let Some(tip) = self
            .chain_tips
            .iter_mut()
            .find(|t| t.block_hash == block_hash)
        {
            tip.is_active = true;
            tip.chain_work = chain_work;
        } else {
            // Add new tip
            self.chain_tips.push(ChainTip::new(
                block_hash.to_string(),
                height,
                chain_work,
                true,
            ));
        }
    }

    /// Get the active chain tip
    pub fn get_active_tip(&self) -> Option<&ChainTip> {
        self.chain_tips.iter().find(|t| t.is_active)
    }

    /// Remove old chain tips that are too far behind
    pub fn prune_old_tips(&mut self, active_height: u64, max_depth: u64) {
        self.chain_tips
            .retain(|tip| tip.is_active || tip.height + max_depth >= active_height);
    }

    /// Calculate cumulative proof-of-work for a difficulty level
    /// Work = 2^256 / (target + 1), approximated as 2^difficulty
    pub fn calculate_work(difficulty: u32) -> u128 {
        // Each difficulty bit doubles the work required
        // We use u128 to handle large work values
        1u128 << difficulty.min(127) as u128
    }

    /// Check if we have a block at the given height
    pub fn has_block_at_height(&self, height: u64) -> bool {
        self.height_index.contains_key(&height)
    }

    /// Get block hash at height
    pub fn get_hash_at_height(&self, height: u64) -> Option<&String> {
        self.height_index.get(&height)
    }
}

/// Result of attempting to add a block
#[derive(Debug, Clone, PartialEq)]
pub enum BlockStatus {
    /// Block was added to the main chain
    AddedToMainChain,
    /// Block was added as an orphan (waiting for parent)
    AddedAsOrphan,
    /// Block caused a chain reorganization
    CausedReorg { disconnected: u64, connected: u64 },
    /// Block is a duplicate
    Duplicate,
    /// Block is invalid
    Invalid(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_state_manager_new() {
        let manager = ChainStateManager::new();
        assert!(manager.orphan_pool.is_empty());
        assert!(manager.chain_tips.is_empty());
    }

    #[test]
    fn test_calculate_work() {
        // Higher difficulty = more work
        let work_8 = ChainStateManager::calculate_work(8);
        let work_16 = ChainStateManager::calculate_work(16);
        let work_24 = ChainStateManager::calculate_work(24);

        assert_eq!(work_8, 256); // 2^8
        assert_eq!(work_16, 65536); // 2^16
        assert_eq!(work_24, 16777216); // 2^24
        assert!(work_24 > work_16);
        assert!(work_16 > work_8);
    }

    #[test]
    fn test_chain_tip_management() {
        let mut manager = ChainStateManager::new();

        // Set initial tip
        manager.set_active_tip("hash1", 1, 100);
        assert_eq!(manager.chain_tips.len(), 1);
        assert!(manager.get_active_tip().unwrap().is_active);

        // Update to new tip
        manager.set_active_tip("hash2", 2, 200);
        assert_eq!(manager.chain_tips.len(), 2);

        // Only one should be active
        let active_count = manager.chain_tips.iter().filter(|t| t.is_active).count();
        assert_eq!(active_count, 1);
        assert_eq!(manager.get_active_tip().unwrap().block_hash, "hash2");
    }

    #[test]
    fn test_undo_data() {
        let mut manager = ChainStateManager::new();
        let mut undo = UndoData::new("block1".to_string());

        undo.record_spent(
            "tx1:0".to_string(),
            TransactionOutput {
                amount: 100,
                recipient: "addr1".to_string(),
            },
        );
        undo.record_added_tx("tx2".to_string());

        manager.store_undo_data(undo);

        let retrieved = manager.get_undo_data("block1").unwrap();
        assert_eq!(retrieved.spent_outputs.len(), 1);
        assert_eq!(retrieved.added_tx_ids.len(), 1);
    }
}
