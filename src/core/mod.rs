//! Core blockchain components
//!
//! This module contains the fundamental building blocks:
//! - Transactions (UTXO model with locktime, RBF, replay protection)
//! - Blocks (with proof of work)
//! - Blockchain (chain management)
//! - Chain state (fork resolution, orphans, reorgs)
//! - SPV support (bloom filters, Merkle proofs)
//! - Fee estimation
//! - Block compression

pub mod block;
pub mod blockchain;
pub mod chain_state;
pub mod compression;
pub mod fee;
pub mod spv;
pub mod transaction;

pub use block::{Block, BlockHeader};
pub use blockchain::{
    Blockchain, BlockchainError, ChainStats, BLOCK_REWARD, DEFAULT_DIFFICULTY,
    DIFFICULTY_ADJUSTMENT_INTERVAL, MAX_DIFFICULTY_ADJUSTMENT_FACTOR, TARGET_BLOCK_TIME,
};
pub use chain_state::{
    BlockStatus, ChainStateManager, ChainTip, OrphanBlock, UndoData, MAX_FUTURE_BLOCK_TIME,
    MTP_BLOCK_COUNT,
};
pub use compression::{BlockCompressor, CompressedBlock, CompressionStats};
pub use fee::{BlockFeeStats, FeeEstimates, FeeEstimator, FeeRate, Priority};
pub use spv::{BloomFilter, MerkleProof, SpvClient};
pub use transaction::{
    Transaction, TransactionBuilder, TransactionError, TransactionInput, TransactionOutput,
    DEFAULT_CHAIN_ID, LOCKTIME_THRESHOLD, SEQUENCE_FINAL, SEQUENCE_RBF_MAX, TX_VERSION, UTXO,
};
