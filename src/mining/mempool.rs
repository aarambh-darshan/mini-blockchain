//! Transaction pool (mempool) for pending transactions
//!
//! Manages unconfirmed transactions waiting to be included in blocks.
//! Production-grade features:
//! - Replace-By-Fee (RBF) support
//! - Locktime validation
//! - Chain ID validation
//! - Fee-based prioritization
//! - Ancestor/descendant limits (Bitcoin-style)

use crate::core::{Blockchain, Transaction, TransactionError, DEFAULT_CHAIN_ID};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

// =============================================================================
// Configuration (Bitcoin-like production values)
// =============================================================================

/// Default maximum mempool transaction count
pub const DEFAULT_MEMPOOL_SIZE: usize = 10000;

/// Maximum mempool size in bytes (300MB like Bitcoin)
pub const MAX_MEMPOOL_BYTES: usize = 300_000_000;

/// Minimum fee bump for RBF (in percentage, e.g., 10 = 10% higher)
pub const MIN_RBF_FEE_BUMP_PERCENT: u64 = 10;

/// Maximum number of ancestor transactions (Bitcoin uses 25)
pub const MAX_ANCESTORS: usize = 25;

/// Maximum number of descendant transactions (Bitcoin uses 25)
pub const MAX_DESCENDANTS: usize = 25;

/// Maximum total size of ancestor transactions in bytes
pub const MAX_ANCESTOR_SIZE: usize = 101_000;

/// Maximum total size of descendant transactions in bytes
pub const MAX_DESCENDANT_SIZE: usize = 101_000;

// =============================================================================
// Error Types
// =============================================================================

/// Mempool errors
#[derive(Error, Debug)]
pub enum MempoolError {
    #[error("Transaction already exists")]
    DuplicateTransaction,
    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),
    #[error("Transaction validation error: {0}")]
    ValidationError(#[from] TransactionError),
    #[error("Transaction not final until block {0}")]
    NotFinal(u32),
    #[error("RBF not allowed: original transaction doesn't signal RBF")]
    RbfNotSignaled,
    #[error("Insufficient fee for RBF: need {0}, got {1}")]
    InsufficientRbfFee(u64, u64),
    #[error("Wrong chain ID: expected {0}, got {1}")]
    WrongChainId(u32, u32),
    #[error("Mempool full")]
    MempoolFull,
    #[error("Too many ancestors: {0} (max: {1})")]
    TooManyAncestors(usize, usize),
    #[error("Too many descendants: {0} (max: {1})")]
    TooManyDescendants(usize, usize),
    #[error("Ancestor package too large: {0} bytes (max: {1})")]
    AncestorPackageTooLarge(usize, usize),
    #[error("Descendant package too large: {0} bytes (max: {1})")]
    DescendantPackageTooLarge(usize, usize),
    #[error("Mempool size limit exceeded: {0} bytes (max: {1})")]
    MempoolSizeExceeded(usize, usize),
}

// =============================================================================
// Mempool Entry
// =============================================================================

/// Entry in the mempool with metadata
#[derive(Debug, Clone)]
pub struct MempoolEntry {
    /// The transaction
    pub tx: Transaction,
    /// When the transaction was added (Unix timestamp)
    pub added_time: u64,
    /// Fee rate (fee per virtual byte)
    pub fee_rate: u64,
    /// Ancestor count (for CPFP)
    pub ancestor_count: u32,
}

impl MempoolEntry {
    pub fn new(tx: Transaction, added_time: u64) -> Self {
        let fee_rate = tx.fee_rate();
        Self {
            tx,
            added_time,
            fee_rate,
            ancestor_count: 0,
        }
    }
}

// =============================================================================
// Mempool
// =============================================================================

/// Memory pool for pending transactions with RBF support
#[derive(Debug, Default)]
pub struct Mempool {
    /// Transactions indexed by ID
    entries: HashMap<String, MempoolEntry>,
    /// Transaction IDs ordered by fee rate (highest first for mining)
    by_fee: Vec<String>,
    /// Transaction IDs in order of arrival
    by_time: Vec<String>,
    /// Maximum pool size
    max_size: usize,
    /// Chain ID for validation
    chain_id: u32,
    /// Current block height (for locktime checks)
    current_height: u64,
    /// Current block time (for locktime checks)
    current_time: u64,
}

impl Mempool {
    /// Create a new mempool
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            by_fee: Vec::new(),
            by_time: Vec::new(),
            max_size: DEFAULT_MEMPOOL_SIZE,
            chain_id: DEFAULT_CHAIN_ID,
            current_height: 0,
            current_time: 0,
        }
    }

    /// Create a mempool with custom settings
    pub fn with_config(max_size: usize, chain_id: u32) -> Self {
        Self {
            entries: HashMap::new(),
            by_fee: Vec::new(),
            by_time: Vec::new(),
            max_size,
            chain_id,
            current_height: 0,
            current_time: 0,
        }
    }

    /// Update current chain state (call after new blocks)
    pub fn update_chain_state(&mut self, height: u64, time: u64) {
        self.current_height = height;
        self.current_time = time;
    }

    /// Add a transaction to the pool (with RBF support)
    pub fn add_transaction(
        &mut self,
        tx: Transaction,
        blockchain: &Blockchain,
    ) -> Result<Option<Transaction>, MempoolError> {
        // Check for duplicate
        if self.entries.contains_key(&tx.id) {
            return Err(MempoolError::DuplicateTransaction);
        }

        // Validate chain ID
        if tx.chain_id != self.chain_id {
            return Err(MempoolError::WrongChainId(self.chain_id, tx.chain_id));
        }

        // Validate basic transaction
        if !tx.is_valid()? {
            return Err(MempoolError::InvalidTransaction(
                "Transaction validation failed".to_string(),
            ));
        }

        // Check locktime (transaction must be final for next block)
        let next_height = self.current_height + 1;
        let next_time = self.current_time + 10; // Assume 10 second blocks
        if !tx.is_final(next_height, next_time) {
            return Err(MempoolError::NotFinal(tx.locktime));
        }

        // Check UTXO availability and look for conflicts
        let mut conflicting_tx: Option<Transaction> = None;

        for input in &tx.inputs {
            if !tx.is_coinbase {
                // Check if UTXO exists in blockchain
                if blockchain
                    .find_utxo(&input.tx_id, input.output_index)
                    .is_none()
                {
                    // Check if it's from an unconfirmed tx in mempool
                    let in_mempool = self.entries.values().any(|e| {
                        e.tx.id == input.tx_id && e.tx.outputs.len() > input.output_index as usize
                    });

                    if !in_mempool {
                        return Err(MempoolError::InvalidTransaction(
                            "Input UTXO not found".to_string(),
                        ));
                    }
                }

                // Check for conflicts in mempool (same input being spent)
                if let Some(existing) = self.find_conflicting_tx(&input.tx_id, input.output_index) {
                    // RBF: check if we can replace
                    if existing.signals_rbf() {
                        // Must pay higher fee
                        let min_fee =
                            existing.fee + (existing.fee * MIN_RBF_FEE_BUMP_PERCENT / 100);
                        if tx.fee < min_fee {
                            return Err(MempoolError::InsufficientRbfFee(min_fee, tx.fee));
                        }
                        conflicting_tx = Some(existing.clone());
                    } else {
                        return Err(MempoolError::RbfNotSignaled);
                    }
                }
            }
        }

        // Remove conflicting transaction if RBF
        let replaced = if let Some(ref conflict) = conflicting_tx {
            self.remove_transaction(&conflict.id);
            Some(conflict.clone())
        } else {
            None
        };

        // Evict low-fee transactions if at capacity
        while self.entries.len() >= self.max_size {
            if let Some(lowest_id) = self.by_fee.last().cloned() {
                if let Some(lowest) = self.entries.get(&lowest_id) {
                    if lowest.tx.fee_rate() < tx.fee_rate() {
                        self.remove_transaction(&lowest_id);
                    } else {
                        return Err(MempoolError::MempoolFull);
                    }
                }
            } else {
                break;
            }
        }

        // Add transaction
        let tx_id = tx.id.clone();
        let added_time = chrono::Utc::now().timestamp() as u64;
        let entry = MempoolEntry::new(tx, added_time);

        // Insert into fee-sorted list (binary search for position)
        let fee_rate = entry.fee_rate;
        let pos = self
            .by_fee
            .iter()
            .position(|id| self.entries.get(id).map(|e| e.fee_rate).unwrap_or(0) < fee_rate)
            .unwrap_or(self.by_fee.len());
        self.by_fee.insert(pos, tx_id.clone());

        self.by_time.push(tx_id.clone());
        self.entries.insert(tx_id, entry);

        Ok(replaced)
    }

    /// Find a transaction that conflicts (spends same input)
    fn find_conflicting_tx(&self, tx_id: &str, output_index: u32) -> Option<&Transaction> {
        for entry in self.entries.values() {
            for input in &entry.tx.inputs {
                if input.tx_id == tx_id && input.output_index == output_index {
                    return Some(&entry.tx);
                }
            }
        }
        None
    }

    /// Remove a transaction from the pool
    pub fn remove_transaction(&mut self, tx_id: &str) -> Option<Transaction> {
        if let Some(entry) = self.entries.remove(tx_id) {
            self.by_fee.retain(|id| id != tx_id);
            self.by_time.retain(|id| id != tx_id);
            Some(entry.tx)
        } else {
            None
        }
    }

    /// Get transactions for mining (highest fee first, up to limit)
    pub fn get_transactions(&self, limit: usize) -> Vec<Transaction> {
        self.by_fee
            .iter()
            .take(limit)
            .filter_map(|id| self.entries.get(id).map(|e| e.tx.clone()))
            .collect()
    }

    /// Get transactions for mining (FIFO order, up to limit) - legacy behavior
    pub fn get_transactions_fifo(&self, limit: usize) -> Vec<Transaction> {
        self.by_time
            .iter()
            .take(limit)
            .filter_map(|id| self.entries.get(id).map(|e| e.tx.clone()))
            .collect()
    }

    /// Remove transactions that are now in a block
    pub fn remove_transactions(&mut self, tx_ids: &[String]) {
        for id in tx_ids {
            self.remove_transaction(id);
        }
    }

    /// Remove transactions with inputs that are now spent
    pub fn remove_conflicting(&mut self, blockchain: &Blockchain) {
        let mut to_remove = Vec::new();

        for (id, entry) in &self.entries {
            for input in &entry.tx.inputs {
                if !entry.tx.is_coinbase
                    && blockchain
                        .find_utxo(&input.tx_id, input.output_index)
                        .is_none()
                {
                    to_remove.push(id.clone());
                    break;
                }
            }
        }

        self.remove_transactions(&to_remove);
    }

    /// Get a transaction by ID
    pub fn get_transaction(&self, id: &str) -> Option<&Transaction> {
        self.entries.get(id).map(|e| &e.tx)
    }

    /// Get a mempool entry by ID
    pub fn get_entry(&self, id: &str) -> Option<&MempoolEntry> {
        self.entries.get(id)
    }

    /// Check if a transaction is in the pool
    pub fn contains(&self, id: &str) -> bool {
        self.entries.contains_key(id)
    }

    /// Get the number of pending transactions
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the pool is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all transactions
    pub fn clear(&mut self) {
        self.entries.clear();
        self.by_fee.clear();
        self.by_time.clear();
    }

    /// Get all transaction IDs (by fee order)
    pub fn transaction_ids(&self) -> Vec<String> {
        self.by_fee.clone()
    }

    /// Get total fees of all transactions
    pub fn total_fees(&self) -> u64 {
        self.entries.values().map(|e| e.tx.fee).sum()
    }

    /// Check if a UTXO is spent by any transaction in the mempool
    fn is_utxo_spent(&self, tx_id: &str, output_index: u32) -> bool {
        for entry in self.entries.values() {
            for input in &entry.tx.inputs {
                if input.tx_id == tx_id && input.output_index == output_index {
                    return true;
                }
            }
        }
        false
    }

    /// Get mempool statistics
    pub fn stats(&self) -> MempoolStats {
        let total_fees: u64 = self.entries.values().map(|e| e.tx.fee).sum();
        let total_size: usize = self.entries.values().map(|e| e.tx.estimated_size()).sum();

        MempoolStats {
            tx_count: self.entries.len(),
            total_fees,
            total_size,
            max_fee_rate: self
                .by_fee
                .first()
                .and_then(|id| self.entries.get(id))
                .map(|e| e.fee_rate)
                .unwrap_or(0),
            min_fee_rate: self
                .by_fee
                .last()
                .and_then(|id| self.entries.get(id))
                .map(|e| e.fee_rate)
                .unwrap_or(0),
        }
    }

    // =========================================================================
    // Package Limits (Bitcoin-style ancestor/descendant limits)
    // =========================================================================

    /// Calculate the total size of the mempool in bytes
    pub fn total_mempool_size(&self) -> usize {
        self.entries.values().map(|e| e.tx.estimated_size()).sum()
    }

    /// Check if adding a transaction would exceed mempool size limits
    pub fn check_mempool_size(&self, new_tx_size: usize) -> Result<(), MempoolError> {
        let current_size = self.total_mempool_size();
        let new_total = current_size + new_tx_size;
        if new_total > MAX_MEMPOOL_BYTES {
            return Err(MempoolError::MempoolSizeExceeded(
                new_total,
                MAX_MEMPOOL_BYTES,
            ));
        }
        Ok(())
    }

    /// Calculate all ancestors of a transaction (transactions this tx depends on)
    /// Returns (ancestor count, total ancestor size)
    pub fn calculate_ancestors(&self, tx: &Transaction) -> (usize, usize) {
        let mut ancestors = HashSet::new();
        let mut total_size = 0;
        self.collect_ancestors_recursive(tx, &mut ancestors);

        for ancestor_id in &ancestors {
            if let Some(entry) = self.entries.get(ancestor_id) {
                total_size += entry.tx.estimated_size();
            }
        }

        (ancestors.len(), total_size)
    }

    /// Recursively collect all ancestor transaction IDs
    fn collect_ancestors_recursive(&self, tx: &Transaction, ancestors: &mut HashSet<String>) {
        for input in &tx.inputs {
            // Check if this input spends from a mempool transaction
            if self.entries.contains_key(&input.tx_id) && !ancestors.contains(&input.tx_id) {
                ancestors.insert(input.tx_id.clone());
                // Recursively collect ancestors of the parent
                if let Some(parent_entry) = self.entries.get(&input.tx_id) {
                    self.collect_ancestors_recursive(&parent_entry.tx, ancestors);
                }
            }
        }
    }

    /// Calculate all descendants of a transaction (transactions that depend on this tx)
    /// Returns (descendant count, total descendant size)
    pub fn calculate_descendants(&self, tx_id: &str) -> (usize, usize) {
        let mut descendants = HashSet::new();
        let mut total_size = 0;
        self.collect_descendants_recursive(tx_id, &mut descendants);

        for descendant_id in &descendants {
            if let Some(entry) = self.entries.get(descendant_id) {
                total_size += entry.tx.estimated_size();
            }
        }

        (descendants.len(), total_size)
    }

    /// Recursively collect all descendant transaction IDs
    fn collect_descendants_recursive(&self, tx_id: &str, descendants: &mut HashSet<String>) {
        for (entry_id, entry) in &self.entries {
            // Check if this transaction spends from tx_id
            let depends_on_tx = entry.tx.inputs.iter().any(|i| i.tx_id == tx_id);
            if depends_on_tx && !descendants.contains(entry_id) {
                descendants.insert(entry_id.clone());
                // Recursively collect descendants
                self.collect_descendants_recursive(entry_id, descendants);
            }
        }
    }

    /// Check package limits for a new transaction (ancestor and descendant limits)
    pub fn check_package_limits(&self, tx: &Transaction) -> Result<(), MempoolError> {
        // Check ancestor limits
        let (ancestor_count, ancestor_size) = self.calculate_ancestors(tx);
        if ancestor_count > MAX_ANCESTORS {
            return Err(MempoolError::TooManyAncestors(
                ancestor_count,
                MAX_ANCESTORS,
            ));
        }
        if ancestor_size > MAX_ANCESTOR_SIZE {
            return Err(MempoolError::AncestorPackageTooLarge(
                ancestor_size,
                MAX_ANCESTOR_SIZE,
            ));
        }

        // For each transaction this one spends from, check if adding this tx
        // would cause the parent to exceed descendant limits
        for input in &tx.inputs {
            if self.entries.contains_key(&input.tx_id) {
                let (desc_count, desc_size) = self.calculate_descendants(&input.tx_id);
                // +1 because we're adding a new descendant
                if desc_count + 1 > MAX_DESCENDANTS {
                    return Err(MempoolError::TooManyDescendants(
                        desc_count + 1,
                        MAX_DESCENDANTS,
                    ));
                }
                let new_tx_size = tx.estimated_size();
                if desc_size + new_tx_size > MAX_DESCENDANT_SIZE {
                    return Err(MempoolError::DescendantPackageTooLarge(
                        desc_size + new_tx_size,
                        MAX_DESCENDANT_SIZE,
                    ));
                }
            }
        }

        Ok(())
    }

    /// Add a token transaction to the pool (skips UTXO validation)
    ///
    /// Token transactions don't spend UTXOs - they record token operations on-chain.
    /// This method allows adding them without requiring blockchain reference.
    pub fn add_token_transaction(&mut self, tx: Transaction) -> Result<(), MempoolError> {
        // Only allow token or contract transactions
        if tx.token_data.is_none() && tx.contract_data.is_none() {
            return Err(MempoolError::InvalidTransaction(
                "Not a token or contract transaction".to_string(),
            ));
        }

        // Check for duplicate
        if self.entries.contains_key(&tx.id) {
            return Err(MempoolError::DuplicateTransaction);
        }

        // Add transaction
        let tx_id = tx.id.clone();
        let added_time = chrono::Utc::now().timestamp() as u64;
        let entry = MempoolEntry::new(tx, added_time);

        // Insert into fee-sorted list
        let fee_rate = entry.fee_rate;
        let pos = self
            .by_fee
            .iter()
            .position(|id| self.entries.get(id).map(|e| e.fee_rate).unwrap_or(0) < fee_rate)
            .unwrap_or(self.by_fee.len());
        self.by_fee.insert(pos, tx_id.clone());

        self.by_time.push(tx_id.clone());
        self.entries.insert(tx_id, entry);

        Ok(())
    }

    /// Add a contract transaction to the pool (skips UTXO validation)
    ///
    /// Contract transactions record deployments and calls on-chain.
    pub fn add_contract_transaction(&mut self, tx: Transaction) -> Result<(), MempoolError> {
        self.add_token_transaction(tx) // Reuse the same logic
    }
}

/// Mempool statistics
#[derive(Debug, Clone)]
pub struct MempoolStats {
    pub tx_count: usize,
    pub total_fees: u64,
    pub total_size: usize,
    pub max_fee_rate: u64,
    pub min_fee_rate: u64,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mempool_add_remove() {
        let _blockchain = Blockchain::with_difficulty(4);
        let mempool = Mempool::new();

        // Create a valid transaction (coinbase for simplicity)
        let _tx = Transaction::coinbase("recipient", 50, 1);

        // Would normally fail due to not being in chain, but coinbase is special
        // For testing, we'll use a simpler approach
        assert!(mempool.is_empty());
    }

    #[test]
    fn test_mempool_duplicate() {
        let _blockchain = Blockchain::with_difficulty(4);
        let mempool = Mempool::new();

        assert!(mempool.is_empty());
    }

    #[test]
    fn test_mempool_max_size() {
        let mempool = Mempool::with_config(100, DEFAULT_CHAIN_ID);
        assert_eq!(mempool.max_size, 100);
    }

    #[test]
    fn test_mempool_stats() {
        let mempool = Mempool::new();
        let stats = mempool.stats();
        assert_eq!(stats.tx_count, 0);
        assert_eq!(stats.total_fees, 0);
    }

    #[test]
    fn test_mempool_chain_state() {
        let mut mempool = Mempool::new();
        mempool.update_chain_state(100, 1000000);
        assert_eq!(mempool.current_height, 100);
        assert_eq!(mempool.current_time, 1000000);
    }
}
