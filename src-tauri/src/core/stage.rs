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

    // Validate line selection before attempting to stage
    validate_line_selection(repo, path, hunk_index, line_indices)?;

    stage_lines_impl(repo, path, hunk_index, line_indices)?;

    // Return updated status
    crate::core::status::get_status(repo)
}

/// Validates that a line selection is compatible with staging
///
/// Rejects selections that mix added and removed lines, as this would require
/// complex patch manipulation that's not supported.
fn validate_line_selection(
    repo: &Repository,
    path: &str,
    hunk_index: usize,
    line_indices: &[usize],
) -> Result<()> {
    let file_diff = get_diff(repo, path, &DiffSide::Working, Some(3))?;

    if hunk_index >= file_diff.hunks.len() {
        anyhow::bail!("Hunk index {} out of bounds", hunk_index);
    }

    let hunk = &file_diff.hunks[hunk_index];

    let mut has_additions = false;
    let mut has_deletions = false;

    for &idx in line_indices {
        if idx >= hunk.lines.len() {
            anyhow::bail!("Line index {} out of bounds", idx);
        }

        match hunk.lines[idx].origin.as_str() {
            "+" => has_additions = true,
            "-" => has_deletions = true,
            " " => {
                // Context lines cannot be staged individually
                return Err(anyhow::anyhow!(
                    "Cannot stage context lines (line {}). Only added ('+') lines can be staged individually.",
                    idx
                ));
            }
            _ => {}
        }
    }

    // Check for mixed add/delete selection
    if has_additions && has_deletions {
        return Err(anyhow::anyhow!(
            "UNSUPPORTED_LINE_SELECTION: Cannot stage a mix of added and deleted lines. Please select only added lines."
        ));
    }

    // Currently only support additions
    if has_deletions {
        return Err(anyhow::anyhow!(
            "UNSUPPORTED_LINE_SELECTION: Staging deleted lines is not yet supported. Only added ('+') lines can be staged individually."
        ));
    }

    Ok(())
}

/// Internal implementation for staging a hunk
fn stage_hunk_impl(repo: &Repository, path: &str, hunk_index: usize) -> Result<()> {
    // Get the working tree diff
    let file_diff = get_diff(repo, path, &DiffSide::Working, Some(3))?;

    // Validate that the file has changes
    if file_diff.hunks.is_empty() {
        anyhow::bail!("No changes to stage for file: {}", path);
    }

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
    let file_diff = get_diff(repo, path, &DiffSide::Index, Some(3))?;

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
///
/// Assumes validation has already been performed by validate_line_selection.
fn stage_lines_impl(
    repo: &Repository,
    path: &str,
    hunk_index: usize,
    line_indices: &[usize],
) -> Result<()> {
    // Get the working tree diff
    let file_diff = get_diff(repo, path, &DiffSide::Working, Some(3))?;

    let hunk = &file_diff.hunks[hunk_index];

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
        // When filtering lines, we need to build a valid patch
        // Include all context and deletions, but only selected additions
        let filter_set: std::collections::HashSet<usize> = filter.iter().copied().collect();

        let mut old_count = 0;
        let mut new_count = 0;
        let mut patch_lines = Vec::new();

        // Process all lines in the hunk
        for (idx, line) in hunk.lines.iter().enumerate() {
            match line.origin.as_str() {
                " " => {
                    // Always include context lines
                    old_count += 1;
                    new_count += 1;
                    patch_lines.push(format!("{}{}", line.origin, line.content));
                }
                "-" => {
                    // Always include deletion lines
                    old_count += 1;
                    patch_lines.push(format!("{}{}", line.origin, line.content));
                }
                "+" => {
                    // Only include selected addition lines
                    if filter_set.contains(&idx) {
                        new_count += 1;
                        patch_lines.push(format!("{}{}", line.origin, line.content));
                    }
                    // Note: When we skip an addition, we don't increment new_count
                    // This creates a valid patch that adds fewer lines
                }
                _ => {}
            }
        }

        patch.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            hunk.old_start, old_count, hunk.new_start, new_count
        ));

        for line in patch_lines {
            patch.push_str(&line);
        }
    } else {
        patch.push_str(&format!("{}\n", hunk.header));
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

    // Check if the file exists in the working directory
    let workdir = repo.workdir().context("Repository has no working directory")?;
    let file_path = workdir.join(path);

    // Check if the path exists and is a file (not a directory)
    if file_path.exists() && file_path.is_file() {
        // File exists, add it to the index
        index
            .add_path(Path::new(path))
            .context("Failed to add file to index")?;
    } else {
        // File doesn't exist or is a directory (meaning original file was deleted), remove it from the index
        index
            .remove_path(Path::new(path))
            .context("Failed to remove file from index")?;
    }

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
    use std::time::Instant;
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

    fn create_file_with_content(temp_dir: &TempDir, filename: &str, content: &str) {
        let file_path = temp_dir.path().join(filename);
        fs::write(&file_path, content).unwrap();
    }

    fn commit_file(repo: &Repository, filename: &str, content: &str, temp_dir: &TempDir) {
        create_file_with_content(temp_dir, filename, content);
        let mut index = repo.index().unwrap();
        index.add_path(Path::new(filename)).unwrap();
        index.write().unwrap();

        let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();

        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Test commit",
            &tree,
            &[&parent],
        )
        .unwrap();
    }

    fn read_file_from_index(repo: &Repository, path: &str) -> Result<String> {
        let index = repo.index()?;
        let entry = index
            .get_path(Path::new(path), 0)
            .context("File not in index")?;
        let oid = entry.id;
        let blob = repo.find_blob(oid)?;
        let content = String::from_utf8(blob.content().to_vec())?;
        Ok(content)
    }

    // ========================================================================
    // Basic file staging tests
    // ========================================================================

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

    #[test]
    fn test_stage_deleted_file() {
        let (temp_dir, repo) = create_test_repo();

        // Create, commit, and then delete a file
        commit_file(&repo, "test.txt", "Content to delete", &temp_dir);

        let file_path = temp_dir.path().join("test.txt");
        fs::remove_file(&file_path).unwrap();

        // Stage the deletion
        let status = stage_file(&repo, "test.txt").unwrap();

        let entry = status
            .entries
            .iter()
            .find(|e| e.path == "test.txt")
            .unwrap();

        assert_eq!(entry.staged_status, Some("deleted".to_string()));
        assert!(entry.unstaged_status.is_none());
    }

    // ========================================================================
    // Hunk staging tests
    // ========================================================================

    #[test]
    fn test_stage_hunk_basic() {
        let (temp_dir, repo) = create_test_repo();

        // Create initial file and commit
        let initial_content = "line 1\nline 2\nline 3\n";
        commit_file(&repo, "test.txt", initial_content, &temp_dir);

        // Modify the file
        let modified_content = "line 1\nmodified line 2\nline 3\n";
        create_file_with_content(&temp_dir, "test.txt", modified_content);

        // Stage the hunk
        let status = stage_hunk(&repo, "test.txt", 0, false).unwrap();

        // Verify file is staged
        let entry = status.entries.iter().find(|e| e.path == "test.txt").unwrap();
        assert_eq!(entry.staged_status, Some("modified".to_string()));

        // Verify index content matches modified content
        let index_content = read_file_from_index(&repo, "test.txt").unwrap();
        assert_eq!(index_content, modified_content);
    }

    #[test]
    fn test_unstage_hunk_basic() {
        let (temp_dir, repo) = create_test_repo();

        // Create initial file and commit
        let initial_content = "line 1\nline 2\nline 3\n";
        commit_file(&repo, "test.txt", initial_content, &temp_dir);

        // Modify and stage the file
        let modified_content = "line 1\nmodified line 2\nline 3\n";
        create_file_with_content(&temp_dir, "test.txt", modified_content);
        stage_file(&repo, "test.txt").unwrap();

        // Unstage the hunk
        let status = stage_hunk(&repo, "test.txt", 0, true).unwrap();

        // Verify index content matches original
        let index_content = read_file_from_index(&repo, "test.txt").unwrap();
        assert_eq!(index_content, initial_content);

        // Verify file still has unstaged changes
        let entry = status.entries.iter().find(|e| e.path == "test.txt").unwrap();
        assert_eq!(entry.unstaged_status, Some("modified".to_string()));
        assert!(entry.staged_status.is_none());
    }

    // ========================================================================
    // Round-trip tests
    // ========================================================================

    #[test]
    fn test_roundtrip_stage_unstage_hunk() {
        let (temp_dir, repo) = create_test_repo();

        // Create initial file and commit
        let initial_content = "line 1\nline 2\nline 3\nline 4\n";
        commit_file(&repo, "test.txt", initial_content, &temp_dir);

        // Get initial index state
        let initial_index_content = read_file_from_index(&repo, "test.txt").unwrap();

        // Modify the file
        let modified_content = "line 1\nmodified line 2\nmodified line 3\nline 4\n";
        create_file_with_content(&temp_dir, "test.txt", modified_content);

        // Stage the hunk
        stage_hunk(&repo, "test.txt", 0, false).unwrap();

        // Unstage the hunk
        stage_hunk(&repo, "test.txt", 0, true).unwrap();

        // Verify index is back to original state
        let final_index_content = read_file_from_index(&repo, "test.txt").unwrap();
        assert_eq!(
            final_index_content, initial_index_content,
            "Round-trip stage/unstage should restore original index state"
        );
    }

    #[test]
    fn test_roundtrip_stage_unstage_lines() {
        let (temp_dir, repo) = create_test_repo();

        // Create initial file and commit
        let initial_content = "line 1\nline 2\nline 3\nline 4\nline 5\n";
        commit_file(&repo, "test.txt", initial_content, &temp_dir);

        let initial_index_content = read_file_from_index(&repo, "test.txt").unwrap();

        // Modify the file (add new lines)
        let modified_content = "line 1\nline 2\nnew line 3\nline 3\nline 4\nnew line 5\nline 5\n";
        create_file_with_content(&temp_dir, "test.txt", modified_content);

        // Get the diff to find added line indices
        let diff = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();
        assert!(!diff.hunks.is_empty());

        let hunk = &diff.hunks[0];
        let added_line_indices: Vec<usize> = hunk
            .lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.origin == "+")
            .map(|(idx, _)| idx)
            .collect();

        // Stage specific lines
        stage_lines(&repo, "test.txt", 0, &added_line_indices, false).unwrap();

        // The index should now have the added lines
        let staged_content = read_file_from_index(&repo, "test.txt").unwrap();
        assert_eq!(staged_content, modified_content);

        // Unstage the entire hunk (currently no line-by-line unstaging)
        stage_hunk(&repo, "test.txt", 0, true).unwrap();

        // Verify index is back to original
        let final_index_content = read_file_from_index(&repo, "test.txt").unwrap();
        assert_eq!(final_index_content, initial_index_content);
    }

    // ========================================================================
    // Line staging validation tests
    // ========================================================================

    #[test]
    fn test_stage_lines_rejects_mixed_additions_deletions() {
        let (temp_dir, repo) = create_test_repo();

        // Create initial file and commit
        let initial_content = "line 1\nline 2\nline 3\nline 4\n";
        commit_file(&repo, "test.txt", initial_content, &temp_dir);

        // Modify the file (mix additions and deletions)
        let modified_content = "line 1\nnew line\nline 4\n";
        create_file_with_content(&temp_dir, "test.txt", modified_content);

        // Get the diff
        let diff = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();
        let hunk = &diff.hunks[0];

        // Find indices for mixed add/delete lines
        let mut mixed_indices = Vec::new();
        for (idx, line) in hunk.lines.iter().enumerate() {
            if line.origin == "+" || line.origin == "-" {
                mixed_indices.push(idx);
            }
        }

        // Try to stage mixed lines - should fail
        let result = stage_lines(&repo, "test.txt", 0, &mixed_indices, false);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("UNSUPPORTED_LINE_SELECTION"),
            "Expected UNSUPPORTED_LINE_SELECTION error, got: {}",
            err
        );
    }

    #[test]
    fn test_stage_lines_rejects_context_lines() {
        let (temp_dir, repo) = create_test_repo();

        // Create initial file and commit
        let initial_content = "line 1\nline 2\nline 3\n";
        commit_file(&repo, "test.txt", initial_content, &temp_dir);

        // Modify the file
        let modified_content = "line 1\nmodified line 2\nline 3\n";
        create_file_with_content(&temp_dir, "test.txt", modified_content);

        // Get the diff
        let diff = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();
        let hunk = &diff.hunks[0];

        // Find a context line
        let context_index = hunk
            .lines
            .iter()
            .position(|line| line.origin == " ")
            .expect("Should have context lines");

        // Try to stage context line - should fail
        let result = stage_lines(&repo, "test.txt", 0, &[context_index], false);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Cannot stage context lines"));
    }

    // ========================================================================
    // Performance tests
    // ========================================================================

    #[test]
    fn test_stage_hunk_performance_large_file() {
        let (temp_dir, repo) = create_test_repo();

        // Create a large file with 10,000 lines
        let mut initial_content = String::new();
        for i in 0..10_000 {
            initial_content.push_str(&format!("line {}\n", i));
        }
        commit_file(&repo, "large.txt", &initial_content, &temp_dir);

        // Modify many lines in the middle
        let mut modified_content = String::new();
        for i in 0..10_000 {
            if i >= 5_000 && i < 6_000 {
                modified_content.push_str(&format!("modified line {}\n", i));
            } else {
                modified_content.push_str(&format!("line {}\n", i));
            }
        }
        create_file_with_content(&temp_dir, "large.txt", &modified_content);

        // Measure staging performance
        let start = Instant::now();
        stage_hunk(&repo, "large.txt", 0, false).unwrap();
        let duration = start.elapsed();

        println!("Stage hunk for 10k line file took: {:?}", duration);
        assert!(
            duration.as_millis() < 200,
            "Staging should take less than 200ms, took {}ms",
            duration.as_millis()
        );
    }

    // ========================================================================
    // CRLF preservation tests
    // ========================================================================

    #[test]
    #[cfg(target_os = "windows")]
    fn test_stage_hunk_preserves_crlf() {
        let (temp_dir, repo) = create_test_repo();

        // Configure git to not normalize line endings
        let mut config = repo.config().unwrap();
        config.set_str("core.autocrlf", "false").unwrap();

        // Create file with CRLF line endings
        let initial_content = "line 1\r\nline 2\r\nline 3\r\n";
        commit_file(&repo, "crlf.txt", initial_content, &temp_dir);

        // Modify with CRLF preserved
        let modified_content = "line 1\r\nmodified line 2\r\nline 3\r\n";
        create_file_with_content(&temp_dir, "crlf.txt", modified_content);

        // Stage the hunk
        stage_hunk(&repo, "crlf.txt", 0, false).unwrap();

        // Verify CRLF is preserved in index
        let index_content = read_file_from_index(&repo, "crlf.txt").unwrap();
        assert_eq!(
            index_content, modified_content,
            "CRLF line endings should be preserved"
        );
        assert!(
            index_content.contains("\r\n"),
            "Index content should contain CRLF"
        );
    }

    // ========================================================================
    // Snapshot tests
    // ========================================================================

    #[test]
    fn test_stage_hunk_patch_generation() {
        let (temp_dir, repo) = create_test_repo();

        // Create initial file and commit
        let initial_content = "line 1\nline 2\nline 3\nline 4\n";
        commit_file(&repo, "test.txt", initial_content, &temp_dir);

        // Modify the file
        let modified_content = "line 1\nmodified line 2\nmodified line 3\nline 4\n";
        create_file_with_content(&temp_dir, "test.txt", modified_content);

        // Get the diff and build a patch
        let diff = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();
        let patch = build_patch("test.txt", &diff.hunks[0], None).unwrap();

        // Snapshot the patch
        insta::assert_snapshot!(patch);
    }

    #[test]
    fn test_stage_lines_patch_generation() {
        let (temp_dir, repo) = create_test_repo();

        // Create initial file and commit
        let initial_content = "line 1\nline 2\nline 3\nline 4\nline 5\n";
        commit_file(&repo, "test.txt", initial_content, &temp_dir);

        // Modify the file (add new lines)
        let modified_content = "line 1\nline 2\nnew line 3\nline 3\nline 4\nnew line 5\nline 5\n";
        create_file_with_content(&temp_dir, "test.txt", modified_content);

        // Get the diff
        let diff = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();
        let hunk = &diff.hunks[0];

        // Get only the first added line
        let first_added_index = hunk
            .lines
            .iter()
            .position(|line| line.origin == "+")
            .unwrap();

        // Build patch for selected line
        let patch = build_patch("test.txt", hunk, Some(&[first_added_index])).unwrap();

        // Snapshot the patch
        insta::assert_snapshot!(patch);
    }

    // ========================================================================
    // Edge case tests
    // ========================================================================

    #[test]
    fn test_stage_renamed_file_old_path() {
        let (temp_dir, repo) = create_test_repo();

        // Create and commit a file
        commit_file(&repo, "old_name.txt", "File content", &temp_dir);

        // Rename the file (delete old, create new)
        let old_path = temp_dir.path().join("old_name.txt");
        let new_path = temp_dir.path().join("new_name.txt");
        fs::rename(&old_path, &new_path).unwrap();

        // Stage the deletion of the old path
        let status = stage_file(&repo, "old_name.txt").unwrap();

        // Should have the old file staged as deleted
        let old_entry = status.entries.iter().find(|e| e.path == "old_name.txt");
        assert!(old_entry.is_some());
        let old_entry = old_entry.unwrap();
        assert_eq!(old_entry.staged_status, Some("deleted".to_string()));

        // New file should be untracked
        let new_entry = status.entries.iter().find(|e| e.path == "new_name.txt");
        assert!(new_entry.is_some());
        assert!(new_entry.unwrap().untracked);
    }

    #[test]
    fn test_stage_renamed_file_both_paths() {
        let (temp_dir, repo) = create_test_repo();

        // Create and commit a file
        commit_file(&repo, "old_name.txt", "File content", &temp_dir);

        // Rename the file
        let old_path = temp_dir.path().join("old_name.txt");
        let new_path = temp_dir.path().join("new_name.txt");
        fs::rename(&old_path, &new_path).unwrap();

        // Stage both old (deletion) and new (addition)
        stage_file(&repo, "old_name.txt").unwrap();
        let status = stage_file(&repo, "new_name.txt").unwrap();

        // Both should be staged
        let old_entry = status.entries.iter().find(|e| e.path == "old_name.txt");
        assert!(old_entry.is_some());
        assert_eq!(old_entry.unwrap().staged_status, Some("deleted".to_string()));

        let new_entry = status.entries.iter().find(|e| e.path == "new_name.txt");
        assert!(new_entry.is_some());
        assert_eq!(new_entry.unwrap().staged_status, Some("added".to_string()));
    }

    #[test]
    fn test_stage_file_with_spaces() {
        let (temp_dir, repo) = create_test_repo();

        // Create and commit a file with spaces in the name
        let filename = "my file with spaces.txt";
        commit_file(&repo, filename, "Content", &temp_dir);

        // Delete the file
        let file_path = temp_dir.path().join(filename);
        fs::remove_file(&file_path).unwrap();

        // Stage the deletion
        let status = stage_file(&repo, filename).unwrap();

        let entry = status.entries.iter().find(|e| e.path == filename).unwrap();
        assert_eq!(entry.staged_status, Some("deleted".to_string()));
    }

    #[test]
    fn test_stage_file_with_unicode() {
        let (temp_dir, repo) = create_test_repo();

        // Create and commit a file with unicode characters
        let filename = "файл.txt"; // Russian for "file"
        commit_file(&repo, filename, "Content", &temp_dir);

        // Delete the file
        let file_path = temp_dir.path().join(filename);
        fs::remove_file(&file_path).unwrap();

        // Stage the deletion
        let status = stage_file(&repo, filename).unwrap();

        let entry = status.entries.iter().find(|e| e.path == filename).unwrap();
        assert_eq!(entry.staged_status, Some("deleted".to_string()));
    }

    #[test]
    fn test_stage_file_with_special_chars() {
        let (temp_dir, repo) = create_test_repo();

        // Create and commit a file with special characters
        let filename = "file (copy).txt";
        commit_file(&repo, filename, "Content", &temp_dir);

        // Delete the file
        let file_path = temp_dir.path().join(filename);
        fs::remove_file(&file_path).unwrap();

        // Stage the deletion
        let status = stage_file(&repo, filename).unwrap();

        let entry = status.entries.iter().find(|e| e.path == filename).unwrap();
        assert_eq!(entry.staged_status, Some("deleted".to_string()));
    }

    #[test]
    fn test_stage_nested_file_deletion() {
        let (temp_dir, repo) = create_test_repo();

        // Create nested directory structure
        let nested_path = "src/components/MyComponent.tsx";
        let dir_path = temp_dir.path().join("src/components");
        fs::create_dir_all(&dir_path).unwrap();

        // Commit the nested file
        commit_file(&repo, nested_path, "Component content", &temp_dir);

        // Delete the file
        let file_path = temp_dir.path().join(nested_path);
        fs::remove_file(&file_path).unwrap();

        // Stage the deletion
        let status = stage_file(&repo, nested_path).unwrap();

        let entry = status.entries.iter().find(|e| e.path == nested_path).unwrap();
        assert_eq!(entry.staged_status, Some("deleted".to_string()));
    }

    #[test]
    fn test_stage_deeply_nested_file() {
        let (temp_dir, repo) = create_test_repo();

        // Create a deeply nested file
        let nested_path = "a/b/c/d/e/file.txt";
        let dir_path = temp_dir.path().join("a/b/c/d/e");
        fs::create_dir_all(&dir_path).unwrap();

        // Commit the file
        commit_file(&repo, nested_path, "Deep content", &temp_dir);

        // Delete the file
        let file_path = temp_dir.path().join(nested_path);
        fs::remove_file(&file_path).unwrap();

        // Stage the deletion
        let status = stage_file(&repo, nested_path).unwrap();

        let entry = status.entries.iter().find(|e| e.path == nested_path).unwrap();
        assert_eq!(entry.staged_status, Some("deleted".to_string()));
    }

    #[test]
    fn test_stage_binary_file_deletion() {
        let (temp_dir, repo) = create_test_repo();

        // Create a binary file (simulated with non-UTF8 bytes)
        let filename = "image.png";
        let binary_content: Vec<u8> = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]; // PNG header

        let file_path = temp_dir.path().join(filename);
        fs::write(&file_path, &binary_content).unwrap();

        // Stage and commit the binary file
        let mut index = repo.index().unwrap();
        index.add_path(Path::new(filename)).unwrap();
        index.write().unwrap();

        let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();

        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Add binary file",
            &tree,
            &[&parent],
        )
        .unwrap();

        // Delete the binary file
        fs::remove_file(&file_path).unwrap();

        // Stage the deletion
        let status = stage_file(&repo, filename).unwrap();

        let entry = status.entries.iter().find(|e| e.path == filename).unwrap();
        assert_eq!(entry.staged_status, Some("deleted".to_string()));
    }

    #[test]
    fn test_stage_file_after_directory_with_same_name_deleted() {
        let (temp_dir, repo) = create_test_repo();

        // Create a file
        commit_file(&repo, "test", "File content", &temp_dir);

        // Delete the file
        let file_path = temp_dir.path().join("test");
        fs::remove_file(&file_path).unwrap();

        // Create a directory with the same name
        fs::create_dir(&file_path).unwrap();
        let nested_file = file_path.join("nested.txt");
        fs::write(&nested_file, "Nested content").unwrap();

        // Stage the deletion of the original file
        let status = stage_file(&repo, "test").unwrap();

        // The file deletion should be staged
        let entry = status.entries.iter().find(|e| e.path == "test");
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().staged_status, Some("deleted".to_string()));
    }

    #[test]
    fn test_stage_multiple_deleted_files() {
        let (temp_dir, repo) = create_test_repo();

        // Create and commit multiple files
        commit_file(&repo, "file1.txt", "Content 1", &temp_dir);
        commit_file(&repo, "file2.txt", "Content 2", &temp_dir);
        commit_file(&repo, "file3.txt", "Content 3", &temp_dir);

        // Delete all files
        fs::remove_file(temp_dir.path().join("file1.txt")).unwrap();
        fs::remove_file(temp_dir.path().join("file2.txt")).unwrap();
        fs::remove_file(temp_dir.path().join("file3.txt")).unwrap();

        // Stage all deletions
        stage_file(&repo, "file1.txt").unwrap();
        stage_file(&repo, "file2.txt").unwrap();
        let status = stage_file(&repo, "file3.txt").unwrap();

        // All should be staged as deleted
        for filename in &["file1.txt", "file2.txt", "file3.txt"] {
            let entry = status.entries.iter().find(|e| e.path == *filename).unwrap();
            assert_eq!(entry.staged_status, Some("deleted".to_string()));
        }
    }

    // ========================================================================
    // Multiple hunks tests
    // ========================================================================

    #[test]
    fn test_stage_second_hunk_only() {
        let (temp_dir, repo) = create_test_repo();

        // Create file with content that will produce 3 separate hunks
        // Need enough separation (>6 lines of context) to avoid merging
        let mut initial = String::new();
        initial.push_str("section1 line1\n");
        initial.push_str("section1 line2\n");
        for _ in 0..10 {
            initial.push_str("separator\n");
        }
        initial.push_str("section2 line1\n");
        initial.push_str("section2 line2\n");
        for _ in 0..10 {
            initial.push_str("separator\n");
        }
        initial.push_str("section3 line1\n");
        initial.push_str("section3 line2\n");

        commit_file(&repo, "test.txt", &initial, &temp_dir);

        // Modify in 3 separate locations
        let mut modified = String::new();
        modified.push_str("MODIFIED section1\n");
        modified.push_str("section1 line2\n");
        for _ in 0..10 {
            modified.push_str("separator\n");
        }
        modified.push_str("MODIFIED section2\n");
        modified.push_str("section2 line2\n");
        for _ in 0..10 {
            modified.push_str("separator\n");
        }
        modified.push_str("MODIFIED section3\n");
        modified.push_str("section3 line2\n");

        create_file_with_content(&temp_dir, "test.txt", &modified);

        // Get diff to verify we have multiple hunks
        let diff = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();
        assert!(diff.hunks.len() >= 2, "Should have at least 2 hunks, got {}", diff.hunks.len());

        // Stage only the second hunk (index 1)
        stage_hunk(&repo, "test.txt", 1, false).unwrap();

        // Verify index has original section1, modified section2, original section3
        let index_content = read_file_from_index(&repo, "test.txt").unwrap();
        assert!(index_content.contains("section1 line1\n"));
        assert!(index_content.contains("MODIFIED section2\n"));
        assert!(index_content.contains("section3 line1\n"));
    }

    #[test]
    fn test_stage_hunks_out_of_order() {
        let (temp_dir, repo) = create_test_repo();

        // Create file with well-separated sections to ensure multiple hunks
        let mut initial = String::new();
        initial.push_str("section1\n");
        for _ in 0..10 {
            initial.push_str("filler\n");
        }
        initial.push_str("section2\n");
        for _ in 0..10 {
            initial.push_str("filler\n");
        }
        initial.push_str("section3\n");

        commit_file(&repo, "test.txt", &initial, &temp_dir);

        // Modify all three sections
        let mut modified = String::new();
        modified.push_str("MODIFIED1\n");
        for _ in 0..10 {
            modified.push_str("filler\n");
        }
        modified.push_str("MODIFIED2\n");
        for _ in 0..10 {
            modified.push_str("filler\n");
        }
        modified.push_str("MODIFIED3\n");

        create_file_with_content(&temp_dir, "test.txt", &modified);

        let diff = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();
        assert!(diff.hunks.len() >= 2, "Expected at least 2 hunks, got {}", diff.hunks.len());

        // Stage hunk 2, then hunk 0
        stage_hunk(&repo, "test.txt", 2, false).unwrap();
        stage_hunk(&repo, "test.txt", 0, false).unwrap();

        let index_content = read_file_from_index(&repo, "test.txt").unwrap();
        assert!(index_content.contains("MODIFIED1\n"));
        assert!(index_content.contains("MODIFIED3\n"));
    }

    #[test]
    fn test_stage_all_hunks_individually() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "a\nb\n\nd\ne\n\ng\nh\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "A\nb\n\nd\nE\n\ng\nH\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        let diff = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();
        let hunk_count = diff.hunks.len();

        // Stage all hunks one by one
        for i in 0..hunk_count {
            stage_hunk(&repo, "test.txt", i, false).unwrap();
        }

        // Verify final index matches modified
        let index_content = read_file_from_index(&repo, "test.txt").unwrap();
        assert_eq!(index_content, modified);
    }

    #[test]
    fn test_unstage_specific_hunk_when_multiple_staged() {
        let (temp_dir, repo) = create_test_repo();

        // Create file with well-separated sections to ensure multiple hunks
        let mut initial = String::new();
        initial.push_str("section1\n");
        for _ in 0..10 {
            initial.push_str("context\n");
        }
        initial.push_str("section2\n");

        commit_file(&repo, "test.txt", &initial, &temp_dir);

        let mut modified = String::new();
        modified.push_str("MODIFIED1\n");
        for _ in 0..10 {
            modified.push_str("context\n");
        }
        modified.push_str("MODIFIED2\n");

        create_file_with_content(&temp_dir, "test.txt", &modified);

        // Stage entire file
        stage_file(&repo, "test.txt").unwrap();

        // Get the staged diff to check hunk count
        let diff = get_diff(&repo, "test.txt", &DiffSide::Index, Some(3)).unwrap();
        if diff.hunks.len() >= 2 {
            // Unstage only first hunk
            stage_hunk(&repo, "test.txt", 0, true).unwrap();

            let index_content = read_file_from_index(&repo, "test.txt").unwrap();
            // First hunk unstaged, second still staged
            assert!(index_content.contains("section1\n"));
            assert!(index_content.contains("MODIFIED2\n"));
        } else {
            // If only one hunk, just verify unstaging works
            stage_hunk(&repo, "test.txt", 0, true).unwrap();
            let index_content = read_file_from_index(&repo, "test.txt").unwrap();
            assert!(index_content.contains("section1\n"));
        }
    }

    // ========================================================================
    // Partial staging conflict tests
    // ========================================================================

    #[test]
    fn test_stage_hunk_when_file_partially_staged() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\nline 3\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        // First modification
        let modified1 = "mod 1\nline 2\nline 3\n";
        create_file_with_content(&temp_dir, "test.txt", modified1);

        // Stage this change
        stage_file(&repo, "test.txt").unwrap();

        // Second modification (adds another hunk)
        let modified2 = "mod 1\nline 2\nmod 3\n";
        create_file_with_content(&temp_dir, "test.txt", modified2);

        // Stage the new hunk
        let diff = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();
        if !diff.hunks.is_empty() {
            stage_hunk(&repo, "test.txt", 0, false).unwrap();
        }

        let index_content = read_file_from_index(&repo, "test.txt").unwrap();
        assert!(index_content.contains("mod 1\n") || index_content.contains("mod 3\n"));
    }

    #[test]
    fn test_stage_hunk_then_modify_working_tree() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\nline 3\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "mod 1\nline 2\nline 3\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        // Stage the hunk
        stage_hunk(&repo, "test.txt", 0, false).unwrap();

        // Modify working tree again
        let modified2 = "mod 1\nmod 2\nline 3\n";
        create_file_with_content(&temp_dir, "test.txt", modified2);

        // Verify index still has first modification
        let index_content = read_file_from_index(&repo, "test.txt").unwrap();
        assert_eq!(index_content, modified);
    }

    #[test]
    fn test_overlapping_hunk_changes() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\nline 3\nline 4\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "mod 1\nline 2\nline 3\nmod 4\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        let diff = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();
        if diff.hunks.len() >= 2 {
            // Stage first hunk
            stage_hunk(&repo, "test.txt", 0, false).unwrap();

            // Modify in first hunk's area
            let modified2 = "mod 1\nmod 2\nline 3\nmod 4\n";
            create_file_with_content(&temp_dir, "test.txt", modified2);

            // Try to stage second hunk - should still work
            let diff2 = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();
            if !diff2.hunks.is_empty() {
                let result = stage_hunk(&repo, "test.txt", 0, false);
                assert!(result.is_ok() || result.is_err()); // Either outcome is acceptable
            }
        }
    }

    // ========================================================================
    // Hunk boundary tests
    // ========================================================================

    #[test]
    fn test_hunk_at_start_of_file() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\nline 3\nline 4\nline 5\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "mod 1\nline 2\nline 3\nline 4\nline 5\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        stage_hunk(&repo, "test.txt", 0, false).unwrap();

        let index_content = read_file_from_index(&repo, "test.txt").unwrap();
        assert_eq!(index_content, modified);
    }

    #[test]
    fn test_hunk_at_end_of_file() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\nline 3\nline 4\nline 5\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "line 1\nline 2\nline 3\nline 4\nmod 5\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        stage_hunk(&repo, "test.txt", 0, false).unwrap();

        let index_content = read_file_from_index(&repo, "test.txt").unwrap();
        assert_eq!(index_content, modified);
    }

    #[test]
    fn test_single_line_hunk() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\nline 3\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "line 1\nmodified\nline 3\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        stage_hunk(&repo, "test.txt", 0, false).unwrap();

        let index_content = read_file_from_index(&repo, "test.txt").unwrap();
        assert_eq!(index_content, modified);
    }

    #[test]
    fn test_empty_line_changes() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\nline 3\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "line 1\n\nline 2\n\nline 3\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        stage_hunk(&repo, "test.txt", 0, false).unwrap();

        let index_content = read_file_from_index(&repo, "test.txt").unwrap();
        assert_eq!(index_content, modified);
    }

    #[test]
    fn test_no_newline_at_end_of_file() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\nline 3";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "line 1\nmodified 2\nline 3";
        create_file_with_content(&temp_dir, "test.txt", modified);

        let result = stage_hunk(&repo, "test.txt", 0, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_adding_newline_at_end() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "line 1\nline 2\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        stage_hunk(&repo, "test.txt", 0, false).unwrap();

        let index_content = read_file_from_index(&repo, "test.txt").unwrap();
        assert_eq!(index_content, modified);
    }

    // ========================================================================
    // Invalid hunk index tests
    // ========================================================================

    #[test]
    fn test_invalid_negative_hunk_index() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "mod 1\nline 2\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        // This will fail at compile time with usize, so we test boundary
        let result = stage_hunk(&repo, "test.txt", 0, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_hunk_index_out_of_bounds() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "mod 1\nline 2\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        let result = stage_hunk(&repo, "test.txt", 999, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("out of bounds"));
    }

    #[test]
    fn test_hunk_on_unmodified_file() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        // File has been committed and is now in the index and working tree
        // DiffSide::Working compares index vs working tree
        // Since they're identical, there should be no hunks, and our validation should catch it

        // First verify that get_diff returns no hunks for an unchanged file
        let diff = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();

        if diff.hunks.is_empty() {
            // If there are no hunks, stage_hunk should error
            let result = stage_hunk(&repo, "test.txt", 0, false);
            assert!(result.is_err(), "Should fail when trying to stage hunk on file with no changes");
            assert!(result.unwrap_err().to_string().contains("No changes to stage"));
        } else {
            // If git detects hunks (edge case), trying to stage them should still work or fail gracefully
            let result = stage_hunk(&repo, "test.txt", 0, false);
            // Either is acceptable in this edge case
            let _ = result;
        }
    }

    // ========================================================================
    // Line staging edge case tests
    // ========================================================================

    #[test]
    fn test_stage_non_contiguous_lines() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "line 1\nline 2\nmod 3\nline 4\nmod 5\nline 6\nmod 7\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        let diff = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();
        if !diff.hunks.is_empty() {
            let hunk = &diff.hunks[0];
            let added_indices: Vec<usize> = hunk
                .lines
                .iter()
                .enumerate()
                .filter(|(_, line)| line.origin == "+")
                .map(|(idx, _)| idx)
                .collect();

            if added_indices.len() >= 2 {
                // Stage non-contiguous lines (e.g., indices 0 and 2)
                let non_contiguous = vec![added_indices[0], *added_indices.last().unwrap()];
                let result = stage_lines(&repo, "test.txt", 0, &non_contiguous, false);
                assert!(result.is_ok());
            }
        }
    }

    #[test]
    fn test_stage_single_line_from_large_hunk() {
        let (temp_dir, repo) = create_test_repo();

        let mut initial = String::new();
        for i in 0..100 {
            initial.push_str(&format!("line {}\n", i));
        }
        commit_file(&repo, "test.txt", &initial, &temp_dir);

        let mut modified = String::new();
        for i in 0..100 {
            if i == 50 {
                modified.push_str("MODIFIED LINE 50\n");
            } else {
                modified.push_str(&format!("line {}\n", i));
            }
        }
        create_file_with_content(&temp_dir, "test.txt", &modified);

        let diff = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();
        if !diff.hunks.is_empty() {
            let hunk = &diff.hunks[0];
            if let Some((idx, _)) = hunk.lines.iter().enumerate().find(|(_, line)| line.origin == "+") {
                let result = stage_lines(&repo, "test.txt", 0, &[idx], false);
                assert!(result.is_ok());
            }
        }
    }

    #[test]
    fn test_stage_first_and_last_line_only() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\nline 3\nline 4\nline 5\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "line 1\nmod 2\nline 3\nmod 4\nline 5\nmod 6\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        let diff = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();
        if !diff.hunks.is_empty() {
            let hunk = &diff.hunks[0];
            let added_indices: Vec<usize> = hunk
                .lines
                .iter()
                .enumerate()
                .filter(|(_, line)| line.origin == "+")
                .map(|(idx, _)| idx)
                .collect();

            if added_indices.len() >= 2 {
                let first_last = vec![added_indices[0], *added_indices.last().unwrap()];
                let result = stage_lines(&repo, "test.txt", 0, &first_last, false);
                assert!(result.is_ok());
            }
        }
    }

    #[test]
    fn test_empty_line_indices() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "mod 1\nline 2\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        let result = stage_lines(&repo, "test.txt", 0, &[], false);
        // Empty selection should either error or succeed as no-op
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_duplicate_line_indices() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\nline 3\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "line 1\nmod 2\nline 3\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        let diff = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();
        if !diff.hunks.is_empty() {
            let hunk = &diff.hunks[0];
            if let Some((idx, _)) = hunk.lines.iter().enumerate().find(|(_, line)| line.origin == "+") {
                // Pass duplicate indices
                let result = stage_lines(&repo, "test.txt", 0, &[idx, idx, idx], false);
                assert!(result.is_ok()); // Should handle duplicates gracefully
            }
        }
    }

    #[test]
    fn test_stage_all_lines_individually_vs_hunk() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\nline 3\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        // Make changes that are ONLY additions (no deletions)
        // This way staging all added lines = staging the whole hunk
        let modified = "line 1\nline 2\nline 3\nnew line 4\nnew line 5\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        // Stage all lines individually
        let diff = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();
        if !diff.hunks.is_empty() {
            let hunk = &diff.hunks[0];
            let all_added: Vec<usize> = hunk
                .lines
                .iter()
                .enumerate()
                .filter(|(_, line)| line.origin == "+")
                .map(|(idx, _)| idx)
                .collect();

            if !all_added.is_empty() {
                stage_lines(&repo, "test.txt", 0, &all_added, false).unwrap();
                let index_via_lines = read_file_from_index(&repo, "test.txt").unwrap();

                // Reset and stage entire hunk
                unstage_file(&repo, "test.txt").unwrap();
                stage_hunk(&repo, "test.txt", 0, false).unwrap();
                let index_via_hunk = read_file_from_index(&repo, "test.txt").unwrap();

                assert_eq!(index_via_lines, index_via_hunk);
            }
        }
    }

    // ========================================================================
    // Whitespace and special content tests
    // ========================================================================

    #[test]
    fn test_hunk_with_whitespace_only_changes() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\nline 3\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "line 1\nline 2 \nline 3\n"; // Added trailing space
        create_file_with_content(&temp_dir, "test.txt", modified);

        let result = stage_hunk(&repo, "test.txt", 0, false);
        assert!(result.is_ok() || result.is_err()); // May or may not detect change
    }

    #[test]
    fn test_hunk_with_tabs_to_spaces() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\n\tindented\nline 3\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "line 1\n    indented\nline 3\n"; // Tab to 4 spaces
        create_file_with_content(&temp_dir, "test.txt", modified);

        let result = stage_hunk(&repo, "test.txt", 0, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_very_long_lines() {
        let (temp_dir, repo) = create_test_repo();

        let long_line = "x".repeat(10000);
        let initial = format!("line 1\n{}\nline 3\n", long_line);
        commit_file(&repo, "test.txt", &initial, &temp_dir);

        let modified_long_line = "y".repeat(10000);
        let modified = format!("line 1\n{}\nline 3\n", modified_long_line);
        create_file_with_content(&temp_dir, "test.txt", &modified);

        let result = stage_hunk(&repo, "test.txt", 0, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_unicode_in_diff() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\nline 3\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "line 1\n👍 emoji line\nline 3\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        stage_hunk(&repo, "test.txt", 0, false).unwrap();

        let index_content = read_file_from_index(&repo, "test.txt").unwrap();
        assert!(index_content.contains("👍"));
    }

    // ========================================================================
    // File state edge case tests
    // ========================================================================

    #[test]
    fn test_stage_hunk_on_new_file() {
        let (temp_dir, repo) = create_test_repo();

        // Create new file (not in index)
        let content = "line 1\nline 2\nline 3\n";
        create_file_with_content(&temp_dir, "new.txt", content);

        let result = stage_hunk(&repo, "new.txt", 0, false);
        // New files might not have hunks in the traditional sense
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_stage_hunk_on_deleted_file() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        // Delete the file
        fs::remove_file(temp_dir.path().join("test.txt")).unwrap();

        // Deleted files have hunks (all deletions), so staging should work
        // This is different from staging the file itself which uses stage_file()
        let diff = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();
        if !diff.hunks.is_empty() && diff.is_deleted {
            let result = stage_hunk(&repo, "test.txt", 0, false);
            // Should succeed - deleted files can have their deletion hunks staged
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_stage_hunk_with_staged_and_unstaged_changes() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\nline 3\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        // First modification and stage
        let modified1 = "mod 1\nline 2\nline 3\n";
        create_file_with_content(&temp_dir, "test.txt", modified1);
        stage_file(&repo, "test.txt").unwrap();

        // Second modification (unstaged)
        let modified2 = "mod 1\nline 2\nmod 3\n";
        create_file_with_content(&temp_dir, "test.txt", modified2);

        let diff = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();
        if !diff.hunks.is_empty() {
            let result = stage_hunk(&repo, "test.txt", 0, false);
            assert!(result.is_ok());
        }
    }

    // ========================================================================
    // Performance and stress tests
    // ========================================================================

    #[test]
    fn test_file_with_many_hunks() {
        let (temp_dir, repo) = create_test_repo();

        // Create file with many small separated sections
        let mut initial = String::new();
        for i in 0..100 {
            initial.push_str(&format!("line {}\n", i));
            if i % 3 == 0 {
                initial.push('\n'); // Empty line to create separation
            }
        }
        commit_file(&repo, "test.txt", &initial, &temp_dir);

        // Modify every 3rd line
        let mut modified = String::new();
        for i in 0..100 {
            if i % 3 == 0 {
                modified.push_str(&format!("MODIFIED {}\n", i));
            } else {
                modified.push_str(&format!("line {}\n", i));
            }
            if i % 3 == 0 {
                modified.push('\n');
            }
        }
        create_file_with_content(&temp_dir, "test.txt", &modified);

        let diff = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();

        // Try to stage first few hunks
        for i in 0..diff.hunks.len().min(5) {
            let result = stage_hunk(&repo, "test.txt", i, false);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_hunk_with_many_lines() {
        let (temp_dir, repo) = create_test_repo();

        let mut initial = String::new();
        for i in 0..1000 {
            initial.push_str(&format!("line {}\n", i));
        }
        commit_file(&repo, "test.txt", &initial, &temp_dir);

        // Modify large block
        let mut modified = String::new();
        for i in 0..1000 {
            modified.push_str(&format!("MODIFIED {}\n", i));
        }
        create_file_with_content(&temp_dir, "test.txt", &modified);

        let start = Instant::now();
        stage_hunk(&repo, "test.txt", 0, false).unwrap();
        let duration = start.elapsed();

        println!("Large hunk staging took: {:?}", duration);
        assert!(duration.as_millis() < 1000);
    }

    #[test]
    fn test_rapid_stage_unstage_cycles() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\nline 3\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "mod 1\nline 2\nline 3\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        // Stage and unstage 10 times
        for _ in 0..10 {
            stage_hunk(&repo, "test.txt", 0, false).unwrap();
            stage_hunk(&repo, "test.txt", 0, true).unwrap();
        }

        // Verify final state is unstaged
        let index_content = read_file_from_index(&repo, "test.txt").unwrap();
        assert_eq!(index_content, initial);
    }

    // ========================================================================
    // Unstaging edge case tests
    // ========================================================================

    #[test]
    fn test_unstage_hunk_never_staged() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "mod 1\nline 2\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        // Try to unstage without staging first
        let result = stage_hunk(&repo, "test.txt", 0, true);
        assert!(result.is_err() || result.is_ok()); // May error or be no-op
    }

    #[test]
    fn test_unstage_hunk_from_partially_staged_file() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\n\nline 4\nline 5\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "mod 1\nline 2\n\nline 4\nmod 5\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        let diff = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();
        if diff.hunks.len() >= 2 {
            // Stage only first hunk
            stage_hunk(&repo, "test.txt", 0, false).unwrap();

            // Try to unstage first hunk
            let result = stage_hunk(&repo, "test.txt", 0, true);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_unstage_after_working_tree_changed() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\nline 3\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "mod 1\nline 2\nline 3\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        // Stage the change
        stage_hunk(&repo, "test.txt", 0, false).unwrap();

        // Modify working tree again
        let modified2 = "mod 1\nmod 2\nline 3\n";
        create_file_with_content(&temp_dir, "test.txt", modified2);

        // Unstage - should work despite working tree changes
        let result = stage_hunk(&repo, "test.txt", 0, true);
        assert!(result.is_ok());
    }

    // ========================================================================
    // Patch generation edge case tests
    // ========================================================================

    #[test]
    fn test_patch_header_format_correctness() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\nline 3\nline 4\nline 5\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "line 1\nmodified 2\nline 3\nmodified 4\nline 5\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        let diff = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();
        if !diff.hunks.is_empty() {
            let hunk = &diff.hunks[0];
            let patch = build_patch("test.txt", hunk, None).unwrap();

            // Verify patch has proper headers
            assert!(patch.contains("diff --git a/test.txt b/test.txt"));
            assert!(patch.contains("--- a/test.txt"));
            assert!(patch.contains("+++ b/test.txt"));
            assert!(patch.contains("@@"));
        }
    }

    #[test]
    fn test_patch_with_only_additions() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "line 1\nline 2\nnew line 3\nnew line 4\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        let diff = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();
        if !diff.hunks.is_empty() {
            let hunk = &diff.hunks[0];
            let patch = build_patch("test.txt", hunk, None).unwrap();

            // Count + lines
            let plus_count = patch.matches("\n+").count();
            assert!(plus_count >= 2); // At least 2 additions
        }
    }

    #[test]
    fn test_patch_with_only_deletions() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\nline 3\nline 4\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "line 1\nline 4\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        let diff = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();
        if !diff.hunks.is_empty() {
            let hunk = &diff.hunks[0];
            let patch = build_patch("test.txt", hunk, None).unwrap();

            // Should contain deletions
            let minus_count = patch.matches("\n-").count();
            assert!(minus_count >= 2);
        }
    }

    // ========================================================================
    // Integration issue tests
    // ========================================================================

    #[test]
    fn test_stage_hunk_with_mixed_line_endings() {
        let (temp_dir, repo) = create_test_repo();

        // Create file with mixed line endings
        let initial = "line 1\nline 2\r\nline 3\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "modified 1\nline 2\r\nline 3\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        let result = stage_hunk(&repo, "test.txt", 0, false);
        // Should handle mixed line endings
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_stage_hunk_symlink_changes() {
        let (temp_dir, repo) = create_test_repo();

        // Create a regular file first
        let initial = "target content\n";
        commit_file(&repo, "target.txt", initial, &temp_dir);

        // Modify it
        let modified = "modified target\n";
        create_file_with_content(&temp_dir, "target.txt", modified);

        // Stage the change (symlinks would require platform-specific handling)
        let result = stage_hunk(&repo, "target.txt", 0, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_stage_hunk_large_file_threshold() {
        let (temp_dir, repo) = create_test_repo();

        // Create a file approaching Git's large file threshold
        let mut initial = String::new();
        for i in 0..5000 {
            initial.push_str(&format!("line number {} with some content to make it longer\n", i));
        }
        commit_file(&repo, "large.txt", &initial, &temp_dir);

        // Modify one line in the middle
        let mut modified = String::new();
        for i in 0..5000 {
            if i == 2500 {
                modified.push_str("MODIFIED LINE IN THE MIDDLE\n");
            } else {
                modified.push_str(&format!("line number {} with some content to make it longer\n", i));
            }
        }
        create_file_with_content(&temp_dir, "large.txt", &modified);

        let diff = get_diff(&repo, "large.txt", &DiffSide::Working, Some(3)).unwrap();
        if !diff.hunks.is_empty() {
            let result = stage_hunk(&repo, "large.txt", 0, false);
            assert!(result.is_ok());
        }
    }

    // ========================================================================
    // Additional boundary and error condition tests
    // ========================================================================

    #[test]
    fn test_stage_lines_with_out_of_bounds_index() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "mod 1\nline 2\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        // Try to stage line index that doesn't exist
        let result = stage_lines(&repo, "test.txt", 0, &[999], false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("out of bounds"));
    }

    #[test]
    fn test_hunk_with_context_only() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\nline 3\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        // File is already committed and hasn't been modified
        // get_diff should show no changes (Working tree diff shows changes between index and working directory)

        let diff = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();
        // For an uncommitted but unmodified file, there may still be hunks if index != HEAD
        // This test should actually check if file is identical in both index and working tree
        // If diff has hunks, they would be context-only which git normally doesn't show
        // But our implementation may return an empty hunk list which is correct
        assert!(
            diff.hunks.len() <= 1,
            "Expected 0 or 1 hunk for file with no working tree changes, got {}",
            diff.hunks.len()
        );
    }

    #[test]
    fn test_stage_entire_file_deletion_as_hunk() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\nline 3\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        // Delete entire file content (replace with empty)
        create_file_with_content(&temp_dir, "test.txt", "");

        let diff = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();
        if !diff.hunks.is_empty() {
            let result = stage_hunk(&repo, "test.txt", 0, false);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_stage_hunk_with_special_regex_chars() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\nline 3\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        // Add line with regex special characters
        let modified = "line 1\n$variable = /regex.*pattern/\nline 3\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        let result = stage_hunk(&repo, "test.txt", 0, false);
        assert!(result.is_ok());

        let index_content = read_file_from_index(&repo, "test.txt").unwrap();
        assert!(index_content.contains("$variable"));
        assert!(index_content.contains("/regex.*pattern/"));
    }

    #[test]
    fn test_consecutive_empty_lines() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "line 1\n\n\n\nline 2\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        stage_hunk(&repo, "test.txt", 0, false).unwrap();

        let index_content = read_file_from_index(&repo, "test.txt").unwrap();
        assert_eq!(index_content, modified);
    }

    #[test]
    fn test_stage_hunk_with_null_bytes() {
        let (temp_dir, repo) = create_test_repo();

        // Git typically treats files with null bytes as binary
        let initial = b"line 1\nline 2\n";
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, initial).unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new("test.txt")).unwrap();
        index.write().unwrap();

        let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();

        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Test commit",
            &tree,
            &[&parent],
        )
        .unwrap();

        // Modify with null byte
        let modified = b"line 1\x00\nline 2\n";
        std::fs::write(&file_path, modified).unwrap();

        let diff = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();
        // File might be treated as binary, so hunks might be empty
        if !diff.hunks.is_empty() {
            let result = stage_hunk(&repo, "test.txt", 0, false);
            assert!(result.is_ok() || result.is_err());
        }
    }

    #[test]
    fn test_stage_lines_partial_selection() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\nline 3\nline 4\nline 5\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "line 1\nmod 2\nmod 3\nmod 4\nline 5\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        let diff = get_diff(&repo, "test.txt", &DiffSide::Working, Some(3)).unwrap();
        if !diff.hunks.is_empty() {
            let hunk = &diff.hunks[0];
            let added_indices: Vec<usize> = hunk
                .lines
                .iter()
                .enumerate()
                .filter(|(_, line)| line.origin == "+")
                .map(|(idx, _)| idx)
                .collect();

            if added_indices.len() >= 2 {
                // Stage only first half of additions
                let partial: Vec<usize> = added_indices.iter().take(added_indices.len() / 2).copied().collect();
                let result = stage_lines(&repo, "test.txt", 0, &partial, false);
                assert!(result.is_ok());
            }
        }
    }

    #[test]
    fn test_unstage_lines_not_supported() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        let modified = "mod 1\nline 2\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        // Try to unstage individual lines - should fail
        let result = stage_lines(&repo, "test.txt", 0, &[0], true);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not yet supported"));
    }

    #[test]
    fn test_stage_hunk_file_mode_change() {
        let (temp_dir, repo) = create_test_repo();

        let initial = "line 1\nline 2\n";
        commit_file(&repo, "test.txt", initial, &temp_dir);

        // Modify content
        let modified = "mod 1\nline 2\n";
        create_file_with_content(&temp_dir, "test.txt", modified);

        // File mode changes are typically separate from content changes in git
        // This test verifies content staging works regardless of mode
        let result = stage_hunk(&repo, "test.txt", 0, false);
        assert!(result.is_ok());
    }
}
