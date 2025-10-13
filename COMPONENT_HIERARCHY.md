# Component Hierarchy

## Visual Structure

```
App
├── Toaster (sonner)
├── KeyboardShortcutsHelp (modal)
└── Main Layout
    ├── Header
    │   ├── Title + Path
    │   └── Branch Badge + Change Repo Button
    │
    ├── Progress Bar (conditional)
    │
    ├── PanelGroup (horizontal)
    │   ├── Panel (Left - 25%)
    │   │   ├── Section Header ("Changes")
    │   │   └── ChangeList (type="unstaged")
    │   │       └── Virtuoso
    │   │           └── StatusEntry Items
    │   │
    │   ├── PanelResizeHandle
    │   │
    │   ├── Panel (Middle - 50%)
    │   │   └── DiffPanel
    │   │       ├── Header (file path + badges)
    │   │       ├── Hunk List
    │   │       │   └── DiffHunk
    │   │       │       ├── Hunk Header + Stage Button
    │   │       │       └── DiffLine Items
    │   │       └── Context Menu (conditional)
    │   │
    │   ├── PanelResizeHandle
    │   │
    │   └── Panel (Right - 25%)
    │       └── StagePanel
    │           ├── Section Header ("Staged Changes")
    │           ├── ChangeList (type="staged")
    │           └── Commit Box
    │               ├── Textarea (message)
    │               ├── Commit Button
    │               └── Branch/HEAD Info
    │
    └── Footer
        ├── Keyboard Shortcuts Hint
        └── HEAD Commit
```

## Component Communication

### Data Flow

```
Zustand Store (Central State)
    ↓ ↑
┌───────────┴───────────┐
│                       │
App                     │
│                       │
├─ ChangeList ─────────┤
│  (subscribes to:     │
│   - entries          │
│   - selectedPath)    │
│                      │
├─ DiffPanel ──────────┤
│  (subscribes to:     │
│   - currentDiff      │
│   - selectedPath)    │
│                      │
└─ StagePanel ─────────┤
   (subscribes to:     │
    - stagedEntries    │
    - repo.head)       │
                       │
   API Wrapper ────────┘
   (Tauri IPC)
```

### State Slices

```typescript
AppStore {
  // Repo Slice
  repo: RepoOpenResponse | null
  openRepo(path): Promise<void>
  closeRepo(): Promise<void>

  // Status Slice
  entries: StatusEntry[]
  refreshStatus(repoId): Promise<void>
  updateStatus(status): void
  getStagedEntries(): StatusEntry[]
  getUnstagedEntries(): StatusEntry[]

  // Diff Slice
  currentDiff: FileDiff | null
  loadDiff(repoId, path, side): Promise<void>
  clearDiff(): void

  // Selection Slice
  selectedPath: string | null
  selectedHunkIndex: number | null
  selectedLineIndices: number[]
  setSelectedPath(path): void
  setSelectedHunk(index): void
  setSelectedLines(indices): void
  clearSelection(): void

  // UI Slice
  isLoading: boolean
  isOperating: boolean
  setIsOperating(value): void
}
```

## Event Flow Examples

### Opening a Repository

```
User clicks "Open Repository"
  → App.handleOpenFolder()
  → Tauri Dialog opens
  → User selects folder
  → store.openRepo(path)
  → api.openRepo({ path })
  → Tauri IPC → Rust bridge → core::repo
  → Returns RepoOpenResponse
  → Store updates: { repo, entries }
  → api.subscribeWatch(repoId)
  → api.onWatchEvent(callback)
  → All components re-render with new state
```

### Staging a File

```
User presses 'S' key in ChangeList
  → ChangeList.handleStageToggle(entry)
  → store.setIsOperating(true)
  → api.stageFile(repoId, path)
  → Tauri IPC → Rust bridge → core::stage
  → Returns updated StatusMatrix
  → store.updateStatus(newStatus)
  → store.setIsOperating(false)
  → ChangeList re-renders with new entries
  → File moves from unstaged to staged panel
  → Toast notification appears
```

### Viewing a Diff

```
User clicks file in ChangeList
  → ChangeList.handleSelect(entry)
  → store.setSelectedPath(path)
  → store.loadDiff(repoId, path, 'working')
  → api.getDiff({ repoId, path, side: 'working' })
  → Tauri IPC → Rust bridge → core::diff
  → Returns FileDiff with hunks and lines
  → store.currentDiff = fileDiff
  → DiffPanel re-renders with diff content
```

### Watch Event

```
File changes on disk
  → notify (Rust) detects change
  → Debounced 200ms
  → Coalesced with other changes
  → emit("watch-event", WatchEvent)
  → Frontend api.onWatchEvent receives event
  → store.refreshStatus(repoId)
  → api.getStatus(repoId)
  → Returns new StatusMatrix
  → store.entries = newEntries
  → ChangeList re-renders
  → Toast notification (optional)
```

### Committing

```
User enters message and clicks "Commit"
  → StagePanel.handleCommit()
  → Validation (message + staged files)
  → store.setIsOperating(true)
  → api.commit({ repoId, message })
  → Tauri IPC → Rust bridge → core::commit
  → Returns CommitResponse with OID
  → store.clearSelection()
  → store.refreshStatus(repoId)
  → StagePanel clears message
  → store.setIsOperating(false)
  → Toast: "Committed abc1234"
  → Staged list empties
```

## Props Flow

### ChangeList
```typescript
interface ChangeListProps {
  type: 'unstaged' | 'staged';
}

// Subscribes to:
- store.repo
- store.entries (filtered by type)
- store.selectedPath
- store.setSelectedPath
- store.loadDiff
- store.refreshStatus
- store.setIsOperating
```

### DiffPanel
```typescript
// No props (uses store directly)

// Subscribes to:
- store.repo
- store.currentDiff
- store.selectedPath
- store.setIsOperating
- store.updateStatus
- store.loadDiff
```

### StagePanel
```typescript
// No props (uses store directly)

// Subscribes to:
- store.repo
- store.getStagedEntries
- store.setIsOperating
- store.refreshStatus
- store.clearSelection
```

### KeyboardShortcutsHelp
```typescript
// No props
// Self-contained modal with internal state
```

## Keyboard Event Handling

```
Window.addEventListener('keydown')
  │
  ├── ChangeList (when .change-list focused)
  │   ├── ↑ → selectPrevious()
  │   ├── ↓ → selectNext()
  │   ├── Enter → loadDiff()
  │   ├── S → toggleStage()
  │   └── D → discard()
  │
  └── KeyboardShortcutsHelp (global)
      ├── ? → show modal
      └── Esc → hide modal
```

## Context Menu

```
DiffPanel
  └── Hunk (right-click)
      → onContextMenu(e, hunkIndex)
      → setContextMenu({ x, y, hunkIndex })
      → Render <div> at (x, y)
          ├── "Stage Hunk" → handleStageHunk(hunkIndex)
          └── "Unstage Hunk" → handleStageHunk(hunkIndex, true)
```

## Styling Architecture

```
Tailwind CSS
├── Colors
│   ├── Gray (backgrounds, borders, text)
│   ├── Blue (primary, selection, links)
│   ├── Green (additions, new files)
│   ├── Red (deletions, removed files)
│   └── Yellow (modifications)
│
├── Layout
│   ├── Flexbox (main layout)
│   ├── Grid (not used yet)
│   └── Absolute (modals, context menu)
│
├── Spacing
│   ├── px-N, py-N (padding)
│   ├── mx-N, my-N (margin)
│   └── gap-N (flex/grid gaps)
│
├── Typography
│   ├── text-N (sizes)
│   ├── font-mono (code/paths)
│   └── font-semibold/bold (headings)
│
├── Dark Mode
│   └── dark: variants for all colors
│
└── Animations
    ├── animate-pulse (progress bar)
    └── transition-colors (hover states)
```

## Dependencies Graph

```
App
 ├─ react-resizable-panels (layout)
 ├─ sonner (notifications)
 └─ state/store (Zustand)

ChangeList
 ├─ react-virtuoso (virtualization)
 └─ state/store

DiffPanel
 ├─ @monaco-editor/react (not used yet)
 └─ state/store

StagePanel
 ├─ ChangeList
 └─ state/store

KeyboardShortcutsHelp
 └─ React (useState, useEffect)

state/store
 ├─ zustand
 ├─ lib/api
 └─ sonner

lib/api
 └─ @tauri-apps/api (core, event)

lib/types
 └─ (pure TypeScript)
```
