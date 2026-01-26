//! Parser for /etc/pacman.conf to extract repository configuration

use std::path::Path;

use anyhow::{Context, Result};

#[derive(Debug, Clone, Default)]
pub struct RepoConfig {
    pub name: String,
    pub servers: Vec<String>,
    pub sig_level: Option<String>,
    pub include: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PacmanConfig {
    pub root_dir: Option<String>,
    pub db_path: Option<String>,
    pub cache_dir: Option<String>,
    pub log_file: Option<String>,
    pub gpg_dir: Option<String>,
    pub hook_dir: Option<String>,
    pub hold_pkg: Vec<String>,
    pub ignore_pkg: Vec<String>,
    pub ignore_group: Vec<String>,
    pub architecture: Option<String>,
    pub sig_level: Option<String>,
    pub local_file_sig_level: Option<String>,
    pub remote_file_sig_level: Option<String>,
    pub repos: Vec<RepoConfig>,
}

impl PacmanConfig {
    pub fn parse<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read {}", path.as_ref().display()))?;

        Self::parse_str(&content)
    }

    pub fn parse_str(content: &str) -> Result<Self> {
        let mut config = PacmanConfig::default();
        let mut current_section: Option<String> = None;
        let mut current_repo: Option<RepoConfig> = None;

        for line in content.lines() {
            let line = line.trim();

            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                if let Some(repo) = current_repo.take() {
                    config.repos.push(repo);
                }

                let section = &line[1..line.len() - 1];
                current_section = Some(section.to_string());

                if section != "options" {
                    current_repo = Some(RepoConfig {
                        name: section.to_string(),
                        ..Default::default()
                    });
                }
                continue;
            }

            let (key, value) = if let Some(eq_pos) = line.find('=') {
                let k = line[..eq_pos].trim();
                let v = line[eq_pos + 1..].trim();
                (k, Some(v))
            } else {
                (line, None)
            };

            match current_section.as_deref() {
                Some("options") => {
                    Self::parse_option(&mut config, key, value);
                }
                Some(_) => {
                    if let Some(ref mut repo) = current_repo {
                        Self::parse_repo_option(repo, key, value);
                    }
                }
                None => {}
            }
        }

        if let Some(repo) = current_repo {
            config.repos.push(repo);
        }

        Ok(config)
    }

    fn parse_option(config: &mut PacmanConfig, key: &str, value: Option<&str>) {
        match key {
            "RootDir" => config.root_dir = value.map(String::from),
            "DBPath" => config.db_path = value.map(String::from),
            "CacheDir" => config.cache_dir = value.map(String::from),
            "LogFile" => config.log_file = value.map(String::from),
            "GPGDir" => config.gpg_dir = value.map(String::from),
            "HookDir" => config.hook_dir = value.map(String::from),
            "Architecture" => config.architecture = value.map(String::from),
            "SigLevel" => config.sig_level = value.map(String::from),
            "LocalFileSigLevel" => config.local_file_sig_level = value.map(String::from),
            "RemoteFileSigLevel" => config.remote_file_sig_level = value.map(String::from),
            "HoldPkg" => {
                if let Some(v) = value {
                    config
                        .hold_pkg
                        .extend(v.split_whitespace().map(String::from));
                }
            }
            "IgnorePkg" => {
                if let Some(v) = value {
                    config
                        .ignore_pkg
                        .extend(v.split_whitespace().map(String::from));
                }
            }
            "IgnoreGroup" => {
                if let Some(v) = value {
                    config
                        .ignore_group
                        .extend(v.split_whitespace().map(String::from));
                }
            }
            _ => {}
        }
    }

    fn parse_repo_option(repo: &mut RepoConfig, key: &str, value: Option<&str>) {
        match key {
            "Server" => {
                if let Some(v) = value {
                    repo.servers.push(v.to_string());
                }
            }
            "SigLevel" => repo.sig_level = value.map(String::from),
            "Include" => repo.include = value.map(String::from),
            _ => {}
        }
    }

    pub fn get_repo_names(&self) -> Vec<&str> {
        self.repos.iter().map(|r| r.name.as_str()).collect()
    }

    pub fn resolve_servers(&self, repo: &RepoConfig, arch: &str) -> Result<Vec<String>> {
        let mut servers = Vec::new();

        for server in &repo.servers {
            servers.push(server.replace("$repo", &repo.name).replace("$arch", arch));
        }

        if let Some(ref include_path) = repo.include {
            let mirrorlist = std::fs::read_to_string(include_path)
                .with_context(|| format!("Failed to read mirrorlist: {include_path}"))?;

            for line in mirrorlist.lines() {
                let line = line.trim();
                if line.starts_with("Server")
                    && let Some(eq_pos) = line.find('=')
                {
                    let url = line[eq_pos + 1..].trim();
                    servers.push(url.replace("$repo", &repo.name).replace("$arch", arch));
                }
            }
        }

        Ok(servers)
    }
}

pub fn get_configured_repos() -> Result<Vec<String>> {
    let conf_path = crate::core::paths::pacman_conf_path();
    if !conf_path.exists() {
        return Ok(vec![
            "core".to_string(),
            "extra".to_string(),
            "multilib".to_string(),
        ]);
    }

    let config = PacmanConfig::parse(&conf_path)?;
    Ok(config.repos.into_iter().map(|r| r.name).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_config() {
        let content = r#"
[options]
RootDir = /
DBPath = /var/lib/pacman
Architecture = auto

[core]
Include = /etc/pacman.d/mirrorlist

[extra]
Include = /etc/pacman.d/mirrorlist

[multilib]
Include = /etc/pacman.d/mirrorlist
"#;

        let config = PacmanConfig::parse_str(content).unwrap();
        assert_eq!(config.root_dir, Some("/".to_string()));
        assert_eq!(config.db_path, Some("/var/lib/pacman".to_string()));
        assert_eq!(config.repos.len(), 3);
        assert_eq!(config.repos[0].name, "core");
        assert_eq!(config.repos[1].name, "extra");
        assert_eq!(config.repos[2].name, "multilib");
    }

    #[test]
    fn test_parse_custom_repos() {
        let content = r#"
[options]
Architecture = x86_64

[core]
Include = /etc/pacman.d/mirrorlist

[extra]
Include = /etc/pacman.d/mirrorlist

[chaotic-aur]
Server = https://cdn-mirror.chaotic.cx/$repo/$arch
Server = https://us-tx-mirror.chaotic.cx/$repo/$arch
"#;

        let config = PacmanConfig::parse_str(content).unwrap();
        assert_eq!(config.repos.len(), 3);
        assert_eq!(config.repos[2].name, "chaotic-aur");
        assert_eq!(config.repos[2].servers.len(), 2);
    }

    #[test]
    fn test_get_repo_names() {
        let content = r#"
[core]
Include = /etc/pacman.d/mirrorlist

[extra]
Include = /etc/pacman.d/mirrorlist

[custom-repo]
Server = https://example.com/$repo/$arch
"#;

        let config = PacmanConfig::parse_str(content).unwrap();
        let names = config.get_repo_names();
        assert_eq!(names, vec!["core", "extra", "custom-repo"]);
    }
}
