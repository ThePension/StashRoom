use tauri::{AppHandle, State};

use crate::core::{diff, discard, fs_watch, history, repo, stage, status};
use crate::types::*;

/// Global application state
pub struct AppState {
    pub repo_registry: repo::RepoRegistry,
    pub watch_manager: fs_watch::WatchManager,
}

// Explicitly implement Send + Sync for AppState
unsafe impl Send for AppState {}
unsafe impl Sync for AppState {}

impl AppState {
    pub fn new() -> Self {
        Self {
            repo_registry: repo::RepoRegistry::new(),
            watch_manager: fs_watch::WatchManager::new(),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Repository Commands
// ============================================================================

#[tauri::command]
pub fn open_repo(
    request: RepoOpenRequest,
    state: State<AppState>,
) -> ApiResponse<RepoOpenResponse> {
    match state.repo_registry.open_repo(&request.path) {
        Ok(response) => ApiResponse::success(response),
        Err(e) => ApiResponse::error("REPO_OPEN_ERROR".to_string(), e.to_string()),
    }
}

#[tauri::command]
pub fn close_repo(repo_id: String, state: State<AppState>) -> ApiResponse<()> {
    match state.repo_registry.close_repo(&repo_id) {
        Ok(_) => ApiResponse::success(()),
        Err(e) => ApiResponse::error("REPO_CLOSE_ERROR".to_string(), e.to_string()),
    }
}

#[tauri::command]
pub fn list_repos(state: State<AppState>) -> ApiResponse<Vec<RepoOpenResponse>> {
    let repos = state.repo_registry.list_repos();
    ApiResponse::success(repos)
}

// ============================================================================
// Status Commands
// ============================================================================

#[tauri::command]
pub fn get_status(repo_id: String, state: State<AppState>) -> ApiResponse<StatusMatrix> {
    let repo = match state.repo_registry.get_repo(&repo_id) {
        Ok(r) => r,
        Err(e) => return ApiResponse::error("REPO_NOT_FOUND".to_string(), e.to_string()),
    };

    match status::get_status(&repo) {
        Ok(status) => ApiResponse::success(status),
        Err(e) => ApiResponse::error("STATUS_ERROR".to_string(), e.to_string()),
    }
}

// ============================================================================
// Diff Commands
// ============================================================================

#[tauri::command]
pub fn get_diff(request: DiffRequest, state: State<AppState>) -> ApiResponse<FileDiff> {
    let repo = match state.repo_registry.get_repo(&request.repo_id) {
        Ok(r) => r,
        Err(e) => return ApiResponse::error("REPO_NOT_FOUND".to_string(), e.to_string()),
    };

    match diff::get_diff(&repo, &request.path, &request.side, request.context_lines) {
        Ok(diff) => ApiResponse::success(diff),
        Err(e) => ApiResponse::error("DIFF_ERROR".to_string(), e.to_string()),
    }
}

// ============================================================================
// Stage Commands
// ============================================================================

#[tauri::command]
pub fn stage_hunk(
    request: StageHunkRequest,
    state: State<AppState>,
) -> ApiResponse<StatusMatrix> {
    let repo = match state.repo_registry.get_repo(&request.repo_id) {
        Ok(r) => r,
        Err(e) => return ApiResponse::error("REPO_NOT_FOUND".to_string(), e.to_string()),
    };

    match stage::stage_hunk(&repo, &request.path, request.hunk_index, request.unstage) {
        Ok(status) => ApiResponse::success(status),
        Err(e) => ApiResponse::error("STAGE_HUNK_ERROR".to_string(), e.to_string()),
    }
}

#[tauri::command]
pub fn stage_lines(
    request: StageLinesRequest,
    state: State<AppState>,
) -> ApiResponse<StatusMatrix> {
    let repo = match state.repo_registry.get_repo(&request.repo_id) {
        Ok(r) => r,
        Err(e) => return ApiResponse::error("REPO_NOT_FOUND".to_string(), e.to_string()),
    };

    match stage::stage_lines(
        &repo,
        &request.path,
        request.hunk_index,
        &request.line_indices,
        request.unstage,
    ) {
        Ok(status) => ApiResponse::success(status),
        Err(e) => ApiResponse::error("STAGE_LINES_ERROR".to_string(), e.to_string()),
    }
}

#[tauri::command]
pub fn stage_file(repo_id: String, path: String, state: State<AppState>) -> ApiResponse<StatusMatrix> {
    let repo = match state.repo_registry.get_repo(&repo_id) {
        Ok(r) => r,
        Err(e) => return ApiResponse::error("REPO_NOT_FOUND".to_string(), e.to_string()),
    };

    match stage::stage_file(&repo, &path) {
        Ok(status) => ApiResponse::success(status),
        Err(e) => ApiResponse::error("STAGE_FILE_ERROR".to_string(), e.to_string()),
    }
}

#[tauri::command]
pub fn unstage_file(
    repo_id: String,
    path: String,
    state: State<AppState>,
) -> ApiResponse<StatusMatrix> {
    let repo = match state.repo_registry.get_repo(&repo_id) {
        Ok(r) => r,
        Err(e) => return ApiResponse::error("REPO_NOT_FOUND".to_string(), e.to_string()),
    };

    match stage::unstage_file(&repo, &path) {
        Ok(status) => ApiResponse::success(status),
        Err(e) => ApiResponse::error("UNSTAGE_FILE_ERROR".to_string(), e.to_string()),
    }
}

#[tauri::command]
pub fn stage_dir(
    repo_id: String,
    dir: String,
    include_untracked: bool,
    state: State<AppState>,
) -> ApiResponse<StatusMatrix> {
    let repo = match state.repo_registry.get_repo(&repo_id) {
        Ok(r) => r,
        Err(e) => return ApiResponse::error("REPO_NOT_FOUND".to_string(), e.to_string()),
    };

    match stage::stage_dir(&repo, &dir, include_untracked) {
        Ok(status) => ApiResponse::success(status),
        Err(e) => ApiResponse::error("STAGE_DIR_ERROR".to_string(), e.to_string()),
    }
}

#[tauri::command]
pub fn unstage_dir(
    repo_id: String,
    dir: String,
    state: State<AppState>,
) -> ApiResponse<StatusMatrix> {
    let repo = match state.repo_registry.get_repo(&repo_id) {
        Ok(r) => r,
        Err(e) => return ApiResponse::error("REPO_NOT_FOUND".to_string(), e.to_string()),
    };

    match stage::unstage_dir(&repo, &dir) {
        Ok(status) => ApiResponse::success(status),
        Err(e) => ApiResponse::error("UNSTAGE_DIR_ERROR".to_string(), e.to_string()),
    }
}

// ============================================================================
// Discard Commands
// ============================================================================

#[tauri::command]
pub fn discard(request: DiscardRequest, state: State<AppState>) -> ApiResponse<discard::DiscardResult> {
    let repo = match state.repo_registry.get_repo(&request.repo_id) {
        Ok(r) => r,
        Err(e) => return ApiResponse::error("REPO_NOT_FOUND".to_string(), e.to_string()),
    };

    let hunks = request.hunks.as_deref();
    match discard::discard(&repo, &request.path, hunks) {
        Ok(result) => ApiResponse::success(result),
        Err(e) => ApiResponse::error("DISCARD_ERROR".to_string(), e.to_string()),
    }
}

// ============================================================================
// Delete Commands
// ============================================================================

#[tauri::command]
pub fn delete_file(
    repo_id: String,
    path: String,
    state: State<AppState>,
) -> ApiResponse<StatusMatrix> {
    let repo = match state.repo_registry.get_repo(&repo_id) {
        Ok(r) => r,
        Err(e) => return ApiResponse::error("REPO_NOT_FOUND".to_string(), e.to_string()),
    };

    let workdir = match repo.workdir() {
        Some(w) => w,
        None => return ApiResponse::error("NO_WORKDIR".to_string(), "Repository has no working directory".to_string()),
    };

    let file_path = workdir.join(&path);

    // Delete the file from filesystem
    match std::fs::remove_file(&file_path) {
        Ok(_) => {},
        Err(e) => return ApiResponse::error("DELETE_ERROR".to_string(), format!("Failed to delete file: {}", e)),
    }

    // Get updated status
    match status::get_status(&repo) {
        Ok(status) => ApiResponse::success(status),
        Err(e) => ApiResponse::error("STATUS_ERROR".to_string(), e.to_string()),
    }
}

// ============================================================================
// Commit Commands
// ============================================================================

#[tauri::command]
pub fn commit(request: CommitRequest, state: State<AppState>) -> ApiResponse<CommitResponse> {
    let repo = match state.repo_registry.get_repo(&request.repo_id) {
        Ok(r) => r,
        Err(e) => return ApiResponse::error("REPO_NOT_FOUND".to_string(), e.to_string()),
    };

    // Get author information
    let sig = match (request.author_name, request.author_email) {
        (Some(name), Some(email)) => {
            match git2::Signature::now(&name, &email) {
                Ok(s) => s,
                Err(e) => return ApiResponse::error("SIGNATURE_ERROR".to_string(), e.to_string()),
            }
        }
        _ => {
            // Try to get signature from git config
            match repo.signature() {
                Ok(s) => s,
                Err(e) => {
                    return ApiResponse::error(
                        "SIGNATURE_ERROR".to_string(),
                        format!("No author info provided and git config not found: {}", e),
                    )
                }
            }
        }
    };

    // Get the current HEAD commit
    let parent_commit = match repo.head() {
        Ok(head) => match head.peel_to_commit() {
            Ok(c) => Some(c),
            Err(_) => None,
        },
        Err(_) => None,
    };

    // Get the index and write it to a tree
    let mut index = match repo.index() {
        Ok(i) => i,
        Err(e) => return ApiResponse::error("INDEX_ERROR".to_string(), e.to_string()),
    };

    let tree_id = match index.write_tree() {
        Ok(t) => t,
        Err(e) => return ApiResponse::error("TREE_WRITE_ERROR".to_string(), e.to_string()),
    };

    let tree = match repo.find_tree(tree_id) {
        Ok(t) => t,
        Err(e) => return ApiResponse::error("TREE_FIND_ERROR".to_string(), e.to_string()),
    };

    // Create the commit
    let parents: Vec<&git2::Commit> = parent_commit.iter().collect();
    let oid = match repo.commit(Some("HEAD"), &sig, &sig, &request.message, &tree, &parents) {
        Ok(o) => o,
        Err(e) => return ApiResponse::error("COMMIT_ERROR".to_string(), e.to_string()),
    };

    ApiResponse::success(CommitResponse {
        oid: oid.to_string(),
    })
}

// ============================================================================
// Watch Commands
// ============================================================================

#[tauri::command]
pub fn subscribe_watch(
    repo_id: String,
    state: State<AppState>,
    app_handle: AppHandle,
) -> ApiResponse<()> {
    let repo = match state.repo_registry.get_repo(&repo_id) {
        Ok(r) => r,
        Err(e) => return ApiResponse::error("REPO_NOT_FOUND".to_string(), e.to_string()),
    };

    let repo_path = match repo.workdir() {
        Some(p) => p.to_path_buf(),
        None => return ApiResponse::error("NO_WORKDIR".to_string(), "Repository has no working directory".to_string()),
    };

    match state
        .watch_manager
        .watch_repo(repo_id, repo_path, app_handle)
    {
        Ok(_) => ApiResponse::success(()),
        Err(e) => ApiResponse::error("WATCH_ERROR".to_string(), e.to_string()),
    }
}

#[tauri::command]
pub fn unsubscribe_watch(repo_id: String, state: State<AppState>) -> ApiResponse<()> {
    match state.watch_manager.unwatch_repo(&repo_id) {
        Ok(_) => ApiResponse::success(()),
        Err(e) => ApiResponse::error("UNWATCH_ERROR".to_string(), e.to_string()),
    }
}

// ============================================================================
// Branch Commands
// ============================================================================

#[tauri::command]
pub fn get_head_info(repo_id: String, state: State<AppState>) -> ApiResponse<HeadInfo> {
    let repo = match state.repo_registry.get_repo(&repo_id) {
        Ok(r) => r,
        Err(e) => return ApiResponse::error("REPO_NOT_FOUND".to_string(), e.to_string()),
    };

    // Get HEAD reference
    let head = match repo.head() {
        Ok(h) => h,
        Err(e) => return ApiResponse::error("HEAD_ERROR".to_string(), e.to_string()),
    };

    // Get branch name if on a branch
    let branch = if head.is_branch() {
        head.shorthand().map(|s| s.to_string())
    } else {
        None
    };

    // Get commit OID
    let commit_oid = match head.peel_to_commit() {
        Ok(c) => c.id().to_string(),
        Err(e) => return ApiResponse::error("COMMIT_ERROR".to_string(), e.to_string()),
    };

    // Get commit message
    let message = match head.peel_to_commit() {
        Ok(c) => c.message().map(|m| m.to_string()),
        Err(_) => None,
    };

    ApiResponse::success(HeadInfo {
        branch,
        commit: commit_oid,
        message,
    })
}

#[tauri::command]
pub fn list_branches(
    repo_id: String,
    state: State<AppState>,
) -> ApiResponse<ListBranchesResponse> {
    let repo = match state.repo_registry.get_repo(&repo_id) {
        Ok(r) => r,
        Err(e) => return ApiResponse::error("REPO_NOT_FOUND".to_string(), e.to_string()),
    };

    // Get all local branches
    let branches = match repo.branches(Some(git2::BranchType::Local)) {
        Ok(b) => b,
        Err(e) => return ApiResponse::error("BRANCH_LIST_ERROR".to_string(), e.to_string()),
    };

    let mut locals = Vec::new();
    let mut current: Option<String> = None;

    for branch in branches {
        let (branch, _) = match branch {
            Ok(b) => b,
            Err(e) => return ApiResponse::error("BRANCH_ITER_ERROR".to_string(), e.to_string()),
        };

        let name = match branch.name() {
            Ok(Some(n)) => n.to_string(),
            Ok(None) => continue, // Skip branches with invalid UTF-8 names
            Err(e) => return ApiResponse::error("BRANCH_NAME_ERROR".to_string(), e.to_string()),
        };

        let is_head = match branch.is_head() {
            true => {
                current = Some(name.clone());
                true
            }
            false => false,
        };

        locals.push(BranchInfo { name, is_head });
    }

    ApiResponse::success(ListBranchesResponse { current, locals })
}

#[tauri::command]
pub fn switch_branch(
    request: SwitchBranchRequest,
    state: State<AppState>,
) -> ApiResponse<SwitchBranchResponse> {
    let repo = match state.repo_registry.get_repo(&request.repo_id) {
        Ok(r) => r,
        Err(e) => return ApiResponse::error("REPO_NOT_FOUND".to_string(), e.to_string()),
    };

    // Check if there are uncommitted changes
    match status::get_status(&repo) {
        Ok(status) => {
            if !status.entries.is_empty() {
                return ApiResponse::error(
                    "UNCOMMITTED_CHANGES".to_string(),
                    "You have uncommitted changes. Commit or stash before switching.".to_string(),
                );
            }
        }
        Err(e) => return ApiResponse::error("STATUS_ERROR".to_string(), e.to_string()),
    }

    // Find the branch
    let branch = match repo.find_branch(&request.name, git2::BranchType::Local) {
        Ok(b) => b,
        Err(e) => return ApiResponse::error("BRANCH_NOT_FOUND".to_string(), e.to_string()),
    };

    // Get the reference
    let reference = match branch.get() {
        r => r,
    };

    // Get the commit that the branch points to
    let commit = match reference.peel_to_commit() {
        Ok(c) => c,
        Err(e) => return ApiResponse::error("COMMIT_NOT_FOUND".to_string(), e.to_string()),
    };

    // Checkout the commit
    match repo.checkout_tree(commit.as_object(), Some(git2::build::CheckoutBuilder::new().safe())) {
        Ok(_) => {},
        Err(e) => return ApiResponse::error("CHECKOUT_ERROR".to_string(), e.to_string()),
    };

    // Set HEAD to the branch
    match repo.set_head(&format!("refs/heads/{}", request.name)) {
        Ok(_) => {},
        Err(e) => return ApiResponse::error("SET_HEAD_ERROR".to_string(), e.to_string()),
    };

    ApiResponse::success(SwitchBranchResponse {
        name: request.name,
    })
}

// ============================================================================
// History Commands
// ============================================================================

#[tauri::command]
pub fn get_log(request: GetLogRequest, state: State<AppState>) -> ApiResponse<GetLogResponse> {
    let repo = match state.repo_registry.get_repo(&request.repo_id) {
        Ok(r) => r,
        Err(e) => return ApiResponse::error("REPO_NOT_FOUND".to_string(), e.to_string()),
    };

    match history::get_log(&repo, request.limit, request.skip) {
        Ok(response) => ApiResponse::success(response),
        Err(e) => ApiResponse::error("GET_LOG_ERROR".to_string(), e.to_string()),
    }
}

#[tauri::command]
pub async fn get_commit_diff(
    request: GetCommitDiffRequest,
    state: State<'_, AppState>,
) -> Result<ApiResponse<GetCommitDiffResponse>, String> {
    let repo_id = request.repo_id.clone();
    let oid = request.oid.clone();
    let parent = request.parent;

    // Get the repository path (not the repo itself, since it's not Send)
    let repo_path = {
        let repo = match state.repo_registry.get_repo(&repo_id) {
            Ok(r) => r,
            Err(e) => return Ok(ApiResponse::error("REPO_NOT_FOUND".to_string(), e.to_string())),
        };

        match repo.path().parent() {
            Some(p) => p.to_path_buf(),
            None => return Ok(ApiResponse::error("INVALID_REPO_PATH".to_string(), "Repository has no parent path".to_string())),
        }
    };

    // Run the blocking git operation on a dedicated thread pool
    // We reopen the repository in the blocking task since Repository is not Send
    let result = tokio::task::spawn_blocking(move || {
        // Reopen the repository in this thread
        let repo = match git2::Repository::open(&repo_path) {
            Ok(r) => r,
            Err(e) => return ApiResponse::error("REPO_OPEN_ERROR".to_string(), e.to_string()),
        };

        match history::get_commit_diff(&repo, &oid, parent) {
            Ok(response) => ApiResponse::success(response),
            Err(e) => ApiResponse::error("GET_COMMIT_DIFF_ERROR".to_string(), e.to_string()),
        }
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?;

    Ok(result)
}

#[tauri::command]
pub async fn search_files(
    request: SearchFilesRequest,
    state: State<'_, AppState>,
) -> Result<ApiResponse<SearchFilesResponse>, String> {
    let repo_id = request.repo_id.clone();
    let query = request.query.clone();
    let limit = request.limit;

    // Get the repository path
    let repo_path = {
        let repo = match state.repo_registry.get_repo(&repo_id) {
            Ok(r) => r,
            Err(e) => return Ok(ApiResponse::error("REPO_NOT_FOUND".to_string(), e.to_string())),
        };

        match repo.path().parent() {
            Some(p) => p.to_path_buf(),
            None => {
                return Ok(ApiResponse::error(
                    "INVALID_REPO_PATH".to_string(),
                    "Repository has no parent path".to_string(),
                ))
            }
        }
    };

    // Run the blocking git operation on a dedicated thread pool
    let result = tokio::task::spawn_blocking(move || {
        // Reopen the repository in this thread
        let repo = match git2::Repository::open(&repo_path) {
            Ok(r) => r,
            Err(e) => return ApiResponse::error("REPO_OPEN_ERROR".to_string(), e.to_string()),
        };

        match history::search_files(&repo, &query, limit) {
            Ok(response) => ApiResponse::success(response),
            Err(e) => ApiResponse::error("SEARCH_FILES_ERROR".to_string(), e.to_string()),
        }
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?;

    Ok(result)
}

// ============================================================================
// Backup Commands (bonus utilities)
// ============================================================================

#[tauri::command]
pub fn list_backups(repo_id: String, state: State<AppState>) -> ApiResponse<Vec<String>> {
    let repo = match state.repo_registry.get_repo(&repo_id) {
        Ok(r) => r,
        Err(e) => return ApiResponse::error("REPO_NOT_FOUND".to_string(), e.to_string()),
    };

    match discard::list_backups(&repo) {
        Ok(backups) => ApiResponse::success(backups),
        Err(e) => ApiResponse::error("BACKUP_LIST_ERROR".to_string(), e.to_string()),
    }
}

#[tauri::command]
pub fn restore_from_backup(
    repo_id: String,
    backup_id: String,
    path: String,
    state: State<AppState>,
) -> ApiResponse<StatusMatrix> {
    let repo = match state.repo_registry.get_repo(&repo_id) {
        Ok(r) => r,
        Err(e) => return ApiResponse::error("REPO_NOT_FOUND".to_string(), e.to_string()),
    };

    match discard::restore_from_backup(&repo, &backup_id, &path) {
        Ok(status) => ApiResponse::success(status),
        Err(e) => ApiResponse::error("RESTORE_ERROR".to_string(), e.to_string()),
    }
}

// ============================================================================
// Persistence Commands
// ============================================================================

#[tauri::command]
pub fn validate_repo_paths(paths: Vec<String>) -> ApiResponse<Vec<ValidatedRepo>> {
    let validated: Vec<ValidatedRepo> = paths
        .into_iter()
        .map(|path| {
            let path_buf = std::path::PathBuf::from(&path);
            let exists = path_buf.exists();
            let is_git_repo = exists && path_buf.join(".git").exists();

            ValidatedRepo {
                path,
                exists,
                is_git_repo,
            }
        })
        .collect();

    ApiResponse::success(validated)
}

#[tauri::command]
pub fn list_backup_files(repo_id: String, backup_id: String, state: State<AppState>) -> ApiResponse<Vec<String>> {
    let repo = match state.repo_registry.get_repo(&repo_id) {
        Ok(r) => r,
        Err(e) => return ApiResponse::error("REPO_NOT_FOUND".to_string(), e.to_string()),
    };

    match discard::list_backup_files(&repo, &backup_id) {
        Ok(files) => ApiResponse::success(files),
        Err(e) => ApiResponse::error("BACKUP_FILES_ERROR".to_string(), e.to_string()),
    }
}

#[tauri::command]
pub fn restore_many(
    repo_id: String,
    backup_id: String,
    paths: Vec<String>,
    state: State<AppState>,
) -> ApiResponse<StatusMatrix> {
    let repo = match state.repo_registry.get_repo(&repo_id) {
        Ok(r) => r,
        Err(e) => return ApiResponse::error("REPO_NOT_FOUND".to_string(), e.to_string()),
    };

    match discard::restore_many(&repo, &backup_id, &paths) {
        Ok(status) => ApiResponse::success(status),
        Err(e) => ApiResponse::error("RESTORE_MANY_ERROR".to_string(), e.to_string()),
    }
}

#[tauri::command]
pub fn clear_backups(repo_id: String, state: State<AppState>) -> ApiResponse<()> {
    let repo = match state.repo_registry.get_repo(&repo_id) {
        Ok(r) => r,
        Err(e) => return ApiResponse::error("REPO_NOT_FOUND".to_string(), e.to_string()),
    };

    match discard::clear_backups(&repo) {
        Ok(_) => ApiResponse::success(()),
        Err(e) => ApiResponse::error("CLEAR_BACKUPS_ERROR".to_string(), e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Repository;
    use tempfile::TempDir;

    fn create_test_repo() -> (TempDir, String) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_string_lossy().to_string();

        let repo = Repository::init(&repo_path).unwrap();
        let sig = git2::Signature::now("Test User", "test@example.com").unwrap();

        // Create initial commit
        let tree_id = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
            .unwrap();

        (temp_dir, repo_path)
    }

    #[test]
    fn test_validate_repo_paths_all_valid() {
        let (_temp_dir1, path1) = create_test_repo();
        let (_temp_dir2, path2) = create_test_repo();

        let paths = vec![path1.clone(), path2.clone()];
        let response = validate_repo_paths(paths);

        assert!(response.ok);
        let validated = response.data.unwrap();
        assert_eq!(validated.len(), 2);

        assert_eq!(validated[0].path, path1);
        assert!(validated[0].exists);
        assert!(validated[0].is_git_repo);

        assert_eq!(validated[1].path, path2);
        assert!(validated[1].exists);
        assert!(validated[1].is_git_repo);
    }

    #[test]
    fn test_validate_repo_paths_missing_paths() {
        let response = validate_repo_paths(vec![
            "/path/that/does/not/exist".to_string(),
            "/another/missing/path".to_string(),
        ]);

        assert!(response.ok);
        let validated = response.data.unwrap();
        assert_eq!(validated.len(), 2);

        assert_eq!(validated[0].path, "/path/that/does/not/exist");
        assert!(!validated[0].exists);
        assert!(!validated[0].is_git_repo);

        assert_eq!(validated[1].path, "/another/missing/path");
        assert!(!validated[1].exists);
        assert!(!validated[1].is_git_repo);
    }

    #[test]
    fn test_validate_repo_paths_mixed_valid_and_invalid() {
        let (_temp_dir, valid_path) = create_test_repo();

        let paths = vec![
            valid_path.clone(),
            "/missing/path".to_string(),
        ];
        let response = validate_repo_paths(paths);

        assert!(response.ok);
        let validated = response.data.unwrap();
        assert_eq!(validated.len(), 2);

        // First one should be valid
        assert_eq!(validated[0].path, valid_path);
        assert!(validated[0].exists);
        assert!(validated[0].is_git_repo);

        // Second one should be invalid
        assert_eq!(validated[1].path, "/missing/path");
        assert!(!validated[1].exists);
        assert!(!validated[1].is_git_repo);
    }

    #[test]
    fn test_validate_repo_paths_non_git_directory() {
        // Create a regular directory (not a git repo)
        let temp_dir = TempDir::new().unwrap();
        let non_git_path = temp_dir.path().to_string_lossy().to_string();

        let response = validate_repo_paths(vec![non_git_path.clone()]);

        assert!(response.ok);
        let validated = response.data.unwrap();
        assert_eq!(validated.len(), 1);

        // Directory exists but is not a git repo
        assert_eq!(validated[0].path, non_git_path);
        assert!(validated[0].exists);
        assert!(!validated[0].is_git_repo);
    }

    #[test]
    fn test_validate_repo_paths_empty_list() {
        let response = validate_repo_paths(vec![]);

        assert!(response.ok);
        let validated = response.data.unwrap();
        assert_eq!(validated.len(), 0);
    }

    #[test]
    fn test_validate_repo_paths_with_duplicates() {
        let (_temp_dir, path) = create_test_repo();

        // Pass the same path multiple times
        let paths = vec![path.clone(), path.clone(), path.clone()];
        let response = validate_repo_paths(paths);

        assert!(response.ok);
        let validated = response.data.unwrap();
        assert_eq!(validated.len(), 3);

        // All three should be valid and identical
        for v in &validated {
            assert_eq!(v.path, path);
            assert!(v.exists);
            assert!(v.is_git_repo);
        }
    }

    #[test]
    fn test_validate_repo_paths_preserves_order() {
        let (_temp_dir1, path1) = create_test_repo();
        let (_temp_dir2, path2) = create_test_repo();
        let missing_path = "/missing/path".to_string();

        let paths = vec![path1.clone(), missing_path.clone(), path2.clone()];
        let response = validate_repo_paths(paths);

        assert!(response.ok);
        let validated = response.data.unwrap();
        assert_eq!(validated.len(), 3);

        // Order should be preserved
        assert_eq!(validated[0].path, path1);
        assert_eq!(validated[1].path, missing_path);
        assert_eq!(validated[2].path, path2);
    }

    #[test]
    fn test_validate_repo_paths_handles_special_characters() {
        // Test with paths containing spaces and special characters
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Create a directory with spaces (but not a git repo)
        let special_dir = base_path.join("my repo with spaces");
        std::fs::create_dir(&special_dir).unwrap();

        let special_path = special_dir.to_string_lossy().to_string();
        let response = validate_repo_paths(vec![special_path.clone()]);

        assert!(response.ok);
        let validated = response.data.unwrap();
        assert_eq!(validated.len(), 1);
        assert_eq!(validated[0].path, special_path);
        assert!(validated[0].exists);
        assert!(!validated[0].is_git_repo); // Directory exists but is not a git repo
    }
}
