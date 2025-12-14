//! ECDSA key management for the blockchain
//!
//! Provides key pair generation, signing, and verification using
//! the secp256k1 elliptic curve (same as Bitcoin).

use rand::rngs::OsRng;
use ripemd::Ripemd160;
use secp256k1::{Message, PublicKey, Secp256k1, SecretKey};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::hash::sha256;

/// Errors that can occur during key operations
#[derive(Error, Debug)]
pub enum KeyError {
    #[error("Invalid private key")]
    InvalidPrivateKey,
    #[error("Invalid public key")]
    InvalidPublicKey,
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Signature verification failed")]
    VerificationFailed,
    #[error("Secp256k1 error: {0}")]
    Secp256k1Error(#[from] secp256k1::Error),
}

/// A key pair consisting of a private key and its corresponding public key
#[derive(Clone)]
pub struct KeyPair {
    pub secret_key: SecretKey,
    pub public_key: PublicKey,
}

impl KeyPair {
    /// Generate a new random key pair
    pub fn generate() -> Self {
        let secp = Secp256k1::new();
        let (secret_key, public_key) = secp.generate_keypair(&mut OsRng);
        Self {
            secret_key,
            public_key,
        }
    }

    /// Create a key pair from an existing secret key
    pub fn from_secret_key(secret_key: SecretKey) -> Self {
        let secp = Secp256k1::new();
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);
        Self {
            secret_key,
            public_key,
        }
    }

    /// Create a key pair from a hex-encoded private key
    pub fn from_private_key_hex(hex_key: &str) -> Result<Self, KeyError> {
        let bytes = hex::decode(hex_key).map_err(|_| KeyError::InvalidPrivateKey)?;
        let secret_key =
            SecretKey::from_slice(&bytes).map_err(|_| KeyError::InvalidPrivateKey)?;
        Ok(Self::from_secret_key(secret_key))
    }

    /// Get the private key as a hex string
    pub fn private_key_hex(&self) -> String {
        hex::encode(self.secret_key.secret_bytes())
    }

    /// Get the public key as a hex string (compressed format)
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.public_key.serialize())
    }

    /// Generate a blockchain address from the public key
    /// Uses Bitcoin-style address generation: Base58Check(RIPEMD160(SHA256(pubkey)))
    pub fn address(&self) -> String {
        public_key_to_address(&self.public_key)
    }

    /// Sign a message hash with the private key
    pub fn sign(&self, message_hash: &[u8]) -> Result<Vec<u8>, KeyError> {
        sign_message(&self.secret_key, message_hash)
    }

    /// Verify a signature against this key pair's public key
    pub fn verify(&self, message_hash: &[u8], signature: &[u8]) -> Result<bool, KeyError> {
        verify_signature(&self.public_key, message_hash, signature)
    }
}

/// Convert a public key to a blockchain address
pub fn public_key_to_address(public_key: &PublicKey) -> String {
    // SHA256 of the public key
    let sha256_hash = sha256(&public_key.serialize());

    // RIPEMD160 of the SHA256 hash
    let mut ripemd = Ripemd160::new();
    ripemd.update(&sha256_hash);
    let ripemd_hash = ripemd.finalize();

    // Add version byte (0x00 for mainnet)
    let mut address_bytes = vec![0x00];
    address_bytes.extend_from_slice(&ripemd_hash);

    // Calculate checksum (first 4 bytes of double SHA256)
    let checksum = {
        let mut hasher = Sha256::new();
        hasher.update(&address_bytes);
        let first_hash = hasher.finalize();
        let mut hasher = Sha256::new();
        hasher.update(first_hash);
        hasher.finalize()
    };
    address_bytes.extend_from_slice(&checksum[..4]);

    // Base58 encode
    bs58::encode(address_bytes).into_string()
}

/// Parse a public key from hex string
pub fn public_key_from_hex(hex_key: &str) -> Result<PublicKey, KeyError> {
    let bytes = hex::decode(hex_key).map_err(|_| KeyError::InvalidPublicKey)?;
    PublicKey::from_slice(&bytes).map_err(|_| KeyError::InvalidPublicKey)
}

/// Sign a message hash with a secret key
pub fn sign_message(secret_key: &SecretKey, message_hash: &[u8]) -> Result<Vec<u8>, KeyError> {
    let secp = Secp256k1::new();
    
    // Ensure message hash is 32 bytes
    let hash = if message_hash.len() == 32 {
        message_hash.to_vec()
    } else {
        sha256(message_hash)
    };

    let message = Message::from_digest_slice(&hash)?;
    let signature = secp.sign_ecdsa(&message, secret_key);
    Ok(signature.serialize_compact().to_vec())
}

/// Verify a signature against a public key
pub fn verify_signature(
    public_key: &PublicKey,
    message_hash: &[u8],
    signature: &[u8],
) -> Result<bool, KeyError> {
    let secp = Secp256k1::new();

    // Ensure message hash is 32 bytes
    let hash = if message_hash.len() == 32 {
        message_hash.to_vec()
    } else {
        sha256(message_hash)
    };

    let message = Message::from_digest_slice(&hash)?;
    let sig = secp256k1::ecdsa::Signature::from_compact(signature)
        .map_err(|_| KeyError::InvalidSignature)?;

    match secp.verify_ecdsa(&message, &sig, public_key) {
        Ok(()) => Ok(true),
        Err(_) => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_pair_generation() {
        let kp = KeyPair::generate();
        assert!(!kp.private_key_hex().is_empty());
        assert!(!kp.public_key_hex().is_empty());
        assert!(!kp.address().is_empty());
    }

    #[test]
    fn test_sign_and_verify() {
        let kp = KeyPair::generate();
        let message = b"Hello, blockchain!";
        let message_hash = sha256(message);

        let signature = kp.sign(&message_hash).unwrap();
        assert!(kp.verify(&message_hash, &signature).unwrap());
    }

    #[test]
    fn test_key_pair_from_hex() {
        let kp1 = KeyPair::generate();
        let private_hex = kp1.private_key_hex();

        let kp2 = KeyPair::from_private_key_hex(&private_hex).unwrap();
        assert_eq!(kp1.public_key_hex(), kp2.public_key_hex());
        assert_eq!(kp1.address(), kp2.address());
    }

    #[test]
    fn test_address_format() {
        let kp = KeyPair::generate();
        let address = kp.address();
        // Bitcoin-style addresses start with 1 (mainnet)
        assert!(address.starts_with('1'));
    }
}
