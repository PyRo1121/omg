/// Split `output` into lowercased alphanumeric tokens so term checks are
/// word-boundary aware (e.g. "apt" must not match "adapt" or "capture").
fn tokens(output: &str) -> Vec<String> {
    output
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

fn assert_no_terms(output: &str, context: &str, terms: &[&str]) {
    let words = tokens(output);
    for term in terms {
        assert!(
            !words.iter().any(|word| word == term),
            "{context} must not show '{term}' semantics, got: {output}"
        );
    }
}

/// Assert output does not include Arch-specific terms.
///
/// # Panics
/// Panics if the output contains Arch or AUR semantics.
pub fn assert_no_arch_terms(output: &str, context: &str) {
    assert_no_terms(
        output,
        context,
        &["aur", "pacman", "makepkg", "yay", "paru"],
    );
}

/// Assert output does not include Debian-specific terms.
///
/// # Panics
/// Panics if the output contains Debian or APT semantics.
pub fn assert_no_debian_terms(output: &str, context: &str) {
    assert_no_terms(output, context, &["apt", "aptget", "dpkg", "deb"]);
}

/// Assert output does not include Fedora-specific terms.
///
/// # Panics
/// Panics if the output contains Fedora or DNF semantics.
pub fn assert_no_fedora_terms(output: &str, context: &str) {
    assert_no_terms(output, context, &["dnf", "yum", "rpm", "fedora"]);
}

/// Assert output does not include macOS-specific terms.
///
/// # Panics
/// Panics if the output contains Homebrew semantics.
pub fn assert_no_macos_terms(output: &str, context: &str) {
    assert_no_terms(
        output,
        context,
        &["brew", "homebrew", "cask", "formula", "cellar"],
    );
}
