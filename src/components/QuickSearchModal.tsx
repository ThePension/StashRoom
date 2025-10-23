import { useEffect, useState, useMemo } from 'react';
import { Search, FileText, GitCommit, ChevronRight, ChevronDown } from 'lucide-react';
import { useStore } from '../state/store';
import { api } from '../lib/api';
import type { FileSearchResult } from '../lib/types';

interface QuickSearchModalProps {
  isOpen: boolean;
  onClose: () => void;
  onNavigateToHistory?: () => void;
}

export function QuickSearchModal({ isOpen, onClose, onNavigateToHistory }: QuickSearchModalProps) {
  const [searchQuery, setSearchQuery] = useState('');
  const [searchResults, setSearchResults] = useState<FileSearchResult[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [expandedFiles, setExpandedFiles] = useState<Set<string>>(new Set());

  const getActiveRepo = useStore((s) => s.getActiveRepo);
  const repo = getActiveRepo();
  const selectCommit = useStore((s) => s.selectCommit);
  const selectCommitFile = useStore((s) => s.selectCommitFile);

  // Perform search when query changes
  useEffect(() => {
    if (!isOpen || !repo || !searchQuery.trim()) {
      setSearchResults([]);
      setSelectedIndex(0);
      return;
    }

    const performSearch = async () => {
      setIsSearching(true);
      try {
        const response = await api.searchFiles({
          repoId: repo.repoId,
          query: searchQuery,
          limit: 50,
        });

        if (response.ok && response.data) {
          setSearchResults(response.data.results);
          setSelectedIndex(0);
        }
      } catch (error) {
        console.error('Search error:', error);
      } finally {
        setIsSearching(false);
      }
    };

    // Debounce search
    const timeoutId = setTimeout(performSearch, 300);
    return () => clearTimeout(timeoutId);
  }, [searchQuery, repo, isOpen]);

  // Reset state when modal opens/closes
  useEffect(() => {
    if (isOpen) {
      setSearchQuery('');
      setSearchResults([]);
      setSelectedIndex(0);
      setExpandedFiles(new Set());
    }
  }, [isOpen]);

  // Build a flat list of selectable items for navigation
  const selectableItems = useMemo(() => {
    const items: Array<{ type: 'file' | 'commit'; fileIndex: number; commitIndex?: number }> = [];

    searchResults.forEach((result, fileIndex) => {
      items.push({ type: 'file', fileIndex });

      if (expandedFiles.has(result.path)) {
        result.commits.forEach((_, commitIndex) => {
          items.push({ type: 'commit', fileIndex, commitIndex });
        });
      }
    });

    return items;
  }, [searchResults, expandedFiles]);

  // Handle keyboard navigation
  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      // Ignore if user is typing in the input
      if (e.target instanceof HTMLInputElement) {
        if (e.key === 'ArrowDown' || e.key === 'ArrowUp' || e.key === 'Enter') {
          e.preventDefault();
        } else if (e.key === 'Escape') {
          e.preventDefault();
          onClose();
          return;
        } else {
          return; // Let typing work normally
        }
      }

      if (e.key === 'Escape') {
        e.preventDefault();
        onClose();
      } else if (e.key === 'ArrowDown') {
        e.preventDefault();
        setSelectedIndex((prev) => Math.min(prev + 1, selectableItems.length - 1));
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        setSelectedIndex((prev) => Math.max(prev - 1, 0));
      } else if (e.key === 'Enter') {
        e.preventDefault();
        handleSelection(selectedIndex);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, selectedIndex, selectableItems, onClose]);

  const handleSelection = async (index: number) => {
    if (!repo || index < 0 || index >= selectableItems.length) return;

    const item = selectableItems[index];
    const result = searchResults[item.fileIndex];

    if (item.type === 'file') {
      // Toggle expand/collapse
      setExpandedFiles((prev) => {
        const newSet = new Set(prev);
        if (newSet.has(result.path)) {
          newSet.delete(result.path);
        } else {
          newSet.add(result.path);
        }
        return newSet;
      });
    } else if (item.type === 'commit' && item.commitIndex !== undefined) {
      // Navigate to commit and file
      const commitMatch = result.commits[item.commitIndex];

      // Switch to History view
      if (onNavigateToHistory) {
        onNavigateToHistory();
      }

      // Select the commit first
      await selectCommit(repo.repoId, commitMatch.commit);

      // Close the modal
      onClose();

      // After a brief delay, select the file
      setTimeout(() => {
        const commitDiff = useStore.getState().commitDiff;
        const fileInCommit = commitDiff?.find((f) => f.path === result.path);
        if (fileInCommit) {
          selectCommitFile(fileInCommit);
        }
      }, 100);
    }
  };

  if (!isOpen) return null;

  return (
    <div
      className="fixed inset-0 bg-black bg-opacity-50 flex items-start justify-center pt-32 z-50"
      onClick={onClose}
    >
      <div
        className="bg-white dark:bg-gray-800 rounded-lg shadow-xl w-full max-w-2xl mx-4"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Search Input */}
        <div className="flex items-center gap-3 p-4 border-b border-gray-200 dark:border-gray-700">
          <Search className="w-5 h-5 text-gray-400" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search files in commit history..."
            className="flex-1 bg-transparent outline-none text-gray-900 dark:text-gray-100 placeholder-gray-400"
            autoFocus
          />
          {isSearching && (
            <div className="w-4 h-4 border-2 border-gray-400 border-t-transparent rounded-full animate-spin" />
          )}
          <kbd className="px-2 py-1 text-xs bg-gray-100 dark:bg-gray-700 rounded">Esc</kbd>
        </div>

        {/* Results */}
        <div className="max-h-96 overflow-y-auto">
          {searchQuery.trim() === '' ? (
            <div className="p-8 text-center text-gray-500 dark:text-gray-400">
              <Search className="w-12 h-12 mx-auto mb-3 opacity-50" />
              <p>Type to search files in commit history</p>
              <p className="text-sm mt-2">Use arrow keys to navigate, Enter to select</p>
            </div>
          ) : searchResults.length === 0 && !isSearching ? (
            <div className="p-8 text-center text-gray-500 dark:text-gray-400">
              <FileText className="w-12 h-12 mx-auto mb-3 opacity-50" />
              <p>No files found matching "{searchQuery}"</p>
            </div>
          ) : (
            <div className="py-2">
              {searchResults.map((result, fileIdx) => {
                const isExpanded = expandedFiles.has(result.path);
                const fileItemIndex = selectableItems.findIndex(
                  (item) => item.type === 'file' && item.fileIndex === fileIdx
                );

                return (
                  <div
                    key={result.path}
                    className="border-b border-gray-100 dark:border-gray-700 last:border-b-0"
                  >
                    {/* File row */}
                    <div
                      className={`px-4 py-2 hover:bg-gray-50 dark:hover:bg-gray-700 cursor-pointer flex items-center gap-2 ${
                        selectedIndex === fileItemIndex
                          ? 'bg-blue-50 dark:bg-blue-900/30'
                          : ''
                      }`}
                      onClick={() => handleSelection(fileItemIndex)}
                    >
                      {isExpanded ? (
                        <ChevronDown className="w-4 h-4 text-gray-500" />
                      ) : (
                        <ChevronRight className="w-4 h-4 text-gray-500" />
                      )}
                      <FileText className="w-4 h-4 text-blue-500 flex-shrink-0" />
                      <span className="font-mono text-sm text-gray-900 dark:text-gray-100 flex-1 truncate">
                        {result.path}
                      </span>
                      <span className="text-xs text-gray-500 flex-shrink-0">
                        {result.commits.length} commit{result.commits.length !== 1 ? 's' : ''}
                      </span>
                    </div>

                    {/* Expanded commits */}
                    {isExpanded && (
                      <div className="bg-gray-50 dark:bg-gray-900">
                        {result.commits.map((commitMatch, commitIdx) => {
                          const commitItemIndex = selectableItems.findIndex(
                            (item) =>
                              item.type === 'commit' &&
                              item.fileIndex === fileIdx &&
                              item.commitIndex === commitIdx
                          );

                          return (
                            <div
                              key={commitMatch.commit.oid}
                              className={`pl-12 pr-4 py-2 hover:bg-gray-100 dark:hover:bg-gray-800 cursor-pointer flex items-center gap-2 ${
                                selectedIndex === commitItemIndex
                                  ? 'bg-blue-50 dark:bg-blue-900/30'
                                  : ''
                              }`}
                              onClick={() => handleSelection(commitItemIndex)}
                            >
                              <GitCommit className="w-3 h-3 text-gray-400 flex-shrink-0" />
                              <span className="font-mono text-xs text-gray-500 flex-shrink-0">
                                {commitMatch.commit.shortOid}
                              </span>
                              <span className="text-sm text-gray-700 dark:text-gray-300 flex-1 truncate">
                                {commitMatch.commit.subject}
                              </span>
                              <span
                                className={`text-xs px-2 py-0.5 rounded flex-shrink-0 ${
                                  commitMatch.change === 'added'
                                    ? 'bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300'
                                    : commitMatch.change === 'deleted'
                                    ? 'bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-300'
                                    : commitMatch.change === 'renamed'
                                    ? 'bg-yellow-100 text-yellow-700 dark:bg-yellow-900 dark:text-yellow-300'
                                    : 'bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300'
                                }`}
                              >
                                {commitMatch.change}
                              </span>
                            </div>
                          );
                        })}
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="px-4 py-3 bg-gray-50 dark:bg-gray-900 border-t border-gray-200 dark:border-gray-700 rounded-b-lg">
          <div className="flex items-center gap-4 text-xs text-gray-500 dark:text-gray-400">
            <div className="flex items-center gap-1">
              <kbd className="px-1.5 py-0.5 bg-white dark:bg-gray-800 rounded border border-gray-300 dark:border-gray-600">
                ↑↓
              </kbd>
              <span>Navigate</span>
            </div>
            <div className="flex items-center gap-1">
              <kbd className="px-1.5 py-0.5 bg-white dark:bg-gray-800 rounded border border-gray-300 dark:border-gray-600">
                Enter
              </kbd>
              <span>Select</span>
            </div>
            <div className="flex items-center gap-1">
              <kbd className="px-1.5 py-0.5 bg-white dark:bg-gray-800 rounded border border-gray-300 dark:border-gray-600">
                Esc
              </kbd>
              <span>Close</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
