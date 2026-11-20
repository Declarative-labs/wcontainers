//! The translator as a WebAssembly module for the browser runner. The page writes its input into the buffer
//! from [`input`], calls an operation, and reads the result from [`output`].

#![recursion_limit = "256"]

use std::cell::RefCell;
use wcontainers_abi::Permissions;
#[cfg(feature = "arm64")]
use wcontainers_arm64::Arm64 as G;
#[cfg(not(any(feature = "arm64", feature = "x86_64")))]
compile_error!("name the guest with a feature: arm64 or x86_64");
use wcontainers_guest_cpu::{
    GUEST_REASONS, Guest, HookBody, HookDone, HookType, Personality, Step, StepAt,
};
use wcontainers_translator::contract;
use wcontainers_translator::{
    Error, LookAhead, Omit, Optimise, Translation, translate_in_modules, translate_omitting,
};
#[cfg(feature = "x86_64")]
use wcontainers_x86::X86_64 as G;

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

/// Translate the blob in the input buffer from one start, linked at `link_address` when `linked` is 1 (see
/// `translate`), at look-ahead none (0), full (1) or bounded (2, N in `bound`), for the host the probe found (`probe`, its answer), leaving out the checks set in `omit` (1:
/// stop, 2: permissions, 4: returns, 8: depth; only to measure them), for speed (0) or size (1), split into
/// modules of at most `module_bytes` of code when not 0 (only to measure calls between modules). Returns the
/// length of the first code module, or -1 if the start, the code range or the bound is not valid;
/// [`more_modules`] gives the rest.
#[unsafe(no_mangle)]
pub extern "C" fn translate(
    code_start: u64,
    code_end: u64,
    linked: u32,
    link_address: u64,
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
    let code = code_start..code_end;
    let link_address = (linked == 1).then_some(link_address);
    let result = INPUT.with_borrow(|blob| {
        translate_for::<G>(
            blob,
            (code, link_address),
            start,
            look_ahead,
            optimise,
            probe,
            omit,
            module_bytes,
        )
    });
    match result {
        Ok(translation) => {
            READ.set(translation.read);
            let mut modules = translation.modules.into_iter().map(|m| m.wasm);
            let first = modules.next().expect("a translation has a module");
            MORE.set(modules.collect());
            answer(first)
        }
        Err(_) => -1,
    }
}

/// Translate for guest `C` as [`translate`] says, with what the CPU claims when the container asks for nothing
/// more.
#[allow(clippy::too_many_arguments)]
fn translate_for<C: Guest>(
    blob: &[u8],
    (code, link_address): (core::ops::Range<u64>, Option<u64>),
    start: u64,
    look_ahead: LookAhead,
    optimise: Optimise,
    probe: i32,
    omit: Omit,
    module_bytes: u32,
) -> Result<Translation, Error> {
    let host = C::host_from_probe(probe);
    let offer = C::Offer::default();
    match module_bytes {
        0 => translate_omitting::<C>(
            blob,
            code,
            link_address,
            &[start],
            look_ahead,
            optimise,
            host,
            offer,
            omit,
        ),
        _ => translate_in_modules::<C>(
            blob,
            code,
            link_address,
            &[start],
            look_ahead,
            optimise,
            host,
            offer,
            module_bytes as usize,
        ),
    }
}

thread_local! {
    /// The code modules of the last translation after its first.
    static MORE: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
    /// The blob bytes the last translation was made from.
    static READ: RefCell<Vec<core::ops::Range<u64>>> = const { RefCell::new(Vec::new()) };
}

/// The blob bytes the last translation was made from, as little-endian u64 pairs of start and end. Returns
/// their length in bytes.
#[unsafe(no_mangle)]
pub extern "C" fn translation_read() -> i32 {
    let bytes = READ.with_borrow(|read| {
        read.iter()
            .flat_map(|r| [r.start, r.end])
            .flat_map(u64::to_le_bytes)
            .collect()
    });
    answer(bytes)
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
    answer(G::probe_module())
}

thread_local! {
    /// A copy of the thread's state for the guest's own hooks, allocated once and never moved, with room
    /// after it for an instruction's bytes.
    static STATE: RefCell<Vec<u8>> = RefCell::new(vec![
        0;
        contract::thread_state_size::<G>() + G::MAX_LENGTH as usize
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
    let args: [i64; HookType::MAX_PARAMS] = [a, b, c, d];
    let types = &G::HOOKS[hook as usize];
    let (count, length) = (types.params.len(), usize::from(types.state.len));
    let done = STATE
        .with_borrow_mut(|state| G::run_hook(hook as usize, &args[..count], &mut state[..length]));
    DONE.set(done);
}

/// The fault a memory fault at `address` is (`Guest::memory_fault`), as its place in the contract's
/// `memoryFaults`: worked out from the thread's state copied to [`cpu_state`] and the faulting instruction's
/// bytes, `length` of them, copied after it.
#[unsafe(no_mangle)]
pub extern "C" fn cpu_memory_fault(address: i64, length: u32) -> i32 {
    use zerocopy::FromBytes;
    STATE.with_borrow(|state| {
        let block = <G as Personality>::Block::read_from_prefix(state)
            .expect("a register block")
            .0;
        let code_at = contract::thread_state_size::<G>();
        let code = &state[code_at..code_at + length as usize];
        G::memory_fault(address as u64, code, &block) as i32
    })
}

/// What single stepping does now (`Guest::step`), on the thread's state copied to [`cpu_state`], which the
/// runner copies back, with `kind` the place in `STEP_ATS` of when it is asked. Returns the place of the
/// answer in the contract's `steps`.
#[unsafe(no_mangle)]
pub extern "C" fn cpu_step(kind: u32, pc: i64) -> i32 {
    let at = STEP_ATS[kind as usize].1(pc as u64);
    let length = contract::thread_state_size::<G>();
    let step = STATE.with_borrow_mut(|state| G::step(&mut state[..length], at));
    STEPS.iter().position(|s| *s == step).unwrap() as i32
}

/// The answers of `Guest::step`, in the order the contract names them (`STEP_NAMES`).
const STEPS: [Step; 4] = [Step::Off, Step::Run, Step::Exact, Step::Trap];
/// The names runner.js gives the answers of `Guest::step`.
const STEP_NAMES: [&str; 4] = ["off", "run", "exact", "trap"];
/// A `StepAt` made from the pc it is asked at.
type StepAtPc = fn(u64) -> StepAt;
/// When `Guest::step` is asked, by the name runner.js uses; `cpu_step`'s `kind` is the place here.
const STEP_ATS: [(&str, StepAtPc); 3] = [
    ("stop", StepAt::Stop),
    ("kernel", |_| StepAt::Kernel),
    ("retry", StepAt::Retry),
];
/// Keys of the contract that runner.js reads.
const STEP_AT_KEY: &str = "stepAt";
const UNWIND_GO_TO_EXACT_KEY: &str = "unwindGoToExact";
const STEPS_KEY: &str = "steps";
const CPU_STEPS_KEY: &str = "cpuSteps";
const SP_OFFSET_KEY: &str = "spOffset";
const RED_ZONE_KEY: &str = "redZone";
const CODE_CHANGES_DETECTED_KEY: &str = "codeChangesDetected";

// Every hook's arguments fit `cpu_hook`'s and its results `HookDone`'s.
const _: () = {
    let mut i = 0;
    while i < G::HOOKS.len() {
        assert!(
            G::HOOKS[i].params.len() <= HookType::MAX_PARAMS
                && G::HOOKS[i].results.len() <= HookType::MAX_RESULTS
        );
        i += 1;
    }
};

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
    answer(wcontainers_translator::thread_module::<G>())
}

/// The guest's thread block to start a program at `pc` with stack pointer `sp`, in the output buffer.
/// Returns its length.
#[unsafe(no_mangle)]
pub extern "C" fn initial_block(pc: i64, sp: i64) -> i32 {
    use zerocopy::IntoBytes;
    answer(G::initial_block(pc as u64, sp as u64).as_bytes().to_vec())
}

/// Everything the runner must agree on with the translator and the shared crate, as JSON. Returns its
/// length.
#[unsafe(no_mangle)]
pub extern "C" fn contract() -> i32 {
    use contract::*;
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
    let guest_hooks: Vec<serde_json::Value> = G::HOOKS
        .iter()
        .map(|hook| {
            let (body, line) = match hook.body {
                HookBody::Clock => ("clock", 0),
                HookBody::CodeChanged { line } => ("codeChanged", line),
                HookBody::Cpu => ("cpu", 0),
            };
            serde_json::json!({
                "name": hook.name,
                "body": body,
                // For `codeChanged`, the bytes of code it drops around its address.
                "line": line,
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
    let mut description = serde_json::json!({
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
        // How many arguments `cpu_hook` takes after the hook's index.
        "hookParams": HookType::MAX_PARAMS,
        "align": G::ALIGN,
        "maxLength": G::MAX_LENGTH,
        "memoryFaults": ["page", "general protection", "stack fault"],
        "lookup": LOOKUP,
        "probe": PROBE,
        "threadStateSize": thread_state_size::<G>(),
        "pageRead": PAGE_READ,
        "pageWrite": PAGE_WRITE,
        "nextPageRead": NEXT_PAGE_READ,
        "nextPageWrite": NEXT_PAGE_WRITE,
        "read": Permissions::READ.bits(),
        "write": Permissions::WRITE.bits(),
        "execute": Permissions::EXECUTE.bits(),
        "pageSize": wcontainers_abi::PAGE_SIZE,
        "warningThreshold": WARNING_THRESHOLD,
        "reasons": (0..GUEST_REASONS + G::REASONS.len() as u32)
            .map(Reason::name_of::<G>)
            .collect::<Vec<_>>(),
        "trapKinds": trap_kinds,
        "stopFlag": G::STOP_FLAG_OFFSET,
        "threadBlockSize": G::BLOCK_SIZE,
        "pcOffset": G::PC_OFFSET,
        "flagsWordOffset": G::FLAGS_WORD_OFFSET,
    });
    let detected = G::CODE_CHANGES == wcontainers_guest_cpu::CodeChanges::Detected;
    let more = description.as_object_mut().expect("an object");
    for (key, value) in [
        (
            UNWIND_GO_TO_EXACT_KEY,
            serde_json::json!(UNWIND_GO_TO_EXACT),
        ),
        (STEPS_KEY, serde_json::json!(STEP_NAMES)),
        (
            STEP_AT_KEY,
            serde_json::Value::Object(
                STEP_ATS
                    .iter()
                    .enumerate()
                    .map(|(kind, (name, _))| (name.to_string(), kind.into()))
                    .collect(),
            ),
        ),
        (CPU_STEPS_KEY, serde_json::json!(G::STEPS)),
        (CODE_CHANGES_DETECTED_KEY, serde_json::json!(detected)),
        (
            SP_OFFSET_KEY,
            serde_json::json!(
                G::BITS
                    .iter()
                    .find(|b| b.bit == G::SP_BIT)
                    .map(|b| b.offset)
            ),
        ),
        (RED_ZONE_KEY, serde_json::json!(G::RED_ZONE)),
    ] {
        more.insert(key.into(), value);
    }
    answer(description.to_string().into_bytes())
}
