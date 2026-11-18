# Direct3D 11 front status

Branch: phase-2-graphics-direct3d-11. Plan: docs/phase-2-graphics.md, milestone G4 (section 12).
Runs so far: six on G4a, the first slice; then runs 1 to 16 of G4 as a whole (2026-11-17 and
18; runs 13 to 16 are the orchestrator's second series on G4).

## Where things stand

G4 run 16 (2026-11-18, about 01:00 to 03:00 Pacific). Merged origin/phase-2-graphics-direct3d-12,
-shaders and -rig cleanly (the others had nothing new; the shaders crate's change numbering
variant slots by a binding's place in its kind's range is Direct3D 11's register numbering, as
this front already gave); the merged tree linted with warnings denied on the desktop.
- Direct3D 11.3's interfaces (the census: every bgfx example calls `CreateRasterizerState2` and
  `CreateTexture2D1`, every program `CheckMultisampleQualityLevels1`): the device is
  `ID3D11Device3`, the contexts `ID3D11DeviceContext3`, textures `ID3D11Texture2D1`/`3D1`, the
  views their `1` forms, rasterizer states `ID3D11RasterizerState2` and queries `ID3D11Query1`
  (generate.py; `convert::same_fields` copies a description into its 11.3 form and back, the
  added fields being last). test_create_rasterizer_state now passes.
- Read-only depth-stencil views (`D3D11_DSV_READ_ONLY_DEPTH`/`_STENCIL`, 11_0): a shader view
  of an aspect the bound depth view only reads stays bound (Direct3D's binding rule, Wine's
  test_sample_attached_rtv), the pass marks that aspect read-only, and draws use a twin of the
  depth-stencil state writing nothing there (`DepthStencil::read_only`, made at the first draw).
  New front test `a_read_only_depth_view_keeps_views_of_its_aspect_bound_and_its_draws_write_nothing_there`.
- Creation checks real Direct3D makes (test_create_texture2d had removed the device at test 41
  and failed 29 checks; now 1): depth-stencil binding excludes target and storage bindings; the
  depth groups with stencil, shared-exponent, two-pixel and block formats are never targets or
  storage; typed depth formats (`D32_FLOAT`, `D24_UNORM_S8_UINT`, ...) are bound for depth only,
  and `CheckFormatSupport` no longer claims shader reads of them; shared and keyed-mutex sharing
  exclude each other; an sRGB texture bound for storage is made (no storage view of it can be)
  and an sRGB storage view is invalid. test_create_texture3d and
  test_create_unordered_access_view now pass.
- Formats: `R32G32B32_*` (5 to 8) map to the interface's three-channel formats (served where the
  back end holds them: WebGL 2; the WebGPU back end lacks them, Asked) and `R8G8_B8G8_UNORM`,
  `G8R8_G8B8_UNORM` to `Rgbg8Unorm`/`Grgb8Unorm`, which the WebGPU back end holds decoded. New
  program `own_packed` (tests/programs.rs
  `two_pixel_formats_sample_load_and_copy_as_direct3d_does`): both formats sampled through a
  point sampler, loaded, and copied to staging and mapped, identical to real Direct3D (a listed
  case of the pass bar: `R8G8_B8G8_UNORM` sampled).
- Storage views of some slices of a 3D level, on a texture of those slices (Departures).
- `ClearUnorderedAccessViewUint` of a view of floats writes the texels' bytes (Departures):
  test_clear_image_unordered_access_view now gives real Direct3D's verdicts (it fails the same 3
  checks real Direct3D fails).
- bgfx: NVIDIA's NVAPI took our device for its driver's (Departures); without it bgfx-01-cubes
  now stops at a pixel shader declaring `Texture2DMSArray` (`MultisampledArrayTexture`, shaders
  track; Asked).
- Module size (release, on the desktop): see Measurements.
- Resolves name a swap chain buffer's sRGB twin as DXGI's targets do (the Tool Kit's
  DeviceResources resolves its MSAA target into the `B8G8R8A8_UNORM` back buffer as
  `B8G8R8A8_UNORM_SRGB`): atg-simplemsaa_pc went from whole frames wrong (the resolve dropped)
  to frames 60 and 120 matching and frame 10 failing by 4 pixels in 2 tiles past the rig's
  allowance (largest difference 11, on edges; logs/g4s4q-programs.log).
- 32-bit vertex elements are given to the vertex program as the type its converted inputs have
  (Direct3D hands the bits over; WebGPU checks the types): `format::vertex_format_read_as`, in
  `layout_desc` with the bound vertex shader's reflection, so layout blocks are kept per input
  layout, strides and vertex shader (`LayoutKey`). Godot's 3D platformer (through ANGLE) had
  removed the device at its first pipeline ("Location[3] Sint32x4 ... provided Uint32x4").
  It still stops there after this change (logs/g4s4t-programs.log: the same message): which call
  builds that pipeline, and what the reflection says of location 3, is not read yet (What comes
  next). The change itself left every matching program matching (godot-platformer-2d-d3d11,
  atg-simplelightingpc, atg-simpleinstancingpc, all frames largest difference 0).
- Results at d5f9d35 (RTX 3080 through the queue's hazard group, not alone): Wine's d3d11 file
  118 of 166 (114 at run 15; none lost; logs/g4s4o-wine.log). Real programs through the whole
  library (logs/g4s4o-programs.log): the nine as before match with largest difference 0
  (sdk-tutorial02, 04 to 07, chocolate-doom, atg-simplelightingpc, atg-simpleinstancingpc,
  godot-platformer-2d-d3d11); atg-simplecomputepc differs as before (154515 pixels);
  atg-simplebezierpc stops at `VertexWithoutPosition` (shaders track; its hull stage is next).

G4 run 15 (2026-11-18, about 00:05 to 01:15 Pacific). Merged origin/phase-2-graphics-webgpu,
-webgl, -direct3d-12, -shaders and -rig cleanly (-graphics had nothing new).
- The cache of plan section 8 in this front (src/cache.rs; the shaders crate's `Cache`,
  `convert_cached`, `record_combination`, `combinations`): every conversion goes through
  `convert_cached`, so a shader converted in an earlier run is decoded, not converted. Each
  combination drawn the first time is recorded in interface terms (`cache::Record`: the program's
  two modules by content key, the blend, depth-stencil and raster blocks (the raster twin of a
  sample mask or a bias in the program included), the vertex layout block, targets, topology and
  binding layout keys). The process's first device builds every record before its first draw
  (`Warm::start`: modules decoded from the cache, programs, blocks, `prepare`); back-end pipelines
  are keyed by their objects, so the program built there is kept by its modules' keys and taken by
  the first `program()` of that pair (blocks are the back end's by description). The library opens
  one file per program in the directory `WC_GPU_CACHE` names (src/com/cache.rs; Departures).
  `report_ahead` now prints the start's counts first. New test tests/cache.rs (its own binary: the
  cache is the process's): a run records two shaders and one combination; the next run's first
  device prepares it before any draw, its first draw uses that program, nothing is appended, and a
  second device builds nothing.
- Building-ahead hit counts on real programs (the pass bar's item), five programs run twice from
  one directory with an empty cache first (scratch run-warm.sh; logs/g4s3x-warm.log and
  programs/g4s3x): every combination recorded in the first run was built at the second run's start
  and drawn (sdk-tutorial07 1 of 1, chocolate-doom 1 of 1, atg-simplelightingpc 2 of 2,
  atg-simpleinstancingpc 3 of 3, godot-platformer-2d-d3d11 3 of 3); pipelines still built at their
  first draw went from 2, 2, 3, 4, 13 to 1, 1, 1, 1, 10. The one left in each program is not
  identified yet; Godot's others are draws through its geometry stage and the stage helper's
  dispatches, which are not recorded yet (expected from the paths, not checked one by one). Frames
  identical to real Direct3D's (largest difference 0) in both runs, 40 of 40 compared.
- 10_x devices answer `ComputeShaders_Plus_RawAndStructuredBuffers_Via_Shader_4_x` as the RTX
  3080 does where the back end runs compute programs, and take shader model 4 compute shaders then
  (before: refused below 11_0). A float clear of a raw or structured buffer view is dropped, as
  Direct3D drops it (it removed the device). test_unaligned_raw_buffer_access now passes.
- Wine's d3d11 file at 8862cef: 114 of 166 (113 at run 14; none lost; logs/g4s3y-wine.log).
  The gap list below is written in full for the first time: every failing test, under the absent
  feature or the named work behind it.
- At 8862cef on the desktop: lint of both builds clean with warnings denied, front tests 40 +
  cache 1 + 3 unit pass (logs/g4s3y-*), library tests com 15 and programs 14 pass
  (logs/g4s3z-lib.log).
- At the end the desktop's C:\wc\target\phase-2-graphics-direct3d-11 (13.1 GB) and the
  checkout's own target (588 MB, from `cargo check`) were deleted.
- The status file was cut from 1,880 lines: the run entries before run 13 and the Wine file's runs
  1 to 12 are in its git history.

G4 run 14 (2026-11-17, about 22:45 to 00:30 Pacific). Merged origin/phase-2-graphics-webgpu,
-direct3d-12, -shaders and -rig cleanly (-graphics and -webgl had nothing new); the merged tree
linted (both builds) and passed the front tests (37) on the desktop before any change. The rig
brought its display reset guard (desktop/hazard.ps1: jobs not marked `--reference`, ours
included, run one at a time in the group "hazard", and a reset holds the queue); this run's
jobs waited there a few minutes each.
- Geometry shaders and stream output (src/context/geometry.rs, src/stream.rs) through the
  shaders crate's stage helper: `CreateGeometryShader` and `...WithStreamOutput` make a
  `Geometry` object (from a geometry shader, or a vertex shader's bytecode with a declaration);
  the declaration is checked against the output signature (`shader::output_signature`, OSGN,
  OSG5, OSG1) by the rules Wine's test_stream_output and test_fl10_stream_output_desc show real
  Direct3D applying (names and indices, gaps, component ranges, each component once, a buffer's
  entries together and not gaps alone, strides, 10_x's one value per buffer, the rasterized
  stream). A draw with one bound runs `Stages::run` outside a pass: the vertex shader converted
  with every input fetched (`InputLayout::all`, the bases in its helper block, bound for compute
  through `ContextState::vertex_compute`), the geometry stage's slots in their own set
  (`GEOMETRY` = 3: constant buffers, views and samplers, with getters), then draws the output
  with the pass-through program (converted again for the pixel shader's interpolation) and the
  pixel shader. `SOSetTargets`/`SOGetTargets`: each buffer keeps the bytes it is filled to in a
  4-byte GPU counter (`Buffer::filled`), set by the offsets (-1 appends), gathered into the
  helper's block around each streaming draw. `DrawAuto` reads that counter back
  (`ReadBackWait`). Stream output statistics and overflow predicates of stream 0 add what each
  draw wrote and needed (read back while one runs). Indirect draws through a geometry shader read
  their arguments back (`IndirectReadBack`). Buffers bound for vertices, indices or stream output
  get storage usage where the back end has compute. Refused by name: geometry shaders without
  the back end's compute programs (WebGL 2), vertex shader variants behind a geometry shader,
  counted occlusion and statistics queries during such draws, helper constants in a geometry
  shader, and what the stage helper refuses (`GeometryStreams`, `GeometryWithoutPosition`,
  indexed strips, triangle strips with adjacency). New front tests
  `a_geometry_shader_runs_as_compute_before_the_draw_of_its_output`,
  `stream_output_writes_its_targets_and_draw_auto_draws_what_they_hold`, and the declaration
  rules in stream.rs's unit test.
- Storage textures read through a buffer of their texels (src/context/through.rs) where the
  converter refuses atomics on storage textures (`StorageImageAtomics`): `shader::convert` marks
  the typed texture views (`TypedView::through`, `texels_through_buffer`), the set binds a buffer
  of the view's level (`Through`, its layout in `HelperValue::TexelLayout`), copied in before
  each draw or dispatch and out after (`AtomicsThroughBuffer`). Compute helper blocks are now
  written by the same `helper_block` as the draws'.
- Views reinterpreting a typeless texture's bits (`format::reinterprets`): made on a twin
  texture of the view's format (`View::twin`): shader resource and storage views copied in
  before each draw or dispatch reading them and storage views out after, target views copied in
  at the pass's start and out at its end, clears of them cleared on the twin and copied out
  (`ReinterpretCopy`). Depth views never reinterpret.
- Depth clamping in the programs where the back end clips only (WebGL 2 without
  `EXT_depth_clamp`): the shaders crate's `DEPTH_CLAMP` variant on both programs with their added
  value one location past both stages' own (`StageVariant::linked`) and `DepthRange` from the
  first viewport (`DepthClampInShader`); the raster block then keeps clipping on.
- Stencil views (`X24_TYPELESS_G8_UINT`, `X32_TYPELESS_G8X24_UINT`) are now refused by name
  without the back end's `VIEW_SWIZZLE`: Direct3D gives stencil in the second component and the
  interface's stencil views in the first, so they read wrong values before (the views scene's
  stencil cell; Wine's test_depth_stencil_sampling; Asked since run 9).
- `own_refused` now asks for a hull shader where it asked for a geometry shader (geometry
  shaders are served and answer as real Direct3D).
- Results at 1bcfe5f (RTX 3080 through the queue, not alone; scratch logs/g4s2i-*): Wine's
  d3d11 file 113 of the 166 the rig counts (104 at run 13; none lost). New:
  test_create_shader_resource_view, test_draw_uav_only, test_fl10_stream_output_desc,
  test_geometry_shader, test_index_buffer_offset, test_stream_output, test_stream_output_resume,
  test_texture, test_texture1d. The rig's scenes through the whole library: 122 pass, 16 fail, 4
  absent (113, 28, 4 at run 13): compute and streamout now match; views now draws (its stencil
  cell then read wrong, now refused); geometry stops at `PrimitiveId` (a pixel shader reading
  SV_PrimitiveID, the shaders crate's unbuilt `PRIMITIVE_ID_FROM_STAGE`); raster (wireframe) and
  tessellation (hull shaders) as before. Through the WebGL 2 build: 34 pass, 96 fail, 19 absent,
  as at run 13. Library tests 15 + 14 and front tests 40 pass.
- After 1bcfe5f: target views reinterpreting bits are drawn into their twins (copied in at the
  pass's start unless cleared, out at its end); building ahead skips combinations whose targets
  are of another kind (float or integer) than the pixel shader writes (it made a pipeline WebGPU
  refused, which removed the device in Wine's test_uav_load). At the last commit (see git log):
  the compute, streamout and depth scenes match; the WebGL 2 build's depth scene now matches
  (clamping in the programs; its 11_0 cells absent); test_uav_load reaches its 3D-slices view
  and test_depth_stencil_sampling the stencil view, both refused by name. Wine's full file again
  113 of 166 (none lost; logs/g4s2m-wine.log). Real programs through the whole library
  (logs/g4s2m-programs.log): the eight as before match with largest difference 0, and
  godot-platformer-2d-d3d11 (Godot through ANGLE, which builds a point-sprite geometry shader)
  now matches too at frames 10, 60 and 120 with largest difference 0: four engine families.
  atg-simplecomputepc runs its 120 frames and still differs (154515 pixels; its Mandelbrot is
  drawn, so a computed value differs: not read yet).

Runs before 14 (G4a's six runs, G4 runs 1 to 13, all 2026-11-17) are in this file's git
history (git log -p docs/status-graphics-direct3d-11.md, before runs 15 and 16): what each
fixed, from the independent checks, and why.

What the front serves: device creation at 11_0 (10_0 without compute), buffers, 1D, 2D and 3D
textures (arrays, cubes and cube arrays, mips, immutable, dynamic and staging), render target,
depth-stencil and shader resource views of them, `GenerateMips`, event queries, occlusion
predicates and counted occlusion queries, timestamps (reported disjoint), predication, swap chains, samplers, blend, depth-stencil and rasterizer states, input layouts, vertex and pixel
shaders converted from DXBC, constant buffers, draws (plain, indexed, instanced; every list and strip
topology; adjacency topologies without a geometry shader through `AdjacencyDropped`; indirect;
`SV_VertexID` and `SV_InstanceID` at any start and base), clears, copies,
`UpdateSubresource`, maps of staging resources and of dynamic buffers (`MappedUploadWhole`),
`ClearState`, `Flush`, the context's `Get` methods, and the device's capability queries; since
run 11 instance step rates on WebGPU (fetched in the program) and Direct3D 10's interfaces on its
resources, views and states; since runs 8 to 14 levels 9_1 to 9_3, multisampled textures,
deferred contexts and command lists, compute shaders (shader model 4 at 10_x since run 15),
unordered access views, structured, raw and typed buffers, atomics and view counters, geometry
shaders and stream output (through the shaders crate's stage helper); since run 15 the cache.

## Wine's Direct3D 11 tests through ours

Files passing (the bar: real Direct3D's verdict for every test, except the gap list):
- d3d11: not passing. G4 run 16 (2026-11-18, our library copied in as d3d11.dll, RTX 3080
  through the queue's hazard group, not alone, through scratch rigfix/wine.sh, refreshed from
  the rig's runner with the same one-line fix): full file at d5f9d35: 118 of the 166 the rig
  counts (48 fail; logs/g4s4o-wine.log; none lost). New: test_clear_image_unordered_access_view,
  test_create_rasterizer_state, test_create_texture3d, test_create_unordered_access_view. Each
  failing test is in the gap list below.
- d3d11: not passing. G4 run 15 (2026-11-18, our library copied in as d3d11.dll, RTX 3080 through
  the queue's hazard group, not alone, through scratch rigfix/wine.sh): full file at 8862cef: 114
  of the 166 the rig counts (52 fail; 193 s; logs/g4s3y-wine.log; none lost). New:
  test_unaligned_raw_buffer_access. Each failing test is in the gap list below.
- Earlier runs of the file (G4 runs 1 to 14: 11, 56, 73, 68, 72, 74, 78, 79, 82, 88, 90, 104
  and 113 passing) and their first gaps are in this file's git history (before run 16).
- d3d10core: not run (needs a d3d10core.dll of ours and `ID3D10Device`). dxgi: the rig has no
  baseline of real Direct3D's verdicts for it.

The gap list, G4 run 16 (the 48 failing tests of run 16's full file, 118 of 166; the first
named refusal, skip or failed check of each failing test's log, logs/g4s4o-wine-gaps.txt, read
against Wine's d3d11.c in scratch winesrc/). A test is in part A only where the first thing it
fails on is a feature absent on this stack; everything else is work, in part B, with whose it
is. Nothing is excluded for convenience: each of the 48 failing tests is in exactly one place.
A. Absent on this stack (gaps):
- Logic operations (WebGPU has none, so 11_1 is not offered; Wine's helper then asks for WARP,
  which this library does not provide): test_create_shader, test_state_refcounting,
  test_resource_access, test_swapchain_formats, test_format_support, test_shared_resource,
  test_device_interfaces (also debug devices: no debug layer), test_logic_op (skips "Logic op is
  not supported").
- Min/max reduction filtering (WebGPU has none): test_filter_minmax (skips).
- Video decoding and processing (`ID3D11VideoDevice`, `ID3D11VideoContext`; plan 4.12 gives them
  no route): test_immediate_context (line 2450), test_video_support, test_h264_decoder.
- Shared keyed mutexes between devices (`MISC_SHARED_KEYEDMUTEX`): test_keyed_mutex,
  test_create_buffer (test 36), test_texture1d_interfaces, test_texture2d_interfaces,
  test_texture3d_interfaces (their keyed-mutex entries).
- The 10_0/10_1 rasteriser's corner rule for a negative viewport origin (WebGPU rasterises as
  11_0): test_negative_viewports at levels 10_x.
B. Work, named (not gaps):
- Shaders track: `VertexWithoutPosition` (test_so_statistics_query, test_device_context_state,
  test_clear_state, test_vertex_id, test_stream_output_components, test_stream_output_vs),
  `StructType` (test_vs_input_relative_addressing, test_tgsm), `DynamicOffset` (test_gather,
  test_gather_c), `AtomicSignedness` (test_atomic_instructions), `CullDistance`
  (test_combined_clip_and_cull_distances), `RenderTargetArrayIndex` (test_layered_rendering),
  `ViewportArrayIndex` (test_multiple_viewports), hull and domain stages in `Stages`
  (test_quad_tessellation), early tests in the program with storage writes (`EarlyTestsInShader`:
  test_early_depth_stencil), constants read past the declared array (test_cb_relative_addressing),
  clip distances in the program (test_clip_distance).
- WebGPU back end: copies between `R16_UNORM` and `D16_UNORM` (test_format_compatibility, removed
  at line 992), the BGRA vertex format (test_vertex_formats, 98 of 100 checks), a view swizzle for
  stencil views (test_depth_stencil_sampling, test_sample_attached_rtv: its read-only depth views
  are served since run 16, its stencil view is the first refusal), `R32G32B32_*` textures
  (test_create_texture2d test 21, its only failing check).
- Infrastructure crate: every buffer of a sequential swap chain (test_swapchain_flip, line 16228);
  a swap chain on the desktop window (test_desktop_window: wgpu's surface refuses it at
  `Surface::configure`, the device is lost at line 32666).
- This front: NV12 and YUY2 (plan 4.12's exact routes: test_nv12 skips "NV12 textures are not
  supported"), target views of buffers (test_create_rendertarget_view), `IDXGISurface1::GetDC`
  (test_getdc; test_dxgi_resources), test_uav_load (test 1, a uint pixel shader drawing into an
  `R32_UINT` target view of an `R32_TYPELESS` texture, removes the device: WebGPU builds its
  pipeline with a float target, "Output format Float32 is incompatible with the shader
  Uint32x4"; not read further).
Wine's d3d10core file is not run (it needs a d3d10core.dll of ours and `ID3D10Device`).

## Known failures

- `depth_with_stencil_copies_as_direct3d_does` (programs.rs, ignored with its reason): the example
  `own_depth_copy` clears an `R24G8_TYPELESS` depth-stencil texture, copies it by `CopyResource` and
  `CopySubresourceRegion`, and loads the copy's depth. Real Direct3D: no removal, depth 0.25 read
  back. Ours: removed (0x887A0020) with "Copy source aspects must refer to all aspects of the source
  texture format": the WebGPU back end (crates/wcontainers-gpu-webgpu, memory.rs `copy_texture_impl`,
  through `convert::aspect`) still copies the depth aspect alone. It passes once that back end
  follows interface change c8a7be9 (a copy between textures of a combined format names no aspect).

## What comes next

After G4 run 16, in order (the branch is pushed; everything on it built and passed on the desktop):
0. Wine's d3d11 file: the gap list above has 30 tests of named work. This front's share first:
   test_uav_load (a uint shader into an `R32_UINT` target twin of an `R32_TYPELESS` texture gets
   a float pipeline target: find which call builds it), NV12 and YUY2 (plan 4.12's exact routes;
   also a listed case), `GetDC` (a GDI-compatible texture read back into a DIB section and
   written back at `ReleaseDC`), target views of buffers (a 2D twin of the elements, copied to
   the buffer at the pass's end). Then the shaders track's and the back ends' items as they land
   (merge, rerun the file).
1. Real programs: bgfx waits on `MultisampledArrayTexture` (shaders track); atg-simplecomputepc's
   frames differ (compare its compute shader's output with real Direct3D's); Godot's 3D
   platformer (a pipeline whose vertex input at location 3 is `Sint32x4` against a `Uint32x4`
   element, still after run 16's typing of 32-bit elements by the reflection: read which call
   builds it). Run bgfx with the empty nvapi64.dll (How to run).
2. Building ahead: record the draws through a geometry stage and the stage helper's dispatches
   too (Godot's 10 remaining first-draw builds), and find the one pipeline per program still
   built at its first draw on a warm start.
3. The listed cases still without a test against real Direct3D (texture atomics,
   earlydepthstencil with storage writes, a depth buffer larger than its target, rasterised
   stream output, typed storage views, NV12 and YUY2, a copy between multisampled textures,
   antialiased lines, UsageScopeSplit, FilterInShader on depth). Texture atomics and rasterised
   stream output can be written as `own_` programs now, as `own_packed` was.
4. Wireframe without the back end's polygon mode (the raster scene on WebGPU): no slow path is
   planned; read section 4 first (culling must still apply to the triangles).
5. The WebGL 2 cases (34 of the rig's outputs match through the WebGL 2 build at run 13): texture
   views (the back end's `TEXTURE_VIEWS`: the views scene), polygon mode, the constants and
   samplers scenes (device lost in the back end, not read yet), typed buffers
   (`TexelBufferAsTexture`). Not rerun at runs 14 to 16.
6. Wine's d3d10core file needs a d3d10core.dll of ours and `ID3D10Device`; not started.
7. Move the context fully onto the pass tracker (fitted attachments, `UsageScopeSplit`); compute
   shaders sampling a clamped texture or through a biased sampler (refused by name); track which
   copy of a twin is current instead of copying at every draw (measure first).

## How to run

- Wine's tests (G4 run 13, until the rig's runner is fixed): scratch `rigfix/wine.sh --dll
  'd3d11.dll=%CARGO_TARGET_DIR%\debug\wcontainers_directx.dll' d3d11[:test_a/test_b]` copies the
  rig's wine-run.ps1, fixed in one line, to C:\wc\out\phase-2-graphics-direct3d-11\rigfix and
  runs it as `wine-tests run` does (logs to the same wine-logs directory). `gaps.sh <label>` lists
  the first named gap of each failing test of logs/<label>-wine.log. `run-all.sh <label>` runs the
  library tests, every scene through the whole library, Wine's d3d11 file and the WebGL 2 scenes.
  Real programs' logs are under graphics-rig/programs/phase-2-graphics-direct3d-11/<program>.

All of it in one command (never run yet): the scratch script `resume.sh <label> [steps]` runs
lint (both builds), the front tests, the library tests, every Direct3D 11 scene through our
library, Wine's d3d11 file and every scene through the WebGL 2 build, in that order (steps `lint
front lib scenes wine webgl`, default all), each step's output in scratch
logs/<label>-<step>.log, printing each exit code and last lines. The steps are the commands below.

- One Wine test alone: `tools/graphics-rig/wine-tests run --dll
  'd3d11.dll=%CARGO_TARGET_DIR%\debug\wcontainers_directx.dll' d3d11:test_a/test_b`; each test's log
  is pyro-parked/wcontainers-scratch/graphics-rig/wine-logs/phase-2-graphics-direct3d-11/d3d11-<test>-hardware.log
  (the copies directly in wine-logs/ are older).
- Scenes through the whole library (our DXGI too, as programs in the container get it): the
  scenes command with `--dll 'd3d11.dll=%CARGO_TARGET_DIR%\debug\d3d11.dll' --dll
  'dxgi.dll=%CARGO_TARGET_DIR%\debug\dxgi.dll' --dll
  'wcontainers_directx.dll=%CARGO_TARGET_DIR%\debug\wcontainers_directx.dll'` (the device scene
  matches only so).
- bgfx's examples through ours (G4 run 16) also take `--dll
  'nvapi64.dll=C:\wc\out\phase-2-graphics-direct3d-11\nonvapi\nvapi64.dll'` (an empty file:
  NVIDIA's NVAPI then does not load; Departures).
- Real programs (G4 run 7), frames compared with real Direct3D's references by the rig:
  `tools/graphics-rig/programs run --dll 'd3d11.dll=%CARGO_TARGET_DIR%\debug\d3d11.dll' --dll
  'dxgi.dll=%CARGO_TARGET_DIR%\debug\dxgi.dll' --dll
  'wcontainers_directx.dll=%CARGO_TARGET_DIR%\debug\wcontainers_directx.dll' <program>...`
  after `tools/graphics-rig/remote "cargo build -q -j 3 -p wcontainers-directx"`. Always give the
  three together for programs using dxgi.dll: the library copied in as d3d11.dll beside our
  dxgi.dll loads the library twice. Each program's output.log and frames land in
  pyro-parked/wcontainers-scratch/graphics-rig/programs/<program>.
- Do not send the tree (remote, gpu-job, scenes) while another of this checkout's jobs waits in
  the queue: the job checks the desktop still holds the tree it sent and stops otherwise
  ("holds tree ..., not the sent ...").
- The WebGL 2 build (G4 run 6): `cargo build -p wcontainers-directx --features webgl
  --target-dir %CARGO_TARGET_DIR%\webgl` on the desktop (its own target directory, so the two
  builds' wcontainers_directx.dll do not replace each other), then the scenes with `--dll
  'd3d11.dll=%CARGO_TARGET_DIR%\webgl\debug\wcontainers_directx.dll'`. It loads ANGLE from
  `WEBGL_ANGLE_DIR` (default C:\wc\tools\angle-mozangle-0.7.1) and takes the WebGL extensions
  `WEBGL_OFF` lists as missing. Check-only on the Mac: `wincheck/check.sh clippy --features
  wcontainers-directx/webgl -- -D warnings`.

Without the desktop (G4 run 4): a check-only cross build on the Mac proves the Windows library
compiles and is clippy-clean, nothing more (nothing links or runs). The script
pyro-parked/wcontainers-scratch/graphics-direct3d-11/wincheck/check.sh runs rustup's cargo 1.99.0
with `--target x86_64-pc-windows-msvc -j 2` on both crates with all targets, with `VCINSTALLDIR`
and `CC_`/`CXX_`/`AR_x86_64_pc_windows_msvc` pointing at a stub (wincheck/bin/stub as cl.exe,
lib.exe and link.exe) that writes the files it is asked for, empty. `check.sh` checks;
`check.sh clippy -- -D warnings` lints. It needs `rustup target add x86_64-pc-windows-msvc
--toolchain 1.99.0` (done on this Mac) and the submodules
(`git submodule update --init --recursive`). A deliberate type error in Windows-only code was
caught by it, so it does see that code.

- The desktop's clock moved back about eight hours during G4 run 2 (gpu-job times went from 17:17
  to 09:25), which left build outputs dated in the future, so cargo skipped rebuilding our crates
  after edits. If a run on the desktop does not reflect a change, run
  `tools/graphics-rig/remote "cargo clean -q -p wcontainers-directx -p wcontainers-direct3d-11"`
  first.

From the checkout root on the Mac (the desktop does all building and testing). The desktop's
checkout needs the shaders crate's submodules, so commands start with `git submodule update --init --recursive -q`.
- Front tests (recording back end, no GPU), 34 tests:
  `tools/graphics-rig/remote "git submodule update --init --recursive -q && cargo test -q -j 3 -p wcontainers-direct3d-11"`.
- Library tests (COM behaviour, the forwarder, threads, our programs; they make real devices and a
  window, so a GPU job on the hidden desktop), 29 tests (com 15, programs 14):
  `tools/graphics-rig/gpu-job --desktop --build "git submodule update --init --recursive -q && cargo test -q -j 3 -p wcontainers-directx --no-run" -- "cargo test -q -j 3 -p wcontainers-directx -- --test-threads 1"`
  (add `--nocapture` to see what the programs print).
  The random lifetimes comparison takes `WC_LIFETIMES=<first seed>,<seeds>,<steps>` (default 1,8,300);
  one seed alone: `set WC_LIFETIMES=17,1,300&& cargo test -q -j 3 -p wcontainers-directx --test
  programs object_lifetimes -- --nocapture`. A failure names the seed and the first line that differs.
  They work from a clean target directory: no `cargo build` is needed first.
- Lint: `tools/graphics-rig/remote "git submodule update --init --recursive -q && cargo clippy -q -j 3 -p wcontainers-directx -p wcontainers-direct3d-11 --all-targets -- -D warnings"`.
- The scenes through our library copied in as d3d11.dll:
  `tools/graphics-rig/scenes --build "git submodule update --init --recursive -q && cargo build -q -j 3 -p wcontainers-directx" --dll 'd3d11.dll=%CARGO_TARGET_DIR%\debug\wcontainers_directx.dll' --api d3d11 triangle quads topology`.
- The forwarder placement is checked by the library tests (`own_handled` through our d3d11.dll
  beside wcontainers_directx.dll), not by the scenes command: the rig's replacement check fails any
  `--dll` library its programs do not report, and they report only d3d11.dll and dxgi.dll.
- Wine's Direct3D 11 tests through our library (one GPU job; build the library first):
  `tools/graphics-rig/remote "git submodule update --init --recursive -q && cargo build -q -j 3 -p wcontainers-directx"`,
  then `tools/graphics-rig/wine-tests run --dll 'd3d11.dll=%CARGO_TARGET_DIR%\debug\wcontainers_directx.dll' d3d11`
  (or `d3d11:test_a/test_b` for some). Logs land in pyro-parked/wcontainers-scratch/graphics-rig/wine-logs
  (the rig's script copies them there); this track keeps its own copies in logs/.
- The rig's own forwarding library (the control): `--build "cargo build -q -j 3 -p wcontainers-graphics-test-forward"`
  and `--dll 'd3d11.dll=%CARGO_TARGET_DIR%\debug\wcontainers_graphics_test_forward.dll'`.
- Logs on the desktop in C:\wc\out\phase-2-graphics-direct3d-11\scenes\d3d11\<scene>.log and .stdout.
- src/com/generated.rs: `python3 crates/wcontainers-directx/generate.py <windows-0.62.2 crate dir>`
  (the directory cargo unpacked, e.g. ~/.cargo/registry/src/<index>/windows-0.62.2), then
  `cargo fmt -p wcontainers-directx`. A method named in its `hand` table calls the hand-written one.
- Scratch (pyro-parked/wcontainers-scratch/graphics-direct3d-11): pngdiff.py says which cells of two
  PNGs differ; logs/ holds the runs' outputs (run6-*.log for run 6).

## How it is built

- `Device<B: Backend>` holds the back end, the immediate context's state and caches of programs (per
  vertex and pixel shader pair), vertex layout blocks (per input layout and strides) and vertex
  shaders' input signatures (by blake3 of the bytecode, for `CreateInputLayout`). Front objects hold
  back-end handles in `Kept`, which on drop hands the handle to a graveyard emptied at the next call.
- Shaders (src/shader.rs): the stage is read from the DXBC container's `SHDR`/`SHEX` version token,
  so bytecode of the wrong stage is `E_INVALIDARG` as Direct3D answers; then `convert` with the
  default `VariantKey`. Bytecode the converter rejects is `E_INVALIDARG`; what it cannot convert yet,
  and a shader reading the helper constants, is `E_NOTIMPL`, warned with the converter's reason.
- Binding sets: set 0 vertex, set 1 pixel, made from the reflection when a slot or shader changed.
- Empty slots read what Direct3D defines (`Zeros` in device.rs, each made at its first use): an
  empty vertex slot the input layout reads binds a 64 KiB buffer of zeros at stride zero; an empty
  constant buffer slot binds the same buffer at the shader's declared size; an empty texture slot a
  view of a 1x1 texture of zeros of the shader's dimension and kind (float, unsigned, signed); an
  empty sampler slot Direct3D's default sampler state (linear, clamped). What stands in for each
  slot is read from the converted module (`Shader::stand_ins`).
- Copies between textures: formats of one typeless group copy their bits (the interface's
  `copy_texture` allows them; the WebGPU back end copies sRGB twins directly and the others through
  a buffer); Direct3D 10.1's conversions between groups (block-compressed or shared-exponent and an
  integer format) are not served and remove the device; other pairs are invalid and dropped.
- Creation checks run in the front before any back-end call and answer `E_INVALIDARG` as Direct3D
  does; what is valid but not served answers `E_NOTIMPL` with a warning. A draw that cannot be served
  (an empty slot read as a multisampled or depth texture, indexed or indirect adjacency, lines under
  a multisampling or antialiased-line state) removes the device; COM methods not
  served answer `E_NOTIMPL`, and those without a result (on the device and context) remove the
  device, each warned once.
- The Windows library: `wcontainers_directx.dll` exports `D3D11CreateDevice`,
  `D3D11CreateDeviceAndSwapChain` and `D3D11On12CreateDevice`; the device, context and adapter are made
  with the `windows` crate's `implement`, the device children with our `counted!` (two counts, above);
  src/com/generated.rs is generated (above), each method
  taking the library lock (`com::lock`; on the device and context `com::Call`, which also prints the
  front's new slow-path warnings on leaving) and calling the hand-written method. The context keeps
  the program's bound objects (`Bound`, library references) for the `Get` methods; the device keeps
  its state objects by description. State the objects share
  (`Rc`s of front objects and of the device) sits in `Locked<T>`, which drops it under the lock.
- The back end is wgpu on Direct3D 12 (`com::backend`): what browsers' WebGPU uses on Windows, and it
  rasterises aliased lines by Direct3D's rule. On wgpu's default choice (Vulkan on the desktop) the
  topology scene differed on 43 pixels, all on line draws.
- build.rs makes d3d11.dll with the MSVC linker (found by the `cc` crate) from a generated `.def`:
  no code, each export forwarding to `wcontainers_directx`. It is written beside the library in the
  profile's output directory (target\debug).

## Tests

At G4 run 16: front.rs 41 (new: the read-only depth view test), tests/cache.rs 1, unit 3,
com.rs 15, programs.rs 15 (new: `two_pixel_formats_sample_load_and_copy_as_direct3d_does`,
example `own_packed`); all passing on the desktop, lint of both builds clean with warnings
denied (logs/g4s4*).

At G4 run 15: front.rs 40, tests/cache.rs 1 (new: `a_run_builds_at_its_start_what_the_last_run_drew`),
unit 3, com.rs 15, programs.rs 14; all passing on the desktop at 8862cef, lint of both builds
clean with warnings denied. The library tests run without `WC_GPU_CACHE`, so without a file.

- crates/wcontainers-direct3d-11/tests/front.rs, 25 tests since G4 run 5 (the bases in the
  helper block and indirect draws read back, in place of the removal test; building ahead; depth
  with stencil staging in Direct3D's layouts; empty input layouts), never run; 22 before (since G4 run 1 also: depth-stencil
  copies whole and of both aspects, typeless textures viewed through their group's members): the three scenes' calls on the recording
  back end (the scenes' own DXBC from the rig's corpus, converted), shader stages read from every
  corpus container, feature level choice, destruction after release, creation refusals (wrong stage,
  unconvertible shader, blend targets past the first, valid formats not served), dynamic constant
  upload, empty slots bound to zeros and the default sampler (made once), copies between formats of
  one group, of two groups and of Direct3D 10.1's conversions, draws that remove the device (a vertex id with a start location, lines under
  the other line rules, dual-source blending), a back-end loss reported at the next wait, slow-path
  warnings handed out once, a constant buffer grown to its shader's size, staging read backs started
  at the copy with maps that may not wait (the recorder can hold read backs until a submit), no new
  binding set for objects bound again, and a view of a target unbound and refused.
- crates/wcontainers-directx/tests/com.rs, 14 tests: QueryInterface identity and parents, children
  holding one reference on their device, the immediate context, views holding their resource with a
  reference the program does not see (and handing it back after the program released it),
  private data, unserved shaders refused (bad bytes `E_INVALIDARG`, unconvertible `E_NOTIMPL`), the
  forwarder resolving `D3D11CreateDevice` to the library's own, 8 threads making buffers at once,
  equal state descriptions giving the live object, the context's getters (with reference counts and
  `ClearState`), unused stages taking unbinding calls, a target unbinding its shader views (by mip),
  device creation's answers (adapter with a driver type, the swap chain entry point), and capability
  queries claiming only what creation serves.
- crates/wcontainers-directx/tests/programs.rs, 10 tests (1 the known failure, ignored). New in
  G4 run 2, both compared line by line with real Direct3D: `own_volumes` (1D and 3D initial data at
  slice pitches that are and are not whole rows, 3D boxes in `UpdateSubresource` and
  `CopySubresourceRegion`, staging 3D maps, a clear of two slices of a 3D target, a copy between
  levels and layers of 1D arrays, default view descriptions, refused 3D depth and wrong-dimension
  views, a dynamic texture written twice and the dynamic usages Direct3D refuses) and `own_queries`
  (an event polled to completion, predicates around drawn and scissored draws, overlapping
  predicates, a predicate ended twice, `GetData` before an end and at wrong sizes, interfaces and
  sizes; it calls `GetData` through the vtable, since the windows crate reads `S_FALSE` as success).
  `own_refused` now asks for counted occlusion and timestamp queries in place of a 3D texture. Our
  programs, system and ours; `own_swap_chain` (since G4 run 1; it makes a window, so the library
  tests run with `gpu-job --desktop`) and `own_depth_copy` compared line by line; `own_edges` (example: copies within and between groups, an empty vertex slot, empty texture,
  cube, constant buffer and sampler slots, each printed as hex rows) compared line by line with real
  Direct3D; `lifetimes` (example), a seeded random sequence of makes, references, releases, bindings,
  getters, `GetResource`, `GetDevice` and `ClearState`, printing every count after each step, run beside
  the system's d3d11.dll and ours and compared line by line; `own_handled` with a D32_FLOAT and with a D24_UNORM_S8_UINT depth-stencil view.
  `own_handled` now also writes and copies a buffer at unaligned byte ranges and checks every byte.
- `own_interfaces` (example, since G4 run 3; tests/programs.rs `interfaces_answer_as_direct3d_does`):
  the interfaces of the device, context, states and resources, 11.1's states and constant buffer
  ranges, DXGI on resources of each dimension, frame latency, and what failed creations and maps
  leave; identical to real Direct3D's output line by line. com.rs's identity test now checks that
  `ID3D11Device1` is the same object (it had checked it was refused).
- Last full run: 2026-11-17, G4 run 9, on the desktop from d51d9fb's tree: lint of both crates
  clean with warnings denied; front 33 passed; library com 15 and programs 13 passed (own_indirect
  new), none ignored.
- Before it: 2026-11-17, G4 run 3, on the desktop from commit 8d07989's tree: front 22
  passed; library 14 + 11 passed, none ignored (the depth copy passes since the merge); lint clean.
- Before it: 2026-11-17, G4 run 2, on the desktop from this tree after a clean rebuild of our
  crates: front 22 passed; library 14 + 9 passed and 1 ignored (the known failure); lint clean.
  The clean rebuild showed that a front test (`creation_checks_refuse_what_direct3d_refuses`)
  claimed Direct3D refuses zero factors on an independent target that does not blend; Wine's
  test_independent_blend and test_color_mask show real Direct3D accepts them, so the test now
  checks that and refuses only a target that blends with them.
- Before it: 2026-11-17, G4 run 1, on the desktop from this tree: front 22 passed; library
  14 + 7 passed and 1 ignored (the known failure, run alone with `--ignored`: fails as described);
  lint clean.
- Before it: 2026-11-17, run 6, on the desktop from this tree: front 20 passed; library 14 + 6
  passed (lifetimes at 8 seeds of 300 steps; `own_edges` identical to real Direct3D); lint clean.
- Before it: 2026-11-17, run 5, on the desktop from this tree: front 19 passed; library 14 + 4
  passed (lifetimes at 8 seeds of 300 steps), and with `--include-ignored` the known failure fails as
  described (0x887A0005); lint clean. Before it: 2026-11-17, run 4, on the desktop from this tree after the merges: 16 + 11 + 3
  passed, and the known failure run with `--include-ignored` fails as described (0x887A0005); lint
  clean.

## Departures from the plan

- Run 16: a uint clear of a view of floats (`ClearUnorderedAccessViewUint`) is written as the
  texels' bytes from the host (`write_texture` of the view's region filled with one element),
  not as a GPU clear of float values: floats lose -32768 of snorm16 and flush denormals, and
  Direct3D defines the clear as copying the values' low bits. Its cost is a host buffer of the
  region per clear; no interface slow path names it, so it is not counted (rare in programs).
- Run 16: a storage view of some slices of a 3D level is made on a texture of those slices
  (`View::window`), copied in before each draw or dispatch and out after, counted as
  `ReinterpretCopy` (the interface's text: a part the texture cannot be viewed as).
- Run 16: Direct3D 11.3's interfaces are served on every device, context and object
  (`ID3D11Device3`, `ID3D11DeviceContext3`, `ID3D11Texture2D1`/`3D1`, the views' `1` forms,
  `ID3D11RasterizerState2`, `ID3D11Query1`). Refused by name: `CreateQuery1` of one context
  type (one queue runs all work) and of predicates; textures of a layout other than undefined
  and conservative rasterization are invalid (not reported). `Flush1` with an event waits on the
  calling thread for the GPU, then signals it. Tiled resources stay unserved (not reported).
- Run 16: bgfx's examples load NVIDIA's NVAPI when the adapter is NVIDIA's and hand it our
  device, which it treats as its driver's (an access violation at start). Runs through our
  library put an empty nvapi64.dll beside the program (`--dll
  'nvapi64.dll=C:\wc\out\phase-2-graphics-direct3d-11\nonvapi\nvapi64.dll'`), so the load
  fails as it would in the container, where no host driver library exists; the Direct3D 12
  track does the same.

- Run 15: the native cache file (plan section 8: "natively it is a file") lives in the directory
  the host names in `WC_GPU_CACHE`, one file per program (`<exe stem>-<16 hex digits of BLAKE3 of
  its lower-case full path>.wcgpu`); without the variable nothing is kept and every run is a first
  run. Beside the library was tried first: the rig copies the library and the program into a fresh
  directory for each run (its LOCALAPPDATA too), so nothing there would reach a second run, and a
  game's install directory may not be writable. Building the record happens at the process's first
  device only (the program's start); Wine's test program makes hundreds of devices, and later ones
  build nothing. Processes appending one file at once can interleave entries; loading stops at the
  first damaged entry, so that costs entries, never wrong ones.
- Run 14: views reinterpreting a typeless texture and storage textures read through buffers are
  copied at every draw or dispatch that uses them, and out after every write, without tracking
  which copy holds the newest texels: correct whatever wrote the texture last, slow (counted as
  `ReinterpretCopy` and `AtomicsThroughBuffer`). A target view on a twin splits its pass at
  nothing more, but its copies run at each pass's start and end.
- Run 14: a stream output target's filled size lives in a 4-byte GPU buffer per resource (made at
  its first binding as a target); `DrawAuto` reads it back and draws directly, waiting for the
  GPU, where Direct3D draws from it on the GPU (the interface has no draw counted from a buffer
  word but indirect draws, and those need the stride's division in a program; not measured).
- Run 14: the stage helper's buffers and programs are kept per (vertex shader, input layout,
  geometry shader, pixel shader) for the device's life, as the programs are.

- Run 13: a texture view whose kind differs from its slot's declaration (integers read as floats
  or the reverse, undefined in Direct3D) reads as an empty slot, zeros, as Wine's test_generate_mips
  sees real Direct3D answer; WebGPU refuses binding such a view.
- Run 13: a cube texture whose layers are not whole cubes (Direct3D takes 9 or 11) is a 2D array in
  the back end, which the interface's cube flag (layers in sixes) does not describe; its cube views
  see its first sixes.

- Run 12: a constant buffer window's copy is counted as `ConstantBufferGrown` (a buffer of the
  shader's size whose bytes past the program's are zero), not a slow path of its own; its cost is a
  copy, and a pass split, each time the buffer is written while the window is bound.
- Run 12: `EnumAdapterByLuid` with another identifier answers `DXGI_ERROR_NOT_FOUND` (Microsoft's
  page names no code; Wine's DXGI answers so). `IDXGIFactory3::GetCreationFlags` answers 0, since
  the debug flag's layer is not served.
- Run 12: the device scene's line 25 (the device made on an adapter reports that adapter) differs
  when only d3d11.dll is replaced: the scene's adapter then comes from the system's DXGI and our
  device reports our own adapter and its identifier. With our dxgi.dll too (as in the container)
  it matches. Reporting the system's adapter would hand programs a factory that cannot make our
  swap chains.
- Run 12: depth bias in the program is refused on float depth formats (where the back end lacks
  the clamp): Direct3D's float step depends on each primitive's largest depth exponent, which the
  shaders crate's variant does not take.
- Run 11: resources made with `MISC_SHARED` are taken although nothing can open them: the
  resource works for its device, and the sharing call (`GetSharedHandle`) is refused by name.
  Keyed-mutex resources stay refused (`E_NOTIMPL`), since their whole use is the sharing.
- Run 11: building ahead's counts are printed at each present where they changed (not only when
  the device ends, which programs ended by the rig's `TerminateProcess` never reach). A program
  streaming new combinations prints a line per frame that built one.
- Run 11: `D3D11CreateDevice` takes only `D3D11_SDK_VERSION` (7), as Microsoft documents; real
  Direct3D was seen refusing 0 only.
- Run 11: vertex buffers get storage usage on back ends without `INSTANCE_STEP_RATE`, since a
  buffer cannot be known at creation not to feed a pulled input; the cost is not measured.

- Run 10: the plan (4.12) calls `LodBiasInShader` exact for implicit-level sampling. Measured on
  the RTX 3080 (own_min_lod, `samplers.ps_sample` with a 1.5 bias): one unit of green apart at 4
  of 256 texels, where the bias the program adds rounds the level apart from the sampler's own.
  The comparison takes rows drawn through a biased sampler within one unit a channel (Direct3D's
  filtering tolerance); every other row is compared exactly.
- Run 10: pipeline statistics' vertex shader invocations equal the vertices fetched, as the plan
  says for a GPU without vertex reuse; real Direct3D on the RTX 3080 reuses vertices even in draws
  without indices (9 invocations for 18 vertices in own_queries), so that member is not compared.
  Pixel shader invocations count covered pixels (or samples, for a per-sample shader), not helper
  pixels; Direct3D only promises a lower bound there, and the tests check that.
- Run 10: stream output statistics answer zero and the overflow predicates false, since no stream
  output can run (geometry shaders with stream output are refused, targets cannot be bound). They
  must count once stream output is served.
- Run 9: a sampler whose `MinLOD` is above its `MaxLOD` is made with the maximum raised to the
  minimum, since WebGPU refuses such a range; what real Direct3D samples then is not checked (the
  one Wine case uses no texture).
- Run 8: timestamps are answered with the disjoint query's `Disjoint` TRUE and CPU times, since
  no back end writes GPU timestamps yet (the WebGPU back end declares no `TIMESTAMPS`). Where a
  back end declares `TIMESTAMPS` the front should use them; not done.
- Run 8: the occlusion counter is given a pass tracker that only holds the bound targets; the
  front still opens and ends its own passes (section 4.7 has implicit fronts on the tracker).
  Every pass end flushes the counter while a counted query runs, which counts more often than the
  tracker's own route would (exact either way).
- Run 8: `CheckMultisampleQualityLevels` and `CheckFormatSupport` are served through a copy of the
  device's function table with those two entries replaced, made once a process: the windows
  crate's generated entries cannot leave an out value written on failure or check its pointer.

- Run 7: every default-usage 2D texture of a format that multisamples and resolves gets render
  usage on the back end, since it may be a resolve's destination, which Direct3D does not ask to be
  bound as a target and the back end resolves only into attachments. The alternative, resolving
  into a temporary target and copying, costs a copy per resolve; the cost of the render usage is not
  measured.

- An indirect draw whose vertex program reads `SV_VertexID` or `SV_InstanceID` has its arguments
  read back and is drawn directly (`IndirectReadBack`, which pauses for the GPU), not read from the
  arguments in the program (`DrawBaseFromArguments`): the shaders crate refuses that variant by
  name. Exact, slow; it goes when the converter builds `DrawBaseSource::Arguments`.
- The helper block of the draw's bases is kept between draws with equal bases on the strength of
  the front being the only writer of the back end's data ring (the interface's defaults that write
  it, `clear_rects` and spilled immediates, are not called by this front). A front that starts
  calling them must write the block again after them.
- Building ahead pairs shaders and fits tuples by location and scalar type, not by semantic name,
  so that a pipeline it prepares is one the back end can build (a failed `prepare` loses the
  WebGPU device). Pairs that would fail only on something else (a vertex attribute the format of
  which WebGPU lacks) are not filtered; none is known.

- DXGI's factory entry points, and since G4 run 6 `D3D11CreateDevice` and
  `D3D11CreateDeviceAndSwapChain`, decide by the caller's module whose library a call wants:
  callers in this library (wgpu), in the back end's own libraries (ANGLE's directory, WebGL 2
  build) or in the system directory get the system's dxgi.dll or d3d11.dll, every other caller
  ours. The Direct3D 12 track decides by thread (its device thread); this library has no device
  thread yet (calls run on the caller's thread), so the caller's module is the mark that exists.
- A factory made by `CreateDXGIFactory*` describes its adapter from the caps of the first back end
  made, or of one made for them and dropped while none was made (one extra device at most).
- Direct3D leaves comparing through a sampler that does not compare (and the reverse) undefined;
  the front binds a twin comparing with the function the description names, or not comparing.
- The WebGL 2 build is a cargo feature (`webgl`) choosing the back end type: section 10 builds the
  library "a second time with B = the WebGL 2 back end", and the feature is how one crate is built
  twice with another type. It reads `WEBGL_ANGLE_DIR` and `WEBGL_OFF`, as the WebGL 2 back end's
  tests do; the build is a desktop test route, ANGLE is never shipped.

- No call buffer or device thread yet: COM calls run the front on the calling thread under one
  re-entrant lock (section 4.8 puts the channel in the second batch).
- Device children are made by our own `counted!` macro (crates/wcontainers-directx/src/com/counted.rs),
  not the windows crate's `implement`, which section 10 names: Direct3D keeps the program's
  references apart from the library's own, and `implement` has one count. The macro builds
  windows-core's object layout (its vtables, `ComObjectInner`, the dynamic-cast IID), so
  windows-core is pinned to exactly 0.62.2 in the library's Cargo.toml; moving to a newer
  windows-core means checking that layout again. The device, context and adapter still use
  `implement`.
- The recording back end is in the front crate (`record.rs`), not G1's wcontainers-gpu-test, which
  landed in run 4; moving over is item 8 of what comes next.
- The front does not use G1's pass tracker or combination tracker yet (they landed in run 4; item 7
  of what comes next); it opens a pass at the first draw after its targets change and ends it before
  copies, maps and clears.
- Buffers are padded to the back end's copy granule (4 bytes on WebGPU); the program sees its own
  `ByteWidth`.
- State objects are kept by the device for its life once made (DXVK does the same); Direct3D may free
  one whose references all went. The program sees the same answers (a count of one on the object it
  gets back); memory grows with the number of distinct descriptions (Direct3D caps it at 4096 each).
- `D3D11CreateDeviceAndSwapChain` with a swap chain asked for answers `E_NOTIMPL`: no swap chain yet.
- `CheckFeatureSupport` answers `D3D11_FEATURE_THREADING` with both FALSE (calls are serialised by
  the library's lock; deferred contexts are not served).
- The feature level is 11_0 at most; 11_1's requirements have not been checked.
- Stencil fields are validated only with stencil on, the depth function only with depth on.
- The adapter reports zero for its memory sizes (the infrastructure crate's rule; no browser API
  reports them). Programs that budget by `DedicatedVideoMemory` see none; whether to report a stated
  estimate instead is the owner's call.
- `D3D11CreateDevice` with an adapter uses ours whatever is passed; the debug flag answers
  `DXGI_ERROR_SDK_COMPONENT_MISSING`; software, WARP, reference and null drivers `DXGI_ERROR_UNSUPPORTED`.
- Only d3d11.dll is a forwarder (section 10 names d3d12.dll and dxgi.dll too): the library exports
  no Direct3D 12 or DXGI entry points yet, and a forwarder must not name exports that do not exist.
- The library draws through wgpu on Direct3D 12 only (section 10 allows Direct3D 12 or Vulkan): a
  machine without Direct3D 12 gets `DXGI_ERROR_UNSUPPORTED` from `D3D11CreateDevice`.
- Lines under `MultisampleEnable` or `AntialiasedLineEnable` are refused at the draw on any target.
  Microsoft's pages disagree on whether these apply to single-sample targets; this follows the
  reading that they always pick the quadrilateral and alpha rules (as DXVK does). Not yet measured
  against real Direct3D.

## Interface changes

None in G4 runs 3 to 12.

- c8a7be9 "interface: copied combined depth-stencil textures whole in copy_texture": the
  documentation of `copy_texture` and `TextureRegion::stencil`, and the plan's sentence in 4.2: a copy
  between two textures of a combined depth-stencil format copies both aspects in one call, naming
  none; copies to and from buffers name one aspect.
- 7f73347 "interface: named the slow path of a constant buffer grown to its shader's declared size,
  and said a depth attachment holds both aspects": `SlowPath::ConstantBufferGrown`, and one sentence
  on `TextureViewDesc::stencil` (a combined-format view a pass draws into holds both aspects), with
  the plan's text in section 7 (Direct3D 11's constant buffers).

## Measurements

- 2026-11-18, G4 run 16, the desktop (i9-9900K, RTX 3080) through the queue's hazard group, not
  alone (other tracks' hazard jobs ran one at a time before ours; Mac load average about 20):
  module size of section 10, the release Windows library built at 01:39 desktop time from the
  tree after ad4cc1d (the front, the shaders crate with dxbc-spirv, the back end; Direct3D 12 is
  the other track's own library, not in it): wcontainers_directx.dll 12,699,136 bytes on WebGPU
  (wgpu) and 8,163,840 bytes on WebGL 2 (glow over ANGLE, ANGLE not counted); the forwarders
  d3d11.dll and dxgi.dll 1,024 bytes each (11,617,792 and 7,124,480 at run 11). There is still
  no WebAssembly build of the library (the container's call channel and stand-ins are second
  batch), so no `directx-webgpu.wasm` size and no load time. Wine's d3d11 file at d5f9d35: 118
  of 166, the job 6 min 35 s from start to end (02:21:24 to 02:27:59, including the test
  runner's own build). Real programs through the whole library at d5f9d35, 120 frames each, every
  compared frame identical to real Direct3D's (largest difference 0): sdk-tutorial02, 04 to 07
  (0.9 to 1.0 s), chocolate-doom 3.2 s, atg-simplelightingpc 3.2 s, atg-simpleinstancingpc 3.4 s,
  godot-platformer-2d-d3d11 13.2 s (logs/g4s4o-programs.log).

- 2026-11-18, G4 run 15, the desktop (i9-9900K, RTX 3080) through the queue, not alone (the
  hazard group one job at a time; at 01:03, after the runs: Mac load average 21.8 over one
  minute and 56.1 over fifteen, desktop CPU load 33 %): Wine's d3d11 file 114 of 166
  in 193 s. Real programs through the whole library, 120 frames each, first run (recording) and
  second (built from the record at start): sdk-tutorial07 1.0 s and 1.3 s, chocolate-doom 3.0 s
  and 3.2 s, atg-simplelightingpc 3.1 s and 3.2 s, atg-simpleinstancingpc 3.3 s both,
  godot-platformer-2d-d3d11 13.3 s and 14.5 s (wall time of a run including start and frame
  saving, single runs: no difference shown either way at this size). Building-ahead hits: see
  Where things stand. Cache files after the two runs: 1,623 bytes (sdk-tutorial07) to 141,207
  bytes (Godot). Module size not measured again.

- 2026-11-17/18, G4 run 14, the desktop (i9-9900K, RTX 3080) through the queue, not alone (the
  rig's hazard group runs our jobs one at a time beside other tracks' reference jobs; Mac load
  average 47 at 23:51): Wine's d3d11 file 113 of 166 in 198.7 s (23:35) and 209.9 s (00:08).
  Real programs through the whole library, 120 frames each: chocolate-doom 4.4 s,
  atg-simplelightingpc 4.1 s, atg-simpleinstancingpc 3.7 s, atg-simplecomputepc 3.5 s,
  godot-platformer-2d-d3d11 17.9 s. At the end the desktop's C:\wc\target\phase-2-graphics-direct3d-11
  (14.6 GB) and the checkout's own target (1.6 GB, from `cargo check`) were deleted. Building ahead on them: no combination built ahead (as
  before; item 8 of What comes next). Module size not measured again.

- 2026-11-17, G4 run 13, the desktop (i9-9900K, RTX 3080) through the queue, not alone (other
  tracks' jobs beside; Mac load average 57 to 65 at 22:42): Wine's d3d11 file 104 of 166 in
  197.5 s. Real programs through the whole library, every compared frame identical to real
  Direct3D's (largest difference 0): sdk-tutorial02, 04 to 07, chocolate-doom,
  atg-simplelightingpc (3.2 s), atg-simpleinstancingpc (3.4 s), 120 frames each
  (logs/g4s1e-programs.log). Building ahead and module size: not measured again this run.

- 2026-11-17, G4 run 12 (RTX 3080, through the queue, not alone; desktop CPU load 2 % at 19:33,
  Mac load average 18 to 23): Wine's d3d11 file 90 of 162 in 169 s. Real programs through the
  forwarding d3d11.dll, dxgi.dll and wcontainers_directx.dll at d6d3441, every compared frame
  identical to real Direct3D's (largest difference 0): sdk-tutorial02, 04, 05, 06, 07,
  chocolate-doom (4.3 s), atg-simplelightingpc (3.1 s), atg-simpleinstancingpc (3.3 s), 120
  frames each (logs/g4r12s-programs.log). Rig scenes: see Where things stand. Module size and
  building ahead's hits not measured again (no change that touches them).

- 2026-11-17, G4 run 11 (RTX 3080, through the queue, not alone; desktop CPU load 25 % at 18:29,
  Mac load average 9 to 15): Wine's d3d11 file 88 of 162 in 184 s. Real programs through ours,
  every compared frame identical to real Direct3D's (largest difference 0): sdk-tutorial02, 04,
  05, 06, 07, chocolate-doom, atg-simplelightingpc, atg-simpleinstancingpc (3.1 to 3.3 s for the
  Tool Kit samples' 120 frames). Building ahead on those eight: 0 combinations built ahead, so 0
  hits; 1 to 4 combinations built at their first draw per program (their output.log copies in
  scratch programs/g4r11/).
- 2026-11-17, G4 run 11, the rig's Direct3D 11 scenes at 64b209a (same machine): through our
  library 76 outputs match, 65 fail, 1 absent (75, 67, 1 at the start of the run; the draws scene
  now matches; logs/g4r11h-scenes.log). Through the WebGL 2 build, now that scenes ask for the
  level they need: 26 match (largest difference 0: copies, deferred, dynamic, instanced, quads,
  queries, stencil, swapchain, 1D, 1D array, 2D array, 3D, cube and mip textures, topology,
  triangle, levels-rules), 23 absent (they need 10_1 or 11_0 and the build offers 10_0), the rest
  stop at what the WebGL 2 back end lacks: dual-source blending (blend), texture views (views), a
  depth bias clamp (depth), polygon mode (raster), reading half floats back (mips), formats
  (formats), and the constant window (constants) and border colours (samplers) as on WebGPU
  (logs/g4r11h-webgl.log).
- 2026-11-17, G4 run 11, module size: the release Windows library (wcontainers_directx.dll, the
  front, the shaders crate with dxbc-spirv, and the back end with wgpu) is 11,617,792 bytes on
  WebGPU and 7,124,480 bytes on WebGL 2 (glow over ANGLE, ANGLE not counted). No WebAssembly
  build of the front exists yet, so no module size of it.

- 2026-11-17, G4 run 8, Wine's d3d11 file through ours (RTX 3080, through the queue, not alone;
  Mac load average about 20): 72 of 162 in 210 s at f66ffe4, 74 of 162 in 199 s at 5fc1e89. test_occlusion_query alone takes
  28.6 s here (each counted query's end waits for its count).
- 2026-11-17, G4 run 8, Godot's 2D platformer through ours: 120 frames in about 100 s of job time
  (cold shader compiles in ANGLE; frames show only the clear colour, above).

- 2026-11-17, G4 run 3, Wine's d3d11 file through ours (RTX 3080, through the queue, not alone;
  Mac load average 30 to 40): 73 of 166 in 4 min 27 s (one 120 s timeout, test_dispatch_indirect).
- 2026-11-17, G4 run 3, real programs beside real Direct3D through the rig's forwarding library
  (same machine): sdk-tutorial07 1.3 s, chocolate-doom 2.6 s, godot-platformer-2d-d3d11 6.1 s,
  bgfx-01-cubes 2.5 s, 120 frames each; through ours sdk-tutorial07 1.8 s, the others failed
  (above).

- 2026-11-17, G4 run 2, every Direct3D 11 rig scene through our library copied in as d3d11.dll
  (RTX 3080, through the queue, not alone; Mac load average 40 to 60): 40 outputs match with
  largest difference 0 (9 before): triangle, quads, topology, instanced, dynamic, targets x4,
  textures x8 (1d, 1d_array, 2d_array, 3d, cube, cube_array, mips, bc), mips x18, swapchain x5.
  The others stop with an error code: blend (dual-source), depth (depth bias), stencil (the known
  back-end gap), raster, samplers, views (reinterpreting view), queries (counted occlusion),
  compute, geometry, tessellation, streamout, deferred `E_NOTIMPL`; msaa `E_INVALIDARG` (no
  multisampling reported); formats `E_FAIL`; constants `E_NOINTERFACE`; copies (10.1's
  conversion); levels `DXGI_ERROR_UNSUPPORTED`.
- 2026-11-17, G4 run 2, Wine's d3d11 file through ours: 65 of 166 in 4 min 46 s, then 73 of 166 in
  4 min 23 s (one 120 s timeout in each).

- 2026-11-17, G4 run 1, Wine's d3d11 file through our library copied in as d3d11.dll (RTX 3080,
  through the queue, not alone; Mac load average about 40 to 60): before swap chains 11 of 166 pass
  in 2 min 10 s; after, 56 of 166 in 4 min 44 s (most of the longer time is one 120 s timeout).
  Real Direct3D's baseline (the rig's): 162 pass in 51.5 s.
- 2026-11-17, G4 run 1, `own_swap_chain` beside the system's library and ours: identical output.

- 2026-11-17, run 6, after the merges and fixes, through our library copied in as d3d11.dll
  (through the queue, not alone; desktop CPU load 28 % sampled after; Mac load average 47): the same
  9 pictures match with largest difference 0 (triangle, quads, topology, instanced, dynamic,
  targets x4); the same 9 through our forwarder and through the rig's own forwarding library. The
  rig now has more Direct3D 11 scenes (151 outputs); the others stop with an error code: copies now
  gets past the typeless copies and stops at a BC-to-integer copy (10.1's conversion, not served);
  depth gets past depth with stencil and stops at `E_NOTIMPL` for depth bias; stencil stops at the
  back-end gap above; formats gets `E_FAIL` from `CheckFormatSupport` (Direct3D's answer for a
  format the device does not support: one of its 22 formats is not served here); blend at
  dual-source blending; constants and swapchain `E_NOINTERFACE`; levels `DXGI_ERROR_UNSUPPORTED`;
  the rest `E_NOTIMPL`.
- 2026-11-17, run 6, `lifetimes` seeds 1 to 8 at 300 steps each, identical beside both libraries.

- 2026-11-17, run 5, after the merges and fixes, every Direct3D 11 rig scene through our library
  copied in as d3d11.dll (through the queue, not alone; desktop CPU load 44 % sampled after; Mac load
  average 53): the same 9 pictures match with largest difference 0 (triangle, quads, topology,
  instanced, dynamic, targets x4); the same 9 through our forwarder and through the rig's own
  forwarding library. The other scenes stop with an error code as before, except formats, which now
  passes `CheckFormatSupport` and stops with `E_NOTIMPL` at an `R8G8B8A8_SNORM` target.
- 2026-11-17, run 5, `lifetimes` beside the system's d3d11.dll and ours: seeds 1 to 8, 300 steps each
  (4,422 lines in all), identical; the device's last `Release` answers 0 both ways.

- 2026-11-17, run 4, after the merges, every Direct3D 11 rig scene through our library copied in as
  d3d11.dll (through the queue, not alone; desktop CPU load 23 % sampled after; Mac load average
  42): 9 pictures match exactly (largest difference 0): triangle, quads, topology, instanced,
  dynamic (new) and the four targets outputs. The other scenes stop with the same error codes as in
  run 3, except dynamic. The five matching scenes match the same through our forwarder and through
  the rig's own forwarding library (5 of 5).
- 2026-11-17, run 3, every Direct3D 11 rig scene through our library copied in as d3d11.dll (desktop
  CPU load 12 % sampled after; Mac load average 40): matching exactly (largest difference 0) are
  triangle, quads, topology, instanced (new: the grown constant buffer) and the four targets
  outputs. Every other scene now stops with an error code instead of a wrong picture: draws, raster,
  msaa, formats, textures, samplers, views, queries, mips, compute, geometry, tessellation,
  streamout, deferred `E_NOTIMPL`; constants and swapchain `E_NOINTERFACE` (Direct3D 11.1 and DXGI
  interfaces not offered); levels `DXGI_ERROR_UNSUPPORTED` (feature levels below 10_0); blend
  (dual-source refused), copies (typeless copies refused), depth and stencil (the known failure),
  dynamic (FXC) removed. The four matching scenes match the same through the forwarder, and through
  the rig's own forwarding library.

All on the desktop (Windows 10, i9-9900K, RTX 3080) through the queue, not alone; desktop CPU load
26 % when sampled after the last runs; Mac load average 18 to 25.
- 2026-11-17, run 2: triangle, quads, topology through our library (both placements) 0 of 65536
  pixels differ each, largest difference 0, wgpu on Direct3D 12.
- 2026-11-17, run 2, wgpu's default adapter (Vulkan), before choosing Direct3D 12: topology 43 of
  65536 differ (largest difference 229), all in the six line cells; triangle and quads 0. Run 1 had
  measured the same 43 and wrongly put it on Direct3D 12.
- Rig corpus through the shaders crate (its corpus test, run here for the verdicts): 56 of 79 shaders
  convert to WGSL (3 of them not to GLSL ES); the rest name their reason (geometry, hull and domain shaders, storage images and
  counters, atomics, `SV_PrimitiveID`, a vertex shader without a position).

## Known issues

- Threads: one lock serialises all calls; no parallelism between a program's threads.
- An occlusion set running out of slots inside a pass (4096 stretches in one pass) ends the pass
  and opens the next on a fresh set; that split is not counted as a slow path.
- No front (recording back end) tests for 1D and 3D textures or queries: the front's recorder
  keeps 2D layers only and has no query sets. They are checked against real Direct3D instead
  (`own_volumes`, `own_queries`, the rig's textures scene); the recorder goes with item 8.
- Program and vertex layout caches are pruned when a shader or layout dies; binding sets are made per
  change and destroyed when replaced.
- The rig's replacement check cannot see wcontainers_directx.dll behind the forwarder (above).

## Asked of other tracks

- Shaders track (G4 run 16): `MultisampledArrayTexture` (a pixel shader declaring
  `Texture2DMSArray`; plan 4.2 has the front hold such a texture as one texture per layer, so the
  conversion needs a variant reading per-layer bindings): bgfx's examples create one at start and
  stop (`BGFX FATAL ... Failed to create fragment shader`).
- WebGPU back end (G4 run 16): `R32G32B32_*` textures (`Rgb32Float`, `Uint`, `Sint`) held as
  four-channel ones (`FormatConverted`, as the 16-bit packed formats); Wine's
  test_create_texture2d test 21 asks for one.
- Infrastructure crate (G4 run 16): a swap chain on the desktop window (`GetDesktopWindow()`):
  wgpu's surface refuses it ("Invalid surface" at `Surface::configure`) and the device is lost;
  Wine's test_desktop_window.

- Rig (G4 run 14): the runner bug of run 13 is still in desktop/wine-run.ps1 (now line 256 after
  the reset guard's lines); this track's fixed copy was refreshed from the new runner, so the
  quarantine and the reset checks still apply to its runs.
- Shaders track (G4 run 14): `PRIMITIVE_ID_FROM_STAGE` (the rig's geometry scene stops there: a
  pixel shader behind a geometry shader reading SV_PrimitiveID), `VertexWithoutPosition` both in
  the plain conversion and in the stage helper's vertex compute form (stream output from a vertex
  shader writing no position, six of Wine's tests), `GeometryStreams`, and hull and domain stages
  in `Stages`.

- Rig (G4 run 13): tools/graphics-rig/desktop/wine-run.ps1 line 224, `$allowed = if
  ($known.ContainsKey($name)) { $known[$name] } else { @() }`, gives `$null` (and a one-element
  set unrolled to a string) when a test without known failures fails a check, so the whole run
  stops ("You cannot call a method on a null-valued expression"). Assigning the set directly
  (`$allowed = New-Object 'System.Collections.Generic.HashSet[string]'; if (...) { $allowed =
  $known[$name] }`) fixes it; this track runs such a copy from scratch.
- Shaders track (G4 run 13): both fronts read the typed views a DXBC program declares
  themselves (this one in `shader::typed_views`, Direct3D 12's in its dxbc.rs) to give the
  conversion its formats; the converter's reflection could list them.

- WebGPU back end (G4 run 11, atg-simplemsaa_pc): `write_texture` of a whole level of a
  block-compressed texture smaller than a block (a 2x2 level of BC) passes the level's texel size
  to wgpu, which refuses it ("Copy width is not a multiple of block width"); WebGPU wants the
  level's size rounded up to whole blocks there, WebGL 2 the texel size, so it belongs in the back
  end.

- WebGPU back end (G4 run 10, Wine's test_format_compatibility): `copy_texture` from `R16Unorm`
  into `Depth16Unorm` (one typeless group in Direct3D) fails "a copy into depth from a format
  outside its typeless group" and the device is lost.
- Interface (G4 run 10): the merged samplers of a minimum level-of-detail clamp are numbered from
  `CLAMPED_SAMPLER_BINDING` in the one helper set by both draw stages, so a draw whose vertex and
  pixel shaders both sample clamped textures cannot bind both; the front refuses it by name.
  Separate ranges per stage (as the helper blocks have) would serve it.

- Shaders track (G4 run 8): `GeometryShader` (ANGLE links a point sprite geometry shader into
  every program Godot uses) and `TypedBuffer` (bgfx's compute shaders) are what stop the third
  engine family.
- WebGPU back end: `TIMESTAMPS` (wgpu's timestamp queries), so timestamps need not be reported
  disjoint.

- WebGL 2 back end, natively on ANGLE (from the WebGL 2 build of this library): a back end makes
  its EGL context current when made and never again, and dropping one makes nothing current and
  terminates EGL's display, which every back end on the default display shares. Wine's tests often
  hold two devices at once, so through the library the second device's context would take the
  first's calls. Making the context current again when another back end on the thread made its own
  current (and not terminating a display others use) would serve them.

- Interface and shaders track (from Wine's test_depth_stencil_sampling): Direct3D's stencil views
  (`X24_TYPELESS_G8_UINT`, `X32_TYPELESS_G8X24_UINT`) give the stencil in the green channel;
  WebGPU's stencil-aspect view gives it in red. It needs a view swizzle (the interface names
  `VIEW_SWIZZLE` but `TextureViewDesc` has no swizzle field yet; WebGPU's texture component swizzle
  where present) with a program rewrite as the default, which Direct3D 12's and Vulkan's component
  mappings need too.
- Shaders track (from Wine's test_cb_relative_addressing): a dynamically indexed constant buffer
  read past the array the shader declares, inside the bound buffer (Direct3D reads the buffer's
  bytes; up to 4096 registers), comes back as the clamped last element. Declaring such buffers at
  their exact size is what lets small buffers bind without `ConstantBufferGrown`; reading them as
  storage (`ConstantsAsStorage`, bounded by the bound range) would serve both.

- Infrastructure crate (Direct3D 12 track), from Wine's test_swapchain_views and
  test_swapchain_flip: list the sRGB twin in the surface's view formats, so a back buffer takes
  an `R8G8B8A8_UNORM_SRGB` view as DXGI allows; give out every buffer of a sequential
  (bit-block-transfer) swap chain, as DXGI does (today buffer 0 only); and give back buffers
  binding usage when the swap chain asks for `DXGI_USAGE_SHADER_INPUT`.
- WebGPU back end: the vertex formats WebGPU lacks (`VertexFormatConverted`, "not built yet" in
  its log; Wine's test_vertex_formats, format 0x1a).

- WebGPU back end: follow interface change c8a7be9 (copy both aspects of a combined depth-stencil
  format in `copy_texture`, with `wgpu::TextureAspect::All`); still not done at G4 run 2's merge.
- WebGPU back end: `write_texture` of a 3D region of several slices with `rows_per_layer` above the
  extent's rows lost every slice after the first (seen on a 2x2x2 level, rows_per_layer 3, through
  `queue.write_texture`). The front now writes such data one slice at a time, so nothing of ours
  depends on it; found with `own_volumes`.
- WebGPU back end: declare `TIMESTAMPS` (wgpu's timestamp queries) and, where the adapter counts
  samples exactly, `OCCLUSION_COUNT`; Direct3D 11 programs ask for both (census: queries in 8 of 14).
- Rig: let the scene programs report where every `--dll` library came from (they print d3d11.dll and
  dxgi.dll only), so the forwarder placement can be checked by the scenes command.

## Licences

G4 run 16 added the windows crate's `Win32_System_Threading` feature (same crate, MIT OR
Apache-2.0; `SetEvent` for `Flush1`) and dropped `half` from the front (no longer used). On the
desktop it made one empty file, C:\wc\out\phase-2-graphics-direct3d-11\nonvapi\nvapi64.dll
(0 bytes), placed beside bgfx's examples so NVIDIA's NVAPI does not load (Departures).

G4 run 15 added `serde` 1 (MIT OR Apache-2.0, already in the lock file) to the front, and our
`wcontainers-shaders` and `blake3` 1 (CC0-1.0 OR Apache-2.0, already a dependency of the front) to
the library. Installed nothing on the desktop; the programs' cache files are written under
C:\wc\out\phase-2-graphics-direct3d-11\gpu-cache.

G4 run 13 added `half` 2.7 (MIT OR Apache-2.0, read in its Cargo.toml; already in the lock file
through naga) for half floats in uint clears, and installed nothing on the desktop (the runner
copy in C:\wc\out\phase-2-graphics-direct3d-11\rigfix is a 15 KB script).

G4 run 12 added no dependency and installed nothing on the desktop.

G4 run 11 added the windows crate's `Win32_Graphics_Direct3D10` feature (same crate, MIT OR
Apache-2.0) and installed nothing on the desktop.

G4 runs 9 and 10 added no dependency and installed nothing on the desktop.

G4 run 8 added no dependency and installed nothing on the desktop.

G4 run 6 added our `wcontainers-gpu-webgl` to the library as an optional dependency (the `webgl`
feature), which brings `glow` 0.18.0 (MIT OR Apache-2.0 OR Zlib), `khronos-egl` 6.0.0
(MIT/Apache-2.0), `libloading` (ISC) and `bcdec_rs` 0.2.1 (MIT), all already in the lock file and
read in their Cargo.toml. ANGLE (BSD-3-Clause) is a desktop test tool the WebGL 2 track set up;
nothing of it is shipped. Nothing installed anywhere.

G4 run 5 added no dependency and installed nothing anywhere.

G4 run 4 added the windows crate's `Win32_System_Diagnostics_Debug` and
`Win32_System_SystemInformation` features (same crate, MIT OR Apache-2.0). On the Mac (not the
desktop) it added rustup's x86_64-pc-windows-msvc standard library for 1.99.0 (rust-std, MIT OR
Apache-2.0), for the check-only cross build. Nothing installed on the desktop.

G4 run 3 added the windows crate's `Win32_Graphics_Gdi` and `Win32_Security` features (same crate,
MIT OR Apache-2.0; `IDXGISurface1` and `IDXGIResource1` name their types). Nothing installed on
the desktop.

G4 run 2 added no dependency and installed nothing on the desktop.
Added in G4 run 1: the windows crate's `Win32_System_LibraryLoader` and
`Win32_UI_WindowsAndMessaging` features (same crate, MIT OR Apache-2.0); the front now depends on
our infrastructure crate. Nothing installed on the desktop.
Added in run 5: `rand` 0.9.5 and `rand_chacha` 0.9.0 (MIT OR Apache-2.0, already in the lock file)
as dev-dependencies of the library, for the `lifetimes` program. Added in run 3: `naga` (the
workspace's, MIT OR Apache-2.0) to the front, for the declared sizes of constant buffers (still a direct
dependency); our `wcontainers-directx-graphics-infrastructure` to the library. Added in run 2:
`cc` 1.11.2 (MIT OR Apache-2.0, build dependency), `parking_lot` 0.12.5 (MIT OR
Apache-2.0), both already in the lock file; our `wcontainers-shaders`, which builds dxbc-spirv (MIT)
in. Earlier: `windows` and `windows-core` 0.62.2,
`wgpu` 30.0.2, `naga` 30.0.2 (MIT OR Apache-2.0), `pollster` 1.0.1 (Apache-2.0/MIT), `blake3` 1.8.7
(CC0-1.0 OR Apache-2.0). Nothing installed on the desktop beyond build output in
C:\wc\target\phase-2-graphics-direct3d-11 (including the tests' directx-tests\ copies) and pictures in C:\wc\out.
