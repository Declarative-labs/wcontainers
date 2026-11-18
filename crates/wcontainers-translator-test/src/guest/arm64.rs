//! ARM64's part of the harness: its forms from Arm's list, its value strategies, its state as the
//! reference's fields, and what its architecture leaves open.

mod assemble;
mod corpora;

use super::*;
use crate::layout::{CODE, DATA, DATA_BYTES, DATA_WRITABLE};
use crate::reference::Target;
use proptest::prelude::*;
use std::sync::OnceLock;
use wcontainers_abi::RegisterBlock;
use wcontainers_arm64::forms::{self, Form};
use wcontainers_arm64::{Arm64, decode};
use zerocopy::{FromBytes, IntoBytes};

/// Fields: x0 to x30, sp, pc, NZCV, tpidr_el0, v0 to v31, FPSR, FPCR.
const SP: usize = 31;
const PC: usize = 32;
const NZCV: usize = 33;
const TPIDR: usize = 34;
const V: usize = 35;
const FPSR: usize = 67;
const FPCR: usize = 68;

impl GuestTest for Arm64 {
    const REFERENCE: Target = Target {
        rust_target: "aarch64-unknown-linux-musl",
        docker_platform: "linux/arm64",
        reference_host: "WC_REFERENCE_AARCH64",
        fields: &[
            "x0",
            "x1",
            "x2",
            "x3",
            "x4",
            "x5",
            "x6",
            "x7",
            "x8",
            "x9",
            "x10",
            "x11",
            "x12",
            "x13",
            "x14",
            "x15",
            "x16",
            "x17",
            "x18",
            "x19",
            "x20",
            "x21",
            "x22",
            "x23",
            "x24",
            "x25",
            "x26",
            "x27",
            "x28",
            "x29",
            "x30",
            "sp",
            "pc",
            "nzcv",
            "tpidr_el0",
            "v0",
            "v1",
            "v2",
            "v3",
            "v4",
            "v5",
            "v6",
            "v7",
            "v8",
            "v9",
            "v10",
            "v11",
            "v12",
            "v13",
            "v14",
            "v15",
            "v16",
            "v17",
            "v18",
            "v19",
            "v20",
            "v21",
            "v22",
            "v23",
            "v24",
            "v25",
            "v26",
            "v27",
            "v28",
            "v29",
            "v30",
            "v31",
            "fpsr",
            "fpcr",
        ],
    };
    const C_TARGET: &'static str = "aarch64-linux-musl";
    // zig sets the CPU's features itself; this is how it leaves out floating point and vectors.
    const GENERAL_ONLY: &'static [&'static str] = &["-mcpu=generic-neon-fp_armv8"];
    const NAME: &'static str = "arm64";
    const ELF_MACHINE: object::elf::Machine = object::elf::EM_AARCH64;
    const MUSL_LOADER: &'static str = "/lib/ld-musl-aarch64.so.1";

    fn relocation(kind: object::elf::RelocationType) -> Relocation {
        use object::elf;
        match kind {
            elf::R_AARCH64_RELATIVE => Relocation::Relative,
            elf::R_AARCH64_ABS64 | elf::R_AARCH64_GLOB_DAT | elf::R_AARCH64_JUMP_SLOT => {
                Relocation::Absolute
            }
            _ => Relocation::Other,
        }
    }

    fn disassemble(code: &[u8]) -> String {
        format!("{:?}", decode(word(code)))
    }

    const DEFAULT_OFFER: () = ();

    type Form = Form;

    fn forms() -> &'static [Form] {
        static COVERED: OnceLock<Vec<Form>> = OnceLock::new();
        COVERED.get_or_init(|| {
            wcontainers_arm64::forms::all()
                .iter()
                .filter(|f| covered(f))
                .cloned()
                .collect()
        })
    }

    fn form_name(form: &Form) -> &str {
        &form.name
    }

    fn cases(form: &Form) -> BoxedStrategy<Case> {
        cases(form.clone()).boxed()
    }

    fn no_form(seed: u64) -> BoxedStrategy<Case> {
        any::<u32>()
            .prop_filter("in a form", |&word| {
                !wcontainers_arm64::forms::all()
                    .iter()
                    .any(|f| word & f.mask == f.value)
            })
            .prop_map(move |word| {
                let mut registers = [0u64; 34];
                registers[31] = DATA + 0x1000;
                case(word, registers, [0; 32], [0; 2], false, seed)
            })
            .boxed()
    }

    fn hand_cases() -> Vec<Designed> {
        hand_cases()
    }

    const FILLER: u8 = crate::layout::aarch64::FILLER;
    const CODE_SPAN: u64 = crate::layout::aarch64::CODE_SPAN;

    const PC_FIELD: usize = PC;
    const SP_FIELD: usize = SP;
    const GENERAL_FIELDS: core::ops::Range<usize> = 0..31;

    fn to_state(thread: &[u8]) -> State {
        let block = RegisterBlock::read_from_bytes(&thread[..size_of::<RegisterBlock>()])
            .expect("a register block");
        let mut state: Vec<u128> = block.x.iter().map(|&r| r.into()).collect();
        state.extend([block.sp, block.pc, block.pstate, block.tpidr_el0].map(u128::from));
        state.extend(
            block
                .v
                .map(|[low, high]| u128::from(high) << 64 | u128::from(low)),
        );
        state.extend([block.fpsr, block.fpcr].map(u128::from));
        State(state)
    }

    fn from_state(state: &State, thread: &mut [u8]) {
        let s = &state.0;
        let mut block = RegisterBlock::ZERO;
        for (to, from) in block.x.iter_mut().zip(s) {
            *to = *from as u64;
        }
        block.sp = s[SP] as u64;
        block.pc = s[PC] as u64;
        block.pstate = s[NZCV] as u64;
        block.tpidr_el0 = s[TPIDR] as u64;
        for (to, from) in block.v.iter_mut().zip(&s[V..V + 32]) {
            *to = [*from as u64, (*from >> 64) as u64];
        }
        block.fpsr = s[FPSR] as u32;
        block.fpcr = s[FPCR] as u32;
        thread[..size_of::<RegisterBlock>()].copy_from_slice(block.as_bytes());
    }

    fn track(thread: &mut [u8]) {
        let at = wcontainers_arm64::FLOAT_TRACKING;
        thread[at..at + 4].copy_from_slice(&1u32.to_le_bytes());
    }

    fn reconcile(case: &Case, expected: &mut Outcome, actual: &Outcome) {
        unknown_after_fault(case, expected, actual);
        // Without the flags kept and with FPCR zero, the cumulative exception flags are not kept: FPSR keeps
        // what it had there.
        let writes_fpsr = matches!(
            decode(word(case.code.code())),
            wcontainers_arm64::Instruction::WriteSystem {
                register: wcontainers_arm64::SystemRegister::Fpsr,
                ..
            }
        );
        if !case.tracking && case.state.0[FPCR] == 0 && !writes_fpsr {
            const CUMULATIVE: u128 = 0x9f;
            let fpsr = &mut expected.state.0[FPSR];
            *fpsr = (*fpsr & !CUMULATIVE) | (case.state.0[FPSR] & CUMULATIVE);
        }
    }

    fn reference_stop(signal: &Signal, _length: u8) -> Stop {
        const SIGILL: u32 = 4;
        const SIGBUS: u32 = 7;
        const SIGTRAP: u32 = 5;
        const SIGSEGV: u32 = 11;
        const SIGSYS: u32 = 31;
        const BUS_ADRALN: u32 = 1;
        let pc = signal.pc;
        match signal.number {
            SIGILL if pc == CODE => Stop::Illegal,
            SIGILL => Stop::Landed(pc),
            SIGBUS if signal.code == BUS_ADRALN => Stop::Alignment(signal.address),
            // Control reached an address that cannot be run because nothing is mapped there. Turning that
            // into the right signal is the CPU side's job when it finds no code there.
            SIGSEGV if signal.address == pc && pc != CODE => Stop::Landed(pc),
            SIGSEGV => Stop::MemoryFault(signal.address),
            SIGTRAP => Stop::Breakpoint(signal.code),
            SIGSYS => Stop::Syscall,
            number => Stop::Other(number, signal.address),
        }
    }

    fn breakpoint_code(_detail: u16) -> u32 {
        // TRAP_BRKPT, for every `brk`.
        1
    }

    /// HWCAP: fp, asimd, cpuid and atomics.
    const AUXILIARY: &'static [(u64, u64)] = &[
        (crate::layout::AT_HWCAP, 0x183),
        (crate::layout::AT_HWCAP2, 0),
    ];

    const STAND_IN: StandIn = StandIn {
        write: 64,
        writev: 66,
        exit: 93,
        exit_group: 94,
        brk: 214,
    };

    fn excluded(code: &[u8]) -> Option<&'static str> {
        let word = word(code);
        forms::form(word).and_then(|form| excluded(form, word))
    }

    /// Everything but x16 to x18 (scratch for loops, recursion and jumps), x29 (the area) and x30.
    const MODEL: Model = Model {
        registers: &[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 19, 20, 21, 22, 23, 24, 25, 26,
            27, 28,
        ],
        syscall_sets: &[0, 1, 2, 8],
        kept: 28,
    };

    fn assemble(program: &Program) -> Blob {
        assemble::assemble(program)
    }

    fn smoke() -> Smoke {
        corpora::smoke()
    }

    fn checks() -> Vec<Check> {
        corpora::checks()
    }

    fn changes() -> Vec<Change> {
        corpora::changes()
    }
}

fn word(code: &[u8]) -> u32 {
    u32::from_le_bytes(code[..4].try_into().expect("a 4 byte instruction"))
}

/// A case from x0 to x30, sp, the flags and the thread pointer, the vector registers, FPSR and FPCR.
fn case(
    word: u32,
    registers: [u64; 34],
    vectors: [u128; 32],
    float: [u32; 2],
    tracking: bool,
    seed: u64,
) -> Case {
    let mut state: Vec<u128> = registers[..32].iter().map(|&r| r.into()).collect();
    state.push(CODE.into());
    state.extend(registers[32..].iter().map(|&r| u128::from(r)));
    state.extend(vectors);
    state.extend(float.map(u128::from));
    Case {
        code: Encoded::new(&word.to_le_bytes()),
        state: State(state),
        tracking,
        seed,
    }
}

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
fn cases(form: Form) -> impl Strategy<Value = Case> {
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
                let float = [
                    fpsr & wcontainers_arm64::FPSR_BITS & if pick & 1 == 0 { 0 } else { !0 },
                    fpcr,
                ];
                case(
                    word,
                    registers,
                    v.try_into().unwrap(),
                    float,
                    tracking,
                    seed,
                )
            },
        )
        .prop_filter("excluded encodings", move |case| {
            let word = word(case.code.code());
            kept.presented(word) && excluded(&kept, word).is_none()
        })
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
    let instruction = decode(word(case.code.code()));
    if let I::SimdMemory {
        load,
        rt,
        registers,
        ..
    } = instruction
    {
        if load {
            for k in 0..registers {
                let n = V + usize::from((rt + k) % 32);
                expected.state.0[n] = actual.state.0[n];
            }
        } else {
            expected.changed.clear();
        }
        return;
    }
    let (general, vector) = match instruction {
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
        expected.state.0[n] = actual.state.0[n];
    }
    for n in vector.into_iter().flatten() {
        let n = V + usize::from(n);
        expected.state.0[n] = actual.state.0[n];
    }
}

/// The hand-made cases: alignment faults, vector registers reaching the CPU, the float step instructions'
/// edges, and what we answer by design.
fn hand_cases() -> Vec<Designed> {
    let mut all = Vec::new();
    let mut add = |name, case, stop, fields: Vec<(usize, u128)>, compared| {
        all.push(Designed {
            name,
            case,
            stop,
            fields,
            compared,
        })
    };
    // Alignment faults, so the cases reach them.
    let stack = DATA + 0x1008;
    let target = CODE + 0x102;
    for (name, word, sp, stop) in [
        ("ldr x0, [sp]", 0xf940_03e0, stack, Stop::Alignment(stack)),
        (
            "stp x29, x30, [sp, #-16]!",
            0xa9bf_7bfd,
            stack,
            Stop::Alignment(stack),
        ),
        (
            "ldr x0, [sp, x1]",
            0xf861_6be0,
            stack + 3,
            Stop::Alignment(stack + 3),
        ),
        (
            "ldr x0, [sp, #8], with the stack pointer aligned",
            0xf940_07e0,
            stack + 8,
            Stop::Landed(CODE + 4),
        ),
        (
            "prfm pldl1keep, [sp]: a prefetch is not checked",
            0xf980_03e0,
            stack,
            Stop::Landed(CODE + 4),
        ),
        (
            "add sp, sp, #1: only loads and stores are checked",
            0x9100_07ff,
            stack,
            Stop::Landed(CODE + 4),
        ),
        (
            "br x1, to an address that is not a multiple of 4",
            0xd61f_0020,
            stack + 8,
            Stop::Alignment(target),
        ),
        (
            "rprfm through a misaligned stack pointer: a hint, so nothing",
            0xf8a0_4bf8,
            stack,
            Stop::Landed(CODE + 4),
        ),
    ] {
        let mut registers = [0u64; 34];
        registers[1] = target;
        registers[31] = sp;
        let case = case(word, registers, [0; 32], [0; 2], false, 1);
        add(name, case, Some(stop), vec![], true);
    }
    // The vector registers reach the CPU and come back: add v0.16b, v1.16b, v2.16b.
    let mut vectors = [0u128; 32];
    vectors[1] = 0x0102_0304_0506_0708_090a_0b0c_0d0e_0fff;
    vectors[2] = 0x1010_1010_1010_1010_1010_1010_1010_1002;
    add(
        "add v0.16b, v1.16b, v2.16b",
        case(0x4e22_8420, [0; 34], vectors, [0; 2], false, 1),
        None,
        vec![
            (V, 0x1112_1314_1516_1718_191a_1b1c_1d1e_1f01),
            (V + 1, vectors[1]),
            (V + 2, vectors[2]),
        ],
        true,
    );
    // FRECPS and FRSQRTS of the smallest denormals against infinities, in every shape, with the flags kept
    // and not: the fast path's overflow branch must not turn an exact infinity into not-a-number.
    let lanes32 = |l: [u32; 4]| l.iter().rev().fold(0u128, |v, &x| v << 32 | u128::from(x));
    let lanes64 = |l: [u64; 2]| u128::from(l[1]) << 64 | u128::from(l[0]);
    let (inf32, inf64) = (0x7f80_0000u32, 0x7ff0_0000_0000_0000u64);
    let pairs = [
        (
            lanes32([1, 0x8000_0001, inf32, inf32 | 1 << 31]),
            lanes32([inf32, inf32, 1, 0x8000_0001]),
        ),
        (
            lanes32([0x8000_0001, 1, 0x0000_0002, 0x7f7f_ffff]),
            lanes32([inf32 | 1 << 31, inf32 | 1 << 31, inf32, 0x7f7f_ffff]),
        ),
        (
            lanes64([1, inf64]),
            lanes64([inf64 | 1 << 63, 0x8000_0000_0000_0001]),
        ),
        (
            lanes64([0x8000_0000_0000_0001, 0x7fef_ffff_ffff_ffff]),
            lanes64([inf64, 0x7fef_ffff_ffff_ffff]),
        ),
    ];
    // frsqrts and frecps v0, v1, v2: 2s, 4s, 2d, s, d.
    for word in [
        0x0ea2_fc20u32,
        0x4ea2_fc20,
        0x4ee2_fc20,
        0x5ea2_fc20,
        0x5ee2_fc20,
        0x0e22_fc20,
        0x4e22_fc20,
        0x4e62_fc20,
        0x5e22_fc20,
        0x5e62_fc20,
    ] {
        for (a, b) in pairs {
            for tracking in [false, true] {
                let mut vectors = [0u128; 32];
                vectors[1] = a;
                vectors[2] = b;
                let mut registers = [0u64; 34];
                registers[31] = DATA + 0x1000;
                let case = case(word, registers, vectors, [0; 2], tracking, 1);
                add("frecps or frsqrts at an edge", case, None, vec![], true);
            }
        }
    }
    // What we answer by design rather than as the reference CPU does, and the faults of the CPU we present
    // where the reference CPU has more features.
    let at = |word: u32, x1: u64, x2: u64| {
        let mut registers = [0u64; 34];
        registers[1] = x1;
        registers[2] = x2;
        registers[31] = DATA + 0x1000;
        case(word, registers, [0; 32], [0; 2], false, 1)
    };
    use wcontainers_arm64::{CACHE_TYPE, MAIN_ID, ZERO_BLOCK_ID};
    for (name, word, value) in [
        ("mrs x0, cntfrq_el0", 0xd53b_e000, 1_000_000_000),
        ("mrs x0, ctr_el0", 0xd53b_0020, CACHE_TYPE),
        ("mrs x0, dczid_el0", 0xd53b_00e0, ZERO_BLOCK_ID),
        ("mrs x0, midr_el1", 0xd538_0000, MAIN_ID),
        ("mrs x0, mpidr_el1", 0xd538_00a0, 0x8000_0000),
        ("mrs x0, id_aa64isar0_el1", 0xd538_0600, 0x0021_0000),
        ("mrs x0, id_aa64pfr0_el1", 0xd538_0400, 0x11),
    ] {
        let stop = Some(Stop::Landed(CODE + 4));
        add(name, at(word, 0, 0), stop, vec![(0, value.into())], false);
    }
    let landed = Some(Stop::Landed(CODE + 4));
    add(
        "mrs x0, cntvct_el0 counts up",
        at(0xd53b_e040, 0, 0),
        landed,
        vec![],
        false,
    );
    add(
        "msr fpcr, x1 keeps only the bits we present",
        at(0xd51b_4401, u64::MAX, 0),
        landed,
        vec![(FPCR, wcontainers_arm64::FPCR_BITS.into())],
        false,
    );
    // Unaligned atomics, load-acquire and exclusives fault, as a CPU without FEAT_LSE2 does.
    let unaligned = DATA + 0x101;
    for (name, word) in [
        ("ldadd x2, x0, [x1], unaligned", 0xf822_0020u32),
        ("cas x0, x2, [x1], unaligned", 0xc8a0_7c22),
        ("ldar x0, [x1], unaligned", 0xc8df_fc20),
        ("stlr x0, [x1], unaligned", 0xc89f_fc20),
        ("ldxr x0, [x1], unaligned", 0xc85f_7c20),
        ("ldaxp x0, x3, [x1], unaligned", 0xc87f_8c20),
    ] {
        let stop = Some(Stop::Alignment(unaligned));
        add(name, at(word, unaligned, 1), stop, vec![], false);
    }
    // Aligned, they work.
    add(
        "ldadd x2, x0, [x1], aligned",
        at(0xf822_0020, DATA + 0x100, 1),
        landed,
        vec![],
        false,
    );
    all
}
