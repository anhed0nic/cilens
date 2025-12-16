use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CIInsights {
    pub provider: String,
    pub project: String,
    pub collected_at: DateTime<Utc>,
    pub pipeline_summary: PipelineSummary,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PipelineSummary {
    pub total_pipelines: usize,
    pub successful_pipelines: usize,
    pub failed_pipelines: usize,
    pub pipeline_success_rate: f64,
    pub average_successful_pipeline_duration_seconds: f64,
    pub average_critical_path_duration_seconds: f64,
    pub example_critical_path: Option<CriticalPath>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalPath {
    pub jobs: Vec<String>,
    pub total_duration_seconds: f64,
}
