# New Features Ideas

* **Diff Search:** Find text within diff

* **Syntax Highlighting:** Monaco with language detection
* Improve branching management
* Stashes management
* Stage all unstaged files
* There is no feature to discard a hunk ?

* Backups :
    * Add auto backup purge (remove all files that are older than X -> Let the user define that parameter ?)

* Restore to a specific commit
* Update a commit ?
* Remove specific file from history ?

* Secrets & Large Files Guard (pre-commit, fast)

* In the settings :
    * Add a toggle to activate the discard backup

* Search file in commit history :
    * Add lazy loading when user scrolls
    * Expend search also in the current staging context

* Select multiple files on the left panel, and stage them all

* Create a new branch
    * And select from which branch

Commit Stack View (local) :
* Manage a stack of local commits (like mini PRs): reword/reorder/squash with preview, and a safety “dry-run rebase” check that reports conflicts ahead of time.

Settings :

- Word wrap - Default state for line wrapping (you already have a toggle per-file, but could set default)
- Syntax highlighting - Enable/disable code highlighting in diffs
- Ignore whitespace - Option to ignore whitespace when displaying diffs
- Side-by-side view - Alternative diff layout (currently only unified view)
- Sign commits - Enable GPG signing
- Author name/email - Override git config for this session
- Keyboard shortcuts - Customize keybindings
- File list limit - Max files to show (for huge repos)
- Diff size limit - Skip diffs for very large files
- Terminal application - Choose which terminal to open