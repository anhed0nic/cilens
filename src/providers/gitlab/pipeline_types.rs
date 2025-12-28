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
        "Production Pipeline".to_string()
    } else if has_keyword(&["staging", "dev", "test", "qa"]) {
        "Development Pipeline".to_string()
    } else {
        "Unknown Pipeline".to_string()
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
