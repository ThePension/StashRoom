import { useState, useMemo } from 'react';
import { useStore } from '../state/store';
import { ChangeList } from './ChangeList';
import { api } from '../lib/api';
import { toast } from 'sonner';

export function StagePanel() {
  const repo = useStore((s) => s.repo);
  const allEntries = useStore((s) => s.entries);
  const setIsOperating = useStore((s) => s.setIsOperating);
  const refreshStatus = useStore((s) => s.refreshStatus);
  const clearSelection = useStore((s) => s.clearSelection);

  // Filter staged entries (memoized to prevent infinite loops)
  const stagedEntries = useMemo(() => {
    return allEntries.filter((e) => e.stagedStatus !== null);
  }, [allEntries]);

  const [commitMessage, setCommitMessage] = useState('');
  const [isCommitting, setIsCommitting] = useState(false);

  const handleCommit = async () => {
    if (!repo || !commitMessage.trim() || !stagedEntries.length) {
      toast.error('Please enter a commit message and stage some changes');
      return;
    }

    setIsCommitting(true);
    setIsOperating(true);

    try {
      const response = await api.commit({
        repoId: repo.repoId,
        message: commitMessage.trim(),
      });

      if (response.ok && response.data) {
        toast.success(`Committed ${response.data.oid.substring(0, 7)}`);
        setCommitMessage('');
        clearSelection();
        // Refresh status to show updated state
        await refreshStatus(repo.repoId);
      } else {
        toast.error(response.message || 'Failed to commit');
      }
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Unknown error');
    } finally {
      setIsCommitting(false);
      setIsOperating(false);
    }
  };

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="px-4 py-2 bg-gray-50 dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700 flex-shrink-0">
        <h3 className="text-sm font-semibold text-gray-700 dark:text-gray-300">
          Staged Changes ({stagedEntries.length})
        </h3>
      </div>

      {/* Staged Changes List - Fixed height area */}
      <div className="flex-1 overflow-hidden border-b border-gray-200 dark:border-gray-700" style={{ minHeight: '150px' }}>
        <ChangeList type="staged" />
      </div>

      {/* Commit Box */}
      <div className="p-4 bg-gray-50 dark:bg-gray-900 flex-shrink-0">
        <div className="mb-3">
          <label
            htmlFor="commit-message"
            className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
          >
            Commit Message
          </label>
          <textarea
            id="commit-message"
            className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm focus:ring-blue-500 focus:border-blue-500 dark:bg-gray-800 dark:text-gray-100 font-mono text-sm resize-none"
            rows={4}
            placeholder="Enter commit message..."
            value={commitMessage}
            onChange={(e) => setCommitMessage(e.target.value)}
            disabled={isCommitting}
          />
        </div>

        <div className="flex items-center justify-between">
          <div className="text-xs text-gray-500 dark:text-gray-400">
            {stagedEntries.length} file(s) staged
          </div>
          <button
            onClick={handleCommit}
            disabled={isCommitting || !commitMessage.trim() || !stagedEntries.length}
            className="px-4 py-2 bg-blue-500 text-white rounded-md hover:bg-blue-600 disabled:opacity-50 disabled:cursor-not-allowed text-sm font-medium"
          >
            {isCommitting ? 'Committing...' : 'Commit'}
          </button>
        </div>

        {/* Quick stats */}
        {repo?.head && (
          <div className="mt-3 pt-3 border-t border-gray-200 dark:border-gray-700">
            <div className="text-xs text-gray-600 dark:text-gray-400 space-y-1">
              <div className="flex items-center justify-between">
                <span>Branch:</span>
                <span className="font-mono font-medium">
                  {repo.head.branch || 'detached HEAD'}
                </span>
              </div>
              <div className="flex items-center justify-between">
                <span>Last commit:</span>
                <span className="font-mono">
                  {repo.head.commit.substring(0, 7)}
                </span>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
