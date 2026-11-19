# Status of the literals track

Plan and rule: docs/literals.md. Branch: phase-1-literals.

## Done (milestone L1)

- Crate `crates/wcontainers-literals`: the audit's scanner, changed to the final rule:
  - -1, 0, 1, 2 free as arithmetic, gated in a role (uid, fd, pid, exit status, user, owner and the rest, and the C library's `getuid`, `setpgid`, `chown` and the like); the role is read through a closure's or local function's parameter names, `Some`/`Ok`, `if`/`match`/block values, and a local binding later given to a role (`exit(code)`); a name that also says count, mask, len, size, total or number is not a role;
  - the mark `// standalone NUMBER: reason` on the line above, refused without a reason in words, on text, with nothing under it, on a number any constant anywhere has (any size), or on a number another mark also covers; every mark listed and counted by every run;
  - text shown to a person free unless it holds a number, a path, the value of a string constant its crate can reach (name-like words only), or the crate's own package name; the same for string constants (a constant that writes out another one inside a longer text); a string in a message position with no space that is not one plain lower-case word is data, so code, except a single trailing `:` or `%`; a number joined to a word by a hyphen (`UTF-8`) is part of a name;
  - `stringify!` is a string literal; attributes inside `cfg_attr` are read; `-(2)` is -2; a test that writes a named number above 255 as a float or a string of digits is `restated`;
  - notes name only constants the literal's crate could import (public, its own, linux-raw-sys); the action mentions the note only when there is one;
  - one baseline for the workspace, `literals.baseline` at the root, sorted, one key per line;
  - growth checked from git history (src/history.rs). Any line may be added only by the start and the init after the baseline arrives by a merge. A change where `src/rules.rs` lists more versions than at the baseline's previous change (or than before a merge that brought them) may add lines only for literals that were already in the tree then (or on a side it merges). Editing the checker without a new version opens nothing. A line removed after it grew is no longer growth. Replace refs are ignored (`git --no-replace-objects`). Every change that could add lines, on every side, is listed by every run, and the workspace test writes that list and the marks past the test harness's capture, so `cargo test` shows them when it passes.
  - scanned files: what `git ls-files --cached --others --exclude-standard` lists, minus `skip` policy lines, plus every module a scanned file declares wherever it lies; the root of a shipped target placed in tests/ (also through `..` in its path) and its modules are code.
  - every linux-raw-sys version in Cargo.lock must be unpacked under CARGO_HOME, or the check stops with an error saying to run `cargo fetch`.
  - the scan runs on rayon (one thread per core, or `RAYON_NUM_THREADS`).
- Commands: `check`, `init`, `list`. Every output line ends in `do:` and what to do.
- Tests (all pass, run 4): `tests/attack.rs` (110), `tests/second_attack.rs` (32), `tests/third_attack.rs` (18), `tests/fourth_attack.rs` (19), `tests/fifth_attack.rs` (27, the run 3 tip's review), `tests/baseline.rs` (24 git scenarios), `tests/fixture.rs` (1), `tests/workspace.rs` (1, the check on this workspace). Every snippet test first checks that the snippet is in the fixture text.
- `literals.baseline`: main's state, 8125 lines (11 of them `generated` policy lines). History on this branch: started at 68214b2e; run 3's 102 data-string lines were taken out (b4dabb45) and added back by version 1 of the rules (89f88281) as literals already in the tree; version 2 (7119e3f3) added 67 lines for the new rules' findings in other crates (roles through wrappers and bindings, tests writing named numbers as floats, messages naming their own crate) and freed one (`"rustfmt output is UTF-8"`).
- The crate passes its own rule with no marks and no baseline line, tests included. Run 4 removed the file name, the note mark, the package name and the line ending it had written out again (the new string-constant rule found two more: the mark syntax in the mark errors and linux-raw-sys in an instruction).

## How to run

- `nix develop -c cargo run -q -p wcontainers-literals -- check` (exit 0 is clean); `-- list` for every finding; `-- init` once per track after the merge that brings the baseline, and again after a merge that brings a new version of the rules.
- `RAYON_NUM_THREADS=3 nix develop -c cargo test -p wcontainers-literals -- --test-threads 3`; the workspace check alone: `--test workspace`.
- On a machine where Cargo has not yet downloaded the workspace's crates, run `cargo fetch` once.
- The bar on a scratch clone: `python3 -I pyro-parked/wcontainers-scratch/literals/bar.py <path to the built wcontainers-literals>` (clones the branch into scratch/literals/clone; cheats 1 to 20). Clean-build timing: `scratch/literals/clean-build.sh`.
- Build output goes to `CARGO_TARGET_DIR=/Users/thomaseaton/Documents/GitHub/pyro-parked/wcontainers-scratch/literals/target`.
- A rules change that finds more appends an entry to `src/rules.rs`, runs `init`, and commits both together.

## Measurements (Apple M5 Max, shared with other tracks)

- Check on the whole workspace (295 Rust files, 6 scripts), run 4 (16:35 to 16:36): test build 2.85 to 3.16 s wall (scan 1.6 to 1.9 s, the rest the history) at a load average of 45 to 55; release build 2.36 to 2.94 s at 41 to 45; test build with `RAYON_NUM_THREADS=3` 3.29 to 3.67 s at 35 to 37. Three runs each.
- From an empty build folder, `cargo test -j 3 -p wcontainers-literals --test workspace` took 25 s including the build, at a load of 47 to 57 (`clean-build.sh`, log `clean-build-run4.log`).
- Whole crate suite, run 4: about 40 s wall including the build, the git scenarios 19 to 36 s, at a load of 72 to 93 (16:21 to 16:31).
- The bar, run 4 (`bar-run4.log`, against the pushed tip 7119e3f3): each check 2.3 to 4.0 s at a load of 37 to 61 (16:33).

## Bar, run 4 (on the pushed tip)

- Clean tree passes; one bare literal in the kernel fails naming its line; a deleted baseline line fails; growth fails uncommitted and committed; a mark with a reason passes and is listed, one without fails; two branches fixing different literals merge with no conflict and pass.
- Cheats caught: 1 to 4, 6 to 13, 15 to 18, 20 (17: a comment in the checker with growth; 18: a new version with a new literal; 20: a graft with no upstream).
- Pass but listed: 14 (side adoption) and 19 (a literal committed with another baseline change, then a made-up version); both documented.
- Still open: 5 (a literal written into a generated file), documented.

## Independent check of the run 3 tip, and how each finding was settled (run 4)

- Open growth window after any edit of the checker's source (findings 1 and 2): growth now needs a new entry in `src/rules.rs` and only covers literals already in the tree; tests `new_rules_may_add_lines_for_literals_already_there`, bar cheats 17 and 18. The list of changes that could add lines is now printed by `cargo test` too.
- The tool's own duplicates (finding 3): fixed by hand, and string constants are now compared with other constants and the package name, which found two more. Punctuation-only constants (`"# "`, `'\n'`) written inside strings remain a documented gap.
- `..` in a bin path, `cfg_attr`, `-(2)`, the four roles in sight, `stringify!`, tests writing float or string numbers, marks on numbers named elsewhere or with "." as the reason, labels and `UTF-8` wrongly gated, the note-less action, notes naming private constants, replace refs: fixed, each with a test in `tests/fifth_attack.rs` or `tests/baseline.rs`.
- `line!()`, implicit enum discriminants, float constants not compared, `{done}/{total}` wrongly caught: documented with tests that record them.
- The hand-written thread pool: replaced by rayon, which honours `RAYON_NUM_THREADS`; the test command above sets it to 3.

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
- `MANIFEST` and `usage()` are now public so that the tests and the command line use the names instead of writing them out.

## Next

- Milestone L1 is complete. Possible later: gating numbers and paths in attribute strings, reading module names in role hints, comparing float constants.
