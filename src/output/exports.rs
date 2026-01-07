use anyhow::Result;
use std::io::Write;

use crate::config::OutputFormat;
use crate::insights::CIInsights;

/// Exports CI insights to various formats.
///
/// Supports multiple output formats for different use cases:
/// - CSV: Spreadsheet analysis and reporting
/// - HTML: Self-contained reports with formatting
/// - JSON: Programmatic access (already supported)
/// - Summary: Human-readable terminal output (already supported)
pub fn export_insights(
    insights: &CIInsights,
    format: OutputFormat,
    pretty: bool,
    output: &mut dyn Write,
) -> Result<()> {
    match format {
        OutputFormat::Summary => {
            // Summary format is handled separately in cli.rs
            unreachable!("Summary format should be handled in CLI")
        }
        OutputFormat::Json => export_json(insights, pretty, output),
        OutputFormat::Csv => export_csv(insights, output),
        OutputFormat::Html => export_html(insights, output),
    }
}

fn export_json(insights: &CIInsights, pretty: bool, output: &mut dyn Write) -> Result<()> {
    let json = if pretty {
        serde_json::to_string_pretty(insights)?
    } else {
        serde_json::to_string(insights)?
    };
    writeln!(output, "{}", json)?;
    Ok(())
}

fn export_csv(insights: &CIInsights, output: &mut dyn Write) -> Result<()> {
    // Write CSV header
    writeln!(output, "Pipeline Type,Percentage,Total Pipelines,Success Rate,Duration P50,Duration P95,Duration P99,Time to Feedback P50,Time to Feedback P95,Time to Feedback P99,Cost per Pipeline,Total Cost")?;

    // Write pipeline type data
    for pipeline_type in &insights.pipeline_types {
        let metrics = &pipeline_type.metrics;
        writeln!(
            output,
            "\"{}\",{:.1},{},{:.1},{:.1},{:.1},{:.1},{:.1},{:.1},{:.1},{:.2},{:.2}",
            pipeline_type.label,
            metrics.percentage,
            metrics.total_pipelines,
            metrics.success_rate,
            metrics.duration_p50,
            metrics.duration_p95,
            metrics.duration_p99,
            metrics.time_to_feedback_p50,
            metrics.time_to_feedback_p95,
            metrics.time_to_feedback_p99,
            metrics.cost_per_pipeline.unwrap_or(0.0),
            metrics.total_cost.unwrap_or(0.0)
        )?;
    }

    // Write job data header
    writeln!(output)?;
    writeln!(output, "Job Name,Pipeline Type,Duration P50,Duration P95,Duration P99,Time to Feedback P50,Time to Feedback P95,Time to Feedback P99,Flakiness Rate,Failure Rate,Total Executions,Cost per Execution,Total Cost")?;

    // Write job data
    for pipeline_type in &insights.pipeline_types {
        for job in &pipeline_type.metrics.jobs {
            writeln!(
                output,
                "\"{}\",\"{}\",{:.1},{:.1},{:.1},{:.1},{:.1},{:.1},{:.1},{:.1},{},{:.2},{:.2}",
                job.name,
                pipeline_type.label,
                job.duration_p50,
                job.duration_p95,
                job.duration_p99,
                job.time_to_feedback_p50,
                job.time_to_feedback_p95,
                job.time_to_feedback_p99,
                job.flakiness_rate,
                job.failure_rate,
                job.total_executions,
                job.cost_per_execution.unwrap_or(0.0),
                job.total_cost.unwrap_or(0.0)
            )?;
        }
    }

    Ok(())
}

fn export_html(insights: &CIInsights, output: &mut dyn Write) -> Result<()> {
    writeln!(output, "<!DOCTYPE html>")?;
    writeln!(output, "<html lang=\"en\">")?;
    writeln!(output, "<head>")?;
    writeln!(output, "    <meta charset=\"UTF-8\">")?;
    writeln!(output, "    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">")?;
    writeln!(output, "    <title>CILens Report - {}</title>", insights.project)?;
    writeln!(output, "    <style>")?;
    writeln!(output, "        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; margin: 40px; background: #f5f5f5; }}")?;
    writeln!(output, "        .container {{ max-width: 1200px; margin: 0 auto; background: white; padding: 30px; border-radius: 8px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }}")?;
    writeln!(output, "        h1 {{ color: #2c3e50; border-bottom: 3px solid #3498db; padding-bottom: 10px; }}")?;
    writeln!(output, "        h2 {{ color: #34495e; margin-top: 30px; }}")?;
    writeln!(output, "        .summary {{ background: #ecf0f1; padding: 20px; border-radius: 5px; margin: 20px 0; }}")?;
    writeln!(output, "        table {{ width: 100%; border-collapse: collapse; margin: 20px 0; }}")?;
    writeln!(output, "        th, td {{ padding: 12px; text-align: left; border-bottom: 1px solid #ddd; }}")?;
    writeln!(output, "        th {{ background: #3498db; color: white; }}")?;
    writeln!(output, "        tr:nth-child(even) {{ background: #f8f9fa; }}")?;
    writeln!(output, "        .good {{ color: #27ae60; }}")?;
    writeln!(output, "        .warning {{ color: #f39c12; }}")?;
    writeln!(output, "        .bad {{ color: #e74c3c; }}")?;
    writeln!(output, "        .metric {{ font-weight: bold; }}")?;
    writeln!(output, "    </style>")?;
    writeln!(output, "</head>")?;
    writeln!(output, "<body>")?;
    writeln!(output, "    <div class=\"container\">")?;
    writeln!(output, "        <h1>🔍 CILens CI/CD Insights Report</h1>")?;
    writeln!(output, "        <div class=\"summary\">")?;
    writeln!(output, "            <h2>Project Summary</h2>")?;
    writeln!(output, "            <p><strong>Project:</strong> {}</p>", insights.project)?;
    writeln!(output, "            <p><strong>Provider:</strong> {}</p>", insights.provider)?;
    writeln!(output, "            <p><strong>Analysis Date:</strong> {}</p>", insights.collected_at.format("%Y-%m-%d %H:%M UTC"))?;
    writeln!(output, "            <p><strong>Total Pipelines:</strong> {}</p>", insights.total_pipelines)?;
    writeln!(output, "            <p><strong>Pipeline Types:</strong> {}</p>", insights.total_pipeline_types)?;
    writeln!(output, "        </div>")?;

    // Pipeline Types Table
    writeln!(output, "        <h2>Pipeline Types</h2>")?;
    writeln!(output, "        <table>")?;
    writeln!(output, "            <thead>")?;
    writeln!(output, "                <tr>")?;
    writeln!(output, "                    <th>Type</th>")?;
    writeln!(output, "                    <th>Percentage</th>")?;
    writeln!(output, "                    <th>Total</th>")?;
    writeln!(output, "                    <th>Success Rate</th>")?;
    writeln!(output, "                    <th>P95 Duration</th>")?;
    if insights.pipeline_types.iter().any(|pt| pt.metrics.cost_per_pipeline.is_some()) {
        writeln!(output, "                    <th>Cost/Pipeline</th>")?;
        writeln!(output, "                    <th>Total Cost</th>")?;
    }
    writeln!(output, "                </tr>")?;
    writeln!(output, "            </thead>")?;
    writeln!(output, "            <tbody>")?;

    for pipeline_type in &insights.pipeline_types {
        let metrics = &pipeline_type.metrics;
        let success_class = if metrics.success_rate >= 80.0 { "good" } else if metrics.success_rate >= 50.0 { "warning" } else { "bad" };
        writeln!(output, "                <tr>")?;
        writeln!(output, "                    <td>{}</td>", pipeline_type.label)?;
        writeln!(output, "                    <td>{:.1}%</td>", metrics.percentage)?;
        writeln!(output, "                    <td>{}</td>", metrics.total_pipelines)?;
        writeln!(output, "                    <td class=\"{}\">{:.1}%</td>", success_class, metrics.success_rate)?;
        writeln!(output, "                    <td>{:.1}s</td>", metrics.duration_p95)?;
        if let (Some(cost_per), Some(total_cost)) = (metrics.cost_per_pipeline, metrics.total_cost) {
            writeln!(output, "                    <td>${:.2}</td>", cost_per)?;
            writeln!(output, "                    <td>${:.2}</td>", total_cost)?;
        }
        writeln!(output, "                </tr>")?;
    }
    writeln!(output, "            </tbody>")?;
    writeln!(output, "        </table>")?;

    // Jobs Table
    writeln!(output, "        <h2>Job Performance</h2>")?;
    writeln!(output, "        <table>")?;
    writeln!(output, "            <thead>")?;
    writeln!(output, "                <tr>")?;
    writeln!(output, "                    <th>Job Name</th>")?;
    writeln!(output, "                    <th>Pipeline Type</th>")?;
    writeln!(output, "                    <th>P95 Duration</th>")?;
    writeln!(output, "                    <th>P95 Time to Feedback</th>")?;
    writeln!(output, "                    <th>Failure Rate</th>")?;
    writeln!(output, "                    <th>Flakiness Rate</th>")?;
    if insights.pipeline_types.iter().any(|pt| pt.metrics.jobs.iter().any(|j| j.cost_per_execution.is_some())) {
        writeln!(output, "                    <th>Cost/Execution</th>")?;
        writeln!(output, "                    <th>Total Cost</th>")?;
    }
    writeln!(output, "                </tr>")?;
    writeln!(output, "            </thead>")?;
    writeln!(output, "            <tbody>")?;

    for pipeline_type in &insights.pipeline_types {
        for job in &pipeline_type.metrics.jobs {
            let failure_class = if job.failure_rate <= 25.0 { "good" } else if job.failure_rate <= 50.0 { "warning" } else { "bad" };
            let flakiness_class = if job.flakiness_rate <= 5.0 { "good" } else if job.flakiness_rate <= 15.0 { "warning" } else { "bad" };
            writeln!(output, "                <tr>")?;
            writeln!(output, "                    <td>{}</td>", job.name)?;
            writeln!(output, "                    <td>{}</td>", pipeline_type.label)?;
            writeln!(output, "                    <td>{:.1}s</td>", job.duration_p95)?;
            writeln!(output, "                    <td>{:.1}s</td>", job.time_to_feedback_p95)?;
            writeln!(output, "                    <td class=\"{}\">{:.1}%</td>", failure_class, job.failure_rate)?;
            writeln!(output, "                    <td class=\"{}\">{:.1}%</td>", flakiness_class, job.flakiness_rate)?;
            if let (Some(cost_per), Some(total_cost)) = (job.cost_per_execution, job.total_cost) {
                writeln!(output, "                    <td>${:.2}</td>", cost_per)?;
                writeln!(output, "                    <td>${:.2}</td>", total_cost)?;
            }
            writeln!(output, "                </tr>")?;
        }
    }
    writeln!(output, "            </tbody>")?;
    writeln!(output, "        </table>")?;

    writeln!(output, "        <footer style=\"margin-top: 40px; padding-top: 20px; border-top: 1px solid #ddd; color: #666; text-align: center;\">")?;
    writeln!(output, "            <p>Report generated by CILens v{} on {}</p>", env!("CARGO_PKG_VERSION"), insights.collected_at.format("%Y-%m-%d %H:%M UTC"))?;
    writeln!(output, "        </footer>")?;
    writeln!(output, "    </div>")?;
    writeln!(output, "</body>")?;
    writeln!(output, "</html>")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::insights::{CIInsights, JobMetrics, PipelineType, TypeMetrics};
    use chrono::Utc;

    #[test]
    fn test_export_json() {
        let insights = create_test_insights();
        let mut output = Vec::new();
        export_json(&insights, false, &mut output).unwrap();
        let json_str = String::from_utf8(output).unwrap();
        assert!(json_str.contains("GitLab"));
        assert!(json_str.contains("test/project"));
        assert!(json_str.contains("test-job"));
    }

    #[test]
    fn test_export_json_pretty() {
        let insights = create_test_insights();
        let mut output = Vec::new();
        export_json(&insights, true, &mut output).unwrap();
        let json_str = String::from_utf8(output).unwrap();
        assert!(json_str.contains('\n'));
        assert!(json_str.contains("  "));
    }

    #[test]
    fn test_export_csv_with_costs() {
        let insights = create_test_insights();
        let mut output = Vec::new();
        export_csv(&insights, &mut output).unwrap();
        let csv = String::from_utf8(output).unwrap();
        assert!(csv.contains("Cost per Pipeline"));
        assert!(csv.contains("$0.25"));
        assert!(csv.contains("$0.05"));
    }

    #[test]
    fn test_export_html_structure() {
        let insights = create_test_insights();
        let mut output = Vec::new();
        export_html(&insights, &mut output).unwrap();
        let html = String::from_utf8(output).unwrap();
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("<table>"));
        assert!(html.contains("</html>"));
        assert!(html.contains("CILens"));
        assert!(html.contains("test/project"));
    }

    #[test]
    fn test_export_html_with_costs() {
        let insights = create_test_insights();
        let mut output = Vec::new();
        export_html(&insights, &mut output).unwrap();
        let html = String::from_utf8(output).unwrap();
        assert!(html.contains("$0.25"));
        assert!(html.contains("$0.05"));
    }

    fn create_test_insights() -> CIInsights {
        let job_metrics = JobMetrics {
            name: "test-job".to_string(),
            duration_p50: 60.0,
            duration_p95: 120.0,
            duration_p99: 180.0,
            time_to_feedback_p50: 120.0,
            time_to_feedback_p95: 240.0,
            time_to_feedback_p99: 360.0,
            predecessors: vec![],
            flakiness_rate: 5.0,
            flaky_retries: Default::default(),
            failed_executions: Default::default(),
            failure_rate: 10.0,
            total_executions: 100,
            cost_per_execution: Some(0.05),
            total_cost: Some(5.0),
        };

        let type_metrics = TypeMetrics {
            percentage: 100.0,
            total_pipelines: 50,
            successful_pipelines: Default::default(),
            failed_pipelines: Default::default(),
            success_rate: 90.0,
            duration_p50: 300.0,
            duration_p95: 600.0,
            duration_p99: 900.0,
            time_to_feedback_p50: 600.0,
            time_to_feedback_p95: 1200.0,
            time_to_feedback_p99: 1800.0,
            jobs: vec![job_metrics],
            cost_per_pipeline: Some(0.25),
            total_cost: Some(12.5),
        };

        let pipeline_type = PipelineType {
            label: "Test Pipeline".to_string(),
            stages: vec!["build".to_string(), "test".to_string()],
            ref_patterns: vec!["main".to_string()],
            sources: vec!["push".to_string()],
            metrics: type_metrics,
        };

        CIInsights {
            provider: "GitLab".to_string(),
            project: "test/project".to_string(),
            collected_at: Utc::now(),
            total_pipelines: 50,
            total_pipeline_types: 1,
            pipeline_types: vec![pipeline_type],
        }
    }
}