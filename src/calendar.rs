use crate::tr;
use std::collections::BTreeMap;

use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use time::{Date, Duration, Month};

use crate::dates;
use crate::theme::*;

#[derive(Clone)]
pub struct Calendar {
    pub selected: Date,
    pub all_workspaces: bool,
    pub focus_days: bool,
}

impl Calendar {
    pub fn new(date: Date) -> Self {
        Self {
            selected: date,
            all_workspaces: false,
            focus_days: true,
        }
    }

    pub fn navigate(&mut self, key: KeyCode) -> bool {
        let delta = match key {
            KeyCode::Left | KeyCode::Char('h') => -1,
            KeyCode::Right | KeyCode::Char('l') => 1,
            KeyCode::Up | KeyCode::Char('k') => -7,
            KeyCode::Down | KeyCode::Char('j') => 7,
            KeyCode::PageUp | KeyCode::PageDown => {
                let delta = if key == KeyCode::PageUp { -1 } else { 1 };
                let month = self.selected.year() * 12 + self.selected.month() as i32 - 1 + delta;
                let year = month.div_euclid(12);
                if (1..=9999).contains(&year) {
                    let month = Month::try_from((month.rem_euclid(12) + 1) as u8).unwrap();
                    self.selected = Date::from_calendar_date(
                        year,
                        month,
                        self.selected.day().min(month.length(year)),
                    )
                    .unwrap();
                }
                return true;
            }
            KeyCode::Char('t') => {
                self.selected = dates::today();
                return true;
            }
            _ => return false,
        };
        if let Some(date) = self.selected.checked_add(Duration::days(delta))
            && (1..=9999).contains(&date.year())
        {
            self.selected = date;
        }
        true
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, counts: &BTreeMap<Date, usize>) {
        let today = dates::today();
        let block = block(
            tr!(
                "calendar.text_002",
                self.selected.year(),
                format!("{:02}", self.selected.month() as u8)
            ),
            self.focus_days,
        );
        let inner = block.inner(area);
        let first = self.selected.replace_day(1).unwrap();
        let offset = first.weekday().number_days_from_monday() as usize;
        let days = self.selected.month().length(self.selected.year()) as usize;
        let selected_week = (offset + self.selected.day() as usize - 1) / 7;
        let weeks = ((inner.height.saturating_sub(3) / 2) as usize).clamp(1, 6);
        let start = selected_week.saturating_sub(weeks / 2).min(6 - weeks);
        let mut lines = vec![Line::styled(tr!("calendar.text_004"), muted().bg(SURFACE))];
        for week in start..start + weeks {
            let mut cells = Vec::new();
            let mut totals = Vec::new();
            for weekday in 0..7 {
                let position = week * 7 + weekday;
                if position < offset || position >= offset + days {
                    cells.push(Span::raw("     "));
                    totals.push(Span::raw("     "));
                    continue;
                }
                let day = (position - offset + 1) as u8;
                let date = first.replace_day(day).unwrap();
                let count = counts.get(&date).copied().unwrap_or(0);
                let mut style = Style::default().fg(if weekday >= 5 { PURPLE } else { TEXT });
                if date == today {
                    style = style
                        .fg(GREEN)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
                }
                if date == self.selected {
                    style = style.patch(selected()).fg(TEXT);
                }
                cells.push(Span::styled(
                    format!(
                        "{}{day:>2}  ",
                        if date == self.selected { ">" } else { " " }
                    ),
                    style,
                ));
                let text = if count == 0 {
                    "     ".into()
                } else if count > 99 {
                    " 99+ ".into()
                } else {
                    format!(" {count:>2}  ")
                };
                totals.push(Span::styled(
                    text,
                    accent().bg(if count > 0 { SURFACE } else { PANEL }),
                ));
            }
            lines.push(Line::from(cells));
            lines.push(Line::from(totals));
        }
        lines.push(Line::styled(tr!("calendar.text_003"), muted()));
        lines.push(Line::styled(tr!("calendar.text_001", today), muted()));
        frame.render_widget(Paragraph::new(lines).block(block), area);
    }
}
