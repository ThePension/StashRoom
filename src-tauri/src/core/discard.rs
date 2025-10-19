use anyhow::{Context, Result};
use git2::Repository;
use std::fs;
use std::path::Path;

use crate::types::StatusMatrix;

/// Discards changes to a file, creating a backup before doing so
///
/// If `hunks` is None, the entire file is discarded.
/// If `hunks` is Some, only the specified hunks are discarded.
///
/// A backup is created at `.git/recover/<timestamp>/<path>`
pub fn discard(repo: &Repository, path: &str, hunks: Option<&[usize]>) -> Result<StatusMatrix> {
    let workdir = repo
        .workdir()
        .context("Repository has no working directory")?;
    let _file_path = workdir.join(path);

    // Create backup before discarding
    create_backup(repo, path)?;

    if hunks.is_some() {
        anyhow::bail!("Discarding specific hunks is not yet implemented. Use discard_file for entire file.");
    } else {
        discard_file(repo, path)?;
    }

    // Return updated status
    crate::core::status::get_status(repo)
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

/// Creates a backup of the file in .git/recover
fn create_backup(repo: &Repository, path: &str) -> Result<()> {
    let workdir = repo
        .workdir()
        .context("Repository has no working directory")?;
    let git_dir = repo.path();
    let file_path = workdir.join(path);

    // Only backup if the file exists
    if !file_path.exists() {
        return Ok(());
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

    Ok(())
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
        discard(&repo, "file.txt", None).unwrap();

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
        discard(&repo, "new_file.txt", None).unwrap();

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
        discard(&repo, "file.txt", None).unwrap();

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
        discard(&repo, "file.txt", None).unwrap();

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
        let status_after = discard(&repo, "file.txt", None).unwrap();

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
        let status_after = discard(&repo, "file.txt", None).unwrap();

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
        let status_after = discard(&repo, "file1.txt", None).unwrap();

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
        let status_after = discard(&repo, "new_file.txt", None).unwrap();

        // Verify file is deleted and no longer in status
        assert!(!file_path.exists());
        assert_eq!(status_after.entries.len(), 0);
    }
}
