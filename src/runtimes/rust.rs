//! Native Rust toolchain manager - PURE RUST, NO RUSTUP
//!
//! Downloads Rust toolchains directly from static.rust-lang.org

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use tar::Archive;

use crate::core::http::download_client;
const RUST_DIST_URL: &str = "https://static.rust-lang.org/dist";
const RUST_MANIFEST_PREFIX: &str = "channel-rust";
const RUST_METADATA_FILE: &str = ".omg-toolchain.toml";

/// Rust version info
#[derive(Debug, Clone)]
pub struct RustVersion {
    pub version: String,
    pub channel: String,
}

fn manifest_component_version(manifest: &toml::Value, component: &str) -> Result<String> {
    let value = manifest
        .get("pkg")
        .and_then(|pkg| pkg.get(component))
        .and_then(|pkg| pkg.get("version"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("Missing version for component '{component}'"))?;
    Ok(value.split(' ').next().unwrap_or(value).to_string())
}

fn is_date_parts(year: &str, month: &str, day: &str) -> bool {
    year.len() == 4
        && month.len() == 2
        && day.len() == 2
        && year.chars().all(|c| c.is_ascii_digit())
        && month.chars().all(|c| c.is_ascii_digit())
        && day.chars().all(|c| c.is_ascii_digit())
}

#[derive(Debug, Clone)]
pub struct RustToolchainSpec {
    pub channel: String,
    pub date: Option<String>,
    pub host: String,
}

#[derive(Debug, Clone, Default)]
pub struct RustToolchainRequest {
    pub channel: String,
    pub profile: Option<String>,
    pub components: Vec<String>,
    pub targets: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RustToolchainStatus {
    pub name: String,
    pub needs_install: bool,
    pub missing_components: Vec<String>,
    pub missing_targets: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct RustToolchainFile {
    toolchain: RustToolchainSection,
}

#[derive(Debug, Deserialize)]
pub struct RustToolchainSection {
    channel: String,
    components: Option<Vec<String>>,
    targets: Option<Vec<String>>,
    profile: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct RustToolchainMetadata {
    components: BTreeSet<String>,
    targets: BTreeSet<String>,
}

pub struct RustManager {
    versions_dir: PathBuf,
    current_link: PathBuf,
    client: reqwest::Client,
}

impl RustManager {
    pub fn new() -> Self {
        let data_dir = super::DATA_DIR.clone();

        Self {
            versions_dir: data_dir.join("versions").join("rust"),
            current_link: data_dir.join("versions").join("rust").join("current"),
            client: download_client().clone(),
        }
    }

    #[must_use]
    pub fn bin_dir(&self) -> PathBuf {
        self.current_link.join("bin")
    }

    /// List available Rust versions (stable, beta, nightly + recent releases)
    pub async fn list_available(&self) -> Result<Vec<RustVersion>> {
        let mut versions = Vec::new();

        // Get stable version from manifest
        if let Ok(manifest) = self.fetch_manifest("stable", None).await
            && let Some(version) = manifest_version(&manifest)
        {
            versions.push(RustVersion {
                version,
                channel: "stable".to_string(),
            });
        }

        // Add channel aliases
        versions.push(RustVersion {
            version: "stable".to_string(),
            channel: "stable".to_string(),
        });
        versions.push(RustVersion {
            version: "beta".to_string(),
            channel: "beta".to_string(),
        });
        versions.push(RustVersion {
            version: "nightly".to_string(),
            channel: "nightly".to_string(),
        });

        Ok(versions)
    }

    pub fn list_installed(&self) -> Result<Vec<String>> {
        if !self.versions_dir.exists() {
            return Ok(Vec::new());
        }

        let mut versions = Vec::new();
        for entry in fs::read_dir(&self.versions_dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name != "current" && entry.file_type()?.is_dir() {
                versions.push(name);
            }
        }
        versions.sort();
        versions.reverse();
        Ok(versions)
    }

    #[must_use]
    pub fn current_version(&self) -> Option<String> {
        fs::read_link(&self.current_link)
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
    }

    /// Install Rust - PURE RUST, NO SUBPROCESS
    pub async fn install(&self, version: &str) -> Result<()> {
        let toolchain = RustToolchainSpec::parse(version)?;
        let version_dir = self.toolchain_dir(&toolchain);

        if version_dir.exists() {
            println!("{} Rust {} is already installed", "✓".green(), version);
            return self.use_version(version);
        }

        println!(
            "{} Installing Rust {}...\n",
            "OMG".cyan().bold(),
            version.yellow()
        );

        self.install_with_profile(&toolchain, "default", &[], &[])
            .await?;
        self.use_version(version)?;

        Ok(())
    }

    pub fn toolchain_status(&self, request: &RustToolchainRequest) -> Result<RustToolchainStatus> {
        let toolchain = RustToolchainSpec::parse(&request.channel)?;
        let needs_install = !self.toolchain_dir(&toolchain).exists();
        let missing_components = self.missing_components(&toolchain, &request.components)?;
        let missing_targets = self.missing_targets(&toolchain, &request.targets)?;

        Ok(RustToolchainStatus {
            name: toolchain.name(),
            needs_install,
            missing_components,
            missing_targets,
        })
    }

    pub async fn ensure_toolchain(&self, request: &RustToolchainRequest) -> Result<()> {
        let toolchain = RustToolchainSpec::parse(&request.channel)?;
        let profile = request.profile.as_deref().unwrap_or("default");
        let needs_install = !self.toolchain_dir(&toolchain).exists();
        let needs_components = self.missing_components(&toolchain, &request.components)?;
        let needs_targets = self.missing_targets(&toolchain, &request.targets)?;

        if !needs_install && needs_components.is_empty() && needs_targets.is_empty() {
            return Ok(());
        }

        if needs_install {
            self.install_with_profile(&toolchain, profile, &request.components, &request.targets)
                .await?;
        } else {
            if !needs_components.is_empty() {
                self.install_components(&toolchain, &needs_components)
                    .await?;
            }
            if !needs_targets.is_empty() {
                self.install_targets(&toolchain, &needs_targets).await?;
            }
        }

        Ok(())
    }

    pub fn toolchain_dir(&self, toolchain: &RustToolchainSpec) -> PathBuf {
        self.versions_dir.join(toolchain.name())
    }

    fn extract_component(
        archive_path: &Path,
        dest_dir: &Path,
        component: &str,
        version: &str,
        target: &str,
    ) -> Result<()> {
        let filename = archive_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");

        if filename.ends_with(".tar.xz") {
            let file = File::open(archive_path)?;
            let mut decompressed = Vec::new();
            lzma_rs::xz_decompress(&mut std::io::BufReader::new(file), &mut decompressed)
                .with_context(|| "Failed to decompress XZ archive")?;
            let mut archive = Archive::new(decompressed.as_slice());
            Self::extract_component_entries(&mut archive, dest_dir, component, version, target)?;
        } else {
            let file = File::open(archive_path)?;
            let decoder = GzDecoder::new(file);
            let mut archive = Archive::new(decoder);
            Self::extract_component_entries(&mut archive, dest_dir, component, version, target)?;
        }

        Ok(())
    }

    fn extract_component_entries<R: std::io::Read>(
        archive: &mut Archive<R>,
        dest_dir: &Path,
        component: &str,
        version: &str,
        target: &str,
    ) -> Result<()> {
        let _prefix = format!("{component}-{version}-{target}");

        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;
            let path_str = path.to_string_lossy();

            // Skip manifest and installer files, only extract from the component subdirectory
            if !path_str.contains("/lib/")
                && !path_str.contains("/bin/")
                && !path_str.contains("/share/")
            {
                continue;
            }

            // Strip prefix and component name
            let stripped: PathBuf = path
                .components()
                .skip(2) // Skip "component-version-target/component/"
                .collect();

            if stripped.as_os_str().is_empty() {
                continue;
            }

            let dest_path = dest_dir.join(&stripped);

            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }

            if entry.header().entry_type().is_dir() {
                fs::create_dir_all(&dest_path)?;
            } else {
                entry.unpack(&dest_path)?;
            }
        }

        Ok(())
    }

    async fn download_file(&self, url: &str, path: &PathBuf) -> Result<()> {
        let response =
            self.client.get(url).send().await.with_context(|| {
                format!("Failed to download from {url}. Check your connection.")
            })?;

        if !response.status().is_success() {
            anyhow::bail!(
                "✗ Download Error: Server returned {} for {}",
                response.status(),
                url
            );
        }

        let total = response.content_length().unwrap_or(0);
        let pb = ProgressBar::new(total);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .unwrap()
                .progress_chars("█▓▒░"),
        );

        let bytes = response
            .bytes()
            .await
            .context("Failed to read download stream")?;
        pb.set_position(bytes.len() as u64);
        tokio::fs::write(path, &bytes)
            .await
            .with_context(|| format!("Failed to write to {}. Check disk space.", path.display()))?;
        pb.finish_and_clear();
        Ok(())
    }

    pub fn use_version(&self, version: &str) -> Result<()> {
        let toolchain = RustToolchainSpec::parse(version)?;
        let version_dir = self.toolchain_dir(&toolchain);

        if !version_dir.exists() {
            anyhow::bail!("Rust {version} is not installed");
        }

        if self.current_link.exists() {
            fs::remove_file(&self.current_link)?;
        }

        #[cfg(unix)]
        std::os::unix::fs::symlink(&version_dir, &self.current_link)?;

        println!("{} Now using Rust {}", "✓".green(), version);
        println!(
            "  Add to PATH: {}",
            self.bin_dir().display().to_string().dimmed()
        );

        Ok(())
    }

    pub fn uninstall(&self, version: &str) -> Result<()> {
        let toolchain = RustToolchainSpec::parse(version)?;
        let version_dir = self.toolchain_dir(&toolchain);

        if !version_dir.exists() {
            println!("{} Rust {} is not installed", "→".dimmed(), version);
            return Ok(());
        }

        if let Some(current) = self.current_version()
            && current == version
        {
            let _ = fs::remove_file(&self.current_link);
        }

        fs::remove_dir_all(&version_dir)?;
        println!("{} Rust {} uninstalled", "✓".green(), version);
        Ok(())
    }
}

impl RustManager {
    pub fn parse_toolchain_file(path: &Path) -> Result<RustToolchainRequest> {
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "rust-toolchain.toml")
        {
            let content = fs::read_to_string(path)?;
            let parsed: RustToolchainFile = toml::from_str(&content)?;
            return Ok(RustToolchainRequest {
                channel: parsed.toolchain.channel,
                profile: parsed.toolchain.profile,
                components: parsed.toolchain.components.unwrap_or_default(),
                targets: parsed.toolchain.targets.unwrap_or_default(),
            });
        }

        let channel = fs::read_to_string(path)?.trim().to_string();
        Ok(RustToolchainRequest {
            channel,
            ..RustToolchainRequest::default()
        })
    }

    async fn install_with_profile(
        &self,
        toolchain: &RustToolchainSpec,
        profile: &str,
        components: &[String],
        targets: &[String],
    ) -> Result<()> {
        let version_dir = self.toolchain_dir(toolchain);
        fs::create_dir_all(&self.versions_dir)?;
        fs::create_dir_all(&version_dir)?;

        let mut required_components = profile_components(profile)?;
        required_components.extend(components.iter().cloned());
        required_components.sort();
        required_components.dedup();

        for component in required_components.clone() {
            self.install_component(toolchain, &component, &toolchain.host)
                .await?;
        }

        if !targets.is_empty() {
            self.install_targets(toolchain, targets).await?;
        }

        let mut metadata = Self::read_metadata(&version_dir)?;
        for component in required_components {
            metadata.components.insert(component);
        }
        for target in targets {
            metadata.targets.insert(target.clone());
        }
        Self::write_metadata(&version_dir, &metadata)?;

        println!("{} Rust {} installed!", "✓".green(), toolchain.name());

        Ok(())
    }

    async fn install_components(
        &self,
        toolchain: &RustToolchainSpec,
        components: &[String],
    ) -> Result<()> {
        let version_dir = self.toolchain_dir(toolchain);
        let mut metadata = Self::read_metadata(&version_dir)?;

        for component in components {
            self.install_component(toolchain, component, &toolchain.host)
                .await?;
            metadata.components.insert(component.clone());
        }

        Self::write_metadata(&version_dir, &metadata)?;
        Ok(())
    }

    async fn install_targets(
        &self,
        toolchain: &RustToolchainSpec,
        targets: &[String],
    ) -> Result<()> {
        let version_dir = self.toolchain_dir(toolchain);
        let mut metadata = Self::read_metadata(&version_dir)?;

        for target in targets {
            self.install_component(toolchain, "rust-std", target)
                .await?;
            metadata.targets.insert(target.clone());
        }

        Self::write_metadata(&version_dir, &metadata)?;
        Ok(())
    }

    async fn install_component(
        &self,
        toolchain: &RustToolchainSpec,
        component: &str,
        target: &str,
    ) -> Result<()> {
        println!("{} Downloading {}...", "→".blue(), component);
        let manifest = self
            .fetch_manifest(&toolchain.channel, toolchain.date.as_deref())
            .await?;
        let component_version = manifest_component_version(&manifest, component)?;
        let url = manifest_component_url(&manifest, component, target)?;
        let filename = url
            .split('/')
            .next_back()
            .ok_or_else(|| anyhow::anyhow!("Invalid download URL for {component}"))?;
        let download_path = self.versions_dir.join(filename);

        self.download_file(&url, &download_path).await?;
        println!("{} Extracting {}...", "→".blue(), component);
        Self::extract_component(
            &download_path,
            &self.toolchain_dir(toolchain),
            component,
            &component_version,
            target,
        )?;
        let _ = fs::remove_file(&download_path);
        Ok(())
    }

    fn read_metadata(toolchain_dir: &Path) -> Result<RustToolchainMetadata> {
        let path = toolchain_dir.join(RUST_METADATA_FILE);
        if !path.exists() {
            return Ok(RustToolchainMetadata::default());
        }
        let content = fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }

    fn write_metadata(toolchain_dir: &Path, metadata: &RustToolchainMetadata) -> Result<()> {
        let path = toolchain_dir.join(RUST_METADATA_FILE);
        let content = toml::to_string_pretty(metadata)?;
        fs::write(path, content)?;
        Ok(())
    }

    fn missing_components(
        &self,
        toolchain: &RustToolchainSpec,
        requested: &[String],
    ) -> Result<Vec<String>> {
        let metadata = Self::read_metadata(&self.toolchain_dir(toolchain))?;
        Ok(requested
            .iter()
            .filter(|component| !metadata.components.contains(*component))
            .cloned()
            .collect())
    }

    fn missing_targets(
        &self,
        toolchain: &RustToolchainSpec,
        requested: &[String],
    ) -> Result<Vec<String>> {
        let metadata = Self::read_metadata(&self.toolchain_dir(toolchain))?;
        Ok(requested
            .iter()
            .filter(|target| !metadata.targets.contains(*target))
            .cloned()
            .collect())
    }

    async fn fetch_manifest(&self, channel: &str, date: Option<&str>) -> Result<toml::Value> {
        let filename = format!("{RUST_MANIFEST_PREFIX}-{channel}.toml");
        let url = if let Some(date) = date {
            format!("{RUST_DIST_URL}/{date}/{filename}")
        } else {
            format!("{RUST_DIST_URL}/{filename}")
        };

        let manifest = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch Rust version manifest. Check your internet connection.")?
            .text()
            .await
            .context("Failed to read Rust version manifest")?;

        Ok(toml::from_str(&manifest)?)
    }
}

impl Default for RustManager {
    fn default() -> Self {
        Self::new()
    }
}

impl RustToolchainSpec {
    pub fn parse(input: &str) -> Result<Self> {
        let input = input.trim();
        let mut segments: Vec<&str> = input.split('-').collect();
        let mut channel = segments.first().copied().unwrap_or(input).to_string();

        if channel.chars().next().is_some_and(|c| c.is_ascii_digit())
            && segments.get(1).is_some_and(|seg| seg.starts_with("beta"))
        {
            channel.push('-');
            channel.push_str(segments[1]);
            segments.drain(0..2);
        } else {
            segments.remove(0);
        }

        let mut date = None;
        if segments.len() >= 3 && is_date_parts(segments[0], segments[1], segments[2]) {
            date = Some(format!("{}-{}-{}", segments[0], segments[1], segments[2]));
            segments.drain(0..3);
        }

        let host = if segments.is_empty() {
            default_host_triple()?
        } else {
            segments.join("-")
        };

        Ok(Self {
            channel,
            date,
            host,
        })
    }

    pub fn name(&self) -> String {
        let mut name = self.channel.clone();
        if let Some(date) = &self.date {
            name.push('-');
            name.push_str(date);
        }
        name.push('-');
        name.push_str(&self.host);
        name
    }
}

fn default_host_triple() -> Result<String> {
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;
    match (arch, os) {
        ("x86_64", "linux") => Ok("x86_64-unknown-linux-gnu".to_string()),
        ("aarch64", "linux") => Ok("aarch64-unknown-linux-gnu".to_string()),
        _ => anyhow::bail!("Unsupported host platform: {arch}-{os}"),
    }
}

fn profile_components(profile: &str) -> Result<Vec<String>> {
    match profile {
        "minimal" => Ok(vec!["rustc", "cargo", "rust-std"]
            .into_iter()
            .map(str::to_string)
            .collect()),
        "default" | "complete" => Ok(vec![
            "rustc",
            "cargo",
            "rust-std",
            "rustfmt",
            "clippy",
            "rust-docs",
        ]
        .into_iter()
        .map(str::to_string)
        .collect()),
        other => anyhow::bail!("Unknown Rust profile: {other}"),
    }
}

fn manifest_version(manifest: &toml::Value) -> Option<String> {
    manifest
        .get("pkg")
        .and_then(|pkg| pkg.get("rustc"))
        .and_then(|rustc| rustc.get("version"))
        .and_then(|value| value.as_str())
        .map(|value| value.split(' ').next().unwrap_or(value).to_string())
}

fn manifest_component_url(manifest: &toml::Value, component: &str, target: &str) -> Result<String> {
    let pkg = manifest
        .get("pkg")
        .and_then(|pkg| pkg.get(component))
        .ok_or_else(|| anyhow::anyhow!("Component '{component}' not found in manifest"))?;
    let target_info = pkg
        .get("target")
        .and_then(|targets| targets.get(target))
        .ok_or_else(|| {
            anyhow::anyhow!("Target '{target}' not found for component '{component}'")
        })?;

    let url = target_info
        .get("xz_url")
        .and_then(toml::Value::as_str)
        .or_else(|| target_info.get("url").and_then(toml::Value::as_str))
        .ok_or_else(|| anyhow::anyhow!("No download URL for {component} on {target}"))?;

    Ok(url.to_string())
}
