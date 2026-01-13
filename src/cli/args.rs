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
    #[command(visible_alias = "s")]
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
    },

    /// Update all packages (system + runtimes)
    #[command(visible_alias = "u")]
    Update {
        /// Only check for updates, don't install
        #[arg(short, long)]
        check: bool,
    },

    /// Show package information
    Info {
        /// Package name
        package: String,
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
    Explicit,

    /// Sync package databases from mirrors (parallel, fast)
    #[command(visible_alias = "sy")]
    Sync,

    // ═══════════════════════════════════════════════════════════════════════
    // RUNTIME VERSION MANAGEMENT
    // ═══════════════════════════════════════════════════════════════════════
    /// Switch runtime version (e.g., omg use node 20.10.0)
    #[command(disable_version_flag = true)]
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

    /// Show system status and statistics
    Status,

    /// Check system health and environment configuration
    Doctor,

    /// Perform a security audit for vulnerabilities
    Audit,

    /// Run project scripts (e.g., 'omg run build' runs npm/cargo/make)
    #[command(visible_alias = "run")]
    Run {
        /// The task to run (e.g., build, test, start)
        #[arg(required = true)]
        task: String,

        /// Arguments to pass to the task
        #[arg(last = true)]
        args: Vec<String>,
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
    Env {
        #[command(subcommand)]
        command: EnvCommands,
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
