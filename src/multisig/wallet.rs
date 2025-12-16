//! Multi-signature wallet implementation
//!
//! Provides threshold-based wallets requiring M-of-N signatures.

use crate::crypto::sha256;
use chrono::{DateTime, Utc};
use ripemd::Ripemd160;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use thiserror::Error;

/// Errors related to multisig operations
#[derive(Error, Debug)]
pub enum MultisigError {
    #[error("Invalid threshold: {0}")]
    InvalidThreshold(String),
    #[error("Invalid signer count: need at least 2 signers")]
    InsufficientSigners,
    #[error("Duplicate signer public key")]
    DuplicateSigner,
    #[error("Signer not authorized: {0}")]
    UnauthorizedSigner(String),
    #[error("Already signed by this signer")]
    AlreadySigned,
    #[error("Transaction not found: {0}")]
    TransactionNotFound(String),
    #[error("Wallet not found: {0}")]
    WalletNotFound(String),
    #[error("Insufficient signatures: have {have}, need {need}")]
    InsufficientSignatures { have: usize, need: u8 },
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Transaction error: {0}")]
    TransactionError(#[from] crate::core::TransactionError),
    #[error("Crypto error: {0}")]
    CryptoError(#[from] crate::crypto::KeyError),
}

/// Configuration for a multisig wallet
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MultisigConfig {
    /// Minimum signatures required (M in M-of-N)
    pub threshold: u8,
    /// Public keys of all authorized signers (hex-encoded)
    pub signers: Vec<String>,
    /// Optional human-readable label
    pub label: Option<String>,
}

impl MultisigConfig {
    /// Create a new multisig configuration
    ///
    /// # Arguments
    /// * `threshold` - Minimum signatures required (M)
    /// * `signers` - Public keys of authorized signers (N)
    /// * `label` - Optional label
    ///
    /// # Errors
    /// Returns error if threshold is invalid or signers list is invalid
    pub fn new(
        threshold: u8,
        signers: Vec<String>,
        label: Option<String>,
    ) -> Result<Self, MultisigError> {
        // Validate threshold
        if threshold == 0 {
            return Err(MultisigError::InvalidThreshold(
                "threshold must be at least 1".to_string(),
            ));
        }

        if signers.len() < 2 {
            return Err(MultisigError::InsufficientSigners);
        }

        if threshold as usize > signers.len() {
            return Err(MultisigError::InvalidThreshold(format!(
                "threshold {} exceeds signer count {}",
                threshold,
                signers.len()
            )));
        }

        // Check for duplicates
        let mut sorted_signers = signers.clone();
        sorted_signers.sort();
        for i in 1..sorted_signers.len() {
            if sorted_signers[i] == sorted_signers[i - 1] {
                return Err(MultisigError::DuplicateSigner);
            }
        }

        Ok(Self {
            threshold,
            signers,
            label,
        })
    }

    /// Get the threshold (M)
    pub fn threshold(&self) -> u8 {
        self.threshold
    }

    /// Get the total signer count (N)
    pub fn signer_count(&self) -> usize {
        self.signers.len()
    }

    /// Check if a public key or wallet address is an authorized signer
    ///
    /// Signers can be registered as either addresses or public keys.
    /// This method accepts a public key and its corresponding address to check both.
    pub fn is_signer(&self, pubkey: &str) -> bool {
        self.signers.iter().any(|s| s == pubkey)
    }

    /// Check if authorized by pubkey - also derives address from pubkey and checks that
    pub fn is_signer_with_address_check(&self, pubkey: &str) -> bool {
        // First check direct pubkey match
        if self.signers.iter().any(|s| s == pubkey) {
            return true;
        }

        // Try to derive address from pubkey and check that
        if let Ok(pk) = crate::crypto::public_key_from_hex(pubkey) {
            let address = crate::crypto::public_key_to_address(&pk);
            if self.signers.iter().any(|s| s == &address) {
                return true;
            }
        }

        false
    }

    /// Check if authorized by either pubkey or address
    pub fn is_authorized(&self, pubkey: &str, address: &str) -> bool {
        self.signers.iter().any(|s| s == pubkey || s == address)
    }

    /// Get description like "2-of-3"
    pub fn description(&self) -> String {
        format!("{}-of-{}", self.threshold, self.signers.len())
    }
}

/// A multi-signature wallet
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MultisigWallet {
    /// Unique multisig address (P2SH-style, starts with '3')
    pub address: String,
    /// Wallet configuration
    pub config: MultisigConfig,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

impl MultisigWallet {
    /// Create a new multisig wallet
    pub fn new(config: MultisigConfig) -> Result<Self, MultisigError> {
        let address = Self::generate_address(&config);

        Ok(Self {
            address,
            config,
            created_at: Utc::now(),
        })
    }

    /// Generate P2SH-style address from config
    ///
    /// Address = Base58Check(version || RIPEMD160(SHA256(threshold || sorted_pubkeys)))
    fn generate_address(config: &MultisigConfig) -> String {
        // Sort public keys for deterministic address
        let mut sorted_signers = config.signers.clone();
        sorted_signers.sort();

        // Create redeem script data: threshold + sorted pubkeys
        let mut script_data = vec![config.threshold];
        for pubkey in &sorted_signers {
            script_data.extend_from_slice(pubkey.as_bytes());
        }

        // SHA256 of script data
        let sha256_hash = sha256(&script_data);

        // RIPEMD160 of SHA256 hash
        let mut ripemd = Ripemd160::new();
        ripemd.update(&sha256_hash);
        let ripemd_hash = ripemd.finalize();

        // Add P2SH version byte (0x05 for mainnet -> produces addresses starting with '3')
        let mut address_bytes = vec![0x05];
        address_bytes.extend_from_slice(&ripemd_hash);

        // Calculate checksum (first 4 bytes of double SHA256)
        let checksum = {
            use sha2::Sha256;
            let first_hash = Sha256::digest(&address_bytes);
            let second_hash = Sha256::digest(&first_hash);
            second_hash[..4].to_vec()
        };
        address_bytes.extend_from_slice(&checksum);

        // Base58 encode
        bs58::encode(address_bytes).into_string()
    }

    /// Get the wallet address
    pub fn address(&self) -> &str {
        &self.address
    }

    /// Get the configuration
    pub fn config(&self) -> &MultisigConfig {
        &self.config
    }

    /// Check if a public key is an authorized signer (also checks derived address)
    pub fn is_signer(&self, pubkey: &str) -> bool {
        self.config.is_signer_with_address_check(pubkey)
    }

    /// Get the required threshold
    pub fn threshold(&self) -> u8 {
        self.config.threshold
    }

    /// Get the total number of signers
    pub fn signer_count(&self) -> usize {
        self.config.signer_count()
    }

    /// Get human-readable description
    pub fn description(&self) -> String {
        self.config.description()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_pubkeys() -> Vec<String> {
        vec![
            "02a1633cafcc01ebfb6d78e39f687a1f0995c62fc95f51ead10a02ee0be551b5dc".to_string(),
            "03b31cc9a4c7a6c2b0f3c0e7d2f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4".to_string(),
            "02c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6".to_string(),
        ]
    }

    #[test]
    fn test_config_creation() {
        let config = MultisigConfig::new(2, sample_pubkeys(), Some("Test".to_string())).unwrap();

        assert_eq!(config.threshold(), 2);
        assert_eq!(config.signer_count(), 3);
        assert_eq!(config.description(), "2-of-3");
        assert!(config.label.is_some());
    }

    #[test]
    fn test_config_validation() {
        // Zero threshold
        assert!(MultisigConfig::new(0, sample_pubkeys(), None).is_err());

        // Threshold > signers
        assert!(MultisigConfig::new(4, sample_pubkeys(), None).is_err());

        // Only one signer
        assert!(MultisigConfig::new(1, vec!["pubkey1".to_string()], None).is_err());

        // Duplicate signers
        assert!(
            MultisigConfig::new(2, vec!["same".to_string(), "same".to_string()], None).is_err()
        );
    }

    #[test]
    fn test_wallet_creation() {
        let config = MultisigConfig::new(2, sample_pubkeys(), None).unwrap();
        let wallet = MultisigWallet::new(config).unwrap();

        // P2SH addresses start with '3'
        assert!(wallet.address().starts_with('3'));
        assert_eq!(wallet.threshold(), 2);
        assert_eq!(wallet.signer_count(), 3);
    }

    #[test]
    fn test_address_determinism() {
        let pubkeys = sample_pubkeys();

        let config1 = MultisigConfig::new(2, pubkeys.clone(), None).unwrap();
        let config2 = MultisigConfig::new(2, pubkeys, None).unwrap();

        let wallet1 = MultisigWallet::new(config1).unwrap();
        let wallet2 = MultisigWallet::new(config2).unwrap();

        // Same config should produce same address
        assert_eq!(wallet1.address(), wallet2.address());
    }

    #[test]
    fn test_is_signer() {
        let pubkeys = sample_pubkeys();
        let config = MultisigConfig::new(2, pubkeys.clone(), None).unwrap();
        let wallet = MultisigWallet::new(config).unwrap();

        assert!(wallet.is_signer(&pubkeys[0]));
        assert!(wallet.is_signer(&pubkeys[1]));
        assert!(!wallet.is_signer("not_a_signer"));
    }
}
