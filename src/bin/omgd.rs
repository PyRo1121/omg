//! OMG Daemon Binary
//!
//! Persistent daemon with Unix socket IPC for fast package operations.

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tokio::net::UnixListener;

use omg_lib::core::paths;
use omg_lib::daemon::server;

#[cfg(not(target_env = "msvc"))]
use mimalloc::MiMalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

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

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_target(false)
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
    server::run(listener).await?;

    // Cleanup socket on exit
    if socket_path.exists() {
        let _ = std::fs::remove_file(&socket_path);
    }

    Ok(())
}
