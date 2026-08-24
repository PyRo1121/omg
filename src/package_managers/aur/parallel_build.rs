use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::task::JoinSet;

use super::AurClient;

#[derive(Debug, Clone)]
pub struct BuildJob {
    /// AUR package base to clone and build.
    pub package: String,
    /// Split-package outputs from this base that should be installed.
    pub outputs: Vec<String>,
    /// Other package bases in this update set that must finish first.
    pub dependencies: Vec<String>,
}

impl BuildJob {
    #[must_use]
    pub fn new(package: String, dependencies: Vec<String>) -> Self {
        Self {
            outputs: vec![package.clone()],
            package,
            dependencies,
        }
    }

    #[must_use]
    pub fn for_package_base(
        package_base: String,
        mut outputs: Vec<String>,
        mut dependencies: Vec<String>,
    ) -> Self {
        outputs.sort();
        outputs.dedup();
        dependencies.sort();
        dependencies.dedup();
        Self {
            package: package_base,
            outputs,
            dependencies,
        }
    }
}

pub struct ParallelBuilder {
    client: Arc<AurClient>,
    max_concurrent: usize,
}

impl ParallelBuilder {
    /// # Panics
    /// Never panics; a `max_concurrent` of 0 is clamped to 1 so a wave can
    /// never silently skip every package.
    #[must_use]
    pub fn new(client: Arc<AurClient>, max_concurrent: usize) -> Self {
        Self {
            client,
            // Clamp to >=1: a zero limit would spawn no tasks per wave and
            // report success without building anything.
            max_concurrent: max_concurrent.max(1),
        }
    }

    pub async fn build_packages(&self, jobs: Vec<BuildJob>) -> Result<()> {
        if jobs.is_empty() {
            return Ok(());
        }

        // Start a shared sudoloop for the entire parallel build session.
        // This keeps credentials alive across all waves and prevents
        // individual builds from each trying to prompt for sudo.
        let _sudoloop = if crate::core::sudoloop::can_use_sudoloop() {
            tracing::debug!("Starting shared sudoloop for parallel AUR builds");
            Some(crate::core::sudoloop::SudoLoop::start())
        } else {
            None
        };

        let dep_graph = Self::build_dependency_graph(&jobs);
        let build_levels = Self::topological_levels(&dep_graph)?;
        let jobs_by_package: HashMap<String, BuildJob> = jobs
            .into_iter()
            .map(|job| (job.package.clone(), job))
            .collect();

        tracing::info!(
            "Building {} package base(s) in {} parallel wave(s)",
            jobs_by_package.len(),
            build_levels.len()
        );

        for (level_idx, level) in build_levels.iter().enumerate() {
            self.build_level(level_idx + 1, build_levels.len(), level, &jobs_by_package)
                .await?;
        }

        // _sudoloop is dropped here after all waves complete
        Ok(())
    }

    #[must_use]
    pub fn build_dependency_graph(jobs: &[BuildJob]) -> HashMap<String, HashSet<String>> {
        let mut graph: HashMap<String, HashSet<String>> = HashMap::new();

        let all_packages: HashSet<String> = jobs.iter().map(|j| j.package.clone()).collect();

        for job in jobs {
            let deps: HashSet<String> = job
                .dependencies
                .iter()
                .filter(|dep| all_packages.contains(*dep))
                .cloned()
                .collect();

            graph.insert(job.package.clone(), deps);
        }

        graph
    }

    pub fn topological_levels(
        graph: &HashMap<String, HashSet<String>>,
    ) -> Result<Vec<Vec<String>>> {
        let mut levels: Vec<Vec<String>> = Vec::new();
        let mut remaining: HashSet<String> = graph.keys().cloned().collect();
        let mut satisfied: HashSet<String> = HashSet::new();

        while !remaining.is_empty() {
            let mut current_level: Vec<String> = remaining
                .iter()
                .filter(|pkg| {
                    graph
                        .get(*pkg)
                        .is_none_or(|deps| deps.iter().all(|dep| satisfied.contains(dep)))
                })
                .cloned()
                .collect();

            if current_level.is_empty() {
                let cycle_packages: Vec<_> = remaining.iter().take(5).cloned().collect();
                anyhow::bail!("Circular dependency detected in AUR packages: {cycle_packages:?}");
            }

            current_level.sort();

            for pkg in &current_level {
                remaining.remove(pkg);
                satisfied.insert(pkg.clone());
            }

            levels.push(current_level);
        }

        Ok(levels)
    }

    /// Wave concurrency for a level. Clamped to >= 1: a zero limit would
    /// spawn no tasks and silently report success without building anything.
    #[must_use]
    fn concurrency_for_level(max_concurrent: usize, level_len: usize) -> usize {
        max_concurrent.max(1).min(level_len)
    }

    async fn build_level(
        &self,
        level_num: usize,
        total_levels: usize,
        packages: &[String],
        jobs: &HashMap<String, BuildJob>,
    ) -> Result<()> {
        use owo_colors::OwoColorize;

        // Builds within a wave run concurrently; the final `pacman -U` step
        // inside `AurClient::install` serializes on the process-wide
        // INSTALL_LOCK so concurrent installs cannot race pacman's database
        // lock (/var/lib/pacman/db.lck).
        println!();
        println!(
            "  {} Building wave {}/{} ({} package{})",
            "→".cyan().bold(),
            level_num,
            total_levels,
            packages.len(),
            if packages.len() == 1 { "" } else { "s" }
        );
        println!();

        let concurrency = Self::concurrency_for_level(self.max_concurrent, packages.len());

        let mut tasks = JoinSet::new();
        let mut package_iter = packages.iter();

        for _ in 0..concurrency {
            if let Some(pkg) = package_iter.next() {
                let client = Arc::clone(&self.client);
                let job = jobs
                    .get(pkg)
                    .cloned()
                    .with_context(|| format!("Missing build job for package base '{pkg}'"))?;

                tasks.spawn(async move {
                    tracing::info!("Building {} for outputs {:?}", job.package, job.outputs);
                    client
                        .install_package_outputs(&job.package, &job.outputs)
                        .await
                        .with_context(|| format!("Failed to build {}", job.package))
                });
            }
        }

        while let Some(result) = tasks.join_next().await {
            match result {
                Ok(Ok(())) => {
                    if let Some(pkg) = package_iter.next() {
                        let client = Arc::clone(&self.client);
                        let job = jobs.get(pkg).cloned().with_context(|| {
                            format!("Missing build job for package base '{pkg}'")
                        })?;

                        tasks.spawn(async move {
                            tracing::info!(
                                "Building {} for outputs {:?}",
                                job.package,
                                job.outputs
                            );
                            client
                                .install_package_outputs(&job.package, &job.outputs)
                                .await
                                .with_context(|| format!("Failed to build {}", job.package))
                        });
                    }
                }
                Ok(Err(e)) => {
                    tasks.abort_all();
                    return Err(e);
                }
                Err(e) => {
                    tasks.abort_all();
                    return Err(e.into());
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dependency_graph_simple() {
        let jobs = vec![
            BuildJob::new("a".to_string(), vec![]),
            BuildJob::new("b".to_string(), vec!["a".to_string()]),
            BuildJob::new("c".to_string(), vec!["b".to_string()]),
        ];

        let graph = ParallelBuilder::build_dependency_graph(&jobs);

        assert_eq!(graph.get("a").unwrap().len(), 0);
        assert!(graph.get("b").unwrap().contains("a"));
        assert!(graph.get("c").unwrap().contains("b"));
    }

    #[test]
    fn test_topological_levels() {
        let mut graph = HashMap::new();
        graph.insert("a".to_string(), HashSet::new());
        graph.insert("b".to_string(), ["a".to_string()].into());
        graph.insert("c".to_string(), ["a".to_string()].into());
        graph.insert("d".to_string(), ["b".to_string(), "c".to_string()].into());

        let levels = ParallelBuilder::topological_levels(&graph).unwrap();

        assert_eq!(levels.len(), 3);
        assert_eq!(levels[0], vec!["a"]);
        assert_eq!(levels[1].len(), 2);
        assert!(levels[1].contains(&"b".to_string()));
        assert!(levels[1].contains(&"c".to_string()));
        assert_eq!(levels[2], vec!["d"]);
    }

    #[test]
    fn test_independent_packages() {
        let mut graph = HashMap::new();
        graph.insert("pkg1".to_string(), HashSet::new());
        graph.insert("pkg2".to_string(), HashSet::new());
        graph.insert("pkg3".to_string(), HashSet::new());

        let levels = ParallelBuilder::topological_levels(&graph).unwrap();

        assert_eq!(levels.len(), 1);
        assert_eq!(levels[0].len(), 3);
    }

    #[test]
    fn test_zero_max_concurrent_is_clamped_to_one() {
        // Regression: max_concurrent == 0 used to spawn zero tasks per wave,
        // silently skipping every package while reporting success.
        assert_eq!(ParallelBuilder::concurrency_for_level(0, 5), 1);
        assert_eq!(ParallelBuilder::concurrency_for_level(2, 5), 2);
        assert_eq!(ParallelBuilder::concurrency_for_level(8, 3), 3);
    }

    #[test]
    fn test_circular_dependency_detection() {
        let mut graph = HashMap::new();
        graph.insert("a".to_string(), ["b".to_string()].into());
        graph.insert("b".to_string(), ["a".to_string()].into());

        let result = ParallelBuilder::topological_levels(&graph);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Circular dependency")
        );
    }
}
