//! What x86-64 tells the translator about single instructions: whether a real CPU may give a different
//! result, how the stack pointer moves, and which memory is reached.

use wcontainers_guest_cpu::{AccessKind, BuiltAddress, Guest, StackMove};
use wcontainers_x86::{X86_64, decode};

fn instruction(bytes: &[u8]) -> wcontainers_x86::Instruction {
    let (i, length) = decode(bytes).expect("an instruction");
    assert_eq!(usize::from(length), bytes.len(), "{bytes:02x?}");
    i
}

/// True where the meaning fixes what Intel leaves undefined, answers by design, or stops where a CPU with
/// more features runs on; false where every x86-64 CPU gives the meaning's result.
#[test]
fn what_may_vary_between_cpus() {
    let cases: &[(&[u8], bool)] = &[
        // bt, bts, btr and btc leave OF, SF, AF and PF undefined.
        (&[0x48, 0x0f, 0xa3, 0xc8], true),
        (&[0x0f, 0xba, 0xe0, 0x03], true),
        (&[0x48, 0x0f, 0xab, 0x03], true),
        (&[0x48, 0x0f, 0xb3, 0x03], true),
        (&[0x48, 0x0f, 0xbb, 0x03], true),
        // and leaves AF undefined; mul and div more.
        (&[0x48, 0x21, 0xc8], true),
        (&[0x48, 0xf7, 0xe1], true),
        (&[0x48, 0xf7, 0xf1], true),
        // bsf and bsr (undefined flags, and the destination for a zero source), tzcnt and lzcnt (undefined
        // flags), bswap of 16 bits, shld and shrd (undefined OF and AF, and 16 bits past 16).
        (&[0x48, 0x0f, 0xbc, 0xc1], true),
        (&[0x0f, 0xbd, 0xc1], true),
        (&[0xf3, 0x48, 0x0f, 0xbc, 0xc1], true),
        (&[0xf3, 0x0f, 0xbd, 0xc1], true),
        (&[0x66, 0x0f, 0xc8], true),
        (&[0x48, 0x0f, 0xa4, 0xc8, 0x03], true),
        (&[0x0f, 0xad, 0xc8], true),
        // Fully defined: bswap of 32 and 64 bits, popcnt, movbe, xadd, cmpxchg, cmpxchg16b, adcx, adox,
        // loop, lahf, sahf, clflush, rdfsbase, wrgsbase.
        (&[0x0f, 0xc8], false),
        (&[0x48, 0x0f, 0xc8], false),
        (&[0xf3, 0x48, 0x0f, 0xb8, 0xc1], false),
        (&[0x48, 0x0f, 0x38, 0xf0, 0x03], false),
        (&[0x48, 0x0f, 0xc1, 0x03], false),
        (&[0x48, 0x0f, 0xb1, 0x0b], false),
        (&[0x48, 0x0f, 0xc7, 0x0b], false),
        (&[0x66, 0x48, 0x0f, 0x38, 0xf6, 0xc1], false),
        (&[0xf3, 0x48, 0x0f, 0x38, 0xf6, 0xc1], false),
        (&[0xe2, 0xfe], false),
        (&[0x9f], false),
        (&[0x9e], false),
        (&[0x0f, 0xae, 0x3b], false),
        (&[0xf3, 0x48, 0x0f, 0xae, 0xc0], false),
        (&[0xf3, 0x48, 0x0f, 0xae, 0xd8], false),
        // Fully defined: crc32, xlat, pshufb, ptest, pmulhrsw.
        (&[0xf2, 0x0f, 0x38, 0xf1, 0xc1], false),
        (&[0xd7], false),
        (&[0x66, 0x0f, 0x38, 0x00, 0xc1], false),
        (&[0x66, 0x0f, 0x38, 0x17, 0xc1], false),
        (&[0x66, 0x0f, 0x38, 0x0b, 0xc1], false),
        // The reference's own: dpps and dppd stopped part way by an exception, maskmovdqu's faults under an
        // all-zero mask, ldmxcsr's refusal of bit 17.
        (&[0x66, 0x0f, 0x3a, 0x40, 0xc1, 0xff], true),
        (&[0x66, 0x0f, 0x3a, 0x41, 0xc1, 0x33], true),
        (&[0x66, 0x0f, 0xf7, 0xc1], true),
        (&[0x0f, 0xae, 0x13], true),
        // Fully defined: addps, sqrtsd, cvttsd2si.
        (&[0x0f, 0x58, 0xc1], false),
        (&[0xf2, 0x0f, 0x51, 0xc1], false),
        (&[0xf2, 0x48, 0x0f, 0x2c, 0xc1], false),
        // stmxcsr, which stores MXCSR's flags as last written while they are not kept.
        (&[0x0f, 0xae, 0x1b], true),
        // Answered by design: rdtsc, rdtscp, cpuid; popf holds AC clear and does not step on TF.
        (&[0x0f, 0x31], true),
        (&[0x0f, 0x01, 0xf9], true),
        (&[0x0f, 0xa2], true),
        (&[0x9d], true),
        // Linux's 32-bit system call gates, general protection by design.
        (&[0xcd, 0x80], true),
        (&[0x0f, 0x34], true),
        // An EVEX instruction (AVX-512), an AVX one, and an invalid encoding: illegal instruction here.
        (&[0x62, 0xf1, 0x7c, 0x48, 0x58, 0xc0], true),
        (&[0xc5, 0xfc, 0x58, 0xc0], true),
        (&[0x06], true),
        // Kernel-only instructions a kernel may let user code run: rdpmc, in, outsb, cli.
        (&[0x0f, 0x33], true),
        (&[0xec], true),
        (&[0x6e], true),
        (&[0xfa], true),
        // BMI1: andn, bextr, blsi, blsmsk and blsr leave AF and PF (bextr SF too) undefined.
        (&[0xc4, 0xe2, 0x78, 0xf2, 0xc1], true),
        (&[0xc4, 0xe2, 0x68, 0xf7, 0xc1], true),
        (&[0xc4, 0xe2, 0xf8, 0xf3, 0xd9], true),
        (&[0xc4, 0xe2, 0xf8, 0xf3, 0xd1], true),
        (&[0xc4, 0xe2, 0xf8, 0xf3, 0xc9], true),
        // Near branches with 66, which AMD takes as 16 bits; the same without it are fully defined.
        (&[0x66, 0x74, 0x10], true),
        (&[0x66, 0x0f, 0x84, 0x10, 0, 0, 0], true),
        (&[0x66, 0xe8, 0x10, 0, 0, 0], true),
        (&[0x66, 0xe9, 0x10, 0, 0, 0], true),
        (&[0x66, 0xc3], true),
        (&[0x66, 0xc2, 0x08, 0x00], true),
        (&[0x66, 0xff, 0xe0], true),
        (&[0x66, 0xff, 0xd0], true),
        (&[0x66, 0xe2, 0x10], true),
        (&[0x66, 0xe3, 0x10], true),
        (&[0xe8, 0x10, 0, 0, 0], false),
        (&[0xc3], false),
        (&[0xff, 0xe0], false),
        (&[0xe2, 0x10], false),
        // lss with REX.W, which AMD reads as m16:32; without it, fully defined.
        (&[0x48, 0x0f, 0xb2, 0x07], true),
        (&[0x0f, 0xb2, 0x07], false),
        // ud0, whose ModRM AMD does not read.
        (&[0x0f, 0xff, 0xc0], true),
        // Fully defined: add, mov, push, a jump, int3, a kernel-only instruction, int 0x81.
        (&[0x48, 0x01, 0xc8], false),
        (&[0x48, 0x89, 0xc8], false),
        (&[0x50], false),
        (&[0x74, 0x10], false),
        (&[0xcc], false),
        (&[0x0f, 0x01, 0xf8], false),
        (&[0xcd, 0x81], false),
    ];
    for &(bytes, vary) in cases {
        assert_eq!(X86_64::may_vary(&instruction(bytes)), vary, "{bytes:02x?}");
    }
}

/// Every encoding iced reads otherwise as an AMD CPU (16-bit near branches, REX.W far pointers, `ud0`'s
/// ModRM) is marked: over every opcode of the legacy maps, behind each of these prefixes, with ModRMs of
/// every mode and register field.
#[test]
fn what_amd_reads_otherwise_is_marked() {
    use iced_x86::{Decoder, DecoderOptions};
    const PREFIXES: &[&[u8]] = &[
        &[],
        &[0x66],
        &[0x48],
        &[0x66, 0x48],
        &[0xf0],
        &[0xf2],
        &[0xf3],
        &[0x67],
    ];
    const MAPS: &[&[u8]] = &[&[], &[0x0f], &[0x0f, 0x38], &[0x0f, 0x3a]];
    // A memory operand through a register, through a SIB byte, and rip-relative.
    const RM: [u8; 3] = [0, 4, 5];
    const MOD_SHIFT: u32 = 6;
    const REG_SHIFT: u32 = 3;
    const MODS: u8 = 4;
    const REGS: u8 = 8;
    let decode_as = |bytes: &[u8], options: u32| {
        let mut d = Decoder::with_ip(wcontainers_x86::MODE_BITS, bytes, 0, options);
        let i = d.decode();
        (i.code(), i.len())
    };
    let intel = DecoderOptions::FORCE_RESERVED_NOP;
    let amd = intel | DecoderOptions::AMD;
    let mut differing = 0;
    for prefixes in PREFIXES {
        for map in MAPS {
            for opcode in 0..=u8::MAX {
                for modrm in (0..MODS).flat_map(|m| {
                    (0..REGS).flat_map(move |r| RM.map(|rm| m << MOD_SHIFT | r << REG_SHIFT | rm))
                }) {
                    let mut bytes = [prefixes, map, &[opcode, modrm][..]].concat();
                    bytes.resize(bytes.len() + X86_64::MAX_LENGTH as usize, 0);
                    if decode_as(&bytes, intel) == decode_as(&bytes, amd) {
                        continue;
                    }
                    differing += 1;
                    let (i, _) = decode(&bytes).expect("an instruction");
                    assert!(X86_64::may_vary(&i), "{:02x?}", &bytes[..i.iced.len()]);
                }
            }
        }
    }
    assert!(differing > 0);
}

/// `add` or `sub` of rsp by memory moves it by an amount unknown when translating.
#[test]
fn stack_moves() {
    let cases: &[(&[u8], StackMove)] = &[
        (&[0x48, 0x03, 0x23], StackMove::Unknown),
        (&[0x48, 0x2b, 0x23], StackMove::Unknown),
        (
            &[0x48, 0x83, 0xec, 0x10],
            StackMove::By {
                delta: -0x10,
                before_access: false,
            },
        ),
        (
            &[0x50],
            StackMove::By {
                delta: -8,
                before_access: true,
            },
        ),
    ];
    for &(bytes, ref moved) in cases {
        assert_eq!(
            &X86_64::stack_move(&instruction(bytes)),
            moved,
            "{bytes:02x?}"
        );
    }
}

/// Multi-byte and reserved NOPs reach no memory; the SSE stores only write.
#[test]
fn memory_reached() {
    for bytes in [
        &[0x0f, 0x1f, 0x80, 0, 0, 0, 0][..],
        &[0x0f, 0x1f, 0x00],
        &[0x0f, 0x18, 0x08],
        &[0x0f, 0x0d, 0x08],
        &[0xf3, 0x0f, 0x1e, 0x08],
    ] {
        assert_eq!(
            X86_64::accesses(&instruction(bytes)),
            [None, None],
            "{bytes:02x?}"
        );
    }
    for bytes in [
        &[0x66, 0x0f, 0x29, 0x03][..],
        &[0x66, 0x0f, 0x11, 0x03],
        &[0x0f, 0x29, 0x03],
        &[0x0f, 0x11, 0x03],
    ] {
        let [first, second] = X86_64::accesses(&instruction(bytes));
        assert_eq!(
            first.map(|a| a.kind),
            Some(AccessKind::Write),
            "{bytes:02x?}"
        );
        assert!(second.is_none());
    }
}

/// Addresses an instruction writes out or builds relative to itself, for code that may take a function's.
#[test]
fn built_addresses() {
    const AT: u64 = 0x20;
    let cases: &[(&[u8], Option<BuiltAddress>)] = &[
        // lea rax, [rip + 0x10]: relative to the next instruction.
        (
            &[0x48, 0x8d, 0x05, 0x10, 0, 0, 0],
            Some(BuiltAddress::Offset(AT + 7 + 0x10)),
        ),
        // mov edi, 0x401000 and push 0x401000.
        (
            &[0xbf, 0x00, 0x10, 0x40, 0x00],
            Some(BuiltAddress::Absolute(0x40_1000)),
        ),
        (
            &[0x68, 0x00, 0x10, 0x40, 0x00],
            Some(BuiltAddress::Absolute(0x40_1000)),
        ),
        // lea rax, [0x401000] and lea eax, [0x401000]: a displacement alone.
        (
            &[0x48, 0x8d, 0x04, 0x25, 0x00, 0x10, 0x40, 0x00],
            Some(BuiltAddress::Absolute(0x40_1000)),
        ),
        (
            &[0x8d, 0x04, 0x25, 0x00, 0x10, 0x40, 0x00],
            Some(BuiltAddress::Absolute(0x40_1000)),
        ),
        // The displacement is sign-extended to 64 bits, and a 32-bit register keeps its low half.
        (
            &[0x48, 0x8d, 0x04, 0x25, 0x00, 0x00, 0x00, 0x80],
            Some(BuiltAddress::Absolute(0xffff_ffff_8000_0000)),
        ),
        (
            &[0x8d, 0x04, 0x25, 0x00, 0x00, 0x00, 0x80],
            Some(BuiltAddress::Absolute(0x8000_0000)),
        ),
        // lea ax, [0x401000] keeps 16 bits, not the address; lea rax, [rbx + 0x401000] adds a register.
        (&[0x66, 0x8d, 0x04, 0x25, 0x00, 0x10, 0x40, 0x00], None),
        (&[0x48, 0x8d, 0x83, 0x00, 0x10, 0x40, 0x00], None),
    ];
    for &(bytes, built) in cases {
        assert_eq!(
            X86_64::built_address(AT, &instruction(bytes), &[], 0..0x100, |_| None),
            built,
            "{bytes:02x?}"
        );
    }
}

/// `wait` runs alone before the x87 instruction iced reads with it, whatever its other prefixes, and a lock
/// prefix makes it illegal, as the reference CPU raises.
#[test]
fn wait_alone_and_locked() {
    use iced_x86::Code;
    for (bytes, code, length) in [
        (&[0x9b, 0xdf, 0xe0][..], Code::Wait, 1),
        (&[0x66, 0x48, 0x9b, 0xdf, 0xe0], Code::Wait, 3),
        (&[0x3e, 0xf3, 0x9b], Code::Wait, 3),
        (&[0xf0, 0x9b], Code::INVALID, 2),
        (&[0xf0, 0x9b, 0xdf, 0xe0], Code::INVALID, 2),
        (
            &[
                0x3e, 0x3e, 0x40, 0x49, 0x49, 0x42, 0x65, 0xf0, 0xf3, 0x48, 0x67, 0x36, 0x9b, 0, 0,
                0,
            ],
            Code::INVALID,
            13,
        ),
    ] {
        let (i, decoded) = decode(bytes).expect("an instruction");
        assert_eq!((i.iced.code(), decoded), (code, length), "{bytes:02x?}");
    }
}
