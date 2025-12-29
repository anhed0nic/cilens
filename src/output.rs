use crate::insights::CIInsights;
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, Color as TableColor, ContentArrangement, Table};
use console::style;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

// Styling helpers

fn bright_yellow(text: impl std::fmt::Display) -> console::StyledObject<String> {
    style(text.to_string()).bright().yellow()
}

fn bright_green(text: impl std::fmt::Display) -> console::StyledObject<String> {
    style(text.to_string()).bright().green()
}

fn cyan(text: impl std::fmt::Display) -> console::StyledObject<String> {
    style(text.to_string()).cyan()
}

fn dim(text: impl std::fmt::Display) -> console::StyledObject<String> {
    style(text.to_string()).dim()
}

fn bright(text: impl std::fmt::Display) -> console::StyledObject<String> {
    style(text.to_string()).bright()
}

fn magenta_bold(text: impl std::fmt::Display) -> console::StyledObject<String> {
    style(text.to_string()).magenta().bold()
}

// Banner

pub fn print_banner() {
    eprintln!(
        r"
{} {}
  {}
",
        magenta_bold("🔍 CILens"),
        dim(env!("CARGO_PKG_VERSION")),
        dim("CI/CD Insights Tool")
    );
}

// Progress tracking

pub struct PhaseProgress {
    pb: ProgressBar,
}

impl PhaseProgress {
    pub fn start_phase_1(limit: usize) -> Self {
        eprintln!("{}  {}", bright("⚙️"), bright("Phases").underlined());

        let pb = ProgressBar::new_spinner();
        pb.set_draw_target(ProgressDrawTarget::stderr());
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("  {msg} {spinner}")
                .unwrap(),
        );
        pb.set_message(
            bright_yellow(format!("Phase 1/3: Fetching pipelines (limit: {limit})")).to_string(),
        );
        pb.enable_steady_tick(std::time::Duration::from_millis(100));

        Self { pb }
    }

    pub fn finish_phase_1_start_phase_2(self, pipeline_count: usize) -> Self {
        self.pb.finish_with_message(
            bright_green(format!("Phase 1/3: Fetched {pipeline_count} pipelines ✓")).to_string(),
        );

        let pb = ProgressBar::new_spinner();
        pb.set_draw_target(ProgressDrawTarget::stderr());
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("  {msg} {spinner}")
                .unwrap(),
        );
        pb.set_message(bright_yellow("Phase 2/3: Fetching jobs for pipelines").to_string());
        pb.enable_steady_tick(std::time::Duration::from_millis(100));

        Self { pb }
    }

    pub fn finish_phase_2_start_phase_3(self) -> Self {
        self.pb.finish_with_message(
            bright_green("Phase 2/3: Fetched jobs for all pipelines ✓").to_string(),
        );

        let pb = ProgressBar::new_spinner();
        pb.set_draw_target(ProgressDrawTarget::stderr());
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("  {msg} {spinner}")
                .unwrap(),
        );
        pb.set_message(bright_yellow("Phase 3/3: Processing insights").to_string());
        pb.enable_steady_tick(std::time::Duration::from_millis(100));

        Self { pb }
    }

    pub fn finish_phase_3(self) {
        self.pb.finish_with_message(
            bright_green("Phase 3/3: Insights processed successfully ✓").to_string(),
        );
        eprintln!("\n");
    }
}

// Summary rendering

pub fn print_summary(insights: &CIInsights) {
    println!("{}", render_summary(insights));
}

#[allow(clippy::too_many_lines, clippy::format_push_string)]
fn render_summary(insights: &CIInsights) -> String {
    let mut output = String::new();

    // Overview section
    output.push_str(&format!(
        "{} {}\n",
        bright("📊"),
        bright("Overview").underlined()
    ));
    output.push_str(&format!(
        "  {} {}\n",
        dim("Project:"),
        cyan(&insights.project)
    ));
    output.push_str(&format!(
        "  {} {}\n",
        dim("Pipelines analyzed:"),
        bright_yellow(insights.total_pipelines)
    ));
    output.push_str(&format!(
        "  {} {}\n",
        dim("Pipeline types:"),
        bright_yellow(insights.total_pipeline_types)
    ));
    output.push_str(&format!(
        "  {} {}\n",
        dim("Analysis date:"),
        dim(insights.collected_at.format("%Y-%m-%d %H:%M UTC"))
    ));
    output.push('\n');

    if insights.pipeline_types.is_empty() {
        output.push_str(&format!("{}\n", bright_yellow("No pipeline data found.")));
        return output;
    }

    // Collect and deduplicate jobs by name (taking worst metrics across pipeline types)
    let mut jobs_by_name: std::collections::HashMap<String, &crate::insights::JobMetrics> =
        std::collections::HashMap::new();

    for pt in &insights.pipeline_types {
        for job in &pt.metrics.jobs {
            jobs_by_name
                .entry(job.name.clone())
                .and_modify(|existing| {
                    // Keep the job with worse metrics (max of P95 time-to-feedback)
                    if job.time_to_feedback_p95 > existing.time_to_feedback_p95 {
                        *existing = job;
                    }
                })
                .or_insert(job);
        }
    }

    let all_jobs: Vec<&crate::insights::JobMetrics> = jobs_by_name.values().copied().collect();

    // Top 10 Slowest Jobs
    output.push_str(&format!(
        "{} {}\n",
        bright("🐌"),
        bright("Top 10 Slowest Jobs").underlined()
    ));

    let mut sorted_by_time = all_jobs.clone();
    sorted_by_time.sort_by(|a, b| {
        b.time_to_feedback_p95
            .partial_cmp(&a.time_to_feedback_p95)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut slowest_table = Table::new();
    slowest_table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("#").fg(TableColor::Cyan),
            Cell::new("Job Name").fg(TableColor::Cyan),
            Cell::new("P95 Time").fg(TableColor::Cyan),
            Cell::new("Fail %").fg(TableColor::Cyan),
            Cell::new("Flaky %").fg(TableColor::Cyan),
        ]);

    for (idx, job) in sorted_by_time.iter().take(10).enumerate() {
        let time_cell = Cell::new(format!("{:.1}min", job.time_to_feedback_p95 / 60.0));

        let fail_cell = if job.failure_rate >= 50.0 {
            Cell::new(format!("{:.1}", job.failure_rate)).fg(TableColor::Red)
        } else if job.failure_rate >= 25.0 {
            Cell::new(format!("{:.1}", job.failure_rate)).fg(TableColor::Yellow)
        } else {
            Cell::new(format!("{:.1}", job.failure_rate)).fg(TableColor::Green)
        };

        let flaky_cell = if job.flakiness_rate >= 10.0 {
            Cell::new(format!("{:.1}", job.flakiness_rate)).fg(TableColor::Red)
        } else if job.flakiness_rate >= 5.0 {
            Cell::new(format!("{:.1}", job.flakiness_rate)).fg(TableColor::Yellow)
        } else {
            Cell::new(format!("{:.1}", job.flakiness_rate)).fg(TableColor::Green)
        };

        slowest_table.add_row(vec![
            Cell::new(idx + 1),
            Cell::new(&job.name),
            time_cell,
            fail_cell,
            flaky_cell,
        ]);
    }

    output.push_str(&format!("{slowest_table}\n\n"));

    // Top 5 Failing Jobs
    output.push_str(&format!(
        "{} {}\n",
        bright("❌"),
        bright("Top 5 Failing Jobs").underlined()
    ));

    let mut sorted_by_failure = all_jobs.clone();
    sorted_by_failure.sort_by(|a, b| {
        b.failure_rate
            .partial_cmp(&a.failure_rate)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut failing_table = Table::new();
    failing_table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("#").fg(TableColor::Cyan),
            Cell::new("Job Name").fg(TableColor::Cyan),
            Cell::new("Fail %").fg(TableColor::Cyan),
            Cell::new("P95 Time").fg(TableColor::Cyan),
        ]);

    for (idx, job) in sorted_by_failure.iter().take(5).enumerate() {
        let fail_cell = if job.failure_rate >= 50.0 {
            Cell::new(format!("{:.1}", job.failure_rate)).fg(TableColor::Red)
        } else if job.failure_rate >= 25.0 {
            Cell::new(format!("{:.1}", job.failure_rate)).fg(TableColor::Yellow)
        } else {
            Cell::new(format!("{:.1}", job.failure_rate)).fg(TableColor::Green)
        };

        failing_table.add_row(vec![
            Cell::new(idx + 1),
            Cell::new(&job.name),
            fail_cell,
            Cell::new(format!("{:.1}min", job.time_to_feedback_p95 / 60.0)),
        ]);
    }

    output.push_str(&format!("{failing_table}\n\n"));

    // Top 5 Flaky Jobs
    output.push_str(&format!(
        "{} {}\n",
        bright("🔄"),
        bright("Top 5 Flaky Jobs").underlined()
    ));

    let mut sorted_by_flakiness = all_jobs.clone();
    sorted_by_flakiness.sort_by(|a, b| {
        b.flakiness_rate
            .partial_cmp(&a.flakiness_rate)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut flaky_table = Table::new();
    flaky_table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("#").fg(TableColor::Cyan),
            Cell::new("Job Name").fg(TableColor::Cyan),
            Cell::new("Flaky %").fg(TableColor::Cyan),
            Cell::new("P95 Time").fg(TableColor::Cyan),
        ]);

    for (idx, job) in sorted_by_flakiness.iter().take(5).enumerate() {
        let flaky_cell = if job.flakiness_rate >= 10.0 {
            Cell::new(format!("{:.1}", job.flakiness_rate)).fg(TableColor::Red)
        } else if job.flakiness_rate >= 5.0 {
            Cell::new(format!("{:.1}", job.flakiness_rate)).fg(TableColor::Yellow)
        } else {
            Cell::new(format!("{:.1}", job.flakiness_rate)).fg(TableColor::Green)
        };

        flaky_table.add_row(vec![
            Cell::new(idx + 1),
            Cell::new(&job.name),
            flaky_cell,
            Cell::new(format!("{:.1}min", job.time_to_feedback_p95 / 60.0)),
        ]);
    }

    output.push_str(&format!("{flaky_table}\n\n"));

    // Pipeline Types (context)
    output.push_str(&format!(
        "{} {}\n",
        bright("📋"),
        bright("Pipeline Types (Context)").underlined()
    ));

    let mut types_table = Table::new();
    types_table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Pipeline Type").fg(TableColor::Cyan),
            Cell::new("Total %").fg(TableColor::Cyan),
            Cell::new("Success %").fg(TableColor::Cyan),
            Cell::new("P95 Time").fg(TableColor::Cyan),
            Cell::new("Example").fg(TableColor::Cyan),
        ]);

    for pt in insights.pipeline_types.iter().take(10) {
        let success_cell = if pt.metrics.success_rate >= 90.0 {
            Cell::new(format!("{:.1}", pt.metrics.success_rate)).fg(TableColor::Green)
        } else if pt.metrics.success_rate >= 70.0 {
            Cell::new(format!("{:.1}", pt.metrics.success_rate)).fg(TableColor::Yellow)
        } else {
            Cell::new(format!("{:.1}", pt.metrics.success_rate)).fg(TableColor::Red)
        };

        // Get example pipeline URL (prefer successful, fallback to failed)
        let example_url = pt
            .metrics
            .successful_pipelines
            .links
            .first()
            .or_else(|| pt.metrics.failed_pipelines.links.first())
            .map_or("N/A", |url| url.as_str());

        types_table.add_row(vec![
            Cell::new(&pt.label),
            Cell::new(format!("{:.1}", pt.metrics.percentage)),
            success_cell,
            Cell::new(format!("{:.1}min", pt.metrics.duration_p95 / 60.0)),
            Cell::new(example_url),
        ]);
    }

    if insights.pipeline_types.len() > 10 {
        types_table.add_row(vec![
            Cell::new(format!(
                "... and {} more types",
                insights.pipeline_types.len() - 10
            ))
            .fg(TableColor::DarkGrey),
            Cell::new(""),
            Cell::new(""),
            Cell::new(""),
            Cell::new(""),
        ]);
    }

    output.push_str(&format!("{types_table}\n\n"));

    // Next Steps
    output.push_str(&format!(
        "{} {}\n",
        bright("💡"),
        bright("Next Steps").underlined()
    ));
    output.push_str(&format!(
        "  {} Use {} flag to get detailed metrics and job dependencies\n",
        cyan("•"),
        bright_yellow("--json")
    ));
    output.push_str(&format!(
        "  {} Prioritize slowest jobs - they block developer feedback\n",
        cyan("•")
    ));
    output.push_str(&format!(
        "  {} Fix failing jobs - they create noise and reduce trust\n",
        cyan("•")
    ));
    output.push_str(&format!(
        "  {} Investigate flaky jobs - they waste CI resources and time\n",
        cyan("•")
    ));

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::insights::{
        CIInsights, JobCountWithLinks, JobMetrics, PipelineCountWithLinks, PipelineType,
        TypeMetrics,
    };
    use chrono::Utc;

    fn create_test_job(
        name: &str,
        time_to_feedback_p95: f64,
        failure_rate: f64,
        flakiness_rate: f64,
    ) -> JobMetrics {
        JobMetrics {
            name: name.to_string(),
            duration_p50: 100.0,
            duration_p95: 200.0,
            duration_p99: 300.0,
            time_to_feedback_p50: time_to_feedback_p95 * 0.5,
            time_to_feedback_p95,
            time_to_feedback_p99: time_to_feedback_p95 * 1.5,
            predecessors: vec![],
            flakiness_rate,
            flaky_retries: JobCountWithLinks::default(),
            failed_executions: JobCountWithLinks::default(),
            failure_rate,
            total_executions: 100,
        }
    }

    fn create_test_pipeline_type(
        label: &str,
        percentage: f64,
        success_rate: f64,
        duration_p95: f64,
        jobs: Vec<JobMetrics>,
        example_url: &str,
    ) -> PipelineType {
        PipelineType {
            label: label.to_string(),
            stages: vec!["test".to_string()],
            ref_patterns: vec!["main".to_string()],
            sources: vec!["push".to_string()],
            metrics: TypeMetrics {
                percentage,
                total_pipelines: 100,
                successful_pipelines: PipelineCountWithLinks {
                    count: 90,
                    links: vec![example_url.to_string()],
                },
                failed_pipelines: PipelineCountWithLinks::default(),
                success_rate,
                duration_p50: duration_p95 * 0.5,
                duration_p95,
                duration_p99: duration_p95 * 1.5,
                time_to_feedback_p50: 100.0,
                time_to_feedback_p95: 200.0,
                time_to_feedback_p99: 300.0,
                jobs,
            },
        }
    }

    #[test]
    fn test_render_summary_empty_pipeline_types() {
        let insights = CIInsights {
            provider: "GitLab".to_string(),
            project: "test/project".to_string(),
            collected_at: Utc::now(),
            total_pipelines: 0,
            total_pipeline_types: 0,
            pipeline_types: vec![],
        };

        let output = render_summary(&insights);

        assert!(output.contains("test/project"));
        assert!(output.contains("Pipelines analyzed:"));
        assert!(output.contains("No pipeline data found"));
    }

    #[test]
    fn test_render_summary_with_jobs() {
        let jobs = vec![
            create_test_job("slow-job", 1800.0, 10.0, 5.0),
            create_test_job("fast-job", 300.0, 0.0, 0.0),
        ];

        let pipeline_type = create_test_pipeline_type(
            "Development Pipeline",
            50.0,
            85.0,
            600.0,
            jobs,
            "https://gitlab.com/test/project/-/pipelines/123",
        );

        let insights = CIInsights {
            provider: "GitLab".to_string(),
            project: "test/project".to_string(),
            collected_at: Utc::now(),
            total_pipelines: 100,
            total_pipeline_types: 1,
            pipeline_types: vec![pipeline_type],
        };

        let output = render_summary(&insights);

        // Check overview
        assert!(output.contains("test/project"));
        assert!(output.contains("Pipelines analyzed:"));
        assert!(output.contains("Pipeline types:"));

        // Check job tables are present
        assert!(output.contains("Top 10 Slowest Jobs"));
        assert!(output.contains("Top 5 Failing Jobs"));
        assert!(output.contains("Top 5 Flaky Jobs"));

        // Check job names appear
        assert!(output.contains("slow-job"));
        assert!(output.contains("fast-job"));

        // Check pipeline types table
        assert!(output.contains("Pipeline Types (Context)"));
        assert!(output.contains("Development Pipeline"));
        assert!(output.contains("https://gitlab.com/test/project/-/pipelines/123"));

        // Check Next Steps
        assert!(output.contains("Next Steps"));
        assert!(output.contains("--json"));
    }

    #[test]
    fn test_render_summary_deduplicates_jobs_across_pipeline_types() {
        let job1 = create_test_job("same-job", 1000.0, 20.0, 10.0);
        let job2 = create_test_job("same-job", 2000.0, 30.0, 15.0); // Worse metrics

        let pt1 = create_test_pipeline_type(
            "Pipeline A",
            40.0,
            90.0,
            500.0,
            vec![job1],
            "https://example.com/1",
        );

        let pt2 = create_test_pipeline_type(
            "Pipeline B",
            60.0,
            85.0,
            600.0,
            vec![job2],
            "https://example.com/2",
        );

        let insights = CIInsights {
            provider: "GitLab".to_string(),
            project: "test/project".to_string(),
            collected_at: Utc::now(),
            total_pipelines: 200,
            total_pipeline_types: 2,
            pipeline_types: vec![pt1, pt2],
        };

        let output = render_summary(&insights);

        // Job should appear only once in each table
        let job_count = output.matches("same-job").count();
        // Should appear in: slowest (1), failing (1), flaky (1) = 3 times max
        assert!(
            job_count <= 3,
            "Job appears {job_count} times, expected <= 3"
        );
    }

    #[test]
    fn test_render_summary_formats_percentages_correctly() {
        let job = create_test_job("test-job", 600.0, 25.5, 10.3);

        let pipeline_type = create_test_pipeline_type(
            "Test Pipeline",
            33.3,
            87.6,
            500.0,
            vec![job],
            "https://example.com/pipeline",
        );

        let insights = CIInsights {
            provider: "GitLab".to_string(),
            project: "test/project".to_string(),
            collected_at: Utc::now(),
            total_pipelines: 100,
            total_pipeline_types: 1,
            pipeline_types: vec![pipeline_type],
        };

        let output = render_summary(&insights);

        // Check percentage values don't include % sign (it's in the header)
        assert!(output.contains("25.5")); // failure_rate
        assert!(output.contains("10.3")); // flakiness_rate
        assert!(output.contains("33.3")); // pipeline type percentage
        assert!(output.contains("87.6")); // success_rate

        // Verify headers include %
        assert!(output.contains("Fail %"));
        assert!(output.contains("Flaky %"));
        assert!(output.contains("Total %"));
        assert!(output.contains("Success %"));
    }

    #[test]
    fn test_render_summary_formats_time_in_minutes() {
        let job = create_test_job("long-job", 3600.0, 0.0, 0.0); // 60 minutes

        let pipeline_type = create_test_pipeline_type(
            "Test Pipeline",
            100.0,
            100.0,
            7200.0, // 120 minutes
            vec![job],
            "https://example.com/pipeline",
        );

        let insights = CIInsights {
            provider: "GitLab".to_string(),
            project: "test/project".to_string(),
            collected_at: Utc::now(),
            total_pipelines: 100,
            total_pipeline_types: 1,
            pipeline_types: vec![pipeline_type],
        };

        let output = render_summary(&insights);

        // Check times are in minutes with .1 precision
        assert!(output.contains("60.0min"));
        assert!(output.contains("120.0min"));
    }

    #[test]
    fn test_render_summary_includes_example_pipeline_urls() {
        let pipeline_type = create_test_pipeline_type(
            "Test Pipeline",
            100.0,
            95.0,
            500.0,
            vec![],
            "https://gitlab.com/org/repo/-/pipelines/12345",
        );

        let insights = CIInsights {
            provider: "GitLab".to_string(),
            project: "test/project".to_string(),
            collected_at: Utc::now(),
            total_pipelines: 100,
            total_pipeline_types: 1,
            pipeline_types: vec![pipeline_type],
        };

        let output = render_summary(&insights);

        assert!(output.contains("https://gitlab.com/org/repo/-/pipelines/12345"));
        assert!(output.contains("Example")); // Column header
    }

    #[test]
    fn test_render_summary_shows_top_10_slowest_jobs() {
        let jobs: Vec<JobMetrics> = (0..15)
            .map(|i| {
                create_test_job(
                    &format!("slowjob-{i:02}"),
                    f64::from(1000 + i * 100),
                    0.0,
                    0.0,
                )
            })
            .collect();

        let pipeline_type =
            create_test_pipeline_type("Test", 100.0, 100.0, 500.0, jobs, "https://example.com");

        let insights = CIInsights {
            provider: "GitLab".to_string(),
            project: "test/project".to_string(),
            collected_at: Utc::now(),
            total_pipelines: 100,
            total_pipeline_types: 1,
            pipeline_types: vec![pipeline_type],
        };

        let output = render_summary(&insights);

        // Verify slowest jobs section exists and contains expected jobs
        assert!(output.contains("Top 10 Slowest Jobs"));
        assert!(output.contains("slowjob-14")); // Slowest job should appear
        assert!(output.contains("slowjob-13")); // 2nd slowest should appear
        assert!(output.contains("slowjob-05")); // 10th slowest should appear
    }

    #[test]
    fn test_render_summary_shows_top_5_failing_and_flaky_jobs() {
        let jobs: Vec<JobMetrics> = (0..10)
            .map(|i| {
                create_test_job(
                    &format!("job-{i}"),
                    1000.0,
                    f64::from(50 + i * 5), // failure_rate
                    f64::from(10 + i * 2), // flakiness_rate
                )
            })
            .collect();

        let pipeline_type =
            create_test_pipeline_type("Test", 100.0, 100.0, 500.0, jobs, "https://example.com");

        let insights = CIInsights {
            provider: "GitLab".to_string(),
            project: "test/project".to_string(),
            collected_at: Utc::now(),
            total_pipelines: 100,
            total_pipeline_types: 1,
            pipeline_types: vec![pipeline_type],
        };

        let output = render_summary(&insights);

        // Failing jobs section should show top 5
        assert!(output.contains("Top 5 Failing Jobs"));

        // Flaky jobs section should show top 5
        assert!(output.contains("Top 5 Flaky Jobs"));
    }
}
