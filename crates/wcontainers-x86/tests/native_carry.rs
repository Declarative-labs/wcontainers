//! The carrying code of an operand on the thread area's segment (docs/x86-native-output.md section 3.1), checked
//! by the address its host access reaches on FS, which holds the guest's GS base for it, against the meaning's: the
//! guest's GS base plus the operand's offset part taken at the operand's own address size.

use iced_x86::{
    Code, Decoder, DecoderOptions, Encoder, Instruction, MemoryOperand, Mnemonic, OpKind, Register,
};
use wcontainers_translator::{LookAhead, translate_native_for};
use wcontainers_x86::area::GUEST_GS_BASE;
use wcontainers_x86::native::{HostMatches, Target, Way};
use wcontainers_x86::{Offer, V3, X86_64};

/// The host's GS base: the thread area.
const AREA: u64 = 0x7f00_0000_0000;

/// The guest registers an operand may name (the withheld r10, r11 and r15 excepted), with their low halves.
const WIDE: [Register; 13] = [
    Register::RAX,
    Register::RCX,
    Register::RDX,
    Register::RBX,
    Register::RSP,
    Register::RBP,
    Register::RSI,
    Register::RDI,
    Register::R8,
    Register::R9,
    Register::R12,
    Register::R13,
    Register::R14,
];
const LOW: [Register; 13] = [
    Register::EAX,
    Register::ECX,
    Register::EDX,
    Register::EBX,
    Register::ESP,
    Register::EBP,
    Register::ESI,
    Register::EDI,
    Register::R8D,
    Register::R9D,
    Register::R12D,
    Register::R13D,
    Register::R14D,
];

/// Host registers' values by full register; a 32-bit name reads the low half.
struct Values(Vec<(Register, u64)>);

impl Values {
    fn get(&self, r: Register) -> Option<u64> {
        if r == Register::None {
            return Some(0);
        }
        if r == Register::GS {
            return Some(AREA);
        }
        if r.is_segment_register() {
            return Some(0);
        }
        let low = LOW
            .iter()
            .chain(&[Register::R11D, Register::R15D])
            .position(|&l| l == r);
        let wide = WIDE.iter().chain(&[Register::R11, Register::R15]);
        let full = low.map_or(r, |at| *wide.clone().nth(at).expect("a pair"));
        let value = self.0.iter().find(|(f, _)| *f == full).map(|&(_, v)| v)?;
        Some(if low.is_some() {
            value & u64::from(u32::MAX)
        } else {
            value
        })
    }

    fn set(&mut self, r: Register, value: u64) {
        let full = match r {
            Register::R11D => Register::R11,
            Register::R15D => Register::R15,
            other => other,
        };
        self.0.retain(|(f, _)| *f != full);
        self.0.push((full, value));
    }
}

/// The guest's FS base, which the carrying code keeps and puts back.
const GUEST_FS: u64 = 0x7e00_0000_1000;

/// The address the carried access reaches, the code around it run: the guest's FS base kept, the guest's GS base
/// loaded from the register area onto FS, the access on FS, the FS base put back. Also gives the access's bytes.
fn reached(code: &[u8], mut values: Values, guest_gs: u64) -> (u64, Vec<u8>) {
    let all: Vec<Instruction> = Decoder::with_ip(64, code, 0, DecoderOptions::NONE)
        .into_iter()
        .collect();
    let memory = |i: &Instruction| (0..i.op_count()).find(|&n| i.op_kind(n) == OpKind::Memory);
    let mut fs = GUEST_FS;
    let mut access = None;
    for i in &all {
        let at = |values: &Values, fs: u64| {
            memory(i).map(|n| {
                i.virtual_address(n, 0, |r, _, _| {
                    if r == Register::FS {
                        Some(fs)
                    } else {
                        values.get(r)
                    }
                })
                .expect("registers known")
            })
        };
        match i.mnemonic() {
            // The Float status route's calls into the inside, around the window.
            Mnemonic::Int => assert_eq!(fs, GUEST_FS, "a call into the inside outside the window"),
            Mnemonic::Rdfsbase => values.set(i.op0_register(), fs),
            Mnemonic::Wrfsbase => fs = values.get(i.op0_register()).expect("a known register"),
            Mnemonic::Mov if i.segment_prefix() == Register::GS => {
                assert_eq!(
                    at(&values, fs),
                    Some(AREA + GUEST_GS_BASE as u64),
                    "the load reads the register area"
                );
                values.set(i.op0_register(), guest_gs);
            }
            _ => {
                assert_eq!(i.segment_prefix(), Register::FS, "the access is on FS");
                assert!(access.is_none(), "one access");
                let bytes = &code[i.ip() as usize..i.next_ip() as usize];
                access = Some((at(&values, fs).expect("a memory operand"), bytes.to_vec()));
            }
        }
    }
    assert_eq!(fs, GUEST_FS, "the guest's FS base is put back");
    access.expect("an access")
}

/// `mov eax, gs:[...]` as the guest wrote it.
fn guest_load(base: Register, index: Register, scale: u32, disp: i64, narrow: bool) -> Vec<u8> {
    let displ_size = if disp == 0 {
        0
    } else if i8::try_from(disp).is_ok() {
        1
    } else if narrow {
        4
    } else {
        8
    };
    let operand = MemoryOperand::new(base, index, scale, disp, displ_size, false, Register::GS);
    let i = Instruction::with2(Code::Mov_r32_rm32, Register::EAX, operand).expect("an instruction");
    let mut encoder = Encoder::new(64);
    encoder.encode(&i, 0).expect("encodes");
    encoder.take_buffer()
}

/// The host code of the one guest instruction `code`, up to the exit after it.
fn carried(code: &[u8]) -> Vec<u8> {
    let target = Target {
        host: HostMatches {
            five_level_paging: false,
            partial_writes: true,
            undefined_results: true,
        },
        offer: Offer::new(V3).expect("the v3 list has meanings"),
    };
    let piece = translate_native_for::<X86_64>(
        code,
        0x40_1000,
        LookAhead::None,
        false,
        &Way::Machine,
        &target,
    )
    .expect("a piece");
    let end = piece.exits[0].start as usize;
    piece.bytes[..end].to_vec()
}

/// The eighth check's gs67.c: GS base 0x300000000, rax 0x100000010.
#[test]
fn the_address_size_prefix_wraps_before_the_base() {
    let values = || Values(vec![(Register::RAX, 0x1_0000_0010)]);
    let base = 0x3_0000_0000;
    assert_eq!(
        reached(&carried(&[0x65, 0x67, 0x8b, 0x00]), values(), base),
        (0x3_0000_0010, vec![0x64, 0x67, 0x8b, 0x00])
    );
    assert_eq!(
        reached(&carried(&[0x65, 0x8b, 0x00]), values(), base).0,
        0x4_0000_0010
    );
}

/// The guest's prefixes and their order are kept, each GS override made FS, so the CPU picks the segment it picks
/// for the guest (the check's `2e 65 36` setg, whose last prefix is ss) and wraps each part of an access under 0x67
/// (its fxsave64 across offset 2^32).
#[test]
fn the_guest_bytes_run_on_fs() {
    let values = || {
        Values(vec![
            (Register::R9, 0xc7),
            (Register::RAX, 1),
            (Register::RSI, 2),
        ])
    };
    let setg = [
        0x2e, 0x65, 0x36, 0x4f, 0x0f, 0x9f, 0x81, 0xdf, 0xbc, 0x7c, 0x48,
    ];
    let (_, bytes) = reached(&carried(&setg), values(), 0x4400_0000);
    assert_eq!(bytes[..3], [0x2e, 0x64, 0x36]);
    assert_eq!(bytes[3..], setg[3..]);
    let fxsave = [
        0x65, 0x67, 0x48, 0x0f, 0xae, 0x84, 0x30, 0x37, 0x05, 0xf1, 0x4b,
    ];
    let (_, bytes) = reached(&carried(&fxsave), values(), 0x4400_0000);
    assert_eq!(bytes[0], 0x64);
    assert_eq!(bytes[1..], fxsave[1..]);
}

fn knob(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.trim().parse().expect("a number"))
}

/// Seeded operands of every shape and both address sizes. `X86_CARRY_SEED` and `X86_CARRY_CASES` set the sweep.
#[test]
fn every_operand_shape_reaches_the_meanings_address() {
    let seed = knob("X86_CARRY_SEED", 0x5eed);
    let cases = knob("X86_CARRY_CASES", 100_000);
    let mut rng = fastrand::Rng::with_seed(seed);
    for _ in 0..cases {
        let narrow = rng.bool();
        let names = if narrow { &LOW } else { &WIDE };
        let pick = |rng: &mut fastrand::Rng, stack: bool| match rng.usize(0..names.len() + 1) {
            n if n == names.len() => (Register::None, 0),
            n if !stack && n == 4 => (Register::None, 0),
            n => (names[n], n),
        };
        let (mut base, mut b) = pick(&mut rng, true);
        let (index, x) = pick(&mut rng, false);
        // With no register the encoder takes the 64-bit form, so a 32-bit operand names one.
        if narrow && base == Register::None && index == Register::None {
            (base, b) = (names[0], 0);
        }
        let scale = [1, 2, 4, 8][rng.usize(0..4)];
        let disp = match rng.u8(0..3) {
            0 => 0,
            1 => i64::from(rng.i8(..)),
            _ => i64::from(rng.i32(..)),
        };
        let values: Vec<(Register, u64)> = WIDE.iter().map(|&r| (r, rng.u64(..))).collect();
        let guest_gs = rng.u64(..);
        let value = |r: Register, n: usize| {
            if r == Register::None {
                0
            } else if narrow {
                values[n].1 & u64::from(u32::MAX)
            } else {
                values[n].1
            }
        };
        let mut offset = value(base, b)
            .wrapping_add(value(index, x).wrapping_mul(scale.into()))
            .wrapping_add(disp as u64);
        if narrow {
            offset &= u64::from(u32::MAX);
        }
        let guest = guest_load(base, index, scale, disp, narrow);
        let (host, _) = reached(&carried(&guest), Values(values.clone()), guest_gs);
        assert_eq!(
            host,
            guest_gs.wrapping_add(offset),
            "seed {seed:#x}: {guest:02x?} with {values:x?}, GS base {guest_gs:#x}"
        );
    }
}
