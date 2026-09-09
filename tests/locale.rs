use minitask::{store::Store, ui::App};
use ratatui::{Terminal, backend::TestBackend};
use std::{fs, process::Command};

#[test]
fn invalid_calendar_dates_have_japanese_errors() {
    for date in ["2026-13-01", "2026-02-30"] {
        assert_eq!(
            minitask::dates::parse_date(date).unwrap_err().to_string(),
            "存在する日付を入力してください"
        );
    }
}

#[test]
fn navigation_is_japanese_and_keeps_the_milestone_label_visible() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let mut store = Store::open(dir.path())?;
    store.create_workspace("仕事")?;
    let mut app = App::new(store, "nvim".into());
    let mut terminal = Terminal::new(TestBackend::new(80, 24))?;
    terminal.draw(|f| app.draw(f))?;
    let buffer = terminal.backend().buffer();
    let mut text = String::new();
    for y in 0..24 {
        let mut x = 0;
        while x < 80 {
            let s = buffer[(x, y)].symbol();
            text.push_str(s);
            x += unicode_width::UnicodeWidthStr::width(s).max(1) as u16;
        }
        text.push('\n');
    }
    for label in [
        "作業の続き",
        "マイルストーン",
        "ノート",
        "方針",
        "質問",
        "見通し・記録",
    ] {
        assert!(text.contains(label), "missing {label}: {text}");
    }
    for old in [
        "Resume",
        "Horizon",
        "Notes",
        "Directions",
        "Questions",
        "workspace",
    ] {
        assert!(!text.contains(old), "{text}");
    }
    Ok(())
}

#[test]
fn external_locale_overrides_text_without_changing_cli_json() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let locale = dir.path().join("locales");
    fs::create_dir(&locale)?;
    fs::write(
        locale.join("test.json"),
        r#"{"cli.help":"Custom help","view.resume":"Continue"}"#,
    )?;
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_minitask"))
            .arg("--locale")
            .arg("test")
            .arg("--locale-dir")
            .arg(&locale)
            .args(args)
            .output()
    };
    let help = run(&["--help"])?;
    assert!(
        help.status.success(),
        "{}",
        String::from_utf8_lossy(&help.stderr)
    );
    assert!(String::from_utf8_lossy(&help.stdout).contains("Custom help"));
    fs::create_dir(dir.path().join("work"))?;
    fs::write(
        dir.path().join("work/a.md"),
        "- [ ] Task [status:: in_progress]\n",
    )?;
    let output = run(&["--home", dir.path().to_str().unwrap(), "tasks"])?;
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["items"][0]["status"], "in_progress");
    assert_eq!(json["schema_version"], 2);
    Ok(())
}

#[test]
fn catalogs_validate_placeholders_and_keep_missing_keys_in_japanese() -> anyhow::Result<()> {
    use minitask::i18n::Catalog;
    let dir = tempfile::tempdir()?;
    fs::write(
        dir.path().join("custom.json"),
        r#"{"view.resume":"Continue","ui.text_051":"{1} entries / {0}"}"#,
    )?;
    let catalog = Catalog::load("custom", dir.path())?;
    assert_eq!(catalog.get("view.resume"), Some("Continue"));
    assert_eq!(catalog.get("view.questions"), Some("質問"));
    for invalid in [
        r#"{"ui.text_051":"{0}"}"#,
        r#"{"ui.text_051":"{name}"}"#,
        r#"{"view.resuem":"typo"}"#,
        r#"{"view.resume":"\u001b[31m"}"#,
    ] {
        fs::write(dir.path().join("custom.json"), invalid)?;
        assert!(Catalog::load("custom", dir.path()).is_err());
    }
    assert!(Catalog::load("../custom", dir.path()).is_err());
    assert!(Catalog::load("missing", dir.path()).is_err());
    // Inserted user text is not interpreted as another template.
    assert_eq!(minitask::tr!("ui.text_051", "{1}", 3), "{1} / 3件");
    Ok(())
}

#[test]
fn translated_status_selection_updates_canonical_state() -> anyhow::Result<()> {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    let dir = tempfile::tempdir()?;
    let mut store = Store::open(dir.path())?;
    store.create_workspace("仕事")?;
    store.add_task("仕事", "検証")?;
    let mut app = App::new(store, "nvim".into());
    app.event(Event::Key(KeyEvent::new(
        KeyCode::Char('v'),
        KeyModifiers::NONE,
    )))?;
    app.event(Event::Paste("進行中".into()))?;
    app.event(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )))?;
    assert_eq!(app.store.files["仕事"].tasks[0].status, "in_progress");
    let path = app
        .store
        .task_path("仕事", &app.store.files["仕事"].tasks[0])?;
    assert!(fs::read_to_string(path)?.contains("[status:: in_progress]"));
    Ok(())
}
