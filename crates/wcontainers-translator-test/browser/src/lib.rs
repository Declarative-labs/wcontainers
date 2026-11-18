//! The translator as a WebAssembly module for the browser runner. The page writes its input into the buffer
//! from [`input`], calls an operation, and reads the result from [`output`].

#![recursion_limit = "256"]

use std::cell::RefCell;
use wcontainers_abi::{Permissions, RegisterBlock};
use wcontainers_arm64::Arm64;
use wcontainers_guest_cpu::{GUEST_REASONS, Guest, HookBody, HookDone, Personality};
use wcontainers_translator::contract;
use wcontainers_translator::{LookAhead, Omit, Optimise, translate_in_modules, translate_omitting};

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
/// stop, 2: permissions, 4: returns, 8: depth; only to measure them), for speed (0) or size (1), split into
/// modules of at most `module_bytes` of code when not 0 (only to measure calls between modules). Returns the
/// length of the first code module, or -1 if the start, the code range or the bound is not valid;
/// [`more_modules`] gives the rest.
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
    module_bytes: u32,
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
    let optimise = if optimise == 1 {
        Optimise::Size
    } else {
        Optimise::Speed
    };
    let host = Arm64::host_from_probe(probe);
    let code = code_start..code_end;
    let result = INPUT.with_borrow(|blob| match module_bytes {
        0 => {
            translate_omitting::<Arm64>(blob, code, &[start], look_ahead, optimise, host, (), omit)
        }
        _ => {
            let module_bytes = module_bytes as usize;
            translate_in_modules::<Arm64>(
                blob,
                code,
                &[start],
                look_ahead,
                optimise,
                host,
                (),
                module_bytes,
            )
        }
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
    answer(Arm64::probe_module())
}

thread_local! {
    /// A copy of the thread's state for the guest's own hooks, allocated once and never moved, with room
    /// after it for an instruction's bytes.
    static STATE: RefCell<Vec<u8>> = RefCell::new(vec![
        0;
        contract::thread_state_size::<Arm64>() + Arm64::MAX_LENGTH as usize
    ]);
    static DONE: std::cell::Cell<HookDone> = const {
        std::cell::Cell::new(HookDone {
            results: [0; 2],
            reason: None,
        })
    };
}

/// Where the runner copies the thread's state before [`cpu_hook`] and reads it back after.
#[unsafe(no_mangle)]
pub extern "C" fn cpu_state() -> *mut u8 {
    STATE.with_borrow_mut(|state| state.as_mut_ptr())
}

/// A hook of the guest's own (by its index in the guest's hooks): `Guest::run_hook` on the hook's bytes of
/// the thread's state, copied to [`cpu_state`], with the hook's arguments first (the rest 0). Its results are then read with
/// [`cpu_hook_result`] and the slow path it was with [`cpu_hook_reason`].
#[unsafe(no_mangle)]
pub extern "C" fn cpu_hook(hook: u32, a: i64, b: i64, c: i64, d: i64) {
    let args = [a, b, c, d];
    let types = &Arm64::HOOKS[hook as usize];
    let (count, length) = (types.params.len(), usize::from(types.state.len));
    let done = STATE.with_borrow_mut(|state| {
        Arm64::run_hook(hook as usize, &args[..count], &mut state[..length])
    });
    DONE.set(done);
}

/// The fault a memory fault at `address` is (`Guest::memory_fault`), as its place in the contract's
/// `memoryFaults`: worked out from the thread's state copied to [`cpu_state`] and the faulting instruction's
/// bytes, `length` of them, copied after it.
#[unsafe(no_mangle)]
pub extern "C" fn cpu_memory_fault(address: i64, length: u32) -> i32 {
    use zerocopy::FromBytes;
    STATE.with_borrow(|state| {
        let block = <Arm64 as Personality>::Block::read_from_prefix(state)
            .expect("a register block")
            .0;
        let code_at = contract::thread_state_size::<Arm64>();
        let code = &state[code_at..code_at + length as usize];
        Arm64::memory_fault(address as u64, code, &block) as i32
    })
}

/// Result `i` of the last [`cpu_hook`].
#[unsafe(no_mangle)]
pub extern "C" fn cpu_hook_result(i: u32) -> i64 {
    DONE.get().results[i as usize]
}

/// The reason number of the slow path the last [`cpu_hook`] was, or -1.
#[unsafe(no_mangle)]
pub extern "C" fn cpu_hook_reason() -> i32 {
    DONE.get().reason.map_or(-1, |r| r as i32)
}

/// The thread module. Returns its length.
#[unsafe(no_mangle)]
pub extern "C" fn thread_module() -> i32 {
    answer(wcontainers_translator::thread_module::<Arm64>())
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
            TrapKind::DivideError => "divide error",
            TrapKind::GeneralProtection => "general protection",
            TrapKind::FloatException => "float exception",
            TrapKind::Overflow => "overflow",
        })
        .collect();
    let guest_hooks: Vec<serde_json::Value> = Arm64::HOOKS
        .iter()
        .map(|hook| {
            let body = match hook.body {
                HookBody::Clock => "clock",
                HookBody::CodeChanged => "codeChanged",
                HookBody::Cpu => "cpu",
            };
            serde_json::json!({
                "name": hook.name,
                "body": body,
                "stateAt": hook.state.at,
                "stateLength": hook.state.len,
                // "i32" or "i64" for each result.
                "results": hook
                    .results
                    .iter()
                    .map(|ty| format!("{ty:?}").to_lowercase())
                    .collect::<Vec<_>>(),
            })
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
        "guestHooks": guest_hooks,
        "align": Arm64::ALIGN,
        "maxLength": Arm64::MAX_LENGTH,
        "memoryFaults": ["page", "general protection", "stack fault"],
        "lookup": LOOKUP,
        "probe": PROBE,
        "threadStateSize": thread_state_size::<Arm64>(),
        "pageRead": PAGE_READ,
        "pageWrite": PAGE_WRITE,
        "nextPageRead": NEXT_PAGE_READ,
        "nextPageWrite": NEXT_PAGE_WRITE,
        "read": Permissions::READ.bits(),
        "write": Permissions::WRITE.bits(),
        "execute": Permissions::EXECUTE.bits(),
        "pageSize": wcontainers_abi::PAGE_SIZE,
        "warningThreshold": WARNING_THRESHOLD,
        "reasons": (0..GUEST_REASONS + Arm64::REASONS.len() as u32)
            .map(Reason::name_of::<Arm64>)
            .collect::<Vec<_>>(),
        "trapKinds": trap_kinds,
        "stopFlag": Arm64::STOP_FLAG_OFFSET,
        "threadBlockSize": Arm64::BLOCK_SIZE,
        "registerX": offset_of!(RegisterBlock, x),
        "registerSp": offset_of!(RegisterBlock, sp),
        "registerPc": offset_of!(RegisterBlock, pc),
    });
    answer(description.to_string().into_bytes())
}
