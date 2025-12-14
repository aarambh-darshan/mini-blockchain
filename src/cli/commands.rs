//! CLI commands for the blockchain
//!
//! Implements all command handlers for the CLI interface.

use crate::core::Blockchain;
use crate::mining::{Mempool, Miner};
use crate::storage::{Storage, StorageConfig};
use crate::wallet::WalletManager;
use std::path::PathBuf;

/// Result type for CLI operations
pub type CliResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Application state
pub struct AppState {
    pub blockchain: Blockchain,
    pub mempool: Mempool,
    pub storage: Storage,
    pub wallet_manager: WalletManager,
    pub data_dir: PathBuf,
}

impl AppState {
    /// Initialize application state
    pub fn new(data_dir: PathBuf) -> CliResult<Self> {
        let storage_config = StorageConfig {
            data_dir: data_dir.clone(),
            ..Default::default()
        };

        let storage = Storage::new(storage_config)?;
        let wallet_dir = data_dir.join("wallets");
        let wallet_manager = WalletManager::new(&wallet_dir)?;

        // Load or create blockchain
        let blockchain = if storage.exists() {
            println!("📂 Loading existing blockchain...");
            storage.load()?
        } else {
            println!("🆕 Creating new blockchain...");
            let blockchain = Blockchain::new();
            storage.save(&blockchain)?;
            blockchain
        };

        Ok(Self {
            blockchain,
            mempool: Mempool::new(),
            storage,
            wallet_manager,
            data_dir,
        })
    }

    /// Save the current state
    pub fn save(&self) -> CliResult<()> {
        self.storage.save(&self.blockchain)?;
        Ok(())
    }
}

/// Initialize a new blockchain
pub fn cmd_init(data_dir: &PathBuf, difficulty: Option<u32>) -> CliResult<()> {
    let storage_config = StorageConfig {
        data_dir: data_dir.clone(),
        ..Default::default()
    };

    let storage = Storage::new(storage_config)?;

    if storage.exists() {
        println!("⚠️  Blockchain already exists at {:?}", data_dir);
        println!("   Use --force to reinitialize (this will delete existing data)");
        return Ok(());
    }

    let blockchain = match difficulty {
        Some(d) => Blockchain::with_difficulty(d),
        None => Blockchain::new(),
    };

    storage.save(&blockchain)?;

    println!("✅ Blockchain initialized!");
    println!("   📁 Data directory: {:?}", data_dir);
    println!("   🔧 Difficulty: {}", blockchain.difficulty);
    println!(
        "   🧱 Genesis block hash: {}",
        blockchain.latest_block().hash
    );

    Ok(())
}

/// Mine a new block
pub fn cmd_mine(state: &mut AppState, address: &str, count: u32) -> CliResult<()> {
    let miner = Miner::new(address);

    println!("⛏️  Mining {} block(s) for address: {}", count, address);
    println!("   Current difficulty: {}", state.blockchain.difficulty);

    for _ in 0..count {
        // Get transactions from mempool
        let transactions = state.mempool.get_transactions(100);
        let tx_count = transactions.len();

        // Get transaction IDs before mining
        let tx_ids: Vec<String> = transactions.iter().map(|t| t.id.clone()).collect();

        let (block, stats) = miner.mine_block(&mut state.blockchain, transactions)?;

        // Remove mined transactions from mempool
        state.mempool.remove_transactions(&tx_ids);

        println!("\n   Block {} mined!", block.index);
        println!("   ├─ Hash: {}", &block.hash[..16]);
        println!("   ├─ Transactions: {}", tx_count + 1);
        println!("   ├─ Time: {}ms", stats.time_ms);
        println!("   ├─ Attempts: {}", stats.hash_attempts);
        println!("   └─ Hash rate: {:.2} H/s", stats.hash_rate);

        // Save after each block
        state.save()?;
    }

    let balance = state.blockchain.get_balance(address);
    println!("\n💰 New balance for miner: {} coins", balance);

    Ok(())
}

/// Create a new wallet
pub fn cmd_wallet_new(state: &mut AppState, label: Option<&str>) -> CliResult<()> {
    let wallet = state.wallet_manager.create_wallet(label)?;

    println!("🔐 New wallet created!");
    println!("   📍 Address: {}", wallet.address());
    println!("   🔑 Public Key: {}...", &wallet.public_key()[..32]);
    if let Some(l) = &wallet.label {
        println!("   🏷️  Label: {}", l);
    }
    println!("\n   ⚠️  IMPORTANT: Your private key is stored in the wallets directory.");
    println!("   Back up this directory to avoid losing access to your funds!");

    Ok(())
}

/// List all wallets
pub fn cmd_wallet_list(state: &mut AppState) -> CliResult<()> {
    let addresses = state.wallet_manager.list_wallets()?;

    if addresses.is_empty() {
        println!("📭 No wallets found. Create one with: blockchain wallet new");
        return Ok(());
    }

    println!("📋 Wallets:");
    for address in &addresses {
        let balance = state.blockchain.get_balance(address);
        let wallet = state.wallet_manager.load_wallet(address)?;
        let label = wallet.label.as_deref().unwrap_or("-");
        println!("   {} ({}) - {} coins", address, label, balance);
    }

    Ok(())
}

/// Get wallet balance
pub fn cmd_wallet_balance(state: &AppState, address: &str) -> CliResult<()> {
    let balance = state.blockchain.get_balance(address);
    let utxos = state.blockchain.get_utxos_for_address(address);

    println!("💰 Balance for {}", address);
    println!("   Total: {} coins", balance);
    println!("   UTXOs: {}", utxos.len());

    if !utxos.is_empty() {
        println!("\n   Transaction outputs:");
        for utxo in utxos.iter().take(10) {
            println!(
                "   └─ {}:{} = {} coins",
                &utxo.tx_id[..8],
                utxo.output_index,
                utxo.output.amount
            );
        }
        if utxos.len() > 10 {
            println!("   ... and {} more", utxos.len() - 10);
        }
    }

    Ok(())
}

/// Send coins
pub fn cmd_send(state: &mut AppState, from: &str, to: &str, amount: u64) -> CliResult<()> {
    // Load sender wallet
    let wallet = state.wallet_manager.load_wallet(from)?;
    let balance = wallet.balance(&state.blockchain);

    if balance < amount {
        println!("❌ Insufficient funds: have {}, need {}", balance, amount);
        return Ok(());
    }

    // Create transaction
    let tx = wallet.create_transaction(to, amount, &state.blockchain)?;

    println!("📤 Transaction created:");
    println!("   ID: {}", tx.id);
    println!("   From: {}", from);
    println!("   To: {}", to);
    println!("   Amount: {} coins", amount);

    // Add to mempool
    state.mempool.add_transaction(tx, &state.blockchain)?;

    println!("\n✅ Transaction added to mempool");
    println!("   It will be included in the next mined block.");

    Ok(())
}

/// Display blockchain info
pub fn cmd_chain_info(state: &AppState) -> CliResult<()> {
    let stats = state.blockchain.stats();

    println!("⛓️  Blockchain Info");
    println!("   ├─ Height: {}", stats.height);
    println!("   ├─ Total blocks: {}", stats.total_blocks);
    println!("   ├─ Total transactions: {}", stats.total_transactions);
    println!("   ├─ Total coins: {}", stats.total_coins);
    println!("   ├─ Difficulty: {}", stats.difficulty);
    println!("   └─ Latest hash: {}...", &stats.latest_hash[..32]);

    Ok(())
}

/// List recent blocks
pub fn cmd_chain_blocks(state: &AppState, count: u32) -> CliResult<()> {
    let height = state.blockchain.height() as usize;
    let start = if height >= count as usize {
        height - count as usize + 1
    } else {
        0
    };

    println!("🧱 Recent blocks:");
    for i in (start..=height).rev() {
        if let Some(block) = state.blockchain.get_block(i as u64) {
            println!(
                "   #{} | {} | {} tx | {}",
                block.index,
                &block.hash[..16],
                block.transactions.len(),
                block.header.timestamp.format("%Y-%m-%d %H:%M:%S")
            );
        }
    }

    Ok(())
}

/// Validate the blockchain
pub fn cmd_validate(state: &AppState) -> CliResult<()> {
    println!("🔍 Validating blockchain...");

    if state.blockchain.is_valid() {
        println!("✅ Blockchain is valid!");
        println!("   {} blocks verified", state.blockchain.blocks.len());
    } else {
        println!("❌ Blockchain validation FAILED!");
        println!("   The chain may have been tampered with.");
    }

    Ok(())
}

/// Show mempool status
pub fn cmd_mempool(state: &AppState) -> CliResult<()> {
    println!("📬 Mempool Status");
    println!("   Pending transactions: {}", state.mempool.len());

    if !state.mempool.is_empty() {
        println!("\n   Transactions:");
        for id in state.mempool.transaction_ids().iter().take(10) {
            if let Some(tx) = state.mempool.get_transaction(id) {
                println!("   └─ {} ({} outputs)", &id[..16], tx.outputs.len());
            }
        }
    }

    Ok(())
}

/// Export blockchain to file
pub fn cmd_export(state: &AppState, path: &PathBuf) -> CliResult<()> {
    crate::storage::save_to_file(&state.blockchain, path)?;
    println!("📦 Blockchain exported to {:?}", path);
    Ok(())
}

/// Import blockchain from file
pub fn cmd_import(state: &mut AppState, path: &PathBuf) -> CliResult<()> {
    let blockchain = crate::storage::load_from_file(path)?;

    if !blockchain.is_valid() {
        println!("❌ Imported blockchain is invalid!");
        return Ok(());
    }

    state.blockchain = blockchain;
    state.save()?;

    println!("📥 Blockchain imported from {:?}", path);
    println!("   Height: {}", state.blockchain.height());

    Ok(())
}
