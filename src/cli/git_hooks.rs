//! Git hooks management for OMG CLI
//!
//! Installs and manages Git hooks for environment synchronization:
//! - pre-commit: Warn if omg.lock changed but not staged
//! - post-checkout: Auto-run `omg env sync` on branch switch
//! - post-merge: Notify of environment changes

use anyhow::{Context, Result};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use crate::cli::style;

/// Git hook scripts
const PRE_COMMIT_HOOK: &str = r#"#!/bin/sh
# OMG Pre-commit Hook
# Warns if omg.lock has unstaged changes

if [ -f "omg.lock" ]; then
    # Check if omg.lock is modified but not staged
    if git diff --name-only | grep -q "^omg.lock$"; then
        echo ""
        echo "⚠  Warning: omg.lock has unstaged changes"
        echo "   Consider staging it with: git add omg.lock"
        echo ""
    fi
fi

# Continue with commit
exit 0
"#;

const POST_CHECKOUT_HOOK: &str = r#"#!/bin/sh
# OMG Post-checkout Hook
# Auto-syncs environment when switching branches

PREV_HEAD=$1
NEW_HEAD=$2
BRANCH_CHECKOUT=$3

# Only run on branch checkout (not file checkout)
if [ "$BRANCH_CHECKOUT" = "1" ]; then
    # Check if omg.lock changed
    if [ -f "omg.lock" ]; then
        if ! git diff --quiet "$PREV_HEAD" "$NEW_HEAD" -- omg.lock 2>/dev/null; then
            echo ""
            echo "📦 OMG: Environment changed on branch switch"
            echo "   Run 'omg env check' to see differences"
            echo "   Run 'omg env sync' to synchronize"
            echo ""
        fi
    fi
fi

exit 0
"#;

const POST_MERGE_HOOK: &str = r#"#!/bin/sh
# OMG Post-merge Hook
# Notifies of environment changes after merge

# Check if omg.lock was part of the merge
if git diff-tree -r --name-only --no-commit-id ORIG_HEAD HEAD 2>/dev/null | grep -q "^omg.lock$"; then
    echo ""
    echo "📦 OMG: Environment changed after merge"
    echo "   Run 'omg env check' to see differences"
    echo "   Run 'omg env sync' to synchronize"
    echo ""
fi

exit 0
"#;

/// Find the .git directory (handles submodules and worktrees)
fn find_git_dir() -> Result<PathBuf> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()
        .context("Failed to run git rev-parse")?;

    if !output.status.success() {
        anyhow::bail!("Not a git repository");
    }

    let git_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(git_dir))
}

/// Get the hooks directory
fn get_hooks_dir() -> Result<PathBuf> {
    let git_dir = find_git_dir()?;
    Ok(git_dir.join("hooks"))
}

/// Install all Git hooks
pub fn install(force: bool) -> Result<()> {
    println!("{} Installing Git hooks...\n", style::header("OMG"));

    let hooks_dir = get_hooks_dir()?;
    fs::create_dir_all(&hooks_dir)?;

    let hooks = [
        ("pre-commit", PRE_COMMIT_HOOK),
        ("post-checkout", POST_CHECKOUT_HOOK),
        ("post-merge", POST_MERGE_HOOK),
    ];

    let mut installed = 0;
    let mut skipped = 0;

    for (name, content) in hooks {
        let hook_path = hooks_dir.join(name);

        // Check if hook already exists
        if hook_path.exists()
            && !force
            && let Ok(existing) = fs::read_to_string(&hook_path)
            && existing.contains("# OMG")
        {
            println!("  {} {} (already installed)", style::dim("•"), name);
            continue;
        }

        if hook_path.exists() && !force {
            println!(
                "  {} {} (exists, use --force to overwrite)",
                style::warning("⚠"),
                name
            );
            skipped += 1;
            continue;
        }

        // Write the hook
        fs::write(&hook_path, content).with_context(|| format!("Failed to write {name} hook"))?;

        // Make executable (Unix only)
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&hook_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&hook_path, perms)?;
        }

        println!("  {} {}", style::success("✓"), name);
        installed += 1;
    }

    println!();
    if installed > 0 {
        println!("{} Installed {} hook(s)", style::success("✓"), installed);
    }
    if skipped > 0 {
        println!(
            "{} Skipped {} existing hook(s)",
            style::warning("⚠"),
            skipped
        );
    }

    println!();
    println!("{}", style::dim("Hooks will:"));
    println!("  • Warn if omg.lock has unstaged changes (pre-commit)");
    println!("  • Notify of environment changes on branch switch (post-checkout)");
    println!("  • Notify of environment changes after merge (post-merge)");

    Ok(())
}

/// Uninstall Git hooks
pub fn uninstall() -> Result<()> {
    println!("{} Uninstalling Git hooks...\n", style::header("OMG"));

    let hooks_dir = get_hooks_dir()?;
    let hooks = ["pre-commit", "post-checkout", "post-merge"];

    let mut removed = 0;

    for name in hooks {
        let hook_path = hooks_dir.join(name);

        if !hook_path.exists() {
            println!("  {} {} (not installed)", style::dim("•"), name);
            continue;
        }

        // Check if it's our hook before removing
        if let Ok(content) = fs::read_to_string(&hook_path)
            && !content.contains("# OMG")
        {
            println!(
                "  {} {} (not an OMG hook, skipping)",
                style::warning("⚠"),
                name
            );
            continue;
        }

        fs::remove_file(&hook_path).with_context(|| format!("Failed to remove {name} hook"))?;

        println!("  {} {}", style::success("✓"), name);
        removed += 1;
    }

    println!();
    if removed > 0 {
        println!("{} Removed {} hook(s)", style::success("✓"), removed);
    } else {
        println!("{}", style::dim("No OMG hooks to remove"));
    }

    Ok(())
}

/// Show hook status
pub fn status() -> Result<()> {
    println!("{} Git Hooks Status\n", style::header("OMG"));

    let Ok(hooks_dir) = get_hooks_dir() else {
        println!("  {} Not a git repository", style::error("✗"));
        return Ok(());
    };

    let hooks = ["pre-commit", "post-checkout", "post-merge"];

    for name in hooks {
        let hook_path = hooks_dir.join(name);

        if !hook_path.exists() {
            println!("  {} {} - not installed", style::dim("○"), name);
            continue;
        }

        // Check if it's our hook
        if let Ok(content) = fs::read_to_string(&hook_path) {
            if content.contains("# OMG") {
                println!("  {} {} - installed (OMG)", style::success("●"), name);
            } else {
                println!(
                    "  {} {} - installed (custom, not OMG)",
                    style::warning("●"),
                    name
                );
            }
        } else {
            println!("  {} {} - exists (unreadable)", style::warning("●"), name);
        }
    }

    println!();
    println!(
        "  {} {}",
        style::dim("Hooks directory:"),
        hooks_dir.display()
    );

    Ok(())
}

/// Run a specific hook manually
pub fn run_hook(hook_name: &str) -> Result<()> {
    let hooks_dir = get_hooks_dir()?;
    let hook_path = hooks_dir.join(hook_name);

    if !hook_path.exists() {
        anyhow::bail!("Hook '{hook_name}' is not installed");
    }

    println!("{} Running {hook_name} hook...\n", style::header("OMG"));

    let status = std::process::Command::new(&hook_path)
        .status()
        .with_context(|| format!("Failed to execute {hook_name} hook"))?;

    if status.success() {
        println!("\n{} Hook completed successfully", style::success("✓"));
    } else {
        println!(
            "\n{} Hook exited with code {}",
            style::warning("⚠"),
            status.code().unwrap_or(-1)
        );
    }

    Ok(())
}
