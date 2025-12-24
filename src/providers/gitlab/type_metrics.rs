use std::collections::HashMap;

use super::types::{GitLabJob, GitLabPipeline};
use crate::insights::{JobMetrics, PredecessorJob, TypeMetrics};

pub fn calculate_type_metrics(pipelines: &[&GitLabPipeline]) -> TypeMetrics {
    let total_pipelines = pipelines.len();
    let successful: Vec<_> = pipelines
        .iter()
        .filter(|p| p.status == "success")
        .copied()
        .collect();

    let failed = pipelines.iter().filter(|p| p.status == "failed").count();

    let jobs = calculate_all_job_metrics(&successful, pipelines);

    TypeMetrics {
        total_pipelines,
        successful_pipelines: successful.len(),
        failed_pipelines: failed,
        success_rate: calculate_success_rate(successful.len(), total_pipelines),
        average_duration_seconds: calculate_avg_duration(&successful),
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

fn calculate_all_job_metrics(
    successful_pipelines: &[&GitLabPipeline],
    all_pipelines: &[&GitLabPipeline],
) -> Vec<JobMetrics> {
    if successful_pipelines.is_empty() {
        return vec![];
    }

    // Get per-pipeline job metrics (duration, time to feedback, predecessors)
    let all_metrics: Vec<Vec<JobMetrics>> = successful_pipelines
        .iter()
        .map(|p| super::job_analysis::calculate_job_metrics(p))
        .collect();

    // Collect all raw data by job name
    let mut job_data: HashMap<String, JobData> = HashMap::new();

    for metrics in &all_metrics {
        for job_metric in metrics {
            let data = job_data
                .entry(job_metric.name.clone())
                .or_insert_with(JobData::new);

            data.durations.push(job_metric.avg_duration_seconds);
            data.total_durations
                .push(job_metric.avg_time_to_feedback_seconds);
            data.all_predecessors.push(job_metric.predecessors.clone());
        }
    }

    // Get flakiness data from ALL pipelines (not just successful)
    let flaky_data = calculate_flakiness(all_pipelines);

    // Calculate complete metrics for each job
    let mut jobs: Vec<JobMetrics> = job_data
        .into_iter()
        .map(|(name, data)| calculate_job_metrics(&name, &data, &flaky_data))
        .collect();

    // Sort by time to feedback descending (longest time-to-feedback first)
    jobs.sort_by(|a, b| {
        b.avg_time_to_feedback_seconds
            .partial_cmp(&a.avg_time_to_feedback_seconds)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    jobs
}

struct JobData {
    durations: Vec<f64>,
    total_durations: Vec<f64>,
    all_predecessors: Vec<Vec<PredecessorJob>>,
}

impl JobData {
    fn new() -> Self {
        Self {
            durations: vec![],
            total_durations: vec![],
            all_predecessors: vec![],
        }
    }
}

fn calculate_job_metrics(
    name: &str,
    data: &JobData,
    flaky_data: &HashMap<String, FlakinessMetrics>,
) -> JobMetrics {
    #[allow(clippy::cast_precision_loss)]
    let avg_duration_seconds = data.durations.iter().sum::<f64>() / data.durations.len() as f64;

    #[allow(clippy::cast_precision_loss)]
    let avg_time_to_feedback_seconds =
        data.total_durations.iter().sum::<f64>() / data.total_durations.len() as f64;

    let predecessors = aggregate_predecessors(&data.all_predecessors);

    // Get flakiness metrics if available
    let (flakiness_score, retry_count, total_occurrences) =
        flaky_data.get(name).map_or((0.0, 0, 0), |f| {
            (f.score, f.retry_count, f.total_occurrences)
        });

    JobMetrics {
        name: name.to_string(),
        avg_duration_seconds,
        avg_time_to_feedback_seconds,
        predecessors,
        flakiness_score,
        retry_count,
        total_occurrences,
    }
}

struct FlakinessMetrics {
    score: f64,
    retry_count: usize,
    total_occurrences: usize,
}

fn aggregate_predecessors(all_predecessors: &[Vec<PredecessorJob>]) -> Vec<PredecessorJob> {
    if all_predecessors.is_empty() {
        return vec![];
    }

    // Count how many times each predecessor appears
    let mut pred_data: HashMap<String, Vec<f64>> = HashMap::new();

    for predecessors in all_predecessors {
        for pred in predecessors {
            pred_data
                .entry(pred.name.clone())
                .or_default()
                .push(pred.avg_duration);
        }
    }

    let threshold = all_predecessors.len() / 2;

    // Keep predecessors that appear in >50% of pipelines
    let mut result: Vec<PredecessorJob> = pred_data
        .into_iter()
        .filter_map(|(name, durations)| {
            if durations.len() > threshold {
                #[allow(clippy::cast_precision_loss)]
                let avg_duration = durations.iter().sum::<f64>() / durations.len() as f64;
                Some(PredecessorJob { name, avg_duration })
            } else {
                None
            }
        })
        .collect();

    // Sort by avg_duration descending (slowest first)
    result.sort_by(|a, b| {
        b.avg_duration
            .partial_cmp(&a.avg_duration)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    result
}

fn calculate_flakiness(pipelines: &[&GitLabPipeline]) -> HashMap<String, FlakinessMetrics> {
    let mut retry_counts: HashMap<String, usize> = HashMap::new();
    let mut total_counts: HashMap<String, usize> = HashMap::new();

    for pipeline in pipelines {
        // Group jobs by name (a job may appear multiple times if retried)
        let jobs_by_name = group_jobs_by_name(&pipeline.jobs);

        for (name, jobs) in jobs_by_name {
            *total_counts.entry(name.to_string()).or_insert(0) += 1;

            // Check if this job was flaky (retried and eventually succeeded)
            if is_job_flaky(&jobs) {
                *retry_counts.entry(name.to_string()).or_insert(0) += 1;
            }
        }
    }

    // Calculate flakiness scores
    retry_counts
        .into_iter()
        .filter_map(|(name, retry_count)| {
            let total_occurrences = *total_counts.get(&name)?;

            // Only include jobs that appear multiple times
            if total_occurrences < 2 {
                return None;
            }

            #[allow(clippy::cast_precision_loss)]
            let score = (retry_count as f64 / total_occurrences as f64) * 100.0;

            Some((
                name,
                FlakinessMetrics {
                    score,
                    retry_count,
                    total_occurrences,
                },
            ))
        })
        .collect()
}

fn group_jobs_by_name(jobs: &[GitLabJob]) -> HashMap<&str, Vec<&GitLabJob>> {
    let mut grouped = HashMap::new();

    for job in jobs {
        grouped
            .entry(job.name.as_str())
            .or_insert_with(Vec::new)
            .push(job);
    }

    grouped
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
