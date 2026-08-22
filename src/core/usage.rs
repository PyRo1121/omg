//! Usage tracking for OMG
//!
//! Tracks command usage, time saved, and syncs with the API for dashboard display.
//! Works for all tiers (free included) when a license is activated.
//!
//! ## Timing Integration
//!
//! This module now integrates with the telemetry system to track:
//! - Operation durations (install, search, update, remove)
//! - Performance metrics
//! - Feature usage

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const USAGE_SYNC_API: &str = "https://api.pyro1121.com/api/report-usage";

/// Time saved per operation (in milliseconds)
/// Based on benchmark comparisons vs traditional tools
pub mod time_saved {
    /// Search: OMG 6ms vs pacman 133ms = 127ms saved
    pub const SEARCH_MS: u64 = 127;
    /// Info: OMG 6.5ms vs pacman 138ms = 131.5ms saved
    pub const INFO_MS: u64 = 132;
    /// Explicit: OMG 1.2ms vs pacman 14ms = 12.8ms saved
    pub const EXPLICIT_MS: u64 = 13;
    /// Status: OMG 1ms vs pacman 50ms = 49ms saved
    pub const STATUS_MS: u64 = 49;
    /// Runtime switch: OMG 1.8ms vs nvm 150ms = 148.2ms saved
    pub const RUNTIME_SWITCH_MS: u64 = 148;
    /// Install: OMG parallel vs sequential = ~30% time saved (estimated 5s per package)
    pub const INSTALL_MS: u64 = 1500;
    /// Update: no dedicated benchmark yet; conservatively estimated at the
    /// same per-package saving as install.
    pub const UPDATE_MS: u64 = INSTALL_MS;
    /// Remove: no dedicated benchmark yet; conservatively estimated at the
    /// same per-package saving as install.
    pub const REMOVE_MS: u64 = INSTALL_MS;
}

/// Achievement definitions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Achievement {
    /// First command executed
    FirstStep,
    /// 100 commands executed
    Centurion,
    /// 1,000 commands executed
    PowerUser,
    /// 10,000 commands executed
    Legend,
    /// 1 minute saved
    MinuteSaver,
    /// 1 hour saved
    HourSaver,
    /// 1 day saved (24 hours)
    DaySaver,
    /// 7-day usage streak
    WeekStreak,
    /// 30-day usage streak
    MonthStreak,
    /// Used all 7 runtimes
    Polyglot,
    /// First SBOM generated
    SecurityFirst,
    /// Found and fixed vulnerabilities
    BugHunter,
}

impl Achievement {
    #[must_use]
    pub fn emoji(&self) -> &'static str {
        match self {
            Self::FirstStep => "🚀",
            Self::Centurion => "💯",
            Self::PowerUser => "⚡",
            Self::Legend => "🏆",
            Self::MinuteSaver => "⏱️",
            Self::HourSaver => "⏰",
            Self::DaySaver => "📅",
            Self::WeekStreak => "🔥",
            Self::MonthStreak => "💎",
            Self::Polyglot => "🌐",
            Self::SecurityFirst => "🛡️",
            Self::BugHunter => "🐛",
        }
    }

    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::FirstStep => "First Step",
            Self::Centurion => "Centurion",
            Self::PowerUser => "Power User",
            Self::Legend => "Legend",
            Self::MinuteSaver => "Minute Saver",
            Self::HourSaver => "Hour Saver",
            Self::DaySaver => "Day Saver",
            Self::WeekStreak => "Week Streak",
            Self::MonthStreak => "Month Streak",
            Self::Polyglot => "Polyglot",
            Self::SecurityFirst => "Security First",
            Self::BugHunter => "Bug Hunter",
        }
    }

    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            Self::FirstStep => "Executed your first command",
            Self::Centurion => "Executed 100 commands",
            Self::PowerUser => "Executed 1,000 commands",
            Self::Legend => "Executed 10,000 commands",
            Self::MinuteSaver => "Saved 1 minute of time",
            Self::HourSaver => "Saved 1 hour of time",
            Self::DaySaver => "Saved 24 hours of time",
            Self::WeekStreak => "Used OMG for 7 days straight",
            Self::MonthStreak => "Used OMG for 30 days straight",
            Self::Polyglot => "Used all 7 built-in runtimes",
            Self::SecurityFirst => "Generated your first SBOM",
            Self::BugHunter => "Found and addressed vulnerabilities",
        }
    }
}

/// Usage statistics stored locally
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageStats {
    /// Total commands executed
    pub total_commands: u64,
    /// Commands by type
    pub commands: HashMap<String, u64>,
    /// Total time saved in milliseconds
    pub time_saved_ms: u64,
    /// Queries today (resets daily)
    pub queries_today: u64,
    /// Queries this month
    pub queries_this_month: u64,
    /// Last query date (YYYY-MM-DD)
    pub last_query_date: String,
    /// Last month (YYYY-MM)
    pub last_month: String,
    /// SBOMs generated (Pro+)
    #[serde(default)]
    pub sbom_generated: u64,
    /// Vulnerabilities found (Pro+)
    #[serde(default)]
    pub vulnerabilities_found: u64,
    /// Last sync timestamp
    pub last_sync: i64,
    /// Current streak (consecutive days)
    #[serde(default)]
    pub current_streak: u32,
    /// Longest streak ever
    #[serde(default)]
    pub longest_streak: u32,
    /// Unlocked achievements
    #[serde(default)]
    pub achievements: Vec<Achievement>,
    /// Runtimes used (for Polyglot achievement)
    #[serde(default)]
    pub runtimes_used: Vec<String>,
    /// Installed packages (for Global Insights)
    #[serde(default)]
    pub installed_packages: HashMap<String, u64>,
    /// Runtime usage counts (for Global Insights)
    #[serde(default)]
    pub runtime_usage_counts: HashMap<String, u64>,
    /// First use date
    #[serde(default)]
    pub first_use_date: String,
    /// Daily installs (resets daily, same as `queries_today`)
    #[serde(default)]
    pub installs_today: u64,
    /// Daily searches (resets daily)
    #[serde(default)]
    pub searches_today: u64,
    /// Daily runtime switches (resets daily)
    #[serde(default)]
    pub runtimes_today: u64,
    /// Daily time saved in ms (resets daily)
    #[serde(default)]
    pub time_saved_today_ms: u64,
}

#[derive(Clone, Copy)]
enum DailyOperation {
    Install,
    Search,
    RuntimeSwitch,
}

impl UsageStats {
    /// Get the usage stats file path
    fn path() -> Result<PathBuf> {
        let data_dir = crate::core::paths::data_dir();
        std::fs::create_dir_all(&data_dir)?;
        Ok(data_dir.join("usage.json"))
    }

    /// Load usage stats from disk.
    pub fn load() -> Result<Self> {
        Self::load_from(&Self::path()?)
    }

    fn load_from(path: &std::path::Path) -> Result<Self> {
        let content = match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Failed to read usage stats: {}", path.display()));
            }
        };
        serde_json::from_str(&content)
            .with_context(|| format!("Malformed usage stats: {}", path.display()))
    }

    /// Save usage stats to disk.
    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        self.save_to(&path)
            .with_context(|| format!("Failed to save usage stats: {}", path.display()))
    }

    fn save_to(&self, path: &std::path::Path) -> Result<()> {
        let content = serde_json::to_vec_pretty(self).context("Failed to serialize usage stats")?;
        crate::core::safe_ops::atomic_write_file_sync(path, content)
    }

    /// Record a command execution.
    pub fn record_command(&mut self, command: &str, time_saved_ms: u64) {
        self.record_command_on(command, time_saved_ms, jiff::Zoned::now().date());
        self.save_best_effort();
    }

    fn record_specialized_command(
        &mut self,
        command: &str,
        time_saved_ms: u64,
        operation: DailyOperation,
    ) {
        self.record_specialized_command_on(
            command,
            time_saved_ms,
            operation,
            jiff::Zoned::now().date(),
        );
        self.save_best_effort();
    }

    fn record_specialized_command_on(
        &mut self,
        command: &str,
        time_saved_ms: u64,
        operation: DailyOperation,
        today: jiff::civil::Date,
    ) {
        self.rollover_for(today);
        match operation {
            DailyOperation::Install => self.installs_today += 1,
            DailyOperation::Search => self.searches_today += 1,
            DailyOperation::RuntimeSwitch => self.runtimes_today += 1,
        }
        self.record_command_on(command, time_saved_ms, today);
    }

    fn record_command_on(&mut self, command: &str, time_saved_ms: u64, today: jiff::civil::Date) {
        self.rollover_for(today);
        self.total_commands += 1;
        self.time_saved_ms += time_saved_ms;
        *self.commands.entry(command.to_string()).or_insert(0) += 1;
        self.queries_today += 1;
        self.time_saved_today_ms += time_saved_ms;
        self.queries_this_month += 1;
        self.check_achievements();
    }

    fn save_best_effort(&self) {
        if let Err(error) = self.save() {
            tracing::warn!("Failed to save usage stats: {error}");
        }
    }

    fn rollover_for(&mut self, today: jiff::civil::Date) {
        let today_string = today.to_string();
        let month = today_string[..7].to_string();

        if self.first_use_date.is_empty() {
            self.first_use_date.clone_from(&today_string);
        }

        if self.last_query_date != today_string {
            if let Ok(last_date) = jiff::civil::Date::strptime("%Y-%m-%d", &self.last_query_date) {
                let elapsed_days = (today - last_date).get_days();
                if elapsed_days == 1 {
                    self.current_streak += 1;
                } else if elapsed_days > 1 {
                    self.current_streak = 1;
                }
            } else {
                self.current_streak = 1;
            }
            self.longest_streak = self.longest_streak.max(self.current_streak);
            self.queries_today = 0;
            self.installs_today = 0;
            self.searches_today = 0;
            self.runtimes_today = 0;
            self.time_saved_today_ms = 0;
            self.last_query_date = today_string;
        }

        if self.last_month != month {
            self.queries_this_month = 0;
            self.last_month = month;
        }
    }

    /// Check and unlock achievements
    fn check_achievements(&mut self) {
        let mut new_achievements = Vec::new();

        // Command milestones
        if self.total_commands >= 1 && !self.achievements.contains(&Achievement::FirstStep) {
            new_achievements.push(Achievement::FirstStep);
        }
        if self.total_commands >= 100 && !self.achievements.contains(&Achievement::Centurion) {
            new_achievements.push(Achievement::Centurion);
        }
        if self.total_commands >= 1000 && !self.achievements.contains(&Achievement::PowerUser) {
            new_achievements.push(Achievement::PowerUser);
        }
        if self.total_commands >= 10000 && !self.achievements.contains(&Achievement::Legend) {
            new_achievements.push(Achievement::Legend);
        }

        // Time saved milestones
        if self.time_saved_ms >= 60_000 && !self.achievements.contains(&Achievement::MinuteSaver) {
            new_achievements.push(Achievement::MinuteSaver);
        }
        if self.time_saved_ms >= 3_600_000 && !self.achievements.contains(&Achievement::HourSaver) {
            new_achievements.push(Achievement::HourSaver);
        }
        if self.time_saved_ms >= 86_400_000 && !self.achievements.contains(&Achievement::DaySaver) {
            new_achievements.push(Achievement::DaySaver);
        }

        // Streak milestones
        if self.current_streak >= 7 && !self.achievements.contains(&Achievement::WeekStreak) {
            new_achievements.push(Achievement::WeekStreak);
        }
        if self.current_streak >= 30 && !self.achievements.contains(&Achievement::MonthStreak) {
            new_achievements.push(Achievement::MonthStreak);
        }

        // Security milestones
        if self.sbom_generated >= 1 && !self.achievements.contains(&Achievement::SecurityFirst) {
            new_achievements.push(Achievement::SecurityFirst);
        }
        if self.vulnerabilities_found >= 1 && !self.achievements.contains(&Achievement::BugHunter) {
            new_achievements.push(Achievement::BugHunter);
        }

        // Polyglot (7 runtimes)
        if self.runtimes_used.len() >= 7 && !self.achievements.contains(&Achievement::Polyglot) {
            new_achievements.push(Achievement::Polyglot);
        }

        // Add new achievements
        for achievement in new_achievements {
            self.achievements.push(achievement);
        }
    }

    /// Record runtime usage (for Polyglot achievement)
    pub fn record_runtime(&mut self, runtime: &str) {
        let runtime_lower = runtime.to_lowercase();
        if !self.runtimes_used.contains(&runtime_lower) {
            self.runtimes_used.push(runtime_lower);
            self.check_achievements();
            self.save_best_effort();
        }
    }

    /// Record SBOM generation
    pub fn record_sbom(&mut self) {
        self.sbom_generated += 1;
        self.save_best_effort();
    }

    /// Record vulnerabilities found
    pub fn record_vulnerabilities(&mut self, count: u64) {
        self.vulnerabilities_found += count;
        self.save_best_effort();
    }

    /// Get time saved as human-readable string
    #[must_use]
    pub fn time_saved_human(&self) -> String {
        let ms = self.time_saved_ms;
        if ms < 1000 {
            format!("{ms}ms")
        } else if ms < 60_000 {
            format!("{:.1}s", ms as f64 / 1000.0)
        } else if ms < 3_600_000 {
            format!("{:.1}min", ms as f64 / 60_000.0)
        } else {
            format!("{:.1}hr", ms as f64 / 3_600_000.0)
        }
    }

    /// Get most used commands (top 5)
    #[must_use]
    pub fn top_commands(&self) -> Vec<(String, u64)> {
        let mut sorted: Vec<_> = self.commands.iter().map(|(k, v)| (k.clone(), *v)).collect();
        sorted.sort_by_key(|&(_, count)| std::cmp::Reverse(count));
        sorted.truncate(5);
        sorted
    }

    /// Check if sync is needed (every 30 seconds for real-time dashboard)
    #[must_use]
    pub fn needs_sync(&self) -> bool {
        let now = jiff::Timestamp::now().as_second();
        now - self.last_sync > 30 // 30 seconds for near real-time updates
    }

    /// Check if immediate sync is needed (for important events)
    #[must_use]
    pub fn needs_immediate_sync(&self) -> bool {
        // Sync immediately after first command, achievements, or milestones.
        // An empty stats file must never trigger a sync.
        if self.total_commands == 0 {
            return false;
        }
        self.total_commands == 1
            || self.total_commands.is_multiple_of(100)
            || (self.time_saved_ms >= 60_000 && self.last_sync == 0)
    }

    /// Sync usage stats to API (async)
    pub async fn sync(&mut self, license_key: &str) -> Result<()> {
        // Get machine info for richer telemetry
        let machine_id = crate::core::license::get_machine_id();

        // Hash hostname for privacy (SHA-256) - do not send in plaintext
        let hostname_hash =
            if let Ok(hostname_raw) = tokio::fs::read_to_string("/etc/hostname").await {
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(hostname_raw.trim().as_bytes());
                let result = hasher.finalize();
                Some(hex::encode(result)[..16].to_string()) // First 16 chars of hash
            } else {
                None
            };

        // Send TODAY's values only (server replaces, not adds)
        let payload = serde_json::json!({
            "license_key": license_key,
            "machine_id": machine_id,
            "hostname": hostname_hash, // Hashed, not plaintext
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "omg_version": env!("CARGO_PKG_VERSION"),
            "commands_run": self.queries_today,
            "packages_installed": self.installs_today,
            "packages_searched": self.searches_today,
            "runtimes_switched": self.runtimes_today,
            "installed_packages": self.installed_packages,
            "runtime_usage_counts": self.runtime_usage_counts,
            "sbom_generated": self.sbom_generated,
            "vulnerabilities_found": self.vulnerabilities_found,
            "time_saved_ms": self.time_saved_today_ms,
            "current_streak": self.current_streak,
            "achievements": self.achievements,
        });

        let client = crate::core::http::shared_client();
        client
            .post(USAGE_SYNC_API)
            .json(&payload)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?
            .error_for_status()
            .context("Usage sync server rejected the update")?;

        self.last_sync = jiff::Timestamp::now().as_second();
        // Merge only the sync timestamp into freshly-loaded state under the
        // cross-process lock: writing back this task's snapshot would clobber
        // counters recorded by concurrent invocations while the network
        // request was in flight.
        let synced_at = self.last_sync;
        update_locked(&Self::path()?, |stats| stats.last_sync = synced_at)?;

        Ok(())
    }
}

fn load_for_tracking() -> Option<UsageStats> {
    match UsageStats::load() {
        Ok(stats) => Some(stats),
        Err(error) => {
            tracing::warn!("Skipping usage tracking because persisted stats are invalid: {error}");
            None
        }
    }
}

/// Acquire the cross-process usage lock (`usage.json.lock`) so a full
/// load-modify-save cycle cannot interleave with another omg invocation
/// (which would silently lose counters to last-writer-wins). The lock is
/// released when the returned file is dropped. Same pattern as
/// [`crate::core::history::HistoryManager`].
fn lock_usage_file() -> Option<std::fs::File> {
    lock_file_at(&UsageStats::path().ok()?.with_extension("lock"))
}

fn lock_file_at(lock_path: &std::path::Path) -> Option<std::fs::File> {
    let lock = match std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(lock_path)
    {
        Ok(lock) => lock,
        Err(error) => {
            tracing::warn!(
                "Failed to open usage lock {}: skipping locked update ({error})",
                lock_path.display()
            );
            return None;
        }
    };
    if let Err(error) = lock.lock() {
        tracing::warn!(
            "Failed to lock usage stats {}: {error}",
            lock_path.display()
        );
        return None;
    }
    Some(lock)
}

/// Run a load-modify-save cycle while holding the usage file lock.
fn with_usage_lock<T>(mutate: impl FnOnce() -> T) -> T {
    let _lock = lock_usage_file();
    mutate()
}

/// Run a mandatory load-modify-save cycle against `path` under the
/// cross-process usage lock. Unlike [`with_usage_lock`] (used by fire-and-
/// forget tracking), a failed lock acquisition is an error here because the
/// caller is about to write persisted state and must not race a concurrent
/// writer.
fn update_locked<T>(path: &Path, f: impl FnOnce(&mut UsageStats) -> T) -> Result<T> {
    let lock_path = path.with_extension("lock");
    let lock = lock_file_at(&lock_path).ok_or_else(|| {
        anyhow::anyhow!("Failed to acquire usage stats lock {}", lock_path.display())
    })?;
    let _lock_guard = lock;
    let mut stats = UsageStats::load_from(path)?;
    let out = f(&mut stats);
    stats.save_to(path)?;
    Ok(out)
}

/// Track a command execution (convenience function)
pub fn track(command: &str, time_saved_ms: u64) {
    with_usage_lock(|| {
        let Some(mut stats) = load_for_tracking() else {
            return;
        };
        stats.record_command(command, time_saved_ms);
    });
}

/// Track search command
pub fn track_search() {
    with_usage_lock(|| {
        let Some(mut stats) = load_for_tracking() else {
            return;
        };
        stats.record_specialized_command("search", time_saved::SEARCH_MS, DailyOperation::Search);
    });
}

/// Track info command
pub fn track_info() {
    track("info", time_saved::INFO_MS);
}

/// Track explicit command
pub fn track_explicit() {
    track("explicit", time_saved::EXPLICIT_MS);
}

/// Track status command
pub fn track_status() {
    track("status", time_saved::STATUS_MS);
}

/// Track install command
pub fn track_install(packages: &[String]) {
    with_usage_lock(|| {
        let Some(mut stats) = load_for_tracking() else {
            return;
        };

        for pkg in packages {
            *stats.installed_packages.entry(pkg.clone()).or_insert(0) += 1;
        }

        stats.record_specialized_command(
            "install",
            time_saved::INSTALL_MS,
            DailyOperation::Install,
        );
    });
}

/// Track runtime switch
pub fn track_runtime_switch(runtime: &str) {
    with_usage_lock(|| {
        let Some(mut stats) = load_for_tracking() else {
            return;
        };

        *stats
            .runtime_usage_counts
            .entry(runtime.to_string())
            .or_insert(0) += 1;

        stats.record_runtime(runtime);

        stats.record_specialized_command(
            "runtime_switch",
            time_saved::RUNTIME_SWITCH_MS,
            DailyOperation::RuntimeSwitch,
        );
    });
}

// =============================================================================
// Timing and Telemetry Integration
// =============================================================================

/// Operation timer for tracking command durations
pub struct OperationTimer {
    start: Instant,
    command: String,
    packages: Option<Vec<String>>,
}

impl OperationTimer {
    /// Start timing an operation
    #[must_use]
    pub fn start(command: &str) -> Self {
        Self {
            start: Instant::now(),
            command: command.to_string(),
            packages: None,
        }
    }

    /// Start timing with package names
    #[must_use]
    pub fn start_with_packages(command: &str, packages: &[String]) -> Self {
        Self {
            start: Instant::now(),
            command: command.to_string(),
            packages: Some(packages.to_vec()),
        }
    }

    /// Get elapsed time in milliseconds
    #[must_use]
    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    /// Finish timing and record success
    pub fn finish_success(self) {
        let duration_ms = self.elapsed_ms();

        // Record to telemetry
        crate::core::telemetry::track_command_event(
            &self.command,
            None,
            self.packages.as_deref(),
            duration_ms,
            true,
            None,
        );

        // Also flush telemetry if needed
        crate::core::telemetry::maybe_flush_background();
    }

    /// Finish timing and record failure
    pub fn finish_error(self, error: &str) {
        let duration_ms = self.elapsed_ms();

        // Record to telemetry
        crate::core::telemetry::track_command_event(
            &self.command,
            None,
            self.packages.as_deref(),
            duration_ms,
            false,
            Some(error),
        );

        // Also flush telemetry if needed
        crate::core::telemetry::maybe_flush_background();
    }
}

/// Track install command with timing
pub fn track_install_timed(
    packages: &[String],
    duration_ms: u64,
    success: bool,
    error: Option<&str>,
) {
    // Record to usage stats
    if success {
        track_install(packages);
    }

    // Record to telemetry
    crate::core::telemetry::track_command_event(
        "install",
        None,
        Some(packages),
        duration_ms,
        success,
        error,
    );

    maybe_sync_background();
}

/// Track search command with timing and result count
pub fn track_search_timed(query: &str, result_count: usize, duration_ms: u64, success: bool) {
    // Record to usage stats
    if success {
        track_search();
    }

    // Record to telemetry
    crate::core::telemetry::track_search_event(query, result_count, duration_ms, success);

    maybe_sync_background();
}

/// Track update command with timing
pub fn track_update_timed(
    updated_count: usize,
    duration_ms: u64,
    success: bool,
    error: Option<&str>,
) {
    // Record to usage stats
    if success {
        track(
            "update",
            time_saved::UPDATE_MS.saturating_mul(u64::try_from(updated_count).unwrap_or(0)),
        );
    }

    // Record to telemetry
    crate::core::telemetry::track_update_event(updated_count, duration_ms, success, error);

    maybe_sync_background();
}

/// Track remove command with timing
pub fn track_remove_timed(
    packages: &[String],
    duration_ms: u64,
    success: bool,
    error: Option<&str>,
) {
    // Record to usage stats
    if success {
        track("remove", time_saved::REMOVE_MS);
    }

    // Record to telemetry
    crate::core::telemetry::track_command_event(
        "remove",
        None,
        Some(packages),
        duration_ms,
        success,
        error,
    );

    maybe_sync_background();
}

/// Track feature usage (daemon, parallel, sbom, fleet, aur)
pub fn track_feature_usage(feature: &str, enabled: bool) {
    crate::core::telemetry::track_feature_event(feature, enabled, None);
}

/// Track feature usage with metadata
pub fn track_feature_usage_with_metadata(
    feature: &str,
    enabled: bool,
    metadata: serde_json::Value,
) {
    crate::core::telemetry::track_feature_event(feature, enabled, Some(metadata));
}

/// Load the stored license only when its token is valid, mirroring the
/// enhanced-telemetry gate so expired or unverifiable licenses stop syncing.
fn licensed_for_sync() -> Option<crate::core::license::StoredLicense> {
    crate::core::license::load_license().filter(super::license::StoredLicense::is_token_valid)
}

/// Sync usage in background if needed
pub fn maybe_sync_background() {
    // Only sync if we have a valid license and persisted usage state.
    if crate::core::paths::test_mode() {
        return;
    }
    let Some(license) = licensed_for_sync() else {
        return;
    };
    let Some(mut stats) = load_for_tracking() else {
        return;
    };
    if stats.needs_sync() || stats.needs_immediate_sync() {
        tokio::spawn(async move {
            if let Err(e) = stats.sync(&license.key).await {
                tracing::debug!("Usage sync failed: {e}");
            }
        });
    }
}

/// Force immediate sync (for important events like achievements)
pub fn force_sync_background() {
    if crate::core::paths::test_mode() {
        return;
    }
    let Some(license) = licensed_for_sync() else {
        return;
    };
    let Some(mut stats) = load_for_tracking() else {
        return;
    };
    tokio::spawn(async move {
        if let Err(e) = stats.sync(&license.key).await {
            tracing::debug!("Force sync failed: {e}");
        }
    });
}

/// Track and sync immediately (for real-time dashboard updates)
pub fn track_and_sync(command: &str, time_saved_ms: u64) {
    with_usage_lock(|| {
        let Some(mut stats) = load_for_tracking() else {
            return;
        };
        stats.record_command(command, time_saved_ms);
    });
    maybe_sync_background();
}

/// Sync usage now (awaitable, for end of CLI commands)
pub async fn sync_usage_now() {
    if crate::core::paths::test_mode() {
        return;
    }
    let Some(license) = licensed_for_sync() else {
        return;
    };
    let Some(mut stats) = load_for_tracking() else {
        return;
    };
    if let Err(e) = stats.sync(&license.key).await {
        tracing::debug!("Usage sync failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_saved_human_formats_units() {
        let stats = UsageStats {
            time_saved_ms: 500,
            ..Default::default()
        };
        assert_eq!(stats.time_saved_human(), "500ms");

        let stats = UsageStats {
            time_saved_ms: 5000,
            ..Default::default()
        };
        assert_eq!(stats.time_saved_human(), "5.0s");

        let stats = UsageStats {
            time_saved_ms: 120_000,
            ..Default::default()
        };
        assert_eq!(stats.time_saved_human(), "2.0min");

        let stats = UsageStats {
            time_saved_ms: 7_200_000,
            ..Default::default()
        };
        assert_eq!(stats.time_saved_human(), "2.0hr");
    }

    #[test]
    fn first_specialized_operation_after_daily_rollover_is_counted() {
        let today =
            jiff::civil::Date::strptime("%Y-%m-%d", "2025-04-10").expect("valid fixture date");
        let mut stats = UsageStats {
            last_query_date: "2025-04-09".to_string(),
            last_month: "2025-04".to_string(),
            searches_today: 9,
            ..Default::default()
        };

        stats.record_specialized_command_on(
            "search",
            time_saved::SEARCH_MS,
            DailyOperation::Search,
            today,
        );

        assert_eq!(stats.searches_today, 1);
        assert_eq!(stats.queries_today, 1);
        assert_eq!(stats.commands.get("search"), Some(&1));
    }

    #[test]
    fn malformed_usage_stats_are_rejected_without_modification() {
        let directory = tempfile::tempdir().expect("create temporary directory");
        let path = directory.path().join("usage.json");
        std::fs::write(&path, b"{not-json").expect("write malformed fixture");

        let error = UsageStats::load_from(&path).expect_err("malformed stats must be rejected");

        assert!(error.to_string().contains("Malformed usage stats"));
        assert_eq!(std::fs::read(&path).expect("read fixture"), b"{not-json");
    }

    #[test]
    fn missing_usage_stats_load_as_empty() {
        let directory = tempfile::tempdir().expect("create temporary directory");
        let stats = UsageStats::load_from(&directory.path().join("missing.json"))
            .expect("missing stats are the initial state");
        assert_eq!(stats.total_commands, 0);
    }

    #[test]
    fn concurrent_locked_updates_do_not_lose_counters() {
        // Regression: usage.json load-modify-save cycles used to run without
        // a cross-process lock, so concurrent omg invocations clobbered each
        // other's counters (last-writer-wins).
        const WRITERS: usize = 8;
        const UPDATES_PER_WRITER: usize = 5;
        let directory = tempfile::tempdir().expect("create temporary directory");
        let path = directory.path().join("usage.json");

        let barrier = std::sync::Arc::new(std::sync::Barrier::new(WRITERS));
        let mut writers = Vec::new();
        for writer_index in 0..WRITERS {
            let path = path.clone();
            let barrier = std::sync::Arc::clone(&barrier);
            writers.push(std::thread::spawn(move || {
                barrier.wait();
                for _ in 0..UPDATES_PER_WRITER {
                    // Same lock + load-modify-save shape as the public track* functions.
                    let _lock = lock_file_at(&path.with_extension("lock"));
                    let mut stats = UsageStats::load_from(&path).expect("valid usage stats");
                    stats.record_command_on(
                        "search",
                        1,
                        jiff::civil::Date::strptime("%Y-%m-%d", "2025-04-10")
                            .expect("valid fixture date"),
                    );
                    stats.save_to(&path).expect("locked save must succeed");
                }
                writer_index
            }));
        }
        for writer in writers {
            writer.join().expect("usage writer panicked");
        }

        let stats = UsageStats::load_from(&path).expect("final stats must be valid");
        assert_eq!(stats.total_commands, (WRITERS * UPDATES_PER_WRITER) as u64);
    }

    #[test]
    fn record_command_accumulates_counts() {
        let today =
            jiff::civil::Date::strptime("%Y-%m-%d", "2025-04-10").expect("valid fixture date");
        let mut stats = UsageStats::default();
        stats.record_command_on("search", 127, today);
        stats.record_command_on("search", 127, today);
        stats.record_command_on("info", 132, today);

        assert_eq!(stats.total_commands, 3);
        assert_eq!(stats.commands.get("search"), Some(&2));
        assert_eq!(stats.commands.get("info"), Some(&1));
        assert_eq!(stats.time_saved_ms, 127 + 127 + 132);
    }

    #[test]
    fn locked_sync_timestamp_merge_does_not_lose_concurrent_counters() {
        // Regression for the background-sync race: UsageStats::sync used to
        // write its stale snapshot back without the cross-process lock,
        // clobbering counters recorded while the network request was in
        // flight. The timestamp-only merge must preserve them.
        const WRITERS: usize = 4;
        let directory = tempfile::tempdir().expect("create temporary directory");
        let path = directory.path().join("usage.json");

        let barrier = std::sync::Arc::new(std::sync::Barrier::new(WRITERS));
        let mut writers = Vec::new();
        for writer_index in 0..WRITERS {
            let path = path.clone();
            let barrier = std::sync::Arc::clone(&barrier);
            writers.push(std::thread::spawn(move || {
                barrier.wait();
                if writer_index == 0 {
                    // Background-sync side: merge only the timestamp.
                    update_locked(&path, |stats| stats.last_sync = 42)
                        .expect("timestamp merge must succeed");
                } else {
                    // Tracking side: record a full command.
                    update_locked(&path, |stats| {
                        stats.record_command_on(
                            "search",
                            1,
                            jiff::civil::Date::strptime("%Y-%m-%d", "2025-04-10")
                                .expect("valid fixture date"),
                        );
                    })
                    .expect("counter write must succeed");
                }
            }));
        }
        for writer in writers {
            writer.join().expect("usage writer panicked");
        }

        let stats = UsageStats::load_from(&path).expect("final stats must be valid");
        assert_eq!(stats.total_commands, (WRITERS - 1) as u64);
        assert_eq!(stats.last_sync, 42);
    }
}
