use anyhow::Result;
use clap::{Parser, Subcommand};
use log::info;
use std::path::PathBuf;

use crate::auth::Token;
use crate::providers::GitLabProvider;

#[derive(Parser)]
#[command(name = "cilens")]
#[command(author, version, about = "CI/CD Insights Tool", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, global = true)]
    output: Option<PathBuf>,

    #[arg(short, long, global = true, default_value_t = false)]
    pretty: bool,
}

#[derive(Subcommand)]
enum Commands {
    Gitlab {
        #[arg(long, env = "GITLAB_TOKEN")]
        token: Option<String>,

        #[arg(long, default_value = "https://gitlab.com")]
        base_url: String,

        #[arg(long)]
        project_path: String,

        #[arg(long, default_value_t = 20)]
        limit: usize,

        #[arg(long, name = "ref")]
        ref_: Option<String>,

        #[arg(
            long,
            default_value_t = 1.0,
            help = "Minimum percentage for pipeline type filtering (0-100)"
        )]
        min_type_percentage: f64,
    },
}

impl Cli {
    async fn execute_gitlab(
        &self,
        token: Option<&String>,
        base_url: &str,
        project_path: &str,
        limit: usize,
        ref_: Option<&str>,
        min_type_percentage: f64,
    ) -> Result<()> {
        info!("Collecting GitLab insights for project: {project_path}");

        let token = token.map(|t| Token::from(t.as_str()));

        let provider = GitLabProvider::new(base_url, project_path.to_owned(), token)?;

        let insights = provider
            .collect_insights(limit, ref_, min_type_percentage)
            .await?;

        let json_output = if self.pretty {
            serde_json::to_string_pretty(&insights)?
        } else {
            serde_json::to_string(&insights)?
        };

        if let Some(output_path) = &self.output {
            std::fs::write(output_path, json_output)?;
            info!("Insights written to: {}", output_path.display());
        } else {
            println!("{json_output}");
        }

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
                min_type_percentage,
            } => {
                self.execute_gitlab(
                    token.as_ref(),
                    base_url,
                    project_path,
                    *limit,
                    ref_.as_deref(),
                    *min_type_percentage,
                )
                .await
            }
        }
    }
}
