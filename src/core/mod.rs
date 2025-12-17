//! Core blockchain components
//!
//! This module contains the fundamental building blocks:
//! - Transactions (UTXO model with locktime, RBF, replay protection)
//! - Blocks (with proof of work and size limits)
//! - Blockchain (chain management with coinbase maturity)
//! - Chain state (fork resolution, orphans, reorgs)
//! - SPV support (bloom filters, Merkle proofs)
//! - Fee estimation
//! - Block compression
//! - Script system (P2PKH, P2SH, MultiSig, TimeLock)

pub mod block;
pub mod blockchain;
pub mod chain_state;
pub mod compression;
pub mod fee;
pub mod script;
pub mod spv;
pub mod transaction;

pub use block::{
    Block, BlockError, BlockHeader, BLOCK_HEADER_SIZE, MAX_BLOCK_SIZE, MAX_BLOCK_TXS,
    MAX_BLOCK_WEIGHT,
};
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
pub use script::{ScriptError, ScriptType, ScriptValidator, SigHashType};
pub use spv::{BloomFilter, MerkleProof, SpvClient};
pub use transaction::{
    ContractOperationType, TokenOperationType, Transaction, TransactionBuilder, TransactionError,
    TransactionInput, TransactionOutput, COINBASE_MATURITY, DEFAULT_CHAIN_ID, LOCKTIME_THRESHOLD,
    MAX_TX_SIGOPS, MAX_TX_SIZE, SEQUENCE_FINAL, SEQUENCE_RBF_MAX, TX_VERSION, UTXO,
};
