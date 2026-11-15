//! The translator as a WebAssembly module for the browser runner. The page writes its input into the buffer
//! from [`input`], calls an operation, and reads the result from [`output`].

use std::cell::RefCell;
use wcontainers_abi::{Permissions, RegisterBlock};
use wcontainers_translator::contract;
use wcontainers_translator::{LookAhead, Omit, translate_omitting};

thread_local! {
    static INPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static OUTPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// A buffer of `length` bytes for the next operation's input.
#[unsafe(no_mangle)]
pub extern "C" fn input(length: usize) -> *mut u8 {
    INPUT.with_borrow_mut(|input| {
        input.clear();
        input.resize(length, 0);
        input.as_mut_ptr()
    })
}

/// Where the last operation's result is.
#[unsafe(no_mangle)]
pub extern "C" fn output() -> *const u8 {
    OUTPUT.with_borrow(|output| output.as_ptr())
}

fn answer(bytes: Vec<u8>) -> i32 {
    let length = bytes.len() as i32;
    OUTPUT.set(bytes);
    length
}

/// Translate the blob in the input buffer from one start, at look-ahead none (0) or full (1), leaving out the
/// checks set in `omit` (1: stop, 2: permissions, 4: returns; only to measure them). Returns the length of
/// the code module, or -1 if the start or the code range is not valid.
#[unsafe(no_mangle)]
pub extern "C" fn translate(
    code_start: u64,
    code_end: u64,
    start: u64,
    look_ahead: u32,
    omit: u32,
) -> i32 {
    let look_ahead = if look_ahead == 1 {
        LookAhead::Full
    } else {
        LookAhead::None
    };
    let omit = Omit {
        stop: omit & 1 != 0,
        permissions: omit & 2 != 0,
        returns: omit & 4 != 0,
    };
    let result = INPUT.with_borrow(|blob| {
        translate_omitting(blob, code_start..code_end, &[start], look_ahead, omit)
    });
    match result {
        Ok(translation) => answer(translation.wasm),
        Err(_) => -1,
    }
}

/// The thread module with a lookup table of `slots` entries. Returns its length.
#[unsafe(no_mangle)]
pub extern "C" fn thread_module(slots: u32) -> i32 {
    answer(wcontainers_translator::thread_module(slots))
}

/// Everything the runner must agree on with the translator and the shared crate, as JSON. Returns its
/// length.
#[unsafe(no_mangle)]
pub extern "C" fn contract() -> i32 {
    use contract::*;
    use core::mem::offset_of;
    let trap_kinds: Vec<&str> = (0..)
        .map_while(TrapKind::from_i32)
        .map(|kind| match kind {
            TrapKind::Undefined => "undefined",
            TrapKind::Unsupported => "unsupported",
            TrapKind::Breakpoint => "breakpoint",
            TrapKind::Alignment => "alignment",
        })
        .collect();
    let description = serde_json::json!({
        "env": ENV,
        "hooks": HOOKS,
        "thread": THREAD,
        "memory": MEMORY,
        "state": STATE,
        "loadAddress": LOAD_ADDRESS,
        "slotBase": SLOT_BASE,
        "tableBase": TABLE_BASE,
        "threadBlock": THREAD_BLOCK,
        "permissions": PERMISSIONS,
        "permissionPages": PERMISSION_PAGES,
        "run": RUN,
        "saveState": SAVE_STATE,
        "loadState": LOAD_STATE,
        "unwindLeave": UNWIND_LEAVE,
        "hookSyscall": HOOK_SYSCALL,
        "hookStop": HOOK_STOP,
        "hookMemoryFault": HOOK_MEMORY_FAULT,
        "hookLoadCode": HOOK_LOAD_CODE,
        "hookWarn": HOOK_WARN,
        "hookTrap": HOOK_TRAP,
        "pageRead": PAGE_READ,
        "pageWrite": PAGE_WRITE,
        "nextPageRead": NEXT_PAGE_READ,
        "nextPageWrite": NEXT_PAGE_WRITE,
        "read": Permissions::READ.bits(),
        "write": Permissions::WRITE.bits(),
        "execute": Permissions::EXECUTE.bits(),
        "pageSize": wcontainers_abi::PAGE_SIZE,
        "warningThreshold": WARNING_THRESHOLD,
        "reasons": Reason::ALL.map(Reason::name),
        "trapKinds": trap_kinds,
        "stopFlag": wcontainers_abi::STOP_FLAG_OFFSET,
        "threadBlockSize": wcontainers_abi::THREAD_BLOCK_SIZE,
        "registerX": offset_of!(RegisterBlock, x),
        "registerSp": offset_of!(RegisterBlock, sp),
        "registerPc": offset_of!(RegisterBlock, pc),
    });
    answer(description.to_string().into_bytes())
}
