use minitask::store::lock_store;
use serde_json::Value;
use std::{fs, process::Command};

fn query(root: &std::path::Path, args: &[&str]) -> anyhow::Result<Value> {
    let output = Command::new(env!("CARGO_BIN_EXE_minitask"))
        .arg("--home")
        .arg(root)
        .args(args)
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(serde_json::from_slice(&output.stdout)?)
}

#[test]
fn lists_omit_bodies_but_keep_body_search_and_explicit_details() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    fs::create_dir(dir.path().join("work"))?;
    let body = "本文だけの検索語\n".repeat(1000);
    for kind in ["action", "note", "direction", "question", "expectation"] {
        let mut args = vec![
            "add",
            "--workspace",
            "work",
            "--kind",
            kind,
            "--title",
            kind,
            "--body",
            &body,
        ];
        if kind == "action" {
            args.extend(["--completion-condition", "検証条件"]);
        }
        if kind == "expectation" {
            args.extend(["--date", "2028-09-30"]);
        }
        query(dir.path(), &args)?;
    }
    for command in ["tasks", "notes", "directions", "questions", "expectations"] {
        let summary = query(dir.path(), &[command, "--query", "本文だけの検索語"])?;
        let item = &summary["items"][0];
        for field in ["notes", "completion_condition", "supplement"] {
            assert!(item.get(field).is_none(), "{command} leaked {field}");
        }
        assert_eq!(summary["schema_version"], 2);
        let full = query(dir.path(), &[command, "--full"])?;
        let detail = query(dir.path(), &["show", "--id", item["id"].as_str().unwrap()])?;
        assert_eq!(detail["item"], full["items"][0]);
        assert!(
            detail["item"]["notes"]
                .as_str()
                .unwrap()
                .contains("本文だけの検索語")
        );
        assert!(serde_json::to_vec(&summary)?.len() * 10 < serde_json::to_vec(&full)?.len());
    }
    let summary = query(dir.path(), &["resume"])?;
    let full = query(dir.path(), &["resume", "--full"])?;
    for key in [
        "actions",
        "notes",
        "directions",
        "questions",
        "expectations",
    ] {
        assert!(summary[key][0].get("notes").is_none());
        assert!(
            full[key][0]["notes"]
                .as_str()
                .unwrap()
                .contains("本文だけの検索語")
        );
    }
    Ok(())
}

#[test]
fn queries_are_read_only_and_keep_tasks_separate_from_expectations() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    fs::create_dir(dir.path().join("work"))?;
    fs::write(
        dir.path().join("work/task.md"),
        "- [ ] 作業 [due:: 2028-09-20]\n\n詳細\n",
    )?;
    fs::write(
        dir.path().join("work/milestone.md"),
        "- [ ] 試作を見せられる [expectation:: 2028-09-30]\n\n気軽な目標\n",
    )?;
    let _lock = lock_store(dir.path())?;
    let tasks = query(dir.path(), &["tasks", "--workspace", "work", "--json"])?;
    assert_eq!(tasks["schema_version"], 2);
    assert_eq!(tasks["items"].as_array().unwrap().len(), 1);
    let milestones = query(
        dir.path(),
        &["expectations", "--from", "2028-09-01", "--to", "2028-09-30"],
    )?;
    assert_eq!(milestones["items"][0]["title"], "試作を見せられる");
    assert_eq!(milestones["items"][0]["date"], "2028-09-30");
    let id = milestones["items"][0]["id"].as_str().unwrap();
    let detail = query(dir.path(), &["show", "--id", id])?;
    assert!(
        detail["item"]["notes"]
            .as_str()
            .unwrap()
            .contains("気軽な目標")
    );
    assert!(!dir.path().join(".index.json").exists());
    for args in [
        vec!["tasks", "--workspace", "missing"],
        vec!["show", "--id", "../escape"],
        vec!["tasks", "--status", "nonsense"],
        vec!["tasks", "--full", "--full"],
        vec!["workspaces", "--full"],
        vec!["show", "--id", id, "--full"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_minitask"))
            .arg("--home")
            .arg(dir.path())
            .args(args)
            .output()?;
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
    Ok(())
}

#[test]
fn free_notes_keep_literal_metadata_and_checklists() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    fs::create_dir(dir.path().join("work"))?;
    let title = "# Discuss [due:: someday]";
    let body =
        "- [ ] just a checklist\n[status:: anything]\n## arbitrary heading\nNo structure required.";
    let result = query(
        dir.path(),
        &[
            "add",
            "--workspace",
            "work",
            "--kind",
            "note",
            "--title",
            title,
            "--body",
            body,
        ],
    )?;
    assert_eq!(result["item"]["title"], title);
    assert!(result["item"]["notes"].as_str().unwrap().contains(body));
    let notes = query(dir.path(), &["notes"])?;
    assert_eq!(notes["items"].as_array().unwrap().len(), 1);
    assert!(
        query(dir.path(), &["tasks"])?["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    Ok(())
}

#[test]
fn option_like_values_remain_data_and_completion_headings_ignore_fences() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    fs::create_dir(dir.path().join("work"))?;
    let result = query(
        dir.path(),
        &[
            "add",
            "--workspace",
            "work",
            "--kind",
            "note",
            "--title",
            "Flags",
            "--body",
            "--help",
        ],
    )?;
    assert!(result["item"]["notes"].as_str().unwrap().contains("--help"));
    assert_eq!(
        query(dir.path(), &["notes", "--query", "--help"])?["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    fs::write(
        dir.path().join("work/action.md"),
        "- [ ] Actual\n\n```text\n~~~\n```\n\n## Completion Condition\nThe actual condition\n\n## Supplement\nbody\n",
    )?;
    let action = query(dir.path(), &["tasks", "--full"])?;
    assert_eq!(
        action["items"][0]["completion_condition"],
        "The actual condition"
    );
    Ok(())
}
