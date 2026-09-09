# Obsidian Markdown and calendar

User approved this design in chat, including the correction prohibiting emoji. Continue implementation in the current initially uncommitted workspace; do not commit or push.

- New tasks use `- [ ] title [scheduled:: YYYY-MM-DD] [due:: YYYY-MM-DD]`. Both dates are optional; notes are indented under the task. This is Obsidian's checkbox syntax plus Tasks/Dataview bracketed date fields, not a core Obsidian date property.
- Read legacy `## [ ]` tasks without rewriting them. Preserve their existing notes semantics. An explicit `## Tasks` heading separates a new list section after legacy tasks; add it on first append so old note checklists are not promoted to tasks. For new list tasks, indented content is the note; nested checkboxes remain note content, not independent minitask tasks. Do not parse fenced code, frontmatter or Obsidian comments as tasks.
- Cache version increases and old indexes rebuild once. Markdown, workspace state and editor selection remain compatible. Date edits replace only the chosen field on the selected source line, checking the source hash and using the existing atomic save.
- Invalid or duplicate recognized date fields stay visible as an error on that task. Refuse date edits until ambiguous metadata is corrected; never silently drop source text. ISO dates are strictly validated, including leap days. No time-of-day or recurrence in this change.
- `c` opens/closes a Monday-first month calendar with daily counts, today/selection marks, selected-day tasks and details. `h/j/k/l` or arrows move dates; PgUp/PgDn change month; `t` goes to today; `w` switches selected workspace/all workspaces; Tab switches calendar/task focus; Esc returns to list.
- `s`/`d` open a date picker for the selected task's scheduled/due date. Calendar movement is the same; Enter saves, Delete clears, Esc cancels. The picker retains the original file hash to prevent applying an edit to a changed task. Dates are editable from both list and calendar.
- Calendar entries retain workspace and source position so completion/edit/date actions target the correct file in all-workspaces view. Day membership matches either scheduled or due date, once per task/day. Done tasks remain visible and marked. Undated tasks remain in the ordinary list. Overdue means an unfinished task with due date before local today.
- UI and generated data use no emoji. Dates are labeled `予定` and `期限`. Use the already cached `time` crate for date arithmetic and local today, rather than hand-writing Gregorian arithmetic.
- Verify parsing/save preservation, old cache invalidation, invalid dates, leap/month navigation, day filtering, all-workspace targeting, cancel/clear/stale picker behavior, small screens and real editor handoff. Build, lint and test the resulting executable before replacing it.

References: https://obsidian.md/help/syntax and https://github.com/obsidian-tasks-group/obsidian-tasks/blob/main/docs/Reference/Task%20Formats/Dataview%20Format.md
