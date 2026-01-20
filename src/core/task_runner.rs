use anyhow::{Context, Result};
use dialoguer::{Confirm, theme::ColorfulTheme};
use owo_colors::OwoColorize;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::PathBuf;
use std::process::Command;

use crate::config::Settings;
use crate::core::{RuntimeBackend, paths};
use crate::hooks;
use crate::runtimes::rust::RustManager;
use crate::runtimes::{BunManager, NodeManager};

#[derive(Debug, Clone)]
pub struct Task {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub source: String,
}

/// Run an async future from sync context, reusing existing runtime if available
/// This is the Rust 2024 best practice - avoid creating multiple runtimes
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

fn ensure_python_runtime(version: &str) -> Result<String> {
    let normalized = version.trim_start_matches('v');
    let manager = crate::runtimes::PythonManager::new();
    let installed = manager.list_installed().unwrap_or_default();
    if installed.iter().any(|v| v == normalized) {
        return Ok(normalized.to_string());
    }

    let prompt = format!("Python '{normalized}' is missing. Install now?");
    if Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .default(true)
        .interact()?
    {
        run_async(manager.install(normalized))?;
        Ok(normalized.to_string())
    } else {
        anyhow::bail!("Python setup cancelled");
    }
}

fn ensure_go_runtime(version: &str) -> Result<String> {
    let normalized = version.trim_start_matches('v');
    let manager = crate::runtimes::GoManager::new();
    let installed = manager.list_installed().unwrap_or_default();
    if installed.iter().any(|v| v == normalized) {
        return Ok(normalized.to_string());
    }

    let prompt = format!("Go '{normalized}' is missing. Install now?");
    if Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .default(true)
        .interact()?
    {
        run_async(manager.install(normalized))?;
        Ok(normalized.to_string())
    } else {
        anyhow::bail!("Go setup cancelled");
    }
}

fn ensure_ruby_runtime(version: &str) -> Result<String> {
    let normalized = version.trim_start_matches('v');
    let manager = crate::runtimes::RubyManager::new();
    let installed = manager.list_installed().unwrap_or_default();
    if installed.iter().any(|v| v == normalized) {
        return Ok(normalized.to_string());
    }

    let prompt = format!("Ruby '{normalized}' is missing. Install now?");
    if Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .default(true)
        .interact()?
    {
        run_async(manager.install(normalized))?;
        Ok(normalized.to_string())
    } else {
        anyhow::bail!("Ruby setup cancelled");
    }
}

fn ensure_java_runtime(version: &str) -> Result<String> {
    let normalized = version.trim();
    let manager = crate::runtimes::JavaManager::new();
    let installed = manager.list_installed().unwrap_or_default();
    if installed.iter().any(|v| v == normalized) {
        return Ok(normalized.to_string());
    }

    let prompt = format!("Java '{normalized}' is missing. Install now?");
    if Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .default(true)
        .interact()?
    {
        run_async(manager.install(normalized))?;
        Ok(normalized.to_string())
    } else {
        anyhow::bail!("Java setup cancelled");
    }
}

fn detect_js_package_manager(current_dir: &std::path::Path) -> Option<String> {
    if !current_dir.join("package.json").exists() {
        return None;
    }

    if let Ok(file) = std::fs::File::open(current_dir.join("package.json"))
        && let Ok(pkg) = serde_json::from_reader::<_, PackageJson>(file)
        && let Some(package_manager) = pkg.package_manager
        && let Some(name) = parse_package_manager_name(&package_manager)
    {
        return Some(name);
    }

    if current_dir.join("bun.lockb").exists() {
        return Some("bun".to_string());
    }
    if current_dir.join("pnpm-lock.yaml").exists() {
        return Some("pnpm".to_string());
    }
    if current_dir.join("yarn.lock").exists() {
        return Some("yarn".to_string());
    }
    if current_dir.join("package-lock.json").exists()
        || current_dir.join("npm-shrinkwrap.json").exists()
    {
        return Some("npm".to_string());
    }

    Some("bun".to_string())
}

fn detect_js_runtime(current_dir: &std::path::Path) -> Option<(String, String)> {
    let package_manager = detect_js_package_manager(current_dir)?;
    let runtime = if package_manager == "bun" {
        "bun"
    } else {
        "node"
    };
    let default_version = if runtime == "bun" { "latest" } else { "lts" };
    Some((runtime.to_string(), default_version.to_string()))
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

/// Detect available tasks in the current directory
pub fn detect_tasks() -> Result<Vec<Task>> {
    let mut tasks = Vec::new();
    let current_dir = std::env::current_dir()?;

    // 1. Node.js / Bun (package.json)
    if let Some(package_manager) = detect_js_package_manager(&current_dir) {
        if let Ok(file) = std::fs::File::open(current_dir.join("package.json"))
            && let Ok(pkg) = serde_json::from_reader::<_, PackageJson>(file)
            && let Some(scripts) = pkg.scripts
        {
            for (name, _) in scripts {
                tasks.push(Task {
                    name: name.clone(),
                    command: package_manager.clone(),
                    args: vec!["run".to_string(), name],
                    source: "package.json".to_string(),
                });
            }
        }

        tasks.push(Task {
            name: "install".to_string(),
            command: package_manager,
            args: vec!["install".to_string()],
            source: "package.json".to_string(),
        });
    }

    // 2. Deno (deno.json)
    if let Ok(file) = std::fs::File::open(current_dir.join("deno.json"))
        && let Ok(pkg) = serde_json::from_reader::<_, DenoJson>(file)
        && let Some(dtasks) = pkg.tasks
    {
        for (name, _) in dtasks {
            tasks.push(Task {
                name: name.clone(),
                command: "deno".to_string(),
                args: vec!["task".to_string(), name],
                source: "deno.json".to_string(),
            });
        }
    }

    // 3. PHP (composer.json)
    if let Ok(file) = std::fs::File::open(current_dir.join("composer.json"))
        && let Ok(pkg) = serde_json::from_reader::<_, ComposerJson>(file)
        && let Some(scripts) = pkg.scripts
    {
        for (name, _) in scripts {
            tasks.push(Task {
                name: name.clone(),
                command: "composer".to_string(),
                args: vec!["run-script".to_string(), name],
                source: "composer.json".to_string(),
            });
        }
    }

    // 4. Rust (Cargo.toml)
    if current_dir.join("Cargo.toml").exists() {
        let standard_tasks = vec!["build", "test", "check", "run", "clippy", "fmt"];
        for t in standard_tasks {
            tasks.push(Task {
                name: t.to_string(),
                command: "cargo".to_string(),
                args: vec![t.to_string()],
                source: "Cargo.toml".to_string(),
            });
        }
    }

    // 5. Makefile
    if current_dir.join("Makefile").exists()
        && let Ok(content) = std::fs::read_to_string(current_dir.join("Makefile"))
    {
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
                    });
                }
            }
        }
    }

    // 6. Go (Taskfile)
    if current_dir.join("Taskfile.yml").exists() || current_dir.join("Taskfile.yaml").exists() {
        // We assume 'task' binary is available or installed by omg
        // Parsing Taskfile is complex without YAML parser, so we rely on fallback mostly,
        // but we can register standard 'build', 'test' blindly if we want?
        // Better: rely on fallback. But user asked for detection.
        // I'll add a generic entry point.
        tasks.push(Task {
            name: "list".to_string(),
            command: "task".to_string(),
            args: vec!["--list".to_string()],
            source: "Taskfile.yml".to_string(),
        });
    }

    // 7. Ruby (Rakefile)
    if current_dir.join("Rakefile").exists() {
        tasks.push(Task {
            name: "tasks".to_string(),
            command: "rake".to_string(),
            args: vec!["-T".to_string()],
            source: "Rakefile".to_string(),
        });
    }

    // 8. Python (Poetry)
    if let Ok(content) = std::fs::read_to_string(current_dir.join("pyproject.toml"))
        && let Ok(proj) = toml::from_str::<PyProject>(&content)
        && let Some(tool) = proj.tool
        && let Some(poetry) = tool.poetry
        && let Some(scripts) = poetry.scripts
    {
        for (name, _) in scripts {
            tasks.push(Task {
                name: name.clone(),
                command: "poetry".to_string(),
                args: vec!["run".to_string(), name],
                source: "pyproject.toml".to_string(),
            });
        }
    }

    // 9. Python (Pipenv)
    if current_dir.join("Pipfile").exists() {
        // Pipenv scripts are in [scripts] section of Pipfile.
        // Pipfile is TOML-like.
        // If we can parse it as TOML, great.
        if let Ok(content) = std::fs::read_to_string(current_dir.join("Pipfile")) {
            // Basic manual parsing for [scripts]
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
                    });
                }
            }
        }
    }

    // 10. Java (Maven/Gradle)
    if current_dir.join("pom.xml").exists() {
        for t in ["clean", "compile", "test", "package", "install"] {
            tasks.push(Task {
                name: t.to_string(),
                command: "mvn".to_string(),
                args: vec![t.to_string()],
                source: "pom.xml".to_string(),
            });
        }
    }
    if current_dir.join("build.gradle").exists() || current_dir.join("build.gradle.kts").exists() {
        for t in ["build", "test", "run", "clean"] {
            tasks.push(Task {
                name: t.to_string(),
                command: "./gradlew".to_string(),
                args: vec![t.to_string()],
                source: "build.gradle".to_string(),
            });
        }
    }

    Ok(tasks)
}

/// Execute a task
pub fn run_task(
    task_name: &str,
    extra_args: &[String],
    backend_override: Option<RuntimeBackend>,
) -> Result<()> {
    let tasks = detect_tasks()?;

    // Find the task
    let matches: Vec<&Task> = tasks.iter().filter(|t| t.name == task_name).collect();

    if matches.is_empty() {
        // Fallback: Smart guessing
        let current_dir = std::env::current_dir()?;

        if current_dir.join("Makefile").exists() {
            println!(
                "{} Task '{}' not explicitly found, trying 'make {}'...",
                "→".yellow(),
                task_name,
                task_name
            );
            return execute_process(
                "make",
                &[task_name.to_string()],
                extra_args,
                backend_override,
            );
        }

        if current_dir.join("package.json").exists() {
            println!(
                "{} Task '{}' not explicitly found, trying 'npm run {}'...",
                "→".yellow(),
                task_name,
                task_name
            );
            return execute_process(
                "npm",
                &["run".to_string(), task_name.to_string()],
                extra_args,
                backend_override,
            );
        }

        if current_dir.join("Taskfile.yml").exists() || current_dir.join("Taskfile.yaml").exists() {
            println!(
                "{} Task '{}' not parsed, trying 'task {}'...",
                "→".yellow(),
                task_name,
                task_name
            );
            return execute_process(
                "task",
                &[task_name.to_string()],
                extra_args,
                backend_override,
            );
        }

        if current_dir.join("Rakefile").exists() {
            println!(
                "{} Task '{}' not parsed, trying 'rake {}'...",
                "→".yellow(),
                task_name,
                task_name
            );
            return execute_process(
                "rake",
                &[task_name.to_string()],
                extra_args,
                backend_override,
            );
        }

        if current_dir.join("Pipfile").exists() {
            println!(
                "{} Task '{}' not explicitly found, trying 'pipenv run {}'...",
                "→".yellow(),
                task_name,
                task_name
            );
            return execute_process(
                "pipenv",
                &["run".to_string(), task_name.to_string()],
                extra_args,
                backend_override,
            );
        }

        if current_dir.join("deno.json").exists() {
            println!(
                "{} Task '{}' not explicitly found, trying 'deno task {}'...",
                "→".yellow(),
                task_name,
                task_name
            );
            return execute_process(
                "deno",
                &["task".to_string(), task_name.to_string()],
                extra_args,
                backend_override,
            );
        }

        if current_dir.join("composer.json").exists() {
            println!(
                "{} Task '{}' not explicitly found, trying 'composer run-script {}'...",
                "→".yellow(),
                task_name,
                task_name
            );
            return execute_process(
                "composer",
                &["run-script".to_string(), task_name.to_string()],
                extra_args,
                backend_override,
            );
        }

        // Final fallback: run the command directly with runtime-aware PATH
        return execute_process(task_name, &[], extra_args, backend_override);
    }

    let Some(task) = matches.first() else {
        return execute_process(task_name, &[], extra_args, backend_override);
    };

    println!(
        "{} Running task '{}' via {}...",
        "OMG".cyan().bold(),
        task.name.white().bold(),
        task.source.blue()
    );

    execute_process(&task.command, &task.args, extra_args, backend_override)
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
    let mut versions = hooks::detect_versions(&current_dir);
    if let Some((runtime, default_version)) = detect_js_runtime(&current_dir) {
        versions.entry(runtime).or_insert(default_version);
    }
    ensure_js_package_manager(cmd)?;
    let settings = Settings::load()?;
    let backend = backend_override.unwrap_or(settings.runtime_backend);
    if backend != RuntimeBackend::Mise {
        if let Some(node_version) = versions.get("node").cloned() {
            let resolved = ensure_node_runtime(&node_version)?;
            versions.insert("node".to_string(), resolved);
        }
        if let Some(bun_version) = versions.get("bun").cloned() {
            let resolved = ensure_bun_runtime(&bun_version)?;
            versions.insert("bun".to_string(), resolved);
        }
        if let Some(python_version) = versions.get("python").cloned() {
            let resolved = ensure_python_runtime(&python_version)?;
            versions.insert("python".to_string(), resolved);
        }
        if let Some(go_version) = versions.get("go").cloned() {
            let resolved = ensure_go_runtime(&go_version)?;
            versions.insert("go".to_string(), resolved);
        }
        if let Some(ruby_version) = versions.get("ruby").cloned() {
            let resolved = ensure_ruby_runtime(&ruby_version)?;
            versions.insert("ruby".to_string(), resolved);
        }
        if let Some(java_version) = versions.get("java").cloned() {
            let resolved = ensure_java_runtime(&java_version)?;
            versions.insert("java".to_string(), resolved);
        }
    }
    let mut path_additions = match backend {
        RuntimeBackend::Mise => Vec::new(),
        _ => hooks::build_path_additions(&versions),
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
    let installed = node_manager.list_installed().unwrap_or_default();
    if installed.iter().any(|v| v == normalized) {
        return Ok(normalized.to_string());
    }

    // Check nvm-managed Node
    if let Some(nvm_version) = nvm_resolve_version(normalized) {
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
    let installed = bun_manager.list_installed().unwrap_or_default();
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

fn nvm_resolve_version(version: &str) -> Option<String> {
    let nvm_dir = std::env::var_os("NVM_DIR")
        .map(PathBuf::from)
        .or_else(|| home::home_dir().map(|dir| dir.join(".nvm")));
    let nvm_dir = nvm_dir?;

    let resolved = resolve_nvm_alias(&nvm_dir, version).unwrap_or_else(|| version.to_string());
    let normalized = resolved.trim_start_matches('v');
    let bin_path = nvm_dir
        .join("versions/node")
        .join(format!("v{normalized}"))
        .join("bin");
    if bin_path.exists() {
        Some(normalized.to_string())
    } else {
        None
    }
}

fn resolve_nvm_alias(nvm_dir: &std::path::Path, alias: &str) -> Option<String> {
    let alias_path = nvm_dir.join("alias").join(alias);
    if !alias_path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(alias_path).ok()?;
    let resolved = content.trim();
    if resolved.is_empty() {
        None
    } else {
        Some(resolved.to_string())
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
            .args(["prepare", &format!("{command}@latest"), "--activate"])
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

fn find_in_path(binary: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(binary);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn add_mise_path_fallbacks(
    versions: &HashMap<String, String>,
    path_additions: &mut Vec<String>,
    backend: RuntimeBackend,
) {
    if !matches!(
        backend,
        RuntimeBackend::Mise | RuntimeBackend::NativeThenMise
    ) {
        return;
    }

    if !mise_available() {
        return;
    }

    let mut seen: HashSet<String> = path_additions.iter().cloned().collect();
    for (runtime, version) in versions {
        if backend == RuntimeBackend::NativeThenMise
            && native_runtime_bin_path(runtime, version).is_some()
        {
            continue;
        }

        if let Some(bin_dir) = mise_runtime_bin_path(runtime, version) {
            let bin = bin_dir.display().to_string();
            if seen.insert(bin.clone()) {
                path_additions.push(bin);
            }
        }
    }
}

fn native_runtime_bin_path(runtime: &str, version: &str) -> Option<PathBuf> {
    let data_dir = paths::data_dir();
    let bin_path = match runtime {
        "node" => data_dir.join("versions/node").join(version).join("bin"),
        "python" => data_dir.join("versions/python").join(version).join("bin"),
        "go" => data_dir.join("versions/go").join(version).join("bin"),
        "ruby" => data_dir.join("versions/ruby").join(version).join("bin"),
        "java" => data_dir.join("versions/java").join(version).join("bin"),
        "bun" => data_dir.join("versions/bun").join(version),
        "rust" => home::home_dir().unwrap_or_default().join(".cargo/bin"),
        _ => return None,
    };

    if bin_path.exists() {
        Some(bin_path)
    } else {
        None
    }
}

fn mise_available() -> bool {
    find_in_path("mise").is_some()
}

fn mise_runtime_bin_path(runtime: &str, version: &str) -> Option<PathBuf> {
    let tool_spec = if version.is_empty() {
        runtime.to_string()
    } else {
        format!("{runtime}@{version}")
    };

    let output = Command::new("mise")
        .args(["where", &tool_spec])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let install_dir = PathBuf::from(stdout.trim());
    if install_dir.as_os_str().is_empty() {
        return None;
    }

    let bin_dir = install_dir.join("bin");
    if bin_dir.exists() {
        Some(bin_dir)
    } else if install_dir.exists() {
        Some(install_dir)
    } else {
        None
    }
}

/// Run a task in watch mode - re-run on file changes
pub async fn run_task_watch(
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

    // Initial run
    let _ = run_task(task_name, extra_args, backend_override);

    // Set up file watcher
    let (tx, rx) = channel();
    let mut watcher = RecommendedWatcher::new(
        move |res| {
            if let Ok(event) = res {
                let _ = tx.send(event);
            }
        },
        Config::default().with_poll_interval(Duration::from_millis(500)),
    )?;

    let current_dir = std::env::current_dir()?;

    // Watch common source directories
    let watch_dirs = ["src", "lib", "app", "pages", "components", "tests", "."];
    for dir in watch_dirs {
        let path = current_dir.join(dir);
        if path.exists() {
            let _ = watcher.watch(&path, RecursiveMode::Recursive);
        }
    }

    println!(
        "  {} Watching for changes...\n",
        "→".dimmed()
    );

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
                let _ = run_task(task_name, extra_args, backend_override);
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
        return run_task(tasks_str, extra_args, backend_override).map_err(Into::into);
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
