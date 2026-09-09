# Obsidian calendar Implementation Plan

**Goal:** Add text-only Obsidian task dates and a TUI month calendar to minitask.

**Architecture:** Extend the existing Store and App; use `time::Date` for arithmetic. Keep source files authoritative and rebuild versioned JSON cache once. Calendar entries refer to existing workspace/task indexes, not copied storage.

**Tech Stack:** Rust, Ratatui, Crossterm, time, existing serde and atomic file replacement.

**Spec:** `docs/superpowers/specs/2026-09-08-obsidian-calendar-design.md`, approved in chat.

## Constraints

No emoji. Preserve old Markdown and current worktree. No commits/push. Execute inline; user already approved the concrete scope.

## 1. Storage and date metadata

Files: `src/store.rs`, `src/dates.rs`, `src/lib.rs`, `Cargo.toml`, `tests/calendar.rs`, `tests/workflow.rs`.

- [x] Write and run tests showing list tasks currently fail parsing and new task creation still emits the old format.
- [x] Add strict date parsing and bracketed metadata recognition. Use `Task.scheduled`/`Task.due` plus `date_error`; include fields in cache version 2.
- [x] Extend source parsing for standard lists while retaining legacy sections. Test fences/frontmatter/comments, nested notes and byte preservation.
- [x] Implement `Store::set_date(name, selected, hash, field, Option<Date>)` using existing source hash validation and atomic save; test replacement, clear, invalid metadata and stale selections.
- [x] Run `cargo test --offline --test calendar` and existing workflows; update old cache test to assert one rebuild and unchanged source instead of v1 cache reuse.

## 2. Calendar and date picker

Files: `src/calendar.rs`, `src/ui.rs`, `tests/calendar.rs`.

- [x] Write failing real App/TestBackend workflows for `c`, `s`, `d`, date movement, month boundaries, scope switching, cancel/clear and source targeting.
- [x] Implement shared calendar navigation with `time::Date`, local today and Monday-first layout. Render day counts and date labels without emoji.
- [x] Integrate calendar/day filtering with App selection and editing; preserve picker task/hash through refresh. Show undated tasks in normal list and overdue status in list/details/calendar.
- [x] Verify small-screen behavior and editor paths from all-workspace entries.

## 3. Verification and delivery

Files: `README.md`, executable (ignored), design/plan progress.

- [x] Document Markdown, date keys, calendar scope and old-format compatibility.
- [x] Run `cargo fmt --check`, `cargo test --offline --locked`, `cargo clippy --offline --all-targets -- -D warnings`, `cargo build --offline --release --locked`.
- [x] Review storage safety and calendar interactions. Run a dedicated terminal smoke test using isolated data, including Neovim/Emacs date edits and return to TUI.
- [x] Back up and replace root executable when not running, verify version/hash and final status. Keep Yazi Ctrl+T path unchanged.
