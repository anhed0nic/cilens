use chrono::{DateTime, Utc};
use graphql_client::GraphQLQuery;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use super::core::{GitLabClient, PAGE_SIZE};
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

impl GitLabClient {
    #[allow(clippy::too_many_lines, clippy::too_many_arguments)]
    async fn fetch_pipelines_with_status(
        &self,
        project_path: &str,
        limit: usize,
        ref_: Option<&str>,
        status: Option<fetch_pipelines::PipelineStatusEnum>,
        updated_after: Option<DateTime<Utc>>,
        updated_before: Option<DateTime<Utc>>,
        shared_counter: Option<Arc<AtomicUsize>>,
    ) -> Result<Vec<fetch_pipelines::FetchPipelinesProjectPipelinesNodes>> {
        let mut all_pipelines = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            // Check shared counter if provided (for coordinated fetching)
            if let Some(ref counter) = shared_counter {
                if counter.load(Ordering::Relaxed) >= limit {
                    break;
                }
            }

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

            let project = data
                .project
                .ok_or_else(|| CILensError::ProjectNotFound(project_path.to_string()))?;

            let pipelines = project
                .pipelines
                .ok_or_else(|| CILensError::NoPipelineData(project_path.to_string()))?;

            let fetched_count = pipelines.nodes.iter().flatten().flatten().count();
            all_pipelines.extend(pipelines.nodes.into_iter().flatten().flatten());

            // Update shared counter if provided
            if let Some(ref counter) = shared_counter {
                counter.fetch_add(fetched_count, Ordering::Relaxed);
            }

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
        // Fetch SUCCESS and FAILED pipelines in parallel with shared counter
        // Both tasks will stop when combined total reaches limit
        let shared_counter = Arc::new(AtomicUsize::new(0));

        let (success_result, failed_result) = tokio::join!(
            self.fetch_pipelines_with_status(
                project_path,
                limit,
                ref_,
                Some(fetch_pipelines::PipelineStatusEnum::SUCCESS),
                updated_after,
                updated_before,
                Some(Arc::clone(&shared_counter)),
            ),
            self.fetch_pipelines_with_status(
                project_path,
                limit,
                ref_,
                Some(fetch_pipelines::PipelineStatusEnum::FAILED),
                updated_after,
                updated_before,
                Some(Arc::clone(&shared_counter)),
            ),
        );

        let mut all_pipelines = success_result?;
        all_pipelines.extend(failed_result?);

        // Truncate to exact limit (both tasks may have fetched slightly over due to page granularity)
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
            #[allow(clippy::cast_possible_wrap)]
            let variables = fetch_pipeline_jobs::Variables {
                project_path: project_path.to_string(),
                pipeline_id: pipeline_id.to_string(),
                first: PAGE_SIZE as i64,
                after: cursor.clone(),
            };

            let request_body = FetchPipelineJobs::build_query(variables);

            let data: fetch_pipeline_jobs::ResponseData =
                self.execute_graphql_request(&request_body).await?;

            let project = data
                .project
                .ok_or_else(|| CILensError::ProjectNotFound(project_path.to_string()))?;

            let pipeline = project
                .pipeline
                .ok_or_else(|| CILensError::PipelineNotFound(pipeline_id.to_string()))?;

            let jobs = pipeline
                .jobs
                .ok_or_else(|| CILensError::NoJobData(pipeline_id.to_string()))?;

            all_jobs.extend(jobs.nodes.into_iter().flatten().flatten());

            if !jobs.page_info.has_next_page {
                break;
            }

            cursor = jobs.page_info.end_cursor;
        }

        Ok(all_jobs)
    }
}
