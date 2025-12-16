//! Block Compression
//!
//! Efficient block storage and transmission:
//! - Delta encoding for block headers
//! - Transaction deduplication
//! - Compact serialization

use crate::core::{Block, Transaction};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// Constants
// =============================================================================

/// Compression format version
pub const COMPRESSION_VERSION: u8 = 1;

/// Minimum block size to compress
pub const MIN_COMPRESS_SIZE: usize = 1024;

// =============================================================================
// Compressed Block
// =============================================================================

/// A compressed block for efficient storage/transmission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedBlock {
    /// Compression format version
    pub version: u8,
    /// Block height
    pub height: u64,
    /// Block hash
    pub hash: String,
    /// Compressed header (delta from previous)
    pub header: CompressedHeader,
    /// Transaction references (known txs by ID, or full tx)
    pub transactions: Vec<TxRef>,
    /// Original size in bytes
    pub original_size: u32,
    /// Compressed size in bytes
    pub compressed_size: u32,
}

/// Compressed block header (stores only changes from previous)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedHeader {
    /// Previous block hash (short form if possible)
    pub prev_hash: ShortHash,
    /// Merkle root
    pub merkle_root: String,
    /// Time delta from previous block (seconds)
    pub time_delta: i32,
    /// Difficulty (only if changed)
    pub difficulty: Option<u32>,
    /// Nonce
    pub nonce: u64,
}

/// Short hash reference (either full or truncated)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShortHash {
    /// Known hash by height reference
    ByHeight(u64),
    /// Truncated hash (first N bytes)
    Truncated(Vec<u8>),
    /// Full hash
    Full(String),
}

/// Transaction reference in compressed block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TxRef {
    /// Transaction known by ID (in mempool or recent blocks)
    Known(String),
    /// Short ID (for compact blocks)
    ShortId(u64),
    /// Full transaction data
    Full(Box<Transaction>),
}

// =============================================================================
// Block Compressor
// =============================================================================

/// Compresses blocks for storage/transmission
#[derive(Debug, Default)]
pub struct BlockCompressor {
    /// Recent block headers for delta encoding
    recent_headers: Vec<RecentBlock>,
    /// Known transaction IDs
    known_txs: HashMap<String, ()>,
    /// Statistics
    stats: CompressionStats,
}

#[derive(Debug, Clone)]
struct RecentBlock {
    height: u64,
    hash: String,
    timestamp: i64,
    difficulty: u32,
}

impl BlockCompressor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Compress a block
    pub fn compress(&mut self, block: &Block, prev_block: Option<&Block>) -> CompressedBlock {
        let original_size = self.estimate_size(block);

        // Compress header
        let header = self.compress_header(block, prev_block);

        // Compress transactions
        let transactions = self.compress_transactions(block);

        let result = CompressedBlock {
            version: COMPRESSION_VERSION,
            height: block.index,
            hash: block.hash.clone(),
            header,
            transactions,
            original_size: original_size as u32,
            compressed_size: 0, // Will be set after serialization
        };

        // Update stats
        self.stats.blocks_compressed += 1;
        self.stats.original_bytes += original_size as u64;

        // Remember this block
        self.recent_headers.push(RecentBlock {
            height: block.index,
            hash: block.hash.clone(),
            timestamp: block.header.timestamp.timestamp(),
            difficulty: block.header.difficulty,
        });

        // Keep only recent headers
        if self.recent_headers.len() > 100 {
            self.recent_headers.remove(0);
        }

        // Remember transactions
        for tx in &block.transactions {
            self.known_txs.insert(tx.id.clone(), ());
        }

        result
    }

    /// Decompress a block
    pub fn decompress(
        &self,
        compressed: &CompressedBlock,
        prev_block: Option<&Block>,
        tx_lookup: impl Fn(&str) -> Option<Transaction>,
    ) -> Option<Block> {
        // Decompress header
        let header = self.decompress_header(&compressed.header, prev_block)?;

        // Decompress transactions
        let transactions = self.decompress_transactions(&compressed.transactions, tx_lookup)?;

        // Reconstruct block
        Some(Block {
            index: compressed.height,
            header,
            hash: compressed.hash.clone(),
            transactions,
        })
    }

    /// Add known transaction (from mempool or previous blocks)
    pub fn add_known_tx(&mut self, tx_id: &str) {
        self.known_txs.insert(tx_id.to_string(), ());
    }

    /// Get compression statistics
    pub fn stats(&self) -> &CompressionStats {
        &self.stats
    }

    // Private methods

    fn compress_header(&self, block: &Block, prev: Option<&Block>) -> CompressedHeader {
        let prev_hash = if let Some(prev) = prev {
            // Reference by height if we know it
            if self.recent_headers.iter().any(|h| h.hash == prev.hash) {
                ShortHash::ByHeight(prev.index)
            } else {
                ShortHash::Full(block.header.previous_hash.clone())
            }
        } else {
            ShortHash::Full(block.header.previous_hash.clone())
        };

        let time_delta = if let Some(prev) = prev {
            (block.header.timestamp.timestamp() - prev.header.timestamp.timestamp()) as i32
        } else {
            0
        };

        let difficulty = if let Some(prev) = prev {
            if block.header.difficulty != prev.header.difficulty {
                Some(block.header.difficulty)
            } else {
                None
            }
        } else {
            Some(block.header.difficulty)
        };

        CompressedHeader {
            prev_hash,
            merkle_root: block.header.merkle_root.clone(),
            time_delta,
            difficulty,
            nonce: block.header.nonce,
        }
    }

    fn decompress_header(
        &self,
        compressed: &CompressedHeader,
        prev: Option<&Block>,
    ) -> Option<crate::core::BlockHeader> {
        let previous_hash = match &compressed.prev_hash {
            ShortHash::ByHeight(height) => self
                .recent_headers
                .iter()
                .find(|h| h.height == *height)
                .map(|h| h.hash.clone())?,
            ShortHash::Full(hash) => hash.clone(),
            ShortHash::Truncated(_) => return None, // Not supported yet
        };

        let timestamp = if let Some(prev) = prev {
            prev.header.timestamp + chrono::Duration::seconds(compressed.time_delta as i64)
        } else {
            chrono::Utc::now()
        };

        let difficulty = compressed
            .difficulty
            .or_else(|| prev.map(|p| p.header.difficulty))?;

        Some(crate::core::BlockHeader {
            version: 1,
            previous_hash,
            merkle_root: compressed.merkle_root.clone(),
            timestamp,
            difficulty,
            nonce: compressed.nonce,
        })
    }

    fn compress_transactions(&self, block: &Block) -> Vec<TxRef> {
        block
            .transactions
            .iter()
            .map(|tx| {
                if self.known_txs.contains_key(&tx.id) {
                    TxRef::Known(tx.id.clone())
                } else {
                    TxRef::Full(Box::new(tx.clone()))
                }
            })
            .collect()
    }

    fn decompress_transactions(
        &self,
        refs: &[TxRef],
        lookup: impl Fn(&str) -> Option<Transaction>,
    ) -> Option<Vec<Transaction>> {
        refs.iter()
            .map(|tx_ref| match tx_ref {
                TxRef::Known(id) => lookup(id),
                TxRef::ShortId(_) => None, // Not supported yet
                TxRef::Full(tx) => Some((**tx).clone()),
            })
            .collect()
    }

    fn estimate_size(&self, block: &Block) -> usize {
        // Rough estimate
        200 + block.transactions.len() * 300
    }
}

// =============================================================================
// Compression Statistics
// =============================================================================

/// Statistics about compression
#[derive(Debug, Clone, Default)]
pub struct CompressionStats {
    pub blocks_compressed: u64,
    pub original_bytes: u64,
    pub compressed_bytes: u64,
}

impl CompressionStats {
    pub fn compression_ratio(&self) -> f64 {
        if self.original_bytes == 0 {
            1.0
        } else {
            self.compressed_bytes as f64 / self.original_bytes as f64
        }
    }

    pub fn space_saved(&self) -> u64 {
        self.original_bytes.saturating_sub(self.compressed_bytes)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Blockchain;

    #[test]
    fn test_block_compression() {
        let mut compressor = BlockCompressor::new();
        let blockchain = Blockchain::with_difficulty(4);
        let block = blockchain.latest_block();

        let compressed = compressor.compress(block, None);

        assert_eq!(compressed.height, block.index);
        assert_eq!(compressed.hash, block.hash);
        assert_eq!(compressed.version, COMPRESSION_VERSION);
    }

    #[test]
    fn test_known_tx_compression() {
        let mut compressor = BlockCompressor::new();

        // Add known tx
        compressor.add_known_tx("known_tx_id");

        let blockchain = Blockchain::with_difficulty(4);
        let block = blockchain.latest_block();

        let compressed = compressor.compress(block, None);

        // Coinbase tx should be full since it's not "known"
        assert!(matches!(compressed.transactions[0], TxRef::Full(_)));
    }

    #[test]
    fn test_short_hash() {
        let by_height = ShortHash::ByHeight(100);
        let full = ShortHash::Full("abc123".to_string());

        match by_height {
            ShortHash::ByHeight(h) => assert_eq!(h, 100),
            _ => panic!("Wrong variant"),
        }

        match full {
            ShortHash::Full(s) => assert_eq!(s, "abc123"),
            _ => panic!("Wrong variant"),
        }
    }
}
