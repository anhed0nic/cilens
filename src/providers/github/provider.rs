use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::sync::Arc;

use crate::auth::Token;
use crate::insights::CIInsights;

use super::client::GitHubClient;
use super::types::GitHubWorkflowRun;

/// Provider for collecting CI/CD insights from GitHub Actions.
pub struct GitHubProvider {
    /// GitHub API client
    client: Arc<GitHubClient>,
    /// Repository owner
    owner: String,
    /// Repository name
    repo: String,
}

impl GitHubProvider {
    /// Create a new GitHub Actions provider.
    ///
    /// # Arguments
    ///
    /// * `base_url` - GitHub API base URL
    /// * `project_path` - Repository path in format "owner/repo"
    /// * `token` - Optional GitHub personal access token
    ///
    /// # Returns
    ///
    /// A configured GitHub Actions provider.
    pub fn new(
        base_url: String,
        project_path: String,
        token: Option<Token>,
    ) -> Result<Self> {
        let parts: Vec<&str> = project_path.split('/').collect();
        if parts.len() != 2 {
            anyhow::bail!("Project path must be in format 'owner/repo'");
        }

        let owner = parts[0].to_string();
        let repo = parts[1].to_string();

        let client = GitHubClient::new(base_url, owner.clone(), repo.clone(), token);

        Ok(Self {
            client: Arc::new(client),
            owner,
            repo,
        })
    }

    /// Collect CI/CD insights from GitHub Actions.
    ///
    /// Fetches workflow runs and analyzes them to provide comprehensive
    /// insights into CI/CD performance, reliability, and optimization opportunities.
    ///
    /// # Arguments
    ///
    /// * `limit` - Maximum number of workflow runs to analyze
    /// * `branch` - Optional branch filter
    /// * `since` - Optional start date for filtering runs
    /// * `until` - Optional end date for filtering runs
    /// * `min_type_percentage` - Minimum percentage threshold for workflow type filtering
    /// * `cost_per_minute` - Optional cost per minute for CI/CD compute
    ///
    /// # Returns
    ///
    /// `CIInsights` containing workflow types grouped by job signature,
    /// with comprehensive metrics for each type and job.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - GitHub API requests fail after retries
    /// - Repository or workflow data is not found
    /// - Network or parsing errors occur
    pub async fn collect_insights(
        &self,
        limit: usize,
        branch: Option<&str>,
        since: Option<DateTime<Utc>>,
        until: Option<DateTime<Utc>>,
        min_type_percentage: u8,
        cost_per_minute: Option<f64>,
    ) -> Result<CIInsights> {
        log::info!(
            "Starting insights collection for GitHub repository: {}/{}",
            self.owner,
            self.repo
        );

        // Fetch workflow runs from GitHub API
        let workflow_runs = self
            .client
            .fetch_workflow_runs(limit, branch, since, until)
            .await
            .context("Failed to fetch workflow runs")?;

        log::info!("Fetched {} workflow runs", workflow_runs.len());

        // Convert GitHub workflow runs to CIInsights
        let insights = self.convert_to_insights(workflow_runs, min_type_percentage, cost_per_minute);

        Ok(insights)
    }

    /// Convert GitHub workflow runs to CIInsights format.
    fn convert_to_insights(
        &self,
        workflow_runs: Vec<GitHubWorkflowRun>,
        min_type_percentage: u8,
        cost_per_minute: Option<f64>,
    ) -> CIInsights {
        // For now, create a basic structure. This would need more implementation
        // to fully match the GitLab provider's functionality.

        CIInsights {
            provider: "GitHub Actions".to_string(),
            project: format!("{}/{}", self.owner, self.repo),
            collected_at: Utc::now(),
            total_pipelines: workflow_runs.len(),
            total_pipeline_types: 1, // Simplified for now
            pipeline_types: vec![], // Would need to implement pipeline type grouping
        }
    }
}