//! Explicit package listing functionality

use anyhow::{Context, Result};
use serde::Serialize;

#[cfg(unix)]
use crate::core::client::DaemonClient;
#[cfg(unix)]
use crate::daemon::protocol::{Request, ResponseResult};

#[derive(Serialize)]
struct ExplicitJson {
    packages: Vec<String>,
    count: usize,
}

pub fn explicit_sync(count: bool) -> Result<()> {
    explicit_sync_with_json(count, false)
}

#[allow(
    clippy::needless_return,
    reason = "additive backend feature branches return before compiled fallbacks"
)]
pub fn explicit_sync_with_json(count: bool, json: bool) -> Result<()> {
    #[cfg(unix)]
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

    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    if crate::core::env::distro::is_debian_like() {
        let packages = crate::package_managers::debian_db::list_explicit_fast()
            .context("Failed to list explicitly installed packages")?;
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

        #[cfg(all(
            any(feature = "debian", feature = "debian-pure"),
            not(feature = "arch")
        ))]
        {
            let packages = crate::package_managers::list_explicit_fast()
                .context("Failed to list explicitly installed packages")?;
            if json {
                println!(r#"{{"count": {}}}"#, packages.len());
            } else {
                println!("{}", packages.len());
            }
            return Ok(());
        }

        #[cfg(not(any(feature = "arch", feature = "debian", feature = "debian-pure")))]
        {
            return explicit_requires_backend();
        }
    }

    #[cfg(feature = "arch")]
    {
        let packages = crate::package_managers::list_explicit_fast()
            .context("Failed to list explicitly installed packages")?;
        display_explicit_list(packages, json)?;
        #[allow(
            clippy::needless_return,
            reason = "required when additive backend features compile later fallback blocks"
        )]
        return Ok(());
    }

    #[cfg(all(
        any(feature = "debian", feature = "debian-pure"),
        not(feature = "arch")
    ))]
    {
        let packages = crate::package_managers::list_explicit_fast()
            .context("Failed to list explicitly installed packages")?;
        display_explicit_list(packages, json)?;
        return Ok(());
    }

    #[cfg(not(any(feature = "arch", feature = "debian", feature = "debian-pure")))]
    explicit_requires_backend()
}

#[cfg(any(
    not(any(feature = "arch", feature = "debian", feature = "debian-pure")),
    test
))]
fn explicit_requires_backend() -> Result<()> {
    anyhow::bail!(
        "Explicit package listing is not available without an Arch or Debian package backend"
    )
}

fn display_explicit_list(mut packages: Vec<String>, json: bool) -> Result<()> {
    packages.sort();

    if json {
        let output = ExplicitJson {
            count: packages.len(),
            packages,
        };
        let json_str = serde_json::to_string_pretty(&output)
            .context("Failed to serialize explicit package list as JSON")?;
        println!("{json_str}");
        return Ok(());
    }

    use owo_colors::OwoColorize;
    use std::io::Write;
    let mut stdout = std::io::BufWriter::new(std::io::stdout());

    // Modern header
    crate::cli::modern_ui::print_phase_header(
        "📦",
        "Explicit Packages",
        &format!("{} installed", packages.len()),
    );
    println!();

    for pkg in &packages {
        if crate::cli::style::colors_enabled() {
            println!("  {} {}", "·".cyan(), pkg.bold());
        } else {
            println!("  · {pkg}");
        }
    }

    println!();
    stdout.flush()?;
    Ok(())
}

/// List explicitly installed packages (Async fallback)
pub async fn explicit(count: bool) -> Result<()> {
    explicit_sync(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_without_backend_is_an_error() {
        let error = explicit_requires_backend()
            .expect_err("explicit listing with no backend must not look like success");
        assert!(
            error
                .to_string()
                .contains("not available without an Arch or Debian package backend"),
            "got: {error}"
        );
    }
}
