use chrono::{DateTime, Utc};
use graphql_client::GraphQLQuery;

use super::core::GitLabClient;
use crate::error::{CILensError, Result};

pub type JobID = String;
pub type CiPipelineID = String;
pub type Time = DateTime<Utc>;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/providers/gitlab/client/schema.json",
    query_path = "src/providers/gitlab/client/pipelines.graphql",
    response_derives = "Debug,PartialEq,Clone"
)]
pub struct FetchPipelines;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/providers/gitlab/client/schema.json",
    query_path = "src/providers/gitlab/client/pipelines.graphql",
    query_name = "FetchPipelineJobs",
    response_derives = "Debug,PartialEq,Clone"
)]
pub struct FetchPipelineJobs;

const PAGE_SIZE: usize = 50;

impl GitLabClient {
    #[allow(clippy::too_many_lines)]
    async fn fetch_pipelines_with_status(
        &self,
        project_path: &str,
        limit: usize,
        ref_: Option<&str>,
        status: Option<fetch_pipelines::PipelineStatusEnum>,
        updated_after: Option<DateTime<Utc>>,
        updated_before: Option<DateTime<Utc>>,
    ) -> Result<Vec<fetch_pipelines::FetchPipelinesProjectPipelinesNodes>> {
        let mut all_pipelines = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let remaining = limit.saturating_sub(all_pipelines.len());
            if remaining == 0 {
                break;
            }

            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            let fetch_count = std::cmp::min(remaining, PAGE_SIZE) as i64;

            let variables = fetch_pipelines::Variables {
                project_path: project_path.to_string(),
                first: fetch_count,
                after: cursor.clone(),
                ref_: ref_.map(ToString::to_string),
                status: status.clone(),
                updated_after,
                updated_before,
            };

            let request_body = FetchPipelines::build_query(variables);

            let data: fetch_pipelines::ResponseData =
                self.execute_graphql_request(&request_body).await?;

            let project = data.project.ok_or_else(|| {
                CILensError::Config(format!("Project '{project_path}' not found"))
            })?;

            let pipelines = project.pipelines.ok_or_else(|| {
                CILensError::Config(format!(
                    "No pipeline data available for project '{project_path}'"
                ))
            })?;

            all_pipelines.extend(pipelines.nodes.into_iter().flatten().flatten());

            // Stop if we have enough pipelines or no more pages
            if all_pipelines.len() >= limit || !pipelines.page_info.has_next_page {
                break;
            }

            cursor = pipelines.page_info.end_cursor;
        }

        all_pipelines.truncate(limit);

        Ok(all_pipelines)
    }

    pub async fn fetch_pipelines(
        &self,
        project_path: &str,
        limit: usize,
        ref_: Option<&str>,
        updated_after: Option<DateTime<Utc>>,
        updated_before: Option<DateTime<Utc>>,
    ) -> Result<Vec<fetch_pipelines::FetchPipelinesProjectPipelinesNodes>> {
        // Fetch SUCCESS and FAILED pipelines in parallel
        let half_limit = limit / 2;

        let (success_result, failed_result) = tokio::join!(
            self.fetch_pipelines_with_status(
                project_path,
                half_limit,
                ref_,
                Some(fetch_pipelines::PipelineStatusEnum::SUCCESS),
                updated_after,
                updated_before,
            ),
            self.fetch_pipelines_with_status(
                project_path,
                half_limit,
                ref_,
                Some(fetch_pipelines::PipelineStatusEnum::FAILED),
                updated_after,
                updated_before,
            ),
        );

        let mut all_pipelines = success_result?;
        all_pipelines.extend(failed_result?);

        // Note: Pipelines are already sorted by creation time in GitLab's response
        all_pipelines.truncate(limit);

        Ok(all_pipelines)
    }

    #[allow(clippy::too_many_lines)]
    pub async fn fetch_pipeline_jobs(
        &self,
        project_path: &str,
        pipeline_id: &str,
    ) -> Result<Vec<fetch_pipeline_jobs::FetchPipelineJobsProjectPipelineJobsNodes>> {
        let mut all_jobs = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let variables = fetch_pipeline_jobs::Variables {
                project_path: project_path.to_string(),
                pipeline_id: pipeline_id.to_string(),
                first: PAGE_SIZE as i64,
                after: cursor.clone(),
            };

            let request_body = FetchPipelineJobs::build_query(variables);

            let data: fetch_pipeline_jobs::ResponseData =
                self.execute_graphql_request(&request_body).await?;

            let project = data.project.ok_or_else(|| {
                CILensError::Config(format!("Project '{project_path}' not found"))
            })?;

            let pipeline = project.pipeline.ok_or_else(|| {
                CILensError::Config(format!("Pipeline '{pipeline_id}' not found"))
            })?;

            let jobs = pipeline.jobs.ok_or_else(|| {
                CILensError::Config(format!(
                    "No job data available for pipeline '{pipeline_id}'"
                ))
            })?;

            all_jobs.extend(jobs.nodes.into_iter().flatten().flatten());

            if !jobs.page_info.has_next_page {
                break;
            }

            cursor = jobs.page_info.end_cursor;
        }

        Ok(all_jobs)
    }
}
