use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Top-level CI/CD insights for a project.
///
/// Contains aggregated metrics for all pipeline types, grouped by job signature.
#[derive(Debug, Serialize, Deserialize)]
pub struct CIInsights {
    /// CI provider name (e.g., "GitLab")
    pub provider: String,
    /// Project identifier (e.g., "group/project")
    pub project: String,
    /// Timestamp when insights were collected
    pub collected_at: DateTime<Utc>,
    /// Total number of pipelines analyzed
    pub total_pipelines: usize,
    /// Number of distinct pipeline types found
    pub total_pipeline_types: usize,
    /// Detailed metrics for each pipeline type
    pub pipeline_types: Vec<PipelineType>,
}

/// A job that must complete before the current job can start.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredecessorJob {
    /// Job name
    pub name: String,
    /// Median duration in seconds
    pub duration_p50: f64,
}

/// Pipeline count with clickable URLs for investigation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PipelineCountWithLinks {
    /// Number of pipelines
    pub count: usize,
    /// GitLab URLs to individual pipelines
    pub links: Vec<String>,
}

/// Job execution count with clickable URLs for investigation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JobCountWithLinks {
    /// Number of job executions
    pub count: usize,
    /// GitLab URLs to individual job runs
    pub links: Vec<String>,
}

/// Comprehensive metrics for a specific job across multiple pipeline executions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobMetrics {
    /// Job name
    pub name: String,
    /// Pipeline type ID this job belongs to
    pub pipeline_type_id: String,
    /// Median job execution duration (seconds)
    pub duration_p50: f64,
    /// 95th percentile job duration (seconds) - useful for SLA planning
    pub duration_p95: f64,
    /// 99th percentile job duration (seconds) - captures outliers
    pub duration_p99: f64,
    /// Median time from pipeline start to job completion (seconds)
    pub time_to_feedback_p50: f64,
    /// 95th percentile time-to-feedback (seconds) - use for planning
    pub time_to_feedback_p95: f64,
    /// 99th percentile time-to-feedback (seconds) - worst-case scenario
    pub time_to_feedback_p99: f64,
    /// Jobs that must complete before this job (critical path)
    pub predecessors: Vec<PredecessorJob>,
    /// Percentage of executions that were flaky retries (0.0 if never retried)
    pub flakiness_rate: f64,
    /// Flaky retry executions with clickable URLs
    pub flaky_retries: JobCountWithLinks,
    /// Failed executions (stayed failed) with clickable URLs
    pub failed_executions: JobCountWithLinks,
    /// Percentage of executions that failed and stayed failed
    pub failure_rate: f64,
    /// Total executions across all pipelines (includes retries and failures)
    pub total_executions: usize,
}

/// A group of pipelines with identical job signatures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineType {
    /// Unique identifier for this pipeline type
    pub id: String,
    /// Human-readable label (e.g., "Production Pipeline", "Development Pipeline")
    pub label: String,
    /// Unique CI stages found in this pipeline type
    pub stages: Vec<String>,
    /// Git refs (branches/tags) that triggered these pipelines
    pub ref_patterns: Vec<String>,
    /// Pipeline trigger sources (e.g., "push", "schedule")
    pub sources: Vec<String>,
    /// Aggregated metrics for this pipeline type
    pub metrics: TypeMetrics,
}

/// Aggregated metrics for a pipeline type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeMetrics {
    /// Percentage of total pipelines that belong to this type
    pub percentage: f64,
    /// Number of pipelines in this type
    pub total_pipelines: usize,
    /// Successful pipeline runs with clickable URLs
    pub successful_pipelines: PipelineCountWithLinks,
    /// Failed pipeline runs with clickable URLs
    pub failed_pipelines: PipelineCountWithLinks,
    /// Percentage of successful pipeline runs
    pub success_rate: f64,
    /// Median pipeline duration (seconds)
    pub duration_p50: f64,
    /// 95th percentile pipeline duration (seconds)
    pub duration_p95: f64,
    /// 99th percentile pipeline duration (seconds)
    pub duration_p99: f64,
    /// Median time to first job feedback (seconds)
    pub time_to_feedback_p50: f64,
    /// 95th percentile time to first feedback (seconds)
    pub time_to_feedback_p95: f64,
    /// 99th percentile time to first feedback (seconds)
    pub time_to_feedback_p99: f64,
    /// Per-job metrics, sorted by `time_to_feedback_p95` descending
    pub jobs: Vec<JobMetrics>,
}
