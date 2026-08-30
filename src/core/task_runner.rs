use anyhow::{Context, Result};
use dialoguer::{Confirm, Select, theme::ColorfulTheme};
use owo_colors::OwoColorize;
use semver::{Version, VersionReq};
use serde::Deserialize;
use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::core::runtime_resolver::find_in_path;
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
            for t in ["build", "test", "check", "run", "clippy", "fmt"] {
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
        let mut seen = std::collections::HashSet::new();
        for line in content.lines() {
            // Recipe lines and comments are never rules.
            if line.starts_with('\t') || line.trim_start().starts_with('#') {
                continue;
            }
            let Some((targets, after_colon)) = line.split_once(':') else {
                continue;
            };
            // Colon-style variable assignments (`A := b`, `A ::= b`) have `=`
            // immediately after the colon (modulo the assignment operator's
            // own colons); rule lines never do.
            let normalized = after_colon.trim_start().trim_start_matches(':');
            if normalized.starts_with('=') || normalized.contains('%') {
                // `%` on the right side marks a pattern/static-pattern rule,
                // whose left side is not a plain runnable target.
                continue;
            }
            for target in targets.split_whitespace() {
                let is_target_like = !target.is_empty()
                    && !target.contains('=')
                    && !target.contains('.')
                    && !target.contains('%')
                    && !target.contains('$')
                    && !target.starts_with('#');
                // Deduplicate repeated targets so `resolve` does not present
                // identical duplicates as an ambiguous multi-match.
                if is_target_like && seen.insert(target) {
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
        if let Some(preferred_ecosystem) = self.config.scripts.get(task_name)
            && matches
                .iter()
                .any(|task| task.ecosystem.matches(preferred_ecosystem))
        {
            matches.retain(|task| task.ecosystem.matches(preferred_ecosystem));
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

/// Package managers that consume flags meant for the underlying script unless
/// the flags follow a `--` separator.
fn needs_arg_separator(command: &str) -> bool {
    ["npm", "pnpm", "yarn", "composer"]
        .iter()
        .any(|manager| command.eq_ignore_ascii_case(manager))
}

/// Execute a task with advanced options.
///
/// Task resolution order:
/// 1. Detected tasks from project manifests (`package.json`, `Cargo.toml`, …).
/// 2. Ordered manifest-driven fallbacks (`make`, `npm run`, `task`, …).
/// 3. As a final resort, `task_name` itself is executed as a command resolved
///    from `PATH`. This passthrough is intentional and bounded: the name was
///    validated against `[A-Za-z0-9._-]` up front, so `omg run <tool>` behaves
///    like invoking `<tool>` directly.
pub fn run_task_advanced(
    task_name: &str,
    extra_args: &[String],
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
                return execute_process(
                    cmd,
                    &with_arg_separator(cmd, args, extra_args),
                    extra_args,
                );
            }
        }

        return execute_process(task_name, &[], extra_args);
    }

    for task in matches {
        println!(
            "{} Running task '{}' via {} ({})...",
            "OMG".cyan().bold(),
            task.name.white().bold(),
            task.ecosystem.to_string().magenta(),
            task.source.blue()
        );

        execute_process(
            &task.command,
            &with_arg_separator(&task.command, task.args, extra_args),
            extra_args,
        )?;
    }

    Ok(())
}

pub fn run_task(task_name: &str, extra_args: &[String]) -> Result<()> {
    run_task_advanced(task_name, extra_args, None, false)
}

/// Prepend a `--` separator before user extra args for package managers that
/// would otherwise swallow script-intended flags (e.g. `npm run build --minify`
/// is consumed by npm itself).
fn with_arg_separator(
    command: &str,
    mut task_args: Vec<String>,
    extra_args: &[String],
) -> Vec<String> {
    if needs_arg_separator(command) && !extra_args.is_empty() {
        task_args.push("--".to_string());
    }
    task_args
}

/// Validate an executable command spawned argv-directly (no shell).
fn validate_executable_command(cmd: &str) -> anyhow::Result<()> {
    if cmd.is_empty() {
        anyhow::bail!("Command must not be empty");
    }
    if let Some(c) = cmd.chars().find(|c| c.is_control()) {
        anyhow::bail!("Invalid control character {c:?} in command");
    }
    Ok(())
}

fn run_async<F, T>(future: F) -> Result<T>
where
    F: Future<Output = Result<T>> + Send,
    T: Send,
{
    if let Ok(_handle) = tokio::runtime::Handle::try_current() {
        // We're inside an async context. `block_in_place` PANICS on a
        // current-thread runtime (the production flavor), so instead isolate
        // this work on a dedicated thread with its own runtime — the same
        // pattern privilege.rs uses for elevation from async contexts.
        std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()?
                        .block_on(future)
                })
                .join()
                .map_err(|_| anyhow::anyhow!("async task worker panicked"))?
        })
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

    Ok(Some("npm".to_string()))
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

fn execute_process(cmd: &str, args: &[String], extra_args: &[String]) -> Result<()> {
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
    // Resolve all required runtimes. Sequential processing is required because
    // individual resolvers may ask the user to confirm an installation.
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
    let mut path_additions = hooks::build_path_additions(&versions)?;

    // Auto-activate python virtual environment if present
    // Check for .venv or venv in current directory
    let venv_path = [".venv", "venv"]
        .into_iter()
        .map(|name| current_dir.join(name))
        .find(|path| path.exists());

    let mut command = Command::new(cmd);
    // SECURITY: cmd is spawned argv-directly (no shell), so metacharacters are
    // inert. Reject only what makes an executable path unusable: emptiness and
    // control characters (including NUL). Package-name rules would wrongly
    // reject legitimate relative-path tools like `./gradlew` or `./mvnw`.
    validate_executable_command(cmd)?;

    // Arguments are passed directly to `Command::args` without a shell.
    // Preserve metacharacters verbatim for filters, regular expressions, and
    // literal values such as `$HOME`.

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

fn find_rust_toolchain_file(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start);
    while let Some(dir) = current {
        let rust_toml = dir.join("rust-toolchain.toml");
        if rust_toml.exists() {
            return Some(rust_toml);
        }
        let rust_plain = dir.join("rust-toolchain");
        if rust_plain.exists() {
            return Some(rust_plain);
        }
        current = dir.parent();
    }
    None
}

fn ensure_node_runtime(version: &str) -> Result<String> {
    let normalized = version.trim_start_matches('v');

    // A system executable is usable only when its reported version satisfies
    // the project pin. Merely finding `node` used to bypass the pin entirely.
    if system_runtime_satisfies("node", normalized) {
        return Ok(normalized.to_string());
    }

    // Check OMG-managed Node
    let node_manager = NodeManager::new();
    let installed = node_manager
        .list_installed()
        .context("Failed to list installed Node.js versions")?;
    if installed
        .iter()
        .any(|installed_version| runtime_version_satisfies(installed_version, normalized))
    {
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

    // Check the executable's version instead of treating any Bun on PATH as
    // a match for the project's pin.
    if system_runtime_satisfies("bun", normalized) {
        return Ok(normalized.to_string());
    }

    // Check OMG-managed Bun
    let bun_manager = BunManager::new();
    let installed = bun_manager
        .list_installed()
        .context("Failed to list installed Bun versions")?;
    if installed
        .iter()
        .any(|installed_version| runtime_version_satisfies(installed_version, normalized))
    {
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

fn system_runtime_satisfies(command: &str, requested: &str) -> bool {
    let Ok(output) = Command::new(command).arg("--version").output() else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    if is_floating_runtime_request(requested) {
        return true;
    }
    let Ok(actual) = std::str::from_utf8(&output.stdout) else {
        return false;
    };
    runtime_version_satisfies(actual.trim().trim_start_matches('v'), requested)
}

fn is_floating_runtime_request(requested: &str) -> bool {
    let requested = requested.trim_start_matches('v');
    matches!(requested, "latest" | "lts") || requested.starts_with("lts/")
}

fn runtime_version_satisfies(actual: &str, requested: &str) -> bool {
    let Ok(actual) = Version::parse(actual.trim_start_matches('v')) else {
        return false;
    };
    let requested = requested.trim_start_matches('v');
    if is_floating_runtime_request(requested) {
        return true;
    }
    VersionReq::parse(requested).is_ok_and(|requirement| requirement.matches(&actual))
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
pub fn run_task_watch(task_name: &str, extra_args: &[String]) -> Result<()> {
    use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc::channel;
    use std::time::Duration;

    println!(
        "{} Watch mode: {} (Ctrl+C to stop)\n",
        "OMG".cyan().bold(),
        task_name.white().bold()
    );

    // Initial run; surface failures in watch mode instead of discarding them.
    if let Err(error) = run_task(task_name, extra_args) {
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
    // Set when a change arrives inside the debounce window: the change must
    // still trigger one re-run once the window closes instead of being lost.
    let mut rerun_pending = false;

    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(_event) => {
                // Debounce multiple rapid events
                if last_run.elapsed() < debounce {
                    rerun_pending = true;
                    continue;
                }
                rerun_pending = false;
                last_run = std::time::Instant::now();
                rerun_task_in_watch(task_name, extra_args);
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if rerun_pending && last_run.elapsed() >= debounce {
                    rerun_pending = false;
                    last_run = std::time::Instant::now();
                    rerun_task_in_watch(task_name, extra_args);
                }
                // No events otherwise; continue watching
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    Ok(())
}

fn rerun_task_in_watch(task_name: &str, extra_args: &[String]) {
    println!(
        "\n{} File changed, re-running {}...\n",
        "→".yellow(),
        task_name.cyan()
    );
    if let Err(error) = run_task(task_name, extra_args) {
        eprintln!("{} Task failed: {error}", "!".yellow());
    }
}

const MAX_PARALLEL_TASKS: usize = 16;

fn parse_parallel_task_names(tasks: &str) -> Result<Vec<String>> {
    let task_names = tasks
        .split(',')
        .map(str::trim)
        .map(str::to_string)
        .collect::<Vec<_>>();
    if task_names.iter().any(String::is_empty) {
        anyhow::bail!("Parallel task names must not be empty");
    }
    if task_names.len() > MAX_PARALLEL_TASKS {
        anyhow::bail!(
            "At most {MAX_PARALLEL_TASKS} parallel tasks are allowed (received {})",
            task_names.len()
        );
    }
    Ok(task_names)
}

/// Run multiple tasks in parallel (comma-separated task names).
pub async fn run_tasks_parallel(tasks_str: &str, extra_args: &[String]) -> Result<()> {
    let task_names = parse_parallel_task_names(tasks_str)?;

    if task_names.len() == 1 {
        return run_task(&task_names[0], extra_args);
    }

    println!(
        "{} Running {} tasks in parallel: {}\n",
        "OMG".cyan().bold(),
        task_names.len(),
        task_names.join(", ").white().bold()
    );

    let handles: Vec<_> = task_names
        .into_iter()
        .map(|task| {
            let args = extra_args.to_vec();
            tokio::task::spawn_blocking(move || {
                let result = run_task(&task, &args);
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
    fn executable_commands_allow_relative_path_tools() {
        // Regression: package-name validation rejected `./gradlew`, which this
        // runner itself detects and generates for Gradle projects.
        assert!(validate_executable_command("./gradlew").is_ok());
        assert!(validate_executable_command("./mvnw").is_ok());
        assert!(validate_executable_command("node").is_ok());
        assert!(validate_executable_command("").is_err());
        assert!(validate_executable_command("bad\u{0}cmd").is_err());
        assert!(validate_executable_command("cmd\n").is_err());
    }

    #[test]
    fn runtime_pins_require_a_matching_semver_version() {
        assert!(runtime_version_satisfies("20.11.1", "20"));
        assert!(runtime_version_satisfies("20.11.1", "^20.0.0"));
        assert!(runtime_version_satisfies("20.11.1", "lts"));
        assert!(runtime_version_satisfies("20.11.1", "lts/iron"));
        assert!(runtime_version_satisfies("1.2.3", "latest"));
        assert!(!runtime_version_satisfies("18.20.0", "20"));
        assert!(!runtime_version_satisfies("20.11.1", "not-a-version"));
    }

    #[test]
    fn package_json_without_manager_metadata_defaults_to_npm() {
        let project = TempDir::new().unwrap();
        fs::write(project.path().join("package.json"), r#"{"scripts":{}}"#).unwrap();

        assert_eq!(
            detect_js_package_manager(project.path())
                .unwrap()
                .as_deref(),
            Some("npm")
        );
        assert_eq!(
            detect_js_runtime(project.path()).unwrap(),
            Some(("node".to_string(), "lts".to_string()))
        );
    }

    #[test]
    fn parallel_task_names_reject_empty_entries() {
        let error = parse_parallel_task_names("build,,test")
            .expect_err("empty parallel task must be rejected");
        assert!(error.to_string().contains("must not be empty"));
    }

    #[test]
    fn parallel_task_names_are_bounded() {
        let tasks = (0..=MAX_PARALLEL_TASKS)
            .map(|index| format!("task-{index}"))
            .collect::<Vec<_>>()
            .join(",");

        let error = parse_parallel_task_names(&tasks)
            .expect_err("unbounded parallel task fan-out must be rejected");
        assert!(error.to_string().contains("At most 16"));
    }

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

#[cfg(test)]
#[expect(clippy::unwrap_used)] // Idiomatic in tests: panics on failure with clear error context
mod wave3_tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn makefile_parser_extracts_only_rule_targets() {
        let temp = TempDir::new().unwrap();
        let makefile = "\
CC := gcc
CFLAGS = -Wall
include other.mk
-include generated.d

build: main.o
\t@echo building

test run:
\tcargo test

.PHONY: build
pattern: %.o
var_colon ::= value
";
        fs::write(temp.path().join("Makefile"), makefile).unwrap();

        let detector = TaskDetector::new(temp.path().to_path_buf()).unwrap();
        let mut tasks = Vec::new();
        detector.detect_makefile_tasks(&mut tasks).unwrap();

        let names: Vec<&str> = tasks.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["build", "test", "run"]);
    }

    #[test]
    fn makefile_parser_dedupes_repeated_targets() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("Makefile"),
            "build:\n\t@echo one\n\nbuild:\n\t@echo two\n",
        )
        .unwrap();

        let detector = TaskDetector::new(temp.path().to_path_buf()).unwrap();
        let mut tasks = Vec::new();
        detector.detect_makefile_tasks(&mut tasks).unwrap();

        assert_eq!(tasks.len(), 1);
    }

    #[test]
    fn arg_separator_only_applies_to_flag_swallowing_managers() {
        assert!(needs_arg_separator("npm"));
        assert!(needs_arg_separator("pnpm"));
        assert!(needs_arg_separator("yarn"));
        assert!(needs_arg_separator("composer"));
        assert!(!needs_arg_separator("cargo"));
        assert!(!needs_arg_separator("bun"));

        // Separator is inserted only when there are extra args to protect.
        assert_eq!(
            with_arg_separator(
                "npm",
                vec!["run".into(), "build".into()],
                &["--minify".to_string()]
            ),
            vec!["run".to_string(), "build".to_string(), "--".to_string()]
        );
        assert_eq!(
            with_arg_separator("npm", vec!["run".into()], &[]),
            vec!["run".to_string()]
        );
    }
}
