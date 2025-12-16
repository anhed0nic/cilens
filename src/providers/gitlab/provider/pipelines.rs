use chrono::Utc;
use log::{info, warn};

use std::collections::HashMap;

use super::core::GitLabProvider;
use crate::error::Result;
use crate::insights::{CIInsights, CriticalPath, PipelineSummary};
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

    fn calculate_critical_path(pipeline: &GitLabPipeline) -> Option<CriticalPath> {
        if pipeline.jobs.is_empty() {
            return None;
        }

        let job_map: HashMap<&str, &GitLabJob> =
            pipeline.jobs.iter().map(|j| (j.name.as_str(), j)).collect();

        let mut earliest_finish: HashMap<&str, f64> = HashMap::new();
        let mut predecessors: HashMap<&str, Option<&str>> = HashMap::new();

        fn calculate_earliest_finish<'a>(
            job_name: &'a str,
            job_map: &HashMap<&'a str, &'a GitLabJob>,
            earliest_finish: &mut HashMap<&'a str, f64>,
            predecessors: &mut HashMap<&'a str, Option<&'a str>>,
        ) -> f64 {
            if let Some(&time) = earliest_finish.get(job_name) {
                return time;
            }

            let job = match job_map.get(job_name) {
                Some(j) => j,
                None => return 0.0,
            };

            if job.needs.is_empty() {
                let finish_time = job.duration;
                earliest_finish.insert(job_name, finish_time);
                predecessors.insert(job_name, None);
                return finish_time;
            }

            let mut max_predecessor_finish = 0.0;
            let mut critical_predecessor = None;

            for need in &job.needs {
                let predecessor_finish = calculate_earliest_finish(
                    need.as_str(),
                    job_map,
                    earliest_finish,
                    predecessors,
                );
                if predecessor_finish > max_predecessor_finish {
                    max_predecessor_finish = predecessor_finish;
                    critical_predecessor = Some(need.as_str());
                }
            }

            let finish_time = max_predecessor_finish + job.duration;
            earliest_finish.insert(job_name, finish_time);
            predecessors.insert(job_name, critical_predecessor);
            finish_time
        }

        for job_name in job_map.keys() {
            calculate_earliest_finish(job_name, &job_map, &mut earliest_finish, &mut predecessors);
        }

        let critical_job = earliest_finish
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())?;

        let mut path = Vec::new();
        let mut current = Some(*critical_job.0);

        while let Some(job_name) = current {
            path.push(job_name.to_string());
            current = predecessors.get(job_name).and_then(|&p| p);
        }

        path.reverse();

        Some(CriticalPath {
            jobs: path,
            total_duration_seconds: *critical_job.1,
        })
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
        let average_successful_pipeline_duration_seconds = pipelines
            .iter()
            .filter(|p| p.status == "success")
            .map(|p| p.duration as f64)
            .sum::<f64>()
            / successful_pipelines.max(1) as f64;

        let critical_paths: Vec<CriticalPath> = pipelines
            .iter()
            .filter(|p| p.status == "success")
            .filter_map(Self::calculate_critical_path)
            .collect();

        #[allow(clippy::cast_precision_loss)]
        let average_critical_path_duration_seconds = if !critical_paths.is_empty() {
            critical_paths
                .iter()
                .map(|cp| cp.total_duration_seconds)
                .sum::<f64>()
                / critical_paths.len() as f64
        } else {
            0.0
        };

        let example_critical_path = critical_paths.first().cloned();

        PipelineSummary {
            total_pipelines,
            successful_pipelines,
            failed_pipelines,
            pipeline_success_rate,
            average_successful_pipeline_duration_seconds,
            average_critical_path_duration_seconds,
            example_critical_path,
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
