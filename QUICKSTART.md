# Kite - Quick Start Guide

## Prerequisites

- Node.js (v18 or later)
- Rust (latest stable)
- Git

## Installation

1. **Install Frontend Dependencies**:
   ```bash
   npm install
   ```

2. **Verify Rust Installation**:
   ```bash
   cargo --version
   ```

## Running the Application

### Development Mode

```bash
npm run tauri dev
```

This will:
1. Build the Rust backend
2. Start the Vite development server
3. Open the Kite application window

First launch may take a few minutes to compile Rust dependencies.

### Production Build

```bash
npm run tauri build
```

The executable will be created in:
- Windows: `src-tauri\target\release\kite.exe`
- macOS: `src-tauri/target/release/bundle/macos/kite.app`
- Linux: `src-tauri/target/release/kite`

## First Run

1. **Open Repository**:
   - Click "Open Repository" button
   - Select any Git repository folder
   - Status loads automatically

2. **View Changes**:
   - Left panel shows unstaged changes
   - Click a file to view its diff in the middle panel
   - Right panel shows staged changes

3. **Stage Files**:
   - **Method 1**: Select file, press `S` key
   - **Method 2**: Double-click file
   - **Method 3**: Click "Stage Hunk" button in diff view
   - **Method 4**: Right-click hunk → "Stage Hunk"

4. **Commit Changes**:
   - Stage one or more files
   - Enter commit message in right panel
   - Click "Commit" button
   - Status refreshes automatically

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `?` | Show keyboard shortcuts help |
| `↑` | Previous file in list |
| `↓` | Next file in list |
| `Enter` | View file diff |
| `S` | Stage/unstage selected file |
| `D` | Discard changes (creates backup) |
| `Esc` | Close modals |

## Features to Try

### 1. File Operations
- Stage/unstage files
- View diffs for modified files
- Commit staged changes

### 2. Hunk Operations
- Stage individual hunks via context menu
- Right-click any hunk in diff view

### 3. Discard Changes
- Select an unstaged file
- Press `D` key
- Confirm discard
- Backup created in `.git/recover/`

### 4. Watch Events
- Keep app open
- Edit a file in another editor
- Watch status auto-refresh
- See toast notification

### 5. Keyboard Navigation
- Use arrow keys to navigate file list
- Press `Enter` to load diff
- Press `S` to stage/unstage
- Press `?` to see all shortcuts

## Testing with Sample Repository

If you don't have a Git repository handy:

```bash
# Create a test repository
mkdir test-repo
cd test-repo
git init
echo "Hello, Kite!" > README.md
git add README.md
git commit -m "Initial commit"

# Make some changes
echo "Testing Kite features" >> README.md
echo "Another file" > test.txt

# Open test-repo in Kite
```

You should see:
- `README.md` as modified (unstaged)
- `test.txt` as new file (untracked)

## Troubleshooting

### Build Errors

**Issue**: Rust compilation fails
```bash
# Clean and rebuild
cd src-tauri
cargo clean
cargo build
```

**Issue**: Frontend build fails
```bash
# Clear cache and reinstall
rm -rf node_modules package-lock.json
npm install
```

### Runtime Issues

**Issue**: "Repository not found"
- Ensure the selected folder is a valid Git repository
- Check that `.git` folder exists

**Issue**: Watch events not working
- Watch events require repository to be open
- Check console for subscription errors

**Issue**: Cannot stage files
- Ensure files are in "Changes" (unstaged) panel
- Check Git repository is not in a conflicted state

### Common Questions

**Q: Where are discarded changes backed up?**
A: In `.git/recover/<timestamp>/<file-path>` within the repository.

**Q: Can I stage individual lines?**
A: Yes, but only added lines (`+` lines). Removed lines cannot be staged individually (documented limitation).

**Q: Does it support CRLF line endings?**
A: Yes! Line endings are preserved exactly throughout the stack.

**Q: Can I undo a commit?**
A: Not yet - this feature is planned for future releases.

## Development Commands

```bash
# Run frontend only (without Tauri)
npm run dev

# Build frontend
npm run build

# Run Rust tests
cd src-tauri
cargo test

# Run Rust with logging
cd src-tauri
RUST_LOG=debug cargo run

# Format Rust code
cd src-tauri
cargo fmt

# Lint Rust code
cd src-tauri
cargo clippy
```

## Performance Notes

- **Large repositories**: Tested with 10,000+ files using virtualized lists
- **Diff rendering**: Hunk-based display for fast rendering
- **Watch events**: 200ms debounced to avoid excessive updates
- **Memory usage**: UUID-based registry keeps repositories in memory

## What Works

✅ Open/close repositories
✅ View file status (staged/unstaged/untracked)
✅ View file diffs (hunk-based)
✅ Stage/unstage files
✅ Stage/unstage hunks
✅ Discard changes with backup
✅ Commit with message
✅ Filesystem watching
✅ Keyboard shortcuts
✅ Dark mode
✅ Resizable panels
✅ Binary file detection

## Known Limitations

1. **Line staging**: Only added lines supported
2. **Hunk discard**: Not implemented (only full-file discard)
3. **Conflict resolution**: No merge UI yet
4. **Submodules**: Not shown in status
5. **Remote operations**: No push/pull/fetch yet

## Next Steps

After trying the basic features:

1. Read `IMPLEMENTATION_SUMMARY.md` for architecture overview
2. Check `COMPONENT_HIERARCHY.md` for component structure
3. See `FRONTEND_README.md` for frontend details
4. See `src-tauri/GIT_INTEGRATION_README.md` for backend details

## Getting Help

- Check `PROJECT_STATUS.md` for implementation status
- All code includes inline documentation
- Unit tests in `src-tauri/src/core/*_tests`

## Enjoy using Kite! 🪁

A modern Git client built with Rust + React + TypeScript.
