# Status of the literals track

Plan and rule: docs/literals.md. Branch: phase-1-literals.

## Done (milestone L1)

- Crate `crates/wcontainers-literals`: the audit's scanner, changed to the final rule:
  - -1, 0, 1, 2 free as arithmetic, gated in a role (uid, fd, pid, exit status and the rest);
  - the mark `// standalone NUMBER: reason` on the line above, refused without a reason, on text, with nothing under it, or on a number that has a name; every mark listed and counted by every run;
  - text shown to a person free unless it holds a number, a path, or the value of a string constant its crate can reach (name-like words only);
  - one baseline for the workspace, `literals.baseline` at the root, sorted, one key per line;
  - growth checked from git history (src/history.rs): the baseline's start on the first-parent chain, one `init` after it arrives by a merge, merges allowed to bring the other side's lines (checked back to the merge base), deletion and re-adding counted as growth, shallow clones refused.
- Commands: `check`, `init`, `list`. Every output line ends in `do:` and what to do.
- Tests (all pass): `tests/attack.rs` (the audit's 110 snippet tests), `tests/second_attack.rs` (my 20 hiding and 10 legitimate snippets, and two tests of marks), `tests/baseline.rs` (18 git scenarios), `tests/fixture.rs` (the audit's fixture, output reviewed line by line), `tests/workspace.rs` (the check on this workspace).
- `literals.baseline`: main's state, 7972 lines, of which 11 are `generated` policy lines (kernel `src/linux/*.rs`, `src/syscall/list.rs`), proposed by init and as the audit expected. The check passes on this branch.
- The crate passes its own rule with no marks and no baseline line: no gated finding under crates/wcontainers-literals, tests included (the first baseline had 17 lines for its tests, which wrote the output prefixes, git's name and "::"; the tests now import them, and the lines were removed); its 15 free numbers were read one by one (pairs in `windows(2)`, "more than one parent", "one argument", the first column, the next change) and none carries a role.

## How to run

- `nix develop -c cargo run -q -p wcontainers-literals -- check` (exit 0 is clean); `-- list` for every finding; `-- init` once per track after the merge that brings the baseline.
- `nix develop -c cargo test -p wcontainers-literals -- --test-threads 3`; the workspace check alone: `--test workspace`.
- The bar on a scratch clone: `python3 -I pyro-parked/wcontainers-scratch/literals/bar.py <path to the built wcontainers-literals>` (clones the branch into scratch/literals/clone). Clean-build timing: `scratch/literals/clean-build.sh`.
- Build output goes to `CARGO_TARGET_DIR=/Users/thomaseaton/Documents/GitHub/pyro-parked/wcontainers-scratch/literals/target`.

## Measurements (Apple M5 Max, shared with other tracks)

- Check on the whole workspace (291 Rust files, 6 scripts): 1.9 to 2.4 s wall with the test build, 1.7 to 2.0 s with a release build, three runs each, load average 66 to 77 (2026-11-18 14:23).
- From an empty build folder, `cargo test -j 3 -p wcontainers-literals --test workspace`: 24 s including the build, load 66 to 74.
- Whole crate test suite: about 12 s, most of it the git scenarios (8 to 11 s), load 60 to 70.

## Independent check, as run (bar.py on a clone of the branch, debug binary)

Run twice, the second time with the final binary, with the same results. Each check in it took 1.9 to 7.1 s wall while the load average went from 42 to 74; a plain check right after took 1.8 to 2.1 s at load 70.


- Clean tree: passes.
- One bare literal added to crates/wcontainers-kernel/src/lib.rs: fails with `+ code crates/wcontainers-kernel/src/lib.rs:686:39 ... 31337`.
- A baseline line deleted while its literal stays: fails with `+` for that literal.
- Baseline grown: fails with `>` before commit ("the working tree") and after ("added by <commit>").
- A mark with a reason on crates/wcontainers-arm64/src/decode.rs:265 `0x9ff8_0400`, its line deleted: passes, listed as `marks: 1` with the reason. Without a reason: fails with `+ mark`.
- Two branches marking literals in crates/wcontainers-arm64 and crates/wcontainers-kernel-test, each deleting its line: git merged them without conflict and the check passed.
- Second attack (tests/second_attack.rs): of 20 hiding snippets 14 are caught (byte string, tuple struct argument, joined path, -2, 1e3, match guard, byte arithmetic, include_bytes path, a function named like a test, a message that writes out a constant's value, a message with a path, a message with a number, exit(-1), a mark on a named number); 6 get through (1+1+1+1, 2<<2<<2, a constant made of twelve 2s equal to the page size, a format width, write(2, ..), kill(-1, ..)). Of 10 legitimate snippets (half, double, previous, size_of times a constant, u32::MAX and PI, a plain message, a placeholder count, a valid mark, a named mask, a message naming by constant) none is wrongly caught; mark-like text inside a string is not a mark.
- Cheats: growth on a side branch merged with --no-ff: caught (`>`). Deleting and re-adding the baseline: caught (7989 `>` lines) and init refused. A new `skip` policy line: caught (`>`, plus `-` for every line under it). An accepted literal put under `#[cfg(test)]`: caught (`~`). A shallow clone: refused. A new literal written into a file under a `generated` policy line: gets through. A side branch forked before the baseline existed that writes its own baseline and is merged (git reports an add/add conflict, resolved with the side's copy): gets through (test `a_side_branch_that_starts_its_own_baseline_gets_through`).

## Dependencies and licences

syn 2.0.119, proc-macro2 1.0.107, quote 1.0.47, toml 1.2.1: MIT OR Apache-2.0. fnv 1.0.7: Apache-2.0 / MIT. All were already in Cargo.lock; the lock gained only this crate's entry.

## Departures from the plan

- The workspace manifest needed no line: `members = ["crates/*"]` already takes the crate. Cargo.lock gained the crate's own entry.
- One baseline for the workspace, as the task asks, instead of the audit's one per crate. Growth is checked against the branch's own history in git, not against main, because each track starts its own baseline after merging.
- A policy line added after the baseline started counts as growth. There is no approval path for a new generated file or vendored folder yet; that is the orchestrator's decision.
- `list` does not propose generated files; only `init` does (printed as `proposed:`), so the audit's fixture now lists its "@generated" file as code.
- The free numbers count as no number in messages too (the audit exempted 0 and 1).
- The rule's "a name that has a constant" in text is read as a word with an upper-case letter, a digit or one of `_ - . : /` that equals the value of a string constant its crate can reach: plain English words are left alone.

## Next

- Milestone L1 is complete. Nothing further is planned in this file.
