//! A stub's touches on a real host, standing in for guest code: they reach the guest's memory at its own addresses,
//! and a touch outside its mappings, of a page it may not write, or of the embedder's memory, is the turn's fault
//! exit and nothing else.
#![cfg(all(
    target_os = "linux",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]

use rustix::mm::ProtFlags;
use wcontainers_executor_host_process::layout::{
    GUEST_START, OUR_AREA, REGISTER_PAGES, SIGNAL_STACKS, THREAD_PAGES, ZYGOTE,
};
use wcontainers_executor_host_process::pages::Command;
use wcontainers_executor_host_process::probe::HostProbe;
use wcontainers_executor_host_process::stub::{Stub, TurnEnd};
use wcontainers_executor_host_process::zygote::{Ended, Zygote};

/// One embedder process has one zygote place, so tests running at once start theirs one at a time.
static STARTING: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn zygote() -> Zygote {
    let probe = HostProbe::new().expect("the host is refused");
    let _one_at_a_time = STARTING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    Zygote::start(&probe).expect("the zygote did not start")
}

const PAGE: usize = 4096;
const SIGSEGV: u32 = linux_raw_sys::general::SIGSEGV;
const MAPPED: u32 = linux_raw_sys::general::SEGV_MAPERR;
const REFUSED: u32 = linux_raw_sys::general::SEGV_ACCERR;

fn map(address: usize, length: usize, permissions: ProtFlags) -> Command {
    Command::Map {
        address,
        length,
        permissions,
        offset: address,
    }
}

fn fault(address: usize, code: u32) -> TurnEnd {
    TurnEnd::Fault {
        address: address as u64,
        signal: SIGSEGV,
        code,
    }
}

fn ended(zygote: &Zygote, stub: &Stub) -> Ended {
    stub.end(0).expect("end");
    zygote.reap(stub.process()).expect("reap")
}

#[test]
fn touches_reach_the_guest_s_memory_at_its_own_addresses() {
    let zygote = zygote();
    let stub = zygote.fork_stub().expect("fork");
    let top = OUR_AREA - PAGE;
    let read_write = ProtFlags::READ | ProtFlags::WRITE;
    assert_eq!(
        stub.turn(
            0,
            &[
                map(GUEST_START, PAGE, read_write),
                map(top, PAGE, read_write)
            ]
        ),
        Ok(TurnEnd::Done {
            commands_applied: 2
        })
    );
    for (index, address) in [GUEST_START, GUEST_START + PAGE - 8, top, top + PAGE - 8]
        .into_iter()
        .enumerate()
    {
        let value = 0x0123_4567_89ab_cdef_u64.rotate_left(index as u32 * 8);
        assert_eq!(
            stub.store(0, address, value),
            Ok(TurnEnd::Done {
                commands_applied: 0
            })
        );
        assert_eq!(stub.load(0, address), Ok(TurnEnd::Loaded { value }));
        let mut bytes = [0; 8];
        rustix::io::pread(stub.memory(), &mut bytes, address as u64).expect("read the memfd");
        assert_eq!(
            u64::from_ne_bytes(bytes),
            value,
            "the memfd holds the guest's write"
        );
    }
    assert_eq!(ended(&zygote, &stub), Ended::Exited(0));
}

#[test]
fn a_touch_outside_the_mappings_or_against_their_permissions_is_a_fault_exit() {
    let zygote = zygote();
    let stub = zygote.fork_stub().expect("fork");
    let read_only = GUEST_START + 2 * PAGE;
    assert_eq!(
        stub.turn(0, &[map(read_only, PAGE, ProtFlags::READ)]),
        Ok(TurnEnd::Done {
            commands_applied: 1
        })
    );
    for _ in 0..100 {
        assert_eq!(
            stub.store(0, GUEST_START, 1),
            Ok(fault(GUEST_START, MAPPED))
        );
        assert_eq!(
            stub.load(0, GUEST_START + PAGE),
            Ok(fault(GUEST_START + PAGE, MAPPED))
        );
        assert_eq!(stub.store(0, read_only, 1), Ok(fault(read_only, REFUSED)));
        assert_eq!(stub.load(0, read_only), Ok(TurnEnd::Loaded { value: 0 }));
    }
    // The stub lives on and answers.
    assert_eq!(
        stub.turn(0, &[]),
        Ok(TurnEnd::Done {
            commands_applied: 0
        })
    );
    assert_eq!(ended(&zygote, &stub), Ended::Exited(0));
}

/// What a compromised guest could try with its own accesses: the embedder's memory is not in its process at all, and
/// the stub's own pages refuse every write but the register page's, which the plan names (section 12).
#[test]
fn a_guest_cannot_reach_the_embedder_or_write_the_stub_s_own_pages() {
    let zygote = zygote();
    let stub = zygote.fork_stub().expect("fork");
    let secret = Box::new(0x5ec2_e7a1_u64);
    let secret_at = &raw const *secret as usize;
    let on_our_stack = 0u64;
    let stack_at = &raw const on_our_stack as usize;
    let code_at =
        a_guest_cannot_reach_the_embedder_or_write_the_stub_s_own_pages as *const () as usize;
    for address in [secret_at, stack_at, code_at] {
        assert_eq!(
            stub.load(0, address),
            Ok(fault(address, MAPPED)),
            "{address:#x}"
        );
        assert_eq!(
            stub.store(0, address, 0),
            Ok(fault(address, MAPPED)),
            "{address:#x}"
        );
    }
    assert_eq!(*secret, 0x5ec2_e7a1);
    // Its control page, the zygote's code it runs: readable, never writable.
    for address in [THREAD_PAGES, ZYGOTE] {
        assert!(matches!(stub.load(0, address), Ok(TurnEnd::Loaded { .. })));
        assert_eq!(
            stub.store(0, address, 0),
            Ok(fault(address, REFUSED)),
            "{address:#x}"
        );
    }
    // A page of another thread's register pages and a signal stack are its own process's writable memory.
    assert_eq!(
        stub.store(0, REGISTER_PAGES + 7 * PAGE, 5),
        Ok(TurnEnd::Done {
            commands_applied: 0
        })
    );
    assert_eq!(
        stub.store(0, SIGNAL_STACKS + 3 * PAGE, 5),
        Ok(TurnEnd::Done {
            commands_applied: 0
        })
    );
    assert_eq!(ended(&zygote, &stub), Ended::Exited(0));
}
