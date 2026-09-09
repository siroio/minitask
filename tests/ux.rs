use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use minitask::{dates, store::Store, ui::App};
use ratatui::{Terminal, backend::TestBackend};
use std::fs;
use tempfile::tempdir;

fn key(app: &mut App, code: KeyCode) -> anyhow::Result<()> {
    app.event(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)))?;
    Ok(())
}
fn text(app: &mut App, text: &str) -> anyhow::Result<()> {
    app.event(Event::Paste(text.into()))?;
    Ok(())
}
fn screen(app: &mut App, width: u16, height: u16) -> anyhow::Result<String> {
    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    terminal.draw(|f| app.draw(f))?;
    let b = terminal.backend().buffer();
    let mut s = String::new();
    for y in 0..height {
        let mut x = 0;
        while x < width {
            let c = b[(x, y)].symbol();
            s.push_str(c);
            x += unicode_width::UnicodeWidthStr::width(c).max(1) as u16;
        }
        s.push('\n');
    }
    Ok(s)
}

#[test]
fn tab_reaches_workspaces_and_shift_tab_returns_through_all_panels() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let mut store = Store::open(dir.path())?;
    for workspace in ["alpha", "beta"] {
        store.create_workspace(workspace)?;
        store.add_task(
            workspace,
            &format!("{workspace} task [scheduled:: {}]", dates::today()),
        )?;
    }
    let mut app = App::new(store, "nvim".into());
    key(&mut app, KeyCode::Char('1'))?;
    key(&mut app, KeyCode::Tab)?; // Tasks -> views.
    key(&mut app, KeyCode::Tab)?; // Views -> workspaces.
    assert!(screen(&mut app, 40, 12)?.contains("alpha"));
    key(&mut app, KeyCode::Char('R'))?; // Refresh must retain the focused panel.
    key(&mut app, KeyCode::Down)?;
    assert_eq!(app.workspace(), "beta");
    let shown = screen(&mut app, 100, 30)?;
    assert!(shown.contains("beta task") && !shown.contains("alpha task"));
    key(&mut app, KeyCode::Tab)?; // Workspaces -> tasks.
    key(&mut app, KeyCode::Char(' '))?;
    assert!(app.store.files["beta"].tasks[0].done);
    key(&mut app, KeyCode::Char('u'))?;
    key(&mut app, KeyCode::BackTab)?; // Tasks -> workspaces.
    key(&mut app, KeyCode::Up)?;
    assert_eq!(app.workspace(), "alpha");
    key(&mut app, KeyCode::BackTab)?; // Workspaces -> views (last selected: today).
    key(&mut app, KeyCode::Enter)?; // Activate today and enter tasks.
    let shown = screen(&mut app, 100, 30)?;
    assert!(shown.contains("alpha task") && shown.contains("beta task"));
    Ok(())
}

#[test]
fn smart_views_live_search_and_compact_rows() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let mut store = Store::open(dir.path())?;
    store.create_workspace("work")?;
    store.add_task(
        "work",
        &format!("Today task [scheduled:: {}]", dates::today()),
    )?;
    for i in 0..12 {
        store.add_task("work", &format!("Item {i:02}"))?;
    }
    let mut app = App::new(store, "nvim".into());
    key(&mut app, KeyCode::Char('1'))?;
    let today = screen(&mut app, 80, 24)?;
    assert!(today.contains("今日"));
    assert!(today.contains("Today task"));
    assert!(!today.contains("Item 00"));
    key(&mut app, KeyCode::Char('4'))?;
    let all = screen(&mut app, 80, 24)?;
    for i in 0..12 {
        assert!(all.contains(&format!("Item {i:02}")), "{all}");
    }
    key(&mut app, KeyCode::Char('/'))?;
    text(&mut app, "Item 11")?;
    let search = screen(&mut app, 80, 24)?;
    assert!(search.contains("Item 11"));
    assert!(!search.contains("Item 00"));
    key(&mut app, KeyCode::Esc)?;
    assert!(screen(&mut app, 80, 24)?.contains("Item 00"));
    key(&mut app, KeyCode::Char(' '))?;
    assert!(app.store.files["work"].tasks.iter().any(|t| t.done));
    key(&mut app, KeyCode::Char('u'))?;
    assert!(app.store.files["work"].tasks.iter().all(|t| !t.done));
    key(&mut app, KeyCode::Char('3'))?;
    app.save_state()?;
    let mut restored = App::new(Store::open(dir.path())?, "nvim".into());
    assert!(screen(&mut restored, 80, 24)?.contains("日付未設定 / すべてのワークスペース"));
    Ok(())
}

#[test]
fn new_action_starts_with_today_and_dates_can_be_cleared() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let mut store = Store::open(dir.path())?;
    store.create_workspace("work")?;
    let mut app = App::new(store, "nvim".into());
    key(&mut app, KeyCode::Char('a'))?;
    text(&mut app, "Default dates")?;
    assert!(screen(&mut app, 100, 30)?.contains(&dates::today().to_string()));
    key(&mut app, KeyCode::Enter)?;
    assert_eq!(
        app.store.files["work"].tasks[0].scheduled,
        Some(dates::today())
    );
    assert_eq!(app.store.files["work"].tasks[0].due, Some(dates::today()));

    key(&mut app, KeyCode::Char('a'))?;
    text(&mut app, "No dates")?;
    key(&mut app, KeyCode::Tab)?;
    for _ in 0..2 {
        key(&mut app, KeyCode::Tab)?;
        key(&mut app, KeyCode::Home)?;
        for _ in 0..10 {
            key(&mut app, KeyCode::Delete)?;
        }
    }
    key(&mut app, KeyCode::Enter)?;
    let task = app.store.files["work"]
        .tasks
        .iter()
        .find(|t| t.title == "No dates")
        .unwrap();
    assert!(task.scheduled.is_none() && task.due.is_none());
    Ok(())
}
fn replace_date_input(app: &mut App, date: &str) -> anyhow::Result<()> {
    key(app, KeyCode::Home)?;
    for _ in 0..10 {
        key(app, KeyCode::Delete)?;
    }
    text(app, date)
}

#[test]
fn add_form_dates_help_palette_and_undo() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let mut store = Store::open(dir.path())?;
    store.create_workspace("alpha")?;
    store.create_workspace("beta")?;
    let mut app = App::new(store, "nvim".into());
    key(&mut app, KeyCode::Char('a'))?;
    text(&mut app, "Added task")?;
    key(&mut app, KeyCode::Tab)?;
    key(&mut app, KeyCode::Down)?;
    key(&mut app, KeyCode::Tab)?;
    replace_date_input(&mut app, "2028-02-29")?;
    key(&mut app, KeyCode::Tab)?;
    replace_date_input(&mut app, "2028-03-02")?;
    key(&mut app, KeyCode::Enter)?;
    assert!(app.store.files["alpha"].tasks.is_empty());
    assert_eq!(
        app.store.files["beta"].tasks[0].due,
        Some(dates::parse_date("2028-03-02")?)
    );
    let path = app
        .store
        .task_path("beta", &app.store.files["beta"].tasks[0])?;
    let original = fs::read(&path)?;
    key(&mut app, KeyCode::Char('d'))?;
    key(&mut app, KeyCode::Char('i'))?;
    key(&mut app, KeyCode::Home)?;
    for _ in 0..10 {
        key(&mut app, KeyCode::Delete)?;
    }
    text(&mut app, "2028-03-10")?;
    key(&mut app, KeyCode::Enter)?;
    assert_eq!(
        app.store.files["beta"].tasks[0].due,
        Some(dates::parse_date("2028-03-10")?)
    );
    key(&mut app, KeyCode::Char('u'))?;
    assert_eq!(fs::read(&path)?, original);
    key(&mut app, KeyCode::Char('d'))?;
    key(&mut app, KeyCode::Right)?;
    key(&mut app, KeyCode::Enter)?;
    fs::write(&path, "- [ ] external\n")?;
    assert!(key(&mut app, KeyCode::Char('u')).is_err());
    assert_eq!(fs::read_to_string(&path)?, "- [ ] external\n");
    key(&mut app, KeyCode::Char('?'))?;
    assert!(screen(&mut app, 80, 24)?.contains("ヘルプ"));
    key(&mut app, KeyCode::Esc)?;
    key(&mut app, KeyCode::Char(':'))?;
    text(&mut app, "日付未設定")?;
    key(&mut app, KeyCode::Enter)?;
    assert!(screen(&mut app, 80, 24)?.contains("日付未設定 / すべてのワークスペース"));
    Ok(())
}

#[test]
fn form_validation_calendar_add_presets_and_small_overlays() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let mut store = Store::open(dir.path())?;
    store.create_workspace("work")?;
    let mut app = App::new(store, "nvim".into());
    key(&mut app, KeyCode::Char('a'))?;
    text(&mut app, "日本語の下書き")?;
    key(&mut app, KeyCode::Tab)?;
    key(&mut app, KeyCode::Tab)?;
    replace_date_input(&mut app, "2028-02-30")?;
    assert!(key(&mut app, KeyCode::Enter).is_err());
    assert_eq!(fs::read_dir(dir.path().join("work"))?.count(), 0);
    assert!(screen(&mut app, 80, 24)?.contains("日本語の下書き"));
    key(&mut app, KeyCode::Home)?;
    for _ in 0..10 {
        key(&mut app, KeyCode::Delete)?;
    }
    text(&mut app, "2028-02-29")?;
    key(&mut app, KeyCode::Enter)?;
    assert_eq!(app.store.files["work"].tasks.len(), 1);
    key(&mut app, KeyCode::Char('s'))?;
    key(&mut app, KeyCode::Char('?'))?;
    assert!(screen(&mut app, 80, 24)?.contains("日付設定:"));
    key(&mut app, KeyCode::Esc)?;
    key(&mut app, KeyCode::Char('2'))?;
    key(&mut app, KeyCode::Enter)?;
    assert_eq!(
        app.store.files["work"].tasks[0].scheduled,
        Some(dates::today() + time::Duration::days(1))
    );
    key(&mut app, KeyCode::Enter)?;
    assert!(screen(&mut app, 60, 20)?.contains("詳細 /"));
    key(&mut app, KeyCode::Char('c'))?;
    assert!(!screen(&mut app, 60, 20)?.contains("詳細 /"));
    key(&mut app, KeyCode::Char('a'))?;
    text(&mut app, "Calendar added")?;
    key(&mut app, KeyCode::Enter)?;
    assert_eq!(
        app.store.files["work"].tasks[1].scheduled,
        Some(dates::today() + time::Duration::days(1))
    );
    assert!(screen(&mut app, 80, 24)?.contains("Calendar added"));
    assert_eq!(
        app.store.files["work"].tasks[1].due,
        app.store.files["work"].tasks[1].scheduled
    );
    for (w, h) in [(40, 12), (60, 20), (80, 24), (150, 40)] {
        screen(&mut app, w, h)?;
        key(&mut app, KeyCode::Enter)?;
        screen(&mut app, w, h)?;
        key(&mut app, KeyCode::Enter)?;
    }
    Ok(())
}

#[test]
fn views_use_either_date_and_hide_completed_tasks() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let mut store = Store::open(dir.path())?;
    store.create_workspace("work")?;
    let day = dates::today();
    fs::write(
        store.path("work"),
        format!(
            "- [ ] Late [due:: {}]\n- [ ] Carry [scheduled:: {}]\n- [ ] Today [due:: {day}]\n- [ ] Next [scheduled:: {}]\n- [ ] Week [due:: {}]\n- [ ] Later [due:: {}]\n- [ ] Undated\n- [x] Finished [due:: {day}]\n",
            day - time::Duration::days(1),
            day - time::Duration::days(2),
            day + time::Duration::days(1),
            day + time::Duration::days(7),
            day + time::Duration::days(8)
        ),
    )?;
    store.refresh(true, None)?;
    let mut app = App::new(store, "nvim".into());
    key(&mut app, KeyCode::Char('1'))?;
    let today = screen(&mut app, 100, 30)?;
    for s in ["Late", "Carry", "Today", "期限超過", "持ち越し"] {
        assert!(today.contains(s), "{today}");
    }
    for s in ["Next", "Week", "Later", "Undated", "Finished"] {
        assert!(!today.contains(s), "{today}");
    }
    key(&mut app, KeyCode::Char('2'))?;
    let upcoming = screen(&mut app, 100, 30)?;
    assert!(upcoming.contains("Next") && upcoming.contains("Week"));
    assert!(!upcoming.contains("Later") && !upcoming.contains("Finished"));
    key(&mut app, KeyCode::Char('3'))?;
    let unscheduled = screen(&mut app, 100, 30)?;
    assert!(unscheduled.contains("Undated") && !unscheduled.contains("Next"));
    key(&mut app, KeyCode::Char('5'))?;
    let done = screen(&mut app, 100, 30)?;
    assert!(done.contains("Finished") && !done.contains("Undated"));
    Ok(())
}

#[test]
#[ignore = "manual 10000-task interaction benchmark"]
fn interaction_benchmark() -> anyhow::Result<()> {
    use std::time::Instant;
    let dir = tempdir()?;
    let today = dates::today();
    for w in 0..100 {
        let path = dir.path().join(format!("project-{w:03}"));
        fs::create_dir(&path)?;
        let source=(0..100).map(|i|format!("- [ ] Task {w:03}-{i:03} 日本語の作業 [scheduled:: {}] [due:: {}]\n  メモと詳細の記述\n",today+time::Duration::days(i % 28),today+time::Duration::days((i+3) % 28))).collect::<String>();
        fs::write(path.join("tasks.md"), source)?;
    }
    let mut app = App::new(Store::open(dir.path())?, "nvim".into());
    key(&mut app, KeyCode::Char('4'))?;
    let mut terminal = Terminal::new(TestBackend::new(120, 30))?;
    let mut render = Vec::new();
    let mut search = Vec::new();
    let mut calendar = Vec::new();
    for _ in 0..11 {
        let start = Instant::now();
        terminal.draw(|f| app.draw(f))?;
        render.push(start.elapsed());
        key(&mut app, KeyCode::Char('/'))?;
        let start = Instant::now();
        text(&mut app, "Task 099-099")?;
        terminal.draw(|f| app.draw(f))?;
        search.push(start.elapsed());
        key(&mut app, KeyCode::Esc)?;
    }
    key(&mut app, KeyCode::Char('c'))?;
    for _ in 0..11 {
        let start = Instant::now();
        key(&mut app, KeyCode::Right)?;
        terminal.draw(|f| app.draw(f))?;
        calendar.push(start.elapsed());
    }
    render.sort();
    search.sort();
    calendar.sort();
    println!(
        "10000 tasks, 120x30, median of 11: render={:?} search+render={:?} calendar move+render={:?}",
        render[5], search[5], calendar[5]
    );
    Ok(())
}

#[test]
fn scrolling_search_cancel_and_calendar_return_keep_selection() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let mut store = Store::open(dir.path())?;
    store.create_workspace("work")?;
    fs::write(
        store.path("work"),
        (0..70)
            .map(|i| format!("- [ ] Item {i:02} [due:: {}]\n", dates::today()))
            .collect::<String>(),
    )?;
    store.refresh(true, None)?;
    let mut app = App::new(store, "nvim".into());
    key(&mut app, KeyCode::Char('4'))?;
    for _ in 0..50 {
        key(&mut app, KeyCode::Down)?;
    }
    assert!(screen(&mut app, 80, 24)?.contains("Item 50"));
    key(&mut app, KeyCode::Char('/'))?;
    text(&mut app, "Item 01")?;
    assert!(!screen(&mut app, 80, 24)?.contains("Item 50"));
    key(&mut app, KeyCode::Esc)?;
    key(&mut app, KeyCode::Char('c'))?;
    key(&mut app, KeyCode::Char('c'))?;
    key(&mut app, KeyCode::Char(' '))?;
    assert!(app.store.files["work"].tasks[50].done);
    assert!(!app.store.files["work"].tasks[1].done);
    key(&mut app, KeyCode::Char('?'))?;
    let first = screen(&mut app, 40, 12)?;
    for _ in 0..12 {
        key(&mut app, KeyCode::PageDown)?;
        screen(&mut app, 40, 12)?;
    }
    let last = screen(&mut app, 40, 12)?;
    assert_ne!(first, last);
    assert!(last.contains("外部変更"), "{last}");
    Ok(())
}

#[test]
fn add_form_shows_the_focused_deadline_on_a_small_terminal() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let mut store = Store::open(dir.path())?;
    store.create_workspace("work")?;
    let mut app = App::new(store, "nvim".into());
    key(&mut app, KeyCode::Char('a'))?;
    text(&mut app, "日本語タスク")?;
    for _ in 0..3 {
        key(&mut app, KeyCode::Tab)?;
    }
    text(&mut app, "2028-03-02")?;
    let rendered = screen(&mut app, 40, 12)?;
    assert!(rendered.contains("> 期限"), "{rendered}");
    assert!(rendered.contains("2028-03-02"), "{rendered}");
    Ok(())
}
