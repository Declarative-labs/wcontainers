# Status: engine-wasmedge

The WebAssembly engine interface (crates/webassembly-engine) implemented on WasmEdge, every module compiled ahead
of time by its LLVM compiler. Crate: crates/webassembly-engine/implementation/wasmedge, package
wcontainers-webassembly-engine-wasmedge.

## Where things stand (2026-11-23, run 1 of milestone E1)

- Done: every trait of the interface; the suite's 29 tests, the 6 proposal checks and the crate's own 7 checks pass on
  the rented ARM64 machine against the stock WasmEdge 0.18.1 release; clippy with warnings denied is clean; the
  crate inherits the workspace lints (unsafe forbidden), has no unsafe and no exemption. The interface and suite
  files are the same as on main (ea9d491e).
- Against the release the bindings pin, 0.17.1: the suite passes, and the exception-handling proposal check fails
  (see "Proposals").
- The same tests pass on the rented x86-64 machine (0.18.1). The whole check (scratch engine-wasmedge/check.sh) ran
  on ARM at ec93a63d's tree: clean build of the three crates, 42 tests pass, clippy clean, no unsafe or allow in
  the crate, adding `unsafe {}` fails to compile ("usage of an `unsafe` block").
- Not done: the suite on the Mac (the notice of 2026-11-22 keeps builds off it).

## How to run

The bindings find the library where `WASMEDGE_DIR` points (or WasmEdge's usual install folders); nothing is
downloaded at build time. On the ARM machine the releases are unpacked in /work/scratch-engine-wasmedge/<version>
(the stock manylinux_2_28_aarch64 archives from WasmEdge's GitHub releases; 0.17.1's sha256 is the one the bindings
pin). Under the Nix loader the library's libz and libstdc++ are not found, so
/work/scratch-engine-wasmedge/system-libraries holds links to the system's two and goes on `LD_LIBRARY_PATH`.

- Tests: scratch engine-wasmedge/run-tests.sh defines `run <version> [cargo test arguments]`:
  `arm-run <checkout> --name engine-wasmedge -- "$(cat run-tests.sh)
  run 0.18.1"`
- Whole check (clean build, tests, clippy, unsafe grep, unsafe probe): `arm-run <checkout> --name engine-wasmedge
  -- "$(cat check.sh)"` (scratch engine-wasmedge/check.sh; `WASMEDGE_VERSION=0.17.1` for the older release).
- Benchmark: `cargo bench -p wcontainers-webassembly-engine-wasmedge` (benchmark.rs is the wasmtime crate's, only
  `main` names another type); scratch engine-wasmedge/bench.sh runs it and the wasmtime crate's in one job.
- Direct comparison (outside the repository, has unsafe): scratch engine-wasmedge/direct-bench, the C interface used
  directly against the same operations through the interface.

## Design

- Bindings: wasmedge-sdk 0.17.1 over wasmedge-sys 0.21.1, their safe layer only. wasmedge-sys runs bindgen over the
  installed headers, so the same bindings build against 0.18.1.
- `Config`: two WasmEdge configurations (one for compiled modules, run mode AOT; one for the modules that only pass
  on imports, which hold no code) and the two helper modules, compiled once on first use. `Default` sets every
  proposal the translated modules use.
- `compile` writes a native shared library (output format Native) to the system's temporary folder, loads it and
  removes the file. WasmEdge loads a native library as native code or not at all: the loader's silent fall-back to
  the interpreter only applies to universal wasm files, so no module this crate compiles is ever interpreted.
- WasmEdge links imports only by module and item name, through a store. `instantiate` makes a store of its own,
  registers in it every instance that owns an import, and under each module name the module imports from, a
  generated module that imports each item from its owner and exports it under the name asked for. A function
  known only by a reference that no instance exports cannot be linked (`InstantiateError::FunctionNotExported`).
- Addresses: a function is WasmEdge's reference value (Copy, no lifetime) plus where it is exported; memories, tables
  and globals are their owner (an index into the engine value's instances) and export name, looked up on each
  access, because the bindings' handles to them borrow their instance. Instances live as long as the engine value;
  when it is dropped, function addresses still held keep them alive.
- Calls: `FunctionTypeBinder<(A1..An), R>` for n from 0 to 17 and results that are one bare value or a tuple of 0 to
  17, value types i32, i64, f32, f64, i128 (v128): one macro, bounded by the bindings' own `WasmValType` and
  `WasmVal` and `From<Received>` (the conversion back, which the bindings lack). A call goes through
  `Executor::call_func_ref` with the parameters in an array.
- Host functions: one module per host function, its data the closure and a shared slot where the host context is
  lent during a call (moved in before the call, back after). The bindings give a host function only a `fn` pointer
  and its module's data, so the host context cannot sit in the engine value and also be reached from the function.
- Shared memories: the bindings give shared ownership only, and write a memory only through `&mut`. As for wasmtime,
  each engine value reaches a shared memory's bytes through a copier instance (load, store, grow through an unshared
  scratch memory). An instance that exports a shared memory is held shared too; writing its unshared memories or
  its globals then panics (the bindings cannot write through shared ownership).
- Tables: the bindings have no mutable access to an instance's table, so writes and growth go through a table
  writer instance (`set`, `grow`); reads use the table directly.
- Traps are `Box<WasmEdgeError>`; an uncaught exception is WasmEdge's error code UncaughtException, answered
  `InvokeError::Exception`.

## Proposals (each its own module in proposals.rs, compiled ahead of time through the interface)

On the ARM machine, released libraries, 2026-11-23:
- 0.18.1: tail calls (return_call ten million deep), typed function references (call_ref, return_call_ref,
  ref.as_non_null), exception handling (try_table, throw, catch_ref, throw_ref, an uncaught exception), threads
  (atomic add, wait, notify, fence on a shared memory) and shared memory across two threads, 64-bit memories (loads,
  stores, size, grow, an out-of-bounds trap), multiple memories (memory.copy between two): all pass.
- 0.17.1 (the release the bindings pin): all pass except exception handling, which does not compile ahead of time
  (`Core(UnknownError(14))` from the compiler), as the survey's first reader said.
- Not checked: the legacy exception opcodes (try, catch, rethrow); WasmEdge reserves them and the survey says it
  rejects them.

## Measurements

Crate benchmark, through the interface, release builds, the WasmEdge crate and the wasmtime crate run in turn in one
job, two rounds each; each figure is the lower of the two rounds (each round is the best of 5).

Rented ARM64 machine (Graviton4, 96 cores), load average 17 to 22 (other tracks' jobs), 2026-11-23 09:02 to 09:13 UTC,
WasmEdge 0.18.1, then wasmtime 50.0.1 (same binary set, same job):
- compile the benchmark module: WasmEdge 6.37 ms, wasmtime 207 µs
- instantiate it (one import): WasmEdge 5.42 µs, wasmtime 489 ns
- an empty call into the module: WasmEdge 1047 ns, wasmtime 18.2 ns
- an empty host function called from a module loop: WasmEdge 398 ns per call, wasmtime 4.39 ns
- unshared memory, write / read 64 B: WasmEdge 69.4 / 61.5 ns, wasmtime 7.3 / 6.8 ns
- unshared memory, write / read 64 KiB: WasmEdge 821 / 817 ns, wasmtime 767 / 771 ns
- shared memory, write / read 64 B: WasmEdge 1409 / 1402 ns, wasmtime 42.1 / 55.7 ns (both through a copier)
- shared memory, write / read 64 KiB: WasmEdge 3269 / 3232 ns, wasmtime 1813 / 1789 ns
- table write: WasmEdge 1209 ns, wasmtime 17.3 ns

Rented x86-64 machine (Xeon Platinum 8488C, 96 cores), load average 1.0 to 3.0 (10 at the start, from the build),
2026-11-23 09:23 to 09:32 UTC, same job shape:
- compile: WasmEdge 5.99 ms, wasmtime 244 µs
- instantiate: WasmEdge 5.18 µs, wasmtime 386 ns
- empty call: WasmEdge 863 ns, wasmtime 12.7 ns
- host call: WasmEdge 307 ns per call, wasmtime 3.99 ns
- unshared write / read 64 B: WasmEdge 64.2 / 55.4 ns, wasmtime 4.3 / 4.2 ns
- unshared write / read 64 KiB: WasmEdge 1337 / 1337 ns, wasmtime 1273 / 1291 ns
- shared write / read 64 B: WasmEdge 1150 / 1133 ns, wasmtime 36.3 / 37.9 ns
- shared write / read 64 KiB: WasmEdge 3770 / 3831 ns, wasmtime 2584 / 2561 ns
- table write: WasmEdge 966 ns, wasmtime 12.6 ns

Direct comparison (scratch direct-bench: WasmEdge's C interface called directly, unsafe, against the interface, same
binary), ARM machine, load 18 to 20, 2026-11-23 09:14 to 09:20 UTC, best of 7 rounds:
- empty call: raw `WasmEdge_ExecutorInvoke` with arrays on the stack 1085 ns, the bindings' `call_func_ref` 1103 ns,
  the interface 1105 ns. The engine's own entry is what costs; the bindings and the interface add about 20 ns.
- 1000 host calls: a raw host function on an executor without hooks 76.4 µs (76 ns per call); the same raw host
  function on an executor with the two signal-mask hooks the bindings register 391 µs; the interface 398 µs. The
  hooks (two `pthread_sigmask` system calls per host call, for the bindings' call timeouts) cost about 315 ns per
  call; the bindings' value lists and the interface add about 7 ns.
- unshared memory 64 B: raw `WasmEdge_MemoryInstanceGetData` / `SetData` on a held pointer 3.6 / 4.7 ns, the
  interface 62 / 71 ns (the name lookup, below); 64 KiB: raw 756 / 760 ns, the interface 842 / 857 ns.
- shared memory 64 B: raw 3.6 / 4.7 ns, the interface (copier) 1437 / 1441 ns; 64 KiB raw 755 / 759 ns, the
  interface 3264 / 3293 ns.
- table write: raw `WasmEdge_TableInstanceSetData` 4.2 ns, the interface (table writer) 1219 ns.

## For the owner

1. Condition (1) cannot hold on WasmEdge through its bindings' safe layer, and mostly not even through its C
   interface. Measured above (ARM): an empty call costs 1085 ns in the engine itself (raw C call) against wasmtime's
   18 ns; a host call costs 76 ns raw, 398 ns through the bindings' executor. Every call and every host call through
   the safe layer allocates: `call_func_ref` collects the parameters into a `Vec`, reads the function type (two more
   `Vec`s), and returns a `Vec`; a host function receives a `Vec` and must return one (`SyncFn`). The bindings have
   no typed call. Nothing of ours is in those paths beyond an array of the parameters, a move of the host context in
   and out of a `Cell`, and converting each value.
2. The bindings' `Executor::create` registers, on Linux, a pre and post host-function hook that each make a
   `pthread_sigmask` system call (for their call-timeout feature): about 315 ns of the 398 ns per host call. Without
   unsafe they cannot be avoided. The unsafe lines that would: `let executor = unsafe {
   ffi::WasmEdge_ExecutorCreate(config.as_ptr(), core::ptr::null_mut()) };` (the `ffi` feature), and invoking with
   `unsafe { ffi::WasmEdge_ExecutorInvoke(executor, function.get_func_raw(), parameters.as_ptr(), n,
   results.as_mut_ptr(), m) }` with stack arrays; host functions made with `ffi::WasmEdge_FunctionInstanceCreate`
   and an `extern "C"` trampoline that reads its closure and the host context from the data pointer. Measured gain:
   398 to 76 ns per host call; calls into a module gain about 20 ns of 1100.
3. The safe layer's handles to memories, tables and globals borrow the instance that owns them, so an address is the
   owner and the export name, and every access looks the name up (a C string is made, then a map is searched):
   about 58 ns of the 62 ns of a 64-byte read. The unsafe line that would remove it: hold `unsafe {
   memory.as_ptr() }` in the address and call `unsafe { ffi::WasmEdge_MemoryInstanceGetData(pointer,
   into.as_mut_ptr(), offset, length) }` (and `SetData`); measured 3.6 against 62 ns for a 64-byte read.
4. Shared memories: the bindings hand a memory out mutably only through `&mut` of its owner, and a shared memory's
   owner is shared, so writes go through a copier, as for wasmtime. Reads could use `Memory::slice`, but a `&[u8]`
   over a memory other threads write is a data race, so they go through the copier too. The bindings also offer
   `Memory::mut_slice(&self) -> &mut [T]`, a safe function that hands out `&mut` from `&`; I do not use it. Unsafe
   route: the `GetData` / `SetData` lines of point 3, 3.6 against 1437 ns for 64 bytes. An instance that exports a
   shared memory is shared the same way, and writing its unshared memories or globals panics.
5. Tables: the safe layer has no mutable access to an instance's table (`AsInstance` has `get_table` only), so every
   table write and growth is a call into a table-writer module: 1219 ns against 4.2 ns for
   `unsafe { ffi::WasmEdge_TableInstanceSetData(table.as_ptr().cast_mut(), value.as_raw(), index) }`.
6. Imports are linked by name only. Each instantiation makes a store and one generated module per imported module
   name that re-exports each item from its owner (5.4 µs for the benchmark's one import, against wasmtime's 0.5 µs).
   A function known only by a reference that no instance exports (an element of a module's table that points at an
   unexported function) cannot be imported at all: `InstantiateError::FunctionNotExported`.
7. Not reachable through the bindings at all, answered as absent: tags (no type in the bindings, so `TagAddress` is
   uninhabited, `find_export` never answers a tag, a module that imports one cannot be instantiated); 64-bit host
   memories and tables (`allocate_*` answer `AllocateError::AddressTypeOutsideBindings`); bytes beyond the first
   4 GiB of a 64-bit memory (the bindings take 32-bit offsets): `read_memory` and `write_memory` panic there,
   because the interface's errors say "outside the memory", which would be false. One concrete fix for the last: an
   `Unsupported` variant of `ReadMemoryError` and `WriteMemoryError`, or a binding with 64-bit offsets.
8. Version: the only released bindings (wasmedge-sys 0.21.1) pin engine 0.17.1, whose compiler refuses exception
   handling; 0.18.1, with the same bindings unchanged, passes everything. The crate takes the library
   `WASMEDGE_DIR` points at, so nothing is pinned in the code. The bindings' build script fails without an installed
   library, so a workspace-wide build on a machine without WasmEdge fails at wasmedge-sys (builds with `-p` of other
   packages are unaffected).
9. The wasmtime track's seven points, from WasmEdge's side: (1) `&self` reads: WasmEdge's tables, globals and
   unshared memories read fine through `&self`; only the shared-memory copier needs the executor, which sits in a
   `RefCell` borrowed only there; `read_memory(&mut self)` would remove it. (2) shared memory bytes: confirmed, same
   copier, see point 4. (3) fresh `Config::default()` in the suite: does not bite here, WasmEdge memories belong to
   no engine, so no process-wide config was needed; cloning in the suite would still be the clean form. (4) a host
   function cannot trap: confirmed; WasmEdge's host functions return a result already (`Result<_, CoreError>`), so a
   trapping closure would cost nothing. (5) nothing frees an instance: confirmed (no cap like wasmtime's 10000 here,
   but each instantiation with imports also keeps its generated linking modules). (6) the suite's crate fails
   `clippy -D warnings`: not checked again here (`--no-deps`). (7) inference and tuple parameters: confirmed, same
   shape.
10. Places where an engine-specific fast path would pay, none built: held raw pointers for memories and tables
    (points 3 to 5), an executor without the signal hooks and host functions on the C interface (point 2). The
    engine's own call entry (about 1.1 µs) is beyond any embedding.
11. WasmEdge logs at info level to standard error (each compilation) and logs every trap as an error; the crate does
    not change the process-wide log level.

## Departures from the plan

- Tests and clippy ran on the rented ARM machine only (the notice of 2026-11-22), not on the Mac.
- Engine 0.18.1 is used with bindings made for 0.17.1: the only released bindings pin 0.17.1, whose compiler lacks
  exception handling. Nothing is patched; the bindings' build script runs bindgen over 0.18.1's own headers.

## Dependencies (licence; occurrences of the word `unsafe` in its src/)

- wasmedge-sys 0.21.1 (Apache-2.0; 389), FFI to the C library; build dependencies bindgen, cmake, and reqwest with
  rustls (used only by its `standalone` feature, which this crate leaves off, but built anyway)
- wasmedge-sdk 0.17.1 (Apache-2.0; 2), wasmedge-types 0.6.1 (Apache-2.0; 2), wasmedge-macro 0.6.1 (Apache-2.0; 3)
- the WasmEdge library itself, 0.18.1 (Apache-2.0), C++ with LLVM linked in; not a Cargo dependency
- wasm-encoder 0.263.0, already in the workspace, writes the helper and linking modules (Apache-2.0 WITH
  LLVM-exception OR Apache-2.0 OR MIT; 0)
- thiserror 2.0.21, already in the workspace (MIT OR Apache-2.0; 0)
- wat 1.263.0, tests only, already in the lock file through wasmedge-sys (Apache-2.0 WITH LLVM-exception)
