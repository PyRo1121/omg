#![cfg(feature = "arch")]
#![expect(clippy::unwrap_used, clippy::pedantic, clippy::nursery)]

//! E2E Tests: Telemetry Pipeline
//!
//! Comprehensive end-to-end tests for the telemetry system:
//! 1. Event data structures and serialization
//! 2. Session lifecycle management
//! 3. Performance timer utilities
//! 4. Batch payload generation
//! 5. Data integrity and determinism
//!
//! **Note**: These tests focus on data structures, serialization, and logic.
//! Actual telemetry sending is disabled in test mode by design (see common/mod.rs).
//! For full integration testing with network mocking, run with OMG_TEST_MODE=0.

use anyhow::Result;
use omg_lib::core::telemetry::{TelemetrySession, Timer, get_session_id, record_startup_time};
use omg_lib::core::telemetry_client::{
    BatchPayload, CommandEvent, FeatureEvent, PerformanceEvent, SessionEvent, TelemetryEvent,
    TelemetryPayload,
};
use serial_test::serial;
use std::time::Duration;
use tokio::time::sleep;

// ============================================================================
// Test 1: Event Serialization and Deserialization
// ============================================================================

#[tokio::test]
async fn test_command_event_serialization() -> Result<()> {
    let event = CommandEvent {
        command: "install".to_string(),
        subcommand: Some("multi".to_string()),
        packages: Some(vec!["firefox".to_string(), "vim".to_string()]),
        duration_ms: 5000,
        success: true,
        error: None,
        result_count: None,
        updated_count: None,
    };

    // Serialize
    let json = serde_json::to_string(&event)?;
    assert!(json.contains("install"));
    assert!(json.contains("multi"));
    assert!(json.contains("firefox"));
    assert!(json.contains("5000"));
    assert!(json.contains("true"));

    // Deserialize
    let deserialized: CommandEvent = serde_json::from_str(&json)?;
    assert_eq!(deserialized.command, "install");
    assert_eq!(deserialized.subcommand, Some("multi".to_string()));
    assert_eq!(deserialized.duration_ms, 5000);
    assert_eq!(deserialized.packages.as_ref().unwrap().len(), 2);

    Ok(())
}

#[tokio::test]
async fn test_search_event_with_result_count() -> Result<()> {
    let event = CommandEvent {
        command: "search".to_string(),
        subcommand: None,
        packages: Some(vec!["firefox".to_string()]),
        duration_ms: 75,
        success: true,
        error: None,
        result_count: Some(25),
        updated_count: None,
    };

    let json = serde_json::to_string(&event)?;
    assert!(json.contains("\"result_count\":25"));

    let deserialized: CommandEvent = serde_json::from_str(&json)?;
    assert_eq!(deserialized.result_count, Some(25));

    Ok(())
}

#[tokio::test]
async fn test_update_event_with_counts() -> Result<()> {
    let event = CommandEvent {
        command: "update".to_string(),
        subcommand: None,
        packages: None,
        duration_ms: 10000,
        success: true,
        error: None,
        result_count: None,
        updated_count: Some(5),
    };

    let json = serde_json::to_string(&event)?;
    assert!(json.contains("\"updated_count\":5"));

    Ok(())
}

#[tokio::test]
async fn test_error_event_serialization() -> Result<()> {
    let event = CommandEvent {
        command: "install".to_string(),
        subcommand: None,
        packages: Some(vec!["nonexistent-package".to_string()]),
        duration_ms: 500,
        success: false,
        error: Some("Package not found in repositories".to_string()),
        result_count: None,
        updated_count: None,
    };

    let json = serde_json::to_string(&event)?;
    assert!(json.contains("false"));
    assert!(json.contains("Package not found"));

    let deserialized: CommandEvent = serde_json::from_str(&json)?;
    assert!(!deserialized.success);
    assert!(deserialized.error.is_some());

    Ok(())
}

// ============================================================================
// Test 2: Performance Event
// ============================================================================

#[tokio::test]
async fn test_performance_event_serialization() -> Result<()> {
    let event = PerformanceEvent {
        metric_type: "daemon_startup".to_string(),
        duration_ms: 150,
        context: Some("cold_start".to_string()),
    };

    let json = serde_json::to_string(&event)?;
    assert!(json.contains("daemon_startup"));
    assert!(json.contains("150"));
    assert!(json.contains("cold_start"));

    let deserialized: PerformanceEvent = serde_json::from_str(&json)?;
    assert_eq!(deserialized.metric_type, "daemon_startup");
    assert_eq!(deserialized.duration_ms, 150);

    Ok(())
}

// ============================================================================
// Test 3: Feature Event
// ============================================================================

#[tokio::test]
async fn test_feature_event_serialization() -> Result<()> {
    let metadata = serde_json::json!({"parallel_jobs": 4, "cache_enabled": true});
    let event = FeatureEvent {
        feature: "parallel_install".to_string(),
        enabled: true,
        metadata: Some(metadata.clone()),
    };

    let json = serde_json::to_string(&event)?;
    assert!(json.contains("parallel_install"));
    assert!(json.contains("parallel_jobs"));

    let deserialized: FeatureEvent = serde_json::from_str(&json)?;
    assert_eq!(deserialized.feature, "parallel_install");
    assert!(deserialized.enabled);
    assert!(deserialized.metadata.is_some());

    Ok(())
}

// ============================================================================
// Test 4: Session Event
// ============================================================================

#[tokio::test]
async fn test_session_start_event() -> Result<()> {
    let event = SessionEvent {
        session_id: "test-session-123".to_string(),
        event_type: "start".to_string(),
        start_time: Some("2024-01-01T00:00:00.000Z".to_string()),
        end_time: None,
        commands_run: None,
        duration_secs: None,
    };

    let json = serde_json::to_string(&event)?;
    assert!(json.contains("test-session-123"));
    assert!(json.contains("start"));

    Ok(())
}

#[tokio::test]
async fn test_session_end_event() -> Result<()> {
    let event = SessionEvent {
        session_id: "test-session-123".to_string(),
        event_type: "end".to_string(),
        start_time: None,
        end_time: Some("2024-01-01T00:05:00.000Z".to_string()),
        commands_run: Some(10),
        duration_secs: Some(300),
    };

    let json = serde_json::to_string(&event)?;
    assert!(json.contains("\"commands_run\":10"));
    assert!(json.contains("\"duration_secs\":300"));

    Ok(())
}

// ============================================================================
// Test 5: Telemetry Payload Wrapper
// ============================================================================

#[tokio::test]
async fn test_telemetry_payload_structure() -> Result<()> {
    let event = TelemetryEvent::Command(CommandEvent {
        command: "search".to_string(),
        subcommand: None,
        packages: Some(vec!["test".to_string()]),
        duration_ms: 50,
        success: true,
        error: None,
        result_count: Some(10),
        updated_count: None,
    });

    let payload = TelemetryPayload::new(event);

    // Verify all required fields are populated
    assert!(!payload.timestamp.is_empty(), "Timestamp should be set");
    assert!(!payload.machine_id.is_empty(), "Machine ID should be set");
    assert!(!payload.version.is_empty(), "Version should be set");
    assert!(!payload.platform.is_empty(), "Platform should be set");
    assert_eq!(payload.retries, 0, "Initial retry count should be 0");

    // Verify serialization produces valid JSON
    let json = serde_json::to_string(&payload)?;
    assert!(json.contains("timestamp"));
    assert!(json.contains("machine_id"));
    assert!(json.contains("version"));
    assert!(json.contains("platform"));

    Ok(())
}

#[tokio::test]
async fn test_payload_contains_event_data() -> Result<()> {
    let event = TelemetryEvent::Command(CommandEvent {
        command: "install".to_string(),
        subcommand: None,
        packages: Some(vec!["firefox".to_string()]),
        duration_ms: 5000,
        success: true,
        error: None,
        result_count: None,
        updated_count: None,
    });

    let payload = TelemetryPayload::new(event);
    let json = serde_json::to_string(&payload)?;

    // Verify event data is nested correctly
    assert!(json.contains("install"));
    assert!(json.contains("firefox"));
    assert!(json.contains("5000"));

    Ok(())
}

// ============================================================================
// Test 6: Batch Payload
// ============================================================================

#[tokio::test]
async fn test_batch_payload_structure() -> Result<()> {
    let events = vec![
        TelemetryEvent::Command(CommandEvent {
            command: "search".to_string(),
            subcommand: None,
            packages: None,
            duration_ms: 50,
            success: true,
            error: None,
            result_count: Some(5),
            updated_count: None,
        }),
        TelemetryEvent::Performance(PerformanceEvent {
            metric_type: "startup".to_string(),
            duration_ms: 100,
            context: None,
        }),
        TelemetryEvent::Feature(FeatureEvent {
            feature: "daemon".to_string(),
            enabled: true,
            metadata: None,
        }),
    ];

    let payloads: Vec<TelemetryPayload> = events.into_iter().map(TelemetryPayload::new).collect();

    let batch = BatchPayload {
        events: payloads,
        batch_timestamp: jiff::Timestamp::now()
            .strftime("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string(),
        machine_id: "test-machine".to_string(),
    };

    assert_eq!(batch.events.len(), 3, "Batch should contain 3 events");
    assert!(!batch.batch_timestamp.is_empty());
    assert_eq!(batch.machine_id, "test-machine");

    // Verify serialization
    let json = serde_json::to_string(&batch)?;
    assert!(json.contains("batch_timestamp"));
    assert!(json.contains("machine_id"));
    assert!(json.contains("events"));

    Ok(())
}

#[tokio::test]
async fn test_empty_batch_payload() -> Result<()> {
    let batch = BatchPayload {
        events: Vec::new(),
        batch_timestamp: jiff::Timestamp::now()
            .strftime("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string(),
        machine_id: "test-machine".to_string(),
    };

    assert_eq!(batch.events.len(), 0, "Empty batch should have 0 events");

    let json = serde_json::to_string(&batch)?;
    assert!(json.contains("\"events\":[]"));

    Ok(())
}

// ============================================================================
// Test 7: Session Management
// ============================================================================

#[tokio::test]
#[serial]
async fn test_session_creation() -> Result<()> {
    use std::sync::atomic::Ordering;

    let session = TelemetrySession::default();

    assert!(
        !session.session_id.is_empty(),
        "Session ID should be generated"
    );
    assert!(!session.started_at.is_empty(), "Start time should be set");
    assert_eq!(
        session.commands_run.load(Ordering::Relaxed),
        0,
        "Initial command count should be 0"
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_session_id_retrieval() -> Result<()> {
    let session_id = get_session_id();
    assert!(!session_id.is_empty(), "Session ID should not be empty");

    // Should return the same session ID on subsequent calls
    let session_id_2 = get_session_id();
    assert_eq!(session_id, session_id_2, "Session ID should be stable");

    Ok(())
}

// ============================================================================
// Test 8: Timer Utility
// ============================================================================

#[tokio::test]
async fn test_timer_basic_usage() -> Result<()> {
    let timer = Timer::new("test_operation");

    // Simulate operation
    sleep(Duration::from_millis(50)).await;

    let elapsed = timer.elapsed_ms();
    assert!(
        elapsed >= 50,
        "Timer should track at least 50ms, got {}",
        elapsed
    );
    assert!(
        elapsed < 200,
        "Timer should not be wildly inaccurate, got {}",
        elapsed
    );

    Ok(())
}

#[tokio::test]
async fn test_timer_accuracy() -> Result<()> {
    let timer = Timer::new("precision_test");

    sleep(Duration::from_millis(100)).await;

    let elapsed = timer.elapsed_ms();
    assert!(elapsed >= 100, "Timer should be at least 100ms");
    assert!(elapsed < 150, "Timer should be reasonably accurate");

    Ok(())
}

// ============================================================================
// Test 9: Startup Time Tracking
// ============================================================================

#[tokio::test]
#[serial]
async fn test_startup_time_recording() -> Result<()> {
    record_startup_time();

    // Simulate some work
    sleep(Duration::from_millis(50)).await;

    let duration = omg_lib::core::telemetry::get_startup_duration_ms();
    assert!(duration.is_some(), "Startup duration should be recorded");

    let duration_value = duration.unwrap();
    assert!(
        duration_value >= 50,
        "Startup duration should be at least 50ms"
    );

    Ok(())
}

// ============================================================================
// Test 10: Data Integrity and Determinism
// ============================================================================

#[tokio::test]
async fn test_event_type_discrimination() -> Result<()> {
    // Create different event types
    let cmd_event = TelemetryEvent::Command(CommandEvent {
        command: "test".to_string(),
        subcommand: None,
        packages: None,
        duration_ms: 100,
        success: true,
        error: None,
        result_count: None,
        updated_count: None,
    });

    let perf_event = TelemetryEvent::Performance(PerformanceEvent {
        metric_type: "test".to_string(),
        duration_ms: 100,
        context: None,
    });

    let feat_event = TelemetryEvent::Feature(FeatureEvent {
        feature: "test".to_string(),
        enabled: true,
        metadata: None,
    });

    // Serialize each
    let cmd_json = serde_json::to_string(&cmd_event)?;
    let perf_json = serde_json::to_string(&perf_event)?;
    let feat_json = serde_json::to_string(&feat_event)?;

    // Verify type tags
    assert!(cmd_json.contains("\"type\":\"command\""));
    assert!(perf_json.contains("\"type\":\"performance\""));
    assert!(feat_json.contains("\"type\":\"feature\""));

    // Verify we can deserialize back
    let _: TelemetryEvent = serde_json::from_str(&cmd_json)?;
    let _: TelemetryEvent = serde_json::from_str(&perf_json)?;
    let _: TelemetryEvent = serde_json::from_str(&feat_json)?;

    Ok(())
}

#[tokio::test]
async fn test_optional_fields_omitted_when_none() -> Result<()> {
    let event = CommandEvent {
        command: "search".to_string(),
        subcommand: None,
        packages: None,
        duration_ms: 50,
        success: true,
        error: None,
        result_count: None,
        updated_count: None,
    };

    let json = serde_json::to_string(&event)?;

    // Optional fields should be omitted from JSON when None
    assert!(!json.contains("subcommand"), "subcommand should be omitted");
    assert!(!json.contains("packages"), "packages should be omitted");
    assert!(!json.contains("error"), "error should be omitted");
    assert!(
        !json.contains("result_count"),
        "result_count should be omitted"
    );
    assert!(
        !json.contains("updated_count"),
        "updated_count should be omitted"
    );

    Ok(())
}

#[tokio::test]
async fn test_round_trip_serialization() -> Result<()> {
    let original = CommandEvent {
        command: "install".to_string(),
        subcommand: Some("multi".to_string()),
        packages: Some(vec!["pkg1".to_string(), "pkg2".to_string()]),
        duration_ms: 5000,
        success: true,
        error: None,
        result_count: None,
        updated_count: None,
    };

    // Serialize
    let json = serde_json::to_string(&original)?;

    // Deserialize
    let deserialized: CommandEvent = serde_json::from_str(&json)?;

    // Verify all fields match
    assert_eq!(deserialized.command, original.command);
    assert_eq!(deserialized.subcommand, original.subcommand);
    assert_eq!(deserialized.packages, original.packages);
    assert_eq!(deserialized.duration_ms, original.duration_ms);
    assert_eq!(deserialized.success, original.success);
    assert_eq!(deserialized.error, original.error);

    Ok(())
}

#[tokio::test]
async fn test_timestamp_format() -> Result<()> {
    let payload = TelemetryPayload::new(TelemetryEvent::Command(CommandEvent {
        command: "test".to_string(),
        subcommand: None,
        packages: None,
        duration_ms: 100,
        success: true,
        error: None,
        result_count: None,
        updated_count: None,
    }));

    // Verify timestamp is ISO 8601 format
    let timestamp_regex = regex::Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$")?;
    assert!(
        timestamp_regex.is_match(&payload.timestamp),
        "Timestamp should be ISO 8601 format, got: {}",
        payload.timestamp
    );

    Ok(())
}

// ============================================================================
// Test 11: Platform Information
// ============================================================================

#[tokio::test]
async fn test_platform_string_format() -> Result<()> {
    let payload = TelemetryPayload::new(TelemetryEvent::Command(CommandEvent {
        command: "test".to_string(),
        subcommand: None,
        packages: None,
        duration_ms: 100,
        success: true,
        error: None,
        result_count: None,
        updated_count: None,
    }));

    // Platform should be in format "os-arch"
    assert!(
        payload.platform.contains('-'),
        "Platform should contain hyphen separator"
    );

    let parts: Vec<&str> = payload.platform.split('-').collect();
    assert_eq!(parts.len(), 2, "Platform should have exactly 2 parts");

    // First part should be OS (native Windows is no longer a supported target)
    assert!(
        ["linux", "macos"].contains(&parts[0]),
        "Invalid OS: {}",
        parts[0]
    );

    // Second part should be architecture
    assert!(
        ["x86_64", "aarch64", "arm"]
            .iter()
            .any(|&arch| parts[1].contains(arch)),
        "Invalid architecture: {}",
        parts[1]
    );

    Ok(())
}

#[tokio::test]
async fn test_version_populated() -> Result<()> {
    let payload = TelemetryPayload::new(TelemetryEvent::Command(CommandEvent {
        command: "test".to_string(),
        subcommand: None,
        packages: None,
        duration_ms: 100,
        success: true,
        error: None,
        result_count: None,
        updated_count: None,
    }));

    assert!(!payload.version.is_empty(), "Version should not be empty");

    // Version should follow semver format (basic check)
    let parts: Vec<&str> = payload.version.split('.').collect();
    assert!(parts.len() >= 2, "Version should have at least major.minor");

    Ok(())
}
