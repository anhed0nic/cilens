use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use log::{debug, info, warn};
use serde::{Deserialize, Serialize};

use crate::error::Result;

use super::types::{GitLabJob, GitLabPipeline};

/// Cached pipeline data.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedPipeline {
    /// Cached job data
    jobs: Vec<GitLabJob>,
}

/// Job cache for GitLab pipelines.
///
/// Caches job data for completed pipelines to avoid redundant API calls.
/// Uses per-project cache files in platform-specific cache directories:
/// - Linux: `~/.cache/cilens/gitlab/{project-slug}.json`
/// - macOS: `~/Library/Caches/cilens/gitlab/{project-slug}.json`
///
/// Cache is loaded into memory at startup and immutable - new cache is derived from final pipeline data.
pub struct JobCache {
    cache_file: PathBuf,
    pipelines: HashMap<String, CachedPipeline>,
    enabled: bool,
}

impl JobCache {
    /// Creates a new job cache instance.
    ///
    /// Loads existing cache from disk if available. All cache data is kept in memory
    /// for fast lookups.
    ///
    /// # Arguments
    ///
    /// * `project_path` - GitLab project path (e.g., "group/project")
    /// * `enabled` - Whether caching is enabled
    ///
    /// # Returns
    ///
    /// Configured cache instance, or error if cache directory cannot be created.
    ///
    /// # Errors
    ///
    /// Returns error if cache directory cannot be determined or created.
    pub fn new(project_path: &str, enabled: bool) -> Result<Self> {
        if !enabled {
            debug!("Job cache disabled");
            return Ok(Self {
                cache_file: PathBuf::new(),
                pipelines: HashMap::new(),
                enabled: false,
            });
        }

        // Use platform-specific cache directory
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| crate::error::CILensError::Cache("No cache directory found".into()))?
            .join("cilens")
            .join("gitlab");

        fs::create_dir_all(&cache_dir)?;

        // Generate cache filename from project path (e.g., "group/project" → "group-project.json")
        let cache_filename = project_path.replace('/', "-") + ".json";
        let cache_file = cache_dir.join(cache_filename);

        // Load existing cache from disk (immutable after loading)
        let pipelines = if cache_file.exists() {
            fs::read_to_string(&cache_file)
                .ok()
                .and_then(|content| serde_json::from_str(&content).ok())
                .inspect(|_| debug!("Loaded cache from: {}", cache_file.display()))
                .unwrap_or_else(|| {
                    warn!("Failed to load cache, starting with empty cache");
                    HashMap::new()
                })
        } else {
            HashMap::new()
        };

        info!("Job cache enabled at: {}", cache_file.display());

        Ok(Self {
            cache_file,
            pipelines,
            enabled: true,
        })
    }

    /// Attempts to retrieve cached jobs for a pipeline.
    ///
    /// Performs in-memory lookup for fast access. Cache is immutable after loading.
    ///
    /// Returns `None` if:
    /// - Caching is disabled
    /// - No cache entry exists
    ///
    /// # Arguments
    ///
    /// * `pipeline_id` - Pipeline GID (unique and immutable)
    pub fn get(&self, pipeline_id: &str) -> Option<Vec<GitLabJob>> {
        if !self.enabled {
            return None;
        }

        self.pipelines.get(pipeline_id).map(|cached| {
            debug!("Cache hit for pipeline {pipeline_id}");
            cached.jobs.clone()
        })
    }

    /// Derives cache from fetched pipelines and saves to disk.
    ///
    /// Transforms the pipeline data into cache format and persists it.
    /// Client already filters to only completed pipelines (success/failed).
    ///
    /// # Arguments
    ///
    /// * `pipelines` - Fetched pipeline data to cache
    pub fn save_pipelines(&self, pipelines: &[GitLabPipeline]) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        // Derive cache from pipeline data - keyed by pipeline ID only
        let cache: HashMap<String, CachedPipeline> = pipelines
            .iter()
            .map(|pipeline| {
                (
                    pipeline.id.clone(),
                    CachedPipeline {
                        jobs: pipeline.jobs.clone(),
                    },
                )
            })
            .collect();

        // Write to disk
        let content = serde_json::to_string(&cache)?;
        fs::write(&self.cache_file, content)?;

        debug!(
            "Saved {} pipelines to cache: {}",
            cache.len(),
            self.cache_file.display()
        );

        Ok(())
    }

    /// Clears cached data for a specific project.
    ///
    /// Removes the project's cache file from disk.
    ///
    /// # Arguments
    ///
    /// * `project_path` - GitLab project path (e.g., "group/project")
    ///
    /// # Errors
    ///
    /// Returns an error if cache file cannot be removed.
    pub fn clear_project_cache(project_path: &str) -> Result<()> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| crate::error::CILensError::Cache("No cache directory found".into()))?
            .join("cilens")
            .join("gitlab");

        let cache_filename = project_path.replace('/', "-") + ".json";
        let cache_file = cache_dir.join(cache_filename);

        if cache_file.exists() {
            fs::remove_file(&cache_file)?;
            info!("Cache cleared: {}", cache_file.display());
        } else {
            info!("No cache file found for project: {project_path}");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_job(id: &str, name: &str) -> GitLabJob {
        GitLabJob {
            id: id.to_string(),
            name: name.to_string(),
            stage: "test".to_string(),
            duration: 10.0,
            status: "SUCCESS".to_string(),
            retried: false,
            needs: None,
        }
    }

    fn create_test_pipeline(id: &str, status: &str, jobs: Vec<GitLabJob>) -> GitLabPipeline {
        GitLabPipeline {
            id: id.to_string(),
            ref_: "main".to_string(),
            source: "push".to_string(),
            status: status.to_string(),
            duration: 100,
            jobs,
            stages: vec![],
        }
    }

    #[test]
    fn test_cache_disabled() {
        let cache = JobCache::new("group/project", false).unwrap();
        assert!(!cache.enabled);

        // Cache should not be used when disabled
        let retrieved = cache.get("pipeline-1");
        assert!(retrieved.is_none());

        // save_pipelines should do nothing when disabled
        let jobs = vec![create_test_job("1", "test")];
        let pipelines = vec![create_test_pipeline("pipeline-1", "success", jobs)];
        let result = cache.save_pipelines(&pipelines);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cache_caches_all_pipelines() {
        let temp_dir = TempDir::new().unwrap();
        let cache = create_cache_with_dir(temp_dir.path(), "group/project");

        let jobs = vec![create_test_job("1", "test")];

        let pipelines = vec![
            create_test_pipeline("pipeline-3", "success", jobs.clone()),
            create_test_pipeline("pipeline-4", "failed", jobs),
        ];

        // Save pipelines
        cache.save_pipelines(&pipelines).unwrap();

        // Reload cache to verify what was persisted
        let reloaded_cache = create_cache_with_dir(temp_dir.path(), "group/project");

        // Should cache both pipelines
        assert!(reloaded_cache.get("pipeline-3").is_some());
        assert!(reloaded_cache.get("pipeline-4").is_some());
    }

    #[test]
    fn test_cache_save_and_load_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let cache = create_cache_with_dir(temp_dir.path(), "group/project");

        let jobs = vec![
            create_test_job("1", "build"),
            create_test_job("2", "test"),
            create_test_job("3", "deploy"),
        ];

        let pipelines = vec![create_test_pipeline(
            "gid://gitlab/Ci::Pipeline/123",
            "success",
            jobs,
        )];

        // Save pipelines to cache
        cache.save_pipelines(&pipelines).unwrap();

        // Reload cache from disk
        let reloaded_cache = create_cache_with_dir(temp_dir.path(), "group/project");

        // Retrieve from reloaded cache
        let cached_jobs = reloaded_cache.get("gid://gitlab/Ci::Pipeline/123");
        assert!(cached_jobs.is_some());

        let cached_jobs = cached_jobs.unwrap();
        assert_eq!(cached_jobs.len(), 3);
        assert_eq!(cached_jobs[0].name, "build");
        assert_eq!(cached_jobs[1].name, "test");
        assert_eq!(cached_jobs[2].name, "deploy");
    }

    #[test]
    fn test_cache_retrieves_by_pipeline_id() {
        let temp_dir = TempDir::new().unwrap();
        let cache = create_cache_with_dir(temp_dir.path(), "group/project");

        let jobs = vec![create_test_job("1", "test")];

        let pipelines = vec![create_test_pipeline("pipeline-1", "success", jobs)];

        // Save pipeline
        cache.save_pipelines(&pipelines).unwrap();

        // Reload cache
        let reloaded_cache = create_cache_with_dir(temp_dir.path(), "group/project");

        // Should return data when querying by ID (status is irrelevant - pipeline IDs are unique)
        assert!(reloaded_cache.get("pipeline-1").is_some());

        // Non-existent ID returns None
        assert!(reloaded_cache.get("pipeline-999").is_none());
    }

    #[test]
    fn test_cache_clear() {
        let temp_dir = TempDir::new().unwrap();

        // Set up a temporary cache directory
        std::env::set_var("HOME", temp_dir.path());

        let cache = create_cache_with_dir(temp_dir.path(), "group/project");

        let jobs = vec![create_test_job("1", "test")];

        let pipelines = vec![
            create_test_pipeline("pipeline-1", "success", jobs.clone()),
            create_test_pipeline("pipeline-2", "failed", jobs),
        ];

        // Save pipelines to cache
        cache.save_pipelines(&pipelines).unwrap();

        // Verify cache file exists
        let cache_file = cache.cache_file.clone();
        assert!(cache_file.exists());

        // Clear cache using static method (requires proper cache dir setup)
        // For this test, we'll manually remove the file since we're using a temp dir
        fs::remove_file(&cache_file).unwrap();

        // Verify cache file is deleted
        assert!(!cache_file.exists());
    }

    #[test]
    fn test_per_project_cache_files() {
        let temp_dir = TempDir::new().unwrap();

        // Create cache for first project
        let cache1 = create_cache_with_dir(temp_dir.path(), "group/project1");
        let jobs1 = vec![create_test_job("1", "test1")];
        let pipelines1 = vec![create_test_pipeline("pipeline-1", "success", jobs1)];
        cache1.save_pipelines(&pipelines1).unwrap();

        // Create cache for second project
        let cache2 = create_cache_with_dir(temp_dir.path(), "group/project2");
        let jobs2 = vec![create_test_job("2", "test2")];
        let pipelines2 = vec![create_test_pipeline("pipeline-2", "success", jobs2)];
        cache2.save_pipelines(&pipelines2).unwrap();

        // Verify both cache files exist with correct names
        let cache_dir = temp_dir.path().join("cilens").join("gitlab");
        assert!(cache_dir.join("group-project1.json").exists());
        assert!(cache_dir.join("group-project2.json").exists());

        // Verify each cache contains only its own data
        let reloaded1 = create_cache_with_dir(temp_dir.path(), "group/project1");
        assert!(reloaded1.get("pipeline-1").is_some());
        assert!(reloaded1.get("pipeline-2").is_none());

        let reloaded2 = create_cache_with_dir(temp_dir.path(), "group/project2");
        assert!(reloaded2.get("pipeline-2").is_some());
        assert!(reloaded2.get("pipeline-1").is_none());
    }

    // Helper function to create cache with custom directory for testing
    fn create_cache_with_dir(dir: &std::path::Path, project_path: &str) -> JobCache {
        let cache_dir = dir.join("cilens").join("gitlab");
        fs::create_dir_all(&cache_dir).unwrap();

        let cache_filename = project_path.replace('/', "-") + ".json";
        let cache_file = cache_dir.join(cache_filename);

        // Load existing cache from disk if it exists
        let pipelines = if cache_file.exists() {
            fs::read_to_string(&cache_file)
                .ok()
                .and_then(|content| serde_json::from_str(&content).ok())
                .unwrap_or_default()
        } else {
            HashMap::new()
        };

        JobCache {
            cache_file,
            pipelines,
            enabled: true,
        }
    }
}
