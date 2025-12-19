//! Peer Discovery for P2P networking
//!
//! Coordinates peer discovery through:
//! - DNS seed resolution
//! - Address exchange with peers (Addr/GetAddr)
//! - Automatic peer selection

use crate::network::addrman::AddrManager;
use crate::network::message::{Message, NetAddr, ServiceFlags};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

// =============================================================================
// Constants
// =============================================================================

/// How often to request addresses from peers
const GETADDR_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60); // 24 hours

/// How often to try connecting to new peers
const CONNECT_INTERVAL: Duration = Duration::from_secs(30);

/// Minimum number of outbound connections to maintain
const MIN_OUTBOUND_CONNECTIONS: usize = 8;

/// Maximum addresses to request per GetAddr
const MAX_GETADDR_RESPONSE: usize = 1000;

// =============================================================================
// Default DNS Seeds
// =============================================================================

/// Default DNS seeds for the network (placeholder - these would be real seeds in production)
pub const DEFAULT_DNS_SEEDS: &[&str] = &[
    // These are placeholders - in a real network, these would be 
    // DNS hostnames that resolve to known good nodes
    // "seed.minichain.example.com:8333",
    // "seed2.minichain.example.com:8333",
];

// =============================================================================
// Peer Discovery
// =============================================================================

/// Peer discovery coordinator
pub struct PeerDiscovery {
    /// Address manager
    addr_manager: Arc<RwLock<AddrManager>>,
    /// Our service flags
    services: ServiceFlags,
    /// Whether discovery is active
    active: RwLock<bool>,
}

impl PeerDiscovery {
    /// Create a new peer discovery instance
    pub fn new(services: ServiceFlags) -> Self {
        Self {
            addr_manager: Arc::new(RwLock::new(AddrManager::new())),
            services,
            active: RwLock::new(false),
        }
    }

    /// Create with custom DNS seeds
    pub fn with_seeds(services: ServiceFlags, seeds: Vec<String>) -> Self {
        Self {
            addr_manager: Arc::new(RwLock::new(AddrManager::with_seeds(seeds))),
            services,
            active: RwLock::new(false),
        }
    }

    /// Get the address manager
    pub fn addr_manager(&self) -> Arc<RwLock<AddrManager>> {
        Arc::clone(&self.addr_manager)
    }

    /// Start peer discovery
    pub async fn start(&self) {
        let mut active = self.active.write().await;
        if *active {
            return;
        }
        *active = true;
        drop(active);

        log::info!("Starting peer discovery");

        // Resolve DNS seeds
        let mut mgr = self.addr_manager.write().await;
        let added = mgr.resolve_seeds().await;
        log::info!("Discovered {} addresses from DNS seeds", added);
    }

    /// Stop peer discovery
    pub async fn stop(&self) {
        let mut active = self.active.write().await;
        *active = false;
        log::info!("Stopped peer discovery");
    }

    /// Handle incoming Addr message
    pub async fn handle_addr(&self, addrs: Vec<NetAddr>, source: String) -> usize {
        let mut mgr = self.addr_manager.write().await;
        let added = mgr.add_many(addrs, Some(source));
        log::debug!("Added {} addresses from peer", added);
        added
    }

    /// Handle GetAddr request - return addresses to send
    pub async fn handle_getaddr(&self, max: usize) -> Vec<NetAddr> {
        let mgr = self.addr_manager.read().await;
        mgr.get_addr(max.min(MAX_GETADDR_RESPONSE))
    }

    /// Get an address to connect to
    pub async fn select_addr(&self, new_only: bool) -> Option<NetAddr> {
        let mgr = self.addr_manager.read().await;
        mgr.select(new_only)
    }

    /// Mark address as good (successful connection)
    pub async fn mark_good(&self, addr: &str) {
        let mut mgr = self.addr_manager.write().await;
        mgr.good(addr);
    }

    /// Mark address as attempted
    pub async fn mark_attempt(&self, addr: &str) {
        let mut mgr = self.addr_manager.write().await;
        mgr.attempt(addr);
    }

    /// Mark address as connected
    pub async fn mark_connected(&self, addr: &str) {
        let mut mgr = self.addr_manager.write().await;
        mgr.connected(addr);
    }

    /// Mark address as disconnected
    pub async fn mark_disconnected(&self, addr: &str) {
        let mut mgr = self.addr_manager.write().await;
        mgr.disconnected(addr);
    }

    /// Add a peer address manually
    pub async fn add_addr(&self, addr: NetAddr) -> bool {
        let mut mgr = self.addr_manager.write().await;
        mgr.add(addr, None)
    }

    /// Set our local address (discovered via UPnP or config)
    pub async fn set_local_addr(&self, addr: NetAddr) {
        let mut mgr = self.addr_manager.write().await;
        mgr.set_local(addr);
    }

    /// Get our local address
    pub async fn local_addr(&self) -> Option<NetAddr> {
        let mgr = self.addr_manager.read().await;
        mgr.local().cloned()
    }

    /// Get discovery statistics
    pub async fn stats(&self) -> DiscoveryStats {
        let mgr = self.addr_manager.read().await;
        DiscoveryStats {
            total: mgr.size(),
            new: mgr.new_count(),
            tried: mgr.tried_count(),
        }
    }

    /// Create GetAddr message
    pub fn create_getaddr_message() -> Message {
        Message::GetAddr
    }

    /// Create Addr message with our addresses
    pub async fn create_addr_message(&self, count: usize) -> Message {
        let addrs = self.handle_getaddr(count).await;
        Message::Addr(addrs)
    }
}

// =============================================================================
// Discovery Stats
// =============================================================================

/// Statistics about peer discovery
#[derive(Debug, Clone)]
pub struct DiscoveryStats {
    /// Total known addresses
    pub total: usize,
    /// Addresses in new table
    pub new: usize,
    /// Addresses in tried table
    pub tried: usize,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_discovery_creation() {
        let discovery = PeerDiscovery::new(ServiceFlags::NODE_NETWORK);
        let stats = discovery.stats().await;

        assert_eq!(stats.total, 0);
        assert_eq!(stats.new, 0);
        assert_eq!(stats.tried, 0);
    }

    #[tokio::test]
    async fn test_add_address() {
        let discovery = PeerDiscovery::new(ServiceFlags::NODE_NETWORK);

        let addr = NetAddr::new("8.8.8.8".to_string(), 8333, ServiceFlags::NODE_NETWORK);
        assert!(discovery.add_addr(addr).await);

        let stats = discovery.stats().await;
        assert_eq!(stats.total, 1);
    }

    #[tokio::test]
    async fn test_select_address() {
        let discovery = PeerDiscovery::new(ServiceFlags::NODE_NETWORK);

        let addr = NetAddr::new("8.8.8.8".to_string(), 8333, ServiceFlags::NODE_NETWORK);
        discovery.add_addr(addr).await;

        let selected = discovery.select_addr(false).await;
        assert!(selected.is_some());
    }

    #[tokio::test]
    async fn test_handle_addr() {
        let discovery = PeerDiscovery::new(ServiceFlags::NODE_NETWORK);

        let addrs = vec![
            NetAddr::new("1.1.1.1".to_string(), 8333, ServiceFlags::NODE_NETWORK),
            NetAddr::new("8.8.8.8".to_string(), 8333, ServiceFlags::NODE_NETWORK),
        ];

        let added = discovery.handle_addr(addrs, "peer1".to_string()).await;
        assert_eq!(added, 2);
    }
}
