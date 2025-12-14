//! Blockchain persistence layer
//!
//! Provides save/load functionality for the blockchain.

use crate::core::Blockchain;
use std::fs;
use std::io::{self, BufReader, BufWriter};
use std::path::Path;
use thiserror::Error;

/// Storage errors
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("Invalid data: {0}")]
    InvalidData(String),
}

/// Storage configuration
#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub data_dir: std::path::PathBuf,
    pub blockchain_file: String,
    pub backup_enabled: bool,
    pub max_backups: usize,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            data_dir: std::path::PathBuf::from(".blockchain_data"),
            blockchain_file: "blockchain.json".to_string(),
            backup_enabled: true,
            max_backups: 5,
        }
    }
}

/// Blockchain storage manager
pub struct Storage {
    config: StorageConfig,
}

impl Storage {
    /// Create a new storage manager
    pub fn new(config: StorageConfig) -> Result<Self, StorageError> {
        fs::create_dir_all(&config.data_dir)?;
        Ok(Self { config })
    }

    /// Create with default configuration
    pub fn with_defaults() -> Result<Self, StorageError> {
        Self::new(StorageConfig::default())
    }

    /// Get the blockchain file path
    fn blockchain_path(&self) -> std::path::PathBuf {
        self.config.data_dir.join(&self.config.blockchain_file)
    }

    /// Get a backup file path
    fn backup_path(&self, index: usize) -> std::path::PathBuf {
        self.config
            .data_dir
            .join(format!("{}.backup.{}", self.config.blockchain_file, index))
    }

    /// Save the blockchain to disk
    pub fn save(&self, blockchain: &Blockchain) -> Result<(), StorageError> {
        let path = self.blockchain_path();

        // Create backup if enabled
        if self.config.backup_enabled && path.exists() {
            self.rotate_backups()?;
            fs::copy(&path, self.backup_path(0))?;
        }

        // Write to temporary file first
        let temp_path = self.config.data_dir.join("blockchain.tmp");
        let file = fs::File::create(&temp_path)?;
        let writer = BufWriter::new(file);

        serde_json::to_writer_pretty(writer, blockchain)?;

        // Atomic rename
        fs::rename(&temp_path, &path)?;

        Ok(())
    }

    /// Load the blockchain from disk
    pub fn load(&self) -> Result<Blockchain, StorageError> {
        let path = self.blockchain_path();

        if !path.exists() {
            return Err(StorageError::InvalidData(
                "Blockchain file not found".to_string(),
            ));
        }

        let file = fs::File::open(&path)?;
        let reader = BufReader::new(file);

        let mut blockchain: Blockchain = serde_json::from_reader(reader)?;

        // Rebuild UTXO set (not serialized)
        blockchain.rebuild_utxo_set();

        Ok(blockchain)
    }

    /// Check if a saved blockchain exists
    pub fn exists(&self) -> bool {
        self.blockchain_path().exists()
    }

    /// Delete the saved blockchain
    pub fn delete(&self) -> Result<(), StorageError> {
        let path = self.blockchain_path();
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    /// Rotate backup files
    fn rotate_backups(&self) -> Result<(), StorageError> {
        // Delete oldest backup
        let oldest = self.backup_path(self.config.max_backups - 1);
        if oldest.exists() {
            fs::remove_file(&oldest)?;
        }

        // Shift existing backups
        for i in (0..self.config.max_backups - 1).rev() {
            let current = self.backup_path(i);
            if current.exists() {
                let next = self.backup_path(i + 1);
                fs::rename(&current, &next)?;
            }
        }

        Ok(())
    }

    /// Restore from a backup
    pub fn restore_backup(&self, backup_index: usize) -> Result<Blockchain, StorageError> {
        let backup_path = self.backup_path(backup_index);

        if !backup_path.exists() {
            return Err(StorageError::InvalidData(format!(
                "Backup {} not found",
                backup_index
            )));
        }

        let file = fs::File::open(&backup_path)?;
        let reader = BufReader::new(file);

        let mut blockchain: Blockchain = serde_json::from_reader(reader)?;
        blockchain.rebuild_utxo_set();

        Ok(blockchain)
    }

    /// List available backups
    pub fn list_backups(&self) -> Vec<usize> {
        let mut backups = Vec::new();

        for i in 0..self.config.max_backups {
            if self.backup_path(i).exists() {
                backups.push(i);
            }
        }

        backups
    }

    /// Get storage statistics
    pub fn stats(&self) -> Result<StorageStats, StorageError> {
        let path = self.blockchain_path();

        let file_size = if path.exists() {
            fs::metadata(&path)?.len()
        } else {
            0
        };

        let backup_count = self.list_backups().len();

        Ok(StorageStats {
            file_size,
            backup_count,
            data_dir: self.config.data_dir.clone(),
        })
    }
}

/// Storage statistics
#[derive(Debug)]
pub struct StorageStats {
    pub file_size: u64,
    pub backup_count: usize,
    pub data_dir: std::path::PathBuf,
}

/// Save blockchain to a specific file path
pub fn save_to_file(blockchain: &Blockchain, path: &Path) -> Result<(), StorageError> {
    let file = fs::File::create(path)?;
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, blockchain)?;
    Ok(())
}

/// Load blockchain from a specific file path
pub fn load_from_file(path: &Path) -> Result<Blockchain, StorageError> {
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut blockchain: Blockchain = serde_json::from_reader(reader)?;
    blockchain.rebuild_utxo_set();
    Ok(blockchain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_load_blockchain() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = StorageConfig {
            data_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let storage = Storage::new(config).unwrap();
        let blockchain = Blockchain::with_difficulty(4);

        // Save
        storage.save(&blockchain).unwrap();
        assert!(storage.exists());

        // Load
        let loaded = storage.load().unwrap();
        assert_eq!(loaded.blocks.len(), blockchain.blocks.len());
        assert_eq!(loaded.difficulty, blockchain.difficulty);
    }

    #[test]
    fn test_backup_rotation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = StorageConfig {
            data_dir: temp_dir.path().to_path_buf(),
            max_backups: 3,
            ..Default::default()
        };

        let storage = Storage::new(config).unwrap();
        let mut blockchain = Blockchain::with_difficulty(4);

        // Save multiple times
        for _ in 0..5 {
            storage.save(&blockchain).unwrap();
            blockchain.mine_block(vec![], "miner").unwrap();
        }

        // Should have 3 backups (max)
        let backups = storage.list_backups();
        assert!(backups.len() <= 3);
    }
}
