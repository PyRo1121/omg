//! Telemetry API client for OMG
//!
//! Handles sending telemetry events to the API endpoints:
//! - POST /api/cli/event - Individual events
//! - POST /api/cli/batch - Batched events
//!
//! Features:
//! - Async with tokio
//! - Graceful error handling with retry queue
//! - Only sends when the user has a signed, unexpired license token
//!
//! Retry pacing is enforced by the circuit breaker plus a bounded flush
//! budget ([`RETRY_FLUSH_BUDGET`]); there is no per-attempt sleep.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::core::license::get_machine_id;

const EVENT_API_URL: &str = "https://api.pyro1121.com/api/cli/event";
const BATCH_API_URL: &str = "https://api.pyro1121.com/api/cli/batch";
const MAX_RETRY_QUEUE_SIZE: usize = 500;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_RETRIES: u32 = 3;

// Circuit breaker constants
const CIRCUIT_FAILURE_THRESHOLD: u32 = 5; // Open circuit after 5 consecutive failures
const CIRCUIT_OPEN_DURATION_SECS: u64 = 300; // Stay open for 5 minutes (300s)

/// Upper bound on time spent draining the retry queue per flush. Flushing
/// runs on CLI-exit paths (`end_session_and_flush`), so the drain must stay
/// far shorter than a user's patience; events that do not fit stay queued
/// for the next flush.
const RETRY_FLUSH_BUDGET: Duration = Duration::from_secs(2);

/// Circuit breaker states: 0 = Closed, 1 = Open, 2 = Half-Open
static CIRCUIT_STATE: AtomicU32 = AtomicU32::new(0);

/// Consecutive failure count for circuit breaker
static FAILURE_COUNT: AtomicU32 = AtomicU32::new(0);

/// Single-flight flag for half-open probes: only one request may probe the
/// recovering endpoint at a time; all others stay queued while it is in
/// flight (otherwise every queued event would probe concurrently).
static HALF_OPEN_PROBE_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

/// Timestamp of last failure (Unix epoch seconds as u64)
static LAST_FAILURE: AtomicU64 = AtomicU64::new(0);

/// Retry queue for failed events
static RETRY_QUEUE: Mutex<VecDeque<TelemetryPayload>> = Mutex::new(VecDeque::new());

/// Command-level telemetry event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandEvent {
    /// Command name (e.g., "install", "search", "update")
    pub command: String,
    /// Optional subcommand (e.g., "packages" for "install packages")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subcommand: Option<String>,
    /// Package name(s) if applicable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packages: Option<Vec<String>>,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Whether the command succeeded
    pub success: bool,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Result count (for search)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_count: Option<usize>,
    /// Packages updated count (for update)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_count: Option<usize>,
}

/// Session tracking event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEvent {
    /// Session ID (UUID)
    pub session_id: String,
    /// Event type: "start", "heartbeat", "end"
    pub event_type: String,
    /// Session start timestamp (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    /// Session end timestamp (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
    /// Commands run in this session
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commands_run: Option<u32>,
    /// Duration in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<u64>,
}

/// Performance metrics event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceEvent {
    /// Metric type: "startup", "search", "install", "sync", etc.
    pub metric_type: String,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Additional context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

/// Feature usage event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureEvent {
    /// Feature name: "daemon", "parallel", "sbom", "fleet", "aur", etc.
    pub feature: String,
    /// Whether the feature is enabled/used
    pub enabled: bool,
    /// Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Unified telemetry event wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TelemetryEvent {
    Command(CommandEvent),
    Session(SessionEvent),
    Performance(PerformanceEvent),
    Feature(FeatureEvent),
}

/// Telemetry event with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryPayload {
    /// The event data
    pub event: TelemetryEvent,
    /// Timestamp (ISO 8601)
    pub timestamp: String,
    /// Machine ID
    pub machine_id: String,
    /// OMG version
    pub version: String,
    /// Platform (e.g., "linux-x86_64")
    pub platform: String,
    /// License key (if activated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license_key: Option<String>,
    /// Retry count
    #[serde(default)]
    pub retries: u32,
}

/// Batch telemetry payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchPayload {
    /// List of events
    pub events: Vec<TelemetryPayload>,
    /// Batch timestamp
    pub batch_timestamp: String,
    /// Machine ID
    pub machine_id: String,
}

impl TelemetryPayload {
    /// Create a new telemetry payload
    #[must_use]
    pub fn new(event: TelemetryEvent) -> Self {
        let license = crate::core::license::load_license();

        Self {
            event,
            timestamp: jiff::Timestamp::now()
                .strftime("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string(),
            machine_id: get_machine_id(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
            license_key: license.map(|l| l.key),
            retries: 0,
        }
    }
}

/// Circuit breaker state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CircuitState {
    Closed = 0,
    Open = 1,
    HalfOpen = 2,
}

impl From<u32> for CircuitState {
    fn from(value: u32) -> Self {
        match value {
            1 => CircuitState::Open,
            2 => CircuitState::HalfOpen,
            _ => CircuitState::Closed, // 0 or invalid values default to Closed
        }
    }
}

/// Check circuit breaker state and update if needed
fn check_circuit_breaker() -> CircuitState {
    let current_state = CircuitState::from(CIRCUIT_STATE.load(Ordering::Relaxed));

    match current_state {
        CircuitState::Closed => CircuitState::Closed,
        CircuitState::Open => {
            // Check if enough time has passed to try again (half-open)
            let last_failure = LAST_FAILURE.load(Ordering::Relaxed);
            let now = jiff::Timestamp::now().as_second() as u64;

            if now.saturating_sub(last_failure) >= CIRCUIT_OPEN_DURATION_SECS {
                // Transition to half-open with single-flight semantics: the
                // first caller wins the probe slot; concurrent callers stay
                // queued until the probe resolves (record_success/failure).
                if HALF_OPEN_PROBE_IN_FLIGHT
                    .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
                    .is_ok()
                {
                    CIRCUIT_STATE.store(CircuitState::HalfOpen as u32, Ordering::Relaxed);
                    tracing::debug!("Circuit breaker transitioning to half-open state");
                    CircuitState::HalfOpen
                } else {
                    CircuitState::Open
                }
            } else {
                CircuitState::Open
            }
        }
        CircuitState::HalfOpen => CircuitState::HalfOpen,
    }
}

/// Record a successful request (reset circuit breaker)
fn record_success() {
    FAILURE_COUNT.store(0, Ordering::Relaxed);
    CIRCUIT_STATE.store(CircuitState::Closed as u32, Ordering::Relaxed);
    HALF_OPEN_PROBE_IN_FLIGHT.store(false, Ordering::Relaxed);
    tracing::debug!("Circuit breaker reset to closed state");
}

/// Record a failed request (increment failure count, possibly open circuit)
fn record_failure() {
    let failures = FAILURE_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    let now = jiff::Timestamp::now().as_second() as u64;
    LAST_FAILURE.store(now, Ordering::Relaxed);
    // Any in-flight half-open probe has now resolved (as a failure).
    HALF_OPEN_PROBE_IN_FLIGHT.store(false, Ordering::Relaxed);

    if failures >= CIRCUIT_FAILURE_THRESHOLD {
        CIRCUIT_STATE.store(CircuitState::Open as u32, Ordering::Relaxed);
        tracing::debug!("Circuit breaker opened after {failures} consecutive failures");
    } else {
        tracing::debug!("Circuit breaker failure count: {failures}/{CIRCUIT_FAILURE_THRESHOLD}");
    }
}

/// Check if telemetry should be sent (signed license token present)
fn should_send_telemetry() -> bool {
    // Only send telemetry if:
    // 1. User has a license (opted in)
    // 2. Telemetry is not explicitly disabled
    // 3. Not in test mode
    if crate::core::paths::test_mode() {
        return false;
    }

    if crate::core::telemetry::is_telemetry_opt_out() {
        return false;
    }

    // Enhanced telemetry requires a signed, unexpired license token.
    crate::core::license::load_license().is_some_and(|license| license.is_token_valid())
}

/// Attempt to send one telemetry event.
///
/// This is deliberately infallible: telemetry failures are never propagated
/// as errors. Every failure path updates the circuit breaker and queues the
/// payload for a later retry instead.
async fn send_event_internal(payload: TelemetryPayload) {
    // Check circuit breaker state
    let circuit_state = check_circuit_breaker();

    if circuit_state == CircuitState::Open {
        tracing::debug!("Circuit breaker is open, queuing event locally");
        queue_for_retry(payload);
        return;
    }

    let client = crate::core::http::shared_client();

    let response = client
        .post(EVENT_API_URL)
        .json(&payload)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            tracing::debug!("Telemetry event sent successfully");
            record_success();
        }
        Ok(resp) => {
            tracing::debug!("Telemetry event failed with status: {}", resp.status());
            record_failure();
            queue_for_retry(payload);
        }
        Err(e) => {
            tracing::debug!("Telemetry event send error: {e}");
            record_failure();
            queue_for_retry(payload);
        }
    }
}

/// Queue an event for retry
fn queue_for_retry(mut payload: TelemetryPayload) {
    payload.retries += 1;

    if payload.retries > MAX_RETRIES {
        tracing::debug!("Dropping telemetry event after {MAX_RETRIES} retries");
        return;
    }

    if let Ok(mut queue) = RETRY_QUEUE.lock() {
        if queue.len() >= MAX_RETRY_QUEUE_SIZE {
            // Drop oldest events to make room
            queue.drain(0..MAX_RETRY_QUEUE_SIZE / 2);
        }
        queue.push_back(payload);
    }
}

/// Remove the oldest queued payload.
fn pop_retry_queue() -> Option<TelemetryPayload> {
    RETRY_QUEUE
        .lock()
        .ok()
        .and_then(|mut queue| queue.pop_front())
}

/// Return a payload to the front of the retry queue with its retry count
/// preserved. Used when the flush budget runs out so un-sent events are
/// retried first on the next flush.
fn push_retry_queue_front(payload: TelemetryPayload) {
    if let Ok(mut queue) = RETRY_QUEUE.lock() {
        if queue.len() >= MAX_RETRY_QUEUE_SIZE {
            return; // Queue is full; drop rather than evict newer events.
        }
        queue.push_front(payload);
    }
}

/// Drain queued retry events within [`RETRY_FLUSH_BUDGET`].
///
/// Events are sent one at a time, each capped at the remaining budget. When
/// the budget is exhausted (or a single request would exceed it), the
/// un-sent event is returned to the *front* of the queue with its retry
/// count preserved and the rest of the queue is left untouched. At most one
/// event can be duplicated in the rare case where a request succeeds
/// server-side but exceeds the time budget.
async fn drain_retry_queue_within_budget() {
    let deadline = tokio::time::Instant::now() + RETRY_FLUSH_BUDGET;
    loop {
        if check_circuit_breaker() == CircuitState::Open {
            tracing::debug!("Circuit breaker is open; leaving retry queue untouched");
            return;
        }
        let Some(payload) = pop_retry_queue() else {
            return;
        };
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            push_retry_queue_front(payload);
            return;
        }
        match tokio::time::timeout(remaining, send_event_internal(payload.clone())).await {
            Ok(()) => {}
            Err(_elapsed) => {
                tracing::debug!("Retry flush budget exhausted; returning event to queue");
                push_retry_queue_front(payload);
                return;
            }
        }
    }
}

/// Flush retry queue (called periodically or on exit)
pub async fn flush_retry_queue() -> Result<()> {
    if !should_send_telemetry() {
        return Ok(());
    }

    drain_retry_queue_within_budget().await;
    Ok(())
}

/// Send batched telemetry events with circuit breaker support
pub async fn send_batch(events: Vec<TelemetryEvent>) -> Result<()> {
    if !should_send_telemetry() || events.is_empty() {
        return Ok(());
    }

    // Check circuit breaker state
    let circuit_state = check_circuit_breaker();

    if circuit_state == CircuitState::Open {
        anyhow::bail!("Telemetry circuit breaker is open");
    }

    let payloads: Vec<TelemetryPayload> = events.into_iter().map(TelemetryPayload::new).collect();

    let batch = BatchPayload {
        events: payloads,
        batch_timestamp: jiff::Timestamp::now()
            .strftime("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string(),
        machine_id: get_machine_id(),
    };

    let client = crate::core::http::shared_client();

    let response = client
        .post(BATCH_API_URL)
        .json(&batch)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            tracing::debug!(
                "Telemetry batch sent successfully ({} events)",
                batch.events.len()
            );
            record_success();
            Ok(())
        }
        Ok(resp) => {
            let status = resp.status();
            tracing::debug!("Telemetry batch failed with status: {status}");
            record_failure();
            anyhow::bail!("Telemetry batch rejected with status {status}")
        }
        Err(error) => {
            tracing::debug!("Telemetry batch send error: {error}");
            record_failure();
            Err(error.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_creation_fills_metadata() {
        let event = TelemetryEvent::Command(CommandEvent {
            command: "install".to_string(),
            subcommand: None,
            packages: Some(vec!["firefox".to_string()]),
            duration_ms: 1500,
            success: true,
            error: None,
            result_count: None,
            updated_count: None,
        });

        let payload = TelemetryPayload::new(event);
        assert!(!payload.timestamp.is_empty());
        assert!(!payload.machine_id.is_empty());
        assert!(!payload.version.is_empty());
        assert!(!payload.platform.is_empty());
    }

    #[test]
    #[serial_test::serial]
    fn retry_queue_preserves_attempt_count() {
        reset_retry_queue();
        let payload = TelemetryPayload {
            event: TelemetryEvent::Feature(FeatureEvent {
                feature: "test".to_string(),
                enabled: true,
                metadata: None,
            }),
            timestamp: String::new(),
            machine_id: String::new(),
            version: String::new(),
            platform: String::new(),
            license_key: None,
            retries: 1,
        };

        queue_for_retry(payload);

        let mut queue = RETRY_QUEUE.lock().expect("retry queue");
        assert_eq!(queue.front().map(|payload| payload.retries), Some(2));
        queue.clear();
    }

    #[test]
    fn command_event_serializes_payload_fields() {
        let event = CommandEvent {
            command: "search".to_string(),
            subcommand: None,
            packages: Some(vec!["vim".to_string()]),
            duration_ms: 50,
            success: true,
            error: None,
            result_count: Some(25),
            updated_count: None,
        };

        let json = serde_json::to_string(&event).expect("serialization should succeed");
        assert!(json.contains("search"));
        assert!(json.contains("vim"));
        assert!(json.contains("25"));
    }

    #[test]
    fn session_event_serializes_session_fields() {
        let event = SessionEvent {
            session_id: "test-session-123".to_string(),
            event_type: "start".to_string(),
            start_time: Some("2024-01-01T00:00:00.000Z".to_string()),
            end_time: None,
            commands_run: None,
            duration_secs: None,
        };

        let json = serde_json::to_string(&event).expect("serialization should succeed");
        assert!(json.contains("test-session-123"));
        assert!(json.contains("start"));
    }

    #[test]
    fn circuit_breaker_state_transitions_match_threshold() {
        // Reset state
        CIRCUIT_STATE.store(CircuitState::Closed as u32, Ordering::Relaxed);
        FAILURE_COUNT.store(0, Ordering::Relaxed);

        // Initially closed
        assert_eq!(check_circuit_breaker(), CircuitState::Closed);

        // Record failures to open circuit
        for i in 1..=CIRCUIT_FAILURE_THRESHOLD {
            record_failure();
            if i < CIRCUIT_FAILURE_THRESHOLD {
                assert_eq!(check_circuit_breaker(), CircuitState::Closed);
            } else {
                assert_eq!(check_circuit_breaker(), CircuitState::Open);
            }
        }

        // Circuit should stay open
        assert_eq!(check_circuit_breaker(), CircuitState::Open);

        // Success should reset circuit
        record_success();
        assert_eq!(check_circuit_breaker(), CircuitState::Closed);
        assert_eq!(FAILURE_COUNT.load(Ordering::Relaxed), 0);
    }

    #[test]
    #[serial_test::serial]
    fn retry_queue_front_push_preserves_order_and_retries() {
        reset_retry_queue();
        let first = sample_payload(0);
        let second = sample_payload(2);
        queue_for_retry(first);
        queue_for_retry(second);

        let head = sample_payload(9);
        push_retry_queue_front(head);

        // Copy observations out under the guard so a failing assert cannot
        // poison the shared mutex for the other serial tests.
        let (front, back, len) = {
            let queue = RETRY_QUEUE
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (
                queue.front().map(|p| p.retries),
                queue.back().map(|p| p.retries),
                queue.len(),
            )
        };
        assert_eq!(front, Some(9));
        assert_eq!(back, Some(3));
        assert_eq!(len, 3);
        reset_retry_queue();
    }

    /// Clear the shared retry queue, recovering from poisoning so one
    /// panicking test cannot break its serial successors.
    fn reset_retry_queue() {
        RETRY_QUEUE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }

    #[test]
    #[serial_test::serial]
    fn drain_leaves_queue_untouched_when_circuit_is_open() {
        // Regression for the bounded retry flush: an open circuit must end
        // the drain immediately without consuming or altering queued events.
        reset_retry_queue();
        CIRCUIT_STATE.store(CircuitState::Open as u32, Ordering::Relaxed);
        FAILURE_COUNT.store(CIRCUIT_FAILURE_THRESHOLD, Ordering::Relaxed);
        LAST_FAILURE.store(
            u64::try_from(jiff::Timestamp::now().as_second()).unwrap_or(0),
            Ordering::Relaxed,
        );
        HALF_OPEN_PROBE_IN_FLIGHT.store(false, Ordering::Relaxed);
        queue_for_retry(sample_payload(1));
        queue_for_retry(sample_payload(1));

        let started = std::time::Instant::now();
        // A current-thread runtime guarantees the drain observes this
        // thread's circuit-breaker setup (Relaxed atomics are only
        // same-thread coherent); a multi-thread runtime could poll the
        // future on a worker with a stale view and hit the real network.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime");
        runtime.block_on(drain_retry_queue_within_budget());
        let elapsed = started.elapsed();

        let remaining = RETRY_QUEUE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len();
        assert_eq!(remaining, 2);
        assert!(
            elapsed < RETRY_FLUSH_BUDGET,
            "drain must return promptly when the circuit is open, took {elapsed:?}"
        );

        // Restore closed state and clear the queue for other tests.
        record_success();
        reset_retry_queue();
    }

    /// A minimal valid payload for retry-queue tests.
    fn sample_payload(retries: u32) -> TelemetryPayload {
        TelemetryPayload {
            event: TelemetryEvent::Feature(FeatureEvent {
                feature: "test".to_string(),
                enabled: true,
                metadata: None,
            }),
            timestamp: String::new(),
            machine_id: String::new(),
            version: String::new(),
            platform: String::new(),
            license_key: None,
            retries,
        }
    }
}
