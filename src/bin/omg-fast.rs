//! Ultra-minimal binary for instant package queries
//!
//! This binary has ZERO dependencies and starts in <1ms.
//! It reads the daemon's binary status file directly.
//!
//! Usage:
//!   omg-fast ec       # explicit count
//!   omg-fast tc       # total count  
//!   omg-fast oc       # orphan count
//!   omg-fast uc       # updates count
//!   omg-fast status   # full status display

use std::fs::File;
use std::io::Read;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("ec");
    
    // Get status file path (same logic as daemon)
    let path = std::env::var("XDG_RUNTIME_DIR")
        .map(|d| format!("{}/omg.status", d))
        .unwrap_or_else(|_| "/tmp/omg.status".to_string());
    
    // Read 32-byte status file
    let mut file = match File::open(&path) {
        Ok(f) => f,
        Err(_) => {
            eprintln!("0");
            std::process::exit(1);
        }
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
            eprintln!("Usage: omg-fast [ec|tc|oc|uc|status]");
            std::process::exit(1);
        }
    };
}
