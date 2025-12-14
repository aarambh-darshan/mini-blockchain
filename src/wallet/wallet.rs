//! Wallet implementation for the blockchain
//!
//! Provides key management and transaction creation.

use crate::core::{Blockchain, Transaction, TransactionBuilder, TransactionError, UTXO};
use crate::crypto::KeyPair;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use thiserror::Error;

/// Wallet-related errors
#[derive(Error, Debug)]
pub enum WalletError {
    #[error("Insufficient funds: have {have}, need {need}")]
    InsufficientFunds { have: u64, need: u64 },
    #[error("Transaction error: {0}")]
    TransactionError(#[from] TransactionError),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("Crypto error: {0}")]
    CryptoError(#[from] crate::crypto::KeyError),
}

/// Serializable wallet data for persistence
#[derive(Debug, Serialize, Deserialize)]
struct WalletData {
    private_key_hex: String,
    address: String,
    label: Option<String>,
}

/// A blockchain wallet for managing keys and creating transactions
pub struct Wallet {
    /// The key pair for signing transactions
    key_pair: KeyPair,
    /// Optional label for the wallet
    pub label: Option<String>,
}

impl Wallet {
    /// Create a new wallet with a fresh key pair
    pub fn new() -> Self {
        Self {
            key_pair: KeyPair::generate(),
            label: None,
        }
    }

    /// Create a wallet with a label
    pub fn with_label(label: &str) -> Self {
        Self {
            key_pair: KeyPair::generate(),
            label: Some(label.to_string()),
        }
    }

    /// Import a wallet from a private key
    pub fn from_private_key(private_key_hex: &str) -> Result<Self, WalletError> {
        let key_pair = KeyPair::from_private_key_hex(private_key_hex)?;
        Ok(Self {
            key_pair,
            label: None,
        })
    }

    /// Get the wallet's address
    pub fn address(&self) -> String {
        self.key_pair.address()
    }

    /// Get the wallet's public key (hex)
    pub fn public_key(&self) -> String {
        self.key_pair.public_key_hex()
    }

    /// Get the wallet's private key (hex)
    /// WARNING: Keep this secret!
    pub fn private_key(&self) -> String {
        self.key_pair.private_key_hex()
    }

    /// Get the balance from the blockchain
    pub fn balance(&self, blockchain: &Blockchain) -> u64 {
        blockchain.get_balance(&self.address())
    }

    /// Get UTXOs owned by this wallet
    pub fn utxos(&self, blockchain: &Blockchain) -> Vec<UTXO> {
        blockchain.get_utxos_for_address(&self.address())
    }

    /// Create a transaction to send funds
    pub fn create_transaction(
        &self,
        recipient: &str,
        amount: u64,
        blockchain: &Blockchain,
    ) -> Result<Transaction, WalletError> {
        let utxos = self.utxos(blockchain);
        let balance: u64 = utxos.iter().map(|u| u.output.amount).sum();

        if balance < amount {
            return Err(WalletError::InsufficientFunds {
                have: balance,
                need: amount,
            });
        }

        // Select UTXOs to cover the amount
        let mut selected_utxos = Vec::new();
        let mut selected_amount = 0u64;

        for utxo in utxos {
            selected_utxos.push(utxo.clone());
            selected_amount += utxo.output.amount;

            if selected_amount >= amount {
                break;
            }
        }

        // Build transaction
        let mut builder = TransactionBuilder::new();

        for utxo in &selected_utxos {
            builder = builder.add_input(utxo);
        }

        // Output to recipient
        builder = builder.add_output(recipient, amount);

        // Change back to self
        let change = selected_amount - amount;
        if change > 0 {
            builder = builder.add_output(&self.address(), change);
        }

        // Build and sign
        let tx = builder.build_and_sign(&self.key_pair)?;
        Ok(tx)
    }

    /// Save wallet to file
    pub fn save(&self, path: &Path) -> Result<(), WalletError> {
        let data = WalletData {
            private_key_hex: self.private_key(),
            address: self.address(),
            label: self.label.clone(),
        };

        let json = serde_json::to_string_pretty(&data)?;
        fs::write(path, json)?;
        Ok(())
    }

    /// Load wallet from file
    pub fn load(path: &Path) -> Result<Self, WalletError> {
        let json = fs::read_to_string(path)?;
        let data: WalletData = serde_json::from_str(&json)?;

        let mut wallet = Self::from_private_key(&data.private_key_hex)?;
        wallet.label = data.label;
        Ok(wallet)
    }

    /// Export wallet info (without private key)
    pub fn export_public_info(&self) -> WalletInfo {
        WalletInfo {
            address: self.address(),
            public_key: self.public_key(),
            label: self.label.clone(),
        }
    }
}

impl Default for Wallet {
    fn default() -> Self {
        Self::new()
    }
}

/// Public wallet information (safe to share)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletInfo {
    pub address: String,
    pub public_key: String,
    pub label: Option<String>,
}

/// Wallet manager for handling multiple wallets
pub struct WalletManager {
    wallets_dir: std::path::PathBuf,
}

impl WalletManager {
    /// Create a new wallet manager
    pub fn new(wallets_dir: &Path) -> Result<Self, WalletError> {
        fs::create_dir_all(wallets_dir)?;
        Ok(Self {
            wallets_dir: wallets_dir.to_path_buf(),
        })
    }

    /// Create and save a new wallet
    pub fn create_wallet(&self, label: Option<&str>) -> Result<Wallet, WalletError> {
        let wallet = match label {
            Some(l) => Wallet::with_label(l),
            None => Wallet::new(),
        };

        let filename = format!("{}.json", wallet.address());
        let path = self.wallets_dir.join(filename);
        wallet.save(&path)?;

        Ok(wallet)
    }

    /// List all wallet addresses
    pub fn list_wallets(&self) -> Result<Vec<String>, WalletError> {
        let mut addresses = Vec::new();

        for entry in fs::read_dir(&self.wallets_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(wallet) = Wallet::load(&path) {
                    addresses.push(wallet.address());
                }
            }
        }

        Ok(addresses)
    }

    /// Load a specific wallet by address
    pub fn load_wallet(&self, address: &str) -> Result<Wallet, WalletError> {
        let filename = format!("{}.json", address);
        let path = self.wallets_dir.join(filename);
        Wallet::load(&path)
    }

    /// Delete a wallet
    pub fn delete_wallet(&self, address: &str) -> Result<(), WalletError> {
        let filename = format!("{}.json", address);
        let path = self.wallets_dir.join(filename);
        fs::remove_file(path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wallet_creation() {
        let wallet = Wallet::new();
        assert!(!wallet.address().is_empty());
        assert!(!wallet.public_key().is_empty());
        assert!(!wallet.private_key().is_empty());
    }

    #[test]
    fn test_wallet_import() {
        let wallet1 = Wallet::new();
        let private_key = wallet1.private_key();

        let wallet2 = Wallet::from_private_key(&private_key).unwrap();
        assert_eq!(wallet1.address(), wallet2.address());
    }

    #[test]
    fn test_wallet_save_load() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("test_wallet.json");

        let wallet1 = Wallet::with_label("Test Wallet");
        wallet1.save(&path).unwrap();

        let wallet2 = Wallet::load(&path).unwrap();
        assert_eq!(wallet1.address(), wallet2.address());
        assert_eq!(wallet1.label, wallet2.label);
    }

    #[test]
    fn test_transaction_creation() {
        let mut blockchain = crate::core::Blockchain::with_difficulty(4);
        let wallet = Wallet::new();

        // Mine some blocks to get funds
        blockchain.mine_block(vec![], &wallet.address()).unwrap();
        blockchain.mine_block(vec![], &wallet.address()).unwrap();

        let balance = wallet.balance(&blockchain);
        assert!(balance > 0);

        // Create a transaction
        let recipient = Wallet::new().address();
        let tx = wallet
            .create_transaction(&recipient, 10, &blockchain)
            .unwrap();

        assert!(tx.verify_signatures().unwrap());
    }
}
