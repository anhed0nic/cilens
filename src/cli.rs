use anyhow::Result;
use clap::{Parser, Subcommand};
use log::info;
use std::path::PathBuf;

use crate::auth::Token;
use crate::providers::gitlab::provider::GitLabProvider;

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
        #[arg(short, long, env = "GITLAB_TOKEN")]
        token: Option<String>,

        #[arg(short, long, default_value = "https://gitlab.com")]
        url: String,

        #[arg(short = 'P', long)]
        project: String,

        #[arg(short, long, default_value_t = 20)]
        limit: usize,

        #[arg(short, long)]
        branch: Option<String>,
    },
}

impl Cli {
    async fn execute_gitlab(
        &self,
        token: &Option<String>,
        url: &str,
        project: &str,
        limit: usize,
        branch: Option<&str>,
    ) -> Result<()> {
        info!("Collecting GitLab insights for project: {}", project);

        let token = token.as_ref().map(|t| Token::from(t.as_str()));

        let provider = GitLabProvider::new(url.to_owned(), project.to_owned(), token)?;

        let insights = provider.collect_insights(project, limit, branch).await?;

        let json_output = if self.pretty {
            serde_json::to_string_pretty(&insights)?
        } else {
            serde_json::to_string(&insights)?
        };

        if let Some(output_path) = &self.output {
            std::fs::write(output_path, json_output)?;
            info!("Insights written to: {}", output_path.display());
        } else {
            println!("{}", json_output);
        }

        Ok(())
    }

    pub async fn execute(&self) -> Result<()> {
        match &self.command {
            Commands::Gitlab {
                token,
                url,
                project,
                limit,
                branch,
            } => {
                self.execute_gitlab(token, url, project, *limit, branch.as_deref())
                    .await
            }
        }
    }
}
