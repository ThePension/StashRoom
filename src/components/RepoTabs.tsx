import { X } from 'lucide-react';
import { useStore } from '../state/store';

export function RepoTabs() {
  const repos = useStore((s) => s.repos);
  const activeRepoId = useStore((s) => s.activeRepoId);
  const setActiveRepo = useStore((s) => s.setActiveRepo);
  const closeRepo = useStore((s) => s.closeRepo);

  if (repos.length === 0) return null;

  return (
    <div className="flex items-center gap-1 bg-gray-100 dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700 px-2 overflow-x-auto">
      {repos.map((repo) => {
        const isActive = repo.repoId === activeRepoId;
        // Extract just the folder name from the path
        const folderName = repo.path.split(/[/\\]/).pop() || repo.path;

        return (
          <div
            key={repo.repoId}
            className={`
              flex items-center gap-2 px-3 py-1.5 text-sm cursor-pointer group
              border-b-2 transition-colors
              ${
                isActive
                  ? 'bg-white dark:bg-gray-900 border-blue-500 text-gray-900 dark:text-gray-100'
                  : 'border-transparent text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100 hover:bg-gray-50 dark:hover:bg-gray-700'
              }
            `}
            onClick={() => {
              if (!isActive) {
                setActiveRepo(repo.repoId);
              }
            }}
          >
            <span className="font-mono max-w-[150px] truncate" title={repo.path}>
              {folderName}
            </span>
            {repo.head?.branch && (
              <span className="text-xs text-gray-500 dark:text-gray-400">
                ({repo.head.branch})
              </span>
            )}
            <button
              onClick={(e) => {
                e.stopPropagation();
                closeRepo(repo.repoId);
              }}
              className="p-0.5 rounded hover:bg-gray-200 dark:hover:bg-gray-600 opacity-0 group-hover:opacity-100 transition-opacity"
              title="Close repository"
            >
              <X className="w-3 h-3" />
            </button>
          </div>
        );
      })}
    </div>
  );
}
