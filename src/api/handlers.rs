//! REST API handlers for blockchain operations

use crate::api::websocket::WsBroadcaster;
use crate::contract::{Compiler, ContractManager};
use crate::core::{Blockchain, Transaction};
use crate::mining::{Mempool, Miner};
use crate::storage::Storage;
use crate::wallet::WalletManager;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared application state for API handlers
#[derive(Clone)]
pub struct ApiState {
    pub blockchain: Arc<RwLock<Blockchain>>,
    pub mempool: Arc<RwLock<Mempool>>,
    pub storage: Arc<Storage>,
    pub wallet_manager: Arc<RwLock<WalletManager>>,
    pub contract_manager: Arc<RwLock<ContractManager>>,
    pub ws_broadcaster: Arc<WsBroadcaster>,
}

// ============================================================================
// Response Types
// ============================================================================

#[derive(Serialize)]
pub struct ChainInfo {
    pub height: u64,
    pub difficulty: u32,
    pub total_blocks: usize,
    pub total_transactions: usize,
    pub total_coins: u64,
    pub latest_hash: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct BlockInfo {
    pub index: u64,
    pub hash: String,
    pub previous_hash: String,
    pub merkle_root: String,
    pub timestamp: String,
    pub difficulty: u32,
    pub nonce: u64,
    pub transactions: usize,
}

#[derive(Serialize)]
pub struct MineResponse {
    pub block: BlockInfo,
    pub reward: u64,
    pub time_ms: u128,
    pub attempts: u64,
}

#[derive(Serialize)]
pub struct BalanceResponse {
    pub address: String,
    pub balance: u64,
    pub utxo_count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct TransactionResponse {
    pub id: String,
    pub is_coinbase: bool,
    pub inputs: usize,
    pub outputs: usize,
    pub total_output: u64,
}

impl From<&Transaction> for TransactionResponse {
    fn from(tx: &Transaction) -> Self {
        Self {
            id: tx.id.clone(),
            is_coinbase: tx.is_coinbase,
            inputs: tx.inputs.len(),
            outputs: tx.outputs.len(),
            total_output: tx.total_output(),
        }
    }
}

#[derive(Serialize)]
pub struct WalletResponse {
    pub address: String,
    pub label: Option<String>,
}

#[derive(Serialize)]
pub struct MempoolResponse {
    pub pending_transactions: usize,
    pub transactions: Vec<TransactionResponse>,
}

#[derive(Serialize)]
pub struct ValidationResponse {
    pub valid: bool,
    pub blocks_checked: usize,
    pub message: String,
}

#[derive(Serialize)]
pub struct ApiError {
    pub error: String,
}

// ============================================================================
// Request Types
// ============================================================================

#[derive(Deserialize)]
pub struct MineRequest {
    pub miner_address: String,
}

#[derive(Deserialize)]
pub struct CreateWalletRequest {
    pub label: Option<String>,
}

#[derive(Deserialize)]
pub struct DeployContractRequest {
    pub source: String,
}

#[derive(Deserialize)]
pub struct CallContractRequest {
    pub args: Vec<u64>,
    pub gas_limit: Option<u64>,
}

#[derive(Serialize)]
pub struct ContractInfo {
    pub address: String,
    pub deployer: String,
    pub deployed_at: u64,
    pub code_size: usize,
}

#[derive(Serialize)]
pub struct DeployResponse {
    pub address: String,
    pub code_size: usize,
}

#[derive(Serialize)]
pub struct CallResponse {
    pub success: bool,
    pub return_value: Option<u64>,
    pub gas_used: u64,
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/chain - Get blockchain info
pub async fn get_chain_info(State(state): State<ApiState>) -> Json<ChainInfo> {
    let chain = state.blockchain.read().await;

    let total_tx: usize = chain.blocks.iter().map(|b| b.transactions.len()).sum();
    let total_coins: u64 = chain
        .blocks
        .iter()
        .flat_map(|b| &b.transactions)
        .filter(|tx| tx.is_coinbase)
        .map(|tx| tx.total_output())
        .sum();

    Json(ChainInfo {
        height: chain.height(),
        difficulty: chain.difficulty,
        total_blocks: chain.blocks.len(),
        total_transactions: total_tx,
        total_coins,
        latest_hash: chain.latest_block().hash.clone(),
    })
}

/// GET /api/chain/blocks - List recent blocks
pub async fn get_blocks(State(state): State<ApiState>) -> Json<Vec<BlockInfo>> {
    let chain = state.blockchain.read().await;
    let blocks: Vec<BlockInfo> = chain
        .blocks
        .iter()
        .rev()
        .take(10)
        .map(|block| BlockInfo {
            index: block.index,
            hash: block.hash.clone(),
            previous_hash: block.header.previous_hash.clone(),
            merkle_root: block.header.merkle_root.clone(),
            timestamp: block.header.timestamp.to_rfc3339(),
            difficulty: block.header.difficulty,
            nonce: block.header.nonce,
            transactions: block.transactions.len(),
        })
        .collect();

    Json(blocks)
}

/// GET /api/chain/blocks/:height - Get block by height
pub async fn get_block_by_height(
    State(state): State<ApiState>,
    Path(height): Path<u64>,
) -> Result<Json<BlockInfo>, (StatusCode, Json<ApiError>)> {
    let chain = state.blockchain.read().await;

    if let Some(block) = chain.get_block(height) {
        Ok(Json(BlockInfo {
            index: block.index,
            hash: block.hash.clone(),
            previous_hash: block.header.previous_hash.clone(),
            merkle_root: block.header.merkle_root.clone(),
            timestamp: block.header.timestamp.to_rfc3339(),
            difficulty: block.header.difficulty,
            nonce: block.header.nonce,
            transactions: block.transactions.len(),
        }))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(ApiError {
                error: format!("Block at height {} not found", height),
            }),
        ))
    }
}

/// GET /api/chain/validate - Validate blockchain
pub async fn validate_chain(State(state): State<ApiState>) -> Json<ValidationResponse> {
    let chain = state.blockchain.read().await;
    let is_valid = chain.is_valid();
    let block_count = chain.blocks.len();

    Json(ValidationResponse {
        valid: is_valid,
        blocks_checked: block_count,
        message: if is_valid {
            format!("Blockchain is valid ({} blocks verified)", block_count)
        } else {
            "Blockchain validation failed".to_string()
        },
    })
}

/// POST /api/mine - Mine a new block
pub async fn mine_block(
    State(state): State<ApiState>,
    Json(req): Json<MineRequest>,
) -> Result<Json<MineResponse>, (StatusCode, Json<ApiError>)> {
    let mut chain = state.blockchain.write().await;
    let mempool = state.mempool.read().await;

    let miner = Miner::new(&req.miner_address);
    let transactions = mempool.get_transactions(10);

    match miner.mine_block(&mut chain, transactions) {
        Ok((block, stats)) => {
            // Create block info for response and WebSocket
            let block_info = BlockInfo {
                index: block.index,
                hash: block.hash.clone(),
                previous_hash: block.header.previous_hash.clone(),
                merkle_root: block.header.merkle_root.clone(),
                timestamp: block.header.timestamp.to_rfc3339(),
                difficulty: block.header.difficulty,
                nonce: block.header.nonce,
                transactions: block.transactions.len(),
            };
            let reward = block.mining_reward();

            drop(chain);

            // Save blockchain
            let chain = state.blockchain.read().await;
            if let Err(e) = state.storage.save(&chain) {
                log::error!("Failed to save blockchain: {}", e);
            }

            // Broadcast BlockMined event to WebSocket clients
            state
                .ws_broadcaster
                .broadcast(crate::api::websocket::WsEvent::BlockMined {
                    block: block_info.clone(),
                    reward,
                });

            Ok(Json(MineResponse {
                block: block_info,
                reward,
                time_ms: stats.time_ms,
                attempts: stats.hash_attempts,
            }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: format!("Mining failed: {}", e),
            }),
        )),
    }
}

/// GET /api/mempool - Get pending transactions
pub async fn get_mempool(State(state): State<ApiState>) -> Json<MempoolResponse> {
    let mempool = state.mempool.read().await;
    let transactions: Vec<TransactionResponse> = mempool
        .get_transactions(100)
        .iter()
        .map(TransactionResponse::from)
        .collect();

    Json(MempoolResponse {
        pending_transactions: mempool.len(),
        transactions,
    })
}

/// GET /api/transactions/:id - Get transaction by ID
pub async fn get_transaction(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<TransactionResponse>, (StatusCode, Json<ApiError>)> {
    let chain = state.blockchain.read().await;

    // Search in blockchain
    for block in &chain.blocks {
        for tx in &block.transactions {
            if tx.id == id {
                return Ok(Json(TransactionResponse::from(tx)));
            }
        }
    }

    // Search in mempool
    let mempool = state.mempool.read().await;
    for tx in mempool.get_transactions(1000) {
        if tx.id == id {
            return Ok(Json(TransactionResponse::from(&tx)));
        }
    }

    Err((
        StatusCode::NOT_FOUND,
        Json(ApiError {
            error: format!("Transaction {} not found", id),
        }),
    ))
}

/// POST /api/wallets - Create new wallet
pub async fn create_wallet(
    State(state): State<ApiState>,
    Json(req): Json<CreateWalletRequest>,
) -> Result<Json<WalletResponse>, (StatusCode, Json<ApiError>)> {
    let manager = state.wallet_manager.read().await;

    match manager.create_wallet(req.label.as_deref()) {
        Ok(wallet) => Ok(Json(WalletResponse {
            address: wallet.address(),
            label: req.label,
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: format!("Failed to create wallet: {}", e),
            }),
        )),
    }
}

/// GET /api/wallets - List all wallets
pub async fn list_wallets(
    State(state): State<ApiState>,
) -> Result<Json<Vec<WalletResponse>>, (StatusCode, Json<ApiError>)> {
    let manager = state.wallet_manager.read().await;

    match manager.list_wallets() {
        Ok(addresses) => {
            let wallets: Vec<WalletResponse> = addresses
                .into_iter()
                .map(|addr| WalletResponse {
                    address: addr,
                    label: None,
                })
                .collect();
            Ok(Json(wallets))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: format!("Failed to list wallets: {}", e),
            }),
        )),
    }
}

/// GET /api/wallets/:address/balance - Get wallet balance
pub async fn get_wallet_balance(
    State(state): State<ApiState>,
    Path(address): Path<String>,
) -> Json<BalanceResponse> {
    let chain = state.blockchain.read().await;
    let utxos = chain.get_utxos_for_address(&address);
    let balance: u64 = utxos.iter().map(|u| u.output.amount).sum();

    Json(BalanceResponse {
        address,
        balance,
        utxo_count: utxos.len(),
    })
}

/// Health check endpoint
pub async fn health_check() -> &'static str {
    "OK"
}

// ============================================================================
// Contract Handlers
// ============================================================================

/// GET /api/contracts - List all contracts
pub async fn list_contracts(State(state): State<ApiState>) -> Json<Vec<ContractInfo>> {
    let manager = state.contract_manager.read().await;
    let contracts: Vec<ContractInfo> = manager
        .list()
        .iter()
        .filter_map(|addr| {
            manager.get(addr).map(|c| ContractInfo {
                address: c.address.clone(),
                deployer: c.deployer.clone(),
                deployed_at: c.deployed_at,
                code_size: c.code.len(),
            })
        })
        .collect();
    Json(contracts)
}

/// POST /api/contracts - Deploy a new contract
pub async fn deploy_contract(
    State(state): State<ApiState>,
    Json(req): Json<DeployContractRequest>,
) -> Result<Json<DeployResponse>, (StatusCode, Json<ApiError>)> {
    // Compile source code
    let mut compiler = Compiler::new();
    let bytecode = match compiler.compile(&req.source) {
        Ok(code) => code,
        Err(e) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiError {
                    error: format!("Compilation failed: {}", e),
                }),
            ));
        }
    };

    // Deploy contract
    let chain = state.blockchain.read().await;
    let mut manager = state.contract_manager.write().await;

    match manager.deploy(bytecode.clone(), "web-deployer", chain.height()) {
        Ok(address) => Ok(Json(DeployResponse {
            address,
            code_size: bytecode.len(),
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: format!("Deployment failed: {}", e),
            }),
        )),
    }
}

/// GET /api/contracts/:address - Get contract info
pub async fn get_contract(
    State(state): State<ApiState>,
    Path(address): Path<String>,
) -> Result<Json<ContractInfo>, (StatusCode, Json<ApiError>)> {
    let manager = state.contract_manager.read().await;

    match manager.get(&address) {
        Some(contract) => Ok(Json(ContractInfo {
            address: contract.address.clone(),
            deployer: contract.deployer.clone(),
            deployed_at: contract.deployed_at,
            code_size: contract.code.len(),
        })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError {
                error: format!("Contract not found: {}", address),
            }),
        )),
    }
}

/// POST /api/contracts/:address/call - Call a contract
pub async fn call_contract(
    State(state): State<ApiState>,
    Path(address): Path<String>,
    Json(req): Json<CallContractRequest>,
) -> Result<Json<CallResponse>, (StatusCode, Json<ApiError>)> {
    let chain = state.blockchain.read().await;
    let mut manager = state.contract_manager.write().await;

    let timestamp = chrono::Utc::now().timestamp() as u64;

    match manager.call(
        &address,
        "web-caller",
        req.args,
        timestamp,
        chain.height(),
        req.gas_limit,
    ) {
        Ok(result) => Ok(Json(CallResponse {
            success: result.success,
            return_value: result.return_value,
            gas_used: result.gas_used,
        })),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: format!("Contract call failed: {}", e),
            }),
        )),
    }
}
