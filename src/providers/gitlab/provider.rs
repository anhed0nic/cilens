use chrono::{DateTime, Utc};
use log::{info, warn};

use crate::auth::Token;
use crate::error::Result;
use crate::insights::CIInsights;
use crate::providers::gitlab::client::pipelines::{fetch_pipeline_jobs, fetch_pipelines};
use crate::providers::gitlab::client::GitLabClient;

use super::progress_bar::PhaseProgress;
use super::types::{GitLabJob, GitLabPipeline};

/// GitLab CI/CD insights provider.
///
/// Fetches pipeline and job data from GitLab's GraphQL API and calculates
/// comprehensive metrics including percentiles, success rates, flakiness detection,
/// and time-to-feedback analysis.
pub struct GitLabProvider {
    pub client: GitLabClient,
    pub project_path: String,
}

impl GitLabProvider {
    /// Creates a new GitLab provider for the specified project.
    ///
    /// # Arguments
    ///
    /// * `base_url` - GitLab instance base URL (e.g., <https://gitlab.com>)
    /// * `project_path` - Project path (e.g., "group/project")
    /// * `token` - Optional authentication token
    ///
    /// # Errors
    ///
    /// Returns an error if the GraphQL endpoint URL cannot be constructed.
    pub fn new(base_url: &str, project_path: String, token: Option<Token>) -> Result<Self> {
        let client = GitLabClient::new(base_url, token)?;

        Ok(Self {
            client,
            project_path,
        })
    }

    async fn fetch_pipelines(
        &self,
        limit: usize,
        ref_: Option<&str>,
        updated_after: Option<DateTime<Utc>>,
        updated_before: Option<DateTime<Utc>>,
    ) -> Result<Vec<GitLabPipeline>> {
        info!("Fetching up to {limit} pipelines...");

        let pipeline_nodes = self
            .client
            .fetch_pipelines(
                &self.project_path,
                limit,
                ref_,
                updated_after,
                updated_before,
            )
            .await?;

        info!(
            "Fetching jobs for {} pipelines in parallel...",
            pipeline_nodes.len()
        );

        // Fetch jobs for all pipelines concurrently
        let futures: Vec<_> = pipeline_nodes
            .into_iter()
            .map(|node| self.transform_pipeline_with_jobs(node))
            .collect();

        let results = futures::future::join_all(futures).await;

        // Collect successful results, filtering out pipelines without duration
        let pipelines: Vec<_> = results
            .into_iter()
            .filter_map(Result::transpose)
            .collect::<Result<_>>()?;

        info!("Processed {} pipelines", pipelines.len());

        Ok(pipelines)
    }

    async fn transform_pipeline_with_jobs(
        &self,
        node: fetch_pipelines::FetchPipelinesProjectPipelinesNodes,
    ) -> Result<Option<GitLabPipeline>> {
        // Only include pipelines with duration
        let Some(duration) = node.duration else {
            return Ok(None);
        };

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let duration = duration as usize;

        // Fetch all jobs for this pipeline
        let job_nodes = self
            .client
            .fetch_pipeline_jobs(&self.project_path, &node.id)
            .await?;

        let jobs = Self::transform_job_nodes(job_nodes);

        // Extract stage order from pipeline metadata
        let stages = node
            .stages
            .map(|stages_conn| {
                stages_conn
                    .nodes
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter_map(|stage| stage.name)
                    .collect()
            })
            .unwrap_or_default();

        Ok(Some(GitLabPipeline {
            id: node.id,
            ref_: node.ref_.unwrap_or_default(),
            source: node.source.unwrap_or_default(),
            status: format!("{:?}", node.status).to_lowercase(),
            duration,
            stages,
            jobs,
        }))
    }

    fn transform_job_nodes(
        job_nodes: Vec<fetch_pipeline_jobs::FetchPipelineJobsProjectPipelineJobsNodes>,
    ) -> Vec<GitLabJob> {
        job_nodes
            .into_iter()
            .map(|job_node| {
                #[allow(clippy::cast_precision_loss)]
                GitLabJob {
                    id: job_node.id.unwrap_or_default(),
                    name: job_node.name.unwrap_or_default(),
                    stage: job_node.stage.and_then(|s| s.name).unwrap_or_default(),
                    duration: job_node.duration.unwrap_or(0) as f64,
                    status: job_node
                        .status
                        .map(|s| format!("{s:?}"))
                        .unwrap_or_default(),
                    retried: job_node.retried.unwrap_or(false),
                    needs: job_node.needs.map(|needs_conn| {
                        needs_conn
                            .nodes
                            .into_iter()
                            .flatten()
                            .flatten()
                            .filter_map(|need| need.name)
                            .collect()
                    }),
                }
            })
            .collect()
    }

    /// Collects comprehensive CI/CD insights for the configured project.
    ///
    /// Fetches pipelines and their jobs from GitLab, then analyzes them to calculate
    /// metrics including duration percentiles, success rates, flakiness detection,
    /// time-to-feedback, and job dependencies.
    ///
    /// Progress is displayed in three phases:
    /// 1. Fetching pipelines (SUCCESS and FAILED statuses)
    /// 2. Fetching jobs for each pipeline
    /// 3. Processing insights (grouping, calculating metrics)
    ///
    /// # Arguments
    ///
    /// * `limit` - Maximum number of pipelines to fetch (fetches SUCCESS and FAILED concurrently until limit reached)
    /// * `ref_` - Optional git ref filter (e.g., "main", "develop")
    /// * `updated_after` - Optional start date for pipeline filtering
    /// * `updated_before` - Optional end date for pipeline filtering
    /// * `min_type_percentage` - Minimum percentage (0-100) for pipeline type inclusion
    ///
    /// # Returns
    ///
    /// Returns `CIInsights` containing pipeline types grouped by job signature,
    /// with comprehensive metrics for each type and job.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - GraphQL API requests fail after 30 retries
    /// - Project or pipeline data is not found
    /// - Network or parsing errors occur
    pub async fn collect_insights(
        &self,
        limit: usize,
        ref_: Option<&str>,
        updated_after: Option<DateTime<Utc>>,
        updated_before: Option<DateTime<Utc>>,
        min_type_percentage: u8,
    ) -> Result<CIInsights> {
        info!(
            "Starting insights collection for project: {}",
            self.project_path
        );

        // Phase 1: Fetching pipelines
        let progress = PhaseProgress::start_phase_1(limit);

        let pipelines = self
            .fetch_pipelines(limit, ref_, updated_after, updated_before)
            .await?;

        if pipelines.is_empty() {
            warn!("No pipelines found for project: {}", self.project_path);
        }

        // Phase 2: Fetching jobs
        let progress = progress.finish_phase_1_start_phase_2(pipelines.len());

        // Extract base URL from graphql_url (e.g., https://gitlab.com/api/graphql -> https://gitlab.com)
        let base_url = self.client.graphql_url.origin().ascii_serialization();

        let pipeline_types = super::pipeline_types::group_pipeline_types(
            &pipelines,
            min_type_percentage,
            &base_url,
            &self.project_path,
        );

        // Phase 3: Processing data
        let progress = progress.finish_phase_2_start_phase_3();

        let insights = CIInsights {
            provider: "GitLab".to_string(),
            project: self.project_path.clone(),
            collected_at: Utc::now(),
            total_pipelines: pipelines.len(),
            total_pipeline_types: pipeline_types.len(),
            pipeline_types,
        };

        progress.finish_phase_3();

        Ok(insights)
    }
}
