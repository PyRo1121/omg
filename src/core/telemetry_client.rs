//! Telemetry API client for OMG
//!
//! Handles sending telemetry events to the API endpoints:
//! - POST /api/cli/event - Individual events
//! - POST /api/cli/batch - Batched events
//!
//! Features:
//! - Async with tokio
//! - Graceful error handling with retry queue
//! - Only sends when user has opted in (license key exists)

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::core::license::get_machine_id;

const EVENT_API_URL: &str = "https://api.pyro1121.com/api/cli/event";
const BATCH_API_URL: &str = "https://api.pyro1121.com/api/cli/batch";
const MAX_RETRY_QUEUE_SIZE: usize = 500;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5); // Reduced for faster failure detection
const MAX_RETRIES: u32 = 3;

// Circuit breaker constants
const CIRCUIT_FAILURE_THRESHOLD: u32 = 5; // Open circuit after 5 consecutive failures
const CIRCUIT_OPEN_DURATION_SECS: u64 = 300; // Stay open for 5 minutes (300s)

// Exponential backoff constants
const BASE_DELAY_MS: u64 = 1000; // 1 second base delay
const MAX_DELAY_MS: u64 = 60000; // 60 seconds max delay
const JITTER_PERCENT: u64 = 25; // ±25% jitter

/// Circuit breaker states: 0 = Closed, 1 = Open, 2 = Half-Open
static CIRCUIT_STATE: AtomicU32 = AtomicU32::new(0);

/// Consecutive failure count for circuit breaker
static FAILURE_COUNT: AtomicU32 = AtomicU32::new(0);

/// Timestamp of last failure (Unix epoch seconds as u64)
static LAST_FAILURE: AtomicU64 = AtomicU64::new(0);

/// Retry queue for failed events
static RETRY_QUEUE: Mutex<VecDeque<TelemetryEvent>> = Mutex::new(VecDeque::new());

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
            timestamp: jiff::Timestamp::now().strftime("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
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
                // Transition to half-open (will try one request)
                CIRCUIT_STATE.store(CircuitState::HalfOpen as u32, Ordering::Relaxed);
                tracing::debug!("Circuit breaker transitioning to half-open state");
                CircuitState::HalfOpen
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
    tracing::debug!("Circuit breaker reset to closed state");
}

/// Record a failed request (increment failure count, possibly open circuit)
fn record_failure() {
    let failures = FAILURE_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    let now = jiff::Timestamp::now().as_second() as u64;
    LAST_FAILURE.store(now, Ordering::Relaxed);

    if failures >= CIRCUIT_FAILURE_THRESHOLD {
        CIRCUIT_STATE.store(CircuitState::Open as u32, Ordering::Relaxed);
        tracing::debug!("Circuit breaker opened after {failures} consecutive failures");
    } else {
        tracing::debug!("Circuit breaker failure count: {failures}/{CIRCUIT_FAILURE_THRESHOLD}");
    }
}

/// Calculate exponential backoff delay with jitter
/// Formula: `min(BASE_DELAY * 2^attempt, MAX_DELAY) ± jitter`
fn calculate_backoff_delay(attempt: u32) -> Duration {
    // Calculate base delay: 1s * 2^attempt
    let base_delay = BASE_DELAY_MS.saturating_mul(2_u64.saturating_pow(attempt));

    // Cap at max delay
    let capped_delay = base_delay.min(MAX_DELAY_MS);

    // Add jitter: ±25% of the delay
    let jitter_range = capped_delay * JITTER_PERCENT / 100;
    let jitter = fastrand::u64(0..=jitter_range * 2); // 0 to 2*jitter_range
    let jitter_offset = jitter.saturating_sub(jitter_range); // -jitter_range to +jitter_range

    let final_delay = capped_delay.saturating_add(jitter_offset as u64);

    tracing::debug!("Backoff attempt {attempt}: {final_delay}ms (base={capped_delay}ms, jitter=±{jitter_range}ms)");
    Duration::from_millis(final_delay)
}

/// Check if telemetry should be sent (user opted in via license)
#[must_use]
pub fn should_send_telemetry() -> bool {
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

    // Require license for telemetry (user has explicitly opted in)
    crate::core::license::load_license().is_some()
}

/// Send a single telemetry event
pub async fn send_event(event: TelemetryEvent) -> Result<()> {
    if !should_send_telemetry() {
        return Ok(());
    }

    let payload = TelemetryPayload::new(event);
    send_event_internal(payload).await
}

/// Internal event sending with retry logic, circuit breaker, and exponential backoff
async fn send_event_internal(payload: TelemetryPayload) -> Result<()> {
    // Check circuit breaker state
    let circuit_state = check_circuit_breaker();

    if circuit_state == CircuitState::Open {
        tracing::debug!("Circuit breaker is open, queuing event locally");
        queue_for_retry(payload);
        return Ok(());
    }

    // Apply exponential backoff if this is a retry
    if payload.retries > 0 {
        let delay = calculate_backoff_delay(payload.retries);
        tracing::debug!("Applying backoff delay: {}ms", delay.as_millis());
        tokio::time::sleep(delay).await;
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
            Ok(())
        }
        Ok(resp) => {
            tracing::debug!("Telemetry event failed with status: {}", resp.status());
            record_failure();
            queue_for_retry(payload);
            Ok(())
        }
        Err(e) => {
            tracing::debug!("Telemetry event send error: {e}");
            record_failure();
            queue_for_retry(payload);
            Ok(())
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
        queue.push_back(payload.event);
    }
}

/// Send batched telemetry events with circuit breaker support
pub async fn send_batch(events: Vec<TelemetryEvent>) -> Result<()> {
    if !should_send_telemetry() || events.is_empty() {
        return Ok(());
    }

    // Check circuit breaker state
    let circuit_state = check_circuit_breaker();

    if circuit_state == CircuitState::Open {
        tracing::debug!("Circuit breaker is open, queuing {} events locally", events.len());
        for event in events {
            queue_for_retry(TelemetryPayload::new(event));
        }
        return Ok(());
    }

    let payloads: Vec<TelemetryPayload> = events
        .into_iter()
        .map(TelemetryPayload::new)
        .collect();

    let batch = BatchPayload {
        events: payloads,
        batch_timestamp: jiff::Timestamp::now().strftime("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
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
            tracing::debug!("Telemetry batch sent successfully ({} events)", batch.events.len());
            record_success();
            Ok(())
        }
        Ok(resp) => {
            tracing::debug!("Telemetry batch failed with status: {}", resp.status());
            record_failure();
            // Re-queue events for retry
            for payload in batch.events {
                queue_for_retry(payload);
            }
            Ok(())
        }
        Err(e) => {
            tracing::debug!("Telemetry batch send error: {e}");
            record_failure();
            for payload in batch.events {
                queue_for_retry(payload);
            }
            Ok(())
        }
    }
}

/// Flush retry queue (called periodically or on exit)
pub async fn flush_retry_queue() -> Result<()> {
    if !should_send_telemetry() {
        return Ok(());
    }

    let events: Vec<TelemetryEvent> = {
        if let Ok(mut queue) = RETRY_QUEUE.lock() {
            queue.drain(..).collect()
        } else {
            Vec::new()
        }
    };

    if events.is_empty() {
        return Ok(());
    }

    tracing::debug!("Flushing {} events from retry queue", events.len());
    send_batch(events).await
}

/// Send command telemetry event
#[inline]
pub async fn track_command(
    command: &str,
    subcommand: Option<&str>,
    packages: Option<&[String]>,
    duration_ms: u64,
    success: bool,
    error: Option<&str>,
) {
    let event = TelemetryEvent::Command(CommandEvent {
        command: command.to_string(),
        subcommand: subcommand.map(String::from),
        packages: packages.map(<[String]>::to_vec),
        duration_ms,
        success,
        error: error.map(String::from),
        result_count: None,
        updated_count: None,
    });

    if let Err(e) = send_event(event).await {
        tracing::debug!("Failed to send command telemetry: {e}");
    }
}

/// Send search telemetry event
#[inline]
pub async fn track_search(query: &str, result_count: usize, duration_ms: u64, success: bool) {
    let event = TelemetryEvent::Command(CommandEvent {
        command: "search".to_string(),
        subcommand: None,
        packages: Some(vec![query.to_string()]),
        duration_ms,
        success,
        error: None,
        result_count: Some(result_count),
        updated_count: None,
    });

    if let Err(e) = send_event(event).await {
        tracing::debug!("Failed to send search telemetry: {e}");
    }
}

/// Send update telemetry event
#[inline]
pub async fn track_update(updated_count: usize, duration_ms: u64, success: bool, error: Option<&str>) {
    let event = TelemetryEvent::Command(CommandEvent {
        command: "update".to_string(),
        subcommand: None,
        packages: None,
        duration_ms,
        success,
        error: error.map(String::from),
        result_count: None,
        updated_count: Some(updated_count),
    });

    if let Err(e) = send_event(event).await {
        tracing::debug!("Failed to send update telemetry: {e}");
    }
}

/// Send performance telemetry event
#[inline]
pub async fn track_performance(metric_type: &str, duration_ms: u64, context: Option<&str>) {
    let event = TelemetryEvent::Performance(PerformanceEvent {
        metric_type: metric_type.to_string(),
        duration_ms,
        context: context.map(String::from),
    });

    if let Err(e) = send_event(event).await {
        tracing::debug!("Failed to send performance telemetry: {e}");
    }
}

/// Send feature usage telemetry event
#[inline]
pub async fn track_feature(feature: &str, enabled: bool, metadata: Option<serde_json::Value>) {
    let event = TelemetryEvent::Feature(FeatureEvent {
        feature: feature.to_string(),
        enabled,
        metadata,
    });

    if let Err(e) = send_event(event).await {
        tracing::debug!("Failed to send feature telemetry: {e}");
    }
}

/// Send session start event
pub async fn track_session_start(session_id: &str) {
    let event = TelemetryEvent::Session(SessionEvent {
        session_id: session_id.to_string(),
        event_type: "start".to_string(),
        start_time: Some(jiff::Timestamp::now().strftime("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()),
        end_time: None,
        commands_run: None,
        duration_secs: None,
    });

    if let Err(e) = send_event(event).await {
        tracing::debug!("Failed to send session start telemetry: {e}");
    }
}

/// Send session end event
pub async fn track_session_end(session_id: &str, commands_run: u32, duration_secs: u64) {
    let event = TelemetryEvent::Session(SessionEvent {
        session_id: session_id.to_string(),
        event_type: "end".to_string(),
        start_time: None,
        end_time: Some(jiff::Timestamp::now().strftime("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()),
        commands_run: Some(commands_run),
        duration_secs: Some(duration_secs),
    });

    if let Err(e) = send_event(event).await {
        tracing::debug!("Failed to send session end telemetry: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_payload_creation() {
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
    fn test_command_event_serialization() {
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
    fn test_session_event_serialization() {
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
    fn test_circuit_breaker_state_transitions() {
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
    fn test_exponential_backoff_calculation() {
        // Attempt 0: 1s ± 25%
        let delay0 = calculate_backoff_delay(0);
        assert!(delay0.as_millis() >= 750 && delay0.as_millis() <= 1250);

        // Attempt 1: 2s ± 25%
        let delay1 = calculate_backoff_delay(1);
        assert!(delay1.as_millis() >= 1500 && delay1.as_millis() <= 2500);

        // Attempt 2: 4s ± 25%
        let delay2 = calculate_backoff_delay(2);
        assert!(delay2.as_millis() >= 3000 && delay2.as_millis() <= 5000);

        // Attempt 10: Should be capped at 60s ± 25%
        let delay10 = calculate_backoff_delay(10);
        assert!(delay10.as_millis() >= 45000 && delay10.as_millis() <= 75000);
    }

    #[test]
    fn test_backoff_max_delay_cap() {
        // Very high attempt number should be capped at MAX_DELAY_MS
        let delay = calculate_backoff_delay(20);
        assert!(delay.as_millis() <= MAX_DELAY_MS as u128 * 125 / 100); // Max + 25% jitter
    }
}
