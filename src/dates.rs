use crate::tr;
use std::ops::Range;

use anyhow::{Result, bail, ensure};
use time::{Date, Month, OffsetDateTime};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateField {
    Scheduled,
    Due,
    Expectation,
}

impl DateField {
    pub fn key(self) -> &'static str {
        match self {
            Self::Scheduled => "scheduled",
            Self::Due => "due",
            Self::Expectation => "expectation",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Scheduled => tr!("dates.text_010"),
            Self::Due => tr!("dates.text_009"),
            Self::Expectation => tr!("dates.text_008"),
        }
    }
}

pub fn today() -> Date {
    OffsetDateTime::now_local()
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .date()
}

pub fn parse_date(value: &str) -> Result<Date> {
    let bytes = value.as_bytes();
    ensure!(
        bytes.len() == 10
            && bytes[4] == b'-'
            && bytes[7] == b'-'
            && bytes
                .iter()
                .enumerate()
                .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit()),
        "{}",
        tr!("dates.text_006")
    );
    let year = value[..4].parse()?;
    ensure!((1..=9999).contains(&year), "{}", tr!("dates.text_001"));
    Date::from_calendar_date(
        year,
        Month::try_from(value[5..7].parse::<u8>()?)
            .map_err(|_| anyhow::anyhow!(tr!("dates.invalid_calendar")))?,
        value[8..].parse()?,
    )
    .map_err(|_| anyhow::anyhow!(tr!("dates.invalid_calendar")))
}

pub(crate) struct Metadata {
    pub title: String,
    pub scheduled: Option<Date>,
    pub due: Option<Date>,
    pub expectation: Option<Date>,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub error: String,
    properties: Vec<(String, Range<usize>)>,
    spans: Vec<(DateField, Range<usize>)>,
}

pub(crate) fn metadata(text: &str) -> Metadata {
    let visible = uncomment(text, &mut false);
    let text = visible.as_str();
    let mut result = Metadata {
        title: String::new(),
        scheduled: None,
        due: None,
        expectation: None,
        kind: None,
        status: None,
        properties: Vec::new(),
        error: String::new(),
        spans: Vec::new(),
    };
    let bytes = text.as_bytes();
    let (mut i, mut code) = (0, 0);
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == b'`' {
            let count = bytes[i..].iter().take_while(|&&c| c == b'`').count();
            if code == 0 {
                code = count;
            } else if code == count {
                code = 0;
            }
            i += count;
            continue;
        }
        if code == 0 && bytes[i] == b'[' {
            let tail = &text[i + 1..];
            if let Some(key) = ["kind", "status"]
                .into_iter()
                .find(|key| tail.starts_with(&format!("{key}::")))
            {
                if let Some(end) = tail.find(']') {
                    let value = tail[key.len() + 2..end].trim().to_owned();
                    if result.properties.iter().any(|(k, _)| k == key) {
                        result.error = tr!("dates.text_004", key);
                    }
                    if key == "kind" {
                        result.kind = Some(value);
                    } else {
                        result.status = Some(value);
                    }
                    result.properties.push((key.into(), i..i + end + 2));
                    i += end + 2;
                    continue;
                }
                result.error = tr!("dates.text_005", key);
            }
            let field = [DateField::Scheduled, DateField::Due, DateField::Expectation]
                .into_iter()
                .find(|field| tail.starts_with(&format!("{}::", field.key())));
            if let Some(field) = field {
                let Some(end) = tail.find(']') else {
                    result.error = tr!("dates.text_005", field.key());
                    break;
                };
                let range = i..i + end + 2;
                let value = tail[field.key().len() + 2..end].trim();
                if result.spans.iter().any(|(f, _)| *f == field) {
                    result.error = tr!("dates.text_004", field.key());
                }
                match parse_date(value) {
                    Ok(date) => match field {
                        DateField::Scheduled => result.scheduled = Some(date),
                        DateField::Due => result.due = Some(date),
                        DateField::Expectation => result.expectation = Some(date),
                    },
                    Err(_) => result.error = tr!("dates.text_003", field.key(), value),
                }
                i = range.end;
                result.spans.push((field, range));
                continue;
            }
        }
        i += 1;
    }
    let mut start = 0;
    let mut ranges: Vec<_> = result
        .spans
        .iter()
        .map(|(_, r)| r)
        .chain(result.properties.iter().map(|(_, r)| r))
        .collect();
    ranges.sort_by_key(|r| r.start);
    for range in ranges {
        result.title.push_str(&text[start..range.start]);
        start = range.end;
    }
    result.title.push_str(&text[start..]);
    result.title = result.title.trim().to_owned();
    result
}

// Mask comments with equal-length spaces so byte offsets still address source.
// Backtick code spans and escaped percent signs are literal text.
pub(crate) fn uncomment(text: &str, comment: &mut bool) -> String {
    let mut bytes = text.as_bytes().to_vec();
    let (mut i, mut code) = (0, 0);
    while i < bytes.len() {
        if !*comment && bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if !*comment && bytes[i] == b'`' {
            let count = bytes[i..].iter().take_while(|&&c| c == b'`').count();
            if code == 0 {
                code = count;
            } else if code == count {
                code = 0;
            }
            i += count;
            continue;
        }
        if code == 0 && bytes[i..].starts_with(b"%%") {
            *comment = !*comment;
            bytes[i..i + 2].fill(b' ');
            i += 2;
        } else {
            if *comment {
                bytes[i] = b' ';
            }
            i += 1;
        }
    }
    String::from_utf8(bytes).expect(tr!("dates.text_007"))
}

pub(crate) fn replace_date(text: &str, field: DateField, date: Option<Date>) -> Result<String> {
    let meta = metadata(text);
    ensure!(
        meta.error.is_empty(),
        "{}",
        tr!("dates.text_002", meta.error)
    );
    let mut result = text.to_owned();
    let replacement = date
        .map(|date| format!("[{}:: {date}]", field.key()))
        .unwrap_or_default();
    if let Some((_, range)) = meta.spans.iter().find(|(f, _)| *f == field) {
        result.replace_range(range.clone(), &replacement);
    } else if date.is_some() {
        // Keep Obsidian block IDs last on the line.
        let visible = uncomment(text, &mut false);
        let visible = visible.trim_end();
        let insert = visible
            .rfind(" ^")
            .filter(|&i| {
                visible[i + 2..]
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-')
            })
            .unwrap_or(visible.len());
        result.insert_str(insert, &format!(" {replacement}"));
    }
    if let Some(date) = date
        && !(1..=9999).contains(&date.year())
    {
        bail!("{}", tr!("dates.text_001"));
    }
    Ok(result)
}

pub(crate) fn replace_property(text: &str, key: &str, value: &str) -> Result<String> {
    let metadata = metadata(text);
    ensure!(metadata.error.is_empty(), "{}", metadata.error);
    let replacement = format!("[{key}:: {value}]");
    let mut result = text.to_owned();
    if let Some((_, range)) = metadata.properties.iter().find(|(k, _)| k == key) {
        result.replace_range(range.clone(), &replacement);
    } else {
        let visible = uncomment(text, &mut false);
        let visible = visible.trim_end();
        let at = visible.rfind(" ^").unwrap_or(visible.len());
        result.insert_str(at, &format!(" {replacement}"));
    }
    Ok(result)
}
