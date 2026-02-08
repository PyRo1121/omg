//! Telemetry and install tracking
//!
//! Privacy-first telemetry that tracks install counts for GitHub badge display.
//! - Opt-out via `OMG_TELEMETRY=0` environment variable
//! - Anonymous data collection (no personal information)
//! - One-time ping on first install only
//! - Silent failure if network unavailable
//!
//! ## Enhanced Telemetry (requires license)
//!
//! When a user has activated a license, additional telemetry is collected:
//! - Command-level event tracking (command, package, duration, success/error)
//! - Session tracking (`session_id`, start/end times)
//! - Performance metrics (`startup_ms`, `search_ms`, `install_ms`)
//! - Feature usage tracking (daemon, parallel, sbom, fleet, aur)
//!
//! Events are batched and sent every 60 seconds or on CLI exit.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::core::telemetry_client::{
    CommandEvent, FeatureEvent, PerformanceEvent, SessionEvent, TelemetryEvent,
};

const TELEMETRY_API_URL: &str = "https://api.pyro1121.com/api/install-ping";
/// Batch flush interval in seconds
const BATCH_FLUSH_INTERVAL_SECS: i64 = 60;
/// Maximum events to queue before forcing flush
const MAX_QUEUED_EVENTS: usize = 100;

/// Install telemetry payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallPayload {
    /// Anonymous install ID (UUID v4)
    pub install_id: String,
    /// Install timestamp (ISO 8601)
    pub timestamp: String,
    /// OMG version
    pub version: String,
    /// Platform (e.g., "linux-x86_64")
    pub platform: String,
    /// Package manager backend (arch/debian)
    pub backend: String,
}

/// Marker file content
#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstallMarker {
    install_id: String,
    timestamp: String,
    version: String,
}

/// Check if telemetry is opted out
#[must_use]
pub fn is_telemetry_opt_out() -> bool {
    if crate::core::paths::test_mode() {
        return true;
    }

    // Check environment variables first (highest priority)
    let env_opt_out = matches!(
        std::env::var("OMG_TELEMETRY").as_deref(),
        Ok("0" | "false" | "FALSE" | "off" | "OFF")
    ) || matches!(
        std::env::var("OMG_DISABLE_TELEMETRY").as_deref(),
        Ok("1" | "true" | "TRUE")
    );

    if env_opt_out {
        return true;
    }

    // Check settings file
    if let Ok(settings) = crate::config::Settings::load()
        && !settings.telemetry_enabled
    {
        return true;
    }

    false
}

/// Check if this is the first run
pub fn is_first_run() -> bool {
    let marker_path = super::paths::installed_marker_path();
    !marker_path.exists()
}

/// Generate or load install ID
fn generate_or_load_id() -> Result<String> {
    let marker_path = super::paths::installed_marker_path();

    if marker_path.exists() {
        // Load existing ID
        let content = std::fs::read_to_string(&marker_path)?;
        let marker: InstallMarker = serde_json::from_str(&content)?;
        Ok(marker.install_id)
    } else {
        // Generate new ID
        Ok(uuid::Uuid::new_v4().to_string())
    }
}

/// Get platform string (e.g., "linux-x86_64")
fn get_platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

/// Get package manager backend
pub fn get_backend() -> String {
    #[cfg(feature = "arch")]
    return "arch".to_string();

    #[cfg(all(feature = "debian", not(feature = "arch")))]
    return "debian".to_string();

    #[cfg(not(any(feature = "arch", feature = "debian")))]
    return "unknown".to_string();
}

/// Create install marker file
fn create_marker(install_id: &str) -> Result<()> {
    let marker_path = super::paths::installed_marker_path();

    // Ensure parent directory exists
    if let Some(parent) = marker_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let marker = InstallMarker {
        install_id: install_id.to_string(),
        timestamp: jiff::Timestamp::now().to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };

    let content = serde_json::to_string_pretty(&marker)?;
    std::fs::write(marker_path, content)?;

    Ok(())
}

/// Ping install telemetry endpoint
pub async fn ping_install() -> Result<()> {
    // Generate or load install ID
    let install_id = generate_or_load_id()?;

    // Create payload
    let payload = InstallPayload {
        install_id: install_id.clone(),
        timestamp: jiff::Timestamp::now().to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        platform: get_platform(),
        backend: get_backend(),
    };

    // Send ping with timeout
    let client = crate::core::http::shared_client();
    let response = client.post(TELEMETRY_API_URL).json(&payload).send().await;

    // Create marker file on success or network error (don't block on failure)
    match response {
        Ok(_) => {
            tracing::debug!("Install telemetry ping successful");
            create_marker(&install_id)?;
        }
        Err(e) => {
            // Silent fail - create marker anyway to avoid retrying
            tracing::debug!("Install telemetry ping failed (silent): {}", e);
            create_marker(&install_id)?;
        }
    }

    Ok(())
}

// =============================================================================
// Enhanced Telemetry (requires license)
// =============================================================================

/// Global event queue for batching
static EVENT_QUEUE: OnceLock<Mutex<EventQueue>> = OnceLock::new();

/// Global session state
static SESSION_STATE: OnceLock<Mutex<TelemetrySession>> = OnceLock::new();

/// Global CLI start time for startup metrics
static CLI_START_TIME: OnceLock<Instant> = OnceLock::new();

/// Event queue for batching telemetry events
#[derive(Debug, Default)]
struct EventQueue {
    events: VecDeque<TelemetryEvent>,
    last_flush: i64,
}

impl EventQueue {
    fn push(&mut self, event: TelemetryEvent) {
        self.events.push_back(event);
    }

    fn needs_flush(&self) -> bool {
        let now = jiff::Timestamp::now().as_second();
        now - self.last_flush >= BATCH_FLUSH_INTERVAL_SECS
            || self.events.len() >= MAX_QUEUED_EVENTS
    }

    fn take_events(&mut self) -> Vec<TelemetryEvent> {
        self.last_flush = jiff::Timestamp::now().as_second();
        self.events.drain(..).collect()
    }

    fn path() -> Result<PathBuf> {
        let data_dir = crate::core::paths::data_dir();
        std::fs::create_dir_all(&data_dir)?;
        Ok(data_dir.join("telemetry_queue.json"))
    }

    fn load() -> Self {
        Self::path()
            .ok()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| {
                let events: Vec<TelemetryEvent> = serde_json::from_str(&s).ok()?;
                Some(Self {
                    events: events.into(),
                    last_flush: jiff::Timestamp::now().as_second(),
                })
            })
            .unwrap_or_default()
    }

    fn save(&self) -> Result<()> {
        let path = Self::path()?;
        let events: Vec<&TelemetryEvent> = self.events.iter().collect();
        let content = serde_json::to_string(&events)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

/// Session state for tracking CLI sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetrySession {
    /// Session ID (UUID)
    pub session_id: String,
    /// Session start timestamp
    pub started_at: String,
    /// Commands run this session
    pub commands_run: u32,
    /// Last activity timestamp (unix seconds)
    pub last_activity: i64,
}

impl Default for TelemetrySession {
    fn default() -> Self {
        Self::new()
    }
}

impl TelemetrySession {
    fn new() -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            started_at: jiff::Timestamp::now().strftime("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            commands_run: 0,
            last_activity: jiff::Timestamp::now().as_second(),
        }
    }

    fn path() -> Result<PathBuf> {
        let data_dir = crate::core::paths::data_dir();
        std::fs::create_dir_all(&data_dir)?;
        Ok(data_dir.join("telemetry_session.json"))
    }

    fn load() -> Self {
        Self::path()
            .ok()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save(&self) -> Result<()> {
        let path = Self::path()?;
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Check if session has expired (30 min inactivity)
    fn is_expired(&self) -> bool {
        let now = jiff::Timestamp::now().as_second();
        now - self.last_activity > 1800 // 30 minutes
    }

    /// Get session duration in seconds
    fn duration_secs(&self) -> u64 {
        if let Ok(started) = jiff::Timestamp::strptime("%Y-%m-%dT%H:%M:%S%.fZ", &self.started_at) {
            let now = jiff::Timestamp::now().as_second();
            (now - started.as_second()).max(0) as u64
        } else {
            0
        }
    }
}

/// Get or create the event queue
fn get_event_queue() -> &'static Mutex<EventQueue> {
    EVENT_QUEUE.get_or_init(|| Mutex::new(EventQueue::load()))
}

/// Get or create the session state
fn get_session() -> &'static Mutex<TelemetrySession> {
    SESSION_STATE.get_or_init(|| {
        let mut session = TelemetrySession::load();
        if session.is_expired() {
            session = TelemetrySession::new();
            let _ = session.save();
        }
        Mutex::new(session)
    })
}

/// Check if enhanced telemetry is enabled (requires license)
#[must_use]
pub fn is_enhanced_telemetry_enabled() -> bool {
    if is_telemetry_opt_out() {
        return false;
    }
    crate::core::license::load_license().is_some()
}

/// Record CLI startup time
pub fn record_startup_time() {
    CLI_START_TIME.get_or_init(Instant::now);
}

/// Get CLI startup duration in milliseconds
#[must_use]
pub fn get_startup_duration_ms() -> Option<u64> {
    CLI_START_TIME.get().map(|start| start.elapsed().as_millis() as u64)
}

/// Get current session ID
#[must_use]
pub fn get_session_id() -> String {
    if let Ok(session) = get_session().lock() {
        session.session_id.clone()
    } else {
        uuid::Uuid::new_v4().to_string()
    }
}

/// Queue a telemetry event
pub fn queue_event(event: TelemetryEvent) {
    if !is_enhanced_telemetry_enabled() {
        return;
    }

    if let Ok(mut queue) = get_event_queue().lock() {
        queue.push(event);
        let _ = queue.save();
    }
}

/// Track command execution
pub fn track_command_event(
    command: &str,
    subcommand: Option<&str>,
    packages: Option<&[String]>,
    duration_ms: u64,
    success: bool,
    error: Option<&str>,
) {
    if !is_enhanced_telemetry_enabled() {
        return;
    }

    // Update session
    if let Ok(mut session) = get_session().lock() {
        session.commands_run += 1;
        session.last_activity = jiff::Timestamp::now().as_second();
        let _ = session.save();
    }

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

    queue_event(event);
}

/// Track search command with result count
pub fn track_search_event(query: &str, result_count: usize, duration_ms: u64, success: bool) {
    if !is_enhanced_telemetry_enabled() {
        return;
    }

    // Update session
    if let Ok(mut session) = get_session().lock() {
        session.commands_run += 1;
        session.last_activity = jiff::Timestamp::now().as_second();
        let _ = session.save();
    }

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

    queue_event(event);
}

/// Track update command with updated count
pub fn track_update_event(updated_count: usize, duration_ms: u64, success: bool, error: Option<&str>) {
    if !is_enhanced_telemetry_enabled() {
        return;
    }

    // Update session
    if let Ok(mut session) = get_session().lock() {
        session.commands_run += 1;
        session.last_activity = jiff::Timestamp::now().as_second();
        let _ = session.save();
    }

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

    queue_event(event);
}

/// Track performance metric
pub fn track_performance_event(metric_type: &str, duration_ms: u64, context: Option<&str>) {
    if !is_enhanced_telemetry_enabled() {
        return;
    }

    let event = TelemetryEvent::Performance(PerformanceEvent {
        metric_type: metric_type.to_string(),
        duration_ms,
        context: context.map(String::from),
    });

    queue_event(event);
}

/// Track feature usage
pub fn track_feature_event(feature: &str, enabled: bool, metadata: Option<serde_json::Value>) {
    if !is_enhanced_telemetry_enabled() {
        return;
    }

    let event = TelemetryEvent::Feature(FeatureEvent {
        feature: feature.to_string(),
        enabled,
        metadata,
    });

    queue_event(event);
}

/// Track session start
pub fn track_session_start() {
    if !is_enhanced_telemetry_enabled() {
        return;
    }

    let event = TelemetryEvent::Session(SessionEvent {
        session_id: get_session_id(),
        event_type: "start".to_string(),
        start_time: Some(jiff::Timestamp::now().strftime("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()),
        end_time: None,
        commands_run: None,
        duration_secs: None,
    });

    queue_event(event);
}

/// Check if events need to be flushed
#[must_use]
pub fn needs_flush() -> bool {
    if !is_enhanced_telemetry_enabled() {
        return false;
    }

    if let Ok(queue) = get_event_queue().lock() {
        queue.needs_flush()
    } else {
        false
    }
}

/// Flush queued events (call periodically or on exit)
pub async fn flush_events() {
    if !is_enhanced_telemetry_enabled() {
        return;
    }

    let events = {
        if let Ok(mut queue) = get_event_queue().lock() {
            let events = queue.take_events();
            let _ = queue.save();
            events
        } else {
            return;
        }
    };

    if events.is_empty() {
        return;
    }

    tracing::debug!("Flushing {} telemetry events", events.len());

    if let Err(e) = crate::core::telemetry_client::send_batch(events).await {
        tracing::debug!("Failed to flush telemetry events: {e}");
    }
}

/// Flush events in background (fire and forget)
pub fn flush_events_background() {
    if !is_enhanced_telemetry_enabled() {
        return;
    }

    tokio::spawn(async {
        flush_events().await;
    });
}

/// Maybe flush events if needed (call at end of CLI commands)
pub fn maybe_flush_background() {
    if needs_flush() {
        flush_events_background();
    }
}

/// End session and flush all events (call on CLI exit)
pub async fn end_session_and_flush() {
    if !is_enhanced_telemetry_enabled() {
        return;
    }

    // Record session end event
    let (session_id, commands_run, duration_secs) = {
        if let Ok(session) = get_session().lock() {
            (session.session_id.clone(), session.commands_run, session.duration_secs())
        } else {
            return;
        }
    };

    let event = TelemetryEvent::Session(SessionEvent {
        session_id,
        event_type: "end".to_string(),
        start_time: None,
        end_time: Some(jiff::Timestamp::now().strftime("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()),
        commands_run: Some(commands_run),
        duration_secs: Some(duration_secs),
    });

    queue_event(event);

    // Track startup performance if available
    if let Some(startup_ms) = get_startup_duration_ms() {
        track_performance_event("cli_startup", startup_ms, None);
    }

    // Flush all events
    flush_events().await;

    // Also flush retry queue
    if let Err(e) = crate::core::telemetry_client::flush_retry_queue().await {
        tracing::debug!("Failed to flush retry queue: {e}");
    }
}

/// Convenience timer for measuring operation duration
pub struct Timer {
    start: Instant,
    operation: String,
}

impl Timer {
    /// Start a new timer for an operation
    #[must_use]
    pub fn new(operation: &str) -> Self {
        Self {
            start: Instant::now(),
            operation: operation.to_string(),
        }
    }

    /// Get elapsed time in milliseconds
    #[must_use]
    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    /// Finish and record as performance event
    pub fn finish(self) {
        track_performance_event(&self.operation, self.elapsed_ms(), None);
    }

    /// Finish with context
    pub fn finish_with_context(self, context: &str) {
        track_performance_event(&self.operation, self.elapsed_ms(), Some(context));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let session = TelemetrySession::new();
        assert!(!session.session_id.is_empty());
        assert!(!session.started_at.is_empty());
        assert_eq!(session.commands_run, 0);
    }

    #[test]
    fn test_session_expiry() {
        let mut session = TelemetrySession::new();
        // Session should not be expired immediately
        assert!(!session.is_expired());

        // Set last activity to 31 minutes ago
        session.last_activity = jiff::Timestamp::now().as_second() - 1860;
        assert!(session.is_expired());
    }

    #[test]
    fn test_event_queue() {
        // Create a queue with current time for last_flush
        let mut queue = EventQueue {
            events: VecDeque::new(),
            last_flush: jiff::Timestamp::now().as_second(),
        };
        // Queue should not need flush when empty and recently flushed
        assert!(!queue.needs_flush());

        // Add events
        for _ in 0..MAX_QUEUED_EVENTS {
            queue.push(TelemetryEvent::Performance(PerformanceEvent {
                metric_type: "test".to_string(),
                duration_ms: 100,
                context: None,
            }));
        }

        // Queue should need flush when at max capacity
        assert!(queue.needs_flush());

        let events = queue.take_events();
        assert_eq!(events.len(), MAX_QUEUED_EVENTS);
        assert!(queue.events.is_empty());
    }

    #[test]
    fn test_timer() {
        let timer = Timer::new("test_operation");
        std::thread::sleep(std::time::Duration::from_millis(10));
        let elapsed = timer.elapsed_ms();
        assert!(elapsed >= 10);
    }
}
