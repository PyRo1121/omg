use crate::cli::{style, ui};

/// Debian dry run: the APT backend never cleans orphaned dependencies, so no
/// recursion claim is printed.
pub fn remove_dry_run(packages: &[String]) {
    crate::cli::modern_ui::print_phase_header("🗑️", "Remove Preview", "dry run");
    println!();

    println!(
        "  {} The following packages would be removed:\n",
        style::info("→")
    );

    for pkg_name in packages {
        if let Ok(Some(info)) =
            crate::package_managers::debian_db::get_installed_info_fast(pkg_name)
        {
            println!(
                "    {} {} {}",
                style::error("✗"),
                style::package(&info.name),
                style::version(&info.version),
            );
        } else {
            println!(
                "    {} {} (not installed)",
                style::warning("?"),
                style::package(pkg_name)
            );
        }
    }

    ui::print_spacer();
    println!("  {} Space that would be freed: unknown", style::info("→"));
    ui::print_dry_run_footer();
}
