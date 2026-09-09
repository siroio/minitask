use crate::tr;
use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    #[default]
    Action,
    Note,
    Direction,
    Question,
    Expectation,
}

impl Kind {
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "action" => Self::Action,
            "note" => Self::Note,
            "direction" => Self::Direction,
            "question" => Self::Question,
            "expectation" => Self::Expectation,
            _ => bail!("{}", tr!("memory.text_003", s)),
        })
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Action => "action",
            Self::Note => "note",
            Self::Direction => "direction",
            Self::Question => "question",
            Self::Expectation => "expectation",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Action => tr!("kind.action"),
            Self::Note => tr!("kind.note"),
            Self::Direction => tr!("kind.direction"),
            Self::Question => tr!("kind.question"),
            Self::Expectation => tr!("kind.expectation"),
        }
    }
    pub fn status_label(self, status: &str) -> &str {
        if self == Self::Expectation && status == "open" {
            tr!("state.expected")
        } else {
            crate::i18n::status_label(status)
        }
    }
    pub fn initial(self) -> &'static str {
        match self {
            Self::Action => "actionable",
            Self::Question | Self::Expectation => "open",
            _ => "active",
        }
    }
    pub fn closed(self) -> &'static str {
        match self {
            Self::Action => "completed",
            Self::Question => "answered",
            Self::Expectation => "achieved",
            _ => "archived",
        }
    }
    pub fn statuses(self) -> &'static [&'static str] {
        match self {
            Self::Action => &[
                "backlog",
                "actionable",
                "in_progress",
                "blocked",
                "completed",
                "cancelled",
            ],
            Self::Question => &["open", "answered", "closed"],
            Self::Expectation => &["open", "achieved", "dropped"],
            _ => &["active", "archived"],
        }
    }
    pub fn transition(self, from: &str, to: &str) -> Result<()> {
        ensure!(
            self.statuses().contains(&from) && self.statuses().contains(&to),
            "{}",
            tr!(
                "memory.text_002",
                self.label(),
                self.status_label(from),
                self.status_label(to)
            )
        );
        let allowed = match self {
            Self::Action => match from {
                "backlog" => matches!(to, "actionable" | "cancelled"),
                "actionable" => matches!(
                    to,
                    "backlog" | "in_progress" | "blocked" | "completed" | "cancelled"
                ),
                "in_progress" => matches!(to, "actionable" | "blocked" | "completed" | "cancelled"),
                "blocked" => matches!(to, "actionable" | "in_progress" | "cancelled"),
                _ => to == "actionable",
            },
            _ => from == self.initial() || to == self.initial(),
        };
        ensure!(
            from != to && allowed,
            "{}",
            tr!(
                "memory.text_001",
                self.status_label(from),
                self.status_label(to)
            )
        );
        Ok(())
    }
}

pub fn is_closed(status: &str) -> bool {
    matches!(
        status,
        "completed" | "cancelled" | "answered" | "closed" | "achieved" | "dropped" | "archived"
    )
}

pub fn section(text: &str, heading: &str) -> String {
    let mut found = false;
    let mut result = Vec::new();
    let (mut fence, mut length) = (0u8, 0usize);
    let mut comment = false;
    for raw in text.lines() {
        let visible = if fence == 0 {
            crate::dates::uncomment(raw, &mut comment)
        } else {
            raw.to_owned()
        };
        let line = visible.trim_start_matches(' ');
        let bytes = line.as_bytes();
        if visible.len() - line.len() <= 3 && bytes.len() >= 3 && matches!(bytes[0], b'`' | b'~') {
            let count = bytes.iter().take_while(|&&b| b == bytes[0]).count();
            if fence == 0 && count >= 3 {
                fence = bytes[0];
                length = count;
            } else if fence == bytes[0] && count >= length && line[count..].trim().is_empty() {
                fence = 0;
            }
            if found {
                result.push(raw);
            }
            continue;
        }
        if fence == 0 && visible.len() - line.len() <= 3 && line.starts_with("## ") {
            if found {
                break;
            }
            found = line.trim() == format!("## {heading}");
        } else if found {
            result.push(raw);
        }
    }
    result.join("\n").trim().into()
}

pub fn in_resume(task: &crate::store::Task) -> bool {
    match task.kind {
        Kind::Action => matches!(task.status.as_str(), "actionable" | "in_progress"),
        Kind::Direction | Kind::Note => task.status == "active",
        Kind::Question | Kind::Expectation => task.status == "open",
    }
}
