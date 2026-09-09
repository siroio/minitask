use crate::memory::{self, Kind};
use crate::tr;
use crate::{
    calendar::Calendar,
    dates::{self, DateField},
    editor,
    store::{Store, Task, atomic_write},
};
use anyhow::{Context, Result, ensure};
use crossterm::{
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers,
    },
    execute,
    terminal::{EnterAlternateScreen, enable_raw_mode},
};
use ratatui::{DefaultTerminal, Frame, widgets::ListState};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs, io,
    time::{Duration, Instant},
};
mod render;

#[derive(Clone, Copy, PartialEq, Deserialize, Serialize)]
enum View {
    Today,
    Upcoming,
    Unscheduled,
    All,
    Done,
    Workspace,
    Resume,
    Horizon,
    Notes,
    Directions,
    Questions,
}
impl View {
    fn key(self) -> char {
        match self {
            Self::Today => '1',
            Self::Upcoming => '2',
            Self::Unscheduled => '3',
            Self::All => '4',
            Self::Done => '5',
            Self::Resume => '6',
            Self::Horizon => '7',
            Self::Notes => '8',
            Self::Directions => '9',
            Self::Questions => 'Q',
            Self::Workspace => '0',
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Today => tr!("view.today"),
            Self::Upcoming => tr!("view.upcoming"),
            Self::Unscheduled => tr!("view.unscheduled"),
            Self::All => tr!("view.all"),
            Self::Done => tr!("view.done"),
            Self::Workspace => tr!("view.workspace"),
            Self::Resume => tr!("view.resume"),
            Self::Horizon => tr!("view.horizon"),
            Self::Notes => tr!("view.notes"),
            Self::Directions => tr!("view.directions"),
            Self::Questions => tr!("view.questions"),
        }
    }
    fn matches(self, task: &Task, today: time::Date) -> bool {
        match self {
            Self::Resume => return memory::in_resume(task) || !task.date_error.is_empty(),
            Self::Horizon => return task.kind == Kind::Expectation,
            Self::Notes => return task.kind == Kind::Note,
            Self::Directions => return task.kind == Kind::Direction,
            Self::Questions => return task.kind == Kind::Question,
            Self::Workspace => return !task.done,
            _ => {}
        }
        if task.kind != Kind::Action {
            return false;
        }
        if self == Self::Done {
            return task.done;
        }
        if task.done {
            return false;
        }
        match self {
            Self::Today => [task.scheduled, task.due]
                .into_iter()
                .flatten()
                .any(|d| d <= today),
            Self::Upcoming => [task.scheduled, task.due]
                .into_iter()
                .flatten()
                .any(|d| d > today && (d - today).whole_days() <= 7),
            Self::Unscheduled => task.scheduled.is_none() && task.due.is_none(),
            _ => true,
        }
    }
}
const VIEWS: [View; 10] = [
    View::Resume,
    View::Today,
    View::Upcoming,
    View::Unscheduled,
    View::All,
    View::Done,
    View::Horizon,
    View::Notes,
    View::Directions,
    View::Questions,
];
#[derive(Clone, Copy, PartialEq)]
enum Focus {
    Views,
    Workspaces,
    Tasks,
}
const COMMANDS: &[(&str, char)] = &[
    ("ui.text_048", '1'),
    ("ui.text_047", '2'),
    ("ui.text_046", '3'),
    ("ui.text_045", '4'),
    ("ui.text_044", '5'),
    ("ui.text_043", '0'),
    ("ui.text_042", '6'),
    ("ui.text_041", '7'),
    ("ui.text_040", '8'),
    ("ui.text_039", '9'),
    ("ui.text_038", 'Q'),
    ("ui.text_037", 'm'),
    ("ui.text_036", 'v'),
    ("ui.text_035", 'a'),
    ("ui.text_034", 'n'),
    ("ui.text_033", 'c'),
    ("ui.text_032", 's'),
    ("ui.text_031", 'd'),
    ("ui.text_030", 'e'),
    ("ui.text_029", 'u'),
    ("ui.text_028", 'r'),
    ("ui.text_027", 'R'),
    ("ui.text_026", '?'),
    ("ui.text_025", 'q'),
];
#[derive(Clone, Copy, PartialEq)]
enum Prompt {
    Workspace,
    Search,
    Palette,
    Status,
}
#[derive(Default)]
struct Input {
    chars: Vec<char>,
    cursor: usize,
}
impl Input {
    fn new(s: &str) -> Self {
        let chars: Vec<_> = s.chars().collect();
        let cursor = chars.len();
        Self { chars, cursor }
    }
    fn text(&self) -> String {
        self.chars.iter().collect()
    }
    fn paste(&mut self, s: &str) {
        for c in s.chars().filter(|c| !c.is_control()) {
            self.chars.insert(self.cursor, c);
            self.cursor += 1;
        }
    }
    fn key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(self.chars.len()),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.chars.len(),
            KeyCode::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                self.chars.remove(self.cursor);
            }
            KeyCode::Delete if self.cursor < self.chars.len() => {
                self.chars.remove(self.cursor);
            }
            KeyCode::Char(c) if !c.is_control() => self.paste(&c.to_string()),
            _ => {}
        }
    }
}
struct AddForm {
    kind: Kind,
    fields: [Input; 3],
    workspace: String,
    focus: usize,
}
impl AddForm {
    fn field_count(&self) -> usize {
        match self.kind {
            Kind::Action => 4,
            Kind::Expectation => 3,
            _ => 2,
        }
    }
}
#[derive(Deserialize, Serialize)]
struct State {
    #[serde(rename = "Workspace")]
    workspace: String,
    #[serde(default, rename = "View")]
    view: Option<View>,
}
pub enum Action {
    None,
    Edit,
}
struct DateEdit {
    workspace: String,
    task: Task,
    field: DateField,
    previous: Option<Calendar>,
}
pub struct App {
    pub store: Store,
    pub quit: bool,
    editor: String,
    names: Vec<String>,
    workspaces: ListState,
    sidebar: ListState,
    tasks: ListState,
    focus: Focus,
    view: View,
    counts: [usize; 10],
    day: time::Date,
    visible: Vec<(String, usize)>,
    query: String,
    prompt: Option<Prompt>,
    input: Input,
    search_before: Option<(String, ListState)>,
    palette_index: usize,
    add: Option<AddForm>,
    detail: bool,
    help: bool,
    help_offset: usize,
    detail_offset: usize,
    message: String,
    calendar: Option<Calendar>,
    list_before: Option<(String, ListState)>,
    date_edit: Option<DateEdit>,
    date_input: Option<Input>,
    status_edit: Option<(String, Task)>,
}
impl App {
    pub fn new(store: Store, editor: String) -> Self {
        let state = fs::read(store.root.join(".state.json"))
            .ok()
            .and_then(|d| serde_json::from_slice::<State>(&d).ok());
        let preferred = state
            .as_ref()
            .map(|s| s.workspace.as_str())
            .unwrap_or("")
            .to_owned();
        let view = state
            .map(|s| s.view.unwrap_or(View::Workspace))
            .unwrap_or(View::Resume);
        let mut app = Self {
            store,
            quit: false,
            editor,
            names: Vec::new(),
            workspaces: ListState::default(),
            sidebar: ListState::default(),
            tasks: ListState::default(),
            focus: Focus::Tasks,
            view,
            counts: [0; 10],
            day: dates::today(),
            visible: Vec::new(),
            query: String::new(),
            prompt: None,
            input: Input::default(),
            search_before: None,
            palette_index: 0,
            add: None,
            detail: false,
            help: false,
            help_offset: 0,
            detail_offset: 0,
            message: String::new(),
            calendar: None,
            list_before: None,
            date_edit: None,
            date_input: None,
            status_edit: None,
        };
        app.reload(&preferred);
        app
    }
    pub fn workspace(&self) -> &str {
        self.names
            .get(self.workspaces.selected().unwrap_or(0))
            .map(String::as_str)
            .unwrap_or("")
    }
    fn reload(&mut self, preferred: &str) {
        self.names = self.store.files.keys().cloned().collect();
        let i = self
            .names
            .iter()
            .position(|n| n == preferred)
            .unwrap_or(self.workspaces.selected().unwrap_or(0))
            .min(self.names.len().saturating_sub(1));
        self.workspaces
            .select((!self.names.is_empty()).then_some(i));
        if let Some(i) = VIEWS.iter().position(|v| *v == self.view) {
            self.sidebar.select(Some(i));
        }
        if self.names.is_empty() && self.focus == Focus::Workspaces {
            self.focus = Focus::Views;
        }
        self.filter();
    }
    fn filter(&mut self) {
        let query = self.query.to_lowercase();
        let today = dates::today();
        self.day = today;
        self.counts = [0; 10];
        let mut visible = Vec::new();
        // ponytail: scan the existing in-memory index; add search postings only if measured latency requires them.
        for (name, file) in &self.store.files {
            for (i, task) in file.tasks.iter().enumerate() {
                for (j, v) in VIEWS.iter().enumerate() {
                    if v.matches(task, today) {
                        self.counts[j] += 1;
                    }
                }
                let included = if let Some(c) = &self.calendar {
                    (c.all_workspaces || name == self.workspace())
                        && task.date_error.is_empty()
                        && (task.scheduled == Some(c.selected)
                            || task.due == Some(c.selected)
                            || task.expectation == Some(c.selected))
                } else {
                    (self.view != View::Workspace || name == self.workspace())
                        && self.view.matches(task, today)
                };
                if !included {
                    continue;
                }
                if query.is_empty()
                    || task.title.to_lowercase().contains(&query)
                    || task.notes.to_lowercase().contains(&query)
                    || name.to_lowercase().contains(&query)
                    || [task.scheduled, task.due, task.expectation]
                        .into_iter()
                        .flatten()
                        .any(|d| d.to_string().contains(&query))
                {
                    visible.push((name.clone(), i));
                }
            }
        }
        if self.calendar.is_none() && matches!(self.view, View::Today | View::Upcoming) {
            visible.sort_by_key(|(n, i)| {
                let t = &self.store.files[n].tasks[*i];
                let group = if self.view == View::Today {
                    today_group(t, today).0
                } else {
                    0
                };
                (
                    group,
                    [t.scheduled, t.due]
                        .into_iter()
                        .flatten()
                        .filter(|d| self.view != View::Upcoming || *d > today)
                        .min(),
                )
            });
        }
        if self.calendar.is_none() && self.view == View::Resume {
            visible.sort_by_key(|(n, i)| {
                let t = &self.store.files[n].tasks[*i];
                (
                    match t.kind {
                        Kind::Action if t.status == "in_progress" => 0,
                        Kind::Action => 1,
                        Kind::Direction => 2,
                        Kind::Question => 3,
                        Kind::Expectation => 4,
                        Kind::Note => 5,
                    },
                    t.expectation,
                )
            });
        } else if self.view == View::Horizon {
            visible.sort_by_key(|(n, i)| self.store.files[n].tasks[*i].expectation);
        }
        self.visible = visible;
        self.tasks.select(
            (!self.visible.is_empty()).then_some(
                self.tasks
                    .selected()
                    .unwrap_or(0)
                    .min(self.visible.len().saturating_sub(1)),
            ),
        );
        self.detail_offset = 0;
    }
    fn selected_task(&self) -> Option<&Task> {
        let (n, i) = self.visible.get(self.tasks.selected()?)?;
        self.store.files.get(n)?.tasks.get(*i)
    }
    fn task_workspace(&self) -> &str {
        self.visible
            .get(self.tasks.selected().unwrap_or(0))
            .map(|(n, _)| n.as_str())
            .unwrap_or(self.workspace())
    }
    fn set_view(&mut self, view: View) {
        self.view = view;
        self.calendar = None;
        self.list_before = None;
        self.detail = false;
        self.query.clear();
        self.tasks.select(Some(0));
        self.focus = Focus::Tasks;
        let n = self.workspace().to_owned();
        self.reload(&n);
    }
    fn open_date(&mut self, field: DateField) -> Result<()> {
        let task = self.selected_task().context(tr!("ui.text_024"))?.clone();
        ensure!(
            task.date_error.is_empty(),
            "{}",
            tr!("ui.text_010", task.date_error)
        );
        ensure!(
            matches!(task.kind, Kind::Action | Kind::Expectation),
            "{}",
            tr!("ui.text_009")
        );
        let field = if task.kind == Kind::Expectation {
            DateField::Expectation
        } else {
            field
        };
        let name = self.task_workspace().to_owned();
        let date = match field {
            DateField::Scheduled => task.scheduled,
            DateField::Due => task.due,
            DateField::Expectation => task.expectation,
        }
        .unwrap_or_else(|| {
            self.calendar
                .as_ref()
                .map(|c| c.selected)
                .unwrap_or_else(dates::today)
        });
        let previous = self.calendar.replace(Calendar::new(date));
        self.date_edit = Some(DateEdit {
            workspace: name.clone(),
            task,
            field,
            previous,
        });
        Ok(())
    }
    fn finish_date(&mut self, save: bool, clear: bool) -> Result<()> {
        let edit = self.date_edit.as_ref().context(tr!("ui.text_023"))?;
        if save {
            self.store.set_date(
                &edit.workspace,
                &edit.task,
                edit.field,
                self.calendar
                    .as_ref()
                    .map(|c| c.selected)
                    .filter(|_| !clear),
            )?;
        }
        self.calendar = self.date_edit.take().unwrap().previous;
        self.date_input = None;
        if save {
            self.message = tr!("ui.text_022").into();
        }
        let n = self.workspace().to_owned();
        self.reload(&n);
        Ok(())
    }
    fn calendar_counts(&self) -> BTreeMap<time::Date, usize> {
        let mut counts = BTreeMap::new();
        let scope = self
            .date_edit
            .as_ref()
            .map(|e| e.workspace.as_str())
            .unwrap_or(self.workspace());
        for (n, f) in &self.store.files {
            if n != scope && !self.calendar.as_ref().is_some_and(|c| c.all_workspaces) {
                continue;
            }
            for t in &f.tasks {
                if !t.date_error.is_empty() {
                    continue;
                }
                for d in [
                    t.scheduled,
                    t.due.filter(|d| Some(*d) != t.scheduled),
                    t.expectation,
                ]
                .into_iter()
                .flatten()
                {
                    *counts.entry(d).or_insert(0) += 1;
                }
            }
        }
        counts
    }
    pub fn save_state(&self) -> Result<()> {
        atomic_write(
            &self.store.root.join(".state.json"),
            &serde_json::to_vec(&State {
                workspace: self.workspace().into(),
                view: Some(self.view),
            })?,
            None,
        )
    }
    fn refresh(&mut self, force: bool) -> Result<bool> {
        let preferred = self.workspace().to_owned();
        let revision = self.store.revision;
        self.store.refresh(force, None)?;
        let changed = force
            || revision != self.store.revision
            || self.names.len() != self.store.files.len()
            || dates::today() != self.day;
        if changed {
            self.reload(&preferred);
        }
        Ok(changed)
    }
    fn open_prompt(&mut self, prompt: Prompt) {
        self.prompt = Some(prompt);
        self.palette_index = 0;
        if prompt == Prompt::Search {
            self.search_before = Some((self.query.clone(), self.tasks));
        }
        self.input = Input::new(if prompt == Prompt::Search {
            &self.query
        } else {
            ""
        });
    }
    fn update_search(&mut self) {
        if self.prompt == Some(Prompt::Search) {
            self.query = self.input.text();
            self.tasks.select(Some(0));
            self.filter();
        }
    }
    fn commands(&self) -> Vec<(&'static str, char)> {
        let q = self.input.text().to_lowercase();
        COMMANDS
            .iter()
            .map(|(s, k)| (crate::i18n::text(s), *k))
            .filter(|(s, k)| s.to_lowercase().contains(&q) || k.to_string() == q)
            .collect()
    }
    fn paste(&mut self, s: &str) {
        if let Some(input) = &mut self.date_input {
            input.paste(s);
        } else if let Some(add) = &mut self.add {
            if add.focus != 1 {
                add.fields[if add.focus == 0 { 0 } else { add.focus - 1 }].paste(s);
            }
        } else if self.prompt.is_some() {
            self.input.paste(s);
            self.palette_index = 0;
            self.update_search();
        }
    }
    fn save_add(&mut self) -> Result<()> {
        let add = self.add.as_ref().unwrap();
        let n = add.workspace.clone();
        let mut title = add.fields[0].text();
        let kind = add.kind;
        for (i, field) in [(1, DateField::Scheduled), (2, DateField::Due)] {
            if kind != Kind::Action {
                continue;
            }
            let s = add.fields[i].text();
            if !s.trim().is_empty() {
                title = dates::replace_date(
                    &title,
                    field,
                    Some(
                        dates::parse_date(s.trim())
                            .with_context(|| tr!("ui.text_008", field.label()))?,
                    ),
                )?;
            }
        }
        let date = if kind == Kind::Expectation {
            Some(dates::parse_date(add.fields[1].text().trim()).context(tr!("ui.text_021"))?)
        } else {
            None
        };
        let source = self.store.add_entity(&n, kind, &title, date, "", "")?;
        self.add = None;
        self.detail = false;
        self.query.clear();
        self.focus = Focus::Tasks;
        if self.calendar.is_none() {
            self.view = match kind {
                Kind::Action => View::Workspace,
                Kind::Expectation => View::Horizon,
                Kind::Note => View::Notes,
                Kind::Direction => View::Directions,
                Kind::Question => View::Questions,
            };
        }
        self.reload(&n);
        self.tasks
            .select(self.visible.iter().position(|(workspace, index)| {
                workspace == &n && self.store.files[workspace].tasks[*index].source == source
            }));
        if let Some(c) = &mut self.calendar {
            c.focus_days = false;
        }
        self.message = tr!("ui.text_007", n);
        Ok(())
    }
    pub fn event(&mut self, event: Event) -> Result<Action> {
        let key = match event {
            Event::Key(k) if k.kind != KeyEventKind::Release => k,
            Event::Paste(s) => {
                self.paste(&s);
                return Ok(Action::None);
            }
            _ => return Ok(Action::None),
        };
        let cancel = key.code == KeyCode::Esc
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL));
        if self.help {
            if cancel || matches!(key.code, KeyCode::Char('?') | KeyCode::Enter) {
                self.help = false;
                self.help_offset = 0;
            } else {
                let delta = match key.code {
                    KeyCode::Down | KeyCode::Char('j') => 1,
                    KeyCode::Up | KeyCode::Char('k') => -1,
                    KeyCode::PageDown => 5,
                    KeyCode::PageUp => -5,
                    _ => 0,
                };
                self.help_offset = self.help_offset.saturating_add_signed(delta);
            }
            return Ok(Action::None);
        }
        if let Some(input) = &mut self.date_input {
            if cancel {
                self.date_input = None;
            } else if key.code == KeyCode::Enter {
                let date = dates::parse_date(input.text().trim())?;
                self.calendar.as_mut().unwrap().selected = date;
                self.finish_date(true, false)?;
            } else {
                input.key(key.code);
            }
            return Ok(Action::None);
        }
        if self.date_edit.is_some() {
            if key.code == KeyCode::Char('?') {
                self.help = true;
            } else if cancel {
                self.finish_date(false, false)?;
            } else if matches!(
                key.code,
                KeyCode::Enter | KeyCode::Delete | KeyCode::Char('0')
            ) {
                self.finish_date(true, key.code != KeyCode::Enter)?;
            } else if key.code == KeyCode::Char('i') {
                self.date_input = Some(Input::new(
                    &self.calendar.as_ref().unwrap().selected.to_string(),
                ));
            } else if let Some(c) = &mut self.calendar {
                match key.code {
                    KeyCode::Char('1') => c.selected = dates::today(),
                    KeyCode::Char('2') => c.selected = dates::today() + time::Duration::days(1),
                    KeyCode::Char('7') => c.selected = dates::today() + time::Duration::days(7),
                    _ => {
                        c.navigate(key.code);
                    }
                }
            }
            return Ok(Action::None);
        }
        if let Some(add) = &mut self.add {
            if cancel {
                self.add = None;
            } else {
                match key.code {
                    KeyCode::Enter => self.save_add()?,
                    KeyCode::Tab => add.focus = (add.focus + 1) % add.field_count(),
                    KeyCode::BackTab => {
                        add.focus = (add.focus + add.field_count() - 1) % add.field_count()
                    }
                    KeyCode::Down | KeyCode::Right if add.focus == 1 => {
                        let i = self
                            .names
                            .iter()
                            .position(|n| *n == add.workspace)
                            .unwrap_or(0);
                        if let Some(n) = self
                            .names
                            .get((i + 1).min(self.names.len().saturating_sub(1)))
                        {
                            add.workspace = n.clone();
                        }
                    }
                    KeyCode::Up | KeyCode::Left if add.focus == 1 => {
                        let i = self
                            .names
                            .iter()
                            .position(|n| *n == add.workspace)
                            .unwrap_or(0);
                        if let Some(n) = self.names.get(i.saturating_sub(1)) {
                            add.workspace = n.clone();
                        }
                    }
                    _ if add.focus != 1 => {
                        add.fields[if add.focus == 0 { 0 } else { add.focus - 1 }].key(key.code)
                    }
                    _ => {}
                }
            }
            return Ok(Action::None);
        }
        if let Some(prompt) = self.prompt {
            if cancel {
                self.status_edit = None;
                self.prompt = None;
                if let Some((q, selection)) = self.search_before.take() {
                    self.query = q;
                    self.tasks = selection;
                    self.filter();
                }
            } else if key.code == KeyCode::Enter {
                match prompt {
                    Prompt::Workspace => {
                        let n = self.input.text().trim().to_owned();
                        self.store.create_workspace(&n)?;
                        self.view = View::Workspace;
                        self.query.clear();
                        self.focus = Focus::Tasks;
                        self.reload(&n);
                        self.message = tr!("ui.text_006", n);
                    }
                    Prompt::Status => {
                        let (name, task) = self.status_edit.as_ref().context(tr!("ui.text_020"))?;
                        let input = self.input.text();
                        let status = task
                            .kind
                            .statuses()
                            .iter()
                            .copied()
                            .find(|s| {
                                *s == input.trim() || task.kind.status_label(s) == input.trim()
                            })
                            .unwrap_or(input.trim());
                        self.store.transition(name, task, status, "")?;
                        self.status_edit = None;
                        let workspace = self.workspace().to_owned();
                        self.reload(&workspace);
                        self.message = tr!("ui.text_019").into();
                    }
                    Prompt::Search => {
                        self.search_before = None;
                        self.focus = Focus::Tasks;
                    }
                    Prompt::Palette => {
                        if let Some((_, c)) = self.commands().get(self.palette_index).copied() {
                            self.prompt = None;
                            return self.event(Event::Key(KeyEvent::new(
                                KeyCode::Char(c),
                                KeyModifiers::NONE,
                            )));
                        } else {
                            return Ok(Action::None);
                        }
                    }
                }
                self.prompt = None;
            } else if prompt == Prompt::Status && matches!(key.code, KeyCode::Up | KeyCode::Down) {
                if let Some((_, task)) = &self.status_edit {
                    let choices: Vec<_> = task
                        .kind
                        .statuses()
                        .iter()
                        .copied()
                        .filter(|s| task.kind.transition(&task.status, s).is_ok())
                        .collect();
                    let current = choices.iter().position(|s| {
                        task.kind.status_label(s) == self.input.text() || *s == self.input.text()
                    });
                    let next = current.map_or(0, |i| {
                        if key.code == KeyCode::Down {
                            (i + 1) % choices.len()
                        } else {
                            (i + choices.len() - 1) % choices.len()
                        }
                    });
                    if let Some(choice) = choices.get(next) {
                        self.input = Input::new(task.kind.status_label(choice));
                    }
                }
            } else if prompt == Prompt::Palette && matches!(key.code, KeyCode::Up | KeyCode::Down) {
                self.palette_index = self
                    .palette_index
                    .saturating_add_signed(if key.code == KeyCode::Down { 1 } else { -1 })
                    .min(self.commands().len().saturating_sub(1));
            } else {
                self.input.key(key.code);
                self.palette_index = 0;
                self.update_search();
            }
            return Ok(Action::None);
        }
        self.message.clear();
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.quit = true;
            return Ok(Action::None);
        }
        if self.detail && cancel {
            self.detail = false;
            return Ok(Action::None);
        }
        if self.detail && matches!(key.code, KeyCode::PageUp | KeyCode::PageDown) {
            self.detail_offset = self
                .detail_offset
                .saturating_add_signed(if key.code == KeyCode::PageDown { 5 } else { -5 });
            return Ok(Action::None);
        }
        if matches!(
            key.code,
            KeyCode::Tab
                | KeyCode::BackTab
                | KeyCode::Left
                | KeyCode::Right
                | KeyCode::Char('h' | 'l' | 'c')
        ) {
            self.detail = false;
        }
        if let Some(c) = &mut self.calendar {
            match key.code {
                KeyCode::Char('c') | KeyCode::Esc => {
                    self.calendar = None;
                    let before = self.list_before.take();
                    self.query = before.as_ref().map(|(q, _)| q.clone()).unwrap_or_default();
                    if let Some((_, tasks)) = before {
                        self.tasks = tasks;
                    }
                    self.filter();
                    return Ok(Action::None);
                }
                KeyCode::Tab | KeyCode::BackTab => {
                    c.focus_days = !c.focus_days;
                    return Ok(Action::None);
                }
                KeyCode::Enter if c.focus_days => {
                    c.focus_days = false;
                    return Ok(Action::None);
                }
                KeyCode::Char('w') => {
                    c.all_workspaces = !c.all_workspaces;
                    self.tasks.select(Some(0));
                    self.filter();
                    return Ok(Action::None);
                }
                _ => {}
            }
            if (c.focus_days
                || matches!(
                    key.code,
                    KeyCode::PageUp | KeyCode::PageDown | KeyCode::Char('t')
                ))
                && c.navigate(key.code)
            {
                self.tasks.select(Some(0));
                self.filter();
                return Ok(Action::None);
            }
        }
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char(c @ '1'..='9') => {
                self.set_view(*VIEWS.iter().find(|v| v.key() == c).unwrap())
            }
            KeyCode::Char('Q') => self.set_view(View::Questions),
            KeyCode::Char('0') => self.set_view(View::Workspace),
            KeyCode::Char('c') => {
                let date = self
                    .selected_task()
                    .and_then(|t| t.expectation.or(t.scheduled).or(t.due))
                    .unwrap_or_else(dates::today);
                let mut c = Calendar::new(date);
                self.list_before = Some((self.query.clone(), self.tasks));
                c.all_workspaces = self.view != View::Workspace;
                self.calendar = Some(c);
                self.query.clear();
                self.tasks.select(Some(0));
                self.filter();
            }
            KeyCode::Char('s') => self.open_date(DateField::Scheduled)?,
            KeyCode::Char('d') => self.open_date(DateField::Due)?,
            KeyCode::Tab | KeyCode::BackTab => {
                let panels: &[Focus] = if self.names.is_empty() {
                    &[Focus::Views, Focus::Tasks]
                } else {
                    &[Focus::Views, Focus::Workspaces, Focus::Tasks]
                };
                let i = panels.iter().position(|f| *f == self.focus).unwrap_or(0);
                let step = if key.code == KeyCode::BackTab {
                    panels.len() - 1
                } else {
                    1
                };
                self.focus = panels[(i + step) % panels.len()];
            }
            KeyCode::Char('h') | KeyCode::Left => {
                if let Some(c) = &mut self.calendar {
                    c.focus_days = true;
                } else {
                    self.focus = if self.view == View::Workspace && !self.names.is_empty() {
                        Focus::Workspaces
                    } else {
                        Focus::Views
                    };
                }
            }
            KeyCode::Char('l') | KeyCode::Right => self.focus = Focus::Tasks,
            KeyCode::Enter => {
                if self.focus != Focus::Tasks && self.calendar.is_none() {
                    self.move_selection(0);
                    self.focus = Focus::Tasks;
                } else {
                    self.detail = !self.detail;
                }
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_selection(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_selection(-1),
            KeyCode::PageUp => self.detail_offset = self.detail_offset.saturating_sub(5),
            KeyCode::PageDown => self.detail_offset = self.detail_offset.saturating_add(5),
            KeyCode::Esc => {
                self.query.clear();
                self.filter();
            }
            KeyCode::Char('n') => self.open_prompt(Prompt::Workspace),
            KeyCode::Char('a' | 'm') => {
                ensure!(!self.names.is_empty(), "{}", tr!("ui.text_005"));
                let kind = if key.code == KeyCode::Char('m') {
                    Kind::Expectation
                } else {
                    match self.view {
                        View::Horizon => Kind::Expectation,
                        View::Notes => Kind::Note,
                        View::Directions => Kind::Direction,
                        View::Questions => Kind::Question,
                        _ => Kind::Action,
                    }
                };
                let date = self
                    .calendar
                    .as_ref()
                    .map(|c| c.selected)
                    .or_else(|| (kind == Kind::Action).then(dates::today))
                    .map(|d| d.to_string())
                    .unwrap_or_default();
                self.add = Some(AddForm {
                    kind,
                    fields: [
                        Input::default(),
                        Input::new(&date),
                        Input::new(if kind == Kind::Action { &date } else { "" }),
                    ],
                    workspace: self.workspace().to_owned(),
                    focus: 0,
                });
            }
            KeyCode::Char('v') => {
                ensure!(
                    self.selected_task().is_some_and(|t| t.kind != Kind::Note),
                    "{}",
                    tr!("ui.text_004")
                );
                self.status_edit = Some((
                    self.task_workspace().to_owned(),
                    self.selected_task().unwrap().clone(),
                ));
                self.open_prompt(Prompt::Status);
            }
            KeyCode::Char('/') => self.open_prompt(Prompt::Search),
            KeyCode::Char(':') => self.open_prompt(Prompt::Palette),
            KeyCode::Char('?') => self.help = true,
            KeyCode::Char('u') => {
                self.store.undo_last()?;
                let n = self.workspace().to_owned();
                self.reload(&n);
                self.message = tr!("ui.text_018").into();
            }
            KeyCode::Char(' ')
                if self
                    .calendar
                    .as_ref()
                    .map(|c| !c.focus_days)
                    .unwrap_or(self.focus == Focus::Tasks) =>
            {
                if let Some(t) = self.selected_task().cloned() {
                    let n = self.task_workspace().to_owned();
                    self.store.toggle(&n, &t)?;
                    let ws = self.workspace().to_owned();
                    self.reload(&ws);
                    self.message = if t.done {
                        tr!("ui.text_017")
                    } else {
                        tr!("ui.text_016")
                    }
                    .into();
                }
            }
            KeyCode::Char('e') => return Ok(Action::Edit),
            KeyCode::Char(c @ ('r' | 'R')) => {
                self.refresh(c == 'R')?;
                self.message = tr!("ui.text_015").into();
            }
            _ => {}
        }
        Ok(Action::None)
    }
    fn move_selection(&mut self, delta: isize) {
        if self.focus != Focus::Tasks && self.calendar.is_none() {
            let (selection, count) = if self.focus == Focus::Views {
                (&mut self.sidebar, VIEWS.len())
            } else {
                (&mut self.workspaces, self.names.len())
            };
            let i = selection
                .selected()
                .unwrap_or(0)
                .saturating_add_signed(delta)
                .min(count.saturating_sub(1));
            selection.select((count > 0).then_some(i));
            self.view = if self.focus == Focus::Views {
                VIEWS[i]
            } else {
                View::Workspace
            };
            self.query.clear();
            self.tasks.select(Some(0));
            self.filter();
        } else {
            let i = self
                .tasks
                .selected()
                .unwrap_or(0)
                .saturating_add_signed(delta)
                .min(self.visible.len().saturating_sub(1));
            self.tasks.select((!self.visible.is_empty()).then_some(i));
            self.detail_offset = 0;
        }
    }
    fn edit(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        if self.calendar.is_some() {
            ensure!(self.selected_task().is_some(), "{}", tr!("ui.text_003"));
        }
        let name = self.task_workspace().to_owned();
        ensure!(!name.is_empty(), "{}", tr!("ui.text_002"));
        let task = self.selected_task().context(tr!("ui.text_014"))?;
        let changed = format!("{name}/{}", task.source);
        let mut command =
            editor::command(&self.editor, &self.store.task_path(&name, task)?, task.line)?;
        execute!(io::stdout(), DisableBracketedPaste)?;
        ratatui::try_restore().inspect_err(|_| self.quit = true)?;
        let status = command.status();
        // Reuse the terminal so repeated edits do not stack Ratatui's panic hook.
        enable_raw_mode()
            .and_then(|()| execute!(io::stdout(), EnterAlternateScreen, EnableBracketedPaste))
            .and_then(|()| terminal.clear())
            .inspect_err(|_| self.quit = true)?;
        let refresh = self.store.refresh(false, Some(&changed));
        let workspace = self.workspace().to_owned();
        self.reload(&workspace);
        refresh?;
        ensure!(
            status.context(tr!("ui.text_013"))?.success(),
            "{}",
            tr!("ui.text_001")
        );
        Ok(())
    }
}
fn today_group(task: &Task, today: time::Date) -> (u8, &'static str) {
    if task.due.is_some_and(|d| d < today) {
        (0, tr!("ui.text_012"))
    } else if task.scheduled.is_some_and(|d| d < today) {
        (1, tr!("ui.text_011"))
    } else {
        (2, tr!("view.today"))
    }
}
fn enter_terminal() -> Result<DefaultTerminal> {
    let terminal = ratatui::try_init()?;
    execute!(io::stdout(), EnableBracketedPaste)?;
    Ok(terminal)
}

struct RestoreTerminal;
impl Drop for RestoreTerminal {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), DisableBracketedPaste);
        ratatui::restore();
    }
}

pub fn run(mut app: App) -> Result<()> {
    let _restore = RestoreTerminal;
    let mut terminal = enter_terminal()?;
    let mut last_refresh = Instant::now();
    let mut redraw = true;
    while !app.quit {
        if redraw {
            terminal.draw(|frame| app.draw(frame))?;
        }
        redraw = false;
        if event::poll(Duration::from_secs(2).saturating_sub(last_refresh.elapsed()))? {
            let result = app.event(event::read()?).and_then(|action| match action {
                Action::Edit => app.edit(&mut terminal),
                Action::None => Ok(()),
            });
            if let Err(error) = result {
                if app.quit {
                    return Err(error);
                }
                app.message = format!("{error:#}");
                let _ = app.refresh(false);
            }
            redraw = true;
        }
        if last_refresh.elapsed() >= Duration::from_secs(2) {
            match app.refresh(false) {
                Ok(changed) => redraw |= changed,
                Err(error) => {
                    app.message = error.to_string();
                    redraw = true;
                }
            }
            last_refresh = Instant::now();
        }
    }
    app.save_state()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tab_skips_empty_workspaces_in_both_directions() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let mut app = App::new(Store::open(dir.path())?, "nvim".into());
        for code in [
            KeyCode::Tab,
            KeyCode::BackTab,
            KeyCode::BackTab,
            KeyCode::Tab,
        ] {
            let before = app.focus;
            app.event(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)))?;
            assert!(app.focus != before && app.focus != Focus::Workspaces);
        }
        Ok(())
    }
    #[test]
    fn global_view_picker_counts_the_task_workspace() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let mut store = Store::open(dir.path())?;
        for name in ["alpha", "beta"] {
            store.create_workspace(name)?;
        }
        let day = dates::today();
        store.add_task("alpha", &format!("First [due:: {day}]"))?;
        store.add_task("beta", &format!("Second [due:: {day}]"))?;
        store.add_task("beta", &format!("Third [due:: {day}]"))?;
        let mut app = App::new(store, "nvim".into());
        app.set_view(View::All);
        app.tasks.select(Some(1));
        app.open_date(DateField::Due)?;
        assert_eq!(app.workspace(), "alpha");
        assert_eq!(app.date_edit.as_ref().unwrap().workspace, "beta");
        assert_eq!(app.calendar_counts()[&day], 2);
        Ok(())
    }
    #[test]
    fn search_cancel_clamps_selection_after_external_removal() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let mut store = Store::open(dir.path())?;
        store.create_workspace("work")?;
        fs::write(
            store.path("work"),
            "- [ ] First\n- [ ] Second\n- [ ] Third\n",
        )?;
        store.refresh(true, None)?;
        let mut app = App::new(store, "nvim".into());
        app.set_view(View::All);
        app.tasks.select(Some(2));
        app.open_prompt(Prompt::Search);
        fs::write(app.store.path("work"), "- [ ] First\n")?;
        app.refresh(false)?;
        app.event(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)))?;
        assert_eq!(app.tasks.selected(), Some(0));
        assert_eq!(app.selected_task().unwrap().title, "First");
        Ok(())
    }
    #[test]
    fn refresh_detects_a_new_local_day_without_source_changes() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let mut app = App::new(Store::open(dir.path())?, "nvim".into());
        app.day = dates::today() - time::Duration::days(1);
        assert!(app.refresh(false)?);
        assert_eq!(app.day, dates::today());
        assert_eq!(app.store.parsed, 0);
        assert!(!app.refresh(false)?);
        Ok(())
    }
    #[test]
    fn deleting_an_individual_file_refreshes_selection_without_reparsing() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let mut store = Store::open(dir.path())?;
        store.create_workspace("work")?;
        store.add_task("work", "First")?;
        store.add_task("work", "Second")?;
        let path = store.task_path("work", &store.files["work"].tasks[0])?;
        let mut app = App::new(store, "nvim".into());
        app.set_view(View::All);
        fs::remove_file(path)?;
        assert!(app.refresh(false)?);
        assert_eq!(app.store.parsed, 0);
        assert_eq!(app.visible.len(), 1);
        assert_eq!(app.selected_task().unwrap().title, "Second");
        Ok(())
    }
    #[test]
    fn status_prompt_keeps_its_original_target_after_external_deletion() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let mut store = Store::open(dir.path())?;
        store.create_workspace("work")?;
        store.add_task("work", "First")?;
        store.add_task("work", "Second")?;
        let path = store.task_path("work", &store.files["work"].tasks[0])?;
        let mut app = App::new(store, "nvim".into());
        app.set_view(View::All);
        app.event(Event::Key(KeyEvent::new(
            KeyCode::Char('v'),
            KeyModifiers::NONE,
        )))?;
        fs::remove_file(path)?;
        app.refresh(false)?;
        app.event(Event::Paste("completed".into()))?;
        assert!(
            app.event(Event::Key(KeyEvent::new(
                KeyCode::Enter,
                KeyModifiers::NONE
            )))
            .is_err()
        );
        assert!(!app.store.files["work"].tasks[0].done);
        Ok(())
    }
}
