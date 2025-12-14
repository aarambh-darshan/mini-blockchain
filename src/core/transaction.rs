//! Transaction handling for the blockchain
//!
//! Implements a UTXO-based transaction model with digital signatures.

use crate::crypto::{public_key_from_hex, sha256, verify_signature, KeyPair};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Transaction-related errors
#[derive(Error, Debug)]
pub enum TransactionError {
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Insufficient funds")]
    InsufficientFunds,
    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),
    #[error("Crypto error: {0}")]
    CryptoError(#[from] crate::crypto::KeyError),
}

/// Transaction input (reference to previous output)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransactionInput {
    /// Transaction ID of the previous transaction
    pub tx_id: String,
    /// Index of the output in the previous transaction
    pub output_index: u32,
    /// Signature proving ownership
    pub signature: String,
    /// Public key of the sender (for verification)
    pub public_key: String,
}

/// Transaction output
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransactionOutput {
    /// Amount of coins
    pub amount: u64,
    /// Recipient's address (hash of public key)
    pub recipient: String,
}

impl TransactionOutput {
    /// Check if this output belongs to the given address
    pub fn is_owned_by(&self, address: &str) -> bool {
        self.recipient == address
    }
}

/// Unspent Transaction Output (UTXO)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UTXO {
    pub tx_id: String,
    pub output_index: u32,
    pub output: TransactionOutput,
}

/// A blockchain transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// Unique transaction ID (hash of transaction data)
    pub id: String,
    /// Transaction inputs
    pub inputs: Vec<TransactionInput>,
    /// Transaction outputs
    pub outputs: Vec<TransactionOutput>,
    /// Timestamp of transaction creation
    pub timestamp: DateTime<Utc>,
    /// Whether this is a coinbase (mining reward) transaction
    pub is_coinbase: bool,
}

impl Transaction {
    /// Create a new transaction (unsigned)
    pub fn new(inputs: Vec<TransactionInput>, outputs: Vec<TransactionOutput>) -> Self {
        let mut tx = Self {
            id: String::new(),
            inputs,
            outputs,
            timestamp: Utc::now(),
            is_coinbase: false,
        };
        tx.id = tx.calculate_hash();
        tx
    }

    /// Create a coinbase (mining reward) transaction
    pub fn coinbase(recipient: &str, amount: u64, block_height: u64) -> Self {
        let outputs = vec![TransactionOutput {
            amount,
            recipient: recipient.to_string(),
        }];

        // Coinbase input contains block height as data
        let inputs = vec![TransactionInput {
            tx_id: "0".repeat(64),
            output_index: block_height as u32,
            signature: String::new(),
            public_key: String::new(),
        }];

        let mut tx = Self {
            id: String::new(),
            inputs,
            outputs,
            timestamp: Utc::now(),
            is_coinbase: true,
        };
        tx.id = tx.calculate_hash();
        tx
    }

    /// Calculate the transaction hash
    pub fn calculate_hash(&self) -> String {
        let data = format!(
            "{:?}{:?}{}{}",
            self.inputs, self.outputs, self.timestamp, self.is_coinbase
        );
        hex::encode(sha256(data.as_bytes()))
    }

    /// Get the data to be signed (excludes signatures)
    pub fn signing_data(&self) -> Vec<u8> {
        let data = format!("{:?}{:?}{}", self.outputs, self.timestamp, self.is_coinbase);
        sha256(data.as_bytes())
    }

    /// Sign all inputs with the provided key pair
    pub fn sign(&mut self, key_pair: &KeyPair) -> Result<(), TransactionError> {
        let signing_data = self.signing_data();
        let public_key_hex = key_pair.public_key_hex();

        for input in &mut self.inputs {
            let signature = key_pair.sign(&signing_data)?;
            input.signature = hex::encode(&signature);
            input.public_key = public_key_hex.clone();
        }

        // Recalculate hash after signing
        self.id = self.calculate_hash();
        Ok(())
    }

    /// Verify all input signatures
    pub fn verify_signatures(&self) -> Result<bool, TransactionError> {
        if self.is_coinbase {
            return Ok(true);
        }

        let signing_data = self.signing_data();

        for input in &self.inputs {
            if input.signature.is_empty() || input.public_key.is_empty() {
                return Ok(false);
            }

            let public_key = public_key_from_hex(&input.public_key)?;
            let signature =
                hex::decode(&input.signature).map_err(|_| TransactionError::InvalidSignature)?;

            if !verify_signature(&public_key, &signing_data, &signature)? {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Get total input amount (requires UTXO lookup - returns 0 for coinbase)
    pub fn total_output(&self) -> u64 {
        self.outputs.iter().map(|o| o.amount).sum()
    }

    /// Check if this transaction is valid
    pub fn is_valid(&self) -> Result<bool, TransactionError> {
        // Check that outputs are not empty
        if self.outputs.is_empty() {
            return Ok(false);
        }

        // Check that all outputs have positive amounts
        for output in &self.outputs {
            if output.amount == 0 {
                return Ok(false);
            }
        }

        // Verify signatures
        self.verify_signatures()
    }
}

/// Builder for creating transactions
pub struct TransactionBuilder {
    inputs: Vec<TransactionInput>,
    outputs: Vec<TransactionOutput>,
}

impl TransactionBuilder {
    pub fn new() -> Self {
        Self {
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }

    /// Add an input from a UTXO
    pub fn add_input(mut self, utxo: &UTXO) -> Self {
        self.inputs.push(TransactionInput {
            tx_id: utxo.tx_id.clone(),
            output_index: utxo.output_index,
            signature: String::new(),
            public_key: String::new(),
        });
        self
    }

    /// Add an output
    pub fn add_output(mut self, recipient: &str, amount: u64) -> Self {
        self.outputs.push(TransactionOutput {
            amount,
            recipient: recipient.to_string(),
        });
        self
    }

    /// Build and sign the transaction
    pub fn build_and_sign(self, key_pair: &KeyPair) -> Result<Transaction, TransactionError> {
        let mut tx = Transaction::new(self.inputs, self.outputs);
        tx.sign(key_pair)?;
        Ok(tx)
    }

    /// Build without signing
    pub fn build(self) -> Transaction {
        Transaction::new(self.inputs, self.outputs)
    }
}

impl Default for TransactionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coinbase_transaction() {
        let tx = Transaction::coinbase("recipient_address", 50, 0);
        assert!(tx.is_coinbase);
        assert_eq!(tx.total_output(), 50);
        assert!(tx.is_valid().unwrap());
    }

    #[test]
    fn test_transaction_signing() {
        let key_pair = KeyPair::generate();

        let utxo = UTXO {
            tx_id: "abc123".to_string(),
            output_index: 0,
            output: TransactionOutput {
                amount: 100,
                recipient: key_pair.address(),
            },
        };

        let tx = TransactionBuilder::new()
            .add_input(&utxo)
            .add_output("recipient", 50)
            .add_output(&key_pair.address(), 50)
            .build_and_sign(&key_pair)
            .unwrap();

        assert!(tx.verify_signatures().unwrap());
        assert!(tx.is_valid().unwrap());
    }

    #[test]
    fn test_transaction_hash() {
        let tx1 = Transaction::coinbase("addr1", 50, 0);
        let tx2 = Transaction::coinbase("addr2", 50, 0);
        assert_ne!(tx1.id, tx2.id);
    }
}
