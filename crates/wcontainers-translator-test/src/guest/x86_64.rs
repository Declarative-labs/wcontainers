//! x86-64's part of the harness: the forms from iced-x86's own tables, cases drawn for each, the state both
//! sides compare, and what a reference signal means.

mod assemble;
mod loops;

use super::*;
use crate::layout::{CODE, DATA, DATA_BYTES, DATA_WRITABLE};
use crate::reference::Target;
use iced_x86::{
    Code, Decoder, DecoderOptions, Encoder, Formatter, Instruction, InstructionInfoFactory,
    IntelFormatter, MemoryOperand, Mnemonic, OpAccess, OpCodeOperandKind as K, OpKind, Register,
    RepPrefixKind,
};
use proptest::prelude::*;
use std::sync::OnceLock;
use wcontainers_abi::PAGE_SIZE;
use wcontainers_x86::encoding::{ESCAPE, OPERAND_SIZE, REP, REPNE, REX, REX_W, is_rex};
use wcontainers_x86::{USER_END, USER_TOP};
use wcontainers_x86::{X86_64, X86Block};
use zerocopy::{FromBytes, IntoBytes};

const RSP: usize = 4;
/// The field of rip in the reference's state.
pub const RIP: usize = 16;
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
const SIX_FLAGS: u64 = wcontainers_x86::ARITHMETIC_FLAGS;
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
    // zig sets the CPU's features itself, and `-mgeneral-regs-only` does not keep its vectoriser off SSE;
    // this leaves out the vector units.
    const GENERAL_ONLY: &'static [&'static str] = &["-mcpu=x86_64-sse-sse2-mmx"];
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
        let instruction = Decoder::with_ip(
            wcontainers_x86::MODE_BITS,
            code,
            CODE,
            DecoderOptions::FORCE_RESERVED_NOP,
        )
        .decode();
        let mut text = String::new();
        IntelFormatter::new().format(&instruction, &mut text);
        format!("{text} ({:?})", instruction.code())
    }

    // Not-a-number results follow x86's rules or not.
    const PROBE_ANSWERS: &'static [i32] = &[0, 1];

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

    fn no_form(seed: u64) -> BoxedStrategy<Case> {
        any::<u64>()
            .prop_filter_map("not in no form", move |draw| no_form(draw, seed))
            .boxed()
    }

    fn hand_cases() -> Vec<Designed> {
        hand_cases()
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
        state.extend([
            f.mxcsr.into(),
            f.fcw.into(),
            f.fsw.into(),
            u128::from(f.ftw),
        ]);
        for slot in f.st {
            state.push(u128::from_le_bytes(slot) & ((1 << wcontainers_x86::X87_REGISTER_BITS) - 1));
        }
        state.extend(f.xmm.map(u128::from_le_bytes));
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
        f.mxcsr = s[MXCSR] as u32;
        f.fcw = s[FCW] as u16;
        f.fsw = s[FSW] as u16;
        f.ftw = s[FTW] as u8;
        for (to, from) in f.st.iter_mut().zip(&s[R0..]) {
            *to = from.to_le_bytes();
        }
        for (to, from) in f.xmm.iter_mut().zip(&s[XMM..]) {
            *to = from.to_le_bytes();
        }
        for (to, from) in block.ymm_high.iter_mut().zip(&s[YMMH..YMMH + 16]) {
            *to = from.to_le_bytes();
        }
        thread[..size_of::<X86Block>()].copy_from_slice(block.as_bytes());
    }

    fn track(thread: &mut [u8]) {
        let at = wcontainers_x86::MXCSR_TRACKING;
        thread[at..at + size_of::<u32>()].copy_from_slice(&1u32.to_le_bytes());
    }

    fn reconcile(case: &Case, expected: &mut Outcome, _actual: &Outcome) {
        // RF is in the frame of every fault, and the runner's own trap flag in r11 after `syscall`.
        expected.state.0[RFLAGS] &= !RF;
        let instruction = Decoder::with_ip(
            wcontainers_x86::MODE_BITS,
            case.code.code(),
            CODE,
            DecoderOptions::NONE,
        )
        .decode();
        // AC is never held (plan 5.1): a popped AC reads back clear by design.
        if matches!(instruction.mnemonic(), Mnemonic::Popf | Mnemonic::Popfq) {
            expected.state.0[RFLAGS] &= !u128::from(wcontainers_x86::ALIGNMENT_CHECK);
        }
        // The CPU's part of an unmasked SIMD exception is MXCSR's flags; the kernel works out the si_code.
        if let Stop::FloatException(_) = expected.stop {
            let mxcsr = expected.state.0[MXCSR] as u32;
            expected.stop = Stop::FloatException(wcontainers_x86::mxcsr_raised(mxcsr));
        }
        // In MXCSR's default mode, with its flags not kept, they keep what they had (plan 5.8).
        let mode = u128::from(wcontainers_x86::MXCSR_MODE);
        let flags = u128::from(wcontainers_x86::MXCSR_FLAGS);
        if !case.tracking
            && case.state.0[MXCSR] & mode == u128::from(wcontainers_x86::MXCSR_AT_START) & mode
            && instruction.mnemonic() != Mnemonic::Ldmxcsr
        {
            let mxcsr = &mut expected.state.0[MXCSR];
            *mxcsr = *mxcsr & !flags | case.state.0[MXCSR] & flags;
        }
        if instruction.mnemonic() == Mnemonic::Syscall {
            const R11: usize = 11;
            expected.state.0[R11] &= !u128::from(wcontainers_x86::TF);
        }
        // The same trap flag in the image `pushf` stores; a byte it then leaves as it was is not changed.
        if matches!(instruction.mnemonic(), Mnemonic::Pushf | Mnemonic::Pushfq)
            && matches!(expected.stop, Stop::Landed(_))
        {
            let tf = wcontainers_x86::TF.trailing_zeros();
            let bytes = u64::from(instruction.stack_pointer_increment().unsigned_abs());
            let at = (case.state.0[RSP] as u64)
                .wrapping_sub(bytes)
                .wrapping_add(u64::from(tf / u8::BITS))
                .wrapping_sub(DATA);
            let mut start = vec![0; DATA_BYTES as usize];
            crate::layout::fill(case.seed, &mut start);
            if let Some(&first) = start.get(at as usize) {
                let changed = &mut expected.changed;
                let place = changed.partition_point(|&(o, _)| u64::from(o) < at);
                let found = changed.get(place).filter(|&&(o, _)| u64::from(o) == at);
                let stored = found.map_or(first, |&(_, v)| v) & !(1 << (tf % u8::BITS));
                match (found.is_some(), stored == first) {
                    (true, true) => _ = changed.remove(place),
                    (true, false) => changed[place].1 = stored,
                    (false, false) => changed.insert(place, (at as u32, stored)),
                    (false, true) => {}
                }
            }
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

    /// `int 0x80` and `sysenter` run Linux's 32-bit system calls on the reference, which this phase does
    /// not provide (plan 5.2); the designed-answers cases check what we give.
    fn excluded(code: &[u8]) -> Option<&'static str> {
        let instruction =
            Decoder::with_ip(wcontainers_x86::MODE_BITS, code, CODE, DecoderOptions::NONE).decode();
        match instruction.code() {
            Code::Int_imm8
                if instruction.immediate8() == wcontainers_x86::LINUX_32_BIT_SYSTEM_CALL =>
            {
                Some("int 0x80: a 32-bit system call on the reference")
            }
            Code::Sysenter => Some("sysenter: a 32-bit system call on the reference"),
            Code::Rdtsc | Code::Rdtscp => Some(TIME_STAMP_BY_DESIGN),
            Code::Cpuid => Some(CPUID_BY_DESIGN),
            _ => None,
        }
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
        // arch_prctl with ARCH_SET_GS or ARCH_SET_FS, below the top of user space.
        set_base: Some(SetBase {
            number: 158,
            bases: &[
                (0x1001, core::mem::offset_of!(X86Block, gs_base)),
                (0x1002, core::mem::offset_of!(X86Block, fs_base)),
            ],
            limit: USER_TOP,
        }),
    };

    /// Every general register but rsp, rbp (the area) and r12 to r15 (scratch, the loop and recursion
    /// counters, a moving base); the model's system call sets rax, rcx, rdx, rsi, rdi and r11; rbx is kept.
    const MODEL: crate::layer2::Model = {
        use wcontainers_x86::{R8, R9, R10, R11, RAX, RBX, RCX, RDI, RDX, RSI};
        crate::layer2::Model {
            registers: &[RAX, RCX, RDX, RBX, RSI, RDI, R8, R9, R10, R11],
            syscall_sets: &[RAX, RCX, RDX, RSI, RDI, R11],
            kept: RBX,
        }
    };

    fn assemble(program: &crate::layer2::Program) -> crate::programs::Blob {
        assemble::assemble(program)
    }

    fn smoke() -> Smoke {
        panic!("x86-64's smoke programs are milestone X5's")
    }

    fn checks() -> Vec<Check> {
        let mut checks = assemble::checks();
        checks.extend(loops::checks());
        checks
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

/// Whether `bytes` decode to `code` for the translator, and its meaning is written or it is kernel only (its
/// meaning the fault Linux gives a program).
fn written(bytes: &[u8], code: Code) -> bool {
    wcontainers_x86::decode(bytes).is_some_and(|(i, length)| {
        usize::from(length) == bytes.len()
            && i.iced.code() == code
            && (wcontainers_x86::written(&i)
                || wcontainers_x86::claim_group(code) == wcontainers_x86::claims::GROUP_KERNEL)
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

/// An xmm register of the sixteen legacy SSE names.
fn xmm(draw: &mut Draw) -> Register {
    register(Register::XMM0 as u32 + draw.below(16) as u32)
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
        [Unsigned(a), R(b)] => Instruction::with2(code, a, b),
        [R(a), R(b), Signed(c)] => Instruction::with3(code, a, b, c),
        [R(a), R(b), Unsigned(c)] => Instruction::with3(code, a, b, c),
        [R(a), M(b), Signed(c)] => Instruction::with3(code, a, b, c),
        [R(a), M(b), Unsigned(c)] => Instruction::with3(code, a, b, c),
        [R(a), R(b), R(c)] => Instruction::with3(code, a, b, c),
        [M(a), R(b), R(c)] => Instruction::with3(code, a, b, c),
        [M(a), R(b), Unsigned(c)] => Instruction::with3(code, a, b, c),
        _ => return None,
    };
    made.ok()
}

/// A string instruction of `code`, with this address size and segment for rsi.
fn string(
    code: Code,
    address_size: u32,
    segment: Register,
    rep: RepPrefixKind,
) -> Option<Instruction> {
    use Code::*;
    let n = rep;
    let made = match code {
        Movsb_m8_m8 => Instruction::with_movsb(address_size, segment, n),
        Movsw_m16_m16 => Instruction::with_movsw(address_size, segment, n),
        Movsd_m32_m32 => Instruction::with_movsd(address_size, segment, n),
        Movsq_m64_m64 => Instruction::with_movsq(address_size, segment, n),
        Cmpsb_m8_m8 => Instruction::with_cmpsb(address_size, segment, n),
        Cmpsw_m16_m16 => Instruction::with_cmpsw(address_size, segment, n),
        Cmpsd_m32_m32 => Instruction::with_cmpsd(address_size, segment, n),
        Cmpsq_m64_m64 => Instruction::with_cmpsq(address_size, segment, n),
        Lodsb_AL_m8 => Instruction::with_lodsb(address_size, segment, n),
        Lodsw_AX_m16 => Instruction::with_lodsw(address_size, segment, n),
        Lodsd_EAX_m32 => Instruction::with_lodsd(address_size, segment, n),
        Lodsq_RAX_m64 => Instruction::with_lodsq(address_size, segment, n),
        Stosb_m8_AL => Instruction::with_stosb(address_size, n),
        Stosw_m16_AX => Instruction::with_stosw(address_size, n),
        Stosd_m32_EAX => Instruction::with_stosd(address_size, n),
        Stosq_m64_RAX => Instruction::with_stosq(address_size, n),
        Scasb_AL_m8 => Instruction::with_scasb(address_size, n),
        Scasw_AX_m16 => Instruction::with_scasw(address_size, n),
        Scasd_EAX_m32 => Instruction::with_scasd(address_size, n),
        Scasq_RAX_m64 => Instruction::with_scasq(address_size, n),
        Insb_m8_DX => Instruction::with_insb(address_size, n),
        Insw_m16_DX => Instruction::with_insw(address_size, n),
        Insd_m32_DX => Instruction::with_insd(address_size, n),
        Outsb_DX_m8 => Instruction::with_outsb(address_size, segment, n),
        Outsw_DX_m16 => Instruction::with_outsw(address_size, segment, n),
        Outsd_DX_m32 => Instruction::with_outsd(address_size, segment, n),
        _ => return None,
    };
    made.ok()
}

/// In a hundred rep string cases: counts of zero, counts that run past the data area, and (of the rest)
/// destinations just above their sources.
const ZERO_COUNTS: u64 = 12;
const LONG_COUNTS: u64 = 12;
const OVERLAPPED: u64 = 8;
/// Why layer 1 leaves out `cpuid`.
const CPUID_BY_DESIGN: &str = "cpuid: the claims and designed leaves, not the CPU's";
/// Why layer 1 leaves out `rdtsc` and `rdtscp`.
const TIME_STAMP_BY_DESIGN: &str = "the time stamp counter: answered by design";
/// How near an edge of the address space a memory operand drawn there points.
const EDGE_BYTES: u64 = 64;
/// In a hundred: bit offsets aimed with their operand, and reserved NOPs drawn with REX.W.
const AIMED_BIT_OFFSETS: u64 = 75;
const DRAWN_REX_W: u64 = 30;

/// Where a memory operand points: mostly into the data area, and a share of edges (just outside it, its
/// read-only page, the last 64 bytes below 2^47, non-canonical).
fn target(draw: &mut Draw, bytes: u64) -> u64 {
    match draw.below(20) {
        0..=12 => DATA + draw.below(DATA_BYTES - bytes + 1),
        13 => DATA + DATA_WRITABLE - bytes + draw.below(2 * bytes),
        14 => DATA - bytes + draw.below(2 * bytes),
        15 => DATA + DATA_BYTES - bytes + draw.below(2 * bytes),
        16 | 17 => USER_END - EDGE_BYTES + draw.below(EDGE_BYTES),
        18 => 1 << (u64::BITS - 1) | draw.next() >> 1,
        // Just below the upper canonical half.
        _ => USER_END.wrapping_neg().wrapping_sub(draw.below(EDGE_BYTES)),
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
    state[FS] = (draw.next() % USER_TOP).into();
    state[GS] = (draw.next() % USER_TOP).into();
    state[MXCSR] = mxcsr(&mut draw).into();
    let tracking = draw.chance(TRACKED);
    state[FCW] = wcontainers_x86::FCW_AT_START.into();
    for x in &mut state[XMM..XMM + 16] {
        *x = vector(&mut draw);
    }
    let address32 = draw.chance(10);
    let mut memory = None;
    let instruction = if let Some(i) = {
        let segment = [Register::None, Register::FS, Register::GS][draw.below(3) as usize];
        let reps = [
            RepPrefixKind::None,
            RepPrefixKind::Repe,
            RepPrefixKind::Repne,
        ];
        let rep = reps[draw.below(reps.len() as u64) as usize];
        string(code, if address32 { 32 } else { 64 }, segment, rep)
    } {
        // Each of rsi and rdi where the draw says.
        let bytes = u64::from(memory_size(&i));
        let [rcx, rsi, rdi] = [
            wcontainers_x86::RCX,
            wcontainers_x86::RSI,
            wcontainers_x86::RDI,
        ]
        .map(usize::from);
        for field in [rsi, rdi] {
            let to = target(&mut draw, bytes);
            state[field] = if address32 {
                (u128::from(draw.next()) << 32 | u128::from(to as u32)) & u128::from(u64::MAX)
            } else {
                to.into()
            };
        }
        if i.has_rep_prefix() || i.has_repne_prefix() {
            // A count that mostly stays in the data area, sometimes runs past it, and is sometimes zero;
            // rdi sometimes just above rsi, where a forward copy repeats what it wrote.
            let count = if draw.chance(ZERO_COUNTS) {
                0
            } else if draw.chance(LONG_COUNTS) {
                draw.below(DATA_BYTES / bytes + EDGE_BYTES)
            } else {
                draw.below(EDGE_BYTES)
            };
            state[rcx] = if address32 {
                state[rcx] & !u128::from(u32::MAX) | u128::from(count)
            } else {
                count.into()
            };
            if draw.chance(OVERLAPPED) {
                state[rdi] =
                    (state[rsi] + u128::from(draw.below(EDGE_BYTES))) & u128::from(u64::MAX);
            }
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
                // `xlat`: rbx (ebx) plus al, through a drawn segment.
                K::seg_rBX_al => {
                    let drawn = memory_operand(&mut draw, false, address32);
                    let base = if address32 {
                        Register::EBX
                    } else {
                        Register::RBX
                    };
                    let m = MemoryOperand {
                        base,
                        index: Register::AL,
                        scale: 1,
                        displacement: 0,
                        displ_size: 0,
                        is_broadcast: false,
                        segment_prefix: drawn.segment_prefix,
                    };
                    memory = Some(m);
                    Operand::M(m)
                }
                K::xmm_or_mem if draw.chance(40) => Operand::R(xmm(&mut draw)),
                K::xmm_or_mem => {
                    let m = memory_operand(&mut draw, false, address32);
                    memory = Some(m);
                    Operand::M(m)
                }
                K::xmm_reg | K::xmm_rm => Operand::R(xmm(&mut draw)),
                K::r8_reg | K::r8_opcode => Operand::R(general(&mut draw, 8)),
                K::r16_reg | K::r16_rm | K::r16_opcode => Operand::R(general(&mut draw, 16)),
                K::r32_reg | K::r32_rm | K::r32_opcode => Operand::R(general(&mut draw, 32)),
                K::r64_reg | K::r64_rm | K::r64_opcode => Operand::R(general(&mut draw, 64)),
                K::al => Operand::R(Register::AL),
                K::cl => Operand::R(Register::CL),
                K::ax => Operand::R(Register::AX),
                K::eax => Operand::R(Register::EAX),
                K::rax => Operand::R(Register::RAX),
                K::dx => Operand::R(Register::DX),
                K::cr_reg => Operand::R(
                    [
                        Register::CR0,
                        Register::CR2,
                        Register::CR3,
                        Register::CR4,
                        Register::CR8,
                    ][draw.below(5) as usize],
                ),
                K::dr_reg => Operand::R(Register::DR0 + draw.below(8) as u32),
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
    let mut bytes = encoder.take_buffer();
    if code.mnemonic() == Mnemonic::Reservednop {
        bytes = with_drawn_prefixes(&mut draw, bytes);
    }
    if bytes.len() > 15 || X86_64::excluded(&bytes).is_some() {
        return None;
    }
    if branches_into_itself(&bytes) {
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
    let mut seed = None;
    if let Some(m) = memory {
        let size = u64::from(memory_size(&instruction)).max(1);
        let mut to = target(&mut draw, size);
        // Most `ldmxcsr` cases load a value it takes: a place in the data whose bits above MXCSR's are zero.
        if code == Code::Ldmxcsr_m32
            && draw.chance(LOADABLE_MXCSR)
            && let Some((s, at)) = loadable_mxcsr(&mut draw)
        {
            seed = Some(s);
            to = DATA + at;
        }
        // Most 16-byte operands aligned, as SSE's aligned forms need.
        if size == 16 && draw.chance(80) {
            to &= !15;
        }
        // A bit offset in a register moves the address by whole operands: most are aimed with it.
        let moved = if draw.chance(AIMED_BIT_OFFSETS) {
            bit_offset_move(&instruction, &state)
        } else {
            0
        };
        aim(&mut draw, &mut state, &m, to.wrapping_sub(moved), address32);
    }
    if reaches_runner(&bytes, &state) {
        return None;
    }
    Some(Case {
        code: Encoded::new(&bytes),
        state: State(state),
        tracking,
        seed: seed.unwrap_or_else(|| draw.next()),
    })
}

/// In a hundred: cases with MXCSR's flags kept, with MXCSR's start value, with its flags drawn, and with an
/// exception unmasked (of those with a mode drawn); `ldmxcsr` cases aimed at a value it takes.
const TRACKED: u64 = 50;
const DEFAULT_MXCSR: u64 = 40;
const FLAGS_DRAWN: u64 = 50;
const UNMASKED: u64 = 30;
const LOADABLE_MXCSR: u64 = 70;
/// In a hundred: floats drawn negative. An xmm register is single or double lanes or integers.
const NEGATIVE: u64 = 50;
const VECTOR_KINDS: u64 = 3;
/// Data seeds tried for a value `ldmxcsr` takes.
const SEEDS_TRIED: usize = 8;

/// MXCSR at the start of a case: mostly the default mode, with or without flags, else any mode.
fn mxcsr(draw: &mut Draw) -> u32 {
    use wcontainers_x86::{MXCSR_AT_START, MXCSR_FLAGS, MXCSR_MASK_SHIFT, MXCSR_MODE};
    let flags = draw.next() as u32 & MXCSR_FLAGS;
    if draw.chance(DEFAULT_MXCSR) {
        return MXCSR_AT_START | if draw.chance(FLAGS_DRAWN) { flags } else { 0 };
    }
    let mut mode = draw.next() as u32 & MXCSR_MODE;
    if !draw.chance(UNMASKED) {
        mode |= MXCSR_FLAGS << MXCSR_MASK_SHIFT;
    }
    mode | flags
}

/// A float of `bits` bits weighted toward the edges: signed zeros, infinities, quiet and signaling
/// not-a-numbers, denormals, the smallest and largest normals, one, and anything.
fn float(draw: &mut Draw, bits: u32) -> u128 {
    let fraction = if bits == u32::BITS {
        f32::MANTISSA_DIGITS
    } else {
        f64::MANTISSA_DIGITS
    } - 1;
    let all = u128::MAX >> (u128::BITS - bits);
    let sign = 1u128 << (bits - 1);
    let exponent = all >> 1 & !((1 << fraction) - 1);
    let quiet = 1u128 << (fraction - 1);
    let any = u128::from(draw.next()) & all;
    let some_fraction = any & ((1 << fraction) - 1);
    let edges = [
        0,
        exponent,
        exponent | quiet | some_fraction,
        exponent | (some_fraction & !quiet).max(1),
        some_fraction,
        1 << fraction,
        exponent - 1,
        (exponent >> 1) & exponent,
    ];
    // Every edge as often as anything.
    let pick = draw.below(edges.len() as u64 + 1) as usize;
    let Some(&edge) = edges.get(pick) else {
        return any;
    };
    edge | if draw.chance(NEGATIVE) { sign } else { 0 }
}

/// An xmm register's value: single or double precision lanes, or integers.
fn vector(draw: &mut Draw) -> u128 {
    let lanes = |draw: &mut Draw, bits: u32| {
        (0..u128::BITS / bits).fold(0, |v, n| v | float(draw, bits) << (n * bits))
    };
    match draw.below(VECTOR_KINDS) {
        0 => lanes(draw, u32::BITS),
        1 => lanes(draw, u64::BITS),
        _ => u128::from(draw.value()) << u64::BITS | u128::from(draw.value()),
    }
}

/// A data seed and an offset in its data that `ldmxcsr` takes, when one turns up in a few seeds.
fn loadable_mxcsr(draw: &mut Draw) -> Option<(u64, u64)> {
    let unwritable = !wcontainers_x86::MXCSR_WRITABLE;
    let mut data = vec![0u8; DATA_BYTES as usize];
    (0..SEEDS_TRIED).find_map(|_| {
        let seed = draw.next();
        crate::layout::fill(seed, &mut data);
        let start = draw.below(DATA_BYTES) as usize;
        (0..data.len() - size_of::<u32>())
            .map(|o| (start + o) % (data.len() - size_of::<u32>()))
            .find(|&o| {
                let word = u32::from_le_bytes(data[o..o + size_of::<u32>()].try_into().unwrap());
                word & unwritable == 0
            })
            .map(|o| (seed, o as u64))
    })
}

/// A reserved NOP's encoding with `66`, `F2` and `F3` drawn before it in any order and REX.W drawn, so that
/// the cases reach every encoding of the space (`endbr64`, `rdsspq`, MPX's, those of features not yet known),
/// as the translator's decoder gives them all as reserved NOPs; the encoding as built where it does not.
fn with_drawn_prefixes(draw: &mut Draw, bytes: Vec<u8>) -> Vec<u8> {
    let mut legacy: Vec<u8> = [OPERAND_SIZE, REPNE, REP]
        .into_iter()
        .filter(|_| draw.chance(40))
        .collect();
    for k in (1..legacy.len()).rev() {
        legacy.swap(k, draw.below(k as u64 + 1) as usize);
    }
    let opcode = bytes
        .iter()
        .position(|&b| b == ESCAPE)
        .expect("a two-byte opcode");
    let mut drawn = legacy;
    drawn.extend_from_slice(&bytes[..opcode]);
    if draw.chance(DRAWN_REX_W) {
        match drawn.last_mut() {
            Some(rex) if is_rex(*rex) => *rex |= REX_W,
            _ => drawn.push(REX | REX_W),
        }
    }
    drawn.extend_from_slice(&bytes[opcode..]);
    let reserved = wcontainers_x86::decode(&drawn).is_some_and(|(i, length)| {
        usize::from(length) == drawn.len() && i.iced.mnemonic() == Mnemonic::Reservednop
    });
    if reserved { drawn } else { bytes }
}

/// How far a bit offset in a register (`bt [m], r` and its kin) moves the address: whole operands, by the
/// offset's sign-extended value. Zero for every other instruction, and where the offset's register is also
/// in the address, so that it cannot be aimed apart from it.
fn bit_offset_move(instruction: &Instruction, state: &[u128]) -> u64 {
    if !matches!(
        instruction.mnemonic(),
        Mnemonic::Bt | Mnemonic::Bts | Mnemonic::Btr | Mnemonic::Btc
    ) || instruction.op0_kind() != OpKind::Memory
        || instruction.op1_kind() != OpKind::Register
    {
        return 0;
    }
    let offset = instruction.op1_register();
    let field = field_of(offset);
    if field.is_none()
        || [instruction.memory_base(), instruction.memory_index()]
            .iter()
            .any(|&r| field_of(r) == field)
    {
        return 0;
    }
    let bits = offset.size() as u32 * 8;
    let value = register_value(offset, state).expect("a general register");
    let signed = (value << (u64::BITS - bits)) as i64 >> (u64::BITS - bits);
    ((signed >> bits.trailing_zeros()) * i64::from(bits / 8)) as u64
}

/// The value of a register an address is made of, from the state: a general register at its width, a
/// segment's base (FS and GS; the others are zero in 64-bit mode).
/// rax's field.
const RAX_FIELD: usize = wcontainers_x86::RAX as usize;

fn register_value(r: Register, state: &[u128]) -> Option<u64> {
    match r {
        Register::FS => Some(state[FS] as u64),
        Register::GS => Some(state[GS] as u64),
        Register::ES | Register::CS | Register::SS | Register::DS => Some(0),
        // `xlat`'s index.
        Register::AL => Some(state[RAX_FIELD] as u64 & u64::from(u8::MAX)),
        _ => {
            let value = state[field_of(r)?] as u64;
            Some(match r.size() {
                8 => value,
                size => value & (u64::MAX >> (u64::BITS - size as u32 * 8)),
            })
        }
    }
}

/// Whether any memory the instruction reaches from `state` (its operands, the stack, the string
/// registers, a bit offset's move) is the reference runner's own, where the CPU would read or write the
/// runner itself where a clean process faults.
fn reaches_runner(bytes: &[u8], state: &[u128]) -> bool {
    let runner = crate::layout::x86_64::RUNNER;
    let instruction = Decoder::with_ip(
        wcontainers_x86::MODE_BITS,
        bytes,
        CODE,
        DecoderOptions::NONE,
    )
    .decode();
    if instruction.is_invalid() {
        return false;
    }
    let moved = bit_offset_move(&instruction, state);
    let mut factory = InstructionInfoFactory::new();
    factory
        .info(&instruction)
        .used_memory()
        .iter()
        .filter(|m| m.access() != OpAccess::NoMemAccess)
        .any(|m| {
            let value = |r: Register| match r {
                Register::None => Some(0),
                Register::RIP => Some(instruction.next_ip()),
                r => register_value(r, state),
            };
            let (Some(base), Some(index), Some(segment)) =
                (value(m.base()), value(m.index()), value(m.segment()))
            else {
                return true;
            };
            let mut effective = m
                .displacement()
                .wrapping_add(base)
                .wrapping_add(index.wrapping_mul(u64::from(m.scale())))
                .wrapping_add(moved);
            if m.address_size() == iced_x86::CodeSize::Code32 {
                effective &= u64::from(u32::MAX);
            }
            let start = effective.wrapping_add(segment);
            let end = start.saturating_add(m.memory_size().size().max(1) as u64);
            start < runner.end && end > runner.start
        })
}

/// A branch into its own bytes, which our side runs again where the reference stops there (and so never
/// stops when it lands on its own start).
fn branches_into_itself(bytes: &[u8]) -> bool {
    let instruction = Decoder::with_ip(
        wcontainers_x86::MODE_BITS,
        bytes,
        CODE,
        DecoderOptions::NONE,
    )
    .decode();
    instruction.op_count() == 1
        && instruction.op0_kind() == iced_x86::OpKind::NearBranch64
        && (CODE..CODE + bytes.len() as u64).contains(&instruction.near_branch64())
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
        // `mov` with a full address in the instruction: the address where the draw says.
        let (size, to) = if address32 {
            (4, target(draw, 8) & 0xffff_ffff)
        } else {
            (8, target(draw, 8))
        };
        return MemoryOperand::new(
            Register::None,
            Register::None,
            1,
            to as i64,
            size,
            false,
            segment,
        );
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
        // With no register the displacement is the address: one the draw aims, as registers are aimed, so
        // nothing reaches the reference runner's own memory.
        _ if base == Register::None && index == Register::None => {
            target(draw, 16).min(0x7fff_ffff) as i64
        }
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
    // Only the index register's own width counts (`xlat`'s al).
    let index_value = register_value(m.index, state).unwrap_or(0);
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
            if base < USER_TOP {
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

/// A case of bytes in no form, drawn from `draw`, with the data area from `seed`: half are random bytes iced
/// decodes as no instruction, half a run of prefixes before an instruction of a form or a VEX or EVEX one,
/// 13 to 16 bytes in all, so that both sides of the 15-byte limit are reached. A run that makes an instruction
/// of 15 bytes or fewer is kept only when the instruction's meaning is written, so that what is compared is
/// the decoding.
fn no_form(draw: u64, seed: u64) -> Option<Case> {
    const PREFIXES: [u8; 27] = [
        0x26, 0x2e, 0x36, 0x3e, 0x64, 0x65, 0x66, 0x67, 0xf0, 0xf2, 0xf3, 0x40, 0x41, 0x42, 0x43,
        0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4a, 0x4b, 0x4c, 0x4d, 0x4e, 0x4f,
    ];
    let mut draw = Draw(draw);
    let forms = X86_64::forms();
    let base = case(
        forms[draw.below(forms.len() as u64) as usize].code,
        draw.next(),
    )?;
    let bytes: Vec<u8> = if draw.chance(50) {
        let random: Vec<u8> = (0..15).map(|_| draw.next() as u8).collect();
        let (i, length) = wcontainers_x86::decode(&random)?;
        if !i.iced.is_invalid() || i.too_long {
            return None;
        }
        random[..usize::from(length)].to_vec()
    } else {
        let body: Vec<u8> = match draw.below(4) {
            // A VEX or EVEX start after a prefix that makes it invalid.
            0 => {
                let mut v = vec![[0xc4, 0xc5, 0x62][draw.below(3) as usize]];
                v.extend((0..6).map(|_| draw.next() as u8));
                v
            }
            _ => base.code.code().to_vec(),
        };
        let total = 13 + draw.below(4) as usize;
        if body.len() >= total {
            return None;
        }
        let mut run: Vec<u8> = (0..total - body.len())
            .map(|_| PREFIXES[draw.below(PREFIXES.len() as u64) as usize])
            .collect();
        run.extend(body);
        // The whole encoding must be in the case's bytes: past them the reference reads its filler.
        let (i, length) = wcontainers_x86::decode(&run)?;
        if !(i.too_long || i.iced.is_invalid() || wcontainers_x86::written(&i)) {
            return None;
        }
        run.truncate(usize::from(length));
        // Prefixes move a branch's target with its end.
        if branches_into_itself(&run) {
            return None;
        }
        // The same answer from the bytes alone.
        let (again, same) = wcontainers_x86::decode(&run)?;
        if same != length || again.too_long != i.too_long {
            return None;
        }
        run
    };
    if X86_64::excluded(&bytes).is_some() || reaches_runner(&bytes, &base.state.0) {
        return None;
    }
    Some(Case {
        code: Encoded::new(&bytes),
        state: base.state,
        tracking: false,
        seed,
    })
}

/// A case of `code` from the state a thread starts in: registers zero, the stack in the data area.
pub fn plain(code: &[u8]) -> Case {
    let mut state = vec![0u128; FIELDS];
    state[RSP] = (DATA + PAGE_SIZE).into();
    state[RIP] = CODE.into();
    state[RFLAGS] = 0x202;
    state[MXCSR] = wcontainers_x86::MXCSR_AT_START.into();
    state[FCW] = wcontainers_x86::FCW_AT_START.into();
    Case {
        code: Encoded::new(code),
        state: State(state),
        tracking: false,
        seed: 0x5eed,
    }
}

/// The hand-made cases: the 32-bit system call gates, which this phase answers by design with general
/// protection at the instruction (plan 5.2), and a few stops compared with the reference.
fn hand_cases() -> Vec<Designed> {
    let designed = |name, code: &[u8], stop, compared| Designed {
        name,
        case: plain(code),
        stop: Some(stop),
        fields: vec![(RIP, CODE.into())],
        compared,
    };
    vec![
        designed("int 0x80", &[0xcd, 0x80], Stop::GeneralProtection, false),
        designed("sysenter", &[0x0f, 0x34], Stop::GeneralProtection, false),
        designed("ud2", &[0x0f, 0x0b], Stop::Illegal, true),
        designed("int 0x81", &[0xcd, 0x81], Stop::GeneralProtection, true),
        Designed {
            name: "int 4",
            case: plain(&[0xcd, 0x04]),
            stop: Some(Stop::GeneralProtection),
            fields: vec![(RIP, (CODE + 2).into())],
            compared: true,
        },
        // The destination's address adds the moved stack pointer before the address-size prefix wraps it.
        designed(
            "pop [esp + d] wrapping past 2^32",
            &[0x67, 0x8f, 0x84, 0x24, 0xf8, 0xef, 0xff, 0xbb],
            Stop::MemoryFault(0),
            true,
        ),
        Designed {
            name: "endbr64",
            case: plain(&[0xf3, 0x0f, 0x1e, 0xfa]),
            stop: Some(Stop::Landed(CODE + 4)),
            fields: vec![],
            compared: true,
        },
        Designed {
            name: "int3",
            case: plain(&[0xcc]),
            stop: Some(Stop::Breakpoint(SI_KERNEL)),
            fields: vec![(RIP, (CODE + 1).into())],
            compared: true,
        },
    ]
    .into_iter()
    .chain(cpuid_cases())
    .collect()
}

/// `cpuid` gives the table's answer for its leaf and sub-leaf, read from the low halves of rax and rcx, with
/// the upper halves of all four registers cleared (plan 5.10).
fn cpuid_cases() -> Vec<Designed> {
    use wcontainers_x86::cpuid::{EXTENDED, MAX_BASIC, MAX_EXTENDED, answer, leaves, sub_leaves};
    const CPUID: [u8; 2] = [0x0f, 0xa2];
    const HIGH: u128 = 0x5a5a_5a5a << u32::BITS;
    let [rax, rbx, rcx, rdx] = [
        wcontainers_x86::RAX,
        wcontainers_x86::RBX,
        wcontainers_x86::RCX,
        wcontainers_x86::RDX,
    ]
    .map(usize::from);
    // Every leaf with each of its sub-leaves, the first past them and the last; leaves past the highest.
    let asked: Vec<(u32, u32)> = leaves()
        .chain([MAX_BASIC + 1, EXTENDED - 1, MAX_EXTENDED + 1, u32::MAX])
        .flat_map(|leaf| {
            (0..=sub_leaves(leaf))
                .chain([u32::MAX])
                .map(move |sub_leaf| (leaf, sub_leaf))
        })
        .collect();
    asked
        .into_iter()
        .map(|(leaf, sub_leaf)| {
            let mut case = plain(&CPUID);
            case.state.0[rax] = HIGH | u128::from(leaf);
            case.state.0[rcx] = HIGH | u128::from(sub_leaf);
            case.state.0[rbx] = HIGH;
            case.state.0[rdx] = HIGH;
            let mut fields: Vec<(usize, u128)> = [rax, rbx, rcx, rdx]
                .into_iter()
                .zip(answer(false, leaf, sub_leaf).map(u128::from))
                .collect();
            fields.push((RIP, (CODE + CPUID.len() as u64).into()));
            Designed {
                name: Box::leak(format!("cpuid {leaf:#x} {sub_leaf:#x}").into_boxed_str()),
                case,
                stop: Some(Stop::Landed(CODE + CPUID.len() as u64)),
                fields,
                compared: false,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reserved NOP forms' cases reach encodings with each of `66`, `F2` and `F3` first, and with REX.W.
    #[test]
    fn reserved_nop_cases_draw_prefixes() {
        const DRAWS: u64 = 4096;
        let cases: Vec<Vec<u8>> = (0..DRAWS)
            .filter_map(|draw| case(Code::Reservednop_rm32_r32_0F1E, draw))
            .map(|c| c.code.code().to_vec())
            .collect();
        for prefix in [OPERAND_SIZE, REPNE, REP] {
            assert!(cases.iter().any(|b| b[0] == prefix), "{prefix:#x}");
        }
        assert!(
            cases
                .iter()
                .any(|b| b.iter().any(|&x| is_rex(x) && x & REX_W != 0))
        );
    }

    /// Accesses into the reference runner's own memory are found: through a bit offset's move and a plain
    /// load (the independent check's cases), and not for the same load into the data area.
    #[test]
    fn accesses_into_the_runner_are_found() {
        let in_runner = crate::layout::x86_64::RUNNER.start + 0x9_adeb;
        let mut btr = plain(&[0x4c, 0x0f, 0xb3, 0x14, 0x2d, 0, 0, 0, 0]);
        btr.state.0[5] = 0x4400_2ffb;
        btr.state.0[10] = 0xe04b_efbd;
        assert!(reaches_runner(btr.code.code(), &btr.state.0));
        let mut load = plain(&[0x48, 0x8b, 0x03]);
        load.state.0[3] = in_runner.into();
        assert!(reaches_runner(load.code.code(), &load.state.0));
        load.state.0[3] = DATA.into();
        assert!(!reaches_runner(load.code.code(), &load.state.0));
    }
}
