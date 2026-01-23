use crate::core::env::team::TeamStatus;
use crate::core::history::Transaction;
use crate::daemon::protocol::StatusResult;
use anyhow::Result;
use crossterm::event::KeyCode;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Dashboard = 0,
    Packages,
    Runtimes,
    Security,
    Activity,
    Team,
}

pub struct App {
    pub status: Option<StatusResult>,
    pub team_status: Option<TeamStatus>,
    pub history: Vec<Transaction>,
    pub last_tick: Instant,
    pub current_tab: Tab,
    pub selected_index: usize,
    pub show_popup: bool,
    pub search_query: String,
    pub search_mode: bool,
    pub daemon_connected: bool,

    // Search results
    pub search_results: Vec<crate::package_managers::SyncPackage>,

    // System metrics
    pub system_metrics: SystemMetrics,

    // Last update time
    pub last_update: Instant,

    // Usage stats
    pub usage_stats: crate::core::usage::UsageStats,
}

#[derive(Debug, Clone, Default)]
pub struct SystemMetrics {
    pub cpu_usage: f32,
    pub memory_usage: f32,
    pub disk_usage: u64,
    pub disk_free: u64,
    pub network_rx: u64,
    pub network_tx: u64,
}

impl App {
    pub async fn new() -> Result<Self> {
        let mut app = Self {
            status: None,
            team_status: None,
            history: Vec::new(),
            last_tick: Instant::now(),
            current_tab: Tab::Dashboard,
            selected_index: 0,
            show_popup: false,
            search_query: String::new(),
            search_mode: false,
            daemon_connected: false,
            search_results: Vec::new(),
            system_metrics: SystemMetrics::default(),
            last_update: Instant::now(),
            usage_stats: crate::core::usage::UsageStats::load(),
        };
        app.refresh().await?;
        Ok(app)
    }

    #[must_use]
    pub fn with_tab(mut self, tab: Tab) -> Self {
        self.current_tab = tab;
        self
    }

    pub async fn refresh(&mut self) -> Result<()> {
        // Check if daemon is connected
        self.daemon_connected = crate::core::client::DaemonClient::connect().await.is_ok();

        // 1. Fetch status from daemon
        if let Ok(mut client) = crate::core::client::DaemonClient::connect().await
            && let Ok(crate::daemon::protocol::ResponseResult::Status(status)) = client
                .call(crate::daemon::protocol::Request::Status { id: 0 })
                .await
        {
            self.status = Some(status);
        }

        // 2. Fetch history
        if let Ok(history_mgr) = crate::core::history::HistoryManager::new()
            && let Ok(entries) = history_mgr.load()
        {
            self.history = entries.into_iter().rev().take(50).collect();
        }

        // 3. Update system metrics
        self.update_system_metrics();

        // 4. Fetch team status if in a team workspace
        self.fetch_team_status().await;

        Ok(())
    }

    async fn fetch_team_status(&mut self) {
        // 1. Try to load local team workspace status
        if let Ok(cwd) = std::env::current_dir() {
            let workspace = crate::core::env::team::TeamWorkspace::new(&cwd);
            if workspace.is_team_workspace()
                && let Ok(status) = workspace.load_status()
            {
                self.team_status = Some(status);
            }
        }

        // 2. If we have a Team+ license, try to fetch real-time member data from the API
        if let Some(license) = crate::core::license::load_license() {
            let tier = license.tier_enum();
            if matches!(
                tier,
                crate::core::license::Tier::Team | crate::core::license::Tier::Enterprise
            ) && let Ok(members) = crate::core::license::fetch_team_members().await
            {
                // If we don't have a local team workspace, create a synthetic one from API data
                if self.team_status.is_none() {
                    self.team_status = Some(crate::core::env::team::TeamStatus {
                        config: crate::core::env::team::TeamConfig {
                            team_id: "fleet".to_string(),
                            name: format!(
                                "{} Fleet",
                                license.customer.as_deref().unwrap_or("Your")
                            ),
                            ..Default::default()
                        },
                        lock_hash: String::new(),
                        members: members
                            .into_iter()
                            .map(|m| crate::core::env::team::TeamMember {
                                id: m.machine_id,
                                name: m.hostname.unwrap_or_else(|| "Unknown".to_string()),
                                env_hash: String::new(),
                                last_sync: parse_timestamp(&m.last_seen_at),
                                in_sync: m.is_active,
                                drift_summary: None,
                            })
                            .collect(),
                        updated_at: jiff::Timestamp::now().as_second(),
                    });
                }
            }
        }
    }

    fn update_system_metrics(&mut self) {
        // Get actual system metrics
        self.system_metrics = {
            // CPU usage
            let cpu_usage = Self::get_cpu_usage();

            // Memory usage
            let memory_usage = Self::get_memory_usage();

            // Disk usage - use sync version
            let (disk_used, disk_free) = Self::get_disk_usage_sync();

            // Network stats
            let (network_rx, network_tx) = Self::get_network_stats();

            SystemMetrics {
                cpu_usage,
                memory_usage,
                disk_usage: disk_used,
                disk_free,
                network_rx,
                network_tx,
            }
        };
    }

    fn get_cpu_usage() -> f32 {
        // Read /proc/stat for CPU usage
        if let Ok(stat) = std::fs::read_to_string("/proc/stat")
            && let Some(line) = stat.lines().next()
        {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() > 4 && parts.first() == Some(&"cpu") {
                let user: u64 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                let nice: u64 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
                let system: u64 = parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
                let idle: u64 = parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);

                let total = user + nice + system + idle;
                if total > 0 {
                    return ((total - idle) as f32 / total as f32) * 100.0;
                }
            }
        }
        0.0
    }

    fn get_memory_usage() -> f32 {
        // Read /proc/meminfo for memory usage
        if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
            let mut total = 0u64;
            let mut available = 0u64;

            for line in meminfo.lines() {
                if line.starts_with("MemTotal:") {
                    if let Some(kb) = line.split_whitespace().nth(1) {
                        total = kb.parse().unwrap_or(0);
                    }
                } else if line.starts_with("MemAvailable:")
                    && let Some(kb) = line.split_whitespace().nth(1)
                {
                    available = kb.parse().unwrap_or(0);
                }
            }

            if total > 0 {
                return ((total - available) as f32 / total as f32) * 100.0;
            }
        }
        0.0
    }

    fn get_disk_usage_sync() -> (u64, u64) {
        // Use rustix for safe statvfs
        if let Ok(stat) = rustix::fs::statvfs("/") {
            let block_size = stat.f_frsize;
            let total_blocks = stat.f_blocks;
            let free_blocks = stat.f_bfree;
            let used = (total_blocks - free_blocks) * block_size / 1024; // KB
            let free = free_blocks * block_size / 1024; // KB
            return (used, free);
        }
        (0, 0)
    }

    fn get_network_stats() -> (u64, u64) {
        // Read /proc/net/dev for network stats
        if let Ok(netdev) = std::fs::read_to_string("/proc/net/dev") {
            let mut total_rx = 0u64;
            let mut total_tx = 0u64;

            for line in netdev.lines().skip(2) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() > 9
                    && parts.first().is_some_and(|s| !s.starts_with("lo"))
                    && let (Some(rx_str), Some(tx_str)) = (parts.get(1), parts.get(9))
                    && let (Ok(rx), Ok(tx)) = (rx_str.parse::<u64>(), tx_str.parse::<u64>())
                {
                    total_rx += rx;
                    total_tx += tx;
                }
            }

            return (total_rx, total_tx);
        }
        (0, 0)
    }

    pub async fn search_packages(&mut self, query: &str) -> Result<()> {
        if query.is_empty() {
            self.search_results.clear();
            return Ok(());
        }

        // Search packages using the actual package manager
        if let Ok(mut client) = crate::core::client::DaemonClient::connect().await
            && let Ok(crate::daemon::protocol::ResponseResult::Search(res)) = client
                .call(crate::daemon::protocol::Request::Search {
                    id: 0,
                    query: query.to_string(),
                    limit: Some(50),
                })
                .await
        {
            self.search_results = res
                .packages
                .into_iter()
                .map(|p| crate::package_managers::SyncPackage {
                    name: p.name,
                    version: crate::package_managers::parse_version_or_zero(&p.version),
                    description: p.description,
                    repo: "official".to_string(),
                    download_size: 0,
                    installed: false,
                })
                .collect();
            return Ok(());
        }

        // Fallback to direct search if daemon is not available
        #[cfg(feature = "arch")]
        {
            self.search_results = crate::package_managers::search_sync(query).unwrap_or_default();
        }
        #[cfg(feature = "debian")]
        {
            self.search_results =
                crate::package_managers::apt_search_sync(query).unwrap_or_default();
        }

        Ok(())
    }

    pub async fn install_package(&self, package_name: &str) -> Result<()> {
        let packages = vec![package_name.to_string()];
        crate::cli::packages::install(&packages, false).await
    }

    pub async fn update_system(&self) -> Result<()> {
        crate::cli::packages::update(false, false).await
    }

    pub async fn clean_cache(&self) -> Result<()> {
        crate::cli::packages::clean(true, true, true, false).await
    }

    #[allow(clippy::unused_async)]
    pub async fn remove_orphans(&self) -> Result<()> {
        // Use the actual orphan removal
        #[cfg(feature = "arch")]
        {
            crate::package_managers::remove_orphans().await
        }
        #[cfg(all(feature = "debian", not(feature = "arch")))]
        {
            crate::package_managers::apt_remove_orphans()
        }
        #[cfg(not(any(feature = "arch", feature = "debian")))]
        Ok(())
    }

    pub async fn run_security_audit(&self) -> Result<usize> {
        let scanner = crate::core::security::vulnerability::VulnerabilityScanner::new();
        scanner.scan_system().await
    }

    pub async fn tick(&mut self) -> Result<()> {
        if self.last_tick.elapsed() >= std::time::Duration::from_secs(5) {
            self.refresh().await?;
            self.last_tick = Instant::now();
        }

        // Update metrics more frequently
        if self.last_update.elapsed() >= std::time::Duration::from_millis(1000) {
            self.update_system_metrics();
            self.last_update = Instant::now();
        }

        Ok(())
    }

    pub fn handle_key(&mut self, key: KeyCode) {
        match key {
            // Navigation
            KeyCode::Char('q') => std::process::exit(0),
            KeyCode::Char('r') => {
                // Trigger refresh - force it by setting last_tick to a past time
                self.last_tick = Instant::now()
                    .checked_sub(std::time::Duration::from_secs(10))
                    .unwrap_or_else(Instant::now);
            }

            // Tab switching
            KeyCode::Char('1') => self.current_tab = Tab::Dashboard,
            KeyCode::Char('2') => self.current_tab = Tab::Packages,
            KeyCode::Char('3') => self.current_tab = Tab::Runtimes,
            KeyCode::Char('4') => self.current_tab = Tab::Security,
            KeyCode::Char('5') => self.current_tab = Tab::Activity,
            KeyCode::Char('6') => self.current_tab = Tab::Team,

            // List navigation
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = match self.current_tab {
                    Tab::Packages => self.search_results.len().saturating_sub(1),
                    Tab::Activity => self.history.len().saturating_sub(1),
                    _ => 0,
                };
                if self.selected_index < max {
                    self.selected_index += 1;
                }
            }

            // Search
            KeyCode::Char('/') => {
                if self.current_tab == Tab::Packages {
                    self.search_mode = true;
                    self.search_query.clear();
                    self.search_results.clear();
                }
            }
            KeyCode::Esc => {
                self.search_mode = false;
                self.show_popup = false;
            }
            KeyCode::Backspace => {
                if self.search_mode && !self.search_query.is_empty() {
                    self.search_query.pop();
                }
            }
            KeyCode::Enter => {
                if self.search_mode {
                    self.search_mode = false;
                    // Search will be triggered in the main loop
                } else if self.current_tab == Tab::Packages && !self.search_results.is_empty() {
                    // Install selected package
                    self.show_popup = true;
                }
            }

            // Tab switching with arrow keys
            KeyCode::Tab => {
                self.current_tab = match self.current_tab {
                    Tab::Dashboard => Tab::Packages,
                    Tab::Packages => Tab::Runtimes,
                    Tab::Runtimes => Tab::Security,
                    Tab::Security => Tab::Activity,
                    Tab::Activity => Tab::Team,
                    Tab::Team => Tab::Dashboard,
                };
            }
            KeyCode::BackTab => {
                self.current_tab = match self.current_tab {
                    Tab::Dashboard => Tab::Team,
                    Tab::Team => Tab::Activity,
                    Tab::Activity => Tab::Security,
                    Tab::Security => Tab::Runtimes,
                    Tab::Runtimes => Tab::Packages,
                    Tab::Packages => Tab::Dashboard,
                };
            }

            // Character input for search
            KeyCode::Char(c) => {
                if self.search_mode {
                    self.search_query.push(c);
                }
            }

            _ => {}
        }
    }

    pub fn get_total_packages(&self) -> usize {
        self.status.as_ref().map_or(0, |s| s.total_packages)
    }

    pub fn get_orphan_packages(&self) -> usize {
        self.status.as_ref().map_or(0, |s| s.orphan_packages)
    }

    pub fn get_updates_available(&self) -> usize {
        self.status.as_ref().map_or(0, |s| s.updates_available)
    }

    pub fn get_security_vulnerabilities(&self) -> usize {
        self.status
            .as_ref()
            .map_or(0, |s| s.security_vulnerabilities)
    }

    pub fn get_runtime_versions(&self) -> std::collections::HashMap<String, String> {
        self.status
            .as_ref()
            .map(|s| s.runtime_versions.iter().cloned().collect())
            .unwrap_or_default()
    }
}

fn parse_timestamp(s: &str) -> i64 {
    use std::str::FromStr;
    if let Ok(ts) = jiff::Timestamp::from_str(s) {
        ts.as_second()
    } else {
        0
    }
}
