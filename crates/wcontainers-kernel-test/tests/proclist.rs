//! Listings of `/proc` and `/dev` compared with real ARM64 Linux. `/dev` must list the same entries in
//! the same order. `/proc` holds a chosen subset of Linux's entries (a declared difference, each
//! lookup of a missing one counted), so the entries listed there must be those of the reference's
//! listing that we build, in its order, each with the same position after it (d_off), type, mode,
//! owner, group and link count; the entries only the reference has are printed.

use std::collections::BTreeSet;
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

/// The entries in the order they were listed: each line's directory and name, and the rest of it.
/// The link counts of `/proc` and `/` are left out: Linux counts every process on the machine in
/// `/proc`'s (`proc_root_getattr`), and `/` is the root file system, overlayfs in the reference and
/// tmpfs here until the file backend exists.
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
    let our_entries = listed(&ours.outcome.stdout);
    let their_entries = listed(&theirs.stdout);
    assert!(!our_entries.is_empty());
    let ours_named: BTreeSet<_> = our_entries.iter().map(|(key, _)| key.clone()).collect();
    let (theirs_built, theirs_only): (Vec<_>, Vec<_>) = their_entries
        .into_iter()
        .partition(|(key, _)| ours_named.contains(key));
    let show = |entries: &[((String, String), String)]| -> Vec<String> {
        entries
            .iter()
            .map(|((dir, name), rest)| format!("{dir} {name} {rest}"))
            .collect()
    };
    let (ours_shown, theirs_shown) = (show(&our_entries), show(&theirs_built));
    let mut differences: Vec<String> = ours_shown
        .iter()
        .zip(&theirs_shown)
        .filter(|(our, their)| our != their)
        .map(|(our, their)| format!("ours {our:?}\ntheirs {their:?}"))
        .collect();
    if ours_shown.len() != theirs_shown.len() {
        differences.push(format!(
            "ours lists {} entries the reference has, of {}",
            theirs_shown.len(),
            ours_shown.len()
        ));
    }
    let only_theirs: Vec<String> = theirs_only
        .iter()
        .map(|((dir, name), _)| format!("{dir}/{name}"))
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
        "/proc/sys/net",
        "/proc/sys/net/core",
    ]);
}

#[test]
fn thread_listings_are_a_subset_of_the_reference() {
    compare(&["/proc/self/task", "/proc/self/task/1"]);
}
