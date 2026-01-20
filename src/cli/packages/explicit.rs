//! Explicit package listing functionality

use anyhow::Result;

use crate::cli::style;
use crate::core::client::DaemonClient;
use crate::daemon::protocol::{Request, ResponseResult};

use super::common::use_debian_backend;

#[cfg(feature = "debian")]
use crate::package_managers::apt_list_explicit;

/// List explicitly installed packages (Synchronous)
pub fn explicit_sync(count: bool) -> Result<()> {
    if use_debian_backend() {
        #[cfg(feature = "debian")]
        {
            let mut packages = apt_list_explicit().unwrap_or_default();
            packages.sort();
            if count {
                println!("{}", packages.len());
            } else {
                for pkg in packages {
                    println!("{}", pkg);
                }
            }
            return Ok(());
        }
    }
    if count {
        // FAST PATH: Read from daemon's status file (zero IPC, sub-ms)
        if let Some(count) = crate::core::fast_status::FastStatus::read_explicit_count() {
            println!("{count}");
            return Ok(());
        }

        // Fallback: IPC to daemon
        let total = DaemonClient::connect_sync()
            .ok()
            .and_then(|mut client| {
                if let Ok(ResponseResult::ExplicitCount(count)) =
                    client.call_sync(&Request::ExplicitCount { id: 0 })
                {
                    Some(count)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| {
                #[cfg(feature = "arch")]
                {
                    crate::package_managers::list_explicit_fast()
                        .map(|pkgs| pkgs.len())
                        .unwrap_or_default()
                }
                #[cfg(not(feature = "arch"))]
                {
                    0
                }
            });

        println!("{total}");
        return Ok(());
    }

    // Try daemon first
    let packages = DaemonClient::connect_sync()
        .ok()
        .and_then(|mut client| {
            if let Ok(ResponseResult::Explicit(res)) =
                client.call_sync(&Request::Explicit { id: 0 })
            {
                Some(res.packages)
            } else {
                None
            }
        })
        .unwrap_or_else(|| {
            #[cfg(feature = "arch")]
            {
                crate::package_managers::list_explicit_fast().unwrap_or_default()
            }
            #[cfg(not(feature = "arch"))]
            {
                Vec::new()
            }
        });

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
    // Just call sync version for now as it's already fast and safe
    explicit_sync(count)
}
