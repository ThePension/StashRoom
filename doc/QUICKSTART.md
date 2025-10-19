# StashRoom - Quick Start Guide

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
3. Open the StashRoom application window

First launch may take a few minutes to compile Rust dependencies.

### Production Build

```bash
npm run tauri build
```

The executable will be created in:
- Windows: `src-tauri\target\release\StashRoom.exe`
- macOS: `src-tauri/target/release/bundle/macos/StashRoom.app`
- Linux: `src-tauri/target/release/StashRoom`

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

## Enjoy using StashRoom!

A modern Git client built with Rust + React + TypeScript.
