//! OMG Daemon Binary
//!
//! Persistent daemon with Unix socket IPC for fast package operations.

use anyhow::Result;
use clap::Parser;
use sentry_tracing::EventFilter;
use std::path::PathBuf;
use tokio::net::UnixListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use omg_lib::core::paths;
use omg_lib::daemon::server;

// Using system allocator (pure Rust - no C dependency)

/// OMG Daemon - Background service for fast package operations
#[derive(Parser, Debug)]
#[command(name = "omgd")]
#[command(author = "OMG Team")]
#[command(version)]
#[command(about = "OMG Daemon for fast package operations")]
struct Args {
    /// Run in foreground (don't daemonize)
    #[arg(short, long)]
    foreground: bool,

    /// Socket path (default: $`XDG_RUNTIME_DIR/omg.sock`)
    #[arg(short, long)]
    socket: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize Sentry
    // DSN is loaded from OMG_SENTRY_DSN environment variable
    let _guard = sentry::init((
        std::env::var("OMG_SENTRY_DSN").ok(),
        sentry::ClientOptions {
            release: sentry::release_name!(),
            attach_stacktrace: true,
            ..Default::default()
        },
    ));

    // Initialize tracing with Sentry integration
    let env_filter = tracing_subscriber::EnvFilter::from_default_env()
        .add_directive(tracing::Level::INFO.into());

    let sentry_layer = sentry_tracing::layer().event_filter(|md| match md.level() {
        &tracing::Level::ERROR => EventFilter::Event,
        _ => EventFilter::Breadcrumb,
    });

    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .with(sentry_layer)
        .init();

    // Determine socket path
    let socket_path = args.socket.unwrap_or_else(paths::socket_path);

    tracing::info!("Starting OMG daemon (omgd) v{}", env!("CARGO_PKG_VERSION"));

    // Remove existing socket
    if socket_path.exists() {
        tracing::debug!("Removing existing socket at {:?}", socket_path);
        std::fs::remove_file(&socket_path)?;
    }

    // Create Unix socket listener
    let listener = UnixListener::bind(&socket_path)?;
    tracing::info!("Listening on {:?}", socket_path);

    // Set socket permissions (user only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&socket_path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&socket_path, perms)?;
    }

    // Run server
    // Capture panics in Sentry
    use futures::FutureExt;

    let result = std::panic::AssertUnwindSafe(async { server::run(listener).await })
        .catch_unwind()
        .await;

    match result {
        Ok(run_result) => run_result?,
        Err(e) => {
            let msg = if let Some(s) = e.downcast_ref::<&str>() {
                format!("Daemon panicked: {}", s)
            } else if let Some(s) = e.downcast_ref::<String>() {
                format!("Daemon panicked: {}", s)
            } else {
                "Daemon panicked: unknown error".to_string()
            };

            tracing::error!("{}", msg);
            anyhow::bail!(msg);
        }
    }

    // Cleanup socket on exit
    if socket_path.exists() {
        let _ = std::fs::remove_file(&socket_path);
    }

    Ok(())
}
