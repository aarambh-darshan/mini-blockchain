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
    public_key: string;
    label: string | null;
}

export interface BalanceResponse {
    address: string;
    balance: number;
    spendable_balance: number;
    immature_balance: number;
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
    gas_cost: number;
    caller_balance: number | null;
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

export async function callContract(
    address: string,
    args: number[],
    options?: {
        gasLimit?: number;
        gasPrice?: number;
        callerAddress?: string;
    }
): Promise<CallResponse> {
    const res = await fetch(`${API_BASE}/contracts/${address}/call`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
            args,
            gas_limit: options?.gasLimit,
            gas_price: options?.gasPrice,
            caller_address: options?.callerAddress
        })
    });
    if (!res.ok) {
        const error = await res.json();
        throw new Error(error.error || 'Call failed');
    }
    return res.json();
}

// Multisig types
export interface MultisigWalletInfo {
    address: string;
    threshold: number;
    signer_count: number;
    signers: string[];
    label: string | null;
    description: string;
    created_at: string;
}

export interface PendingTxInfo {
    id: string;
    from_address: string;
    to_address: string;
    amount: number;
    signatures_collected: number;
    signatures_required: number;
    signed_by: string[];
    status: string;
    created_at: string;
}

// Multisig endpoints
export async function listMultisig(): Promise<MultisigWalletInfo[]> {
    const res = await fetch(`${API_BASE}/multisig`);
    return res.json();
}

export async function createMultisig(
    threshold: number,
    signers: string[],
    label?: string
): Promise<MultisigWalletInfo> {
    const res = await fetch(`${API_BASE}/multisig`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ threshold, signers, label })
    });
    if (!res.ok) {
        const error = await res.json();
        throw new Error(error.error || 'Create multisig failed');
    }
    return res.json();
}

export async function getMultisig(address: string): Promise<MultisigWalletInfo> {
    const res = await fetch(`${API_BASE}/multisig/${address}`);
    if (!res.ok) {
        const error = await res.json();
        throw new Error(error.error || 'Multisig not found');
    }
    return res.json();
}

export async function getMultisigBalance(address: string): Promise<BalanceResponse> {
    const res = await fetch(`${API_BASE}/multisig/${address}/balance`);
    if (!res.ok) {
        const error = await res.json();
        throw new Error(error.error || 'Balance fetch failed');
    }
    return res.json();
}

export async function proposeMultisigTx(
    address: string,
    to: string,
    amount: number
): Promise<PendingTxInfo> {
    const res = await fetch(`${API_BASE}/multisig/${address}/propose`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ to, amount })
    });
    if (!res.ok) {
        const error = await res.json();
        throw new Error(error.error || 'Propose failed');
    }
    return res.json();
}

export async function signMultisigTx(
    address: string,
    txId: string,
    signerPubkey: string,
    signature: string
): Promise<PendingTxInfo> {
    const res = await fetch(`${API_BASE}/multisig/${address}/sign`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ tx_id: txId, signer_pubkey: signerPubkey, signature })
    });
    if (!res.ok) {
        const error = await res.json();
        throw new Error(error.error || 'Sign failed');
    }
    return res.json();
}

export async function listPendingTx(address: string): Promise<PendingTxInfo[]> {
    const res = await fetch(`${API_BASE}/multisig/${address}/pending`);
    return res.json();
}

export async function signWithWallet(
    multisigAddress: string,
    txId: string,
    walletAddress: string
): Promise<PendingTxInfo> {
    const res = await fetch(`${API_BASE}/multisig/${multisigAddress}/sign-with-wallet`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ tx_id: txId, wallet_address: walletAddress })
    });
    if (!res.ok) {
        const error = await res.json();
        throw new Error(error.error || 'Sign failed');
    }
    return res.json();
}

export interface BroadcastResponse {
    tx_id: string;
    status: string;
    message: string;
}

export async function broadcastMultisigTx(
    multisigAddress: string,
    txId: string
): Promise<BroadcastResponse> {
    const res = await fetch(`${API_BASE}/multisig/${multisigAddress}/broadcast`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ tx_id: txId })
    });
    if (!res.ok) {
        const error = await res.json();
        throw new Error(error.error || 'Broadcast failed');
    }
    return res.json();
}

// Token types (ERC-20 style)
export interface TokenInfo {
    address: string;
    name: string;
    symbol: string;
    decimals: number;
    total_supply: string;
    current_supply: string;
    creator: string;
    created_at_block: number;
    holder_count: number;
    is_mintable: boolean;
    minter: string;
}

export interface TokenBalanceResponse {
    token: string;
    holder: string;
    balance: string;
}

export interface TransferResponse {
    success: boolean;
    from: string;
    to: string;
    amount: string;
}

// Token endpoints
export async function listTokens(): Promise<TokenInfo[]> {
    const res = await fetch(`${API_BASE}/tokens`);
    return res.json();
}

export async function createToken(
    name: string,
    symbol: string,
    decimals: number,
    totalSupply: string,
    creator: string,
    isMintable: boolean = false
): Promise<TokenInfo> {
    const res = await fetch(`${API_BASE}/tokens`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
            name,
            symbol,
            decimals,
            total_supply: totalSupply,
            creator,
            is_mintable: isMintable
        })
    });
    if (!res.ok) {
        const error = await res.json();
        throw new Error(error.error || 'Create token failed');
    }
    return res.json();
}

export async function getToken(address: string): Promise<TokenInfo> {
    const res = await fetch(`${API_BASE}/tokens/${address}`);
    if (!res.ok) {
        const error = await res.json();
        throw new Error(error.error || 'Token not found');
    }
    return res.json();
}

export async function getTokenBalance(
    tokenAddress: string,
    holder: string
): Promise<TokenBalanceResponse> {
    const res = await fetch(`${API_BASE}/tokens/${tokenAddress}/balance/${holder}`);
    if (!res.ok) {
        const error = await res.json();
        throw new Error(error.error || 'Balance fetch failed');
    }
    return res.json();
}

export async function transferTokens(
    tokenAddress: string,
    from: string,
    to: string,
    amount: string
): Promise<TransferResponse> {
    const res = await fetch(`${API_BASE}/tokens/${tokenAddress}/transfer`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ from, to, amount })
    });
    if (!res.ok) {
        const error = await res.json();
        throw new Error(error.error || 'Transfer failed');
    }
    return res.json();
}

export async function approveToken(
    tokenAddress: string,
    owner: string,
    spender: string,
    amount: string
): Promise<{ success: boolean }> {
    const res = await fetch(`${API_BASE}/tokens/${tokenAddress}/approve`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ owner, spender, amount })
    });
    if (!res.ok) {
        const error = await res.json();
        throw new Error(error.error || 'Approve failed');
    }
    return res.json();
}

export async function getTokenAllowance(
    tokenAddress: string,
    owner: string,
    spender: string
): Promise<{ allowance: string }> {
    const res = await fetch(
        `${API_BASE}/tokens/${tokenAddress}/allowance?owner=${encodeURIComponent(owner)}&spender=${encodeURIComponent(spender)}`
    );
    if (!res.ok) {
        const error = await res.json();
        throw new Error(error.error || 'Allowance fetch failed');
    }
    return res.json();
}

export interface TokenHistoryEntry {
    from: string;
    to: string;
    amount: string;
    timestamp: string;
}

export async function burnTokens(
    tokenAddress: string,
    from: string,
    amount: string
): Promise<{ success: boolean }> {
    const res = await fetch(`${API_BASE}/tokens/${tokenAddress}/burn`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ from, amount })
    });
    if (!res.ok) {
        const error = await res.json();
        throw new Error(error.error || 'Burn failed');
    }
    return res.json();
}

export async function mintTokens(
    tokenAddress: string,
    caller: string,
    to: string,
    amount: string
): Promise<{ success: boolean }> {
    const res = await fetch(`${API_BASE}/tokens/${tokenAddress}/mint`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ caller, to, amount })
    });
    if (!res.ok) {
        const error = await res.json();
        throw new Error(error.error || 'Mint failed');
    }
    return res.json();
}

export async function getTokenHistory(
    tokenAddress: string
): Promise<TokenHistoryEntry[]> {
    const res = await fetch(`${API_BASE}/tokens/${tokenAddress}/history`);
    if (!res.ok) {
        const error = await res.json();
        throw new Error(error.error || 'History fetch failed');
    }
    return res.json();
}

export async function transferFromTokens(
    tokenAddress: string,
    spender: string,
    from: string,
    to: string,
    amount: string
): Promise<TransferResponse> {
    const res = await fetch(`${API_BASE}/tokens/${tokenAddress}/transferFrom`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ spender, from, to, amount })
    });
    if (!res.ok) {
        const error = await res.json();
        throw new Error(error.error || 'Transfer from failed');
    }
    return res.json();
}

// Search types
export interface SearchResult {
    query: string;
    blocks: BlockInfo[];
    transactions: TransactionResponse[];
    wallets: WalletResponse[];
    contracts: ContractInfo[];
    tokens: TokenInfo[];
    multisig: MultisigWalletInfo[];
}

// Search endpoint
export async function search(query: string): Promise<SearchResult> {
    const res = await fetch(`${API_BASE}/search?q=${encodeURIComponent(query)}`);
    if (!res.ok) {
        const error = await res.json();
        throw new Error(error.error || 'Search failed');
    }
    return res.json();
}

// Fee estimation types
export interface FeeEstimate {
    high_priority: number;
    normal: number;
    low_priority: number;
    economy: number;
    unit: string;
}

// Advanced stats types
export interface NetworkStats {
    protocol_version: number;
    min_protocol_version: number;
    peer_count: number;
    max_peers: number;
    banned_count: number;
}

export interface StorageStats {
    block_count: number;
    transaction_count: number;
    utxo_count: number;
    difficulty: number;
    chain_work: string;
}

export interface AdvancedStats {
    network: NetworkStats;
    storage: StorageStats;
    mempool_size: number;
    mempool_bytes: number;
}

// Fee estimation endpoint
export async function getFeeEstimates(): Promise<FeeEstimate> {
    const res = await fetch(`${API_BASE}/fees`);
    return res.json();
}

// Advanced stats endpoint
export async function getAdvancedStats(): Promise<AdvancedStats> {
    const res = await fetch(`${API_BASE}/stats`);
    return res.json();
}
