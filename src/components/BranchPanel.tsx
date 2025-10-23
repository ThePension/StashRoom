import { useEffect, useState } from 'react';
import { useStore } from '../state/store';
import { api } from '../lib/api';
import { toast } from 'sonner';
import type { BranchInfo } from '../lib/types';

export function BranchPanel() {
  const getActiveRepo = useStore((s) => s.getActiveRepo);
  const repo = getActiveRepo();
  const compactMode = useStore((s) => s.settings.compactMode);
  const [branches, setBranches] = useState<BranchInfo[]>([]);
  const [currentBranch, setCurrentBranch] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isSwitching, setIsSwitching] = useState(false);

  useEffect(() => {
    if (repo) {
      loadBranches();
    }
  }, [repo?.repoId]);

  const loadBranches = async () => {
    if (!repo) return;

    setIsLoading(true);
    try {
      const response = await api.listBranches(repo.repoId);
      if (response.ok && response.data) {
        setBranches(response.data.locals);
        setCurrentBranch(response.data.current);
      } else {
        toast.error(response.message || 'Failed to load branches');
      }
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Unknown error');
    } finally {
      setIsLoading(false);
    }
  };

  const handleSwitchBranch = async (branch: BranchInfo) => {
    if (!repo || branch.isHead || isSwitching) return;

    setIsSwitching(true);
    try {
      const response = await api.switchBranch({
        repoId: repo.repoId,
        name: branch.name,
      });

      if (response.ok && response.data) {
        toast.success(`Switched to branch: ${response.data.name}`);
        setCurrentBranch(response.data.name);

        // Update HEAD info (this updates the header and commit form)
        await useStore.getState().updateHeadInfo(repo.repoId);

        // Refresh status and clear current selection
        await useStore.getState().refreshStatus(repo.repoId);
        useStore.getState().clearSelection();
        useStore.getState().clearHistory();

        // Reload history for new branch
        await useStore.getState().loadHistory(repo.repoId);

        // Update branch list
        await loadBranches();
      } else {
        if (response.code === 'UNCOMMITTED_CHANGES') {
          toast.error('You have uncommitted changes. Commit or stash before switching.');
        } else {
          toast.error(response.message || 'Failed to switch branch');
        }
      }
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Unknown error');
    } finally {
      setIsSwitching(false);
    }
  };

  if (!repo) return null;

  return (
    <div className="h-full flex flex-col border-l border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900">
      {/* Header */}
      <div className={`${compactMode ? 'px-2 py-1.5' : 'px-4 py-3'} border-b border-gray-200 dark:border-gray-700`}>
        <div className="flex items-center justify-between gap-2">
          <h3 className={`${compactMode ? 'text-xs' : 'text-sm'} font-semibold text-gray-900 dark:text-gray-100`}>
            Branches
          </h3>
          {currentBranch && (
            <span
              className={`${compactMode ? 'text-[10px] px-1.5 py-0' : 'text-xs px-2 py-0.5'} rounded-full bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300 font-mono max-w-[150px] truncate`}
              title={currentBranch}
            >
              {currentBranch}
            </span>
          )}
        </div>
      </div>

      {/* Branch List */}
      <div className="flex-1 overflow-y-auto">
        {isLoading ? (
          <div className="flex items-center justify-center py-8">
            <div className="text-sm text-gray-500 dark:text-gray-400">Loading...</div>
          </div>
        ) : branches.length === 0 ? (
          <div className="flex items-center justify-center py-8">
            <div className="text-sm text-gray-500 dark:text-gray-400">No branches</div>
          </div>
        ) : (
          <div>
            {branches.map((branch) => (
              <div
                key={branch.name}
                onClick={() => !branch.isHead && !isSwitching && handleSwitchBranch(branch)}
                className={`${compactMode ? 'px-2 py-1' : 'px-4 py-2'} transition-colors ${
                  branch.isHead
                    ? 'bg-gray-50 dark:bg-gray-800 cursor-default'
                    : 'cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-800'
                } ${
                  isSwitching ? 'cursor-wait opacity-50' : ''
                }`}
                title={branch.name}
              >
                <div className="flex items-center justify-between gap-2">
                  <span className={`${compactMode ? 'text-xs' : 'text-sm'} font-mono text-gray-700 dark:text-gray-300 truncate`}>
                    {branch.name}
                  </span>
                  {branch.isHead && (
                    <span className={`${compactMode ? 'text-[10px] px-1 py-0' : 'text-xs px-1.5 py-0.5'} rounded bg-green-100 dark:bg-green-900 text-green-700 dark:text-green-300 flex-shrink-0`}>
                      current
                    </span>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
