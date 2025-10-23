import { useState, useEffect } from 'react';
import { X, FolderOpen, RotateCcw, Trash2 } from 'lucide-react';
import { api } from '../lib/api';
import { toast } from 'sonner';
import { useStore } from '../state/store';
import { useConfirm } from '../hooks/useConfirm';

interface BackupsModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export function BackupsModal({ isOpen, onClose }: BackupsModalProps) {
  const getActiveRepo = useStore((s) => s.getActiveRepo);
  const repo = getActiveRepo();

  const [backups, setBackups] = useState<string[]>([]);
  const [selectedBackup, setSelectedBackup] = useState<string | null>(null);
  const [files, setFiles] = useState<string[]>([]);
  const [selectedFiles, setSelectedFiles] = useState<Set<string>>(new Set());
  const [isLoading, setIsLoading] = useState(false);
  const [isRestoring, setIsRestoring] = useState(false);
  const { confirm, ConfirmDialog } = useConfirm();

  // Load backups when modal opens
  useEffect(() => {
    if (isOpen && repo) {
      loadBackups();
    }
  }, [isOpen, repo]);

  // Load files when backup is selected
  useEffect(() => {
    if (selectedBackup && repo) {
      loadFiles(selectedBackup);
    }
  }, [selectedBackup, repo]);

  const loadBackups = async () => {
    if (!repo) return;

    setIsLoading(true);
    try {
      const response = await api.listBackups(repo.repoId);
      if (response.ok && response.data) {
        setBackups(response.data);
        // Select the newest backup by default
        if (response.data.length > 0) {
          setSelectedBackup(response.data[0]);
        }
      } else {
        toast.error('Failed to load backups');
      }
    } catch (error) {
      toast.error('Failed to load backups');
    } finally {
      setIsLoading(false);
    }
  };

  const loadFiles = async (backupId: string) => {
    if (!repo) return;

    setIsLoading(true);
    setSelectedFiles(new Set());
    try {
      const response = await api.listBackupFiles(repo.repoId, backupId);
      if (response.ok && response.data) {
        setFiles(response.data);
      } else {
        toast.error('Failed to load backup files');
      }
    } catch (error) {
      toast.error('Failed to load backup files');
    } finally {
      setIsLoading(false);
    }
  };

  const handleSelectAll = () => {
    if (selectedFiles.size === files.length) {
      setSelectedFiles(new Set());
    } else {
      setSelectedFiles(new Set(files));
    }
  };

  const handleToggleFile = (file: string) => {
    const newSelection = new Set(selectedFiles);
    if (newSelection.has(file)) {
      newSelection.delete(file);
    } else {
      newSelection.add(file);
    }
    setSelectedFiles(newSelection);
  };

  const handleRestoreSelected = async () => {
    if (!repo || !selectedBackup || selectedFiles.size === 0) return;

    setIsRestoring(true);
    try {
      const pathsToRestore = Array.from(selectedFiles);
      const response = await api.restoreMany(repo.repoId, selectedBackup, pathsToRestore);

      if (response.ok && response.data) {
        useStore.getState().updateStatus(response.data);
        await useStore.getState().refreshStatus(repo.repoId, true);

        const timestamp = formatTimestamp(selectedBackup);
        toast.success(`Restored ${pathsToRestore.length} file(s) from ${timestamp}`);
        onClose();
      } else {
        toast.error(response.message || 'Failed to restore files');
      }
    } catch (error) {
      toast.error('Failed to restore files');
    } finally {
      setIsRestoring(false);
    }
  };

  const handleRestoreAll = async () => {
    if (!repo || !selectedBackup || files.length === 0) return;

    setIsRestoring(true);
    try {
      const response = await api.restoreMany(repo.repoId, selectedBackup, files);

      if (response.ok && response.data) {
        useStore.getState().updateStatus(response.data);
        await useStore.getState().refreshStatus(repo.repoId, true);

        const timestamp = formatTimestamp(selectedBackup);
        toast.success(`Restored ${files.length} file(s) from ${timestamp}`);
        onClose();
      } else {
        toast.error(response.message || 'Failed to restore files');
      }
    } catch (error) {
      toast.error('Failed to restore files');
    } finally {
      setIsRestoring(false);
    }
  };

  const handleRevealFile = async (file: string) => {
    if (!repo || !selectedBackup) return;

    try {
      const { Command } = await import('@tauri-apps/plugin-shell');
      const { platform } = await import('@tauri-apps/plugin-os');
      const currentPlatform = platform();

      // Construct the backup file path
      let backupPath = `${repo.path}/.git/recover/${selectedBackup}/${file}`;

      if (currentPlatform === 'windows') {
        // Windows: Use explorer /select with backslashes
        backupPath = backupPath.replace(/\//g, '\\');
        await Command.create('explorer', ['/select,', backupPath]).execute();
      } else {
        // Linux: Open the parent directory
        const dirPath = backupPath.substring(0, backupPath.lastIndexOf('/'));
        await Command.create('xdg-open', [dirPath]).execute();
      }
    } catch (error) {
      toast.error('Failed to reveal file');
    }
  };

  const handleClearBackups = async () => {
    if (!repo) return;

    const confirmed = await confirm({
      title: 'Clear All Backups',
      message: 'Are you sure you want to delete all backups? This action cannot be undone.',
      confirmText: 'Clear All',
      cancelText: 'Cancel',
      variant: 'danger',
    });

    if (!confirmed) return;

    setIsLoading(true);
    try {
      const response = await api.clearBackups(repo.repoId);

      if (response.ok) {
        setBackups([]);
        setSelectedBackup(null);
        setFiles([]);
        setSelectedFiles(new Set());
        toast.success('All backups have been cleared');
      } else {
        toast.error(response.message || 'Failed to clear backups');
      }
    } catch (error) {
      toast.error('Failed to clear backups');
    } finally {
      setIsLoading(false);
    }
  };

  const formatTimestamp = (timestamp: string): string => {
    // Format: YYYYMMDD_HHMMSS
    try {
      const year = timestamp.substring(0, 4);
      const month = timestamp.substring(4, 6);
      const day = timestamp.substring(6, 8);
      const hour = timestamp.substring(9, 11);
      const minute = timestamp.substring(11, 13);

      const date = new Date(`${year}-${month}-${day}T${hour}:${minute}:00`);
      const now = new Date();
      const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
      const backupDate = new Date(date.getFullYear(), date.getMonth(), date.getDate());

      if (backupDate.getTime() === today.getTime()) {
        return `Today ${hour}:${minute}`;
      } else if (backupDate.getTime() === today.getTime() - 86400000) {
        return `Yesterday ${hour}:${minute}`;
      } else {
        return `${year}-${month}-${day} ${hour}:${minute}`;
      }
    } catch {
      return timestamp;
    }
  };

  if (!isOpen) return null;

  return (
    <>
      <ConfirmDialog />
      <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
        <div className="bg-white dark:bg-gray-800 rounded-lg shadow-xl w-[800px] max-h-[600px] flex flex-col">
          {/* Header */}
          <div className="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
            <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
              Backups of discarded files
            </h2>
            <div className="flex items-center gap-2">
              <button
                onClick={handleClearBackups}
                disabled={backups.length === 0 || isLoading || isRestoring}
                className="px-3 py-1 text-sm bg-red-500 text-white rounded hover:bg-red-600 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
                title="Clear all backups"
              >
                <Trash2 className="w-4 h-4" />
                Clear All
              </button>
              <button
                onClick={onClose}
                className="p-1 hover:bg-gray-100 dark:hover:bg-gray-700 rounded"
              >
                <X className="w-5 h-5" />
              </button>
            </div>
          </div>

        {/* Content */}
        <div className="flex flex-1 overflow-hidden">
          {/* Left: Backup timestamps */}
          <div className="w-1/3 border-r border-gray-200 dark:border-gray-700 overflow-y-auto">
            {isLoading && backups.length === 0 ? (
              <div className="p-4 text-center text-gray-500 dark:text-gray-400">
                Loading...
              </div>
            ) : backups.length === 0 ? (
              <div className="p-4 text-center text-gray-500 dark:text-gray-400">
                No backups yet
              </div>
            ) : (
              backups.map((backup) => (
                <button
                  key={backup}
                  onClick={() => setSelectedBackup(backup)}
                  className={`w-full text-left px-4 py-2 hover:bg-gray-100 dark:hover:bg-gray-700 ${
                    selectedBackup === backup
                      ? 'bg-blue-100 dark:bg-blue-900 text-blue-900 dark:text-blue-100'
                      : 'text-gray-700 dark:text-gray-300'
                  }`}
                >
                  {formatTimestamp(backup)}
                </button>
              ))
            )}
          </div>

          {/* Right: Files in selected backup */}
          <div className="flex-1 flex flex-col">
            {selectedBackup && (
              <>
                {/* File list header */}
                <div className="px-4 py-2 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
                  <label className="flex items-center gap-2 text-sm">
                    <input
                      type="checkbox"
                      checked={selectedFiles.size === files.length && files.length > 0}
                      onChange={handleSelectAll}
                      className="rounded"
                    />
                    Select all ({files.length})
                  </label>
                </div>

                {/* File list */}
                <div className="flex-1 overflow-y-auto">
                  {isLoading ? (
                    <div className="p-4 text-center text-gray-500 dark:text-gray-400">
                      Loading files...
                    </div>
                  ) : files.length === 0 ? (
                    <div className="p-4 text-center text-gray-500 dark:text-gray-400">
                      No files in this backup
                    </div>
                  ) : (
                    files.map((file) => (
                      <div
                        key={file}
                        className="flex items-center gap-2 px-4 py-2 hover:bg-gray-50 dark:hover:bg-gray-700 border-b border-gray-100 dark:border-gray-700"
                      >
                        <input
                          type="checkbox"
                          checked={selectedFiles.has(file)}
                          onChange={() => handleToggleFile(file)}
                          className="rounded"
                        />
                        <span className="flex-1 text-sm font-mono text-gray-700 dark:text-gray-300">
                          {file}
                        </span>
                        <button
                          onClick={() => handleRevealFile(file)}
                          className="p-1 hover:bg-gray-200 dark:hover:bg-gray-600 rounded"
                          title="Reveal on disk"
                        >
                          <FolderOpen className="w-4 h-4" />
                        </button>
                      </div>
                    ))
                  )}
                </div>
              </>
            )}
          </div>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 p-4 border-t border-gray-200 dark:border-gray-700">
          <button
            onClick={onClose}
            className="px-4 py-2 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded"
          >
            Cancel
          </button>
          <button
            onClick={handleRestoreAll}
            disabled={!selectedBackup || files.length === 0 || isRestoring}
            className="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
          >
            <RotateCcw className="w-4 h-4" />
            Restore All
          </button>
          <button
            onClick={handleRestoreSelected}
            disabled={!selectedBackup || selectedFiles.size === 0 || isRestoring}
            className="px-4 py-2 bg-green-500 text-white rounded hover:bg-green-600 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
          >
            <RotateCcw className="w-4 h-4" />
            Restore Selected ({selectedFiles.size})
          </button>
        </div>
      </div>
    </div>
    </>
  );
}
