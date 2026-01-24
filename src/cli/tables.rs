//! Table formatting utilities for OMG CLI output
//!
//! Provides beautiful table layouts using `comfy-table` with:
//! - Auto-sizing columns
//! - Multiple border presets
//! - Color/row styling
//! - UTF-8 and ASCII fallback support

use crate::cli::style;
use comfy_table::{Attribute, Cell, Color, ContentArrangement, Table, presets::UTF8_FULL};

/// Table output style presets
#[derive(Debug, Clone, Copy, Default)]
pub enum TableStyle {
    /// Full UTF-8 borders with rounded corners (default)
    #[default]
    Full,

    /// Simple ASCII borders
    Simple,

    /// No borders, just spacing
    Compact,

    /// Minimal borders (header separator only)
    Minimal,
}

impl TableStyle {
    /// Get whether to hide borders
    #[must_use]
    #[allow(dead_code)]
    fn hide_borders(self) -> bool {
        matches!(self, Self::Compact)
    }
}

/// Create a new table with OMG styling
#[must_use]
pub fn new_table() -> Table {
    let use_color = style::colors_enabled();
    let use_unicode = style::use_unicode();

    let mut table = Table::new();

    // Load appropriate preset
    if use_unicode {
        table.load_preset(UTF8_FULL);
    } else {
        table.load_preset(comfy_table::presets::ASCII_FULL);
    }

    // Set content arrangement
    table.set_content_arrangement(ContentArrangement::DynamicFullWidth);

    // Configure colors if enabled
    if use_color {
        table.apply_modifier(comfy_table::modifiers::UTF8_ROUND_CORNERS);
    }

    table
}

/// Create a table with specific columns
#[must_use]
pub fn table_with_columns(columns: &[&str]) -> Table {
    let mut table = new_table();
    let use_color = style::colors_enabled();

    let header_cells: Vec<Cell> = columns
        .iter()
        .map(|col| {
            let mut cell = Cell::new(*col);
            if use_color {
                cell = cell.add_attribute(Attribute::Bold);
            }
            cell
        })
        .collect();

    table.set_header(header_cells);
    table
}

/// Add a row to a table
pub fn add_row(table: &mut Table, cells: &[&str]) {
    let use_color = style::colors_enabled();

    let row_cells: Vec<Cell> = cells
        .iter()
        .enumerate()
        .map(|(idx, cell)| {
            let mut c = Cell::new(*cell);

            // Color the first column (usually the name/package)
            if use_color && idx == 0 {
                c = c.add_attribute(Attribute::Bold);
            }

            c
        })
        .collect();

    table.add_row(row_cells);
}

/// Add a colored row to a table
pub fn add_colored_row(table: &mut Table, cells: &[(&str, Option<Color>)]) {
    let use_color = style::colors_enabled();

    let row_cells: Vec<Cell> = cells
        .iter()
        .map(|(cell_text, color)| {
            let mut cell = Cell::new(*cell_text);

            if use_color && let Some(c) = color {
                cell = cell.fg(*c);
            }

            cell
        })
        .collect();

    table.add_row(row_cells);
}

/// Render a package search results table
///
/// # Example
/// ```ignore
/// let packages = vec![
///     ("python", "3.12.1", "extra", "High-level scripting language"),
///     ("nodejs", "22.1.0", "extra", "JavaScript runtime"),
/// ];
/// tables::render_search_results(&packages);
/// ```
pub fn render_search_results(packages: &[(&str, &str, &str, &str)]) {
    if packages.is_empty() {
        println!("{} No packages found", style::warning("⚠"));
        return;
    }

    let use_color = style::colors_enabled();

    let mut table = table_with_columns(&["Package", "Version", "Repo", "Description"]);

    for (name, version, repo, desc) in packages {
        let mut name_cell = Cell::new(*name);

        // Color package name if AUR
        if use_color {
            name_cell = name_cell.add_attribute(Attribute::Bold);
            if *repo == "aur" || *repo == "AUR" {
                name_cell = name_cell.fg(Color::Magenta);
            }
        }

        let version_cell = if use_color {
            Cell::new(*version).fg(Color::Green)
        } else {
            Cell::new(*version)
        };

        let repo_cell = if use_color {
            let color = match *repo {
                "aur" | "AUR" => Color::Magenta,
                _ => Color::Cyan,
            };
            Cell::new(*repo).fg(color).add_attribute(Attribute::Dim)
        } else {
            Cell::new(*repo)
        };

        table.add_row(vec![name_cell, version_cell, repo_cell, Cell::new(*desc)]);
    }

    println!("{table}");
}

/// Render an outdated packages table
pub fn render_outdated(packages: &[(&str, &str, &str, &str)]) {
    if packages.is_empty() {
        println!("{} All packages are up to date!", style::success("✓"));
        return;
    }

    let use_color = style::colors_enabled();

    let mut table = table_with_columns(&["Package", "Current", "Latest", "Repo"]);

    for (name, old_ver, new_ver, repo) in packages {
        let name_cell = if use_color {
            Cell::new(*name).add_attribute(Attribute::Bold)
        } else {
            Cell::new(*name)
        };

        let old_cell = Cell::new(*old_ver);
        let new_cell = if use_color {
            Cell::new(*new_ver).fg(Color::Green)
        } else {
            Cell::new(*new_ver)
        };

        let repo_cell = if use_color {
            Cell::new(*repo).fg(Color::Cyan)
        } else {
            Cell::new(*repo)
        };

        table.add_row(vec![name_cell, old_cell, new_cell, repo_cell]);
    }

    println!();
    println!("{}", style::header("Updates Available"));
    println!("{table}");
    println!();
    println!("  Run {} to install updates", style::command("omg update"));
}

/// Render installed runtimes table
pub fn render_runtimes(runtimes: &[(&str, &str, &str)]) {
    if runtimes.is_empty() {
        println!("{} No runtimes installed", style::info("ℹ"));
        return;
    }

    let use_color = style::colors_enabled();

    let mut table = table_with_columns(&["Runtime", "Version", "Status"]);

    for (name, version, status) in runtimes {
        let name_cell = if use_color {
            Cell::new(*name)
                .fg(Color::Cyan)
                .add_attribute(Attribute::Bold)
        } else {
            Cell::new(*name)
        };

        let version_cell = Cell::new(*version);

        let status_cell = if use_color {
            let color = match *status {
                "active" | "●" => Color::Green,
                "inactive" | "○" => Color::Grey,
                _ => Color::Yellow,
            };
            Cell::new(*status).fg(color)
        } else {
            Cell::new(*status)
        };

        table.add_row(vec![name_cell, version_cell, status_cell]);
    }

    println!();
    println!("{}", style::header("Installed Runtimes"));
    println!("{table}");
}

/// Render orphan packages table
pub fn render_orphans(orphans: &[&str]) {
    if orphans.is_empty() {
        println!("{} No orphan packages found", style::success("✓"));
        return;
    }

    let use_color = style::colors_enabled();

    let mut table = new_table();
    table.set_header(vec![
        Cell::new("Orphan Packages").add_attribute(Attribute::Bold),
    ]);

    for orphan in orphans {
        let cell = if use_color {
            Cell::new(*orphan).fg(Color::Yellow)
        } else {
            Cell::new(*orphan)
        };
        table.add_row(vec![cell]);
    }

    println!();
    println!(
        "{} {} orphan package(s):",
        style::warning("⚠"),
        orphans.len()
    );
    println!("{table}");
    println!();
    println!(
        "  Run {} to remove orphans",
        style::command("omg clean --orphans")
    );
}

/// Simple key-value table for configuration display
pub fn render_kv_table(data: &[(&str, &str)]) {
    let use_color = style::colors_enabled();

    let mut table = new_table();
    table.set_header(vec![
        Cell::new("Setting").add_attribute(Attribute::Bold),
        Cell::new("Value").add_attribute(Attribute::Bold),
    ]);

    for (key, value) in data {
        let key_cell = if use_color {
            Cell::new(*key).fg(Color::Cyan)
        } else {
            Cell::new(*key)
        };

        table.add_row(vec![key_cell, Cell::new(*value)]);
    }

    println!("{table}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_empty_search() {
        // Should not panic on empty results
        render_search_results(&[]);
    }

    #[test]
    fn test_render_empty_outdated() {
        // Should show success message for no updates
        render_outdated(&[]);
    }

    #[test]
    fn test_render_empty_orphans() {
        // Should show success message for no orphans
        render_orphans(&[]);
    }

    #[test]
    fn test_table_creation() {
        let table = new_table();
        // Should create without panic
        drop(table);
    }

    #[test]
    fn test_table_with_columns() {
        let table = table_with_columns(&["Name", "Version"]);
        // Should create with columns
        drop(table);
    }
}
