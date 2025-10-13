import { useEffect, useState } from 'react';

export function KeyboardShortcutsHelp() {
  const [isOpen, setIsOpen] = useState(false);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === '?' && !e.ctrlKey && !e.metaKey) {
        e.preventDefault();
        setIsOpen(true);
      }
      if (e.key === 'Escape') {
        setIsOpen(false);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  if (!isOpen) return null;

  const shortcuts = [
    { key: '↑ / ↓', description: 'Navigate file list' },
    { key: 'Enter', description: 'View file diff' },
    { key: 'S', description: 'Stage/unstage selected file' },
    { key: 'D', description: 'Discard changes (with confirmation)' },
    { key: '?', description: 'Show this help' },
    { key: 'Esc', description: 'Close this help' },
  ];

  return (
    <div
      className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
      onClick={() => setIsOpen(false)}
    >
      <div
        className="bg-white dark:bg-gray-800 rounded-lg shadow-xl p-6 max-w-md w-full mx-4"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-xl font-bold text-gray-800 dark:text-gray-100">
            Keyboard Shortcuts
          </h2>
          <button
            onClick={() => setIsOpen(false)}
            className="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
          >
            ✕
          </button>
        </div>

        <div className="space-y-3">
          {shortcuts.map((shortcut) => (
            <div
              key={shortcut.key}
              className="flex items-center justify-between py-2 border-b border-gray-200 dark:border-gray-700"
            >
              <span className="font-mono text-sm text-gray-600 dark:text-gray-400 bg-gray-100 dark:bg-gray-900 px-2 py-1 rounded">
                {shortcut.key}
              </span>
              <span className="text-sm text-gray-700 dark:text-gray-300 ml-4 flex-1 text-right">
                {shortcut.description}
              </span>
            </div>
          ))}
        </div>

        <div className="mt-6 text-xs text-gray-500 dark:text-gray-400 text-center">
          Press <span className="font-mono">Esc</span> to close
        </div>
      </div>
    </div>
  );
}
