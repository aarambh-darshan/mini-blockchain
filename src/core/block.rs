//! Block implementation for the blockchain
//!
//! A block contains a header with metadata and a list of transactions.

use crate::core::transaction::Transaction;
use crate::crypto::{calculate_merkle_root, double_sha256, meets_difficulty};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Block header containing metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    /// Block version
    pub version: u32,
    /// Hash of the previous block
    pub previous_hash: String,
    /// Merkle root of all transactions
    pub merkle_root: String,
    /// Block creation timestamp
    pub timestamp: DateTime<Utc>,
    /// Difficulty target (number of leading zero bits required)
    pub difficulty: u32,
    /// Nonce used for proof of work
    pub nonce: u64,
}

impl BlockHeader {
    /// Calculate the hash of the block header
    pub fn hash(&self) -> String {
        let data = format!(
            "{}{}{}{}{}{}",
            self.version,
            self.previous_hash,
            self.merkle_root,
            self.timestamp.timestamp(),
            self.difficulty,
            self.nonce
        );
        hex::encode(double_sha256(data.as_bytes()))
    }

    /// Check if the hash meets the difficulty target
    pub fn is_valid_hash(&self) -> bool {
        let hash = hex::decode(self.hash()).unwrap_or_default();
        meets_difficulty(&hash, self.difficulty)
    }
}

/// A block in the blockchain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    /// Block index/height
    pub index: u64,
    /// Block header
    pub header: BlockHeader,
    /// Block hash (cached for efficiency)
    pub hash: String,
    /// List of transactions in the block
    pub transactions: Vec<Transaction>,
}

impl Block {
    /// Create a new block (unmined)
    pub fn new(
        index: u64,
        previous_hash: String,
        transactions: Vec<Transaction>,
        difficulty: u32,
    ) -> Self {
        let merkle_root = Self::calculate_merkle_root(&transactions);

        let header = BlockHeader {
            version: 1,
            previous_hash,
            merkle_root,
            timestamp: Utc::now(),
            difficulty,
            nonce: 0,
        };

        let hash = header.hash();

        Self {
            index,
            header,
            hash,
            transactions,
        }
    }

    /// Create the genesis block
    pub fn genesis(difficulty: u32) -> Self {
        let coinbase = Transaction::coinbase("genesis", 0, 0);

        let merkle_root = Self::calculate_merkle_root(&[coinbase.clone()]);

        let header = BlockHeader {
            version: 1,
            previous_hash: "0".repeat(64),
            merkle_root,
            timestamp: Utc::now(),
            difficulty,
            nonce: 0,
        };

        let mut block = Self {
            index: 0,
            header,
            hash: String::new(),
            transactions: vec![coinbase],
        };

        // Mine the genesis block
        block.mine();
        block
    }

    /// Calculate the merkle root from transactions
    fn calculate_merkle_root(transactions: &[Transaction]) -> String {
        let tx_hashes: Vec<Vec<u8>> = transactions
            .iter()
            .map(|tx| hex::decode(&tx.id).unwrap_or_default())
            .collect();

        hex::encode(calculate_merkle_root(&tx_hashes))
    }

    /// Mine the block (find a valid nonce)
    pub fn mine(&mut self) -> u64 {
        let mut attempts = 0u64;

        loop {
            self.header.nonce = attempts;
            self.hash = self.header.hash();

            if self.is_valid_pow() {
                return attempts;
            }

            attempts += 1;

            // Prevent infinite loop in case of very high difficulty
            if attempts == u64::MAX {
                break;
            }
        }

        attempts
    }

    /// Check if the proof of work is valid
    pub fn is_valid_pow(&self) -> bool {
        let hash_bytes = hex::decode(&self.hash).unwrap_or_default();
        meets_difficulty(&hash_bytes, self.header.difficulty)
    }

    /// Verify the block's merkle root
    pub fn verify_merkle_root(&self) -> bool {
        let calculated = Self::calculate_merkle_root(&self.transactions);
        calculated == self.header.merkle_root
    }

    /// Verify the block hash
    pub fn verify_hash(&self) -> bool {
        self.hash == self.header.hash()
    }

    /// Get the total transaction fees in this block
    pub fn total_fees(&self) -> u64 {
        // In a full implementation, this would calculate input - output
        // For simplicity, we return 0
        0
    }

    /// Get the coinbase transaction (first transaction)
    pub fn coinbase_tx(&self) -> Option<&Transaction> {
        self.transactions.first().filter(|tx| tx.is_coinbase)
    }

    /// Get the mining reward from this block
    pub fn mining_reward(&self) -> u64 {
        self.coinbase_tx().map(|tx| tx.total_output()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genesis_block() {
        let genesis = Block::genesis(8); // Low difficulty for testing
        assert_eq!(genesis.index, 0);
        assert!(genesis.is_valid_pow());
        assert_eq!(genesis.header.previous_hash, "0".repeat(64));
    }

    #[test]
    fn test_block_mining() {
        let transactions = vec![Transaction::coinbase("miner", 50, 1)];
        let mut block = Block::new(1, "0".repeat(64), transactions, 8);

        block.mine();

        assert!(block.is_valid_pow());
        assert!(block.verify_merkle_root());
        assert!(block.verify_hash());
    }

    #[test]
    fn test_merkle_root_verification() {
        let transactions = vec![Transaction::coinbase("addr1", 50, 1)];
        let mut block = Block::new(1, "0".repeat(64), transactions, 4);
        block.mine();

        assert!(block.verify_merkle_root());

        // Tamper with the transaction ID (this is what gets hashed into merkle root)
        block.transactions[0].id = "tampered_id".to_string();
        assert!(!block.verify_merkle_root());
    }

    #[test]
    fn test_block_hash_verification() {
        let mut block = Block::genesis(4);
        assert!(block.verify_hash());

        // Tamper with nonce
        block.header.nonce += 1;
        assert!(!block.verify_hash());
    }
}
