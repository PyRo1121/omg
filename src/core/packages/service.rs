use crate::core::Package;
use crate::core::history::{HistoryManager, PackageChange, TransactionType};
use crate::core::security::SecurityPolicy;
use crate::package_managers::PackageManager;
use crate::package_managers::types::UpdateInfo;
use anyhow::Result;
use std::sync::Arc;

/// Service for orchestrating package operations across different backends.
pub struct PackageService {
    backend: Arc<dyn PackageManager>,
    policy: SecurityPolicy,
    history: Option<HistoryManager>,
    #[cfg(feature = "arch")]
    aur_client: Option<crate::package_managers::AurClient>,
}

impl PackageService {
    /// Create a new `PackageService` with the given backend
    pub fn new(backend: Arc<dyn PackageManager>) -> Self {
        Self::builder(backend).build()
    }

    /// Create a builder for constructing `PackageService` with custom dependencies
    pub fn builder(backend: Arc<dyn PackageManager>) -> PackageServiceBuilder {
        PackageServiceBuilder::new(backend)
    }

    /// Search for packages
    pub async fn search(&self, query: &str) -> Result<Vec<Package>> {
        let results = self.backend.search(query).await?;
        Ok(results)
    }

    /// Install packages, automatically resolving AUR packages if on Arch
    ///
    /// The `yes` parameter is accepted for API compatibility but not used at this layer.
    /// Interactive prompts are handled by the underlying package manager implementations.
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
                    let grade = self
                        .policy
                        .assign_grade(&info.name, &info.version, false, true)
                        .await;
                    self.policy.check_package(&info.name, false, None, grade)?;

                    official.push(pkg.clone());
                    changes.push(PackageChange {
                        name: info.name,
                        old_version: None,
                        #[allow(clippy::implicit_clone)]
                        new_version: Some(info.version.to_string()),
                        source: "official".to_string(),
                    });
                } else if let Ok(Some(info)) = aur.info(pkg).await {
                    let grade = self
                        .policy
                        .assign_grade(&info.name, &info.version, true, false)
                        .await;
                    self.policy.check_package(&info.name, true, None, grade)?;

                    aur_pkgs.push(pkg.clone());
                    changes.push(PackageChange {
                        name: info.name,
                        old_version: None,
                        #[allow(clippy::implicit_clone)]
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
            }
            .await;

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
                    let grade = self
                        .policy
                        .assign_grade(&info.name, &info.version, false, true)
                        .await;
                    self.policy.check_package(&info.name, false, None, grade)?;

                    changes.push(PackageChange {
                        name: info.name,
                        old_version: None,
                        #[allow(clippy::implicit_clone)]
                        new_version: Some(info.version.to_string()),
                        source: self.backend.name().to_string(),
                    });
                } else {
                    anyhow::bail!("Package not found: {pkg}");
                }
            }

            let result = self.backend.install(packages).await;
            if let Some(history) = &self.history {
                let _ = history.add_transaction(TransactionType::Install, changes, result.is_ok());
            }
            result
        }

        // Fallback for Arch without AUR (shouldn't happen in practice)
        #[cfg(feature = "arch")]
        {
            for pkg in packages {
                if let Ok(Some(info)) = self.backend.info(pkg).await {
                    let grade = self
                        .policy
                        .assign_grade(&info.name, &info.version, false, true)
                        .await;
                    self.policy.check_package(&info.name, false, None, grade)?;

                    changes.push(PackageChange {
                        name: info.name,
                        old_version: None,
                        #[allow(clippy::implicit_clone)]
                        new_version: Some(info.version.to_string()),
                        source: self.backend.name().to_string(),
                    });
                } else {
                    anyhow::bail!("Package not found: {pkg}");
                }
            }

            let result = self.backend.install(packages).await;
            if let Some(history) = &self.history {
                let _ = history.add_transaction(TransactionType::Install, changes, result.is_ok());
            }
            result
        }
    }

    /// Remove packages
    pub async fn remove(&self, packages: &[String], _recursive: bool) -> Result<()> {
        let mut changes = Vec::new();
        for pkg in packages {
            if let Ok(Some(info)) = self.backend.info(pkg).await {
                changes.push(PackageChange {
                    name: info.name,
                    #[allow(clippy::implicit_clone)]
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
        }
        .await;

        if let Some(history) = &self.history {
            let _ = history.add_transaction(TransactionType::Update, changes, result.is_ok());
        }
        result
    }

    /// List available updates
    pub async fn list_updates(&self) -> Result<Vec<UpdateInfo>> {
        let mut updates = self.backend.list_updates().await?;

        #[cfg(feature = "arch")]
        if let Some(aur) = &self.aur_client {
            match aur.get_update_list().await {
                Ok(aur_updates) => {
                    for (name, old, new) in aur_updates {
                        updates.push(UpdateInfo {
                            name,
                            old_version: old.to_string(),
                            new_version: new.to_string(),
                            repo: "aur".to_string(),
                        });
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to check AUR for updates: {}", e);
                }
            }
        }

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

    /// Get system status (total, explicit, orphans, updates)
    pub async fn get_status(&self, fast: bool) -> Result<(usize, usize, usize, usize)> {
        self.backend.get_status(fast).await
    }
}

/// Builder for `PackageService` with dependency injection support
pub struct PackageServiceBuilder {
    backend: Arc<dyn PackageManager>,
    policy: Option<SecurityPolicy>,
    history: Option<HistoryManager>,
    #[cfg(feature = "arch")]
    aur_client: Option<crate::package_managers::AurClient>,
    #[cfg(feature = "arch")]
    enable_aur: bool,
}

impl PackageServiceBuilder {
    /// Create a new builder with the required backend
    pub fn new(backend: Arc<dyn PackageManager>) -> Self {
        Self {
            backend,
            policy: None,
            history: None,
            #[cfg(feature = "arch")]
            aur_client: None,
            #[cfg(feature = "arch")]
            enable_aur: true,
        }
    }

    /// Set the security policy (defaults to `SecurityPolicy::default()`)
    #[must_use]
    pub fn policy(mut self, policy: SecurityPolicy) -> Self {
        self.policy = Some(policy);
        self
    }

    /// Set the history manager (defaults to `HistoryManager::new()`)
    #[must_use]
    pub fn history(mut self, history: HistoryManager) -> Self {
        self.history = Some(history);
        self
    }

    /// Disable history tracking
    #[must_use]
    pub fn without_history(mut self) -> Self {
        self.history = None;
        self
    }

    /// Set the AUR client (Arch only)
    #[cfg(feature = "arch")]
    #[must_use]
    pub fn aur_client(mut self, client: crate::package_managers::AurClient) -> Self {
        self.aur_client = Some(client);
        self
    }

    /// Disable AUR support (Arch only)
    #[cfg(feature = "arch")]
    #[must_use]
    pub fn without_aur(mut self) -> Self {
        self.enable_aur = false;
        self.aur_client = None;
        self
    }

    /// Build the `PackageService`
    pub fn build(self) -> PackageService {
        #[cfg(feature = "arch")]
        let aur_client = if self.enable_aur && self.backend.name() == "pacman" {
            self.aur_client
                .or_else(|| Some(crate::package_managers::AurClient::new()))
        } else {
            self.aur_client
        };

        #[cfg(not(feature = "arch"))]
        let aur_client = None;

        PackageService {
            backend: self.backend,
            policy: self.policy.unwrap_or_default(),
            history: self.history.or_else(|| HistoryManager::new().ok()),
            #[cfg(feature = "arch")]
            aur_client,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_with_defaults() {
        // This test just verifies the builder compiles and runs
        // In a real test, we'd use a mock backend
        let backend = Arc::new(crate::core::testing::TestPackageManager::new());
        let service = PackageService::builder(backend).build();
        // Service was created successfully
        let _ = service;
    }

    #[test]
    fn test_builder_without_history() {
        let backend = Arc::new(crate::core::testing::TestPackageManager::new());
        let service = PackageService::builder(backend).without_history().build();
        // Service was created without history
        let _ = service;
    }

    #[test]
    fn test_builder_with_custom_policy() {
        let backend = Arc::new(crate::core::testing::TestPackageManager::new());
        let policy = SecurityPolicy::default();
        let service = PackageService::builder(backend).policy(policy).build();
        // Service was created with custom policy
        let _ = service;
    }
}
