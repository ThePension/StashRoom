import { Store } from '@tauri-apps/plugin-store';

// Store file names
const WORKSPACE_STORE = 'workspace.json';
const SETTINGS_STORE = 'settings.json';

// Store instances
let workspaceStore: Store | null = null;
let settingsStore: Store | null = null;

// Debounce timers
let workspaceDebounceTimer: number | null = null;
let settingsDebounceTimer: number | null = null;

const DEBOUNCE_MS = 300;

// Workspace state interface
export interface WorkspaceState {
  version: number;
  repos: RepoEntry[];
  activeRepoPath: string | null;
}

export interface RepoEntry {
  path: string;
  lastOpened?: number;
}

// Settings state interface
export interface SettingsState {
  version: number;
  theme: 'light' | 'dark' | 'system';
  showLineNumbers: boolean;
  contextLines: number | 'all';
  compactMode: boolean;
  treeViewMode: 'flat' | 'tree';
}

// Initialize stores
// Store files are saved in the app's data directory:
// Windows: %APPDATA%\com.stashroom.dev\workspace.json and settings.json
// Linux: ~/.config/stashroom/workspace.json and settings.json
// macOS: ~/Library/Application Support/com.stashroom.dev/workspace.json and settings.json
async function initStores() {
  if (!workspaceStore) {
    workspaceStore = await Store.load(WORKSPACE_STORE);
  }
  if (!settingsStore) {
    settingsStore = await Store.load(SETTINGS_STORE);
  }
}

// Workspace store operations
export async function loadWorkspace(): Promise<WorkspaceState | null> {
  await initStores();
  const data = await workspaceStore!.get<WorkspaceState>('workspace');
  return data || null;
}

export async function saveWorkspace(state: WorkspaceState) {
  await initStores();

  // Debounce writes
  if (workspaceDebounceTimer) {
    clearTimeout(workspaceDebounceTimer);
  }

  workspaceDebounceTimer = setTimeout(async () => {
    try {
      await workspaceStore!.set('workspace', state);
      await workspaceStore!.save();
    } catch (error) {
      console.error('Failed to save workspace:', error);
    }
  }, DEBOUNCE_MS);
}

// Settings store operations
export async function loadSettings(): Promise<SettingsState | null> {
  await initStores();
  const data = await settingsStore!.get<SettingsState>('settings');
  return data || null;
}

export async function saveSettings(state: SettingsState) {
  await initStores();

  // Debounce writes
  if (settingsDebounceTimer) {
    clearTimeout(settingsDebounceTimer);
  }

  settingsDebounceTimer = setTimeout(async () => {
    try {
      await settingsStore!.set('settings', state);
      await settingsStore!.save();
    } catch (error) {
      console.error('Failed to save settings:', error);
    }
  }, DEBOUNCE_MS);
}

// Helper to create default workspace state
export function createDefaultWorkspace(): WorkspaceState {
  return {
    version: 1,
    repos: [],
    activeRepoPath: null,
  };
}

// Helper to create default settings state
export function createDefaultSettings(): SettingsState {
  return {
    version: 1,
    theme: 'system',
    showLineNumbers: false,
    contextLines: 3,
    compactMode: false,
    treeViewMode: 'flat',
  };
}
