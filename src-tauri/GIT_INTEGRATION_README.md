# Git Integration Implementation Summary

## Overview

This implementation provides a complete Rust/Tauri bridge for Git operations using `libgit2` via the `git2` crate. All commands are exposed as Tauri commands that can be invoked from the frontend.

## Architecture

### Core Modules (`src/core/`)

- **repo.rs**: Repository registry management with UUID-based identification
- **status.rs**: Git status computation (staged/unstaged/untracked files)
- **diff.rs**: Diff generation with CRLF-safe line ending preservation
- **stage.rs**: Hunk and line staging/unstaging operations
- **discard.rs**: Discard changes with automatic backup to `.git/recover/`
- **fs_watch.rs**: Filesystem watching with 200ms debouncing

### Data Types (`src/types.rs`)

All API types use consistent JSON serialization with camelCase field names:
- `RepoOpenRequest/Response`
- `StatusMatrix`, `StatusEntry`, `FileStatus`
- `FileDiff`, `DiffHunk`, `DiffLine`
- `StageHunkRequest`, `StageLinesRequest`
- `DiscardRequest`, `CommitRequest`
- `WatchEvent`
- `ApiResponse<T>` (envelope with ok/data/code/message)

### Tauri Bridge (`src/bridge.rs`)

Exposes 14 Tauri commands:
- `open_repo`, `close_repo`
- `get_status`
- `get_diff`
- `stage_hunk`, `stage_lines`, `stage_file`, `unstage_file`
- `discard`
- `commit`
- `subscribe_watch`, `unsubscribe_watch`
- `list_backups`, `restore_from_backup`

## Key Features & Tradeoffs

### CRLF Safety
- Line endings are preserved exactly as stored in the repository
- No normalization during diff or staging operations
- Uses `libgit2` patch application to maintain byte-exact content

### Hunk & Line Staging
- **Hunk staging**: Fully implemented for both staging and unstaging
- **Line staging**: Only supports staging **added lines** (`+` lines)
  - Limitation: Staging individual removed lines requires complex patch manipulation
  - Documented in code comments

### Discard with Backup
- Automatically creates backups before discarding changes
- Backup location: `.git/recover/<timestamp>/<path>`
- Provides `list_backups` and `restore_from_backup` commands for recovery

### Filesystem Watching
- Uses `notify` crate with 200ms debounce
- Filters out `.git` directory changes
- Coalesces multiple events before emitting to UI
- Emits `watch-event` via `tauri::AppHandle.emit_all()`

### Binary Detection
- Uses `libgit2`'s built-in binary detection
- Falls back to simple heuristics (null bytes, non-printable ratio)
- Binary files are flagged but not diffed

### Repository Management
- In-memory registry with UUID-based repo IDs
- Thread-safe with `Arc<RwLock<HashMap>>`
- Stable IDs across operations

## Usage from Frontend

```typescript
import { invoke } from '@tauri-apps/api/core';

// Open repository
const response = await invoke('open_repo', {
  request: { path: '/path/to/repo' }
});
const repoId = response.data.repoId;

// Get status
const status = await invoke('get_status', { repoId });

// Get diff
const diff = await invoke('get_diff', {
  request: {
    repoId,
    path: 'src/main.rs',
    side: 'working'
  }
});

// Stage a hunk
await invoke('stage_hunk', {
  request: {
    repoId,
    path: 'src/main.rs',
    hunkIndex: 0,
    unstage: false
  }
});

// Commit
await invoke('commit', {
  request: {
    repoId,
    message: 'feat: add new feature'
  }
});

// Subscribe to file changes
await invoke('subscribe_watch', { repoId });

// Listen for watch events
import { listen } from '@tauri-apps/api/event';
await listen('watch-event', (event) => {
  console.log('Files changed:', event.payload.paths);
});
```

## Testing

Unit tests are included for:
- Repository operations (`core/repo.rs`)
- Status computation (`core/status.rs`)
- Diff generation (`core/diff.rs`)
- Staging operations (`core/stage.rs`)
- Discard with backup (`core/discard.rs`)

Run tests with:
```bash
cargo test
```

## Dependencies

- **git2**: 0.19 - libgit2 bindings
- **anyhow**: 1.0 - Error handling
- **uuid**: 1.0 - Unique repository IDs
- **notify**: 6.1 - Filesystem watching
- **chrono**: 0.4 - Timestamp generation for backups
- **serde/serde_json**: JSON serialization
- **tauri**: 2.x - Tauri framework

Dev dependencies:
- **tempfile**: 3.8 - Temporary test repositories
- **insta**: 1.34 - Snapshot testing (prepared for future use)

## Limitations & Future Work

1. **Line staging**: Currently only supports added lines
2. **Hunk discard**: Not yet implemented (only full-file discard works)
3. **Diff caching**: `DiffCacheKey` type defined but not used yet
4. **Conflict resolution**: Conflicted files are detected but no merge tools provided
5. **Submodules**: Explicitly excluded from status operations

## Acceptance Criteria ✓

- [x] Opening a repo returns a stable repoId and head info
- [x] Status includes staged vs unstaged correctly across modified/added/deleted/untracked
- [x] Diff returns hunks with headers and line arrays; binary files are flagged
- [x] Stage/unstage hunk updates the index and re-computes status
- [x] Discard(file) reverts the worktree file and creates a backup
- [x] Watch events fire when a file changes on disk
- [x] CRLF-safe operations throughout
- [x] Compiles without errors
- [x] Unit tests for core functionality

## File Structure

```
src-tauri/
├── src/
│   ├── core/
│   │   ├── mod.rs
│   │   ├── repo.rs          # Repository registry
│   │   ├── status.rs        # Git status
│   │   ├── diff.rs          # Diff generation
│   │   ├── stage.rs         # Staging operations
│   │   ├── discard.rs       # Discard with backup
│   │   └── fs_watch.rs      # Filesystem watching
│   ├── types.rs             # Data models
│   ├── bridge.rs            # Tauri command handlers
│   ├── lib.rs               # Module exports & app setup
│   └── main.rs              # Entry point
├── Cargo.toml
└── GIT_INTEGRATION_README.md  # This file
```

## Notes

- All commands return `ApiResponse<T>` envelope for consistent error handling
- Repository paths should be absolute
- Watch events are emitted globally to all windows
- Backups are never automatically cleaned up (future enhancement)
