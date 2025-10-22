use anyhow::{Context, Result};
use git2::{Commit, Diff, DiffOptions, Oid, Repository};
use std::collections::HashMap;

use crate::types::{
    CommitAuthor, CommitDiffHunk, CommitDiffLine, CommitFileDiff, CommitFileChangeType,
    CommitFileMatch, CommitSummary, FileSearchResult, GetCommitDiffResponse, GetLogResponse,
    SearchFilesResponse,
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

/// Search for files across commit history
pub fn search_files(repo: &Repository, query: &str, result_limit: usize) -> Result<SearchFilesResponse> {
    if query.trim().is_empty() {
        return Ok(SearchFilesResponse {
            results: Vec::new(),
        });
    }

    let query_lower = query.to_lowercase();
    let mut revwalk = repo.revwalk().context("Failed to create revwalk")?;

    // Start from HEAD
    let head = repo.head().context("Failed to get HEAD")?;
    revwalk
        .push(head.target().context("HEAD has no target")?)
        .context("Failed to push HEAD to revwalk")?;

    revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;

    // Get references for building commit summaries
    let refs_map = get_refs_map(repo)?;

    // Map: file path -> (list of commits, best score for this file)
    let mut file_map: HashMap<String, (Vec<CommitFileMatch>, i32)> = HashMap::new();
    let mut commit_count = 0;
    let commit_limit = 200; // Search through up to 200 commits

    for oid_result in revwalk {
        if commit_count >= commit_limit {
            break;
        }

        let oid = oid_result?;
        let commit = repo.find_commit(oid)?;

        // Get parent tree or None for root commits
        let parent_tree = if commit.parent_count() > 0 {
            Some(commit.parent(0)?.tree()?)
        } else {
            None
        };

        let commit_tree = commit.tree()?;

        // Create diff options
        let mut diff_opts = DiffOptions::new();
        diff_opts.ignore_whitespace(false);

        // Create diff
        let diff = if let Some(parent_tree) = parent_tree {
            repo.diff_tree_to_tree(Some(&parent_tree), Some(&commit_tree), Some(&mut diff_opts))?
        } else {
            repo.diff_tree_to_tree(None, Some(&commit_tree), Some(&mut diff_opts))?
        };

        // Enable rename detection
        let mut find_opts = git2::DiffFindOptions::new();
        find_opts.renames(true);
        let mut diff = diff;
        diff.find_similar(Some(&mut find_opts))?;

        // Check each file in the diff
        diff.foreach(
            &mut |delta, _progress| {
                let path = delta
                    .new_file()
                    .path()
                    .unwrap_or_else(|| std::path::Path::new(""))
                    .to_string_lossy()
                    .to_string();

                // Extract filename from path for matching
                let filename = path.rsplit('/').next()
                    .or_else(|| path.rsplit('\\').next())
                    .unwrap_or(&path);

                // Fuzzy match with scoring (only against filename)
                let (matches, score) = fuzzy_match_with_score(&query_lower, &filename.to_lowercase());

                if matches {
                    let change = match delta.status() {
                        git2::Delta::Added => CommitFileChangeType::Added,
                        git2::Delta::Deleted => CommitFileChangeType::Deleted,
                        git2::Delta::Modified => CommitFileChangeType::Modified,
                        git2::Delta::Renamed => CommitFileChangeType::Renamed,
                        git2::Delta::Copied => CommitFileChangeType::Copied,
                        _ => CommitFileChangeType::Modified,
                    };

                    // Build commit summary
                    if let Ok(commit_summary) = build_commit_summary(&commit, &refs_map) {
                        let entry = file_map
                            .entry(path.clone())
                            .or_insert_with(|| (Vec::new(), 0));

                        entry.0.push(CommitFileMatch {
                            commit: commit_summary,
                            change,
                        });

                        // Keep track of the best score for this file
                        entry.1 = entry.1.max(score);
                    }
                }

                true
            },
            None,
            None,
            None,
        )?;

        commit_count += 1;
    }

    // Convert map to results
    let mut results: Vec<(FileSearchResult, i32)> = file_map
        .into_iter()
        .map(|(path, (commits, score))| {
            (FileSearchResult { path, commits }, score)
        })
        .collect();

    // Sort by score (descending), then by number of commits (descending)
    results.sort_by(|a, b| {
        b.1.cmp(&a.1) // Primary: sort by score
            .then_with(|| b.0.commits.len().cmp(&a.0.commits.len())) // Secondary: by commit count
    });

    // Remove scores and limit results
    let results: Vec<FileSearchResult> = results
        .into_iter()
        .take(result_limit)
        .map(|(result, _)| result)
        .collect();

    Ok(SearchFilesResponse { results })
}

/// Advanced fuzzy matching with scoring
/// Returns (matches: bool, score: i32) where higher scores indicate better matches
fn fuzzy_match_with_score(pattern: &str, text: &str) -> (bool, i32) {
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();

    if pattern_chars.is_empty() {
        return (true, 0);
    }

    if text_chars.is_empty() {
        return (false, 0);
    }

    // Check if all pattern characters exist in text (case-insensitive)
    let mut pattern_idx = 0;
    for &text_ch in &text_chars {
        if pattern_idx < pattern_chars.len() {
            let pattern_ch_lower = pattern_chars[pattern_idx].to_lowercase().next().unwrap_or(pattern_chars[pattern_idx]);
            let text_ch_lower = text_ch.to_lowercase().next().unwrap_or(text_ch);
            if text_ch_lower == pattern_ch_lower {
                pattern_idx += 1;
            }
        }
    }

    if pattern_idx != pattern_chars.len() {
        return (false, 0); // Not all pattern chars found
    }

    // Calculate score
    let score = calculate_match_score(&pattern_chars, &text_chars);
    (true, score)
}

/// Calculate match score with various bonuses and penalties
fn calculate_match_score(pattern_chars: &[char], text_chars: &[char]) -> i32 {
    let mut score = 0;
    let mut pattern_idx = 0;
    let mut consecutive_matches = 0;
    let mut prev_match_idx: Option<usize> = None;

    for (text_idx, &text_ch) in text_chars.iter().enumerate() {
        if pattern_idx < pattern_chars.len() {
            let pattern_ch_lower = pattern_chars[pattern_idx].to_lowercase().next().unwrap_or(pattern_chars[pattern_idx]);
            let text_ch_lower = text_ch.to_lowercase().next().unwrap_or(text_ch);

            if text_ch_lower != pattern_ch_lower {
                continue;
            }
            // Base score for match
            score += 10;

            // Bonus #1: Start of string (very important)
            if text_idx == 0 {
                score += 50;
            }

            // Bonus #2: Word boundary detection
            if is_word_boundary(text_chars, text_idx) {
                score += 30;
            }

            // Bonus #3: Consecutive character bonus
            if let Some(prev_idx) = prev_match_idx {
                if text_idx == prev_idx + 1 {
                    consecutive_matches += 1;
                    score += 15 * consecutive_matches; // Increasing bonus for longer runs
                } else {
                    consecutive_matches = 0;
                }
            }

            // Bonus #4: Position-based scoring (earlier is better)
            let position_bonus = (100 - text_idx as i32).max(0);
            score += position_bonus / 10;

            // Penalty #5: Gap penalty
            if let Some(prev_idx) = prev_match_idx {
                let gap = text_idx - prev_idx - 1;
                if gap > 0 {
                    score -= (gap as i32) * 3; // Penalize gaps between matches
                }
            }

            // Bonus: Case match
            if pattern_chars[pattern_idx] == text_ch {
                score += 5; // Exact case match bonus
            }

            prev_match_idx = Some(text_idx);
            pattern_idx += 1;
        }
    }

    score
}

/// Check if a position in text is at a word boundary
fn is_word_boundary(text_chars: &[char], idx: usize) -> bool {
    if idx == 0 {
        return true;
    }

    let prev_char = text_chars[idx - 1];
    let curr_char = text_chars[idx];

    // Path separators
    if prev_char == '/' || prev_char == '\\' || prev_char == '.' {
        return true;
    }

    // CamelCase/PascalCase boundary (lowercase to uppercase)
    if prev_char.is_lowercase() && curr_char.is_uppercase() {
        return true;
    }

    // Underscore or hyphen boundaries
    if prev_char == '_' || prev_char == '-' {
        return true;
    }

    // Digit to letter boundary
    if prev_char.is_numeric() && curr_char.is_alphabetic() {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_match_exact() {
        let (matches, score) = fuzzy_match_with_score("app", "app");
        assert!(matches);
        assert!(score > 100); // Should have high score for exact match
    }

    #[test]
    fn test_fuzzy_match_case_insensitive() {
        let (matches, _) = fuzzy_match_with_score("app", "APP");
        assert!(matches);

        let (matches2, _) = fuzzy_match_with_score("APP", "app");
        assert!(matches2);
    }

    #[test]
    fn test_fuzzy_match_substring() {
        let (matches, score) = fuzzy_match_with_score("app", "application.tsx");
        assert!(matches);
        assert!(score > 0);
    }

    #[test]
    fn test_fuzzy_match_scattered() {
        let (matches, score) = fuzzy_match_with_score("apc", "application_config.rs");
        assert!(matches);
        assert!(score > 0);
    }

    #[test]
    fn test_fuzzy_match_no_match() {
        let (matches, score) = fuzzy_match_with_score("xyz", "application.tsx");
        assert!(!matches);
        assert_eq!(score, 0);
    }

    #[test]
    fn test_fuzzy_match_empty_pattern() {
        let (matches, score) = fuzzy_match_with_score("", "app.tsx");
        assert!(matches);
        assert_eq!(score, 0);
    }

    #[test]
    fn test_fuzzy_match_empty_text() {
        let (matches, score) = fuzzy_match_with_score("app", "");
        assert!(!matches);
        assert_eq!(score, 0);
    }

    #[test]
    fn test_fuzzy_match_both_empty() {
        let (matches, score) = fuzzy_match_with_score("", "");
        assert!(matches);
        assert_eq!(score, 0);
    }

    #[test]
    fn test_consecutive_bonus() {
        let (matches1, score1) = fuzzy_match_with_score("abc", "abcdef.tsx");
        let (matches2, score2) = fuzzy_match_with_score("abc", "axbxcxdef.tsx");

        assert!(matches1 && matches2);
        // Consecutive characters should score higher than scattered (with 'x' gaps)
        assert!(score1 > score2, "Consecutive matches should score higher than scattered. score1={}, score2={}", score1, score2);
    }

    #[test]
    fn test_start_of_string_bonus() {
        let (_, score_start) = fuzzy_match_with_score("app", "app.tsx");
        let (_, score_middle) = fuzzy_match_with_score("app", "myapp.tsx");

        // Same pattern, but starting at position 0 vs position 2
        assert!(score_start > score_middle, "Matches at start should score higher. score_start={}, score_middle={}", score_start, score_middle);
    }

    #[test]
    fn test_word_boundary_bonus() {
        let (_, score_boundary) = fuzzy_match_with_score("bar", "FooBar.tsx");
        let (_, score_middle) = fuzzy_match_with_score("oba", "FooBar.tsx");

        assert!(score_boundary > score_middle, "Word boundary matches should score higher");
    }

    #[test]
    fn test_gap_penalty() {
        let (_, score_small_gap) = fuzzy_match_with_score("abc", "abc.tsx");
        let (_, score_large_gap) = fuzzy_match_with_score("abc", "a___b___c.tsx");

        assert!(score_small_gap > score_large_gap, "Smaller gaps should score higher");
    }

    #[test]
    fn test_camel_case_matching() {
        let (matches, score) = fuzzy_match_with_score("uc", "UserController.tsx");
        assert!(matches);
        assert!(score > 100, "CamelCase word boundaries should get bonus");
    }

    #[test]
    fn test_position_based_scoring() {
        let (_, score_early) = fuzzy_match_with_score("a", "app.tsx");
        let (_, score_late) = fuzzy_match_with_score("x", "app.tsx");

        assert!(score_early > score_late, "Earlier positions should score higher");
    }

    #[test]
    fn test_special_characters() {
        let (matches, _) = fuzzy_match_with_score("app", "app-config.tsx");
        assert!(matches);

        let (matches2, _) = fuzzy_match_with_score("app", "app_config.tsx");
        assert!(matches2);

        let (matches3, _) = fuzzy_match_with_score("app", "app.config.tsx");
        assert!(matches3);
    }

    #[test]
    fn test_unicode_characters() {
        // Note: Unicode normalization makes "ë" different from "e", so this might not match
        // This test verifies the algorithm doesn't crash with unicode
        let (matches, _) = fuzzy_match_with_score("tst", "tëst.tsx");
        assert!(matches); // Should match 't', 's', 't' ignoring 'ë'

        let (matches2, _) = fuzzy_match_with_score("app", "应用程序app.tsx");
        assert!(matches2); // Should match "app" at the end
    }

    #[test]
    fn test_very_long_strings() {
        let long_text = "a".repeat(1000) + "bc";
        let (matches, _) = fuzzy_match_with_score("abc", &long_text);
        assert!(matches);
    }

    #[test]
    fn test_pattern_longer_than_text() {
        let (matches, score) = fuzzy_match_with_score("application", "app");
        assert!(!matches);
        assert_eq!(score, 0);
    }

    #[test]
    fn test_repeated_characters() {
        let (matches, _) = fuzzy_match_with_score("aaa", "aaa.tsx");
        assert!(matches);

        let (matches2, _) = fuzzy_match_with_score("aa", "banana.tsx");
        assert!(matches2);
    }

    #[test]
    fn test_numbers_in_filename() {
        let (matches, _) = fuzzy_match_with_score("v2", "appV2Controller.tsx");
        assert!(matches);

        let (matches2, _) = fuzzy_match_with_score("123", "test123.tsx");
        assert!(matches2);
    }

    #[test]
    fn test_is_word_boundary_slash() {
        let chars: Vec<char> = "src/app".chars().collect();
        assert!(is_word_boundary(&chars, 4)); // 'a' after '/'
    }

    #[test]
    fn test_is_word_boundary_backslash() {
        let chars: Vec<char> = "src\\app".chars().collect();
        assert!(is_word_boundary(&chars, 4)); // 'a' after '\\'
    }

    #[test]
    fn test_is_word_boundary_dot() {
        let chars: Vec<char> = "app.tsx".chars().collect();
        assert!(is_word_boundary(&chars, 4)); // 't' after '.'
    }

    #[test]
    fn test_is_word_boundary_camel_case() {
        let chars: Vec<char> = "appController".chars().collect();
        assert!(is_word_boundary(&chars, 3)); // 'C' after 'p'
    }

    #[test]
    fn test_is_word_boundary_underscore() {
        let chars: Vec<char> = "app_config".chars().collect();
        assert!(is_word_boundary(&chars, 4)); // 'c' after '_'
    }

    #[test]
    fn test_is_word_boundary_hyphen() {
        let chars: Vec<char> = "app-config".chars().collect();
        assert!(is_word_boundary(&chars, 4)); // 'c' after '-'
    }

    #[test]
    fn test_is_word_boundary_digit_to_letter() {
        let chars: Vec<char> = "test123abc".chars().collect();
        assert!(is_word_boundary(&chars, 7)); // 'a' after '3'
    }

    #[test]
    fn test_is_word_boundary_start() {
        let chars: Vec<char> = "app".chars().collect();
        assert!(is_word_boundary(&chars, 0)); // First character
    }

    #[test]
    fn test_is_word_boundary_lowercase_to_lowercase() {
        let chars: Vec<char> = "application".chars().collect();
        assert!(!is_word_boundary(&chars, 3)); // 'l' after 'p'
    }

    #[test]
    fn test_scoring_comparison_real_world() {
        // Test realistic scenarios
        let files = vec![
            "App.tsx",
            "AppBar.tsx",
            "application.ts",
            "NavigationApp.tsx",
            "helpers/app_utils.ts",
        ];

        let mut results: Vec<(&str, i32)> = files
            .iter()
            .map(|f| {
                let (matches, score) = fuzzy_match_with_score("app", &f.to_lowercase());
                (*f, if matches { score } else { 0 })
            })
            .collect();

        results.sort_by(|a, b| b.1.cmp(&a.1));

        // App.tsx should rank highest (exact match at start)
        assert_eq!(results[0].0, "App.tsx");

        // application.ts should rank high (consecutive at start)
        assert!(results.iter().position(|r| r.0 == "application.ts").unwrap() < 3);
    }

    #[test]
    fn test_edge_case_single_character() {
        let (matches, _) = fuzzy_match_with_score("a", "a");
        assert!(matches);

        let (matches2, _) = fuzzy_match_with_score("a", "b");
        assert!(!matches2);
    }

    #[test]
    fn test_edge_case_all_same_character() {
        let (matches, _) = fuzzy_match_with_score("aaa", "aaaaaaa");
        assert!(matches);
    }

    #[test]
    fn test_edge_case_reverse_order() {
        let (matches, _) = fuzzy_match_with_score("cba", "abc");
        assert!(!matches); // Pattern chars must appear in order
    }

    #[test]
    fn test_whitespace_handling() {
        let (matches, _) = fuzzy_match_with_score("a b", "a b.tsx");
        assert!(matches);

        let (matches2, _) = fuzzy_match_with_score("ab", "a b.tsx");
        assert!(matches2); // Can skip whitespace
    }
}

