use std::sync::Arc;

use tokio::net::TcpListener;
use tracing::info;

use rlnk::{
    config::AppConfig,
    error::{AppError, BootstrapError},
    http::{AppState, app},
    store::{LinkStore, MongoLinkStore},
};

#[tokio::main]
async fn main() -> Result<(), BootstrapError> {
    init_tracing();

    let config = Arc::new(AppConfig::from_env()?);
    let store = MongoLinkStore::connect(&config).await?;
    store
        .create_indexes()
        .await
        .map_err(bootstrap_store_error)?;

    let listener = TcpListener::bind(config.bind_addr).await?;
    info!(address = %config.bind_addr, "rlnk listening");

    axum::serve(listener, app(AppState::new(config, store)))
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rlnk=debug,tower_http=info".into()),
        )
        .with_target(false)
        .compact()
        .init();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::warn!(%error, "failed to listen for ctrl-c");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                tracing::warn!(%error, "failed to listen for SIGTERM");
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}

fn bootstrap_store_error(error: AppError) -> BootstrapError {
    match error {
        AppError::Database(source) => BootstrapError::Mongo(source),
        other => BootstrapError::Io(std::io::Error::other(other.to_string())),
    }
}
