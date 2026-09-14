//! `omg ci` - Generate CI/CD configuration

use anyhow::Result;
use owo_colors::OwoColorize;
use std::fs;
use std::io::Write as _;

use crate::cli::style;

/// Initialize CI configuration
pub fn init(provider: &str, advanced: bool) -> Result<()> {
    // SECURITY: Validate provider
    let valid_providers = ["github", "gitlab", "circleci"];
    if !valid_providers.contains(&provider.to_lowercase().as_str()) {
        anyhow::bail!("Unknown CI provider '{provider}'. Supported: github, gitlab, circleci");
    }

    let mode_str = if advanced {
        style::maybe_color("advanced", |t| t.magenta().to_string())
    } else {
        style::maybe_color("basic", |t| t.blue().to_string())
    };

    println!(
        "{} Generating {} CI configuration ({} mode)...\n",
        style::runtime("OMG"),
        style::path(provider),
        mode_str
    );

    match provider.to_lowercase().as_str() {
        "github" => generate_github_actions(advanced)?,
        "gitlab" => generate_gitlab_ci(advanced)?,
        "circleci" => generate_circleci(advanced)?,
        _ => anyhow::bail!("Unknown CI provider '{provider}'. Supported: github, gitlab, circleci"),
    }

    Ok(())
}

/// Validate environment matches CI expectations
pub async fn validate() -> Result<()> {
    println!("{} Validating CI environment...\n", style::runtime("OMG"));

    let lock_path = std::path::Path::new("omg.lock");
    require_ci_lockfile(lock_path)?;
    let state = crate::core::env::fingerprint::EnvironmentState::capture().await?;
    let lock = crate::core::env::fingerprint::EnvironmentState::load(lock_path)?;

    if state.hash == lock.hash {
        println!(
            "  {} Environment matches omg.lock",
            style::maybe_color("✓", |t| t.green().to_string())
        );
        Ok(())
    } else {
        println!(
            "  {} Environment drift detected!",
            style::maybe_color("✗", |t| t.red().to_string())
        );
        println!(
            "  Run {} to see differences",
            style::command("omg diff omg.lock")
        );
        anyhow::bail!("Environment drift detected")
    }
}

fn require_ci_lockfile(path: &std::path::Path) -> Result<()> {
    anyhow::ensure!(
        path.is_file(),
        "No omg.lock found; run 'omg env capture' before CI validation"
    );
    Ok(())
}

/// Generate cache manifest for CI
pub fn cache() -> Result<()> {
    println!("{} CI Cache Paths\n", style::runtime("OMG"));

    println!(
        "  {}",
        style::maybe_color("Recommended cache paths:", |t| t.bold().to_string())
    );
    println!();
    println!("  # OMG data directory");
    println!("  ~/.local/share/omg/");
    println!();
    println!("  # Runtime versions");
    println!("  ~/.local/share/omg/versions/");
    println!();

    #[cfg(feature = "arch")]
    {
        println!("  # Pacman cache (Arch)");
        println!("  /var/cache/pacman/pkg/");
        println!();
    }

    println!("  # Cargo cache");
    println!("  ~/.cargo/registry/");
    println!("  ~/.cargo/git/");
    println!();
    println!("  # NPM cache");
    println!("  ~/.npm/");
    println!();

    println!(
        "  {}",
        style::maybe_color("Cache key suggestion:", |t| t.bold().to_string())
    );
    println!(
        "  {}",
        style::maybe_color("omg-${{ runner.os }}-${{ hashFiles('omg.lock') }}", |t| t
            .cyan()
            .to_string())
    );

    Ok(())
}

/// Write a generated config file, previewing instead of overwriting.
fn write_config_file(path: &str, config: &str) -> Result<()> {
    // Pin the installer bootstrap to this release tag. A mutable `main`
    // reference would let a future commit change what new pipelines execute.
    let config = config.replace(
        "raw.githubusercontent.com/PyRo1121/omg/main/install.sh",
        &format!(
            "raw.githubusercontent.com/PyRo1121/omg/v{}/install.sh",
            env!("CARGO_PKG_VERSION")
        ),
    );
    let config = config.as_str();
    ensure_safe_config_parent(std::path::Path::new(path))?;

    let created = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut file) => {
            file.write_all(config.as_bytes())?;
            file.sync_all()?;
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => false,
        Err(error) => return Err(error.into()),
    };

    if !created {
        println!(
            "  {} {} already exists - not overwriting",
            style::maybe_color("⚠", |t| t.yellow().to_string()),
            path
        );
        println!("  Here's what we'd generate:\n");
        println!("{}", style::dim(config));
    } else {
        println!(
            "  {} Created {}",
            style::maybe_color("✓", |t| t.green().to_string()),
            style::maybe_color(path, |t| t.cyan().to_string())
        );
    }
    Ok(())
}

fn ensure_safe_config_parent(path: &std::path::Path) -> Result<()> {
    use std::path::Component;

    anyhow::ensure!(!path.is_absolute(), "CI config path must be relative");
    let mut directory = std::path::PathBuf::new();
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    for component in parent.components() {
        match component {
            Component::CurDir => continue,
            Component::Normal(name) => directory.push(name),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!("CI config path must stay inside the current repository")
            }
        }
        match fs::symlink_metadata(&directory) {
            Ok(metadata) => anyhow::ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "Refusing symlinked or non-directory CI config ancestor: {}",
                directory.display()
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&directory)?;
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn generate_github_actions(advanced: bool) -> Result<()> {
    let config = if advanced {
        r#"name: CI (Advanced)

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  test:
    name: Test (${{ matrix.os }}, features=[${{ matrix.features }}])
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest]
        features: ["arch", "debian", "license,pgp", "arch,debian,license,pgp"]
        include:
          - os: ubuntu-latest
            container: archlinux:latest
            features: "arch"

    container: ${{ matrix.container }}

    steps:
      - uses: actions/checkout@v4

      - name: Install dependencies (Arch)
        if: matrix.container == 'archlinux:latest'
        run: |
          pacman -Syu --noconfirm rustup base-devel git
          rustup default stable

      - name: Cache Cargo & OMG
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            ~/.local/share/omg
            target
          key: omg-${{ runner.os }}-${{ matrix.features }}-${{ hashFiles('Cargo.lock', 'omg.lock') }}
          restore-keys: |
            omg-${{ runner.os }}-${{ matrix.features }}-
            omg-${{ runner.os }}-

      - name: Install OMG
        run: |
          curl -fsSL https://raw.githubusercontent.com/PyRo1121/omg/main/install.sh | sh
          echo "$HOME/.local/bin" >> $GITHUB_PATH

      - name: Lint
        run: |
          cargo fmt --check
          cargo clippy --all-targets --all-features -- -D warnings

      - name: Mock Enterprise License (for SBOM/Security)
        run: |
          mkdir -p ~/.local/share/omg
          echo '{"key":"CI-MOCK-KEY","tier":"enterprise","features":["sbom","audit","secrets","slsa","policy"],"validated_at":9999999999}' > ~/.local/share/omg/license.json

      - name: Sync environment
        run: |
          omg env check || omg env sync omg.lock

      - name: Build
        run: cargo build --release --features ${{ matrix.features }}

      - name: Test
        run: cargo test --features ${{ matrix.features }}

  security:
    name: Security Audit & SBOM
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install cargo-audit
        run: cargo install cargo-audit
        
      - name: Audit dependencies
        run: cargo audit

      - name: Install OMG
        run: |
          curl -fsSL https://raw.githubusercontent.com/PyRo1121/omg/main/install.sh | sh
          echo "$HOME/.local/bin" >> $GITHUB_PATH

      - name: Generate SBOM
        run: omg audit sbom --output sbom.json

      - name: Upload SBOM
        uses: actions/upload-artifact@v4
        with:
          name: sbom
          path: sbom.json
"#
    } else {
        r"name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  lint:
    name: Lint
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      
      - name: Formatting
        run: cargo fmt --all -- --check
        
      - name: Clippy
        run: cargo clippy --all-targets --all-features -- -D warnings

  test:
    name: Test
    needs: [lint]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Cache Cargo & OMG
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            ~/.local/share/omg
            target
          key: omg-${{ runner.os }}-${{ hashFiles('Cargo.lock', 'omg.lock') }}
          restore-keys: |
            omg-${{ runner.os }}-
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
      
      - name: Build
        run: cargo build --release
      
      - name: Run Tests
        run: cargo test --all-features
"
    };

    write_config_file(".github/workflows/ci.yml", config)?;

    println!();
    println!(
        "  {}",
        style::maybe_color("Next steps:", |t| t.bold().to_string())
    );
    println!("    1. Commit the workflow file");
    println!("    2. Ensure omg.lock is committed");
    println!("    3. Push to trigger the workflow");

    Ok(())
}

fn generate_gitlab_ci(advanced: bool) -> Result<()> {
    let config = if advanced {
        r#"stages:
  - lint
  - test
  - build
  - security

variables:
  CARGO_HOME: $CI_PROJECT_DIR/.cargo
  OMG_CACHE_DIR: $CI_PROJECT_DIR/.omg-cache

.omg_template: &omg_definition
  image: rust:latest
  before_script:
    - curl -fsSL https://raw.githubusercontent.com/PyRo1121/omg/main/install.sh | sh
    - export PATH="$HOME/.local/bin:$PATH"
    - omg env check || omg env sync omg.lock
  cache:
    key: omg-$CI_COMMIT_REF_SLUG
    paths:
      - .cargo/
      - .omg-cache/
      - target/

lint:
  stage: lint
  <<: *omg_definition
  script:
    - cargo fmt --check
    - cargo clippy -- -D warnings

test:
  stage: test
  <<: *omg_definition
  parallel:
    matrix:
      - FEATURES: ["arch", "debian", "license,pgp", "arch,debian,license,pgp"]
  script:
    - cargo test --features $FEATURES

build:
  stage: build
  <<: *omg_definition
  script:
    - cargo build --release --all-features
  artifacts:
    paths:
      - target/release/omg
      - target/release/omgd

security:
  stage: security
  <<: *omg_definition
  script:
    - cargo install cargo-audit
    - cargo audit
    - omg audit sbom --output sbom.json
  artifacts:
    reports:
      cyclonedx: sbom.json
"#
    } else {
        r#"stages:
  - build
  - test

variables:
  OMG_CACHE_DIR: $CI_PROJECT_DIR/.omg-cache

cache:
  key: omg-$CI_COMMIT_REF_SLUG
  paths:
    - .omg-cache/
    - .cargo/

before_script:
  - curl -fsSL https://raw.githubusercontent.com/PyRo1121/omg/main/install.sh | sh
  - export PATH="$HOME/.local/bin:$PATH"
  - omg env check || omg env sync omg.lock

build:
  stage: build
  script:
    - omg run build
  artifacts:
    paths:
      - target/

test:
  stage: test
  script:
    - omg run test
"#
    };

    write_config_file(".gitlab-ci.yml", config)
}

fn generate_circleci(advanced: bool) -> Result<()> {
    let config = if advanced {
        r#"version: 2.1

orbs:
  rust: circleci/rust@1.6.0

jobs:
  test:
    docker:
      - image: cimg/rust:1.82
    parameters:
      features:
        type: string
        default: ""
    steps:
      - checkout
      - rust/install
      - restore_cache:
          keys:
            - omg-v2-{{ checksum "Cargo.lock" }}-{{ checksum "omg.lock" }}
            - omg-v2-{{ checksum "Cargo.lock" }}-
            - omg-v2-
      - run:
          name: Install OMG
          command: |
            curl -fsSL https://raw.githubusercontent.com/PyRo1121/omg/main/install.sh | sh
            echo 'export PATH="$HOME/.local/bin:$PATH"' >> $BASH_ENV
      - run:
          name: Sync environment
          command: omg env check || omg env sync omg.lock
      - run:
          name: Build & Test
          command: |
            cargo test --features << parameters.features >>
      - save_cache:
          key: omg-v2-{{ checksum "Cargo.lock" }}-{{ checksum "omg.lock" }}
          paths:
            - "~/.cargo"
            - "~/.local/share/omg"
            - "target"

  security:
    docker:
      - image: cimg/rust:1.82
    steps:
      - checkout
      - run:
          name: Security Audit
          command: |
            cargo install cargo-audit
            cargo audit
      - run:
          name: Generate SBOM
          command: |
            curl -fsSL https://raw.githubusercontent.com/PyRo1121/omg/main/install.sh | sh
            export PATH="$HOME/.local/bin:$PATH"
            omg audit sbom --output sbom.json
      - store_artifacts:
          path: sbom.json

workflows:
  build-and-test:
    jobs:
      - test:
          name: test-arch
          features: "arch"
      - test:
          name: test-debian
          features: "debian"
      - test:
          name: test-all
          features: "arch,debian,license,pgp"
      - security:
          requires:
            - test-all
"#
    } else {
        r#"version: 2.1

executors:
  linux:
    docker:
      - image: cimg/base:stable

jobs:
  build:
    executor: linux
    steps:
      - checkout
      - restore_cache:
          keys:
            - omg-{{ checksum "omg.lock" }}
            - omg-
      - run:
          name: Install OMG
          command: |
            curl -fsSL https://raw.githubusercontent.com/PyRo1121/omg/main/install.sh | sh
            echo 'export PATH="$HOME/.local/bin:$PATH"' >> $BASH_ENV
      - run:
          name: Sync environment
          command: omg env check || omg env sync omg.lock
      - save_cache:
          key: omg-{{ checksum "omg.lock" }}
          paths:
            - ~/.local/share/omg
      - run:
          name: Build
          command: omg run build
      - run:
          name: Test
          command: omg run test

workflows:
  build-and-test:
    jobs:
      - build
"#
    };

    write_config_file(".circleci/config.yml", config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn ci_generation_refuses_a_dangling_destination_symlink() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir_in(".").expect("relative temp directory");
        let outside = directory.path().join("outside.yml");
        let destination = directory.path().join("ci.yml");
        let outside_absolute = std::fs::canonicalize(directory.path())
            .expect("canonical fixture directory")
            .join("outside.yml");
        symlink(&outside_absolute, &destination).expect("dangling destination symlink");

        write_config_file(
            destination.to_str().expect("UTF-8 fixture path"),
            "untrusted overwrite",
        )
        .expect("existing destination is a preview, not an error");

        assert!(!outside.exists(), "writer must not follow the symlink");
        assert!(
            destination.is_symlink(),
            "existing entry must remain intact"
        );
    }

    #[cfg(unix)]
    #[test]
    fn ci_generation_refuses_a_symlinked_parent_directory() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir_in(".").expect("relative temp directory");
        let outside = directory.path().join("outside");
        std::fs::create_dir(&outside).expect("outside directory");
        let outside_absolute = std::fs::canonicalize(&outside).expect("canonical outside");
        let linked_parent = directory.path().join(".github");
        symlink(outside_absolute, &linked_parent).expect("linked parent");

        let destination = linked_parent.join("workflows/ci.yml");
        ensure_safe_config_parent(&destination).expect_err("symlinked parent must be refused");
        assert!(!outside.join("workflows").exists());
    }

    #[test]
    fn ci_validation_fails_closed_without_lockfile() {
        let directory = tempfile::tempdir().expect("temp directory");
        let missing = directory.path().join("omg.lock");

        let error =
            require_ci_lockfile(&missing).expect_err("CI validation without a lockfile must fail");

        assert!(error.to_string().contains("No omg.lock found"), "{error}");
    }
}
