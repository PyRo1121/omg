//! Container runtime integration (Docker/Podman)
//!
//! Provides:
//! - Auto-detection of Docker or Podman
//! - Run commands in containers with OMG environment
//! - Build development containers with runtime versions
//! - Interactive shell access to containers

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::{Command, Stdio};

/// Supported container runtimes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerRuntime {
    Docker,
    Podman,
}

impl ContainerRuntime {
    /// Get the command name for this runtime
    #[must_use]
    pub fn command(&self) -> &'static str {
        match self {
            Self::Docker => "docker",
            Self::Podman => "podman",
        }
    }

    /// Check if this runtime is available
    #[must_use]
    pub fn is_available(&self) -> bool {
        Command::new(self.command())
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }
}

impl std::fmt::Display for ContainerRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Docker => write!(f, "Docker"),
            Self::Podman => write!(f, "Podman"),
        }
    }
}

/// Detect available container runtime (prefers Podman for rootless)
#[must_use]
pub fn detect_runtime() -> Option<ContainerRuntime> {
    // Prefer Podman (rootless by default, better security)
    if ContainerRuntime::Podman.is_available() {
        return Some(ContainerRuntime::Podman);
    }
    if ContainerRuntime::Docker.is_available() {
        return Some(ContainerRuntime::Docker);
    }
    None
}

/// Container configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerConfig {
    /// Base image to use
    pub image: String,
    /// Container name (optional)
    pub name: Option<String>,
    /// Environment variables
    pub env: Vec<(String, String)>,
    /// Volume mounts (host:container)
    pub volumes: Vec<(String, String)>,
    /// Ports to expose (host:container)
    pub ports: Vec<(u16, u16)>,
    /// Working directory inside container
    pub workdir: Option<String>,
    /// Whether to remove container after exit
    pub rm: bool,
    /// Whether to run interactively with TTY
    pub interactive: bool,
}

impl Default for ContainerConfig {
    fn default() -> Self {
        Self {
            image: "ubuntu:24.04".to_string(),
            name: None,
            env: Vec::new(),
            volumes: Vec::new(),
            ports: Vec::new(),
            workdir: None,
            rm: true,
            interactive: true,
        }
    }
}

/// Container manager for running commands in containers
pub struct ContainerManager {
    runtime: ContainerRuntime,
}

impl ContainerManager {
    /// Create a new container manager
    pub fn new() -> Result<Self> {
        let runtime =
            detect_runtime().context("No container runtime found. Install Docker or Podman.")?;
        Ok(Self { runtime })
    }

    /// Create with a specific runtime
    #[must_use]
    pub fn with_runtime(runtime: ContainerRuntime) -> Self {
        Self { runtime }
    }

    /// Get the active runtime
    #[must_use]
    pub fn runtime(&self) -> ContainerRuntime {
        self.runtime
    }

    /// Run a command in a container
    pub fn run(&self, config: &ContainerConfig, command: &[&str]) -> Result<i32> {
        let mut cmd = Command::new(self.runtime.command());
        cmd.arg("run");

        if config.rm {
            cmd.arg("--rm");
        }

        if config.interactive {
            cmd.arg("-it");
        }

        if let Some(ref name) = config.name {
            cmd.args(["--name", name]);
        }

        if let Some(ref workdir) = config.workdir {
            cmd.args(["-w", workdir]);
        }

        for (key, value) in &config.env {
            cmd.args(["-e", &format!("{key}={value}")]);
        }

        for (host, container) in &config.volumes {
            cmd.args(["-v", &format!("{host}:{container}")]);
        }

        for (host, container) in &config.ports {
            cmd.args(["-p", &format!("{host}:{container}")]);
        }

        cmd.arg(&config.image);
        cmd.args(command);

        let status = cmd.status().context("Failed to run container")?;
        Ok(status.code().unwrap_or(1))
    }

    /// Run an interactive shell in a container
    pub fn shell(&self, config: &ContainerConfig) -> Result<i32> {
        let shell = detect_container_shell(&config.image);
        self.run(config, &[&shell])
    }

    /// Execute a command in a running container
    pub fn exec(&self, container: &str, command: &[&str], interactive: bool) -> Result<i32> {
        let mut cmd = Command::new(self.runtime.command());
        cmd.arg("exec");

        if interactive {
            cmd.arg("-it");
        }

        cmd.arg(container);
        cmd.args(command);

        let status = cmd.status().context("Failed to exec in container")?;
        Ok(status.code().unwrap_or(1))
    }

    /// Build a container image from a Dockerfile
    pub fn build(&self, dockerfile: &Path, tag: &str, context: &Path) -> Result<()> {
        self.build_with_options(dockerfile, tag, context, false, &[], None)
    }

    /// Build a container image with advanced options
    pub fn build_with_options(
        &self,
        dockerfile: &Path,
        tag: &str,
        context: &Path,
        no_cache: bool,
        build_args: &[String],
        target: Option<&str>,
    ) -> Result<()> {
        let mut cmd = Command::new(self.runtime.command());
        cmd.arg("build");
        cmd.args(["-f", &dockerfile.display().to_string()]);
        cmd.args(["-t", tag]);

        if no_cache {
            cmd.arg("--no-cache");
        }

        for arg in build_args {
            cmd.args(["--build-arg", arg]);
        }

        if let Some(t) = target {
            cmd.args(["--target", t]);
        }

        cmd.arg(context.display().to_string());

        let status = cmd.status().context("Failed to build container")?;
        if !status.success() {
            anyhow::bail!("Container build failed with exit code: {:?}", status.code());
        }
        Ok(())
    }

    /// List running containers
    pub fn list_running(&self) -> Result<Vec<ContainerInfo>> {
        let output = Command::new(self.runtime.command())
            .args([
                "ps",
                "--format",
                "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.Status}}",
            ])
            .output()
            .context("Failed to list containers")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let containers = stdout
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 4 {
                    Some(ContainerInfo {
                        id: parts[0].to_string(),
                        name: parts[1].to_string(),
                        image: parts[2].to_string(),
                        status: parts[3].to_string(),
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(containers)
    }

    /// Stop a running container
    pub fn stop(&self, container: &str) -> Result<()> {
        let status = Command::new(self.runtime.command())
            .args(["stop", container])
            .status()
            .context("Failed to stop container")?;

        if !status.success() {
            anyhow::bail!("Failed to stop container: {container}");
        }
        Ok(())
    }

    /// Remove a container
    pub fn remove(&self, container: &str, force: bool) -> Result<()> {
        let mut cmd = Command::new(self.runtime.command());
        cmd.arg("rm");
        if force {
            cmd.arg("-f");
        }
        cmd.arg(container);

        let status = cmd.status().context("Failed to remove container")?;
        if !status.success() {
            anyhow::bail!("Failed to remove container: {container}");
        }
        Ok(())
    }

    /// Pull an image
    pub fn pull(&self, image: &str) -> Result<()> {
        let status = Command::new(self.runtime.command())
            .args(["pull", image])
            .status()
            .context("Failed to pull image")?;

        if !status.success() {
            anyhow::bail!("Failed to pull image: {image}");
        }
        Ok(())
    }

    /// List available images
    pub fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let output = Command::new(self.runtime.command())
            .args([
                "images",
                "--format",
                "{{.Repository}}\t{{.Tag}}\t{{.ID}}\t{{.Size}}",
            ])
            .output()
            .context("Failed to list images")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let images = stdout
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 4 {
                    Some(ImageInfo {
                        repository: parts[0].to_string(),
                        tag: parts[1].to_string(),
                        id: parts[2].to_string(),
                        size: parts[3].to_string(),
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(images)
    }

    /// Generate a Dockerfile for OMG development environment
    pub fn generate_dockerfile(&self, base_image: &str, runtimes: &[(&str, &str)]) -> String {
        let mut dockerfile = format!("FROM {base_image}\n\n");
        dockerfile.push_str("# OMG Development Environment\n");
        dockerfile.push_str("LABEL maintainer=\"OMG Team\"\n\n");

        // Install common dependencies based on base image
        if base_image.contains("ubuntu") || base_image.contains("debian") {
            dockerfile.push_str("RUN apt-get update && apt-get install -y \\\n");
            dockerfile.push_str("    curl wget git build-essential ca-certificates \\\n");
            dockerfile.push_str("    && rm -rf /var/lib/apt/lists/*\n\n");
        } else if base_image.contains("arch") {
            dockerfile.push_str("RUN pacman -Syu --noconfirm && pacman -S --noconfirm \\\n");
            dockerfile.push_str("    curl wget git base-devel\n\n");
        } else if base_image.contains("alpine") {
            dockerfile.push_str("RUN apk add --no-cache \\\n");
            dockerfile.push_str("    curl wget git build-base\n\n");
        }

        // Install runtimes
        for (runtime, version) in runtimes {
            match *runtime {
                "node" => {
                    dockerfile.push_str("# Install Node.js\n");
                    dockerfile.push_str("ENV NODE_VERSION=");
                    dockerfile.push_str(if *version == "lts" { "20" } else { version });
                    dockerfile.push('\n');
                    dockerfile.push_str("RUN curl -fsSL https://deb.nodesource.com/setup_${NODE_VERSION}.x | bash - \\\n");
                    dockerfile.push_str("    && apt-get install -y nodejs \\\n");
                    dockerfile.push_str("    && rm -rf /var/lib/apt/lists/*\n\n");
                }
                "python" => {
                    dockerfile.push_str("# Install Python\n");
                    dockerfile.push_str("ENV PYTHON_VERSION=");
                    dockerfile.push_str(version);
                    dockerfile.push('\n');
                    dockerfile.push_str("RUN apt-get update && apt-get install -y \\\n");
                    dockerfile.push_str("    python3 python3-pip python3-venv \\\n");
                    dockerfile.push_str("    && rm -rf /var/lib/apt/lists/* \\\n");
                    dockerfile.push_str("    && ln -sf /usr/bin/python3 /usr/bin/python\n\n");
                }
                "rust" => {
                    dockerfile.push_str("# Install Rust\n");
                    dockerfile.push_str("ENV RUSTUP_HOME=/usr/local/rustup \\\n");
                    dockerfile.push_str("    CARGO_HOME=/usr/local/cargo \\\n");
                    dockerfile.push_str("    PATH=/usr/local/cargo/bin:$PATH\n");
                    dockerfile.push_str("RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain ");
                    dockerfile.push_str(version);
                    dockerfile.push_str("\n\n");
                }
                "go" => {
                    dockerfile.push_str("# Install Go\n");
                    let go_ver = if *version == "latest" {
                        "1.22"
                    } else {
                        version
                    };
                    dockerfile.push_str("ENV GO_VERSION=");
                    dockerfile.push_str(go_ver);
                    dockerfile.push('\n');
                    dockerfile.push_str("RUN curl -fsSL https://go.dev/dl/go${GO_VERSION}.linux-amd64.tar.gz | tar -C /usr/local -xzf - \\\n");
                    dockerfile.push_str("    && ln -sf /usr/local/go/bin/go /usr/local/bin/go\n");
                    dockerfile.push_str("ENV PATH=$PATH:/usr/local/go/bin\n\n");
                }
                "bun" => {
                    dockerfile.push_str("# Install Bun\n");
                    dockerfile.push_str("RUN curl -fsSL https://bun.sh/install | bash\n");
                    dockerfile.push_str("ENV PATH=$PATH:/root/.bun/bin\n\n");
                }
                "ruby" => {
                    dockerfile.push_str("# Install Ruby\n");
                    dockerfile.push_str("RUN apt-get update && apt-get install -y ruby-full \\\n");
                    dockerfile.push_str("    && rm -rf /var/lib/apt/lists/*\n\n");
                }
                "java" => {
                    dockerfile.push_str("# Install Java\n");
                    dockerfile
                        .push_str("RUN apt-get update && apt-get install -y default-jdk \\\n");
                    dockerfile.push_str("    && rm -rf /var/lib/apt/lists/*\n\n");
                }
                _ => {
                    use std::fmt::Write as _;
                    let _ = writeln!(dockerfile, "# TODO: Install {runtime} {version}");
                }
            }
        }

        dockerfile.push_str("WORKDIR /app\n\n");
        dockerfile.push_str("# Copy project files\n");
        dockerfile.push_str("COPY . .\n\n");
        dockerfile.push_str("CMD [\"/bin/bash\"]\n");

        dockerfile
    }
}

/// Information about a running container
#[derive(Debug, Clone)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
}

/// Information about a container image
#[derive(Debug, Clone)]
pub struct ImageInfo {
    pub repository: String,
    pub tag: String,
    pub id: String,
    pub size: String,
}

/// Detect the best shell for a container image
fn detect_container_shell(image: &str) -> String {
    if image.contains("alpine") {
        "/bin/sh".to_string()
    } else {
        "/bin/bash".to_string()
    }
}

/// Create a development container config for the current project
pub fn dev_container_config(project_dir: &Path) -> ContainerConfig {
    let project_name = project_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("omg-dev");

    ContainerConfig {
        image: "ubuntu:24.04".to_string(),
        name: Some(format!("{project_name}-dev")),
        env: vec![("TERM".to_string(), "xterm-256color".to_string())],
        volumes: vec![(project_dir.display().to_string(), "/app".to_string())],
        ports: Vec::new(),
        workdir: Some("/app".to_string()),
        rm: true,
        interactive: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_runtime() {
        // This test will pass if either Docker or Podman is installed
        let runtime = detect_runtime();
        // Just verify it doesn't panic
        if let Some(rt) = runtime {
            assert!(rt.is_available());
        }
    }

    #[test]
    fn test_container_config_default() {
        let config = ContainerConfig::default();
        assert_eq!(config.image, "ubuntu:24.04");
        assert!(config.rm);
        assert!(config.interactive);
    }

    #[test]
    fn test_generate_dockerfile() {
        let manager = ContainerManager::with_runtime(ContainerRuntime::Docker);
        let dockerfile = manager.generate_dockerfile("ubuntu:24.04", &[("node", "20.10.0")]);
        assert!(dockerfile.contains("FROM ubuntu:24.04"));
        // Check for Node.js installation (new format installs runtimes)
        assert!(dockerfile.contains("Install Node.js") || dockerfile.contains("NODE_VERSION"));
    }
}
