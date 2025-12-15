//! REST API module
//!
//! Provides HTTP REST API for programmatic access to the blockchain.
//!
//! # Endpoints
//!
//! ## Chain
//! - `GET /api/chain` - Blockchain info
//! - `GET /api/chain/blocks` - List recent blocks
//! - `GET /api/chain/blocks/:height` - Get block by height
//! - `GET /api/chain/validate` - Validate chain
//!
//! ## Mining
//! - `POST /api/mine` - Mine new block
//!
//! ## Transactions
//! - `GET /api/transactions/:id` - Get transaction
//! - `GET /api/mempool` - List pending transactions
//!
//! ## Wallets
//! - `GET /api/wallets` - List wallets
//! - `POST /api/wallets` - Create wallet
//! - `GET /api/wallets/:address/balance` - Get balance
//!
//! ## WebSocket
//! - `GET /ws` - Real-time updates (BlockMined, TransactionAdded, ChainUpdated)

pub mod handlers;
pub mod routes;
pub mod websocket;

pub use handlers::ApiState;
pub use routes::create_router;
pub use websocket::WsBroadcaster;
