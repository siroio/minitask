use std::fs;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use minitask::dates::{DateField, parse_date};

#[test]
fn calendar_clamps_month_ends_and_stays_inside_four_digit_years() -> anyhow::Result<()> {
    use minitask::calendar::Calendar;
    for (start, key, expected) in [
        ("2027-01-31", KeyCode::PageDown, "2027-02-28"),
        ("2028-01-31", KeyCode::PageDown, "2028-02-29"),
        ("2028-12-31", KeyCode::PageDown, "2029-01-31"),
        ("2028-01-31", KeyCode::PageUp, "2027-12-31"),
        ("0001-01-01", KeyCode::Left, "0001-01-01"),
        ("9999-12-31", KeyCode::PageDown, "9999-12-31"),
    ] {
        let mut calendar = Calendar::new(parse_date(start)?);
        assert!(calendar.navigate(key));
        assert_eq!(calendar.selected, parse_date(expected)?);
    }
    Ok(())
}
use minitask::{store::Store, ui::App};
use ratatui::{Terminal, backend::TestBackend};
use tempfile::tempdir;

#[test]
fn date_edits_preserve_source_and_reject_stale_or_invalid_fields() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let mut store = Store::open(dir.path())?;
    store.create_workspace("work")?;
    let original = "\u{feff}# work\r\n\r\n- [ ] 日本語 `sample [due:: 0000-00-00]` [scheduled:: 2028-02-29] [due:: 2028-03-01] #tag ^block-id\r\n  untouched notes\r\n";
    fs::write(store.path("work"), original)?;
    store.refresh(true, None)?;
    let index = store.files["work"].clone();
    assert_eq!(index.tasks[0].scheduled, Some(parse_date("2028-02-29")?));
    assert_eq!(index.tasks[0].due, Some(parse_date("2028-03-01")?));
    assert!(index.tasks[0].date_error.is_empty());
    store.set_date(
        "work",
        &index.tasks[0],
        DateField::Due,
        Some(parse_date("2028-03-05")?),
    )?;
    let updated = original.replace("[due:: 2028-03-01]", "[due:: 2028-03-05]");
    assert_eq!(fs::read_to_string(store.path("work"))?, updated);
    assert!(
        store
            .set_date("work", &index.tasks[0], DateField::Due, None)
            .is_err()
    );
    let index = store.files["work"].clone();
    store.set_date("work", &index.tasks[0], DateField::Scheduled, None)?;
    assert_eq!(
        fs::read_to_string(store.path("work"))?,
        updated.replace("[scheduled:: 2028-02-29]", "")
    );
    let index = store.files["work"].clone();
    store.set_date(
        "work",
        &index.tasks[0],
        DateField::Scheduled,
        Some(parse_date("2028-02-28")?),
    )?;
    assert!(
        fs::read_to_string(store.path("work"))?.contains("[scheduled:: 2028-02-28] ^block-id\r\n")
    );
    for invalid in [
        "2026-02-29",
        "2028-02-30",
        "2028-13-01",
        "2028-1-01",
        "0000-01-01",
        "2028-02-29suffix",
    ] {
        assert!(parse_date(invalid).is_err(), "{invalid}");
    }
    for fields in [
        "[due:: 2026-02-29]",
        "[due:: 2028-03-01] [due:: 2028-03-02]",
        "[scheduled:: 2028-03-01",
    ] {
        let text = format!("- [ ] invalid {fields}\n");
        fs::write(store.path("work"), &text)?;
        store.refresh(true, None)?;
        let index = store.files["work"].clone();
        assert!(!index.tasks[0].date_error.is_empty());
        assert!(
            store
                .set_date("work", &index.tasks[0], DateField::Due, None)
                .is_err()
        );
        assert_eq!(fs::read_to_string(store.path("work"))?, text);
    }
    Ok(())
}

fn key(app: &mut App, key: KeyCode) -> anyhow::Result<()> {
    app.event(Event::Key(KeyEvent::new(key, KeyModifiers::NONE)))?;
    Ok(())
}

fn screen(app: &mut App, width: u16, height: u16) -> anyhow::Result<String> {
    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    terminal.draw(|frame| app.draw(frame))?;
    let buffer = terminal.backend().buffer();
    let mut output = String::new();
    for y in 0..height {
        let mut x = 0;
        while x < width {
            let symbol = buffer[(x, y)].symbol();
            output.push_str(symbol);
            x += unicode_width::UnicodeWidthStr::width(symbol).max(1) as u16;
        }
        output.push('\n');
    }
    Ok(output)
}

#[test]
fn obsidian_lists_keep_notes_and_ignore_non_task_blocks() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let mut store = Store::open(dir.path())?;
    store.create_workspace("work")?;
    let original = "\u{feff}---\r\nexample: |\r\n  - [ ] yaml example\r\n---\r\n# work\r\n\r\n- [ ] 日本語 [scheduled:: 2028-02-29] [due:: 2028-03-01]\r\n  詳細メモ\r\n  - [ ] nested checklist\r\n  ```md\r\n  - [x] code sample\r\n  ```\r\n\r\n## unrelated section\r\nordinary text\r\n%%\r\n- [ ] hidden\r\n%%\r\n- [x] Finished\r\n  final note\r\n\r\n## [ ] Legacy\r\nlegacy notes\r\n";
    fs::write(store.path("work"), original)?;
    store.refresh(true, None)?;
    let index = store.files["work"].clone();
    assert_eq!(index.tasks.len(), 3);
    assert_eq!(index.tasks[0].title, "日本語");
    assert_eq!(index.tasks[0].line, 7);
    assert!(index.tasks[0].notes.contains("nested checklist"));
    assert!(!index.tasks[0].notes.contains("unrelated section"));
    assert_eq!(index.tasks[2].notes, "legacy notes");
    store.toggle("work", &index.tasks[0])?;
    assert_eq!(
        fs::read_to_string(store.path("work"))?,
        original.replacen("- [ ] 日本語", "- [x] 日本語", 1)
    );
    Ok(())
}

#[test]
fn newly_created_tasks_use_obsidian_checkboxes() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let mut store = Store::open(dir.path())?;
    store.create_workspace("work")?;
    store.add_task("work", "Release [due:: 2026-09-12]")?;
    assert_eq!(
        fs::read_to_string(store.task_path("work", &store.files["work"].tasks[0])?)?,
        "- [ ] Release [due:: 2026-09-12]\n\n## 詳細\n\n"
    );
    assert_eq!(store.files["work"].tasks[0].title, "Release");
    Ok(())
}

#[test]
fn adding_obsidian_tasks_does_not_promote_legacy_note_checklists() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let mut store = Store::open(dir.path())?;
    store.create_workspace("work")?;
    let original = "# work\n\n## [ ] Legacy\nNotes\n- [ ] checklist in legacy notes\n\n## [x] Finished legacy\n- [ ] another note checklist\n";
    fs::write(store.path("work"), original)?;
    store.refresh(true, None)?;
    assert_eq!(store.files["work"].tasks.len(), 2);
    assert!(
        store.files["work"].tasks[0]
            .notes
            .contains("- [ ] checklist")
    );
    store.add_task("work", "New format")?;
    store.add_task("work", "Next task")?;
    let text = fs::read_to_string(store.path("work"))?;
    assert_eq!(text, original);
    assert_eq!(store.files["work"].tasks.len(), 4);
    assert!(
        store.files["work"]
            .tasks
            .iter()
            .find(|t| t.title == "Finished legacy")
            .unwrap()
            .notes
            .contains("another note checklist")
    );
    Ok(())
}

#[test]
fn inline_comments_and_code_do_not_hide_visible_tasks_or_dates() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let mut store = Store::open(dir.path())?;
    store.create_workspace("work")?;
    let text = "- [ ] Show `%%` marker [due:: 2028-03-01]\n- [ ] Visible %%hidden [due:: 1900-01-01]%% [due:: 2028-03-02]\n- [ ] Escaped \\%% marker [due:: 2028-03-03]\n%%\n- [ ] Hidden\n%%\n- [ ] Last\n";
    fs::write(store.path("work"), text)?;
    store.refresh(true, None)?;
    assert_eq!(store.files["work"].tasks.len(), 4);
    let index = store.files["work"].clone();
    assert!(index.tasks[1].date_error.is_empty());
    assert_eq!(index.tasks[1].due, Some(parse_date("2028-03-02")?));
    store.set_date(
        "work",
        &index.tasks[1],
        DateField::Due,
        Some(parse_date("2028-03-05")?),
    )?;
    assert_eq!(
        fs::read_to_string(store.path("work"))?,
        text.replace("[due:: 2028-03-02]", "[due:: 2028-03-05]")
    );
    Ok(())
}

#[test]
fn missing_date_is_inserted_before_trailing_comments_and_block_id() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let mut store = Store::open(dir.path())?;
    store.create_workspace("work")?;
    for original in [
        "- [ ] Visible %% private notes\ncontinued\n%%\n",
        "- [ ] Visible ^block-id %% private %%\n",
    ] {
        fs::write(store.path("work"), original)?;
        store.refresh(true, None)?;
        let index = store.files["work"].clone();
        store.set_date(
            "work",
            &index.tasks[0],
            DateField::Due,
            Some(parse_date("2028-03-01")?),
        )?;
        assert_eq!(
            store.files["work"].tasks[0].due,
            Some(parse_date("2028-03-01")?)
        );
        assert_eq!(
            fs::read_to_string(store.path("work"))?,
            original.replacen("Visible", "Visible [due:: 2028-03-01]", 1)
        );
    }
    Ok(())
}

#[test]
fn calendar_can_open_without_workspaces_and_return_to_list() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let mut app = App::new(Store::open(dir.path())?, "nvim".into());
    key(&mut app, KeyCode::Char('c'))?;
    assert!(screen(&mut app, 100, 30)?.contains("月間 "));
    for (w, h) in [(8, 3), (40, 12), (80, 24)] {
        screen(&mut app, w, h)?;
    }
    key(&mut app, KeyCode::Esc)?;
    assert!(!screen(&mut app, 100, 30)?.contains("月間 "));
    Ok(())
}

#[test]
fn picker_moves_across_leap_day_and_months_and_can_cancel_or_clear() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let mut store = Store::open(dir.path())?;
    store.create_workspace("work")?;
    store.add_task("work", "Leap task [due:: 2028-02-28]")?;
    let path = store.task_path("work", &store.files["work"].tasks[0])?;
    let original = fs::read(&path)?;
    let mut app = App::new(store, "nvim".into());
    key(&mut app, KeyCode::Char('0'))?;
    key(&mut app, KeyCode::Char('d'))?;
    assert!(screen(&mut app, 100, 30)?.contains("2028-02"));
    key(&mut app, KeyCode::Right)?;
    key(&mut app, KeyCode::Enter)?;
    assert_eq!(
        app.store.files["work"].tasks[0].due,
        Some(parse_date("2028-02-29")?)
    );
    key(&mut app, KeyCode::Char('d'))?;
    key(&mut app, KeyCode::Right)?;
    key(&mut app, KeyCode::Enter)?;
    assert_eq!(
        app.store.files["work"].tasks[0].due,
        Some(parse_date("2028-03-01")?)
    );
    key(&mut app, KeyCode::Char('d'))?;
    key(&mut app, KeyCode::Left)?;
    key(&mut app, KeyCode::PageUp)?;
    key(&mut app, KeyCode::Enter)?;
    assert_eq!(
        app.store.files["work"].tasks[0].due,
        Some(parse_date("2028-01-29")?)
    );
    let before_cancel = fs::read(&path)?;
    key(&mut app, KeyCode::Char('d'))?;
    key(&mut app, KeyCode::Delete)?;
    assert!(app.store.files["work"].tasks[0].due.is_none());
    key(&mut app, KeyCode::Char('s'))?;
    key(&mut app, KeyCode::Right)?;
    key(&mut app, KeyCode::Esc)?;
    assert!(app.store.files["work"].tasks[0].scheduled.is_none());
    assert_ne!(original, before_cancel);
    key(&mut app, KeyCode::Char('d'))?;
    fs::write(&path, "- [ ] external replacement\n")?;
    assert!(key(&mut app, KeyCode::Enter).is_err());
    assert_eq!(fs::read_to_string(&path)?, "- [ ] external replacement\n");
    Ok(())
}

#[test]
fn calendar_scope_and_actions_target_the_selected_workspace() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let mut store = Store::open(dir.path())?;
    for name in ["alpha", "beta"] {
        store.create_workspace(name)?;
    }
    store.add_task("alpha", "First [scheduled:: 2028-03-01] [due:: 2028-03-01]")?;
    store.add_task("alpha", "Undated")?;
    store.add_task("beta", "Second [due:: 2028-03-01]")?;
    let mut app = App::new(store, "nvim".into());
    key(&mut app, KeyCode::Char('0'))?;
    key(&mut app, KeyCode::Char('c'))?;
    let local = screen(&mut app, 100, 30)?;
    assert!(local.contains("First"));
    assert!(!local.contains("Second"));
    assert!(!local.contains("Undated"));
    key(&mut app, KeyCode::Char('w'))?;
    let all = screen(&mut app, 100, 30)?;
    assert!(all.contains("Second"));
    key(&mut app, KeyCode::Tab)?;
    key(&mut app, KeyCode::Down)?;
    key(&mut app, KeyCode::Char(' '))?;
    assert!(app.store.files["beta"].tasks[0].done);
    assert!(!app.store.files["alpha"].tasks[0].done);
    key(&mut app, KeyCode::Char('d'))?;
    key(&mut app, KeyCode::Right)?;
    key(&mut app, KeyCode::Enter)?;
    assert_eq!(
        app.store.files["beta"].tasks[0].due,
        Some(parse_date("2028-03-02")?)
    );
    key(&mut app, KeyCode::Char('c'))?;
    assert!(screen(&mut app, 100, 30)?.contains("Undated"));
    Ok(())
}
