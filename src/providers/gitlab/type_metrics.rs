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

    let jobs = calculate_all_job_metrics(&successful, pipelines);

    TypeMetrics {
        percentage,
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

    // Get execution counts and flakiness data from ALL pipelines
    let (execution_counts, flaky_data) = calculate_flakiness(all_pipelines);

    // Calculate complete metrics for each job
    let mut jobs: Vec<JobMetrics> = job_data
        .into_iter()
        .map(|(name, data)| calculate_job_metrics(&name, &data, &execution_counts, &flaky_data))
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
    execution_counts: &HashMap<String, usize>,
    flaky_data: &HashMap<String, FlakinessMetrics>,
) -> JobMetrics {
    #[allow(clippy::cast_precision_loss)]
    let avg_duration_seconds = data.durations.iter().sum::<f64>() / data.durations.len() as f64;

    #[allow(clippy::cast_precision_loss)]
    let avg_time_to_feedback_seconds =
        data.total_durations.iter().sum::<f64>() / data.total_durations.len() as f64;

    let predecessors = aggregate_predecessors(&data.all_predecessors);

    // Get total executions (always present)
    let total_executions = *execution_counts.get(name).unwrap_or(&0);

    // Get flakiness metrics if available (only for flaky jobs)
    let (flakiness_score, flaky_retries) = flaky_data
        .get(name)
        .map_or((0.0, 0), |f| (f.score, f.flaky_retries));

    JobMetrics {
        name: name.to_string(),
        avg_duration_seconds,
        avg_time_to_feedback_seconds,
        predecessors,
        flakiness_score,
        flaky_retries,
        total_executions,
    }
}

struct FlakinessMetrics {
    score: f64,
    flaky_retries: usize,
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

fn calculate_flakiness(
    pipelines: &[&GitLabPipeline],
) -> (HashMap<String, usize>, HashMap<String, FlakinessMetrics>) {
    let mut flaky_retriess: HashMap<String, usize> = HashMap::new();
    let mut execution_counts: HashMap<String, usize> = HashMap::new();

    for pipeline in pipelines {
        // Group jobs by name (a job may appear multiple times if retried)
        let jobs_by_name = group_jobs_by_name(&pipeline.jobs);

        for (name, jobs) in jobs_by_name {
            // Count total executions for this job in this pipeline
            let execution_count = jobs.len();
            *execution_counts.entry(name.to_string()).or_insert(0) += execution_count;

            // Only count retries if the job eventually succeeded in this pipeline
            if is_job_flaky(&jobs) {
                // Count retry attempts (jobs with retried: true)
                let retries = jobs.iter().filter(|j| j.retried).count();
                *flaky_retriess.entry(name.to_string()).or_insert(0) += retries;
            }
        }
    }

    // Calculate flakiness scores for jobs with retries
    let flaky_data = flaky_retriess
        .into_iter()
        .filter_map(|(name, flaky_retries)| {
            let total_executions = *execution_counts.get(&name)?;

            // Only include jobs that had at least one retry
            if flaky_retries == 0 {
                return None;
            }

            #[allow(clippy::cast_precision_loss)]
            let score = (flaky_retries as f64 / total_executions as f64) * 100.0;

            Some((
                name,
                FlakinessMetrics {
                    score,
                    flaky_retries,
                },
            ))
        })
        .collect();

    (execution_counts, flaky_data)
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
