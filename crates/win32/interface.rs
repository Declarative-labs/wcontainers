//! Windows system calls other than Direct3D's: processes run in job objects, debugging, modules and windows.

use std::ffi::OsString;
use std::fs::File;
use std::io;
use std::path::PathBuf;

use raw_window_handle::Win32WindowHandle;

/// The Windows system calls the graphics tests and the OpenGL driver make, other than Direct3D's.
pub trait Win32 {
    /// Returns this process's command line as Windows gave it.
    fn command_line(&self) -> OsString;

    /// Returns the path this process loaded the module `name` from, or none when it has not loaded it.
    fn module_path(&self, name: &str) -> Option<String>;

    /// Runs `run` to its end, then closes its job, which ends what it left running.
    ///
    /// Returns the process's exit code, or none when the wait ended before the process did.
    fn run_process(&self, run: ProcessRun) -> io::Result<Option<u32>>;

    /// Sets whether the processes this thread debugs are ended when this thread exits.
    fn set_debugged_processes_ended_on_exit(&self, ended: bool) -> io::Result<()>;

    /// Waits for the next event of a process this thread debugs.
    fn wait_for_debug_event(&self) -> io::Result<DebugEvent>;

    /// Lets the thread of an event go on as `continuation` says.
    fn continue_debug_event(
        &self,
        process: u32,
        thread: u32,
        continuation: DebugContinuation,
    ) -> io::Result<()>;

    /// Stops debugging `process`, which goes on running.
    fn stop_debugging(&self, process: u32) -> io::Result<()>;

    /// Returns the handle of the window a device context draws into now, or none when it draws into no window.
    ///
    /// The handle only names the window: nothing here keeps it alive, so whoever presents into it promises that.
    fn window_of_device_context(&self, device_context: isize) -> Option<Win32WindowHandle>;
}

/// A process to start suspended in a job object that ends every process in it when it closes, then run and wait for.
pub struct ProcessRun {
    /// The program file, or none to take the program from the command line's first word.
    pub application: Option<PathBuf>,
    /// The whole command line, the program's name first.
    pub command_line: OsString,
    /// The folder the process starts in, or none for this process's own.
    pub directory: Option<PathBuf>,
    /// The process's whole environment as `NAME=VALUE` entries, or none for this process's own.
    pub environment: Option<Vec<OsString>>,
    /// Where the process's standard output and standard error go.
    pub standard_handles: StandardHandles,
    /// A desktop of the interactive window station `WinSta0`, made for the run and closed after it, or none for this
    /// process's own desktop.
    pub desktop: Option<String>,
    /// Whether the process starts without a console window.
    pub without_window: bool,
    /// Whether processes the process starts may leave its job.
    pub breakaway_allowed: bool,
}

/// The standard handles a process starts with.
pub enum StandardHandles {
    /// The handles of this process's own startup information.
    Inherited,
    /// Two files, which become inheritable.
    Files {
        /// The file standard output writes to.
        output: File,
        /// The file standard error writes to.
        error: File,
    },
}

/// One event of a process this thread debugs.
pub struct DebugEvent {
    /// The process the event is of.
    pub process: u32,
    /// The thread the event is of.
    pub thread: u32,
    /// What happened.
    pub kind: DebugEventKind,
}

/// What a debug event says happened.
pub enum DebugEventKind {
    /// A process started, with the path of its program file when the event names one.
    ProcessCreated {
        /// The program file's path.
        image: Option<String>,
    },
    /// A process loaded a library, with the library file's path when the event names one.
    LibraryLoaded {
        /// The library file's path.
        path: Option<String>,
    },
    /// A thread raised an exception.
    Exception {
        /// The exception's `NTSTATUS` code.
        code: i32,
        /// Whether the debugger is told before the process's own handlers have seen it.
        first_chance: bool,
    },
    /// A process ended.
    ProcessExited,
    /// Any other event.
    Other,
}

/// How a debugged thread goes on after an event.
pub enum DebugContinuation {
    /// As if the debugger handled the event.
    Continue,
    /// With the exception passed to the process's own handlers.
    ExceptionNotHandled,
}

/// The calls as a value of their own, made with no outside state.
pub trait StandaloneWin32: Win32 {
    type Config;

    fn new(config: Self::Config) -> Self;
}
