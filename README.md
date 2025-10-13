# Kite 🪁

A modern desktop Git client built with Rust (Tauri) + React + TypeScript.

![Status](https://img.shields.io/badge/status-ready-green)
![Rust](https://img.shields.io/badge/rust-1.80%2B-orange)
![TypeScript](https://img.shields.io/badge/typescript-5.6-blue)
![Tauri](https://img.shields.io/badge/tauri-2.0-purple)

## Features

- 🎯 **Three-Panel Interface**: Unstaged changes, diff viewer, staged changes + commit
- ⚡ **Fast**: Virtualized lists handle 10,000+ files smoothly
- ⌨️ **Keyboard-First**: Navigate, stage, commit without touching the mouse
- 🔍 **Hunk Staging**: Stage individual hunks or entire files
- 💾 **Safe Discard**: Automatic backups before discarding changes
- 👀 **Live Updates**: Filesystem watching with auto-refresh
- 🎨 **Dark Mode**: Tailwind CSS with dark mode support
- 🔧 **Type-Safe**: Full TypeScript coverage with Rust types

## Quick Start

```bash
# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

See [QUICKSTART.md](./QUICKSTART.md) for detailed instructions.

## Screenshots

### Main Interface
```
┌─────────────────────────────────────────────────────────┐
│ Kite                  /path/to/repo       [main] Change │
├─────────────┬──────────────────────┬────────────────────┤
│  Changes    │                      │  Staged Changes    │
│             │                      │                    │
│ • file1.rs  │  --- a/file1.rs      │ • committed.ts     │
│   file2.ts  │  +++ b/file1.rs      │                    │
│   file3.md  │  @@ -10,3 +10,4 @@   │ ┌────────────────┐ │
│             │   context line       │ │ Commit message │ │
│             │  -removed line       │ │                │ │
│             │  +added line         │ └────────────────┘ │
│             │   context line       │  [Commit] button   │
│             │  [Stage Hunk]        │                    │
└─────────────┴──────────────────────┴────────────────────┘
│ Press ? for shortcuts             HEAD: abc1234         │
└─────────────────────────────────────────────────────────┘
```

## Tech Stack

### Backend (Rust)
- **Tauri 2**: Desktop framework
- **git2 (libgit2)**: Git operations
- **notify**: Filesystem watching
- **anyhow**: Error handling

### Frontend (TypeScript/React)
- **React 18**: UI framework
- **Zustand**: State management
- **Tailwind CSS**: Styling
- **react-virtuoso**: Virtualized lists
- **react-resizable-panels**: Resizable layout
- **sonner**: Toast notifications

## Architecture

```
┌─────────────────────────────────────────────┐
│              React UI                       │
│  ┌──────────┬──────────┬──────────┐        │
│  │ChangeList│DiffPanel │StagePanel│        │
│  └────┬─────┴────┬─────┴────┬─────┘        │
│       │          │          │               │
│       └──────────┴──────────┘               │
│              Zustand Store                  │
│                   ↕                         │
│            API Wrapper (api.ts)             │
└───────────────────┬─────────────────────────┘
                    │ Tauri IPC
┌───────────────────┴─────────────────────────┐
│            Tauri Bridge                     │
│         (14 commands)                       │
│                   ↕                         │
│  ┌────────────────────────────────┐        │
│  │ Core Modules (Rust)            │        │
│  │  • repo.rs    - Repository mgmt│        │
│  │  • status.rs  - Git status     │        │
│  │  • diff.rs    - Diff generation│        │
│  │  • stage.rs   - Staging ops    │        │
│  │  • discard.rs - Discard changes│        │
│  │  • fs_watch.rs- File watching  │        │
│  └────────────┬───────────────────┘        │
│               ↕                             │
│         libgit2 (git2 crate)               │
└─────────────────────────────────────────────┘
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `?` | Show keyboard shortcuts help |
| `↑` / `↓` | Navigate file list |
| `Enter` | View file diff |
| `S` | Stage/unstage selected file |
| `D` | Discard changes (with backup) |
| `Esc` | Close modals |

## Project Structure

```
Kite/
├── src/                        # React frontend
│   ├── lib/
│   │   ├── types.ts           # TypeScript types
│   │   └── api.ts             # Tauri API wrapper
│   ├── state/
│   │   └── store.ts           # Zustand store
│   ├── components/
│   │   ├── ChangeList.tsx     # File list
│   │   ├── DiffPanel.tsx      # Diff viewer
│   │   ├── StagePanel.tsx     # Staged files + commit
│   │   └── KeyboardShortcutsHelp.tsx
│   ├── App.tsx                # Main layout
│   └── main.tsx               # Entry point
│
├── src-tauri/                  # Rust backend
│   └── src/
│       ├── core/
│       │   ├── repo.rs        # Repository registry
│       │   ├── status.rs      # Git status
│       │   ├── diff.rs        # Diff generation
│       │   ├── stage.rs       # Staging operations
│       │   ├── discard.rs     # Discard with backup
│       │   └── fs_watch.rs    # Filesystem watching
│       ├── types.rs           # Rust type definitions
│       ├── bridge.rs          # Tauri command handlers
│       └── lib.rs             # Main entry point
│
├── README.md                   # This file
├── QUICKSTART.md              # Quick start guide
├── PROJECT_STATUS.md          # Implementation status
├── IMPLEMENTATION_SUMMARY.md  # Full-stack overview
├── COMPONENT_HIERARCHY.md     # Component structure
├── FRONTEND_README.md         # Frontend details
└── src-tauri/
    └── GIT_INTEGRATION_README.md  # Backend details
```

## Documentation

- **[QUICKSTART.md](./QUICKSTART.md)** - Get started in 5 minutes
- **[PROJECT_STATUS.md](./PROJECT_STATUS.md)** - Implementation status and checklist
- **[IMPLEMENTATION_SUMMARY.md](./IMPLEMENTATION_SUMMARY.md)** - Full-stack architecture overview
- **[COMPONENT_HIERARCHY.md](./COMPONENT_HIERARCHY.md)** - Visual component structure and data flow
- **[FRONTEND_README.md](./FRONTEND_README.md)** - Frontend architecture and features
- **[src-tauri/GIT_INTEGRATION_README.md](./src-tauri/GIT_INTEGRATION_README.md)** - Backend implementation details

## Key Features

### ✅ Repository Management
- Open repository via native folder picker
- UUID-based repository registry
- Display current branch and HEAD
- Close/switch repositories

### ✅ File Status
- Show unstaged changes (modified, added, deleted, untracked)
- Show staged changes
- Real-time updates via filesystem watcher
- Virtualized lists (handles 10,000+ files)

### ✅ Diff Viewing
- Hunk-based diff display
- Color-coded additions (green) and deletions (red)
- Binary file detection
- Context menu on hunks

### ✅ Staging Operations
- Stage/unstage entire file
- Stage/unstage specific hunk
- Stage specific lines (added lines only)
- Keyboard shortcuts and double-click

### ✅ Discard Changes
- Discard entire file with confirmation
- Automatic backup to `.git/recover/<timestamp>/`
- Recovery functions available

### ✅ Committing
- Commit message textarea
- Validation (requires message + staged files)
- Auto-refresh after commit
- Shows commit OID in toast

### ✅ Filesystem Watching
- 200ms debounced events
- Filters out `.git` directory changes
- Auto-refreshes status on external changes

## Known Limitations

1. **Line Staging**: Only supports staging added lines (`+`), not removed lines (`-`)
2. **Hunk Discard**: Not yet implemented (only full-file discard works)
3. **Conflict Resolution**: Conflicted files detected but no merge UI
4. **Submodules**: Explicitly excluded from status
5. **Remote Operations**: No push/pull/fetch yet

## Development

### Run Tests
```bash
# Rust tests
cd src-tauri
cargo test

# TypeScript type checking
npm run build
```

### Code Style
```bash
# Format Rust code
cd src-tauri
cargo fmt

# Lint Rust code
cd src-tauri
cargo clippy
```

### Debug Mode
```bash
# Run with Rust logging
cd src-tauri
RUST_LOG=debug cargo run
```

## Performance

- **File Lists**: Virtualized with react-virtuoso, tested with 10,000+ files
- **Diff Rendering**: Direct hunk display (no heavy Monaco computation)
- **State Updates**: Zustand with selective re-renders
- **Watch Events**: 200ms debounced

## Type Safety

- **Rust → TypeScript**: All types mirrored exactly
- **camelCase ↔ snake_case**: Handled by serde
- **API Responses**: Wrapped in `ApiResponse<T>`
- **No `any` types**: Full TypeScript coverage

## CRLF Safety

- Line endings preserved throughout stack
- No normalization during diff or staging
- Uses libgit2 patch application
- Works correctly on Windows with CRLF files

## Future Enhancements

### High Priority
1. Syntax highlighting in diff view
2. Conflict resolution UI
3. Branch switcher/creator
4. Commit history viewer
5. Undo/redo for operations

### Medium Priority
6. Line staging for removed lines
7. Partial hunk discard
8. Search in diffs
9. Diff statistics
10. File tree view

### Low Priority
11. Blame annotations
12. Interactive rebase
13. Stash management
14. Remote operations (push/pull/fetch)
15. Settings/preferences UI

## Contributing

This project was built as a complete implementation of a modern Git client. Future contributions welcome!

## License

[Specify license]

## Acknowledgments

- **libgit2**: Core Git functionality
- **Tauri**: Desktop framework
- **React**: UI framework
- **Tailwind CSS**: Styling system

---

**Built with ❤️ using Rust, React, and TypeScript**
