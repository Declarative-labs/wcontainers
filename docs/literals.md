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
- `restated`: a test literal whose value is a constant its crate can name (numbers above 255, text of two or more characters), also when the test writes the number as a float with no fraction (`4096.0`) or as a string of digits (`"4096"`), and a text whether it is written as a string or a byte string (`b"etc/group"` and `"etc/group"` are one fact); a literal inside a constant whose value and a specific word of its name match an upstream constant (`LOCAL_PAGE = 4096` against `PAGE_SIZE`), or one in a crate that does not depend on it (`HOSTS_FILE = "/etc/hosts"` against `HOSTS_PATH`, `enum Granule { Page = 4096 }`; reported on the crate fewer crates depend on, else on the one whose folder sorts later; two such names in one crate are left apart, since a default equal to a maximum is two facts, and the same name with the same value is a `redefinition` instead); a message, attribute string or string constant that writes out, word for word, the value of another string constant its crate can reach (`"set WC_REFERENCE_HOST first"` where `HOST_VARIABLE` holds that name, `#[serde(rename = "st_uid")]` where a constant holds `st_uid`, `const HINT: &str = "delete it from literals.baseline"` where `BASELINE_FILE` holds the file name), or the crate's own package name, which `env!("CARGO_PKG_NAME")` gives. Only words that look like names count: with an upper-case letter, a digit, or one of `_ - . : /` (the package name counts whatever it looks like). A constant whose whole value is the word is a name of its own; `redefinition` covers it.
- `role`: -1, 0, 1 or 2 given to, or compared with, a name whose words say uid, gid, fd, pid, tid, exit, status, stdin, stdout, stderr, errno, signal, code (an exit status's or a signal's), user or owner, or one of the C library's one-word functions of them (`setuid`, `getpid`, `chown`). A name that also says count, counter, mask, len, length, total, size or number is a quantity or a set, not one id (`pid_count`, `signal_mask`). The name is a variable, a field, a function with the types it belongs to (`ExitCode::from(2)`, but not the modules it sits in), the parameter of a function or closure the file defines (`let run_as = |uid: u32| ..; run_as(0)`), the parts of a tuple bound to a tuple (`let (uid, gid) = (0, 0)`), or the function whose value it is (`fn stdout_fd() -> i32 { 1 }`, `return -1` in `fn pid`). A value wrapped in `Some` or `Ok`, taken by reference, an element of an array, or chosen by an `if`, a `match` or a block, goes to the same name; a local binding also goes to what the function later gives it to, itself, by reference or by an element (`let code = if ok { 0 } else { 1 }; exit(code)`, `let ids = [0, 0]; setuid(ids[0])`); the argument of a comparing method goes to its receiver (`uid.eq(&0)`), and a method's value is named by the method (`status.code() == Some(2)`).
- `script`: a number other than -1, 0, 1, 2 in a shell or Python script (shell syntax like `2>&1`, `$1` and `[0-9]` left out; only a one-digit number next to `<` or `>` is read as a descriptor, so `4096>size` and `$((4096>>2))` are numbers).
- `unparsed`: a Rust file the parser cannot read, so nothing in it was checked.
- `mark`: a mark that cannot stand (below).

These lists are printed by `list` but never fail: `const` (the one definition of a named constant), `test`, `generated`, `free` (the free numbers as arithmetic), `message` (text shown to a person: format strings, panics, errors, log macros, `expect`, `Display`; and the strings of attributes, which Rust requires as literals), `marked`.

A string in a message position is a message only when it reads as words for a person. It is code when, outside its placeholders, it has no space and is not one plain lower-case word: `format!("HOME={home}")`, `format!("{name}.tar")`, `"{}:{}"`, a `Display` that writes `"GET"`, `Err("EAGAIN")`, a build script's `"cargo:rerun-if-changed={}"`. A single `:` or `%` at the end is a label or a unit, not data (`"{name}:"`, `"{pct}%"`). Only the first message argument of a macro is its text; a string given as one of the values after it is code (`format!("{}=", "HOME")`). A number joined to a word by a hyphen is part of a name, not a number (`UTF-8`, `SHA-256`).

`stringify!` makes a string of its tokens, and `concat!` of literals joins them, so what either makes is one string literal like any other (`std::env::var(stringify!(HOME))` is code, and a test's `concat!("HO", "ME")` restates `HOME_KEY`). An error text returned from a closure (`map_err(|_| "too large")`, `ok_or_else`) is a message like `Err("too large")`. The attributes inside `#[cfg_attr(predicate, ...)]` are read like attributes written plainly.

The name of a variable Cargo sets, given to `env!` or `option_env!` (`CARGO_*`, `OUT_DIR`), is left out: Cargo owns the name and the macros take only a literal. Any other variable name given to them is code.

Test code is a crate's `tests/`, `benches/` and `examples/` folders (except the root of a shipped target the crate's manifest places there with `[lib] path`, `[[bin]] path` or `package.build`, and every module code declares), modules declared under `#[cfg(test)]`, and items under `#[test]`, `#[bench]` or a cfg that holds only under test. Messages in tests are messages too. A message keeps its numbers and paths gated: `"needs 64 bytes"` and `"cannot open /etc/hosts"` fail, `"cannot read {path}: {e}"` passes. Free numbers in text count as no number.

## The mark

A mark is a line comment directly above the line that holds the number, naming the number exactly as written, with the reason after a colon:

```rust
// standalone 0.577: the damping the reference implementation uses, which has no name upstream
let damped = x * 0.577;
```

Several marks may stand one above the other for several numbers on the line below them. The comment must start the line (after indentation); `///` and `//!` are documentation, never marks; mark-like text inside a string is text.

Why this form: rustfmt never moves a comment that has a line of its own, while a comment at the end of a line can end up on another line than the number when rustfmt wraps an expression; and naming the number pins the mark to that one number, so a second number added to the line later is not covered by accident.

The check refuses (lists as `mark`, which fails) a mark:
- with no reason, or one with no word in it (`.`);
- that names text or anything else that is not a number;
- with no unnamed gated number of that exact text on the line below it (a stale or misplaced mark, or a mark in test code, which needs none);
- on a number that already has a name anywhere: a constant of the same value, of any size, in any crate of the workspace or in linux-raw-sys. If the constant is a different fact that only shares the value, give the number its own name: that is always allowed;
- on a number another mark also covers, anywhere in the workspace: a number written twice is one fact, so it gets one name. Both marks fail.

Every accepted mark is printed by every run of the check, after a count (`marks: N`), with its place, number and reason, so a reviewer can judge each reason and marks cannot pile up unseen. The checker itself carries none.

## Running it

- One command, from anywhere in the workspace: `cargo run -q -p wcontainers-literals -- check`. Exit 0 means clean.
- What is scanned: every Rust file and script git lists (committed, or new and not ignored), also inside a submodule or another repository nested in the tree, except under `skip` policy lines; every target root a crate's manifest names (`path` of `[lib]`, `[[bin]]`, `[[test]]`, `[[bench]]`, `[[example]]`, and `package.build`), whatever its extension (`src/lib.txt` is Rust when Cargo builds it as Rust); and every module a scanned file declares, wherever that file lies (a hidden, ignored or skipped folder). A folder git lists as one entry that cannot be read (a submodule not checked out, a link to a folder) is an `unparsed` finding. The workspace manifest's `exclude` list takes nothing out of the check. Build output is left out by the repository's own ignore rules.
- The check cannot see its own absence. Leaving the crate out of the ordinary test run (a root `default-members`, an `exclude` of `crates/wcontainers-literals`, `cargo test` run on other packages only) or deleting it passes in silence. Whoever merges a track runs the one command below on the merge, and checks that the crate is still a workspace member.
- In the ordinary test run: `cargo test -p wcontainers-literals` (alone: `cargo test -p wcontainers-literals --test workspace`), and so in `cargo test --workspace`. That test writes the marks and the changes that could add lines past the test harness's capture, so every test run shows them, also when it passes.
- The scan runs on rayon's threads, one per core unless `RAYON_NUM_THREADS` says fewer (this machine's runs use 3).
- `cargo run -q -p wcontainers-literals -- list` prints every finding of every list, with the constants of the same value the literal's crate could import in a note (`# = PAGE_SIZE (crates/...)`: public ones, its own, and linux-raw-sys's): the work list for fixing.
- On the whole workspace (296 Rust files, 6 scripts) the check takes about 4 seconds, of which the scan is 2 to 2.3 and the rest the history in git (each change that could add lines has its older files read back). Measured on the Apple M5 Max, shared with other work, at a load average of 51 to 59: 4.1 to 4.4 s wall with the test build, 3.8 to 4.2 s with a release build. From a clean build folder, building and running the workspace test took 42 s.
- It needs git and the whole history (a shallow clone is refused), and the sources of every linux-raw-sys version Cargo.lock names, unpacked by Cargo under its home. If they are missing the check stops with an error that says to run `cargo fetch`; it never runs without them.

The check prints only what needs doing, sorted, one line each, every line ending in `do:` and the action:
- `+` a gated finding the baseline does not accept, with its file, line and column: name it as the note says.
- `-` a baseline line no longer found: delete it (this is how the baseline shrinks).
- `~` a baselined literal still in the tree but now in a list that is not gated (moved under `#[cfg(test)]`, into a test, a message or a generated file), whether its line is still in the baseline or was deleted at any point since the baseline started: it was hidden, not fixed; undo the move and name the value. A `restated` line is left out: it leaves its list when the constant it restated goes, not by being moved.
- `>` a baseline line added after the baseline started, with the commit that added it (or "the working tree"): delete it and name the value.
- `!` the baseline is not sorted: sort it (`LC_ALL=C sort`).

## The baseline

`literals.baseline` holds one line per accepted finding: list, path, kind, enclosing item and literal, separated by tabs, without line numbers, so edits elsewhere in a file do not disturb it (a literal longer than 160 characters is cut and keyed with a hash of all of it). The same line twice means the literal is there twice in that item. Lines are sorted, so two branches that remove different lines merge without touching each other. Git still conflicts when the two removed lines are neighbours; then keep neither line and run the check, which says what is left.

`policy` lines say which paths leave the normal lists: `policy<TAB>skip<TAB>PATH` (not scanned), `policy<TAB>generated<TAB>PATH` (scanned into the generated list), `policy<TAB>test<TAB>PATH` (test code in src/). A policy line is the only way a path leaves the check, and it is a baseline line like the others, so a new one fails as growth. On main the generated lines are the kernel's `src/linux/*.rs` and `src/syscall/list.rs`.

The versions of the rules: `crates/wcontainers-literals/src/rules.rs` holds one entry per version, each saying what that version gates beyond the one before. Whoever changes the checker so that it finds more appends an entry. Editing the checker without a new entry changes nothing about what the baseline may gain.

The history rule, checked on every run from git:
- The baseline starts with the first commit on the branch's first-parent history that has it.
- These changes may add any line: the start; if the start is a merge (the file arrived from another branch), the next change, which is the branch's `init`.
- A change where the rules have more versions than at the baseline's previous change (or, for the change right after a merge that brought them, than before that merge) may add lines, but only for literals that were already in the tree: at the baseline's previous change, or on a side the change merges. Any other line it adds is growth (`>` "new rules, but the literal was not in the tree before the change"). This is how new rules reach a baseline: they find literals that were there all along.
- Every other change may only remove lines. A line removed after it grew is no longer growth.
- A merge may bring lines from the other side; that side is checked the same way back to the merge base, so growth on a side branch merged in is still seen.
- A line removed by a change that could add lines is not counted as hidden: new rules may move a literal out of a gated list. Any other line removed while its literal is still there, in a list that is not gated, is reported with `~`.
- Deleting the file and adding it back is growth of every line.
- The file has one start in the whole history. A second start that is not a descendant of the first (a branch that wrote its own baseline, wherever it was merged) fails (`>` "started more than once").
- Diffs are read as text whatever `.gitattributes` says, and any git attribute set on the baseline fails (`>`): marking it binary would otherwise hide its diffs. Git's replace refs are not read, so a graft cannot turn a growth into the start.
- Every run lists every change, on every side of every merge, that could add lines, with why ("changes that could add lines to the baseline: N", then one per line). When the branch has an upstream (`@{upstream}`, as a clone of a pushed branch does), every such change on the upstream's first-parent history must also be one on the branch's: a replaced start fails (`>` "the start was replaced").

## How a track adopts the check

1. Merge main into the track's branch once main has the check: the orchestrator first merges `phase-1-literals` into main and runs `init` there, so main's baseline holds every track's literals that are on main. A track does not merge `phase-1-literals` directly: if it did and later merged a main that lacks the check, or that holds code of tracks the check has not seen, the check would report `+` lines in crates the track may not edit, which it can neither fix nor add. A track that already did so does not merge main until main has the check. Then the merge brings main's lines, which main's own init accepted; where the baseline conflicts, keep the lines of both sides, run the check, and delete the `-` lines it prints.
2. Run `cargo run -q -p wcontainers-literals -- init` once, either before committing the merge or as the next change to `literals.baseline` after it. It rewrites the baseline from the branch's tree, keeps the policy lines it had, and adds a `generated` line for each file whose first five lines say "@generated", "do not edit" or "automatically generated" (printed as `proposed:`). Read every proposed line; anything other than the kernel's generated Linux files goes to the orchestrator before it is committed.
3. Commit the baseline. From then on `init` refuses, and `cargo test -p wcontainers-literals` must pass before every push. Record in the track's status file the list the check prints under "changes that could add lines to the baseline". Whoever reviews or merges the track compares the list with it: a new entry must be a start or init of another track merged in, or a change of the checker's rules from the literals track.
4. When a merge brings a new version of the rules and the check then finds literals of the track that the old rules did not, run `init` before committing the merge or as the next change of the baseline, and commit. That change may add lines only for literals already in the tree; the run lists it.
5. To fix a literal: name it, run the check, delete the `-` line it prints, in the same commit.
6. Never add a line. A new value gets a name; a number that truly stands alone gets a mark.
7. A fact owned by another track's crate is never defined locally while waiting for the owner: its line stays until the owner's constant is on main.
8. Naming a value can make other crates' tests restate it: once a constant of that value exists upstream of them, their test literals of it become `restated` findings, and a track may not edit another track's tests. Before naming a value, run `list` and read the notes (`# = NAME`) on that value's test literals in other crates; where any would become `restated`, either leave the value's line for now and tell the owner of those tests, or name it in the same change as the merge that brings both. Never add their lines to the baseline.

## What the check cannot see

A person still has to look at these. Each one the attacks found has a test that records it (`tests/attack.rs`, `tests/second_attack.rs`, `tests/third_attack.rs`, `tests/fourth_attack.rs`, `tests/fifth_attack.rs`, `tests/sixth_attack.rs`, module `gets_through`; `tests/baseline.rs`), so an improvement shows as a failing test to update.

- A role given a value built from free numbers (`let stderr_fd = 1 + 1`), and one fact written twice inside one constant (`const X: [u64; 2] = [4094, 4094]`).
- A test that writes a named value as an expression (`assert_eq!(PAGE_SIZE, 4 * 1024)`); a value computed at run time is no constant, so a test that writes it out is not compared with anything (`format!("{NEW_MARK}code")` where `List::Code.name()` gives `code`).
- Two constants of one crate with the same value under names that share a word (`MQ_MSGSIZE_DEFAULT` and `MQ_MSGSIZE_MAX`, both 8192): too often two facts to gate.
- A literal in a submodule enters the baseline only at its start or the init after it arrives: a new version of the rules cannot add its line, since the history reads the superproject's commits, which hold only the submodule's commit.
- A note does not name a constant that a private module holds and a `pub use` re-exports; it names only constants whose every module is `pub`.
- A free number with no role in sight: `Duration::from_secs(1)`, `from_millis(1)`, `return 0` as a success code from a function whose name says nothing, `!0`, a 1 in a `macro_rules!` body, `x.max(1.0)`, a value in a `match` arm of a function named for a role.
- -1 or 2 given to a function whose name, and the types it belongs to, do not say what it takes and that the file does not define: `write(2, ..)` for stderr, `kill(-1, ..)` for every process, `signal::sender_info(sig, code, 0, 0)` (a module's name is not read).
- A value built only from free numbers or from the standard library's own numbers: `1 + 1 + 1 + 1`, `2 << 2 << 2`, `1u64 << (u8::BITS + 2 + 2)`, `1u64 << (u8::BITS + u8::BITS / 2)`, a constant of twelve 2s multiplied that equals the page size, or booleans used as numbers (`true as u64 + true as u64`).
- A number with no literal at all: `line!()` (fixed by the file's layout), an enum's implicit discriminant used as a number (`Implicit::E as u32` is 4).
- The same fact under another name with a small value (`enum Signal { Segv = 11 }`), a constant whose name shares no specific word with the owner's (`DIR_VARIABLE = "HOME"` beside `HOME_KEY`), or a value written as an expression of small numbers inside a constant (`1 << 12`). Floating-point constants are not compared by value at all, so `const DAMPING: f64 = 0.577` defined twice is not a redefinition.
- A test that writes a value the code names, when the value is 255 or less (`assert_eq!(sig, 9)` with `SIGNAL_KILL = 9`): rule 5 says it is a violation, but small numbers are too often other facts to gate them on their value alone. It is listed in the `test` list with the constant in its note (`# = SIGNAL_KILL`), so a reviewer can read them there.
- A linux-raw-sys value retyped under another name in a crate that does not depend on linux-raw-sys (`OPEN_CLOEXEC = 0o2000000`).
- A literal moved within the same function, or into a closure of it (baseline lines are per enclosing item).
- Text that hides a value: format widths and precisions (`{:08x}`, `{x:>12}`), numbers written as words ("needs sixty-four bytes"), a decimal in a message (`"needs 4.5 GiB"`, read like a version such as "Linux 6.8"), numbers inside strings parsed later (WGSL, JSON, `"0x1000".parse()`), and numbers or paths in attribute strings (`#![recursion_limit = "256"]`, `#[path = "..."]`), which are gated only when they write out a string constant.
- Data that reads as a sentence: a string with a space in a message position that a program parses (`format!("Content-Length: {n}\r\n")`, a command line `format!("{} && mkdir -p {}", ..)`), and a key or protocol word that is one plain lower-case word (`Err("eof")`, `write!(f, "get")`). Only strings without a space, other than one plain lower-case word, are read as data.
- A string constant made only of punctuation (`"::"`, `"/"`, `"# "`, `'\n'`) written out inside a message or inside another string constant: words are compared with constants only when they look like names, and punctuation is never a word.
- Paths and other strings in scripts: only their numbers are checked (`open("/etc/hosts")`, `cat /etc/hosts`).
- Wrongly caught: messages passed to a crate's own message helpers (`fail(...)`) are gated like any code string; a count over a total (`format!("{done}/{total}")`) reads like a path joined by hand, and a range (`format!("{a}..{b}")`) like data; a crate whose package name is a plain English word has that word gated in its messages.
- Output of derive and procedural macros, `include_str!` data, and other languages: C, JavaScript, WGSL, HLSL, PowerShell, TOML (including Cargo.toml versions).
- New literals written by hand into a file under a `generated` policy line: whoever owns the generators must review those.
- A mark inside a `/* */` block comment is read as a mark. A mark's reason is never judged beyond having a word: "x" is a reason. Every run lists every mark with its reason so that a person judges them.
- A change that could add lines is allowed whoever makes it. A side branch made before the check arrived can merge the literals branch, run init with new literals in its tree, and be merged into a track that already adopted the check: its init is a start of another branch, which the rules accept as they accept another track's adoption merged through main. The check passes; what shows is the list of changes that could add lines, which every run prints (the test run too) and which then names one more commit. Whoever merges a track compares that list with the one the track recorded (adoption, step 3).
- A made-up version of the rules: a branch that commits a new literal together with any other change of the baseline (the check fails at that commit), then appends an entry to `rules.rs` and adds the literal's line, passes, because the literal was in the tree at the baseline's previous change. It shows in the list of changes that could add lines, and as an edit of the checker's crate, which belongs to the literals track.
- A second adoption on a branch with no upstream: a fork from before the track adopted the check merges the literals branch, runs init with new literals, then takes the track in and replaces it by a fast-forward. Where the upstream still holds the earlier push the check fails; elsewhere only the list of changes that could add lines shows it.
