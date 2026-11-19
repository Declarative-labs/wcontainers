//! Layer 1: one instruction at a time, on the real CPU and through the translator at look-ahead none, with
//! every register, flag and touched byte compared.

use crate::layout::*;
use crate::reference::{Cases, Landed};
use crate::runner::{Machine, Stopped, Syscalls};
use proptest::prelude::*;
use wcontainers_abi::{Access, Exit, Permissions, RegisterBlock};
use wcontainers_arm64::forms::Form;
use wcontainers_translator::{LookAhead, contract};
use zerocopy::{FromBytes, IntoBytes};

/// Every form of the CPU we present: one some word of which is presented. Forms whose condition limits
/// fields are found by trying words.
pub fn covered(form: &Form) -> bool {
    (0..256u32).any(|i| form.presented(form.word(i.wrapping_mul(0x9e37_79b9) ^ i << 5)))
}

/// System registers layer 1 picks from for `mrs` and `msr`, as o0:op1:CRn:CRm:op2: NZCV, TPIDR_EL0, FPCR,
/// FPSR and TPIDRRO_EL0. Others come from random fields.
const SYSTEM_REGISTERS: [u32; 5] = [
    (1 << 14) | (3 << 11) | (4 << 7) | (2 << 3),
    (1 << 14) | (3 << 11) | (13 << 7) | 2,
    (1 << 14) | (3 << 11) | (4 << 7) | (4 << 3),
    (1 << 14) | (3 << 11) | (4 << 7) | (4 << 3) | 1,
    (1 << 14) | (3 << 11) | (13 << 7) | 3,
];

/// Narrow a word toward the system registers that matter, most of the time.
fn restrict(form: &Form, word: u32, pick: u8) -> u32 {
    if form.group == "control/systemmove" && usize::from(pick) < SYSTEM_REGISTERS.len() {
        return (word & !(0x7fff << 5)) | (SYSTEM_REGISTERS[usize::from(pick)] << 5);
    }
    word
}

/// o0:op1:CRn:CRm:op2 of a system register move.
fn system_register(word: u32) -> u32 {
    (word >> 5) & 0x7fff
}

/// Why a word of a form we present is an instruction only of a feature we do not offer, if it is: Arm's
/// decode makes it undefined without the feature. We raise illegal instruction for it (counted as
/// unsupported); the reference CPU has these features and runs it.
pub fn of_feature_not_offered(form: &Form, word: u32) -> Option<&'static str> {
    let field = |name| form.field(word, name);
    let shift_group = form.group.ends_with("asimdshf") || form.group.ends_with("asisdshf");
    let fp_conversion = ["SCVTF", "UCVTF", "FCVTZS", "FCVTZU"]
        .iter()
        .any(|stem| form.name.starts_with(&format!("{stem}_")));
    if shift_group && fp_conversion && field("immh").is_some_and(|immh| immh >> 1 == 1) {
        return Some("half precision, which needs FEAT_FP16");
    }
    if form.name.starts_with("PMULL_") && field("size") == Some(3) {
        return Some("64 bit pmull, which needs FEAT_PMULL");
    }
    if form.group == "control/systemmove" {
        let r = system_register(word);
        let (o0, op1, crn, crm, op2) =
            (r >> 14, (r >> 11) & 7, (r >> 7) & 15, (r >> 3) & 15, r & 7);
        // SSBS, DIT, TCO and CNTVCTSS_EL0; RNDR and RNDRRS (read only).
        let read = field("L") == Some(1);
        if o0 == 1 && matches!((op1, crn, crm, op2), (3, 4, 2, 5..=7) | (3, 14, 0, 6))
            || o0 == 1 && read && matches!((op1, crn, crm, op2), (3, 2, 4, 0 | 1))
        {
            return Some("a register of a feature we do not offer");
        }
    }
    if form.name == "MSR_SI_pstate"
        && field("op1") == Some(3)
        && matches!(field("op2"), Some(1..=4))
    {
        return Some("SSBS, DIT, the streaming mode and TCO, of features we do not offer");
    }
    if form.group == "control/systeminstrs"
        && field("L") == Some(0)
        && (field("op1"), field("CRn")) == (Some(3), Some(7))
        && matches!(
            (field("CRm").unwrap_or(0), field("op2").unwrap_or(0)),
            (4, 3 | 4) | (10 | 14, 3 | 5) | (12 | 13, 1 | 3 | 5)
        )
    {
        return Some("cache operations of features we do not offer");
    }
    None
}

/// Why a word is left out of the comparison, if it is. Each is an encoding the architecture leaves
/// CONSTRAINED UNPREDICTABLE, a branch to itself (which never finishes), an instruction or a hint whose
/// meaning depends on features the reference CPU has and the CPU we present does not, or a register we
/// answer by design.
pub fn excluded(form: &Form, word: u32) -> Option<&'static str> {
    let field = |name| form.field(word, name);
    let writeback = ["immpre", "immpost", "pair_pre", "pair_post"]
        .iter()
        .any(|kind| form.group.ends_with(kind));
    let rn = field("Rn");
    let loads_pair = form.group.contains("pair") && field("L") == Some(1);
    let integer = field("VR") != Some(1);
    if writeback
        && integer
        && rn != Some(31)
        && (rn == field("Rt") || (rn.is_some() && rn == field("Rt2")))
    {
        return Some("write back to a transfer register");
    }
    if loads_pair && field("Rt") == field("Rt2") {
        return Some("load pair into one register");
    }
    let offset = field("imm26").or(field("imm19")).or(field("imm14"));
    if form.group.starts_with("control/") && offset == Some(0) {
        return Some("branch to itself");
    }
    let exclusive = form.group.starts_with("ldst/ldstexcl");
    if exclusive && field("L") == Some(0) {
        let rs = field("Rs");
        let pair = form.group.ends_with('p');
        if rs == field("Rt") || (pair && rs == field("Rt2")) || (rs == rn && rn != Some(31)) {
            return Some("store exclusive whose status register is also transferred or the base");
        }
    }
    if exclusive
        && field("L") == Some(1)
        && form.group.ends_with('p')
        && field("Rt") == field("Rt2")
    {
        return Some("load pair into one register");
    }
    if let Some(reason) = of_feature_not_offered(form, word) {
        return Some(reason);
    }
    if form.group == "control/systemmove" {
        let r = system_register(word);
        let (op1, crn, crm, op2) = ((r >> 11) & 7, (r >> 7) & 15, (r >> 3) & 15, r & 7);
        let by_design = matches!((op1, crn, crm, op2), (3, 14, 0, 0 | 2) | (3, 0, 0, 1 | 7))
            || (r >> 14 == 1 && op1 == 0 && crn == 0);
        if by_design {
            return Some("a register we answer by design, not as the reference CPU does");
        }
    }
    if form.group == "control/hints" {
        let (crm, op2) = (field("CRm").unwrap_or(0), field("op2").unwrap_or(0));
        if crm == 1 || crm == 3 || (crm == 0 && op2 == 7) || crm >= 4 {
            return Some("hint with a meaning on CPUs with pointer signing or later features");
        }
    }
    None
}

/// One case: the instruction word, starting registers (x0 to x30, sp, flags, thread pointer), vector
/// registers and the data area's seed.
#[derive(Clone, Debug)]
pub struct Case {
    pub word: u32,
    pub registers: [u64; 34],
    pub vectors: [u128; 32],
    /// FPSR and FPCR.
    pub float: [u32; 2],
    /// The flags are kept exact, as after a program wrote FPSR.
    pub tracking: bool,
    pub seed: u64,
}

fn value() -> impl Strategy<Value = u64> {
    prop_oneof![
        2 => prop::sample::select(vec![
            0,
            1,
            2,
            u64::MAX,
            u64::MAX - 1,
            i64::MAX as u64,
            i64::MIN as u64,
            0x7fff_ffff,
            0x8000_0000,
            0xffff_ffff,
            0x1_0000_0000,
            0xffff_ffff_8000_0000,
        ]),
        2 => (0u64..64).prop_map(|v| v.wrapping_sub(32)),
        2 => data_pointer(),
        1 => (0u64..1024, any::<bool>()).prop_map(|(k, up)| {
            let step = 4 * (k + 1);
            if up { CODE + step } else { CODE - step }
        }),
        3 => any::<u64>(),
    ]
}

/// Single and double precision values that matter: not-a-numbers of both kinds with payloads,
/// infinities, zeros, denormals, the edges of the normal range, halves and integers.
fn float_bits(double: bool) -> impl Strategy<Value = u64> {
    let special: Vec<u64> = if double {
        [
            0.0f64,
            -0.0,
            1.0,
            -1.0,
            0.5,
            1.5,
            2.5,
            -2.5,
            3.0,
            0.1,
            1e300,
            -1e-300,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::MAX,
            f64::MIN_POSITIVE,
            4503599627370496.5,
            9.223_372_036_854_776e18,
            -9.223_372_036_854_776e18,
            1.8446744073709552e19,
            2147483648.0,
            -2147483649.0,
            4294967296.0,
            65504.0,
            65520.0,
            6.1e-5,
            6e-8,
        ]
        .iter()
        .map(|v| v.to_bits())
        .chain([
            0x7ff8_0000_0000_0000,
            0x7ff0_0000_0000_0001,
            0xfff4_0000_0000_dead,
            0x7ffc_0000_beef_0000,
            1,
            0x000f_ffff_ffff_ffff,
            0x8000_0000_0000_0001,
        ])
        .collect()
    } else {
        [
            0.0f32,
            -0.0,
            1.0,
            -1.0,
            0.5,
            1.5,
            2.5,
            -2.5,
            3.0,
            0.1,
            1e30,
            -1e-30,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::MAX,
            f32::MIN_POSITIVE,
            8388608.5,
            2147483648.0,
            -2147483649.0,
            4294967296.0,
            9.223372e18,
            65504.0,
            65520.0,
            6.1e-5,
            6e-8,
        ]
        .iter()
        .map(|v| u64::from(v.to_bits()))
        .chain([
            0x7fc0_0000,
            0x7f80_0001,
            0xffa0_dead,
            0x7fe0_beef,
            1,
            0x007f_ffff,
            0x8000_0001,
        ])
        .collect()
    };
    let width = if double { 64 } else { 32 };
    prop_oneof![
        3 => prop::sample::select(special),
        1 => any::<u64>().prop_map(move |v| if width == 64 { v } else { v & 0xffff_ffff }),
        // Exponents near the denormal edge.
        1 => (any::<u64>(), any::<bool>()).prop_map(move |(v, neg)| {
            let (fraction, exponent, sign) = if width == 64 { (52, 1u64, 63) } else { (23, 1, 31) };
            (v & ((1 << fraction) - 1)) | ((v >> 60) % 2 * exponent) << fraction | u64::from(neg) << sign
        }),
    ]
}

fn vector_value() -> impl Strategy<Value = u128> {
    prop_oneof![
        2 => prop::collection::vec(float_bits(false), 4).prop_map(|v| {
            v.iter().enumerate().fold(0u128, |all, (i, &x)| all | u128::from(x) << (32 * i))
        }),
        2 => prop::collection::vec(float_bits(true), 2)
            .prop_map(|v| u128::from(v[1]) << 64 | u128::from(v[0])),
        // Half precision lanes.
        1 => prop::collection::vec(prop_oneof![
            Just(0x7c00u64), Just(0xfc00), Just(0x7e01), Just(0x7d00), Just(0x0001), Just(0x03ff),
            Just(0x3c00), Just(0x7bff), Just(0x8000), any::<u16>().prop_map(u64::from)
        ], 8).prop_map(|v| v.iter().enumerate().fold(0u128, |all, (i, &x)| all | u128::from(x) << (16 * i))),
        2 => any::<u128>(),
        2 => (value(), value()).prop_map(|(low, high)| u128::from(high) << 64 | u128::from(low)),
        // Bytes from a few values, so elements of different registers are often equal.
        2 => prop::array::uniform16(prop::sample::select(vec![0u8, 1, 0x7f, 0x80, 0xff]))
            .prop_map(u128::from_le_bytes),
    ]
}

fn data_pointer() -> impl Strategy<Value = u64> {
    (0..DATA_BYTES, any::<bool>()).prop_map(|(at, align)| DATA + if align { at & !7 } else { at })
}

/// FPCR and whether the flags are kept: half the cases in the fast mode (FPCR zero, flags not kept), the
/// rest with flags kept, and FPCR's bits random in half of those.
fn float_control() -> impl Strategy<Value = (u32, bool)> {
    prop_oneof![
        2 => Just((0, false)),
        1 => Just((0, true)),
        1 => (any::<u32>(), any::<bool>())
            .prop_map(|(bits, tracking)| (bits & wcontainers_arm64::FPCR_BITS, tracking)),
    ]
}

/// Cases for one form.
pub fn cases(form: Form) -> impl Strategy<Value = Case> {
    let kept = form.clone();
    let registers = prop::collection::vec(value(), 31);
    // The stack pointer is mostly a multiple of 16, as the ABI keeps it, and sometimes not, which faults
    // when it is a load or store's base.
    let stack = (
        DATA_WRITABLE / 16 / 4..DATA_WRITABLE / 16 * 3 / 4,
        prop_oneof![3 => Just(0u64), 1 => 1u64..16],
    )
        .prop_map(|(k, misaligned)| DATA + 16 * k + misaligned);
    let small = (0u64..128).prop_map(|v| v.wrapping_sub(64));
    (
        any::<u32>(),
        any::<bool>(),
        registers,
        stack,
        0u64..16,
        any::<u64>(),
        any::<u64>(),
        data_pointer(),
        small,
        prop::collection::vec(vector_value(), 32),
        (0u8..8, float_control(), any::<u32>()),
    )
        .prop_map(
            move |(
                free,
                _,
                x,
                sp,
                flags,
                thread_pointer,
                seed,
                base,
                index,
                v,
                (pick, (fpcr, tracking), fpsr),
            )| {
                let word = restrict(&form, form.word(free), pick);
                let mut registers = [0u64; 34];
                registers[..31].copy_from_slice(&x);
                registers[31] = sp;
                registers[32] = flags << 28;
                registers[33] = thread_pointer;
                // Loads and stores mostly reach the data area: the base points into it and an index is small.
                // Elsewhere Rn and Rm may name general or vector registers.
                if form.group.starts_with("ldst/") {
                    if let Some(rn) = form.field(word, "Rn").filter(|&r| r < 31) {
                        registers[rn as usize] = base;
                    }
                    if let Some(rm) = form.field(word, "Rm").filter(|&r| r < 31) {
                        registers[rm as usize] = index;
                    }
                    // The reference CPU runs unaligned atomics (FEAT_LSE2) and exclusives, where the CPU
                    // we present faults: keep their addresses aligned here; the fault is tested on its own.
                    let atomic = [
                        "memop",
                        "comswap",
                        "comswappr",
                        "ldstord",
                        "ldstexclr",
                        "ldstexclp",
                    ]
                    .iter()
                    .any(|g| form.group.ends_with(g));
                    if let Some(rn) = form.field(word, "Rn").filter(|&r| r < 31 && atomic) {
                        registers[rn as usize] &= !15;
                    }
                }
                // Only the FPCR bits we present: the reference CPU has more.
                if form.name.starts_with("MSR_SR")
                    && system_register(word) == SYSTEM_REGISTERS[2]
                    && let Some(rt) = form.field(word, "Rt").filter(|&r| r < 31)
                {
                    registers[rt as usize] &= u64::from(wcontainers_arm64::FPCR_BITS);
                }
                Case {
                    word,
                    registers,
                    vectors: v.try_into().unwrap(),
                    float: [
                        fpsr & wcontainers_arm64::FPSR_BITS & if pick & 1 == 0 { 0 } else { !0 },
                        fpcr,
                    ],
                    tracking,
                    seed,
                }
            },
        )
        .prop_filter("excluded encodings", move |case| {
            kept.presented(case.word) && excluded(&kept, case.word).is_none()
        })
}

/// What the translator did with a case, in the reference's terms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// Where control went or why the thread stopped, as the reference reports it.
    pub stop: Stop,
    pub registers: [u64; 35],
    pub vectors: [u128; 32],
    /// FPSR and FPCR.
    pub float: [u32; 2],
    pub changed: Vec<(u32, u8)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    /// Control reached this address and the instruction finished.
    Landed(u64),
    Illegal,
    Breakpoint,
    Syscall,
    MemoryFault(u64),
    /// SIGBUS with BUS_ADRALN at this address.
    Alignment(u64),
    /// Anything that has no counterpart on the other side.
    Other(u32, u64),
}

/// The reference's result as an [`Outcome`].
pub fn reference_outcome(landed: &Landed) -> Outcome {
    const SIGILL: u32 = 4;
    const SIGBUS: u32 = 7;
    const SIGTRAP: u32 = 5;
    const SIGSEGV: u32 = 11;
    const SIGSYS: u32 = 31;
    const BUS_ADRALN: u32 = 1;
    let pc = landed.registers[32];
    let stop = match landed.signal {
        SIGILL if pc == CODE => Stop::Illegal,
        SIGILL => Stop::Landed(pc),
        SIGBUS if landed.code == BUS_ADRALN => Stop::Alignment(landed.address),
        // Control reached an address that cannot be run because nothing is mapped there. Turning that into
        // the right signal is the CPU side's job when it finds no code there.
        SIGSEGV if landed.address == pc && pc != CODE => Stop::Landed(pc),
        SIGSEGV => Stop::MemoryFault(landed.address),
        SIGTRAP => Stop::Breakpoint,
        SIGSYS => Stop::Syscall,
        signal => Stop::Other(signal, landed.address),
    };
    Outcome {
        stop,
        registers: landed.registers,
        vectors: landed.vectors,
        float: landed.float,
        changed: landed.changed.clone(),
    }
}

/// Memory size of the translator's machine: everything up to the end of the code region.
const MEMORY: u64 = CODE + CODE_SPAN;

/// The data area, the code region and what each allows.
fn regions() -> [(std::ops::Range<u64>, Permissions); 3] {
    [
        (
            DATA..DATA + DATA_WRITABLE,
            Permissions::READ | Permissions::WRITE,
        ),
        (DATA + DATA_WRITABLE..DATA + DATA_BYTES, Permissions::READ),
        (
            CODE - CODE_SPAN..CODE + CODE_SPAN,
            Permissions::READ | Permissions::EXECUTE,
        ),
    ]
}

/// The data area's contents at the start of a case.
fn start_data(case: &Case) -> Vec<u8> {
    let mut data = vec![0u8; DATA_BYTES as usize];
    fill(case.seed, &mut data);
    data
}

/// The registers at the start of a case.
fn start_registers(case: &Case) -> RegisterBlock {
    let mut registers = RegisterBlock::ZERO;
    registers.x.copy_from_slice(&case.registers[..31]);
    registers.sp = case.registers[31];
    registers.pc = CODE;
    registers.pstate = case.registers[32];
    registers.tpidr_el0 = case.registers[33];
    for (to, from) in registers.v.iter_mut().zip(case.vectors) {
        *to = [from as u64, (from >> 64) as u64];
    }
    registers.fpsr = case.float[0];
    registers.fpcr = case.float[1];
    registers
}

/// An outcome from how the translated case stopped, its registers after and the data bytes it changed.
fn outcome(stop: Stop, after: &RegisterBlock, changed: Vec<(u32, u8)>) -> Outcome {
    let mut out = [0u64; 35];
    out[..31].copy_from_slice(&after.x);
    out[31] = after.sp;
    out[32] = after.pc;
    out[33] = after.pstate;
    out[34] = after.tpidr_el0;
    Outcome {
        stop,
        registers: out,
        vectors: after
            .v
            .map(|[low, high]| u128::from(high) << 64 | u128::from(low)),
        float: [after.fpsr, after.fpcr],
        changed,
    }
}

/// Run a case through the translator.
pub fn translated(case: &Case) -> Outcome {
    // The table covers the case's one instruction only, so control that lands anywhere else stops there.
    let mut machine = Machine::new(MEMORY, CODE, 1, Syscalls::Stop);
    for (range, permissions) in regions() {
        machine.set_permissions(range, permissions);
    }
    let data = start_data(case);
    machine.write(DATA, &data);
    machine.load(&case.word.to_le_bytes(), 0..4, CODE, &[0], LookAhead::None);
    machine.set_registers(&start_registers(case));
    if case.tracking {
        machine.track_float_flags();
    }
    let (stopped, after) = machine.run();
    let stop = match stopped {
        Stopped::NoCode(address) => Stop::Landed(address),
        // Control reached a page that does not allow running code.
        Stopped::Exit(Exit::MemoryFault {
            address,
            access: Access::Execute,
        }) => Stop::Landed(address),
        Stopped::Exit(Exit::IllegalInstruction) => Stop::Illegal,
        Stopped::Exit(Exit::Breakpoint { .. }) => Stop::Breakpoint,
        Stopped::Exit(Exit::Syscall(_)) => Stop::Syscall,
        Stopped::Exit(Exit::MemoryFault { address, .. }) => Stop::MemoryFault(address),
        Stopped::Exit(Exit::AlignmentFault { address }) => Stop::Alignment(address),
        other => panic!("no counterpart on the reference: {other:?}"),
    };
    let mut now = vec![0u8; DATA_BYTES as usize];
    machine.read(DATA, &mut now);
    let changed = now
        .iter()
        .zip(&data)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, (a, _))| (i as u32, *a))
        .collect();
    outcome(stop, &after, changed)
}

/// Cases one Chrome page runs; a page given several thousand ran out of memory.
const CASES_A_PAGE: usize = 1000;

/// Run cases through the translator in Chrome, [`CASES_A_PAGE`] to a page.
pub fn translated_in_chrome(cases: &[Case]) -> Vec<Outcome> {
    cases
        .chunks(CASES_A_PAGE)
        .flat_map(translated_in_one_page)
        .collect()
}

fn translated_in_one_page(cases: &[Case]) -> Vec<Outcome> {
    let states: Vec<crate::browser::StateCase> = cases
        .iter()
        .map(|case| {
            let mut state = vec![0u8; contract::thread_state_size::<wcontainers_arm64::Arm64>()];
            let registers = start_registers(case);
            state[..size_of::<RegisterBlock>()].copy_from_slice(registers.as_bytes());
            if case.tracking {
                state[wcontainers_arm64::FLOAT_TRACKING..wcontainers_arm64::FLOAT_TRACKING + 4]
                    .copy_from_slice(&1u32.to_le_bytes());
            }
            crate::browser::StateCase {
                word: case.word,
                state,
                places: vec![start_data(case)],
            }
        })
        .collect();
    let address = |value: &serde_json::Value| -> u64 {
        value
            .as_str()
            .expect("an address")
            .parse()
            .expect("a number")
    };
    crate::browser::cases(
        CODE,
        MEMORY,
        &regions(),
        std::slice::from_ref(&(DATA..DATA + DATA_BYTES)),
        &states,
    )
    .into_iter()
    .map(|run| {
        let stopped = &run.stopped;
        let stop = match stopped["kind"].as_str() {
            Some("no code") => Stop::Landed(address(&stopped["address"])),
            Some("undefined" | "unsupported") => Stop::Illegal,
            Some("breakpoint") => Stop::Breakpoint,
            Some("syscall") => Stop::Syscall,
            Some("memory fault") if stopped["execute"] == true => {
                Stop::Landed(address(&stopped["address"]))
            }
            Some("memory fault") => Stop::MemoryFault(address(&stopped["address"])),
            Some("alignment") => Stop::Alignment(address(&stopped["detail"])),
            _ => panic!("no counterpart on the reference: {stopped}"),
        };
        let after = RegisterBlock::read_from_bytes(&run.state[..size_of::<RegisterBlock>()])
            .expect("a register block");
        outcome(stop, &after, run.changed)
    })
    .collect()
}

/// When a load or store faults, Arm leaves the registers it loads and the memory it stores UNKNOWN: a CPU
/// may have done part of the access first (this one does, for 32 byte pairs that cross onto a page that
/// faults). Ours does none of it. Take the reference's UNKNOWN parts as ours, provided ours touched no memory.
fn unknown_after_fault(case: &Case, expected: &mut Outcome, actual: &Outcome) {
    use wcontainers_arm64::Instruction as I;
    let Stop::MemoryFault(address) = expected.stop else {
        return;
    };
    if actual.stop != Stop::MemoryFault(address) || !actual.changed.is_empty() {
        return;
    }
    let reg = |r: wcontainers_arm64::Reg| match r {
        wcontainers_arm64::Reg::X(n) => Some(usize::from(n)),
        _ => None,
    };
    if let I::SimdMemory {
        load,
        rt,
        registers,
        ..
    } = wcontainers_arm64::decode(case.word)
    {
        if load {
            for k in 0..registers {
                let n = usize::from((rt + k) % 32);
                expected.vectors[n] = actual.vectors[n];
            }
        } else {
            expected.changed.clear();
        }
        return;
    }
    let (general, vector) = match wcontainers_arm64::decode(case.word) {
        I::Load { rt, .. } => ([reg(rt), None], [None, None]),
        I::LoadPair { rt, rt2, .. } => ([reg(rt), reg(rt2)], [None, None]),
        I::LoadVector { rt, .. } => ([None, None], [Some(rt), None]),
        I::LoadVectorPair { rt, rt2, .. } => ([None, None], [Some(rt), Some(rt2)]),
        I::Store { .. }
        | I::StorePair { .. }
        | I::StoreVector { .. }
        | I::StoreVectorPair { .. } => {
            expected.changed.clear();
            return;
        }
        _ => return,
    };
    for n in general.into_iter().flatten() {
        expected.registers[n] = actual.registers[n];
    }
    for n in vector.into_iter().flatten() {
        expected.vectors[usize::from(n)] = actual.vectors[usize::from(n)];
    }
}

/// Compare one case on both sides; describe the difference if there is one.
pub fn compare(reference: &mut Cases, case: &Case) -> Result<(), String> {
    compare_outcome(reference, case, translated(case))
}

/// Compare a translated outcome of a case with the reference's.
pub fn compare_outcome(reference: &mut Cases, case: &Case, actual: Outcome) -> Result<(), String> {
    let landed = reference.run(
        case.word,
        case.seed,
        &case.registers,
        &case.vectors,
        case.float,
    );
    let mut expected = reference_outcome(&landed);
    unknown_after_fault(case, &mut expected, &actual);
    // Without the flags kept and with FPCR zero, the cumulative exception flags are not kept: FPSR keeps
    // what it had there.
    let writes_fpsr = matches!(
        wcontainers_arm64::decode(case.word),
        wcontainers_arm64::Instruction::WriteSystem {
            register: wcontainers_arm64::SystemRegister::Fpsr,
            ..
        }
    );
    if !case.tracking && case.float[1] == 0 && !writes_fpsr {
        const CUMULATIVE: u32 = 0x9f;
        expected.float[0] = (expected.float[0] & !CUMULATIVE) | (case.float[0] & CUMULATIVE);
    }
    if expected == actual {
        return Ok(());
    }
    let names: Vec<String> = (0..31)
        .map(|i| format!("x{i}"))
        .chain(["sp", "pc", "nzcv", "tpidr_el0"].map(String::from))
        .collect();
    let mut report = format!(
        "word {:#010x} ({:?})\n  reference: {:?}\n  translated: {:?}\n",
        case.word,
        wcontainers_arm64::decode(case.word),
        expected.stop,
        actual.stop
    );
    for (i, name) in names.iter().enumerate() {
        if expected.registers[i] != actual.registers[i] {
            report += &format!(
                "  {name}: reference {:#x}, translated {:#x}, before {:#x}\n",
                expected.registers[i],
                actual.registers[i],
                case.registers
                    .get(if i < 32 { i } else { i - 1 })
                    .copied()
                    .unwrap_or(0)
            );
        }
    }
    for (i, before) in case.vectors.iter().enumerate() {
        if expected.vectors[i] != actual.vectors[i] {
            report += &format!(
                "  v{i}: reference {:#034x}, translated {:#034x}, before {before:#034x}\n",
                expected.vectors[i], actual.vectors[i]
            );
        }
    }
    if expected.float != actual.float {
        report += &format!(
            "  fpsr, fpcr: reference {:#x?}, translated {:#x?}, before {:#x?} (flags kept: {})\n",
            expected.float, actual.float, case.float, case.tracking
        );
    }
    if expected.changed != actual.changed {
        report += &format!(
            "  data: reference {:?}, translated {:?}\n",
            expected.changed, actual.changed
        );
    }
    Err(report)
}
