//! ARM64's hand-made programs of the smoke, checks and changes tests, assembled with dynasm.

// dynasm's expansion converts register numbers to their own type.
#![allow(clippy::useless_conversion)]

use crate::guest::{Change, Check, Counted, Smoke};
use crate::programs::{Blob, LOAD, STACK_BYTES};
use dynasmrt::aarch64::Aarch64Relocation;
use dynasmrt::{DynamicLabel, DynasmApi, DynasmLabelApi, VecAssembler, dynasm};
use wcontainers_abi::PAGE_SIZE;
use wcontainers_translator::contract::Reason;

type Ops = VecAssembler<Aarch64Relocation>;

fn words(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

/// x0 to x30, sp, pc: the fields the programs name.
const X0: usize = 0;
const X16: usize = 16;

pub(super) fn smoke() -> Smoke {
    Smoke {
        // movz x0, #42; movz x8, #93; svc #0
        exit: words(&[0xd280_0540, 0xd280_0ba8, 0xd400_0001]),
        // x0 = 0; x1 = 10; loop: bl add; subs x1, x1, #1; b.ne loop; exit(x0)
        // add: add x0, x0, x1; ret
        loop_and_call: words(&[
            0xd280_0000, // movz x0, #0
            0xd280_0141, // movz x1, #10
            0x9400_0005, // bl +5 (add)
            0xf100_0421, // subs x1, x1, #1
            0x54ff_ffc1, // b.ne -2
            0xd280_0ba8, // movz x8, #93
            0xd400_0001, // svc #0
            0x8b01_0000, // add x0, x0, x1
            0xd65f_03c0, // ret
        ]),
        // movz x8, #999; svc #0; svc #0; movz x0, #3; movz x8, #93; svc #0
        unknown_call: words(&[
            0xd280_7ce8,
            0xd400_0001,
            0xd400_0001,
            0xd280_0060,
            0xd280_0ba8,
            0xd400_0001,
        ]),
        // movz x0, #0x4000, lsl #16; loop: subs x0, x0, #1; b.ne loop; movz x8, #93; svc #0
        long_loop: words(&[
            0xd2a8_0000,
            0xf100_0400,
            0x54ff_ffe1,
            0xd280_0ba8,
            0xd400_0001,
        ]),
        long_loop_counter: X0,
        long_loop_body: vec![4, 8],
        // movz x0, #1
        mark: words(&[0xd280_0020]),
        marked: X0,
        illegal: vec![
            (words(&[0x4e28_4820]), 1), // aese v0.16b, v1.16b
            (words(&[0x04a2_0020]), 1), // add z0.s, z1.s, z2.s
            (words(&[0x0000_0000]), 0), // udf #0
        ],
    }
}

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
        link_address: None,
        main: None,
        program_headers: None,
        stack_bytes: STACK_BYTES,
    }
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

/// Stores and post-indexed loads through one base, the last crossing onto a page that allows nothing.
fn group_crossing_onto_a_forbidden_page(out: &mut Vec<Check>) {
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
    out.push(plain(blob("group-store", ops)));

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
    out.push(plain(blob("group-postindex", ops)));
}

/// A loop indexing through a register that a narrow load writes back as its base (post- or pre-indexed), so
/// the load does not bound it: the indexed load reaches a page that allows nothing.
fn index_written_back_by_a_narrow_load(out: &mut Vec<Check>) {
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
        out.push(plain(blob(&format!("index-written-back-{pre}"), ops)));
    }
}

/// A loop that moves the stack down 16 bytes each turn; 70,000 turns run off the stack.
fn alloca_loop_off_the_stack(out: &mut Vec<Check>) {
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
        out.push(plain(blob(&format!("alloca-{turns}"), ops)));
    }
}

/// The same with structures: each turn stores four registers at a new frame and pops half of it with a
/// structure load post-indexed by its 64 bytes; 20,000 turns run off the stack.
fn structure_frames_off_the_stack(out: &mut Vec<Check>) {
    for turns in [1000u32, 20000] {
        let mut ops: Ops = VecAssembler::new(0);
        zero(&mut ops);
        let top = ops.new_dynamic_label();
        dynasm!(ops ; .arch aarch64
            ; mov x29, sp
            ; movz x3, #turns
            ; =>top
            ; sub sp, sp, #128
            ; .u32 0x4c00_23e0 // st1 {v0.16b-v3.16b}, [sp]
            ; .u32 0x4cdf_03e0 // ld4 {v0.16b-v3.16b}, [sp], #64
            ; subs x3, x3, #1
            ; b.ne =>top
            ; mov sp, x29
        );
        dump(&mut ops);
        out.push(plain(blob(&format!("structure-frames-{turns}"), ops)));
    }
}

/// Loops that make no call read or write through a base and an index moved by a fixed step each turn,
/// until the access reaches the page after the blob, which allows nothing. These accesses keep the page they
/// last found allowed, so the fault must still land on the exact turn.
fn loops_running_onto_a_forbidden_page(out: &mut Vec<Check>) {
    let page = wcontainers_abi::PAGE_SIZE as u32;
    for step in [1u32, 8, page - 1, page, page + 1] {
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
            ops.extend(std::iter::repeat_n(
                0u8,
                3 * wcontainers_abi::PAGE_SIZE as usize,
            ));
            let name = format!("loop-step-{step}-{}", if write { "store" } else { "load" });
            out.push(plain(blob(&name, ops)));
        }
    }
}

/// The same with the longest accesses there are, structures of three and four 128 bit registers (48 and 64
/// bytes), so the access that runs onto the forbidden page starts well inside the last allowed one.
fn structure_loops_running_onto_a_forbidden_page(out: &mut Vec<Check>) {
    // ld1 and st1 of four registers, ld4 and st4, ld1 and st3 of three, all of 16 bytes, through x7.
    let forms = [
        ("ld1x4", 0x4c40_20e0u32),
        ("st1x4", 0x4c00_20e0),
        ("ld4", 0x4c40_00e0),
        ("st4", 0x4c00_00e0),
        ("ld1x3", 0x4c40_60e0),
        ("st3", 0x4c00_40e0),
    ];
    for step in [16u32, 32, 64] {
        for (name, word) in forms {
            let mut ops: Ops = VecAssembler::new(0);
            zero(&mut ops);
            dynasm!(ops ; .arch aarch64
                ; movz x1, #0x40, lsl #16
                ; movz x3, #0x200
                ; movz x6, step
                ; ->turn:
                ; add x7, x1, x3
                ; .u32 word
                ; add x3, x3, x6
                ; add x5, x5, #1
                ; b ->turn
            );
            dump(&mut ops);
            ops.extend(std::iter::repeat_n(
                0u8,
                3 * wcontainers_abi::PAGE_SIZE as usize,
            ));
            out.push(plain(blob(&format!("structure-step-{step}-{name}"), ops)));
        }
    }
}

/// A counted loop of loads whose range is proven allowed at its entry, or not when its last turns run onto
/// the page after the blob: the loop that is not proven is counted once for each entry.
fn loop_not_proven_is_counted(out: &mut Vec<Check>) {
    for (turns, entries) in [(1000u32, 0u64), (4000, 1)] {
        let mut ops: Ops = VecAssembler::new(0);
        zero(&mut ops);
        dynasm!(ops ; .arch aarch64
            ; movz x1, #0x40, lsl #16
            ; movz x3, turns
            ; ->turn:
            ; ldr x2, [x1], #8
            ; add x4, x4, x2
            ; subs x3, x3, #1
            ; b.ne ->turn
        );
        dump(&mut ops);
        ops.extend(std::iter::repeat_n(
            0u8,
            3 * wcontainers_abi::PAGE_SIZE as usize,
        ));
        out.push(Check {
            blob: blob(&format!("not-proven-{turns}"), ops),
            counted: Some((Reason::LoopNotProven, Counted::Exactly(entries))),
            at_entry: false,
        });
    }
}

/// Loops of structure loads and stores post-indexed by their size are proven too: counted when one runs onto
/// the page after the blob.
fn structure_loops_are_proven(out: &mut Vec<Check>) {
    // ld1 {v0.16b-v3.16b}, [x1], #64; st4 {v0.16b-v3.16b}, [x1], #64; ld4r {v0.16b-v3.16b}, [x1], #4.
    for (name, word, bytes) in [
        ("ld1x4", 0x4cdf_2020u32, 64u32),
        ("st4", 0x4c9f_0020, 64),
        ("ld4r", 0x4dff_e020, 4),
    ] {
        for (span, entries) in [(8192u32, 0u64), (20480, 1)] {
            let turns = span / bytes;
            let mut ops: Ops = VecAssembler::new(0);
            zero(&mut ops);
            // From the second page, after the code.
            dynasm!(ops ; .arch aarch64
                ; movz x1, #0x40, lsl #16
                ; movk x1, #0x1000
                ; movz x3, turns
                ; ->turn:
                ; .u32 word
                ; subs x3, x3, #1
                ; b.ne ->turn
            );
            dump(&mut ops);
            ops.extend(std::iter::repeat_n(
                0u8,
                3 * wcontainers_abi::PAGE_SIZE as usize,
            ));
            out.push(Check {
                blob: blob(&format!("structure-proven-{name}-{span}"), ops),
                counted: Some((Reason::LoopNotProven, Counted::Exactly(entries))),
                at_entry: false,
            });
        }
    }
}

fn plain(blob: Blob) -> Check {
    Check {
        blob,
        counted: None,
        at_entry: false,
    }
}

/// A check that fails on a function's first instruction and leaves to the bottom loop there.
fn at_entry(blob: Blob) -> Check {
    Check {
        blob,
        counted: Some((Reason::LeftToAddress, Counted::Some)),
        at_entry: true,
    }
}

pub(super) fn checks() -> Vec<Check> {
    let mut out = vec![
        Check {
            blob: recursion(100),
            counted: Some((Reason::LeftToAddress, Counted::Exactly(0))),
            at_entry: false,
        },
        at_entry(recursion(300)),
        at_entry(group_entry()),
        at_entry(frame_below(false)),
        at_entry(frame_below(true)),
    ];
    group_crossing_onto_a_forbidden_page(&mut out);
    index_written_back_by_a_narrow_load(&mut out);
    alloca_loop_off_the_stack(&mut out);
    structure_frames_off_the_stack(&mut out);
    loops_running_onto_a_forbidden_page(&mut out);
    structure_loops_running_onto_a_forbidden_page(&mut out);
    loop_not_proven_is_counted(&mut out);
    structure_loops_are_proven(&mut out);
    out
}

/// The page of data after the code page of the programs below.
use crate::programs::PAGE_AFTER_LOAD as DATA;

/// Start with the registers the reference does not clear at zero and x5 at `DATA`.
fn start(ops: &mut Ops) {
    for r in 0..22u8 {
        dynasm!(ops ; .arch aarch64 ; movz X(r), #0);
    }
    dynasm!(ops ; .arch aarch64 ; movz x5, #0x40, lsl #16 ; movk x5, #0x1000);
}

/// A system call that writes nothing.
fn write_nothing(ops: &mut Ops) {
    dynasm!(ops ; .arch aarch64 ; movz x0, #1 ; mov x1, x5 ; movz x2, #0 ; movz x8, #64 ; svc #0);
}

/// Exit with 0 after the program's code, and two pages of data after the code page.
fn data_blob(name: &str, mut ops: Ops) -> Blob {
    dynasm!(ops ; .arch aarch64 ; movz x0, #0 ; movz x8, #94 ; svc #0);
    let mut bytes = ops.finalize().expect("labels");
    bytes.resize(3 * PAGE_SIZE as usize, 0);
    Blob {
        name: name.into(),
        code_bytes: PAGE_SIZE,
        bytes,
        load: LOAD,
        link_address: None,
        main: None,
        program_headers: None,
        stack_bytes: STACK_BYTES,
    }
}

/// A function that makes a system call like `write_nothing` and returns, at `f`.
fn writing_nothing(ops: &mut Ops, f: DynamicLabel) {
    dynasm!(ops ; .arch aarch64 ; =>f);
    write_nothing(ops);
    dynasm!(ops ; .arch aarch64 ; ret);
}

/// The page below the one sp is moved to.
const BELOW: u64 = 0x480e_f000;

/// Move sp to the top of `BELOW`, then make and pop a frame there (x29 keeps the old sp).
fn pop_a_frame_at_below(ops: &mut Ops) {
    dynasm!(ops ; .arch aarch64
        ; movz x6, #0x480f, lsl #16
        ; mov x29, sp
        ; mov sp, x6
        ; sub sp, sp, #64
        ; str x6, [sp]
        ; add sp, sp, #64
    );
}

/// Two loads through one base with a system call between them: the second is checked again.
fn group_across_a_syscall(out: &mut Vec<Change>) {
    let mut ops: Ops = VecAssembler::new(0);
    start(&mut ops);
    dynasm!(ops ; .arch aarch64 ; ldr x6, [x5]);
    write_nothing(&mut ops);
    dynasm!(ops ; .arch aarch64 ; ldr x7, [x5, #8]);
    out.push(Change {
        blob: data_blob("group-across-syscall", ops),
        at: None,
        page: DATA,
    });
}

/// A loop that loads through a base and then makes a system call: the next turn's load is checked again.
fn loop_holding_a_syscall(out: &mut Vec<Change>) {
    let mut ops: Ops = VecAssembler::new(0);
    start(&mut ops);
    let top = ops.new_dynamic_label();
    dynasm!(ops ; .arch aarch64
        ; movz x16, #3
        ; =>top
        ; ldr x6, [x5, x16, lsl #3]
        ; add x9, x9, x6
    );
    write_nothing(&mut ops);
    dynasm!(ops ; .arch aarch64 ; subs x16, x16, #1 ; b.ne =>top);
    out.push(Change {
        blob: data_blob("loop-holding-syscall", ops),
        at: None,
        page: DATA,
    });
}

/// Two loads through one base with a call between them whose callee makes a system call.
fn group_across_a_call(out: &mut Vec<Change>) {
    let mut ops: Ops = VecAssembler::new(0);
    start(&mut ops);
    let f = ops.new_dynamic_label();
    let end = ops.new_dynamic_label();
    dynasm!(ops ; .arch aarch64 ; ldr x6, [x5] ; bl =>f ; ldr x7, [x5, #8] ; b =>end);
    writing_nothing(&mut ops, f);
    dynasm!(ops ; .arch aarch64 ; =>end);
    out.push(Change {
        blob: data_blob("group-across-call", ops),
        at: None,
        page: DATA,
    });
}

/// A loop over memory holding a call whose callee makes a system call.
fn loop_holding_a_call(out: &mut Vec<Change>) {
    let mut ops: Ops = VecAssembler::new(0);
    start(&mut ops);
    let f = ops.new_dynamic_label();
    let top = ops.new_dynamic_label();
    let end = ops.new_dynamic_label();
    dynasm!(ops ; .arch aarch64
        ; movz x16, #3
        ; =>top
        ; ldr x6, [x5, x16, lsl #3]
        ; add x9, x9, x6
        ; bl =>f
        ; subs x16, x16, #1
        ; b.ne =>top
        ; b =>end
    );
    writing_nothing(&mut ops, f);
    dynasm!(ops ; .arch aarch64 ; =>end);
    out.push(Change {
        blob: data_blob("loop-holding-call", ops),
        at: None,
        page: DATA,
    });
}

/// Below sp once a frame is popped is no frame: a change there at a system call is seen at the next access.
fn below_sp_after_a_pop_across_a_syscall(out: &mut Vec<Change>) {
    let mut ops: Ops = VecAssembler::new(0);
    start(&mut ops);
    pop_a_frame_at_below(&mut ops);
    write_nothing(&mut ops);
    dynasm!(ops ; .arch aarch64 ; ldur x9, [sp, #-8] ; mov sp, x29);
    out.push(Change {
        blob: data_blob("below-sp-across-syscall", ops),
        at: None,
        page: BELOW,
    });
}

/// The same with the access in a loop and the change at its first stop.
fn below_sp_after_a_pop_in_a_loop(out: &mut Vec<Change>) {
    let mut ops: Ops = VecAssembler::new(0);
    start(&mut ops);
    pop_a_frame_at_below(&mut ops);
    let top = ops.new_dynamic_label();
    dynasm!(ops ; .arch aarch64 ; movz x16, #4);
    let head = LOAD + ops.offset().0 as u64;
    dynasm!(ops ; .arch aarch64
        ; =>top
        ; ldur x9, [sp, #-8]
        ; add x10, x10, x9
        ; subs x16, x16, #1
        ; b.ne =>top
        ; mov sp, x29
    );
    // At the head, after the first turn.
    out.push(Change {
        blob: data_blob("below-sp-in-a-loop", ops),
        at: Some((head, X16, 3)),
        page: BELOW,
    });
}

/// A counted loop of 64 byte loads walking to the end of `DATA`, the last one 32 bytes onto the page after,
/// which is made to allow nothing at the first stop: the loop's proof fails and its checked copy keeps the run
/// of pages it finds allowed, which must not let that last load through.
fn longest_access_onto_a_page_changed_at_a_stop(out: &mut Vec<Change>) {
    // ld1 {v0.16b-v3.16b}, [x7]
    let ld1x4 = 0x4c40_20e0u32;
    for (name, word) in [("ld1x4", ld1x4), ("st1x4", ld1x4 & !(1 << 22))] {
        let mut ops: Ops = VecAssembler::new(0);
        start(&mut ops);
        dynasm!(ops ; .arch aarch64 ; add x7, x5, #0xf80 ; movz x16, #4);
        let head = LOAD + ops.offset().0 as u64;
        let top = ops.new_dynamic_label();
        dynasm!(ops ; .arch aarch64
            ; =>top
            ; .u32 word
            ; add x7, x7, #32
            ; subs x16, x16, #1
            ; b.ne =>top
        );
        out.push(Change {
            blob: data_blob(&format!("longest-access-{name}"), ops),
            at: Some((head, X16, 3)),
            page: DATA + PAGE_SIZE,
        });
    }
}

pub(super) fn changes() -> Vec<Change> {
    let mut out = Vec::new();
    group_across_a_syscall(&mut out);
    loop_holding_a_syscall(&mut out);
    group_across_a_call(&mut out);
    loop_holding_a_call(&mut out);
    below_sp_after_a_pop_across_a_syscall(&mut out);
    below_sp_after_a_pop_in_a_loop(&mut out);
    longest_access_onto_a_page_changed_at_a_stop(&mut out);
    out
}
