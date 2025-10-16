# StashRoom

A modern desktop Git client built with Rust (Tauri) + React + TypeScript.

![Status](https://img.shields.io/badge/status-ready-green)
![Rust](https://img.shields.io/badge/rust-1.80%2B-orange)
![TypeScript](https://img.shields.io/badge/typescript-5.6-blue)
![Tauri](https://img.shields.io/badge/tauri-2.0-purple)

## Features

- **Three-Panel Interface**: Unstaged changes, diff viewer, staged changes + commit
- **Fast**: Virtualized lists handle 10,000+ files smoothly
- **Keyboard-First**: Navigate, stage, commit without touching the mouse
- **Hunk Staging**: Stage individual hunks or entire files
- **Safe Discard**: Automatic backups before discarding changes
- **Live Updates**: Filesystem watching with auto-refresh
- **Commits History**: Visualize all commits and changes
- **Branches management**: Manage your branches locally

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

![Main screenshot](./public/screenshot1.png)

![Main screenshot with commits history](./public/screenshot2.png)

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

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `?` | Show keyboard shortcuts help |
| `↑` / `↓` | Navigate file list |
| `Enter` | View file diff |
| `S` | Stage/unstage selected file |
| `D` | Discard changes (with backup) |
| `Esc` | Close modals |

---

**Built with ❤️ using Rust, React, and TypeScript**
