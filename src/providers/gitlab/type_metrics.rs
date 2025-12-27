use std::collections::HashMap;

use super::types::{GitLabJob, GitLabPipeline};
use crate::insights::{JobMetrics, PredecessorJob, TypeMetrics};

pub fn calculate_type_metrics(pipelines: &[&GitLabPipeline], percentage: f64) -> TypeMetrics {
    let total_pipelines = pipelines.len();
    let successful: Vec<_> = pipelines
        .iter()
        .filter(|p| p.status == "success")
        .copied()
        .collect();

    let failed = pipelines.iter().filter(|p| p.status == "failed").count();

    let (jobs, avg_time_to_feedback_seconds) = calculate_all_job_metrics(&successful, pipelines);

    TypeMetrics {
        percentage,
        total_pipelines,
        successful_pipelines: successful.len(),
        failed_pipelines: failed,
        success_rate: calculate_success_rate(successful.len(), total_pipelines),
        avg_duration_seconds: calculate_avg_duration(&successful),
        avg_time_to_feedback_seconds,
        jobs,
    }
}

fn calculate_success_rate(successful: usize, total: usize) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let rate = (successful as f64 / total.max(1) as f64) * 100.0;
    rate
}

fn calculate_avg_duration(pipelines: &[&GitLabPipeline]) -> f64 {
    if pipelines.is_empty() {
        return 0.0;
    }

    #[allow(clippy::cast_precision_loss)]
    let avg = pipelines.iter().map(|p| p.duration as f64).sum::<f64>() / pipelines.len() as f64;
    avg
}

#[allow(clippy::cast_precision_loss)]
fn calculate_pipeline_time_to_feedback(all_metrics: &[Vec<JobMetrics>]) -> f64 {
    if all_metrics.is_empty() {
        return 0.0;
    }

    let first_feedback_times: Vec<f64> = all_metrics
        .iter()
        .filter_map(|pipeline_metrics| {
            pipeline_metrics
                .iter()
                .map(|job| job.avg_time_to_feedback_seconds)
                .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        })
        .collect();

    if first_feedback_times.is_empty() {
        return 0.0;
    }

    first_feedback_times.iter().sum::<f64>() / first_feedback_times.len() as f64
}

fn calculate_all_job_metrics(
    successful_pipelines: &[&GitLabPipeline],
    all_pipelines: &[&GitLabPipeline],
) -> (Vec<JobMetrics>, f64) {
    if successful_pipelines.is_empty() {
        return (vec![], 0.0);
    }

    let all_metrics: Vec<Vec<JobMetrics>> = successful_pipelines
        .iter()
        .map(|p| super::job_analysis::calculate_job_metrics(p))
        .collect();

    // Calculate pipeline-level avg_time_to_feedback from per-pipeline data
    let avg_time_to_feedback = calculate_pipeline_time_to_feedback(&all_metrics);

    // Aggregate job data across all pipelines
    let mut job_data: HashMap<String, JobData> = HashMap::new();
    for metrics in &all_metrics {
        for job_metric in metrics {
            let data = job_data
                .entry(job_metric.name.clone())
                .or_insert_with(JobData::new);
            data.durations.push(job_metric.avg_duration_seconds);
            data.total_durations
                .push(job_metric.avg_time_to_feedback_seconds);
            let predecessor_names = job_metric.predecessors.iter().map(|p| p.name.clone()).collect();
            data.all_predecessor_names.push(predecessor_names);
        }
    }

    let avg_durations = compute_avg_durations(&job_data);
    let (execution_counts, reliability_data) = calculate_job_reliability(all_pipelines);

    let mut jobs: Vec<JobMetrics> = job_data
        .into_iter()
        .map(|(name, data)| {
            build_job_metrics(&name, &data, &avg_durations, &execution_counts, &reliability_data)
        })
        .collect();

    jobs.sort_by(|a, b| {
        b.avg_time_to_feedback_seconds
            .partial_cmp(&a.avg_time_to_feedback_seconds)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    (jobs, avg_time_to_feedback)
}

struct JobData {
    durations: Vec<f64>,
    total_durations: Vec<f64>,
    all_predecessor_names: Vec<Vec<String>>,
}

impl JobData {
    fn new() -> Self {
        Self {
            durations: vec![],
            total_durations: vec![],
            all_predecessor_names: vec![],
        }
    }
}

fn compute_avg_durations(job_data: &HashMap<String, JobData>) -> HashMap<String, f64> {
    job_data
        .iter()
        .map(|(name, data)| (name.clone(), compute_mean(&data.durations)))
        .collect()
}

#[allow(clippy::cast_precision_loss)]
fn compute_mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

fn build_job_metrics(
    name: &str,
    data: &JobData,
    avg_durations: &HashMap<String, f64>,
    execution_counts: &HashMap<String, usize>,
    reliability_data: &HashMap<String, JobReliabilityMetrics>,
) -> JobMetrics {
    let avg_duration_seconds = *avg_durations.get(name).unwrap_or(&0.0);
    let avg_time_to_feedback_seconds = compute_mean(&data.total_durations);
    let predecessors = aggregate_predecessors(&data.all_predecessor_names, avg_durations);
    let total_executions = *execution_counts.get(name).unwrap_or(&0);
    let (flakiness_rate, flaky_retries, failure_rate, failed_executions) = reliability_data
        .get(name)
        .map_or((0.0, 0, 0.0, 0), |r| {
            (
                r.flakiness_rate,
                r.flaky_retries,
                r.failure_rate,
                r.failed_executions,
            )
        });

    JobMetrics {
        name: name.to_string(),
        avg_duration_seconds,
        avg_time_to_feedback_seconds,
        predecessors,
        flakiness_rate,
        flaky_retries,
        failed_executions,
        failure_rate,
        total_executions,
    }
}

struct JobReliabilityMetrics {
    flakiness_rate: f64,
    flaky_retries: usize,
    failure_rate: f64,
    failed_executions: usize,
}

fn aggregate_predecessors(
    all_predecessor_names: &[Vec<String>],
    avg_durations: &HashMap<String, f64>,
) -> Vec<PredecessorJob> {
    if all_predecessor_names.is_empty() {
        return vec![];
    }

    let predecessor_names: std::collections::HashSet<String> = all_predecessor_names
        .iter()
        .flat_map(|names| names.iter())
        .cloned()
        .collect();

    let mut result: Vec<PredecessorJob> = predecessor_names
        .into_iter()
        .filter_map(|name| create_predecessor_job(name, avg_durations))
        .collect();

    result.sort_by(|a, b| {
        b.avg_duration_seconds
            .partial_cmp(&a.avg_duration_seconds)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    result
}

fn create_predecessor_job(
    name: String,
    avg_durations: &HashMap<String, f64>,
) -> Option<PredecessorJob> {
    avg_durations.get(&name).map(|&avg_duration_seconds| {
        PredecessorJob {
            name,
            avg_duration_seconds,
        }
    })
}

fn calculate_job_reliability(
    pipelines: &[&GitLabPipeline],
) -> (HashMap<String, usize>, HashMap<String, JobReliabilityMetrics>) {
    let mut execution_counts: HashMap<String, usize> = HashMap::new();
    let mut flaky_retries: HashMap<String, usize> = HashMap::new();
    let mut failed_executions: HashMap<String, usize> = HashMap::new();

    for pipeline in pipelines {
        let jobs_by_name = group_jobs_by_name(&pipeline.jobs);

        for (name, jobs) in jobs_by_name {
            *execution_counts.entry(name.to_string()).or_insert(0) += jobs.len();

            if is_job_flaky(&jobs) {
                let retries = count_retries(&jobs);
                *flaky_retries.entry(name.to_string()).or_insert(0) += retries;
            } else if is_job_failed(&jobs) {
                *failed_executions.entry(name.to_string()).or_insert(0) += 1;
            }
        }
    }

    let reliability_data = compute_reliability_metrics(
        &flaky_retries,
        &failed_executions,
        &execution_counts,
    );

    (execution_counts, reliability_data)
}

fn count_retries(jobs: &[&GitLabJob]) -> usize {
    jobs.iter().filter(|j| j.retried).count()
}

fn compute_reliability_metrics(
    retry_counts: &HashMap<String, usize>,
    failure_counts: &HashMap<String, usize>,
    execution_counts: &HashMap<String, usize>,
) -> HashMap<String, JobReliabilityMetrics> {
    // Collect all job names that have either flaky retries or failures
    let mut all_job_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    all_job_names.extend(retry_counts.keys().cloned());
    all_job_names.extend(failure_counts.keys().cloned());

    all_job_names
        .into_iter()
        .filter_map(|name| {
            create_reliability_metric(
                name,
                retry_counts,
                failure_counts,
                execution_counts,
            )
        })
        .collect()
}

#[allow(clippy::cast_precision_loss)]
fn create_reliability_metric(
    name: String,
    retry_counts: &HashMap<String, usize>,
    failure_counts: &HashMap<String, usize>,
    execution_counts: &HashMap<String, usize>,
) -> Option<(String, JobReliabilityMetrics)> {
    let flaky_retries = *retry_counts.get(&name).unwrap_or(&0);
    let failed_executions = *failure_counts.get(&name).unwrap_or(&0);

    // Skip if both are zero
    if flaky_retries == 0 && failed_executions == 0 {
        return None;
    }

    let total_executions = *execution_counts.get(&name)?;
    let flakiness_rate = (flaky_retries as f64 / total_executions as f64) * 100.0;
    let failure_rate = (failed_executions as f64 / total_executions as f64) * 100.0;

    Some((
        name,
        JobReliabilityMetrics {
            flakiness_rate,
            flaky_retries,
            failure_rate,
            failed_executions,
        },
    ))
}

fn group_jobs_by_name(jobs: &[GitLabJob]) -> HashMap<&str, Vec<&GitLabJob>> {
    jobs.iter().fold(HashMap::new(), |mut grouped, job| {
        grouped
            .entry(job.name.as_str())
            .or_insert_with(Vec::new)
            .push(job);
        grouped
    })
}

fn is_job_flaky(jobs: &[&GitLabJob]) -> bool {
    // Flaky = job was retried AND eventually succeeded
    let was_retried = jobs.iter().any(|j| j.retried);
    let final_succeeded = jobs
        .iter()
        .find(|j| !j.retried)
        .is_some_and(|j| j.status == "SUCCESS");

    was_retried && final_succeeded
}

fn is_job_failed(jobs: &[&GitLabJob]) -> bool {
    // Failed = job did not eventually succeed (opposite of flaky)
    // A job failed if there's no successful non-retried job
    jobs.iter()
        .find(|j| !j.retried)
        .is_none_or(|j| j.status != "SUCCESS")
}
