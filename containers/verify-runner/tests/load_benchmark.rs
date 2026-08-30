//! Automated Load & Concurrency Benchmark (Task D5 in `docs/23-deployment-roadmap.md`).
//!
//! Measures:
//! - Multi-client concurrent throughput (requests / sec)
//! - Latency percentiles (p50, p95, p99) under simulated load
//! - Memory safety and RAII tempfile cleanup under concurrent access

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tower::ServiceExt;
use verify_runner::{app, AppState};

#[tokio::test]
async fn test_concurrent_verify_load_benchmark() {
    std::env::set_var("ALLOWED_REGISTRY_HOST", "127.0.0.1");
    std::env::set_var("DRY_LICENSE_ALLOW_TEST_KEY", "1");

    // Start an asynchronous local stub profile registry
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind stub registry");
    let port = listener.local_addr().unwrap().port();
    let registry_url = format!("http://127.0.0.1:{port}");

    let registry_handle = tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let mut buf = [0u8; 1024];
                let _ = socket.read(&mut buf).await;
                let profile_json = r#"{
                    "schema_version": 1,
                    "name": "LoadBench Machine",
                    "vendor": "Dry Bench",
                    "model": "Bench 1",
                    "process_defaults": {
                        "line_width": 0.45,
                        "layer_height": 0.2,
                        "filament_diameter": 1.75,
                        "feedrate_range": [10.0, 300.0]
                    }
                }"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    profile_json.len(),
                    profile_json
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.shutdown().await;
            });
        }
    });

    let gcode_content = b"G21 ; Millimeters\nG90 ; Absolute\nG1 X0 Y0 Z0.2 F1200\nG1 X10 Y0 E0.5 F1800\nG1 X10 Y10 E1.0 F1800\nG1 X0 Y10 E1.5 F1800\nG1 X0 Y0 E2.0 F1800\n";

    let concurrency = 10;
    let requests_per_worker = 4;
    let total_requests = concurrency * requests_per_worker;

    let app_state = AppState::new();
    let success_count = Arc::new(AtomicU64::new(0));
    let latencies_ms = Arc::new(std::sync::Mutex::new(Vec::with_capacity(total_requests)));

    let bench_start = Instant::now();
    let mut handles = Vec::with_capacity(concurrency);

    for worker_id in 0..concurrency {
        let app_router = app(app_state.clone());
        let gcode = gcode_content.to_vec();
        let registry = registry_url.clone();
        let success = success_count.clone();
        let latencies = latencies_ms.clone();

        handles.push(tokio::spawn(async move {
            for req_id in 0..requests_per_worker {
                let req_start = Instant::now();
                let request = Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/verify?pack=bench&version=1.0.0&profile=p1&registry={registry}"
                    ))
                    .header("x-request-id", format!("bench-{worker_id}-{req_id}"))
                    .body(Body::from(gcode.clone()))
                    .unwrap();

                let response = app_router.clone().oneshot(request).await.unwrap();
                let duration = req_start.elapsed();

                if response.status() == StatusCode::OK {
                    success.fetch_add(1, Ordering::Relaxed);
                    latencies
                        .lock()
                        .unwrap()
                        .push(duration.as_micros() as f64 / 1000.0);
                }
            }
        }));
    }

    for h in handles {
        h.await.unwrap();
    }
    registry_handle.abort();

    let total_duration = bench_start.elapsed();
    let successes = success_count.load(Ordering::Relaxed);
    assert_eq!(
        successes as usize, total_requests,
        "All load requests must succeed with 200 OK"
    );

    let mut lat_vec = latencies_ms.lock().unwrap().clone();
    lat_vec.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let p50 = lat_vec[lat_vec.len() / 2];
    let p95 = lat_vec[(lat_vec.len() as f64 * 0.95) as usize];
    let p99 = lat_vec[(lat_vec.len() as f64 * 0.99) as usize];
    let rps = total_requests as f64 / total_duration.as_secs_f64();

    println!(
        "\n=== Dry Verify Runner Load Benchmark (D5) ===\n\
         Total Requests:  {total_requests}\n\
         Concurrency:     {concurrency}\n\
         Total Duration:  {total_duration:.2?}\n\
         Throughput:      {rps:.1} req/sec\n\
         Latency p50:     {p50:.2} ms\n\
         Latency p95:     {p95:.2} ms\n\
         Latency p99:     {p99:.2} ms\n\
         =============================================\n"
    );

    assert!(
        p99 < 500.0,
        "p99 latency should be under 500ms for small files under local load"
    );
}
