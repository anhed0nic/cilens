use std::collections::{BTreeSet, HashMap};

use super::types::GitLabPipeline;
use crate::insights::PipelineType;

fn extract_job_signature(pipeline: &GitLabPipeline) -> Vec<String> {
    pipeline
        .jobs
        .iter()
        .map(|j| j.name.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Groups pipelines by their job signatures and filters by minimum percentage threshold.
///
/// Pipelines with identical sets of job names are grouped into the same type. Each type
/// receives a human-readable label (e.g., "Production", "Development")
/// based on keywords found in job names, and comprehensive metrics are calculated.
///
/// # Arguments
///
/// * `pipelines` - Collection of GitLab pipelines to analyze
/// * `min_type_percentage` - Minimum percentage (0-100) required for a pipeline type to be included
/// * `base_url` - GitLab instance base URL (e.g., <https://gitlab.com>) for generating pipeline/job URLs
/// * `project_path` - Project path (e.g., "group/project") for generating URLs
///
/// # Returns
///
/// Vector of pipeline types sorted by frequency (most common first), filtered to only
/// include types that represent at least `min_type_percentage` of total pipelines.
///
/// # Examples
///
/// ```no_run
/// // Group pipelines, excluding types that are less than 5% of total
/// let pipeline_types = group_pipeline_types(
///     &pipelines,
///     5,  // min 5% threshold
///     "https://gitlab.com",
///     "my-org/my-project"
/// );
/// ```
pub fn group_pipeline_types(
    pipelines: &[GitLabPipeline],
    min_type_percentage: u8,
    base_url: &str,
    project_path: &str,
) -> Vec<PipelineType> {
    let total_pipelines = pipelines.len();

    let mut clusters: HashMap<Vec<String>, Vec<&GitLabPipeline>> = HashMap::new();
    for pipeline in pipelines {
        let job_signature = extract_job_signature(pipeline);
        clusters.entry(job_signature).or_default().push(pipeline);
    }

    let mut pipeline_types: Vec<PipelineType> = clusters
        .into_iter()
        .map(|(job_names, cluster_pipelines)| {
            create_pipeline_type(
                &job_names,
                &cluster_pipelines,
                total_pipelines,
                base_url,
                project_path,
            )
        })
        .filter(|pt| pt.metrics.percentage >= f64::from(min_type_percentage))
        .collect();

    pipeline_types.sort_by(|a, b| b.metrics.total_pipelines.cmp(&a.metrics.total_pipelines));
    pipeline_types
}

fn create_pipeline_type(
    job_names: &[String],
    pipelines: &[&GitLabPipeline],
    total_pipelines: usize,
    base_url: &str,
    project_path: &str,
) -> PipelineType {
    let count = pipelines.len();
    #[allow(clippy::cast_precision_loss)]
    let percentage = (count as f64 / total_pipelines.max(1) as f64) * 100.0;

    let label = generate_label(job_names);
    let (stages, ref_patterns, sources) = extract_characteristics(pipelines);
    let metrics = super::pipeline_metrics::calculate_type_metrics(
        pipelines,
        percentage,
        base_url,
        project_path,
    );

    PipelineType {
        label,
        stages,
        ref_patterns,
        sources,
        metrics,
    }
}

fn generate_label(job_names: &[String]) -> String {
    let has_keyword = |keywords: &[&str]| {
        job_names.iter().any(|name| {
            let lower = name.to_lowercase();
            keywords.iter().any(|kw| lower.contains(kw))
        })
    };

    if has_keyword(&["prod"]) {
        "Production".to_string()
    } else if has_keyword(&["staging", "dev", "test", "qa"]) {
        "Development".to_string()
    } else {
        "Unknown".to_string()
    }
}

fn extract_characteristics(
    pipelines: &[&GitLabPipeline],
) -> (Vec<String>, Vec<String>, Vec<String>) {
    use std::collections::HashSet;

    let collect_unique = |iter: Vec<String>| -> Vec<String> {
        iter.into_iter()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect()
    };

    let stages = collect_unique(
        pipelines
            .iter()
            .flat_map(|p| p.jobs.iter().map(|j| j.stage.clone()))
            .collect(),
    );

    let ref_patterns = collect_unique(pipelines.iter().map(|p| p.ref_.clone()).collect());

    let sources = collect_unique(pipelines.iter().map(|p| p.source.clone()).collect());

    (stages, ref_patterns, sources)
}

#[cfg(test)]
#[allow(clippy::similar_names)]
mod tests {
    use super::super::types::{GitLabJob, GitLabPipeline};
    use super::*;

    // Helper function to create a test GitLabJob
    fn create_job(name: &str, stage: &str) -> GitLabJob {
        GitLabJob {
            id: format!("job-{name}"),
            name: name.to_string(),
            stage: stage.to_string(),
            duration: 10.0,
            status: "success".to_string(),
            retried: false,
            needs: None,
        }
    }

    // Helper function to create a test GitLabPipeline
    fn create_pipeline(id: &str, ref_: &str, source: &str, jobs: Vec<GitLabJob>) -> GitLabPipeline {
        let stages: Vec<String> = jobs
            .iter()
            .map(|j| j.stage.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();

        GitLabPipeline {
            id: id.to_string(),
            ref_: ref_.to_string(),
            source: source.to_string(),
            status: "success".to_string(),
            duration: 100,
            stages,
            jobs,
        }
    }

    mod extract_job_signature_tests {
        use super::*;

        #[test]
        fn returns_empty_vec_for_pipeline_with_no_jobs() {
            // Arrange: Create a pipeline with no jobs
            let pipeline = create_pipeline("1", "main", "push", vec![]);

            // Act: Extract job signature
            let signature = extract_job_signature(&pipeline);

            // Assert: Signature should be empty
            assert!(signature.is_empty());
        }

        #[test]
        fn returns_single_job_name_for_pipeline_with_one_job() {
            // Arrange: Create a pipeline with one job
            let job = create_job("build", "build");
            let pipeline = create_pipeline("1", "main", "push", vec![job]);

            // Act: Extract job signature
            let signature = extract_job_signature(&pipeline);

            // Assert: Signature should contain only the one job name
            assert_eq!(signature, vec!["build"]);
        }

        #[test]
        fn deduplicates_jobs_with_same_name() {
            // Arrange: Create a pipeline with multiple jobs having the same name
            let job1 = create_job("test", "test");
            let job2 = create_job("test", "test");
            let job3 = create_job("test", "test");
            let pipeline = create_pipeline("1", "main", "push", vec![job1, job2, job3]);

            // Act: Extract job signature
            let signature = extract_job_signature(&pipeline);

            // Assert: Signature should contain only one instance of "test"
            assert_eq!(signature, vec!["test"]);
        }

        #[test]
        fn returns_all_unique_job_names() {
            // Arrange: Create a pipeline with multiple jobs with different names
            let job1 = create_job("build", "build");
            let job2 = create_job("test", "test");
            let job3 = create_job("deploy", "deploy");
            let pipeline = create_pipeline("1", "main", "push", vec![job1, job2, job3]);

            // Act: Extract job signature
            let signature = extract_job_signature(&pipeline);

            // Assert: Signature should contain all three job names
            assert_eq!(signature.len(), 3);
            assert!(signature.contains(&"build".to_string()));
            assert!(signature.contains(&"test".to_string()));
            assert!(signature.contains(&"deploy".to_string()));
        }

        #[test]
        fn returns_alphabetically_sorted_job_names() {
            // Arrange: Create jobs in non-alphabetical order
            let job1 = create_job("zebra", "stage1");
            let job2 = create_job("alpha", "stage2");
            let job3 = create_job("beta", "stage3");
            let pipeline = create_pipeline("1", "main", "push", vec![job1, job2, job3]);

            // Act: Extract job signature
            let signature = extract_job_signature(&pipeline);

            // Assert: BTreeSet should sort alphabetically
            assert_eq!(signature, vec!["alpha", "beta", "zebra"]);
        }

        #[test]
        fn handles_mixed_duplicate_and_unique_jobs() {
            // Arrange: Create a pipeline with some duplicate and some unique job names
            let jobs = vec![
                create_job("build", "build"),
                create_job("test", "test"),
                create_job("build", "build"), // duplicate
                create_job("deploy", "deploy"),
                create_job("test", "test"), // duplicate
            ];
            let pipeline = create_pipeline("1", "main", "push", jobs);

            // Act: Extract job signature
            let signature = extract_job_signature(&pipeline);

            // Assert: Should deduplicate and sort
            assert_eq!(signature, vec!["build", "deploy", "test"]);
        }
    }

    mod generate_label_tests {
        use super::*;

        #[test]
        fn returns_production_label_for_prod_keyword() {
            // Arrange: Job names containing "prod"
            let job_names = vec!["deploy-prod".to_string(), "test".to_string()];

            // Act: Generate label
            let label = generate_label(&job_names);

            // Assert: Should identify as Production
            assert_eq!(label, "Production");
        }

        #[test]
        fn returns_production_label_for_production_keyword() {
            // Arrange: Job names containing "production"
            let job_names = vec!["deploy-production".to_string(), "build".to_string()];

            // Act: Generate label
            let label = generate_label(&job_names);

            // Assert: Should identify as Production
            assert_eq!(label, "Production");
        }

        #[test]
        fn returns_development_label_for_staging_keyword() {
            // Arrange: Job names containing "staging"
            let job_names = vec!["deploy-staging".to_string(), "test".to_string()];

            // Act: Generate label
            let label = generate_label(&job_names);

            // Assert: Should identify as Development
            assert_eq!(label, "Development");
        }

        #[test]
        fn returns_development_label_for_dev_keyword() {
            // Arrange: Job names containing "dev"
            let job_names = vec!["deploy-dev".to_string(), "build".to_string()];

            // Act: Generate label
            let label = generate_label(&job_names);

            // Assert: Should identify as Development
            assert_eq!(label, "Development");
        }

        #[test]
        fn returns_development_label_for_test_keyword() {
            // Arrange: Job names containing "test"
            let job_names = vec!["run-tests".to_string(), "build".to_string()];

            // Act: Generate label
            let label = generate_label(&job_names);

            // Assert: Should identify as Development
            assert_eq!(label, "Development");
        }

        #[test]
        fn returns_development_label_for_qa_keyword() {
            // Arrange: Job names containing "qa"
            let job_names = vec!["deploy-qa".to_string(), "build".to_string()];

            // Act: Generate label
            let label = generate_label(&job_names);

            // Assert: Should identify as Development
            assert_eq!(label, "Development");
        }

        #[test]
        fn is_case_insensitive_for_prod() {
            // Arrange: Job names with uppercase PROD
            let job_names = vec!["deploy-PROD".to_string()];

            // Act: Generate label
            let label = generate_label(&job_names);

            // Assert: Should identify as Production Pipeline despite case
            assert_eq!(label, "Production");
        }

        #[test]
        fn is_case_insensitive_for_dev() {
            // Arrange: Job names with mixed case Dev
            let job_names = vec!["deploy-Dev".to_string()];

            // Act: Generate label
            let label = generate_label(&job_names);

            // Assert: Should identify as Development Pipeline despite case
            assert_eq!(label, "Development");
        }

        #[test]
        fn returns_unknown_label_when_no_keywords_match() {
            // Arrange: Job names without any recognized keywords
            let job_names = vec![
                "build".to_string(),
                "compile".to_string(),
                "package".to_string(),
            ];

            // Act: Generate label
            let label = generate_label(&job_names);

            // Assert: Should identify as Unknown
            assert_eq!(label, "Unknown");
        }

        #[test]
        fn production_takes_precedence_over_development() {
            // Arrange: Job names containing both production and development keywords
            let job_names = vec!["deploy-prod".to_string(), "test-staging".to_string()];

            // Act: Generate label
            let label = generate_label(&job_names);

            // Assert: Production should take precedence
            assert_eq!(label, "Production");
        }

        #[test]
        fn handles_empty_job_names() {
            // Arrange: Empty job names list
            let job_names: Vec<String> = vec![];

            // Act: Generate label
            let label = generate_label(&job_names);

            // Assert: Should return Unknown Pipeline
            assert_eq!(label, "Unknown");
        }

        #[test]
        fn keyword_can_be_anywhere_in_job_name() {
            // Arrange: Keywords embedded in middle or end of job names
            let job_names = vec![
                "my-production-deployment".to_string(),
                "another-job".to_string(),
            ];

            // Act: Generate label
            let label = generate_label(&job_names);

            // Assert: Should find "production" keyword
            assert_eq!(label, "Production");
        }
    }

    mod extract_characteristics_tests {
        use super::*;

        #[test]
        fn returns_empty_vecs_for_empty_pipeline_list() {
            // Arrange: Empty pipeline list
            let pipelines: Vec<&GitLabPipeline> = vec![];

            // Act: Extract characteristics
            let (stages, ref_patterns, sources) = extract_characteristics(&pipelines);

            // Assert: All should be empty
            assert!(stages.is_empty());
            assert!(ref_patterns.is_empty());
            assert!(sources.is_empty());
        }

        #[test]
        fn extracts_characteristics_from_single_pipeline() {
            // Arrange: Create a single pipeline
            let jobs = vec![create_job("build", "build"), create_job("test", "test")];
            let pipeline = create_pipeline("1", "main", "push", jobs);
            let pipelines = vec![&pipeline];

            // Act: Extract characteristics
            let (stages, ref_patterns, sources) = extract_characteristics(&pipelines);

            // Assert: Should extract all characteristics
            assert_eq!(stages.len(), 2);
            assert!(stages.contains(&"build".to_string()));
            assert!(stages.contains(&"test".to_string()));
            assert_eq!(ref_patterns, vec!["main"]);
            assert_eq!(sources, vec!["push"]);
        }

        #[test]
        fn deduplicates_stages_across_pipelines() {
            // Arrange: Create multiple pipelines with overlapping stages
            let pipeline1 = create_pipeline(
                "1",
                "main",
                "push",
                vec![create_job("build", "build"), create_job("test", "test")],
            );
            let pipeline2 = create_pipeline(
                "2",
                "main",
                "push",
                vec![create_job("build", "build"), create_job("deploy", "deploy")],
            );
            let pipelines = vec![&pipeline1, &pipeline2];

            // Act: Extract characteristics
            let (stages, _, _) = extract_characteristics(&pipelines);

            // Assert: Should deduplicate stages
            assert_eq!(stages.len(), 3);
            assert!(stages.contains(&"build".to_string()));
            assert!(stages.contains(&"test".to_string()));
            assert!(stages.contains(&"deploy".to_string()));
        }

        #[test]
        fn deduplicates_ref_patterns() {
            // Arrange: Create multiple pipelines with same and different refs
            let pipeline1 =
                create_pipeline("1", "main", "push", vec![create_job("build", "build")]);
            let pipeline2 = create_pipeline("2", "main", "push", vec![create_job("test", "test")]);
            let pipeline3 =
                create_pipeline("3", "develop", "push", vec![create_job("build", "build")]);
            let pipelines = vec![&pipeline1, &pipeline2, &pipeline3];

            // Act: Extract characteristics
            let (_, ref_patterns, _) = extract_characteristics(&pipelines);

            // Assert: Should deduplicate ref patterns
            assert_eq!(ref_patterns.len(), 2);
            assert!(ref_patterns.contains(&"main".to_string()));
            assert!(ref_patterns.contains(&"develop".to_string()));
        }

        #[test]
        fn deduplicates_sources() {
            // Arrange: Create multiple pipelines with same and different sources
            let pipeline1 =
                create_pipeline("1", "main", "push", vec![create_job("build", "build")]);
            let pipeline2 = create_pipeline("2", "main", "push", vec![create_job("test", "test")]);
            let pipeline3 =
                create_pipeline("3", "main", "schedule", vec![create_job("build", "build")]);
            let pipelines = vec![&pipeline1, &pipeline2, &pipeline3];

            // Act: Extract characteristics
            let (_, _, sources) = extract_characteristics(&pipelines);

            // Assert: Should deduplicate sources
            assert_eq!(sources.len(), 2);
            assert!(sources.contains(&"push".to_string()));
            assert!(sources.contains(&"schedule".to_string()));
        }

        #[test]
        fn collects_stages_from_all_jobs() {
            // Arrange: Create pipelines with multiple jobs in different stages
            let pipeline1 = create_pipeline(
                "1",
                "main",
                "push",
                vec![
                    create_job("compile", "build"),
                    create_job("unit-test", "test"),
                    create_job("integration-test", "integration"),
                ],
            );
            let pipeline2 = create_pipeline(
                "2",
                "main",
                "push",
                vec![create_job("build", "build"), create_job("deploy", "deploy")],
            );
            let pipelines = vec![&pipeline1, &pipeline2];

            // Act: Extract characteristics
            let (stages, _, _) = extract_characteristics(&pipelines);

            // Assert: Should collect all unique stages
            assert_eq!(stages.len(), 4);
            assert!(stages.contains(&"build".to_string()));
            assert!(stages.contains(&"test".to_string()));
            assert!(stages.contains(&"integration".to_string()));
            assert!(stages.contains(&"deploy".to_string()));
        }

        #[test]
        fn handles_pipelines_with_no_jobs() {
            // Arrange: Create pipelines with no jobs
            let pipeline1 = create_pipeline("1", "main", "push", vec![]);
            let pipeline2 = create_pipeline("2", "develop", "schedule", vec![]);
            let pipelines = vec![&pipeline1, &pipeline2];

            // Act: Extract characteristics
            let (stages, ref_patterns, sources) = extract_characteristics(&pipelines);

            // Assert: Stages should be empty, but refs and sources should be present
            assert!(stages.is_empty());
            assert_eq!(ref_patterns.len(), 2);
            assert_eq!(sources.len(), 2);
        }
    }

    mod group_pipeline_types_tests {
        use super::*;

        #[test]
        fn returns_empty_vec_for_empty_pipeline_list() {
            // Arrange: Empty pipeline list
            let pipelines: Vec<GitLabPipeline> = vec![];

            // Act: Group pipeline types
            let result = group_pipeline_types(&pipelines, 0, "https://gitlab.com", "org/repo");

            // Assert: Should return empty vec
            assert!(result.is_empty());
        }

        #[test]
        fn creates_single_type_for_identical_job_signatures() {
            // Arrange: Create pipelines with identical job signatures
            let pipeline1 = create_pipeline(
                "1",
                "main",
                "push",
                vec![create_job("build", "build"), create_job("test", "test")],
            );
            let pipeline2 = create_pipeline(
                "2",
                "main",
                "push",
                vec![create_job("build", "build"), create_job("test", "test")],
            );
            let pipeline3 = create_pipeline(
                "3",
                "develop",
                "push",
                vec![create_job("build", "build"), create_job("test", "test")],
            );
            let pipelines = vec![pipeline1, pipeline2, pipeline3];

            // Act: Group pipeline types
            let result = group_pipeline_types(&pipelines, 0, "https://gitlab.com", "org/repo");

            // Assert: Should create only one pipeline type
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].metrics.total_pipelines, 3);
        }

        #[test]
        fn creates_multiple_types_for_different_job_signatures() {
            // Arrange: Create pipelines with different job signatures
            let pipeline1 =
                create_pipeline("1", "main", "push", vec![create_job("build", "build")]);
            let pipeline2 = create_pipeline("2", "main", "push", vec![create_job("test", "test")]);
            let pipeline3 =
                create_pipeline("3", "main", "push", vec![create_job("deploy", "deploy")]);
            let pipelines = vec![pipeline1, pipeline2, pipeline3];

            // Act: Group pipeline types
            let result = group_pipeline_types(&pipelines, 0, "https://gitlab.com", "org/repo");

            // Assert: Should create three different pipeline types
            assert_eq!(result.len(), 3);
            assert!(result.iter().all(|pt| pt.metrics.total_pipelines == 1));
        }

        #[test]
        fn filters_by_min_type_percentage() {
            // Arrange: Create 10 pipelines, 8 with one signature, 2 with another
            let mut pipelines = vec![];
            for i in 0..8 {
                pipelines.push(create_pipeline(
                    &i.to_string(),
                    "main",
                    "push",
                    vec![create_job("build", "build"), create_job("test", "test")],
                ));
            }
            for i in 8..10 {
                pipelines.push(create_pipeline(
                    &i.to_string(),
                    "main",
                    "push",
                    vec![create_job("deploy", "deploy")],
                ));
            }

            // Act: Group with 25% minimum threshold
            let result = group_pipeline_types(&pipelines, 25, "https://gitlab.com", "org/repo");

            // Assert: Only the type with 80% (8/10) should be included
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].metrics.total_pipelines, 8);
        }

        #[test]
        fn calculates_percentage_correctly() {
            // Arrange: Create 100 pipelines with different distributions
            let mut pipelines = vec![];
            for i in 0..50 {
                pipelines.push(create_pipeline(
                    &i.to_string(),
                    "main",
                    "push",
                    vec![create_job("build", "build")],
                ));
            }
            for i in 50..80 {
                pipelines.push(create_pipeline(
                    &i.to_string(),
                    "main",
                    "push",
                    vec![create_job("test", "test")],
                ));
            }
            for i in 80..100 {
                pipelines.push(create_pipeline(
                    &i.to_string(),
                    "main",
                    "push",
                    vec![create_job("deploy", "deploy")],
                ));
            }

            // Act: Group pipeline types
            let result = group_pipeline_types(&pipelines, 0, "https://gitlab.com", "org/repo");

            // Assert: Should have correct percentages
            assert_eq!(result.len(), 3);
            let build_type = result
                .iter()
                .find(|pt| pt.metrics.total_pipelines == 50)
                .unwrap();
            let test_type = result
                .iter()
                .find(|pt| pt.metrics.total_pipelines == 30)
                .unwrap();
            let deploy_type = result
                .iter()
                .find(|pt| pt.metrics.total_pipelines == 20)
                .unwrap();

            assert!((build_type.metrics.percentage - 50.0).abs() < 0.01);
            assert!((test_type.metrics.percentage - 30.0).abs() < 0.01);
            assert!((deploy_type.metrics.percentage - 20.0).abs() < 0.01);
        }

        #[test]
        fn sorts_by_total_pipelines_descending() {
            // Arrange: Create pipelines with different frequencies
            let mut pipelines = vec![];
            for i in 0..5 {
                pipelines.push(create_pipeline(
                    &i.to_string(),
                    "main",
                    "push",
                    vec![create_job("build", "build")],
                ));
            }
            for i in 5..15 {
                pipelines.push(create_pipeline(
                    &i.to_string(),
                    "main",
                    "push",
                    vec![create_job("test", "test")],
                ));
            }
            for i in 15..20 {
                pipelines.push(create_pipeline(
                    &i.to_string(),
                    "main",
                    "push",
                    vec![create_job("deploy", "deploy")],
                ));
            }

            // Act: Group pipeline types
            let result = group_pipeline_types(&pipelines, 0, "https://gitlab.com", "org/repo");

            // Assert: Should be sorted by total_pipelines descending
            assert_eq!(result.len(), 3);
            assert_eq!(result[0].metrics.total_pipelines, 10); // test
            assert_eq!(result[1].metrics.total_pipelines, 5); // deploy
            assert_eq!(result[2].metrics.total_pipelines, 5); // build
        }

        #[test]
        fn handles_min_type_percentage_of_100() {
            // Arrange: Create pipelines with no type reaching 100%
            let pipeline1 =
                create_pipeline("1", "main", "push", vec![create_job("build", "build")]);
            let pipeline2 = create_pipeline("2", "main", "push", vec![create_job("test", "test")]);
            let pipelines = vec![pipeline1, pipeline2];

            // Act: Group with 100% threshold
            let result = group_pipeline_types(&pipelines, 100, "https://gitlab.com", "org/repo");

            // Assert: Should return empty since no type is 100%
            assert!(result.is_empty());
        }

        #[test]
        fn groups_pipelines_with_same_jobs_in_different_order() {
            // Arrange: Create pipelines with same jobs but potentially different order
            let pipeline1 = create_pipeline(
                "1",
                "main",
                "push",
                vec![create_job("build", "build"), create_job("test", "test")],
            );
            let pipeline2 = create_pipeline(
                "2",
                "main",
                "push",
                vec![create_job("test", "test"), create_job("build", "build")],
            );
            let pipelines = vec![pipeline1, pipeline2];

            // Act: Group pipeline types
            let result = group_pipeline_types(&pipelines, 0, "https://gitlab.com", "org/repo");

            // Assert: Should group together since signatures are the same (BTreeSet sorts)
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].metrics.total_pipelines, 2);
        }
    }
}
