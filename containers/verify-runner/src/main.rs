//! Binary entry point: binds the axum router built in `lib.rs` to `0.0.0.0:8080`.

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use verify_runner::{app, AppState};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "verify_runner=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("bind 0.0.0.0:8080");
    tracing::info!("dry-verify-runner listening on 0.0.0.0:8080");
    axum::serve(listener, app(AppState::new()))
        .await
        .expect("axum::serve");
}
