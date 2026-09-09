use crate::tr;
use std::{fs, path::Path};

use anyhow::{Result, ensure};

use crate::store::{atomic_create, digest, parse_tasks, task_slug, validate_name, validate_source};

const PENDING: &str = ".minitask-split";

fn read_regular(path: &Path) -> Result<Vec<u8>> {
    ensure!(
        fs::symlink_metadata(path)?.is_file(),
        "{}",
        tr!("migration.text_013", path.display())
    );
    Ok(fs::read(path)?)
}

pub(crate) fn recover(root: &Path) -> Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir()
            && validate_name(&entry.file_name().to_string_lossy()).is_ok()
            && entry.path().join(PENDING).try_exists()?
        {
            resume(&entry.path())?;
        }
    }
    Ok(())
}

pub(crate) fn split(root: &Path, name: &str) -> Result<usize> {
    validate_name(name)?;
    let workspace = root.join(name);
    ensure!(
        fs::symlink_metadata(&workspace)?.is_dir(),
        "{}",
        tr!("migration.text_012")
    );
    let source = workspace.join("tasks.md");
    if !source.try_exists()? {
        return Ok(0);
    }
    let original = read_regular(&source)?;
    let (tasks, incomplete) = parse_tasks(&original)?;
    ensure!(!incomplete, "{}", tr!("migration.text_011", name));
    let hash = digest(&original);
    let stage = tempfile::Builder::new()
        .prefix(".split-")
        .tempdir_in(&workspace)?;
    atomic_create(&stage.path().join("original"), &original)?;
    let mut names = Vec::new();
    for (i, task) in tasks.iter().enumerate() {
        // Offsets are byte positions, including BOM and CRLF. Preserve the entire body.
        let line_start = |offset: usize| {
            original[..offset]
                .iter()
                .rposition(|&b| b == b'\n')
                .map_or(0, |p| p + 1)
        };
        let start = line_start(task.offset);
        let end = tasks
            .get(i + 1)
            .map_or(original.len(), |next| line_start(next.offset));
        let mut document = original[start..end].to_vec();
        let bom = if document.starts_with(&[0xef, 0xbb, 0xbf]) {
            3
        } else {
            0
        };
        if document[bom..].starts_with(b"## [") {
            document.splice(bom..bom + 3, b"- ".iter().copied());
        }
        let filename = format!(
            "legacy-{}-{i:05}-{}.md",
            &hash[..16],
            task_slug(&task.title)
        );
        ensure!(
            !workspace.join(&filename).try_exists()?,
            "{}",
            tr!("migration.text_010", filename)
        );
        atomic_create(&stage.path().join(&filename), &document)?;
        names.push(filename);
    }
    atomic_create(
        &stage.path().join("manifest.json"),
        &serde_json::to_vec(&names)?,
    )?;
    ensure!(
        !workspace.join(PENDING).try_exists()?,
        "{}",
        tr!("migration.text_009")
    );
    fs::rename(stage.path(), workspace.join(PENDING))?;
    resume(&workspace)?;
    Ok(tasks.len())
}

fn resume(workspace: &Path) -> Result<()> {
    let pending = workspace.join(PENDING);
    ensure!(
        fs::symlink_metadata(&pending)?.is_dir(),
        "{}",
        tr!("migration.text_008")
    );
    let original = read_regular(&pending.join("original"))?;
    let hash = digest(&original);
    let names: Vec<String> =
        serde_json::from_slice(&read_regular(&pending.join("manifest.json"))?)?;
    let prefix = format!("legacy-{}-", &hash[..16]);
    let mut documents = Vec::new();
    for name in &names {
        validate_source(name)?;
        ensure!(name.starts_with(&prefix), "{}", tr!("migration.text_007"));
        let document = read_regular(&pending.join(name))?;
        let target = workspace.join(name);
        if target.try_exists()? {
            ensure!(
                read_regular(&target)? == document,
                "{}",
                tr!("migration.text_006", name)
            );
        }
        documents.push(document);
    }
    let source = workspace.join("tasks.md");
    let backup = workspace.join(format!(".tasks-{hash}.bak"));
    if source.try_exists()? {
        ensure!(
            read_regular(&source)? == original,
            "{}",
            tr!("migration.text_004")
        );
        ensure!(!backup.try_exists()?, "{}", tr!("migration.text_003"));
        fs::rename(&source, &backup)?;
    }
    ensure!(
        read_regular(&backup)? == original,
        "{}",
        tr!("migration.text_002")
    );
    for (name, document) in names.iter().zip(documents) {
        let target = workspace.join(name);
        if !target.try_exists()? {
            atomic_create(&target, &document)?;
        }
    }
    // Publish completion before cleanup so an interrupted cleanup cannot break recovery.
    let complete = workspace.join(format!(".minitask-split-complete-{hash}"));
    ensure!(!complete.try_exists()?, "{}", tr!("migration.text_001"));
    fs::rename(&pending, &complete)?;
    for name in names
        .iter()
        .map(String::as_str)
        .chain(["manifest.json", "original"])
    {
        let _ = fs::remove_file(complete.join(name));
    }
    let _ = fs::remove_dir(complete);
    Ok(())
}
