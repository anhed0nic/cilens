use chrono::Utc;
use log::{info, warn};

use super::core::GitLabProvider;
use crate::error::Result;
use crate::insights::{CIInsights, PipelineSummary};
use crate::providers::gitlab::client::graphql::fetch_pipelines;

/// Represents a GitLab pipeline with its jobs
#[derive(Debug)]
pub struct GitLabPipeline {
    pub status: String,
    pub duration: usize,
    #[allow(dead_code)]
    pub jobs: Vec<GitLabJob>,
}

/// Represents a job within a GitLab pipeline
#[derive(Debug)]
#[allow(dead_code)]
pub struct GitLabJob {
    pub name: String,
    pub status: String,
    pub duration: f64,
    pub needs: Vec<String>,
}

impl GitLabProvider {
    /// Fetch pipelines using GraphQL API
    ///
    /// This method fetches pipelines with their jobs and dependencies in a single query,
    /// filtering for valid pipelines (success/failed status with duration data).
    ///
    /// # Arguments
    /// * `limit` - Maximum number of pipelines to fetch
    /// * `branch` - Optional branch name to filter pipelines
    ///
    /// # Returns
    /// * `Result<Vec<GitLabPipeline>>` - Vector of valid pipelines or an error
    pub async fn fetch_pipelines(
        &self,
        limit: usize,
        branch: Option<&str>,
    ) -> Result<Vec<GitLabPipeline>> {
        info!("Fetching up to {limit} pipelines...");

        // Fetch pipeline nodes using GraphQL
        let pipeline_nodes = self
            .client
            .fetch_pipelines_graphql(&self.project_path, limit, branch)
            .await?;

        // Transform and filter pipeline nodes
        let pipelines: Vec<GitLabPipeline> = pipeline_nodes
            .into_iter()
            .filter_map(|node| {
                // Only include pipelines with success or failed status and valid duration
                if (node.status == fetch_pipelines::PipelineStatusEnum::SUCCESS
                    || node.status == fetch_pipelines::PipelineStatusEnum::FAILED)
                    && node.duration.is_some()
                {
                    let duration = node.duration.unwrap() as usize;

                    // Transform jobs if available
                    let jobs = node
                        .jobs
                        .map(|job_conn| {
                            job_conn
                                .nodes
                                .into_iter()
                                .flatten()
                                .flatten() // job_node is Option<T>
                                .filter_map(|job_node| {
                                    // Only include jobs with valid duration
                                    job_node.duration.map(|dur| {
                                        // Extract job dependency names
                                        let needs = job_node
                                            .needs
                                            .map(|needs_conn| {
                                                needs_conn
                                                    .nodes
                                                    .into_iter()
                                                    .flatten()
                                                    .flatten()
                                                    .filter_map(|need| need.name)
                                                    .collect()
                                            })
                                            .unwrap_or_default();

                                        GitLabJob {
                                            name: job_node.name.unwrap_or_default(),
                                            status: format!("{:?}", job_node.status),
                                            duration: dur as f64,
                                            needs,
                                        }
                                    })
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    Some(GitLabPipeline {
                        status: format!("{:?}", node.status).to_lowercase(),
                        duration,
                        jobs,
                    })
                } else {
                    None
                }
            })
            .collect();

        info!("Fetched {} valid pipelines", pipelines.len());

        Ok(pipelines)
    }

    /// Calculate summary statistics from a collection of pipelines
    fn calculate_summary(pipelines: &[GitLabPipeline]) -> PipelineSummary {
        let total_pipelines = pipelines.len();
        let successful_pipelines = pipelines.iter().filter(|p| p.status == "success").count();
        let failed_pipelines = pipelines.iter().filter(|p| p.status == "failed").count();

        #[allow(clippy::cast_precision_loss)]
        let pipeline_success_rate = if total_pipelines > 0 {
            (successful_pipelines as f64 / total_pipelines as f64) * 100.0
        } else {
            0.0
        };

        #[allow(clippy::cast_precision_loss)]
        let average_pipeline_duration = pipelines.iter().map(|p| p.duration as f64).sum::<f64>()
            / total_pipelines.max(1) as f64;

        PipelineSummary {
            total_pipelines,
            successful_pipelines,
            failed_pipelines,
            pipeline_success_rate,
            average_pipeline_duration,
        }
    }

    /// Collect CI/CD insights for the project
    ///
    /// # Arguments
    /// * `limit` - Maximum number of pipelines to analyze
    /// * `branch` - Optional branch name to filter pipelines
    ///
    /// # Returns
    /// * `Result<CIInsights>` - Aggregated insights or an error
    pub async fn collect_insights(
        &self,
        limit: usize,
        branch: Option<&str>,
    ) -> Result<CIInsights> {
        info!(
            "Starting insights collection for project: {}",
            self.project_path
        );

        let pipelines = self.fetch_pipelines(limit, branch).await?;

        if pipelines.is_empty() {
            warn!("No pipelines found for project: {}", self.project_path);
        }

        let pipeline_summary = Self::calculate_summary(&pipelines);

        Ok(CIInsights {
            provider: "GitLab".to_string(),
            project: self.project_path.clone(),
            collected_at: Utc::now(),
            pipelines_analyzed: pipelines.len(),
            pipeline_summary,
        })
    }
}
