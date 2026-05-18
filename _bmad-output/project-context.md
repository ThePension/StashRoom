---
project_name: 'StashRoom'
user_name: 'Thepension'
date: '2026-05-18'
sections_completed: ['technology_stack', 'language_rules', 'framework_rules', 'testing_rules', 'quality_rules', 'workflow_rules', 'anti_patterns']
status: 'complete'
rule_count: 32
optimized_for_llm: true
---

# Project Context for AI Agents

_This file contains critical rules and patterns that AI agents must follow when implementing code in this project. Focus on unobvious details that agents might otherwise miss._

---

## Technology Stack & Versions

**Desktop Framework:** Tauri v2 (Rust backend + Web frontend via WebView)
**Frontend:** React 18.3.1, TypeScript ~5.6.2, Vite ^6.0.3
**Styling:** Tailwind CSS ^3.4.18 — dark mode via `class` strategy
**State:** Zustand ^5.0.8
**Backend:** Rust edition 2021, git2 0.19, tokio v1 (rt-multi-thread)
**Persistence:** @tauri-apps/plugin-store ^2.4.0
**UI libs:** @monaco-editor/react ^4.7.0, lucide-react ^0.546.0,
            sonner ^2.0.7, react-virtuoso ^4.14.1,
            react-resizable-panels ^3.0.6
**Test deps (Rust):** tempfile 3.8, insta 1.34

## Critical Implementation Rules

### Language-Specific Rules

**TypeScript:**
- Strict mode ON: `noUnusedLocals`, `noUnusedParameters`, `noFallthroughCasesInSwitch` — all unused vars/params are compile errors
- `isolatedModules: true` — each file must be independently compilable; avoid type-only re-exports without `export type`
- `moduleResolution: "bundler"` — use bare imports, no `.js` extensions
- All Tauri API calls go through `src/lib/api.ts` (the `GitAPI` class) — never call `invoke()` directly from components
- All shared types live in `src/lib/types.ts` — do not define API-related types elsewhere

**Rust:**
- `git2::Repository` is **not** `Send` — any async command needing blocking git ops must use `tokio::task::spawn_blocking` and reopen the repository inside the closure
- All Tauri commands return `ApiResponse<T>` — never return raw values or `Result<T, E>` directly from `#[tauri::command]` functions (except async ones that return `Result<ApiResponse<T>, String>`)
- Serde serialization: all Rust structs use `#[serde(rename_all = "camelCase")]` — JSON field names are camelCase even though Rust fields are snake_case
- Enums use `#[serde(rename_all = "lowercase")]` or `#[serde(rename_all = "camelCase")]` — check existing types before adding new enums

### Framework-Specific Rules

**React:**
- Components are function components with hooks — no class components
- State selectors with Zustand: always use granular selectors (`useStore((s) => s.fieldName)`) to avoid unnecessary re-renders — do NOT call `useStore()` with no selector
- UI-triggered side effects (loading data, etc.) go through store actions, not directly from components
- Icons: use `lucide-react` exclusively — do not add other icon libraries
- Toasts: use `sonner` (`toast.success`, `toast.error`) — do not use other notification systems

**Tauri IPC Bridge:**
- New backend features require 4 steps: (1) add Rust command in `src-tauri/src/bridge.rs`, (2) register in `lib.rs` invoke_handler, (3) add method to `GitAPI` class in `src/lib/api.ts`, (4) add types to both `src/lib/types.ts` AND `src-tauri/src/types.rs`
- Command parameter serialization: TypeScript camelCase ↔ Rust snake_case is handled automatically by serde — do not manually convert
- Only use `async fn` + `spawn_blocking` when git2 blocking operations are needed; synchronous commands return `ApiResponse<T>` directly

**Zustand Store:**
- Store is organized in logical slices (RepoState, StatusState, DiffState, SelectionState, UIState, SettingsState, HistoryState) but all merged into a single flat `AppStore` in `src/state/store.ts`
- Add new state to the appropriate existing slice — do not create new store files
- `isLoading` is shared at the top level — use `silent: true` pattern for background refreshes that should not trigger a spinner (see `refreshStatus`)
- After any mutating operation (stage, unstage, commit, discard), always refresh status via `refreshStatus(repoId)`

**Persistence:**
- Settings and workspace are persisted via `@tauri-apps/plugin-store` with 300ms debounce
- Store files: `workspace.json` (open repos + active repo) and `settings.json` (UI preferences)
- All persistence logic is in `src/lib/persistence.ts` — do not use localStorage or other mechanisms

### Testing Rules

**Rust (only test suite currently present):**
- Test files are in `#[cfg(test)]` modules within each source file — not separate test files
- Use `tempfile::TempDir` for all tests needing a real git repository — never hardcode paths
- Always create a proper initial commit in test repos (empty repos without HEAD cause many git2 operations to fail)
- Keep `TempDir` alive for the duration of the test by binding it to a variable — if dropped, the directory is deleted
- Use `insta` for snapshot testing of complex output structures

**Frontend:**
- No frontend test framework is currently configured — do not add test files or infrastructure without explicit instruction

### Code Quality & Style Rules

**Naming Conventions:**
- TypeScript: PascalCase for components and interfaces, camelCase for variables/functions
- React component files: PascalCase (`ChangeList.tsx`, `DiffPanel.tsx`)
- Utility/lib files: camelCase (`api.ts`, `persistence.ts`, `treeUtils.ts`)
- Custom hooks: camelCase with `use` prefix (`useConfirm.tsx`)
- Rust: snake_case for functions/variables/modules, PascalCase for structs/enums

**File Organization:**
- React components: `src/components/` (one file = one primary component)
- Custom hooks: `src/hooks/`
- Shared types: `src/lib/types.ts` only — do not define API types elsewhere
- Utilities: `src/lib/`
- Store: `src/state/store.ts` — single file, do not split
- Rust core modules: `src-tauri/src/core/` (one module per domain: diff, discard, fs_watch, history, repo, stage, status)

**Code Style:**
- No explicit linter/formatter configured on the frontend — match existing style
- Avoid `console.log` — use `console.error` only for unexpected errors
- No JSDoc on functions unless logic is non-obvious
- Rust: use `anyhow::Result` for error propagation in `core/` modules

### Development Workflow Rules

**Commands:**
- `npm run dev` — starts Vite dev server (port 1420, strict — fails if occupied)
- `npm run tauri` — alias for the Tauri CLI
- `cargo test` — Rust tests only (run from `src-tauri/`)
- Build: `tsc && vite build` then Tauri packaging

**Tauri Architecture:**
- The frontend cannot access the filesystem directly — everything goes through Tauri commands
- Real-time events (filesystem watch) use `listen('watch-event', ...)` on the frontend and `app_handle.emit(...)` on the Rust side
- Tauri capabilities are defined in `src-tauri/capabilities/default.json` — any new plugin API requires a capabilities update

**No enforced Git/PR conventions** documented in the codebase.

### Critical Don't-Miss Rules

**Anti-patterns to avoid:**
- Never call `invoke()` directly from a component — always go through `api.*` in `src/lib/api.ts`
- Never use `git2::Repository` in an async context without `spawn_blocking` — causes a compile error (non-Send)
- Never let `TempDir` go unbound in Rust tests — the directory is deleted immediately and tests fail silently
- Never add Tailwind dark mode styles without the `dark:` prefix — dark mode is driven by the `.dark` class on `<html>`, not CSS `prefers-color-scheme`
- Never call `useStore()` without a selector — causes re-renders on every store change

**Critical edge cases:**
- Empty repository (no initial commit): many git2 operations fail on `HEAD` — tests must always create an initial commit
- `spawn_blocking` closures must reopen the repo via `git2::Repository::open` — `Repository` cannot be captured from the outer scope
- Closing the last repo is blocked by the store (`repos.length <= 1`) — do not bypass this guard
- `contextLines: 'all'` in settings is converted to `null` for the API (`contextLinesParam`) — never send the string `'all'` to the backend

**Security:**
- File paths always come from the user (open dialog) — validate on the Rust side before any filesystem operation
- Backup files from discarded changes are stored inside the repo directory — do not expose internal backup paths directly to the user

---

## Usage Guidelines

**For AI Agents:**
- Read this file before implementing any code in this project
- Follow ALL rules exactly as documented
- When in doubt, prefer the more restrictive option
- The 4-step Tauri bridge checklist is mandatory for any new backend feature

**For Humans:**
- Keep this file lean and focused on agent needs
- Update when the technology stack or patterns evolve
- Remove rules that become obvious over time

_Last Updated: 2026-05-18_
