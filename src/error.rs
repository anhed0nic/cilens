use thiserror::Error;

#[derive(Error, Debug)]
pub enum CILensError {
    #[error("API request failed: {0}")]
    Api(String),

    #[error("Invalid configuration: {0}")]
    Config(String),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, CILensError>;
