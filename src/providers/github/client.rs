use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::auth::Token;

use super::types::{GitHubJob, GitHubWorkflowRun};

/// GitHub API client for fetching workflow data.
#[derive(Clone)]
pub struct GitHubClient {
    /// HTTP client
    client: reqwest::Client,
    /// Base URL for GitHub API
    base_url: String,
    /// Repository owner
    owner: String,
    /// Repository name
    repo: String,
}

impl GitHubClient {
    /// Create a new GitHub API client.
    ///
    /// # Arguments
    ///
    /// * `base_url` - GitHub API base URL (e.g., "https://api.github.com")
    /// * `owner` - Repository owner/organization
    /// * `repo` - Repository name
    /// * `token` - Optional GitHub personal access token
    ///
    /// # Returns
    ///
    /// A configured GitHub API client.
    pub fn new(base_url: String, owner: String, repo: String, token: Option<Token>) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("cilens/1.0"));

        if let Some(token) = token {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", token.as_str())).unwrap(),
            );
        }

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .expect("Failed to build HTTP client");

        Self {
            client,
            base_url,
            owner,
            repo,
        }
    }

    /// Fetch workflow runs from GitHub API.
    ///
    /// # Arguments
    ///
    /// * `limit` - Maximum number of workflow runs to fetch
    /// * `branch` - Optional branch filter
    /// * `since` - Optional start date filter
    /// * `until` - Optional end date filter
    ///
    /// # Returns
    ///
    /// Vector of workflow runs with their jobs populated.
    pub async fn fetch_workflow_runs(
        &self,
        limit: usize,
        branch: Option<&str>,
        since: Option<chrono::DateTime<chrono::Utc>>,
        until: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Vec<GitHubWorkflowRun>> {
        let mut all_runs = Vec::new();
        let mut page = 1;
        let per_page = 100.min(limit);

        loop {
            let mut url = format!(
                "{}/repos/{}/{}/actions/runs?per_page={}&page={}",
                self.base_url, self.owner, self.repo, per_page, page
            );

            if let Some(branch) = branch {
                url.push_str(&format!("&branch={}", branch));
            }

            if let Some(since) = since {
                url.push_str(&format!("&created=>={}", since.format("%Y-%m-%dT%H:%M:%SZ")));
            }

            if let Some(until) = until {
                url.push_str(&format!("&created=<={}", until.format("%Y-%m-%dT%H:%M:%SZ")));
            }

            let response: WorkflowRunsResponse = self
                .client
                .get(&url)
                .send()
                .await
                .context("Failed to fetch workflow runs")?
                .json()
                .await
                .context("Failed to parse workflow runs response")?;

            let runs = response.workflow_runs;
            let response_len = runs.len();

            // Filter out runs without jobs or that are still in progress
            let mut filtered_runs: Vec<GitHubWorkflowRun> = runs.into_iter()
                .filter(|run| run.conclusion.is_some() && run.status == "completed")
                .collect();

            // Fetch jobs for each run
            for run in &mut filtered_runs {
                if let Ok(jobs) = self.fetch_jobs_for_run(run.id).await {
                    run.jobs = jobs;
                    run.jobs_count = run.jobs.len();
                }
            }

            all_runs.extend(filtered_runs);

            if response_len < per_page || all_runs.len() >= limit {
                break;
            }

            page += 1;
        }

        // Limit the results
        all_runs.truncate(limit);

        Ok(all_runs)
    }

    /// Fetch jobs for a specific workflow run.
    async fn fetch_jobs_for_run(&self, run_id: u64) -> Result<Vec<GitHubJob>> {
        let url = format!(
            "{}/repos/{}/{}/actions/runs/{}/jobs",
            self.base_url, self.owner, self.repo, run_id
        );

        let response: WorkflowJobsResponse = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch workflow jobs")?
            .json()
            .await
            .context("Failed to parse workflow jobs response")?;

        Ok(response.jobs)
    }
}

/// Response from GitHub API for workflow runs.
#[derive(Deserialize)]
struct WorkflowRunsResponse {
    workflow_runs: Vec<GitHubWorkflowRun>,
}

/// Response from GitHub API for workflow jobs.
#[derive(Deserialize)]
struct WorkflowJobsResponse {
    jobs: Vec<GitHubJob>,
}