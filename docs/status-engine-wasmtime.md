# Status: engine-wasmtime

The WebAssembly engine interface (crates/webassembly-engine) implemented on wasmtime.
Crate: crates/webassembly-engine/implementation/wasmtime, package wcontainers-webassembly-engine-wasmtime.

## Where things stand (2026-11-23, run 3 of milestone E1)

- Done: the crate implements every trait of the interface; the suite's 29 tests and the crate's own 24
  pass on the rented ARM64 machine and on the rented x86-64 machine (run 2, after cleaning the three
  crates, and on x86 from an empty build folder); clippy with warnings denied is clean on the crate on
  both; the crate inherits the workspace lints (unsafe forbidden) and has no unsafe and no exemption;
  adding `unsafe {}` fails to compile on both ("usage of an `unsafe` block"). Run 3 repeated the whole
  check on ARM at cbcd0f8d (after the merge of a00ab267), same results, load 3.1 to 3.2. The interface
  and suite files are the same as on main (ea9d491e); the changes of a00ab267 touch none of them, and
  the crate's comments hold no links. The whole check is one script:
  pyro-parked/wcontainers-scratch/engine-wasmtime/check.sh, run as `arm-run <checkout> --name
  engine-wasmtime -- "$(cat check.sh)"` (or x86-run).
- Run 4 found no change to the interface, the suite or the crate since 89d4324f, so it built and ran
  nothing; the checks the suite lacks (two threads on one shared memory, a trap after a host call, an
  uncaught exception, every arity) are in beyond_the_suite.rs and pass with the crate's own tests.
- Not done: the suite on this Mac. The notice of 2026-11-22 moves every build and test off the Mac, so it
  was not run there.
- Waiting on the owner: the two points under "For the owner" that keep condition (1) from holding fully
  (a borrow check on `read_memory`, and the safe slow route for shared memories).

## How to run

- Tests: `arm-run <checkout> --name engine-wasmtime -- 'cargo test -p wcontainers-webassembly-engine-wasmtime -j $ARM_JOBS'`
- Lint: `cargo clippy -p wcontainers-webassembly-engine-wasmtime --all-targets --no-deps -- -D warnings`
  (`--no-deps`, because the suite's own crate fails `-D warnings` on `ok().expect()`, see below).
- Benchmark: `cargo bench -p wcontainers-webassembly-engine-wasmtime`; `ENGINE_BENCHMARK_SCALE=n` repeats n
  times more. benchmark.rs is written against the interface's traits alone (`measure::<T>(config)`), so
  another engine runs it by naming its own type in `main`.
- Direct comparison (outside the repository, has unsafe): pyro-parked/wcontainers-scratch/engine-wasmtime/
  direct-bench, copied to /work/scratch-engine-wasmtime/direct-bench on the ARM machine; it needs the
  checkout copy at /work/src/engine-wasmtime. `cargo run --release` there.

## Design

- wasmtime 50.0.1, the version and features of crates/executor/implementation/wasmtime, with tail calls,
  typed function references, GC, exceptions, threads and shared memory, 64-bit memories and multiple
  memories on.
- `Config`: a wasmtime `Engine` plus the copier modules (below), compiled once on first use. `Default`
  returns one process-wide engine (see "For the owner", point 3).
- `WasmtimeEngine<HostContext>`: one `Store<HostContext>`; host functions get `&mut HostContext` from
  `Caller::data_mut`. Addresses are wasmtime's handles (`Func`, `Table`, `Global`, `Tag`, `Module`,
  `Instance`, `SharedMemory`); `MemoryAddress` is ours, over a `Memory` or a shared memory, one match per access.
- `FunctionTypeBinder<(A1..An), R>` for n from 0 to 17 (wasmtime's typed-call limit), one macro,
  `Ai: WasmTy`, `R: WasmResults + WasmRet`: `Func::typed`, `TypedFunc::call`, `Func::wrap` of a closure
  that gathers the arguments into the tuple.
- A trap is `wasmtime::Error`. An exception leaving the call is wasmtime's `ThrownException`: taken out
  of the store inside a `RootScope` and answered `InvokeError::Exception`.
- Shared memory: wasmtime gives no safe access to its bytes (`SharedMemory::data` returns
  `&[UnsafeCell<u8>]`; its documentation: "Casting to those types must currently be done unsafely").
  Safe route: per shared memory per engine value, a copier instance that imports it, defines an
  unshared scratch memory and exports `load`/`store`, which run `memory.copy` between the two; the host
  reads and writes the scratch memory with `Memory::read`/`write`. Copiers are found again by the shared
  memory's base address, so finding the same export twice makes one copier.

## Measurements

All on the rented ARM64 machine (Graviton4, 96 cores), load average 4.7 to 6.2, 2026-11-23 06:36 to 06:45
UTC, release builds, best of 5 (crate benchmark) or 7 (direct comparison) rounds.

Crate benchmark (through the interface):
- compile the benchmark module (two functions, one import): 201 µs
- instantiate it: 496 ns
- an empty call into the module: 18.1 ns
- an empty host function called from a module loop: 4.32 ns per call (4318 ns per 1000)
- unshared memory: write 64 B 7.4 ns, read 64 B 6.9 ns, write 64 KiB 772 ns, read 64 KiB 769 ns
- shared memory (copier): write 64 B 42.0 ns, read 64 B 55.1 ns, write 64 KiB 1819 ns, read 64 KiB 1807 ns
- table write: 17.4 ns

Direct wasmtime against the interface, same binary, same run (run 2 of 2):
- empty call: direct 19.09 ns, interface 19.01 ns
- 1000 host calls: direct 4636 ns, interface 4358 ns
- read 64 B unshared: direct 4.62 ns, interface 7.02 ns (the RefCell borrow, point 1)
- write 64 B unshared: direct 6.29 ns, interface 7.44 ns
- read 64 KiB unshared: direct 2546 ns, interface 2548 ns; write 64 KiB: direct 768 ns, interface 778 ns
  (this binary reads before it writes, which made both reads slower than in the crate benchmark)
- shared 64 B: unsafe copy read 2.39 ns, write 2.57 ns; copier read 56.3 ns, write 43.0 ns
- shared 64 KiB: unsafe copy read 2550 ns, write 752 ns; copier read 3392 ns, write 1821 ns
- table write: direct 17.16 ns, interface 17.72 ns (bounds are checked only when the write fails)

Crate benchmark on the rented x86-64 machine (Xeon Platinum 8488C, 96 cores), load average 2.0 to 3.8,
2026-11-23 06:50 to 06:52 UTC, best of 5 rounds (no direct comparison taken there):
- compile 248 µs; instantiate 418 ns; empty call 12.3 ns; empty host call 4.00 ns per call
- unshared memory: write 64 B 4.3 ns, read 64 B 4.3 ns, write 64 KiB 1271 ns, read 64 KiB 1283 ns
- shared memory (copier): write 64 B 37.1 ns, read 64 B 38.9 ns, write 64 KiB 2596 ns, read 64 KiB 2690 ns
- table write: 12.6 ns

## For the owner

1. `&self` on reads cannot be served by wasmtime without interior mutability. `Table::get` and
   `Global::get` take `impl AsContextMut` (lazy table initialisation and GC rooting), but
   `TableStore::read_table` and `GlobalStore::read_global` take `&self`. So the store is a
   `RefCell<Store<_>>`: every `&mut self` method uses `RefCell::get_mut` (no check), but the `&self` ones
   pay a run-time borrow check, including the hot `read_memory` (measured above: 7.02 against 4.62 ns
   for 64 bytes). Smallest fix: `read_table(&mut self, ..)` and `read_global(&mut self, ..)`; with that
   the RefCell goes, and `read_memory` costs what `Memory::read` costs. `read_memory(&mut self, ..)` would
   also let the shared route below drop it.
2. Shared memory bytes, the exact unsafe lines that would replace the copier (measured above: 2.4 against
   56 ns for a 64-byte read, 752 against 1821 ns for a 64 KiB write):
   read: `let cells = shared.data().get(offset..offset + into.len()).ok_or(ReadMemoryError)?;`
   then `unsafe { core::ptr::copy_nonoverlapping(cells.as_ptr().cast::<u8>(), into.as_mut_ptr(), into.len()) }`;
   write: the same with `copy_nonoverlapping(bytes.as_ptr(), cells.as_ptr().cast_mut().cast::<u8>(), bytes.len())`.
   No vetted library covers it: wasmtime's own documentation says the cast must be done unsafely, and the
   standard library's `AtomicU8::from_ptr` is itself unsafe. They would gain the copier's call and copy, and
   one wasmtime instance per shared memory per engine value.
3. The suite's `shares_a_memory_with_another_engine` makes the second engine from a fresh
   `T::Config::default()`, not a clone. A wasmtime `SharedMemory` belongs to one wasmtime `Engine`, so the
   only way to pass is that `Default` hands out one process-wide engine, which is what the crate does. A
   shared memory added to an engine value of a non-default config of another engine would panic, since
   `add_shared_memory` has no error to return. Either the suite clones the config, or
   `add_shared_memory` returns a `Result`.
4. A host function cannot trap: the closure returns `Results`. A kernel that must stop a guest from a
   host call (exit, a fatal signal) would need `Result<Results, Trap>` or similar from the closure; wasmtime
   supports it at no cost (`Func::wrap` takes a closure returning `Result<R>`).
5. Instances live as long as the engine value, and wasmtime's store holds at most 10000 of them (the crate
   benchmark hit this, so it now instantiates 1000 times). Nothing in the interface frees an instance.
6. The suite's crate fails `cargo clippy -D warnings` (`clippy::ok_expect` at conformance.rs lines 259,
   309, 463: `.ok().expect(..)` on a `Result<_, BindFunctionError>`).
7. A concrete engine type has one `FunctionTypeBinder` per signature, so `engine.bind_engine_function(f)`
   does not infer; callers write `<E as FunctionTypeBinder<P, R>>::bind_engine_function(&mut e, f)`.
   Parameters are tuples only (`(i32,)`, not `i32`): an impl over a bare `WasmTy` overlaps the tuple
   impls under coherence. Results can be a bare type or a tuple.

Places where an engine-specific fast path would pay, none built: the shared-memory copy (point 2, about
54 ns per small access); a borrowed view of an unshared memory (`Memory::data`) instead of a copy into the
caller's buffer, which the interface does not offer.

## Departures from the plan

- `Config::default()` is one process-wide engine (point 3), not a new engine per call.
- Tests and clippy ran on the two rented machines (the notice of 2026-11-22), not on the Mac.

## Dependencies (licence; occurrences of the word `unsafe` in its src/)

- wasmtime 50.0.1, already in the workspace (Apache-2.0 WITH LLVM-exception; 1839)
- wasm-encoder 0.263.0, already in the workspace, writes the copier (Apache-2.0 WITH LLVM-exception OR
  Apache-2.0 OR MIT; 0)
- thiserror 2.0.21, already in the workspace (MIT OR Apache-2.0; 0, and 1 in thiserror-impl)
