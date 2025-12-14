// API client for blockchain backend

const API_BASE = '/api';

export interface ChainInfo {
    height: number;
    difficulty: number;
    total_blocks: number;
    total_transactions: number;
    total_coins: number;
    latest_hash: string;
}

export interface BlockInfo {
    index: number;
    hash: string;
    previous_hash: string;
    merkle_root: string;
    timestamp: string;
    difficulty: number;
    nonce: number;
    transactions: number;
}

export interface WalletResponse {
    address: string;
    label: string | null;
}

export interface BalanceResponse {
    address: string;
    balance: number;
    utxo_count: number;
}

export interface MineResponse {
    block: BlockInfo;
    reward: number;
    time_ms: number;
    attempts: number;
}

export interface MempoolResponse {
    pending_transactions: number;
    transactions: TransactionResponse[];
}

export interface TransactionResponse {
    id: string;
    is_coinbase: boolean;
    inputs: number;
    outputs: number;
    total_output: number;
}

export interface ValidationResponse {
    valid: boolean;
    blocks_checked: number;
    message: string;
}

// Chain endpoints
export async function getChainInfo(): Promise<ChainInfo> {
    const res = await fetch(`${API_BASE}/chain`);
    return res.json();
}

export async function getBlocks(): Promise<BlockInfo[]> {
    const res = await fetch(`${API_BASE}/chain/blocks`);
    return res.json();
}

export async function getBlock(height: number): Promise<BlockInfo> {
    const res = await fetch(`${API_BASE}/chain/blocks/${height}`);
    return res.json();
}

export async function validateChain(): Promise<ValidationResponse> {
    const res = await fetch(`${API_BASE}/chain/validate`);
    return res.json();
}

// Mining endpoints
export async function mineBlock(address: string): Promise<MineResponse> {
    const res = await fetch(`${API_BASE}/mine`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ miner_address: address })
    });
    return res.json();
}

// Wallet endpoints
export async function getWallets(): Promise<WalletResponse[]> {
    const res = await fetch(`${API_BASE}/wallets`);
    return res.json();
}

export async function createWallet(label?: string): Promise<WalletResponse> {
    const res = await fetch(`${API_BASE}/wallets`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ label })
    });
    return res.json();
}

export async function getBalance(address: string): Promise<BalanceResponse> {
    const res = await fetch(`${API_BASE}/wallets/${address}/balance`);
    return res.json();
}

// Transaction endpoints
export async function getMempool(): Promise<MempoolResponse> {
    const res = await fetch(`${API_BASE}/mempool`);
    return res.json();
}

export async function getTransaction(id: string): Promise<TransactionResponse> {
    const res = await fetch(`${API_BASE}/transactions/${id}`);
    return res.json();
}

// Health check
export async function healthCheck(): Promise<boolean> {
    try {
        const res = await fetch('/health');
        return res.ok;
    } catch {
        return false;
    }
}

// Contract types
export interface ContractInfo {
    address: string;
    deployer: string;
    deployed_at: number;
    code_size: number;
}

export interface DeployResponse {
    address: string;
    code_size: number;
}

export interface CallResponse {
    success: boolean;
    return_value: number | null;
    gas_used: number;
}

// Contract endpoints
export async function listContracts(): Promise<ContractInfo[]> {
    const res = await fetch(`${API_BASE}/contracts`);
    return res.json();
}

export async function deployContract(source: string): Promise<DeployResponse> {
    const res = await fetch(`${API_BASE}/contracts`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ source })
    });
    if (!res.ok) {
        const error = await res.json();
        throw new Error(error.error || 'Deploy failed');
    }
    return res.json();
}

export async function getContract(address: string): Promise<ContractInfo> {
    const res = await fetch(`${API_BASE}/contracts/${address}`);
    return res.json();
}

export async function callContract(address: string, args: number[], gasLimit?: number): Promise<CallResponse> {
    const res = await fetch(`${API_BASE}/contracts/${address}/call`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ args, gas_limit: gasLimit })
    });
    if (!res.ok) {
        const error = await res.json();
        throw new Error(error.error || 'Call failed');
    }
    return res.json();
}
