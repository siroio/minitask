# Expectation and CLI implementation

User approved implementing lightweight dated milestones and CLI information access.

Constraints: Rust, no additional dependencies, no emoji, workspace directories, one entity per Markdown, Neovim/Emacs editing, existing tasks preserved. No commit or push.

1. Add runnable integration checks in tests/cli.rs for JSON queries, workspace filtering, entity lookup, invalid arguments, and reading while the TUI lock is held. Confirm they fail with the current binary.
2. Extend the existing inline date parser and Task index with `[expectation:: YYYY-MM-DD]`. An Expectation uses a checkbox, a title, an optional freeform body, and its own date; no task decomposition or completion condition is required. Preserve task date behavior and exclude milestones from ordinary task counts.
3. Add read-only Store::snapshot and src/cli.rs. Commands: workspaces, tasks, expectations, resume, show. Arguments: --workspace, --query, --id, --status, --from, --to, --json. Output a versioned JSON object with stable source identity, source hash, readable dates, and absolute paths. Do not recover migrations or write caches during queries; report pending migration instead.
4. Reuse the TUI list, add form, date picker, calendar, and editor for milestones. Add Horizon view and a Resume entry point; retain existing keyboard workflows. Validate calendar targeting and new milestone creation via simulated key events.
5. Document commands and Markdown with examples for LLM callers. Run fmt, tests including both editors, clippy, release build, CLI and terminal smoke checks, review, then replace the installed binary after preserving a backup.

User clarified full Creative Memory scope: implement Note, Action states/Completion Condition/Supplement, Direction, Question, Resume, Expectation and CLI. Added a shared kind/state table, read-only JSON queries, guarded CLI writes, per-kind TUI views/forms and a portable LLM workflow guide.
