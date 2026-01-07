use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// GitHub Actions workflow run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubWorkflowRun {
    /// Unique identifier for the workflow run
    pub id: u64,
    /// Name of the workflow
    pub name: Option<String>,
    /// Head branch or tag name
    pub head_branch: Option<String>,
    /// SHA of the head commit
    pub head_sha: String,
    /// Path to the workflow file
    pub path: String,
    /// Display title for the run
    pub display_title: String,
    /// Run number
    pub run_number: u64,
    /// Event that triggered the run
    pub event: String,
    /// Status of the run
    pub status: String,
    /// Conclusion of the run (success, failure, etc.)
    pub conclusion: Option<String>,
    /// Number of jobs in the workflow
    pub jobs_count: usize,
    /// Jobs in this workflow run
    pub jobs: Vec<GitHubJob>,
    /// When the run was created
    pub created_at: DateTime<Utc>,
    /// When the run was updated
    pub updated_at: DateTime<Utc>,
    /// Total duration in seconds
    pub duration: u64,
}

/// Job within a GitHub Actions workflow run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubJob {
    /// Unique identifier for the job
    pub id: u64,
    /// Name of the job
    pub name: String,
    /// Status of the job
    pub status: String,
    /// Conclusion of the job
    pub conclusion: Option<String>,
    /// When the job started
    pub started_at: Option<DateTime<Utc>>,
    /// When the job completed
    pub completed_at: Option<DateTime<Utc>>,
    /// Steps in this job
    pub steps: Vec<GitHubStep>,
    /// Labels for the runner
    pub labels: Vec<String>,
}

/// Step within a GitHub Actions job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubStep {
    /// Name of the step
    pub name: String,
    /// Status of the step
    pub status: String,
    /// Conclusion of the step
    pub conclusion: Option<String>,
    /// When the step started
    pub started_at: Option<DateTime<Utc>>,
    /// When the step completed
    pub completed_at: Option<DateTime<Utc>>,
    /// Step number
    pub number: u32,
}

/// Links for GitHub resources.
pub mod links {
    use super::*;

    /// Generate URL for a workflow run.
    pub fn workflow_run_url(owner: &str, repo: &str, run_id: u64) -> String {
        format!("https://github.com/{}/{}/actions/runs/{}", owner, repo, run_id)
    }

    /// Generate URL for a job.
    pub fn job_url(owner: &str, repo: &str, job_id: u64) -> String {
        format!("https://github.com/{}/{}/actions/runs/{}", owner, repo, job_id)
    }
}