# Status of the literals track

Plan and rule: docs/literals.md. Branch: phase-1-literals.

## Done (milestone L1)

- Crate `crates/wcontainers-literals`: the audit's scanner, changed to the final rule:
  - -1, 0, 1, 2 free as arithmetic, gated in a role (uid, fd, pid, exit status and the rest);
  - the mark `// standalone NUMBER: reason` on the line above, refused without a reason, on text, with nothing under it, on a number any reachable constant has (any size), or on a number another mark also covers; every mark listed and counted by every run;
  - text shown to a person free unless it holds a number, a path, or the value of a string constant its crate can reach (name-like words only);
  - one baseline for the workspace, `literals.baseline` at the root, sorted, one key per line;
  - growth checked from git history (src/history.rs): the baseline's start on the first-parent chain, one `init` after it arrives by a merge, merges allowed to bring the other side's lines (checked back to the merge base), deletion and re-adding counted as growth, shallow clones refused; diffs read with `--text`, any git attribute on the baseline refused, a second unrelated start anywhere in the history refused, and the last commit that could add lines printed by every run and compared with the branch's upstream when it has one.
  - scanned files: what `git ls-files --cached --others --exclude-standard` lists, plus every module placed with `#[path]` or declared under a test attribute wherever it lies; the old folder walk (hidden folders, CACHEDIR.TAG, node_modules) is gone.
  - every linux-raw-sys version in Cargo.lock must be unpacked under CARGO_HOME, or the check stops with an error saying to run `cargo fetch`.
- Commands: `check`, `init`, `list`. Every output line ends in `do:` and what to do.
- Tests (all pass): `tests/attack.rs` (the audit's 110 snippet tests), `tests/second_attack.rs` (my 20 hiding and 10 legitimate snippets, and two tests of marks), `tests/third_attack.rs` (18 tests from the independent check of run 1's work), `tests/baseline.rs` (21 git scenarios), `tests/fixture.rs` (the audit's fixture, output reviewed line by line), `tests/workspace.rs` (the check on this workspace).
- `literals.baseline`: main's state, 7972 lines at its start, 7958 now (run 2 removed the 14 `env!("CARGO_MANIFEST_DIR")` lines, now free), of which 11 are `generated` policy lines (kernel `src/linux/*.rs`, `src/syscall/list.rs`), proposed by init and as the audit expected. The check passes on this branch.
- The crate passes its own rule with no marks and no baseline line: no gated finding under crates/wcontainers-literals, tests included (the first baseline had 17 lines for its tests, which wrote the output prefixes, git's name and "::"; the tests now import them, and the lines were removed); its 16 free numbers listed by `list` were read one by one in run 2 (pairs in `windows(2)`, "more than one parent", "one argument", the first column, the second change after an arrival, an index of the first or last segment) and none carries a role; the free numbers used as arithmetic (`checked_sub(1)`, `start + 1`) are the previous and the next.

## How to run

- `nix develop -c cargo run -q -p wcontainers-literals -- check` (exit 0 is clean); `-- list` for every finding; `-- init` once per track after the merge that brings the baseline.
- `nix develop -c cargo test -p wcontainers-literals -- --test-threads 3`; the workspace check alone: `--test workspace`.
- On a machine where Cargo has not yet downloaded the workspace's crates, run `cargo fetch` once: the check needs the linux-raw-sys sources, and stops with an error without them.
- The bar on a scratch clone: `python3 -I pyro-parked/wcontainers-scratch/literals/bar.py <path to the built wcontainers-literals>` (clones the branch into scratch/literals/clone). Clean-build timing: `scratch/literals/clean-build.sh`.
- Build output goes to `CARGO_TARGET_DIR=/Users/thomaseaton/Documents/GitHub/pyro-parked/wcontainers-scratch/literals/target`.

## Measurements (Apple M5 Max, shared with other tracks)

- Check on the whole workspace (292 Rust files, 6 scripts), run 2: 2.36 to 2.55 s wall with the test build and 1.52 to 1.83 s with a release build, three runs each, at a load average of 48 to 51 (2026-11-18 15:08). Run 1 measured 1.9 to 2.4 s and 1.7 to 2.0 s, at a load of 66 to 77.
- From an empty build folder, `cargo test -j 3 -p wcontainers-literals --test workspace` took 22 s including the build, at a load of 48 to 51 (`clean-build.sh`, run 2).
- Whole crate test suite: 14 to 28 s, most of it the 21 git scenarios (14 to 28 s), at a load of 50 to 80 (run 2).

## Independent check of run 1, and how each finding was settled (run 2)

- Growth hidden by `literals.baseline binary` in .gitattributes: fixed. History is read with `--text`, and any attribute on the file fails. Bar cheat 7 and test `growth_behind_a_binary_attribute_fails`.
- A folder holding CACHEDIR.TAG was skipped: fixed. The scan lists files with git, so build output is left out only by the ignore rules. Bar cheat 8 and test `code_in_tagged_hidden_and_ignored_folders_is_scanned`.
- Missing linux-raw-sys sources were skipped silently: fixed. That is now an error naming `cargo fetch`. Bar cheat 11 and test `a_missing_reference_crate_is_an_error`.
- A fresh start fast-forwarded onto the track: fixed. Starts are found over the whole history (`git log --full-history -m --diff-filter=A`), and two unrelated starts fail. Bar cheat 10, and tests `a_fresh_start_fast_forwarded_onto_the_track_fails` and `a_side_branch_that_starts_its_own_baseline_fails` (the latter used to record that it got through).
- What remains of that hole is a second adoption: a fork from before the track adopted merges the literals branch, runs init with new literals, and replaces the track. There is still one start, so this is caught only by comparing with the upstream (test `a_second_adoption_is_seen_only_against_the_upstream`) or by a person comparing the printed commit. The document says so plainly, and asks each track to record that commit.
- Marks: fixed. A mark on any value a reachable constant has is refused (`9` with `SIGNAL_KILL`), and the same number marked twice fails both marks. Bar cheat 12 and tests r07/r08.
- `#[path]` into a hidden folder: fixed. Placed modules are scanned wherever they are. Bar cheat 9.
- `env!("CARGO_PKG_VERSION")`: `CARGO_*` and `OUT_DIR` given to `env!`/`option_env!` are now free; any other name is still code.
- The review's snippets:
  - Now caught: `#[serde(rename = "st_uid")]` (restated), `ExitCode::from(2)`, `let (uid, gid) = (0, 0)`, `fn stdout_fd() -> i32 { 1 }`, `return -1` in `fn pid`, Python `4096>size`, shell `$((4096>>2))`.
  - Still get through, now listed in the document with tests: a decimal in a message, a build script's `cargo:` lines, paths in scripts.

The bar, run 2, on a scratch clone (`bar.py`, debug binary, load average 52 to 82, 2026-11-18 15:07):
- Clean tree: passes, 2.26 s.
- A bare literal added to the kernel fails with `+ code crates/wcontainers-kernel/src/lib.rs:686:39 ... 31337`.
- A deleted baseline line fails with `+` for the literal it covered.
- Growth fails with `>`, both uncommitted and committed.
- A mark with a reason passes and is listed (`marks: 1` and the reason). A mark without a reason fails with `+ mark`.
- Two branches that each mark a literal in a different crate (arm64, kernel-test): git merged them with no conflict, and the check passed.
- Cheats 1 to 4 and 6 are caught as before: side-branch growth, deleting and re-adding the baseline, a skip policy line, a literal moved under cfg(test), and a shallow clone. Cheat 5, a literal written into a generated file, still gets through, as documented.
- Cheats 7 to 12, from the review, are all caught (see above).

Second attack (run 1, tests/second_attack.rs): 14 of 20 hiding snippets caught and 6 get through (1+1+1+1, 2<<2<<2, twelve 2s, a format width, write(2, ..), kill(-1, ..)). None of the 10 legitimate snippets is wrongly caught. Unchanged in run 2.

## Dependencies and licences

syn 2.0.119, proc-macro2 1.0.107, quote 1.0.47, toml 1.2.1: MIT OR Apache-2.0. fnv 1.0.7: Apache-2.0 / MIT. All were already in Cargo.lock; the lock gained only this crate's entry.

## Departures from the plan

- The workspace manifest needed no line: `members = ["crates/*"]` already takes the crate. Cargo.lock gained the crate's own entry.
- One baseline for the workspace, as the task asks, instead of the audit's one per crate. Growth is checked against the branch's own history in git, not against main, because each track starts its own baseline after merging.
- A policy line added after the baseline started counts as growth. There is no approval path for a new generated file or vendored folder yet; that is the orchestrator's decision.
- `list` does not propose generated files; only `init` does (printed as `proposed:`), so the audit's fixture now lists its "@generated" file as code.
- The free numbers count as no number in messages too (the audit exempted 0 and 1).
- Improvements that would gate literals already on main could not be made, because the baseline cannot grow. They are listed as gaps in the document instead:
  - a build script's `cargo:` lines as code (4 in crates/wcontainers-wasmtime/build.rs);
  - numbers and paths in attribute strings (`#![recursion_limit = "256"]` in the browser crate);
  - the module names in a role hint (`signal::sender_info(.., 0, 0)` in the kernel, 5 literals).
  Attribute strings are gated only when they write out a string constant, and role hints read only a function's types, never its modules.
- The upstream comparison works only where `@{upstream}` exists. A run without one says so in its summary line ("no upstream branch to compare it with"); it is not silent.
- The rule's "a name that has a constant" in text is read as a word with an upper-case letter, a digit or one of `_ - . : /` that equals the value of a string constant its crate can reach: plain English words are left alone.

## Next

- Milestone L1 is complete. Nothing further is planned in this file.
