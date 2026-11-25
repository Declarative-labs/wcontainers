//! Runs a job's command on the desktop "wc-rig" of the logged-in user's session, where windows and
//! swap chains work but nothing shows on the user's screen. Started by the scheduled task
//! wc-desktop (interactive, so in that session) as `gt-desktop <dir>`: <dir> holds command.txt (a
//! cmd.exe command line), directory.txt (where it runs) and environment.txt (NAME=VALUE lines,
//! its whole environment). Writes the command's standard output to out.log and standard error to
//! err.log, then its exit code to exit.txt (125 and the reason in err.log when the launcher itself
//! failed). Holds running.lock open without sharing while it runs, so the waiter knows it has ended
//! even if it died. Leftover processes of the command are killed when it ends.
#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    #[cfg(windows)]
    {
        use std::io::Write;
        let Some(dir) = std::env::args_os().nth(1).map(std::path::PathBuf::from) else {
            std::process::exit(2)
        };
        let lock = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .share_mode(0)
            .open(dir.join("running.lock"));
        let code = match lock
            .map_err(anyhow::Error::from)
            .and_then(|_lock| desktop::run(&dir))
        {
            Ok(code) => code,
            Err(error) => {
                if let Ok(mut log) = std::fs::OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(dir.join("err.log"))
                {
                    let _ = writeln!(log, "gt-desktop: {error:#}");
                }
                125
            }
        };
        let _ = std::fs::write(dir.join("exit.tmp"), code.to_string())
            .and_then(|()| std::fs::rename(dir.join("exit.tmp"), dir.join("exit.txt")));
    }
}

#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;

#[cfg(windows)]
mod desktop {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    use anyhow::{Context, Result};
    use wcontainers_win32::{ProcessRun, StandaloneWin32, StandardHandles, Win32};

    pub fn run(dir: &Path) -> Result<i32> {
        let read = |name: &str| {
            std::fs::read_to_string(dir.join(name)).with_context(|| format!("reading {name}"))
        };
        let command = read("command.txt")?;
        let directory = read("directory.txt")?;
        let environment: Vec<OsString> = read("environment.txt")?
            .lines()
            .filter(|l| !l.is_empty())
            .map(OsString::from)
            .collect();
        let output = std::fs::File::create(dir.join("out.log"))?;
        let error = std::fs::File::create(dir.join("err.log"))?;
        let code = wcontainers_win32_windows::WindowsWin32::new(())
            .run_process(ProcessRun {
                application: None,
                command_line: OsString::from(format!("cmd.exe /d /s /c \"{command}\"")),
                directory: Some(PathBuf::from(directory.trim_end())),
                environment: Some(environment),
                standard_handles: StandardHandles::Files { output, error },
                desktop: Some("wc-rig".to_owned()),
                without_window: true,
                breakaway_allowed: false,
            })?
            .context("waiting for the command")?;
        Ok(code as i32)
    }
}
