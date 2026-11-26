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

Known gaps, in the order they are next:
1. The trait-layer exception lines above (Map's lent memory as the top trait's `Mapped: VolatileBytes` type with a
   shadow copy in the serving glue; initial data lengths; GetBuffer's buffer kind; a foreign IUnknown).
2. Direct3D 11's immediate context is not free threaded: the calling glue must turn on the system's multithread
   protection at device creation (ID3D11Multithread), as the spike did. Until then `SystemDeviceContext`'s `Send`
   and `Sync` rest on the table promise alone: not reachable on Linux, where only our own serving glue is called.
3. A non-null `HMODULE` reaches the system unchecked through the calling glue (the system reads it as a module).
4. The spike's harness on the layout (C fake d3d11, correct and hostile C games, the liar, the attack tests), the
   fuzzing of 4 modes x 3 seeds x 10 million calls under AddressSanitizer, and the cost table.
5. The linter's shape findings above: whether a direct crate may hold public items outside an interface, and
   whether the byte forms' offsets need names.

## Scope added 2026-11-25 (the orchestrator's ruling)

This track owns every outside graphics API crate, including the OpenGL ES and EGL calls of the WebGL native route
(glow over ANGLE, crates/gpu-webgl) and the Mesa compiler calls of the shader path (crates/nir's bridge). They are
milestones AC10 (OpenGL ES and EGL) and AC11 (the Mesa compiler), after Direct3D 12. The implementations track uses
these traits and writes no glue.
