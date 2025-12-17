# 📜 Smart Contracts Guide

Complete guide to deploying and interacting with smart contracts on mini-blockchain.

---

## 🏛️ Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                    SMART CONTRACT SYSTEM                             │
├──────────────────┬──────────────────┬──────────────────────────────────┤
│     Compiler     │       VM         │        Manager               │
│  (Assembly→Byte) │  (Stack Machine) │     (Deploy/Call)            │
└──────────────────┴──────────────────┴──────────────────────────────────┘
```

| Component | File | Purpose |
|-----------|------|---------|
| **Compiler** | `src/contract/compiler.rs` | Assembles source to bytecode |
| **VM** | `src/contract/vm.rs` | Executes bytecode |
| **Opcodes** | `src/contract/opcodes.rs` | Instruction set |
| **Manager** | `src/contract/contract.rs` | Contract storage & execution |

---

## 🔤 Instruction Set

### Stack Operations (0x00-0x0F)

| Opcode | Name | Description | Gas |
|--------|------|-------------|-----|
| `0x00` | `PUSH` | Push 64-bit value | 3 |
| `0x01` | `POP` | Remove top value | 2 |
| `0x02` | `DUP` | Duplicate top | 3 |
| `0x03` | `SWAP` | Swap top two | 3 |

### Arithmetic (0x10-0x1F)

| Opcode | Name | Description | Gas |
|--------|------|-------------|-----|
| `0x10` | `ADD` | a + b | 3 |
| `0x11` | `SUB` | a - b | 3 |
| `0x12` | `MUL` | a × b | 5 |
| `0x13` | `DIV` | a ÷ b | 5 |
| `0x14` | `MOD` | a % b | 5 |

### Comparison (0x20-0x2F)

| Opcode | Name | Description | Gas |
|--------|------|-------------|-----|
| `0x20` | `EQ` | a == b → 1 or 0 | 3 |
| `0x21` | `LT` | a < b | 3 |
| `0x22` | `GT` | a > b | 3 |
| `0x23` | `LE` | a ≤ b | 3 |
| `0x24` | `GE` | a ≥ b | 3 |
| `0x25` | `NEQ` | a ≠ b | 3 |
| `0x26` | `ISZERO` | a == 0 | 3 |

### Logic (0x30-0x3F)

| Opcode | Name | Description | Gas |
|--------|------|-------------|-----|
| `0x30` | `AND` | a && b | 3 |
| `0x31` | `OR` | a \|\| b | 3 |
| `0x32` | `NOT` | !a | 3 |

### Control Flow (0x40-0x4F)

| Opcode | Name | Description | Gas |
|--------|------|-------------|-----|
| `0x40` | `JUMP` | Unconditional jump | 8 |
| `0x41` | `JUMPI` | Jump if top ≠ 0 | 10 |
| `0x42` | `HALT` | Stop execution | 0 |
| `0x43` | `RETURN` | Return with value | 0 |
| `0x44` | `REVERT` | Revert all changes | 0 |

### Storage (0x50-0x5F)

| Opcode | Name | Description | Gas |
|--------|------|-------------|-----|
| `0x50` | `SSTORE` | key, value → storage | 5000 |
| `0x51` | `SLOAD` | key → value | 200 |

### Context (0x60-0x6F)

| Opcode | Name | Description | Gas |
|--------|------|-------------|-----|
| `0x60` | `BALANCE` | Get address balance | 400 |
| `0x61` | `TRANSFER` | to, amount → success | 2300 |
| `0x62` | `CALLER` | Push caller address | 2 |
| `0x63` | `SELF` | Push contract address | 2 |
| `0x64` | `TIMESTAMP` | Current block time | 2 |
| `0x65` | `BLOCKNUMBER` | Current height | 2 |
| `0x66` | `SELFBALANCE` | Contract's balance | 5 |

### Arguments (0x70-0x7F)

| Opcode | Name | Description | Gas |
|--------|------|-------------|-----|
| `0x70` | `ARG` | Load argument[index] | 3 |
| `0x71` | `ARGCOUNT` | Number of arguments | 2 |

---

## 🔒 Security Features

| Feature | Value | Description |
|---------|-------|-------------|
| **Call Depth Limit** | 1024 | Max nested calls (prevents stack overflow) |
| **Memory Pages** | 256 max | 64KB maximum memory |
| **Memory Gas** | 3/page | Gas charged per memory page expansion |
| **Reentrancy Protection** | Automatic | Tracks executing contracts to prevent reentrancy |
| **Stack Size** | 1024 | Maximum stack depth |
| **Default Gas Limit** | 100,000 | Per-call limit |

---

## 📝 Writing Contracts

### Syntax

```asm
; Comment
OPCODE           ; Instruction
PUSH 42          ; Push literal value
:label           ; Define jump target
JUMP label       ; Jump to label
JUMPI label      ; Jump if condition is true
```

### Example 1: Simple Addition

```asm
; Add two numbers
ARG 0            ; Load first argument
ARG 1            ; Load second argument
ADD              ; Stack: [a+b]
RETURN           ; Return result
```

**Call:** `args: [10, 32]` → Returns `42`

### Example 2: Persistent Counter

```asm
; Load current count from storage[0]
PUSH 0           ; Key
SLOAD            ; Stack: [count]

; Increment
PUSH 1
ADD              ; Stack: [count+1]

; Save back
DUP              ; Stack: [count+1, count+1]
PUSH 0           ; Stack: [count+1, count+1, 0]
SWAP             ; Stack: [count+1, 0, count+1]
SSTORE           ; Store: storage[0] = count+1

; Return new count
RETURN
```

**Call 1:** Returns `1`  
**Call 2:** Returns `2`  
**Call 3:** Returns `3`

### Example 3: Max Function

```asm
ARG 0            ; a
ARG 1            ; b
DUP              ; [a, b, b]
ARG 0            ; [a, b, b, a]
LT               ; [a, b, (a < b)]
JUMPI return_b   ; if true, go return b

; return a
ARG 0
RETURN

:return_b
ARG 1
RETURN
```

### Example 4: Access Control

```asm
; Only allow specific caller
CALLER           ; Push caller address
PUSH 12345       ; Expected owner (simplified)
EQ               ; Is caller == owner?
JUMPI authorized

; Unauthorized - revert
REVERT

:authorized
; ... contract logic ...
PUSH 1
RETURN
```

---

## 🚀 API Usage

### Deploy Contract

```bash
POST /api/contracts
Content-Type: application/json

{
  "source": "ARG 0\nARG 1\nADD\nRETURN"
}
```

**Response:**
```json
{
  "address": "0x04c4b5c2c1d096de63803f759a49d9657663b33d",
  "code_size": 12
}
```

### Call Contract

```bash
POST /api/contracts/{address}/call
Content-Type: application/json

{
  "args": [10, 32],
  "gas_limit": 10000,
  "gas_price": 1,
  "caller_address": "your-wallet-address"
}
```

**Response:**
```json
{
  "success": true,
  "return_value": 42,
  "gas_used": 18,
  "gas_cost": 18,
  "caller_balance": 982
}
```

### List Contracts

```bash
GET /api/contracts
```

**Response:**
```json
[
  {
    "address": "0x04c4b5c2...",
    "deployer": "web-deployer",
    "deployed_at": 21,
    "code_size": 12
  }
]
```

---

## ⛽ Gas Economics

### Gas Costs by Operation

| Operation | Gas Cost | Notes |
|-----------|----------|-------|
| Simple ops | 2-5 | PUSH, POP, arithmetic |
| Jumps | 8-10 | JUMP, JUMPI |
| Storage read | 200 | SLOAD |
| Storage write | 5000 | SSTORE |
| Transfer | 2300 | Sending coins |
| Memory expansion | 3/page | Per 256-byte page |

### Gas Payment

When calling with `caller_address` and `gas_price > 0`:
1. Gas is calculated: `gas_used × gas_price`
2. Coins are deducted from caller's UTXO balance
3. A burn transaction is mined to record the payment

---

## 🔄 On-Chain Recording

Contract operations are recorded as transactions:

```rust
pub enum ContractOperationType {
    Deploy {
        bytecode: Vec<u8>,
        constructor_args: Vec<u64>,
    },
    Call {
        contract_address: String,
        args: Vec<u64>,
        gas_limit: Option<u64>,
    },
}
```

This means:
- All deployments are permanently recorded on-chain
- All calls are permanently recorded on-chain
- Contract state changes are verifiable

---

## 🧪 Testing Contracts

### Via CLI

```bash
# Deploy
./blockchain contract deploy --source "ARG 0\nARG 1\nADD\nRETURN"

# Call
./blockchain contract call --address 0x... --args 10,32
```

### Via Web UI

1. Go to **Contracts** page
2. Paste source code
3. Click **Deploy**
4. Select deployed contract
5. Enter arguments
6. Click **Call**

---

## 📚 Advanced Patterns

### 1. Escrow Contract

```asm
; Check if unlock time has passed
TIMESTAMP        ; Current time
PUSH 1700000000  ; Unlock timestamp
GE               ; time >= unlock?
JUMPI release

; Not ready yet
PUSH 0
RETURN

:release
SELFBALANCE      ; Get contract balance
CALLER           ; Get who called
SWAP             ; [caller, balance]
TRANSFER         ; Send all to caller
RETURN
```

### 2. Simple Voting

```asm
; Get vote option (0 or 1)
ARG 0
DUP

; Validate: option must be 0 or 1
PUSH 2
LT
JUMPI valid

REVERT

:valid
; Load current count for option
DUP
SLOAD

; Increment
PUSH 1
ADD

; Save back
SWAP
SWAP
SSTORE

PUSH 1
RETURN
```

### 3. Token Balance Check

```asm
; Get balance of address
ARG 0            ; Address to check
SLOAD            ; Load balance from storage[address]
RETURN
```

---

## 🔧 Contract Manager

The `ContractManager` handles:

```rust
pub struct ContractManager {
    contracts: HashMap<String, Contract>,
}

impl ContractManager {
    pub fn deploy(&mut self, bytecode, deployer, height) -> Result<address>;
    pub fn call(&mut self, address, caller, args, time, height, gas) -> Result<ExecutionResult>;
    pub fn get(&self, address) -> Option<&Contract>;
    pub fn list(&self) -> Vec<&String>;
}
```

---

## 🎯 Best Practices

1. **Always test locally first** - Use low gas limits for testing
2. **Check return values** - `REVERT` for errors, `RETURN` for success
3. **Minimize storage operations** - `SSTORE` is expensive (5000 gas)
4. **Use comments** - Document your assembly code
5. **Validate inputs** - Check argument bounds, addresses
6. **Consider reentrancy** - Though automatic protection exists
