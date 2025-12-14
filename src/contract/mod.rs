//! Smart contract module
//!
//! Provides a simple smart contract system with a stack-based virtual machine.
//!
//! # Overview
//!
//! This module implements:
//! - A stack-based VM for executing contract bytecode
//! - Contract deployment and invocation
//! - A simple assembly-like compiler
//! - Gas metering to prevent infinite loops
//!
//! # Example
//!
//! ```rust
//! use mini_blockchain::contract::{Compiler, ContractManager};
//!
//! // Compile a simple contract
//! let mut compiler = Compiler::new();
//! let bytecode = compiler.compile("
//!     PUSH 42
//!     RETURN
//! ").unwrap();
//!
//! // Deploy the contract
//! let mut manager = ContractManager::new();
//! let address = manager.deploy(bytecode, "deployer_address", 1).unwrap();
//!
//! // Call the contract
//! let result = manager.call(&address, "caller", vec![], 0, 1, None).unwrap();
//! assert_eq!(result.return_value, Some(42));
//! ```

pub mod compiler;
pub mod contract;
pub mod opcodes;
pub mod vm;

pub use compiler::{disassemble, Compiler, CompilerError};
pub use contract::{Contract, ContractError, ContractManager};
pub use opcodes::OpCode;
pub use vm::{ExecutionContext, ExecutionResult, VmError, DEFAULT_GAS_LIMIT, VM};
