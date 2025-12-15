//! Multi-signature wallet and transaction manager
//!
//! Handles persistence and coordination of multisig wallets and pending transactions.

use crate::core::{Blockchain, TransactionOutput, UTXO};
use crate::multisig::transaction::PendingMultisigTx;
use crate::multisig::wallet::{MultisigConfig, MultisigError, MultisigWallet};
use crate::multisig::MultisigSignature;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Manager for multisig wallets and pending transactions
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MultisigManager {
    /// Multisig wallets by address
    wallets: HashMap<String, MultisigWallet>,
    /// Pending transactions by ID
    pending: HashMap<String, PendingMultisigTx>,
}

impl MultisigManager {
    /// Create a new empty manager
    pub fn new() -> Self {
        Self {
            wallets: HashMap::new(),
            pending: HashMap::new(),
        }
    }

    /// Create a new multisig wallet
    pub fn create_wallet(
        &mut self,
        config: MultisigConfig,
    ) -> Result<MultisigWallet, MultisigError> {
        let wallet = MultisigWallet::new(config)?;

        // Check for duplicate
        if self.wallets.contains_key(&wallet.address) {
            // Same config produces same address, just return existing
            return Ok(self.wallets.get(&wallet.address).unwrap().clone());
        }

        self.wallets.insert(wallet.address.clone(), wallet.clone());
        Ok(wallet)
    }

    /// Get a wallet by address
    pub fn get_wallet(&self, address: &str) -> Option<&MultisigWallet> {
        self.wallets.get(address)
    }

    /// List all multisig wallets
    pub fn list_wallets(&self) -> Vec<&MultisigWallet> {
        self.wallets.values().collect()
    }

    /// Get wallet count
    pub fn wallet_count(&self) -> usize {
        self.wallets.len()
    }

    /// Check if an address is a multisig address
    pub fn is_multisig_address(&self, address: &str) -> bool {
        self.wallets.contains_key(address)
    }

    /// Propose a new transaction from a multisig wallet
    pub fn propose_transaction(
        &mut self,
        from_address: &str,
        to_address: &str,
        amount: u64,
        blockchain: &Blockchain,
    ) -> Result<PendingMultisigTx, MultisigError> {
        // Get the wallet
        let wallet = self
            .wallets
            .get(from_address)
            .ok_or_else(|| MultisigError::WalletNotFound(from_address.to_string()))?;

        // Get UTXOs for this multisig address
        let utxos = blockchain.get_utxos_for_address(from_address);
        let balance: u64 = utxos.iter().map(|u| u.output.amount).sum();

        if balance < amount {
            return Err(MultisigError::TransactionError(
                crate::core::TransactionError::InsufficientFunds,
            ));
        }

        // Select UTXOs (simple: use all available)
        let mut selected_utxos: Vec<UTXO> = Vec::new();
        let mut total_input: u64 = 0;

        for utxo in utxos {
            selected_utxos.push(utxo.clone());
            total_input += utxo.output.amount;
            if total_input >= amount {
                break;
            }
        }

        // Create outputs
        let mut outputs = vec![TransactionOutput {
            amount,
            recipient: to_address.to_string(),
        }];

        // Add change output if needed
        let change = total_input - amount;
        if change > 0 {
            outputs.push(TransactionOutput {
                amount: change,
                recipient: from_address.to_string(),
            });
        }

        // Create pending transaction
        let pending = PendingMultisigTx::new(
            from_address.to_string(),
            to_address.to_string(),
            amount,
            selected_utxos,
            outputs,
            wallet.threshold(),
        );

        self.pending.insert(pending.id.clone(), pending.clone());

        Ok(pending)
    }

    /// Get a pending transaction by ID
    pub fn get_pending(&self, tx_id: &str) -> Option<&PendingMultisigTx> {
        self.pending.get(tx_id)
    }

    /// Get a mutable reference to a pending transaction
    pub fn get_pending_mut(&mut self, tx_id: &str) -> Option<&mut PendingMultisigTx> {
        self.pending.get_mut(tx_id)
    }

    /// Add a signature to a pending transaction
    pub fn sign_transaction(
        &mut self,
        tx_id: &str,
        signature: MultisigSignature,
    ) -> Result<&PendingMultisigTx, MultisigError> {
        // Get the pending transaction
        let pending = self
            .pending
            .get_mut(tx_id)
            .ok_or_else(|| MultisigError::TransactionNotFound(tx_id.to_string()))?;

        // Get the wallet
        let wallet = self
            .wallets
            .get(&pending.from_address)
            .ok_or_else(|| MultisigError::WalletNotFound(pending.from_address.clone()))?
            .clone();

        // Add signature
        pending.add_signature(signature, &wallet)?;

        Ok(self.pending.get(tx_id).unwrap())
    }

    /// List pending transactions for a multisig address
    pub fn pending_for_address(&self, address: &str) -> Vec<&PendingMultisigTx> {
        self.pending
            .values()
            .filter(|tx| tx.from_address == address)
            .collect()
    }

    /// List all pending transactions
    pub fn list_pending(&self) -> Vec<&PendingMultisigTx> {
        self.pending.values().collect()
    }

    /// Get pending transaction count
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Remove a pending transaction (after broadcast or expiry)
    pub fn remove_pending(&mut self, tx_id: &str) -> Option<PendingMultisigTx> {
        self.pending.remove(tx_id)
    }

    /// Get balance for a multisig address
    pub fn get_balance(&self, address: &str, blockchain: &Blockchain) -> Option<u64> {
        if !self.wallets.contains_key(address) {
            return None;
        }
        let utxos = blockchain.get_utxos_for_address(address);
        Some(utxos.iter().map(|u| u.output.amount).sum())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::KeyPair;

    fn create_test_config() -> (MultisigConfig, Vec<KeyPair>) {
        let keys: Vec<KeyPair> = (0..3).map(|_| KeyPair::generate()).collect();
        let pubkeys: Vec<String> = keys.iter().map(|k| k.public_key_hex()).collect();

        let config = MultisigConfig::new(2, pubkeys, Some("Test".to_string())).unwrap();
        (config, keys)
    }

    #[test]
    fn test_manager_creation() {
        let manager = MultisigManager::new();
        assert_eq!(manager.wallet_count(), 0);
        assert_eq!(manager.pending_count(), 0);
    }

    #[test]
    fn test_wallet_creation() {
        let mut manager = MultisigManager::new();
        let (config, _) = create_test_config();

        let wallet = manager.create_wallet(config.clone()).unwrap();
        assert!(wallet.address().starts_with('3'));
        assert_eq!(manager.wallet_count(), 1);

        // Creating with same config returns same wallet
        let wallet2 = manager.create_wallet(config).unwrap();
        assert_eq!(wallet.address(), wallet2.address());
        assert_eq!(manager.wallet_count(), 1);
    }

    #[test]
    fn test_list_wallets() {
        let mut manager = MultisigManager::new();

        let keys1: Vec<KeyPair> = (0..2).map(|_| KeyPair::generate()).collect();
        let keys2: Vec<KeyPair> = (0..2).map(|_| KeyPair::generate()).collect();

        let config1 =
            MultisigConfig::new(2, keys1.iter().map(|k| k.public_key_hex()).collect(), None)
                .unwrap();
        let config2 =
            MultisigConfig::new(2, keys2.iter().map(|k| k.public_key_hex()).collect(), None)
                .unwrap();

        manager.create_wallet(config1).unwrap();
        manager.create_wallet(config2).unwrap();

        assert_eq!(manager.list_wallets().len(), 2);
    }

    #[test]
    fn test_is_multisig_address() {
        let mut manager = MultisigManager::new();
        let (config, _) = create_test_config();

        let wallet = manager.create_wallet(config).unwrap();

        assert!(manager.is_multisig_address(wallet.address()));
        assert!(!manager.is_multisig_address("not_a_multisig"));
    }
}
