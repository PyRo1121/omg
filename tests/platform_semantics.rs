/// Assert output does not include Arch-specific terms.
///
/// # Panics
/// Panics if the output contains Arch or AUR semantics.
pub fn assert_no_arch_terms(output: &str, context: &str) {
    assert!(
        !output.contains("AUR")
            && !output.contains("pacman")
            && !output.contains("makepkg")
            && !output.contains("yay")
            && !output.contains("paru"),
        "{context} must not show Arch/AUR semantics"
    );
}

/// Assert output does not include Debian-specific terms.
///
/// # Panics
/// Panics if the output contains Debian or APT semantics.
pub fn assert_no_debian_terms(output: &str, context: &str) {
    assert!(
        !output.contains("apt")
            && !output.contains("dpkg")
            && !output.contains("apt-get")
            && !output.contains("deb "),
        "{context} must not show Debian/APT semantics"
    );
}

/// Assert output does not include Fedora-specific terms.
///
/// # Panics
/// Panics if the output contains Fedora or DNF semantics.
pub fn assert_no_fedora_terms(output: &str, context: &str) {
    assert!(
        !output.contains("dnf")
            && !output.contains("yum")
            && !output.contains("rpm ")
            && !output.contains("fedora"),
        "{context} must not show Fedora/DNF semantics"
    );
}

/// Assert output does not include macOS-specific terms.
///
/// # Panics
/// Panics if the output contains Homebrew semantics.
pub fn assert_no_macos_terms(output: &str, context: &str) {
    assert!(
        !output.contains("brew")
            && !output.contains("homebrew")
            && !output.contains("cask")
            && !output.contains("formula")
            && !output.contains("cellar"),
        "{context} must not show macOS/Homebrew semantics"
    );
}
