//! Cryptographic hashing utilities for the blockchain
//!
//! Provides SHA-256 based hashing functions used for block hashes,
//! transaction IDs, and merkle tree calculations.

use sha2::{Digest, Sha256};

/// Computes SHA-256 hash of the input data
pub fn sha256(data: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

/// Computes double SHA-256 hash (SHA-256 of SHA-256)
/// Used for block hashes in Bitcoin-style blockchains
pub fn double_sha256(data: &[u8]) -> Vec<u8> {
    sha256(&sha256(data))
}

/// Computes SHA-256 hash and returns it as a hex string
pub fn sha256_hex(data: &[u8]) -> String {
    hex::encode(sha256(data))
}

/// Computes double SHA-256 hash and returns it as a hex string
pub fn double_sha256_hex(data: &[u8]) -> String {
    hex::encode(double_sha256(data))
}

/// Checks if a hash meets the difficulty target
/// The hash must have `difficulty` leading zeros
pub fn meets_difficulty(hash: &[u8], difficulty: u32) -> bool {
    let required_zeros = difficulty as usize / 8;
    let remaining_bits = difficulty as usize % 8;

    // Check full zero bytes
    for byte in hash.iter().take(required_zeros) {
        if *byte != 0 {
            return false;
        }
    }

    // Check remaining bits
    if remaining_bits > 0 && required_zeros < hash.len() {
        let mask = 0xFF << (8 - remaining_bits);
        if hash[required_zeros] & mask != 0 {
            return false;
        }
    }

    true
}

/// Calculate target hash from difficulty
pub fn calculate_target(difficulty: u32) -> Vec<u8> {
    let mut target = vec![0xFF; 32];
    let full_bytes = difficulty as usize / 8;
    let remaining_bits = difficulty as usize % 8;

    for byte in target.iter_mut().take(full_bytes) {
        *byte = 0;
    }

    if remaining_bits > 0 && full_bytes < 32 {
        target[full_bytes] = 0xFF >> remaining_bits;
    }

    target
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256() {
        let data = b"hello world";
        let hash = sha256(data);
        assert_eq!(hash.len(), 32);
        assert_eq!(
            sha256_hex(data),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_double_sha256() {
        let data = b"hello world";
        let hash = double_sha256(data);
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_meets_difficulty() {
        // Hash with leading zeros
        let hash = vec![0x00, 0x00, 0x0F, 0xFF, 0xFF, 0xFF];
        assert!(meets_difficulty(&hash, 16)); // 16 bits = 2 bytes of zeros
        assert!(meets_difficulty(&hash, 12)); // 12 bits = 1.5 bytes of zeros
        assert!(!meets_difficulty(&hash, 24)); // Need 3 bytes of zeros
    }
}
