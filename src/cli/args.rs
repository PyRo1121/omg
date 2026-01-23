//! Command-line argument definitions using clap derive macros.

use clap::{Parser, Subcommand};

/// OMG - The Fastest Unified Package Manager for Arch Linux + All Language Runtimes
///
/// 50-200x faster than nvm, pyenv, yay, and pacman combined.
/// Manages system packages (pacman/AUR) and all 7 major language runtimes.
#[derive(Parser, Debug)]
#[command(name = "omg")]
#[command(author = "OMG Team")]
#[command(version)]
#[command(about = "The Fastest Unified Package Manager for Arch Linux + All Language Runtimes", long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Increase verbosity (-v, -vv, -vvv)
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Suppress all output except errors
    #[arg(short, long, global = true)]
    pub quiet: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    // ═══════════════════════════════════════════════════════════════════════
    // PACKAGE MANAGEMENT (Arch + AUR)
    // ═══════════════════════════════════════════════════════════════════════
    /// Search for packages (official repos + AUR)
    #[command(visible_alias = "s", next_help_heading = "Package Management")]
    Search {
        /// Search query
        query: String,
        /// Show detailed AUR info (votes, popularity)
        #[arg(short, long)]
        detailed: bool,
        /// Interactive mode: select packages to install from results
        #[arg(short, long)]
        interactive: bool,
    },

    /// Install packages (auto-detects AUR packages)
    #[command(visible_alias = "i")]
    Install {
        /// Package names to install
        #[arg(required = true)]
        packages: Vec<String>,
        /// Skip confirmation
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Remove packages (with optional dependency cleanup)
    #[command(visible_alias = "r")]
    Remove {
        /// Package names to remove
        #[arg(required = true)]
        packages: Vec<String>,
        /// Also remove unused dependencies
        #[arg(short, long)]
        recursive: bool,
        /// Skip confirmation
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Update all packages (system + runtimes)
    #[command(visible_alias = "u")]
    Update {
        /// Only check for updates, don't install
        #[arg(short, long)]
        check: bool,
        /// Skip confirmation
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Show package information
    Info {
        /// Package name
        package: String,
    },

    /// Explain why a package is installed (dependency chain)
    Why {
        /// Package name to explain
        package: String,
        /// Show reverse dependencies (what depends on this)
        #[arg(short, long)]
        reverse: bool,
    },

    /// Show what packages would be updated
    Outdated {
        /// Show security updates only
        #[arg(short, long)]
        security: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Pin a package to prevent updates
    Pin {
        /// Package or runtime to pin (e.g., "node@20.10.0" or "gcc")
        target: String,
        /// Unpin instead of pin
        #[arg(short, long)]
        unpin: bool,
        /// List all pins
        #[arg(short, long)]
        list: bool,
    },

    /// Show disk usage by packages
    Size {
        /// Show dependency tree for a specific package
        #[arg(short, long)]
        tree: Option<String>,
        /// Number of top packages to show
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },

    /// Show when and why a package was installed
    Blame {
        /// Package name
        package: String,
    },

    /// Compare two environment lock files
    Diff {
        /// First lock file (default: current environment)
        #[arg(short, long)]
        from: Option<String>,
        /// Second lock file to compare against
        to: String,
    },

    /// Create or restore environment snapshots
    Snapshot {
        #[command(subcommand)]
        command: SnapshotCommands,
    },

    /// Generate CI/CD configuration
    Ci {
        #[command(subcommand)]
        command: CiCommands,
    },

    /// Cross-distro migration tools
    Migrate {
        #[command(subcommand)]
        command: MigrateCommands,
    },

    /// Clean up orphan packages and caches
    Clean {
        /// Remove orphan packages (dependencies no longer needed)
        #[arg(short, long)]
        orphans: bool,
        /// Clear package cache
        #[arg(short, long)]
        cache: bool,
        /// Clear AUR build directories
        #[arg(short, long)]
        aur: bool,
        /// Remove all (orphans + cache + aur)
        #[arg(short = 'A', long)]
        all: bool,
    },

    /// List explicitly installed packages
    Explicit {
        /// Only print the count of explicit packages
        #[arg(short, long)]
        count: bool,
    },

    /// Sync package databases from mirrors (parallel, fast)
    #[command(visible_alias = "sy")]
    Sync,

    // ═══════════════════════════════════════════════════════════════════════
    // RUNTIME VERSION MANAGEMENT
    // ═══════════════════════════════════════════════════════════════════════
    /// Switch runtime version (e.g., omg use node 20.10.0)
    #[command(disable_version_flag = true, next_help_heading = "Runtime Management")]
    Use {
        /// Runtime name (node, python, go, rust, ruby, java, bun)
        runtime: String,
        /// Version to use (e.g., 20.10.0, latest, lts). If omitted, detects from version file.
        version: Option<String>,
    },

    /// List installed versions (or available if --all)
    #[command(visible_alias = "ls")]
    List {
        /// Runtime to list versions for (omit for all)
        runtime: Option<String>,
        /// Show available versions, not just installed
        #[arg(short, long)]
        available: bool,
    },

    // ═══════════════════════════════════════════════════════════════════════
    // SHELL INTEGRATION
    // ═══════════════════════════════════════════════════════════════════════
    /// Print shell hook for initialization (add to .zshrc/.bashrc)
    ///
    /// Usage: eval "$(omg hook zsh)"
    #[command(next_help_heading = "Shell Integration")]
    Hook {
        /// Shell type (bash, zsh, fish)
        shell: String,
    },

    /// Internal: Called by shell hook on directory change
    #[command(hide = true)]
    HookEnv {
        /// Shell type
        #[arg(short, long, default_value = "zsh")]
        shell: String,
    },

    // ═══════════════════════════════════════════════════════════════════════
    // DAEMON & CONFIG
    // ═══════════════════════════════════════════════════════════════════════
    /// Start the OMG daemon
    #[command(next_help_heading = "System & Configuration")]
    Daemon {
        /// Run in foreground (don't daemonize)
        #[arg(short, long)]
        foreground: bool,
    },

    /// Get or set configuration
    Config {
        /// Configuration key
        key: Option<String>,
        /// Configuration value (if setting)
        value: Option<String>,
    },

    /// Generate shell completions
    Completions {
        /// Shell type (bash, zsh, fish, powershell, elvish)
        shell: String,
        /// Print to stdout instead of installing
        #[arg(long)]
        stdout: bool,
    },

    /// Show which version of a runtime would be used
    Which {
        /// Runtime name (node, python, go, etc.)
        runtime: String,
    },

    /// Internal: Dynamic shell completions
    #[command(hide = true)]
    Complete {
        /// Shell type (bash, zsh, fish)
        #[arg(short, long)]
        shell: String,
        /// Current word being completed
        #[arg(short, long)]
        current: String,
        /// Last word on the command line
        #[arg(short, long)]
        last: String,
        /// Full command line
        #[arg(short, long)]
        full: Option<String>,
    },

    /// Show system status
    Status {
        /// Use fast path (counts only, skips full dependency scan)
        #[arg(long, short)]
        fast: bool,
    },

    /// Check system health and environment configuration
    Doctor,

    /// Security audit and compliance tools
    Audit {
        #[command(subcommand)]
        command: Option<AuditCommands>,
    },

    /// Run project scripts (e.g., 'omg run build' runs npm/cargo/make)
    #[command(next_help_heading = "Development Tools")]
    Run {
        /// The task to run (e.g., build, test, start)
        #[arg(required = true)]
        task: String,

        /// Arguments to pass to the task
        #[arg(last = true)]
        args: Vec<String>,

        /// Runtime backend (native, mise, native-then-mise)
        #[arg(long)]
        runtime_backend: Option<String>,

        /// Watch mode: re-run task on file changes
        #[arg(short, long)]
        watch: bool,

        /// Run multiple tasks in parallel (comma-separated)
        #[arg(short, long)]
        parallel: bool,

        /// Ecosystem to use (e.g., node, rust, python, make)
        #[arg(short, long)]
        using: Option<String>,

        /// Run task across all detected ecosystems
        #[arg(short, long)]
        all: bool,
    },

    /// Create a new project from a template
    #[command(visible_alias = "create")]
    New {
        /// Stack template (rust, react, node, python, go)
        #[arg(required = true)]
        stack: String,

        /// Project name
        #[arg(required = true)]
        name: String,
    },

    /// Manage cross-ecosystem dev tools (e.g., ripgrep, jq, tldr)
    Tool {
        #[command(subcommand)]
        command: ToolCommands,
    },

    // ═══════════════════════════════════════════════════════════════════════
    // TEAM & ENVIRONMENT
    // ═══════════════════════════════════════════════════════════════════════
    /// Environment management (fingerprinting, drift detection)
    #[command(next_help_heading = "Team & Enterprise")]
    Env {
        #[command(subcommand)]
        command: EnvCommands,
    },

    /// Team collaboration (shared locks, sync, status)
    Team {
        #[command(subcommand)]
        command: TeamCommands,
    },

    /// Container management (Docker/Podman)
    Container {
        #[command(subcommand)]
        command: ContainerCommands,
    },

    /// License management (activate, status, deactivate)
    #[cfg(feature = "license")]
    License {
        #[command(subcommand)]
        command: LicenseCommands,
    },

    /// Fleet management for enterprise (multi-machine)
    Fleet {
        #[command(subcommand)]
        command: FleetCommands,
    },

    /// Enterprise features (reports, policies, compliance)
    Enterprise {
        #[command(subcommand)]
        command: EnterpriseCommands,
    },

    /// View package transaction history
    History {
        /// Number of entries to show
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },

    /// Roll back to a previous system state
    Rollback {
        /// Transaction ID to roll back to (omitted for interactive selection)
        id: Option<String>,
    },

    /// Launch the interactive TUI dashboard for system monitoring and management
    #[command(visible_alias = "d", next_help_heading = "System & Configuration")]
    Dash,

    /// Show usage statistics (time saved, commands used, etc.)
    Stats,

    /// Show system metrics (Prometheus-style)
    Metrics,

    /// Update OMG to the latest version
    #[command(visible_alias = "up", disable_version_flag = true)]
    SelfUpdate {
        /// Force update even if already latest
        #[arg(long)]
        force: bool,
        /// Update to a specific version
        #[arg(long)]
        version: Option<String>,
    },

    /// Interactive first-run setup wizard
    ///
    /// Configures shell hooks, daemon startup, and captures initial environment.
    /// Reduces time from install to first successful command to <2 minutes.
    Init {
        /// Run in non-interactive mode with defaults
        #[arg(long)]
        defaults: bool,
        /// Skip shell hook installation
        #[arg(long)]
        skip_shell: bool,
        /// Skip daemon setup
        #[arg(long)]
        skip_daemon: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum EnvCommands {
    /// Capture current environment state to omg.lock
    Capture,
    /// Check for drift against omg.lock
    Check,
    /// Share environment state as a GitHub Gist
    Share {
        /// Description for the Gist
        #[arg(short, long, default_value = "OMG Environment State")]
        description: String,
        /// Make Gist public (default: secret)
        #[arg(long)]
        public: bool,
    },
    /// Sync environment from a Gist URL or ID
    Sync {
        /// Gist URL or ID
        url: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum ToolCommands {
    /// Install a dev tool from any source (Pacman, Cargo, NPM, Pip, Go)
    Install {
        /// Tool name (e.g. ripgrep, jq, tldr)
        name: String,
    },
    /// List installed tools
    List,
    /// Remove a tool
    Remove { name: String },
    /// Update an installed tool to latest version
    Update {
        /// Tool name (or 'all' to update everything)
        name: String,
    },
    /// Search for tools in the registry
    Search {
        /// Search query
        query: String,
    },
    /// Show available tools in the registry
    Registry,
}

#[derive(Subcommand, Debug)]
pub enum TeamCommands {
    /// Initialize a new team workspace
    Init {
        /// Team identifier (e.g., "mycompany/frontend")
        team_id: String,
        /// Display name for the team
        #[arg(short, long)]
        name: Option<String>,
    },
    /// Join an existing team by remote URL
    Join {
        /// Remote URL (GitHub repo or Gist)
        url: String,
    },
    /// Show team status and member sync state
    Status,
    /// Push local environment to team lock
    Push,
    /// Pull team lock and check for drift
    Pull,
    /// List team members and their sync status
    Members,
    /// Interactive team dashboard (TUI)
    Dashboard,
    /// Generate team invite link
    Invite {
        /// Email address for the invite
        #[arg(short, long)]
        email: Option<String>,
        /// Role for the invitee (admin, lead, developer, readonly)
        #[arg(short, long, default_value = "developer")]
        role: String,
    },
    /// Manage team roles and permissions
    Roles {
        #[command(subcommand)]
        command: TeamRoleCommands,
    },
    /// Propose environment changes for review
    Propose {
        /// Message describing the changes
        message: String,
    },

    /// List pending team proposals
    Proposals,

    /// Review and approve/reject a proposal
    Review {
        /// Proposal ID
        id: u32,
        /// Approve the proposal
        #[arg(long)]
        approve: bool,
        /// Request changes
        #[arg(long)]
        request_changes: Option<String>,
    },
    /// Manage golden path templates
    GoldenPath {
        #[command(subcommand)]
        command: GoldenPathCommands,
    },
    /// Check compliance status
    Compliance {
        /// Export compliance report
        #[arg(long)]
        export: Option<String>,
        /// Enforce compliance (block non-compliant operations)
        #[arg(long)]
        enforce: bool,
    },
    /// Show team activity stream
    Activity {
        /// Number of days to show
        #[arg(short, long, default_value = "7")]
        days: u32,
    },
    /// Manage webhook notifications
    Notify {
        #[command(subcommand)]
        command: NotifyCommands,
    },
}

#[derive(Subcommand, Debug)]
pub enum TeamRoleCommands {
    /// List all roles
    List,
    /// Assign a role to a member
    Assign {
        /// Member username or email
        member: String,
        /// Role to assign (admin, lead, developer, readonly)
        role: String,
    },
    /// Remove a role from a member
    Remove {
        /// Member username or email
        member: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum GoldenPathCommands {
    /// Create a new golden path template
    Create {
        /// Template name
        name: String,
        /// Node version requirement
        #[arg(long)]
        node: Option<String>,
        /// Python version requirement
        #[arg(long)]
        python: Option<String>,
        /// Additional packages to include
        #[arg(long)]
        packages: Option<String>,
    },
    /// List available golden path templates
    List,
    /// Delete a golden path template
    Delete {
        /// Template name
        name: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum NotifyCommands {
    /// Add a webhook notification
    Add {
        /// Notification type (slack, discord, webhook)
        notify_type: String,
        /// Webhook URL
        url: String,
    },
    /// List configured notifications
    List,
    /// Remove a notification
    Remove {
        /// Notification ID
        id: String,
    },
    /// Test a notification
    Test {
        /// Notification ID (or "all")
        id: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum ContainerCommands {
    /// Show container runtime status (Docker/Podman)
    Status,
    /// Run a command in a container
    Run {
        /// Container image to use
        image: String,
        /// Command to run
        #[arg(last = true)]
        command: Vec<String>,
        /// Container name
        #[arg(short, long)]
        name: Option<String>,
        /// Run in background (detached)
        #[arg(short, long)]
        detach: bool,
        /// Run interactively with TTY
        #[arg(short, long)]
        interactive: bool,
        /// Environment variables (KEY=VALUE)
        #[arg(short, long, value_name = "KEY=VALUE")]
        env: Vec<String>,
        /// Volume mounts (host:container)
        #[arg(long, value_name = "HOST:CONTAINER")]
        volume: Vec<String>,
        /// Working directory inside container
        #[arg(short, long)]
        workdir: Option<String>,
    },
    /// Start an interactive shell in a container
    Shell {
        /// Container image (default: ubuntu:24.04 with project mounted)
        #[arg(short, long)]
        image: Option<String>,
        /// Working directory inside container
        #[arg(short, long)]
        workdir: Option<String>,
        /// Environment variables (KEY=VALUE)
        #[arg(short, long, value_name = "KEY=VALUE")]
        env: Vec<String>,
        /// Additional volume mounts (host:container)
        #[arg(long, value_name = "HOST:CONTAINER")]
        volume: Vec<String>,
    },
    /// Build a container image
    Build {
        /// Path to Dockerfile
        #[arg(short = 'f', long)]
        dockerfile: Option<String>,
        /// Image tag
        #[arg(short, long, default_value = "omg-dev:latest")]
        tag: String,
        /// Disable build cache
        #[arg(long)]
        no_cache: bool,
        /// Build arguments (KEY=VALUE)
        #[arg(long, value_name = "KEY=VALUE")]
        build_arg: Vec<String>,
        /// Target build stage
        #[arg(long)]
        target: Option<String>,
    },
    /// List running containers
    List,
    /// List container images
    Images,
    /// Pull a container image
    Pull {
        /// Image to pull
        image: String,
    },
    /// Stop a running container
    Stop {
        /// Container name or ID
        container: String,
    },
    /// Execute a command in a running container
    Exec {
        /// Container name or ID
        container: String,
        /// Command to execute
        #[arg(last = true)]
        command: Vec<String>,
    },
    /// Generate a Dockerfile for the current project
    Init {
        /// Base image to use
        #[arg(short, long)]
        base: Option<String>,
    },
}

#[cfg(feature = "license")]
#[derive(Subcommand, Debug)]
pub enum LicenseCommands {
    /// Activate a license key
    Activate {
        /// License key to activate
        key: String,
    },
    /// Show current license status
    Status,
    /// Deactivate current license
    Deactivate,
    /// Check if a feature is available
    Check {
        /// Feature name to check
        feature: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum AuditCommands {
    /// Scan for vulnerabilities in installed packages (default)
    Scan,
    /// Generate Software Bill of Materials (SBOM) in `CycloneDX` format
    Sbom {
        /// Output file path (default: ~/.local/share/omg/sbom/sbom-`<timestamp>`.json)
        #[arg(short, long)]
        output: Option<String>,
        /// Include vulnerability data in SBOM
        #[arg(long, default_value = "true")]
        vulns: bool,
    },
    /// Scan for leaked secrets and credentials
    Secrets {
        /// Directory to scan (default: current directory)
        #[arg(short, long)]
        path: Option<String>,
    },
    /// View audit log entries
    Log {
        /// Number of entries to show
        #[arg(short, long, default_value = "20")]
        limit: usize,
        /// Filter by severity (debug, info, warning, error, critical)
        #[arg(short, long)]
        severity: Option<String>,
        /// Export log to file
        #[arg(short, long)]
        export: Option<String>,
    },
    /// Verify audit log integrity (tamper detection)
    Verify,
    /// Show security policy status
    Policy,
    /// Check SLSA provenance for a package
    Slsa {
        /// Package file to verify
        package: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum SnapshotCommands {
    /// Create a new snapshot
    Create {
        /// Description for the snapshot
        #[arg(short, long)]
        message: Option<String>,
    },
    /// List all snapshots
    List,
    /// Restore a snapshot
    Restore {
        /// Snapshot ID to restore
        id: String,
        /// Dry run - show what would change
        #[arg(long)]
        dry_run: bool,
        /// Skip confirmation
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Delete a snapshot
    Delete {
        /// Snapshot ID to delete
        id: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum CiCommands {
    /// Initialize CI configuration for a provider
    Init {
        /// CI provider (github, gitlab, circleci)
        provider: String,
        /// Generate advanced "world-class" configuration with matrices and security audits
        #[arg(short, long)]
        advanced: bool,
    },
    /// Validate current environment matches CI expectations
    Validate,
    /// Generate cache manifest for CI
    Cache,
}

#[derive(Subcommand, Debug)]
pub enum MigrateCommands {
    /// Export current environment to a portable manifest
    Export {
        /// Output file path
        #[arg(short, long, default_value = "omg-manifest.json")]
        output: String,
    },
    /// Import environment from a manifest (with package mapping)
    Import {
        /// Manifest file to import
        manifest: String,
        /// Dry run - show what would be installed
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum FleetCommands {
    /// Show fleet status across all machines
    Status,
    /// Push configuration to fleet
    Push {
        /// Target team (or all)
        #[arg(short, long)]
        team: Option<String>,
        /// Message describing the push
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Auto-remediate drift across fleet
    Remediate {
        /// Dry run - show what would change
        #[arg(long)]
        dry_run: bool,
        /// Require confirmation
        #[arg(long)]
        confirm: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum EnterpriseCommands {
    /// Generate executive reports
    Reports {
        /// Report type (monthly, quarterly, custom)
        #[arg(short, long, default_value = "monthly")]
        report_type: String,
        /// Output format (pdf, html, json)
        #[arg(short, long, default_value = "pdf")]
        format: String,
    },
    /// Manage hierarchical policies
    Policy {
        #[command(subcommand)]
        command: EnterprisePolicyCommands,
    },
    /// Export audit evidence for compliance
    AuditExport {
        /// Compliance framework (soc2, iso27001, fedramp, hipaa, pci-dss)
        #[arg(short, long, default_value = "soc2")]
        format: String,
        /// Time period (e.g., "2025-Q4")
        #[arg(short, long)]
        period: Option<String>,
        /// Output directory
        #[arg(short, long, default_value = "audit-evidence")]
        output: String,
    },
    /// Scan for license compliance issues
    LicenseScan {
        /// Export format (spdx, csv, json)
        #[arg(long)]
        export: Option<String>,
    },
    /// Initialize self-hosted/air-gapped server
    Server {
        #[command(subcommand)]
        command: ServerCommands,
    },
}

#[derive(Subcommand, Debug)]
pub enum EnterprisePolicyCommands {
    /// Set a policy rule
    Set {
        /// Scope (org, team:`<name>`, project)
        #[arg(short, long)]
        scope: String,
        /// Rule to set
        rule: String,
    },
    /// Show current policies
    Show {
        /// Scope to show
        #[arg(short, long)]
        scope: Option<String>,
    },
    /// Inherit policies from parent scope
    Inherit {
        /// Source scope
        #[arg(long)]
        from: String,
        /// Target scope
        #[arg(long)]
        to: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum ServerCommands {
    /// Initialize a self-hosted OMG server
    Init {
        /// License key
        #[arg(short, long)]
        license: String,
        /// Storage path
        #[arg(short, long)]
        storage: String,
        /// Domain for the server
        #[arg(short, long)]
        domain: String,
    },
    /// Sync/mirror packages from upstream
    Mirror {
        /// Upstream registry URL
        #[arg(long, default_value = "https://registry.pyro1121.com")]
        upstream: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
    }
}
