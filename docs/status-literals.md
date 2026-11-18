# Status of the literals track

Plan and rule: docs/literals.md. Branch: phase-1-literals.

## Done (milestone L1)

- Crate `crates/wcontainers-literals`: the audit's scanner, changed to the final rule:
  - -1, 0, 1, 2 free as arithmetic, gated in a role (uid, fd, pid, exit status and the rest, and the C library's `getuid`, `setpgid` and the like); a name that also says count, mask, len, size, total or number is not a role;
  - the mark `// standalone NUMBER: reason` on the line above, refused without a reason, on text, with nothing under it, on a number any reachable constant has (any size), or on a number another mark also covers; every mark listed and counted by every run;
  - text shown to a person free unless it holds a number, a path, or the value of a string constant its crate can reach (name-like words only); a string in a message position with no space that is not one plain lower-case word is data, so code (`"HOME={}"`, `"{name}.tar"`, `"GET"`); a macro's string arguments after its format string are code;
  - one baseline for the workspace, `literals.baseline` at the root, sorted, one key per line;
  - growth checked from git history (src/history.rs): lines may be added only by the start, the init after the baseline arrives by a merge, a change where the checker's own `src` differs from the baseline's previous change (new rules), or the change after a merge that brought new rules. Every such change, on every side of every merge, is listed by every run ("changes that could add lines to the baseline: N"). Merges may bring the other side's lines (checked back to the merge base); deletion and re-adding is growth; shallow clones, git attributes on the baseline and a second unrelated start fail; with an upstream, each change of the upstream's that could add lines must also be one on the branch.
  - scanned files: what `git ls-files --cached --others --exclude-standard` lists, minus `skip` policy lines, plus every module a scanned file declares wherever it lies. The workspace manifest's `exclude` no longer takes anything out. The root of a shipped target placed in tests/ (`[lib] path`, `[[bin]] path`, `package.build`) and the modules it declares are code.
  - every linux-raw-sys version in Cargo.lock must be unpacked under CARGO_HOME, or the check stops with an error saying to run `cargo fetch`.
- Commands: `check`, `init`, `list`. Every output line ends in `do:` and what to do.
- Tests (all pass): `tests/attack.rs` (110, the audit's snippets), `tests/second_attack.rs` (32), `tests/third_attack.rs` (18), `tests/fourth_attack.rs` (19, the third run's review), `tests/baseline.rs` (23 git scenarios), `tests/fixture.rs` (1), `tests/workspace.rs` (1, the check on this workspace). Every snippet test first checks that the snippet is in the fixture text, so a renamed or deleted snippet fails.
- `literals.baseline`: main's state. 7972 lines at its start; 7958 after run 2; 8059 now: run 3's rules gate 102 data strings in other crates' code (`format!("{name}.tar")`, `"HOME={}"`, `"cargo:rerun-if-changed={}"`, ...) and free one role line (`signal_mask: 0` in the kernel's `Thread::new`). Written by `init` in the commit that changed the rules (1696d3f1), which the history allows and lists. 11 of the lines are `generated` policy lines (kernel `src/linux/*.rs`, `src/syscall/list.rs`).
- The crate passes its own rule with no marks and no baseline line, tests included. Run 3 named the separators it wrote (`LOCATION_SEPARATOR` ':', `PATH_SEPARATOR` "::" where it was written out, `NEGATIVE_SIGN`, the byte literal's `b'` and quote), and its tests now import the list names, kinds, separators, `DO`, `DELETE_LINE`, `MARKS_HEADING`, `MARK_WORD` and `policy_line` instead of writing them.

## How to run

- `nix develop -c cargo run -q -p wcontainers-literals -- check` (exit 0 is clean); `-- list` for every finding; `-- init` once per track after the merge that brings the baseline, and again only after a merge that brings new rules.
- `nix develop -c cargo test -p wcontainers-literals -- --test-threads 3`; the workspace check alone: `--test workspace`.
- On a machine where Cargo has not yet downloaded the workspace's crates, run `cargo fetch` once.
- The bar on a scratch clone: `python3 -I pyro-parked/wcontainers-scratch/literals/bar.py <path to the built wcontainers-literals>` (clones the branch into scratch/literals/clone; cheats 1 to 17). Clean-build timing: `scratch/literals/clean-build.sh`.
- Build output goes to `CARGO_TARGET_DIR=/Users/thomaseaton/Documents/GitHub/pyro-parked/wcontainers-scratch/literals/target`.

## Measurements (Apple M5 Max, shared with other tracks)

- Check on the whole workspace (293 Rust files, 6 scripts), run 3: 2.88 to 3.85 s wall with the test build and 2.15 to 2.65 s with a release build, three runs each, at a load average of 112 to 125 (15:51). Run 2 measured 2.36 to 2.55 s and 1.52 to 1.83 s at a load of 48 to 51.
- From an empty build folder, `cargo test -j 3 -p wcontainers-literals --test workspace` took 33 s including the build, at a load of 112 to 125 (`clean-build.sh`, run 3; 22 s at a load of 48 to 51 in run 2).
- Whole crate test suite, run 3: 45.6 s wall including the build, most of it the 23 git scenarios (29 s), at a load average of 98 to 103 (15:45).
- The bar, run 3: each check 2.0 to 2.8 s at a load of 75 to 98 (15:46 to 15:48), and 4.4 to 5.5 s in the second pass at a load of about 97.

## Independent check of run 2, and how each finding was settled (run 3)

- Workspace `exclude` hid a member's folder: fixed by dropping the special case; only a `skip` policy line (growth) takes a path out. Bar cheat 13 and test `f03_excluded_folder_is_scanned`.
- A side adoption merged into a track that had adopted: cannot be told apart from another track's adoption merged through main, so it passes. It is now seen: every run lists every change that could add lines, on every side, and the side's init appears in it (bar cheat 14, test `a_side_adoption_merged_into_a_track_is_listed`). The old line "the last that could add lines", which hid it, is gone. Documented as a gap with what a reviewer compares.
- Data in message positions: gated (see above), including a key split across format arguments (bar cheat 16, tests d01 to d05). A string with a space that a program parses still passes, documented with test g01.
- A [[bin]] rooted in tests/: code, and its modules too (bar cheat 15, test f01).
- `setuid(0)`, `getuid() == 0`: role (d06, d07). `pid_count`, `status_messages`, `signal_mask`: free (e01 to e03).
- The checker's own "::" and ':': named. The punctuation-only constant in a sentence remains a documented gap (g04).
- Booleans as numbers, a page size from std bit widths, a test writing a small named value: documented gaps with tests (g02, g03, g05).
- `passes()` testing nothing: every snippet test now asserts the snippet is written in the fixture.
- Naming a value can turn other tracks' test literals into `restated`: adoption step 8 says what to do.

## Dependencies and licences

syn 2.0.119, proc-macro2 1.0.107, quote 1.0.47, toml 1.2.1: MIT OR Apache-2.0. fnv 1.0.7: Apache-2.0 / MIT. All were already in Cargo.lock; the lock gained only this crate's entry.

## Departures from the plan

- The workspace manifest needed no line: `members = ["crates/*"]` already takes the crate.
- One baseline for the workspace, as the task asks, instead of the audit's one per crate. Growth is checked against the branch's own history in git, not against main.
- The baseline may grow at a change of the checker's rules (run 3). Without this, every tightening of the rules would fail every track that adopted the check, with no legal fix. Anyone can edit the checker's source to pass as new rules, so such a change is not proof; it is listed by every run for a reviewer to judge.
- A policy line added after the baseline started counts as growth. There is no approval path for a new generated file or vendored folder yet; that is the orchestrator's decision.
- `list` does not propose generated files; only `init` does.
- The free numbers count as no number in messages too (the audit exempted 0 and 1).
- Still not gated, because it would gate literals on main that run 2 could not add (now possible with the rules path, left for a later run): numbers and paths in attribute strings, and module names in a role hint. Both are documented gaps.
- The upstream comparison works only where `@{upstream}` exists; a run without one says so.
- "A name that has a constant" in text is read as a word with an upper-case letter, a digit or one of `_ - . : /` that equals a string constant its crate can reach.

## Next

- Milestone L1 is complete. Not yet done and possible now that new rules may rewrite the baseline: gating numbers and paths in attribute strings, and reading module names in role hints.
