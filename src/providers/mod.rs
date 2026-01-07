mod github;
mod gitlab;

pub use github::GitHubProvider;
pub use gitlab::{GitLabProvider, JobCache};
