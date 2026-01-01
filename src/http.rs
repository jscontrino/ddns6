use axum::{routing::get, Router};
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::cloudflare::CloudflareClient;
use crate::config::Config;
use crate::dyndns2::{handle_update, AppState};
use crate::error::Result;
use crate::state::StateCache;

pub async fn create_server(config: Arc<Config>) -> Result<Router> {
    let cloudflare_client = Arc::new(CloudflareClient::new(
        config.cloudflare.api_token.clone(),
        config.cloudflare.zone_id.clone(),
        config.cloudflare.ttl,
    ));

    let state = AppState {
        config: config.clone(),
        state_cache: StateCache::new(),
        cloudflare_client,
    };

    let app = Router::new()
        .route("/update", get(handle_update))
        .route("/", get(health_check))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    info!("HTTP server configured");

    Ok(app)
}

async fn health_check() -> &'static str {
    "OK"
}
