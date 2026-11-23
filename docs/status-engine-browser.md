# Status: engine-browser

The WebAssembly engine interface (crates/webassembly-engine) run on a browser's own engine through the standard
WebAssembly JavaScript API, from our Rust code compiled to wasm32-unknown-unknown and running in a worker.
Crate: crates/webassembly-engine/implementation/browser, package wcontainers-webassembly-engine-browser.

## Where it stands (milestone E1, run 3, 2026-11-23)

- Done: the crate; the whole suite (29 tests) passing in headless Chrome 158 and headless Firefox 160 in a
  dedicated worker; 8 tests of this engine's own behaviour (behaviour.rs); a two-worker test where an engine of
  ours in a second worker uses a module and a shared memory posted from the first; the benchmark and its figures
  for both browsers; the safe routes measured against unsafe replacements built in scratch (For the owner 4).
  No unsafe in the crate; `[lints] workspace = true`; clippy for wasm32 with warnings denied is clean.
- Run 3 fixed what the second independent check found:
  - A call into a module no longer moves the host context and the host functions in and out: the context stays
    where the host functions take it from until `host_context` takes it home, and the host functions live in one
    list per thread. What is left of ours on the call is one branch and the store and restore of one number
    (measured below at 0.5 ns or less).
  - The host-context check on a host call is measured (raw-context variant, below); the old line saying it cost
    nothing measurable was wrong.
  - A table knows its address type (`typeof table.length` is "bigint" for a 64-bit one) and whether its elements
    are `funcref` (one module that imports a `funcref` table, instantiated once when the table is described).
    Every table read and write is one JavaScript call; bounds are checked only when it fails. No throw-and-retry
    remains; memory and table growth pass the known address type.
  - An `externref` table refuses a function (ReferenceTypeMismatch) and reads as ElementNotFunction; a mutable
    `externref` global answers ValueTypeMismatch on write and ReadGlobalError on read; a global of a type the
    engine cannot tell apart answers ValueTypeMismatch.
  - A shared 64-bit memory works: its access module imports it with the JavaScript API's page limit as maximum.
  - `allocate_global` no longer searches for the type it already knows.
  - A module or shared memory dropped on another thread (possible only in a build whose own memory is shared) is
    queued and forgotten by its own thread at that thread's next registration. Not tested: a build without
    atomics cannot have a second thread.
  - js-sys is used without its default features, which took futures-util, futures-task, pin-project-lite and
    slab out of the library's dependencies. The dependency list below is complete.
- Not done: Safari (below). The owner's linter was not run (the notice of 2026-11-22 says not to for now). The
  literal check was not run: it lists files through git, and the rented machine's copy has no `.git`, while the
  Mac may not build it. Its likely finding is adapter.rs's `MEMORY64_PAGES = 262_144`, the JavaScript API's page
  limit for a 64-bit memory, which no crate defines.
- Next: the items under "For the owner" wait for his rulings.

## How to run

- Everything runs on the rented x86 machine, which has Chrome 158 installed (Chrome for Testing has no
  ChromeDriver for ARM64 Linux, so the ARM machine cannot drive its Chrome through WebDriver).
- Tools, once, under /work/engine-browser on that machine: the scratch script
  pyro-parked/wcontainers-scratch/engine-browser/setup-tools.sh installs ChromeDriver 158.0.8127.36, Firefox
  160.0.1, geckodriver 0.37.1 and wasm-bindgen-cli 0.2.130 (the version wasm-bindgen is pinned to).
- `x86-run <checkout> --name engine-browser -- 'WC_BROWSER_TOOLS=/work/engine-browser bash
  crates/webassembly-engine/implementation/browser/run-in-browsers.sh --test conformance --test behaviour
  --test workers'` runs the suite, the behaviour tests and the two-worker test in Chrome, then Firefox. For the
  figures: `... run-in-browsers.sh --release --test benchmark -- --nocapture`.
- The unsafe variants (measuring only, never committed): pyro-parked/wcontainers-scratch/engine-browser/run3 holds
  variant.py (turns a copy of the crate into variant no-run or raw-context) and bench.sh, which runs the
  benchmark of the copy as it is or made into the variant named; run 2's variants (u1, u1dyn, u2, u3, made for
  run 2's code) are in engine-browser/variants with make-variant.py and bench-variants.sh.
  Run one variant at a time in its own copy (`--name engine-browser-variants`), with
  `CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS="-C link-arg=--growable-table"` for u2 and u3: the loop in
  bench-variants.sh once ran two variants unchanged (their figures equalled the current crate's), so every
  variant figure below comes from a run of that variant alone or was checked by one.
- wasm-bindgen-test-runner serves the page cross-origin isolated (the benchmark logs `crossOriginIsolated:
  true` and the global scope `DedicatedWorkerGlobalScope`).

## Design

- `BrowserEngine<HostContext>`; `Config` is `()`; `new` cannot fail. Addresses are the JavaScript API's objects
  (`Function`, `Table`, `Instance`, a tag as a `JsValue`), a memory with its access module, a global with its
  value type. Errors are what the API throws (`JsValue`).
- Calls carry no JavaScript values: each thread has 16 scratch slots in our own memory, and the engine writes
  small modules with wasm-encoder that move a call's values between the slots and the function's arguments and
  results, importing our own memory.
  - `bind_engine_function` instantiates a caller module that imports the function with the bound type. The
    instantiation fails with a LinkError when the function has another type, so the check is the standard's own
    and works for a function from any source. `invoke` writes the arguments to the slots, makes one JavaScript
    call of the caller's export (glue.js `call`, which catches a throw), and reads the result. A thrown
    `WebAssembly.Exception` is `InvokeError::Exception`, anything else a trap.
  - `bind_host_function` boxes the function into the engine's own list and instantiates a host module whose
    export has the bound type and which knows the engine's number and the function's index. The guest calls it
    wasm to wasm; it writes the arguments to the slots and calls `run_host_function(engine, index)` straight out
    of our function table. No JavaScript runs on a host call.
  - The host context lives boxed in a `Cell<Option<Box<_>>>` the engine shares with its host functions; a host
    function takes it, runs, and puts it back (no borrow check). `host_context` takes it home into the engine;
    the next call into the engine's code puts it back (one branch). While an engine's code runs, a thread-local
    holds its number; the runner compares it with the number the host module passes (a host function of another
    engine traps), then takes the function from the thread's list (a `RefCell` borrow and an `Rc` clone, so a
    host function can bind in, and call into, another engine). An engine removes its host functions from the
    list when dropped. When a call does not return, the engine checks that its context came back; if not, a
    host function panicked (a panic is an abort on this target, it cannot unwind) and the call panics in turn.
- Memory reads and writes: an access module per memory that imports it and our own memory and runs
  `memory.copy` (multi-memory). Out of bounds traps, which is answered with the interface's error.
- Tables: the address type from `typeof table.length`, and whether the elements are `funcref` from whether one
  small module importing a `funcref` table instantiates with it, both found once when an exported table is
  described (an allocated table's type is known).
- Globals: JavaScript cannot tell a global's type, so the engine finds an exported global's type by which of 14
  small modules (7 value types, constant or variable) instantiates with the global; an allocated global's type
  is known. Floats and vectors are read, written and
  created through a module and the first slot: a JavaScript number does not keep a NaN's bits, and JavaScript
  cannot hold a vector at all.
- `Module: Send + Sync` and `SharedMemory: Clone + Send + Sync` hold only a registration id (a number from one
  atomic counter in our own memory, so unique across every thread that shares it). The JavaScript object stays
  in a per-worker registry in glue.js and leaves it when the handle is dropped. Another worker learns the object
  when it is posted: `module_to_post`/`receive_module` and `shared_memory_to_post`/`receive_shared_memory`.
  Instantiating a module this worker does not know returns an error saying so. Nothing claims thread safety a
  JavaScript object lacks.
- `compile_streaming` keeps the interface's default: the stream it is given is a Rust stream of chunks, which can
  never be handed to `WebAssembly.compileStreaming` as a Response without copying.
- Value types without a trait (the linter forbids a trait in an implementation): `FunctionTypeBinder` is one
  blanket impl per arity, 0 to 16, generated by a macro, over any `'static` types. Each type is matched to a
  value type by `TypeId` (i32, u32, i64, u64, f32, f64, u128, i128; results `()` or one of those), resolved
  when the code is compiled. A type that is not a value type is refused when binding.

## Glue: hand-written JavaScript or js-sys's generic objects (measured, figures below)

- An empty call: our route (one call into glue.js plus the caller module) 16.3 ns in Chrome, 14.3 ns in
  Firefox; js-sys's `Function::call0` on the export 34.1 and 58.8 ns.
- A 64-byte read: js-sys's typed-array route 181.6 / 201.2 ns; hand-written glue with typed-array views
  77.2 / 131.0 ns; the access module 24.6 / 20.4 ns.
- Chosen: hand-written glue (39 imported functions, each one call of the JavaScript API) and the access module.

## Measurements

Rented x86 machine (AWS c7i.metal-24xl, Xeon Platinum 8488C, 96 threads), release build, 2026-11-23, Chrome 158
headless then Firefox 160 headless, dedicated worker. Two runs of the current code (loads 0.2 to 0.9, then 2.5);
first run, with the second in brackets where it was taken:
- compile a small module (two functions, one import): 11.4 µs; 34.6 µs
- instantiate it: 5.1 µs; 2.8 µs
- bind a function (compile and instantiate a caller module): 14.8 µs; 37.0 µs
- empty call into a module: 16.3 (16.2) ns; 14.3 (15.2) ns
- empty host function called from a module in a loop: 9.6 (9.8) ns; 10.7 (10.7) ns
- the same, touching the host context: 13.7 (10.4) ns; 9.9 (10.1) ns. Both go through the same take and put of
  the context, so their difference does not measure it; the raw-context variant does (For the owner 4).
- read / write 64 B, unshared memory: 24.6 / 23.8 ns; 20.4 / 20.0 ns
- read / write 64 B, shared memory: 24.2 / 23.7 ns; 19.8 / 20.4 ns
- read / write 64 KiB, unshared: 975.5 / 1293.4 ns; 1513.6 / 1697.4 ns
- read / write 64 KiB, shared: 970.8 / 1292.5 ns; 1535.0 / 1685.6 ns
- write / read a function, 32-bit table: 82.1 (86.1) / 95.5 (109.7) ns; 46.2 (45.8) / 68.4 (67.6) ns
- write / read a function, 64-bit table: 84.2 (88.2) / 87.1 (89.4) ns; 46.0 (48.0) / 74.6 (78.4) ns
- not used, for comparison: js-sys `Function::call0`, an empty call 34.1 ns; 58.8 ns. js-sys read 64 B 181.6;
  201.2 ns, 64 KiB 1134.0; 1255.6 ns. Glue typed-array views, unshared read / write 64 B 77.2 / 71.9; 131.0 /
  152.2 ns, 64 KiB 1138.8 / 1450.0; 1181.6 / 1463.0 ns; shared 64 KiB 1539.5 / 1745.9; 1600.0 / 1776.6 ns.
- Run 2's code on the same machine: empty call 26.6 / 25.9 ns, host call 18.3 / 17.3 ns; the independent check
  measured its 64-bit table write at 2034.7 / 14456.0 ns (the throw-and-retry now gone).

Places where an engine-specific path, chosen by a probe, would pay (none built):
- Firefox, 64 KiB reads and writes of an unshared memory: typed-array views beat memory.copy by about 20%
  (1181.6 against 1513.6 ns read, 1463.0 against 1697.4 ns write). Not for a shared memory, and not in Chrome.
- Chrome, small reads and writes: calling the access module through our own table instead of JavaScript halves
  them (variant u3 below), but needs unsafe; in Firefox it gains nothing.
- Binding compiles a caller module each time, 15 to 37 µs. A cache per signature would leave only the
  instantiation. That is general work, not engine-specific.

## For the owner

1. No trait may be declared in an implementation, and the browser has no Rust trait for its value types.
   So value types are matched by `TypeId`. Two consequences: results can only be `()` or one value, because
   multi-value results cannot be told apart from a single type without a trait. And `bind_host_function`,
   which cannot return an error, panics on a type that is not a value type. Example:
   `FunctionTypeBinder<(i32,), (i32, i64)>` is implemented but `bind_engine_function` always answers
   `BindFunctionError`. Smallest change: a value-type trait in the interface (for example `pub trait
   ValueType: Copy + 'static` with the value's type and its bits, implemented there for the number types and
   tuples of them), or allowing a private trait in an implementation.
2. `add_shared_memory` cannot fail, but in a browser it can: the memory may not have been posted to this
   worker. Example: a `BrowserSharedMemory` sent through a Rust channel to a thread whose worker never received
   the object panics there. Smallest change: `-> Result<Self::MemoryAddress, Self::AddSharedMemoryError>`.
3. Posting between workers is outside the interface: four public functions (`module_to_post`,
   `receive_module`, `shared_memory_to_post`, `receive_shared_memory`). Whatever starts a worker must post the
   objects with its first message. The linter will report them as public outside the interface. A small
   coupled trait would hold them if he wants it in the contract.
4. The zero-cost condition is not fully met by the safe routes. What of ours is left on each hot path, and the
   unsafe that would replace it (none committed; each variant built and measured in a scratch copy on the same
   machine, Chrome; Firefox):
   - Call into a module, 16.3 / 14.3 ns. Ours: the branch that puts the context back if `host_context` took it
     home, and the store and restore of the running engine's number. With both removed (variant no-run, which
     breaks host functions, so only this figure counts): 15.8 / 14.1 ns, so ours is 0.5 ns or less, near the
     noise between runs. The rest is the JavaScript hop, the standard's only way to catch a trap. Replacing the
     hop (u2, run 2): the caller module's export in our own table, called with `unsafe {
     core::mem::transmute::<usize, extern "C" fn()>(index) }()`, measured 17.6 / 19.0 ns against run 2's 26.6 /
     25.9 (not measured against the new code). Not usable: WebAssembly cannot catch a trap, so a trap or an
     uncaught exception in the guest would unwind through our frames with nothing to catch it, and `invoke`
     could never answer `InvokeError`.
   - Host call, 9.6 / 10.7 ns. Ours: the compare of the running engine's number (thread-local), a `RefCell`
     borrow of the thread's list, a bounds check, an `Rc` clone and drop, a dynamic call into the closure, and
     the take and put of the host context. The context's take and put, replaced (variant raw-context) by a
     pointer to the boxed context taken when binding, `let pointer = &mut *boxed as *mut HostContext;`, and
     `function(unsafe { &mut *pointer }, arguments)` in the closure: 8.3 / 8.8 ns, so the context check costs
     1.3 / 1.9 ns. The lookup and the dynamic call, replaced by a pointer to a monomorphic closure (u1, run 2:
     `extern "C" fn run_host_function<F: Fn()>(address: u32) { let function = unsafe { &*(address as usize as
     *const F) }; function(); }`), measured 4.6 / 5.2 ns against run 2's 18.3 / 17.3 ns; not measured against
     the new code. Caveat for both: a host module's `Function` can outlive its engine in JavaScript, and then
     the pointer dangles; a real change needs the engine to cut its host modules off when dropped (for example
     a per-engine table it nulls), not built or measured.
   - Memory copy, 24.6 / 20.4 ns at 64 B. Replacement (u3, run 2): the access module checks bounds itself and
     returns 0 or 1 instead of trapping, is placed in our own function table, and is called with `let entry =
     unsafe { core::mem::transmute::<usize, extern "C" fn() -> u32>(index as usize) }; entry() != 0`.
     Measured 10.8 / 10.6 ns in Chrome and 19.0 / 18.2 ns in Firefox; 64 KiB unchanged. Needs our table to be
     growable (`-C link-arg=--growable-table`).
   No vetted library covers these: each turns an integer from outside Rust into a reference or a function.
5. The workspace's `forbid(unsafe_code)` does not see macro output. glue.rs's `#[wasm_bindgen] extern` block
   expands to wasm-bindgen's generated code for each of its 39 functions, unsafe included, the same code js-sys
   holds for its own imports.
6. The suite's crate fails `cargo clippy -D warnings` (`ok().expect()` at conformance.rs lines 259, 309, 463).
   It is his file, so I left it alone. Clippy of this crate alone uses `--no-deps`.
7. Safari is not tested: driving it needs `safaridriver --enable` (administrator) and "Allow remote automation",
   both system settings on the Mac.
8. The suite does not check that a global keeps a NaN's bits, that host functions are dropped with their engine,
   that a host function can bind in another engine, 64-bit tables of a module, `externref` tables and globals,
   or shared 64-bit memories; behaviour.rs checks them for this engine. They may
   belong in the suite.
9. A panic in a host function makes the call panic (as under wasmtime); the engine's host functions and context
   are lost with it, because a panic is an abort on wasm32-unknown-unknown. That is what the interface's silence
   leaves; a ruling on what a host function's panic should do would settle it.

## Dependencies (licences; lines with the word `unsafe` in the registry sources)

What `cargo tree -e normal --target wasm32-unknown-unknown` shows for the library:
- wasm-bindgen 0.2.130 (MIT OR Apache-2.0): 141. Its build-time crates: wasm-bindgen-macro 0.2.130: 0,
  wasm-bindgen-macro-support 0.2.130: 60 (in the code it generates), wasm-bindgen-shared 0.2.130: 0, bumpalo
  3.20.4 (MIT OR Apache-2.0): 236, syn 3.0.10: 103, quote 1.0.47: 0, proc-macro2 1.0.107: 6, unicode-ident
  1.0.27: 2 (each MIT OR Apache-2.0; unicode-ident also Unicode-3.0).
- js-sys 0.3.107 (MIT OR Apache-2.0), without default features: 37. It still brings futures-core 0.3.34 (MIT
  OR Apache-2.0): 8.
- cfg-if 1.0.5 (MIT OR Apache-2.0): 0.
- wasm-encoder 0.263.0 (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT): 0; it brings leb128fmt 0.1.0
  (MIT OR Apache-2.0): 0. The workspace already uses wasm-encoder.
- thiserror 2.0.21 (0) and thiserror-impl (1), through the interface crate.
- Test only: wasm-bindgen-test 0.3.80 and wasm-bindgen-futures 0.4.80 (MIT OR Apache-2.0), which bring
  futures-util 0.3.34 (137), futures-task 0.3.34 (65), pin-project-lite 0.2.17 (50), slab 0.4.12 (9), and the
  test runner's own crates (serde, serde_json, oorandom, nu-ansi-term and others).

## Departures from the plan

- Memory copies use an access module (memory.copy between the guest's memory and ours) instead of typed-array
  views: 3 to 6 times faster at 64 bytes in both browsers.
- Browsers run on the x86 machine, not the ARM one (no ARM64 ChromeDriver).
- A call into a module leaves the host context where the host functions take it from, and `host_context` takes it
  home, instead of lending it for each call and taking it back: the per-call lending was the cost the check
  found on the call path.
- The two-worker test loads our code in the second worker from the files wasm-bindgen-test-runner serves
  (wasm-bindgen-test.js and its wasm), the only way a test there can start our code in another worker.
