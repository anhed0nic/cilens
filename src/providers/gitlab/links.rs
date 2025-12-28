/// Converts a GitLab pipeline GID to a clickable web URL.
///
/// Extracts the numeric ID from a GraphQL Global ID (GID) and constructs
/// a direct link to the pipeline's web page in GitLab.
///
/// # Arguments
///
/// * `base_url` - GitLab instance base URL (e.g., <https://gitlab.com>)
/// * `project_path` - Project path (e.g., "group/project")
/// * `gid` - GraphQL Global ID (e.g., <gid://gitlab/Ci::Pipeline/123>)
///
/// # Returns
///
/// Clickable URL to the pipeline (e.g., <https://gitlab.com/group/project/-/pipelines/123>)
pub fn pipeline_id_to_url(base_url: &str, project_path: &str, gid: &str) -> String {
    let id = extract_numeric_id(gid);
    format!("{base_url}/{project_path}/-/pipelines/{id}")
}

/// Converts a GitLab job GID to a clickable web URL.
///
/// Extracts the numeric ID from a GraphQL Global ID (GID) and constructs
/// a direct link to the job's web page in GitLab.
///
/// # Arguments
///
/// * `base_url` - GitLab instance base URL (e.g., <https://gitlab.com>)
/// * `project_path` - Project path (e.g., "group/project")
/// * `gid` - GraphQL Global ID (e.g., <gid://gitlab/Ci::Job/456>)
///
/// # Returns
///
/// Clickable URL to the job (e.g., <https://gitlab.com/group/project/-/jobs/456>)
pub fn job_id_to_url(base_url: &str, project_path: &str, gid: &str) -> String {
    let id = extract_numeric_id(gid);
    format!("{base_url}/{project_path}/-/jobs/{id}")
}

fn extract_numeric_id(gid: &str) -> &str {
    // GitLab GIDs format: gid://gitlab/Ci::Pipeline/123 or gid://gitlab/Ci::Job/456
    // Extract the numeric ID after the last slash
    gid.rsplit('/').next().unwrap_or(gid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_numeric_id_pipeline() {
        assert_eq!(extract_numeric_id("gid://gitlab/Ci::Pipeline/123"), "123");
    }

    #[test]
    fn test_extract_numeric_id_job() {
        assert_eq!(extract_numeric_id("gid://gitlab/Ci::Job/456"), "456");
    }

    #[test]
    fn test_pipeline_id_to_url() {
        let url = pipeline_id_to_url(
            "https://gitlab.com",
            "group/project",
            "gid://gitlab/Ci::Pipeline/123456",
        );
        assert_eq!(url, "https://gitlab.com/group/project/-/pipelines/123456");
    }

    #[test]
    fn test_job_id_to_url() {
        let url = job_id_to_url(
            "https://gitlab.com",
            "group/project",
            "gid://gitlab/Ci::Job/789012",
        );
        assert_eq!(url, "https://gitlab.com/group/project/-/jobs/789012");
    }
}
