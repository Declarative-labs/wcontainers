//! Layer 1: one instruction at a time, on the real CPU and through the translator at look-ahead none, with
//! every register, flag and touched byte compared.

use super::browser::{MEMORY_FAULT, NO_CODE, STACK_FAULT, STOP_KIND_KEY, SYSCALL};
use super::guest::{Case, Outcome, Signal, State, Stop};
use super::runner::{Fault, Machine, Stopped, Syscalls, thread_state_size};
use super::{Cpu, part};
use crate::layout::*;
use crate::reference::Reference;
use wcontainers_abi::{Access, Exit, Permissions};
use wcontainers_guest_cpu::Guest as _;
use wcontainers_guest_cpu::TrapKind;
use wcontainers_translator::LookAhead;

/// Memory size of the translator's machine: everything up to the end of the code region.
fn memory() -> u64 {
    CODE + part::CODE_SPAN
}

/// The code region around the instruction, which holds the guest's filler.
pub(crate) fn code_area() -> std::ops::Range<u64> {
    CODE - part::CODE_SPAN..CODE + part::CODE_SPAN
}

/// The data area, the code region and what each allows.
pub(crate) fn regions() -> [(std::ops::Range<u64>, Permissions); 3] {
    [
        (
            DATA..DATA + DATA_WRITABLE,
            Permissions::READ | Permissions::WRITE,
        ),
        (DATA + DATA_WRITABLE..DATA + DATA_BYTES, Permissions::READ),
        (code_area(), Permissions::READ | Permissions::EXECUTE),
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
fn start_thread(case: &Case) -> Vec<u8> {
    let mut thread = vec![0u8; thread_state_size::<Cpu>()];
    part::from_state(&case.state, &mut thread);
    if case.tracking {
        part::track(&mut thread);
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
fn translated_stop(stopped: Stopped) -> Stop {
    match stopped {
        Stopped::NoCode(address) => Stop::Landed(address),
        // Control reached a page that does not allow running code.
        Stopped::Exit(Exit::MemoryFault {
            address,
            access: Access::Execute,
        }) => Stop::Landed(address),
        Stopped::Exit(Exit::IllegalInstruction) => Stop::Illegal,
        Stopped::Exit(Exit::Breakpoint { comment }) => {
            Stop::Breakpoint(part::breakpoint_code(comment))
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
pub fn translated(case: &Case) -> Outcome {
    translated_with(case, |_| {})
}

/// Run a case through the translator, on a machine `prepare` has changed.
pub fn translated_with(case: &Case, prepare: impl FnOnce(&mut Machine)) -> Outcome {
    translated_on(case, prepare).0
}

/// Run a case through the translator, on a machine `prepare` has changed, and keep the machine.
pub fn translated_on(case: &Case, prepare: impl FnOnce(&mut Machine)) -> (Outcome, Machine) {
    // The table covers the case's one instruction only, so control that lands anywhere else stops there.
    let mut machine = Machine::new(memory(), CODE, 1, Syscalls::Stop);
    machine.set_offer(part::case_offer(case));
    prepare(&mut machine);
    for (range, permissions) in regions() {
        machine.set_permissions(range, permissions);
    }
    // Fresh memory is already zero.
    if part::FILLER != 0 {
        let area = code_area();
        machine.write(
            area.start,
            &vec![part::FILLER; (area.end - area.start) as usize],
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
    machine.set_thread_state(&start_thread(case));
    let (stopped, _) = machine.run();
    let stop = translated_stop(stopped);
    let state = part::to_state(machine.stopped_state());
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
pub fn translated_in_chrome(cases: &[Case]) -> Vec<Outcome> {
    // Each page runs cases told one offer, its cases' outcomes put back in their places.
    let mut outcomes: Vec<Option<Outcome>> = cases.iter().map(|_| None).collect();
    let mut keys: Vec<u64> = cases
        .iter()
        .map(|case| Cpu::offer_key(part::case_offer(case)))
        .collect();
    keys.sort_unstable();
    keys.dedup();
    for key in keys {
        let chosen: Vec<usize> = (0..cases.len())
            .filter(|&n| Cpu::offer_key(part::case_offer(&cases[n])) == key)
            .collect();
        for places in chosen.chunks(CASES_A_PAGE) {
            let page: Vec<Case> = places.iter().map(|&n| cases[n].clone()).collect();
            for (&n, outcome) in places.iter().zip(translated_in_one_page(&page, key)) {
                outcomes[n] = Some(outcome);
            }
        }
    }
    outcomes
        .into_iter()
        .map(|outcome| outcome.expect("every case ran"))
        .collect()
}

fn translated_in_one_page(cases: &[Case], offer: u64) -> Vec<Outcome> {
    let states: Vec<super::browser::StateCase> = cases
        .iter()
        .map(|case| super::browser::StateCase {
            code: case.code,
            state: start_thread(case),
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
    super::browser::cases(
        CODE,
        memory(),
        &regions(),
        code_area(),
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
                Stop::Breakpoint(part::breakpoint_code(detail(stopped) as u16))
            }
            (_, Some(TrapKind::Alignment)) => Stop::Alignment(address(&stopped["detail"])),
            (_, Some(TrapKind::GeneralProtection | TrapKind::Overflow)) => Stop::GeneralProtection,
            (_, Some(TrapKind::DivideError)) => Stop::DivideError,
            (_, Some(TrapKind::FloatException)) => Stop::FloatException(detail(stopped) as u32),
            _ => panic!("no counterpart on the reference: {stopped}"),
        };
        Outcome {
            stop,
            state: part::to_state(&run.state),
            changed: run.changed,
        }
    })
    .collect()
}

/// Run a case on the reference.
pub fn on_reference(reference: &mut Reference, case: &Case) -> Outcome {
    let landed = reference
        .case(case.code.code(), case.seed, &case.placed, &case.state.0)
        .unwrap_or_else(|e| panic!("the reference runner failed: {e}"))
        .landed();
    let signal = Signal {
        number: landed.signal,
        code: landed.code,
        address: landed.address,
        pc: landed.state[part::PC_FIELD] as u64,
    };
    Outcome {
        stop: part::reference_stop(&signal, case.code.length),
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
pub fn compare(reference: &mut Reference, case: &Case) -> Result<Agreement, String> {
    compare_outcome(reference, case, translated(case))
}

/// Compare a translated outcome of a case with the reference's.
pub fn compare_outcome(
    reference: &mut Reference,
    case: &Case,
    actual: Outcome,
) -> Result<Agreement, String> {
    let mut expected = on_reference(reference, case);
    part::reconcile(case, &mut expected, &actual);
    if expected == actual {
        return Ok(Agreement::Same);
    }
    if let Some(reason) = part::known_failure(case, &expected, &actual) {
        return Ok(Agreement::KnownFailure(reason));
    }
    let mut report = format!(
        "code {:02x?} ({})\n  reference: {:?}\n  translated: {:?}\n",
        case.code.code(),
        part::disassemble(case.code.code()),
        expected.stop,
        actual.stop
    );
    for (i, name) in part::REFERENCE.fields.iter().enumerate() {
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
