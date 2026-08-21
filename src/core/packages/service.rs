use crate::core::Package;
use crate::core::history::{HistoryManager, PackageChange, TransactionType};
use crate::core::security::{
    SecurityPolicy,
    vulnerability::{VulnerabilityScanner, VulnerabilitySource},
};
use crate::package_managers::PackageManager;
use crate::package_managers::types::UpdateInfo;
#[cfg(feature = "arch")]
use anyhow::Context;
use anyhow::Result;
use std::sync::Arc;

/// Service for orchestrating package operations across different backends.
pub struct PackageService {
    backend: Arc<dyn PackageManager>,
    policy: SecurityPolicy,
    vulnerability_source: Arc<dyn VulnerabilitySource>,
    history: Option<HistoryManager>,
    #[cfg(feature = "arch")]
    aur_client: Option<crate::package_managers::AurClient>,
}

impl PackageService {
    /// Create a new `PackageService` with the given backend
    pub fn new(backend: Arc<dyn PackageManager>) -> Result<Self> {
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

                // Check if it's in official repos. Repository lookup errors
                // are distinct from a package not being present.
                if let Some(info) = self.backend.info(pkg).await? {
                    let grade = self
                        .policy
                        .assign_grade(
                            self.vulnerability_source.as_ref(),
                            &info.name,
                            &info.version,
                            true,
                        )
                        .await?;
                    self.policy.check_package(&info.name, false, None, grade)?;

                    official.push(pkg.clone());
                    changes.push(PackageChange {
                        name: info.name,
                        old_version: None,
                        #[allow(
                            clippy::implicit_clone,
                            reason = "the package version type varies by backend feature"
                        )]
                        new_version: Some(info.version.to_string()),
                        source: "official".to_string(),
                    });
                } else if let Some(info) = aur.info(pkg).await? {
                    let grade = self
                        .policy
                        .assign_grade(
                            self.vulnerability_source.as_ref(),
                            &info.name,
                            &info.version,
                            false,
                        )
                        .await?;
                    self.policy.check_package(&info.name, true, None, grade)?;

                    aur_pkgs.push(pkg.clone());
                    changes.push(PackageChange {
                        name: info.name,
                        old_version: None,
                        #[allow(
                            clippy::implicit_clone,
                            reason = "the package version type varies by backend feature"
                        )]
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

            return self.finish_transaction(TransactionType::Install, changes, result);
        }

        // Generic fallback for non-arch
        #[cfg(not(feature = "arch"))]
        {
            for pkg in packages {
                if let Some(info) = self.backend.info(pkg).await? {
                    let grade = self
                        .policy
                        .assign_grade(
                            self.vulnerability_source.as_ref(),
                            &info.name,
                            &info.version,
                            true,
                        )
                        .await?;
                    self.policy.check_package(&info.name, false, None, grade)?;

                    changes.push(PackageChange {
                        name: info.name,
                        old_version: None,
                        #[allow(
                            clippy::implicit_clone,
                            reason = "the package version type varies by backend feature"
                        )]
                        new_version: Some(info.version.to_string()),
                        source: self.backend.name().to_string(),
                    });
                } else {
                    anyhow::bail!("Package not found: {pkg}");
                }
            }

            let result = self.backend.install(packages).await;
            self.finish_transaction(TransactionType::Install, changes, result)
        }

        // Fallback for Arch without AUR (shouldn't happen in practice)
        #[cfg(feature = "arch")]
        {
            for pkg in packages {
                if let Some(info) = self.backend.info(pkg).await? {
                    let grade = self
                        .policy
                        .assign_grade(
                            self.vulnerability_source.as_ref(),
                            &info.name,
                            &info.version,
                            true,
                        )
                        .await?;
                    self.policy.check_package(&info.name, false, None, grade)?;

                    changes.push(PackageChange {
                        name: info.name,
                        old_version: None,
                        #[allow(
                            clippy::implicit_clone,
                            reason = "the package version type varies by backend feature"
                        )]
                        new_version: Some(info.version.to_string()),
                        source: self.backend.name().to_string(),
                    });
                } else {
                    anyhow::bail!("Package not found: {pkg}");
                }
            }

            let result = self.backend.install(packages).await;
            self.finish_transaction(TransactionType::Install, changes, result)
        }
    }

    /// Remove packages
    pub async fn remove(&self, packages: &[String], _recursive: bool) -> Result<()> {
        let mut changes = Vec::new();
        for pkg in packages {
            if let Some(info) = self.backend.info(pkg).await? {
                changes.push(PackageChange {
                    name: info.name,
                    #[allow(
                        clippy::implicit_clone,
                        reason = "the package version type varies by backend feature"
                    )]
                    old_version: Some(info.version.to_string()),
                    new_version: None,
                    source: self.backend.name().to_string(),
                });
            }
        }

        let result = self.backend.remove(packages).await;

        self.finish_transaction(TransactionType::Remove, changes, result)
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

        self.finish_transaction(TransactionType::Update, changes, result)
    }

    fn finish_transaction(
        &self,
        transaction_type: TransactionType,
        changes: Vec<PackageChange>,
        operation_result: Result<()>,
    ) -> Result<()> {
        match &self.history {
            Some(history) => history.finish_operation(transaction_type, changes, operation_result),
            None => operation_result,
        }
    }

    /// List available updates
    pub async fn list_updates(&self) -> Result<Vec<UpdateInfo>> {
        #[allow(
            unused_mut,
            reason = "mutated only when the Arch AUR branch is compiled"
        )]
        let mut updates = self.backend.list_updates().await?;

        #[cfg(feature = "arch")]
        if let Some(aur) = &self.aur_client {
            for (name, old, new) in aur
                .get_update_list()
                .await
                .context("Failed to check AUR for updates")?
            {
                updates.push(UpdateInfo {
                    name,
                    old_version: old.to_string(),
                    new_version: new.to_string(),
                    repo: "aur".to_string(),
                });
            }
        }

        Ok(updates)
    }

    /// Get package info
    pub async fn info(&self, package: &str) -> Result<Option<Package>> {
        if let Some(pkg) = self.backend.info(package).await? {
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
enum HistoryConfiguration {
    Default,
    Custom(HistoryManager),
    Disabled,
}

pub struct PackageServiceBuilder {
    backend: Arc<dyn PackageManager>,
    policy: Option<SecurityPolicy>,
    vulnerability_source: Arc<dyn VulnerabilitySource>,
    history: HistoryConfiguration,
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
            vulnerability_source: Arc::new(VulnerabilityScanner::new()),
            history: HistoryConfiguration::Default,
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

    /// Set the vulnerability evidence source.
    #[must_use]
    pub fn vulnerability_source(mut self, source: Arc<dyn VulnerabilitySource>) -> Self {
        self.vulnerability_source = source;
        self
    }

    /// Set the history manager (defaults to `HistoryManager::new()`)
    #[must_use]
    pub fn history(mut self, history: HistoryManager) -> Self {
        self.history = HistoryConfiguration::Custom(history);
        self
    }

    /// Disable history tracking
    #[must_use]
    pub fn without_history(mut self) -> Self {
        self.history = HistoryConfiguration::Disabled;
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
    pub fn build(self) -> Result<PackageService> {
        #[cfg(feature = "arch")]
        let aur_client = if self.enable_aur && self.backend.name() == "pacman" {
            match self.aur_client {
                Some(client) => Some(client),
                None => Some(crate::package_managers::AurClient::new()?),
            }
        } else {
            self.aur_client
        };

        let history = match self.history {
            HistoryConfiguration::Default => Some(HistoryManager::new()?),
            HistoryConfiguration::Custom(history) => Some(history),
            HistoryConfiguration::Disabled => None,
        };

        Ok(PackageService {
            backend: self.backend,
            policy: self.policy.unwrap_or_default(),
            vulnerability_source: self.vulnerability_source,
            history,
            #[cfg(feature = "arch")]
            aur_client,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::pin::Pin;

    struct CleanVulnerabilitySource;

    impl VulnerabilitySource for CleanVulnerabilitySource {
        fn scan_package<'a>(
            &'a self,
            _name: &'a str,
            _version: &'a crate::package_managers::types::Version,
        ) -> Pin<
            Box<
                dyn Future<
                        Output = Result<
                            Vec<crate::core::security::vulnerability::VulnerabilityReport>,
                            crate::core::security::vulnerability::VulnerabilityError,
                        >,
                    > + Send
                    + 'a,
            >,
        > {
            Box::pin(async { Ok(Vec::new()) })
        }
    }

    #[test]
    fn builder_without_history_disables_explicit_history() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let history = HistoryManager::new_in(directory.path().join("history.json"))?;
        let backend = Arc::new(crate::core::testing::TestPackageManager::new());

        let service = PackageService::builder(backend)
            .history(history)
            .without_history()
            .build()?;

        assert!(service.history.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn operation_reports_history_persistence_failure() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let history_path = directory.path().join("history.json");
        let history = HistoryManager::new_in(&history_path)?;
        std::fs::create_dir(&history_path)?;

        let backend = Arc::new(crate::core::testing::TestPackageManager::new());
        backend.add_package("example", "1.0.0", "Example package");
        let service = PackageService::builder(backend)
            .history(history)
            .vulnerability_source(Arc::new(CleanVulnerabilitySource))
            .build()?;

        let error = service
            .install(&["example".to_string()], false)
            .await
            .expect_err("history persistence failure must be returned");
        assert!(error.to_string().contains("history"));
        Ok(())
    }
}
