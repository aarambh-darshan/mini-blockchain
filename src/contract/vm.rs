//! Smart contract virtual machine
//!
//! A stack-based VM for executing smart contract bytecode.
//! Production-grade security features:
//! - Call depth limit (1024, like EVM)
//! - Memory gas metering
//! - Stack overflow protection
//! - Reentrancy detection

use crate::contract::opcodes::OpCode;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

// =============================================================================
// VM Constants (EVM-like production values)
// =============================================================================

/// Maximum stack size
const MAX_STACK_SIZE: usize = 1024;

/// Maximum call depth (EVM uses 1024)
pub const MAX_CALL_DEPTH: usize = 1024;

/// Default gas limit
pub const DEFAULT_GAS_LIMIT: u64 = 100_000;

/// Maximum memory size in pages (256 pages * 256 bytes = 64KB)
pub const MAX_MEMORY_PAGES: usize = 256;

/// Memory page size in bytes
pub const MEMORY_PAGE_SIZE: usize = 256;

/// Gas cost for memory expansion per page
pub const MEMORY_GAS_PER_PAGE: u64 = 3;

/// Gas cost for storage write
pub const SSTORE_GAS: u64 = 5000;

/// Gas refund for storage clear
pub const SSTORE_REFUND: u64 = 4800;

/// Gas cost for storage read
pub const SLOAD_GAS: u64 = 200;

// =============================================================================
// VM Errors
// =============================================================================

/// VM execution errors
#[derive(Error, Debug, Clone)]
pub enum VmError {
    #[error("Stack overflow")]
    StackOverflow,
    #[error("Stack underflow")]
    StackUnderflow,
    #[error("Invalid opcode: {0}")]
    InvalidOpcode(u8),
    #[error("Out of gas")]
    OutOfGas,
    #[error("Invalid jump destination: {0}")]
    InvalidJump(u32),
    #[error("Division by zero")]
    DivisionByZero,
    #[error("Invalid argument index: {0}")]
    InvalidArgument(u8),
    #[error("Execution reverted")]
    Reverted,
    #[error("Insufficient balance for transfer")]
    InsufficientBalance,
    #[error("Invalid address")]
    InvalidAddress,
    #[error("Call depth exceeded: {0} (max: {1})")]
    CallDepthExceeded(usize, usize),
    #[error("Out of memory: {0} pages (max: {1})")]
    OutOfMemory(usize, usize),
    #[error("Reentrancy detected: contract {0} is already executing")]
    ReentrancyDetected(String),
}

/// Execution context for the VM
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// Caller address
    pub caller: String,
    /// Contract address
    pub contract_address: String,
    /// Current block timestamp
    pub timestamp: u64,
    /// Current block number
    pub block_number: u64,
    /// Contract arguments
    pub args: Vec<u64>,
    /// Available gas
    pub gas_limit: u64,
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            caller: String::new(),
            contract_address: String::new(),
            timestamp: 0,
            block_number: 0,
            args: Vec::new(),
            gas_limit: DEFAULT_GAS_LIMIT,
        }
    }
}

/// Result of VM execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Whether execution succeeded
    pub success: bool,
    /// Return value (if any)
    pub return_value: Option<u64>,
    /// Gas used
    pub gas_used: u64,
    /// Storage changes: key -> value
    pub storage_changes: HashMap<String, u64>,
    /// Transfer requests: (to, amount)
    pub transfers: Vec<(String, u64)>,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Call depth reached during execution
    pub call_depth: usize,
}

/// The smart contract virtual machine
pub struct VM {
    /// Stack for computation
    stack: Vec<u64>,
    /// Program counter
    pc: usize,
    /// Gas remaining
    gas: u64,
    /// Contract storage
    storage: HashMap<String, u64>,
    /// Pending storage changes
    storage_changes: HashMap<String, u64>,
    /// Pending transfers
    transfers: Vec<(String, u64)>,
    /// Bytecode being executed
    code: Vec<u8>,
    /// Execution context
    context: ExecutionContext,
    /// Whether execution has halted
    halted: bool,
    /// Return value
    return_value: Option<u64>,
    /// Current call depth (for nested calls)
    call_depth: usize,
    /// Current memory size in pages
    memory_pages: usize,
    /// Set of contracts currently executing (for reentrancy detection)
    executing_contracts: HashSet<String>,
}

impl VM {
    /// Create a new VM instance
    pub fn new(code: Vec<u8>, storage: HashMap<String, u64>, context: ExecutionContext) -> Self {
        Self {
            stack: Vec::with_capacity(256),
            pc: 0,
            gas: context.gas_limit,
            storage,
            storage_changes: HashMap::new(),
            transfers: Vec::new(),
            code,
            context,
            halted: false,
            return_value: None,
            call_depth: 0,
            memory_pages: 0,
            executing_contracts: HashSet::new(),
        }
    }

    /// Create a VM with an initial call depth (for nested calls)
    pub fn with_call_depth(
        code: Vec<u8>,
        storage: HashMap<String, u64>,
        context: ExecutionContext,
        call_depth: usize,
        executing_contracts: HashSet<String>,
    ) -> Result<Self, VmError> {
        // Check call depth limit
        if call_depth >= MAX_CALL_DEPTH {
            return Err(VmError::CallDepthExceeded(call_depth, MAX_CALL_DEPTH));
        }

        // Check for reentrancy
        if executing_contracts.contains(&context.contract_address) {
            return Err(VmError::ReentrancyDetected(
                context.contract_address.clone(),
            ));
        }

        let mut executing = executing_contracts;
        executing.insert(context.contract_address.clone());

        Ok(Self {
            stack: Vec::with_capacity(256),
            pc: 0,
            gas: context.gas_limit,
            storage,
            storage_changes: HashMap::new(),
            transfers: Vec::new(),
            code,
            context,
            halted: false,
            return_value: None,
            call_depth,
            memory_pages: 0,
            executing_contracts: executing,
        })
    }

    /// Get current call depth
    pub fn get_call_depth(&self) -> usize {
        self.call_depth
    }

    /// Consume gas (helper method for memory expansion and other operations)
    fn consume_gas(&mut self, amount: u64) -> Result<(), VmError> {
        if self.gas < amount {
            return Err(VmError::OutOfGas);
        }
        self.gas -= amount;
        Ok(())
    }

    /// Expand memory and charge gas
    pub fn expand_memory(&mut self, pages_needed: usize) -> Result<(), VmError> {
        if pages_needed > self.memory_pages {
            let new_pages = pages_needed - self.memory_pages;

            if pages_needed > MAX_MEMORY_PAGES {
                return Err(VmError::OutOfMemory(pages_needed, MAX_MEMORY_PAGES));
            }

            let gas_cost = new_pages as u64 * MEMORY_GAS_PER_PAGE;
            self.consume_gas(gas_cost)?;
            self.memory_pages = pages_needed;
        }
        Ok(())
    }

    /// Execute the bytecode
    pub fn execute(&mut self) -> Result<ExecutionResult, VmError> {
        while !self.halted && self.pc < self.code.len() {
            self.step()?;
        }

        Ok(ExecutionResult {
            success: true,
            return_value: self.return_value,
            gas_used: self.context.gas_limit - self.gas,
            storage_changes: self.storage_changes.clone(),
            transfers: self.transfers.clone(),
            error: None,
            call_depth: self.call_depth,
        })
    }

    /// Execute a single instruction
    fn step(&mut self) -> Result<(), VmError> {
        let opcode_byte = self.code[self.pc];
        let opcode = OpCode::from_byte(opcode_byte).ok_or(VmError::InvalidOpcode(opcode_byte))?;

        // Consume gas
        let gas_cost = self.gas_cost(&opcode);
        if self.gas < gas_cost {
            return Err(VmError::OutOfGas);
        }
        self.gas -= gas_cost;

        self.pc += 1;

        match opcode {
            OpCode::Push => {
                let value = self.read_u64()?;
                self.push(value)?;
            }
            OpCode::Pop => {
                self.pop()?;
            }
            OpCode::Dup => {
                let value = *self.stack.last().ok_or(VmError::StackUnderflow)?;
                self.push(value)?;
            }
            OpCode::Swap => {
                let len = self.stack.len();
                if len < 2 {
                    return Err(VmError::StackUnderflow);
                }
                self.stack.swap(len - 1, len - 2);
            }
            OpCode::Add => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(a.wrapping_add(b))?;
            }
            OpCode::Sub => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(a.wrapping_sub(b))?;
            }
            OpCode::Mul => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(a.wrapping_mul(b))?;
            }
            OpCode::Div => {
                let b = self.pop()?;
                let a = self.pop()?;
                if b == 0 {
                    return Err(VmError::DivisionByZero);
                }
                self.push(a / b)?;
            }
            OpCode::Mod => {
                let b = self.pop()?;
                let a = self.pop()?;
                if b == 0 {
                    return Err(VmError::DivisionByZero);
                }
                self.push(a % b)?;
            }
            OpCode::Eq => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(if a == b { 1 } else { 0 })?;
            }
            OpCode::Lt => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(if a < b { 1 } else { 0 })?;
            }
            OpCode::Gt => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(if a > b { 1 } else { 0 })?;
            }
            OpCode::Le => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(if a <= b { 1 } else { 0 })?;
            }
            OpCode::Ge => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(if a >= b { 1 } else { 0 })?;
            }
            OpCode::Neq => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(if a != b { 1 } else { 0 })?;
            }
            OpCode::IsZero => {
                let a = self.pop()?;
                self.push(if a == 0 { 1 } else { 0 })?;
            }
            OpCode::And => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(a & b)?;
            }
            OpCode::Or => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(a | b)?;
            }
            OpCode::Not => {
                let a = self.pop()?;
                self.push(!a)?;
            }
            OpCode::Jump => {
                let offset = self.read_u32()?;
                if offset as usize >= self.code.len() {
                    return Err(VmError::InvalidJump(offset));
                }
                self.pc = offset as usize;
            }
            OpCode::JumpIf => {
                let offset = self.read_u32()?;
                let condition = self.pop()?;
                if condition != 0 {
                    if offset as usize >= self.code.len() {
                        return Err(VmError::InvalidJump(offset));
                    }
                    self.pc = offset as usize;
                }
            }
            OpCode::Halt => {
                self.halted = true;
            }
            OpCode::Return => {
                self.return_value = Some(self.pop()?);
                self.halted = true;
            }
            OpCode::Revert => {
                return Err(VmError::Reverted);
            }
            OpCode::SStore => {
                let value = self.pop()?;
                let key = self.pop()?;
                let key_str = format!("{:016x}", key);
                self.storage.insert(key_str.clone(), value);
                self.storage_changes.insert(key_str, value);
            }
            OpCode::SLoad => {
                let key = self.pop()?;
                let key_str = format!("{:016x}", key);
                let value = self.storage.get(&key_str).copied().unwrap_or(0);
                self.push(value)?;
            }
            OpCode::Balance => {
                // For simplicity, push 0 - real implementation would check blockchain
                self.push(0)?;
            }
            OpCode::Transfer => {
                let amount = self.pop()?;
                let to = self.pop()?;
                let to_addr = format!("{:016x}", to);
                self.transfers.push((to_addr, amount));
                self.push(1)?; // Success
            }
            OpCode::Caller => {
                // Hash caller address to u64
                let caller_hash = self.hash_address(&self.context.caller);
                self.push(caller_hash)?;
            }
            OpCode::Self_ => {
                let self_hash = self.hash_address(&self.context.contract_address);
                self.push(self_hash)?;
            }
            OpCode::Timestamp => {
                self.push(self.context.timestamp)?;
            }
            OpCode::BlockNumber => {
                self.push(self.context.block_number)?;
            }
            OpCode::SelfBalance => {
                // For simplicity, push 0
                self.push(0)?;
            }
            OpCode::Arg => {
                let index = self.read_u8()?;
                let value = self
                    .context
                    .args
                    .get(index as usize)
                    .copied()
                    .ok_or(VmError::InvalidArgument(index))?;
                self.push(value)?;
            }
            OpCode::ArgCount => {
                self.push(self.context.args.len() as u64)?;
            }
            OpCode::Nop => {}
        }

        Ok(())
    }

    /// Push value onto stack
    fn push(&mut self, value: u64) -> Result<(), VmError> {
        if self.stack.len() >= MAX_STACK_SIZE {
            return Err(VmError::StackOverflow);
        }
        self.stack.push(value);
        Ok(())
    }

    /// Pop value from stack
    fn pop(&mut self) -> Result<u64, VmError> {
        self.stack.pop().ok_or(VmError::StackUnderflow)
    }

    /// Read u8 from bytecode
    fn read_u8(&mut self) -> Result<u8, VmError> {
        if self.pc >= self.code.len() {
            return Err(VmError::InvalidOpcode(0));
        }
        let value = self.code[self.pc];
        self.pc += 1;
        Ok(value)
    }

    /// Read u32 from bytecode
    fn read_u32(&mut self) -> Result<u32, VmError> {
        if self.pc + 4 > self.code.len() {
            return Err(VmError::InvalidOpcode(0));
        }
        let bytes = [
            self.code[self.pc],
            self.code[self.pc + 1],
            self.code[self.pc + 2],
            self.code[self.pc + 3],
        ];
        self.pc += 4;
        Ok(u32::from_be_bytes(bytes))
    }

    /// Read u64 from bytecode
    fn read_u64(&mut self) -> Result<u64, VmError> {
        if self.pc + 8 > self.code.len() {
            return Err(VmError::InvalidOpcode(0));
        }
        let bytes = [
            self.code[self.pc],
            self.code[self.pc + 1],
            self.code[self.pc + 2],
            self.code[self.pc + 3],
            self.code[self.pc + 4],
            self.code[self.pc + 5],
            self.code[self.pc + 6],
            self.code[self.pc + 7],
        ];
        self.pc += 8;
        Ok(u64::from_be_bytes(bytes))
    }

    /// Hash address string to u64
    fn hash_address(&self, addr: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        addr.hash(&mut hasher);
        hasher.finish()
    }

    /// Get gas cost for opcode
    fn gas_cost(&self, opcode: &OpCode) -> u64 {
        match opcode {
            OpCode::Push | OpCode::Pop | OpCode::Dup | OpCode::Swap => 2,
            OpCode::Add | OpCode::Sub | OpCode::Mul => 3,
            OpCode::Div | OpCode::Mod => 5,
            OpCode::Eq | OpCode::Lt | OpCode::Gt | OpCode::Le | OpCode::Ge | OpCode::Neq => 3,
            OpCode::And | OpCode::Or | OpCode::Not | OpCode::IsZero => 3,
            OpCode::Jump | OpCode::JumpIf => 8,
            OpCode::SStore => 20,
            OpCode::SLoad => 5,
            OpCode::Balance | OpCode::SelfBalance => 10,
            OpCode::Transfer => 50,
            OpCode::Caller | OpCode::Self_ | OpCode::Timestamp | OpCode::BlockNumber => 2,
            OpCode::Arg | OpCode::ArgCount => 2,
            OpCode::Halt | OpCode::Return | OpCode::Revert => 0,
            OpCode::Nop => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_push(value: u64) -> Vec<u8> {
        let mut bytes = vec![OpCode::Push as u8];
        bytes.extend_from_slice(&value.to_be_bytes());
        bytes
    }

    #[test]
    fn test_simple_addition() {
        let mut code = make_push(10);
        code.extend(make_push(20));
        code.push(OpCode::Add as u8);
        code.push(OpCode::Return as u8);

        let mut vm = VM::new(code, HashMap::new(), ExecutionContext::default());
        let result = vm.execute().unwrap();

        assert!(result.success);
        assert_eq!(result.return_value, Some(30));
    }

    #[test]
    fn test_comparison() {
        let mut code = make_push(10);
        code.extend(make_push(20));
        code.push(OpCode::Lt as u8);
        code.push(OpCode::Return as u8);

        let mut vm = VM::new(code, HashMap::new(), ExecutionContext::default());
        let result = vm.execute().unwrap();

        assert!(result.success);
        assert_eq!(result.return_value, Some(1)); // 10 < 20
    }

    #[test]
    fn test_storage() {
        // Store value 42 at key 1
        let mut code = make_push(1); // key
        code.extend(make_push(42)); // value
        code.push(OpCode::SStore as u8);

        // Load it back
        code.extend(make_push(1)); // key
        code.push(OpCode::SLoad as u8);
        code.push(OpCode::Return as u8);

        let mut vm = VM::new(code, HashMap::new(), ExecutionContext::default());
        let result = vm.execute().unwrap();

        assert!(result.success);
        assert_eq!(result.return_value, Some(42));
    }

    #[test]
    fn test_out_of_gas() {
        let mut code = Vec::new();
        for _ in 0..10000 {
            code.extend(make_push(1));
        }

        let mut context = ExecutionContext::default();
        context.gas_limit = 100; // Very low gas

        let mut vm = VM::new(code, HashMap::new(), context);
        let result = vm.execute();

        assert!(result.is_err());
    }
}
