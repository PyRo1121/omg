use std::future::Future;

use anyhow::{Context, Result};

pub(crate) fn run_blocking_future<F, T>(fut: F) -> Result<T>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        return std::thread::spawn(move || handle.block_on(fut))
            .join()
            .map_err(|_| anyhow::anyhow!("Background thread panicked while executing async task"));
    }

    let rt = tokio::runtime::Runtime::new().context("Failed to create async runtime")?;
    Ok(rt.block_on(fut))
}
