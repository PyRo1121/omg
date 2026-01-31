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
    match rustix::thread::capabilities(None) {
        Ok(caps) => {
            let effective = caps.effective;
            // We need CAP_DAC_OVERRIDE to write to root-owned directories
            effective.contains(CapabilitySet::DAC_OVERRIDE)
        }
        Err(_) => false,
    }
}

#[cfg(not(target_os = "linux"))]
pub fn has_package_caps() -> bool {
    false
}

/// Check if we're running in elevated mode (re-exec'd with sudo)
#[inline]
pub fn is_elevated() -> bool {
    std::env::var_os("OMG_ELEVATED").is_some()
}

/// Check if we can perform privileged operations (either via caps or being root)
#[inline]
pub fn can_write_pacman_db() -> bool {
    has_package_caps() || crate::core::privilege::is_root()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_package_caps_does_not_panic() {
        // Just ensure it doesn't panic - actual result depends on how test is run
        let _ = has_package_caps();
    }

    #[test]
    fn test_is_elevated_without_env() {
        // Test default state - is_elevated returns based on current env
        // We can't modify env safely, so just verify the function works
        let _ = is_elevated();
    }

    #[test]
    fn test_is_elevated_with_env() {
        // Use temp_env for safe env var manipulation
        temp_env::with_var("OMG_ELEVATED", Some("1"), || {
            assert!(is_elevated());
        });
    }

    #[test]
    fn test_is_elevated_without_env_explicit() {
        // Verify unset state
        temp_env::with_var_unset("OMG_ELEVATED", || {
            assert!(!is_elevated());
        });
    }

    #[test]
    fn test_can_write_pacman_db() {
        // Just ensure it doesn't panic
        let _ = can_write_pacman_db();
    }
}
