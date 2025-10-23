use anyhow::{Context, Result};
use git2::Repository;
use std::fs;
use std::path::Path;

use crate::types::StatusMatrix;

/// Result of a discard operation, including the backup timestamp
#[derive(Debug, Clone, serde::Serialize)]
pub struct DiscardResult {
    pub status: StatusMatrix,
    pub backup_timestamp: Option<String>,
}

/// Discards changes to a file, creating a backup before doing so
///
/// If `hunks` is None, the entire file is discarded.
/// If `hunks` is Some, only the specified hunks are discarded.
///
/// A backup is created at `.git/recover/<timestamp>/<path>`
pub fn discard(repo: &Repository, path: &str, hunks: Option<&[usize]>) -> Result<DiscardResult> {
    let workdir = repo
        .workdir()
        .context("Repository has no working directory")?;
    let _file_path = workdir.join(path);

    // Create backup before discarding and get the timestamp
    let backup_timestamp = create_backup(repo, path)?;

    if hunks.is_some() {
        anyhow::bail!("Discarding specific hunks is not yet implemented. Use discard_file for entire file.");
    } else {
        discard_file(repo, path)?;
    }

    // Return updated status with backup timestamp
    let status = crate::core::status::get_status(repo)?;
    Ok(DiscardResult {
        status,
        backup_timestamp,
    })
}

/// Discards changes to an entire file
fn discard_file(repo: &Repository, path: &str) -> Result<()> {
    let workdir = repo
        .workdir()
        .context("Repository has no working directory")?;
    let file_path = workdir.join(path);

    // Check if the file exists in HEAD
    let head = repo.head()?;
    let commit = head.peel_to_commit()?;
    let tree = commit.tree()?;

    if let Ok(entry) = tree.get_path(Path::new(path)) {
        // File exists in HEAD, use Git's checkout functionality to restore it
        // This is equivalent to 'git checkout HEAD -- <path>'
        let tree_obj = commit.as_object();
        repo.checkout_tree(
            tree_obj,
            Some(
                git2::build::CheckoutBuilder::new()
                    .path(path)
                    .force()
                    .remove_untracked(false),
            ),
        )
        .context("Failed to checkout file from HEAD")?;

        // Update the index to match HEAD for this file
        // This ensures the file is no longer staged
        let mut index = repo.index()?;
        let head_tree = commit.tree()?;
        let head_entry = head_tree.get_path(Path::new(path))?;

        // Remove existing index entry and add from HEAD
        let _ = index.remove_path(Path::new(path));
        index.add(&git2::IndexEntry {
            ctime: git2::IndexTime::new(0, 0),
            mtime: git2::IndexTime::new(0, 0),
            dev: 0,
            ino: 0,
            mode: head_entry.filemode() as u32,
            uid: 0,
            gid: 0,
            file_size: 0,
            id: head_entry.id(),
            flags: path.len().min(0xfff) as u16,
            flags_extended: 0,
            path: path.as_bytes().to_vec(),
        })?;
        index.write()?;
    } else {
        // File doesn't exist in HEAD (it's new), so delete it
        if file_path.exists() {
            fs::remove_file(&file_path).context("Failed to remove file")?;
        }
    }

    Ok(())
}

/// Checks if a file appears to be binary by sampling its content
fn is_binary_file(path: &Path) -> Result<bool> {
    // Read up to 8KB to check for binary content
    let mut file = fs::File::open(path)?;
    let mut buffer = vec![0u8; 8192];
    let bytes_read = std::io::Read::read(&mut file, &mut buffer)?;

    if bytes_read == 0 {
        return Ok(false); // Empty file is not binary
    }

    // Check for null bytes or high percentage of non-text bytes
    let null_count = buffer[..bytes_read].iter().filter(|&&b| b == 0).count();
    if null_count > 0 {
        return Ok(true);
    }

    // Check for high percentage of non-printable characters (excluding common whitespace)
    let non_printable = buffer[..bytes_read]
        .iter()
        .filter(|&&b| b < 32 && b != b'\n' && b != b'\r' && b != b'\t')
        .count();

    // If more than 30% non-printable, consider it binary
    Ok(non_printable > bytes_read * 3 / 10)
}

/// Creates a backup of the file in .git/recover and returns the timestamp
fn create_backup(repo: &Repository, path: &str) -> Result<Option<String>> {
    let workdir = repo
        .workdir()
        .context("Repository has no working directory")?;
    let git_dir = repo.path();
    let file_path = workdir.join(path);

    // Only backup if the file exists
    if !file_path.exists() {
        return Ok(None);
    }

    // Skip backup for binary files to save disk space
    if is_binary_file(&file_path)? {
        return Ok(None);
    }

    // Create timestamp-based backup directory
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let backup_dir = git_dir.join("recover").join(&timestamp);

    // Create backup path preserving directory structure
    let backup_path = backup_dir.join(path);

    // Ensure the backup directory exists
    if let Some(parent) = backup_path.parent() {
        fs::create_dir_all(parent).context("Failed to create backup directory")?;
    }

    // Copy the file to the backup location
    fs::copy(&file_path, &backup_path).context("Failed to create backup")?;

    Ok(Some(timestamp))
}

/// Lists available backups for recovery
pub fn list_backups(repo: &Repository) -> Result<Vec<String>> {
    let git_dir = repo.path();
    let recover_dir = git_dir.join("recover");

    if !recover_dir.exists() {
        return Ok(Vec::new());
    }

    let mut backups = Vec::new();

    for entry in fs::read_dir(&recover_dir).context("Failed to read recover directory")? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                backups.push(name.to_string());
            }
        }
    }

    backups.sort();
    backups.reverse(); // Most recent first

    Ok(backups)
}

/// Restores a file from a backup
pub fn restore_from_backup(
    repo: &Repository,
    backup_id: &str,
    path: &str,
) -> Result<StatusMatrix> {
    let git_dir = repo.path();
    let workdir = repo
        .workdir()
        .context("Repository has no working directory")?;

    let backup_path = git_dir.join("recover").join(backup_id).join(path);

    if !backup_path.exists() {
        anyhow::bail!("Backup not found: {}/{}", backup_id, path);
    }

    let file_path = workdir.join(path);

    // Ensure the target directory exists
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).context("Failed to create target directory")?;
    }

    // Copy the backup back to the working directory
    fs::copy(&backup_path, &file_path).context("Failed to restore backup")?;

    crate::core::status::get_status(repo)
}

/// Lists all files in a specific backup
pub fn list_backup_files(repo: &Repository, backup_id: &str) -> Result<Vec<String>> {
    let git_dir = repo.path();
    let backup_dir = git_dir.join("recover").join(backup_id);

    if !backup_dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    collect_files_recursive(&backup_dir, &backup_dir, &mut files)?;

    Ok(files)
}

/// Clears all backups for a repository
pub fn clear_backups(repo: &Repository) -> Result<()> {
    let git_dir = repo.path();
    let recover_dir = git_dir.join("recover");

    if recover_dir.exists() {
        fs::remove_dir_all(&recover_dir).context("Failed to remove backup directory")?;
    }

    Ok(())
}

/// Helper function to recursively collect files
fn collect_files_recursive(
    current_dir: &Path,
    base_dir: &Path,
    files: &mut Vec<String>,
) -> Result<()> {
    for entry in fs::read_dir(current_dir).context("Failed to read backup directory")? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            collect_files_recursive(&path, base_dir, files)?;
        } else if path.is_file() {
            // Get relative path from base_dir
            if let Ok(relative) = path.strip_prefix(base_dir) {
                if let Some(path_str) = relative.to_str() {
                    // Convert Windows backslashes to forward slashes
                    let normalized = path_str.replace('\\', "/");
                    files.push(normalized);
                }
            }
        }
    }

    Ok(())
}

/// Check if a file is dirty (has unstaged or staged changes)
fn is_file_dirty(repo: &Repository, path: &str) -> Result<bool> {
    let mut opts = git2::StatusOptions::new();
    opts.pathspec(path);

    let statuses = repo.statuses(Some(&mut opts))
        .context("Failed to get file status")?;

    if statuses.is_empty() {
        // File is not tracked or doesn't exist
        return Ok(false);
    }

    let status = statuses.get(0).unwrap().status();

    // Check if file has any changes (staged or unstaged)
    Ok(status.intersects(
        git2::Status::INDEX_MODIFIED |
        git2::Status::INDEX_NEW |
        git2::Status::INDEX_DELETED |
        git2::Status::WT_MODIFIED |
        git2::Status::WT_DELETED |
        git2::Status::WT_NEW
    ))
}

/// Restores multiple files from a backup
pub fn restore_many(
    repo: &Repository,
    backup_id: &str,
    paths: &[String],
) -> Result<StatusMatrix> {
    let git_dir = repo.path();
    let workdir = repo
        .workdir()
        .context("Repository has no working directory")?;

    for path in paths {
        // Normalize and validate the path to prevent directory traversal
        let normalized_path = Path::new(path);

        // Check for path traversal attempts
        for component in normalized_path.components() {
            if matches!(component, std::path::Component::ParentDir) {
                anyhow::bail!("Path traversal not allowed: {}", path);
            }
        }

        let backup_path = git_dir.join("recover").join(backup_id).join(path);

        if !backup_path.exists() {
            anyhow::bail!("Backup file not found: {}/{}", backup_id, path);
        }

        // Check if file is dirty (has unstaged or staged changes)
        let is_dirty = is_file_dirty(repo, path)?;

        let file_path = if is_dirty {
            // Non-destructive restore: write to .restore file
            let restore_path = format!("{}.restore", path);
            workdir.join(&restore_path)
        } else {
            // Clean file: restore in-place
            workdir.join(path)
        };

        // Validate that the resolved path is within the working directory
        let canonical_workdir = workdir.canonicalize()
            .context("Failed to canonicalize working directory")?;

        // For the file_path, we need to check its parent if it doesn't exist yet
        let path_to_check = if file_path.exists() {
            file_path.canonicalize()
                .context("Failed to canonicalize target path")?
        } else {
            // If file doesn't exist, check the parent directory will be within workdir
            if let Some(parent) = file_path.parent() {
                if parent.exists() {
                    let canonical_parent = parent.canonicalize()
                        .context("Failed to canonicalize parent directory")?;
                    if !canonical_parent.starts_with(&canonical_workdir) {
                        anyhow::bail!("Target path is outside working directory: {}", path);
                    }
                }
            }
            // For non-existent paths, verify the logical path is safe
            file_path.clone()
        };

        // Final check: if we were able to canonicalize, verify it's within workdir
        if path_to_check != file_path {
            if let Ok(canonical) = path_to_check.canonicalize() {
                if !canonical.starts_with(&canonical_workdir) {
                    anyhow::bail!("Target path is outside working directory: {}", path);
                }
            }
        }

        // Ensure the target directory exists
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).context("Failed to create target directory")?;
        }

        // Copy the backup back to the working directory
        fs::copy(&backup_path, &file_path).context("Failed to restore backup")?;
    }

    crate::core::status::get_status(repo)
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

        let repo = Repository::init(repo_path).unwrap();

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

        (temp_dir, repo)
    }

    #[test]
    fn test_discard_modified_file() {
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

        // Discard changes
        let _result = discard(&repo, "file.txt", None).unwrap();

        // Verify file is restored
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Original content");
    }

    #[test]
    fn test_discard_new_file() {
        let (temp_dir, repo) = create_test_repo();

        // Create a new file
        let file_path = temp_dir.path().join("new_file.txt");
        fs::write(&file_path, "New content").unwrap();

        // Discard changes (should delete the file)
        let _result = discard(&repo, "new_file.txt", None).unwrap();

        // Verify file is deleted
        assert!(!file_path.exists());
    }

    #[test]
    fn test_backup_created() {
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

        // Discard changes
        let result = discard(&repo, "file.txt", None).unwrap();
        assert!(result.backup_timestamp.is_some());

        // Verify backup exists
        let backups = list_backups(&repo).unwrap();
        assert_eq!(backups.len(), 1);

        // Verify backup content
        let git_dir = repo.path();
        let backup_path = git_dir
            .join("recover")
            .join(&backups[0])
            .join("file.txt");
        let backup_content = fs::read_to_string(&backup_path).unwrap();
        assert_eq!(backup_content, "Modified content");
    }

    #[test]
    fn test_restore_from_backup() {
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

        // Modify and discard
        fs::write(&file_path, "Modified content").unwrap();
        let _result = discard(&repo, "file.txt", None).unwrap();

        // File should be at original content
        assert_eq!(
            fs::read_to_string(&file_path).unwrap(),
            "Original content"
        );

        // Restore from backup
        let backups = list_backups(&repo).unwrap();
        restore_from_backup(&repo, &backups[0], "file.txt").unwrap();

        // File should be at modified content again
        assert_eq!(
            fs::read_to_string(&file_path).unwrap(),
            "Modified content"
        );
    }

    #[test]
    fn test_discard_removes_file_from_status() {
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

        // Verify file appears in status before discard
        let status_before = crate::core::status::get_status(&repo).unwrap();
        assert_eq!(status_before.entries.len(), 1);
        assert_eq!(status_before.entries[0].path, "file.txt");
        assert_eq!(status_before.entries[0].unstaged_status, Some("modified".to_string()));

        // Discard changes
        let result = discard(&repo, "file.txt", None).unwrap();
        let status_after = result.status;

        // Verify file no longer appears in status (no changes)
        assert_eq!(status_after.entries.len(), 0, "File should not appear in status after discard");
    }

    #[test]
    fn test_discard_staged_and_unstaged_changes() {
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

        // Verify both staged and unstaged changes exist
        let status_before = crate::core::status::get_status(&repo).unwrap();
        assert_eq!(status_before.entries.len(), 1);
        assert_eq!(status_before.entries[0].staged_status, Some("modified".to_string()));
        assert_eq!(status_before.entries[0].unstaged_status, Some("modified".to_string()));

        // Discard changes (should restore working tree to HEAD and reset index)
        let result = discard(&repo, "file.txt", None).unwrap();
        let status_after = result.status;

        // Verify file is completely clean
        assert_eq!(status_after.entries.len(), 0, "File should have no changes after discard");

        // Verify file content is back to original
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Original content");
    }

    #[test]
    fn test_discard_preserves_other_files_status() {
        let (temp_dir, repo) = create_test_repo();

        // Create and commit two files
        let file1_path = temp_dir.path().join("file1.txt");
        let file2_path = temp_dir.path().join("file2.txt");
        fs::write(&file1_path, "File 1 original").unwrap();
        fs::write(&file2_path, "File 2 original").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file1.txt")).unwrap();
        index.add_path(Path::new("file2.txt")).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();

        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Add files",
            &tree,
            &[&parent],
        )
        .unwrap();

        // Modify both files
        fs::write(&file1_path, "File 1 modified").unwrap();
        fs::write(&file2_path, "File 2 modified").unwrap();

        // Verify both files appear in status
        let status_before = crate::core::status::get_status(&repo).unwrap();
        assert_eq!(status_before.entries.len(), 2);

        // Discard only file1
        let result = discard(&repo, "file1.txt", None).unwrap();
        let status_after = result.status;

        // Verify file1 is clean but file2 still has changes
        assert_eq!(status_after.entries.len(), 1);
        assert_eq!(status_after.entries[0].path, "file2.txt");
        assert_eq!(status_after.entries[0].unstaged_status, Some("modified".to_string()));

        // Verify file1 is restored
        let content1 = fs::read_to_string(&file1_path).unwrap();
        assert_eq!(content1, "File 1 original");

        // Verify file2 is still modified
        let content2 = fs::read_to_string(&file2_path).unwrap();
        assert_eq!(content2, "File 2 modified");
    }

    #[test]
    fn test_discard_new_file_removes_from_status() {
        let (temp_dir, repo) = create_test_repo();

        // Create a new untracked file
        let file_path = temp_dir.path().join("new_file.txt");
        fs::write(&file_path, "New content").unwrap();

        // Verify file appears in status as untracked
        let status_before = crate::core::status::get_status(&repo).unwrap();
        assert_eq!(status_before.entries.len(), 1);
        assert_eq!(status_before.entries[0].path, "new_file.txt");
        assert!(status_before.entries[0].untracked);

        // Discard the new file (should delete it)
        let result = discard(&repo, "new_file.txt", None).unwrap();
        let status_after = result.status;

        // Verify file is deleted and no longer in status
        assert!(!file_path.exists());
        assert_eq!(status_after.entries.len(), 0);
    }

    #[test]
    fn test_restore_path_validation_prevents_directory_traversal() {
        let (temp_dir, repo) = create_test_repo();

        // Create and commit a file
        let file_path = temp_dir.path().join("file.txt");
        fs::write(&file_path, "original").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Initial", &tree, &[&parent]).unwrap();

        // Modify and discard to create backup
        fs::write(&file_path, "modified").unwrap();
        let result = discard(&repo, "file.txt", None).unwrap();
        let backup_id = result.backup_timestamp.unwrap();

        // Attempt to restore with path traversal attack
        let malicious_paths = vec![
            "../../../etc/passwd".to_string(),
            "..\\..\\..\\windows\\system32\\config\\sam".to_string(),
            "./../../outside.txt".to_string(),
        ];

        for malicious_path in malicious_paths {
            let result = restore_many(&repo, &backup_id, &[malicious_path.clone()]);
            // Should fail because backup file doesn't exist at that path
            assert!(result.is_err(), "Should reject path traversal: {}", malicious_path);
        }
    }

    #[test]
    fn test_restore_validates_path_within_workdir() {
        let (temp_dir, repo) = create_test_repo();

        // Create and commit a file
        let file_path = temp_dir.path().join("file.txt");
        fs::write(&file_path, "original").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Initial", &tree, &[&parent]).unwrap();

        // Modify and discard to create backup
        fs::write(&file_path, "modified").unwrap();
        let result = discard(&repo, "file.txt", None).unwrap();
        let backup_id = result.backup_timestamp.unwrap();

        // Manually create a backup file with path traversal in the backup dir
        let git_dir = repo.path();
        let malicious_backup_path = git_dir.join("recover").join(&backup_id).join("..").join("escape.txt");
        fs::create_dir_all(malicious_backup_path.parent().unwrap()).unwrap();
        fs::write(&malicious_backup_path, "malicious").unwrap();

        // Try to restore it - the path "../escape.txt" should be treated as relative to workdir
        let result = restore_many(&repo, &backup_id, &["../escape.txt".to_string()]);

        // Should fail because backup doesn't exist at that normalized path
        assert!(result.is_err());
    }

    #[test]
    fn test_restore_many_with_unstaged_changes() {
        let (temp_dir, repo) = create_test_repo();

        // Create and commit a file
        let file_path = temp_dir.path().join("file.txt");
        fs::write(&file_path, "original").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Initial", &tree, &[&parent]).unwrap();

        // Modify and discard to create backup
        fs::write(&file_path, "modified v1").unwrap();
        let result = discard(&repo, "file.txt", None).unwrap();
        let backup_id = result.backup_timestamp.unwrap();

        // Now make NEW unstaged changes to the same file
        fs::write(&file_path, "modified v2 - current work").unwrap();

        // Verify the file has unstaged changes
        let status = crate::core::status::get_status(&repo).unwrap();
        assert_eq!(status.entries.len(), 1);
        assert!(status.entries[0].unstaged_status.is_some());

        // Restore the backup - should create .restore file (non-destructive)
        let restore_result = restore_many(&repo, &backup_id, &["file.txt".to_string()]);
        assert!(restore_result.is_ok());

        // Verify original file is untouched
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "modified v2 - current work");

        // Verify backup is in .restore file
        let restore_path = temp_dir.path().join("file.txt.restore");
        assert!(restore_path.exists());
        let restore_content = fs::read_to_string(&restore_path).unwrap();
        assert_eq!(restore_content, "modified v1");
    }

    #[test]
    fn test_restore_creates_missing_directories() {
        let (temp_dir, repo) = create_test_repo();

        // Create and commit a file in a subdirectory
        let dir_path = temp_dir.path().join("subdir");
        fs::create_dir(&dir_path).unwrap();
        let file_path = dir_path.join("file.txt");
        fs::write(&file_path, "original").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new("subdir/file.txt")).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Initial", &tree, &[&parent]).unwrap();

        // Modify and discard to create backup
        fs::write(&file_path, "modified").unwrap();
        let result = discard(&repo, "subdir/file.txt", None).unwrap();
        let backup_id = result.backup_timestamp.unwrap();

        // Delete the entire subdirectory
        fs::remove_dir_all(&dir_path).unwrap();
        assert!(!dir_path.exists());

        // Verify file is tracked as deleted (dirty state)
        let status = crate::core::status::get_status(&repo).unwrap();
        assert_eq!(status.entries.len(), 1);
        assert!(status.entries[0].unstaged_status.is_some());

        // Restore should create .restore file (non-destructive because file is dirty/deleted)
        let restore_result = restore_many(&repo, &backup_id, &["subdir/file.txt".to_string()]);
        assert!(restore_result.is_ok());

        // Verify directory and .restore file were created
        assert!(dir_path.exists());
        let restore_path = dir_path.join("file.txt.restore");
        assert!(restore_path.exists());
        let content = fs::read_to_string(&restore_path).unwrap();
        assert_eq!(content, "modified");
    }

    #[test]
    fn test_restore_nonexistent_backup() {
        let (_temp_dir, repo) = create_test_repo();

        // Try to restore from a backup that doesn't exist
        let result = restore_many(&repo, "20250101_120000", &["file.txt".to_string()]);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_list_backup_files_empty_backup() {
        let (_temp_dir, repo) = create_test_repo();

        // Try to list files from non-existent backup
        let result = list_backup_files(&repo, "20250101_120000").unwrap();

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_restore_many_partial_failure() {
        let (temp_dir, repo) = create_test_repo();

        // Create and commit a file
        let file_path = temp_dir.path().join("file.txt");
        fs::write(&file_path, "original").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Initial", &tree, &[&parent]).unwrap();

        // Modify and discard to create backup
        fs::write(&file_path, "modified").unwrap();
        let result = discard(&repo, "file.txt", None).unwrap();
        let backup_id = result.backup_timestamp.unwrap();

        // Try to restore one existing file and one non-existing file
        let paths = vec!["file.txt".to_string(), "nonexistent.txt".to_string()];
        let result = restore_many(&repo, &backup_id, &paths);

        // Should fail on the first missing file
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_clear_backups_removes_all() {
        let (temp_dir, repo) = create_test_repo();

        // Create and commit a file
        let file_path = temp_dir.path().join("file.txt");
        fs::write(&file_path, "original").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Initial", &tree, &[&parent]).unwrap();

        // Create multiple backups
        fs::write(&file_path, "modified 1").unwrap();
        discard(&repo, "file.txt", None).unwrap();

        // Add a small delay to ensure different timestamps
        std::thread::sleep(std::time::Duration::from_millis(1100));

        fs::write(&file_path, "modified 2").unwrap();
        discard(&repo, "file.txt", None).unwrap();

        // Verify backups directory exists and has at least one backup
        let recover_dir = repo.path().join("recover");
        assert!(recover_dir.exists());
        let backup_count = fs::read_dir(&recover_dir).unwrap().count();
        assert!(backup_count >= 1, "Expected at least 1 backup, got {}", backup_count);

        // Clear all backups
        clear_backups(&repo).unwrap();

        // Verify backups directory is gone
        assert!(!recover_dir.exists());
    }

    #[test]
    fn test_clear_backups_when_no_backups_exist() {
        let (_temp_dir, repo) = create_test_repo();

        // Verify no backups directory exists
        let recover_dir = repo.path().join("recover");
        assert!(!recover_dir.exists());

        // Clearing should succeed even with no backups
        let result = clear_backups(&repo);
        assert!(result.is_ok());
    }

    #[test]
    fn test_multiple_discards_same_second_aggregate() {
        let (temp_dir, repo) = create_test_repo();

        // Create and commit two files
        let file1_path = temp_dir.path().join("file1.txt");
        let file2_path = temp_dir.path().join("file2.txt");
        fs::write(&file1_path, "original 1").unwrap();
        fs::write(&file2_path, "original 2").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file1.txt")).unwrap();
        index.add_path(Path::new("file2.txt")).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Add files", &tree, &[&parent]).unwrap();

        // Modify both files
        fs::write(&file1_path, "modified 1").unwrap();
        fs::write(&file2_path, "modified 2").unwrap();

        // Discard both files rapidly (same second)
        let result1 = discard(&repo, "file1.txt", None).unwrap();
        let result2 = discard(&repo, "file2.txt", None).unwrap();

        let timestamp1 = result1.backup_timestamp.unwrap();
        let timestamp2 = result2.backup_timestamp.unwrap();

        // Both should use the same backup directory (or close timestamps)
        let recover_dir = repo.path().join("recover");

        // Verify both backups were created
        let backup1_path = recover_dir.join(&timestamp1).join("file1.txt");
        let backup2_path = recover_dir.join(&timestamp2).join("file2.txt");

        // If timestamps are the same, both files should be in same backup dir
        if timestamp1 == timestamp2 {
            assert!(backup1_path.exists());
            assert!(backup2_path.exists());
        } else {
            // If different timestamps, each should be in their own directory
            let backup2_alt_path = recover_dir.join(&timestamp1).join("file2.txt");
            assert!(backup1_path.exists());
            assert!(backup2_path.exists() || backup2_alt_path.exists());
        }
    }

    #[test]
    fn test_skip_binary_file_backup() {
        let (temp_dir, repo) = create_test_repo();

        // Create and commit a binary file
        let file_path = temp_dir.path().join("image.png");
        // Write binary data (PNG magic number + null bytes to trigger binary detection)
        let binary_data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00];
        fs::write(&file_path, &binary_data).unwrap();

        // Verify file is detected as binary
        assert!(is_binary_file(&file_path).unwrap(), "PNG file should be detected as binary");

        let mut index = repo.index().unwrap();
        index.add_path(Path::new("image.png")).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Add binary", &tree, &[&parent]).unwrap();

        // Modify binary file (keep it binary with null bytes)
        let modified_data = vec![0x89, 0x50, 0x4E, 0x47, 0xFF, 0xFF, 0x00, 0x00];
        fs::write(&file_path, &modified_data).unwrap();

        // Verify modified file is still binary
        assert!(is_binary_file(&file_path).unwrap(), "Modified PNG file should be detected as binary");

        // Discard should NOT create a backup for binary files
        let result = discard(&repo, "image.png", None).unwrap();

        // Should return None for binary files
        assert!(result.backup_timestamp.is_none(), "Binary files should not be backed up");

        // Verify no backup was created
        let recover_dir = repo.path().join("recover");
        if recover_dir.exists() {
            let backup_count = fs::read_dir(&recover_dir).unwrap().count();
            assert_eq!(backup_count, 0, "No backups should exist for binary files");
        }
    }

    #[test]
    fn test_repo_moved_after_backup() {
        let temp_dir = TempDir::new().unwrap();
        let original_path = temp_dir.path().join("original");
        fs::create_dir(&original_path).unwrap();

        // Create repo in original location
        let repo = Repository::init(&original_path).unwrap();

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

        // Create and commit a file
        let file_path = original_path.join("file.txt");
        fs::write(&file_path, "original").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        {
            let parent = repo.head().unwrap().peel_to_commit().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            repo.commit(Some("HEAD"), &signature, &signature, "Add file", &tree, &[&parent]).unwrap();
        }

        // Modify and discard to create backup
        fs::write(&file_path, "modified").unwrap();
        let result = discard(&repo, "file.txt", None).unwrap();
        let backup_id = result.backup_timestamp.unwrap();

        // Close the repo (drop all references first)
        drop(index);
        drop(repo);

        // Move the repository to a new location
        let new_path = temp_dir.path().join("moved");
        fs::rename(&original_path, &new_path).unwrap();

        // Open repo in new location
        let moved_repo = Repository::open(&new_path).unwrap();

        // Restore should work with the new path
        let restore_result = restore_many(&moved_repo, &backup_id, &["file.txt".to_string()]);
        assert!(restore_result.is_ok(), "Restore should work after repo move");

        // Verify content was restored to new location
        let new_file_path = new_path.join("file.txt");
        assert!(new_file_path.exists());
        let content = fs::read_to_string(&new_file_path).unwrap();
        assert_eq!(content, "modified");
    }

    #[test]
    fn test_corrupted_backup_file_handling() {
        let (temp_dir, repo) = create_test_repo();

        // Create and commit a file
        let file_path = temp_dir.path().join("file.txt");
        fs::write(&file_path, "original").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Initial", &tree, &[&parent]).unwrap();

        // Modify and discard to create backup
        fs::write(&file_path, "modified").unwrap();
        let result = discard(&repo, "file.txt", None).unwrap();
        let backup_id = result.backup_timestamp.unwrap();

        // Corrupt the backup file by writing incomplete data
        let backup_path = repo.path().join("recover").join(&backup_id).join("file.txt");
        fs::write(&backup_path, "corrupted partial").unwrap();

        // Verify backup exists but is corrupted
        assert!(backup_path.exists());

        // Restore should succeed (it just copies the corrupted data)
        // The user will notice when they see the wrong content
        let restore_result = restore_many(&repo, &backup_id, &["file.txt".to_string()]);
        assert!(restore_result.is_ok());

        // Content will be the corrupted version
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "corrupted partial");
    }

    #[test]
    fn test_concurrent_discard_operations() {
        use std::sync::Arc;
        use std::thread;

        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        let repo = Repository::init(repo_path).unwrap();

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

        // Create and commit multiple files
        for i in 0..3 {
            let file_path = repo_path.join(format!("file{}.txt", i));
            fs::write(&file_path, format!("original {}", i)).unwrap();

            let mut index = repo.index().unwrap();
            index.add_path(Path::new(&format!("file{}.txt", i))).unwrap();
            index.write().unwrap();

            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let parent = repo.head().unwrap().peel_to_commit().unwrap();
            repo.commit(Some("HEAD"), &signature, &signature, &format!("Add file{}", i), &tree, &[&parent]).unwrap();
        }

        // Modify all files
        for i in 0..3 {
            let file_path = repo_path.join(format!("file{}.txt", i));
            fs::write(&file_path, format!("modified {}", i)).unwrap();
        }

        // Drop repo to release locks
        drop(repo);

        let repo_path_arc = Arc::new(repo_path.to_path_buf());
        let mut handles = vec![];

        // Try to discard files concurrently
        for i in 0..3 {
            let path = repo_path_arc.clone();
            let handle = thread::spawn(move || {
                let repo = Repository::open(path.as_path()).unwrap();
                discard(&repo, &format!("file{}.txt", i), None)
            });
            handles.push(handle);
        }

        // Wait for all threads and collect results
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // At least some operations should succeed (concurrent Git operations may conflict)
        let success_count = results.iter().filter(|r| r.is_ok()).count();
        assert!(success_count > 0, "At least one discard should succeed, got {}/3", success_count);

        // Verify at least one backup was created
        let repo = Repository::open(repo_path).unwrap();
        let backups = list_backups(&repo).unwrap();
        assert!(!backups.is_empty(), "At least one backup should exist");

        // Verify that succeeded operations created backups
        for (i, result) in results.iter().enumerate() {
            if let Ok(discard_result) = result {
                if let Some(_timestamp) = &discard_result.backup_timestamp {
                    // File should have been discarded
                    let file_path = repo_path.join(format!("file{}.txt", i));
                    let content = fs::read_to_string(&file_path).unwrap();
                    assert_eq!(content, format!("original {}", i), "File {} should be restored to original", i);
                }
            }
        }
    }

    #[test]
    fn test_restore_clean_file_in_place() {
        let (temp_dir, repo) = create_test_repo();

        // Create and commit a file
        let file_path = temp_dir.path().join("file.txt");
        fs::write(&file_path, "original").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Initial", &tree, &[&parent]).unwrap();

        // Modify and discard to create backup
        fs::write(&file_path, "modified").unwrap();
        let result = discard(&repo, "file.txt", None).unwrap();
        let backup_id = result.backup_timestamp.unwrap();

        // File is now clean (back to "original")
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "original");

        // Restore should happen in-place since file is clean
        let restore_result = restore_many(&repo, &backup_id, &["file.txt".to_string()]);
        assert!(restore_result.is_ok());

        // Should restore to the exact file (not .restore)
        assert!(file_path.exists());
        assert!(!temp_dir.path().join("file.txt.restore").exists());

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "modified");
    }

    #[test]
    fn test_restore_dirty_file_non_destructive() {
        let (temp_dir, repo) = create_test_repo();

        // Create and commit a file
        let file_path = temp_dir.path().join("file.txt");
        fs::write(&file_path, "original").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Initial", &tree, &[&parent]).unwrap();

        // Modify and discard to create backup
        fs::write(&file_path, "backup version").unwrap();
        let result = discard(&repo, "file.txt", None).unwrap();
        let backup_id = result.backup_timestamp.unwrap();

        // Make new unstaged changes (file is now dirty)
        fs::write(&file_path, "current work - IMPORTANT").unwrap();

        // Verify the file has unstaged changes
        let status = crate::core::status::get_status(&repo).unwrap();
        assert_eq!(status.entries.len(), 1);
        assert!(status.entries[0].unstaged_status.is_some());

        // Restore the backup - should create .restore file instead of overwriting
        let restore_result = restore_many(&repo, &backup_id, &["file.txt".to_string()]);
        assert!(restore_result.is_ok());

        // Original file should be untouched
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "current work - IMPORTANT");

        // Backup should be in .restore file
        let restore_path = temp_dir.path().join("file.txt.restore");
        assert!(restore_path.exists());
        let restore_content = fs::read_to_string(&restore_path).unwrap();
        assert_eq!(restore_content, "backup version");
    }

    #[test]
    fn test_restore_staged_file_non_destructive() {
        let (temp_dir, repo) = create_test_repo();

        // Create and commit a file
        let file_path = temp_dir.path().join("file.txt");
        fs::write(&file_path, "original").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Initial", &tree, &[&parent]).unwrap();

        // Modify and discard to create backup
        fs::write(&file_path, "backup version").unwrap();
        let result = discard(&repo, "file.txt", None).unwrap();
        let backup_id = result.backup_timestamp.unwrap();

        // Make staged changes (file is now dirty)
        fs::write(&file_path, "staged work").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        index.write().unwrap();

        // Verify the file has staged changes
        let status = crate::core::status::get_status(&repo).unwrap();
        assert_eq!(status.entries.len(), 1);
        assert!(status.entries[0].staged_status.is_some());

        // Restore the backup - should create .restore file instead of overwriting
        let restore_result = restore_many(&repo, &backup_id, &["file.txt".to_string()]);
        assert!(restore_result.is_ok());

        // Original file should be untouched
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "staged work");

        // Backup should be in .restore file
        let restore_path = temp_dir.path().join("file.txt.restore");
        assert!(restore_path.exists());
        let restore_content = fs::read_to_string(&restore_path).unwrap();
        assert_eq!(restore_content, "backup version");
    }

    #[test]
    fn test_restore_dirty_file_in_subdirectory() {
        let (temp_dir, repo) = create_test_repo();

        // Create and commit a file in a subdirectory
        let dir_path = temp_dir.path().join("subdir");
        fs::create_dir(&dir_path).unwrap();
        let file_path = dir_path.join("file.txt");
        fs::write(&file_path, "original").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new("subdir/file.txt")).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Initial", &tree, &[&parent]).unwrap();

        // Modify and discard to create backup
        fs::write(&file_path, "backup version").unwrap();
        let result = discard(&repo, "subdir/file.txt", None).unwrap();
        let backup_id = result.backup_timestamp.unwrap();

        // Make new unstaged changes (file is now dirty)
        fs::write(&file_path, "current work").unwrap();

        // Restore the backup - should create .restore file in subdirectory
        let restore_result = restore_many(&repo, &backup_id, &["subdir/file.txt".to_string()]);
        assert!(restore_result.is_ok());

        // Original file should be untouched
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "current work");

        // Backup should be in .restore file in subdirectory
        let restore_path = dir_path.join("file.txt.restore");
        assert!(restore_path.exists());
        let restore_content = fs::read_to_string(&restore_path).unwrap();
        assert_eq!(restore_content, "backup version");
    }
}
