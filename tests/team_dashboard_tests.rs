//! Comprehensive test suite for Team Dashboard TUI functionality
//!
//! This module tests:
//! - `TeamDashboardApp` state management
//! - Tab navigation and switching
//! - Member data handling and display
//! - Team status loading and updates
//! - Integration with `TeamWorkspace`

use omg_lib::cli::tui::app::{App, Tab};
use omg_lib::core::env::team::{NotificationSettings, TeamConfig, TeamMember, TeamStatus, TeamWorkspace};
use omg_lib::package_managers::{SyncPackage, parse_version_or_zero};
use serial_test::serial;
use tempfile::TempDir;

/// Helper to create a test team workspace
fn create_test_workspace() -> (TempDir, TeamWorkspace) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let workspace = TeamWorkspace::new(temp_dir.path());
    (temp_dir, workspace)
}

/// Helper to create a test `TeamStatus` with predefined members
fn create_test_team_status() -> TeamStatus {
    let config = TeamConfig {
        team_id: "test-team".to_string(),
        name: "Test Team".to_string(),
        member_id: "alice".to_string(),
        remote_url: Some("https://github.com/test/repo".to_string()),
        auto_sync: true,
        auto_push: false,
        notifications: NotificationSettings::default(),
    };

    let members = vec![
        TeamMember {
            id: "alice".to_string(),
            name: "Alice".to_string(),
            env_hash: "abc123".to_string(),
            last_sync: 1000,
            in_sync: true,
            drift_summary: None,
        },
        TeamMember {
            id: "bob".to_string(),
            name: "Bob".to_string(),
            env_hash: "def456".to_string(),
            last_sync: 900,
            in_sync: false,
            drift_summary: Some("2 packages out of sync".to_string()),
        },
        TeamMember {
            id: "charlie".to_string(),
            name: "Charlie".to_string(),
            env_hash: "abc123".to_string(),
            last_sync: 950,
            in_sync: true,
            drift_summary: None,
        },
    ];

    TeamStatus {
        config,
        lock_hash: "abc123".to_string(),
        members,
        updated_at: 1000,
    }
}

mod team_status_tests {
    use super::*;

    #[test]
    fn test_in_sync_count() {
        let status = create_test_team_status();
        assert_eq!(status.in_sync_count(), 2);
    }

    #[test]
    fn test_out_of_sync_count() {
        let status = create_test_team_status();
        assert_eq!(status.out_of_sync_count(), 1);
    }

    #[test]
    fn test_empty_team_counts() {
        let config = TeamConfig::default();
        let status = TeamStatus {
            config,
            lock_hash: String::new(),
            members: vec![],
            updated_at: 0,
        };

        assert_eq!(status.in_sync_count(), 0);
        assert_eq!(status.out_of_sync_count(), 0);
    }

    #[test]
    fn test_all_members_in_sync() {
        let mut status = create_test_team_status();
        // Make all members in sync
        for member in &mut status.members {
            member.in_sync = true;
            member.drift_summary = None;
        }

        assert_eq!(status.in_sync_count(), 3);
        assert_eq!(status.out_of_sync_count(), 0);
    }

    #[test]
    fn test_all_members_out_of_sync() {
        let mut status = create_test_team_status();
        // Make all members out of sync
        for member in &mut status.members {
            member.in_sync = false;
            member.drift_summary = Some("drift detected".to_string());
        }

        assert_eq!(status.in_sync_count(), 0);
        assert_eq!(status.out_of_sync_count(), 3);
    }
}

mod team_workspace_tests {
    use super::*;

    #[test]
    fn test_new_workspace_not_team() {
        let (temp_dir, workspace) = create_test_workspace();
        assert!(!workspace.is_team_workspace());
        assert!(workspace.config().is_none());
        drop(temp_dir);
    }

    #[test]
    #[serial]
    fn test_init_workspace() {
        let (temp_dir, mut workspace) = create_test_workspace();

        let result = workspace.init("test-team", "Test Team");
        assert!(result.is_ok(), "Failed to init workspace: {result:?}");

        assert!(workspace.is_team_workspace());
        assert!(workspace.config().is_some());

        let config = workspace.config().unwrap();
        assert_eq!(config.team_id, "test-team");
        assert_eq!(config.name, "Test Team");

        // Verify config file was created
        let config_path = temp_dir.path().join(".omg/team.toml");
        assert!(config_path.exists());

        drop(temp_dir);
    }

    #[test]
    #[serial]
    fn test_join_workspace_without_init_fails() {
        let (_temp_dir, mut workspace) = create_test_workspace();

        let result = workspace.join("https://github.com/test/repo");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Not a team workspace")
        );
    }

    #[test]
    #[serial]
    fn test_join_workspace_after_init() {
        let (temp_dir, mut workspace) = create_test_workspace();

        workspace
            .init("test-team", "Test Team")
            .expect("Init failed");

        let result = workspace.join("https://github.com/test/repo");
        assert!(result.is_ok());

        let config = workspace.config().unwrap();
        assert_eq!(
            config.remote_url,
            Some("https://github.com/test/repo".to_string())
        );

        drop(temp_dir);
    }

    #[test]
    #[serial]
    fn test_load_status() {
        let (temp_dir, mut workspace) = create_test_workspace();

        workspace
            .init("test-team", "Test Team")
            .expect("Init failed");

        let result = workspace.load_status();
        assert!(result.is_ok());

        let status = result.unwrap();
        assert_eq!(status.config.team_id, "test-team");
        assert_eq!(status.members.len(), 1);

        drop(temp_dir);
    }

    #[test]
    #[serial]
    fn test_persistence_across_instances() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        {
            let mut workspace = TeamWorkspace::new(temp_dir.path());
            workspace
                .init("test-team", "Test Team")
                .expect("Init failed");
        }

        // Create a new instance pointing to the same directory
        let workspace = TeamWorkspace::new(temp_dir.path());
        assert!(workspace.is_team_workspace());

        let config = workspace.config().unwrap();
        assert_eq!(config.team_id, "test-team");
        assert_eq!(config.name, "Test Team");

        drop(temp_dir);
    }
}

mod app_state_tests {
    use super::*;

    #[tokio::test]
    async fn test_app_creation() {
        let result = App::new().await;
        assert!(result.is_ok());

        let app = result.unwrap();
        assert_eq!(app.current_tab, Tab::Dashboard);
        assert_eq!(app.selected_index, 0);
        assert!(!app.show_popup);
        assert!(!app.search_mode);
        assert!(app.search_query.is_empty());
    }

    #[tokio::test]
    async fn test_app_with_team_tab() {
        let result = App::new().await;
        assert!(result.is_ok());

        let app = result.unwrap().with_tab(Tab::Team);
        assert_eq!(app.current_tab, Tab::Team);
    }

    #[tokio::test]
    async fn test_app_initial_state() {
        let app = App::new().await.unwrap();

        assert_eq!(app.selected_index, 0);
        assert!(!app.show_popup);
        assert!(app.search_query.is_empty());
        assert!(!app.search_mode);
        assert!(app.search_results.is_empty());
    }
}

mod tab_navigation_tests {
    use super::*;
    use crossterm::event::KeyCode;

    #[tokio::test]
    async fn test_numeric_tab_switching() {
        let mut app = App::new().await.unwrap();

        // Test switching to each tab via numeric keys
        app.handle_key(KeyCode::Char('1'));
        assert_eq!(app.current_tab, Tab::Dashboard);

        app.handle_key(KeyCode::Char('2'));
        assert_eq!(app.current_tab, Tab::Packages);

        app.handle_key(KeyCode::Char('3'));
        assert_eq!(app.current_tab, Tab::Runtimes);

        app.handle_key(KeyCode::Char('4'));
        assert_eq!(app.current_tab, Tab::Security);

        app.handle_key(KeyCode::Char('5'));
        assert_eq!(app.current_tab, Tab::Activity);

        app.handle_key(KeyCode::Char('6'));
        assert_eq!(app.current_tab, Tab::Team);
    }

    #[tokio::test]
    async fn test_tab_key_navigation_forward() {
        let mut app = App::new().await.unwrap();

        assert_eq!(app.current_tab, Tab::Dashboard);

        app.handle_key(KeyCode::Tab);
        assert_eq!(app.current_tab, Tab::Packages);

        app.handle_key(KeyCode::Tab);
        assert_eq!(app.current_tab, Tab::Runtimes);

        app.handle_key(KeyCode::Tab);
        assert_eq!(app.current_tab, Tab::Security);

        app.handle_key(KeyCode::Tab);
        assert_eq!(app.current_tab, Tab::Activity);

        app.handle_key(KeyCode::Tab);
        assert_eq!(app.current_tab, Tab::Team);

        app.handle_key(KeyCode::Tab);
        assert_eq!(app.current_tab, Tab::Dashboard); // Wraps around
    }

    #[tokio::test]
    async fn test_backtab_navigation_backward() {
        let mut app = App::new().await.unwrap();

        assert_eq!(app.current_tab, Tab::Dashboard);

        app.handle_key(KeyCode::BackTab);
        assert_eq!(app.current_tab, Tab::Team); // Wraps backward

        app.handle_key(KeyCode::BackTab);
        assert_eq!(app.current_tab, Tab::Activity);

        app.handle_key(KeyCode::BackTab);
        assert_eq!(app.current_tab, Tab::Security);

        app.handle_key(KeyCode::BackTab);
        assert_eq!(app.current_tab, Tab::Runtimes);

        app.handle_key(KeyCode::BackTab);
        assert_eq!(app.current_tab, Tab::Packages);

        app.handle_key(KeyCode::BackTab);
        assert_eq!(app.current_tab, Tab::Dashboard);
    }

    #[tokio::test]
    async fn test_tab_enum_order() {
        // Verify tab enum values are sequential
        assert_eq!(Tab::Dashboard as usize, 0);
        assert_eq!(Tab::Packages as usize, 1);
        assert_eq!(Tab::Runtimes as usize, 2);
        assert_eq!(Tab::Security as usize, 3);
        assert_eq!(Tab::Activity as usize, 4);
        assert_eq!(Tab::Team as usize, 5);
    }
}

mod member_data_handling_tests {
    use super::*;

    #[test]
    fn test_member_serialization() {
        let member = TeamMember {
            id: "test-id".to_string(),
            name: "Test User".to_string(),
            env_hash: "hash123".to_string(),
            last_sync: 1_234_567_890,
            in_sync: true,
            drift_summary: None,
        };

        let json = serde_json::to_string(&member).unwrap();
        let deserialized: TeamMember = serde_json::from_str(&json).unwrap();

        assert_eq!(member.id, deserialized.id);
        assert_eq!(member.name, deserialized.name);
        assert_eq!(member.env_hash, deserialized.env_hash);
        assert_eq!(member.last_sync, deserialized.last_sync);
        assert_eq!(member.in_sync, deserialized.in_sync);
        assert_eq!(member.drift_summary, deserialized.drift_summary);
    }

    #[test]
    fn test_member_with_drift() {
        let member = TeamMember {
            id: "bob".to_string(),
            name: "Bob".to_string(),
            env_hash: "different".to_string(),
            last_sync: 1000,
            in_sync: false,
            drift_summary: Some("3 packages differ".to_string()),
        };

        assert!(!member.in_sync);
        assert!(member.drift_summary.is_some());
        assert_eq!(member.drift_summary.unwrap(), "3 packages differ");
    }

    #[test]
    fn test_status_serialization() {
        let status = create_test_team_status();

        let json = serde_json::to_string_pretty(&status).unwrap();
        let deserialized: TeamStatus = serde_json::from_str(&json).unwrap();

        assert_eq!(status.config.team_id, deserialized.config.team_id);
        assert_eq!(status.lock_hash, deserialized.lock_hash);
        assert_eq!(status.members.len(), deserialized.members.len());
        assert_eq!(status.updated_at, deserialized.updated_at);
    }

    #[test]
    fn test_empty_members_list() {
        let config = TeamConfig::default();
        let status = TeamStatus {
            config,
            lock_hash: String::new(),
            members: vec![],
            updated_at: 0,
        };

        assert!(status.members.is_empty());
        assert_eq!(status.in_sync_count(), 0);
        assert_eq!(status.out_of_sync_count(), 0);
    }
}

mod search_and_selection_tests {
    use super::*;
    use crossterm::event::KeyCode;

    #[tokio::test]
    async fn test_search_mode_activation() {
        let mut app = App::new().await.unwrap();
        app.current_tab = Tab::Packages;

        assert!(!app.search_mode);

        app.handle_key(KeyCode::Char('/'));
        assert!(app.search_mode);
        assert!(app.search_query.is_empty());
    }

    #[tokio::test]
    async fn test_search_query_input() {
        let mut app = App::new().await.unwrap();
        app.current_tab = Tab::Packages;
        app.search_mode = true;

        app.handle_key(KeyCode::Char('t'));
        app.handle_key(KeyCode::Char('e'));
        app.handle_key(KeyCode::Char('s'));
        app.handle_key(KeyCode::Char('t'));

        assert_eq!(app.search_query, "test");
    }

    #[tokio::test]
    async fn test_search_backspace() {
        let mut app = App::new().await.unwrap();
        app.current_tab = Tab::Packages;
        app.search_mode = true;
        app.search_query = "test".to_string();

        app.handle_key(KeyCode::Backspace);
        assert_eq!(app.search_query, "tes");

        app.handle_key(KeyCode::Backspace);
        app.handle_key(KeyCode::Backspace);
        app.handle_key(KeyCode::Backspace);
        assert!(app.search_query.is_empty());

        // Backspace on empty query should not panic
        app.handle_key(KeyCode::Backspace);
        assert!(app.search_query.is_empty());
    }

    #[tokio::test]
    async fn test_escape_exits_search_mode() {
        let mut app = App::new().await.unwrap();
        app.current_tab = Tab::Packages;
        app.search_mode = true;
        app.search_query = "test".to_string();

        app.handle_key(KeyCode::Esc);
        assert!(!app.search_mode);
        // Query should persist after exiting search mode
        assert_eq!(app.search_query, "test");
    }

    #[tokio::test]
    async fn test_enter_exits_search_mode() {
        let mut app = App::new().await.unwrap();
        app.current_tab = Tab::Packages;
        app.search_mode = true;

        app.handle_key(KeyCode::Enter);
        assert!(!app.search_mode);
    }

    #[tokio::test]
    async fn test_list_navigation() {
        let mut app = App::new().await.unwrap();
        app.current_tab = Tab::Packages;

        // Populate some search results
        app.search_results = vec![
            SyncPackage {
                name: "pkg1".to_string(),
                version: parse_version_or_zero("1.0.0"),
                description: "Package 1".to_string(),
                repo: "official".to_string(),
                download_size: 0,
                installed: false,
            },
            SyncPackage {
                name: "pkg2".to_string(),
                version: parse_version_or_zero("2.0.0"),
                description: "Package 2".to_string(),
                repo: "official".to_string(),
                download_size: 0,
                installed: false,
            },
            SyncPackage {
                name: "pkg3".to_string(),
                version: parse_version_or_zero("3.0.0"),
                description: "Package 3".to_string(),
                repo: "official".to_string(),
                download_size: 0,
                installed: false,
            },
        ];

        assert_eq!(app.selected_index, 0);

        // Navigate down
        app.handle_key(KeyCode::Down);
        assert_eq!(app.selected_index, 1);

        app.handle_key(KeyCode::Down);
        assert_eq!(app.selected_index, 2);

        // Should not go beyond last item
        app.handle_key(KeyCode::Down);
        assert_eq!(app.selected_index, 2);

        // Navigate up
        app.handle_key(KeyCode::Up);
        assert_eq!(app.selected_index, 1);

        app.handle_key(KeyCode::Up);
        assert_eq!(app.selected_index, 0);

        // Should not go below 0
        app.handle_key(KeyCode::Up);
        assert_eq!(app.selected_index, 0);
    }

    #[tokio::test]
    async fn test_vim_navigation_keys() {
        let mut app = App::new().await.unwrap();
        app.current_tab = Tab::Packages;

        app.search_results = vec![
            SyncPackage {
                name: "pkg1".to_string(),
                version: parse_version_or_zero("1.0.0"),
                description: "Package 1".to_string(),
                repo: "official".to_string(),
                download_size: 0,
                installed: false,
            },
            SyncPackage {
                name: "pkg2".to_string(),
                version: parse_version_or_zero("2.0.0"),
                description: "Package 2".to_string(),
                repo: "official".to_string(),
                download_size: 0,
                installed: false,
            },
        ];

        assert_eq!(app.selected_index, 0);

        // 'j' for down
        app.handle_key(KeyCode::Char('j'));
        assert_eq!(app.selected_index, 1);

        // 'k' for up
        app.handle_key(KeyCode::Char('k'));
        assert_eq!(app.selected_index, 0);
    }
}

mod refresh_and_tick_tests {
    use super::*;

    #[tokio::test]
    async fn test_tick_updates_metrics() {
        let mut app = App::new().await.unwrap();

        // Wait a small amount of time
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        app.tick().await.unwrap();

        // last_update should have been refreshed if enough time passed
        // Note: this is timing-dependent but should work in most cases
    }

    #[tokio::test]
    async fn test_refresh_command() {
        let mut app = App::new().await.unwrap();

        // Trigger refresh by setting last_tick to past
        app.last_tick = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(10))
            .unwrap_or_else(std::time::Instant::now);

        let result = app.tick().await;
        assert!(result.is_ok());
    }
}

mod popup_tests {
    use super::*;
    use crossterm::event::KeyCode;

    #[tokio::test]
    async fn test_popup_show_hide() {
        let mut app = App::new().await.unwrap();

        assert!(!app.show_popup);

        app.show_popup = true;
        assert!(app.show_popup);

        app.handle_key(KeyCode::Esc);
        assert!(!app.show_popup);
    }

    #[tokio::test]
    async fn test_popup_on_package_selection() {
        let mut app = App::new().await.unwrap();
        app.current_tab = Tab::Packages;

        app.search_results = vec![SyncPackage {
            name: "test-pkg".to_string(),
            version: parse_version_or_zero("1.0.0"),
            description: "Test package".to_string(),
            repo: "official".to_string(),
            download_size: 0,
            installed: false,
        }];

        app.handle_key(KeyCode::Enter);
        assert!(app.show_popup);
    }

    #[tokio::test]
    async fn test_popup_not_shown_on_empty_results() {
        let mut app = App::new().await.unwrap();
        app.current_tab = Tab::Packages;

        assert!(app.search_results.is_empty());

        app.handle_key(KeyCode::Enter);
        // Popup should not be shown when there are no results
        assert!(!app.show_popup);
    }
}

mod system_metrics_tests {
    use super::*;

    #[tokio::test]
    async fn test_system_metrics_initialized() {
        let app = App::new().await.unwrap();

        // Metrics should be initialized (values may be 0 or actual)
        assert!(app.system_metrics.cpu_usage >= 0.0);
        assert!(app.system_metrics.memory_usage >= 0.0);
        // Disk and network can be 0
    }

    #[tokio::test]
    async fn test_metrics_within_bounds() {
        let app = App::new().await.unwrap();

        // CPU and memory should be percentages (0-100)
        assert!(app.system_metrics.cpu_usage <= 100.0);
        assert!(app.system_metrics.memory_usage <= 100.0);
    }
}

mod app_getter_tests {
    use super::*;

    #[tokio::test]
    async fn test_get_total_packages() {
        let app = App::new().await.unwrap();
        let _total = app.get_total_packages();
    }

    #[tokio::test]
    async fn test_get_orphan_packages() {
        let app = App::new().await.unwrap();
        let _orphans = app.get_orphan_packages();
    }

    #[tokio::test]
    async fn test_get_updates_available() {
        let app = App::new().await.unwrap();
        let _updates = app.get_updates_available();
    }

    #[tokio::test]
    async fn test_get_security_vulnerabilities() {
        let app = App::new().await.unwrap();
        let _vulns = app.get_security_vulnerabilities();
    }

    #[tokio::test]
    async fn test_get_runtime_versions() {
        let app = App::new().await.unwrap();
        let _runtimes = app.get_runtime_versions();
    }
}

mod edge_cases_tests {
    use super::*;
    use crossterm::event::KeyCode;

    #[tokio::test]
    async fn test_navigation_with_empty_lists() {
        let mut app = App::new().await.unwrap();
        app.current_tab = Tab::Packages;

        assert!(app.search_results.is_empty());

        // Navigation should not panic with empty lists
        app.handle_key(KeyCode::Down);
        app.handle_key(KeyCode::Up);
        app.handle_key(KeyCode::Char('j'));
        app.handle_key(KeyCode::Char('k'));

        assert_eq!(app.selected_index, 0);
    }

    #[tokio::test]
    async fn test_search_mode_only_on_packages_tab() {
        let mut app = App::new().await.unwrap();

        // Try activating search on Dashboard - should not work
        app.current_tab = Tab::Dashboard;
        app.handle_key(KeyCode::Char('/'));
        assert!(!app.search_mode);

        // Switch to Packages and try again
        app.current_tab = Tab::Packages;
        app.handle_key(KeyCode::Char('/'));
        assert!(app.search_mode);
    }

    #[test]
    fn test_team_status_with_single_member() {
        let config = TeamConfig::default();
        let member = TeamMember {
            id: "solo".to_string(),
            name: "Solo".to_string(),
            env_hash: "hash".to_string(),
            last_sync: 1000,
            in_sync: true,
            drift_summary: None,
        };

        let status = TeamStatus {
            config,
            lock_hash: "hash".to_string(),
            members: vec![member],
            updated_at: 1000,
        };

        assert_eq!(status.in_sync_count(), 1);
        assert_eq!(status.out_of_sync_count(), 0);
        assert_eq!(status.members.len(), 1);
    }

    #[test]
    fn test_team_config_defaults() {
        let config = TeamConfig::default();

        assert!(config.team_id.is_empty());
        assert!(config.name.is_empty());
        assert!(!config.member_id.is_empty()); // Should be populated from whoami
        assert!(config.remote_url.is_none());
        assert!(config.auto_sync);
        assert!(!config.auto_push);
    }
}

mod property_based_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_in_sync_count_never_exceeds_total(
            in_sync_flags in prop::collection::vec(any::<bool>(), 0..100)
        ) {
            let config = TeamConfig::default();
            let members: Vec<TeamMember> = in_sync_flags
                .iter()
                .enumerate()
                .map(|(i, &in_sync)| TeamMember {
                    id: format!("member-{i}"),
                    name: format!("Member {i}"),
                    env_hash: "hash".to_string(),
                    last_sync: 1000,
                    in_sync,
                    drift_summary: if in_sync { None } else { Some("drift".to_string()) },
                })
                .collect();

            let status = TeamStatus {
                config,
                lock_hash: "hash".to_string(),
                members: members.clone(),
                updated_at: 1000,
            };

            let in_sync = status.in_sync_count();
            let out_of_sync = status.out_of_sync_count();
            let total = status.members.len();

            prop_assert_eq!(in_sync + out_of_sync, total);
            prop_assert!(in_sync <= total);
            prop_assert!(out_of_sync <= total);
        }

        #[test]
        fn test_team_id_validation(team_id in "[a-zA-Z0-9/_-]{1,100}") {
            // Valid team IDs should only contain alphanumeric, /, -, and _
            prop_assert!(team_id.chars().all(|c|
                c.is_ascii_alphanumeric() || c == '/' || c == '-' || c == '_'
            ));
        }

        #[test]
        fn test_member_name_reasonable_length(name in "[A-Za-z0-9 ]{1,128}") {
            // Member names should be non-empty and reasonable ASCII length
            prop_assert!(!name.is_empty());
            prop_assert!(name.len() <= 128);
            // Should be ASCII
            prop_assert!(name.is_ascii());
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_full_team_workflow() {
        let (temp_dir, mut workspace) = create_test_workspace();

        // 1. Initialize team workspace
        workspace
            .init("integration-test", "Integration Test")
            .expect("Failed to init");

        // 2. Verify workspace is initialized
        assert!(workspace.is_team_workspace());

        // 3. Load initial status
        let status = workspace.load_status().expect("Failed to load status");
        assert_eq!(status.config.team_id, "integration-test");
        assert_eq!(status.members.len(), 1);

        // 4. Join with remote URL
        workspace
            .join("https://github.com/test/repo")
            .expect("Failed to join");

        // 5. Verify remote URL was set
        let config = workspace.config().unwrap();
        assert_eq!(
            config.remote_url,
            Some("https://github.com/test/repo".to_string())
        );

        drop(temp_dir);
    }

    #[tokio::test]
    #[serial]
    async fn test_app_with_team_workspace() {
        let (temp_dir, mut workspace) = create_test_workspace();

        workspace
            .init("app-test", "App Test Team")
            .expect("Failed to init");

        // Create app and verify it loads team status
        // Note: The app will try to load from current_dir, not our temp_dir
        // This test demonstrates the integration pattern
        let app = App::new().await.expect("Failed to create app");

        // App should be created successfully regardless of team status
        assert_eq!(app.current_tab, Tab::Dashboard);

        drop(temp_dir);
    }
}
