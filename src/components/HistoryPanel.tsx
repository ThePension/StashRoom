import { useEffect, useRef } from 'react';
import { Virtuoso } from 'react-virtuoso';
import { useStore } from '../state/store';
import type { CommitSummary, CommitFileDiff } from '../lib/types';

export function HistoryPanel() {
  const getActiveRepo = useStore((s) => s.getActiveRepo);
  const repo = getActiveRepo();
  const commits = useStore((s) => s.commits);
  const hasMore = useStore((s) => s.hasMore);
  const isLoading = useStore((s) => s.isLoading);
  const isLoadingCommitDiff = useStore((s) => s.isLoadingCommitDiff);
  const selectedCommit = useStore((s) => s.selectedCommit);
  const commitDiff = useStore((s) => s.commitDiff);
  const selectedCommitFile = useStore((s) => s.selectedCommitFile);
  const loadHistory = useStore((s) => s.loadHistory);
  const loadMore = useStore((s) => s.loadMore);
  const selectCommit = useStore((s) => s.selectCommit);
  const selectCommitFile = useStore((s) => s.selectCommitFile);
  const compactMode = useStore((s) => s.settings.compactMode);

  const virtuosoRef = useRef<any>(null);

  // Load history when repo opens
  useEffect(() => {
    if (repo) {
      loadHistory(repo.repoId);
    }
  }, [repo, loadHistory]);

  // Scroll to selected commit when it changes
  useEffect(() => {
    if (!selectedCommit || !virtuosoRef.current || commits.length === 0) {
      return;
    }

    const scrollToCommit = async () => {
      let index = commits.findIndex((c) => c.oid === selectedCommit.oid);

      // If commit not found and there are more commits to load, keep loading
      while (index === -1 && hasMore && !isLoading && repo) {
        await loadMore(repo.repoId);
        // After loading, check again
        const updatedCommits = useStore.getState().commits;
        index = updatedCommits.findIndex((c) => c.oid === selectedCommit.oid);
      }

      // Now scroll if we found the commit
      if (index !== -1 && virtuosoRef.current) {
        // Use setTimeout to ensure the DOM has updated
        setTimeout(() => {
          virtuosoRef.current?.scrollToIndex({
            index,
            align: 'center',
            behavior: 'smooth',
          });
        }, 100);
      }
    };

    scrollToCommit();
  }, [selectedCommit?.oid]); // Only depend on oid to avoid infinite loops

  const handleSelectCommit = (commit: CommitSummary) => {
    if (!repo) return;
    selectCommit(repo.repoId, commit);
  };

  const handleLoadMore = () => {
    if (!repo || !hasMore || isLoading) return;
    loadMore(repo.repoId);
  };

  const handleSelectFile = (file: CommitFileDiff, e: React.MouseEvent) => {
    e.stopPropagation();
    selectCommitFile(file);
  };

  const getChangeIcon = (change: string) => {
    // Handle both capitalized and lowercase variants
    const changeType = change.toLowerCase();
    switch (changeType) {
      case 'added': return { icon: '+', color: 'text-green-600 dark:text-green-400' };
      case 'deleted': return { icon: '-', color: 'text-red-600 dark:text-red-400' };
      case 'modified': return { icon: 'M', color: 'text-blue-600 dark:text-blue-400' };
      case 'renamed': return { icon: 'R', color: 'text-purple-600 dark:text-purple-400' };
      case 'copied': return { icon: 'C', color: 'text-cyan-600 dark:text-cyan-400' };
      default: return { icon: '?', color: 'text-gray-600 dark:text-gray-400' };
    }
  };

  const formatTime = (timestamp: number) => {
    const date = new Date(timestamp * 1000);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMs / 3600000);
    const diffDays = Math.floor(diffMs / 86400000);

    if (diffMins < 60) {
      return `${diffMins} minute${diffMins !== 1 ? 's' : ''} ago`;
    } else if (diffHours < 24) {
      return `${diffHours} hour${diffHours !== 1 ? 's' : ''} ago`;
    } else if (diffDays < 7) {
      return `${diffDays} day${diffDays !== 1 ? 's' : ''} ago`;
    } else {
      return date.toLocaleDateString();
    }
  };

  if (!repo) {
    return (
      <div className="h-full flex items-center justify-center text-gray-400 dark:text-gray-500">
        <p className="text-sm">No repository open</p>
      </div>
    );
  }

  if (commits.length === 0 && !isLoading) {
    return (
      <div className="h-full flex items-center justify-center text-gray-400 dark:text-gray-500">
        <p className="text-sm">No commits found</p>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col bg-white dark:bg-gray-900">
      {isLoading && commits.length === 0 ? (
        <div className="flex-1 flex items-center justify-center">
          <div className="text-sm text-gray-500 dark:text-gray-400">Loading commits...</div>
        </div>
      ) : (
        <Virtuoso
          ref={virtuosoRef}
          style={{ height: '100%' }}
          totalCount={commits.length}
          endReached={handleLoadMore}
          itemContent={(index) => {
            const commit = commits[index];
            const isSelected = selectedCommit?.oid === commit.oid;

            return (
              <div>
                {/* Commit Header */}
                <div
                  onClick={() => handleSelectCommit(commit)}
                  className={`${compactMode ? 'px-2 py-1' : 'px-3 py-2'} border-b border-gray-200 dark:border-gray-700 transition-colors cursor-pointer hover:bg-gray-50 dark:hover:bg-gray-800 ${
                    isSelected ? 'bg-blue-50 dark:bg-blue-900/20 border-l-4 border-l-blue-500' : ''
                  }`}
                >
                  <div className="flex items-start gap-2">
                    <div className="flex-1 min-w-0">
                      {/* Subject */}
                      <div className={`${compactMode ? 'text-xs' : 'text-sm'} font-medium text-gray-900 dark:text-gray-100 truncate`}>
                        {commit.subject}
                      </div>

                      {/* Author & Time */}
                      <div className={`flex items-center gap-2 ${compactMode ? 'mt-0.5' : 'mt-1'} ${compactMode ? 'text-[10px]' : 'text-xs'} text-gray-500 dark:text-gray-400`}>
                        <span className="truncate">{commit.author.name}</span>
                        <span>•</span>
                        <span>{formatTime(commit.time)}</span>
                      </div>

                      {/* Refs (branches, tags) */}
                      {commit.refs && commit.refs.length > 0 && (
                        <div className="flex flex-wrap gap-1 mt-1">
                          {commit.refs.map((ref, i) => (
                            <span
                              key={i}
                              className="text-xs px-1.5 py-0.5 rounded bg-green-100 dark:bg-green-900 text-green-700 dark:text-green-300 font-mono"
                            >
                              {ref}
                            </span>
                          ))}
                        </div>
                      )}
                    </div>

                    {/* Short OID */}
                    <div className={`${compactMode ? 'text-[10px]' : 'text-xs'} font-mono text-gray-400 dark:text-gray-500 flex-shrink-0`}>
                      {commit.shortOid}
                    </div>
                  </div>
                </div>

                {/* File List or Loading (shown when commit is selected) */}
                {isSelected && (
                  <div className="bg-gray-50 dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700">
                    {isLoadingCommitDiff ? (
                      // Loading spinner
                      <div className={`${compactMode ? 'px-3 py-2' : 'px-6 py-4'} flex items-center justify-center gap-2`}>
                        <div className="animate-spin rounded-full h-4 w-4 border-2 border-blue-500 border-t-transparent"></div>
                        <span className={`${compactMode ? 'text-[10px]' : 'text-xs'} text-gray-500 dark:text-gray-400`}>Loading files...</span>
                      </div>
                    ) : commitDiff && commitDiff.length > 0 ? (
                      // File list
                      commitDiff.map((file, fileIndex) => {
                        const { icon, color } = getChangeIcon(file.change);
                        const isFileSelected = selectedCommitFile?.path === file.path;

                        return (
                          <div
                            key={fileIndex}
                            onClick={(e) => handleSelectFile(file, e)}
                            className={`${compactMode ? 'px-3 py-0.5' : 'px-6 py-1.5'} cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors flex items-center gap-2 ${
                              isFileSelected ? 'bg-gray-200 dark:bg-gray-800' : ''
                            }`}
                          >
                            <span className={`${compactMode ? 'text-[10px]' : 'text-xs'} font-bold w-4 ${color}`}>{icon}</span>
                            <span className={`${compactMode ? 'text-[10px]' : 'text-xs'} font-mono text-gray-700 dark:text-gray-300 truncate`}>
                              {file.path}
                            </span>
                            {file.isBinary && (
                              <span className={`${compactMode ? 'text-[10px]' : 'text-xs'} text-gray-500 dark:text-gray-400 italic`}>binary</span>
                            )}
                          </div>
                        );
                      })
                    ) : (
                      // No files
                      <div className={`${compactMode ? 'px-3 py-1' : 'px-6 py-2'} ${compactMode ? 'text-[10px]' : 'text-xs'} text-gray-500 dark:text-gray-400 italic`}>
                        No files changed
                      </div>
                    )}
                  </div>
                )}
              </div>
            );
          }}
          components={{
            Footer: () => {
              if (hasMore && !isLoading) {
                return (
                  <div className="p-4 text-center">
                    <button
                      onClick={handleLoadMore}
                      className="text-sm text-blue-500 hover:text-blue-600 dark:text-blue-400 dark:hover:text-blue-300"
                    >
                      Load more...
                    </button>
                  </div>
                );
              }
              if (isLoading && commits.length > 0) {
                return (
                  <div className="p-4 text-center text-sm text-gray-500 dark:text-gray-400">
                    Loading...
                  </div>
                );
              }
              return null;
            },
          }}
        />
      )}
    </div>
  );
}
