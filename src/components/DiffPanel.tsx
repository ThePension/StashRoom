import { useEffect, useState } from 'react';
import { useStore } from '../state/store';
import { api } from '../lib/api';
import { toast } from 'sonner';

export function DiffPanel() {
  const repo = useStore((s) => s.repo);
  const currentDiff = useStore((s) => s.currentDiff);
  const currentDiffSide = useStore((s) => s.currentDiffSide);
  const selectedPath = useStore((s) => s.selectedPath);
  const selectedCommitFile = useStore((s) => s.selectedCommitFile);
  const setIsOperating = useStore((s) => s.setIsOperating);
  const entries = useStore((s) => s.entries);
  const showLineNumbers = useStore((s) => s.settings.showLineNumbers);
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    hunkIndex: number;
  } | null>(null);

  // Line selection state: Map of hunkIndex -> Set of lineIndices
  const [selectedLines, setSelectedLines] = useState<Map<number, Set<number>>>(new Map());
  // Track last clicked line for shift-select
  const [lastClickedLine, setLastClickedLine] = useState<{ hunkIndex: number; lineIndex: number } | null>(null);
  // Track drag state
  const [isDragging, setIsDragging] = useState(false);
  const [dragStart, setDragStart] = useState<{ hunkIndex: number; lineIndex: number } | null>(null);
  // Track line wrap preference
  const [wrapLines, setWrapLines] = useState(false);

  // Determine if we're viewing staged changes based on which diff side we loaded
  const isViewingStaged = currentDiffSide === 'index';

  // Check if the current file is a newly added file (for staged view)
  const currentEntry = entries.find(e => e.path === selectedPath);
  const isStagedNewFile = isViewingStaged && currentEntry?.stagedStatus === 'added';

  // Clear selection when diff changes
  useEffect(() => {
    setSelectedLines(new Map());
    setLastClickedLine(null);
    setIsDragging(false);
    setDragStart(null);
  }, [currentDiff]);

  // Handle mouse up globally to end drag
  useEffect(() => {
    const handleMouseUp = () => {
      setIsDragging(false);
      setDragStart(null);
    };
    window.addEventListener('mouseup', handleMouseUp);
    return () => window.removeEventListener('mouseup', handleMouseUp);
  }, []);

  useEffect(() => {
    const handleClick = () => setContextMenu(null);
    window.addEventListener('click', handleClick);
    return () => window.removeEventListener('click', handleClick);
  }, []);

  // Determine which diff to display: commit file diff or working tree diff
  const displayDiff = selectedCommitFile || currentDiff;
  const isCommitDiff = !!selectedCommitFile;

  if (!displayDiff) {
    return (
      <div className="flex-1 flex items-center justify-center text-gray-400">
        <div className="text-center">
          <p className="text-lg mb-2">No file selected</p>
          <p className="text-sm">Select a file from the change list or commit history to view its diff</p>
        </div>
      </div>
    );
  }

  if (displayDiff.isBinary) {
    return (
      <div className="flex-1 flex items-center justify-center text-gray-400">
        <div className="text-center">
          <p className="text-lg mb-2">Binary File</p>
          <p className="text-sm">{displayDiff.path}</p>
        </div>
      </div>
    );
  }

  const handleContextMenu = (e: React.MouseEvent, hunkIndex: number) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY, hunkIndex });
  };

  const handleStageHunk = async (hunkIndex: number, unstage: boolean = false) => {
    if (!repo || !selectedPath) return;

    setIsOperating(true);
    setContextMenu(null);

    try {
      const response = await api.stageHunk({
        repoId: repo.repoId,
        path: selectedPath,
        hunkIndex,
        unstage,
      });

      if (response.ok && response.data) {
        useStore.getState().updateStatus(response.data);
        toast.success(unstage ? 'Hunk unstaged' : 'Hunk staged');

        // After staging/unstaging, check if file still has changes to show
        const updatedEntry = response.data.entries.find((e) => e.path === selectedPath);

        if (updatedEntry) {
          // If we staged and there are still unstaged changes, reload working diff
          if (!unstage && updatedEntry.unstagedStatus) {
            await useStore.getState().loadDiff(repo.repoId, selectedPath, 'working');
          }
          // If we unstaged and file is still staged, reload index diff
          else if (unstage && updatedEntry.stagedStatus) {
            await useStore.getState().loadDiff(repo.repoId, selectedPath, 'index');
          }
          // If file no longer has changes in current view, clear the diff
          else {
            useStore.getState().clearDiff();
          }
        } else {
          // File no longer in status, clear diff
          useStore.getState().clearDiff();
        }
      } else {
        toast.error(response.message || 'Failed to stage hunk');
      }
    } catch (error) {
      console.error('Error staging hunk:', error);
      toast.error(error instanceof Error ? error.message : 'Unknown error');
    } finally {
      setIsOperating(false);
    }
  };

  const handleLineMouseDown = (hunkIndex: number, lineIndex: number, origin: string, e: React.MouseEvent) => {
    // Only allow selecting added lines
    if (origin !== '+') {
      return;
    }

    e.preventDefault();
    setIsDragging(true);
    setDragStart({ hunkIndex, lineIndex });

    setSelectedLines((prev) => {
      const newMap = new Map(prev);
      const hunkLines = newMap.get(hunkIndex) || new Set<number>();

      if (e.shiftKey && lastClickedLine && lastClickedLine.hunkIndex === hunkIndex) {
        // Shift-select: select range from last clicked to current
        const start = Math.min(lastClickedLine.lineIndex, lineIndex);
        const end = Math.max(lastClickedLine.lineIndex, lineIndex);

        const hunk = currentDiff?.hunks[hunkIndex];
        if (hunk) {
          for (let i = start; i <= end; i++) {
            if (hunk.lines[i]?.origin === '+') {
              hunkLines.add(i);
            }
          }
        }
      } else if (e.ctrlKey || e.metaKey) {
        // Ctrl/Cmd-select: toggle line
        if (hunkLines.has(lineIndex)) {
          hunkLines.delete(lineIndex);
        } else {
          hunkLines.add(lineIndex);
        }
      } else {
        // Single select: clear others and select this line
        hunkLines.clear();
        hunkLines.add(lineIndex);
      }

      if (hunkLines.size === 0) {
        newMap.delete(hunkIndex);
      } else {
        newMap.set(hunkIndex, hunkLines);
      }

      return newMap;
    });

    setLastClickedLine({ hunkIndex, lineIndex });
  };

  const handleLineMouseEnter = (hunkIndex: number, lineIndex: number, origin: string) => {
    if (!isDragging || !dragStart || origin !== '+' || dragStart.hunkIndex !== hunkIndex) {
      return;
    }

    // During drag, select range from drag start to current
    setSelectedLines((prev) => {
      const newMap = new Map(prev);
      const hunkLines = newMap.get(hunkIndex) || new Set<number>();

      const start = Math.min(dragStart.lineIndex, lineIndex);
      const end = Math.max(dragStart.lineIndex, lineIndex);

      const hunk = currentDiff?.hunks[hunkIndex];
      if (hunk) {
        // Clear and reselect range
        hunkLines.clear();
        for (let i = start; i <= end; i++) {
          if (hunk.lines[i]?.origin === '+') {
            hunkLines.add(i);
          }
        }
      }

      if (hunkLines.size === 0) {
        newMap.delete(hunkIndex);
      } else {
        newMap.set(hunkIndex, hunkLines);
      }

      return newMap;
    });
  };

  const handleStageSelectedLines = async (hunkIndex: number) => {
    const lineIndices = Array.from(selectedLines.get(hunkIndex) || []);
    if (lineIndices.length === 0) return;

    await handleStageLines(hunkIndex, lineIndices, isViewingStaged);

    // Clear selection after staging/unstaging
    setSelectedLines((prev) => {
      const newMap = new Map(prev);
      newMap.delete(hunkIndex);
      return newMap;
    });
  };

  const handleStageLines = async (hunkIndex: number, lineIndices: number[], unstage: boolean) => {
    if (!repo || !selectedPath || lineIndices.length === 0) return;

    setIsOperating(true);
    setContextMenu(null);

    try {
      const response = await api.stageLines({
        repoId: repo.repoId,
        path: selectedPath,
        hunkIndex,
        lineIndices,
        unstage,
      });

      if (response.ok && response.data) {
        useStore.getState().updateStatus(response.data);
        toast.success(unstage ? `Unstaged ${lineIndices.length} line(s)` : `Staged ${lineIndices.length} line(s)`);

        // Check if file still has changes to show
        const updatedEntry = response.data.entries.find((e) => e.path === selectedPath);
        if (updatedEntry) {
          // Check if the file still has changes in the current view
          if (!unstage && updatedEntry.unstagedStatus) {
            await useStore.getState().loadDiff(repo.repoId, selectedPath, 'working');
          } else if (unstage && updatedEntry.stagedStatus) {
            await useStore.getState().loadDiff(repo.repoId, selectedPath, 'index');
          } else {
            useStore.getState().clearDiff();
          }
        } else {
          useStore.getState().clearDiff();
        }
      } else {
        toast.error(response.message || (unstage ? 'Failed to unstage lines' : 'Failed to stage lines'));
      }
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Unknown error');
    } finally {
      setIsOperating(false);
    }
  };

  // Helper to get change type badge
  const getChangeBadge = () => {
    if (isCommitDiff && selectedCommitFile) {
      const changeColors = {
        Added: 'bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300',
        Deleted: 'bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-300',
        Modified: 'bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300',
        Renamed: 'bg-purple-100 text-purple-700 dark:bg-purple-900 dark:text-purple-300',
        Copied: 'bg-cyan-100 text-cyan-700 dark:bg-cyan-900 dark:text-cyan-300',
      };
      const color = changeColors[selectedCommitFile.change as keyof typeof changeColors] || 'bg-gray-100 text-gray-700';
      return (
        <span className={`text-xs px-2 py-0.5 rounded ${color}`}>
          {selectedCommitFile.change}
        </span>
      );
    }
    return null;
  };

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="px-4 py-2 bg-gray-50 dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700 flex-shrink-0">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <span className="text-sm font-mono text-gray-600 dark:text-gray-400">
              {displayDiff.path}
            </span>
            {!isCommitDiff && currentDiff?.isNew && (
              <span className="text-xs px-2 py-0.5 bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300 rounded">
                New
              </span>
            )}
            {!isCommitDiff && currentDiff?.isDeleted && (
              <span className="text-xs px-2 py-0.5 bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-300 rounded">
                Deleted
              </span>
            )}
            {getChangeBadge()}
          </div>
          <div className="flex items-center gap-3">
            <button
              onClick={() => setWrapLines(!wrapLines)}
              className="text-xs px-2 py-1 bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 hover:bg-gray-300 dark:hover:bg-gray-600 rounded"
              title={wrapLines ? 'Disable line wrapping' : 'Enable line wrapping'}
            >
              {wrapLines ? 'Unwrap' : 'Wrap'}
            </button>
            <div className="text-xs text-gray-500 dark:text-gray-400">
              {displayDiff.hunks?.length || 0} hunk(s)
            </div>
          </div>
        </div>
      </div>

      {/* Hunk-based view */}
      <div className="flex-1 overflow-auto">
        {displayDiff.hunks?.map((hunk, hunkIndex) => (
          <div
            key={hunkIndex}
            className="border-b border-gray-200 dark:border-gray-700"
            onContextMenu={!isCommitDiff && !currentDiff?.isNew && !isStagedNewFile ? (e) => handleContextMenu(e, hunkIndex) : undefined}
          >
            {/* Hunk header */}
            <div className="px-4 py-1 bg-blue-50 dark:bg-blue-900/20 text-xs font-mono text-blue-600 dark:text-blue-400 flex items-center justify-between">
              <span>{hunk.header}</span>
              {!isCommitDiff && !currentDiff?.isNew && !isStagedNewFile && (
                <div className="flex items-center gap-2">
                  {selectedLines.get(hunkIndex)?.size ? (
                    <button
                      onClick={() => handleStageSelectedLines(hunkIndex)}
                      className="px-2 py-0.5 bg-green-500 text-white rounded text-xs hover:bg-green-600"
                    >
                      {isViewingStaged ? 'Unstage' : 'Stage'} {selectedLines.get(hunkIndex)?.size} Line(s)
                    </button>
                  ) : null}
                  <button
                    onClick={() => handleStageHunk(hunkIndex, isViewingStaged)}
                    className="px-2 py-0.5 bg-blue-500 text-white rounded text-xs hover:bg-blue-600"
                  >
                    {isViewingStaged ? 'Unstage Hunk' : 'Stage Hunk'}
                  </button>
                </div>
              )}
            </div>

            {/* Hunk lines */}
            <div className="font-mono text-xs select-none">
              {hunk.lines.map((line, lineIndex) => {
                // Handle both FileDiff (working tree) and CommitFileDiff (history) line types
                const origin = isCommitDiff
                  ? ((line as any).type === 'add' ? '+' : (line as any).type === 'del' ? '-' : ' ')
                  : (line as any).origin;
                const content = isCommitDiff ? (line as any).text : (line as any).content;
                const oldLineno = isCommitDiff ? (line as any).lnOld : (line as any).oldLineno;
                const newLineno = isCommitDiff ? (line as any).lnNew : (line as any).newLineno;

                const isSelected = selectedLines.get(hunkIndex)?.has(lineIndex);
                const isAddedLine = origin === '+' && !isViewingStaged && !isCommitDiff;

                // Build className string more cleanly
                let lineClasses = `py-0.5 ${wrapLines ? 'whitespace-pre-wrap break-all' : 'whitespace-pre'} transition-colors`;
                lineClasses += showLineNumbers ? ' pl-2 pr-4' : ' px-4';

                if (isSelected && !isCommitDiff) {
                  // Selected state - always blue with consistent styling
                  lineClasses += ' bg-blue-100 dark:bg-blue-900 border-l-4 border-blue-500 text-blue-900 dark:text-blue-100 font-medium';
                } else {
                  // Unselected state
                  if (origin === '+') {
                    lineClasses += ' bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-300';
                  } else if (origin === '-') {
                    lineClasses += ' bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300';
                  } else {
                    lineClasses += ' text-gray-700 dark:text-gray-300';
                  }
                }

                if (isAddedLine) {
                  lineClasses += ' cursor-pointer hover:bg-blue-50 dark:hover:bg-blue-900/30';
                }

                return (
                  <div
                    key={lineIndex}
                    onMouseDown={!isCommitDiff ? (e) => handleLineMouseDown(hunkIndex, lineIndex, origin, e) : undefined}
                    onMouseEnter={!isCommitDiff ? () => handleLineMouseEnter(hunkIndex, lineIndex, origin) : undefined}
                    className={`flex ${lineClasses}`}
                  >
                    {showLineNumbers && (
                      <div className="flex-shrink-0 select-none text-gray-400 mr-2">
                        {!isCommitDiff && !currentDiff?.isNew && (
                          <span className="inline-block w-10 text-right">
                            {oldLineno ?? ''}
                          </span>
                        )}
                        <span className={`inline-block w-10 text-right ${!isCommitDiff && !currentDiff?.isNew ? 'ml-1' : ''}`}>
                          {newLineno ?? ''}
                        </span>
                      </div>
                    )}
                    <span className="inline-block w-4 text-gray-400 select-none flex-shrink-0">
                      {origin}
                    </span>
                    <span className="flex-1">{content?.trimEnd() || ''}</span>
                  </div>
                );
              })}
            </div>
          </div>
        ))}
      </div>

      {/* Context Menu */}
      {contextMenu && (
        <div
          className="fixed bg-white dark:bg-gray-800 shadow-lg rounded border border-gray-200 dark:border-gray-700 py-1 z-50"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          onClick={(e) => e.stopPropagation()}
        >
          <button
            className="w-full px-4 py-2 text-left text-sm hover:bg-gray-100 dark:hover:bg-gray-700"
            onClick={() => handleStageHunk(contextMenu.hunkIndex, isViewingStaged)}
          >
            {isViewingStaged ? 'Unstage Hunk' : 'Stage Hunk'}
          </button>
        </div>
      )}
    </div>
  );
}
