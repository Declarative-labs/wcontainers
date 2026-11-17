# Status: graphics, shaders track (G6)

Branch `phase-2-graphics-shaders`. Plan: `docs/phase-2-graphics.md`, sections 6, 7, 8 and G6 in 12.

## Where things stand

Milestone G6a (first slice), run 1, in progress.

Done:
- naga carried in the tree with the GLSL writer changes (below). Other tracks get it by merging this
  branch: the workspace `Cargo.toml` patches `naga` to `third_party/naga`.
- `crates/wcontainers-shaders` exists; its test `tests/glsl_writer.rs` checks the writer changes.

## naga as carried

`third_party/naga` is naga 30.0.2 exactly as published on crates.io (commit "carried naga 30.0.2 in
the tree as published"), plus these changes, each a later commit touching only `third_party/naga`:
- GLSL writer, `BuiltIn::VertexIndex` without `WriterFlags::DRAW_PARAMETERS` is written
  `(uint(gl_VertexID) + naga_vs_base_vertex)`, with `uniform uint naga_vs_base_vertex;` declared in
  vertex shaders that read it (`back::glsl::BASE_VERTEX_BINDING`, feature `VERTEX_INDEX`), as naga
  already does for `instance_index` with `naga_vs_first_instance`. The back end sets it to each indexed
  draw's base vertex (0 otherwise); GL leaves an unset uniform 0. With `DRAW_PARAMETERS` nothing
  changes (desktop GL's `gl_VertexID` already includes the base vertex).
- GLSL writer, `BuiltIn::SampleIndex` is written `uint(gl_SampleID)` (it is a `u32` in naga and an
  `int` in GLSL; 6.3's corpus had one shader failing on it).
Not yet sent upstream.

## How to run

- Linux laptop (no GPU), one command from the Mac: `tools/graphics-rig/linux <command>`, for example
  `tools/graphics-rig/linux cargo test -p wcontainers-shaders`. It copies the working tree (submodules
  included) to `~/wcontainers-graphics/src/<branch>` and builds into `~/wcontainers-graphics/target/<branch>`,
  with 5 cargo jobs (2 when the laptop's load is 8 or more).
- The laptop already had rustup and the 1.99.0 toolchain (installed by another track); gcc 9.4 and
  clang 10 from Ubuntu 20.04.

## Departures from the plan

- naga is carried as a copy in the tree (`third_party/naga`, patched in by the workspace), not as a fork
  by git revision (6.2): a fork needs a new repository on the owner's account, and this track commits
  only to its own branch. Moving to a git revision later changes one line of the workspace manifest.

## Interface changes

None.
