//! Usage tracking for OMG
//!
//! Tracks command usage, time saved, and syncs with the API for dashboard display.
//! Works for all tiers (free included) when a license is activated.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

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
    pub sbom_generated: u64,
    /// Vulnerabilities found (Pro+)
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
    /// First use date
    #[serde(default)]
    pub first_use_date: String,
}

impl UsageStats {
    /// Get the usage stats file path
    fn path() -> Result<PathBuf> {
        let data_dir = crate::core::paths::data_dir();
        std::fs::create_dir_all(&data_dir)?;
        Ok(data_dir.join("usage.json"))
    }

    /// Load usage stats from disk
    pub fn load() -> Self {
        Self::path()
            .ok()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Save usage stats to disk
    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Record a command execution
    pub fn record_command(&mut self, command: &str, time_saved_ms: u64) {
        // Update totals
        self.total_commands += 1;
        self.time_saved_ms += time_saved_ms;

        // Update command counts
        *self.commands.entry(command.to_string()).or_insert(0) += 1;

        // Update daily/monthly counters and streak
        let today = jiff::Zoned::now().date().to_string();
        let month = today[..7].to_string(); // YYYY-MM

        // Set first use date if not set
        if self.first_use_date.is_empty() {
            self.first_use_date = today.clone();
        }

        // Update streak
        if self.last_query_date != today {
            // Check if yesterday (streak continues)
            if let Ok(last_date) = jiff::civil::Date::strptime("%Y-%m-%d", &self.last_query_date) {
                let today_date = jiff::Zoned::now().date();
                let diff = today_date - last_date;
                if diff.get_days() == 1 {
                    self.current_streak += 1;
                } else if diff.get_days() > 1 {
                    self.current_streak = 1; // Reset streak
                }
            } else {
                self.current_streak = 1; // First day
            }

            // Update longest streak
            if self.current_streak > self.longest_streak {
                self.longest_streak = self.current_streak;
            }

            self.queries_today = 0;
            self.last_query_date = today;
        }
        self.queries_today += 1;

        if self.last_month != month {
            self.queries_this_month = 0;
            self.last_month = month;
        }
        self.queries_this_month += 1;

        // Check for new achievements
        self.check_achievements();

        // Auto-save
        if let Err(e) = self.save() {
            tracing::warn!("Failed to save usage stats: {}", e);
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
            if let Err(e) = self.save() {
            tracing::warn!("Failed to save usage stats: {}", e);
        }
        }
    }

    /// Record SBOM generation
    pub fn record_sbom(&mut self) {
        self.sbom_generated += 1;
        if let Err(e) = self.save() {
            tracing::warn!("Failed to save usage stats: {}", e);
        }
    }

    /// Record vulnerabilities found
    pub fn record_vulnerabilities(&mut self, count: u64) {
        self.vulnerabilities_found += count;
        if let Err(e) = self.save() {
            tracing::warn!("Failed to save usage stats: {}", e);
        }
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
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
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
        // Sync immediately after first command, achievements, or milestones
        self.total_commands == 1
            || self.total_commands.is_multiple_of(100)
            || (self.time_saved_ms >= 60_000 && self.last_sync == 0)
    }

    /// Sync usage stats to API (async)
    pub async fn sync(&mut self, license_key: &str) -> Result<()> {
        // Get machine info for richer telemetry
        let machine_id = crate::core::license::get_machine_id();
        let hostname = std::fs::read_to_string("/etc/hostname")
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        // Calculate incremental usage since last sync
        let payload = serde_json::json!({
            "license_key": license_key,
            "machine_id": machine_id,
            "hostname": hostname,
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "omg_version": env!("CARGO_PKG_VERSION"),
            "commands_run": self.queries_today,
            "packages_installed": self.commands.get("install").copied().unwrap_or(0),
            "packages_searched": self.commands.get("search").copied().unwrap_or(0),
            "runtimes_switched": self.commands.get("runtime_switch").copied().unwrap_or(0),
            "sbom_generated": self.sbom_generated,
            "vulnerabilities_found": self.vulnerabilities_found,
            "time_saved_ms": self.time_saved_ms,
            "current_streak": self.current_streak,
            "achievements": self.achievements,
        });

        let client = reqwest::Client::new();
        let _response = client
            .post(USAGE_SYNC_API)
            .json(&payload)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?;

        self.last_sync = jiff::Timestamp::now().as_second();
        self.save()?;

        Ok(())
    }
}

/// Track a command execution (convenience function)
pub fn track(command: &str, time_saved_ms: u64) {
    let mut stats = UsageStats::load();
    stats.record_command(command, time_saved_ms);
}

/// Track search command
pub fn track_search() {
    track("search", time_saved::SEARCH_MS);
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

/// Track runtime switch
pub fn track_runtime_switch() {
    track("runtime_switch", time_saved::RUNTIME_SWITCH_MS);
}

/// Track install command
pub fn track_install() {
    track("install", time_saved::INSTALL_MS);
}

/// Sync usage in background if needed
pub fn maybe_sync_background() {
    // Only sync if we have a license
    if let Some(license) = crate::core::license::load_license() {
        let stats = UsageStats::load();
        if stats.needs_sync() || stats.needs_immediate_sync() {
            // Spawn background task
            tokio::spawn(async move {
                let mut stats = UsageStats::load();
                if let Err(e) = stats.sync(&license.key).await {
                    tracing::debug!("Usage sync failed: {e}");
                }
            });
        }
    }
}

/// Force immediate sync (for important events like achievements)
pub fn force_sync_background() {
    if let Some(license) = crate::core::license::load_license() {
        tokio::spawn(async move {
            let mut stats = UsageStats::load();
            if let Err(e) = stats.sync(&license.key).await {
                tracing::debug!("Force sync failed: {e}");
            }
        });
    }
}

/// Track and sync immediately (for real-time dashboard updates)
pub fn track_and_sync(command: &str, time_saved_ms: u64) {
    let mut stats = UsageStats::load();
    stats.record_command(command, time_saved_ms);
    maybe_sync_background();
}

/// Sync usage now (awaitable, for end of CLI commands)
pub async fn sync_usage_now() {
    if crate::core::paths::test_mode() {
        return;
    }
    if let Some(license) = crate::core::license::load_license() {
        let mut stats = UsageStats::load();
        if (stats.needs_sync() || stats.needs_immediate_sync() || stats.total_commands > 0)
            && let Err(e) = stats.sync(&license.key).await
        {
            tracing::debug!("Usage sync failed: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_saved_human() {
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
    fn test_record_command() {
        let mut stats = UsageStats::default();
        stats.record_command("search", 127);
        stats.record_command("search", 127);
        stats.record_command("info", 132);

        assert_eq!(stats.total_commands, 3);
        assert_eq!(stats.commands.get("search"), Some(&2));
        assert_eq!(stats.commands.get("info"), Some(&1));
        assert_eq!(stats.time_saved_ms, 127 + 127 + 132);
    }
}
