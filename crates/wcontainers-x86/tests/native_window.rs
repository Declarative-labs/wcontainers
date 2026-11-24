//! The in-process way's two forms of a guest access read back through iced's decoder (docs/x86-native-output.md
//! sections 4 and 7): every access the output makes to guest memory is on GS at a 32-bit address, or on rsp, which
//! holds B plus the guest's rsp and is written only in the window's form, so that none, run or only speculated, leaves
//! the window; the exact form probes the address an access reaches before it.

use iced_x86::{
    Code, Decoder, DecoderOptions, Formatter, Instruction as Iced, InstructionInfoFactory,
    IntelFormatter, Mnemonic, OpAccess, OpKind, Register,
};
use wcontainers_abi::PAGE_SIZE;
use wcontainers_guest_cpu::HostCode;
use wcontainers_guest_cpu::position::{Held, Side};
use wcontainers_translator::{LookAhead, translate_native_for};
use wcontainers_x86::area::{PROBE_PAGE, WINDOW, WINDOW_NEGATED};
use wcontainers_x86::carry::{T, U};
use wcontainers_x86::native::{Check, HostMatches, Target, Way};
use wcontainers_x86::{Offer, V3, X86_64, decode};

const WAYS: [Way; 2] = [
    Way::InProcess(Check::Wrapping),
    Way::InProcess(Check::Exact),
];
/// Where the pieces start, inside a window's 2^32 bytes.
const AT: u64 = 0x40_1000;

fn target() -> Target {
    Target {
        host: HostMatches {
            five_level_paging: true,
            partial_writes: true,
            undefined_results: true,
        },
        offer: Offer::new(V3).expect("the v3 list has meanings"),
    }
}

fn bytes(hex: &str) -> Vec<u8> {
    let hex: String = hex.split_whitespace().collect();
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex"))
        .collect()
}

fn knob(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| {
        let v = v.trim();
        v.strip_prefix("0x").map_or_else(
            || v.parse().expect("a number"),
            |hex| u64::from_str_radix(hex, 16).expect("a hex number"),
        )
    })
}

fn host(code: &[u8]) -> Vec<(usize, Iced)> {
    let mut decoder = Decoder::with_ip(64, code, 0, DecoderOptions::NONE);
    let mut out = Vec::new();
    while decoder.can_decode() {
        let at = decoder.position();
        let i = decoder.decode();
        assert!(!i.is_invalid(), "host code decodes at {at}: {code:02x?}");
        out.push((at, i));
    }
    out
}

fn listing(code: &[u8]) -> String {
    let mut formatter = IntelFormatter::new();
    let mut text = String::new();
    for (at, i) in host(code) {
        let mut line = String::new();
        formatter.format(&i, &mut line);
        text += &format!("{at:4} {line}\n");
    }
    text
}

fn piece(guest: &[u8], way: Way, counted: bool) -> Option<HostCode> {
    let mut piece =
        translate_native_for::<X86_64>(guest, AT, LookAhead::Full, counted, &way, &target())?;
    piece.finish();
    Some(piece)
}

fn is_32(r: Register) -> bool {
    r.is_gpr32()
}

/// What the sweep knows of rsi and rdi before a host instruction: cut to 32 bits since last written, rdi then moved
/// by the window's base, and t holding that base.
#[derive(Clone, Copy, Default)]
struct Starts {
    rsi_cut: bool,
    rdi_cut: bool,
    rdi_rebased: bool,
    t_window: bool,
    /// By register number: holding the window's base B, and holding a value below 2^32 plus a ret's pop.
    window: [bool; 16],
    small: [bool; 16],
}

/// The most a host `lea rsp, [rsp + d]` (a guest return's pop) or the cut guest rsp's `lea` may add.
const POPPED: i64 = u16::MAX as i64 + 1 + 8;

fn number(r: Register) -> Option<usize> {
    r.is_gpr().then(|| r.full_register().number())
}

impl Starts {
    fn after(mut self, i: &Iced) -> Self {
        let same = |r: Register| {
            i.code() == Code::Mov_r32_rm32
                && i.op1_kind() == OpKind::Register
                && i.op0_register() == r
                && i.op1_register() == r
        };
        let window = i.code() == Code::Mov_r64_rm64
            && i.op0_register() == T
            && i.op1_kind() == OpKind::Memory
            && i.memory_segment() == Register::FS
            && i.memory_base() == Register::None
            && i.memory_displacement64() == WINDOW as u64;
        let rebase = i.code() == Code::Lea_r64_m
            && i.op0_register() == Register::RDI
            && i.memory_base() == Register::RDI
            && i.memory_index() == T
            && i.memory_index_scale() == 1
            && i.memory_displacement64() == 0;
        let rebased = rebase && self.t_window && self.rdi_cut && !self.rdi_rebased;
        let mut info = InstructionInfoFactory::new();
        let used = info.info(i).used_registers().to_vec();
        let written = |r: Register| {
            used.iter().any(|u| {
                u.register().full_register() == r
                    && matches!(
                        u.access(),
                        OpAccess::Write
                            | OpAccess::ReadWrite
                            | OpAccess::CondWrite
                            | OpAccess::ReadCondWrite
                    )
            })
        };
        let (rsi, rdi, t) = (written(Register::RSI), written(Register::RDI), written(T));
        self.rsi_cut = same(Register::ESI) || self.rsi_cut && !rsi;
        self.rdi_cut = same(Register::EDI) || self.rdi_cut && !rdi;
        self.rdi_rebased = rebased || self.rdi_rebased && !rdi;
        if same(Register::EDI) {
            self.rdi_rebased = false;
        }
        if rebased {
            self.rdi_cut = true;
        }
        self.t_window = window || self.t_window && !t;
        let destination = (i.op0_kind() == OpKind::Register)
            .then(|| i.op0_register())
            .filter(|&r| number(r).is_some());
        for u in &used {
            let Some(n) = number(u.register()) else {
                continue;
            };
            if matches!(
                u.access(),
                OpAccess::Read | OpAccess::CondRead | OpAccess::NoMemAccess
            ) {
                continue;
            }
            let own =
                destination.is_some_and(|d| d.full_register() == u.register().full_register());
            self.window[n] = own
                && i.code() == Code::Mov_r64_rm64
                && i.op1_kind() == OpKind::Memory
                && i.memory_segment() == Register::FS
                && i.memory_base() == Register::None
                && i.memory_displacement64() == WINDOW as u64;
            self.small[n] = own
                && (destination.is_some_and(|d| d.is_gpr32())
                    || i.code() == Code::Lea_r64_m
                        && i.memory_index() == Register::None
                        && number(i.memory_base()).is_some_and(|b| self.small[b])
                        && (0..=POPPED).contains(&(i.memory_displacement64() as i64)));
        }
        self
    }
}

/// What is wrong with a host write of rsp, if anything: rsp keeps B plus a guest rsp of at most 2^32 plus a return's
/// pop by the stack's own instructions, a return's `lea rsp, [rsp + d]`, or `lea rsp, [x + y]` of B and a cut value.
fn rsp_outside(i: &Iced, starts: Starts) -> Option<&'static str> {
    let mut info = InstructionInfoFactory::new();
    let writes = info
        .info(i)
        .used_registers()
        .iter()
        .any(|u| u.register().full_register() == Register::RSP && u.access() != OpAccess::Read);
    if !writes || matches!(i.mnemonic(), Mnemonic::Push | Mnemonic::Pop) {
        return None;
    }
    let lea = i.code() == Code::Lea_r64_m && i.op0_register() == Register::RSP;
    let returned = lea
        && i.memory_base() == Register::RSP
        && i.memory_index() == Register::None
        && (0..=POPPED).contains(&(i.memory_displacement64() as i64));
    let (base, index) = (number(i.memory_base()), number(i.memory_index()));
    let made = lea
        && i.memory_displacement64() == 0
        && i.memory_index_scale() == 1
        && match (base, index) {
            (Some(b), Some(x)) => {
                starts.window[b] && starts.small[x] || starts.window[x] && starts.small[b]
            }
            _ => false,
        };
    (!(returned || made)).then_some("rsp written outside the window's rule")
}

/// What is wrong with a string's accesses, if anything: its rsi side on GS at an rsi cut to 32 bits, its rdi side at
/// an rdi cut and then moved by the window's base.
fn string_outside(i: &Iced, starts: Starts) -> Option<&'static str> {
    let kinds: Vec<OpKind> = (0..i.op_count()).map(|n| i.op_kind(n)).collect();
    if kinds.contains(&OpKind::MemorySegRSI)
        && !(i.memory_segment() == Register::GS && starts.rsi_cut)
    {
        return Some("a string's rsi side not on GS at a cut rsi");
    }
    if kinds.contains(&OpKind::MemoryESRDI) && !starts.rdi_rebased {
        return Some("a string's rdi side not at a cut and rebased rdi");
    }
    if kinds.iter().any(|k| {
        matches!(
            k,
            OpKind::MemorySegSI | OpKind::MemorySegESI | OpKind::MemoryESDI | OpKind::MemoryESEDI
        )
    }) {
        return Some("a string with the guest's 0x67 not answered by the inside");
    }
    None
}

/// What is wrong with host instruction `i`'s memory access under the window's rule, if anything: guest memory only
/// on GS at a 32-bit address; else the thread area on FS, the lookup's second level, or the exact form's probe.
fn outside(i: &Iced, starts: Starts) -> Option<&'static str> {
    let mut info = InstructionInfoFactory::new();
    let touches = info
        .info(i)
        .used_memory()
        .iter()
        .any(|m| m.access() != OpAccess::NoMemAccess);
    if !touches {
        return None;
    }
    if i.mnemonic() == Mnemonic::Jmp && i.memory_base() == U && i.memory_index() == T {
        return None;
    }
    if string(i) {
        return string_outside(i, starts);
    }
    let explicit = (0..i.op_count()).any(|n| i.op_kind(n) == OpKind::Memory);
    if matches!(i.mnemonic(), Mnemonic::Push | Mnemonic::Pop) && !explicit {
        return None;
    }
    // On rsp as written, at most the guard page below it.
    let on_stack = i.memory_base() == Register::RSP
        && i.memory_index() == Register::None
        && !matches!(i.memory_segment(), Register::FS | Register::GS)
        && i.memory_displacement64() as i64 >= -(PAGE_SIZE as i64)
        && !(matches!(
            i.mnemonic(),
            Mnemonic::Bt | Mnemonic::Bts | Mnemonic::Btr | Mnemonic::Btc
        ) && i.op1_kind() == OpKind::Register);
    if explicit && on_stack {
        return None;
    }
    let memory = (0..i.op_count()).find(|&n| {
        matches!(
            i.op_kind(n),
            OpKind::Memory | OpKind::MemorySegEDI | OpKind::MemorySegRDI
        )
    });
    let Some(n) = memory else {
        return Some("an implicit access (the stack's or a string's)");
    };
    match i.memory_segment() {
        Register::FS => return None,
        Register::GS => {}
        _ => return Some("not on FS or GS"),
    }
    let probe = i.mnemonic() == Mnemonic::Mov
        && i.memory_base() == U
        && i.memory_displacement64() as i64 == 1 - PROBE_PAGE as i64;
    if probe {
        return None;
    }
    let cut = match i.op_kind(n) {
        OpKind::MemorySegEDI => true,
        OpKind::MemorySegRDI => false,
        _ => match (i.memory_base(), i.memory_index()) {
            (Register::None, Register::None) => i.memory_displ_size() == 4,
            (Register::None, index) => is_32(index) || index.is_vector_register(),
            (base, _) => is_32(base),
        },
    };
    (!cut).then_some("a guest access not cut to 32 bits")
}

fn string(i: &Iced) -> bool {
    (0..i.op_count()).any(|n| {
        matches!(
            i.op_kind(n),
            OpKind::MemorySegSI
                | OpKind::MemorySegESI
                | OpKind::MemorySegRSI
                | OpKind::MemoryESDI
                | OpKind::MemoryESEDI
                | OpKind::MemoryESRDI
        )
    })
}

/// Bytes shaped like an instruction: some legacy prefixes, maybe a REX, an opcode map, then random bytes.
fn instruction_like(rng: &mut fastrand::Rng) -> [u8; 16] {
    const PREFIXES: [u8; 8] = [0x64, 0x65, 0x66, 0x67, 0xf2, 0xf3, 0xf0, 0x2e];
    const MAPS: [&[u8]; 5] = [&[], &[], &[0x0f], &[0x0f, 0x38], &[0x0f, 0x3a]];
    let mut code = [0; 16];
    rng.fill(&mut code);
    let mut at = 0;
    for _ in 0..rng.usize(0..3) {
        code[at] = PREFIXES[rng.usize(..PREFIXES.len())];
        at += 1;
    }
    if rng.bool() {
        code[at] = 0x40 | rng.u8(0..16);
        at += 1;
    }
    for &b in MAPS[rng.usize(..MAPS.len())] {
        code[at] = b;
        at += 1;
    }
    code
}

/// The string instructions, each with a rep prefix or none, with 0x66 or REX.W or neither.
fn a_string(rng: &mut fastrand::Rng) -> Vec<u8> {
    const OPCODES: [u8; 10] = [0xa4, 0xa5, 0xa6, 0xa7, 0xaa, 0xab, 0xac, 0xad, 0xae, 0xaf];
    const PREFIXES: [&[u8]; 6] = [&[], &[0xf3], &[0xf2], &[0x66], &[0xf3, 0x48], &[0x48]];
    let mut out = PREFIXES[rng.usize(..PREFIXES.len())].to_vec();
    out.push(OPCODES[rng.usize(..OPCODES.len())]);
    out
}

/// A seeded sweep of instruction mixes in both forms, counted and not: every host access to memory keeps to the
/// window's rule (the speculation rule of section 4).
#[test]
fn every_guest_access_is_cut_to_the_window() {
    let seed = knob("X86_WINDOW_SEED", 0x5eed);
    let cases = knob("X86_WINDOW_CASES", 20_000);
    let mut rng = fastrand::Rng::with_seed(seed);
    let (mut pieces, mut accesses, mut strings) = (0u64, 0u64, 0u64);
    for case in 0..cases {
        let mut guest = Vec::new();
        for _ in 0..rng.usize(1..12) {
            if rng.usize(..6) == 0 {
                guest.extend(a_string(&mut rng));
                continue;
            }
            let candidate = instruction_like(&mut rng);
            let Some((_, length)) = decode(&candidate) else {
                continue;
            };
            let one = &candidate[..usize::from(length)];
            guest.extend_from_slice(one);
        }
        for way in WAYS {
            for counted in [false, true] {
                let made = std::panic::catch_unwind(|| piece(&guest, way, counted));
                let Some(piece) = made.unwrap_or_else(|_| {
                    panic!("case {case} of seed {seed:#x} ({way:?}, counted {counted}): guest {guest:02x?}")
                }) else {
                    continue;
                };
                pieces += 1;
                let mut starts = Starts::default();
                for (at, i) in host(&piece.bytes) {
                    strings += u64::from(string(&i));
                    if let Some(what) = outside(&i, starts).or_else(|| rsp_outside(&i, starts)) {
                        panic!(
                            "case {case} of seed {seed:#x} ({way:?}, counted {counted}): {what} at {at}\nguest \
                             {guest:02x?}\n{}",
                            listing(&piece.bytes)
                        );
                    }
                    accesses += u64::from(i.op_count() > 0);
                    starts = starts.after(&i);
                }
            }
        }
    }
    println!(
        "seed {seed:#x}: {cases} cases, {pieces} pieces, {accesses} host instructions, {strings} strings, 0 outside"
    );
}

/// The host instructions of the piece made of `hex`, by mnemonic.
fn shape(hex: &str, way: Way) -> Vec<Mnemonic> {
    let piece = piece(&bytes(hex), way, false).expect("a piece");
    host(&piece.bytes)
        .iter()
        .map(|(_, i)| i.mnemonic())
        .collect()
}

/// The host instructions of the piece made of `hex` up to its first exit.
fn decoded(hex: &str, way: Way) -> Vec<Iced> {
    let piece = piece(&bytes(hex), way, false).expect("a piece");
    host(&piece.bytes).into_iter().map(|(_, i)| i).collect()
}

const WRAPPING: Way = Way::InProcess(Check::Wrapping);
const EXACT: Way = Way::InProcess(Check::Exact);
/// The exact form's probe, before the access it guards.
const PROBE: [Mnemonic; 5] = [
    Mnemonic::Rorx,
    Mnemonic::Mov,
    Mnemonic::Lea,
    Mnemonic::Not,
    Mnemonic::Mov,
];

#[test]
fn a_plain_operand_is_cut_and_probed() {
    use Mnemonic::*;
    // mov rax, [rbx+rcx*8+10h]; ret
    let hex = "48 8b 44 cb 10 c3";
    let wrapping = decoded(hex, WRAPPING);
    assert_eq!(wrapping[0].code(), Code::Mov_r64_rm64);
    assert_eq!(
        (
            wrapping[0].memory_segment(),
            wrapping[0].memory_base(),
            wrapping[0].memory_index()
        ),
        (Register::GS, Register::EBX, Register::ECX)
    );
    assert_eq!(wrapping[0].memory_displacement64(), 0x10);
    let exact = shape(hex, EXACT);
    assert_eq!(exact[0], Lea);
    assert_eq!(exact[1..6], PROBE);
    let access = &decoded(hex, EXACT)[6];
    assert_eq!(
        (
            access.memory_segment(),
            access.memory_base(),
            access.memory_index()
        ),
        (Register::GS, Register::R11D, Register::None)
    );
    // mov rax, [rbx]: the probe on rbx itself.
    let exact = decoded("48 8b 03 c3", EXACT);
    assert_eq!(exact[0].mnemonic(), Rorx);
    assert_eq!(exact[0].op1_register(), Register::RBX);
    assert_eq!(exact[5].memory_base(), Register::EBX);
    // The guest's own 0x67: its address is below 2^32 already, so neither form probes.
    let wrapped = decoded("67 48 8b 03 c3", EXACT);
    assert_eq!(wrapped[0].memory_base(), Register::EBX);
    assert_eq!(wrapped[0].memory_segment(), Register::GS);
}

#[test]
fn the_stack_runs_as_written() {
    use Mnemonic::*;
    // push rax; pop rbx; push qword [rsp+8]; mov rax, [rsp-8]; ret: as the guest wrote them in both forms.
    let hex = "50 5b ff 74 24 08 48 8b 44 24 f8 c3";
    for way in WAYS {
        let made = decoded(hex, way);
        assert_eq!(made[0].code(), Code::Push_r64);
        assert_eq!(made[1].code(), Code::Pop_r64);
        assert_eq!(
            (made[2].code(), made[2].memory_base()),
            (Code::Push_rm64, Register::RSP)
        );
        assert_eq!(
            (made[3].code(), made[3].memory_base()),
            (Code::Mov_r64_rm64, Register::RSP)
        );
        // The return's read of its target on rsp.
        assert_eq!(
            (made[4].op0_register(), made[4].memory_base()),
            (T, Register::RSP)
        );
    }
    // An index, or a displacement past the guard page below, is the window's form: on GS at esp in 32 bits.
    let indexed = decoded("48 8b 04 cc c3", WRAPPING);
    assert_eq!(
        (indexed[0].memory_segment(), indexed[0].memory_base()),
        (Register::GS, Register::ESP)
    );
    let far = decoded("48 8b 84 24 00 e0 ff ff c3", WRAPPING);
    assert_eq!(far[0].memory_base(), Register::ESP);
    // In the exact form the offset is formed less B, then probed.
    let exact = shape("48 8b 04 cc c3", EXACT);
    assert_eq!(exact[..3], [Lea, Mov, Lea]);
    assert_eq!(exact[3..8], PROBE);
}

#[test]
fn rsp_as_a_value_is_less_b_and_written_back_cut() {
    use Mnemonic::*;
    // mov rbp, rsp: rsp less B into t, then the instruction on t.
    let read = decoded("48 89 e5 c3", WRAPPING);
    assert_eq!(
        read[0].memory_displacement64(),
        wcontainers_x86::area::WINDOW_NEGATED as u64
    );
    assert_eq!(
        (
            read[1].code(),
            read[1].memory_base(),
            read[1].memory_index()
        ),
        (Code::Lea_r64_m, Register::RSP, T)
    );
    assert_eq!(
        (read[2].op0_register(), read[2].op1_register()),
        (Register::RBP, T)
    );
    // sub rsp, 16: on t, then t cut to 32 bits and rsp B more; the exact form probes t first.
    let hex = "48 83 ec 10 c3";
    assert_eq!(shape(hex, WRAPPING)[..6], [Mov, Lea, Sub, Mov, Mov, Lea]);
    let exact = shape(hex, EXACT);
    assert_eq!(exact[..3], [Mov, Lea, Sub]);
    assert_eq!(exact[3..8], PROBE);
    assert_eq!(exact[8..11], [Mov, Mov, Lea]);
    // After the commit point the guest's rsp is t's.
    let piece = piece(&bytes(hex), WRAPPING, false).expect("a piece");
    let host = host(&piece.bytes);
    let (cut, _) = host[3];
    let span = piece.span(cut as u32).expect("a span");
    assert_eq!(span.side, Side::After);
    assert!(
        span.relocated
            .iter()
            .any(|r| r.guest == wcontainers_x86::RSP
                && r.held == Held::Register(wcontainers_x86::R11)),
        "rsp in t: {span:?}"
    );
    // leave: rbp's load, the guest's rsp in u from rbp's low half, rbp written (the commit), rsp B more.
    let leave = decoded("c9 c3", WRAPPING);
    assert_eq!(
        (leave[0].memory_segment(), leave[0].memory_base()),
        (Register::GS, Register::EBP)
    );
    assert_eq!(leave[1].code(), Code::Mov_r32_rm32);
    assert_eq!(
        (leave[4].op0_register(), leave[4].memory_displacement64()),
        (T, WINDOW as u64)
    );
    assert_eq!(leave[5].op0_register(), Register::RSP);
    assert_eq!(shape("c9 c3", EXACT)[..5], PROBE);
    // A memory operand beside rsp as a value: a guest register stands in, kept in the area.
    let beside = decoded("48 8b 64 24 08 c3", WRAPPING);
    assert_eq!(
        (beside[0].memory_segment(), beside[0].op1_register()),
        (Register::FS, Register::RAX)
    );
    assert_eq!(
        (beside[1].op0_register(), beside[1].memory_base()),
        (Register::RAX, Register::RSP)
    );
}

#[test]
fn implicit_operands_are_cut() {
    use Mnemonic::*;
    // xlat: probed on rbx + al, then on GS at ebx.
    let exact = decoded("d7 c3", EXACT);
    assert_eq!(exact[0].mnemonic(), Movzx);
    assert_eq!(
        exact[2..7].iter().map(|i| i.mnemonic()).collect::<Vec<_>>(),
        PROBE
    );
    assert_eq!(
        (
            exact[7].mnemonic(),
            exact[7].memory_segment(),
            exact[7].memory_base()
        ),
        (Xlatb, Register::GS, Register::EBX)
    );
    // With the guest's 0x67 the address is wrapped already.
    let wrapped = decoded("67 d7 c3", EXACT);
    assert_eq!(wrapped[0].mnemonic(), Xlatb);
    // maskmovq mm0, mm1: probed on rdi, stored at edi.
    let exact = decoded("0f f7 c1 c3", EXACT);
    assert_eq!(exact[0].op1_register(), Register::RDI);
    assert_eq!(
        (
            exact[5].mnemonic(),
            exact[5].memory_segment(),
            exact[5].op0_kind()
        ),
        (Maskmovq, Register::GS, OpKind::MemorySegEDI)
    );
}

#[test]
fn a_bit_test_is_probed_where_it_reaches() {
    use Mnemonic::*;
    // bt [rax], rcx: A = rax + 8 * (rcx SAR 6), probed; the access on GS at eax with rcx.
    let exact = decoded("48 0f a3 08 c3", EXACT);
    let mnemonics: Vec<Mnemonic> = exact.iter().map(|i| i.mnemonic()).collect();
    assert_eq!(mnemonics[..3], [Mov, Sarx, Lea]);
    assert_eq!(mnemonics[3..8], PROBE);
    assert_eq!(
        (
            exact[2].memory_base(),
            exact[2].memory_index(),
            exact[2].memory_index_scale()
        ),
        (Register::RAX, U, 8)
    );
    assert_eq!(
        (
            exact[8].mnemonic(),
            exact[8].memory_base(),
            exact[8].op1_register()
        ),
        (Bt, Register::EAX, Register::RCX)
    );
    // The guest's 0x67 wraps A as the meaning does: no probe.
    assert_eq!(shape("67 0f a3 08 c3", EXACT)[0], Bt);
    // A 16-bit offset borrows t for the count.
    let exact = shape("66 0f a3 08 c3", EXACT);
    assert_eq!(exact[..4], [Movsx, Mov, Sarx, Lea]);
    // bt fs:[eax], ecx under the guest's 0x67: the offset and the bit's words wrapped, then the base added; the
    // probe on that sum, the access on t holding the base plus the wrapped offset.
    let exact = decoded("67 64 0f a3 08 c3", EXACT);
    let mnemonics: Vec<Mnemonic> = exact.iter().map(|i| i.mnemonic()).collect();
    assert_eq!(
        mnemonics[..9],
        [Lea, Mov, Sarx, Movsxd, Lea, Mov, Mov, Lea, Rorx]
    );
    assert_eq!(exact[4].op0_register(), Register::R15D);
    assert_eq!(mnemonics[12..16], [Mov, Mov, Lea, Bt]);
    assert_eq!(exact[15].memory_base(), Register::R11D);
}

#[test]
fn a_segment_operand_adds_the_guests_base() {
    use Mnemonic::*;
    // mov rax, fs:[rbx]: t = rbx + the guest's FS base from the area; the access on GS at r11d.
    for way in WAYS {
        let host = decoded("64 48 8b 03 c3", way);
        assert_eq!(host[0].mnemonic(), Lea);
        assert_eq!(
            (host[1].mnemonic(), host[1].memory_segment()),
            (Mov, Register::FS)
        );
        assert_eq!(host[2].mnemonic(), Lea);
        let access = if way == EXACT { &host[8] } else { &host[3] };
        assert_eq!(
            (access.memory_segment(), access.memory_base()),
            (Register::GS, Register::R11D),
            "{way:?}"
        );
    }
}

#[test]
fn an_image_in_parts_is_probed_at_both_ends() {
    // fxsave64 [rax]: the start and the last byte probed.
    let exact = shape("48 0f ae 00 c3", EXACT);
    let before: Vec<Mnemonic> = exact
        .iter()
        .copied()
        .take_while(|&m| m != Mnemonic::Fxsave64)
        .collect();
    let probes = before.iter().filter(|&&m| m == Mnemonic::Rorx).count();
    assert_eq!(probes, 2, "{exact:?}");
}

#[test]
fn a_string_is_probed_cut_and_rebased() {
    use Mnemonic::*;
    // rep movsb: both starts probed (in both forms), cut, rdi moved by B around the copy on GS, then back.
    for way in WAYS {
        let host = decoded("f3 a4 c3", way);
        let mnemonics: Vec<Mnemonic> = host.iter().map(|i| i.mnemonic()).collect();
        assert_eq!(mnemonics[..5], PROBE, "{way:?}");
        assert_eq!(mnemonics[5..10], PROBE, "{way:?}");
        assert_eq!(mnemonics[10..15], [Mov, Mov, Mov, Lea, Movsb], "{way:?}");
        assert_eq!(host[0].op1_register(), Register::RSI);
        assert_eq!(host[5].op1_register(), Register::RDI);
        assert_eq!(host[14].memory_segment(), Register::GS);
        assert!(host[14].has_rep_prefix());
        assert_eq!(mnemonics[15..17], [Mov, Lea], "{way:?}");
        // While rebased the map holds rdi as the host's less B.
        let piece = piece(&bytes("f3 a4 c3"), way, false).expect("a piece");
        let at: usize = host[..14].iter().map(|i| i.len()).sum();
        let span = piece.span(at as u32).expect("a span");
        assert!(
            span.relocated
                .iter()
                .any(|r| r.guest == wcontainers_x86::RDI
                    && r.held == Held::Rebased(wcontainers_x86::RDI)),
            "{span:?}"
        );
    }
    // stos: rdi alone; lods: rsi alone, and no rebase.
    let stos = shape("aa c3", EXACT);
    assert_eq!(stos[5..9], [Mov, Mov, Lea, Stosb]);
    let lods = decoded("ac c3", EXACT);
    assert_eq!(lods[5].mnemonic(), Mov);
    assert_eq!(
        (lods[6].mnemonic(), lods[6].memory_segment()),
        (Lodsb, Register::GS)
    );
    // With the guest's 0x67 the inside answers it.
    assert!(!shape("67 aa c3", EXACT).contains(&Stosb));
}

/// Forms whose window sequences met a withheld register, a RIP-relative stack operand or an image at rsp: each
/// is made in both forms, counted and not.
#[test]
fn stack_and_withheld_forms_are_made() {
    for hex in [
        "66 41 5f c3",
        "41 5b c3",
        "66 41 8f c3 c3",
        "8f 05 ce ff ff ff c3",
        "66 41 52 c3",
        "41 53 c3",
        "0f ae 4c 64 f7 c3",
        "41 ff 33 c3",
        // push rsp; pop rsp; call rsp; rdfsbase rsp; lea rsp, [rbp-8]; xchg rsp, [rbx]
        "54 c3",
        "5c c3",
        "ff d4",
        "f3 48 0f ae c4 c3",
        "48 8d 65 f8 c3",
        "48 87 23 c3",
    ] {
        for way in WAYS {
            for counted in [false, true] {
                assert!(piece(&bytes(hex), way, counted).is_some(), "{hex}");
            }
        }
    }
}

/// The emitted code of guest sequences in the exact form, up to the piece's first call into the inside, as gadgets
/// for the speculation probe (docs/x86-native-output.md section 4, Speculation): each reads a byte or bit at a
/// refused address (or stores there) and loads the probe array's line it picks. Printed after the fixed part's
/// offsets of B and -B as `name form hex` lines, the form `cut` as emitted or `uncut` with each guest access's
/// address-size prefix and each cut of a register to its low half taken out, the control that must leak. Run by
/// the scratch folder's probe on an x86-64 machine.
#[test]
#[ignore = "prints the gadgets for the speculation probe"]
fn speculation_gadgets() {
    use iced_x86::Encoder;
    let gadgets = [
        // movzx eax, byte [rbx]; shl eax, 12; mov eax, [rax + 100000h]; ud2
        ("load", "0f b6 03 c1 e0 0c 8b 80 00 00 10 00 0f 0b"),
        // lodsb; movzx eax, al; shl eax, 12; mov eax, [rax + 100000h]; ud2
        ("lods", "ac 0f b6 c0 c1 e0 0c 8b 80 00 00 10 00 0f 0b"),
        // bt [rax], rcx; setc al; movzx eax, al; shl eax, 12; mov eax, [rax + 140000h]; ud2
        (
            "bt",
            "48 0f a3 08 0f 92 c0 0f b6 c0 c1 e0 0c 8b 80 00 00 14 00 0f 0b",
        ),
        // mov al, cl; scasb; setz al; movzx eax, al; shl eax, 12; mov eax, [rax + 140000h]; ud2
        (
            "scas",
            "88 c8 ae 0f 94 c0 0f b6 c0 c1 e0 0c 8b 80 00 00 14 00 0f 0b",
        ),
        // xor eax, eax; xlat; movzx eax, al; shl eax, 12; mov eax, [rax + 100000h]; ud2
        ("xlat", "31 c0 d7 0f b6 c0 c1 e0 0c 8b 80 00 00 10 00 0f 0b"),
        // mov rsp, rbx; pop rax; movzx eax, al; shl eax, 12; mov eax, [rax + 100000h]; ud2
        (
            "pop",
            "48 89 dc 58 0f b6 c0 c1 e0 0c 8b 80 00 00 10 00 0f 0b",
        ),
        // mov [rbx], al; ud2
        ("store", "88 03 0f 0b"),
        // pcmpeqb xmm1, xmm1; maskmovdqu xmm0, xmm1; ud2
        ("maskmov", "66 0f 76 c9 66 0f f7 c1 0f 0b"),
    ];
    println!("area {WINDOW:#x} {WINDOW_NEGATED:#x}");
    for (name, hex) in gadgets {
        let piece = piece(&bytes(hex), EXACT, false).expect("a piece");
        let emitted: Vec<Iced> = host(&piece.bytes)
            .into_iter()
            .map(|(_, i)| i)
            .take_while(|i| !(i.code() == Code::Lea_r64_m && i.memory_base() == Register::RIP))
            .collect();
        for form in ["cut", "uncut"] {
            let mut out = Vec::new();
            for i in &emitted {
                let mut i = *i;
                if form == "uncut" {
                    // A register cut to its low half (a string's start, the guest's rsp before the base is added),
                    // other than the probe's own.
                    let cut = i.code() == Code::Mov_r32_rm32
                        && i.op1_kind() == OpKind::Register
                        && i.op0_register() == i.op1_register()
                        && i.op0_register().full_register() != U;
                    if cut {
                        continue;
                    }
                    if i.memory_segment() == Register::GS && i.memory_base().is_gpr32() {
                        i.set_memory_base(i.memory_base().full_register());
                    }
                    if i.op0_kind() == OpKind::MemorySegEDI {
                        i.set_op0_kind(OpKind::MemorySegRDI);
                    }
                }
                let mut encoder = Encoder::new(64);
                encoder.encode(&i, 0).expect("encodes");
                out.extend(encoder.take_buffer());
            }
            let hex: String = out.iter().map(|b| format!("{b:02x}")).collect();
            println!("{name} {form} {hex}");
        }
    }
}
