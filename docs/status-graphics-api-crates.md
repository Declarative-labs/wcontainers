# Status: the graphics API crates

Brief: wcontainers-scratch/orchestrator/restart-2026-11-25/brief-graphics-api-crates.md. Branch `graphics-api-crates`,
made from the owner's main 95bceadf0. Marks: MEASURED (a run; its job named), READ, INFERRED.

## AC1, the generator and the trait layer (2026-11-25)

`tools/api-crate-generator` reads Microsoft's Win32 metadata (version 63.0.31, fetched and checked by
`fetch-metadata.sh`) and writes, for the set `direct3d-11`, four crates: `component-object-model`,
`direct3d-common`, `directx-graphics-infrastructure` and `direct3d-11`, each an interface.rs with its data files
(constants, enumerations, structures) and a manifest. `check-output.sh` writes them twice, compares, and builds
them in a workspace of their own beside copies of volatile-bytes, byte-sink and byte-source.

MEASURED (ARM64 job gac, 2026-11-25 22:2x, `LINTER=... check-output.sh direct3d-11 /work/target/gac/ac1`):

| Bar | Result |
|---|---|
| The generator builds under `forbid(unsafe_code)` | status 0, no warning; its test passes |
| A second run is byte-identical | `diff -r`: IDENTICAL |
| Output compiles under the workspace lints | `cargo check --workspace`: status 0, no warning |
| Clippy | `cargo clippy --workspace -- -D warnings`: status 0 |
| Every in-scope interface and method once | direct3d-11: 77 interfaces + the top trait = 78 traits; 440 methods + 2 exports = 442 `fn`; DXGI 47 + 1 traits, 191 + 6 `fn`; d3dcommon 2 + 1, 4; no method twice in a trait |
| Every parameter mapped by a row or listed | the rows' counts are printed in the report; 39 places listed as unmapped (below) |
| Linter on the output | 17 findings in the four crates (from 9,263 at the first run): comment_budget_exceeded 9, conformance_missing 3, independent_trait 2, associated_type_unused 2, summary_too_long 1 |

The 15 interfaces of other libraries (d3dcompiler_47.dll's reflection and linking, d3dcsx.dll) and
d3dcommon's ID3DInclude are left out by name in src/api.rs. 49 object kinds in Direct3D 11, 16 in DXGI, 2 in
d3dcommon.

Unmapped, each to get an exception line at AC2 or AC3 (from the report): the shader bytecode is mapped (an array of
void is bytes); left are UpdateSubresource, UpdateSubresource1, WriteToSubresource, ReadFromSubresource,
UpdateTiles (byte lengths that follow from a box and pitches), the video extensions' `pData`,
GetDecoderBuffer (memory lent like Map), ID3DBlob::GetBufferPointer, RegisterDestructionCallback (a callback),
and pointer fields whose count is another field the metadata does not link (the info queues' filter lists,
DXGI_PRESENT_PARAMETERS's rectangles, the messages' descriptions, the video decoder buffers, D3D11_MAPPED_SUBRESOURCE
and D3D11_SUBRESOURCE_DATA, SECURITY_ATTRIBUTES).

Linter findings left, and why (each with its smallest example in the job's linter.txt):
- comment_budget_exceeded (9 files): every named number needs its link just above it (number_constant_without_link),
  and a file that is mostly constants then holds more comment than 14 characters a line allows. The doc is already
  one short sentence. Not loosened; it goes to the orchestrator as a rule interaction.
- independent_trait, associated_type_unused (ID3DBlob, ID3DDestructionNotifier): no signature of this set names
  them; Direct3D 12 and 10 will.
- conformance_missing: the generated suites are AC3.
- summary_too_long: one DXGI message name is longer than the summary limit.

Decided here (also in the brief): the generator is a tool in its own workspace, so the repository's cargo commands
never build it. CORRECTED 2026-11-25: an earlier line here and in the brief said the linter never builds it either;
that is false. The linter walks every folder outside crates/ and reports a folder with a Cargo.toml that is not a
workspace member, so the generator's findings are reported and counted, with no exclusion and no change to the root
manifest. Where a standalone program lives is asked of the owner for all such programs together. A C keyword name gets a trailing
underscore (the linter does not read raw identifiers); the reasoned `explicit_shared_error` allow sits on each trait
with two or more HRESULT methods and names `HResult`, which interface.rs imports; a root interface whose methods never
name another object holds no `type Api`.

SECURITY: no `unsafe` in the generator or its output (both `forbid`); one new tool dependency, windows-metadata 0.60.0
(Microsoft, MIT or Apache-2.0, 17 lines with `unsafe`, no dependencies of its own), never in the product. No
hostile-guest suite is run by this milestone.

## AC2, started: the C layout the glue rests on (2026-11-25)

`check-layout.sh` has the generator write one compile-time assertion per record size and per named field offset, by
C's rules from the metadata (packing included), and compiles them with zig 0.16.0 against MinGW's headers for
x86_64-windows-gnu. MEASURED (ARM64 job gac/20261125-222419-478254): 1,097 assertions over Direct3D 11, DXGI,
d3dcommon and the Foundation records they name; compile status 0, 0 failing; a planted wrong size fails exactly once.
25 records MinGW's headers do not declare are listed by the job and left out (shader tracing and reflection,
d3dcsx, D3DMATRIX and D3DVECTOR, DXGI's JPEG tables, two newer video records).

## AC2, continued (2026-11-26): COM's bookkeeping, the hang fixed, both glues of the slice

Logs: wcontainers-scratch/graphics-api-crates/ac2/. All jobs on the x86-64 machine.

- **COM's bookkeeping** (`component-object-model/implementation/direct`, written by the generator from
  templates/component-object-model-direct): the object table, foreign objects, IUnknown, the C data pointers, and
  lent memory. Input bytes are copied out by memcpy, never viewed as writable (his `PointerAndLength::new` promises
  writable memory). 89 lines with `unsafe`, 21 marks (MEASURED, slice-clippy-and-counts.log).
- **The hang the spike found is fixed and tested** (MEASURED): `ForeignLent::read_to` copies the bytes out before the
  sink runs and `write_from` lets the source fill a buffer of ours first, so no lock is held while code of either
  side runs. The same two tests (a sink or source that revokes, as Unmap does, from inside the call; 10 s limit):
  spike's `Lent` FAILED both after 10.00 s, "HANG: read_to did not end within 10 s" (spike-hang.log); the generated
  crate passes both, also under AddressSanitizer (runtime-check.log, round-trip-asan.log).
- **The slice set** `direct3d-11-slice` (src/api.rs `only`): the spike's 27 methods (24 of Direct3D 11, 3 of DXGI)
  with the interfaces they need; the C tables keep every entry of the full metadata, those left out null.
- **Both glues, generated** (src/glue.rs, glue_write.rs, glue_call.rs) into each API's `implementation/direct`:
  the C tables (one per interface, every slot in the metadata's order), serving entries generic over the object's
  type, each kind's table of interfaces (with DXGI's surface on Direct3D 11's textures), the exports, the calling
  glue's object types and a `System` trait binding each kind to them. Records cross as bytes at the offsets C's
  layout gives (the layout check asserts them against MinGW's headers). An object handed out again (GetDevice,
  GetImmediateContext) is answered from relations the glue recorded when the objects crossed, never by asking the
  implementation. MEASURED (slice-and-full-check.log): generator builds with no warning; slice written twice
  IDENTICAL; `cargo check` and `cargo clippy -- -D warnings` status 0 over the four crates and their direct crates;
  the whole set (AC1) unchanged: 77 interfaces / 440 methods, 47 / 191, IDENTICAL, check and clippy 0.
- **Glued 22 of the spike's 27** (and 2 of d3dcommon): GLUED D3D11CreateDevice, D3D11CreateDeviceAndSwapChain,
  GetDataSize, Buffer/Texture2D/Query GetDesc, the three Create*View, CreateQuery, GetImmediateContext, GetDevice,
  Begin, Draw, End, GetData, IASetVertexBuffers, OMSetRenderTargetsAndUnorderedAccessViews, Unmap, GetType, DXGI
  Surface GetDesc, Present. NOT GLUED (null in our tables; E_NOTIMPL when called), each waiting on a trait-layer
  exception line: CreateBuffer and CreateTexture2D (initial data: its length follows from the description, and
  CreateTexture2D's is an array of subresources); Map (`D3D11_MAPPED_SUBRESOURCE.pData`: memory lent until Unmap);
  SetPrivateDataInterface (a foreign IUnknown, which the serving glue cannot make into the implementation's
  `Unknown`); GetBuffer (riid + void**: the swap chain's buffer is a Direct3D 11 texture, which DXGI's
  `UnknownObject` cannot name).
- **Pass-through on Linux** (MEASURED, round-trip.log; round-trip-asan.log under AddressSanitizer, 24
  `__asan_report` calls in the binary): a toy implementation served as C exports and called back through the
  calling glue as if it were the system's library: device creation with feature levels, a query's description
  both ways, GetDataSize, Draw, Begin/End, GetData writing through the caller's `VolatileBytes`, E_NOTIMPL for a
  method not glued, the context's and a child's device being the objects created (COM's identity), and every toy
  object released at the end. 1 test, passes.
- **Marks** (MEASURED `expect(unsafe_code` count): direct3d-11 direct 84, directx-graphics-infrastructure direct 13,
  direct3d-common direct 7, COM direct 21; lines naming `unsafe`: 380, 69, 47, 89. INFERRED against SPIKE.md's 130
  marks for the same 27 methods (86 serving, 44 calling): 97 marks for 22 glued methods plus every kind's
  bookkeeping, about 4.4 a method for both glues against the spike's 4.8. Not yet split per method.
- **Linter on the slice's output** (MEASURED, slice-lint-check.log, linter.txt): 1,236 findings in the crates
  written: public_outside_interface 690 (every `pub` item of a direct crate's files), magic_literal 373 (the sizes
  and offsets of the byte forms), conformance_not_run 38, implementation_folder 37, trait_not_implemented 20,
  comment_budget_exceeded 19, summary_not_one_line 15, comment_too_long 10, summary_too_long 9,
  number_constant_without_link 7, trait_outside_interface 4, summary_not_one_sentence 4, conformance_missing 3,
  summary_starts_with_name 2, independent_trait 2, associated_type_unused 2, summary_repeated 1. Nothing loosened.

Known gaps of the start of AC2 (1 to 4 done below); left:
5. The linter's shape findings above: whether a direct crate may hold public items outside an interface, and
   whether the byte forms' offsets need names. The linter was not rerun on the slice after the restart.

## AC2, finished parts (2026-11-26, after the restart; logs in wcontainers-scratch/graphics-api-crates/ac2/)

All jobs on the x86-64 machine, the slice written by the generator, built in a workspace of its own.

- **All 27 methods of the slice glued in both glues**, plus `ID3D11Multithread::SetMultithreadProtected` (the calling
  glue calls it): report line "GLUED 30" (2 of d3dcommon's that the spike did not have); NOT GLUED only d3dcommon's
  GetBufferPointer and RegisterDestructionCallback, outside the spike's 27 (MEASURED, report.txt).
- **Exception lines** (src/api.rs, row ExceptionLine in MAPPING.md): initial data at the length the description and
  pitches give (one record per subresource for a texture); Map's memory as the top trait's `Mapped: VolatileBytes`,
  lent by the serving glue as a shadow of ours written back at Unmap, held by the calling glue as `ForeignLent`
  revoked before the system's Unmap; GetBuffer's buffer as DXGI's `Buffer`, which Direct3D 11 binds to its texture;
  multithread protection of the immediate context at creation (refused without it); a non-null module handle
  refused; the header's count ranges and keep values (OMSetRenderTargetsAndUnorderedAccessViews, IASetVertexBuffers,
  FeatureLevels at most 16); descriptions within d3d11.h's ranges in both glues; descriptions, kinds and query data
  sizes answered from what the glue recorded, never asked of the implementation; a returned feature level one of
  those asked; a back buffer index below the recorded count. An `IUnknown*` of no kind of the API is COM's
  `Foreign` (UnknownIn row changed), so the per-API `type Unknown` is gone. Unions in an input structure are written
  as their largest non-zero member.
- **Defects the harness found and the generated glue fixes**: an object made for a null `_COM_Outptr_opt_` (Direct3D's
  validation-only call) was handed out through the null pointer and its reference lost (pass-correct left 379 fake
  objects; now 0); counts past the header's range read garbage arrays (SIGSEGV in the output-merger attack); a 256-byte
  stack over-read of feature levels (ASan); a shadow sized from an implementation's pitches let the C side write past
  it (ASan heap overflow), now bounded by the recorded description and the implementation's own bytes; relations
  keyed by Rust type confused kinds served by one type (now keyed by role).
- **The spike's harness on the layout** (`crates/direct3d-11/implementation/direct/harness`, written beside the glue):
  the spike's C (fake d3d11, game, attacks, check-2's games, fuzz drivers) unchanged but two counters, the front ported
  to the generated traits, a new lying implementation, and the tests game, attack_c, attack_table, check2_c, fuzz,
  measure. MEASURED (harness-all.log): 21 tests, each alone under a time limit, in the default build, the
  small-table build and under AddressSanitizer: 63 of 63 status 0; the C instrumented (30 `__asan_report` symbols).
- **Marks per method** (MEASURED, marks-per-method.txt): serving 75 marks over 30 entries = 2.50 (spike 3.2);
  calling 69 over 45 methods = 1.53 (spike 1.6); the rest are per kind or per table (93 in Direct3D 11's crate, 30
  in DXGI's, 19 in d3dcommon's) and COM's 35. Totals: 321 marks against the spike's 182, because each kind of the
  full chain gets its tables, adoption and `Adopt` impl.

- **Fuzzing, 4 modes x 3 seeds (11, 12, 13) x 10 million calls under AddressSanitizer, 8 threads each** (MEASURED,
  fuzz-long.log): 12 of 12 status 0; 0 AddressSanitizer reports, 0 hangs, 0 crashes; liar + correct game: 0 promises
  broken (about 397,000 resources, 110,000 mappings written, 78,000 query results, 65,000 back buffers, 836,000 lies
  per run); front, pass-through and liar + hostile game: 8 of 8 kinds of table reached each. Before the fixes above
  the same runs found the defects listed (fuzz-long-first-killed.log: the liar runs killed for memory).
- **Cost per call, ns** (MEASURED, measure.log; median of 31 runs, pinned to core 13; load 45 before, 23 after, the
  spike's was 4.6):

| Operation | Direct (fake) | Added by the serving glue | Pass-through | Spike: added / pass-through |
|---|---|---|---|---|
| Draw | 15.0 | +24.1 | 46.6 | +23.9 / 37.8 |
| AddRef + Release | 14.5 | +26.4 | 43.1 | +24.7 / 37.4 |
| Map + Unmap | 60.6 | +344.4 | 545.2 | +215.1 / 455.9 |
| CreateBuffer + Release | 68.1 | +455.4 | 547.3 | +337.3 / 397.4 |
| QueryInterface + Release | 14.7 | +78.9 | 91.3 | +80.8 / 93.7 |
| GetDevice + Release | 15.8 | +101.5 | 117.6 | +60.2 / 72.9 |

  Within 10% of the spike: Draw and AddRef+Release added, QueryInterface. Not within it, and why (INFERRED): Map+Unmap
  copies the implementation's bytes into the shadow and back (the spike lent the implementation's memory); CreateBuffer
  copies the initial bytes once (the brief's decided cost, question 2 removes it), checks the description and pushes
  the recorded description; GetDevice looks up relations by role. The machine was ten times busier than for the
  spike's table.

## AC2 repaired after the check of ec8df4b51 (2026-11-26; logs in wcontainers-scratch/graphics-api-crates/ac2r/)

The check did not hold: safe code could call `set_multithread_protected(false)` through the calling glue and turn off
the protection its `Send` and `Sync` rest on; `the_toy_through_both_glues` failed at the tip; the whole set's build
listed a harness glob it does not write; Map+Unmap, CreateBuffer and GetDevice cost more than the spike; two seeds gave
one game; the fake never lowered its count of contexts. Every bar below is MEASURED at ab721bebf (x86-64 job
gac-r/20261126-1129xx, `.gactip.sh`, log tip-ab721bebf.log and the folder tip-ab721bebf/).

- **The class, closed in the generator** (src/undo.rs, the `pins` of src/api.rs): every mark's reason is matched to the
  facts it relies on (layout, a live object, free threading, a call that returns, memory lent until its end, no
  callback, the C caller's pointers, the library's exports, the glue's key, the harness's stand-ins); a reason no line
  matches fails the generator. For each fact a search over the WHOLE metadata of every API of the set (a slice
  included) finds the methods that could undo it (an interface named Multithread; Deferred, DeviceContextState; a flags
  type with a SINGLETHREADED or DEBUG value; ExceptionMode, BreakOn; Map, GetDC, ReleaseDC, Discard, Offer, Reclaim,
  UseRef, Resize, Trim, Evict, DecoderBuffer; Release, Destroy, Free, PrivateData; a delegate or Callback). Each method
  found needs a pin: answered (SetMultithreadProtected answers TRUE and never reaches the system), a value cleared
  (D3D11_CREATE_DEVICE_SINGLETHREADED in both exports, the single-threaded context state flag), a value refused
  (SetExceptionMode non-zero, SetBreakOn* TRUE in both info queues), never called (Enter, Leave, CreateDeferredContext*,
  Discard*, DXGI's surface Map/Unmap/GetDC/ReleaseDC, Offer/Reclaim*, MapDesktopSurface, SetUseRef, decoder buffers,
  RegisterDestructionCallback), or shown harmless with the reason. An unpinned method, a stale pin, or a pin of a method
  the slice's calling glue carries without its hostile test fails the generator. Report: 341 marks, 219 reasons, 85
  pins, 0 failures (slice); the whole set's 35 marks of COM's crate, 0 failures.
- **Hostile tests** (harness tests/undo.rs, and the C attack `attack_undo_threads`): through the calling glue over the
  fake, SetMultithreadProtected(false) twice on the context and on the multithread object answers TRUE and the fake sees
  0 unprotections; a single-threaded device asked through both exports reaches the fake without the flag and protected;
  the same C program through both glues gets TRUE and the fake sees nothing undone, while on the fake directly (the
  control) it turns protection off. The checker's own probe, unchanged, under forbid: "protection before the first call
  true, before the second true", status 0.
- **The tip's failures fixed**: the round trip asked an 88-byte query for 4 bytes (now an event query, 4 bytes) and read
  a discarding Map's contents (now READ_WRITE; a discarding Map lends cleared memory, a read-only Map writes nothing
  back, both tested); the whole set's workspace lists the harness glob only when a harness is written.
- **Small faults**: the fuzzer's first state is splitmix64 of the seed and the thread (seeds 12 and 13 now give
  10000197 and 10000207 calls in pass-correct); the fake lowers its count of contexts when a device dies (pass-correct
  alive by kind all 0).

| Bar at ab721bebf | Result |
|---|---|
| Generator builds, test | status 0, 0 warnings |
| Whole set (AC1): written twice | IDENTICAL; 77 / 440 Direct3D 11, 47 / 191 DXGI, 2 / 4 d3dcommon; unmapped 37 lines |
| Whole set: check, clippy -D warnings, tests | 0, 0, 0 |
| Slice: written twice; check; clippy; clippy --all-targets; direct crates' tests | IDENTICAL; 0; 0; 0; 0 (the round trip passes) |
| Harness, each test alone, default + small-table + AddressSanitizer | 69 of 69 status 0 (23 tests, undo included) |
| Fuzzing, 4 modes x 3 seeds x 10M, ASan, plus front-correct and pass-correct | 16 of 16 status 0; 0 reports, 0 hangs, 0 promises broken |
| Linter, whole set / slice | 283 / 2,069 findings (public_outside_interface 130 / 769, magic_literal 53 / 924, rest in the logs) |

- **Costs: the cause, by profile** (perf on the served path, profile/perf-*.txt): GetDevice looked our object up twice
  (once for `this`, again inside `related`); CreateBuffer took a counted reference of the device and looked the new
  object up again only to relate them, grew the record past its first allocation, and gave back two vectors per dead
  object; Map filled the shadow even for a discarding Map and wrote back after a read-only one. Removed: the second
  lookup (`related_of`, and `made_by_of`, which reads the maker from the record under the word's lock with no reference
  taken; the maker is immutable in the record and read with no lock), the relation made in the record before serving
  (`record_related`), the record's room for its parts, the vectors at death, the fill for a discarding Map (cleared
  instead: no earlier lending's bytes), the write-back after a read-only Map, and the pool's lock for a thread's own
  blocks. Same core, one after another, low load (measure-tip-b93d1d922-side-by-side.log, spike's run.sh measure):

| ns, same core 92, load 10 | Ours: added by serving / pass-through | Spike: added / pass-through | ec8df4b51 (A/B, load 6) |
|---|---|---|---|
| Draw | 31.6 / 58.6 | 24.1 / 37.8 | 22.7 / 43.7 |
| GetDevice + Release | 53.5 / 68.9 | 60.5 / 73.1 | 104.2 / 122.7 |
| Map + Unmap | 308.0 / 496.3 | 215.5 / 456.9 | 304.1 / 543.1 |
| CreateBuffer + Release | 271.5 / 343.9 | 353.6 / 397.2 | 382.5 / 478.3 |

  At ab721bebf (load 29 to 35): Draw 24.1 / 46.1, GetDevice 57.9 / 74.5, Map+Unmap 324.0 / 459.8, CreateBuffer 271.8 /
  366.3. What remains, and why it is needed (profile): Map+Unmap's pass-through is within 9% of the spike's; its added
  cost is four lookups (context and resource, each a word's lock and a counted `Arc`, 17%), four round trips of the
  lendings' lock (11%: a subresource stays busy while the implementation's map and unmap run, with no lock held), and
  the shadow written back through the implementation's `VolatileBytes` at Unmap (the C side needs raw memory, and bytes
  another party may change pass only through `VolatileBytes`). Draw's added cost is one lookup (31% of a served Draw:
  the word's lock and the `Arc` that keeps the object alive while the implementation runs with no lock); run-to-run
  spread of the same binary on this shared machine is 10 to 20% (the front alone, which no change touched, measured 120
  and 147 ns for Map in two builds).

## AC3, started (2026-11-26, commit 47e9f83bb)

The set `direct3d-11-glue` (src/api.rs; the trait-layer set `direct3d-11` is unchanged and writes no glue) writes both
glues for the whole of Direct3D 11, DXGI and d3dcommon. MEASURED (x86-64 jobs gac-r 2026-11-26 11:4x, report
ac2r/ac3-report.txt): written twice IDENTICAL; `cargo check` and `cargo clippy -- -D warnings` status 0; type-checked for
x86_64-pc-windows-gnu (the spike's toolchain, which carries that target; this repository's does not), status 0, and the
slice too; GLUED 461 of 643, NOT GLUED 182, each with its reason (most: 36 DXGI info-queue methods taking a GUID by value,
32 arrays of objects out, 7 shader bytecodes, structures holding a pointer the glue cannot copy as bytes); 2,359 marks,
all matched to their reliances, 85 pins, 0 failures; 0 lines with `unsafe` outside the direct crates. Fixed on the way:
a required `IUnknown*` in the calling glue, a face's trait impl written once per type, a face's methods typed by its own
API, the other library's exports in the supertrait's impl (answered E_NOTIMPL: its own API's System loads it), a
parameter named `status`, a reborrowed array element, `index * 1`, and an enum's interface picked by a hash map's order
(two APIs each have a `Resource` trait: the whole set wrote differently on each run).
Left of AC3: the 182 methods; a generated hostile C caller over every table and a generated lying implementation; the
generated conformance suite; fuzzing 3 seeds x 10M over every table under AddressSanitizer; the linter per rule; the
hostile tests the pins of the whole set owe (SetExceptionMode, the info queues' breaks, CreateDeviceContextState, the
calls never made), which the generated caller is to try.

## AC3, after the check of 8beac0bb8: default deny (2026-11-26, commit 330c3352b)

The check did not hold on the claim that the safety hole was closed as a class: safe code reached
`IDXGIFactory::CreateSoftwareAdapter(module)` and the system would load and run code from the module. The cause was the
method: src/undo.rs searched the API by names and flag values for what could undo a reliance, so a name the list missed
got through. Replaced by default deny (src/kinds.rs); src/undo.rs now only matches each mark's reason to what it relies on.

- **Kinds.** Every parameter and return of every method the set carries is classified by its metadata type into nine
  kinds, each with one rule (printed as `RULE` lines): number; flags (an enumeration, by type or `AssociatedEnum`, or a
  number named Flag/Mask/Option the metadata links to none); object of a kind of the set; memory the caller lends for the
  call; memory lent until a later call (refused unless an exception line holds it); a handle the kernel checks (HANDLE,
  HWND, HDC, HMONITOR); a handle or address the system trusts (HMODULE, HINSTANCE, LPARAM, WPARAM, function pointers,
  raw `void*` with no size, `void**` not an object, an integer of pointer width passed in, an object of no kind of the
  set); text; a structure (its fields by their own kinds). A type of no kind fails the generator, and so does a typedef
  src/api.rs `HANDLES` does not name. A trusted part, or memory lent until later with no line, makes the calling glue
  refuse the method (E_NOTIMPL, the system never called) unless a row of src/api.rs gives that exact method's rule with
  its reason and hostile test.
- **Every value judged.** Every enumeration met has one judgment (`Any`: every value plain, with its reason; or
  `Declared`: only declared values or bits pass, the glue refusing others), and special values cleared or refused
  wherever a parameter passes them, each with a hostile test. An enumeration judged by declared or special values inside
  memory (a structure, an array) makes the method refused: the glue checks them only on a parameter by value.
- **Effects judged.** Every interface and export of the set has one effects judgment (src/api.rs `effects`): what none of
  its methods but the rows can do to a fact the marks rely on. These judgments are READ, not tested.
- **Rows** (`pins`, now rows for exact methods): Answers, Clears, Refuses, NotCalled, and NullOnly (a trusted handle passed
  only as null, replacing the exception line NullHandle). The rows the search had marked harmless are gone (no search
  finds them any more); no row is marked harmless. Each row and each special value names a hostile test; one missing from
  the tests written fails the generator (a never-called row may instead be shown by its calling form holding no call out).
- **Changed judgments**, by reason: D3D11_CREATE_DEVICE_DEBUG and DXGI_CREATE_FACTORY_DEBUG refused (the debug layer
  breaks, through our frames, when the DirectX control panel's registry settings say so); undeclared bits of
  D3D11_CREATE_DEVICE_FLAG and DXGI_CREATE_FACTORY_FLAGS refused; D3D11_MAP declared values only; D3D11_COPY_DISCARD in
  CopySubresourceRegion1 refused (it may drop memory a view holds, as DiscardResource, never called).
- **Interface by id glued** (`REFIID riid, void** ppv`, both glues, to make the factory reachable for the probes): the
  calling glue asks each candidate kind for its newest interface by QueryInterface, else holds a foreign object; the
  serving glue serves the kind and asks it for the id (a foreign object answered E_NOINTERFACE). 14 methods more glued.
- **The recorder** (`crates/<api>/implementation/recorder`, generated, `forbid`): every trait of the API implemented,
  every call logged with its by-value arguments, objects made on demand; and `served(module)` in each direct crate, the
  calling glue over the serving glue's own exports of any implementation (one mark each). The whole set's hostile tests
  and probes (harness/whole/hostile.rs) reach the recorder through both glues.

MEASURED, x86-64 job gac3/20261126-130049 at 330c3352b (log ac3/measure/tip-330c3352b.log, reports beside it):

| Bar | Result |
|---|---|
| Generator: build, test | status 0, 0 warnings; 2 tests pass |
| Trait layer (AC1): twice, check, clippy, linter | IDENTICAL; 0; 0; 283 findings (unchanged) |
| Slice (AC2): twice, check, clippy, clippy --all-targets, direct tests | IDENTICAL; 0; 0; 0; 0 (round trip passes) |
| Slice harness, each test alone, default + small-table + AddressSanitizer | 75 of 75 status 0 |
| Slice fuzzing, 16 runs of 10M under AddressSanitizer | 16 of 16 status 0; 0 reports, 0 hangs, 0 promises broken |
| Whole set (AC3): twice, check, clippy, clippy --all-targets | IDENTICAL; 0; 0; 0 |
| Whole set and slice type-checked for x86_64-pc-windows-gnu | 0, 0 (job gac3win) |
| Whole set: treatments of 643 methods | passed 583, checked 13, answered 1, never called 22, refused 24 |
| Whole set: parts by kind | number 827, lends 581, object 316, flags 122, kernel-handle 20, text 18, trusted 12, lent-until 2, structure 2 |
| Whole set: GLUED / NOT GLUED | 475 / 168 (was 461 / 182) |
| Whole set: tests owed by rows and special values | all found (40 lines) |
| Whole set: hostile tests and 30 probes, default and AddressSanitizer | 47 of 47 pass, both |
| Marks, matched to reasons | slice 354, whole set 2,553; 0 unmatched |
| Linter, whole set with its glue / slice | 10,521 / 2,079 (magic_literal 5,305 / 927, public_outside_interface 4,517 / 771) |

**Calling forms changed** against 8beac0bb8 (ac3/measure/compare.txt, every method of every call.rs): 686 forms; 48
changed. calls -> refused 1: IDXGIFactory::CreateSoftwareAdapter (the hole). calls -> calls with new checks 4:
D3D11CreateDevice and D3D11CreateDeviceAndSwapChain (flags' judgment, null-only module), ID3D11DeviceContext::Map
(declared values), CopySubresourceRegion1 (discard refused). not glued -> calls 17 (interface by id: OpenSharedResource,
OpenSharedResource1, CreateFence, OpenSharedFence, GetParent, GetDevice, GetResource, EnumWarpAdapter,
EnumAdapterByGpuPreference, GetCoreWindow, the three CreateDXGIFactory, DXGIGetDebugInterface1). not glued -> refused 24
(same answer, now by kind: raw pointers in UpdateSubresource(1), UpdateTiles, Write/ReadFromSubresource, the video
extensions and decoder buffers' descriptions, SECURITY_ATTRIBUTES, the info queues' messages and filters, GetBufferPointer,
RegisterDestructionCallback, CreateTexture2D1/3D1). The rest are comment changes of rows.

**The thirty probes** (safe Rust under `forbid(unsafe_code)`, through both glues to the recorder): 28 held; 2 got
through, both by rule: probe 15 (MakeWindowAssociation with any HWND) and probe 16 (OpenSharedResource with any HANDLE)
reach the system, being handles the kernel checks. Held: a module through CreateSoftwareAdapter and both exports, a
forged IUnknown, exceptions and breaks turned on, the debug layers, undeclared flag bits, protection turned off, the
device lock, a deferred context, the reference rasterizer, raw-pointer CPU access, UpdateSubresource, bytes lying about
their length, mismatched and out-of-range counts, short initial data, an oversized buffer, undeclared Map kinds, a double
Map, lent memory after Unmap (reads and writes nothing) and after every object is dropped (still the system's), a child
outliving its device, 16,000 clones and drops on eight threads (no object leaked or freed early). Three probes were
corrected after their first run, each a wrong expectation, not the glue: 19 used CreateQuery (not glued: a query or a
predicate) instead of CreateQuery1; 27 expected 256 lent bytes where the glue lends the description's bytes raised to the
system's pitches (lengths.rs `at_least_pitches`, 64 KiB here, the serving shadow holding them whole); 28 counted the
device's death, which the lent memory does not rely on (it holds the context and the resource).

Left of AC3: the 168 methods not glued (most: 39 arrays out, 37 GUIDs by value, 12 structures holding a pointer); a
generated hostile C caller over every table; the generated conformance suite; fuzzing 3 seeds x 10M over every table
under AddressSanitizer (the recorder is the first generated implementation it can drive); the linter per rule; the effects
judgments are not yet tested.

## AC3, after the check of 90b14f72a: what the marks relied on of the other party (2026-11-26)

The check did not hold: safe code could make the system write memory that was never lent. The calling glue asked a
caller's `VolatileBytes` for `len()` twice, sized its buffer by one answer and told the system the other; a caller
answering 8 then 0 (or 16 then 4096) got a heap overflow (READ: the check's probes.log, SIGSEGV in the default build,
an AddressSanitizer report under it). Repaired as a class: marked code relied on something a safe trait implemented
by the other party does not promise.

- **The list** (src/trust.rs `OTHER_PARTY`, printed in every report under `OTHER PARTY`): every safe trait the glue
  meets whose implementation comes from the caller or the implementation behind the glue, the methods the glue calls,
  and what the marks hold whatever they answer. The caller's `VolatileBytes` (len once, write once, last); an
  implementation's `Mapped` (len once, read_to once at Map, write_from once at Unmap, into sinks and sources of ours
  bounded by our shadow); every generated API trait (answers recorded, never asked again; no lock held; panics caught);
  an implementation's `Clone`, `PartialEq`, `Hash` (never called: tables are keyed by our own words' addresses and by
  role); `Drop` (after every lock, caught); slices and Options of plain data and the glue's own objects (the
  language's length; the glue writes their `Clone` and `Hash`); `Adopt` (an `unsafe fn`, which no crate under `forbid`
  can write); `Deref`, `AsRef`, `Borrow`, `Ord`, iterators (no trait signature takes a type bound by them). READ: the
  serving glue calls no `Clone`, `PartialEq` or `Hash` of an implementation (grep of serve.rs) and asks `Mapped::len`
  once.
- **Asked once** (src/glue_call.rs): each length of the caller's bytes is asked once, before any step of the call; that
  answer both sizes our buffer and is the count the system is told; the caller's `write` runs last, after every object
  is adopted. **The check** (src/trust.rs `check`, run on every set written): every function of every direct crate's
  files is read, and a second call of one `VolatileBytes` method on one value (a parameter of that type, its closure
  or `if let` binding, or an expression passed to `VolatileBytes::m` or to `fill_from`/`empty_into`) fails the
  generator. Five tests of it, among them the exact form 90b14f72a wrote.
- **The hostile implementation set** (src/glue_hostile.rs, `crates/<api>/implementation/hostile`, generated for the
  slice and the whole set, `forbid`): the recorder's text turned hostile. Every method of every trait, every object's
  `Drop` and the bytes Map lends can panic, re-enter the glue or never return at a chosen call (`plan`); `Clone`,
  `PartialEq` and `Hash` panic and are counted; every caller's bytes lent to it are written and read in full (as the
  system writes all it is told); Map's bytes answer a different length on every call, more bytes than the length,
  other bytes on every read, and pitches that lie. `LyingBytes` lies the same ways on the caller's side. A generated
  driver makes an object of every kind it can (makers: every method that hands one out, an interface by id once per
  kind) and calls every method the calling glue carries, through both glues (`served`). Tests: harness/hostile/lies.rs.

MEASURED, x86-64 job gac4/20261126-141945-2400779 at 354e36e6d (log wcontainers-scratch/graphics-api-crates/ac3r/tip-354e36e6d/):

| Bar | Result |
|---|---|
| Generator: build, tests | status 0, 0 warnings; 7 tests pass (5 of them the check's) |
| Trait layer (AC1): twice, check, clippy, unmapped, linter | IDENTICAL; 0; 0; 37 lines; 283 findings (unchanged) |
| Slice (AC2): twice, check, clippy, clippy --all-targets, direct tests | IDENTICAL; 0; 0; 0; 0 (the round trip passes) |
| Slice harness, each test alone, default + small-table + AddressSanitizer | 75 of 75 status 0 |
| Slice fuzzing, 16 runs of 10M (4 modes x seeds 11, 12, 13, + front-correct, pass-correct), AddressSanitizer | 16 of 16 status 0; 0 reports, 0 hangs, 0 promises broken |
| Whole set (AC3): twice, check, clippy, clippy --all-targets | IDENTICAL; 0; 0; 0 |
| Whole set's 47 hostile tests and 30 probes, default and AddressSanitizer | 47 of 47, both; probes 28 held, 15 and 16 through by rule (unchanged) |
| Asked once: calls of the other party's `VolatileBytes` in the direct crates | slice 7, whole set 43; 0 asked twice |
| Hostile set, 10 tests x slice and whole set x default and AddressSanitizer | 40 of 40 status 0; 0 reports, 0 hangs, 0 never-asked calls, 0 objects left alive |
| Driver's reach, honest drive | whole set: 65 kinds, 627 of 629 calls made (501 reach the implementation); slice: 9 kinds, 41 of 45 |
| Lies told, whole set (default) | caller's bytes: 9 length sequences, panics at calls 0 to 3 (19 and 18 panicked back, caught), re-entry, 37 calls blocked while other drives ended (longest 0.9 ms; 7.4 ms under AddressSanitizer); implementation's methods: 173 panic plans, 855 panics caught, 186 re-entries, 32 blocks; Drop: 234 panics, 19,008 re-entries, 19 blocks; Map's bytes: 85 Maps of 256 succeeded on lying lengths, 88 panics, 135 re-entries, 5 blocks |

Controls (the same job): **negative**: the check run on 90b14f72a's own output (`once-check`) fails it, naming 19 functions
of the whole set and GetData of the slice (an independent regular expression over the same files also counts 19; the
check's 20 is taken to include the slice's GetData, INFERRED). **Planted**: a copy of the generator with the length
asked twice again and the check's failure ignored: the hostile set's lengths test under AddressSanitizer gets a report in
both sets (`SEGV on unknown address 0x000000000001`: the buffer of the first answer, 0, written for the second, 8).

Costs at 354e36e6d (MEASURED, job gac4/20261126-142925-2858484, the harness's measure test on the tip's slice build,
pinned to cores 92 and 93, load 64 to 73; ns, serving glue's added / pass-through, core 93): Draw 24.9 / 49.3, AddRef +
Release 42.7 / 55.4, QueryInterface + Release 81.1 / 96.1, GetDevice + Release 59.0 / 82.4, Map + Unmap 411.2 / 662.3,
CreateBuffer + Release 240.2 / 343.1. No code on these paths changed since ab721bebf but 330c3352b's value checks (READ:
the repair touches only methods with the caller's bytes); the load was twice that run's (INFERRED: the spread).

## AC3, continued: fuzzing every method (2026-11-26, commit b34d648bf)

The hostile set's driver also fuzzes: each method the calling glue carries is a function of random arguments (values with
edges, structures decoded from random bytes, slices of one length per call, objects held or none, the caller's bytes of
lengths that change), picked at random; every 4096 calls some objects are dropped and made again; each round of 50,000
calls runs under a lie picked at random (method, `Drop` or Map's bytes; panic or re-entry; every 7th to 506th call), one
round in three honest. MEASURED, job gac4/20261126-144005-3206473 at b34d648bf (log ac3r/fuzz-b34d648bf/): everything
but the hostile crates IDENTICAL to 354e36e6d's output (`diff -r`), clippy --all-targets 0 on both sets, the 10 lie tests
40 of 40 again; fuzzing under AddressSanitizer, 3 seeds x 10M calls each:

| Set, seed | Status, time | Calls made / skipped | Lies told | AddressSanitizer reports, hangs |
|---|---|---|---|---|
| slice 11, 12, 13 | 0, 47 s; 0, 44 s; 0, 45 s | 9,069,597 / 930,403; 9,070,152 / 929,848; 9,069,126 / 930,874 | 10,813; 6,378; 10,084 | 0, 0 each |
| whole set 11, 12, 13 | 0, 123 s; 0, 121 s; 0, 123 s | 9,967,980 / 32,020; 9,968,126 / 31,874; 9,967,980 / 32,020 | 17,047; 9,508; 14,962 | 0, 0 each |

0 never-asked calls in every run. The slice leaves 13 to 22 Maps the fuzzer never unmapped; each keeps its context and
resource alive for the memory lent (14 to 23 implementation objects alive after every object is dropped), as the system
would; the whole set's 0.

This fuzzes every table the calling glue reaches through the serving glue (calls well formed by the calling glue). Left
of AC3: a generated hostile C caller over every table (raw pointers and counts of C's choosing), the 168 methods not
glued, the generated conformance suite (the driver is its start: it calls every glued method), the linter per rule, and
tests of the effects judgments.

## AC3, continued (2026-11-26, tip c285c1292; logs in wcontainers-scratch/graphics-api-crates/ac3v/)

Every bar below is MEASURED at c285c1292 (x86-64 job gac5/20261126-165529-1057658, log ac3v/tip-c285c1292/final.log)
unless it names another commit. His main (ac696ea19) was already in the branch (`git rev-list --count HEAD..FETCH_HEAD` 0).

- **Asked once, by value** (src/trust.rs `Model`, `Ledger`): every call the generator emits of a method of the other
  party is made through the model of the function being emitted, which names the value by where it came from (the
  method's parameter by its place in the metadata, the `Mapped` the implementation's Map returned, the implementation's
  object of the entry), never by its name in the text. Each entry of `OTHER_PARTY` now carries a limit per method
  (`Once`, `Never`, `Free`); ByteSink and ByteSource of the caller were added to the list (ForeignLent's read_to and
  write_from). The text scan stays as a cross-check: the calls of `VolatileBytes` it finds must equal the asks the models
  recorded plus the templates' own (lent.rs, declared in `TEMPLATE_ASKS` and tested against the template text).
  Whole set: 1,423 functions modeled, 0 failures; text 43 calls (2 template) equal to the models' 41. Controls (job
  gac5/20261126-1535, at 3ccf0fd5a, ac3v/): A, the length asked again through another name, through the model: the
  generator fails, ASKED TWICE 1 (slice) and 19 (whole set); B, the length's call emitted around the model, its text
  unchanged: the generator fails, NOT MODELED. The generated crates at 3ccf0fd5a were byte-identical to 6f40f3c9d's.
- **(1) The hostile C caller** (src/glue_c_caller.rs, `crates/direct3d-11/implementation/direct/c-caller`): C written
  from the tables calls every entry of every table and every export: `this` null, released, of another kind or no
  object; counts 0, small, at and past the glue's limit (then with one element's memory), all ones; every buffer
  malloc'd at exactly the size the count gives; random structures and interface ids; out-pointers null or garbage; the
  objects handed out kept per interface, released at random, all given back each round; a lie of the hostile
  implementation per round. 3 seeds x 10M calls x each set under AddressSanitizer: 6 of 6 status 0, 0 reports, 0
  implementation objects left, 0 refusing entries answering other than E_NOTIMPL, 0 failed QueryInterface leaving a
  pointer (whole set: 543 entries and exports, about 4.2M successes and 1.1M refusals per seed). Control: the serving
  glue's element limit removed: AddressSanitizer heap-buffer-overflow (READ of 527,320 bytes over an 8-byte region).
- **(2) The methods not glued**: a GUID by value is now glued (`GuidAbi`, 29 methods): GLUED 504 of 643. The 139 left are
  each refused by their form, with the reason in the report (NOT GLUED BY FORM): 39 by an entry that reads and writes
  nothing and answers E_NOTIMPL (no null slot for C to call), 100 by a null entry (a structure by value in the call, or
  an out-parameter an entry would leave unwritten): 39 arrays out, 16 structures holding a pointer, 14 counts passed by
  pointer, 11 bytes in, 9 PSTR and 5 PWSTR by value, 9 `void` returns of other forms, 7 void pointers, 6 arrays whose
  count is no parameter, the rest one or two each. The calling glue answers E_NOTIMPL for all 139.
  CORRECTED 2026-11-26 (the check of 49bc737b7): three statements of this item were false. The calling glue answered
  E_NOTIMPL only where the method's result is a `Result`; 44 methods returning a plain value had an `unimplemented!`
  body, so safe code calling them panicked. The 100 methods "refused by a null entry" were not refused: C calling one
  jumps to address 0. RSSetViewports, RSSetScissorRects and DiscardView1 take const arrays with a count, not structures by
  value. The report line and this item were written by emit.rs from a list, not from what glue_call.rs wrote. Repaired
  below (AC3, repaired after the check of 49bc737b7).
- **(3) The conformance suite** (`crates/<api>/conformance/suite.rs`, from the driver): generic over the top trait, it
  makes an object of every kind it can, calls every method and export, and checks each answer (a failure negative, an
  object equal to its clone); run by the recorder and the hostile implementation through `tests!` (conformance_run.rs).
  It found the recorder panicking on 87 methods returning a structure or a list; the recorder now makes those (zero bytes
  decoded, a default, empty lists). Results: Direct3D 11 65 kinds, 843 of 847 calls made, 0 panics; DXGI 16 kinds,
  211 of 211; d3dcommon 0 of 4 (its kinds are made only by Direct3D 11's methods); 10 of 10 runs status 0. Not yet run
  by the direct crates (through both glues): the calling glue still panics for a method not glued that returns a plain
  value.
- **(4) The effects judgments, tested** (harness/hostile/effects.rs): for each of the 23 groups, every method of the
  group driven through both glues after a buffer was mapped, then: no call panicked back; no method a never-called row
  names reached the implementation; a method a row answers reached it only with the answer's arguments; the memory lent
  intact unless the group's own Unmap ended it, then reaching nothing; the buffer's description unchanged. Slice and
  whole set, default and AddressSanitizer: 4 of 4 status 0, 23 groups each, 0 reports. Control: the calling glue planted
  to pass SetMultithreadProtected through: the test fails ("reached ID3D11Multithread::SetMultithreadProtected with other
  than true", 3).
- **(5) Linter** (whole set with its glue, per rule, b09f41a23 then c285c1292): magic_literal 8,612 then 6,916 (the
  fuzzers picked from a table, the slices' length and the interface filters named, the never-asked traits named);
  public_outside_interface 6,259 then 5,392 (src/visibility.rs: an item of a direct crate no other crate names becomes
  `pub(crate)`, 366 narrowed); conformance_untested_method 643 (the suite's calls are inside closures given to
  `attempt`; not yet worked); implementation_folder 288; conformance_not_run 223. The rest of the causes, read from the
  findings: magic_literal is the byte forms' offsets and sizes (data.rs 2,061, serve.rs 1,611, call.rs 560: `get_u32(bytes,
  16)`, `[0_u8; 24]`) and the recorded call texts of the recorder and hostile crates; the fix that removes the first is
  a `#[repr(C)]` mirror of each record with `offset_of!` and `size_of`, not yet written. public_outside_interface is the
  direct crates' tables, entries and helpers that other direct crates name, and data.rs counted again in each crate that
  includes it.
- Kept from before, at c285c1292: the trait layer and both glue sets written twice IDENTICAL; check, clippy and clippy
  --all-targets 0; the whole set's 47 hostile tests and probes, default and AddressSanitizer, 47 of 47; slice harness 75 of
  75; the hostile set's lie tests 44 of 44; its fuzzing 3 seeds x 10M per set under AddressSanitizer 6 of 6, 0 reports;
  the once-check's negative control and planted defect fail as before.
- **Costs** (job gac5/20261126-160631, b09f41a23 against the spike's own binary, one after the other on core 92, then 93;
  load 13.4, 8.4, 8.0, 6.0), ns, serving glue's added / pass-through:

| Operation | Ours (core 92) | Spike (core 92) | Within a tenth |
|---|---|---|---|
| Draw | 20.9 / 41.7 | 23.8 / 38.1 | added 12% faster; pass-through 9.4% slower: within |
| AddRef + Release | 24.8 / 37.4 | 24.7 / 37.5 | both within |
| QueryInterface + Release | 68.1 / 81.0 | 80.9 / 93.8 | both 14 to 16% faster |
| GetDevice + Release | 53.4 / 69.3 | 60.2 / 73.2 | added 11% faster; pass-through within |
| Map + Unmap | 269.9 / 453.4 | 215.0 / 456.5 | pass-through within; added 26% slower (the shadow copy and its write-back) |
| CreateBuffer + Release | 245.0 / 318.7 | 336.9 / 397.8 | both 20 to 27% faster |

Left of AC3: the conformance run through both glues (the calling glue's not-glued plain returns), the 100 methods
refused by a null entry (glue for arrays out and counts by pointer), the byte forms' mirrors for magic_literal, the
direct crates' cross-crate surface for public_outside_interface (a layout question when no narrowing is left), and
conformance_untested_method.

## AC3, repaired after the check of 49bc737b7 (2026-11-26; logs in wcontainers-scratch/graphics-api-crates/ac3w/)

His main (ac696ea19) was already in the branch: `git fetch` of his checkout's main, `git rev-list --count HEAD..FETCH_HEAD`
0 (MEASURED). Every bar below is MEASURED at de137e0e1 (pushed), x86-64 job gac6/20261126-191343-3657339, log
ac3w/tip-de137e0e1/final.log, reports and the whole set's linter output beside it.

- **No null entry, no panicking body.** A method no form of the glue carries gets an entry (src/glue.rs `refusal`) with
  C's types by calling convention only (a pointer, an interface pointer, a GUID by value, a value; a structure by value
  only when it is all integers and 1, 2, 4 or 8 bytes, which both x64 conventions pass as one integer; a PSTR or PWSTR as
  a pointer; a callback as an address). It reads nothing, calls nothing, writes COM's null through each out-pointer to one
  object or pointer and zeros through each value out of a fixed size (nothing through arrays and bytes: no count of them
  is trusted), and answers E_NOTIMPL, or for a method with no result nothing. A parameter whose convention it does not
  know fails the generator: no null entry can be written. The calling glue's refused methods answer E_NOTIMPL, no result,
  no object, or a value that holds nothing (zero bytes decoded for a structure); a type of which none can be written fails
  the generator, and the recorder's two `unimplemented!` fallbacks are gone the same way. The `Absent` type is removed.
- **The check** (src/complete.rs, run on every set written): every file of every direct and recorder crate is read; a
  table field `None`, an `Absent` field, or `unimplemented!`, `todo!`, `panic!`, `unreachable!` fails the generator; 3 unit
  tests. Result: "COMPLETE: 31 files of the direct and recorder crates; null entries 0, panicking bodies 0". Controls (same
  job, plant4.sh): a generator copy writing `None` for the refused entries: status 1, "COMPLETE FAILED: 189 findings"; one
  writing `unimplemented!` for the refused calling bodies: status 1, 104 findings; one writing 1 instead of 0 through the
  refusals' value outs: generator status 0, the C caller's honest test status 101 (its new assertion).
- **The report is printed from what each glue wrote** (src/glue.rs `Outcome`, recorded when each entry's and each calling
  method's text is emitted): per method not glued, its reason, what the serving entry does and what the calling method
  answers. Whole set: "GLUED 575", "NOT GLUED 68", "NULL ENTRIES 0", "CALLING GLUE METHODS THAT DO NOT CALL THE SYSTEM 89"
  (refused by kinds, by form, or pinned). The 68 by entry: 28 answer E_NOTIMPL writing nothing, 11 no result writing
  nothing, 4 zero, 1 null; 24 write COM's null or zeros through their out-pointers (ac3w/tip-de137e0e1/report-direct3d-11-glue.txt).
- **Glued, of the forms that are plain to carry** (both glues, the C caller, the recorder and the hostile set): const arrays
  of structures with a count (RSSetViewports, RSSetScissorRects, DiscardView1, ClearView's rectangles: structures copied
  as C lays them out, passed in aligned words); fixed arrays by `CountConst` in and out (ClearRenderTargetView,
  ClearUnorderedAccessViewFloat and Uint, OMSetBlendState, ClearView, OMGetBlendState); bytes in with a count (the seven
  Create*Shader, SetPrivateData of the four interfaces, the video extensions; copied once, as decided); arrays out with a
  count by value, of values and of objects (the 30 *Get{ConstantBuffers,Samplers,ShaderResources,UnorderedAccessViews}
  and *GetConstantBuffers1, SOGetTargets, OMGetRenderTargets, OMGetRenderTargetsAndUnorderedAccessViews, IAGetVertexBuffers,
  VideoProcessorGetStreamPalette, ReclaimResources, ReclaimResources1, CreateSurface); arrays of values out counted by
  pointer when they are the method's one out (RSGetViewports, RSGetScissorRects, GetDisplayModeList, GetDisplayModeList1:
  the serving glue reads the room once, writes no more than it and the number written; the calling glue asks how many,
  then passes that much room, none past the glue's limit); a GUID out (4). GLUED 504 -> 575. Not glued, each with its
  reason in the report: objects out counted by pointer (the six *GetShader), bytes out counted by pointer (GetPrivateData
  of three interfaces, the info queues' messages and filters: 9), PSTR/PWSTR by value (14), structures holding a pointer
  (16 + 6 arrays), void pointers (7), objects out of several kinds (2), and 8 single forms.
- **A leak found and fixed** (MEASURED): at 6fc6df5a2 the whole set's C caller fuzzing ended with 1 and 5 implementation
  objects alive (seeds 11, 13): an array out's objects were served one by one while the implementation's extra objects
  (more than the room) were dropped after, and a `Drop` that panicked lost the references already served. The serving
  glue now drops every object no array out hands over before it serves any (de72bb484); seeds 11 and 13 then 0 alive.
  The fuzzing test now fails when an object outlives the run.
- **The hostile C caller over every entry** (643 entries and exports, 0 absent: the 100 formerly null entries are called,
  each refusing one with out-buffers of exactly the size it writes, checked to be null or zeros after the call). Honest run
  default and AddressSanitizer: status 0, refused 6,198 calls, refused_wrong 0. Fuzzing 3 seeds x 10M per set under
  AddressSanitizer: 6 of 6 status 0, 0 reports, implementation objects alive 0 (before 0) in each; whole set about 5.3M
  successes per seed. Control: the serving glue's element limit removed at 4 places: AddressSanitizer report, status 101.
- **The conformance suite through both glues** (`conformance_through_glues`, the suite run on `served(implementation)`:
  the calling glue over the serving glue's exports): Direct3D 11 65 kinds, 843 of 847 calls made, 0 panics; DXGI 16
  kinds, 211 of 211; on the recorder and the hostile implementation, default and AddressSanitizer: 8 of 8 runs status 0, 0
  reports (d3dcommon has no exports, so no `served`: its kinds are made by Direct3D 11's methods). The direct runs as
  before: 10 of 10 default, 10 of 10 AddressSanitizer.
- **conformance_untested_method 643 -> 65** (whole set). Measured first (two experiments on a copy of the tip's output):
  moving every call of DXGI's suite out of its closure into a named function left DXGI's 197 findings at 197; writing the
  calls as `Trait::method(this, ..)` instead of `<<A as Top>::Kind as Trait>::method(this, ..)` took them to 27. So the
  qualified self type hid the calls, not the closures. The generator now writes every driven and suite call by the trait's
  path (de137e0e1). The 65 left are methods the suite cannot make an argument for (structures holding pointers, the
  methods not glued).
- Kept at de137e0e1: the three sets written twice IDENTICAL; check, clippy -D warnings and clippy --all-targets status 0
  on both glue sets; generator 17 tests (3 new); whole set's 47 hostile tests and probes, default and AddressSanitizer;
  slice harness 75 of 75; the lie tests both sets default and AddressSanitizer; the hostile set's fuzzing 3 seeds x 10M per
  set under AddressSanitizer 6 of 6 status 0, 0 reports, 0 hangs; the effects tests 4 of 4 (23 groups each) and their
  control (status 101); the once-check's negative control and planted defect fail as before.
- **Linter, whole set with its glue, per rule** (ac3w/tip-de137e0e1/linter-direct3d-11-glue.txt; the copies of
  volatile-bytes, byte-sink and byte-source counted apart, 56): 14,719 findings; magic_literal 7,869 (was 6,916: the 71
  methods glued and the refusals' sizes), public_outside_interface 5,486, implementation_folder 288, conformance_not_run
  227, literal_restates_constant 170, summary_not_one_line 139, summary_too_long 93, number_constant_without_link 86,
  conformance_untested_method 65, constant_defined_twice 63, trait_not_implemented 58, the rest below 33 each. Not done in
  this run: the `#[repr(C)]` mirror with `offset_of!` and `size_of` that removes the byte forms' offsets and sizes
  (data.rs, serve.rs, call.rs), the main cause of magic_literal.
- Marks (the generator's count): whole set 2,613 -> 3,032, slice 356 -> 475, COM 35 (the new glue and the refusals'
  writes, each one operation with its reason matched, 0 unmatched). Lines naming `unsafe` (MEASURED, tip.sh's count),
  c285c1292 -> de137e0e1: direct3d-11 direct 5,459 -> 6,656, DXGI direct 2,353 -> 2,534, d3dcommon direct 57 -> 63, COM
  direct 91 -> 90; 0 outside the direct crates. Costs were not measured again (no change on the measured paths).

Left of AC3: the linter's magic_literal by the repr(C) mirror; public_outside_interface (the direct crates' cross-crate
surface, a layout question once no narrowing is left); the 68 methods not glued (objects out counted by pointer, bytes
out counted by pointer, strings by value, structures holding pointers).

## The merge of graphics-layout (asked for AC4): stopped

2026-11-26: the nine interface files are written for the orchestrator, one diff each with three lines of context, in
wcontainers-scratch/graphics-api-crates/graphics-layout-interface-changes/. MEASURED (`git diff` of the merge base
e9fff41e3, graphics-layout 97e1daafe and his main ac696ea19): six of the nine (icmp-sockets, raw-sockets, system,
tcp-sockets, udp-sockets, volatile-bytes) are already on his main exactly as graphics-layout has them, so the merge
changes nothing in them; three are new interface files his main does not have: shader-converters (+140), win32 (+128),
test-window (+26).

`git merge-tree` of the track against origin's graphics-layout (97e1daafe): 97 conflicting files (shaders 25,
wcontainers-shaders 13, shader-converters 13, opengl 8, nir 4, direct3d-12 4, ...). Since the merge base e9fff41e3,
graphics-layout changes 10 of the owner's interface files (MEASURED `git diff --stat`): shader-converters (+140),
win32 (+128), test-window (+26), system (+8), tcp-sockets (+8), icmp-sockets and raw-sockets (8 changed lines each),
udp-sockets (+4), volatile-bytes (+6). A normal merge carries those into his interface files, which this run may not
change; the merge waits on the owner taking (or refusing) those interface changes first. No generated crate is in
crates/ yet, so no file has both sides. Hand-written pieces the generated crates must absorb (lines naming unsafe on
graphics-layout): directx 1,056 (src/com/device.rs 84, context.rs 45, mod.rs 35; examples own_* ; tests/com.rs 68),
direct3d-12 498 (examples/directx 316, examples/check 169), graphics-test 321 (src/d3d11, src/d3d12), graphics-test-
forward 190 (census.rs 151, clock.rs 13), opengl 357, gpu-webgl 164, nir 115.

## Scope added 2026-11-25 (the orchestrator's ruling)

This track owns every outside graphics API crate, including the OpenGL ES and EGL calls of the WebGL native route
(glow over ANGLE, crates/gpu-webgl) and the Mesa compiler calls of the shader path (crates/nir's bridge). They are
milestones AC10 (OpenGL ES and EGL) and AC11 (the Mesa compiler), after Direct3D 12. The implementations track uses
these traits and writes no glue.
