import { Panel, PanelGroup, PanelResizeHandle } from 'react-resizable-panels';
import { Toaster } from 'sonner';
import { useStore } from './state/store';
import { ChangeList } from './components/ChangeList';
import { DiffPanel } from './components/DiffPanel';
import { StagePanel } from './components/StagePanel';
import { KeyboardShortcutsHelp } from './components/KeyboardShortcutsHelp';

function App() {
  const repo = useStore((s) => s.repo);
  const isLoading = useStore((s) => s.isLoading);
  const isOperating = useStore((s) => s.isOperating);
  const openRepo = useStore((s) => s.openRepo);

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
            Kite
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

      {/* Progress Bar */}
      {(isLoading || isOperating) && (
        <div className="h-1 w-full bg-blue-500 animate-pulse" />
      )}

      {/* Header */}
      <header className="flex items-center justify-between px-4 py-3 bg-gray-50 dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700">
        <div className="flex items-center gap-4">
          <h1 className="text-xl font-bold text-gray-800 dark:text-gray-100">
            Kite
          </h1>
          <div className="text-sm text-gray-600 dark:text-gray-400">
            <span className="font-mono">{repo.path}</span>
          </div>
        </div>

        <div className="flex items-center gap-2">
          {repo.head?.branch && (
            <div className="px-3 py-1 bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300 rounded text-sm font-mono">
              {repo.head.branch}
            </div>
          )}
          <button
            onClick={handleOpenFolder}
            className="px-3 py-1 text-sm bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 hover:bg-gray-300 dark:hover:bg-gray-600 rounded"
            title="Open repository folder in file manager"
          >
            Open Folder
          </button>
          <button
            onClick={handleOpenTerminal}
            className="px-3 py-1 text-sm bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 hover:bg-gray-300 dark:hover:bg-gray-600 rounded"
            title="Open terminal at repository location"
          >
            Terminal
          </button>
          <button
            onClick={handleSelectRepo}
            className="px-3 py-1 text-sm text-gray-600 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200"
          >
            Change Repo
          </button>
        </div>
      </header>

      {/* Main Content */}
      <div className="flex-1 overflow-hidden">
        <PanelGroup direction="horizontal">
          {/* Left Panel - Unstaged Changes */}
          <Panel defaultSize={25} minSize={15}>
            <div className="h-full flex flex-col border-r border-gray-200 dark:border-gray-700">
              <div className="px-4 py-2 bg-gray-50 dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700">
                <h3 className="text-sm font-semibold text-gray-700 dark:text-gray-300">
                  Changes
                </h3>
                <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                  ↑↓ navigate • Enter view • S stage • D discard
                </p>
              </div>
              <ChangeList type="unstaged" />
            </div>
          </Panel>

          <PanelResizeHandle className="w-1 bg-gray-200 dark:bg-gray-700 hover:bg-blue-500 transition-colors" />

          {/* Middle Panel - Diff Viewer */}
          <Panel defaultSize={50} minSize={30}>
            <div className="h-full flex flex-col overflow-hidden">
              <DiffPanel />
            </div>
          </Panel>

          <PanelResizeHandle className="w-1 bg-gray-200 dark:bg-gray-700 hover:bg-blue-500 transition-colors" />

          {/* Right Panel - Staged + Commit */}
          <Panel defaultSize={25} minSize={20}>
            <StagePanel />
          </Panel>
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
