//! Runs a program and reports where it and the processes it starts loaded the named libraries
//! from, so a replacement placed beside a program that Windows silently passed over is caught:
//! `gt-launch <report> <library>[,<library>...]|- <program> [<argument>...]`. The program runs with
//! this one's standard handles, with this as the debugger of it and of every process it starts,
//! until each has loaded every named library (then it is let go) or ends; nothing else of their run
//! is changed (exceptions go to them as they would without a debugger, a closed invalid handle is
//! not raised, as without one, and the debug heap stays off); with `-` it is not debugged at all.
//! Writes `<library> from <path>` per library and place a process first loaded it from (`<library>
//! from not loaded` if none) to <report> and exits with the program's exit code (2 if it could not
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
    use wcontainers_win32::{DebugContinuation, DebugEventKind, StandaloneWin32, Win32};
    use windows::Win32::Foundation::{EXCEPTION_BREAKPOINT, STATUS_INVALID_HANDLE};
    use windows::Win32::System::Threading::DEBUG_PROCESS;

    let win32 = wcontainers_win32_windows::WindowsWin32::new(());

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
        // With no library to look for, the program runs as it would alone.
        .creation_flags(if wanted.is_empty() {
            0
        } else {
            DEBUG_PROCESS.0
        })
        .spawn()
        .with_context(|| format!("starting {program}"))?;
    let root = child.id();
    if !wanted.is_empty() {
        win32.set_debugged_processes_ended_on_exit(false)?;
    }
    // Processes being debugged: whether the loader's first breakpoint has passed, and which named
    // libraries each has loaded.
    let mut processes: HashMap<u32, (bool, Vec<bool>)> = HashMap::new();
    let mut root_let_go = false;
    while !wanted.is_empty() {
        let event = win32.wait_for_debug_event()?;
        let pid = event.process;
        let process = processes
            .entry(pid)
            .or_insert_with(|| (false, vec![false; wanted.len()]));
        let mut status = DebugContinuation::Continue;
        match &event.kind {
            DebugEventKind::LibraryLoaded { path: Some(loaded) } => {
                let name = loaded.rsplit('\\').next().unwrap_or("").to_lowercase();
                for (i, library) in wanted.iter().enumerate() {
                    // Only a process's first load of a name is the one its program asked
                    // for; a later one is by full path, as a stand-in loads the library it
                    // stands in for.
                    if *library == name && !process.1[i] {
                        process.1[i] = true;
                        if !found[i].iter().any(|p| p.eq_ignore_ascii_case(loaded)) {
                            found[i].push(loaded.clone());
                        }
                    }
                }
            }
            DebugEventKind::Exception { code, first_chance } => {
                // The loader's breakpoint for a starting debugger is the debugger's, and so is
                // an invalid handle closed, which Windows raises only under a debugger; any
                // other exception is the program's own business.
                let ours = (!process.0 && *code == EXCEPTION_BREAKPOINT.0)
                    || (*code == STATUS_INVALID_HANDLE.0 && *first_chance);
                if !ours {
                    status = DebugContinuation::ExceptionNotHandled;
                }
                if *code == EXCEPTION_BREAKPOINT.0 {
                    process.0 = true;
                }
            }
            _ => {}
        }
        let done = process.1.iter().all(|&loaded| loaded);
        win32.continue_debug_event(pid, event.thread, status)?;
        if matches!(event.kind, DebugEventKind::ProcessExited) {
            processes.remove(&pid);
            // The program has ended: what it left running is not followed further.
            if pid == root {
                break;
            }
        } else if done {
            win32.stop_debugging(pid)?;
            processes.remove(&pid);
            root_let_go |= pid == root;
        }
        if root_let_go && processes.is_empty() {
            break;
        }
    }
    // Processes the program left running are let go.
    for pid in processes.keys() {
        let _ = win32.stop_debugging(*pid);
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
