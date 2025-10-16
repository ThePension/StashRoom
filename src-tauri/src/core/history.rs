use anyhow::{Context, Result};
use git2::{Commit, Diff, DiffOptions, Oid, Repository};
use std::collections::HashMap;

use crate::types::{
    CommitAuthor, CommitDiffHunk, CommitDiffLine, CommitFileDiff, CommitFileChangeType,
    CommitSummary, GetCommitDiffResponse, GetLogResponse,
};

/// Get commit log for the current branch
pub fn get_log(repo: &Repository, limit: usize, skip: Option<usize>) -> Result<GetLogResponse> {
    let mut revwalk = repo.revwalk().context("Failed to create revwalk")?;

    // Start from HEAD
    let head = repo.head().context("Failed to get HEAD")?;
    revwalk
        .push(head.target().context("HEAD has no target")?)
        .context("Failed to push HEAD to revwalk")?;

    // Sort by topological order and time
    revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;

    let skip_count = skip.unwrap_or(0);
    let mut commits = Vec::new();
    let mut count = 0;
    let mut skipped = 0;

    // Get references for detecting branch/tag names
    let refs_map = get_refs_map(repo)?;

    let mut has_more = false;
    for oid_result in revwalk {
        let oid = oid_result?;

        // Skip the requested number of commits
        if skipped < skip_count {
            skipped += 1;
            continue;
        }

        // Stop when we've collected enough
        if count >= limit {
            // There's at least one more commit
            has_more = true;
            break;
        }

        let commit = repo.find_commit(oid)?;
        let summary = build_commit_summary(&commit, &refs_map)?;
        commits.push(summary);
        count += 1;
    }

    Ok(GetLogResponse { commits, has_more })
}

/// Build a CommitSummary from a git2::Commit
fn build_commit_summary(
    commit: &Commit,
    refs_map: &HashMap<Oid, Vec<String>>,
) -> Result<CommitSummary> {
    let oid = commit.id().to_string();
    let short_oid = commit.id().to_string()[..7].to_string();

    let author = commit.author();
    let author_info = CommitAuthor {
        name: author.name().unwrap_or("Unknown").to_string(),
        email: author.email().unwrap_or("").to_string(),
    };

    let time = author.when().seconds();
    let message = commit.message().unwrap_or("").to_string();

    // Subject is the first line
    let subject = message.lines().next().unwrap_or("").to_string();

    // Get refs pointing to this commit
    let refs = refs_map.get(&commit.id()).cloned();

    Ok(CommitSummary {
        oid,
        short_oid,
        author: author_info,
        time,
        message,
        subject,
        refs,
    })
}

/// Get a map of OID -> ref names (branches, tags, HEAD)
fn get_refs_map(repo: &Repository) -> Result<HashMap<Oid, Vec<String>>> {
    let mut refs_map: HashMap<Oid, Vec<String>> = HashMap::new();

    // Get HEAD
    if let Ok(head) = repo.head() {
        if let Some(target) = head.target() {
            let mut ref_names = vec![];

            // Add HEAD indicator
            if head.is_branch() {
                if let Some(name) = head.shorthand() {
                    ref_names.push(format!("HEAD -> {}", name));
                }
            } else {
                ref_names.push("HEAD".to_string());
            }

            refs_map.insert(target, ref_names);
        }
    }

    // Get all branches
    let branches = repo.branches(None)?;
    for branch_result in branches {
        let (branch, _) = branch_result?;
        if let Some(oid) = branch.get().target() {
            if let Some(name) = branch.name()? {
                refs_map
                    .entry(oid)
                    .or_insert_with(Vec::new)
                    .push(name.to_string());
            }
        }
    }

    // Get all tags
    repo.tag_foreach(|oid, name| {
        // Convert name from bytes to string
        if let Ok(name_str) = std::str::from_utf8(name) {
            // Remove "refs/tags/" prefix
            let tag_name = name_str.strip_prefix("refs/tags/").unwrap_or(name_str);

            // For annotated tags, we need to peel to the commit
            if let Ok(obj) = repo.find_object(oid, None) {
                if let Ok(commit_oid) = obj.peel_to_commit().map(|c| c.id()) {
                    refs_map
                        .entry(commit_oid)
                        .or_insert_with(Vec::new)
                        .push(format!("tag: {}", tag_name));
                }
            }
        }
        true // Continue iteration
    })?;

    Ok(refs_map)
}

/// Get diff for a specific commit vs its parent
pub fn get_commit_diff(
    repo: &Repository,
    oid_str: &str,
    parent_idx: Option<usize>,
) -> Result<GetCommitDiffResponse> {
    let oid = Oid::from_str(oid_str).context("Invalid commit OID")?;
    let commit = repo.find_commit(oid).context("Commit not found")?;

    let parent_idx = parent_idx.unwrap_or(0);

    // Get parent commit or empty tree for root commits
    let parent_tree = if commit.parent_count() > 0 {
        if parent_idx >= commit.parent_count() {
            anyhow::bail!("Parent index {} out of range (commit has {} parents)", parent_idx, commit.parent_count());
        }
        let parent = commit.parent(parent_idx)?;
        Some(parent.tree()?)
    } else {
        // Root commit - compare against empty tree
        None
    };

    let commit_tree = commit.tree()?;

    // Create diff options with rename detection
    let mut diff_opts = DiffOptions::new();
    diff_opts
        .ignore_whitespace(false)
        .context_lines(3)
        .interhunk_lines(0);

    // Create diff
    let diff = if let Some(parent_tree) = parent_tree {
        repo.diff_tree_to_tree(Some(&parent_tree), Some(&commit_tree), Some(&mut diff_opts))?
    } else {
        // Root commit: diff against empty tree
        repo.diff_tree_to_tree(None, Some(&commit_tree), Some(&mut diff_opts))?
    };

    // Enable rename detection
    let mut find_opts = git2::DiffFindOptions::new();
    find_opts.renames(true).copies(true);

    let mut diff = diff;
    diff.find_similar(Some(&mut find_opts))?;

    // Build file diffs
    let files = build_commit_file_diffs(&diff)?;

    Ok(GetCommitDiffResponse { files })
}

/// Build CommitFileDiff array from git2::Diff
fn build_commit_file_diffs(diff: &Diff) -> Result<Vec<CommitFileDiff>> {
    let mut files = Vec::new();

    diff.foreach(
        &mut |delta, _progress| {
            let path = delta.new_file().path().unwrap_or_else(|| std::path::Path::new("")).to_string_lossy().to_string();
            let old_path = if delta.status() == git2::Delta::Renamed || delta.status() == git2::Delta::Copied {
                delta.old_file().path().map(|p| p.to_string_lossy().to_string())
            } else {
                None
            };

            let change = match delta.status() {
                git2::Delta::Added => CommitFileChangeType::Added,
                git2::Delta::Deleted => CommitFileChangeType::Deleted,
                git2::Delta::Modified => CommitFileChangeType::Modified,
                git2::Delta::Renamed => CommitFileChangeType::Renamed,
                git2::Delta::Copied => CommitFileChangeType::Copied,
                _ => CommitFileChangeType::Modified,
            };

            let is_binary = delta.new_file().is_binary() || delta.old_file().is_binary();

            files.push(CommitFileDiff {
                path,
                old_path,
                change,
                is_binary,
                hunks: None, // Will be filled in print_cb
            });

            true
        },
        None, // binary_cb
        None, // hunk_cb
        None, // line_cb
    )?;

    // Now collect hunks and lines for each file
    for file_diff in &mut files {
        if file_diff.is_binary {
            continue;
        }

        let hunks = collect_hunks_for_file(diff, &file_diff.path)?;
        file_diff.hunks = Some(hunks);
    }

    Ok(files)
}

/// Collect hunks and lines for a specific file
fn collect_hunks_for_file(diff: &Diff, file_path: &str) -> Result<Vec<CommitDiffHunk>> {
    use std::cell::RefCell;
    use std::rc::Rc;

    let hunks = Rc::new(RefCell::new(Vec::new()));
    let current_hunk: Rc<RefCell<Option<CommitDiffHunk>>> = Rc::new(RefCell::new(None));

    let hunks_clone = Rc::clone(&hunks);
    let current_hunk_clone = Rc::clone(&current_hunk);
    let current_hunk_clone2 = Rc::clone(&current_hunk);
    let file_path_owned = file_path.to_string();
    let file_path_owned2 = file_path.to_string();

    diff.foreach(
        &mut |_delta, _progress| {
            // Always return true to continue processing all files
            true
        },
        None,
        Some(&mut move |delta, hunk| {
            let path = delta.new_file().path().unwrap_or_else(|| std::path::Path::new("")).to_string_lossy();
            // Only process hunks for the requested file
            if path != file_path_owned.as_str() {
                return true;
            }

            // Save previous hunk if exists
            if let Some(hunk_data) = current_hunk_clone.borrow_mut().take() {
                hunks_clone.borrow_mut().push(hunk_data);
            }

            // Start new hunk
            let header = String::from_utf8_lossy(hunk.header()).to_string();
            *current_hunk_clone.borrow_mut() = Some(CommitDiffHunk {
                header,
                old_start: hunk.old_start(),
                old_lines: hunk.old_lines(),
                new_start: hunk.new_start(),
                new_lines: hunk.new_lines(),
                lines: Vec::new(),
            });

            true
        }),
        Some(&mut move |delta, _hunk, line| {
            let path = delta.new_file().path().unwrap_or_else(|| std::path::Path::new("")).to_string_lossy();
            // Only process lines for the requested file
            if path != file_path_owned2.as_str() {
                return true;
            }

            if let Some(ref mut hunk_data) = *current_hunk_clone2.borrow_mut() {
                let line_type = match line.origin() {
                    '+' => "add",
                    '-' => "del",
                    ' ' => "ctx",
                    _ => "ctx",
                };

                let text = String::from_utf8_lossy(line.content()).to_string();

                hunk_data.lines.push(CommitDiffLine {
                    ln_old: line.old_lineno(),
                    ln_new: line.new_lineno(),
                    line_type: line_type.to_string(),
                    text,
                });
            }

            true
        }),
    )?;

    // Save last hunk
    if let Some(hunk_data) = current_hunk.borrow_mut().take() {
        hunks.borrow_mut().push(hunk_data);
    }

    // Extract the result from Rc<RefCell<>>
    let result = Rc::try_unwrap(hunks)
        .map_err(|_| anyhow::anyhow!("Failed to unwrap Rc"))?
        .into_inner();

    Ok(result)
}
