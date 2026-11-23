//! Layer 1 of the native output (docs/x86-native-output.md section 8): a case's instruction translated for a host
//! process and run on the reference machine in a blob of its own. The blob maps the case's data and code areas,
//! the thread area and the lookup's first level, installs a stop handler, loads the case's state and enters the
//! piece; at the first signal the handler writes out its frame, the data area and the register area. The guest's
//! state there is read through the position map, as the inside reads it. Where the code calls on the inside to
//! answer (or a target lies past the lookup's first level), that state must be the case's start, and the
//! meaning, which the inside follows, gives the outcome from there.

use super::*;
use crate::layer1::{Agreement, compare_outcome, on_reference, translated};
use crate::layout::{DATA_READ_ONLY, fill, place};
use crate::reference::{BlobRequest, Reference};
use iced_x86::IcedError;
use iced_x86::code_asm::{
    AsmRegister64, CodeAssembler, ax, ds, dword_ptr, eax, ecx, edx, es, fs, gs, ptr, qword_ptr, r8,
    r9, r10, r11, r12, r13, r14, r15, rax, rbp, rbx, rcx, rdi, rdx, rsi, rsp,
};
use linux_raw_sys::general::{
    SA_ONSTACK, SA_RESTORER, SA_SIGINFO, kernel_sigaction, kernel_sigset_t, sigaltstack,
};
use wcontainers_guest_cpu::InsideCall;
use wcontainers_guest_cpu::position::{Fixup, Held, Resume, Side, Span};
use wcontainers_translator::{LookAhead, translate_native_for};
use wcontainers_x86::Fxsave;
use wcontainers_x86::area::{FIRST_LEVEL, FIXED, FixedPart, RegisterArea};
use wcontainers_x86::carry::WITHHELD;
use wcontainers_x86::cpuid::{AVX_STATE_BYTES, AVX_STATE_OFFSET, XSAVE_AVX, XSAVE_SSE, XSAVE_X87};
use wcontainers_x86::native::{HostMatches, Target, Way};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

/// What the probe finds on the rented x86-64 machine, the reference (a Xeon 8488C with 5-level paging,
/// measured at XN0): it is the CPU whose results the meaning follows.
pub const REFERENCE_HOST: HostMatches = HostMatches {
    five_level_paging: true,
    partial_writes: true,
    undefined_results: true,
};

const PAGE: u64 = PAGE_SIZE;
const WORD: u64 = size_of::<u64>() as u64;
/// Where the blob is loaded, far from every address a case reaches; its parts follow at fixed offsets.
const LOAD: u64 = 1 << 43;
const PROLOGUE: u64 = LOAD;
const HANDLER: u64 = LOAD + PAGE;
const PIECE: u64 = LOAD + 2 * PAGE;
/// The most host code one instruction's piece may take here.
const PIECE_ROOM: u64 = 2 * PAGE;
const BLOCK: u64 = PIECE + PIECE_ROOM;
const XSAVE_IMAGE: u64 = BLOCK + PAGE;
const DATA_IMAGE: u64 = XSAVE_IMAGE + PAGE;
/// The thread area: the register area, then the fixed part, then the lookup's first level, which the prologue
/// maps.
const AREA: u64 = DATA_IMAGE + DATA_BYTES;
const FIRST: u64 = AREA + FIRST_LEVEL as u64;
/// The first level's words of the lower half's regions, mapped; a target at or above `USER_END` faults on
/// reading its word, as the plan has it (section 3.3).
const LOWER_REGIONS: u64 = USER_END >> u32::BITS;
/// The second level every first-level word points into, never mapped: every lookup misses there, its target in t.
const SECOND_LEVEL: u64 = 1 << 44;
/// How far apart the second-level tables of two regions are.
const REGION_BYTES: u64 = WORD << u32::BITS;
/// The inside's entries: call code k jumps to `ENTRY_TRAPS + k * ENTRY_STRIDE`, in a page never mapped.
const ENTRY_TRAPS: u64 = LOAD - (1 << 20);
const ENTRY_STRIDE: u64 = 16;
/// Where a call the inside makes nothing of in a host process (before a float status instruction: the hardware
/// holds the guest's float status) goes: `jmp t`, back to the piece. The handler's page ends there.
const RESUME: u64 = PIECE - ENTRY_STRIDE;
const ALTERNATE_STACK: u64 = LOAD - (2 << 20);
const ALTERNATE_STACK_BYTES: u64 = 16 * PAGE;
const STACK_TOP: u64 = LOAD - (4 << 20);
const STACK_BYTES: u64 = 16 * PAGE;
/// The signals a stop raises, which the handler takes.
const STOP_SIGNALS: [u32; 5] = [SIGILL, SIGTRAP, SIGBUS, SIGFPE, SIGSEGV];
const STANDARD_OUTPUT: u64 = 1;

const GENERAL_REGISTERS: usize = 16;
/// The block's words: the case's general registers in encoding order, rflags, the FS base, the selectors.
const GENERAL_AT: u64 = 0;
const FLAGS_AT: u64 = GENERAL_AT + GENERAL_REGISTERS as u64 * WORD;
const FS_BASE_AT: u64 = FLAGS_AT + WORD;
const SELECTORS_AT: u64 = FS_BASE_AT + WORD;
const INSTRUCTION_AT: u64 = SELECTORS_AT + wcontainers_x86::SEGMENT_REGISTERS as u64 * WORD;
const ACTION_AT: u64 = INSTRUCTION_AT + size_of::<[u8; 16]>() as u64;
const ALTERNATE_AT: u64 = ACTION_AT + size_of::<kernel_sigaction>() as u64;
const RECORD_AT: u64 = (ALTERNATE_AT + size_of::<sigaltstack>() as u64).next_multiple_of(WORD);

/// The bytes of a frame's general registers, which its XSAVE area's pointer follows, and how many there are.
const FRAME_REGISTER_BYTES: usize = core::mem::offset_of!(libc::mcontext_t, fpregs)
    - core::mem::offset_of!(libc::mcontext_t, gregs);
const FRAME_REGISTERS: usize = FRAME_REGISTER_BYTES / size_of::<libc::greg_t>();
/// What the XSAVE image holds: x87, SSE and the upper halves of the ymm registers.
const XSTATE_BYTES: usize = (AVX_STATE_OFFSET + AVX_STATE_BYTES) as usize;
const XSTATE_COMPONENTS: u32 = XSAVE_X87 | XSAVE_SSE | XSAVE_AVX;
/// Where XSTATE_BV is: just after the legacy area.
const XSTATE_BV_AT: usize = size_of::<Fxsave>();

/// What the handler writes first: the signal, its `si_code` and `si_addr`, the selectors and FS base, the frame's
/// general registers, its XSAVE area.
#[derive(Clone, Copy, FromBytes, IntoBytes, Immutable, KnownLayout)]
#[repr(C)]
struct Record {
    signal: u64,
    code: u64,
    address: u64,
    selectors: [u64; wcontainers_x86::SEGMENT_REGISTERS],
    fs_base: u64,
    registers: [u64; FRAME_REGISTERS],
    xstate: [u8; XSTATE_BYTES],
}

/// The frame's index of each general register, in encoding order.
const FRAME_GENERAL: [libc::c_int; GENERAL_REGISTERS] = [
    libc::REG_RAX,
    libc::REG_RCX,
    libc::REG_RDX,
    libc::REG_RBX,
    libc::REG_RSP,
    libc::REG_RBP,
    libc::REG_RSI,
    libc::REG_RDI,
    libc::REG_R8,
    libc::REG_R9,
    libc::REG_R10,
    libc::REG_R11,
    libc::REG_R12,
    libc::REG_R13,
    libc::REG_R14,
    libc::REG_R15,
];

fn general_register(number: usize) -> AsmRegister64 {
    [
        rax, rcx, rdx, rbx, rsp, rbp, rsi, rdi, r8, r9, r10, r11, r12, r13, r14, r15,
    ][number]
}

/// How a case's run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ended {
    /// The piece's exit: the guest goes on at the next instruction or a branch's target.
    Exit,
    /// A fault in the piece, a copied instruction's own.
    Fault,
    /// A jump through a register, a return or a call through one reached the lookup's second level.
    Lookup,
    /// A call into the inside that answers as the meaning does, with the guest's state as it started.
    Answered(InsideCall),
    /// A target at or above `USER_END`, which the inside answers by the meaning's rule.
    FarTarget,
    /// Not run natively, for this reason.
    NotRun(&'static str),
}

/// A host process never sets the hardware's TF or AC (section 3.6): a guest with either set runs on stepping
/// pieces, which are not built yet.
const STEPPING_NOT_BUILT: &str =
    "not run natively: TF or AC set, which a host process runs on stepping pieces (not built yet)";

/// Compares a case's native run with the CPU's: `Ok` with how it ended when they agree.
pub fn compare(
    reference: &mut Reference,
    case: &Case,
    host: HostMatches,
) -> Result<(Agreement, Ended), String> {
    let held_off = u128::from(wcontainers_x86::TF | wcontainers_x86::ALIGNMENT_CHECK);
    if case.state.0[RFLAGS] & held_off != 0 {
        let ended = Ended::NotRun(STEPPING_NOT_BUILT);
        return Ok((Agreement::KnownFailure(STEPPING_NOT_BUILT), ended));
    }
    // The hardware keeps MXCSR's flags exact, as the meaning does when told to keep them.
    let case = &Case {
        tracking: true,
        ..case.clone()
    };
    let (mut outcome, ended) = run(reference, case, host)?;
    if matches!(ended, Ended::Exit | Ended::Fault | Ended::Lookup) {
        // Seen through a signal frame, as the reference's run is, it is read as the reference's is.
        let seen = outcome.clone();
        X86_64::reconcile(case, &mut outcome, &seen);
    }
    let ours = outcome.stop;
    match compare_outcome::<X86_64>(reference, case, outcome) {
        Ok(agreement) => Ok((agreement, ended)),
        Err(report) => {
            let theirs = on_reference::<X86_64>(reference, case).stop;
            if five_level_only(theirs) && matches!(ours, Stop::GeneralProtection | Stop::StackFault)
            {
                Ok((Agreement::KnownFailure(FIVE_LEVEL_REFERENCE), ended))
            } else if matches!(ended, Ended::Answered(_) | Ended::FarTarget) {
                eprintln!("{report}");
                Ok((Agreement::KnownFailure(MEANING_DIFFERS), ended))
            } else {
                Err(report)
            }
        }
    }
}

/// The reference machine has 5-level paging (cloud/x86/x86-machine.md): an access or a transfer to an address
/// the guest's 4-level rule makes non-canonical faults there as a page fault, where the meaning gives general
/// protection.
const FIVE_LEVEL_REFERENCE: &str = "the reference's 5-level paging: a page fault where a 4-level CPU, and the meaning, give general protection";

/// An answer of the inside is the meaning's, the WebAssembly output's too: where it differs from the reference,
/// so does that output's layer 1, which is where the difference is answered.
const MEANING_DIFFERS: &str = "answered by the meaning, which differs from the reference here as the WebAssembly output's layer 1 does";

/// The end of the lower half and the start of the upper half of 5-level linear addresses.
const FIVE_LEVEL_USER_END: u64 = 1 << 56;
const FIVE_LEVEL_UPPER: u64 = !(FIVE_LEVEL_USER_END - 1);
const FOUR_LEVEL_UPPER: u64 = !(USER_END - 1);

/// Whether a stop of the reference is at an address only 5-level paging makes canonical, or one an access from
/// the last user page crosses into.
fn five_level_only(stop: Stop) -> bool {
    match stop {
        Stop::MemoryFault(a) | Stop::Landed(a) => {
            (USER_TOP..FIVE_LEVEL_USER_END).contains(&a)
                || (FIVE_LEVEL_UPPER..FOUR_LEVEL_UPPER).contains(&a)
        }
        _ => false,
    }
}

/// Runs a case natively: its outcome and how it ended.
pub fn run(
    reference: &mut Reference,
    case: &Case,
    host: HostMatches,
) -> Result<(Outcome, Ended), String> {
    let target = Target {
        host,
        offer: X86_64::case_offer(case),
    };
    let code = translate_native_for::<X86_64>(
        case.code.code(),
        CODE,
        LookAhead::None,
        false,
        &Way::Process,
        &target,
    )
    .ok_or("no instruction at the case's start")?;
    if code.bytes.len() as u64 > PIECE_ROOM {
        return Err(format!("a piece of {} bytes", code.bytes.len()));
    }
    let blob = blob(case, &code.bytes);
    let ran = reference.blob(&BlobRequest {
        bytes: &blob,
        load: LOAD,
        stack_top: STACK_TOP,
        stack_bytes: STACK_BYTES,
        headers: None,
        auxiliary: &[],
        stdin: &[],
        runs: 0,
    })?;
    let (record, rest) = Record::read_from_prefix(&ran.stdout).map_err(|_| {
        format!(
            "the blob wrote no record: status {}, stop {:?}, error {}",
            ran.status,
            ran.stop,
            String::from_utf8_lossy(&ran.stderr)
        )
    })?;
    let data_bytes = DATA_BYTES as usize;
    let (Some(data), Some(area)) = (
        rest.get(..data_bytes),
        rest.get(data_bytes..)
            .and_then(|r| RegisterArea::read_from_prefix(r).ok()),
    ) else {
        return Err(format!(
            "the blob's output ended early: {} bytes",
            ran.stdout.len()
        ));
    };
    let area = area.0;
    let changed = changed(case, data);
    let host = HostState::new(&record);
    let rip = host.rip;
    let signal = record.signal as u32;
    let in_piece = |at: u64| {
        at.checked_sub(PIECE)
            .filter(|&o| o < code.bytes.len() as u64)
            .and_then(|o| code.span(o as u32))
    };
    let first_level = FIRST..FIRST + (u64::from(u32::MAX) + 1) * WORD;
    let second_level = SECOND_LEVEL..SECOND_LEVEL + (u64::from(u32::MAX) + 1) * WORD;
    let calls = ENTRY_TRAPS..ENTRY_TRAPS + InsideCall::ALL.len() as u64 * ENTRY_STRIDE;
    let (span, ended) = if signal == SIGSEGV && calls.contains(&rip) {
        let kind = InsideCall::from_code(((rip - ENTRY_TRAPS) / ENTRY_STRIDE) as u16)
            .ok_or("an entry no call has")?;
        // The call's return point is in t; its jump through the area is the span's last instruction.
        let t = host.general[usize::from(wcontainers_x86::R11)];
        let span =
            in_piece(t.wrapping_sub(1)).ok_or_else(|| format!("a call returning to {t:#x}"))?;
        let ended = match kind {
            InsideCall::Exit | InsideCall::X87Control | InsideCall::FloatAfter => Ended::Exit,
            other => Ended::Answered(other),
        };
        (span, ended)
    } else if let Some(span) = in_piece(rip) {
        let ended = if signal == SIGSEGV && second_level.contains(&record.address) {
            Ended::Lookup
        } else if signal == SIGSEGV && first_level.contains(&record.address) {
            Ended::FarTarget
        } else {
            Ended::Fault
        };
        (span, ended)
    } else {
        return Err(format!(
            "the blob stopped outside the piece: signal {signal}, code {}, rip {rip:#x}, address {:#x}",
            record.code, record.address
        ));
    };
    let state = guest_state(case, &record, &host, &area, span)?;
    let stop = match ended {
        Ended::Exit | Ended::Lookup => Stop::Landed(state.0[RIP] as u64),
        Ended::Fault => X86_64::reference_stop(
            &Signal {
                number: signal,
                code: record.code as u32,
                address: record.address,
                pc: state.0[RIP] as u64,
            },
            case.code.length,
        ),
        Ended::Answered(_) | Ended::FarTarget => {
            if state != case.state || !changed.is_empty() {
                return Err(format!(
                    "the piece changed the guest before the inside's answer ({ended:?}): {}",
                    differences(&case.state, &state)
                ));
            }
            return Ok((translated::<X86_64>(case), ended));
        }
        Ended::NotRun(_) => unreachable!("a run that was made"),
    };
    Ok((
        Outcome {
            stop,
            state,
            changed,
        },
        ended,
    ))
}

/// The fields that differ, named.
fn differences(want: &State, got: &State) -> String {
    X86_64::REFERENCE
        .fields
        .iter()
        .zip(want.0.iter().zip(&got.0))
        .filter(|(_, (a, b))| a != b)
        .map(|(name, (a, b))| format!("{name} {a:#x} became {b:#x}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The data area's bytes that differ from the case's start.
fn changed(case: &Case, now: &[u8]) -> Vec<(u32, u8)> {
    start_data(case)
        .iter()
        .zip(now)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(at, (_, &b))| (at as u32, b))
        .collect()
}

fn start_data(case: &Case) -> Vec<u8> {
    let mut data = vec![0; DATA_BYTES as usize];
    fill(case.seed, &mut data);
    place(&case.placed, &mut data);
    data
}

/// The host's registers at the stop, as the frame has them.
struct HostState {
    general: [u64; GENERAL_REGISTERS],
    rip: u64,
    rflags: u64,
}

impl HostState {
    fn new(record: &Record) -> Self {
        let at = |index: libc::c_int| record.registers[index as usize];
        HostState {
            general: FRAME_GENERAL.map(at),
            rip: at(libc::REG_RIP),
            rflags: at(libc::REG_EFL),
        }
    }
}

/// The guest's state at a stop in `span`, read as the inside reads it: fix-ups first, then each register where the
/// span says it is, the withheld ones in the register area otherwise.
fn guest_state(
    case: &Case,
    record: &Record,
    host: &HostState,
    area: &RegisterArea,
    span: &Span,
) -> Result<State, String> {
    let mut registers = host.general;
    for fixup in &span.fixups {
        match *fixup {
            Fixup::Constant { register, value } | Fixup::Address { register, value } => {
                registers[usize::from(register)] = value;
            }
            Fixup::X87Pointer(_) | Fixup::FloatStatus => {}
            other => return Err(format!("a fix-up x86-64 never makes: {other:?}")),
        }
    }
    let page = area.as_bytes();
    let held = |held: Held| -> Result<u64, String> {
        match held {
            Held::Register(r) => Ok(registers[usize::from(r)]),
            Held::Page(at) => page
                .get(usize::from(at)..usize::from(at) + WORD as usize)
                .map(|w| u64::from_le_bytes(w.try_into().expect("a word")))
                .ok_or_else(|| format!("a word past the register area at {at}")),
        }
    };
    let mut general = registers;
    for (k, &w) in WITHHELD.iter().enumerate() {
        general[usize::from(w)] = area.withheld[k];
    }
    for r in &span.relocated {
        general[usize::from(r.guest)] = held(r.held)?;
    }
    let rip = match (span.side, span.resume) {
        (Side::Before, _) => span.guest,
        (Side::After, Resume::At(address)) => address,
        (Side::After, Resume::Through(at)) => held(at)?,
        (Side::After, Resume::Slot(_)) => return Err("an entry slot x86-64 never has".into()),
    };
    let mut s = vec![0u128; FIELDS];
    for (to, from) in s.iter_mut().zip(general) {
        *to = from.into();
    }
    s[RIP] = rip.into();
    s[RFLAGS] = u128::from(host.rflags) & !RF;
    s[FS] = record.fs_base.into();
    s[GS] = area.gs_base.into();
    let (legacy, _) = Fxsave::read_from_prefix(&record.xstate).expect("the legacy area");
    let bv = u64::from_le_bytes(
        record.xstate[XSTATE_BV_AT..XSTATE_BV_AT + WORD as usize]
            .try_into()
            .expect("a word"),
    );
    let has = |component: u32| bv & u64::from(component) != 0;
    s[MXCSR] = legacy.mxcsr.into();
    if has(XSAVE_X87) {
        s[FCW] = legacy.fcw.into();
        s[FSW] = legacy.fsw.into();
        s[FTW] = legacy.ftw.into();
        let top = usize::from(legacy.fsw >> wcontainers_x86::x87::TOP_SHIFT)
            & (wcontainers_x86::x87::REGISTERS - 1);
        for (i, slot) in legacy.st.iter().enumerate() {
            let value =
                u128::from_le_bytes(*slot) & ((1 << wcontainers_x86::X87_REGISTER_BITS) - 1);
            s[R0 + (top + i) % wcontainers_x86::x87::REGISTERS] = value;
        }
    } else {
        s[FCW] = wcontainers_x86::FCW_AT_START.into();
    }
    if has(XSAVE_SSE) {
        for (i, x) in legacy.xmm.iter().enumerate() {
            s[XMM + i] = u128::from_le_bytes(*x);
        }
    }
    if has(XSAVE_AVX) {
        let upper = &record.xstate[AVX_STATE_OFFSET as usize..];
        let (halves, _) = upper.as_chunks::<{ size_of::<u128>() }>();
        for (i, half) in halves.iter().enumerate() {
            s[YMMH + i] = u128::from_le_bytes(*half);
        }
    }
    for (n, &selector) in record.selectors.iter().enumerate() {
        s[SELECTORS + n] = selector.into();
    }
    // The guest's GS selector is the inside's: the hardware's GS holds the thread area.
    const GS_SELECTOR: usize = SELECTORS + wcontainers_x86::SEGMENT_REGISTERS - 1;
    s[GS_SELECTOR] = case.state.0[GS_SELECTOR];
    Ok(State(s))
}

/// The blob of a case and its piece: the prologue, the handler, the piece, the block, the XSAVE image, the data
/// area's bytes and the thread area's first two pages.
fn blob(case: &Case, piece: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0u8; (FIRST - LOAD) as usize];
    let mut put = |at: u64, part: &[u8]| {
        let at = (at - LOAD) as usize;
        bytes[at..at + part.len()].copy_from_slice(part);
    };
    put(PROLOGUE, &prologue(case).expect("the prologue assembles"));
    let handler = handler().expect("the handler assembles");
    assert!(
        HANDLER + handler.len() as u64 <= RESUME,
        "the handler fits its page"
    );
    put(HANDLER, &handler);
    put(RESUME, &resume().expect("the jump back assembles"));
    put(PIECE, piece);
    let s = &case.state.0;
    let word = |v: u128| (v as u64).to_le_bytes();
    for (n, &value) in s[..GENERAL_REGISTERS].iter().enumerate() {
        put(BLOCK + GENERAL_AT + n as u64 * WORD, &word(value));
    }
    put(BLOCK + FLAGS_AT, &word(s[RFLAGS]));
    put(BLOCK + FS_BASE_AT, &word(s[FS]));
    for n in 0..wcontainers_x86::SEGMENT_REGISTERS {
        put(
            BLOCK + SELECTORS_AT + n as u64 * WORD,
            &word(s[SELECTORS + n]),
        );
    }
    put(BLOCK + INSTRUCTION_AT, &case.code.bytes);
    let action = |field: usize, value: u64| (ACTION_AT + field as u64, value.to_le_bytes());
    let flags = SA_SIGINFO | SA_ONSTACK | SA_RESTORER;
    for (at, value) in [
        action(
            core::mem::offset_of!(kernel_sigaction, sa_handler_kernel),
            HANDLER,
        ),
        action(
            core::mem::offset_of!(kernel_sigaction, sa_flags),
            flags as u64,
        ),
        // Never returned to: the handler ends the process.
        action(
            core::mem::offset_of!(kernel_sigaction, sa_restorer),
            HANDLER,
        ),
        action(core::mem::offset_of!(kernel_sigaction, sa_mask), 0),
        (
            ALTERNATE_AT + core::mem::offset_of!(sigaltstack, ss_sp) as u64,
            ALTERNATE_STACK.to_le_bytes(),
        ),
        (
            ALTERNATE_AT + core::mem::offset_of!(sigaltstack, ss_size) as u64,
            ALTERNATE_STACK_BYTES.to_le_bytes(),
        ),
    ] {
        put(BLOCK + at, &value);
    }
    put(XSAVE_IMAGE, &xsave_image(s));
    put(DATA_IMAGE, &start_data(case));
    let mut area = RegisterArea {
        gs_base: s[GS] as u64,
        ..RegisterArea::default()
    };
    for (k, &w) in WITHHELD.iter().enumerate() {
        area.withheld[k] = s[usize::from(w)] as u64;
    }
    put(AREA, area.as_bytes());
    let fixed = FixedPart {
        entries: core::array::from_fn(|k| match InsideCall::ALL[k] {
            InsideCall::FloatBefore => RESUME,
            _ => ENTRY_TRAPS + k as u64 * ENTRY_STRIDE,
        }),
    };
    put(AREA + FIXED as u64, fixed.as_bytes());
    bytes
}

/// The case's x87, SSE and AVX state as XSAVE lays it out, for `xrstor`.
fn xsave_image(s: &[u128]) -> Vec<u8> {
    let mut legacy = Fxsave::ZERO;
    legacy.fcw = s[FCW] as u16;
    legacy.fsw = s[FSW] as u16;
    legacy.ftw = s[FTW] as u8;
    legacy.mxcsr = s[MXCSR] as u32;
    let registers = wcontainers_x86::x87::REGISTERS;
    let top = (s[FSW] as usize >> wcontainers_x86::x87::TOP_SHIFT) & (registers - 1);
    for (i, slot) in legacy.st.iter_mut().enumerate() {
        let value = s[R0 + (top + i) % registers] & ((1 << wcontainers_x86::X87_REGISTER_BITS) - 1);
        *slot = value.to_le_bytes();
    }
    for (i, x) in legacy.xmm.iter_mut().enumerate() {
        *x = s[XMM + i].to_le_bytes();
    }
    let mut image = vec![0u8; XSTATE_BYTES];
    image[..size_of::<Fxsave>()].copy_from_slice(legacy.as_bytes());
    image[XSTATE_BV_AT..XSTATE_BV_AT + WORD as usize]
        .copy_from_slice(&u64::from(XSTATE_COMPONENTS).to_le_bytes());
    for i in 0..GENERAL_REGISTERS {
        let at = AVX_STATE_OFFSET as usize + i * size_of::<u128>();
        image[at..at + size_of::<u128>()].copy_from_slice(&s[YMMH + i].to_le_bytes());
    }
    image
}

/// `syscall` with its number and arguments.
fn system_call(
    a: &mut CodeAssembler,
    number: libc::c_long,
    arguments: &[u64],
) -> Result<(), IcedError> {
    let registers = [rdi, rsi, rdx, r10, r8, r9];
    a.mov(rax, number as u64)?;
    for (&r, &value) in registers.iter().zip(arguments) {
        a.mov(r, value)?;
    }
    a.syscall()
}

/// `rep movsb` of `bytes` bytes from `from` to `to`.
fn copy(a: &mut CodeAssembler, to: u64, from: u64, bytes: u64) -> Result<(), IcedError> {
    a.mov(rdi, to)?;
    a.mov(rsi, from)?;
    a.mov(rcx, bytes)?;
    a.rep().movsb()
}

/// An anonymous private mapping at `address`, refused where anything is mapped already.
fn map(
    a: &mut CodeAssembler,
    address: u64,
    bytes: u64,
    protection: libc::c_int,
) -> Result<(), IcedError> {
    let flags = libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_FIXED_NOREPLACE;
    let no_file = u64::MAX;
    system_call(
        a,
        libc::SYS_mmap,
        &[address, bytes, protection as u64, flags as u64, no_file, 0],
    )
}

fn protect(
    a: &mut CodeAssembler,
    address: u64,
    bytes: u64,
    protection: libc::c_int,
) -> Result<(), IcedError> {
    system_call(a, libc::SYS_mprotect, &[address, bytes, protection as u64])
}

/// The code that sets the case up and enters the piece.
fn prologue(case: &Case) -> Result<Vec<u8>, IcedError> {
    let mut a = CodeAssembler::new(wcontainers_x86::MODE_BITS)?;
    let read_write = libc::PROT_READ | libc::PROT_WRITE;
    // The data area with the case's bytes, its last page read-only.
    map(&mut a, DATA, DATA_BYTES, read_write)?;
    copy(&mut a, DATA, DATA_IMAGE, DATA_BYTES)?;
    protect(
        &mut a,
        DATA + DATA_WRITABLE,
        DATA_READ_ONLY,
        libc::PROT_READ,
    )?;
    // The code area: the filler, and the instruction at CODE.
    let (code_start, code_bytes) = (CODE - X86_64::CODE_SPAN, 2 * X86_64::CODE_SPAN);
    map(&mut a, code_start, code_bytes, read_write)?;
    a.mov(rdi, code_start)?;
    a.mov(eax, u32::from(X86_64::FILLER))?;
    a.mov(rcx, code_bytes)?;
    a.rep().stosb()?;
    copy(
        &mut a,
        CODE,
        BLOCK + INSTRUCTION_AT,
        u64::from(case.code.length),
    )?;
    protect(
        &mut a,
        code_start,
        code_bytes,
        libc::PROT_READ | libc::PROT_EXEC,
    )?;
    // The first level: region m's word is the second level less m's table offset, for every lower region.
    let first_bytes = LOWER_REGIONS * WORD;
    map(&mut a, FIRST, first_bytes, read_write)?;
    a.mov(rdi, FIRST)?;
    a.mov(rax, SECOND_LEVEL)?;
    a.mov(rdx, REGION_BYTES)?;
    a.mov(rcx, LOWER_REGIONS)?;
    let mut fill = a.create_label();
    a.set_label(&mut fill)?;
    a.mov(qword_ptr(rdi), rax)?;
    a.sub(rax, rdx)?;
    a.add(rdi, WORD as i32)?;
    a.dec(rcx)?;
    a.jnz(fill)?;
    protect(&mut a, FIRST, first_bytes, libc::PROT_READ)?;
    protect(&mut a, AREA + FIXED as u64, PAGE, libc::PROT_READ)?;
    // The handler, on a stack of its own, for every signal a stop raises.
    map(&mut a, ALTERNATE_STACK, ALTERNATE_STACK_BYTES, read_write)?;
    system_call(&mut a, libc::SYS_sigaltstack, &[BLOCK + ALTERNATE_AT, 0])?;
    for signal in STOP_SIGNALS {
        system_call(
            &mut a,
            libc::SYS_rt_sigaction,
            &[
                signal.into(),
                BLOCK + ACTION_AT,
                0,
                size_of::<kernel_sigset_t>() as u64,
            ],
        )?;
    }
    // The thread area on GS; the case's FS base and selectors (one of FS not null clears its base, as the
    // reference loads it).
    a.mov(rax, AREA)?;
    a.wrgsbase(rax)?;
    a.mov(rbx, BLOCK)?;
    a.mov(rax, qword_ptr(rbx + FS_BASE_AT as i32))?;
    a.wrfsbase(rax)?;
    a.mov(eax, dword_ptr(rbx + SELECTORS_AT as i32))?;
    a.mov(ds, ax)?;
    a.mov(eax, dword_ptr(rbx + (SELECTORS_AT + WORD) as i32))?;
    a.mov(es, ax)?;
    const FS_SELECTOR: usize = SELECTORS + 2;
    if case.state.0[FS_SELECTOR] != 0 {
        a.mov(eax, dword_ptr(rbx + (SELECTORS_AT + 2 * WORD) as i32))?;
        a.mov(fs, ax)?;
    }
    // x87, SSE and AVX state.
    a.mov(rbx, XSAVE_IMAGE)?;
    a.mov(eax, XSTATE_COMPONENTS)?;
    a.mov(edx, 0u32)?;
    a.xrstor64(ptr(rbx))?;
    // rflags, then the general registers by moves, which write no flag; c and u start at zero.
    a.mov(rbx, BLOCK)?;
    a.push(qword_ptr(rbx + FLAGS_AT as i32))?;
    a.popfq()?;
    a.mov(r11, BLOCK + GENERAL_AT)?;
    let rsp_number = RSP;
    for n in 0..GENERAL_REGISTERS {
        if n == rsp_number || WITHHELD.contains(&(n as u8)) {
            continue;
        }
        a.mov(
            general_register(n),
            qword_ptr(r11 + (n as u64 * WORD) as i32),
        )?;
    }
    a.mov(rsp, qword_ptr(r11 + (rsp_number as u64 * WORD) as i32))?;
    a.mov(r10, 0u64)?;
    a.mov(r15, 0u64)?;
    a.mov(r11, PIECE)?;
    a.jmp(r11)?;
    a.assemble(PROLOGUE)
}

/// `jmp t`.
fn resume() -> Result<Vec<u8>, IcedError> {
    let mut a = CodeAssembler::new(wcontainers_x86::MODE_BITS)?;
    a.jmp(r11)?;
    a.assemble(RESUME)
}

/// The stop handler: writes the record, the data area and the register area to standard output and ends the
/// process.
fn handler() -> Result<Vec<u8>, IcedError> {
    use linux_raw_sys::general::{__sifields__bindgen_ty_5, siginfo__bindgen_ty_1__bindgen_ty_1};
    let mut a = CodeAssembler::new(wcontainers_x86::MODE_BITS)?;
    let field = |offset: usize| (RECORD_AT as usize + offset) as i32;
    let info_code = core::mem::offset_of!(siginfo__bindgen_ty_1__bindgen_ty_1, si_code) as i32;
    let info_address = (core::mem::offset_of!(siginfo__bindgen_ty_1__bindgen_ty_1, _sifields)
        + core::mem::offset_of!(__sifields__bindgen_ty_5, _addr)) as i32;
    let context = core::mem::offset_of!(libc::ucontext_t, uc_mcontext);
    let frame_registers = (context + core::mem::offset_of!(libc::mcontext_t, gregs)) as i32;
    let frame_xstate = (context + core::mem::offset_of!(libc::mcontext_t, fpregs)) as i32;
    // rdi the signal, rsi its information, rdx the context.
    a.mov(r12, rdx)?;
    a.mov(r13, rsi)?;
    a.mov(rbx, BLOCK)?;
    a.mov(
        qword_ptr(rbx + field(core::mem::offset_of!(Record, signal))),
        rdi,
    )?;
    a.mov(eax, dword_ptr(r13 + info_code))?;
    a.mov(
        qword_ptr(rbx + field(core::mem::offset_of!(Record, code))),
        rax,
    )?;
    a.mov(rax, qword_ptr(r13 + info_address))?;
    a.mov(
        qword_ptr(rbx + field(core::mem::offset_of!(Record, address))),
        rax,
    )?;
    let selectors = core::mem::offset_of!(Record, selectors);
    for (n, segment) in [ds, es, fs, gs].into_iter().enumerate() {
        a.mov(rax, segment)?;
        a.mov(qword_ptr(rbx + field(selectors + n * WORD as usize)), rax)?;
    }
    a.rdfsbase(rax)?;
    a.mov(
        qword_ptr(rbx + field(core::mem::offset_of!(Record, fs_base))),
        rax,
    )?;
    a.lea(rsi, qword_ptr(r12 + frame_registers))?;
    a.lea(
        rdi,
        qword_ptr(rbx + field(core::mem::offset_of!(Record, registers))),
    )?;
    a.mov(ecx, FRAME_REGISTER_BYTES as u32)?;
    a.rep().movsb()?;
    a.mov(rsi, qword_ptr(r12 + frame_xstate))?;
    a.lea(
        rdi,
        qword_ptr(rbx + field(core::mem::offset_of!(Record, xstate))),
    )?;
    a.mov(ecx, XSTATE_BYTES as u32)?;
    a.rep().movsb()?;
    for (from, bytes) in [
        (BLOCK + RECORD_AT, size_of::<Record>() as u64),
        (DATA, DATA_BYTES),
        (AREA, size_of::<RegisterArea>() as u64),
    ] {
        system_call(&mut a, libc::SYS_write, &[STANDARD_OUTPUT, from, bytes])?;
    }
    system_call(&mut a, libc::SYS_exit_group, &[0])?;
    a.assemble(HANDLER)
}
