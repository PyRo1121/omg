//! `omg ci` - Generate CI/CD configuration

use anyhow::Result;
use owo_colors::OwoColorize;
use std::fs;

/// Initialize CI configuration
pub fn init(provider: &str, advanced: bool) -> Result<()> {
    // SECURITY: Validate provider
    let valid_providers = ["github", "gitlab", "circleci"];
    if !valid_providers.contains(&provider.to_lowercase().as_str()) {
        anyhow::bail!("Unknown CI provider '{provider}'. Supported: github, gitlab, circleci");
    }

    let mode_str = if advanced {
        "advanced".magenta().to_string()
    } else {
        "basic".blue().to_string()
    };

    println!(
        "{} Generating {} CI configuration ({} mode)...\n",
        "OMG".cyan().bold(),
        provider.yellow(),
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
    println!("{} Validating CI environment...\n", "OMG".cyan().bold());

    let state = crate::core::env::fingerprint::EnvironmentState::capture().await?;

    // Check for omg.lock
    if !std::path::Path::new("omg.lock").exists() {
        println!(
            "  {} No omg.lock found - run {} first",
            "⚠".yellow(),
            "omg env capture".cyan()
        );
        return Ok(());
    }

    let lock = crate::core::env::fingerprint::EnvironmentState::load("omg.lock")?;

    if state.hash == lock.hash {
        println!("  {} Environment matches omg.lock", "✓".green());
        Ok(())
    } else {
        println!("  {} Environment drift detected!", "✗".red());
        println!("  Run {} to see differences", "omg diff omg.lock".cyan());
        anyhow::bail!("Environment drift detected")
    }
}

/// Generate cache manifest for CI
pub fn cache() -> Result<()> {
    println!("{} CI Cache Paths\n", "OMG".cyan().bold());

    println!("  {}", "Recommended cache paths:".bold());
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

    println!("  {}", "Cache key suggestion:".bold());
    println!(
        "  {}",
        "omg-${{ runner.os }}-${{ hashFiles('omg.lock') }}".cyan()
    );

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
          curl -fsSL https://pyro1121.com/install.sh | sh
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
          curl -fsSL https://pyro1121.com/install.sh | sh
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

    let path = ".github/workflows/ci.yml";
    if let Some(parent) = std::path::Path::new(path).parent() {
        fs::create_dir_all(parent)?;
    }

    if std::path::Path::new(path).exists() {
        println!(
            "  {} {} already exists - not overwriting",
            "⚠".yellow(),
            path
        );
        println!("  Here's what we'd generate:\n");
        println!("{}", config.dimmed());
    } else {
        fs::write(path, config)?;
        println!("  {} Created {}", "✓".green(), path.cyan());
    }

    println!();
    println!("  {}", "Next steps:".bold());
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
    - curl -fsSL https://pyro1121.com/install.sh | sh
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
  - curl -fsSL https://pyro1121.com/install.sh | sh
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

    let path = ".gitlab-ci.yml";

    if std::path::Path::new(path).exists() {
        println!(
            "  {} {} already exists - not overwriting",
            "⚠".yellow(),
            path
        );
        println!("  Here's what we'd generate:\n");
        println!("{}", config.dimmed());
    } else {
        fs::write(path, config)?;
        println!("  {} Created {}", "✓".green(), path.cyan());
    }

    Ok(())
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
            curl -fsSL https://pyro1121.com/install.sh | sh
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
            curl -fsSL https://pyro1121.com/install.sh | sh
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
            curl -fsSL https://pyro1121.com/install.sh | sh
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

    let path = ".circleci/config.yml";
    if let Some(parent) = std::path::Path::new(path).parent() {
        fs::create_dir_all(parent)?;
    }

    if std::path::Path::new(path).exists() {
        println!(
            "  {} {} already exists - not overwriting",
            "⚠".yellow(),
            path
        );
        println!("  Here's what we'd generate:\n");
        println!("{}", config.dimmed());
    } else {
        fs::write(path, config)?;
        println!("  {} Created {}", "✓".green(), path.cyan());
    }

    Ok(())
}
