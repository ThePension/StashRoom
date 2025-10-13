// ============================================================================
// API Response Envelope
// ============================================================================

export interface ApiResponse<T> {
  ok: boolean;
  data?: T;
  code?: string;
  message?: string;
}

// ============================================================================
// Repository Operations
// ============================================================================

export interface RepoOpenRequest {
  path: string;
}

export interface RepoOpenResponse {
  repoId: string;
  path: string;
  head: HeadInfo | null;
}

export interface HeadInfo {
  branch: string | null;
  commit: string;
  message: string | null;
}

// ============================================================================
// Status Operations
// ============================================================================

export interface StatusMatrix {
  entries: StatusEntry[];
}

export interface StatusEntry {
  path: string;
  status: FileStatus;
  stagedStatus: string | null;   // "added" | "modified" | "deleted"
  unstagedStatus: string | null; // "modified" | "deleted"
  untracked: boolean;
}

export type FileStatus =
  | "untracked"
  | "modified"
  | "added"
  | "deleted"
  | "renamed"
  | "conflicted";

// ============================================================================
// Diff Operations
// ============================================================================

export interface DiffRequest {
  repoId: string;
  path: string;
  side: DiffSide;
}

export type DiffSide = "working" | "index" | "head";

export interface FileDiff {
  path: string;
  oldPath: string | null;
  isBinary: boolean;
  isNew: boolean;
  isDeleted: boolean;
  hunks: DiffHunk[];
}

export interface DiffHunk {
  header: string;
  oldStart: number;
  oldLines: number;
  newStart: number;
  newLines: number;
  lines: DiffLine[];
}

export interface DiffLine {
  origin: string; // "+" | "-" | " "
  content: string;
  oldLineno: number | null;
  newLineno: number | null;
}

// ============================================================================
// Stage Operations
// ============================================================================

export interface StageHunkRequest {
  repoId: string;
  path: string;
  hunkIndex: number;
  unstage: boolean;
}

export interface StageLinesRequest {
  repoId: string;
  path: string;
  hunkIndex: number;
  lineIndices: number[];
  unstage: boolean;
}

// ============================================================================
// Discard Operations
// ============================================================================

export interface DiscardRequest {
  repoId: string;
  path: string;
  hunks: number[] | null;
}

// ============================================================================
// Commit Operations
// ============================================================================

export interface CommitRequest {
  repoId: string;
  message: string;
  authorName?: string;
  authorEmail?: string;
}

export interface CommitResponse {
  oid: string;
}

// ============================================================================
// Watch Operations
// ============================================================================

export interface WatchEvent {
  repoId: string;
  paths: string[];
  eventType: string; // "modified" | "created" | "deleted"
}
