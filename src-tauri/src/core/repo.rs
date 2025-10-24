use anyhow::{Context, Result};
use git2::Repository;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

use crate::types::{HeadInfo, RepoOpenResponse};

/// Metadata about an open repository
#[derive(Clone)]
pub struct RepoMetadata {
    pub repo_id: String,
    pub path: String,
    pub repo: Arc<Repository>,
}

/// Thread-safe repository registry that maps UUIDs to open repositories
pub struct RepoRegistry {
    repos: Arc<RwLock<HashMap<String, RepoMetadata>>>,
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

        // Check if this repository path is already open
        {
            let repos = self.repos.read().unwrap();
            if let Some(existing) = repos.values().find(|m| m.path == path) {
                // Repository already open, return existing entry with updated HEAD info
                let head_info = Self::get_head_info(&existing.repo)?;
                return Ok(RepoOpenResponse {
                    repo_id: existing.repo_id.clone(),
                    path: existing.path.clone(),
                    head: Some(head_info),
                });
            }
        }

        // Open the repository using libgit2
        let repo = Repository::open(repo_path)
            .with_context(|| format!("Failed to open repository at {}", path))?;

        // Generate a stable UUID for this repository
        let repo_id = Uuid::new_v4().to_string();

        // Get HEAD information
        let head_info = Self::get_head_info(&repo)?;

        // Store the repository in the registry
        let mut repos = self.repos.write().unwrap();
        repos.insert(
            repo_id.clone(),
            RepoMetadata {
                repo_id: repo_id.clone(),
                path: path.to_string(),
                repo: Arc::new(repo),
            },
        );

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
            .map(|metadata| metadata.repo.clone())
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

    /// Lists all currently open repositories with their metadata
    pub fn list_repos(&self) -> Vec<RepoOpenResponse> {
        let repos = self.repos.read().unwrap();
        repos
            .values()
            .map(|metadata| {
                let head = Self::get_head_info(&metadata.repo).ok();
                RepoOpenResponse {
                    repo_id: metadata.repo_id.clone(),
                    path: metadata.path.clone(),
                    head,
                }
            })
            .collect()
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

        let repo_ids: Vec<String> = repos.iter().map(|r| r.repo_id.clone()).collect();
        assert!(repo_ids.contains(&response1.repo_id));
        assert!(repo_ids.contains(&response2.repo_id));
    }

    #[test]
    fn test_list_repos_returns_metadata() {
        let (_temp_dir, repo_path) = create_test_repo();
        let registry = RepoRegistry::new();

        let open_response = registry.open_repo(&repo_path).unwrap();
        let repos = registry.list_repos();

        assert_eq!(repos.len(), 1);
        let listed_repo = &repos[0];

        // Verify all metadata is returned
        assert_eq!(listed_repo.repo_id, open_response.repo_id);
        assert_eq!(listed_repo.path, repo_path);
        assert!(listed_repo.head.is_some());

        let head = listed_repo.head.as_ref().unwrap();
        assert_eq!(head.branch, Some("master".to_string()));
        assert!(!head.commit.is_empty());
        assert_eq!(head.message, Some("Initial commit".to_string()));
    }

    #[test]
    fn test_multiple_repos_independent_access() {
        let (_temp_dir1, repo_path1) = create_test_repo();
        let (_temp_dir2, repo_path2) = create_test_repo();
        let registry = RepoRegistry::new();

        let response1 = registry.open_repo(&repo_path1).unwrap();
        let response2 = registry.open_repo(&repo_path2).unwrap();

        // Both repos should be accessible independently
        let repo1 = registry.get_repo(&response1.repo_id).unwrap();
        let repo2 = registry.get_repo(&response2.repo_id).unwrap();

        // Verify they are different repositories
        assert_ne!(
            repo1.path().to_string_lossy(),
            repo2.path().to_string_lossy()
        );
    }

    #[test]
    fn test_close_repo_does_not_affect_others() {
        let (_temp_dir1, repo_path1) = create_test_repo();
        let (_temp_dir2, repo_path2) = create_test_repo();
        let (_temp_dir3, repo_path3) = create_test_repo();
        let registry = RepoRegistry::new();

        let response1 = registry.open_repo(&repo_path1).unwrap();
        let response2 = registry.open_repo(&repo_path2).unwrap();
        let response3 = registry.open_repo(&repo_path3).unwrap();

        // Close the middle repo
        registry.close_repo(&response2.repo_id).unwrap();

        // Verify repo2 is closed
        assert!(registry.get_repo(&response2.repo_id).is_err());

        // Verify repo1 and repo3 are still accessible
        assert!(registry.get_repo(&response1.repo_id).is_ok());
        assert!(registry.get_repo(&response3.repo_id).is_ok());

        // Verify list shows only 2 repos
        let repos = registry.list_repos();
        assert_eq!(repos.len(), 2);
    }

    #[test]
    fn test_unique_repo_ids_for_same_path() {
        let (_temp_dir, repo_path) = create_test_repo();
        let registry = RepoRegistry::new();

        // Open the same repo twice (simulating close and reopen)
        let response1 = registry.open_repo(&repo_path).unwrap();
        registry.close_repo(&response1.repo_id).unwrap();
        let response2 = registry.open_repo(&repo_path).unwrap();

        // Each opening should generate a unique UUID
        assert_ne!(response1.repo_id, response2.repo_id);
    }

    #[test]
    fn test_concurrent_multiple_repos() {
        let (_temp_dir1, repo_path1) = create_test_repo();
        let (_temp_dir2, repo_path2) = create_test_repo();
        let (_temp_dir3, repo_path3) = create_test_repo();
        let registry = RepoRegistry::new();

        // Open multiple repos
        let response1 = registry.open_repo(&repo_path1).unwrap();
        let response2 = registry.open_repo(&repo_path2).unwrap();
        let response3 = registry.open_repo(&repo_path3).unwrap();

        // All should be in the list
        let repos = registry.list_repos();
        assert_eq!(repos.len(), 3);

        // All should have unique IDs
        let ids: Vec<String> = repos.iter().map(|r| r.repo_id.clone()).collect();
        assert_eq!(
            ids.len(),
            3,
            "All repo IDs should be present"
        );
        assert!(ids.contains(&response1.repo_id));
        assert!(ids.contains(&response2.repo_id));
        assert!(ids.contains(&response3.repo_id));
    }

    #[test]
    fn test_list_repos_empty_after_closing_all() {
        let (_temp_dir1, repo_path1) = create_test_repo();
        let (_temp_dir2, repo_path2) = create_test_repo();
        let registry = RepoRegistry::new();

        let response1 = registry.open_repo(&repo_path1).unwrap();
        let response2 = registry.open_repo(&repo_path2).unwrap();

        // Close all repos
        registry.close_repo(&response1.repo_id).unwrap();
        registry.close_repo(&response2.repo_id).unwrap();

        // List should be empty
        let repos = registry.list_repos();
        assert_eq!(repos.len(), 0);
    }

    #[test]
    fn test_open_same_repo_twice_returns_same_id() {
        let (_temp_dir, repo_path) = create_test_repo();
        let registry = RepoRegistry::new();

        // Open the same repo twice without closing
        let response1 = registry.open_repo(&repo_path).unwrap();
        let response2 = registry.open_repo(&repo_path).unwrap();

        // Should return the same repo_id to prevent duplicates
        assert_eq!(response1.repo_id, response2.repo_id);
        assert_eq!(response1.path, response2.path);

        // List should only contain one repo
        let repos = registry.list_repos();
        assert_eq!(repos.len(), 1);
    }
}
