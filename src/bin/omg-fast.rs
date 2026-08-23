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

// Allow pedantic lints that are too strict for this minimal binary
#![allow(
    clippy::cast_possible_truncation,
    reason = "IPC message lengths are explicitly bounded"
)]

/// Maximum daemon response size to prevent memory exhaustion (10 MB)
#[cfg(unix)]
const MAX_RESPONSE_SIZE: usize = 10 * 1024 * 1024;

// Use mimalloc for even faster startup and allocations
#[cfg(unix)]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(unix)]
use std::fs::File;
#[cfg(unix)]
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::net::UnixStream;

#[cfg(unix)]
use anyhow::{Context, Result};
#[cfg(unix)]
use omg_lib::core::format::truncate;

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
    // (socket dir, honors OMG_SOCKET_PATH/XDG_RUNTIME_DIR). Do not hardcode
    // /tmp paths here; they would drift from paths.rs and miss permission
    // hardening.
    let path = omg_lib::core::paths::fast_status_path();

    // Read 32-byte status file
    let Ok(mut file) = File::open(&path) else {
        eprintln!(
            "omg-fast: no status file at {} (is the omg daemon running? try 'omg daemon' or 'omg status')",
            path.display()
        );
        std::process::exit(1);
    };

    let mut buf = [0u8; 32];
    if let Err(error) = file.read_exact(&mut buf) {
        eprintln!(
            "omg-fast: failed to read status file {}: {error}",
            path.display()
        );
        std::process::exit(1);
    }

    // Validate magic (0x4F4D4753 = "OMGS")
    let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    if magic != 0x4F4D_4753 {
        eprintln!(
            "omg-fast: status file {} has invalid magic bytes (stale or corrupt); rerun 'omg status'",
            path.display()
        );
        std::process::exit(1);
    }

    // Extract values
    let total = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
    let explicit = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);
    let orphans = u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]);
    let updates = u32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]);

    match cmd {
        "tc" | "total" => println!("{total}"),
        "ec" | "explicit" => println!("{explicit}"),
        "oc" | "orphan" => println!("{orphans}"),
        "uc" | "updates" => println!("{updates}"),
        "status" | "s" => {
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

/// Get socket path via the shared helper in `omg_lib::core::paths` so this
/// binary and the daemon can never disagree about where the socket lives.
#[cfg(unix)]
fn socket_path() -> String {
    omg_lib::core::paths::socket_path().display().to_string()
}

/// Fast search via raw IPC (no serde, minimal parsing)
#[cfg(unix)]
fn fast_search(query: &str) -> Result<()> {
    let path = socket_path();
    let mut stream = UnixStream::connect(&path)
        .with_context(|| format!("daemon not running (no listener at {path})"))?;
    send_search_request(&mut stream, query)
}

/// Fast info via raw IPC
#[cfg(unix)]
fn fast_info(package: &str) -> Result<()> {
    let path = socket_path();
    let mut stream = UnixStream::connect(&path)
        .with_context(|| format!("daemon not running (no listener at {path})"))?;
    send_info_request(&mut stream, package)
}

#[cfg(unix)]
fn send_search_request(stream: &mut UnixStream, query: &str) -> Result<()> {
    use omg_lib::daemon::protocol::{Request, Response, ResponseResult};
    // Compile-time guarantee: u32 response lengths fit in usize on all supported targets.
    const { assert!(usize::BITS >= 32, "omg-fast requires at least 32-bit usize") };

    let request = Request::Search {
        id: 0,
        query: query.to_string(),
        limit: Some(20),
    };

    let request_bytes = omg_lib::daemon::protocol::encode_frame(&request)?;
    let len = u32::try_from(request_bytes.len())
        .context("Search request too large for protocol framing")?;

    stream.write_all(&len.to_be_bytes())?;
    stream.write_all(&request_bytes)?;

    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let resp_len = u32::from_be_bytes(len_buf) as usize;
    if resp_len > MAX_RESPONSE_SIZE {
        anyhow::bail!("Response too large: {resp_len} bytes exceeds {MAX_RESPONSE_SIZE}");
    }

    let mut resp_bytes = vec![0u8; resp_len];
    stream.read_exact(&mut resp_bytes)?;

    let (_, payload) = omg_lib::daemon::protocol::split_frame(&resp_bytes)?;
    let response: Response = bitcode::deserialize(payload)?;

    match response {
        Response::Success {
            result: ResponseResult::Search(res),
            ..
        } => {
            println!("Found {} packages:", res.total);
            for pkg in res.packages.iter().take(20) {
                println!(
                    "  {} {} - {}",
                    pkg.name,
                    pkg.version,
                    truncate(&pkg.description, 50)
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
    // Compile-time guarantee: u32 response lengths fit in usize on all supported targets.
    const { assert!(usize::BITS >= 32, "omg-fast requires at least 32-bit usize") };

    let request = Request::Info {
        id: 0,
        package: package.to_string(),
    };

    let request_bytes = omg_lib::daemon::protocol::encode_frame(&request)?;
    let len = u32::try_from(request_bytes.len())
        .context("Info request too large for protocol framing")?;

    stream.write_all(&len.to_be_bytes())?;
    stream.write_all(&request_bytes)?;

    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let resp_len = u32::from_be_bytes(len_buf) as usize;
    if resp_len > MAX_RESPONSE_SIZE {
        anyhow::bail!("Response too large: {resp_len} bytes exceeds {MAX_RESPONSE_SIZE}");
    }

    let mut resp_bytes = vec![0u8; resp_len];
    stream.read_exact(&mut resp_bytes)?;

    let (_, payload) = omg_lib::daemon::protocol::split_frame(&resp_bytes)?;
    let response: Response = bitcode::deserialize(payload)?;

    match response {
        Response::Success {
            result: ResponseResult::Info(info),
            ..
        } => {
            println!("{} {}", info.name, info.version);
            println!("  {}", info.description);
            if !info.url.is_empty() {
                println!("  URL: {}", info.url);
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
