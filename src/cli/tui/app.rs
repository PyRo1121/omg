use crate::core::env::team::TeamStatus;
use crate::core::history::Transaction;
#[cfg(unix)]
use crate::daemon::protocol::StatusResult;
#[cfg(any(feature = "arch", feature = "debian", feature = "debian-pure"))]
use anyhow::Context;
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
    #[cfg(unix)]
    pub status: Option<StatusResult>,
    #[cfg(not(unix))]
    pub status: Option<()>,
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
    pub search_error: Option<String>,
    pub action_error: Option<String>,

    // System metrics
    pub system_metrics: SystemMetrics,

    // Last update time
    pub last_update: Instant,

    // Previous cumulative CPU sample (total, idle) used to compute an
    // instantaneous usage percentage instead of a since-boot average.
    pub prev_cpu_sample: Option<(u64, u64)>,

    // Usage stats
    pub usage_stats: crate::core::usage::UsageStats,

    /// True while a long-running action (update/install/clean/audit) is
    /// executing on a background task. Serializes actions so their results
    /// cannot interleave.
    pub action_in_flight: bool,

    /// Last time the search query was modified; used to debounce searches.
    pub last_query_change: Instant,
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
            search_error: None,
            action_error: None,
            system_metrics: SystemMetrics::default(),
            last_update: Instant::now(),
            prev_cpu_sample: None,
            usage_stats: crate::core::usage::UsageStats::load().unwrap_or_default(),
            action_in_flight: false,
            last_query_change: Instant::now(),
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
        #[cfg(unix)]
        {
            match crate::core::client::DaemonClient::connect().await {
                Ok(mut client) => {
                    self.daemon_connected = true;
                    if let Ok(crate::daemon::protocol::ResponseResult::Status(status)) = client
                        .call(crate::daemon::protocol::Request::Status { id: 0 })
                        .await
                    {
                        self.status = Some(status);
                    }
                }
                Err(_) => self.daemon_connected = false,
            }
        }
        #[cfg(not(unix))]
        {
            self.daemon_connected = false;
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
        if let Ok(cwd) = std::env::current_dir()
            && let Ok(workspace) = crate::core::env::team::TeamWorkspace::new(&cwd)
            && workspace.is_team_workspace()
            && let Ok(status) = workspace.load_status()
        {
            self.team_status = Some(status);
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
                        format_version: crate::core::env::team::TeamStatus::STATUS_FORMAT_VERSION,
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
                                last_sync: crate::cli::parse_timestamp_opt(&m.last_seen_at)
                                    .unwrap_or(0),
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
        // A single /proc/stat read yields a since-boot average, so CPU usage
        // must be derived from consecutive cumulative samples.
        let cpu_usage = Self::cpu_usage_delta(&mut self.prev_cpu_sample);
        let (disk_usage, disk_free) = Self::get_disk_usage_sync();
        let (network_rx, network_tx) = Self::get_network_stats();

        self.system_metrics = SystemMetrics {
            cpu_usage,
            memory_usage: Self::get_memory_usage(),
            disk_usage,
            disk_free,
            network_rx,
            network_tx,
        };
    }

    /// Read the cumulative `cpu` line of `/proc/stat` as `(total, idle)`.
    fn sample_cpu_totals() -> Option<(u64, u64)> {
        let stat = std::fs::read_to_string("/proc/stat").ok()?;
        let line = stat.lines().next()?;
        let mut fields = line.split_whitespace();
        if fields.next()? != "cpu" {
            return None;
        }

        let mut total = 0u64;
        let mut idle = None;
        for (index, field) in fields.enumerate() {
            let value = field.parse::<u64>().ok()?;
            total = total.checked_add(value)?;
            if index == 3 {
                idle = Some(value);
            }
        }
        Some((total, idle?))
    }

    /// Usage percent since the previous sample; the very first sample only
    /// establishes the baseline and reports the previous reading.
    fn cpu_usage_delta(previous: &mut Option<(u64, u64)>) -> f32 {
        let Some(sample) = Self::sample_cpu_totals() else {
            return 0.0;
        };
        let usage = match *previous {
            Some((prev_total, prev_idle)) => {
                let d_total = sample.0.saturating_sub(prev_total);
                let d_idle = sample.1.saturating_sub(prev_idle);
                if d_total > 0 {
                    (d_total - d_idle) as f32 / d_total as f32 * 100.0
                } else {
                    0.0
                }
            }
            None => 0.0,
        };
        *previous = Some(sample);
        usage
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
                return (total.saturating_sub(available) as f32 / total as f32) * 100.0;
            }
        }
        0.0
    }

    fn get_disk_usage_sync() -> (u64, u64) {
        #[cfg(unix)]
        {
            // Use rustix for safe statvfs
            if let Ok(stat) = rustix::fs::statvfs("/") {
                let block_size = stat.f_frsize;
                let total_blocks = stat.f_blocks;
                let free_blocks = stat.f_bfree;
                let used = total_blocks.saturating_sub(free_blocks) * block_size / 1024; // KB
                let free = free_blocks * block_size / 1024; // KB
                return (used, free);
            }
        }
        (0, 0)
    }

    fn get_network_stats() -> (u64, u64) {
        // Read /proc/net/dev for network stats
        if let Ok(netdev) = std::fs::read_to_string("/proc/net/dev") {
            let mut total_rx = 0u64;
            let mut total_tx = 0u64;

            for line in netdev.lines().skip(2) {
                let mut fields = line.split_whitespace();
                let Some(interface) = fields.next() else {
                    continue;
                };
                if interface.starts_with("lo") {
                    continue;
                }
                let (Some(rx), Some(tx)) = (fields.next(), fields.nth(7)) else {
                    continue;
                };
                if let (Ok(rx), Ok(tx)) = (rx.parse::<u64>(), tx.parse::<u64>()) {
                    total_rx = total_rx.saturating_add(rx);
                    total_tx = total_tx.saturating_add(tx);
                }
            }

            return (total_rx, total_tx);
        }
        (0, 0)
    }

    pub async fn search_packages(&mut self, query: &str) -> Result<()> {
        if query.is_empty() {
            self.search_results.clear();
            self.search_error = None;
            return Ok(());
        }

        // Search packages using the actual package manager
        #[cfg(unix)]
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
            self.search_error = None;
            return Ok(());
        }

        // Fallback to direct search if daemon is not available
        #[cfg(any(feature = "debian", feature = "debian-pure"))]
        if crate::core::env::distro::is_debian_like() {
            self.search_results = crate::package_managers::debian_db::search_fast(query)
                .context("Failed to search official packages")?
                .into_iter()
                .map(|pkg| crate::package_managers::SyncPackage {
                    name: pkg.name,
                    version: pkg.version,
                    description: pkg.description,
                    repo: "official".to_string(),
                    download_size: 0,
                    installed: pkg.installed,
                })
                .collect();
            self.search_error = None;
            return Ok(());
        }

        #[cfg(feature = "arch")]
        {
            self.search_results = crate::package_managers::search_sync(query)
                .context("Failed to search official packages")?;
        }
        #[cfg(all(feature = "debian", not(feature = "arch")))]
        {
            self.search_results = crate::package_managers::apt_search_sync(query)
                .context("Failed to search official packages")?;
        }
        #[cfg(all(
            feature = "debian-pure",
            not(feature = "arch"),
            not(feature = "debian")
        ))]
        {
            self.search_results = crate::package_managers::apt_search_fast(query)
                .context("Failed to search official packages")?
                .into_iter()
                .map(|pkg| crate::package_managers::SyncPackage {
                    name: pkg.name,
                    version: pkg.version,
                    description: pkg.description,
                    repo: "official".to_string(),
                    download_size: 0,
                    installed: pkg.installed,
                })
                .collect();
        }
        #[cfg(not(any(feature = "arch", feature = "debian", feature = "debian-pure")))]
        {
            anyhow::bail!("Failed to search official packages: no package manager backend enabled");
        }

        #[cfg(any(feature = "arch", feature = "debian", feature = "debian-pure"))]
        {
            self.search_error = None;
            Ok(())
        }
    }

    // Long-running actions are associated functions (they never read model
    // state) so the event loop can spawn them without borrowing the model.
    pub async fn install_package(package_name: &str) -> Result<()> {
        let packages = vec![package_name.to_string()];
        crate::cli::packages::install(&packages, false, false, false).await
    }

    pub async fn update_system() -> Result<()> {
        crate::cli::packages::update(false, false, false).await
    }

    pub async fn clean_cache() -> Result<()> {
        crate::cli::packages::clean(true, true, true, false, false).await
    }

    #[allow(
        clippy::unused_async,
        reason = "feature-gated implementations await while fallback builds do not"
    )]
    pub async fn remove_orphans() -> Result<()> {
        #[cfg(any(feature = "debian", feature = "debian-pure"))]
        if crate::core::env::distro::is_debian_like() {
            #[cfg(feature = "debian-pure")]
            {
                let orphan_list = crate::package_managers::debian_db::list_orphans_fast()
                    .context("Failed to list orphan packages")?;
                if orphan_list.is_empty() {
                    return Ok(());
                }
                let pm = crate::package_managers::get_package_manager()?;
                return pm.remove(&orphan_list).await;
            }
            #[cfg(all(feature = "debian", not(feature = "debian-pure")))]
            {
                return crate::package_managers::apt_remove_orphans();
            }
        }

        #[cfg(feature = "arch")]
        {
            crate::package_managers::remove_orphans().await
        }
        #[cfg(all(feature = "debian", not(feature = "arch")))]
        {
            crate::package_managers::apt_remove_orphans()
        }
        #[cfg(all(
            feature = "debian-pure",
            not(feature = "arch"),
            not(feature = "debian")
        ))]
        {
            let orphan_list = crate::package_managers::debian_db::list_orphans_fast()
                .context("Failed to list orphan packages")?;
            if orphan_list.is_empty() {
                return Ok(());
            }
            let pm = crate::package_managers::get_package_manager()?;
            pm.remove(&orphan_list).await
        }
        #[cfg(not(any(feature = "arch", feature = "debian", feature = "debian-pure")))]
        {
            anyhow::bail!("Cannot remove orphans: no package manager backend enabled");
        }
    }

    pub async fn run_security_audit() -> Result<usize> {
        let scanner = crate::core::security::vulnerability::VulnerabilityScanner::new();
        Ok(scanner.scan_system().await?)
    }

    /// Whether pressing Enter should open the install-confirmation popup.
    /// Pure decision helper; the event loop performs the actual install only
    /// after the popup is confirmed with a second Enter.
    pub fn enter_requests_confirmation(&self) -> bool {
        !self.search_mode
            && self.current_tab == Tab::Packages
            && !self.search_results.is_empty()
            && !self.show_popup
    }

    /// Switch tabs, resetting transient list/search state so a stale
    /// selection or armed search mode cannot leak across tabs.
    pub fn switch_tab(&mut self, tab: Tab) {
        self.current_tab = tab;
        self.selected_index = 0;
        self.search_mode = false;
        self.show_popup = false;
    }

    /// Record a search-query mutation for debounce purposes.
    pub fn note_query_change(&mut self) {
        self.last_query_change = Instant::now();
    }

    pub async fn tick(&mut self) -> Result<()> {
        if self.last_tick.elapsed() >= std::time::Duration::from_secs(5) {
            self.refresh().await?;
            self.last_tick = Instant::now();
        }

        // Update metrics more frequently
        if self.last_update.elapsed() >= std::time::Duration::from_secs(1) {
            self.update_system_metrics();
            self.last_update = Instant::now();
        }

        Ok(())
    }

    pub fn handle_key(&mut self, key: KeyCode) {
        // While typing a query, every key belongs to the search input.
        // Navigation, refresh, and tab shortcuts must not fire mid-typing.
        if self.search_mode {
            match key {
                KeyCode::Esc => {
                    // Cancel: discard the query so the main loop cannot
                    // mistake a cancelled search for a committed one.
                    self.search_mode = false;
                    self.search_query.clear();
                    self.search_results.clear();
                    self.note_query_change();
                }
                KeyCode::Enter => {
                    self.search_mode = false;
                    // Search will be triggered in the main loop
                }
                KeyCode::Backspace => {
                    if !self.search_query.is_empty() {
                        self.search_query.pop();
                        self.note_query_change();
                    }
                }
                KeyCode::Char(c) => {
                    self.search_query.push(c);
                    self.note_query_change();
                }
                _ => {}
            }
            return;
        }

        match key {
            // Navigation. Quit is handled by the main loop (which restores the
            // terminal); an abrupt `process::exit` here would skip cleanup.
            KeyCode::Char('r') => {
                // Trigger refresh - force it by setting last_tick to a past time
                self.last_tick = Instant::now()
                    .checked_sub(std::time::Duration::from_secs(10))
                    .unwrap_or_else(Instant::now);
            }

            // Tab switching
            KeyCode::Char('1') => self.switch_tab(Tab::Dashboard),
            KeyCode::Char('2') => self.switch_tab(Tab::Packages),
            KeyCode::Char('3') => self.switch_tab(Tab::Runtimes),
            KeyCode::Char('4') => self.switch_tab(Tab::Security),
            KeyCode::Char('5') => self.switch_tab(Tab::Activity),
            KeyCode::Char('6') => self.switch_tab(Tab::Team),

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
                    self.search_error = None;
                    self.selected_index = 0;
                    self.note_query_change();
                }
            }
            KeyCode::Esc => {
                self.show_popup = false;
            }
            KeyCode::Enter => {
                if self.enter_requests_confirmation() {
                    // Ask for confirmation first; the install runs only after
                    // a second Enter confirms the popup (handled by the loop).
                    self.show_popup = true;
                }
            }

            // Tab switching with arrow keys
            KeyCode::Tab => {
                let next = match self.current_tab {
                    Tab::Dashboard => Tab::Packages,
                    Tab::Packages => Tab::Runtimes,
                    Tab::Runtimes => Tab::Security,
                    Tab::Security => Tab::Activity,
                    Tab::Activity => Tab::Team,
                    Tab::Team => Tab::Dashboard,
                };
                self.switch_tab(next);
            }
            KeyCode::BackTab => {
                let prev = match self.current_tab {
                    Tab::Dashboard => Tab::Team,
                    Tab::Team => Tab::Activity,
                    Tab::Activity => Tab::Security,
                    Tab::Security => Tab::Runtimes,
                    Tab::Runtimes => Tab::Packages,
                    Tab::Packages => Tab::Dashboard,
                };
                self.switch_tab(prev);
            }

            _ => {}
        }
    }

    pub fn get_total_packages(&self) -> usize {
        #[cfg(unix)]
        return self.status.as_ref().map_or(0, |s| s.total_packages);
        #[cfg(not(unix))]
        0
    }

    pub fn get_orphan_packages(&self) -> usize {
        #[cfg(unix)]
        return self.status.as_ref().map_or(0, |s| s.orphan_packages);
        #[cfg(not(unix))]
        0
    }

    pub fn get_updates_available(&self) -> usize {
        #[cfg(unix)]
        return self.status.as_ref().map_or(0, |s| s.updates_available);
        #[cfg(not(unix))]
        0
    }

    pub fn get_security_vulnerabilities(&self) -> Option<usize> {
        #[cfg(unix)]
        return self
            .status
            .as_ref()
            .and_then(crate::daemon::protocol::StatusResult::scanned_vulnerability_count);
        #[cfg(not(unix))]
        None
    }

    pub fn get_runtime_versions(&self) -> &[(String, String)] {
        #[cfg(unix)]
        return self
            .status
            .as_ref()
            .map_or(&[], |status| status.runtime_versions.as_slice());
        #[cfg(not(unix))]
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app() -> App {
        App {
            status: None,
            team_status: None,
            history: Vec::new(),
            last_tick: Instant::now(),
            current_tab: Tab::Packages,
            selected_index: 0,
            show_popup: false,
            search_query: String::new(),
            search_mode: false,
            daemon_connected: false,
            search_results: vec![crate::package_managers::SyncPackage {
                name: "firefox".to_string(),
                version: crate::package_managers::parse_version_or_zero("1.0"),
                description: "Browser".to_string(),
                repo: "official".to_string(),
                download_size: 0,
                installed: false,
            }],
            search_error: None,
            action_error: None,
            system_metrics: SystemMetrics::default(),
            last_update: Instant::now(),
            prev_cpu_sample: None,
            usage_stats: crate::core::usage::UsageStats::default(),
            action_in_flight: false,
            last_query_change: Instant::now(),
        }
    }

    #[test]
    fn enter_with_results_opens_confirmation_popup() {
        let mut app = test_app();
        app.handle_key(KeyCode::Enter);
        assert!(app.show_popup, "first Enter must open the confirm popup");
        assert!(
            !app.enter_requests_confirmation(),
            "popup must suppress a second confirmation request"
        );
    }

    #[test]
    fn esc_cancels_popup_without_installing() {
        let mut app = test_app();
        app.show_popup = true;
        app.handle_key(KeyCode::Esc);
        assert!(!app.show_popup);
    }

    #[test]
    fn enter_during_search_ends_search_and_does_not_open_popup() {
        let mut app = test_app();
        app.search_mode = true;
        app.search_query.push_str("fire");
        app.handle_key(KeyCode::Enter);
        assert!(!app.search_mode);
        assert!(
            !app.show_popup,
            "committing a query must never install the stale selection"
        );
    }

    #[test]
    fn search_mode_treats_reserved_characters_as_query_text() {
        let mut app = test_app();
        app.search_mode = true;

        // Every one of these keys is a global shortcut outside search mode;
        // while typing they must be inserted into the query instead.
        for c in ['r', 'k', 'j', '5', '/'] {
            app.handle_key(KeyCode::Char(c));
        }

        assert_eq!(app.search_query, "rkj5/");
        assert!(app.search_mode);
        assert_eq!(app.current_tab, Tab::Packages);
    }

    #[test]
    fn esc_cancels_search_by_discarding_the_query() {
        let mut app = test_app();
        app.search_mode = true;
        app.search_query.push_str("fir");

        app.handle_key(KeyCode::Esc);

        assert!(!app.search_mode);
        assert!(
            app.search_query.is_empty(),
            "a cancelled query must not commit"
        );
        assert!(app.search_results.is_empty());
    }

    #[test]
    fn tab_switch_resets_selection_and_search_state() {
        let mut app = test_app();
        app.search_mode = true;
        app.selected_index = 10;
        // Exit search first: while the query is open even digits belong to it.
        app.handle_key(KeyCode::Enter);
        app.handle_key(KeyCode::Char('5')); // Activity
        assert_eq!(app.current_tab, Tab::Activity);
        assert_eq!(app.selected_index, 0);
        assert!(!app.search_mode, "search mode must not leak across tabs");

        // Returning to Packages starts clean.
        app.handle_key(KeyCode::Tab);
        assert_eq!(app.current_tab, Tab::Team);
    }
}
