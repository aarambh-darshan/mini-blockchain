//! Block Pruning Support
//!
//! Provides pruning for reduced storage footprint:
//! - Keep only recent blocks (configurable window)
//! - Maintain UTXO set integrity
//! - Track pruned ranges

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

// =============================================================================
// Constants
// =============================================================================

/// Minimum blocks to keep (safety margin)
pub const MIN_KEEP_BLOCKS: u64 = 288; // ~1 day at 5min blocks

/// Default keep window (last N blocks)
pub const DEFAULT_KEEP_BLOCKS: u64 = 550; // ~2 days margin

// =============================================================================
// Prune State
// =============================================================================

/// Tracks what has been pruned
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PruneState {
    /// Lowest block height we have full data for
    pub lowest_block: u64,
    /// Highest block we've pruned to
    pub pruned_to: u64,
    /// Block hashes we've pruned (for verification)
    pub pruned_hashes: HashSet<String>,
    /// Total blocks pruned
    pub total_pruned: u64,
    /// Total bytes saved (estimated)
    pub bytes_saved: u64,
}

impl PruneState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a pruned block
    pub fn record_prune(&mut self, height: u64, hash: &str, size_bytes: u64) {
        self.pruned_hashes.insert(hash.to_string());
        self.total_pruned += 1;
        self.bytes_saved += size_bytes;

        if height > self.pruned_to {
            self.pruned_to = height;
        }

        self.lowest_block = self.pruned_to + 1;
    }

    /// Check if a block has been pruned
    pub fn is_pruned(&self, height: u64) -> bool {
        height < self.lowest_block
    }

    /// Check if we have a specific block
    pub fn has_block(&self, height: u64) -> bool {
        height >= self.lowest_block
    }
}

// =============================================================================
// Prune Target
// =============================================================================

/// What to prune
#[derive(Debug, Clone, Copy)]
pub enum PruneTarget {
    /// Keep last N blocks
    KeepRecent(u64),
    /// Prune to specific height
    ToHeight(u64),
    /// Prune blocks older than N seconds
    OlderThan(u64),
}

// =============================================================================
// Pruner Configuration
// =============================================================================

/// Configuration for the pruner
#[derive(Debug, Clone)]
pub struct PrunerConfig {
    /// Whether pruning is enabled
    pub enabled: bool,
    /// Number of blocks to keep
    pub keep_blocks: u64,
    /// Automatically prune on new blocks
    pub auto_prune: bool,
    /// Prune interval (every N blocks)
    pub prune_interval: u64,
}

impl Default for PrunerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            keep_blocks: DEFAULT_KEEP_BLOCKS,
            auto_prune: false,
            prune_interval: 100,
        }
    }
}

// =============================================================================
// Pruner
// =============================================================================

/// Manages block pruning
#[derive(Debug)]
pub struct Pruner {
    /// Configuration
    pub config: PrunerConfig,
    /// Current prune state
    pub state: PruneState,
    /// Blocks since last prune
    blocks_since_prune: u64,
}

impl Pruner {
    pub fn new(config: PrunerConfig) -> Self {
        Self {
            config,
            state: PruneState::new(),
            blocks_since_prune: 0,
        }
    }

    /// Create with default config
    pub fn disabled() -> Self {
        Self::new(PrunerConfig::default())
    }

    /// Create with pruning enabled
    pub fn enabled(keep_blocks: u64) -> Self {
        Self::new(PrunerConfig {
            enabled: true,
            keep_blocks: keep_blocks.max(MIN_KEEP_BLOCKS),
            auto_prune: true,
            prune_interval: 100,
        })
    }

    /// Check if pruning is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Calculate which blocks should be pruned
    pub fn calculate_prune_range(&self, current_height: u64) -> Option<PruneRange> {
        if !self.config.enabled {
            return None;
        }

        // Calculate target height to keep from
        let keep_from = current_height.saturating_sub(self.config.keep_blocks);

        // Already pruned to this point or beyond
        if keep_from <= self.state.pruned_to {
            return None;
        }

        // Calculate range to prune
        let start = self.state.pruned_to + 1;
        let end = keep_from;

        if end > start {
            Some(PruneRange { start, end })
        } else {
            None
        }
    }

    /// Record a new block (for auto-prune timing)
    pub fn on_new_block(&mut self, height: u64) -> bool {
        if !self.config.auto_prune {
            return false;
        }

        self.blocks_since_prune += 1;

        if self.blocks_since_prune >= self.config.prune_interval {
            self.blocks_since_prune = 0;
            self.calculate_prune_range(height).is_some()
        } else {
            false
        }
    }

    /// Record a pruned block
    pub fn record_prune(&mut self, height: u64, hash: &str, size_bytes: u64) {
        self.state.record_prune(height, hash, size_bytes);
    }

    /// Get prune statistics
    pub fn stats(&self) -> PruneStats {
        PruneStats {
            enabled: self.config.enabled,
            keep_blocks: self.config.keep_blocks,
            lowest_block: self.state.lowest_block,
            total_pruned: self.state.total_pruned,
            bytes_saved: self.state.bytes_saved,
        }
    }
}

// =============================================================================
// Prune Range
// =============================================================================

/// Range of blocks to prune
#[derive(Debug, Clone)]
pub struct PruneRange {
    /// First block to prune (inclusive)
    pub start: u64,
    /// Last block to prune (exclusive)
    pub end: u64,
}

impl PruneRange {
    pub fn count(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    pub fn iter(&self) -> impl Iterator<Item = u64> {
        self.start..self.end
    }
}

// =============================================================================
// Prune Statistics
// =============================================================================

/// Pruning statistics
#[derive(Debug, Clone)]
pub struct PruneStats {
    pub enabled: bool,
    pub keep_blocks: u64,
    pub lowest_block: u64,
    pub total_pruned: u64,
    pub bytes_saved: u64,
}

impl PruneStats {
    pub fn bytes_saved_mb(&self) -> f64 {
        self.bytes_saved as f64 / (1024.0 * 1024.0)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prune_calculation() {
        // Note: MIN_KEEP_BLOCKS = 288, so enabled(100) will use 288
        let pruner = Pruner::enabled(100);
        assert_eq!(pruner.config.keep_blocks, MIN_KEEP_BLOCKS);

        // At height 500, should prune up to height 500 - 288 = 212
        let range = pruner.calculate_prune_range(500);
        assert!(range.is_some());
        let range = range.unwrap();
        assert_eq!(range.start, 1);
        assert_eq!(range.end, 212);

        // Test with explicit higher keep_blocks
        let pruner2 = Pruner::enabled(400); // 400 > 288, so uses 400
        let range2 = pruner2.calculate_prune_range(500);
        assert!(range2.is_some());
        let range2 = range2.unwrap();
        assert_eq!(range2.end, 100); // 500 - 400 = 100
    }

    #[test]
    fn test_prune_state() {
        let mut state = PruneState::new();

        state.record_prune(10, "hash10", 1000);
        state.record_prune(11, "hash11", 1000);

        assert!(state.is_pruned(10));
        assert!(state.is_pruned(11));
        assert!(!state.is_pruned(12));
        assert!(state.has_block(12));
        assert_eq!(state.total_pruned, 2);
        assert_eq!(state.bytes_saved, 2000);
    }

    #[test]
    fn test_auto_prune() {
        let mut pruner = Pruner::new(PrunerConfig {
            enabled: true,
            keep_blocks: 100,
            auto_prune: true,
            prune_interval: 10,
        });

        // First 9 blocks shouldn't trigger
        for i in 1..10 {
            assert!(!pruner.on_new_block(i));
        }

        // 10th block should trigger (at height 200, prune range exists)
        assert!(pruner.on_new_block(200));
    }
}
