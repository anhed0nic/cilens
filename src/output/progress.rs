use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

use super::styling::{bright, bright_green, bright_yellow};

/// Progress tracking for multi-phase operations
pub struct PhaseProgress {
    pb: ProgressBar,
}

impl PhaseProgress {
    pub fn start_phase_1() -> Self {
        eprintln!("{}  {}", bright("⚙️"), bright("Phases").underlined());
        let pb = create_spinner(bright_yellow("Phase 1/3: Fetching pipelines").to_string());
        Self { pb }
    }

    pub fn finish_phase_1_start_phase_2(self) -> Self {
        self.pb
            .finish_with_message(bright_green("Phase 1/3: Fetched pipelines ✓").to_string());
        let pb =
            create_spinner(bright_yellow("Phase 2/3: Fetching jobs for pipelines").to_string());
        Self { pb }
    }

    pub fn finish_phase_2_start_phase_3(self) -> Self {
        self.pb.finish_with_message(
            bright_green("Phase 2/3: Fetched jobs for all pipelines ✓").to_string(),
        );
        let pb = create_spinner(bright_yellow("Phase 3/3: Processing insights").to_string());
        Self { pb }
    }

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
