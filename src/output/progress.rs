use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

use super::styling::{bright, bright_green, bright_yellow};

/// Progress tracking for multi-phase CI/CD insights collection.
///
/// Manages a spinner-based progress indicator through three phases:
/// 1. Fetching pipelines from the CI provider
/// 2. Fetching jobs for each pipeline
/// 3. Processing and analyzing the collected data
pub struct PhaseProgress {
    pb: ProgressBar,
}

impl PhaseProgress {
    /// Starts phase 1: Fetching pipelines.
    ///
    /// Creates and displays a progress spinner for pipeline fetching.
    #[must_use]
    pub fn start_phase_1() -> Self {
        eprintln!("{}  {}", bright("⚙️"), bright("Phases").underlined());
        let pb = create_spinner(bright_yellow("Phase 1/3: Fetching pipelines").to_string());
        Self { pb }
    }

    /// Finishes phase 1 and starts phase 2: Fetching jobs.
    ///
    /// Marks pipeline fetching as complete and starts job fetching progress.
    #[must_use]
    pub fn finish_phase_1_start_phase_2(self) -> Self {
        self.pb
            .finish_with_message(bright_green("Phase 1/3: Fetched pipelines ✓").to_string());
        let pb =
            create_spinner(bright_yellow("Phase 2/3: Fetching jobs for pipelines").to_string());
        Self { pb }
    }

    /// Finishes phase 2 and starts phase 3: Processing insights.
    ///
    /// Marks job fetching as complete and starts processing progress.
    #[must_use]
    pub fn finish_phase_2_start_phase_3(self) -> Self {
        self.pb.finish_with_message(
            bright_green("Phase 2/3: Fetched jobs for all pipelines ✓").to_string(),
        );
        let pb = create_spinner(bright_yellow("Phase 3/3: Processing insights").to_string());
        Self { pb }
    }

    /// Finishes phase 3: Processing complete.
    ///
    /// Marks all phases as complete and clears the progress indicator.
    pub fn finish_phase_3(self) {
        self.pb.finish_with_message(
            bright_green("Phase 3/3: Insights processed successfully ✓").to_string(),
        );
        eprintln!("\n");
    }
}

fn create_spinner(message: String) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_draw_target(ProgressDrawTarget::stderr());
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("  {msg} {spinner}")
            .unwrap(),
    );
    pb.set_message(message);
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    pb
}
