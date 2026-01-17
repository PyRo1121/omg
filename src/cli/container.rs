//! Container CLI commands

use anyhow::Result;
use owo_colors::OwoColorize;

use crate::core::container::{
    ContainerConfig, ContainerManager, ContainerRuntime, detect_runtime, dev_container_config,
};

/// Show container runtime status
pub fn status() -> Result<()> {
    println!("{} Container Status\n", "OMG".cyan().bold());

    match detect_runtime() {
        Some(runtime) => {
            println!("  Runtime: {} ✓", runtime.to_string().green());
            
            let manager = ContainerManager::with_runtime(runtime);
            
            // List running containers
            match manager.list_running() {
                Ok(containers) if !containers.is_empty() => {
                    println!("\n  Running containers:");
                    for c in containers {
                        println!(
                            "    {} {} ({})",
                            "•".cyan(),
                            c.name.bold(),
                            c.image.dimmed()
                        );
                    }
                }
                Ok(_) => {
                    println!("\n  No running containers");
                }
                Err(e) => {
                    println!("\n  {} Failed to list containers: {}", "⚠".yellow(), e);
                }
            }
        }
        None => {
            println!("  Runtime: {} ✗", "Not found".red());
            println!("\n  Install Docker or Podman to use container features.");
            println!("    Docker: https://docs.docker.com/engine/install/");
            println!("    Podman: https://podman.io/getting-started/installation");
        }
    }

    Ok(())
}

/// Run a command in a container
pub fn run(image: &str, command: Vec<String>, name: Option<String>, detach: bool) -> Result<()> {
    let manager = ContainerManager::new()?;

    println!(
        "{} Running in {} container...",
        "OMG".cyan().bold(),
        image.cyan()
    );

    let config = ContainerConfig {
        image: image.to_string(),
        name,
        interactive: !detach,
        rm: !detach,
        ..Default::default()
    };

    let cmd_refs: Vec<&str> = command.iter().map(String::as_str).collect();
    let exit_code = manager.run(&config, &cmd_refs)?;

    if exit_code != 0 {
        std::process::exit(exit_code);
    }

    Ok(())
}

/// Start an interactive shell in a container
pub fn shell(image: Option<String>) -> Result<()> {
    let manager = ContainerManager::new()?;
    let cwd = std::env::current_dir()?;

    let config = if let Some(img) = image {
        ContainerConfig {
            image: img,
            ..dev_container_config(&cwd)
        }
    } else {
        dev_container_config(&cwd)
    };

    println!(
        "{} Starting shell in {} container...",
        "OMG".cyan().bold(),
        config.image.cyan()
    );
    println!("  Mounting: {} → /app", cwd.display().dimmed());

    let exit_code = manager.shell(&config)?;

    if exit_code != 0 {
        std::process::exit(exit_code);
    }

    Ok(())
}

/// Build a container image
pub fn build(dockerfile: Option<String>, tag: &str) -> Result<()> {
    let manager = ContainerManager::new()?;
    let cwd = std::env::current_dir()?;

    let dockerfile_path = dockerfile
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| cwd.join("Dockerfile"));

    if !dockerfile_path.exists() {
        anyhow::bail!(
            "Dockerfile not found: {}. Use --dockerfile to specify a path.",
            dockerfile_path.display()
        );
    }

    println!(
        "{} Building container image: {}",
        "OMG".cyan().bold(),
        tag.cyan()
    );

    manager.build(&dockerfile_path, tag, &cwd)?;

    println!("{} Image built successfully!", "✓".green());

    Ok(())
}

/// List running containers
pub fn list() -> Result<()> {
    let manager = ContainerManager::new()?;

    println!("{} Running Containers\n", "OMG".cyan().bold());

    let containers = manager.list_running()?;

    if containers.is_empty() {
        println!("  No running containers");
        return Ok(());
    }

    println!(
        "  {:<12} {:<20} {:<25} {}",
        "ID".bold(),
        "NAME".bold(),
        "IMAGE".bold(),
        "STATUS".bold()
    );

    for c in containers {
        println!(
            "  {:<12} {:<20} {:<25} {}",
            &c.id[..12.min(c.id.len())],
            c.name,
            c.image,
            c.status.green()
        );
    }

    Ok(())
}

/// List container images
pub fn images() -> Result<()> {
    let manager = ContainerManager::new()?;

    println!("{} Container Images\n", "OMG".cyan().bold());

    let images = manager.list_images()?;

    if images.is_empty() {
        println!("  No images found");
        return Ok(());
    }

    println!(
        "  {:<30} {:<15} {:<12} {}",
        "REPOSITORY".bold(),
        "TAG".bold(),
        "ID".bold(),
        "SIZE".bold()
    );

    for img in images {
        println!(
            "  {:<30} {:<15} {:<12} {}",
            img.repository,
            img.tag,
            &img.id[..12.min(img.id.len())],
            img.size
        );
    }

    Ok(())
}

/// Pull a container image
pub fn pull(image: &str) -> Result<()> {
    let manager = ContainerManager::new()?;

    println!(
        "{} Pulling image: {}",
        "OMG".cyan().bold(),
        image.cyan()
    );

    manager.pull(image)?;

    println!("{} Image pulled successfully!", "✓".green());

    Ok(())
}

/// Stop a running container
pub fn stop(container: &str) -> Result<()> {
    let manager = ContainerManager::new()?;

    println!(
        "{} Stopping container: {}",
        "OMG".cyan().bold(),
        container.cyan()
    );

    manager.stop(container)?;

    println!("{} Container stopped!", "✓".green());

    Ok(())
}

/// Execute a command in a running container
pub fn exec(container: &str, command: Vec<String>) -> Result<()> {
    let manager = ContainerManager::new()?;

    let cmd_refs: Vec<&str> = command.iter().map(String::as_str).collect();
    let exit_code = manager.exec(container, &cmd_refs, true)?;

    if exit_code != 0 {
        std::process::exit(exit_code);
    }

    Ok(())
}

/// Generate a Dockerfile for the current project
pub fn init(base_image: Option<String>) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let dockerfile_path = cwd.join("Dockerfile.omg");

    if dockerfile_path.exists() {
        anyhow::bail!(
            "Dockerfile.omg already exists. Remove it first or use a different name."
        );
    }

    let base = base_image.unwrap_or_else(|| "ubuntu:24.04".to_string());

    // Detect runtimes from project
    let mut runtimes: Vec<(&str, String)> = Vec::new();

    if cwd.join("package.json").exists() {
        runtimes.push(("node", "lts".to_string()));
    }
    if cwd.join("Cargo.toml").exists() {
        runtimes.push(("rust", "stable".to_string()));
    }
    if cwd.join("go.mod").exists() {
        runtimes.push(("go", "latest".to_string()));
    }
    if cwd.join("pyproject.toml").exists() || cwd.join("requirements.txt").exists() {
        runtimes.push(("python", "3.12".to_string()));
    }

    let manager = ContainerManager::new().unwrap_or_else(|_| {
        ContainerManager::with_runtime(ContainerRuntime::Docker)
    });

    let runtime_refs: Vec<(&str, &str)> = runtimes
        .iter()
        .map(|(r, v)| (*r, v.as_str()))
        .collect();

    let dockerfile = manager.generate_dockerfile(&base, &runtime_refs);

    std::fs::write(&dockerfile_path, dockerfile)?;

    println!("{} Created Dockerfile.omg", "✓".green());
    println!("  Base image: {}", base.cyan());
    
    if !runtimes.is_empty() {
        println!("  Detected runtimes:");
        for (rt, ver) in &runtimes {
            println!("    {} {}", rt.cyan(), ver.dimmed());
        }
    }

    println!("\n  Build with: {} container build -t myapp .", "omg".cyan());

    Ok(())
}
