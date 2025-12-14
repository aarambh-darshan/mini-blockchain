//! Core blockchain components
//!
//! This module contains the fundamental building blocks:
//! - Transactions (UTXO model)
//! - Blocks (with proof of work)
//! - Blockchain (chain management)

pub mod block;
pub mod blockchain;
pub mod transaction;

pub use block::{Block, BlockHeader};
pub use blockchain::{
    Blockchain, BlockchainError, ChainStats, BLOCK_REWARD, DEFAULT_DIFFICULTY,
    DIFFICULTY_ADJUSTMENT_INTERVAL, TARGET_BLOCK_TIME,
};
pub use transaction::{
    Transaction, TransactionBuilder, TransactionError, TransactionInput, TransactionOutput, UTXO,
};
