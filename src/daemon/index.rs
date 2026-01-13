//! High-performance in-memory package index using Nucleo
//!
//! Provides sub-millisecond fuzzy search and instant lookups
//! by mirroring libalpm databases in memory.

use anyhow::{Context, Result};
use nucleo_matcher::{Config, Matcher, Utf32String};
use ahash::AHashMap;
use parking_lot::{Mutex, RwLock};

use crate::daemon::protocol::PackageInfo;

pub struct PackageIndex {
    /// Maps package name to info (using ahash for speed)
    packages: AHashMap<String, PackageInfo>,
    /// Search items for Nucleo
    search_items: Vec<(String, Utf32String)>,
    /// The matcher (protected by Mutex for thread safety)
    matcher: Mutex<Matcher>,
    /// Reader-writer lock for package lookups
    lock: RwLock<()>,
}

impl PackageIndex {
    pub fn new() -> Result<Self> {
        let mut packages = AHashMap::default();
        let mut search_items = Vec::new();

        // Initialize ALPM and read all databases
        let alpm = alpm::Alpm::new("/", "/var/lib/pacman")
            .context("Failed to initialize ALPM for indexing")?;
        
        for db_name in ["core", "extra", "multilib"] {
            let db = alpm.register_syncdb(db_name, alpm::SigLevel::USE_DEFAULT)?;
            for pkg in db.pkgs() {
                let info = PackageInfo {
                    name: pkg.name().to_string(),
                    version: pkg.version().to_string(),
                    description: pkg.desc().unwrap_or("").to_string(),
                    source: "official".to_string(),
                };
                
                let search_str = format!("{} {}", info.name, info.description);
                search_items.push((info.name.clone(), Utf32String::from(search_str.as_str())));
                packages.insert(info.name.clone(), info);
            }
        }

        tracing::info!("Indexed {} packages from official repos", packages.len());

        Ok(Self {
            packages,
            search_items,
            matcher: Mutex::new(Matcher::new(Config::DEFAULT)),
            lock: RwLock::new(()),
        })
    }

    /// Fuzzy search for packages
    pub fn search(&self, query: &str, limit: usize) -> Vec<PackageInfo> {
        if query.is_empty() {
            return Vec::new();
        }

        let _read_guard = self.lock.read();
        let mut matches = Vec::new();
        let query_utf32 = Utf32String::from(query);
        let mut matcher = self.matcher.lock();

        // Perform search
        for (name, search_str) in &self.search_items {
            if let Some(score) = matcher.fuzzy_match(query_utf32.slice(..), search_str.slice(..)) {
                matches.push((score, name));
            }
        }

        // Sort by score (descending)
        matches.sort_unstable_by(|a, b| b.0.cmp(&a.0));

        // Map back to info
        matches.into_iter()
            .take(limit)
            .filter_map(|(_, name)| self.packages.get(name).cloned())
            .collect()
    }

    /// Get package info by name (instant)
    pub fn get(&self, name: &str) -> Option<PackageInfo> {
        let _read_guard = self.lock.read();
        self.packages.get(name).cloned()
    }

    /// Total number of indexed packages
    pub fn len(&self) -> usize {
        self.packages.len()
    }
}
