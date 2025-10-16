import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type {
  ApiResponse,
  RepoOpenRequest,
  RepoOpenResponse,
  StatusMatrix,
  DiffRequest,
  FileDiff,
  StageHunkRequest,
  StageLinesRequest,
  DiscardRequest,
  CommitRequest,
  CommitResponse,
  WatchEvent,
  GetLogRequest,
  GetLogResponse,
  GetCommitDiffRequest,
  GetCommitDiffResponse,
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
  async discard(request: DiscardRequest): Promise<ApiResponse<StatusMatrix>> {
    return invoke<ApiResponse<StatusMatrix>>('discard', { request });
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
}

export const api = new GitAPI();
