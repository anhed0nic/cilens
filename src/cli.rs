use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use clap::{value_parser, Parser, Subcommand};
use log::info;

use crate::auth::Token;
use crate::providers::{GitLabProvider, JobCache};

#[derive(Parser)]
#[command(name = "cilens")]
#[command(author, version, about = "CI/CD Insights Tool", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(
        short,
        long,
        global = true,
        default_value_t = false,
        help = "Pretty-print JSON output"
    )]
    pretty: bool,
}

struct GitLabConfig<'a> {
    token: Option<&'a String>,
    base_url: &'a str,
    project_path: &'a str,
    limit: usize,
    ref_: Option<&'a str>,
    since: Option<DateTime<Utc>>,
    until: Option<DateTime<Utc>>,
    min_type_percentage: u8,
    no_cache: bool,
    clear_cache: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Collect CI/CD insights from GitLab
    Gitlab {
        #[arg(help = "GitLab project path (e.g., 'group/project')")]
        project_path: String,

        #[arg(
            long,
            env = "GITLAB_TOKEN",
            help = "GitLab personal access token (or set GITLAB_TOKEN env var)"
        )]
        token: Option<String>,

        #[arg(
            long,
            default_value = "https://gitlab.com",
            help = "GitLab instance base URL"
        )]
        base_url: String,

        #[arg(
            long,
            default_value_t = 500,
            help = "Maximum number of pipelines to fetch"
        )]
        limit: usize,

        #[arg(long, name = "ref", help = "Filter pipelines by git ref (branch/tag)")]
        ref_: Option<String>,

        #[arg(long, help = "Fetch pipelines since this date (YYYY-MM-DD)")]
        since: Option<NaiveDate>,

        #[arg(long, help = "Fetch pipelines until this date (YYYY-MM-DD)")]
        until: Option<NaiveDate>,

        #[arg(
            long,
            default_value_t = 1,
            help = "Minimum percentage for pipeline type filtering (0-100)",
            value_parser = value_parser!(u8).range(0..=100),
        )]
        min_type_percentage: u8,

        #[arg(long, help = "Disable job caching (fetch all data fresh)")]
        no_cache: bool,

        #[arg(long, help = "Clear the job cache before running")]
        clear_cache: bool,
    },
}

impl Cli {
    async fn execute_gitlab(&self, config: GitLabConfig<'_>) -> Result<()> {
        // Handle cache-only operations
        if config.clear_cache {
            JobCache::clear_project_cache(config.project_path)?;
            info!("Cache cleared successfully");
            return Ok(());
        }

        let token = config.token.map(|t| Token::from(t.as_str()));

        let provider = GitLabProvider::new(
            config.base_url,
            config.project_path.to_owned(),
            token,
            !config.no_cache,
        )?;

        // Normal insights collection
        info!(
            "Collecting GitLab insights for project: {}",
            config.project_path
        );
        if config.since.is_some() || config.until.is_some() {
            info!(
                "Date range: {} to {}",
                config
                    .since
                    .map_or_else(|| "beginning".to_string(), |d| d.date_naive().to_string()),
                config
                    .until
                    .map_or_else(|| "now".to_string(), |d| d.date_naive().to_string())
            );
        }

        let insights = provider
            .collect_insights(
                config.limit,
                config.ref_,
                config.since,
                config.until,
                config.min_type_percentage,
            )
            .await?;

        let json_output = if self.pretty {
            serde_json::to_string_pretty(&insights)?
        } else {
            serde_json::to_string(&insights)?
        };

        println!("{json_output}");

        Ok(())
    }

    pub async fn execute(&self) -> Result<()> {
        match &self.command {
            Commands::Gitlab {
                token,
                base_url,
                project_path,
                limit,
                ref_,
                since,
                until,
                min_type_percentage,
                no_cache,
                clear_cache,
            } => {
                // Convert NaiveDate to DateTime<Utc> (start of day UTC)
                let since_datetime =
                    since.map(|date| date.and_hms_opt(0, 0, 0).expect("Valid time").and_utc());

                // For until, use end of day (23:59:59) to be inclusive
                let until_datetime =
                    until.map(|date| date.and_hms_opt(23, 59, 59).expect("Valid time").and_utc());

                let config = GitLabConfig {
                    token: token.as_ref(),
                    base_url,
                    project_path,
                    limit: *limit,
                    ref_: ref_.as_deref(),
                    since: since_datetime,
                    until: until_datetime,
                    min_type_percentage: *min_type_percentage,
                    no_cache: *no_cache,
                    clear_cache: *clear_cache,
                };

                self.execute_gitlab(config).await
            }
        }
    }
}
