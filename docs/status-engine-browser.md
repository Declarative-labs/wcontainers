# Status: engine-browser

The WebAssembly engine interface (crates/webassembly-engine) run on a browser's own engine through the standard
WebAssembly JavaScript API, from our Rust code compiled to wasm32-unknown-unknown and running in a worker.
Crate: crates/webassembly-engine/implementation/browser, package wcontainers-webassembly-engine-browser.

## Where it stands (milestone E1, run 2, 2026-11-23)

- Done: the crate; the whole suite (29 tests) passing in headless Chrome 158 and headless Firefox 160 in a
  dedicated worker; 5 tests of this engine's own behaviour (behaviour.rs); a two-worker test where an engine of
  ours in a second worker uses a module and a shared memory posted from the first; the benchmark and its figures
  for both browsers; the safe host-call route measured against unsafe replacements built in scratch (For the
  owner 4). No unsafe in the crate; `[lints] workspace = true`; clippy for wasm32 with warnings denied is clean.
- Run 2 fixed what the independent check found: host functions now belong to their engine and are dropped
  with it; binding or calling into another engine inside a host function works; a host function's panic goes
  on from the call (as it does under wasmtime) instead of leaving the engine without its context; f32 and f64
  globals keep their NaN bits; registration ids come from one counter in our own memory; the glue's
  typed-array figures come from code in the benchmark.
- Not done: Safari (below). The owner's linter was not run (the notice of 2026-11-22 says not to for now).
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
- The unsafe variants (measuring only, never committed): pyro-parked/wcontainers-scratch/engine-browser/variants
  holds make-variant.py (turns a copy of the crate into variant u1, u1dyn, u2 or u3) and bench-variants.sh.
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
  - While its code runs, an engine lends its host context (through a `Cell<Option<Box<_>>>`, no borrow check)
    and its list of host functions (moved into a thread-local cell with its number). The runner takes the list,
    checks the number (a host function of another engine traps), calls the function, and puts the list back,
    so a host function can bind in, and call into, another engine. Taking both back after the call checks
    they came back; if not, a host function panicked (a panic is an abort on this target, it cannot unwind)
    and the call panics in turn.
- Memory reads and writes: an access module per memory that imports it and our own memory and runs
  `memory.copy` (multi-memory). Out of bounds traps, which is answered with the interface's error.
- Globals: JavaScript cannot tell a global's type, so the engine finds it by which of 12 small modules (6
  value types, constant or variable) instantiates with the global. Floats and vectors are read, written and
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

- An empty call: our route (one call into glue.js plus the caller module) 26.6 ns in Chrome, 25.9 ns in
  Firefox; js-sys's `Function::call0` on the export 35.6 and 55.3 ns.
- A 64-byte read: js-sys's typed-array route 186.1 / 188.4 ns; hand-written glue with typed-array views
  75.4 / 123.6 ns; the access module 23.5 / 21.6 ns.
- Chosen: hand-written glue (38 imported functions, each one call of the JavaScript API) and the access module.

## Measurements

Rented x86 machine (AWS c7i.metal-24xl, Xeon Platinum 8488C, 96 threads), load 0.5 to 1.2, release build, one
run each, 2026-11-23. Chrome 158 headless, then Firefox 160 headless, dedicated worker:
- compile a small module (two functions, one import): 10.8 µs; 34.7 µs
- instantiate it: 5.0 µs; 2.9 µs
- bind a function (compile and instantiate a caller module): 14.8 µs; 35.5 µs
- empty call into a module: 26.6 ns; 25.9 ns
- empty host function called from a module in a loop: 18.3 ns; 17.3 ns
- the same, touching the host context: 18.2 ns; 18.9 ns (lending the context costs nothing measurable)
- read / write 64 B, unshared memory: 23.5 / 22.0 ns; 21.6 / 21.6 ns
- read / write 64 B, shared memory: 22.8 / 22.8 ns; 20.8 / 20.4 ns
- read / write 64 KiB, unshared: 970.3 / 1289.7 ns; 1533.4 / 1741.8 ns
- read / write 64 KiB, shared: 968.7 / 1284.5 ns; 1556.6 / 1710.6 ns
- write a function into a table: 92.5 ns; 45.2 ns
- not used, for comparison: js-sys read 64 KiB 1248.9; 1296.8 ns. Glue typed-array views, unshared read /
  write 64 KiB 1141.5 / 1365.0; 1200.6 / 1431.2 ns, shared 1545.0 / 1745.3; 1581.4 / 1834.0 ns.
- Run 1's figures (empty call 16.8 / 14.1 ns, host call 6.9 / 8.3 ns) were taken on a c7i.metal-48xl. On this
  machine the code of run 1, rebuilt unchanged, measured 26.2 / 25.9 ns and 17.9 / 18.4 ns, so the change
  between the runs is the machine, not the code.

Places where an engine-specific path, chosen by a probe, would pay (none built):
- Firefox, 64 KiB reads and writes of an unshared memory: typed-array views beat memory.copy by about 20%
  (1200.6 against 1533.4 ns read, 1431.2 against 1741.8 ns write). Not for a shared memory (1581.4 against
  1556.6 ns), and not in Chrome.
- Chrome, small reads and writes: calling the access module through our own table instead of JavaScript halves
  them (variant u3 below), but needs unsafe; in Firefox it gains nothing (19.0 against 19.6 ns).
- Binding compiles a caller module each time, 15 to 36 µs. A cache per signature would leave only the
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
4. The zero-cost condition is not met by the safe routes. Unsafe that would replace them (none written; each
   built and measured in a scratch copy, same machine and session as the figures above, Chrome; Firefox):
   - Host call, now 18.3 / 17.3 ns. What is ours on it: a thread-local cell taken and put back (in the release
     wasm: a shadow-stack frame, about 20 loads and stores, std's thread-local state check), the engine-number
     compare, a bounds check, and a dynamic call into the boxed closure. Replacement (u1): keep each closure as a
     `Box<F>` in the engine, give the host module its address and the monomorphic runner
     `extern "C" fn run_host_function<F: Fn()>(address: u32) { let function = unsafe { &*(address as usize as
     *const F) }; function(); }`. Measured 4.6 / 5.2 ns. With only the lookup removed and the dynamic call kept
     (u1dyn, `unsafe { &*(address as usize as *const Box<dyn Fn()>) }`): 7.2 / 7.0 ns. So the lookup costs
     about 10.5 ns and the dynamic call about 2.5 ns. Caveat: a host module's `Function` can outlive its
     engine in JavaScript, and then the address dangles; a real change needs the engine to cut the host
     module off when dropped (for example a per-engine table it nulls), not built or measured.
   - Memory copy, now 23.5 / 22.0 ns at 64 B. Replacement (u3): the access module checks bounds itself and
     returns 0 or 1 instead of trapping, is placed in our own function table, and is called with `let entry =
     unsafe { core::mem::transmute::<usize, extern "C" fn() -> u32>(index as usize) }; entry() != 0`.
     Measured 10.8 / 10.6 ns in Chrome and 19.0 / 18.2 ns in Firefox; 64 KiB unchanged. Needs our table to be
     growable (`-C link-arg=--growable-table`).
   - Call into a module, now 26.6 / 25.9 ns. Replacement (u2): the caller module's export in our own table,
     called with `unsafe { core::mem::transmute::<usize, extern "C" fn()>(index) }()`. Measured 17.6 / 19.0 ns.
     Not usable: WebAssembly cannot catch a trap, so a trap or an uncaught exception in the guest would unwind
     through our frames with nothing to catch it, and `invoke` could never answer `InvokeError`. The JavaScript
     hop is the standard's only way to catch one.
   No vetted library covers these: each turns an integer from outside Rust into a reference or a function.
5. The workspace's `forbid(unsafe_code)` does not see macro output. glue.rs's `#[wasm_bindgen] extern` block
   expands to wasm-bindgen's generated code for each of its 38 functions, unsafe included, the same code js-sys
   holds for its own imports.
6. The suite's crate fails `cargo clippy -D warnings` (`ok().expect()` at conformance.rs lines 259, 309, 463).
   It is his file, so I left it alone. Clippy of this crate alone uses `--no-deps`.
7. Safari is not tested: driving it needs `safaridriver --enable` (administrator) and "Allow remote automation",
   both system settings on the Mac.
8. The suite does not check that a global keeps a NaN's bits, that host functions are dropped with their engine,
   or that a host function can bind in another engine; behaviour.rs checks them for this engine. They may
   belong in the suite.
9. A panic in a host function makes the call panic (as under wasmtime); the engine's host functions and context
   are lost with it, because a panic is an abort on wasm32-unknown-unknown. That is what the interface's silence
   leaves; a ruling on what a host function's panic should do would settle it.

## Dependencies (licences; `unsafe` keyword counts in the sources)

- wasm-bindgen 0.2.130 (MIT OR Apache-2.0): 141; its macro-support 0.2.130: 61 (in generated code); shared and
  macro: 0.
- js-sys 0.3.107 (MIT OR Apache-2.0): 37.
- wasm-encoder 0.263.0 (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT): 0. The workspace already uses it.
- Test only: wasm-bindgen-test 0.3.80 and wasm-bindgen-futures 0.4.80 (MIT OR Apache-2.0).

## Departures from the plan

- Memory copies use an access module (memory.copy between the guest's memory and ours) instead of typed-array
  views: 3 to 6 times faster at 64 bytes in both browsers.
- Browsers run on the x86 machine, not the ARM one (no ARM64 ChromeDriver).
- The two-worker test loads our code in the second worker from the files wasm-bindgen-test-runner serves
  (wasm-bindgen-test.js and its wasm), the only way a test there can start our code in another worker.
