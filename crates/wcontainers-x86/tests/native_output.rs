//! The native output's x86-64 code read back through iced's decoder (docs/x86-native-output.md section 7): every
//! piece decodes whole, with every span, exit and address place where the position map says, the guest's
//! copied bytes unchanged, and each route's sequence as the plan writes it.

use iced_x86::{
    Code, Decoder, DecoderOptions, Formatter, Instruction as Iced, IntelFormatter, Mnemonic,
    OpKind, Register,
};
use wcontainers_guest_cpu::position::Side;
use wcontainers_guest_cpu::{HostCode, Native, Route};
use wcontainers_translator::{LookAhead, translate_native_for};
use wcontainers_x86::carry::{COUNT, T, U, names_withheld};
use wcontainers_x86::native::{HostMatches, Target, Way};
use wcontainers_x86::{Offer, V3, X86_64, decode};

/// The ways this output is built for so far: no memory check.
const WAYS: [Way; 2] = [Way::Machine, Way::Process];
/// Where the sweep's pieces start.
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

/// A whole number from the environment, for the sweep's seed and size.
fn knob(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| {
        let v = v.trim();
        v.strip_prefix("0x").map_or_else(
            || v.parse().expect("a number"),
            |hex| u64::from_str_radix(hex, 16).expect("a hex number"),
        )
    })
}

/// The host instructions of `code`, decoded one after another from its first byte, with their offsets.
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

fn translate(code: &[u8], address: u64, way: Way, exact: bool) -> Option<HostCode> {
    translate_native_for::<X86_64>(code, address, LookAhead::Full, exact, &way, &target())
}

/// Checks one piece against its own position map and the guest code it came from, giving what was broken.
fn broken(piece: &HostCode, guest: &[u8], address: u64, way: Way) -> Option<String> {
    let decoded = host(&piece.bytes);
    let starts: std::collections::BTreeMap<usize, Iced> = decoded.iter().copied().collect();
    let at = |offset: u32| starts.get(&(offset as usize));
    for span in &piece.spans {
        at(span.start)?;
    }
    for exit in &piece.exits {
        let i = at(exit.start)?;
        let fits = match exit.retarget {
            None => i.code() == Code::Jmp_rel32_64,
            Some(field) => i.is_jcc_near() && field as usize == i.len() - 4,
        };
        if !fits {
            return Some(format!("exit {exit:?} is {:?}", i.code()));
        }
    }
    for place in &piece.places {
        let i = at(place.start)?;
        if i.code() != Code::Mov_r64_imm64 || i.immediate64() != place.address {
            return Some(format!("place {place:?} is {i:?}"));
        }
    }
    for (offset, i) in &decoded {
        // Translated code never keeps a host address on any stack, and never reads the code as data.
        if matches!(i.mnemonic(), Mnemonic::Ret | Mnemonic::Call) {
            return Some(format!("host {:?} at {offset}", i.mnemonic()));
        }
        let lea_back = i.mnemonic() == Mnemonic::Lea && i.op0_register() == T;
        if i.is_ip_rel_memory_operand() && !(way == Way::Process && lea_back) {
            return Some(format!("RIP-relative host access at {offset}"));
        }
    }
    // Each guest instruction's commit point: the last host instruction before its first span after it.
    for (k, span) in piece.spans.iter().enumerate() {
        let commit = piece
            .spans
            .get(k + 1)
            .is_some_and(|n| n.guest == span.guest && n.side == Side::After)
            && span.side == Side::Before;
        if !commit {
            continue;
        }
        let offset = span.guest.wrapping_sub(address) as usize;
        let Some((i, length)) = guest.get(offset..).and_then(decode) else {
            continue;
        };
        if X86_64::route(&way, &i, &target()) != Route::Copy {
            continue;
        }
        let end = piece.spans[k + 1].start as usize;
        let (&last, &host_i) = starts.range(..end).next_back()?;
        let own = &guest[offset..offset + usize::from(length)];
        if names_withheld(&i.iced) {
            let same = host_i.code() == i.iced.code()
                && !names_withheld(&host_i)
                && host_i.memory_size() == i.iced.memory_size();
            if !same {
                return Some(format!("stand-in form of {own:02x?} is {host_i:?}"));
            }
        } else if piece.bytes.get(last..last + own.len()) != Some(own) {
            return Some(format!("copy of {own:02x?} at {last} differs"));
        }
    }
    None
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

/// Branches back and forward inside a piece, a call and a return, to mix with the swept forms.
fn branch(rng: &mut fastrand::Rng, so_far: usize) -> Vec<u8> {
    let back = -(rng.i32(0..=so_far as i32 + 6));
    match rng.u8(0..6) {
        0 => [&[0x0f, 0x80 | rng.u8(0..16)][..], &(back - 6).to_le_bytes()].concat(),
        1 => vec![0x70 | rng.u8(0..16), rng.u8(0..32)],
        2 => vec![0xe2, (back - 2) as u8],
        3 => vec![0xe3, rng.u8(0..16)],
        4 => [&[0xe8][..], &rng.i32(-4096..4096).to_le_bytes()].concat(),
        _ => vec![0xc3],
    }
}

/// Every piece of a seeded sweep of instruction mixes decodes whole and matches its position map, in both ways,
/// with and without the count.
#[test]
fn pieces_decode_and_match_their_maps() {
    let seed = knob("X86_OUTPUT_SEED", 0x5eed);
    let cases = knob("X86_OUTPUT_CASES", 20_000);
    let mut rng = fastrand::Rng::with_seed(seed);
    let (mut pieces, mut instructions) = (0u64, 0u64);
    for case in 0..cases {
        let mut guest = Vec::new();
        for _ in 0..rng.usize(1..12) {
            if rng.u8(0..4) == 0 {
                guest.extend(branch(&mut rng, guest.len()));
            } else {
                let candidate = instruction_like(&mut rng);
                if let Some((_, length)) = decode(&candidate) {
                    guest.extend_from_slice(&candidate[..usize::from(length)]);
                }
            }
        }
        if guest.is_empty() {
            continue;
        }
        for way in WAYS {
            for exact in [false, true] {
                let Some(mut piece) = translate(&guest, AT, way, exact) else {
                    continue;
                };
                piece.finish();
                pieces += 1;
                instructions += piece.spans.len() as u64;
                if let Some(what) = broken(&piece, &guest, AT, way) {
                    panic!(
                        "case {case} of seed {seed:#x} ({way:?}, count {exact}): {what}\nguest \
                         {guest:02x?}\n{}",
                        listing(&piece.bytes)
                    );
                }
            }
        }
    }
    println!("seed {seed:#x}: {cases} cases, {pieces} pieces, {instructions} spans, 0 broken");
}

/// The host instructions of the piece made of `hex` at [`AT`], by mnemonic.
fn shape(hex: &str, way: Way) -> Vec<Mnemonic> {
    let piece = translate(&bytes(hex), AT, way, true).expect("a piece");
    println!("{hex} in {way:?}:\n{}", listing(&piece.bytes));
    host(&piece.bytes)
        .iter()
        .map(|(_, i)| i.mnemonic())
        .collect()
}

fn decoded(hex: &str, way: Way) -> Vec<Iced> {
    let piece = translate(&bytes(hex), AT, way, true).expect("a piece");
    host(&piece.bytes).into_iter().map(|(_, i)| i).collect()
}

/// A return reads its target into t, looks it up through the two levels and pops at its commit point.
#[test]
fn a_return_goes_through_the_mirror() {
    use Mnemonic::*;
    for way in WAYS {
        assert_eq!(shape("c3", way), [Lea, Mov, Rorx, Mov, Mov, Lea, Jmp]);
        let code = decoded("c2 10 00", way);
        assert_eq!(code[0].op0_register(), COUNT, "the count's add comes first");
        assert_eq!(code[1].memory_base(), Register::RSP);
        assert_eq!(
            code[4].memory_segment(),
            Register::GS,
            "the first level is the area's"
        );
        assert_eq!(
            code[5].memory_displacement64(),
            8 + 0x10,
            "the pop and the immediate"
        );
        assert_eq!((code[6].memory_base(), code[6].memory_index()), (U, T));
    }
}

/// A call through a register pushes the guest return address from a stand-in, never a host address.
#[test]
fn a_call_through_a_register_pushes_the_guest_address() {
    use Mnemonic::*;
    for way in WAYS {
        let code = decoded("ff d0", way);
        let push = code
            .iter()
            .position(|i| i.mnemonic() == Push)
            .expect("a push");
        let built = &code[push - 1];
        assert_eq!(built.code(), Code::Mov_r64_imm64);
        assert_eq!(built.immediate64(), AT + 2, "the guest return address");
        assert_eq!(code.last().map(Iced::mnemonic), Some(Jmp));
    }
}

/// A direct call pushes its guest return address and leaves by a linkable `jmp rel32` before its call into the
/// inside.
#[test]
fn a_direct_call_pushes_and_exits() {
    use Mnemonic::*;
    assert_eq!(
        shape("e8 00 01 00 00", Way::Machine),
        [Lea, Mov, Push, Jmp, Int]
    );
    assert_eq!(
        shape("e8 00 01 00 00", Way::Process),
        [Lea, Mov, Push, Jmp, Lea, Jmp]
    );
}

/// A loop back to itself stays in the piece: its `jcc` is linked to the piece's start.
#[test]
fn a_loop_is_linked_inside_the_piece() {
    // dec ecx; jnz back to the dec
    let piece = translate(&bytes("ff c9 75 fc"), AT, Way::Process, true).expect("a piece");
    let code = host(&piece.bytes);
    let jcc = code.iter().find(|(_, i)| i.is_jcc_near()).expect("a jcc");
    assert_eq!(
        jcc.1.near_branch64(),
        0,
        "the jcc goes to the piece's start:\n{}",
        listing(&piece.bytes)
    );
}

/// A guest instruction naming a withheld register runs on a stand-in loaded from and stored to the area.
#[test]
fn a_withheld_register_takes_a_stand_in() {
    for way in WAYS {
        // add r10, r11
        let code = decoded("4d 01 da", way);
        let add = code
            .iter()
            .find(|i| i.mnemonic() == Mnemonic::Add)
            .expect("the add");
        assert!(!names_withheld(add), "{add:?}");
        let area = code
            .iter()
            .filter(|i| i.mnemonic() == Mnemonic::Mov && i.memory_segment() == Register::GS)
            .count();
        let piece = translate(&bytes("4d 01 da"), AT, way, true).expect("a piece");
        assert_eq!(
            area,
            8,
            "two stand-ins saved, loaded, stored back and restored:\n{}",
            listing(&piece.bytes)
        );
    }
}

/// A guest `gs:` operand is rebuilt on the guest's base from the area, and a `fs:` one is copied.
#[test]
fn the_area_segment_is_rebuilt_and_fs_is_copied() {
    for way in WAYS {
        let code = decoded("65 48 8b 03", way);
        assert_eq!(code[0].memory_segment(), Register::GS);
        assert_eq!(code[0].op0_register(), T);
        assert_eq!(code[1].memory_segment(), Register::DS);
        assert_eq!(
            (code[1].memory_base(), code[1].memory_index()),
            (Register::RBX, T)
        );
        // mov rax, gs:[moffs64]: the 64-bit offset goes to u.
        let code = decoded("65 48 a1 10 00 00 00 00 00 00 01", way);
        assert_eq!(code[1].immediate64(), 0x0100_0000_0000_0010);
        assert_eq!((code[2].memory_base(), code[2].memory_index()), (T, U));
        assert_eq!(code[2].code(), Code::Mov_r64_rm64);
        let copied = translate(&bytes("64 48 8b 03"), AT, way, false).expect("a piece");
        assert_eq!(&copied.bytes[..4], &bytes("64 48 8b 03")[..]);
    }
}

/// A RIP-relative load is made on the guest address built in t, an address place.
#[test]
fn a_rip_relative_load_is_rebased() {
    for way in WAYS {
        let piece = translate(&bytes("48 8b 05 10 00 00 00"), AT, way, false).expect("a piece");
        assert_eq!(piece.places[0].address, AT + 7 + 0x10);
        let code = host(&piece.bytes);
        assert_eq!(code[1].1.memory_base(), T);
    }
}

/// `syscall` is copied in the machine, whose floor owns it, and a call into the inside in a host process.
#[test]
fn a_system_call_per_way() {
    use Mnemonic::*;
    assert_eq!(shape("0f 05", Way::Machine)[0], Syscall);
    assert_eq!(&shape("0f 05", Way::Process)[..2], [Lea, Jmp]);
}

/// The emitted code of a fault and a stop: a `ud2` is the Trap call, and a stop inside a loop's body lands in a
/// span the map resolves.
#[test]
fn a_trap_and_its_map() {
    for way in WAYS {
        let piece = translate(&bytes("0f 0b"), AT, way, false).expect("a piece");
        let code = host(&piece.bytes);
        let expected = match way {
            Way::Machine => Mnemonic::Int,
            _ => Mnemonic::Lea,
        };
        assert_eq!(code[0].1.mnemonic(), expected);
        assert_eq!(piece.spans[0].guest, AT);
        assert_eq!(piece.spans[0].side, Side::Before);
    }
}

/// The count's add is the first instruction of a counted run and touches no flag.
#[test]
fn the_count_is_a_lea() {
    let code = decoded("ff c9 75 fc", Way::Machine);
    assert_eq!(code[0].mnemonic(), Mnemonic::Lea);
    assert_eq!(code[0].op0_register(), COUNT);
    assert_eq!(code[0].op_kind(1), OpKind::Memory);
}
