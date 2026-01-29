//! Workspace management for OMG CLI
//!
//! Enables monorepo support with:
//! - Multi-project management
//! - Shared runtime versions
//! - Parallel task execution with dependency awareness

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    /// Shared runtime versions (overrides project-level)
    #[serde(default)]
    pub runtimes: HashMap<String, String>,
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
            runtimes: HashMap::new(),
            created_at: jiff::Timestamp::now().to_string(),
        }
    }
}

impl Workspace {
    /// Load workspace from file
    pub fn load() -> Result<Self> {
        let path = PathBuf::from(WORKSPACE_FILE);
        if !path.exists() {
            anyhow::bail!("No workspace found. Run 'omg workspace init' first.");
        }

        let content = fs::read_to_string(&path).context("Failed to read workspace file")?;

        toml::from_str(&content).context("Failed to parse workspace file")
    }

    /// Save workspace to file
    pub fn save(&self) -> Result<()> {
        let content = toml::to_string_pretty(self).context("Failed to serialize workspace")?;

        fs::write(WORKSPACE_FILE, content).context("Failed to write workspace file")?;

        Ok(())
    }

    /// Get topologically sorted projects (respecting dependencies)
    pub fn sorted_projects(&self) -> Result<Vec<String>> {
        let mut result = Vec::new();
        let mut visited = HashMap::new();
        let mut temp_mark = HashMap::new();

        for name in self.projects.keys() {
            if !visited.contains_key(name) {
                self.visit_project(name, &mut visited, &mut temp_mark, &mut result)?;
            }
        }

        Ok(result)
    }

    fn visit_project(
        &self,
        name: &str,
        visited: &mut HashMap<String, bool>,
        temp_mark: &mut HashMap<String, bool>,
        result: &mut Vec<String>,
    ) -> Result<()> {
        if temp_mark.get(name).copied().unwrap_or(false) {
            anyhow::bail!("Circular dependency detected involving project: {name}");
        }

        if !visited.contains_key(name) {
            temp_mark.insert(name.to_string(), true);

            if let Some(project) = self.projects.get(name) {
                for dep in &project.depends_on {
                    self.visit_project(dep, visited, temp_mark, result)?;
                }
            }

            temp_mark.insert(name.to_string(), false);
            visited.insert(name.to_string(), true);
            result.push(name.to_string());
        }

        Ok(())
    }
}

/// Initialize a new workspace
pub fn init(name: &str) -> Result<()> {
    println!(
        "{} Initializing workspace '{}'\n",
        style::header("OMG"),
        name
    );

    let path = PathBuf::from(WORKSPACE_FILE);
    if path.exists() {
        anyhow::bail!(
            "Workspace already exists. Delete {WORKSPACE_FILE} to reinitialize."
        );
    }

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
        .map(String::from)
        .or_else(|| {
            project_path
                .file_name()
                .and_then(|n| n.to_str())
                .map(String::from)
        })
        .unwrap_or_else(|| "project".to_string());

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
                let cmds: Vec<&str> = project.commands.keys().map(std::string::String::as_str).collect();
                println!("     {} {}", style::dim("commands:"), cmds.join(", "));
            }
        }
    }

    // Show shared runtimes
    if !workspace.runtimes.is_empty() {
        println!();
        println!("  {}", style::dim("Shared Runtimes:"));
        for (runtime, version) in &workspace.runtimes {
            println!(
                "    {} @ {}",
                style::runtime(runtime),
                style::version(version)
            );
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
    let projects: Vec<_> = sorted
        .iter()
        .filter(|name| {
            if let Some(f) = filter {
                name.contains(f)
            } else {
                true
            }
        })
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
        run_sequential(&workspace, &projects, command, args);
    }

    Ok(())
}

fn run_sequential(
    workspace: &Workspace,
    projects: &[&String],
    command: &str,
    args: &[String],
) {
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

    println!(
        "{} {success} succeeded, {failed} failed",
        if failed == 0 {
            style::success("✓")
        } else {
            style::warning("⚠")
        }
    );
}

async fn run_parallel(
    workspace: &Workspace,
    projects: &[&String],
    command: &str,
    args: &[String],
) -> Result<()> {
    use tokio::task;

    let mut handles = Vec::new();

    for name in projects {
        if let Some(project) = workspace.projects.get(*name) {
            let path = project.path.clone();
            let proj = project.clone();
            let cmd = command.to_string();
            let cmd_args = args.to_vec();
            let proj_name = (*name).clone();

            let handle = task::spawn_blocking(move || {
                let result = run_project_command(&path, &proj, &cmd, &cmd_args);
                (proj_name, result)
            });

            handles.push(handle);
        }
    }

    let mut success = 0;
    let mut failed = 0;

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
            Err(e) => {
                println!("{} {} {e}", style::error("✗"), style::package(&name));
                failed += 1;
            }
        }
    }

    println!();
    println!(
        "{} {} succeeded, {} failed",
        if failed == 0 {
            style::success("✓")
        } else {
            style::warning("⚠")
        },
        success,
        failed
    );

    Ok(())
}

fn run_project_command(
    path: &str,
    project: &WorkspaceProject,
    command: &str,
    args: &[String],
) -> Result<()> {
    // Check for custom command first
    if let Some(custom_cmd) = project.commands.get(command) {
        let status = std::process::Command::new("sh")
            .arg("-c")
            .arg(custom_cmd)
            .current_dir(path)
            .status()
            .context("Failed to execute command")?;

        if !status.success() {
            anyhow::bail!("Command exited with code {}", status.code().unwrap_or(-1));
        }
        return Ok(());
    }

    // Try to detect and run via omg run
    let mut cmd = std::process::Command::new("omg");
    cmd.arg("run").arg(command);
    cmd.args(args);
    cmd.current_dir(path);

    let status = cmd.status().context("Failed to execute omg run")?;

    if !status.success() {
        anyhow::bail!("Command exited with code {}", status.code().unwrap_or(-1));
    }

    Ok(())
}

/// Show environment diff across workspace
pub fn diff(branch: &str) -> Result<()> {
    let workspace = Workspace::load()?;

    println!(
        "{} Comparing workspace environments vs {}\n",
        style::header("OMG"),
        branch
    );

    for (name, project) in &workspace.projects {
        println!("{} {}", style::arrow("→"), style::package(name));

        let lock_path = PathBuf::from(&project.path).join("omg.lock");
        if !lock_path.exists() {
            println!("  {}", style::dim("No omg.lock file"));
            continue;
        }

        // Check git diff for omg.lock
        let output = std::process::Command::new("git")
            .args(["diff", branch, "--", "omg.lock"])
            .current_dir(&project.path)
            .output()
            .context("Failed to run git diff")?;

        if output.stdout.is_empty() {
            println!("  {}", style::success("No changes"));
        } else {
            let diff = String::from_utf8_lossy(&output.stdout);
            let added = diff.lines().filter(|l| l.starts_with('+')).count();
            let removed = diff.lines().filter(|l| l.starts_with('-')).count();
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

/// Sync all project environments
pub fn sync(yes: bool) -> Result<()> {
    let workspace = Workspace::load()?;

    println!(
        "{} Syncing workspace environments...\n",
        style::header("OMG")
    );

    if !yes {
        let confirm = dialoguer::Confirm::new()
            .with_prompt("Sync all project environments?")
            .default(true)
            .interact()?;

        if !confirm {
            println!("{}", style::dim("Cancelled"));
            return Ok(());
        }
    }

    for (name, project) in &workspace.projects {
        println!("{} {}", style::arrow("→"), style::package(name));

        let result = std::process::Command::new("omg")
            .args(["env", "check"])
            .current_dir(&project.path)
            .status();

        match result {
            Ok(status) if status.success() => {
                println!("  {}", style::success("✓ synced"));
            }
            Ok(_) => {
                println!("  {}", style::warning("⚠ needs attention"));
            }
            Err(e) => {
                println!("  {} {}", style::error("✗"), e);
            }
        }
    }

    Ok(())
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

    for (name, project) in &workspace.projects {
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
