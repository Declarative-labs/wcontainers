# No magic literals: the rule and its check

The owner's rule (2026-11-18): no hard-coded magic numbers, magic literals, magic anything. A value comes from one clear place: a labelled constant referenced everywhere, better a standard library that has it, better still derived from some structure (the size of a structure). A hard-coded constant is fine, but it is never redefined. This must never be violated.

The crate `wcontainers-literals` checks it on the whole workspace, as part of the ordinary test run, against `literals.baseline` at the repository root: the findings main had when the check arrived, which may only shrink.

## The rule

1. -1, 0, 1 and 2 used as plain arithmetic (nothing, the first, the next, the previous, half, double) are free. Where one of them carries a meaning (given to or compared with a user or group id, a descriptor, a process id, an exit status, an errno, a signal) it is gated like any other number.
2. Every other number, and every string a program interprets (a key, a path, a name, a protocol word), is derived, imported from the standard library or an established crate, or is one named constant defined once. A fact defined twice anywhere is a violation, also under another name.
3. Text shown to a person is free unless it holds a number, a path, or a name that has a constant.
4. A number that truly stands alone may stay where it is if it carries a mark with a one-line reason (below). A mark is for a number nobody could name better, never a way round a number that has a name.
5. Test inputs and expected results are free, but a test that writes a value the code has a name for is a violation.

## How the check reads the rule

Every literal in a Rust file lands in one list. These lists fail the check:
- `code`: a literal in non-test code that is not one of the free numbers used as arithmetic.
- `redefinition`: a constant with the same name and value as another one in the workspace or in linux-raw-sys (aarch64 and x86_64, the versions in Cargo.lock). Reported on the copy: the owner is linux-raw-sys, the crate the other depends on, or, between unrelated crates, the crate more crates depend on.
- `restated`: a test literal whose value is a constant its crate can name (numbers above 255, text of two or more characters); a literal inside a constant whose value and a specific word of its name match an upstream constant (`LOCAL_PAGE = 4096` against `PAGE_SIZE`); a message or attribute string that writes out, word for word, the value of a string constant its crate can reach (`"set WC_REFERENCE_HOST first"` where `HOST_VARIABLE` holds that name, `#[serde(rename = "st_uid")]` where a constant holds `st_uid`). Only words that look like names count: with an upper-case letter, a digit, or one of `_ - . : /`.
- `role`: -1, 0, 1 or 2 given to, or compared with, a name whose words say uid, gid, fd, pid, tid, exit, status, stdin, stdout, stderr, errno or signal. The name is a variable, a field, a function with the types it belongs to (`ExitCode::from(2)`, but not the modules it sits in), the parts of a tuple bound to a tuple (`let (uid, gid) = (0, 0)`), or the function whose value it is (`fn stdout_fd() -> i32 { 1 }`, `return -1` in `fn pid`).
- `script`: a number other than -1, 0, 1, 2 in a shell or Python script (shell syntax like `2>&1`, `$1` and `[0-9]` left out; only a one-digit number next to `<` or `>` is read as a descriptor, so `4096>size` and `$((4096>>2))` are numbers).
- `unparsed`: a Rust file the parser cannot read, so nothing in it was checked.
- `mark`: a mark that cannot stand (below).

These lists are printed by `list` but never fail: `const` (the one definition of a named constant), `test`, `generated`, `free` (the free numbers as arithmetic), `message` (text shown to a person: format strings, panics, errors, log macros, `expect`, `Display`; and the strings of attributes, which Rust requires as literals), `marked`.

The name of a variable Cargo sets, given to `env!` or `option_env!` (`CARGO_*`, `OUT_DIR`), is left out: Cargo owns the name and the macros take only a literal. Any other variable name given to them is code.

Test code is a crate's `tests/`, `benches/` and `examples/` folders, modules declared under `#[cfg(test)]`, and items under `#[test]`, `#[bench]` or a cfg that holds only under test. Messages in tests are messages too. A message keeps its numbers and paths gated: `"needs 64 bytes"` and `"cannot open /etc/hosts"` fail, `"cannot read {path}: {e}"` passes. Free numbers in text count as no number.

## The mark

A mark is a line comment directly above the line that holds the number, naming the number exactly as written, with the reason after a colon:

```rust
// standalone 0.577: the damping the reference implementation uses, which has no name upstream
let damped = x * 0.577;
```

Several marks may stand one above the other for several numbers on the line below them. The comment must start the line (after indentation); `///` and `//!` are documentation, never marks; mark-like text inside a string is text.

Why this form: rustfmt never moves a comment that has a line of its own, while a comment at the end of a line can end up on another line than the number when rustfmt wraps an expression; and naming the number pins the mark to that one number, so a second number added to the line later is not covered by accident.

The check refuses (lists as `mark`, which fails) a mark:
- with no reason, or an empty one;
- that names text or anything else that is not a number;
- with no unnamed gated number of that exact text on the line below it (a stale or misplaced mark, or a mark in test code, which needs none);
- on a number its crate can already name: a constant of the same value, of any size, in the same crate or an upstream one (linux-raw-sys included). If the constant is a different fact that only shares the value, give the number its own name: that is always allowed;
- on a number another mark also covers, anywhere in the workspace: a number written twice is one fact, so it gets one name. Both marks fail.

Every accepted mark is printed by every run of the check, after a count (`marks: N`), with its place, number and reason, so a reviewer can judge each reason and marks cannot pile up unseen. The checker itself carries none.

## Running it

- One command, from anywhere in the workspace: `cargo run -q -p wcontainers-literals -- check`. Exit 0 means clean.
- What is scanned: every Rust file and script git lists (committed, or new and not ignored), except the workspace manifest's `exclude` folders and `skip` policy lines; and every module a scanned file places with `#[path]` or declares under a test-only attribute, wherever that file lies (a hidden, ignored, excluded or skipped folder). Build output is left out by the repository's own ignore rules.
- In the ordinary test run: `cargo test -p wcontainers-literals` (alone: `cargo test -p wcontainers-literals --test workspace`), and so in `cargo test --workspace`.
- `cargo run -q -p wcontainers-literals -- list` prints every finding of every list, with the constants of the same value in a note (`# = PAGE_SIZE (crates/...)`): the work list for fixing.
- On the whole workspace (292 Rust files, 6 scripts) the check takes about 2 seconds. Measured on the Apple M5 Max at a load average of 48 to 51: 2.4 to 2.6 s wall with the test build, and 1.5 to 1.8 s with a release build. From a clean build folder, building and running the workspace test took 22 s.
- It needs git and the whole history (a shallow clone is refused), and the sources of every linux-raw-sys version Cargo.lock names, unpacked by Cargo under its home. If they are missing the check stops with an error that says to run `cargo fetch`; it never runs without them.

The check prints only what needs doing, sorted, one line each, every line ending in `do:` and the action:
- `+` a gated finding the baseline does not accept, with its file, line and column: name it as the note says.
- `-` a baseline line no longer found: delete it (this is how the baseline shrinks).
- `~` a baselined literal still in the tree but now in a list that is not gated (moved under `#[cfg(test)]`, into a test, a message or a generated file), whether its line is still in the baseline or was deleted at any point since the baseline started: it was hidden, not fixed; undo the move and name the value. A `restated` line is left out: it leaves its list when the constant it restated goes, not by being moved.
- `>` a baseline line added after the baseline started, with the commit that added it (or "the working tree"): delete it and name the value.
- `!` the baseline is not sorted: sort it (`LC_ALL=C sort`).

## The baseline

`literals.baseline` holds one line per accepted finding: list, path, kind, enclosing item and literal, separated by tabs, without line numbers, so edits elsewhere in a file do not disturb it (a literal longer than 160 characters is cut and keyed with a hash of all of it). The same line twice means the literal is there twice in that item. Lines are sorted, so two branches that remove different lines merge without touching each other. Git still conflicts when the two removed lines are neighbours; then keep neither line and run the check, which says what is left.

`policy` lines say which paths leave the normal lists: `policy<TAB>skip<TAB>PATH` (not scanned), `policy<TAB>generated<TAB>PATH` (scanned into the generated list), `policy<TAB>test<TAB>PATH` (test code in src/). A policy line is the only way a path leaves the check, and it is a baseline line like the others, so a new one fails as growth. The workspace manifest's `exclude` folders are always skipped. On main the generated lines are the kernel's `src/linux/*.rs` and `src/syscall/list.rs`.

The history rule, checked on every run from git:
- The baseline starts with the first commit on the branch's first-parent history that has it.
- If that commit is a merge (the file arrived from another branch), the next change to it is the branch's `init` and may add lines.
- Every other change may only remove lines, except lines a merge brings from the other side; that side is checked the same way back to the merge base, so growth on a side branch merged in is still seen.
- Deleting the file and adding it back is growth of every line.
- The file has one start in the whole history. A second start that is not a descendant of the first (a branch that wrote its own baseline, wherever it was merged) fails (`>` "started more than once").
- Diffs are read as text whatever `.gitattributes` says, and any git attribute set on the baseline fails (`>`): marking it binary would otherwise hide its diffs.
- Every run prints the last commit that could add lines (the start, or the init after the baseline arrived). When the branch has an upstream (`@{upstream}`, as a clone of a pushed branch does), that commit must be the same as the upstream's: a replaced start fails (`>` "the start was replaced").

## How a track adopts the check

1. Merge `phase-1-literals` (or main, once it has the check) into the track's branch.
2. Run `cargo run -q -p wcontainers-literals -- init` once, either before committing the merge or as the next change to `literals.baseline` after it. It rewrites the baseline from the branch's tree, keeps the policy lines it had, and adds a `generated` line for each file whose first five lines say "@generated", "do not edit" or "automatically generated" (printed as `proposed:`). Read every proposed line; anything other than the kernel's generated Linux files goes to the orchestrator before it is committed.
3. Commit the baseline. From then on `init` refuses, and `cargo test -p wcontainers-literals` must pass before every push. Record in the track's status file the commit the check prints as "the last that could add lines" (the init commit): it must never change.
4. To fix a literal: name it, run the check, delete the `-` line it prints, in the same commit.
5. Never add a line. A new value gets a name; a number that truly stands alone gets a mark.
6. A fact owned by another track's crate is never defined locally while waiting for the owner: its line stays until the owner's constant is on main.

## What the check cannot see

A person still has to look at these. Each one the attacks found has a test that records it (`tests/attack.rs`, `tests/second_attack.rs`, `tests/third_attack.rs`, module `gets_through`; `tests/baseline.rs`), so an improvement shows as a failing test to update.

- A free number with no role in sight: `Duration::from_secs(1)`, `from_millis(1)`, `return 0` as a success code from a function whose name says nothing, `!0`, a 1 in a `macro_rules!` body, `x.max(1.0)`, a value in a `match` arm of a function named for a role.
- -1 or 2 given to a function whose name, and the types it belongs to, do not say what it takes: `write(2, ..)` for stderr, `kill(-1, ..)` for every process, `signal::sender_info(sig, code, 0, 0)` (a module's name is not read).
- A value built only from free numbers: `1 + 1 + 1 + 1`, `2 << 2 << 2`, `1u64 << (u8::BITS + 2 + 2)`, or a constant of twelve 2s multiplied that equals the page size.
- The same fact under another name with a small value (`enum Signal { Segv = 11 }`), a constant whose name shares no specific word with the owner's, or a value written as an expression of small numbers inside a constant (`1 << 12`).
- A linux-raw-sys value retyped under another name in a crate that does not depend on linux-raw-sys (`OPEN_CLOEXEC = 0o2000000`).
- A literal moved within the same function, or into a closure of it (baseline lines are per enclosing item).
- Text that hides a value: format widths and precisions (`{:08x}`, `{x:>12}`), numbers written as words ("needs sixty-four bytes"), a decimal in a message (`"needs 4.5 GiB"`, read like a version such as "Linux 6.8"), numbers inside strings parsed later (WGSL, JSON, `"0x1000".parse()`), and numbers or paths in attribute strings (`#![recursion_limit = "256"]`, `#[path = "..."]`), which are gated only when they write out a string constant.
- A build script's `println!("cargo:...")` lines are read as messages, although Cargo reads them.
- Paths and other strings in scripts: only their numbers are checked (`open("/etc/hosts")`, `cat /etc/hosts`).
- Messages passed to a crate's own message helpers (`fail(...)`) are gated like any code string: a false alarm in the other direction.
- Output of derive and procedural macros, `include_str!` data, and other languages: C, JavaScript, WGSL, HLSL, PowerShell, TOML (including Cargo.toml versions).
- The workspace manifest's excluded folders (unless code there is placed with `#[path]` from a scanned file), and new literals written by hand into a file under a `generated` policy line: whoever owns the root manifest and the generators must review those.
- A mark inside a `/* */` block comment is read as a mark. A mark's reason is never judged: "x" is a reason. Every run lists every mark with its reason so that a person judges them.
- A second adoption on a branch with no upstream: a fork from before the track adopted the check merges the literals branch, runs init with new literals in its tree, then takes the track in keeping its own copy of the baseline, and the track is fast-forwarded to it. Each step is one the rules allow, so the check passes where it cannot compare with the branch's upstream (a local branch never pushed, or one whose upstream was unset). What shows is the line every run prints, "the last that could add lines <commit>": it never changes once a track has adopted the check. The check fails where the upstream still holds the earlier push (the track's own checkout before it pushes); once the replaced start is pushed, a clone's upstream holds it too, and only the printed commit shows it. Whoever merges a track compares that commit with the one the track recorded when it adopted the check.
