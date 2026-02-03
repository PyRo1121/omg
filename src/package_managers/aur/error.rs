use thiserror::Error;

#[derive(Error, Debug)]
pub enum AurError {
    #[error("Package '{0}' not found on AUR")]
    #[allow(dead_code)]
    PackageNotFound(String),

    #[error("PKGBUILD not found for '{package}'\n  → The AUR package may not exist or the clone failed\n  → Try: omg aur clean {package} && omg install {package}", package = .0)]
    PkgbuildNotFound(String),

    #[error("Build failed for '{package}'\n  → Check the build log: {log_path}\n  → Common fixes:\n    - Install missing dependencies: omg install <dep>\n    - Clean and retry: omg aur clean {package}\n    - Check AUR comments for known issues", package = .package, log_path = .log_path)]
    BuildFailed { package: String, log_path: String },

    #[error("Git clone failed for '{package}'\n  → Check if the package exists: https://aur.archlinux.org/packages/{package}\n  → Verify your internet connection\n  → Try again: omg install {package}", package = .0)]
    GitCloneFailed(String),

    #[error("Git pull failed for '{package}'\n  → The local clone may have conflicts\n  → Fix: omg aur clean {package} && omg install {package}", package = .0)]
    GitPullFailed(String),

    #[error(
        "Network error connecting to AUR\n  → Check your internet connection\n  → AUR may be temporarily unavailable\n  → Try again in a few minutes"
    )]
    NetworkError(#[from] reqwest::Error),

    #[error(
        "Sandbox build failed\n  → bubblewrap is not installed\n  → Install: sudo pacman -S bubblewrap\n  → Or enable unsafe builds: omg config set aur.allow_unsafe_builds true"
    )]
    SandboxUnavailable,

    #[error(
        "No package archive found after build for '{0}'\n  → The build may have produced a different package name\n  → Check ~/.cache/omg/aur/_pkgdest/ for the built package"
    )]
    PackageArchiveNotFound(String),
}
