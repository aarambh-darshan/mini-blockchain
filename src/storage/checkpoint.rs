//! Checkpoint System
//!
//! Provides hardcoded checkpoints for fast sync and security:
//! - Skip full validation for known-good blocks
//! - Detect chain divergence attacks
//! - Speed up initial block download

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// Checkpoint Entry
// =============================================================================

/// A checkpoint representing a known-good block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Block height
    pub height: u64,
    /// Block hash
    pub hash: String,
    /// Unix timestamp (optional, for additional validation)
    pub timestamp: Option<i64>,
}

impl Checkpoint {
    pub fn new(height: u64, hash: &str) -> Self {
        Self {
            height,
            hash: hash.to_string(),
            timestamp: None,
        }
    }

    pub fn with_timestamp(height: u64, hash: &str, timestamp: i64) -> Self {
        Self {
            height,
            hash: hash.to_string(),
            timestamp: Some(timestamp),
        }
    }
}

// =============================================================================
// Checkpoint Manager
// =============================================================================

/// Manages checkpoints for chain validation
#[derive(Debug, Default)]
pub struct CheckpointManager {
    /// Checkpoints by height
    checkpoints: HashMap<u64, Checkpoint>,
    /// Highest checkpoint height
    highest_checkpoint: u64,
    /// Whether to enforce checkpoints strictly
    strict_mode: bool,
}

impl CheckpointManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with built-in mainnet checkpoints
    pub fn mainnet() -> Self {
        let mut manager = Self::new();

        // Add default genesis checkpoint
        manager.add_checkpoint(Checkpoint::new(0, "genesis"));

        manager
    }

    /// Create with testnet checkpoints
    pub fn testnet() -> Self {
        let mut manager = Self::new();
        manager.add_checkpoint(Checkpoint::new(0, "testnet_genesis"));
        manager
    }

    /// Add a checkpoint
    pub fn add_checkpoint(&mut self, checkpoint: Checkpoint) {
        if checkpoint.height > self.highest_checkpoint {
            self.highest_checkpoint = checkpoint.height;
        }
        self.checkpoints.insert(checkpoint.height, checkpoint);
    }

    /// Add multiple checkpoints
    pub fn add_checkpoints(&mut self, checkpoints: Vec<Checkpoint>) {
        for cp in checkpoints {
            self.add_checkpoint(cp);
        }
    }

    /// Check if a block matches the checkpoint at its height
    pub fn verify_checkpoint(&self, height: u64, hash: &str) -> CheckpointResult {
        match self.checkpoints.get(&height) {
            Some(cp) => {
                if cp.hash == hash {
                    CheckpointResult::Match
                } else {
                    CheckpointResult::Mismatch {
                        expected: cp.hash.clone(),
                        got: hash.to_string(),
                    }
                }
            }
            None => CheckpointResult::NoCheckpoint,
        }
    }

    /// Check if we're before the last checkpoint (can skip validation)
    pub fn can_skip_validation(&self, height: u64) -> bool {
        height < self.highest_checkpoint
    }

    /// Get checkpoint at height
    pub fn get_checkpoint(&self, height: u64) -> Option<&Checkpoint> {
        self.checkpoints.get(&height)
    }

    /// Get the highest checkpoint
    pub fn get_highest(&self) -> Option<&Checkpoint> {
        self.checkpoints.get(&self.highest_checkpoint)
    }

    /// Get highest checkpoint height
    pub fn highest_height(&self) -> u64 {
        self.highest_checkpoint
    }

    /// Get all checkpoints
    pub fn all(&self) -> Vec<&Checkpoint> {
        let mut cps: Vec<_> = self.checkpoints.values().collect();
        cps.sort_by_key(|cp| cp.height);
        cps
    }

    /// Set strict mode (reject blocks that don't match checkpoints)
    pub fn set_strict_mode(&mut self, strict: bool) {
        self.strict_mode = strict;
    }

    /// Check if in strict mode
    pub fn is_strict(&self) -> bool {
        self.strict_mode
    }

    /// Get checkpoint count
    pub fn len(&self) -> usize {
        self.checkpoints.len()
    }

    /// Check if no checkpoints
    pub fn is_empty(&self) -> bool {
        self.checkpoints.is_empty()
    }
}

// =============================================================================
// Checkpoint Result
// =============================================================================

/// Result of checkpoint verification
#[derive(Debug, Clone, PartialEq)]
pub enum CheckpointResult {
    /// Block matches the checkpoint
    Match,
    /// Block doesn't match the checkpoint
    Mismatch { expected: String, got: String },
    /// No checkpoint at this height
    NoCheckpoint,
}

impl CheckpointResult {
    pub fn is_valid(&self) -> bool {
        matches!(
            self,
            CheckpointResult::Match | CheckpointResult::NoCheckpoint
        )
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_verification() {
        let mut manager = CheckpointManager::new();
        manager.add_checkpoint(Checkpoint::new(100, "hash_at_100"));
        manager.add_checkpoint(Checkpoint::new(200, "hash_at_200"));

        // Match
        assert_eq!(
            manager.verify_checkpoint(100, "hash_at_100"),
            CheckpointResult::Match
        );

        // Mismatch
        let result = manager.verify_checkpoint(100, "wrong_hash");
        assert!(matches!(result, CheckpointResult::Mismatch { .. }));

        // No checkpoint
        assert_eq!(
            manager.verify_checkpoint(150, "any_hash"),
            CheckpointResult::NoCheckpoint
        );
    }

    #[test]
    fn test_skip_validation() {
        let mut manager = CheckpointManager::new();
        manager.add_checkpoint(Checkpoint::new(1000, "hash"));

        // Below checkpoint - can skip
        assert!(manager.can_skip_validation(500));
        assert!(manager.can_skip_validation(999));

        // At or above checkpoint - cannot skip
        assert!(!manager.can_skip_validation(1000));
        assert!(!manager.can_skip_validation(1001));
    }

    #[test]
    fn test_highest_checkpoint() {
        let mut manager = CheckpointManager::new();
        manager.add_checkpoint(Checkpoint::new(100, "a"));
        manager.add_checkpoint(Checkpoint::new(500, "b"));
        manager.add_checkpoint(Checkpoint::new(200, "c"));

        assert_eq!(manager.highest_height(), 500);
    }
}
