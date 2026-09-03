use anyhow::{Context, Result};
use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use crate::cli::style;

fn require_command_success(status: std::process::ExitStatus, operation: &str) -> Result<()> {
    anyhow::ensure!(status.success(), "{operation} failed with {status}");
    Ok(())
}

fn canonical_stack(stack: &str) -> Option<&'static str> {
    match stack.to_ascii_lowercase().as_str() {
        "rust" | "rs" => Some("rust"),
        "react" | "react-ts" => Some("react"),
        "node" | "ts" | "typescript" => Some("node"),
        "python" | "py" => Some("python"),
        "go" | "golang" => Some("go"),
        _ => None,
    }
}

/// Create a new project
pub fn run(stack: &str, name: &str) -> Result<()> {
    // SECURITY: Validate project name (reuse package name rules as they are safe for directories)
    crate::core::security::validate_package_name(name)?;

    let target_dir = std::env::current_dir()?.join(name);
    if target_dir.exists() {
        anyhow::bail!("Directory '{name}' already exists");
    }

    println!(
        "{} Creating new {} project: {}\n",
        style::header("OMG Scaffolder"),
        style::info(stack),
        style::package(name)
    );

    let canonical_stack = canonical_stack(stack).ok_or_else(|| {
        anyhow::anyhow!("Unknown stack: {stack}. Supported: rust, react, node, python, go")
    })?;
    match canonical_stack {
        "rust" => scaffold_rust(name)?,
        "react" => scaffold_react(name)?,
        "node" => scaffold_node(name)?,
        "python" => scaffold_python(name)?,
        "go" => scaffold_go(name)?,
        _ => unreachable!("canonical_stack returned an unknown family"),
    }

    // Post-scaffold setup
    println!("\n{}", style::header("Finalizing..."));

    // 1. Create .tool-versions if not present
    let tool_versions_path = target_dir.join(".tool-versions");
    if !tool_versions_path.exists() {
        lock_runtimes(&target_dir, canonical_stack)?;
    }

    // 2. Initialize Git if not present
    if !target_dir.join(".git").exists() {
        println!("  {} Initializing git...", style::dim("→"));
        let status = Command::new("git")
            .arg("init")
            .current_dir(&target_dir)
            .status()
            .context("Failed to launch git init")?;
        require_command_success(status, "git init")?;
    }

    println!("\n{}", style::success("Project created successfully! 🚀"));
    println!("\nTo get started:");
    println!("  cd {name}");
    println!("  omg run dev  (or build/start)");

    Ok(())
}

fn scaffold_rust(name: &str) -> Result<()> {
    let pb = style::spinner("Running cargo new...");
    let status = Command::new("cargo").args(["new", name]).status()?;
    pb.finish_and_clear();

    require_command_success(status, "cargo new")?;

    println!("  {} Created Cargo project", style::success("✓"));
    Ok(())
}

fn scaffold_react(name: &str) -> Result<()> {
    let pb = style::spinner("Running npm create vite...");
    // npm create vite@latest my-app -- --template react-ts
    let status = Command::new("npm")
        .args([
            "create",
            "vite@latest",
            "--",
            name,
            "--template",
            "react-ts",
        ])
        .status()?;
    pb.finish_and_clear();

    require_command_success(status, "npm create vite")?;

    println!("  {} Created React (Vite+TS) project", style::success("✓"));

    // Auto install
    println!("  {} Installing dependencies...", style::dim("→"));
    let status = Command::new("npm")
        .args(["install", "--ignore-scripts"])
        .current_dir(name)
        .status()
        .context("Failed to launch npm install")?;
    require_command_success(status, "npm install")?;

    Ok(())
}

fn scaffold_node(name: &str) -> Result<()> {
    // Minimal Node+TS setup
    std::fs::create_dir_all(name)?;
    let root = Path::new(name);

    // package.json
    let pkg_json = format!(
        r#"{{
  "name": "{name}",
  "version": "0.1.0",
  "scripts": {{
    "start": "ts-node src/index.ts",
    "build": "tsc",
    "dev": "ts-node-dev --respawn src/index.ts"
  }},
  "dependencies": {{}},
  "devDependencies": {{
    "typescript": "latest",
    "ts-node": "latest",
    "ts-node-dev": "latest",
    "@types/node": "latest"
  }}
}}"#
    );
    std::fs::write(root.join("package.json"), pkg_json)?;

    // tsconfig.json
    std::fs::write(
        root.join("tsconfig.json"),
        r#"{
  "compilerOptions": {
    "target": "es2020",
    "module": "commonjs",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "outDir": "./dist"
  }
}"#,
    )?;

    // src/index.ts
    std::fs::create_dir_all(root.join("src"))?;
    std::fs::write(
        root.join("src/index.ts"),
        r#"console.log("Hello from OMG!");"#,
    )?;

    // .gitignore
    std::fs::write(root.join(".gitignore"), "node_modules\ndist\n.env\n")?;

    println!("  {} Created Node+TS template", style::success("✓"));

    println!("  {} Installing dependencies...", style::dim("→"));
    let status = Command::new("npm")
        .args(["install", "--ignore-scripts"])
        .current_dir(root)
        .status()
        .context("Failed to launch npm install")?;
    require_command_success(status, "npm install")?;

    Ok(())
}

fn scaffold_python(name: &str) -> Result<()> {
    let pb = style::spinner("Running poetry new...");
    let status = Command::new("poetry").args(["new", name]).status();

    pb.finish_and_clear();

    match status {
        Ok(status) => require_command_success(status, "poetry new")?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            println!(
                "  {} Poetry not found, using venv fallback...",
                style::warning("⚠")
            );
            std::fs::create_dir_all(name)?;
            let status = Command::new("python")
                .args(["-m", "venv", ".venv"])
                .current_dir(name)
                .status()
                .context("Failed to launch python venv")?;
            require_command_success(status, "python venv")?;
            std::fs::write(Path::new(name).join("requirements.txt"), "")?;
            std::fs::write(Path::new(name).join("main.py"), "print('Hello from OMG!')")?;
            return Ok(());
        }
        Err(error) => return Err(error).context("Failed to launch poetry new"),
    }

    println!("  {} Created Python (Poetry) project", style::success("✓"));
    Ok(())
}

fn scaffold_go(name: &str) -> Result<()> {
    std::fs::create_dir_all(name)?;
    let root = Path::new(name);

    // go mod init
    let status = Command::new("go")
        .args(["mod", "init", name])
        .current_dir(root)
        .status()
        .context("Failed to launch go mod init")?;
    require_command_success(status, "go mod init")?;

    // main.go
    std::fs::write(
        root.join("main.go"),
        r#"package main

import "fmt"

func main() {
	fmt.Println("Hello from OMG!")
}
"#,
    )?;

    // Taskfile.yml
    std::fs::write(
        root.join("Taskfile.yml"),
        r"version: '3'

tasks:
  build:
    cmds:
      - go build -v .
  run:
    cmds:
      - go run main.go
",
    )?;

    println!("  {} Created Go project", style::success("✓"));
    Ok(())
}

/// Detect current runtime versions and lock them
fn lock_runtimes(target_dir: &Path, stack: &str) -> Result<()> {
    let mut content = String::new();

    // Helper to get version
    let get_ver = |cmd: &str, args: &[&str]| -> Option<String> {
        if let Ok(output) = Command::new(cmd).args(args).output() {
            let s = String::from_utf8_lossy(&output.stdout);
            // Parse "v1.2.3" or "cargo 1.2.3 (date)"
            // Simple approach: look for first thing that looks like a version
            // For now, let's just grab the whole string or format it
            // Node: "v18.1.0\n" -> "18.1.0"
            if cmd == "node" {
                return Some(s.trim().trim_start_matches('v').to_string());
            }
            // Python: "Python 3.11.0\n" -> "3.11.0"
            if cmd == "python"
                && let Some(v) = s.split_whitespace().nth(1)
            {
                return Some(v.to_string());
            }
            // Go: "go version go1.21.0 linux/amd64" -> "1.21.0"
            if cmd == "go"
                && let Some(v) = s.split_whitespace().nth(2)
            {
                return Some(v.trim_start_matches("go").to_string());
            }
            // Rust: "rustc 1.75.0 (....)" -> "1.75.0"
            if cmd == "rustc"
                && let Some(v) = s.split_whitespace().nth(1)
            {
                return Some(v.to_string());
            }
        }
        None
    };

    match stack.to_lowercase().as_str() {
        "react" | "node" | "ts" => {
            if let Some(v) = get_ver("node", &["--version"]) {
                let _ = writeln!(content, "node {v}");
                println!("  {} Locked node to {}", style::success("✓"), v);
            }
        }
        "python" => {
            if let Some(v) = get_ver("python", &["--version"]) {
                let _ = writeln!(content, "python {v}");
                println!("  {} Locked python to {}", style::success("✓"), v);
            }
        }
        "go" => {
            if let Some(v) = get_ver("go", &["version"]) {
                let _ = writeln!(content, "go {v}");
                println!("  {} Locked go to {}", style::success("✓"), v);
            }
        }
        "rust" => {
            if let Some(v) = get_ver("rustc", &["--version"]) {
                let _ = writeln!(content, "rust {v}");
                println!("  {} Locked rust to {}", style::success("✓"), v);
            }
        }
        _ => {}
    }

    if !content.is_empty() {
        std::fs::write(target_dir.join(".tool-versions"), content)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_aliases_share_their_runtime_lock_family() {
        assert_eq!(canonical_stack("react-ts"), Some("react"));
        assert_eq!(canonical_stack("typescript"), Some("node"));
        assert_eq!(canonical_stack("py"), Some("python"));
        assert_eq!(canonical_stack("golang"), Some("go"));
        assert_eq!(canonical_stack("rs"), Some("rust"));
    }

    #[test]
    fn failed_scaffold_commands_are_errors() {
        let status = Command::new("sh")
            .args(["-c", "exit 23"])
            .status()
            .expect("run fixture command");

        let error = require_command_success(status, "fixture scaffold")
            .expect_err("non-zero scaffold command must fail");
        assert!(error.to_string().contains("fixture scaffold"));
        assert!(error.to_string().contains("23"));
    }
}
