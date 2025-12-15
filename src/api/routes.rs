//! REST API routes configuration

use crate::api::handlers::{self, ApiState};
use crate::api::websocket::ws_handler;
use axum::{
    body::Body,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use rust_embed::Embed;
use tower_http::cors::{Any, CorsLayer};

/// Embedded static files from web-ui/build
#[derive(Embed)]
#[folder = "web-ui/build"]
struct Assets;

/// Serve root index.html
async fn index_handler() -> impl IntoResponse {
    serve_static("index.html")
}

/// Internal function to serve embedded files
fn serve_static(path: &str) -> Response {
    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(Body::from(content.data.into_owned()))
                .unwrap()
        }
        None => {
            // For SPA: serve index.html for unknown routes
            match Assets::get("index.html") {
                Some(content) => Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "text/html")
                    .body(Body::from(content.data.into_owned()))
                    .unwrap(),
                None => Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Body::from("Not Found"))
                    .unwrap(),
            }
        }
    }
}

/// Fallback handler for static files and SPA routing
async fn fallback_handler(uri: axum::http::Uri) -> impl IntoResponse {
    let path = uri.path();

    // Don't serve HTML for API routes - return 404 JSON instead
    if path.starts_with("/api/") {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"error":"Not Found"}"#))
            .unwrap();
    }

    let path = path.trim_start_matches('/');
    serve_static(if path.is_empty() { "index.html" } else { path })
}

/// Create the API router with all routes
pub fn create_router(state: ApiState) -> Router {
    // Configure CORS for browser access
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Health check
        .route("/health", get(handlers::health_check))
        // WebSocket for real-time updates
        .route("/ws", get(ws_handler))
        // Chain endpoints
        .route("/api/chain", get(handlers::get_chain_info))
        .route("/api/chain/blocks", get(handlers::get_blocks))
        .route(
            "/api/chain/blocks/{height}",
            get(handlers::get_block_by_height),
        )
        .route("/api/chain/validate", get(handlers::validate_chain))
        // Mining
        .route("/api/mine", post(handlers::mine_block))
        // Transactions
        .route("/api/transactions/{id}", get(handlers::get_transaction))
        .route("/api/mempool", get(handlers::get_mempool))
        // Wallets
        .route("/api/wallets", get(handlers::list_wallets))
        .route("/api/wallets", post(handlers::create_wallet))
        .route(
            "/api/wallets/{address}/balance",
            get(handlers::get_wallet_balance),
        )
        // Contracts
        .route("/api/contracts", get(handlers::list_contracts))
        .route("/api/contracts", post(handlers::deploy_contract))
        .route("/api/contracts/{address}", get(handlers::get_contract))
        .route(
            "/api/contracts/{address}/call",
            post(handlers::call_contract),
        )
        // Static files (Web UI)
        .route("/", get(index_handler))
        .fallback(fallback_handler)
        // Add state and middleware
        .with_state(state)
        .layer(cors)
}
