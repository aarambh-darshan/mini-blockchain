//! P2P Node implementation
//!
//! The main node that orchestrates all networking components.

use crate::core::{Blockchain, Transaction};
use crate::mining::Mempool;
use crate::network::message::{Handshake, Message};
use crate::network::peer::{PeerError, PeerManager};
use crate::network::server::{connect_to_peer, handle_connection, Server};
use crate::network::sync::ChainSync;
use crate::storage::Storage;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// P2P Node configuration
#[derive(Clone)]
pub struct NodeConfig {
    /// Port to listen on
    pub port: u16,
    /// Initial peers to connect to
    pub bootstrap_peers: Vec<String>,
    /// Data directory for blockchain storage
    pub data_dir: std::path::PathBuf,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            port: 8333,
            bootstrap_peers: Vec::new(),
            data_dir: std::path::PathBuf::from(".blockchain_data"),
        }
    }
}

/// The main P2P node
pub struct Node {
    pub config: NodeConfig,
    pub blockchain: Arc<RwLock<Blockchain>>,
    pub mempool: Arc<RwLock<Mempool>>,
    pub peer_manager: Arc<PeerManager>,
    pub chain_sync: Arc<ChainSync>,
    pub storage: Arc<Storage>,
    shutdown_tx: Option<mpsc::Sender<()>>,
    /// Message channel sender - set after start() is called
    message_tx: Option<mpsc::Sender<(SocketAddr, Message)>>,
}

impl Node {
    /// Create a new node
    pub async fn new(config: NodeConfig) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Initialize storage
        let storage_config = crate::storage::StorageConfig {
            data_dir: config.data_dir.clone(),
            ..Default::default()
        };
        let storage = Arc::new(Storage::new(storage_config)?);

        // Load or create blockchain
        let blockchain = if storage.exists() {
            log::info!("Loading existing blockchain...");
            Arc::new(RwLock::new(storage.load()?))
        } else {
            log::info!("Creating new blockchain...");
            let chain = Blockchain::new();
            storage.save(&chain)?;
            Arc::new(RwLock::new(chain))
        };

        let mempool = Arc::new(RwLock::new(Mempool::new()));
        let peer_manager = Arc::new(PeerManager::new(config.port));
        let chain_sync = Arc::new(ChainSync::new(blockchain.clone(), peer_manager.clone()));

        Ok(Self {
            config,
            blockchain,
            mempool,
            peer_manager,
            chain_sync,
            storage,
            shutdown_tx: None,
            message_tx: None,
        })
    }

    /// Create a new node with shared blockchain and mempool (for API integration)
    /// This allows the API server and P2P node to share the same blockchain instance,
    /// so blocks mined via API are automatically visible to the P2P network.
    pub fn new_with_shared(
        config: NodeConfig,
        blockchain: Arc<RwLock<Blockchain>>,
        mempool: Arc<RwLock<Mempool>>,
        storage: Arc<Storage>,
    ) -> Self {
        let peer_manager = Arc::new(PeerManager::new(config.port));
        let chain_sync = Arc::new(ChainSync::new(blockchain.clone(), peer_manager.clone()));

        Self {
            config,
            blockchain,
            mempool,
            peer_manager,
            chain_sync,
            storage,
            shutdown_tx: None,
            message_tx: None,
        }
    }

    /// Create a new node with shared blockchain, mempool, AND peer_manager
    /// This is the full integration mode - API and P2P share everything including
    /// the peer manager, so blocks mined via API are automatically broadcast.
    pub fn new_with_shared_and_peer_manager(
        config: NodeConfig,
        blockchain: Arc<RwLock<Blockchain>>,
        mempool: Arc<RwLock<Mempool>>,
        storage: Arc<Storage>,
        peer_manager: Arc<PeerManager>,
    ) -> Self {
        let chain_sync = Arc::new(ChainSync::new(blockchain.clone(), peer_manager.clone()));

        Self {
            config,
            blockchain,
            mempool,
            peer_manager,
            chain_sync,
            storage,
            shutdown_tx: None,
            message_tx: None,
        }
    }

    /// Get the peer manager (for broadcasting blocks from external sources)
    pub fn peer_manager(&self) -> Arc<PeerManager> {
        self.peer_manager.clone()
    }

    /// Start the node
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx);

        // Start server
        let server = Server::bind(self.config.port).await?;
        log::info!("Node started on port {}", self.config.port);

        // Create message channel and store it for use by connect_to
        let (message_tx, mut message_rx) = mpsc::channel::<(SocketAddr, Message)>(1000);
        self.message_tx = Some(message_tx.clone());

        // Clone for tasks
        let peer_manager = self.peer_manager.clone();
        let blockchain = self.blockchain.clone();
        let _mempool = self.mempool.clone();
        let _chain_sync = self.chain_sync.clone();
        let _storage = self.storage.clone();
        let port = self.config.port;

        // Spawn connection acceptor
        let accept_peer_manager = peer_manager.clone();
        let accept_message_tx = message_tx.clone();
        let accept_blockchain = blockchain.clone();
        tokio::spawn(async move {
            loop {
                match server.accept().await {
                    Ok((stream, addr)) => {
                        log::info!("Incoming connection from {}", addr);

                        let handshake = {
                            let chain = accept_blockchain.read().await;
                            Handshake::new(chain.height(), chain.latest_block().hash.clone(), port)
                        };

                        let pm = accept_peer_manager.clone();
                        let tx = accept_message_tx.clone();
                        tokio::spawn(async move {
                            if let Err(e) =
                                handle_connection(stream, addr, pm, handshake, tx, false).await
                            {
                                log::warn!("Connection error with {}: {}", addr, e);
                            }
                        });
                    }
                    Err(e) => {
                        log::error!("Accept error: {}", e);
                    }
                }
            }
        });

        // Connect to bootstrap peers
        for peer_addr in &self.config.bootstrap_peers {
            let _ = self.connect_to(peer_addr).await;
        }

        // Message handling loop
        loop {
            tokio::select! {
                Some((from, msg)) = message_rx.recv() => {
                    self.handle_message(from, msg).await;
                }
                _ = shutdown_rx.recv() => {
                    log::info!("Node shutting down...");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Connect to a peer
    pub async fn connect_to(&self, addr: &str) -> Result<(), PeerError> {
        log::info!("Connecting to peer: {}", addr);

        let (stream, peer_addr) = connect_to_peer(addr).await?;

        let handshake = {
            let chain = self.blockchain.read().await;
            Handshake::new(
                chain.height(),
                chain.latest_block().hash.clone(),
                self.config.port,
            )
        };

        // Use the stored message_tx so messages go to the main handler
        // If start() hasn't been called yet, fall back to a dummy channel
        let message_tx = self.message_tx.clone().unwrap_or_else(|| {
            log::warn!(
                "connect_to called before start() - messages from this peer won't be processed"
            );
            mpsc::channel::<(SocketAddr, Message)>(100).0
        });

        let pm = self.peer_manager.clone();
        tokio::spawn(async move {
            if let Err(e) =
                handle_connection(stream, peer_addr, pm, handshake, message_tx, true).await
            {
                log::warn!("Connection error with {}: {}", peer_addr, e);
            }
        });

        Ok(())
    }

    /// Handle incoming messages
    async fn handle_message(&self, from: SocketAddr, msg: Message) {
        log::debug!("Received {} from {}", msg.type_name(), from);

        match msg {
            Message::Version(version) => {
                // Handle version message (new protocol)
                if let Err(e) = self.peer_manager.update_peer_version(&from, &version).await {
                    log::warn!("Version incompatible from {}: {}", from, e);
                    return;
                }
                // Send VerAck
                if let Err(e) = self.peer_manager.send_to(&from, Message::VerAck).await {
                    log::warn!("Failed to send VerAck to {}: {}", from, e);
                }
                // Check if we need to sync
                self.chain_sync.check_sync().await;
            }

            Message::VerAck => {
                log::debug!("Received VerAck from {}", from);
            }

            Message::Handshake(handshake) => {
                self.peer_manager.update_peer(&from, &handshake).await;

                // Check if we need to sync
                self.chain_sync.check_sync().await;
            }

            Message::NewBlock(block) => {
                if let Err(e) = self.chain_sync.handle_new_block(block, from).await {
                    log::warn!("Failed to handle new block: {}", e);
                }

                // Save blockchain
                let chain = self.blockchain.read().await;
                if let Err(e) = self.storage.save(&chain) {
                    log::error!("Failed to save blockchain: {}", e);
                }
            }

            Message::NewTransaction(tx) => {
                let chain = self.blockchain.read().await;
                let mut mempool = self.mempool.write().await;

                if mempool.add_transaction(tx.clone(), &chain).is_ok() {
                    // Relay to other peers
                    drop(chain);
                    drop(mempool);
                    self.peer_manager
                        .broadcast_except(Message::NewTransaction(tx), &from)
                        .await;
                }
            }

            Message::GetBlocks {
                start_height,
                count,
            } => {
                let blocks = self.chain_sync.get_blocks(start_height, count).await;
                if let Err(e) = self
                    .peer_manager
                    .send_to(&from, Message::Blocks(blocks))
                    .await
                {
                    log::warn!("Failed to send blocks: {}", e);
                }
            }

            Message::Blocks(blocks) => {
                if let Err(e) = self.chain_sync.handle_blocks(blocks, from).await {
                    log::warn!("Failed to handle blocks: {}", e);
                }

                // Save blockchain
                let chain = self.blockchain.read().await;
                if let Err(e) = self.storage.save(&chain) {
                    log::error!("Failed to save blockchain: {}", e);
                }
            }

            Message::GetHeaders {
                start_height,
                count,
            } => {
                // TODO: Implement headers-first sync
                log::debug!(
                    "GetHeaders from {}: start={}, count={}",
                    from,
                    start_height,
                    count
                );
            }

            Message::Headers(_headers) => {
                // TODO: Implement headers-first sync
                log::debug!("Received headers from {}", from);
            }

            Message::GetPeers => {
                let peers = self.peer_manager.get_known_peers().await;
                if let Err(e) = self
                    .peer_manager
                    .send_to(&from, Message::Peers(peers))
                    .await
                {
                    log::warn!("Failed to send peers: {}", e);
                }
            }

            Message::Peers(peers) => {
                self.peer_manager.add_known_peers(peers).await;
            }

            Message::Ping(nonce) => {
                if let Err(e) = self.peer_manager.send_to(&from, Message::Pong(nonce)).await {
                    log::warn!("Failed to send pong: {}", e);
                }
            }

            Message::Pong(_) => {
                // Peer is alive, nothing to do
            }

            Message::GetHeight => {
                let height = {
                    let chain = self.blockchain.read().await;
                    chain.height()
                };
                if let Err(e) = self
                    .peer_manager
                    .send_to(&from, Message::Height(height))
                    .await
                {
                    log::warn!("Failed to send height: {}", e);
                }
            }

            Message::Height(_) => {
                // Used for sync checking
            }

            Message::Inv(items) => {
                // Handle inventory announcements
                log::debug!("Received {} inventory items from {}", items.len(), from);
                // TODO: Request missing items via GetData
            }

            Message::GetData(items) => {
                // Handle data requests
                log::debug!("GetData request for {} items from {}", items.len(), from);
                // TODO: Send requested blocks/transactions
            }

            Message::NotFound(_items) => {
                log::debug!("NotFound from {}", from);
            }

            Message::Reject(reject) => {
                log::warn!(
                    "Reject from {}: {} - {:?} - {}",
                    from,
                    reject.message,
                    reject.code,
                    reject.reason
                );
            }

            Message::CompactBlock(_) => {
                // TODO: Implement compact block relay
                log::debug!("Received compact block from {}", from);
            }

            Message::GetBlockTxn {
                block_hash,
                indexes,
            } => {
                log::debug!(
                    "GetBlockTxn for {} txs in {} from {}",
                    indexes.len(),
                    block_hash,
                    from
                );
            }

            Message::BlockTxn {
                block_hash,
                transactions,
            } => {
                log::debug!(
                    "BlockTxn with {} txs for {} from {}",
                    transactions.len(),
                    block_hash,
                    from
                );
            }

            Message::FeeFilter(min_fee) => {
                log::debug!("FeeFilter {} from {}", min_fee, from);
            }

            Message::SendCmpct { enable, version } => {
                log::debug!(
                    "SendCmpct enable={} version={} from {}",
                    enable,
                    version,
                    from
                );
            }

            Message::GetAddr => {
                // Respond with known peer addresses
                log::debug!("GetAddr from {}", from);
                let peers = self.peer_manager.get_known_peers().await;
                let addrs: Vec<_> = peers
                    .iter()
                    .filter_map(|p| {
                        crate::network::message::NetAddr::from_addr_str(
                            p,
                            crate::network::message::ServiceFlags::NODE_NETWORK,
                        )
                    })
                    .take(1000)
                    .collect();
                if let Err(e) = self.peer_manager.send_to(&from, Message::Addr(addrs)).await {
                    log::warn!("Failed to send Addr: {}", e);
                }
            }

            Message::Addr(addrs) => {
                // Add received addresses to known peers
                log::debug!("Received {} addresses from {}", addrs.len(), from);
                let addr_strings: Vec<String> = addrs.iter().map(|a| a.to_addr_string()).collect();
                self.peer_manager.add_known_peers(addr_strings).await;
            }
        }
    }

    /// Broadcast a new block to all peers
    pub async fn broadcast_block(&self, block: crate::core::Block) {
        self.peer_manager.broadcast(Message::NewBlock(block)).await;
    }

    /// Broadcast a new transaction to all peers
    pub async fn broadcast_transaction(&self, tx: Transaction) {
        self.peer_manager
            .broadcast(Message::NewTransaction(tx))
            .await;
    }

    /// Get node status
    pub async fn status(&self) -> NodeStatus {
        let chain = self.blockchain.read().await;
        let peers = self.peer_manager.get_all_peer_info().await;
        let mempool = self.mempool.read().await;

        NodeStatus {
            port: self.config.port,
            height: chain.height(),
            peers: peers.len(),
            pending_tx: mempool.len(),
            syncing: self.chain_sync.is_syncing().await,
        }
    }

    /// Shutdown the node
    pub async fn shutdown(&self) {
        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(()).await;
        }
    }
}

/// Node status information
#[derive(Debug, Clone)]
pub struct NodeStatus {
    pub port: u16,
    pub height: u64,
    pub peers: usize,
    pub pending_tx: usize,
    pub syncing: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_node_creation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = NodeConfig {
            port: 18333,
            bootstrap_peers: vec![],
            data_dir: temp_dir.path().to_path_buf(),
        };

        let node = Node::new(config).await.unwrap();
        let status = node.status().await;

        assert_eq!(status.port, 18333);
        assert_eq!(status.height, 0);
        assert_eq!(status.peers, 0);
    }
}
