//! Layer 1: one instruction at a time, on the real CPU and through the translator at look-ahead none, with
//! every register, flag and touched byte compared.

use crate::layout::*;
use crate::reference::{Cases, Landed};
use crate::runner::{Machine, Stopped, Syscalls};
use proptest::prelude::*;
use wcontainers_abi::{Exit, Permissions, RegisterBlock};
use wcontainers_arm64::forms::{self, Form};

/// The SIMD and floating point data processing forms translated so far.
const VECTOR_FORMS: &[&str] = &[
    "MOVI_asimdimm_L_sl",
    "ORR_asimdimm_L_sl",
    "MOVI_asimdimm_L_hl",
    "ORR_asimdimm_L_hl",
    "MOVI_asimdimm_M_sm",
    "MOVI_asimdimm_N_b",
    "FMOV_asimdimm_S_s",
    "MVNI_asimdimm_L_sl",
    "BIC_asimdimm_L_sl",
    "MVNI_asimdimm_L_hl",
    "BIC_asimdimm_L_hl",
    "MVNI_asimdimm_M_sm",
    "MOVI_asimdimm_D_ds",
    "MOVI_asimdimm_D2_d",
    "FMOV_asimdimm_D2_d",
    "AND_asimdsame_only",
    "BIC_asimdsame_only",
    "ORR_asimdsame_only",
    "ORN_asimdsame_only",
    "EOR_asimdsame_only",
    "BSL_asimdsame_only",
    "BIT_asimdsame_only",
    "BIF_asimdsame_only",
    "ADD_asimdsame_only",
    "SUB_asimdsame_only",
    "MUL_asimdsame_only",
    "CMEQ_asimdsame_only",
    "CMTST_asimdsame_only",
    "CMGT_asimdsame_only",
    "CMGE_asimdsame_only",
    "CMHI_asimdsame_only",
    "CMHS_asimdsame_only",
    "CNT_asimdmisc_R",
    "NOT_asimdmisc_R",
    "ADDV_asimdall_only",
    "SMAXV_asimdall_only",
    "SMINV_asimdall_only",
    "UMAXV_asimdall_only",
    "UMINV_asimdall_only",
    "DUP_asimdins_DV_v",
    "DUP_asimdins_DR_r",
    "SMOV_asimdins_W_w",
    "UMOV_asimdins_W_w",
    "INS_asimdins_IR_r",
    "SMOV_asimdins_X_x",
    "UMOV_asimdins_X_x",
    "INS_asimdins_IV_v",
    "EXT_asimdext_only",
    "FMOV_32S_float2int",
    "FMOV_S32_float2int",
    "FMOV_64D_float2int",
    "FMOV_D64_float2int",
    "FMOV_64VX_float2int",
    "FMOV_V64I_float2int",
];

/// Every feature the form's condition names is one we offer.
fn offered(form: &Form) -> bool {
    form.features
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|token| token.starts_with("FEAT_"))
        .all(|feature| forms::OFFERED_FEATURES.contains(&feature))
}

/// The forms layer 1 covers now: the base integer instructions, the SIMD and floating point register
/// loads and stores, and [`VECTOR_FORMS`]. System register moves are limited to the registers
/// [`restrict`] allows. The rest of floating point and vectors, atomics, exclusives, load-acquire and
/// store-release and the other system instructions come with milestone 3.
pub fn covered(form: &Form) -> bool {
    const GROUPS: &[&str] = &[
        "dpimm/",
        "dpreg/",
        "control/branch_imm",
        "control/branch_reg",
        "control/compbranch",
        "control/condbranch",
        "control/testbranch",
        "control/exception",
        "control/hints",
        "control/barriers",
        "control/systemmove",
        "ldst/ldst_pos",
        "ldst/ldst_unscaled",
        "ldst/ldst_immpre",
        "ldst/ldst_immpost",
        "ldst/ldst_regoff",
        "ldst/ldst_unpriv",
        "ldst/ldstpair_off",
        "ldst/ldstpair_pre",
        "ldst/ldstpair_post",
        "ldst/ldstnapair_offs",
        "ldst/loadlit",
        "reserved/perm_undef",
    ];
    if VECTOR_FORMS.contains(&form.name.as_str()) {
        return true;
    }
    let vector = form.field(form.value, "VR") == Some(1);
    // Integer forms of the base set: their condition names no feature, though it may limit their fields.
    let base = !form.features.contains("FEAT_") && !vector;
    let vector_access = vector && form.group.starts_with("ldst/") && offered(form);
    (base || vector_access) && GROUPS.iter().any(|g| form.group.starts_with(g))
}

/// Narrow a word where milestone 0 covers only part of a form: system register moves only reach NZCV and
/// TPIDR_EL0.
fn restrict(form: &Form, word: u32, choice: bool) -> u32 {
    if form.group == "control/systemmove" {
        // o0:op1:CRn:CRm:op2 of NZCV (3,3,4,2,0) or TPIDR_EL0 (3,3,13,0,2).
        let register: u32 = if choice {
            (1 << 14) | (3 << 11) | (4 << 7) | (2 << 3)
        } else {
            (1 << 14) | (3 << 11) | (13 << 7) | 2
        };
        return (word & !(0x7fff << 5)) | (register << 5);
    }
    word
}

/// Why a word is left out of the comparison, if it is. Each is an encoding the architecture leaves
/// CONSTRAINED UNPREDICTABLE, a branch to itself (which never finishes), or a hint whose meaning depends on
/// features the reference CPU has and the CPU we present does not.
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

fn vector_value() -> impl Strategy<Value = u128> {
    prop_oneof![
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
    )
        .prop_map(
            move |(free, choice, x, sp, flags, thread_pointer, seed, base, index, v)| {
                let word = restrict(&form, form.word(free), choice);
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
                }
                Case {
                    word,
                    registers,
                    vectors: v.try_into().unwrap(),
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
        changed: landed.changed.clone(),
    }
}

/// Memory size of the translator's machine: everything up to the end of the code region.
const MEMORY: u64 = CODE + CODE_SPAN;

/// Run a case through the translator.
pub fn translated(case: &Case) -> Outcome {
    let mut machine = Machine::new(MEMORY, CODE, 1024, Syscalls::Stop);
    machine.set_permissions(
        DATA..DATA + DATA_WRITABLE,
        Permissions::READ | Permissions::WRITE,
    );
    machine.set_permissions(DATA + DATA_WRITABLE..DATA + DATA_BYTES, Permissions::READ);
    machine.set_permissions(
        CODE - CODE_SPAN..CODE + CODE_SPAN,
        Permissions::READ | Permissions::EXECUTE,
    );
    let mut data = vec![0u8; DATA_BYTES as usize];
    fill(case.seed, &mut data);
    machine.write(DATA, &data);
    machine.load(&case.word.to_le_bytes(), 0..4, CODE, &[0]);
    let mut registers = RegisterBlock::ZERO;
    registers.x.copy_from_slice(&case.registers[..31]);
    registers.sp = case.registers[31];
    registers.pc = CODE;
    registers.pstate = case.registers[32];
    registers.tpidr_el0 = case.registers[33];
    for (to, from) in registers.v.iter_mut().zip(case.vectors) {
        *to = [from as u64, (from >> 64) as u64];
    }
    machine.set_registers(&registers);
    let (stopped, after) = machine.run();
    let stop = match stopped {
        Stopped::NoCode(address) => Stop::Landed(address),
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
        changed,
    }
}

/// Compare one case on both sides; describe the difference if there is one.
pub fn compare(reference: &mut Cases, case: &Case) -> Result<(), String> {
    let landed = reference.run(case.word, case.seed, &case.registers, &case.vectors);
    let expected = reference_outcome(&landed);
    let actual = translated(case);
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
    if expected.changed != actual.changed {
        report += &format!(
            "  data: reference {:?}, translated {:?}\n",
            expected.changed, actual.changed
        );
    }
    Err(report)
}
