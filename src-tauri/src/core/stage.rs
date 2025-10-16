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
    let file_diff = get_diff(repo, path, &DiffSide::Working)?;

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
///
/// Assumes validation has already been performed by validate_line_selection.
fn stage_lines_impl(
    repo: &Repository,
    path: &str,
    hunk_index: usize,
    line_indices: &[usize],
) -> Result<()> {
    // Get the working tree diff
    let file_diff = get_diff(repo, path, &DiffSide::Working)?;

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
        // When filtering lines, we need to build a contiguous patch
        // Strategy: Find the range from first to last selected line, include all context
        // and deletions in that range, but only selected additions
        let filter_set: std::collections::HashSet<usize> = filter.iter().copied().collect();

        // Find the range of selected lines
        let min_idx = *filter.iter().min().unwrap_or(&0);
        let max_idx = *filter.iter().max().unwrap_or(&0);

        let mut old_count = 0;
        let mut new_count = 0;
        let mut old_start = hunk.old_start;
        let mut new_start = hunk.new_start;

        // Adjust start positions based on skipped lines
        let mut old_offset = 0;
        let mut new_offset = 0;
        for (idx, line) in hunk.lines.iter().enumerate() {
            if idx < min_idx {
                match line.origin.as_str() {
                    " " => {
                        old_offset += 1;
                        new_offset += 1;
                    }
                    "-" => old_offset += 1,
                    "+" => new_offset += 1,
                    _ => {}
                }
            }
        }
        old_start += old_offset;
        new_start += new_offset;

        // Count lines in the selected range
        for (idx, line) in hunk.lines.iter().enumerate() {
            if idx >= min_idx && idx <= max_idx {
                match line.origin.as_str() {
                    " " => {
                        old_count += 1;
                        new_count += 1;
                    }
                    "-" => old_count += 1,
                    "+" => {
                        if filter_set.contains(&idx) {
                            new_count += 1;
                        }
                    }
                    _ => {}
                }
            }
        }

        patch.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            old_start, old_count, new_start, new_count
        ));

        // Add lines in the selected range
        for (idx, line) in hunk.lines.iter().enumerate() {
            if idx >= min_idx && idx <= max_idx {
                match line.origin.as_str() {
                    " " | "-" => {
                        // Include all context and deletions in range
                        patch.push_str(&format!("{}{}", line.origin, line.content));
                    }
                    "+" => {
                        // Only include selected additions
                        if filter_set.contains(&idx) {
                            patch.push_str(&format!("{}{}", line.origin, line.content));
                        }
                    }
                    _ => {}
                }
            }
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
        let diff = get_diff(&repo, "test.txt", &DiffSide::Working).unwrap();
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
        let diff = get_diff(&repo, "test.txt", &DiffSide::Working).unwrap();
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
        let diff = get_diff(&repo, "test.txt", &DiffSide::Working).unwrap();
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
        let diff = get_diff(&repo, "test.txt", &DiffSide::Working).unwrap();
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
        let diff = get_diff(&repo, "test.txt", &DiffSide::Working).unwrap();
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
}
