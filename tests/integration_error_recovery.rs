use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use omg_lib::core::retry::{RetryConfig, retry_async};
use omg_lib::core::transactions::{Operation, Transaction, TransactionState};

#[tokio::test]
async fn integration_transaction_full_lifecycle() {
    let mut tx = Transaction::new();
    assert_eq!(tx.state, TransactionState::Pending);

    tx.begin().unwrap();
    assert_eq!(tx.state, TransactionState::InProgress);

    tx.add_operation(Operation::Install {
        package: "integration-test-pkg".to_string(),
        version: "1.0.0".parse().unwrap(),
        source: omg_lib::core::PackageSource::Official,
    });

    let duration_before_commit = tx.duration();
    assert!(duration_before_commit.as_nanos() > 0);

    tx.commit().unwrap();
    assert_eq!(tx.state, TransactionState::Committed);
}

#[tokio::test]
async fn integration_retry_with_recovery() {
    let config = RetryConfig::new(5);
    let attempts = Arc::new(AtomicU32::new(0));

    let result = retry_async(&config, || {
        let counter = Arc::clone(&attempts);
        async move {
            let current = counter.fetch_add(1, Ordering::SeqCst);
            if current < 2 {
                Err(anyhow::anyhow!("connection timeout"))
            } else {
                Ok(42)
            }
        }
    })
    .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 42);
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn integration_transaction_rollback_sequence() {
    let mut tx = Transaction::new();

    tx.add_operation(Operation::Install {
        package: "pkg-a".to_string(),
        version: "1.0.0".parse().unwrap(),
        source: omg_lib::core::PackageSource::Official,
    });

    tx.add_operation(Operation::Install {
        package: "pkg-b".to_string(),
        version: "2.0.0".parse().unwrap(),
        source: omg_lib::core::PackageSource::Official,
    });

    tx.add_operation(Operation::Update {
        package: "pkg-c".to_string(),
        old_version: "1.0.0".parse().unwrap(),
        new_version: "1.5.0".parse().unwrap(),
    });

    let rollback_result = tx.rollback().await;
    assert!(rollback_result.is_ok());
    assert_eq!(tx.state, TransactionState::RolledBack);
    assert_eq!(tx.operations.len(), 3);
}

#[tokio::test]
async fn integration_retry_exhausts_attempts() {
    let config = RetryConfig::new(2);
    let attempts = Arc::new(AtomicU32::new(0));

    let result = retry_async(&config, || {
        let counter = Arc::clone(&attempts);
        async move {
            counter.fetch_add(1, Ordering::SeqCst);
            Err::<i32, _>(anyhow::anyhow!("network connection refused"))
        }
    })
    .await;

    assert!(result.is_err());
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn integration_transaction_prevents_double_begin() {
    let mut tx = Transaction::new();

    tx.begin().unwrap();
    assert_eq!(tx.state, TransactionState::InProgress);

    let second_begin = tx.begin();
    assert!(second_begin.is_err());
    assert!(
        second_begin
            .unwrap_err()
            .to_string()
            .contains("already started")
    );
}

#[tokio::test]
async fn integration_transaction_snapshot_cleanup() {
    let mut tx = Transaction::new();

    tx.begin().unwrap();
    assert!(tx.snapshot.is_some());

    tx.commit().unwrap();
    assert!(tx.snapshot.is_none());
}

#[tokio::test]
async fn integration_retry_backoff_increases() {
    use std::time::Instant;

    let config = RetryConfig::default().with_backoff(50, 5000);
    let attempts = Arc::new(AtomicU32::new(0));
    let start = Instant::now();

    let _result = retry_async(&config, || {
        let counter = Arc::clone(&attempts);
        async move {
            let current = counter.fetch_add(1, Ordering::SeqCst);
            if current < 2 {
                Err::<i32, _>(anyhow::anyhow!("timeout"))
            } else {
                Ok(100)
            }
        }
    })
    .await;

    let elapsed = start.elapsed();
    assert!(elapsed.as_millis() >= 150);
}
