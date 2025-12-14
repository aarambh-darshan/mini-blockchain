//! Blockchain implementation
//!
//! The main blockchain struct that manages the chain of blocks.

use crate::core::block::Block;
use crate::core::transaction::{Transaction, UTXO};
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
}

/// The main blockchain structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blockchain {
    /// The chain of blocks
    pub blocks: Vec<Block>,
    /// Current mining difficulty
    pub difficulty: u32,
    /// Unspent transaction outputs
    #[serde(skip)]
    pub utxo_set: HashMap<String, UTXO>,
}

impl Blockchain {
    /// Create a new blockchain with genesis block
    pub fn new() -> Self {
        let genesis = Block::genesis(DEFAULT_DIFFICULTY);
        let mut blockchain = Self {
            blocks: vec![genesis],
            difficulty: DEFAULT_DIFFICULTY,
            utxo_set: HashMap::new(),
        };
        blockchain.rebuild_utxo_set();
        blockchain
    }

    /// Create a blockchain with custom difficulty
    pub fn with_difficulty(difficulty: u32) -> Self {
        let genesis = Block::genesis(difficulty);
        let mut blockchain = Self {
            blocks: vec![genesis],
            difficulty,
            utxo_set: HashMap::new(),
        };
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

    /// Add a new block to the chain
    pub fn add_block(&mut self, block: Block) -> Result<(), BlockchainError> {
        // Validate the block
        self.validate_block(&block)?;

        // Update UTXO set
        self.update_utxo_set(&block);

        // Add to chain
        self.blocks.push(block);

        // Check if we need to adjust difficulty
        if self.blocks.len() as u64 % DIFFICULTY_ADJUSTMENT_INTERVAL == 0 {
            self.adjust_difficulty();
        }

        Ok(())
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

    /// Validate a block before adding
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

        // Check proof of work
        if !block.is_valid_pow() {
            return Err(BlockchainError::InvalidBlock(
                "Invalid proof of work".to_string(),
            ));
        }

        // Verify merkle root
        if !block.verify_merkle_root() {
            return Err(BlockchainError::InvalidBlock(
                "Invalid merkle root".to_string(),
            ));
        }

        // Verify block hash
        if !block.verify_hash() {
            return Err(BlockchainError::InvalidBlock(
                "Invalid block hash".to_string(),
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

    /// Adjust mining difficulty based on block times
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

        // Adjust difficulty
        if time_taken < expected_time / 2 {
            // Blocks too fast, increase difficulty
            self.difficulty = self.difficulty.saturating_add(1).min(32);
        } else if time_taken > expected_time * 2 {
            // Blocks too slow, decrease difficulty
            self.difficulty = self.difficulty.saturating_sub(1).max(1);
        }

        log::info!(
            "Difficulty adjusted from {} to {} (time taken: {}s, expected: {}s)",
            self.blocks.last().unwrap().header.difficulty,
            self.difficulty,
            time_taken,
            expected_time
        );
    }

    /// Rebuild the UTXO set from the blockchain
    pub fn rebuild_utxo_set(&mut self) {
        self.utxo_set.clear();

        // Clone blocks to avoid borrow checker issues
        let blocks = self.blocks.clone();
        for block in &blocks {
            self.process_block_utxos(block);
        }
    }

    /// Process a block's transactions for UTXO updates
    fn process_block_utxos(&mut self, block: &Block) {
        for tx in &block.transactions {
            // Remove spent outputs (inputs)
            for input in &tx.inputs {
                if !tx.is_coinbase {
                    let key = format!("{}:{}", input.tx_id, input.output_index);
                    self.utxo_set.remove(&key);
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

    /// Get UTXOs for a specific address
    pub fn get_utxos_for_address(&self, address: &str) -> Vec<UTXO> {
        self.utxo_set
            .values()
            .filter(|utxo| utxo.output.recipient == address)
            .cloned()
            .collect()
    }

    /// Get balance for an address
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
}
