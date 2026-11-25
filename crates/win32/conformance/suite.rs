//! The tests every implementation of wcontainers-win32 runs, written against its traits alone.
//!
//! Each starts its processes from Windows' own `cmd.exe`.
#![cfg(windows)]

use std::ffi::OsString;
use std::fs::File;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use wcontainers_win32::{DebugContinuation, DebugEventKind, ProcessRun, StandardHandles, Win32};

/// The command interpreter every test starts, which Windows has in its system folder.
const SHELL: &str = "cmd.exe";

/// The exit code the run's command ends with.
const EXIT_CODE: u32 = 7;

/// The module every Windows process loads.
const KERNEL: &str = "kernel32.dll";

/// A module no process loads.
const MISSING_MODULE: &str = "no-such-module.dll";

/// The creation flag that makes the starting thread the debugger of a process and of every process it starts.
///
/// https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags#:~:text=DEBUG_PROCESS
const DEBUG_PROCESS: u32 = 0x0000_0001;

/// Returns a run of the shell that ends with `EXIT_CODE`.
fn exiting_run(standard_handles: StandardHandles) -> ProcessRun {
    ProcessRun {
        application: None,
        command_line: OsString::from(format!("{SHELL} /d /s /c \"exit {EXIT_CODE}\"")),
        directory: None,
        environment: None,
        standard_handles,
        desktop: None,
        without_window: true,
        breakaway_allowed: false,
    }
}

/// Checks that the command line of the test's process names a program.
pub fn gives_the_command_line<T: Win32>(subject: T) {
    assert!(!subject.command_line().is_empty());
}

/// Checks that a loaded module has the path of its file and one not loaded has none.
pub fn finds_loaded_modules<T: Win32>(subject: T) {
    let path = subject.module_path(KERNEL).expect("kernel32.dll is loaded");
    assert!(path.to_lowercase().ends_with(KERNEL));
    assert_eq!(subject.module_path(MISSING_MODULE), None);
}

/// Checks that a run gives its process's exit code, with the handles inherited or written to files.
pub fn runs_a_process_to_its_exit_code<T: Win32>(subject: T) {
    let code = subject
        .run_process(exiting_run(StandardHandles::Inherited))
        .unwrap();
    assert_eq!(code, Some(EXIT_CODE));
    let output = file_named("output");
    let error = file_named("error");
    let files = StandardHandles::Files {
        output: File::create(&output).unwrap(),
        error: File::create(&error).unwrap(),
    };
    let code = subject.run_process(exiting_run(files)).unwrap();
    assert_eq!(code, Some(EXIT_CODE));
    std::fs::remove_file(output).unwrap();
    std::fs::remove_file(error).unwrap();
}

/// Checks that a debugged process's events start with its creation and end with its exit, each let go on.
pub fn follows_a_debugged_process<T: Win32>(subject: T) {
    let mut child = Command::new(SHELL)
        .args(["/d", "/s", "/c", "exit 0"])
        .creation_flags(DEBUG_PROCESS)
        .spawn()
        .unwrap();
    subject.set_debugged_processes_ended_on_exit(false).unwrap();
    let root = child.id();
    let mut created = false;
    loop {
        let event = subject.wait_for_debug_event().unwrap();
        let exited = event.process == root && matches!(event.kind, DebugEventKind::ProcessExited);
        if let DebugEventKind::ProcessCreated { image } = &event.kind {
            created |= event.process == root && image.is_some();
        }
        // The loader's breakpoint is the debugger's; the shell raises no other exception.
        subject
            .continue_debug_event(event.process, event.thread, DebugContinuation::Continue)
            .unwrap();
        if exited {
            break;
        }
    }
    assert!(created);
    assert!(child.wait().unwrap().success());
}

/// Checks that a debugged process goes on alone once the debugger stops.
pub fn stops_debugging_a_process<T: Win32>(subject: T) {
    let mut child = Command::new(SHELL)
        .args(["/d", "/s", "/c", "exit 0"])
        .creation_flags(DEBUG_PROCESS)
        .spawn()
        .unwrap();
    let event = subject.wait_for_debug_event().unwrap();
    subject
        .continue_debug_event(event.process, event.thread, DebugContinuation::Continue)
        .unwrap();
    subject.stop_debugging(child.id()).unwrap();
    assert!(child.wait().unwrap().success());
}

/// Checks that a device context that names nothing draws into no window.
pub fn finds_no_window_without_a_device_context<T: Win32>(subject: T) {
    assert!(subject.window_of_device_context(0).is_none());
}

/// Returns the path of a file in the temporary folder named by `stem` and this process's number.
fn file_named(stem: &str) -> PathBuf {
    std::env::temp_dir().join(format!("{stem}-{}.txt", std::process::id()))
}

/// Runs each suite function above as its own test on a value made anew for every test, and each group's where the value
/// has it.
#[macro_export]
macro_rules! tests {
    (@StandaloneWin32 yes ($value:expr)) => {
        $crate::tests!(@StandaloneWin32 yes ($value) ($value));
    };
    (@StandaloneWin32 yes ($base:expr) ($value:expr)) => {
        mod standalone_win32 {}
    };
    (@StandaloneWin32 no ($value:expr)) => {
        mod standalone_win32 {
            #[test]
            fn lacks_standalone_win32() {
                struct Probe<T>(::core::marker::PhantomData<T>);
                impl<T> ::core::clone::Clone for Probe<T> {
                    fn clone(&self) -> Self {
                        Probe(::core::marker::PhantomData)
                    }
                }
                struct Implemented;
                #[allow(dead_code)]
                impl<T: ::wcontainers_win32::StandaloneWin32> Probe<T> {
                    fn clone(&self) -> Implemented {
                        Implemented
                    }
                }
                fn probe_of<T>(_value: &T) -> Probe<T> {
                    Probe(::core::marker::PhantomData)
                }
                let _: Probe<_> = probe_of(&$value).clone();
            }
        }
    };
    ($value:expr; StandaloneWin32: $standalone_win32:ident $(($standalone_win32_value:expr))?) => {
        #[test]
        fn gives_the_command_line() {
            $crate::gives_the_command_line($value);
        }

        #[test]
        fn finds_loaded_modules() {
            $crate::finds_loaded_modules($value);
        }

        #[test]
        fn runs_a_process_to_its_exit_code() {
            $crate::runs_a_process_to_its_exit_code($value);
        }

        #[test]
        fn follows_a_debugged_process() {
            $crate::follows_a_debugged_process($value);
        }

        #[test]
        fn stops_debugging_a_process() {
            $crate::stops_debugging_a_process($value);
        }

        #[test]
        fn finds_no_window_without_a_device_context() {
            $crate::finds_no_window_without_a_device_context($value);
        }
        $crate::tests!(@StandaloneWin32 $standalone_win32 ($value) $(($standalone_win32_value))?);
    };
}
