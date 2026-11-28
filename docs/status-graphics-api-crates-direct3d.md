# Status: the graphics API crates, Direct3D part

Brief: wcontainers-scratch/orchestrator/restart-2026-11-25/brief-graphics-api-crates.md, with the orchestrator's runs of
2026-11-27 (this branch is the Direct3D half of the API track; the Gallium half stays on `graphics-api-crates`). Branch
`graphics-api-crates-direct3d`, made from f3af6417f. Marks: MEASURED (a run; its job and log named), READ, INFERRED.
Logs: wcontainers-scratch/graphics-api-crates/d3d/ (this run's in d3d/run-1127b/). Every job ran on the x86-64 machine
with `--jobs 12`.

His main: `git fetch` of his checkout's main gives ac696ea19, already inside this branch (`git rev-list --count
HEAD..FETCH_HEAD` 0, MEASURED): nothing to merge. origin/graphics-api-crates (ce6790aaa) merged with a normal merge
(9333bd4b4); only Cargo.lock conflicted (both sides kept: the Gallium direct crate's entry). No interface file of his is
touched by any commit of this branch.

## S. Run of 2026-11-27 night: the safe system constructors, valid suite arguments, every API's plant list (logs d3d/run-1127d/)

His main 1f24bddad merged with a normal merge (bfba07972, clean). Commits, all pushed: f485f13b1, 75d363f22, 78e995e15,
6faaeb1cd, ca328ceb0, e962bc8ac, 3c1e8e8b6, c7f8c6846, 009770561 and the plant runner's last change. The x86-64 machine
was lost twice (about 15:55 to 19:20 Pacific); those hours ran on the ARM64 machine, whose disk was nearly full.

### S.1 A safe way to the system's library (the implementations track's request)

Each direct crate with a library now has `System<Api>::system()`, Windows only (src/glue_write.rs `system_library`):
an `unsafe extern "system"` block `#[link(name = "d3d11" | "dxgi" | "d3d12")]` declaring each export with the
metadata's parameters (`#[link_name = "D3D11CreateDevice"]` ...), and `system()` building `Library` from those items and
calling `new` in the direct crate's one marked block, so the promise `new` asks for is kept where the exports are
declared. Two marks a crate, reasons in src/undo.rs. Its test, tests/system.rs in each direct crate (templates/system/),
is `#![forbid(unsafe_code)]`: hostile arguments refused before the system sees them (a software module, the
switch-to-reference flag and an undeclared flag, 17 feature levels; DXGI's debug and an undeclared factory flag; an
interface no device has), and an honest call gets its object (WARP at 11_0, a factory, a device). MEASURED: windows-gnu
type-check of the three direct crates' tests status 0 (x86 job gacd 20261127-213114, ARM job gacd 20261128-010350).
Not run: it needs Windows (AC9). The serving side is checked by `refusals_through_glues` (S.2).

### S.2 What the comparison now covers (the check's blocking 2)

- `SUITE_DIFFERENT` accepts only a line of what the implementation received, never an answer, and only 7 methods remain
  (GetImmediateContext 0 to 3, GetDevice, GetClassLinkage: answered from the glue's record, no arguments). The suite's
  invalid arguments became valid ones in src/api.rs `SUITE_ARGUMENTS` (CreateBuffer, CreateTexture1D/2D/3D with initial
  bytes of exactly the rule's length, UpdateSubresource(1), Map, Unmap, IASetVertexBuffers, the UAV slot, the swap
  chain's description, GetBuffer, CheckFeatureSupport, and the one value each pin passes), and the calls the calling glue
  makes of its own are taken out by `SUITE_EXTRA_CALLS` (the buffer's description for Map and UpdateSubresource, the
  multithread protection of D3D11CreateDevice(AndSwapChain)). Makers that create come first, those by interface id only
  after; a compared call that hands out an interface by id asks for one the implementation has.
- The recorder logs which objects an argument holds (an optional object, each element of an array of objects); a
  mapping's pitches are compared; a union has a known value the same in every member (src/glue_hostile.rs
  `union_known`), and each union codec has a test that a member given alone is written.
- `refusals_through_glues` (each recorder): the whole drive through both glues reaches no refusal entry of the serving
  glue, so a calling glue that forwards what it should refuse (the check's plant c, a GPU address) fails it.
- tests/exact: `the_serving_glue_passes_what_the_calling_glue_pins` (a raise flag, breaks turned on, a software module
  reach the recorder through the serving glue) and `one_byte_past_the_lent_memory_is_stopped` (under AddressSanitizer,
  C touching one byte past the lent memory of a buffer and of each texture shape is a heap-buffer-overflow).
- The calling glue passes a pin's one value (0, false, null) as a constant; codecs nothing reaches are not written.

### S.3 The bar

| Group | At | Total | Caught | Survived | Other |
|---|---|---|---|---|---|
| lengths.rs, cargo-mutants (unit tests and exact tests, dev profile: overflow checks on) | e962bc8ac | 289 | 281 | 0 | 8 unviable |
| plants/direct3d-12.tsv | e962bc8ac (86), then with the runner fix (164) | 250 | 250 | 0 | 0 invalid; plant 87 hung (killed at 600 s) |
| plants/lengths.tsv, reference tests | 009770561 | 39 | 39 | 0 | 0 |
| plants/lengths.tsv, exact C tests under AddressSanitizer | 009770561 | 39 | 39 (the four `lent_length_*.over` among them, by `one_byte_past_the_lent_memory_is_stopped`) | 0 | 0 |
| fuzz under AddressSanitizer, 3 seeds x 10M: hostile set / C caller | e962bc8ac | 6 runs | status 0, 0 reports | | |
| plants/direct3d-11.tsv (3,201; stage command: the direct crate's unit tests, the recorder's comparison and refusal count) | e962bc8ac to 009770561 | 2,305 judged | 2,217 | 23 | 5 invalid, 60 hung (killed at 900 s, most likely builds on a loaded machine), 896 not run |
| plants/directx-graphics-infrastructure.tsv (867) | e962bc8ac to 009770561 | 702 judged | 686 | 13 | 3 hung, 165 not run |
| generator, this run's functions at the tip (`--re`; answer_summary, presence_of, argument, read_value, union_known, without_unused_codecs) | 64ddf38a1 | 83 | 78 | 0 | 3 unviable, 2 timeouts (an endless loop each), the run stopped before its end |
| generator, the whole diff since f710cf5b2 | e962bc8ac | 345 | 294 | 37 | 14 unviable; most survivors since fixed (S.4) |

Verdict logs: d3d/run-1127d/gacdpx-verdicts.log (Direct3D 11), gacdpg-verdicts.log (DXGI), gacdp12-verdicts.log.

### S.4 What survives, and why (to fix next, in this order)

- Direct3D 11 and DXGI plants judged under an earlier suite and fixed since (to be rerun at the tip with the full
  command): the mapped pitches (650, 651), GetType and Buffer::GetDesc of a buffer (the new probe in the recorder's
  hostile tests, which the stage command does not run), SetPrivateDataInterface's object (the suite now passes one),
  IDXGIFactory5::CheckFeatureSupport's feature.
- Answers the serving glue gives from its record (GetType of each texture, GetDesc of textures, queries, views, a
  surface and a swap chain, GetBuffer): the suite does not compare them (the recorder is never asked); the probe of S.2
  covers a buffer only. Next: compare each with what its creation passed, for every kind.
- Arguments the suite passes as zero where zero is the only valid value it has: a buffer's subresource for Map, Unmap and
  UpdateSubresource(1), RSGetViewports' and RSGetScissorRects' counts. Next: map a texture's second subresource.
- Single cases: SetMultithreadProtected's value in the glue's own call, set_marker, SetMuteDebugOutput, the shader trace
  registers, DecoderBeginFrame1's arrays, the video device's feature check and key exchange, DXGI's
  Offer/ReclaimResources, CreateSoftwareAdapter, GetDisplayModeList1's out count, two InfoQueue filters.
- 63 hung and 1,061 not run: to rerun with fewer copies at once and a longer limit.
- The generator's remaining survivors at e962bc8ac outside the functions fixed: known_value's tuple salts,
  recorded_method (4), Shapes::zero, codecs' newtype detection.

### S.5 Not done this run

D3 (the (heap, index) line, (resource, offset), the pipeline states, the unions holding resources, ExecuteCommandLists,
Map), the Direct3D 12 hostile set, D2, A6, A7: not started; the run's time went to the bar above and to two losses of
the x86-64 machine.

## R. Run of 2026-11-27 evening: values compared, plants committed, mutation runs (logs d3d/run-1127c/)

Merged origin/graphics-api-crates d2cd0e0c4 (055ce667f, clean); his main ac696ea19 already inside (count 0). Commits:
db940d1ee, ebc7e6ac6, 24582daa0, 7051deefa, 0bcfa1fbc, 3304a1492 (all pushed).

### R.1 Every answer compared with what the implementation gave (FIX 1)

- The recorder answers KNOWN values: every integer 1 to 126 by a salt of the method and field, `true`, enumerations
  and handles non-zero, every structure a literal of known fields, made from the interface crates' text only
  (src/glue_hostile.rs `Shapes`), never from the glue's decoders (the check's point (i): the reference shares no code).
  It writes known bytes into every caller's buffer it is lent, and logs every argument it receives.
- The suite passes KNOWN arguments (enumerations a declared non-zero value no special line refuses, one element in every
  array, an object it holds where one is optional) and notes, for every method both glues carry to the implementation,
  its answer (`Debug`, every field; an object named only as one), the caller's bytes after it, and what the
  implementation received. `every_answer_and_argument_equals_the_reference` (the recorder's conformance_through_glues)
  drives the recorder directly and through both glues and compares line by line; a method that differs by design is
  named in src/api.rs `SUITE_DIFFERENT` with the reason (28 lines: debug breaks and exception modes refused, the
  immediate context and a context's device answered from the glue's record, multithread protection turned on, a row
  clearing bit 0, and the suite's own known arguments past documented ranges); any other difference fails.
- Counts by method (the generator's ANSWERS lines, MEASURED report d3d/run-1127c/report.txt):

| API | plain data, every field | a status only | holds objects (all but identity) / object or failure / object or none / list of objects | returns nothing (arguments compared) | answered by a glue's record (not the implementation's) | not glued |
|---|---|---|---|---|---|---|
| d3dcommon | 1 | 1 | 0 / 0 / 0 / 0 | 0 | 0 | 2 |
| DXGI | 74 | 51 | 3 / 24 / 1 / 1 | 17 | 11 | 15 |
| Direct3D 11 (with the DXGI faces its kinds answer) | 201 | 99 | 19 / 69 / 11 / 21 | 132 (+3 compared by the bytes they write) | 30 | 51 |
| Direct3D 12 | 6 | 6 | 0 / 9 / 0 / 0 | 12 | 1 | 19 |
| COM | (no methods: IUnknown is the bookkeeping's, tested by its own tests) |

  Not compared: an object's identity (an object comes back "an object"); the 42 methods a glue answers from its record
  (descriptions recorded at creation, the immediate context; compared with that record by the slice's round trip and
  the hostile probes, not by this test); the not-glued (both glues refuse, the hostile tests check the refusal).
- The check's three plants (d3d12 plant list, last three lines): CAUGHT, each by `every_answer_equals_the_reference`
  at suite.rs:2029 (MEASURED, job gacdp3 at ebc7e6ac6, `PLANT 248 checker.serve-getdesc-garbage: CAUGHT status 101`, 249,
  250 the same).

### R.2 The exact C tests catch what only the reference caught (FIX 2)

tests/exact now also makes BC2 and BC3 textures (2D off the block grid, 3D, a full chain) and the four 96-bit formats
(1D array, 2D, 3D), hands every box past an edge to both glues (the serving glue gets one byte and must refuse before the
implementation; the calling glue gets the whole subresource plus 64 KiB and must refuse before the system), and COM's
shadow memory is exactly the length lent under the `c-caller` feature (`exact-shadow`, lent.rs), so that a touch past
what the glue lends is outside the block. MEASURED (job gacd 20261127-152343, 24582daa0's tree): "EXACT SERVING: 21
resources created, 996 uploads, 312 boxes past an edge refused, 97 maps"; "EXACT CALLING: 622 short bytes refused, 996
exact uploads, 312 boxes past an edge refused, 406 maps"; under AddressSanitizer status 0, 0 reports.
Plants (plants/lengths.tsv, 39: each rule one byte under and over, and the checks' seven), MEASURED jobs gacdl1/gacdl2
(24582daa0) and gacd 20261127-170918 (the two whose `const fn` the runner missed, rerun): reference tests 39 of 39
CAUGHT; exact under AddressSanitizer 35 of 39 CAUGHT, among them all four the check named: bc2-eight-bytes,
r32g32b32-as-8, lent-rows-minus-one, box-past-accepted, each `ERROR: AddressSanitizer: heap-buffer-overflow`. Survived
under the exact tests: `lent_length_*.over` (4): lending one byte more of the glue's own memory is no access a C caller
touching only the documented bytes can see; the reference tests catch all four.

### R.3 The default-deny branch for an unjudged UINT64 (FIX 3)

The judging is now one function, `uint64_refusals` (src/glue.rs); generator tests `an_unjudged_uint64_refuses_the_method`
(the fence's value with its line taken out of the table, and a UINT64 no line ever held: each refused with "a UINT64
src/api.rs has not judged an address or a number"), `a_judged_number_passes`, `addresses_refuse_the_method_by_their_kind`.
MEASURED: generator tests status 0 in every job of this run.

### R.4 The Windows-target setup for the implementations track (FIX 4)

wcontainers-scratch/graphics-api-crates/d3d/wintool/: flake.nix, flake.lock, rust-toolchain.toml (Rust 1.99.0 with
x86_64-pc-windows-gnu and -msvc), check-windows.sh, README (three lines). MinGW's gcc/g++ 16.2.0 come in as
`CC/CXX/AR_x86_64_pc_windows_gnu` only (as a shell input the cross compiler also became the host's `CC` and broke the
host's build scripts), with mcfgthread's headers added (`CXXFLAGS_x86_64_pc_windows_gnu`). MEASURED (job gacd-win
20261127-161938 on the implementations branch 398a91f7b with its submodules filled): `wcontainers-shader-converters-linked`
builds for windows-gnu (libdxbc_spirv.a, libdxil_spirv.a written by MinGW's g++); the windows-gnu check then stops only
in `wcontainers-directx` (733) and the Direct3D 12 front's examples (169, 415): `unsafe` under the workspace's `forbid`,
the implementations track's code, not the toolchain. windows-msvc: crates without C or C++ build scripts only (no MSVC
compiler here): the three direct crates status 0.

### R.5 The test bar: plants and the mutation tool

| Group | Run at | Mutants or plants | Caught | Survived | Unviable / invalid | Not run |
|---|---|---|---|---|---|---|
| generator, lines changed this run and the last (cargo-mutants `--in-diff`, oracle tests/written.rs) | 7051deefa | 193 | 157 | 30 | 6 | 0 |
| lengths.rs (cargo-mutants, suite: the crate's unit tests and the exact C tests, release) | shards 0 to 2 at 7051deefa, 3 to 5 at 3304a1492 | 289 | 200 | 81 | 8 | 0 |
| plants/direct3d-12.tsv (a field of every copy function, an argument of every call each way, the check's 3) | 0bcfa1fbc | 250 | 209 | 23 | 18 (an object or a structure with no other value to name) | 0 |
| plants/lengths.tsv, reference tests / exact under ASan | 24582daa0 | 39 / 39 | 39 / 35 | 0 / 4 | 0 | 0 |

Survivors, each with why (MEASURED lists in d3d/run-1127c/):
- Generator (30, mutants-generator-missed.txt): 8 in codecs' `first_values` and flag detection (src/glue_hostile.rs
  1308 to 1331), superseded this run by `Shapes` and reached by nothing: to remove; 7 in `known_value`'s tuple salts
  and `answer_summary`'s variable names (equivalent: the output differs only where no set has a tuple answer); 4 in
  `recorded_method` and 2 in `uint64s` (fixed arrays of UINT64: none in the set); 5 in `with_dev_dependencies` and
  `returned_structures_through_pointers` and 2 in `write_direct`, paths the five APIs never take; 1 `Shapes::zero`,
  1 `argument`. Not reached for the committed set; each is to be removed or given a set that reaches it.
- lengths.rs (81, mutants-lengths-missed.txt): the description checks and the derived getters, which no test of this
  suite gives a documented value: checked_swap_chain 18, checked_texture_1d/2d/3d 23, within_served_bound and its four
  forms 10, padded_extent 3, resolved_texture_* 3, type_of_* 4, data_size_of_query 7, checked_buffer 2, total, mips;
  the 12 of data_size_of_query, type_of_buffer and checked_buffer were shards 1 and 2 at 7051deefa, before the tests
  `data_size_of_every_query_kind_is_its_documented_structure` and `a_buffer_is_of_the_buffer_dimension_and_checked_by_
  its_documented_ranges` (3304a1492; not re-measured). Next: tests from Microsoft's pages for the swap chain's,
  textures' and the served bound's rules, then a rerun of these mutants (`cargo mutants --re`).
- Direct3D 12 plants (23; jobs gacdp1 to gacdp3 at 0bcfa1fbc, logs plants-d3d12-0bcf-{1,2,3}.log; the first run at
  ebc7e6ac6, before known arguments, had 108 survivors): the unions D3D12_CLEAR_VALUE and D3D12_DEPTH_STENCIL_VALUE (5:
  a union is passed as zero bytes, its members reading the same bytes); D3D12_RANGE and the CPU and GPU descriptor
  handles (6: Unmap and the handles are not carried to the implementation through both glues, so nothing compares
  them: reached by nothing in the suite); and 12 arguments that are optional objects or their pointers
  (SetCompute/GraphicsRootSignature, CreateCommandList's and Reset's initial state, SetDescriptorHeaps, Unmap, the
  clear value's and the query heap's pointers): the recorder logs an object as "an object", so passing none instead of
  one is not seen. Next: log whether an optional object is there, and carry Unmap and the handles (D3).
### R.7 The claim at 3304a1492 (pushed; x86-64; logs claim-3304a1492-*.log)

| Bar | Result (MEASURED) |
|---|---|
| Generator build, its tests (the three UINT64 tests among them); written twice; equal to the tree | 0, 0; IDENTICAL; SAME AS WRITTEN |
| check, clippy -D warnings (22 packages); windows-gnu, windows-msvc type-check | 0, 0; 0, 0 |
| Tests (the comparison through both glues among them); under AddressSanitizer | status 0; status 0, 0 reports |
| C caller honest, default and AddressSanitizer | 0, 0: "cases 753 of 755, exports 9 of 9; never reached 2 = unreachable listed 2" |
| Exact C tests default and AddressSanitizer; lengths reference tests | 0; 0, 0 reports; 0 |
| Fuzzing under AddressSanitizer, 10M calls: hostile set / C caller | seed 11: 0 (1,475 s; 625,966 lies, 0 panics back; with known arguments ten times the work of a2f3a88d0's 117 s) / 0 (100 s); seeds 12, 13: 0 and 0 (1,434 s and 1,425 s; 546,371 and 938,934 lies; 0 panics back) / 0 and 0 (221 s, 222 s); 0 reports in all six |
| Lines naming `unsafe` | COM direct 95, d3dcommon direct 63, DXGI direct 2,548, Direct3D 11 direct 7,115, Direct3D 12 direct 784: unchanged from c6339e7a7; interface, recorder, hostile, conformance crates 0 (the hostile and recorder 1 is `#![forbid(unsafe_code)]` of their tests) |
| Linter | not run this run: its source copy (.gacd/lint) was moved away by the last run and no binary was left on the machine |

### R.6 Not done this run, and why

D3's 11 remaining Direct3D 12 methods, the Direct3D 12 hostile set, D2, A6 and A7: not started; this run's time went to
the bar above. Next in order: D3 (the (heap, index) line first), waiting on nothing.

## 0. Run of 2026-11-27 afternoon: the check's findings, the two rulings, D3 continued

Commits: f710cf5b2 (lengths reference, exact C tests, CreateQuery test, the C caller moved), 54a46fafb (GPU addresses
refused, whole rows lent), c6339e7a7 (structures returned by value glued, descriptor addresses refused). All pushed.

### 0.1 The length rules against a reference that shares no code with them (FIX 1)

- **The reference** (`crates/direct3d-11/implementation/direct/lengths_reference.rs`, template
  `tools/api-crate-generator/templates/direct3d-11-direct/lengths_reference.rs`): every expected length worked out afresh
  from Microsoft's pages, each cited in its header: D3D11_SUBRESOURCE_DATA (the element at (x, y, d) is at `d *
  SysMemSlicePitch + y * SysMemPitch + x * BytesPerPixel`, pitches used only for 2D/3D), UpdateSubresource (row and depth
  pitch, box in bytes for a buffer, texels otherwise, an empty box a no-op, its worked 2 x 3 x 4 example), D3D11_BOX,
  D3D11_MAPPED_SUBRESOURCE (pData is row 0 of slice 0, the pitches are only for iterating rows and slices, a 2D texture's
  depth pitch is not used, a buffer's pitches are at least its size), DXGI_FORMAT (each format listed by number in a
  table of its own, 100 entries, every other number refused), texture block compression, subresources and
  D3D11CalcSubresource, mip sizes, and the resource limit (`min(max(128, 0.25 x VRAM), 2048)` MiB, so 2048 MiB at most).
  It uses no function of lengths.rs: only the interface's description structures, to call the rules.
- **The tests** (`lengths_tests.rs`, a unit test module of the direct crate): `upload_length_buffer_matches_the_reference`,
  `initial_and_mapped_lengths_of_a_texture_1d_...`, `upload_initial_and_mapped_lengths_of_a_texture_2d_...` and `..._3d_...`
  compare `upload_length_*`, `initial_length_*`, `mapped_length_*` and `lent_length_*` with the reference over every format
  number -1 to 200 and four past the list (plain, packed pairs, block-compressed, depth, planar and the other refused),
  every dimension (buffers of 1 byte to 4 GiB - 1, 1D and 2D arrays, 3D, full chains and fixed mip counts), every
  subresource and one past the last, 15 boxes (none, whole, inside off the block grid, one texel, empty on each axis,
  past each edge, block-aligned, one texel inside a block) and 8 pairs of pitches (packed, padded to 256, padded by an odd
  amount, zero, a depth pitch far past the rows, pitches past the largest resource); `lengths_worked_out_by_hand` checks
  the reference itself with 32 assertions of numbers written out from the pages.
- **What the reference found in lengths.rs** (both fixed):
  1. A box that does not start or end on a 4 x 4 block's edge (block-compressed) or a pair's edge (R8G8_B8G8) was
     counted by its texel width, not the blocks it touches: the box (2, 2)-(6, 6) of BC1 is 2 x 2 blocks, the rule
     said 1 x 1; the system reads every block it touches. Now `blocks_touched` (ceil(end / side) - start / side).
  2. Map's two uses needed two rules. The calling glue viewed the system's memory raised to the pitches (a 2D texture's
     depth pitch included, which the page says is not used: a system answering a large one would have had the glue read
     past what it lent); the serving glue lent no more than that. Now `mapped_length_*` is exactly what the page lets a
     program view (the calling glue's view) and `lent_length_*` is what the serving glue lends: the view raised to every
     row whole at the row pitch and every slice whole at the depth pitch (a buffer's and 1D texture's to both pitches),
     which programs fill whole, bounded as before by `within_served_bound_*`. The serving glue now lends that even past
     the implementation's own bytes (the rest cleared, nothing written back past them): the slice's pass-through game,
     whose implementation is the calling glue's exact view, needed it (it failed at f710cf5b2, passes at 54a46fafb).
- **Exactly sized C, both directions** (`tests/exact`, the direct crate's integration test, `exact.c` built by build.rs
  under the `c-caller` feature): a C caller of the serving glue (over the recorder) hands initial data and uploads in heap
  blocks of exactly the reference's length and touches exactly the reference's lent bytes of Map's memory; a C stand-in
  d3d11.dll for the calling glue reads exactly the reference's length of each initial subresource and upload (byte by
  byte, so no read is optimized away) and lends from Map a heap block of exactly the reference's view. The calling test
  also hands one byte fewer (must be refused before the system) and checks the view's length. MEASURED at 54a46fafb and
  c6339e7a7: "EXACT SERVING: 14 resources created, 591 uploads, 57 maps touching 3792896 bytes"; "EXACT CALLING: 371 short
  bytes refused, 591 exact uploads, 240 maps viewing 13019327412 bytes; the stand-in saw [31, 591, 248, 248, 0]" (the last
  0: no slot it does not serve was called); under AddressSanitizer (C instrumented with zig's clang): status 0, 0 reports.
- **Plants** (job gacdp/20261127-125951-3023771 at 54a46fafb, `.gacd/plant-lengths.sh`, log
  d3d/run-1127b/plants-lengths-54a46fafb.log): each rule's result made one byte fewer (under) or more (over) in a copy of
  lengths.rs, and the checker's three plants. "PLANTS: 35; caught by the reference tests 35; caught under asan 27".

| Rule (each of buffer, 1D, 2D, 3D) | Under: reference / exact under ASan | Over: reference / exact under ASan |
|---|---|---|
| `upload_length_*` | 4 of 4 fail / 4 of 4 ASan report (`READ of size 1`: the stand-in reading past the one-short slice the calling glue let through) | 4 of 4 / 4 of 4 ASan report (`READ of size n+1`: the serving glue reading past C's exact block) |
| `initial_length_*` | 4 of 4 / 4 of 4 ASan report (the same) | 4 of 4 / 4 of 4 ASan report (the same) |
| `mapped_length_*` (the calling glue's view) | 4 of 4 / 4 of 4 assertion (the view is shorter than the reference's: main.rs:789) | 4 of 4 / 4 of 4 ASan report (the glue reading past the stand-in's exact block) |
| `lent_length_*` (what the serving glue lends) | 4 of 4 / not caught (0 of 4) | 4 of 4 / not caught (0 of 4) |
| checker's plants: one row pitch short; depth ignored; the pitches ignored | 3 of 3 / 3 of 3 ASan report | |

  Why the exact test cannot see a lent rule one byte off (READ): the serving glue's shadow memory comes in power-of-two
  classes (COM direct `Shadow::allocate`), so a touch one byte past the lent length is still inside the glue's own block:
  not memory outside the glue, but a byte it does not copy back. The reference tests catch all 8.

### 0.2 CreateQuery's kind, both ways (FIX 2)

`create_query_hands_out_the_kind_the_object_answers` (tests/exact): the stand-in's predicate answers ID3D11Predicate and,
as the runtime's does, ID3D11Query1; its query answers ID3D11Query1. MEASURED: "CREATE QUERY: 6 predicates as predicates,
10 queries as queries, a query without ID3D11Query1 refused". `query_object_of` asks for the predicate first, so a
predicate (which also answers the query kind's interface) comes back as a predicate; asking the query kind first would
hand it out as a query. Plant (job gacdp/20261127-133458-3700618 at c6339e7a7, `.gacd/plant-query.sh`, log
d3d/run-1127b/plant-query.log): the two questions of `query_object_of` swapped in a copy: "exact create_query status 101",
"a predicate's object (5) comes back as a predicate" (main.rs:859). MEASURED.

The order for older runtimes: not followed, and why. Each kind is held by its newest interface (the brief's object
model: ID3D11Device5, ID3D11DeviceContext4, ID3D11Query1, ID3D11Texture2D1 ...), and every glued call goes through that
interface's table; an object answering only ID3D11Query (the 11.0 runtime) has no ID3D11Query1 table to hold, so
`query_object_of` asks ID3D11Predicate, then ID3D11Query1, and refuses the rest (E_NOINTERFACE; the test above). The same
holds for every kind (from_owned_older asks the newest interface): the calling glue needs the 11.4 runtime (Windows 10).
Asking ID3D11Query after ID3D11Query1 would only make sense with a per-version table, which no caller needs.

### 0.3 The rulings followed

- **Q1, the C caller**: it is the direct crate's own integration test (`crates/direct3d-11/implementation/direct/tests/
  c_caller/{main.rs,caller.rs,caller.c}`); the direct crate's build.rs compiles its C (and exact.c) only under the cargo
  feature `c-caller` (`cc` optional, `[[test]] required-features`), linking the archive into test programs only
  (`rustc-link-arg-tests`); the suite's job runs `cargo test -p wcontainers-direct3d-11-direct --features c-caller --test
  c_caller` (and `--test exact`). The separate workspace and its manifest are gone; the root manifest is not changed. The
  linter's `crate_not_member` finding is gone (0 at c6339e7a7; MEASURED in the claim below).
- **Q2, GPU virtual addresses**: under default deny each is an address. The metadata drops the type name
  `D3D12_GPU_VIRTUAL_ADDRESS`, so every UINT64 and SIZE_T a carried Direct3D 12 method reaches is judged by
  `src/api.rs UINT64_JUDGED` (41 rows: 14 addresses, 27 numbers); an address, or one not judged, refuses the method in
  both glues (it is in NOT GLUED with its reason). No line lets one through as a number. Refused for it now:
  IASetVertexBuffers, IASetIndexBuffer, SOSetTargets, SetGraphicsRootConstantBufferView, SetComputeRootConstantBufferView,
  SetComputeRootUnorderedAccessView, GetGPUVirtualAddress (all glued as numbers before), and for the descriptor handles'
  addresses (`D3D12_CPU_DESCRIPTOR_HANDLE.ptr`, `D3D12_GPU_DESCRIPTOR_HANDLE.ptr`) also OMSetRenderTargets (glued before),
  CreateRenderTargetView, CreateShaderResourceView, ClearRenderTargetView, Get{CPU,GPU}DescriptorHandleForHeapStart,
  SetGraphicsRootDescriptorTable. The line itself is next (0.5).

### 0.4 D3 continued: structures returned by value

A structure a COM method returns by value is read (src/emit.rs `returned_structures_through_pointers`) as a pointer the
caller passes first (`RetVal`, a structure out; attribute indices moved by one) and marked `ReturnsPointer`: the trait
returns the structure as for any structure out, the serving glue fills the caller's pointer and returns it, the calling
glue passes its own buffer, the refusing entries keep the same C form. Glued now: ID3D12Resource::GetDesc,
ID3D12DescriptorHeap::GetDesc. GLUED 622 -> 623 over the set (MEASURED, report d3d/run-1127b/r2/report.txt: +2 GetDesc,
-OMSetRenderTargets refused by its descriptor address). Direct3D 11, DXGI, d3dcommon and COM crates: unchanged by it
(DIFFERENT only in Direct3D 12 and the C caller's table).

### 0.5 Next, in order (D3's 11 remaining, then the rest)

| Method(s) | Needs |
|---|---|
| Get{CPU,GPU}DescriptorHandleForHeapStart, CreateRenderTargetView, CreateShaderResourceView, ClearRenderTargetView, OMSetRenderTargets, SetGraphicsRootDescriptorTable | the (heap, index) line: the trait takes a heap and an index; the calling glue bounds the index by the heap's recorded description and computes the address from the heap's start and the device's increment; the serving glue answers its own heap starts and decodes an address back to (heap, index) or refuses |
| IASetVertexBuffers, IASetIndexBuffer, SOSetTargets, Set*RootConstantBufferView / UnorderedAccessView, GetGPUVirtualAddress | the (resource, offset) line (ruling Q2): offset and the view's size checked against the resource's recorded width; the serving glue hands out addresses of its own per resource and decodes them back |
| CreateGraphicsPipelineState, CreateComputePipelineState | descriptions holding bytecode pointers, input layout, stream output, root signature object |
| CopyTextureRegion, ResourceBarrier | a union whose member the selector picks, holding a resource object |
| ExecuteCommandLists | an array of objects of several kinds |
| Map / Unmap (ID3D12Resource) | the `Lent` line of Direct3D 11's Map, range as numbers checked against the recorded width, with a reference of its own |

Then the Direct3D 12 hostile set (lies and fuzzing of its own), the layout cross-check (rerun below), D2 (the HLSL
compiler crate, section 4's design), A6 and A7 (section 6). Nothing of these waits on another track.

### 0.7 The claim at c6339e7a7 (pushed)

Job gacd/20261127-133259-3685280 (log d3d/run-1127b/claim-c6339e7a7.log); its sanitizer steps rerun on a fresh target
folder in job gacd/20261127-140114-4062562 (log asan-fresh-c6339e7a7.log), because the first job's AddressSanitizer test
step failed on probe 32 ("THROUGH 1 calls", hostile.rs:1039) three times out of three: the old plant script (`.gacd/
plant.sh`) built its planted copies into the tree's own AddressSanitizer target folder, and cargo took the planted
text-limit build, whose sources are older by mtime and hashed by relative path, as fresh for the tree. The plant script
now builds into its own folder; on the fresh folder every sanitizer step passes (below). INFERRED from that: the earlier
claims' sanitizer rows may have run on a stale planted build; only this fresh run is claimed.

| Bar | Result (MEASURED) |
|---|---|
| Generator: build, its tests; written twice; equal to the tree | 0, 0; IDENTICAL; SAME AS WRITTEN |
| GLUED / NOT GLUED over the set | 623 / 141 (a2f3a88d0: 629 / 135; the change is section 0.3's refusals and 0.4's two) |
| check, clippy -D warnings (all targets, 22 packages); windows-gnu, windows-msvc type-check | 0, 0; 0, 0 |
| Tests, default; under AddressSanitizer (fresh target) | status 0; status 0, 0 reports |
| The direct crate with `--features c-caller`: clippy; C caller honest default and AddressSanitizer | 0; 0, 0 ("cases 753 of 755, exports 9 of 9; never reached 2 = unreachable listed 2") |
| Exact C tests, default and AddressSanitizer; lengths reference tests | 0, 0 (0 reports); 0 |
| Fuzzing under AddressSanitizer, seeds 11, 12, 13 x 10M: Direct3D 11 hostile set; C caller over every table | 3 of 3 status 0 (116 to 118 s; 11,376 to 19,330 lies; 0 panics back); 3 of 3 status 0; 0 reports |
| The slice and its harness (each test alone, default, small table, AddressSanitizer) | IDENTICAL, check 0, clippy 0, tests 0; "harness runs: 75 status 0: 75" |
| The five plants of section 2 | each caught (upload-calling, upload-serving: AddressSanitizer READ past the region; text-limit: probe 32 panics; the two not-yet plants: round_trip.rs:633) |
| Layout cross-check against MinGW (`check-layout.sh direct3d-glue`, d3d12.h added) | 2,633 assertions written; 79 records MinGW does not declare (listed, left out); 2,124 compiled: compile status 0, 0 fail; the planted one is the only failure. At a830d8964's run (no d3d12.h): 1,097 compiled |
| Linter (API crates, translated fronts apart) | 27,118 findings (public_outside_interface 4,292, magic_literal 4,269, name_unanswered 3,183, function_summary_not_third_person 3,060, summary_unanswered 2,669, the rest below 500); `crate_not_member` for the C caller: gone |

One number moved that the bars do not judge: the hostile fuzz's "mappings of the system never unmapped" at the end of a
run is 10 to 12 (a2f3a88d0: 3 to 6), INFERRED from the lend change (the serving glue now lends whole rows past a lying
implementation's short bytes where it refused before, so more of the fuzz's maps stay open when its run stops).

### 0.6 How the implementations track checks its code against Windows on the rented Linux machines

It has no Windows target; this branch's jobs used two things, both on the x86-64 machine:
1. **Type-checking for Windows**: the repository's toolchain has no Windows target, so `.gacd/wintool/` (on this branch's
   scratch, not committed) is a copy of the repository's flake whose `rust-toolchain.toml` adds the targets
   `x86_64-pc-windows-gnu` and `x86_64-pc-windows-msvc` to the same Rust 1.99.0. The step is
   `CARGO_TARGET_DIR=<own folder> nix develop path:$PWD/.gacd/wintool -c cargo check -p <packages> --target
   x86_64-pc-windows-gnu` (and `-msvc`). It checks that the code compiles for Windows (the `cfg(windows)` branches
   included); it links nothing and runs nothing.
2. **Layouts against MinGW's headers**: `tools/api-crate-generator/check-layout.sh direct3d-glue <work folder>` (run from
   the repository's root inside `nix develop`, which has zig) writes one `_Static_assert` per record size and named field
   offset as the generator lays them out, and compiles it with `zig cc -target x86_64-windows-gnu` against MinGW's
   d3d11, dxgi and (from c6339e7a7) d3d12 headers; a record MinGW does not declare is listed and left out; one planted
   wrong size must be the only failure.
What neither shows (Windows, AC9): behavior of Microsoft's d3d11.dll and d3d12.dll themselves.

## 1. The Direct3D 11, DXGI, d3dcommon and COM crates in crates/ (a830d8964)

What was generator output in job folders is now committed, written by `api-crate-generator generate <winmd>
direct3d-11-glue`, in the layout of the other API crates:

| Crate (package) | Holds |
|---|---|
| `crates/component-object-model` (+ `implementation/direct`) | COM's `Guid`, `HResult`, Win32 plain data; COM's bookkeeping glue (object table, foreign objects, lent memory) |
| `crates/direct3d-common` (+ `direct`, `recorder`, `hostile`, `conformance`) | d3dcommon.h's traits and data |
| `crates/directx-graphics-infrastructure` (+ the same four) | dxgi.dll: 47 interfaces, 191 methods, 6 exports |
| `crates/direct3d-11` (+ the same four, and `direct/c-caller`) | d3d11.dll: 77 interfaces, 440 methods, 2 exports |

- Every `unsafe` line is in an `implementation/direct` crate (`unsafe_code = "deny"`, `unsafe_op_in_unsafe_fn = "deny"`,
  each use `#[expect(unsafe_code, reason = ..)]` on one operation). The interface, recorder, hostile and conformance
  crates inherit the workspace's `forbid`.
- The hand-written fronts moved out of the way to where the layout puts them (51637d1c4): `crates/direct3d-11/
  implementation/translated` (package `wcontainers-direct3d-11-translated`) and `crates/directx-graphics-infrastructure/
  implementation/translated` (`-translated`). Their dependents (`directx`, `direct3d-12`) name them with `package =`, so
  no code of theirs changed; the fronts' own tests name their crate by its new name. Nothing of the fronts implements
  the traits yet (AC4).
- The hostile C caller was a workspace of its own (b4b1dd679); by the orchestrator's ruling it is now the direct crate's
  integration test behind the `c-caller` feature (section 0.3).

MEASURED at a830d8964 (job gacd/20261127-090931-99723, log d3d/claim-a830d8964/tree.log):

| Bar | Result |
|---|---|
| Written twice; equal to the tree | IDENTICAL; SAME AS WRITTEN (every generated crate) |
| `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings` (the 17 packages, workspace lints) | 0, 0 |
| Type-check for x86_64-pc-windows-gnu and -msvc (the spike's toolchain, the same Rust 1.99.0 with both targets) | 0, 0 |
| Tests (direct crates, conformance on and through both glues, 47 hostile tests and probes, effects, lies) | status 0, default and under AddressSanitizer (0 reports) |
| The C caller, honest run, default and AddressSanitizer | 0, 0; "cases 633 of 635, exports 8 of 8; never reached 2 = unreachable listed 2" |
| Fuzzing under AddressSanitizer, seeds 11, 12, 13 x 10M calls: hostile set / C caller | 6 of 6 status 0, 0 reports |
| The DXGI front: check --all-targets, tests | 0, 0 (11 tests) |
| Lines naming `unsafe` | COM direct 95, d3dcommon direct 63, DXGI direct 2,548, Direct3D 11 direct 7,023, its C caller 4; 0 in the interface, recorder, hostile and conformance crates |

The Direct3D 11 front, `directx` and `direct3d-12` stop where they stopped on his main: `wcontainers-shaders` alone
fails with 15 `unsafe` errors under the workspace's `forbid` (MEASURED, same job), the implementations track's.

## 2. D1: what the implementations track's 70 lines need first (4b8c9e736 generator, acfe942d4 crates)

NEEDED-FROM-API-CRATES.md, fourteenth run, D1 (57 methods). At a830d8964 three of them were not glued (CreateQuery,
UpdateSubresource, CreateGeometryShaderWithStreamOutput) and GetData could not tell S_FALSE from S_OK. Now all 57 are
glued, and CreateInputLayout with them (graphics-test's helpers make input layouts): GLUED 586 -> 591, NOT GLUED 57 ->
52 (MEASURED, report d3d/d1/report.txt; each of the 57 names, and CreateInputLayout, found in the GLUED line, none missing).

1. **GetData answers whether its data is ready.** Exception line `NotYet` (src/api.rs): the trait method returns
   `Result<bool, HResult>`, `true` for S_OK (written), `false` for S_FALSE (not yet, nothing written). The serving glue
   answers S_FALSE for `false` (`status_ready` in COM's direct crate); the calling glue answers `false` for S_FALSE and
   writes nothing to the caller's bytes. The hostile implementation answers `false` every time and the recorder `true`,
   so both answers cross both glues in every run; the slice's round trip asks both and checks the caller's bytes are
   untouched while not ready.
2. **CreateQuery hands out a query or a predicate.** An object made of one of several kinds (the trait's `QueryObject`)
   is now glued for any such method: the calling glue takes the reference as an IUnknown and asks it for each kind's
   newest interface, the most derived kind first (`query_object_of`), so the kind is what the object answers, never what
   a description says; the serving glue serves the kind the implementation made, with that kind's records. Probe 31:
   an occlusion predicate's description through both glues to the recorder crosses as the query the recorder made.
   Which kind Microsoft's runtime makes for a predicate's description is not seen here (Windows, AC9).
3. **UpdateSubresource (and UpdateSubresource1) take `&[u8]`.** Exception line `Upload`: the length is the resource's
   shape's rule (lengths.rs `upload_length_*`) from its description, the box (none: the whole subresource; past the
   subresource: refused; empty: zero) and the pitches. The calling glue refuses fewer bytes; the serving glue copies that
   many once, only for a resource whose description it recorded when the resource crossed (an unrecorded one, such as
   a swap chain's buffer, is refused). Probe 18: 4 bytes for a 256-byte buffer and a box past its end refused; the exact
   256 bytes and a 32-byte box pass, each a heap allocation of exactly that length.
4. **Structures holding texts in an array in** (`D3D11_INPUT_ELEMENT_DESC`, `D3D11_SO_DECLARATION_ENTRY`): the calling
   glue passes each `CString`'s address; the serving glue copies each text once up to its first zero byte and refuses a
   null one or one past 65,536 bytes (`read_text`, one mark per text field). Such a structure anywhere else (by pointer,
   out, a fixed array) is refused by the generator. The C caller writes each element's text field as a text of ours
   (short, empty, 70,000 bytes, or null), never the random bytes' address. Probe 32.
5. **Map as a copy** needs nothing new (READ): the calling glue's `map` returns `MappedSubresource<ForeignLent>`, whose
   `VolatileBytes::read` copies the system's mapped bytes out, revoked before Unmap; the pitches come back with it.
6. **The other 53** were glued already (READ, the GLUED line at a830d8964).

MEASURED at acfe942d4 (job gacd/20261127-095247-1083459, log d3d/d1/claim-acfe942d4.log; plants job
gacd/20261127-101119-1469839):

| Bar | Result |
|---|---|
| Generator: build (no warning), its 17 tests; written twice; equal to the tree | 0, 17 pass; IDENTICAL; SAME AS WRITTEN |
| check, clippy -D warnings (all targets); windows-gnu and -msvc type-check | 0, 0; 0, 0 |
| Tests, default and AddressSanitizer (49 hostile tests and probes now) | status 0, 0 reports |
| C caller honest, default and AddressSanitizer | 0, 0; cases 633 of 635, never reached 2 = listed 2 |
| Fuzzing under AddressSanitizer, 3 seeds x 10M: hostile set; C caller | 3 of 3 (117 to 123 s, 11,338 to 19,294 lies, 0 panics back); 3 of 3; 0 reports |
| The slice: written twice, check, clippy, tests; its harness, each test alone, default + small table + AddressSanitizer | IDENTICAL, 0, 0, 0; 75 of 75 status 0 (30 asan symbols in the C) |
| Lines naming `unsafe` | Direct3D 11 direct 7,023 -> 7,081; DXGI direct 2,548; COM direct 95; d3dcommon direct 63 |

Every new check has a plant (a copy of the generator with the check removed; each must fail):

| Plant | Caught by | Deciding line |
|---|---|---|
| calling glue: upload length check removed | probe 18 under AddressSanitizer, status 101 | `READ of size 256 ... 0 bytes after 4-byte region` |
| serving glue: one byte more than the upload's rule | probe 18 under AddressSanitizer, status 101 | `READ of size 257 ... 0 bytes after 256-byte region` |
| serving glue: a structure's text limit raised to 1 MiB | probe 32, status 101 | `panicked at .../recorder/tests/hostile.rs:1039` (the 70,000-byte name reached the implementation) |
| calling glue: S_FALSE answered as ready | the slice's round trip, status 101 | `the_toy_through_both_glues ... FAILED`, round_trip.rs:633 |
| serving glue: S_OK answered for not ready | the slice's round trip, status 101 | the same |

## 3. What can be measured on Linux, and what needs Windows

Measured here, on the rented Linux machines (READ: the jobs above): the generator's own tests and its checks (complete,
asked-once, reach, kinds, reasons); both glues compiled, clippy-clean and type-checked for both Windows targets; the
whole calling glue over the whole serving glue's exports (`served`) with the recorder and the hostile implementation;
the hostile C caller (a stand-in C program calling every table entry and export with C's own pointers and counts);
fuzzing both under AddressSanitizer; the slice's spike harness (a fake d3d11 in C, correct and hostile C games). Not
rerun in this run but standing: the layout check (`check-layout.sh`: every record size and field offset compiled with
zig against MinGW's headers for x86_64-windows-gnu, 1,097 assertions, MEASURED 2026-11-25) and the mirror check.

Needs Windows (AC9; the desktop is not used in this run): the calling glue against Microsoft's d3d11.dll and dxgi.dll
(the 70 lines' tests moved onto `direct`), which kind CreateQuery's object answers for a predicate's description,
S_FALSE from a real query, real Map pitches, and the picture comparisons.

## 4. D2: what an API crate for the HLSL compiler would be (1 line: gpu-webgpu tests/direct3d.rs:612)

READ (the metadata census of 2026-11-25: d3dcompiler_47.dll's export D3DCompile and the ID3DBlob of d3dcommon):
`crates/hlsl-compiler` (top trait `HlslCompiler`, library d3dcompiler_47.dll), one export carried, `D3DCompile`. Its
parameters by kind: the source bytes and their length (bytes in), the source name (text, optional), the defines (an
array of `D3D_SHADER_MACRO`, structures holding two texts: the array-in form item 4 above gives), `pInclude`
(`ID3DInclude*`, a callback table of the program's: a kind the system trusts, so a row `NullOnly`, and the special value
`D3D_COMPILE_STANDARD_FILE_INCLUDE` refused, it reads files), entry point and target (texts), two flag words (judged),
`ppCode` and `ppErrorMsgs` (d3dcommon's `Blob` objects out). Two exception lines are new: the blob's bytes (its
`GetBufferPointer` is memory lent until Release, refused today) copied out inside the calling glue's own method while it
holds the reference, so the trait's blob gives `Vec<u8>`; and an out-parameter that holds an answer on failure (the
error blob), so the trait method returns `Result<Vec<u8>, (HResult, Vec<u8>)>` as the needs file asks. The calling glue
would load d3dcompiler_47.dll as the others load theirs. Nothing of it is built in this run.

## 5. D3: Direct3D 12's API crate, started (73f4fd2d6 generator, 891bc55ce front moved, a2f3a88d0 crates)

- **One set for every Direct3D API.** `direct3d-glue` (src/api.rs) writes COM, d3dcommon, DXGI, Direct3D 11 and Direct3D
  12 in one generation, so one COM crate holds the plain data all of them name (a second generation for Direct3D 12 alone
  would write a different COM crate into the same folder). The Direct3D 11, DXGI and COM crates it writes keep every bar
  of section 2 (below). `direct3d-11-glue` and `direct3d-11-slice` stay as they were.
- **Direct3D 12 cut to what its users call** (`DIRECT3D_12_SUBSET`): D3's 33 names (its "30 methods" with Map, Unmap
  and GetDesc) and the 18 that make the device, queue, allocator, list, heap, root signature, committed resource and
  fence and run the list: 14 interfaces, 51 methods, 10 kinds; the C tables keep every slot of each interface, the rest
  refusing. 34 enumerations, 2 masks and 8 interfaces judged (READ, src/api.rs `DIRECT3D_12_JUDGED`, `_EFFECTS`).
- **Generator changes** it needed: a structure a COM method returns crosses through a pointer the caller passes after
  `this`, which the method returns (d3d12.h's C form, `RetVal`), in every refusing entry and the C caller; an `IUnknown`
  field makes its structure generic over the API; the spike's slice is Direct3D 11 cut down, not any API with a cut;
  the C caller serves Direct3D 12's export by its own hostile library; its sweeps call an export that takes no interface
  id (D3D11CreateDevice) eight times a pass, so that the objects it makes are held by the sweeps however many cases the
  set has (with Direct3D 12's 120 cases added, the honest run had reached only 469 of 755 cases; now 753 of 755).
- **The front moved** to `crates/direct3d-12/implementation/translated` (`wcontainers-direct3d-12-translated`), its
  paths to the shared test files and shaders two levels deeper.
- **Glued: 38 of the 51.** Not glued, each with its reason in the report (READ, d3d/d3/report.txt), and what each needs:

| Method | Why not | Needs (next run) |
|---|---|---|
| CreateRenderTargetView, CreateShaderResourceView, ClearRenderTargetView, OMSetRenderTargets | `D3D12_CPU_DESCRIPTOR_HANDLE.ptr`, a pointer-width integer the system writes descriptors at: refused by kind | an exception line: a handle is (heap, index), the glue bounds the index by the heap's recorded description and computes the address from the heap's start and the device's increment |
| GetCPUDescriptorHandleForHeapStart, GetGPUDescriptorHandleForHeapStart, ID3D12Resource::GetDesc, ID3D12DescriptorHeap::GetDesc | a structure returned by value (refusing entries carry the convention; the glued form does not yet) | the `RetVal` form in both glues (the calling glue's own heap start feeds the handle line above) |
| SetGraphicsRootDescriptorTable | `D3D12_GPU_DESCRIPTOR_HANDLE` by value (a structure of one UINT64) | glue for a one-integer structure by value, and the same (heap, index) line for GPU handles |
| CreateGraphicsPipelineState, CreateComputePipelineState | descriptions holding shader bytecode pointers, the input layout, stream output and the root signature's object | the description's pointers as slices of the trait (bytes and structures in, copied once), its root signature an object |
| CopyTextureRegion, ResourceBarrier | structures holding a resource's object inside a union | a union whose member the selector field picks (the type of copy location, of barrier) |
| ExecuteCommandLists | an array of command lists of several kinds | the array form of section 2's "several kinds" |
| Map (and Unmap's range) | memory lent until Unmap, `void**`, and `D3D12_RANGE` of pointer-width integers | the `Lent` line of Direct3D 11's Map, with the range as numbers checked against the resource's recorded size |

  GPU virtual addresses (`D3D12_GPU_VIRTUAL_ADDRESS`, a UINT64 in SetGraphicsRootConstantBufferView,
  SetComputeRootConstantBufferView, SetComputeRootUnorderedAccessView, IASetVertexBuffers, IASetIndexBuffer, SOSetTargets)
  pass today as numbers, by their type. INFERRED: such an address reaches only the GPU's memory of this device, which the
  CPU reaches only through Map's lent bytes (`VolatileBytes`), so a wrong one is the GPU's fault, not memory unsafety of
  ours; the default-deny rule cannot tell an address from a number by type here. To settle: either an exception line that
  makes each a (resource, offset) bounded by the resource's size, or a ruling that they are numbers.

MEASURED at a2f3a88d0 (job gacd/20261127-103754-942542, log d3d/d3/claim-a2f3a88d0.log):

| Bar | Result |
|---|---|
| Generator build, written twice, equal to the tree | 0; IDENTICAL; SAME AS WRITTEN (all five API crates) |
| GLUED / NOT GLUED over the set | 629 / 135 (Direct3D 11's 591 and 52 unchanged; Direct3D 12's 38 glued, 13 of its 51 refused with their reasons, the rest of its slots outside the cut) |
| check, clippy -D warnings (all targets, the 22 packages); windows-gnu and -msvc type-check | 0, 0; 0, 0 |
| Tests, default and AddressSanitizer (Direct3D 12's conformance on its recorder and hostile implementation, directly and through both glues, among them) | status 0; 0 reports |
| C caller honest, default and AddressSanitizer | 0, 0; cases 753 of 755, exports 9 of 9, never reached 2 = listed 2 |
| Fuzzing under AddressSanitizer, 3 seeds x 10M: Direct3D 11's hostile set; the C caller over every table of the five APIs | 3 of 3, 3 of 3; 0 reports |
| The slice and its harness (each test alone, three builds) | IDENTICAL, check 0, clippy 0, tests 0; 75 of 75 |
| The five plants of section 2 at this commit | each caught again (status 101; the two upload plants an AddressSanitizer READ past the region) |
| The fronts with every lint capped at a warning (a measuring device: shaders' unsafe is the implementations track's) | check --all-targets of the Direct3D 11 and 12 fronts and directx 0; the Direct3D 11 front's tests 0 |
| Lines naming `unsafe` | Direct3D 12 direct 826 (new), its interface, recorder, hostile, conformance 0; Direct3D 11 direct 7,081; DXGI direct 2,548; COM direct 95; d3dcommon direct 63; the Direct3D 12 front 503 (moved, unchanged) |
| Linter on the five API crates (the translated fronts apart) | 27,104 findings: public_outside_interface 4,292, magic_literal 4,272, name_unanswered 3,236 and summary_unanswered 2,745 (the model rule sets, no key on the machine), function_summary_not_third_person 3,032, the rest below 500 each; 1 crate_not_member (the C caller, below) |

Not run on Direct3D 12 yet: a hostile set's lie tests and fuzzing of its own (Direct3D 11's lies.rs and effects.rs are
written for Direct3D 11 alone), and hostile tests of its rows (it has none: no row is needed by the 38 glued).

## 6. A6 and A7 (desktop OpenGL with EGL, Vulkan): not started

READ (the needs file's A6, A7): A6 is desktop OpenGL 4.5, compatibility profile, through Mesa's EGL; the Khronos front
end already reads gl.xml, so it is a new set of the `khronos` command for the `gl` API (A4's calls plus the 14 and the 16
entry points listed), and EGL's device enumeration (`eglQueryDevicesEXT`, `eglQueryDeviceStringEXT`,
`eglGetPlatformDisplay`) added to the EGL crate; its folder cannot be `crates/opengl`, which is the Gallium driver. A7
is the Vulkan API from vk.xml (Apache-2.0), a front end of its own: handles, structures chained by `pNext` (a raw pointer
the default-deny rule refuses unless a line names each chain), and `map_memory` lent until `unmap_memory`. Waits on
nothing but this track's time.

## Questions for the orchestrator

None open. Both of the last run's were ruled and followed (section 0.3).

## Security

Lines naming `unsafe` (the root manifest's count, comments included), a2f3a88d0 -> c6339e7a7 (MEASURED, claim above):
COM direct 95 -> 95; d3dcommon direct 63 -> 63; DXGI direct 2,548 -> 2,548; Direct3D 11 direct 7,081 (+ its separate C
caller 4) -> 7,115 (the C caller's 4 moved in as its integration test, plus tests/exact's marked calls into exact.c);
Direct3D 12 direct 826 -> 784 (the refused GPU-address and descriptor-address methods carry no marks). Direct3D 11
hostile and recorder 1 each: the line `#![forbid(unsafe_code)]` of their tests, not code. Every interface, recorder,
hostile and conformance crate: 0 `unsafe` code, under the workspace's `forbid`. Marks (`expect(unsafe_code`) in direct
crates: 953. The fronts unchanged: Direct3D 11 front 10, Direct3D 12 front 503, directx 1,077. Every `unsafe` is a
marked single operation in an `implementation/direct` crate (or its own tests) with its own deny table.

New dependencies: none. `cc` (already in the lock) is now an optional build-dependency of the Direct3D 11 direct crate,
used only under its `c-caller` feature; the test crates it names as dev-dependencies are this set's own.

Hostile suites run (MEASURED, fresh AddressSanitizer target): the C caller over every table of the five APIs, the exact C
tests both ways, Direct3D 11's hostile set's lies and 3 x 10M fuzzing, the probes; 0 reports. The repository's
hostile-guest suite is not this track's and was not run.
