# Status: engine-wasmedge

The WebAssembly engine interface (crates/webassembly-engine) implemented on WasmEdge, every module compiled ahead
of time by its LLVM compiler. Crate: crates/webassembly-engine/implementation/wasmedge, package
wcontainers-webassembly-engine-wasmedge.

## Where things stand (2026-11-23, run 5: milestone E1f, run 1)

- Done: every trait of the interface, tags included since run 4. On the rented ARM64 machine, against the stock
  WasmEdge 0.18.1 release (run 5: tree 1f4682e3 at 13:37 UTC, and tree cbe454f9, which renames one test global,
  at 13:53 UTC; 2026-11-23, load 10 to 22): the suite's 29 tests, the 6 proposal checks and the crate's own 21 checks pass, and clippy with
  warnings denied is clean. Without the feature and with no library, clippy on all targets and `cargo test` pass
  (scratch engine-wasmedge/run5/job.sh, logs run5/job1.log and run5/job2.log). Run 5 did not rerun x86-64; run 4
  passed there (tree 89148cf4).
  The crate inherits the workspace lints (unsafe forbidden), has no unsafe and no exemption. Since run 5 it
  also builds no WasmEdge value from raw bits (a check of its own says so). The interface and suite files are
  the same as on main (`git diff main` over them is empty, run 5).
- The engine is behind the crate's feature `wasmedge`, off by default: without it the crate is empty, so the
  workspace builds and checks where no WasmEdge is installed (see "Fixed in run 4"). Every command below that
  builds the engine passes `--features wasmedge`.
- The repository's literal check has no finding in this crate (run 5, ARM, tree cbe454f9; scratch
  engine-wasmedge/run3/lit.sh, log run5/lit2.log). The literal lines left under crates/webassembly-engine are in
  the wasmtime crate, the interface and the suite, none of them mine. The check as a whole still fails on the
  branch, with about 5000 lines in other tracks' crates.
- Run 5 fixed the findings of the independent check of run 4 (see "Fixed in run 5"), run 4 those of run 3, run 3
  those of run 2, and run 2 those of run 1.
  The checker's five cases of run 2 (scratch engine-wasmedge/check2/check3.rs) pass on both WasmEdge and
  wasmtime (10 of 10, ARM, 2026-11-23 11:28 UTC, load 4.9 to 11; job run3/job-and-check3.sh, log run3/job4.log).
- Against 0.17.1, the release the bindings pin: the suite and the crate's checks pass (run 2), and the
  exception-handling proposal check fails (see "Proposals").
- Not done: the suite on the Mac (the notice of 2026-11-22 keeps builds off it).

## Fixed in run 5 (the independent check of run 4)

- Blocking: every reference read from a table or a global was rebuilt from a raw FFI struct
  (`From<ffi::WasmEdge_Value> for WasmValue`, in `passable`), a conversion that can forge any reference. My
  statement that no safe route existed was wrong: the checker showed one, and it is now built. A reference read
  from a slot whose type is not `funcref` goes to a holder, a compiled module of ours. Its type section begins
  with the slot module's own types, and its `set_item` takes the slot's exact reference type and stores the
  reference in its own funcref table. Read back from there, the reference has the outer type `funcref` and still
  carries the function's own type. WasmEdge itself checks the parameter at the call, so nothing of ours decides
  the type. One holder serves every slot of one reference type in modules with the same types (it is cached by
  its compiled module), and each use counts `SlowPath::ReferenceHeld`. A reference read from a plain `funcref`
  slot is passed on as WasmEdge gave it. The crate no longer names `WasmEdge_Value` at all; the only `ffi` item
  it names is the error code of an uncaught exception, a constant.
  Tested: `a_reference_read_from_a_typed_slot_carries_the_functions_type_and_no_other`. A reference read from a
  typed table and one from a typed global (of a type at index 1) each go into another module whose types are in
  the other order, into its plain table and its table of the same type. That module's `ref.test` answers 1 for
  the function's type and 0 for the other type; `call_ref` calls it. Each read reference is refused by a table
  and by a global of the other type (`ReferenceTypeMismatch`, `ValueTypeMismatch`). It is imported with its own
  type and called, and refused as an import of the other type. Another engine value refuses it
  (`ForeignAddress`). `the_crate_builds_no_engine_value_from_raw_bits` fails if implementation.rs names
  `WasmEdge_Value` again.
  Cost (ARM, run 5, "Measurements"): a read from a typed table went from 105 to 1408 ns, and one from a typed
  global from 104 to 1388 ns. Reads from plain slots are unchanged within noise (105 to 120 ns and 104 to 98 ns).
- The unsafe count of wasmedge-sys was taken over an incomplete copy. The Mac's copy (388) holds only src/. The
  crate as published on the rented machine gives the stated command's 395: bindings/wasmedge.rs adds 7. That
  file is a pre-generated copy that is not compiled. The ffi module that is compiled is the file bindgen
  writes at build time, which was not counted before. Against 0.18.1's headers it has 2536 lines and the word
  `unsafe` only 7 times, all in function pointer types (`unsafe extern "C" fn`). That count understates it: the
  file declares 299 foreign functions in 299 plain `extern "C"` blocks, and every one of them is unsafe to call.
  Of src/, the async module (27) is not compiled, because the crate leaves the `async` feature off. Details are
  under "Dependencies".
- The host context was moved by value into its cell and back around every call into a module and every host
  call, so the cost grew with its size. Now the engine value holds it boxed (`Option<Box<HostContext>>`, made
  once in `new`), and each move is of one pointer. Per call into a module, what is left of ours is two
  pointer-sized moves (out of the engine value into the shared cell, and back). Per host call it is two more,
  plus the test that the context is there. That test makes a host function run outside a call fail instead of
  panicking. None of these can be removed without unsafe or a run-time borrow check, both of which condition (1)
  forbids. Measured with the new 4 KiB context in the benchmark (ARM, run 5): an empty call went from 1422 ns
  to 1129 ns, about what a call with a u64 context costs (1101 before, 1111 after). The 1000 host calls went
  from 398.5 µs to 397.1 µs, so the old copies cost about 1.4 ns per host call there.
- The feature `wasmedge` stays as it is, for the owner to rule on ("For the owner" 8). The suite crate's three
  `clippy::ok_expect` errors stay recorded ("For the owner" 9 (6)); that file is not mine.

## Fixed in run 4 (the independent check of run 3)

- Tags were missing, with a wrong reason given ("the bindings have no type for them"). A tag needs no type of the
  bindings: it links by name like the other items. `TagAddress` is now an export (owner and name), `find_export`
  answers a tag, and `instantiate` links an imported tag through the forwarder, with the type the importing module
  declares. Tested: `tags_link_between_modules_as_the_tag_they_are` (a second module catches what the first throws
  through the imported tag; the same through the tag the catcher exports again; another instance's tag of the same
  type does not catch it, `InvokeError::Exception`).
- The crate did not build where no WasmEdge is installed, and it is a workspace member, so whole-workspace
  commands failed (another track's `cargo check --workspace` on the ARM machine did). My earlier statement that
  nothing had broken was false. Now the bindings are optional dependencies under the feature `wasmedge`, off by
  default; implementation.rs is `#![cfg(feature = "wasmedge")]` and the tests and the benchmark have
  `required-features = ["wasmedge"]`. Without the feature the crate is empty and keeps `[lints] workspace = true`.
  Checked on ARM with no library and no `WASMEDGE_DIR`: clippy on all targets and `cargo test` pass (run4/job.sh).
  Of the three choices I had put to the orchestrator this is (b); (a) leaves every other track's command to
  remember an exclusion, and (c) would cost the inherited lints.
- An address made by one engine value and given to another was resolved by its owner index in the wrong engine
  value, a silent wrong link. Now every address and instance carries the identity of the engine value that made it
  (a number from a process-wide counter). `instantiate` answers `InstantiateError::ForeignAddress` for such an
  import; every other method panics, as wasmtime does for an object of another store. Tested:
  `an_address_another_engine_value_made_is_refused_at_instantiation`, `a_memory_another_engine_value_made_is_refused`,
  `a_function_another_engine_value_made_is_refused_when_bound`. The check is one comparison of two numbers per
  access, also on the hot paths.
- The re-wrapping of references read from tables and globals (`passable`) was listed for the owner as a
  conversion from a raw FFI struct that can forge any reference. Run 5 replaced it with the holder.
- Two wrong statements here: the shared memory across two threads is a check of beyond_the_suite.rs, not of
  proposals.rs; and `measure` differs from the wasmtime crate's in the import's names (host.upcall against
  env.hook), which changes no figure.

## Fixed in run 3 (the independent check's findings)

- A function the host wrote into a plain funcref table or global, or gave `allocate_global`, carried only
  `funcref`, so a guest's `ref.test` and `ref.cast` to its type failed. Now every function reference the host
  writes, plain slots included, goes through the retyper first. Tested:
  `every_function_the_host_writes_carries_its_own_type` (a module's function and a host function, into a plain
  table, a typed table, a plain global and an allocated global, then `ref.test`, `ref.cast` + `call_ref` by the
  guest; a reference read back from a global keeps its type).
- A host function could not go into a typed slot. The retyper of a host function now takes its type from the
  function's own value types, which are numbers only; the slot's writer then casts. Tested in the same check.
- A function no instance exports (read from a table) could not be imported. Now a pass-on helper holds it in a
  typed table and exports a function of the import's type that calls it with `call_indirect`. A function of
  another type fails the helper's cast, answered `InstantiateError::FunctionTypeMismatch`. Tested:
  `a_function_read_from_a_table_is_imported`.
- Slow paths had no counter and no warning. Now each has a named reason (`SlowPath`, with `reason()`), a counter
  per engine value (`WasmEdgeEngine::slow_path_count`), and a `log::warn!` the first time the process takes it.
  The paths: shared memory copied, unshared memory of a shared owner copied, bytes beyond the bindings' 32-bit
  reach copied, table written by a module, global set by a module, function retyped, function not retyped (no
  module describes its type), imports forwarded, function passed on, and since run 5 reference held. Tested: `slow_paths_are_counted`, and the
  counts in the two checks above.
- The crate added 53 lines to the literal check. Now none: the folder prefix and helper names come from
  `env!("CARGO_PKG_NAME")`, the folder mode from `rustix::fs::Mode::RWXU`, the compiled file's name is a constant,
  the helpers' item names no longer equal the wasmtime crate's or the translator's constants, and the tests use
  names no constant holds and the interface's `PAGE_SIZE`.
- The unsafe count of wasmedge-sys: 388. This was wrong, taken over a copy that holds only src/. Run 5 corrected
  it to 395 for the crate as published, and also counted the bindings generated at build time (see "Fixed in
  run 5").

## Fixed in run 2 (the independent check's findings)

- Writing a global, or writing or growing an unshared memory, of an instance that also exports a shared memory
  panicked. Now a global setter helper (imports the global, `global.set`) and a copier for the unshared memory do
  it. Tested: `the_items_beside_an_exported_shared_memory_are_written`.
- `find_export` panicked on an exported shared 64-bit memory. The copier now comes in four shapes (shared or not,
  32 or 64 bits); a helper that cannot be made makes `find_export` answer nothing (absent), never a panic.
  Tested: `a_shared_64_bit_memory_is_found_shared_and_imported`.
- Importing a 64-bit memory or a table of typed function references failed. Each module's declarations are now
  read with wasmparser, and the module that passes imports on declares each one with the importing module's own
  type and type section. Tested: `a_64_bit_memory_is_imported_and_allocated`,
  `typed_function_references_are_written_into_tables_and_globals`.
- Writing a typed function table answered a false `ReferenceTypeMismatch`, and growing it failed. The table writer
  now imports the table with its exact element type and casts. Writing a function of another type answers
  `ReferenceTypeMismatch`; writing past the end answers `IndexOutsideTable`.
- Each compiled library went to a predictable path in the shared temporary folder. Now it goes into a new private
  folder (tempfile, mode 0700), removed after loading.
- Host-made 64-bit memories and tables, and bytes beyond the first 4 GiB of a memory, were answered as absent or
  panicked. Now a host memory or table is a small module of ours that defines it. Bytes beyond 4 GiB go through the
  64-bit copier. Tested: `bytes_beyond_4_gib_of_a_64_bit_memory_are_read_and_written` (65537 pages, at
  4 GiB - 4 and 4 GiB + 8), and `a_64_bit_table_is_allocated_written_and_grown`.
- The dependency list was incomplete. It is now complete, with licences and unsafe counts (see "Dependencies").
- The wasmtime track's point 6 was unchecked. It is confirmed (see "For the owner" 9).

## How to run

The bindings find the library where `WASMEDGE_DIR` points (or WasmEdge's usual install folders); nothing is
downloaded at build time. On both rented machines the releases are unpacked in /work/scratch-engine-wasmedge/<version>
(the stock manylinux_2_28 archives from WasmEdge's GitHub releases; 0.17.1's sha256 is the one the bindings pin).
Under the Nix loader the library's libz and libstdc++ are not found, so /work/scratch-engine-wasmedge/system-libraries
holds links to the system's two and goes on `LD_LIBRARY_PATH`.

- Clippy and tests: `arm-run <checkout> --name engine-wasmedge -- "$(cat run5/job.sh)"` from scratch
  engine-wasmedge (run5/job.sh also counts the unsafe of wasmedge-sys, published and generated, and greps this
  crate for unsafe and exemptions; run5/bench.sh takes this crate's benchmark alone, twice). It first builds, lints and tests the crate with no library and no feature (the empty crate
  every other track builds), then with `--features wasmedge` against 0.18.1 (the exports of WASMEDGE_DIR,
  LD_LIBRARY_PATH and CARGO_TARGET_DIR come first). run-tests.sh defines `run <version> [cargo test arguments]`;
  the older scripts of runs 1 to 3 lack `--features wasmedge` and now build the empty crate.
- Literal check (needs the git history, which arm-run does not copy): run3/lit.sh. Commit, then send a bundle of
  the new commits since the rented machine's clone (`git bundle create head.bundle HEAD ^<its commit>`) and the
  patch of uncommitted changes to /work/scratch-engine-wasmedge with `arm-run --ssh 'cat > ...'`, then
  `arm-run --ssh 'bash -s' < run3/lit.sh`.
- Verification (the suite crate's clippy, the checker's 15 cases on both engines, then 0.17.1):
  `arm-run <checkout> --name engine-wasmedge -- "$(cat run2-verify.sh)"`.
- Whole clean check (clean build, tests, clippy, unsafe grep, unsafe probe): check.sh, `WASMEDGE_VERSION=0.17.1`
  for the older release.
- Benchmark: `cargo bench -p wcontainers-webassembly-engine-wasmedge --features wasmedge`. `measure` in
  benchmark.rs is the wasmtime crate's but for the import's names (host.upcall against env.hook), which change no
  figure; `measure_helpers` adds the helper routes. bench.sh runs both crates' benchmarks in one job.
- Dependencies with licences and unsafe counts: deps.sh (output deps-run2-clean.txt).
- Direct comparison (outside the repository, has unsafe): scratch engine-wasmedge/direct-bench.

## Design

- Bindings: wasmedge-sdk 0.17.1 over wasmedge-sys 0.21.1, their safe layer only. wasmedge-sys runs bindgen over the
  installed headers, so the same bindings build against 0.18.1.
- `Config`: two WasmEdge configurations, sharing a cache of compiled helper modules keyed by their bytes:
  - one for compiled modules, in AOT run mode;
  - one for our modules that hold no code (forwarders, host-made memories and tables, retypers).
  `Default` turns on every proposal the translated modules use. It also raises WasmEdge's memory cap from 65536
  pages to the most the bindings can set (see "For the owner" 13).
- `compile` writes a native shared library into a private folder, loads it, validates it, and reads the module's
  declarations with wasmparser. The declarations are its type section (re-encoded with wasm-encoder) and the exact
  type of each import and export. WasmEdge loads a native library as native code or not at all: the loader falls
  back to the interpreter only for universal wasm files, so no module this crate compiles is ever interpreted.
- Linking: WasmEdge links imports only by module and item name, through a store. `instantiate` makes a store of its
  own and registers in it every instance that owns an import. Under each module name the module imports from, it
  registers a generated forwarder. The forwarder holds the importing module's type section, imports each item from
  its owner with the type the importing module declares, and exports it under the name asked for. A function no
  instance exports (one read from a table) is linked through a pass-on helper (below).
- Addresses: a function is WasmEdge's reference value plus where it is exported. Memories, tables, globals and
  tags are their owner (an index into the engine value's instances) and export name, with how this engine value reaches
  them; the name is looked up on each access, because the bindings' handles borrow their instance. Instances live as
  long as the engine value; when it is dropped, function addresses still held keep them alive. Every address and
  instance also carries the identity of the engine value that made it, and another engine value refuses it.
- Calls: `FunctionTypeBinder<(A1..An), R>` covers n from 0 to 17, and results that are one bare value or a tuple of
  0 to 17. Value types: i32, i64, f32, f64, i128 (v128). One macro generates it, bounded by the bindings' own
  `WasmValType` and `WasmVal` and by `From<Received>`, the conversion back, which the bindings lack. A call goes
  through `Executor::call_func_ref` with the parameters in an array.
- Host functions: one module per host function. Its data is the closure and a shared slot where the host context is
  lent during a call. The context is boxed once, so lending it moves a pointer.
- Helpers, each a module of ours compiled once per Config and linked to one item per engine value. They reach what
  the bindings' safe layer cannot:
  - the copier (`copy_out`, `copy_in`, `grow_item` through an unshared scratch memory), for shared memories, for unshared
    memories of a shared owner, and for bytes beyond 4 GiB;
  - the table writer (`set_item`, and `grow_item` when elements may be null), for every table of function references;
  - the global setter, for globals of a shared owner and globals of typed function references;
  - the retyper, which holds a code-free table filled by `ref.func`, for every function reference the host writes
    (see "For the owner" 12);
  - the pass-on helper, a compiled module whose typed table holds a function no instance exports and whose
    exported function of the import's type calls it through `call_indirect` (see "For the owner" 6);
  - the holder, a compiled module whose `set_item` takes a reference of a typed slot's own type and stores it in
    a funcref table, from which the host reads it back as `funcref` (see "For the owner" 12).
  Each is a `SlowPath` with a reason, a counter per engine value (`slow_path_count`) and a `log` warning the
  first time the process takes it. The crate installs no logger: the program that uses it chooses where warnings
  go.
  A helper never makes the hot paths of unshared memories, calls or host calls slower: those are unchanged.
- An instance that exports a shared memory is held shared (`Arc`), so that other engine values can add the memory.
  Its unshared memories are still read directly. They are written and grown through their copier, and its globals
  are written through their setter.
- Traps are `Box<WasmEdgeError>`. An uncaught exception is WasmEdge's error code UncaughtException, answered
  `InvokeError::Exception`.

## Proposals (each its own module in proposals.rs, compiled ahead of time through the interface)

On the ARM machine, released libraries, 2026-11-23 (rechecked in run 2):
- 0.18.1: every check passes:
  - tail calls: return_call ten million deep;
  - typed function references: call_ref, return_call_ref, ref.as_non_null;
  - exception handling: try_table, throw, catch_ref, throw_ref, an uncaught exception;
  - threads: atomic add, wait, notify and fence on a shared memory, in one thread (two threads sharing one module
    and one shared memory is `two_threads_share_one_module_and_one_shared_memory` in beyond_the_suite.rs);
  - 64-bit memories: loads, stores, size, grow, an out-of-bounds trap;
  - multiple memories: memory.copy between two.
  The checker's cases add return_call_indirect, br_on_null, catch_all_ref and wait/notify across two engine
  values; all pass. Beyond 4 GiB of a 64-bit memory works once the memory cap is raised.
- 0.17.1, the release the bindings pin: everything passes except exception handling, which does not compile ahead
  of time (`Core(UnknownError(14))` from the compiler), as the survey's first reader said.
- Not checked: the legacy exception opcodes (try, catch, rethrow). WasmEdge reserves them, and the survey says it
  rejects them.

## Measurements

Crate benchmark, through the interface, release builds. The WasmEdge crate and the wasmtime crate run in turn in one
job, two rounds each; each figure is the lower of the two rounds, and each round is the best of 5.

Run 5, rented ARM64 machine, WasmEdge 0.18.1, the WasmEdge crate alone, two rounds; each figure is the lower of
the two rounds, and each round is the best of 5. Before: tree d0b13b7d with run 5's benchmark, 13:25 to 13:37 UTC,
load 10 to 17 (log scratch run5/bench-before.log). After: tree 1f4682e3, 13:38 to 13:50 UTC, load 8 to 32 (another
track's build rose to 32 at the start of round 2; log run5/bench-after.log). Script run5/bench.sh.
- read a function from a typed table: 105.0 ns before, 1408 ns after (the holder)
- read a function from a plain table: 105.0 ns before, 119.7 ns after
- read a function from a typed global: 104.1 ns before, 1388 ns after (the holder)
- read a function from a plain global: 103.9 ns before, 98.3 ns after
- an empty call into the module with a 4 KiB host context: 1422 ns before, 1129 ns after (the boxed context)
- 1000 calls of an empty host function with a 4 KiB host context: 398.5 µs before, 397.1 µs after
- with the benchmark's u64 context: empty call 1101 / 1111 ns, 1000 host calls 396.5 / 397.7 µs; every other
  figure of the crate benchmark is within 2 % of its value before, except compiling (6.37 to 6.11 ms) and the
  64-byte unshared write and read (79.2 / 72.2 to 75.9 / 69.3 ns), which are about 4 % lower after, on paths
  run 5 did not change
- I did not find the cause of the plain table read's 15 ns: on that path only the address's new field changed.
wasmtime was not rerun in run 5; run 4's figures below are the comparison.

Run 4, rented ARM64 machine, load average 12.5 to 18 (other tracks' jobs), 2026-11-23 12:44 to 12:56 UTC, tree
2e96eb11 (adds the engine identity check on every access), WasmEdge 0.18.1, then wasmtime 50.0.1 in the same job
(scratch run4/bench.sh, log run4/bench.log):
- compile the benchmark module: WasmEdge 5.87 ms, wasmtime 200 µs
- instantiate it (one import): WasmEdge 6.54 µs, wasmtime 489 ns
- an empty call into the module: WasmEdge 1094 ns, wasmtime 18.2 ns
- an empty host function called from a module loop: WasmEdge 396 ns per call, wasmtime 4.41 ns
- unshared memory, write / read 64 B: WasmEdge 74.6 / 69.7 ns, wasmtime 7.3 / 6.9 ns
- unshared memory, write / read 64 KiB: WasmEdge 832 / 826 ns, wasmtime 761 / 762 ns
- shared memory, write / read 64 B: WasmEdge 1421 / 1425 ns, wasmtime 41.8 / 55.1 ns (both through a copier)
- shared memory, write / read 64 KiB: WasmEdge 3301 / 3267 ns, wasmtime 1812 / 1803 ns
- table write: WasmEdge 1221 ns, wasmtime 17.5 ns
- WasmEdge only, the helper routes: global setter 1191 ns; unshared memory beside a shared one, write / read
  64 B 1429 / 66.6 ns; a function into a typed table 1261 ns, one read from a table 1265 ns; an import of an
  exported function 1260 ns, one passed on 1258 ns.
The identity check costs nothing measurable: every figure is within run 3's, the 64-byte unshared reads and
writes 2 to 4 ns lower at a higher load.

Run 3, rented ARM64 machine, load average 4.8 to 7.9 (other tracks' jobs), 2026-11-23 11:29 to 11:41 UTC, tree
ef9fb9fc, WasmEdge 0.18.1, then wasmtime 50.0.1 in the same job (log scratch run3/bench-arm2.log):
- compile the benchmark module: WasmEdge 5.72 ms, wasmtime 202 µs
- instantiate it (one import): WasmEdge 6.54 µs, wasmtime 489 ns
- an empty call into the module: WasmEdge 1095 ns, wasmtime 18.2 ns
- an empty host function called from a module loop: WasmEdge 395 ns per call, wasmtime 4.31 ns
- unshared memory, write / read 64 B: WasmEdge 78.6 / 71.7 ns, wasmtime 7.4 / 6.9 ns. Run 2 gave 72.5 / 66.0; no
  code on this path changed in run 3, and I did not find the cause of the 6 ns.
- unshared memory, write / read 64 KiB: WasmEdge 833 / 827 ns, wasmtime 765 / 762 ns
- shared memory, write / read 64 B: WasmEdge 1442 / 1456 ns, wasmtime 41.9 / 54.7 ns (both through a copier)
- shared memory, write / read 64 KiB: WasmEdge 3299 / 3265 ns, wasmtime 1806 / 1804 ns
- table write: WasmEdge 1214 ns, wasmtime 17.2 ns. The write now goes through the retyper too, whose reference
  each address keeps after its first write, so it costs what run 2's did.
- WasmEdge only, the helper routes:
  - write a global of an instance that exports a shared memory (setter): 1193 ns;
  - write / read 64 B of an unshared memory of such an instance: 1458 ns (copier) / 68.6 ns (direct);
  - write a function into a typed table: 1258 ns; the same with a function read from a table, which needs no
    retyper: 1260 ns;
  - call an import of an exported function: 1260 ns; call an import passed on to a function no instance exports:
    1261 ns, so the pass-on's indirect call costs about 2 ns.
- Before each address kept its retyped reference, the first run-3 tree took 1969 ns for a table write: making the
  retyper's bytes on every write cost about 750 ns.

Run 2, rented ARM64 machine, load average 8.5 to 15 (other tracks' jobs), 2026-11-23 10:12 to 10:23 UTC, tree
8d8794d5, WasmEdge 0.18.1, then wasmtime 50.0.1 in the same job:
- compile the benchmark module: WasmEdge 6.38 ms, wasmtime 198 µs
- instantiate it (one import): WasmEdge 6.50 µs, wasmtime 491 ns. Run 1 gave 5.42 µs; reading the forwarder's
  declarations and loading each forwarder with its own loader account for about 1 µs.
- an empty call into the module: WasmEdge 1051 ns, wasmtime 18.2 ns
- an empty host function called from a module loop: WasmEdge 395 ns per call, wasmtime 4.39 ns
- unshared memory, write / read 64 B: WasmEdge 72.5 / 66.0 ns, wasmtime 7.3 / 6.9 ns. Run 1 gave 69.4 / 61.5 at a
  higher load; the path gained one test of a flag.
- unshared memory, write / read 64 KiB: WasmEdge 831 / 818 ns, wasmtime 760 / 763 ns
- shared memory, write / read 64 B: WasmEdge 1443 / 1425 ns, wasmtime 42.1 / 54.6 ns (both through a copier)
- shared memory, write / read 64 KiB: WasmEdge 3320 / 3268 ns, wasmtime 1810 / 1798 ns
- table write: WasmEdge 1219 ns, wasmtime 17.3 ns

Run 2, rented x86-64 machine (Xeon Platinum 8488C, 96 cores), load average 0.8 to 5.2, 2026-11-23 10:15 to 10:25
UTC, tree 6067f76a (adds `measure_helpers`), same job shape:
- compile: WasmEdge 6.12 ms, wasmtime 245 µs
- instantiate: WasmEdge 6.31 µs, wasmtime 387 ns
- empty call: WasmEdge 862 ns, wasmtime 12.7 ns
- host call: WasmEdge 303 ns per call, wasmtime 4.00 ns
- unshared write / read 64 B: WasmEdge 64.9 / 57.3 ns, wasmtime 4.1 / 4.2 ns
- unshared write / read 64 KiB: WasmEdge 1337 / 1334 ns, wasmtime 1294 / 1293 ns
- shared write / read 64 B: WasmEdge 1159 / 1141 ns, wasmtime 36.9 / 37.9 ns
- shared write / read 64 KiB: WasmEdge 3772 / 3797 ns, wasmtime 2622 / 2683 ns
- table write: WasmEdge 970 ns, wasmtime 12.6 ns
- WasmEdge only, the helper routes:
  - write a global of an instance that exports a shared memory (setter): 946 ns;
  - write / read 64 B of an unshared memory of such an instance: 1161 ns (copier) / 61.0 ns (direct);
  - write a function into a typed table (retyper, then writer): 1095 ns, so about 125 ns more than a plain table
    write once the retyper exists.
  wasmtime has no such figures: it writes all four directly.

Run 1 (tree ec93a63d), rented ARM64 machine (Graviton4, 96 cores), load average 17 to 22 (other tracks' jobs),
2026-11-23 09:02 to 09:13 UTC, WasmEdge 0.18.1, then wasmtime 50.0.1 in the same job:
- compile the benchmark module: WasmEdge 6.37 ms, wasmtime 207 µs
- instantiate it (one import): WasmEdge 5.42 µs, wasmtime 489 ns
- an empty call into the module: WasmEdge 1047 ns, wasmtime 18.2 ns
- an empty host function called from a module loop: WasmEdge 398 ns per call, wasmtime 4.39 ns
- unshared memory, write / read 64 B: WasmEdge 69.4 / 61.5 ns, wasmtime 7.3 / 6.8 ns
- unshared memory, write / read 64 KiB: WasmEdge 821 / 817 ns, wasmtime 767 / 771 ns
- shared memory, write / read 64 B: WasmEdge 1409 / 1402 ns, wasmtime 42.1 / 55.7 ns (both through a copier)
- shared memory, write / read 64 KiB: WasmEdge 3269 / 3232 ns, wasmtime 1813 / 1789 ns
- table write: WasmEdge 1209 ns, wasmtime 17.3 ns

Run 1, rented x86-64 machine (Xeon Platinum 8488C, 96 cores), load average 1.0 to 3.0 (10 at the start, from the
build), 2026-11-23 09:23 to 09:32 UTC, same job shape:
- compile: WasmEdge 5.99 ms, wasmtime 244 µs
- instantiate: WasmEdge 5.18 µs, wasmtime 386 ns
- empty call: WasmEdge 863 ns, wasmtime 12.7 ns
- host call: WasmEdge 307 ns per call, wasmtime 3.99 ns
- unshared write / read 64 B: WasmEdge 64.2 / 55.4 ns, wasmtime 4.3 / 4.2 ns
- unshared write / read 64 KiB: WasmEdge 1337 / 1337 ns, wasmtime 1273 / 1291 ns
- shared write / read 64 B: WasmEdge 1150 / 1133 ns, wasmtime 36.3 / 37.9 ns
- shared write / read 64 KiB: WasmEdge 3770 / 3831 ns, wasmtime 2584 / 2561 ns
- table write: WasmEdge 966 ns, wasmtime 12.6 ns

The checker's `instantiates_many_times` (instantiate with one import, then find a table, 4000 times), ARM, load 19
to 24, 2026-11-23 10:11 UTC: WasmEdge 26.9 µs for the first 500 and 17.2 µs for the last 500; wasmtime 941 / 809 ns.
The run-1 tree gave 37.8 / 15.1 µs in the checker's job.

Direct comparison (scratch direct-bench: WasmEdge's C interface called directly, unsafe, against the interface, same
binary), ARM machine, load 18 to 20, 2026-11-23 09:14 to 09:20 UTC, best of 7 rounds (run 1; those paths are
unchanged):
- empty call: raw `WasmEdge_ExecutorInvoke` with arrays on the stack 1085 ns, the bindings' `call_func_ref` 1103 ns,
  the interface 1105 ns. The engine's own entry is what costs; the bindings and the interface add about 20 ns.
- 1000 host calls:
  - a raw host function on an executor without hooks: 76.4 µs (76 ns per call);
  - the same raw host function on an executor with the two signal-mask hooks the bindings register: 391 µs;
  - the interface: 398 µs.
  The hooks (two `pthread_sigmask` system calls per host call, for the bindings' call timeouts) cost about 315 ns
  per call; the bindings' value lists and the interface add about 7 ns.
- unshared memory 64 B: raw `WasmEdge_MemoryInstanceGetData` / `SetData` on a held pointer 3.6 / 4.7 ns, the
  interface 62 / 71 ns (the name lookup, below); 64 KiB: raw 756 / 760 ns, the interface 842 / 857 ns.
- shared memory 64 B: raw 3.6 / 4.7 ns, the interface (copier) 1437 / 1441 ns; 64 KiB: raw 755 / 759 ns, the
  interface 3264 / 3293 ns.
- table write: raw `WasmEdge_TableInstanceSetData` 4.2 ns, the interface (table writer) 1219 ns.

## For the owner

1. Condition (1) cannot hold on WasmEdge through its bindings' safe layer, and mostly not even through its C
   interface. Measured above (ARM): an empty call costs 1085 ns in the engine itself (raw C call) against wasmtime's
   18 ns. A host call costs 76 ns raw and 398 ns through the bindings' executor. Every call and every host call
   through the safe layer allocates:
   - `call_func_ref` collects the parameters into a `Vec`, reads the function type (two more `Vec`s), and returns a
     `Vec`;
   - a host function receives a `Vec` and must return one (`SyncFn`).
   The bindings have no typed call. What is ours in those paths: an array of the parameters; converting each
   value; the boxed host context moved into an `Rc<Cell<Option<Box<_>>>>` and back, once around each call into a
   module and once around each host call (two pointer-sized moves each time, no lock or borrow check, whatever
   the context's size; since run 5); one comparison of engine identities per call
   and per memory access (run 4); and, on the shared-memory read path only, the `RefCell` borrow of point 4.
   Each memory access also looks its export's name up, which the bindings force (point 3). Executor::create
   registers the two signal-mask hooks of point 2.
2. On Linux, the bindings' `Executor::create` registers a pre and a post host-function hook. Each makes a
   `pthread_sigmask` system call, for the bindings' call-timeout feature. Together they cost about 315 ns of the
   398 ns per host call, and without unsafe they cannot be avoided. The unsafe lines that would avoid them:
   - `let executor = unsafe { ffi::WasmEdge_ExecutorCreate(config.as_ptr(), core::ptr::null_mut()) };` (the `ffi`
     feature);
   - invoking with `unsafe { ffi::WasmEdge_ExecutorInvoke(executor, function.get_func_raw(), parameters.as_ptr(), n,
     results.as_mut_ptr(), m) }` and arrays on the stack;
   - host functions made with `ffi::WasmEdge_FunctionInstanceCreate` and an `extern "C"` trampoline that reads its
     closure and the host context from the data pointer.
   Measured gain: 398 to 76 ns per host call; calls into a module gain about 20 ns of 1100.
3. The safe layer's handles to memories, tables and globals borrow the instance that owns them. So an address is the
   owner and the export name, and every access looks the name up (a C string is made, then a map is searched):
   about 58 ns of the 62 ns of a 64-byte read. The unsafe lines that would remove the lookup: hold
   `unsafe { memory.as_ptr() }` in the address and call `unsafe { ffi::WasmEdge_MemoryInstanceGetData(pointer,
   into.as_mut_ptr(), offset, length) }` (and `SetData`). Measured: 3.6 ns against 62 ns for a 64-byte read.
4. Shared owners. The bindings hand out a memory mutably only through `&mut` of its owner, and a shared memory's
   owner is shared, so writes go through a copier, as for wasmtime. Reads could use `Memory::slice`, but a `&[u8]`
   over a memory other threads write is a data race, so they go through the copier too. The bindings also offer
   `Memory::mut_slice(&self) -> &mut [T]`, a safe function that hands out `&mut` from `&`; I do not use it.
   Because the interface gives `read_memory` `&self` while the copier needs the executor mutably, the copier read
   borrows the executor from a `RefCell`. That is a run-time borrow check of ours on the shared-memory read path,
   negligible next to the 1.4 µs copier call, and `read_memory(&mut self)` would remove it.
   An instance that exports a shared memory is held shared the same way. Its unshared memories' writes and growth
   go through their own copier, and its globals' writes through a setter (figures in "Measurements", run 2). The
   unsafe route for all of them is the `GetData` / `SetData` lines of point 3 and `ffi::WasmEdge_GlobalInstanceSetValue`
   on a held pointer: 3.6 against 1437 ns for 64 bytes.
5. Tables: the safe layer has no mutable access to an instance's table (`AsInstance` has `get_table` only). So every
   table write and growth is a call into a table-writer module: 1219 ns against 4.2 ns for
   `unsafe { ffi::WasmEdge_TableInstanceSetData(table.as_ptr().cast_mut(), value.as_raw(), index) }`.
6. Imports are linked by name only. Each instantiation makes a store and one forwarder per imported module name, and
   reads the module's declarations, which the bindings' types cannot describe exactly (no 64-bit flag, no typed
   reference): 5.4 µs for the benchmark's one import (run 1), against wasmtime's 0.5 µs. A function no instance
   exports (a table element that points at an unexported function) is imported through a pass-on helper: a
   compiled module of ours holds it in a typed table and exports a function of the import's type that calls it
   with `call_indirect`. The cost is one instantiation of that helper per such import and one indirect call per
   call (measured under "Measurements", run 3). A function of another type fails the helper's cast:
   `InstantiateError::FunctionTypeMismatch`, where the standard gives a link error.
7. Nothing of the interface is left unreachable. Tags, which I wrongly reported unreachable until run 3, link
   by name through the forwarder like the other items and need no type of the bindings. Host-made 64-bit memories
   and tables are modules of ours, and bytes beyond 4 GiB go through the 64-bit copier.
8. Version: the only released bindings (wasmedge-sys 0.21.1) pin engine 0.17.1, whose compiler refuses exception
   handling. 0.18.1, with the same bindings unchanged, passes everything. The crate takes the library
   `WASMEDGE_DIR` points at, so nothing is pinned in the code.
   The bindings' build script panics without an installed library, and the crate is a workspace member through
   the glob `crates/*/implementation/*`, so until run 4 whole-workspace commands failed on any machine without
   WasmEdge (another track's `cargo check --workspace` on the ARM machine did). Since run 4 the bindings sit
   behind the crate's feature `wasmedge`, off by default, and without it the crate is empty. For the owner to
   rule: whether that is how an implementation that needs an installed engine joins the workspace. The cost:
   `cargo test --workspace` does not run this crate's suite unless the feature is asked for, and
   `--all-features` fails again where WasmEdge is absent.
9. The wasmtime track's seven points, from WasmEdge's side:
   - (1) `&self` reads: tables, globals and unshared memories read fine through `&self`. Only the copier needs the
     executor (point 4).
   - (2) the bytes of a shared memory: confirmed, the same copier (point 4).
   - (3) a fresh `Config::default()` in the suite: it does not bite here, because WasmEdge memories belong to no
     engine. Cloning in the suite would still be the clean form.
   - (4) a host function cannot trap: confirmed. WasmEdge's host functions already return a result
     (`Result<_, CoreError>`), so a trapping closure would cost nothing.
   - (5) nothing frees an instance: confirmed. There is no cap here like wasmtime's 10000, but each instantiation
     with imports also keeps its forwarders, and each helper keeps its instance.
   - (6) the suite's crate fails `clippy -D warnings`: confirmed on ARM in run 2. There are three `clippy::ok_expect`
     errors ("called `ok().expect()` on a `Result` value"), at crates/webassembly-engine/conformance/conformance.rs
     lines 259, 309 and 463.
   - (7) inference and tuple parameters: confirmed, the same shape.
10. Places where an engine-specific fast path would pay, none built:
    - held raw pointers for memories, tables and globals (points 3 to 5);
    - an executor without the signal hooks, and host functions on the C interface (point 2).
    The engine's own call entry (about 1.1 µs) is beyond any embedding.
11. Logging. WasmEdge logs at info level to standard error (each compilation) and logs every trap as an error. The
    crate does not change the process-wide log level.
12. Typed function references. WasmEdge casts a reference by the type the reference carries, not by the function's
    type. It drops that type to `funcref` both in a reference the C interface makes (`WasmEdge_ValueGenFuncRef`)
    and in any reference a call returns to the host. Its executor says why: type indices are opaque to the host.
    So a function from `find_export` or `bind_host_function`, written as it is, fails a guest's `ref.test` and
    `ref.cast` even in a plain funcref table, and cannot go into a `(ref null $t)` slot by a cast alone.
    The safe route: every function reference the host writes, into any table or global or through
    `allocate_global`, first goes through a retyper, a code-free module of ours. It imports the function from where
    it is exported, with the type its module declares (for a host function, the type of its values, which are
    numbers only), and holds one table element filled by `ref.func`; a reference read from a table or a global keeps
    its type, so such references need no retyper. Then a typed slot's writer or setter casts it.
    Cost (ARM, run 2): about 125 ns on a write once the retyper exists, and one helper instantiation per function
    the first time. A function whose module nobody describes and whose type has references would be written with
    only `funcref` and counted as `SlowPath::FunctionNotRetyped`; no function the interface can name is such a one
    today.
    A reference the host reads from a typed slot has a second problem: WasmEdge gives it the slot's type as its
    outer type, a type index of the slot's module, and then refuses it as a `funcref` parameter of any call (the
    writer's, the setter's, the pass-on helper's). The safe route (since run 5, found by the independent check):
    the holder, a compiled module of ours whose types begin with the slot module's own, takes the reference as a
    parameter of the slot's exact type and stores it in its own funcref table. Read back from there it has the
    outer type `funcref` and keeps the function's own type. WasmEdge checks the parameter's type itself. Cost
    (ARM, run 5): a read from a typed table or global goes from about 105 ns to about 1400 ns, one helper call
    per read; reads from plain `funcref` slots need no holder. The unsafe-equivalent it replaces, which the
    crate no longer has, was one line: `WasmValue::from(ffi::WasmEdge_Value { Value: reference.as_raw().Value,
    Type: <a funcref's type> })`. It gains about 1.3 µs per read from a typed slot. It needs no `unsafe` keyword,
    because the bindings' `From<ffi::WasmEdge_Value> for WasmValue` is a safe function that accepts any bits, so a
    forged reference could be made with it. No vetted library covers this: the bindings are the only Rust layer
    over WasmEdge. Tested: `every_function_the_host_writes_carries_its_own_type` and
    `a_reference_read_from_a_typed_slot_carries_the_functions_type_and_no_other`.
    The nulls WasmEdge gives the host carry `funcref` too, so a helper writes the slot's own `ref.null` for a null.
    The fix that would remove all of this belongs in WasmEdge: references the host makes or receives should keep
    the function's type, or casts should check the function instance's type.
13. Memory cap. By default WasmEdge caps every memory at 65536 pages (4 GiB). A 64-bit module memory declared larger
    then instantiates with fewer pages than it declares, and only logs an error; I saw this in run 2 before raising
    the cap. The bindings' `set_max_memory_pages` takes a `u32`, so the most it can set is 2^32 - 1 pages. WasmEdge
    then logs a false error, "Limited 4294967295 page larger than the 32-bit page upper bound 65536", for every
    32-bit memory it makes; the memory is made correctly. Its own 64-bit default (2^48 pages, which logs nothing)
    is out of the bindings' reach. Correct by the standard came first: the cap is raised and the noise remains.

## Departures from the plan

- Tests and clippy ran on the rented machines only (the notice of 2026-11-22), not on the Mac.
- Engine 0.18.1 is used with bindings made for 0.17.1: the only released bindings pin 0.17.1, whose compiler lacks
  exception handling. Nothing is patched; the bindings' build script runs bindgen over 0.18.1's own headers.
- Slow paths report through an inherent method, `WasmEdgeEngine::slow_path_count`, and the `log` crate: the
  interface has no channel for them. The wasmtime crate's copier has neither yet.
- The engine is behind a feature of the crate (`wasmedge`, off by default), so that the workspace builds where no
  WasmEdge is installed; the plan had none ("For the owner" 8).
- An address of another engine value makes the methods without an error for it panic, and `instantiate` answers
  `InstantiateError::ForeignAddress`, a variant of this crate: the interface does not say what such an address does.

## Dependencies

Each entry gives the licence, then the number of occurrences of the word `unsafe` in the package's `.rs` files
(tests, benches and examples left out; comments count too). Taken with `cargo tree -p
wcontainers-webassembly-engine-wasmedge` on ARM, 2026-11-23 (scratch deps.sh, full list in deps-run2-clean.txt).
The count is whole words: `find <crate> -name '*.rs' -not -path '*/tests/*' -not -path '*/examples/*' -not -path
'*/benches/*' | xargs grep -ow unsafe | wc -l` over the crate as published, on the rented machine's registry copy
(run 5 for wasmedge-sys, script run5/job.sh; run 3 recounted -sdk, -types, log and rustix on the Mac's copies,
which hold only src/, as the published crates have no other sources).
Run 3 added log 0.4.35 and rustix 1.1.6 (unix only, features fs and std) as direct dependencies; both were already
shipped through wasmedge-sys and tempfile.
Run 2 added wasmparser 0.263.0, tempfile 3.27.0, and the `wasmparser` feature of wasm-encoder; all three were already
in the lock file for other crates.

Direct dependencies:
- wasmedge-sys 0.21.1 (Apache-2.0; 395 as published), the FFI to the C library. Of the 395, 388 are in src/
  (27 of them in src/async, not compiled without the `async` feature) and 7 in bindings/wasmedge.rs, a
  pre-generated copy that is not compiled. What is compiled as its `ffi` module is the file bindgen writes at
  build time from the installed headers. Against 0.18.1 it has 2536 lines and 7 occurrences of `unsafe`, all in
  function pointer types, but it declares 299 foreign functions in plain `extern "C"` blocks, each unsafe to
  call. So what is compiled holds 361 occurrences in src/ plus those 299 foreign functions. Its safe
  `From<ffi::WasmEdge_Value> for WasmValue` forges any reference from raw bits; since run 5 the crate does not
  use it ("For the owner" 12)
- wasmedge-sdk 0.17.1 (Apache-2.0; 2), wasmedge-types 0.6.1 (Apache-2.0; 2), wasmedge-macro 0.6.1 (Apache-2.0; 3)
- wasm-encoder 0.263.0 (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT; 0)
- wasmparser 0.263.0 (same licence; 0)
- tempfile 3.27.0 (MIT OR Apache-2.0; 4)
- thiserror 2.0.21 (MIT OR Apache-2.0; 0)
- log 0.4.35 (MIT OR Apache-2.0; 8), for the warnings
- rustix 1.1.6 (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT; 1673), for the folder's mode bits
- dev only: wat 1.263.0 (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT; 0)
- the WasmEdge library itself, 0.18.1 (Apache-2.0): C++ with LLVM linked in, not a Cargo dependency

Shipped, through those (normal dependencies):
- anyhow 1.0.104 (MIT OR Apache-2.0; 106)
- bitflags 2.13.2 (MIT OR Apache-2.0; 3)
- bumpalo 3.20.4 (MIT OR Apache-2.0; 239)
- cfg-if 1.0.5 (MIT OR Apache-2.0; 0)
- fastrand 2.5.0 (Apache-2.0 OR MIT; 0)
- futures-core 0.3.34 (MIT OR Apache-2.0; 8)
- getrandom 0.2.17 (MIT OR Apache-2.0; 58), getrandom 0.4.3 (MIT OR Apache-2.0; 114)
- lazy_static 1.5.1 (MIT OR Apache-2.0; 2)
- leb128fmt 0.1.0 (MIT OR Apache-2.0; 0)
- libc 0.2.191 (MIT OR Apache-2.0; 810)
- linux-raw-sys 0.12.1 (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT; 6932)
- lock_api 0.4.14 (MIT OR Apache-2.0; 302)
- log 0.4.35 (MIT OR Apache-2.0; 8)
- memchr 2.8.3 (Unlicense OR MIT; 340)
- num-derive 0.3.3 (MIT OR Apache-2.0; 0)
- num-traits 0.2.19 (MIT OR Apache-2.0; 1)
- once_cell 1.21.4 (MIT OR Apache-2.0; 51)
- parking_lot 0.12.5 (MIT OR Apache-2.0; 88), parking_lot_core 0.9.12 (MIT OR Apache-2.0; 149)
- paste 1.0.15 (MIT OR Apache-2.0; 0)
- ppv-lite86 0.2.21 (MIT OR Apache-2.0; 168)
- proc-macro2 1.0.107 (MIT OR Apache-2.0; 6)
- quote 1.0.47 (MIT OR Apache-2.0; 0)
- rand 0.8.8 (MIT OR Apache-2.0; 14), rand_chacha 0.3.1 (MIT OR Apache-2.0; 1), rand_core 0.6.4 (MIT OR Apache-2.0; 2)
- rustix 1.1.6 (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT; 1673)
- scoped-tls 1.0.1 (MIT/Apache-2.0; 2)
- scopeguard 1.2.0 (MIT OR Apache-2.0; 4)
- semver 1.0.28 (MIT OR Apache-2.0; 48)
- setjmp 0.1.0 (PostgreSQL OR MIT; 0), a Linux-only dependency of wasmedge-sys
- smallvec 1.16.5 (MIT OR Apache-2.0; 83)
- syn 1.0.109 (MIT OR Apache-2.0; 62), syn 2.0.119 (MIT OR Apache-2.0; 78), syn 3.0.10 (MIT OR Apache-2.0; 97)
- thiserror 1.0.69 (MIT OR Apache-2.0; 0)
- thiserror-impl 1.0.69 (MIT OR Apache-2.0; 1), thiserror-impl 2.0.21 (MIT OR Apache-2.0; 1)
- unicode-ident 1.0.27 ((MIT OR Apache-2.0) AND Unicode-3.0; 2)
- unicode-width 0.2.3 (MIT OR Apache-2.0; 0)
- wast 263.0.0 (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT; 0)
- wat 1.263.0, also a normal dependency of wasmedge-sys (same licence; 0)
- zerocopy 0.8.63 (BSD-2-Clause OR Apache-2.0 OR MIT; 554)

Build only (wasmedge-sys's build script: bindgen, cmake, and a download stack for its `standalone` feature, which
this crate leaves off but which is built anyway). Not shipped. Licences that need a look:
- webpki-roots 0.25.4: MPL-2.0, a file-level copyleft
- ring 0.17.14: Apache-2.0 AND ISC
- bindgen 0.69.5: BSD-3-Clause
- encoding_rs 0.8.44: (Apache-2.0 OR MIT) AND BSD-3-Clause
- libloading 0.8.9, rustls-webpki 0.101.7, untrusted 0.9.0: ISC
- the icu_*, litemap, potential_utf, tinystr, writeable, yoke, zerofrom, zerotrie and zerovec crates: Unicode-3.0

The rest are MIT, Apache-2.0 or both: adler2, async-trait, autocfg, base64, block-buffer, bytes, cc, cexpr,
clang-sys, cmake, cpufeatures, crc32fast, crypto-common, digest, displaydoc, either, equivalent, filetime,
find-msvc-tools, flate2, fnv, form_urlencoded, futures-channel/io/sink/task/util, generic-array, glob, h2,
hashbrown, hex, http, http-body, httparse, httpdate, hyper, hyper-rustls, idna, idna_adapter, indexmap, ipnet,
itertools, itoa, lazycell, mime, minimal-lexical, miniz_oxide, mio, multiversion_no_op, nom, percent-encoding,
phf (four crates), pin-project-lite, regex (three crates), reqwest, rustc-hash, rustls, rustls-pemfile, rustversion,
ryu, sct, serde, serde_core, serde_urlencoded, sha2, sha256, shlex (two), simd-adler32, simdutf8, siphasher, slab,
socket2 (two), stable_deref_trait, sync_wrapper, synstructure, tar, tokio, tokio-rustls, tokio-util, tower-service,
tracing, tracing-core, try-lock, typenum, url, utf8_iter, utf_types, version_check, want, xattr. Their unsafe counts
are in deps-run2-clean.txt.
