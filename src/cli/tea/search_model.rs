//! Search Model - Elm Architecture implementation for search command
//!
//! Modern, stylish package search interface with Bubble Tea-inspired UX.

use crate::cli::style;
use crate::cli::tea::{Cmd, Model};
use crate::core::Package;
use crate::package_managers::SyncPackage;
use std::fmt::Write;

#[cfg(feature = "arch")]
use crate::package_managers::AurPackageDetail;

/// Search state machine
#[derive(Debug, Clone, PartialEq)]
pub enum SearchState {
    Idle,
    Searching,
    ShowingResults,
    NoResults,
    Failed,
}

/// Package source for styling
#[derive(Debug, Clone, PartialEq)]
pub enum PackageSource {
    Official,
    Aur,
}

impl PackageSource {
    /// Get styled label for this source
    pub fn styled_label(&self) -> String {
        match self {
            Self::Official => "official".cyan().to_string(),
            Self::Aur => "aur".yellow().to_string(),
        }
    }
}

use owo_colors::OwoColorize;

/// Search result package
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub name: String,
    pub version: String,
    pub description: String,
    pub source: PackageSource,
    pub repo: String,
    pub installed: bool,
    #[cfg(feature = "arch")]
    pub votes: Option<usize>,
    #[cfg(feature = "arch")]
    pub popularity: Option<f64>,
    #[cfg(feature = "arch")]
    pub out_of_date: bool,
}

/// Search messages
#[derive(Debug, Clone)]
pub enum SearchMsg {
    Search(String),
    ResultsFound(Vec<SearchResult>),
    NoResults,
    Error(String),
}

/// Search model state
#[derive(Debug, Clone)]
pub struct SearchModel {
    pub state: SearchState,
    pub query: String,
    pub results: Vec<SearchResult>,
    pub official_count: usize,
    pub aur_count: usize,
    pub error: Option<String>,
    pub search_time_ms: f64,
}

impl Default for SearchModel {
    fn default() -> Self {
        Self {
            state: SearchState::Idle,
            query: String::new(),
            results: Vec::new(),
            official_count: 0,
            aur_count: 0,
            error: None,
            search_time_ms: 0.0,
        }
    }
}

impl SearchModel {
    /// Create new search model
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set search query
    #[must_use]
    pub fn with_query(mut self, query: String) -> Self {
        self.query = query;
        self
    }

    /// Render a single search result with beautiful styling
    fn render_result(result: &SearchResult, index: usize) -> String {
        let installed_mark = if result.installed {
            style::dim(" [installed]")
        } else {
            String::new()
        };

        #[cfg(feature = "arch")]
        let aur_info = if result.source == PackageSource::Aur {
            let votes = result.votes.unwrap_or(0);
            let pop = result.popularity.unwrap_or(0.0);
            let out_of_date = if result.out_of_date {
                " ".to_string() + style::error("[OUT OF DATE]").as_str()
            } else {
                String::new()
            };
            format!(
                " {} {}{}",
                style::info(&format!("↑{votes}")),
                style::info(&format!("{pop:.1}%")),
                out_of_date
            )
        } else {
            String::new()
        };

        #[cfg(not(feature = "arch"))]
        let aur_info = String::new();

        format!(
            "  {:>2}) {} {} ({}){} - {}{}{}",
            style::dim(&(index + 1).to_string()),
            style::package(&result.name),
            style::version(&result.version),
            result.source.styled_label(),
            installed_mark,
            style::dim(&truncate(&result.description, 50)),
            aur_info,
            if result.repo == "official" {
                String::new()
            } else {
                format!(" {}", style::dim(&format!("[{}]", result.repo)))
            }
        )
    }

    /// Render header with beautiful box drawing
    fn render_header(title: &str, subtitle: &str) -> String {
        format!(
            "\n{} {}\n{} {}\n{}{}\n",
            "┌─".cyan().bold(),
            title.cyan().bold(),
            "│".cyan().bold(),
            subtitle.white(),
            "└".cyan().bold(),
            "─".repeat(subtitle.len()).cyan().bold()
        )
    }
}

impl Model for SearchModel {
    type Msg = SearchMsg;

    fn init(&self) -> Cmd<Self::Msg> {
        Cmd::Exec(Box::new(|| {
            // Return immediately - search is triggered by user
            SearchMsg::Search(String::new())
        }))
    }

    fn update(&mut self, msg: Self::Msg) -> Cmd<Self::Msg> {
        match msg {
            SearchMsg::Search(query) => {
                self.query.clone_from(&query);
                self.state = SearchState::Searching;

                if query.is_empty() {
                    self.state = SearchState::Idle;
                    return Cmd::Info("Enter a search query".to_string());
                }

                // In production, this would trigger actual search
                // For now, return no-op
                Cmd::none()
            }
            SearchMsg::ResultsFound(results) => {
                self.official_count = results
                    .iter()
                    .filter(|r| r.source == PackageSource::Official)
                    .count();
                self.aur_count = results
                    .iter()
                    .filter(|r| r.source == PackageSource::Aur)
                    .count();
                self.results = results;
                self.state = if self.results.is_empty() {
                    SearchState::NoResults
                } else {
                    SearchState::ShowingResults
                };

                if self.state == SearchState::NoResults {
                    Cmd::batch([
                        Cmd::PrintLn(String::new()),
                        Cmd::Warning(format!("No packages found for '{}'", self.query)),
                        Cmd::PrintLn(String::new()),
                    ])
                } else {
                    Cmd::PrintLn(String::new())
                }
            }
            SearchMsg::NoResults => {
                self.state = SearchState::NoResults;
                Cmd::batch([
                    Cmd::PrintLn(String::new()),
                    Cmd::Warning(format!("No packages found for '{}'", self.query)),
                    Cmd::PrintLn(String::new()),
                ])
            }
            SearchMsg::Error(err) => {
                self.state = SearchState::Failed;
                self.error = Some(err.clone());
                Cmd::Error(format!("Search failed: {err}"))
            }
        }
    }

    fn view(&self) -> String {
        match self.state {
            SearchState::Idle => String::new(),
            SearchState::Searching => "⟳ Searching...".cyan().dimmed().to_string(),
            SearchState::ShowingResults => {
                let mut output = String::new();

                // Beautiful header
                output.push_str(&Self::render_header(
                    "OMG",
                    &format!(
                        "{} results ({:.1}ms)",
                        self.results.len(),
                        self.search_time_ms
                    ),
                ));

                // Group results by source
                let official: Vec<_> = self
                    .results
                    .iter()
                    .filter(|r| r.source == PackageSource::Official)
                    .collect();

                let aur: Vec<_> = self
                    .results
                    .iter()
                    .filter(|r| r.source == PackageSource::Aur)
                    .collect();

                // Display official packages
                if !official.is_empty() {
                    let _ = writeln!(output, "{}", "Official Repositories".cyan().bold());
                    for (i, result) in official.iter().enumerate() {
                        output.push_str(&Self::render_result(result, i));
                        output.push('\n');
                    }
                    if !aur.is_empty() {
                        output.push('\n');
                    }
                }

                // Display AUR packages
                if !aur.is_empty() {
                    let _ = writeln!(output, "{}", "AUR (Arch User Repository)".cyan().bold());
                    for (i, result) in aur.iter().enumerate() {
                        output.push_str(&Self::render_result(result, i));
                        output.push('\n');
                    }
                }

                // Footer tip
                let _ = write!(
                    output,
                    "\n{} {}\n",
                    style::arrow("→"),
                    style::command("omg info <package> for details")
                );

                output
            }
            SearchState::NoResults => {
                format!(
                    "\n✓ {}\n",
                    format!("No packages found for '{}'", style::package(&self.query))
                        .green()
                        .bold()
                )
            }
            SearchState::Failed => {
                if let Some(err) = &self.error {
                    format!("\n✗ Search failed: {}\n", err.red())
                } else {
                    "\n✗ Search failed\n".to_string()
                }
            }
        }
    }
}

/// Helper function to truncate text
fn truncate(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}...", &text[..max_len])
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONVERSIONS
// ═══════════════════════════════════════════════════════════════════════════════

#[allow(clippy::implicit_clone)] // Version type varies by feature flag
impl From<SyncPackage> for SearchResult {
    fn from(pkg: SyncPackage) -> Self {
        Self {
            name: pkg.name,
            version: pkg.version.to_string(),
            description: pkg.description,
            source: PackageSource::Official,
            repo: pkg.repo,
            installed: pkg.installed,
            #[cfg(feature = "arch")]
            votes: None,
            #[cfg(feature = "arch")]
            popularity: None,
            #[cfg(feature = "arch")]
            out_of_date: false,
        }
    }
}

#[allow(clippy::implicit_clone)] // Version type varies by feature flag
impl From<Package> for SearchResult {
    fn from(pkg: Package) -> Self {
        let source = match pkg.source {
            crate::core::PackageSource::Official => PackageSource::Official,
            crate::core::PackageSource::Aur => PackageSource::Aur,
        };

        Self {
            name: pkg.name,
            version: pkg.version.to_string(),
            description: pkg.description,
            source,
            repo: pkg.source.to_string().to_lowercase(),
            installed: pkg.installed,
            #[cfg(feature = "arch")]
            votes: None,
            #[cfg(feature = "arch")]
            popularity: None,
            #[cfg(feature = "arch")]
            out_of_date: false,
        }
    }
}

#[cfg(feature = "arch")]
impl From<AurPackageDetail> for SearchResult {
    fn from(pkg: AurPackageDetail) -> Self {
        Self {
            name: pkg.name,
            version: pkg.version,
            description: pkg.description.unwrap_or_default(),
            source: PackageSource::Aur,
            repo: "aur".to_string(),
            installed: false,
            votes: Some(pkg.num_votes as usize),
            popularity: Some(pkg.popularity),
            out_of_date: pkg.out_of_date.is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_model_initial_state() {
        let model = SearchModel::new();
        assert_eq!(model.state, SearchState::Idle);
        assert!(model.results.is_empty());
    }

    #[test]
    fn test_search_model_with_query() {
        let model = SearchModel::new().with_query("firefox".to_string());
        assert_eq!(model.query, "firefox");
    }

    #[test]
    fn test_package_source_labels() {
        assert!(PackageSource::Official.styled_label().contains("official"));
        assert!(PackageSource::Aur.styled_label().contains("aur"));
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello world", 5), "hello...");
        assert_eq!(truncate("hi", 10), "hi");
    }

    #[test]
    fn test_render_result() {
        let result = SearchResult {
            name: "test-pkg".to_string(),
            version: "1.0.0".to_string(),
            description: "A test package".to_string(),
            source: PackageSource::Official,
            repo: "extra".to_string(),
            installed: false,
            #[cfg(feature = "arch")]
            votes: None,
            #[cfg(feature = "arch")]
            popularity: None,
            #[cfg(feature = "arch")]
            out_of_date: false,
        };

        let rendered = SearchModel::render_result(&result, 0);
        assert!(rendered.contains("test-pkg"));
        assert!(rendered.contains("1.0.0"));
    }

    #[test]
    fn test_search_msg_results_found() {
        let mut model = SearchModel::new();
        let results = vec![SearchResult {
            name: "pkg1".to_string(),
            version: "1.0".to_string(),
            description: "desc1".to_string(),
            source: PackageSource::Official,
            repo: "extra".to_string(),
            installed: false,
            #[cfg(feature = "arch")]
            votes: None,
            #[cfg(feature = "arch")]
            popularity: None,
            #[cfg(feature = "arch")]
            out_of_date: false,
        }];

        let _cmd = model.update(SearchMsg::ResultsFound(results));
        assert_eq!(model.state, SearchState::ShowingResults);
        assert_eq!(model.results.len(), 1);
        assert_eq!(model.official_count, 1);
    }

    #[test]
    fn test_search_msg_no_results() {
        let mut model = SearchModel::new().with_query("nonexistent".to_string());
        let _cmd = model.update(SearchMsg::NoResults);
        assert_eq!(model.state, SearchState::NoResults);
    }

    #[test]
    fn test_search_msg_error() {
        let mut model = SearchModel::new();
        let _cmd = model.update(SearchMsg::Error("network error".to_string()));
        assert_eq!(model.state, SearchState::Failed);
        assert_eq!(model.error, Some("network error".to_string()));
    }

    #[test]
    fn test_from_sync_package() {
        let sync_pkg = SyncPackage {
            name: "firefox".to_string(),
            version: crate::package_managers::parse_version_or_zero("123.0"),
            description: "Web browser".to_string(),
            repo: "extra".to_string(),
            download_size: 0,
            installed: true,
        };

        let result: SearchResult = sync_pkg.into();
        assert_eq!(result.name, "firefox");
        assert!(result.installed);
        assert_eq!(result.source, PackageSource::Official);
    }

    #[test]
    fn test_from_core_package() {
        let core_pkg = Package {
            name: "vim".to_string(),
            version: crate::package_managers::parse_version_or_zero("9.0"),
            description: "Editor".to_string(),
            source: crate::core::PackageSource::Aur,
            installed: false,
        };

        let result: SearchResult = core_pkg.into();
        assert_eq!(result.name, "vim");
        assert_eq!(result.source, PackageSource::Aur);
    }
}
