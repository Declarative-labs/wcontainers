# Direct3D 12 and DXGI infrastructure status (G5)

Branch: phase-2-graphics-direct3d-12. Plan: docs/phase-2-graphics.md, sections 4.9, 5.2, 9, 10 and
G5 in 12. This track owns crates/wcontainers-directx-graphics-infrastructure and
crates/wcontainers-direct3d-12. This file holds the current state only; the full record of runs 1
to 47 (what the front does subject by subject, every run's numbers, older scripts) is this file at
commit cf55bbe8 (`git show cf55bbe8:docs/status-graphics-direct3d-12.md`).

## Where things stand

G5 is declared complete for the independent check (run 48, 2026-11-19). The check of run 47 found
seven items; run 48 did each:
- Format shapes (blocking): `format_support` gives every texture shape Direct3D's 11_0 tables
  require: 3D for the BC formats, 1D and 3D for `R8G8_B8G8` and `G8R8_G8B8`. The rule is now
  derived from the format's block (1D only where blocks are one texel high, no 3D for depth), and
  the interface says every back end serves those shapes (interface change below). The 23 lines of
  examples/check/features-differing.txt that allowed the old answers are deleted.
- Depth clamping and other refusals missing from the gap list (blocking): the gap list now names
  every refusal in the front, generated from the code and sorted by cause ("Gap list"), the three
  depth clamping refusals and the three UAV refusals the check named among them.
- The pixel half of `DepthClampInShader` is tested: `depth_clamped_in_the_programs_into_a_depth_target_draws_as_the_back_ends_clamp`
  (tests/gpu.rs: D32 target, w from 1 to 4, z/w 1.5 and -0.4, two viewport depth ranges, against
  the back end's own clamp), the check's own test made the crate's.
- COM out pointers: every interface out pointer (one beside an interface id) is set to null
  before the call, in the generated COM layer (com.py) and in `D3D12CreateDevice`, so a failed
  `Create*` leaves NULL as COM asks; `Map`'s pointer is left as the caller passed it, as real
  Direct3D 12 does (vkd3d's test_map_resource checks it). vkd3d's test_create_graphics_pipeline_state no longer aborts in our library; it fails
  on logic operations alone (run48/logs-full2: only "logic operations without LOGIC_OP", then the test's own null release, 0xc0000005).
- Magic numbers in the tests: the cull modes, comparisons, primitive topologies, root signature
  versions and flag and the buffer layout are named once in the front (`state`, `root`,
  `resource`) and used there and in the tests; the tests import the DXGI formats and the entry
  point name.
- This file: rewritten to the current state.

The pass bar, item by item:
- Wine's and vkd3d's Direct3D 12 tests giving real Direct3D 12's verdicts on the RTX 3080:
  189 of 198 ("Test files") ("Test files"); every failing file is on the gap list with the absent feature, or
  not to be matched as real Direct3D 12 crashes in it.
- A real Direct3D 12 program drawing matching frames: Microsoft's D3D12HelloTriangle and
  D3D12HelloConstBuffers, bgfx 01-cubes, 26-occlusion, 35-dynamic, 37-gpudrivenrendering and
  38-bloom, and Godot's 2D platformer ("Real programs").
- CheckFeatureSupport answering as section 9 says: the own program's `features` against real
  Direct3D 12, every differing answer listed with its reason in features-differing.txt, and the
  crates' tests of section 9's answers ("CheckFeatureSupport").
- The queue, fence, query, predication, placed-resource and indirect cases against real Direct3D
  12: the own program's 33 cases ("Cases").

For the independent check, one command each (scratch `bin/all48.sh <n>` runs them all):
- a sample of test files: `wine-tests run --dll ... vkd3d-d3d12:test_a/test_b` ("How to run");
- the 33 cases: `check_d3d12 cases` through the own program ("How to run");
- the real programs: scratch `bin/real48.sh <n>` (Microsoft's two samples, Godot's two
  platformers, bgfx 37) and `bin/bgfx48.sh <n>` (bgfx 01-cubes, 26-occlusion, 35-dynamic,
  38-bloom).
Before running test files apply scratch run20/rig-wine-run-fix.patch to
tools/graphics-rig/desktop/wine-run.ps1 and undo it afterwards ("How to run"); it is never
committed.

## Results of run 48

2026-11-19, RTX 3080, desktop shared with other tracks unless said; scratch run48/. Run 1 at
3dfef503 (`bin/all48.sh 1` then `bin/bgfx48.sh 1`), run 2 at 708bd126, which only stops clearing
`Map`'s out pointer (`full48.sh 2`, `real48.sh 2`, `lit48.sh 2`):
- Literal check passes (lit3.log, lit2.log). Lint clean (lint1.log, lint2.log). The crates'
  tests: 59 pass, none fail (test2.log: 57 of the two crates, among them the 3 GPU tests, and
  the interface's 2 slow path tests).
- Test files 189 of 198 at 708bd126 (full2.log); 188 at 3dfef503 (full1.log: test_map_resource
  failed on the cleared `Map` pointer, the cause of 708bd126).
- Own program (check2.log; check1.log the same): draw 0 of 4096 pixels beyond 1; features 169
  of 872 differ, each as listed (192 before run 48: the 23 shape lines are gone); outputs 22 of
  110 and answers 2 of 40 differ, none beyond real; 33 of 33 cases equal; `antialiased-lines` 0
  of 4096 beyond 63, largest difference 38.
- Shader runner 214 of 302 (runner1.log, runner2.log). Scenes 95 pass, 22 fail, 3 absent
  (scenes1.log). Wine's five dxgi tests: 3 pass, 2 fail on the absent software adapter
  (dxgi1.log).
- Real programs: see "Real programs". Fence round trip `--alone` (alone1.log): see
  "Measurements".

## Test files (Wine's and vkd3d's, through our libraries, RTX 3080)

How a file counts: the rig compares each test's verdict with real Direct3D 12's on this GPU; a
test that skips what real runs, or runs fewer checks, fails. Tests failing some checks on real
are held to every other check. The counted set is 198 files: d3d12 (Wine's) 10,
vkd3d-d3d12_invalid_usage 3, vkd3d-hlsl_d3d12 9, vkd3d-d3d12 176.

Failing at run 48 (run48/full2.log, logs-full2/, at 708bd126 with the rig patch; run 1 at
3dfef503 failed test_map_resource too, which the Map fix of 708bd126 made pass again), each on the
gap list: d3d12 10 of 10, vkd3d-d3d12_invalid_usage 3 of 3, vkd3d-hlsl_d3d12 9 of 9, vkd3d-d3d12
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
The refusals the counted test files printed (logs-full2): "logic operations without LOGIC_OP"
(create_graphics_pipeline_state) and "a shader feature the converter lacks"
(`RenderTargetArrayIndex`, ps_layer); none other.

Whole counts by run: 129 of 198 (run 17), 157 (20), 164 (25), 169 (28), 180 (31), 184 (34), 186
(35 to 45), 189 (46), 189 (48).

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
  (refused, E_NOTIMPL) and then releases the pipeline it did not get, a null pointer, and crashes in its own code (0xc0000005; no panic in our library since run 48).
- Absent, float64 in shaders (`DoublePrecisionFloatShaderOps` false): shader_instructions.
- Absent, resource binding tier 2 (tier 1 answered where the back end lacks big tables):
  unbounded_resource_arrays, unbounded_samplers.
- Absent, the layer from the vertex shader
  (`VPAndRTArrayIndexFromAnyShaderFeedingRasterizerSupportedWithoutGSEmulation` false, plan
  4.12): ps_layer, whose context makes such a pipeline before asking (`RenderTargetArrayIndex`).
- Not to be matched: copy_block_compressed_texture (real Direct3D 12 crashes in it: its texture
  of part of a block is refused on both, and the test then uses the null texture).

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

Every refusal by name in the front (106 refusal sites, from
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
dimension than the program declares" and "an unordered access view of another dimension than
the program declares" (one-layer 2D arrays bound to 2D declarations are bound as 2D views);
"a shared buffer read after an overlapping one was written in the same submission".

The front's own open parts, each a route not built (no counted test file, case or real program
meets one on the RTX 3080; "Next" 2). Those only met where the back end lacks a feature name it;
browsers' WebGPU lacks `depth-clip-control`, `primitive-index`, polygon modes and texel buffers
on some adapters, and WebGL 2 makes no Direct3D 12 device:
- Depth clamping where the back end lacks `depth-clip-control` (`Features::DEPTH_CLAMP`;
  `DepthClampInShader` serves the rest): into a depth target with no pixel shader ("depth
  clamping the back end lacks into a depth target, without a pixel shader to apply it": the
  front needs a pixel program made for the variant alone, asked of the shaders track); a pipeline
  made with no depth format drawing while a depth view is bound ("depth clamping the back end
  lacks, in a pipeline made with no depth-stencil format drawing while a depth-stencil view is
  bound": its pixel variant would be made at the draw); behind a geometry, tessellation, stream output, culling or
  antialiased line stage ("depth clamping the back end lacks, behind a geometry, tessellation,
  stream output, culling or antialiased line stage": the stage helper's pass-through takes no
  `DEPTH_CLAMP`, asked of the shaders track). The shadow-map case (no pixel shader) is the common
  one.
- Clip distances where the back end lacks them (`Features::CLIP_DISTANCE`): "clip distances the
  back end lacks, without a pixel shader to apply them", "clip distances from a geometry,
  domain, culling or antialiased line stage without the back end's (ClipDistanceInShader)".
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
  upload heap", "a texture read as another type than its view's format" (needs a variant reading
  the slot in the view's type, with the shaders track), "a view of several layers of a
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
`StreamOutAsCompute` for `SO_BUFFER`), every texture shape the format's blocks allow. Tests:
`webgpu_reports_11_0_and_tier_1`, `resource_tables_give_tier_3_but_not_11_1`,
`no_device_without_compute_11_0_sizes_or_d32_s8_targets`,
`browsers_without_polygon_mode_or_depth_clamping_get_11_0`,
`format_support_follows_the_back_ends_format_uses`. The own program's `features` compares every
answer with real Direct3D 12's: an answer above real fails unless served
(`SERVED_BEYOND_REAL`: `R32G32B32_FLOAT` sampled and gathered), an answer below real fails unless
listed with both values in examples/check/features-differing.txt under section 9's reason, and a
listed line that no longer holds fails. `outputs` and `answers` are compared the same way.

## Cases (own program against real Direct3D 12)

`check_d3d12 cases` runs each case in its own process on real Direct3D 12 and through ours and
compares the lines exactly (examples/check/cases.rs); `antialiased-lines` also compares its
picture within 63 of 255 (Direct3D leaves the line filter to the implementation). The 33 cases:
- Placed resources: alias-copy, alias-storage, alias-cpu (overlapping placed buffers written by a
  copy, a root UAV and the CPU, read through the other), alias-texture, custom-heap.
- Queues and fences: queues (copy and compute queues waiting on each other and on a late CPU
  signal), timestamps (both queues, calibrations, resolved to default and readback heaps).
- Queries and predication: predication, predicated-resolve, occlusion-count, statistics.
- Indirect: indirect-draw, indirect-count, indirect-dispatch, indirect-short,
  indirect-vertex-buffer, indirect-vertex-id.
- Others: depth-planes, vertex-past-end, vertex-pulled, restart, render-pass, ms-array, geometry,
  layers, antialiased-lines, filter-depth, vertex-views, line-bias, markers, undefined-topology.

## Shader runner (vkd3d-shader_runner, 302 files)

Run with our d3d12.dll alone (our dxgi.dll serves Direct3D 12 only and every file starts with a
Direct3D 11 part). Run 48 (run48/runner1.log and runner2.log): 214 of 302, the same 88 failing files as runs 34 to 46. Each failing file's cause was sorted in run 44 (scratch
`bin/classify30.py`; the list is in this file at cf55bbe8, "Shader runner"): 61 run fewer checks
than real with none failing (50 shader models above 6.0, which section 9 does not answer; the
rest 64-bit integers, stencil export, depth bounds, raster order views, the layer and viewport
index from any stage, all absent); 27 fail some checks (2 absent features, 2 not to be matched,
22 named items of the shaders track, 1 refused by the front by name waiting on the shaders
track: a texture read as another type than its view).

## Scenes (the rig's Direct3D 12 scenes, beyond the bar)

Run 48 (run48/scenes1.log): 95 outputs pass, 22 fail, 3 absent, the same outputs as runs 38 to 46. The failing outputs: raster, geometry, streamout, tessellation (DXIL geometry and
hull shaders and one pixel shader the converter refuses, the shaders track's); topology (43
pixels beyond 1, all in line cells: the aliased line listed loss), samplers-filters (anisotropy
cells) and msaa-alpha_to_coverage, which draw with largest difference 0 on wgpu's Direct3D 12
path (run 36): the NVIDIA driver's Vulkan path this library draws on ("Next" 3). Absent: `B5G6R5`
targets, minimum and maximum filtering.

## Real programs

RTX 3080, desktop shared with other tracks, not `--alone`, debug builds, 120 frames. Frames 10, 60
and 120 compared with real Direct3D 12's at 8 of 255.
- Run 48 (run48/p1-*.log at 3dfef503, p2-*.log at 708bd126; b1-*.log): Microsoft's
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
- Shaders (G6), new in run 48: a pixel program made for a variant alone, for pipelines with no
  pixel shader whose variant needs one (`DEPTH_CLAMP`, `CLIP_DISTANCE`), and `DEPTH_CLAMP` on the
  stage helper's pass-throughs (gap list, depth clamping).
- Shaders (G6), earlier and still open (evidence in the record at cf55bbe8): the converter
  refusals the runner prints (`TypedBufferFormatKind`, `ClipDistance` read by a pixel shader,
  DXIL `InterpolateAt`, `DxilGeometryShader`, `DxilHullShader`, `CalculateLevelOfDetail`,
  `DepthGatherComponent`, `SwizzledDepth` moving red, `PrimitiveId`, `SampleClamp`,
  `SparseFeedback`, `globallycoherent` memory operands, unbounded arrays on the DXIL route); values
  differing from Direct3D's (a centroid `SV_Position` per sample, `min16float` constants, float
  subnormals in shader models 4 and 5, DXIL `frexp` of a NaN, the shader model 4.0 multisampled
  load in a loop); a slot read in its view's type; `VariantFlags::CLIP_DISTANCE` on
  `Stages::culled`'s pass-through.
- WebGPU back end (G2), on wgpu's Direct3D 12 path: clears through a float that keeps the bits
  (an `R32_FLOAT` denormal), and buffer-to-texture copies of several slices whose
  `rows_per_image` exceeds the copy's height (wgpu-hal 30's placed footprint).

## Next, in this order

1. At the start of each run merge the shared line and every graphics branch with something new,
   and origin/phase-1-literals; keep the literal check passing (after a merge bringing new rules,
   run `init` once). Apply the rig patch uncommitted for test file runs (below).
2. Fix only what the independent check finds. The front's own open parts, beyond the bar: the
   depth clamping refusals (with the shaders track's pixel program for a variant alone),
   wireframe without polygon modes (`FillModeConverted` through the stage helper's vertex
   capture), Godot's 3D platformer's one pixel (trace which draw writes it, on real and ours), the
   other refusals of the gap list, recording state combinations of other vertex strides in the
   cache file.
3. Draw on wgpu's Direct3D 12 path as browsers do on Windows (run 36's experiment, scratch
   run36/dx12-path.patch, measured equal in the three scenes above; it broke three things, each
   another track's: the culling stage's clip distances, the two wgpu-hal items above, and the
   rig's forwarder wrapping wgpu's own device).
4. Plan 4.8's call buffer with plan section 10's one library (`crates/wcontainers-directx`): move
   this track's COM objects there, which also lets Wine's other 18 dxgi tests be counted.
5. The literal sweep, a part per run.

## How to run

All on the Windows desktop through the rig; on the Mac only `nix develop -c cargo fmt`. Every
build command starts with `set CARGO_INCREMENTAL=0&& ` (stale incremental objects once failed to
link). Run one rig command of this branch at a time, and do not change the working tree while a
`gpu-job` of it waits (the job refuses another tree). Scratch bin/ (this track's scratch
directory) holds run 48's scripts: `quick48.sh <n>` (lint, the crates' tests, the literal check),
`full48.sh <n>` (lint, tests, every test file, the own program, the shader runner), `scenes48.sh
<n> [scenes]`, `real48.sh <n>`, `bgfx48.sh <n>`, `all48.sh <n>` (everything, with Wine's five
dxgi tests and the `--alone` fence timing); logs land in scratch run48/.
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
  `bin/prog48.sh <log> <programs>`); bgfx through scratch bgfx-compare.ps1 in
  C:\wc\out\phase-2-graphics-direct3d-12 (`bgfx48.sh`).
- com.rs: `python3 examples/directx/com.py > examples/directx/com.rs`, then `cargo fmt` (it reads
  the windows crate under `$CARGO_HOME`).
- The desktop needs the shaders crate's submodules once (checked out since run 4).

## No magic numbers

The literal check passes (run48/lit3.log at the last code commit and lit2.log at the Map fix). Baseline lines of this track's crates: run 43
wcontainers-direct3d-12 471 and the infrastructure crate 63; run 48 458 and 63. Named in run 48:
the cull modes, comparison functions and primitive topologies (`state`), the root signature
versions and the input layout flag (`root`), the buffer layout (`resource`); the tests' DXGI
formats, entry point and write mask from their one definitions. Left: Direct3D's own enum values
no crate this branch uses names (`features.rs`'s wave lane counts, `resource_heap_tier`, the
feature level numbers in main.rs), the depth-stencil view's read-only flags (both fronts define
them; they wait for the Direct3D 11 track to put them in the `d3d` module), and the fix list's
order: Direct3D 12 placement alignments, WebGPU alignments and limits, byte widths, then the
Direct3D enum values. Facts another crate owns stay until its constant is on a branch this one
merges.

## Interface changes

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
  back end lacks them (depth clamping with no pixel shader, behind the stage helper, or with no
  depth format; wireframe without polygon modes): section 9 gives 11_0, and every such refusal is
  on the gap list as the front's own open part.
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
  we present; a 16-bit float back buffer is presented encoded into `rgb10a2unorm` (or
  `rgba8unorm`); sync intervals 2 to 4 show a frame for at least one refresh; `DXGI_SCALING_NONE`
  frames are shown stretched; `SetHDRMetaData` is checked and changes nothing.
- DXGI's memory sizes are zero and `QueryVideoMemoryInfo` and its relatives answer E_NOTIMPL (no
  browser reports memory). No software adapter is offered.
- `GetResourceAllocationInfo` answers this front's layout (copyable footprints, 64 KiB pages, 4
  MiB for multisampled textures). Planar depth-stencil footprints align rows to 512 bytes as
  real Direct3D 12 does here.
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
- bgfx's frames are compared with the overlay's GPU time and memory lines ignored.

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
  this repository. No dependency added in run 48.

## Installed on the desktop

Nothing of this track beyond the rig's setup:
- The shaders crate's submodules in this branch's source directory
  (C:\wc\src\phase-2-graphics-direct3d-12\crates\wcontainers-shaders, 68,417,586 bytes with git's
  files).
- C:\wc\out\phase-2-graphics-direct3d-12: bgfx-run.ps1, bgfx-capture.ps1, bgfx-compare.ps1 (1 to 2
  KB each), bgfx's captured frames and logs (about 12 MB, evidence), the rig's outputs, which each
  run overwrites.
- The rig's build folder C:\wc\target\phase-2-graphics-direct3d-12 is deleted at the end of each
  run (run 48: 8,492,639,967 bytes deleted). Nothing installed in run 48; no build output in scratch
  or on the Mac.
