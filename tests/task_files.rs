use minitask::store::Store;
use std::fs;
use tempfile::tempdir;

#[test]
fn creates_one_markdown_per_task_and_reads_freeform_details() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let mut store = Store::open(dir.path())?;
    store.create_workspace("仕事")?;
    store.add_task("仕事", "リリース準備 [due:: 2026-09-12]")?;
    store.add_task("仕事", "リリース準備")?;
    let paths: Vec<_> = fs::read_dir(dir.path().join("仕事"))?
        .map(|entry| entry.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .collect();
    assert_eq!(
        paths.len(),
        2,
        "one Markdown file per task, even with identical titles"
    );
    assert!(!dir.path().join("仕事/tasks.md").exists());
    let doc = "- [ ] 詳細のあるタスク [due:: 2026-09-12]\n\n## 背景\n通常の段落。[[関連ノート]]\n\n## 手順\n- [ ] 子チェック項目\n\n```rust\nfn main() {}\n```\n";
    fs::write(&paths[0], doc)?;
    assert_eq!(store.refresh(false, None)?, 1);
    let task = store.files["仕事"]
        .tasks
        .iter()
        .find(|t| t.title == "詳細のあるタスク")
        .unwrap();
    assert!(task.notes.contains("## 背景"));
    assert!(task.notes.contains("- [ ] 子チェック項目"));
    assert_eq!(store.files["仕事"].tasks.len(), 2);
    assert_eq!(Store::open(dir.path())?.parsed, 0);
    Ok(())
}

#[test]
fn external_task_files_and_empty_workspaces_are_discovered() -> anyhow::Result<()> {
    let dir = tempdir()?;
    fs::create_dir(dir.path().join("empty"))?;
    fs::create_dir(dir.path().join("work"))?;
    fs::write(
        dir.path().join("work/custom.md"),
        "# メモ\n\n- [ ] 手書きのタスク\n\n詳細の段落\n",
    )?;
    let store = Store::open(dir.path())?;
    assert!(store.files["empty"].tasks.is_empty());
    assert_eq!(store.files["work"].tasks[0].title, "手書きのタスク");
    assert_eq!(store.files["work"].tasks[0].notes, "詳細の段落");
    Ok(())
}

#[test]
fn migration_preserves_backup_bodies_dates_and_is_idempotent() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let mut store = Store::open(dir.path())?;
    store.create_workspace("work")?;
    let original = "\u{feff}# work\r\n\r\n## [ ] Legacy [due:: 2028-03-01]\r\n\r\n## 背景\r\n自由な段落\r\n- [ ] checklist\r\n\r\n## Tasks\r\n- [x] Other\r\n  note\r\n\r\n## 詳細\r\nmore text\r\n";
    fs::write(store.path("work"), original)?;
    store.refresh(true, None)?;
    assert_eq!(store.split_legacy()?, 2);
    assert!(!store.path("work").exists());
    let backup = fs::read_dir(dir.path().join("work"))?
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|s| s == "bak"))
        .unwrap();
    assert_eq!(fs::read(backup)?, original.as_bytes());
    let tasks = &store.files["work"].tasks;
    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].title, "Legacy");
    assert_eq!(
        tasks[0].due,
        Some(minitask::dates::parse_date("2028-03-01")?)
    );
    assert_eq!(
        tasks[0].notes,
        "## 背景\r\n自由な段落\r\n- [ ] checklist\r\n\r\n## Tasks"
    );
    assert!(tasks[1].done);
    assert!(tasks[1].notes.contains("more text"));
    assert_eq!(store.split_legacy()?, 0);
    assert_eq!(Store::open(dir.path())?.files["work"].tasks.len(), 2);
    Ok(())
}

#[test]
fn interrupted_migration_resumes_and_refuses_changed_destinations() -> anyhow::Result<()> {
    use sha2::{Digest, Sha256};
    let dir = tempdir()?;
    let workspace = dir.path().join("work");
    let stage = workspace.join(".minitask-split");
    fs::create_dir_all(&stage)?;
    let original = b"## [ ] First\nnotes\n## [x] Second\n";
    let hash = format!("{:x}", Sha256::digest(original));
    let names = [
        format!("legacy-{}-00000-First.md", &hash[..16]),
        format!("legacy-{}-00001-Second.md", &hash[..16]),
    ];
    fs::write(stage.join("original"), original)?;
    fs::write(stage.join("manifest.json"), serde_json::to_vec(&names)?)?;
    fs::write(stage.join(&names[0]), "- [ ] First\nnotes\n")?;
    fs::write(stage.join(&names[1]), "- [x] Second\n")?;
    fs::write(workspace.join(format!(".tasks-{hash}.bak")), original)?;
    // Crash after moving the original aside and publishing the first task.
    fs::write(workspace.join(&names[0]), "external edit\n")?;
    assert!(Store::open(dir.path()).is_err());
    assert_eq!(
        fs::read_to_string(workspace.join(&names[0]))?,
        "external edit\n"
    );
    fs::write(workspace.join(&names[0]), "- [ ] First\nnotes\n")?;
    let store = Store::open(dir.path())?;
    assert_eq!(store.files["work"].tasks.len(), 2);
    assert!(!stage.exists());
    assert_eq!(Store::open(dir.path())?.parsed, 0);
    Ok(())
}

#[test]
fn edits_and_undo_target_only_the_selected_source() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let mut store = Store::open(dir.path())?;
    store.create_workspace("work")?;
    store.add_task("work", "First")?;
    store.add_task("work", "Second")?;
    let first = store.files["work"].tasks[0].clone();
    let second = store.files["work"].tasks[1].clone();
    let path = store.task_path("work", &first)?;
    let original = fs::read(&path)?;
    store.toggle("work", &first)?;
    assert_eq!(store.parsed, 1);
    let sibling = store.task_path("work", &second)?;
    fs::write(&sibling, "- [ ] Edited sibling\n\nfreeform notes\n")?;
    store.undo_last()?;
    assert_eq!(fs::read(&path)?, original);
    assert_eq!(
        fs::read_to_string(&sibling)?,
        "- [ ] Edited sibling\n\nfreeform notes\n"
    );
    fs::write(&path, "- [ ] external replacement\n")?;
    assert!(store.toggle("work", &first).is_err());
    Ok(())
}
