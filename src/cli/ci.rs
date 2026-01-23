//! `omg ci` - Generate CI/CD configuration

use anyhow::Result;
use owo_colors::OwoColorize;
use std::fs;

/// Initialize CI configuration
pub fn init(provider: &str) -> Result<()> {
    // SECURITY: Validate provider
    let valid_providers = ["github", "gitlab", "circleci"];
    if !valid_providers.contains(&provider.to_lowercase().as_str()) {
        anyhow::bail!("Unknown CI provider '{provider}'. Supported: github, gitlab, circleci");
    }

    println!(
        "{} Generating {} CI configuration...\n",
        "OMG".cyan().bold(),
        provider.yellow()
    );

    match provider.to_lowercase().as_str() {
        "github" => generate_github_actions()?,
        "gitlab" => generate_gitlab_ci()?,
        "circleci" => generate_circleci()?,
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

fn generate_github_actions() -> Result<()> {
    let config = r#"name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  build:
    runs-on: ubuntu-latest
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Cache OMG data
        uses: actions/cache@v4
        with:
          path: |
            ~/.local/share/omg
            ~/.cargo/registry
            ~/.cargo/git
          key: omg-${{ runner.os }}-${{ hashFiles('omg.lock') }}
          restore-keys: |
            omg-${{ runner.os }}-
      
      - name: Install OMG
        run: |
          curl -fsSL https://pyro1121.com/install.sh | sh
          echo "$HOME/.local/bin" >> $GITHUB_PATH
      
      - name: Sync environment
        run: |
          omg env check || omg env sync omg.lock
      
      - name: Build
        run: |
          omg run build
      
      - name: Test
        run: |
          omg run test
"#;

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

fn generate_gitlab_ci() -> Result<()> {
    let config = r#"stages:
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
"#;

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

fn generate_circleci() -> Result<()> {
    let config = r#"version: 2.1

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
"#;

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
