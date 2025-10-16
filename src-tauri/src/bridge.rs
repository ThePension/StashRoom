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

    match diff::get_diff(&repo, &request.path, &request.side) {
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

// ============================================================================
// Discard Commands
// ============================================================================

#[tauri::command]
pub fn discard(request: DiscardRequest, state: State<AppState>) -> ApiResponse<StatusMatrix> {
    let repo = match state.repo_registry.get_repo(&request.repo_id) {
        Ok(r) => r,
        Err(e) => return ApiResponse::error("REPO_NOT_FOUND".to_string(), e.to_string()),
    };

    let hunks = request.hunks.as_deref();
    match discard::discard(&repo, &request.path, hunks) {
        Ok(status) => ApiResponse::success(status),
        Err(e) => ApiResponse::error("DISCARD_ERROR".to_string(), e.to_string()),
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
pub fn get_commit_diff(
    request: GetCommitDiffRequest,
    state: State<AppState>,
) -> ApiResponse<GetCommitDiffResponse> {
    let repo = match state.repo_registry.get_repo(&request.repo_id) {
        Ok(r) => r,
        Err(e) => return ApiResponse::error("REPO_NOT_FOUND".to_string(), e.to_string()),
    };

    match history::get_commit_diff(&repo, &request.oid, request.parent) {
        Ok(response) => ApiResponse::success(response),
        Err(e) => ApiResponse::error("GET_COMMIT_DIFF_ERROR".to_string(), e.to_string()),
    }
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
