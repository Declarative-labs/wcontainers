//! The baseline and its history: new literals, lines no longer needed, hidden literals, growth (also
//! through a merge of a side branch, and by deleting and re-adding the file), marks, init, and two
//! branches that shrink it in different places.

mod common;

use std::path::{Path, PathBuf};

use common::{append, check, commit_all, git, replace, repository};
use wcontainers_literals::{
    BASELINE_FILE, DELETE_LINE, DO, FIELD_SEPARATOR, GROWN_MARK, HIDDEN_MARK, Kind,
    LOCATION_SEPARATOR, List, MANIFEST, MARK_WORD, MARKS_HEADING, NEW_MARK, ORDER_MARK, PolicyKind,
    RULES_SOURCE, STALE_MARK, WORKING_TREE, policy_line,
};

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
    [
        List::Code.name(),
        LIB.into(),
        Kind::Int.name(),
        item.into(),
        literal.into(),
    ]
    .join(&FIELD_SEPARATOR.to_string())
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
    assert_eq!(out.marks, [format!("{MARKS_HEADING}0")]);
}

#[test]
fn a_new_literal_fails_naming_its_line() {
    let root = started("new-literal");
    append(&root, LIB, "\npub fn five(x: u64) -> u64 { x * 31337 }\n");
    let out = check(&root);
    assert!(!out.clean);
    let new = lines_starting(&out, NEW_MARK);
    assert_eq!(new.len(), 1, "{}", out.report());
    assert!(
        new[0].starts_with(&format!(
            "{NEW_MARK}{}{FIELD_SEPARATOR}{LIB}{LOCATION_SEPARATOR}9{LOCATION_SEPARATOR}",
            List::Code.name()
        )) && new[0].contains(&format!("{FIELD_SEPARATOR}31337"))
            && new[0].contains(&format!("{FIELD_SEPARATOR}{DO}")),
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
        lines_starting(&out, &format!("{NEW_MARK}{}", List::Code.name()))
            .iter()
            .any(|l| l.contains(&format!("{FIELD_SEPARATOR}one{FIELD_SEPARATOR}4096"))),
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
        lines_starting(&out, STALE_MARK),
        [format!(
            "{STALE_MARK}{}{FIELD_SEPARATOR}{DO}{DELETE_LINE}{BASELINE_FILE}",
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
        lines_starting(&out, GROWN_MARK)
            .iter()
            .any(|l| l.contains("31337") && l.contains(WORKING_TREE)),
        "{}",
        out.report()
    );
    commit_all(&root, "grew the baseline");
    let out = check(&root);
    assert!(!out.clean);
    assert!(
        lines_starting(&out, GROWN_MARK)
            .iter()
            .any(|l| l.contains("31337") && l.contains("added by")),
        "{}",
        out.report()
    );
}

#[test]
fn a_new_policy_line_is_growth() {
    let root = started("policy-growth");
    let skip = policy_line(PolicyKind::Skip, "crates/a");
    set_baseline(&root, &with(&baseline(&root), &skip));
    commit_all(&root, "skipped a crate");
    let out = check(&root);
    assert!(
        lines_starting(&out, &format!("{GROWN_MARK}{skip}")).len() == 1,
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
        lines_starting(&out, GROWN_MARK).len(),
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
        lines_starting(&out, GROWN_MARK)
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
        lines_starting(&out, HIDDEN_MARK)
            .iter()
            .any(|l| l.contains("4096") && l.contains("now in the test list")),
        "{}",
        out.report()
    );
    set_baseline(&root, &without(&baseline(&root), &key("one", "4096")));
    commit_all(&root, "hid it and dropped its line");
    let out = check(&root);
    assert!(
        lines_starting(&out, HIDDEN_MARK)
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
            format!("{MARKS_HEADING}1"),
            format!(
                "{LIB}{LOCATION_SEPARATOR}3: {MARK_WORD} 77: the port the fixture's protocol fixes"
            )
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
            lines_starting(&out, &format!("{NEW_MARK}mark"))
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
        lines_starting(&out, &format!("{NEW_MARK}mark"))
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
    assert_eq!(
        lines_starting(&out, ORDER_MARK).len(),
        1,
        "{}",
        out.report()
    );
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
        lines_starting(&out, &format!("{NEW_MARK}{}", List::Code.name()))
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
        lines_starting(&check(&root), GROWN_MARK)
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

/// A side branch forked from before the baseline existed starts a baseline of its own: the baseline
/// then has two starts, which fails.
#[test]
fn a_side_branch_that_starts_its_own_baseline_fails() {
    let root = repository("fresh-side", FILES);
    git(&root, &["checkout", "-q", "-b", "side"]);
    git(&root, &["checkout", "-q", "main"]);
    wcontainers_literals::init(&root).expect("init");
    commit_all(&root, "baseline");
    git(&root, &["checkout", "-q", "side"]);
    append(&root, LIB, "\npub fn five(x: u64) -> u64 { x * 31337 }\n");
    wcontainers_literals::init(&root).expect("init on the side branch");
    commit_all(&root, "a baseline of its own");
    git(&root, &["checkout", "-q", "main"]);
    // Both sides added the file, so git asks; the side's copy is taken whole.
    let merge = common::git_output(&root, &["merge", "-q", "--no-edit", "side"]);
    assert!(!merge.status.success());
    git(&root, &["checkout", "--theirs", BASELINE_FILE]);
    commit_all(&root, "merged the side branch");
    started_twice(&root);
}

fn started_twice(root: &Path) {
    let out = check(root);
    assert!(
        lines_starting(&out, GROWN_MARK)
            .iter()
            .any(|l| l.contains("started more than once")),
        "{}",
        out.report()
    );
}

/// The same, with the track merged into the fresh start and then fast-forwarded to it, so that the
/// fresh start is on the track's own first-parent history.
#[test]
fn a_fresh_start_fast_forwarded_onto_the_track_fails() {
    let root = repository("fresh-forward", FILES);
    git(&root, &["branch", "before"]);
    git(&root, &["checkout", "-q", "-b", "track"]);
    wcontainers_literals::init(&root).expect("init");
    commit_all(&root, "baseline");
    git(&root, &["checkout", "-q", "-b", "tmp", "before"]);
    append(&root, LIB, "\npub fn five(x: u64) -> u64 { x * 31337 }\n");
    wcontainers_literals::init(&root).expect("init on the fork");
    commit_all(&root, "a larger baseline");
    let merge = common::git_output(&root, &["merge", "-q", "--no-edit", "track"]);
    assert!(!merge.status.success());
    git(&root, &["checkout", "--ours", BASELINE_FILE]);
    commit_all(&root, "merged the track");
    git(&root, &["checkout", "-q", "track"]);
    git(&root, &["merge", "-q", "--ff-only", "tmp"]);
    started_twice(&root);
}

/// Marking the baseline binary would hide its diffs; the history is read as text and the attribute
/// itself fails.
#[test]
fn growth_behind_a_binary_attribute_fails() {
    let root = started("binary");
    append(&root, LIB, "\npub fn five(x: u64) -> u64 { x * 31337 }\n");
    set_baseline(&root, &with(&baseline(&root), &key("five", "31337")));
    std::fs::write(
        root.join(".gitattributes"),
        format!("{BASELINE_FILE} binary\n"),
    )
    .expect("write the attributes");
    commit_all(&root, "grew it behind an attribute");
    let grown = lines_starting(&check(&root), GROWN_MARK);
    assert!(grown.iter().any(|l| l.contains("31337")), "{grown:?}");
    assert!(grown.iter().any(|l| l.contains("attributes")), "{grown:?}");
}

/// Still gets through without an upstream (docs/literals.md): a fork from before the track adopted the
/// check merges the literals branch and runs init again, with a new literal, then takes the track in
/// keeping its own copy. With the track's upstream known, the moved start fails.
#[test]
fn a_second_adoption_is_seen_only_against_the_upstream() {
    let origin = repository("second-adoption", FILES);
    git(&origin, &["branch", "before"]);
    git(&origin, &["checkout", "-q", "-b", "literals"]);
    wcontainers_literals::init(&origin).expect("init");
    commit_all(&origin, "baseline of main");
    git(&origin, &["checkout", "-q", "-b", "track", "before"]);
    git(
        &origin,
        &["merge", "-q", "--no-ff", "--no-edit", "literals"],
    );
    wcontainers_literals::init(&origin).expect("the track's init");
    commit_all(&origin, "the track's baseline");
    git(&origin, &["checkout", "-q", "main"]);
    let clone = origin.with_file_name("second-adoption-clone");
    if clone.exists() {
        std::fs::remove_dir_all(&clone).expect("remove the old clone");
    }
    git(
        &origin,
        &[
            "clone",
            "-q",
            "-b",
            "track",
            ".",
            clone.to_str().expect("a path"),
        ],
    );
    let root = clone;
    git(&root, &["checkout", "-q", "-b", "tmp", "origin/before"]);
    append(&root, LIB, "\npub fn five(x: u64) -> u64 { x * 31337 }\n");
    commit_all(&root, "a literal before adopting");
    git(&root, &["merge", "-q", "--no-edit", "origin/literals"]);
    wcontainers_literals::init(&root).expect("a second init");
    commit_all(&root, "a second baseline");
    let merge = common::git_output(&root, &["merge", "-q", "--no-edit", "track"]);
    if !merge.status.success() {
        git(&root, &["checkout", "--ours", BASELINE_FILE]);
        commit_all(&root, "merged the track");
    }
    git(&root, &["checkout", "-q", "track"]);
    git(&root, &["merge", "-q", "--ff-only", "tmp"]);
    let out = check(&root);
    assert!(
        lines_starting(&out, GROWN_MARK)
            .iter()
            .any(|l| l.contains("upstream")),
        "{}",
        out.report()
    );
    git(&root, &["branch", "--unset-upstream"]);
    let out = check(&root);
    assert!(out.clean, "{}", out.report());
}

/// A side branch made before the check arrived adopts it with new literals and is merged into a track
/// that already adopted it: the merge brings lines from an init on its side, which the rules allow, so
/// the check passes; what shows is the list of changes that could add lines, which names the side's
/// init next to the track's own.
#[test]
fn a_side_adoption_merged_into_a_track_is_listed() {
    let root = repository("side-adoption", FILES);
    git(&root, &["branch", "before"]);
    git(&root, &["checkout", "-q", "-b", "literals"]);
    wcontainers_literals::init(&root).expect("init");
    commit_all(&root, "baseline of main");
    git(&root, &["checkout", "-q", "-b", "track", "before"]);
    git(&root, &["merge", "-q", "--no-ff", "--no-edit", "literals"]);
    wcontainers_literals::init(&root).expect("the track's init");
    commit_all(&root, "the track's baseline");
    let before = check(&root);
    git(&root, &["checkout", "-q", "-b", "side", "before"]);
    append(&root, LIB, "\npub fn five(x: u64) -> u64 { x * 5553 }\n");
    commit_all(&root, "a literal before adopting");
    git(&root, &["merge", "-q", "--no-ff", "--no-edit", "literals"]);
    wcontainers_literals::init(&root).expect("the side's init");
    commit_all(&root, "the side's baseline");
    let side_init = git(&root, &["describe", "--always", "--abbrev=12"]);
    git(&root, &["checkout", "-q", "track"]);
    git(&root, &["merge", "-q", "--no-edit", "side"]);
    let out = check(&root);
    assert!(out.clean, "{}", out.report());
    assert!(
        !before.summary.iter().any(|l| l.contains(side_init.trim())),
        "{}",
        before.report()
    );
    assert!(
        out.summary
            .iter()
            .any(|l| l.contains(side_init.trim()) && l.contains("arrived by a merge")),
        "{}",
        out.report()
    );
}

/// The checker's own crate, whose rules file counts the versions of the rules.
fn with_checker(versions: &[&str]) -> Vec<(String, String)> {
    let rules = Path::new(RULES_SOURCE);
    let manifest = rules
        .parent()
        .and_then(Path::parent)
        .expect("the checker's crate folder")
        .join(MANIFEST);
    let quoted: String = versions.iter().map(|v| format!("{v:?},")).collect();
    vec![
        (
            MANIFEST.into(),
            "[workspace]\nmembers = [\"crates/*\"]\n".into(),
        ),
        (
            "crates/a/Cargo.toml".into(),
            "[package]\nname = \"a\"\n".into(),
        ),
        (LIB.into(), CODE.into()),
        (
            manifest.to_string_lossy().into_owned(),
            format!("[package]\nname = {:?}\n", env!("CARGO_PKG_NAME")),
        ),
        (
            RULES_SOURCE.into(),
            format!("pub const RULES: &[&str] = &[{quoted}];\n"),
        ),
    ]
}

fn borrowed(files: &[(String, String)]) -> Vec<(&str, &str)> {
    files
        .iter()
        .map(|(p, t)| (p.as_str(), t.as_str()))
        .collect()
}

/// A new version of the rules lets the branch's next change of the baseline add lines for literals
/// that were already in the tree, also after a merge that brings it; each such change is listed. A
/// literal added with the new rules is growth, and so is any growth when only the checker's code
/// changes.
#[test]
fn new_rules_may_add_lines_for_literals_already_there() {
    let files = with_checker(&["first"]);
    let root = repository("new-rules", &borrowed(&files));
    git(&root, &["checkout", "-q", "-b", "literals"]);
    wcontainers_literals::init(&root).expect("init");
    commit_all(&root, "baseline of main");
    git(&root, &["checkout", "-q", "-b", "track"]);
    let five = "\npub fn five(x: u64) -> u64 { x * 6161 }\n";
    let six = "\npub fn six(x: u64) -> u64 { x * 6262 }\n";
    append(&root, LIB, five);
    set_baseline(&root, &without(&baseline(&root), &key("two", "77")));
    replace(&root, LIB, "x + 77", "x");
    commit_all(&root, "a literal the old rules let through, and a fix");

    // The checker's code changes without a new version: growth is still growth.
    git(&root, &["checkout", "-q", "literals"]);
    append(&root, RULES_SOURCE, "pub fn stricter() {}\n");
    commit_all(&root, "a change of the checker's code");
    git(&root, &["checkout", "-q", "track"]);
    git(&root, &["merge", "-q", "--no-edit", "literals"]);
    let merged = baseline(&root);
    set_baseline(&root, &with(&merged, &key("five", "6161")));
    let out = check(&root);
    assert_eq!(
        lines_starting(&out, GROWN_MARK).len(),
        1,
        "{}",
        out.report()
    );
    assert!(wcontainers_literals::init(&root).is_err());
    set_baseline(&root, &merged);

    git(&root, &["checkout", "-q", "literals"]);
    replace(&root, RULES_SOURCE, "\"first\",]", "\"first\",\"second\",]");
    commit_all(&root, "new rules");
    git(&root, &["checkout", "-q", "track"]);
    git(&root, &["merge", "-q", "--no-edit", "literals"]);
    append(&root, LIB, six);
    wcontainers_literals::init(&root).expect("init after new rules");
    let out = check(&root);
    let grown = lines_starting(&out, GROWN_MARK);
    assert!(
        grown.len() == 1 && grown[0].contains("6262") && grown[0].contains(WORKING_TREE),
        "{}",
        out.report()
    );
    commit_all(
        &root,
        "the baseline for the new rules, with a literal added with them",
    );
    let out = check(&root);
    let grown = lines_starting(&out, GROWN_MARK);
    assert!(
        grown.len() == 1 && grown[0].contains("6262"),
        "{}",
        out.report()
    );

    git(&root, &["reset", "-q", "--hard", "HEAD~1"]);
    wcontainers_literals::init(&root).expect("init after new rules");
    commit_all(&root, "the baseline for the new rules");
    let init = git(&root, &["describe", "--always", "--abbrev=12"]);
    let out = check(&root);
    assert!(out.clean, "{}", out.report());
    assert!(
        out.summary
            .iter()
            .any(|l| l.contains(init.trim()) && l.contains("new rules")),
        "{}",
        out.report()
    );
    assert!(wcontainers_literals::init(&root).is_err());
}

/// Replace refs are not read: a graft that turns a growth into the start hides nothing.
#[test]
fn a_graft_does_not_hide_growth() {
    let root = started("graft");
    append(&root, LIB, "\npub fn five(x: u64) -> u64 { x * 6363 }\n");
    set_baseline(&root, &with(&baseline(&root), &key("five", "6363")));
    commit_all(&root, "growth");
    let growth = git(&root, &["describe", "--always"]);
    git(&root, &["replace", "--graft", growth.trim()]);
    let out = check(&root);
    assert_eq!(
        lines_starting(&out, GROWN_MARK).len(),
        1,
        "{}",
        out.report()
    );
}
