//! Runs a program and reports where it and the processes it starts loaded the named libraries
//! from, so a replacement placed beside a program that Windows silently passed over is caught:
//! `gt-launch <report> <library>[,<library>...]|- <program> [<argument>...]`. The program runs with
//! this one's standard handles, with this as the debugger of it and of every process it starts,
//! until each has loaded every named library (then it is let go) or ends; nothing else of their run
//! is changed (exceptions go to them as they would without a debugger, and the debug heap stays
//! off). Writes `<library> from <path>` per library and place a process first loaded it from
//! (`<library> from not loaded` if none) to <report> and exits with the program's exit code (2 if it could not
//! start).

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
    use std::collections::HashMap;
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
    use windows::Win32::System::Threading::{DEBUG_PROCESS, INFINITE};

    let args: Vec<String> = std::env::args().skip(1).collect();
    let [report, libraries, program, arguments @ ..] = &args[..] else {
        anyhow::bail!(
            "usage: gt-launch <report> <library>[,<library>...]|- <program> [<argument>...]"
        );
    };
    let wanted: Vec<String> = libraries
        .split(',')
        .filter(|l| !l.is_empty() && *l != "-")
        .map(str::to_lowercase)
        .collect();
    // Every place each library was loaded from, in the order first seen.
    let mut found: Vec<Vec<String>> = vec![Vec::new(); wanted.len()];
    let mut child = std::process::Command::new(program)
        .args(arguments)
        .env("_NO_DEBUG_HEAP", "1")
        .creation_flags(DEBUG_PROCESS.0)
        .spawn()
        .with_context(|| format!("starting {program}"))?;
    let root = child.id();
    let path = |file: HANDLE| -> String {
        let mut buffer = [0u16; 1024];
        let n = unsafe { GetFinalPathNameByHandleW(file, &mut buffer, FILE_NAME_NORMALIZED) };
        let path = String::from_utf16_lossy(&buffer[..(n as usize).min(buffer.len())]);
        let _ = unsafe { CloseHandle(file) };
        path.strip_prefix(r"\\?\").unwrap_or(&path).to_owned()
    };
    unsafe { DebugSetProcessKillOnExit(false)? };
    // Processes being debugged: whether the loader's first breakpoint has passed, and which named
    // libraries each has loaded.
    let mut processes: HashMap<u32, (bool, Vec<bool>)> = HashMap::new();
    let mut root_let_go = false;
    loop {
        let mut event = DEBUG_EVENT::default();
        unsafe { WaitForDebugEvent(&mut event, INFINITE)? };
        let pid = event.dwProcessId;
        let process = processes
            .entry(pid)
            .or_insert_with(|| (false, vec![false; wanted.len()]));
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
                    for (i, library) in wanted.iter().enumerate() {
                        // Only a process's first load of a name is the one its program asked
                        // for; a later one is by full path, as a stand-in loads the library it
                        // stands in for.
                        if *library == name && !process.1[i] {
                            process.1[i] = true;
                            if !found[i].iter().any(|p| p.eq_ignore_ascii_case(&loaded)) {
                                found[i].push(loaded.clone());
                            }
                        }
                    }
                }
            }
            EXCEPTION_DEBUG_EVENT => {
                // The loader's breakpoint for a starting debugger is the debugger's; any other
                // exception is the program's own business.
                let code = unsafe { event.u.Exception.ExceptionRecord.ExceptionCode };
                if process.0 || code != EXCEPTION_BREAKPOINT {
                    status = DBG_EXCEPTION_NOT_HANDLED;
                }
                if code == EXCEPTION_BREAKPOINT {
                    process.0 = true;
                }
            }
            _ => {}
        }
        let done = !wanted.is_empty() && process.1.iter().all(|&loaded| loaded);
        unsafe { ContinueDebugEvent(pid, event.dwThreadId, status)? };
        if event.dwDebugEventCode == EXIT_PROCESS_DEBUG_EVENT {
            processes.remove(&pid);
            // The program has ended: what it left running is not followed further.
            if pid == root {
                break;
            }
        } else if done {
            unsafe { DebugActiveProcessStop(pid)? };
            processes.remove(&pid);
            root_let_go |= pid == root;
        }
        if root_let_go && processes.is_empty() {
            break;
        }
    }
    // Processes the program left running are let go.
    for pid in processes.keys() {
        let _ = unsafe { DebugActiveProcessStop(*pid) };
    }
    let code = child.wait()?.code().unwrap_or(2);
    let lines: Vec<String> = wanted
        .iter()
        .zip(&found)
        .flat_map(|(library, places)| {
            if places.is_empty() {
                vec![format!("{library} from not loaded")]
            } else {
                places
                    .iter()
                    .map(|p| format!("{library} from {p}"))
                    .collect()
            }
        })
        .collect();
    std::fs::write(report, lines.join("\n") + "\n").with_context(|| format!("writing {report}"))?;
    Ok(code)
}
