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
    /// Status-file format version. Files written by a NEWER omg are rejected
    /// on load instead of best-effort parsed.
    #[serde(default = "default_status_format_version")]
    pub format_version: u32,
    /// Team configuration
    pub config: TeamConfig,
    /// Current team lock hash
    pub lock_hash: String,
    /// All team members and their status
    pub members: Vec<TeamMember>,
    /// Last update timestamp
    pub updated_at: i64,
}

fn default_status_format_version() -> u32 {
    TeamStatus::STATUS_FORMAT_VERSION
}

impl TeamStatus {
    /// Current team-status file format version.
    pub const STATUS_FORMAT_VERSION: u32 = 1;

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
    /// Create a new team workspace manager.
    ///
    /// A missing team config means the directory is not initialized. Existing
    /// but unreadable or malformed config is rejected rather than treated as
    /// absent.
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        Self::validate_config_dir(&root)?;
        let config_path = root.join(".omg/team.toml");
        let config = match std::fs::symlink_metadata(&config_path) {
            Ok(metadata) => {
                anyhow::ensure!(
                    !metadata.file_type().is_symlink() && metadata.is_file(),
                    "Team config must be a regular file: {}",
                    config_path.display()
                );
                Some(Self::load_config(&root)?)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Failed to inspect team config: {}", config_path.display())
                });
            }
        };
        Ok(Self { root, config })
    }

    fn validate_config_dir(root: &Path) -> Result<()> {
        let config_dir = root.join(".omg");
        match std::fs::symlink_metadata(&config_dir) {
            Ok(metadata) => {
                anyhow::ensure!(
                    !metadata.file_type().is_symlink() && metadata.is_dir(),
                    "Team config path must be a real directory: {}",
                    config_dir.display()
                );
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).with_context(|| {
                format!(
                    "Failed to inspect team config directory: {}",
                    config_dir.display()
                )
            }),
        }
    }

    fn ensure_config_dir(&self) -> Result<()> {
        Self::validate_config_dir(&self.root)?;
        std::fs::create_dir_all(self.config_dir())
            .context("Failed to create team config directory")?;
        Self::validate_config_dir(&self.root)
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
        Self::validate_config_dir(root)?;
        let path = root.join(".omg/team.toml");
        let metadata = std::fs::symlink_metadata(&path)
            .with_context(|| format!("Failed to inspect team config: {}", path.display()))?;
        anyhow::ensure!(
            !metadata.file_type().is_symlink() && metadata.is_file(),
            "Team config must be a regular file: {}",
            path.display()
        );
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read team config: {}", path.display()))?;
        toml::from_str(&content).context("Failed to parse team config")
    }

    /// Initialize a new team workspace
    pub fn init(&mut self, team_id: &str, name: &str) -> Result<()> {
        self.ensure_config_dir()?;

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

        // Create initial status
        let status = TeamStatus {
            format_version: TeamStatus::STATUS_FORMAT_VERSION,
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

        // Publish the config last. Its presence is the initialization marker,
        // so a failed status write cannot leave a half-initialized workspace.
        let status_json = serde_json::to_vec_pretty(&status)?;
        crate::core::safe_ops::atomic_write_file_sync(self.status_path(), status_json)?;
        let content = toml::to_string_pretty(&config)?;
        crate::core::safe_ops::atomic_write_file_sync(self.config_path(), content)?;

        self.config = Some(config);

        // Install git hooks if in a git repo
        self.install_git_hooks()?;

        Ok(())
    }

    /// Join an existing team workspace
    pub fn join(&mut self, remote_url: &str) -> Result<()> {
        // Fail before creating anything: joining is only valid in an
        // initialized workspace, and a failed call must not leave an empty
        // `.omg/` directory behind as a side effect.
        anyhow::ensure!(
            self.config.is_some(),
            "Not a team workspace. Run 'omg team init' first."
        );
        self.ensure_config_dir()?;
        // For now, just set the remote URL and sync
        if let Some(ref mut config) = self.config {
            config.remote_url = Some(remote_url.to_string());
            let content = toml::to_string_pretty(config)?;
            crate::core::safe_ops::atomic_write_file_sync(self.config_path(), content)?;
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

        // Existing team state is durable data. Reject missing or malformed
        // status instead of replacing it with an empty member set.
        let mut status = self.load_status()?;

        // Update or add member
        if let Some(existing) = status.members.iter_mut().find(|m| m.id == member.id) {
            *existing = member;
        } else {
            status.members.push(member);
        }

        status.lock_hash = lock_hash;
        status.updated_at = jiff::Timestamp::now().as_second();

        // Save status through an atomic replacement so an interruption cannot
        // truncate durable team state.
        self.ensure_config_dir()?;
        let status_json = serde_json::to_vec_pretty(&status)?;
        crate::core::safe_ops::atomic_write_file_sync(self.status_path(), status_json)?;

        Ok(status)
    }

    /// Load team status from disk
    pub fn load_status(&self) -> Result<TeamStatus> {
        Self::validate_config_dir(&self.root)?;
        let path = self.status_path();
        let metadata = std::fs::symlink_metadata(&path)
            .with_context(|| format!("Failed to inspect team status: {}", path.display()))?;
        anyhow::ensure!(
            !metadata.file_type().is_symlink() && metadata.is_file(),
            "Team status must be a regular file: {}",
            path.display()
        );
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read team status: {}", path.display()))?;
        let status: TeamStatus =
            serde_json::from_str(&content).context("Failed to parse team status")?;
        if status.format_version > TeamStatus::STATUS_FORMAT_VERSION {
            anyhow::bail!(
                "Team status {} was written by a newer omg (format version {}). Upgrade omg to read it.",
                path.display(),
                status.format_version
            );
        }
        Ok(status)
    }

    /// Push local environment to team lock
    pub async fn push(&self) -> Result<()> {
        let config = self.config.as_ref().context("Not a team workspace")?;

        // Capture and save
        let state = EnvironmentState::capture().await?;
        let lock_path = self.root.join("omg.lock");
        state.save(&lock_path)?;

        // Update status
        self.update_status().await?;

        if config.auto_push && self.is_git_repo() {
            self.git_commit_lock("Update omg.lock via team push")?;
        }

        Ok(())
    }

    /// Pull team lock and check for drift
    pub async fn pull(&self) -> Result<bool> {
        let config = self.config.as_ref().context("Not a team workspace")?;

        // If we have a remote, fetch from it. Anything other than a Gist must
        // fail loudly: silently skipping the fetch and then reporting purely
        // local state as team sync would mislead operators into trusting a
        // comparison that never saw the team's lock.
        if let Some(remote_url) = &config.remote_url {
            if remote_url.contains("gist.github.com") {
                super::super::super::cli::env::sync(remote_url.clone()).await?;
            } else {
                anyhow::bail!(
                    "Unsupported team remote URL '{remote_url}': pull currently supports only gist.github.com remotes"
                );
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

        let add = Command::new("git")
            .args(["add", "--", "omg.lock"])
            .current_dir(&self.root)
            .output()
            .context("Failed to run git add for omg.lock")?;
        if !add.status.success() {
            anyhow::bail!(
                "git add failed: {}",
                String::from_utf8_lossy(&add.stderr).trim()
            );
        }

        let commit = Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(&self.root)
            .output()
            .context("Failed to run git commit for omg.lock")?;
        if !commit.status.success() {
            anyhow::bail!(
                "git commit failed: {}",
                String::from_utf8_lossy(&commit.stderr).trim()
            );
        }

        Ok(())
    }
}

/// Detect if omg.lock has changed in git
pub fn detect_lock_changes() -> Result<bool> {
    use std::process::Command;

    let output = Command::new("git")
        .args(["diff", "--name-only", "HEAD~1", "HEAD"])
        .output()
        .context("Failed to run git diff for omg.lock change detection")?;

    // A non-zero git status (not a repo, shallow clone, single-commit repo)
    // must surface as an error: silently answering "no changes" would swallow
    // a missed drift notification.
    anyhow::ensure!(
        output.status.success(),
        "git diff failed while detecting lock changes: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_team_config_is_an_uninitialized_workspace() {
        let directory = tempfile::tempdir().expect("temp dir");
        let workspace = TeamWorkspace::new(directory.path()).expect("create workspace");

        assert!(!workspace.is_team_workspace());
    }

    #[cfg(unix)]
    #[test]
    fn init_rejects_symlinked_team_config_directory() {
        use std::os::unix::fs::symlink;

        let workspace_directory = tempfile::tempdir().expect("workspace temp dir");
        let outside_directory = tempfile::tempdir().expect("outside temp dir");
        let mut workspace =
            TeamWorkspace::new(workspace_directory.path()).expect("create workspace manager");
        symlink(
            outside_directory.path(),
            workspace_directory.path().join(".omg"),
        )
        .expect("create malicious config symlink");

        let error = workspace
            .init("team-id", "Team")
            .expect_err("symlinked config directory must be rejected");

        assert!(error.to_string().contains("must be a real directory"));
        assert!(!outside_directory.path().join("team.toml").exists());
        assert!(!outside_directory.path().join("team-status.json").exists());
    }

    #[test]
    fn malformed_team_config_is_rejected_without_rewriting_it() {
        let directory = tempfile::tempdir().expect("temp dir");
        let config_dir = directory.path().join(".omg");
        std::fs::create_dir(&config_dir).expect("create config dir");
        let config_path = config_dir.join("team.toml");
        std::fs::write(&config_path, "team_id = [").expect("write malformed config");

        let error = TeamWorkspace::new(directory.path())
            .err()
            .expect("malformed config must be rejected");

        assert!(error.to_string().contains("Failed to parse team config"));
        assert_eq!(
            std::fs::read_to_string(config_path).expect("read original config"),
            "team_id = ["
        );
    }

    #[test]
    fn join_uninitialized_workspace_creates_nothing() {
        let directory = tempfile::tempdir().expect("temp dir");
        let mut workspace = TeamWorkspace::new(directory.path()).expect("create workspace");

        let error = workspace
            .join("https://gist.github.com/example")
            .expect_err("join on uninitialized workspace must fail");

        assert!(error.to_string().contains("Not a team workspace"));
        assert!(
            !directory.path().join(".omg").exists(),
            "a failed join must not create the .omg directory"
        );
    }

    #[tokio::test]
    async fn pull_rejects_non_gist_remote_instead_of_reporting_local_state() {
        let directory = tempfile::tempdir().expect("temp dir");
        let config_dir = directory.path().join(".omg");
        std::fs::create_dir(&config_dir).expect("create config dir");
        std::fs::write(
            config_dir.join("team.toml"),
            "team_id = 't'\nname = 'Team'\nmember_id = 'm'\nremote_url = 'https://github.com/example/repo.git'\nauto_sync = true\nauto_push = false\n\n[notifications]\non_lock_update = true\non_drift = true\non_member_join = false\n",
        )
        .expect("write team config");
        let workspace = TeamWorkspace::new(directory.path()).expect("create workspace");

        let error = workspace
            .pull()
            .await
            .expect_err("non-gist remote must fail loudly, not fake a local-only sync");

        assert!(error.to_string().contains("Unsupported team remote URL"));
    }
}
