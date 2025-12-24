use std::collections::HashMap;

use super::types::GitLabPipeline;
use crate::insights::PipelineType;

pub fn group_pipeline_types(
    pipelines: &[GitLabPipeline],
    min_type_percentage: f64,
) -> Vec<PipelineType> {
    let mut clusters: HashMap<Vec<String>, Vec<&GitLabPipeline>> = HashMap::new();

    // Group pipelines by their job signature
    for pipeline in pipelines {
        let mut job_names: Vec<String> = pipeline.jobs.iter().map(|j| j.name.clone()).collect();
        job_names.sort();
        job_names.dedup();

        clusters.entry(job_names).or_default().push(pipeline);
    }

    let total_pipelines = pipelines.len();
    let mut pipeline_types: Vec<PipelineType> = clusters
        .into_iter()
        .map(|(job_names, cluster_pipelines)| {
            create_pipeline_type(&job_names, &cluster_pipelines, total_pipelines)
        })
        .collect();

    // Filter out pipeline types below threshold
    pipeline_types.retain(|pt| pt.percentage >= min_type_percentage);

    pipeline_types.sort_by(|a, b| b.count.cmp(&a.count));
    pipeline_types
}

fn create_pipeline_type(
    job_names: &[String],
    pipelines: &[&GitLabPipeline],
    total_pipelines: usize,
) -> PipelineType {
    let count = pipelines.len();
    #[allow(clippy::cast_precision_loss)]
    let percentage = (count as f64 / total_pipelines.max(1) as f64) * 100.0;

    // Generate label from job names
    let label = if job_names.iter().any(|j| j.to_lowercase().contains("prod")) {
        "Production Pipeline".to_string()
    } else if job_names.iter().any(|j| {
        let lower = j.to_lowercase();
        lower.contains("staging")
            || lower.contains("dev")
            || lower.contains("test")
            || lower.contains("qa")
    }) {
        "Development Pipeline".to_string()
    } else {
        "Unknown Pipeline".to_string()
    };

    // Extract common characteristics
    let (stages, ref_patterns, sources) = extract_characteristics(pipelines);

    // Collect pipeline IDs
    let ids: Vec<String> = pipelines.iter().map(|p| p.id.clone()).collect();

    // Calculate metrics
    let metrics = super::type_metrics::calculate_type_metrics(pipelines);

    PipelineType {
        label,
        count,
        percentage,
        ids,
        stages,
        ref_patterns,
        sources,
        metrics,
    }
}

fn extract_characteristics(
    pipelines: &[&GitLabPipeline],
) -> (Vec<String>, Vec<String>, Vec<String>) {
    use std::collections::HashSet;

    // Collect all unique stages
    let stages: HashSet<String> = pipelines
        .iter()
        .flat_map(|p| p.jobs.iter().map(|j| j.stage.clone()))
        .collect();

    // Collect all unique refs
    let ref_patterns: HashSet<String> = pipelines.iter().map(|p| p.ref_.clone()).collect();

    // Collect all unique sources
    let sources: HashSet<String> = pipelines.iter().map(|p| p.source.clone()).collect();

    (
        stages.into_iter().collect(),
        ref_patterns.into_iter().collect(),
        sources.into_iter().collect(),
    )
}
