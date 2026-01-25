mod alpm_harness;
use anyhow::Result;
use omg_lib::cli::{CliContext, CommandRunner, Commands, EnvCommands, FleetCommands, ToolCommands};
use serial_test::serial;
use std::fs;
use tempfile::tempdir;

fn get_ctx() -> CliContext {
    CliContext {
        verbose: 0,
        json: false,
        quiet: false,
        no_color: true,
    }
}

/// Remove license file to test license-gated features
fn ensure_no_license() {
    if let Some(data_dir) = dirs::data_dir() {
        let license_path = data_dir.join("omg").join("license.json");
        let _ = fs::remove_file(&license_path);
    }
}

#[tokio::test]
#[serial]
async fn test_env_capture_and_check_success() -> Result<()> {
    let temp = tempdir()?;
    std::env::set_current_dir(temp.path())?;

    let ctx = get_ctx();
    let capture_cmd = Commands::Env {
        command: EnvCommands::Capture,
    };

    capture_cmd.execute(&ctx).await?;
    assert!(temp.path().join("omg.lock").exists());

    let check_cmd = Commands::Env {
        command: EnvCommands::Check,
    };
    check_cmd.execute(&ctx).await?;

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_env_check_fails_without_lock() -> Result<()> {
    let temp = tempdir()?;
    std::env::set_current_dir(temp.path())?;

    let ctx = get_ctx();
    let check_cmd = Commands::Env {
        command: EnvCommands::Check,
    };

    let result = check_cmd.execute(&ctx).await;
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("No omg.lock file found")
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_env_check_fails_on_drift() -> Result<()> {
    let temp = tempdir()?;
    std::env::set_current_dir(temp.path())?;

    let ctx = get_ctx();

    fs::write(temp.path().join("omg.lock"), "{}")?;

    let check_cmd = Commands::Env {
        command: EnvCommands::Check,
    };
    let result = check_cmd.execute(&ctx).await;

    assert!(result.is_err());
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_tool_list_empty() -> Result<()> {
    let temp = tempdir()?;
    unsafe {
        std::env::set_var("HOME", temp.path());
    }

    let ctx = get_ctx();
    let list_cmd = Commands::Tool {
        command: ToolCommands::List,
    };
    list_cmd.execute(&ctx).await?;

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_tool_registry_output() -> Result<()> {
    let ctx = get_ctx();
    let reg_cmd = Commands::Tool {
        command: ToolCommands::Registry,
    };
    reg_cmd.execute(&ctx).await?;
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_tool_install_invalid_name_fails() -> Result<()> {
    ensure_no_license(); // Clear any existing license
    let ctx = get_ctx();
    let install_cmd = Commands::Tool {
        command: ToolCommands::Install {
            name: "../dangerous".to_string(),
        },
    };

    let result = install_cmd.execute(&ctx).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("cannot start with")
            || err.contains("path traversal")
            || err.contains("Invalid character")
    );
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_fleet_status_requires_license() -> Result<()> {
    ensure_no_license(); // Clear any existing license
    let ctx = get_ctx();
    let status_cmd = Commands::Fleet {
        command: FleetCommands::Status,
    };

    let result = status_cmd.execute(&ctx).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    eprintln!("Actual error: {err}");
    assert!(err.contains("license") || err.contains("feature") || err.contains("tier"));
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_fleet_push_invalid_team_fails() -> Result<()> {
    let ctx = get_ctx();
    let push_cmd = Commands::Fleet {
        command: FleetCommands::Push {
            team: Some("; rm -rf /".to_string()),
            message: None,
        },
    };

    let result = push_cmd.execute(&ctx).await;
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Invalid team identifier")
    );
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_run_invalid_task_fails() -> Result<()> {
    let ctx = get_ctx();
    let run_cmd = Commands::Run {
        task: "dangerous; command".to_string(),
        args: vec![],
        runtime_backend: None,
        watch: false,
        parallel: false,
        using: None,
        all: false,
    };

    let result = run_cmd.execute(&ctx).await;
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Invalid task name")
    );
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_run_detect_and_execute_mock_task() -> Result<()> {
    let temp = tempdir()?;
    std::env::set_current_dir(temp.path())?;

    fs::write(
        temp.path().join("Makefile"),
        "test:\n\t@echo 'Hello Test'\n",
    )?;

    let ctx = get_ctx();
    let run_cmd = Commands::Run {
        task: "test".to_string(),
        args: vec![],
        runtime_backend: None,
        watch: false,
        parallel: false,
        using: Some("make".to_string()),
        all: false,
    };

    let _ = run_cmd.execute(&ctx).await;

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_tool_install_not_in_registry_fails() -> Result<()> {
    unsafe {
        std::env::set_var("OMG_TEST_MODE", "1");
    }
    let ctx = get_ctx();
    let install_cmd = Commands::Tool {
        command: ToolCommands::Install {
            name: "non-existent-tool-xyz-123".to_string(),
        },
    };

    let result = install_cmd.execute(&ctx).await;
    unsafe {
        std::env::remove_var("OMG_TEST_MODE");
    }

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not in registry"));

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_env_share_missing_token_fails() -> Result<()> {
    let temp = tempdir()?;
    std::env::set_current_dir(temp.path())?;
    fs::write(temp.path().join("omg.lock"), "{}")?;

    unsafe {
        std::env::remove_var("GITHUB_TOKEN");
    }

    let ctx = get_ctx();
    let share_cmd = Commands::Env {
        command: EnvCommands::Share {
            description: "test".to_string(),
            public: false,
        },
    };

    let result = share_cmd.execute(&ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("GITHUB_TOKEN"));

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_fleet_remediate_dry_run_no_license_fails() -> Result<()> {
    ensure_no_license(); // Clear any existing license
    let ctx = get_ctx();
    let remediate_cmd = Commands::Fleet {
        command: FleetCommands::Remediate {
            dry_run: true,
            confirm: false,
        },
    };

    let result = remediate_cmd.execute(&ctx).await;
    assert!(result.is_err());

    Ok(())
}
