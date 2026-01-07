use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Configuration file structure for CILens.
///
/// Allows users to save common analysis settings and reuse them across runs.
/// Configuration files are loaded from the current directory or specified path.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    /// Default GitLab configuration
    #[serde(default)]
    pub gitlab: GitLabConfig,

    /// Default GitHub configuration
    #[serde(default)]
    pub github: GitHubConfig,

    /// Output format preferences
    #[serde(default)]
    pub output: OutputConfig,

    /// Analysis parameters
    #[serde(default)]
    pub analysis: AnalysisConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct GitLabConfig {
    /// GitLab personal access token
    pub token: Option<String>,

    /// GitLab instance base URL
    #[serde(default = "default_gitlab_base_url")]
    pub base_url: String,

    /// GitLab project path (e.g., 'group/project')
    pub project_path: Option<String>,

    /// Maximum number of pipelines to fetch
    #[serde(default = "default_limit")]
    pub limit: usize,

    /// Filter pipelines by git ref (branch/tag)
    pub ref_: Option<String>,

    /// Fetch pipelines since this date
    pub since: Option<String>,

    /// Fetch pipelines until this date
    pub until: Option<String>,

    /// Minimum percentage for pipeline type filtering
    #[serde(default = "default_min_type_percentage")]
    pub min_type_percentage: u8,

    /// Cost per minute for CI/CD compute (in cents)
    #[serde(default)]
    pub cost_per_minute: Option<f64>,

    /// Disable job caching
    #[serde(default)]
    pub no_cache: bool,

    /// Clear job cache before running
    #[serde(default)]
    pub clear_cache: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct GitHubConfig {
    /// GitHub personal access token
    pub token: Option<String>,

    /// GitHub API base URL
    #[serde(default = "default_github_base_url")]
    pub base_url: String,

    /// GitHub repository path (e.g., 'owner/repo')
    pub repo_path: Option<String>,

    /// Maximum number of workflow runs to fetch
    #[serde(default = "default_limit")]
    pub limit: usize,

    /// Filter workflow runs by git ref (branch/tag)
    pub ref_: Option<String>,

    /// Fetch workflow runs since this date
    pub since: Option<String>,

    /// Fetch workflow runs until this date
    pub until: Option<String>,

    /// Minimum percentage for workflow type filtering
    #[serde(default = "default_min_type_percentage")]
    pub min_type_percentage: u8,

    /// Cost per minute for CI/CD compute (in cents)
    #[serde(default)]
    pub cost_per_minute: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct OutputConfig {
    /// Default output format
    #[serde(default)]
    pub format: OutputFormat,

    /// Pretty-print JSON output
    #[serde(default)]
    pub pretty: bool,

    /// Include cost analysis in output
    #[serde(default)]
    pub include_costs: bool,

    /// Include optimization recommendations
    #[serde(default)]
    pub include_recommendations: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Summary,
    Json,
    Csv,
    Html,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct AnalysisConfig {
    /// Enable historical trend analysis
    #[serde(default)]
    pub enable_history: bool,

    /// History database path
    pub history_db: Option<String>,

    /// Enable issue tracker integration
    #[serde(default)]
    pub enable_issues: bool,

    /// GitHub repository for issue integration (format: owner/repo)
    pub github_repo: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            gitlab: GitLabConfig::default(),
            github: GitHubConfig::default(),
            output: OutputConfig::default(),
            analysis: AnalysisConfig::default(),
        }
    }
}

impl Default for GitLabConfig {
    fn default() -> Self {
        Self {
            token: None,
            base_url: default_gitlab_base_url(),
            project_path: None,
            limit: default_limit(),
            ref_: None,
            since: None,
            until: None,
            min_type_percentage: default_min_type_percentage(),
            cost_per_minute: None,
            no_cache: false,
            clear_cache: false,
        }
    }
}

impl Default for GitHubConfig {
    fn default() -> Self {
        Self {
            token: None,
            base_url: default_github_base_url(),
            repo_path: None,
            limit: default_limit(),
            ref_: None,
            since: None,
            until: None,
            min_type_percentage: default_min_type_percentage(),
            cost_per_minute: None,
        }
    }
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            format: OutputFormat::Summary,
            pretty: false,
            include_costs: false,
            include_recommendations: false,
        }
    }
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            enable_history: false,
            history_db: None,
            enable_issues: false,
            github_repo: None,
        }
    }
}

fn default_gitlab_base_url() -> String {
    "https://gitlab.com".to_string()
}

fn default_github_base_url() -> String {
    "https://api.github.com".to_string()
}

fn default_limit() -> usize {
    500
}

fn default_min_type_percentage() -> u8 {
    1
}

impl Config {
    /// Load configuration from a file.
    ///
    /// Searches for configuration files in this order:
    /// 1. Specified path
    /// 2. ./cilens.toml
    /// 3. ./cilens.json
    /// 4. ./cilens.yaml
    /// 5. ./cilens.yml
    ///
    /// Returns default configuration if no file is found.
    pub fn load(path: Option<&Path>) -> Result<Self> {
        if let Some(path) = path {
            return Self::load_from_path(path);
        }

        // Try common configuration file names
        let candidates = ["cilens.toml", "cilens.json", "cilens.yaml", "cilens.yml"];

        for candidate in &candidates {
            let path = Path::new(candidate);
            if path.exists() {
                return Self::load_from_path(path);
            }
        }

        // No config file found, return defaults
        Ok(Self::default())
    }

    /// Load configuration from a specific file path.
    fn load_from_path(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let extension = path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        match extension {
            "toml" => {
                toml::from_str(&contents)
                    .with_context(|| format!("Failed to parse TOML config: {}", path.display()))
            }
            "json" => {
                serde_json::from_str(&contents)
                    .with_context(|| format!("Failed to parse JSON config: {}", path.display()))
            }
            "yaml" | "yml" => {
                serde_yaml::from_str(&contents)
                    .with_context(|| format!("Failed to parse YAML config: {}", path.display()))
            }
            _ => {
                // Try TOML first, then JSON, then YAML
                toml::from_str(&contents)
                    .or_else(|_| serde_json::from_str(&contents))
                    .or_else(|_| serde_yaml::from_str(&contents))
                    .with_context(|| format!("Failed to parse config file: {}", path.display()))
            }
        }
    }

    /// Save configuration to a file.
    pub fn save(&self, path: &Path) -> Result<()> {
        let contents = match path.extension().and_then(|ext| ext.to_str()) {
            Some("json") => serde_json::to_string_pretty(self)?,
            Some("yaml") | Some("yml") => serde_yaml::to_string(self)?,
            _ => toml::to_string_pretty(self)?,
        };

        std::fs::write(path, contents)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.gitlab.base_url, "https://gitlab.com");
        assert_eq!(config.gitlab.limit, 500);
        assert_eq!(config.gitlab.min_type_percentage, 1);
        assert!(!config.output.include_costs);
        assert!(!config.analysis.enable_history);
    }

    #[test]
    fn test_load_toml_config() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let toml_content = r#"
[gitlab]
token = "glpat-test-token"
base-url = "https://gitlab.example.com"
limit = 100

[output]
format = "json"
include-costs = true

[analysis]
enable-history = true
history-db = "/tmp/cilens.db"
"#;
        write!(temp_file, "{}", toml_content).unwrap();

        let config = Config::load_from_path(temp_file.path()).unwrap();
        assert_eq!(config.gitlab.token, Some("glpat-test-token".to_string()));
        assert_eq!(config.gitlab.base_url, "https://gitlab.example.com");
        assert_eq!(config.gitlab.limit, 100);
        assert!(matches!(config.output.format, OutputFormat::Json));
        assert!(config.output.include_costs);
        assert!(config.analysis.enable_history);
        assert_eq!(config.analysis.history_db, Some("/tmp/cilens.db".to_string()));
    }

    #[test]
    fn test_load_json_config() {
        let mut temp_file = NamedTempFile::with_suffix(".json").unwrap();
        let json_content = r#"{
  "gitlab": {
    "token": "glpat-json-token",
    "base-url": "https://gitlab.json.com"
  },
  "output": {
    "format": "csv"
  }
}"#;
        write!(temp_file, "{}", json_content).unwrap();

        let config = Config::load_from_path(temp_file.path()).unwrap();
        assert_eq!(config.gitlab.token, Some("glpat-json-token".to_string()));
        assert_eq!(config.gitlab.base_url, "https://gitlab.json.com");
        assert!(matches!(config.output.format, OutputFormat::Csv));
    }

    #[test]
    fn test_load_nonexistent_config() {
        let config = Config::load(Some(Path::new("nonexistent.toml"))).unwrap();
        assert_eq!(config.gitlab.base_url, "https://gitlab.com");
        assert_eq!(config.gitlab.limit, 500);
    }

    #[test]
    fn test_load_config_from_multiple_candidates() {
        // Create a temporary directory with a cilens.toml file
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("cilens.toml");
        std::fs::write(&config_path, r#"
[gitlab]
token = "test-token"
base-url = "https://test.gitlab.com"
limit = 100
"#).unwrap();

        // Change to the temp directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&temp_dir).unwrap();

        let config = Config::load(None).unwrap();
        assert_eq!(config.gitlab.token, Some("test-token".to_string()));
        assert_eq!(config.gitlab.base_url, "https://test.gitlab.com");
        assert_eq!(config.gitlab.limit, 100);

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
    }

    #[tokio::test]
    async fn test_config_with_github_integration() {
        let config = Config {
            gitlab: GitLabConfig {
                token: Some("glpat-test".to_string()),
                base_url: "https://gitlab.example.com".to_string(),
                limit: 200,
                ref_: Some("main".to_string()),
                min_type_percentage: 5,
                cost_per_minute: Some(0.10),
            },
            output: OutputConfig {
                format: OutputFormat::Json,
                pretty: true,
                include_costs: true,
                include_recommendations: true,
            },
            analysis: AnalysisConfig {
                enable_history: true,
                history_db: Some("test.db".to_string()),
                enable_issues: true,
                github_repo: Some("myorg/myrepo".to_string()),
            },
        };

        // Test that config serializes correctly
        let toml = toml::to_string_pretty(&config).unwrap();
        assert!(toml.contains("glpat-test"));
        assert!(toml.contains("gitlab.example.com"));
        assert!(toml.contains("0.10"));
        assert!(toml.contains("myorg/myrepo"));
    }
}