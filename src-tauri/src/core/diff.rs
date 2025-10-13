use anyhow::{Context, Result};
use git2::{Diff, DiffDelta, DiffOptions, Repository};
use std::cell::RefCell;
use std::path::Path;

use crate::types::{DiffHunk, DiffLine, DiffSide, FileDiff};

/// Generates a diff for a specific file based on the specified side
pub fn get_diff(repo: &Repository, path: &str, side: &DiffSide) -> Result<FileDiff> {
    let mut opts = DiffOptions::new();
    opts.pathspec(path);
    opts.context_lines(3);
    // Disable text conversion to preserve CRLF exactly as stored
    opts.disable_pathspec_match(false);
    opts.ignore_whitespace_change(false);

    let diff = match side {
        DiffSide::Working => {
            // Diff between index and working directory (unstaged changes only)
            repo.diff_index_to_workdir(None, Some(&mut opts))?
        }
        DiffSide::Index => {
            // Diff between HEAD and index (staged changes)
            let head = repo.head()?.peel_to_tree()?;
            let mut index = repo.index()?;
            let index_tree = repo.find_tree(index.write_tree()?)?;
            repo.diff_tree_to_tree(Some(&head), Some(&index_tree), Some(&mut opts))?
        }
        DiffSide::Head => {
            // Diff between HEAD and working directory (all changes)
            let head = repo.head()?.peel_to_tree()?;
            repo.diff_tree_to_workdir_with_index(Some(&head), Some(&mut opts))?
        }
    };

    parse_diff(&diff, path, repo)
}

/// Parses a libgit2 Diff into our FileDiff format
fn parse_diff(diff: &Diff, path: &str, repo: &Repository) -> Result<FileDiff> {
    let file_diff = RefCell::new(None);

    diff.foreach(
        &mut |delta: DiffDelta, _progress| {
            if let Some(new_path) = delta.new_file().path() {
                if new_path.to_str() == Some(path) {
                    let old_path = delta.old_file().path().and_then(|p| p.to_str()).map(String::from);
                    let is_binary = delta.new_file().is_binary() || delta.old_file().is_binary();
                    let is_new = delta.status() == git2::Delta::Added;
                    let is_deleted = delta.status() == git2::Delta::Deleted;

                    *file_diff.borrow_mut() = Some(FileDiff {
                        path: path.to_string(),
                        old_path,
                        is_binary,
                        is_new,
                        is_deleted,
                        hunks: Vec::new(),
                    });
                }
            }
            true
        },
        None,
        Some(&mut |_delta, hunk| {
            if let Some(ref mut fd) = *file_diff.borrow_mut() {
                let header = String::from_utf8_lossy(hunk.header()).to_string();
                fd.hunks.push(DiffHunk {
                    header: header.trim_end().to_string(),
                    old_start: hunk.old_start(),
                    old_lines: hunk.old_lines(),
                    new_start: hunk.new_start(),
                    new_lines: hunk.new_lines(),
                    lines: Vec::new(),
                });
            }
            true
        }),
        Some(&mut |_delta, _hunk, line| {
            if let Some(ref mut fd) = *file_diff.borrow_mut() {
                if let Some(last_hunk) = fd.hunks.last_mut() {
                    let origin = match line.origin() {
                        '+' => "+",
                        '-' => "-",
                        ' ' => " ",
                        _ => "",
                    };

                    // Preserve exact content including CRLF
                    let content = String::from_utf8_lossy(line.content()).to_string();

                    last_hunk.lines.push(DiffLine {
                        origin: origin.to_string(),
                        content,
                        old_lineno: line.old_lineno(),
                        new_lineno: line.new_lineno(),
                    });
                }
            }
            true
        }),
    )
    .context("Failed to parse diff")?;

    let mut file_diff = file_diff.into_inner();

    // If no diff was found, it might be a new untracked file
    if file_diff.is_none() {
        // Try to read the file from working directory
        let workdir = repo.workdir().context("Repository has no working directory")?;
        let file_path = workdir.join(path);

        if file_path.exists() {
            let is_binary = is_binary_file(&file_path)?;

            if !is_binary {
                let content = std::fs::read_to_string(&file_path)?;
                let lines: Vec<DiffLine> = content
                    .lines()
                    .enumerate()
                    .map(|(i, line)| DiffLine {
                        origin: "+".to_string(),
                        content: format!("{}\n", line),
                        old_lineno: None,
                        new_lineno: Some(i as u32 + 1),
                    })
                    .collect();

                let hunk = DiffHunk {
                    header: format!("@@ -0,0 +1,{} @@", lines.len()),
                    old_start: 0,
                    old_lines: 0,
                    new_start: 1,
                    new_lines: lines.len() as u32,
                    lines,
                };

                file_diff = Some(FileDiff {
                    path: path.to_string(),
                    old_path: None,
                    is_binary: false,
                    is_new: true,
                    is_deleted: false,
                    hunks: vec![hunk],
                });
            } else {
                file_diff = Some(FileDiff {
                    path: path.to_string(),
                    old_path: None,
                    is_binary: true,
                    is_new: true,
                    is_deleted: false,
                    hunks: Vec::new(),
                });
            }
        }
    }

    file_diff.ok_or_else(|| anyhow::anyhow!("No diff found for path: {}", path))
}

/// Simple binary detection heuristic
fn is_binary_file(path: &Path) -> Result<bool> {
    let file = std::fs::File::open(path)?;
    let mut buffer = [0u8; 8192];
    let bytes_read = std::io::Read::read(&mut std::io::BufReader::new(file), &mut buffer)?;

    // Check for null bytes (common binary indicator)
    let has_null = buffer[..bytes_read].contains(&0);

    // Check for high ratio of non-printable characters
    let non_printable_count = buffer[..bytes_read]
        .iter()
        .filter(|&&b| b < 32 && b != b'\n' && b != b'\r' && b != b'\t')
        .count();

    let ratio = non_printable_count as f64 / bytes_read as f64;

    Ok(has_null || ratio > 0.3)
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
    fn test_new_file_diff() {
        let (temp_dir, repo) = create_test_repo();

        let file_path = temp_dir.path().join("new_file.txt");
        fs::write(&file_path, "Line 1\nLine 2\nLine 3\n").unwrap();

        let diff = get_diff(&repo, "new_file.txt", &DiffSide::Working).unwrap();

        assert_eq!(diff.path, "new_file.txt");
        assert!(!diff.is_binary);
        assert!(diff.is_new);
        assert!(!diff.is_deleted);
        assert_eq!(diff.hunks.len(), 1);

        let hunk = &diff.hunks[0];
        assert_eq!(hunk.new_lines, 3);
        assert_eq!(hunk.lines.len(), 3);
        assert!(hunk.lines.iter().all(|l| l.origin == "+"));
    }

    #[test]
    fn test_modified_file_diff() {
        let (temp_dir, repo) = create_test_repo();

        // Create and commit a file
        let file_path = temp_dir.path().join("file.txt");
        fs::write(&file_path, "Line 1\nLine 2\nLine 3\n").unwrap();

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
        fs::write(&file_path, "Line 1\nModified Line 2\nLine 3\n").unwrap();

        let diff = get_diff(&repo, "file.txt", &DiffSide::Working).unwrap();

        assert_eq!(diff.path, "file.txt");
        assert!(!diff.is_binary);
        assert!(!diff.is_new);
        assert!(!diff.is_deleted);
        assert!(diff.hunks.len() > 0);

        // Check that we have both removed and added lines
        let has_removal = diff.hunks.iter().any(|h| h.lines.iter().any(|l| l.origin == "-"));
        let has_addition = diff.hunks.iter().any(|h| h.lines.iter().any(|l| l.origin == "+"));

        assert!(has_removal);
        assert!(has_addition);
    }

    #[test]
    fn test_staged_diff() {
        let (temp_dir, repo) = create_test_repo();

        // Create and commit a file
        let file_path = temp_dir.path().join("file.txt");
        fs::write(&file_path, "Line 1\nLine 2\n").unwrap();

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
        fs::write(&file_path, "Line 1\nStaged Line 2\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        index.write().unwrap();

        let diff = get_diff(&repo, "file.txt", &DiffSide::Index).unwrap();

        assert_eq!(diff.path, "file.txt");
        assert!(!diff.is_binary);
        assert!(!diff.is_new);
        assert!(!diff.is_deleted);
        assert!(diff.hunks.len() > 0);
    }

    #[test]
    fn test_binary_file_detection() {
        let temp_dir = TempDir::new().unwrap();

        // Create a binary file
        let binary_path = temp_dir.path().join("binary.bin");
        let binary_data: Vec<u8> = vec![0, 1, 2, 3, 0xFF, 0xFE, 0xFD];
        fs::write(&binary_path, &binary_data).unwrap();

        assert!(is_binary_file(&binary_path).unwrap());

        // Create a text file
        let text_path = temp_dir.path().join("text.txt");
        fs::write(&text_path, "Hello, World!\n").unwrap();

        assert!(!is_binary_file(&text_path).unwrap());
    }
}
