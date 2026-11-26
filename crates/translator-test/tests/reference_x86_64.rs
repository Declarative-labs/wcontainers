//! The x86-64 reference runner, built here for x86-64 Linux, uploaded and run on the reference machine through
//! the harness's transport, answers hand-made cases with what Intel's manual says, each checked here by hand:
//! among them every stop the runner reports (the trap flag's stop, a `mov ss` whose trap is held back, `int3`,
//! `int 3`, `int1`, the system call gates as SIGSYS, general protection, the stack fault, page faults, a divide
//! error, an illegal instruction), the x87 state written and read back through TOP, vector registers with their
//! upper halves, the thread pointers, a rep string instruction stepped to its end, two blobs and ordinary
//! executables, and the runner's own failure reported as its own.
//!
//! Needs `WC_REFERENCE_X86_64` (such as `tom@192.168.2.240`) on a host that is not x86-64; it fails without it.

use wcontainers_abi::PAGE_SIZE;
use wcontainers_translator_test::layout::{CODE, DATA, DATA_BYTES, DATA_WRITABLE, fill};
use wcontainers_translator_test::reference::{BlobRequest, CaseRan, Landed, Reference, Target};

const X86_64: Target = wcontainers_translator_test::x86_64::part::REFERENCE;

const SIGILL: u32 = 4;
const SIGTRAP: u32 = 5;
const SIGBUS: u32 = 7;
const SIGFPE: u32 = 8;
const SIGSEGV: u32 = 11;
const SIGSYS: u32 = 31;
const TRAP_BRKPT: u32 = 1;
const TRAP_TRACE: u32 = 2;
const SI_KERNEL: u32 = 128;
const SEGV_MAPERR: u32 = 1;
const SEGV_ACCERR: u32 = 2;
const ILL_ILLOPN: u32 = 2;
const FPE_INTDIV: u32 = 1;
const SYS_SECCOMP: u32 = 1;
const BUS_ADRALN: u32 = 1;

const CF: u128 = 1;
const PF: u128 = 4;
const AF: u128 = 0x10;
const ZF: u128 = 0x40;
const SF: u128 = 0x80;
const OF: u128 = 0x800;
const TF: u128 = 0x100;
const NT: u128 = 0x4000;
const AC: u128 = 0x4_0000;
const ID: u128 = 0x20_0000;
/// rflags as Linux starts a thread: IF and the reserved bit 1.
const START_FLAGS: u128 = 0x202;
const SEED: u64 = 0x5eed;
/// 1.0 and 2.0 as 80-bit values.
const ONE: u128 = 0x3fff_8000_0000_0000_0000;
const TWO: u128 = 0x4000_8000_0000_0000_0000;
const NON_CANONICAL: u128 = 1 << (u64::BITS - 1);

fn at(name: &str) -> usize {
    X86_64
        .fields
        .iter()
        .position(|f| *f == name)
        .unwrap_or_else(|| panic!("no field {name}"))
}

fn start() -> Vec<u128> {
    let mut state = vec![0u128; X86_64.fields.len()];
    state[at("rsp")] = (DATA + PAGE_SIZE).into();
    state[at("rflags")] = START_FLAGS;
    state[at("mxcsr")] = wcontainers_x86::MXCSR_AT_START.into();
    state[at("fcw")] = 0x37f;
    state
}

/// The data area's word at `offset` as the seed fills it.
fn data_word(offset: usize) -> u128 {
    let mut data = vec![0u8; DATA_BYTES as usize];
    fill(SEED, &mut data);
    u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap()).into()
}

struct Check<'a> {
    reference: &'a mut Reference,
    passed: usize,
}

impl Check<'_> {
    /// Run `code` from the start state changed by `setup`; check the stop, then hand the result to `then`.
    fn case(
        &mut self,
        name: &str,
        code: &[u8],
        setup: impl Fn(&mut Vec<u128>),
        stop: (u32, u32),
        then: impl Fn(&Landed, &[u128]),
    ) {
        let mut state = start();
        setup(&mut state);
        let landed = self
            .reference
            .case(code, SEED, &Default::default(), &state)
            .unwrap_or_else(|e| panic!("{name}: the runner failed: {e}"))
            .landed();
        assert_eq!(
            (landed.signal, landed.code),
            stop,
            "{name}: the stop, rip {:#x}",
            landed.state[at("rip")]
        );
        then(&landed, &state);
        self.passed += 1;
    }
}

fn rip(landed: &Landed) -> u128 {
    landed.state[at("rip")]
}

fn get(landed: &Landed, name: &str) -> u128 {
    landed.state[at(name)]
}

/// The changed data bytes from `offset` on, as the runner reports them.
fn changed_from(landed: &Landed, offset: u32, count: u32) -> Vec<(u32, u8)> {
    landed
        .changed
        .iter()
        .copied()
        .filter(|(o, _)| (offset..offset + count).contains(o))
        .collect()
}

/// The bytes `value` puts at `offset` that differ from the seed's.
fn expected_writes(offset: u32, value: &[u8]) -> Vec<(u32, u8)> {
    let mut data = vec![0u8; DATA_BYTES as usize];
    fill(SEED, &mut data);
    value
        .iter()
        .enumerate()
        .map(|(i, &b)| (offset + i as u32, b))
        .filter(|&(o, b)| data[o as usize] != b)
        .collect()
}

const STEP: (u32, u32) = (SIGTRAP, TRAP_TRACE);
const CODE128: u128 = CODE as u128;

#[test]
fn the_x86_64_reference_answers_by_the_manual() {
    let mut reference = Reference::start(&X86_64);
    eprintln!("reference CPU: {}", reference.model);
    let mut check = Check {
        reference: &mut reference,
        passed: 0,
    };

    // Plain instructions stop after themselves by the trap flag.
    check.case(
        "nop",
        &[0x90],
        |_| {},
        STEP,
        |l, s| {
            assert_eq!(rip(l), CODE128 + 1);
            assert_eq!(get(l, "rflags"), s[at("rflags")]);
        },
    );
    check.case(
        "mov rax, rbx",
        &[0x48, 0x89, 0xd8],
        |s| s[at("rbx")] = 0x1234_5678_9abc_def0,
        STEP,
        |l, _| {
            assert_eq!(get(l, "rax"), 0x1234_5678_9abc_def0);
            assert_eq!(rip(l), CODE128 + 3);
        },
    );
    check.case(
        "add rax, rbx: carry out of all ones",
        &[0x48, 0x01, 0xd8],
        |s| {
            s[at("rax")] = u64::MAX.into();
            s[at("rbx")] = 1;
        },
        STEP,
        |l, _| {
            assert_eq!(get(l, "rax"), 0);
            assert_eq!(get(l, "rflags"), START_FLAGS | CF | PF | AF | ZF);
        },
    );
    check.case(
        "sub eax, eax clears the upper half",
        &[0x29, 0xc0],
        |s| s[at("rax")] = u64::MAX.into(),
        STEP,
        |l, _| {
            assert_eq!(get(l, "rax"), 0);
            assert_eq!(get(l, "rflags"), START_FLAGS | PF | ZF);
        },
    );
    check.case(
        "inc rax overflows and keeps CF",
        &[0x48, 0xff, 0xc0],
        |s| {
            s[at("rax")] = i64::MAX as u128;
            s[at("rflags")] = START_FLAGS | CF;
        },
        STEP,
        |l, _| {
            assert_eq!(get(l, "rax"), 1 << 63);
            assert_eq!(get(l, "rflags"), START_FLAGS | CF | PF | AF | SF | OF);
        },
    );
    check.case(
        "lea rax, [rbx + rcx*4 + 0x10]",
        &[0x48, 0x8d, 0x44, 0x8b, 0x10],
        |s| {
            s[at("rbx")] = 0x1000;
            s[at("rcx")] = 3;
        },
        STEP,
        |l, _| assert_eq!(get(l, "rax"), 0x1000 + 12 + 0x10),
    );
    check.case(
        "shl rax, 4",
        &[0x48, 0xc1, 0xe0, 0x04],
        |s| s[at("rax")] = 0x0123_4567_89ab_cdef,
        STEP,
        |l, _| {
            assert_eq!(get(l, "rax"), 0x1234_5678_9abc_def0);
        },
    );
    check.case(
        "bswap eax",
        &[0x0f, 0xc8],
        |s| s[at("rax")] = 0xffff_ffff_1122_3344,
        STEP,
        |l, _| {
            assert_eq!(get(l, "rax"), 0x4433_2211);
        },
    );
    check.case(
        "div rbx",
        &[0x48, 0xf7, 0xf3],
        |s| {
            s[at("rax")] = 100;
            s[at("rbx")] = 7;
        },
        STEP,
        |l, _| {
            assert_eq!((get(l, "rax"), get(l, "rdx")), (14, 2));
        },
    );
    check.case(
        "push rax writes below rsp",
        &[0x50],
        |s| s[at("rax")] = 0x1122_3344_5566_7788,
        STEP,
        |l, s| {
            assert_eq!(get(l, "rsp"), s[at("rsp")] - 8);
            assert_eq!(
                changed_from(l, 0xff8, 8),
                expected_writes(0xff8, &0x1122_3344_5566_7788u64.to_le_bytes())
            );
        },
    );
    check.case(
        "pop rbx reads the data area",
        &[0x5b],
        |_| {},
        STEP,
        |l, s| {
            assert_eq!(get(l, "rbx"), data_word(0x1000));
            assert_eq!(get(l, "rsp"), s[at("rsp")] + 8);
        },
    );
    check.case(
        "mov rax, fs:[0] reads through the FS base",
        &[0x64, 0x48, 0x8b, 0x04, 0x25, 0, 0, 0, 0],
        |s| {
            s[at("fs_base")] = (DATA + 0x40).into();
        },
        STEP,
        |l, _| {
            assert_eq!(get(l, "rax"), data_word(0x40));
            assert_eq!(get(l, "fs_base"), (DATA + 0x40).into());
        },
    );
    check.case(
        "rdgsbase rax",
        &[0xf3, 0x48, 0x0f, 0xae, 0xc8],
        |s| s[at("gs_base")] = 0x7654_3210,
        STEP,
        |l, _| {
            assert_eq!(get(l, "rax"), 0x7654_3210);
        },
    );
    check.case(
        "movaps xmm0, xmm1",
        &[0x0f, 0x28, 0xc1],
        |s| s[at("xmm1")] = 0x0011_2233_4455_6677_8899_aabb_ccdd_eeff,
        STEP,
        |l, _| {
            assert_eq!(get(l, "xmm0"), 0x0011_2233_4455_6677_8899_aabb_ccdd_eeff);
        },
    );
    check.case(
        "vpaddd ymm0, ymm1, ymm2 adds both halves",
        &[0xc5, 0xf5, 0xfe, 0xc2],
        |s| {
            s[at("xmm1")] = 0x0000_0004_0000_0003_0000_0002_ffff_ffff;
            s[at("ymmh1")] = 0x0000_0008_0000_0007_0000_0006_0000_0005;
            s[at("xmm2")] = 0x0000_0001_0000_0001_0000_0001_0000_0002;
            s[at("ymmh2")] = 0x0000_0010_0000_0010_0000_0010_0000_0010;
        },
        STEP,
        |l, _| {
            assert_eq!(get(l, "xmm0"), 0x0000_0005_0000_0004_0000_0003_0000_0001);
            assert_eq!(get(l, "ymmh0"), 0x0000_0018_0000_0017_0000_0016_0000_0015);
        },
    );
    check.case(
        "stmxcsr [rbx]",
        &[0x0f, 0xae, 0x1b],
        |s| {
            s[at("rbx")] = (DATA + 0x200).into();
            s[at("mxcsr")] =
                (wcontainers_x86::MXCSR_AT_START | wcontainers_x86::MXCSR_ROUNDING).into();
        },
        STEP,
        |l, _| {
            assert_eq!(
                changed_from(l, 0x200, 4),
                expected_writes(
                    0x200,
                    &(wcontainers_x86::MXCSR_AT_START | wcontainers_x86::MXCSR_ROUNDING)
                        .to_le_bytes(),
                )
            );
            assert_eq!(
                get(l, "mxcsr"),
                u128::from(wcontainers_x86::MXCSR_AT_START | wcontainers_x86::MXCSR_ROUNDING)
            );
        },
    );
    check.case(
        "rep stosb is stepped to its end",
        &[0xf3, 0xaa],
        |s| {
            s[at("rcx")] = 3;
            s[at("rdi")] = (DATA + 0x100).into();
            s[at("rax")] = 0x5a;
        },
        STEP,
        |l, _| {
            assert_eq!(rip(l), CODE128 + 2);
            assert_eq!((get(l, "rcx"), get(l, "rdi")), (0, (DATA + 0x103).into()));
            assert_eq!(
                changed_from(l, 0x100, 3),
                expected_writes(0x100, &[0x5a; 3])
            );
        },
    );
    check.case(
        "jmp beyond the code area stops at its target",
        &[0xe9, 0, 0, 1, 0],
        |_| {},
        STEP,
        |l, _| {
            assert_eq!(rip(l), CODE128 + 5 + 0x10000);
        },
    );
    // call far [rip], through a pointer of 32-bit offset and selector 0x33 (Linux's 64-bit user code
    // segment) that follows it: with a 32-bit operand size it pushes CS, then the return eip, 4 bytes each.
    let target = (CODE + 0x100) as u32;
    let mut far = vec![0xff, 0x1d, 0, 0, 0, 0];
    far.extend_from_slice(&target.to_le_bytes());
    far.extend_from_slice(&0x33u16.to_le_bytes());
    check.case(
        "call far through selector 0x33",
        &far,
        |_| {},
        STEP,
        |l, s| {
            assert_eq!(rip(l), target.into());
            assert_eq!(get(l, "rsp"), s[at("rsp")] - 8);
            let mut pushed = ((CODE + 6) as u32).to_le_bytes().to_vec();
            pushed.extend_from_slice(&0x33u32.to_le_bytes());
            assert_eq!(changed_from(l, 0xff8, 8), expected_writes(0xff8, &pushed));
        },
    );

    // x87: written in physical order, read back through TOP.
    check.case(
        "fld1 pushes 1.0 into R7",
        &[0xd9, 0xe8],
        |_| {},
        STEP,
        |l, _| {
            assert_eq!(get(l, "fsw") >> 11 & 7, 7);
            assert_eq!(get(l, "r7"), ONE);
            assert_eq!(get(l, "ftw"), 0x80);
        },
    );
    check.case(
        "fadd st(0), st(0) with TOP 7",
        &[0xd8, 0xc0],
        |s| {
            s[at("fsw")] = 7 << 11;
            s[at("ftw")] = 0x80;
            s[at("r7")] = ONE;
        },
        STEP,
        |l, _| {
            assert_eq!(get(l, "r7"), TWO);
            assert_eq!(get(l, "fsw") >> 11 & 7, 7);
        },
    );
    check.case(
        "fld st(0) with TOP 2 fills R1",
        &[0xd9, 0xc0],
        |s| {
            s[at("fsw")] = 2 << 11;
            s[at("ftw")] = 0x04;
            s[at("r2")] = TWO;
        },
        STEP,
        |l, _| {
            assert_eq!((get(l, "r1"), get(l, "r2")), (TWO, TWO));
            assert_eq!(get(l, "fsw") >> 11 & 7, 1);
            assert_eq!(get(l, "ftw"), 0x06);
        },
    );

    // Traps, which leave rip after the instruction.
    check.case(
        "int3",
        &[0xcc],
        |_| {},
        (SIGTRAP, SI_KERNEL),
        |l, _| assert_eq!(rip(l), CODE128 + 1),
    );
    check.case(
        "int 3",
        &[0xcd, 0x03],
        |_| {},
        (SIGTRAP, SI_KERNEL),
        |l, _| assert_eq!(rip(l), CODE128 + 2),
    );
    check.case(
        "int1",
        &[0xf1],
        |_| {},
        (SIGTRAP, TRAP_BRKPT),
        |l, _| assert_eq!(rip(l), CODE128 + 1),
    );
    // mov ss holds the trap back one instruction, so the int3 filler after it stops the case.
    check.case(
        "mov ss, eax",
        &[0x8e, 0xd0],
        |s| s[at("rax")] = 0x2b,
        (SIGTRAP, SI_KERNEL),
        |l, _| {
            assert_eq!(rip(l), CODE128 + 3);
        },
    );
    // The system call gates never run: the filter turns each into SIGSYS.
    check.case(
        "syscall",
        &[0x0f, 0x05],
        |s| s[at("rax")] = 39,
        (SIGSYS, SYS_SECCOMP),
        |l, _| {
            assert_eq!(rip(l), CODE128 + 2);
        },
    );
    check.case(
        "int 0x80",
        &[0xcd, 0x80],
        |s| s[at("rax")] = 20,
        (SIGSYS, SYS_SECCOMP),
        |l, _| {
            assert_eq!(rip(l), CODE128 + 2);
        },
    );
    // sysenter's 32-bit entry first reads the caller's stack at rbp (a fault there is SIGSEGV), and its
    // SIGSYS arrives with rip in the vDSO.
    check.case(
        "sysenter",
        &[0x0f, 0x34],
        |s| {
            s[at("rax")] = 20;
            s[at("rbp")] = (DATA + PAGE_SIZE).into();
        },
        (SIGSYS, SYS_SECCOMP),
        |_, _| {},
    );

    // pushf from a state with NT and ID set stores both, and they stay set.
    check.case(
        "pushf with NT and ID",
        &[0x9c],
        |s| s[at("rflags")] = START_FLAGS | NT | ID,
        STEP,
        |l, _| {
            assert_eq!(get(l, "rflags") & (NT | ID), NT | ID);
            let word = l
                .changed
                .iter()
                .filter(|(o, _)| (0xff8..0x1000).contains(o))
                .fold(data_word(0xff8), |w, &(o, b)| {
                    let shift = 8 * (o - 0xff8);
                    w & !(0xff << shift) | u128::from(b) << shift
                });
            assert_eq!(word & (NT | ID), NT | ID);
        },
    );

    // Faults, which leave rip on the instruction and the state as before it.
    check.case(
        "ud2",
        &[0x0f, 0x0b],
        |_| {},
        (SIGILL, ILL_ILLOPN),
        |l, _| assert_eq!(rip(l), CODE128),
    );
    check.case(
        "div rbx by zero",
        &[0x48, 0xf7, 0xf3],
        |s| s[at("rax")] = 1,
        (SIGFPE, FPE_INTDIV),
        |l, _| {
            assert_eq!(rip(l), CODE128);
            assert_eq!(get(l, "rax"), 1);
        },
    );
    check.case(
        "ret to a non-canonical address",
        &[0xc3],
        |s| s[at("rsp")] = DATA.into(),
        (SIGSEGV, SI_KERNEL),
        |l, s| {
            assert!(
                !wcontainers_x86::is_canonical(data_word(0) as u64),
                "the seed's first word is not canonical"
            );
            assert_eq!((rip(l), get(l, "rsp")), (CODE128, s[at("rsp")]));
            assert_eq!(l.address, 0);
        },
    );
    check.case(
        "mov rax, [rbx] non-canonical: general protection",
        &[0x48, 0x8b, 0x03],
        |s| s[at("rbx")] = NON_CANONICAL,
        (SIGSEGV, SI_KERNEL),
        |l, _| {
            assert_eq!((rip(l), l.address), (CODE128, 0));
        },
    );
    check.case(
        "mov rax, [rbp] non-canonical: the stack fault",
        &[0x48, 0x8b, 0x45, 0x00],
        |s| s[at("rbp")] = NON_CANONICAL,
        (SIGBUS, SI_KERNEL),
        |l, _| {
            assert_eq!((rip(l), l.address), (CODE128, 0));
        },
    );
    check.case(
        "push rax with rsp non-canonical: the stack fault",
        &[0x50],
        |s| s[at("rsp")] = NON_CANONICAL + 8,
        (SIGBUS, SI_KERNEL),
        |l, s| {
            assert_eq!((rip(l), get(l, "rsp")), (CODE128, s[at("rsp")]));
        },
    );
    check.case(
        "mov rax, [rbx] unmapped: page fault",
        &[0x48, 0x8b, 0x03],
        |s| s[at("rbx")] = 0x1000,
        (SIGSEGV, SEGV_MAPERR),
        |l, _| {
            assert_eq!((rip(l), l.address), (CODE128, 0x1000));
        },
    );
    check.case(
        "mov [rbx], al read-only: page fault",
        &[0x88, 0x03],
        |s| s[at("rbx")] = (DATA + DATA_WRITABLE).into(),
        (SIGSEGV, SEGV_ACCERR),
        |l, _| {
            assert_eq!((rip(l), l.address), (CODE128, DATA + DATA_WRITABLE));
            assert!(l.changed.is_empty());
        },
    );

    // With AC set, an 8-byte load at an odd address: alignment check, which Linux reports with si_addr 0.
    check.case(
        "mov rax, [rbx] misaligned with AC",
        &[0x48, 0x8b, 0x03],
        |s| {
            s[at("rflags")] = START_FLAGS | AC;
            s[at("rbx")] = (DATA + 1).into();
        },
        (SIGBUS, BUS_ADRALN),
        |l, _| {
            assert_eq!((rip(l), l.address), (CODE128, 0));
        },
    );

    // popf writes TF; the runner's own TF is never reported.
    for (name, set) in [
        ("popf of a word with TF", true),
        ("popf of a word without TF", false),
    ] {
        let offset = (0..DATA_WRITABLE as usize - 8)
            .step_by(8)
            .find(|&o| (data_word(o) & TF != 0) == set)
            .unwrap();
        check.case(
            name,
            &[0x9d],
            |s| s[at("rsp")] = DATA as u128 + offset as u128,
            STEP,
            |l, _| {
                assert_eq!(rip(l), CODE128 + 1);
                assert_eq!(get(l, "rflags") & TF, data_word(offset) & TF);
            },
        );
    }
    check.case(
        "popf from an unmapped stack leaves TF clear",
        &[0x9d],
        |s| s[at("rsp")] = 0x1000,
        (SIGSEGV, SEGV_MAPERR),
        |l, s| {
            assert_eq!(rip(l), CODE128);
            assert_eq!(get(l, "rflags") & TF, 0);
            assert_eq!(get(l, "rsp"), s[at("rsp")]);
        },
    );
    check.case(
        "nop leaves TF clear",
        &[0x90],
        |_| {},
        STEP,
        |l, _| assert_eq!(get(l, "rflags") & TF, 0),
    );

    let passed = check.passed;
    eprintln!("{passed} hand-made cases answered by the manual");
    assert!(passed >= 30);
}

/// A thread pointer Linux does not accept (at or above the top of user space, 2^47 - 4096) is reported as
/// refused, not run; the runner's own thread pointer is untouched, so the next case runs.
#[test]
fn the_x86_64_reference_refuses_what_linux_refuses() {
    let mut reference = Reference::start(&X86_64);
    for (field, value) in [
        ("fs_base", NON_CANONICAL),
        ("fs_base", wcontainers_x86::USER_END.into()),
        ("gs_base", wcontainers_x86::USER_END.into()),
        ("gs_base", wcontainers_x86::USER_TOP.into()),
    ] {
        let mut state = start();
        state[at(field)] = value;
        let ran = reference
            .case(&[0x90], SEED, &Default::default(), &state)
            .expect("the runner");
        let CaseRan::Refused(why) = ran else {
            panic!("{field} {value:#x}: ran, {ran:?}");
        };
        assert!(why.contains(&field[..2].to_uppercase()), "{why}");
    }
    let mut state = start();
    state[at("fs_base")] = (DATA + 0x40).into();
    let landed = reference
        .case(
            &[0x64, 0x48, 0x8b, 0x04, 0x25, 0, 0, 0, 0],
            SEED,
            &Default::default(),
            &state,
        )
        .expect("the runner")
        .landed();
    assert_eq!((landed.signal, landed.code), STEP);
    assert_eq!(get(&landed, "rax"), data_word(0x40));
    let mut state = start();
    state[at("gs_base")] = (wcontainers_x86::USER_END - 2 * wcontainers_abi::PAGE_SIZE).into();
    let landed = reference
        .case(&[0x90], SEED, &Default::default(), &state)
        .expect("the runner")
        .landed();
    assert_eq!(
        get(&landed, "gs_base"),
        u128::from(wcontainers_x86::USER_END - 2 * wcontainers_abi::PAGE_SIZE)
    );
}

/// A flat blob prints and exits with its status; another stops by its own illegal instruction, with its stop
/// record.
#[test]
fn the_x86_64_reference_runs_blobs() {
    let mut reference = Reference::start(&X86_64);
    const LOAD: u64 = 0x40_0000;
    let message = b"hello\n";
    let mut code = vec![
        0xb8,
        1,
        0,
        0,
        0, // mov eax, 1 (write)
        0xbf,
        1,
        0,
        0,
        0, // mov edi, 1
        0x48,
        0x8d,
        0x35,
        0x13,
        0,
        0,
        0, // lea rsi, [rip + 0x13]: the message after the code
        0xba,
        message.len() as u8,
        0,
        0,
        0, // mov edx, 6
        0x0f,
        0x05, // syscall
        0xb8,
        231,
        0,
        0,
        0, // mov eax, 231 (exit_group)
        0xbf,
        7,
        0,
        0,
        0, // mov edi, 7
        0x0f,
        0x05, // syscall
    ];
    code.extend_from_slice(message);
    fn blob(bytes: &[u8]) -> BlobRequest<'_> {
        BlobRequest {
            bytes,
            load: LOAD,
            stack_top: 0x4810_0000,
            stack_bytes: 1 << 16,
            headers: None,
            auxiliary: &[],
            stdin: &[],
            runs: 0,
        }
    }
    let ran = reference.blob(&blob(&code)).expect("the runner");
    assert_eq!((ran.stdout.as_slice(), ran.status), (&message[..], 7));
    assert!(ran.stop.is_none());

    // nop; ud2: ends by SIGILL with rip on the ud2 and the registers of the blob start state.
    let ran = reference
        .blob(&blob(&[0x90, 0x0f, 0x0b]))
        .expect("the runner");
    assert_eq!(ran.status, 128 + SIGILL as i32);
    let (signal, code, registers) = ran.stop.expect("a stop record");
    assert_eq!((signal, code), (SIGILL, ILL_ILLOPN));
    assert_eq!(registers.len(), 18);
    assert_eq!(registers[16], LOAD + 1, "rip");
    assert_eq!(registers[0], 0, "rax");
    // The flags up to and including OF, the 8086's.
    let flags = (wcontainers_x86::OF << 1) - 1;
    assert_eq!(registers[17] & flags, 0x202, "rflags");
}

/// Ordinary executables built for x86-64 Linux run as Linux starts a program.
#[test]
fn the_x86_64_reference_runs_programs() {
    let mut reference = Reference::start(&X86_64);
    x86_64::check_ordinary_programs("x86_64-linux-musl", |executable| {
        let ran = reference.program(executable).expect("the runner");
        assert!(ran.stop.is_none());
        x86_64::Ran {
            stdout: ran.stdout,
            stderr: ran.stderr,
            status: ran.status,
        }
    });
}

/// A connection lost between requests or during one is started again and the request answered, as when
/// the machine drops off its network.
#[test]
fn the_x86_64_reference_comes_back_after_its_connection_is_lost() {
    let mut reference = Reference::start(&X86_64);
    let nop = |reference: &mut Reference| {
        let landed = reference
            .case(&[0x90], SEED, &Default::default(), &start())
            .expect("the runner")
            .landed();
        assert_eq!((landed.signal, landed.code), STEP);
        assert_eq!(rip(&landed), CODE128 + 1);
        landed
    };
    let before = nop(&mut reference);
    reference.cut();
    assert_eq!(nop(&mut reference), before);
    reference.cut();
    assert_eq!(nop(&mut reference), before);
}

/// A request the runner cannot read fails the run with the runner's own message, not as a lost connection.
#[test]
fn the_x86_64_reference_reports_its_own_failure() {
    Reference::check_runner_failure(&X86_64);
}

/// The tests with x86-64 as the guest.
mod x86_64 {
    pub(super) use wcontainers_translator_test::x86_64::programs::{Ran, check_ordinary_programs};
}
