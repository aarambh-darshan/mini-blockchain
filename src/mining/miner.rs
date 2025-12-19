//! Mining engine for the blockchain
//!
//! Provides block mining and mempool management.

use crate::core::{Block, Blockchain, BlockchainError, Transaction, BLOCK_REWARD};
use log::info;
use std::time::Instant;

/// Mining statistics
#[derive(Debug, Clone)]
pub struct MiningStats {
    /// Number of hash attempts
    pub hash_attempts: u64,
    /// Time taken in milliseconds
    pub time_ms: u128,
    /// Hash rate (hashes per second)
    pub hash_rate: f64,
}

/// Miner for creating new blocks
pub struct Miner {
    /// Miner's address for receiving rewards
    pub address: String,
}

impl Miner {
    /// Create a new miner
    pub fn new(address: &str) -> Self {
        Self {
            address: address.to_string(),
        }
    }

    /// Mine a new block with the given transactions
    pub fn mine_block(
        &self,
        blockchain: &mut Blockchain,
        transactions: Vec<Transaction>,
    ) -> Result<(Block, MiningStats), BlockchainError> {
        let start = Instant::now();

        // Create coinbase transaction
        let coinbase = Transaction::coinbase(&self.address, BLOCK_REWARD, blockchain.height() + 1);

        // Combine coinbase with other transactions
        let mut all_transactions = vec![coinbase];
        all_transactions.extend(transactions);

        // Create new block
        let mut block = Block::new(
            blockchain.height() + 1,
            blockchain.latest_block().hash.clone(),
            all_transactions,
            blockchain.difficulty,
        );

        info!(
            "Mining block {} with difficulty {}...",
            block.index, block.header.difficulty
        );

        // Mine the block
        let attempts = block.mine();

        let elapsed = start.elapsed().as_millis();
        let hash_rate = if elapsed > 0 {
            (attempts as f64) / (elapsed as f64 / 1000.0)
        } else {
            attempts as f64
        };

        let stats = MiningStats {
            hash_attempts: attempts,
            time_ms: elapsed,
            hash_rate,
        };

        info!(
            "Block {} mined in {}ms ({} attempts, {:.2} H/s)",
            block.index, elapsed, attempts, hash_rate
        );

        // Add to blockchain
        blockchain.add_block(block.clone())?;

        Ok((block, stats))
    }

    /// Mine a block without holding blockchain lock (for async/concurrent use)
    ///
    /// This method takes snapshot data from the blockchain, performs CPU-intensive
    /// mining, and returns the mined block. The caller must then add the block
    /// to the chain with a write lock.
    pub fn mine_block_detached(
        &self,
        current_height: u64,
        previous_hash: String,
        difficulty: u32,
        transactions: Vec<Transaction>,
    ) -> (Block, MiningStats) {
        let start = Instant::now();

        // Create coinbase transaction
        let coinbase = Transaction::coinbase(&self.address, BLOCK_REWARD, current_height + 1);

        // Combine coinbase with other transactions
        let mut all_transactions = vec![coinbase];
        all_transactions.extend(transactions);

        // Create new block
        let mut block = Block::new(
            current_height + 1,
            previous_hash,
            all_transactions,
            difficulty,
        );

        info!(
            "Mining block {} with difficulty {}...",
            block.index, block.header.difficulty
        );

        // Mine the block (CPU-intensive)
        let attempts = block.mine();

        let elapsed = start.elapsed().as_millis();
        let hash_rate = if elapsed > 0 {
            (attempts as f64) / (elapsed as f64 / 1000.0)
        } else {
            attempts as f64
        };

        let stats = MiningStats {
            hash_attempts: attempts,
            time_ms: elapsed,
            hash_rate,
        };

        info!(
            "Block {} mined in {}ms ({} attempts, {:.2} H/s)",
            block.index, elapsed, attempts, hash_rate
        );

        (block, stats)
    }

    /// Continuously mine blocks (for testing)
    pub fn mine_continuously(
        &self,
        blockchain: &mut Blockchain,
        num_blocks: u64,
    ) -> Vec<(Block, MiningStats)> {
        let mut results = Vec::new();

        for _ in 0..num_blocks {
            if let Ok(result) = self.mine_block(blockchain, vec![]) {
                results.push(result);
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_miner() {
        let mut blockchain = Blockchain::with_difficulty(4);
        let miner = Miner::new("miner_address");

        let (block, stats) = miner.mine_block(&mut blockchain, vec![]).unwrap();

        assert_eq!(block.index, 1);
        assert!(block.is_valid_pow());
        assert!(stats.hash_attempts > 0);
    }

    #[test]
    fn test_mine_multiple_blocks() {
        let mut blockchain = Blockchain::with_difficulty(4);
        let miner = Miner::new("miner_address");

        let results = miner.mine_continuously(&mut blockchain, 3);

        assert_eq!(results.len(), 3);
        assert_eq!(blockchain.height(), 3);
    }
}
