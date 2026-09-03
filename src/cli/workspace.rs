//! Workspace management for OMG CLI
//!
//! Enables monorepo support with:
//! - Multi-project management
//! - Shared runtime versions
//! - Parallel task execution with dependency awareness

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::style;

/// Workspace configuration file name
const WORKSPACE_FILE: &str = "omg-workspace.toml";

/// Workspace configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    /// Workspace name
    pub name: String,
    /// Projects in the workspace
    #[serde(default)]
    pub projects: HashMap<String, WorkspaceProject>,
    /// Created timestamp
    pub created_at: String,
}

/// Project within a workspace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceProject {
    /// Path relative to workspace root
    pub path: String,
    /// Project dependencies (other projects it depends on)
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Project-specific commands
    #[serde(default)]
    pub commands: HashMap<String, String>,
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            name: "workspace".to_string(),
            projects: HashMap::new(),
            created_at: jiff::Timestamp::now().to_string(),
        }
    }
}

impl Workspace {
    /// Load workspace from file
    pub fn load() -> Result<Self> {
        if !Path::new(WORKSPACE_FILE).exists() {
            anyhow::bail!("No workspace found. Run 'omg workspace init' first.");
        }

        let content =
            fs::read_to_string(WORKSPACE_FILE).context("Failed to read workspace file")?;
        toml::from_str(&content).context("Failed to parse workspace file")
    }

    /// Save workspace to file
    pub fn save(&self) -> Result<()> {
        let content = toml::to_string_pretty(self).context("Failed to serialize workspace")?;

        crate::core::safe_ops::atomic_write_file_sync(WORKSPACE_FILE, content)
            .context("Failed to write workspace file")
    }

    /// Get topologically sorted projects (respecting dependencies)
    pub fn sorted_projects(&self) -> Result<Vec<String>> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut temp_mark = HashSet::new();

        let mut names: Vec<&String> = self.projects.keys().collect();
        names.sort();
        for name in names {
            self.visit_project(name, &mut visited, &mut temp_mark, &mut result)?;
        }

        Ok(result)
    }

    fn visit_project<'a>(
        &'a self,
        name: &'a str,
        visited: &mut HashSet<&'a str>,
        temp_mark: &mut HashSet<&'a str>,
        result: &mut Vec<String>,
    ) -> Result<()> {
        if visited.contains(name) {
            return Ok(());
        }
        if !temp_mark.insert(name) {
            anyhow::bail!("Circular dependency detected involving project: {name}");
        }

        // A dependency that isn't a registered project would otherwise appear as a
        // phantom entry in the sorted output and be silently skipped by `run`.
        let Some(project) = self.projects.get(name) else {
            anyhow::bail!(
                "Unknown project '{name}' referenced in depends_on; add it with \
                 'omg workspace add' or fix the reference"
            );
        };
        for dep in &project.depends_on {
            self.visit_project(dep, visited, temp_mark, result)?;
        }

        temp_mark.remove(name);
        visited.insert(name);
        result.push(name.to_string());
        Ok(())
    }
}

/// Initialize a new workspace
pub fn init(name: &str) -> Result<()> {
    // Fail fast before printing any banner so errors aren't buried under it
    if Path::new(WORKSPACE_FILE).exists() {
        anyhow::bail!("Workspace already exists. Delete {WORKSPACE_FILE} to reinitialize.");
    }

    println!(
        "{} Initializing workspace '{}'\n",
        style::header("OMG"),
        name
    );

    let workspace = Workspace {
        name: name.to_string(),
        ..Default::default()
    };

    workspace.save()?;

    println!("{} Created {}", style::success("✓"), WORKSPACE_FILE);
    println!();
    println!("{}", style::dim("Next steps:"));
    println!("  • Add projects: omg workspace add ./api");
    println!("  • Run commands: omg workspace run build");

    Ok(())
}

/// Add a project to the workspace
pub fn add(path: &str, name: Option<&str>) -> Result<()> {
    let mut workspace = Workspace::load()?;

    let project_path = PathBuf::from(path);
    if !project_path.exists() {
        anyhow::bail!("Project path does not exist: {path}");
    }

    // Determine project name
    let project_name = name
        .or_else(|| project_path.file_name().and_then(|name| name.to_str()))
        .unwrap_or("project")
        .to_string();

    if workspace.projects.contains_key(&project_name) {
        anyhow::bail!("Project '{project_name}' already exists in workspace");
    }

    // Detect project type and commands
    let commands = detect_project_commands(&project_path);

    let project = WorkspaceProject {
        path: path.to_string(),
        depends_on: Vec::new(),
        commands,
    };

    workspace.projects.insert(project_name.clone(), project);
    workspace.save()?;

    println!(
        "{} Added project '{project_name}' ({path})",
        style::success("✓")
    );

    Ok(())
}

/// Remove a project from the workspace
pub fn remove(project: &str) -> Result<()> {
    let mut workspace = Workspace::load()?;

    if workspace.projects.remove(project).is_none() {
        anyhow::bail!("Project '{project}' not found in workspace");
    }

    // Remove from dependencies of other projects
    for p in workspace.projects.values_mut() {
        p.depends_on.retain(|d| d != project);
    }

    workspace.save()?;

    println!("{} Removed project '{project}'", style::success("✓"));

    Ok(())
}

/// List all projects in the workspace
pub fn list() -> Result<()> {
    let workspace = Workspace::load()?;

    println!("{} Workspace: {}\n", style::header("OMG"), workspace.name);

    if workspace.projects.is_empty() {
        println!("  {}", style::dim("No projects in workspace"));
        println!("  Run 'omg workspace add ./path' to add a project");
        return Ok(());
    }

    // Get sorted order
    let sorted = workspace.sorted_projects()?;

    for (i, name) in sorted.iter().enumerate() {
        if let Some(project) = workspace.projects.get(name) {
            let deps = if project.depends_on.is_empty() {
                String::new()
            } else {
                format!(" (depends on: {})", project.depends_on.join(", "))
            };

            println!(
                "  {}. {} → {}{}",
                i + 1,
                style::package(name),
                style::dim(&project.path),
                style::dim(&deps)
            );

            // Show available commands
            if !project.commands.is_empty() {
                let cmds: Vec<&str> = project
                    .commands
                    .keys()
                    .map(std::string::String::as_str)
                    .collect();
                println!("     {} {}", style::dim("commands:"), cmds.join(", "));
            }
        }
    }

    Ok(())
}

/// Run a command across all projects
pub async fn run(
    command: &str,
    args: &[String],
    parallel: bool,
    filter: Option<&str>,
) -> Result<()> {
    let workspace = Workspace::load()?;

    println!(
        "{} Running '{command}' across workspace...\n",
        style::header("OMG")
    );

    // Get sorted projects
    let sorted = workspace.sorted_projects()?;

    // Filter projects if requested
    let projects: Vec<&str> = sorted
        .iter()
        .filter(|name| filter.is_none_or(|f| name.contains(f)))
        .map(String::as_str)
        .collect();

    if projects.is_empty() {
        println!("  {}", style::dim("No matching projects"));
        return Ok(());
    }

    if parallel {
        // Run in parallel using tokio
        run_parallel(&workspace, &projects, command, args).await?;
    } else {
        // Run sequentially
        run_sequential(&workspace, &projects, command, args)?;
    }

    Ok(())
}

/// Print the outcome summary and fail loudly when anything failed.
///
/// A partial workspace failure must surface as a nonzero exit code so scripts
/// and CI wrapping `omg workspace run` observe it instead of a misleading zero.
fn print_summary(command: &str, success: usize, failed: usize) -> Result<()> {
    println!();
    println!(
        "{} {success} succeeded, {failed} failed",
        if failed == 0 {
            style::success("✓")
        } else {
            style::warning("⚠")
        }
    );

    if failed == 0 {
        Ok(())
    } else {
        anyhow::bail!("{failed} project(s) failed to run '{command}'")
    }
}

fn run_sequential(
    workspace: &Workspace,
    projects: &[&str],
    command: &str,
    args: &[String],
) -> Result<()> {
    let mut success = 0;
    let mut failed = 0;

    for name in projects {
        if let Some(project) = workspace.projects.get(*name) {
            println!("{} {}", style::arrow("→"), style::package(name));

            let result = run_project_command(&project.path, project, command, args);

            match result {
                Ok(()) => {
                    println!("  {}", style::success("✓ completed"));
                    success += 1;
                }
                Err(e) => {
                    println!("  {} {e}", style::error("✗"));
                    failed += 1;
                }
            }
            println!();
        }
    }

    print_summary(command, success, failed)
}

fn dependency_levels(workspace: &Workspace, projects: &[&str]) -> Result<Vec<Vec<String>>> {
    let mut remaining: std::collections::BTreeSet<String> =
        projects.iter().map(|name| (*name).to_string()).collect();
    let mut levels = Vec::new();

    while !remaining.is_empty() {
        let ready: Vec<String> = remaining
            .iter()
            .filter(|name| {
                workspace.projects[*name]
                    .depends_on
                    .iter()
                    .all(|dependency| !remaining.contains(dependency))
            })
            .cloned()
            .collect();
        anyhow::ensure!(
            !ready.is_empty(),
            "Circular dependency detected while scheduling parallel workspace tasks"
        );
        for name in &ready {
            remaining.remove(name);
        }
        levels.push(ready);
    }

    Ok(levels)
}

async fn run_parallel(
    workspace: &Workspace,
    projects: &[&str],
    command: &str,
    args: &[String],
) -> Result<()> {
    use tokio::task;

    let levels = dependency_levels(workspace, projects)?;
    let selected: HashSet<&str> = projects.iter().copied().collect();
    let mut failed_projects = HashSet::new();
    let mut success = 0;
    let mut failed = 0;

    for level in levels {
        let mut handles = Vec::new();
        for name in level {
            let project = &workspace.projects[&name];
            if project.depends_on.iter().any(|dependency| {
                selected.contains(dependency.as_str()) && failed_projects.contains(dependency)
            }) {
                println!(
                    "{} {} {}",
                    style::error("✗"),
                    style::package(&name),
                    style::dim("skipped because a dependency failed")
                );
                failed_projects.insert(name);
                failed += 1;
                continue;
            }

            let project = project.clone();
            let command = command.to_string();
            let args = args.to_vec();
            let project_name = name.clone();
            handles.push(task::spawn_blocking(move || {
                let result = run_project_command(&project.path, &project, &command, &args);
                (project_name, result)
            }));
        }

        // Wait for the complete dependency level before starting dependents.
        for handle in handles {
            let (name, result) = handle.await?;
            match result {
                Ok(()) => {
                    println!(
                        "{} {} {}",
                        style::success("✓"),
                        style::package(&name),
                        style::dim("completed")
                    );
                    success += 1;
                }
                Err(error) => {
                    println!("{} {} {error}", style::error("✗"), style::package(&name));
                    failed_projects.insert(name);
                    failed += 1;
                }
            }
        }
    }

    print_summary(command, success, failed)
}

fn run_project_command(
    path: &str,
    project: &WorkspaceProject,
    command: &str,
    args: &[String],
) -> Result<()> {
    // Check for custom command first
    if let Some(custom_cmd) = project.commands.get(command) {
        // Repo-defined shell: ask before running it in an attended
        // terminal. Automation without a terminal keeps working, and the
        // command is always echoed so logs show what ran.
        println!("  {} Running repo-defined command: {custom_cmd}", style::arrow("→"));
        if console::user_attended()
            && !dialoguer::Confirm::new()
                .with_prompt(format!(
                    "Run '{command}' from the workspace file in '{path}'?"
                ))
                .default(false)
                .interact()?
        {
            anyhow::bail!("Cancelled '{command}' in '{path}'");
        }
        // POSIX sh: operands after the -c command string become positional
        // parameters ($1, "$@", ...) inside the script, so extra args reach the
        // command without ad-hoc shell quoting of interpolated values.
        // https://pubs.opengroup.org/onlinepubs/9699919799/utilities/sh.html
        let status = std::process::Command::new("sh")
            .arg("-c")
            .arg(custom_cmd)
            .arg("sh")
            .args(args)
            .current_dir(path)
            .status()
            .with_context(|| format!("Failed to execute '{command}' in '{path}'"))?;

        if !status.success() {
            anyhow::bail!(
                "'{command}' in '{path}' exited with code {}",
                status.code().unwrap_or(-1)
            );
        }
        return Ok(());
    }

    // Try to detect and run via omg run
    let mut cmd = std::process::Command::new(sibling_omg());
    cmd.arg("run").arg(command);
    cmd.args(args);
    cmd.current_dir(path);

    let status = cmd
        .status()
        .with_context(|| format!("Failed to execute 'omg run {command}' in '{path}'"))?;

    if !status.success() {
        anyhow::bail!(
            "'omg run {command}' in '{path}' exited with code {}",
            status.code().unwrap_or(-1)
        );
    }

    Ok(())
}

/// Resolve the `omg` binary next to the running executable, falling back to
/// PATH lookup. A PATH-shadowed `omg` must not run with workspace privileges.
fn sibling_omg() -> std::ffi::OsString {
    crate::core::paths::sibling_binary("omg")
        .map(|path| path.into_os_string())
        .unwrap_or_else(|| "omg".into())
}

/// Show environment diff across workspace
pub fn diff(branch: &str) -> Result<()> {
    let workspace = Workspace::load()?;

    println!(
        "{} Comparing workspace environments vs {}\n",
        style::header("OMG"),
        branch
    );

    // Sort names so output order is deterministic despite HashMap iteration
    let mut names: Vec<&String> = workspace.projects.keys().collect();
    names.sort();

    for name in names {
        let project = &workspace.projects[name];
        println!("{} {}", style::arrow("→"), style::package(name));

        let lock_path = PathBuf::from(&project.path).join("omg.lock");
        if !lock_path.exists() {
            println!("  {}", style::dim("No omg.lock file"));
            continue;
        }

        let diff_bytes = git_lockfile_diff(Path::new(&project.path), branch)?;

        if diff_bytes.is_empty() {
            println!("  {}", style::success("No changes"));
        } else {
            let diff = String::from_utf8_lossy(&diff_bytes);
            // Skip the +++/--- file-header lines so they aren't counted as changes
            let added = diff
                .lines()
                .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
                .count();
            let removed = diff
                .lines()
                .filter(|l| l.starts_with('-') && !l.starts_with("---"))
                .count();
            println!(
                "  {} +{} lines, -{} lines",
                style::warning("Changes:"),
                added,
                removed
            );
        }
        println!();
    }

    Ok(())
}

fn git_lockfile_diff(project_path: &Path, branch: &str) -> Result<Vec<u8>> {
    anyhow::ensure!(
        !branch.trim_start().starts_with('-'),
        "Git branch or revision must not begin with '-'"
    );
    let output = std::process::Command::new("git")
        .args(["diff", "--no-ext-diff", branch, "--", "omg.lock"])
        .current_dir(project_path)
        .output()
        .with_context(|| format!("Failed to run git diff in {}", project_path.display()))?;
    if !output.status.success() {
        let stderr =
            crate::cli::style::sanitize_terminal_text(&String::from_utf8_lossy(&output.stderr));
        anyhow::bail!(
            "git diff failed in {} against {branch}: {}",
            project_path.display(),
            stderr.trim()
        );
    }
    Ok(output.stdout)
}

/// Check all project environments without mutating them.
pub fn check() -> Result<()> {
    let workspace = Workspace::load()?;

    println!(
        "{} Checking workspace environments...\n",
        style::header("OMG")
    );

    // Projects needing attention make the command exit nonzero so scripts can
    // distinguish a healthy workspace from an incomplete environment.
    let mut ok_count = 0usize;
    let mut attention_count = 0usize;
    let mut error_count = 0usize;

    // Sort names so output order is deterministic despite HashMap iteration
    let mut names: Vec<&String> = workspace.projects.keys().collect();
    names.sort();

    for name in names {
        let project = &workspace.projects[name];
        println!("{} {}", style::arrow("→"), style::package(name));

        let result = std::process::Command::new("omg")
            .args(["env", "check"])
            .current_dir(&project.path)
            .status();

        match result {
            Ok(status) if status.success() => {
                println!("  {}", style::success("✓ checked"));
                ok_count += 1;
            }
            Ok(_) => {
                println!("  {}", style::warning("⚠ needs attention"));
                attention_count += 1;
            }
            Err(e) => {
                println!("  {} {}", style::error("✗"), e);
                error_count += 1;
            }
        }
    }

    println!();
    if attention_count == 0 && error_count == 0 {
        println!(
            "  {} All {} project environment(s) healthy",
            style::success("✓"),
            ok_count
        );
        Ok(())
    } else {
        anyhow::bail!(
            "{attention_count} project(s) need attention, {error_count} failed to check \
             (of {} total)",
            workspace.projects.len()
        )
    }
}

/// Show workspace status
pub fn status() -> Result<()> {
    let workspace = Workspace::load()?;

    println!(
        "{} Workspace Status: {}\n",
        style::header("OMG"),
        workspace.name
    );

    println!(
        "  {} {} projects",
        style::info("Projects:"),
        workspace.projects.len()
    );

    // Check each project's status
    let mut healthy = 0;
    let mut needs_attention = 0;

    // Sort names so output order is deterministic despite HashMap iteration
    let mut names: Vec<&String> = workspace.projects.keys().collect();
    names.sort();

    for name in names {
        let project = &workspace.projects[name];
        let project_path = PathBuf::from(&project.path);
        let lock_path = project_path.join("omg.lock");

        let status_icon;
        let status_text;

        if !project_path.exists() {
            status_icon = style::error("✗");
            status_text = "path not found";
            needs_attention += 1;
        } else if !lock_path.exists() {
            status_icon = style::warning("○");
            status_text = "no omg.lock";
            needs_attention += 1;
        } else {
            status_icon = style::success("●");
            status_text = "ok";
            healthy += 1;
        }

        println!(
            "    {} {} - {}",
            status_icon,
            style::package(name),
            style::dim(status_text)
        );
    }

    println!();
    println!(
        "  {} {} healthy, {} need attention",
        if needs_attention == 0 {
            style::success("✓")
        } else {
            style::warning("⚠")
        },
        healthy,
        needs_attention
    );

    Ok(())
}

/// Detect project commands based on files present
fn detect_project_commands(path: &Path) -> HashMap<String, String> {
    let mut commands = HashMap::new();

    // Node.js (package.json)
    if path.join("package.json").exists() {
        commands.insert("build".to_string(), "npm run build".to_string());
        commands.insert("test".to_string(), "npm test".to_string());
        commands.insert("lint".to_string(), "npm run lint".to_string());
    }

    // Rust (Cargo.toml)
    if path.join("Cargo.toml").exists() {
        commands.insert("build".to_string(), "cargo build".to_string());
        commands.insert("test".to_string(), "cargo test".to_string());
        commands.insert("lint".to_string(), "cargo clippy".to_string());
    }

    // Python (pyproject.toml or setup.py)
    if path.join("pyproject.toml").exists() || path.join("setup.py").exists() {
        commands.insert("test".to_string(), "pytest".to_string());
        commands.insert("lint".to_string(), "ruff check .".to_string());
    }

    // Go (go.mod)
    if path.join("go.mod").exists() {
        commands.insert("build".to_string(), "go build ./...".to_string());
        commands.insert("test".to_string(), "go test ./...".to_string());
        commands.insert("lint".to_string(), "golangci-lint run".to_string());
    }

    // Makefile
    if path.join("Makefile").exists() {
        commands.insert("build".to_string(), "make build".to_string());
        commands.insert("test".to_string(), "make test".to_string());
    }

    commands
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(path: &str, depends_on: &[&str]) -> WorkspaceProject {
        WorkspaceProject {
            path: path.to_string(),
            depends_on: depends_on.iter().map(|name| (*name).to_string()).collect(),
            commands: HashMap::new(),
        }
    }

    #[test]
    fn sorted_projects_are_dependency_first_and_deterministic() {
        let mut workspace = Workspace::default();
        workspace
            .projects
            .insert("web".to_string(), project("web", &["api"]));
        workspace
            .projects
            .insert("db".to_string(), project("db", &[]));
        workspace
            .projects
            .insert("api".to_string(), project("api", &["db"]));

        assert_eq!(workspace.sorted_projects().unwrap(), ["db", "api", "web"]);
        assert_eq!(
            dependency_levels(&workspace, &["web", "db", "api"]).unwrap(),
            vec![
                vec!["db".to_string()],
                vec!["api".to_string()],
                vec!["web".to_string()]
            ]
        );
    }

    #[test]
    fn independent_projects_share_a_parallel_level() {
        let mut workspace = Workspace::default();
        workspace
            .projects
            .insert("web".to_string(), project("web", &["api"]));
        workspace
            .projects
            .insert("docs".to_string(), project("docs", &[]));
        workspace
            .projects
            .insert("api".to_string(), project("api", &[]));

        assert_eq!(
            dependency_levels(&workspace, &["web", "docs", "api"]).unwrap(),
            vec![
                vec!["api".to_string(), "docs".to_string()],
                vec!["web".to_string()]
            ]
        );
    }

    #[test]
    fn git_diff_rejects_option_injection_before_spawning_git() {
        let directory = tempfile::tempdir().expect("temp directory");
        let output = directory.path().join("injected-output");
        let branch = format!("--output={}", output.display());

        let error = git_lockfile_diff(directory.path(), &branch)
            .expect_err("option-like revisions must be rejected");

        assert!(error.to_string().contains("must not begin with '-'"));
        assert!(!output.exists());
    }

    #[test]
    fn git_diff_failures_are_not_reported_as_no_changes() {
        let directory = tempfile::tempdir().expect("temp directory");

        let error = git_lockfile_diff(directory.path(), "main")
            .expect_err("a non-repository must fail instead of returning an empty diff");

        assert!(error.to_string().contains("git diff failed"), "{error}");
    }
}
