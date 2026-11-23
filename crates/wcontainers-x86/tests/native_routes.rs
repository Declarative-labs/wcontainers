//! The static routing test (docs/x86-native-output.md section 7): every form of a seeded sweep of decoded
//! instructions is routed in every way, and none the plan keeps from the hardware is copied there.

use iced_x86::{Mnemonic, OpKind, Register};
use wcontainers_guest_cpu::Route;
use wcontainers_x86::native::{Check, HostMatches, Way, route};
use wcontainers_x86::{Offer, V3, decode};

const WAYS: [Way; 4] = [
    Way::Machine,
    Way::Process,
    Way::InProcess(Check::Wrapping),
    Way::InProcess(Check::Exact),
];

/// A host like the rented Xeon, whose results match the meanings, and one that matches nothing.
const HOSTS: [HostMatches; 2] = [
    HostMatches {
        five_level_paging: false,
        partial_writes: true,
        undefined_results: true,
    },
    HostMatches {
        five_level_paging: true,
        partial_writes: false,
        undefined_results: false,
    },
];

fn bytes(hex: &str) -> Vec<u8> {
    let hex: String = hex.split_whitespace().collect();
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex"))
        .collect()
}

fn offer() -> Offer {
    Offer::new(V3).expect("the v3 list has meanings")
}

fn route_of(way: Way, host: &HostMatches, hex: &str) -> Route {
    let mut code = bytes(hex);
    code.resize(16, 0);
    let (i, _) = decode(&code).expect("decodes");
    route(way, &i, host, &offer())
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

/// Bytes shaped like an instruction: some legacy prefixes, maybe a REX, an opcode map, then random bytes.
fn instruction_like(rng: &mut fastrand::Rng) -> [u8; 16] {
    const PREFIXES: [u8; 8] = [0x64, 0x65, 0x66, 0x67, 0xf2, 0xf3, 0xf0, 0x2e];
    const MAPS: [&[u8]; 5] = [&[], &[], &[0x0f], &[0x0f, 0x38], &[0x0f, 0x3a]];
    let mut code = [0; 16];
    rng.fill(&mut code);
    let mut at = 0;
    for _ in 0..rng.usize(0..4) {
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

fn memory(i: &wcontainers_x86::Instruction) -> bool {
    (0..i.iced.op_count()).any(|n| {
        matches!(
            i.iced.op_kind(n),
            OpKind::Memory
                | OpKind::MemorySegSI
                | OpKind::MemorySegESI
                | OpKind::MemorySegRSI
                | OpKind::MemorySegDI
                | OpKind::MemorySegEDI
                | OpKind::MemorySegRDI
        )
    })
}

/// The first of the plan's rules the route of `i` under `way` breaks.
fn broken(
    way: Way,
    host: &HostMatches,
    i: &wcontainers_x86::Instruction,
    r: Route,
) -> Option<&'static str> {
    use Mnemonic::*;
    if r == Route::Trap {
        return None;
    }
    let ins = &i.iced;
    let m = ins.mnemonic();
    let copied = r == Route::Copy;
    let in_process = matches!(way, Way::InProcess(_));
    let segment = ins.memory_segment();
    let on_area = if in_process {
        matches!(segment, Register::FS | Register::GS)
    } else {
        segment == Register::GS
    };
    let implicit = (0..ins.op_count()).any(|n| {
        matches!(
            ins.op_kind(n),
            OpKind::MemorySegSI
                | OpKind::MemorySegESI
                | OpKind::MemorySegRSI
                | OpKind::MemorySegDI
                | OpKind::MemorySegEDI
                | OpKind::MemorySegRDI
        )
    }) || m == Xlatb;
    let string = (0..ins.op_count()).any(|n| {
        matches!(
            ins.op_kind(n),
            OpKind::MemorySegRSI | OpKind::MemorySegESI | OpKind::MemoryESRDI | OpKind::MemoryESEDI
        )
    });
    let selector = (0..ins.op_count())
        .any(|n| ins.op_kind(n) == OpKind::Register && ins.op_register(n).is_segment_register());
    if matches!(
        m,
        Nop | Reservednop | Prefetchnta | Prefetcht0 | Prefetcht1 | Prefetcht2 | Prefetchw
    ) {
        return (r != Route::Nothing).then_some("a form that does nothing is carried");
    }
    if copied && ins.is_ip_rel_memory_operand() {
        return Some("a rip-relative operand copied");
    }
    if copied && m != Lea && memory(i) && on_area {
        return Some("an operand on the thread area's segment copied");
    }
    let overridden = implicit && on_area && ins.segment_prefix() != Register::None;
    if overridden && in_process && r != Route::Kernel {
        return Some(
            "an implicit address on a segment override not answered by the inside in process",
        );
    }
    if overridden && !in_process && !matches!(r, Route::Area | Route::Kernel) {
        return Some(
            "an implicit address on the thread area's segment not carried on the guest's base",
        );
    }
    if in_process && string && i.address32 && r != Route::Kernel {
        return Some(
            "a string with its own address-size prefix not answered by the inside in process",
        );
    }
    if copied && matches!(m, Rdgsbase | Wrgsbase) {
        return Some("a base of the thread area's segment copied");
    }
    if copied && m == Wrfsbase && (in_process || way == Way::Process && host.five_level_paging) {
        return Some("wrfsbase copied where the host's canonical rule is not the guest's");
    }
    if copied && in_process && m == Rdfsbase {
        return Some("rdfsbase copied in process, where FS holds the thread area");
    }
    if m == Syscall
        && r != if way == Way::Machine {
            Route::Copy
        } else {
            Route::Kernel
        }
    {
        return Some("syscall routed against its way");
    }
    if selector && r != Route::Kernel {
        return Some("a segment register read or written other than by the inside");
    }
    let near = ins.is_jcc_short_or_near()
        || matches!(m, Ret | Loop | Loope | Loopne | Jrcxz | Jecxz)
        || matches!(m, Call | Jmp)
            && !matches!(ins.op_kind(0), OpKind::FarBranch16 | OpKind::FarBranch32);
    if near && !matches!(r, Route::Transfer | Route::Kernel) {
        return Some("a near transfer not carried as one");
    }
    if copied
        && matches!(
            m,
            Cpuid | Rdtsc | Rdtscp | Popf | Popfq | Xsave | Xsave64 | Xrstor | Xrstor64 | Retf
        )
    {
        return Some("a form the inside answers copied");
    }
    None
}

/// Every decoded form of the sweep, in every way and on both hosts. `X86_ROUTE_SEED` and `X86_ROUTE_CASES` set
/// the sweep.
#[test]
fn every_decoded_form_is_routed_by_the_plan() {
    let seed = knob("X86_ROUTE_SEED", 0x5eed);
    let cases = knob("X86_ROUTE_CASES", 200_000);
    let mut rng = fastrand::Rng::with_seed(seed);
    let offer = offer();
    let (mut routed, mut wrong) = ([0u64; 13], Vec::new());
    for _ in 0..cases {
        let code = instruction_like(&mut rng);
        let Some((i, length)) = decode(&code) else {
            continue;
        };
        for way in WAYS {
            for host in &HOSTS {
                let r = route(way, &i, host, &offer);
                routed[r as usize] += 1;
                if let Some(rule) = broken(way, host, &i, r) {
                    wrong.push(format!(
                        "{way:?} {host:?}: {:02x?} {:?}: {rule} ({r:?})",
                        &code[..usize::from(length).min(code.len())],
                        i.iced.code()
                    ));
                }
            }
        }
    }
    println!("seed {seed:#x}, {cases} cases, routes {routed:?}");
    assert!(
        wrong.is_empty(),
        "{} broken, first: {:#?}",
        wrong.len(),
        &wrong[..wrong.len().min(20)]
    );
}

/// The forms the plan and its checks named, each in the ways it differs.
#[test]
fn the_named_forms() {
    let [xeon, other] = &HOSTS;
    let (machine, process, wrapping, exact) = (
        Way::Machine,
        Way::Process,
        Way::InProcess(Check::Wrapping),
        Way::InProcess(Check::Exact),
    );
    let cases: &[(Way, &HostMatches, &str, Route)] = &[
        // A gs: operand with its own address-size prefix (the eighth check's gs67.c).
        (machine, xeon, "65 67 8b 00", Route::Area),
        (process, xeon, "65 67 8b 00", Route::Area),
        (exact, xeon, "65 67 8b 00", Route::Area),
        (machine, xeon, "65 48 8b 04 25 28 00 00 00", Route::Area),
        (machine, xeon, "64 48 8b 04 25 28 00 00 00", Route::Copy),
        (wrapping, xeon, "64 48 8b 04 25 28 00 00 00", Route::Area),
        (machine, xeon, "65 48 8d 00", Route::Copy),
        (machine, xeon, "48 8b 05 00 00 00 00", Route::Rebase),
        (machine, xeon, "48 8d 05 00 00 00 00", Route::Rebase),
        (machine, xeon, "0f 05", Route::Copy),
        (process, xeon, "0f 05", Route::Kernel),
        (exact, xeon, "0f 05", Route::Kernel),
        // wrfsbase, rdfsbase, rdgsbase, wrgsbase.
        (machine, xeon, "f3 48 0f ae d0", Route::Copy),
        (process, xeon, "f3 48 0f ae d0", Route::Copy),
        (process, other, "f3 48 0f ae d0", Route::Kernel),
        (wrapping, xeon, "f3 48 0f ae d0", Route::Kernel),
        (machine, xeon, "f3 48 0f ae c0", Route::Copy),
        (exact, xeon, "f3 48 0f ae c0", Route::Area),
        (machine, xeon, "f3 48 0f ae c8", Route::Area),
        (machine, xeon, "f3 48 0f ae d8", Route::Kernel),
        // maskmovq and maskmovdqu, on and off the thread area's segment.
        (machine, xeon, "0f f7 c1", Route::Copy),
        (machine, xeon, "65 0f f7 c1", Route::Area),
        (machine, xeon, "66 65 0f f7 c1", Route::Area),
        (process, xeon, "64 0f f7 c1", Route::Copy),
        (wrapping, xeon, "65 0f f7 c1", Route::Kernel),
        // A bit test with a register offset and a gather, each on gs: under 0x67 (the check's 0x67 forms).
        (process, xeon, "67 65 48 0f a3 00", Route::Area),
        (process, xeon, "67 65 c4 e2 69 90 04 88", Route::Area),
        // Strings with the guest's 0x67 (the eighth check's str67.c), and on the area's segment.
        (machine, xeon, "67 aa", Route::Copy),
        (machine, xeon, "67 f3 aa", Route::Copy),
        (wrapping, xeon, "67 aa", Route::Kernel),
        (exact, xeon, "67 f3 aa", Route::Kernel),
        (machine, xeon, "65 a4", Route::Area),
        (process, xeon, "65 49 a4", Route::Area),
        (machine, xeon, "65 d7", Route::Area),
        (process, xeon, "65 4c d7", Route::Area),
        (wrapping, xeon, "65 a4", Route::Kernel),
        // stos and scas reach es:rdi, which no override changes.
        (process, xeon, "65 aa", Route::Copy),
        (process, xeon, "65 f3 ae", Route::Copy),
        (machine, xeon, "f3 a4", Route::Copy),
        // Transfers, near and far.
        (machine, xeon, "e8 00 00 00 00", Route::Transfer),
        (machine, xeon, "c3", Route::Transfer),
        (machine, xeon, "ff 10", Route::Transfer),
        (machine, xeon, "cb", Route::Kernel),
        (machine, xeon, "ff 18", Route::Kernel),
        // Selectors, flags, identification, traps, nothing, float status.
        (machine, xeon, "8e e8", Route::Kernel),
        (machine, xeon, "8c e8", Route::Kernel),
        (machine, xeon, "9d", Route::Kernel),
        (machine, xeon, "9c", Route::Copy),
        (machine, xeon, "0f a2", Route::Kernel),
        (machine, xeon, "cc", Route::Trap),
        (machine, xeon, "0f 0b", Route::Trap),
        (machine, xeon, "90", Route::Nothing),
        (machine, xeon, "f3 0f 1e fa", Route::Nothing),
        (machine, xeon, "0f 18 00", Route::Nothing),
        (machine, xeon, "d9 fe", Route::Kernel),
        (machine, xeon, "0f ae 10", Route::FloatStatus),
        // Forms whose host results the probe compares.
        (machine, xeon, "48 0f af c1", Route::Copy),
        (machine, other, "48 0f af c1", Route::Kernel),
        (machine, xeon, "db 38", Route::Copy),
        (machine, other, "db 38", Route::Kernel),
        (machine, other, "c8 08 00 03", Route::Kernel),
        (machine, other, "c8 08 00 00", Route::Copy),
    ];
    let wrong: Vec<_> = cases
        .iter()
        .filter_map(|&(way, host, hex, expected)| {
            let r = route_of(way, host, hex);
            (r != expected).then(|| format!("{way:?} {hex}: {r:?}, not {expected:?}"))
        })
        .collect();
    assert!(wrong.is_empty(), "{wrong:#?}");
}
