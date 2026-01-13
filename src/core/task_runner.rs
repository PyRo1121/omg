use anyhow::{Context, Result};
use colored::Colorize;
use serde::Deserialize;
use std::collections::HashMap;
use std::process::Command;

use crate::hooks;

#[derive(Debug, Clone)]
pub struct Task {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub source: String,
}

#[derive(Deserialize)]
struct PackageJson {
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
    if let Ok(file) = std::fs::File::open(current_dir.join("package.json")) {
        if let Ok(pkg) = serde_json::from_reader::<_, PackageJson>(file) {
            if let Some(scripts) = pkg.scripts {
                for (name, _) in scripts {
                    tasks.push(Task {
                        name: name.clone(),
                        command: "npm".to_string(),
                        args: vec!["run".to_string(), name],
                        source: "package.json".to_string(),
                    });
                }
            }
        }
    }

    // 2. Deno (deno.json)
    if let Ok(file) = std::fs::File::open(current_dir.join("deno.json")) {
        if let Ok(pkg) = serde_json::from_reader::<_, DenoJson>(file) {
            if let Some(dtasks) = pkg.tasks {
                for (name, _) in dtasks {
                    tasks.push(Task {
                        name: name.clone(),
                        command: "deno".to_string(),
                        args: vec!["task".to_string(), name],
                        source: "deno.json".to_string(),
                    });
                }
            }
        }
    }

    // 3. PHP (composer.json)
    if let Ok(file) = std::fs::File::open(current_dir.join("composer.json")) {
        if let Ok(pkg) = serde_json::from_reader::<_, ComposerJson>(file) {
            if let Some(scripts) = pkg.scripts {
                for (name, _) in scripts {
                    tasks.push(Task {
                        name: name.clone(),
                        command: "composer".to_string(),
                        args: vec!["run-script".to_string(), name],
                        source: "composer.json".to_string(),
                    });
                }
            }
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
    if current_dir.join("Makefile").exists() {
        if let Ok(content) = std::fs::read_to_string(current_dir.join("Makefile")) {
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
    if let Ok(content) = std::fs::read_to_string(current_dir.join("pyproject.toml")) {
        if let Ok(proj) = toml::from_str::<PyProject>(&content) {
            if let Some(tool) = proj.tool {
                if let Some(poetry) = tool.poetry {
                    if let Some(scripts) = poetry.scripts {
                        for (name, _) in scripts {
                            tasks.push(Task {
                                name: name.clone(),
                                command: "poetry".to_string(),
                                args: vec!["run".to_string(), name],
                                source: "pyproject.toml".to_string(),
                            });
                        }
                    }
                }
            }
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
                if in_scripts && !line.is_empty() && !line.starts_with('#') {
                    if let Some((key, _)) = line.split_once('=') {
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
pub fn run_task(task_name: &str, extra_args: &[String]) -> Result<()> {
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
            return execute_process("make", &[task_name.to_string()], extra_args);
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
            );
        }

        if current_dir.join("Taskfile.yml").exists() || current_dir.join("Taskfile.yaml").exists() {
            println!(
                "{} Task '{}' not parsed, trying 'task {}'...",
                "→".yellow(),
                task_name,
                task_name
            );
            return execute_process("task", &[task_name.to_string()], extra_args);
        }

        if current_dir.join("Rakefile").exists() {
            println!(
                "{} Task '{}' not parsed, trying 'rake {}'...",
                "→".yellow(),
                task_name,
                task_name
            );
            return execute_process("rake", &[task_name.to_string()], extra_args);
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
            );
        }

        anyhow::bail!("Task '{}' not found in this project.", task_name);
    }

    let task = matches[0];

    println!(
        "{} Running task '{}' via {}...",
        "OMG".cyan().bold(),
        task.name.white().bold(),
        task.source.blue()
    );

    execute_process(&task.command, &task.args, extra_args)
}

fn execute_process(cmd: &str, args: &[String], extra_args: &[String]) -> Result<()> {
    // Detect required runtime versions and inject them into PATH
    // This ensures 'npm' uses the correct node version, 'cargo' uses correct rust channel, etc.
    let current_dir = std::env::current_dir()?;
    let versions = hooks::detect_versions(&current_dir);
    let mut path_additions = hooks::build_path_additions(&versions);

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

    if !path_additions.is_empty() {
        if let Ok(current_path) = std::env::var("PATH") {
            let new_path = format!("{}:{}", path_additions.join(":"), current_path);
            command.env("PATH", new_path);
        }
    }

    let status = command
        .status()
        .with_context(|| format!("Failed to execute '{}'", cmd))?;

    if !status.success() {
        anyhow::bail!("Task failed with exit code: {:?}", status.code());
    }

    Ok(())
}
