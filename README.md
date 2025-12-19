<p align="center">
  <img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust"/>
  <img src="https://img.shields.io/badge/License-MIT-green.svg?style=for-the-badge" alt="License"/>
  <img src="https://img.shields.io/badge/Status-Production_Ready-blue?style=for-the-badge" alt="Status"/>
</p>

<h1 align="center">⛓️ Mini-Blockchain</h1>

<p align="center">
  <strong>A production-ready blockchain implementation in Rust</strong>
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#installation">Installation</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#cli-commands">CLI Commands</a> •
  <a href="#architecture">Architecture</a> •
  <a href="#api">API</a>
</p>

---

## ✨ Features

### Core Blockchain
| Feature | Description |
|---------|-------------|
| ⛏️ **Proof of Work** | SHA-256 mining with dynamic difficulty adjustment |
| 🔐 **ECDSA Signatures** | secp256k1 curve for secure transaction signing |
| 💰 **UTXO Model** | Bitcoin-style with locktime, RBF, and chain ID |
| 🌳 **Merkle Trees** | Efficient transaction verification and integrity |
| 🔀 **Fork Resolution** | Longest chain rule with orphan block handling |
| ⏱️ **MTP Validation** | Median Time Past for timestamp security |

### Wallets & Transactions
| Feature | Description |
|---------|-------------|
| 👛 **Wallet System** | Key generation with Base58Check addresses |
| ✍️ **Multi-Signature** | M-of-N threshold signatures for shared wallets |
| 🔄 **Replace-By-Fee** | BIP-125 transaction replacement |
| ⏳ **Locktime** | BIP-65 time-locked transactions |
| 🛡️ **Replay Protection** | EIP-155 style chain ID |

### Network & Security
| Feature | Description |
|---------|-------------|
| 🌐 **P2P Networking** | TCP with 24-byte header, SHA-256 checksums |
| 🔗 **API + P2P Integration** | Run API with embedded P2P node for block broadcasting |
| 📡 **Block Propagation** | Real-time gossip with relay to all peers |
| 🔍 **Peer Discovery** | DNS seeds + GetAddr/Addr exchange |
| 🗂️ **Address Manager** | Bitcoin-style new/tried tables |
| 🚫 **Peer Scoring** | Misbehavior detection and banning |
| 🛑 **Rate Limiting** | DOS protection (1000 msg/min) |
| 🌍 **NAT Traversal** | UPnP port mapping with auto-renewal |
| ⚡ **Parallel Sync** | Download blocks from multiple peers |
| 🛡️ **Eclipse Resistance** | Subnet diversity for connection selection |

### Storage & Performance
| Feature | Description |
|---------|-------------|
| 📇 **Block Indexing** | Fast lookups by hash, height, address |
| 💾 **UTXO Cache** | LRU cache with 100K entries |
| ✅ **Checkpoints** | Fast sync for known-good blocks |
| 🗑️ **Pruning** | Configurable block retention |

### Advanced Features
| Feature | Description |
|---------|-------------|
| 🔍 **SPV Support** | Bloom filters (BIP-37) for light clients |
| 💵 **Fee Estimation** | Smart fee calculation (high/normal/low) |
| 🗜️ **Compression** | Delta encoding for efficient storage |
| 🪙 **On-Chain Tokens** | ERC-20 style fungible tokens via transaction opcodes |
| ⛽ **Gas System** | Real gas payments for contracts |
| 📜 **On-Chain Smart Contracts** | Stack-based VM with bytecode, deployed via transactions |
| 🌐 **Web UI** | SvelteKit + shadcn-svelte dashboard |
| 🚀 **REST API** | HTTP API with Axum |
| 🔌 **WebSocket** | Real-time updates |

### Production-Grade Security (Bitcoin/Ethereum Inspired)
| Feature | Value | Description |
|---------|-------|-------------|
| 🔒 **Coinbase Maturity** | 100 blocks | Mining rewards can't be spent until 100 confirmations |
| 📦 **Block Size Limit** | 1 MB | Maximum block size (Bitcoin-style) |
| 📄 **Transaction Size** | 100 KB | Maximum transaction size |
| 📑 **Script System** | P2PKH, P2SH, MultiSig | Bitcoin-like output locking scripts |
| 🔐 **Signature Hash Types** | ALL, NONE, SINGLE | SIGHASH types for flexible signing |
| 👨‍👩‍👧 **Package Limits** | 25/25 | Max ancestors/descendants per tx chain |
| 💾 **Mempool Size** | 300 MB | Maximum mempool memory usage |
| 🔁 **Call Depth Limit** | 1024 | Prevents stack overflow attacks (EVM-style) |
| 🧠 **Memory Gas Metering** | Linear | Gas charged for VM memory expansion |
| 🛡️ **Reentrancy Protection** | Automatic | Prevents contract reentrancy attacks |

---

## 📦 Installation


### Prerequisites

- Rust 1.70+ (install via [rustup](https://rustup.rs/))

### Build from Source

```bash
# Clone the repository
git clone https://github.com/yourusername/mini-blockchain.git
cd mini-blockchain

# Build release version
cargo build --release

# The binary will be at ./target/release/blockchain
```

---

## 🚀 Quick Start

```bash
# 1. Initialize a new blockchain
./target/release/blockchain init

# 2. Create a wallet
./target/release/blockchain wallet new --label "MyWallet"

# 3. Start mining (replace ADDRESS with your wallet address)
./target/release/blockchain mine --address <ADDRESS> --count 5

# 4. Check your balance
./target/release/blockchain wallet balance --address <ADDRESS>

# 5. View blockchain info
./target/release/blockchain chain
```

### Example Output

```
⛏️  Mining 5 block(s) for address: 1FUdbKMd9nfCC6gGmczBXniNDuYw81Zyyg

   Block 1 mined!
   ├─ Hash: 00fcfbb82a7dfe25
   ├─ Transactions: 1
   ├─ Time: 0ms
   ├─ Attempts: 577
   └─ Hash rate: 577.00 H/s

💰 New balance for miner: 250 coins
```

---

## 🖥️ CLI Commands

### Blockchain Management

| Command | Description |
|---------|-------------|
| `init` | Initialize a new blockchain |
| `chain` | Display blockchain information |
| `chain blocks --count N` | Show last N blocks |
| `validate` | Verify chain integrity |
| `export --output FILE` | Export blockchain to JSON |
| `import --input FILE` | Import blockchain from JSON |

### Wallet Operations

| Command | Description |
|---------|-------------|
| `wallet new` | Create a new wallet |
| `wallet new --label NAME` | Create wallet with label |
| `wallet list` | List all wallets |
| `wallet balance --address ADDR` | Check wallet balance |

### Mining & Transactions

| Command | Description |
|---------|-------------|
| `mine --address ADDR` | Mine a single block |
| `mine --address ADDR --count N` | Mine N blocks |
| `send --from ADDR --to ADDR --amount N` | Send coins |
| `mempool` | Show pending transactions |

### Examples

```bash
# Initialize with custom difficulty (higher = harder)
blockchain init --difficulty 16

# Mine 10 blocks
blockchain mine --address 1ABC123... --count 10

# Send 25 coins
blockchain send --from 1ABC123... --to 1XYZ789... --amount 25

# Export blockchain backup
blockchain export --output backup.json
```

### P2P Networking

| Command | Description |
|---------|-------------|
| `node start` | Start P2P node on default port (8333) |
| `node start --port PORT` | Start node on custom port |
| `node start --peers HOST:PORT` | Start and connect to peers |
| `node status` | Show node connection info |

```bash
# Terminal 1: Start first standalone P2P node
RUST_LOG=info blockchain node start --port 8334

# Terminal 2: Start second node and connect
RUST_LOG=info blockchain node start --port 8335 --peers 127.0.0.1:8334
```

### REST API

| Command | Description |
|---------|-------------|
| `api start` | Start REST API on default port (3000) |
| `api start --port PORT` | Start on custom port |
| `api start --p2p-port PORT` | **NEW:** Enable embedded P2P node |
| `api start --peers HOST:PORT` | Connect to P2P network |

```bash
# API-only mode (no P2P)
blockchain api start --port 3000

# 🆕 API + P2P mode (blocks mined via UI broadcast to network!)
RUST_LOG=info blockchain api start --port 3000 --p2p-port 8333 --peers 127.0.0.1:8334

# Test endpoints with curl
curl http://localhost:3000/api/chain
curl http://localhost:3000/api/wallets/1ABC.../balance
curl -X POST http://localhost:3000/api/mine -H "Content-Type: application/json" -d '{"miner_address": "1ABC..."}'
```

> **💡 Tip:** With `--p2p-port`, blocks mined via the Web UI are automatically broadcast to all connected P2P nodes in real-time!

### Web UI

The REST API server includes an embedded Web UI built with SvelteKit + shadcn-svelte.

| Page | Features |
|------|----------|
| Dashboard | Chain stats, recent blocks, **real-time updates** |
| Blocks | Block explorer with details |
| **Search** | **Global search across blocks, transactions, addresses, tokens** |
| Wallets | Create/list wallets, view balances |
| Mining | Mine blocks with reward display |
| Contracts | Deploy, list, and call contracts |
| Multisig | Create M-of-N wallets, view pending transactions |
| Tokens | Create ERC-20 tokens, transfer, check balances |
| Mempool | View pending transactions |

```bash
# Start the server and open the Web UI
blockchain api start --port 3000
# Visit http://localhost:3000
```

### WebSocket

Connect to `/ws` for real-time updates. Events are JSON with the following types:

| Event | Description |
|-------|-------------|
| `Connected` | Connection established |
| `BlockMined` | New block mined (includes block info and reward) |
| `TransactionAdded` | Transaction added to mempool |
| `ChainUpdated` | Chain state changed |

```javascript
// JavaScript example
const ws = new WebSocket('ws://localhost:3000/ws');
ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  console.log(data.type, data.data);
};
```

### Smart Contracts

See [CONTRACT_EXAMPLES.md](CONTRACT_EXAMPLES.md) for detailed examples and assembly code.

| Command | Description |
|---------|-------------|
| `contract deploy --file FILE` | Deploy contract from .asm file |
| `contract call --address ADDR --args ARGS` | Call contract |
| `contract list` | List all deployed contracts |
| `contract info --address ADDR` | Show contract details |

```bash
# Create a simple add contract (examples/add.asm):
# ARG 0
# ARG 1  
# ADD
# RETURN

# Deploy it
blockchain contract deploy --file examples/add.asm

# Call it with args
blockchain contract call --address 0x... --args "10,25"
# Returns: 35
```

### Multi-Signature Wallets

Create wallets requiring M-of-N signatures to spend funds:

```bash
# Create a 2-of-3 multisig wallet via API
curl -X POST http://localhost:3000/api/multisig \
  -H "Content-Type: application/json" \
  -d '{
    "threshold": 2,
    "signers": ["<pubkey1>", "<pubkey2>", "<pubkey3>"],
    "label": "Team Treasury"
  }'

# Response includes the multisig address (starts with '3')
# {"address": "3ABC...", "threshold": 2, "signer_count": 3, ...}

# Propose a transaction
curl -X POST http://localhost:3000/api/multisig/3ABC.../propose \
  -H "Content-Type: application/json" \
  -d '{"to": "1RECIPIENT...", "amount": 100}'

# Sign with each authorized wallet (need M signatures)
curl -X POST http://localhost:3000/api/multisig/3ABC.../sign \
  -H "Content-Type: application/json" \
  -d '{"tx_id": "TX_ID", "signer_pubkey": "<pubkey1>", "signature": "<sig1>"}'
```

### Tokens (ERC-20 Style)

Create and manage fungible tokens with a standard ERC-20 interface:

| API Endpoint | Description |
|--------------|-------------|
| `POST /api/tokens` | Create new token |
| `GET /api/tokens` | List all tokens |
| `GET /api/tokens/{addr}` | Get token info |
| `GET /api/tokens/{addr}/balance/{holder}` | Get balance |
| `POST /api/tokens/{addr}/transfer` | Transfer tokens |
| `POST /api/tokens/{addr}/approve` | Approve spender |
| `GET /api/tokens/{addr}/allowance` | Check allowance |
| `POST /api/tokens/{addr}/transferFrom` | Delegated transfer |

```bash
# Create a new token
curl -X POST http://localhost:3000/api/tokens \
  -H "Content-Type: application/json" \
  -d '{
    "name": "My Token",
    "symbol": "MTK",
    "decimals": 18,
    "total_supply": "1000000",
    "creator": "1ABC..."
  }'

# Check token balance
curl http://localhost:3000/api/tokens/0xTOKEN.../balance/1ABC...

# Transfer tokens
curl -X POST http://localhost:3000/api/tokens/0xTOKEN.../transfer \
  -H "Content-Type: application/json" \
  -d '{"from": "1ABC...", "to": "1DEF...", "amount": "1000"}'

# Approve a spender
curl -X POST http://localhost:3000/api/tokens/0xTOKEN.../approve \
  -H "Content-Type: application/json" \
  -d '{"owner": "1ABC...", "spender": "1DEF...", "amount": "5000"}'
```

**Token vs Coins:**
| Asset | Description |
|-------|-------------|
| **Coins** | Native blockchain currency (from mining) |
| **Tokens** | Custom assets created via `/api/tokens` |

---

## 🏗️ Architecture

```
src/
├── main.rs              # CLI entry point
├── lib.rs               # Library exports
│
├── core/                # 🧱 Core Blockchain
│   ├── block.rs         # Block structure & PoW mining
│   ├── blockchain.rs    # Chain management & validation
│   └── transaction.rs   # UTXO transactions, signatures & on-chain ops
│
├── crypto/              # 🔐 Cryptography
│   ├── hash.rs          # SHA-256 hashing utilities
│   ├── keys.rs          # ECDSA key management
│   └── merkle.rs        # Merkle tree implementation
│
├── wallet/              # 👛 Wallet
│   └── wallet.rs        # Key storage & tx creation
│
├── mining/              # ⛏️ Mining
│   ├── miner.rs         # Block mining engine
│   └── mempool.rs       # Transaction pool
│
├── network/             # 🌐 P2P Networking
│   ├── message.rs       # Protocol messages
│   ├── node.rs          # Main P2P node
│   ├── peer.rs          # Peer management
│   ├── server.rs        # TCP server & codec
│   └── sync.rs          # Chain synchronization
│
├── api/                 # 🚀 REST API
│   ├── handlers.rs      # Endpoint handlers
│   └── routes.rs        # Route configuration
│
├── contract/            # 📜 Smart Contracts
│   ├── opcodes.rs       # VM instruction set
│   ├── vm.rs            # Stack-based VM
│   ├── contract.rs      # Contract management
│   └── compiler.rs      # Assembly compiler
│
├── storage/             # 💾 Storage
│   └── persistence.rs   # Save/load blockchain
│
└── cli/                 # 🖥️ CLI
    └── commands.rs      # Command handlers
```

---

## 📚 API

### Library Usage

```rust
use mini_blockchain::{Blockchain, Wallet, Miner, BLOCK_REWARD};

fn main() {
    // Create a blockchain with custom difficulty
    let mut blockchain = Blockchain::with_difficulty(8);
    
    // Create wallets
    let alice = Wallet::new();
    let bob = Wallet::new();
    
    println!("Alice: {}", alice.address());
    println!("Bob: {}", bob.address());
    
    // Mine blocks to earn coins
    let miner = Miner::new(&alice.address());
    for _ in 0..3 {
        let (block, stats) = miner.mine_block(&mut blockchain, vec![]).unwrap();
        println!("Mined block {} in {}ms", block.index, stats.time_ms);
    }
    
    // Check balance
    println!("Alice's balance: {} coins", alice.balance(&blockchain));
    
    // Create a transaction
    let tx = alice.create_transaction(&bob.address(), 50, &blockchain).unwrap();
    println!("Transaction: {}", tx.id);
    
    // Validate chain
    assert!(blockchain.is_valid());
}
```

---

## ⚙️ Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `difficulty` | 16 | Mining difficulty (leading zero bits) |
| `block_reward` | 50 | Coins per mined block |
| `target_block_time` | 10s | Target time between blocks |
| `difficulty_adjustment` | 10 blocks | Blocks between difficulty changes |

---

## 🧪 Testing

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific module tests
cargo test crypto::
cargo test core::blockchain::
```

**Test Coverage:**
- ✅ 52 unit tests
- ✅ Block creation & mining
- ✅ Chain validation
- ✅ Transaction signing
- ✅ UTXO tracking
- ✅ Wallet operations
- ✅ Storage persistence
- ✅ Network message codec
- ✅ P2P node creation
- ✅ Smart contract VM
- ✅ Contract deployment & execution

---

## 📊 Performance

Benchmarks on Intel i7 (single-threaded):

| Difficulty | Avg. Attempts | Avg. Time |
|------------|---------------|-----------|
| 8 bits | ~256 | < 1ms |
| 16 bits | ~65,536 | ~50ms |
| 20 bits | ~1M | ~500ms |
| 24 bits | ~16M | ~8s |

---

## 🛣️ Roadmap

### ✅ Completed

- [x] Core blockchain with PoW
- [x] ECDSA transaction signing  
- [x] UTXO model
- [x] Wallet management
- [x] CLI interface
- [x] Persistence layer
- [x] P2P networking
- [x] REST API
- [x] Smart contracts (VM, compiler, storage)
- [x] Web UI (SvelteKit + shadcn-svelte)
- [x] Contract deployment via Web UI
- [x] WebSocket for real-time updates
- [x] Multi-signature transactions
- [x] Token standards (ERC-20 style)
- [x] Block explorer search

### 🔮 Future Ideas

- [ ] Mobile-responsive UI
- [ ] Docker deployment

---

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

<p align="center">
  Made with ❤️ in Rust
</p>
