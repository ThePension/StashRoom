import { create } from 'zustand';
import { api } from '../lib/api';
import { toast } from 'sonner';
import type {
  RepoOpenResponse,
  StatusEntry,
  FileDiff,
  StatusMatrix,
  CommitSummary,
  CommitFileDiff,
} from '../lib/types';

interface RepoState {
  repo: RepoOpenResponse | null;
  isLoading: boolean;
  error: string | null;
  openRepo: (path: string) => Promise<void>;
  closeRepo: () => Promise<void>;
  updateHeadInfo: (repoId: string) => Promise<void>;
}

interface StatusState {
  entries: StatusEntry[];
  isLoading: boolean;
  refreshStatus: (repoId: string, silent?: boolean) => Promise<void>;
  updateStatus: (status: StatusMatrix) => void;
  getStagedEntries: () => StatusEntry[];
  getUnstagedEntries: () => StatusEntry[];
}

interface DiffState {
  currentDiff: FileDiff | null;
  currentDiffSide: 'working' | 'index' | 'head' | null;
  isLoading: boolean;
  loadDiff: (repoId: string, path: string, side: 'working' | 'index' | 'head') => Promise<void>;
  clearDiff: () => void;
}

interface SelectionState {
  selectedPath: string | null;
  selectedHunkIndex: number | null;
  selectedLineIndices: number[];
  setSelectedPath: (path: string | null) => void;
  setSelectedHunk: (index: number | null) => void;
  setSelectedLines: (indices: number[]) => void;
  clearSelection: () => void;
}

interface UIState {
  isOperating: boolean;
  setIsOperating: (value: boolean) => void;
}

interface Settings {
  showLineNumbers: boolean;
  theme: 'light' | 'dark' | 'system';
}

interface SettingsState {
  settings: Settings;
  updateSettings: (settings: Partial<Settings>) => void;
}

interface HistoryState {
  commits: CommitSummary[];
  hasMore: boolean;
  isLoading: boolean;
  isLoadingCommitDiff: boolean;
  selectedCommit: CommitSummary | null;
  commitDiff: CommitFileDiff[] | null;
  selectedCommitFile: CommitFileDiff | null;
  selectedParent: number;
  loadHistory: (repoId: string, limit?: number, skip?: number) => Promise<void>;
  loadMore: (repoId: string) => Promise<void>;
  selectCommit: (repoId: string, commit: CommitSummary, parent?: number) => Promise<void>;
  selectCommitFile: (file: CommitFileDiff | null) => void;
  setSelectedParent: (parent: number) => Promise<void>;
  clearHistory: () => void;
}

export interface AppStore
  extends RepoState,
    StatusState,
    DiffState,
    SelectionState,
    UIState,
    SettingsState,
    HistoryState {}

export const useStore = create<AppStore>((set, get) => ({
  // Repo state
  repo: null,
  isLoading: false,
  error: null,

  openRepo: async (path: string) => {
    set({ isLoading: true, error: null });
    try {
      const response = await api.openRepo({ path });
      if (response.ok && response.data) {
        set({ repo: response.data, isLoading: false });
        // Automatically load status
        await get().refreshStatus(response.data.repoId);
        // Subscribe to watch events
        await api.subscribeWatch(response.data.repoId);
        // Listen for watch events
        api.onWatchEvent(async (event) => {
          if (event.repoId === get().repo?.repoId) {
            // Refresh status silently (no loading indicator) to avoid flicker
            await get().refreshStatus(event.repoId, true);

            // If a file is currently selected, reload its diff to keep it in sync
            const state = get();
            if (state.selectedPath && state.currentDiffSide) {
              // Check if the selected file still exists in the new status
              const fileStillExists = state.entries.some(e => e.path === state.selectedPath);

              if (fileStillExists) {
                // Silently reload the diff for the currently selected file
                const response = await api.getDiff({
                  repoId: event.repoId,
                  path: state.selectedPath,
                  side: state.currentDiffSide,
                });

                if (response.ok && response.data) {
                  set({ currentDiff: response.data });
                }
              } else {
                // File was deleted or no longer has changes, clear the diff
                state.clearDiff();
              }
            }
          }
        });
        toast.success(`Opened repository: ${path}`);
      } else {
        set({ error: response.message || 'Failed to open repository', isLoading: false });
        toast.error(response.message || 'Failed to open repository');
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Unknown error';
      set({ error: message, isLoading: false });
      toast.error(message);
    }
  },

  closeRepo: async () => {
    const { repo } = get();
    if (repo) {
      await api.unsubscribeWatch(repo.repoId);
      await api.closeRepo(repo.repoId);
      set({
        repo: null,
        entries: [],
        currentDiff: null,
        selectedPath: null,
        selectedHunkIndex: null,
        selectedLineIndices: [],
      });
    }
  },

  updateHeadInfo: async (repoId: string) => {
    const { repo } = get();
    if (!repo || repo.repoId !== repoId) return;

    try {
      const response = await api.getHeadInfo(repoId);
      if (response.ok && response.data) {
        // Update just the head info in the repo object
        set({
          repo: {
            ...repo,
            head: response.data,
          },
        });
      }
    } catch (error) {
      console.error('Failed to update HEAD info:', error);
    }
  },

  // Status state
  entries: [],

  refreshStatus: async (repoId: string, silent: boolean = false) => {
    // Only show loading indicator if not a silent/automatic refresh
    if (!silent) {
      set({ isLoading: true });
    }

    try {
      const response = await api.getStatus(repoId);
      if (response.ok && response.data) {
        set({ entries: response.data.entries, isLoading: false });
      } else {
        if (!silent) {
          toast.error(response.message || 'Failed to refresh status');
        }
        set({ isLoading: false });
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Unknown error';
      if (!silent) {
        toast.error(message);
      }
      set({ isLoading: false });
    }
  },

  updateStatus: (status: StatusMatrix) => {
    set({ entries: status.entries });
  },

  getStagedEntries: () => {
    return get().entries.filter((e) => e.stagedStatus !== null);
  },

  getUnstagedEntries: () => {
    return get().entries.filter(
      (e) => e.unstagedStatus !== null || e.untracked
    );
  },

  // Diff state
  currentDiff: null,
  currentDiffSide: null,

  loadDiff: async (repoId: string, path: string, side: 'working' | 'index' | 'head') => {
    set({ isLoading: true });
    try {
      const response = await api.getDiff({ repoId, path, side });
      if (response.ok && response.data) {
        set({ currentDiff: response.data, currentDiffSide: side, isLoading: false });
      } else {
        toast.error(response.message || 'Failed to load diff');
        set({ isLoading: false, currentDiff: null, currentDiffSide: null });
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Unknown error';
      toast.error(message);
      set({ isLoading: false, currentDiff: null, currentDiffSide: null });
    }
  },

  clearDiff: () => {
    set({ currentDiff: null, currentDiffSide: null });
  },

  // Selection state
  selectedPath: null,
  selectedHunkIndex: null,
  selectedLineIndices: [],

  setSelectedPath: (path: string | null) => {
    set({ selectedPath: path, selectedHunkIndex: null, selectedLineIndices: [] });
  },

  setSelectedHunk: (index: number | null) => {
    set({ selectedHunkIndex: index, selectedLineIndices: [] });
  },

  setSelectedLines: (indices: number[]) => {
    set({ selectedLineIndices: indices });
  },

  clearSelection: () => {
    set({ selectedPath: null, selectedHunkIndex: null, selectedLineIndices: [] });
  },

  // UI state
  isOperating: false,

  setIsOperating: (value: boolean) => {
    set({ isOperating: value });
  },

  // Settings state
  settings: {
    showLineNumbers: false,
    theme: 'system',
  },

  updateSettings: (newSettings: Partial<Settings>) => {
    set((state) => ({
      settings: {
        ...state.settings,
        ...newSettings,
      },
    }));
  },

  // History state
  commits: [],
  hasMore: false,
  isLoadingCommitDiff: false,
  selectedCommit: null,
  commitDiff: null,
  selectedCommitFile: null,
  selectedParent: 0,

  loadHistory: async (repoId: string, limit: number = 50, skip: number = 0) => {
    set({ isLoading: true });
    try {
      const response = await api.getLog({ repoId, limit, skip });
      if (response.ok && response.data) {
        if (skip === 0) {
          // Initial load
          set({
            commits: response.data.commits,
            hasMore: response.data.hasMore,
            isLoading: false,
          });
        } else {
          // Load more (append)
          set({
            commits: [...get().commits, ...response.data.commits],
            hasMore: response.data.hasMore,
            isLoading: false,
          });
        }
      } else {
        toast.error(response.message || 'Failed to load history');
        set({ isLoading: false });
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Unknown error';
      toast.error(message);
      set({ isLoading: false });
    }
  },

  loadMore: async (repoId: string) => {
    const { commits, hasMore, isLoading } = get();
    if (!hasMore || isLoading) return;

    await get().loadHistory(repoId, 50, commits.length);
  },

  selectCommit: async (repoId: string, commit: CommitSummary, parent: number = 0) => {
    const requestOid = commit.oid; // Capture the OID for this request

    set({ selectedCommit: commit, selectedParent: parent, selectedCommitFile: null, isLoadingCommitDiff: true, commitDiff: null });
    // Clear working diff state when viewing commit
    set({ currentDiff: null, currentDiffSide: null, selectedPath: null });

    try {
      const response = await api.getCommitDiff({ repoId, oid: commit.oid, parent });

      // Only update state if this commit is still the selected one
      // (user might have clicked on another commit while this was loading)
      const currentlySelected = get().selectedCommit;
      if (currentlySelected?.oid === requestOid) {
        if (response.ok && response.data) {
          set({ commitDiff: response.data.files, isLoadingCommitDiff: false });
        } else {
          toast.error(response.message || 'Failed to load commit diff');
          set({ isLoadingCommitDiff: false, commitDiff: null });
        }
      }
      // If different commit is now selected, silently ignore this response
    } catch (error) {
      // Only show error if this commit is still selected
      const currentlySelected = get().selectedCommit;
      if (currentlySelected?.oid === requestOid) {
        const message = error instanceof Error ? error.message : 'Unknown error';
        toast.error(message);
        set({ isLoadingCommitDiff: false, commitDiff: null });
      }
    }
  },

  selectCommitFile: (file: CommitFileDiff | null) => {
    set({ selectedCommitFile: file });
  },

  setSelectedParent: async (parent: number) => {
    const { repo, selectedCommit } = get();
    if (!repo || !selectedCommit) return;

    await get().selectCommit(repo.repoId, selectedCommit, parent);
  },

  clearHistory: () => {
    set({
      commits: [],
      hasMore: false,
      selectedCommit: null,
      commitDiff: null,
      selectedCommitFile: null,
      selectedParent: 0,
    });
  },
}));
