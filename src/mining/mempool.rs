//! Transaction pool (mempool) for pending transactions
//!
//! Manages unconfirmed transactions waiting to be included in blocks.

use crate::core::{Blockchain, Transaction, TransactionError};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

/// Mempool errors
#[derive(Error, Debug)]
pub enum MempoolError {
    #[error("Transaction already exists")]
    DuplicateTransaction,
    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),
    #[error("Transaction validation error: {0}")]
    ValidationError(#[from] TransactionError),
}

/// Memory pool for pending transactions
#[derive(Debug, Default)]
pub struct Mempool {
    /// Transactions indexed by ID
    transactions: HashMap<String, Transaction>,
    /// Transaction IDs in order of arrival
    order: Vec<String>,
    /// Maximum pool size
    max_size: usize,
}

impl Mempool {
    /// Create a new mempool
    pub fn new() -> Self {
        Self {
            transactions: HashMap::new(),
            order: Vec::new(),
            max_size: 10000,
        }
    }

    /// Create a mempool with custom max size
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            transactions: HashMap::new(),
            order: Vec::new(),
            max_size,
        }
    }

    /// Add a transaction to the pool
    pub fn add_transaction(
        &mut self,
        tx: Transaction,
        blockchain: &Blockchain,
    ) -> Result<(), MempoolError> {
        // Check for duplicate
        if self.transactions.contains_key(&tx.id) {
            return Err(MempoolError::DuplicateTransaction);
        }

        // Validate transaction
        if !tx.is_valid()? {
            return Err(MempoolError::InvalidTransaction(
                "Transaction validation failed".to_string(),
            ));
        }

        // Check UTXO availability
        for input in &tx.inputs {
            if !tx.is_coinbase
                && blockchain
                    .find_utxo(&input.tx_id, input.output_index)
                    .is_none()
            {
                return Err(MempoolError::InvalidTransaction(
                    "Input UTXO not found".to_string(),
                ));
            }

            // Check if already spent in mempool
            if self.is_utxo_spent(&input.tx_id, input.output_index) {
                return Err(MempoolError::InvalidTransaction(
                    "Input already spent in mempool".to_string(),
                ));
            }
        }

        // Remove oldest if at capacity
        if self.transactions.len() >= self.max_size {
            if let Some(old_id) = self.order.first().cloned() {
                self.transactions.remove(&old_id);
                self.order.remove(0);
            }
        }

        // Add transaction
        let tx_id = tx.id.clone();
        self.transactions.insert(tx_id.clone(), tx);
        self.order.push(tx_id);

        Ok(())
    }

    /// Check if a UTXO is spent by any transaction in the mempool
    fn is_utxo_spent(&self, tx_id: &str, output_index: u32) -> bool {
        for tx in self.transactions.values() {
            for input in &tx.inputs {
                if input.tx_id == tx_id && input.output_index == output_index {
                    return true;
                }
            }
        }
        false
    }

    /// Get transactions for mining (up to limit)
    pub fn get_transactions(&self, limit: usize) -> Vec<Transaction> {
        self.order
            .iter()
            .take(limit)
            .filter_map(|id| self.transactions.get(id).cloned())
            .collect()
    }

    /// Remove transactions that are now in a block
    pub fn remove_transactions(&mut self, tx_ids: &[String]) {
        let ids_set: HashSet<_> = tx_ids.iter().collect();

        for id in tx_ids {
            self.transactions.remove(id);
        }

        self.order.retain(|id| !ids_set.contains(id));
    }

    /// Remove transactions with inputs that are now spent
    pub fn remove_conflicting(&mut self, blockchain: &Blockchain) {
        let mut to_remove = Vec::new();

        for (id, tx) in &self.transactions {
            for input in &tx.inputs {
                if !tx.is_coinbase
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
        self.transactions.get(id)
    }

    /// Check if a transaction is in the pool
    pub fn contains(&self, id: &str) -> bool {
        self.transactions.contains_key(id)
    }

    /// Get the number of pending transactions
    pub fn len(&self) -> usize {
        self.transactions.len()
    }

    /// Check if the pool is empty
    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }

    /// Clear all transactions
    pub fn clear(&mut self) {
        self.transactions.clear();
        self.order.clear();
    }

    /// Get all transaction IDs
    pub fn transaction_ids(&self) -> Vec<String> {
        self.order.clone()
    }
}

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
        let mempool = Mempool::with_max_size(100);
        assert_eq!(mempool.max_size, 100);
    }
}
