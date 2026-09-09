use crate::tr;
use crate::{
    dates::{self, DateField},
    memory::{self, Kind},
    store::{Store, Task, lock_store},
};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

pub const COMMANDS: &[&str] = &[
    "resume",
    "workspaces",
    "tasks",
    "expectations",
    "notes",
    "directions",
    "questions",
    "show",
    "add",
    "set-status",
    "set-date",
];
pub fn help() -> &'static str {
    tr!("cli.help")
}

pub fn run(root: &Path, command: &str, args: &[String]) -> Result<()> {
    let mut opts = BTreeMap::new();
    let mut i = 0;
    while i < args.len() {
        let key = args[i].as_str();
        ensure!(key.starts_with("--"), "{}", tr!("cli.text_016", key));
        if key == "--json" {
            i += 1;
            continue;
        }
        if key == "--full" {
            ensure!(
                opts.insert(key, "").is_none(),
                "{}",
                tr!("cli.text_014", key)
            );
            i += 1;
            continue;
        }
        let value = args.get(i + 1).with_context(|| tr!("cli.text_015", key))?;
        ensure!(
            opts.insert(key, value.as_str()).is_none(),
            "{}",
            tr!("cli.text_014", key)
        );
        i += 2;
    }
    let allowed: &[&str] = match command {
        "show" => &["--id"],
        "workspaces" => &[],
        "add" => &[
            "--workspace",
            "--kind",
            "--title",
            "--date",
            "--body",
            "--completion-condition",
        ],
        "set-status" => &["--id", "--status", "--expected-hash", "--evidence"],
        "set-date" => &["--id", "--field", "--date", "--expected-hash"],
        "resume" => &["--workspace", "--full"],
        _ => &[
            "--workspace",
            "--query",
            "--status",
            "--from",
            "--to",
            "--full",
        ],
    };
    for key in opts.keys() {
        ensure!(
            allowed.contains(key),
            "{}",
            tr!("cli.text_013", key, command)
        );
    }
    let required = |key: &str| -> Result<&str> {
        opts.get(key)
            .copied()
            .with_context(|| tr!("cli.text_012", key))
    };
    let write = matches!(command, "add" | "set-status" | "set-date");
    let _lock = if write { Some(lock_store(root)?) } else { None };
    let mut store = if write {
        Store::open(root)?
    } else {
        Store::snapshot(root)?
    };
    if let Some(workspace) = opts.get("--workspace") {
        ensure!(
            store.files.contains_key(*workspace),
            "{}",
            tr!("cli.text_011", workspace)
        );
    }
    let result = if command == "add" {
        let workspace = required("--workspace")?;
        let kind = Kind::parse(required("--kind")?)?;
        let date = opts
            .get("--date")
            .map(|s| dates::parse_date(s))
            .transpose()?;
        ensure!(
            kind == Kind::Action || !opts.contains_key("--completion-condition"),
            "{}",
            tr!("cli.text_010")
        );
        let source = store.add_entity(
            workspace,
            kind,
            required("--title")?,
            date,
            opts.get("--body").copied().unwrap_or(""),
            opts.get("--completion-condition").copied().unwrap_or(""),
        )?;
        let task = store.files[workspace]
            .tasks
            .iter()
            .find(|t| t.source == source)
            .context(tr!("cli.text_019"))?;
        json!({"item": entity(&store, workspace, task, true)})
    } else if matches!(command, "show" | "set-status" | "set-date") {
        let (workspace, task) = find(&store, required("--id")?)?;
        let workspace = workspace.to_owned();
        let task = task.clone();
        if write {
            ensure!(
                required("--expected-hash")? == task.hash,
                "{}",
                tr!("cli.text_009")
            );
            if command == "set-status" {
                let status = required("--status")?;
                let evidence = opts.get("--evidence").copied().unwrap_or("");
                if task.kind == Kind::Action && status == "completed" {
                    ensure!(
                        !memory::section(&task.notes, "Completion Condition").is_empty(),
                        "{}",
                        tr!("cli.text_008")
                    );
                    ensure!(!evidence.trim().is_empty(), "{}", tr!("cli.text_007"));
                }
                if task.kind == Kind::Question && status == "answered" {
                    ensure!(!evidence.trim().is_empty(), "{}", tr!("cli.text_006"));
                }
                store.transition(&workspace, &task, status, evidence)?;
            } else {
                let field = match required("--field")? {
                    "scheduled" => DateField::Scheduled,
                    "due" => DateField::Due,
                    "expectation" => DateField::Expectation,
                    other => bail!("{}", tr!("cli.text_005", other)),
                };
                let raw = required("--date")?;
                let date = if raw == "none" {
                    None
                } else {
                    Some(dates::parse_date(raw)?)
                };
                store.set_date(&workspace, &task, field, date)?;
            }
        }
        let current = store.files[&workspace]
            .tasks
            .iter()
            .find(|t| t.source == task.source && t.line == task.line)
            .context(tr!("cli.text_018"))?;
        json!({"item": entity(&store, &workspace, current, true)})
    } else if command == "workspaces" {
        json!({"items": store.files.iter().map(|(n,w)|json!({"name":n,"path":store.root.join(n),"entities":w.tasks.len()})).collect::<Vec<_>>()})
    } else if command == "resume" {
        resume(
            &store,
            opts.get("--workspace").copied(),
            opts.contains_key("--full"),
        )
    } else {
        let kind = match command {
            "tasks" => Kind::Action,
            "notes" => Kind::Note,
            "directions" => Kind::Direction,
            "questions" => Kind::Question,
            "expectations" => Kind::Expectation,
            _ => bail!("{}", tr!("cli.text_004", command)),
        };
        if let Some(status) = opts.get("--status") {
            ensure!(
                kind.statuses().contains(status),
                "{}",
                tr!("cli.text_003", kind.key(), status)
            );
        }
        let from = opts
            .get("--from")
            .map(|s| dates::parse_date(s))
            .transpose()?;
        let to = opts.get("--to").map(|s| dates::parse_date(s)).transpose()?;
        ensure!(
            !matches!((from,to),(Some(a),Some(b)) if a>b),
            "{}",
            tr!("cli.text_002")
        );
        let query = opts
            .get("--query")
            .map(|s| s.to_lowercase())
            .unwrap_or_default();
        let items = store
            .files
            .iter()
            .filter(|(n, _)| opts.get("--workspace").is_none_or(|w| *w == n.as_str()))
            .flat_map(|(n, w)| w.tasks.iter().map(move |t| (n, t)))
            .filter(|(_, t)| t.kind == kind && opts.get("--status").is_none_or(|s| *s == t.status))
            .filter(|(n, t)| {
                query.is_empty()
                    || format!("{n}\n{}\n{}", t.title, t.notes)
                        .to_lowercase()
                        .contains(&query)
            })
            .filter(|(_, t)| {
                from.is_none() && to.is_none()
                    || [t.scheduled, t.due, t.expectation]
                        .into_iter()
                        .flatten()
                        .any(|d| from.is_none_or(|f| d >= f) && to.is_none_or(|end| d <= end))
            })
            .map(|(n, t)| entity(&store, n, t, opts.contains_key("--full")))
            .collect::<Vec<_>>();
        json!({"items":items})
    };
    if write {
        ensure!(
            store.warning.is_empty(),
            "{}",
            tr!("cli.text_001", store.warning)
        );
    }
    let mut result = result;
    result["schema_version"] = json!(2);
    result["home"] = json!(store.root);
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

pub fn id(workspace: &str, task: &Task) -> String {
    if task.source.eq_ignore_ascii_case("tasks.md") {
        format!("{workspace}/{}#L{}", task.source, task.line)
    } else {
        format!("{workspace}/{}", task.source)
    }
}
fn find<'a>(store: &'a Store, target: &str) -> Result<(&'a str, &'a Task)> {
    store
        .files
        .iter()
        .flat_map(|(n, w)| w.tasks.iter().map(move |t| (n.as_str(), t)))
        .find(|(n, t)| id(n, t) == target)
        .context(tr!("cli.text_017"))
}
pub fn entity(store: &Store, workspace: &str, task: &Task, full: bool) -> Value {
    let mut item = json!({"id":id(workspace,task), "kind":task.kind, "workspace":workspace, "title":task.title,
        "status":task.status, "scheduled":task.scheduled.map(|d|d.to_string()), "due":task.due.map(|d|d.to_string()),
        "date":task.expectation.map(|d|d.to_string()),
        "path":store.root.join(workspace).join(&task.source), "line":task.line,"hash":task.hash,
        "validation_error":task.date_error,
        "allowed_transitions":task.kind.statuses().iter().filter(|s|task.kind != Kind::Note && task.kind.transition(&task.status,s).is_ok()).collect::<Vec<_>>()});
    if full {
        item["notes"] = json!(task.notes);
        item["completion_condition"] = json!(memory::section(&task.notes, "Completion Condition"));
        item["supplement"] = json!(memory::section(&task.notes, "Supplement"));
    }
    item
}
pub fn resume(store: &Store, workspace: Option<&str>, full: bool) -> Value {
    let mut result = json!({"today": dates::today().to_string(), "actions":[], "notes":[], "directions":[], "questions":[], "expectations":[], "issues":[]});
    for (name, file) in &store.files {
        if workspace.is_some_and(|w| w != name) {
            continue;
        }
        for task in &file.tasks {
            let item = entity(store, name, task, full);
            if !task.date_error.is_empty() {
                result["issues"].as_array_mut().unwrap().push(item);
                continue;
            }
            if !memory::in_resume(task) {
                continue;
            }
            let key = match task.kind {
                Kind::Action => "actions",
                Kind::Note => "notes",
                Kind::Direction => "directions",
                Kind::Question => "questions",
                Kind::Expectation => "expectations",
            };
            result[key].as_array_mut().unwrap().push(item);
        }
    }
    result["expectations"]
        .as_array_mut()
        .unwrap()
        .sort_by(|a, b| a["date"].as_str().cmp(&b["date"].as_str()));
    result
}
