//! Progress indication utilities
//!
//! Provides progress bars and spinners for long-running operations
//! to give users immediate feedback during operations >100ms.

use indicatif::{ProgressBar, ProgressStyle};
use std::io::{self, Write};
use std::time::Duration;

/// Simple spinner for operations with indeterminate progress
pub fn show_spinner(message: &str) -> anyhow::Result<()> {
    let style = ProgressStyle::default_spinner();
    let pb = ProgressBar::new_spinner();
    pb.set_style(style);
    pb.set_message(message);
    pb.enable_steady_tick();

    // Show spinner for at least 500ms to be visible
    std::thread::sleep(Duration::from_millis(500));
    pb.finish();

    Ok(())
}

/// Progress bar for operations with determinate progress
pub fn show_progress_bar(total: u64, message: &str) -> ProgressBar {
    let style = ProgressStyle::default_bar();
    let pb = ProgressBar::new(total);
    pb.set_style(style);
    pb.set_message(message);
    pb.enable_steady_tick();

    pb
}

/// Multi-progress manager for handling multiple parallel operations
pub struct MultiProgress {
    bars: Vec<ProgressBar>,
    total: u64,
    completed: u64,
}

impl MultiProgress {
    pub fn new(total: u64) -> Self {
        Self {
            bars: Vec::new(),
            total,
            completed: 0,
        }
    }

    pub fn add(&mut self, message: &str, current: u64, increment: u64) -> ProgressBar {
        let pb = ProgressBar::new(increment);
        pb.set_message(message);
        pb.set_position(current);

        self.bars.push(pb);
        self.completed += increment;
        current += increment;

        pb
    }

    pub fn inc(&mut self, bar_index: usize) -> anyhow::Result<()> {
        if bar_index < self.bars.len() {
            self.bars[bar_index].inc(1)?;
            self.completed += 1;
        }
        Ok(())
    }

    pub fn finish(self) -> anyhow::Result<()> {
        for bar in &self.bars {
            bar.finish();
        }

        // Print overall progress summary
        if self.total > 0 {
            let percent = (self.completed * 100) / self.total;
            println!(
                "\n✅ Completed {}/{} operations ({}%)",
                self.completed, percent
            );
        }

        Ok(())
    }
}

/// Check if an operation might take longer than 100ms
pub fn should_show_progress(operation_start: std::time::Instant) -> bool {
    operation_start.elapsed().as_millis() > 100
}
