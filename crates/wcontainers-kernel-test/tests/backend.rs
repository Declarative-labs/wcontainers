//! Files on the backend: fuse-backend-rs's passthrough server, reached by our kernel through its
//! one backend hook and by the reference through Linux's own FUSE.

use std::path::Path;

use wcontainers_kernel_test::{backend, driver, guest, image, reference};

/// Where both mount the backend, and where the program starts.
const AT: &str = "/data";

/// Runs a guest program in a container with the backend at [`AT`], on our kernel under each
/// schedule seed and in the reference, and checks that both print the same and end the same way.
fn same_on_backend(name: &str, injections: driver::Injections) -> Vec<driver::Run> {
    same_on(name, injections, false)
}

/// As [`same_on_backend`], with the backend opening files for the page cache if `cached`.
fn same_on(name: &str, injections: driver::Injections, cached: bool) -> Vec<driver::Run> {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = guest::build(name, &[], &out);
    let theirs = backend::reference_run(&backend::out(), &program, AT, &[], cached);
    let mut files = image::reference_image().to_vec();
    files.extend(image::host_directory(
        program.parent().expect("a directory"),
        "wc",
    ));
    let path = format!("/wc/{name}");
    let mut runs = Vec::new();
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let container = driver::Container {
            files: &files,
            path: &path,
            arguments: &[&path],
            environment: reference::ENVIRONMENT,
            working_directory: AT,
            backend: Some(AT),
            cached,
        };
        let ours =
            driver::run_injected(&container, seed, injections).expect("an outcome on our kernel");
        let ours_text = String::from_utf8_lossy(&ours.outcome.stdout).into_owned();
        let theirs_text = String::from_utf8_lossy(&theirs.stdout).into_owned();
        let differences: Vec<String> = ours_text
            .lines()
            .zip(theirs_text.lines())
            .filter(|(our, their)| our != their)
            .map(|(our, their)| format!("seed {seed}: ours {our:?}\ntheirs {their:?}"))
            .collect();
        assert!(differences.is_empty(), "{}", differences.join("\n"));
        assert_eq!(ours.outcome, theirs, "seed {seed}");
        runs.push(ours);
    }
    runs
}

#[test]
fn file_calls_match_reference() {
    for run in same_on_backend("backendfiles", driver::Injections::default()) {
        assert_warned_only(&run, &[]);
        let messages: u64 = run
            .counters
            .iter()
            .filter(|counter| counter.reason == wcontainers_kernel::reason::BACKEND_MESSAGE)
            .map(|counter| counter.count)
            .sum();
        assert!(messages > 0, "no message reached the backend");
    }
}

/// Extended attributes and fallocate on backend files.
#[test]
fn attributes_and_allocation_match_reference() {
    for run in same_on_backend("backendattrs", driver::Injections::default()) {
        assert_warned_only(&run, &[]);
    }
}

/// File capabilities set and read back with getxattr on a backend file (the reference answers
/// EINVAL to the reading).
#[test]
fn file_capabilities_match_reference() {
    for run in same_on_backend("backendcaps", driver::Injections::default()) {
        assert_warned_only(&run, &[]);
    }
}

/// Processes and threads sharing one open file's position while their calls wait for the backend,
/// also saved and brought back.
#[test]
fn shared_positions_match_reference() {
    let injections = driver::Injections {
        saves: 4,
        ..Default::default()
    };
    for run in same_on_backend("sharedpos", injections) {
        assert_warned_only(&run, &[]);
    }
}

/// The same with the backend opening files for the page cache, where reads ask for attributes too.
#[test]
fn cached_shared_positions_match_reference() {
    let injections = driver::Injections {
        saves: 4,
        ..Default::default()
    };
    for run in same_on("sharedpos", injections, true) {
        assert_warned_only(&run, &[]);
    }
}

/// Calls waiting for the backend while another thread changes the descriptor or the path they
/// named, and a thread spinning while another waits for the backend: each call acts once, on what
/// it named when it started, and the spinning thread gives way when an answer comes.
#[test]
fn arguments_taken_once_match_reference() {
    let injections = driver::Injections {
        saves: 2,
        ..Default::default()
    };
    for run in same_on_backend("backendrace", injections) {
        assert_warned_only(&run, &[]);
    }
}

/// Listing a directory while removing from it, where the entries each answer holds show, and
/// O_TMPFILE's checks of the directory.
#[test]
fn listings_match_reference() {
    for cached in [false, true] {
        for run in same_on("backendlisting", driver::Injections::default(), cached) {
            assert_warned_only(&run, &[wcontainers_kernel::reason::FLAG_NOT_BUILT]);
        }
    }
}

/// Appends while another thread truncates the file: the inode is held across each.
#[test]
fn truncations_and_appends_match_reference() {
    let injections = driver::Injections {
        saves: 2,
        ..Default::default()
    };
    for run in same_on_backend("backendlocks", injections) {
        assert_warned_only(&run, &[]);
    }
}

/// sendfile, splice, tee and copy_file_range with backend files, then fadvise, readahead and
/// cachestat, on both kinds of backend.
#[test]
fn transfers_match_reference() {
    for cached in [false, true] {
        for run in same_on("backendtransfer", driver::Injections::default(), cached) {
            assert_warned_only(&run, &[]);
        }
    }
}

/// Attributes and access control lists read as root and as another user, on both kinds of backend.
#[test]
fn attributes_read_as_a_user_match_reference() {
    for cached in [false, true] {
        for run in same_on("backendacluser", driver::Injections::default(), cached) {
            assert_warned_only(&run, &[]);
        }
    }
}

/// Creating through links to missing files, and extended attributes of named pipes.
#[test]
fn special_files_match_reference() {
    for run in same_on_backend("backendspecial", driver::Injections::default()) {
        assert_warned_only(&run, &[]);
    }
}

/// Private and shared mappings of backend files, also saved and brought back. The first backend
/// opens files for direct reads and writes, so Linux refuses shared mappings of them.
#[test]
fn file_mappings_match_reference() {
    let injections = driver::Injections {
        saves: 4,
        ..Default::default()
    };
    for run in same_on_backend("backendmap", injections) {
        assert_warned_only(&run, &[]);
    }
}

/// The same with the backend opening files for the page cache, where shared mappings are made.
#[test]
fn cached_file_mappings_match_reference() {
    let injections = driver::Injections {
        saves: 4,
        ..Default::default()
    };
    for run in same_on("backendmap", injections, true) {
        assert_only_written_whole(&run);
    }
}

/// Every way a mapping, a read and a write of a file the backend opens for the page cache meet:
/// shared writes reaching the backend at msync, unmapping and exit, truncation and growth, private
/// pages dropped and mappings grown.
#[test]
fn cached_mappings_share_pages_as_reference() {
    let injections = driver::Injections {
        saves: 6,
        ..Default::default()
    };
    for run in same_on("backendshared", injections, true) {
        assert_only_written_whole(&run);
    }
}

/// The only warnings a run with shared mappings of backend files gives: their pages written back
/// whole, and the syndrome of a fault past a file's end.
fn assert_only_written_whole(run: &driver::Run) {
    use wcontainers_kernel::reason;
    assert_warned_only(
        run,
        &[
            reason::BACKEND_PAGES_WRITTEN_WHOLE,
            reason::FAULT_SYNDROME_APPROXIMATED,
        ],
    );
}

/// Checks that a run warned of nothing but `allowed` and of backend files read whole when mapped.
fn assert_warned_only(run: &driver::Run, allowed: &[&str]) {
    let others: Vec<_> = run
        .warnings
        .iter()
        .filter(|warning| {
            warning.reason != wcontainers_kernel::reason::BACKEND_READ_WHOLE
                && !allowed.contains(&warning.reason.as_str())
        })
        .collect();
    assert!(others.is_empty(), "{others:?}");
}

/// Saved and brought back at moments drawn from each seed, with files open on the backend and
/// answers from it on their way: the backend and what it owes go on with the copy.
#[test]
fn file_calls_survive_saves() {
    let injections = driver::Injections {
        saves: 12,
        ..Default::default()
    };
    for run in same_on_backend("backendfiles", injections) {
        assert_warned_only(&run, &[]);
        assert!(run.saved > 0);
    }
}

/// Notices the backend sends unasked, of every kind and about nodes the kernel holds or not, change
/// nothing the program sees, and none is left unhandled. Every kind reaches the runs, each
/// RETRIEVE is answered, and those cut short are refused.
#[test]
fn backend_notices_are_handled() {
    use fuse_abi::linux::fuse_notify_code::*;
    use wcontainers_kernel::reason;
    let injections = driver::Injections {
        notices: 400,
        ..Default::default()
    };
    let mut handled = std::collections::BTreeMap::<u64, u64>::new();
    let mut replies = 0;
    let mut refused = 0;
    for run in same_on_backend("backendfiles", injections) {
        assert_warned_only(&run, &[reason::BACKEND_NOTICE_REFUSED]);
        for counter in &run.counters {
            if counter.reason == reason::BACKEND_NOTICE_REFUSED {
                refused += counter.count;
            }
            if counter.reason == reason::BACKEND_NOTICE {
                *handled.entry(counter.detail).or_default() += counter.count;
            }
            if counter.reason == reason::BACKEND_MESSAGE
                && counter.detail == fuse_abi::linux::fuse_opcode::FUSE_NOTIFY_REPLY as u64
            {
                replies += counter.count;
            }
        }
    }
    eprintln!("notices handled by code: {handled:?}, refused {refused}");
    assert!(refused > 0, "no notice cut short was refused");
    for code in [
        FUSE_NOTIFY_POLL,
        FUSE_NOTIFY_INVAL_INODE,
        FUSE_NOTIFY_INVAL_ENTRY,
        FUSE_NOTIFY_STORE,
        FUSE_NOTIFY_RETRIEVE,
        FUSE_NOTIFY_DELETE,
    ] {
        assert!(
            handled.get(&(code as u64)).is_some_and(|count| *count > 0),
            "{code:?}"
        );
    }
    assert_eq!(Some(&replies), handled.get(&(FUSE_NOTIFY_RETRIEVE as u64)));
}

/// The messages each kind of file call sends to the backend (every call goes there in phase one;
/// this is what the cache will be measured against). Prints them; run with `--nocapture`.
#[test]
fn messages_per_call() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = guest::build("backendcosts", &[], &out);
    let mut files = image::reference_image().to_vec();
    files.extend(image::host_directory(
        program.parent().expect("a directory"),
        "wc",
    ));
    let count = |kind: &str| -> std::collections::BTreeMap<u64, u64> {
        let container = driver::Container {
            files: &files,
            path: "/wc/backendcosts",
            arguments: &["/wc/backendcosts", kind],
            environment: reference::ENVIRONMENT,
            working_directory: AT,
            backend: Some(AT),
            cached: false,
        };
        let run = driver::run(&container, 1).expect("an outcome on our kernel");
        assert_eq!(run.outcome.stdout, format!("{kind} done\n").into_bytes());
        run.counters
            .iter()
            .filter(|counter| counter.reason == wcontainers_kernel::reason::BACKEND_MESSAGE)
            .map(|counter| (counter.detail, counter.count))
            .collect()
    };
    let none = count("none");
    for kind in [
        "stat",
        "fstat",
        "open-close",
        "read",
        "write",
        "list",
        "create-unlink",
        "mkdir-rmdir",
        "rename",
    ] {
        let counts = count(kind);
        let per_call: Vec<String> = counts
            .iter()
            .filter_map(|(opcode, total)| {
                let extra = total.saturating_sub(none.get(opcode).copied().unwrap_or(0));
                (extra > 0).then(|| format!("opcode {opcode}: {:.2}", extra as f64 / 100.0))
            })
            .collect();
        eprintln!("{kind}: {}", per_call.join(", "));
        assert!(!per_call.is_empty(), "{kind} sent nothing to the backend");
    }
}
