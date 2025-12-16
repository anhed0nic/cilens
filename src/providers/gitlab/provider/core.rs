use crate::auth::Token;
use crate::error::Result;
use crate::providers::gitlab::client::GitLabClient;

pub struct GitLabProvider {
    pub client: GitLabClient,
    pub project_path: String,
}

impl GitLabProvider {
    pub fn new(base_url: &str, project_path: String, token: Option<Token>) -> Result<Self> {
        let client = GitLabClient::new(base_url, token)?;

        Ok(Self {
            client,
            project_path,
        })
    }
}
