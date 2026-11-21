# Status: graphics, NIR route (G8) and OpenGL natively (G9)

Branch phase-2-graphics-nir. Plan: docs/phase-2-graphics.md, sections 15.2, 15.3, 15.4 and 15.8 (G8,
G9).

## Where it stands

G9 run 10 (2026-11-20). G9 is still short of its bar. The driver reports OpenGL ES 3.0 and GLSL
1.30; ES 3.1 and 3.2 and desktop 3.3 and 4.6 cannot start (no compute dispatch, storage buffers or
images, geometry or tessellation stages in the context), so five of the nine lists run no case.

The table of lists and shares on our driver, WHOLE lists (desktop, RTX 3080, one GPU job, 8 glcts
at once, other tracks' jobs queued beside), at 9a5baaf7 (run B, 18:02 to 18:15):
- dEQP-GLES2: 17,165 cases; Pass 16,925, Warn 6, Fail 57, Crash 42, Skip 135: 16,931 of 17,030
  run, 99.42%.
- dEQP-GLES3: 44,768 cases; Pass 43,796, Warn 5, Fail 245, Crash 188, Flake 1, Skip 533: 43,801
  of 44,235 run, 99.02%.
- KHR-GLES2: 473 cases; Pass 438, Skip 35: 438 of 438, 100.00%.
- KHR-GLES3: 6,498 cases; Pass 6,107, Fail 87, Crash 73, Skip 231: 6,107 of 6,267, 97.45%.
- dEQP-GLES31, KHR-GLES32, KHR-GL33, KHR-GL46: cannot start (deqp-runner gets no context of their
  version). KHR-GLES31: its 4,101 cases are reported Missing (no context), counted as run and
  failed.
- Over the four lists that start: 67,277 of 67,970 run, 98.98% (run 9's best: 94.68%). The bar's
  measure is over the GLES lists and KHR-GL33 together, five of which run nothing: not met.
- Flakes (failures by the measure, listed): run B dEQP-GLES3.functional.flush_finish.wait, run A
  (at a6f887ee) dEQP-GLES3.functional.flush_finish.flush; both time GL calls against each other
  and passed when run again; not yet read.
- Built after run B and measured on its groups alone (desktop, same queue), ad51f14f: occlusion
  queries ended now start their read back at the next flush; dEQP-GLES3's occlusion_query group
  went from 62 Fail to all passing ("failed to return a result after glFinish"), and
  transform_feedback has no Fail left. Not in the table above.
- Run A at a6f887ee (17:40 to 17:56, same machine and queue): dEQP-GLES2 99.40%, dEQP-GLES3
  98.30%, KHR-GLES2 100%, KHR-GLES3 97.45%; over the four 66,956 of 67,970, 98.51%.
- Results: scratch `results/g9r10/full-a` and `full-b` (`<list>/results.csv`, the runner's logs of
  dEQP-GLES2, -GLES3 and KHR-GLES3 in `<list>/logs`, the job's output `g9r10-full-*.log`); the
  group runs in `results/g9r10/groups-1` to `groups-4`.

Run 9's "regressions" were not regressions: every one was a cascade. One case's wgpu validation
error (a sampler's level-of-detail bounds, an HLSL compile failure, a partial depth or stencil
write) marked the device lost and every later case of that glcts process read nothing back;
deqp-runner groups cases into processes by where the crashes fall, so the cascades moved between
the runs. The driver now ends the process when the back end reports its device lost (the
fault stays with its case), and the faults behind them are fixed (below) or named.

Done this run (each from the failing cases' logs):
- The driver ends the process, naming the loss, when the back end reports its device lost (after
  each submit and on a failed read back): `context::end_if_lost`.
- Uniform blocks are sized at the block index the loads name (the linker's, in the variable's
  `driver_location`), not at `data.binding`, which OpenGL ES 3.0 leaves at zero for every block;
  and by the linker's explicit layout, not by std140's rules on the type, which read row-major
  matrices as column-major. dEQP-GLES3 ubo 291 crashes and KHR-GLES3 uniform_block 115 crashes to
  none.
- Part of a depth or stencil level is written through a buffer and the interface's
  `copy_buffer_to_texture` (which draws such regions on WebGPU), not `write_texture` (whose
  stencil aspect wgpu refuses unless whole: "Partial copy ... Depth24PlusStencil8").
- A sampler's level-of-detail bounds past the deepest level any texture can have read as that
  level (the back end clamped the maximum to 32 and not the minimum of 100 or 1000).
- An output of another kind than its target reaches it as its bits, not converted (st/mesa's
  scissored integer clears and the blitter's integer copies hand integers through float
  outputs): fbo.blit 188 and fbo.color 60 Fail to none.
- Stream output counts only draws of at least one whole primitive (the stage helper writes no
  statistics for none, and the stale count was read), and streams the program's own position,
  before the depth range rewrite and flips (`ClipDepthRewrite` was streamed): transform_feedback
  648 Fail to none.
- An integer cube map is loaded from a 2D array view of its six faces: the bridge makes its
  variable a 2D array, picks the face and texel by OpenGL's cube map table, the level by
  OpenGL's nearest rule (from the face coordinates' derivatives), and the driver binds the view
  of the faces (`View::faces`). dEQP-GLES3's 486 refusals to none.
- Route B reads `fcanonicalize` as the value itself (Mesa's algebraic step does the same where
  denormals are not flushed; the host keeps denormals).
- The texture units the screen reports fit the sampler and texture binding ranges (16): the
  adapter's own limit was larger, and texture.units refused "more resources of a kind than its
  range holds" (100 logs).
- The literal check: 7 findings, the same as run 9 (none new).
- Not a fix: run 9's first item (bisect bc5c0da1, 15cbc920, cd5162cf) is closed by the above;
  bc5c0da1's std140 term was wrong for row-major matrices and is gone, 15cbc920's maximum stays,
  cd5162cf had no effect on the regressions.

Every failure left in run B, with its reason (dEQP-GLES3 unless named):
- fbo.render 109 Fail: recreate_color 70, shared_colorbuffer 26, resize 10, stencil_clear 3
  (dEQP-GLES2's fbo.render 35 are the same, with STENCIL_INDEX8). The draw after the colour
  buffer is re-specified (with the framebuffer unbound) does not show: in dEQP-GLES2's
  no_rebind_rbo_rgb565_stencil_index8 the flat blue quad drawn before the depth test is missing
  entirely. Not yet found; next to read (case alone, the pass tracker's targets around the
  framebuffer rebind).
- Fetched vertex components across words (shaders track's fetch): vertex_arrays 83 crashes, and
  dEQP-GLES2's 37 (76 logs).
- shaders.texture_functions 45 crashes: 3D textures with constant offsets, which naga's HLSL
  writer writes as `int2(int3(...))` and DXC refuses ("too many elements in vector
  initialization"; 40 logs): naga's, asked of the shaders track. And comparisons with a bias or
  gradients (34 logs) and at a level other than 0 (16), not taken by route B yet.
- shaders.builtin_functions 26 Fail and shaders.invariance 20 (and dEQP-GLES2's 20): precision
  and invariance; not yet read.
- fbo.msaa 18, fbo.invalidate 12, fbo.blit 8 crashes: "a blit Gallium's blitter does not make"
  (76 logs) and depth-stencil resolves.
- Vertex formats the screen does not report (R10G10B10A2 SSCALED, USCALED, SNORM; 84 logs):
  OpenGL ES 3.0 requires them as vertex formats; to fetch in the vertex program.
- transform_feedback.point_size 6 crashes ("a streamed value that is neither at a location nor
  the position": gl_PointSize), random 5 crashes.
- multisample.fbo_* 9 Fail, fbo.completeness 3, state_query.floats 4 and
  implementation_limits.max_texture_lod_bias 1 (the bias limit), rasterization.fbo 2,
  fragment_ops.scissor 2, negative_api.vertex_array 1, draw.random 5, shaders.random 3,
  shaders.builtin_variable 2 (a stage value of Mesa's slots 24 and 25), texture.specification 3:
  not yet read.
- KHR-GLES3: copy_tex_image_conversions 140 (forbidden 70 Fail: glCopyTexImage from an RGB565
  cube face gives GL_INVALID_FRAMEBUFFER_OPERATION where GL_INVALID_OPERATION is expected, so the
  framebuffer holding the 565 texture is incomplete: the 565 format is sampled but not rendered;
  required 63 Crash: "sampling an integer texture" still refused in these programs, 126 logs,
  not yet traced); packed_depth_stencil 8 crashes and framebuffer_blit 3 (the blitter);
  packed_pixels 6 Fail; shader_integer_mix 3 Fail.
- dEQP-GLES2: polygon_offset 2 Fail, an array of arrays of textures (4 logs), shaders.random 4,
  uniform_api.random 2: not yet read.
- None of these is checked yet against the RTX 3080's own OpenGL (`-Driver system`), which the
  bar asks for every failure.

What is left, in order:
1. fbo.render's re-specified colour buffers (above): about 145 cases over the two lists.
2. The integer sampling still refused in KHR-GLES3 copy_tex_image_conversions.required (63), and
   the 565 framebuffer that is incomplete (70): both one cause each, to trace.
3. Run the whole lists again at ad51f14f (or later) for the table; run every failing group with
   `-Driver system` and list which the RTX 3080's own OpenGL passes.
4. Vertex formats of 10-10-10-2 and scaled integers fetched in the vertex program; comparisons
   with a bias, gradients or a level; the blits the blitter does not make; gl_PointSize streamed.
5. ES 3.1 (compute dispatch, storage buffers and images, indirect draws, the 3.10 shading
   language) for dEQP-GLES31 and KHR-GLES31; then 3.2, desktop 3.3 (geometry shaders) and 4.6.
6. Reach 92.9% over the GLES lists and KHR-GL33 together, and 98.6% over KHR-GL46.
7. bgfx's and Godot's OpenGL renderers on the rig, with every slow path counted.
8. Check whether u_vbuf converts vertex formats the screen does not report on the CPU without
   being counted.
9. Known gap: a strip drawn without restart from an index buffer whose indices hold the largest
   value is not detected (no case has been seen to hit it). Known gap: an array of integer cube
   samplers whose units have samplers of different wraps (one with a border) is retyped whole
   while the driver binds the faces view only where a unit's sampler is listed (no case seen).

Earlier runs (G8 runs 1 to 15, G9 runs 1 to 9) are told in this file's history:
`git show 054465ac:docs/status-graphics-nir.md` held runs up to G9 run 8 in full, and
`git show 8d7f5472:docs/status-graphics-nir.md` held G9 run 9.

## How to run

- The corpus: `crates/wcontainers-nir/corpus/fetch` (into `$CARGO_TARGET_DIR/nir-corpus`), then with
  `WCONTAINERS_MESA` set: `WC_NIR_CORPUS=$CARGO_TARGET_DIR/nir-corpus WC_NIR_RESULTS=<file> cargo
  test -p wcontainers-nir --test corpus`. From the Mac: `orchestrator/cloud/x86-run <checkout> --name
  graphics-nir --jobs 64 -- 'crates/wcontainers-nir/corpus/fetch && export
  WCONTAINERS_MESA=$CARGO_TARGET_DIR/mesa/build && WC_NIR_CORPUS=$CARGO_TARGET_DIR/nir-corpus
  WC_NIR_RESULTS=$CARGO_TARGET_DIR/nir-results-a.jsonl cargo test -p wcontainers-nir --test corpus'`,
  then `x86-run --fetch graphics-nir target/nir-results-a.jsonl <scratch>`. 1005 shaders take about
  2 minutes with the comparison (192 at once). Run 3's results are in
  `pyro-parked/wcontainers-scratch/graphics-nir/results/run3/nir-results-a.jsonl` (run 2's, conversion
  only, one folder up); run 4's in `results/run4/` (`nir-results-r4b.jsonl` the last, with the
  patch; `nir-choice-r4.jsonl` the timed one); `scripts/reasons.py <results> [n]` counts the reasons.
- The choice between the candidates, one command: `crates/wcontainers-nir/corpus/choose [results]`
  (fetches the corpus, builds optimised, runs one shader and route at a time on ten cores; its last
  lines give each route's counts and its median and mean time on the shaders every route matches).
  From the Mac: `x86-run <checkout> --name graphics-nir -- 'WC_NIR_TIMING_CORES=0-9
  crates/wcontainers-nir/corpus/choose $CARGO_TARGET_DIR/nir-choice.jsonl'` (the whole corpus of 18,222 items took 4 h 38 min in run 11; most of it is Zink's compiler
  aborting on GLSL in route A, each abort a core dump the machine's `apport` writes).
  `WC_NIR_SHADERS=<n>` takes a seeded sample: for the independent check,
  `WC_NIR_TIMING_CORES=0-9 WC_NIR_SHADERS=2000 crates/wcontainers-nir/corpus/choose
  $CARGO_TARGET_DIR/nir-choice-check.jsonl` (run 15: about 1,850 s at load 54 to 98; B 1,288 match,
  A 119; run `corpus/cts-programs` first, or the sample is drawn from fewer items). A reference
  that does not end on the seeded inputs is held on up to four seeds, two minutes each, so a
  sample with such programs takes longer.
- The licence of every linked Mesa file: `crates/wcontainers-nir/mesa/linked-licences [file]` (after
  the Mesa build; the route test linked with a map, each kept object traced to its source).

- Stopping this track's own CTS run on the desktop (a run on a build known to be broken, which
  holds the GPU queue for everyone): scratch `scripts/stop-cts.ps1`, copied to
  `C:\wc\graphics-nir\stop-cts.ps1` and run with `powershell -NoProfile -ExecutionPolicy Bypass
  -File`; it stops only processes running this track's run-windows.ps1, deqp-runner and glcts.
- One group alone (G9 run 7): add `-Include <regex>[,<regex>]` to `run-windows.ps1`, e.g.
  `-Lists gles2-main -Include dEQP-GLES2.functional.vertex_arrays.*`. The refusals of a whole
  run, from the Mac: `ssh -o BatchMode=yes tom@192.168.2.183 'findstr /C:"refused:" /C:"not
  built:" C:\wc\graphics-nir\cts\results\wcontainers\<list>\*.log'`.
- Reading a run's failures (G9 run 6): copy the list's folder back (`scp -o BatchMode=yes -r
  tom@192.168.2.183:C:/wc/graphics-nir/cts/results/wcontainers/<list> <scratch>`), then
  `scripts/qpa_images.py <case>.qpa` (scratch) writes each picture of a case's log as a PNG, and
  `scripts/crash_reasons.py <folder> <runner output>` lists each failed case with the driver's
  refusals in its process's log (a process runs many cases: a refusal is the crash's own, a
  validation error may be an earlier case's).
- Build Mesa for Windows (G9) on the desktop, from a checkout on the Mac: `tools/graphics-rig/remote
  'powershell -NoProfile -ExecutionPolicy Bypass -File crates\wcontainers-nir\mesa\build-windows.ps1'`.
  Tools, source, build and install under `C:\wc\graphics-nir` (kept between runs; the tools'
  downloads with their SHA-256 printed); `opengl32.dll` and `libgallium_wgl.dll` in
  `C:\wc\graphics-nir\mesa\install\bin`. Uses half the desktop's threads. The first build
  after G9 run 5 also builds llvmpipe's LLVM from the 21.1.8 source into
  `tools\llvm-21.1.8-mt` (X86 only, static runtime, libraries, headers and llvm-config; the
  source and build folders deleted after). A remote command waits
  while another track's hazard job holds the GPU slot. Kept between runs on the desktop for G9:
  `C:\wc\graphics-nir`, 4.8 GB at the end of G9 run 1 (the unpacked tools, LLVM most of it; the
  Mesa source, build and install); the downloaded archives were deleted (fetched again only if a
  tool is missing).
- The conformance suite on the desktop (G9): build once with `tools/graphics-rig/remote
  'powershell -NoProfile -ExecutionPolicy Bypass -File crates\wcontainers-opengl\cts\build-windows.ps1'`
  (glcts and deqp-runner; about 40 minutes the first time, at 8 jobs beside other tracks'
  builds). Run lists as a GPU job: `tools/graphics-rig/gpu-job --desktop --build 'if not exist
  "%CARGO_TARGET_DIR%" mkdir "%CARGO_TARGET_DIR%"' -- 'powershell -NoProfile -ExecutionPolicy
  Bypass -File crates\wcontainers-opengl\cts\run-windows.ps1 -Driver <wcontainers|llvmpipe|system>
  [-Lists gles2-main,gl46-main] [-Fraction 100 -FractionStart 1]'` (a list's name is its file's
  name without `.txt`). It prints each list's counts and share; deqp-runner's `results.csv`,
  `failures.csv` and logs are in `C:\wc\graphics-nir\cts\results\<driver>\<list>`. One case
  alone, for a hang or a crash: `tools/graphics-rig/gpu-job --desktop --build 'echo sent' --
  'powershell -NoProfile -ExecutionPolicy Bypass -File
  crates\wcontainers-opengl\cts\run-case-windows.ps1 -Driver wcontainers -List gles2-khr-main
  -Case <case> [-Seconds 40]'` (glcts's output, exit code and log printed; killed and reported
  as hung after the limit). Add `-Debugger` to run it under the LLVM release's lldb (with Python
  3.10 from NuGet, fetched once): every thread's stack and the stopped thread's stack words as
  symbols, read from the PDB files the build keeps (`buildtype = 'debugoptimized'`, no
  assertions). The release's folder had to be opened to the local Users group once
  (`build-windows.ps1` now does it when it unpacks LLVM), as the hidden desktop's user could
  not run lldb. Linting the driver on the desktop: `tools/graphics-rig/remote 'set
  WCONTAINERS_MESA=C:\wc\graphics-nir\mesa\build&& set RUSTFLAGS=-C target-feature=+crt-static&&
  set LIBCLANG_PATH=C:\wc\graphics-nir\tools\clang+llvm-21.1.8-x86_64-pc-windows-msvc\bin&& cargo
  clippy -q -p wcontainers-opengl --release --target-dir C:\wc\graphics-nir\opengl-target -- -D
  warnings'`; on the x86 machine with `WCONTAINERS_MESA` set and `LIBCLANG_PATH=$(nix build
  --no-link --print-out-paths --inputs-from path:/work/flake/graphics-nir
  nixpkgs#llvmPackages.libclang.lib)/lib`. Logs of
  run 2 in scratch `logs/g9r2-*.log`, its `results.csv` files in `results/g9r2/cts/<driver>/<list>`. The Mesa build (above) must have run first for
  `wcontainers` and `llvmpipe`. Kept on the desktop after run 3 (reused by every build): `cts` 3.5 GB, `mesa` 0.7 GB, `opengl-target` 2.4 GB (now with wcontainers-nir and the shaders crate's C++), `tools` 3.9 GB; the run copy's submodules are checked out by the build. After run 2: `C:\wc\graphics-nir\cts`
  3.8 GB (the suite's source and build), `tools` 3.9 GB (with deqp-runner), `mesa` 0.8 GB,
  `opengl-target` 0.8 GB (the driver's cargo output, reused by each Mesa build).
- Build Mesa, from the Mac: `orchestrator/cloud/x86-run <checkout> --name graphics-nir --jobs 64 --
  'crates/wcontainers-nir/mesa/build path:/work/flake/graphics-nir'`. On any Linux machine with Nix:
  `crates/wcontainers-nir/mesa/build` (its argument, the flake, defaults to the checkout). The tools
  come from Nixpkgs' own `mesa` package (`nix print-dev-env --inputs-from <flake> nixpkgs#mesa`), so the
  repository's flake is unchanged and the tools are those of its locked nixpkgs. Output in
  `$CARGO_TARGET_DIR/mesa/{src,build,install}` (target/mesa without it). The script refuses a tag whose
  commit is not the pinned one, makes the build folder again when the commit or the options change
  (Meson reads a machine file only when a folder is made), and fails unless llvmpipe and lavapipe were
  built.
- GLSL tests: `cargo test -p wcontainers-nir --test route --test glsl_buffers --test
  glsl_reference` (with `WCONTAINERS_MESA`, on the x86 machine). The corpus run takes piglit's
  `.shader_test` files beside the `.spv` files, from the same `WC_NIR_CORPUS`; all 3546 take about
  4 minutes at 192 at once. `scripts/glsl_reasons.py <results> [route] [n]` (scratch) counts
  piglit's reasons; `scripts/piglit_outcomes.py <results> [route] [n] [prefix]` counts every
  outcome of the files under a prefix (`piglit/`, `bgfx/`). bgfx's shaders come with the same
  `corpus/fetch` and run in the same corpus run (any `.bin` in a folder named `glsl`).
- Test with Mesa (on the x86 machine, after the build):
  `export WCONTAINERS_MESA=$CARGO_TARGET_DIR/mesa/build; cargo test -p wcontainers-nir` and
  `cargo clippy -p wcontainers-nir --all-targets -- -D warnings`; and both again without the variable.
- dEQP's cases come with the same `corpus/fetch` (VK-GL-CTS's shader library data, sparse) and run
  in the same corpus run: 13,630 items take about 35 s alone (`WC_NIR_CORPUS` at
  `nir-corpus/vk-gl-cts`). `scripts/deqp_differences.py <results>` (scratch) groups route B's
  differences by file and message.
- The Vulkan CTS's programs: `crates/wcontainers-nir/corpus/cts-programs [flake]` (from the Mac:
  `x86-run <checkout> --name graphics-nir --jobs 96 -- 'crates/wcontainers-nir/corpus/cts-programs
  path:/work/flake/graphics-nir'`); the corpus run takes them from `nir-corpus` like the rest.
- The literal check needs the git history, which the rented machine's copy lacks. Run 6 ran it on
  the x86 machine from a git bundle of the branch (`git bundle create <scratch>/nir.bundle
  phase-2-graphics-nir`, sent with `x86-run --ssh 'cat > /work/probes/graphics-nir.bundle'`,
  cloned there, `cargo fetch`, then the check in `nix develop`; the clone and its build deleted
  after). Runs 1 to 5 ran it on the Mac (`RAYON_NUM_THREADS=3 cargo run -j 4 -q -p
  wcontainers-literals -- check`). With
  `crates/wcontainers-shaders/dxbc-spirv`'s own submodule checked out the check fails on its Python
  scripts (the baseline holds it not checked out): fetch only the shaders crate's two submodules and
  dxil-spirv's `third_party/spirv-headers`, which the shaders crate's build needs.

- A quick look at one shader: copy it alone into a folder and point `WC_NIR_CORPUS` there with
  `WC_NIR_DUMP` set; `nix shell nixpkgs#spirv-tools -c spirv-dis` reads the dumps.
- Run 8 fetched VK-GL-CTS's shader library data (sparse, at 5bb4c7e8) and a sparse bgfx on the
  x86 machine under `/work/probes` to read their formats; both deleted at the end of the run.
- Runs beside the timed choice: pin to `taskset -c 10-95,106-191` (cores 0-9 and their hyperthread
  siblings 96-105 are the choice's), and never build `--release` in `/work/target/graphics-nir`
  while it runs. Probe programs (a changed copy of one corpus program, compiled with Nixpkgs'
  glslang as `corpus/godot-forward-plus` does) go in a folder of their own under the build folder,
  pointed at by `WC_NIR_CORPUS`. `scripts/outcome_changes.py <old> <new> <route>` (scratch) lists
  the programs whose outcome changed between two results files.
- Kept between runs: a sparse clone of Dolphin's `Source/Core/VideoCommon` at 32a2b828 for reading
  how it makes its shaders (`pyro-parked/wcontainers-scratch/graphics-nir/downloads/dolphin`,
  GPL-2.0-or-later: read only, nothing of it copied or paraphrased here), and bgfx's two ASSAO
  sources and their `uniforms.sh` (`downloads/bgfx-src`, BSD-2-Clause).
- Kept between runs: a sparse clone of Godot at 73ca6be2 for reading its shader formats and their
  C++ users (`pyro-parked/wcontainers-scratch/graphics-nir/downloads/godot`, 12 MB, blobs fetched
  on demand: never `git grep` its history there, which fetches every blob; add paths with `git
  sparse-checkout add /<path>`).
- Godot's RenderingDevice shaders, one command after the fetch's sources: `crates/wcontainers-nir/corpus/godot-forward-plus [flake]`
  (from the Mac: `x86-run <checkout> --name graphics-nir --jobs 64 --
  'crates/wcontainers-nir/corpus/godot-forward-plus path:/work/flake/graphics-nir'`; 9 s). It
  fetches, writes the variants' sources (the corpus binary's `--godot-rd`, no Mesa needed) and
  compiles them with Nixpkgs' glslang into `nir-corpus/godot-forward-plus`, listing what glslang
  refuses in `glslang-refused`. Run it (and `corpus/cts-programs`) before `corpus/choose`, which
  fetches only `corpus/sources`. Alone: `WC_NIR_CORPUS=$CARGO_TARGET_DIR/nir-corpus/godot-forward-plus`.
- Godot's shaders come with the same `corpus/fetch` and run in the same corpus run (any `.glsl`
  under `drivers/gles3/shaders`); alone: `WC_NIR_CORPUS=$CARGO_TARGET_DIR/nir-corpus/godot`.
- Kept between runs: a sparse clone of VK-GL-CTS at 5bb4c7e8 for reading dEQP's formats
  (`pyro-parked/wcontainers-scratch/graphics-nir/downloads/vkglcts`, no build output); on the x86
  machine, the CTS source and build under `/work/target/graphics-nir/vk-gl-cts` (with Mesa and the
  rest, 18 GB in all after run 9).
- Kept between runs: a shallow clone of the pinned Mesa for reading on the Mac
  (`pyro-parked/wcontainers-scratch/graphics-nir/mesa-src`, 560 MB, no build output; logs beside it in
  `logs/`), and the Mesa build and cargo target on the x86 machine (`/work/target/graphics-nir`).
  No scratch copy of the checkout was made on the Mac, the laptop or the desktop. The literal check's
  own build in the checkout's `target` (282 MB) is deleted at the end of each run. On the x86
  machine `/work/target/graphics-nir` (11 GB after run 4: Mesa, the corpus, cargo's debug and release output, the last results) is kept.

## Measurements

- G9 run 10, the whole lists on our driver (desktop, RTX 3080, one GPU job each, 8 glcts at
  once, other tracks' jobs queued beside): run A 17:40:35 to 17:56:43 (about 16 minutes with the
  build; dEQP-GLES2 about 3, dEQP-GLES3 about 11), run B 18:02:36 to 18:15:19 (about 13). Group
  runs alone (`-Include`): ubo and texture, 8,252 cases, about 6 minutes; transform_feedback,
  ubo.random and fbo.blit and color, 2,588 cases, about 3.
- G9 run 10, wcontainers-nir's tests with Mesa and clippy (x86-64 machine, load 3 to 19): fmt
  clean; clippy with warnings denied clean for wcontainers-nir and wcontainers-opengl with and
  without Mesa; with Mesa `glsl_buffers` 1, `glsl_reference` 10 and 2 ignored (the known
  failures), `narrow` 2, `route` 11 passed. The corpus run was not repeated (route B's changes:
  `fcanonicalize` read as a move, and the driver's own passes, which the corpus does not take).

- G9 run 9, the whole corpus run (12,748 items of route B's count, 96 children, x86-64 machine,
  load 15.7 at the end; Mesa's build kept from earlier runs): 742 s. Route B: 12,090 match, 156
  differ, 6 not compared (run 1 of G9: 12,068, 156, 6): the 22 more matches are programs the
  uniform block sizing and `b2b32` now take; no new difference. Time per shader to the validated
  common form at 96 at once on the 1,072 shaders both routes match: A median 422 us, mean 650 us;
  B median 343 us, mean 411 us. Results: scratch `results/g9r9/nir-results-g9r9.jsonl`, log
  `logs/g9r9-corpus.log`.
- G9 run 9, wcontainers-nir's tests with Mesa (x86-64 machine, load under 3): `glsl_buffers` 1,
  `glsl_reference` 10 and 2 ignored (the known failures), `narrow` 2, `route` 11 passed; clippy
  with warnings denied clean for wcontainers-nir and wcontainers-opengl with Mesa.
- G9 run 9, the four GLES lists on our driver (desktop, RTX 3080, one GPU job, 8 glcts at once,
  other tracks' jobs queued beside): 16:04:42 to 16:31, about 27 minutes for 68,904 cases.

- G9 run 7, wcontainers-nir's suite (x86-64 machine, load 1 to 5): fmt clean; clippy with
  warnings denied clean with and without Mesa (wcontainers-nir and wcontainers-opengl without it);
  with Mesa `glsl_buffers` 1, `glsl_reference` 10 and 2 ignored (the known failures), `narrow` 2,
  `route` 11 passed. The driver's clippy with warnings denied clean on the desktop with Mesa. The
  corpus run was not repeated (route B's changes this run: the shared NIR step's texture
  lowerings, which the GLSL corpus front already made before it).
- G9 run 7, the whole dEQP-GLES2 and KHR-GLES2 lists on our driver (desktop, RTX 3080, one GPU
  job each, 8 glcts at once, other tracks' jobs queued beside): 14:39 to 14:45 (with the build),
  14:57:12 to 15:01:49 for the last (4 min 37 s with the incremental build). `vertex_arrays`
  alone (571 cases): 10 s.

- The whole suite of wcontainers-nir and wcontainers-opengl (G9 run 6, x86-64 machine, load 3
  to 7): fmt clean; clippy with warnings denied clean with and without Mesa; with Mesa
  `glsl_buffers` 1, `glsl_reference` 10 and 2 ignored (the known failures), `narrow` 2, `route`
  11 passed, and the driver's doc tests none (bindgen no longer copies Mesa's comments, whose C
  examples rustdoc ran as Rust and failed: 2 failures before); without Mesa every target builds,
  no tests run. The corpus run was not repeated this run (route B's one change, the sample level
  outside comparisons, is held by the conformance runs; the corpus run is the next run's).
- The whole dEQP-GLES2 and KHR-GLES2 lists on our driver (G9 run 6, desktop, RTX 3080, one GPU
  job, 8 glcts processes at once, other tracks' jobs queued beside; the desktop's processors at
  8% just after): 17,638 cases in 6 min 45 s (14:00:07 to 14:06:52, with the job's start). A 1%
  shard of both (177 cases): about 35 s; a 10% shard (1,765 cases): 71 s.
- The Windows Mesa build with the driver, incremental after a driver change (desktop, 8 jobs,
  shared): about 1 to 2 minutes, most of it cargo's release build of the driver.

- The whole corpus run (G9 run 1, 18,738 items, 96 children, x86-64 machine, load 10.9 at the
  start and 2.2 at the end; a new spot machine, Mesa and the corpus built again first): 740 s
  (12 min 20 s). Route B: 12,068 match, 156 differ, 518 failed by name, 6 not compared (run 15:
  12,069, 154, 519, 6); the three changes are piglit's `fs-shl-of-shr-int64` (was "not taken:
  umul_high", now matches) and two Slang vertex programs of Sascha Willems' samples
  (`occlusionquery/simple.vert`, `subpasses/gbuffer.vert`) whose outputs the route drops, now
  differences by the new vertex check: each stores a load of a local the program never writes
  (checked in the disassembly), an undefined value, listed with the undefined-behaviour
  differences. Route A unchanged (1,075 match, 7 differ, 147 not compared). Time per shader to
  the validated common form at 96 at once on the 1,072 shaders both routes match: A median
  448 us, mean 710 us; B median 358 us, mean 460 us. Results: scratch
  `results/g9r1/nir-results-g9r1.jsonl`.
- The whole suite of wcontainers-nir (G9 run 1, x86-64 machine, load 2 to 15): clippy with
  warnings denied clean with and without Mesa; without Mesa every target builds (no tests run);
  with Mesa `glsl_buffers` 1, `glsl_reference` 10 and 2 ignored (the known llvmpipe failure and
  the negated NaN comparisons), `narrow` 2 (2,097,152 16-bit cases and 65,600 multiplications,
  0 differ), `route` 11 passed. `corpus/cts-programs` from nothing: about 400 s at `-j 96`.

- The whole corpus run (run 15, 18,738 items, 172 children on cores 10-95 and 106-191, x86-64
  machine, load 41.1 at the start and 100.4 at the end, other tracks' jobs and the timed sample
  beside it): 735 s. Time per shader to the validated common form at 172 at once on the 1,074
  shaders both routes match: A median 536 us, mean 863 us; B median 428 us, mean 556 us (B's time
  no longer includes the cache key, which neither route's now does).
- The whole suite of wcontainers-nir (run 15, x86-64 machine, load 84 to 115): fmt clean; clippy
  with warnings denied clean with and without Mesa; without Mesa every test target builds and
  passes (no tests run without it); with Mesa `glsl_buffers` 1, `glsl_reference` 10 and 1
  ignored (the known failure, llvmpipe's), `narrow` 1, `route` 11 passed.

- The whole corpus run (run 14, 18,222 items, 180 children, x86-64 machine, load 11.4 at the
  start and 3.3 at the end): 731 s. Time per shader to the validated common form at 180 at once
  on the 1,012 shaders both routes match: A median 519 us, mean 790 us; B median 448 us, mean
  564 us.
- The whole corpus run (run 13, 18,222 items, 172 children on cores 10-95 and 106-191, x86-64
  machine, load 14.3 at the start and 3.7 at the end, the timed choice on cores 0-9 beside it):
  732 s, up from 251 s: the references that do not end are now run on up to four seeds, two
  minutes each, one after another. Time per shader to the validated common form at 172 at once on
  the 998 shaders both routes match: A median 542 us, mean 831 us; B median 429 us, mean 519 us.
- The whole suite of wcontainers-nir (run 14, x86-64 machine, load 10.3 to 11.2): without Mesa
  every test target builds and passes (no tests run without it); with Mesa `glsl_buffers` 1,
  `glsl_reference` 9 and 1 ignored (the known failure), `narrow` 1, `route` 11 passed. Clippy with
  warnings denied clean with and without Mesa; the literal check exits 0 (from a bundle of the
  branch on the x86 machine, as run 6).
- The whole suite of wcontainers-nir (run 13, x86-64 machine, load about 14): without Mesa every
  test target builds and passes (no tests run without it); with Mesa `glsl_buffers` 1,
  `glsl_reference` 7, `narrow` 1, `route` 11 passed.

- The whole corpus run (run 12, 18,222 items, 172 children on cores 10-95 and 106-191, x86-64
  machine, load 6.0 at the start and 13.3 at the end, the timed choice running on cores 0-9 and
  other tracks' jobs beside): 251 s. Time per shader to the validated common form at 172 at once
  on the 991 shaders both routes match: A median 454 us, mean 714 us; B median 358 us, mean
  440 us. The same run before the subroutine change (load 87 at the start, 27 at the end): 253 s.
- The whole suite of wcontainers-nir (run 12, x86-64 machine, load about 6): without Mesa every
  test target builds and passes (no tests run without it); with Mesa `glsl_buffers` 1, `glsl_reference` 7,
  `narrow` 1, `route` 11 passed.
- The whole corpus run (run 11, 18,222 items, 192 children, x86-64 machine, load 10.0 at the
  start and 26.7 at the end, another run of ours just before): 250 s. Time per shader to the
  validated common form at 192 at once on the 991 shaders both routes match: A median 488 us, mean
  767 us; B median 381 us, mean 481 us (busy machine; the choice's times are the ten-core ones).
  `corpus/godot-forward-plus` (fetch, the sources, 572 glslang compilations at 192 at once): 9 s,
  load 1.0. Godot's RenderingDevice programs alone: 16 s, load 0.8 to 1.7.
- The whole corpus run (run 10, 17,650 items, 192 children, x86-64 machine, load 8.4 at the start
  and 10.8 at the end): 248 s. Time per shader to the validated common form at 192 at once on the
  748 shaders both routes match: A median 421 us, mean 607 us; B median 346 us, mean 406 us (busy
  machine; the choice's times are the ten-core ones). Godot's 241 items alone: 128 to 134 s (most
  of it the one program whose reference does not end, held to the 120 s limit).

- `corpus/cts-programs` from nothing (VK-GL-CTS clone, its `fetch_sources.py`, CMake and Ninja
  for `vk-build-programs`, the alpha_to_one programs), x86-64 machine, `-j 96`, load 5.2 at the
  start and 10.4 at the end: 320 s wall, 76 min user.
- dEQP's 13,630 items alone, 192 children, x86-64 machine, load 0.1 to 32.6: about 32 s.

- Mesa 26.3.1 clean build (configure, 1239 steps, install), x86-64 machine (c7i.metal-48xl, 192
  threads), `-j 64`: 45.9 s wall, 11 min 37 s user, load average 4.96 at the start and 10.51 at the
  end (other tracks' jobs). An earlier clean build of the same options with SPIRV-Tools: 45.9 s wall
  at load 0.43 at the start.
- Corpus run with the comparison, 1005 shaders, 192 children, x86-64 machine at load 0.3 rising to
  5.3: 124 s including an incremental build (run 3).
- The corpus run with bgfx (run 8, 3871 files, x86-64 machine, 192 children, load 4.5 at the start
  and 16.1 at the end): 250 s. At 192 at once, on the 735 shaders both routes match: A median
  441 us, B median 352 us (a busy machine; the choice's times are the ten-core ones).
- The whole suite with the SPIR-V corpus and piglit compared (run 7, x86-64 machine, 192 children,
  load 4.9 at the start and 13.2 at the end): the corpus run 245 s for 3546 files. Time per shader
  to the validated common form at 192 at once, on the 728 shaders both match: A median 431 us, B
  median 341 us (busy machine; the choice's times are the ten-core ones below).
- The whole suite with the SPIR-V corpus compared (run 6, after the GLSL work; x86-64 machine,
  192 children, load 1.0 at the start and 12.5 at the end, 245 s with the build;
  `results/run6/nir-results-r6.jsonl`): the same counts as runs 4 and 5, A 728 match, B 773
  match, 0 differ, 8 not compared on each.
- `tests/glsl_buffers.rs` and `tests/route.rs` (run 6): 0.08 s and 0.01 s, x86-64 machine, load
  0.5 to 4.2.
- `tests/narrow.rs` (2,097,152 invocations, both programs on lavapipe): 0.47 s, x86-64 machine,
  load 0.08 (run 5).
- The choice's timing (run 4, `corpus/choose`, x86-64 machine, ten cores, one child at a time, load
  5.5 at the start and 3.2 at the end with another track's tests running): 1781 s in all; time per
  shader to the validated common form on the 725 shaders both match: A median 219 us, mean 261 us;
  B median 107 us, mean 125 us. The same at 192 children at once (load 9 to 25): A median 383 us, B
  median 196 us.

## Routed decisions (15.9, owner not yet answered; followed as the orchestrator set them)

- Our own OpenGL driver on the interface, as 15.2 recommends. gex_wgpu and wvk are not adopted; nothing of
  theirs is copied or paraphrased (their repository has no licence; a related project is AGPL-3.0).
- Crate and module names are the plan's suggestions: `wcontainers-nir`, `wcontainers-opengl`.
- Mesa under its MIT licence with the permissive files 15.2 lists; dEQP, VK-GL-CTS and piglit for testing only.
- No change to the interface crate: what the second batch needs from it (15.7) is written below as requests.

## Licences brought in (for the owner to confirm)

- G9 run 10: nothing new. The driver now also calls Mesa's `nir_fixup_deref_types` and the GLSL
  type constructors (`glsl_sampler_type`, `glsl_texture_type`, `glsl_type_wrap_in_arrays`), all
  in Mesa's MIT `src/compiler` files already linked.

- G9 run 9: nothing new from outside. Three more of Mesa's MIT files are called by the driver:
  `src/gallium/auxiliary/indices/u_indices.c` and the `u_indices_gen.c` Mesa's build makes from
  `u_indices_gen.py` (both VMware, MIT: the index translator and generator), and
  `src/gallium/auxiliary/util/u_prim_restart.c` (VMware, MIT: the restart splitter). All are in
  `libgallium`, which the Windows build already links.

- G9 run 8: nothing new from outside. Two more of Mesa's MIT files are called by the driver:
  `src/gallium/auxiliary/nir/tgsi_to_nir.c` (Broadcom and Rob Clark, MIT) and, through the
  bridge, `src/compiler/nir/nir_xfb_info.h`'s data (MIT). wcontainers-opengl now depends on
  wcontainers-shaders (ours) directly.

- G9 run 6: the DirectX Shader Compiler's Windows release v1.9.2609 (`dxcompiler.dll`,
  `dxil.dll`), fetched by `mesa/build-windows.ps1` onto the desktop only and placed beside the
  test driver; not in this repository. Its LICENSE.TXT is the University of Illinois/NCSA Open
  Source License (permissive) with notices for its LLVM-derived parts; to confirm from the
  archive's own files before anything ships with it. The `cc` crate (MIT or Apache-2.0, already
  a build dependency of wcontainers-nir) is now also a build dependency of wcontainers-opengl.

- Mesa 26.3.1 (fetched by the build scripts, not in this repository). Its `docs/license.rst` says
  each file's SPDX tag governs; the core is MIT. Linked into wcontainers-nir's tests: Mesa's static
  libraries `libzink`, `libglsl_standalone`, `libglsl`, `libglcpp`, `libglsl_util`,
  `libglcpp_standalone`, `libmesa`, `libvtn`, `libnir`, `libcompiler`, `libmesa_util`, `libblake3`
  and `libmesa_util_c11` (the linker keeps only the objects used). The files kept, listed again in
  G9 run 1 by `mesa/linked-licences` (scratch `results/g9r1/nir-linked-licences-g9r1.txt`): 242
  files. 196 carry the MIT permission text and 34 the SPDX tag MIT; 2 are generated by Mesa's
  build from MIT scripts (`nir_opcodes.c`, `nir_opt_algebraic.c`); 1 is BSL-1.0
  (`c11/impl/threads_posix.c`); 9 carry no licence text in the file: the 7 of `util/blake3`
  (BLAKE3 1.8.2 copied verbatim, its README says; upstream's licence is CC0-1.0 OR Apache-2.0 OR
  Apache-2.0 WITH LLVM-exception), `util/softfloat.c` (Berkeley SoftFloat 3e, BSD-3-Clause text
  in the file, which the script's MIT match does not count) and `util/format/u_format_etc.c` (MIT
  text in the header it includes, `texcompress_etc_tmp.h`). Against run 6's list (243 files) the
  one change is `nir_lower_fragcolor.c`, no longer linked (the bridge writes that step itself).
  Four of the MIT files are generated and carry a second notice: `glsl_parser.cpp` and
  `glcpp-parse.c` are Bison's output (Bison's skeleton is GPL-3.0-or-later with the Bison special
  exception, which lets the output be distributed under any terms), `glsl_lexer.cpp` and
  `glcpp-lex.c` flex's (flex's own BSD-style licence). All permissive. The Windows build
  (`mesa/build-windows.ps1`, G9) links nothing into anything of ours yet; G9 lists the driver's
  linked files when there is one.
- G9 run 2, wcontainers-opengl's dependencies: `pollster` 1 (Apache-2.0 OR MIT), `wgpu` 30.0.2
  (MIT OR Apache-2.0, the workspace's patched copy), `wcontainers-gpu` and `wcontainers-gpu-webgpu`
  (ours); build-dependencies `cc` (MIT OR Apache-2.0), `serde` and `serde_json` (MIT OR
  Apache-2.0). All already in the lock file.
- G9 run 3, wcontainers-opengl: `windows-sys` 0.61.2 (MIT OR Apache-2.0, already in the lock
  file, Windows only), `wcontainers-nir` (ours, without `standalone`: its bridge, route B and
  through it naga and wcontainers-shaders, as listed above), and the build-dependency `bindgen`
  0.71.1 (BSD-3-Clause, already in the lock file for unicorn-engine-sys) in place of `cc`.
  bindgen runs libclang at build time only (on the desktop, the LLVM 21.1.8 release the Mesa
  build already fetches, Apache-2.0 WITH LLVM-exception; on Linux Nixpkgs' clang 21.1.8);
  nothing of it is linked. The driver now links Gallium's `u_init_pipe_screen_caps`
  (`u_screen.c`, MIT) from Mesa's own library.
- G9 run 4, wcontainers-opengl: `naga` (the workspace's, MIT OR Apache-2.0, already a
  dependency of wcontainers-nir and the interface) as a direct dependency (the uniform blocks'
  declared sizes). The driver now also calls Mesa's `u_upload_mgr.c`, `u_blitter.c`,
  `u_framebuffer.c` and `util/format/u_format.c` (each MIT by its header) from Mesa's own
  library, which the WGL target links anyway; no new file of Mesa's is built.
- Testing only, on the Windows desktop (`crates/wcontainers-opengl/cts`, never linked into
  anything of ours): VK-GL-CTS `opengl-cts-4.6.8.1` (Apache-2.0, its `LICENSE`) with what its
  `fetch_sources.py` pins (glslang BSD-3-Clause and others, SPIRV-Tools and SPIRV-Headers
  Apache-2.0 and MIT, Amber Apache-2.0, jsoncpp MIT, zlib Zlib, libpng libpng-2.0, Vulkan-Docs and
  the video samples Apache-2.0 and MIT), and deqp-runner v0.23.3 (MIT, as 15.2 says; its own
  crates from crates.io, built by `cargo install --locked`).
- zlib (Zlib licence), linked as Mesa's build found it (Nixpkgs' static zlib).
- `serde_json` 1 and `serde` 1 (both MIT OR Apache-2.0), build- and dev-dependencies of
  wcontainers-nir; `cc` (MIT OR Apache-2.0), already in the workspace.
- `blake3` 1 (CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception, already in the workspace), a
  dependency and build-dependency (the cache key and the converter's version).
- `wcontainers-gpu` (ours), a dependency (`SlotKind`, `ReflectedBinding`, `ShaderStage`).
- `glow` 0.18 (MIT OR Apache-2.0) and `khronos-egl` 6 with `dynamic` (MIT OR Apache-2.0),
  dev-dependencies since run 7 (llvmpipe in the corpus run); both already in the workspace through
  the WebGL back end.
- `ash` 0.38 (MIT OR Apache-2.0) and `libloading` 0.8 (ISC), dev-dependencies (lavapipe in the
  corpus run); both already in the lock file through wgpu.
- `spirv` 0.4 (Apache-2.0; the SPIR-V grammar's names, already in the workspace through naga), a
  dependency. `rand` 0.9 and `rand_chacha` 0.9 (MIT OR Apache-2.0, already in the workspace),
  dev-dependencies.
- `glam` 0.30.10 (MIT OR Apache-2.0), a dev-dependency since run 15 (new to the lock file): the
  GLSL comparison's seeded fixed-function matrices, their product and inverses.
- Corpus, test data only, fetched into the build folder and never in the repository or shipped:
  VK-GL-CTS at 5bb4c7e8 (since run 9; Apache-2.0, its `LICENSE`; only `data/gles2/shaders`,
  `data/gles3/shaders`, `data/gles31/shaders` and the licence are fetched by `corpus/fetch`;
  `corpus/cts-programs` fetches the whole tree and the dependencies its `fetch_sources.py` pins:
  glslang (BSD-3-Clause and others), SPIRV-Tools and SPIRV-Headers (Apache-2.0 and MIT), Amber
  (Apache-2.0), jsoncpp (MIT), Vulkan-Docs and the video parser (Apache-2.0 and MIT), built only to
  dump test programs, never linked into anything of ours);
  bgfx at e76ecedf (since run 8; BSD-2-Clause, its `LICENSE`; only `examples/runtime/shaders/glsl`
  and the licence are fetched);
  Godot at 73ca6be2 (since run 10; MIT, its `LICENSE.txt`; only `drivers/gles3/shaders`,
  `LICENSE.txt` and `COPYRIGHT.txt` are fetched; `COPYRIGHT.txt` names no other licence for those
  files, and `tonemap_inc.glsl` notes an ACES fit adapted from MJP's BakingLab, MIT);
  since run 11 also Godot's `servers/rendering/renderer_rd/shaders` (MIT) and the AMD FSR and FSR2
  code it includes (`thirdparty/amd-fsr`, `thirdparty/amd-fsr2`: Expat, that is MIT, by Godot's
  `COPYRIGHT.txt`; their own `license.txt` and `LICENSE.txt` fetched beside them), compiled by
  glslang (Nixpkgs' `glslang`, BSD-3-Clause and others; a tool run by `corpus/godot-forward-plus`,
  never linked);
  Sascha Willems' Vulkan samples at d2d49bf7 (MIT, `LICENSE.md`); piglit at 92510281 (since run 7;
  its `COPYING`: MIT for most of it, with some test shaders under their projects' own licences,
  e.g. GStreamer's in `glslparsertest`; piglit is for testing only, as the routed decision says,
  and never a dependency).
- Build tools only, on the Windows desktop (G9, `mesa/build-windows.ps1`, never linked into
  anything of ours; nothing installed, all under `C:\wc\graphics-nir\tools`): Python 3.13.1 from
  its NuGet package (PSF-2.0), Meson 1.12.1 (Apache-2.0), Ninja 1.13.0 (Apache-2.0), Mako 1.3.10
  (MIT), PyYAML 6.0.2 (MIT), packaging 25.0 (Apache-2.0 OR BSD-2-Clause), win_flex_bison 2.5.25
  (flex BSD-2-Clause; bison GPL-3.0-or-later, a generator whose output carries Bison's exception,
  as GCC's Bison on Linux), LLVM 21.1.8 for the Visual C++ runtime (Apache-2.0 WITH
  LLVM-exception; since G9 run 5 its release only gives bindgen's libclang and lldb, and
  llvmpipe links the same version built there from its source, the reference only; the
  release's `LLVMSupport.lib` holds rpmalloc, public domain or MIT, never linked now), Python
  3.10.11 from its NuGet package (PSF-2.0; only for lldb, which loads it, for debugging a
  case), and Visual Studio 2022 Build Tools, already on the desktop (Microsoft's licence).
- Build tools only, from the repository's locked nixpkgs: Meson 1.12.1 (Apache-2.0), Ninja (Apache-2.0),
  LLVM 21.1.8 (Apache-2.0 WITH LLVM-exception; llvmpipe and lavapipe use it, wcontainers-nir does not
  link it), GCC 16.2.0 (GPL-3.0 with the GCC runtime library exception).

## Departures from the plan

- G9 run 10: an integer cube map is sampled through a 2D array view of its faces, loaded at the
  texel OpenGL's cube map table and nearest rule pick, in a program variant keyed on the bound
  samplers (WebGPU loads no cube texel and gathers only the base level). Its level of detail comes
  from the face coordinates' derivatives, as OpenGL's does.
- G9 run 10: the driver ends the process when the back end reports its device lost; a context of
  OpenGL ES 3.0 without robustness has no way to tell a program, and going on would make every
  later call read nothing.
- G9 run 10: route B reads NIR's `fcanonicalize` as the value itself.

- G9 run 5: llvmpipe's LLVM on Windows is built from LLVM's source on the desktop rather than
  taken from LLVM's release (the plan names no source for it; the release's integrated allocator
  corrupted Mesa's heap, Where it stands). The Windows Mesa build keeps debug information
  (`debugoptimized`, assertions still off as in a release build), for the stacks of crashes; its
  speed is not what G9 measures.

- G9 run 1: the Windows build of Mesa is a script beside the Linux one
  (`crates/wcontainers-nir/mesa/build-windows.ps1`), sharing its pin and patches, not a step of
  the rig's setup (the rig's scripts are another track's). It fetches its own tools into
  `C:\wc\graphics-nir\tools` rather than installing them system-wide. llvmpipe is built with
  LLVM's own Windows release (static runtime), not an LLVM built here.

- G9 run 2: `wcontainers_screen_create` is in the crate's C bridge, not in a C file of Mesa's
  driver folder (15.2): the folder holds only its meson file and the public header, so every line
  of the driver's code is in this repository. The library reaches Mesa's link through one meson
  option, `wcontainers-link-args`, set after cargo builds it (the native libraries are known only
  then), not through a path Mesa's meson file computes.
- G9 run 2: each list runs with the first configuration its `mustpass.xml` gives it (for the
  KHR lists, 64 by 64 and seed 1; the dEQP lists have one). The KHR lists' other configurations
  (other sizes and seeds, FBO surfaces) are not run: 15.8's measure names the lists, and gex_wgpu's
  112,212 GLES cases are one configuration of each.
- G9 run 2: the pinned CTS is unpatched (Mesa's own CI patches its copy); a case that fails by a
  CTS fault is listed as such with its reason, not excluded.

- 6.4 states the binding rule for Direct3D. For Vulkan and OpenGL programs the NIR route reads it as
  Direct3D 12's: within the stage's set each kind's resources in the order of the program's own
  (set, binding), with the set in `ReflectedBinding::space` and the binding in `register`. A combined
  image sampler is a texture and a sampler reflected at the same set and binding.
- The NIR the routes take is fixed by one set of NIR compiler options in the bridge (Zink's list plus
  all 64-bit integer lowering and the instance index counting from the first instance), which 15.3
  does not name; G9 and G10 must hand over NIR made with the same options.
- The corpus is fetched by `corpus/fetch` into the build folder on the rented machine (each source
  pinned by commit), not kept in the Mac's scratch (15.8): the runs happen there, and the pins make
  it the same corpus everywhere.
- Mesa is not a git submodule at `third_party/mesa` (15.2). It is pinned by commit in
  `crates/wcontainers-nir/mesa/pin.toml` and fetched by the build script. Reasons: the literal check
  scans every Rust file and script of a submodule (Mesa has hundreds of Python scripts and Rust files),
  any new baseline or policy line is refused as growth, and an unchecked-out submodule is itself a
  finding; and where the fork 15.2 describes is kept is the owner's decision (15.9). Until then the
  patches live as files in `crates/wcontainers-nir/mesa/patches/` (one so far, Zink's), applied by
  the build script; they move into the fork when there is one.
- The Mesa build runs on the rented x86-64 machine, not the laptop (15.8 says laptop): the brief of
  2026-11-19 makes the rented machines the default. The choice's times (15.3: "measured on the
  laptop") are taken there too, alone on ten cores, as that machine's notes take timings.
- The choice was made on the SPIR-V corpus first (run 4) and made final over the whole corpus,
  GLSL and the other sources in, by `corpus/choose` over all 18,222 items (run 11's, read in run 13).
- Zink is built beside llvmpipe and lavapipe: its `nir_to_spirv` is candidate A's first step.
  SPIRV-Tools is off (Mesa takes it when present; nothing of ours needs it).
- Candidate B reads NIR through a flattened copy the bridge makes (`bridge/flat.c`), not through
  NIR's own structures from Rust: bindgen is out (below), and the copy keeps Mesa's numbering and
  layout on the C side, so a Mesa update changes C only.
- wcontainers-opengl does use bindgen, for Gallium's headers (G9 run 3), as 15.2 says: Gallium's
  structs are what the driver fills, and copying them by hand would define their layout twice. The
  literal check's objection (below) is met by reading `OUT_DIR` inline in `env::var_os`, which
  the check accepts, and by handing the bindings' path to the crate through its own variable.
- wcontainers-nir's bridge-only build for the driver is the absence of a default feature,
  `standalone`, not a feature of its own: Cargo unifies features, so a `driver` feature would have
  taken the SPIR-V steps and the GLSL front away from the corpus and tests whenever both crates
  are built together; an additive `standalone` cannot. Its tests and the corpus run require it.
- The bridge's functions are declared by hand in `src/mesa.rs` and `src/direct.rs`, not by bindgen: bindgen's output
  is included from Cargo's `OUT_DIR`, whose name the literal check refuses as a new code string here
  (its one constant, `CARGO_OUT_DIR`, is private to wcontainers-literals).

- Lowered IO keeps a stage value's interpolation only where a fragment input is loaded. Route B
  gives a vertex output Perspective (Flat for an integer one); where a fragment program reads it
  another way, the pipeline (G9, G10) must give the vertex side the fragment side's. The plan does
  not say; the comparison runs each stage alone, so it does not see this.
- The 16-bit test's reference is lavapipe running the same program with native 16-bit integers and
  Rust's 16-bit arithmetic, both, rather than a corpus mode (the program is the test's own).
- The route's module is run by lavapipe through naga's SPIR-V writer, not by the recording back
  end's interpreter (`wcontainers-gpu-test`) or the WebGPU back end on the RTX 3080 (15.3). Reasons:
  the interpreter's limits (no derivatives, implicit levels only on one-level views, no atomics or
  workgroup memory) would leave most real fragment programs to the GPU queue, while lavapipe runs
  every program the reference runs, on the same machine, with one harness for both sides. What this
  holds is the conversion (NIR to the common form); naga's SPIR-V writer is the same one wgpu's
  Vulkan back end uses. The WebGPU back end's own conversion is the shaders track's.
- The plan says "within the format's rounding" for floats; the targets are 32-bit, and two programs
  that differ only in the order of operations differ by more than one rounding. The tolerance is
  1024 units in the last place or 2^-20 absolute (`compare.rs`), stated and checked to still catch a
  change of constants (Where it stands).
- GLSL is compiled on Mesa's standalone scaffolding with st/mesa's steps after a link written out
  in `glsl.cpp` for one set of capabilities, not by st/mesa itself (which needs a gallium screen
  and context: G9's driver). The steps and their order are st/mesa's; the shader-variant steps
  st/mesa takes at draw time (clamped colours, flat shading, two-sided colour, user clip planes,
  point size), which depend on the draw's state, are not taken here: G9 hands its variants over
  and its own corpus run holds them.
- OpenGL's buffer accesses reach route B as arrays of words, not as typed blocks: after
  `gl_nir_lower_buffers` only byte offsets remain (the block types are gone from the
  instructions), so the bridge makes the words form, the one 15.4 names for robustBufferAccess.
  Program binding for OpenGL: uniform block index, storage block index after them.
- Vulkan capability classes: a program needing what our Vulkan 1.1 will not report is counted apart,
  not as a route failure (the pass bar's "named reason").

- llvmpipe is reached through EGL's software device (`EGL_EXT_platform_device`), not the surfaceless
  platform 15.2's options name: the rented machine has no render node, and surfaceless takes software
  only when told by an environment variable (`LIBGL_ALWAYS_SOFTWARE`), which a test process should
  not set for itself. The context is OpenGL 4.5 compatibility, llvmpipe's; the bridge reports 4.6
  (G9's), so a program needing 4.6 is refused by the reference and counted not compared.
- The GLSL comparison fills its inputs from OpenGL's reflection of the program on llvmpipe (names,
  types, offsets, strides) and the bridge's names for where each lands in the route's module; the
  SPIR-V comparison fills from the route's module alone.
- GLSL vertex and fragment programs are compared as a drawn program (the vertex program wrapped in
  its own source, a feeder or catcher written beside it), not each stage alone: names between two
  stages do not survive OpenGL's linker, and the draw is how llvmpipe runs a stage. In a vertex
  and fragment pair, what the vertex program computes is compared through what the fragment
  program makes of it; its position is compared only when it is alone (the catcher).
- The corpus's `sources` lines gained a paths column (`.` for the whole tree): bgfx is fetched
  sparsely (its shaders and licence only), as dEQP's data files will be.
- bgfx's 3 differences (infinities from seeded RGBE texels, and one unit in the last place made
  large by `fract`) are left as differences with their reasons, as piglit's 17 are.
- dEQP's cases are run on the corpus's seeded inputs, not on the values in each case's `values`
  block: the corpus holds the route to llvmpipe, not to dEQP's expected results, and seeded inputs
  reach more of each program. Where dEQP compares the outputs in the shader, the corpus writes them
  to colour targets (Done). Of a requirement's alternative extensions the first is taken (dEQP
  takes the first the context reports); a case whose first alternative Mesa's compiler refuses is
  counted as reaching no route with Mesa's log.
- ES programs are compared at full precision on both sides (`#define mediump highp` and the rest):
  llvmpipe lowers `mediump` to 16 bits, our driver does not; both are allowed by GLSL ES, and the
  reference must compute what the route is to compute.
- The 17 differences from undefined behaviour on seeded inputs are left as differences with their
  reason, not hidden: the seed gives indices out of range and negative square roots, which the
  programs' own tests never give.

- Godot's Compatibility shaders are made for desktop OpenGL (`#version 330`, what Godot gives a
  desktop driver; our OpenGL reports 4.6 compatibility), at Godot's default project settings, with
  each mode at the default specializations and with each one specialization flipped, not every
  combination (scene alone has 33 specializations: 2^33 programs, most never built by Godot).
  Combinations Godot never builds that Mesa refuses are counted as reaching no route with Mesa's
  log.
- A vertex program without a position gets (0, 0, 0, -1) in the bridge (NIR), not through the
  shaders crate's `stage::clip_unpositioned` (crate-private, and on naga); the same rule, the same
  value. It moves to the shaders crate's entry point when there is one (Requests).

- Godot's RenderingDevice variants are every variant each `initialize` call lists, at Godot's
  default desktop settings, with no material (as the Compatibility shaders are taken), compiled for
  a Vulkan 1.1 device (SPIR-V 1.3); Godot would compile only the groups a device and project
  enable. Texture streaming (a module the default build leaves out) adds nothing to the scene
  defines.
- The GLSL comment marker Godot's reader cuts is named `GLSL_COMMENT_START`, not imported from the
  literal check's `LINE_COMMENT` (Rust's, private there): the same two characters, another
  language's fact.
- Subgroup operations (`reduce` and the like) are refused by name by route B: 15.4 has the Vulkan
  driver (G10) lower them to their one-invocation meaning before the route, so the corpus, which
  hands over NIR as spirv_to_nir makes it, counts them as failures until G10's step exists.

- A storage cube is a 2D array of its faces in the common form (run 13): the plan does not say;
  WGSL has no storage cubes, and Vulkan and OpenGL address a storage cube's faces as layers.
- A reference that does not end on the seeded inputs is held on the next seeds, up to four (run
  13), rather than counted not compared: the program is the same and the inputs are as seeded as
  before; the seed used is in the results.
- Dolphin is left out of G8's corpus (run 13): no permissively licensed boot image was found, as
  the orchestrator's ruling of 2026-11-19 night requires; the choice is final without it.
- `direct::rewrite` takes only 15.4's part of a variant key (storage formats, texels through a
  buffer) and refuses the rest by name (run 14): the other rewrites read the program's stage
  values, which the driver's reflection holds (G9, G10); it gives the bindings, not a whole
  `Reflection`, so nothing reflected is left empty and passed on as if filled.
- A storage image without a format is declared `r32float`, `r32sint` or `r32uint` by its texel's
  kind until the variant names the bound view's format (run 14): the shaders crate's contract
  asks for any format of the texel's kind.

- G9 run 9: a blend reading the blend constant's colour and its alpha, on a back end without
  constant alpha factors, gives the alpha as the blend's second source from the fragment program
  and counts it as `BlendInShader`, whose description in the interface is blending that reads a
  copy of the target. It reads no copy: the blend itself stays the back end's; only one factor's
  value comes from the program. The interface has no closer name and this track adds none
  (routed decision 4); asked under Requests.
- G9 run 9: Mesa's blitter's viewport, which maps depth -1..1 to itself, is read as the
  zero-to-one viewport (`viewport` in draw.rs): exact for the blitter's own depths, which are 0
  or above, and OpenGL itself clamps every depth range it is given to 0..1, so no program's draw
  meets it.
- G9 run 9: integer textures are sampled through their nearest texel's load, made in NIR from
  the bound sampler's wraps and level bounds, which the program variant is keyed on (one variant
  per such sampler state). Cube maps and the border and mirrored-clamp wraps are refused by name
  until built.

## Interface changes

None.

## Requests to other tracks

- Shaders track (G9 run 10, a finding): naga's HLSL writer writes every constant texture offset
  as `int2(...)` (`back/hlsl/writer.rs`, the DXC workaround), so a 3D texture's `int3` offset
  becomes `int2(int3(...))`, which DXC refuses ("too many elements in vector initialization").
  40 logs of dEQP-GLES3 shaders.texture_functions; Direct3D 11's 3D textures with offsets reach
  the same writer.
- WebGPU back end (G9 run 10, findings): `create_sampler` clamps the largest level of detail to
  wgpu's 32 and not the smallest, so a sampler with a minimum over 32 is invalid (the driver now
  clamps both itself); `write_texture` of part of a stencil aspect, or of 16-bit depth, goes
  straight to wgpu, which refuses it (the driver now writes such parts through
  `copy_buffer_to_texture`); `copy_buffer_to_texture` of part of a 16-bit depth level still
  fails ("part of a 16-bit depth level written from a buffer").

- Interface track (G9 run 9): a slow path for a blend factor's value given by the program as the
  second source (the OpenGL driver's constant colour and alpha in one block on WebGPU), apart
  from `BlendInShader`, which names blending that reads a copy of the target. Counted as
  `BlendInShader` until then (Departures).
- Literals track (G9 run 9): the check reads the inside of `macro_rules!` bodies as code, so the
  driver's `unbuilt!` stub's `extern "C"` and its `eprintln!` text are findings (3); an ABI string
  and a message inside a macro body are what they are outside one.

- Shaders track (G9 run 7): the pulled-input fetch (`variant/pull.rs`, `fetch`) reads each
  component from one word at `address >> 2`, so a component that does not start at a multiple of
  its size (a float at byte 17) reads the wrong bits. Direct3D's alignment rules never make one;
  OpenGL allows any stride and offset (dEQP-GLES2's `multiple_attributes.stride.*_17`). A
  component across two words needs two loads joined by shifts. Until then the OpenGL driver
  refuses such an input by name.
- Interface track (G9 run 7): the alignment a back end needs of vertex strides, attribute offsets
  and stream offsets (WebGPU: 4, and an attribute's offset to its size up to 4, within the
  stride) as a limit or a documented rule of `VertexLayoutDesc`. The OpenGL driver takes
  `wgpu::VERTEX_ALIGNMENT` for it today, which is WebGPU's rule applied to every back end.
- WebGPU back end (G9 run 6; the driver no longer depends on it since run 7, which fetches such
  streams itself): `create_vertex_layout` accepts a stream whose stride is not a
  multiple of 4 (OpenGL allows any stride: dEQP-GLES2's `vertex_arrays` cases with strides 2 and
  17), and wgpu then refuses the pipeline ("Vertex buffer 0 stride 2 does not respect
  VERTEX_ALIGNMENT"), which fails the whole submit and every case after it in the process. The
  back end should take such a stream by a counted slow path (repacked, or fetched in the program)
  or refuse the layout so that a front can. (Run 5's request about 2D views of 3D textures is
  withdrawn: it was this driver's misread view target, fixed in run 6.)

- Literals track (G9 run 3): a lawful form for a build script's output folder. The check accepts
  `env!("OUT_DIR")` but not `env::var_os("OUT_DIR")`, the only way a build script can read it,
  and its own constant for the name (`CARGO_OUT_DIR`) is private; and `include!` takes only a
  literal, so the file a build script writes there is named twice. Either accept those two
  positions or export the constant; until then the two findings stay (Literal check).
- Interface track (15.7): everything 15.7 lists; G8 needs `HelperValue::BufferRange` and the slow path
  names `RobustAccessInShader` and `AlphaToOneInShader` before its robustness and alphaToOne parts can
  be reflected.
- Shaders track (a finding): naga's SPIR-V reader fails on HLSL's row-major array of matrices once
  it is decorated (`shadowmappingcascade/scene.frag`, "invalid access As" on candidate A); not needed
  by route B.
- Shaders track (run 14): `wcontainers_shaders::rewrite` validates without
  `Capabilities::IMMEDIATES`, so a module with push constants is refused ("rewrite: invalid: ...
  IMMEDIATES is not supported"): 20 of Godot's RenderingDevice programs with arrays of textures
  (`gi.glsl` and kin). The same as the `capabilities` request below, for `rewrite`'s validation.
- Shaders track (run 14, information): `rewrite` recomputes `sampled_pairs` only when it splits
  an array; a module with none keeps the pairs given. The NIR route gives none (its driver makes
  the reflection), so a public `sampled_pairs` of a module, or `rewrite` always recomputing them,
  would let G9 take them from one place.
- Interface track: `pub const fn stage_set(stage: ShaderStage) -> u32` in `reflect.rs` beside
  `helper_binding` (vertex and compute 0, fragment 1, plan 6.4). The shaders crate writes the rule as
  a literal in `lower.rs`; the NIR route defines `wcontainers_nir::bind::stage_set` until the
  interface has it, and will import it then.
- Shaders track: `stage::clip_unpositioned` as part of that entry point, so the NIR route gives a
  vertex program without a position the same position from one place (the bridge does it in NIR
  until then).
- Shaders track: the draw's bases rewrite (`HelperValue::DrawBase`) as part of the public entry point
  below, for Vulkan programs reading `BaseVertex` or `BaseInstance`.
- Shaders track: `wcontainers_shaders::capabilities` to add `Capabilities::IMMEDIATES` for a module
  with an `AddressSpace::Immediate` global, as it adds the other extensions a front asks for only
  where the back end has them; until then the NIR route adds it itself.
- Shaders track (a finding, not a request for this route): naga's SPIR-V reader turns a sampled
  array layer into an integer with a truncating `As` (`front/spv/image.rs`), where Vulkan and
  Direct3D round to the nearest even integer; a layer of 1.6 reads layer 1 instead of 2. The NIR
  route rounds in NIR first; dxil-spirv's and dxbc-spirv's output reaches the same reader.
- Interface or shaders track (later, when the Vulkan driver reports `vkCmdDispatchBase`): a helper
  value for the base workgroup; the routes now count dispatches from workgroup 0.
- Interface track: a helper value for the view index (e.g. `HelperValue::ViewIndex`, u32, written
  by the front for each view of a `MultiviewSerial` draw), so a program reading `gl_ViewIndex`
  (Vulkan 1.1 requires `multiview`) has it in the common form; route B refuses `load_view_index`
  by name until then (66 of Godot's RenderingDevice programs).
- Interface track: a public `CUBE_FACES` (the layers of one cube) beside `ViewDimension`;
  `resource.rs` writes it as a literal, and `wcontainers_nir::CUBE_FACES` defines it until the
  interface has it, and will import it then. Since the Direct3D 11 track's 7e4d8df6 its device
  defines one too (`pub(crate)`), and the literal check, on a tree holding both, reports the NIR
  crate's as a redefinition (run 15): neither front nor this crate can import the other's, so the
  interface's is the one place for it.
- Shaders track (a finding of run 15, from G8's second check): naga's SPIR-V reader maps
  `OpFUnordNotEqual` to `BinaryOperator::NotEqual` (`front/spv/convert.rs`), and naga's SPIR-V
  writer emits `NotEqual` on floats as `OpFOrdNotEqual` (`back/spv/block.rs`), so an unordered
  not-equal (true on NaN) becomes ordered (false on NaN) on the way through naga. Route B no longer
  uses `NotEqual` for NIR's `fneu`; dxil-spirv's and dxbc-spirv's output reaches the same reader
  (HLSL's `!=` and `isnan` on floats are unordered).
- Literals track: `CARGO_OUT_DIR` public (in a public module), so a build script can name `OUT_DIR`
  through it.

## Literal check

G9 run 10: exit 1 at 9a5baaf7 (x86-64 machine, from a git bundle; marks 0; the clone, its build
and the bundle deleted after; output in scratch `logs/g9r10-literals.out`): the same 7 findings
as run 9, none new (`unbuilt!`'s three, the two `OUT_DIR` ones, `CUBE_FACES` twice).
wcontainers-nir and wcontainers-opengl have no baseline lines.

G9 run 9: exit 1 at cd5162cf, the run's last code commit (x86-64 machine, from a git bundle;
marks 0; the clone, its build and the bundle deleted after; output in scratch
`logs/g9r9-literals-2.out`, a first pass at 207e7587 in `logs/g9r9-literals.out`): 7 findings,
from 47 at run 8. Fixed this run: the
driver's refusal texts are now messages the check reads (`refuse` takes any `Display`, given
`format_args!`; `refused` returns one as an error), the colour arrays' 4 is `COLOR_CHANNELS`
(derived from Gallium's `pipe_blend_color`), the copy's entry points are named, its vertex count
is the shaders crate's `TRIANGLE_VERTICES`, and `QUERIES_PER_SET` (which shared a word and a value
with the Direct3D 11 front's `SET_SLOTS`) is `OCCLUSION_QUERY_BATCH`. Left: `unbuilt!`'s three
(inside `macro_rules!`, asked of the literals track), the two `OUT_DIR` ones (asked since run 3),
and `CUBE_FACES` twice (waiting on the interface). wcontainers-nir and wcontainers-opengl have no
baseline lines.

G9 run 8: exit 1 at dd5c5606 (x86-64 machine, from a git bundle; marks 0; clone, build and
bundle deleted after; output in scratch `logs/g9r8-literals.out`): 47 findings, 45 in
wcontainers-opengl (draw.rs 18, state.rs 12, query.rs 5, context.rs 3, present.rs 3, and one
each in build.rs, blit.rs, gallium.rs, view.rs; most are refusal texts passed to `refuse`, five
are a bare 4, and `QUERIES_PER_SET` restates the Direct3D 11 front's `SET_SLOTS`), and
`CUBE_FACES` twice (waiting on others). Not yet worked down: next run, after the provoking
vertex.

G9 run 7: not run (the run's budget went to the desktop). The Rust added names its numbers
(`QUERIES_PER_SET`, `VERTEX_ALIGNMENT` from wgpu's own constant, sizes from `size_of` and
Gallium's format table); its new refusal texts go through `refuse` like run 6's, which the check
reported. Run 6's 38 findings are still to be worked down; to run first next run.

G9 run 6: exit 1 at 30a4c084's successor 7ead95aa (x86-64 machine, from a git bundle; marks 0;
the clone and its build deleted after; output in scratch `logs/g9r6-literals.out`). 38
findings: `CUBE_FACES` twice (Waiting on others) and 36 in wcontainers-opengl, most from runs 3
to 5: the refusal texts given to the driver's own `refuse` and returned as `Err("...".into())`
(the check's documented false catch of a crate's own message helpers; to be made messages the
check reads, or named), `"C"` and the format string of the `unbuilt!` macro, the shader entry
names `"vs"`/`"fs"` in present.rs, `[u32; 4]` and `[f32; 4]` colour arrays, and run 3's two
`OUT_DIR` ones (build.rs's `"OUT_DIR"` and gallium.rs's `"/gallium.rs"`, still reported). Fixed
this run: the draw's binding sets (from `MAX_BINDING_SETS` by `array::from_fn`) and two zero
colours (`Default::default()`), 2062f8fe. wcontainers-nir and wcontainers-opengl have no
baseline lines.

G9 run 5: run at 99e5ff55 (x86-64 machine, from a git bundle; marks 0; the clone and
its build deleted after). Findings shown: `CUBE_FACES` twice (Waiting on others) and
`COMPONENTS` in `wcontainers-nir/src/mesa.rs` ("a second name for MAX"), now gone: the swizzle's
length is `naga::VectorSize::MAX` itself (d287ead9). Only the summary's last lines were kept, so
whether the two `OUT_DIR` findings of run 3 are still reported was not seen; the next run runs
it again whole. wcontainers-nir and wcontainers-opengl have no baseline lines.

G9 run 3: exit 1 at d6bd4c8a (x86-64 machine, from a git bundle; marks 0; the clone and its
build deleted after). The first try found 11; fixed this run: the cargo words and the Mesa
variable both build scripts defined (`MESA_VARIABLE`, `CHECK_CFG`, `CFG`,
`RERUN_IF_ENV_CHANGED`, `RUSTC_ENV`, `ASSIGN`) now live once in `mesa/meson_build.rs`, which both
include, with the defines bindgen takes (`mesa_defines`); the driver's `'\0'` (run 2's) is gone.
Left, 4 findings: `CUBE_FACES` twice (Waiting on others), and two of the driver's that no legal
form avoids today: `env::var_os("OUT_DIR")` in its build script (the check accepts `OUT_DIR` only
inside `env!`, which a build script cannot use for it, and its own name for it, `CARGO_OUT_DIR`,
is private) and `include!(concat!(env!("OUT_DIR"), "/gallium.rs"))` (a macro that takes only a
literal; handing the path through a variable of the build script's instead was found as a
restated name the same way). Asked of the literals track under Requests. wcontainers-nir and
wcontainers-opengl have no baseline lines.

G9 run 2: not run (the run's budget went to the desktop); the Rust added has no bare literal
(names, paths and flags are named constants; the build scripts share `mesa/meson_build.rs`
rather than defining its constants twice). To run first in the next run.

G9 run 1: exit 1 at 248050c5 (x86-64 machine, from a git bundle; marks 0), with the one finding
outside this track's reach, `CUBE_FACES` (Waiting on others). wcontainers-nir still has no
baseline lines; this run added no literal (the PowerShell script names its versions and paths once
at its top; the check does not scan PowerShell).

Run 15: exit 1 at 52c083b9 (x86-64 machine, from a git bundle; marks 0), for one finding outside
this track's reach: `CUBE_FACES` in `wcontainers-nir/src/lib.rs` (since run 9) is reported as a
redefinition of the Direct3D 11 device's, added on that branch at 7e4d8df6 and merged here this
run (Requests: the interface's one constant). Every other finding is the shaders crate's
submodules, not checked out in the bundle's clone (as run 6 found); this run added no literal.

Run 13: exit 0 at the run's last code commit, 86cefeb2 (x86-64 machine, from a git bundle;
marks 0). wcontainers-nir still has no baseline lines.
Run 12: exit 0 at the run's last code commit (x86-64 machine, from a git bundle; marks 0).
wcontainers-nir still has no baseline lines.
Run 11: exit 0 at the run's last code commit (x86-64 machine, from a git bundle; marks 0). A first
try found the Godot reader's `LINE_COMMENT` ("//") named like the check's own (Rust's comment, private
there): renamed for what it is, GLSL's (Departures). wcontainers-nir still has no baseline lines.
Run 10: exit 0 at the run's last code commit (x86-64 machine, from a git bundle; marks 0). A
first try found the new reader's comment marker defined beside dEQP's (`COMMENT`, '#'): Godot's
reader now imports dEQP's. wcontainers-nir still has no baseline lines.
Run 9: exit 0 with the run's code (x86-64 machine, from a git bundle; marks 0). A first try
found `"in"`, `"out"` and a decimal point in `tests/corpus/deqp.rs` restating constants of the
WebGL back end and the check (private there): GLSL's qualifiers are now written through one
function of format strings, and the dEQP grammar's one word before a stage list is taken as any
word. wcontainers-nir still has no baseline lines. Run 8: exit 0 at the run's last code commit, 9e0a7c6e's parent (x86-64 machine, from a git bundle; marks 0; a first
try found the GLSL version directive named twice, in `glsl.rs` and the new `bgfx.rs`: now imported);
wcontainers-nir still has no baseline lines. Run 7: exit 0 at the run's last code commit (x86-64 machine, from a git bundle of the branch; marks
0); wcontainers-nir still has no baseline lines. Run 6: exit 0 at 4294fcf0 (x86-64 machine, from a git bundle; marks 0; the 18 changes that could
add lines are the shaders branch's, as before). The bridge's numbers are `#define`s, named once;
the Rust added has no new bare literals (test data apart). Adopted with the branch (merged from the
shaders branch, which carries it). wcontainers-nir has no
baseline lines: the check passes with none for it (run on the Mac, runs 1 to 4, exit 0; in run 4 the
two new scripts' numbers were taken out (the timing cores are passed in, the load read by `read`), and
one baseline line the Direct3D 11 merge left stale was deleted, as the check asked;
it needs the git history, which the rented machines' copies lack). The list of
changes that could add lines is the 18 the shaders branch brought; this branch added none. C files are
not scanned; the bridge names its one version by `#define`s all the same.
