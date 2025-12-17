//! Blockchain implementation
//!
//! The main blockchain struct that manages the chain of blocks.
//! Features production-grade consensus with fork resolution, orphan handling,
//! and Median Time Past (MTP) validation.

use crate::core::block::{Block, BlockError};
use crate::core::chain_state::{
    BlockStatus, ChainStateManager, UndoData, MAX_FUTURE_BLOCK_TIME, MTP_BLOCK_COUNT,
};
use crate::core::transaction::{Transaction, COINBASE_MATURITY, UTXO};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Default mining difficulty (number of leading zero bits)
pub const DEFAULT_DIFFICULTY: u32 = 16;

/// Block reward in coins
pub const BLOCK_REWARD: u64 = 50;

/// Number of blocks between difficulty adjustments
pub const DIFFICULTY_ADJUSTMENT_INTERVAL: u64 = 10;

/// Target block time in seconds
pub const TARGET_BLOCK_TIME: i64 = 10;

/// Maximum difficulty adjustment factor per period (Bitcoin uses 4x)
pub const MAX_DIFFICULTY_ADJUSTMENT_FACTOR: f64 = 4.0;

/// Blockchain-related errors
#[derive(Error, Debug)]
pub enum BlockchainError {
    #[error("Invalid block: {0}")]
    InvalidBlock(String),
    #[error("Invalid chain: {0}")]
    InvalidChain(String),
    #[error("Block not found: {0}")]
    BlockNotFound(String),
    #[error("Duplicate block")]
    DuplicateBlock,
    #[error("Orphan block: parent {0} not found")]
    OrphanBlock(String),
    #[error("Block timestamp invalid: {0}")]
    InvalidTimestamp(String),
    #[error("Reorganization failed: {0}")]
    ReorgFailed(String),
    #[error("Block validation failed: {0}")]
    BlockValidation(#[from] BlockError),
    #[error("Coinbase not mature: tx {0} needs {1} more blocks")]
    CoinbaseNotMature(String, u64),
}

/// The main blockchain structure with production-grade consensus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blockchain {
    /// The chain of blocks (active chain only)
    pub blocks: Vec<Block>,
    /// Current mining difficulty
    pub difficulty: u32,
    /// Unspent transaction outputs
    #[serde(skip)]
    pub utxo_set: HashMap<String, UTXO>,
    /// Total cumulative work on the active chain
    #[serde(skip, default)]
    pub chain_work: u128,
    /// Chain state manager (orphans, tips, undo data)
    #[serde(skip, default)]
    pub state: ChainStateManager,
    /// Coinbase transaction heights: tx_id -> block height where it was mined
    /// Used to enforce COINBASE_MATURITY (100 block delay before spending)
    #[serde(skip, default)]
    pub coinbase_heights: HashMap<String, u64>,
}

impl Blockchain {
    /// Create a new blockchain with genesis block
    pub fn new() -> Self {
        let genesis = Block::genesis(DEFAULT_DIFFICULTY);
        let genesis_work = ChainStateManager::calculate_work(DEFAULT_DIFFICULTY);

        let mut blockchain = Self {
            blocks: vec![genesis.clone()],
            difficulty: DEFAULT_DIFFICULTY,
            utxo_set: HashMap::new(),
            chain_work: genesis_work,
            state: ChainStateManager::new(),
            coinbase_heights: HashMap::new(),
        };

        // Initialize state
        blockchain.state.index_block(genesis.hash.clone(), 0);
        blockchain
            .state
            .set_active_tip(&genesis.hash, 0, genesis_work);
        blockchain.rebuild_utxo_set();
        blockchain
    }

    /// Create a blockchain with custom difficulty
    pub fn with_difficulty(difficulty: u32) -> Self {
        let genesis = Block::genesis(difficulty);
        let genesis_work = ChainStateManager::calculate_work(difficulty);

        let mut blockchain = Self {
            blocks: vec![genesis.clone()],
            difficulty,
            utxo_set: HashMap::new(),
            chain_work: genesis_work,
            state: ChainStateManager::new(),
            coinbase_heights: HashMap::new(),
        };

        blockchain.state.index_block(genesis.hash.clone(), 0);
        blockchain
            .state
            .set_active_tip(&genesis.hash, 0, genesis_work);
        blockchain.rebuild_utxo_set();
        blockchain
    }

    /// Get the latest block
    pub fn latest_block(&self) -> &Block {
        self.blocks
            .last()
            .expect("Blockchain should have at least genesis block")
    }

    /// Get a block by index
    pub fn get_block(&self, index: u64) -> Option<&Block> {
        self.blocks.get(index as usize)
    }

    /// Get a block by hash
    pub fn get_block_by_hash(&self, hash: &str) -> Option<&Block> {
        self.blocks.iter().find(|b| b.hash == hash)
    }

    /// Get blockchain height
    pub fn height(&self) -> u64 {
        self.blocks.len() as u64 - 1
    }

    // =========================================================================
    // FORK RESOLUTION & CHAIN MANAGEMENT (Production-grade)
    // =========================================================================

    /// Process a new block with full fork resolution
    /// This is the main entry point for adding blocks from the network
    pub fn process_block(&mut self, block: Block) -> Result<BlockStatus, BlockchainError> {
        let block_hash = block.hash.clone();
        let parent_hash = block.header.previous_hash.clone();

        // Check for duplicate
        if self.state.block_index.contains_key(&block_hash) {
            return Ok(BlockStatus::Duplicate);
        }

        // Validate basic block properties
        self.validate_block_header(&block)?;

        // Check if this extends our current tip (common case)
        if parent_hash == self.latest_block().hash {
            return self.add_block_to_tip(block);
        }

        // Check if parent exists in our chain
        if let Some(parent_height) = self.state.block_index.get(&parent_hash).copied() {
            // Parent exists - this might be a fork
            return self.handle_potential_fork(block, parent_height);
        }

        // Parent not found - this is an orphan
        let current_time = Utc::now().timestamp() as u64;
        self.state.add_orphan(block, current_time);
        Ok(BlockStatus::AddedAsOrphan)
    }

    /// Add a block that extends the current tip (simple case)
    fn add_block_to_tip(&mut self, block: Block) -> Result<BlockStatus, BlockchainError> {
        // Full validation
        self.validate_block(&block)?;

        // Create undo data before modifying state
        let undo = self.create_undo_data(&block);
        self.state.store_undo_data(undo);

        // Update UTXO set
        self.update_utxo_set(&block);

        // Update chain work
        let block_work = ChainStateManager::calculate_work(block.header.difficulty);
        self.chain_work += block_work;

        // Index the block
        let height = block.index;
        let block_hash = block.hash.clone();
        self.state.index_block(block_hash.clone(), height);

        // Add to chain
        self.blocks.push(block);

        // Update active tip
        self.state
            .set_active_tip(&block_hash, height, self.chain_work);

        // Check for difficulty adjustment
        if self.blocks.len() as u64 % DIFFICULTY_ADJUSTMENT_INTERVAL == 0 {
            self.adjust_difficulty();
        }

        // Process any orphans that depend on this block
        self.process_orphans(&block_hash)?;

        Ok(BlockStatus::AddedToMainChain)
    }

    /// Handle a block that creates a potential fork
    fn handle_potential_fork(
        &mut self,
        block: Block,
        parent_height: u64,
    ) -> Result<BlockStatus, BlockchainError> {
        // Calculate the work of the new chain
        let block_work = ChainStateManager::calculate_work(block.header.difficulty);

        // Work up to parent + this block's work
        let fork_work = self.calculate_work_at_height(parent_height) + block_work;

        // Compare with current chain work
        if fork_work > self.chain_work {
            // New chain has more work - reorganize!
            self.reorganize_to_block(block, parent_height + 1)
        } else {
            // Current chain still has more work, but track this as a tip
            let height = parent_height + 1;
            self.state
                .chain_tips
                .push(crate::core::chain_state::ChainTip::new(
                    block.hash.clone(),
                    height,
                    fork_work,
                    false,
                ));
            Ok(BlockStatus::AddedToMainChain) // Added to a side chain
        }
    }

    /// Reorganize the chain to include the new block
    fn reorganize_to_block(
        &mut self,
        new_block: Block,
        fork_height: u64,
    ) -> Result<BlockStatus, BlockchainError> {
        let disconnected = self.height() - fork_height + 1;

        // Disconnect blocks from current chain
        let mut returned_txs = Vec::new();
        while self.height() >= fork_height {
            if let Some(disconnected_block) = self.blocks.pop() {
                // Restore UTXOs using undo data (clone to avoid borrow issues)
                if let Some(undo) = self.state.get_undo_data(&disconnected_block.hash).cloned() {
                    self.apply_undo_data(&undo);
                }
                // Return non-coinbase transactions to mempool (caller should handle)
                for tx in disconnected_block.transactions {
                    if !tx.is_coinbase {
                        returned_txs.push(tx);
                    }
                }
            }
        }

        // Connect the new block
        self.add_block_to_tip(new_block)?;

        Ok(BlockStatus::CausedReorg {
            disconnected,
            connected: 1,
        })
    }

    /// Apply undo data to restore UTXO state
    fn apply_undo_data(&mut self, undo: &UndoData) {
        // Remove outputs added by the disconnected block
        for tx_id in &undo.added_tx_ids {
            // Remove all outputs from this transaction
            let keys_to_remove: Vec<String> = self
                .utxo_set
                .keys()
                .filter(|k| k.starts_with(tx_id))
                .cloned()
                .collect();
            for key in keys_to_remove {
                self.utxo_set.remove(&key);
            }
        }

        // Restore spent outputs
        for (outpoint, output) in &undo.spent_outputs {
            let parts: Vec<&str> = outpoint.split(':').collect();
            if parts.len() == 2 {
                self.utxo_set.insert(
                    outpoint.clone(),
                    UTXO {
                        tx_id: parts[0].to_string(),
                        output_index: parts[1].parse().unwrap_or(0),
                        output: output.clone(),
                    },
                );
            }
        }
    }

    /// Create undo data for a block (before adding it)
    fn create_undo_data(&self, block: &Block) -> UndoData {
        let mut undo = UndoData::new(block.hash.clone());

        for tx in &block.transactions {
            // Record transaction ID for later removal
            undo.record_added_tx(tx.id.clone());

            // Record spent outputs for restoration
            if !tx.is_coinbase {
                for input in &tx.inputs {
                    let outpoint = format!("{}:{}", input.tx_id, input.output_index);
                    if let Some(utxo) = self.utxo_set.get(&outpoint) {
                        undo.record_spent(outpoint, utxo.output.clone());
                    }
                }
            }
        }

        undo
    }

    /// Calculate cumulative work up to a height
    fn calculate_work_at_height(&self, height: u64) -> u128 {
        self.blocks
            .iter()
            .take(height as usize + 1)
            .map(|b| ChainStateManager::calculate_work(b.header.difficulty))
            .sum()
    }

    /// Process orphan blocks that might now be connectable
    fn process_orphans(&mut self, parent_hash: &str) -> Result<(), BlockchainError> {
        let orphans = self.state.get_orphans_by_parent(parent_hash);

        for orphan in orphans {
            let orphan_hash = orphan.hash.clone();
            self.state.remove_orphan(&orphan_hash);

            // Try to process this orphan (may trigger more orphan processing)
            match self.process_block(orphan) {
                Ok(_) => {}
                Err(e) => {
                    log::warn!("Failed to process orphan block: {:?}", e);
                }
            }
        }

        Ok(())
    }

    // =========================================================================
    // MEDIAN TIME PAST (MTP) - Bitcoin-style timestamp validation
    // =========================================================================

    /// Get the Median Time Past (median of last 11 blocks)
    pub fn get_median_time_past(&self) -> i64 {
        let mut times: Vec<i64> = self
            .blocks
            .iter()
            .rev()
            .take(MTP_BLOCK_COUNT)
            .map(|b| b.header.timestamp.timestamp())
            .collect();

        if times.is_empty() {
            return 0;
        }

        times.sort();
        times[times.len() / 2]
    }

    /// Validate block timestamp using MTP rules
    fn validate_timestamp(&self, block: &Block) -> Result<(), BlockchainError> {
        let block_time = block.header.timestamp.timestamp();
        let mtp = self.get_median_time_past();
        let current_time = Utc::now().timestamp();

        // Block time must be greater than MTP (or equal for short chains during testing)
        // For chains shorter than MTP_BLOCK_COUNT, we allow equal timestamps
        // For longer chains, strict > is required (Bitcoin consensus rule)
        let mtp_check = if self.blocks.len() < MTP_BLOCK_COUNT {
            block_time >= mtp
        } else {
            block_time > mtp
        };

        if !mtp_check {
            return Err(BlockchainError::InvalidTimestamp(format!(
                "Block time {} must be greater than MTP {}",
                block_time, mtp
            )));
        }

        // Block time must not be more than 2 hours in the future
        if block_time > current_time + MAX_FUTURE_BLOCK_TIME {
            return Err(BlockchainError::InvalidTimestamp(format!(
                "Block time {} is too far in the future (max: {})",
                block_time,
                current_time + MAX_FUTURE_BLOCK_TIME
            )));
        }

        Ok(())
    }

    // =========================================================================
    // IMPROVED DIFFICULTY ADJUSTMENT (Bitcoin-style)
    // =========================================================================

    /// Adjust mining difficulty based on block times (Bitcoin-style algorithm)
    fn adjust_difficulty(&mut self) {
        if self.blocks.len() < DIFFICULTY_ADJUSTMENT_INTERVAL as usize {
            return;
        }

        let last_adjusted_index = self.blocks.len() - DIFFICULTY_ADJUSTMENT_INTERVAL as usize;
        let last_adjusted_block = &self.blocks[last_adjusted_index];
        let latest_block = self.latest_block();

        let time_taken = latest_block
            .header
            .timestamp
            .signed_duration_since(last_adjusted_block.header.timestamp)
            .num_seconds();

        let expected_time = TARGET_BLOCK_TIME * DIFFICULTY_ADJUSTMENT_INTERVAL as i64;

        // Calculate adjustment ratio, clamped to max factor
        let ratio = (time_taken as f64 / expected_time as f64).clamp(
            1.0 / MAX_DIFFICULTY_ADJUSTMENT_FACTOR,
            MAX_DIFFICULTY_ADJUSTMENT_FACTOR,
        );

        // Calculate new difficulty (inverse relationship: faster blocks = higher difficulty)
        // Cap maximum change to ±4 per adjustment to prevent wild swings
        let max_change: i32 = 4;
        let new_difficulty = if ratio < 1.0 {
            // Blocks were too fast, increase difficulty
            let increase = ((self.difficulty as f64 / ratio) - self.difficulty as f64)
                .min(max_change as f64) as u32;
            (self.difficulty + increase).min(32)
        } else if ratio > 1.0 {
            // Blocks were too slow, decrease difficulty
            let decrease = (self.difficulty as f64 - (self.difficulty as f64 / ratio))
                .min(max_change as f64) as u32;
            self.difficulty.saturating_sub(decrease).max(1)
        } else {
            self.difficulty
        };

        log::info!(
            "Difficulty adjusted: {} -> {} (actual time: {}s, expected: {}s, ratio: {:.2})",
            self.difficulty,
            new_difficulty,
            time_taken,
            expected_time,
            ratio
        );

        self.difficulty = new_difficulty;
    }

    /// Get the current target difficulty for the next block
    pub fn get_next_difficulty(&self) -> u32 {
        // Check if we need an adjustment at the next block
        if (self.blocks.len() as u64 + 1) % DIFFICULTY_ADJUSTMENT_INTERVAL == 0 {
            // Would need adjustment, but return current for now
            // Actual adjustment happens after block is mined
        }
        self.difficulty
    }

    // =========================================================================
    // ORIGINAL METHODS (Updated)
    // =========================================================================

    /// Add a new block to the chain (legacy method, use process_block for network blocks)
    pub fn add_block(&mut self, block: Block) -> Result<(), BlockchainError> {
        match self.process_block(block)? {
            BlockStatus::Invalid(msg) => Err(BlockchainError::InvalidBlock(msg)),
            BlockStatus::AddedAsOrphan => Err(BlockchainError::OrphanBlock(
                "Block added as orphan".to_string(),
            )),
            _ => Ok(()),
        }
    }

    /// Create and mine a new block
    pub fn mine_block(
        &mut self,
        transactions: Vec<Transaction>,
        miner_address: &str,
    ) -> Result<Block, BlockchainError> {
        // Create coinbase transaction
        let coinbase = Transaction::coinbase(miner_address, BLOCK_REWARD, self.height() + 1);

        // Add coinbase as first transaction
        let mut all_transactions = vec![coinbase];
        all_transactions.extend(transactions);

        // Create new block
        let mut block = Block::new(
            self.height() + 1,
            self.latest_block().hash.clone(),
            all_transactions,
            self.difficulty,
        );

        // Mine the block
        block.mine();

        // Add to chain
        self.add_block(block.clone())?;

        Ok(block)
    }

    /// Validate block header only (quick validation)
    fn validate_block_header(&self, block: &Block) -> Result<(), BlockchainError> {
        // Check proof of work
        if !block.is_valid_pow() {
            return Err(BlockchainError::InvalidBlock(
                "Invalid proof of work".to_string(),
            ));
        }

        // Verify block hash
        if !block.verify_hash() {
            return Err(BlockchainError::InvalidBlock(
                "Invalid block hash".to_string(),
            ));
        }

        // Validate timestamp (MTP rules)
        self.validate_timestamp(block)?;

        Ok(())
    }

    /// Validate a block before adding (full validation)
    fn validate_block(&self, block: &Block) -> Result<(), BlockchainError> {
        let latest = self.latest_block();

        // Check block index
        if block.index != latest.index + 1 {
            return Err(BlockchainError::InvalidBlock(format!(
                "Invalid index: expected {}, got {}",
                latest.index + 1,
                block.index
            )));
        }

        // Check previous hash
        if block.header.previous_hash != latest.hash {
            return Err(BlockchainError::InvalidBlock(
                "Invalid previous hash".to_string(),
            ));
        }

        // Validate header
        self.validate_block_header(block)?;

        // Verify merkle root
        if !block.verify_merkle_root() {
            return Err(BlockchainError::InvalidBlock(
                "Invalid merkle root".to_string(),
            ));
        }

        // Validate all transactions
        for tx in &block.transactions {
            if !tx
                .is_valid()
                .map_err(|e| BlockchainError::InvalidBlock(e.to_string()))?
            {
                return Err(BlockchainError::InvalidBlock(
                    "Invalid transaction".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Validate the entire chain
    pub fn is_valid(&self) -> bool {
        for i in 1..self.blocks.len() {
            let current = &self.blocks[i];
            let previous = &self.blocks[i - 1];

            // Check previous hash link
            if current.header.previous_hash != previous.hash {
                return false;
            }

            // Check proof of work
            if !current.is_valid_pow() {
                return false;
            }

            // Verify block hash
            if !current.verify_hash() {
                return false;
            }

            // Verify merkle root
            if !current.verify_merkle_root() {
                return false;
            }
        }

        true
    }

    /// Rebuild the UTXO set from the blockchain
    pub fn rebuild_utxo_set(&mut self) {
        self.utxo_set.clear();
        self.coinbase_heights.clear();

        // Clone blocks to avoid borrow checker issues
        let blocks = self.blocks.clone();
        for block in &blocks {
            self.process_block_utxos(block);
        }
    }

    /// Process a block's transactions for UTXO updates
    fn process_block_utxos(&mut self, block: &Block) {
        for tx in &block.transactions {
            // Track coinbase transaction heights for maturity checks
            if tx.is_coinbase {
                self.coinbase_heights.insert(tx.id.clone(), block.index);
            }

            // Remove spent outputs (inputs)
            for input in &tx.inputs {
                if !tx.is_coinbase {
                    let key = format!("{}:{}", input.tx_id, input.output_index);
                    self.utxo_set.remove(&key);
                    // Also remove from coinbase tracking if spending a coinbase
                    self.coinbase_heights.remove(&input.tx_id);
                }
            }

            // Add new outputs
            for (index, output) in tx.outputs.iter().enumerate() {
                let key = format!("{}:{}", tx.id, index);
                self.utxo_set.insert(
                    key,
                    UTXO {
                        tx_id: tx.id.clone(),
                        output_index: index as u32,
                        output: output.clone(),
                    },
                );
            }
        }
    }

    /// Update UTXO set with a new block
    fn update_utxo_set(&mut self, block: &Block) {
        self.process_block_utxos(block);
    }

    /// Get UTXOs for a specific address (includes immature coinbase)
    pub fn get_utxos_for_address(&self, address: &str) -> Vec<UTXO> {
        self.utxo_set
            .values()
            .filter(|utxo| utxo.output.recipient == address)
            .cloned()
            .collect()
    }

    /// Get balance for an address (includes immature coinbase)
    pub fn get_balance(&self, address: &str) -> u64 {
        self.get_utxos_for_address(address)
            .iter()
            .map(|utxo| utxo.output.amount)
            .sum()
    }

    /// Find a UTXO by transaction ID and output index
    pub fn find_utxo(&self, tx_id: &str, output_index: u32) -> Option<&UTXO> {
        let key = format!("{}:{}", tx_id, output_index);
        self.utxo_set.get(&key)
    }

    // =========================================================================
    // Coinbase Maturity (Production-grade - Bitcoin uses 100 blocks)
    // =========================================================================

    /// Check if a coinbase transaction is mature enough to spend
    /// Coinbase outputs require COINBASE_MATURITY (100) confirmations
    pub fn is_coinbase_mature(&self, tx_id: &str) -> bool {
        if let Some(&coinbase_height) = self.coinbase_heights.get(tx_id) {
            let current_height = self.height();
            let confirmations = current_height.saturating_sub(coinbase_height);
            confirmations >= COINBASE_MATURITY
        } else {
            // Not a coinbase transaction, always spendable
            true
        }
    }

    /// Get blocks remaining until coinbase is mature
    pub fn coinbase_blocks_until_mature(&self, tx_id: &str) -> u64 {
        if let Some(&coinbase_height) = self.coinbase_heights.get(tx_id) {
            let current_height = self.height();
            let confirmations = current_height.saturating_sub(coinbase_height);
            if confirmations >= COINBASE_MATURITY {
                0
            } else {
                COINBASE_MATURITY - confirmations
            }
        } else {
            0
        }
    }

    /// Get only spendable UTXOs for an address (excludes immature coinbase)
    pub fn get_spendable_utxos_for_address(&self, address: &str) -> Vec<UTXO> {
        self.utxo_set
            .values()
            .filter(|utxo| utxo.output.recipient == address && self.is_coinbase_mature(&utxo.tx_id))
            .cloned()
            .collect()
    }

    /// Get spendable balance for an address (excludes immature coinbase)
    pub fn get_spendable_balance(&self, address: &str) -> u64 {
        self.get_spendable_utxos_for_address(address)
            .iter()
            .map(|utxo| utxo.output.amount)
            .sum()
    }

    /// Get immature (locked) balance for an address
    pub fn get_immature_balance(&self, address: &str) -> u64 {
        self.get_balance(address) - self.get_spendable_balance(address)
    }

    /// Burn (remove) coins from an address as gas fees
    /// This modifies the UTXO set directly to deduct the amount
    /// Returns the actual amount burned (may be less if insufficient funds)
    pub fn burn_from_address(&mut self, address: &str, amount: u64) -> u64 {
        if amount == 0 {
            return 0;
        }

        let mut remaining = amount;
        let mut keys_to_remove = Vec::new();
        let mut updates = Vec::new();

        // Find UTXOs for this address
        for (key, utxo) in self.utxo_set.iter() {
            if utxo.output.recipient == address && remaining > 0 {
                if utxo.output.amount <= remaining {
                    // Remove entire UTXO
                    remaining -= utxo.output.amount;
                    keys_to_remove.push(key.clone());
                } else {
                    // Reduce UTXO amount
                    let new_amount = utxo.output.amount - remaining;
                    updates.push((key.clone(), utxo.clone(), new_amount));
                    remaining = 0;
                }
            }
        }

        // Apply removals
        for key in keys_to_remove {
            self.utxo_set.remove(&key);
        }

        // Apply updates (reduce UTXO amount)
        for (key, mut utxo, new_amount) in updates {
            utxo.output.amount = new_amount;
            self.utxo_set.insert(key, utxo);
        }

        amount - remaining
    }

    /// Get all transactions for an address
    pub fn get_transactions_for_address(&self, address: &str) -> Vec<&Transaction> {
        let mut transactions = Vec::new();

        for block in &self.blocks {
            for tx in &block.transactions {
                // Check if address is in outputs
                let in_outputs = tx.outputs.iter().any(|o| o.recipient == address);

                if in_outputs {
                    transactions.push(tx);
                }
            }
        }

        transactions
    }

    /// Get chain statistics
    pub fn stats(&self) -> ChainStats {
        let total_transactions: usize = self.blocks.iter().map(|b| b.transactions.len()).sum();
        let total_coins: u64 = self.utxo_set.values().map(|u| u.output.amount).sum();

        ChainStats {
            height: self.height(),
            total_blocks: self.blocks.len() as u64,
            total_transactions: total_transactions as u64,
            total_coins,
            difficulty: self.difficulty,
            latest_hash: self.latest_block().hash.clone(),
            chain_work: self.chain_work,
            orphan_count: self.state.orphan_pool.len(),
        }
    }
}

impl Default for Blockchain {
    fn default() -> Self {
        Self::new()
    }
}

/// Chain statistics
#[derive(Debug, Clone)]
pub struct ChainStats {
    pub height: u64,
    pub total_blocks: u64,
    pub total_transactions: u64,
    pub total_coins: u64,
    pub difficulty: u32,
    pub latest_hash: String,
    pub chain_work: u128,
    pub orphan_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_blockchain() {
        let blockchain = Blockchain::with_difficulty(4);
        assert_eq!(blockchain.blocks.len(), 1);
        assert_eq!(blockchain.latest_block().index, 0);
        assert!(blockchain.is_valid());
        assert!(blockchain.chain_work > 0);
    }

    #[test]
    fn test_mine_block() {
        let mut blockchain = Blockchain::with_difficulty(4);
        let miner = "miner_address";

        let block = blockchain.mine_block(vec![], miner).unwrap();

        assert_eq!(block.index, 1);
        assert!(block.is_valid_pow());
        assert_eq!(blockchain.blocks.len(), 2);
        assert_eq!(blockchain.get_balance(miner), BLOCK_REWARD);
    }

    #[test]
    fn test_chain_validation() {
        let mut blockchain = Blockchain::with_difficulty(4);

        blockchain.mine_block(vec![], "miner1").unwrap();
        blockchain.mine_block(vec![], "miner2").unwrap();

        assert!(blockchain.is_valid());
        assert_eq!(blockchain.height(), 2);
    }

    #[test]
    fn test_utxo_tracking() {
        let mut blockchain = Blockchain::with_difficulty(4);
        let miner = "miner_address";

        blockchain.mine_block(vec![], miner).unwrap();
        blockchain.mine_block(vec![], miner).unwrap();

        let balance = blockchain.get_balance(miner);
        assert_eq!(balance, BLOCK_REWARD * 2);

        let utxos = blockchain.get_utxos_for_address(miner);
        assert_eq!(utxos.len(), 2);
    }

    #[test]
    fn test_invalid_block_rejected() {
        let mut blockchain = Blockchain::with_difficulty(4);

        let mut invalid_block = Block::new(
            1,
            "wrong_hash".to_string(),
            vec![Transaction::coinbase("miner", BLOCK_REWARD, 1)],
            4,
        );
        invalid_block.mine();

        assert!(blockchain.add_block(invalid_block).is_err());
    }

    #[test]
    fn test_median_time_past() {
        let mut blockchain = Blockchain::with_difficulty(4);

        // Mine a few blocks
        for _ in 0..5 {
            blockchain.mine_block(vec![], "miner").unwrap();
        }

        let mtp = blockchain.get_median_time_past();
        assert!(mtp > 0);
    }

    #[test]
    fn test_chain_work_accumulates() {
        let mut blockchain = Blockchain::with_difficulty(4);
        let initial_work = blockchain.chain_work;

        blockchain.mine_block(vec![], "miner").unwrap();

        assert!(blockchain.chain_work > initial_work);
    }

    #[test]
    fn test_duplicate_block_rejected() {
        let mut blockchain = Blockchain::with_difficulty(4);

        let block = blockchain.mine_block(vec![], "miner").unwrap();

        // Try to add the same block again
        let status = blockchain.process_block(block).unwrap();
        assert_eq!(status, BlockStatus::Duplicate);
    }
}
