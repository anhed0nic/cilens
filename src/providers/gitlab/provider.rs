use chrono::Utc;
use futures::{stream, StreamExt, TryStreamExt};
use log::{info, warn};
use serde::Deserialize;

use crate::auth::Token;
use crate::error::Result;
use crate::insights::{CIInsights, PipelineSummary};
use crate::providers::gitlab::client::GitLabClient;

const CONCURRENCY: usize = 10;

#[derive(Debug, Deserialize)]
pub struct GitLabPipeline {
    status: String,
    duration: Option<f64>,
}

pub struct GitLabProvider {
    client: GitLabClient,
    project_id: String,
}

impl GitLabProvider {
    pub fn new(base_url: String, project_id: String, token: Option<Token>) -> Result<Self> {
        let client = GitLabClient::new(&base_url, token)?;

        Ok(Self { client, project_id })
    }

    async fn fetch_pipelines(
        &self,
        limit: usize,
        branch: Option<&str>,
    ) -> Result<Vec<GitLabPipeline>> {
        let mut all_pipelines = Vec::with_capacity(limit);
        let mut page = 1;
        let per_page = 100;

        info!("Fetching up to {} pipelines...", limit);

        loop {
            let pipeline_ids = self
                .client
                .fetch_pipeline_ids_page(&self.project_id, page, per_page, branch)
                .await?;

            if pipeline_ids.is_empty() {
                info!("No more pipelines returned by API, stopping");
                break;
            }

            let pipelines = stream::iter(pipeline_ids)
                .map(|id| async move { self.client.fetch_pipeline(&self.project_id, &id).await })
                .buffer_unordered(CONCURRENCY)
                .try_collect::<Vec<_>>()
                .await?;

            let remaining = limit.saturating_sub(all_pipelines.len());
            all_pipelines.extend(pipelines.into_iter().take(remaining));

            info!(
                "Page {}: fetched {} pipelines (total: {})",
                page,
                all_pipelines.len().min(per_page as usize),
                all_pipelines.len()
            );

            if all_pipelines.len() >= limit {
                break;
            }

            page += 1;
        }

        Ok(all_pipelines
            .into_iter()
            .map(|p| GitLabPipeline {
                status: p.status,
                duration: p.duration,
            })
            .collect())
    }

    fn calculate_summary(&self, pipelines: &[GitLabPipeline]) -> PipelineSummary {
        let total_pipelines = pipelines.len();
        let successful_pipelines = pipelines.iter().filter(|p| p.status == "success").count();
        let failed_pipelines = pipelines.iter().filter(|p| p.status == "failed").count();

        let pipeline_success_rate = if total_pipelines > 0 {
            (successful_pipelines as f64 / total_pipelines as f64) * 100.0
        } else {
            0.0
        };

        let average_pipeline_duration = pipelines.iter().filter_map(|p| p.duration).sum::<f64>()
            / total_pipelines.max(1) as f64;

        PipelineSummary {
            total_pipelines,
            successful_pipelines,
            failed_pipelines,
            pipeline_success_rate,
            average_pipeline_duration,
        }
    }

    pub async fn collect_insights(
        &self,
        project: &str,
        limit: usize,
        branch: Option<&str>,
    ) -> Result<CIInsights> {
        info!("Starting insights collection for project: {}", project);

        let pipelines = self.fetch_pipelines(limit, branch).await?;

        if pipelines.is_empty() {
            warn!("No pipelines found for project: {}", project);
        }

        let pipeline_summary = self.calculate_summary(&pipelines);

        Ok(CIInsights {
            provider: "GitLab".to_string(),
            project: project.to_string(),
            collected_at: Utc::now(),
            pipelines_analyzed: pipelines.len(),
            pipeline_summary,
        })
    }
}
