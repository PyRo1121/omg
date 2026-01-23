//! Explicit package listing functionality

use anyhow::Result;

use crate::cli::style;
use crate::core::client::DaemonClient;
use crate::daemon::protocol::{Request, ResponseResult};

use super::common::use_debian_backend;

/// List explicitly installed packages (Synchronous)
pub fn explicit_sync(count: bool) -> Result<()> {
    // 1. Try Daemon First (Hot Path - ALL DISTROS)
    if let Ok(mut client) = DaemonClient::connect_sync() {
        let request = if count {
            Request::ExplicitCount { id: 0 }
        } else {
            Request::Explicit { id: 0 }
        };

        if let Ok(res) = client.call_sync(&request) {
            match res {
                ResponseResult::ExplicitCount(c) => {
                    println!("{c}");
                    return Ok(());
                }
                ResponseResult::Explicit(res) => {
                    display_explicit_list(res.packages)?;
                    return Ok(());
                }
                _ => {}
            }
        }
    }

    // 2. Fallback to optimized direct parser (Cold Path)
    if use_debian_backend() {
        #[cfg(feature = "debian")]
        {
            let packages = crate::package_managers::list_explicit_fast().unwrap_or_default();
            if count {
                println!("{}", packages.len());
            } else {
                display_explicit_list(packages)?;
            }
            return Ok(());
        }
    }

    // 3. Arch Fallback
    if count {
        // FAST PATH: Read from daemon's status file (zero IPC, sub-ms)
        if let Some(c) = crate::core::fast_status::FastStatus::read_explicit_count() {
            println!("{c}");
            return Ok(());
        }

        #[cfg(feature = "arch")]
        {
            let count = crate::package_managers::pacman_db::get_explicit_count()?;
            println!("{count}");
            return Ok(());
        }

        #[cfg(not(feature = "arch"))]
        {
            anyhow::bail!("Explicit count only supported on Arch Linux");
        }
    }

    #[cfg(feature = "arch")]
    {
        let packages = crate::package_managers::list_explicit_fast().unwrap_or_default();
        display_explicit_list(packages)?;
    }

    #[cfg(not(any(feature = "arch", feature = "debian")))]
    {
        println!("No package manager backend enabled");
    }

    Ok(())
}

fn display_explicit_list(mut packages: Vec<String>) -> Result<()> {
    packages.sort();
    use std::io::Write;
    let mut stdout = std::io::BufWriter::new(std::io::stdout());

    writeln!(
        stdout,
        "{} Explicitly installed packages:\n",
        style::header("OMG")
    )?;

    for pkg in &packages {
        writeln!(stdout, "  {}", style::package(pkg))?;
    }

    writeln!(
        stdout,
        "\n{} {} packages",
        style::success("Total:"),
        packages.len()
    )?;
    stdout.flush()?;
    Ok(())
}

/// List explicitly installed packages (Async fallback)
pub async fn explicit(count: bool) -> Result<()> {
    explicit_sync(count)
}
