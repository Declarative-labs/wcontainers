# Direct3D 12 and DXGI infrastructure status (G5)

Branch: phase-2-graphics-direct3d-12. Plan: docs/phase-2-graphics.md, sections 4.9, 5.2, 9, 10 and
G5 in 12. This track owns crates/wcontainers-directx-graphics-infrastructure and
crates/wcontainers-direct3d-12. This file holds the current state only; the full record of runs 1
to 47 (what the front does subject by subject, every run's numbers, older scripts) is this file at
commit cf55bbe8 (`git show cf55bbe8:docs/status-graphics-direct3d-12.md`).

Asked by the Direct3D 11 track (2026-11-22, G4h; the owner's ruling that a device reports no
feature level whose requirements it does not meet): `R10G10B10_XR_BIAS_A2_UNORM` swap chains, the
format's `DISPLAY` (10_0 and up) and `BACK_BUFFER_CAST` (11_0) in Microsoft's tables, are the last
11_0 requirement besides one vertex format, and they live in this track's infrastructure crate
(swapchain.rs, which refuses the format). A route that draws the same: back buffers held in
`Rgb10a2Unorm`'s bits (as textures of the format already are), back buffer views of it as
`R10G10B10A2_UNORM` and the reverse, and the present copy decoding each colour channel as the
functional specification's extended-range mapping, `(c * 1023 - 384) / 510` (-0.7529 to 1.2529),
alpha as stored, into an `rgba16float` surface (a canvas configured `rgba16float` with extended
tone mapping in a browser; wgpu's `Rgba16Float` surface natively, which is linear scRGB, so the
copy also applies sRGB's curve mirrored about 0); where no extended range is shown the values
clamp to 0 to 1, as on a display without it. The Direct3D 11 track may build it if this track
allows, or the orchestrator rules. Also, once the shaders track lands `VertexFormat::Bgrx8Unorm`
(asked in its status file): the row `dxgi::B8G8R8X8_UNORM => (Bgrx8Unorm, 4)` in format.rs
`dxgi_vertex_format`, which gives both fronts the format's `IA_VERTEX_BUFFER`, required from 10_0.

## Where things stand

G5 is ready for the independent check again (run 52, 2026-11-20). The sixth check (at 7a3a7fe0)
left one blocking item and five minor ones, handled in this run:
- Items the gap list said waited on the shaders track, which had delivered them (blocking): all
  three are now used by the front, so nothing on the gap list waits on another track's item that
  is delivered.
  - A pipeline without a pixel shader whose variant needs one takes the shaders crate's
    `builtin::pixel_program_for_variant` (depth clamped into a depth target, clip distances), also
    for its counted draws.
  - Depth clamping the back end lacks is applied behind the stage helper by its last vertex
    stage: a geometry shader converted with `DEPTH_CLAMP` (its pass-through clamps), the domain
    program, and the vertex program the cull route runs (which also passes clip distances the back
    end lacks, the shaders track's answer to the run-51 ask). The draw after the stage helper
    gives the pixel program the viewport's depth range as direct draws do.
  - A texture view of another component kind than the program declares is given in
    `texel_formats` and the program converted again, reading the view's values by their bits.
  Tests: `depth_clamped_in_the_programs_into_a_depth_target_draws_as_the_back_ends_clamp` (GPU)
  now draws seven routes (direct, without a pixel shader, geometry shader, cull route,
  tessellation, and with a clip distance without a pixel shader and through the cull route), each
  with wgpu's own depth clamping and clip distances and with both hidden, two depth ranges each:
  0 pixels differ in all 14 comparisons, and the slow paths are counted exactly where hidden.
  The own program's new case `texture-kinds` loads four textures as another kind than their
  views: our words equal real Direct3D 12's. Still refused by name, the front's own open parts
  (gap list): depth clamping of alpha-antialiased lines, into stream output of a culled vertex
  or domain program, behind a layer-naming geometry shader without a depth format, and of a
  pipeline without a depth format drawing into a bound depth view; a texture of another kind in a
  stage run before a geometry or domain stage, or through a stencil view.
- Also in this run: a shader resource view single-sampled where the program declares a
  multisampled texture (or the reverse) is refused by name as a view of another dimension (not
  to be matched); the back end had lost the device on it (the shader runner's texture-load).
- features-differing.txt's `R32G32B32` lines now say the 11_0 table requires multisample load
  (6, 7, 8) and resolve (6); the gap list and the Departures item name them under section 9's
  multisample count shortfall.
- The WebGPU track's shared constant: `d3d::TEXTURE_DATA_PLACEMENT_ALIGNMENT` is now the one
  definition, in the infrastructure crate, written `1 << 9` like the three resource placement
  alignments beside it (an expression of small numbers, which the literal check does not compare
  by value; as the plain 512 it would be taken as restating `VariantFlags::TEXTURE_INFO`, another
  fact of the same value). `resource::PLACEMENT_ALIGNMENT` is gone with its baseline line;
  `shared_constants_are_the_sdks` ties it to the SDK's value.
- The crates' test count is now written as the owned crates' alone ("Results of run 52").
- bgfx 26-occlusion: the check's two notes for the record are under "Real programs".
- Godot's 3D platformer's one pixel stays listed and untraced.
- No seeded random test: none in these crates takes random input (unchanged).
- Merges (run 52): origin/phase-2-graphics-shaders (the shaders crate's run 58 and G6a work,
  with the Direct3D 11 track's commits it had merged); nothing new on the shared line, the WebGPU,
  WebGL and rig branches or phase-1-literals.
- Section 10's one family library is, by the orchestrator's ruling of 2026-11-20, a joint step
  after G4 and G5 have passed their checks ("Next" 4); it is not part of G5.

The pass bar, item by item:
- Wine's and vkd3d's Direct3D 12 tests giving real Direct3D 12's verdicts on the RTX 3080:
  189 of 198 ("Test files"); every failing file is on the gap list with the absent feature, or
  not to be matched as real Direct3D 12 crashes in it.
- A real Direct3D 12 program drawing matching frames: Microsoft's D3D12HelloTriangle and
  D3D12HelloConstBuffers, bgfx 01-cubes, 26-occlusion, 35-dynamic, 37-gpudrivenrendering and
  38-bloom, and Godot's 2D platformer ("Real programs").
- CheckFeatureSupport answering as section 9 says: the own program's `features` against real
  Direct3D 12, every differing answer listed with its reason in features-differing.txt, and the
  crates' tests of section 9's answers ("CheckFeatureSupport").
- The queue, fence, query, predication, placed-resource and indirect cases against real Direct3D
  12: the own program's 35 cases ("Cases").

For the independent check, one command each (scratch `bin/all52.sh <n>` runs them all):
- a sample of test files: `wine-tests run --dll ... vkd3d-d3d12:test_a/test_b` ("How to run");
- the 35 cases: `check_d3d12 cases` through the own program ("How to run");
- bgfx 26-occlusion repeated: scratch `bin/loop52.sh <tag> 30 26-occlusion record ours 10`;
- the real programs: scratch `bin/real52.sh <n>` (Microsoft's two samples, Godot's two
  platformers, bgfx 37) and `bin/bgfx52.sh <n>` (bgfx 01-cubes, 26-occlusion, 35-dynamic,
  38-bloom).
Before running test files apply scratch run20/rig-wine-run-fix.patch to
tools/graphics-rig/desktop/wine-run.ps1 and undo it afterwards ("How to run"); it is never
committed.

## Results of run 52

2026-11-20, RTX 3080, desktop shared with other tracks unless said; scratch run52/:
- Lint clean (lint2.log to lint4.log). The owned crates' tests: 61 pass, none fail, across their
  binaries 16, 1, 18, 13, 3 and 10 (test2.log, and test4.log at 9c90facc's code); the same command also runs 2 tests of
  wcontainers-gpu (its `slow` filter), not counted here. The GPU test of the seven clamp and clip
  routes is clamp5.log (14 comparisons, 0 pixels differing), and with its slow path counts
  asserted it passes in test2.log and test4.log. Literal check passes (lit3.log, lit4.log at 9c90facc,
  marks 0).
- Test files 189 of 198 (full2.log, logs-full2/; again after the last code change: full4.log, logs-full4/, at 9c90facc's code):
  the same 9 failures as runs 48 to 51, each on the gap list; the refusals the logs print are the
  same as run 51's.
- Own program (check2.log, check3.log, check4.log): draw 0 of 4096 pixels beyond 1; features 166 of 872
  differ, each listed under its cause; outputs 22 of 110 and answers 2 of 40 differ, each listed;
  35 of 35 cases equal (new `texture-kinds`); `antialiased-lines` 0 of 4096 beyond 63, largest
  difference 38.
- Shader runner 214 of 302 (runner2.log; runner4.log at 9c90facc's code), the same 88 failing files.
- Scenes 95 pass, 22 fail, 3 absent, the same outputs as run 51 (scenes1.log, at 9c90facc). Wine's five dxgi tests not rerun (nothing of dxgi changed).
- Real programs: Microsoft's two samples equal (0 pixels beyond 8, p1-samples.log); Godot's 2D
  platformer equal (0 of 518,400 beyond 8); bgfx 37 (p1-bgfx37.log, 711 to 805 pixels beyond 8)
  and 01-cubes, 26-occlusion, 35-dynamic and 38-bloom (b1-*.log, references from 6 real runs,
  762 to 1204 beyond 8) every frame of both our runs within the allowance of 20,480 (the
  overlay's text). Godot's 3D platformer the same one pixel as before (1 of 518,400 beyond 8,
  difference 48, frames 10, 60 and 120).
- Fence round trip `--alone`: see "Measurements".

## Test files (Wine's and vkd3d's, through our libraries, RTX 3080)

How a file counts: the rig compares each test's verdict with real Direct3D 12's on this GPU; a
test that skips what real runs, or runs fewer checks, fails. Tests failing some checks on real
are held to every other check. The counted set is 198 files: d3d12 (Wine's) 10,
vkd3d-d3d12_invalid_usage 3, vkd3d-hlsl_d3d12 9, vkd3d-d3d12 176.

Failing at run 52 (run52/full2.log, logs-full2/, with the rig patch), each on the gap list: d3d12 10 of 10, vkd3d-d3d12_invalid_usage 3 of 3, vkd3d-hlsl_d3d12 9 of 9, vkd3d-d3d12
167 of 176. The 9:
- test_create_reserved_resource: ran 4 checks of real's 22, skipped 1. Tiled resources absent.
- test_get_resource_tiling: ran 11 of 1410, skipped 1. Tiled resources absent.
- test_output_merger_logic_op: ran 9 of 347, skipped 1. Logic operations absent.
- test_create_graphics_pipeline_state: d3d12.c:3487, a pipeline with a logic operation made
  without asking refused (E_NOTIMPL); the test then crashes on its null pipeline. Logic
  operations absent.
- test_shader_instructions: ran 4455 of 4759, skipped 28 against real's 2. Float64 absent.
- test_unbounded_resource_arrays: ran 7 of 2922, skipped 1. Resource binding tier 2 absent.
- test_unbounded_samplers: ran 7 of 97, skipped 1. Resource binding tier 2 absent.
- test_ps_layer: d3d12.c:28593, a pipeline whose vertex shader writes the layer refused
  (E_NOTIMPL). The layer from the vertex shader absent.
- test_copy_block_compressed_texture: 5 checks, the first d3d12_test_utils.h:669. Not to be
  matched (real Direct3D 12 crashes in it).
The refusals the counted test files printed (run 48's logs-full2): "logic operations without LOGIC_OP"
(create_graphics_pipeline_state) and "a shader feature the converter lacks"
(`RenderTargetArrayIndex`, ps_layer); none other.

Whole counts by run: 129 of 198 (run 17), 157 (20), 164 (25), 169 (28), 180 (31), 184 (34), 186
(35 to 45), 189 (46), 189 (48), 189 (49), 189 (50), 189 (51), 189 (52).

## Gap list

Every failing test file, shader runner file, scene output and real-program difference, and every
refusal by name in the front, is one of: an absent feature (reported absent as Direct3D 12 asks,
or refused by name where Direct3D 12 has no query), a listed loss naming what the back end lacks
(counted and warned), not to be matched (real Direct3D 12 crashes, or Direct3D leaves it
undefined), a named item of another track, or the front's own open part. Every refusal prints
its name ("wcontainers: ..."); a program meeting one gets E_NOTIMPL, E_INVALIDARG or a removed
device, never a fake success.

Failing test files:
- Absent, tiled resources (`TiledResourcesTier` 0): create_reserved_resource,
  get_resource_tiling.
- Absent, logic operations (`OutputMergerLogicOp` false): output_merger_logic_op;
  create_graphics_pipeline_state, which makes a pipeline with a logic operation without asking
  (d3d12.c:3487, refused, E_NOTIMPL) and then releases the pipeline it did not get, a null
  pointer, and crashes in its own code (0xc0000005; no panic in our library). Its 34 checks
  after that line (among them independent blending with a logic operation, d3d12.c:3491 on) are
  therefore never compared with real. What real Direct3D 12 answers on hardware without the
  ability (E_NOTIMPL here) is not known: every adapter at hand has it.
- Absent, float64 in shaders (`DoublePrecisionFloatShaderOps` false): shader_instructions.
- Absent, resource binding tier 2 (tier 1 answered where the back end lacks big tables):
  unbounded_resource_arrays, unbounded_samplers.
- Absent, the layer from the vertex shader
  (`VPAndRTArrayIndexFromAnyShaderFeedingRasterizerSupportedWithoutGSEmulation` false, plan
  4.12): ps_layer, whose context makes such a pipeline before asking (`RenderTargetArrayIndex`).
- Not to be matched: copy_block_compressed_texture (real Direct3D 12 crashes in it: its texture
  of part of a block is refused on both, and the test then uses the null texture). Before the
  crash the two part on when the device is removed: after `CopyTextureRegion` of the null texture
  `Close` answers E_INVALIDARG on both, but real has already removed the device (its
  `CreateFence` fails with 0x887a0005, d3d12_test_utils.h:669), where ours removes it only at
  `ExecuteCommandLists` of the list that did not close (its `CreateFence` succeeds, its `Signal`
  fails). Both remove the device; only when differs, on a call Direct3D leaves undefined.

Formats CheckFeatureSupport answers below real, against section 9 (features-differing.txt names
each line under its cause):
- The other video formats (`AYUV`, `Y410`, `Y416`, `P010`, `P016`, `Y210`, `Y216`, `NV11`,
  `AI44`, `IA44`, `P8`, `A8P8`): answered absent; the 11_0 table requires abilities of most of
  them, the plan names no route (Departures).
- Drawing, blending, multisampling and storing into `B5G6R5` (85), `B5G5R5A1` (86) and
  `B4G4R4A4` (115): answered absent; sampling them is served (`FormatConverted` to
  `rgba8unorm`). WebGPU has no 16-bit packed format, and an `rgba8unorm` target keeps blended
  values at 8 bits where Direct3D keeps 4, 5 or 6, so no exact route exists (Departures). The
  11_0 table requires `B5G6R5`'s RenderTarget, blending and 4x and 8x multisampled targets.
- The video abilities of every format (decoder output, video processor input and output),
  section 9's named shortfall; `NV12`, `420_OPAQUE` and `YUY2` are served as ordinary textures
  (plan 4.12) with every other ability Direct3D defines for them.
- `A8_UNORM` (65) typed unordered access views (Typed UAV and UAV Typed Store, which the 11_0
  table requires; Typed Load): section 9 serves them as `rgba8unorm` storage or by
  `StorageFormatThroughBuffer`, but the WebGPU back end gives `A8Unorm`, which it holds as
  `rgba8unorm` (`FormatConverted`), no storage use, and the front's buffer route is not built for
  a format the back end holds converted. The front's own open part, with the ask of G2 below.
- `B8G8R8X8_UNORM` (88) typed buffer views and vertex input (Buffer and Input Assembler Vertex
  Buffer, required): the interface's `VertexFormat` has `Bgra8Unorm` only, which reads the fourth
  byte as alpha, where Direct3D reads one; a named item of the interface track (G1, asked below).
- `R10G10B10_XR_BIAS_A2_UNORM` (89), the whole format (Texture2D, Display Scan-Out, Cast Within
  Bit Layout and BackBuffer Castable required), and `R10G10B10A2_UNORM` (24) back buffers cast to
  it (BackBuffer Castable Even Fully Typed, required): the format has no shader, target or storage
  use at any level; it exists to be shown in the extended-range bias encoding, cast to and from
  the 10-bit layout and copied for that, and no browser canvas shows that encoding (a canvas
  shows `bgra8unorm`, `rgba8unorm` or `rgba16float`, in sRGB or Display P3). Absent, not built.
- `R32G32B32_FLOAT`, `_UINT` and `_SINT` (6, 7, 8) multisample load (required by the 11_0
  table for all three) and `R32G32B32_FLOAT` multisample resolve (required): WebGPU renders no
  three-channel format, so none is multisampled; section 9's named multisample count shortfall.
- Atomics on typed storage views of `R32_UINT`, `R32_SINT` (every operation) and `R32_FLOAT`
  (exchange) are answered present (run 51): the front serves them through a buffer
  (`AtomicsThroughBuffer`, counted and warned); the fifth check's own case (scratch
  icheck50/case-icheck-texture-atomics.log) gave real's atomic results through ours. The
  route's own refusals stand on the list below (in vertex and geometry shaders, multisampled, past
  slot 63, behind a geometry or domain shader).

Also absent, with no failing test file: conservative rasterisation, raster order views, ray
tracing, mesh shaders, variable rate shading, sampler feedback, 64-bit integers, depth bounds,
stencil export (answered absent whatever the back end has, section 9); timestamps where the
adapter lacks `timestamp-query`; drawing into and storing into `B5G6R5`, `B5G5R5A1`,
`B4G4R4A4`; minimum and maximum filters (no query in Direct3D 12; Microsoft's `D3D12_FILTER` page
sends the reader to Direct3D 11's `MinMaxFiltering`), refused at use.

A listed loss: `ExecuteIndirect` draws without indices whose arguments lie in GPU memory and that
fetch past a vertex buffer's end are skipped whole by WebGPU, where Direct3D 12 reads zeros
(`IndirectFetchPastEnd`, counted and warned). Direct draws, and indirect ones whose arguments
lie in an upload heap, are exact (`VertexFetchZeroed`).

Every refusal by name in the front (107 refusal sites by run 51's count, one more than then:
three depth clamping refusals and the pixel program for a variant's own replace three; from
`NotImplemented` in src/; `cargo test` of none of them fails; which ones the test files, the
runner, the scenes and the programs printed in run 48 is under "Results of run 48"). By cause:

Absent features or back ends a Direct3D 12 device is never made on:
- Answered absent by Direct3D 12's own query, refused at use: "logic operations without
  LOGIC_OP", "forced sample counts and conservative rasterization", "mesh shader pipeline
  statistics", "video decode statistics queries", "timestamps on this back end" (no
  `timestamp-query`), "minimum and maximum filters" (no query, above), "custom heaps" (custom heap
  properties this adapter does not have, as `ARCHITECTURE` answers).
- Back ends without what feature level 11_0 needs, on which `D3D12CreateDevice` makes no device
  (section 9, `LEVEL_11_0_FIXED_FUNCTION`, test `no_device_without_compute_11_0_sizes_or_d32_s8_targets`):
  "compute shaders on this back end", "geometry shaders, stream output and tessellation on this
  back end", "a depth bias clamp", "independent blending without INDEPENDENT_BLEND", "a partial
  sample mask without SAMPLE_MASK".
- Invalid in Direct3D 12, refused before this is reached: "a depth bias with no primitive
  topology type" (E_INVALIDARG first, case `undefined-topology`), "patch lists" and "patch
  topologies without a hull shader", "adjacency topologies" (direct adjacency draws drop the
  adjacent vertices, `AdjacencyDropped`; this names the others, below).

Named items of the shaders track (G6), printed with the converter's or stage helper's own name:
"a shader feature the converter lacks", "a shader the converter got wrong", "a geometry, hull or
domain shader the converter got wrong", "a shader feature the stage helper lacks", "a vertex
shader fetching its inputs did not convert" ("Shader runner" names each file's).

Not to be matched (Direct3D 12 leaves it undefined): "a shader resource view of another
dimension than the program declares" (since run 52 also a single-sampled view where the program
declares a multisampled texture, or the reverse, which the back end refused by losing the device)
and "an unordered access view of another dimension than
the program declares" (one-layer 2D arrays bound to 2D declarations are bound as 2D views);
"a shared buffer read after an overlapping one was written in the same submission".

The front's own open parts, each a route not built (no counted test file, case or real program
meets one on the RTX 3080; "Next" 2). Those only met where the back end lacks a feature name it;
browsers' WebGPU lacks `depth-clip-control`, `primitive-index`, polygon modes and texel buffers
on some adapters, and WebGL 2 makes no Direct3D 12 device:
- Depth clamping where the back end lacks `depth-clip-control` (`Features::DEPTH_CLAMP`):
  `DepthClampInShader` serves direct draws, pipelines without a pixel shader (the shaders crate's
  `builtin::pixel_program_for_variant` writes the clamped depth), and draws behind a geometry
  shader (its pass-through clamps), tessellation (the domain program clamps) and the cull route
  (the vertex program clamps); test
  `depth_clamped_in_the_programs_into_a_depth_target_draws_as_the_back_ends_clamp` draws each
  against wgpu's own clamp (run 52). Refused by name: "depth clamping the back end lacks, of
  alpha-antialiased lines" (the line route's coverage values are linked where the unclamped
  depth would pass), "... in the culled vertex or domain program stream output records" (the
  records would hold the clamped position), "... behind a geometry shader naming a layer, in a
  pipeline made with no depth-stencil format" (the pass-through passes the layer after a depth
  value the pixel program, which writes no depth there, does not take), and "... in a pipeline
  made with no depth-stencil format drawing while a depth-stencil view is bound" (its pixel
  variant would be made at the draw).
- Clip distances where the back end lacks them (`Features::CLIP_DISTANCE`):
  `ClipDistanceInShader` serves direct draws, pipelines without a pixel shader (the pixel program
  for the variant), geometry and domain shaders, and the cull route (the vertex program passes
  them, the pass-through passes them on); the same test draws the two new routes with wgpu's clip
  distances hidden (run 52). Refused by name: "clip distances from a geometry, domain, culling or
  antialiased line stage without the back end's (ClipDistanceInShader)", now met only by the
  antialiased line route and a culled vertex shader with stream output.
- Wireframe where the back end lacks polygon modes: "wireframe without the back end's polygon
  mode (FillModeConverted, which needs the stage helper's route)". Direct3D culls triangles
  before drawing their edges, so the route needs each triangle's positions after the vertex
  shader, through the stage helper's vertex capture (Departures).
- The primitive index where the back end lacks it: "the primitive index without the back end's
  (PrimitiveIdAsGeometry)" (the stage helper's `primitive_numbered` route is not used yet).
- Unordered access views: "storage views of part of a 3D texture" (`FirstWSlice` not 0: WebGPU's
  3D views cover every slice; needs the slice offset added in the program), "clearing a
  structured buffer view whose stride is no multiple of 4", "float clears of raw and structured
  buffer views", "clearing a typed buffer view of this format", "clearing an unordered access
  view of this format", "clearing rectangles of a 3D texture's view in another format", "null
  views of this storage texture", "null views of one-dimensional textures", "typed buffer views
  bound as storage textures", "unordered access buffers in a custom heap the CPU sees".
- Views and binding: "a buffer view starting inside one of the program's elements", "a constant
  buffer view shorter than the program's block, or past its buffer's end, of a buffer not in an
  upload heap", "a texture read as another type than its view's format, of a stencil view or in
  a geometry, hull or domain shader or a vertex shader before one" (elsewhere the program is
  converted again reading the view's values by their bits, `VariantKey::texel_formats`, run 52;
  a sample of such a view the converter refuses, `SampledViewKind`), "a view of several layers of a
  multisampled texture array", "a view reinterpreting a texture's bits without its twin",
  "arrays of resources in a shader" (beyond those the root signature bounds), "buffers read
  through views", "this kind of root binding", "a multisampled texture array viewed in a format
  it cannot be viewed as", "a multisampled texture viewed in a format reinterpreting its bits",
  "comparison sampling of a multisampled texture not stored in a depth format", "comparison
  sampling of a texture of no depth format's bits", "a resolve in a format reinterpreting the
  stored bits".
- Sampling and formats in the program (`FilterInShader` and the like) in stages before the pixel
  shader: "a component mapping or stencil view read in a geometry, hull or domain shader or a
  vertex shader before one", "level-of-detail bias, border or mirror-once sampling in a
  geometry, hull or domain shader or a vertex shader before one", "a minimum level-of-detail
  clamp in a vertex or geometry shader or past texture slot 127", "a multisampled array of
  several layers read in a vertex, geometry, hull or domain shader", "sampling in the program
  through a sampler slot past 15", "typed views of other formats in a vertex shader", "a pixel
  program converted for the bound views and samplers behind a geometry or domain shader", "a
  program for the bound views and samplers", "shaders reading helper constants the front does
  not write", "a helper value the front does not write".
- Storage textures read through buffers (where the converter refuses atomics on them):
  "storage textures read through a buffer in a vertex or geometry shader", "multisampled storage
  textures read through a buffer", "a storage texture read through a buffer at a slot past 63",
  "storage textures read through buffers behind a geometry or domain shader".
- Vertex input fetched in the program: "a vertex input fetched in the program from no vertex
  buffer", "a per-instance element beyond 4 GiB", "an indirect draw of a per-instance stream
  fetched in the program that steps other than once an instance", "a per-instance stream that
  never steps in an indirect draw", "a vertex buffer view read past its end at an address not a
  multiple of 4".
- Draws: "indexed or indirect adjacency draws without a geometry shader", "a predicated draw
  with a first instance on this back end", "a predicated strip draw whose indices restart at
  another value", "fractional viewports" (where the back end lacks `FRACTIONAL_VIEWPORT`; the
  `ViewportInShader` variant is not used yet), "alpha-antialiased lines without a pixel shader", "alpha-antialiased
  lines after a geometry or tessellation stage or stream output, or of wireframe triangles".
- Geometry, tessellation and stream output through the stage helper: "a draw choosing a layer of
  target views of different layer counts", "a draw choosing the slice of a 3D texture's target
  view", "a draw split per layer or viewport while a query runs", "a draw through a geometry,
  hull or domain shader or stream output while a counted occlusion or statistics query runs", "a
  predicated draw through a geometry, hull or domain shader or stream output", "an indirect draw
  through a geometry, hull or domain shader or stream output", "a stream output buffer at an
  offset the back end cannot bind", "drawing a stream other than 0", "patches of another control
  point count than the hull shader takes".
- Queries: "an occlusion query across passes", "counting pixel shader invocations inside a
  binary occlusion query", "counting pixel shader invocations of a draw fetching its streams in
  the program", "a counted draw of a vertex program converted for the bound views", "a counted
  draw of vertex inputs fetched in the program", "a counted draw reading storage textures
  through buffers".
- Early depth-stencil tests in the program (`EarlyTestsInShader`): "early depth-stencil tests in
  the program behind a geometry or tessellation stage", "... on a depth buffer denying shader
  resources", and per primitive "of a pixel shader reading its primitive index", "of a predicated
  draw or while a query runs", "of an indexed strip", "of an indirect draw", "of an instanced
  draw", "of an adjacency draw", "of a vertex program converted for the draw".
- "a feature the back end lacks": a back end's own `GpuError::Absent` passed on with the bit's
  name.

The Windows library's COM methods answering E_NOTIMPL, each printing "wcontainers: X::Y is not
implemented" (87 names, `unsupported` in examples/directx; list them with `grep -oh
'unsupported("[A-Za-z0-9]*")' examples/directx/*.rs | sort -u`). By cause:
- Abilities Direct3D 12's own queries answer absent (section 9): ray tracing
  (`CreateStateObject`, `AddToStateObject`, `SetPipelineState1`, `DispatchRays`, the four
  acceleration structure methods), mesh shaders (`DispatchMesh`), tiled resources
  (`CreateReservedResource`, `CreateReservedResource1`, `GetResourceTiling`,
  `UpdateTileMappings`, `CopyTileMappings`, `CopyTiles`), sampler feedback
  (`CreateSamplerFeedbackUnorderedAccessView`), variable rate shading (`RSSetShadingRate`,
  `RSSetShadingRateImage`), depth bounds (`OMSetDepthBounds`), view instancing
  (`SetViewInstanceMask`), programmable sample positions (`SetSamplePositions`), shared
  resources (`CreateSharedHandle`, `OpenSharedHandle`, `OpenSharedHandleByName`,
  `GetSharedResourceAdapterLuid`, `OpenExistingHeapFromAddress`,
  `OpenExistingHeapFromFileMapping`), protected sessions (`CreateProtectedResourceSession`,
  `CreateProtectedResourceSession1`, `SetProtectedResourceSession`, `CreateLifetimeTracker`),
  meta commands (`InitializeMetaCommand`, `ExecuteMetaCommand`: none enumerated), cached
  pipeline blobs (`PipelineState::GetCachedBlob`: `SHADER_CACHE` answers no single-pipeline
  cache; every bgfx run calls it and goes on without one).
- No software adapter is offered: `EnumWarpAdapter`, `CreateSoftwareAdapter` (Wine's two dxgi
  tests below).
- Parts of a Windows display a browser's canvas has no counterpart of: full screen and outputs
  (`GetFullscreenDesc`, `ResizeTarget`, `TakeOwnership`, `ReleaseOwnership`, `WaitForVBlank`,
  `GetFrameStatistics`, `GetDisplaySurfaceData`, `GetDisplaySurfaceData1`,
  `SetDisplaySurface`, the three gamma control methods, `DuplicateOutput`, `DuplicateOutput1`,
  `CheckOverlaySupport`, `CheckOverlayColorSpaceSupport`, `GetRestrictToOutput`), composition
  and CoreWindow swap chains (`CreateSwapChainForComposition`, `CreateSwapChainForCoreWindow`,
  `GetCoreWindow`, `GetSourceSize`, `SetSourceSize`, `GetMatrixTransform`,
  `SetMatrixTransform`, `GetRotation`, `SetRotation`, `GetBackgroundColor`,
  `SetBackgroundColor`), status notifications (`Register` and `Unregister` of occlusion,
  stereo, content protection teardown and video memory budget), memory budgets
  (`QueryVideoMemoryInfo`, `SetVideoMemoryReservation`: no browser reports memory, Departures).
- The front's own open part (no counted test file, case or real program calls one; of all 87,
  bgfx calls `GetCachedBlob`, `QueryVideoMemoryInfo` and `CreateReservedResource`, the last
  once a run, absent above, and goes on without it): `IDXGIObject`'s
  `GetPrivateData`, `SetPrivateData` and `SetPrivateDataInterface` on the factory, adapter,
  output and swap chain and its `GetParent` on the factory; `CheckInterfaceSupport`;
  `ResizeBuffers1`; `ClearState`; `AtomicCopyBufferUINT`, `AtomicCopyBufferUINT64`;
  `ResolveSubresourceRegion`; `SetStablePowerState`.

Other differences beyond the bar, each named: the shader runner's failing files ("Shader
runner"), the scenes' failing outputs ("Scenes"), Wine's two dxgi tests that need a software
adapter (none is offered; `EnumWarpAdapter` answers E_NOTIMPL), and Godot's 3D platformer's one
pixel (1 of 518,400 beyond 8, difference 48 at 921,317 in frames 10, 60 and 120; not traced, the
front's own open part).

## CheckFeatureSupport

The answers are section 9's: feature level 11_0 at most (`LEVEL_11_0_FIXED_FUNCTION` is only what
core WebGPU has: a depth bias clamp, independent blending, a partial sample mask; depth clamping
the back end lacks is done in the programs, wireframe it lacks is refused by name), resource
binding tier 1 without big resource tables (tier 3 with them, never 11_1), shader model 6.0,
everything section 9 reports absent answered absent. Format answers: what the back end holds
natively or by the front's exact routes (`FilterInShader`, twin textures for casts,
`StreamOutAsCompute` for `SO_BUFFER`), every texture shape the format's blocks allow; `NV12`,
`420_OPAQUE` and `YUY2` what their planes' or pixel pairs' formats have of the abilities
Direct3D defines for them (`video_format_support`). Tests:
`webgpu_reports_11_0_and_tier_1`, `resource_tables_give_tier_3_but_not_11_1`,
`no_device_without_compute_11_0_sizes_or_d32_s8_targets`,
`browsers_without_polygon_mode_or_depth_clamping_get_11_0`,
`format_support_follows_the_back_ends_format_uses`. The own program's `features` compares every
answer with real Direct3D 12's: an answer above real fails unless served
(`SERVED_BEYOND_REAL`: `R32G32B32_FLOAT` sampled and gathered), an answer below real fails unless
listed with both values in examples/check/features-differing.txt under section 9's reason, and a
listed line that no longer holds fails. `outputs` and `answers` are compared the same way, with
their own lists (examples/check/outputs-differing.txt: the 21 `CheckColorSpaceSupport` answers for
studio-range, BT.2020, PQ and 2.4-gamma spaces on `R10G10B10A2`, `R8G8B8A8` and `B8G8R8A8`, which
real presents on this desktop and a canvas cannot show, and the output's primaries, the monitor's
own on real and BT.709's on ours; answers-differing.txt: the two refusals, each with its cause).

## Cases (own program against real Direct3D 12)

`check_d3d12 cases` runs each case in its own process on real Direct3D 12 and through ours and
compares the lines exactly (examples/check/cases.rs); `antialiased-lines` also compares its
picture within 63 of 255 (Direct3D leaves the line filter to the implementation). The 35 cases:
- Placed resources: alias-copy, alias-storage, alias-cpu (overlapping placed buffers written by a
  copy, a root UAV and the CPU, read through the other), alias-texture, custom-heap.
- Queues and fences: queues (copy and compute queues waiting on each other and on a late CPU
  signal), timestamps (both queues, calibrations, resolved to default and readback heaps).
- Queries and predication: predication, predicated-resolve, occlusion-count, statistics.
- Indirect: indirect-draw, indirect-count, indirect-dispatch, indirect-short,
  indirect-vertex-buffer, indirect-vertex-id.
- Others: depth-planes, vertex-past-end, vertex-pulled, restart, render-pass, ms-array, geometry,
  layers, antialiased-lines, filter-depth, vertex-views, line-bias, markers, undefined-topology,
  no-scissors, early-per-primitive.
- Formats: video-formats (`NV12`, `420_OPAQUE` and `YUY2` stored as ordinary textures),
  texture-kinds (textures of `R8G8B8A8_UINT`, `R32G32B32A32_FLOAT`, `R8G8B8A8_SINT` and
  `R8G8B8A8_UNORM` loaded by a compute shader declaring another component kind; new in run 52).

## Shader runner (vkd3d-shader_runner, 302 files)

Run with our d3d12.dll alone (our dxgi.dll serves Direct3D 12 only and every file starts with a
Direct3D 11 part). Run 52 (run52/runner2.log): 214 of 302, the same 88 failing files as runs 34
to 51. Each failing file's cause was sorted in run 44 (scratch `bin/classify30.py`; the list is
in this file at cf55bbe8, "Shader runner"): 61 run fewer checks than real with none failing (50
shader models above 6.0, which section 9 does not answer; the rest 64-bit integers, stencil
export, depth bounds, raster order views, the layer and viewport index from any stage, all
absent); 27 fail some checks (2 absent features, 3 not to be matched, 22 named items of the
shaders track). texture-load, which run 44 counted as refused for a texture read as another type
than its view (lines 235 to 267, a route built in run 52 and compared with real Direct3D 12 by
case `texture-kinds`), has since run 46 ended earlier, at line 51: a `Texture2DMS` declaration
bound to a view of a single-sampled texture, a view of another dimension than the program
declares, which Direct3D 12 leaves undefined (not to be matched; real draws it; ours refuses it
by name since run 52, where the back end lost the device before, and the device is removed, so
the lines after it are not reached).

## Scenes (the rig's Direct3D 12 scenes, beyond the bar)

Run 52 (run52/scenes1.log): 95 outputs pass, 22 fail, 3 absent, the same outputs as runs 38 to 51. The failing outputs: raster, geometry, streamout, tessellation (DXIL geometry and
hull shaders and one pixel shader the converter refuses, the shaders track's); topology (43
pixels beyond 1, all in line cells: the aliased line listed loss), samplers-filters (anisotropy
cells) and msaa-alpha_to_coverage, which draw with largest difference 0 on wgpu's Direct3D 12
path (run 36): the NVIDIA driver's Vulkan path this library draws on ("Next" 3). Absent: `B5G6R5`
targets, minimum and maximum filtering.

## Real programs

RTX 3080, desktop shared with other tracks, not `--alone`, debug builds, 120 frames. Frames 10, 60
and 120 compared with real Direct3D 12's at 8 of 255.
- Run 52: as listed under "Results of run 52". Run 51 the same. Run 50: the same, bgfx against two real runs. Run 49: the same but times (Godot's 3D platformer
  10 s), and bgfx 741 to 1225 pixels beyond 8. Run 48 (run48/p1-*.log at 3dfef503, p2-*.log at 708bd126; b1-*.log): Microsoft's
  D3D12HelloTriangle and D3D12HelloConstBuffers equal (0 pixels beyond 8; 2.6 and 2.5 s);
  Godot's 2D platformer equal (0 of 518,400 beyond 8; 3.8 s); bgfx 37-gpudrivenrendering,
  01-cubes, 26-occlusion, 35-dynamic and 38-bloom every frame of both our runs within the
  allowance (751 to 1218 pixels beyond 8 of 20,480 allowed, the overlay's text). Godot's 3D
  platformer runs 120 frames (10.1 s) and draws frames 10, 60 and 120 each with 1 of 518,400
  pixels beyond 8, largest difference 48 at 921,317 (gap list).
  At 708bd126 (p2-*.log) the same: the samples equal (2.7 and 2.5 s), Godot's 2D platformer
  equal (3.8 s), bgfx 37 every frame of both runs within its allowance, Godot's 3D platformer
  the same one pixel (10 s).
- bgfx is compared with the overlay's GPU time and memory lines ignored (rectangle 0,208,320,64);
  24-nbody draws differently on every real run, so it has no frames to compare.
- bgfx 26-occlusion, for the record (sixth check): with references from several real runs, a
  tile any real run draws differently is not compared, so in a session where a real run draws
  frame 10's rare outcome, frame 10's occlusion tiles are not checked for ours at all. Ours shows
  "L: 0" in the overlay's latency counter where real shows "L: 1": ours never runs a frame
  behind, which is why its occlusion result latency is the same in every frame. Not measured
  further.
- Setup learnt on the way: the rig's forwarding library takes the name d3d12.dll and loads ours
  behind it (`WC_D3D12`), hence `wcontainers_d3d12.dll` and two forwarders; it aborts a program
  calling an export the library behind it lacks; bgfx calls `QueryVideoMemoryInfo` on the adapter
  `EnumAdapters` gives; NVIDIA's NVAPI calls into our device as if it were the driver's
  (harmless, absent in a container); a Rust panic in the library shows as 0xc0000409.

## Asked of other tracks

- WebGPU back end (G2), new in run 48: plan 4.11 and the interface's `FormatCaps` say BC formats
  are served in 3D textures, held decoded where the API lacks it. The back end sets
  `BLOCK_COMPRESSION` from `texture-compression-bc` alone and makes 3D BC textures natively; on an
  adapter with `texture-compression-bc` but without `texture-compression-bc-sliced-3d` such a
  texture fails. Asked: hold 3D BC textures decoded there (`BlockDecoded`). The RTX 3080 has both.
- WebGL 2 back end (G3): the same for S3TC and BPTC, which WebGL takes in 2D arrays only.
- WebGPU back end (G2), run 52: `d3d::TEXTURE_DATA_PLACEMENT_ALIGNMENT` is in the infrastructure
  crate, as asked; its `DX12_PLACEMENT_ALIGNMENT` can import it.
- WebGPU back end (G2), new in run 51: section 9 serves `A8_UNORM` storage stores as
  `rgba8unorm` storage where `texture-formats-tier1` is present; the back end gives `A8Unorm`
  (held as `rgba8unorm`) no storage use, so `A8_UNORM` typed unordered access views are answered
  absent (gap list).
- Interface (G1), new in run 51: a vertex format for `B8G8R8X8_UNORM` (bytes B, G, R, unused,
  read with alpha one), for its vertex input and typed buffer views, which the 11_0 table
  requires (gap list).
- Shaders (G6), earlier and still open (evidence in the record at cf55bbe8): the converter
  refusals the runner prints (`TypedBufferFormatKind`, `ClipDistance` read by a pixel shader,
  DXIL `InterpolateAt`, `DxilGeometryShader`, `DxilHullShader`, `CalculateLevelOfDetail`,
  `DepthGatherComponent`, `SwizzledDepth` moving red, `PrimitiveId`, `SampleClamp`,
  `SparseFeedback`, `globallycoherent` memory operands, unbounded arrays on the DXIL route); values
  differing from Direct3D's (a centroid `SV_Position` per sample, `min16float` constants, float
  subnormals in shader models 4 and 5, DXIL `frexp` of a NaN, the shader model 4.0 multisampled
  load in a loop). (The pixel program for a variant alone, `DEPTH_CLAMP` on the pass-throughs, a
  texture view read in another component kind and clip distances through the cull route were
  delivered and are used since run 52.)
- WebGPU back end (G2), on wgpu's Direct3D 12 path: clears through a float that keeps the bits
  (an `R32_FLOAT` denormal), and buffer-to-texture copies of several slices whose
  `rows_per_image` exceeds the copy's height (wgpu-hal 30's placed footprint).

## Next, in this order

1. At the start of each run merge the shared line and every graphics branch with something new,
   and origin/phase-1-literals; keep the literal check passing (after a merge bringing new rules,
   run `init` once). Apply the rig patch uncommitted for test file runs (below).
2. Fix only what the independent check finds. The front's own open parts, beyond the bar: the
   four remaining depth clamping refusals, wireframe without polygon modes (`FillModeConverted` through the stage helper's vertex
   capture), Godot's 3D platformer's one pixel (trace which draw writes it, on real and ours), the
   other refusals of the gap list, recording state combinations of other vertex strides in the
   cache file.
3. Draw on wgpu's Direct3D 12 path as browsers do on Windows (run 36's experiment, scratch
   run36/dx12-path.patch, measured equal in the three scenes above; it broke three things, each
   another track's: the culling stage's clip distances, the two wgpu-hal items above, and the
   rig's forwarder wrapping wgpu's own device).
4. Plan section 10's one library (`crates/wcontainers-directx`): by the orchestrator's ruling of
   2026-11-20, joining this track's COM objects with the Direct3D 11 track's is one joint step
   after both G4 and G5 have passed their checks, not part of G5; it also lets Wine's other 18
   dxgi tests be counted. Plan 4.8's call buffer comes with it.
5. The literal sweep, a part per run.

## How to run

All on the Windows desktop through the rig; on the Mac only `nix develop -c cargo fmt`. Every
build command starts with `set CARGO_INCREMENTAL=0&& ` (stale incremental objects once failed to
link). Run one rig command of this branch at a time, and do not change the working tree while a
`gpu-job` of it waits (the job refuses another tree). Scratch bin/ (this track's scratch
directory) holds run 52's scripts (run 51's with run52 paths; `programs52.sh` for Godot and bgfx 37,
`bgfx52.sh` with references of 6 real runs, `loop52.sh <tag> <count> [example] [record|norecord]
[ours|real] [real runs]` for repeated runs of one bgfx example): `quick52.sh <n>` (lint, the crates' tests, the literal check),
`full52.sh <n>` (lint, tests, every test file, the own program, the shader runner), `own52.sh <n>`
(lint, build, the own program), `scenes52.sh <n> [scenes]`, `real52.sh <n>`, `bgfx52.sh <n>`,
`all52.sh <n>` (everything, with Wine's five dxgi tests and the `--alone` fence timing); logs land
in scratch run52/.
- Lint: `tools/graphics-rig/remote "cargo clippy -j 3 --all-targets -p wcontainers-gpu -p
  wcontainers-directx-graphics-infrastructure -p wcontainers-direct3d-12 -- -D warnings"`.
- Tests (three draw on the GPU): `tools/graphics-rig/gpu-job --build "cargo test -j 3 -p
  wcontainers-directx-graphics-infrastructure -p wcontainers-direct3d-12 --no-run" -- "cargo test
  -j 3 --no-fail-fast -p wcontainers-directx-graphics-infrastructure -p wcontainers-direct3d-12"`.
  `WEBGPU_API=dx12|vulkan` picks wgpu's path for the GPU tests.
- Literal check: `tools/graphics-rig/remote "cargo run -j 3 -q -p wcontainers-literals -- check"`.
- The libraries: `wcontainers_d3d12.dll` holds everything; `d3d12.dll` and `dxgi.dll` forward to
  it by that name; all three go beside a program. Build: `tools/graphics-rig/remote "cargo build
  -j 3 -p wcontainers-direct3d-12 --examples"`.
- Test files: `E='%CARGO_TARGET_DIR%\debug\examples'; tools/graphics-rig/wine-tests run --dll
  "wcontainers_d3d12.dll=$E\wcontainers_d3d12.dll" --dll "d3d12.dll=$E\d3d12.dll" --dll
  "dxgi.dll=$E\dxgi.dll" d3d12 vkd3d-d3d12 vkd3d-d3d12_invalid_usage vkd3d-hlsl_d3d12` (or
  `vkd3d-d3d12:test_a/test_b`). Logs land in pyro-parked/wcontainers-scratch/graphics-rig/wine-logs.
  The shader runner: the same with `vkd3d-shader_runner` and the first two `--dll`s.
- The rig patch: the rig's comparison stops at the first test failing a check real passes
  (`$allowed` is `$null` for a test with no known failures). Before test file runs, `git apply`
  scratch run20/rig-wine-run-fix.patch (it replaces the `$allowed` and `$worse` lines of
  tools/graphics-rig/desktop/wine-run.ps1 with `$worse = @($run.Failed.Keys | Where-Object { -not
  ($known.ContainsKey($name) -and $known[$name].Contains($_)) })`); `git checkout` the file
  afterwards and before any merge; never commit it.
- Own program: `tools/graphics-rig/gpu-job --desktop --build "cargo build -j 3 -p
  wcontainers-direct3d-12 --examples" -- "crates\wcontainers-direct3d-12\examples\check_d3d12.cmd"`
  (`draw`, `features`, `outputs`, `answers`, `cases`, `fence-wait 2000`, real and ours, then the
  comparisons; `--desktop` because real DXGI refuses outputs in session 0). One case:
  `crates\wcontainers-direct3d-12\examples\check_case.cmd <name>`. Cases compile HLSL with the
  system's `D3DCompile`.
- Scenes: `tools/graphics-rig/scenes --build "cargo build -j 3 -p wcontainers-direct3d-12
  --examples" --dll ... (all three) --api d3d12 [scenes]`.
- Real programs: `tools/graphics-rig/programs run` with the three `--dll`s (scratch
  `bin/prog49.sh <log> <programs>`); bgfx through scratch bgfx-compare.ps1 in
  C:\wc\out\phase-2-graphics-direct3d-12 (`bgfx49.sh`).
- com.rs: `python3 examples/directx/com.py > examples/directx/com.rs`, then `cargo fmt` (it reads
  the windows crate under `$CARGO_HOME`).
- The desktop needs the shaders crate's submodules once (checked out since run 4).

## No magic numbers

The literal check passes (run52/lit4.log at the last code commit, marks 0). Baseline lines of this track's crates: run 43
wcontainers-direct3d-12 471 and the infrastructure crate 63; run 48 458 and 63; run 49 457 and
63 (main.rs's feature levels now the windows crate's `D3D_FEATURE_LEVEL_*`); run 50 456 and 63; run 51 456 and 63; run 52 455 and 63 (the placement alignment's line gone)
(the planes' block sizes from the interface's formats). Named in run 48:
the cull modes, comparison functions and primitive topologies (`state`), the root signature
versions and the input layout flag (`root`), the buffer layout (`resource`); the tests' DXGI
formats, entry point and write mask from their one definitions. Left: Direct3D's own enum values
no crate this branch uses names (`features.rs`'s wave lane counts, `resource_heap_tier`, the
feature level numbers in tests/front.rs), the depth-stencil view's read-only flags (both fronts define
them; they wait for the Direct3D 11 track to put them in the `d3d` module), and the fix list's
order: Direct3D 12 placement alignments, WebGPU alignments and limits, byte widths, then the
Direct3D enum values. Facts another crate owns stay until its constant is on a branch this one
merges.

## Interface changes

- Run 49: `SlowPath::FrameStretched`, a frame shown stretched to a window of another size against
  the swap chain's scaling (a listed loss, counted at each such present), with a sentence in
  plan 4.9's surface formats paragraph (commit "interface: named a frame shown stretched ...").

- Run 48: `FormatCaps` says every format with a use is served in every texture dimension it can
  have (1D where its blocks are one texel high, 3D but for depth and stencil), held decoded or
  converted where the back end's API lacks it; plan 4.11 says the same (commit "interface:
  ..."). Doc only; the asks of G2 and G3 above follow from it.
- Earlier, each its own "interface:" commit: `Caps::adapter: AdapterInfo` (run 1);
  `ShaderModule::from_wgsl` (run 5); `COUNTER_BINDING` and `SlowPath::PixelInvocationsCounted`
  (run 22); `OcclusionCounter::end_into` and the counter's GPU sum (run 23);
  `SlowPath::SequentialBuffersCopied` (run 26); `SlowPath::IndirectFetchPastEnd` (run 30);
  `SlowPath::StorageOffsetInShader` (run 34). Details in the record at cf55bbe8.

## Measurements

- Run 52, fence-wait (own program, 2000 rounds) as `gpu-job --alone` at ce605d0d
  (run52/alone1.log, 06:15 to 06:17 on the desktop's clock; desktop CPU load 1 % before and 2 %
  after, Win32_Processor LoadPercentage): real first 6 µs, mean 1 µs, 99th percentile 1 µs,
  largest 6 µs; ours first 42 µs, mean 17 µs, 99th percentile 20 µs, largest 42 µs.
- Run 51, fence-wait (own program, 2000 rounds) as `gpu-job --alone` at d9497d6e
  (run51/alone1.log; desktop CPU load 4 % before and 5 % after, Win32_Processor LoadPercentage):
  real first 8 µs, mean 1 µs, 99th percentile 1 µs, largest 58 µs; ours first 54 µs, mean 26 µs,
  99th percentile 66 µs, largest 154 µs.
- Run 50, fence-wait (own program, 2000 rounds) as `gpu-job --alone` at a6412ea6
  (run50/alone1.log, 00:13; desktop CPU load 1 % before and 2 % after, Win32_Processor
  LoadPercentage): real first 8 µs, mean 1 µs, 99th percentile 1 µs, largest 8 µs; ours first
  72 µs, mean 20 µs, 99th percentile 24 µs, largest 72 µs.
- Run 49, fence-wait (own program, 2000 rounds) as `gpu-job --alone` at d3ec3f99
  (run49/alone1.log, ended 23:06; desktop CPU load 1 % before and 1 % after, Win32_Processor
  LoadPercentage): real first 8 µs, mean 1 µs, 99th percentile 1 µs, largest 8 µs; ours first
  157 µs, mean 26 µs, 99th percentile 30 µs, largest 157 µs (run 48: mean 18 µs; the change
  is not explained: nothing on the fence path changed in this run).

- Run 48, fence-wait (own program, 2000 rounds) as `gpu-job --alone` at 3dfef503
  (run48/alone1.log, 21:10; desktop CPU load 1 % before, 32 % after, Win32_Processor
  LoadPercentage): real first 8 µs, mean 1 µs, 99th percentile 1 µs, largest 8 µs; ours first 94
  µs, mean 18 µs, 99th percentile 26 µs, largest 105 µs.
- Run 46, fence-wait (own program, 2000 rounds) as `gpu-job --alone` (run46/alone9.log, desktop
  CPU load 2 % before and after): real mean 1 µs, largest 8 µs; ours mean 17 µs, largest 97 µs.
- Real programs' times (not alone, shared desktop, load not taken), run 47: bgfx 01-cubes,
  26-occlusion, 35-dynamic and 38-bloom ours 2.5, 2.6 to 2.8, 3.0 to 3.1 and 2.5 s against real's
  2.4 to 2.5 s; Godot's 3D platformer 9.9 s for 120 frames.

## Departures from the plan

- Feature level 11_0 on browsers' WebGPU while some 11_0 abilities are refused by name where the
  back end lacks them (depth clamping of alpha-antialiased lines, into culled vertex or domain
  stream output, behind a layer-naming geometry shader without a depth format, or in a pipeline
  without a depth format drawing into a bound depth view; wireframe without polygon modes):
  section 9 gives 11_0, and every such refusal is on the gap list as the front's own open part.
- Wireframe without the back end's polygon mode is refused by name, not converted
  (`FillModeConverted`): Direct3D culls triangles before drawing their edges, so the route needs
  positions after the vertex shader through the stage helper's capture (the Direct3D 11 front
  refuses it the same way).
- Depth bias on lines and points is not applied: real Direct3D 12 draws them unbiased (case
  `line-bias`), where plan 5.7 routed them through `DepthBiasInShader`.
- The own program's cases are compared line for line, exactly; `antialiased-lines` compares its
  picture within 63 (the line filter is the implementation's).
- Clearing a float format's bits that a colour clear does not keep (an `R32_FLOAT` denormal on
  wgpu's Direct3D 12 path) is the back end's part, not the front's.
- A texture the converted program declares as depth is filtered in the program wherever a linear
  non-comparison sampler samples it: WebGPU's depth textures take no filtering sampler.
- A 2D array view of one layer bound where the program declares a 2D texture is bound as a 2D view
  of that layer.
- The literal check was adopted by merging origin/phase-1-literals itself, not through main.
- Every COM call crosses to the device thread as a boxed closure and waits for its answer; plan
  4.8's call buffer is not used (Next 4).
- Indirect draws without indices fetching past a vertex buffer's end: exact for direct draws and
  for arguments in an upload heap (`VertexFetchZeroed`), the listed loss otherwise.
- The Windows library is built as cdylib examples of `wcontainers-direct3d-12`, not in
  `wcontainers-directx` (Next 4); it is `wcontainers_d3d12.dll` with `d3d12.dll` and `dxgi.dll`
  forwarding to it.
- The library draws on wgpu's Vulkan path: with our d3d12.dll loaded, wgpu's Direct3D 12 path
  would load it as the system's (Next 3).
- Presenting: `CheckColorSpaceSupport` answers as Windows does on this SDR desktop for the spaces
  we present and absent for the 7 others real presents (outputs-differing.txt); the output's
  primaries are BT.709's (no browser reports the monitor's); a 16-bit float back buffer is presented encoded into `rgb10a2unorm` (or
  `rgba8unorm`); sync intervals 2 to 4 show a frame for at least one refresh; `DXGI_SCALING_NONE`
  frames are shown stretched (`FrameStretched`, counted at each such frame); `SetHDRMetaData` is checked and changes nothing.
- DXGI's memory sizes are zero and `QueryVideoMemoryInfo` and its relatives answer E_NOTIMPL (no
  browser reports memory). No software adapter is offered.
- `420_OPAQUE` is stored as `NV12` (plan 4.12) but is one plane to the program, its footprint
  the luma plane's, as real Direct3D 12 answers (case `video-formats`); its chroma texture is
  never reached. `NV12`'s target and storage views write the plane they name only.
- `GetResourceAllocationInfo` answers this front's layout (copyable footprints, 64 KiB pages, 4
  MiB for multisampled textures). Planar footprints (depth-stencil, `NV12`, `420_OPAQUE`) align
  rows to 512 bytes as real Direct3D 12 does here.
- Feature level 11_1 is never reported (logic operations and target-independent rasterisation).
  `ARCHITECTURE` answers a discrete adapter. Wave lane counts are WebGPU's bounds (4 to 128).
  `CheckFeatureSupport` answers E_INVALIDARG past feature 34 and for values naming no shader
  model, as this desktop's runtime does; check_d3d12 asks no shader model 6.7 or root signature
  1.2.
- The infrastructure crate's tests keep their own recorder: G1's finishes every fence at a submit.
- `SetEventOnCompletion` with no event waits on an event of the calling thread's own.
- A draw with no pipeline state set draws nothing; a draw reading a root argument not set since
  the root signature was set is skipped with a warning (real keeps the device).
- After the device is removed every fence reports `u64::MAX`.
- Descriptors are 64 bytes (real NVIDIA's are 32); a view counter is kept in two of their words.
- `MULTISAMPLE_RESOLVE` only for float and normalised formats rendering multisampled,
  `MULTISAMPLE_LOAD` only for target formats, whatever the back end's flags.
- Plan 4.12 and section 9 say every format ability Microsoft's 11_0 table requires is served
  but for multisample counts and video abilities. Not so in this front (gap list, formats):
  `NV12`, `YUY2` and `420_OPAQUE` are served by plan 4.12's route, but the table also requires
  abilities of `AYUV`, `Y410`, `Y416`, `P010`, `P016`, `Y210`, `Y216`, `NV11`, `AI44`, `IA44`,
  `P8` and `A8P8`, for which the plan has no route, and `B5G6R5`'s targets, blending and 4x and
  8x multisampled targets, which have no exact route on WebGPU (no 16-bit packed formats; an
  `rgba8unorm` target blends at 8 bits); `A8_UNORM`'s typed unordered access views (the back end
  gives the format no storage use, and the front's buffer route is not built for it);
  `B8G8R8X8_UNORM`'s typed buffer views and vertex input (no interface vertex format for it);
  `R32G32B32`'s multisample load (`FLOAT`, `UINT`, `SINT`) and resolve (`FLOAT`), section 9's
  multisample count shortfall (WebGPU renders no three-channel format); and
  `R10G10B10_XR_BIAS_A2_UNORM` with `R10G10B10A2_UNORM`'s back buffer casts to it (no canvas shows
  the encoding). All are answered absent, never faked, and the gap list names each with its cause;
  features-differing.txt lists every lower answer under its cause.
- `ExecuteIndirect` leaves the state the last read-back command set (Direct3D leaves it
  undefined). `ResolveQueryData` resolves into a buffer of the front's and copies. A predicate
  covers the work up to the next `SetPredication` in the same list. An occlusion query across
  passes is refused, not summed.
- Null descriptors bind a front-owned resource of zeros of the shape the program reads.
- Custom heaps are checked as a discrete adapter's. `ALLOW_SHADER_ATOMICS` is taken on committed
  resources allowing unordered access and refused elsewhere, as this desktop's runtime does.
  `MakeResident`, `Evict` and the like succeed and do nothing.
- A texture made with more mip levels than its chain has is stored with the full chain only.
- Bundles record into the direct list and replay in its state. Back buffers share one reference
  on their swap chain.
- Creation checks follow this desktop's runtime (write states combined, buffer clear values,
  textures in upload heaps, block-compressed textures of part of a block, and more;
  `resource::check_creation`).
- Typeless `R32` and `R16` textures allowing depth views are stored as depth formats; views
  reinterpreting a typeless texture's bits get a twin texture.
- Clears cover every slice of the view; 3D UAV clears with rectangles copy from a scratch 2D
  texture.
- The frame latency waitable object is a semaphore of the swap chain's.
- `GetClockCalibration` answers E_FAIL without timestamps and pairs a lone timestamp with
  `QueryPerformanceCounter`.
- Root signatures are checked at `CreateRootSignature` only and are shared objects (`Arc`).
- Storage formats both read and written follow WGSL's rule (`R32_FLOAT`, `R32_UINT`,
  `R32_SINT`). Storage textures are first converted as 32-bit single-channel formats and again
  for other bound formats at the draw (`PipelineMiss`); where the converter refuses atomics on
  them, every typed texture view of that shader takes the buffer route, which ends the pass
  around the draw.
- Filtering in the program takes the sampler from its descriptor at the draw (a program per
  pipeline and bound state, `PipelineMiss`). Mirror-once addressing the back end lacks goes
  through `border_in_shader`.
- `CheckFeatureSupport` answers `BUFFER` and `SHADER_LOAD` for typed buffer formats the storage
  route decodes; the texel buffer route is `Storage` wherever the back end lacks `TEXEL_BUFFERS`.
- Stream output filled sizes are written back as their low 4 bytes. Every draw through a geometry
  shader ends the open pass. A draw whose topology does not give the geometry shader's input
  primitive draws nothing.
- bgfx's frames are compared with the overlay's GPU time and memory lines ignored, against a
  reference recorded from several real runs (6 by default, bin/bgfx-compare.ps1): bgfx
  26-occlusion's frame 10 is drawn three ways by real Direct3D 12 itself (run 51).

## Licences

- `slab` 0.4 (MIT): the device's object tables.
- `half` 2.7.1 (MIT OR Apache-2.0), half floats in UAV clears.
- `wcontainers-shaders` (ours), through it dxil-spirv and dxbc-spirv (MIT).
- `naga` (MIT OR Apache-2.0): the sizes a program's uniform blocks declare.
- `md-5` 0.11 with `digest` 0.11, `block-buffer` 0.12, `crypto-common` 0.2, `hybrid-array` 0.4
  (each MIT OR Apache-2.0): the DXBC checksum.
- `llvm-bitcode` 0.4 (MIT AND Apache-2.0 WITH LLVM-exception) with `num_enum` 0.7.6
  (BSD-3-Clause OR MIT OR Apache-2.0): DXIL's resource list.
- `bytemuck` 1.25.2 with `derive` (MIT OR Apache-2.0 OR Zlib): query data structures' bytes.
- Windows library only (dev-dependencies): `windows` and `windows-core` 0.62.2 (MIT OR
  Apache-2.0); `blake3` 1 (CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception): the cache
  file's name.
- Tests only: `wcontainers-gpu-webgpu`, `wgpu` 30.0.2, `pollster`, `png` (MIT OR Apache-2.0).
- Wine's and vkd3d's tests (LGPL-2.1) are run from the rig for testing only; nothing of them is in
  this repository. No dependency added in runs 49 to 51.

## Installed on the desktop

Nothing of this track beyond the rig's setup:
- The shaders crate's submodules in this branch's source directory
  (C:\wc\src\phase-2-graphics-direct3d-12\crates\wcontainers-shaders, 68,417,586 bytes with git's
  files).
- C:\wc\out\phase-2-graphics-direct3d-12: bgfx-run.ps1, bgfx-capture.ps1, bgfx-compare.ps1 (1 to 2
  KB each), bgfx's captured frames and logs (about 12 MB, evidence), the rig's outputs, which each
  run overwrites.
- The rig's build folder C:\wc\target\phase-2-graphics-direct3d-12 is deleted at the end of each
  run (run 49: 8,318,929,943 bytes deleted; run 50: 8,314,346,230 bytes; run 51: 8,390,595,197
  bytes; run 52: 8,393,597,398 bytes). Run 51's repeated bgfx runs keep their logs and the evidence frames in
  C:\wc\out\phase-2-graphics-direct3d-12\loop (5,119,280 bytes; the copied programs and libraries
  deleted). On the rented x86 machine run 51's one copy (/work/src and /work/target
  graphics-direct3d-12, 359,310,563 bytes) is deleted; run 52 used no rented machine (every heavy
  run went to the desktop through the rig). Nothing installed in runs 49 to 52; no build output
  in scratch or on the Mac.
