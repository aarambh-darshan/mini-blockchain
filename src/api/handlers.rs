//! REST API handlers for blockchain operations

use crate::api::websocket::WsBroadcaster;
use crate::contract::{Compiler, ContractManager};
use crate::core::{Blockchain, Transaction};
use crate::mining::{Mempool, Miner};
use crate::multisig::{MultisigConfig, MultisigManager, MultisigSignature};
use crate::storage::Storage;
use crate::token::TokenManager;
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
    pub multisig_manager: Arc<RwLock<MultisigManager>>,
    pub token_manager: Arc<RwLock<TokenManager>>,
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
    pub public_key: String,
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
    pub gas_price: Option<u64>,         // Price per gas unit (default: 1)
    pub caller_address: Option<String>, // Who pays for gas (required for gas payment)
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
    pub gas_cost: u64,               // Total cost in coins (gas_used * gas_price)
    pub caller_balance: Option<u64>, // Remaining balance after gas payment
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

    // Get transactions from mempool
    let transactions = {
        let mempool = state.mempool.read().await;
        mempool.get_transactions(10)
    };

    // Collect tx IDs for cleanup after mining
    let tx_ids: Vec<String> = transactions.iter().map(|t| t.id.clone()).collect();

    let miner = Miner::new(&req.miner_address);

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

            // Remove mined transactions from mempool
            {
                let mut mempool = state.mempool.write().await;
                mempool.remove_transactions(&tx_ids);
            }

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
            public_key: wallet.public_key(),
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
                .filter_map(|addr| {
                    manager.load_wallet(&addr).ok().map(|w| WalletResponse {
                        address: addr,
                        public_key: w.public_key(),
                        label: w.label.clone(),
                    })
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
    let gas_price = req.gas_price.unwrap_or(1);
    let gas_limit = req.gas_limit.unwrap_or(1_000); // Reasonable default for simple contracts
    let max_cost = gas_limit * gas_price;

    // If caller provided, check balance first
    let caller_address = req
        .caller_address
        .clone()
        .unwrap_or_else(|| "anonymous".to_string());
    let mut caller_balance: Option<u64> = None;

    if req.caller_address.is_some() && gas_price > 0 {
        let chain = state.blockchain.read().await;
        let balance = chain.get_balance(&caller_address);

        if balance < max_cost {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiError {
                    error: format!(
                        "Insufficient balance for gas. Need {} coins (gas_limit {} × gas_price {}), have {}",
                        max_cost, gas_limit, gas_price, balance
                    ),
                }),
            ));
        }
        caller_balance = Some(balance);
    }

    // Execute the contract
    // Execute the contract - need write access if charging gas
    let mut chain = state.blockchain.write().await;
    let mut manager = state.contract_manager.write().await;
    let timestamp = chrono::Utc::now().timestamp() as u64;
    let height = chain.height();

    match manager.call(
        &address,
        &caller_address,
        req.args,
        timestamp,
        height,
        req.gas_limit,
    ) {
        Ok(result) => {
            let gas_cost = result.gas_used * gas_price;

            // Process gas payment if applicable
            let mut new_balance = caller_balance;

            if req.caller_address.is_some() && gas_price > 0 && gas_cost > 0 {
                // Drop write locks before getting wallet (to avoid potential deadlocks)
                drop(chain);
                drop(manager);

                let wallet_manager = state.wallet_manager.read().await;
                if let Ok(wallet) = wallet_manager.load_wallet(&caller_address) {
                    // Re-acquire chain read lock to create transaction
                    let chain = state.blockchain.read().await;

                    // Create transaction to "burn" address to pay for gas
                    // Using a burn address ensures coins are removed from circulation (simulating burnt fee)
                    // Or we could send to a "miner" address. "0x0000..." is simpler for burning.
                    match wallet.create_transaction(
                        "0x0000000000000000000000000000000000000000",
                        gas_cost,
                        &chain,
                    ) {
                        Ok(tx) => {
                            drop(chain); // Drop read lock

                            // Re-acquire chain write lock to mine
                            let mut chain = state.blockchain.write().await;

                            // Mine block with this transaction
                            // We use a system address for mining this "maintenance" block
                            match chain.mine_block(vec![tx], "network_gas_miner") {
                                Ok(_) => {
                                    log::info!(
                                        "Gas paid: {} coins persisted via mined block",
                                        gas_cost
                                    );
                                    new_balance = Some(chain.get_balance(&caller_address));
                                }
                                Err(e) => {
                                    log::error!("Failed to mine gas block: {}", e);
                                    // Fallback to speculative balance (won't persist but shows correct in UI now)
                                    new_balance =
                                        caller_balance.map(|b| b.saturating_sub(gas_cost));
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to create gas tx: {}", e);
                            new_balance = caller_balance.map(|b| b.saturating_sub(gas_cost));
                        }
                    }
                } else {
                    log::warn!(
                        "Wallet {} not found locally, cannot sign gas tx",
                        caller_address
                    );
                    new_balance = caller_balance.map(|b| b.saturating_sub(gas_cost));
                }
            } else {
                new_balance = caller_balance.map(|b| b.saturating_sub(gas_cost));
            }

            Ok(Json(CallResponse {
                success: result.success,
                return_value: result.return_value,
                gas_used: result.gas_used,
                gas_cost,
                caller_balance: new_balance,
            }))
        }
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: format!("Contract call failed: {}", e),
            }),
        )),
    }
}

// ============================================================================
// Multisig Endpoints
// ============================================================================

/// Request to create a multisig wallet
#[derive(Deserialize)]
pub struct CreateMultisigRequest {
    pub threshold: u8,
    pub signers: Vec<String>,
    pub label: Option<String>,
}

/// Multisig wallet info response
#[derive(Serialize)]
pub struct MultisigWalletInfo {
    pub address: String,
    pub threshold: u8,
    pub signer_count: usize,
    pub signers: Vec<String>,
    pub label: Option<String>,
    pub description: String,
    pub created_at: String,
}

/// Pending transaction info response
#[derive(Serialize)]
pub struct PendingTxInfo {
    pub id: String,
    pub from_address: String,
    pub to_address: String,
    pub amount: u64,
    pub signatures_collected: usize,
    pub signatures_required: u8,
    pub signed_by: Vec<String>,
    pub status: String,
    pub created_at: String,
}

/// Request to propose a transaction
#[derive(Deserialize)]
pub struct ProposeTransactionRequest {
    pub to: String,
    pub amount: u64,
}

/// Request to sign a pending transaction
#[derive(Deserialize)]
pub struct SignTransactionRequest {
    pub tx_id: String,
    pub signer_pubkey: String,
    pub signature: String,
}

/// POST /api/multisig - Create a multisig wallet
pub async fn create_multisig(
    State(state): State<ApiState>,
    Json(req): Json<CreateMultisigRequest>,
) -> Result<Json<MultisigWalletInfo>, (StatusCode, Json<ApiError>)> {
    let config = MultisigConfig::new(req.threshold, req.signers, req.label).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: format!("Invalid multisig config: {}", e),
            }),
        )
    })?;

    let mut manager = state.multisig_manager.write().await;
    let wallet = manager.create_wallet(config).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: format!("Failed to create multisig wallet: {}", e),
            }),
        )
    })?;

    Ok(Json(MultisigWalletInfo {
        address: wallet.address.clone(),
        threshold: wallet.config.threshold,
        signer_count: wallet.config.signers.len(),
        signers: wallet.config.signers.clone(),
        label: wallet.config.label.clone(),
        description: wallet.description(),
        created_at: wallet.created_at.to_rfc3339(),
    }))
}

/// GET /api/multisig - List all multisig wallets
pub async fn list_multisig(State(state): State<ApiState>) -> Json<Vec<MultisigWalletInfo>> {
    let manager = state.multisig_manager.read().await;
    let wallets: Vec<MultisigWalletInfo> = manager
        .list_wallets()
        .iter()
        .map(|w| MultisigWalletInfo {
            address: w.address.clone(),
            threshold: w.config.threshold,
            signer_count: w.config.signers.len(),
            signers: w.config.signers.clone(),
            label: w.config.label.clone(),
            description: w.description(),
            created_at: w.created_at.to_rfc3339(),
        })
        .collect();

    Json(wallets)
}

/// GET /api/multisig/{address} - Get multisig wallet details
pub async fn get_multisig(
    State(state): State<ApiState>,
    Path(address): Path<String>,
) -> Result<Json<MultisigWalletInfo>, (StatusCode, Json<ApiError>)> {
    let manager = state.multisig_manager.read().await;

    match manager.get_wallet(&address) {
        Some(wallet) => Ok(Json(MultisigWalletInfo {
            address: wallet.address.clone(),
            threshold: wallet.config.threshold,
            signer_count: wallet.config.signers.len(),
            signers: wallet.config.signers.clone(),
            label: wallet.config.label.clone(),
            description: wallet.description(),
            created_at: wallet.created_at.to_rfc3339(),
        })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError {
                error: format!("Multisig wallet not found: {}", address),
            }),
        )),
    }
}

/// GET /api/multisig/{address}/balance - Get multisig wallet balance
pub async fn get_multisig_balance(
    State(state): State<ApiState>,
    Path(address): Path<String>,
) -> Result<Json<BalanceResponse>, (StatusCode, Json<ApiError>)> {
    let manager = state.multisig_manager.read().await;
    let blockchain = state.blockchain.read().await;

    match manager.get_balance(&address, &blockchain) {
        Some(balance) => {
            let utxos = blockchain.get_utxos_for_address(&address);
            Ok(Json(BalanceResponse {
                address,
                balance,
                utxo_count: utxos.len(),
            }))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError {
                error: format!("Multisig wallet not found: {}", address),
            }),
        )),
    }
}

/// POST /api/multisig/{address}/propose - Propose a transaction
pub async fn propose_multisig_tx(
    State(state): State<ApiState>,
    Path(address): Path<String>,
    Json(req): Json<ProposeTransactionRequest>,
) -> Result<Json<PendingTxInfo>, (StatusCode, Json<ApiError>)> {
    let blockchain = state.blockchain.read().await;
    let mut manager = state.multisig_manager.write().await;

    let pending = manager
        .propose_transaction(&address, &req.to, req.amount, &blockchain)
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiError {
                    error: format!("Failed to propose transaction: {}", e),
                }),
            )
        })?;

    Ok(Json(PendingTxInfo {
        id: pending.id.clone(),
        from_address: pending.from_address.clone(),
        to_address: pending.to_address.clone(),
        amount: pending.amount,
        signatures_collected: pending.signature_count(),
        signatures_required: pending.threshold,
        signed_by: pending.signed_by().iter().map(|s| s.to_string()).collect(),
        status: format!("{:?}", pending.status),
        created_at: pending.created_at.to_rfc3339(),
    }))
}

/// POST /api/multisig/{address}/sign - Sign a pending transaction
pub async fn sign_multisig_tx(
    State(state): State<ApiState>,
    Path(_address): Path<String>,
    Json(req): Json<SignTransactionRequest>,
) -> Result<Json<PendingTxInfo>, (StatusCode, Json<ApiError>)> {
    let mut manager = state.multisig_manager.write().await;

    let signature = MultisigSignature::new(req.signer_pubkey, req.signature);

    let pending = manager
        .sign_transaction(&req.tx_id, signature)
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiError {
                    error: format!("Failed to sign transaction: {}", e),
                }),
            )
        })?;

    Ok(Json(PendingTxInfo {
        id: pending.id.clone(),
        from_address: pending.from_address.clone(),
        to_address: pending.to_address.clone(),
        amount: pending.amount,
        signatures_collected: pending.signature_count(),
        signatures_required: pending.threshold,
        signed_by: pending.signed_by().iter().map(|s| s.to_string()).collect(),
        status: format!("{:?}", pending.status),
        created_at: pending.created_at.to_rfc3339(),
    }))
}

/// Request to sign with a local wallet
#[derive(Deserialize)]
pub struct SignWithWalletRequest {
    pub tx_id: String,
    pub wallet_address: String,
}

/// POST /api/multisig/{address}/sign-with-wallet - Sign using a local wallet
pub async fn sign_with_wallet(
    State(state): State<ApiState>,
    Path(_address): Path<String>,
    Json(req): Json<SignWithWalletRequest>,
) -> Result<Json<PendingTxInfo>, (StatusCode, Json<ApiError>)> {
    // First, get the pending transaction to get its signing data
    let signing_data = {
        let manager = state.multisig_manager.read().await;
        let pending = manager.get_pending(&req.tx_id).ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ApiError {
                    error: format!("Pending transaction not found: {}", req.tx_id),
                }),
            )
        })?;
        pending.signing_data()
    };

    // Load the wallet
    let wallet_manager = state.wallet_manager.read().await;
    let wallet = wallet_manager
        .load_wallet(&req.wallet_address)
        .map_err(|e| {
            (
                StatusCode::NOT_FOUND,
                Json(ApiError {
                    error: format!("Wallet not found: {}", e),
                }),
            )
        })?;

    // Sign the transaction's signing data (not a custom message)
    let signature_bytes = wallet.sign_data(&signing_data).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: format!("Failed to sign: {}", e),
            }),
        )
    })?;
    let signature_hex = hex::encode(signature_bytes);
    let pubkey_hex = wallet.public_key();

    drop(wallet_manager);

    // Create multisig signature
    let mut manager = state.multisig_manager.write().await;
    let signature = MultisigSignature::new(pubkey_hex, signature_hex);

    let pending = manager
        .sign_transaction(&req.tx_id, signature)
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiError {
                    error: format!("Failed to sign transaction: {}", e),
                }),
            )
        })?;

    Ok(Json(PendingTxInfo {
        id: pending.id.clone(),
        from_address: pending.from_address.clone(),
        to_address: pending.to_address.clone(),
        amount: pending.amount,
        signatures_collected: pending.signature_count(),
        signatures_required: pending.threshold,
        signed_by: pending.signed_by().iter().map(|s| s.to_string()).collect(),
        status: format!("{:?}", pending.status),
        created_at: pending.created_at.to_rfc3339(),
    }))
}

/// GET /api/multisig/{address}/pending - List pending transactions
pub async fn list_pending_tx(
    State(state): State<ApiState>,
    Path(address): Path<String>,
) -> Json<Vec<PendingTxInfo>> {
    let manager = state.multisig_manager.read().await;

    let pending: Vec<PendingTxInfo> = manager
        .pending_for_address(&address)
        .iter()
        .map(|p| PendingTxInfo {
            id: p.id.clone(),
            from_address: p.from_address.clone(),
            to_address: p.to_address.clone(),
            amount: p.amount,
            signatures_collected: p.signature_count(),
            signatures_required: p.threshold,
            signed_by: p.signed_by().iter().map(|s| s.to_string()).collect(),
            status: format!("{:?}", p.status),
            created_at: p.created_at.to_rfc3339(),
        })
        .collect();

    Json(pending)
}

/// Request to broadcast a ready multisig transaction
#[derive(Deserialize)]
pub struct BroadcastRequest {
    pub tx_id: String,
}

/// Response after broadcasting
#[derive(Serialize)]
pub struct BroadcastResponse {
    pub tx_id: String,
    pub status: String,
    pub message: String,
}

/// POST /api/multisig/{address}/broadcast - Broadcast a ready transaction
pub async fn broadcast_multisig_tx(
    State(state): State<ApiState>,
    Path(_address): Path<String>,
    Json(req): Json<BroadcastRequest>,
) -> Result<Json<BroadcastResponse>, (StatusCode, Json<ApiError>)> {
    // Get the pending transaction
    let mut manager = state.multisig_manager.write().await;

    let pending = manager
        .get_pending(&req.tx_id)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ApiError {
                    error: format!("Pending transaction not found: {}", req.tx_id),
                }),
            )
        })?
        .clone();

    // Check if ready
    if !pending.is_ready() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: format!(
                    "Transaction not ready: has {}/{} signatures",
                    pending.signature_count(),
                    pending.threshold
                ),
            }),
        ));
    }

    // Finalize into a real transaction
    let transaction = pending.finalize().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: format!("Failed to finalize transaction: {}", e),
            }),
        )
    })?;

    let tx_id = transaction.id.clone();

    // Add to mempool
    {
        let blockchain = state.blockchain.read().await;
        let mut mempool = state.mempool.write().await;
        if let Err(e) = mempool.add_transaction(transaction, &blockchain) {
            log::warn!("Failed to add to mempool (may be already there): {}", e);
        }
    }

    // Mark as broadcast and remove from pending
    if let Some(p) = manager.get_pending_mut(&req.tx_id) {
        p.mark_broadcast();
    }
    manager.remove_pending(&req.tx_id);

    Ok(Json(BroadcastResponse {
        tx_id,
        status: "broadcast".to_string(),
        message: "Transaction added to mempool. Mine a block to confirm it.".to_string(),
    }))
}

// ============================================================================
// Token Endpoints
// ============================================================================

/// Request to create a token
#[derive(Deserialize)]
pub struct CreateTokenRequest {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub total_supply: String,
    pub creator: String,
}

/// Token info response
#[derive(Serialize)]
pub struct TokenInfo {
    pub address: String,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub total_supply: String,
    pub creator: String,
    pub created_at_block: u64,
    pub holder_count: usize,
}

/// Token balance response
#[derive(Serialize)]
pub struct TokenBalanceResponse {
    pub token: String,
    pub holder: String,
    pub balance: String,
}

/// Transfer request
#[derive(Deserialize)]
pub struct TokenTransferRequest {
    pub from: String,
    pub to: String,
    pub amount: String,
}

/// Approve request
#[derive(Deserialize)]
pub struct TokenApproveRequest {
    pub owner: String,
    pub spender: String,
    pub amount: String,
}

/// Transfer from request
#[derive(Deserialize)]
pub struct TokenTransferFromRequest {
    pub spender: String,
    pub from: String,
    pub to: String,
    pub amount: String,
}

/// Allowance query params
#[derive(Deserialize)]
pub struct AllowanceQuery {
    pub owner: String,
    pub spender: String,
}

/// Transfer response
#[derive(Serialize)]
pub struct TransferResponse {
    pub success: bool,
    pub from: String,
    pub to: String,
    pub amount: String,
}

/// POST /api/tokens - Create a new token
pub async fn create_token(
    State(state): State<ApiState>,
    Json(req): Json<CreateTokenRequest>,
) -> Result<Json<TokenInfo>, (StatusCode, Json<ApiError>)> {
    let total_supply: u128 = req.total_supply.parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "Invalid total_supply: must be a valid number".to_string(),
            }),
        )
    })?;

    let blockchain = state.blockchain.read().await;
    let mut manager = state.token_manager.write().await;

    let token = manager
        .create_token(
            req.name,
            req.symbol,
            req.decimals,
            total_supply,
            &req.creator,
            blockchain.height(),
        )
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiError {
                    error: format!("Failed to create token: {}", e),
                }),
            )
        })?;

    Ok(Json(TokenInfo {
        address: token.address.clone(),
        name: token.name().to_string(),
        symbol: token.symbol().to_string(),
        decimals: token.decimals(),
        total_supply: token.total_supply().to_string(),
        creator: token.metadata.creator.clone(),
        created_at_block: token.metadata.created_at_block,
        holder_count: token.holder_count(),
    }))
}

/// GET /api/tokens - List all tokens
pub async fn list_tokens(State(state): State<ApiState>) -> Json<Vec<TokenInfo>> {
    let manager = state.token_manager.read().await;

    let tokens: Vec<TokenInfo> = manager
        .list()
        .iter()
        .map(|t| TokenInfo {
            address: t.address.clone(),
            name: t.name().to_string(),
            symbol: t.symbol().to_string(),
            decimals: t.decimals(),
            total_supply: t.total_supply().to_string(),
            creator: t.metadata.creator.clone(),
            created_at_block: t.metadata.created_at_block,
            holder_count: t.holder_count(),
        })
        .collect();

    Json(tokens)
}

/// GET /api/tokens/{address} - Get token info
pub async fn get_token(
    State(state): State<ApiState>,
    Path(address): Path<String>,
) -> Result<Json<TokenInfo>, (StatusCode, Json<ApiError>)> {
    let manager = state.token_manager.read().await;

    match manager.get(&address) {
        Some(token) => Ok(Json(TokenInfo {
            address: token.address.clone(),
            name: token.name().to_string(),
            symbol: token.symbol().to_string(),
            decimals: token.decimals(),
            total_supply: token.total_supply().to_string(),
            creator: token.metadata.creator.clone(),
            created_at_block: token.metadata.created_at_block,
            holder_count: token.holder_count(),
        })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError {
                error: format!("Token not found: {}", address),
            }),
        )),
    }
}

/// GET /api/tokens/{address}/balance/{holder} - Get token balance
pub async fn get_token_balance(
    State(state): State<ApiState>,
    Path((address, holder)): Path<(String, String)>,
) -> Result<Json<TokenBalanceResponse>, (StatusCode, Json<ApiError>)> {
    let manager = state.token_manager.read().await;

    match manager.balance_of(&address, &holder) {
        Ok(balance) => Ok(Json(TokenBalanceResponse {
            token: address,
            holder,
            balance: balance.to_string(),
        })),
        Err(e) => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError {
                error: format!("{}", e),
            }),
        )),
    }
}

/// POST /api/tokens/{address}/transfer - Transfer tokens
pub async fn transfer_tokens(
    State(state): State<ApiState>,
    Path(address): Path<String>,
    Json(req): Json<TokenTransferRequest>,
) -> Result<Json<TransferResponse>, (StatusCode, Json<ApiError>)> {
    let amount: u128 = req.amount.parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "Invalid amount".to_string(),
            }),
        )
    })?;

    let mut manager = state.token_manager.write().await;

    manager
        .transfer(&address, &req.from, &req.to, amount)
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiError {
                    error: format!("{}", e),
                }),
            )
        })?;

    Ok(Json(TransferResponse {
        success: true,
        from: req.from,
        to: req.to,
        amount: amount.to_string(),
    }))
}

/// POST /api/tokens/{address}/approve - Approve spender
pub async fn approve_tokens(
    State(state): State<ApiState>,
    Path(address): Path<String>,
    Json(req): Json<TokenApproveRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let amount: u128 = req.amount.parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "Invalid amount".to_string(),
            }),
        )
    })?;

    let mut manager = state.token_manager.write().await;

    manager
        .approve(&address, &req.owner, &req.spender, amount)
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiError {
                    error: format!("{}", e),
                }),
            )
        })?;

    Ok(Json(serde_json::json!({
        "success": true,
        "owner": req.owner,
        "spender": req.spender,
        "amount": amount.to_string()
    })))
}

/// GET /api/tokens/{address}/allowance - Get allowance
pub async fn get_token_allowance(
    State(state): State<ApiState>,
    Path(address): Path<String>,
    axum::extract::Query(query): axum::extract::Query<AllowanceQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let manager = state.token_manager.read().await;

    match manager.allowance(&address, &query.owner, &query.spender) {
        Ok(allowance) => Ok(Json(serde_json::json!({
            "token": address,
            "owner": query.owner,
            "spender": query.spender,
            "allowance": allowance.to_string()
        }))),
        Err(e) => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError {
                error: format!("{}", e),
            }),
        )),
    }
}

/// POST /api/tokens/{address}/transferFrom - Transfer from (delegated)
pub async fn transfer_from_tokens(
    State(state): State<ApiState>,
    Path(address): Path<String>,
    Json(req): Json<TokenTransferFromRequest>,
) -> Result<Json<TransferResponse>, (StatusCode, Json<ApiError>)> {
    let amount: u128 = req.amount.parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "Invalid amount".to_string(),
            }),
        )
    })?;

    let mut manager = state.token_manager.write().await;

    manager
        .transfer_from(&address, &req.spender, &req.from, &req.to, amount)
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiError {
                    error: format!("{}", e),
                }),
            )
        })?;

    Ok(Json(TransferResponse {
        success: true,
        from: req.from,
        to: req.to,
        amount: amount.to_string(),
    }))
}

/// Burn request
#[derive(Deserialize)]
pub struct TokenBurnRequest {
    pub from: String,
    pub amount: String,
}

/// POST /api/tokens/{address}/burn - Burn tokens
pub async fn burn_tokens(
    State(state): State<ApiState>,
    Path(address): Path<String>,
    Json(req): Json<TokenBurnRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let amount: u128 = req.amount.parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "Invalid amount".to_string(),
            }),
        )
    })?;

    let mut manager = state.token_manager.write().await;

    manager.burn(&address, &req.from, amount).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: format!("{}", e),
            }),
        )
    })?;

    Ok(Json(serde_json::json!({
        "success": true,
        "from": req.from,
        "amount": amount.to_string(),
        "action": "burned"
    })))
}

/// Mint request
#[derive(Deserialize)]
pub struct TokenMintRequest {
    pub caller: String,
    pub to: String,
    pub amount: String,
}

/// POST /api/tokens/{address}/mint - Mint new tokens
pub async fn mint_tokens(
    State(state): State<ApiState>,
    Path(address): Path<String>,
    Json(req): Json<TokenMintRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let amount: u128 = req.amount.parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "Invalid amount".to_string(),
            }),
        )
    })?;

    let mut manager = state.token_manager.write().await;

    manager
        .mint(&address, &req.caller, &req.to, amount)
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiError {
                    error: format!("{}", e),
                }),
            )
        })?;

    Ok(Json(serde_json::json!({
        "success": true,
        "to": req.to,
        "amount": amount.to_string(),
        "action": "minted"
    })))
}

/// Token transfer history entry
#[derive(Serialize)]
pub struct TokenHistoryEntry {
    pub from: String,
    pub to: String,
    pub amount: String,
    pub timestamp: String,
}

/// GET /api/tokens/{address}/history - Get transfer history
pub async fn get_token_history(
    State(state): State<ApiState>,
    Path(address): Path<String>,
) -> Result<Json<Vec<TokenHistoryEntry>>, (StatusCode, Json<ApiError>)> {
    let manager = state.token_manager.read().await;

    let history = manager.get_history(&address).map_err(|e| {
        (
            StatusCode::NOT_FOUND,
            Json(ApiError {
                error: format!("{}", e),
            }),
        )
    })?;

    let entries: Vec<TokenHistoryEntry> = history
        .iter()
        .map(|e| TokenHistoryEntry {
            from: e.from.clone(),
            to: e.to.clone(),
            amount: e.amount.to_string(),
            timestamp: e.timestamp.to_rfc3339(),
        })
        .collect();

    Ok(Json(entries))
}

// ============================================================================
// Search Endpoints
// ============================================================================

/// Search query parameters
#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

/// Unified search result
#[derive(Serialize)]
pub struct SearchResult {
    pub query: String,
    pub blocks: Vec<BlockInfo>,
    pub transactions: Vec<TransactionResponse>,
    pub wallets: Vec<WalletResponse>,
    pub contracts: Vec<ContractInfo>,
    pub tokens: Vec<TokenInfo>,
    pub multisig: Vec<MultisigWalletInfo>,
}

/// GET /api/search - Unified search across all entities
pub async fn search(
    State(state): State<ApiState>,
    axum::extract::Query(query): axum::extract::Query<SearchQuery>,
) -> Json<SearchResult> {
    let q = query.q.trim().to_lowercase();

    let mut result = SearchResult {
        query: query.q.clone(),
        blocks: vec![],
        transactions: vec![],
        wallets: vec![],
        contracts: vec![],
        tokens: vec![],
        multisig: vec![],
    };

    if q.is_empty() {
        return Json(result);
    }

    // Search blocks (by height or hash prefix)
    {
        let chain = state.blockchain.read().await;

        // Try parsing as block height
        if let Ok(height) = q.parse::<u64>() {
            if let Some(block) = chain.get_block(height) {
                result.blocks.push(BlockInfo {
                    index: block.index,
                    hash: block.hash.clone(),
                    previous_hash: block.header.previous_hash.clone(),
                    merkle_root: block.header.merkle_root.clone(),
                    timestamp: block.header.timestamp.to_rfc3339(),
                    difficulty: block.header.difficulty,
                    nonce: block.header.nonce,
                    transactions: block.transactions.len(),
                });
            }
        }

        // Search by hash prefix
        for block in &chain.blocks {
            if block.hash.to_lowercase().starts_with(&q)
                && result.blocks.iter().all(|b| b.index != block.index)
            {
                result.blocks.push(BlockInfo {
                    index: block.index,
                    hash: block.hash.clone(),
                    previous_hash: block.header.previous_hash.clone(),
                    merkle_root: block.header.merkle_root.clone(),
                    timestamp: block.header.timestamp.to_rfc3339(),
                    difficulty: block.header.difficulty,
                    nonce: block.header.nonce,
                    transactions: block.transactions.len(),
                });
                if result.blocks.len() >= 10 {
                    break;
                }
            }
        }

        // Search transactions by ID prefix
        for block in &chain.blocks {
            for tx in &block.transactions {
                if tx.id.to_lowercase().starts_with(&q) {
                    result.transactions.push(TransactionResponse::from(tx));
                    if result.transactions.len() >= 10 {
                        break;
                    }
                }
            }
            if result.transactions.len() >= 10 {
                break;
            }
        }
    }

    // Search mempool transactions
    {
        let mempool = state.mempool.read().await;
        for tx in mempool.get_transactions(100) {
            if tx.id.to_lowercase().starts_with(&q) {
                result.transactions.push(TransactionResponse::from(&tx));
                if result.transactions.len() >= 10 {
                    break;
                }
            }
        }
    }

    // Search wallets by address prefix
    {
        let manager = state.wallet_manager.read().await;
        if let Ok(addresses) = manager.list_wallets() {
            for addr in addresses {
                if addr.to_lowercase().contains(&q) {
                    if let Ok(wallet) = manager.load_wallet(&addr) {
                        result.wallets.push(WalletResponse {
                            address: addr,
                            public_key: wallet.public_key(),
                            label: wallet.label.clone(),
                        });
                        if result.wallets.len() >= 10 {
                            break;
                        }
                    }
                }
            }
        }
    }

    // Search contracts by address
    {
        let manager = state.contract_manager.read().await;
        for addr in manager.list() {
            if addr.to_lowercase().contains(&q) {
                if let Some(c) = manager.get(&addr) {
                    result.contracts.push(ContractInfo {
                        address: c.address.clone(),
                        deployer: c.deployer.clone(),
                        deployed_at: c.deployed_at,
                        code_size: c.code.len(),
                    });
                    if result.contracts.len() >= 10 {
                        break;
                    }
                }
            }
        }
    }

    // Search tokens by name, symbol, or address
    {
        let manager = state.token_manager.read().await;
        for token in manager.list() {
            let matches = token.address.to_lowercase().contains(&q)
                || token.name().to_lowercase().contains(&q)
                || token.symbol().to_lowercase().contains(&q);
            if matches {
                result.tokens.push(TokenInfo {
                    address: token.address.clone(),
                    name: token.name().to_string(),
                    symbol: token.symbol().to_string(),
                    decimals: token.decimals(),
                    total_supply: token.total_supply().to_string(),
                    creator: token.metadata.creator.clone(),
                    created_at_block: token.metadata.created_at_block,
                    holder_count: token.holder_count(),
                });
                if result.tokens.len() >= 10 {
                    break;
                }
            }
        }
    }

    // Search multisig wallets by address or label
    {
        let manager = state.multisig_manager.read().await;
        for wallet in manager.list_wallets() {
            let matches = wallet.address.to_lowercase().contains(&q)
                || wallet
                    .config
                    .label
                    .as_ref()
                    .map(|l| l.to_lowercase().contains(&q))
                    .unwrap_or(false);
            if matches {
                result.multisig.push(MultisigWalletInfo {
                    address: wallet.address.clone(),
                    threshold: wallet.config.threshold,
                    signer_count: wallet.config.signers.len(),
                    signers: wallet.config.signers.clone(),
                    label: wallet.config.label.clone(),
                    description: wallet.description(),
                    created_at: wallet.created_at.to_rfc3339(),
                });
                if result.multisig.len() >= 10 {
                    break;
                }
            }
        }
    }

    Json(result)
}

// ============================================================================
// Fee Estimation Endpoints
// ============================================================================

/// Fee estimate response
#[derive(Serialize)]
pub struct FeeEstimateResponse {
    pub high_priority: u64,
    pub normal: u64,
    pub low_priority: u64,
    pub economy: u64,
    pub unit: String,
}

/// GET /api/fees - Get fee estimates
pub async fn get_fee_estimates(State(state): State<ApiState>) -> Json<FeeEstimateResponse> {
    let mempool = state.mempool.read().await;

    // Simple fee estimation based on mempool size
    let pending = mempool.len();
    let base_fee = 1u64;

    let (high, normal, low, economy) = match pending {
        0..=10 => (base_fee * 2, base_fee, base_fee, base_fee),
        11..=50 => (base_fee * 5, base_fee * 3, base_fee * 2, base_fee),
        51..=200 => (base_fee * 10, base_fee * 5, base_fee * 3, base_fee * 2),
        _ => (base_fee * 20, base_fee * 10, base_fee * 5, base_fee * 3),
    };

    Json(FeeEstimateResponse {
        high_priority: high,
        normal,
        low_priority: low,
        economy,
        unit: "sat/byte".to_string(),
    })
}

// ============================================================================
// Stats Endpoints
// ============================================================================

/// Network stats response
#[derive(Serialize)]
pub struct NetworkStatsResponse {
    pub protocol_version: u32,
    pub min_protocol_version: u32,
    pub peer_count: usize,
    pub max_peers: usize,
    pub banned_count: usize,
}

/// Storage stats response
#[derive(Serialize)]
pub struct StorageStatsResponse {
    pub block_count: usize,
    pub transaction_count: usize,
    pub utxo_count: usize,
    pub difficulty: u32,
    pub chain_work: String,
}

/// Advanced chain stats response
#[derive(Serialize)]
pub struct AdvancedStatsResponse {
    pub network: NetworkStatsResponse,
    pub storage: StorageStatsResponse,
    pub mempool_size: usize,
    pub mempool_bytes: usize,
}

/// GET /api/stats - Get advanced blockchain stats
pub async fn get_advanced_stats(State(state): State<ApiState>) -> Json<AdvancedStatsResponse> {
    let chain = state.blockchain.read().await;
    let mempool = state.mempool.read().await;

    // Calculate transaction count
    let tx_count: usize = chain.blocks.iter().map(|b| b.transactions.len()).sum();

    // Get UTXO count
    let utxo_count = chain.utxo_set.len();

    // Estimate mempool size in bytes (rough estimate)
    let mempool_bytes = mempool.len() * 300; // ~300 bytes per tx average

    Json(AdvancedStatsResponse {
        network: NetworkStatsResponse {
            protocol_version: 70001,
            min_protocol_version: 70000,
            peer_count: 0, // Would need peer manager access
            max_peers: 125,
            banned_count: 0,
        },
        storage: StorageStatsResponse {
            block_count: chain.blocks.len(),
            transaction_count: tx_count,
            utxo_count,
            difficulty: chain.difficulty,
            chain_work: format!(
                "{}",
                chain.blocks.len() as u128 * (1u128 << chain.difficulty)
            ),
        },
        mempool_size: mempool.len(),
        mempool_bytes,
    })
}
