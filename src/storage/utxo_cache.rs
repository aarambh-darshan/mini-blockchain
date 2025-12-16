//! UTXO Cache for optimized transaction validation
//!
//! Provides a memory-efficient cache for the UTXO set with:
//! - LRU eviction for memory management
//! - Dirty tracking for persistence
//! - Batch updates for efficiency

use crate::core::transaction::UTXO;
use std::collections::{HashMap, VecDeque};

// =============================================================================
// Constants
// =============================================================================

/// Default maximum cache entries
pub const DEFAULT_CACHE_SIZE: usize = 100_000;

/// Flush threshold (number of dirty entries before auto-flush)
pub const FLUSH_THRESHOLD: usize = 10_000;

// =============================================================================
// Cache Entry
// =============================================================================

/// A cached UTXO entry with metadata
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// The UTXO data
    pub utxo: UTXO,
    /// Whether this entry has been modified since last flush
    pub dirty: bool,
    /// Whether this entry is marked for deletion
    pub deleted: bool,
}

impl CacheEntry {
    pub fn new(utxo: UTXO) -> Self {
        Self {
            utxo,
            dirty: true,
            deleted: false,
        }
    }

    pub fn clean(utxo: UTXO) -> Self {
        Self {
            utxo,
            dirty: false,
            deleted: false,
        }
    }
}

// =============================================================================
// UTXO Cache
// =============================================================================

/// Memory-efficient UTXO cache with LRU eviction
#[derive(Debug)]
pub struct UtxoCache {
    /// Cached entries by outpoint (tx_id:output_index)
    entries: HashMap<String, CacheEntry>,
    /// LRU order (front = most recently used)
    lru_order: VecDeque<String>,
    /// Maximum cache size
    max_size: usize,
    /// Number of dirty entries
    dirty_count: usize,
    /// Cache statistics
    stats: CacheStats,
}

impl UtxoCache {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CACHE_SIZE)
    }

    pub fn with_capacity(max_size: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(max_size),
            lru_order: VecDeque::with_capacity(max_size),
            max_size,
            dirty_count: 0,
            stats: CacheStats::default(),
        }
    }

    /// Get a UTXO from cache (clones the UTXO to avoid borrow issues)
    pub fn get(&mut self, tx_id: &str, output_index: u32) -> Option<UTXO> {
        let key = format!("{}:{}", tx_id, output_index);

        let result = self.entries.get(&key).and_then(|entry| {
            if entry.deleted {
                None
            } else {
                Some(entry.utxo.clone())
            }
        });

        if result.is_some() {
            self.stats.hits += 1;
            self.touch_lru(&key);
        } else {
            self.stats.misses += 1;
        }

        result
    }

    /// Get a UTXO reference from cache (no LRU update, immutable)
    pub fn peek(&self, tx_id: &str, output_index: u32) -> Option<&UTXO> {
        let key = format!("{}:{}", tx_id, output_index);
        self.entries.get(&key).and_then(|entry| {
            if entry.deleted {
                None
            } else {
                Some(&entry.utxo)
            }
        })
    }

    /// Insert a UTXO into cache
    pub fn insert(&mut self, utxo: UTXO) {
        let key = format!("{}:{}", utxo.tx_id, utxo.output_index);

        // Evict if at capacity
        if self.entries.len() >= self.max_size {
            self.evict_one();
        }

        let entry = CacheEntry::new(utxo);
        if entry.dirty {
            self.dirty_count += 1;
        }

        self.entries.insert(key.clone(), entry);
        self.lru_order.push_front(key);
        self.stats.inserts += 1;
    }

    /// Mark a UTXO as spent (soft delete)
    pub fn spend(&mut self, tx_id: &str, output_index: u32) -> bool {
        let key = format!("{}:{}", tx_id, output_index);

        if let Some(entry) = self.entries.get_mut(&key) {
            if !entry.deleted {
                entry.deleted = true;
                entry.dirty = true;
                self.dirty_count += 1;
                self.stats.deletes += 1;
                return true;
            }
        }
        false
    }

    /// Remove a UTXO from cache entirely
    pub fn remove(&mut self, tx_id: &str, output_index: u32) -> Option<UTXO> {
        let key = format!("{}:{}", tx_id, output_index);

        if let Some(entry) = self.entries.remove(&key) {
            self.lru_order.retain(|k| k != &key);
            if entry.dirty {
                self.dirty_count = self.dirty_count.saturating_sub(1);
            }
            if !entry.deleted {
                return Some(entry.utxo);
            }
        }
        None
    }

    /// Check if UTXO exists and is unspent
    pub fn contains(&self, tx_id: &str, output_index: u32) -> bool {
        let key = format!("{}:{}", tx_id, output_index);
        self.entries.get(&key).map(|e| !e.deleted).unwrap_or(false)
    }

    /// Get all UTXOs for an address
    pub fn get_by_address(&self, address: &str) -> Vec<&UTXO> {
        self.entries
            .values()
            .filter(|e| !e.deleted && e.utxo.output.recipient == address)
            .map(|e| &e.utxo)
            .collect()
    }

    /// Get balance for an address
    pub fn get_balance(&self, address: &str) -> u64 {
        self.get_by_address(address)
            .iter()
            .map(|u| u.output.amount)
            .sum()
    }

    /// Get dirty entries for flushing
    pub fn get_dirty(&self) -> Vec<(&String, &CacheEntry)> {
        self.entries.iter().filter(|(_, e)| e.dirty).collect()
    }

    /// Mark all entries as clean
    pub fn mark_clean(&mut self) {
        for entry in self.entries.values_mut() {
            entry.dirty = false;
        }
        self.dirty_count = 0;
    }

    /// Remove deleted entries
    pub fn compact(&mut self) {
        self.entries.retain(|_, e| !e.deleted);
        self.lru_order.retain(|k| self.entries.contains_key(k));
    }

    /// Should we flush based on dirty count?
    pub fn should_flush(&self) -> bool {
        self.dirty_count >= FLUSH_THRESHOLD
    }

    /// Get cache statistics
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// Get cache size
    pub fn len(&self) -> usize {
        self.entries.values().filter(|e| !e.deleted).count()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear the cache
    pub fn clear(&mut self) {
        self.entries.clear();
        self.lru_order.clear();
        self.dirty_count = 0;
    }

    // Private helper methods

    fn touch_lru(&mut self, key: &str) {
        // Remove from current position
        self.lru_order.retain(|k| k != key);
        // Add to front
        self.lru_order.push_front(key.to_string());
    }

    fn evict_one(&mut self) {
        // Evict least recently used (from back)
        while let Some(key) = self.lru_order.pop_back() {
            if let Some(entry) = self.entries.get(&key) {
                // Don't evict dirty entries if possible
                if !entry.dirty {
                    self.entries.remove(&key);
                    self.stats.evictions += 1;
                    return;
                }
                // Put it back and try next
                self.lru_order.push_front(key);
            }
        }

        // All entries are dirty, force evict anyway
        if let Some(key) = self.lru_order.pop_back() {
            if let Some(entry) = self.entries.remove(&key) {
                if entry.dirty {
                    self.dirty_count = self.dirty_count.saturating_sub(1);
                }
                self.stats.evictions += 1;
            }
        }
    }
}

impl Default for UtxoCache {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Cache Statistics
// =============================================================================

/// Cache performance statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub inserts: u64,
    pub deletes: u64,
    pub evictions: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::transaction::TransactionOutput;

    fn make_utxo(tx_id: &str, index: u32, amount: u64, recipient: &str) -> UTXO {
        UTXO {
            tx_id: tx_id.to_string(),
            output_index: index,
            output: TransactionOutput {
                amount,
                recipient: recipient.to_string(),
            },
        }
    }

    #[test]
    fn test_cache_insert_get() {
        let mut cache = UtxoCache::new();

        let utxo = make_utxo("tx1", 0, 100, "addr1");
        cache.insert(utxo.clone());

        let result = cache.get("tx1", 0);
        assert!(result.is_some());
        assert_eq!(result.unwrap().output.amount, 100);
    }

    #[test]
    fn test_cache_spend() {
        let mut cache = UtxoCache::new();

        cache.insert(make_utxo("tx1", 0, 100, "addr1"));
        assert!(cache.contains("tx1", 0));

        cache.spend("tx1", 0);
        assert!(!cache.contains("tx1", 0));
    }

    #[test]
    fn test_cache_balance() {
        let mut cache = UtxoCache::new();

        cache.insert(make_utxo("tx1", 0, 100, "addr1"));
        cache.insert(make_utxo("tx2", 0, 50, "addr1"));
        cache.insert(make_utxo("tx3", 0, 25, "addr2"));

        assert_eq!(cache.get_balance("addr1"), 150);
        assert_eq!(cache.get_balance("addr2"), 25);
    }

    #[test]
    fn test_cache_eviction() {
        let mut cache = UtxoCache::with_capacity(3);

        cache.insert(make_utxo("tx1", 0, 100, "addr1"));
        cache.mark_clean(); // Make it evictable
        cache.insert(make_utxo("tx2", 0, 100, "addr1"));
        cache.insert(make_utxo("tx3", 0, 100, "addr1"));
        cache.insert(make_utxo("tx4", 0, 100, "addr1")); // Should trigger eviction

        assert_eq!(cache.entries.len(), 3);
        assert_eq!(cache.stats.evictions, 1);
    }
}
