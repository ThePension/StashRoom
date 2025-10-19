import { useStore } from '../state/store';

interface SettingsDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

export function SettingsDialog({ isOpen, onClose }: SettingsDialogProps) {
  const showLineNumbers = useStore((s) => s.settings.showLineNumbers);
  const theme = useStore((s) => s.settings.theme);
  const contextLines = useStore((s) => s.settings.contextLines);
  const compactMode = useStore((s) => s.settings.compactMode);
  const updateSettings = useStore((s) => s.updateSettings);

  if (!isOpen) return null;

  const handleToggleLineNumbers = () => {
    updateSettings({ showLineNumbers: !showLineNumbers });
  };

  const handleToggleCompactMode = () => {
    updateSettings({ compactMode: !compactMode });
  };

  const handleThemeChange = (newTheme: 'light' | 'dark' | 'system') => {
    updateSettings({ theme: newTheme });
  };

  const handleContextLinesChange = (value: number | 'all') => {
    updateSettings({ contextLines: value });
  };

  const handleClose = () => {
    onClose();
  };

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-white dark:bg-gray-800 rounded-lg shadow-xl w-full max-w-2xl mx-4">
        {/* Header */}
        <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">Settings</h2>
          <button
            onClick={handleClose}
            className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        {/* Content */}
        <div className="px-6 py-4">
          <div className="space-y-6">
            {/* Appearance Section */}
            <div>
              <h3 className="text-sm font-medium text-gray-900 dark:text-gray-100 mb-3">Appearance</h3>
              <div className="space-y-3">
                {/* Theme Selection */}
                <div className="py-2 px-3 rounded">
                  <div className="text-sm text-gray-900 dark:text-gray-100 mb-2">Theme</div>
                  <div className="text-xs text-gray-500 dark:text-gray-400 mb-3">
                    Choose the application theme
                  </div>
                  <div className="flex gap-2">
                    <button
                      onClick={() => handleThemeChange('light')}
                      className={`
                        flex-1 px-4 py-2 rounded text-sm font-medium transition-colors
                        ${theme === 'light'
                          ? 'bg-blue-600 text-white'
                          : 'bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-600'
                        }
                      `}
                    >
                      Light
                    </button>
                    <button
                      onClick={() => handleThemeChange('dark')}
                      className={`
                        flex-1 px-4 py-2 rounded text-sm font-medium transition-colors
                        ${theme === 'dark'
                          ? 'bg-blue-600 text-white'
                          : 'bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-600'
                        }
                      `}
                    >
                      Dark
                    </button>
                    <button
                      onClick={() => handleThemeChange('system')}
                      className={`
                        flex-1 px-4 py-2 rounded text-sm font-medium transition-colors
                        ${theme === 'system'
                          ? 'bg-blue-600 text-white'
                          : 'bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-600'
                        }
                      `}
                    >
                      System
                    </button>
                  </div>
                </div>

                {/* Compact Mode Toggle */}
                <label className="flex items-center justify-between py-2 px-3 rounded hover:bg-gray-50 dark:hover:bg-gray-700 cursor-pointer">
                  <div className="flex-1">
                    <div className="text-sm text-gray-900 dark:text-gray-100">Compact mode</div>
                    <div className="text-xs text-gray-500 dark:text-gray-400">
                      Reduce padding and spacing for more content
                    </div>
                  </div>
                  <div className="ml-4">
                    <button
                      type="button"
                      onClick={handleToggleCompactMode}
                      className={`
                        relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent
                        transition-colors duration-200 ease-in-out focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2
                        ${compactMode ? 'bg-blue-600' : 'bg-gray-200 dark:bg-gray-600'}
                      `}
                    >
                      <span
                        className={`
                          pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0
                          transition duration-200 ease-in-out
                          ${compactMode ? 'translate-x-5' : 'translate-x-0'}
                        `}
                      />
                    </button>
                  </div>
                </label>
              </div>
            </div>

            {/* Editor Section */}
            <div>
              <h3 className="text-sm font-medium text-gray-900 dark:text-gray-100 mb-3">Editor</h3>
              <div className="space-y-3">
                {/* Line Numbers Toggle */}
                <label className="flex items-center justify-between py-2 px-3 rounded hover:bg-gray-50 dark:hover:bg-gray-700 cursor-pointer">
                  <div className="flex-1">
                    <div className="text-sm text-gray-900 dark:text-gray-100">Show line numbers</div>
                    <div className="text-xs text-gray-500 dark:text-gray-400">
                      Display line numbers from the source file in the diff view
                    </div>
                  </div>
                  <div className="ml-4">
                    <button
                      type="button"
                      onClick={handleToggleLineNumbers}
                      className={`
                        relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent
                        transition-colors duration-200 ease-in-out focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2
                        ${showLineNumbers ? 'bg-blue-600' : 'bg-gray-200 dark:bg-gray-600'}
                      `}
                    >
                      <span
                        className={`
                          pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0
                          transition duration-200 ease-in-out
                          ${showLineNumbers ? 'translate-x-5' : 'translate-x-0'}
                        `}
                      />
                    </button>
                  </div>
                </label>

                {/* Context Lines */}
                <div className="py-2 px-3 rounded">
                  <div className="text-sm text-gray-900 dark:text-gray-100 mb-2">Context lines</div>
                  <div className="text-xs text-gray-500 dark:text-gray-400 mb-3">
                    Number of unchanged lines to show around changes
                  </div>
                  <div className="flex gap-2 flex-wrap">
                    {[1, 3, 5, 10].map((num) => (
                      <button
                        key={num}
                        onClick={() => handleContextLinesChange(num)}
                        className={`
                          px-4 py-2 rounded text-sm font-medium transition-colors
                          ${contextLines === num
                            ? 'bg-blue-600 text-white'
                            : 'bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-600'
                          }
                        `}
                      >
                        {num}
                      </button>
                    ))}
                    <button
                      onClick={() => handleContextLinesChange('all')}
                      className={`
                        px-4 py-2 rounded text-sm font-medium transition-colors
                        ${contextLines === 'all'
                          ? 'bg-blue-600 text-white'
                          : 'bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-600'
                        }
                      `}
                    >
                      All
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="px-6 py-4 border-t border-gray-200 dark:border-gray-700 flex justify-end">
          <button
            onClick={handleClose}
            className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
