use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders},
};

pub const BG: Color = Color::Rgb(22, 25, 35);
pub const PANEL: Color = Color::Rgb(29, 34, 48);
pub const SURFACE: Color = Color::Rgb(39, 46, 65);
pub const SELECTION: Color = Color::Rgb(49, 65, 91);
pub const TEXT: Color = Color::Rgb(222, 229, 243);
pub const MUTED: Color = Color::Rgb(166, 177, 199);
pub const BORDER: Color = Color::Rgb(77, 89, 115);
pub const ACCENT: Color = Color::Rgb(139, 190, 247);
pub const GREEN: Color = Color::Rgb(164, 211, 168);
pub const YELLOW: Color = Color::Rgb(237, 199, 135);
pub const RED: Color = Color::Rgb(244, 143, 157);
pub const PURPLE: Color = Color::Rgb(191, 172, 239);

pub fn base() -> Style {
    Style::default().fg(TEXT).bg(BG)
}
pub fn panel() -> Style {
    base().bg(PANEL)
}
pub fn accent() -> Style {
    Style::default().fg(ACCENT)
}
pub fn muted() -> Style {
    Style::default().fg(MUTED)
}
pub fn selected() -> Style {
    Style::default().bg(SELECTION).add_modifier(Modifier::BOLD)
}
pub fn block(title: impl Into<String>, focus: bool) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(panel())
        .title(Line::from(vec![Span::styled(
            format!(" {} ", title.into()),
            if focus {
                accent().add_modifier(Modifier::BOLD)
            } else {
                muted()
            },
        )]))
        .border_style(Style::default().fg(if focus { ACCENT } else { BORDER }))
}

pub fn key_hints(keys: &[(&str, &str)]) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    for (key, label) in keys {
        spans.push(Span::styled(
            format!(" {key} "),
            accent().bg(SURFACE).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(format!(" {label}  "), muted()));
    }
    Line::from(spans)
}
