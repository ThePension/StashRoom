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
        // File exists in HEAD, restore it from there
        let blob = repo.find_blob(entry.id())?;
        fs::write(&file_path, blob.content())
            .context("Failed to write restored file")?;
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
}
