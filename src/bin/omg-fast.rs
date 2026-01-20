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
#![allow(clippy::cast_possible_truncation)] // IPC message lengths are bounded

use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map_or("ec", String::as_str);

    // Fast path for search/info via daemon IPC
    if (cmd == "s" || cmd == "search") && args.len() >= 3 {
        fast_search(&args[2]);
        return;
    }
    if (cmd == "i" || cmd == "info") && args.len() >= 3 {
        fast_info(&args[2]);
        return;
    }

    // Get status file path (same logic as daemon)
    let path = std::env::var("XDG_RUNTIME_DIR").map_or_else(
        |_| "/tmp/omg.status".to_string(),
        |d| format!("{d}/omg.status"),
    );

    // Read 32-byte status file
    let Ok(mut file) = File::open(&path) else {
        eprintln!("0");
        std::process::exit(1);
    };

    let mut buf = [0u8; 32];
    if file.read_exact(&mut buf).is_err() {
        eprintln!("0");
        std::process::exit(1);
    }

    // Validate magic (0x4F4D4753 = "OMGS")
    let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    if magic != 0x4F4D_4753 {
        eprintln!("0");
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

/// Get socket path
fn socket_path() -> String {
    std::env::var("OMG_SOCKET_PATH").unwrap_or_else(|_| {
        std::env::var("XDG_RUNTIME_DIR")
            .map_or_else(|_| "/tmp/omg.sock".to_string(), |d| format!("{d}/omg.sock"))
    })
}

/// Fast search via raw IPC (no serde, minimal parsing)
fn fast_search(query: &str) {
    let Ok(mut stream) = UnixStream::connect(socket_path()) else {
        eprintln!("Daemon not running");
        std::process::exit(1);
    };

    // Build minimal bitcode request manually
    // Request::Search { id: 0, query, limit: Some(20) }
    // We'll use the library for now since bitcode format is complex
    if let Err(e) = send_search_request(&mut stream, query) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

/// Fast info via raw IPC
fn fast_info(package: &str) {
    let Ok(mut stream) = UnixStream::connect(socket_path()) else {
        eprintln!("Daemon not running");
        std::process::exit(1);
    };

    if let Err(e) = send_info_request(&mut stream, package) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn send_search_request(
    stream: &mut UnixStream,
    query: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use omg_lib::daemon::protocol::{Request, Response, ResponseResult};

    let request = Request::Search {
        id: 0,
        query: query.to_string(),
        limit: Some(20),
    };

    let request_bytes = bitcode::serialize(&request)?;
    let len = request_bytes.len() as u32;

    stream.write_all(&len.to_be_bytes())?;
    stream.write_all(&request_bytes)?;

    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let resp_len = u32::from_be_bytes(len_buf) as usize;

    let mut resp_bytes = vec![0u8; resp_len];
    stream.read_exact(&mut resp_bytes)?;

    let response: Response = bitcode::deserialize(&resp_bytes)?;

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
            eprintln!("Error: {message}");
        }
        Response::Success { .. } => {}
    }

    Ok(())
}

fn send_info_request(
    stream: &mut UnixStream,
    package: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use omg_lib::daemon::protocol::{Request, Response, ResponseResult};

    let request = Request::Info {
        id: 0,
        package: package.to_string(),
    };

    let request_bytes = bitcode::serialize(&request)?;
    let len = request_bytes.len() as u32;

    stream.write_all(&len.to_be_bytes())?;
    stream.write_all(&request_bytes)?;

    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let resp_len = u32::from_be_bytes(len_buf) as usize;

    let mut resp_bytes = vec![0u8; resp_len];
    stream.read_exact(&mut resp_bytes)?;

    let response: Response = bitcode::deserialize(&resp_bytes)?;

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
            eprintln!("Package not found: {message}");
        }
        Response::Success { .. } => {}
    }

    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}
