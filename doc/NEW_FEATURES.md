# New Features Ideas

* Handling multiple projects
* **Diff Search:** Find text within diff

* Search file in commit history :
    * Add lazy loading when user scrolls
    * Expend search also in the current staging context

* **Syntax Highlighting:** Monaco with language detection
* Improve branching management
* Feature to display the whole file, and not only hunks (or maybe a param to define the number or lines around the hunks)
* Stashes management
* Stage all unstaged files
* Detect "renamed" files
* Detect "moved" files
* Hierarchical/Tree view of the modified files

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