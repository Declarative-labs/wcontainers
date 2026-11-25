//! Code the native output translates for the process way, run in a stub: a call into the floor saves the hardware
//! as the call left it and resolves to the guest's own state, and the code names the process way's slot table and
//! floor entries, never the virtual machine's mirror or `brk`.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use std::mem::offset_of;

use wcontainers_abi::RegisterBlock;
use wcontainers_arm64::encode::{self, form};
use wcontainers_arm64::mirror::{MIRROR, SLOT_SHIFT};
use wcontainers_arm64::thread_state::{COUNT, PAGE, RegisterPage, SCRATCH, WITHHELD};
use wcontainers_arm64::way::{
    PROCESS_ENTRIES, PROCESS_ENTRY_BYTES, PROCESS_REACH_BITS, PROCESS_SLOTS,
};
use wcontainers_arm64::{Arm64, Instruction, Way, decode};
use wcontainers_executor_arm64_stop::{FloatStatus, Interrupted, Piece, Resolved, Stop, Taken};
use wcontainers_executor_host_process::code::CodeArea;
use wcontainers_executor_host_process::layout::CODE_AREA;
use wcontainers_executor_host_process::pages::Command;
use wcontainers_executor_host_process::probe::HostProbe;
use wcontainers_executor_host_process::stub::TurnEnd;
use wcontainers_executor_host_process::zygote::Zygote;
use wcontainers_guest_cpu::HostCode;
use wcontainers_guest_cpu::position::{Held, InsideCall, span_at};
use wcontainers_translator::{LookAhead, translate_native_for};

/// Where the guest code lies.
const GUEST: u64 = 0x40_0000;
/// The link register.
const LINK: u8 = 30;
/// The bytes of one instruction.
const INSTRUCTION: u64 = size_of::<u32>() as u64;
/// The guest's count before its code runs.
const COUNT_BEFORE: u64 = 100;

/// The guest words `words` translated for `way` at [`GUEST`], with exact replay.
fn translated(words: &[u32], way: Way) -> HostCode {
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    translate_native_for::<Arm64>(&bytes, GUEST, LookAhead::Full, true, &way, &())
        .expect("translated")
}

/// The host words of a translation.
fn host_words(code: &HostCode) -> Vec<u32> {
    let (words, rest) = code.bytes.as_chunks::<{ size_of::<u32>() }>();
    assert!(rest.is_empty(), "host code is whole words");
    words.iter().map(|w| u32::from_le_bytes(*w)).collect()
}

/// Whether `inner` appears in `words` as a run.
fn contains(words: &[u32], inner: &[u32]) -> bool {
    words.windows(inner.len()).any(|w| w == inner)
}

/// Where the guest's x30 waits during a call.
fn link_offset() -> u64 {
    offset_of!(RegisterPage, link) as u64
}

/// The words a process-way call of `call` ends with: x30 kept, the entry's address built, the call.
fn call_words(call: InsideCall) -> Vec<u32> {
    let keep = encode::load_store(
        form::STR,
        LINK,
        PAGE,
        link_offset(),
        size_of::<u64>() as u64,
    )
    .expect("a store to the page");
    let entry = PROCESS_ENTRIES + u64::from(call.code()) * PROCESS_ENTRY_BYTES;
    let mut words = vec![keep];
    words.extend(encode::move_address(LINK, entry));
    words.push(encode::branch_register(form::BLR, LINK));
    words
}

#[test]
fn the_process_way_names_its_slot_table_and_floor_entries() {
    let target = GUEST + 0x100;
    let branch = encode::branch(false, (target - GUEST) as i64).expect("a branch");
    let jump = encode::branch_register(form::BR, 1);
    let call = encode::exception(form::SVC, 0);
    let undefined = encode::exception(form::UDF, 0);
    let far = 1u64 << PROCESS_REACH_BITS;
    for words in [vec![branch], vec![jump], vec![call], vec![undefined]] {
        let process = host_words(&translated(&words, Way::Process));
        let machine = host_words(&translated(&words, Way::Machine));
        let brk = |w: &u32| matches!(decode(*w), Instruction::Breakpoint { .. });
        assert!(
            !process.iter().any(brk),
            "{words:x?}: no brk in the process way"
        );
        assert!(
            machine.iter().any(brk) || words == [branch],
            "{words:x?}: the machine way's calls are brk"
        );
    }
    // A direct exit builds its target's slot in the process way's table.
    let exit = host_words(&translated(&[branch], Way::Process));
    let slot = encode::move_address(SCRATCH, PROCESS_SLOTS | target << SLOT_SHIFT);
    assert!(contains(&exit, &slot), "the exit's slot");
    let mirror = encode::move_address(SCRATCH, MIRROR | target << SLOT_SHIFT);
    assert!(contains(
        &host_words(&translated(&[branch], Way::Machine)),
        &mirror
    ));
    // A jump through a register tests the process way's reach and goes through its table.
    let jumped = host_words(&translated(&[jump], Way::Process));
    let reach = encode::ubfx(
        SCRATCH,
        1,
        PROCESS_REACH_BITS,
        u64::BITS - u8::BITS - PROCESS_REACH_BITS,
    );
    let into_table = encode::eor_immediate(SCRATCH, SCRATCH, PROCESS_SLOTS).expect("one bit");
    assert!(
        jumped.contains(&reach) && jumped.contains(&into_table),
        "the jump's reach and table"
    );
    // A target past the reach leaves through the floor.
    let far_code = translate_native_for::<Arm64>(
        &branch.to_le_bytes(),
        far - 0x100,
        LookAhead::Full,
        true,
        &Way::Process,
        &(),
    )
    .expect("translated");
    assert!(
        contains(&host_words(&far_code), &call_words(InsideCall::Exit)),
        "an exit past the reach calls the floor"
    );
    // A call keeps x30 in the page, where the position map finds it from the store on.
    let called = translated(&[call], Way::Process);
    let words = host_words(&called);
    let expected = call_words(InsideCall::Answer);
    let at = words
        .windows(expected.len())
        .position(|w| w == expected.as_slice())
        .expect("the system call's call into the floor");
    let blr = (at + expected.len() - 1) as u32 * INSTRUCTION as u32;
    let span = span_at(&called.spans, called.bytes.len(), blr).expect("a span at the call");
    assert!(
        span.relocated
            .iter()
            .any(|r| r.guest == LINK && r.held == Held::Page(link_offset() as u16)),
        "x30 is the page's at the call: {span:?}"
    );
    assert_eq!(
        words[at + expected.len()],
        encode::load_store(
            form::LDR,
            LINK,
            PAGE,
            link_offset(),
            size_of::<u64>() as u64
        )
        .expect("a load"),
        "x30 is taken back after the call"
    );
}

/// The guest's x30 and the withheld registers' guest values, told apart from everything the hardware holds.
const GUEST_LINK: u64 = 0x1234;
const GUEST_WITHHELD: [u64; WITHHELD.len()] = [0xaa18, 0xaa23, 0xaa28];

#[test]
fn a_system_call_in_translated_code_reaches_its_floor_entry_and_resolves_to_the_guest() {
    let guest = [
        encode::movz(0, 7, 0),
        encode::movz(LINK, GUEST_LINK as u16, 0),
        encode::exception(form::SVC, 0),
    ];
    let code = translated(&guest, Way::Process);
    let probe = HostProbe::new().expect("the host is refused");
    let zygote = Zygote::start(&probe).expect("zygote");
    let stub = zygote.fork_stub().expect("fork");
    let mut area = CodeArea::new().expect("code area");
    area.write(&probe, CODE_AREA, &host_words(&code));
    stub.send_block(area.file()).expect("send the code area");
    assert_eq!(
        stub.turn(0, &[Command::MapCode]).expect("turn"),
        TurnEnd::Done {
            commands_applied: 1
        }
    );
    let mut loaded = RegisterBlock::ZERO;
    for (n, x) in loaded.x.iter_mut().enumerate() {
        *x = 0x1000 + n as u64;
    }
    loaded.x[usize::from(COUNT)] = COUNT_BEFORE;
    stub.set_registers(0, &loaded);
    let page = RegisterPage {
        withheld: GUEST_WITHHELD,
        ..RegisterPage::default()
    };
    stub.set_translated_page(0, &page);
    assert_eq!(
        stub.enter(0, &[], CODE_AREA).expect("enter"),
        TurnEnd::Called {
            entry: u64::from(InsideCall::Answer.code())
        }
    );
    let block = stub.registers(0);
    let page = stub.translated_page(0);
    assert_eq!(page.link, GUEST_LINK, "the guest's x30 is kept in the page");
    assert_eq!(
        block.x[usize::from(SCRATCH)],
        CODE_AREA as u64,
        "x23 as the entry found it"
    );
    let at_call = block.pc - INSTRUCTION;
    assert_eq!(
        host_words(&code)[((at_call - CODE_AREA as u64) / INSTRUCTION) as usize],
        encode::branch_register(form::BLR, LINK),
        "the call is reported at its blr"
    );
    let h = Interrupted {
        x: block.x,
        sp: block.sp,
        pc: at_call,
        state: block.pstate,
        fpsr: block.fpsr,
        fpcr: block.fpcr,
    };
    let piece = Piece {
        base: CODE_AREA as u64,
        length: code.bytes.len(),
        spans: &code.spans,
    };
    let mut float = FloatStatus::default();
    let resolved = wcontainers_executor_arm64_stop::resolve(
        Way::Process,
        &piece,
        Taken::Call(InsideCall::Answer.code()),
        &h,
        &page,
        &mut float,
        &mut |_, _| panic!("a system call reads no guest memory"),
    );
    let Some(Resolved::Stop(Stop::Syscall, state)) = resolved else {
        panic!("not a system call: {resolved:?}");
    };
    assert_eq!(state.pc, GUEST + 3 * INSTRUCTION, "after the svc");
    assert_eq!(state.x[0], 7);
    assert_eq!(state.x[usize::from(LINK)], GUEST_LINK);
    for (k, &r) in WITHHELD.iter().enumerate() {
        assert_eq!(state.x[usize::from(r)], GUEST_WITHHELD[k], "withheld x{r}");
    }
    for n in (1..usize::from(LINK)).filter(|&n| !WITHHELD.contains(&(n as u8))) {
        assert_eq!(state.x[n], loaded.x[n], "x{n} untouched");
    }
    // The count is exact against the code's own adds; against the WebAssembly output it waits on K2 (plan 3.1).
    assert!(
        (COUNT_BEFORE..=COUNT_BEFORE + guest.len() as u64).contains(&state.count),
        "count {}",
        state.count
    );
    stub.end(0).expect("end");
}
