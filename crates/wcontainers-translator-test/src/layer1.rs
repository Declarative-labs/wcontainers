//! Layer 1: one instruction at a time, on the real CPU and through the translator at look-ahead none, with
//! every register, flag and touched byte compared.

use crate::guest::{Case, GuestTest, Outcome, Signal, State, Stop};
use crate::layout::*;
use crate::reference::Reference;
use crate::runner::{Fault, Machine, Stopped, Syscalls, thread_state_size};
use wcontainers_abi::{Access, Exit, Permissions};
use wcontainers_translator::LookAhead;

/// Memory size of the translator's machine: everything up to the end of the code region.
fn memory<G: GuestTest>() -> u64 {
    CODE + G::CODE_SPAN
}

/// The data area, the code region and what each allows.
fn regions<G: GuestTest>() -> [(std::ops::Range<u64>, Permissions); 3] {
    [
        (
            DATA..DATA + DATA_WRITABLE,
            Permissions::READ | Permissions::WRITE,
        ),
        (DATA + DATA_WRITABLE..DATA + DATA_BYTES, Permissions::READ),
        (
            CODE - G::CODE_SPAN..CODE + G::CODE_SPAN,
            Permissions::READ | Permissions::EXECUTE,
        ),
    ]
}

/// The data area's contents at the start of a case.
fn start_data(case: &Case) -> Vec<u8> {
    let mut data = vec![0u8; DATA_BYTES as usize];
    fill(case.seed, &mut data);
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
fn changed(case: &Case, now: &[u8]) -> Vec<(u32, u8)> {
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
    // The table covers the case's one instruction only, so control that lands anywhere else stops there.
    let mut machine = Machine::<G>::new(memory::<G>(), CODE, 1, Syscalls::Stop);
    for (range, permissions) in regions::<G>() {
        machine.set_permissions(range, permissions);
    }
    // Fresh memory is already zero.
    if G::FILLER != 0 {
        let span = 2 * G::CODE_SPAN as usize;
        machine.write(CODE - G::CODE_SPAN, &vec![G::FILLER; span]);
    }
    machine.write(DATA, &start_data(case));
    let code = case.code.code();
    machine.load(code, 0..code.len() as u64, CODE, &[0], LookAhead::None);
    machine.set_thread_state(&start_thread::<G>(case));
    let (stopped, _) = machine.run();
    let stop = translated_stop::<G>(stopped);
    let state = G::to_state(machine.stopped_state());
    let mut now = vec![0u8; DATA_BYTES as usize];
    machine.read(DATA, &mut now);
    Outcome {
        stop,
        state,
        changed: changed(case, &now),
    }
}

/// Cases one Chrome page runs; a page given several thousand ran out of memory.
const CASES_A_PAGE: usize = 1000;

/// Run cases through the translator in Chrome, [`CASES_A_PAGE`] to a page.
pub fn translated_in_chrome<G: GuestTest>(cases: &[Case]) -> Vec<Outcome> {
    cases
        .chunks(CASES_A_PAGE)
        .flat_map(translated_in_one_page::<G>)
        .collect()
}

fn translated_in_one_page<G: GuestTest>(cases: &[Case]) -> Vec<Outcome> {
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
        std::slice::from_ref(&(DATA..DATA + DATA_BYTES)),
        &states,
    )
    .into_iter()
    .map(|run| {
        let stopped = &run.stopped;
        let stop = match stopped["kind"].as_str() {
            Some("no code") => Stop::Landed(address(&stopped["address"])),
            Some("undefined" | "unsupported") => Stop::Illegal,
            Some("breakpoint") => Stop::Breakpoint(G::breakpoint_code(detail(stopped) as u16)),
            Some("syscall") => Stop::Syscall,
            Some("memory fault") if stopped["execute"] == true => {
                Stop::Landed(address(&stopped["address"]))
            }
            Some("memory fault") => Stop::MemoryFault(address(&stopped["address"])),
            Some("alignment") => Stop::Alignment(address(&stopped["detail"])),
            Some("general protection" | "overflow") => Stop::GeneralProtection,
            Some("stack fault") => Stop::StackFault,
            Some("divide error") => Stop::DivideError,
            Some("float exception") => Stop::FloatException(detail(stopped) as u32),
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
        .case(case.code.code(), case.seed, &case.state.0)
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

/// Compare one case on both sides; describe the difference if there is one.
pub fn compare<G: GuestTest>(reference: &mut Reference, case: &Case) -> Result<(), String> {
    compare_outcome::<G>(reference, case, translated::<G>(case))
}

/// Compare a translated outcome of a case with the reference's.
pub fn compare_outcome<G: GuestTest>(
    reference: &mut Reference,
    case: &Case,
    actual: Outcome,
) -> Result<(), String> {
    let mut expected = on_reference::<G>(reference, case);
    G::reconcile(case, &mut expected, &actual);
    if expected == actual {
        return Ok(());
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
