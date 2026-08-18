//! Linux capabilities support for zero-sudo package operations
//!
//! When omg is installed with the right capabilities, it can perform
//! package operations without sudo, eliminating all privilege elevation overhead.
//!
//! Setup (run once during install):
//! ```bash
//! sudo setcap 'cap_dac_override,cap_fowner,cap_chown+ep' /usr/bin/omg
//! ```

#[cfg(target_os = "linux")]
use rustix::thread::CapabilitySet;

/// Check if the current process has the capabilities needed for package operations.
/// Returns true if we can write to /var/lib/pacman without sudo.
#[cfg(target_os = "linux")]
pub fn has_package_caps() -> bool {
    // Check effective capabilities
    // We need CAP_DAC_OVERRIDE to write to root-owned directories
    rustix::thread::capabilities(None)
        .is_ok_and(|caps| caps.effective.contains(CapabilitySet::DAC_OVERRIDE))
}

#[cfg(not(target_os = "linux"))]
pub fn has_package_caps() -> bool {
    false
}

/// Check if we're running in elevated mode (re-exec'd with sudo).
/// `OMG_ELEVATED` alone is not privilege; the process must also be root.
#[inline]
pub fn is_elevated() -> bool {
    std::env::var_os("OMG_ELEVATED").is_some() && crate::core::privilege::is_root()
}

/// Check if we can perform privileged operations (either via caps or being root)
#[inline]
pub fn can_write_pacman_db() -> bool {
    has_package_caps() || crate::core::privilege::is_root()
}

/// Show a one-time hint about turbo mode if not enabled
/// Returns true if the hint was shown
#[cfg(target_os = "linux")]
pub fn maybe_show_turbo_hint() -> bool {
    use std::io::Write;

    if has_package_caps() || is_elevated() || crate::core::privilege::is_root() {
        return false;
    }

    let hint_file = crate::core::paths::data_dir().join(".turbo_hint_shown");
    if hint_file.exists() {
        return false;
    }

    use owo_colors::OwoColorize;
    eprintln!();
    eprintln!(
        "  {} {}",
        "TIP:".bright_cyan().bold(),
        "Enable turbo mode for instant package operations:".dimmed()
    );
    eprintln!("       {}", "omg doctor --turbo".cyan().bold());
    eprintln!(
        "       {}",
        "(one-time setup, eliminates sudo prompts)".dimmed()
    );
    eprintln!();

    if let Ok(mut file) = std::fs::File::create(&hint_file) {
        let _ = file.write_all(b"1");
    }

    true
}

#[cfg(not(target_os = "linux"))]
pub fn maybe_show_turbo_hint() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_elevated_with_env() {
        temp_env::with_var("OMG_ELEVATED", Some("1"), || {
            assert_eq!(
                is_elevated(),
                crate::core::privilege::is_root(),
                "OMG_ELEVATED without root must not count as elevated"
            );
        });
    }

    #[test]
    fn test_is_elevated_without_env_explicit() {
        // Verify unset state
        temp_env::with_var_unset("OMG_ELEVATED", || {
            assert!(!is_elevated());
        });
    }
}
