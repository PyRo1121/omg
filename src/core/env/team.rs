//! Team collaboration features for shared environment management
//!
//! Provides:
//! - Team workspaces with centralized lock management
//! - Git-based sync with automatic drift detection
//! - Real-time team status dashboard
//! - Conflict resolution for environment changes

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::fingerprint::EnvironmentState;

/// Team configuration stored in `.omg/team.toml`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamConfig {
    /// Team identifier (e.g., "mycompany/frontend")
    pub team_id: String,
    /// Display name for the team
    pub name: String,
    /// Current user's identifier
    pub member_id: String,
    /// Remote sync URL (GitHub repo or Gist)
    pub remote_url: Option<String>,
    /// Whether to auto-sync on git pull
    pub auto_sync: bool,
    /// Whether to auto-push on env capture
    pub auto_push: bool,
    /// Notification settings
    pub notifications: NotificationSettings,
}

impl Default for TeamConfig {
    fn default() -> Self {
        Self {
            team_id: String::new(),
            name: String::new(),
            member_id: whoami::username().unwrap_or_else(|_| "unknown".to_string()),
            remote_url: None,
            auto_sync: true,
            auto_push: false,
            notifications: NotificationSettings::default(),
        }
    }
}

/// Notification preferences
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NotificationSettings {
    /// Notify when teammates update the lock file
    pub on_lock_update: bool,
    /// Notify when drift is detected
    pub on_drift: bool,
    /// Notify when a teammate joins
    pub on_member_join: bool,
}

/// Team member status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    /// Member identifier (username or email)
    pub id: String,
    /// Display name
    pub name: String,
    /// Current environment hash
    pub env_hash: String,
    /// Last sync timestamp
    pub last_sync: i64,
    /// Whether member is in sync with team lock
    pub in_sync: bool,
    /// Drift details if out of sync
    pub drift_summary: Option<String>,
}

/// Team status snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamStatus {
    /// Team configuration
    pub config: TeamConfig,
    /// Current team lock hash
    pub lock_hash: String,
    /// All team members and their status
    pub members: Vec<TeamMember>,
    /// Last update timestamp
    pub updated_at: i64,
}

impl TeamStatus {
    /// Count members in sync
    #[must_use]
    pub fn in_sync_count(&self) -> usize {
        self.members.iter().filter(|m| m.in_sync).count()
    }

    /// Count members out of sync
    #[must_use]
    pub fn out_of_sync_count(&self) -> usize {
        self.members.iter().filter(|m| !m.in_sync).count()
    }
}

/// Team workspace manager
pub struct TeamWorkspace {
    /// Root directory of the workspace
    root: PathBuf,
    /// Team configuration
    config: Option<TeamConfig>,
}

impl TeamWorkspace {
    /// Create a new team workspace manager
    pub fn new(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        let config = Self::load_config(&root).ok();
        Self { root, config }
    }

    /// Get the team config directory
    fn config_dir(&self) -> PathBuf {
        self.root.join(".omg")
    }

    /// Get the team config file path
    fn config_path(&self) -> PathBuf {
        self.config_dir().join("team.toml")
    }

    /// Get the team status file path
    fn status_path(&self) -> PathBuf {
        self.config_dir().join("team-status.json")
    }

    /// Check if this is a team workspace
    #[must_use]
    pub fn is_team_workspace(&self) -> bool {
        self.config.is_some()
    }

    /// Get the team configuration
    #[must_use]
    pub fn config(&self) -> Option<&TeamConfig> {
        self.config.as_ref()
    }

    /// Load team configuration from disk
    fn load_config(root: &Path) -> Result<TeamConfig> {
        let path = root.join(".omg/team.toml");
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read team config: {}", path.display()))?;
        toml::from_str(&content).context("Failed to parse team config")
    }

    /// Initialize a new team workspace
    pub fn init(&mut self, team_id: &str, name: &str) -> Result<()> {
        let config_dir = self.config_dir();
        std::fs::create_dir_all(&config_dir)?;

        let config = TeamConfig {
            team_id: team_id.to_string(),
            name: name.to_string(),
            member_id: whoami::username().unwrap_or_else(|_| "unknown".to_string()),
            remote_url: None,
            auto_sync: true,
            auto_push: false,
            notifications: NotificationSettings {
                on_lock_update: true,
                on_drift: true,
                on_member_join: false,
            },
        };

        let content = toml::to_string_pretty(&config)?;
        std::fs::write(self.config_path(), content)?;

        // Create initial status
        let status = TeamStatus {
            config: config.clone(),
            lock_hash: String::new(),
            members: vec![TeamMember {
                id: config.member_id.clone(),
                name: whoami::realname().unwrap_or_else(|_| "Unknown".to_string()),
                env_hash: String::new(),
                last_sync: jiff::Timestamp::now().as_second(),
                in_sync: true,
                drift_summary: None,
            }],
            updated_at: jiff::Timestamp::now().as_second(),
        };

        let status_json = serde_json::to_string_pretty(&status)?;
        std::fs::write(self.status_path(), status_json)?;

        self.config = Some(config);

        // Install git hooks if in a git repo
        self.install_git_hooks()?;

        Ok(())
    }

    /// Join an existing team workspace
    pub fn join(&mut self, remote_url: &str) -> Result<()> {
        // For now, just set the remote URL and sync
        if let Some(ref mut config) = self.config {
            config.remote_url = Some(remote_url.to_string());
            let content = toml::to_string_pretty(config)?;
            std::fs::write(self.config_path(), content)?;
        } else {
            anyhow::bail!("Not a team workspace. Run 'omg team init' first.");
        }

        Ok(())
    }

    /// Update local member status
    pub async fn update_status(&self) -> Result<TeamStatus> {
        let config = self.config.as_ref().context("Not a team workspace")?;

        // Capture current environment
        let current_env = EnvironmentState::capture().await?;

        // Load team lock if exists
        let lock_path = self.root.join("omg.lock");
        let lock_hash = if lock_path.exists() {
            let lock = EnvironmentState::load(&lock_path)?;
            lock.hash
        } else {
            String::new()
        };

        let in_sync = lock_hash.is_empty() || current_env.hash == lock_hash;

        let member = TeamMember {
            id: config.member_id.clone(),
            name: whoami::realname().unwrap_or_else(|_| "Unknown".to_string()),
            env_hash: current_env.hash,
            last_sync: jiff::Timestamp::now().as_second(),
            in_sync,
            drift_summary: if in_sync {
                None
            } else {
                Some("Environment differs from team lock".to_string())
            },
        };

        // Load existing status or create new
        let mut status = self.load_status().unwrap_or_else(|_| TeamStatus {
            config: config.clone(),
            lock_hash: lock_hash.clone(),
            members: Vec::new(),
            updated_at: jiff::Timestamp::now().as_second(),
        });

        // Update or add member
        if let Some(existing) = status.members.iter_mut().find(|m| m.id == member.id) {
            *existing = member;
        } else {
            status.members.push(member);
        }

        status.lock_hash = lock_hash;
        status.updated_at = jiff::Timestamp::now().as_second();

        // Save status
        let status_json = serde_json::to_string_pretty(&status)?;
        std::fs::write(self.status_path(), status_json)?;

        Ok(status)
    }

    /// Load team status from disk
    pub fn load_status(&self) -> Result<TeamStatus> {
        let content = std::fs::read_to_string(self.status_path())?;
        serde_json::from_str(&content).context("Failed to parse team status")
    }

    /// Push local environment to team lock
    pub async fn push(&self) -> Result<()> {
        let _config = self.config.as_ref().context("Not a team workspace")?;

        // Capture and save
        let state = EnvironmentState::capture().await?;
        let lock_path = self.root.join("omg.lock");
        state.save(&lock_path)?;

        // Update status
        self.update_status().await?;

        // If auto-commit is enabled and we're in a git repo, commit the change
        if self.is_git_repo() {
            self.git_commit_lock("Update omg.lock via team push")?;
        }

        Ok(())
    }

    /// Pull team lock and check for drift
    pub async fn pull(&self) -> Result<bool> {
        let config = self.config.as_ref().context("Not a team workspace")?;

        // If we have a remote, fetch from it
        if let Some(ref remote_url) = config.remote_url {
            // For GitHub Gist URLs, use the existing sync logic
            if remote_url.contains("gist.github.com") {
                super::super::super::cli::env::sync(remote_url.clone()).await?;
            }
        }

        // Update status and return whether we're in sync
        let status = self.update_status().await?;
        let member = status.members.iter().find(|m| m.id == config.member_id);

        Ok(member.is_some_and(|m| m.in_sync))
    }

    /// Check if we're in a git repository
    fn is_git_repo(&self) -> bool {
        self.root.join(".git").exists()
    }

    /// Install git hooks for auto-sync
    fn install_git_hooks(&self) -> Result<()> {
        if !self.is_git_repo() {
            return Ok(());
        }

        let hooks_dir = self.root.join(".git/hooks");
        std::fs::create_dir_all(&hooks_dir)?;

        // Post-merge hook (runs after git pull)
        let post_merge = hooks_dir.join("post-merge");
        let hook_content = r#"#!/bin/sh
# OMG Team Sync Hook
# Auto-check for environment drift after git pull

if [ -f "omg.lock" ]; then
    echo "🔄 OMG: Checking for environment drift..."
    omg env check 2>/dev/null || echo "⚠️  OMG: Environment drift detected! Run 'omg env check' for details."
fi
"#;

        // Only write if hook doesn't exist or is our hook
        if !post_merge.exists() {
            std::fs::write(&post_merge, hook_content)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&post_merge, std::fs::Permissions::from_mode(0o755))?;
            }
        }

        // Post-checkout hook (runs after git checkout)
        let post_checkout = hooks_dir.join("post-checkout");
        let checkout_hook = r#"#!/bin/sh
# OMG Team Sync Hook
# Auto-check for environment drift after git checkout

if [ -f "omg.lock" ]; then
    omg env check 2>/dev/null || true
fi
"#;

        if !post_checkout.exists() {
            std::fs::write(&post_checkout, checkout_hook)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&post_checkout, std::fs::Permissions::from_mode(0o755))?;
            }
        }

        Ok(())
    }

    /// Commit omg.lock to git
    fn git_commit_lock(&self, message: &str) -> Result<()> {
        use std::process::Command;

        let lock_path = self.root.join("omg.lock");
        if !lock_path.exists() {
            return Ok(());
        }

        // Stage omg.lock
        Command::new("git")
            .args(["add", "--", "omg.lock"])
            .current_dir(&self.root)
            .output()?;

        // Commit
        Command::new("git")
            .args(["commit", "-m", message, "--no-verify"])
            .current_dir(&self.root)
            .output()?;

        Ok(())
    }
}

/// Detect if omg.lock has changed in git
pub fn detect_lock_changes() -> Result<bool> {
    use std::process::Command;

    let output = Command::new("git")
        .args(["diff", "--name-only", "HEAD~1", "HEAD"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().any(|line| line.trim() == "omg.lock"))
}

/// Get the git remote URL for the current repo
pub fn get_git_remote() -> Result<Option<String>> {
    use std::process::Command;

    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()?;

    if output.status.success() {
        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(Some(url))
    } else {
        Ok(None)
    }
}
