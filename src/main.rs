use anyhow::{Context, Result, bail, ensure};
use minitask::tr;
use minitask::{
    editor,
    store::{Store, lock_store},
    ui::{self, App},
};
use std::{env, ffi::OsString, fs, path::PathBuf};

fn main() {
    if let Err(error) = run() {
        eprintln!("minitask: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let (mut home, mut editor_arg, mut reindex) = (None, None, false);
    let mut split_legacy = false;
    let mut command = None;
    let mut query_args = Vec::new();
    let mut args = minitask::i18n::configure(env::args_os().skip(1).collect())?.into_iter();
    while let Some(arg) = args.next() {
        if arg
            .to_str()
            .is_some_and(|a| minitask::cli::COMMANDS.contains(&a))
            && command.is_none()
        {
            command = Some(
                arg.into_string()
                    .map_err(|_| anyhow::anyhow!("{}", tr!("main.text_007")))?,
            );
            continue;
        }
        if command.is_some() {
            query_args.push(
                arg.into_string()
                    .map_err(|_| anyhow::anyhow!("{}", tr!("main.text_006")))?,
            );
            continue;
        }
        match arg.to_str() {
            Some("--home" | "-home") => {
                home = Some(PathBuf::from(args.next().context(tr!("main.text_011"))?))
            }
            Some("--editor" | "-editor") => {
                editor_arg = Some(
                    args.next()
                        .context(tr!("main.text_010"))?
                        .into_string()
                        .map_err(|_| anyhow::anyhow!("{}", tr!("main.text_005")))?,
                )
            }
            Some("--reindex" | "-reindex") => reindex = true,
            Some("--split-legacy") => split_legacy = true,
            Some("--help" | "-h" | "-help") => {
                println!("{}", tr!("cli.options"));
                print!("{}", minitask::cli::help());
                return Ok(());
            }
            Some("--version" | "-V") => {
                println!("{}", tr!("main.text_004", env!("CARGO_PKG_VERSION")));
                return Ok(());
            }
            _ => bail!("{}", tr!("cli.text_016", arg.to_string_lossy())),
        }
    }
    let root = match home.or_else(|| nonempty_env("MINITASK_HOME").map(PathBuf::from)) {
        Some(root) => root,
        None => default_home()?,
    };
    if let Some(command) = command {
        if query_args.len() == 1 && matches!(query_args[0].as_str(), "--help" | "-h") {
            print!("{}", minitask::cli::help());
            return Ok(());
        }
        ensure!(
            !reindex && !split_legacy && editor_arg.is_none(),
            "{}",
            tr!("main.text_003")
        );
        return minitask::cli::run(&root, &command, &query_args);
    }
    fs::create_dir_all(&root)?;
    let _lock = lock_store(&root)?;
    let mut store = Store::open(&root)?;
    if split_legacy {
        let count = store.split_legacy()?;
        ensure!(store.warning.is_empty(), "{}", store.warning);
        println!("{}", tr!("main.text_002", count, store.root.display()));
        return Ok(());
    }
    if reindex {
        let count = store.refresh(true, None)?;
        ensure!(store.warning.is_empty(), "{}", store.warning);
        println!("{}", tr!("main.text_001", count, store.root.display()));
        return Ok(());
    }
    let visual = env::var("VISUAL").ok();
    let fallback = env::var("EDITOR").ok();
    let editor = editor::choose(
        editor_arg.as_deref(),
        visual.as_deref(),
        fallback.as_deref(),
    );
    ui::run(App::new(store, editor))
}

fn nonempty_env(name: &str) -> Option<OsString> {
    env::var_os(name).filter(|value| !value.is_empty())
}

fn default_home() -> Result<PathBuf> {
    #[cfg(windows)]
    let config = nonempty_env("APPDATA")
        .map(PathBuf::from)
        .context(tr!("main.text_009"))?;
    #[cfg(target_os = "macos")]
    let config = PathBuf::from(nonempty_env("HOME").context(tr!("main.text_008"))?)
        .join("Library/Application Support");
    #[cfg(not(any(windows, target_os = "macos")))]
    let config = match nonempty_env("XDG_CONFIG_HOME") {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from(nonempty_env("HOME").context(tr!("main.text_008"))?).join(".config"),
    };
    Ok(config.join("minitask"))
}
