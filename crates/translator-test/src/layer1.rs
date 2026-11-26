//! Layer 1: one instruction at a time, on the real CPU and through the translator at look-ahead none, with
//! every register, flag and touched byte compared.

use crate::browser::{MEMORY_FAULT, NO_CODE, STACK_FAULT, STOP_KIND_KEY, SYSCALL};
use crate::guest::{Case, GuestTest, Outcome, Signal, State, Stop};
use crate::layout::*;
use crate::reference::Reference;
use crate::runner::{Fault, Machine, Stopped, Syscalls, thread_state_size};
use wcontainers_abi::{Access, Exit, Permissions};
use wcontainers_guest_cpu::TrapKind;
use wcontainers_translator::LookAhead;

/// Memory size of the translator's machine: everything up to the end of the code region.
fn memory<G: GuestTest>() -> u64 {
    CODE + G::CODE_SPAN
}

/// The code region around the instruction, which holds the guest's filler.
pub(crate) fn code_area<G: GuestTest>() -> std::ops::Range<u64> {
    CODE - G::CODE_SPAN..CODE + G::CODE_SPAN
}

/// The data area, the code region and what each allows.
pub(crate) fn regions<G: GuestTest>() -> [(std::ops::Range<u64>, Permissions); 3] {
    [
        (
            DATA..DATA + DATA_WRITABLE,
            Permissions::READ | Permissions::WRITE,
        ),
        (DATA + DATA_WRITABLE..DATA + DATA_BYTES, Permissions::READ),
        (code_area::<G>(), Permissions::READ | Permissions::EXECUTE),
    ]
}

/// The data area's contents at the start of a case.
pub(crate) fn start_data(case: &Case) -> Vec<u8> {
    let mut data = vec![0u8; DATA_BYTES as usize];
    fill(case.seed, &mut data);
    crate::layout::place(&case.placed, &mut data);
    data
}

/// The thread state at the start of a case.
fn start_thread<G: GuestTest>(case: &Case) -> Vec<u8> {
    let mut thread = vec![0u8; thread_state_size::<G>()];
    G::from_state(&case.state, &mut thread);
    if case.tracking {
        G::track(&mut thread);
    }
    thread
}

/// The data bytes that differ from the case's start.
pub(crate) fn changed(case: &Case, now: &[u8]) -> Vec<(u32, u8)> {
    now.iter()
        .zip(&start_data(case))
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, (a, _))| (i as u32, *a))
        .collect()
}

/// How the translated side stopped, in the reference's terms.
fn translated_stop<G: GuestTest>(stopped: Stopped) -> Stop {
    match stopped {
        Stopped::NoCode(address) => Stop::Landed(address),
        // Control reached a page that does not allow running code.
        Stopped::Exit(Exit::MemoryFault {
            address,
            access: Access::Execute,
        }) => Stop::Landed(address),
        Stopped::Exit(Exit::IllegalInstruction) => Stop::Illegal,
        Stopped::Exit(Exit::Breakpoint { comment }) => {
            Stop::Breakpoint(G::breakpoint_code(comment))
        }
        Stopped::Exit(Exit::Syscall(_)) => Stop::Syscall,
        Stopped::Exit(Exit::MemoryFault { address, .. }) => Stop::MemoryFault(address),
        Stopped::Exit(Exit::AlignmentFault { address }) => Stop::Alignment(address),
        Stopped::Fault(Fault::GeneralProtection | Fault::Overflow) => Stop::GeneralProtection,
        Stopped::Fault(Fault::Stack) => Stop::StackFault,
        Stopped::Fault(Fault::DivideError) => Stop::DivideError,
        Stopped::Fault(Fault::FloatException(code)) => Stop::FloatException(code as u32),
        other => panic!("no counterpart on the reference: {other:?}"),
    }
}

/// Run a case through the translator.
pub fn translated<G: GuestTest>(case: &Case) -> Outcome {
    translated_with::<G>(case, |_| {})
}

/// Run a case through the translator, on a machine `prepare` has changed.
pub fn translated_with<G: GuestTest>(
    case: &Case,
    prepare: impl FnOnce(&mut Machine<G>),
) -> Outcome {
    translated_on(case, prepare).0
}

/// Run a case through the translator, on a machine `prepare` has changed, and keep the machine.
pub fn translated_on<G: GuestTest>(
    case: &Case,
    prepare: impl FnOnce(&mut Machine<G>),
) -> (Outcome, Machine<G>) {
    // The table covers the case's one instruction only, so control that lands anywhere else stops there.
    let mut machine = Machine::<G>::new(memory::<G>(), CODE, 1, Syscalls::Stop);
    machine.set_offer(G::case_offer(case));
    prepare(&mut machine);
    for (range, permissions) in regions::<G>() {
        machine.set_permissions(range, permissions);
    }
    // Fresh memory is already zero.
    if G::FILLER != 0 {
        let area = code_area::<G>();
        machine.write(
            area.start,
            &vec![G::FILLER; (area.end - area.start) as usize],
        );
    }
    machine.write(DATA, &start_data(case));
    let code = case.code.code();
    machine.load(
        code,
        0..code.len() as u64,
        (CODE, None),
        &[0],
        LookAhead::None,
    );
    machine.set_thread_state(&start_thread::<G>(case));
    let (stopped, _) = machine.run();
    let stop = translated_stop::<G>(stopped);
    let state = G::to_state(machine.stopped_state());
    let mut now = vec![0u8; DATA_BYTES as usize];
    machine.read(DATA, &mut now);
    let outcome = Outcome {
        stop,
        state,
        changed: changed(case, &now),
    };
    (outcome, machine)
}

/// Cases one Chrome page runs; a page given several thousand ran out of memory.
const CASES_A_PAGE: usize = 1000;

/// Run cases through the translator in Chrome, [`CASES_A_PAGE`] to a page.
pub fn translated_in_chrome<G: GuestTest>(cases: &[Case]) -> Vec<Outcome> {
    // Each page runs cases told one offer, its cases' outcomes put back in their places.
    let mut outcomes: Vec<Option<Outcome>> = cases.iter().map(|_| None).collect();
    let mut keys: Vec<u64> = cases
        .iter()
        .map(|case| G::offer_key(G::case_offer(case)))
        .collect();
    keys.sort_unstable();
    keys.dedup();
    for key in keys {
        let chosen: Vec<usize> = (0..cases.len())
            .filter(|&n| G::offer_key(G::case_offer(&cases[n])) == key)
            .collect();
        for places in chosen.chunks(CASES_A_PAGE) {
            let page: Vec<Case> = places.iter().map(|&n| cases[n].clone()).collect();
            for (&n, outcome) in places.iter().zip(translated_in_one_page::<G>(&page, key)) {
                outcomes[n] = Some(outcome);
            }
        }
    }
    outcomes
        .into_iter()
        .map(|outcome| outcome.expect("every case ran"))
        .collect()
}

fn translated_in_one_page<G: GuestTest>(cases: &[Case], offer: u64) -> Vec<Outcome> {
    let states: Vec<crate::browser::StateCase> = cases
        .iter()
        .map(|case| crate::browser::StateCase {
            code: case.code,
            state: start_thread::<G>(case),
            places: vec![start_data(case)],
        })
        .collect();
    let address = |value: &serde_json::Value| -> u64 {
        value
            .as_str()
            .expect("an address")
            .parse()
            .expect("a number")
    };
    let detail = |stopped: &serde_json::Value| -> u64 {
        stopped["detail"]
            .as_str()
            .map_or(0, |d| d.parse().expect("a number"))
    };
    crate::browser::cases::<G>(
        CODE,
        memory::<G>(),
        &regions::<G>(),
        code_area::<G>(),
        std::slice::from_ref(&(DATA..DATA + DATA_BYTES)),
        &states,
        Some(offer),
    )
    .into_iter()
    .map(|run| {
        let stopped = &run.stopped;
        let kind = stopped[STOP_KIND_KEY].as_str().unwrap_or_default();
        let stop = match (kind, TrapKind::from_name(kind)) {
            (NO_CODE, _) => Stop::Landed(address(&stopped["address"])),
            (SYSCALL, _) => Stop::Syscall,
            (MEMORY_FAULT, _) if stopped["execute"] == true => {
                Stop::Landed(address(&stopped["address"]))
            }
            (MEMORY_FAULT, _) => Stop::MemoryFault(address(&stopped["address"])),
            (STACK_FAULT, _) => Stop::StackFault,
            (_, Some(TrapKind::Undefined | TrapKind::Unsupported)) => Stop::Illegal,
            (_, Some(TrapKind::Breakpoint)) => {
                Stop::Breakpoint(G::breakpoint_code(detail(stopped) as u16))
            }
            (_, Some(TrapKind::Alignment)) => Stop::Alignment(address(&stopped["detail"])),
            (_, Some(TrapKind::GeneralProtection | TrapKind::Overflow)) => Stop::GeneralProtection,
            (_, Some(TrapKind::DivideError)) => Stop::DivideError,
            (_, Some(TrapKind::FloatException)) => Stop::FloatException(detail(stopped) as u32),
            _ => panic!("no counterpart on the reference: {stopped}"),
        };
        Outcome {
            stop,
            state: G::to_state(&run.state),
            changed: run.changed,
        }
    })
    .collect()
}

/// Run a case on the reference.
pub fn on_reference<G: GuestTest>(reference: &mut Reference, case: &Case) -> Outcome {
    let landed = reference
        .case(case.code.code(), case.seed, &case.placed, &case.state.0)
        .unwrap_or_else(|e| panic!("the reference runner failed: {e}"))
        .landed();
    let signal = Signal {
        number: landed.signal,
        code: landed.code,
        address: landed.address,
        pc: landed.state[G::PC_FIELD] as u64,
    };
    Outcome {
        stop: G::reference_stop(&signal, case.code.length),
        state: State(landed.state),
        changed: landed.changed,
    }
}

/// How the two sides of a case agreed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Agreement {
    Same,
    /// They differ where the reference is known to fail, for this reason.
    KnownFailure(&'static str),
}

/// Compare one case on both sides; describe the difference if there is one.
pub fn compare<G: GuestTest>(reference: &mut Reference, case: &Case) -> Result<Agreement, String> {
    compare_outcome::<G>(reference, case, translated::<G>(case))
}

/// Compare a translated outcome of a case with the reference's.
pub fn compare_outcome<G: GuestTest>(
    reference: &mut Reference,
    case: &Case,
    actual: Outcome,
) -> Result<Agreement, String> {
    let mut expected = on_reference::<G>(reference, case);
    G::reconcile(case, &mut expected, &actual);
    if expected == actual {
        return Ok(Agreement::Same);
    }
    if let Some(reason) = G::known_failure(case, &expected, &actual) {
        return Ok(Agreement::KnownFailure(reason));
    }
    let mut report = format!(
        "code {:02x?} ({})\n  reference: {:?}\n  translated: {:?}\n",
        case.code.code(),
        G::disassemble(case.code.code()),
        expected.stop,
        actual.stop
    );
    for (i, name) in G::REFERENCE.fields.iter().enumerate() {
        let (reference, translated) = (expected.state.0[i], actual.state.0[i]);
        if reference != translated {
            report += &format!(
                "  {name}: reference {reference:#x}, translated {translated:#x}, before {:#x}\n",
                case.state.0[i]
            );
        }
    }
    if case.tracking {
        report += "  (float flags kept)\n";
    }
    if expected.changed != actual.changed {
        report += &format!(
            "  data: reference {:?}, translated {:?}\n",
            expected.changed, actual.changed
        );
    }
    Err(report)
}
