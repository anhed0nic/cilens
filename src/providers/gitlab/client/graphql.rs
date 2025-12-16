use graphql_client::GraphQLQuery;

use super::core::GitLabClient;
use crate::error::{CILensError, Result};

/// GraphQL query for fetching pipelines with jobs and dependencies
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/gitlab_schema.json",
    query_path = "graphql/pipelines.graphql",
    response_derives = "Debug,PartialEq"
)]
pub struct FetchPipelines;

impl GitLabClient {
    /// Fetch pipelines using GraphQL with cursor-based pagination
    ///
    /// # Arguments
    /// * `project_path` - The full path of the project (e.g., "group/project")
    /// * `limit` - Maximum number of pipelines to fetch
    /// * `branch` - Optional branch name to filter pipelines
    ///
    /// # Returns
    /// * `Result<Vec<PipelineNode>>` - Vector of pipeline nodes or an error
    ///
    /// # Errors
    /// Returns an error if:
    /// * The GraphQL query fails
    /// * The project is not found
    /// * The response cannot be deserialized
    pub async fn fetch_pipelines_graphql(
        &self,
        project_path: &str,
        limit: usize,
        branch: Option<&str>,
    ) -> Result<Vec<fetch_pipelines::FetchPipelinesProjectPipelinesNodes>> {
        let mut all_pipelines = Vec::new();
        let mut cursor: Option<String> = None;

        // GitLab GraphQL typically allows up to 100 items per page
        const PAGE_SIZE: i64 = 100;

        loop {
            // Calculate how many more pipelines we need
            let remaining = limit.saturating_sub(all_pipelines.len());
            if remaining == 0 {
                break;
            }

            // Request at most PAGE_SIZE items, but no more than what we need
            let fetch_count = std::cmp::min(remaining, PAGE_SIZE as usize) as i64;

            let variables = fetch_pipelines::Variables {
                project_path: project_path.to_string(),
                first: fetch_count,
                after: cursor.clone(),
                ref_: branch.map(|b| b.to_string()),
            };

            let request_body = FetchPipelines::build_query(variables);

            let request = self.client.post(self.graphql_url.clone()).json(&request_body);
            let request = self.auth_request(request);

            let response = request.send().await?;
            let response_body: graphql_client::Response<fetch_pipelines::ResponseData> =
                response.json().await?;

            // Check for GraphQL errors
            if let Some(errors) = response_body.errors {
                let error_messages: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
                return Err(CILensError::Config(format!(
                    "GraphQL errors: {}",
                    error_messages.join(", ")
                )));
            }

            // Extract data
            let data = response_body.data.ok_or_else(|| {
                CILensError::Config("GraphQL response contained no data".to_string())
            })?;

            // Extract project data
            let project = data.project.ok_or_else(|| {
                CILensError::Config(format!("Project '{}' not found", project_path))
            })?;

            // Extract pipelines
            let pipelines = project.pipelines.ok_or_else(|| {
                CILensError::Config(format!(
                    "No pipeline data available for project '{}'",
                    project_path
                ))
            })?;

            // Collect pipeline nodes (nodes is Option<Vec<Option<T>>>, so we flatten twice)
            all_pipelines.extend(pipelines.nodes.into_iter().flatten().flatten());

            // Check if there are more pages and we haven't reached the limit
            if !pipelines.page_info.has_next_page || all_pipelines.len() >= limit {
                break;
            }

            // Update cursor for next iteration
            cursor = pipelines.page_info.end_cursor;

            // Safety check: if we have an empty cursor but hasNextPage is true, break to avoid infinite loop
            if cursor.is_none() {
                break;
            }
        }

        // Ensure we don't return more than the requested limit
        all_pipelines.truncate(limit);

        Ok(all_pipelines)
    }
}
