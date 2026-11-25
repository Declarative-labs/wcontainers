//! The calls made through Microsoft's `windows` crate, each pointer they take made from a value of the call itself.
#![cfg(windows)]

use std::ffi::{OsStr, OsString, c_void};
use std::fs::File;
use std::io;
use std::num::NonZeroIsize;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::io::AsRawHandle;

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawWindowHandle,
    Win32WindowHandle, WindowHandle,
};
use wcontainers_win32::{
    DebugContinuation, DebugEvent, DebugEventKind, ProcessRun, StandaloneWin32, StandardHandles,
    Win32,
};
use windows::Win32::Foundation::{
    CloseHandle, DBG_CONTINUE, DBG_EXCEPTION_NOT_HANDLED, HANDLE, HANDLE_FLAG_INHERIT,
    SetHandleInformation, WAIT_OBJECT_0,
};
use windows::Win32::Graphics::Gdi::{HDC, WindowFromDC};
use windows::Win32::Storage::FileSystem::{FILE_NAME_NORMALIZED, GetFinalPathNameByHandleW};
use windows::Win32::System::Diagnostics::Debug::{
    CREATE_PROCESS_DEBUG_EVENT, ContinueDebugEvent, DEBUG_EVENT, DebugActiveProcessStop,
    DebugSetProcessKillOnExit, EXCEPTION_DEBUG_EVENT, EXIT_PROCESS_DEBUG_EVENT,
    LOAD_DLL_DEBUG_EVENT, WaitForDebugEvent,
};
use windows::Win32::System::Environment::GetCommandLineW;
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_BREAKAWAY_OK,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_BASIC_LIMIT_INFORMATION,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};
use windows::Win32::System::LibraryLoader::{GetModuleFileNameW, GetModuleHandleW};
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, CreateDesktopW, DESKTOP_CONTROL_FLAGS,
};
use windows::Win32::System::Threading::{
    CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, CreateProcessW,
    GetExitCodeProcess, GetStartupInfoW, INFINITE, PROCESS_CREATION_FLAGS, PROCESS_INFORMATION,
    ResumeThread, STARTF_USESTDHANDLES, STARTUPINFOW, WaitForSingleObject,
};
use windows::Win32::UI::WindowsAndMessaging::{GWLP_HINSTANCE, GetWindowLongPtrW};
use windows::core::{PCWSTR, PWSTR};

/// The interactive window station, whose desktops a run's desktop is one of.
const WINDOW_STATION: &str = "WinSta0";

/// The access a run's desktop is made with, `GENERIC_ALL`: processes are made on it and they make windows there.
///
/// https://learn.microsoft.com/en-us/windows/win32/secauthz/generic-access-rights
const DESKTOP_ACCESS: u32 = 0x1000_0000;

/// The longest path read back, in UTF-16 units.
const PATH_UNITS: usize = 1024;

/// The prefix of a path `GetFinalPathNameByHandleW` gives, which the paths given here leave out.
const LONG_PATH_PREFIX: &str = r"\\?\";

/// The calls, which keep no state between them.
pub struct WindowsWin32;

/// A window of the program, named by its handle.
pub struct Win32Window {
    /// The window's handle, kept as its number so that a value may go to any thread.
    window: NonZeroIsize,
    /// The module of the window's instance, which Vulkan's surfaces need.
    module: Option<NonZeroIsize>,
}

impl StandaloneWin32 for WindowsWin32 {
    type Config = ();

    fn new(_config: ()) -> WindowsWin32 {
        WindowsWin32
    }
}

impl Win32 for WindowsWin32 {
    type Window = Win32Window;

    fn command_line(&self) -> OsString {
        #[expect(unsafe_code, reason = "the read of this process's command line")]
        // SAFETY: GetCommandLineW returns the process's nul-terminated command line, which lives as long as the
        // process.
        let line = unsafe { GetCommandLineW().as_wide().to_vec() };
        OsString::from_wide(&line)
    }

    fn module_path(&self, name: &str) -> Option<String> {
        let wide: Vec<u16> = name.encode_utf16().chain([0]).collect();
        #[expect(unsafe_code, reason = "the call that finds a loaded module")]
        // SAFETY: a nul-terminated name that outlives the call.
        let module = unsafe { GetModuleHandleW(PCWSTR(wide.as_ptr())) }.ok()?;
        let mut buffer = [0u16; PATH_UNITS];
        #[expect(unsafe_code, reason = "the call that reads a module's path")]
        // SAFETY: a module this process has loaded, and a buffer of this function.
        let length = unsafe { GetModuleFileNameW(Some(module), &mut buffer) } as usize;
        Some(String::from_utf16_lossy(&buffer[..length]))
    }

    fn run_process(&self, run: ProcessRun) -> io::Result<Option<u32>> {
        let mut line = wide(&run.command_line);
        let application = run.application.as_ref().map(|path| wide(path.as_os_str()));
        let directory = run.directory.as_ref().map(|path| wide(path.as_os_str()));
        // The environment block: NAME=VALUE strings each ending in a nul, then one more nul.
        let environment = run.environment.as_ref().map(|entries| {
            let mut block: Vec<u16> = entries.iter().flat_map(|entry| wide(entry)).collect();
            block.push(0);
            block
        });
        let desktop = match &run.desktop {
            Some(name) => {
                let name = wide(OsStr::new(name));
                #[expect(unsafe_code, reason = "the call that makes a desktop")]
                // SAFETY: a nul-terminated name that outlives the call, and no device or attributes.
                let desktop = unsafe {
                    CreateDesktopW(
                        PCWSTR(name.as_ptr()),
                        PCWSTR::null(),
                        None,
                        DESKTOP_CONTROL_FLAGS(0),
                        DESKTOP_ACCESS,
                        None,
                    )?
                };
                Some(desktop)
            }
            None => None,
        };
        let mut desktop_name = run
            .desktop
            .as_ref()
            .map(|name| wide(OsStr::new(&format!("{WINDOW_STATION}\\{name}"))));
        let mut startup = STARTUPINFOW::default();
        match &run.standard_handles {
            StandardHandles::Inherited => {
                #[expect(unsafe_code, reason = "the read of this process's startup information")]
                // SAFETY: a structure of this function.
                unsafe {
                    GetStartupInfoW(&mut startup)
                };
            }
            StandardHandles::Files { output, error } => {
                startup.cb = size_of::<STARTUPINFOW>() as u32;
                startup.dwFlags = STARTF_USESTDHANDLES;
                startup.hStdOutput = inheritable(output)?;
                startup.hStdError = inheritable(error)?;
            }
        }
        if let Some(name) = &mut desktop_name {
            startup.lpDesktop = PWSTR(name.as_mut_ptr());
        }
        let mut flags: PROCESS_CREATION_FLAGS = CREATE_SUSPENDED;
        if run.without_window {
            flags |= CREATE_NO_WINDOW;
        }
        if environment.is_some() {
            flags |= CREATE_UNICODE_ENVIRONMENT;
        }
        let mut limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if run.breakaway_allowed {
            limit_flags |= JOB_OBJECT_LIMIT_BREAKAWAY_OK;
        }
        let limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
            BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION {
                LimitFlags: limit_flags,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut process = PROCESS_INFORMATION::default();
        #[expect(
            unsafe_code,
            reason = "the calls that start the process in its job and wait for it"
        )]
        // SAFETY: every pointer is to a buffer or structure of this function, which outlives the calls; the handles
        // are the ones these calls made, each closed once.
        unsafe {
            let job = CreateJobObjectW(None, PCWSTR::null())?;
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast(),
                size_of_val(&limits) as u32,
            )?;
            CreateProcessW(
                application
                    .as_ref()
                    .map_or(PCWSTR::null(), |application| PCWSTR(application.as_ptr())),
                Some(PWSTR(line.as_mut_ptr())),
                None,
                None,
                true,
                flags,
                environment
                    .as_ref()
                    .map(|block| block.as_ptr().cast::<c_void>()),
                directory
                    .as_ref()
                    .map_or(PCWSTR::null(), |directory| PCWSTR(directory.as_ptr())),
                &startup,
                &mut process,
            )
            .map_err(|e| {
                io::Error::new(
                    io::Error::from(e.clone()).kind(),
                    format!("starting the process: {e}"),
                )
            })?;
            AssignProcessToJobObject(job, process.hProcess)?;
            ResumeThread(process.hThread);
            CloseHandle(process.hThread)?;
            let code = if WaitForSingleObject(process.hProcess, INFINITE) == WAIT_OBJECT_0 {
                let mut code = 0u32;
                GetExitCodeProcess(process.hProcess, &mut code)?;
                Some(code)
            } else {
                None
            };
            CloseHandle(process.hProcess)?;
            // Ends what the process left running.
            CloseHandle(job)?;
            if let Some(desktop) = desktop {
                CloseDesktop(desktop)?;
            }
            Ok(code)
        }
    }

    fn set_debugged_processes_ended_on_exit(&self, ended: bool) -> io::Result<()> {
        #[expect(
            unsafe_code,
            reason = "the call that sets what happens to debugged processes"
        )]
        // SAFETY: a flag of this thread's debugging.
        unsafe {
            DebugSetProcessKillOnExit(ended)?
        };
        Ok(())
    }

    fn wait_for_debug_event(&self) -> io::Result<DebugEvent> {
        let mut event = DEBUG_EVENT::default();
        #[expect(unsafe_code, reason = "the call that waits for a debug event")]
        // SAFETY: a structure of this function.
        unsafe {
            WaitForDebugEvent(&mut event, INFINITE)?
        };
        #[expect(
            unsafe_code,
            reason = "the reads of the union member the event's code names"
        )]
        // SAFETY: each member read is the one `dwDebugEventCode` says the union holds.
        let kind = unsafe {
            match event.dwDebugEventCode {
                CREATE_PROCESS_DEBUG_EVENT => DebugEventKind::ProcessCreated {
                    image: path(event.u.CreateProcessInfo.hFile),
                },
                LOAD_DLL_DEBUG_EVENT => DebugEventKind::LibraryLoaded {
                    path: path(event.u.LoadDll.hFile),
                },
                EXCEPTION_DEBUG_EVENT => DebugEventKind::Exception {
                    code: event.u.Exception.ExceptionRecord.ExceptionCode.0,
                    first_chance: event.u.Exception.dwFirstChance != 0,
                },
                EXIT_PROCESS_DEBUG_EVENT => DebugEventKind::ProcessExited,
                _ => DebugEventKind::Other,
            }
        };
        Ok(DebugEvent {
            process: event.dwProcessId,
            thread: event.dwThreadId,
            kind,
        })
    }

    fn continue_debug_event(
        &self,
        process: u32,
        thread: u32,
        continuation: DebugContinuation,
    ) -> io::Result<()> {
        let status = match continuation {
            DebugContinuation::Continue => DBG_CONTINUE,
            DebugContinuation::ExceptionNotHandled => DBG_EXCEPTION_NOT_HANDLED,
        };
        #[expect(unsafe_code, reason = "the call that lets a debugged thread go on")]
        // SAFETY: plain numbers of a debug event.
        unsafe {
            ContinueDebugEvent(process, thread, status)?
        };
        Ok(())
    }

    fn stop_debugging(&self, process: u32) -> io::Result<()> {
        #[expect(unsafe_code, reason = "the call that stops debugging a process")]
        // SAFETY: a plain process number.
        unsafe {
            DebugActiveProcessStop(process)?
        };
        Ok(())
    }

    fn window_of_device_context(&self, device_context: isize) -> Option<Win32Window> {
        #[expect(
            unsafe_code,
            reason = "the call that asks which window a device context draws into"
        )]
        // SAFETY: a handle Windows checks; one that names no device context gives no window.
        let window = unsafe { WindowFromDC(HDC(device_context as *mut c_void)) };
        let handle = NonZeroIsize::new(window.0 as isize)?;
        #[expect(unsafe_code, reason = "the read of a field of the program's window")]
        // SAFETY: a window handle Windows gave, which it checks.
        let module = unsafe { GetWindowLongPtrW(window, GWLP_HINSTANCE) };
        Some(Win32Window {
            window: handle,
            module: NonZeroIsize::new(module),
        })
    }
}

impl HasWindowHandle for Win32Window {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let mut window = Win32WindowHandle::new(self.window);
        window.hinstance = self.module;
        #[expect(
            unsafe_code,
            reason = "the promise that the program's window outlives the handle"
        )]
        // SAFETY: the program keeps its window while it draws into it.
        let handle = unsafe { WindowHandle::borrow_raw(RawWindowHandle::Win32(window)) };
        Ok(handle)
    }
}

impl HasDisplayHandle for Win32Window {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::windows())
    }
}

/// Returns `text` as UTF-16 with a terminating nul.
fn wide(text: &OsStr) -> Vec<u16> {
    text.encode_wide().chain([0]).collect()
}

/// Returns `file`'s handle, made inheritable by the processes this one starts.
fn inheritable(file: &File) -> io::Result<HANDLE> {
    let handle = HANDLE(file.as_raw_handle());
    #[expect(
        unsafe_code,
        reason = "the call that makes a file's handle inheritable"
    )]
    // SAFETY: the handle of a file the caller holds open.
    unsafe {
        SetHandleInformation(handle, HANDLE_FLAG_INHERIT.0, HANDLE_FLAG_INHERIT)?
    };
    Ok(handle)
}

/// Returns the path of the file a debug event's handle names, then closes the handle; none when the event gave none.
fn path(file: HANDLE) -> Option<String> {
    if file.is_invalid() {
        return None;
    }
    let mut buffer = [0u16; PATH_UNITS];
    #[expect(
        unsafe_code,
        reason = "the calls that read a debug event's file path and close its handle"
    )]
    // SAFETY: a file handle a debug event gave this debugger, closed once here, and a buffer of this function.
    let units = unsafe {
        let units = GetFinalPathNameByHandleW(file, &mut buffer, FILE_NAME_NORMALIZED);
        let _ = CloseHandle(file);
        units
    };
    let path = String::from_utf16_lossy(&buffer[..(units as usize).min(buffer.len())]);
    Some(
        path.strip_prefix(LONG_PATH_PREFIX)
            .unwrap_or(&path)
            .to_owned(),
    )
}
