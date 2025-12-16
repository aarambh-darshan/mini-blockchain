//! Fee Estimation
//!
//! Smart fee estimation based on mempool state and recent blocks:
//! - Target confirmation time
//! - Fee rate percentiles
//! - Historical fee tracking

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

// =============================================================================
// Constants
// =============================================================================

/// Maximum blocks to track for fee estimation
pub const FEE_HISTORY_BLOCKS: usize = 100;

/// Fee rate buckets (satoshis per byte)
pub const FEE_BUCKETS: [u64; 10] = [1, 2, 5, 10, 20, 50, 100, 200, 500, 1000];

/// Default minimum fee rate (sat/byte)
pub const MIN_FEE_RATE: u64 = 1;

/// Default maximum fee rate (sat/byte)
pub const MAX_FEE_RATE: u64 = 10_000;

// =============================================================================
// Fee Rate
// =============================================================================

/// Fee rate in satoshis per virtual byte
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FeeRate(pub u64);

impl FeeRate {
    /// Create from satoshis per byte
    pub fn from_sat_per_byte(rate: u64) -> Self {
        Self(rate)
    }

    /// Create from total fee and transaction size
    pub fn from_fee_and_size(fee: u64, size_bytes: usize) -> Self {
        if size_bytes == 0 {
            Self(0)
        } else {
            Self(fee / size_bytes as u64)
        }
    }

    /// Calculate fee for a given size
    pub fn fee_for_size(&self, size_bytes: usize) -> u64 {
        self.0 * size_bytes as u64
    }

    /// Get rate as satoshis per byte
    pub fn as_sat_per_byte(&self) -> u64 {
        self.0
    }
}

impl Default for FeeRate {
    fn default() -> Self {
        Self(MIN_FEE_RATE)
    }
}

// =============================================================================
// Block Fee Stats
// =============================================================================

/// Fee statistics for a mined block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockFeeStats {
    /// Block height
    pub height: u64,
    /// Number of transactions (excluding coinbase)
    pub tx_count: u32,
    /// Total fees collected
    pub total_fees: u64,
    /// Minimum fee rate in block
    pub min_fee_rate: FeeRate,
    /// Maximum fee rate in block
    pub max_fee_rate: FeeRate,
    /// Median fee rate
    pub median_fee_rate: FeeRate,
    /// Fee rate percentiles (10th, 25th, 50th, 75th, 90th)
    pub percentiles: [FeeRate; 5],
}

impl BlockFeeStats {
    /// Calculate stats from a list of (fee, size) pairs
    pub fn from_transactions(height: u64, tx_data: &[(u64, usize)]) -> Self {
        if tx_data.is_empty() {
            return Self {
                height,
                tx_count: 0,
                total_fees: 0,
                min_fee_rate: FeeRate::default(),
                max_fee_rate: FeeRate::default(),
                median_fee_rate: FeeRate::default(),
                percentiles: [FeeRate::default(); 5],
            };
        }

        // Calculate fee rates
        let mut rates: Vec<FeeRate> = tx_data
            .iter()
            .map(|(fee, size)| FeeRate::from_fee_and_size(*fee, *size))
            .collect();
        rates.sort();

        let total_fees: u64 = tx_data.iter().map(|(fee, _)| fee).sum();
        let n = rates.len();

        Self {
            height,
            tx_count: n as u32,
            total_fees,
            min_fee_rate: rates[0],
            max_fee_rate: rates[n - 1],
            median_fee_rate: rates[n / 2],
            percentiles: [
                rates[n / 10],     // 10th percentile
                rates[n / 4],      // 25th percentile
                rates[n / 2],      // 50th (median)
                rates[3 * n / 4],  // 75th percentile
                rates[9 * n / 10], // 90th percentile
            ],
        }
    }
}

// =============================================================================
// Fee Estimator
// =============================================================================

/// Estimates appropriate fees for transactions
#[derive(Debug, Default)]
pub struct FeeEstimator {
    /// Historical block fee stats
    block_history: VecDeque<BlockFeeStats>,
    /// Current mempool fee rates (sorted)
    mempool_rates: Vec<FeeRate>,
    /// Last updated timestamp
    last_update: u64,
}

impl FeeEstimator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new block's fee statistics
    pub fn add_block(&mut self, stats: BlockFeeStats) {
        self.block_history.push_back(stats);
        while self.block_history.len() > FEE_HISTORY_BLOCKS {
            self.block_history.pop_front();
        }
    }

    /// Update mempool state
    pub fn update_mempool(&mut self, tx_data: &[(u64, usize)]) {
        self.mempool_rates = tx_data
            .iter()
            .map(|(fee, size)| FeeRate::from_fee_and_size(*fee, *size))
            .collect();
        self.mempool_rates.sort();
        self.last_update = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }

    /// Estimate fee rate for target confirmation blocks
    pub fn estimate_fee(&self, target_blocks: u32) -> FeeRate {
        // Simple estimation based on historical data and mempool
        if self.block_history.is_empty() {
            return self.estimate_from_mempool(target_blocks);
        }

        // Use historical percentiles based on target
        let percentile_idx = match target_blocks {
            0..=1 => 4,  // 90th percentile for next block
            2..=3 => 3,  // 75th percentile for 2-3 blocks
            4..=6 => 2,  // 50th percentile for 4-6 blocks
            7..=12 => 1, // 25th percentile for ~1 hour
            _ => 0,      // 10th percentile for low priority
        };

        // Average the percentile across recent blocks
        let rates: Vec<u64> = self
            .block_history
            .iter()
            .rev()
            .take(target_blocks.max(6) as usize)
            .map(|b| b.percentiles[percentile_idx].0)
            .collect();

        if rates.is_empty() {
            return FeeRate::default();
        }

        let avg = rates.iter().sum::<u64>() / rates.len() as u64;

        // Bump slightly if mempool is congested
        let mempool_adjustment = self.get_mempool_pressure();
        let adjusted = avg + (avg * mempool_adjustment / 100);

        FeeRate(adjusted.clamp(MIN_FEE_RATE, MAX_FEE_RATE))
    }

    /// Estimate fee for immediate confirmation (next block)
    pub fn estimate_high_priority(&self) -> FeeRate {
        self.estimate_fee(1)
    }

    /// Estimate fee for normal confirmation (~3 blocks)
    pub fn estimate_normal(&self) -> FeeRate {
        self.estimate_fee(3)
    }

    /// Estimate fee for low priority (~6 blocks)
    pub fn estimate_low_priority(&self) -> FeeRate {
        self.estimate_fee(6)
    }

    /// Estimate fee for economic (background) transactions
    pub fn estimate_economy(&self) -> FeeRate {
        self.estimate_fee(25)
    }

    /// Get fee estimates for multiple targets
    pub fn get_all_estimates(&self) -> FeeEstimates {
        FeeEstimates {
            high_priority: self.estimate_high_priority(),
            normal: self.estimate_normal(),
            low_priority: self.estimate_low_priority(),
            economy: self.estimate_economy(),
        }
    }

    /// Get mempool pressure (0-100)
    fn get_mempool_pressure(&self) -> u64 {
        // Simple heuristic: more txs = higher pressure
        let count = self.mempool_rates.len();
        match count {
            0..=100 => 0,
            101..=500 => 10,
            501..=1000 => 25,
            1001..=5000 => 50,
            _ => 75,
        }
    }

    fn estimate_from_mempool(&self, target_blocks: u32) -> FeeRate {
        if self.mempool_rates.is_empty() {
            return FeeRate::default();
        }

        let n = self.mempool_rates.len();
        let percentile = match target_blocks {
            0..=1 => 90,
            2..=3 => 75,
            4..=6 => 50,
            7..=12 => 25,
            _ => 10,
        };

        let idx = (n * percentile / 100).min(n - 1);
        self.mempool_rates[idx]
    }
}

// =============================================================================
// Fee Estimates Result
// =============================================================================

/// Collection of fee estimates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeEstimates {
    /// Fee for next block confirmation
    pub high_priority: FeeRate,
    /// Fee for ~3 block confirmation
    pub normal: FeeRate,
    /// Fee for ~6 block confirmation
    pub low_priority: FeeRate,
    /// Fee for background/economy transactions
    pub economy: FeeRate,
}

impl FeeEstimates {
    /// Get fee for a given priority level
    pub fn for_priority(&self, priority: Priority) -> FeeRate {
        match priority {
            Priority::High => self.high_priority,
            Priority::Normal => self.normal,
            Priority::Low => self.low_priority,
            Priority::Economy => self.economy,
        }
    }
}

/// Transaction priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    High,
    Normal,
    Low,
    Economy,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fee_rate() {
        let rate = FeeRate::from_sat_per_byte(10);
        assert_eq!(rate.fee_for_size(250), 2500);

        let rate2 = FeeRate::from_fee_and_size(1000, 200);
        assert_eq!(rate2.0, 5);
    }

    #[test]
    fn test_block_fee_stats() {
        let tx_data = vec![
            (100, 200),  // 0.5 sat/byte
            (500, 250),  // 2 sat/byte
            (1000, 200), // 5 sat/byte
            (2000, 200), // 10 sat/byte
        ];

        let stats = BlockFeeStats::from_transactions(1, &tx_data);

        assert_eq!(stats.tx_count, 4);
        assert_eq!(stats.total_fees, 3600);
        assert!(stats.min_fee_rate.0 <= stats.median_fee_rate.0);
        assert!(stats.median_fee_rate.0 <= stats.max_fee_rate.0);
    }

    #[test]
    fn test_fee_estimator() {
        let mut estimator = FeeEstimator::new();

        // Add some block history
        for i in 0..10 {
            let tx_data: Vec<(u64, usize)> = (0..100)
                .map(|j| ((j + i * 10 + 1) as u64 * 10, 200))
                .collect();
            let stats = BlockFeeStats::from_transactions(i as u64, &tx_data);
            estimator.add_block(stats);
        }

        let estimates = estimator.get_all_estimates();
        assert!(estimates.high_priority.0 >= estimates.normal.0);
        assert!(estimates.normal.0 >= estimates.low_priority.0);
    }

    #[test]
    fn test_mempool_estimation() {
        let mut estimator = FeeEstimator::new();

        let tx_data: Vec<(u64, usize)> = (1..=100).map(|i| (i as u64 * 10, 200)).collect();
        estimator.update_mempool(&tx_data);

        let high = estimator.estimate_high_priority();
        let low = estimator.estimate_economy();

        assert!(high.0 > low.0);
    }
}
