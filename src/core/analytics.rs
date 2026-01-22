//! Analytics and telemetry for OMG
//!
//! Comprehensive telemetry system for business intelligence:
//! - Session/heartbeat tracking
//! - Feature usage analytics
//! - Error reporting
//! - Performance metrics
//! - Retention signals
//!
//! Privacy-respecting: opt-out via `OMG_TELEMETRY=0`

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Instant;

const ANALYTICS_API_URL: &str = "https://api.pyro1121.com/api/analytics";

/// Format timestamp consistently for analytics
fn format_timestamp(ts: jiff::Timestamp) -> String {
    // Use RFC 3339 format with millisecond precision for consistency
    ts.strftime("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// Parse timestamp from analytics format
fn parse_timestamp(s: &str) -> Result<jiff::Timestamp> {
    // Try parsing with fractional seconds
    if let Ok(ts) = jiff::Timestamp::strptime("%Y-%m-%dT%H:%M:%S%.fZ", s) {
        return Ok(ts);
    }
    // Fallback to parsing without fractional seconds
    jiff::Timestamp::strptime("%Y-%m-%dT%H:%M:%SZ", s)
        .map_err(|e| anyhow::anyhow!("Failed to parse timestamp: {}", e))
}

/// Global session start time
static SESSION_START: OnceLock<Instant> = OnceLock::new();

/// Event types for tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    /// Session started
    SessionStart,
    /// Session heartbeat (every 5 minutes of activity)
    Heartbeat,
    /// Session ended
    SessionEnd,
    /// Command executed
    Command,
    /// Feature used
    Feature,
    /// Error occurred
    Error,
    /// Performance metric
    Performance,
}

/// Analytics event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsEvent {
    /// Event type
    pub event_type: EventType,
    /// Event name (e.g., "search", "install", "`node_use`")
    pub event_name: String,
    /// Event properties
    #[serde(default)]
    pub properties: HashMap<String, serde_json::Value>,
    /// Timestamp (ISO 8601)
    pub timestamp: String,
    /// Session ID (UUID for this CLI session)
    pub session_id: String,
    /// Machine ID
    pub machine_id: String,
    /// License key (if activated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license_key: Option<String>,
    /// OMG version
    pub version: String,
    /// Platform
    pub platform: String,
    /// Duration in milliseconds (for performance events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

/// Session state stored locally
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionState {
    /// Current session ID
    pub session_id: String,
    /// Session start timestamp
    pub started_at: String,
    /// Last heartbeat timestamp
    pub last_heartbeat: i64,
    /// Commands run this session
    pub commands_this_session: u32,
    /// Errors this session
    pub errors_this_session: u32,
    /// Features used this session
    pub features_used: Vec<String>,
}

impl SessionState {
    fn path() -> Result<PathBuf> {
        let data_dir = crate::core::paths::data_dir();
        std::fs::create_dir_all(&data_dir)?;
        Ok(data_dir.join("session.json"))
    }

    pub fn load() -> Self {
        Self::path()
            .ok()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Check if we need a new session (>30 min since last activity)
    pub fn needs_new_session(&self) -> bool {
        if self.session_id.is_empty() {
            return true;
        }
        let now = jiff::Timestamp::now().as_second();
        now - self.last_heartbeat > 1800 // 30 minutes
    }

    /// Start a new session
    pub fn start_new(&mut self) {
        self.session_id = uuid::Uuid::new_v4().to_string();
        self.started_at = format_timestamp(jiff::Timestamp::now());
        self.last_heartbeat = jiff::Timestamp::now().as_second();
        self.commands_this_session = 0;
        self.errors_this_session = 0;
        self.features_used.clear();
        if let Err(e) = self.save() {
            tracing::warn!("Failed to save session state: {}", e);
        }
    }

    /// Update heartbeat
    pub fn heartbeat(&mut self) {
        self.last_heartbeat = jiff::Timestamp::now().as_second();
        if let Err(e) = self.save() {
            tracing::warn!("Failed to save session heartbeat: {}", e);
        }
    }
}

/// Event queue for batching
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventQueue {
    pub events: Vec<AnalyticsEvent>,
    pub last_flush: i64,
}

impl EventQueue {
    fn path() -> Result<PathBuf> {
        let data_dir = crate::core::paths::data_dir();
        std::fs::create_dir_all(&data_dir)?;
        Ok(data_dir.join("event_queue.json"))
    }

    pub fn load() -> Self {
        Self::path()
            .ok()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        let content = serde_json::to_string(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn push(&mut self, event: AnalyticsEvent) {
        self.events.push(event);
        // Keep queue bounded
        if self.events.len() > 1000 {
            self.events.drain(0..500);
        }
        if let Err(e) = self.save() {
            tracing::warn!("Failed to save analytics event queue: {}", e);
        }
    }

    pub fn needs_flush(&self) -> bool {
        let now = jiff::Timestamp::now().as_second();
        // Flush every 60 seconds or if we have 50+ events
        now - self.last_flush > 60 || self.events.len() >= 50
    }

    pub fn take_events(&mut self) -> Vec<AnalyticsEvent> {
        self.last_flush = jiff::Timestamp::now().as_second();
        std::mem::take(&mut self.events)
    }
}

/// Check if analytics is enabled
pub fn is_enabled() -> bool {
    !crate::core::telemetry::is_telemetry_opt_out()
}

/// Get or create session
fn get_session() -> SessionState {
    let mut session = SessionState::load();
    if session.needs_new_session() {
        session.start_new();
        // Queue session start event
        if is_enabled() {
            queue_event(
                EventType::SessionStart,
                "session_start",
                HashMap::new(),
                None,
            );
        }
    }
    session
}

/// Get current session ID
pub fn session_id() -> String {
    get_session().session_id
}

/// Create an analytics event
fn create_event(
    event_type: EventType,
    event_name: &str,
    properties: HashMap<String, serde_json::Value>,
    duration_ms: Option<u64>,
) -> AnalyticsEvent {
    let session = get_session();
    let license = crate::core::license::load_license();

    AnalyticsEvent {
        event_type,
        event_name: event_name.to_string(),
        properties,
        timestamp: format_timestamp(jiff::Timestamp::now()),
        session_id: session.session_id,
        machine_id: crate::core::license::get_machine_id(),
        license_key: license.map(|l| l.key),
        version: env!("CARGO_PKG_VERSION").to_string(),
        platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        duration_ms,
    }
}

/// Queue an event for sending
#[allow(clippy::implicit_hasher)]
pub fn queue_event(
    event_type: EventType,
    event_name: &str,
    properties: HashMap<String, serde_json::Value>,
    duration_ms: Option<u64>,
) {
    if !is_enabled() {
        return;
    }

    let event = create_event(event_type, event_name, properties, duration_ms);
    let mut queue = EventQueue::load();
    queue.push(event);
}

/// Track a command execution
pub fn track_command(command: &str, subcommand: Option<&str>, duration_ms: u64, success: bool) {
    if !is_enabled() || crate::core::paths::test_mode() {
        return;
    }

    let mut props = HashMap::new();
    props.insert("command".to_string(), serde_json::json!(command));
    if let Some(sub) = subcommand {
        props.insert("subcommand".to_string(), serde_json::json!(sub));
    }
    props.insert("success".to_string(), serde_json::json!(success));

    queue_event(EventType::Command, command, props, Some(duration_ms));

    // Update session
    let mut session = SessionState::load();
    session.commands_this_session += 1;
    session.heartbeat();
}

/// Track a feature usage
#[allow(clippy::implicit_hasher)]
pub fn track_feature(feature: &str, properties: HashMap<String, serde_json::Value>) {
    if !is_enabled() {
        return;
    }

    queue_event(EventType::Feature, feature, properties, None);

    // Update session
    let mut session = SessionState::load();
    if !session.features_used.contains(&feature.to_string()) {
        session.features_used.push(feature.to_string());
        let _ = session.save();
    }
}

/// Track an error
pub fn track_error(error_type: &str, message: &str, context: Option<&str>) {
    if !is_enabled() {
        return;
    }

    let mut props = HashMap::new();
    props.insert("error_type".to_string(), serde_json::json!(error_type));
    props.insert("message".to_string(), serde_json::json!(message));
    if let Some(ctx) = context {
        props.insert("context".to_string(), serde_json::json!(ctx));
    }

    queue_event(EventType::Error, error_type, props, None);

    // Update session
    let mut session = SessionState::load();
    session.errors_this_session += 1;
    let _ = session.save();
}

/// Track performance metric
#[allow(clippy::implicit_hasher)]
pub fn track_performance(
    operation: &str,
    duration_ms: u64,
    metadata: HashMap<String, serde_json::Value>,
) {
    if !is_enabled() {
        return;
    }

    queue_event(
        EventType::Performance,
        operation,
        metadata,
        Some(duration_ms),
    );
}

/// Start timing an operation
pub fn start_timer() -> Instant {
    SESSION_START.get_or_init(Instant::now);
    Instant::now()
}

/// End timing and get duration
pub fn end_timer(start: Instant) -> u64 {
    start.elapsed().as_millis() as u64
}

/// Send heartbeat if needed
pub fn maybe_heartbeat() {
    if !is_enabled() {
        return;
    }

    let mut session = SessionState::load();
    let now = jiff::Timestamp::now().as_second();

    // Send heartbeat every 5 minutes of activity
    if now - session.last_heartbeat > 300 {
        let mut props = HashMap::new();
        props.insert(
            "commands_this_session".to_string(),
            serde_json::json!(session.commands_this_session),
        );
        props.insert(
            "features_used".to_string(),
            serde_json::json!(session.features_used),
        );

        queue_event(EventType::Heartbeat, "heartbeat", props, None);
        session.heartbeat();
    }
}

/// Flush event queue to API
pub async fn flush_events() -> Result<()> {
    if !is_enabled() {
        return Ok(());
    }

    let mut queue = EventQueue::load();
    if queue.events.is_empty() {
        return Ok(());
    }

    let events = queue.take_events();
    let _ = queue.save();

    // Send batch to API
    let client = reqwest::Client::new();
    let res = client
        .post(ANALYTICS_API_URL)
        .json(&serde_json::json!({ "events": events }))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await;

    match res {
        Ok(resp) if resp.status().is_success() => {
            tracing::debug!("Flushed {} analytics events", events.len());
        }
        _ => {
            // Re-queue on failure for "Gold Tier" reliability
            let mut queue = EventQueue::load();
            for event in events {
                queue.push(event);
            }
        }
    }

    Ok(())
}

/// Flush events if queue is ready
pub async fn maybe_flush() {
    if !is_enabled() || crate::core::paths::test_mode() {
        return;
    }

    let queue = EventQueue::load();
    if queue.needs_flush() {
        let _ = flush_events().await;
    }
}

/// Flush events in background
pub fn flush_background() {
    if !is_enabled() {
        return;
    }

    tokio::spawn(async {
        let _ = flush_events().await;
    });
}

/// End session (call on CLI exit)
pub fn end_session() {
    if !is_enabled() {
        return;
    }

    let session = SessionState::load();
    let mut props = HashMap::new();
    props.insert(
        "commands_run".to_string(),
        serde_json::json!(session.commands_this_session),
    );
    props.insert(
        "errors".to_string(),
        serde_json::json!(session.errors_this_session),
    );
    props.insert(
        "features_used".to_string(),
        serde_json::json!(session.features_used),
    );

    // Calculate session duration
    if let Ok(started) = parse_timestamp(&session.started_at) {
        let duration = jiff::Timestamp::now().as_second() - started.as_second();
        props.insert("duration_seconds".to_string(), serde_json::json!(duration));
    }

    queue_event(EventType::SessionEnd, "session_end", props, None);
}

/// Aggregate stats for admin dashboard
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AggregateStats {
    /// Total sessions today
    pub sessions_today: u32,
    /// Active users today (unique machines)
    pub active_users_today: u32,
    /// Commands by type
    pub commands_by_type: HashMap<String, u64>,
    /// Features by usage
    pub features_by_usage: HashMap<String, u64>,
    /// Errors by type
    pub errors_by_type: HashMap<String, u64>,
    /// Average session duration (seconds)
    pub avg_session_duration: u64,
    /// Average commands per session
    pub avg_commands_per_session: f64,
    /// Performance percentiles (p50, p95, p99)
    pub performance_percentiles: HashMap<String, PerformanceStats>,
    /// Retention (users active in last 7 days who were active 7-14 days ago)
    pub retention_7d: f64,
    /// Churn signals (users inactive for 7+ days)
    pub churned_users: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PerformanceStats {
    pub p50_ms: u64,
    pub p95_ms: u64,
    pub p99_ms: u64,
    pub count: u64,
}

/// User journey stages for funnel tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserStage {
    /// Just installed
    Installed,
    /// Activated license
    Activated,
    /// Ran first command
    FirstCommand,
    /// Used 3+ different commands
    Exploring,
    /// 7+ days of usage
    Engaged,
    /// 30+ days, 100+ commands
    PowerUser,
    /// Inactive 14+ days
    AtRisk,
    /// Inactive 30+ days
    Churned,
}

impl UserStage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Installed => "installed",
            Self::Activated => "activated",
            Self::FirstCommand => "first_command",
            Self::Exploring => "exploring",
            Self::Engaged => "engaged",
            Self::PowerUser => "power_user",
            Self::AtRisk => "at_risk",
            Self::Churned => "churned",
        }
    }
}

/// Track user journey stage transitions
pub fn track_stage_transition(from: Option<UserStage>, to: UserStage) {
    if !is_enabled() {
        return;
    }

    let mut props = HashMap::new();
    if let Some(from_stage) = from {
        props.insert(
            "from_stage".to_string(),
            serde_json::json!(from_stage.as_str()),
        );
    }
    props.insert("to_stage".to_string(), serde_json::json!(to.as_str()));

    queue_event(EventType::Feature, "stage_transition", props, None);
}

/// Calculate current user stage based on usage
pub fn calculate_user_stage(stats: &super::usage::UsageStats) -> UserStage {
    let days_since_first_use = if stats.first_use_date.is_empty() {
        0
    } else if let Ok(first_date) = jiff::civil::Date::strptime("%Y-%m-%d", &stats.first_use_date) {
        let today = jiff::Zoned::now().date();
        (today - first_date).get_days().max(0) as u32
    } else {
        0
    };

    let days_since_last_use = if stats.last_query_date.is_empty() {
        999
    } else if let Ok(last_date) = jiff::civil::Date::strptime("%Y-%m-%d", &stats.last_query_date) {
        let today = jiff::Zoned::now().date();
        (today - last_date).get_days().max(0) as u32
    } else {
        999
    };

    let unique_commands = stats.commands.len();

    // Churn detection
    if days_since_last_use >= 30 {
        return UserStage::Churned;
    }
    if days_since_last_use >= 14 {
        return UserStage::AtRisk;
    }

    // Progression
    if stats.total_commands >= 100 && days_since_first_use >= 30 {
        return UserStage::PowerUser;
    }
    if days_since_first_use >= 7 && stats.total_commands >= 20 {
        return UserStage::Engaged;
    }
    if unique_commands >= 3 {
        return UserStage::Exploring;
    }
    if stats.total_commands >= 1 {
        return UserStage::FirstCommand;
    }

    // Check if license is activated
    if crate::core::license::load_license().is_some() {
        return UserStage::Activated;
    }

    UserStage::Installed
}

/// Track geographic info (country only, privacy-respecting)
pub fn track_geo_info() {
    if !is_enabled() {
        return;
    }

    // Get timezone as a proxy for region (no IP lookup needed)
    let tz = std::env::var("TZ").unwrap_or_else(|_| "Unknown".to_string());
    let locale = std::env::var("LANG").unwrap_or_else(|_| "en_US".to_string());

    let mut props = HashMap::new();
    props.insert("timezone".to_string(), serde_json::json!(tz));
    props.insert(
        "locale".to_string(),
        serde_json::json!(locale.split('.').next().unwrap_or("en_US")),
    );

    queue_event(EventType::Feature, "geo_info", props, None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_needs_new() {
        let mut session = SessionState::default();
        assert!(session.needs_new_session());

        session.start_new();
        assert!(!session.needs_new_session());
    }

    #[test]
    fn test_event_queue() {
        let mut queue = EventQueue {
            events: Vec::new(),
            last_flush: jiff::Timestamp::now().as_second(), // Set to now to avoid time-based flush
        };
        assert!(!queue.needs_flush());

        // Add events
        for i in 0..50 {
            queue.events.push(AnalyticsEvent {
                event_type: EventType::Command,
                event_name: format!("test_{i}"),
                properties: HashMap::new(),
                timestamp: String::new(),
                session_id: String::new(),
                machine_id: String::new(),
                license_key: None,
                version: String::new(),
                platform: String::new(),
                duration_ms: None,
            });
        }

        assert!(queue.needs_flush());
    }
}
