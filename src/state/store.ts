import { create } from 'zustand';
import { api } from '../lib/api';
import { toast } from 'sonner';
import { saveWorkspace, saveSettings, loadWorkspace, loadSettings } from '../lib/persistence';
import type {
  RepoOpenResponse,
  StatusEntry,
  FileDiff,
  StatusMatrix,
  CommitSummary,
  CommitFileDiff,
} from '../lib/types';

interface RepoState {
  repos: RepoOpenResponse[];
  activeRepoId: string | null;
  isLoading: boolean;
  error: string | null;
  openRepo: (path: string, skipPersist?: boolean) => Promise<void>;
  /**
   * Closes a repository. Prevents closing the last repository to avoid empty UI state.
   * Shows an error toast if attempting to close the last repo.
   */
  closeRepo: (repoId: string) => Promise<void>;
  setActiveRepo: (repoId: string, skipPersist?: boolean) => Promise<void>;
  getActiveRepo: () => RepoOpenResponse | null;
  updateHeadInfo: (repoId: string) => Promise<void>;
  restoreFromPersistence: () => Promise<void>;
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
  contextLines: number | 'all'; // number of context lines or 'all' for whole file
  compactMode: boolean;
  treeViewMode: 'flat' | 'tree'; // file list view mode
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

// Helper function to persist workspace state
const persistWorkspace = (repos: RepoOpenResponse[], activeRepoId: string | null) => {
  const activeRepo = repos.find(r => r.repoId === activeRepoId);
  saveWorkspace({
    version: 1,
    repos: repos.map(r => ({
      path: r.path,
      lastOpened: Date.now(),
    })),
    activeRepoPath: activeRepo?.path || null,
  });
};

export const useStore = create<AppStore>((set, get) => ({
  // Repo state
  repos: [],
  activeRepoId: null,
  isLoading: false,
  error: null,

  openRepo: async (path: string, skipPersist: boolean = false) => {
    set({ isLoading: true, error: null });
    try {
      // Check if repo is already open
      const { repos } = get();
      const existingRepo = repos.find(r => r.path === path);

      if (existingRepo) {
        // Repo already open, just switch to it
        set({ isLoading: false });
        await get().setActiveRepo(existingRepo.repoId, skipPersist);
        if (!skipPersist) {
          toast.success(`Switched to repository: ${path}`);
        }
        return;
      }

      const response = await api.openRepo({ path });
      if (response.ok && response.data) {
        const newRepo = response.data;

        // Check if repo with this ID already exists (can happen in React Strict Mode)
        const currentRepos = get().repos;
        const existingById = currentRepos.find(r => r.repoId === newRepo.repoId);

        if (existingById) {
          // Repo already exists with same ID, just switch to it
          set({ isLoading: false });
          await get().setActiveRepo(existingById.repoId, skipPersist);
          if (!skipPersist) {
            toast.success(`Switched to repository: ${path}`);
          }
          return;
        }

        // Add repo to list and make it active
        set((state) => ({
          repos: [...state.repos, newRepo],
          activeRepoId: newRepo.repoId,
          isLoading: false,
        }));

        // Clear state when switching to new repo
        set({
          entries: [],
          currentDiff: null,
          currentDiffSide: null,
          selectedPath: null,
          selectedHunkIndex: null,
          selectedLineIndices: [],
          commits: [],
          hasMore: false,
          selectedCommit: null,
          commitDiff: null,
          selectedCommitFile: null,
          selectedParent: 0,
        });

        // Automatically load status
        await get().refreshStatus(newRepo.repoId);
        // Subscribe to watch events
        await api.subscribeWatch(newRepo.repoId);

        // Persist workspace state (skip during restoration to avoid duplicates)
        if (!skipPersist) {
          persistWorkspace(get().repos, newRepo.repoId);
          toast.success(`Opened repository: ${path}`);
        }
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

  closeRepo: async (repoId: string) => {
    const { repos, activeRepoId } = get();

    // Prevent closing the last repo
    if (repos.length <= 1) {
      toast.error('Cannot close the last repository');
      return;
    }

    const repoToClose = repos.find(r => r.repoId === repoId);

    if (repoToClose) {
      // Unsubscribe from watch events and close the repo
      await api.unsubscribeWatch(repoId);
      await api.closeRepo(repoId);

      // Remove repo from list
      const updatedRepos = repos.filter(r => r.repoId !== repoId);

      // If we're closing the active repo, switch to another one or clear state
      if (activeRepoId === repoId) {
        const newActiveRepo = updatedRepos[0] || null;
        set({
          repos: updatedRepos,
          activeRepoId: newActiveRepo?.repoId || null,
          entries: [],
          currentDiff: null,
          selectedPath: null,
          selectedHunkIndex: null,
          selectedLineIndices: [],
          commits: [],
          hasMore: false,
          selectedCommit: null,
          commitDiff: null,
          selectedCommitFile: null,
        });

        // Load status for new active repo if exists
        if (newActiveRepo) {
          await get().refreshStatus(newActiveRepo.repoId);
        }

        // Persist workspace state
        persistWorkspace(updatedRepos, newActiveRepo?.repoId || null);
      } else {
        set({ repos: updatedRepos });
        // Persist workspace state
        persistWorkspace(updatedRepos, activeRepoId);
      }
    }
  },

  setActiveRepo: async (repoId: string, skipPersist: boolean = false) => {
    const { activeRepoId } = get();
    if (activeRepoId === repoId) return;

    set({ activeRepoId: repoId });

    // Clear and reload state for the new active repo
    set({
      entries: [],
      currentDiff: null,
      currentDiffSide: null,
      selectedPath: null,
      selectedHunkIndex: null,
      selectedLineIndices: [],
      commits: [],
      hasMore: false,
      selectedCommit: null,
      commitDiff: null,
      selectedCommitFile: null,
      selectedParent: 0,
    });

    // Load status for new active repo
    await get().refreshStatus(repoId);

    // Persist workspace state (skip during restoration)
    if (!skipPersist) {
      persistWorkspace(get().repos, repoId);
    }
  },

  getActiveRepo: () => {
    const { repos, activeRepoId } = get();
    return repos.find(r => r.repoId === activeRepoId) || null;
  },

  updateHeadInfo: async (repoId: string) => {
    const { repos } = get();
    const repo = repos.find(r => r.repoId === repoId);
    if (!repo) return;

    try {
      const response = await api.getHeadInfo(repoId);
      if (response.ok && response.data) {
        // Update just the head info in the repo object
        set({
          repos: repos.map(r =>
            r.repoId === repoId ? { ...r, head: response.data || null } : r
          ),
        });
      }
    } catch (error) {
      console.error('Failed to update HEAD info:', error);
    }
  },

  restoreFromPersistence: async () => {
    try {
      // Load workspace state
      const workspace = await loadWorkspace();
      
      if (!workspace || !workspace.repos || workspace.repos.length === 0) {
        // Still load settings even if no repos
        const settings = await loadSettings();
        if (settings) {
          set({
            settings: {
              showLineNumbers: settings.showLineNumbers,
              theme: settings.theme,
              contextLines: settings.contextLines,
              compactMode: settings.compactMode,
              treeViewMode: settings.treeViewMode ?? 'flat',
            },
          });
        }
        return;
      }

      // Validate repo paths (deduplicate first to avoid opening same repo multiple times)
      const paths = [...new Set(workspace.repos.map(r => r.path))];
      
      const validationResponse = await api.validateRepoPaths(paths);

      if (!validationResponse.ok || !validationResponse.data) {
        return;
      }

      const validatedRepos = validationResponse.data;
      
      const validPaths = validatedRepos
        .filter(v => v.exists && v.isGitRepo)
        .map(v => v.path);

      // Show toast for missing repos
      const missingRepos = validatedRepos.filter(v => !v.exists || !v.isGitRepo);
      if (missingRepos.length > 0) {
        toast.error(`${missingRepos.length} repository(ies) are missing or invalid`);
      }

      // Open valid repos (skip persistence during restoration)
      for (const path of validPaths) {
        try {
          await get().openRepo(path, true); // skipPersist = true
        } catch (error) {
          console.error(`Failed to restore repo ${path}:`, error);
        }
      }

      // Set active repo if it was restored
      if (workspace.activeRepoPath && validPaths.includes(workspace.activeRepoPath)) {
        const { repos } = get();
        const activeRepo = repos.find(r => r.path === workspace.activeRepoPath);
        if (activeRepo) {
          await get().setActiveRepo(activeRepo.repoId, true); // skipPersist = true
        }
      }

      // Restoration complete - the existing workspace.json is already correct

      // Load settings
      const settings = await loadSettings();
      if (settings) {
        set({
          settings: {
            showLineNumbers: settings.showLineNumbers,
            theme: settings.theme,
            contextLines: settings.contextLines,
            compactMode: settings.compactMode,
            treeViewMode: settings.treeViewMode ?? 'flat',
          },
        });
      }
    } catch (error) {
      console.error('Failed to restore from persistence:', error);
      // Don't show error toast on first load - might just be no data yet
      if (error instanceof Error && !error.message.includes('not found')) {
        toast.error('Failed to restore previous session');
      }
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
      const { contextLines } = get().settings;
      const contextLinesParam = contextLines === 'all' ? null : contextLines;

      const response = await api.getDiff({ repoId, path, side, contextLines: contextLinesParam });
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
    set({
      selectedPath: null,
      selectedHunkIndex: null,
      selectedLineIndices: [],
      currentDiff: null,
      currentDiffSide: null
    });
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
    contextLines: 3,
    compactMode: false,
    treeViewMode: 'flat',
  },

  updateSettings: (newSettings: Partial<Settings>) => {
    set((state) => ({
      settings: {
        ...state.settings,
        ...newSettings,
      },
    }));

    // Persist settings
    const { settings } = get();
    saveSettings({
      version: 1,
      theme: settings.theme,
      showLineNumbers: settings.showLineNumbers,
      contextLines: settings.contextLines,
      compactMode: settings.compactMode,
      treeViewMode: settings.treeViewMode,
    });
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
    const activeRepo = get().getActiveRepo();
    const { selectedCommit } = get();
    if (!activeRepo || !selectedCommit) return;

    await get().selectCommit(activeRepo.repoId, selectedCommit, parent);
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
