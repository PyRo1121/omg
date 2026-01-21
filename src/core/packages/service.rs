use anyhow::Result;
use std::sync::Arc;
use crate::package_managers::PackageManager;
use crate::core::Package;
use crate::package_managers::types::UpdateInfo;
use crate::core::security::SecurityPolicy;
use crate::core::history::{HistoryManager, PackageChange, TransactionType};

/// Service for orchestrating package operations across different backends.
pub struct PackageService {
    backend: Arc<dyn PackageManager>,
    policy: SecurityPolicy,
    history: Option<HistoryManager>,
    #[cfg(feature = "arch")]
    aur_client: Option<crate::package_managers::AurClient>,
}

impl PackageService {
    pub fn new(backend: Arc<dyn PackageManager>) -> Self {
        #[cfg(feature = "arch")]
        let aur_client = if backend.name() == "pacman" {
            Some(crate::package_managers::AurClient::new())
        } else {
            None
        };

        let policy = SecurityPolicy::load_default().unwrap_or_default();
        let history = HistoryManager::new().ok();

        Self {
            backend,
            policy,
            history,
            #[cfg(feature = "arch")]
            aur_client,
        }
    }

    /// Search for packages
    pub async fn search(&self, query: &str) -> Result<Vec<Package>> {
        let results = self.backend.search(query).await?;
        Ok(results)
    }

    /// Install packages, automatically resolving AUR packages if on Arch
    pub async fn install(&self, packages: &[String], _yes: bool) -> Result<()> {
        let mut changes: Vec<PackageChange> = Vec::new();

        #[cfg(feature = "arch")]
        if let Some(aur) = &self.aur_client {
            let mut official = Vec::new();
            let mut aur_pkgs = Vec::new();

            for pkg in packages {
                // Check if it's a local file
                if pkg.ends_with(".pkg.tar.zst") || pkg.ends_with(".pkg.tar.xz") {
                    official.push(pkg.clone());
                    // Note: Ideally we'd extract metadata for changes here, but keeping it simple for now
                    changes.push(PackageChange {
                        name: pkg.clone(),
                        old_version: None,
                        new_version: Some("local".to_string()),
                        source: "local".to_string(),
                    });
                    continue;
                }

                // Check if it's in official repos
                if let Ok(Some(info)) = self.backend.info(pkg).await {
                    let grade = self.policy.assign_grade(&info.name, &info.version, false, true).await;
                    self.policy.check_package(&info.name, false, None, grade)?;

                    official.push(pkg.clone());
                    changes.push(PackageChange {
                        name: info.name,
                        old_version: None,
                        new_version: Some(info.version.to_string()),
                        source: "official".to_string(),
                    });
                } else if let Ok(Some(info)) = aur.info(pkg).await {
                    let grade = self.policy.assign_grade(&info.name, &info.version, true, false).await;
                    self.policy.check_package(&info.name, true, None, grade)?;

                    aur_pkgs.push(pkg.clone());
                    changes.push(PackageChange {
                        name: info.name,
                        old_version: None,
                        new_version: Some(info.version.to_string()),
                        source: "aur".to_string(),
                    });
                } else {
                    anyhow::bail!("Package not found: {pkg}");
                }
            }

            let result = async {
                if !official.is_empty() {
                    self.backend.install(&official).await?;
                }

                for pkg in aur_pkgs {
                    aur.install(&pkg).await?;
                }
                Ok(())
            }.await;

            if let Some(history) = &self.history {
                let _ = history.add_transaction(TransactionType::Install, changes, result.is_ok());
            }
            return result;
        }

        // Generic fallback for non-arch
        #[cfg(not(feature = "arch"))]
        {
            for pkg in packages {
                if let Ok(Some(info)) = self.backend.info(pkg).await {
                    let grade = self.policy.assign_grade(&info.name, &info.version, false, true).await;
                    self.policy.check_package(&info.name, false, None, grade)?;

                    changes.push(PackageChange {
                        name: info.name,
                        old_version: None,
                        new_version: Some(info.version.to_string()),
                        source: self.backend.name().to_string(),
                    });
                } else {
                    anyhow::bail!("Package not found: {pkg}");
                }
            }
        }

        let result = self.backend.install(packages).await;
        if let Some(history) = &self.history {
            let _ = history.add_transaction(TransactionType::Install, changes, result.is_ok());
        }
        result
    }

    /// Remove packages
    pub async fn remove(&self, packages: &[String], _recursive: bool) -> Result<()> {
        let mut changes = Vec::new();
        for pkg in packages {
            if let Ok(Some(info)) = self.backend.info(pkg).await {
                 changes.push(PackageChange {
                    name: info.name,
                    old_version: Some(info.version.to_string()),
                    new_version: None,
                    source: self.backend.name().to_string(),
                });
            }
        }

        let result = self.backend.remove(packages).await;

        if let Some(history) = &self.history {
            let _ = history.add_transaction(TransactionType::Remove, changes, result.is_ok());
        }
        result
    }

    /// Update system
    pub async fn update(&self) -> Result<()> {
        let mut changes = Vec::new();

        // Get updates before proceeding to log them
        let updates = self.list_updates().await?;
        for up in &updates {
            changes.push(PackageChange {
                name: up.name.clone(),
                old_version: Some(up.old_version.clone()),
                new_version: Some(up.new_version.clone()),
                source: up.repo.clone(),
            });
        }

        let result = async {
            self.backend.update().await?;

            #[cfg(feature = "arch")]
            if let Some(aur) = &self.aur_client {
                let aur_updates = aur.get_update_list().await?;
                for (name, _, _) in aur_updates {
                    aur.install(&name).await?;
                }
            }
            Ok(())
        }.await;

        if let Some(history) = &self.history {
            let _ = history.add_transaction(TransactionType::Update, changes, result.is_ok());
        }
        result
    }

    /// List available updates
    pub async fn list_updates(&self) -> Result<Vec<UpdateInfo>> {
        let updates = self.backend.list_updates().await?;
        Ok(updates)
    }

    /// Get package info
    pub async fn info(&self, package: &str) -> Result<Option<Package>> {
        if let Ok(Some(pkg)) = self.backend.info(package).await {
            return Ok(Some(pkg));
        }

        #[cfg(feature = "arch")]
        if let Some(aur) = &self.aur_client {
            return aur.info(package).await;
        }

        Ok(None)
    }
}
