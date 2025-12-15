//! ERC-20 style token implementation
//!
//! Provides a fungible token with standard interface.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Token-related errors
#[derive(Error, Debug)]
pub enum TokenError {
    #[error("Insufficient balance: have {have}, need {need}")]
    InsufficientBalance { have: u128, need: u128 },
    #[error("Insufficient allowance: have {have}, need {need}")]
    InsufficientAllowance { have: u128, need: u128 },
    #[error("Invalid amount: amount must be greater than 0")]
    InvalidAmount,
    #[error("Token not found: {0}")]
    TokenNotFound(String),
    #[error("Token already exists: {0}")]
    TokenAlreadyExists(String),
    #[error("Invalid address: cannot transfer to self")]
    SelfTransfer,
    #[error("Invalid symbol: must be 1-10 uppercase characters")]
    InvalidSymbol,
    #[error("Invalid name: must be 1-50 characters")]
    InvalidName,
    #[error("Invalid decimals: must be 0-18")]
    InvalidDecimals,
    #[error("Invalid supply: must be greater than 0")]
    InvalidSupply,
}

/// Token metadata (immutable after creation)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TokenMetadata {
    /// Token name (e.g., "My Token")
    pub name: String,
    /// Token symbol (e.g., "MTK")
    pub symbol: String,
    /// Decimal places (usually 18)
    pub decimals: u8,
    /// Total supply (fixed at creation)
    pub total_supply: u128,
    /// Creator address
    pub creator: String,
    /// Block number when created
    pub created_at_block: u64,
    /// Timestamp when created
    pub created_at: DateTime<Utc>,
}

impl TokenMetadata {
    /// Create new token metadata with validation
    pub fn new(
        name: String,
        symbol: String,
        decimals: u8,
        total_supply: u128,
        creator: String,
        block_number: u64,
    ) -> Result<Self, TokenError> {
        // Validate name
        if name.is_empty() || name.len() > 50 {
            return Err(TokenError::InvalidName);
        }

        // Validate symbol
        if symbol.is_empty() || symbol.len() > 10 {
            return Err(TokenError::InvalidSymbol);
        }

        // Validate decimals
        if decimals > 18 {
            return Err(TokenError::InvalidDecimals);
        }

        // Validate supply
        if total_supply == 0 {
            return Err(TokenError::InvalidSupply);
        }

        Ok(Self {
            name,
            symbol,
            decimals,
            total_supply,
            creator,
            created_at_block: block_number,
            created_at: Utc::now(),
        })
    }
}

/// Transfer event (emitted when tokens are transferred)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransferEvent {
    pub token: String,
    pub from: String,
    pub to: String,
    pub amount: u128,
    pub timestamp: DateTime<Utc>,
}

/// Approval event (emitted when allowance is set)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApprovalEvent {
    pub token: String,
    pub owner: String,
    pub spender: String,
    pub amount: u128,
    pub timestamp: DateTime<Utc>,
}

/// An ERC-20 style fungible token
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Token {
    /// Unique token address
    pub address: String,
    /// Token metadata
    pub metadata: TokenMetadata,
    /// Balances: address -> amount
    balances: HashMap<String, u128>,
    /// Allowances: owner -> (spender -> amount)
    allowances: HashMap<String, HashMap<String, u128>>,
    /// Transfer history (last 100)
    pub transfer_history: Vec<TransferEvent>,
}

impl Token {
    /// Create a new token with all supply allocated to creator
    pub fn new(address: String, metadata: TokenMetadata) -> Self {
        let mut balances = HashMap::new();
        // All tokens initially belong to creator
        balances.insert(metadata.creator.clone(), metadata.total_supply);

        Self {
            address,
            metadata,
            balances,
            allowances: HashMap::new(),
            transfer_history: Vec::new(),
        }
    }

    // =========================================================================
    // ERC-20 View Functions
    // =========================================================================

    /// Get token name
    pub fn name(&self) -> &str {
        &self.metadata.name
    }

    /// Get token symbol
    pub fn symbol(&self) -> &str {
        &self.metadata.symbol
    }

    /// Get decimal places
    pub fn decimals(&self) -> u8 {
        self.metadata.decimals
    }

    /// Get total supply
    pub fn total_supply(&self) -> u128 {
        self.metadata.total_supply
    }

    /// Get balance of an address
    pub fn balance_of(&self, address: &str) -> u128 {
        *self.balances.get(address).unwrap_or(&0)
    }

    /// Get allowance for a spender
    pub fn allowance(&self, owner: &str, spender: &str) -> u128 {
        self.allowances
            .get(owner)
            .and_then(|spenders| spenders.get(spender))
            .copied()
            .unwrap_or(0)
    }

    /// Get all holders with balances
    pub fn holders(&self) -> Vec<(&String, &u128)> {
        self.balances.iter().filter(|(_, &b)| b > 0).collect()
    }

    /// Get holder count
    pub fn holder_count(&self) -> usize {
        self.balances.values().filter(|&&b| b > 0).count()
    }

    // =========================================================================
    // ERC-20 Mutating Functions
    // =========================================================================

    /// Transfer tokens from one address to another
    ///
    /// # Arguments
    /// * `from` - Sender address
    /// * `to` - Recipient address
    /// * `amount` - Amount to transfer
    pub fn transfer(
        &mut self,
        from: &str,
        to: &str,
        amount: u128,
    ) -> Result<TransferEvent, TokenError> {
        if amount == 0 {
            return Err(TokenError::InvalidAmount);
        }

        if from == to {
            return Err(TokenError::SelfTransfer);
        }

        let from_balance = self.balance_of(from);
        if from_balance < amount {
            return Err(TokenError::InsufficientBalance {
                have: from_balance,
                need: amount,
            });
        }

        // Update balances
        *self.balances.entry(from.to_string()).or_insert(0) -= amount;
        *self.balances.entry(to.to_string()).or_insert(0) += amount;

        // Create event
        let event = TransferEvent {
            token: self.address.clone(),
            from: from.to_string(),
            to: to.to_string(),
            amount,
            timestamp: Utc::now(),
        };

        // Store event (keep last 100)
        self.transfer_history.push(event.clone());
        if self.transfer_history.len() > 100 {
            self.transfer_history.remove(0);
        }

        Ok(event)
    }

    /// Approve a spender to transfer tokens on behalf of owner
    ///
    /// # Arguments
    /// * `owner` - Token owner
    /// * `spender` - Address being approved to spend
    /// * `amount` - Maximum amount spender can transfer
    pub fn approve(
        &mut self,
        owner: &str,
        spender: &str,
        amount: u128,
    ) -> Result<ApprovalEvent, TokenError> {
        // Set allowance (can be 0 to revoke)
        self.allowances
            .entry(owner.to_string())
            .or_insert_with(HashMap::new)
            .insert(spender.to_string(), amount);

        Ok(ApprovalEvent {
            token: self.address.clone(),
            owner: owner.to_string(),
            spender: spender.to_string(),
            amount,
            timestamp: Utc::now(),
        })
    }

    /// Transfer tokens on behalf of owner (requires prior approval)
    ///
    /// # Arguments
    /// * `spender` - Address performing the transfer (must have allowance)
    /// * `from` - Token owner
    /// * `to` - Recipient
    /// * `amount` - Amount to transfer
    pub fn transfer_from(
        &mut self,
        spender: &str,
        from: &str,
        to: &str,
        amount: u128,
    ) -> Result<TransferEvent, TokenError> {
        if amount == 0 {
            return Err(TokenError::InvalidAmount);
        }

        if from == to {
            return Err(TokenError::SelfTransfer);
        }

        // Check allowance
        let current_allowance = self.allowance(from, spender);
        if current_allowance < amount {
            return Err(TokenError::InsufficientAllowance {
                have: current_allowance,
                need: amount,
            });
        }

        // Check balance
        let from_balance = self.balance_of(from);
        if from_balance < amount {
            return Err(TokenError::InsufficientBalance {
                have: from_balance,
                need: amount,
            });
        }

        // Update balances
        *self.balances.entry(from.to_string()).or_insert(0) -= amount;
        *self.balances.entry(to.to_string()).or_insert(0) += amount;

        // Reduce allowance
        if let Some(spenders) = self.allowances.get_mut(from) {
            if let Some(allowance) = spenders.get_mut(spender) {
                *allowance -= amount;
            }
        }

        // Create event
        let event = TransferEvent {
            token: self.address.clone(),
            from: from.to_string(),
            to: to.to_string(),
            amount,
            timestamp: Utc::now(),
        };

        self.transfer_history.push(event.clone());
        if self.transfer_history.len() > 100 {
            self.transfer_history.remove(0);
        }

        Ok(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_token() -> Token {
        let metadata = TokenMetadata::new(
            "Test Token".to_string(),
            "TST".to_string(),
            18,
            1_000_000,
            "creator".to_string(),
            1,
        )
        .unwrap();

        Token::new("0xTEST".to_string(), metadata)
    }

    #[test]
    fn test_token_creation() {
        let token = create_test_token();

        assert_eq!(token.name(), "Test Token");
        assert_eq!(token.symbol(), "TST");
        assert_eq!(token.decimals(), 18);
        assert_eq!(token.total_supply(), 1_000_000);
        assert_eq!(token.balance_of("creator"), 1_000_000);
        assert_eq!(token.holder_count(), 1);
    }

    #[test]
    fn test_metadata_validation() {
        // Invalid name (empty)
        assert!(TokenMetadata::new(
            "".to_string(),
            "TST".to_string(),
            18,
            1000,
            "c".to_string(),
            1
        )
        .is_err());

        // Invalid symbol (too long)
        assert!(TokenMetadata::new(
            "Test".to_string(),
            "TOOLONGSYMBOL".to_string(),
            18,
            1000,
            "c".to_string(),
            1
        )
        .is_err());

        // Invalid decimals
        assert!(TokenMetadata::new(
            "Test".to_string(),
            "TST".to_string(),
            19,
            1000,
            "c".to_string(),
            1
        )
        .is_err());

        // Invalid supply
        assert!(TokenMetadata::new(
            "Test".to_string(),
            "TST".to_string(),
            18,
            0,
            "c".to_string(),
            1
        )
        .is_err());
    }

    #[test]
    fn test_transfer() {
        let mut token = create_test_token();

        // Transfer from creator to recipient
        let event = token.transfer("creator", "recipient", 1000).unwrap();

        assert_eq!(event.from, "creator");
        assert_eq!(event.to, "recipient");
        assert_eq!(event.amount, 1000);
        assert_eq!(token.balance_of("creator"), 999_000);
        assert_eq!(token.balance_of("recipient"), 1000);
        assert_eq!(token.holder_count(), 2);
    }

    #[test]
    fn test_transfer_insufficient_balance() {
        let mut token = create_test_token();

        let result = token.transfer("creator", "recipient", 2_000_000);
        assert!(matches!(
            result,
            Err(TokenError::InsufficientBalance { .. })
        ));
    }

    #[test]
    fn test_transfer_zero_amount() {
        let mut token = create_test_token();

        let result = token.transfer("creator", "recipient", 0);
        assert!(matches!(result, Err(TokenError::InvalidAmount)));
    }

    #[test]
    fn test_self_transfer() {
        let mut token = create_test_token();

        let result = token.transfer("creator", "creator", 100);
        assert!(matches!(result, Err(TokenError::SelfTransfer)));
    }

    #[test]
    fn test_approve_and_allowance() {
        let mut token = create_test_token();

        // Initially no allowance
        assert_eq!(token.allowance("creator", "spender"), 0);

        // Approve
        token.approve("creator", "spender", 5000).unwrap();
        assert_eq!(token.allowance("creator", "spender"), 5000);

        // Update allowance
        token.approve("creator", "spender", 3000).unwrap();
        assert_eq!(token.allowance("creator", "spender"), 3000);

        // Revoke (set to 0)
        token.approve("creator", "spender", 0).unwrap();
        assert_eq!(token.allowance("creator", "spender"), 0);
    }

    #[test]
    fn test_transfer_from() {
        let mut token = create_test_token();

        // Approve spender
        token.approve("creator", "spender", 5000).unwrap();

        // Transfer from creator to recipient via spender
        let event = token
            .transfer_from("spender", "creator", "recipient", 1000)
            .unwrap();

        assert_eq!(event.amount, 1000);
        assert_eq!(token.balance_of("creator"), 999_000);
        assert_eq!(token.balance_of("recipient"), 1000);
        assert_eq!(token.allowance("creator", "spender"), 4000); // Reduced
    }

    #[test]
    fn test_transfer_from_insufficient_allowance() {
        let mut token = create_test_token();

        // Approve spender for 500
        token.approve("creator", "spender", 500).unwrap();

        // Try to transfer 1000
        let result = token.transfer_from("spender", "creator", "recipient", 1000);
        assert!(matches!(
            result,
            Err(TokenError::InsufficientAllowance { .. })
        ));
    }
}
