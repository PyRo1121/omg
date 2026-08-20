use anyhow::{Context, Result};
use dialoguer::{Confirm, Select, theme::ColorfulTheme};
use owo_colors::OwoColorize;
use serde::Deserialize;
use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::Settings;
use crate::core::RuntimeBackend;
use crate::core::runtime_resolver::{add_mise_path_fallbacks, find_in_path};
use crate::hooks;
use crate::runtimes::rust::RustManager;
use crate::runtimes::{BunManager, NodeManager};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ecosystem {
    Node,
    Bun,
    Deno,
    Php,
    Rust,
    Make,
    Go,
    Ruby,
    Python,
    Java,
    Custom(String),
}

impl std::fmt::Display for Ecosystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Node => write!(f, "Node.js"),
            Self::Bun => write!(f, "Bun"),
            Self::Deno => write!(f, "Deno"),
            Self::Php => write!(f, "PHP"),
            Self::Rust => write!(f, "Rust"),
            Self::Make => write!(f, "Make"),
            Self::Go => write!(f, "Go"),
            Self::Ruby => write!(f, "Ruby"),
            Self::Python => write!(f, "Python"),
            Self::Java => write!(f, "Java"),
            Self::Custom(s) => write!(f, "{s}"),
        }
    }
}

impl Ecosystem {
    const fn priority(&self) -> i32 {
        match self {
            Self::Rust => 100,
            Self::Node | Self::Bun | Self::Deno => 90,
            Self::Python => 80,
            Self::Go => 75,
            Self::Ruby => 70,
            Self::Java => 60,
            Self::Php => 50,
            Self::Make => 40,
            Self::Custom(_) => 0,
        }
    }

    fn matches(&self, name: &str) -> bool {
        let name = name.to_lowercase();
        if self.to_string().to_lowercase() == name || format!("{self:?}").to_lowercase() == name {
            return true;
        }
        matches!(
            (self, name.as_str()),
            (Self::Node, "node" | "nodejs" | "js" | "javascript")
                | (Self::Python, "py" | "python3")
                | (Self::Rust, "rs" | "cargo")
                | (Self::Make, "makefile")
                | (Self::Go, "golang" | "task")
                | (Self::Ruby, "rb" | "rake")
                | (Self::Php, "composer")
        )
    }
}

#[derive(Debug, Clone)]
pub struct Task {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub source: String,
    pub ecosystem: Ecosystem,
}

#[derive(Deserialize, Default)]
pub struct OmgProjectConfig {
    #[serde(default)]
    pub scripts: HashMap<String, String>,
}

pub struct TaskDetector {
    pub current_dir: PathBuf,
    pub config: OmgProjectConfig,
}

fn read_optional_file(path: &Path) -> Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("Failed to read {}", path.display())),
    }
}

impl TaskDetector {
    pub fn new(current_dir: PathBuf) -> Result<Self> {
        let config = Self::load_config(&current_dir)?;
        Ok(Self {
            current_dir,
            config,
        })
    }

    fn load_config(path: &Path) -> Result<OmgProjectConfig> {
        let config_path = path.join(".omg.toml");
        let Some(content) = read_optional_file(&config_path)? else {
            return Ok(OmgProjectConfig::default());
        };
        toml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", config_path.display()))
    }

    fn detect_js_tasks(&self, tasks: &mut Vec<Task>) -> Result<()> {
        let Some(package_manager) = detect_js_package_manager(&self.current_dir)? else {
            return Ok(());
        };
        let js_ecosystem = if package_manager == "bun" {
            Ecosystem::Bun
        } else {
            Ecosystem::Node
        };

        let path = self.current_dir.join("package.json");
        if let Some(content) = read_optional_file(&path)? {
            let pkg: PackageJson = serde_json::from_str(&content)
                .with_context(|| format!("Failed to parse {}", path.display()))?;
            if let Some(scripts) = pkg.scripts {
                for (name, _) in scripts {
                    tasks.push(Task {
                        name: name.clone(),
                        command: package_manager.clone(),
                        args: vec!["run".to_string(), name],
                        source: "package.json".to_string(),
                        ecosystem: js_ecosystem.clone(),
                    });
                }
            }
        }

        tasks.push(Task {
            name: "install".to_string(),
            command: package_manager,
            args: vec!["install".to_string()],
            source: "package.json".to_string(),
            ecosystem: js_ecosystem,
        });
        Ok(())
    }

    fn detect_deno_tasks(&self, tasks: &mut Vec<Task>) -> Result<()> {
        let path = self.current_dir.join("deno.json");
        let Some(content) = read_optional_file(&path)? else {
            return Ok(());
        };
        let pkg: DenoJson = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))?;
        let Some(dtasks) = pkg.tasks else {
            return Ok(());
        };
        for (name, _) in dtasks {
            tasks.push(Task {
                name: name.clone(),
                command: "deno".to_string(),
                args: vec!["task".to_string(), name],
                source: "deno.json".to_string(),
                ecosystem: Ecosystem::Deno,
            });
        }
        Ok(())
    }

    fn detect_php_tasks(&self, tasks: &mut Vec<Task>) -> Result<()> {
        let path = self.current_dir.join("composer.json");
        let Some(content) = read_optional_file(&path)? else {
            return Ok(());
        };
        let pkg: ComposerJson = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))?;
        let Some(scripts) = pkg.scripts else {
            return Ok(());
        };
        for (name, _) in scripts {
            tasks.push(Task {
                name: name.clone(),
                command: "composer".to_string(),
                args: vec!["run-script".to_string(), name],
                source: "composer.json".to_string(),
                ecosystem: Ecosystem::Php,
            });
        }
        Ok(())
    }

    fn detect_rust_tasks(&self, tasks: &mut Vec<Task>) {
        if self.current_dir.join("Cargo.toml").exists() {
            let standard_tasks = vec!["build", "test", "check", "run", "clippy", "fmt"];
            for t in standard_tasks {
                tasks.push(Task {
                    name: t.to_string(),
                    command: "cargo".to_string(),
                    args: vec![t.to_string()],
                    source: "Cargo.toml".to_string(),
                    ecosystem: Ecosystem::Rust,
                });
            }
        }
    }

    fn detect_makefile_tasks(&self, tasks: &mut Vec<Task>) -> Result<()> {
        let path = self.current_dir.join("Makefile");
        let Some(content) = read_optional_file(&path)? else {
            return Ok(());
        };
        for line in content.lines() {
            if let Some(target) = line.split(':').next() {
                let target = target.trim();
                if !target.is_empty()
                    && !target.contains('=')
                    && !target.contains('.')
                    && !target.starts_with('#')
                    && !target.contains('%')
                {
                    tasks.push(Task {
                        name: target.to_string(),
                        command: "make".to_string(),
                        args: vec![target.to_string()],
                        source: "Makefile".to_string(),
                        ecosystem: Ecosystem::Make,
                    });
                }
            }
        }
        Ok(())
    }

    fn detect_python_tasks(&self, tasks: &mut Vec<Task>) -> Result<()> {
        let pyproject = self.current_dir.join("pyproject.toml");
        if let Some(content) = read_optional_file(&pyproject)? {
            let proj: PyProject = toml::from_str(&content)
                .with_context(|| format!("Failed to parse {}", pyproject.display()))?;
            if let Some(scripts) = proj
                .tool
                .and_then(|tool| tool.poetry)
                .and_then(|p| p.scripts)
            {
                for (name, _) in scripts {
                    tasks.push(Task {
                        name: name.clone(),
                        command: "poetry".to_string(),
                        args: vec!["run".to_string(), name],
                        source: "pyproject.toml".to_string(),
                        ecosystem: Ecosystem::Python,
                    });
                }
            }
        }

        let pipfile = self.current_dir.join("Pipfile");
        if let Some(content) = read_optional_file(&pipfile)? {
            let mut in_scripts = false;
            for line in content.lines() {
                let line = line.trim();
                if line == "[scripts]" {
                    in_scripts = true;
                    continue;
                }
                if line.starts_with('[') && line != "[scripts]" {
                    in_scripts = false;
                }
                if in_scripts
                    && !line.is_empty()
                    && !line.starts_with('#')
                    && let Some((key, _)) = line.split_once('=')
                {
                    let key = key.trim();
                    tasks.push(Task {
                        name: key.to_string(),
                        command: "pipenv".to_string(),
                        args: vec!["run".to_string(), key.to_string()],
                        source: "Pipfile".to_string(),
                        ecosystem: Ecosystem::Python,
                    });
                }
            }
        }
        Ok(())
    }

    fn detect_java_tasks(&self, tasks: &mut Vec<Task>) {
        if self.current_dir.join("pom.xml").exists() {
            for t in ["clean", "compile", "test", "package", "install"] {
                tasks.push(Task {
                    name: t.to_string(),
                    command: "mvn".to_string(),
                    args: vec![t.to_string()],
                    source: "pom.xml".to_string(),
                    ecosystem: Ecosystem::Java,
                });
            }
        }
        if self.current_dir.join("build.gradle").exists()
            || self.current_dir.join("build.gradle.kts").exists()
        {
            for t in ["build", "test", "run", "clean"] {
                tasks.push(Task {
                    name: t.to_string(),
                    command: "./gradlew".to_string(),
                    args: vec![t.to_string()],
                    source: "build.gradle".to_string(),
                    ecosystem: Ecosystem::Java,
                });
            }
        }
    }

    pub fn detect(&self) -> Result<Vec<Task>> {
        let mut tasks = Vec::new();

        self.detect_js_tasks(&mut tasks)?;
        self.detect_deno_tasks(&mut tasks)?;
        self.detect_php_tasks(&mut tasks)?;
        self.detect_rust_tasks(&mut tasks);
        self.detect_makefile_tasks(&mut tasks)?;
        self.detect_python_tasks(&mut tasks)?;
        self.detect_java_tasks(&mut tasks);

        if self.current_dir.join("Taskfile.yml").exists()
            || self.current_dir.join("Taskfile.yaml").exists()
        {
            tasks.push(Task {
                name: "list".to_string(),
                command: "task".to_string(),
                args: vec!["--list".to_string()],
                source: "Taskfile.yml".to_string(),
                ecosystem: Ecosystem::Go,
            });
        }

        if self.current_dir.join("Rakefile").exists() {
            tasks.push(Task {
                name: "tasks".to_string(),
                command: "rake".to_string(),
                args: vec!["-T".to_string()],
                source: "Rakefile".to_string(),
                ecosystem: Ecosystem::Ruby,
            });
        }

        Ok(tasks)
    }

    pub fn resolve(&self, task_name: &str, using: Option<&str>, all: bool) -> Result<Vec<Task>> {
        let all_tasks = self.detect()?;
        let mut matches: Vec<Task> = all_tasks
            .into_iter()
            .filter(|t| t.name == task_name)
            .collect();

        if matches.is_empty() {
            // Fallback logic moved here for better encapsulation
            return Ok(Vec::new());
        }

        // 1. Filter by ecosystem if 'using' is provided
        if let Some(ecosystem_name) = using {
            matches.retain(|t| t.ecosystem.matches(ecosystem_name));

            if matches.is_empty() {
                anyhow::bail!("No task '{task_name}' found for ecosystem '{ecosystem_name}'");
            }
        }

        // 2. Filter by .omg.toml mapping
        if let Some(preferred_ecosystem) = self.config.scripts.get(task_name) {
            let filtered: Vec<Task> = matches
                .iter()
                .filter(|t| t.ecosystem.matches(preferred_ecosystem))
                .cloned()
                .collect();

            if !filtered.is_empty() {
                matches = filtered;
            }
        }

        // 3. If --all, return all matches
        if all {
            return Ok(matches);
        }

        // 4. If multiple matches, resolve ambiguity
        if matches.len() > 1 {
            // Sort by priority
            matches.sort_by_key(|m| std::cmp::Reverse(m.ecosystem.priority()));

            // If priorities are different, and the first one is higher than second, prefer it
            if matches[0].ecosystem.priority() > matches[1].ecosystem.priority() {
                return Ok(vec![matches.swap_remove(0)]);
            }

            // Otherwise, interactive selection
            let items: Vec<String> = matches
                .iter()
                .map(|t| format!("{} (via {})", t.ecosystem, t.source))
                .collect();

            println!(
                "{} Found '{}' in multiple ecosystems:",
                "OMG".cyan().bold(),
                task_name
            );
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Which one did you mean?")
                .items(&items)
                .default(0)
                .interact()?;

            return Ok(vec![matches.swap_remove(selection)]);
        }

        Ok(matches)
    }
}

pub fn detect_tasks() -> Result<Vec<Task>> {
    let detector = TaskDetector::new(std::env::current_dir()?)?;
    detector.detect()
}

/// Execute a task with advanced options
pub fn run_task_advanced(
    task_name: &str,
    extra_args: &[String],
    backend_override: Option<RuntimeBackend>,
    using: Option<&str>,
    all: bool,
) -> Result<()> {
    // SECURITY: Validate task name
    if task_name
        .chars()
        .any(|c| !c.is_ascii_alphanumeric() && c != '-' && c != '_' && c != '.')
    {
        anyhow::bail!("Invalid task name: {task_name}");
    }

    let detector = TaskDetector::new(std::env::current_dir()?)?;
    let matches = detector.resolve(task_name, using, all)?;

    if matches.is_empty() {
        let current_dir = std::env::current_dir()?;

        // Ordered fallback table: (marker_files, command, args_before_task)
        let fallbacks: &[(&[&str], &str, &[&str])] = &[
            (&["Makefile"], "make", &[]),
            (&["package.json"], "npm", &["run"]),
            (&["Taskfile.yml", "Taskfile.yaml"], "task", &[]),
            (&["Rakefile"], "rake", &[]),
            (&["Pipfile"], "pipenv", &["run"]),
            (&["deno.json"], "deno", &["task"]),
            (&["composer.json"], "composer", &["run-script"]),
        ];

        for (markers, cmd, prefix_args) in fallbacks {
            if markers.iter().any(|f| current_dir.join(f).exists()) {
                let display_cmd = if prefix_args.is_empty() {
                    format!("{cmd} {task_name}")
                } else {
                    format!("{cmd} {} {task_name}", prefix_args.join(" "))
                };
                println!(
                    "{} Task '{task_name}' not found, trying '{display_cmd}'...",
                    "→".yellow()
                );
                let mut args: Vec<String> = prefix_args
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect();
                args.push(task_name.to_string());
                return execute_process(cmd, &args, extra_args, backend_override);
            }
        }

        return execute_process(task_name, &[], extra_args, backend_override);
    }

    for task in matches {
        println!(
            "{} Running task '{}' via {} ({})...",
            "OMG".cyan().bold(),
            task.name.white().bold(),
            task.ecosystem.to_string().magenta(),
            task.source.blue()
        );

        execute_process(&task.command, &task.args, extra_args, backend_override)?;
    }

    Ok(())
}

pub fn run_task(
    task_name: &str,
    extra_args: &[String],
    backend_override: Option<RuntimeBackend>,
) -> Result<()> {
    run_task_advanced(task_name, extra_args, backend_override, None, false)
}

fn run_async<F, T>(future: F) -> Result<T>
where
    F: Future<Output = Result<T>>,
{
    // Try to use existing runtime first (if we're already in an async context)
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        // We're in an async context, use block_in_place to avoid deadlock
        tokio::task::block_in_place(|| handle.block_on(future))
    } else {
        // No runtime exists, create a minimal one
        // Use current_thread for sync operations - faster startup than multi_thread
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(future)
    }
}

/// Generic runtime installation helper - DRY implementation for all runtimes
///
/// Handles version normalization, installation check, and interactive prompting.
macro_rules! ensure_runtime_impl {
    ($runtime_name:expr, $normalized:expr, $manager:expr) => {{
        let normalized = $normalized;
        let installed = $manager
            .list_installed()
            .with_context(|| format!("Failed to list installed {} versions", $runtime_name))?;

        if installed.iter().any(|v| v == &normalized) {
            return Ok(normalized);
        }

        let prompt = format!(
            "{} '{}' is missing. Install now?",
            $runtime_name, normalized
        );
        if Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .default(true)
            .interact()?
        {
            run_async($manager.install(&normalized))?;
            Ok(normalized)
        } else {
            anyhow::bail!("{} setup cancelled", $runtime_name)
        }
    }};
}

fn ensure_python_runtime(version: &str) -> Result<String> {
    let manager = crate::runtimes::PythonManager::new();
    ensure_runtime_impl!(
        "Python",
        version.trim_start_matches('v').to_string(),
        manager
    )
}

fn ensure_go_runtime(version: &str) -> Result<String> {
    let manager = crate::runtimes::GoManager::new();
    ensure_runtime_impl!("Go", version.trim_start_matches('v').to_string(), manager)
}

fn ensure_ruby_runtime(version: &str) -> Result<String> {
    let manager = crate::runtimes::RubyManager::new();
    ensure_runtime_impl!("Ruby", version.trim_start_matches('v').to_string(), manager)
}

fn ensure_java_runtime(version: &str) -> Result<String> {
    let manager = crate::runtimes::JavaManager::new();
    ensure_runtime_impl!("Java", version.trim().to_string(), manager)
}

fn detect_js_package_manager(current_dir: &std::path::Path) -> Result<Option<String>> {
    let path = current_dir.join("package.json");
    let Some(content) = read_optional_file(&path)? else {
        return Ok(None);
    };
    let pkg: PackageJson = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {}", path.display()))?;

    if let Some(package_manager) = pkg.package_manager
        && let Some(name) = parse_package_manager_name(&package_manager)
    {
        return Ok(Some(name));
    }

    if current_dir.join("bun.lockb").exists() {
        return Ok(Some("bun".to_string()));
    }
    if current_dir.join("pnpm-lock.yaml").exists() {
        return Ok(Some("pnpm".to_string()));
    }
    if current_dir.join("yarn.lock").exists() {
        return Ok(Some("yarn".to_string()));
    }
    if current_dir.join("package-lock.json").exists()
        || current_dir.join("npm-shrinkwrap.json").exists()
    {
        return Ok(Some("npm".to_string()));
    }

    Ok(Some("bun".to_string()))
}

fn detect_js_runtime(current_dir: &std::path::Path) -> Result<Option<(String, String)>> {
    let Some(package_manager) = detect_js_package_manager(current_dir)? else {
        return Ok(None);
    };
    let runtime = if package_manager == "bun" {
        "bun"
    } else {
        "node"
    };
    let default_version = if runtime == "bun" { "latest" } else { "lts" };
    Ok(Some((runtime.to_string(), default_version.to_string())))
}

#[derive(Deserialize)]
struct PackageJson {
    #[serde(rename = "packageManager")]
    package_manager: Option<String>,
    scripts: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
struct DenoJson {
    tasks: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
struct ComposerJson {
    scripts: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
struct PyProject {
    tool: Option<Tool>,
}

#[derive(Deserialize)]
struct Tool {
    poetry: Option<Poetry>,
}

#[derive(Deserialize)]
struct Poetry {
    scripts: Option<HashMap<String, String>>,
}

fn execute_process(
    cmd: &str,
    args: &[String],
    extra_args: &[String],
    backend_override: Option<RuntimeBackend>,
) -> Result<()> {
    // Detect required runtime versions and inject them into PATH
    // This ensures 'npm' uses the correct node version, 'cargo' uses correct rust channel, etc.
    let current_dir = std::env::current_dir()?;
    if let Some(toolchain_file) = find_rust_toolchain_file(&current_dir) {
        // First check if Rust is available via system (rustup) - if so, let rustup handle it
        let has_system_rust = which::which("rustc").is_ok() || which::which("cargo").is_ok();

        if !has_system_rust {
            // Only use OMG's Rust manager if no system Rust is available
            let rust_manager = RustManager::new();
            let request = RustManager::parse_toolchain_file(&toolchain_file)?;
            let status = rust_manager.toolchain_status(&request)?;

            if status.needs_install
                || !status.missing_components.is_empty()
                || !status.missing_targets.is_empty()
            {
                let prompt = format!("Rust toolchain '{}' is missing. Install now?", status.name);
                if Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt(prompt)
                    .default(true)
                    .interact()?
                {
                    run_async(rust_manager.ensure_toolchain(&request))?;
                } else {
                    anyhow::bail!("Rust toolchain setup cancelled");
                }
            }
        }
        // If system Rust exists, rustup will handle toolchain switching automatically
    }
    let mut versions = hooks::detect_versions(&current_dir)?;
    if let Some((runtime, default_version)) = detect_js_runtime(&current_dir)? {
        versions.entry(runtime).or_insert(default_version);
    }
    ensure_js_package_manager(cmd)?;
    let settings = Settings::load()?;
    let backend = backend_override.unwrap_or(settings.runtime_backend);
    if backend != RuntimeBackend::Mise {
        // Resolve all required runtimes - uses generic ensure_runtime helper
        // Note: Sequential processing required since ensure_* may prompt for user confirmation
        let runtime_resolvers: &[(&str, fn(&str) -> Result<String>)] = &[
            ("node", ensure_node_runtime),
            ("bun", ensure_bun_runtime),
            ("python", ensure_python_runtime),
            ("go", ensure_go_runtime),
            ("ruby", ensure_ruby_runtime),
            ("java", ensure_java_runtime),
        ];

        for (runtime_name, resolver) in runtime_resolvers {
            if let Some(version) = versions.get(*runtime_name).cloned() {
                let resolved = resolver(&version)?;
                versions.insert((*runtime_name).to_string(), resolved);
            }
        }
    }
    let mut path_additions = match backend {
        RuntimeBackend::Mise => Vec::new(),
        _ => hooks::build_path_additions(&versions)?,
    };
    add_mise_path_fallbacks(&versions, &mut path_additions, backend);

    // Auto-activate python virtual environment if present
    // Check for .venv or venv in current directory
    let venv_path = if current_dir.join(".venv").exists() {
        Some(current_dir.join(".venv"))
    } else if current_dir.join("venv").exists() {
        Some(current_dir.join("venv"))
    } else {
        None
    };

    let mut command = Command::new(cmd);
    // SECURITY: Use -- to prevent argument injection if the command supports it
    // Note: We can't blindly add -- to all commands, but we should ensure 'cmd' itself is safe.
    crate::core::security::validate_package_name(cmd)?;

    // SECURITY: Validate extra_args to prevent command injection
    for arg in extra_args {
        // Check for shell metacharacters that could be dangerous
        if arg.contains(';')
            || arg.contains('|')
            || arg.contains('&')
            || arg.contains('`')
            || arg.contains('$')
            || arg.contains('\n')
        {
            anyhow::bail!("Invalid argument '{arg}' - contains shell metacharacters");
        }
        // Warn about potentially dangerous options (but allow them)
        if arg.starts_with("--") && (arg.contains("eval") || arg.contains("exec")) {
            tracing::warn!("Potentially dangerous argument: {}", arg);
        }
    }

    command.args(args);
    command.args(extra_args);

    // Inject virtual env
    if let Some(venv) = venv_path {
        let bin_path = venv.join("bin");
        if bin_path.exists() {
            // Prepend venv/bin to path additions (higher priority)
            path_additions.insert(0, bin_path.display().to_string());

            // Set VIRTUAL_ENV
            command.env("VIRTUAL_ENV", venv.display().to_string());
            // Unset PYTHONHOME if set, to ensure venv is used correctly
            command.env_remove("PYTHONHOME");
        }
    }

    if !path_additions.is_empty()
        && let Ok(current_path) = std::env::var("PATH")
    {
        let new_path = format!("{}:{}", path_additions.join(":"), current_path);
        command.env("PATH", new_path);
    }

    let status = command
        .status()
        .with_context(|| format!("Failed to execute '{cmd}'"))?;

    if !status.success() {
        anyhow::bail!("Task failed with exit code: {:?}", status.code());
    }

    Ok(())
}

fn find_rust_toolchain_file(start: &std::path::Path) -> Option<PathBuf> {
    let mut current = Some(start.to_path_buf());
    while let Some(dir) = current {
        let rust_toml = dir.join("rust-toolchain.toml");
        if rust_toml.exists() {
            return Some(rust_toml);
        }
        let rust_plain = dir.join("rust-toolchain");
        if rust_plain.exists() {
            return Some(rust_plain);
        }
        current = dir.parent().map(std::path::Path::to_path_buf);
    }
    None
}

fn ensure_node_runtime(version: &str) -> Result<String> {
    let normalized = version.trim_start_matches('v');

    // Check if Node is available via system first (nvm, fnm, volta, or system node)
    if which::which("node").is_ok() {
        return Ok(normalized.to_string());
    }

    // Check OMG-managed Node
    let node_manager = NodeManager::new();
    let installed = node_manager
        .list_installed()
        .context("Failed to list installed Node.js versions")?;
    if installed.iter().any(|v| v == normalized) {
        return Ok(normalized.to_string());
    }

    // Check nvm-managed Node
    if let Some(nvm_version) = nvm_resolve_version(normalized)? {
        return Ok(nvm_version);
    }

    let prompt = format!("Node.js '{normalized}' is missing. Install now?");
    if Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .default(true)
        .interact()?
    {
        let resolved = run_async(node_manager.resolve_alias(normalized))?;
        run_async(node_manager.install(&resolved))?;
        Ok(resolved)
    } else {
        anyhow::bail!("Node.js setup cancelled");
    }
}

fn ensure_bun_runtime(version: &str) -> Result<String> {
    let normalized = version.trim_start_matches('v');

    // Check if Bun is available via system first
    if which::which("bun").is_ok() {
        return Ok(normalized.to_string());
    }

    // Check OMG-managed Bun
    let bun_manager = BunManager::new();
    let installed = bun_manager
        .list_installed()
        .context("Failed to list installed Bun versions")?;
    if installed.iter().any(|v| v == normalized) {
        return Ok(normalized.to_string());
    }

    let prompt = format!("Bun '{normalized}' is missing. Install now?");
    if Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .default(true)
        .interact()?
    {
        let resolved = run_async(bun_manager.resolve_alias(normalized))?;
        run_async(bun_manager.install(&resolved))?;
        Ok(resolved)
    } else {
        anyhow::bail!("Bun setup cancelled");
    }
}

fn nvm_resolve_version(version: &str) -> Result<Option<String>> {
    let nvm_dir = std::env::var_os("NVM_DIR")
        .map(PathBuf::from)
        .or_else(|| home::home_dir().map(|dir| dir.join(".nvm")));
    let Some(nvm_dir) = nvm_dir else {
        return Ok(None);
    };

    let resolved = match resolve_nvm_alias(&nvm_dir, version)? {
        Some(alias) => alias,
        None => version.to_string(),
    };
    let normalized = resolved.trim_start_matches('v');
    let bin_path = nvm_dir
        .join("versions/node")
        .join(format!("v{normalized}"))
        .join("bin");
    Ok(bin_path.exists().then(|| normalized.to_string()))
}

fn resolve_nvm_alias(nvm_dir: &std::path::Path, alias: &str) -> Result<Option<String>> {
    let alias_path = nvm_dir.join("alias").join(alias);
    match std::fs::read_to_string(&alias_path) {
        Ok(content) => {
            let resolved = content.trim();
            Ok((!resolved.is_empty()).then(|| resolved.to_string()))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => {
            Err(error).with_context(|| format!("Failed to read nvm alias {}", alias_path.display()))
        }
    }
}

fn parse_package_manager_name(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let (name, _) = trimmed.rsplit_once('@').unwrap_or((trimmed, ""));
    let name = name.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_lowercase())
    }
}

fn ensure_js_package_manager(command: &str) -> Result<()> {
    let command = command.to_lowercase();
    if command != "pnpm" && command != "yarn" {
        return Ok(());
    }

    if find_in_path(&command).is_some() {
        return Ok(());
    }

    if find_in_path("corepack").is_none() {
        anyhow::bail!(
            "{command} is missing and corepack is unavailable. Install {command} or enable corepack."
        );
    }

    let prompt = format!("{command} is missing. Enable via corepack now?");
    if Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .default(true)
        .interact()?
    {
        let status = Command::new("corepack")
            .args(["prepare", "--", &format!("{command}@latest"), "--activate"])
            .status()
            .with_context(|| format!("Failed to run corepack for {command}"))?;
        if !status.success() {
            anyhow::bail!("corepack failed to activate {command}");
        }
        Ok(())
    } else {
        anyhow::bail!("{command} setup cancelled");
    }
}

// Runtime resolution functions moved to core::runtime_resolver module

/// Run a task in watch mode - re-run on file changes
///
/// Synchronous and blocking by design: the process lives in the watch loop.
pub fn run_task_watch(
    task_name: &str,
    extra_args: &[String],
    backend_override: Option<RuntimeBackend>,
) -> Result<()> {
    use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc::channel;
    use std::time::Duration;

    println!(
        "{} Watch mode: {} (Ctrl+C to stop)\n",
        "OMG".cyan().bold(),
        task_name.white().bold()
    );

    // Initial run; surface failures in watch mode instead of discarding them.
    if let Err(error) = run_task(task_name, extra_args, backend_override) {
        eprintln!("{} Task failed: {error}", "!".yellow());
    }

    // Set up file watcher. Events under build-artifact/VCS directories are
    // dropped: without this filter, watching a project root turns `target/`
    // writes from the triggered task itself into an event storm and a
    // self-sustaining rebuild loop.
    let (tx, rx) = channel();
    let mut watcher = RecommendedWatcher::new(
        move |res: std::result::Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                let ignored = event.paths.iter().any(|path| {
                    path.components().any(|component| {
                        matches!(
                            component.as_os_str().to_str(),
                            Some("target" | "node_modules" | ".git")
                        )
                    })
                });
                if !ignored {
                    let _ = tx.send(event);
                }
            }
        },
        Config::default().with_poll_interval(Duration::from_millis(500)),
    )?;

    let current_dir = std::env::current_dir()?;

    // Watch conventional source directories. Only when none exist do we fall
    // back to the project root (with the noise filter above as protection).
    let watch_dirs = ["src", "lib", "app", "pages", "components", "tests"];
    let mut watched_any = false;
    for dir in watch_dirs {
        let path = current_dir.join(dir);
        if !path.exists() {
            continue;
        }
        match watcher.watch(&path, RecursiveMode::Recursive) {
            Ok(()) => watched_any = true,
            Err(error) => {
                eprintln!(
                    "{} Failed to watch {}: {error}",
                    "!".yellow(),
                    path.display()
                );
            }
        }
    }
    if !watched_any {
        watcher
            .watch(&current_dir, RecursiveMode::Recursive)
            .map_err(|error| {
                anyhow::anyhow!("Failed to watch {}: {error}", current_dir.display())
            })?;
    }

    println!("  {} Watching for changes...\n", "→".dimmed());

    // Debounce: wait for changes, then re-run
    let debounce = Duration::from_millis(300);
    let mut last_run = std::time::Instant::now();

    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(_event) => {
                // Debounce multiple rapid events
                if last_run.elapsed() < debounce {
                    continue;
                }
                last_run = std::time::Instant::now();

                println!(
                    "\n{} File changed, re-running {}...\n",
                    "→".yellow(),
                    task_name.cyan()
                );
                if let Err(error) = run_task(task_name, extra_args, backend_override) {
                    eprintln!("{} Task failed: {error}", "!".yellow());
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // No events, continue watching
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    Ok(())
}

/// Run multiple tasks in parallel (comma-separated task names)
pub async fn run_tasks_parallel(
    tasks_str: &str,
    extra_args: &[String],
    backend_override: Option<RuntimeBackend>,
) -> Result<()> {
    let task_names: Vec<&str> = tasks_str.split(',').map(str::trim).collect();

    if task_names.len() == 1 {
        // Single task, just run normally
        return run_task(tasks_str, extra_args, backend_override);
    }

    println!(
        "{} Running {} tasks in parallel: {}\n",
        "OMG".cyan().bold(),
        task_names.len(),
        task_names.join(", ").white().bold()
    );

    let handles: Vec<_> = task_names
        .into_iter()
        .map(|task_name| {
            let task = task_name.to_string();
            let args = extra_args.to_vec();
            let backend = backend_override;
            tokio::spawn(async move {
                let result = run_task(&task, &args, backend);
                (task, result)
            })
        })
        .collect();

    let mut all_success = true;
    for handle in handles {
        match handle.await {
            Ok((task, Ok(()))) => {
                println!("  {} Task '{}' completed", "✓".green(), task);
            }
            Ok((task, Err(e))) => {
                println!("  {} Task '{}' failed: {}", "✗".red(), task, e);
                all_success = false;
            }
            Err(e) => {
                println!("  {} Task panicked: {}", "✗".red(), e);
                all_success = false;
            }
        }
    }

    if all_success {
        println!("\n{}", "All tasks completed successfully!".green());
        Ok(())
    } else {
        anyhow::bail!("Some tasks failed")
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)] // Idiomatic in tests: panics on failure with clear error context
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_ecosystem_priority() {
        assert!(Ecosystem::Rust.priority() > Ecosystem::Node.priority());
        assert!(Ecosystem::Node.priority() > Ecosystem::Python.priority());
        assert!(Ecosystem::Python.priority() > Ecosystem::Make.priority());
    }

    #[test]
    fn test_config_loading() {
        let temp = TempDir::new().unwrap();
        let config_content = r#"[scripts]
test = "rust"
build = "node"
"#;
        fs::write(temp.path().join(".omg.toml"), config_content).unwrap();

        let detector = TaskDetector::new(temp.path().to_path_buf()).unwrap();
        assert_eq!(detector.config.scripts.get("test").unwrap(), "rust");
        assert_eq!(detector.config.scripts.get("build").unwrap(), "node");
    }

    #[tokio::test]
    async fn test_resolve_priority() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        fs::write(
            temp.path().join("package.json"),
            r#"{"scripts": {"test": "echo node"}}"#,
        )
        .unwrap();

        let detector = TaskDetector::new(temp.path().to_path_buf()).unwrap();
        let matches = detector.resolve("test", None, false).unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].ecosystem, Ecosystem::Rust);
    }

    #[tokio::test]
    async fn test_resolve_using_override() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        fs::write(
            temp.path().join("package.json"),
            r#"{"scripts": {"test": "echo node"}}"#,
        )
        .unwrap();

        let detector = TaskDetector::new(temp.path().to_path_buf()).unwrap();
        // Explicitly use 'bun' (default for package.json in detector if no lockfile)
        let matches = detector.resolve("test", Some("bun"), false).unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].ecosystem, Ecosystem::Bun);
    }

    #[tokio::test]
    async fn test_resolve_all() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        fs::write(
            temp.path().join("package.json"),
            r#"{"scripts": {"test": "echo node"}}"#,
        )
        .unwrap();

        let detector = TaskDetector::new(temp.path().to_path_buf()).unwrap();
        let matches = detector.resolve("test", None, true).unwrap();
        assert_eq!(matches.len(), 2);
    }

    #[tokio::test]
    async fn test_resolve_config_override() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        fs::write(
            temp.path().join("package.json"),
            r#"{"scripts": {"test": "echo node"}}"#,
        )
        .unwrap();
        fs::write(temp.path().join(".omg.toml"), "[scripts]\ntest = \"bun\"").unwrap();

        let detector = TaskDetector::new(temp.path().to_path_buf()).unwrap();
        let matches = detector.resolve("test", None, false).unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].ecosystem, Ecosystem::Bun);
    }

    #[test]
    fn corrupt_project_config_fails_closed() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join(".omg.toml"), "scripts = [").unwrap();

        match TaskDetector::new(temp.path().to_path_buf()) {
            Ok(_) => panic!("corrupt .omg.toml must fail"),
            Err(error) => assert!(error.to_string().contains("Failed to parse")),
        }
    }

    #[test]
    fn missing_task_manifests_are_empty() {
        let temp = TempDir::new().unwrap();
        let detector = TaskDetector::new(temp.path().to_path_buf()).unwrap();
        let tasks = detector.detect().unwrap();
        assert!(tasks.is_empty());
    }

    #[test]
    fn unreadable_package_json_fails_closed() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("package.json");
        fs::write(&path, r#"{"scripts": {"test": "echo"}}"#).unwrap();
        let original = fs::metadata(&path).unwrap().permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
        }
        let blocked = fs::read_to_string(&path).is_err();
        let detector = TaskDetector::new(temp.path().to_path_buf()).unwrap();
        let result = detector.detect();
        let _ = fs::set_permissions(&path, original);
        if !blocked {
            return;
        }
        assert!(
            result.is_err(),
            "unreadable package.json must fail closed, got {result:?}"
        );
    }

    #[test]
    fn invalid_package_json_fails_closed() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("package.json"), "not json").unwrap();
        let detector = TaskDetector::new(temp.path().to_path_buf()).unwrap();
        let error = detector.detect().unwrap_err();
        assert!(
            error.to_string().contains("Failed to parse"),
            "unexpected error: {error:#}"
        );
    }

    #[test]
    fn invalid_pyproject_toml_fails_closed() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("pyproject.toml"), "tool = [").unwrap();
        let detector = TaskDetector::new(temp.path().to_path_buf()).unwrap();
        let error = detector.detect().unwrap_err();
        assert!(
            error.to_string().contains("Failed to parse"),
            "unexpected error: {error:#}"
        );
    }

    #[test]
    fn resolve_nvm_alias_missing_is_none() {
        let temp = TempDir::new().unwrap();
        assert!(resolve_nvm_alias(temp.path(), "lts").unwrap().is_none());
    }

    #[test]
    fn resolve_nvm_alias_unreadable_fails_closed() {
        let temp = TempDir::new().unwrap();
        let alias_dir = temp.path().join("alias");
        fs::create_dir(&alias_dir).unwrap();
        let alias = alias_dir.join("lts");
        fs::write(&alias, "20.11.1\n").unwrap();
        let original = fs::metadata(&alias).unwrap().permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&alias, fs::Permissions::from_mode(0o000)).unwrap();
        }
        let blocked = fs::read_to_string(&alias).is_err();
        let result = resolve_nvm_alias(temp.path(), "lts");
        let _ = fs::set_permissions(&alias, original);
        if !blocked {
            return;
        }
        assert!(
            result.is_err(),
            "unreadable nvm alias must fail closed, got {result:?}"
        );
    }
}
