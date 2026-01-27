//! Container CLI commands

use anyhow::Result;

use crate::cli::components::Components;
use crate::cli::tea::Cmd;
use crate::cli::{CliContext, ContainerCommands, LocalCommandRunner};
use crate::core::container::{
    ContainerConfig, ContainerManager, ContainerRuntime, detect_runtime, dev_container_config,
};

impl LocalCommandRunner for ContainerCommands {
    async fn execute(&self, _ctx: &CliContext) -> Result<()> {
        match self {
            ContainerCommands::Status => status(),
            ContainerCommands::Run {
                image,
                command,
                name,
                detach,
                interactive,
                env,
                volume,
                workdir,
            } => run(
                image,
                command,
                name.clone(),
                *detach,
                *interactive,
                env,
                volume,
                workdir.clone(),
            ),
            ContainerCommands::Shell {
                image,
                workdir,
                env,
                volume,
            } => shell(image.clone(), workdir.clone(), env, volume),
            ContainerCommands::Build {
                dockerfile,
                tag,
                no_cache,
                build_arg,
                target,
            } => build(dockerfile.clone(), tag, *no_cache, build_arg, target),
            ContainerCommands::List => list(),
            ContainerCommands::Images => images(),
            ContainerCommands::Pull { image } => pull(image),
            ContainerCommands::Stop { container } => stop(container),
            ContainerCommands::Exec { container, command } => exec(container, command),
            ContainerCommands::Init { base } => init(base.clone()),
        }
    }
}

/// Parse environment variables from KEY=VALUE format
fn parse_env_vars(env: &[String]) -> Vec<(String, String)> {
    env.iter()
        .filter_map(|e| {
            let parts: Vec<&str> = e.splitn(2, '=').collect();
            if parts.len() == 2 {
                Some((parts[0].to_string(), parts[1].to_string()))
            } else {
                None
            }
        })
        .collect()
}

/// Parse volume mounts from host:container format
fn parse_volumes(volumes: &[String]) -> Vec<(String, String)> {
    volumes
        .iter()
        .filter_map(|v| {
            let parts: Vec<&str> = v.splitn(2, ':').collect();
            if parts.len() == 2 {
                Some((parts[0].to_string(), parts[1].to_string()))
            } else {
                None
            }
        })
        .collect()
}

/// Show container runtime status
pub fn status() -> Result<()> {
    use crate::cli::packages::execute_cmd;

    let output = if let Some(runtime) = detect_runtime() {
        let runtime_str = runtime.to_string();
        let manager = ContainerManager::with_runtime(runtime);

        // List running containers
        match manager.list_running() {
            Ok(containers) if !containers.is_empty() => {
                let container_list: Vec<String> = containers
                    .iter()
                    .map(|c| format!("{} {} ({})", "•", c.name, c.image))
                    .collect();

                Cmd::batch([
                    Cmd::header("Container Status", format!("Runtime: {runtime_str}")),
                    Cmd::spacer(),
                    Cmd::card("Running Containers", container_list),
                    Components::complete("Container status retrieved"),
                ])
            }
            Ok(_) => Cmd::batch([
                Cmd::header("Container Status", format!("Runtime: {runtime_str}")),
                Cmd::spacer(),
                Cmd::info("No running containers"),
            ]),
            Err(e) => Cmd::batch([
                Cmd::header("Container Status", format!("Runtime: {runtime_str}")),
                Cmd::spacer(),
                Cmd::error(format!("Failed to list containers: {e}")),
            ]),
        }
    } else {
        Cmd::batch([
            Cmd::header("Container Status", "Runtime: Not found"),
            Cmd::spacer(),
            Components::error_with_suggestion(
                "No container runtime detected",
                "Install Docker or Podman to use container features",
            ),
            Cmd::println("\n  Installation guides:"),
            Cmd::println("    Docker: https://docs.docker.com/engine/install/"),
            Cmd::println("    Podman: https://podman.io/getting-started/installation"),
        ])
    };

    execute_cmd(output);
    Ok(())
}

/// Run a command in a container
#[allow(clippy::too_many_arguments)] // Container config has many options
pub fn run(
    image: &str,
    command: &[String],
    name: Option<String>,
    detach: bool,
    interactive: bool,
    env: &[String],
    volumes: &[String],
    workdir: Option<String>,
) -> Result<()> {
    use crate::cli::packages::execute_cmd;

    // SECURITY: Validate image name and container name
    if image.chars().any(|c| c.is_control() || c == ';') {
        execute_cmd(Components::error_with_suggestion(
            "Invalid image name",
            "Image names must not contain control characters or semicolons",
        ));
        anyhow::bail!("Invalid image name");
    }
    if let Some(ref n) = name
        && n.chars()
            .any(|c| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
    {
        execute_cmd(Components::error_with_suggestion(
            "Invalid container name",
            "Container names must be alphanumeric with hyphens or underscores only",
        ));
        anyhow::bail!("Invalid container name");
    }

    let manager = ContainerManager::new()?;

    execute_cmd(Components::loading(format!(
        "Running in {image} container..."
    )));

    let env_pairs = parse_env_vars(env);
    let volume_pairs = parse_volumes(volumes);

    let config = ContainerConfig {
        image: image.to_string(),
        name,
        interactive: interactive || !detach,
        rm: !detach,
        env: env_pairs,
        volumes: volume_pairs,
        workdir,
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
pub fn shell(
    image: Option<String>,
    workdir: Option<String>,
    env: &[String],
    volumes: &[String],
) -> Result<()> {
    use crate::cli::packages::execute_cmd;

    let manager = ContainerManager::new()?;
    let cwd = std::env::current_dir()?;

    let mut env_pairs = parse_env_vars(env);
    let mut volume_pairs = parse_volumes(volumes);

    let mut config = if let Some(img) = image {
        ContainerConfig {
            image: img,
            ..dev_container_config(&cwd)
        }
    } else {
        dev_container_config(&cwd)
    };

    // Merge env and volumes
    config.env.append(&mut env_pairs);
    config.volumes.append(&mut volume_pairs);

    // Override workdir if specified
    if workdir.is_some() {
        config.workdir = workdir;
    }

    let mut details = vec![
        format!("Image: {}", config.image),
        format!("Mount: {} → /app", cwd.display()),
    ];

    if !config.env.is_empty() {
        details.push(format!("Environment: {} variable(s)", config.env.len()));
    }
    if config.volumes.len() > 1 {
        details.push(format!("Additional mounts: {}", config.volumes.len() - 1));
    }

    execute_cmd(Cmd::batch([
        Components::loading(format!("Starting shell in {} container...", config.image)),
        Cmd::card("Container Configuration", details),
    ]));

    let exit_code = manager.shell(&config)?;

    if exit_code != 0 {
        std::process::exit(exit_code);
    }

    Ok(())
}

/// Build a container image
pub fn build(
    dockerfile: Option<String>,
    tag: &str,
    no_cache: bool,
    build_args: &[String],
    target: &Option<String>,
) -> Result<()> {
    use crate::cli::packages::execute_cmd;

    // SECURITY: Validate tag and paths
    if tag.chars().any(|c| c.is_control() || c == ';') {
        execute_cmd(Components::error_with_suggestion(
            "Invalid tag name",
            "Tags must not contain control characters or semicolons",
        ));
        anyhow::bail!("Invalid tag name");
    }
    if let Some(ref df) = dockerfile
        && let Err(e) = crate::core::security::validate_relative_path(df)
    {
        execute_cmd(Components::error_with_suggestion(
            "Invalid Dockerfile path",
            format!("Path validation failed: {e}"),
        ));
        return Err(e);
    }

    let manager = ContainerManager::new()?;
    let cwd = std::env::current_dir()?;

    let dockerfile_path =
        dockerfile.map_or_else(|| cwd.join("Dockerfile"), std::path::PathBuf::from);

    if !dockerfile_path.exists() {
        let error_msg = format!("Dockerfile not found: {}", dockerfile_path.display());
        execute_cmd(Components::error_with_suggestion(
            &error_msg,
            "Use -f/--dockerfile to specify a path",
        ));
        anyhow::bail!("{error_msg}");
    }

    let mut build_details = vec![
        format!("Tag: {}", tag),
        format!("Dockerfile: {}", dockerfile_path.display()),
    ];

    if no_cache {
        build_details.push("Cache: Disabled".to_string());
    }

    if let Some(t) = target {
        build_details.push(format!("Target: {t}"));
    }

    execute_cmd(Cmd::batch([
        Components::loading(format!("Building image: {tag}")),
        Cmd::card("Build Configuration", build_details),
    ]));

    manager.build_with_options(
        &dockerfile_path,
        tag,
        &cwd,
        no_cache,
        build_args,
        target.as_deref(),
    )?;

    execute_cmd(Components::complete(format!(
        "Image {tag} built successfully"
    )));

    Ok(())
}

/// List running containers
pub fn list() -> Result<()> {
    use crate::cli::packages::execute_cmd;

    let manager = ContainerManager::new()?;

    let containers = manager.list_running()?;

    if containers.is_empty() {
        execute_cmd(Cmd::batch([
            Cmd::header("Running Containers", "No active containers"),
            Cmd::spacer(),
        ]));
        return Ok(());
    }

    let container_list: Vec<String> = containers
        .iter()
        .map(|c| {
            format!(
                "{:<12} {:<20} {:<25} {}",
                &c.id[..12.min(c.id.len())],
                c.name,
                c.image,
                c.status
            )
        })
        .collect();

    execute_cmd(Cmd::batch([
        Cmd::header(
            "Running Containers",
            format!("{} container(s) running", containers.len()),
        ),
        Cmd::spacer(),
        Cmd::card("Active Containers", container_list),
    ]));

    Ok(())
}

/// List container images
pub fn images() -> Result<()> {
    use crate::cli::packages::execute_cmd;

    let manager = ContainerManager::new()?;

    let images = manager.list_images()?;

    if images.is_empty() {
        execute_cmd(Cmd::batch([
            Cmd::header("Container Images", "No images found"),
            Cmd::spacer(),
        ]));
        return Ok(());
    }

    let image_list: Vec<String> = images
        .iter()
        .map(|img| {
            format!(
                "{:<30} {:<15} {:<12} {}",
                img.repository,
                img.tag,
                &img.id[..12.min(img.id.len())],
                img.size
            )
        })
        .collect();

    execute_cmd(Cmd::batch([
        Cmd::header(
            "Container Images",
            format!("{} image(s) available", images.len()),
        ),
        Cmd::spacer(),
        Cmd::card("Available Images", image_list),
    ]));

    Ok(())
}

/// Pull a container image
pub fn pull(image: &str) -> Result<()> {
    use crate::cli::packages::execute_cmd;

    if image
        .chars()
        .any(|c| c.is_control() || c == ';' || c == '|' || c == '&')
    {
        execute_cmd(Components::error_with_suggestion(
            "Invalid image name",
            "Image names must not contain control characters or shell operators",
        ));
        anyhow::bail!("Invalid image name");
    }

    let manager = ContainerManager::new()?;

    execute_cmd(Components::loading(format!("Pulling image: {image}")));

    manager.pull(image)?;

    execute_cmd(Components::complete(format!(
        "Image {image} pulled successfully"
    )));

    Ok(())
}

/// Stop a running container
pub fn stop(container: &str) -> Result<()> {
    use crate::cli::packages::execute_cmd;

    if container
        .chars()
        .any(|c| c.is_control() || c == ';' || c == '|' || c == '&')
    {
        execute_cmd(Components::error_with_suggestion(
            "Invalid container name",
            "Container names must not contain control characters or shell operators",
        ));
        anyhow::bail!("Invalid container name");
    }

    let manager = ContainerManager::new()?;

    execute_cmd(Components::loading(format!(
        "Stopping container: {container}"
    )));

    manager.stop(container)?;

    execute_cmd(Components::complete(format!(
        "Container {container} stopped"
    )));

    Ok(())
}

/// Execute a command in a running container
pub fn exec(container: &str, command: &[String]) -> Result<()> {
    use crate::cli::packages::execute_cmd;

    if container
        .chars()
        .any(|c| c.is_control() || c == ';' || c == '|' || c == '&')
    {
        execute_cmd(Components::error_with_suggestion(
            "Invalid container name",
            "Container names must not contain control characters or shell operators",
        ));
        anyhow::bail!("Invalid container name");
    }

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
    use crate::cli::packages::execute_cmd;

    let cwd = std::env::current_dir()?;
    let dockerfile_path = cwd.join("Dockerfile.omg");

    if dockerfile_path.exists() {
        execute_cmd(Components::error_with_suggestion(
            "Dockerfile.omg already exists",
            "Remove it first or use a different name",
        ));
        anyhow::bail!("Dockerfile.omg already exists");
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

    let manager = ContainerManager::new()
        .unwrap_or_else(|_| ContainerManager::with_runtime(ContainerRuntime::Docker));

    let runtime_refs: Vec<(&str, &str)> = runtimes.iter().map(|(r, v)| (*r, v.as_str())).collect();

    let dockerfile = manager.generate_dockerfile(&base, &runtime_refs);

    std::fs::write(&dockerfile_path, dockerfile)?;

    let mut details = vec![format!("Base image: {}", base)];
    if !runtimes.is_empty() {
        details.push("Detected runtimes:".to_string());
        for (rt, ver) in &runtimes {
            details.push(format!("  • {rt}: {ver}"));
        }
    }

    execute_cmd(Cmd::batch([
        Cmd::success("Created Dockerfile.omg"),
        Cmd::card("Configuration", details),
        Cmd::println("\n  Build with:"),
        Cmd::println("    omg container build -t myapp ."),
    ]));

    Ok(())
}
