//! The guest functions the stepping tests run: each with its seeded starting states.

use wcontainers_abi::{PAGE_SIZE, RegisterBlock};
use wcontainers_arm64::carry::{COUNT, PAGE, SCRATCH};
use wcontainers_arm64::encode::{self, Register, field};
use wcontainers_arm64::{FPCR_BITS, FPSR_BITS};
use wcontainers_translator_test::guest::arm64::words::Case;
use wcontainers_translator_test::layout::{CODE, DATA, DATA_WRITABLE};

use wcontainers_translator::LookAhead;

use super::nzcv;

/// Where the functions return to: an address with no code.
pub const RETURNS: u64 = CODE + PAGE_SIZE;
/// The stack the functions start with, in the writable data.
pub const STACK: u64 = DATA + DATA_WRITABLE / 2;
/// The general registers: x0 to x30, then sp, flags and the thread pointer in a case.
pub const GENERAL: usize = RegisterBlock::ZERO.x.len();
pub const LINK: usize = GENERAL - 1;
pub const SEED_VARIABLE: &str = "WCONTAINERS_INTERRUPTS_SEED";
pub const STARTS_VARIABLE: &str = "WCONTAINERS_INTERRUPTS_STARTS";
/// The seed unless told otherwise.
pub const DEFAULT_SEED: u64 = 0x1405_7b7e_f767_814f;
/// Seeded starting states per case unless told otherwise.
pub const DEFAULT_STARTS: u64 = 1;
/// A bounded look-ahead shorter than most of the functions' straight-line runs, so that runs are cut.
pub const SHORT: u32 = 3;
/// Every look-ahead the functions run at; look-ahead none first, whose stops are every boundary.
pub const LOOK_AHEADS: [LookAhead; 3] =
    [LookAhead::None, LookAhead::Bounded(SHORT), LookAhead::Full];

/// Sums `x1` words from `x0`, keeping the sum in x27 and the pointer in x28 (both withheld: stand-ins).
pub const SUM: [u32; 8] = [
    0xd280001b, // mov x27, #0
    0xaa0003fc, // mov x28, x0
    0xf8408782, // ldr x2, [x28], #8
    0x8b02037b, // add x27, x27, x2
    0xf1000421, // subs x1, x1, #1
    0x54ffffa1, // b.ne back to the load
    0xaa1b03e0, // mov x0, x27
    0xd65f03c0, // ret
];

/// A frame, a direct call, a call through a register and a leaf with a conditional branch.
pub const CALLS: [u32; 12] = [
    0xa9bf7bfd, // stp x29, x30, [sp, #-16]!
    0x910003fd, // mov x29, sp
    0xd28000a0, // mov x0, #5
    0x94000005, // bl leaf
    0x10000083, // adr x3, leaf
    0xd63f0060, // blr x3
    0xa8c17bfd, // ldp x29, x30, [sp], #16
    0xd65f03c0, // ret
    0x91000c00, // leaf: add x0, x0, #3
    0xb4000040, // cbz x0, the second ret
    0xd65f03c0, // ret
    0xd65f03c0, // ret
];

/// A call through a register in a loop, so that it and the leaf's return go to their last targets.
pub const LOOPED_CALLS: [u32; 12] = [
    0xa9bf7bfd, // stp x29, x30, [sp, #-16]!
    0x910003fd, // mov x29, sp
    0xd28000a0, // mov x0, #5
    0x100000e3, // adr x3, leaf
    0xd2800064, // mov x4, #3
    0xd63f0060, // blr x3
    0xf1000484, // subs x4, x4, #1
    0x54ffffc1, // b.ne back to the blr
    0xa8c17bfd, // ldp x29, x30, [sp], #16
    0xd65f03c0, // ret
    0x91000c00, // leaf: add x0, x0, #3
    0xd65f03c0, // ret
];

/// Calls through a register whose target changes every turn, and a leaf called from two places in turn, so that
/// both the call's and the leaf's return's last targets miss.
pub const ALTERNATING_CALLS: [u32; 19] = [
    0xa9bf7bfd, // stp x29, x30, [sp, #-16]!
    0x910003fd, // mov x29, sp
    0xd2800000, // mov x0, #0
    0xd2800084, // mov x4, #4
    0x10000163, // adr x3, the first leaf
    0x10000185, // adr x5, the second leaf
    0xd63f0060, // blr x3
    0x94000008, // bl the first leaf
    0xaa0303e6, // mov x6, x3
    0xaa0503e3, // mov x3, x5
    0xaa0603e5, // mov x5, x6
    0xf1000484, // subs x4, x4, #1
    0x54ffff41, // b.ne back to the blr
    0xa8c17bfd, // ldp x29, x30, [sp], #16
    0xd65f03c0, // ret
    0x91000400, // the first leaf: add x0, x0, #1
    0xd65f03c0, // ret
    0x91000800, // the second leaf: add x0, x0, #2
    0xd65f03c0, // ret
];

/// An exclusive pair in a loop, a compare and swap, loads and stores across a page, a float operation that
/// raises a flag, and the float status read and written.
pub const ATOMICS: [u32; 16] = [
    0xc85f7c02, // ldxr x2, [x0]
    0x91000442, // add x2, x2, #1
    0xc8037c02, // stxr w3, x2, [x0]
    0x35ffffa3, // cbnz w3, back to the ldxr
    0xd2800004, // mov x4, #0
    0xd28000e5, // mov x5, #7
    0xc8a47c05, // cas x4, x5, [x0]
    0xf9400026, // ldr x6, [x1]
    0xf9000026, // str x6, [x1]
    0xa93f8826, // stp x6, x2, [x1, #-8]
    0x1e6e1000, // fmov d0, #1.0
    0x1e611001, // fmov d1, #3.0
    0x1e611802, // fdiv d2, d0, d1
    0xd53b4427, // mrs x7, fpsr
    0xd51b443f, // msr fpsr, xzr
    0xd65f03c0, // ret
];

/// A dispatch loop through a jump table of bytes that one of its operations rewrites: the stream x9 is stored at
/// x0, the table x10 at x0 + 16; operation 0 adds one, 1 doubles, 2 swaps the table's first two entries, 3
/// returns, and an entry of 255 goes where there is no code.
pub const JUMP_TABLE: [u32; 19] = [
    0xf9000009, // str x9, [x0]
    0x91004006, // add x6, x0, #16
    0xf90000ca, // str x10, [x6]
    0xd2800005, // mov x5, #0
    0x38401403, // ldrb w3, [x0], #1
    0x386368c3, // ldrb w3, [x6, x3]
    0x10000061, // adr x1, the base
    0x8b030821, // add x1, x1, x3, lsl #2
    0xd61f0020, // br x1
    0x910004a5, // the base: add x5, x5, #1
    0x17fffffa, // b back to the first ldrb
    0x8b0500a5, // add x5, x5, x5
    0x17fffff8, // b back to the first ldrb
    0x794000c8, // ldrh w8, [x6]
    0x5ac00508, // rev16 w8, w8
    0x790000c8, // strh w8, [x6]
    0x17fffff4, // b back to the first ldrb
    0xaa0503e0, // mov x0, x5
    0xd65f03c0, // ret
];

/// As [`JUMP_TABLE`] with signed bytes: 0 adds one, -1 doubles then adds one, -8 adds ten, 2 returns, and -128
/// goes back before the code.
pub const SIGNED_JUMP_TABLE: [u32; 17] = [
    0xf9000009, // str x9, [x0]
    0x91004006, // add x6, x0, #16
    0xf90000ca, // str x10, [x6]
    0xd2800005, // mov x5, #0
    0x14000003, // b to the first ldrb
    0x910028a5, // add x5, x5, #10
    0x14000001, // b to the first ldrb
    0x38401403, // ldrb w3, [x0], #1
    0x386368c3, // ldrb w3, [x6, x3]
    0x10000081, // adr x1, the base
    0x8b238821, // add x1, x1, w3, sxtb #2
    0xd61f0020, // br x1
    0x8b0500a5, // add x5, x5, x5
    0x910004a5, // the base: add x5, x5, #1
    0x17fffff9, // b back to the first ldrb
    0xaa0503e0, // mov x0, x5
    0xd65f03c0, // ret
];

/// A byte load that writes back its base, which is then the register added to the jump's base: the target is
/// a pointer's worth past it, not one of a byte's.
pub const WRITTEN_BACK_INDEX: [u32; 6] = [
    0x10000081, // adr x1, the base
    0x38401469, // ldrb w9, [x3], #1
    0x8b030821, // add x1, x1, x3, lsl #2
    0xd61f0020, // br x1
    0xd28006e0, // the base: mov x0, #55
    0xd65f03c0, // ret
];

/// A loop of loads at index x3 from bases x0 and x1 that no instruction of it writes, summing their products
/// in x5: the bases' alignment is tested once before the loop.
pub const INVARIANT_BASES: [u32; 8] = [
    0xd2800005, // mov x5, #0
    0xf8637802, // ldr x2, [x0, x3, lsl #3]
    0xf8637824, // ldr x4, [x1, x3, lsl #3]
    0x9b041445, // madd x5, x2, x4, x5
    0xf1000463, // subs x3, x3, #1
    0x54ffff81, // b.ne back to the first ldr
    0xaa0503e0, // mov x0, x5
    0xd65f03c0, // ret
];

/// Search's loops: x2 rounds, each over x1 bytes from x0 + 1 matching the round's low two bits, with a way out
/// of the inner loop back to the outer one and a way on from its branch back that comes back to it.
pub const NEST: [u32; 29] = [
    0x2a1f03e9, // mov w9, wzr
    0xaa1f03e8, // mov x8, xzr
    0x9100040b, // add x11, x0, #1
    0x14000004, // b to the cbz
    0x11000529, // add w9, w9, #1
    0x6b02013f, // cmp w9, w2
    0x540002a0, // b.eq to the mov x0
    0x34ffffa1, // cbz w1, back to the add w9
    0x1200052d, // and w13, w9, #3
    0xaa1f03ec, // mov x12, xzr
    0xaa0b03ee, // mov x14, x11
    0xaa0103ef, // mov x15, x1
    0x14000005, // b to the ldurb
    0xf10005ef, // subs x15, x15, #1
    0x910005ce, // add x14, x14, #1
    0x9100058c, // add x12, x12, #1
    0x54fffe80, // b.eq back to the add w9
    0x385ff1d0, // ldurb w16, [x14, #-1]
    0x12000610, // and w16, w16, #3
    0x6b1001bf, // cmp w13, w16
    0x54ffff21, // b.ne back to the subs
    0x394001d0, // ldrb w16, [x14]
    0x12000610, // and w16, w16, #3
    0x6b1001bf, // cmp w13, w16
    0x9a8c03f0, // csel x16, xzr, x12, eq
    0x8b080208, // add x8, x16, x8
    0x17fffff3, // b back to the subs
    0xaa0803e0, // mov x0, x8
    0xd65f03c0, // ret
];

/// Accesses through sp that may cross a page, and a vector pair into one register through sp, `x3` turns from
/// the stack `x0`.
pub const STACK_CROSSING: [u32; 9] = [
    0x910003e9, // mov x9, sp
    0x9100001f, // mov sp, x0
    0x3cc083e0, // ldur q0, [sp, #8]
    0xf840c3e1, // ldur x1, [sp, #12]
    0xad4007e1, // ldp q1, q1, [sp]
    0xf1000463, // subs x3, x3, #1
    0x54ffff81, // b.ne back to the ldur q0
    0x9100013f, // mov sp, x9
    0xd65f03c0, // ret
];

/// A 128-bit exclusive pair in a loop at `x0`, an acquire load and release store at `x1`, and a load and
/// store exclusive through sp.
pub const EXCLUSIVE_PAIR_LOOP: [u32; 9] = [
    0xc87f0c02, // ldxp x2, x3, [x0]
    0x91000442, // add x2, x2, #1
    0xc8240c02, // stxp w4, x2, x3, [x0]
    0x35ffffa4, // cbnz w4, back to the ldxp
    0xc8dffc25, // ldar x5, [x1]
    0xc89ffc25, // stlr x5, [x1]
    0xc85fffe6, // ldaxr x6, [sp]
    0xc807ffe6, // stlxr w7, x6, [sp]
    0xd65f03c0, // ret
];

/// Test-bit branches forward and back, a backward `cbnz` on a withheld register, forward `b`s and a backward
/// `b` across more than the short look-ahead: guest branches E does not count, beside ones it does.
pub const FORWARD_BRANCHES: [u32; 18] = [
    0xd2800000, // mov x0, #0
    0xd28000bc, // mov x28, #5
    0x91000400, // add x0, x0, #1
    0xd100079c, // sub x28, x28, #1
    0x37000060, // tbnz w0, #0, to the eor
    0x91000800, // add x0, x0, #2
    0x14000002, // b to the tbz
    0xd27d0000, // eor x0, x0, #8
    0xb6f8005c, // tbz x28, #63, to the cbnz
    0x14000008, // b to the ret
    0xb5ffff1c, // cbnz x28, back to the add x0, x0, #1
    0x91000400, // add x0, x0, #1
    0x91000400, // add x0, x0, #1
    0x91000400, // add x0, x0, #1
    0x91000400, // add x0, x0, #1
    0xb400005c, // cbz x28, to the ret
    0x17fffff2, // b back to the first add x0, x0, #1
    0xd65f03c0, // ret
];

/// `clrex` between a load and a store exclusive: the store fails.
pub const CLEARED: [u32; 4] = [
    0xc85f7c02, // ldxr x2, [x0]
    0xd5033f5f, // clrex
    0xc8037c02, // stxr w3, x2, [x0]
    0xd65f03c0, // ret
];

/// A load exclusive and a store exclusive whose addresses the cases choose: the same writable one, the same
/// read-only one (the store faults), different ones, and a misaligned store.
pub const EXCLUSIVE: [u32; 3] = [
    0xc85f7c02, // ldxr x2, [x0]
    0xc8037c22, // stxr w3, x2, [x1]
    0xd65f03c0, // ret
];

/// Fix-ups on stand-ins: a constant built for a withheld register, a store exclusive whose status, data and
/// base are all withheld, and a load pair into one register.
pub fn withheld() -> &'static [u32] {
    let named = |word: u32, fields: &[(encode::FieldName, Register)]| {
        fields.iter().fold(word, |w, &(f, r)| {
            encode::replace(w, f, r.into()).expect("a register fits")
        })
    };
    vec![
        named(0x1000007b, &[(field::RD, SCRATCH)]), // adr t, #12
        named(0xaa1b03e1, &[(field::RM, SCRATCH)]), // mov x1, t
        named(0xc85f7f9b, &[(field::RT, SCRATCH), (field::RN, PAGE)]), // ldxr t, [page]
        named(
            0xc81a7f9b,
            &[(field::RS, COUNT), (field::RT, SCRATCH), (field::RN, PAGE)],
        ), // stxr wcount, t, [page]
        named(0xa9400781, &[(field::RN, PAGE)]),    // ldp x1, x1, [page]
        0xd65f03c0,                                 // ret
    ]
    .leak()
}

/// System registers copied, answered and held as constants, a cache operation the inside answers, hints
/// that are whole pieces, and float status writes followed by them.
pub const SYSTEM: [u32; 21] = [
    0xd53bd042, // mrs x2, tpidr_el0
    0xd51bd043, // msr tpidr_el0, x3
    0xd503201f, // nop
    0xd53b4204, // mrs x4, nzcv
    0xd51b4206, // msr nzcv, x6
    0xd53b00e5, // mrs x5, dczid_el0
    0xd50b7530, // ic ivau, x16
    0x1e6e1000, // fmov d0, #1.0
    0x1e611001, // fmov d1, #3.0
    0x1e611802, // fdiv d2, d0, d1
    0xd51b442d, // msr fpsr, x13
    0xd503201f, // nop
    0xd503203f, // yield
    0xd53b4427, // mrs x7, fpsr
    0xd503201f, // nop
    0x91000631, // add x17, x17, #1
    0xd503201f, // nop
    0xd51b4408, // msr fpcr, x8
    0xd503201f, // nop
    0x9a9081ee, // csel x14, x15, x16, hi
    0xd65f03c0, // ret
];

/// Every float status form after a float operation that raised flags.
pub const FLOAT: [u32; 13] = [
    0x1e6e1000, // fmov d0, #1.0
    0x1e611001, // fmov d1, #3.0
    0x1e611802, // fdiv d2, d0, d1
    0xd53b4427, // mrs x7, fpsr
    0x1e611803, // fdiv d3, d0, d1
    0xd51b4428, // msr fpsr, x8
    0x1e611804, // fdiv d4, d0, d1
    0xd53b4409, // mrs x9, fpcr
    0x1e611805, // fdiv d5, d0, d1
    0xd51b440a, // msr fpcr, x10
    0x1e611806, // fdiv d6, d0, d1
    0xd53b442b, // mrs x11, fpsr
    0xd65f03c0, // ret
];

/// A case's starting state with the registers given, the flags, thread pointer, vectors and data area drawn
/// from `seed`.
pub fn start(seed: u64, registers: &[(usize, u64)]) -> Case {
    let mut rng = fastrand::Rng::with_seed(seed);
    let mut all = [0u64; GENERAL + 3];
    for &(r, v) in registers {
        all[r] = v;
    }
    all[LINK] = RETURNS;
    all[GENERAL] = STACK;
    all[GENERAL + 1] = rng.u64(..) & nzcv();
    all[GENERAL + 2] = rng.u64(..);
    Case {
        word: 0,
        registers: all,
        vectors: std::array::from_fn(|_| rng.u128(..)),
        float: [0, 0],
        tracking: false,
        seed,
    }
}

/// The float status cases: tracking on and off, FPCR default and not, each writing the other mode into
/// `fpcr_written` and every status bit into `fpsr_written`.
pub fn float_cases(
    seed: u64,
    fpsr_written: usize,
    fpcr_written: usize,
    others: &[(usize, u64)],
) -> Vec<Case> {
    let mut cases = Vec::new();
    for fpcr in [0, FPCR_BITS] {
        for tracking in [false, true] {
            let written = FPCR_BITS & !fpcr;
            let mut registers = vec![
                (fpsr_written, FPSR_BITS.into()),
                (fpcr_written, written.into()),
            ];
            registers.extend_from_slice(others);
            let mut case = start(seed, &registers);
            case.float = [0, fpcr];
            case.tracking = tracking;
            cases.push(case);
        }
    }
    cases
}

/// [`JUMP_TABLE`]'s operations, a byte each from the lowest: one that returns, and one that reaches entry 4.
pub const JUMPS: u64 = 0x0300_0201_0002_0100;
pub const JUMPS_NOWHERE: u64 = 0x0303_0303_0304_0100;
/// Its table: 0, 2, 4, 8 and 255.
pub const TABLE: u64 = 0xff_0804_0200;
/// [`SIGNED_JUMP_TABLE`]'s operations, one that returns and one that reaches entry 4, and its table: 0, -1, -8,
/// 2 and -128.
pub const SIGNED_JUMPS: u64 = 0x0301_0002_0100;
pub const SIGNED_JUMPS_BACK: u64 = u64::from_le_bytes([0, 4, 0, 0, 0, 0, 0, 0]);
pub const SIGNED_TABLE: u64 = 0x80_02f8_ff00;

/// The functions and their cases, starting states drawn from `seed`.
pub fn functions(seed: u64) -> Vec<(&'static str, &'static [u32], Vec<Case>)> {
    let start = |registers: &[(usize, u64)]| start(seed, registers);
    let words = 8;
    // Every value of the two low bits a round matches.
    let rounds = 4;
    let writable = DATA + PAGE_SIZE / 2;
    let read_only = DATA + DATA_WRITABLE + PAGE_SIZE / 2;
    let misaligned = writable + size_of::<u32>() as u64;
    // Misaligned, and the access at index two crosses into the next page.
    let crossing = DATA + PAGE_SIZE - 2 * size_of::<u64>() as u64 - size_of::<u32>() as u64;
    // A stack whose accesses past its first vector cross into the next page.
    let stack_at_page_end = DATA + PAGE_SIZE - size_of::<u128>() as u64;
    let turns = 3;
    let acquired = writable + 2 * size_of::<u128>() as u64;
    let mut rng = fastrand::Rng::with_seed(seed);
    let system = [
        (3, rng.u64(..)),
        (6, rng.u64(..) & nzcv()),
        (15, rng.u64(..)),
        (16, DATA),
    ];
    vec![
        ("sum", &SUM, vec![start(&[(0, DATA), (1, words)])]),
        ("calls", &CALLS, vec![start(&[])]),
        ("looped calls", &LOOPED_CALLS, vec![start(&[])]),
        ("alternating calls", &ALTERNATING_CALLS, vec![start(&[])]),
        (
            "atomics",
            &ATOMICS,
            vec![start(&[
                (0, writable),
                (1, DATA + PAGE_SIZE - size_of::<u32>() as u64),
            ])],
        ),
        ("cleared", &CLEARED, vec![start(&[(0, writable)])]),
        (
            "exclusive",
            &EXCLUSIVE,
            vec![
                start(&[(0, writable), (1, writable)]),
                start(&[(0, read_only), (1, read_only)]),
                start(&[(0, writable), (1, read_only)]),
                start(&[(0, writable), (1, misaligned)]),
            ],
        ),
        (
            "jump table",
            &JUMP_TABLE,
            vec![
                start(&[(0, writable), (9, JUMPS), (10, TABLE)]),
                start(&[(0, writable), (9, JUMPS_NOWHERE), (10, TABLE)]),
            ],
        ),
        (
            "signed jump table",
            &SIGNED_JUMP_TABLE,
            vec![
                start(&[(0, writable), (9, SIGNED_JUMPS), (10, SIGNED_TABLE)]),
                start(&[(0, writable), (9, SIGNED_JUMPS_BACK), (10, SIGNED_TABLE)]),
            ],
        ),
        (
            "written-back index",
            &WRITTEN_BACK_INDEX,
            vec![start(&[(3, writable)])],
        ),
        (
            "invariant bases",
            &INVARIANT_BASES,
            vec![
                start(&[(0, writable), (1, read_only), (3, words)]),
                start(&[(0, writable), (1, misaligned), (3, words)]),
                start(&[(0, writable), (1, crossing), (3, words)]),
            ],
        ),
        (
            "nest",
            &NEST,
            vec![start(&[(0, writable), (1, words), (2, rounds)])],
        ),
        (
            "stack crossing",
            &STACK_CROSSING,
            vec![
                start(&[(0, stack_at_page_end), (3, turns)]),
                start(&[(0, writable), (3, turns)]),
            ],
        ),
        (
            "exclusive pair loop",
            &EXCLUSIVE_PAIR_LOOP,
            vec![
                start(&[(0, writable), (1, acquired)]),
                start(&[(0, writable), (1, acquired + size_of::<u32>() as u64)]),
            ],
        ),
        ("forward branches", &FORWARD_BRANCHES, vec![start(&[])]),
        ("float", &FLOAT, float_cases(seed, 8, 10, &[])),
        ("system", &SYSTEM, float_cases(seed, 13, 8, &system)),
        (
            "withheld",
            withheld(),
            vec![
                start(&[(PAGE.into(), writable)]),
                start(&[(PAGE.into(), read_only)]),
            ],
        ),
    ]
}
