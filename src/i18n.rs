use anyhow::{Context, Result, ensure};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    sync::OnceLock,
};

pub fn configure(args: Vec<std::ffi::OsString>) -> Result<Vec<std::ffi::OsString>> {
    let mut locale = std::env::var("MINITASK_LOCALE")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or("ja".into());
    let mut directory = std::env::var_os("MINITASK_LOCALE_DIR")
        .filter(|s| !s.is_empty())
        .map(std::path::PathBuf::from);
    let (mut locale_seen, mut dir_seen) = (false, false);
    let mut remaining = Vec::new();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg
            .to_str()
            .is_some_and(|s| crate::cli::COMMANDS.contains(&s))
        {
            remaining.push(arg);
            remaining.extend(args);
            break;
        }
        match arg.to_str() {
            Some("--locale" | "--locale-dir") => {
                let key = arg.to_string_lossy();
                let value = args
                    .next()
                    .with_context(|| format("locale.option_value", &[key.to_string()]))?;
                let seen = if key == "--locale" {
                    &mut locale_seen
                } else {
                    &mut dir_seen
                };
                ensure!(!*seen, "{}", format("locale.duplicate", &[key.to_string()]));
                *seen = true;
                if key == "--locale" {
                    locale = value
                        .into_string()
                        .map_err(|_| anyhow::anyhow!(text("locale.invalid_name")))?;
                } else {
                    directory = Some(value.into());
                }
            }
            Some("--home" | "-home" | "--editor" | "-editor") => {
                remaining.push(arg);
                if let Some(value) = args.next() {
                    remaining.push(value);
                }
            }
            _ => remaining.push(arg),
        }
    }
    let directory = directory.unwrap_or(
        std::env::current_exe()?
            .parent()
            .context(text("store.text_026"))?
            .join("locales"),
    );
    init(&locale, &directory)?;
    Ok(remaining)
}

pub struct Catalog {
    messages: BTreeMap<String, String>,
}
static CATALOG: OnceLock<Catalog> = OnceLock::new();

fn defaults() -> BTreeMap<String, String> {
    serde_json::from_str(include_str!("../locales/ja.json")).expect("validated built-in locale")
}

impl Catalog {
    pub fn load(locale: &str, directory: &Path) -> Result<Self> {
        ensure!(
            !locale.is_empty()
                && locale
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_')),
            "{}",
            text("locale.invalid_name")
        );
        let mut messages = defaults();
        let path = directory.join(format!("{locale}.json"));
        match fs::read_to_string(&path) {
            Ok(data) => {
                let overrides: BTreeMap<String, String> = serde_json::from_str(&data)
                    .with_context(|| {
                        format!("{}: {}", text("locale.invalid_file"), path.display())
                    })?;
                for (key, value) in overrides {
                    let original = messages
                        .get(&key)
                        .with_context(|| format!("{}: {key}", text("locale.unknown_key")))?;
                    ensure!(
                        slots(original)? == slots(&value)?,
                        "{}: {key}",
                        text("locale.placeholders")
                    );
                    ensure!(
                        !value
                            .chars()
                            .any(|c| c.is_control() && c != '\n' && c != '\t'),
                        "{}: {key}",
                        text("locale.control")
                    );
                    messages.insert(key, value);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && locale == "ja" => {}
            Err(e) => {
                return Err(e)
                    .with_context(|| format!("{}: {}", text("locale.missing"), path.display()));
            }
        }
        Ok(Self { messages })
    }
    pub fn get(&self, key: &str) -> Option<&str> {
        self.messages.get(key).map(String::as_str)
    }
}

pub fn init(locale: &str, directory: &Path) -> Result<()> {
    let catalog = Catalog::load(locale, directory)?;
    // Loading errors use a separate default catalog; a successful load is installed once.
    ensure!(
        CATALOG.set(catalog).is_ok(),
        "{}",
        text("locale.already_loaded")
    );
    Ok(())
}
static DEFAULTS: OnceLock<Catalog> = OnceLock::new();
pub fn text(key: &'static str) -> &'static str {
    CATALOG
        .get()
        .unwrap_or_else(|| {
            DEFAULTS.get_or_init(|| Catalog {
                messages: defaults(),
            })
        })
        .get(key)
        .unwrap_or(key)
}

pub fn cache_key() -> String {
    let catalog = CATALOG.get().unwrap_or_else(|| {
        DEFAULTS.get_or_init(|| Catalog {
            messages: defaults(),
        })
    });
    crate::store::digest(&serde_json::to_vec(&catalog.messages).expect("locale strings serialize"))
}

fn slots(template: &str) -> Result<BTreeSet<usize>> {
    let mut slots = BTreeSet::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        rest = &rest[start + 1..];
        if rest.starts_with('{') {
            rest = &rest[1..];
            continue;
        }
        let end = rest.find('}').context(text("locale.placeholders"))?;
        slots.insert(
            rest[..end]
                .parse::<usize>()
                .context(text("locale.placeholders"))?,
        );
        rest = &rest[end + 1..];
    }
    Ok(slots)
}

pub fn format(key: &'static str, args: &[String]) -> String {
    let mut rest = text(key);
    let mut result = String::new();
    while let Some(start) = rest.find('{') {
        result.push_str(&rest[..start]);
        rest = &rest[start + 1..];
        if rest.starts_with('{') {
            result.push('{');
            rest = &rest[1..];
            continue;
        }
        if let Some(end) = rest.find('}') {
            if let Ok(index) = rest[..end].parse::<usize>()
                && let Some(value) = args.get(index)
            {
                result.push_str(value);
            }
            rest = &rest[end + 1..];
        } else {
            result.push('{');
            break;
        }
    }
    result.push_str(rest);
    result
}

pub fn status_label(status: &str) -> &str {
    let key = match status {
        "backlog" => "state.backlog",
        "actionable" => "state.actionable",
        "in_progress" => "state.in_progress",
        "blocked" => "state.blocked",
        "completed" => "state.completed",
        "cancelled" => "state.cancelled",
        "open" => "state.open",
        "answered" => "state.answered",
        "closed" => "state.closed",
        "active" => "state.active",
        "archived" => "state.archived",
        "achieved" => "state.achieved",
        "dropped" => "state.dropped",
        _ => return status,
    };
    text(key)
}

#[macro_export]
macro_rules! tr {
    ($key:expr) => { $crate::i18n::text($key) };
    ($key:expr, $($arg:expr),+ $(,)?) => { $crate::i18n::format($key, &[$(format!("{}",$arg)),+]) };
}
