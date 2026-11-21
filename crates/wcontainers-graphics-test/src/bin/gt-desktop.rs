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
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::AsRawHandle;
    use std::path::Path;

    use anyhow::{Context, Result};
    use windows::Win32::Foundation::{
        CloseHandle, HANDLE, HANDLE_FLAG_INHERIT, SetHandleInformation, WAIT_OBJECT_0,
    };
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };
    use windows::Win32::System::StationsAndDesktops::{
        CloseDesktop, CreateDesktopW, DESKTOP_CONTROL_FLAGS,
    };
    use windows::Win32::System::Threading::{
        CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, CreateProcessW,
        GetExitCodeProcess, INFINITE, PROCESS_INFORMATION, ResumeThread, STARTF_USESTDHANDLES,
        STARTUPINFOW, WaitForSingleObject,
    };
    use windows::core::{PCWSTR, PWSTR, w};

    fn wide(text: &str) -> Vec<u16> {
        std::ffi::OsStr::new(text)
            .encode_wide()
            .chain([0])
            .collect()
    }

    fn inheritable(file: &std::fs::File) -> Result<HANDLE> {
        let handle = HANDLE(file.as_raw_handle());
        unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT.0, HANDLE_FLAG_INHERIT)? };
        Ok(handle)
    }

    pub fn run(dir: &Path) -> Result<i32> {
        let read = |name: &str| {
            std::fs::read_to_string(dir.join(name)).with_context(|| format!("reading {name}"))
        };
        let command = read("command.txt")?;
        let directory = read("directory.txt")?;
        // The environment block: NAME=VALUE strings each ending in a nul, then one more nul.
        let mut environment: Vec<u16> = Vec::new();
        for line in read("environment.txt")?.lines().filter(|l| !l.is_empty()) {
            environment.extend(wide(line));
        }
        environment.push(0);

        // GENERIC_ALL: the launcher creates processes on it and they create windows there.
        let desktop = unsafe {
            CreateDesktopW(
                w!("wc-rig"),
                PCWSTR::null(),
                None,
                DESKTOP_CONTROL_FLAGS(0),
                0x1000_0000,
                None,
            )?
        };
        let out = std::fs::File::create(dir.join("out.log"))?;
        let err = std::fs::File::create(dir.join("err.log"))?;
        let mut name = wide("WinSta0\\wc-rig");
        let startup = STARTUPINFOW {
            cb: size_of::<STARTUPINFOW>() as u32,
            lpDesktop: PWSTR(name.as_mut_ptr()),
            dwFlags: STARTF_USESTDHANDLES,
            hStdOutput: inheritable(&out)?,
            hStdError: inheritable(&err)?,
            ..Default::default()
        };
        let mut line = wide(&format!("cmd.exe /d /s /c \"{command}\""));
        let directory = wide(directory.trim_end());
        let mut process = PROCESS_INFORMATION::default();
        let job = unsafe { CreateJobObjectW(None, PCWSTR::null())? };
        let limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
            BasicLimitInformation:
                windows::Win32::System::JobObjects::JOBOBJECT_BASIC_LIMIT_INFORMATION {
                    LimitFlags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                    ..Default::default()
                },
            ..Default::default()
        };
        unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast(),
                size_of_val(&limits) as u32,
            )?;
            CreateProcessW(
                PCWSTR::null(),
                Some(PWSTR(line.as_mut_ptr())),
                None,
                None,
                true,
                CREATE_NO_WINDOW | CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT,
                Some(environment.as_ptr().cast()),
                PCWSTR(directory.as_ptr()),
                &startup,
                &mut process,
            )
            .context("starting cmd.exe")?;
            AssignProcessToJobObject(job, process.hProcess)?;
            ResumeThread(process.hThread);
            anyhow::ensure!(
                WaitForSingleObject(process.hProcess, INFINITE) == WAIT_OBJECT_0,
                "waiting for the command"
            );
            let mut code = 0u32;
            GetExitCodeProcess(process.hProcess, &mut code)?;
            CloseHandle(process.hThread)?;
            CloseHandle(process.hProcess)?;
            // Kills what the command left running.
            CloseHandle(job)?;
            CloseDesktop(desktop)?;
            Ok(code as i32)
        }
    }
}
