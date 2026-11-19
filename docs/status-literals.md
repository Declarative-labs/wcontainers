# Status of the literals track

Plan and rule: docs/literals.md. Branch: phase-1-literals.

## Done (milestones L1 and L2)

- Crate `crates/wcontainers-literals`: the audit's scanner, changed to the final rule:
  - -1, 0, 1, 2 free as arithmetic, gated in a role (uid, fd, pid, exit status, code, user, owner and the rest, and the C library's `getuid`, `setpgid`, `chown` and the like); the role is read through a closure's or local function's parameter names, `Some`/`Ok`, references, array elements, `if`/`match`/block values, a local binding later given to a role (`exit(code)`, `setuid(ids[0])`), and comparing methods (`uid.eq(&0)`); a name that also says count, mask, len, size, total or number is not a role;
  - the mark `// standalone NUMBER: reason` on the line above, refused without a reason in words, on text, with nothing under it, on a number any constant anywhere has (any size), or on a number another mark also covers; every mark listed and counted by every run;
  - text shown to a person free unless it holds a number, a path, the value of a string constant its crate can reach (name-like words only), or the crate's own package name; the same for string constants (a constant that writes out another one inside a longer text); a string in a message position with no space that is not one plain lower-case word is data, so code, except a single trailing `:` or `%`; a number joined to a word by a hyphen (`UTF-8`) is part of a name;
  - `stringify!` and `concat!` of literals are one string literal; a byte string of text has that text as its value; an error text returned from a `map_err`/`ok_or_else` closure is a message; a constant restates one of the same value in a crate that does not depend on it when their names share a specific word (reported on the crate fewer crates depend on); the same name and value is only a `redefinition`, no longer also `restated`; attributes inside `cfg_attr` are read; `-(2)` is -2; a test that writes a named number above 255 as a float or a string of digits is `restated`;
  - notes name only constants the literal's crate could import (public with every enclosing module public, file modules included; its own; linux-raw-sys); the action mentions the note only when there is one;
  - one baseline for the workspace, `literals.baseline` at the root, sorted, one key per line;
  - growth checked from git history (src/history.rs). Any line may be added only by the start and the init after the baseline arrives by a merge. A change where `src/rules.rs` lists more versions than at the baseline's previous change (or than before a merge that brought them) may add lines only for literals that were already in the tree then (or on a side it merges). Editing the checker without a new version opens nothing. A line removed after it grew is no longer growth. Replace refs are ignored (`git --no-replace-objects`). Every change that could add lines, on every side, is listed by every run, and the workspace test writes that list and the marks past the test harness's capture, so `cargo test` shows them when it passes.
  - scanned files: what `git ls-files --cached --others --exclude-standard` lists, also inside submodules and nested repositories (listed again inside each), minus `skip` policy lines, plus every target root a manifest names whatever its extension (`[lib]`, `[[bin]]`, `package.build` as code; `[[test]]`, `[[bench]]`, `[[example]]` as test), plus every module a scanned file declares wherever it lies; the root of a shipped target placed in tests/ (also through `..` in its path) and its modules are code. A listed folder that is not a checked-out repository (a submodule not checked out, a link to a folder) is an `unparsed` finding.
  - every linux-raw-sys version in Cargo.lock must be unpacked under CARGO_HOME, or the check stops with an error saying to run `cargo fetch`.
  - the scan runs on rayon (one thread per core, or `RAYON_NUM_THREADS`).
- Commands: `check`, `init`, `list`. Every output line ends in `do:` and what to do.
- Tests (all pass, L2 run 1, 292 in all): `tests/attack.rs` (110), `tests/second_attack.rs` (32), `tests/third_attack.rs` (18), `tests/fourth_attack.rs` (19), `tests/fifth_attack.rs` (27), `tests/sixth_attack.rs` (20), `tests/seventh_attack.rs` (37: the test-constant hole, notes of separate test crates, and one recording test per listed gap that had none, with the last check's new classes), `tests/baseline.rs` (27 git scenarios, 3 of them in `gets_through`: a literal moved within its function, a made-up version in one merge, a submodule's literal that a new version cannot add), `tests/fixture.rs` (1), `tests/workspace.rs` (1, the check on this workspace). Every snippet test first checks that the snippet is in the fixture text.
- `literals.baseline`: main's state, 8159 lines (11 of them `generated` policy lines). History on this branch: started at 68214b2e; run 3's 102 data-string lines were taken out (b4dabb45) and added back by version 1 of the rules (89f88281) as literals already in the tree; version 2 (7119e3f3) added 67 lines for the new rules' findings in other crates (roles through wrappers and bindings, tests writing named numbers as floats, messages naming their own crate) and freed one (`"rustfmt output is UTF-8"`); version 3 (c3bf56d4) added 8 (seven `code` roles: signal codes in the kernel and `load_code`'s 0/1 status in wasmtime; one constant `PAGE = 4096` in the translator-test reference crate restating `PAGE_SIZE`) and freed 4 (`restated` lines that duplicated a `redefinition` of the same name); version 4 (31ec396f) added 30 `restated` lines, all test literals inside a test's own constant (11 in the kernel's seccomp test table, 19 in translator-test and wasmtime tests) and freed none. The format and the meaning of every existing line are unchanged.
- The crate passes its own rule with no marks and no baseline line, tests included, with the test-constant hole closed: its tests now write `MANIFEST` and `LOCK_FILE` (made public) instead of "Cargo.toml" and "Cargo.lock". Its item paths (`<Type as Trait>`, `macro_rules! name`) are joined from named constants, no longer written as messages.
- The summary line holds no time, so two runs on one tree print the same (checked: three runs, identical output).

## How to run

- `nix develop -c cargo run -q -p wcontainers-literals -- check` (exit 0 is clean); `-- list` for every finding; `-- init` once per track after the merge that brings the baseline, and again after a merge that brings a new version of the rules.
- `RAYON_NUM_THREADS=3 nix develop -c cargo test -p wcontainers-literals -- --test-threads 3`; the workspace check alone: `--test workspace`.
- On a machine where Cargo has not yet downloaded the workspace's crates, run `cargo fetch` once.
- The bar on a scratch clone: `python3 -I pyro-parked/wcontainers-scratch/literals/bar.py <path to the built wcontainers-literals>` (clones the branch into scratch/literals/clone; cheats 1 to 20). Clean-build timing: `scratch/literals/clean-build.sh`.
- Build output goes to `CARGO_TARGET_DIR=/Users/thomaseaton/Documents/GitHub/pyro-parked/wcontainers-scratch/literals/target`.
- A rules change that finds more appends an entry to `src/rules.rs`, runs `init`, and commits both together.

## Measurements (Apple M5 Max, shared with other tracks)

- L2 run 1 (17:51 to 17:53, `clean-build-l2.log`, version 4, 297 Rust files): check with the test build 4.22, 4.45, 4.31 s at a load of 56; in the clean-build script 3.93 to 5.48 s at 65 to 73; release build 3.98 to 4.36 s at 73 to 88. From an empty build folder, `cargo test -j 3 -p wcontainers-literals --test workspace` took 32 s including the build, at 65 to 70. Whole crate suite (`suite-l2.log`): 42 s wall including an incremental build, the git scenarios 22 s, at a load of 60 to 64.

- Check on the whole workspace (296 Rust files, 6 scripts), run 5 (17:16 to 17:17, `clean-build-run5.log`): test build 4.08 to 4.36 s wall (scan 2.26 to 2.32 s, the rest the history) at a load average of 51 to 58; release build 3.84 to 4.17 s at 51 to 59. Three runs each. Run 4 measured 2.9 to 3.2 s; the history now reads back one more versioned change, and nested repositories are listed.
- From an empty build folder, `cargo test -j 3 -p wcontainers-literals --test workspace` took 42 s including the build, at a load of 52 to 58 (run 4: 25 s at 47 to 57).
- Whole crate suite, run 5: 29 s wall including the build, the git scenarios 18 s, at a load of 70 to 74 (17:08).
- The bar, run 5 (`bar-run5.log`, against the committed tip c3bf56d4): each check 3.6 to 6.0 s at a load of 55 to 67 (17:12 to 17:15).

## Bar, L2 run 1

- Crate suite: 292 pass. Workspace check from a clean build: passes, 32 s (above). fmt clean; clippy with `-D warnings` on all targets clean. The tool passes its own rule (no baseline line, no mark).
- Scratch adoption (`scratch/literals/adopt-l2.sh <built checker>`): a clone's branch at 0eeb26e4 (version 3) adds a test `const P: u64 = 4096` beside `PAGE_SIZE` (free under version 3), merges this branch with no conflict, fails with one `+ restated` line for it, runs `init` once (one line added), commits, and passes. The list of changes that could add lines then names the merge and the init as "new rules".
- bar.py (cheats 1 to 23) was not rerun this run; nothing in the history code changed.

## Bar, run 5 (python3 -I scratch/literals/bar.py, cheats 1 to 23 and adoption through main)

- Clean tree passes; one bare literal in the kernel fails naming its line; a deleted baseline line fails; growth fails uncommitted and committed; a mark with a reason passes and is listed, one without fails; two branches fixing different literals merge with no conflict and pass.
- Cheats caught: 1 to 4, 6 to 13, 15 to 18, 20, and the new 21 (lib.txt and main.in roots, uncommitted and committed), 22 (a member in a submodule) and 23 (the same submodule not checked out: `unparsed`).
- Pass but listed: 14 (side adoption) and 19 (a made-up version); both documented. Still open: 5 (a literal written into a generated file), documented.
- Adoption through main: a track that merged the branch and ran init, then merged a main that had merged the branch and run init with another track's literal, merged with no conflict and passed.

## Independent check of the run 5 tip, and how each finding was settled (L2 run 1)

- A test that writes a named value inside a constant of its own (blocking): fixed as version 4 of the rules; tests `t06_test_page_in_const`, `T07_SIZES`, `t08_unit_test_constant` in `tests/seventh_attack.rs`; the checker's own tests import `MANIFEST` and `LOCK_FILE`.
- Listed gaps without a recording test (blocking): every gap in docs/literals.md now has one (`tests/seventh_attack.rs` modules `gets_through` and `wrongly_caught`, `tests/baseline.rs` module `gets_through`); the nine new classes are listed with tests (`s01`, `s04`, `s05`, `s07`, `s10`, `s14`, `s16`, `t01`; t06 is the fixed one). For macro output, a procedural attribute's arguments turned out to be read (`#[x(threads = 4)]` is caught), so the recording test is a macro call whose expansion would hold the number (`page_size!()`). `"0x1000".parse()` was taken out of the list: a string with no space in code is gated.
- Made-up version (minor): the document now says it takes one merge commit. Not closed: limiting a merged side's literals to those at that side's last baseline change would refuse an adopter's init when a merge makes another side's literal newly gated. Recorded by `a_made_up_version_in_one_merge`.
- Notes naming a private constant to an integration test (minor): fixed; such a test, bench or example is a crate of its own and is told only of public constants (`notes_name_only_what_the_test_can_import`).
- Summary time (minor): dropped. `USAGE` in main.rs (minor): now `usage()`. Key-building strings as messages (minor): joined from constants.

## Dependencies and licences

syn 2.0.119, proc-macro2 1.0.107, quote 1.0.47, toml 1.2.1: MIT OR Apache-2.0. fnv 1.0.7: Apache-2.0 / MIT. rayon 1.12.0 (with rayon-core 1.13.0, crossbeam-deque, crossbeam-epoch, crossbeam-utils, either): MIT OR Apache-2.0. All were already in Cargo.lock; the lock gained only this crate's entry and its rayon line.

## Departures from the plan

- The workspace manifest needed no line: `members = ["crates/*"]` already takes the crate.
- One baseline for the workspace, as the task asks, instead of the audit's one per crate. Growth is checked against the branch's own history in git, not against main.
- The baseline may grow at a new version of the rules, for literals already in the tree. Without this, every tightening of the rules would fail every track that adopted the check, with no legal fix. A made-up version can still let in a literal that was committed with an earlier baseline change; it is listed by every run and needs an edit of the checker's crate.
- Run 4 rewrote this branch's baseline history once: run 3's growth (1696d3f1) was taken out and added back under version 1, since the new history rule no longer accepts growth at an unversioned rules change.
- A policy line added after the baseline started counts as growth, also at a new version of the rules. There is no approval path for a new generated file or vendored folder yet; that is the orchestrator's decision.
- `list` does not propose generated files; only `init` does.
- The free numbers count as no number in messages too (the audit exempted 0 and 1).
- Still not gated: numbers and paths in attribute strings, and module names in a role hint. Both are documented gaps.
- The upstream comparison works only where `@{upstream}` exists; a run without one says so.
- "A name that has a constant" in text is read as a word with an upper-case letter, a digit or one of `_ - . : /` that equals a string constant its crate can reach, or the crate's package name.
- Tracks adopt by merging main after main has the check (docs/literals.md, adoption step 1), not by merging this branch directly: a direct merge followed by a main that lacks the check leaves the track no legal action.
- Two constants of one crate with the same value under names sharing a word are not compared (too often two facts, `MQ_MSGSIZE_DEFAULT` and `MQ_MSGSIZE_MAX`); unrelated crates are. A constant with the same name and value is reported once, as `redefinition`; the fixture's expectation was changed to match (its test-module copy of `PAGE_SIZE` left `restated`, its `redefinition` line stays).
- `code` became a role word: it catches signal codes and exit codes, and also `load_code`'s 0/1 status in wasmtime, which is a status even though the name means machine code.
- `MANIFEST` and `usage()` are now public so that the tests and the command line use the names instead of writing them out; `LOCK_FILE` too (L2).
- L2: a test literal is still gated against every constant of its own crate and of its upstream crates, private ones included, as before; the finding asked for at least the importable ones. Narrowing to importable constants would have removed 70 `restated` lines from every adopter's baseline (tests that write a private constant's value, which the owner can fix by making it public) and asked every adopter to delete them. Only the notes were narrowed. A test's copy of a constant under the owner's own name stays a `redefinition` only.

## Next

- Milestone L2 is complete. Every adopting track must, after the merge that brings version 4: run `cargo run -q -p wcontainers-literals -- init` once (before committing the merge or as the next change of the baseline) and commit. The format of the baseline and the meaning of its lines did not change.
- Milestone L1 is complete. The orchestrator must run the one command itself on every merge (the check cannot see its own removal from the test run). Possible later: gating numbers and paths in attribute strings, reading module names in role hints, comparing float constants.
