use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CIInsights {
    pub provider: String,
    pub project: String,
    pub collected_at: DateTime<Utc>,
    pub total_pipelines: usize,
    pub total_pipeline_types: usize,
    pub pipeline_types: Vec<PipelineType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredecessorJob {
    pub name: String,
    pub duration_p50: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PipelineCountWithLinks {
    pub count: usize,
    pub links: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JobCountWithLinks {
    pub count: usize,
    pub links: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobMetrics {
    pub name: String,
    pub duration_p50: f64,
    pub duration_p95: f64,
    pub duration_p99: f64,
    pub time_to_feedback_p50: f64,
    pub time_to_feedback_p95: f64,
    pub time_to_feedback_p99: f64,
    pub predecessors: Vec<PredecessorJob>,
    pub flakiness_rate: f64,
    pub flaky_retries: JobCountWithLinks,
    pub failed_executions: JobCountWithLinks,
    pub failure_rate: f64,
    pub total_executions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineType {
    pub label: String,
    pub stages: Vec<String>,
    pub ref_patterns: Vec<String>,
    pub sources: Vec<String>,
    pub metrics: TypeMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeMetrics {
    pub percentage: f64,
    pub total_pipelines: usize,
    pub successful_pipelines: PipelineCountWithLinks,
    pub failed_pipelines: PipelineCountWithLinks,
    pub success_rate: f64,
    pub duration_p50: f64,
    pub duration_p95: f64,
    pub duration_p99: f64,
    pub time_to_feedback_p50: f64,
    pub time_to_feedback_p95: f64,
    pub time_to_feedback_p99: f64,
    pub jobs: Vec<JobMetrics>,
}
