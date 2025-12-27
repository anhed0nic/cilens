use std::cmp::Ordering;
use std::collections::HashMap;

use super::job_reliability::{calculate_job_reliability, JobReliabilityMetrics};
use super::links::pipeline_id_to_url;
use super::types::GitLabPipeline;
use crate::insights::{
    JobCountWithLinks, JobMetrics, PipelineCountWithLinks, PredecessorJob, TypeMetrics,
};

fn cmp_f64(a: &f64, b: &f64) -> Ordering {
    a.partial_cmp(b).unwrap_or(Ordering::Equal)
}

/// Calculate P50, P95, P99 percentiles from a list of values
/// Returns (p50, p95, p99). If insufficient data, returns same value for all.
fn calculate_percentiles(values: &[f64]) -> (f64, f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0, 0.0);
    }

    let mut sorted = values.to_vec();
    sorted.sort_by(cmp_f64);

    let len = sorted.len();

    // For small datasets, return the same value (best we can do)
    if len == 1 {
        let val = sorted[0];
        return (val, val, val);
    }

    let p50_idx = (len as f64 * 0.50) as usize;
    let p95_idx = (len as f64 * 0.95) as usize;
    let p99_idx = (len as f64 * 0.99) as usize;

    let p50 = sorted[p50_idx.min(len - 1)];
    let p95 = sorted[p95_idx.min(len - 1)];
    let p99 = sorted[p99_idx.min(len - 1)];

    (p50, p95, p99)
}

pub fn calculate_type_metrics(
    pipelines: &[&GitLabPipeline],
    percentage: f64,
    base_url: &str,
    project_path: &str,
) -> TypeMetrics {
    let total_pipelines = pipelines.len();

    let (successful, failed): (Vec<_>, Vec<_>) = pipelines
        .iter()
        .partition(|p| p.status == "success");

    let successful_pipelines = to_pipeline_links(&successful, base_url, project_path);
    let failed_pipelines = to_pipeline_links(&failed, base_url, project_path);

    // Calculate duration percentiles from successful pipelines
    let durations: Vec<f64> = successful.iter().map(|p| p.duration as f64).collect();
    let (duration_p50, duration_p95, duration_p99) = calculate_percentiles(&durations);

    let (jobs, time_to_feedback_percentiles) =
        aggregate_job_metrics(&successful, pipelines, base_url, project_path);

    TypeMetrics {
        percentage,
        total_pipelines,
        successful_pipelines,
        failed_pipelines,
        success_rate: calculate_success_rate(successful.len(), total_pipelines),
        duration_p50,
        duration_p95,
        duration_p99,
        time_to_feedback_p50: time_to_feedback_percentiles.0,
        time_to_feedback_p95: time_to_feedback_percentiles.1,
        time_to_feedback_p99: time_to_feedback_percentiles.2,
        jobs,
    }
}

fn to_pipeline_links(
    pipelines: &[&GitLabPipeline],
    base_url: &str,
    project_path: &str,
) -> PipelineCountWithLinks {
    PipelineCountWithLinks {
        count: pipelines.len(),
        links: pipelines
            .iter()
            .map(|p| pipeline_id_to_url(base_url, project_path, &p.id))
            .collect(),
    }
}

#[allow(clippy::cast_precision_loss)]
fn calculate_success_rate(successful: usize, total: usize) -> f64 {
    (successful as f64 / total.max(1) as f64) * 100.0
}

#[allow(clippy::cast_precision_loss)]
fn aggregate_job_metrics(
    successful_pipelines: &[&GitLabPipeline],
    all_pipelines: &[&GitLabPipeline],
    base_url: &str,
    project_path: &str,
) -> (Vec<JobMetrics>, (f64, f64, f64)) {
    if successful_pipelines.is_empty() {
        return (vec![], (0.0, 0.0, 0.0));
    }

    // Calculate job metrics once per pipeline
    let per_pipeline_metrics: Vec<Vec<JobMetrics>> = successful_pipelines
        .iter()
        .map(|p| super::job_metrics::calculate_job_metrics(p))
        .collect();

    // Calculate pipeline-level time_to_feedback percentiles from per-pipeline data
    let first_feedback_times: Vec<f64> = per_pipeline_metrics
        .iter()
        .filter_map(|pipeline_metrics| {
            pipeline_metrics
                .iter()
                .map(|job| job.time_to_feedback_p50)
                .min_by(cmp_f64)
        })
        .collect();

    let time_to_feedback_percentiles = calculate_percentiles(&first_feedback_times);

    // Aggregate job data across all pipelines
    let mut job_data: HashMap<String, JobData> = HashMap::new();
    for metrics in &per_pipeline_metrics {
        for job_metric in metrics {
            let data = job_data.entry(job_metric.name.clone()).or_default();
            data.durations.push(job_metric.duration_p50);
            data.time_to_feedbacks.push(job_metric.time_to_feedback_p50);
            data.all_predecessor_names.push(
                job_metric
                    .predecessors
                    .iter()
                    .map(|p| p.name.clone())
                    .collect(),
            );
        }
    }

    // Calculate percentiles for all jobs first (needed for predecessor lookups)
    let all_percentiles: HashMap<String, (f64, f64, f64)> = job_data
        .iter()
        .map(|(name, data)| (name.clone(), calculate_percentiles(&data.durations)))
        .collect();

    let reliability_data = calculate_job_reliability(all_pipelines, base_url, project_path);

    let mut jobs: Vec<JobMetrics> = job_data
        .into_iter()
        .map(|(name, data)| build_job_metrics(&name, data, &all_percentiles, &reliability_data))
        .collect();

    jobs.sort_by(|a, b| cmp_f64(&b.time_to_feedback_p95, &a.time_to_feedback_p95));

    (jobs, time_to_feedback_percentiles)
}

#[derive(Default)]
struct JobData {
    durations: Vec<f64>,
    time_to_feedbacks: Vec<f64>,
    all_predecessor_names: Vec<Vec<String>>,
}

fn build_job_metrics(
    name: &str,
    data: JobData,
    all_percentiles: &HashMap<String, (f64, f64, f64)>,
    reliability_data: &HashMap<String, JobReliabilityMetrics>,
) -> JobMetrics {
    let (duration_p50, duration_p95, duration_p99) = calculate_percentiles(&data.durations);
    let (time_to_feedback_p50, time_to_feedback_p95, time_to_feedback_p99) =
        calculate_percentiles(&data.time_to_feedbacks);

    let predecessors = aggregate_predecessors(&data.all_predecessor_names, all_percentiles);

    let (total_executions, flakiness_rate, flaky_retries, failure_rate, failed_executions) =
        match reliability_data.get(name) {
            Some(r) => (
                r.total_executions,
                r.flakiness_rate,
                JobCountWithLinks {
                    count: r.flaky_retries,
                    links: r.flaky_job_links.clone(),
                },
                r.failure_rate,
                JobCountWithLinks {
                    count: r.failed_executions,
                    links: r.failed_job_links.clone(),
                },
            ),
            None => (0, 0.0, Default::default(), 0.0, Default::default()),
        };

    JobMetrics {
        name: name.to_string(),
        duration_p50,
        duration_p95,
        duration_p99,
        time_to_feedback_p50,
        time_to_feedback_p95,
        time_to_feedback_p99,
        predecessors,
        flakiness_rate,
        flaky_retries,
        failed_executions,
        failure_rate,
        total_executions,
    }
}

fn aggregate_predecessors(
    all_predecessor_names: &[Vec<String>],
    all_percentiles: &HashMap<String, (f64, f64, f64)>,
) -> Vec<PredecessorJob> {
    let mut result: Vec<PredecessorJob> = all_predecessor_names
        .iter()
        .flat_map(|names| names.iter())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .filter_map(|name| {
            all_percentiles
                .get(name)
                .map(|(duration_p50, _, _)| PredecessorJob {
                    name: name.clone(),
                    duration_p50: *duration_p50,
                })
        })
        .collect();

    result.sort_by(|a, b| cmp_f64(&b.duration_p50, &a.duration_p50));
    result
}