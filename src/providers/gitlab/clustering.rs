use std::collections::HashMap;

use super::core::GitLabPipeline;
use crate::insights::PipelineType;

pub fn cluster_and_analyze(
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
        format!(
            "Pipeline: {}",
            job_names
                .iter()
                .take(3)
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    // Extract common characteristics
    let (stages, ref_patterns, sources) = extract_characteristics(pipelines);

    // Collect pipeline IDs
    let ids: Vec<String> = pipelines.iter().map(|p| p.id.clone()).collect();

    // Calculate metrics
    let metrics = super::metrics::calculate_type_metrics(pipelines);

    PipelineType {
        label,
        count,
        percentage,
        jobs: job_names.to_vec(),
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
    let threshold = pipelines.len() / 10;

    let stages = extract_common(pipelines, threshold * 2, |p| {
        p.jobs.iter().map(|j| j.stage.as_str()).collect()
    });

    let ref_patterns = extract_common(pipelines, threshold, |p| vec![p.ref_.as_str()]);

    let sources = extract_common(pipelines, threshold, |p| vec![p.source.as_str()]);

    (stages, ref_patterns, sources)
}

fn extract_common<F>(pipelines: &[&GitLabPipeline], threshold: usize, extract: F) -> Vec<String>
where
    F: Fn(&GitLabPipeline) -> Vec<&str>,
{
    let mut counts: HashMap<&str, usize> = HashMap::new();

    for pipeline in pipelines {
        for value in extract(pipeline) {
            *counts.entry(value).or_insert(0) += 1;
        }
    }

    let mut items: Vec<(&str, usize)> = counts
        .into_iter()
        .filter(|(_, count)| *count >= threshold)
        .collect();

    items.sort_by(|a, b| b.1.cmp(&a.1));
    items
        .into_iter()
        .take(5)
        .map(|(name, _)| name.to_string())
        .collect()
}
