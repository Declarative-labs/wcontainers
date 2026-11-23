# Status: engine-browser

The WebAssembly engine interface (crates/webassembly-engine) run on a browser's own engine through the standard
WebAssembly JavaScript API, from our Rust code compiled to wasm32-unknown-unknown and running in a worker.
Crate: crates/webassembly-engine/implementation/browser, package wcontainers-webassembly-engine-browser.

## Where it stands (milestone E1, run 1, 2026-11-22)

- Done: the crate, the whole suite passing in headless Chrome 158 and headless Firefox 160 in a dedicated worker,
  a test where a second worker uses a module and a shared memory the engine made, the benchmark and its figures
  for both browsers. No unsafe in the crate; `[lints] workspace = true`; clippy with warnings denied is clean for
  wasm32 and for the host.
- Not done: Safari (below). The owner's linter was not run (the notice of 2026-11-22 says not to for now).
- Next: the items under "For the owner" wait for his rulings; caching the caller module per signature would
  cut the cost of binding (below).

## How to run

- Everything runs on the rented x86 machine, which has Chrome 158 installed (Chrome for Testing has no
  ChromeDriver for ARM64 Linux, so the ARM machine cannot drive its Chrome through WebDriver).
- Tools, once, under /work/engine-browser on that machine: the scratch script
  pyro-parked/wcontainers-scratch/engine-browser/setup-tools.sh installs ChromeDriver 158.0.8127.36, Firefox
  160.0.1, geckodriver 0.37.1 and wasm-bindgen-cli 0.2.130 (the version wasm-bindgen is pinned to).
- `x86-run <checkout> --name engine-browser -- 'WC_BROWSER_TOOLS=/work/engine-browser bash
  crates/webassembly-engine/implementation/browser/run-in-browsers.sh --test conformance --test workers'`
  runs the suite and the two-worker test in Chrome, then Firefox. For the figures: `... run-in-browsers.sh
  --release --test benchmark -- --nocapture`.
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
    and works for a function from any source (an export, a table, a global). `invoke` writes the arguments to
    the slots, makes one JavaScript call of the caller's export (glue.js `call`, which catches a throw), and
    reads the result. A thrown `WebAssembly.Exception` is `InvokeError::Exception`, anything else a trap.
  - `bind_host_function` boxes the function into this thread's list and instantiates a host module whose
    export has the bound type. The guest calls it directly, wasm to wasm. It writes the arguments to the slots
    and calls `run_host_function` straight out of our function table (the `extern "C" fn` taken as a table index
    with a safe cast), which runs the boxed function. No JavaScript runs on a host call.
  - The host context is boxed in the engine and lent through a `Cell<Option<Box<_>>>` for the length of each
    `invoke` and `instantiate`. That is a pointer move each way and no run-time borrow check.
- Memory reads and writes: an access module per memory that imports it and our own memory and runs
  `memory.copy` (multi-memory). Out of bounds traps, which is answered with the interface's error. This
  replaced typed-array views after measuring both (figures below).
- Globals: JavaScript cannot tell a global's type, so the engine finds it by which of 12 small modules (6
  value types, constant or variable) instantiates with the global. Vector globals, which JavaScript cannot
  read, write or create, go through a module of their own and the first slot.
- `Module: Send + Sync` and `SharedMemory: Clone + Send + Sync` hold only a registration id (a number). The
  JavaScript object stays in a per-worker registry in glue.js and leaves it when the handle is dropped. Two
  engines of one worker share it. Another worker learns the object when it is posted:
  `module_to_post`/`receive_module` and `shared_memory_to_post`/`receive_shared_memory`. Instantiating a module
  this worker does not know returns an error saying so. Nothing claims thread safety a JavaScript object lacks.
- `compile_streaming` keeps the interface's default: the stream it is given is a Rust stream of chunks, which can
  never be handed to `WebAssembly.compileStreaming` as a Response without copying.
- Value types without a trait (the linter forbids a trait in an implementation): `FunctionTypeBinder` is one
  blanket impl per arity, 0 to 16, generated by a macro, over any `'static` types. Each type is matched to a
  value type by `TypeId` (i32, u32, i64, u64, f32, f64, u128, i128; results `()` or one of those). The match
  is resolved when the code is compiled. A type that is not a value type is refused when binding.

## Glue: hand-written JavaScript or js-sys's generic objects (measured, x86 machine below)

- An empty call: our route (one call into glue.js plus the caller module) took 16.8 ns in Chrome and
  14.1 ns in Firefox. js-sys's `Function::call0` on the export took 35.7 ns and 57.3 ns.
- A 64-byte read: js-sys's typed-array route took 186.1 ns in Chrome and 203.8 ns in Firefox. The glue's
  typed-array route took 102.2 / 106.8 ns. The access module took 22.9 / 20.0 ns.
- Chosen: hand-written glue (40 imported functions, each one call of the JavaScript API) and the access module.

## Measurements (x86 machine: c7i.metal-48xl, 96 cores, load 0.5 to 0.7; release build; one run each)

Chrome 158 headless, then Firefox 160 headless, dedicated worker:
- compile a small module (two functions, one import): 10.5 µs; 34.4 µs
- instantiate it: 5.0 µs; 2.8 µs
- bind a function (compile and instantiate a caller module): 16.0 µs; 36.2 µs
- empty call into a module: 16.8 ns; 14.1 ns
- empty host function called from a module in a loop: 6.9 ns; 8.3 ns
- the same, touching the host context: 7.4 ns; 8.6 ns
- read / write 64 B, unshared memory: 22.9 / 22.1 ns; 20.0 / 20.4 ns
- read / write 64 B, shared memory: 22.0 / 22.0 ns; 20.2 / 20.8 ns
- read / write 64 KiB, unshared: 969.5 / 1289.1 ns; 1534.6 / 1693.0 ns
- read / write 64 KiB, shared: 973.9 / 1292.0 ns; 1533.4 / 1685.0 ns
- write a function into a table: 93.6 ns; 44.6 ns
- Before the switch to the access module, the glue's typed-array views gave 64 KiB read / write 1134.1 /
  1380.8 ns in Chrome and 1077.2 / 1413.8 ns in Firefox (unshared), and 1559.2 / 1762.7 and 1531.0 /
  1763.0 ns (shared).

Places where an engine-specific path, chosen by a probe, would pay (none built):
- Firefox, 64 KiB reads and writes: typed-array views were faster than memory.copy (1077 against 1535 ns).
  Choosing by size there would pay about 30% on large copies.
- Binding compiles a caller module each time, 16 to 36 µs. A cache per signature would leave only the
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
4. Unsafe that would replace safe routes (none written; gains not measured):
   - A host call goes through a thread-local `RefCell<Vec<Box<dyn Fn()>>>`: one borrow check, one bounds
     check and one dynamic call, inside 6.9 to 8.3 ns in all. The replacement: the host module passes the
     address of the boxed function and a generic `extern "C" fn run::<F>(address: u32)` does
     `let function = unsafe { &*(address as usize as *const F) };`.
   - `invoke` makes one JavaScript call. The replacement: place the caller module's export in our own function
     table and call it through `unsafe { core::mem::transmute::<usize, extern "C" fn()>(index) }`, with no
     JavaScript on the way. Expected to bring an empty call near the host-call cost; not measured.
   No vetted library covers either: both turn an integer from outside into a Rust reference or function.
5. The workspace's `forbid(unsafe_code)` does not see macro output. glue.rs's `#[wasm_bindgen] extern` block
   expands to wasm-bindgen's generated code for each of its 40 functions, unsafe included, the same code js-sys
   holds for its own imports.
6. The suite's crate fails `cargo clippy -D warnings` (`ok().expect()` at conformance.rs lines 259, 309, 463).
   It is his file, so I left it alone. Clippy of this crate alone uses `--no-deps`.
7. Safari is not tested: driving it needs `safaridriver --enable` (administrator) and "Allow remote automation",
   both system settings on the Mac.

## Dependencies (licences; `unsafe` keyword counts in the sources)

- wasm-bindgen 0.2.130 (MIT OR Apache-2.0): 141; its macro-support 0.2.130: 61 (in generated code); shared and
  macro: 0.
- js-sys 0.3.107 (MIT OR Apache-2.0): 37.
- wasm-encoder 0.263.0 (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT): 0. The workspace already uses it.
- Test only: wasm-bindgen-test 0.3.80 and wasm-bindgen-futures 0.4.80 (MIT OR Apache-2.0).

## Departures from the plan

- Memory copies use an access module (memory.copy between the guest's memory and ours) instead of typed-array
  views: 4 to 5 times faster at 64 bytes in both browsers.
- Browsers run on the x86 machine, not the ARM one (no ARM64 ChromeDriver).
- The second worker in the two-worker test is plain JavaScript that instantiates the posted module with the
  posted memory. Our own code does not run there. `receive_module` and `receive_shared_memory` are checked in
  the first worker with a second engine.
