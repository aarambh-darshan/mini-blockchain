//! Storage module for blockchain persistence

pub mod persistence;

pub use persistence::{
    load_from_file, save_to_file, Storage, StorageConfig, StorageError, StorageStats,
};
