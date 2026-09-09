use crate::tr;
use anyhow::{Result, ensure};
use std::{path::Path, process::Command};

pub fn choose(explicit: Option<&str>, visual: Option<&str>, editor: Option<&str>) -> String {
    [explicit, visual, editor]
        .into_iter()
        .flatten()
        .find(|value| !value.trim().is_empty())
        .unwrap_or("nvim")
        .into()
}

pub fn command(editor: &str, path: &Path, line: usize) -> Result<Command> {
    let args = if Path::new(editor).is_file() {
        vec![editor.into()]
    } else {
        split(editor)?
    };
    ensure!(
        !args.is_empty() && !args[0].is_empty(),
        "{}",
        tr!("editor.text_002")
    );
    let name = Path::new(&args[0])
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let mut command = Command::new(&args[0]);
    command.args(&args[1..]);
    if matches!(
        name.as_str(),
        "nvim" | "vim" | "vi" | "nano" | "emacs" | "emacsclient"
    ) {
        command.arg(format!("+{}", line.max(1))).arg("--");
    }
    command.arg(path);
    Ok(command)
}

#[cfg(windows)]
pub fn split(command: &str) -> Result<Vec<String>> {
    use windows_sys::Win32::{Foundation::LocalFree, UI::Shell::CommandLineToArgvW};
    ensure!(!command.contains('\0'), "{}", tr!("editor.text_001"));
    if command.trim().is_empty() {
        return Ok(Vec::new());
    }
    let wide: Vec<u16> = command.trim().encode_utf16().chain(Some(0)).collect();
    let mut argc = 0;
    // SAFETY: the input is NUL-terminated; Windows owns a valid argc-element
    // allocation until LocalFree. Each returned argument is NUL-terminated.
    unsafe {
        let argv = CommandLineToArgvW(wide.as_ptr(), &mut argc);
        if argv.is_null() {
            return Err(std::io::Error::last_os_error().into());
        }
        let args = std::slice::from_raw_parts(argv, argc as usize)
            .iter()
            .map(|&arg| {
                let mut len = 0;
                while *arg.add(len) != 0 {
                    len += 1;
                }
                String::from_utf16_lossy(std::slice::from_raw_parts(arg, len))
            })
            .collect();
        LocalFree(argv.cast());
        Ok(args)
    }
}

#[cfg(not(windows))]
pub fn split(command: &str) -> Result<Vec<String>> {
    Ok(shell_words::split(command)?)
}
