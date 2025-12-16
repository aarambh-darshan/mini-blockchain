//! Block and Transaction Indexing
//!
//! Provides efficient lookup of blocks and transactions by various keys:
//! - Block by hash
//! - Block by height
//! - Transaction by ID
//! - Transactions by address

use crate::core::{Block, Transaction};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// Block Index Entry
// =============================================================================

/// Information about a stored block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockIndexEntry {
    /// Block hash
    pub hash: String,
    /// Block height
    pub height: u64,
    /// File offset (for future use with file-based storage)
    pub file_offset: u64,
    /// Block size in bytes
    pub size: u32,
    /// Number of transactions in the block
    pub tx_count: u32,
    /// Cumulative work up to this block
    pub cumulative_work: u128,
    /// Block timestamp
    pub timestamp: i64,
    /// Whether this block is on the main chain
    pub on_main_chain: bool,
    /// Previous block hash
    pub prev_hash: String,
}

impl BlockIndexEntry {
    pub fn from_block(block: &Block, cumulative_work: u128, on_main_chain: bool) -> Self {
        Self {
            hash: block.hash.clone(),
            height: block.index,
            file_offset: 0, // Not used yet
            size: 0,        // Would calculate serialized size
            tx_count: block.transactions.len() as u32,
            cumulative_work,
            timestamp: block.header.timestamp.timestamp(),
            on_main_chain,
            prev_hash: block.header.previous_hash.clone(),
        }
    }
}

// =============================================================================
// Transaction Index Entry
// =============================================================================

/// Information about a stored transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxIndexEntry {
    /// Transaction ID
    pub tx_id: String,
    /// Block hash containing this transaction
    pub block_hash: String,
    /// Block height
    pub block_height: u64,
    /// Position in block
    pub tx_index: u32,
    /// Is coinbase transaction
    pub is_coinbase: bool,
}

impl TxIndexEntry {
    pub fn from_tx(tx: &Transaction, block_hash: &str, block_height: u64, tx_index: u32) -> Self {
        Self {
            tx_id: tx.id.clone(),
            block_hash: block_hash.to_string(),
            block_height,
            tx_index,
            is_coinbase: tx.is_coinbase,
        }
    }
}

// =============================================================================
// Address Index Entry
// =============================================================================

/// Information about transactions involving an address
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressIndexEntry {
    /// Transaction ID
    pub tx_id: String,
    /// Block height
    pub block_height: u64,
    /// Whether this is a receive (output) or send (input)
    pub is_receive: bool,
    /// Amount involved
    pub amount: u64,
}

// =============================================================================
// Block Index
// =============================================================================

/// Index for efficient block lookups
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct BlockIndex {
    /// Blocks by hash
    by_hash: HashMap<String, BlockIndexEntry>,
    /// Block hash by height (for main chain only)
    by_height: HashMap<u64, String>,
    /// Best chain tip hash
    pub best_block: Option<String>,
    /// Best chain height
    pub best_height: u64,
}

impl BlockIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a block to the index
    pub fn add_block(&mut self, entry: BlockIndexEntry) {
        let hash = entry.hash.clone();
        let height = entry.height;
        let on_main_chain = entry.on_main_chain;

        self.by_hash.insert(hash.clone(), entry);

        if on_main_chain {
            self.by_height.insert(height, hash.clone());
            if height >= self.best_height {
                self.best_height = height;
                self.best_block = Some(hash);
            }
        }
    }

    /// Get block entry by hash
    pub fn get_by_hash(&self, hash: &str) -> Option<&BlockIndexEntry> {
        self.by_hash.get(hash)
    }

    /// Get block hash by height (main chain only)
    pub fn get_by_height(&self, height: u64) -> Option<&str> {
        self.by_height.get(&height).map(|s| s.as_str())
    }

    /// Check if block exists
    pub fn contains(&self, hash: &str) -> bool {
        self.by_hash.contains_key(hash)
    }

    /// Get total indexed blocks
    pub fn len(&self) -> usize {
        self.by_hash.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.by_hash.is_empty()
    }

    /// Mark a block as on/off the main chain
    pub fn set_on_main_chain(&mut self, hash: &str, on_main_chain: bool) {
        if let Some(entry) = self.by_hash.get_mut(hash) {
            entry.on_main_chain = on_main_chain;
            if on_main_chain {
                self.by_height.insert(entry.height, hash.to_string());
            } else {
                // Remove from height index if going off main chain
                if let Some(current) = self.by_height.get(&entry.height) {
                    if current == hash {
                        self.by_height.remove(&entry.height);
                    }
                }
            }
        }
    }

    /// Get statistics
    pub fn stats(&self) -> BlockIndexStats {
        let main_chain_count = self.by_hash.values().filter(|e| e.on_main_chain).count();
        BlockIndexStats {
            total_blocks: self.by_hash.len(),
            main_chain_blocks: main_chain_count,
            best_height: self.best_height,
        }
    }
}

/// Block index statistics
#[derive(Debug, Clone)]
pub struct BlockIndexStats {
    pub total_blocks: usize,
    pub main_chain_blocks: usize,
    pub best_height: u64,
}

// =============================================================================
// Transaction Index
// =============================================================================

/// Index for efficient transaction lookups
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TxIndex {
    /// Transactions by ID
    by_id: HashMap<String, TxIndexEntry>,
    /// Transaction IDs by address
    by_address: HashMap<String, Vec<AddressIndexEntry>>,
}

impl TxIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a transaction to the index
    pub fn add_transaction(
        &mut self,
        tx: &Transaction,
        block_hash: &str,
        block_height: u64,
        tx_index: u32,
    ) {
        let entry = TxIndexEntry::from_tx(tx, block_hash, block_height, tx_index);
        self.by_id.insert(tx.id.clone(), entry);

        // Index by address (outputs = receives)
        for output in &tx.outputs {
            let addr_entry = AddressIndexEntry {
                tx_id: tx.id.clone(),
                block_height,
                is_receive: true,
                amount: output.amount,
            };
            self.by_address
                .entry(output.recipient.clone())
                .or_default()
                .push(addr_entry);
        }
    }

    /// Get transaction entry by ID
    pub fn get_by_id(&self, tx_id: &str) -> Option<&TxIndexEntry> {
        self.by_id.get(tx_id)
    }

    /// Get transactions for an address
    pub fn get_by_address(&self, address: &str) -> Vec<&AddressIndexEntry> {
        self.by_address
            .get(address)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    /// Check if transaction exists
    pub fn contains(&self, tx_id: &str) -> bool {
        self.by_id.contains_key(tx_id)
    }

    /// Get total indexed transactions
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// Remove transactions from a block (for reorgs)
    pub fn remove_block_transactions(&mut self, block_hash: &str) {
        let tx_ids: Vec<String> = self
            .by_id
            .iter()
            .filter(|(_, e)| e.block_hash == block_hash)
            .map(|(id, _)| id.clone())
            .collect();

        for tx_id in tx_ids {
            self.by_id.remove(&tx_id);
        }

        // Clean up address index (less efficient, but handles reorgs)
        for entries in self.by_address.values_mut() {
            entries.retain(|e| self.by_id.contains_key(&e.tx_id));
        }
    }

    /// Get statistics
    pub fn stats(&self) -> TxIndexStats {
        let address_count = self.by_address.len();
        TxIndexStats {
            total_transactions: self.by_id.len(),
            indexed_addresses: address_count,
        }
    }
}

/// Transaction index statistics
#[derive(Debug, Clone)]
pub struct TxIndexStats {
    pub total_transactions: usize,
    pub indexed_addresses: usize,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Blockchain;

    #[test]
    fn test_block_index() {
        let mut index = BlockIndex::new();

        let blockchain = Blockchain::with_difficulty(4);
        let block = blockchain.latest_block();
        let entry = BlockIndexEntry::from_block(block, 1, true);

        index.add_block(entry);

        assert!(index.contains(&block.hash));
        assert_eq!(index.get_by_height(0), Some(block.hash.as_str()));
        assert_eq!(index.len(), 1);
    }

    #[test]
    fn test_tx_index() {
        let mut index = TxIndex::new();
        let tx = crate::core::Transaction::coinbase("recipient", 50, 1);

        index.add_transaction(&tx, "block_hash", 1, 0);

        assert!(index.contains(&tx.id));
        let addr_txs = index.get_by_address("recipient");
        assert_eq!(addr_txs.len(), 1);
        assert_eq!(addr_txs[0].amount, 50);
    }
}
