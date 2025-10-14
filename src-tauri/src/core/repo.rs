use anyhow::{Context, Result};
use git2::Repository;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

use crate::types::{HeadInfo, RepoOpenResponse};

/// Thread-safe repository registry that maps UUIDs to open repositories
pub struct RepoRegistry {
    repos: Arc<RwLock<HashMap<String, Arc<Repository>>>>,
}

impl RepoRegistry {
    pub fn new() -> Self {
        Self {
            repos: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Opens a repository and returns a stable UUID identifier
    pub fn open_repo(&self, path: &str) -> Result<RepoOpenResponse> {
        let repo_path = Path::new(path);

        // Open the repository using libgit2
        let repo = Repository::open(repo_path)
            .with_context(|| format!("Failed to open repository at {}", path))?;

        // Generate a stable UUID for this repository
        let repo_id = Uuid::new_v4().to_string();

        // Get HEAD information
        let head_info = Self::get_head_info(&repo)?;

        // Store the repository in the registry
        let mut repos = self.repos.write().unwrap();
        repos.insert(repo_id.clone(), Arc::new(repo));

        Ok(RepoOpenResponse {
            repo_id,
            path: path.to_string(),
            head: Some(head_info),
        })
    }

    /// Retrieves a repository by its UUID
    pub fn get_repo(&self, repo_id: &str) -> Result<Arc<Repository>> {
        let repos = self.repos.read().unwrap();
        repos
            .get(repo_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Repository not found: {}", repo_id))
    }

    /// Closes a repository and removes it from the registry
    pub fn close_repo(&self, repo_id: &str) -> Result<()> {
        let mut repos = self.repos.write().unwrap();
        repos
            .remove(repo_id)
            .ok_or_else(|| anyhow::anyhow!("Repository not found: {}", repo_id))?;
        Ok(())
    }

    /// Gets information about the current HEAD
    fn get_head_info(repo: &Repository) -> Result<HeadInfo> {
        let head = repo.head().context("Failed to get HEAD reference")?;

        let branch = if head.is_branch() {
            head.shorthand().map(|s| s.to_string())
        } else {
            None
        };

        let commit = head
            .peel_to_commit()
            .context("Failed to peel HEAD to commit")?;
        let commit_oid = commit.id().to_string();

        let message = commit.message().map(|s| {
            // Get only the first line (summary)
            s.lines().next().unwrap_or("").to_string()
        });

        Ok(HeadInfo {
            branch,
            commit: commit_oid,
            message,
        })
    }

    /// Lists all currently open repositories
    pub fn list_repos(&self) -> Vec<String> {
        let repos = self.repos.read().unwrap();
        repos.keys().cloned().collect()
    }
}

impl Default for RepoRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_repo() -> (TempDir, String) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Initialize a new repository
        let repo = Repository::init(repo_path).unwrap();

        // Create an initial commit
        let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        {
            let tree = repo.find_tree(tree_id).unwrap();
            repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                "Initial commit",
                &tree,
                &[],
            )
            .unwrap();
        }

        let repo_path_string = repo_path.to_string_lossy().to_string();
        (temp_dir, repo_path_string)
    }

    #[test]
    fn test_open_repo() {
        let (_temp_dir, repo_path) = create_test_repo();
        let registry = RepoRegistry::new();

        let response = registry.open_repo(&repo_path).unwrap();

        assert!(!response.repo_id.is_empty());
        assert_eq!(response.path, repo_path);
        assert!(response.head.is_some());

        let head = response.head.unwrap();
        assert_eq!(head.branch, Some("master".to_string()));
        assert!(!head.commit.is_empty());
        assert_eq!(head.message, Some("Initial commit".to_string()));
    }

    #[test]
    fn test_get_repo() {
        let (_temp_dir, repo_path) = create_test_repo();
        let registry = RepoRegistry::new();

        let response = registry.open_repo(&repo_path).unwrap();
        let repo = registry.get_repo(&response.repo_id).unwrap();

        assert!(repo.is_bare() == false);
    }

    #[test]
    fn test_close_repo() {
        let (_temp_dir, repo_path) = create_test_repo();
        let registry = RepoRegistry::new();

        let response = registry.open_repo(&repo_path).unwrap();
        registry.close_repo(&response.repo_id).unwrap();

        let result = registry.get_repo(&response.repo_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_list_repos() {
        let (_temp_dir1, repo_path1) = create_test_repo();
        let (_temp_dir2, repo_path2) = create_test_repo();
        let registry = RepoRegistry::new();

        let response1 = registry.open_repo(&repo_path1).unwrap();
        let response2 = registry.open_repo(&repo_path2).unwrap();

        let repos = registry.list_repos();
        assert_eq!(repos.len(), 2);
        assert!(repos.contains(&response1.repo_id));
        assert!(repos.contains(&response2.repo_id));
    }
}
