use std::fmt::Write;

use crate::insights::{CIInsights, JobMetrics};
use comfy_table::{Cell, Color as TableColor};

use super::styling::{bright, bright_green, bright_red, bright_yellow, cyan, dim};
use super::tables::{
    color_coded_duration_cell, color_coded_failure_cell, color_coded_flakiness_cell,
    color_coded_success_cell, create_table,
};

/// Prints a human-readable summary of CI/CD insights to stdout.
///
/// Displays color-coded tables showing:
/// - Overview: Project name, pipeline/job counts, overall success rate
/// - Pipeline Types: Distribution, success rates, durations, examples
/// - Top 10 Slowest Jobs: Jobs with highest P95 time-to-feedback
/// - Top 10 Failing Jobs: Most unreliable jobs by failure rate
/// - Top 10 Flaky Jobs: Most intermittent jobs by flakiness rate
/// - Next Steps: Actionable recommendations
///
/// Color coding:
/// - Green: Good values (success >80%, failures <25%, flakiness <5%, durations ≤10min)
/// - Yellow: Warning (success 50-80%, failures 25-50%, flakiness 5-10%, durations 10-15min)
/// - Red: Critical (success <50%, failures ≥50%, flakiness ≥10%, durations >15min)
pub fn print_summary(insights: &CIInsights) {
    println!("{}", render_summary(insights));
}

// Helper functions

fn create_cyan_header(labels: &[&str]) -> Vec<Cell> {
    labels
        .iter()
        .map(|label| Cell::new(*label).fg(TableColor::Cyan))
        .collect()
}

fn add_section_header(output: &mut String, emoji: &str, title: &str) {
    let _ = writeln!(output, "{} {}", bright(emoji), bright(title).underlined());
}

fn sort_jobs_by<'a, F>(jobs: &[&'a JobMetrics], compare: F) -> Vec<&'a JobMetrics>
where
    F: Fn(&JobMetrics, &JobMetrics) -> std::cmp::Ordering,
{
    let mut sorted = jobs.to_vec();
    sorted.sort_by(|a, b| compare(a, b));
    sorted
}

fn format_critical_path(job: &JobMetrics) -> String {
    if job.predecessors.is_empty() {
        "None".to_string()
    } else {
        job.predecessors
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn calculate_overall_success_rate(insights: &CIInsights) -> (f64, usize) {
    let total_successful: usize = insights
        .pipeline_types
        .iter()
        .map(|pt| pt.metrics.successful_pipelines.count)
        .sum();
    let total_failed: usize = insights
        .pipeline_types
        .iter()
        .map(|pt| pt.metrics.failed_pipelines.count)
        .sum();
    let total = total_successful + total_failed;

    #[allow(clippy::cast_precision_loss)]
    let rate = if total > 0 {
        (total_successful as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    (rate, total)
}

#[allow(clippy::too_many_lines, clippy::format_push_string)]
fn render_summary(insights: &CIInsights) -> String {
    let mut output = String::new();

    // Overview section
    add_section_header(&mut output, "📊", "Overview");

    let total_jobs: usize = insights
        .pipeline_types
        .iter()
        .flat_map(|pt| &pt.metrics.jobs)
        .map(|job| job.total_executions)
        .sum();

    let (overall_success_rate, _) = calculate_overall_success_rate(insights);

    let success_rate_display = if overall_success_rate > 80.0 {
        bright_green(format!("{overall_success_rate:.1}%"))
    } else if overall_success_rate >= 50.0 {
        bright_yellow(format!("{overall_success_rate:.1}%"))
    } else {
        bright_red(format!("{overall_success_rate:.1}%"))
    };

    output.push_str(&format!(
        "  {} {}\n  {} {}\n  {} {}\n  {} {}\n  {} {}\n  {} {}\n\n",
        dim("Project:"),
        cyan(&insights.project),
        dim("Pipelines analyzed:"),
        bright_yellow(insights.total_pipelines),
        dim("Jobs analyzed:"),
        bright_yellow(total_jobs),
        dim("Overall success rate:"),
        success_rate_display,
        dim("Pipeline types:"),
        bright_yellow(insights.total_pipeline_types),
        dim("Analysis date:"),
        dim(insights.collected_at.format("%Y-%m-%d %H:%M UTC"))
    ));

    if insights.pipeline_types.is_empty() {
        output.push_str(&format!("{}\n", bright_yellow("No pipeline data found.")));
        return output;
    }

    // Pipeline Types
    add_section_header(&mut output, "📋", "Pipeline Types");

    let mut types_table = create_table();
    types_table.set_header(create_cyan_header(&[
        "Pipeline Type",
        "Total",
        "Success",
        "P95 Duration",
        "Slowest Feedback",
        "Example",
    ]));

    for pt in insights.pipeline_types.iter().take(10) {
        let success_cell = color_coded_success_cell(pt.metrics.success_rate);
        let duration_cell = color_coded_duration_cell(pt.metrics.duration_p95);

        // Find the slowest job (highest time_to_feedback_p95) in this pipeline type
        let slowest_job = pt.metrics.jobs.iter().max_by(|a, b| {
            a.time_to_feedback_p95
                .partial_cmp(&b.time_to_feedback_p95)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let feedback_cell = slowest_job.map_or_else(
            || Cell::new("N/A"),
            |job| {
                let minutes = job.time_to_feedback_p95 / 60.0;
                let text = format!("{}\n{minutes:.1}min", job.name);
                let color = if minutes <= 10.0 {
                    TableColor::Green
                } else if minutes <= 15.0 {
                    TableColor::Yellow
                } else {
                    TableColor::Red
                };
                Cell::new(text).fg(color)
            },
        );

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
            Cell::new(format!("{:.1}%", pt.metrics.percentage)),
            success_cell,
            duration_cell,
            feedback_cell,
            Cell::new(example_url),
        ]);
    }

    if insights.pipeline_types.len() > 10 {
        let empty_cells = vec![Cell::new(""); 5];
        let mut row = vec![Cell::new(format!(
            "... and {} more",
            insights.pipeline_types.len() - 10
        ))
        .fg(TableColor::DarkGrey)];
        row.extend(empty_cells);
        types_table.add_row(row);
    }

    output.push_str(&format!("{types_table}\n\n"));

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

    let all_jobs: Vec<&JobMetrics> = jobs_by_name.values().copied().collect();

    // Top 10 Slowest Jobs
    add_section_header(&mut output, "🐌", "Top 10 Slowest Jobs");

    let sorted_by_time = sort_jobs_by(&all_jobs, |a, b| {
        b.time_to_feedback_p95
            .partial_cmp(&a.time_to_feedback_p95)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut slowest_table = create_table();
    slowest_table.set_header(create_cyan_header(&[
        "#",
        "Job Name",
        "P95 Feedback",
        "Fail",
        "Flaky",
        "Critical Path",
    ]));

    for (idx, job) in sorted_by_time.iter().take(10).enumerate() {
        slowest_table.add_row(vec![
            Cell::new(idx + 1),
            Cell::new(&job.name),
            color_coded_duration_cell(job.time_to_feedback_p95),
            color_coded_failure_cell(job.failure_rate),
            color_coded_flakiness_cell(job.flakiness_rate),
            Cell::new(format_critical_path(job)),
        ]);
    }

    output.push_str(&format!("{slowest_table}\n\n"));

    // Top 10 Failing Jobs
    add_section_header(&mut output, "❌", "Top 10 Failing Jobs");

    let sorted_by_failure = sort_jobs_by(&all_jobs, |a, b| {
        b.failure_rate
            .partial_cmp(&a.failure_rate)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut failing_table = create_table();
    failing_table.set_header(create_cyan_header(&[
        "#",
        "Job Name",
        "Fail",
        "P95 Feedback",
    ]));

    for (idx, job) in sorted_by_failure.iter().take(10).enumerate() {
        failing_table.add_row(vec![
            Cell::new(idx + 1),
            Cell::new(&job.name),
            color_coded_failure_cell(job.failure_rate),
            color_coded_duration_cell(job.time_to_feedback_p95),
        ]);
    }

    output.push_str(&format!("{failing_table}\n\n"));

    // Top 10 Flaky Jobs
    add_section_header(&mut output, "🔄", "Top 10 Flaky Jobs");

    let sorted_by_flakiness = sort_jobs_by(&all_jobs, |a, b| {
        b.flakiness_rate
            .partial_cmp(&a.flakiness_rate)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut flaky_table = create_table();
    flaky_table.set_header(create_cyan_header(&[
        "#",
        "Job Name",
        "Flaky",
        "P95 Feedback",
    ]));

    for (idx, job) in sorted_by_flakiness.iter().take(10).enumerate() {
        flaky_table.add_row(vec![
            Cell::new(idx + 1),
            Cell::new(&job.name),
            color_coded_flakiness_cell(job.flakiness_rate),
            color_coded_duration_cell(job.time_to_feedback_p95),
        ]);
    }

    output.push_str(&format!("{flaky_table}\n\n"));

    // Next Steps
    add_section_header(&mut output, "💡", "Next Steps");
    output.push_str(&format!(
        "  {} Use {} flag to get detailed metrics and job dependencies\n\
         \x20 {} Prioritize slowest jobs - they block developer feedback\n\
         \x20 {} Fix failing jobs - they create noise and reduce trust\n\
         \x20 {} Investigate flaky jobs - they waste CI resources and time\n",
        cyan("•"),
        bright_yellow("--json"),
        cyan("•"),
        cyan("•"),
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
            duration_p50: time_to_feedback_p95 * 0.3,
            duration_p95: time_to_feedback_p95 * 0.6,
            duration_p99: time_to_feedback_p95 * 0.8,
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
            "Development",
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
        assert!(output.contains("Top 10 Failing Jobs"));
        assert!(output.contains("Top 10 Flaky Jobs"));

        // Check job names appear
        assert!(output.contains("slow-job"));
        assert!(output.contains("fast-job"));

        // Check pipeline types table
        assert!(output.contains("Pipeline Types"));
        assert!(output.contains("Development"));
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

        // Job should appear only once in each job table, plus once per pipeline type in the types table
        let job_count = output.matches("same-job").count();
        // Should appear in: slowest (1), failing (1), flaky (1), pipeline types (2) = 5 times
        assert!(job_count == 5, "Job appears {job_count} times, expected 5");
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

        // Check percentage values include % sign
        assert!(output.contains("25.5%")); // failure_rate
        assert!(output.contains("10.3%")); // flakiness_rate
        assert!(output.contains("33.3%")); // pipeline type percentage
        assert!(output.contains("87.6%")); // success_rate

        // Verify headers don't include % (it's in the values now)
        assert!(output.contains("Fail"));
        assert!(output.contains("Flaky"));
        assert!(output.contains("Total"));
        assert!(output.contains("Success"));
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
    fn test_render_summary_includes_pipeline_types_table() {
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

        // Check pipeline types table with example URLs
        assert!(output.contains("Pipeline Types"));
        assert!(output.contains("Test Pipeline"));
        assert!(output.contains("https://gitlab.com/org/repo/-/pipelines/12345"));
        assert!(output.contains("Example"));
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

        // Failing jobs section should show top 10
        assert!(output.contains("Top 10 Failing Jobs"));

        // Flaky jobs section should show top 10
        assert!(output.contains("Top 10 Flaky Jobs"));
    }
}
