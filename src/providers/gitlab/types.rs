/// A GitLab CI/CD pipeline execution.
///
/// Represents a single pipeline run with its metadata, jobs, and execution details.
/// Used internally to analyze pipeline patterns and calculate metrics.
#[derive(Debug)]
pub struct GitLabPipeline {
    /// GraphQL Global ID (e.g., <gid://gitlab/Ci::Pipeline/123>)
    pub id: String,
    /// Git reference that triggered the pipeline (e.g., "main", "develop")
    pub ref_: String,
    /// Trigger source (e.g., "push", "schedule", "web")
    pub source: String,
    /// Final pipeline status (e.g., "success", "failed")
    pub status: String,
    /// Total pipeline duration in seconds
    pub duration: usize,
    /// Ordered list of stage names
    pub stages: Vec<String>,
    /// All jobs in this pipeline
    pub jobs: Vec<GitLabJob>,
}

/// A job within a GitLab CI/CD pipeline.
///
/// Represents a single job execution with its dependencies and execution details.
#[derive(Debug)]
pub struct GitLabJob {
    /// GraphQL Global ID (e.g., <gid://gitlab/Ci::Job/456>)
    pub id: String,
    /// Job name as defined in .gitlab-ci.yml
    pub name: String,
    /// Stage this job belongs to
    pub stage: String,
    /// Job execution duration in seconds
    pub duration: f64,
    /// Final job status (e.g., "SUCCESS", "FAILED")
    pub status: String,
    /// Whether this job was retried (flaky job indicator)
    pub retried: bool,
    /// Explicit job dependencies via `needs` keyword
    pub needs: Option<Vec<String>>,
}
