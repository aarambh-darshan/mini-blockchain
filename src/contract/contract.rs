//! Smart contract management
//!
//! Handles contract deployment, storage, and invocation.

use crate::contract::vm::{ExecutionContext, ExecutionResult, VmError, DEFAULT_GAS_LIMIT, VM};
use crate::crypto::hash::sha256;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Contract errors
#[derive(Error, Debug)]
pub enum ContractError {
    #[error("Contract not found: {0}")]
    NotFound(String),
    #[error("Contract already exists: {0}")]
    AlreadyExists(String),
    #[error("VM error: {0}")]
    VmError(#[from] VmError),
    #[error("Invalid bytecode")]
    InvalidBytecode,
    #[error("Deployment failed: {0}")]
    DeploymentFailed(String),
}

/// A deployed smart contract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contract {
    /// Contract address (derived from deployer + nonce)
    pub address: String,
    /// Contract bytecode
    pub code: Vec<u8>,
    /// Contract storage (key-value pairs)
    pub storage: HashMap<String, u64>,
    /// Deployer address
    pub deployer: String,
    /// Block number when deployed
    pub deployed_at: u64,
}

impl Contract {
    /// Create a new contract
    pub fn new(address: String, code: Vec<u8>, deployer: String, block_number: u64) -> Self {
        Self {
            address,
            code,
            storage: HashMap::new(),
            deployer,
            deployed_at: block_number,
        }
    }

    /// Execute the contract
    pub fn execute(
        &mut self,
        caller: &str,
        args: Vec<u64>,
        timestamp: u64,
        block_number: u64,
        gas_limit: Option<u64>,
    ) -> Result<ExecutionResult, VmError> {
        let context = ExecutionContext {
            caller: caller.to_string(),
            contract_address: self.address.clone(),
            timestamp,
            block_number,
            args,
            gas_limit: gas_limit.unwrap_or(DEFAULT_GAS_LIMIT),
        };

        let mut vm = VM::new(self.code.clone(), self.storage.clone(), context);
        let result = vm.execute()?;

        // Apply storage changes
        for (key, value) in &result.storage_changes {
            self.storage.insert(key.clone(), *value);
        }

        Ok(result)
    }
}

/// Manages all deployed contracts
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContractManager {
    /// All deployed contracts
    contracts: HashMap<String, Contract>,
    /// Deployment counter for address generation
    nonce: u64,
}

impl ContractManager {
    /// Create a new contract manager
    pub fn new() -> Self {
        Self {
            contracts: HashMap::new(),
            nonce: 0,
        }
    }

    /// Deploy a new contract
    pub fn deploy(
        &mut self,
        code: Vec<u8>,
        deployer: &str,
        block_number: u64,
    ) -> Result<String, ContractError> {
        if code.is_empty() {
            return Err(ContractError::InvalidBytecode);
        }

        // Generate contract address
        let address = self.generate_address(deployer);
        self.nonce += 1;

        if self.contracts.contains_key(&address) {
            return Err(ContractError::AlreadyExists(address));
        }

        let contract = Contract::new(address.clone(), code, deployer.to_string(), block_number);
        self.contracts.insert(address.clone(), contract);

        log::info!("Contract deployed at {}", address);
        Ok(address)
    }

    /// Call a contract
    pub fn call(
        &mut self,
        address: &str,
        caller: &str,
        args: Vec<u64>,
        timestamp: u64,
        block_number: u64,
        gas_limit: Option<u64>,
    ) -> Result<ExecutionResult, ContractError> {
        let contract = self
            .contracts
            .get_mut(address)
            .ok_or_else(|| ContractError::NotFound(address.to_string()))?;

        let result = contract.execute(caller, args, timestamp, block_number, gas_limit)?;
        Ok(result)
    }

    /// Get a contract by address
    pub fn get(&self, address: &str) -> Option<&Contract> {
        self.contracts.get(address)
    }

    /// Get all contract addresses
    pub fn list(&self) -> Vec<String> {
        self.contracts.keys().cloned().collect()
    }

    /// Get contract count
    pub fn count(&self) -> usize {
        self.contracts.len()
    }

    /// Generate contract address from deployer and nonce
    fn generate_address(&self, deployer: &str) -> String {
        let input = format!("{}:{}", deployer, self.nonce);
        let hash = sha256(input.as_bytes());
        // Convert bytes to hex string and take first 40 chars
        let hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
        format!("0x{}", &hex[..40])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::opcodes::OpCode;

    fn make_push(value: u64) -> Vec<u8> {
        let mut bytes = vec![OpCode::Push as u8];
        bytes.extend_from_slice(&value.to_be_bytes());
        bytes
    }

    #[test]
    fn test_contract_deployment() {
        let mut manager = ContractManager::new();

        // Simple return 42 contract
        let mut code = make_push(42);
        code.push(OpCode::Return as u8);

        let address = manager.deploy(code, "deployer123", 1).unwrap();
        assert!(address.starts_with("0x"));
        assert_eq!(manager.count(), 1);
    }

    #[test]
    fn test_contract_call() {
        let mut manager = ContractManager::new();

        // Return first argument + 10
        let mut code = Vec::new();
        code.push(OpCode::Arg as u8);
        code.push(0); // arg index 0
        code.extend(make_push(10));
        code.push(OpCode::Add as u8);
        code.push(OpCode::Return as u8);

        let address = manager.deploy(code, "deployer", 1).unwrap();

        let result = manager
            .call(&address, "caller", vec![5], 12345, 1, None)
            .unwrap();

        assert!(result.success);
        assert_eq!(result.return_value, Some(15)); // 5 + 10
    }

    #[test]
    fn test_contract_storage_persistence() {
        let mut manager = ContractManager::new();

        // Store value at key 1
        let mut code = make_push(1); // key
        code.extend(make_push(100)); // value
        code.push(OpCode::SStore as u8);
        code.push(OpCode::Halt as u8);

        let address = manager.deploy(code.clone(), "deployer", 1).unwrap();

        // First call: store value
        manager
            .call(&address, "caller", vec![], 0, 1, None)
            .unwrap();

        // Check storage was persisted
        let contract = manager.get(&address).unwrap();
        assert!(contract.storage.values().any(|&v| v == 100));
    }
}
