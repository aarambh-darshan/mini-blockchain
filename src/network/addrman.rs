//! Address Manager (AddrMan) for peer discovery
//!
//! Bitcoin-style address manager that maintains:
//! - New addresses: Recently heard but not yet connected
//! - Tried addresses: Successfully connected peers
//!
//! Uses bucketed storage for eclipse attack resistance.

use crate::network::message::{NetAddr, ServiceFlags, MAX_ADDR_PER_MESSAGE};
use rand::Rng;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

// =============================================================================
// Constants
// =============================================================================

/// Number of buckets for new addresses
const NEW_BUCKET_COUNT: usize = 256;

/// Number of entries per new bucket
const NEW_BUCKET_SIZE: usize = 64;

/// Number of buckets for tried addresses
const TRIED_BUCKET_COUNT: usize = 64;

/// Number of entries per tried bucket
const TRIED_BUCKET_SIZE: usize = 64;

/// Maximum age for addresses (30 days)
const MAX_ADDR_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// Minimum time between address relay (24 hours)
const ADDR_RELAY_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// How often to save address database
const SAVE_INTERVAL: Duration = Duration::from_secs(15 * 60);

// =============================================================================
// Address Entry
// =============================================================================

/// Extended address entry with metadata
#[derive(Debug, Clone)]
pub struct AddrEntry {
    /// The network address
    pub addr: NetAddr,
    /// Source peer that told us about this address
    pub source: Option<String>,
    /// Number of connection attempts
    pub attempts: u32,
    /// Number of successful connections
    pub successes: u32,
    /// Last successful connection time
    pub last_success: Option<Instant>,
    /// Last connection attempt time
    pub last_attempt: Option<Instant>,
    /// Is this address in the tried table?
    pub in_tried: bool,
    /// Reference count (how many sources reported this)
    pub ref_count: u32,
}

impl AddrEntry {
    pub fn new(addr: NetAddr, source: Option<String>) -> Self {
        Self {
            addr,
            source,
            attempts: 0,
            successes: 0,
            last_success: None,
            last_attempt: None,
            in_tried: false,
            ref_count: 1,
        }
    }

    /// Check if address is currently "terrible" (too many failures)
    pub fn is_terrible(&self) -> bool {
        if self.last_attempt.is_none() {
            return false;
        }

        // Too many attempts with no success
        if self.successes == 0 && self.attempts >= 3 {
            return true;
        }

        // Many recent failures
        if self.attempts > 10 && self.successes == 0 {
            return true;
        }

        false
    }

    /// Get the chance of being selected (higher = more likely)
    pub fn get_chance(&self) -> f64 {
        let mut chance = 1.0;

        // Reduce chance based on failures
        if self.attempts > 0 && self.successes == 0 {
            chance *= 0.5_f64.powi(self.attempts as i32);
        }

        // Increase chance if we've connected before
        if self.successes > 0 {
            chance *= 2.0;
        }

        chance.max(0.001)
    }

    /// Record a connection attempt
    pub fn attempt(&mut self) {
        self.attempts += 1;
        self.last_attempt = Some(Instant::now());
    }

    /// Record a successful connection
    pub fn good(&mut self) {
        self.successes += 1;
        self.last_success = Some(Instant::now());
        self.attempts = 0; // Reset failure count on success
    }
}

// =============================================================================
// Address Manager
// =============================================================================

/// Bitcoin-style address manager for peer discovery
#[derive(Debug)]
pub struct AddrManager {
    /// Map from address string to entry
    by_addr: HashMap<String, AddrEntry>,

    /// New table buckets (recently heard addresses)
    new_table: Vec<Vec<String>>,

    /// Tried table buckets (successfully connected addresses)
    tried_table: Vec<Vec<String>>,

    /// Addresses we're currently connected to
    connected: HashSet<String>,

    /// List of DNS seeds
    dns_seeds: Vec<String>,

    /// Our own external address (if known)
    local_addr: Option<NetAddr>,

    /// Random key for bucket assignment
    key: u64,
}

impl AddrManager {
    /// Create a new address manager
    pub fn new() -> Self {
        Self {
            by_addr: HashMap::new(),
            new_table: vec![Vec::with_capacity(NEW_BUCKET_SIZE); NEW_BUCKET_COUNT],
            tried_table: vec![Vec::with_capacity(TRIED_BUCKET_SIZE); TRIED_BUCKET_COUNT],
            connected: HashSet::new(),
            dns_seeds: Vec::new(),
            local_addr: None,
            key: rand::thread_rng().gen(),
        }
    }

    /// Create with DNS seeds
    pub fn with_seeds(seeds: Vec<String>) -> Self {
        let mut mgr = Self::new();
        mgr.dns_seeds = seeds;
        mgr
    }

    /// Get number of addresses
    pub fn size(&self) -> usize {
        self.by_addr.len()
    }

    /// Get number of new addresses
    pub fn new_count(&self) -> usize {
        self.by_addr.values().filter(|e| !e.in_tried).count()
    }

    /// Get number of tried addresses
    pub fn tried_count(&self) -> usize {
        self.by_addr.values().filter(|e| e.in_tried).count()
    }

    /// Add a new address
    pub fn add(&mut self, addr: NetAddr, source: Option<String>) -> bool {
        let key = addr.to_addr_string();

        // Don't add unroutable addresses
        if !addr.is_routable() {
            return false;
        }

        // Don't add if we're already connected
        if self.connected.contains(&key) {
            return false;
        }

        // Check if already exists
        if let Some(entry) = self.by_addr.get_mut(&key) {
            entry.ref_count += 1;
            // Update timestamp if newer
            if addr.timestamp > entry.addr.timestamp {
                entry.addr.timestamp = addr.timestamp;
            }
            return false;
        }

        // Add new entry
        let entry = AddrEntry::new(addr, source.clone());
        self.by_addr.insert(key.clone(), entry);

        // Add to new table
        let bucket = self.get_new_bucket(&key, source.as_deref().unwrap_or(""));
        if self.new_table[bucket].len() < NEW_BUCKET_SIZE {
            self.new_table[bucket].push(key);
        }

        true
    }

    /// Add multiple addresses (from an Addr message)
    pub fn add_many(&mut self, addrs: Vec<NetAddr>, source: Option<String>) -> usize {
        let mut added = 0;
        for addr in addrs.into_iter().take(MAX_ADDR_PER_MESSAGE) {
            if self.add(addr, source.clone()) {
                added += 1;
            }
        }
        added
    }

    /// Mark address as good (successful connection)
    pub fn good(&mut self, addr: &str) {
        if let Some(entry) = self.by_addr.get_mut(addr) {
            entry.good();

            // Move to tried table if not already there
            if !entry.in_tried {
                self.make_tried(addr);
            }
        }
    }

    /// Mark address as attempted
    pub fn attempt(&mut self, addr: &str) {
        if let Some(entry) = self.by_addr.get_mut(addr) {
            entry.attempt();
        }
    }

    /// Mark address as connected
    pub fn connected(&mut self, addr: &str) {
        self.connected.insert(addr.to_string());
    }

    /// Mark address as disconnected
    pub fn disconnected(&mut self, addr: &str) {
        self.connected.remove(addr);
    }

    /// Select an address to connect to
    pub fn select(&self, new_only: bool) -> Option<NetAddr> {
        let mut rng = rand::thread_rng();

        // Decide whether to pick from new or tried table
        let use_new = if new_only {
            true
        } else {
            let new_count = self.new_count();
            let tried_count = self.tried_count();

            if tried_count == 0 {
                true
            } else if new_count == 0 {
                false
            } else {
                // 50% chance each
                rng.gen::<bool>()
            }
        };

        // Collect eligible addresses
        let eligible: Vec<_> = self
            .by_addr
            .iter()
            .filter(|(k, e)| {
                !self.connected.contains(*k) && !e.is_terrible() && (use_new == !e.in_tried)
            })
            .collect();

        if eligible.is_empty() {
            return None;
        }

        // Weighted random selection based on chance
        let total_weight: f64 = eligible.iter().map(|(_, e)| e.get_chance()).sum();
        let mut target = rng.gen::<f64>() * total_weight;

        for (_, entry) in &eligible {
            target -= entry.get_chance();
            if target <= 0.0 {
                return Some(entry.addr.clone());
            }
        }

        // Fallback to first
        eligible.first().map(|(_, e)| e.addr.clone())
    }

    /// Get addresses to send in response to GetAddr
    pub fn get_addr(&self, count: usize) -> Vec<NetAddr> {
        let mut rng = rand::thread_rng();
        let count = count.min(MAX_ADDR_PER_MESSAGE);

        // Collect all routable, non-terrible addresses
        let mut addrs: Vec<_> = self
            .by_addr
            .values()
            .filter(|e| !e.is_terrible())
            .map(|e| e.addr.clone())
            .collect();

        // Shuffle and take requested count
        for i in (1..addrs.len()).rev() {
            let j = rng.gen_range(0..=i);
            addrs.swap(i, j);
        }

        addrs.truncate(count);
        addrs
    }

    /// Resolve DNS seeds and add addresses
    pub async fn resolve_seeds(&mut self) -> usize {
        use tokio::net::lookup_host;

        let mut added = 0;

        for seed in &self.dns_seeds.clone() {
            log::info!("Resolving DNS seed: {}", seed);

            match lookup_host(seed).await {
                Ok(addrs) => {
                    for socket_addr in addrs {
                        let addr = NetAddr::new(
                            socket_addr.ip().to_string(),
                            socket_addr.port(),
                            ServiceFlags::NODE_NETWORK,
                        );
                        if self.add(addr, Some(seed.clone())) {
                            added += 1;
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Failed to resolve DNS seed {}: {}", seed, e);
                }
            }
        }

        log::info!("Resolved {} addresses from DNS seeds", added);
        added
    }

    /// Set our local address
    pub fn set_local(&mut self, addr: NetAddr) {
        self.local_addr = Some(addr);
    }

    /// Get our local address
    pub fn local(&self) -> Option<&NetAddr> {
        self.local_addr.as_ref()
    }

    // =========================================================================
    // Private helpers
    // =========================================================================

    /// Get bucket index for new table
    fn get_new_bucket(&self, addr: &str, source: &str) -> usize {
        let hash = self.hash_addr(addr, source);
        (hash as usize) % NEW_BUCKET_COUNT
    }

    /// Get bucket index for tried table
    fn get_tried_bucket(&self, addr: &str) -> usize {
        let hash = self.hash_addr(addr, "");
        (hash as usize) % TRIED_BUCKET_COUNT
    }

    /// Hash address for bucket assignment
    fn hash_addr(&self, addr: &str, source: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.key.hash(&mut hasher);
        addr.hash(&mut hasher);
        source.hash(&mut hasher);
        hasher.finish()
    }

    /// Move address from new to tried table
    fn make_tried(&mut self, addr: &str) {
        // First check if already in tried
        let already_tried = self.by_addr.get(addr).map(|e| e.in_tried).unwrap_or(true);
        if already_tried {
            return;
        }

        // Remove from new table
        for bucket in &mut self.new_table {
            bucket.retain(|a| a != addr);
        }

        // Add to tried table
        let bucket = self.get_tried_bucket(addr);
        if self.tried_table[bucket].len() < TRIED_BUCKET_SIZE {
            self.tried_table[bucket].push(addr.to_string());
        }

        // Mark as tried
        if let Some(entry) = self.by_addr.get_mut(addr) {
            entry.in_tried = true;
        }
    }
}

impl Default for AddrManager {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_address() {
        let mut mgr = AddrManager::new();

        let addr = NetAddr::new("8.8.8.8".to_string(), 8333, ServiceFlags::NODE_NETWORK);
        assert!(mgr.add(addr.clone(), None));

        // Adding same address again should return false
        assert!(!mgr.add(addr, None));

        assert_eq!(mgr.size(), 1);
        assert_eq!(mgr.new_count(), 1);
        assert_eq!(mgr.tried_count(), 0);
    }

    #[test]
    fn test_unroutable_rejected() {
        let mut mgr = AddrManager::new();

        let addr = NetAddr::new("127.0.0.1".to_string(), 8333, ServiceFlags::NODE_NETWORK);
        assert!(!mgr.add(addr, None));

        assert_eq!(mgr.size(), 0);
    }

    #[test]
    fn test_good_moves_to_tried() {
        let mut mgr = AddrManager::new();

        let addr = NetAddr::new("8.8.8.8".to_string(), 8333, ServiceFlags::NODE_NETWORK);
        mgr.add(addr, None);

        mgr.good("8.8.8.8:8333");

        assert_eq!(mgr.new_count(), 0);
        assert_eq!(mgr.tried_count(), 1);
    }

    #[test]
    fn test_select_address() {
        let mut mgr = AddrManager::new();

        let addr1 = NetAddr::new("1.1.1.1".to_string(), 8333, ServiceFlags::NODE_NETWORK);
        let addr2 = NetAddr::new("8.8.8.8".to_string(), 8333, ServiceFlags::NODE_NETWORK);

        mgr.add(addr1, None);
        mgr.add(addr2, None);

        let selected = mgr.select(false);
        assert!(selected.is_some());
    }

    #[test]
    fn test_get_addr() {
        let mut mgr = AddrManager::new();

        for i in 1..10 {
            let addr = NetAddr::new(format!("8.8.8.{}", i), 8333, ServiceFlags::NODE_NETWORK);
            mgr.add(addr, None);
        }

        let addrs = mgr.get_addr(5);
        assert_eq!(addrs.len(), 5);
    }
}
