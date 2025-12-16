//! SPV (Simplified Payment Verification) Support
//!
//! Enables light clients to verify transactions without full blockchain:
//! - Bloom filters for efficient address matching (BIP 37)
//! - Merkle proofs for transaction inclusion
//! - Headers-only sync for light nodes

use crate::core::{Block, Transaction};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

// =============================================================================
// Constants
// =============================================================================

/// Default bloom filter size (in bits)
pub const DEFAULT_BLOOM_SIZE: usize = 36_000; // ~4.5 KB

/// Default number of hash functions
pub const DEFAULT_HASH_FUNCS: u8 = 10;

/// Maximum bloom filter size
pub const MAX_BLOOM_SIZE: usize = 36_000_000; // ~4.5 MB

/// Bloom filter update flags
pub const BLOOM_UPDATE_NONE: u8 = 0;
pub const BLOOM_UPDATE_ALL: u8 = 1;
pub const BLOOM_UPDATE_P2PUBKEY_ONLY: u8 = 2;

// =============================================================================
// Bloom Filter (BIP 37)
// =============================================================================

/// Bloom filter for efficient address/transaction matching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BloomFilter {
    /// Filter data (bit array)
    data: Vec<u8>,
    /// Number of hash functions
    hash_funcs: u8,
    /// Tweak for hash randomization
    tweak: u32,
    /// Update flags
    flags: u8,
}

impl BloomFilter {
    /// Create a new bloom filter
    pub fn new(size_bits: usize, hash_funcs: u8, tweak: u32) -> Self {
        let size_bytes = (size_bits + 7) / 8;
        Self {
            data: vec![0u8; size_bytes.min(MAX_BLOOM_SIZE / 8)],
            hash_funcs: hash_funcs.min(50),
            tweak,
            flags: BLOOM_UPDATE_NONE,
        }
    }

    /// Create with default parameters
    pub fn default_filter() -> Self {
        Self::new(DEFAULT_BLOOM_SIZE, DEFAULT_HASH_FUNCS, rand::random())
    }

    /// Create filter optimized for N elements with target false positive rate
    pub fn for_elements(n_elements: usize, fp_rate: f64) -> Self {
        // Calculate optimal size: -1 / (ln(2)^2) * n * ln(p)
        let ln2_squared = std::f64::consts::LN_2 * std::f64::consts::LN_2;
        let size_bits = ((-1.0 / ln2_squared) * (n_elements as f64) * fp_rate.ln()) as usize;
        let size_bits = size_bits.max(8).min(MAX_BLOOM_SIZE);

        // Calculate optimal hash functions: (m/n) * ln(2)
        let hash_funcs = ((size_bits as f64 / n_elements as f64) * std::f64::consts::LN_2) as u8;
        let hash_funcs = hash_funcs.max(1).min(50);

        Self::new(size_bits, hash_funcs, rand::random())
    }

    /// Add data to the filter
    pub fn insert(&mut self, data: &[u8]) {
        for i in 0..self.hash_funcs as u32 {
            let idx = self.hash(data, i);
            self.set_bit(idx);
        }
    }

    /// Add an address to the filter
    pub fn insert_address(&mut self, address: &str) {
        self.insert(address.as_bytes());
    }

    /// Add a transaction ID to the filter
    pub fn insert_txid(&mut self, txid: &str) {
        self.insert(txid.as_bytes());
    }

    /// Check if data might be in the filter
    pub fn contains(&self, data: &[u8]) -> bool {
        for i in 0..self.hash_funcs as u32 {
            let idx = self.hash(data, i);
            if !self.get_bit(idx) {
                return false;
            }
        }
        true
    }

    /// Check if an address might be in the filter
    pub fn contains_address(&self, address: &str) -> bool {
        self.contains(address.as_bytes())
    }

    /// Check if a transaction matches the filter
    pub fn matches_transaction(&self, tx: &Transaction) -> bool {
        // Check transaction ID
        if self.contains(tx.id.as_bytes()) {
            return true;
        }

        // Check outputs (recipient addresses)
        for output in &tx.outputs {
            if self.contains(output.recipient.as_bytes()) {
                return true;
            }
        }

        // Check inputs (spending from watched addresses)
        for input in &tx.inputs {
            if self.contains(input.tx_id.as_bytes()) {
                return true;
            }
        }

        false
    }

    /// Filter transactions in a block
    pub fn filter_block<'a>(&self, block: &'a Block) -> Vec<&'a Transaction> {
        block
            .transactions
            .iter()
            .filter(|tx| self.matches_transaction(tx))
            .collect()
    }

    /// Get filter size in bytes
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Clear the filter
    pub fn clear(&mut self) {
        self.data.fill(0);
    }

    /// Check if filter is empty
    pub fn is_empty(&self) -> bool {
        self.data.iter().all(|&b| b == 0)
    }

    // Private helpers

    fn hash(&self, data: &[u8], n: u32) -> usize {
        // MurmurHash3-like hashing with seed (use wrapping to avoid overflow)
        let seed = n.wrapping_mul(0xFBA4C795).wrapping_add(self.tweak);
        let mut hasher = Sha256::new();
        hasher.update(seed.to_le_bytes());
        hasher.update(data);
        let hash = hasher.finalize();

        // Convert first 4 bytes to index
        let idx = u32::from_le_bytes([hash[0], hash[1], hash[2], hash[3]]);
        (idx as usize) % (self.data.len() * 8)
    }

    fn set_bit(&mut self, idx: usize) {
        let byte_idx = idx / 8;
        let bit_idx = idx % 8;
        if byte_idx < self.data.len() {
            self.data[byte_idx] |= 1 << bit_idx;
        }
    }

    fn get_bit(&self, idx: usize) -> bool {
        let byte_idx = idx / 8;
        let bit_idx = idx % 8;
        if byte_idx < self.data.len() {
            (self.data[byte_idx] & (1 << bit_idx)) != 0
        } else {
            false
        }
    }
}

// =============================================================================
// Merkle Proof
// =============================================================================

/// Proof that a transaction is included in a block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    /// Transaction ID
    pub tx_id: String,
    /// Block hash
    pub block_hash: String,
    /// Block height
    pub block_height: u64,
    /// Merkle path (sibling hashes from tx to root)
    pub path: Vec<MerkleNode>,
    /// Total transactions in block
    pub tx_count: u32,
}

/// A node in the Merkle proof path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleNode {
    /// Hash at this level
    pub hash: String,
    /// Whether this is a left sibling (false = right)
    pub is_left: bool,
}

impl MerkleProof {
    /// Create a proof for a transaction in a block
    pub fn create(block: &Block, tx_id: &str) -> Option<Self> {
        // Find transaction index
        let tx_index = block.transactions.iter().position(|tx| tx.id == tx_id)?;

        // Build merkle tree and get proof path
        let hashes: Vec<String> = block.transactions.iter().map(|tx| tx.id.clone()).collect();
        let path = Self::build_proof_path(&hashes, tx_index);

        Some(Self {
            tx_id: tx_id.to_string(),
            block_hash: block.hash.clone(),
            block_height: block.index,
            path,
            tx_count: block.transactions.len() as u32,
        })
    }

    /// Verify the proof against a block header
    pub fn verify(&self, merkle_root: &str) -> bool {
        let mut hash = self.tx_id.clone();

        for node in &self.path {
            hash = if node.is_left {
                Self::hash_pair(&node.hash, &hash)
            } else {
                Self::hash_pair(&hash, &node.hash)
            };
        }

        hash == merkle_root
    }

    fn build_proof_path(hashes: &[String], mut index: usize) -> Vec<MerkleNode> {
        let mut path = Vec::new();
        let mut level: Vec<String> = hashes.to_vec();

        while level.len() > 1 {
            // If odd number, duplicate last
            if level.len() % 2 == 1 {
                level.push(level.last().unwrap().clone());
            }

            // Get sibling
            let sibling_idx = if index % 2 == 0 { index + 1 } else { index - 1 };
            let is_left = index % 2 == 1;

            if sibling_idx < level.len() {
                path.push(MerkleNode {
                    hash: level[sibling_idx].clone(),
                    is_left,
                });
            }

            // Build next level
            let mut next_level = Vec::new();
            for chunk in level.chunks(2) {
                next_level.push(Self::hash_pair(&chunk[0], &chunk[1]));
            }
            level = next_level;
            index /= 2;
        }

        path
    }

    fn hash_pair(left: &str, right: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(left.as_bytes());
        hasher.update(right.as_bytes());
        hex::encode(hasher.finalize())
    }
}

// =============================================================================
// SPV Client State
// =============================================================================

/// State for an SPV light client
#[derive(Debug, Default)]
pub struct SpvClient {
    /// Block headers only
    headers: Vec<BlockHeader>,
    /// Addresses being watched
    pub watch_addresses: HashSet<String>,
    /// Transaction IDs being watched
    pub watch_txids: HashSet<String>,
    /// Bloom filter for efficient matching
    filter: Option<BloomFilter>,
    /// Verified transactions with proofs
    pub verified_txs: Vec<VerifiedTransaction>,
}

/// Lightweight block header for SPV
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    pub height: u64,
    pub hash: String,
    pub prev_hash: String,
    pub merkle_root: String,
    pub timestamp: i64,
    pub difficulty: u32,
    pub nonce: u64,
}

impl BlockHeader {
    pub fn from_block(block: &Block) -> Self {
        Self {
            height: block.index,
            hash: block.hash.clone(),
            prev_hash: block.header.previous_hash.clone(),
            merkle_root: block.header.merkle_root.clone(),
            timestamp: block.header.timestamp.timestamp(),
            difficulty: block.header.difficulty,
            nonce: block.header.nonce,
        }
    }
}

/// A transaction verified via SPV
#[derive(Debug, Clone)]
pub struct VerifiedTransaction {
    pub tx: Transaction,
    pub block_hash: String,
    pub block_height: u64,
    pub confirmations: u64,
}

impl SpvClient {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add address to watch
    pub fn watch_address(&mut self, address: &str) {
        self.watch_addresses.insert(address.to_string());
        self.rebuild_filter();
    }

    /// Add multiple addresses to watch
    pub fn watch_addresses(&mut self, addresses: &[String]) {
        for addr in addresses {
            self.watch_addresses.insert(addr.clone());
        }
        self.rebuild_filter();
    }

    /// Get current bloom filter
    pub fn get_filter(&self) -> Option<&BloomFilter> {
        self.filter.as_ref()
    }

    /// Add a block header
    pub fn add_header(&mut self, header: BlockHeader) -> bool {
        // Verify it chains correctly
        if let Some(last) = self.headers.last() {
            if header.prev_hash != last.hash {
                return false;
            }
            if header.height != last.height + 1 {
                return false;
            }
        }

        self.headers.push(header);
        true
    }

    /// Get current height
    pub fn height(&self) -> u64 {
        self.headers.last().map(|h| h.height).unwrap_or(0)
    }

    /// Verify a transaction with proof
    pub fn verify_transaction(&mut self, tx: Transaction, proof: MerkleProof) -> bool {
        // Find the header
        let header = self.headers.iter().find(|h| h.hash == proof.block_hash);

        if let Some(header) = header {
            if proof.verify(&header.merkle_root) {
                let confirmations = self.height().saturating_sub(header.height) + 1;
                self.verified_txs.push(VerifiedTransaction {
                    tx,
                    block_hash: proof.block_hash,
                    block_height: header.height,
                    confirmations,
                });
                return true;
            }
        }

        false
    }

    /// Get transactions for an address
    pub fn get_address_transactions(&self, address: &str) -> Vec<&VerifiedTransaction> {
        self.verified_txs
            .iter()
            .filter(|vtx| vtx.tx.outputs.iter().any(|o| o.recipient == address))
            .collect()
    }

    /// Get balance for an address
    pub fn get_balance(&self, address: &str) -> u64 {
        // Simple: sum outputs to this address (doesn't track spends)
        self.get_address_transactions(address)
            .iter()
            .flat_map(|vtx| vtx.tx.outputs.iter())
            .filter(|o| o.recipient == address)
            .map(|o| o.amount)
            .sum()
    }

    fn rebuild_filter(&mut self) {
        let n = self.watch_addresses.len() + self.watch_txids.len();
        if n == 0 {
            self.filter = None;
            return;
        }

        let mut filter = BloomFilter::for_elements(n.max(10), 0.0001);

        for addr in &self.watch_addresses {
            filter.insert_address(addr);
        }

        for txid in &self.watch_txids {
            filter.insert_txid(txid);
        }

        self.filter = Some(filter);
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bloom_filter() {
        let mut filter = BloomFilter::default_filter();

        filter.insert_address("addr1");
        filter.insert_address("addr2");

        assert!(filter.contains_address("addr1"));
        assert!(filter.contains_address("addr2"));
        assert!(!filter.contains_address("addr3")); // Might have false positives
    }

    #[test]
    fn test_bloom_filter_sizing() {
        // 100 elements with 0.01% false positive rate
        let filter = BloomFilter::for_elements(100, 0.0001);

        assert!(filter.size() > 0);
        assert!(filter.hash_funcs > 0);
    }

    #[test]
    fn test_merkle_proof() {
        // Simple test with mock hashes
        let path = vec![
            MerkleNode {
                hash: "sibling1".to_string(),
                is_left: false,
            },
            MerkleNode {
                hash: "sibling2".to_string(),
                is_left: true,
            },
        ];

        let proof = MerkleProof {
            tx_id: "tx1".to_string(),
            block_hash: "block1".to_string(),
            block_height: 1,
            path,
            tx_count: 4,
        };

        // Calculate expected root
        let hash1 = MerkleProof::hash_pair("tx1", "sibling1");
        let expected_root = MerkleProof::hash_pair("sibling2", &hash1);

        assert!(proof.verify(&expected_root));
    }

    #[test]
    fn test_spv_client() {
        let mut client = SpvClient::new();

        client.watch_address("my_address");
        assert!(client.get_filter().is_some());

        let filter = client.get_filter().unwrap();
        assert!(filter.contains_address("my_address"));
    }
}
