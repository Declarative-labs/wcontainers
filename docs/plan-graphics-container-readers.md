# Plan: graphics container readers

From the library audit of 2026-11-21: the Direct3D shader container (DXBC) is read by hand in four
places and its checksum is written five times. This track leaves one reader and one checksum, in
the shaders crate, and points the others at it. Status: docs/status-graphics-container-readers.md.

## C1: one DXBC container reader

Today:
- crates/wcontainers-shaders/src/container.rs: chunk lookup, signatures of every form, shader model,
  program type, the hull and domain control point scan, the DXIL PSV0 fields. Private to its crate.
- crates/wcontainers-direct3d-11/src/shader.rs: its own chunk walk, signature reader and declaration
  scan (typed views, multisampled arrays, indexed constant buffers).
- crates/wcontainers-direct3d-12/src/dxbc.rs: its own chunk lookup and declaration scan (typed views
  and multisampled arrays placed by space through the root signature, unbounded arrays).
- crates/wcontainers-direct3d-12/src/root.rs: its own RTS0 lookup and the checksum.
- crates/wcontainers-shaders/tests/fuzz.rs: the checksum again.

The one reader, in container.rs, made public:
- the chunk list: each chunk's tag, size and contents, with what is cut short reported, so each
  caller keeps its own answer to a malformed container (the fronts and the root signature reader
  refuse with E_INVALIDARG as before, the converters skip as before);
- signature elements of every form (ISGN, OSGN, OSG5, ISG1, OSG1), with the component type the
  Direct3D 11 front needs;
- the instructions of a program chunk and where a declaration's operand indices start: the part of
  the two fronts' declaration scans that truly agrees;
- `checksum`, on md-5's MD5 core with DXBC's own last block.

What stays apart, and why: the two fronts' declaration scans mean different things. Direct3D 11
numbers views by register and reads constant buffer declarations; Direct3D 12 places shader model
5.1 views by space through the root signature's ranges and sizes unbounded arrays. Each keeps its
own reading of the declarations it needs, on the shared instruction walk. Root signature reading
and writing stays in the Direct3D 12 crate; only its RTS0 lookup and checksum move.

dxbc-spirv: not used to back the reader. Its container and signature classes are C++ behind the
converter's boundary; reaching them from the two fronts and the root signature reader would add
extern functions and copies across the boundary for a reader of about 150 lines, so it would add
more of our code than it removes.

Order, to collide least with the shaders and Direct3D 11 tracks: the shaders crate's container.rs
in one small commit (made public, the shared pieces and `checksum`), then Direct3D 12 (dxbc.rs,
root.rs, dxil.rs), then the fuzz test, then the Direct3D 11 reader last. Each step: format, clippy
with warnings denied, the affected tests, the literal check, then commit.

Behaviour: same outputs for every well-formed container. Where the old readers disagreed on
malformed bytes, the cases are listed in the status file with one example each, and a seeded
comparison of old and new over the rig's, the shaders crate's own and mutated containers shows
where they differ.

Pass bar: one reader and one checksum in the tree; every shader, Direct3D 11 and Direct3D 12 test
that ran before passes; the fuzz test passes with the shared checksum.

## C2: one DXIL resource-list reader

Today crates/wcontainers-direct3d-12/src/dxil.rs reads the DXIL module's resource list itself, on the
llvm-bitcode crate, two or three times per pipeline stage (typed views and multisampled arrays for
the stage's key, typed views again for a pixel or compute stage). The shaders crate reads the same
list through dxil-spirv's module reader (cpp/dxil.cpp, `each_resource`).

Planned: one function on the shaders side (`container::dxil_views`, through one new C function
beside `each_resource`) giving each shader resource and unordered access view's shape, space,
register range and element value kind in one call; the Direct3D 12 front reads it once per stage
and keeps only its own numbering of slots; its bitcode reader and the llvm-bitcode dependency go.

Pass bar: the Direct3D 12 tests pass; pipeline creation on the same shaders is not slower, measured
before and after. If dxil-spirv's reader cannot meet it, the item stops, our reader stays and the
status file gives the measurement.
