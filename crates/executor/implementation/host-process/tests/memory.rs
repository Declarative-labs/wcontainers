//! Form P's memories on a real host: the kernel's reads, writes and permissions, and copy-on-write after forks, seen
//! through accesses that stand in for guest code.
#![cfg(all(
    target_os = "linux",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]

use std::collections::BTreeMap;
use std::sync::Arc;

use wcontainers_abi::{Access, Cpu, CpuError, MemoryId, Permissions, RegisterBlock, ThreadId};
use wcontainers_executor_host_process::executor::{
    COPY_UNIT, FaultOwner, HostProcessExecutor, Touch, Touched,
};
use wcontainers_executor_host_process::host::HostProcessHost;
use wcontainers_executor_host_process::layout::OUR_AREA;
use wcontainers_executor_host_process::pages::control::MAX_COMMANDS;

/// One host for the whole test process, as an embedder has, so the runners of tests running at once spread over CPUs.
static HOST: std::sync::OnceLock<Arc<HostProcessHost>> = std::sync::OnceLock::new();

fn executor() -> HostProcessExecutor {
    HostProcessExecutor::new(
        HOST.get_or_init(|| Arc::new(HostProcessHost::new().expect("the host is refused")))
            .clone(),
    )
}

const PAGE: u64 = 4096;
const BASE: u64 = 0x10_0000;
const READ_WRITE: Permissions = Permissions::READ.union(Permissions::WRITE);

fn page(index: u64) -> u64 {
    BASE + index * PAGE
}

/// The first page of the copy unit at `index` from `BASE`: a fault copies a whole unit of lent pages at once, so the
/// copy-counting tests write one page per unit.
fn unit(index: u64) -> u64 {
    BASE + index * COPY_UNIT
}

fn word(cpu: &mut HostProcessExecutor, memory: MemoryId, address: u64) -> u64 {
    let mut bytes = [0; 8];
    cpu.read(memory, address, &mut bytes).expect("read");
    u64::from_le_bytes(bytes)
}

fn load(cpu: &mut HostProcessExecutor, thread: ThreadId, address: u64) -> Touched {
    cpu.touch(thread, Touch::Load(address), None).expect("load")
}

fn store(cpu: &mut HostProcessExecutor, thread: ThreadId, address: u64, value: u64) -> Touched {
    cpu.touch(thread, Touch::Store(address, value), None)
        .expect("store")
}

/// The host mappings a process holds, as the host lists them (x86's vsyscall page is not one).
fn host_mappings(process: rustix::process::Pid) -> usize {
    std::fs::read_to_string(format!("/proc/{}/maps", process.as_raw_nonzero()))
        .expect("maps")
        .lines()
        .filter(|line| !line.ends_with("[vsyscall]"))
        .count()
}

fn fault(address: u64, access: Access) -> Touched {
    Touched::Fault { address, access }
}

/// A memory with one thread, read-write over `units` copy units from `BASE`.
fn process_units(cpu: &mut HostProcessExecutor, units: u64) -> (MemoryId, ThreadId) {
    process(cpu, units * COPY_UNIT / PAGE)
}

/// A memory with one thread, read-write over `pages` pages from `BASE`.
fn process(cpu: &mut HostProcessExecutor, pages: u64) -> (MemoryId, ThreadId) {
    let memory = cpu.memory_new().expect("memory");
    let thread = cpu.thread_new(memory).expect("thread");
    cpu.set_permissions(memory, BASE, pages * PAGE, READ_WRITE)
        .expect("permissions");
    (memory, thread)
}

fn fork(cpu: &mut HostProcessExecutor, parent: MemoryId) -> (MemoryId, ThreadId) {
    let child = cpu.memory_duplicate(parent).expect("duplicate");
    let thread = cpu.thread_new(child).expect("thread");
    (child, thread)
}

fn end(cpu: &mut HostProcessExecutor, (memory, thread): (MemoryId, ThreadId)) {
    let process = cpu.stub_process(memory);
    cpu.thread_drop(thread);
    cpu.memory_drop(memory);
    assert_eq!(
        rustix::process::test_kill_process(process),
        Err(rustix::io::Errno::SRCH),
        "the stub is ended and reaped"
    );
}

#[test]
fn the_kernel_s_reads_writes_and_permissions_agree_with_the_guest_s_accesses() {
    let mut cpu = executor();
    let memory = cpu.memory_new().expect("memory");
    let thread = cpu.thread_new(memory).expect("thread");
    // A fresh memory allows nothing.
    assert_eq!(load(&mut cpu, thread, BASE), fault(BASE, Access::Read));
    cpu.set_permissions(memory, BASE, 4 * PAGE, READ_WRITE)
        .expect("permissions");
    let across = page(1) - 8;
    let bytes: Vec<u8> = (1..=16).collect();
    cpu.write(memory, across, &bytes).expect("write");
    assert_eq!(
        load(&mut cpu, thread, across),
        Touched::Loaded(u64::from_le_bytes(bytes[..8].try_into().unwrap()))
    );
    assert_eq!(
        load(&mut cpu, thread, page(1)),
        Touched::Loaded(u64::from_le_bytes(bytes[8..].try_into().unwrap()))
    );
    assert_eq!(store(&mut cpu, thread, page(2), 0xfeed), Touched::Stored);
    assert_eq!(word(&mut cpu, memory, page(2)), 0xfeed);
    assert_eq!(word(&mut cpu, memory, page(3)), 0, "never written");
    cpu.zero(memory, BASE, PAGE).expect("zero");
    assert_eq!(load(&mut cpu, thread, across), Touched::Loaded(0));
    assert_ne!(load(&mut cpu, thread, page(1)), Touched::Loaded(0));
    cpu.set_permissions(memory, BASE, PAGE, Permissions::READ)
        .expect("permissions");
    assert_eq!(store(&mut cpu, thread, BASE, 1), fault(BASE, Access::Write));
    assert_eq!(load(&mut cpu, thread, BASE), Touched::Loaded(0));
    cpu.write(memory, BASE, &7u64.to_le_bytes())
        .expect("the kernel writes past the guest's permissions");
    assert_eq!(load(&mut cpu, thread, BASE), Touched::Loaded(7));
    cpu.set_permissions(memory, BASE, PAGE, Permissions::empty())
        .expect("permissions");
    assert_eq!(load(&mut cpu, thread, BASE), fault(BASE, Access::Read));
    cpu.set_permissions(memory, page(1), PAGE, Permissions::EXECUTE)
        .expect("permissions");
    assert_eq!(
        load(&mut cpu, thread, page(1)),
        fault(page(1), Access::Read),
        "guest code is never read from its own pages"
    );
    let mut eight = [0; 8];
    assert_eq!(
        cpu.read(memory, OUR_AREA as u64 - 4, &mut eight),
        Err(CpuError::BadAddress)
    );
    assert_eq!(cpu.read(memory, 0, &mut eight), Err(CpuError::BadAddress));
    assert_eq!(cpu.counters().pages_copied, 0);
    end(&mut cpu, (memory, thread));
}

#[test]
fn threads_keep_their_own_registers_and_a_dropped_one_is_made_again() {
    let mut cpu = executor();
    let (memory, first) = process(&mut cpu, 1);
    let second = cpu.thread_new(memory).expect("a second thread");
    let mut seen = RegisterBlock::ZERO;
    cpu.registers(second, &mut seen);
    assert_eq!(
        seen,
        RegisterBlock::ZERO,
        "a new thread's registers are zero"
    );
    for (index, thread) in [first, second].into_iter().enumerate() {
        let mut registers = RegisterBlock::ZERO;
        registers.x[3] = 100 + index as u64;
        registers.pc = 0x40_0000 + index as u64 * 4;
        registers.v[31] = [index as u64, !(index as u64)];
        registers.tpidr_el0 = 0x7000 + index as u64;
        cpu.set_registers(thread, &registers);
    }
    for (index, thread) in [first, second].into_iter().enumerate() {
        cpu.registers(thread, &mut seen);
        assert_eq!(seen.x[3], 100 + index as u64);
        assert_eq!(seen.v[31], [index as u64, !(index as u64)]);
        assert_eq!(seen.tpidr_el0, 0x7000 + index as u64);
        assert_eq!(
            store(&mut cpu, thread, BASE + 8 * index as u64, index as u64 + 1),
            Touched::Stored
        );
    }
    assert_eq!(load(&mut cpu, first, BASE + 8), Touched::Loaded(2));
    cpu.thread_drop(second);
    let third = cpu.thread_new(memory).expect("a thread after a drop");
    cpu.registers(third, &mut seen);
    assert_eq!(seen, RegisterBlock::ZERO);
    assert_eq!(load(&mut cpu, third, BASE), Touched::Loaded(1));
    cpu.thread_drop(third);
    end(&mut cpu, (memory, first));
}

/// Each case's copy count is exact: one copy per event, and none where the page is already the toucher's own.
#[test]
fn a_fork_child_and_its_parent_each_keep_the_bytes_of_the_fork_s_instant() {
    let mut cpu = executor();
    let (parent, parent_thread) = process_units(&mut cpu, 4);
    for index in 0..2 {
        cpu.write(parent, unit(index), &(10 + index).to_le_bytes())
            .expect("write");
    }
    for index in 2..4 {
        assert_eq!(
            store(&mut cpu, parent_thread, unit(index), 10 + index),
            Touched::Stored
        );
    }
    let (child, child_thread) = fork(&mut cpu, parent);
    assert_eq!(cpu.counters().pages_copied, 0, "a fork copies nothing");
    assert_eq!(load(&mut cpu, child_thread, unit(0)), Touched::Loaded(10));
    assert_eq!(cpu.counters().pages_copied, 1, "the child's read copies");
    assert_eq!(store(&mut cpu, parent_thread, unit(1), 21), Touched::Stored);
    assert_eq!(
        cpu.counters().pages_copied,
        2,
        "the parent's write gives the page away"
    );
    assert_eq!(store(&mut cpu, parent_thread, unit(0), 20), Touched::Stored);
    assert_eq!(
        cpu.counters().pages_copied,
        2,
        "a page no one borrows any more"
    );
    assert_eq!(load(&mut cpu, child_thread, unit(1)), Touched::Loaded(11));
    assert_eq!(
        cpu.counters().pages_copied,
        2,
        "the child owns the page it was given"
    );
    assert_eq!(store(&mut cpu, child_thread, unit(2), 32), Touched::Stored);
    assert_eq!(cpu.counters().pages_copied, 3);
    assert_eq!(load(&mut cpu, parent_thread, unit(2)), Touched::Loaded(12));
    assert_eq!(
        word(&mut cpu, child, unit(3)),
        13,
        "the kernel reads a borrowed page from its owner"
    );
    assert_eq!(cpu.counters().pages_copied, 3);
    for (memory, expected) in [(parent, [20, 21, 12, 13]), (child, [10, 11, 32, 13])] {
        for (index, value) in expected.into_iter().enumerate() {
            assert_eq!(word(&mut cpu, memory, unit(index as u64)), value);
        }
    }
    assert_eq!(store(&mut cpu, child_thread, unit(1), 41), Touched::Stored);
    assert_eq!(word(&mut cpu, parent, unit(1)), 21);
    end(&mut cpu, (child, child_thread));
    end(&mut cpu, (parent, parent_thread));
}

#[test]
fn a_parent_that_forks_twice_and_a_child_that_forks_before_copying_keep_one_owner_per_unit() {
    let mut cpu = executor();
    let a = process_units(&mut cpu, 2);
    for index in 0..2 {
        assert_eq!(
            store(&mut cpu, a.1, unit(index), 100 + index),
            Touched::Stored
        );
    }
    let b = fork(&mut cpu, a.0);
    let c = fork(&mut cpu, a.0);
    assert_eq!(store(&mut cpu, a.1, unit(0), 200), Touched::Stored);
    assert_eq!(cpu.counters().pages_copied, 1, "one copy for two borrowers");
    assert_eq!(load(&mut cpu, c.1, unit(0)), Touched::Loaded(100));
    assert_eq!(
        cpu.counters().pages_copied,
        2,
        "the other borrower copies from the new owner"
    );
    assert_eq!(store(&mut cpu, b.1, unit(0), 300), Touched::Stored);
    assert_eq!(cpu.counters().pages_copied, 2);
    // B forks D before copying page 1: D borrows it from A, its owner, not from B.
    let d = fork(&mut cpu, b.0);
    assert_eq!(store(&mut cpu, a.1, unit(1), 201), Touched::Stored);
    assert_eq!(
        cpu.counters().pages_copied,
        3,
        "one copy for three borrowers"
    );
    assert_eq!(load(&mut cpu, d.1, unit(1)), Touched::Loaded(101));
    assert_eq!(load(&mut cpu, c.1, unit(1)), Touched::Loaded(101));
    assert_eq!(cpu.counters().pages_copied, 5);
    assert_eq!(store(&mut cpu, d.1, unit(0), 400), Touched::Stored);
    for (process, expected) in [
        (a, [200, 201]),
        (b, [300, 101]),
        (c, [100, 101]),
        (d, [400, 101]),
    ] {
        for (index, value) in expected.into_iter().enumerate() {
            assert_eq!(
                load(&mut cpu, process.1, unit(index as u64)),
                Touched::Loaded(value)
            );
            assert_eq!(word(&mut cpu, process.0, unit(index as u64)), value);
        }
    }
    for process in [d, c, b, a] {
        end(&mut cpu, process);
    }
}

/// A fault copies the whole aligned copy unit around the page of the lent pages in it, and the unit's other pages are
/// then the toucher's own.
#[test]
fn a_fault_copies_the_lent_pages_of_its_copy_unit_at_once() {
    let mut cpu = executor();
    let pages = 2 * COPY_UNIT / PAGE;
    let a = process(&mut cpu, pages);
    for index in 0..pages {
        assert_eq!(store(&mut cpu, a.1, page(index), index), Touched::Stored);
    }
    let b = fork(&mut cpu, a.0);
    let in_unit = COPY_UNIT / PAGE;
    assert_eq!(store(&mut cpu, b.1, page(in_unit + 1), 99), Touched::Stored);
    assert_eq!(cpu.counters().pages_copied, in_unit, "the second unit");
    for index in in_unit..pages {
        let value = if index == in_unit + 1 { 99 } else { index };
        assert_eq!(load(&mut cpu, b.1, page(index)), Touched::Loaded(value));
        assert_eq!(store(&mut cpu, b.1, page(index), 7), Touched::Stored);
    }
    assert_eq!(
        cpu.counters().pages_copied,
        in_unit,
        "the unit's pages are the child's"
    );
    for index in 0..pages {
        assert_eq!(
            word(&mut cpu, a.0, page(index)),
            index,
            "the parent's page {index}"
        );
    }
    end(&mut cpu, b);
    end(&mut cpu, a);
}

/// A parent that ends while its child still borrows keeps its file, not its process: nothing is copied for it.
#[test]
fn an_owner_that_ends_while_lending_keeps_its_pages_for_its_borrowers() {
    let mut cpu = executor();
    let a = process_units(&mut cpu, 3);
    for index in 0..3 {
        assert_eq!(
            store(&mut cpu, a.1, unit(index), 50 + index),
            Touched::Stored
        );
    }
    let b = fork(&mut cpu, a.0);
    let c = fork(&mut cpu, a.0);
    end(&mut cpu, a);
    assert_eq!(cpu.counters().retired_owners, 1);
    assert_eq!(cpu.counters().pages_copied, 0);
    assert_eq!(load(&mut cpu, b.1, unit(0)), Touched::Loaded(50));
    assert_eq!(store(&mut cpu, c.1, unit(0), 60), Touched::Stored);
    assert_eq!(cpu.counters().pages_copied, 2);
    for index in 1..3 {
        assert_eq!(
            load(&mut cpu, b.1, unit(index)),
            Touched::Loaded(50 + index)
        );
        assert_eq!(word(&mut cpu, c.0, unit(index)), 50 + index);
    }
    // A grandchild borrows from the ended owner too.
    let d = fork(&mut cpu, c.0);
    end(&mut cpu, c);
    assert_eq!(load(&mut cpu, d.1, unit(0)), Touched::Loaded(60));
    assert_eq!(load(&mut cpu, d.1, unit(2)), Touched::Loaded(52));
    end(&mut cpu, b);
    assert_eq!(load(&mut cpu, d.1, unit(1)), Touched::Loaded(51));
    end(&mut cpu, d);
}

/// A fault report is untrusted: whatever a guest forges, the executor does only what a real access of that guest would
/// do. A forks B and C, A's write makes B the owner and C a borrower from B, and B's reports are forged.
#[test]
fn a_forged_fault_report_does_only_what_a_real_access_would() {
    let mut cpu = executor();
    let a = process(&mut cpu, 2);
    assert_eq!(store(&mut cpu, a.1, page(0), 1), Touched::Stored);
    let b = fork(&mut cpu, a.0);
    let c = fork(&mut cpu, a.0);
    assert_eq!(store(&mut cpu, a.1, page(0), 2), Touched::Stored);
    cpu.set_permissions(b.0, page(1), PAGE, Permissions::empty())
        .expect("permissions");
    for _ in 0..3 {
        for address in [page(0), page(0) + 8, page(0) + PAGE - 1] {
            for access in [Access::Read, Access::Write] {
                assert_eq!(
                    cpu.resolve_fault(b.0, address, access),
                    Ok(FaultOwner::Executor)
                );
            }
            assert_eq!(
                cpu.resolve_fault(b.0, address, Access::Execute),
                Ok(FaultOwner::Guest)
            );
        }
        for address in [0, BASE - 1, page(1), page(2), OUR_AREA as u64, u64::MAX] {
            for access in [Access::Read, Access::Write, Access::Execute] {
                assert_eq!(
                    cpu.resolve_fault(b.0, address, access),
                    Ok(FaultOwner::Guest),
                    "{address:#x} {access:?}"
                );
            }
        }
    }
    assert_eq!(
        cpu.counters().pages_copied,
        2,
        "B's forged write gave its page to C once"
    );
    assert_eq!(store(&mut cpu, b.1, page(0), 3), Touched::Stored);
    assert_eq!(
        load(&mut cpu, c.1, page(0)),
        Touched::Loaded(1),
        "C's copy is the fork's instant"
    );
    assert_eq!(load(&mut cpu, b.1, page(0)), Touched::Loaded(3));
    assert_eq!(load(&mut cpu, a.1, page(0)), Touched::Loaded(2));
    assert_eq!(load(&mut cpu, b.1, page(1)), fault(page(1), Access::Read));
    for process in [c, b, a] {
        end(&mut cpu, process);
    }
}

/// A block mapped by two processes and a fork child shows each one's writes to the others; one that unmaps it keeps a
/// private copy, and one that zeroes its range has private zeros there.
#[test]
fn a_shared_block_shows_every_mapper_s_writes_until_it_is_unmapped() {
    let mut cpu = executor();
    let block = cpu.shared_new(2 * PAGE).expect("block");
    let a = process(&mut cpu, 2);
    let b = process(&mut cpu, 20);
    cpu.shared_map(block, 0, a.0, BASE, 2 * PAGE)
        .expect("map in A");
    cpu.shared_map(block, 0, b.0, page(16), 2 * PAGE)
        .expect("map in B");
    assert_eq!(store(&mut cpu, a.1, BASE, 1), Touched::Stored);
    assert_eq!(load(&mut cpu, b.1, page(16)), Touched::Loaded(1));
    assert_eq!(store(&mut cpu, b.1, page(17) + 8, 2), Touched::Stored);
    assert_eq!(load(&mut cpu, a.1, page(1) + 8), Touched::Loaded(2));
    let mut seen = [0; 8];
    cpu.shared_read(block, PAGE + 8, &mut seen)
        .expect("read the block");
    assert_eq!(u64::from_le_bytes(seen), 2);
    cpu.shared_write(block, 16, &3u64.to_le_bytes())
        .expect("write the block");
    assert_eq!(load(&mut cpu, a.1, BASE + 16), Touched::Loaded(3));
    assert_eq!(word(&mut cpu, b.0, page(16) + 16), 3);
    cpu.write(b.0, page(16) + 24, &4u64.to_le_bytes())
        .expect("the kernel writes B's view of the block");
    assert_eq!(load(&mut cpu, a.1, BASE + 24), Touched::Loaded(4));
    // A fork child maps the block too, and writes to it are no copy.
    let c = fork(&mut cpu, a.0);
    assert_eq!(load(&mut cpu, c.1, BASE), Touched::Loaded(1));
    assert_eq!(store(&mut cpu, c.1, BASE + 32, 5), Touched::Stored);
    assert_eq!(load(&mut cpu, b.1, page(16) + 32), Touched::Loaded(5));
    assert_eq!(store(&mut cpu, a.1, BASE + 40, 6), Touched::Stored);
    assert_eq!(load(&mut cpu, c.1, BASE + 40), Touched::Loaded(6));
    assert_eq!(cpu.counters().pages_copied, 0);
    // Unmapped from A: A keeps what it held, and no longer sees the block.
    cpu.shared_unmap(a.0, BASE, 2 * PAGE).expect("unmap from A");
    assert_eq!(load(&mut cpu, a.1, BASE + 32), Touched::Loaded(5));
    assert_eq!(store(&mut cpu, b.1, page(16), 7), Touched::Stored);
    assert_eq!(load(&mut cpu, a.1, BASE), Touched::Loaded(1));
    assert_eq!(store(&mut cpu, a.1, page(1), 8), Touched::Stored);
    assert_eq!(load(&mut cpu, b.1, page(17)), Touched::Loaded(0));
    assert_eq!(load(&mut cpu, c.1, BASE), Touched::Loaded(7));
    // Zeroed in C: private zeros there from now on.
    cpu.zero(c.0, BASE, PAGE).expect("zero in C");
    assert_eq!(load(&mut cpu, c.1, BASE), Touched::Loaded(0));
    assert_eq!(store(&mut cpu, b.1, page(16), 9), Touched::Stored);
    assert_eq!(load(&mut cpu, c.1, BASE), Touched::Loaded(0));
    assert_eq!(
        load(&mut cpu, c.1, page(1) + 8),
        Touched::Loaded(2),
        "the rest is still shared"
    );
    cpu.set_permissions(b.0, page(16), PAGE, Permissions::READ)
        .expect("permissions");
    assert_eq!(
        store(&mut cpu, b.1, page(16), 1),
        fault(page(16), Access::Write)
    );
    for process in [a, b, c] {
        let _ = load(&mut cpu, process.1, BASE);
        same_mappings(&cpu, process.0, 0);
    }
    for process in [c, b, a] {
        end(&mut cpu, process);
    }
    cpu.shared_drop(block);
}

fn descriptors(process: rustix::process::Pid) -> usize {
    std::fs::read_dir(format!("/proc/{}/fd", process.as_raw_nonzero()))
        .expect("the stub's descriptors")
        .count()
}

/// The embedder's descriptors of shared blocks: this test's, and those of any other test running beside it.
fn blocks_held() -> usize {
    std::fs::read_dir("/proc/self/fd")
        .expect("our descriptors")
        .filter_map(|entry| std::fs::read_link(entry.ok()?.path()).ok())
        .filter(|target| {
            target
                .to_string_lossy()
                .contains("wcontainers shared block")
        })
        .count()
}

/// Each mapping's memfd is closed in the stub once mapped, and in the embedder once sent.
#[test]
fn a_block_mapped_ten_thousand_times_leaves_no_descriptor_behind() {
    let mut cpu = executor();
    let maps = knob("WCHP_MAPS", 10_000);
    let block = cpu.shared_new(PAGE).expect("block");
    cpu.shared_write(block, 0, &11u64.to_le_bytes())
        .expect("write the block");
    let (memory, thread) = process(&mut cpu, 2 * maps);
    for index in 0..maps {
        cpu.shared_map(block, 0, memory, page(2 * index), PAGE)
            .expect("map");
        if index % 1000 == 999 {
            assert_eq!(load(&mut cpu, thread, page(2 * index)), Touched::Loaded(11));
        }
    }
    assert_eq!(load(&mut cpu, thread, page(0)), Touched::Loaded(11));
    assert_eq!(load(&mut cpu, thread, page(1)), Touched::Loaded(0));
    assert_eq!(
        host_mappings(cpu.stub_process(memory)),
        cpu.host_mappings(memory),
        "the host mappings counted"
    );
    assert_eq!(
        descriptors(cpu.stub_process(memory)),
        2,
        "its socket and its memory"
    );
    // Other tests beside this one hold a few blocks; a copy kept per map would be thousands.
    let held = blocks_held();
    assert!(held < 16, "{held} block descriptors after {maps} maps");
    end(&mut cpu, (memory, thread));
    cpu.shared_drop(block);
}

/// More queued changes than one control page holds are applied in rounds before the access.
#[test]
fn a_queue_longer_than_a_page_is_applied_in_rounds() {
    let mut cpu = executor();
    let pages = 3 * MAX_COMMANDS as u64;
    let (memory, thread) = process(&mut cpu, pages);
    let _ = load(&mut cpu, thread, BASE);
    for index in (1..pages).step_by(2) {
        cpu.set_permissions(memory, page(index), PAGE, Permissions::READ)
            .expect("permissions");
    }
    assert_eq!(store(&mut cpu, thread, page(0), 5), Touched::Stored);
    assert_eq!(cpu.counters().commands_past_one_page, 1);
    for index in [1, 2, pages - 2, pages - 1] {
        let expected = if index % 2 == 1 {
            fault(page(index), Access::Write)
        } else {
            Touched::Stored
        };
        assert_eq!(
            store(&mut cpu, thread, page(index), 5),
            expected,
            "page {index}"
        );
    }
    end(&mut cpu, (memory, thread));
}

/// A change that could take a stub past the host's limit on mappings is refused as out of memory before the host
/// refuses it, and every change before it is applied.
#[test]
fn a_change_past_the_host_s_mapping_limit_is_refused_up_front() {
    let mut cpu = executor();
    let limit = HOST.get().expect("the host").probe().mapping_limit();
    // gVisor gives 2^31 - 1, which no test reaches in reasonable time (`WCHP_MAPPING_LIMIT_MAX`).
    let reachable = knob("WCHP_MAPPING_LIMIT_MAX", 1 << 22);
    if limit as u64 > reachable {
        eprintln!("not run: the host's limit of {limit} mappings is past {reachable}");
        return;
    }
    let pages = limit as u64 + 8;
    let (memory, thread) = process(&mut cpu, pages);
    let _ = load(&mut cpu, thread, BASE);
    let refused = (1..pages).step_by(2).find_map(|index| {
        cpu.set_permissions(memory, page(index), PAGE, Permissions::READ)
            .err()
            .map(|error| (index, error))
    });
    let (index, error) = refused.expect("a change refused before the limit");
    assert_eq!(error, CpuError::OutOfMemory);
    assert_eq!(cpu.counters().mappings_exhausted, 1);
    assert_eq!(store(&mut cpu, thread, page(0), 1), Touched::Stored);
    assert_eq!(
        store(&mut cpu, thread, page(index - 2), 1),
        fault(page(index - 2), Access::Write)
    );
    assert_eq!(
        store(&mut cpu, thread, page(index), 1),
        Touched::Stored,
        "the refused change is not made"
    );
    let held = host_mappings(cpu.stub_process(memory));
    assert_eq!(held, cpu.host_mappings(memory));
    assert!(held <= limit && held + 2 > limit, "{held} of {limit}");
    // Merging needs no new mapping, but the rule's margin refuses it too, never the host.
    assert_eq!(
        cpu.set_permissions(memory, page(1), PAGE, READ_WRITE),
        Err(CpuError::OutOfMemory)
    );
    end(&mut cpu, (memory, thread));
}

/// A command the host refuses at a turn never leaves the stub allowing more than the kernel: a page made read-only
/// after a refused mapping is not written by the next turn, though the host takes mappings again by then.
#[test]
fn a_refused_command_never_leaves_a_narrowed_page_writable() {
    use rustix::process::{Resource, Rlimit, prlimit};
    let mut cpu = executor();
    let (memory, thread) = process(&mut cpu, 8);
    assert_eq!(store(&mut cpu, thread, page(4), 1), Touched::Stored);
    let block = cpu.shared_new(PAGE).expect("a block");
    cpu.shared_map(block, 0, memory, page(0), PAGE)
        .expect("the block mapped");
    cpu.set_permissions(memory, page(4), PAGE, Permissions::READ)
        .expect("read only");
    // The host refuses the block's mapping: the stub's address space is past the limit it is given now.
    let stub = cpu.stub_process(memory);
    let limit = |current| Rlimit {
        current,
        maximum: None,
    };
    prlimit(Some(stub), Resource::As, limit(Some(1 << 20))).expect("the stub's limit");
    assert_eq!(
        cpu.touch(thread, Touch::Store(page(4), 2), None),
        Err(CpuError::OutOfMemory)
    );
    // A stub that ended has no limit to put back.
    let _ = prlimit(Some(stub), Resource::As, limit(None));
    let after = cpu.touch(thread, Touch::Store(page(4), 3), None);
    assert_ne!(after, Ok(Touched::Stored), "the read-only page was written");
    assert_eq!(word(&mut cpu, memory, page(4)), 1);
    end(&mut cpu, (memory, thread));
    cpu.shared_drop(block);
}

/// As above, where the kill of the stub after the refused command fails: the zygote is held stopped and its socket
/// shut, so it answers no request, and the stub lives on. The next turn must not run on the narrowed page's old view.
#[test]
fn a_failed_kill_after_a_refused_command_never_leaves_a_narrowed_page_writable() {
    use rustix::process::{PidfdFlags, PidfdGetfdFlags, Resource, Rlimit, Signal, prlimit};
    // A host of its own, as its zygote is broken on purpose, started once the shared one is: two hosts starting at
    // once both want the zygote's fixed pages in this process.
    drop(executor());
    let host = Arc::new(HostProcessHost::new().expect("the host is refused"));
    let zygote = host.zygote_process().expect("a zygote");
    let mut cpu = HostProcessExecutor::new(host.clone());
    /// Kills the zygote, its stubs with it, before the host waits for its end, even where the test fails.
    struct KillZygote(rustix::process::Pid);
    impl Drop for KillZygote {
        fn drop(&mut self) {
            let _ = rustix::process::kill_process(self.0, Signal::KILL);
        }
    }
    let _kill = KillZygote(zygote);
    let (memory, thread) = process(&mut cpu, 8);
    assert_eq!(store(&mut cpu, thread, page(4), 1), Touched::Stored);
    let block = cpu.shared_new(PAGE).expect("a block");
    cpu.shared_map(block, 0, memory, page(0), PAGE)
        .expect("the block mapped");
    cpu.set_permissions(memory, page(4), PAGE, Permissions::READ)
        .expect("read only");
    let stub = cpu.stub_process(memory);
    let limit = |current| Rlimit {
        current,
        maximum: None,
    };
    prlimit(Some(stub), Resource::As, limit(Some(1 << 20))).expect("the stub's limit");
    // Every socket of the zygote shut, while it is stopped so it does not end on seeing it.
    rustix::process::kill_process(zygote, Signal::STOP).expect("stop the zygote");
    let pidfd =
        rustix::process::pidfd_open(zygote, PidfdFlags::empty()).expect("the zygote's pidfd");
    let mut shut = 0;
    for entry in
        std::fs::read_dir(format!("/proc/{}/fd", zygote.as_raw_nonzero())).expect("its descriptors")
    {
        let entry = entry.expect("a descriptor");
        let target = std::fs::read_link(entry.path()).expect("its target");
        if !target.to_string_lossy().starts_with("socket:") {
            continue;
        }
        let number: i32 = entry
            .file_name()
            .to_string_lossy()
            .parse()
            .expect("a number");
        let socket = rustix::process::pidfd_getfd(&pidfd, number, PidfdGetfdFlags::empty())
            .expect("a copy of the zygote's socket");
        rustix::net::shutdown(&socket, rustix::net::Shutdown::Both).expect("shut");
        shut += 1;
    }
    assert!(shut > 0, "the zygote holds a socket");
    assert_eq!(
        cpu.touch(thread, Touch::Store(page(4), 2), None),
        Err(CpuError::OutOfMemory)
    );
    assert_eq!(
        rustix::process::test_kill_process(stub),
        Ok(()),
        "the stub lives on, as the kill failed"
    );
    let _ = prlimit(Some(stub), Resource::As, limit(None));
    let after = cpu.touch(thread, Touch::Store(page(4), 3), None);
    assert_ne!(after, Ok(Touched::Stored), "the read-only page was written");
    assert_eq!(word(&mut cpu, memory, page(4)), 1);
}

/// A dropped memory whose stub is not seen to end (here its socket is held open by a copy, so it never hangs up) is
/// left to the zygote's end: the zygote, which answers nothing while it waits for a stub, is never asked to, and the
/// next memory's stub starts.
#[test]
fn a_dropped_memory_s_stub_not_seen_to_end_is_not_waited_for() {
    use rustix::process::{PidfdFlags, PidfdGetfdFlags};
    use wcontainers_executor_host_process::stub::SOCKET_FD;
    let mut cpu = executor();
    let (memory, thread) = process(&mut cpu, 1);
    assert_eq!(store(&mut cpu, thread, page(0), 1), Touched::Stored);
    let stub = cpu.stub_process(memory);
    let pidfd = rustix::process::pidfd_open(stub, PidfdFlags::empty()).expect("the stub's pidfd");
    let held = rustix::process::pidfd_getfd(&pidfd, SOCKET_FD as i32, PidfdGetfdFlags::empty())
        .expect("a copy of the stub's socket");
    let before = cpu.counters().stubs_left_unreaped;
    cpu.thread_drop(thread);
    cpu.memory_drop(memory);
    assert_eq!(cpu.counters().stubs_left_unreaped, before + 1);
    let state = std::fs::read_to_string(format!("/proc/{}/stat", stub.as_raw_nonzero()))
        .expect("the stub is not reaped");
    let state = state
        .rsplit_once(") ")
        .and_then(|(_, rest)| rest.chars().next());
    assert_eq!(state, Some('Z'), "the stub was killed and left unreaped");
    drop(held);
    let next = process(&mut cpu, 1);
    assert_eq!(store(&mut cpu, next.1, page(0), 2), Touched::Stored);
    end(&mut cpu, next);
}

/// The cost of a fork's copies to its child, one store per page over `WCHP_PAGES` written pages, in order and in a
/// seeded order touching every `WCHP_STRIDE`th page (`WCHP_SEED`), and the host mappings each side holds after.
#[test]
#[ignore = "a measurement"]
fn copy_unit_cost() {
    let pages = knob("WCHP_PAGES", 4096);
    let stride = knob("WCHP_STRIDE", 16);
    let mut random = fastrand::Rng::with_seed(knob("WCHP_SEED", 1));
    let mut cpu = executor();
    let (memory, thread) = process(&mut cpu, pages);
    for index in 0..pages {
        assert_eq!(store(&mut cpu, thread, page(index), index), Touched::Stored);
    }
    let mut scattered: Vec<u64> = (0..pages).step_by(stride as usize).collect();
    random.shuffle(&mut scattered);
    for (name, order) in [
        ("in order", (0..pages).collect::<Vec<_>>()),
        ("scattered", scattered),
    ] {
        let child = fork(&mut cpu, memory);
        let _ = load(&mut cpu, child.1, page(0));
        let copied = cpu.counters().pages_copied;
        let started = std::time::Instant::now();
        for &index in &order {
            assert_eq!(store(&mut cpu, child.1, page(index), 1), Touched::Stored);
        }
        let took = started.elapsed();
        let _ = load(&mut cpu, thread, page(0));
        eprintln!(
            "{name}: {} stores, {:.2} us each, {} pages copied, host mappings child {} parent {}",
            order.len(),
            took.as_secs_f64() * 1e6 / order.len() as f64,
            cpu.counters().pages_copied - copied,
            host_mappings(cpu.stub_process(child.0)),
            host_mappings(cpu.stub_process(memory)),
        );
        end(&mut cpu, child);
    }
    end(&mut cpu, (memory, thread));
}

fn knob(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// The model of one process: its words and what each page allows.
#[derive(Clone)]
struct Model {
    words: Vec<u64>,
    allowed: [Permissions; MODEL_PAGES as usize],
}

const MODEL_PAGES: u64 = 8;
const WORDS_PER_PAGE: u64 = PAGE / 8;

fn same_as_model(
    cpu: &mut HostProcessExecutor,
    memory: MemoryId,
    model: &Model,
    pages: std::ops::Range<u64>,
    step: u64,
) {
    let mut bytes = vec![0; ((pages.end - pages.start) * PAGE) as usize];
    cpu.read(memory, page(pages.start), &mut bytes)
        .expect("read");
    let first = (pages.start * WORDS_PER_PAGE) as usize;
    for (index, chunk) in bytes.as_chunks::<8>().0.iter().enumerate() {
        assert_eq!(
            u64::from_le_bytes(*chunk),
            model.words[first + index],
            "step {step}, memory {memory:?}, word {}",
            first + index
        );
    }
}

/// The host mappings a stub holds once its queue is applied are the ones the executor counts.
fn same_mappings(cpu: &HostProcessExecutor, memory: MemoryId, step: u64) {
    assert_eq!(
        host_mappings(cpu.stub_process(memory)),
        cpu.host_mappings(memory),
        "step {step}, memory {memory:?}"
    );
}

/// Seeded forks, ends, the kernel's writes and zeros, the guest's loads and stores and permission changes on up to six
/// processes, each checked against a model where a fork is a copy (`WCHP_SEED`, `WCHP_STEPS`).
#[test]
fn copy_on_write_matches_a_model_where_a_fork_is_a_copy() {
    let seed = knob("WCHP_SEED", 1);
    let steps = knob("WCHP_STEPS", 30_000);
    let mut random = fastrand::Rng::with_seed(seed);
    let mut cpu = executor();
    let first = process(&mut cpu, MODEL_PAGES);
    let mut live: BTreeMap<u32, ((MemoryId, ThreadId), Model)> = BTreeMap::new();
    live.insert(
        first.0.0,
        (
            first,
            Model {
                words: vec![0; (MODEL_PAGES * WORDS_PER_PAGE) as usize],
                allowed: [READ_WRITE; MODEL_PAGES as usize],
            },
        ),
    );
    let mut forks = 0;
    for step in 0..steps {
        let keys: Vec<u32> = live.keys().copied().collect();
        let key = keys[random.usize(..keys.len())];
        let ((memory, thread), mut model) = live.get(&key).cloned().expect("live");
        let word_index = random.u64(..MODEL_PAGES * WORDS_PER_PAGE);
        let index = word_index / WORDS_PER_PAGE;
        let address = BASE + 8 * word_index;
        let value = random.u64(..);
        match random.u32(..100) {
            0..6 if live.len() < 6 => {
                let child = fork(&mut cpu, memory);
                forks += 1;
                live.insert(child.0.0, (child, model.clone()));
            }
            6..10 if live.len() > 1 => {
                end(&mut cpu, (memory, thread));
                live.remove(&key);
                continue;
            }
            10..25 => {
                // Up to two pages from the word, crossing page edges.
                let count = random
                    .u64(1..=(2 * WORDS_PER_PAGE).min(MODEL_PAGES * WORDS_PER_PAGE - word_index));
                let words: Vec<u64> = (0..count).map(|_| random.u64(..)).collect();
                let bytes: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
                cpu.write(memory, address, &bytes).expect("write");
                model.words[word_index as usize..(word_index + count) as usize]
                    .copy_from_slice(&words);
            }
            25..50 => {
                let got = store(&mut cpu, thread, address, value);
                same_mappings(&cpu, memory, step);
                if model.allowed[index as usize].contains(Permissions::WRITE) {
                    assert_eq!(got, Touched::Stored, "step {step}");
                    model.words[word_index as usize] = value;
                } else {
                    assert_eq!(got, fault(address, Access::Write), "step {step}");
                }
            }
            50..80 => {
                let got = load(&mut cpu, thread, address);
                same_mappings(&cpu, memory, step);
                if model.allowed[index as usize].intersects(READ_WRITE) {
                    assert_eq!(
                        got,
                        Touched::Loaded(model.words[word_index as usize]),
                        "step {step}"
                    );
                } else {
                    assert_eq!(got, fault(address, Access::Read), "step {step}");
                }
            }
            80..87 => {
                cpu.zero(memory, page(index), PAGE).expect("zero");
                let first_word = (index * WORDS_PER_PAGE) as usize;
                model.words[first_word..first_word + WORDS_PER_PAGE as usize].fill(0);
            }
            87..100 => {
                let allowed =
                    [READ_WRITE, Permissions::READ, Permissions::empty()][random.usize(..3)];
                let length = random.u64(1..=MODEL_PAGES - index);
                cpu.set_permissions(memory, page(index), length * PAGE, allowed)
                    .expect("permissions");
                model.allowed[index as usize..(index + length) as usize].fill(allowed);
            }
            _ => {}
        }
        live.get_mut(&key).expect("live").1 = model;
        // Every process's copy of the touched page, so a change that reaches another process shows at once.
        for ((memory, _), model) in live.values() {
            same_as_model(&mut cpu, *memory, model, index..index + 1, step);
        }
    }
    for ((memory, _), model) in live.values() {
        same_as_model(&mut cpu, *memory, model, 0..MODEL_PAGES, steps);
    }
    let counters = cpu.counters();
    eprintln!(
        "seed {seed}: {steps} steps, {forks} forks, {} pages copied, {} faults resolved, {} owners retired",
        counters.pages_copied, counters.faults_resolved, counters.retired_owners
    );
    for (process, _) in live.into_values() {
        end(&mut cpu, process);
    }
}
