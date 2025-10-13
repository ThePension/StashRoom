use anyhow::{Context, Result};
use git2::{ApplyLocation, Diff, Repository};
use std::path::Path;

use crate::core::diff::get_diff;
use crate::types::{DiffSide, StatusMatrix};

/// Stages an entire hunk by applying it to the index
///
/// This preserves line endings (CRLF) by applying patches directly to the index
/// without normalizing the content.
pub fn stage_hunk(
    repo: &Repository,
    path: &str,
    hunk_index: usize,
    unstage: bool,
) -> Result<StatusMatrix> {
    if unstage {
        unstage_hunk_impl(repo, path, hunk_index)?;
    } else {
        stage_hunk_impl(repo, path, hunk_index)?;
    }

    // Return updated status
    crate::core::status::get_status(repo)
}

/// Stages specific lines from a hunk
///
/// Note: This only supports staging added lines ('+' lines) due to libgit2 limitations.
/// Staging removed lines requires more complex patch manipulation.
pub fn stage_lines(
    repo: &Repository,
    path: &str,
    hunk_index: usize,
    line_indices: &[usize],
    unstage: bool,
) -> Result<StatusMatrix> {
    if unstage {
        anyhow::bail!("Unstaging individual lines is not yet supported");
    }

    stage_lines_impl(repo, path, hunk_index, line_indices)?;

    // Return updated status
    crate::core::status::get_status(repo)
}

/// Internal implementation for staging a hunk
fn stage_hunk_impl(repo: &Repository, path: &str, hunk_index: usize) -> Result<()> {
    // Get the working tree diff
    let file_diff = get_diff(repo, path, &DiffSide::Working)?;

    if hunk_index >= file_diff.hunks.len() {
        anyhow::bail!("Hunk index {} out of bounds", hunk_index);
    }

    let hunk = &file_diff.hunks[hunk_index];

    // Build a patch string for this hunk
    let patch = build_patch(&file_diff.path, hunk, None)?;

    // Apply the patch to the index
    apply_patch_to_index(repo, &patch)?;

    Ok(())
}

/// Internal implementation for unstaging a hunk
fn unstage_hunk_impl(repo: &Repository, path: &str, hunk_index: usize) -> Result<()> {
    // Get the staged diff
    let file_diff = get_diff(repo, path, &DiffSide::Index)?;

    if hunk_index >= file_diff.hunks.len() {
        anyhow::bail!("Hunk index {} out of bounds", hunk_index);
    }

    let hunk = &file_diff.hunks[hunk_index];

    // Build a reverse patch (swap + and -)
    let patch = build_reverse_patch(&file_diff.path, hunk)?;

    // Apply the reverse patch to the index
    apply_patch_to_index(repo, &patch)?;

    Ok(())
}

/// Internal implementation for staging specific lines
fn stage_lines_impl(
    repo: &Repository,
    path: &str,
    hunk_index: usize,
    line_indices: &[usize],
) -> Result<()> {
    // Get the working tree diff
    let file_diff = get_diff(repo, path, &DiffSide::Working)?;

    if hunk_index >= file_diff.hunks.len() {
        anyhow::bail!("Hunk index {} out of bounds", hunk_index);
    }

    let hunk = &file_diff.hunks[hunk_index];

    // Filter lines to only include the specified indices
    // Note: Only added lines are supported for staging
    for &idx in line_indices {
        if idx >= hunk.lines.len() {
            anyhow::bail!("Line index {} out of bounds", idx);
        }
        if hunk.lines[idx].origin != "+" {
            anyhow::bail!(
                "Only added lines ('+') can be staged individually. Line {} is '{}'",
                idx,
                hunk.lines[idx].origin
            );
        }
    }

    // Build a patch with only the selected lines
    let patch = build_patch(&file_diff.path, hunk, Some(line_indices))?;

    // Apply the patch to the index
    apply_patch_to_index(repo, &patch)?;

    Ok(())
}

/// Builds a unified diff patch string
fn build_patch(
    path: &str,
    hunk: &crate::types::DiffHunk,
    line_filter: Option<&[usize]>,
) -> Result<String> {
    let mut patch = String::new();

    // Add diff header
    patch.push_str(&format!("diff --git a/{} b/{}\n", path, path));
    patch.push_str(&format!("--- a/{}\n", path));
    patch.push_str(&format!("+++ b/{}\n", path));

    // Add hunk header
    if let Some(filter) = line_filter {
        // Recalculate hunk header for filtered lines
        let add_count = filter
            .iter()
            .filter(|&&idx| hunk.lines[idx].origin == "+")
            .count();
        patch.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            hunk.old_start, hunk.old_lines, hunk.new_start, add_count
        ));
    } else {
        patch.push_str(&format!("{}\n", hunk.header));
    }

    // Add lines
    if let Some(filter) = line_filter {
        for &idx in filter {
            let line = &hunk.lines[idx];
            patch.push_str(&format!("{}{}", line.origin, line.content));
        }
    } else {
        for line in &hunk.lines {
            patch.push_str(&format!("{}{}", line.origin, line.content));
        }
    }

    Ok(patch)
}

/// Builds a reverse patch (for unstaging)
fn build_reverse_patch(path: &str, hunk: &crate::types::DiffHunk) -> Result<String> {
    let mut patch = String::new();

    // Add diff header
    patch.push_str(&format!("diff --git a/{} b/{}\n", path, path));
    patch.push_str(&format!("--- a/{}\n", path));
    patch.push_str(&format!("+++ b/{}\n", path));

    // Add reversed hunk header (swap old/new)
    patch.push_str(&format!(
        "@@ -{},{} +{},{} @@\n",
        hunk.new_start, hunk.new_lines, hunk.old_start, hunk.old_lines
    ));

    // Add lines with reversed origins
    for line in &hunk.lines {
        let reversed_origin = match line.origin.as_str() {
            "+" => "-",
            "-" => "+",
            " " => " ",
            _ => &line.origin,
        };
        patch.push_str(&format!("{}{}", reversed_origin, line.content));
    }

    Ok(patch)
}

/// Applies a patch to the index using libgit2
fn apply_patch_to_index(repo: &Repository, patch: &str) -> Result<()> {
    // Create a diff from the patch string
    let diff = Diff::from_buffer(patch.as_bytes()).context("Failed to parse patch")?;

    // Apply the diff to the index
    repo.apply(&diff, ApplyLocation::Index, None)
        .context("Failed to apply patch to index")?;

    Ok(())
}

/// Stages an entire file
pub fn stage_file(repo: &Repository, path: &str) -> Result<StatusMatrix> {
    let mut index = repo.index().context("Failed to get index")?;
    index
        .add_path(Path::new(path))
        .context("Failed to add file to index")?;
    index.write().context("Failed to write index")?;

    crate::core::status::get_status(repo)
}

/// Unstages an entire file
pub fn unstage_file(repo: &Repository, path: &str) -> Result<StatusMatrix> {
    let head = repo.head().context("Failed to get HEAD")?;
    let commit = head.peel_to_commit().context("Failed to peel to commit")?;
    let tree = commit.tree().context("Failed to get tree")?;

    // Get the entry from HEAD
    if let Ok(entry) = tree.get_path(Path::new(path)) {
        let mut index = repo.index().context("Failed to get index")?;

        // Add the entry from HEAD to the index
        let index_entry = git2::IndexEntry {
            ctime: git2::IndexTime::new(0, 0),
            mtime: git2::IndexTime::new(0, 0),
            dev: 0,
            ino: 0,
            mode: entry.filemode() as u32,
            uid: 0,
            gid: 0,
            file_size: 0,
            id: entry.id(),
            flags: 0,
            flags_extended: 0,
            path: path.as_bytes().to_vec(),
        };

        index
            .add(&index_entry)
            .context("Failed to add entry to index")?;
        index.write().context("Failed to write index")?;
    } else {
        // File doesn't exist in HEAD, so remove it from index
        let mut index = repo.index().context("Failed to get index")?;
        index
            .remove_path(Path::new(path))
            .context("Failed to remove path from index")?;
        index.write().context("Failed to write index")?;
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
    fn test_stage_file() {
        let (temp_dir, repo) = create_test_repo();

        let file_path = temp_dir.path().join("new_file.txt");
        fs::write(&file_path, "Hello, World!\n").unwrap();

        let status = stage_file(&repo, "new_file.txt").unwrap();

        let entry = status
            .entries
            .iter()
            .find(|e| e.path == "new_file.txt")
            .unwrap();

        assert_eq!(entry.staged_status, Some("added".to_string()));
    }

    #[test]
    fn test_unstage_file() {
        let (temp_dir, repo) = create_test_repo();

        // Create and stage a file
        let file_path = temp_dir.path().join("new_file.txt");
        fs::write(&file_path, "Hello, World!\n").unwrap();

        stage_file(&repo, "new_file.txt").unwrap();

        // Unstage the file
        let status = unstage_file(&repo, "new_file.txt").unwrap();

        let entry = status
            .entries
            .iter()
            .find(|e| e.path == "new_file.txt")
            .unwrap();

        assert!(entry.untracked);
        assert!(entry.staged_status.is_none());
    }
}
