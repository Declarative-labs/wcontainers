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
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows::Win32::System::Environment::GetCommandLineW;
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_BREAKAWAY_OK,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_BASIC_LIMIT_INFORMATION,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };
    use windows::Win32::System::Threading::{
        CREATE_SUSPENDED, CreateProcessW, GetExitCodeProcess, GetStartupInfoW, INFINITE,
        PROCESS_INFORMATION, ResumeThread, STARTUPINFOW, WaitForSingleObject,
    };
    use windows::core::{PCWSTR, PWSTR};

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
        // Chrome's own child processes (--type=...) never make a profile.
        if !args.iter().any(|a| switch_value(a, "type").is_some())
            && let Some(dir) = args.iter().find_map(|a| switch_value(a, "user-data-dir"))
        {
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

    /// The value of `--name=value`; Chrome on Windows also takes `-` and `/` and any case.
    fn switch_value<'a>(arg: &'a str, name: &str) -> Option<&'a str> {
        let rest = arg
            .strip_prefix("--")
            .or_else(|| arg.strip_prefix('-'))
            .or_else(|| arg.strip_prefix('/'))?;
        let (key, value) = rest.split_once('=')?;
        key.eq_ignore_ascii_case(name).then_some(value)
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
    fn start(real: &Path) -> windows::core::Result<i32> {
        // SAFETY: GetCommandLineW returns the process's nul-terminated command line, which lives as
        // long as the process; the other calls get pointers to buffers that outlive them.
        unsafe {
            let raw = GetCommandLineW().as_wide().to_vec();
            let mut line: Vec<u16> = Vec::with_capacity(raw.len() + 300);
            line.push(b'"' as u16);
            line.extend(real.as_os_str().encode_wide());
            line.push(b'"' as u16);
            line.extend_from_slice(&raw[program_end(&raw)..]);
            line.push(0);
            let application: Vec<u16> = real.as_os_str().encode_wide().chain([0]).collect();

            let mut startup = STARTUPINFOW::default();
            GetStartupInfoW(&mut startup);
            let job = CreateJobObjectW(None, PCWSTR::null())?;
            let limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
                BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION {
                    LimitFlags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_BREAKAWAY_OK,
                    ..Default::default()
                },
                ..Default::default()
            };
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast(),
                size_of_val(&limits) as u32,
            )?;
            let mut process = PROCESS_INFORMATION::default();
            CreateProcessW(
                PCWSTR(application.as_ptr()),
                Some(PWSTR(line.as_mut_ptr())),
                None,
                None,
                true,
                CREATE_SUSPENDED,
                None,
                PCWSTR::null(),
                &startup,
                &mut process,
            )?;
            AssignProcessToJobObject(job, process.hProcess)?;
            ResumeThread(process.hThread);
            CloseHandle(process.hThread)?;
            let mut code = 1u32;
            if WaitForSingleObject(process.hProcess, INFINITE) == WAIT_OBJECT_0 {
                GetExitCodeProcess(process.hProcess, &mut code)?;
            }
            Ok(code as i32)
        }
    }

    /// Where the program name ends in a command line, by CreateProcess's rules: a quoted name runs
    /// to the next quote, an unquoted one to the first space or tab.
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
}
