//! Storage module for blockchain persistence
//!
//! Production-grade storage features:
//! - Block and transaction indexing
//! - UTXO caching with LRU eviction
//! - Checkpoints for fast sync
//! - Pruning for reduced storage

pub mod checkpoint;
pub mod index;
pub mod persistence;
pub mod pruning;
pub mod utxo_cache;

pub use checkpoint::{Checkpoint, CheckpointManager, CheckpointResult};
pub use index::{
    BlockIndex, BlockIndexEntry, BlockIndexStats, TxIndex, TxIndexEntry, TxIndexStats,
};
pub use persistence::{
    load_from_file, save_to_file, Storage, StorageConfig, StorageError, StorageStats,
};
pub use pruning::{PruneRange, PruneState, PruneStats, Pruner, PrunerConfig};
pub use utxo_cache::{CacheEntry, CacheStats, UtxoCache};
