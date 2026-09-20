//! HTTP surface over the policy engine and the decision log.
//!
//! This is not `hearsay-proxy`. The proxy in `design.md` §8 wraps an upstream
//! VLM and carries the whole pipeline; this serves the parts that exist —
//! the policy engine, the ruleset, and a decision log — so the dashboard has
//! something real to talk to. The `/v1/chat/completions` surface arrives with
//! the proxy.
//!
//! ```text
//! GET  /healthz                 liveness
//! GET  /readyz                  readiness, plus what is and is not durable
//! GET  /metrics                 Prometheus text
//! GET  /api/ruleset             active thresholds, lexicon, and the 7 rules
//! GET  /api/decisions           recent decisions (?since_ms=&limit=)
//! GET  /api/decisions/{id}      one decision
//! GET  /api/stats               dashboard summary
//! POST /api/evaluate            run the engine over supplied regions
//! ```

#![forbid(unsafe_code)]

mod normalize;
mod pipeline;
mod record;
mod routes;
mod seed;
mod state;

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use hearsay_policy::Ruleset;
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::state::AppState;

/// Request bodies above this are rejected before parsing.
const MAX_BODY_BYTES: usize = 512 * 1024;

fn env_var<T: std::str::FromStr>(key: &str, fallback: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(fallback)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("hearsay_api=info,tower_http=warn")),
        )
        .init();

    let ruleset = Ruleset::default();
    tracing::warn!(
        version = %ruleset.version,
        "ruleset thresholds are placeholders, not calibrated; no number from this build is reportable"
    );

    let state = Arc::new(AppState::new(ruleset)?);
    seed::populate(&state);
    tracing::info!(seeded = state.len(), "boot fixtures loaded");

    // Permissive CORS: this binds to localhost and serves a local dashboard.
    // A deployment behind a real origin should replace `Any` with that
    // origin — see the note in the README.
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = routes::router(Arc::clone(&state))
        .layer(TraceLayer::new_for_http())
        .layer(RequestBodyLimitLayer::new(MAX_BODY_BYTES))
        .layer(cors);

    let host: IpAddr = env_var("HEARSAY_HOST", IpAddr::from([127, 0, 0, 1]));
    let port: u16 = env_var("HEARSAY_PORT", 8787);
    let addr = SocketAddr::new(host, port);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("hearsay-api listening on http://{addr}");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    if tokio::signal::ctrl_c().await.is_ok() {
        tracing::info!("shutdown requested");
    }
}
