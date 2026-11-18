//! The baseline and its history: new literals, lines no longer needed, hidden literals, growth (also
//! through a merge of a side branch, and by deleting and re-adding the file), marks, init, and two
//! branches that shrink it in different places.

mod common;

use std::path::{Path, PathBuf};

use common::{append, check, commit_all, git, replace, repository};
use wcontainers_literals::BASELINE_FILE;

const LIB: &str = "crates/a/src/lib.rs";
const CODE: &str = "pub fn one(x: u64) -> u64 { x * 4096 }

pub fn two(x: u64) -> u64 { x + 77 }

pub fn three(x: u64) -> u64 { x >> 12 }

pub fn four(x: u64) -> u64 { x % 1000 }
";
const FILES: &[(&str, &str)] = &[
    ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
    ("crates/a/Cargo.toml", "[package]\nname = \"a\"\n"),
    (LIB, CODE),
];

fn key(item: &str, literal: &str) -> String {
    format!("code\t{LIB}\tint\t{item}\t{literal}")
}

/// A repository with its baseline written by init and committed.
fn started(name: &str) -> PathBuf {
    let root = repository(name, FILES);
    wcontainers_literals::init(&root).expect("init");
    commit_all(&root, "baseline");
    root
}

fn baseline(root: &Path) -> String {
    std::fs::read_to_string(root.join(BASELINE_FILE)).expect("read the baseline")
}

fn set_baseline(root: &Path, text: &str) {
    std::fs::write(root.join(BASELINE_FILE), text).expect("write the baseline");
}

fn without(text: &str, line: &str) -> String {
    text.lines()
        .filter(|l| *l != line)
        .map(|l| format!("{l}\n"))
        .collect()
}

fn with(text: &str, line: &str) -> String {
    let mut lines: Vec<&str> = text.lines().chain([line]).collect();
    lines.sort();
    lines.iter().map(|l| format!("{l}\n")).collect()
}

fn lines_starting(out: &wcontainers_literals::Outcome, prefix: &str) -> Vec<String> {
    out.lines
        .iter()
        .filter(|l| l.starts_with(prefix))
        .cloned()
        .collect()
}

#[test]
fn init_writes_one_sorted_line_per_literal() {
    let root = started("init-writes");
    let expected: String = [
        key("four", "1000"),
        key("one", "4096"),
        key("three", "12"),
        key("two", "77"),
    ]
    .iter()
    .map(|k| format!("{k}\n"))
    .collect();
    assert_eq!(baseline(&root), expected);
    let out = check(&root);
    assert!(out.clean, "{}", out.report());
    assert_eq!(out.marks, ["marks: 0"]);
}

#[test]
fn a_new_literal_fails_naming_its_line() {
    let root = started("new-literal");
    append(&root, LIB, "\npub fn five(x: u64) -> u64 { x * 31337 }\n");
    let out = check(&root);
    assert!(!out.clean);
    let new = lines_starting(&out, "+ ");
    assert_eq!(new.len(), 1, "{}", out.report());
    assert!(
        new[0].starts_with(&format!("+ code\t{LIB}:9:"))
            && new[0].contains("\t31337")
            && new[0].contains("\tdo: "),
        "{}",
        new[0]
    );
}

#[test]
fn deleting_the_line_of_a_literal_still_there_fails() {
    let root = started("deleted-line");
    set_baseline(&root, &without(&baseline(&root), &key("one", "4096")));
    let out = check(&root);
    assert!(!out.clean);
    assert!(
        lines_starting(&out, "+ code")
            .iter()
            .any(|l| l.contains("\tone\t4096")),
        "{}",
        out.report()
    );
    commit_all(&root, "dropped a line");
    assert!(!check(&root).clean);
}

#[test]
fn a_fixed_literal_asks_for_its_line_to_go() {
    let root = started("fixed");
    replace(&root, LIB, "x * 4096", "x * PAGE");
    append(&root, LIB, "\nconst PAGE: u64 = 4096;\n");
    let out = check(&root);
    assert_eq!(
        lines_starting(&out, "- "),
        [format!(
            "- {}\tdo: delete this line from literals.baseline",
            key("one", "4096")
        )],
        "{}",
        out.report()
    );
    set_baseline(&root, &without(&baseline(&root), &key("one", "4096")));
    let out = check(&root);
    assert!(out.clean, "{}", out.report());
}

#[test]
fn growing_the_baseline_fails_before_and_after_commit() {
    let root = started("grown");
    append(&root, LIB, "\npub fn five(x: u64) -> u64 { x * 31337 }\n");
    set_baseline(&root, &with(&baseline(&root), &key("five", "31337")));
    let out = check(&root);
    assert!(
        lines_starting(&out, "> ")
            .iter()
            .any(|l| l.contains("31337") && l.contains("the working tree")),
        "{}",
        out.report()
    );
    commit_all(&root, "grew the baseline");
    let out = check(&root);
    assert!(!out.clean);
    assert!(
        lines_starting(&out, "> ")
            .iter()
            .any(|l| l.contains("31337") && l.contains("added by")),
        "{}",
        out.report()
    );
}

#[test]
fn a_new_policy_line_is_growth() {
    let root = started("policy-growth");
    set_baseline(&root, &with(&baseline(&root), "policy\tskip\tcrates/a"));
    commit_all(&root, "skipped a crate");
    let out = check(&root);
    assert!(
        lines_starting(&out, "> policy\tskip\tcrates/a").len() == 1,
        "{}",
        out.report()
    );
}

#[test]
fn deleting_and_re_adding_the_baseline_is_growth() {
    let root = started("recreated");
    let text = baseline(&root);
    std::fs::remove_file(root.join(BASELINE_FILE)).expect("delete");
    commit_all(&root, "deleted the baseline");
    set_baseline(&root, &text);
    commit_all(&root, "re-added it");
    let out = check(&root);
    assert_eq!(
        lines_starting(&out, "> ").len(),
        text.lines().count(),
        "{}",
        out.report()
    );
    assert!(wcontainers_literals::init(&root).is_err());
}

#[test]
fn growth_on_a_merged_side_branch_is_seen() {
    let root = started("side-branch");
    git(&root, &["checkout", "-q", "-b", "side"]);
    append(&root, LIB, "\npub fn five(x: u64) -> u64 { x * 31337 }\n");
    set_baseline(&root, &with(&baseline(&root), &key("five", "31337")));
    commit_all(&root, "grew on a side branch");
    git(&root, &["checkout", "-q", "main"]);
    git(&root, &["merge", "-q", "--no-ff", "--no-edit", "side"]);
    let out = check(&root);
    assert!(
        lines_starting(&out, "> ")
            .iter()
            .any(|l| l.contains("31337")),
        "{}",
        out.report()
    );
}

#[test]
fn hiding_a_literal_under_cfg_test_fails() {
    let root = started("hidden");
    replace(&root, LIB, "pub fn one", "#[cfg(test)]\npub fn one");
    let out = check(&root);
    assert!(
        lines_starting(&out, "~ ")
            .iter()
            .any(|l| l.contains("4096") && l.contains("now in the test list")),
        "{}",
        out.report()
    );
    set_baseline(&root, &without(&baseline(&root), &key("one", "4096")));
    commit_all(&root, "hid it and dropped its line");
    let out = check(&root);
    assert!(
        lines_starting(&out, "~ ")
            .iter()
            .any(|l| l.contains("4096") && l.contains("removed from the baseline")),
        "{}",
        out.report()
    );
}

#[test]
fn a_mark_with_a_reason_passes_and_is_listed() {
    let root = started("mark");
    replace(
        &root,
        LIB,
        "pub fn two",
        "// standalone 77: the port the fixture's protocol fixes\npub fn two",
    );
    set_baseline(&root, &without(&baseline(&root), &key("two", "77")));
    let out = check(&root);
    assert!(out.clean, "{}", out.report());
    assert_eq!(
        out.marks,
        [
            "marks: 1",
            &format!("{LIB}:3: standalone 77: the port the fixture's protocol fixes")
        ]
    );
}

#[test]
fn a_bad_mark_fails() {
    for (name, mark, why) in [
        ("mark-no-reason", "// standalone 77\n", "needs a reason"),
        (
            "mark-empty-reason",
            "// standalone 77:   \n",
            "needs a reason",
        ),
        (
            "mark-text",
            "// standalone \"name\": a string\n",
            "names one number",
        ),
        (
            "mark-nothing",
            "// standalone 55: wrong number\n",
            "no unnamed number 55",
        ),
    ] {
        let root = started(name);
        replace(&root, LIB, "pub fn two", &format!("{mark}pub fn two"));
        let out = check(&root);
        assert!(
            lines_starting(&out, "+ mark")
                .iter()
                .any(|l| l.contains(why)),
            "{name}: {}",
            out.report()
        );
    }
}

#[test]
fn a_mark_on_a_number_that_has_a_name_fails() {
    let root = started("mark-named");
    append(&root, LIB, "\npub const PAGE_SIZE: u64 = 4096;\n");
    replace(
        &root,
        LIB,
        "pub fn one",
        "// standalone 4096: a page\npub fn one",
    );
    let out = check(&root);
    assert!(
        lines_starting(&out, "+ mark")
            .iter()
            .any(|l| l.contains("has a name, PAGE_SIZE")),
        "{}",
        out.report()
    );
}

#[test]
fn an_unsorted_baseline_fails() {
    let root = started("unsorted");
    let text = baseline(&root);
    let mut lines: Vec<&str> = text.lines().collect();
    lines.reverse();
    set_baseline(
        &root,
        &lines.iter().map(|l| format!("{l}\n")).collect::<String>(),
    );
    let out = check(&root);
    assert_eq!(lines_starting(&out, "! ").len(), 1, "{}", out.report());
}

#[test]
fn init_refuses_once_the_baseline_has_started() {
    let root = started("init-twice");
    let error = wcontainers_literals::init(&root)
        .err()
        .expect("init refuses");
    assert!(error.contains("only shrinks"), "{error}");
}

#[test]
fn two_branches_that_fix_different_literals_merge_cleanly() {
    let root = started("two-branches");
    git(&root, &["checkout", "-q", "-b", "fixes-four"]);
    replace(
        &root,
        LIB,
        "pub fn four(x: u64) -> u64 { x % 1000 }",
        "const THOUSAND: u64 = 1000;\npub fn four(x: u64) -> u64 { x % THOUSAND }",
    );
    set_baseline(&root, &without(&baseline(&root), &key("four", "1000")));
    commit_all(&root, "named the thousand");
    git(&root, &["checkout", "-q", "-b", "fixes-three", "main"]);
    replace(
        &root,
        LIB,
        "pub fn three(x: u64) -> u64 { x >> 12 }",
        "const SHIFT: u32 = 12;\npub fn three(x: u64) -> u64 { x >> SHIFT }",
    );
    set_baseline(&root, &without(&baseline(&root), &key("three", "12")));
    commit_all(&root, "named the shift");
    assert!(check(&root).clean);
    git(&root, &["checkout", "-q", "fixes-four"]);
    assert!(check(&root).clean);
    git(&root, &["merge", "-q", "--no-edit", "fixes-three"]);
    let out = check(&root);
    assert!(out.clean, "{}", out.report());
    assert_eq!(
        baseline(&root),
        format!("{}\n{}\n", key("one", "4096"), key("two", "77"))
    );
}

#[test]
fn a_track_adopts_the_check_by_merge_then_init_once() {
    // The track's own branch, before the check exists.
    let root = repository("adopt", FILES);
    git(&root, &["checkout", "-q", "-b", "literals"]);
    wcontainers_literals::init(&root).expect("init on the literals branch");
    commit_all(&root, "baseline of main");
    git(&root, &["checkout", "-q", "main"]);
    git(&root, &["checkout", "-q", "-b", "track"]);
    append(&root, LIB, "\npub fn five(x: u64) -> u64 { x * 31337 }\n");
    commit_all(&root, "the track's own work");
    git(&root, &["merge", "-q", "--no-edit", "literals"]);
    let out = check(&root);
    assert!(
        lines_starting(&out, "+ code")
            .iter()
            .any(|l| l.contains("31337")),
        "{}",
        out.report()
    );
    wcontainers_literals::init(&root).expect("init after the baseline arrived");
    commit_all(&root, "the track's baseline");
    let out = check(&root);
    assert!(out.clean, "{}", out.report());
    assert!(wcontainers_literals::init(&root).is_err());
    append(&root, LIB, "\npub fn six(x: u64) -> u64 { x * 4242 }\n");
    set_baseline(&root, &with(&baseline(&root), &key("six", "4242")));
    commit_all(&root, "grew it");
    assert!(
        lines_starting(&check(&root), "> ")
            .iter()
            .any(|l| l.contains("4242"))
    );
}

#[test]
fn a_merge_brings_the_other_side_s_accepted_lines() {
    let root = repository("merge-brings", FILES);
    git(&root, &["checkout", "-q", "-b", "literals"]);
    wcontainers_literals::init(&root).expect("init");
    commit_all(&root, "baseline of main");
    // One track adopts and accepts its own literal.
    git(&root, &["checkout", "-q", "-b", "kernel", "main"]);
    append(
        &root,
        "crates/a/src/kernel.rs",
        "pub fn k(x: u64) -> u64 { x * 5555 }\n",
    );
    append(&root, LIB, "pub mod kernel;\n");
    commit_all(&root, "kernel work");
    git(&root, &["merge", "-q", "--no-edit", "literals"]);
    wcontainers_literals::init(&root).expect("kernel init");
    commit_all(&root, "kernel baseline");
    // Another adopts too, then merges the first.
    git(&root, &["checkout", "-q", "-b", "runtime", "main"]);
    git(&root, &["merge", "-q", "--no-edit", "literals"]);
    assert!(check(&root).clean);
    git(&root, &["merge", "-q", "--no-edit", "kernel"]);
    let out = check(&root);
    assert!(out.clean, "{}", out.report());
}
