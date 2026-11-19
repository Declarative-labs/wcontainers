//! N3's kernel and loop inside the machine: the test program `native-kernel` takes a container's files and
//! start through the link and runs its first process to its end in the kernel on the native CPU side, with the
//! runtime's loop and the kernel test crate's policy, the host's services through the link. The same kernel,
//! loop and policy run the same container on the WebAssembly output's CPU side on the host, through the same
//! code over a link in memory. The wait status, the number of turns, every call of the host's services and
//! the root's files at the end must agree. A program of this track's makes getppid calls: the virtual
//! machine's exits must not grow with their number, as a system call is answered inside.
//!
//! `WCONTAINERS_KERNEL_SEED` seeds the policy and the host's random bytes.

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

    use linux_raw_sys::general::{S_IRGRP, S_IROTH, S_IRWXU, S_IXGRP, S_IXOTH};
    use wcontainers_inside::container::{File, FileKind, Starting};

    use crate::common::{
        CLibrary, KERNEL_TEST_GUEST, OWN_GUEST, be_signed, compile, load_average, setting,
    };
    use crate::container::{DEFAULT_SEED, Outcome, SEED_VARIABLE, native, summary, wasm};

    /// The bar's first program, from the kernel test crate.
    const HELLO: &str = "hello";

    /// This track's program that makes as many getppid calls as its argument says.
    const GETPPID: &str = "getppid";

    /// The getppid calls of the exit count (ten to the fifth, as the independent check asks), and twice as many.
    const GETPPID_CALLS: u64 = 10_u64.pow(5);

    /// A container of one program at the root, started with `arguments` after its path.
    fn container(name: &str, image: Vec<u8>, arguments: &[String]) -> (Vec<File>, Starting) {
        let path = format!("/{name}").into_bytes();
        let files = vec![File {
            path: path.clone(),
            mode: S_IRWXU | S_IRGRP | S_IXGRP | S_IROTH | S_IXOTH,
            uid: 0,
            gid: 0,
            mtime_seconds: 0,
            kind: FileKind::Regular(image),
        }];
        let starting = Starting {
            path: path.clone(),
            arguments: std::iter::once(path)
                .chain(arguments.iter().map(|a| a.clone().into_bytes()))
                .collect(),
            environment: vec![b"PATH=/".to_vec()],
            working_directory: b"/".to_vec(),
            hostname: b"native".to_vec(),
            groups: Vec::new(),
            backend: None,
        };
        (files, starting)
    }

    /// Runs a container on both sides, checks they agree, and gives the native side's outcome.
    fn both(seed: u64, name: &str, image: &[u8], arguments: &[String]) -> Outcome {
        let (files, starting) = container(name, image.to_vec(), arguments);
        let native = native(seed, &files, &starting);
        let wasm = wasm(seed, &files, &starting);
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
        assert_eq!(native.turns, wasm.turns, "{name}: the turns");
        assert_eq!(
            native.host.calls, wasm.host.calls,
            "{name}: the calls of the host's services"
        );
        assert_eq!(
            native.root, wasm.root,
            "{name}: the root's files at the end"
        );
        native
    }

    pub fn main() {
        be_signed();
        let seed = setting(SEED_VARIABLE, DEFAULT_SEED);
        let started = Instant::now();
        let hello = compile(KERNEL_TEST_GUEST, HELLO, CLibrary::Without);
        both(seed, HELLO, &hello, &[]);
        let getppid = compile(OWN_GUEST, GETPPID, CLibrary::Musl);
        let exits = [0, GETPPID_CALLS, 2 * GETPPID_CALLS]
            .map(|calls| both(seed, GETPPID, &getppid, &[calls.to_string()]).exits);
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
        println!(
            "kernel: ran alike to the end; seed {seed:#x}; in {:?}, load {}",
            started.elapsed(),
            load_average()
        );
    }
}
