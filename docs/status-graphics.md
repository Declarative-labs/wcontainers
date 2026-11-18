# Graphics track status (interface, G1c)

Plan: `docs/phase-2-graphics.md`. Brief: `pyro-parked/wcontainers-scratch/orchestrator/brief-graphics.md`.
This branch, phase-2-graphics, is the shared line: the interface track merges the other graphics
branches into it at the start and end of every run, keeps the whole workspace building, and pushes.

## Where things stand

- G0, G1a, G1 and G1b are done and passed their independent checks. Their history, run by run and
  finding by finding, is in the git log and in this file's history (G1b's last state: f639e27d).
- G1c, run 1 (2026-11-18), done and pushed:
  - Start merges: webgpu and direct3d-11 (fast-forwards: they held the others' new work), then
    direct3d-12 (a merge); webgl, shaders and rig had nothing more. No conflict. The whole workspace checks after them
    (laptop, `cargo check --workspace --all-targets`, scratch `g1c/start-check.log`); pushed.
  - The six findings of G1b's second check settled (below).
  - Every "interface:" commit the other branches made since G1b (eleven) checked and covered
    (below).
  - The WebGPU track's standing request decided (below).
  - Part 2: plan section 15, the second batch (OpenGL and Vulkan through Mesa on the interface,
    the NIR route, the call channel and stand-ins, sound and input hooks, the interface changes
    they need, milestones G1d and G7 to G12, and the owner's decisions).
  - End merges: direct3d-11 and shaders (below, "End of run"); the rest had nothing new.
- G1c, run 2 (2026-11-18): the fourteen findings of G1c's independent check settled (below, "G1c's
  check"). Start merges: webgl (a merge) and direct3d-11 (a merge); the others had nothing new; no
  conflict, no owned file changed; the whole workspace checks after them (laptop, scratch
  `g1c2/start-check.log`); pushed (33790064).

## G1c's check, and how each finding was settled (run 2)

- Blocking, plan 15.4's "absent, because WebGPU has no route": wrong; the interface has routes for
  most of them and wvk serves them on WebGPU (checked at its README). 15.4 now reports present
  natively `imageCubeArray`, `independentBlend`, `sampleRateShading` (and three optional ones where
  the adapter has them, the Direct3D routes otherwise); present through named, counted routes
  `logicOp`, `fillModeNonSolid`, `largePoints`, `multiViewport`, `shaderFloat64`,
  `shaderCullDistance`, `vertexPipelineStoresAndAtomics`, `occlusionQueryPrecise`,
  `pipelineStatisticsQuery`, `geometryShader`, `tessellationShader`, `shaderDrawParameters`; and
  `wideLines` and `shaderInt64` through two slow path names proposed in 15.7 (`WideLinesAsQuads`,
  `Int64Emulated`, made at G1d). Absent: sparse binding, as a stated scope choice. An owner decision
  in 15.9.
- wvk as prior art: 15.4 names it (checked at the source: `src/wink/vulkan`, MIT headers, Vulkan 1.2
  on Mesa's runtime over `webgpu.h`, its dEQP-VK run: 313,069 cases run, 98.5% passed), with its
  design beside ours; G10's measure takes 98.5% as a floor; 15.9 asks whether to consider it.
- The mesa-wink date: 15.2 now gives the driver directory's last change (2026-10-28, 41e1c1dfab),
  the `wink` head (2026-11-11) and that 2026-11-17 is GitHub's push date; and the native build
  against Dawn (`gex_wgpu_native.c`), and the shared compiler `src/wink/compiler` (all checked).
- G9's bar: one measure (passed over run, NotSupported not run, and each NotSupported group asks for
  something the driver does not report) over 15.2's lists, which are now the families gex_wgpu
  reports (dEQP-GLES2, -GLES3, -GLES31, KHR-GLES) plus KHR-GL33; floors 92.9% and, for KHR-GL46,
  98.6%, said to be floors and not a like-for-like comparison. GLES contexts on Windows come from
  Mesa's WGL ES profile extension (checked in `stw_ext_extensionsstring.c`).
- G8's bar: the route converting more shaders is chosen, on equal counts the faster.
- G7: crossings defined (a hand-over to the device thread; host calls on the guest's thread counted
  apart); Direct3D 11 gains its device thread at G7 (15.1); the Wine stand-ins are proven on the
  laptop under a pinned Wine with the WebGPU back end on lavapipe, against the rig's references.
- Consumed arguments copied into streams: named in 15.1 as a narrowing of the brief, with why
  (the program may reuse the memory when the call returns), measured at G7, and in 15.9.
- The WASI module and one device: 15.2 now makes the back end its own module per page that every
  front module (the Direct3D one; the Mesa one, `wasm32-wasip1`) calls by imports; G9b's first step
  settles it with the other open questions and measures the import's cost; module names in 15.9
  follow (`gpu-webgpu.wasm`, `gpu-webgl.wasm`, `opengl.wasm`, `vulkan.wasm`).
- The suite's copy within one depth or stencil subresource now also covers `Depth24PlusStencil8`
  and `Depth32FloatStencil8`, each aspect written and read back (24-bit depth within one step).
- `CopyWithinTexture`'s warning now says "a temporary buffer or texture".
- `every_vertex_format_reads_its_element` reads each element at attribute offset 12 behind filler
  bytes, and the 11-11-10 case holds an 11-bit denormal, the greatest 11-bit float and a 10-bit
  float with mantissa bits; the check's denormal fault is now caught (below).
- `a_bindings_slot_is_its_place_in_its_kinds_range` checks every slot of all five kinds, that the
  ranges adjoin, and that counters have as many slots as storage.
- `SlowPath` numbers: a new unit test `reasons_keep_their_numbers` holds every reason's number by
  name, and `SlowPath`'s doc and plan 6.4 say new reasons go at the end. The status's warning about
  the direct3d-12 branch was wrong: that branch is wholly contained in this line, so its next merge
  of the line takes this order with no conflict; only caches or logs it recorded under the old order
  are at risk (below, for the other tracks).

## G1b's second check, and how each finding was settled

- The status had the recorder sampling rg11b10ufloat and none of bgra8 and the others. Wrong: the
  recorder holds no rg11b10ufloat and samples bgra8unorm, and none of rgb9e5ufloat, bgrx8, a8 and
  b5g6r5 (the check's probe). This file's old entry is gone; the rule stands as written there.
- A copy between regions of one depth or stencil subresource: decided that the interface allows it
  for every format (Vulkan's `vkCmdCopyImage` and OpenGL's `glCopyImageSubData` allow it; Direct3D
  copies depth and stencil only whole, so only the OpenGL and Vulkan fronts will ask). A back end
  whose API refuses one subresource as both copies colour through a buffer and depth or stencil
  through a temporary texture of the same format, by its routes between textures, counted
  `CopyWithinTexture`. `copy_texture`'s doc, `SlowPath::CopyWithinTexture`'s doc and plan 4.2 say
  so (eb13c84b). The recorder already takes it. Suite test (new):
  `a_copy_within_one_depth_or_stencil_subresource_moves_the_region` (depth32float in eighths,
  depth16unorm, stencil8, each where the back end copies it out). For the WebGPU track, below.
- Plan 6.4's list of helper values now says the texel buffer view serves the texture route too, and
  names the texel layout's view size and the layer and viewport values.
- The random sequences now copy within one subresource: a copy whose source and destination share
  texture, level and layer moves the region to beside the source (right, left, below or above, where
  it fits) and is dropped only if the two still overlap. `tests/sequences.rs` prints how many.
- 871a23bd (the texel layout grown to five u32) merged and covered (below).
- `wcontainers-direct3d-11`'s `format::components` now returns `Format::components` (one table);
  its 3 unit tests and clippy pass. Departure noted below.

## Interface changes

Each entry: the change, whether it fits the interface's style, its rule check and test (recording
back end, `crates/wcontainers-gpu-test/tests/` unless named) and its suite test (`src/suite*`, run
by every back end through `suite!`). Changes up to G1b, with their tests, are listed in this file at
f639e27d; all are kept.

Since G1b (from the other tracks, merged at this run's start):
- 871a23bd (shaders) `HelperValue::TexelLayout` grows to five u32 (pitches, then the view's width,
  height and layers; texels outside read zero). Fits: a helper layout. Back ends do nothing new; the
  front writes it and converted programs read it, which the shaders crate's tests cover. No rule.
- 4a72bce0 (shaders) `FRACTIONAL_VIEWPORT`'s doc: the scissor of a viewport drawn in the program is
  cut to the pixels its API draws (Direct3D rounds each edge down). Fits: a doc of a front's rule;
  no interface helper computes it, so the fronts' tests against real Direct3D cover it.
- 9f79bb5a (shaders) `DOMAIN_PATCHES_BINDING` 13 and `DOMAIN_POINTS_BINDING` 14, and 3fc7fee0
  (direct3d-12) `COUNTER_BINDING` 15: helper set bindings. Fits. Rule test (new, replacing the
  narrower one): `helper_set_bindings_never_meet` (`wcontainers-gpu` unit test; every binding and
  range of the helper set against every other).
- 4a835192 (shaders) a hull shader's reflection gives its point and constant locations. Fits: data
  of the reflection. No rule; no back-end behaviour.
- 97fa85c1 (shaders) `ReflectedBinding::slot`, and per-slot fields named by it. Fits. Rule test
  (new): `a_bindings_slot_is_its_place_in_its_kinds_range` (`wcontainers-gpu` unit test).
- 3fc7fee0 `SlowPath::PixelInvocationsCounted`, c024aa3a (direct3d-12)
  `SequentialBuffersCopied`, c6e80691 (direct3d-12) `SwizzleInShader` and `AtomicExchangeLoop`:
  named slow paths. Rule: `all_lists_every_reason_in_order` (`wcontainers-gpu`).
- 07985aaa (direct3d-12) the occlusion counter sums on the GPU where the back end computes, and
  `end_into` writes a count into a front's buffer in order. Fits: a helper with a default (read
  back) and a better route. The recorder computes with no workgroup memory, so it takes the default.
  Test (new): `the_occlusion_counter_writes_its_count_into_a_buffer_in_order` (`tests/helpers.rs`:
  the words beside it stay, a later write replaces it). Suite (new):
  `the_occlusion_counter_counts_every_sample_that_passes` (37 by 21 pixels of a 40 by 24 target,
  sizes no workgroup divides, one and four samples, through `end` and `end_into`; on WebGPU this
  runs the summing program).
- 02674dd7 (shaders) `HelperValue::Layer` and `Viewport`. Fits: helper values a front writes. No
  back-end behaviour.
- 4aaf34b2 (direct3d-12) `AddedBinding::TextureLayer` and `VariantKey::texture_layers`. Fits: an
  added binding and a variant field. Back ends bind textures at added bindings as before; no new
  rule.
- c6e80691 (direct3d-12) `ViewComponent`, `VariantKey::swizzled`, `Reflection::slow_paths`, serde
  on `SlowPath`. Fits as a variant field. Gap found: `Features::VIEW_SWIZZLE` has no call that uses
  it (`TextureViewDesc` has no mapping), so fronts always apply mappings in the program. Proposed
  in plan 15.7 (G1d); not changed now, since adding a field changes every front's and back end's
  view code.

G1c's own (eb13c84b, one "interface:" commit): copies within one depth or stencil subresource
(above); `prepare` and `PrepareDesc` name no index format, by decision (below); `VertexFormat`'s
doc and plan 4.12 and 5.7: every back end takes every vertex format, so no capability lists them
(below); the two `wcontainers-gpu` rule tests above. Suite tests added for them:
`a_copy_within_one_depth_or_stencil_subresource_moves_the_region`,
`every_vertex_format_reads_its_element` (all 52 formats, one element read as a `vec4` of its number
type into an `rgba32` target, normalised values within 2^-20; the recorder now reads the four packed
formats it refused before: `Bgra8Unorm`, `Unorm10_10_10_2`, `Uint10_10_10_2`, `Float11_11_10`).

G1c run 2 (two "interface:" commits, c1324007 and 4bbfe962): `CopyWithinTexture`'s warning text;
`SlowPath`'s doc (numbers are kept, new reasons at the end) with the rule test
`reasons_keep_their_numbers`; the slot rule test widened to every kind and slot, the ranges
adjoining, and counters as many as storage slots. Suite: the copy within one depth or stencil
subresource and the vertex format test widened (above), 1cfae99c. No new "interface:" commit came from the other branches since
run 1 (the webgl and direct3d-11 merges touched no owned file).

## The WebGPU track's request, decided

- No index format in `PrepareDesc`. Direct3D 11, OpenGL and Vulkan bind index buffers at the draw,
  not with the state, so no front knows the format when it prepares (Direct3D 12's strip cut value
  says nothing when restart is off). A back end whose pipeline holds strips' index format prepares a
  strip for both, as the WebGPU back end already does; `prepare`'s doc and plan 4.3 say so.
- No capability for the vertex formats a back end converts. Every back end takes every
  `VertexFormat` and converts what its API lacks itself (WebGPU's shadow streams); fronts pull in the
  program only for their own reasons (step rates, integers read with the other signedness). Nothing
  a front would do differently was found, so no capability (rule: none without a real use).

## For the other tracks

- WebGPU: a copy within one subresource of a depth or stencil format must no longer lose the device:
  copy the source region into a temporary texture of the same format (your routes between textures:
  whole subresource natively, part by draws), then from it into the destination, counted
  `CopyWithinTexture`. For `Depth24PlusStencil8` and `Depth32FloatStencil8` both aspects move by
  one copy: copy the whole subresource into the temporary natively, then the region back by
  `DepthWrittenByDraw` and `StencilWrittenByDraw` (G1c run 2 added these two formats to the test).
  The new suite tests `a_copy_within_one_depth_or_stencil_subresource_moves_the_region`,
  `every_vertex_format_reads_its_element` and `the_occlusion_counter_counts_every_sample_that_passes`
  (your summing program's first shared test) come in when you merge.
- WebGL: the same three suite tests; `every_vertex_format_reads_its_element` asks for `Bgra8Unorm`
  and the packed formats, which GL ES lacks or reads differently, and `rgba32float` targets (needs
  `EXT_color_buffer_float`; the test asks `RENDER | COPY_OUT` of it, so a missing extension fails it
  by name).
- Direct3D 11: `format::components` now returns `Format::components`; it can go when convenient.
- Direct3D 12: `Features::VIEW_SWIZZLE` gets a call at G1d at the earliest (plan 15.7). Your branch
  is contained in this line, so merging it brings this line's `SlowPath` order without a conflict;
  discard any shader cache or log your branch recorded before 2a00617e, whose numbers for
  `SequentialBuffersCopied`, `SwizzleInShader` and `AtomicExchangeLoop` were in the other order.
- Shaders: `CONVERTER_VERSION` hashes this crate's Rust and the carried naga, not
  `wcontainers-gpu`, yet the cache stores `SlowPath` and the reflection's types by number. The
  numbers of `SlowPath` are now held by a test here; the other serialised types of the interface
  (`VariantKey`, `Reflection`) are not, so consider putting the interface crate's sources into the
  hash.
- Every track: `every_vertex_format_reads_its_element` now reads at attribute offset 12 and holds
  11-11-10 denormals; a shadow-stream converter that drops the offset or scales denormals wrongly
  now fails it.

## The rules and their tests

Every rule a caller can break is refused by the recording back end and broken once in a test:
`tests/rules.rs` (108), `tests/compute.rs`, `tests/recorder.rs`, `tests/helpers.rs` (the usage-scope
rule, the pass tracker, the occlusion counter). The list of rules is the test names.

Known unreachable guards (kept, no test): `sample_view`'s comparison without `compare`, storage
writes from vertex programs, an attachment taken twice by a draw, a colour attachment without
`target_bytes`, a bound range past its buffer at the draw, a running query without an occlusion
set. Recorder limits refused with an error naming them: points and lines drawn (captured only),
sample counts other than one and four, cube sampling, implicit level of detail over several levels,
workgroup variables, barriers and atomics, indirect draws and dispatches, alpha to coverage,
pointers to a function's locals passed to another function.

## The recording back end and the suite

- `Recorder` keeps every object's bytes on the CPU, runs each call as it is recorded through an
  interpreter of naga's IR, checks the rules, logs every base call (`calls`) and replays them
  (`replay`). It draws one or four samples. `Recorder::with_caps` takes capabilities away. It reads
  every vertex format (since G1c).
- Suite: 42 tests (`src/suite.rs`, `src/suite/draws.rs`, `src/suite/routes.rs`), listed in the
  `suite!` macro; `tests/recorder.rs` runs them with 6 rule tests (48). G1c added three (above).
  None has run on a GPU from this track; the back-end tracks run them when they merge.

## Next

- G1c's independent check, second round.
- G1d (plan 15.8) once the owner has answered plan 15.9 (now also: wvk, Vulkan's slow features
  reported present, the copy into streams, and the back end as its own module).
- More suite tests beyond the pass bars: every format's clear and read back against real
  Direct3D's bytes, mip levels, layered and 3D targets, fans and loops on back ends that draw lines.

## How to run

- Linux loop (CPU only): `tools/graphics-rig/linux <command>` sends the working tree to
  `~/wcontainers-graphics/src/phase-2-graphics` on tom@192.168.2.240 and runs the command there with
  `CARGO_TARGET_DIR=~/wcontainers-graphics/target/phase-2-graphics`, 5 cargo jobs (2 at load 8 or
  more), at nice 10. Rust there: rustup with the 1.99.0 toolchain the repository names (installed
  by another track; this run installed nothing).
  - `tools/graphics-rig/linux cargo test -p wcontainers-gpu -p wcontainers-gpu-test`
  - `tools/graphics-rig/linux sh -c 'cargo clippy -p wcontainers-gpu -p wcontainers-gpu-test --all-targets -- -D warnings'`
  - `tools/graphics-rig/linux sh -c 'RUSTDOCFLAGS="-D warnings" cargo doc -p wcontainers-gpu -p wcontainers-gpu-test --no-deps'`
  - `tools/graphics-rig/linux sh -c 'cargo check --workspace --all-targets'`
  - Random sequences, seed and count as knobs (defaults 0x5eed and 1000; the test prints both, the
    count run and what was drawn); a failing sequence is shrunk by proptest and printed:
    `tools/graphics-rig/linux sh -c 'WCONTAINERS_GPU_SEED=0x3c1d0e WCONTAINERS_GPU_SEQUENCES=100000 cargo test --release -p wcontainers-gpu-test --test sequences -- --nocapture'`
  - G1c's faults and long sequences: copy scratch `g1c/faults.py` and `g1c/long.sh` to
    `~/wcontainers-graphics/g1c` on the laptop (after `tools/graphics-rig/linux true` sends the
    tree) and run `nohup ./long.sh > long.out 2>&1 &` there (every fault, then 100,000 sequences at
    0x5eed and 0x3c1d0e), or `python3 -I faults.py <tree> <work dir> <target dir> [names]` (it
    copies the tree, changes one place, runs the tests named and prints CAUGHT, SURVIVED or BUILD
    FAILED). Run 2's faults: scratch `g1c2/faults.py`, the same way, each fault a list of changes.
    Earlier runs' scripts are in scratch `g1r2` to `g1b2`.
- A back end runs the suite with a test file holding a `Host` implementation and
  `wcontainers_gpu_test::suite!(MyHost);` (see `crates/wcontainers-gpu-test/tests/recorder.rs`).
- On the Mac only formatting: `nix develop -c cargo fmt -p wcontainers-gpu -p wcontainers-gpu-test`.

## Measurements

- G1c run 2, on the Linux laptop (i7-8750H, 12 threads):
  - Owned crates' tests (scratch `g1c2/test1.log`, load 1.8, and `g1c2/end-check.log`, load 2.9):
    234 pass (interface 36, compute 8, fast paths 14, helpers 19, suite on the recorder 48, rules
    108, sequences 1 at the default 1,000); clippy and docs with warnings denied clean; the whole
    workspace checks.
  - Faults (scratch `g1c2/faults.log` and `faults2.log`, load 4 to 6): 5 of 5 caught by the test
    written for each: 11-bit denormals scaled by 2^-15 (the check's fault; the vertex format test),
    the attribute offset ignored (that test and six others), a copy within one combined depth and
    stencil subresource dropped (the depth or stencil copy test only, which before this run held
    no combined format), two reasons swapped in `SlowPath` and `ALL` together
    (`reasons_keep_their_numbers`; the older order test passes under it), the counter range one
    slot wider (the slot test, after it was widened to check the ranges adjoin: its first form let
    this fault through).
  - No random sequences this run: the generator and recorder are unchanged since run 1's 100,000
    cases at each of two seeds.
- G1c, all on the Linux laptop (i7-8750H, 12 threads):
  - Owned crates' tests after the changes (scratch `g1c/test1.log`, load 1.2): 233 pass (interface
    35, compute 8, fast paths 14, helpers 19, suite on the recorder 48, rules 108, sequences 1 at
    the default 1,000); clippy with warnings denied is clean.
  - Faults (scratch `g1c/faults.log`, load 7 to 9: another track was building): 8 of 8 caught,
    each by the test written for it: a depth copy within one texture dropped, BGRA vertices not
    swapped, the packed alpha unscaled, the 10-bit float read with a 6-bit mantissa (the vertex
    format test), `end_into` writing at offset 0 and writing a wrong count (the helpers test and the
    suite test), a helper binding clash, `slot` off by one.
  - Random sequences (scratch `g1c/seq-*.log`, `--release`, 4 jobs, on cf096f8b's recorder and
    generator), each 100,000 cases asked and run, recorded with no rule broken and replayed
    identically: seed 0x5eed, 5,240,534 calls, 16,243 copies within one subresource (new), 15,122
    multisampled passes, 21,699 depth-stencil passes, 42,596 dispatches, 114,375 timestamps; 70.2 s
    of test (33 s build before), load about 5 at the start; seed 0x3c1d0e (new), 5,219,789 calls,
    16,032 copies within one subresource, 15,206 multisampled passes, 42,923 dispatches; 69.4 s,
    load about 5.
- G1b's and G1's measurements are in this file's history (f639e27d and earlier).

## The laptop's disk

- G1c found 47 GB free of 122 GB at the start; `target/phase-2-graphics` was absent (deleted at
  G1b's end) and was rebuilt. At the end this run deletes its fault build and tree copy
  (`g1c/work`, `g1c/target`) and the loop's build (`target/phase-2-graphics`); logs stay in `g1c`
  there and in scratch `g1c`. Left alone: the other tracks' folders and the `check-g1*` folders of
  the checks.
- G1c run 2: 47 GB free at the start; at the end deleted `g1c2/work` (the fault tree and its build)
  and `target/phase-2-graphics` (7.2 GB); logs stay in `g1c2` there and in scratch `g1c2`. Not
  ours, left alone: `target/phase-2-graphics-` (4.2 GB, of 2026-11-17, origin unknown) and
  `target/phase-2-graphics-shaders`.

## Dependencies added and their licences

- None this run. In `wcontainers-gpu`: `naga` 30.0.2, `bitflags`, `blake3` 1, `half` 2, `serde`
  1.0.229 (MIT OR Apache-2.0; blake3 CC0-1.0 OR Apache-2.0; plan 3). In `wcontainers-gpu-test`:
  `proptest` 1.12.0 (MIT OR Apache-2.0).
- Plan 15 names what the second batch would add (Mesa MIT with permissive files, bindgen, cpal,
  smithay, test suites); none is added, and each licence is confirmed at its milestone.

## Departures from the plan

- G1c run 2: plan 15 departs from the brief in one place and from section 10 in another, each
  named there and in 15.9: consumed arguments are copied into the call streams (the brief says
  data is read in place; 15.1 gives why), and the back end becomes its own WebAssembly module that
  every front module calls (section 10 had one module per front and back end; 15.2).
- G1c: `crates/wcontainers-direct3d-11/src/format.rs` (another track's crate) changed: its
  `components` returns `Format::components`, so the two tables cannot drift (G1b's check, finding
  6). One function body; the track can drop the function.
- G1c: the plan's second batch (section 15) departs from sections 4.8 and 10 where the first batch's
  experience says so (15.1): the channel is second-batch work and the existing Windows libraries
  move onto it at G7.
- G1b: plan 4.7 said the pass tracker ends a split part with `Store`; it refuses to split a pass
  begun with `Discard`, and the explicit front begins such a pass with `Store`. Plan 4.7 says so.
- G1b: some interface changes have no back-end behaviour (helper layouts a front writes, slow path
  names, serde derives); each is listed with the test that guards it and why no suite test can.
- G1 run 10: resource tables stay absent on the recording back end; their one stated rule is tested
  (`resource_tables_are_absent`). The recorder declares `compute_workgroup_storage` 0.
- G1 run 9: only one binding number rule is checked (paired textures at 32 to 159).
- G1 run 5: a one-statement fix in `crates/wcontainers-gpu-webgl/examples/webgl-scenes.rs` (7a0feba).
- G1 run 1: a split pass names its resolve target in every part; the occlusion counter sums by
  `read_back_now` where the back end does not compute (since 07985aaa); random sequences use
  proptest with a fixed ChaCha seed.

## Known failures

- The occlusion counter refuses (`GpuError::Invalid`) a single fan, strip with restart or indirect
  draw that brings a pixel's sum over its samples to its counting format's bound (2048 in
  `r16float`, only on back ends without blendable `r32float`); no test, as no route exists. The
  same for one primitive drawn as many instances where the first instance cannot be raised; tested
  (`the_occlusion_counter_splits_instances_only_where_each_reads_the_same`).

## End of run

- Run 2: no branch had anything new at the end (fetched after the work); nothing to merge. End
  check above (`g1c2/end-check.log`).

Run 1:

- End merges: direct3d-11 conflicted in `crates/wcontainers-gpu/src/slow.rs` only: the line had
  `SequentialBuffersCopied` before `SwizzleInShader` and `AtomicExchangeLoop`, the direct3d-11 and
  shaders branches after them. `SlowPath` is `repr(u8)` and serialised in the shaders crate's cache
  (`Reflection::slow_paths`), so one order must hold everywhere: the shaders and direct3d-11
  branches' (the three last, in that order) is taken, with G1c's doc of `CopyWithinTexture` kept.
  The direct3d-12 branch's last commit is contained in this line, so it meets no conflict when it
  merges the line (corrected at run 2). Then shaders merged without conflict; the others
  had nothing new. No other owned file changed in the end merges.
- End check after them (scratch `g1c/end-check.log`, laptop, load 2.9): the whole workspace checks,
  the owned crates' clippy and docs are clean with warnings denied, their 233 tests pass, and
  `wcontainers-direct3d-11`'s 5 unit tests pass.
