//! The in-process way's two forms of a guest access read back through iced's decoder (docs/x86-native-output.md
//! sections 4 and 7): every access the output makes to guest memory is on GS at a 32-bit address, so that none, run
//! or only speculated, leaves the window; the exact form probes the address an access reaches before it.

use iced_x86::{
    Code, Decoder, DecoderOptions, Formatter, Instruction as Iced, InstructionInfoFactory,
    IntelFormatter, Mnemonic, OpAccess, OpKind, Register,
};
use wcontainers_guest_cpu::HostCode;
use wcontainers_guest_cpu::position::{Held, Side};
use wcontainers_translator::{LookAhead, translate_native_for};
use wcontainers_x86::area::PROBE_PAGE;
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

/// What is wrong with host instruction `i`'s memory access under the window's rule, if anything: guest memory only
/// on GS at a 32-bit address; else the thread area on FS, the lookup's second level, or the exact form's probe.
fn outside(i: &Iced) -> Option<&'static str> {
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

/// A seeded sweep of instruction mixes in both forms, counted and not: every host access to memory keeps to the
/// window's rule (the speculation rule of section 4). Strings are not made: their forms come later in XN2.
#[test]
fn every_guest_access_is_cut_to_the_window() {
    let seed = knob("X86_WINDOW_SEED", 0x5eed);
    let cases = knob("X86_WINDOW_CASES", 20_000);
    let mut rng = fastrand::Rng::with_seed(seed);
    let (mut pieces, mut accesses) = (0u64, 0u64);
    for case in 0..cases {
        let mut guest = Vec::new();
        for _ in 0..rng.usize(1..12) {
            let candidate = instruction_like(&mut rng);
            let Some((decoded, length)) = decode(&candidate) else {
                continue;
            };
            let one = &candidate[..usize::from(length)];
            let i = decoded.iced;
            // A direct branch may land inside another instruction, where a string may start.
            let direct = matches!(
                i.op0_kind(),
                OpKind::NearBranch16 | OpKind::NearBranch32 | OpKind::NearBranch64
            );
            if string(&i) || direct {
                continue;
            }
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
                for (at, i) in host(&piece.bytes) {
                    if let Some(what) = outside(&i) {
                        panic!(
                            "case {case} of seed {seed:#x} ({way:?}, counted {counted}): {what} at {at}\nguest \
                             {guest:02x?}\n{}",
                            listing(&piece.bytes)
                        );
                    }
                    accesses += u64::from(i.op_count() > 0);
                }
            }
        }
    }
    println!(
        "seed {seed:#x}: {cases} cases, {pieces} pieces, {accesses} host instructions, 0 outside"
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
fn a_stack_access_is_explicit_and_rsp_moves_last() {
    use Mnemonic::*;
    // push rax; pop rbx; ret
    let hex = "50 5b c3";
    let wrapping = shape(hex, WRAPPING);
    assert_eq!(wrapping[..5], [Mov, Lea, Mov, Lea, Mov]);
    let exact = shape(hex, EXACT);
    assert_eq!(exact[0], Lea);
    assert_eq!(exact[1..6], PROBE);
    assert_eq!(exact[6..8], [Mov, Lea]);
    assert_eq!(exact[8..13], PROBE);
    // The pop's commit point is rsp's move, after which t holds rbx until it is written.
    let piece = piece(&bytes(hex), WRAPPING, false).expect("a piece");
    let host = host(&piece.bytes);
    let (moved, _) = host[3];
    assert_eq!(piece.span(moved as u32).expect("a span").side, Side::Before);
    let (written, _) = host[4];
    let span = piece.span(written as u32).expect("a span");
    assert_eq!(span.side, Side::After);
    assert!(
        span.relocated
            .iter()
            .any(|r| r.guest == wcontainers_x86::RBX
                && r.held == Held::Register(wcontainers_x86::R11)),
        "rbx in t: {span:?}"
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
