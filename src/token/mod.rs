//! ERC-20 style fungible token implementation
//!
//! Provides a standard interface for fungible tokens with:
//! - Balances per address
//! - Allowances for delegated transfers
//! - Transfer and approve operations
//!
//! # Example
//!
//! ```ignore
//! use mini_blockchain::token::{TokenManager, TokenMetadata};
//!
//! let mut manager = TokenManager::new();
//!
//! // Create a new token
//! let token = manager.create_token(
//!     "My Token".to_string(),
//!     "MTK".to_string(),
//!     18,
//!     1_000_000_000_000_000_000_000_000, // 1 million with 18 decimals
//!     "creator_address",
//!     1,
//! ).unwrap();
//!
//! // Transfer tokens
//! manager.transfer(&token.address, "creator_address", "recipient", 1000).unwrap();
//!
//! // Check balance
//! let balance = manager.get(&token.address).unwrap().balance_of("recipient");
//! ```

pub mod manager;
pub mod token;

pub use manager::TokenManager;
pub use token::{ApprovalEvent, Token, TokenError, TokenMetadata, TransferEvent};
