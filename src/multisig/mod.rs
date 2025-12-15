//! Multi-signature wallet and transaction support
//!
//! Provides M-of-N threshold signature wallets where M signatures
//! from N authorized signers are required to spend funds.
//!
//! # Example
//!
//! ```ignore
//! use mini_blockchain::multisig::{MultisigWallet, MultisigConfig, MultisigManager};
//!
//! // Create a 2-of-3 multisig wallet
//! let config = MultisigConfig::new(2, vec![pubkey1, pubkey2, pubkey3], None)?;
//! let wallet = MultisigWallet::new(config)?;
//!
//! // Propose a transaction
//! let pending = manager.propose_transaction(&wallet.address, recipient, amount, &blockchain)?;
//!
//! // Collect signatures
//! manager.sign_transaction(&pending.id, signature1)?;
//! manager.sign_transaction(&pending.id, signature2)?;
//!
//! // Transaction is now ready to broadcast
//! ```

pub mod manager;
pub mod transaction;
pub mod wallet;

pub use manager::MultisigManager;
pub use transaction::{MultisigSignature, PendingMultisigTx, PendingStatus};
pub use wallet::{MultisigConfig, MultisigError, MultisigWallet};
