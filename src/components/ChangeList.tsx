import { useEffect, useRef, useMemo } from 'react';
import { Virtuoso } from 'react-virtuoso';
import { useStore } from '../state/store';
import { api } from '../lib/api';
import { toast } from 'sonner';
import type { StatusEntry } from '../lib/types';

interface ChangeListProps {
  type: 'unstaged' | 'staged';
}

export function ChangeList({ type }: ChangeListProps) {
  const repo = useStore((s) => s.repo);
  const allEntries = useStore((s) => s.entries);
  const selectedPath = useStore((s) => s.selectedPath);
  const setSelectedPath = useStore((s) => s.setSelectedPath);
  const loadDiff = useStore((s) => s.loadDiff);
  const refreshStatus = useStore((s) => s.refreshStatus);
  const setIsOperating = useStore((s) => s.setIsOperating);

  // Filter entries based on type (memoized to prevent infinite loops)
  const entries = useMemo(() => {
    return type === 'unstaged'
      ? allEntries.filter((e) => e.unstagedStatus !== null || e.untracked)
      : allEntries.filter((e) => e.stagedStatus !== null);
  }, [allEntries, type]);

  const virtuosoRef = useRef<any>(null);
  const selectedIndex = useMemo(
    () => entries.findIndex((e) => e.path === selectedPath),
    [entries, selectedPath]
  );

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (!entries.length || !document.activeElement?.closest('.change-list')) return;

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
            handleDiscard(entries[selectedIndex]);
          }
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [entries, selectedIndex, selectedPath, repo, type]);

  const handleSelect = (entry: StatusEntry) => {
    setSelectedPath(entry.path);
    if (repo) {
      loadDiff(repo.repoId, entry.path, type === 'unstaged' ? 'working' : 'index');
    }
  };

  const handleStageToggle = async (entry: StatusEntry) => {
    if (!repo) return;

    setIsOperating(true);
    try {
      const response =
        type === 'unstaged'
          ? await api.stageFile(repo.repoId, entry.path)
          : await api.unstageFile(repo.repoId, entry.path);

      if (response.ok && response.data) {
        useStore.getState().updateStatus(response.data);
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

  const handleDiscard = async (entry: StatusEntry) => {
    if (!repo) return;

    const confirmed = window.confirm(
      `Are you sure you want to discard changes to "${entry.path}"? This cannot be undone (but a backup will be created).`
    );

    if (!confirmed) return;

    setIsOperating(true);
    try {
      const response = await api.discard({
        repoId: repo.repoId,
        path: entry.path,
        hunks: null,
      });

      if (response.ok && response.data) {
        useStore.getState().updateStatus(response.data);
        useStore.getState().clearDiff();
        toast.success(`Discarded changes to ${entry.path}`);
      } else {
        toast.error(response.message || 'Failed to discard changes');
      }
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Unknown error');
    } finally {
      setIsOperating(false);
    }
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
    <div className="flex-1 change-list bg-white dark:bg-gray-900" tabIndex={0} style={{ minHeight: '100px' }}>
      <Virtuoso
        ref={virtuosoRef}
        style={{ height: '100%', minHeight: '100px' }}
        totalCount={entries.length}
        itemContent={(index) => {
          const entry = entries[index];
          const isSelected = entry.path === selectedPath;

          return (
            <div
              className={`
                px-3 py-1.5 cursor-pointer select-none text-sm font-mono
                text-gray-700 dark:text-gray-300
                hover:bg-gray-100 dark:hover:bg-gray-800
                ${isSelected ? 'bg-blue-50 dark:bg-blue-900/20 border-l-2 border-blue-500' : ''}
              `}
              onClick={() => handleSelect(entry)}
              onDoubleClick={() => handleStageToggle(entry)}
            >
              <div className="flex items-center gap-2">
                <span className="w-4 text-center">{getStatusIcon(entry)}</span>
                <span className="truncate">{entry.path}</span>
              </div>
            </div>
          );
        }}
      />
    </div>
  );
}
