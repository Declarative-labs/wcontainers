//! The translator as a WebAssembly module for the browser runner. The page writes its input into the buffer
//! from [`input`], calls an operation, and reads the result from [`output`].

#![recursion_limit = "256"]

use std::cell::RefCell;
use wcontainers_abi::{Permissions, RegisterBlock};
use wcontainers_translator::contract;
use wcontainers_translator::{LookAhead, Omit, Optimise, translate_omitting};

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

/// Translate the blob in the input buffer from one start, at look-ahead none (0), full (1) or bounded (2, N
/// in `bound`), for the host the probe found (`probe`, its answer), leaving out the checks set in `omit` (1:
/// stop, 2: permissions, 4: returns, 8: depth; only to measure them). Returns the length of the first code
/// module, or -1 if the start, the code range or the bound is not valid; [`more_modules`] gives the rest.
#[unsafe(no_mangle)]
pub extern "C" fn translate(
    code_start: u64,
    code_end: u64,
    start: u64,
    look_ahead: u32,
    bound: u32,
    probe: i32,
    omit: u32,
    optimise: u32,
) -> i32 {
    let look_ahead = match look_ahead {
        0 => LookAhead::None,
        1 => LookAhead::Full,
        _ => LookAhead::Bounded(bound),
    };
    let omit = Omit {
        stop: omit & 1 != 0,
        permissions: omit & 2 != 0,
        returns: omit & 4 != 0,
        depth: omit & 8 != 0,
    };
    let result = INPUT.with_borrow(|blob| {
        translate_omitting(
            blob,
            code_start..code_end,
            &[start],
            look_ahead,
            if optimise == 1 {
                Optimise::Size
            } else {
                Optimise::Speed
            },
            wcontainers_arm64::Host::from_probe(probe),
            omit,
        )
    });
    match result {
        Ok(translation) => {
            let mut modules = translation.modules.into_iter().map(|m| m.wasm);
            let first = modules.next().expect("a translation has a module");
            MORE.set(modules.collect());
            answer(first)
        }
        Err(_) => -1,
    }
}

thread_local! {
    /// The code modules of the last translation after its first.
    static MORE: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
}

/// How many code modules the last translation has after the one [`translate`] gave.
#[unsafe(no_mangle)]
pub extern "C" fn more_modules() -> u32 {
    MORE.with_borrow(|more| more.len() as u32)
}

/// The `i`th code module of the last translation after its first. Returns its length.
#[unsafe(no_mangle)]
pub extern "C" fn more_module(i: u32) -> i32 {
    let module = MORE.with_borrow(|more| more[i as usize].clone());
    answer(module)
}

/// The module whose `probe` export finds what the engine does where WebAssembly leaves results open. Returns
/// its length.
#[unsafe(no_mangle)]
pub extern "C" fn probe_module() -> i32 {
    answer(wcontainers_arm64::host::probe_module())
}

thread_local! {
    static STATUS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// The float hook's work: one operation in software under `fpcr`, from `fpsr`. Returns the result; the new
/// FPSR is then read with [`float_status`].
#[unsafe(no_mangle)]
pub extern "C" fn float(operation: i32, a: u64, b: u64, c: u64, fpcr: u32, fpsr: u32) -> u64 {
    let mut status = fpsr;
    let result = wcontainers_arm64::exact::run(operation, a, b, c, fpcr, &mut status);
    STATUS.set(status);
    result
}

/// FPSR after the last [`float`].
#[unsafe(no_mangle)]
pub extern "C" fn float_status() -> u32 {
    STATUS.get()
}

/// Whether the last [`float`] was the fused multiply-add of a host that does not fuse (1), not exact floating
/// point the program asked for (0), given FPCR and whether the flags are kept.
#[unsafe(no_mangle)]
pub extern "C" fn float_reason(operation: i32, fpcr: u32, tracking: u32) -> u32 {
    let (kind, _) = wcontainers_arm64::exact::Operation::decode(operation);
    u32::from(kind.fused() && fpcr == 0 && tracking == 0)
}

/// The thread module. Returns its length.
#[unsafe(no_mangle)]
pub extern "C" fn thread_module() -> i32 {
    answer(wcontainers_translator::thread_module())
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
        "slots": SLOTS,
        "slotsCover": SLOTS_COVER,
        "exactSlot": EXACT_SLOT,
        "descriptionSection": DESCRIPTION_SECTION,
        "threadBlock": THREAD_BLOCK,
        "permissions": PERMISSIONS,
        "permissionPages": PERMISSION_PAGES,
        "run": RUN,
        "bound": wcontainers_translator::BOUND,
        "unwindLeave": UNWIND_LEAVE,
        "hookSyscall": HOOK_SYSCALL,
        "hookStop": HOOK_STOP,
        "hookMemoryFault": HOOK_MEMORY_FAULT,
        "hookLoadCode": HOOK_LOAD_CODE,
        "hookWarn": HOOK_WARN,
        "hookTrap": HOOK_TRAP,
        "hookFloat": HOOK_FLOAT,
        "hookClock": HOOK_CLOCK,
        "hookCodeChanged": HOOK_CODE_CHANGED,
        "lookup": LOOKUP,
        "probe": wcontainers_arm64::host::PROBE,
        "threadStateSize": THREAD_STATE_SIZE,
        "floatTracking": FLOAT_TRACKING,
        "registerFpsr": offset_of!(RegisterBlock, fpsr),
        "registerFpcr": offset_of!(RegisterBlock, fpcr),
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
