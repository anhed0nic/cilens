use std::collections::HashMap;

use super::links::job_id_to_url;
use super::types::{GitLabJob, GitLabPipeline};

#[allow(clippy::cast_precision_loss)]
fn calculate_rate(count: usize, total: usize) -> f64 {
    if total > 0 {
        (count as f64 / total as f64) * 100.0
    } else {
        0.0
    }
}

pub(super) struct JobReliabilityMetrics {
    pub total_executions: usize,
    pub flakiness_rate: f64,
    pub flaky_retries: usize,
    pub flaky_job_links: Vec<String>,
    pub failure_rate: f64,
    pub failed_executions: usize,
    pub failed_job_links: Vec<String>,
}

pub(super) fn calculate_job_reliability(
    pipelines: &[&GitLabPipeline],
    base_url: &str,
    project_path: &str,
) -> HashMap<String, JobReliabilityMetrics> {
    let mut execution_counts: HashMap<String, usize> = HashMap::new();
    let mut flaky_retries: HashMap<String, usize> = HashMap::new();
    let mut flaky_job_links: HashMap<String, Vec<String>> = HashMap::new();
    let mut failed_executions: HashMap<String, usize> = HashMap::new();
    let mut failed_job_links: HashMap<String, Vec<String>> = HashMap::new();

    for pipeline in pipelines {
        let jobs_by_name = group_jobs_by_name(&pipeline.jobs);

        for (name, jobs) in jobs_by_name {
            *execution_counts.entry(name.to_string()).or_insert(0) += jobs.len();

            if is_job_flaky(&jobs) {
                let retry_links: Vec<String> = jobs
                    .iter()
                    .filter(|j| j.retried)
                    .map(|j| job_id_to_url(base_url, project_path, &j.id))
                    .collect();
                *flaky_retries.entry(name.to_string()).or_insert(0) += retry_links.len();
                flaky_job_links
                    .entry(name.to_string())
                    .or_default()
                    .extend(retry_links);
            } else if is_job_failed(&jobs) {
                *failed_executions.entry(name.to_string()).or_insert(0) += 1;
                // Get the final non-retried job (the one that failed)
                if let Some(final_job) = jobs.iter().find(|j| !j.retried) {
                    failed_job_links
                        .entry(name.to_string())
                        .or_default()
                        .push(job_id_to_url(base_url, project_path, &final_job.id));
                }
            }
        }
    }

    compute_reliability_metrics(
        &flaky_retries,
        &flaky_job_links,
        &failed_executions,
        &failed_job_links,
        &execution_counts,
    )
}

fn compute_reliability_metrics(
    retry_counts: &HashMap<String, usize>,
    retry_job_links: &HashMap<String, Vec<String>>,
    failure_counts: &HashMap<String, usize>,
    failure_job_links: &HashMap<String, Vec<String>>,
    execution_counts: &HashMap<String, usize>,
) -> HashMap<String, JobReliabilityMetrics> {
    execution_counts
        .iter()
        .map(|(name, &total_executions)| {
            let flaky_retries = *retry_counts.get(name).unwrap_or(&0);
            let failed_executions = *failure_counts.get(name).unwrap_or(&0);
            let flaky_job_links = retry_job_links.get(name).cloned().unwrap_or_default();
            let failed_job_links = failure_job_links.get(name).cloned().unwrap_or_default();

            (
                name.clone(),
                JobReliabilityMetrics {
                    total_executions,
                    flakiness_rate: calculate_rate(flaky_retries, total_executions),
                    flaky_retries,
                    flaky_job_links,
                    failure_rate: calculate_rate(failed_executions, total_executions),
                    failed_executions,
                    failed_job_links,
                },
            )
        })
        .collect()
}

fn group_jobs_by_name(jobs: &[GitLabJob]) -> HashMap<&str, Vec<&GitLabJob>> {
    jobs.iter().fold(HashMap::new(), |mut grouped, job| {
        grouped.entry(job.name.as_str()).or_default().push(job);
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
