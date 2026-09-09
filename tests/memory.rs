use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use minitask::{store::Store, ui::App};
use ratatui::{Terminal, backend::TestBackend};
use std::{fs, process::Command};

fn key(app: &mut App, code: KeyCode) -> anyhow::Result<()> {
    app.event(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)))?;
    Ok(())
}
fn screen(app: &mut App) -> anyhow::Result<String> {
    let mut terminal = Terminal::new(TestBackend::new(120, 35))?;
    terminal.draw(|f| app.draw(f))?;
    Ok(terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect())
}
#[test]
fn resume_calendar_and_milestone_creation_work_from_the_keyboard() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    fs::create_dir(dir.path().join("work"))?;
    for (name, text) in [
        (
            "action.md",
            "- [ ] Working [status:: in_progress]\n\nNext step: inspect save file\n",
        ),
        (
            "direction.md",
            "- [ ] Preserve saves [kind:: direction] [status:: active]\n",
        ),
        (
            "question.md",
            "- [ ] Which platform? [kind:: question] [status:: open]\n",
        ),
        (
            "milestone.md",
            "- [ ] Demo ready [expectation:: 2028-09-30]\n",
        ),
        (
            "note-scratch.md",
            "Unstructured scratch\n- [ ] this is only a note checklist\n",
        ),
    ] {
        fs::write(dir.path().join("work").join(name), text)?;
    }
    let mut app = App::new(Store::open(dir.path())?, "nvim".into());
    key(&mut app, KeyCode::Char('6'))?;
    let resume = screen(&mut app)?;
    for title in [
        "Working",
        "Preserve saves",
        "Which platform?",
        "Demo ready",
        "Unstructured scratch",
    ] {
        assert!(resume.contains(title), "{resume}");
    }
    key(&mut app, KeyCode::Char('7'))?;
    key(&mut app, KeyCode::Char('c'))?;
    let calendar = screen(&mut app)?;
    assert!(calendar.contains("2028-09"));
    assert!(calendar.contains("Demo ready"));
    key(&mut app, KeyCode::Char('c'))?;
    key(&mut app, KeyCode::Char('a'))?;
    app.event(Event::Paste("Second milestone".into()))?;
    key(&mut app, KeyCode::Tab)?;
    key(&mut app, KeyCode::Tab)?;
    app.event(Event::Paste("2028-10-05".into()))?;
    key(&mut app, KeyCode::Enter)?;
    let task = app.store.files["work"]
        .tasks
        .iter()
        .find(|t| t.title == "Second milestone")
        .unwrap();
    assert_eq!(
        task.expectation,
        Some(minitask::dates::parse_date("2028-10-05")?)
    );
    assert!(screen(&mut app)?.contains("Second milestone"));
    Ok(())
}

#[test]
fn cli_rejects_invalid_transitions_and_records_verification() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    fs::create_dir(dir.path().join("work"))?;
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_minitask"))
            .arg("--home")
            .arg(dir.path())
            .args(args)
            .output()
    };
    let result = run(&[
        "add",
        "--workspace",
        "work",
        "--kind",
        "action",
        "--title",
        "Save feature",
        "--completion-condition",
        "Load restores position",
    ])?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let item: serde_json::Value = serde_json::from_slice(&result.stdout)?;
    let id = item["item"]["id"].as_str().unwrap();
    let hash = item["item"]["hash"].as_str().unwrap();
    assert!(
        !run(&[
            "set-status",
            "--id",
            id,
            "--status",
            "completed",
            "--expected-hash",
            hash
        ])?
        .status
        .success()
    );
    let completed = run(&[
        "set-status",
        "--id",
        id,
        "--status",
        "completed",
        "--expected-hash",
        hash,
        "--evidence",
        "Load test passed",
    ])?;
    assert!(
        completed.status.success(),
        "{}",
        String::from_utf8_lossy(&completed.stderr)
    );
    let item: serde_json::Value = serde_json::from_slice(&completed.stdout)?;
    assert_eq!(item["item"]["status"], "completed");
    assert!(
        item["item"]["notes"]
            .as_str()
            .unwrap()
            .contains("Load test passed")
    );
    let new_hash = item["item"]["hash"].as_str().unwrap();
    assert!(
        !run(&[
            "set-status",
            "--id",
            id,
            "--status",
            "in_progress",
            "--expected-hash",
            new_hash
        ])?
        .status
        .success()
    );
    assert!(
        !run(&[
            "set-status",
            "--id",
            id,
            "--status",
            "actionable",
            "--expected-hash",
            hash
        ])?
        .status
        .success()
    );
    Ok(())
}
