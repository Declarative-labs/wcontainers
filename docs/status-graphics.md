# Graphics track status (interface, G1)

Plan: `docs/phase-2-graphics.md`. Brief: `pyro-parked/wcontainers-scratch/orchestrator/brief-graphics.md`.
This branch, phase-2-graphics, is the shared line: the interface track merges the other graphics
branches into it at the start and end of every run.

## Where things stand

- G0 is done (eight runs and eight independent checks on 2026-11-17; their detail is in the git log
  and the plan): the plan, and `crates/wcontainers-gpu` with the interface as real Rust.
- G1a, run 1 (2026-11-17), done and pushed:
  - Every open finding of G0's last check fixed in the plan and the crate (list below).
  - The Linux build loop, `tools/graphics-rig/linux`.
  - `crates/wcontainers-gpu-test`: the recording back end (`Recorder`) and the first seven tests of
    the shared suite, which any back end runs with one macro line (How to run).

## G0's open findings, and how each was settled (plan section, crate item)

- Blocking, Direct3D 10 and 11's core requirements the back ends have only optionally (plan 4.12's new
  bullet, 4.11, 9; `Features` docs, `SlowPath`, `VariantFlags`, `HelperValue::DepthRange`):
  - clip distances: `ClipDistanceInShader` (distance over w interpolated linearly, discard, or the
    sample mask from derivatives on multisampled targets), `VariantFlags::CLIP_DISTANCE`;
  - cull distances: `CullDistanceAsGeometry` (the stage helper drops primitives); the plan's "Mesa
    lowers them to discards" corrected (5.4);
  - dual-source blending: `BlendInShader`;
  - depth clip off: `DepthClampInShader`, `VariantFlags::DEPTH_CLAMP`, `HelperValue::DepthRange`;
  - `SV_PrimitiveID` in pixel shaders: `PrimitiveIdAsGeometry`, `VariantFlags::PRIMITIVE_ID_FROM_STAGE`;
  - `D32_FLOAT_S8X24_UINT`: a condition of 10_0 and above on WebGPU (depth32float-stencil8), so an
    adapter without it reports 9_3; core on WebGL 2;
  - indirect-first-instance: `IndirectReadBack` without it.
- Blocking, resolve without its format: `Backend::resolve(src, dst, format)`; WebGPU exact through
  views, WebGL 2's other-twin resolve is the listed loss `ResolveInStoredFormat` (plan 4.2, 4.12, 5.1).
- `generate_mips` through the sRGB twin of a typeless texture: the default reads and writes through
  twin views (plan 4.3; test `generate_mips_through_the_srgb_twin_reads_and_writes_srgb_views`).
- Layout key for helper-set textures below binding 32: load-only textures take
  `"unfilterable-float"` from the program and need no bit (plan 4.5; `BindingLayoutKey` doc).
- Buffer-texture copies at pitches WebGPU refuses, and buffers into depth: `RowsRepacked`, and
  `DepthWrittenByDraw` through an `r32float` texture (plan 4.2; `CopyBufferTexture::row_pitch`,
  `copy_buffer_to_texture` docs).
- Texture UAV clears of views served through a texel buffer: `clear_buffer` of that buffer with the
  encoded texel (plan 5.1).
- Staging read backs start at the copy into the staging resource, not at `Map(READ)` (plan 4.6, 5.1).
- Stride-zero vertex streams: every vertex reads one element; WebGL 2 uses an instance divisor of
  2^31 (plan 4.4; `VertexStream::stride` doc; the unused-stream doc now says `None`).
- 93 slow path reasons (six new). 32 tests in `wcontainers-gpu`.

## Interface changes

- Commit c60a829 ("interface: gave resolve its format, ..."): `Backend::resolve` takes a `Format`
  (a base method's signature: back ends add the argument); new `SlowPath` variants
  `ClipDistanceInShader`, `DepthClampInShader`, `CullDistanceAsGeometry`, `PrimitiveIdAsGeometry`,
  `RowsRepacked`, `ResolveInStoredFormat`; `VariantFlags::CLIP_DISTANCE`, `DEPTH_CLAMP`,
  `PRIMITIVE_ID_FROM_STAGE`; `HelperValue::DepthRange`; `generate_mips` accepts the sRGB twin as
  `values`. The rest is documentation. No other track had changed the crate yet.

## The recording back end and the suite

- `Recorder` keeps every object's bytes on the CPU and runs each call when it is recorded. It checks
  the interface's rules it meets (draws inside passes, copies and read backs outside, attachments of
  one size, depth aspects matching the format, views in the texture's format or its twin, objects
  used after `destroy`, read backs finished before they are ready, queries) and records each broken
  rule in `violations`, losing the device; every base call is logged in `calls`.
- It draws: programs in the common form run in an interpreter of naga's IR (`program.rs`;
  arithmetic, vectors, matrices, structs, locals, branches, loops, uniform and read-only storage
  buffers, texture samples and loads); triangles are clipped to the depth range, snapped to 1/256
  pixel, filled by pixel centres and the top-left rule, depth and stencil tested, blended. What it
  does not do yet is an error naming it and device loss, never a wrong answer: points and lines,
  multisampling (only one sample is declared), cube sampling, implicit level of detail over more than
  one level (needs derivatives), storage writes, compute, indirect draws, alpha to coverage.
- Its `Caps` declare only what it does: held formats are the 8, 16 and 32-bit colour formats and the
  depth and stencil formats; no BC, no storage, one sample. No interpreter crate for naga's IR
  exists, so `program.rs` is our own.
- Suite tests (`crates/wcontainers-gpu-test/src/suite.rs`), each generic over `Host`:
  `buffer_writes_land_in_order_and_read_back`, `texture_writes_land_and_read_back`,
  `copies_move_bytes_exactly` (includes an unaligned buffer copy, a copy within one buffer and a
  12-byte row pitch, which WebGPU must repack), `clears_fill_exactly`,
  `triangle_covers_the_pixels_whose_centres_it_holds` (with a culled back face),
  `textured_draw_with_depth_and_blending`, `present_shows_the_frame` (checks the shown texels where the
  host can see them: `Host::shown` returns None otherwise).

## Next

- G1b: the pass tracker (plan 4.7, with attachment fitting and the usage-scope split), the occlusion
  counter (4.12), the combination tracker (4.4); the recording back end's remaining rules (usage
  scope, write ordering across passes) and a structured call stream that replays identically; the
  seeded random call-sequence generator (100,000 cases, count recorded); every fast-path default shown
  built from the base by a recorded stream; more suite tests (indexed draws and strips with restart,
  instancing, stencil, every format's clear and read back, mip levels, layered and 3D targets,
  queries, fans through `draw_fan_or_loop`).
- The independent check of G1a.

## How to run

- Linux loop (CPU only): `tools/graphics-rig/linux <command>` sends the working tree to
  `~/wcontainers-graphics/src/phase-2-graphics` on tom@192.168.2.240 (rsync, ignored files left out)
  and runs the command there with `CARGO_TARGET_DIR=~/wcontainers-graphics/target/phase-2-graphics`,
  5 cargo jobs (2 when that machine's load average is above 8), at nice 10. Examples:
  - `tools/graphics-rig/linux cargo test -p wcontainers-gpu -p wcontainers-gpu-test`
  - `tools/graphics-rig/linux sh -c 'cargo clippy -p wcontainers-gpu -p wcontainers-gpu-test --all-targets -- -D warnings'`
  - `tools/graphics-rig/linux sh -c 'RUSTDOCFLAGS="-D warnings" cargo doc -p wcontainers-gpu -p wcontainers-gpu-test --no-deps'`
  - Rust there: rustup with the 1.99.0 toolchain the repository names, already installed by
    another track; this run installed nothing.
- A back end runs the suite with a test file holding a `Host` implementation and
  `wcontainers_gpu_test::suite!(MyHost);` (see `crates/wcontainers-gpu-test/tests/recorder.rs`).
- On the Mac only formatting: `nix develop -c cargo fmt -p wcontainers-gpu -p wcontainers-gpu-test`.
- G0's corpus and Chrome tools are described in the plan, section 6.3, and the git history of this
  file (G0 run 8).

## Measurements

- G1a took no timings. Loads when the tests ran: Linux laptop 0.4 to 1.3 (its own load average,
  printed by the loop).
- Suite on the recording back end: 7 of 7 pass; recorder rule tests 5 of 5; `wcontainers-gpu` 32 of 32.
- The suite catches a wrong back end: three faults put into the recording back end one at a time
  (script `g1a-breaks/break.py` in scratch, results in `g1a-breaks/results.txt`), each reverted after:
  writes and reads ignoring a region's x origin failed `texture_writes_land_and_read_back` and
  `copies_move_bytes_exactly`; front faces taken as clockwise failed
  `triangle_covers_the_pixels_whose_centres_it_holds`; `Less` compared as greater failed
  `textured_draw_with_depth_and_blending`. Linux laptop load 1.6 to 2.7.
- The whole workspace checks on Linux (`cargo check --workspace --all-targets`, 4 m 48 s at load 1.6
  to 4.0 with 5 jobs, after the rig merge).
- G0's shader corpus counts are in the plan, section 6.3.

## Dependencies added and their licences

- `wcontainers-gpu-test` uses `naga` 30.0.2 (`wgsl-in`), `blake3` 1 and `half` 2 (MIT OR Apache-2.0,
  its `Cargo.toml`), all already in the workspace's lock file through other crates.
- Earlier: `naga` 30.0.2 (MIT OR Apache-2.0) and `blake3` 1.8.7 (CC0-1.0 OR Apache-2.0 OR Apache-2.0
  WITH LLVM-exception) in `wcontainers-gpu`; the licences of what later milestones add are in plan 3.

## Departures from the plan

- From G0 (each is written in the plan): back ends write WGSL and GLSL themselves; fronts use the two
  shared helper crates; extra stages lowered by the shaders crate; DXIL through dxil-spirv and a naga
  fork; targets not a block; 11_0, not the top level, on WebGPU; multisample counts per format and the
  video abilities fall short of Microsoft's tables; WebGL 2's multisampled copies and fitted
  attachments keep resolved values; base vertex through a naga fork change; WebGL 2 without
  OES_sample_variables reports no multisampling; EXT_color_buffer_float a 10_0 condition; listed
  precision losses (denormals, clip depth without EXT_clip_control, 24-bit depth stored as float,
  fractional viewports on multisampled WebGL 2 targets, line rasterisation rule); counted refusals
  `BindingsBeyondLimit` and `VaryingsBeyondLimit`; the `wcontainers-directx` crate; reusable bytes
  copied once by the stand-in; the Direct3D family as one module per back end; variants chosen by
  fronts; several queues ordered into one; late memory failures as device loss; the usage-scope rule
  on every back end; the root `Cargo.toml` gained `naga`.
- G1a: `D32_FLOAT_S8X24_UINT` without depth32float-stencil8 lowers the reported level to 9_3 rather
  than being emulated (no exact route; the feature's share of adapters is unmeasured). On WebGL 2 a
  resolve in the other sRGB twin is a listed loss (`ResolveInStoredFormat`). The recording back end
  draws (plan 11 asks only that it check rules and record), because the suite must pass on it and
  fail on a back end that is wrong.

## Known failures

- None.
