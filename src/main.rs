//! Mini-Blockchain CLI Application
//!
//! A command-line interface for interacting with the blockchain.

use clap::{Parser, Subcommand};
use mini_blockchain::api::{create_router, ApiState, WsBroadcaster};
use mini_blockchain::cli::{self, AppState};
use mini_blockchain::contract::{Compiler, ContractManager};
use mini_blockchain::core::Blockchain;
use mini_blockchain::mining::Mempool;
use mini_blockchain::multisig::MultisigManager;
use mini_blockchain::network::{Node, NodeConfig, PeerManager};
use mini_blockchain::storage::{Storage, StorageConfig};
use mini_blockchain::token::TokenManager;
use mini_blockchain::wallet::WalletManager;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Parser)]
#[command(name = "blockchain")]
#[command(author = "Darshan")]
#[command(version = "0.1.0")]
#[command(about = "A production-ready mini-blockchain in Rust", long_about = None)]
struct Cli {
    /// Data directory for blockchain storage
    #[arg(short, long, default_value = ".blockchain_data")]
    data_dir: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new blockchain
    Init {
        /// Mining difficulty (number of leading zero bits)
        #[arg(short, long)]
        difficulty: Option<u32>,
    },

    /// Mine new blocks
    Mine {
        /// Miner's address for receiving rewards
        #[arg(short, long)]
        address: String,

        /// Number of blocks to mine
        #[arg(short, long, default_value = "1")]
        count: u32,
    },

    /// Wallet operations
    Wallet {
        #[command(subcommand)]
        action: WalletCommands,
    },

    /// Send coins to an address
    Send {
        /// Sender's wallet address
        #[arg(short, long)]
        from: String,

        /// Recipient's address
        #[arg(short, long)]
        to: String,

        /// Amount to send
        #[arg(short, long)]
        amount: u64,
    },

    /// Display blockchain information
    Chain {
        #[command(subcommand)]
        action: Option<ChainCommands>,
    },

    /// Validate the blockchain
    Validate,

    /// Show mempool status
    Mempool,

    /// Export blockchain to file
    Export {
        /// Output file path
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Import blockchain from file
    Import {
        /// Input file path
        #[arg(short, long)]
        input: PathBuf,
    },

    /// P2P Node operations
    Node {
        #[command(subcommand)]
        action: NodeCommands,
    },

    /// REST API server
    Api {
        #[command(subcommand)]
        action: ApiCommands,
    },

    /// Smart contract operations
    Contract {
        #[command(subcommand)]
        action: ContractCommands,
    },
}

#[derive(Subcommand)]
enum WalletCommands {
    /// Create a new wallet
    New {
        /// Optional label for the wallet
        #[arg(short, long)]
        label: Option<String>,
    },

    /// List all wallets
    List,

    /// Show wallet balance
    Balance {
        /// Wallet address
        #[arg(short, long)]
        address: String,
    },
}

#[derive(Subcommand)]
enum ChainCommands {
    /// Show detailed info
    Info,

    /// List recent blocks
    Blocks {
        /// Number of blocks to show
        #[arg(short, long, default_value = "10")]
        count: u32,
    },
}

#[derive(Subcommand)]
enum NodeCommands {
    /// Start the P2P node
    Start {
        /// Port to listen on
        #[arg(short, long, default_value = "8333")]
        port: u16,

        /// Initial peers to connect to (comma-separated)
        #[arg(long)]
        peers: Option<String>,
    },

    /// Connect to a peer (while node is running in another terminal)
    Connect {
        /// Peer address (host:port)
        #[arg(short, long)]
        peer: String,
    },

    /// Show node status
    Status,
}

#[derive(Subcommand)]
enum ApiCommands {
    /// Start the REST API server (optionally with embedded P2P node)
    Start {
        /// Port to listen on for REST API
        #[arg(short, long, default_value = "3000")]
        port: u16,

        /// Enable P2P node on this port (optional)
        #[arg(long)]
        p2p_port: Option<u16>,

        /// Initial peers to connect to (comma-separated, requires --p2p-port)
        #[arg(long)]
        peers: Option<String>,
    },
}

#[derive(Subcommand)]
enum ContractCommands {
    /// Deploy a new contract
    Deploy {
        /// Contract source file (.asm)
        #[arg(short, long)]
        file: PathBuf,
    },

    /// Call a contract
    Call {
        /// Contract address
        #[arg(short, long)]
        address: String,

        /// Arguments (comma-separated numbers)
        #[arg(long)]
        args: Option<String>,

        /// Gas limit
        #[arg(long, default_value = "100000")]
        gas: u64,
    },

    /// List all contracts
    List,

    /// Show contract info
    Info {
        /// Contract address
        #[arg(short, long)]
        address: String,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();

    // Handle init command separately (doesn't need full state)
    if let Commands::Init { difficulty } = &cli.command {
        return cli::cmd_init(&cli.data_dir, *difficulty).map_err(Into::into);
    }

    // Handle node commands with tokio runtime
    if let Commands::Node { ref action } = cli.command {
        return run_node_command(action, &cli.data_dir)
            .map_err(|e| -> Box<dyn std::error::Error> { e });
    }

    // Handle API commands with tokio runtime
    if let Commands::Api { ref action } = cli.command {
        return run_api_command(action, &cli.data_dir);
    }

    // Initialize application state
    let mut state = AppState::new(cli.data_dir.clone())?;

    // Process commands
    match cli.command {
        Commands::Init { .. } => unreachable!(),
        Commands::Node { .. } => unreachable!(),
        Commands::Api { .. } => unreachable!(),

        Commands::Mine { address, count } => {
            cli::cmd_mine(&mut state, &address, count)?;
        }

        Commands::Wallet { action } => match action {
            WalletCommands::New { label } => {
                cli::cmd_wallet_new(&mut state, label.as_deref())?;
            }
            WalletCommands::List => {
                cli::cmd_wallet_list(&mut state)?;
            }
            WalletCommands::Balance { address } => {
                cli::cmd_wallet_balance(&state, &address)?;
            }
        },

        Commands::Send { from, to, amount } => {
            cli::cmd_send(&mut state, &from, &to, amount)?;
        }

        Commands::Chain { action } => match action {
            None | Some(ChainCommands::Info) => {
                cli::cmd_chain_info(&state)?;
            }
            Some(ChainCommands::Blocks { count }) => {
                cli::cmd_chain_blocks(&state, count)?;
            }
        },

        Commands::Validate => {
            cli::cmd_validate(&state)?;
        }

        Commands::Mempool => {
            cli::cmd_mempool(&state)?;
        }

        Commands::Export { output } => {
            cli::cmd_export(&state, &output)?;
        }

        Commands::Import { input } => {
            cli::cmd_import(&mut state, &input)?;
        }

        Commands::Contract { action } => {
            run_contract_command(&action, &cli.data_dir)?;
        }
    }

    Ok(())
}

fn run_node_command(
    action: &NodeCommands,
    data_dir: &PathBuf,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let rt = tokio::runtime::Runtime::new()?;

    rt.block_on(async {
        match action {
            NodeCommands::Start { port, peers } => {
                let bootstrap_peers: Vec<String> = peers
                    .clone()
                    .map(|p| p.split(',').map(|s| s.trim().to_string()).collect())
                    .unwrap_or_default();

                let config = NodeConfig {
                    port: *port,
                    bootstrap_peers,
                    data_dir: data_dir.clone(),
                };

                println!("🌐 Starting P2P node on port {}...", port);

                let mut node = Node::new(config).await?;

                // Handle Ctrl+C
                tokio::spawn(async move {
                    tokio::signal::ctrl_c().await.ok();
                    println!("\n📴 Shutting down node...");
                    std::process::exit(0);
                });

                node.start().await?;
            }

            NodeCommands::Connect { peer } => {
                println!("⚠️  To connect to peers, use: node start --peers {}", peer);
                println!("   The node must be running to accept connections.");
            }

            NodeCommands::Status => {
                // For now, just show that the node command exists
                // Full status would require connecting to a running node
                println!("ℹ️  Node status:");
                println!("   Use 'node start' to run a P2P node");
                println!(
                    "   Use 'node start --port 8334 --peers 127.0.0.1:8333' to connect to peers"
                );
            }
        }

        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    })?;

    Ok(())
}

fn run_api_command(
    action: &ApiCommands,
    data_dir: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Runtime::new()?;

    rt.block_on(async {
        match action {
            ApiCommands::Start {
                port,
                p2p_port,
                peers,
            } => {
                // Initialize storage
                let storage_config = StorageConfig {
                    data_dir: data_dir.clone(),
                    ..Default::default()
                };
                let storage = Arc::new(Storage::new(storage_config)?);

                // Load or create blockchain
                let blockchain = if storage.exists() {
                    println!("📂 Loading existing blockchain...");
                    Arc::new(RwLock::new(storage.load()?))
                } else {
                    println!("📂 Creating new blockchain...");
                    let chain = Blockchain::new();
                    storage.save(&chain)?;
                    Arc::new(RwLock::new(chain))
                };

                // Initialize components
                let mempool = Arc::new(RwLock::new(Mempool::new()));
                let wallets_dir = data_dir.join("wallets");
                let wallet_manager = Arc::new(RwLock::new(WalletManager::new(&wallets_dir)?));

                // Load or create contract manager
                let contracts_file = data_dir.join("contracts.json");
                let contract_manager = if contracts_file.exists() {
                    let data = std::fs::read_to_string(&contracts_file)?;
                    Arc::new(RwLock::new(serde_json::from_str(&data)?))
                } else {
                    Arc::new(RwLock::new(ContractManager::new()))
                };

                // Create WebSocket broadcaster
                let ws_broadcaster = Arc::new(WsBroadcaster::new());

                // Load or create multisig manager
                let multisig_file = data_dir.join("multisig.json");
                let multisig_manager = if multisig_file.exists() {
                    let data = std::fs::read_to_string(&multisig_file)?;
                    Arc::new(RwLock::new(serde_json::from_str(&data)?))
                } else {
                    Arc::new(RwLock::new(MultisigManager::new()))
                };

                // Load or create token manager
                let tokens_file = data_dir.join("tokens.json");
                let token_manager = if tokens_file.exists() {
                    let data = std::fs::read_to_string(&tokens_file)?;
                    Arc::new(RwLock::new(serde_json::from_str(&data)?))
                } else {
                    Arc::new(RwLock::new(TokenManager::new()))
                };

                // Create PeerManager if P2P is enabled
                let peer_manager: Option<Arc<PeerManager>> = if p2p_port.is_some() {
                    Some(Arc::new(PeerManager::new(p2p_port.unwrap())))
                } else {
                    None
                };

                // Create API state
                let state = ApiState {
                    blockchain: blockchain.clone(),
                    mempool: mempool.clone(),
                    storage: storage.clone(),
                    wallet_manager,
                    contract_manager,
                    ws_broadcaster,
                    multisig_manager,
                    token_manager,
                    peer_manager: peer_manager.clone(),
                };

                // Clone state for shutdown handler
                let shutdown_state = state.clone();
                let shutdown_data_dir = data_dir.clone();

                // Create router
                let app = create_router(state);

                // Start server
                let addr = format!("0.0.0.0:{}", port);
                println!("🚀 REST API server starting on http://localhost:{}", port);

                // Optionally start P2P node
                if let Some(p2p_port) = p2p_port {
                    let bootstrap_peers: Vec<String> = peers
                        .clone()
                        .map(|p| p.split(',').map(|s| s.trim().to_string()).collect())
                        .unwrap_or_default();

                    let config = NodeConfig {
                        port: *p2p_port,
                        bootstrap_peers: bootstrap_peers.clone(),
                        data_dir: data_dir.clone(),
                    };

                    println!("🌐 P2P node enabled on port {}", p2p_port);
                    if !bootstrap_peers.is_empty() {
                        println!("   Connecting to peers: {:?}", bootstrap_peers);
                    }
                    println!("   Blocks mined via API will be broadcast to peers!");

                    // Start P2P node with SHARED blockchain, mempool, and peer_manager
                    let p2p_blockchain = blockchain.clone();
                    let p2p_mempool = mempool.clone();
                    let p2p_storage = storage.clone();
                    let p2p_peer_manager = peer_manager.clone().unwrap();

                    tokio::spawn(async move {
                        // Create node with shared state - pass the same peer_manager
                        let mut node = Node::new_with_shared_and_peer_manager(
                            config,
                            p2p_blockchain,
                            p2p_mempool,
                            p2p_storage,
                            p2p_peer_manager,
                        );

                        log::info!("P2P node started with shared blockchain");
                        if let Err(e) = node.start().await {
                            log::error!("P2P node error: {}", e);
                        }
                    });
                }

                println!();
                println!("📖 Available endpoints:");
                println!("   GET  /health                      - Health check");
                println!("   GET  /ws                          - WebSocket updates");
                println!("   GET  /api/chain                   - Blockchain info");
                println!("   GET  /api/chain/blocks            - List blocks");
                println!("   GET  /api/chain/blocks/{{height}}   - Get block");
                println!("   GET  /api/chain/validate          - Validate chain");
                println!("   POST /api/mine                    - Mine block");
                println!("   GET  /api/mempool                 - Pending transactions");
                println!("   GET  /api/transactions/{{id}}       - Get transaction");
                println!("   GET  /api/wallets                 - List wallets");
                println!("   POST /api/wallets                 - Create wallet");
                println!("   GET  /api/wallets/{{addr}}/balance   - Get balance");
                println!("   GET  /api/contracts               - List contracts");
                println!("   POST /api/contracts               - Deploy contract");
                println!("   POST /api/contracts/{{addr}}/call    - Call contract");
                println!("   GET  /api/multisig                - List multisig wallets");
                println!("   POST /api/multisig                - Create multisig wallet");
                println!("   POST /api/multisig/{{addr}}/propose  - Propose transaction");
                println!("   POST /api/multisig/{{addr}}/sign     - Sign transaction");
                println!("   GET  /api/tokens                  - List tokens");
                println!("   POST /api/tokens                  - Create token");
                println!("   GET  /api/tokens/{{addr}}/balance/{{h}} - Token balance");
                println!("   POST /api/tokens/{{addr}}/transfer   - Transfer tokens");
                println!();

                // Handle Ctrl+C with graceful shutdown
                tokio::spawn(async move {
                    tokio::signal::ctrl_c().await.ok();
                    println!("\n📴 Shutting down API server...");

                    // Save all data before exit
                    println!("💾 Saving data...");

                    // Save contracts
                    let contracts = shutdown_state.contract_manager.read().await;
                    if let Ok(data) = serde_json::to_string_pretty(&*contracts) {
                        let _ = std::fs::write(shutdown_data_dir.join("contracts.json"), data);
                    }

                    // Save multisig
                    let multisig = shutdown_state.multisig_manager.read().await;
                    if let Ok(data) = serde_json::to_string_pretty(&*multisig) {
                        let _ = std::fs::write(shutdown_data_dir.join("multisig.json"), data);
                    }

                    // Save tokens
                    let tokens = shutdown_state.token_manager.read().await;
                    if let Ok(data) = serde_json::to_string_pretty(&*tokens) {
                        let _ = std::fs::write(shutdown_data_dir.join("tokens.json"), data);
                    }

                    // Save blockchain
                    let blockchain = shutdown_state.blockchain.read().await;
                    let _ = shutdown_state.storage.save(&blockchain);

                    println!("✅ Data saved successfully!");
                    std::process::exit(0);
                });

                let listener = tokio::net::TcpListener::bind(&addr).await?;
                axum::serve(listener, app).await?;
            }
        }

        Ok::<(), Box<dyn std::error::Error>>(())
    })?;

    Ok(())
}

fn run_contract_command(
    action: &ContractCommands,
    data_dir: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    // Load or create contract manager
    let contracts_file = data_dir.join("contracts.json");
    let mut manager = if contracts_file.exists() {
        let data = fs::read_to_string(&contracts_file)?;
        serde_json::from_str(&data)?
    } else {
        ContractManager::new()
    };

    match action {
        ContractCommands::Deploy { file } => {
            println!("📜 Deploying contract from {:?}...", file);

            let source = fs::read_to_string(file)?;
            let mut compiler = Compiler::new();
            let bytecode = compiler.compile(&source)?;

            println!("   Compiled {} bytes of bytecode", bytecode.len());

            // Use a simple deployer address for CLI
            let deployer = "cli_deployer";
            let address = manager.deploy(bytecode, deployer, 1)?;

            // Save manager
            let data = serde_json::to_string_pretty(&manager)?;
            fs::write(&contracts_file, data)?;

            println!("✅ Contract deployed!");
            println!("   Address: {}", address);
        }

        ContractCommands::Call { address, args, gas } => {
            println!("📞 Calling contract {}...", address);

            let args: Vec<u64> = args
                .as_ref()
                .map(|s| s.split(',').filter_map(|n| n.trim().parse().ok()).collect())
                .unwrap_or_default();

            let result = manager.call(address, "cli_caller", args, 0, 1, Some(*gas))?;

            // Save manager (in case storage changed)
            let data = serde_json::to_string_pretty(&manager)?;
            fs::write(&contracts_file, data)?;

            println!("✅ Execution complete!");
            println!("   Success: {}", result.success);
            if let Some(ret) = result.return_value {
                println!("   Return value: {}", ret);
            }
            println!("   Gas used: {}", result.gas_used);
            if !result.storage_changes.is_empty() {
                println!("   Storage changes: {}", result.storage_changes.len());
            }
        }

        ContractCommands::List => {
            let contracts = manager.list();
            if contracts.is_empty() {
                println!("📜 No contracts deployed yet.");
            } else {
                println!("📜 Deployed contracts ({}):", contracts.len());
                for addr in contracts {
                    println!("   {}", addr);
                }
            }
        }

        ContractCommands::Info { address } => {
            if let Some(contract) = manager.get(address) {
                println!("📜 Contract: {}", address);
                println!("   Deployer: {}", contract.deployer);
                println!("   Deployed at block: {}", contract.deployed_at);
                println!("   Code size: {} bytes", contract.code.len());
                println!("   Storage entries: {}", contract.storage.len());

                // Show disassembly
                println!("\n   Bytecode:");
                let disasm = mini_blockchain::contract::disassemble(&contract.code);
                for line in disasm.lines().take(20) {
                    println!("   {}", line);
                }
                if disasm.lines().count() > 20 {
                    println!("   ... ({} more lines)", disasm.lines().count() - 20);
                }
            } else {
                println!("❌ Contract not found: {}", address);
            }
        }
    }

    Ok(())
}
