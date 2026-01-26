//! Explicit package listing functionality

use anyhow::Result;
use serde::Serialize;

use crate::cli::ui;
use crate::core::client::DaemonClient;
use crate::daemon::protocol::{Request, ResponseResult};

use super::common::use_debian_backend;

#[derive(Serialize)]
struct ExplicitJson {
    packages: Vec<String>,
    count: usize,
}

pub fn explicit_sync(count: bool) -> Result<()> {
    explicit_sync_with_json(count, false)
}

pub fn explicit_sync_with_json(count: bool, json: bool) -> Result<()> {
    if let Ok(mut client) = DaemonClient::connect_sync() {
        let request = if count {
            Request::ExplicitCount { id: 0 }
        } else {
            Request::Explicit { id: 0 }
        };

        if let Ok(res) = client.call_sync(&request) {
            match res {
                ResponseResult::ExplicitCount(c) => {
                    if json {
                        println!(r#"{{"count": {c}}}"#);
                    } else {
                        println!("{c}");
                    }
                    return Ok(());
                }
                ResponseResult::Explicit(res) => {
                    display_explicit_list(res.packages, json)?;
                    return Ok(());
                }
                _ => {}
            }
        }
    }

    if use_debian_backend() {
        #[cfg(feature = "debian")]
        {
            let packages = crate::package_managers::list_explicit_fast().unwrap_or_default();
            if count {
                if json {
                    println!(r#"{{"count": {}}}"#, packages.len());
                } else {
                    println!("{}", packages.len());
                }
            } else {
                display_explicit_list(packages, json)?;
            }
            return Ok(());
        }
    }

    if count {
        if let Some(c) = crate::core::fast_status::FastStatus::read_explicit_count() {
            if json {
                println!(r#"{{"count": {c}}}"#);
            } else {
                println!("{c}");
            }
            return Ok(());
        }

        #[cfg(feature = "arch")]
        {
            let count = crate::package_managers::pacman_db::get_explicit_count()?;
            if json {
                println!(r#"{{"count": {count}}}"#);
            } else {
                println!("{count}");
            }
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
        display_explicit_list(packages, json)?;
    }

    #[cfg(not(any(feature = "arch", feature = "debian")))]
    {
        println!("No package manager backend enabled");
    }

    Ok(())
}

fn display_explicit_list(mut packages: Vec<String>, json: bool) -> Result<()> {
    packages.sort();
    
    if json {
        let output = ExplicitJson {
            count: packages.len(),
            packages,
        };
        if let Ok(json_str) = serde_json::to_string_pretty(&output) {
            println!("{json_str}");
        }
        return Ok(());
    }
    
    use std::io::Write;
    let mut stdout = std::io::BufWriter::new(std::io::stdout());

    ui::print_header("OMG", "Explicitly installed packages");
    ui::print_spacer();

    for pkg in &packages {
        ui::print_list_item(pkg, None);
    }

    ui::print_spacer();
    ui::print_success(format!("Total: {} packages", packages.len()));
    ui::print_spacer();
    stdout.flush()?;
    Ok(())
}

/// List explicitly installed packages (Async fallback)
pub async fn explicit(count: bool) -> Result<()> {
    explicit_sync(count)
}
