use crate::dates::{self, DateField};
use crate::memory::{self, Kind};
use crate::tr;
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    rc::Rc,
    time::UNIX_EPOCH,
};
use tempfile::NamedTempFile;
use time::Date;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Task {
    #[serde(default)]
    pub kind: Kind,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub expectation: Option<Date>,
    pub title: String,
    pub notes: String,
    pub done: bool,
    pub line: usize,
    pub offset: usize,
    #[serde(default)]
    pub scheduled: Option<Date>,
    #[serde(default)]
    pub due: Option<Date>,
    #[serde(default)]
    pub date_error: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub hash: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct FileIndex {
    pub size: u64,
    pub mod_time: i64,
    pub hash: String,
    #[serde(default, deserialize_with = "null_tasks")]
    pub tasks: Vec<Task>,
}
fn null_tasks<'de, D: Deserializer<'de>>(de: D) -> std::result::Result<Vec<Task>, D::Error> {
    Ok(Option::<Vec<Task>>::deserialize(de)?.unwrap_or_default())
}
#[derive(Default)]
pub struct WorkspaceIndex {
    pub tasks: Vec<Task>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
struct DiskIndex {
    version: u32,
    #[serde(default)]
    locale: String,
    files: BTreeMap<String, Rc<FileIndex>>,
}
pub struct Store {
    pub root: PathBuf,
    pub files: BTreeMap<String, Rc<WorkspaceIndex>>,
    pub parsed: usize,
    pub warning: String,
    pub revision: u64,
    sources: BTreeMap<String, Rc<FileIndex>>,
    undo: Option<Undo>,
    read_only: bool,
}
struct Undo {
    workspace: String,
    source: String,
    before: Vec<u8>,
    after: Vec<u8>,
}
impl Store {
    pub fn open(root: &Path) -> Result<Self> {
        let root = std::path::absolute(root)?;
        fs::create_dir_all(&root)?;
        crate::migration::recover(&root)?;
        let sources = fs::read(root.join(".index.json"))
            .ok()
            .and_then(|d| serde_json::from_slice::<DiskIndex>(&d).ok())
            .filter(|i| i.version == 5 && i.locale == crate::i18n::cache_key())
            .map(|i| i.files)
            .unwrap_or_default();
        let mut store = Self {
            root,
            files: BTreeMap::new(),
            sources,
            parsed: 0,
            warning: String::new(),
            revision: 0,
            undo: None,
            read_only: false,
        };
        store.refresh(false, None)?;
        Ok(store)
    }
    /// Read current documents without locks, cache writes, or migration recovery.
    pub fn snapshot(root: &Path) -> Result<Self> {
        let root = std::path::absolute(root)?;
        ensure!(root.is_dir(), "{}", tr!("store.text_025", root.display()));
        for entry in fs::read_dir(&root)? {
            let entry = entry?;
            ensure!(
                !entry.file_type()?.is_dir()
                    || !entry.path().join(".minitask-split").try_exists()?,
                "{}",
                tr!("store.text_024")
            );
        }
        let mut store = Self {
            root,
            files: BTreeMap::new(),
            sources: BTreeMap::new(),
            parsed: 0,
            warning: String::new(),
            revision: 0,
            undo: None,
            read_only: true,
        };
        store.refresh(true, None)?;
        Ok(store)
    }
    /// The legacy aggregate file; new task operations use task_path.
    pub fn path(&self, workspace: &str) -> PathBuf {
        self.root.join(workspace).join("tasks.md")
    }
    pub fn task_path(&self, workspace: &str, task: &Task) -> Result<PathBuf> {
        self.source_path(workspace, &task.source)
    }
    fn source_path(&self, workspace: &str, source: &str) -> Result<PathBuf> {
        validate_name(workspace)?;
        validate_source(source)?;
        ensure!(
            fs::symlink_metadata(self.root.join(workspace))?.is_dir(),
            "{}",
            tr!("store.text_023")
        );
        Ok(self.root.join(workspace).join(source))
    }
    pub fn refresh(&mut self, force: bool, changed: Option<&str>) -> Result<usize> {
        let mut sources = BTreeMap::new();
        let mut names = Vec::new();
        let mut parsed = 0;
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            if !entry.file_type()?.is_dir() || validate_name(&name).is_err() {
                continue;
            }
            names.push(name.clone());
            for document in fs::read_dir(entry.path())? {
                let document = document?;
                let Ok(source) = document.file_name().into_string() else {
                    continue;
                };
                if source.starts_with('.') || !source.to_ascii_lowercase().ends_with(".md") {
                    continue;
                }
                validate_source(&source)?;
                let metadata = fs::symlink_metadata(document.path())?;
                ensure!(
                    metadata.is_file(),
                    "{}",
                    tr!("store.text_022", name, source)
                );
                let key = format!("{name}/{source}");
                if let Some(old) = self.sources.get(&key)
                    && !force
                    && changed != Some(name.as_str())
                    && changed != Some(key.as_str())
                    && old.size == metadata.len()
                    && old.mod_time == mod_time(&metadata)?
                {
                    sources.insert(key, old.clone());
                    continue;
                }
                sources.insert(key, Rc::new(read_index(&document.path(), &source)?));
                parsed += 1;
            }
        }
        names.sort();
        let dirty = force
            || parsed > 0
            || !sources.keys().eq(self.sources.keys())
            || !names.iter().eq(self.files.keys());
        self.parsed = parsed;
        if dirty || self.revision == 0 {
            let mut workspaces: BTreeMap<String, WorkspaceIndex> = names
                .into_iter()
                .map(|n| (n, WorkspaceIndex::default()))
                .collect();
            for (key, file) in &sources {
                let (name, _) = key.split_once('/').unwrap();
                workspaces
                    .get_mut(name)
                    .unwrap()
                    .tasks
                    .extend(file.tasks.iter().cloned());
            }
            self.files = workspaces
                .into_iter()
                .map(|(n, w)| (n, Rc::new(w)))
                .collect();
            self.revision += 1;
        }
        self.sources = sources;
        if dirty && !self.read_only {
            #[derive(Serialize)]
            #[serde(rename_all = "PascalCase")]
            struct Cache<'a> {
                version: u32,
                locale: String,
                files: &'a BTreeMap<String, Rc<FileIndex>>,
            }
            let data = serde_json::to_vec(&Cache {
                version: 5,
                locale: crate::i18n::cache_key(),
                files: &self.sources,
            })?;
            self.warning = match atomic_write(&self.root.join(".index.json"), &data, None) {
                Ok(()) => String::new(),
                Err(e) => tr!("store.text_021", e),
            };
        }
        Ok(parsed)
    }
    pub fn create_workspace(&mut self, name: &str) -> Result<()> {
        ensure!(!self.read_only, "{}", tr!("store.text_007"));
        validate_name(name)?;
        fs::create_dir(self.root.join(name))?;
        self.refresh(false, None)?;
        Ok(())
    }
    fn current(&self, name: &str, source: &str) -> Result<Vec<u8>> {
        let path = self.source_path(name, source)?;
        ensure!(
            fs::symlink_metadata(&path)?.is_file(),
            "{}",
            tr!("store.text_020")
        );
        Ok(fs::read(path)?)
    }
    pub fn add_task(&mut self, name: &str, title: &str) -> Result<String> {
        self.add_document(name, title, "## 詳細\n\n", false)
    }
    pub fn add_entity(
        &mut self,
        name: &str,
        kind: Kind,
        title: &str,
        date: Option<Date>,
        body: &str,
        condition: &str,
    ) -> Result<String> {
        ensure!(
            kind == Kind::Expectation || date.is_none(),
            "{}",
            tr!("store.text_019")
        );
        if kind == Kind::Note {
            return self.add_document(name, title, body, true);
        }
        let metadata = dates::metadata(title);
        ensure!(
            metadata.kind.is_none() && metadata.status.is_none() && metadata.expectation.is_none(),
            "{}",
            tr!("store.text_018")
        );
        let title = if kind == Kind::Expectation {
            format!(
                "{title} [kind:: expectation] [expectation:: {}]",
                date.context(tr!("store.text_029"))?
            )
        } else {
            format!(
                "{title} [kind:: {}] [status:: {}]",
                kind.key(),
                kind.initial()
            )
        };
        let body = if kind == Kind::Action {
            format!(
                "## Completion Condition\n{}\n\n## Supplement\n{}\n",
                condition.trim(),
                body
            )
        } else {
            body.to_owned()
        };
        self.add_document(name, &title, &body, false)
    }
    fn add_document(&mut self, name: &str, title: &str, body: &str, note: bool) -> Result<String> {
        ensure!(!self.read_only, "{}", tr!("store.text_007"));
        validate_name(name)?;
        ensure!(
            !title.trim().is_empty() && !title.chars().any(char::is_control),
            "{}",
            tr!("store.text_017")
        );
        let display_title = if note {
            title.trim().to_owned()
        } else {
            let metadata = dates::metadata(title);
            ensure!(metadata.error.is_empty(), "{}", metadata.error);
            metadata.title
        };
        ensure!(!display_title.is_empty(), "{}", tr!("store.text_016"));
        let now = time::OffsetDateTime::now_utc();
        let source = format!(
            "{:04}{:02}{:02}-{:02}{:02}{:02}-{:09}-{}.md",
            now.year(),
            now.month() as u8,
            now.day(),
            now.hour(),
            now.minute(),
            now.second(),
            now.nanosecond(),
            task_slug(&display_title)
        );
        let source = if note {
            format!("note-{source}")
        } else {
            source
        };
        let path = self.source_path(name, &source)?;
        let document = if note {
            format!("# {}\n\n{}", title.trim(), body)
        } else {
            format!("- [ ] {}\n\n{}", title.trim(), body)
        };
        let tasks = parse_document(document.as_bytes(), &source)?;
        ensure!(
            tasks.len() == 1 && tasks[0].title == display_title && tasks[0].date_error.is_empty(),
            "{}",
            tr!("store.text_014")
        );
        atomic_create(&path, document.as_bytes())?;
        self.refresh(false, Some(&format!("{name}/{source}")))?;
        Ok(source)
    }
    pub fn toggle(&mut self, name: &str, selected: &Task) -> Result<()> {
        if selected.kind != Kind::Action
            || selected.status
                != (if selected.done {
                    "completed"
                } else {
                    "actionable"
                })
            || self
                .current(name, &selected.source)?
                .windows(9)
                .any(|w| w == b"[status::")
        {
            let next = if selected.done {
                selected.kind.initial()
            } else {
                selected.kind.closed()
            };
            return self.transition(name, selected, next, "");
        }
        let (data, task) = self.checked_task(name, selected)?;
        let mut updated = data.clone();
        updated[task.offset] = if task.done { b' ' } else { b'x' };
        self.save_task(name, selected, data, updated)
    }
    pub fn set_date(
        &mut self,
        name: &str,
        selected: &Task,
        field: DateField,
        date: Option<Date>,
    ) -> Result<()> {
        ensure!(
            selected.kind == Kind::Action && field != DateField::Expectation
                || selected.kind == Kind::Expectation && field == DateField::Expectation,
            "{}",
            tr!("store.text_013")
        );
        ensure!(
            selected.kind != Kind::Expectation || date.is_some(),
            "{}",
            tr!("store.text_012")
        );
        let (data, task) = self.checked_task(name, selected)?;
        let start = task.offset + 3;
        let end = data[start..]
            .iter()
            .position(|&c| c == b'\r' || c == b'\n')
            .map(|i| start + i)
            .unwrap_or(data.len());
        let line = dates::replace_date(std::str::from_utf8(&data[start..end])?, field, date)?;
        let mut updated = data.clone();
        updated.splice(start..end, line.bytes());
        self.save_task(name, selected, data, updated)
    }
    pub fn transition(
        &mut self,
        name: &str,
        selected: &Task,
        status: &str,
        evidence: &str,
    ) -> Result<()> {
        ensure!(selected.kind != Kind::Note, "{}", tr!("store.text_011"));
        ensure!(selected.date_error.is_empty(), "{}", selected.date_error);
        selected.kind.transition(&selected.status, status)?;
        let (data, task) = self.checked_task(name, selected)?;
        let start = task.offset + 3;
        let end = data[start..]
            .iter()
            .position(|&c| c == b'\r' || c == b'\n')
            .map_or(data.len(), |i| start + i);
        let line =
            dates::replace_property(std::str::from_utf8(&data[start..end])?, "status", status)?;
        let mut updated = data.clone();
        updated[task.offset] = if memory::is_closed(status) {
            b'x'
        } else {
            b' '
        };
        updated.splice(start..end, line.bytes());
        if !evidence.trim().is_empty() {
            ensure!(
                !selected.source.eq_ignore_ascii_case("tasks.md"),
                "{}",
                tr!("store.text_010")
            );
            let heading = if selected.kind == Kind::Question {
                "Answer"
            } else {
                "Verification"
            };
            updated
                .extend_from_slice(format!("\n\n## {heading}\n{}\n", evidence.trim()).as_bytes());
        }
        self.save_task(name, selected, data, updated)
    }
    fn save_task(
        &mut self,
        name: &str,
        task: &Task,
        before: Vec<u8>,
        after: Vec<u8>,
    ) -> Result<()> {
        ensure!(!self.read_only, "{}", tr!("store.text_007"));
        atomic_write(&self.task_path(name, task)?, &after, Some(&before))?;
        self.undo = Some(Undo {
            workspace: name.into(),
            source: task.source.clone(),
            before,
            after,
        });
        self.refresh(false, Some(&format!("{name}/{}", task.source)))?;
        Ok(())
    }
    pub fn undo_last(&mut self) -> Result<()> {
        ensure!(!self.read_only, "{}", tr!("store.text_007"));
        let undo = self.undo.as_ref().context(tr!("store.text_028"))?;
        ensure!(
            self.current(&undo.workspace, &undo.source)? == undo.after,
            "{}",
            tr!("store.text_009")
        );
        atomic_write(
            &self.source_path(&undo.workspace, &undo.source)?,
            &undo.before,
            Some(&undo.after),
        )?;
        let key = format!("{}/{}", undo.workspace, undo.source);
        self.undo = None;
        self.refresh(false, Some(&key))?;
        Ok(())
    }
    fn checked_task(&self, name: &str, selected: &Task) -> Result<(Vec<u8>, Task)> {
        let data = self.current(name, &selected.source)?;
        ensure!(digest(&data) == selected.hash, "{}", tr!("store.text_008"));
        let task = parse_document(&data, &selected.source)?
            .into_iter()
            .find(|t| {
                t.line == selected.line && t.title == selected.title && t.done == selected.done
            })
            .context(tr!("store.text_027"))?;
        Ok((data, task))
    }
    pub fn split_legacy(&mut self) -> Result<usize> {
        ensure!(!self.read_only, "{}", tr!("store.text_007"));
        let mut count = 0;
        for name in self.files.keys() {
            count += crate::migration::split(&self.root, name)?;
        }
        self.undo = None;
        self.refresh(true, None)?;
        Ok(count)
    }
}

pub(crate) fn task_slug(title: &str) -> String {
    let s: String = title
        .chars()
        .take(32)
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | '<' | '>' | ':' | '"' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let s = s.trim_matches([' ', '.']);
    if s.is_empty() {
        "task".into()
    } else {
        s.into()
    }
}
pub(crate) fn validate_source(source: &str) -> Result<()> {
    ensure!(
        !source.is_empty()
            && !source.starts_with('.')
            && source.to_ascii_lowercase().ends_with(".md")
            && !source.contains(['/', '\\', ':'])
            && !source.chars().any(char::is_control),
        "{}",
        tr!("store.text_006")
    );
    Ok(())
}
pub(crate) fn atomic_create(path: &Path, data: &[u8]) -> Result<()> {
    let mut temp = NamedTempFile::new_in(path.parent().context(tr!("store.text_026"))?)?;
    temp.write_all(data)?;
    temp.as_file().sync_all()?;
    temp.persist_noclobber(path).map_err(|e| e.error)?;
    Ok(())
}
pub fn lock_store(root: &Path) -> Result<File> {
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join(".lock"))?;
    lock.try_lock().context(tr!("store.locked"))?;
    Ok(lock)
}

pub(crate) fn digest(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

fn mod_time(metadata: &fs::Metadata) -> Result<i64> {
    Ok(metadata
        .modified()?
        .duration_since(UNIX_EPOCH)?
        .as_nanos()
        .try_into()?)
}

fn read_index(path: &Path, source: &str) -> Result<FileIndex> {
    let mut file = File::open(path)?;
    let before = file.metadata()?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;
    let after = file.metadata()?;
    ensure!(
        before.len() == after.len() && before.modified()? == after.modified()?,
        "{}",
        tr!("store.text_005")
    );
    let mut tasks = parse_document(&data, source)?;
    let hash = digest(&data);
    for task in &mut tasks {
        task.source = source.into();
        task.hash = hash.clone();
    }
    Ok(FileIndex {
        size: after.len(),
        mod_time: mod_time(&after)?,
        hash,
        tasks,
    })
}

pub(crate) fn parse_tasks(data: &[u8]) -> Result<(Vec<Task>, bool)> {
    let text = std::str::from_utf8(data).context(tr!("store.utf8"))?;
    let mut tasks: Vec<Task> = Vec::new();
    let (mut fence, mut fence_len, mut offset) = (0u8, 0usize, 0usize);
    let (mut frontmatter, mut comment) = (false, false);
    let mut note: Option<(usize, usize, bool)> = None;
    for (line_no, raw) in text.split_inclusive('\n').enumerate() {
        let mut line = raw.trim_end_matches(['\r', '\n']);
        let mut line_offset = offset;
        if line_no == 0 && line.starts_with('\u{feff}') {
            line = &line[3..];
            line_offset += 3;
        }
        if line_no == 0 && line == "---" {
            frontmatter = true;
            offset += raw.len();
            continue;
        }
        if frontmatter {
            if matches!(line, "---" | "...") {
                frontmatter = false;
            }
            offset += raw.len();
            continue;
        }
        let visible = if fence == 0 {
            dates::uncomment(line, &mut comment)
        } else {
            line.to_owned()
        };
        // End a list task's note at the first non-indented block. Legacy notes
        // keep their old section-until-next-task behavior.
        if let Some((index, start, false)) = note
            && !line.trim().is_empty()
            && !line.starts_with([' ', '\t'])
            && fence == 0
        {
            tasks[index].notes = list_note(&text[start..offset]);
            note = None;
        }
        let line = visible.as_str();
        let trimmed = line.trim_start_matches(' ');
        let bytes = trimmed.as_bytes();
        if line.len() - trimmed.len() <= 3 && bytes.len() >= 3 && matches!(bytes[0], b'`' | b'~') {
            let count = bytes.iter().take_while(|&&byte| byte == bytes[0]).count();
            if fence == 0 && count >= 3 {
                fence = bytes[0];
                fence_len = count;
            } else if fence == bytes[0] && count >= fence_len && trimmed[count..].trim().is_empty()
            {
                fence = 0;
            }
        } else if fence == 0 {
            if line == "## Tasks"
                && let Some((index, start, true)) = note
            {
                tasks[index].notes = text[start..offset].trim().into();
                note = None;
            }
            let bytes = line.as_bytes();
            let legacy = line.starts_with("## [");
            let status = if legacy { 4 } else { 3 };
            if bytes.len() > status + 3
                && (legacy || !note.is_some_and(|(_, _, legacy)| legacy))
                && (legacy || matches!(bytes[0], b'-' | b'*' | b'+') && &bytes[1..3] == b" [")
                && matches!(bytes[status], b' ' | b'x' | b'X')
                && &bytes[status + 1..status + 3] == b"] "
                && !line[status + 3..].trim().is_empty()
            {
                if let Some((index, start, old_legacy)) = note.take() {
                    tasks[index].notes = if old_legacy {
                        text[start..offset].trim().into()
                    } else {
                        list_note(&text[start..offset])
                    };
                }
                let metadata = dates::metadata(&line[status + 3..]);
                let kind = metadata.kind.as_deref().map(Kind::parse).transpose();
                let mut issue = metadata.error;
                let kind = match kind {
                    Ok(kind) => kind.unwrap_or(if metadata.expectation.is_some() {
                        Kind::Expectation
                    } else {
                        Kind::Action
                    }),
                    Err(e) => {
                        issue = e.to_string();
                        Kind::Action
                    }
                };
                let done = bytes[status] != b' ';
                let state = metadata
                    .status
                    .unwrap_or_else(|| if done { kind.closed() } else { kind.initial() }.into());
                if !kind.statuses().contains(&state.as_str()) || done != memory::is_closed(&state) {
                    issue = tr!("store.inconsistent_status").into();
                }
                if kind == Kind::Expectation
                    && (metadata.expectation.is_none()
                        || metadata.scheduled.is_some()
                        || metadata.due.is_some())
                {
                    issue = tr!("store.expectation_fields").into();
                }
                if kind != Kind::Expectation && metadata.expectation.is_some() {
                    issue = tr!("store.expectation_kind").into();
                }
                if !matches!(kind, Kind::Action | Kind::Expectation)
                    && (metadata.scheduled.is_some() || metadata.due.is_some())
                {
                    issue = tr!("store.action_dates").into();
                }
                tasks.push(Task {
                    kind,
                    status: state,
                    expectation: metadata.expectation,
                    title: metadata.title,
                    source: String::new(),
                    hash: String::new(),
                    notes: String::new(),
                    done: bytes[status] != b' ',
                    line: line_no + 1,
                    offset: line_offset + status,
                    scheduled: metadata.scheduled,
                    due: metadata.due,
                    date_error: issue,
                });
                note = Some((tasks.len() - 1, offset + raw.len(), legacy));
            }
        }
        offset += raw.len();
    }
    if let Some((index, start, legacy)) = note {
        tasks[index].notes = if legacy {
            text[start..].trim().into()
        } else {
            list_note(&text[start..])
        };
    }
    Ok((tasks, fence != 0 || frontmatter || comment))
}

fn list_note(text: &str) -> String {
    text.lines()
        .map(|line| {
            line.strip_prefix("  ")
                .or_else(|| line.strip_prefix('\t'))
                .unwrap_or(line)
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .into()
}

pub(crate) fn validate_name(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty()
            && name.chars().count() <= 64
            && name.trim() == name
            && !name.starts_with('.')
            && !name.ends_with('.')
            && !name.contains(['/', '\\', '<', '>', ':', '"', '|', '?', '*'])
            && !name.chars().any(char::is_control),
        "{}",
        tr!("store.text_004")
    );
    let base = name.split('.').next().unwrap_or_default().to_uppercase();
    if matches!(
        base.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || (base.len() == 4
        && (base.starts_with("COM") || base.starts_with("LPT"))
        && matches!(base.as_bytes()[3], b'1'..=b'9'))
    {
        bail!("{}", tr!("store.text_003"));
    }
    Ok(())
}

pub fn atomic_write(path: &Path, data: &[u8], expected: Option<&[u8]>) -> Result<()> {
    let permissions = match fs::symlink_metadata(path) {
        Ok(metadata) => {
            ensure!(metadata.is_file(), "{}", tr!("store.text_002"));
            Some(metadata.permissions())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let mut temp = NamedTempFile::new_in(path.parent().context(tr!("store.text_026"))?)?;
    if let Some(permissions) = permissions {
        temp.as_file().set_permissions(permissions)?;
    }
    temp.write_all(data)?;
    temp.as_file().sync_all()?;
    if let Some(expected) = expected {
        ensure!(fs::read(path)? == expected, "{}", tr!("store.text_001"));
    }
    temp.persist(path).map_err(|error| error.error)?;
    Ok(())
}

fn parse_document(data: &[u8], source: &str) -> Result<Vec<Task>> {
    if source.starts_with("note-") {
        let text = std::str::from_utf8(data)?;
        let first = text
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or(source);
        let title = first.strip_prefix("# ").unwrap_or(first).trim().to_owned();
        return Ok(vec![Task {
            kind: Kind::Note,
            status: "active".into(),
            expectation: None,
            title,
            notes: text.into(),
            done: false,
            line: 1,
            offset: 0,
            scheduled: None,
            due: None,
            date_error: String::new(),
            source: String::new(),
            hash: String::new(),
        }]);
    }
    let (mut tasks, _) = parse_tasks(data)?;
    if !source.eq_ignore_ascii_case("tasks.md") {
        tasks.truncate(1);
        if let Some(task) = tasks.first_mut() {
            let text = std::str::from_utf8(data)?;
            task.notes = text
                .split_inclusive('\n')
                .skip(task.line)
                .collect::<String>()
                .trim()
                .to_owned();
        }
    }
    Ok(tasks)
}
