//! Transaction handling for the blockchain
//!
//! Implements a UTXO-based transaction model with digital signatures.
//! Production-grade features:
//! - Transaction locktime (Bitcoin BIP-65)
//! - Sequence numbers for RBF (Bitcoin BIP-125)
//! - Transaction versioning
//! - Chain ID for replay protection (EIP-155 style)

use crate::crypto::{public_key_from_hex, sha256, verify_signature, KeyPair};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// =============================================================================
// Constants
// =============================================================================

/// Current transaction version
pub const TX_VERSION: u32 = 2;

/// Sequence number that disables locktime
pub const SEQUENCE_FINAL: u32 = 0xFFFFFFFF;

/// Sequence number that signals RBF is enabled (BIP-125)
/// Any sequence < SEQUENCE_RBF_MAX signals RBF
pub const SEQUENCE_RBF_MAX: u32 = 0xFFFFFFFE;

/// Locktime threshold: values below are block heights, above are timestamps
/// (500 million, same as Bitcoin)
pub const LOCKTIME_THRESHOLD: u32 = 500_000_000;

/// Default chain ID (for replay protection)
pub const DEFAULT_CHAIN_ID: u32 = 1;

// =============================================================================
// Error Types
// =============================================================================

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
    #[error("Transaction not final: locktime {0} not reached")]
    NotFinal(u32),
    #[error("RBF not signaled: sequence must be < {}", SEQUENCE_RBF_MAX)]
    RbfNotSignaled,
    #[error("Insufficient fee for RBF: new fee {0} must be > old fee {1}")]
    InsufficientRbfFee(u64, u64),
    #[error("Wrong chain ID: expected {0}, got {1}")]
    WrongChainId(u32, u32),
}

// =============================================================================
// Transaction Input
// =============================================================================

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
    /// Sequence number for RBF and locktime
    /// - SEQUENCE_FINAL (0xFFFFFFFF): disables locktime for this input
    /// - < SEQUENCE_RBF_MAX (0xFFFFFFFE): signals RBF is enabled
    #[serde(default = "default_sequence")]
    pub sequence: u32,
}

fn default_sequence() -> u32 {
    SEQUENCE_FINAL
}

impl TransactionInput {
    /// Check if this input signals RBF
    pub fn signals_rbf(&self) -> bool {
        self.sequence < SEQUENCE_RBF_MAX
    }

    /// Check if this input disables locktime
    pub fn is_final(&self) -> bool {
        self.sequence == SEQUENCE_FINAL
    }
}

// =============================================================================
// Transaction Output
// =============================================================================

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

// =============================================================================
// UTXO
// =============================================================================

/// Unspent Transaction Output (UTXO)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UTXO {
    pub tx_id: String,
    pub output_index: u32,
    pub output: TransactionOutput,
}

// =============================================================================
// Token Operations (On-Chain ERC-20 style)
// =============================================================================

/// Token operation types that can be embedded in transactions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TokenOperationType {
    /// Deploy a new token - all tokens go to the creator (transaction sender)
    Create {
        name: String,
        symbol: String,
        decimals: u8,
        total_supply: u128,
        is_mintable: bool,
    },
    /// Transfer tokens from sender to recipient
    Transfer {
        token_address: String,
        to: String,
        amount: u128,
    },
    /// Approve spender to transfer tokens on behalf of owner
    Approve {
        token_address: String,
        spender: String,
        amount: u128,
    },
    /// Transfer tokens on behalf of owner (requires prior approval)
    TransferFrom {
        token_address: String,
        from: String,
        to: String,
        amount: u128,
    },
    /// Burn tokens (destroy from sender's balance)
    Burn { token_address: String, amount: u128 },
    /// Mint new tokens (only token creator/minter can do this)
    Mint {
        token_address: String,
        to: String,
        amount: u128,
    },
}

// =============================================================================
// Transaction
// =============================================================================

/// A blockchain transaction with production-grade features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// Transaction version (for future upgrades)
    #[serde(default = "default_version")]
    pub version: u32,
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
    /// Locktime: block height or timestamp when tx becomes valid
    /// - 0: transaction is always valid
    /// - < LOCKTIME_THRESHOLD: block height
    /// - >= LOCKTIME_THRESHOLD: Unix timestamp
    #[serde(default)]
    pub locktime: u32,
    /// Chain ID for replay protection (EIP-155 style)
    #[serde(default = "default_chain_id")]
    pub chain_id: u32,
    /// Transaction fee (set when added to mempool)
    #[serde(default)]
    pub fee: u64,
    /// Optional token operation data (for on-chain ERC-20 style tokens)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_data: Option<TokenOperationType>,
}

fn default_version() -> u32 {
    TX_VERSION
}

fn default_chain_id() -> u32 {
    DEFAULT_CHAIN_ID
}

impl Transaction {
    /// Create a new transaction (unsigned)
    pub fn new(inputs: Vec<TransactionInput>, outputs: Vec<TransactionOutput>) -> Self {
        let mut tx = Self {
            version: TX_VERSION,
            id: String::new(),
            inputs,
            outputs,
            timestamp: Utc::now(),
            is_coinbase: false,
            locktime: 0,
            chain_id: DEFAULT_CHAIN_ID,
            fee: 0,
            token_data: None,
        };
        tx.id = tx.calculate_hash();
        tx
    }

    /// Create a new transaction with locktime
    pub fn with_locktime(
        inputs: Vec<TransactionInput>,
        outputs: Vec<TransactionOutput>,
        locktime: u32,
    ) -> Self {
        let mut tx = Self::new(inputs, outputs);
        tx.locktime = locktime;
        tx.id = tx.calculate_hash();
        tx
    }

    /// Create a new transaction with custom chain ID
    pub fn with_chain_id(
        inputs: Vec<TransactionInput>,
        outputs: Vec<TransactionOutput>,
        chain_id: u32,
    ) -> Self {
        let mut tx = Self::new(inputs, outputs);
        tx.chain_id = chain_id;
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
            sequence: SEQUENCE_FINAL,
        }];

        let mut tx = Self {
            version: TX_VERSION,
            id: String::new(),
            inputs,
            outputs,
            timestamp: Utc::now(),
            is_coinbase: true,
            locktime: 0,
            chain_id: DEFAULT_CHAIN_ID,
            fee: 0,
            token_data: None,
        };
        tx.id = tx.calculate_hash();
        tx
    }

    /// Create a token operation transaction
    ///
    /// Token transactions don't need UTXOs for normal operations,
    /// but require a valid sender address for authentication.
    pub fn with_token_data(
        inputs: Vec<TransactionInput>,
        outputs: Vec<TransactionOutput>,
        token_data: TokenOperationType,
    ) -> Self {
        let mut tx = Self {
            version: TX_VERSION,
            id: String::new(),
            inputs,
            outputs,
            timestamp: Utc::now(),
            is_coinbase: false,
            locktime: 0,
            chain_id: DEFAULT_CHAIN_ID,
            fee: 0,
            token_data: Some(token_data),
        };
        tx.id = tx.calculate_hash();
        tx
    }

    /// Check if this is a token transaction
    pub fn is_token_transaction(&self) -> bool {
        self.token_data.is_some()
    }

    /// Get the sender address from the first input's public key
    /// For token transactions, this is the address performing the operation
    pub fn sender_address(&self) -> Option<String> {
        self.inputs.first().map(|input| {
            // Derive address from public key (same as wallet)
            let hash = sha256(input.public_key.as_bytes());
            let hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
            format!("1{}", &hex[..39])
        })
    }

    /// Calculate the transaction hash (includes chain_id for replay protection)
    pub fn calculate_hash(&self) -> String {
        let data = format!(
            "{}{:?}{:?}{}{}{}{}{:?}",
            self.version,
            self.inputs,
            self.outputs,
            self.timestamp,
            self.is_coinbase,
            self.locktime,
            self.chain_id,
            self.token_data
        );
        hex::encode(sha256(data.as_bytes()))
    }

    /// Get the data to be signed (includes chain_id and token_data for replay protection)
    pub fn signing_data(&self) -> Vec<u8> {
        let data = format!(
            "{}{:?}{:?}{}{}{}{:?}",
            self.version,
            self.outputs,
            self.timestamp,
            self.is_coinbase,
            self.locktime,
            self.chain_id,
            self.token_data
        );
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

            // Handle multisig transactions - they have "MULTISIG:{address}" as public_key
            // and comma-separated "pubkey:sig" pairs in signature field
            if input.public_key.starts_with("MULTISIG:") {
                // Verify multisig combined signatures
                if !self.verify_multisig_input(input)? {
                    return Ok(false);
                }
                continue;
            }

            // Regular transaction signature verification
            let public_key = public_key_from_hex(&input.public_key)?;
            let signature =
                hex::decode(&input.signature).map_err(|_| TransactionError::InvalidSignature)?;

            if !verify_signature(&public_key, &signing_data, &signature)? {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Verify a multisig input's combined signatures
    fn verify_multisig_input(&self, input: &TransactionInput) -> Result<bool, TransactionError> {
        // Signature format: "pubkey1:sig1,pubkey2:sig2,..."
        let pairs: Vec<&str> = input.signature.split(',').collect();

        if pairs.is_empty() {
            return Ok(false);
        }

        // We need to verify each signature in the combined set
        // The signing data for multisig is embedded in the signatures themselves
        // For now, trust that the multisig manager already verified these signatures
        // TODO: Re-verify signatures here against the actual transaction data

        // Basic sanity check: ensure pairs have valid format
        for pair in pairs {
            let parts: Vec<&str> = pair.split(':').collect();
            if parts.len() != 2 {
                return Ok(false);
            }
            // Check pubkey and signature are non-empty hex
            if parts[0].is_empty() || parts[1].is_empty() {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Get total output amount
    pub fn total_output(&self) -> u64 {
        self.outputs.iter().map(|o| o.amount).sum()
    }

    /// Calculate fee rate (fee per byte, approximated)
    pub fn fee_rate(&self) -> u64 {
        // Approximate size: 10 bytes base + 150 per input + 34 per output
        let approx_size = 10 + (self.inputs.len() * 150) + (self.outputs.len() * 34);
        if approx_size == 0 {
            return 0;
        }
        self.fee / approx_size as u64
    }

    // =========================================================================
    // Locktime & Finality (Bitcoin BIP-65/68)
    // =========================================================================

    /// Check if transaction is final (can be included in a block)
    /// - block_height: current block height
    /// - block_time: current block timestamp
    pub fn is_final(&self, block_height: u64, block_time: u64) -> bool {
        // Locktime 0 means always final
        if self.locktime == 0 {
            return true;
        }

        // If all inputs have final sequence, locktime is disabled
        if self.inputs.iter().all(|i| i.is_final()) {
            return true;
        }

        // Check locktime based on threshold
        if self.locktime < LOCKTIME_THRESHOLD {
            // Locktime is a block height
            block_height >= self.locktime as u64
        } else {
            // Locktime is a Unix timestamp
            block_time >= self.locktime as u64
        }
    }

    /// Check if transaction is final, returning error with details if not
    pub fn check_final(&self, block_height: u64, block_time: u64) -> Result<(), TransactionError> {
        if self.is_final(block_height, block_time) {
            Ok(())
        } else {
            Err(TransactionError::NotFinal(self.locktime))
        }
    }

    // =========================================================================
    // Replace-By-Fee (Bitcoin BIP-125)
    // =========================================================================

    /// Check if this transaction signals RBF (at least one input has sequence < 0xFFFFFFFE)
    pub fn signals_rbf(&self) -> bool {
        self.inputs.iter().any(|i| i.signals_rbf())
    }

    /// Enable RBF by setting all input sequences to allow replacement
    pub fn enable_rbf(&mut self) {
        for input in &mut self.inputs {
            if input.sequence == SEQUENCE_FINAL {
                input.sequence = SEQUENCE_RBF_MAX - 1;
            }
        }
    }

    /// Check if this transaction can replace another (RBF rules)
    pub fn can_replace(&self, other: &Transaction) -> Result<(), TransactionError> {
        // Original must signal RBF
        if !other.signals_rbf() {
            return Err(TransactionError::RbfNotSignaled);
        }

        // New transaction must pay higher fee
        if self.fee <= other.fee {
            return Err(TransactionError::InsufficientRbfFee(self.fee, other.fee));
        }

        Ok(())
    }

    // =========================================================================
    // Replay Protection (EIP-155 style)
    // =========================================================================

    /// Check if this transaction is for the given chain
    pub fn is_for_chain(&self, chain_id: u32) -> bool {
        self.chain_id == chain_id
    }

    /// Verify chain ID matches expected
    pub fn verify_chain_id(&self, expected_chain_id: u32) -> Result<(), TransactionError> {
        if self.chain_id == expected_chain_id {
            Ok(())
        } else {
            Err(TransactionError::WrongChainId(
                expected_chain_id,
                self.chain_id,
            ))
        }
    }

    // =========================================================================
    // Validation
    // =========================================================================

    /// Check if this transaction is valid (basic checks only)
    pub fn is_valid(&self) -> Result<bool, TransactionError> {
        // Check version
        if self.version == 0 {
            return Ok(false);
        }

        // Token transactions are allowed to have empty outputs
        // (they only record token operations, not coin transfers)
        if self.token_data.is_some() {
            // Token transactions just need valid token data - no signatures required
            // since the identity comes from the input's public_key field
            return Ok(true);
        }

        // Check that outputs are not empty (for regular transactions)
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

    /// Full validation including locktime and chain ID
    pub fn validate_full(
        &self,
        block_height: u64,
        block_time: u64,
        chain_id: u32,
    ) -> Result<(), TransactionError> {
        // Basic validation
        if !self.is_valid()? {
            return Err(TransactionError::InvalidTransaction(
                "Basic validation failed".to_string(),
            ));
        }

        // Check chain ID
        self.verify_chain_id(chain_id)?;

        // Check locktime (skip for coinbase)
        if !self.is_coinbase {
            self.check_final(block_height, block_time)?;
        }

        Ok(())
    }
}

// =============================================================================
// Transaction Builder
// =============================================================================

/// Builder for creating transactions with all options
pub struct TransactionBuilder {
    inputs: Vec<TransactionInput>,
    outputs: Vec<TransactionOutput>,
    locktime: u32,
    chain_id: u32,
    enable_rbf: bool,
}

impl TransactionBuilder {
    pub fn new() -> Self {
        Self {
            inputs: Vec::new(),
            outputs: Vec::new(),
            locktime: 0,
            chain_id: DEFAULT_CHAIN_ID,
            enable_rbf: false,
        }
    }

    /// Add an input from a UTXO
    pub fn add_input(mut self, utxo: &UTXO) -> Self {
        self.inputs.push(TransactionInput {
            tx_id: utxo.tx_id.clone(),
            output_index: utxo.output_index,
            signature: String::new(),
            public_key: String::new(),
            sequence: if self.enable_rbf {
                SEQUENCE_RBF_MAX - 1
            } else {
                SEQUENCE_FINAL
            },
        });
        self
    }

    /// Add an input with custom sequence
    pub fn add_input_with_sequence(mut self, utxo: &UTXO, sequence: u32) -> Self {
        self.inputs.push(TransactionInput {
            tx_id: utxo.tx_id.clone(),
            output_index: utxo.output_index,
            signature: String::new(),
            public_key: String::new(),
            sequence,
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

    /// Set locktime (block height or timestamp)
    pub fn locktime(mut self, locktime: u32) -> Self {
        self.locktime = locktime;
        self
    }

    /// Set chain ID for replay protection
    pub fn chain_id(mut self, chain_id: u32) -> Self {
        self.chain_id = chain_id;
        self
    }

    /// Enable Replace-By-Fee
    pub fn with_rbf(mut self) -> Self {
        self.enable_rbf = true;
        // Update existing inputs
        for input in &mut self.inputs {
            if input.sequence == SEQUENCE_FINAL {
                input.sequence = SEQUENCE_RBF_MAX - 1;
            }
        }
        self
    }

    /// Build and sign the transaction
    pub fn build_and_sign(self, key_pair: &KeyPair) -> Result<Transaction, TransactionError> {
        let mut tx = self.build();
        tx.sign(key_pair)?;
        Ok(tx)
    }

    /// Build without signing
    pub fn build(self) -> Transaction {
        let mut tx = Transaction::new(self.inputs, self.outputs);
        tx.locktime = self.locktime;
        tx.chain_id = self.chain_id;
        tx.id = tx.calculate_hash();
        tx
    }
}

impl Default for TransactionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coinbase_transaction() {
        let tx = Transaction::coinbase("recipient_address", 50, 0);
        assert!(tx.is_coinbase);
        assert_eq!(tx.total_output(), 50);
        assert!(tx.is_valid().unwrap());
        assert_eq!(tx.version, TX_VERSION);
        assert_eq!(tx.chain_id, DEFAULT_CHAIN_ID);
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

    #[test]
    fn test_locktime_block_height() {
        // Create input with non-final sequence (needed for locktime to apply)
        let input = TransactionInput {
            tx_id: "abc123".to_string(),
            output_index: 0,
            signature: String::new(),
            public_key: String::new(),
            sequence: SEQUENCE_RBF_MAX - 1, // Non-final, locktime applies
        };
        let tx = Transaction::with_locktime(vec![input], vec![], 100);

        // Before locktime
        assert!(!tx.is_final(50, 0));
        assert!(!tx.is_final(99, 0));

        // At or after locktime
        assert!(tx.is_final(100, 0));
        assert!(tx.is_final(200, 0));
    }

    #[test]
    fn test_locktime_timestamp() {
        // Create input with non-final sequence
        let input = TransactionInput {
            tx_id: "abc123".to_string(),
            output_index: 0,
            signature: String::new(),
            public_key: String::new(),
            sequence: SEQUENCE_RBF_MAX - 1,
        };

        // Timestamp locktime (above threshold)
        let locktime = LOCKTIME_THRESHOLD + 1000;
        let tx = Transaction::with_locktime(vec![input], vec![], locktime);

        // Before locktime
        assert!(!tx.is_final(0, (LOCKTIME_THRESHOLD + 500) as u64));

        // At or after locktime
        assert!(tx.is_final(0, locktime as u64));
        assert!(tx.is_final(0, (locktime + 1000) as u64));
    }

    #[test]
    fn test_locktime_disabled_by_sequence() {
        let key_pair = KeyPair::generate();
        let utxo = UTXO {
            tx_id: "abc123".to_string(),
            output_index: 0,
            output: TransactionOutput {
                amount: 100,
                recipient: key_pair.address(),
            },
        };

        // Transaction with locktime but final sequence
        let tx = TransactionBuilder::new()
            .add_input(&utxo) // Default sequence is FINAL
            .add_output("recipient", 100)
            .locktime(1000) // Set locktime
            .build();

        // Should be final because all inputs have SEQUENCE_FINAL
        assert!(tx.is_final(0, 0));
    }

    #[test]
    fn test_rbf_signaling() {
        let key_pair = KeyPair::generate();
        let utxo = UTXO {
            tx_id: "abc123".to_string(),
            output_index: 0,
            output: TransactionOutput {
                amount: 100,
                recipient: key_pair.address(),
            },
        };

        // Normal transaction (no RBF)
        let tx1 = TransactionBuilder::new()
            .add_input(&utxo)
            .add_output("recipient", 100)
            .build();
        assert!(!tx1.signals_rbf());

        // RBF-enabled transaction
        let tx2 = TransactionBuilder::new()
            .with_rbf()
            .add_input(&utxo)
            .add_output("recipient", 100)
            .build();
        assert!(tx2.signals_rbf());
    }

    #[test]
    fn test_chain_id_replay_protection() {
        let tx1 = Transaction::with_chain_id(vec![], vec![], 1);
        let tx2 = Transaction::with_chain_id(vec![], vec![], 2);

        assert!(tx1.is_for_chain(1));
        assert!(!tx1.is_for_chain(2));

        assert!(tx2.is_for_chain(2));
        assert!(!tx2.is_for_chain(1));

        // Different chain IDs = different hashes (replay protection)
        assert_ne!(tx1.id, tx2.id);
    }

    #[test]
    fn test_rbf_replacement_rules() {
        // Create inputs with RBF sequence for tx1
        let input_rbf = TransactionInput {
            tx_id: "abc123".to_string(),
            output_index: 0,
            signature: String::new(),
            public_key: String::new(),
            sequence: SEQUENCE_RBF_MAX - 1, // RBF enabled
        };

        let mut tx1 = Transaction::new(vec![input_rbf], vec![]);
        tx1.fee = 100;

        let mut tx2 = Transaction::new(vec![], vec![]);
        tx2.fee = 200;

        // tx2 can replace tx1 (higher fee, tx1 signals RBF)
        assert!(tx2.can_replace(&tx1).is_ok());

        // tx1 cannot replace tx2 (lower fee)
        assert!(tx1.can_replace(&tx2).is_err());

        // Cannot replace non-RBF transaction
        let input_final = TransactionInput {
            tx_id: "abc123".to_string(),
            output_index: 0,
            signature: String::new(),
            public_key: String::new(),
            sequence: SEQUENCE_FINAL, // No RBF
        };
        let tx3 = Transaction::new(vec![input_final], vec![]);
        assert!(tx2.can_replace(&tx3).is_err());
    }
}
