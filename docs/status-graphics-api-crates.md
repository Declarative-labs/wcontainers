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
  Create*Shader, SetPrivateData of the four interfaces, the video context's DecoderBeginFrame, DecoderBeginFrame1,
  DecryptionBlt, Configure- and QueryAuthenticatedChannel and the HDR metadata setters; copied once, as decided); arrays out with a
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
  kinds, 211 of 211; on the recorder and the hostile implementation of the whole set and the slice's hostile one: 5 of 5
  runs status 0 by default and 5 of 5 under AddressSanitizer, 0 reports (d3dcommon has no exports, so no `served`: its
  kinds are made by Direct3D 11's methods). The runs on the implementations directly: 9 of 9 default, 9 of 9
  AddressSanitizer.
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

## AC3, repaired after the check of a197b550d: every case reached, counted at run time (2026-11-26)

The check did not hold on "the hostile C caller calls all 643 entries": "absent 0" counted the cases written; under
fuzzing 5 cases were never reached (ID3DBlob's two, ID3DDestructionNotifier's two, ID3D11Device5::CreateFence) and no test
failed. Every bar below is MEASURED at a0a9206cb (pushed), x86-64 job gac7/20261126-2144xx, log
wcontainers-scratch/graphics-api-crates/ac3x/final-a0a9206cb.log, unless it names another run.

- **Reach counted at run time** (src/glue_c_caller.rs, harness/c-caller/c_caller.rs): each case counts its calls made with
  `this` the object whose table it is in, each export its calls; the counts come back to Rust (`Counted::reached`). The
  honest test and the fuzzing test fail naming every case or export never reached, fail if a case listed unreachable is
  reached, and fail unless the cases never reached are exactly the listed ones. The "absent" counter is gone.
- **Sweeps**: each run begins, per thread, by sweeps until two in a row hold no new interface: every held object is asked
  for every interface by QueryInterface; each export and case that takes an interface id is called once per interface not
  yet held; every case is called on an object held as its interface (one that makes objects up to 48 times until it hands
  one out). Structures in are also passed as words of small values, which a description's checks let through.
- **What the caller can hold, worked out by the generator** (`reach`): from the exports, the kinds each glued entry hands out
  (objects out, the API's kinds for an interface by id, the bound kind for a bound object) and every interface each kind
  answers (its chain and its faces), to a fixed point. The interfaces of the cases left must equal src/api.rs
  `UNREACHABLE_IN_WHOLE` / `UNREACHABLE_IN_SLICE`, each with its reason, or the generator fails.
- **The destruction notifier made reachable**: Direct3D 11's buffers and 1D, 2D and 3D textures answer
  ID3DDestructionNotifier by QueryInterface (a face, src/api.rs). Which Direct3D 11 objects answer it on Windows is
  INFERRED from Microsoft's description of the interface and not yet seen there (AC9). A program-visible change: our
  resources now answer that id. CreateFence was reachable; the sweeps reach it.
- **Unreachable, by name** (whole set, 2): ID3DBlob::GetBufferPointer and GetBufferSize: a program gets a blob from the shader
  compiler (d3dcompiler_47.dll) or Direct3D 12's serializers, other APIs left out; no method or export of Direct3D 11, DXGI
  or d3dcommon hands one out and no object of theirs answers it. Slice (7): the blob's two, the notifier's two (the
  spike's faces), IDXGIAdapter's three (taken only into D3D11CreateDevice).

| Bar at a0a9206cb | Result |
|---|---|
| C caller honest, whole set, default and ASan | status 0, 0; "C CALLER REACHED: cases 633 of 635, exports 8 of 8; never reached 2 = unreachable listed 2" |
| C caller honest, slice, default and ASan | status 0, 0; cases 185 of 192, exports 2 of 2, never reached 7 = listed 7 |
| C caller fuzzing 3 seeds (11, 12, 13) x 10M x set, ASan | 6 of 6 status 0, 0 reports, implementation objects alive 0; every run 633 of 635 (slice 185 of 192) |
| Control a: CreateFence's case calls nothing | honest status 101 and fuzz 2M status 101, "NEVER REACHED 1: ID3D11Device5::CreateFence" |
| Control b: the blob's line removed from src/api.rs | generator status 1, "C CALLER REACH: ... are {"ID3DBlob"}; src/api.rs lists {}" |
| Control c: the sweeps removed | honest status 101, cases 424 of 635, NEVER REACHED 209 |

- **A defect the sweeps found, fixed** (46cc80ed6): at ebf0983e9 the whole set's C caller fuzzing seed 13 aborted (status
  134, "thread caused non-unwinding panic", job gac7c, gdb: `Vec::truncate` in `device_context_so_get_targets` dropping the
  implementation's extra objects, two of whose `Drop`s panicked under a lie: a panic during cleanup aborts). The serving
  glue now drops each object an array out does not hand over on its own, caught. Seeds 13 and 14 then status 0, 0 reports.

**The rest of AC3 in this run:**
- **repr(C) mirror** (src/glue.rs `mirror_type`): each record the byte forms carry has a `#[repr(C)]` mirror (packing as
  `packed(N)`, a union as a `union`, a GUID as four 32-bit words), and every offset and size is `offset_of!` / `size_of` of
  it. The mirror-check build (MIRROR_CHECK set, never the kept output) writes a test per direct crate comparing every
  mirror offset and size with the generator's own numbers, which check-layout.sh compares with MinGW's headers: whole set
  886 assertions (Direct3D 11 684, DXGI 202), slice 163, both status 0; control (fields written in reverse order) status
  101, "left: 28, right: 0". No record of the set has a packing (0 `packed(` in the output), so packing is not exercised.
  magic_literal (whole set) 7,869 -> 3,730.
- **Text in glued** (11 methods: CreateClassInstance, GetClassInstance, OpenSharedResourceByName, BeginEventInt,
  SetMarkerInt, BeginEvent, SetMarker, both info queues' AddMessage and AddApplicationMessage): the serving glue copies the
  text once up to its first zero unit and refuses one with none within 65,536 units (`read_text`, COM's direct crate, two
  marks); the calling glue passes a `&CStr` as it is and a `&[u16]` copied with a zero unit. GLUED 575 -> 586, NOT GLUED 68
  -> 57. Left, each with its reason in the report: 6 objects out counted by pointer (*GetShader), 7 bytes or filters out
  counted by pointer (GetPrivateData x2, the info queue's messages and filters), 3 text out whose room the metadata links to
  no parameter (GetInstanceName, GetTypeName, CheckCounter; the trait layer maps them as text in, which is wrong and is to
  be changed with them), 13 structures holding pointers, 7 void pointers (refused by kinds, by design), 1 object out of
  several kinds (CreateQuery), 1 array out beside other outs (GetResourceTiling), the rest single forms.
- **public_outside_interface narrowed** (src/visibility.rs): an item stays public only when another crate names it through
  the crate's path or a `use` of it (whole paths, so `Tables::<V>::RESOURCE1` keeps the constant), a field only with its
  structure, a method when any crate mentions it; every interface's table impl allows dead code. 5,486 -> 3,821 (whole
  set; job gac7b q5 before the run): Direct3D 11's direct abi.rs 1,129, serve.rs 391, call.rs 200; DXGI's abi.rs 620,
  serve.rs 167; the hostile and recorder crates' items (their tests' surface); COM's direct crate.
- **Linter, whole set** (8,925 findings, was 14,719): public_outside_interface 3,821, magic_literal 3,730,
  implementation_folder 300, conformance_not_run 227, literal_restates_constant 171, the rest below 133 each.
- Kept at a0a9206cb: the three sets written twice IDENTICAL; check, clippy and clippy --all-targets status 0; generator 17
  tests; whole set's 47 hostile tests and probes default and ASan; slice harness 75 of 75; the lie tests both sets default
  and ASan; hostile-set fuzzing 3 seeds x 10M per set under ASan 6 of 6 status 0, 0 reports, 0 hangs; conformance on and
  through both glues default and ASan, every run status 0 (Direct3D 11 65 kinds, 855 calls, 851 made, 0 panics); effects 4
  of 4, 23 groups; every earlier control fails as before (the refusal-write control rerun with the new pattern: honest
  status 101, job gac7m 21:5x).
- **Costs at low load** (job gac7m/20261126-2150xx, a0a9206cb, ours and the spike's binaries one after the other on core
  92, load 4.8 then 4.3; core 93 the same to 0.1 ns), ns, serving glue's added / pass-through:

| Operation | Ours | Spike | Within a tenth |
|---|---|---|---|
| Draw | 21.0 / 41.8 | 23.8 / 37.7 | added 12% faster; pass-through 10.9% slower |
| AddRef + Release | 24.8 / 37.4 | 24.7 / 37.4 | both within |
| QueryInterface + Release | 68.1 / 80.8 | 80.8 / 93.6 | both 14 to 16% faster |
| GetDevice + Release | 53.5 / 69.2 | 60.3 / 72.9 | added 11% faster; pass-through within |
| Map + Unmap | 271.5 / 456.9 | 215.2 / 456.3 | pass-through within; added 26% slower (the shadow copy and its write-back) |
| CreateBuffer + Release | 240.2 / 313.5 | 335.8 / 397.1 | both 21 to 28% faster |

  run-harness.sh had no measure step though its usage named one; added (a0a9206cb).

**The layout question for the orchestrator (one, for the owner):** may an API's `implementation/direct` crate export to
the direct crates of the APIs built on it, and to the test crates of its set, the surface they call (its C tables, its
serving entries and calling types, and COM's bookkeeping in `component-object-model/implementation/direct`), or must that
surface live in a crate of a kind the layout names (and which)? Smallest example (DXGI's table, which a Direct3D 11 buffer
answers IDXGIResource1 with):

```rust
// crates/directx-graphics-infrastructure/implementation/direct/abi.rs
#[repr(C)]
pub struct Resource1Table { pub unknown: UnknownTable, pub object_set_private_data: unsafe extern "system" fn(Raw, GuidAbi, u32, *const u8) -> i32, /* .. */ }
// crates/direct3d-11/implementation/direct/serve.rs
return Some(unsafe { Table::new(directx_graphics_infrastructure_direct::Tables::<V>::RESOURCE1) });
```

Turns on: public_outside_interface's 3,821 findings, all of this kind (no narrowing is left that keeps the set building).
Build around: the findings stay reported and counted.

Next (AC3): the 57 methods not glued (objects and bytes out counted by pointer first); then AC10 (OpenGL ES and EGL), which
waits on nothing for its trait layer.

## AC10, OpenGL ES and EGL: the trait layer and the direct glue (2026-11-27)

**GI7 can start from commit 9429dd43f** (pushed on `graphics-api-crates`): `crates/opengl-es` (trait `OpenGlEs`, and
`OnEgl<E>` that makes a context over an EGL context), `crates/egl` (trait `Egl`), each with `implementation/direct`,
and `crates/opengl-es/conformance`. His main (ac696ea19) was already in the branch (`git fetch` of his checkout, `git
rev-list --count HEAD..FETCH_HEAD` 0, MEASURED).

The generator's new front end (`tools/api-crate-generator/src/khronos/`, command `khronos`) reads Khronos's gl.xml
(OpenGL-Registry 6af574a1) and egl.xml (EGL-Registry 8e56e86e), Apache-2.0, and ANGLE's gl_angle_ext.xml (6df81ae1,
BSD-3-Clause), which declares glPolygonModeANGLE and glProvokingVertexANGLE (gl.xml does not); `fetch-registries.sh`
checks each sha256. The commands carried are those of NEEDED-FROM-API-CRATES.md A1 to A4 (src/khronos/sets.rs). New
tool dependency: roxmltree 0.20.0 (MIT or Apache-2.0), the XML reader; never in the product.

**Default deny, by the registry's own attributes.** Every parameter gets a kind from its C type and gl.xml's `len`
(`n`, `count*4`, `bufSize`, `1`) or an exception line of sets.rs, else the generator fails (it did, once, on
glGetUniformBlockIndex's `len="COMPSIZE()"` text, now mapped as a zero-ended text). Rows: number, bool, offset (i64
checked into isize), handle (GLsync and EGL's, as values of the glue's own types safe code cannot forge), slice in or
out with its count derived from the slice (a vector count must divide exactly), one value out (returned), zero-ended
text, texts with lengths, bytes. Exception lines (each READ in sets.rs with its reason): `Get*v` of a pname write
into 64 values of the glue's and copy out as many as the caller's slice holds, refusing the three variable lists
(COMPRESSED_TEXTURE_FORMATS, PROGRAM_BINARY_FORMATS, SHADER_BINARY_FORMATS); glClearBuffer*v read 4 of the glue's;
pixels in and out (`PixelUnpack`, `PixelPack`, `CompressedPixelUnpack`) are the caller's bytes only with no pixel
buffer bound and at least as long as the pixel store state makes the transfer (alignment, row length, skipped
rows, pixels and images, OpenGL ES 3.2 §8.4.4.1), or an offset only with one bound; a format and type pair the glue
does not know is refused; vertex attribute pointers and draw indices are offsets, refused with no buffer bound;
every draw is refused while an enabled vertex attribute has no buffer (deleting a bound buffer leaves the attribute
reading client memory at its offset); glMapBufferRange and glUnmapBuffer are offered only as `map_buffer_range_read`
(map for reading, copy, unmap: NEEDED A3); eglCreateContext always gets robust buffer access (FORCED: OpenGL ES leaves
an out-of-range read undefined otherwise); EGL attribute lists end at their first key EGL_NONE and a key with no
value is refused. Extension commands answer `None` unless the context names the extension and gives the address. A
refusal records the error the next `get_error` returns first (GL's INVALID_VALUE, INVALID_ENUM, INVALID_OPERATION,
CONTEXT_LOST; EGL's BAD_*). Not offered: eglCreateWindowSurface (the window loan, his question in
status-graphics-implementations.md; the needs file puts it last), eglTerminate (the display is shared),
eglGetProcAddress and eglGetCurrentContext (the glue's own).

**For GI7, the glow calls in trait terms** (READ, interface.rs): `create_*` are `gen_*(&mut [u32])` (or
`create_program`/`create_shader`); `delete_*` take a slice; `get_parameter_*` are `get_integerv`/`get_floatv`/
`get_booleanv`/`get_integer64v` into a slice; `get_parameter_string` is `get_string`; `supported_extensions` is
`get_integerv(NUM_EXTENSIONS)` and `get_stringi`; `get_*_info_log` write into `&mut [u8]` and return the length;
`get_*_status` are `get_shaderiv`/`get_programiv`/`get_synciv`; `buffer_data_size` is `buffer_data(target, size,
None, usage)`; `get_buffer_sub_data` is `map_buffer_range_read`; `query_counter`, `get_query_parameter_u64` and the
ten A2 calls carry their extension suffix (`query_counter_ext`, `clip_control_ext`, `enablei_oes`, ...); a uniform
location is the `i32` the GL gives; values are `constants::*` without the `GL_`/`EGL_` prefix.

MEASURED at 9429dd43f (x86-64 job gac-k, `.gack/run.sh`, log wcontainers-scratch/graphics-api-crates/ac10/tip-9429dd43f.log):

| Bar | Result |
|---|---|
| Generator: build (every warning shown), tests | status 0, 0 warnings; 17 tests pass |
| Written twice | IDENTICAL; and equal to the crates committed ("SAME AS WRITTEN") |
| Carried | EGL 15 methods + `load`, 176 values; OpenGL ES 154 methods + 1 composed, 1,094 values; every parameter mapped (0 unmapped) |
| `cargo check --all-targets`, the five packages; wasm32 check of the four library packages | 0; 0 |
| `cargo clippy --all-targets -- -D warnings` | 0 |
| The suite on `direct` over Mesa 26.2.4's EGL (nixpkgs, libglvnd, llvmpipe, `EGL_PLATFORM=surfaceless`) | 4 of 4 pass: draws_and_reads_back, refuses_what_would_reach_memory, robust_access_is_forced, every_method_is_called (8 rounds of every method with edge arguments) |
| The same under AddressSanitizer (24 `__asan_report` symbols in the binary) | 4 of 4 pass, 0 reports |
| Control: the pixel length check removed | status 101, "pixels one byte short", left 0 right 1281 |
| Control: the draw check removed | status 101, "a draw whose enabled attribute lost its buffer", left 0 right 1282 |
| Lines naming `unsafe` | egl 0, egl direct 95, opengl-es 0, opengl-es direct 792, conformance 0; marks 357 (39 + 318) |
| Linter, the two crates | 2,794: summary_without_full_stop 1,270 (the values' link lines), magic_literal 805, public_outside_interface 199 (egl direct's `Binding`, which the OpenGL ES glue uses: the layout question below), function_summary_not_third_person 191, name_unanswered 185 and summary_unanswered 35 (the model rule sets, no key on the machine), the rest below 25 each |

Not done in this run and left of AC10: the suite over ANGLE on Linux (no ANGLE build for Linux exists on the
machines; the suite runs on Mesa, which the needs file names for the GLES list); moving gpu-webgl onto the trait
(GI7's, and AC4's merge); the per-draw check's cost, not measured (two glGetVertexAttribiv per attribute per draw:
INFERRED a few microseconds; a cache per vertex array is the way down if it matters); a hostile caller of the C
side is not needed (no serving glue: these crates only call the library).

**Rerun at 1571b4566** (docs only changed in the output: each value's line ends in a full stop, each method's says
what it calls; job gac-k, log ac10/tip-1571b4566.log): every bar above the same (IDENTICAL, SAME AS WRITTEN, check,
wasm32 check, clippy, 4 of 4 tests default and AddressSanitizer, both controls status 101, unsafe lines and marks
unchanged); linter 2,794 -> 1,519 (summary_without_full_stop 1,270 -> 0, function_summary_not_third_person 191 -> 22;
summary_unanswered 35 -> 204 is the model rule set, which has no key on the machine). roxmltree 0.20.0: 1 line
naming `unsafe`, `#![forbid(unsafe_code)]` (MEASURED, same job).

## AC10 after the check of 1571b4566: every check of the glue removed in turn, the fuzzing, the per-draw cost (2026-11-27)

The check found two weak tests: the colour clear given one value passed `&[0.0]` (a promoted static, which
AddressSanitizer does not guard), and the fence tests could not tell the glue's refusal from Mesa's. Repaired as a
class: every hostile test now counts the refusals the implementation made itself (`OpenGlEsContext::refusals`,
`Library::refusals` for the calling thread, handed to the suite as `Counts`), and every buffer it gives is a heap
allocation of exactly the length named or a part of a longer one whose rest the call must not reach (a clear given
one channel followed by 0.75s must clear with 0.25 and zeros; a get into an empty part must leave the values after it;
a list of names followed by a value the library refuses). The fence test keeps a live fence numbered as the other
context's. Eleven tests (draws_and_reads_back, refuses_pixels_past_what_was_given, refuses_vertex_reads_without_a_buffer,
counts_are_the_slices, counts_reach_only_the_slices, rooms_keep_calls_inside, fences_are_the_contexts_own,
egl_handles_are_the_librarys_own, robust_access_is_forced, extensions_answer_only_when_named, every_method_is_called)
and two run only when asked (the fuzzing, the measurement).

Defects found by it and fixed in the generator: the end of a destroyed EGL handle was written twice (once for the
display parameter), so removing one left the other; a library answering a huge extension count kept the glue asking
for ever (now at most 4096, `MOST_EXTENSIONS`; the fuzzing's lying library answers up to 2^31).

`tools/api-crate-generator/plant-khronos.py` removes each check in turn (a per-site plant for every check a generated
method makes, a plant for each part of a shared helper) and runs the suite. MEASURED at 4ae8cb6e2 (x86-64 job gac-h,
log graphics-api-crates/ac10h/claim-4ae8cb6e2.log): **156 CAUGHT, 15 MISSED, 0 BROKEN, 202 LISTED.** The 15 not
caught, each with its reason:

| Plant | Why no test fails |
|---|---|
| extension-named, 10 methods (draw_buffers_indexed's 4, viewport_array's 2, clip_control, polygon_offset_clamp, the two timer queries' 3 less one) | Mesa names their extensions (MEASURED: GL_EXT_clip_control, GL_EXT_disjoint_timer_query, GL_OES_draw_buffers_indexed, GL_OES_viewport_array, GL_EXT_polygon_offset_clamp, GL_EXT_occlusion_query_boolean); the two ANGLE ones Mesa does not name are CAUGHT |
| extensions-bounded | Mesa answers a few hundred; shown by the fuzzing's lying library instead (a run without the bound does not end) |
| egl-config-of-this-display, egl-held-of-this-display | EGL gives one default display per library: no test holds two displays of one library |
| egl-handles-written, egl-handles-null | two checks of one fact (a slot past the count written is also null); the plant removing both is CAUGHT |

LISTED (202): the entry point missing (Mesa gives every one), an i64 not fitting isize (none on 64-bit), a count or a
text length past 2^31 (no test allocates it), the gate (a race), a library without
EGL_KHR_client_get_all_proc_addresses, and a destroyed handle that is not current (the check under the gate refuses
it; without it eglMakeCurrent refuses the destroyed handle and the glue refuses alike).

**Fuzzing (AC2's bar), MEASURED at 4ae8cb6e2, same log**: `tools/api-crate-generator/fuzz-khronos.sh`, the suite's
`fuzz` as a hostile caller (every OpenGL ES method on two contexts with live, deleted and the other context's
fences; every EGL method with this library's handles, another library's and destroyed ones; attribute lists with a
key and no value, no EGL_NONE, empty), under AddressSanitizer:

| Set | Seeds x calls | Reports | Refused by the glue itself (OpenGL ES, EGL) | Seconds each |
|---|---|---|---|---|
| hostile caller over Mesa 26.2.4 (llvmpipe) | 7919, 104729, 1299709 x 10M | 0, status 0 each | about 1.18M and 6.4k per run; the rest reached Mesa | 39 to 40 |
| hostile caller over the lying library | the same | 0, status 0 each | about 1.29M and 32k | 14 |

The lying library (`fakes/opengl-es/fake_egl.c`, generated from the classification and the template
templates/khronos/fake-egl.c) reads every byte and writes every value each command may (the registry's counts, the
guards' 64 and 16, the pixel transfer from its own pixel store state, the attribute lists to EGL_NONE, client vertex
memory for an enabled attribute with no buffer), asking AddressSanitizer for each region
(`__asan_region_is_poisoned`, found in every run); it lies where the registry leaves the answer to the library (random
values and handles, null texts and mappings, more configurations found than written, an extension count up to
2^31). Per run about 1.7M regions read and 24M written. Controls, same log: the clear's room removed, the liar run
ends at once (AddressSanitizer SEGV at 0x4, the empty slice's address); uniform4fv given one count more, "FAKE: a read
past what was given by glUniform4fv ..., 4112 bytes", status 134. Over Mesa the same plant is not seen (Mesa is not
instrumented): the lying library is what makes the fuzzing see reads past.

**The per-draw check's cost**, MEASURED same log (`draw_costs`, 200,000 draws of a triangle, one enabled attribute,
16 attributes, LP_NUM_THREADS=1, pinned to one core, rounds 1 to 4): 938 to 947 ns per draw with the check, 814 to 823
ns with the three checks removed: about 124 ns (15%) per draw on llvmpipe, whose draws are queued. INFERRED: the
check's 17 raw calls (MAX_VERTEX_ATTRIBS, then one or two glGetVertexAttribiv per attribute) cost about 7 ns each
inside Mesa (the 133 ns of a glGetVertexAttribiv through the trait is mostly the glue's current-context test). A cache
per vertex array would remove it; not done (no measurement says it matters).

Also at 4ae8cb6e2: generated twice IDENTICAL, SAME AS WRITTEN; check, wasm32 check, clippy `-D warnings` status 0; 13
of 13 tests default and AddressSanitizer; unsafe lines unchanged (egl direct 95, opengl-es direct 792, every other 0;
marks 357); linter 2,011 findings on the two crates (magic_literal 1,174 from the tests' literals,
summary_unanswered 210 and name_unanswered 203 the model rule sets with no key, public_outside_interface 204: the
two `refusals` and egl direct's `Binding`, the layout question).

## The destruction notifier, settled by Microsoft's page (2026-11-27)

READ, https://learn.microsoft.com/windows/win32/api/d3dcommon/nn-d3dcommon-id3ddestructionotifier: "To acquire an
instance of this interface, call QueryInterface on a Direct3D object with the IID of ID3DDestructionNotifier." The page
names no narrower set, so every Direct3D object answers it, not only buffers and textures (the earlier inference).
Followed: src/api.rs `every_kind` (every object kind of Direct3D 11 answers ID3DDestructionNotifier), read through
`Api::faces_of` by all seven places that read faces; the spike's slice keeps the spike's faces (`every_kind: &[]`).
A program-visible change: every Direct3D 11 object of ours now answers that id, as Windows' do by the page.
MEASURED at 3aa80a955 (x86-64 job gack7, `.gack/ac3/verify.sh`, log ac3x/verify-3aa80a955.log): the three sets written
twice IDENTICAL, check and clippy (and --all-targets) status 0; the whole set's 47 hostile tests and 30 probes default
and ASan status 0 (28 held, 15 and 16 through by rule, as before); slice harness 75 of 75; the hostile set's lie tests
44 of 44 (both sets, default and ASan); C caller honest (default, ASan) and one fuzzing seed of 10M under ASan, each set
status 0, 0 reports, 0 objects left: whole set "cases 633 of 635, exports 8 of 8; never reached 2 = unreachable listed 2
(ID3DBlob's two)", slice 185 of 192 as before; conformance on and through both glues status 0, 0 ASan reports, Direct3D
11 65 kinds, 945 calls (855 before: the notifier's two methods on 45 more kinds), 941 made, 0 panics.

## AC11, started: the C-header front end and the bridge's census (2026-11-27)

`src/cheader.rs`, command `c-header`: reads the C the bridge is written in (functions, structures, enumerations,
defines) and gives every parameter and return of every function one row of the Mesa compiler's mapping rule, or lists
it unmapped. Input: crates/nir/bridge/bridge.h and flat.h as graphics-implementations has them at d19c23829 (where GI8
part B builds). MEASURED at 1571b4566 (job gac-k, ac10/tip-1571b4566.log): 47 functions (bridge.h's 45 and flat.h's
wc_nir_flatten, wc_flat_free), 20 structures, 25 enumerations, 12 defines; UNMAPPED 0. Rows (census in
ac11/census.log): shader changed 24, read 11, made 4 (owned, freed by wc_nir_free on drop), given back 1, shaders made
into an array 1; numbers 40; declared values 2 (the returned wc_bind_result read as a number and checked: the open
SECURITY finding); the bridge's memory copied out whole then given back 5 (+5 frees inside); plain data in by value,
by slice and by fixed array 4, out with a capacity 1; text in 1, texts by stage 1; words in 1, out 1; the compiler's
options 1 (to AC8's Gallium trait, as NEEDED B1 says).
Left of AC11: the trait (`crates/mesa-compiler/interface.rs`, the shader type owning one `nir_shader`) and its direct
crate written from these rows; the bridge's build moved into the direct crate (or linked from nir's); the suite on Mesa.

Not reached in this run: AC8's Gallium trait (after AC11, by NEEDED's order) and the 57 Direct3D 11 methods not glued
(item 2 of the orchestrator's order); neither waits on another track.

## AC11, the trait layer and the direct glue (2026-11-27)

`api-crate-generator mesa-compiler <flat.h> <bridge.h> <out>` (src/mesacompiler.rs, templates/mesa-compiler/) writes
`crates/mesa-compiler` (trait `MesaCompiler`, `type Shader`, owned plain data for every structure, constants.rs with
the defines and every enumeration's values) and `crates/mesa-compiler/implementation/direct` (`Bridge`, `Shader`, the
glue). Input: bridge.h and flat.h copied from graphics-implementations d19c23829 into
tools/api-crate-generator/inputs/mesa-compiler/ (our own C; the copy pins a rerun). The trait follows NEEDED B1 and B2:
38 methods (47 functions: wc_init called once by `new`, wc_nir_free the shader's drop, the five frees inside the copies,
wc_finish never, wc_compiler_options to AC8's Gallium trait); every enumeration the bridge writes is a `u32` the caller
converts by a checked `TryFrom` (the returned wc_bind_result too: `to_common_form` gives `Err(code)` for any code but
0, which closes the open SECURITY finding of a `repr(C)` enum filled by C); C's `bool` is read as a byte `!= 0`; a
declared value passed in is refused past its enumeration (`spirv_to_nir`'s stage, 3 values); every structure the
bridge fills is zeroed by the glue, copied out whole (arrays, texts, the flat form, the GLSL program's names and log),
its shaders taken (set to null) and given back once by its free (READ bridge.c 1565 to 1583, flat.c 1454, glsl.cpp 609:
each takes a zeroed structure as nothing). `clone` is `cloned` (Clone's own method name). `Shader` is not `Send` (no
bridge fact is relied on for that; NEEDED B1 leaves it to this crate).

MEASURED at 051b56c5f (x86-64 job gac-h, log graphics-api-crates/ac11/claim-051b56c5f.log): generator build with
every warning shown: none; its 17 tests pass; written twice IDENTICAL; SAME AS WRITTEN; `cargo check --all-targets` and
`cargo clippy --all-targets -- -D warnings` of both packages status 0; lines naming `unsafe`: interface 0, constants 0,
direct 96 (48 marks: 43 calls and frees, `wc_init`, the drop, the two copies, the `extern` block); linter on
crates/mesa-compiler 506 findings (number_constant_without_link 209 for constants.rs's values, summary_unanswered 79 and
name_unanswered 47 the model rule sets with no key, magic_literal 67, public_outside_interface 8, the rest below 20).

**Where the bridge's C build lives: PROPOSED to graphics-implementations** (their answer goes in their status file).
The bridge (crates/nir/bridge: bridge.c, bridge.h, bridge_private.h, flat.c, flat.h, glsl.cpp, spirv.c) and the part of
crates/nir/build.rs that compiles it and links Mesa's compiler move into `crates/mesa-compiler/implementation/direct/`
(its own build.rs), the outside API's C beside the only crate that calls it; crates/nir then depends on
`wcontainers-mesa-compiler` for the trait and on `wcontainers-mesa-compiler-direct` for the Mesa route, and keeps no C
and no `unsafe` for the bridge (its 97 lines naming `unsafe` on this branch, 115 on graphics-layout, go: the 5 of
direct.rs and mesa.rs's bridge calls). Until then the direct crate declares the functions and links nothing itself: a
program linking crates/nir's built bridge resolves them, so GI8 part B can start on the trait now. The suite on Mesa
(AC11's runtime bar: the conformance calls, the hostile caller, the fuzzing under AddressSanitizer) runs once the
build is in the direct crate; it is not written yet.

## NEEDED "After GI7": the OpenGL ES list done in its order (2026-11-27)

His main (ac696ea19) is already inside the branch: `git fetch` of his checkout, then `git merge-base --is-ancestor
ac696ea19 HEAD` true, nothing to merge (MEASURED). Logs: wcontainers-scratch/graphics-api-crates/ac10/.

| Item | Taken from commit | Default-deny kind | Plant (removed or changed) | Test |
|---|---|---|---|---|
| 1. glStencilFuncSeparate, glStencilOpSeparate | ac4e18eb7 | every parameter a number (registry row) | face passed as FRONT_AND_BACK (each method), mask changed, two operations exchanged: 4 CAUGHT | stencil_faces_are_separate (both faces set apart and read back on a frame buffer with 8 stencil bits; neither refused by the glue) |
| 2. the eight extension values | ac4e18eb7 | values: the four extensions added to the set's list (gl.xml's numbers) | each value written as 0xfff0: 8 CAUGHT | extension_values_are_taken (each value taken without error where Mesa names the extension; all four named, MEASURED) |
| 2. UNMASKED_RENDERER_WEBGL 0x9246 | ac4e18eb7 | a value carried by name (new `values` lines of sets.rs, each with its reason; an undeclared name fails the generator) | LISTED: no native context answers it | none native (the browser's) |
| 3. the implementation on WebGL 2 (web-sys) | 44f2a1e9c | crate `wcontainers-opengl-es-webgl`, `WebGl::new(context, off)`, safe, no `unsafe` line | none: the browser checks every call; no test runs it (below) | wasm32 check and clippy only |
| 5. a context that owns its EGL objects | d932fd4e0 | `OpenGlEsContext::owning(egl, display, context, draw, read)`, destroyed at drop | destroy of the context removed, of the surface removed: 2 CAUGHT | owned::owning_destroys_at_drop |
| 7. EGL kept loaded for the process | d932fd4e0 | the first library of each folder kept (`KEPT`); later loads are libraries of their own | `keep` removed: CAUGHT | kept.rs (its own test program: libEGL still mapped after the last handle) |

Not done, each with its reason: 4, eglCreateWindowSurface (the window's shape is the question in their status, and the
window crate test-window is on graphics-layout, not on his main; Windows only, no desktop this run). 6, a cache of the
glue's per-draw and per-transfer reads: their measurement shows no visible cost on llvmpipe, and the check is a safety
check whose truth a cache would make depend on the glue tracking every change of state (a vertex array switched, a
bound buffer deleted); 124 ns a draw MEASURED before. (3a) the wait for query results between the browser's tasks needs
an import from the page (an `extern` block, `unsafe` under the workspace lints), not placed: it needs a home in an API
crate for the host's device imports. (3b) the extension objects are called by name through `js_sys::Reflect`, and
`get_stringi(EXTENSIONS, i)` answers WebGL's names, as asked. A browser run of the webgl crate needs a test harness
in a page (wasm-bindgen's test macros write `#[no_mangle]`, which the workspace forbids) and a headless browser.

MEASURED at d932fd4e0 (x86-64 job gac-k, log ac10/claim-d932fd4e0.log): generator build, its 17 tests; written twice
IDENTICAL; SAME AS WRITTEN; `cargo check --all-targets` 0 and the wasm32 check of the five libraries (webgl's included)
0; clippy `-D warnings` 0, and webgl's on wasm32 0; the suite 16 of 16 and kept 1 of 1, by default and under
AddressSanitizer; plants **171 CAUGHT, 15 MISSED, 0 BROKEN, 206 LISTED** (the 15 the ones of the table above,
unchanged); fuzzing 3 seeds x 10M over Mesa and 3 x 10M over the lying library, each status 0, no report; lines naming
`unsafe`: egl direct 95, opengl-es direct 802 (792 before: the two stencil methods' marks and the owning drop's
comments), webgl 0, every other 0; marks 361. Linter on the two crates: 2,197 findings (magic_literal 1,274,
summary_unanswered 227 and name_unanswered 226 the model rule sets with no key on the machine, public_outside_interface
216 the `refusals`, `owning` and egl direct's `Binding`: the layout question).

## AC11's runtime bar: the bridge moved into the direct crate; fuzzing not yet clean (2026-11-27)

The implementations track had not answered the proposal by the end of the list (newest graphics-implementations
7e53ee79e, READ): done as proposed. `crates/mesa-compiler/implementation/direct/bridge/` holds the bridge (bridge.c,
bridge.h, bridge_private.h, flat.c, flat.h, glsl.cpp, spirv.c, copied from graphics-implementations 7e53ee79e) and
`build.rs` builds it (from crates/nir/build.rs there, less what only nir hands its own code: the converter's version,
the reference renderers' files) when `WCONTAINERS_MESA` names Mesa's build, with the `standalone` feature linking Mesa's
compiler libraries. crates/nir is left as it is (the implementations track moves it onto the trait and drops its copy);
a program must not link both copies. The generator now reads the headers from there (inputs/mesa-compiler removed).
New: `crates/mesa-compiler/conformance` (the round trip, the refusals, a hostile caller of every method on every shader
of 11 it can make, a seeded fuzzer that also changes SPIR-V words, a probe of one call sequence alone) with three SPIR-V
programs written by glslang 16.4.0 from its GLSL.

New dependency (C, in the bridge, statically linked): **SPIRV-Tools 1.4.357.0** (Khronos, Apache-2.0, nixpkgs
`spirv-tools`), its validator run on every module before Mesa's reader, `WCONTAINERS_SPIRV_TOOLS` naming it. To vet.

Defects the hostile caller found, each fixed in the bridge (our C) and shown by a run before and after:
- to_spirv stopped the process on the OpenGL driver's form (Zink's assertion "unsupported intrinsic"): refused now
  (lowered IO, lowered loads), and written from a copy (its preparation turned the caller's shader into Zink's form,
  after which other steps stopped the process); a copy whose image or sampler location would be written past Zink's
  stack arrays is refused; Zink's `bindless_set_idx` was 0, every program's set (a third sampler written past an array
  of two): now none.
- textures_1d_as_2d validated before fixing the deref types (a SPIR-V program stopped the process).
- spirv_to_nir broke into the debugger on every refused module (`os_break`, SIGTRAP): `skip_os_break_in_debug_build`.
- spirv_to_nir read past its arrays on changed words (Mesa assumes valid SPIR-V): SPIRV-Tools' validator first, and
  the header checked by the bridge (Mesa's header refusals log through options it has not set: a null read).
- uniform_blocks_as_words unlinked block variables still reached by deref (a SPIR-V program, or a second call):
  refused for any shader reaching blocks by deref or by Vulkan's descriptors.
- nearest_integer_textures asked a texture type whether it is a shadow sampler (an assertion).
- point_size_also_at, point_square, geometry_squares, storage_pass_through: a generic slot past VAR31 (or near 2^32,
  which wrapped the old check) is refused; point_square takes only a vertex or evaluation program.
- fragcolor_to_buffers wrote locations past every table for a count past 8 draw buffers: refused.

MEASURED at a125704bb (x86-64 job gac-m, log ac11r/claim-a125704bb.log; Mesa fec8830e9 built by crates/nir/mesa/build
in 103 s): generator build, written twice IDENTICAL, the five generated files SAME AS WRITTEN; `cargo check
--all-targets` 0, also without Mesa 0; clippy `-D warnings` 0; the suite 5 of 5 by default and under AddressSanitizer
(the bridge's C instrumented: 29 `__asan_report` calls in its archive); every single call alone 0 of 2,376 ended the
process, every method then to_spirv (and twice then to_spirv) 0 of 3,168. **Fuzzing (10,000 calls a seed, under
AddressSanitizer, Mesa's assertions on): 3 of 12 seeds clean.** Left: Zink's writer asserts on a block whose
(set, binding) no variable has after the renumbering (`emit_vulkan_resource_index`, 8 seeds), reads a null storage
class of a block cast after a chain (1 seed, ASan SEGV), and Mesa's IO lowering asserts on locations a valid module
may declare (2 seeds). Guarding Zink's writer call by call did not converge in three rounds: brought to the
orchestrator (below). Lines naming `unsafe`: mesa-compiler 0, direct 96 (48 marks), conformance 0.

**For the orchestrator (a check that did not converge):** the trait's `to_spirv` (Zink's writer in its native Vulkan
mode) cannot be made safe for any shader a chain of the other methods leaves: Zink assumes the shapes its own driver
hands it. A ruling is asked between (a) the glue keeps per shader whether any changing method touched it since it
was made from SPIR-V, and `to_spirv` refuses a changed one (a generated flag; nir's route calls it on a shader it just
made), (b) `to_spirv` taken out of the trait until a validation of the form exists, (c) the steps nir's route takes
before it made one method. The IO lowering's assertions on valid modules' large locations need the bridge to check
every stage variable's location range before lowering (not done).

## AC11 under the orchestrator's ruling (c): shaders typed by their place, the bar held (2026-11-27)

The ruling: an order of calls the library does not take must be impossible to write. Done at d6e535461: the 38
free-standing methods became 14 on six shader types, each made only by the method before it in a chain our callers
take (READ crates/mesa-compiler/interface.rs): `SpirvShader` (spirv_read) -> `SpirvCommon` (spirv_to_common_form, the
common form's steps as one call) -> `spirv_common_to_spirv` or `spirv_common_flatten`, each taking it by value;
`Received` (glsl_to_nir; in the driver, what st/mesa hands create_*_state) -> `Prepared` (received_prepare, the driver's
creation steps as one call) -> `Variant` (prepared_variant, a draw's steps on a copy) -> `variant_flatten`; `Made` (the
programs the driver's steps make: pass-through and squares) -> made_variant. No method changes a shader in place; the
bridge's single steps are in bridge/steps.h, which Rust does not call. Six doctests show wrong orders do not compile
(Zink's writer on a read program, twice on one program, on a variant; creation steps on a SPIR-V program; a variant of
an unprepared stage; squares from a made program).

This run (fca6fad55): at d6e535461 two methods never made anything in 100,000 fuzzed calls (made_named_layer_location
0 of about 7,200 a seed; prepared_geometry_points_as_squares 0 of about 4,700): the GLSL route made only vertex,
fragment and compute stages, though the driver receives geometry and tessellation programs (READ graphics-implementations
92cfc5a76 crates/opengl/src/screen.rs:439 to 454). `enum wc_stage` now also has GEOMETRY 3, TESS_CONTROL 4,
TESS_EVALUATION 5 (the first three keep their numbers), glsl.cpp reports Mesa's own geometry and tessellation limits,
and the suite has a geometry program making points that names a layer and a control and evaluation pair.

MEASURED at 2613d8108 (pushed; x86-64 job gac-s/20261127-060920-3503591, log
wcontainers-scratch/graphics-api-crates/ac11t/claim-2613d8108.log; Mesa fec8830e9 with assertions on):
- generator build 0 warnings, 17 tests; written twice IDENTICAL; the five generated files SAME AS WRITTEN; UNMAPPED 0.
- `cargo check --all-targets` 0 (also without Mesa 0); clippy `-D warnings` 0; the suite 5 of 5 by default and under
  AddressSanitizer (29 `__asan_report` calls in the bridge's archive); doctests 7 of 7 (6 compile_fail, each for its code).
- every hostile case alone in its own process: PROBES 640, cases 10, calls 8,206, ENDED 0.
- **fuzzing, 12 seeds x 100,000 calls under AddressSanitizer: 12 of 12 status 0, no report, no assertion** (seeds 7919,
  104729, 1299709, 11, 12, 13, 21 to 26; 54 to 55 s each). Every method made what it makes in every seed, e.g. seed 7919:
  made_named_layer_location 139 of 5,627, prepared_geometry_points_as_squares 141 of 5,550, prepared_points_as_squares
  1,006 of 5,553.
- lines naming `unsafe`: mesa-compiler 0, conformance 0, direct 72; marks 36.
- linter on crates/mesa-compiler (job gac-s/20261127-061529..., log ac11t/lint-2613d8108.log; the linter built from
  reviewable-rust da55b9d7): 797 findings, number_constant_without_link 233 (constants.rs), magic_literal 202 (the suite's
  edge values and programs), name_unanswered 68 and summary_unanswered 66 (the model rule sets, no key on the machine),
  function_summary_not_third_person 52, summary_too_long 45, summary_not_one_line 29, public_outside_interface 28,
  summary_not_one_sentence 24, constant_defined_twice 23, the rest below 10 each.

Which Mesa driver takes each step in that position (READ Mesa 26.3.1 fec8830e9 on the build machine; ours are
graphics-implementations 92cfc5a76's crates/opengl and crates/nir):

| Method | Our caller | A Mesa driver taking the same step there |
|---|---|---|
| spirv_read (+ alphaToOne) | corpus run.rs:493 to 496 | every Vulkan driver: vk_spirv_to_nir (vulkan/runtime/vk_nir.c:246, 282), lavapipe lvp_pipeline.c:422; alphaToOne as nir_lower_alpha_to_one after reading (kosmickrisp msl_nir_lower_common.c:991) |
| spirv_shader_key | run.rs:567 | NIR serialised as the cache's key: vk_pipeline.c:703, vk_pipeline_cache.c:523 |
| spirv_to_common_form | run.rs:570, 671 | resources renumbered into the driver's own layout after reading: lavapipe lvp_lower_pipeline_layout (lvp_pipeline.c:588) |
| spirv_common_to_spirv | run.rs:672 (candidate A) | **none** (below) |
| spirv_common_flatten | run.rs:571, direct.rs:974 to 984 | out of SSA, then the back end's walk: Zink compile_module (zink_compiler.c:4087 to 4102) |
| glsl_to_nir | the suite (st/mesa's part) | st/mesa: st_link_glsl_to_nir (st_glsl_to_nir.cpp:438, 455), st_finalize_nir (732) |
| received_prepare | state.rs:1139 to 1180 | the driver's steps at create_*_state: d3d12_create_shader_impl (d3d12_compiler.cpp:1325), zink_shader_create (zink_compiler.c:5652) |
| prepared_variant | draw.rs:779 to 818 | a draw's variant from a copy of the created shader: d3d12 select_shader_variant (d3d12_compiler.cpp:1064, the clone at 1084, the key's steps to 1174) |
| prepared_points_as_squares | draw.rs:3426 to 3441 | d3d12's point sprite geometry program made for a draw of points (d3d12_compiler.cpp:666 to 679, d3d12_gs_variant.cpp:351) |
| prepared_geometry_points_as_squares | draw.rs:3514 to 3522 | d3d12_lower_point_sprite on the application's geometry program (d3d12_compiler.cpp:1087 to 1094) |
| made_key, made_named_layer_location, made_variant | state.rs:1186, 1195; draw.rs:3445, 3525 | d3d12's made geometry programs are shader selectors keyed and varied like the application's (d3d12_gs_variant.cpp:471, 525; select_shader_variant at d3d12_compiler.cpp:1530) |
| variant_flatten | draw.rs:840, 978, 1034, 1081 | the variant compiled by the back end: d3d12 compile_nir (d3d12_compiler.cpp:90, called at 1204) |

**For the orchestrator: spirv_common_to_spirv has no Mesa driver.** Zink's writer in its native Vulkan mode is reached
only through `ntv_shader_prepare` and `ntv_info.is_native_vulkan`, and nothing in Mesa 26.3.1 calls the one or sets
the other (MEASURED `grep -rn` over src: only nir_to_spirv.h:54, 77 and nir_to_spirv.c's own uses). Zink's own chain
(zink_shader_create, zink_shader_compile, compile_module: prune_io, nir_convert_from_ssa, nir_to_spirv with its screen's
ntv_info) needs a Zink screen, that is a Vulkan device. Our only caller is the corpus's candidate A (run.rs:672). Asked:
keep the method with that caller (its fuzzing is clean, above), or take it out of the trait (the corpus loses candidate
A). Until ruled it stays. spirv_to_common_form and spirv_common_flatten are our own route's steps; the table names the
Mesa steps nearest them.

**For the implementations track:** crates/nir on graphics-implementations 92cfc5a76 has dropped its copy of the bridge
(no crates/nir/bridge; it depends on wcontainers-mesa-compiler-direct) and has 0 lines naming `unsafe` in src (56 in its
corpus tests: lavapipe.rs 17, llvmpipe.rs 39; 115 in all on graphics-layout; MEASURED `git grep -c`). It calls the trait
of 051b56c5f (38 methods, `MesaCompiler + Clone`, wc_nir_* names); the typed trait of d6e535461 replaces it, so its
nir needs the move to the 14 methods (the walk of the suite, conformance/suite.rs, is the pattern).

## AC8, started: Gallium's trait planned with its objects typed from the start (2026-11-27)

Scope: the function pointers our OpenGL driver sets (READ graphics-implementations 92cfc5a76, crates/opengl/src: 11 of
pipe_screen, 84 of pipe_context, and u_transfer_helper's transfer_map, transfer_unmap and transfer_flush_region),
listed in tools/api-crate-generator/inputs/gallium/in-scope.txt. A pointer our driver leaves null is not offered.

`src/gallium.rs` (command `gallium-plan`) gives each pointer its roles over 21 kinds of object, each an associated type
of the trait to come: Screen, Context, Resource, Fence, the five states (Blend, DepthStencilAlpha, Rasterizer, Sampler,
VertexElements), one shader type per stage (VertexShader, FragmentShader, GeometryShader, TessControlShader,
TessEvaluationShader, ComputeShader; each made from mesa-compiler's `Received` of that stage, so a program made for
one stage cannot be bound at another and the NIR the driver gets is the type received_prepare takes), SamplerView,
StreamOutputTarget, Query, ActiveQuery, EndOnlyQuery and Mapping. Roles: makes (returned by value), gives back (taken
by value), binds (`Option<&K>` at slots), turns (begin_query takes a Query and gives an ActiveQuery; end_query the
reverse), reads (by reference), counts (fence_reference, the views' and resources' release: the last release ends the
object). Orders taken from Mesa's callers (READ Mesa 26.3.1): the cso context deletes no bound blend, depth-stencil,
rasterizer or vertex elements state (cso_context.c:137 to 169) but may delete a bound sampler (same place, "nothing to
do for samplers"), so the glue keeps a bound object alive until it is unbound; a timestamp query is only ended
(queryobj.c:269 to 277), hence EndOnlyQuery; DeleteQueries ends an active query first (queryobj.c:676 to 687) but the
context's teardown destroys one without (queryobj.c:1403 to 1417), so destroy_query takes an ActiveQuery too. Every
pointer names its first caller in Mesa (file:line), found by grep over st/mesa, main, the cso context, util and the DRI
front end (wcontainers-scratch/graphics-api-crates/ac8/callers.txt).

The C front end (src/cheader.rs) now reads what Gallium's headers are written in: structures and unions nested in
others (`union { ... } index` as the structure `pipe_draw_info.index`, one copy per declarator of `struct { ... } dst,
src`), bit-fields, unnamed parameters (`struct pipe_context *`), `static inline` bodies, and the conditionals (GCC,
little-endian and POSIX hold; C++, big-endian and Windows do not).

MEASURED at d07d2b122 (pushed; x86-64 job gac-s, log wcontainers-scratch/graphics-api-crates/ac8/plan-d07d2b122.log):
generator build with no warning, its 17 tests; the Mesa compiler's crates written twice IDENTICAL and SAME AS WRITTEN
(the front end's changes leave them as they were); census of the headers: pipe_context 151 function pointers,
pipe_screen 81 (232; the brief's 234 counted two typedefs), 128 parameter types; **plan status 0**: 98 pointers planned,
each once, each found in its structure (u_transfer_vtbl read from u_transfer_helper.h), every kind made and given back
by some pointer; **UNMAPPED 0**: every parameter and return by a row of the Gallium rule (number by value 90, the context
87, nothing 68, an object by reference or taken 31, plain data copied 21 + 1 exception, an object of the pointer's kind
21, an object made 16, a declared value checked 11, the library 11, plain data naming objects 9, a program (Received) 6,
and the rest below 4), five exception lines (two surfaces and a box Mesa passes as non-const but only reads; two of the
front end's own pointers, carried as numbers and never read). The plain structures the pointers read and the objects
they name are listed (e.g. pipe_framebuffer_state: cbufs.texture, zsbuf.texture, resolve; pipe_draw_info:
index.resource, a union with a user pointer).

Next in AC8: interface.rs written from the plan (the kinds as associated types, the plain structures as owned data with
each handle a reference of its kind, bit-fields as their declared type with the width checked), then the serving glue in
`gallium/implementation/direct` over bindgen's layouts (as crates/opengl's build.rs takes them), a hostile C caller of
the 98 pointers and the fuzzing. Open in the plan: pipe_draw_info's user index pointer and pipe_shader_state's
error_message (a string the driver allocates for Mesa) need their own rows; the driver must not report user index
buffers for the first to stay out.

## The orchestrator's rulings (1) to (5) of 2026-11-27, done (logs in wcontainers-scratch/graphics-api-crates/r14/)

His main ac696ea19 is inside the branch (`git fetch` of his checkout, `git rev-list --count HEAD..FETCH_HEAD` 0,
MEASURED): nothing to merge; no interface file of his is touched by any commit of this run.

1. **`spirv_common_to_spirv` is out of the trait** (ce6ec529a): wc_spirv_common_to_spirv and wc_nir_to_spirv out of the
   bridge (bridge.h, steps.h, spirv.c), Zink's writer and its include folders out of the build (build.rs no longer links
   libzink), `struct wc_words` and wc_words_free out (no caller left). The corpus's candidate A goes with it (the
   implementations track's run.rs:672 and its route tests in crates/nir/tests/route.rs call the old `into_spirv`).
2. **The guard `resource_indices_found` is removed** with the writer it guarded (spirv.c: 154 lines out), and
   `locations_fit` and `native_vulkan_form` with it.
3. **The stage is part of the type.** The trait now has `ReceivedVertex` ... `ReceivedTessEvaluation` and
   `PreparedVertex` ... `PreparedTessEvaluation` (one type each per `enum wc_stage` value); `glsl_to_nir` returns a
   `GlslProgram` with one field per stage, each of its stage's type; `received_<stage>_prepare` and
   `prepared_<stage>_variant` exist for every stage; the squares only where a stage draws them:
   `prepared_vertex_points_as_squares` and `prepared_tess_evaluation_points_as_squares` (bridge.c:2355 takes only those
   two) and `prepared_geometry_points_as_squares` (bridge.c:2493 takes only a geometry program). Written by the
   generator (src/mesacompiler.rs: `BY_STAGE`, `STAGES_TAKEN`; the glue's mark names the fact it rests on: wc_glsl_to_nir
   fills stages[s] with a shader of stage s, and wc_received_prepare keeps the stage of the shader it takes). Ten
   doctests now show the wrong orders do not compile, four of them the stages (a geometry stage through the vertex
   creation steps, a fragment program's squares, a vertex program rewritten as geometry squares, a tessellation
   control variant through the vertex method). The trait: 24 methods (was 14), 16 associated types.
4. **No test that tests nothing reports ok.** `fuzz_from_the_environment` and `probe_from_the_environment` (and
   OpenGL ES's fuzz and measure) take a default when the environment names none (the Mesa compiler: seed 1, 10,000
   calls, the probe "0,0"; OpenGL ES: seed 1, 20,000 calls, 1,000 draws), so the default suite runs each and fails it
   where it fails. The first tries, separate programs (0e473154c) and `#[ignore]`, are refused by the linter's
   `conformance_not_run` rule (a test in `tests!` must be a plain `#[test]`; an implementation names its conformance
   crate only from conformance_run.rs), so defaults it is. `every_method_is_called` and every fuzzing seed now assert
   that each of the trait's 24 methods made what it makes (`METHODS`); the hostile rounds went from 4 to 16 for it, and
   the evaluation program writes a point size so its squares are made.

MEASURED at 3d6d06865 (pushed; x86-64 job gac-s/20261127-075636-2737835, log r14/mc-claim-3d6d06865.log, Mesa fec8830e9
with its assertions on):

| Bar | Result |
|---|---|
| Generator: build, tests; written twice; equal to the tree | status 0, 17 pass; IDENTICAL; SAME AS WRITTEN (all five files); UNMAPPED 0 |
| check (also without Mesa), clippy `-D warnings` | 0, 0, 0 |
| The suite by default; under AddressSanitizer (26 `__asan_report` calls in the bridge's archive) | 5 of 5 (fuzz and probe with their defaults: `FUZZ seed 1 calls 10002`, `PROBE 0,0 ended`); 5 of 5 |
| Doctests | 11 of 11 (10 compile_fail, each for its code) |
| Every hostile case alone in a process | PROBES 640, cases 10, calls 7,460, ENDED 0 |
| **Fuzzing, 12 seeds x 100,000 calls under AddressSanitizer** | **12 of 12 status 0, no report, no assertion; every seed asserts every method made something (no `made 0` in any seed)**; e.g. seed 11: made_named_layer_location 134 of 6,448, prepared_tess_evaluation_points_as_squares 183 of 337, prepared_geometry_points_as_squares 135 of 333, prepared_compute_variant 787 of 1,614; 63 to 64 s each |
| Lines naming `unsafe` | mesa-compiler 0, conformance 0, direct 132 (72 before: the stage methods are one glue each, so 6 prepares, 6 variants and 3 squares where there were 1, 1 and 2); marks 66 |
| Linter on crates/mesa-compiler | 977 (797 at 2613d8108): number_constant_without_link 235, magic_literal 213, name_unanswered 97 and summary_unanswered 72 (the model rule sets, no key on the machine), summary_too_long 76, public_outside_interface 68 and function_summary_not_third_person 68 (the stage methods' copies), conformance_not_run 2, conformance_untested_method 1 (`new`), the rest below 50 |

5. **The browser's turn lives in the WebGL implementation** (3d6d06865). crates/opengl-es/implementation/webgl:
   `get_query_objectuiv` and `get_query_objectui64v_ext` asked for QUERY_RESULT wait, as OpenGL ES does, until
   QUERY_RESULT_AVAILABLE: each turn suspends the WebAssembly stack on a promise a message posted to the page itself
   settles (`Promise.resolve` imported through wasm-bindgen's `suspending`, that is `WebAssembly.Suspending`; a
   MessageChannel message is a task of its own, which a nested zero timer is not). `client_wait_sync` waits the same
   way up to its timeout (WebGL signals fences only between tasks too). No method was added to any trait; the crate
   has no `unsafe` line; one `#[expect(deprecated)]` on the module (wasm-bindgen marks every suspending import
   deprecated while it calls JSPI experimental). New web-sys features MessageChannel, MessageEvent, MessagePort (the
   same crate, already a dependency). The caller must enter through `WebAssembly.promising`, as the gpu suite's page
   does.
   **Cost, MEASURED** (headless Chromium 155.0.8059.39 on SwiftShader, x86-64, 200 queries, r14/webgl-suite.txt): a draw
   read back 0.253 ms; the same draw in an occlusion query whose result is read at once 1.575 ms, every result right
   (200 of 200 passed), about 111 turns a query (each turn about 12 us; SwiftShader answers the query after about 1.3
   ms). **A host without JSPI** cannot even instantiate the module (the import is wrapped in `WebAssembly.Suspending`);
   standing in for it (`WebAssembly.Suspending = inner => inner`, as the page's `nojspi=1` does) the wait sees no turn
   taken and stops at once: the result reads as WebGL answers within the task, the query unavailable, 0 (MEASURED: 200
   queries, 0 passed, 0.015 ms each; `query_results_wait` FAILS there, as it must: that host cannot give the answer).
   **The OpenGL ES suite without EGL**: every test that needs only OpenGL ES has a form `*_on(&G, Refusals<G>)` taking
   a context made by any means (`Refusals` is `None` for an implementation whose checks another makes: the browser);
   `ON_ANY_CONTEXT` names the 12 and `run_on_any_context` runs one by name; a new test `query_results_wait` (two
   occlusion queries read at once, one passing samples and one not) runs in both forms. The EGL forms call the `_on`
   forms, so both run the same code. The page: crates/opengl-es/implementation/webgl/examples/browser.rs (wasm32,
   `forbid`), browser/suite.html, browser/suite.mjs (playwright-core 1.55.0 and wasm-bindgen-cli 0.2.130, tools on the
   machine under the job's build folder, never in the lock).
   Four WebGL differences the browser run found, fixed in the WebGL implementation: the variable-length lists
   (COMPRESSED_TEXTURE_FORMATS and the two binary format lists) refused as the trait refuses them; a vertex attribute
   pointer refused with no array buffer bound (WebGL takes offset 0); a clear given fewer than four channels padded
   with zeros (WebGL refuses it); and in the suite, skipped unpack pixels given rows long enough (WebGL refuses a skip
   past a row's end, OpenGL ES reads on) and extension names matched by WebGL's own names.

MEASURED at 3d6d06865 (x86-64 job gac-k/20261127-075642-2738543, log r14/es-claim-3d6d06865.log):

| Bar | Result |
|---|---|
| Generator; written twice; equal to the tree | 17 pass; IDENTICAL; SAME AS WRITTEN |
| check (Linux, wasm32), clippy `-D warnings` (Linux, wasm32, the page included) | 0, 0, 0, 0, 0 |
| The suite on direct over Mesa 26.2.4's EGL (llvmpipe); under AddressSanitizer | 17 of 17 and kept 1 of 1 (fuzz with its default 20,000 calls, measure with 1,000 draws, query_results_wait new); the same, status 0 |
| **The suite on WebGL 2 in headless Chromium, through the WebGL implementation** | **12 passed, 0 failed of 12** (every test of `ON_ANY_CONTEXT`, query_results_wait among them) |
| Lines naming `unsafe` | egl 0, egl direct 95, opengl-es 0, opengl-es direct 802, webgl 0, conformance 0; marks 361 (unchanged) |

For the implementations track: their gpu suite's occlusion test should now pass on the page with this crate as it is
at 3d6d06865 (not run by this track: their page and suite are on their branch). The Mesa compiler's trait changed
shape: nir's calls move to the stage methods (`received_<stage>_prepare`, `prepared_<stage>_variant`, the three square
methods) and `GlslProgram`'s six fields; candidate A and nir's route tests through `into_spirv` go.

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
