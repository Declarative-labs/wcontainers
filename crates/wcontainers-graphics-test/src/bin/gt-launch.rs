//! Runs a program and reports where it loaded the named libraries from, so a replacement placed
//! beside a program that Windows silently passed over is caught:
//! `gt-launch <report> <library>[,<library>...]|- <program> [<argument>...]`. The program runs with
//! this one's standard handles, as its debugger until every named library has loaded (then it is let
//! go) or it ends; nothing else of its run is changed (exceptions go to it as they would without a
//! debugger, and the debug heap stays off). Writes `<library> from <path>` or `<library> from not
//! loaded` per line to <report> and exits with the program's exit code (2 if it could not start).

fn main() {
    #[cfg(windows)]
    match launch() {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("gt-launch: {e:#}");
            std::process::exit(2)
        }
    }
}

#[cfg(windows)]
fn launch() -> anyhow::Result<i32> {
    use std::os::windows::process::CommandExt;

    use anyhow::Context;
    use windows::Win32::Foundation::{
        CloseHandle, DBG_CONTINUE, DBG_EXCEPTION_NOT_HANDLED, EXCEPTION_BREAKPOINT, HANDLE,
    };
    use windows::Win32::Storage::FileSystem::{FILE_NAME_NORMALIZED, GetFinalPathNameByHandleW};
    use windows::Win32::System::Diagnostics::Debug::{
        CREATE_PROCESS_DEBUG_EVENT, ContinueDebugEvent, DEBUG_EVENT, DebugActiveProcessStop,
        DebugSetProcessKillOnExit, EXCEPTION_DEBUG_EVENT, EXIT_PROCESS_DEBUG_EVENT,
        LOAD_DLL_DEBUG_EVENT, WaitForDebugEvent,
    };
    use windows::Win32::System::Threading::{DEBUG_ONLY_THIS_PROCESS, INFINITE};

    let args: Vec<String> = std::env::args().skip(1).collect();
    let [report, libraries, program, arguments @ ..] = &args[..] else {
        anyhow::bail!(
            "usage: gt-launch <report> <library>[,<library>...]|- <program> [<argument>...]"
        );
    };
    let mut wanted: Vec<(String, Option<String>)> = libraries
        .split(',')
        .filter(|l| !l.is_empty() && *l != "-")
        .map(|l| (l.to_lowercase(), None))
        .collect();
    let mut child = std::process::Command::new(program)
        .args(arguments)
        .env("_NO_DEBUG_HEAP", "1")
        .creation_flags(DEBUG_ONLY_THIS_PROCESS.0)
        .spawn()
        .with_context(|| format!("starting {program}"))?;
    let path = |file: HANDLE| -> String {
        let mut buffer = [0u16; 1024];
        let n = unsafe { GetFinalPathNameByHandleW(file, &mut buffer, FILE_NAME_NORMALIZED) };
        let path = String::from_utf16_lossy(&buffer[..(n as usize).min(buffer.len())]);
        let _ = unsafe { CloseHandle(file) };
        path.strip_prefix(r"\\?\").unwrap_or(&path).to_owned()
    };
    unsafe { DebugSetProcessKillOnExit(false)? };
    let mut first_breakpoint = true;
    loop {
        let mut event = DEBUG_EVENT::default();
        unsafe { WaitForDebugEvent(&mut event, INFINITE)? };
        let mut status = DBG_CONTINUE;
        match event.dwDebugEventCode {
            CREATE_PROCESS_DEBUG_EVENT => {
                let file = unsafe { event.u.CreateProcessInfo.hFile };
                if !file.is_invalid() {
                    path(file);
                }
            }
            LOAD_DLL_DEBUG_EVENT => {
                let file = unsafe { event.u.LoadDll.hFile };
                if !file.is_invalid() {
                    let loaded = path(file);
                    let name = loaded.rsplit('\\').next().unwrap_or("").to_lowercase();
                    for (library, found) in &mut wanted {
                        if *library == name && found.is_none() {
                            *found = Some(loaded.clone());
                        }
                    }
                }
            }
            EXCEPTION_DEBUG_EVENT => {
                // The loader's breakpoint for a starting debugger is the debugger's; any other
                // exception is the program's own business.
                let code = unsafe { event.u.Exception.ExceptionRecord.ExceptionCode };
                if !(first_breakpoint && code == EXCEPTION_BREAKPOINT) {
                    status = DBG_EXCEPTION_NOT_HANDLED;
                }
                if code == EXCEPTION_BREAKPOINT {
                    first_breakpoint = false;
                }
            }
            _ => {}
        }
        unsafe { ContinueDebugEvent(event.dwProcessId, event.dwThreadId, status)? };
        if event.dwDebugEventCode == EXIT_PROCESS_DEBUG_EVENT {
            break;
        }
        if wanted.iter().all(|(_, found)| found.is_some()) {
            unsafe { DebugActiveProcessStop(event.dwProcessId)? };
            break;
        }
    }
    let code = child.wait()?.code().unwrap_or(2);
    let lines: Vec<String> = wanted
        .iter()
        .map(|(library, found)| {
            format!(
                "{library} from {}",
                found.as_deref().unwrap_or("not loaded")
            )
        })
        .collect();
    std::fs::write(report, lines.join("\n") + "\n").with_context(|| format!("writing {report}"))?;
    Ok(code)
}
