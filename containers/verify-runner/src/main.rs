//! Binary entry point: binds the axum router built in `lib.rs` to `0.0.0.0:8080`.

use verify_runner::{app, AppState};

#[tokio::main]
async fn main() {
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("bind 0.0.0.0:8080");
    println!("dry-verify-runner listening on 0.0.0.0:8080");
    axum::serve(listener, app(AppState::new()))
        .await
        .expect("axum::serve");
}
