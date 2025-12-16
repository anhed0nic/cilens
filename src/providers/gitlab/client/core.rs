use reqwest::Client;
use url::Url;

use crate::auth::Token;
use crate::error::{CILensError, Result};

pub struct GitLabClient {
    pub client: Client,
    pub graphql_url: Url,
    pub token: Option<Token>,
}

impl GitLabClient {
    pub fn new(base_url: &str, token: Option<Token>) -> Result<Self> {
        let client = Client::builder()
            .user_agent("CILens/0.1.0")
            .build()
            .map_err(|e| CILensError::Config(format!("Failed to create HTTP client: {e}")))?;

        let base = Url::parse(base_url)
            .map_err(|e| CILensError::Config(format!("Invalid base URL: {e}")))?;

        let graphql_url = base
            .join("api/graphql")
            .map_err(|e| CILensError::Config(format!("Invalid GraphQL URL: {e}")))?;

        Ok(Self {
            client,
            graphql_url,
            token,
        })
    }

    pub fn auth_request(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(token) = &self.token {
            request.bearer_auth(token.as_str())
        } else {
            request
        }
    }
}
