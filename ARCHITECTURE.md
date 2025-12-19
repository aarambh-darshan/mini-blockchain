# 🏗️ Mini-Blockchain Architecture

A comprehensive guide to understanding the mini-blockchain project architecture.

---

## 📁 Project Structure

```
src/
├── api/          # REST API & WebSocket handlers
├── cli/          # Command-line interface
├── contract/     # Smart contract VM & compiler
├── core/         # Blockchain primitives (blocks, transactions, chain)
├── crypto/       # Cryptographic utilities
├── mining/       # Mining & mempool
├── multisig/     # Multi-signature wallets
├── network/      # P2P networking
├── storage/      # Persistence & indexing
├── token/        # ERC-20 style tokens
├── wallet/       # Wallet management
├── lib.rs        # Library exports
└── main.rs       # CLI entry point
```

---

## 🏛️ High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                           USER INTERFACES                            │
├──────────────────┬──────────────────┬──────────────────────────────────┤
│   CLI (main.rs)  │    REST API      │         WebSocket              │
│   clap commands  │   Axum handlers  │      Real-time events          │
└────────┬─────────┴────────┬─────────┴────────────┬───────────────────┘
         │                  │                      │
         ▼                  ▼                      ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         APPLICATION LAYER                            │
├──────────────┬──────────────┬──────────────┬──────────────────────────┤
│   Wallet     │    Token     │   Contract   │       Multisig         │
│   Manager    │   Manager    │   Manager    │       Manager          │
└──────┬───────┴──────┬───────┴──────┬───────┴──────────┬──────────────┘
       │              │              │                  │
       ▼              ▼              ▼                  ▼
┌─────────────────────────────────────────────────────────────────────┐
│                          CORE LAYER                                  │
├────────────────┬─────────────────┬──────────────────────────────────────┤
│   Blockchain   │   Transaction   │           Block                  │
│   (chain.rs)   │   (UTXO model)  │       (PoW mining)               │
└───────┬────────┴────────┬────────┴──────────────┬────────────────────┘
        │                 │                       │
        ▼                 ▼                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│                       INFRASTRUCTURE LAYER                           │
├────────────────┬─────────────────┬──────────────────────────────────────┤
│   Storage      │     Crypto      │          Network                 │
│  (persistence) │  (SHA-256/ECDSA)│        (P2P gossip)              │
└────────────────┴─────────────────┴──────────────────────────────────────┘
```

---

## 🧱 Core Layer

### Blockchain (`src/core/blockchain.rs`)

The central data structure managing the chain state.

```rust
pub struct Blockchain {
    pub blocks: Vec<Block>,           // Ordered chain of blocks
    pub difficulty: u32,              // Current mining difficulty
    pub utxo_set: HashMap<String, UTXO>, // Unspent transaction outputs
    pub chain_work: u128,             // Cumulative proof-of-work
    pub state: ChainStateManager,     // Fork handling & orphans
    pub coinbase_heights: HashMap<String, u64>, // Maturity tracking
}
```

**Key Features:**
- **UTXO Model**: Bitcoin-style unspent transaction outputs
- **Difficulty Adjustment**: Every 10 blocks, targets 100s block time
- **Fork Resolution**: Longest chain wins, orphan block handling
- **Coinbase Maturity**: 100-block delay for mining rewards

### Block (`src/core/block.rs`)

```rust
pub struct Block {
    pub index: u64,
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
    pub hash: String,
}

pub struct BlockHeader {
    pub previous_hash: String,
    pub merkle_root: String,
    pub timestamp: DateTime<Utc>,
    pub difficulty: u32,
    pub nonce: u64,
}
```

**Production Limits:**
| Constant | Value | Purpose |
|----------|-------|---------|
| `MAX_BLOCK_SIZE` | 1 MB | Max block bytes |
| `MAX_BLOCK_WEIGHT` | 4 MB | SegWit weight |
| `MAX_BLOCK_TXS` | 10,000 | Max transactions |

### Transaction (`src/core/transaction.rs`)

```rust
pub struct Transaction {
    pub version: u32,
    pub id: String,
    pub inputs: Vec<TransactionInput>,
    pub outputs: Vec<TransactionOutput>,
    pub timestamp: DateTime<Utc>,
    pub is_coinbase: bool,
    pub locktime: u32,           // BIP-65 time lock
    pub chain_id: u32,           // EIP-155 replay protection
    pub fee: u64,
    pub token_data: Option<TokenOperationType>,
    pub contract_data: Option<ContractOperationType>,
}
```

**Transaction Types:**
1. **Standard**: UTXO transfers
2. **Coinbase**: Mining rewards (100-block maturity)
3. **Token**: ERC-20 style operations (Create/Transfer/Approve/Burn/Mint)
4. **Contract**: Smart contract Deploy/Call

### Script System (`src/core/script.rs`)

Bitcoin-like output locking scripts:

```rust
pub enum ScriptType {
    P2PKH,                    // Pay to Public Key Hash
    P2SH { script_hash },     // Pay to Script Hash
    P2WPKH,                   // SegWit native
    MultiSig { threshold, pubkeys },
    TimeLock { locktime, inner },
    OpReturn { data },        // Data carrier (unspendable)
}

pub enum SigHashType {
    All,      // Sign all inputs/outputs
    None,     // Sign inputs only
    Single,   // Sign matching output
    AnyoneCanPay,  // Only sign own input
}
```

---

## ⛏️ Mining Layer

### Mempool (`src/mining/mempool.rs`)

Pending transaction pool with production limits:

```rust
// Bitcoin-style limits
MAX_ANCESTORS = 25       // Max parent chain
MAX_DESCENDANTS = 25     // Max child chain
MAX_MEMPOOL_BYTES = 300_000_000  // 300 MB
```

**Features:**
- **RBF Support**: Replace-By-Fee with 10% minimum bump
- **Locktime Validation**: BIP-65 compliance
- **Fee Ordering**: Highest fee-rate first for mining
- **Package Limits**: Ancestor/descendant tracking

### Miner (`src/mining/miner.rs`)

Proof-of-Work mining with SHA-256 double hashing:

```rust
pub fn mine_block(
    &self,
    blockchain: &mut Blockchain,
    transactions: Vec<Transaction>,
) -> Result<(Block, MiningStats), MiningError>
```

---

## 🔐 Crypto Layer

### Hashing (`src/crypto/hash.rs`)

```rust
pub fn sha256(data: &[u8]) -> String;
pub fn double_sha256(data: &[u8]) -> String;  // Bitcoin-style
pub fn ripemd160(data: &[u8]) -> Vec<u8>;
pub fn hash160(data: &[u8]) -> Vec<u8>;       // SHA256 + RIPEMD160
```

### Signatures (`src/crypto/signature.rs`)

ECDSA with secp256k1 curve:

```rust
pub fn sign(message: &str, private_key: &SecretKey) -> String;
pub fn verify(message: &str, signature: &str, public_key: &PublicKey) -> bool;
```

### Keys (`src/crypto/keys.rs`)

```rust
pub fn generate_keypair() -> (SecretKey, PublicKey);
pub fn public_key_to_address(public_key: &PublicKey) -> String;  // Base58Check
```

---

## 🌐 Network Layer

### P2P Protocol (`src/network/`)

**Production-grade P2P networking** with Bitcoin-inspired protocols:

```
┌──────────────────────────────────────────────────────────────────────────┐
│                           NETWORK NODE                                    │
├────────────┬────────────┬────────────┬────────────┬────────────────────────┤
│ PeerManager│  Server    │ ChainSync  │  AddrMan   │   Discovery          │
│ (scoring,  │  (TCP,     │ (parallel  │ (bucketed  │   (DNS seed,         │
│  eviction) │  checksum) │  download) │  storage)  │    addr exchange)    │
├────────────┴────────────┴────────────┴────────────┴────────────────────────┤
│        MessageCodec: 24-byte header with SHA-256 checksum                 │
│        UPnP NAT Traversal | Parallel Block Sync | Eclipse Resistance     │
└──────────────────────────────────────────────────────────────────────────┘
```

**Protocol Features:**
| Feature | Description |
|---------|-------------|
| **Message Checksums** | SHA-256 double-hash for integrity |
| **24-byte Header** | Magic (4) + Command (12) + Length (4) + Checksum (4) |
| **Peer Discovery** | DNS seeds + GetAddr/Addr exchange |
| **Address Manager** | Bitcoin-style new/tried tables with bucket hashing |
| **Connection Limits** | MAX_PEERS=125, MAX_OUTBOUND=8, MAX_INBOUND=117 |
| **Peer Scoring** | Reputation-based with ban scores |
| **Subnet Diversity** | Eclipse attack resistance |
| **NAT Traversal** | UPnP port mapping with auto-renewal |
| **Parallel Sync** | Download blocks from multiple peers |

**Message Types:**
- `Version` / `VerAck` - Protocol handshake
- `NewBlock` / `NewTransaction` - Block/tx gossip propagation
- `GetBlocks` / `Blocks` - Chain synchronization
- `GetAddr` / `Addr` - Peer discovery
- `Ping` / `Pong` - Connection keep-alive
- `Reject` - Error reporting with reason codes

**API + P2P Integration:**
Run API server with embedded P2P node:
```bash
cargo run -- api start --port 3000 --p2p-port 8333 --peers 127.0.0.1:8334
```
- Shared blockchain instance between API and P2P
- Blocks mined via API auto-broadcast to network
- Real-time sync with connected peers

---

## 💾 Storage Layer

### Persistence (`src/storage/`)

```
┌────────────────────────────────────────────────────────────┐
│                      STORAGE                               │
├──────────────┬─────────────┬─────────────┬────────────────────┤
│  Blockchain  │   Indexes   │  UTXO Cache │   Checkpoints    │
│  (JSON)      │  (hash/addr)│  (LRU 100K) │   (fast sync)    │
└──────────────┴─────────────┴─────────────┴────────────────────┘
```

**Features:**
- **Block Index**: O(1) lookup by hash/height
- **Transaction Index**: Find tx by ID
- **Address Index**: Get all transactions for address
- **UTXO Cache**: LRU cache (100K entries)
- **Pruning**: Configurable block retention

---

## 🌐 API Layer

### REST API (`src/api/`)

Built with Axum framework:

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/chain` | GET | Blockchain info |
| `/api/chain/blocks` | GET | List blocks |
| `/api/mine` | POST | Mine new block |
| `/api/wallets` | GET/POST | Manage wallets |
| `/api/wallets/{addr}/balance` | GET | Get balance (spendable + immature) |
| `/api/contracts` | GET/POST | List/deploy contracts |
| `/api/contracts/{addr}/call` | POST | Call contract |
| `/api/tokens` | GET/POST | ERC-20 tokens |
| `/api/multisig` | GET/POST | Multi-sig wallets |

### WebSocket (`/ws`)

Real-time events:
- `BlockMined { block, reward }`
- `TransactionAdded { tx }`
- `ChainReorg { old_height, new_height }`

---

## 🔄 Data Flow

### Transaction Lifecycle

```
1. Wallet creates transaction
   └── Signs with ECDSA private key
   
2. Transaction enters mempool
   └── Validates: UTXO exists, signature valid, fee sufficient
   └── Checks package limits (ancestors/descendants)
   
3. Miner selects transactions
   └── Ordered by fee rate (highest first)
   └── Checks locktime/sequence
   
4. Block is mined
   └── PoW: Find nonce where hash < target
   └── Includes coinbase (mining reward)
   
5. Block added to chain
   └── UTXO set updated (inputs spent, outputs created)
   └── Coinbase tracked for 100-block maturity
   
6. Block propagated via P2P
   └── NewBlock message to peers
```

### Block Validation Flow

```
receive_block(block)
├── validate_header()
│   ├── Check previous_hash links to chain
│   ├── Check timestamp > MTP (Median Time Past)
│   └── Check proof-of-work meets difficulty
├── validate_size()
│   ├── Block size ≤ 1 MB
│   └── Transaction count ≤ 10,000
├── validate_transactions()
│   ├── Each tx size ≤ 100 KB
│   ├── Verify signatures
│   └── Verify UTXO ownership
├── validate_merkle_root()
│   └── Recompute and compare
└── add_to_chain()
    ├── Update UTXO set
    ├── Adjust difficulty (every 10 blocks)
    └── Broadcast to peers
```

---

## 🔒 Security Features

| Feature | Protection |
|---------|------------|
| **Coinbase Maturity** | Prevents spending unconfirmed mining rewards |
| **Size Limits** | DOS protection (1MB blocks, 100KB txs) |
| **Package Limits** | Prevents mempool flooding via long chains |
| **Call Depth** | Prevents VM stack overflow (1024 depth) |
| **Reentrancy Detection** | Automatic protection for smart contracts |
| **Memory Gas** | Prevents memory exhaustion in VM |
| **Peer Scoring** | Bans misbehaving nodes |
| **Rate Limiting** | 1000 messages/minute per peer |

---

## 📊 Production Constants Summary

| Category | Constant | Value |
|----------|----------|-------|
| **Block** | `MAX_BLOCK_SIZE` | 1 MB |
| **Block** | `MAX_BLOCK_TXS` | 10,000 |
| **Transaction** | `MAX_TX_SIZE` | 100 KB |
| **Mining** | `COINBASE_MATURITY` | 100 blocks |
| **Mining** | `BLOCK_REWARD` | 50 coins |
| **Mempool** | `MAX_ANCESTORS` | 25 |
| **Mempool** | `MAX_DESCENDANTS` | 25 |
| **Mempool** | `MAX_MEMPOOL_BYTES` | 300 MB |
| **VM** | `MAX_CALL_DEPTH` | 1024 |
| **VM** | `DEFAULT_GAS_LIMIT` | 100,000 |
| **Network** | `MAX_MESSAGE_SIZE` | 16 MB |
| **Network** | `PROTOCOL_VERSION` | 70001 |
