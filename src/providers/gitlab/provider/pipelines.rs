use chrono::Utc;
use log::{info, warn};

use super::core::GitLabProvider;
use crate::error::Result;
use crate::insights::{CIInsights, PipelineSummary};
use crate::providers::gitlab::client::pipelines::fetch_pipelines;

#[derive(Debug)]
pub struct GitLabPipeline {
    pub status: String,
    pub duration: usize,
    #[allow(dead_code)]
    pub jobs: Vec<GitLabJob>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct GitLabJob {
    pub name: String,
    pub status: String,
    pub duration: f64,
    pub needs: Vec<String>,
}

impl GitLabProvider {
    pub async fn fetch_pipelines(
        &self,
        limit: usize,
        ref_: Option<&str>,
    ) -> Result<Vec<GitLabPipeline>> {
        info!("Fetching up to {limit} pipelines...");

        let pipeline_nodes = self
            .client
            .fetch_pipelines_graphql(&self.project_path, limit, ref_)
            .await?;

        let pipelines: Vec<GitLabPipeline> = pipeline_nodes
            .into_iter()
            .filter_map(Self::transform_pipeline_node)
            .collect();

        info!("Processed {} pipelines", pipelines.len());

        Ok(pipelines)
    }

    fn is_valid_pipeline(node: &fetch_pipelines::FetchPipelinesProjectPipelinesNodes) -> bool {
        (node.status == fetch_pipelines::PipelineStatusEnum::SUCCESS
            || node.status == fetch_pipelines::PipelineStatusEnum::FAILED)
            && node.duration.is_some()
    }

    fn transform_pipeline_node(
        node: fetch_pipelines::FetchPipelinesProjectPipelinesNodes,
    ) -> Option<GitLabPipeline> {
        if !Self::is_valid_pipeline(&node) {
            return None;
        }

        let duration = node.duration.unwrap() as usize;
        let jobs = Self::transform_jobs(node.jobs);

        Some(GitLabPipeline {
            status: format!("{:?}", node.status).to_lowercase(),
            duration,
            jobs,
        })
    }

    fn transform_jobs(
        job_conn: Option<fetch_pipelines::FetchPipelinesProjectPipelinesNodesJobs>,
    ) -> Vec<GitLabJob> {
        job_conn
            .map(|conn| {
                conn.nodes
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter_map(Self::transform_job_node)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn transform_job_node(
        job_node: fetch_pipelines::FetchPipelinesProjectPipelinesNodesJobsNodes,
    ) -> Option<GitLabJob> {
        job_node.duration.map(|dur| GitLabJob {
            name: job_node.name.unwrap_or_default(),
            status: format!("{:?}", job_node.status),
            duration: dur as f64,
            needs: Self::transform_job_needs(job_node.needs),
        })
    }

    fn transform_job_needs(
        needs_conn: Option<fetch_pipelines::FetchPipelinesProjectPipelinesNodesJobsNodesNeeds>,
    ) -> Vec<String> {
        needs_conn
            .map(|conn| {
                conn.nodes
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter_map(|need| need.name)
                    .collect()
            })
            .unwrap_or_default()
    }

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

    pub async fn collect_insights(&self, limit: usize, ref_: Option<&str>) -> Result<CIInsights> {
        info!(
            "Starting insights collection for project: {}",
            self.project_path
        );

        let pipelines = self.fetch_pipelines(limit, ref_).await?;

        if pipelines.is_empty() {
            warn!("No pipelines found for project: {}", self.project_path);
        }

        let pipeline_summary = Self::calculate_summary(&pipelines);

        Ok(CIInsights {
            provider: "GitLab".to_string(),
            project: self.project_path.clone(),
            collected_at: Utc::now(),
            pipeline_summary,
        })
    }
}
