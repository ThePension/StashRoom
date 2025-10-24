import { useEffect, useRef, useMemo, useState } from 'react';
import { Virtuoso } from 'react-virtuoso';
import { ChevronRight, ChevronDown, Folder, File, MoveRight, MoveLeft, X } from 'lucide-react';
import { useStore } from '../state/store';
import { api } from '../lib/api';
import { toast } from 'sonner';
import { useConfirm } from '../hooks/useConfirm';
import { buildTree, flattenTree } from '../lib/treeUtils';
import type { StatusEntry } from '../lib/types';
import type { FlatTreeNode } from '../lib/treeUtils';

interface ChangeListProps {
  type: 'unstaged' | 'staged';
}

export function ChangeList({ type }: ChangeListProps) {
  const getActiveRepo = useStore((s) => s.getActiveRepo);
  const repo = getActiveRepo();
  const allEntries = useStore((s) => s.entries);
  const selectedPath = useStore((s) => s.selectedPath);
  const setSelectedPath = useStore((s) => s.setSelectedPath);
  const loadDiff = useStore((s) => s.loadDiff);
  const selectCommitFile = useStore((s) => s.selectCommitFile);
  const isOperating = useStore((s) => s.isOperating);
  const setIsOperating = useStore((s) => s.setIsOperating);
  const compactMode = useStore((s) => s.settings.compactMode);
  const treeViewMode = useStore((s) => s.settings.treeViewMode);
  const { confirm, ConfirmDialog } = useConfirm();

  // Tree view state
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set());
  const [selectedTreeIndex, setSelectedTreeIndex] = useState<number>(0);
  const [hoveredFolderPath, setHoveredFolderPath] = useState<string | null>(null);

  // Filter entries based on type (memoized to prevent infinite loops)
  const entries = useMemo(() => {
    return type === 'unstaged'
      ? allEntries.filter((e) => e.unstagedStatus !== null || e.untracked)
      : allEntries.filter((e) => e.stagedStatus !== null);
  }, [allEntries, type]);

  // Build tree structure
  const treeRoot = useMemo(() => {
    if (treeViewMode === 'flat') return null;
    return buildTree(entries);
  }, [entries, treeViewMode]);

  // Flatten tree for virtualization
  const flattenedTree = useMemo(() => {
    if (!treeRoot) return [];
    return flattenTree(treeRoot, expandedPaths);
  }, [treeRoot, expandedPaths]);

  // Determine which list to use
  const itemsList = treeViewMode === 'tree' ? flattenedTree : entries;

  const virtuosoRef = useRef<any>(null);
  const selectedIndex = useMemo(
    () => entries.findIndex((e) => e.path === selectedPath),
    [entries, selectedPath]
  );

  const confirmAndDiscard = async (entry: StatusEntry) => {
    const confirmed = await confirm({
      title: 'Discard Changes',
      message: `Are you sure you want to discard changes to "${entry.path}"? This cannot be undone (but a backup will be created).`,
      confirmText: 'Discard',
      cancelText: 'Cancel',
      variant: 'danger',
    });

    if (confirmed) {
      handleDiscard(entry);
    }
  };

  const confirmAndDelete = async (entry: StatusEntry) => {
    const confirmed = await confirm({
      title: 'Delete File',
      message: `Are you sure you want to delete "${entry.path}"? This file will be permanently removed from the filesystem.`,
      confirmText: 'Delete',
      cancelText: 'Cancel',
      variant: 'danger',
    });

    if (confirmed) {
      handleDelete(entry);
    }
  };

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (!entries.length || !document.activeElement?.closest('.change-list')) return;

      if (treeViewMode === 'tree') {
        // Tree mode navigation
        switch (e.key) {
          case 'ArrowDown':
            e.preventDefault();
            if (selectedTreeIndex < flattenedTree.length - 1) {
              const newIndex = selectedTreeIndex + 1;
              setSelectedTreeIndex(newIndex);
              const nextNode = flattenedTree[newIndex];
              if (nextNode.type === 'file') {
                setSelectedPath(nextNode.path);
              }
              virtuosoRef.current?.scrollToIndex({ index: newIndex, behavior: 'smooth' });
            }
            break;

          case 'ArrowUp':
            e.preventDefault();
            if (selectedTreeIndex > 0) {
              const newIndex = selectedTreeIndex - 1;
              setSelectedTreeIndex(newIndex);
              const prevNode = flattenedTree[newIndex];
              if (prevNode.type === 'file') {
                setSelectedPath(prevNode.path);
              }
              virtuosoRef.current?.scrollToIndex({ index: newIndex, behavior: 'smooth' });
            }
            break;

          case 'Enter':
            e.preventDefault();
            const currentNode = flattenedTree[selectedTreeIndex];
            if (currentNode.type === 'folder') {
              toggleFolder(currentNode.path);
            } else if (currentNode.type === 'file' && repo) {
              loadDiff(repo.repoId, currentNode.path, type === 'unstaged' ? 'working' : 'index');
            }
            break;

          case 's':
          case 'S':
            e.preventDefault();
            const nodeForStage = flattenedTree[selectedTreeIndex];
            if (nodeForStage.type === 'file' && repo) {
              handleStageToggle(nodeForStage.entry!);
            }
            break;

          case 'd':
          case 'D':
            e.preventDefault();
            const nodeForDiscard = flattenedTree[selectedTreeIndex];
            if (nodeForDiscard.type === 'file' && type === 'unstaged' && repo) {
              nodeForDiscard.entry!.untracked
                ? confirmAndDelete(nodeForDiscard.entry!)
                : confirmAndDiscard(nodeForDiscard.entry!);
            }
            break;
        }
      } else {
        // Flat mode navigation (original)
        switch (e.key) {
          case 'ArrowDown':
            e.preventDefault();
            if (selectedIndex < entries.length - 1) {
              const nextEntry = entries[selectedIndex + 1];
              setSelectedPath(nextEntry.path);
              virtuosoRef.current?.scrollToIndex({ index: selectedIndex + 1, behavior: 'smooth' });
            }
            break;

          case 'ArrowUp':
            e.preventDefault();
            if (selectedIndex > 0) {
              const prevEntry = entries[selectedIndex - 1];
              setSelectedPath(prevEntry.path);
              virtuosoRef.current?.scrollToIndex({ index: selectedIndex - 1, behavior: 'smooth' });
            }
            break;

          case 'Enter':
            e.preventDefault();
            if (selectedPath && repo) {
              loadDiff(repo.repoId, selectedPath, type === 'unstaged' ? 'working' : 'index');
            }
            break;

          case 's':
          case 'S':
            e.preventDefault();
            if (selectedPath && repo) {
              handleStageToggle(entries[selectedIndex]);
            }
            break;

          case 'd':
          case 'D':
            e.preventDefault();
            if (selectedPath && repo && type === 'unstaged') {
              const entry = entries[selectedIndex];
              entry.untracked ? confirmAndDelete(entry) : confirmAndDiscard(entry);
            }
            break;
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [entries, selectedIndex, selectedPath, repo, type, treeViewMode, flattenedTree, selectedTreeIndex]);

  const handleSelect = (entry: StatusEntry) => {
    // Clear commit file selection when switching to working tree changes
    selectCommitFile(null);
    setSelectedPath(entry.path);
    if (repo) {
      loadDiff(repo.repoId, entry.path, type === 'unstaged' ? 'working' : 'index');
    }
  };

  const handleStageToggle = async (entry: StatusEntry) => {
    if (!repo) return;

    const currentIndex = entries.findIndex(e => e.path === entry.path);

    setIsOperating(true);
    try {
      const response =
        type === 'unstaged'
          ? await api.stageFile(repo.repoId, entry.path)
          : await api.unstageFile(repo.repoId, entry.path);

      if (response.ok && response.data) {
        // Calculate next file to select
        let nextPath: string | null = null;
        let nextSide: 'working' | 'index' = 'working';

        if (type === 'unstaged') {
          // When staging: select the next unstaged file (or previous if last)
          const newUnstagedEntries = response.data.entries.filter(
            e => e.unstagedStatus !== null || e.untracked
          );

          if (newUnstagedEntries.length > 0) {
            const nextIndex = Math.min(currentIndex, newUnstagedEntries.length - 1);
            nextPath = newUnstagedEntries[nextIndex].path;
            nextSide = 'working';
          }
        } else {
          // When unstaging: select the next staged file (or previous if last)
          const newStagedEntries = response.data.entries.filter(
            e => e.stagedStatus !== null
          );

          if (newStagedEntries.length > 0) {
            const nextIndex = Math.min(currentIndex, newStagedEntries.length - 1);
            nextPath = newStagedEntries[nextIndex].path;
            nextSide = 'index';
          }
        }

        // Clear current diff immediately to prevent showing wrong file
        useStore.getState().clearDiff();

        // Update status
        useStore.getState().updateStatus(response.data);

        // Now select and load the next file
        if (nextPath) {
          setSelectedPath(nextPath);
          loadDiff(repo.repoId, nextPath, nextSide);
        } else {
          setSelectedPath(null);
        }

        toast.success(
          type === 'unstaged'
            ? `Staged ${entry.path}`
            : `Unstaged ${entry.path}`
        );
      } else {
        toast.error(response.message || 'Failed to toggle stage');
      }
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Unknown error');
    } finally {
      setIsOperating(false);
    }
  };

  const handleStageDir = async (dir: string, includeUntracked: boolean) => {
    if (!repo) return;

    setIsOperating(true);
    try {
      const response = await api.stageDir(repo.repoId, dir, includeUntracked);

      if (response.ok && response.data) {
        // Update status
        useStore.getState().updateStatus(response.data);

        const stats = treeRoot?.children?.find(n => n.path === dir)?.stats;
        const count = stats ? stats.modified + stats.added + stats.deleted : 0;

        toast.success(`Staged ${count} change(s) in ${dir}/`, {
          duration: 3000,
        });
      } else {
        toast.error(response.message || 'Failed to stage directory');
      }
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Unknown error');
    } finally {
      setIsOperating(false);
    }
  };

  const handleUnstageDir = async (dir: string) => {
    if (!repo) return;

    setIsOperating(true);
    try {
      const response = await api.unstageDir(repo.repoId, dir);

      if (response.ok && response.data) {
        // Update status
        useStore.getState().updateStatus(response.data);

        toast.success(`Unstaged changes in ${dir}/`, {
          duration: 3000,
        });
      } else {
        toast.error(response.message || 'Failed to unstage directory');
      }
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Unknown error');
    } finally {
      setIsOperating(false);
    }
  };

  const handleDiscard = async (entry: StatusEntry) => {
    if (!repo) return;

    const discardedPath = entry.path;
    setIsOperating(true);
    try {
      const response = await api.discard({
        repoId: repo.repoId,
        path: discardedPath,
        hunks: null,
      });

      if (response.ok && response.data) {
        const { status, backupTimestamp } = response.data;

        // Update the status first
        useStore.getState().updateStatus(status);

        // Check if the file still has unstaged changes in the updated status
        const fileStillInUnstaged = status.entries.some(
          e => e.path === discardedPath && (e.unstagedStatus !== null || e.untracked)
        );

        // Clear selection and diff if file no longer has unstaged changes
        if (!fileStillInUnstaged && selectedPath === discardedPath) {
          setSelectedPath(null);
          useStore.getState().clearDiff();
        }

        // Show success toast with undo option if backup was created
        if (backupTimestamp) {
          toast.success(`Discarded changes to ${discardedPath}`, {
            action: {
              label: 'Undo',
              onClick: async () => {
                try {
                  const restoreResponse = await api.restoreFromBackup(
                    repo.repoId,
                    backupTimestamp,
                    discardedPath
                  );
                  if (restoreResponse.ok && restoreResponse.data) {
                    useStore.getState().updateStatus(restoreResponse.data);
                    await useStore.getState().refreshStatus(repo.repoId, true);
                    toast.success(`Restored ${discardedPath}`);
                  }
                } catch (error) {
                  toast.error('Failed to restore file');
                }
              },
            },
          });
        } else {
          toast.success(`Discarded changes to ${discardedPath}`);
        }
      } else {
        toast.error(response.message || 'Failed to discard changes');
      }
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Unknown error');
    } finally {
      setIsOperating(false);
    }
  };

  const handleDelete = async (entry: StatusEntry) => {
    if (!repo) return;

    setIsOperating(true);
    try {
      const response = await api.deleteFile(repo.repoId, entry.path);

      if (response.ok && response.data) {
        useStore.getState().updateStatus(response.data);
        useStore.getState().clearDiff();

        // Clear selected path if the file is no longer in the entries
        const fileStillExists = response.data.entries.some(e => e.path === entry.path);
        if (!fileStillExists) {
          setSelectedPath(null);
        }

        toast.success(`Deleted ${entry.path}`);
      } else {
        toast.error(response.message || 'Failed to delete file');
      }
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Unknown error');
    } finally {
      setIsOperating(false);
    }
  };

  const toggleFolder = (folderPath: string) => {
    setExpandedPaths((prev) => {
      const newSet = new Set(prev);
      if (newSet.has(folderPath)) {
        newSet.delete(folderPath);
      } else {
        newSet.add(folderPath);
      }
      return newSet;
    });
  };

  const getStatusIcon = (entry: StatusEntry) => {
    if (entry.untracked) return <span className="text-green-500">+</span>;
    if (type === 'unstaged' && entry.unstagedStatus === 'deleted')
      return <span className="text-red-500">-</span>;
    if (type === 'staged' && entry.stagedStatus === 'deleted')
      return <span className="text-red-500">-</span>;
    if (type === 'unstaged' && entry.unstagedStatus === 'modified')
      return <span className="text-yellow-500">~</span>;
    if (type === 'staged' && entry.stagedStatus === 'modified')
      return <span className="text-yellow-500">~</span>;
    if (type === 'staged' && entry.stagedStatus === 'added')
      return <span className="text-green-500">+</span>;
    return <span className="text-gray-500">?</span>;
  };

  if (!entries.length) {
    return (
      <div className="flex-1 flex items-center justify-center text-gray-400 text-sm">
        No {type} changes
      </div>
    );
  }

  return (
    <>
      <ConfirmDialog />
      <div className="h-full change-list bg-white dark:bg-gray-900 overflow-x-auto" tabIndex={0}>
        <Virtuoso
        ref={virtuosoRef}
        style={{ height: '100%', minWidth: 'max-content' }}
        totalCount={itemsList.length}
        itemContent={(index) => {
          const item = itemsList[index];

          // Tree mode - could be folder or file
          if (treeViewMode === 'tree') {
            const node = item as FlatTreeNode;

            if (node.type === 'folder') {
              const isExpanded = node.isExpanded ?? false;
              const stats = node.stats!;
              const hasChanges = stats.modified + stats.added + stats.deleted > 0;
              const isHovered = hoveredFolderPath === node.path;

              return (
                <div
                  className={`
                    ${compactMode ? 'px-2 py-0.5' : 'px-3 py-1.5'} cursor-pointer select-none ${compactMode ? 'text-xs' : 'text-sm'}
                    text-gray-700 dark:text-gray-300
                    hover:bg-gray-100 dark:hover:bg-gray-800
                    whitespace-nowrap
                    relative
                  `}
                  style={{ paddingLeft: `${(node.depth + 1) * 16 + 12}px` }}
                  onMouseEnter={() => setHoveredFolderPath(node.path)}
                  onMouseLeave={() => setHoveredFolderPath(null)}
                >
                  <div className="flex items-center gap-2 flex-1" onClick={() => toggleFolder(node.path)}>
                    {isExpanded ? (
                      <ChevronDown className="w-3 h-3 flex-shrink-0" />
                    ) : (
                      <ChevronRight className="w-3 h-3 flex-shrink-0" />
                    )}
                    <Folder className="w-3.5 h-3.5 text-blue-500 flex-shrink-0" />
                    <span className="font-medium">{node.name}</span>
                    {hasChanges && (
                      <span className="text-xs text-gray-500 dark:text-gray-400">
                        ({stats.added > 0 && `+${stats.added} `}
                        {stats.modified > 0 && `~${stats.modified} `}
                        {stats.deleted > 0 && `-${stats.deleted}`})
                      </span>
                    )}
                  </div>
                  {/* Folder actions - shown on hover */}
                  {hasChanges && (
                    <div
                      className={`absolute right-2 top-1/2 -translate-y-1/2 flex gap-1 z-50 transition-opacity ${isHovered ? 'opacity-100' : 'opacity-0 pointer-events-none'}`}
                    >
                      {type === 'unstaged' && (
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            handleStageDir(node.path, true);
                          }}
                          disabled={isOperating}
                          className="px-2 py-1 text-xs rounded bg-blue-500 text-white hover:bg-blue-600 flex-shrink-0 disabled:opacity-50 disabled:cursor-not-allowed"
                          title="Stage directory (including untracked files)"
                        >
                          <MoveRight className="w-3 h-3" />
                        </button>
                      )}
                      {type === 'staged' && (
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            handleUnstageDir(node.path);
                          }}
                          disabled={isOperating}
                          className="px-2 py-1 text-xs rounded bg-blue-500 text-white hover:bg-blue-600 flex-shrink-0 disabled:opacity-50 disabled:cursor-not-allowed"
                          title="Unstage directory"
                        >
                          <MoveLeft className="w-3 h-3" />
                        </button>
                      )}
                    </div>
                  )}
                </div>
              );
            } else {
              // File node
              const entry = node.entry!;
              const isSelected = entry.path === selectedPath;

              return (
                <div
                  className={`
                    ${compactMode ? 'px-2 py-0.5' : 'px-3 py-1.5'} cursor-pointer select-none ${compactMode ? 'text-xs' : 'text-sm'} font-mono
                    text-gray-700 dark:text-gray-300
                    hover:bg-gray-100 dark:hover:bg-gray-800
                    ${isSelected ? 'bg-blue-50 dark:bg-blue-900/20 border-l-2 border-blue-500' : ''}
                    group
                    whitespace-nowrap
                  `}
                  style={{ paddingLeft: `${(node.depth + 1) * 16 + 12}px` }}
                  onClick={() => handleSelect(entry)}
                >
                  <div className="flex items-center justify-between gap-2">
                    <div className="flex items-center gap-2">
                      <File className="w-3.5 h-3.5 text-gray-400 flex-shrink-0" />
                      <span className="w-4 text-center">{getStatusIcon(entry)}</span>
                      <span>{node.name}</span>
                    </div>
                    <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          handleStageToggle(entry);
                        }}
                        className="px-2 py-1 text-xs rounded bg-blue-500 text-white hover:bg-blue-600 flex-shrink-0"
                        title={type === 'unstaged' ? 'Stage file' : 'Unstage file'}
                      >
                        {type === 'unstaged' ? <MoveRight className="w-3 h-3" /> : <MoveLeft className="w-3 h-3" />}
                      </button>
                      {type === 'unstaged' && (
                        <button
                          onClick={(e) => {
                            e.preventDefault();
                            e.stopPropagation();
                            entry.untracked ? confirmAndDelete(entry) : confirmAndDiscard(entry);
                          }}
                          className="px-2 py-1 text-xs rounded bg-red-500 text-white hover:bg-red-600 flex-shrink-0"
                          title={entry.untracked ? 'Delete file' : 'Discard changes'}
                        >
                          <X className="w-3 h-3" />
                        </button>
                      )}
                    </div>
                  </div>
                </div>
              );
            }
          }

          // Flat mode
          const entry = item as StatusEntry;
          const isSelected = entry.path === selectedPath;

          return (
            <div
              className={`
                ${compactMode ? 'px-2 py-0.5' : 'px-3 py-1.5'} cursor-pointer select-none ${compactMode ? 'text-xs' : 'text-sm'} font-mono
                text-gray-700 dark:text-gray-300
                hover:bg-gray-100 dark:hover:bg-gray-800
                ${isSelected ? 'bg-blue-50 dark:bg-blue-900/20 border-l-2 border-blue-500' : ''}
                group
              `}
              onClick={() => handleSelect(entry)}
            >
              <div className="flex items-center justify-between gap-2">
                <div className="flex items-center gap-2 flex-1 min-w-0">
                  <span className="w-4 text-center">{getStatusIcon(entry)}</span>
                  <span className="truncate">{entry.path}</span>
                </div>
                <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      handleStageToggle(entry);
                    }}
                    className="px-2 py-1 text-xs rounded bg-blue-500 text-white hover:bg-blue-600 flex-shrink-0"
                    title={type === 'unstaged' ? 'Stage file' : 'Unstage file'}
                  >
                    {type === 'unstaged' ? <MoveRight className="w-3 h-3" /> : <MoveLeft className="w-3 h-3" />}
                  </button>
                  {type === 'unstaged' && (
                    <button
                      onClick={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        entry.untracked ? confirmAndDelete(entry) : confirmAndDiscard(entry);
                      }}
                      className="px-2 py-1 text-xs rounded bg-red-500 text-white hover:bg-red-600 flex-shrink-0"
                      title={entry.untracked ? 'Delete file' : 'Discard changes'}
                    >
                      <X className="w-3 h-3" />
                    </button>
                  )}
                </div>
              </div>
            </div>
          );
        }}
      />
      </div>
    </>
  );
}
