use clap::Parser;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::signal;
use tracing::{error, info};

mod cloudflare;
mod config;
mod dyndns2;
mod error;
mod http;
mod ipv6;
mod state;

use config::Config;
use error::Result;

#[derive(Parser, Debug)]
#[command(
    name = "ddns6",
    version,
    about = "IPv6 DynDNS daemon that combines dynamic prefixes with static Interface IDs"
)]
struct Args {
    #[arg(short, long, default_value = "config.toml")]
    config: String,
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        error!("Application error: {}", e);
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    info!("Starting ddns6 daemon");
    info!("Loading configuration from: {}", args.config);

    let config = Arc::new(Config::from_file(&args.config)?);

    info!(
        "Configuration loaded successfully with {} host(s)",
        config.hosts.len()
    );
    info!("Bind address: {}", config.server.bind_address);
    info!("Cloudflare Zone ID: {}", config.cloudflare.zone_id);

    let app = http::create_server(config.clone()).await?;

    let listener = TcpListener::bind(&config.server.bind_address)
        .await
        .map_err(error::Ddns6Error::Io)?;

    info!(
        "ddns6 daemon listening on {}",
        listener.local_addr().unwrap()
    );
    info!(
        "Update endpoint available at: http://{}/update",
        listener.local_addr().unwrap()
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| error::Ddns6Error::Io(std::io::Error::other(e)))?;

    info!("ddns6 daemon shut down gracefully");

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, shutting down...");
        },
        _ = terminate => {
            info!("Received SIGTERM, shutting down...");
        },
    }
}
