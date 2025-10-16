# StashRoom Frontend Implementation

## Overview

A complete React + TypeScript frontend for the StashRoom Git client, featuring a modern three-panel layout with Zustand state management, virtualized lists, and Monaco-based diff viewing.

## Architecture

### Technology Stack

- **React 18** with TypeScript
- **Zustand** for state management
- **Tailwind CSS** for styling (no custom CSS)
- **react-virtuoso** for virtualized file lists
- **@monaco-editor/react** for diff display
- **react-resizable-panels** for resizable layout
- **sonner** for toast notifications
- **Tauri 2** for desktop integration

### Project Structure

```
src/
├── lib/
│   ├── types.ts          # TypeScript type definitions (mirrors Rust types)
│   └── api.ts            # Tauri API wrapper with type safety
├── state/
│   └── store.ts          # Zustand store with repo/status/diff/selection slices
├── components/
│   ├── ChangeList.tsx    # Virtualized file list with keyboard shortcuts
│   ├── DiffPanel.tsx     # Hunk-based diff viewer with context menu
│   └── StagePanel.tsx    # Staged files + commit box
├── App.tsx               # Main app layout with 3 resizable panels
├── main.tsx              # Entry point
└── index.css             # Tailwind directives
```