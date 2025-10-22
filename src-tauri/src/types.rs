use serde::{Deserialize, Serialize};

// ============================================================================
// Repository Operations
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoOpenRequest {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoOpenResponse {
    pub repo_id: String, // UUID
    pub path: String,
    pub head: Option<HeadInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadInfo {
    pub branch: Option<String>,
    pub commit: String, // OID
    pub message: Option<String>,
}

// ============================================================================
// Status Operations
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusMatrix {
    pub entries: Vec<StatusEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusEntry {
    pub path: String,
    pub status: FileStatus,
    pub staged_status: Option<String>,   // "added" | "modified" | "deleted"
    pub unstaged_status: Option<String>, // "modified" | "deleted"
    pub untracked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FileStatus {
    Untracked,
    Modified,
    Added,
    Deleted,
    Renamed,
    Conflicted,
}

// ============================================================================
// Diff Operations
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffRequest {
    pub repo_id: String,
    pub path: String,
    pub side: DiffSide,
    #[serde(default = "default_context_lines")]
    pub context_lines: Option<u32>, // None means show whole file
}

fn default_context_lines() -> Option<u32> {
    Some(3)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffSide {
    Working,
    Index,
    Head,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub path: String,
    pub old_path: Option<String>,
    pub is_binary: bool,
    pub is_new: bool,
    pub is_deleted: bool,
    pub hunks: Vec<DiffHunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunk {
    pub header: String,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub origin: String, // "+" | "-" | " "
    pub content: String,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
}

// ============================================================================
// Stage Operations
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageHunkRequest {
    pub repo_id: String,
    pub path: String,
    pub hunk_index: usize,
    pub unstage: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageLinesRequest {
    pub repo_id: String,
    pub path: String,
    pub hunk_index: usize,
    pub line_indices: Vec<usize>, // Indices within the hunk's lines array
    pub unstage: bool,
}

// ============================================================================
// Discard Operations
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscardRequest {
    pub repo_id: String,
    pub path: String,
    pub hunks: Option<Vec<usize>>, // If None, discard entire file
}

// ============================================================================
// Commit Operations
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitRequest {
    pub repo_id: String,
    pub message: String,
    pub author_name: Option<String>,
    pub author_email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitResponse {
    pub oid: String,
}

// ============================================================================
// Watch Operations
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchEvent {
    pub repo_id: String,
    pub paths: Vec<String>,
    pub event_type: String, // "modified" | "created" | "deleted"
}

// ============================================================================
// Response Envelope
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse<T> {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            ok: true,
            data: Some(data),
            code: None,
            message: None,
        }
    }

    pub fn error(code: String, message: String) -> Self {
        Self {
            ok: false,
            data: None,
            code: Some(code),
            message: Some(message),
        }
    }
}

// ============================================================================
// History Operations
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetLogRequest {
    pub repo_id: String,
    pub limit: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetLogResponse {
    pub commits: Vec<CommitSummary>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitSummary {
    pub oid: String,
    pub short_oid: String,
    pub author: CommitAuthor,
    pub time: i64, // epoch seconds
    pub message: String,
    pub subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refs: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitAuthor {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetCommitDiffRequest {
    pub repo_id: String,
    pub oid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetCommitDiffResponse {
    pub files: Vec<CommitFileDiff>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommitFileChangeType {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitFileDiff {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_path: Option<String>,
    pub change: CommitFileChangeType,
    pub is_binary: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hunks: Option<Vec<CommitDiffHunk>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDiffHunk {
    pub header: String,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<CommitDiffLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDiffLine {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ln_old: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ln_new: Option<u32>,
    #[serde(rename = "type")]
    pub line_type: String, // "add" | "del" | "ctx"
    pub text: String,
}

// ============================================================================
// Branch Operations
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    pub name: String,
    pub is_head: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListBranchesResponse {
    pub current: Option<String>,
    pub locals: Vec<BranchInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchBranchRequest {
    pub repo_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchBranchResponse {
    pub name: String,
}

// ============================================================================
// Search Operations
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchFilesRequest {
    pub repo_id: String,
    pub query: String,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
}

fn default_search_limit() -> usize {
    100
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchFilesResponse {
    pub results: Vec<FileSearchResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSearchResult {
    pub path: String,
    pub commits: Vec<CommitFileMatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitFileMatch {
    pub commit: CommitSummary,
    pub change: CommitFileChangeType,
}

// ============================================================================
// Internal Cache Types
// ============================================================================

// Note: DiffCacheKey is prepared for future caching implementation
#[allow(dead_code)]
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct DiffCacheKey {
    pub path: String,
    pub index_oid: Option<String>,
    pub worktree_mtime: Option<i64>,
}
