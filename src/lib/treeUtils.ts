import type { StatusEntry, FileStatus } from './types';

export interface TreeNode {
  type: 'file' | 'folder';
  name: string;
  path: string;
  depth: number;
  children?: TreeNode[];
  entry?: StatusEntry; // only for files
  // Aggregated stats for folders
  stats?: {
    modified: number;
    added: number;
    deleted: number;
    hasStaged: boolean;
  };
}

export interface FlatTreeNode extends TreeNode {
  isExpanded?: boolean;
}

/**
 * Build a hierarchical tree structure from flat status entries
 */
export function buildTree(entries: StatusEntry[]): TreeNode {
  const root: TreeNode = {
    type: 'folder',
    name: '',
    path: '',
    depth: -1,
    children: [],
    stats: { modified: 0, added: 0, deleted: 0, hasStaged: false },
  };

  entries.forEach((entry) => {
    const parts = entry.path.split('/');
    let currentNode = root;

    // Traverse/create folders
    for (let i = 0; i < parts.length - 1; i++) {
      const folderName = parts[i];
      const folderPath = parts.slice(0, i + 1).join('/');

      let folderNode = currentNode.children?.find(
        (n) => n.type === 'folder' && n.name === folderName
      );

      if (!folderNode) {
        folderNode = {
          type: 'folder',
          name: folderName,
          path: folderPath,
          depth: i,
          children: [],
          stats: { modified: 0, added: 0, deleted: 0, hasStaged: false },
        };
        currentNode.children!.push(folderNode);
      }

      currentNode = folderNode;
    }

    // Add the file
    const fileName = parts[parts.length - 1];
    const fileNode: TreeNode = {
      type: 'file',
      name: fileName,
      path: entry.path,
      depth: parts.length - 1,
      entry,
    };

    currentNode.children!.push(fileNode);

    // Update folder stats
    updateFolderStats(root, entry);
  });

  // Sort children (folders first, then alphabetically)
  sortTree(root);

  return root;
}

/**
 * Update folder statistics recursively
 */
function updateFolderStats(node: TreeNode, entry: StatusEntry): void {
  if (node.type !== 'folder' || !node.stats) return;

  // Count unstaged changes
  if (entry.unstagedStatus === 'modified') {
    node.stats.modified++;
  } else if (entry.untracked) {
    node.stats.added++;
  } else if (entry.unstagedStatus === 'deleted') {
    node.stats.deleted++;
  }

  // Count staged changes (for files that are only staged, not unstaged)
  if (entry.stagedStatus !== null && entry.unstagedStatus === null && !entry.untracked) {
    if (entry.stagedStatus === 'modified') {
      node.stats.modified++;
    } else if (entry.stagedStatus === 'added') {
      node.stats.added++;
    } else if (entry.stagedStatus === 'deleted') {
      node.stats.deleted++;
    }
  }

  // Check for staged changes
  if (entry.stagedStatus !== null) {
    node.stats.hasStaged = true;
  }

  // Recursively update parent folder stats
  if (node.children) {
    node.children.forEach((child) => {
      if (child.type === 'folder' && child.path && entry.path.startsWith(child.path + '/')) {
        updateFolderStats(child, entry);
      }
    });
  }
}

/**
 * Sort tree nodes (folders first, then alphabetically)
 */
function sortTree(node: TreeNode): void {
  if (!node.children) return;

  node.children.sort((a, b) => {
    // Folders before files
    if (a.type !== b.type) {
      return a.type === 'folder' ? -1 : 1;
    }
    // Alphabetical
    return a.name.localeCompare(b.name);
  });

  // Recursively sort children
  node.children.forEach(sortTree);
}

/**
 * Flatten tree into a list of visible nodes based on expansion state
 */
export function flattenTree(
  root: TreeNode,
  expandedPaths: Set<string>
): FlatTreeNode[] {
  const result: FlatTreeNode[] = [];

  function traverse(node: TreeNode): void {
    // Skip root node
    if (node.depth >= 0) {
      result.push({
        ...node,
        isExpanded: expandedPaths.has(node.path),
      });
    }

    // If folder is expanded, traverse children
    if (
      node.type === 'folder' &&
      node.children &&
      (node.depth < 0 || expandedPaths.has(node.path))
    ) {
      node.children.forEach(traverse);
    }
  }

  traverse(root);
  return result;
}

/**
 * Get all file entries from a folder (recursively)
 */
export function getFilesInFolder(node: TreeNode): StatusEntry[] {
  const files: StatusEntry[] = [];

  function collect(n: TreeNode): void {
    if (n.type === 'file' && n.entry) {
      files.push(n.entry);
    } else if (n.type === 'folder' && n.children) {
      n.children.forEach(collect);
    }
  }

  collect(node);
  return files;
}
