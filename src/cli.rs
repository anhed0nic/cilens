use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use clap::{value_parser, Parser, Subcommand};
use log::info;
use std::path::PathBuf;

use crate::auth::Token;
use crate::config::{Config, GitLabConfig, GitHubConfig, OutputFormat};
use crate::providers::{GitHubProvider, GitLabProvider, JobCache};

/// Command-line interface for `CILens`.
///
/// Provides access to CI/CD insights from various providers (currently GitLab).
/// Supports both JSON output for programmatic use and human-readable summaries
/// for quick analysis.
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
        help = "Configuration file to load (searches common names if not specified)"
    )]
    config: Option<PathBuf>,

    #[arg(
        short,
        long,
        global = true,
        default_value_t = false,
        help = "Output JSON instead of human-readable summary"
    )]
    json: bool,

    #[arg(
        short,
        long,
        global = true,
        default_value_t = false,
        help = "Pretty-print JSON output (only works with --json)"
    )]
    pretty: bool,

    #[arg(
        long,
        global = true,
        help = "Output format: summary, json, csv, html"
    )]
    format: Option<String>,
}

/// Configuration for GitHub insights collection.
///
/// Encapsulates all parameters needed to fetch and analyze GitHub Actions workflow data.
struct GitHubConfig<'a> {
    token: Option<&'a String>,
    base_url: &'a str,
    repo_path: &'a str,
    limit: usize,
    ref_: Option<&'a str>,
    since: Option<DateTime<Utc>>,
    until: Option<DateTime<Utc>>,
    min_type_percentage: u8,
    cost_per_minute: Option<f64>,
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
    /// Collect CI/CD insights from GitHub Actions
    Github {
        #[arg(help = "GitHub repository path (e.g., 'owner/repo')")]
        repo_path: String,

        #[arg(
            long,
            env = "GITHUB_TOKEN",
            help = "GitHub personal access token (or set GITHUB_TOKEN env var)"
        )]
        token: Option<String>,

        #[arg(
            long,
            default_value = "https://api.github.com",
            help = "GitHub API base URL"
        )]
        base_url: String,

        #[arg(
            long,
            default_value_t = 500,
            help = "Maximum number of workflow runs to fetch"
        )]
        limit: usize,

        #[arg(long, name = "ref", help = "Filter workflow runs by git ref (branch/tag)")]
        ref_: Option<String>,

        #[arg(long, help = "Fetch workflow runs since this date (YYYY-MM-DD)")]
        since: Option<NaiveDate>,

        #[arg(long, help = "Fetch workflow runs until this date (YYYY-MM-DD)")]
        until: Option<NaiveDate>,

        #[arg(
            long,
            default_value_t = 1,
            help = "Minimum percentage for workflow type filtering (0-100)",
            value_parser = value_parser!(u8).range(0..=100),
        )]
        min_type_percentage: u8,
    },
}

impl Cli {
    /// Executes GitLab insights collection with the provided configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - GitLab configuration including authentication, project path, and filters
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an error if fetching/processing fails.
    ///
    /// # Behavior
    ///
    /// - If `clear_cache` is true, clears the cache and returns without fetching insights
    /// - Otherwise, fetches pipelines from GitLab and displays results in the requested format
    async fn execute_gitlab(&self, config: crate::config::GitLabConfig) -> Result<()> {
        // Handle cache-only operations
        if config.clear_cache.unwrap_or(false) {
            let project_path = config.project_path.as_ref().ok_or_else(|| anyhow::anyhow!("Project path is required"))?;
            JobCache::clear_project_cache(project_path)?;
            info!("Cache cleared successfully");
            return Ok(());
        }

        let token = config.token.as_ref().map(|t| Token::from(t.as_str()));

        let project_path = config.project_path.as_ref().ok_or_else(|| anyhow::anyhow!("Project path is required"))?;
        let provider = GitLabProvider::new(
            &config.base_url,
            project_path.to_owned(),
            token,
            !config.no_cache.unwrap_or(false),
        )?;

        // Normal insights collection
        info!(
            "Collecting GitLab insights for project: {}",
            project_path
        );
        
        // Parse string dates to DateTime<Utc>
        let since_datetime = config.since.as_ref().and_then(|s| {
            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
                .map(|date| date.and_hms_opt(0, 0, 0).unwrap().and_utc())
        });
        let until_datetime = config.until.as_ref().and_then(|s| {
            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
                .map(|date| date.and_hms_opt(23, 59, 59).unwrap().and_utc())
        });
        
        if since_datetime.is_some() || until_datetime.is_some() {
            info!(
                "Date range: {} to {}",
                since_datetime.map_or_else(|| "beginning".to_string(), |d| d.date_naive().to_string()),
                until_datetime.map_or_else(|| "now".to_string(), |d| d.date_naive().to_string())
            );
        }

        let insights = provider
            .collect_insights(
                config.limit,
                config.ref_.as_deref(),
                since_datetime,
                until_datetime,
                config.min_type_percentage,
                config.cost_per_minute,
            )
            .await?;

        // Determine output format
        let output_format = if let Some(fmt_str) = &self.format {
            match fmt_str.to_lowercase().as_str() {
                "json" => OutputFormat::Json,
                "csv" => OutputFormat::Csv,
                "html" => OutputFormat::Html,
                "summary" => OutputFormat::Summary,
                _ => {
                    eprintln!("Unknown format: {}. Using summary format.", fmt_str);
                    OutputFormat::Summary
                }
            }
        } else if self.json {
            OutputFormat::Json
        } else {
            OutputFormat::Summary
        };

        match output_format {
            OutputFormat::Summary => {
                // Summary output mode (default)
                crate::output::print_summary(&insights);
            }
            _ => {
                // Export to other formats
                let mut stdout = std::io::stdout();
                crate::output::export_insights(&insights, output_format, self.pretty, &mut stdout)?;
            }
        }

        Ok(())
    }

    /// Executes GitHub Actions insights collection with the provided configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - GitHub configuration including authentication, repository path, and filters
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an error if fetching/processing fails.
    async fn execute_github(&self, config: crate::config::GitHubConfig) -> Result<()> {
        let token = config.token.as_ref().map(|t| Token::from(t.as_str()));

        let repo_path = config.repo_path.as_ref().ok_or_else(|| anyhow::anyhow!("Repository path is required"))?;
        let provider = GitHubProvider::new(
            config.base_url.to_owned(),
            repo_path.to_owned(),
            token,
        )?;

        // Normal insights collection
        info!(
            "Collecting GitHub Actions insights for repository: {}",
            repo_path
        );
        
        // Parse string dates to DateTime<Utc>
        let since_datetime = config.since.as_ref().and_then(|s| {
            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
                .map(|date| date.and_hms_opt(0, 0, 0).unwrap().and_utc())
        });
        let until_datetime = config.until.as_ref().and_then(|s| {
            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
                .map(|date| date.and_hms_opt(23, 59, 59).unwrap().and_utc())
        });
        
        if since_datetime.is_some() || until_datetime.is_some() {
            info!(
                "Date range: {} to {}",
                since_datetime.map_or_else(|| "beginning".to_string(), |d| d.date_naive().to_string()),
                until_datetime.map_or_else(|| "now".to_string(), |d| d.date_naive().to_string())
            );
        }

        let insights = provider
            .collect_insights(
                config.limit,
                config.ref_.as_deref(),
                since_datetime,
                until_datetime,
                config.min_type_percentage,
                config.cost_per_minute,
            )
            .await?;

        // Determine output format
        let output_format = if let Some(fmt_str) = &self.format {
            match fmt_str.to_lowercase().as_str() {
                "json" => OutputFormat::Json,
                "csv" => OutputFormat::Csv,
                "html" => OutputFormat::Html,
                "summary" => OutputFormat::Summary,
                _ => {
                    eprintln!("Unknown format: {}. Using summary format.", fmt_str);
                    OutputFormat::Summary
                }
            }
        } else if self.json {
            OutputFormat::Json
        } else {
            OutputFormat::Summary
        };

        match output_format {
            OutputFormat::Summary => {
                // Summary output mode (default)
                crate::output::print_summary(&insights);
            }
            _ => {
                // Export to other formats
                let mut stdout = std::io::stdout();
                crate::output::export_insights(&insights, output_format, self.pretty, &mut stdout)?;
            }
        }

        Ok(())
    }

    /// Executes the CLI command.
    ///
    /// Parses the subcommand and routes to the appropriate handler.
    ///
    /// # Returns
    ///
    /// `Ok(())` on successful execution, or an error if the command fails.
    pub async fn execute(&self) -> Result<()> {
        // Load configuration file
        let config_file = Config::load(self.config.as_deref())?;

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

                // Merge CLI args with config file values
                let merged_token = token.as_ref()
                    .or(config_file.gitlab.token.as_ref());
                let merged_base_url = if base_url != "https://gitlab.com" { base_url } else { &config_file.gitlab.base_url };
                let merged_limit = if *limit != 500 { *limit } else { config_file.gitlab.limit };
                let merged_ref = ref_.as_deref()
                    .or(config_file.gitlab.ref_.as_deref());
                let merged_min_type_percentage = if *min_type_percentage != 1 { *min_type_percentage } else { config_file.gitlab.min_type_percentage };

                // Convert DateTime back to string format for config
                let since_str = since_datetime.map(|dt| dt.date_naive().to_string());
                let until_str = until_datetime.map(|dt| dt.date_naive().to_string());

                let config = crate::config::GitLabConfig {
                    token: merged_token.cloned(),
                    base_url: merged_base_url.to_string(),
                    project_path: Some(project_path.to_string()),
                    limit: merged_limit,
                    ref_: merged_ref.map(|s| s.to_string()),
                    since: since_str,
                    until: until_str,
                    min_type_percentage: merged_min_type_percentage,
                    no_cache: *no_cache || config_file.gitlab.no_cache,
                    clear_cache: *clear_cache || config_file.gitlab.clear_cache,
                    cost_per_minute: config_file.gitlab.cost_per_minute,
                };

                self.execute_gitlab(config).await
            }
            Commands::Github {
                token,
                base_url,
                repo_path,
                limit,
                ref_,
                since,
                until,
                min_type_percentage,
            } => {
                // Convert NaiveDate to DateTime<Utc> (start of day UTC)
                let since_datetime =
                    since.map(|date| date.and_hms_opt(0, 0, 0).expect("Valid time").and_utc());

                // For until, use end of day (23:59:59) to be inclusive
                let until_datetime =
                    until.map(|date| date.and_hms_opt(23, 59, 59).expect("Valid time").and_utc());

                // Convert DateTime back to string format for config
                let since_str = since_datetime.map(|dt| dt.date_naive().to_string());
                let until_str = until_datetime.map(|dt| dt.date_naive().to_string());

                let config = crate::config::GitHubConfig {
                    token: token.cloned(),
                    base_url: base_url.to_string(),
                    repo_path: Some(repo_path.to_string()),
                    limit: *limit,
                    ref_: ref_.map(|s| s.to_string()),
                    since: since_str,
                    until: until_str,
                    min_type_percentage: *min_type_percentage,
                    cost_per_minute: config_file.gitlab.cost_per_minute, // Reuse cost setting
                };

                self.execute_github(config).await
            }
        }
    }
}
