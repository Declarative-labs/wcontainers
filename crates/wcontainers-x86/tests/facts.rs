//! What x86-64 tells the translator about single instructions: whether a real CPU may give a different
//! result, how the stack pointer moves, and which memory is reached.

use wcontainers_guest_cpu::{AccessKind, Guest, StackMove};
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
        // Linux's 32-bit system call gates, general protection by design.
        (&[0xcd, 0x80], true),
        (&[0x0f, 0x34], true),
        // An EVEX instruction (AVX-512), an AVX one, and an invalid encoding: illegal instruction here.
        (&[0x62, 0xf1, 0x7c, 0x48, 0x58, 0xc0], true),
        (&[0xc5, 0xfc, 0x58, 0xc0], true),
        (&[0x06], true),
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
