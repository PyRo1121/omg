use std::future::Future;

use anyhow::{Context, Result};

pub(crate) fn run_blocking_future<F, T>(fut: F) -> Result<T>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let run = || -> Result<T> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("Failed to create async runtime")?;
        Ok(rt.block_on(fut))
    };

    if tokio::runtime::Handle::try_current().is_ok() {
        return std::thread::spawn(run).join().map_err(|_| {
            anyhow::anyhow!("Background thread panicked while executing async task")
        })?;
    }

    run()
}

#[cfg(test)]
mod tests {
    use super::run_blocking_future;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn nested_current_thread_runtime_completes() {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime");
            let value = rt.block_on(async {
                run_blocking_future(async {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    42
                })
            });
            let _ = tx.send(value);
        });
        let value = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("run_blocking_future deadlocked on the current-thread runtime");
        assert_eq!(value.expect("background thread panicked"), 42);
    }
}
