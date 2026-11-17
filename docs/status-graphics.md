# Graphics track status

Plan: `docs/phase-2-graphics.md`. Brief: `pyro-parked/wcontainers-scratch/orchestrator/brief-graphics.md`.

## Done

- G0, run 1 (2026-11-17): the plan, and `crates/wcontainers-gpu` with the interface as real Rust: the
  `Backend` trait (base, fast paths with defaults, present-or-absent methods), the capability list
  (`Features`, `Limits`, `FormatCaps`), the state blocks, binding sets, passes, draws, write modes, read
  backs, surfaces, errors and device loss, and the named slow paths with counters. A trivial recording
  back end in the unit tests shows the defaults built from the base (fans, loops with restart,
  read back now, clear texture) and that absent features say which.
- Shader experiments for the plan's decisions, all in scratch (plan section 6.3 has the results).

## Partly done

- Nothing in the code is partial for G0. Designed in the plan but added at G1, not now: the
  `generate_mips` and `write_buffer_ring` fast paths and the shared pass tracker for implicit fronts.

## Next

- G1 to G6 as in plan section 12; they can run in parallel. G1 (interface complete and recording back
  end) is short and every other milestone builds its tests on it, so start it first.

## How to run

- Interface crate: `nix develop -c cargo test -p wcontainers-gpu` (7 tests) and
  `nix develop -c cargo clippy -p wcontainers-gpu --all-targets -- -D warnings`. It also builds for
  `wasm32-unknown-unknown`.
- Shader corpus (scratch, `/Users/thomaseaton/Documents/GitHub/pyro-parked/wcontainers-scratch/graphics`):
  - dxbc-spirv built with `nix shell nixpkgs#meson nixpkgs#ninja nixpkgs#clang -c sh -c 'meson setup
    build/dxbc-spirv -Denable_tools=true -Dbuildtype=release && ninja -C build/dxbc-spirv'` from
    `src/dxbc-spirv`. The scratch copy of `tools/dxbc_compiler.cpp` was changed to drop the float-control
    modes (round and denorm modes set to none), as plan 6.3 says.
  - dxil-spirv built with CMake and Ninja from `src/dxil-spirv` into `build/dxil-spirv`.
  - Corpus: `python3 -I tools/extract_dxbc.py downloads/wine-tests/d3d11.c corpus/dxbc`, then
    `build/dxbc-spirv/tools/dxbc_compiler X.dxbc --spv corpus/spv2/X.spv` per file.
  - Naga and Chrome verdicts: from the checkout,
    `nix develop -c <scratch>/tools/run-corpus.sh <scratch>/corpus/spv2 <scratch>/corpus/out-via-spv2
    <scratch>/corpus/dxbc-via-spv` writes `corpus/dxbc-via-spv-naga.txt` and `-chrome.txt`.
  - DXIL: HLSL in `corpus/hlsl`, DXC from `nix build nixpkgs#directx-shader-compiler`, dxil-spirv with
    `--no-bda --ssbo-uav --ssbo-srv` (plus `--bindless` for the SM 6.6 shader), then `run-corpus.sh` on
    `corpus/dxil-spv`.

## Measurements

- No timings were taken in G0. The shader verdicts (counts only) were taken on the Mac (Apple M5) on
  2026-11-17 with load average 12.51, 20.81, 20.69 when the builds started; Chrome as installed, headless,
  through the flake's Playwright. Results: plan section 6.3, files in `corpus/`.

## Dependencies added and their licences

- `naga` 30.0.2 (MIT OR Apache-2.0), in the workspace dependencies, used by `wcontainers-gpu`. Its tree,
  from `cargo tree`: all MIT or Apache-2.0 except `foldhash` (Zlib), `zerocopy` (BSD-2-Clause OR
  Apache-2.0 OR MIT), `unicode-ident` ((MIT OR Apache-2.0) AND Unicode-3.0), `codespan-reporting`
  (Apache-2.0), `libm` (MIT).
- Read in their repositories, not yet dependencies: dxbc-spirv MIT (2025 Philip Rebohle), dxil-spirv MIT
  (Hans-Kristian Arntzen for Valve), Fossilize MIT (2018 Hans-Kristian Arntzen; not used).
- Test-only, in scratch, never shipped: Wine's d3d11 test source (LGPL-2.1) as DXBC test data; DXC
  (NCSA) to make DXIL test shaders.

## Departures from the plan's brief

- Writing WGSL and GLSL ES 3.00 from the common form happens in each back end (with naga's writers and
  that language's fixes), not in `wcontainers-shaders` as the brief's crate list says. Reason: back ends
  depend only on the interface crate, and the writing is the back end's own language. The shaders crate
  converts into the common form, lowers extra stages, and owns the cache.
- Fronts depend on the interface plus the two shared helper crates (`wcontainers-shaders`,
  `wcontainers-directx-graphics-infrastructure`); "depend only on the interface crate" is read as "never
  on each other or on a back end".
- Geometry, hull and domain stages and stream output are lowered by the shaders crate's stage helper and
  reach back ends as ordinary dispatches and draws; the interface has no method for them.
- DXIL reaches the common form through dxil-spirv's SPIR-V and naga's SPIR-V reader, which needs fixes
  (made in a naga fork pinned by revision until upstream takes them).
- The Direct3D fronts answer occlusion queries with 0 or 1 (counted, warned), because neither browser API
  counts samples and Direct3D has no way to say so. This is an inexact answer the owner should know about.
- The workspace root `Cargo.toml` gained the `naga` workspace dependency (outside the crates this track
  owns, needed to add one).

## Known failures

- None in the code.
