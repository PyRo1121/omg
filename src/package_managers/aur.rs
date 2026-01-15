//! AUR (Arch User Repository) client with build support

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, Result};
use colored::Colorize;
use futures::StreamExt;
use git2::Repository;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::process::Command;
use tracing::instrument;

use super::pkgbuild::PkgBuild;
use crate::config::Settings;
use crate::core::http::shared_client;
use crate::core::{paths, Package, PackageSource};
use crate::package_managers::{get_potential_aur_packages, pacman_db};

const AUR_RPC_URL: &str = "https://aur.archlinux.org/rpc";
const AUR_GIT_URL: &str = "https://aur.archlinux.org";

/// AUR API client with build support
#[derive(Clone)]
pub struct AurClient {
    client: reqwest::Client,
    build_dir: PathBuf,
    settings: Settings,
}

struct MakepkgEnv {
    makeflags: String,
    pkgdest: PathBuf,
    srcdest: PathBuf,
    extra_env: Vec<(String, String)>,
}

#[derive(Debug, Deserialize)]
struct AurResponse {
    results: Vec<AurPackage>,
}

#[derive(Debug, Deserialize)]
struct AurPackage {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Version")]
    version: String,
    #[serde(rename = "Description")]
    description: Option<String>,
    #[serde(rename = "Maintainer")]
    _maintainer: Option<String>,
    #[serde(rename = "NumVotes")]
    _num_votes: Option<i32>,
    #[serde(rename = "Popularity")]
    _popularity: Option<f64>,
    #[serde(rename = "OutOfDate")]
    _out_of_date: Option<i64>,
}

impl AurClient {
    pub fn new() -> Self {
        let settings = Settings::load().unwrap_or_default();
        let build_dir = paths::cache_dir().join("aur");

        Self {
            client: shared_client().clone(),
            build_dir,
            settings,
        }
    }

    /// Search AUR packages
    pub async fn search(&self, query: &str) -> Result<Vec<Package>> {
        let url = format!("{AUR_RPC_URL}?v=5&type=search&arg={query}");

        let response: AurResponse = self.client.get(&url).send().await?.json().await?;

        let mut packages: Vec<Package> = response
            .results
            .into_iter()
            .map(|p| Package {
                name: p.name,
                version: p.version,
                description: p.description.unwrap_or_default(),
                source: PackageSource::Aur,
                installed: false,
            })
            .collect();

        // Sort by popularity (most popular first)
        packages.sort_by(|a, b| b.name.cmp(&a.name));

        Ok(packages)
    }

    /// Get info for a specific AUR package
    pub async fn info(&self, package: &str) -> Result<Option<Package>> {
        let url = format!("{AUR_RPC_URL}?v=5&type=info&arg={package}");

        let response: AurResponse = self.client.get(&url).send().await?.json().await?;

        Ok(response.results.into_iter().next().map(|p| Package {
            name: p.name,
            version: p.version,
            description: p.description.unwrap_or_default(),
            source: PackageSource::Aur,
            installed: false,
        }))
    }

    /// Get list of upgradable AUR packages
    /// Uses pure Rust cache (<1ms) to identify local AUR packages,
    /// and parallel RPC calls to check for updates.
    #[instrument(skip(self))]
    pub async fn get_update_list(&self) -> Result<Vec<(String, String, String)>> {
        // 1. Identify potential AUR packages using pure Rust cache (Fast-path)
        let potential_aur_names = get_potential_aur_packages()?;

        if potential_aur_names.is_empty() {
            return Ok(Vec::new());
        }

        // 2. Query AUR for these packages in parallel (batch query)
        let mut updates = Vec::new();
        let chunked_names: Vec<Vec<String>> = potential_aur_names
            .chunks(50)
            .map(<[std::string::String]>::to_vec)
            .collect();

        let concurrency = self.settings.aur.build_concurrency.clamp(1, 8);
        let mut stream = futures::stream::iter(chunked_names)
            .map(|chunk| {
                let client = &self.client;
                async move {
                    let mut url = format!("{AUR_RPC_URL}?v=5&type=info");
                    for name in chunk {
                        url.push_str("&arg[]=");
                        url.push_str(&name);
                    }
                    client.get(&url).send().await?.json::<AurResponse>().await
                }
            })
            .buffer_unordered(concurrency); // Query chunks in parallel

        while let Some(res) = stream.next().await {
            let response = res?;
            for p in response.results {
                // Version comparison using pure Rust pacman_db version logic
                if let Some(local_pkg) = pacman_db::get_local_package(&p.name)? {
                    if pacman_db::compare_versions(&p.version, &local_pkg.version)
                        == std::cmp::Ordering::Greater
                    {
                        updates.push((p.name, local_pkg.version, p.version));
                    }
                }
            }
        }

        Ok(updates)
    }

    /// Install AUR package by building it
    pub async fn install(&self, package: &str) -> Result<()> {
        println!(
            "{} Installing AUR package: {}\n",
            "OMG".cyan().bold(),
            package.yellow()
        );

        // Ensure build directory exists
        std::fs::create_dir_all(&self.build_dir)?;

        let pkg_dir = self.build_dir.join(package);

        // Clone or update the package
        if pkg_dir.exists() {
            println!("{} Updating existing source...", "→".blue());
            self.git_pull(&pkg_dir).await?;
        } else {
            println!("{} Cloning from AUR...", "→".blue());
            self.git_clone(package).await?;
        }

        // Review PKGBUILD
        let pkgbuild_path = pkg_dir.join("PKGBUILD");
        if !pkgbuild_path.exists() {
            anyhow::bail!(
                "✗ Build Error: PKGBUILD not found for package '{package}'.\n  Verify the package exists on AUR or check your internet connection."
            );
        }

        // Parse PKGBUILD
        let pkgbuild = PkgBuild::parse(&pkgbuild_path).with_context(|| {
            format!("Failed to parse PKGBUILD for '{package}'. The file may be malformed.")
        })?;
        println!(
            "{} Parsed PKGBUILD: {} v{}",
            "→".blue(),
            pkgbuild.name,
            pkgbuild.version
        );

        let env = self.makepkg_env(&pkg_dir)?;
        let cache_key = self.cache_key(&pkg_dir, &env.makeflags)?;

        let pkg_file =
            if let Some(cached) = self.cached_package(package, &env.pkgdest, &cache_key)? {
                println!("{} Using cached build...", "→".blue());
                cached
            } else {
                // Build with makepkg (sandboxed if bubblewrap is available)
                let status = self
                    .run_sandboxed_makepkg(&pkg_dir, &env)
                    .await
                    .with_context(|| format!("Failed to run makepkg for '{package}'"))?;

                if !status.success() {
                    anyhow::bail!(
                        "makepkg failed for '{package}'. Check build output above for details."
                    );
                }

                let pkg_file = self.find_built_package(&pkg_dir, &env.pkgdest)?;
                self.write_cache_key(package, &cache_key)?;
                pkg_file
            };

        // Install the built package
        println!("{} Installing built package...", "→".blue());
        self.install_built_package(&pkg_file).await?;

        println!("\n{} {} installed successfully!", "✓".green(), package);

        Ok(())
    }

    /// Build an AUR package and return the path to the built package (no install)
    /// This is used for batch updates where we want to install all packages at once
    /// Uses makepkg for reliable builds that match yay/paru behavior
    #[instrument(skip(self))]
    pub async fn build_only(&self, package: &str) -> Result<PathBuf> {
        // Ensure build directory exists
        std::fs::create_dir_all(&self.build_dir)?;

        let pkg_dir = self.build_dir.join(package);
        let pkgbuild_path = pkg_dir.join("PKGBUILD");

        // Clone or update the package - detect incomplete clones
        if pkg_dir.exists() && pkgbuild_path.exists() {
            // Valid existing clone - just update
            self.git_pull(&pkg_dir).await.with_context(|| {
                format!(
                    "Failed to update AUR package '{package}'. Try removing ~/.cache/omg/aur/{package}"
                )
            })?;
        } else {
            // Clean up any incomplete clone and start fresh
            if pkg_dir.exists() {
                std::fs::remove_dir_all(&pkg_dir).ok();
            }
            self.git_clone(package).await.with_context(|| {
                format!("Failed to clone AUR package '{package}'. Check if it exists on AUR.")
            })?;
        }

        // Verify PKGBUILD exists after clone
        if !pkgbuild_path.exists() {
            anyhow::bail!(
                "PKGBUILD not found for '{package}'. The AUR package may not exist or clone failed."
            );
        }

        let env = self.makepkg_env(&pkg_dir)?;
        let cache_key = self.cache_key(&pkg_dir, &env.makeflags)?;

        if let Some(cached) = self.cached_package(package, &env.pkgdest, &cache_key)? {
            return Ok(cached);
        }

        // Use sandboxed build with bubblewrap if available, fallback to regular makepkg
        let status = self
            .run_sandboxed_makepkg(&pkg_dir, &env)
            .await
            .with_context(|| format!("Failed to run makepkg for '{package}'"))?;

        if !status.success() {
            anyhow::bail!("makepkg failed for '{package}'. Check build output above for details.");
        }

        let pkg_file = self.find_built_package(&pkg_dir, &env.pkgdest)?;
        self.write_cache_key(package, &cache_key)?;
        Ok(pkg_file)
    }

    fn find_built_package(&self, pkg_dir: &Path, pkgdest: &Path) -> Result<PathBuf> {
        let pkg_path =
            Self::find_package_in_dir(pkgdest).or_else(|| Self::find_package_in_dir(pkg_dir));

        pkg_path.ok_or_else(|| anyhow::anyhow!("No package archive found after makepkg"))
    }

    fn find_package_in_dir(path: &Path) -> Option<PathBuf> {
        let entries = std::fs::read_dir(path).ok()?;
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".pkg.tar.zst") || name.ends_with(".pkg.tar.xz") {
                return Some(entry.path());
            }
        }
        None
    }

    /// Clone package from AUR
    async fn git_clone(&self, package: &str) -> Result<()> {
        let url = format!("{AUR_GIT_URL}/{package}.git");
        let dest = self.build_dir.join(package);

        let spinner = create_spinner("Cloning repository (native)...");

        // Use git2 for native cloning
        tokio::task::spawn_blocking(move || {
            git2::build::RepoBuilder::new()
                .clone(&url, &dest)
                .context("Failed to clone AUR package via git2")
        })
        .await??;

        spinner.finish_and_clear();
        Ok(())
    }

    /// Update existing clone
    async fn git_pull(&self, pkg_dir: &PathBuf) -> Result<()> {
        let spinner = create_spinner("Pulling latest changes (native)...");

        let pkg_dir = pkg_dir.clone();
        tokio::task::spawn_blocking(move || {
            let repo = Repository::open(&pkg_dir).context("Failed to open local repository")?;

            let mut remote = repo
                .find_remote("origin")
                .context("Failed to find remote 'origin'")?;

            // Fetch the latest changes
            remote
                .fetch(&["master"], None, None)
                .context("Failed to fetch from remote")?;

            // Merge changes (simplified: assume fast-forward)
            let fetch_head = repo.find_reference("FETCH_HEAD")?;
            let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)?;

            let (analysis, _) = repo.merge_analysis(&[&fetch_commit])?;

            if analysis.is_up_to_date() {
                return Ok(());
            } else if analysis.is_fast_forward() {
                let mut reference = repo.find_reference("refs/heads/master")?;
                reference.set_target(fetch_commit.id(), "Fast-forward")?;
                repo.set_head("refs/heads/master")?;
                repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))?;
            } else {
                anyhow::bail!("Non-fast-forward merge required (manual intervention needed)");
            }

            Ok::<(), anyhow::Error>(())
        })
        .await??;

        spinner.finish_and_clear();
        Ok(())
    }

    /// Run makepkg with bubblewrap sandboxing if available
    /// Falls back to regular makepkg if bwrap is not installed
    async fn run_sandboxed_makepkg(
        &self,
        pkg_dir: &Path,
        env: &MakepkgEnv,
    ) -> Result<std::process::ExitStatus> {
        let package_name = pkg_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("package");
        let log_dir = self.build_dir.join("_logs");
        std::fs::create_dir_all(&log_dir)?;
        let log_path = log_dir.join(format!("{package_name}.log"));
        let log_file = File::create(&log_path)?;
        let log_file_err = log_file.try_clone()?;

        let spinner = create_spinner(&format!("Building {package_name}..."));

        // Check if bubblewrap is available
        let bwrap_available = Command::new("which")
            .arg("bwrap")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false);

        if bwrap_available {
            tracing::info!("Using bubblewrap sandbox for secure AUR build");
            println!("{} Building in sandbox (bubblewrap)...", "🔒".green());

            // Sandboxed build with bubblewrap
            // - Read-only bind: /usr, /etc, /lib, /lib64
            // - Writable: Build directory, /tmp
            // - Minimal device access
            let pkg_dir_str = pkg_dir.to_str().unwrap();
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());

            let pkgdest_str = env.pkgdest.to_string_lossy().to_string();
            let srcdest_str = env.srcdest.to_string_lossy().to_string();
            let pacman_db_dir = paths::pacman_db_dir().to_string_lossy().to_string();
            let pacman_cache_root = paths::pacman_cache_root_dir().to_string_lossy().to_string();

            let mut args = vec![
                "--ro-bind".to_string(),
                "/usr".to_string(),
                "/usr".to_string(),
                "--ro-bind".to_string(),
                "/etc".to_string(),
                "/etc".to_string(),
                "--ro-bind".to_string(),
                "/lib".to_string(),
                "/lib".to_string(),
                "--ro-bind".to_string(),
                "/lib64".to_string(),
                "/lib64".to_string(),
                "--symlink".to_string(),
                "/usr/bin".to_string(),
                "/bin".to_string(),
                "--symlink".to_string(),
                "/usr/sbin".to_string(),
                "/sbin".to_string(),
                "--bind".to_string(),
                pkg_dir_str.to_string(),
                pkg_dir_str.to_string(),
                "--bind".to_string(),
                pkgdest_str.clone(),
                pkgdest_str.clone(),
                "--bind".to_string(),
                srcdest_str.clone(),
                srcdest_str.clone(),
                "--tmpfs".to_string(),
                "/tmp".to_string(),
                "--dev".to_string(),
                "/dev".to_string(),
                "--proc".to_string(),
                "/proc".to_string(),
                "--ro-bind".to_string(),
                home.clone(),
                home,
                "--ro-bind".to_string(),
                pacman_db_dir.clone(),
                pacman_db_dir,
                "--ro-bind".to_string(),
                pacman_cache_root.clone(),
                pacman_cache_root,
                "--die-with-parent".to_string(),
                "--chdir".to_string(),
                pkg_dir_str.to_string(),
                "--setenv".to_string(),
                "MAKEFLAGS".to_string(),
                env.makeflags.clone(),
                "--setenv".to_string(),
                "PKGDEST".to_string(),
                pkgdest_str,
                "--setenv".to_string(),
                "SRCDEST".to_string(),
                srcdest_str,
            ];

            for (key, value) in &env.extra_env {
                args.push("--setenv".to_string());
                args.push(key.clone());
                args.push(value.clone());
            }

            args.extend([
                "--".to_string(),
                "makepkg".to_string(),
                "-s".to_string(),
                "--noconfirm".to_string(),
                "-f".to_string(),
                "--needed".to_string(),
            ]);

            let status = Command::new("bwrap")
                .args(args)
                .stdout(Stdio::from(log_file))
                .stderr(Stdio::from(log_file_err))
                .status()
                .await
                .context("Failed to run sandboxed makepkg")?;

            spinner.finish_and_clear();
            if !status.success() {
                println!("  {} Build failed. Log: {}", "✗".red(), log_path.display());
            }
            Ok(status)
        } else {
            tracing::debug!("bubblewrap not found, using regular makepkg");
            println!(
                "{} Building (install 'bubblewrap' for sandboxed builds)...",
                "→".dimmed()
            );

            let mut cmd = Command::new("makepkg");
            cmd.args(["-s", "--noconfirm", "-f", "--needed"])
                .env("MAKEFLAGS", &env.makeflags)
                .env("PKGDEST", &env.pkgdest)
                .env("SRCDEST", &env.srcdest);

            for (key, value) in &env.extra_env {
                cmd.env(key, value);
            }

            let status = cmd
                .current_dir(pkg_dir)
                .stdout(Stdio::from(log_file))
                .stderr(Stdio::from(log_file_err))
                .status()
                .await
                .context("Failed to run makepkg")?;

            spinner.finish_and_clear();
            if !status.success() {
                println!("  {} Build failed. Log: {}", "✗".red(), log_path.display());
            }
            Ok(status)
        }
    }

    fn makepkg_env(&self, pkg_dir: &Path) -> Result<MakepkgEnv> {
        let jobs = std::thread::available_parallelism()
            .map(|v| v.get())
            .unwrap_or(1);
        let makeflags = self
            .settings
            .aur
            .makeflags
            .clone()
            .or_else(|| std::env::var("MAKEFLAGS").ok())
            .unwrap_or_else(|| {
                if jobs > 1 {
                    format!("-j{jobs}")
                } else {
                    String::new()
                }
            });

        let pkgdest = self
            .settings
            .aur
            .pkgdest
            .clone()
            .unwrap_or_else(|| self.build_dir.join("_pkgdest"));
        let srcdest = self
            .settings
            .aur
            .srcdest
            .clone()
            .unwrap_or_else(|| self.build_dir.join("_srcdest"));

        std::fs::create_dir_all(&pkgdest)?;
        std::fs::create_dir_all(&srcdest)?;

        let mut extra_env = Vec::new();

        if self.settings.aur.enable_ccache {
            let ccache_dir = self
                .settings
                .aur
                .ccache_dir
                .clone()
                .unwrap_or_else(|| self.build_dir.join("_ccache"));
            std::fs::create_dir_all(&ccache_dir)?;
            extra_env.push((
                "CCACHE_DIR".to_string(),
                ccache_dir.to_string_lossy().to_string(),
            ));
            extra_env.push((
                "CCACHE_BASEDIR".to_string(),
                pkg_dir.to_string_lossy().to_string(),
            ));
        }

        if self.settings.aur.enable_sccache {
            let sccache_dir = self
                .settings
                .aur
                .sccache_dir
                .clone()
                .unwrap_or_else(|| self.build_dir.join("_sccache"));
            std::fs::create_dir_all(&sccache_dir)?;
            extra_env.push(("RUSTC_WRAPPER".to_string(), "sccache".to_string()));
            extra_env.push((
                "SCCACHE_DIR".to_string(),
                sccache_dir.to_string_lossy().to_string(),
            ));
        }

        Ok(MakepkgEnv {
            makeflags,
            pkgdest,
            srcdest,
            extra_env,
        })
    }

    fn cache_key(&self, pkg_dir: &Path, makeflags: &str) -> Result<String> {
        let pkgbuild = std::fs::read(pkg_dir.join("PKGBUILD"))?;
        let srcinfo = std::fs::read(pkg_dir.join(".SRCINFO")).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(pkgbuild);
        hasher.update(srcinfo);
        hasher.update(makeflags.as_bytes());
        Ok(format!("{:x}", hasher.finalize()))
    }

    fn cache_path(&self, package: &str) -> PathBuf {
        self.build_dir
            .join("_buildcache")
            .join(format!("{package}.hash"))
    }

    fn cached_package(
        &self,
        package: &str,
        pkgdest: &Path,
        cache_key: &str,
    ) -> Result<Option<PathBuf>> {
        if !self.settings.aur.cache_builds {
            return Ok(None);
        }

        let cache_path = self.cache_path(package);
        if !cache_path.exists() {
            return Ok(None);
        }

        let cached = std::fs::read_to_string(&cache_path).unwrap_or_default();
        if cached.trim() != cache_key {
            return Ok(None);
        }

        Ok(Self::find_package_in_dir(pkgdest))
    }

    fn write_cache_key(&self, package: &str, cache_key: &str) -> Result<()> {
        if !self.settings.aur.cache_builds {
            return Ok(());
        }

        let cache_path = self.cache_path(package);
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(cache_path, cache_key)?;
        Ok(())
    }

    /// Install the built package via sudo omg install <path>
    async fn install_built_package(&self, pkg_path: &PathBuf) -> Result<()> {
        println!(
            "{} Installing built package (elevating with sudo)...",
            "→".blue()
        );

        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("omg"));

        let status = Command::new("sudo")
            .arg("--")
            .arg(exe)
            .arg("install")
            .arg(pkg_path)
            .status()
            .await?;

        if !status.success() {
            anyhow::bail!("Installation failed");
        }

        Ok(())
    }

    /// Clean build directory for a package
    pub fn clean(&self, package: &str) -> Result<()> {
        let pkg_dir = self.build_dir.join(package);
        if pkg_dir.exists() {
            std::fs::remove_dir_all(&pkg_dir)?;
            println!("{} Cleaned build directory for {}", "✓".green(), package);
        }
        Ok(())
    }

    /// Clean all build directories
    pub fn clean_all(&self) -> Result<()> {
        if self.build_dir.exists() {
            std::fs::remove_dir_all(&self.build_dir)?;
            std::fs::create_dir_all(&self.build_dir)?;
            println!("{} Cleaned all AUR build directories", "✓".green());
        }
        Ok(())
    }
}

impl Default for AurClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a spinner
#[allow(clippy::literal_string_with_formatting_args)]
fn create_spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    pb
}

/// Search AUR with detailed info
pub async fn search_detailed(query: &str) -> Result<Vec<AurPackageDetail>> {
    let client = shared_client().clone();
    let url = format!("{AUR_RPC_URL}?v=5&type=search&arg={query}");

    let response: AurDetailedResponse = client
        .get(&url)
        .send()
        .await
        .context("Failed to connect to AUR RPC. Check your internet connection.")?
        .json()
        .await
        .context("Failed to parse AUR RPC response")?;

    Ok(response.results)
}

#[derive(Debug, Deserialize)]
struct AurDetailedResponse {
    results: Vec<AurPackageDetail>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AurPackageDetail {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Version")]
    pub version: String,
    #[serde(rename = "Description")]
    pub description: Option<String>,
    #[serde(rename = "Maintainer")]
    pub maintainer: Option<String>,
    #[serde(rename = "NumVotes")]
    pub num_votes: i32,
    #[serde(rename = "Popularity")]
    pub popularity: f64,
    #[serde(rename = "OutOfDate")]
    pub out_of_date: Option<i64>,
    #[serde(rename = "FirstSubmitted")]
    pub first_submitted: i64,
    #[serde(rename = "LastModified")]
    pub last_modified: i64,
    #[serde(rename = "URL")]
    pub url: Option<String>,
    #[serde(rename = "Depends")]
    pub depends: Option<Vec<String>>,
    #[serde(rename = "License")]
    pub license: Option<Vec<String>>,
}
