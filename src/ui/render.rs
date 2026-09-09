use super::*;
use crate::theme::*;
use crate::tr;
use ratatui::{
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, HighlightSpacing, LineGauge, List, ListItem, Paragraph, Wrap},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

fn fit(s: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let clipped = s.width() > width;
    let limit = width - usize::from(clipped);
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars().filter(|c| !c.is_control()) {
        let w = c.width().unwrap_or(0);
        if used + w > limit {
            break;
        }
        out.push(c);
        used += w;
    }
    if clipped {
        out.push('…');
        used += 1;
    }
    out.push_str(&" ".repeat(width.saturating_sub(used)));
    out
}
fn input(frame: &mut Frame, area: Rect, value: &Input, active: bool) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    frame.render_widget(Clear, area);
    let mut start = 0;
    while start < value.cursor
        && value.chars[start..value.cursor]
            .iter()
            .collect::<String>()
            .width()
            >= area.width as usize
    {
        start += 1;
    }
    let text: String = value.chars[start..].iter().collect();
    frame.render_widget(
        Paragraph::new(text).style(if active {
            base().bg(SELECTION)
        } else {
            panel()
        }),
        area,
    );
    if active {
        let before: String = value.chars[start..value.cursor].iter().collect();
        frame.set_cursor_position((
            area.x + before.width().min(area.width.saturating_sub(1) as usize) as u16,
            area.y,
        ));
    }
}
fn popup(body: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(body.width);
    let h = height.min(body.height);
    Rect::new(
        body.x + (body.width - w) / 2,
        body.y + (body.height - h) / 2,
        w,
        h,
    )
}
fn date_text(date: Option<time::Date>, wide: bool) -> String {
    date.map(|d| {
        if wide {
            d.to_string()
        } else {
            format!("{:02}-{:02}", d.month() as u8, d.day())
        }
    })
    .unwrap_or_else(|| "-".into())
}

impl App {
    pub fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();
        frame.render_widget(Block::default().style(base()), area);
        if area.width < 40 || area.height < 12 {
            frame.render_widget(Paragraph::new(tr!("ui.text_117")), area);
            return;
        }
        let [header, body, status, footer] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(8),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(area);
        let scope = if let Some(edit) = &self.date_edit {
            edit.workspace.as_str()
        } else if self.calendar.as_ref().is_some_and(|c| c.all_workspaces)
            || (self.calendar.is_none() && self.view != View::Workspace)
        {
            tr!("ui.text_116")
        } else {
            self.workspace()
        };
        let label = if self.calendar.is_some() {
            tr!("ui.text_033")
        } else {
            self.view.label()
        };
        let [brand, breadcrumb] =
            Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(header);
        let [brand_left, brand_right] =
            Layout::horizontal([Constraint::Min(20), Constraint::Length(16)]).areas(brand);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    " minitask ",
                    base().fg(BG).bg(ACCENT).add_modifier(Modifier::BOLD),
                ),
                Span::styled(tr!("ui.text_115"), muted()),
            ]))
            .style(panel()),
            brand_left,
        );
        frame.render_widget(
            Paragraph::new(format!("{}  ", dates::today()))
                .alignment(Alignment::Right)
                .style(panel().fg(MUTED)),
            brand_right,
        );
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!("  {label} / {scope}"), accent()),
                Span::styled(tr!("ui.text_056", self.visible.len()), muted()),
            ])),
            breadcrumb,
        );
        if let Some(calendar) = &self.calendar {
            if area.width >= 70 && body.height >= 18 {
                let [left, right] =
                    Layout::horizontal([Constraint::Length(38), Constraint::Min(30)]).areas(body);
                calendar.draw(frame, left, &self.calendar_counts());
                if self.date_edit.is_some() {
                    self.draw_date(frame, right);
                } else {
                    self.draw_tasks(frame, right);
                }
            } else if self.date_edit.is_some() {
                self.draw_date(frame, body);
            } else if calendar.focus_days {
                calendar.draw(frame, body, &self.calendar_counts());
            } else {
                self.draw_tasks(frame, body);
            }
        } else if area.width < 72 {
            if self.focus != Focus::Tasks {
                self.draw_sidebar(frame, body);
            } else {
                self.draw_tasks(frame, body);
            }
        } else {
            let [left, right] =
                Layout::horizontal([Constraint::Length(26), Constraint::Min(30)]).areas(body);
            self.draw_sidebar(frame, left);
            if self.detail && area.width >= 120 {
                let [tasks, details] =
                    Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)])
                        .areas(right);
                self.draw_tasks(frame, tasks);
                self.draw_detail(frame, details);
            } else {
                self.draw_tasks(frame, right);
            }
        }
        if self.detail && (area.width < 120 || self.calendar.is_some()) && self.date_edit.is_none()
        {
            self.draw_detail(frame, body);
        }
        let message = if !self.message.is_empty() {
            self.message.clone()
        } else if !self.store.warning.is_empty() {
            self.store.warning.clone()
        } else if self.names.is_empty() {
            tr!("ui.text_114").into()
        } else if !self.query.is_empty() {
            tr!("ui.text_055", self.query, self.visible.len())
        } else {
            format!(" {}", self.store.root.display())
        };
        let badge = if !self.message.is_empty() || !self.store.warning.is_empty() {
            tr!("ui.text_113")
        } else if !self.query.is_empty() {
            tr!("ui.text_112")
        } else {
            tr!("ui.text_111")
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(badge, accent().bg(SURFACE)),
                Span::styled(format!(" {message}"), muted()),
            ])),
            status,
        );
        let keys: &[(&str, &str)] = if self.help {
            &[("PgUp/Dn", tr!("ui.text_099")), ("Esc", tr!("ui.text_101"))]
        } else if self.prompt == Some(Prompt::Palette) {
            &[
                ("Up/Dn", tr!("ui.text_110")),
                ("Enter", tr!("ui.text_109")),
                ("Esc", tr!("ui.text_105")),
            ]
        } else if matches!(self.prompt, Some(Prompt::Workspace | Prompt::Status)) {
            &[("Enter", tr!("ui.text_106")), ("Esc", tr!("ui.text_108"))]
        } else if self.add.is_some() {
            &[
                ("Tab", tr!("ui.text_076")),
                ("Enter", tr!("ui.text_106")),
                ("Esc", tr!("ui.text_108")),
            ]
        } else if self.date_edit.is_some() {
            &[
                ("i", tr!("ui.text_107")),
                ("Enter", tr!("ui.text_106")),
                ("Esc", tr!("ui.text_105")),
                ("?", tr!("ui.text_026")),
            ]
        } else if self.calendar.is_some() {
            if area.width < 60 {
                &[
                    ("Tab", tr!("ui.text_104")),
                    ("a", tr!("ui.text_098")),
                    ("?", tr!("ui.text_026")),
                ]
            } else {
                &[
                    ("Tab", tr!("ui.text_103")),
                    ("a", tr!("ui.text_098")),
                    ("w", tr!("ui.text_102")),
                    ("?", tr!("ui.text_026")),
                ]
            }
        } else if self.detail {
            &[
                ("Esc", tr!("ui.text_101")),
                ("e", tr!("ui.text_100")),
                ("PgUp/Dn", tr!("ui.text_099")),
            ]
        } else if area.width < 60 {
            &[
                ("Tab", tr!("ui.focus.next")),
                ("a", tr!("ui.text_098")),
                (":", tr!("ui.text_096")),
                ("?", tr!("ui.text_026")),
            ]
        } else {
            &[
                ("Tab", tr!("ui.focus.next")),
                ("a", tr!("ui.text_098")),
                ("Space", tr!("view.done")),
                ("/", tr!("ui.text_097")),
                (":", tr!("ui.text_096")),
                ("?", tr!("ui.text_026")),
            ]
        };
        frame.render_widget(Paragraph::new(key_hints(keys)), footer);
        if self.add.is_some() || self.help || self.prompt.is_some_and(|p| p != Prompt::Search) {
            frame
                .buffer_mut()
                .set_style(body, Style::default().fg(BORDER).bg(BG));
        }
        if let Some(prompt) = self.prompt {
            if prompt == Prompt::Search {
                let [prefix, field] =
                    Layout::horizontal([Constraint::Length(6), Constraint::Min(1)]).areas(footer);
                frame.render_widget(
                    Paragraph::new(tr!("ui.text_095")).style(accent().bg(SURFACE)),
                    prefix,
                );
                input(frame, field, &self.input, true);
            } else {
                let rect = popup(
                    body,
                    64,
                    if matches!(prompt, Prompt::Workspace | Prompt::Status) {
                        8
                    } else {
                        16
                    },
                );
                let b = block(
                    if prompt == Prompt::Workspace {
                        tr!("ui.text_094")
                    } else if prompt == Prompt::Status {
                        tr!("ui.text_093")
                    } else {
                        tr!("ui.text_092")
                    },
                    true,
                );
                let inner = b.inner(rect);
                frame.render_widget(Clear, rect);
                frame.render_widget(b, rect);
                let [field, results] =
                    Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).areas(inner);
                input(frame, field, &self.input, true);
                if prompt == Prompt::Status
                    && let Some((_, task)) = self.status_edit.as_ref()
                {
                    let choices = task
                        .kind
                        .statuses()
                        .iter()
                        .filter(|s| task.kind.transition(&task.status, s).is_ok())
                        .map(|s| task.kind.status_label(s))
                        .collect::<Vec<_>>()
                        .join(" / ");
                    frame.render_widget(
                        Paragraph::new(tr!(
                            "ui.text_054",
                            task.kind.status_label(&task.status),
                            choices
                        ))
                        .wrap(Wrap { trim: false }),
                        results,
                    );
                }
                if prompt == Prompt::Palette {
                    let commands = self.commands();
                    if commands.is_empty() {
                        frame.render_widget(Paragraph::new(tr!("ui.text_091")), results);
                    } else {
                        let items = commands.iter().map(|(s, k)| {
                            ListItem::new(Line::from(vec![
                                Span::styled(format!(" {k} "), accent().bg(SURFACE)),
                                Span::raw(format!("  {s}")),
                            ]))
                        });
                        let mut state =
                            ListState::default().with_selected(Some(self.palette_index));
                        frame.render_stateful_widget(
                            List::new(items)
                                .highlight_symbol("> ")
                                .highlight_style(selected()),
                            results,
                            &mut state,
                        );
                    }
                }
            }
        }
        if let Some(add) = &self.add {
            let rect = popup(body, 68, 12);
            let b = block(
                tr!(
                    "ui.text_053",
                    if add.kind == Kind::Action {
                        tr!("ui.text_075")
                    } else {
                        add.kind.label()
                    }
                ),
                true,
            );
            let inner = b.inner(rect);
            frame.render_widget(Clear, rect);
            frame.render_widget(b, rect);
            for (i, label) in [
                tr!("ui.text_090"),
                tr!("ui.form.workspace"),
                if add.kind == Kind::Expectation {
                    tr!("ui.text_074")
                } else {
                    tr!("ui.text_089")
                },
                tr!("dates.text_009"),
            ]
            .iter()
            .take(add.field_count())
            .enumerate()
            {
                let y = inner.y + i as u16 * if inner.height >= 7 { 2 } else { 1 };
                if y >= inner.bottom() {
                    break;
                }
                let [label_area, field] =
                    Layout::horizontal([Constraint::Length(12), Constraint::Min(1)])
                        .areas(Rect::new(inner.x, y, inner.width, 1));
                frame.render_widget(
                    Paragraph::new(format!(
                        "{} {label}",
                        if add.focus == i { ">" } else { " " }
                    ))
                    .style(if add.focus == i { accent() } else { muted() }),
                    label_area,
                );
                if i == 1 {
                    frame.render_widget(
                        Paragraph::new(format!("< {} >", add.workspace)).style(if add.focus == i {
                            selected()
                        } else {
                            panel().bg(SURFACE)
                        }),
                        field,
                    );
                } else {
                    let value = &add.fields[if i == 0 { 0 } else { i - 1 }];
                    if value.chars.is_empty() && add.focus != i {
                        frame.render_widget(
                            Paragraph::new(if i == 0 {
                                tr!("ui.text_088")
                            } else {
                                tr!("ui.text_087")
                            })
                            .style(muted().bg(SURFACE)),
                            field,
                        );
                    } else {
                        input(frame, field, value, add.focus == i);
                    }
                }
            }
        }
        if self.help {
            let rect = popup(body, 76, 20);
            frame.render_widget(Clear, rect);
            let context = if self.date_edit.is_some() {
                tr!("ui.text_086")
            } else if self.calendar.is_some() {
                tr!("ui.text_085")
            } else {
                tr!("ui.text_084")
            };
            let help = tr!("ui.text_052", context);
            let b = block(tr!("ui.text_083"), true);
            let inner = b.inner(rect);
            let p = Paragraph::new(help).wrap(Wrap { trim: false });
            self.help_offset = self
                .help_offset
                .min(
                    p.line_count(inner.width)
                        .saturating_sub(inner.height as usize),
                )
                .min(u16::MAX as usize);
            frame.render_widget(p.scroll((self.help_offset as u16, 0)).block(b), rect);
        }
    }
    fn draw_sidebar(&mut self, frame: &mut Frame, area: Rect) {
        let position = if self.focus == Focus::Workspaces
            || (self.focus == Focus::Tasks && self.view == View::Workspace)
        {
            VIEWS.len() + self.workspaces.selected().unwrap_or(0)
        } else {
            self.sidebar.selected().unwrap_or(0)
        };
        let (views, projects) = if area.height >= VIEWS.len() as u16 + 8 {
            let [a, b] = Layout::vertical([
                Constraint::Length(VIEWS.len() as u16 + 4),
                Constraint::Min(5),
            ])
            .areas(area);
            (a, Some(b))
        } else {
            (area, None)
        };
        let row_width = views.width.saturating_sub(4) as usize;
        let mut view_items = VIEWS
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let color = if *v == View::Done {
                    GREEN
                } else if *v == View::Today {
                    ACCENT
                } else {
                    TEXT
                };
                ListItem::new(Line::from(vec![
                    Span::styled(format!("{} ", v.key()), muted()),
                    Span::styled(
                        fit(v.label(), row_width.saturating_sub(7)),
                        Style::default().fg(color),
                    ),
                    Span::styled(format!(" {:>4}", self.counts[i]), muted().bg(SURFACE)),
                ]))
            })
            .collect::<Vec<_>>();
        view_items.insert(6, ListItem::new(Line::styled(tr!("nav.records"), muted())));
        let row_for = |position: usize| position + usize::from(position >= 6);
        if let Some(projects) = projects {
            let view_state = if position < VIEWS.len() {
                Some(position)
            } else {
                VIEWS.iter().position(|v| *v == self.view)
            };
            frame.render_stateful_widget(
                List::new(view_items)
                    .highlight_spacing(HighlightSpacing::Always)
                    .block(block(tr!("nav.work"), self.focus == Focus::Views))
                    .highlight_symbol("> ")
                    .highlight_style(if self.focus == Focus::Views {
                        selected()
                    } else {
                        Style::default().bg(SURFACE)
                    }),
                views,
                &mut ListState::default().with_selected(view_state.map(row_for)),
            );
            let counts: Vec<_> = self
                .names
                .iter()
                .map(|n| {
                    let tasks = &self.store.files[n].tasks;
                    (
                        n,
                        tasks
                            .iter()
                            .filter(|t| t.kind == Kind::Action && t.done)
                            .count(),
                        tasks.iter().filter(|t| t.kind == Kind::Action).count(),
                    )
                })
                .collect();
            let total: usize = counts.iter().map(|(_, _, total)| total).sum();
            let done: usize = counts.iter().map(|(_, done, _)| done).sum();
            let [project_list, progress] = if projects.height >= 8 {
                Layout::vertical([Constraint::Min(5), Constraint::Length(3)]).areas(projects)
            } else {
                [projects, Rect::default()]
            };
            let items = counts.iter().map(|(n, done, total)| {
                ListItem::new(Line::from(vec![
                    Span::styled(
                        fit(n, row_width.saturating_sub(5)),
                        Style::default().fg(PURPLE),
                    ),
                    Span::styled(format!(" {:>4}", total - done), muted().bg(SURFACE)),
                ]))
            });
            let choice = if position >= VIEWS.len() {
                Some(position - VIEWS.len())
            } else {
                self.workspaces.selected()
            };
            frame.render_stateful_widget(
                List::new(items)
                    .block(block(
                        tr!("view.workspace"),
                        self.focus == Focus::Workspaces,
                    ))
                    .highlight_symbol(if position >= VIEWS.len() { "> " } else { "  " })
                    .highlight_style(if self.focus == Focus::Workspaces {
                        selected()
                    } else {
                        Style::default().bg(SURFACE)
                    }),
                project_list,
                &mut ListState::default().with_selected(choice),
            );
            if progress.height > 0 {
                let b = block(tr!("ui.text_081"), false);
                let inner = b.inner(progress);
                frame.render_widget(b, progress);
                frame.render_widget(
                    LineGauge::default()
                        .ratio(if total == 0 {
                            0.0
                        } else {
                            done as f64 / total as f64
                        })
                        .label(format!("{done}/{total}"))
                        .filled_style(Style::default().fg(GREEN))
                        .unfilled_style(Style::default().fg(BORDER)),
                    inner,
                );
            }
        } else {
            let mut items = view_items;
            items.extend(self.names.iter().map(|n| {
                ListItem::new(Line::from(vec![Span::styled(
                    n.clone(),
                    Style::default().fg(PURPLE),
                )]))
            }));
            frame.render_stateful_widget(
                List::new(items)
                    .block(block(tr!("ui.text_080"), self.focus != Focus::Tasks))
                    .highlight_symbol("> ")
                    .highlight_style(selected()),
                views,
                &mut ListState::default().with_selected(Some(row_for(position))),
            );
        }
    }
    fn draw_tasks(&mut self, frame: &mut Frame, area: Rect) {
        let focus = self
            .calendar
            .as_ref()
            .map(|c| !c.focus_days)
            .unwrap_or(self.focus == Focus::Tasks);
        let title = self
            .calendar
            .as_ref()
            .map(|c| tr!("ui.text_051", c.selected, self.visible.len()))
            .unwrap_or_else(|| tr!("ui.text_051", self.view.label(), self.visible.len()));
        let b = block(title, focus).title_bottom(
            Line::from(format!(
                " {} / {} ",
                self.tasks.selected().map(|i| i + 1).unwrap_or(0),
                self.visible.len()
            ))
            .alignment(Alignment::Right)
            .style(muted()),
        );
        let inner = b.inner(area);
        frame.render_widget(b, area);
        if self.visible.is_empty() {
            let msg = if self.names.is_empty() {
                tr!("ui.text_005")
            } else if !self.query.is_empty() {
                tr!("ui.text_079")
            } else if self.view == View::Today && self.calendar.is_none() {
                tr!("ui.text_078")
            } else {
                tr!("ui.text_077")
            };
            frame.render_widget(
                Paragraph::new(msg)
                    .wrap(Wrap { trim: false })
                    .style(muted()),
                inner.inner(Margin {
                    horizontal: 2.min(inner.width / 2),
                    vertical: 1.min(inner.height / 2),
                }),
            );
            return;
        }
        let [header, rows] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(inner);
        let w = inner.width.saturating_sub(2) as usize;
        let show_ws = w >= 48;
        let show_dates = w >= 44;
        let wide_dates = w >= 82;
        let date_width = if wide_dates { 11 } else { 6 };
        let ws_width = if show_ws { 10 } else { 0 };
        let title_width = w.saturating_sub(ws_width + if show_dates { date_width * 2 } else { 0 });
        let headings = format!(
            "  {}{}{}",
            fit(
                if self.view == View::Horizon {
                    tr!("kind.expectation")
                } else if matches!(
                    self.view,
                    View::Resume
                        | View::Notes
                        | View::Directions
                        | View::Questions
                        | View::Workspace
                ) {
                    tr!("ui.text_076")
                } else {
                    tr!("ui.text_075")
                },
                title_width
            ),
            if show_ws {
                fit(tr!("ui.column.workspace"), ws_width)
            } else {
                String::new()
            },
            if show_dates {
                format!(
                    "{}{}",
                    fit(
                        if self.view == View::Horizon {
                            tr!("ui.text_074")
                        } else {
                            tr!("dates.text_010")
                        },
                        date_width
                    ),
                    fit(tr!("dates.text_009"), date_width)
                )
            } else {
                String::new()
            }
        );
        frame.render_widget(Paragraph::new(headings).style(muted().bg(SURFACE)), header);
        let today = dates::today();
        let current = self.tasks.selected().unwrap_or(0);
        let start = self
            .tasks
            .offset()
            .min(current)
            .max(current.saturating_sub(rows.height.saturating_sub(1) as usize));
        *self.tasks.offset_mut() = start;
        let items: Vec<_> = self
            .visible
            .iter()
            .skip(start)
            .take(rows.height as usize)
            .enumerate()
            .map(|(row, (n, i))| {
                let t = &self.store.files[n].tasks[*i];
                let tag = if !t.date_error.is_empty() {
                    tr!("ui.text_073")
                } else if self.view == View::Today && self.calendar.is_none() {
                    today_group(t, today).1
                } else if !t.done && t.due.is_some_and(|d| d < today) {
                    tr!("ui.text_072")
                } else {
                    ""
                };
                let kind_tag = if t.kind != Kind::Action || self.view == View::Resume {
                    match t.kind {
                        Kind::Action if t.status == "in_progress" => tr!("ui.text_071"),
                        Kind::Action => tr!("ui.text_070"),
                        Kind::Direction => tr!("ui.text_069"),
                        Kind::Question => tr!("ui.text_068"),
                        Kind::Expectation => tr!("ui.text_067"),
                        Kind::Note => tr!("ui.text_066"),
                    }
                    .into()
                } else if t.status == "in_progress" {
                    tr!("ui.text_065").into()
                } else {
                    String::new()
                };
                let title = format!(
                    "{}{}{}{}",
                    kind_tag,
                    tag,
                    if tag.is_empty() || tag.ends_with(' ') {
                        ""
                    } else {
                        " "
                    },
                    t.title
                );
                let status_color = if t.done {
                    GREEN
                } else if !t.date_error.is_empty() || t.due.is_some_and(|d| d < today) {
                    RED
                } else {
                    MUTED
                };
                let mut spans = vec![
                    Span::styled(
                        if t.kind == Kind::Note {
                            "    "
                        } else if t.done {
                            "[x] "
                        } else {
                            "[ ] "
                        },
                        Style::default().fg(status_color),
                    ),
                    Span::styled(
                        format!("{} ", fit(&title, title_width.saturating_sub(5))),
                        Style::default().fg(if t.done { MUTED } else { TEXT }),
                    ),
                ];
                if show_ws {
                    spans.push(Span::styled(fit(n, ws_width), Style::default().fg(PURPLE)));
                }
                if show_dates {
                    spans.push(Span::styled(
                        fit(
                            &date_text(t.expectation.or(t.scheduled), wide_dates),
                            date_width,
                        ),
                        if t.scheduled.is_some() || t.expectation.is_some() {
                            accent()
                        } else {
                            muted()
                        },
                    ));
                    spans.push(Span::styled(
                        fit(&date_text(t.due, wide_dates), date_width),
                        if !t.done && t.due.is_some_and(|d| d < today) {
                            Style::default().fg(RED)
                        } else if t.due == Some(today) && !t.done {
                            Style::default().fg(YELLOW)
                        } else {
                            Style::default()
                        },
                    ));
                }
                ListItem::new(Line::from(spans)).style(panel().bg(if row % 2 == 0 {
                    PANEL
                } else {
                    BG
                }))
            })
            .collect();
        frame.render_stateful_widget(
            List::new(items)
                .highlight_symbol("> ")
                .highlight_style(if focus {
                    selected()
                } else {
                    Style::default().bg(SURFACE)
                }),
            rows,
            &mut ListState::default().with_selected(Some(current - start)),
        );
    }
    fn draw_detail(&mut self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Clear, area);
        let text = self
            .selected_task()
            .map(|t| {
                let mut lines = vec![
                    Line::styled(
                        t.title.clone(),
                        Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
                    ),
                    Line::styled(
                        format!("{} / {}:{}", self.task_workspace(), t.source, t.line),
                        Style::default().fg(PURPLE),
                    ),
                    Line::default(),
                    Line::styled(
                        format!(" {} / {} ", t.kind.label(), t.kind.status_label(&t.status)),
                        Style::default()
                            .fg(if t.done { GREEN } else { ACCENT })
                            .bg(SURFACE),
                    ),
                    Line::default(),
                    Line::from(vec![
                        Span::styled(
                            if t.kind == Kind::Expectation {
                                tr!("ui.text_064")
                            } else {
                                tr!("ui.text_063")
                            },
                            muted(),
                        ),
                        Span::styled(date_text(t.expectation.or(t.scheduled), true), accent()),
                    ]),
                    Line::from(vec![
                        Span::styled(tr!("ui.text_062"), muted()),
                        Span::styled(
                            date_text(t.due, true),
                            Style::default().fg(
                                if t.due.is_some_and(|d| d < dates::today()) && !t.done {
                                    RED
                                } else {
                                    YELLOW
                                },
                            ),
                        ),
                    ]),
                ];
                if !t.date_error.is_empty() {
                    lines.push(Line::styled(t.date_error.clone(), Style::default().fg(RED)));
                }
                lines.push(Line::default());
                lines.push(Line::styled(tr!("ui.text_061"), muted()));
                lines.extend(t.notes.replace('\t', "    ").lines().map(|s| {
                    Line::raw(if t.kind == Kind::Note {
                        s.to_owned()
                    } else {
                        match s {
                            "## Completion Condition" => tr!("markdown.condition").into(),
                            "## Supplement" => tr!("markdown.supplement").into(),
                            "## Answer" => tr!("markdown.answer").into(),
                            "## Verification" => tr!("markdown.verification").into(),
                            _ => s.to_owned(),
                        }
                    })
                }));
                if t.notes.trim().is_empty() {
                    lines.push(Line::styled(tr!("ui.text_060"), muted()));
                }
                lines
            })
            .unwrap_or_else(|| vec![Line::styled(tr!("ui.text_024"), muted())]);
        let b = block(tr!("ui.text_059"), true);
        let inner = b.inner(area);
        let p = Paragraph::new(text).wrap(Wrap { trim: false });
        self.detail_offset = self
            .detail_offset
            .min(
                p.line_count(inner.width)
                    .saturating_sub(inner.height as usize),
            )
            .min(u16::MAX as usize);
        frame.render_widget(p.scroll((self.detail_offset as u16, 0)).block(b), area);
    }
    fn draw_date(&self, frame: &mut Frame, area: Rect) {
        let edit = self.date_edit.as_ref().unwrap();
        let date = self.calendar.as_ref().unwrap().selected;
        let b = block(tr!("ui.text_050", edit.field.label()), true);
        let inner = b.inner(area);
        frame.render_widget(b, area);
        let [info, field] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
        let edit_hint = if edit.field == DateField::Expectation {
            tr!("ui.text_058")
        } else {
            tr!("ui.text_057")
        };
        frame.render_widget(
            Paragraph::new(tr!(
                "ui.text_049",
                edit.workspace,
                edit.task.title,
                date,
                edit_hint
            ))
            .wrap(Wrap { trim: false }),
            info,
        );
        if let Some(value) = &self.date_input {
            input(frame, field, value, true);
        }
    }
}
