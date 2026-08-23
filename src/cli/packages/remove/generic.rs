use anyhow::Result;

use crate::cli::{style, ui};

/// Generic dry run: this backend never cleans orphaned dependencies, so no
/// recursion claim is printed.
pub fn remove_dry_run(packages: &[String]) -> Result<()> {
    crate::core::security::validate_package_names(packages)?;

    crate::cli::modern_ui::print_phase_header("🗑️", "Remove Preview", "dry run");
    println!();
    println!(
        "  {} The following packages would be removed:\n",
        style::info("→")
    );

    for pkg_name in packages {
        println!(
            "    {} {} (feature-specific info unavailable)",
            style::dim("○"),
            style::package(pkg_name)
        );
    }

    ui::print_spacer();
    println!("  {} Space that would be freed: unknown", style::info("→"));
    ui::print_dry_run_footer();
    Ok(())
}
