import { useState, useEffect } from 'react';
import { Panel, PanelGroup, PanelResizeHandle } from 'react-resizable-panels';
import { Toaster } from 'sonner';
import { Settings, FolderOpen, Terminal as TerminalIcon, Archive } from 'lucide-react';
import { useStore } from './state/store';
import { ChangeList } from './components/ChangeList';
import { DiffPanel } from './components/DiffPanel';
import { StagePanel } from './components/StagePanel';
import { KeyboardShortcutsHelp } from './components/KeyboardShortcutsHelp';
import { HistoryPanel } from './components/HistoryPanel';
import { BranchPanel } from './components/BranchPanel';
import { SettingsDialog } from './components/SettingsDialog';
import { QuickSearchModal } from './components/QuickSearchModal';
import { RepoTabs } from './components/RepoTabs';
import { BackupsModal } from './components/BackupsModal';

function App() {
  const getActiveRepo = useStore((s) => s.getActiveRepo);
  const repo = getActiveRepo();
  const isLoading = useStore((s) => s.isLoading);
  const isOperating = useStore((s) => s.isOperating);
  const openRepo = useStore((s) => s.openRepo);
  const restoreFromPersistence = useStore((s) => s.restoreFromPersistence);
  const theme = useStore((s) => s.settings.theme);
  const compactMode = useStore((s) => s.settings.compactMode);
  const [showHistory, setShowHistory] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [showQuickSearch, setShowQuickSearch] = useState(false);
  const [showBackups, setShowBackups] = useState(false);

  // Apply theme based on settings - runs on every theme change
  useEffect(() => {
    const applyTheme = () => {
      const root = document.documentElement;

      if (theme === 'system') {
        // Use system preference
        const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
        if (prefersDark) {
          root.classList.add('dark');
        } else {
          root.classList.remove('dark');
        }
      } else if (theme === 'dark') {
        root.classList.add('dark');
      } else {
        root.classList.remove('dark');
      }
    };

    applyTheme();

    // Listen for system theme changes when in 'system' mode
    if (theme === 'system') {
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
      const handler = () => applyTheme();
      mediaQuery.addEventListener('change', handler);
      return () => mediaQuery.removeEventListener('change', handler);
    }
  }, [theme]);

  // Restore from persistence on mount
  useEffect(() => {
    restoreFromPersistence();
  }, []);

  // Global keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Settings shortcut (Ctrl+, or Cmd+,)
      if ((e.ctrlKey || e.metaKey) && e.key === ',') {
        e.preventDefault();
        setShowSettings(true);
      }
      // Quick Search shortcut (Ctrl+P or Cmd+P)
      else if ((e.ctrlKey || e.metaKey) && e.key === 'p') {
        e.preventDefault();
        setShowQuickSearch(true);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  const handleSelectRepo = async () => {
    try {
      // Use Tauri dialog to select folder
      const { open: tauriOpen } = await import('@tauri-apps/plugin-dialog');
      const selected = await tauriOpen({
        directory: true,
        multiple: false,
        title: 'Select Git Repository',
      });

      if (selected && typeof selected === 'string') {
        await openRepo(selected);
      }
    } catch (error) {
      console.error('Error opening folder:', error);
    }
  };

  const handleOpenFolder = async () => {
    if (!repo) return;

    try {
      const { Command } = await import('@tauri-apps/plugin-shell');
      const { platform } = await import('@tauri-apps/plugin-os');
      const currentPlatform = platform();

      if (currentPlatform === 'windows') {
        // Windows: Use explorer
        await Command.create('explorer', [repo.path]).execute();
      } else {
        // Linux: Use xdg-open
        await Command.create('xdg-open', [repo.path]).execute();
      }
    } catch (error) {
      console.error('Error opening folder:', error);
      const { toast } = await import('sonner');
      toast.error('Failed to open folder in file manager.');
    }
  };

  const handleOpenTerminal = async () => {
    if (!repo) return;

    try {
      const { Command } = await import('@tauri-apps/plugin-shell');
      const { platform } = await import('@tauri-apps/plugin-os');
      const currentPlatform = platform();

      if (currentPlatform === 'windows') {
        // Windows: Use cmd with start to open a new terminal window
        await Command.create('cmd', ['/c', 'start', 'cmd', '/k', 'cd', '/d', repo.path]).execute();
      } else {
        // Linux: Try gnome-terminal first, fall back to x-terminal-emulator
        try {
          await Command.create('gnome-terminal', ['--working-directory', repo.path]).execute();
        } catch {
          // Fall back to generic x-terminal-emulator
          await Command.create('x-terminal-emulator', ['-e', `bash -c "cd '${repo.path}' && exec bash"`]).execute();
        }
      }
    } catch (error) {
      console.error('Error opening terminal:', error);
      const { toast } = await import('sonner');
      toast.error('Failed to open terminal.');
    }
  };

  if (!repo) {
    return (
      <div className="h-screen flex items-center justify-center bg-gray-50 dark:bg-gray-900">
        <Toaster position="bottom-right" />
        <div className="text-center">
          <h1 className="text-3xl font-bold text-gray-800 dark:text-gray-100 mb-4">
            StashRoom
          </h1>
          <p className="text-gray-600 dark:text-gray-400 mb-8">
            A modern Git client
          </p>
          <button
            onClick={handleSelectRepo}
            disabled={isLoading}
            className="px-6 py-3 bg-blue-500 text-white rounded-lg hover:bg-blue-600 disabled:opacity-50 font-medium"
          >
            {isLoading ? 'Opening...' : 'Open Repository'}
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="h-screen flex flex-col bg-white dark:bg-gray-900">
      <Toaster position="bottom-right" />
      <KeyboardShortcutsHelp />
      <SettingsDialog isOpen={showSettings} onClose={() => setShowSettings(false)} />
      <QuickSearchModal
        isOpen={showQuickSearch}
        onClose={() => setShowQuickSearch(false)}
        onNavigateToHistory={() => setShowHistory(true)}
      />
      <BackupsModal isOpen={showBackups} onClose={() => setShowBackups(false)} />

      {/* Progress Bar */}
      {(isLoading || isOperating) && (
        <div className="h-1 w-full bg-blue-500 animate-pulse" />
      )}

      {/* Repository Tabs */}
      <RepoTabs />

      {/* Header */}
      <header className={`flex items-center justify-between ${compactMode ? 'px-2 py-1.5' : 'px-4 py-3'} bg-gray-50 dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700`}>
        <div className="flex items-center gap-4">
          <h1 className={`${compactMode ? 'text-base' : 'text-xl'} font-bold text-gray-800 dark:text-gray-100`}>
            StashRoom
          </h1>
          <div className={`${compactMode ? 'text-xs' : 'text-sm'} text-gray-600 dark:text-gray-400`}>
            <span className="font-mono">{repo.path}</span>
          </div>
        </div>

        <div className="flex items-center gap-2">
          {repo.head?.branch && (
            <div
              className={`${compactMode ? 'px-2 py-0.5 text-xs' : 'px-3 py-1 text-sm'} bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300 rounded font-mono max-w-[200px] truncate`}
              title={repo.head.branch}
            >
              {repo.head.branch}
            </div>
          )}
          <button
            onClick={handleOpenFolder}
            className={`${compactMode ? 'p-1' : 'p-2'} text-gray-600 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200 hover:bg-gray-200 dark:hover:bg-gray-700 rounded`}
            title="Open repository folder in file manager"
          >
            <FolderOpen className={compactMode ? 'w-4 h-4' : 'w-5 h-5'} />
          </button>
          <button
            onClick={handleOpenTerminal}
            className={`${compactMode ? 'p-1' : 'p-2'} text-gray-600 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200 hover:bg-gray-200 dark:hover:bg-gray-700 rounded`}
            title="Open terminal at repository location"
          >
            <TerminalIcon className={compactMode ? 'w-4 h-4' : 'w-5 h-5'} />
          </button>
          <button
            onClick={() => setShowBackups(true)}
            className={`${compactMode ? 'p-1' : 'p-2'} text-gray-600 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200 hover:bg-gray-200 dark:hover:bg-gray-700 rounded`}
            title="Backups"
          >
            <Archive className={compactMode ? 'w-4 h-4' : 'w-5 h-5'} />
          </button>
          <button
            onClick={() => setShowSettings(true)}
            className={`${compactMode ? 'p-1' : 'p-2'} text-gray-600 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200 hover:bg-gray-200 dark:hover:bg-gray-700 rounded`}
            title="Settings (Ctrl+,)"
          >
            <Settings className={compactMode ? 'w-4 h-4' : 'w-5 h-5'} />
          </button>
          <button
            onClick={handleSelectRepo}
            className={`${compactMode ? 'px-2 py-0.5 text-xs' : 'px-3 py-1 text-sm'} text-gray-600 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200`}
          >
            Open Repo
          </button>
        </div>
      </header>

      {/* Main Content */}
      <div className="flex-1 overflow-hidden">
        <PanelGroup direction="horizontal">
          {/* Left Panel - Changes / History */}
          <Panel defaultSize={25} minSize={15}>
            <PanelGroup direction="vertical">
              {/* Top: Changes / History */}
              <Panel defaultSize={70} minSize={30}>
                <div className="h-full flex flex-col border-r border-gray-200 dark:border-gray-700">
                  {/* Tab Header */}
                  <div className={`${compactMode ? 'px-2 py-1' : 'px-4 py-2'} bg-gray-50 dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700`}>
                    <div className={`flex items-center gap-2 ${compactMode ? 'mb-0.5' : 'mb-1'}`}>
                      <button
                        onClick={() => setShowHistory(false)}
                        className={`${compactMode ? 'px-2 py-0.5 text-xs' : 'px-3 py-1 text-sm'} rounded ${
                          !showHistory
                            ? 'bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 font-semibold'
                            : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100'
                        }`}
                      >
                        Changes
                      </button>
                      <button
                        onClick={() => setShowHistory(true)}
                        className={`${compactMode ? 'px-2 py-0.5 text-xs' : 'px-3 py-1 text-sm'} rounded ${
                          showHistory
                            ? 'bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 font-semibold'
                            : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100'
                        }`}
                      >
                        History
                      </button>
                    </div>
                    {!showHistory ? (
                      <p className={`${compactMode ? 'text-[10px]' : 'text-xs'} text-gray-500 dark:text-gray-400`}>
                        ↑↓ navigate • Enter view • S stage • D discard
                      </p>
                    ) : (
                      <p className={`${compactMode ? 'text-[10px]' : 'text-xs'} text-gray-500 dark:text-gray-400`}>
                        Click commit to view diff
                      </p>
                    )}
                  </div>

                  {/* Content */}
                  {!showHistory ? <ChangeList type="unstaged" /> : <HistoryPanel />}
                </div>
              </Panel>

              <PanelResizeHandle className="h-1 bg-gray-200 dark:bg-gray-700 hover:bg-blue-500 transition-colors" />

              {/* Bottom: Branch Panel */}
              <Panel defaultSize={30} minSize={20}>
                <BranchPanel />
              </Panel>
            </PanelGroup>
          </Panel>

          <PanelResizeHandle className="w-1 bg-gray-200 dark:bg-gray-700 hover:bg-blue-500 transition-colors" />

          {/* Middle Panel - Diff Viewer */}
          <Panel defaultSize={showHistory ? 75 : 50} minSize={30}>
            <div className="h-full flex flex-col overflow-hidden">
              <DiffPanel />
            </div>
          </Panel>

          {!showHistory && (
            <>
              <PanelResizeHandle className="w-1 bg-gray-200 dark:bg-gray-700 hover:bg-blue-500 transition-colors" />

              {/* Right Panel - Staged + Commit */}
              <Panel defaultSize={25} minSize={20}>
                <StagePanel />
              </Panel>
            </>
          )}
        </PanelGroup>
      </div>

      {/* Footer */}
      <footer className="px-4 py-2 bg-gray-50 dark:bg-gray-800 border-t border-gray-200 dark:border-gray-700 text-xs text-gray-500 dark:text-gray-400">
        <div className="flex items-center justify-between">
          <span>Press ? for keyboard shortcuts</span>
          {repo.head?.commit && (
            <span className="font-mono">HEAD: {repo.head.commit.substring(0, 7)}</span>
          )}
        </div>
      </footer>
    </div>
  );
}

export default App;
