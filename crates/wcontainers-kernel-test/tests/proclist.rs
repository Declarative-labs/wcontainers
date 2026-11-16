//! Listings of `/proc` and `/dev` compared with real ARM64 Linux. `/dev` must list the same entries in
//! the same order. `/proc` holds a chosen subset of Linux's entries (a declared difference, each
//! lookup of a missing one counted), so every entry listed there must be listed in the reference with
//! the same type, mode, owner, group and link count; the entries only the reference has are printed.

use std::collections::BTreeMap;
use std::path::Path;

use wcontainers_kernel_test::{guest, run_both};

/// The directory an entry names, from its directory and name.
fn named(dir: &str, name: &str) -> String {
    match name {
        "." => dir.to_owned(),
        ".." => match dir.rsplit_once('/') {
            Some(("", _)) | None => "/".to_owned(),
            Some((parent, _)) => parent.to_owned(),
        },
        _ => format!("{dir}/{name}"),
    }
}

/// Each line's directory and name, and the rest of it. The link counts of `/proc` and `/` are left
/// out: Linux counts every process on the machine in `/proc`'s (`proc_root_getattr`), and `/` is the
/// root file system, overlayfs in the reference and tmpfs here until the file backend exists.
fn entries(output: &[u8]) -> BTreeMap<(String, String), String> {
    listed(output).into_iter().collect()
}

/// The entries in the order they were listed.
fn listed(output: &[u8]) -> Vec<((String, String), String)> {
    String::from_utf8_lossy(output)
        .lines()
        .map(|line| {
            let mut parts = line.splitn(3, ' ');
            let dir = parts.next().unwrap_or_default().to_owned();
            let name = parts.next().unwrap_or_default().to_owned();
            let mut rest = parts.next().unwrap_or_default().to_owned();
            if matches!(named(&dir, &name).as_str(), "/proc" | "/")
                && let Some((kept, _)) = rest.rsplit_once(' ')
            {
                rest = kept.to_owned();
            }
            ((dir, name), rest)
        })
        .collect()
}

#[test]
fn device_listings_match_the_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = guest::build("proclist", &[], &out);
    let (ours, theirs) = run_both(&program, &["/dev", "/dev/pts", "/dev/shm"]);
    assert_eq!(ours.outcome.exit_code, theirs.exit_code);
    let show = |entries: Vec<((String, String), String)>| {
        entries
            .into_iter()
            .map(|((dir, name), rest)| format!("{dir} {name} {rest}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(
        show(listed(&ours.outcome.stdout)),
        show(listed(&theirs.stdout))
    );
}

fn compare(dirs: &[&str]) {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = guest::build("proclist", &[], &out);
    let (ours, theirs) = run_both(&program, dirs);
    assert_eq!(ours.outcome.exit_code, theirs.exit_code);
    let our_entries = entries(&ours.outcome.stdout);
    let their_entries = entries(&theirs.stdout);
    assert!(!our_entries.is_empty());
    let differences: Vec<String> = our_entries
        .iter()
        .filter(|(key, rest)| their_entries.get(*key) != Some(*rest))
        .map(|((dir, name), rest)| {
            format!(
                "{dir} {name}: ours {rest:?} theirs {:?}",
                their_entries.get(&(dir.clone(), name.clone()))
            )
        })
        .collect();
    let only_theirs: Vec<String> = their_entries
        .keys()
        .filter(|key| !our_entries.contains_key(*key))
        .map(|(dir, name)| format!("{dir}/{name}"))
        .collect();
    println!(
        "only in the reference ({}): {}",
        only_theirs.len(),
        only_theirs.join(" ")
    );
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}

#[test]
fn listings_are_a_subset_of_the_reference() {
    compare(&[
        "/proc",
        "/proc/self",
        "/proc/1",
        "/proc/self/fd",
        "/proc/sys",
        "/proc/sys/kernel",
        "/proc/sys/kernel/random",
        "/proc/sys/fs",
        "/proc/sys/vm",
    ]);
}

#[test]
#[ignore = "known failure: /proc/<pid>/task is not built yet (it comes with threads, K3)"]
fn thread_listings_are_a_subset_of_the_reference() {
    compare(&["/proc/self/task", "/proc/self/task/1"]);
}
