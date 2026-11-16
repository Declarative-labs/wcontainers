//! Checks that cover several accesses at once (a stack frame, a group through one base) failing, also on the
//! first instruction of a function: the same result as Linux, and at full the same registers and memory as at
//! none.

use dynasmrt::aarch64::Aarch64Relocation;
use dynasmrt::{DynasmApi, DynasmLabelApi, VecAssembler, dynasm};
use wcontainers_translator::contract::Reason;
use wcontainers_translator::{LookAhead, Omit};
use wcontainers_translator_test::browser;
use wcontainers_translator_test::programs::{Blob, LOAD, native, reachable_memory, translated};

type Ops = VecAssembler<Aarch64Relocation>;

/// Clear the registers the reference does not start at zero.
// dynasm's expansion converts register numbers to their own type.
#[allow(clippy::useless_conversion)]
fn zero(ops: &mut Ops) {
    for r in 0..22u8 {
        dynasm!(ops ; .arch aarch64 ; movz X(r), #0);
    }
}

/// Write x0 to x15 to standard output and exit with 0.
fn dump(ops: &mut Ops) {
    dynasm!(ops ; .arch aarch64
        ; sub sp, sp, #128
        ; stp x0, x1, [sp, #0]
        ; stp x2, x3, [sp, #16]
        ; stp x4, x5, [sp, #32]
        ; stp x6, x7, [sp, #48]
        ; stp x8, x9, [sp, #64]
        ; stp x10, x11, [sp, #80]
        ; stp x12, x13, [sp, #96]
        ; stp x14, x15, [sp, #112]
        ; mov x1, sp
        ; movz x0, #1
        ; movz x2, #128
        ; movz x8, #64
        ; svc #0
        ; movz x0, #0
        ; movz x8, #94
        ; svc #0
    );
}

fn blob(name: &str, ops: Ops) -> Blob {
    let bytes = ops.finalize().expect("labels");
    Blob {
        name: name.into(),
        code_bytes: bytes.len() as u64,
        bytes,
        load: LOAD,
        program_headers: None,
    }
}

/// Linux, none and full agree, and full ends with the registers and memory of none. Returns how often full
/// left to an address.
fn compare(blob: &Blob) -> u64 {
    let theirs = native(blob);
    let (none, mut none_machine, none_registers) = translated(blob, LookAhead::None);
    let (full, mut full_machine, full_registers) = translated(blob, LookAhead::Full);
    eprintln!(
        "{}: status {}, full counters {:?}",
        blob.name,
        theirs.status,
        full_machine.counters()
    );
    assert_eq!(none, theirs, "{}: none against Linux", blob.name);
    assert_eq!(full, theirs, "{}: full against Linux", blob.name);
    assert_eq!(full_registers, none_registers, "{}: registers", blob.name);
    assert!(
        reachable_memory(&mut full_machine, blob) == reachable_memory(&mut none_machine, blob),
        "{}: memory",
        blob.name
    );
    full_machine.count(Reason::LeftToAddress)
}

/// Recursion with 4 KiB frames, each made by the function's first instruction; 300 deep runs off the 1 MiB
/// stack.
fn recursion(depth: u32) -> Blob {
    let mut ops: Ops = VecAssembler::new(0);
    zero(&mut ops);
    let f = ops.new_dynamic_label();
    let out = ops.new_dynamic_label();
    dynasm!(ops ; .arch aarch64
        ; movz x0, #depth
        ; movz x1, #0
        ; bl =>f
    );
    dump(&mut ops);
    dynasm!(ops ; .arch aarch64
        ; =>f
        ; sub sp, sp, #0x1, lsl #12
        ; stp x29, x30, [sp]
        ; str x0, [sp, #16]
        ; add x1, x1, x0
        ; cbz x0, =>out
        ; sub x0, x0, #1
        ; bl =>f
        ; ldr x2, [sp, #16]
        ; add x1, x1, x2
        ; =>out
        ; ldp x29, x30, [sp]
        ; add sp, sp, #0x1, lsl #12
        ; ret
    );
    blob(&format!("recursion-{depth}"), ops)
}

#[test]
fn recursion_off_the_stack() {
    for depth in [100, 300] {
        let left = compare(&recursion(depth));
        assert_eq!(left > 0, depth == 300, "left to an address {left} times");
    }
}

/// A function whose first two loads go through one base, the second onto the page after the blob, which
/// allows nothing.
fn group_entry() -> Blob {
    let mut ops: Ops = VecAssembler::new(0);
    zero(&mut ops);
    let f = ops.new_dynamic_label();
    dynasm!(ops ; .arch aarch64
        ; movz x1, #0x0ff0
        ; movk x1, #0x40, lsl #16
        ; bl =>f
    );
    dump(&mut ops);
    dynasm!(ops ; .arch aarch64
        ; =>f
        ; ldr x5, [x1]
        ; ldr x6, [x1, #16]
        ; ret
    );
    blob("group-entry", ops)
}

#[test]
fn group_at_a_function_entry() {
    assert!(compare(&group_entry()) > 0);
}

/// A function entered twice whose frame reaches below the stack but is used only in its mapped part, then
/// (`fault`) once below it.
fn frame_below(fault: bool) -> Blob {
    let mut ops: Ops = VecAssembler::new(0);
    zero(&mut ops);
    let f = ops.new_dynamic_label();
    dynasm!(ops ; .arch aarch64
        ; sub sp, sp, #0xfc, lsl #12
        ; movz x0, #5
        ; bl =>f
        ; bl =>f
        ; add sp, sp, #0xfc, lsl #12
    );
    dump(&mut ops);
    dynasm!(ops ; .arch aarch64
        ; =>f
        ; sub sp, sp, #0x8, lsl #12
        ; str x0, [sp, #0x7ff8]
        ; ldr x2, [sp, #0x7ff8]
        ; add x0, x2, #1
    );
    if fault {
        dynasm!(ops ; .arch aarch64 ; movz x9, #77 ; str x0, [sp, #8]);
    }
    dynasm!(ops ; .arch aarch64
        ; add sp, sp, #0x8, lsl #12
        ; ret
    );
    blob(&format!("frame-below-{fault}"), ops)
}

#[test]
fn frame_partly_below_the_stack() {
    for fault in [false, true] {
        assert!(compare(&frame_below(fault)) > 0);
    }
}

/// The checks that fail on a function's first instruction, at full in Chrome.
#[test]
fn failing_at_an_entry_in_chrome() {
    for blob in [
        recursion(300),
        group_entry(),
        frame_below(false),
        frame_below(true),
    ] {
        let theirs = native(&blob);
        let ours = browser::translated_with(&blob, LookAhead::Full, Omit::default());
        eprintln!("{}: {:?}", blob.name, ours.counters);
        assert_eq!(ours.ran, theirs, "{} in Chrome", blob.name);
        assert!(ours.count(Reason::LeftToAddress.name()) > 0);
    }
}

/// Stores and post-indexed loads through one base, the last crossing onto a page that allows nothing.
#[test]
fn group_crossing_onto_a_forbidden_page() {
    let mut ops: Ops = VecAssembler::new(0);
    zero(&mut ops);
    dynasm!(ops ; .arch aarch64
        ; movz x1, #0x0ff0
        ; movk x1, #0x40, lsl #16
        ; movz x5, #0x1234
        ; str x5, [x1]
        ; add x6, x5, #1
        ; str x6, [x1, #8]
        ; movz x7, #9
        ; str x6, [x1, #16]
    );
    dump(&mut ops);
    compare(&blob("group-store", ops));

    let mut ops: Ops = VecAssembler::new(0);
    zero(&mut ops);
    dynasm!(ops ; .arch aarch64
        ; movz x1, #0x0ff0
        ; movk x1, #0x40, lsl #16
        ; ldr x5, [x1], #8
        ; add x9, x5, #3
        ; ldr x6, [x1], #8
        ; ldr x7, [x1], #8
    );
    dump(&mut ops);
    compare(&blob("group-postindex", ops));
}

/// A loop indexing through a register that a narrow load writes back as its base (post- or pre-indexed), so
/// the load does not bound it: the indexed load reaches a page that allows nothing.
#[test]
fn index_written_back_by_a_narrow_load() {
    for pre in [false, true] {
        let mut ops: Ops = VecAssembler::new(0);
        zero(&mut ops);
        let top = ops.new_dynamic_label();
        dynasm!(ops ; .arch aarch64
            ; movz x8, #0x40, lsl #16
            ; movz x14, #0x0100
            ; movk x14, #0x40, lsl #16
            ; movz x16, #4
            ; =>top
        );
        if pre {
            dynasm!(ops ; .arch aarch64 ; ldrb w9, [x14, #1]!);
        } else {
            dynasm!(ops ; .arch aarch64 ; ldrb w9, [x14], #1);
        }
        dynasm!(ops ; .arch aarch64
            ; ldrb w15, [x8, x14]
            ; add x14, x14, #1
            ; add x4, x4, x15
            ; subs x16, x16, #1
            ; b.ne =>top
        );
        dump(&mut ops);
        compare(&blob(&format!("index-written-back-{pre}"), ops));
    }
}

/// A loop that moves the stack down 16 bytes each turn; 70,000 turns run off the stack.
#[test]
fn alloca_loop_off_the_stack() {
    for turns in [1000u32, 70000] {
        let mut ops: Ops = VecAssembler::new(0);
        zero(&mut ops);
        let top = ops.new_dynamic_label();
        dynasm!(ops ; .arch aarch64
            ; mov x29, sp
            ; movz x3, #turns & 0xffff
            ; movk x3, #turns >> 16, lsl #16
            ; movz x4, #0
            ; =>top
            ; sub sp, sp, #16
            ; str x4, [sp]
            ; str x3, [sp, #8]
            ; add x4, x4, #1
            ; subs x3, x3, #1
            ; b.ne =>top
            ; mov sp, x29
        );
        dump(&mut ops);
        compare(&blob(&format!("alloca-{turns}"), ops));
    }
}

/// Loops that make no call read or write through a base and an index moved by a fixed step each turn,
/// until the access reaches the page after the blob, which allows nothing. These accesses keep the page they
/// last found allowed, so the fault must still land on the exact turn.
#[test]
fn loops_running_onto_a_forbidden_page() {
    for step in [1u32, 8, 4095, 4096, 4097] {
        for write in [false, true] {
            let mut ops: Ops = VecAssembler::new(0);
            zero(&mut ops);
            dynasm!(ops ; .arch aarch64
                ; movz x1, #0x40, lsl #16
                ; movz x3, #0x200
                ; movz x6, step
                ; ->turn:
            );
            if write {
                dynasm!(ops ; .arch aarch64 ; strb w3, [x1, x3]);
            } else {
                dynasm!(ops ; .arch aarch64 ; ldrb w2, [x1, x3] ; add x4, x4, x2);
            }
            dynasm!(ops ; .arch aarch64
                ; add x3, x3, x6
                ; add x5, x5, #1
                ; b ->turn
            );
            dump(&mut ops);
            // Three more pages for the loop to walk through first.
            ops.extend(std::iter::repeat_n(0u8, 3 * 4096));
            let name = format!("loop-step-{step}-{}", if write { "store" } else { "load" });
            compare(&blob(&name, ops));
        }
    }
}
