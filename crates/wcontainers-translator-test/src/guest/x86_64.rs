//! x86-64's part of the harness: the forms from iced-x86's own tables, cases drawn for each, the state both
//! sides compare, and what a reference signal means.

use super::*;
use crate::layout::{CODE, DATA, DATA_BYTES, DATA_WRITABLE};
use crate::reference::Target;
use iced_x86::{
    Code, Decoder, DecoderOptions, Encoder, Formatter, Instruction, IntelFormatter, MemoryOperand,
    Mnemonic, OpCodeOperandKind as K, Register,
};
use proptest::prelude::*;
use std::sync::OnceLock;
use wcontainers_x86::{X86_64, X86Block};
use zerocopy::{FromBytes, IntoBytes};

const RSP: usize = 4;
const RIP: usize = 16;
const RFLAGS: usize = 17;
const FS: usize = 18;
const GS: usize = 19;
const MXCSR: usize = 20;
const FCW: usize = 21;
const FSW: usize = 22;
const FTW: usize = 23;
const R0: usize = 24;
const XMM: usize = 32;
const YMMH: usize = 48;
const FIELDS: usize = 64;

/// The six flags and the bits of rflags user code keeps besides them.
const SIX_FLAGS: u64 = 0x8d5;
const KEPT: u64 =
    wcontainers_x86::TF | wcontainers_x86::DF | wcontainers_x86::NT | wcontainers_x86::ID;
/// RF: the CPU sets it in the frame of a fault; no instruction leaves it set.
const RF: u128 = 1 << 16;

const SIGILL: u32 = 4;
const SIGTRAP: u32 = 5;
const SIGBUS: u32 = 7;
const SIGFPE: u32 = 8;
const SIGSEGV: u32 = 11;
const SIGSYS: u32 = 31;
const TRAP_BRKPT: u32 = 1;
const TRAP_TRACE: u32 = 2;
const SI_KERNEL: u32 = 128;
const BUS_ADRALN: u32 = 1;
const FPE_INTDIV: u32 = 1;

/// One layer 1 form: an iced `Code`.
#[derive(Clone, Debug)]
pub struct Form {
    pub code: Code,
    pub name: String,
    pub group: String,
}

impl GuestTest for X86_64 {
    const REFERENCE: Target = Target {
        rust_target: "x86_64-unknown-linux-musl",
        docker_platform: "linux/amd64",
        reference_host: "WC_REFERENCE_X86_64",
        remote_directory: "wcontainers-x86/reference",
        fields: &[
            "rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi", "r8", "r9", "r10", "r11",
            "r12", "r13", "r14", "r15", "rip", "rflags", "fs_base", "gs_base", "mxcsr", "fcw",
            "fsw", "ftw", "r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "xmm0", "xmm1", "xmm2",
            "xmm3", "xmm4", "xmm5", "xmm6", "xmm7", "xmm8", "xmm9", "xmm10", "xmm11", "xmm12",
            "xmm13", "xmm14", "xmm15", "ymmh0", "ymmh1", "ymmh2", "ymmh3", "ymmh4", "ymmh5",
            "ymmh6", "ymmh7", "ymmh8", "ymmh9", "ymmh10", "ymmh11", "ymmh12", "ymmh13", "ymmh14",
            "ymmh15",
        ],
    };
    const C_TARGET: &'static str = "x86_64-linux-musl";
    const GENERAL_ONLY: &'static [&'static str] = &["-mgeneral-regs-only"];
    const NAME: &'static str = "x86_64";
    const ELF_MACHINE: object::elf::Machine = object::elf::EM_X86_64;
    const MUSL_LOADER: &'static str = "/lib/ld-musl-x86_64.so.1";

    fn relocation(kind: object::elf::RelocationType) -> Relocation {
        use object::elf;
        match kind {
            elf::R_X86_64_RELATIVE => Relocation::Relative,
            elf::R_X86_64_64 | elf::R_X86_64_GLOB_DAT | elf::R_X86_64_JUMP_SLOT => {
                Relocation::Absolute
            }
            _ => Relocation::Other,
        }
    }

    fn disassemble(code: &[u8]) -> String {
        let instruction =
            Decoder::with_ip(64, code, CODE, DecoderOptions::FORCE_RESERVED_NOP).decode();
        let mut text = String::new();
        IntelFormatter::new().format(&instruction, &mut text);
        format!("{text} ({:?})", instruction.code())
    }

    const PROBE_ANSWERS: &'static [i32] = &[0];

    fn offered() -> Vec<&'static str> {
        wcontainers_x86::claims::DEFAULT_FEATURES.to_vec()
    }

    fn told() -> Vec<&'static str> {
        panic!("what x86-64 programs are told (CPUID) is milestone X5's")
    }

    type Form = Form;

    fn forms() -> &'static [Form] {
        static FORMS: OnceLock<Vec<Form>> = OnceLock::new();
        FORMS.get_or_init(forms)
    }

    fn form_name(form: &Form) -> &str {
        &form.name
    }

    fn form_group(form: &Form) -> &str {
        &form.group
    }

    fn cases(form: &Form) -> BoxedStrategy<Case> {
        let code = form.code;
        any::<u64>()
            .prop_filter_map("no case of this draw", move |draw| case(code, draw))
            .boxed()
    }

    fn no_form(_seed: u64) -> BoxedStrategy<Case> {
        panic!("x86-64's cases of bytes in no form come later in milestone X3")
    }

    fn hand_cases() -> Vec<Designed> {
        Vec::new()
    }

    const FILLER: u8 = crate::layout::x86_64::FILLER;
    const CODE_SPAN: u64 = crate::layout::x86_64::CODE_SPAN;

    const PC_FIELD: usize = RIP;
    const SP_FIELD: usize = RSP;
    const GENERAL_FIELDS: core::ops::Range<usize> = 0..16;

    fn to_state(thread: &[u8]) -> State {
        let block = X86Block::read_from_bytes(&thread[..size_of::<X86Block>()]).expect("a block");
        let mut state: Vec<u128> = block.gpr.iter().map(|&r| r.into()).collect();
        state
            .extend([block.rip, block.user_rflags(), block.fs_base, block.gs_base].map(u128::from));
        let f = &block.fxsave;
        let half = |at: usize| u128::from(u16::from_le_bytes([f[at], f[at + 1]]));
        state.push(u128::from(u32::from_le_bytes(
            f[24..28].try_into().unwrap(),
        )));
        state.extend([half(0), half(2), u128::from(f[4])]);
        for r in 0..8 {
            let slot = u128::from_le_bytes(f[32 + 16 * r..48 + 16 * r].try_into().unwrap());
            state.push(slot & ((1 << 80) - 1));
        }
        for x in 0..16 {
            state.push(u128::from_le_bytes(
                f[160 + 16 * x..176 + 16 * x].try_into().unwrap(),
            ));
        }
        state.extend(block.ymm_high.map(u128::from_le_bytes));
        assert_eq!(state.len(), FIELDS);
        State(state)
    }

    fn from_state(state: &State, thread: &mut [u8]) {
        let s = &state.0;
        let mut block = X86Block::ZERO;
        for (to, from) in block.gpr.iter_mut().zip(s) {
            *to = *from as u64;
        }
        block.rip = s[RIP] as u64;
        block.flags = (s[RFLAGS] as u64 & SIX_FLAGS) as u32;
        block.rflags = s[RFLAGS] as u64 & KEPT;
        block.fs_base = s[FS] as u64;
        block.gs_base = s[GS] as u64;
        let f = &mut block.fxsave;
        f[24..28].copy_from_slice(&(s[MXCSR] as u32).to_le_bytes());
        f[0..2].copy_from_slice(&(s[FCW] as u16).to_le_bytes());
        f[2..4].copy_from_slice(&(s[FSW] as u16).to_le_bytes());
        f[4] = s[FTW] as u8;
        for r in 0..8 {
            f[32 + 16 * r..48 + 16 * r].copy_from_slice(&s[R0 + r].to_le_bytes());
        }
        for x in 0..16 {
            f[160 + 16 * x..176 + 16 * x].copy_from_slice(&s[XMM + x].to_le_bytes());
        }
        for (to, from) in block.ymm_high.iter_mut().zip(&s[YMMH..YMMH + 16]) {
            *to = from.to_le_bytes();
        }
        thread[..size_of::<X86Block>()].copy_from_slice(block.as_bytes());
    }

    /// The float flags are always kept exact so far: nothing of floating point is translated yet.
    fn track(_thread: &mut [u8]) {}

    fn reconcile(case: &Case, expected: &mut Outcome, _actual: &Outcome) {
        // RF is in the frame of every fault, and the runner's own trap flag in r11 after `syscall`.
        expected.state.0[RFLAGS] &= !RF;
        let instruction =
            Decoder::with_ip(64, case.code.code(), CODE, DecoderOptions::NONE).decode();
        if instruction.mnemonic() == Mnemonic::Syscall {
            const R11: usize = 11;
            expected.state.0[R11] &= !u128::from(wcontainers_x86::TF);
        }
    }

    fn reference_stop(signal: &Signal, length: u8) -> Stop {
        let pc = signal.pc;
        match (signal.number, signal.code) {
            (SIGTRAP, TRAP_TRACE) => Stop::Landed(pc),
            // The `int3` filler right after the instruction: it ran and fell through with its trap held
            // back (`mov ss`).
            (SIGTRAP, SI_KERNEL) if pc == CODE + u64::from(length) + 1 => {
                Stop::Landed(CODE + u64::from(length))
            }
            (SIGTRAP, code) => Stop::Breakpoint(code),
            (SIGILL, _) => Stop::Illegal,
            (SIGSEGV, SI_KERNEL) => Stop::GeneralProtection,
            // Control reached an address where nothing can be run.
            (SIGSEGV, _) if signal.address == pc && pc != CODE => Stop::Landed(pc),
            (SIGSEGV, _) => Stop::MemoryFault(signal.address),
            (SIGBUS, BUS_ADRALN) => Stop::Alignment(signal.address),
            (SIGBUS, SI_KERNEL) => Stop::StackFault,
            (SIGFPE, FPE_INTDIV) => Stop::DivideError,
            (SIGFPE, code) => Stop::FloatException(code),
            (SIGSYS, _) => Stop::Syscall,
            (number, _) => Stop::Other(number, signal.address),
        }
    }

    fn breakpoint_code(detail: u16) -> u32 {
        if detail == 1 { TRAP_BRKPT } else { SI_KERNEL }
    }

    fn excluded(_code: &[u8]) -> Option<&'static str> {
        None
    }

    /// AT_HWCAP is CPUID leaf 1's edx on x86-64: FPU, TSC, CX8, SEP, CMOV, CLFSH, MMX, FXSR, SSE and SSE2.
    /// AT_HWCAP2 bit 1 is FSGSBASE.
    const AUXILIARY: &'static [(u64, u64)] = &[
        (
            crate::layout::AT_HWCAP,
            1 | 1 << 4
                | 1 << 8
                | 1 << 11
                | 1 << 15
                | 1 << 19
                | 1 << 23
                | 1 << 24
                | 1 << 25
                | 1 << 26,
        ),
        (crate::layout::AT_HWCAP2, 2),
    ];

    const STAND_IN: StandIn = StandIn {
        write: 1,
        writev: 20,
        exit: 60,
        exit_group: 231,
        brk: 12,
    };

    /// Every general register but rsp and rbp; system calls set rax, rcx and r11; rbx is kept.
    const MODEL: crate::layer2::Model = crate::layer2::Model {
        registers: &[0, 1, 2, 3, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        syscall_sets: &[0, 1, 11],
        kept: 3,
    };

    fn assemble(_program: &crate::layer2::Program) -> crate::programs::Blob {
        panic!("x86-64's layer 2 programs are milestone X4's")
    }

    fn smoke() -> Smoke {
        panic!("x86-64's smoke programs come with its programs in milestone X3")
    }

    fn checks() -> Vec<Check> {
        panic!("x86-64's check programs are milestone X4's")
    }

    fn changes() -> Vec<Change> {
        panic!("x86-64's changes programs are milestone X6's")
    }
}

/// The forms: every `Code` valid in 64-bit mode with a legacy encoding whose meaning is written, shown by one
/// instruction of it.
fn forms() -> Vec<Form> {
    Code::values()
        .filter(|code| {
            let info = code.op_code();
            info.mode64()
                && info.is_instruction()
                && info.encoding() == iced_x86::EncodingKind::Legacy
        })
        .filter(|&code| {
            (0..64).any(|draw| case(code, draw).is_some_and(|c| written(c.code.code(), code)))
        })
        .map(|code| Form {
            code,
            name: format!("{code:?}"),
            group: format!("{:?}", code.mnemonic()),
        })
        .collect()
}

/// Whether `bytes` decode to `code` for the translator, and its meaning is written.
fn written(bytes: &[u8], code: Code) -> bool {
    wcontainers_x86::decode(bytes).is_some_and(|(i, length)| {
        usize::from(length) == bytes.len() && i.iced.code() == code && wcontainers_x86::written(&i)
    })
}

/// splitmix64, for the draws of one case.
struct Draw(u64);

impl Draw {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    /// True `per` times in a hundred.
    fn chance(&mut self, per: u64) -> bool {
        self.below(100) < per
    }

    /// A value weighted toward the edges of every width.
    fn value(&mut self) -> u64 {
        const EDGES: [u64; 16] = [
            0,
            1,
            2,
            0x7f,
            0x80,
            0xff,
            0x7fff,
            0x8000,
            0xffff,
            0x7fff_ffff,
            0x8000_0000,
            0xffff_ffff,
            0x7fff_ffff_ffff_ffff,
            0x8000_0000_0000_0000,
            u64::MAX,
            u64::MAX - 1,
        ];
        match self.below(8) {
            0..=2 => EDGES[self.below(16) as usize],
            3 => self.below(64),
            4 => self.next() & 0xff,
            5 => self.next() & 0xffff_ffff,
            _ => self.next(),
        }
    }
}

/// iced's register with this number.
fn register(n: u32) -> Register {
    Register::values().nth(n as usize).expect("a register")
}

/// A general register of a width, drawn from all of them.
fn general(draw: &mut Draw, width: u8) -> Register {
    let first = match width {
        8 => return register(Register::AL as u32 + draw.below(20) as u32),
        16 => Register::AX,
        32 => Register::EAX,
        _ => Register::RAX,
    };
    register(first as u32 + draw.below(16) as u32)
}

/// An operand of an instruction being built.
#[derive(Clone, Copy)]
enum Operand {
    R(Register),
    M(MemoryOperand),
    Signed(i32),
    Unsigned(u32),
    Wide(u64),
    Branch(u64),
}

fn build(code: Code, ops: &[Operand]) -> Option<Instruction> {
    use Operand::*;
    let made = match *ops {
        [] => Ok(Instruction::with(code)),
        [R(a)] => Instruction::with1(code, a),
        [M(a)] => Instruction::with1(code, a),
        [Signed(a)] => Instruction::with1(code, a),
        [Unsigned(a)] => Instruction::with1(code, a),
        [Branch(t)] => Instruction::with_branch(code, t),
        [R(a), R(b)] => Instruction::with2(code, a, b),
        [R(a), M(b)] => Instruction::with2(code, a, b),
        [M(a), R(b)] => Instruction::with2(code, a, b),
        [R(a), Signed(b)] => Instruction::with2(code, a, b),
        [R(a), Unsigned(b)] => Instruction::with2(code, a, b),
        [R(a), Wide(b)] => Instruction::with2(code, a, b),
        [M(a), Signed(b)] => Instruction::with2(code, a, b),
        [M(a), Unsigned(b)] => Instruction::with2(code, a, b),
        [R(a), R(b), Signed(c)] => Instruction::with3(code, a, b, c),
        [R(a), R(b), Unsigned(c)] => Instruction::with3(code, a, b, c),
        [R(a), M(b), Signed(c)] => Instruction::with3(code, a, b, c),
        [R(a), M(b), Unsigned(c)] => Instruction::with3(code, a, b, c),
        _ => return None,
    };
    made.ok()
}

/// A string instruction of `code`, with this address size and segment for rsi.
fn string(code: Code, address_size: u32, segment: Register) -> Option<Instruction> {
    use Code::*;
    use iced_x86::RepPrefixKind::None as N;
    let made = match code {
        Movsb_m8_m8 => Instruction::with_movsb(address_size, segment, N),
        Movsw_m16_m16 => Instruction::with_movsw(address_size, segment, N),
        Movsd_m32_m32 => Instruction::with_movsd(address_size, segment, N),
        Movsq_m64_m64 => Instruction::with_movsq(address_size, segment, N),
        Cmpsb_m8_m8 => Instruction::with_cmpsb(address_size, segment, N),
        Cmpsw_m16_m16 => Instruction::with_cmpsw(address_size, segment, N),
        Cmpsd_m32_m32 => Instruction::with_cmpsd(address_size, segment, N),
        Cmpsq_m64_m64 => Instruction::with_cmpsq(address_size, segment, N),
        Lodsb_AL_m8 => Instruction::with_lodsb(address_size, segment, N),
        Lodsw_AX_m16 => Instruction::with_lodsw(address_size, segment, N),
        Lodsd_EAX_m32 => Instruction::with_lodsd(address_size, segment, N),
        Lodsq_RAX_m64 => Instruction::with_lodsq(address_size, segment, N),
        Stosb_m8_AL => Instruction::with_stosb(address_size, N),
        Stosw_m16_AX => Instruction::with_stosw(address_size, N),
        Stosd_m32_EAX => Instruction::with_stosd(address_size, N),
        Stosq_m64_RAX => Instruction::with_stosq(address_size, N),
        Scasb_AL_m8 => Instruction::with_scasb(address_size, N),
        Scasw_AX_m16 => Instruction::with_scasw(address_size, N),
        Scasd_EAX_m32 => Instruction::with_scasd(address_size, N),
        Scasq_RAX_m64 => Instruction::with_scasq(address_size, N),
        _ => return None,
    };
    made.ok()
}

/// Where a memory operand points: mostly into the data area, and a share of edges (just outside it, its
/// read-only page, the last 64 bytes below 2^47, non-canonical).
fn target(draw: &mut Draw, bytes: u64) -> u64 {
    match draw.below(20) {
        0..=12 => DATA + draw.below(DATA_BYTES - bytes + 1),
        13 => DATA + DATA_WRITABLE - bytes + draw.below(2 * bytes),
        14 => DATA - bytes + draw.below(2 * bytes),
        15 => DATA + DATA_BYTES - bytes + draw.below(2 * bytes),
        16 | 17 => (1 << 47) - 64 + draw.below(64),
        18 => 0x8000_0000_0000_0000 | draw.next() >> 1,
        _ => (0xffff_8000_0000_0000u64).wrapping_sub(draw.below(64)),
    }
}

/// Where a rip-relative operand points: the data area, the code area around the instruction, or just past
/// the code area's ends.
fn near_target(draw: &mut Draw) -> u64 {
    let span = crate::layout::x86_64::CODE_SPAN;
    match draw.below(10) {
        0..=6 => DATA + draw.below(DATA_BYTES),
        7 => CODE - 64 + draw.below(128),
        8 => CODE - span - 16 + draw.below(32),
        _ => CODE + span - 16 + draw.below(32),
    }
}

/// The starting state's rflags: the six flags, DF, NT and ID drawn; bits 1 and 9 set.
fn rflags(draw: &mut Draw) -> u128 {
    let keep = SIX_FLAGS | wcontainers_x86::DF | wcontainers_x86::NT | wcontainers_x86::ID;
    u128::from(draw.next() & keep) | 0x202
}

/// One case of `code` from the draw `seed`, or none when the draw gives no encodable instruction.
fn case(code: Code, seed: u64) -> Option<Case> {
    let mut draw = Draw(seed ^ (code as u64).wrapping_mul(0x9e37_79b9));
    let info = code.op_code();
    let mut state = vec![0u128; FIELDS];
    for r in state.iter_mut().take(16) {
        *r = draw.value().into();
    }
    // The stack pointer: mostly in the data area, a share at the edges.
    state[RSP] = match draw.below(10) {
        0..=6 => (DATA + 0x100 + draw.below(DATA_WRITABLE - 0x200)).into(),
        7 => target(&mut draw, 8).into(),
        _ => draw.value().into(),
    };
    state[RIP] = CODE.into();
    state[RFLAGS] = rflags(&mut draw);
    state[FS] = (draw.next() % ((1 << 47) - 4096)).into();
    state[GS] = (draw.next() % ((1 << 47) - 4096)).into();
    state[MXCSR] = 0x1f80;
    state[FCW] = 0x37f;
    let address32 = draw.chance(10);
    let mut memory = None;
    let instruction = if let Some(i) = {
        let segment = [Register::None, Register::FS, Register::GS][draw.below(3) as usize];
        string(code, if address32 { 32 } else { 64 }, segment)
    } {
        // Each of rsi and rdi where the draw says.
        let bytes = u64::from(memory_size(&i));
        for field in [6, 7] {
            let to = target(&mut draw, bytes);
            state[field] = if address32 {
                (u128::from(draw.next()) << 32 | u128::from(to as u32)) & u128::from(u64::MAX)
            } else {
                to.into()
            };
        }
        i
    } else {
        let mut ops = Vec::new();
        for n in 0..info.op_count() {
            let kind = info.op_kind(n);
            let op = match kind {
                K::r8_or_mem
                | K::r16_or_mem
                | K::r32_or_mem
                | K::r64_or_mem
                | K::mem
                | K::mem_offs => {
                    let width = match kind {
                        K::r8_or_mem => 8,
                        K::r16_or_mem => 16,
                        K::r32_or_mem => 32,
                        _ => 64,
                    };
                    if kind != K::mem && kind != K::mem_offs && draw.chance(40) {
                        Operand::R(general(&mut draw, width))
                    } else {
                        let m = memory_operand(&mut draw, kind == K::mem_offs, address32);
                        memory = Some(m);
                        Operand::M(m)
                    }
                }
                K::r8_reg | K::r8_opcode => Operand::R(general(&mut draw, 8)),
                K::r16_reg | K::r16_rm | K::r16_opcode => Operand::R(general(&mut draw, 16)),
                K::r32_reg | K::r32_rm | K::r32_opcode => Operand::R(general(&mut draw, 32)),
                K::r64_reg | K::r64_rm | K::r64_opcode => Operand::R(general(&mut draw, 64)),
                K::al => Operand::R(Register::AL),
                K::cl => Operand::R(Register::CL),
                K::ax => Operand::R(Register::AX),
                K::eax => Operand::R(Register::EAX),
                K::rax => Operand::R(Register::RAX),
                K::imm8 => Operand::Unsigned(draw.value() as u32 & 0xff),
                K::imm8_const_1 => Operand::Unsigned(1),
                K::imm16 => Operand::Unsigned(draw.value() as u32 & 0xffff),
                K::imm32 => Operand::Unsigned(draw.value() as u32),
                K::imm64 => Operand::Wide(draw.value()),
                K::imm8sex16 | K::imm8sex32 | K::imm8sex64 => {
                    Operand::Signed(draw.value() as i8 as i32)
                }
                K::imm32sex64 => Operand::Signed(draw.value() as i32),
                K::br64_1 | K::br64_4 => {
                    // Anywhere in reach but the instruction itself, which would run again.
                    let reach: i64 = if kind == K::br64_1 { 120 } else { 0x7000 };
                    let mut by = draw.below(2 * reach as u64) as i64 - reach;
                    if by == 0 {
                        by = 1;
                    }
                    Operand::Branch(CODE.wrapping_add_signed(by))
                }
                _ => return None,
            };
            ops.push(op);
        }
        build(code, &ops)?
    };
    let mut encoder = Encoder::new(64);
    encoder.encode(&instruction, CODE).ok()?;
    let bytes = encoder.take_buffer();
    if bytes.len() > 15 {
        return None;
    }
    // Divisions: half the dividends have rdx (or ah, dx, edx) the extension of the low half, as compilers
    // leave them, a quarter a high half below most divisors, the rest anything.
    if matches!(code.mnemonic(), Mnemonic::Div | Mnemonic::Idiv) {
        let width = instruction.op_register(0).size() * 8;
        let width = if instruction.op0_kind() == iced_x86::OpKind::Memory {
            memory_size(&instruction) as usize * 8
        } else {
            width
        };
        let signed = code.mnemonic() == Mnemonic::Idiv;
        let low = state[0] as u64;
        let high = match draw.below(4) {
            0 | 1 if signed => {
                let sign = (low >> (width - 1)) & 1;
                if sign == 1 { u64::MAX } else { 0 }
            }
            0 | 1 => 0,
            2 => draw.below(16),
            _ => draw.value(),
        };
        if width == 8 {
            state[0] = u128::from(low & !0xff00 | (high & 0xff) << 8);
        } else {
            state[2] = high.into();
        }
    }
    // Point the memory operand where the draw says, through its registers or the segment base.
    if let Some(m) = memory {
        let size = u64::from(memory_size(&instruction)).max(1);
        let to = target(&mut draw, size);
        aim(&mut draw, &mut state, &m, to, address32);
    }
    Some(Case {
        code: Encoded::new(&bytes),
        state: State(state),
        tracking: false,
        seed: draw.next(),
    })
}

fn memory_size(instruction: &Instruction) -> u32 {
    instruction.memory_size().size() as u32
}

/// A memory operand: a base and an index or not, any scale, a displacement of each size; rip-relative or
/// through `fs:` or `gs:` for a share.
fn memory_operand(draw: &mut Draw, offset_only: bool, address32: bool) -> MemoryOperand {
    let width = if address32 { 32 } else { 64 };
    let segment = match draw.below(10) {
        0 => Register::FS,
        1 => Register::GS,
        _ => Register::None,
    };
    if offset_only {
        let size = if address32 { 4 } else { 8 };
        return MemoryOperand::new(Register::None, Register::None, 1, 0, size, false, segment);
    }
    if !address32 && draw.chance(10) {
        // iced's displacement of a rip-relative operand is the address it reaches.
        let to = near_target(draw) as i64;
        return MemoryOperand::new(Register::RIP, Register::None, 1, to, 4, false, segment);
    }
    let base = if draw.chance(10) {
        Register::None
    } else {
        general(draw, width)
    };
    let index = if draw.chance(50) {
        // rsp cannot be an index.
        let r = general(draw, width);
        if matches!(r, Register::RSP | Register::ESP) {
            Register::None
        } else {
            r
        }
    } else {
        Register::None
    };
    let scale = [1, 2, 4, 8][draw.below(4) as usize];
    let displacement = match draw.below(4) {
        0 => 0,
        1 => draw.below(256) as i64 - 128,
        _ => draw.next() as i32 as i64,
    };
    let size = if displacement == 0 && base != Register::None {
        0
    } else if base != Register::None && i8::try_from(displacement).is_ok() {
        1
    } else {
        4
    };
    MemoryOperand::new(base, index, scale, displacement, size, false, segment)
}

/// Set the registers (or the segment base) so the operand's address is `to`.
fn aim(draw: &mut Draw, state: &mut [u128], m: &MemoryOperand, to: u64, address32: bool) {
    let index_value = field_of(m.index).map_or(0, |n| state[n] as u64);
    let scaled = index_value.wrapping_mul(u64::from(m.scale));
    match field_of(m.base) {
        Some(b) if field_of(m.index) != Some(b) => {
            let base = to.wrapping_sub(m.displacement as u64).wrapping_sub(scaled);
            state[b] = if address32 {
                (u128::from(draw.next()) << 32 | u128::from(base as u32)) & u128::from(u64::MAX)
            } else {
                base.into()
            };
        }
        _ if m.segment_prefix != Register::None && !address32 && m.base != Register::RIP => {
            let effective = field_of(m.base)
                .map_or(0, |n| state[n] as u64)
                .wrapping_add(scaled)
                .wrapping_add(m.displacement as u64);
            let base = to.wrapping_sub(effective);
            if base < (1 << 47) - 4096 {
                let field = if m.segment_prefix == Register::FS {
                    FS
                } else {
                    GS
                };
                state[field] = base.into();
            }
        }
        _ => {
            if let Some(i) = field_of(m.index)
                && m.scale == 1
                && field_of(m.base).is_none()
                && m.base != Register::RIP
            {
                state[i] = to.wrapping_sub(m.displacement as u64).into();
            }
        }
    }
}

/// The field of a general register of 16, 32 or 64 bits.
fn field_of(r: Register) -> Option<usize> {
    let v = r as usize;
    match v {
        21..=36 => Some(v - 21),
        37..=52 => Some(v - 37),
        53..=68 => Some(v - 53),
        _ => None,
    }
}
