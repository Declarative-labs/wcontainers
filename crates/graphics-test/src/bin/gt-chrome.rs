//! Stands at the rig's Chrome for Testing path (chrome-win64\chrome.exe, put there by
//! desktop/install-chrome.ps1) and runs the real browser from the sibling directory
//! chrome-win64-real with this process's command line, standard handles and inherited handles
//! (Playwright's --remote-debugging-pipe uses descriptors 3 and 4) as they are, in a job object that
//! ends the browser when this process ends.
//!
//! Before a browser starts on a new --user-data-dir it writes a "Local State" holding the answer
//! Chrome caches there to "is the Windows password blank": no. Without it Chrome asks Windows on
//! every new profile by logging on with a blank password (from Autofill's device
//! re-authentication check when the first tab opens), which Windows counts as a failed logon
//! (event 4625) of the account; ten in ten minutes lock it, and ssh with it.
#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    #[cfg(windows)]
    std::process::exit(chrome::run());
    #[cfg(not(windows))]
    {
        eprintln!("gt-chrome runs on Windows only");
        std::process::exit(2);
    }
}

#[cfg(windows)]
mod chrome {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::Path;

    use wcontainers_win32::{ProcessRun, StandaloneWin32, StandardHandles, Win32};

    use super::{new_profile, program_end};

    const LOCAL_STATE: &str = r#"{"password_manager":{"os_password_blank":false,"os_password_last_changed":"9000000000000000000"}}"#;

    pub fn run() -> i32 {
        let exe = std::env::current_exe().expect("own path");
        let real = exe
            .parent()
            .and_then(Path::parent)
            .expect("chrome-win64's parent")
            .join("chrome-win64-real")
            .join("chrome.exe");
        if !real.is_file() {
            eprintln!("gt-chrome: {} is missing", real.display());
            return 1;
        }
        let args: Vec<String> = std::env::args_os()
            .skip(1)
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        if let Some(dir) = new_profile(&args) {
            seed_local_state(Path::new(dir));
        }
        match start(&real) {
            Ok(code) => code,
            Err(error) => {
                eprintln!("gt-chrome: could not run {}: {error}", real.display());
                1
            }
        }
    }

    fn seed_local_state(dir: &Path) {
        let file = dir.join("Local State");
        if file.exists() {
            return;
        }
        // A failure here only means Chrome checks the password as it would without this wrapper.
        let _ = std::fs::create_dir_all(dir).and_then(|()| std::fs::write(&file, LOCAL_STATE));
    }

    /// Runs `real` with this process's command line after its program name; returns its exit code.
    fn start(real: &Path) -> std::io::Result<i32> {
        let win32 = wcontainers_win32_windows::WindowsWin32::new(());
        let raw: Vec<u16> = win32.command_line().encode_wide().collect();
        let mut line: Vec<u16> = Vec::with_capacity(raw.len() + 300);
        line.push(b'"' as u16);
        line.extend(real.as_os_str().encode_wide());
        line.push(b'"' as u16);
        line.extend_from_slice(&raw[program_end(&raw)..]);
        let code = win32.run_process(ProcessRun {
            application: Some(real.to_path_buf()),
            command_line: OsString::from_wide(&line),
            directory: None,
            environment: None,
            standard_handles: StandardHandles::Inherited,
            desktop: None,
            without_window: false,
            breakaway_allowed: true,
        })?;
        Ok(code.map_or(1, |code| code as i32))
    }
}

/// The profile directory a browser process starts on, if it may make one: Chrome's own child
/// processes (--type=...) never do.
#[cfg(any(windows, test))]
fn new_profile(args: &[String]) -> Option<&str> {
    if args.iter().any(|a| switch_value(a, "type").is_some()) {
        return None;
    }
    args.iter().find_map(|a| switch_value(a, "user-data-dir"))
}

/// The value of `--name=value`; Chrome on Windows also takes `-` and `/` and any case.
#[cfg(any(windows, test))]
fn switch_value<'a>(arg: &'a str, name: &str) -> Option<&'a str> {
    let rest = arg
        .strip_prefix("--")
        .or_else(|| arg.strip_prefix('-'))
        .or_else(|| arg.strip_prefix('/'))?;
    let (key, value) = rest.split_once('=')?;
    key.eq_ignore_ascii_case(name).then_some(value)
}

/// Where the program name ends in a command line, by CreateProcess's rules: a quoted name runs to
/// the next quote, an unquoted one to the first space or tab.
#[cfg(any(windows, test))]
fn program_end(line: &[u16]) -> usize {
    let quote = b'"' as u16;
    if line.first() == Some(&quote) {
        line[1..]
            .iter()
            .position(|&c| c == quote)
            .map_or(line.len(), |i| i + 2)
    } else {
        line.iter()
            .position(|&c| c == b' ' as u16 || c == b'\t' as u16)
            .unwrap_or(line.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    #[test]
    fn switches_take_any_prefix_and_case() {
        assert_eq!(
            switch_value("--user-data-dir=C:\\p", "user-data-dir"),
            Some("C:\\p")
        );
        assert_eq!(switch_value("-user-data-dir=a", "user-data-dir"), Some("a"));
        assert_eq!(
            switch_value("/User-Data-Dir=a b", "user-data-dir"),
            Some("a b")
        );
        assert_eq!(switch_value("--user-data-dir=", "user-data-dir"), Some(""));
        assert_eq!(switch_value("--user-data-dir", "user-data-dir"), None);
        assert_eq!(
            switch_value("--user-data-directory=a", "user-data-dir"),
            None
        );
        assert_eq!(switch_value("user-data-dir=a", "user-data-dir"), None);
        assert_eq!(switch_value("--type=renderer", "type"), Some("renderer"));
    }

    #[test]
    fn only_the_browser_process_makes_a_profile() {
        let args = |list: &[&str]| list.iter().map(|a| a.to_string()).collect::<Vec<_>>();
        assert_eq!(
            new_profile(&args(&[
                "--headless",
                "--user-data-dir=C:\\p",
                "about:blank"
            ])),
            Some("C:\\p")
        );
        assert_eq!(
            new_profile(&args(&["--type=gpu-process", "--user-data-dir=C:\\p"])),
            None
        );
        assert_eq!(
            new_profile(&args(&["/user-data-dir=q", "-TYPE=renderer"])),
            None
        );
        assert_eq!(new_profile(&args(&["--headless"])), None);
    }

    #[test]
    fn the_program_name_ends_as_createprocess_reads_it() {
        let line = wide("\"C:\\a b\\chrome.exe\" --x");
        assert_eq!(program_end(&line), "\"C:\\a b\\chrome.exe\"".len());
        let line = wide("C:\\chrome.exe --x");
        assert_eq!(program_end(&line), "C:\\chrome.exe".len());
        assert_eq!(program_end(&wide("chrome.exe\t--x")), "chrome.exe".len());
        assert_eq!(program_end(&wide("chrome.exe")), "chrome.exe".len());
        assert_eq!(program_end(&wide("\"C:\\no end")), "\"C:\\no end".len());
        assert_eq!(program_end(&wide("\"\" --x")), 2);
    }
}
