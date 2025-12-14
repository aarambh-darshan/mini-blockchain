//! Cryptographic utilities for the blockchain
//!
//! This module provides:
//! - SHA-256 hashing
//! - ECDSA key management (secp256k1)
//! - Merkle tree calculations

pub mod hash;
pub mod keys;
pub mod merkle;

pub use hash::{double_sha256, double_sha256_hex, meets_difficulty, sha256, sha256_hex};
pub use keys::{
    public_key_from_hex, public_key_to_address, sign_message, verify_signature, KeyError, KeyPair,
};
pub use merkle::{
    build_merkle_tree, calculate_merkle_root, calculate_merkle_root_hex, MerkleProof,
};
