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

#[cfg(test)]
#[allow(clippy::similar_names, clippy::float_cmp)]
mod tests {
    use super::*;

    // Helper function to create a test GitLabJob
    fn create_job(id: &str, name: &str, status: &str, retried: bool) -> GitLabJob {
        GitLabJob {
            id: id.to_string(),
            name: name.to_string(),
            stage: "test".to_string(),
            duration: 10.0,
            status: status.to_string(),
            retried,
            needs: None,
        }
    }

    #[cfg(test)]
    mod calculate_rate {
        use super::*;

        #[test]
        fn returns_zero_when_total_is_zero() {
            let result = calculate_rate(5, 0);
            assert_eq!(result, 0.0, "Should return 0.0 when total is 0");
        }

        #[test]
        fn returns_zero_when_count_is_zero() {
            let result = calculate_rate(0, 100);
            assert_eq!(result, 0.0, "Should return 0.0 when count is 0");
        }

        #[test]
        fn calculates_percentage_correctly() {
            let result = calculate_rate(25, 100);
            assert_eq!(result, 25.0, "Should calculate 25% correctly");
        }

        #[test]
        fn calculates_fifty_percent() {
            let result = calculate_rate(50, 100);
            assert_eq!(result, 50.0, "Should calculate 50% correctly");
        }

        #[test]
        fn calculates_one_hundred_percent() {
            let result = calculate_rate(100, 100);
            assert_eq!(result, 100.0, "Should calculate 100% correctly");
        }

        #[test]
        fn handles_fractional_percentages() {
            let result = calculate_rate(1, 3);
            assert!(
                (result - 33.333_333).abs() < 0.001,
                "Should handle fractional percentages, got {result}",
            );
        }

        #[test]
        fn handles_small_numbers() {
            let result = calculate_rate(1, 1);
            assert_eq!(result, 100.0, "Should handle 1/1 = 100%");
        }

        #[test]
        fn handles_large_numbers() {
            let result = calculate_rate(999, 1000);
            assert_eq!(result, 99.9, "Should handle large numbers correctly");
        }

        #[test]
        fn returns_zero_for_zero_count_and_total() {
            let result = calculate_rate(0, 0);
            assert_eq!(result, 0.0, "Should return 0.0 when both are 0");
        }
    }

    #[cfg(test)]
    mod is_job_flaky {
        use super::*;

        #[test]
        fn returns_false_when_no_retries() {
            let job = create_job("1", "test-job", "SUCCESS", false);
            let jobs = vec![&job];

            assert!(
                !is_job_flaky(&jobs),
                "Job without retries should not be flaky"
            );
        }

        #[test]
        fn returns_true_when_retried_and_succeeded() {
            let job1 = create_job("1", "test-job", "FAILED", true);
            let job2 = create_job("2", "test-job", "SUCCESS", false);
            let jobs = vec![&job1, &job2];

            assert!(
                is_job_flaky(&jobs),
                "Job that was retried and eventually succeeded should be flaky"
            );
        }

        #[test]
        fn returns_false_when_retried_but_failed() {
            let job1 = create_job("1", "test-job", "FAILED", true);
            let job2 = create_job("2", "test-job", "FAILED", false);
            let jobs = vec![&job1, &job2];

            assert!(
                !is_job_flaky(&jobs),
                "Job that was retried but still failed should not be flaky"
            );
        }

        #[test]
        fn returns_false_when_all_jobs_retried() {
            let job1 = create_job("1", "test-job", "FAILED", true);
            let job2 = create_job("2", "test-job", "FAILED", true);
            let jobs = vec![&job1, &job2];

            assert!(
                !is_job_flaky(&jobs),
                "When all jobs are retried (no final job), should not be flaky"
            );
        }

        #[test]
        fn returns_true_with_multiple_retries_and_success() {
            let job1 = create_job("1", "test-job", "FAILED", true);
            let job2 = create_job("2", "test-job", "FAILED", true);
            let job3 = create_job("3", "test-job", "SUCCESS", false);
            let jobs = vec![&job1, &job2, &job3];

            assert!(
                is_job_flaky(&jobs),
                "Job with multiple retries that eventually succeeded should be flaky"
            );
        }

        #[test]
        fn returns_false_when_final_job_not_success() {
            let job1 = create_job("1", "test-job", "FAILED", true);
            let job2 = create_job("2", "test-job", "CANCELED", false);
            let jobs = vec![&job1, &job2];

            assert!(
                !is_job_flaky(&jobs),
                "Job that was retried but final status is not SUCCESS should not be flaky"
            );
        }

        #[test]
        fn handles_empty_job_list() {
            let jobs: Vec<&GitLabJob> = vec![];

            assert!(
                !is_job_flaky(&jobs),
                "Empty job list should not be considered flaky"
            );
        }
    }

    #[cfg(test)]
    mod is_job_failed {
        use super::*;

        #[test]
        fn returns_false_when_job_succeeded_without_retries() {
            let job = create_job("1", "test-job", "SUCCESS", false);
            let jobs = vec![&job];

            assert!(
                !is_job_failed(&jobs),
                "Successful job without retries should not be failed"
            );
        }

        #[test]
        fn returns_true_when_job_failed_without_retries() {
            let job = create_job("1", "test-job", "FAILED", false);
            let jobs = vec![&job];

            assert!(
                is_job_failed(&jobs),
                "Failed job without retries should be considered failed"
            );
        }

        #[test]
        fn returns_false_when_retried_and_succeeded() {
            let job1 = create_job("1", "test-job", "FAILED", true);
            let job2 = create_job("2", "test-job", "SUCCESS", false);
            let jobs = vec![&job1, &job2];

            assert!(
                !is_job_failed(&jobs),
                "Job that was retried and succeeded should not be failed (it's flaky)"
            );
        }

        #[test]
        fn returns_true_when_retried_and_failed() {
            let job1 = create_job("1", "test-job", "FAILED", true);
            let job2 = create_job("2", "test-job", "FAILED", false);
            let jobs = vec![&job1, &job2];

            assert!(
                is_job_failed(&jobs),
                "Job that was retried and still failed should be considered failed"
            );
        }

        #[test]
        fn returns_true_when_no_final_job() {
            let job1 = create_job("1", "test-job", "FAILED", true);
            let job2 = create_job("2", "test-job", "FAILED", true);
            let jobs = vec![&job1, &job2];

            assert!(
                is_job_failed(&jobs),
                "When there's no final (non-retried) job, should be considered failed"
            );
        }

        #[test]
        fn returns_true_when_final_job_canceled() {
            let job1 = create_job("1", "test-job", "FAILED", true);
            let job2 = create_job("2", "test-job", "CANCELED", false);
            let jobs = vec![&job1, &job2];

            assert!(
                is_job_failed(&jobs),
                "Job with final status of CANCELED should be considered failed"
            );
        }

        #[test]
        fn returns_true_when_final_job_skipped() {
            let job1 = create_job("1", "test-job", "FAILED", true);
            let job2 = create_job("2", "test-job", "SKIPPED", false);
            let jobs = vec![&job1, &job2];

            assert!(
                is_job_failed(&jobs),
                "Job with final status of SKIPPED should be considered failed"
            );
        }

        #[test]
        fn handles_empty_job_list() {
            let jobs: Vec<&GitLabJob> = vec![];

            assert!(
                is_job_failed(&jobs),
                "Empty job list should be considered failed (no successful final job)"
            );
        }

        #[test]
        fn returns_false_with_multiple_retries_and_success() {
            let job1 = create_job("1", "test-job", "FAILED", true);
            let job2 = create_job("2", "test-job", "FAILED", true);
            let job3 = create_job("3", "test-job", "SUCCESS", false);
            let jobs = vec![&job1, &job2, &job3];

            assert!(
                !is_job_failed(&jobs),
                "Job with multiple retries that eventually succeeded should not be failed"
            );
        }
    }

    #[cfg(test)]
    mod group_jobs_by_name {
        use super::*;

        #[test]
        fn returns_empty_map_for_empty_list() {
            let jobs: Vec<GitLabJob> = vec![];
            let result = group_jobs_by_name(&jobs);

            assert!(
                result.is_empty(),
                "Should return empty map for empty job list"
            );
        }

        #[test]
        fn groups_single_job() {
            let jobs = vec![create_job("1", "test-job", "SUCCESS", false)];
            let result = group_jobs_by_name(&jobs);

            assert_eq!(result.len(), 1, "Should have one group");
            assert!(result.contains_key("test-job"), "Should contain test-job");
            assert_eq!(
                result.get("test-job").unwrap().len(),
                1,
                "test-job group should have 1 job"
            );
        }

        #[test]
        fn groups_multiple_jobs_with_same_name() {
            let jobs = vec![
                create_job("1", "test-job", "FAILED", true),
                create_job("2", "test-job", "SUCCESS", false),
                create_job("3", "test-job", "FAILED", true),
            ];
            let result = group_jobs_by_name(&jobs);

            assert_eq!(result.len(), 1, "Should have one group");
            assert_eq!(
                result.get("test-job").unwrap().len(),
                3,
                "test-job group should have 3 jobs"
            );
        }

        #[test]
        fn groups_jobs_with_different_names() {
            let jobs = vec![
                create_job("1", "build", "SUCCESS", false),
                create_job("2", "test", "SUCCESS", false),
                create_job("3", "deploy", "SUCCESS", false),
            ];
            let result = group_jobs_by_name(&jobs);

            assert_eq!(result.len(), 3, "Should have three groups");
            assert!(result.contains_key("build"), "Should contain build");
            assert!(result.contains_key("test"), "Should contain test");
            assert!(result.contains_key("deploy"), "Should contain deploy");
            assert_eq!(
                result.get("build").unwrap().len(),
                1,
                "Each group should have 1 job"
            );
        }

        #[test]
        fn groups_mixed_jobs() {
            let jobs = vec![
                create_job("1", "build", "SUCCESS", false),
                create_job("2", "test", "FAILED", true),
                create_job("3", "test", "SUCCESS", false),
                create_job("4", "build", "FAILED", true),
                create_job("5", "deploy", "SUCCESS", false),
            ];
            let result = group_jobs_by_name(&jobs);

            assert_eq!(result.len(), 3, "Should have three groups");
            assert_eq!(
                result.get("build").unwrap().len(),
                2,
                "build group should have 2 jobs"
            );
            assert_eq!(
                result.get("test").unwrap().len(),
                2,
                "test group should have 2 jobs"
            );
            assert_eq!(
                result.get("deploy").unwrap().len(),
                1,
                "deploy group should have 1 job"
            );
        }

        #[test]
        fn preserves_job_order_within_groups() {
            let jobs = vec![
                create_job("1", "test", "FAILED", true),
                create_job("2", "test", "FAILED", true),
                create_job("3", "test", "SUCCESS", false),
            ];
            let result = group_jobs_by_name(&jobs);

            let test_jobs = result.get("test").unwrap();
            assert_eq!(test_jobs[0].id, "1", "First job should have id 1");
            assert_eq!(test_jobs[1].id, "2", "Second job should have id 2");
            assert_eq!(test_jobs[2].id, "3", "Third job should have id 3");
        }

        #[test]
        fn handles_jobs_with_special_characters_in_names() {
            let jobs = vec![
                create_job("1", "test:unit", "SUCCESS", false),
                create_job("2", "test:integration", "SUCCESS", false),
                create_job("3", "test:unit", "FAILED", true),
            ];
            let result = group_jobs_by_name(&jobs);

            assert_eq!(result.len(), 2, "Should have two groups");
            assert!(result.contains_key("test:unit"), "Should contain test:unit");
            assert!(
                result.contains_key("test:integration"),
                "Should contain test:integration"
            );
            assert_eq!(
                result.get("test:unit").unwrap().len(),
                2,
                "test:unit group should have 2 jobs"
            );
        }
    }

    #[cfg(test)]
    mod integration_tests {
        use super::*;

        // Helper to create a test pipeline
        fn create_pipeline(id: &str, jobs: Vec<GitLabJob>) -> GitLabPipeline {
            GitLabPipeline {
                id: id.to_string(),
                ref_: "main".to_string(),
                source: "push".to_string(),
                status: "SUCCESS".to_string(),
                duration: 100,
                stages: vec!["test".to_string()],
                jobs,
            }
        }

        #[test]
        fn calculates_reliability_for_flaky_job() {
            let pipeline = create_pipeline(
                "1",
                vec![
                    create_job("1", "test-job", "FAILED", true),
                    create_job("2", "test-job", "SUCCESS", false),
                ],
            );
            let pipelines = vec![&pipeline];

            let result = calculate_job_reliability(&pipelines, "https://gitlab.com", "owner/repo");

            assert!(result.contains_key("test-job"), "Should have test-job");
            let metrics = result.get("test-job").unwrap();
            assert_eq!(metrics.total_executions, 2);
            assert_eq!(metrics.flaky_retries, 1);
            assert!(metrics.flakiness_rate > 0.0, "Flakiness rate should be > 0");
            assert_eq!(metrics.failed_executions, 0, "Should have no failures");
            assert_eq!(metrics.flaky_job_links.len(), 1);
        }

        #[test]
        fn calculates_reliability_for_failed_job() {
            let pipeline = create_pipeline(
                "1",
                vec![
                    create_job("1", "test-job", "FAILED", true),
                    create_job("2", "test-job", "FAILED", false),
                ],
            );
            let pipelines = vec![&pipeline];

            let result = calculate_job_reliability(&pipelines, "https://gitlab.com", "owner/repo");

            assert!(result.contains_key("test-job"), "Should have test-job");
            let metrics = result.get("test-job").unwrap();
            assert_eq!(metrics.total_executions, 2);
            assert_eq!(metrics.flaky_retries, 0);
            assert_eq!(metrics.failed_executions, 1);
            assert!(metrics.failure_rate > 0.0, "Failure rate should be > 0");
            assert_eq!(metrics.failed_job_links.len(), 1);
        }

        #[test]
        fn calculates_reliability_for_successful_job() {
            let pipeline =
                create_pipeline("1", vec![create_job("1", "test-job", "SUCCESS", false)]);
            let pipelines = vec![&pipeline];

            let result = calculate_job_reliability(&pipelines, "https://gitlab.com", "owner/repo");

            assert!(result.contains_key("test-job"), "Should have test-job");
            let metrics = result.get("test-job").unwrap();
            assert_eq!(metrics.total_executions, 1);
            assert_eq!(metrics.flaky_retries, 0);
            assert_eq!(metrics.failed_executions, 0);
            assert_eq!(metrics.flakiness_rate, 0.0);
            assert_eq!(metrics.failure_rate, 0.0);
        }

        #[test]
        fn handles_multiple_pipelines() {
            let pipeline1 = create_pipeline(
                "1",
                vec![
                    create_job("1", "test-job", "FAILED", true),
                    create_job("2", "test-job", "SUCCESS", false),
                ],
            );
            let pipeline2 =
                create_pipeline("2", vec![create_job("3", "test-job", "SUCCESS", false)]);
            let pipelines = vec![&pipeline1, &pipeline2];

            let result = calculate_job_reliability(&pipelines, "https://gitlab.com", "owner/repo");

            assert!(result.contains_key("test-job"), "Should have test-job");
            let metrics = result.get("test-job").unwrap();
            assert_eq!(
                metrics.total_executions, 3,
                "Should count jobs from both pipelines"
            );
        }

        #[test]
        fn handles_empty_pipelines() {
            let pipelines: Vec<&GitLabPipeline> = vec![];

            let result = calculate_job_reliability(&pipelines, "https://gitlab.com", "owner/repo");

            assert!(
                result.is_empty(),
                "Should return empty map for no pipelines"
            );
        }

        #[test]
        fn handles_pipeline_with_no_jobs() {
            let pipeline = create_pipeline("1", vec![]);
            let pipelines = vec![&pipeline];

            let result = calculate_job_reliability(&pipelines, "https://gitlab.com", "owner/repo");

            assert!(
                result.is_empty(),
                "Should return empty map when pipeline has no jobs"
            );
        }
    }
}
