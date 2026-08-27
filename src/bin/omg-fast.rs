//! Ultra-minimal binary for instant package queries
//!
//! This binary has minimal dependencies and starts in <3ms.
//! It reads the daemon's binary status file or queries via IPC.
//!
//! Usage:
//!   omg-fast ec           # explicit count
//!   omg-fast tc           # total count
//!   omg-fast oc           # orphan count
//!   omg-fast uc           # updates count
//!   omg-fast status       # full status display
//!   omg-fast s `<query>`    # search packages
//!   omg-fast i `<package>`  # package info

// Use mimalloc for even faster startup and allocations
#[cfg(unix)]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;
#[cfg(unix)]
use std::os::unix::net::UnixStream;

#[cfg(unix)]
use anyhow::{Context, Result};
#[cfg(unix)]
use omg_lib::core::{fast_status::FastStatus, format::truncate};
#[cfg(unix)]
use omg_lib::daemon::protocol::{read_frame, write_frame};

#[cfg(unix)]
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map_or("ec", String::as_str);

    // Fast path for search/info via daemon IPC
    if matches!(cmd, "s" | "search") && args.len() >= 3 {
        let query = &args[2];
        // SECURITY: Basic validation for minimal binary
        if query.len() > 100 || query.chars().any(char::is_control) {
            eprintln!("Invalid search query");
            std::process::exit(1);
        }
        if let Err(error) = fast_search(query) {
            eprintln!("Error: {error}");
            std::process::exit(1);
        }
        return;
    }
    if matches!(cmd, "i" | "info") && args.len() >= 3 {
        let package = &args[2];
        // SECURITY: Basic validation for minimal binary
        if package.len() > 100
            || package.chars().any(|c| {
                !c.is_ascii_alphanumeric() && !matches!(c, '-' | '_' | '.' | '+' | '@' | '/')
            })
        {
            eprintln!("Invalid package name");
            std::process::exit(1);
        }
        if let Err(error) = fast_info(package) {
            eprintln!("Error: {error}");
            std::process::exit(1);
        }
        return;
    }

    // Status file path: shared with the daemon via `omg_lib::core::paths`
    // (socket dir, honors OMG_SOCKET_PATH/XDG_RUNTIME_DIR). Route through
    // FastStatus::read_default so magic, layout version, freshness, and
    // directory-ownership checks cannot drift from the daemon's writer.
    let Some(status) = FastStatus::read_default() else {
        eprintln!(
            "omg-fast: no fresh status file (is the omg daemon running? try 'omg daemon' or 'omg status')"
        );
        std::process::exit(1);
    };

    let total = status.total_packages;
    let explicit = status.explicit_packages;
    let orphans = status.orphan_packages;
    let updates = status.updates_available;

    match cmd {
        "tc" | "total" => println!("{total}"),
        "ec" | "explicit" => println!("{explicit}"),
        "oc" | "orphan" => println!("{orphans}"),
        "uc" | "updates" => println!("{updates}"),
        "status" | "s" => {
            // `s <query>` was already routed to search above; a bare "s"
            // intentionally falls through to the status display.
            println!("==> OMG System Status\n");
            if updates > 0 {
                println!("  ⚠ Updates: {updates} available");
            } else {
                println!("  ✓ Updates: System is up to date");
            }
            println!("  ✓ Packages: {total} total ({explicit} explicit)");
            if orphans > 0 {
                println!("  ⚠ Orphans: {orphans} packages");
            }
        }
        _ => {
            eprintln!("Usage: omg-fast [ec|tc|oc|uc|status|s <query>|i <pkg>]");
            std::process::exit(1);
        }
    }
}

/// Connect to the daemon socket with bounded I/O timeouts so a wedged daemon
/// cannot hang the "instant" binary indefinitely (mirrors core/client.rs).
#[cfg(unix)]
fn connect_daemon_stream() -> Result<UnixStream> {
    const DAEMON_IO_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
    let path = omg_lib::core::paths::socket_path();
    let stream = UnixStream::connect(&path)
        .with_context(|| format!("daemon not running (no listener at {})", path.display()))?;
    stream.set_read_timeout(Some(DAEMON_IO_TIMEOUT))?;
    stream.set_write_timeout(Some(DAEMON_IO_TIMEOUT))?;
    Ok(stream)
}

/// Send one framed request and decode the framed response.
#[cfg(unix)]
fn exchange(
    stream: &mut UnixStream,
    request: &omg_lib::daemon::protocol::Request,
) -> Result<omg_lib::daemon::protocol::Response> {
    use omg_lib::daemon::protocol;

    let request_bytes = protocol::encode_frame(request)?;
    write_frame(stream, &request_bytes)?;
    let resp_bytes = read_frame(stream)?;

    let (_, payload) = protocol::split_frame(&resp_bytes)?;
    Ok(bitcode::deserialize(payload)?)
}

/// Fast search via raw IPC (no serde, minimal parsing)
#[cfg(unix)]
fn fast_search(query: &str) -> Result<()> {
    let mut stream = connect_daemon_stream()?;
    send_search_request(&mut stream, query)
}

/// Fast info via raw IPC
#[cfg(unix)]
fn fast_info(package: &str) -> Result<()> {
    let mut stream = connect_daemon_stream()?;
    send_info_request(&mut stream, package)
}

#[cfg(unix)]
fn send_search_request(stream: &mut UnixStream, query: &str) -> Result<()> {
    use omg_lib::daemon::protocol::{Request, Response, ResponseResult};

    let response = exchange(
        stream,
        &Request::Search {
            id: 0,
            query: query.to_string(),
            limit: Some(20),
        },
    )?;

    match response {
        Response::Success {
            result: ResponseResult::Search(res),
            ..
        } => {
            println!("Found {} packages:", res.total);
            for pkg in res.packages.iter().take(20) {
                println!(
                    "  {} {} - {}",
                    omg_lib::cli::style::sanitize_terminal_text(&pkg.name),
                    omg_lib::cli::style::sanitize_terminal_text(&pkg.version),
                    truncate(
                        &omg_lib::cli::style::sanitize_terminal_text(&pkg.description),
                        50,
                    )
                );
            }
        }
        Response::Error { message, .. } => {
            // Protocol-level failure must exit non-zero, not look like an
            // empty result set.
            anyhow::bail!(message)
        }
        Response::Success { .. } => {}
    }

    Ok(())
}

#[cfg(unix)]
fn send_info_request(stream: &mut UnixStream, package: &str) -> Result<()> {
    use omg_lib::daemon::protocol::{Request, Response, ResponseResult};

    let response = exchange(
        stream,
        &Request::Info {
            id: 0,
            package: package.to_string(),
        },
    )?;

    match response {
        Response::Success {
            result: ResponseResult::Info(info),
            ..
        } => {
            println!(
                "{} {}",
                omg_lib::cli::style::sanitize_terminal_text(&info.name),
                omg_lib::cli::style::sanitize_terminal_text(&info.version)
            );
            println!(
                "  {}",
                omg_lib::cli::style::sanitize_terminal_text(&info.description)
            );
            if !info.url.is_empty() {
                println!(
                    "  URL: {}",
                    omg_lib::cli::style::sanitize_terminal_text(&info.url)
                );
            }
        }
        Response::Error { message, .. } => {
            anyhow::bail!(message)
        }
        Response::Success { .. } => {}
    }

    Ok(())
}

// Windows stub - fast queries not supported (no daemon)
#[cfg(not(unix))]
fn main() {
    eprintln!("Error: omg-fast is only supported on Unix-like systems");
    std::process::exit(1);
}
