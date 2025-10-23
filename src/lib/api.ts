import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type {
  ApiResponse,
  RepoOpenRequest,
  RepoOpenResponse,
  HeadInfo,
  StatusMatrix,
  DiffRequest,
  FileDiff,
  StageHunkRequest,
  StageLinesRequest,
  DiscardRequest,
  DiscardResult,
  CommitRequest,
  CommitResponse,
  WatchEvent,
  GetLogRequest,
  GetLogResponse,
  GetCommitDiffRequest,
  GetCommitDiffResponse,
  ListBranchesResponse,
  SwitchBranchRequest,
  SwitchBranchResponse,
  SearchFilesRequest,
  SearchFilesResponse,
  ValidatedRepo,
} from './types';

class GitAPI {
  /**
   * Opens a repository and returns its UUID identifier
   */
  async openRepo(request: RepoOpenRequest): Promise<ApiResponse<RepoOpenResponse>> {
    return invoke<ApiResponse<RepoOpenResponse>>('open_repo', { request });
  }

  /**
   * Closes a repository by its UUID
   */
  async closeRepo(repoId: string): Promise<ApiResponse<void>> {
    return invoke<ApiResponse<void>>('close_repo', { repoId });
  }

  /**
   * Lists all currently open repositories
   */
  async listRepos(): Promise<ApiResponse<RepoOpenResponse[]>> {
    return invoke<ApiResponse<RepoOpenResponse[]>>('list_repos');
  }

  /**
   * Gets the status of all files in the repository
   */
  async getStatus(repoId: string): Promise<ApiResponse<StatusMatrix>> {
    return invoke<ApiResponse<StatusMatrix>>('get_status', { repoId });
  }

  /**
   * Gets the diff for a specific file
   */
  async getDiff(request: DiffRequest): Promise<ApiResponse<FileDiff>> {
    return invoke<ApiResponse<FileDiff>>('get_diff', { request });
  }

  /**
   * Stages a specific hunk
   */
  async stageHunk(request: StageHunkRequest): Promise<ApiResponse<StatusMatrix>> {
    return invoke<ApiResponse<StatusMatrix>>('stage_hunk', { request });
  }

  /**
   * Stages specific lines from a hunk
   */
  async stageLines(request: StageLinesRequest): Promise<ApiResponse<StatusMatrix>> {
    return invoke<ApiResponse<StatusMatrix>>('stage_lines', { request });
  }

  /**
   * Stages an entire file
   */
  async stageFile(repoId: string, path: string): Promise<ApiResponse<StatusMatrix>> {
    return invoke<ApiResponse<StatusMatrix>>('stage_file', { repoId, path });
  }

  /**
   * Unstages an entire file
   */
  async unstageFile(repoId: string, path: string): Promise<ApiResponse<StatusMatrix>> {
    return invoke<ApiResponse<StatusMatrix>>('unstage_file', { repoId, path });
  }

  /**
   * Discards changes to a file or specific hunks
   */
  async discard(request: DiscardRequest): Promise<ApiResponse<DiscardResult>> {
    return invoke<ApiResponse<DiscardResult>>('discard', { request });
  }

  /**
   * Deletes a file from the filesystem
   */
  async deleteFile(repoId: string, path: string): Promise<ApiResponse<StatusMatrix>> {
    return invoke<ApiResponse<StatusMatrix>>('delete_file', { repoId, path });
  }

  /**
   * Creates a commit with staged changes
   */
  async commit(request: CommitRequest): Promise<ApiResponse<CommitResponse>> {
    return invoke<ApiResponse<CommitResponse>>('commit', { request });
  }

  /**
   * Subscribes to file system watch events for a repository
   */
  async subscribeWatch(repoId: string): Promise<ApiResponse<void>> {
    return invoke<ApiResponse<void>>('subscribe_watch', { repoId, appHandle: null });
  }

  /**
   * Unsubscribes from file system watch events
   */
  async unsubscribeWatch(repoId: string): Promise<ApiResponse<void>> {
    return invoke<ApiResponse<void>>('unsubscribe_watch', { repoId });
  }

  /**
   * Lists available backups for recovery
   */
  async listBackups(repoId: string): Promise<ApiResponse<string[]>> {
    return invoke<ApiResponse<string[]>>('list_backups', { repoId });
  }

  /**
   * Restores a file from a backup
   */
  async restoreFromBackup(
    repoId: string,
    backupId: string,
    path: string
  ): Promise<ApiResponse<StatusMatrix>> {
    return invoke<ApiResponse<StatusMatrix>>('restore_from_backup', {
      repoId,
      backupId,
      path,
    });
  }

  /**
   * Lists all files in a specific backup
   */
  async listBackupFiles(repoId: string, backupId: string): Promise<ApiResponse<string[]>> {
    return invoke<ApiResponse<string[]>>('list_backup_files', { repoId, backupId });
  }

  /**
   * Restores multiple files from a backup
   */
  async restoreMany(repoId: string, backupId: string, paths: string[]): Promise<ApiResponse<StatusMatrix>> {
    return invoke<ApiResponse<StatusMatrix>>('restore_many', { repoId, backupId, paths });
  }

  /**
   * Clears all backups for a repository
   */
  async clearBackups(repoId: string): Promise<ApiResponse<void>> {
    return invoke<ApiResponse<void>>('clear_backups', { repoId });
  }

  /**
   * Listens for watch events from the backend
   */
  onWatchEvent(callback: (event: WatchEvent) => void): Promise<() => void> {
    return listen<WatchEvent>('watch-event', (event) => {
      callback(event.payload);
    });
  }

  /**
   * Gets commit history for the current branch
   */
  async getLog(request: GetLogRequest): Promise<ApiResponse<GetLogResponse>> {
    return invoke<ApiResponse<GetLogResponse>>('get_log', { request });
  }

  /**
   * Gets the diff for a specific commit
   */
  async getCommitDiff(request: GetCommitDiffRequest): Promise<ApiResponse<GetCommitDiffResponse>> {
    return invoke<ApiResponse<GetCommitDiffResponse>>('get_commit_diff', { request });
  }

  /**
   * Gets the current HEAD information (branch and commit)
   */
  async getHeadInfo(repoId: string): Promise<ApiResponse<HeadInfo>> {
    return invoke<ApiResponse<HeadInfo>>('get_head_info', { repoId });
  }

  /**
   * Lists all local branches
   */
  async listBranches(repoId: string): Promise<ApiResponse<ListBranchesResponse>> {
    return invoke<ApiResponse<ListBranchesResponse>>('list_branches', { repoId });
  }

  /**
   * Switches to a different branch
   */
  async switchBranch(request: SwitchBranchRequest): Promise<ApiResponse<SwitchBranchResponse>> {
    return invoke<ApiResponse<SwitchBranchResponse>>('switch_branch', { request });
  }

  /**
   * Searches for files across commit history
   */
  async searchFiles(request: SearchFilesRequest): Promise<ApiResponse<SearchFilesResponse>> {
    return invoke<ApiResponse<SearchFilesResponse>>('search_files', { request });
  }

  /**
   * Validates that repository paths exist and are valid git repos
   */
  async validateRepoPaths(paths: string[]): Promise<ApiResponse<ValidatedRepo[]>> {
    return invoke<ApiResponse<ValidatedRepo[]>>('validate_repo_paths', { paths });
  }
}

export const api = new GitAPI();
