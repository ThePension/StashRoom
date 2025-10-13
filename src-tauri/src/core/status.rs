use anyhow::{Context, Result};
use git2::{Repository, Status, StatusOptions};

use crate::types::{FileStatus, StatusEntry, StatusMatrix};

/// Computes the status of all files in the repository
pub fn get_status(repo: &Repository) -> Result<StatusMatrix> {
    let mut opts = StatusOptions::new();
    opts.include_untracked(true);
    opts.recurse_untracked_dirs(true);
    opts.exclude_submodules(true);

    let statuses = repo
        .statuses(Some(&mut opts))
        .context("Failed to get repository status")?;

    let mut entries = Vec::new();

    for entry in statuses.iter() {
        let path = entry.path().context("Invalid UTF-8 path")?.to_string();
        let status = entry.status();

        let status_entry = parse_status_entry(path, status)?;
        entries.push(status_entry);
    }

    // Sort by path for consistent output
    entries.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(StatusMatrix { entries })
}

/// Parses a libgit2 status into our StatusEntry format
fn parse_status_entry(path: String, status: Status) -> Result<StatusEntry> {
    let mut staged_status = None;
    let mut unstaged_status = None;
    let mut file_status = FileStatus::Untracked;
    let mut untracked = false;

    // Check for conflicts first
    if status.is_conflicted() {
        file_status = FileStatus::Conflicted;
    }
    // Check staged changes (index)
    else if status.intersects(Status::INDEX_NEW) {
        staged_status = Some("added".to_string());
        file_status = FileStatus::Added;
    } else if status.intersects(Status::INDEX_MODIFIED) {
        staged_status = Some("modified".to_string());
        file_status = FileStatus::Modified;
    } else if status.intersects(Status::INDEX_DELETED) {
        staged_status = Some("deleted".to_string());
        file_status = FileStatus::Deleted;
    } else if status.intersects(Status::INDEX_RENAMED) {
        staged_status = Some("modified".to_string());
        file_status = FileStatus::Renamed;
    }

    // Check working tree changes (unstaged)
    if status.intersects(Status::WT_MODIFIED) {
        unstaged_status = Some("modified".to_string());
        if file_status == FileStatus::Untracked {
            file_status = FileStatus::Modified;
        }
    } else if status.intersects(Status::WT_DELETED) {
        unstaged_status = Some("deleted".to_string());
        if file_status == FileStatus::Untracked {
            file_status = FileStatus::Deleted;
        }
    } else if status.intersects(Status::WT_NEW) {
        untracked = true;
        file_status = FileStatus::Untracked;
    } else if status.intersects(Status::WT_RENAMED) {
        unstaged_status = Some("modified".to_string());
        if file_status == FileStatus::Untracked {
            file_status = FileStatus::Renamed;
        }
    }

    Ok(StatusEntry {
        path,
        status: file_status,
        staged_status,
        unstaged_status,
        untracked,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Repository;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_repo() -> (TempDir, Repository) {
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

        (temp_dir, repo)
    }

    #[test]
    fn test_empty_status() {
        let (_temp_dir, repo) = create_test_repo();

        let status = get_status(&repo).unwrap();
        assert_eq!(status.entries.len(), 0);
    }

    #[test]
    fn test_untracked_file() {
        let (temp_dir, repo) = create_test_repo();

        // Create an untracked file
        let file_path = temp_dir.path().join("new_file.txt");
        fs::write(&file_path, "Hello, World!").unwrap();

        let status = get_status(&repo).unwrap();
        assert_eq!(status.entries.len(), 1);

        let entry = &status.entries[0];
        assert_eq!(entry.path, "new_file.txt");
        assert!(matches!(entry.status, FileStatus::Untracked));
        assert!(entry.untracked);
        assert!(entry.staged_status.is_none());
        assert!(entry.unstaged_status.is_none());
    }

    #[test]
    fn test_staged_new_file() {
        let (temp_dir, repo) = create_test_repo();

        // Create and stage a new file
        let file_path = temp_dir.path().join("new_file.txt");
        fs::write(&file_path, "Hello, World!").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new("new_file.txt")).unwrap();
        index.write().unwrap();

        let status = get_status(&repo).unwrap();
        assert_eq!(status.entries.len(), 1);

        let entry = &status.entries[0];
        assert_eq!(entry.path, "new_file.txt");
        assert!(matches!(entry.status, FileStatus::Added));
        assert!(!entry.untracked);
        assert_eq!(entry.staged_status, Some("added".to_string()));
        assert!(entry.unstaged_status.is_none());
    }

    #[test]
    fn test_modified_file() {
        let (temp_dir, repo) = create_test_repo();

        // Create and commit a file
        let file_path = temp_dir.path().join("file.txt");
        fs::write(&file_path, "Original content").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();

        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Add file.txt",
            &tree,
            &[&parent],
        )
        .unwrap();

        // Modify the file
        fs::write(&file_path, "Modified content").unwrap();

        let status = get_status(&repo).unwrap();
        assert_eq!(status.entries.len(), 1);

        let entry = &status.entries[0];
        assert_eq!(entry.path, "file.txt");
        assert!(matches!(entry.status, FileStatus::Modified));
        assert!(!entry.untracked);
        assert!(entry.staged_status.is_none());
        assert_eq!(entry.unstaged_status, Some("modified".to_string()));
    }

    #[test]
    fn test_staged_and_unstaged_changes() {
        let (temp_dir, repo) = create_test_repo();

        // Create and commit a file
        let file_path = temp_dir.path().join("file.txt");
        fs::write(&file_path, "Original content").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();

        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Add file.txt",
            &tree,
            &[&parent],
        )
        .unwrap();

        // Modify and stage the file
        fs::write(&file_path, "Staged content").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        index.write().unwrap();

        // Modify again (unstaged)
        fs::write(&file_path, "Unstaged content").unwrap();

        let status = get_status(&repo).unwrap();
        assert_eq!(status.entries.len(), 1);

        let entry = &status.entries[0];
        assert_eq!(entry.path, "file.txt");
        assert!(matches!(entry.status, FileStatus::Modified));
        assert!(!entry.untracked);
        assert_eq!(entry.staged_status, Some("modified".to_string()));
        assert_eq!(entry.unstaged_status, Some("modified".to_string()));
    }

    #[test]
    fn test_deleted_file() {
        let (temp_dir, repo) = create_test_repo();

        // Create and commit a file
        let file_path = temp_dir.path().join("file.txt");
        fs::write(&file_path, "Content").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();

        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Add file.txt",
            &tree,
            &[&parent],
        )
        .unwrap();

        // Delete the file
        fs::remove_file(&file_path).unwrap();

        let status = get_status(&repo).unwrap();
        assert_eq!(status.entries.len(), 1);

        let entry = &status.entries[0];
        assert_eq!(entry.path, "file.txt");
        assert!(matches!(entry.status, FileStatus::Deleted));
        assert!(!entry.untracked);
        assert!(entry.staged_status.is_none());
        assert_eq!(entry.unstaged_status, Some("deleted".to_string()));
    }
}
