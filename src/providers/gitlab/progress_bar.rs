use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

/// Creates and manages progress indication for the three-phase insight collection process
pub struct PhaseProgress {
    pb: ProgressBar,
}

impl PhaseProgress {
    /// Create a new phase progress tracker and start Phase 1
    pub fn start_phase_1(limit: usize) -> Self {
        let pb = ProgressBar::new_spinner();
        pb.set_draw_target(ProgressDrawTarget::stderr());
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap(),
        );
        pb.set_message(format!("Phase 1/3: Fetching pipelines (limit: {limit})..."));
        pb.enable_steady_tick(std::time::Duration::from_millis(100));

        Self { pb }
    }

    /// Finish Phase 1 and start Phase 2
    pub fn finish_phase_1_start_phase_2(self, pipeline_count: usize) -> Self {
        self.pb
            .finish_with_message(format!("✓ Phase 1/3: Fetched {pipeline_count} pipelines"));

        let pb = ProgressBar::new_spinner();
        pb.set_draw_target(ProgressDrawTarget::stderr());
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap(),
        );
        pb.set_message("Phase 2/3: Fetching jobs for pipelines...");
        pb.enable_steady_tick(std::time::Duration::from_millis(100));

        Self { pb }
    }

    /// Finish Phase 2 and start Phase 3
    pub fn finish_phase_2_start_phase_3(self) -> Self {
        self.pb
            .finish_with_message("✓ Phase 2/3: Fetched jobs for all pipelines");

        let pb = ProgressBar::new_spinner();
        pb.set_draw_target(ProgressDrawTarget::stderr());
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap(),
        );
        pb.set_message("Phase 3/3: Processing insights...");
        pb.enable_steady_tick(std::time::Duration::from_millis(100));

        Self { pb }
    }

    /// Finish Phase 3 and complete all progress
    pub fn finish_phase_3(self) {
        self.pb
            .finish_with_message("✓ Phase 3/3: Insights processed successfully");
    }
}
