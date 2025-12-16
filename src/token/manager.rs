//! Token manager for creating and managing tokens
//!
//! Handles token deployment and provides high-level operations.

use crate::crypto::sha256;
use crate::token::token::{ApprovalEvent, Token, TokenError, TokenMetadata, TransferEvent};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Manages all tokens in the system
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TokenManager {
    /// All tokens by address
    tokens: HashMap<String, Token>,
    /// Deployment counter for address generation
    nonce: u64,
}

impl TokenManager {
    /// Create a new token manager
    pub fn new() -> Self {
        Self {
            tokens: HashMap::new(),
            nonce: 0,
        }
    }

    /// Create a new token
    ///
    /// All tokens are initially allocated to the creator.
    pub fn create_token(
        &mut self,
        name: String,
        symbol: String,
        decimals: u8,
        total_supply: u128,
        creator: &str,
        block_number: u64,
    ) -> Result<Token, TokenError> {
        // Create metadata (validates inputs)
        let metadata = TokenMetadata::new(
            name,
            symbol,
            decimals,
            total_supply,
            creator.to_string(),
            block_number,
        )?;

        // Generate unique address
        let address = self.generate_address(creator, &metadata.symbol);
        self.nonce += 1;

        if self.tokens.contains_key(&address) {
            return Err(TokenError::TokenAlreadyExists(address));
        }

        // Create token
        let token = Token::new(address.clone(), metadata);
        self.tokens.insert(address.clone(), token.clone());

        log::info!(
            "Token created: {} ({}) at {}",
            token.name(),
            token.symbol(),
            address
        );

        Ok(token)
    }

    /// Generate token address from creator and symbol
    fn generate_address(&self, creator: &str, symbol: &str) -> String {
        let input = format!("{}:{}:{}", creator, symbol, self.nonce);
        let hash = sha256(input.as_bytes());
        let hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
        format!("0x{}", &hex[..40])
    }

    /// Get a token by address
    pub fn get(&self, address: &str) -> Option<&Token> {
        self.tokens.get(address)
    }

    /// Get mutable reference to a token
    pub fn get_mut(&mut self, address: &str) -> Option<&mut Token> {
        self.tokens.get_mut(address)
    }

    /// List all tokens
    pub fn list(&self) -> Vec<&Token> {
        self.tokens.values().collect()
    }

    /// Get token count
    pub fn count(&self) -> usize {
        self.tokens.len()
    }

    /// Check if a token exists
    pub fn exists(&self, address: &str) -> bool {
        self.tokens.contains_key(address)
    }

    /// Transfer tokens
    pub fn transfer(
        &mut self,
        token_address: &str,
        from: &str,
        to: &str,
        amount: u128,
    ) -> Result<TransferEvent, TokenError> {
        let token = self
            .tokens
            .get_mut(token_address)
            .ok_or_else(|| TokenError::TokenNotFound(token_address.to_string()))?;

        token.transfer(from, to, amount)
    }

    /// Approve spender
    pub fn approve(
        &mut self,
        token_address: &str,
        owner: &str,
        spender: &str,
        amount: u128,
    ) -> Result<ApprovalEvent, TokenError> {
        let token = self
            .tokens
            .get_mut(token_address)
            .ok_or_else(|| TokenError::TokenNotFound(token_address.to_string()))?;

        token.approve(owner, spender, amount)
    }

    /// Transfer from (delegated transfer)
    pub fn transfer_from(
        &mut self,
        token_address: &str,
        spender: &str,
        from: &str,
        to: &str,
        amount: u128,
    ) -> Result<TransferEvent, TokenError> {
        let token = self
            .tokens
            .get_mut(token_address)
            .ok_or_else(|| TokenError::TokenNotFound(token_address.to_string()))?;

        token.transfer_from(spender, from, to, amount)
    }

    /// Get balance for an address across a specific token
    pub fn balance_of(&self, token_address: &str, holder: &str) -> Result<u128, TokenError> {
        let token = self
            .tokens
            .get(token_address)
            .ok_or_else(|| TokenError::TokenNotFound(token_address.to_string()))?;

        Ok(token.balance_of(holder))
    }

    /// Get allowance
    pub fn allowance(
        &self,
        token_address: &str,
        owner: &str,
        spender: &str,
    ) -> Result<u128, TokenError> {
        let token = self
            .tokens
            .get(token_address)
            .ok_or_else(|| TokenError::TokenNotFound(token_address.to_string()))?;

        Ok(token.allowance(owner, spender))
    }

    /// Get all tokens held by an address
    pub fn tokens_for_holder(&self, holder: &str) -> Vec<(&Token, u128)> {
        self.tokens
            .values()
            .filter_map(|token| {
                let balance = token.balance_of(holder);
                if balance > 0 {
                    Some((token, balance))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Burn tokens
    pub fn burn(
        &mut self,
        token_address: &str,
        from: &str,
        amount: u128,
    ) -> Result<crate::token::token::BurnEvent, TokenError> {
        let token = self
            .tokens
            .get_mut(token_address)
            .ok_or_else(|| TokenError::TokenNotFound(token_address.to_string()))?;

        token.burn(from, amount)
    }

    /// Mint new tokens
    pub fn mint(
        &mut self,
        token_address: &str,
        caller: &str,
        to: &str,
        amount: u128,
    ) -> Result<crate::token::token::MintEvent, TokenError> {
        let token = self
            .tokens
            .get_mut(token_address)
            .ok_or_else(|| TokenError::TokenNotFound(token_address.to_string()))?;

        token.mint(caller, to, amount)
    }

    /// Get transfer history for a token
    pub fn get_history(&self, token_address: &str) -> Result<Vec<TransferEvent>, TokenError> {
        let token = self
            .tokens
            .get(token_address)
            .ok_or_else(|| TokenError::TokenNotFound(token_address.to_string()))?;

        Ok(token.transfer_history.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager_creation() {
        let manager = TokenManager::new();
        assert_eq!(manager.count(), 0);
    }

    #[test]
    fn test_token_creation() {
        let mut manager = TokenManager::new();

        let token = manager
            .create_token(
                "Test Token".to_string(),
                "TST".to_string(),
                18,
                1_000_000,
                "creator",
                1,
            )
            .unwrap();

        assert!(token.address.starts_with("0x"));
        assert_eq!(token.balance_of("creator"), 1_000_000);
        assert_eq!(manager.count(), 1);
    }

    #[test]
    fn test_transfer_via_manager() {
        let mut manager = TokenManager::new();

        let token = manager
            .create_token(
                "Test Token".to_string(),
                "TST".to_string(),
                18,
                1_000_000,
                "creator",
                1,
            )
            .unwrap();

        let address = token.address.clone();

        manager
            .transfer(&address, "creator", "recipient", 1000)
            .unwrap();

        assert_eq!(manager.balance_of(&address, "creator").unwrap(), 999_000);
        assert_eq!(manager.balance_of(&address, "recipient").unwrap(), 1000);
    }

    #[test]
    fn test_approve_and_transfer_from() {
        let mut manager = TokenManager::new();

        let token = manager
            .create_token(
                "Test Token".to_string(),
                "TST".to_string(),
                18,
                1_000_000,
                "creator",
                1,
            )
            .unwrap();

        let address = token.address.clone();

        // Approve spender
        manager
            .approve(&address, "creator", "spender", 5000)
            .unwrap();
        assert_eq!(
            manager.allowance(&address, "creator", "spender").unwrap(),
            5000
        );

        // Transfer from
        manager
            .transfer_from(&address, "spender", "creator", "recipient", 1000)
            .unwrap();

        assert_eq!(manager.balance_of(&address, "creator").unwrap(), 999_000);
        assert_eq!(manager.balance_of(&address, "recipient").unwrap(), 1000);
        assert_eq!(
            manager.allowance(&address, "creator", "spender").unwrap(),
            4000
        );
    }

    #[test]
    fn test_tokens_for_holder() {
        let mut manager = TokenManager::new();

        // Create two tokens
        let token1 = manager
            .create_token(
                "Token1".to_string(),
                "TK1".to_string(),
                18,
                1000,
                "alice",
                1,
            )
            .unwrap();

        let _token2 = manager
            .create_token(
                "Token2".to_string(),
                "TK2".to_string(),
                18,
                2000,
                "alice",
                1,
            )
            .unwrap();

        // Alice should have both tokens
        let alice_tokens = manager.tokens_for_holder("alice");
        assert_eq!(alice_tokens.len(), 2);

        // Bob should have none
        let bob_tokens = manager.tokens_for_holder("bob");
        assert_eq!(bob_tokens.len(), 0);

        // Transfer some to Bob
        manager
            .transfer(&token1.address, "alice", "bob", 500)
            .unwrap();

        let bob_tokens = manager.tokens_for_holder("bob");
        assert_eq!(bob_tokens.len(), 1);
        assert_eq!(bob_tokens[0].1, 500);
    }

    #[test]
    fn test_transfer_nonexistent_token() {
        let mut manager = TokenManager::new();

        let result = manager.transfer("0xNONEXISTENT", "from", "to", 100);
        assert!(matches!(result, Err(TokenError::TokenNotFound(_))));
    }
}
