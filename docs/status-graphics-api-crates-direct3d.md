# Status: the graphics API crates, Direct3D part

Brief: wcontainers-scratch/orchestrator/restart-2026-11-25/brief-graphics-api-crates.md, with the orchestrator's run of
2026-11-27 (this run is the Direct3D half of the API track; the Gallium half stays on `graphics-api-crates`). Branch
`graphics-api-crates-direct3d`, made from f3af6417f. Marks: MEASURED (a run; its job and log named), READ, INFERRED.
Logs: wcontainers-scratch/graphics-api-crates/d3d/. Every job ran on the x86-64 machine with `--jobs 12`.

His main: `git fetch` of his checkout's main gives ac696ea19, already inside f3af6417f (`git rev-list --count
HEAD..FETCH_HEAD` 0, MEASURED): nothing to merge. No interface file of his is touched by any commit of this branch.

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
- The hostile C caller is a workspace of its own (b4b1dd679): the repository's members are `crates/*/implementation/*`,
  one level, and the root manifest is his.

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
