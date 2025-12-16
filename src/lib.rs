//! Mini-Blockchain: A production-ready blockchain implementation in Rust
//!
//! This crate provides a complete blockchain implementation featuring:
//! - Proof of Work consensus with fork resolution
//! - ECDSA digital signatures (secp256k1)
//! - UTXO-based transaction model with locktime and RBF
//! - Merkle tree transaction verification
//! - Wallet management with multi-signature support
//! - Transaction mempool with fee-based ordering
//! - SPV light client support (bloom filters)
//! - Fee estimation
//! - JSON persistence with checkpoints and pruning
//!
//! # Example
//!
//! ```rust
//! use mini_blockchain::core::Blockchain;
//! use mini_blockchain::wallet::Wallet;
//! use mini_blockchain::mining::Miner;
//!
//! // Create a new blockchain
//! let mut blockchain = Blockchain::with_difficulty(8);
//!
//! // Create a wallet
//! let wallet = Wallet::new();
//! println!("Address: {}", wallet.address());
//!
//! // Mine a block
//! let miner = Miner::new(&wallet.address());
//! let (block, stats) = miner.mine_block(&mut blockchain, vec![]).unwrap();
//! println!("Mined block {} in {}ms", block.index, stats.time_ms);
//!
//! // Check balance
//! let balance = wallet.balance(&blockchain);
//! println!("Balance: {} coins", balance);
//! ```

pub mod api;
pub mod cli;
pub mod contract;
pub mod core;
pub mod crypto;
pub mod mining;
pub mod multisig;
pub mod network;
pub mod storage;
pub mod token;
pub mod wallet;

// Re-export commonly used types
pub use api::{create_router, ApiState};
pub use contract::{Compiler, Contract, ContractManager, OpCode, VM};
pub use core::{
    Block,
    Blockchain,
    // New Phase 5 exports
    BloomFilter,
    FeeEstimator,
    FeeRate,
    MerkleProof,
    SpvClient,
    Transaction,
    BLOCK_REWARD,
    DEFAULT_DIFFICULTY,
};
pub use crypto::KeyPair;
pub use mining::{Mempool, Miner};
pub use multisig::{MultisigConfig, MultisigManager, MultisigWallet};
pub use network::{Node, NodeConfig};
pub use storage::{BlockIndex, CheckpointManager, Pruner, Storage, UtxoCache};
pub use token::{Token, TokenManager, TokenMetadata};
pub use wallet::Wallet;
