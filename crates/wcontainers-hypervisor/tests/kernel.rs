//! N3's kernel and loop inside the machine: the test program `native-kernel` takes a container's files and
//! start through the link and runs its first process to its end in the kernel on the native CPU side, with the
//! runtime's loop and the kernel test crate's policy, the host's services through the link. The same kernel,
//! loop and policy run the same container on the WebAssembly output's CPU side on the host, through the same
//! code over a link in memory. The wait status, every call of the host's services and the root's files at the
//! end must agree, and for a program with one thread the number of turns. A program of this track's makes
//! getppid calls: the virtual machine's exits must not grow with their number, as a system call is answered
//! inside. Another forks, execs itself and takes signals, so turns end at the limits the policy gives. Another
//! writes code and announces it, in private memory and in memory a child changes. Another asks for memory as
//! programs do, more than the machine has in all; asked to write more than the machine has at once, it is killed
//! natively and the inside goes on, counted and warned. Another zeroes blocks and cleans cache lines on pages
//! never touched, read-only, allowing nothing and unmapped. Another jumps through tables of bytes to targets that
//! can and cannot run, and runs again with a stop at every counted event, whose positions must agree across
//! look-aheads. The benchmark set's programs run at their small size, and so again. hello and these programs
//! also run in the reference, whose output and exit code the native side must give.
//!
//! `WCONTAINERS_KERNEL_SEED` seeds the policy and the host's random bytes; `WCONTAINERS_KERNEL_SEEDS` runs that
//! many seeds, it and the ones after it.

#[cfg(target_os = "macos")]
#[path = "../../wcontainers-inside/src/bin/native-kernel/protocol.rs"]
mod protocol;

#[cfg(target_os = "macos")]
mod common;

#[cfg(target_os = "macos")]
mod container;

#[cfg(target_os = "macos")]
fn main() {
    macos::main();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("the kernel test needs Hypervisor.framework; nothing to run on this host");
}

#[cfg(target_os = "macos")]
mod macos {
    use std::time::Instant;

    use linux_raw_sys::general::SIGSEGV;
    use wcontainers_guest_cpu::reason::{CODE_CHANGED_AFTER_IT_RAN, MEMORY_FAULT};
    use wcontainers_inside::reason::{
        GUEST_PHYSICAL_SPACE_FULL, LARGE_ENTRY_SPLIT, RESERVATION_BROKEN,
    };
    use wcontainers_kernel_test::driver::exit_code;
    use wcontainers_kernel_test::reference;

    use crate::common::{
        BENCH, CALLS, CLibrary, KERNEL_TEST_GUEST, MEMORY, OWN_GUEST, PROGRAMS_DIRECTORY,
        be_signed, compile, compiled, freestanding, load_average,
    };
    use crate::container::{
        Outcome, REALTIME_NANOSECONDS, native, native_at_every_event, one_program as container,
        seeds, summary, wasm,
    };

    /// The bar's first program, from the kernel test crate.
    const HELLO: &str = "hello";

    /// This track's program that makes as many getppid calls as its argument says.
    const GETPPID: &str = "getppid";

    /// This track's program that forks, execs itself and takes signals.
    const FORKS: &str = "forks";

    /// This track's program that writes code and announces it.
    const CODE: &str = "code";

    /// The changes `code` makes to code that ran: one announced, one announced by a child sharing the memory, one
    /// the kernel learns of when the page stops being executable, one announced in the middle of a function, and
    /// one where a return lands.
    const CODE_CHANGES: u64 = 5;
    /// Why `code`'s output differs on the WebAssembly side.
    const CODE_KNOWN: &str = "the WebAssembly side runs a function's old code after only its second cache line is rewritten and announced";

    /// This track's program that asks for memory.
    const MAPPINGS: &str = "mappings";

    /// This track's program that zeroes blocks and cleans cache lines on pages in every state.
    const BLOCKS: &str = "blocks";

    /// Why `blocks` gives other output on the WebAssembly side: it reports a block zeroing's fault at the
    /// block's start, where Linux reports the address in the register (the WebAssembly meaning of `dc zva`).
    const BLOCKS_KNOWN: &str =
        "the WebAssembly side reports a zeroed block's fault at the block's start";

    /// This track's program of jumps through tables of bytes.
    const JUMP_TABLES: &str = "jumptables";

    /// The bits of a mebibyte's size, the unit `memory` takes its argument in.
    const MIB_BITS: u32 = 20;

    /// The getppid calls of the exit count (ten to the fifth, as the independent check asks), and twice as many.
    const GETPPID_CALLS: u64 = 10_u64.pow(5);

    /// Runs a container on both sides, checks that the wait status and the root's files agree, and gives both
    /// outcomes, the native side's first.
    fn run_both(seed: u64, name: &str, image: &[u8], arguments: &[String]) -> (Outcome, Outcome) {
        let (files, starting) = container(name, image.to_vec(), arguments);
        let native = native(seed, REALTIME_NANOSECONDS, &files, &starting);
        let wasm = wasm(seed, REALTIME_NANOSECONDS, &files, &starting);
        for (side, outcome) in [("native", &native), ("WebAssembly", &wasm)] {
            println!(
                "{name} {arguments:?} on the {side} side: status {:#x} after {} turns, {} host calls, {} exits, {} files at the end, warned {:?}",
                outcome.status,
                outcome.turns,
                outcome.host.calls.len(),
                outcome.exits,
                outcome.root.len(),
                summary(&outcome.warnings)
            );
        }
        assert_eq!(native.status, wasm.status, "{name}: the wait status");
        assert_eq!(
            native.root, wasm.root,
            "{name}: the root's files at the end"
        );
        (native, wasm)
    }

    /// Where the two sides' calls of the host's services first differ, if they do.
    fn calls_differ(native: &Outcome, wasm: &Outcome) -> Option<String> {
        let calls = (&native.host.calls, &wasm.host.calls);
        let differ = calls.0.iter().zip(calls.1).position(|(a, b)| a != b);
        (differ.is_some() || calls.0.len() != calls.1.len()).then(|| {
            format!(
                "the calls of the host's services differ at call {differ:?} of {} and {}: {:?} natively, {:?} on the WebAssembly side",
                calls.0.len(),
                calls.1.len(),
                differ.map(|i| &calls.0[i]),
                differ.map(|i| &calls.1[i]),
            )
        })
    }

    /// As [`run_both`], and the calls of the host's services agree.
    fn both(seed: u64, name: &str, image: &[u8], arguments: &[String]) -> (Outcome, Outcome) {
        let (native, wasm) = run_both(seed, name, image, arguments);
        if let Some(difference) = calls_differ(&native, &wasm) {
            panic!("{name}: {difference}");
        }
        (native, wasm)
    }

    /// The count of `reason` among an outcome's warnings.
    fn warned(outcome: &Outcome, reason: &str) -> u64 {
        summary(&outcome.warnings)
            .get(reason)
            .copied()
            .unwrap_or_default()
    }

    /// As [`both`], for a program with one thread, whose turns end only at its system calls: the turns agree
    /// too. (Turns that end at a limit end at different places: the WebAssembly output counts progress by its
    /// own stop checks, the native output by the count's definition.)
    fn alone(seed: u64, name: &str, image: &[u8], arguments: &[String]) -> Outcome {
        let (native, wasm) = both(seed, name, image, arguments);
        assert_eq!(native.turns, wasm.turns, "{name}: the turns");
        native
    }

    /// Checks the native side's output and exit code against the program's run in the reference.
    fn against_reference(name: &str, native: &Outcome) {
        let reference = reference::run(&compiled(name), &[]);
        assert_eq!(
            String::from_utf8_lossy(&native.host.stdout),
            String::from_utf8_lossy(&reference.stdout),
            "{name}: standard output against the reference"
        );
        assert_eq!(
            String::from_utf8_lossy(&native.host.stderr),
            String::from_utf8_lossy(&reference.stderr),
            "{name}: standard error against the reference"
        );
        assert_eq!(
            exit_code(native.status),
            reference.exit_code,
            "{name}: the exit code against the reference"
        );
        println!("{name}: the reference's output and exit code");
    }

    pub fn main() {
        be_signed();
        let seeds = seeds();
        for &seed in &seeds {
            with_seed(seed);
        }
        println!("kernel: {} seeds ran alike", seeds.len());
    }

    fn with_seed(seed: u64) {
        let started = Instant::now();
        let hello = compile(KERNEL_TEST_GUEST, HELLO, CLibrary::Without);
        against_reference(HELLO, &alone(seed, HELLO, &hello, &[]));
        let getppid = compile(OWN_GUEST, GETPPID, CLibrary::Musl);
        let exits = [0, GETPPID_CALLS, 2 * GETPPID_CALLS]
            .map(|calls| alone(seed, GETPPID, &getppid, &[calls.to_string()]).exits);
        println!(
            "getppid: {} exits with no calls, {} with {GETPPID_CALLS}, {} with {}",
            exits[0],
            exits[1],
            exits[2],
            2 * GETPPID_CALLS
        );
        assert_eq!(
            exits[1], exits[2],
            "the virtual machine's exits grow with the number of system calls"
        );
        // The benchmark set at its small size, whose count at every stop must agree across look-aheads, with stops
        // where its system calls end turns and then after every counted event.
        for name in [BENCH, CALLS] {
            let built = freestanding(PROGRAMS_DIRECTORY, name, name, &[], None);
            let image = std::fs::read(built).expect("the built program");
            let native = alone(seed, name, &image, &[]);
            print!("{name}: {}", String::from_utf8_lossy(&native.host.stdout));
            against_reference(name, &native);
            let started = Instant::now();
            let (files, starting) = container(name, image, &[]);
            let every = native_at_every_event(seed, REALTIME_NANOSECONDS, &files, &starting);
            assert_eq!(
                (every.status, &every.host.stdout),
                (native.status, &native.host.stdout),
                "{name}: a stop at every counted event changed the run"
            );
            println!(
                "{name}: a stop at every counted event, (stops, hash of positions) {:?} alike across look-aheads, in {:?}",
                every.positions,
                started.elapsed()
            );
        }
        let jump_tables = compile(OWN_GUEST, JUMP_TABLES, CLibrary::Musl);
        let (native, _) = both(seed, JUMP_TABLES, &jump_tables, &[]);
        print!(
            "{JUMP_TABLES}: {}",
            String::from_utf8_lossy(&native.host.stdout)
        );
        against_reference(JUMP_TABLES, &native);
        let (files, starting) = container(JUMP_TABLES, jump_tables, &[]);
        let every = native_at_every_event(seed, REALTIME_NANOSECONDS, &files, &starting);
        assert_eq!(
            (every.status, &every.host.stdout),
            (native.status, &native.host.stdout),
            "{JUMP_TABLES}: a stop at every counted event changed the run"
        );
        println!(
            "{JUMP_TABLES}: a stop at every counted event, (stops, hash of positions) {:?} alike across look-aheads",
            every.positions
        );
        let forks = compile(OWN_GUEST, FORKS, CLibrary::Musl);
        let (native, _) = both(seed, FORKS, &forks, &[]);
        print!("{FORKS}: {}", String::from_utf8_lossy(&native.host.stdout));
        against_reference(FORKS, &native);
        let code = compile(OWN_GUEST, CODE, CLibrary::Musl);
        let (native, wasm) = run_both(seed, CODE, &code, &[]);
        print!("{CODE}: {}", String::from_utf8_lossy(&native.host.stdout));
        if let Some(difference) = calls_differ(&native, &wasm) {
            println!("{CODE}: KNOWN FAILURE, {CODE_KNOWN}: {difference}");
        }
        let changed = |outcome: &Outcome| warned(outcome, CODE_CHANGED_AFTER_IT_RAN);
        println!(
            "{CODE}: \"{CODE_CHANGED_AFTER_IT_RAN}\" {} natively, {} on the WebAssembly side",
            changed(&native),
            changed(&wasm)
        );
        assert_eq!(
            changed(&native),
            CODE_CHANGES,
            "{CODE}: \"{CODE_CHANGED_AFTER_IT_RAN}\" natively"
        );
        against_reference(CODE, &native);
        let blocks = compile(OWN_GUEST, BLOCKS, CLibrary::Musl);
        let (native, wasm) = run_both(seed, BLOCKS, &blocks, &[]);
        print!("{BLOCKS}: {}", String::from_utf8_lossy(&native.host.stdout));
        if let Some(difference) = calls_differ(&native, &wasm) {
            println!("{BLOCKS}: KNOWN FAILURE, {BLOCKS_KNOWN}: {difference}");
        }
        let faults = [&native, &wasm].map(|outcome| warned(outcome, MEMORY_FAULT));
        println!(
            "{BLOCKS}: \"{MEMORY_FAULT}\" {} natively, {} on the WebAssembly side",
            faults[0], faults[1]
        );
        assert!(
            faults[0] == faults[1] && faults[0] > 0,
            "{BLOCKS}: \"{MEMORY_FAULT}\" counted alike"
        );
        against_reference(BLOCKS, &native);
        let mappings = compile(OWN_GUEST, MAPPINGS, CLibrary::Musl);
        let (native, _) = both(seed, MAPPINGS, &mappings, &[]);
        print!(
            "{MAPPINGS}: {}",
            String::from_utf8_lossy(&native.host.stdout)
        );
        against_reference(MAPPINGS, &native);
        let split = warned(&native, LARGE_ENTRY_SPLIT);
        println!("{MAPPINGS}: \"{LARGE_ENTRY_SPLIT}\" {split} natively");
        assert!(
            split > 0,
            "{MAPPINGS}: a block-sized range written at every page was mapped by a block entry, split when one page's permissions changed"
        );
        let beyond = (MEMORY + MEMORY / 2) >> MIB_BITS;
        let (files, starting) = container(MAPPINGS, mappings, &[beyond.to_string()]);
        let killed = crate::container::native(seed, REALTIME_NANOSECONDS, &files, &starting);
        let full = warned(&killed, GUEST_PHYSICAL_SPACE_FULL);
        let broken = warned(&killed, RESERVATION_BROKEN);
        println!(
            "{MAPPINGS} writing {beyond} MiB natively: status {:#x}, \"{GUEST_PHYSICAL_SPACE_FULL}\" {full}, \"{RESERVATION_BROKEN}\" {broken}",
            killed.status
        );
        assert_eq!(
            killed.status, SIGSEGV as i32,
            "{MAPPINGS}: killed when guest-physical memory is full"
        );
        assert!(
            full > 0,
            "{MAPPINGS}: \"{GUEST_PHYSICAL_SPACE_FULL}\" counted"
        );
        println!(
            "kernel: ran alike to the end; seed {seed:#x}; in {:?}, load {}",
            started.elapsed(),
            load_average()
        );
    }
}
