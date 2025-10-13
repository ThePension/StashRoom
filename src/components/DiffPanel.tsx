import { useEffect, useState, useRef } from 'react';
import Editor, { DiffEditor } from '@monaco-editor/react';
import { useStore } from '../state/store';
import { api } from '../lib/api';
import { toast } from 'sonner';

export function DiffPanel() {
  const repo = useStore((s) => s.repo);
  const currentDiff = useStore((s) => s.currentDiff);
  const selectedPath = useStore((s) => s.selectedPath);
  const setIsOperating = useStore((s) => s.setIsOperating);
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    hunkIndex: number;
  } | null>(null);

  const editorRef = useRef<any>(null);

  useEffect(() => {
    const handleClick = () => setContextMenu(null);
    window.addEventListener('click', handleClick);
    return () => window.removeEventListener('click', handleClick);
  }, []);

  if (!currentDiff) {
    return (
      <div className="flex-1 flex items-center justify-center text-gray-400">
        <div className="text-center">
          <p className="text-lg mb-2">No file selected</p>
          <p className="text-sm">Select a file from the change list to view its diff</p>
        </div>
      </div>
    );
  }

  if (currentDiff.isBinary) {
    return (
      <div className="flex-1 flex items-center justify-center text-gray-400">
        <div className="text-center">
          <p className="text-lg mb-2">Binary File</p>
          <p className="text-sm">{currentDiff.path}</p>
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
      toast.error(error instanceof Error ? error.message : 'Unknown error');
    } finally {
      setIsOperating(false);
    }
  };

  const handleStageLines = async (hunkIndex: number, lineIndices: number[]) => {
    if (!repo || !selectedPath || lineIndices.length === 0) return;

    setIsOperating(true);
    setContextMenu(null);

    try {
      const response = await api.stageLines({
        repoId: repo.repoId,
        path: selectedPath,
        hunkIndex,
        lineIndices,
        unstage: false,
      });

      if (response.ok && response.data) {
        useStore.getState().updateStatus(response.data);
        toast.success(`Staged ${lineIndices.length} line(s)`);
        // Reload diff
        useStore.getState().loadDiff(repo.repoId, selectedPath, 'working');
      } else {
        toast.error(response.message || 'Failed to stage lines');
      }
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Unknown error');
    } finally {
      setIsOperating(false);
    }
  };

  // Convert diff to Monaco format
  const originalContent = currentDiff.hunks
    .flatMap((hunk) =>
      hunk.lines
        .filter((line) => line.origin === '-' || line.origin === ' ')
        .map((line) => line.content.trimEnd())
    )
    .join('\n');

  const modifiedContent = currentDiff.hunks
    .flatMap((hunk) =>
      hunk.lines
        .filter((line) => line.origin === '+' || line.origin === ' ')
        .map((line) => line.content.trimEnd())
    )
    .join('\n');

  return (
    <div className="flex-1 flex flex-col relative">
      {/* Header */}
      <div className="px-4 py-2 bg-gray-50 dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <span className="text-sm font-mono text-gray-600 dark:text-gray-400">
              {currentDiff.path}
            </span>
            {currentDiff.isNew && (
              <span className="text-xs px-2 py-0.5 bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300 rounded">
                New
              </span>
            )}
            {currentDiff.isDeleted && (
              <span className="text-xs px-2 py-0.5 bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-300 rounded">
                Deleted
              </span>
            )}
          </div>
          <div className="text-xs text-gray-500 dark:text-gray-400">
            {currentDiff.hunks.length} hunk(s)
          </div>
        </div>
      </div>

      {/* Hunk-based view */}
      <div className="flex-1 overflow-auto">
        {currentDiff.hunks.map((hunk, hunkIndex) => (
          <div
            key={hunkIndex}
            className="border-b border-gray-200 dark:border-gray-700"
            onContextMenu={(e) => handleContextMenu(e, hunkIndex)}
          >
            {/* Hunk header */}
            <div className="px-4 py-1 bg-blue-50 dark:bg-blue-900/20 text-xs font-mono text-blue-600 dark:text-blue-400 flex items-center justify-between">
              <span>{hunk.header}</span>
              <button
                onClick={() => handleStageHunk(hunkIndex)}
                className="px-2 py-0.5 bg-blue-500 text-white rounded text-xs hover:bg-blue-600"
              >
                Stage Hunk
              </button>
            </div>

            {/* Hunk lines */}
            <div className="font-mono text-xs">
              {hunk.lines.map((line, lineIndex) => (
                <div
                  key={lineIndex}
                  className={`
                    px-4 py-0.5 whitespace-pre
                    ${line.origin === '+' ? 'bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-300' : ''}
                    ${line.origin === '-' ? 'bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300' : ''}
                    ${line.origin === ' ' ? 'text-gray-700 dark:text-gray-300' : ''}
                  `}
                >
                  <span className="inline-block w-4 text-gray-400 select-none">
                    {line.origin}
                  </span>
                  {line.content.trimEnd()}
                </div>
              ))}
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
            onClick={() => handleStageHunk(contextMenu.hunkIndex)}
          >
            Stage Hunk
          </button>
          <button
            className="w-full px-4 py-2 text-left text-sm hover:bg-gray-100 dark:hover:bg-gray-700"
            onClick={() => handleStageHunk(contextMenu.hunkIndex, true)}
          >
            Unstage Hunk
          </button>
        </div>
      )}
    </div>
  );
}
