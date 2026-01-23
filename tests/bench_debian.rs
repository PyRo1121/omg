#[cfg(any(feature = "debian", feature = "debian-pure"))]
use omg_lib::daemon::handlers::DaemonState;
#[cfg(any(feature = "debian", feature = "debian-pure"))]
use omg_lib::daemon::protocol::Request;
#[cfg(any(feature = "debian", feature = "debian-pure"))]
use std::sync::Arc;
#[cfg(any(feature = "debian", feature = "debian-pure"))]
use std::time::Instant;

#[cfg(any(feature = "debian", feature = "debian-pure"))]
#[tokio::test]
async fn bench_debian_search_performance() {
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path().to_str().unwrap().to_string();

    // Setup test environment
    unsafe {
        std::env::set_var("OMG_TEST_MODE", "true");
        std::env::set_var("OMG_TEST_DISTRO", "debian");
        std::env::set_var("OMG_DAEMON_DATA_DIR", &temp_path);
    }

    // Initialize daemon state (this indexes the mock packages)
    let state = Arc::new(DaemonState::new().unwrap());

    // Warmup
    let req = Request::DebianSearch {
        id: 0,
        query: "apt".to_string(),
        limit: Some(10),
    };
    let _ = omg_lib::daemon::handlers::handle_request(state.clone(), req.clone()).await;

    // Benchmark
    let start = Instant::now();
    let iterations: u32 = 100;

    for i in 0..iterations {
        let req = Request::DebianSearch {
            id: u64::from(i),
            query: "apt".to_string(),
            limit: Some(10),
        };
        let _ = omg_lib::daemon::handlers::handle_request(state.clone(), req).await;
    }

    let duration = start.elapsed();
    let avg_ms = duration.as_secs_f64() * 1000.0 / f64::from(iterations);

    println!("Average search time: {avg_ms:.4} ms");

    // Requirement: sub-30ms
    assert!(
        avg_ms < 30.0,
        "Search performance too slow: {avg_ms:.4} ms > 30ms"
    );
}
