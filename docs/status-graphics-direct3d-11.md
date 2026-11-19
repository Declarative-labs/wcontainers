# Direct3D 11 front status

Branch: phase-2-graphics-direct3d-11. Plan: docs/phase-2-graphics.md, milestone G4 (section 12).
Runs so far: six on G4a, the first slice; then runs 1 to 24 of G4 as a whole (2026-11-17 and
18; runs 13 to 24 are the orchestrator's second series on G4).

## Where things stand

G4 run 24 (2026-11-18, from about 18:15 Pacific). Run 23 had committed its code (728773d0) but
not its status or a push; both are done first. Merged origin/phase-2-graphics-webgpu, -webgl and
-direct3d-12 cleanly (-graphics, -shaders and -rig had nothing new). The WebGPU back end now asks
wgpu for `PRIMITIVE_INDEX` and declares it; the interface gained `Features::FINE_DERIVATIVES`.
The literal check is still not adopted: docs/literals.md (origin/phase-1-literals) says a track
adopts it only through main once main has it, and origin/main holds no wcontainers-literals yet.
- Geometry shaders and stream output after a domain shader (Wine's test_quad_tessellation): a
  geometry shader object bound with hull and domain shaders now goes through the staged path
  with the stage helper made by `Stages::tessellated_with` and run by `run_after_domain`; the
  domain stage's set is made visible to compute for it (`domain_compute`) and again for the
  tessellated draw. With a geometry stage its output is drawn as any geometry stage's (layered
  runs included); with stream output alone the tessellated points are still drawn when stream 0
  is. The stream output counters and statistics are the geometry path's. A geometry shader object
  made from a domain shader's bytecode with a declaration is accepted (it was `E_INVALIDARG`).
  Patches of more control points than the hull shader reads are taken (the stage helper accepts
  them). The front test of hull and domain shaders now also streams the domain stage's output.
- Clip distances from a domain shader where the back end has none: the domain program is
  converted with the `CLIP_DISTANCE` variant at the linked location, as the vertex program is,
  and the pixel program applies them; before a geometry shader they are refused by name.
- The stage helper's vertex conversion, the hull and domain conversions and the pixel shader's
  interpolation are each one function now (`pulled_vertex`, `tessellation_programs`,
  `interpolation`), used by the plain, geometry and tessellation paths.
- `B8G8R8X8_UNORM` vertices stay refused: the specification reads an X component as missing (A
  is 1.0), which `unorm8x4-bgra` does not do (Asked of the interface track).
- Stencil views reading 0 (test_depth_stencil_sampling, the rig's views scene) were this front's
  fault, not the back end's: the binding check took a stencil view's storage format (depth with
  stencil) for "the other kind" than the unsigned integers the program reads, so the slot was
  bound to the empty slot's 1 by 1 stand-in (found by the new own_cases `stencil` case: width 1,
  stencil 0, where real Direct3D reads 4 and 100). A stencil view now reads as unsigned
  (`View::read_kind`). The case is compared with real Direct3D by the library test
  `stencil_views_read_the_stencil_as_direct3d_does`; test_depth_stencil_sampling passes in both
  files and the views scene matches.
- The com test of a shader refused at creation used `geometry.ps_id`, which is served now that
  the back end has the primitive index; it uses a vertex shader writing `SV_CullDistance`
  (compiled by the system's d3dcompiler), which the shaders crate still refuses.
- The front's DXGI numbers in format.rs are the infrastructure crate's names, and its vertex
  format table is the infrastructure crate's (Literals).
- Results (desktop, RTX 3080, through the queue, not alone; Measurements): at dd123eec, Wine's
  d3d11 file 140 of 165 (138 at run 23), d3d10core 103 of 111 (102), the WebGL 2 build's scenes
  88 match, 27 fail, 30 absent (67, 52, 27), real programs as at run 23. At 9f52cb91 (the last
  code commit, the rename): lint of both builds clean; front 47, cache 1 and unit 5 passing;
  library com 15 and programs 30 passing, 1 ignored (the known failure); every Direct3D 11 scene
  through the library 135 pass, 0 fail, 4 absent (127, 9, 4 at run 23: the geometry scene by
  the back end's primitive index, views by the stencil fix); Wine's d3d11 file 140 of 165, the
  same 25 failing as at dd123eec (logs/g4s12f-*).

G4 run 23 (2026-11-18, about 16:50 to 18:30 Pacific). Merged origin/phase-2-graphics,
-shaders and -webgl cleanly (-webgpu, -direct3d-12 and -rig had nothing new). The shaders
crate now writes targets of another type than the program's output (`targets_in_shader`) and
reads constant buffers as storage runtime-sized. origin/phase-1-literals exists but is a
phase-one branch still changing its rules (344 commits, no passing milestone stated), and the
task allows merging only the graphics branches, so it was not merged (Departures).
- Layered geometry (plan 4.12, `LayerSplit`, `MultiViewportSplit`): a geometry shader writing
  `SV_RenderTargetArrayIndex` or `SV_ViewportArrayIndex` is no longer refused. The stage helper
  runs once; the final draw runs once per layer of the bound target and depth views (all of one
  layer count, refused by name otherwise or when a view reinterprets its texture), each into
  views of that layer alone (`View::layers`, made at the first such draw), and per bound viewport
  with that viewport and its scissor rectangle; each run's layer and viewport go in the
  pass-through's helpers (`Helpers::of_passthrough`, sized from its module) and in the pixel
  program's `HelperValue::Layer`/`Viewport` (`ContextState::run`; zero for draws without such a
  geometry shader, as Direct3D reads them). A primitive naming a viewport past the bound ones is
  drawn nowhere, as real Direct3D on the RTX 3080 (Wine's test_multiple_viewports). A primitive
  naming a layer past the view's is drawn nowhere too, where real Direct3D draws it into the
  view's first layer and its pixel shader still reads the value it named: the pass-through keeps
  only exact matches (Asked of the shaders track; test_layered_rendering's nine remaining
  failures are exactly those, in both files). New front test
  `a_geometry_shader_choosing_layers_and_viewports_draws_once_per_layer_and_viewport`.
- A vertex or domain shader writing a layer or viewport index is `E_INVALIDARG`, as Direct3D
  answers on a device without `VPAndRTArrayIndexFromAnyShaderFeedingRasterizer` (which ours
  reports absent); it was `E_NOTIMPL`.
- `RSSetViewports` with more viewports than a pipeline has
  (`D3D11_VIEWPORT_AND_SCISSORRECT_OBJECT_COUNT_PER_PIPELINE`) changes nothing, as real
  Direct3D. test_multiple_viewports now passes in both files.
- A pixel output of another kind (float, unsigned, signed) than its target is written as the
  target's type (`StageVariant::retyped`, the shaders crate's `targets_in_shader`), no longer
  refused; a normalised target stays refused by the converter (`NormAsInteger`). Wine's
  d3d10core test_layered_rtv_mismatch passes.
- Constant buffers indexed at run time (the `dcl_constantbuffer` dynamic-indexed flag,
  `shader::indexed_constants`) and bound larger than the shader declares are read as storage
  buffers (`StageVariant::constants_as_storage`, `Helpers::stored`, `ConstantsAsStorage`
  counted), so an index past the declared array reads the bound bytes as in Direct3D; the
  binding set binds them at the variant's binding. Constant buffers are made with storage use
  too. Not on a back end without storage buffers (the WebGL 2 build reads the declared array,
  clamped). test_cb_relative_addressing passes in both files.
- Clip distances where the back end has none (`ClipDistanceInShader`; the WebGPU back end on
  the desktop declares no `CLIP_DISTANCE`): a vertex shader writing them is made with the
  shaders crate's `CLIP_DISTANCE` variant, passing them at the location past its outputs
  (`Shader::clip`, `shader::linked_location`), and each draw's pixel program applies them
  (refused by name without a pixel shader, or when the pixel shader reads past the vertex
  shader's outputs). Single clip distances from vertex shaders now draw as real Direct3D in
  test_clip_distance; the test still fails on the converter's `LinkedClipDistanceCount` (two
  distances) and `ClipDistance` from geometry shaders, and the front's refusal of clip distances
  from a domain shader (Asked).
- Results: see the Wine section (filled at the end of the run).

G4 run 22 (2026-11-18, about 13:45 to 17:00 Pacific). Merged origin/phase-2-graphics,
-webgpu and -shaders cleanly (-webgl, -direct3d-12 and -rig had nothing new beyond them). The
shaders crate now builds `EarlyTestsInShader` and `StorageOffset`, converts geometry shaders
writing a layer or viewport, and converts `SV_PrimitiveID` in pixel shaders natively.
- Early depth and stencil tests before storage writes (a listed case, both halves): a pixel
  shader with `[earlydepthstencil]` and storage writes under a depth or stencil test is drawn
  with the shaders crate's `EARLY_TESTS` variant (`StageVariant::early_tests`) in a pass whose
  depth-stencil attachment is read-only (`ContextState::early`, pending clears of it run apart),
  the attachment's depth and stencil aspects bound at `EARLY_DEPTH_BINDING` and
  `EARLY_STENCIL_BINDING` (`View::stencil_aspect`; zeros for a format without stencil) and the
  tests at `HelperValue::EarlyTests` (`EarlyTestsInShader` counted). Depth-stencil textures are
  made sampleable where the back end samples their format. Where the draw writes depth or
  stencil, each primitive is drawn twice (`draw_early_per_primitive`, `EarlyTestsPerPrimitive`
  counted): with the tests in the program against the read-only attachment, then without a pixel
  shader or targets with the draw's own depth-stencil state; the helpers keep the API draw's
  bases. Refused by name there: strips and patches, instanced and indirect draws, running
  queries, stream output, and early tests behind a geometry or tessellation stage. own_cases's
  `early` case is now two cases, `early` (writes off) and `early_writes` (writes on), each a test
  in programs.rs against real Direct3D.
- Geometry shaders writing a layer or viewport index now convert (shaders crate); the runs per
  layer and viewport are not built, so such shaders are refused by name at creation, as before
  the merge (the shaders track warned a front not drawing the runs fails at the final draw).
- A pixel shader reading `SV_PrimitiveID` without a geometry shader now converts; the front
  refuses it by name where the back end lacks `PRIMITIVE_INDEX` (`PrimitiveIdAsGeometry`, the
  stage helper's route, is not built). The front test that expected the converter's refusal now
  expects this one (the recording back end has no primitive index).
- A colour output of another kind (float, unsigned, signed) than its target is refused by name
  at the draw, where the back end had lost the device (Wine's d3d10core
  test_layered_rtv_mismatch crashed reading the failed map); Direct3D writes the bits as they
  are (Asked).
- Wine's d3d10core failures read and fixed (this front's own): `RSGetViewports` through Direct3D
  10 writes zeros past the bound viewports; Direct3D 10's `CheckMultisampleQualityLevels` and
  `CheckFormatSupport` answer `E_INVALIDARG` for a null out pointer (its table patched as Direct3D
  11's); cube textures below 10_1 have exactly six faces; a texture's misc flags on a buffer are
  `E_INVALIDARG` (Direct3D 10's `GDI_COMPATIBLE` shares `ALLOW_RAW_VIEWS`' bit); Direct3D 10's
  stream output strides follow its rules (one stride for buffer 0, zero for packed buffers, no
  declaration and no stride for a plain geometry shader); Direct3D 10's copies into a
  depth-stencil texture from a colour one copy nothing; `ID3D10Multithread` protection is on for
  Direct3D 10's devices unless made single-threaded, and the `Enter` count and protection flag
  live on the device with the bindings; a Direct3D 10 device made without an adapter has the
  adapter of a factory of DXGI 1.0, whose `QueryInterface` refuses `IDXGIFactory1` and later.
- Direct3D 10's device holds no reference on what it binds: at the program's last release of an
  object, under Direct3D 10's behaviour (`Kept::emulates10`), every slot of the immediate
  context holding it is emptied through the context's own setters (`Child::released`,
  `Context::unbind_released`); rasterizer, blend and depth-stencil states stay bound, as real
  Direct3D 10 keeps a released rasterizer state bound (the blend and depth-stencil states are
  taken to behave alike; not measured). `SOGetTargets` through Direct3D 10 answers all ones for
  each bound target's offset and zero for an empty slot, as real Direct3D 10 does.
- Literal sweep, first part (brief of 2026-11-18): see "Literals" below.
- Results at 3c7b577d's tree (RTX 3080 through the queue's hazard group, not alone; see the
  Wine section): Wine's d3d11 file 136 of 165 (133 at run 21); d3d10core 99 of 111 (90);
  scenes through the whole library 123 pass, 14 fail, 4 absent, the same scenes as at run 21
  (logs/g4s10j-scenes.log); the WebGL 2 build's scenes 67 match, 52 fail, 27 absent, as at run
  21 (g4s10j-webgl.log); real programs as at run 21: eleven of Microsoft's and the engines' and
  four of bgfx's match on every compared frame, bgfx-24-nbody runs to its last frame,
  atg-simplecomputepc, atg-simplemsaa_pc and bgfx-26-occlusion differ by the same pixel counts
  as at run 21 (g4s10j-programs.log, g4s10j-bgfx.log).
- At the end: lint of both builds clean with warnings denied, front 46 + cache 1 + unit 5,
  library com 15 and programs 30 with 1 ignored (the empty slot's resinfo), all passing.
- At the end the desktop's C:\wc\target\phase-2-graphics-direct3d-11 (15.8 GB, debug builds of
  both back ends) and the checkout's own target (600 MB, from `cargo check`) were deleted.
- The shaders crate's work moved to this front: with the converter now serving them, layered
  geometry runs, stream output after a domain shader and clip distances in the program are this
  front's to build (gap list part B, What comes next).

G4 run 21 (2026-11-18, about 09:50 to 12:30 Pacific). Merged origin/phase-2-graphics (which
held -shaders, -direct3d-12 and -webgl's new work) cleanly; -webgpu and -rig had nothing new.
The merged tree linted with warnings denied and passed the front tests on the desktop before any
change (logs/g4s9a-*). The shaders crate still lists `StorageOffset`, `EarlyTestsInShader` and
`AntialiasedLinesAsQuads` as unbuilt, so nothing waiting on them moved.
- Direct3D 10's device on the device (`ID3D10Device1`, `ID3D10Device`, and
  `ID3D10Multithread`; src/com/device10.rs, generate.py): every call of Direct3D 10's device is
  served (but counters and `OpenSharedResource`, unserved as on Direct3D 11's): creation through
  Direct3D 11's calls with Direct3D 10's descriptions (misc flags, depth view, blend and stream
  output declarations converted; the rest share Direct3D 11's layout), the context calls on the
  immediate context. Shaders, input layouts, queries and predicates now offer their Direct3D 10
  interfaces; Direct3D 10's `Map`/`Unmap` on buffers and textures and `Begin`/`End`/`GetData`
  on queries run through the immediate context (pipeline statistics in Direct3D 10's 64 bytes).
  A child's Direct3D 10 `GetDevice` gives the device's Direct3D 10 interface where offered.
- The behaviour real Direct3D shows, measured with a new example `own_d3d10`
  (tests/programs.rs `direct3d_10s_device_draws_and_keeps_its_state_as_direct3d_does`; identical
  to real Direct3D line by line, logs/g4s9c-d3d10.log): `QueryInterface` answers
  `ID3D10Device`/`ID3D10Device1` only once a context state was made (any emulated interface; all
  five of the device's tables are gated); while the active state is of the other API, a call that
  changes state or does work does nothing (`E_INVALIDARG` where it answers), and a getter reads the
  default state (objects none, counts zero, blend factor ones, mask all ones), on either side;
  Direct3D 10's creation calls work whatever is active; `GetFeatureLevel` answers the device's
  level (11_0 too). The example draws a triangle through Direct3D 10's calls only (immutable
  vertex buffer, constant buffer by `UpdateSubresource`, a Direct3D 10 blend state with a write
  mask, an occlusion query, a staging copy mapped through `ID3D10Texture2D::Map`): identical
  pixels and counts. In that mode `ID3D11DeviceContext::PSGetSamplers` writes nothing at all
  (the other getters write the default state), as Wine's test_device_context_state shows real
  Direct3D doing; that test now passes (287 checks).
- Direct3D 10's entry points (src/com/d3d10core.rs): `D3D10CreateDevice` (d3d10.dll's, 10_0),
  `D3D10CreateDevice1` (d3d10_1.dll's) and `D3D10CoreCreateDevice` (d3d10core.dll's, also
  asked by the system's Direct3D 10 runtime for the program) make our device with a state
  emulating Direct3D 10 swapped in from the start; the hardware driver only, no debug layer.
  `D3D10CoreRegisterLayers` answers `E_NOTIMPL`, `D3D10CoreGetVersion` the system's own. A
  forwarding d3d10core.dll is built; no d3d10.dll or d3d10_1.dll forwarder (Departures).
- The immediate context's bindings and state object now live on the device
  (`context::Kept`), not in the COM context object: a Direct3D 10 program never holds the
  context, so each of its calls made a fresh one, and Direct3D's immediate context keeps its
  state for the device's life anyway (a Direct3D 11 program releasing its context and asking
  again now gets its bindings back, as from real Direct3D).
- Wine's d3d10core file through ours (our library copied in as d3d10.dll; the test makes its
  devices with `D3D10CreateDevice`): 90 of 111 pass (logs/g4s9j-wine10.log). Before the
  bindings moved to the device, 19 (every map failed: the context holding the Direct3D 10
  state had gone).
- Results (see the Wine section and Measurements): Wine's d3d11 file 133 of 165 (132 at run
  20; test_device_context_state new, none lost), at 944ff499 too; d3d10core 90 of 111 (not run
  before); scenes through the whole library 123 pass, 14 fail, 4 absent (as at run 20); the WebGL
  2 build's scenes 67 match, 52 fail, 27 absent (63, 61, 23 at run 20; from the WebGL 2 back
  end's merged work, nothing of this front's); real programs as at run 20 (eleven of
  Microsoft's and the engines' plus four of bgfx's matching; atg-simplecomputepc,
  atg-simplemsaa_pc and bgfx-26-occlusion differing as before).
- At the end: lint of both builds clean with warnings denied; front 46 + cache 1 + unit 5,
  library com 15 and programs 27 with 2 ignored (the known failures), all passing.
- At the end the desktop's C:\wc\target\phase-2-graphics-direct3d-11 (15.4 GB, debug builds of
  both back ends) was deleted; no build folder was made on the Mac. Merged origin/phase-2-graphics,
-shaders, -direct3d-12 and -webgl cleanly (-webgpu and -rig had nothing new); the merged tree
linted with warnings denied and passed the front tests on the desktop before any change
(logs/g4s8a-*).
- Per-target blends without the back end's (`TargetsSplit`, plan 4.12; a listed case): a blend
  state the back end cannot take whole is made as one block per group of targets that agree
  (`Blend::groups`); a draw then runs once per group holding a bound target that writes
  something, each run in a pass of only its group's targets, depth and stencil copied aside
  before the first run and back before each later one where the draw writes them
  (src/context/split.rs). Where every bound target is in one group the draw takes that group's
  block and the pass as it is. Refused by name: such draws while queries run, with pixel
  storage views or with stream output (no back end lacking per-target blends has the last two).
  New example `own_level10` (feature level 10_0, as the WebGL 2 build reports), its `targets`
  case in tests/programs.rs: identical to real Direct3D on both builds, the WebGL 2 one run
  without OES_draw_buffers_indexed (`WEBGL_OFF`), the split taken (logs/g4s8g-split.log). New
  front test `targets_blended_differently_without_the_back_ends_draw_once_per_group_from_saved_depth`.
- Found by that case: below 10_1 real Direct3D refuses (`E_INVALIDARG`) blend factors or
  operations that differ between targets that blend, and takes differing enables and write
  masks (Direct3D 10's blend). Ours had taken both; it now answers as real Direct3D does.
- `resinfo` on WebGL 2 (a listed case): `own_level10 resinfo`, a pixel shader asking a texture's
  and an array's views (every level and layer: the WebGL 2 back end has no `TEXTURE_VIEWS`, so
  views of some levels are refused by name there) for sizes, layers and level counts at three
  levels: identical to real Direct3D on both builds. `resinfo_empty` adds an empty slot, where
  real Direct3D answers 0 by 0 with 0 levels: identical on the WebGL 2 build (sizes from the
  helpers), but on WebGPU the empty slot's stand-in answers 1 by 1, so that test is ignored as a
  known failure (Known failures).
- test_sample_attached_rtv's panic: the test releases its sampler once more than it holds, while
  the context still binds it. A release when the program holds no reference now changes nothing
  and answers 0xffffffff, as real Direct3D answers (own_interfaces prints both releases' answers;
  identical to real Direct3D). Before, the count went below zero and the library aborted.
- bgfx-26-occlusion, read: the frames differ in two places. Its overlay panel is one line
  shorter, as the GPU memory line is missing where NVAPI is kept out (Departures, run 16; the
  rig ignores the line but not the panel's last 7 rows), and its "Passing pixels count" reads 49
  where real Direct3D reads 50 (a counted occlusion query one sample short; not read further).
- From the merges: atg-simplebezierpc now matches real Direct3D on every compared frame (the
  shaders crate's tessellator compiles under FXC; its 0xc0000409 of run 19 is gone), and so does
  the rig's tessellation scene. That makes eleven of Microsoft's and the engines' programs and
  four of bgfx's matching.
- Results (see the Wine section and Measurements): Wine's d3d11 file 132 of 165 (128 at run
  19); scenes through the whole library 123 pass, 14 fail, 4 absent (122, 16, 4); the WebGL 2
  build's scenes 63 match, 61 fail, 23 absent (45, 80, 23); real programs as above.
- At the end: lint of both builds clean with warnings denied, front 46 + cache 1 + unit 5,
  library com 15 and programs 26 passing with 2 ignored (logs/g4s8d-lib.log, g4s8h-*).
- At the end the desktop's C:\wc\target\phase-2-graphics-direct3d-11 (16.3 GB, debug builds of
  both back ends), scratch wincheck/target (990 MB, the Mac's check-only Windows build) and the
  checkout's own target (555 MB, from `cargo check`) were deleted.

G4 run 19 (2026-11-18, about 06:30 to 08:00 Pacific). Merged origin/phase-2-graphics-shaders and
-direct3d-12 (one conflict, the slow path list in crates/wcontainers-gpu/src/slow.rs: both sides
kept); the others had nothing new.
- A display driver reset on the owner's PC, caused by this track's job: Wine's
  test_atomic_instructions, which since the merge converts (the shaders crate's
  `AtomicExchangeLoop`, a compare-exchange loop for a signed minimum or maximum on unsigned
  memory) and ran on the GPU for the first time; the driver stopped responding and recovered at
  07:03:58 desktop time. Cause recorded with `tools/graphics-rig/resets record`; the test stays
  quarantined by the rig. The front now refuses any program taking `AtomicExchangeLoop` at
  creation (`E_NOTIMPL`, named) until the loop is shown to end (Asked of the shaders track).
- Stencil views (`X24_TYPELESS_G8_UINT`, `X32_TYPELESS_G8X24_UINT`) are made, and programs
  reading one get the shaders crate's `VariantKey::swizzled` with the stencil moved to the
  second component (`StageVariant::stencil`, `SwizzleInShader` counted). On the desktop the
  stencil still reads 0: wgpu's Direct3D 12 path appears to give a stencil view's value in the
  second component, not the first as WebGPU defines (test_depth_stencil_sampling reads 0 for
  every stencil value; the views scene draws now and differs). Asked of the WebGPU track.
- Multisampled arrays in programs (`Texture2DMSArray`, the shaders crate's `texture_layers`):
  every program declaring one is converted with each such slot one layer (read from the
  declarations, `shader::multisampled_arrays`), and multisampled views of multisampled textures
  are 2D. A multisampled texture of more than one layer is refused at creation by name (WebGPU's
  have one layer; a texture per layer, plan 4.2, is not built). bgfx's examples now run:
  bgfx-01-cubes, -35-dynamic, -37-gpudrivenrendering and -38-bloom match real Direct3D on every
  compared frame, -24-nbody runs to its last frame (no frame compared: real Direct3D differs from
  run to run), -26-occlusion differs (about 6,500 pixels a frame; not read). That is a fifth
  engine family matching.
- `Reflection::slow_paths` (the programs' own slow paths) are counted at each draw or dispatch
  through the program.
- `resinfo` and `sampleinfo` where the back end cannot query levels (WebGL 2): every variant asks
  for `TEXTURE_INFO` there, and the front writes `TextureLevels` (the view's level count) and
  `TextureSamples` from the bound views (`TextureInfoInHelpers` counted; zero for an empty slot).
  The rig's samplers scene now matches real Direct3D on the WebGL 2 build (its four outputs);
  the WebGL 2 build's scenes: 45 match, 80 fail, 23 absent (43, 84, 21 at run 18;
  logs/g4s7e-webgl.log).
- YUY2 (a listed case): an `rgba8unorm` texture of half the width whose texels are pixel pairs;
  views `R8G8B8A8_UNORM` and `_UINT` (shader views) and also `_SNORM` (target views, as real
  Direct3D takes); boxes and places in pixels, dropped at an odd pixel; `CheckFormatSupport`
  reports no target use, as real Direct3D does. New example `own_yuy2` (tests/programs.rs
  `yuy2_pixel_pairs_view_write_copy_and_map_as_direct3d_does`): identical to real Direct3D line
  by line. New front test `yuy2_is_a_half_width_texture_of_pixel_pairs_viewed_as_rgba8`.
- Context states: the immediate context holds its state object by a library reference (Wine's
  refcount checks after `SwapDeviceContextState`), and `RSGetViewports`/`RSGetScissorRects`
  answer the bound count. test_device_context_state still stops at `ID3D10Device` (What comes
  next 0).
- Found, not read yet: test_sample_attached_rtv now gets past the stencil view and panics in the
  library (a program reference count going below zero, counted.rs:67) after binding a stencil
  view while a depth view of its texture is bound; atg-simplebezierpc now exits 0xc0000409 (was
  0xe06d7363), its output not read.
- Results: see the Wine section and Measurements. At the end of the run: lint of both builds,
  front 45 + cache 1 + unit 5, library com 15 and programs 24 with 1 ignored, all passing.
- At the end the desktop's C:\wc\target\phase-2-graphics-direct3d-11 (15.5 GB, debug builds of
  both back ends) and the checkout's own target (594 MB, from `cargo check`) were deleted.

G4 run 18 (2026-11-18, about 04:55 to 07:00 Pacific). Merged origin/phase-2-graphics-shaders and
-direct3d-12 cleanly (the others had nothing new); the merged tree linted clean on the desktop.
- NV12 (plan 4.12; Departures): the texture is its luma plane, with the half-size chroma plane
  as a texture of its own (`Texture::chroma`); views name a plane by their format (`R8_*` the
  luma, `R8G8_*` the chroma; the default and NV12's own format are invalid); initial data,
  `UpdateSubresource`, `CopySubresourceRegion`, `CopyResource`, staging maps and dynamic maps
  join the planes as Direct3D lays them out (chroma rows after luma rows at one pitch, the map's
  slice pitch covering both); boxes and places at an odd texel are dropped, as Direct3D drops
  them; odd sizes are invalid. New example `own_nv12` (tests/programs.rs
  `nv12_planes_view_write_copy_and_map_as_direct3d_does`): identical to real Direct3D line by
  line. New front test `nv12_is_two_planes_viewed_apart_and_written_copied_and_mapped_as_joined_rows`.
- `own_cases` (new example; shaders compiled by d3dcompiler_47.dll, Departures), one listed
  case per argument, each a test in programs.rs against real Direct3D: texture atomics (add,
  min, max, or, xor on `R32_UINT`/`R32_SINT`), typed storage stores (`R11G11B10_FLOAT`,
  `R16G16_UNORM`, `R16G16B16A16_FLOAT`), a pixel shader's storage view read by the next draw on
  the same targets, an `R32_FLOAT` depth and a colour texture through one linear sampler
  (`FilterInShader`; compared within one unit), a copy between multisampled textures (each
  sample loaded), and stream output also drawn: all identical to real Direct3D. A depth buffer
  larger than its target and `[earlydepthstencil]` with storage writes are known failures
  (Known failures).
- Three wrong answers found by those cases and fixed: `OMSetRenderTargets` now unbinds the pixel
  stage's unordered access views, as `OMSetRenderTargetsAndUnorderedAccessViews` binding none
  (real Direct3D read the storage view back through a shader view after it; ours had kept it
  bound and read zeros); a draw reading as a shader resource what an earlier draw of the open
  pass wrote as storage (or the reverse) ends the pass first (`UsageScopeSplit`; WebGPU had
  removed the device); and a pixel shader forcing early depth-stencil tests with storage writes
  is refused at the draw by name, where it had drawn with late tests (a wrong count).
- A depth buffer larger than its colour targets (Direct3D allows it; WebGPU had removed the
  device): such a pass is begun through the pass tracker, which fits the targets to the depth
  buffer (`AttachmentsFitted`, plan 4.7), and its draws use a twin of the raster block with the
  scissor test on, cut to the area every attachment covers. own_cases deep now matches real
  Direct3D (the depth outside the target untouched). Occlusion queries over such a pass are
  refused by name; geometry and tessellation draws do not cut their scissors yet.
- A staging texture the CPU reads, of a format the back end cannot read back, is refused at
  creation (`E_NOTIMPL`), where the read back had removed the device (the WebGL 2 build's
  targets and mips scenes).
- At the end: lint of both builds clean, front 44 + cache 1 + unit 3, library com 15 and
  programs 23 passing with 1 ignored (the early tests case), at 53a45b6 (logs/g4s6i-*).
- At the end the desktop's C:\wc\target\phase-2-graphics-direct3d-11 (17.9 GB, the debug
  builds on both back ends) was deleted; no build folder was made on the Mac.

G4 run 17 (2026-11-18, about 03:45 to 05:30 Pacific). Merged origin/phase-2-graphics-webgpu,
-webgl, -direct3d-12, -shaders and -rig cleanly; the merged tree linted with warnings denied and
passed the front tests on the desktop before any change.
- Godot's 3D platformer (through ANGLE) now matches real Direct3D: frames 10, 60 and 120,
  largest difference 0. Its first pipeline failed because an `R16G16B16A16_UINT` element was
  read by an `int4` input; such small integers read with the other signedness are now fetched
  in the program (Departures). That makes ten real programs matching, of four engine families.
- Building ahead compared a pixel shader's outputs with its targets' kinds through a filter
  that took no output (colour outputs carry `SystemValue::Target`), so it built pipelines
  WebGPU refused; it now compares the exact kind (float, unsigned, signed). test_uav_load passes.
- Hull and domain shaders (src/context/tessellation.rs, the library's `HullShader`,
  `DomainShader` and the HS/DS calls): made as shaders of their stage, with their own slots
  (stages `HULL` and `DOMAIN`), drawn through the shaders crate's `Stages::tessellated` (vertex
  stage and hull stage as compute, the tessellator, then the domain program's indexed indirect
  draw with the pixel shader). Without both stages or from a non-patch topology Direct3D draws
  nothing, and so does this front. Refused by name: a geometry shader or stream output after
  tessellation, counted or statistics queries during such draws, indirect draws, vertex shader
  variants, helper constants in the hull, domain or pixel stage. New front test
  `hull_and_domain_shaders_run_as_compute_before_the_domain_programs_draw`. On the desktop the
  tessellator program fails FXC (wgpu's Direct3D 12 compiler; shaders track, Asked), so the
  rig's tessellation scene stops at its first patch: drawn, not yet compared.
- `IDXGISurface1::GetDC` and `ReleaseDC` on GDI-compatible textures (Departures), and
  `MISC_GDI_COMPATIBLE` accepted where Direct3D accepts it: test_getdc passes.
- Render target views of buffers (Departures): test_create_rendertarget_view passes. New front
  test `a_target_view_of_a_buffer_draws_into_a_row_of_its_elements_copied_into_the_buffer`.
- 3D textures of one level and one slice answer `IDXGISurface`, as real Direct3D's do (Wine's
  test_dxgi_resources ran 78 checks fewer than real Direct3D for those six resources).
- From the merges, passing with no change here: test_clear_state, test_vertex_id,
  test_stream_output_vs (vertex shaders without a position), test_format_compatibility (the
  WebGPU back end's R16 and D16 copies), test_tgsm (structured group-shared memory).
- atg-simplecomputepc differs only inside the Mandelbrot set (Asked: the division's precision).
- Results: see the Wine section and Measurements.
- At the end the desktop's C:\wc\target\phase-2-graphics-direct3d-11 (14.1 GB, debug builds of
  both back ends), the checkout's own target (590 MB, from `cargo check`) and scratch
  wincheck/target (1.0 GB, the Mac's check-only Windows build) were deleted; a scratch git
  worktree used for one change was removed.

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
- At the end the desktop's C:\wc\target\phase-2-graphics-direct3d-11 (17.8 GB, both builds,
  debug and release) and the checkout's own target (698 MB, from `cargo check`) were deleted.
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

Runs before 15 (G4a's six runs, G4 runs 1 to 14, 2026-11-17 and 18) are in this file's git
history (git log -p docs/status-graphics-direct3d-11.md, before runs 15 to 17): what each
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
shaders and stream output (through the shaders crate's stage helper); since run 15 the cache;
since run 17 hull and domain shaders, `GetDC` and target views of buffers; since run 18 NV12
and depth buffers larger than their targets.

## Wine's Direct3D 11 tests through ours

Files passing (the bar: real Direct3D's verdict for every test, except the gap list):
- d3d11: not passing. G4 run 24 (2026-11-18, RTX 3080 through the queue's hazard group, not
  alone, rigfix/wine.sh, at dd123eec's tree): 140 pass of the 165 run (25 fail;
  logs/g4s12e-wine.log, 168 s of tests). New: test_quad_tessellation (stages after a domain
  shader), test_depth_stencil_sampling (stencil views bound); none lost.
- d3d11: not passing. G4 run 23 (2026-11-18, RTX 3080 through the queue's hazard group, not
  alone, rigfix/wine.sh, at 728773d0's tree): 138 pass of the 165 run (27 fail;
  logs/g4s11f-wine.log). New: test_cb_relative_addressing (constant buffers read as storage),
  test_multiple_viewports (layered runs and the viewport limit); none lost.
- d3d11: not passing. G4 run 22 (2026-11-18, RTX 3080 through the queue's hazard group, not
  alone, rigfix/wine.sh, at 3c7b577d's tree): 136 pass of the 165 run (29 fail;
  logs/g4s10j-wine.log). New: test_early_depth_stencil (this run's early tests),
  test_vs_input_relative_addressing (the shaders crate), test_create_texture2d (its
  `R32G32B32_*` check, from the merged back end); none lost.
- d3d11: not passing. G4 run 21 (2026-11-18, RTX 3080 through the queue's hazard group, not
  alone, rigfix/wine.sh, at d2892718's tree): 133 pass of the 165 run (32 fail;
  logs/g4s9f-wine.log). New: test_device_context_state (287 checks, as real Direct3D); none
  lost (the 32 are run 20's 33 but it).
- d3d11 at G4 run 20 (2026-11-18, RTX 3080 through the queue's hazard group, not
  alone, rigfix/wine.sh, at fbcd643's tree): 132 pass of the 165 run (33 fail;
  logs/g4s8h-wine.log, gaps in logs/g4s8h-wine-gaps.txt). New: test_sample_attached_rtv (the
  over-release), test_gather and test_gather_c (the shaders crate's run-time offsets),
  test_stream_output_components (geometry shaders writing no position); none lost.
- d3d11 at G4 run 19 (2026-11-18, RTX 3080 through the queue's hazard group, not
  alone, rigfix/wine.sh, at 01a6bfe's tree): 128 pass of the 165 run (37 fail; logs/g4s7d-wine.log);
  test_atomic_instructions is not run (quarantined after the reset above). The 37 are run 18's
  list but for it; test_create_shader_resource_view, broken mid-run by the multisampled array
  views, passes again.
- d3d11 at G4 run 18 (2026-11-18, our library copied in as d3d11.dll, RTX 3080
  through the queue's hazard group, not alone, through scratch rigfix/wine.sh): full file at
  f5eed62's tree: 128 of the 166 the rig counts (38 fail; logs/g4s6g-wine.log; none lost). New
  since run 17: test_nv12.
- Earlier runs of the file (G4 runs 1 to 17: 11, 56, 73, 68, 72, 74, 78, 79, 82, 88, 90, 104,
  113, 114, 118 and 127 passing) and their gap lists are in this file's git history.
- d3d10core: not passing. G4 run 24 (RTX 3080 through the queue's hazard group, not alone, at
  dd123eec's tree): 103 pass of 111 (8 fail; logs/g4s12e-wine10.log, 97 s of tests). New:
  test_depth_stencil_sampling; none lost. The 8: three absent (logic ops and WARP:
  test_format_support, test_swapchain_formats, test_resource_access), three the shaders track's
  (test_layered_rendering's layers past the view, test_clip_distance's geometry shader distances,
  test_combined_clip_and_cull_distances' cull distances), two the infrastructure crate's
  (test_swapchain_flip, test_desktop_window).
- d3d10core: not passing. G4 run 23 (RTX 3080 through the queue's hazard group, not alone, at
  728773d0's tree): 102 pass of 111 (9 fail; logs/g4s11f-wine10.log). New: test_layered_rtv_mismatch,
  test_cb_relative_addressing, test_multiple_viewports; none lost. The 9: three absent (logic ops
  and WARP: test_format_support, test_swapchain_formats, test_resource_access), the shaders
  track's (test_layered_rendering's layers past the view, test_clip_distance's two distances and
  geometry shader distances, test_combined_clip_and_cull_distances), the WebGPU back end's
  (test_depth_stencil_sampling), the infrastructure crate's (test_swapchain_flip,
  test_desktop_window).
- d3d10core: not passing. G4 run 22's last run (RTX 3080 through the queue's hazard group, not
  alone, at 3c7b577d's tree): 99 pass of 111 (12 fail; logs/g4s10j-wine10.log): the run below's
  14 but test_clear_state and test_state_refcounting, which pass. Each of the 12 is named below:
  three absent (logic ops and WARP: test_format_support, test_swapchain_formats,
  test_resource_access), two the shaders track's (test_combined_clip_and_cull_distances,
  test_cb_relative_addressing), four this front's (test_layered_rendering,
  test_multiple_viewports: layered runs; test_clip_distance: clip distances in the program;
  test_layered_rtv_mismatch: refused by name, Asked of the shaders track), one the WebGPU back
  end's (test_depth_stencil_sampling), two the infrastructure crate's (test_swapchain_flip,
  test_desktop_window).
- d3d10core, G4 run 22's first run (2026-11-18, RTX 3080 through the queue's hazard group,
  not alone; rigfix/wine.sh with `--dll 'd3d10.dll=%CARGO_TARGET_DIR%\debug\wcontainers_directx.dll'`,
  at 54909117's tree): 97 pass of 111 (14 fail; logs/g4s10h-wine10.log; 90 at run 21). New
  passes: test_device_interfaces, test_create_texture2d, test_create_buffer,
  test_check_multisample_quality_levels, test_stream_output, test_format_compatibility,
  test_vs_input_relative_addressing (the last from the shaders crate). The 14, by their first
  named failure:
  - Absent here as in the d3d11 file: logic ops and WARP (test_format_support's
    `CheckFormatSupport` answers, test_swapchain_formats' typeless format, test_resource_access
    runs 294 checks of 318).
  - The shaders track's (as in the d3d11 file): test_clip_distance,
    test_combined_clip_and_cull_distances, test_cb_relative_addressing.
  - This front's, named: test_layered_rendering and test_multiple_viewports (layered geometry
    runs, What comes next 0), test_layered_rtv_mismatch (a float output into an unsigned target,
    refused by name, which removes the device; the test then crashes reading its failed map;
    Asked of the shaders track), test_clear_state and test_state_refcounting (fixed after this
    run, see below).
  - The WebGPU back end's stencil views (test_depth_stencil_sampling); the infrastructure crate's
    (test_swapchain_flip, test_desktop_window).
  Found by that run and fixed after it: Direct3D 10's `SOGetTargets` answers all ones for the
  offset of every bound target, whatever it was bound at, and zero for an empty slot
  (test_clear_state); a released rasterizer state stays bound (test_state_refcounting reads it
  back; the unbinding now leaves rasterizer, blend and depth-stencil states, and our library had
  crashed the test releasing the none it got).
- dxgi: the rig has no
  baseline of real Direct3D's verdicts for it.

The gap list, G4 run 24 (the 25 failing tests of the full file at dd123eec: run 23's 27 but
test_quad_tessellation and test_depth_stencil_sampling, which pass; test_atomic_instructions is
quarantined and not run, and is kept in part B; the first named refusal, skip or failed check of
each, logs/g4s12e-wine.log and the per-test logs, read against Wine's d3d11.c in scratch
winesrc/). A test is in part A only where the first thing it fails on is a feature absent on this
stack; everything else is work, in part B, with whose it is. Nothing is excluded for
convenience: each of the 25 failing tests, and the quarantined one, is in exactly one place.
A. Absent on this stack (gaps), 18 tests (as at run 22):
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
B. Work, named (not gaps), 7 tests and the quarantined one:
- This front's: none left. test_quad_tessellation (stream output and geometry shaders after a
  domain shader) passes since run 24 (54 checks, as real Direct3D; logs/g4s12b-some11.log).
- Shaders track: a primitive naming a layer past the view's (test_layered_rendering: drawn into
  the view's first layer with the value it named, nine checks; Asked); `LinkedClipDistanceCount`
  and `ClipDistance` from a geometry shader (test_clip_distance: its vertex and, since run 24,
  domain shader clip distances draw as real Direct3D; the first failure is now the geometry
  shader's creation at line 30396, and the test then crashes on the shader it did not get);
  test_atomic_instructions (quarantined after it reset the display driver; the
  front still refuses `AtomicExchangeLoop` programs; Asked), `GeometryStreams`
  (test_so_statistics_query), `CullDistance` (test_combined_clip_and_cull_distances).
- Interface track: a vertex format reading `B8G8R8X8_UNORM` with its fourth component as 1.0
  (the specification's 19.1.3.3; WebGPU's `unorm8x4-bgra` reads the byte as alpha):
  test_vertex_formats runs 98 checks where real Direct3D runs 100, since `CheckFormatSupport`
  claims no vertex use of it (Asked, run 24).
- Infrastructure crate: every buffer of a sequential swap chain (test_swapchain_flip, line
  16252); a swap chain on the desktop window (test_desktop_window: wgpu's surface refuses it at
  `Surface::configure`, the device is lost at line 32666).

## Known failures

- `an_empty_slots_size_and_level_count_at_10_0_as_direct3d_does` (programs.rs, `own_level10
  resinfo_empty`, ignored with its reason): `GetDimensions` of an empty slot. Real Direct3D
  answers 0 by 0 with 0 levels; on WebGPU ours answers the stand-in's 1 by 1 (the level count is
  right). It passes on the WebGL 2 build. A fix needs the program to give 0 sizes for an empty
  slot (the front could write it to the helpers as it writes the level count there; not built).

## What comes next

After G4 run 24. Done in run 24: stream output and geometry shaders after a domain shader,
a domain shader's clip distances in the program, stencil views bound. No failing Wine test of
the d3d11 file is this front's own work any more (gap list part B names whose each is). At the
head of the list now:
- Merge the other tracks as they land and rerun both Wine files: out-of-range layers in
  layered draws, `LinkedClipDistanceCount`, `ClipDistance` from a geometry shader,
  `CullDistance`, `GeometryStreams`, `AtomicExchangeLoop` (shaders track); the BGRX vertex
  format (interface track); sequential swap chains and the desktop window (infrastructure).
- The primitive index in pixel shaders where the back end lacks `PRIMITIVE_INDEX` (WebGL 2):
  refused by name; `PrimitiveIdAsGeometry` (the stage helper's route) is not built.
- Adopt the literal check once main has it (docs/literals.md), then work the fix list down.
- Direct3D 10 programs: d3d10.dll and d3d10_1.dll forwarders need the effects framework and
  shader reflection they also export (not ours), so only d3d10core.dll is forwarded.
- bgfx-26-occlusion: the counted occlusion query one sample short (49 for 50); then whether the
  rig should also ignore the overlay panel's last rows where NVAPI is kept out (rig track).
- An empty slot's `resinfo` sizes on WebGPU (Known failures).
- Multisampled textures of more than one layer as a texture per layer (refused by name now).
Then, from run 18 (items 1 and 4's YUY2 are done; item 2's bgfx and bezier are done):
0. Wine's d3d11 file: the shaders track's and the back ends' items as they land (merge, rerun
   the file). Wine's d3d10core file needs the entry point above.
2. Real programs: bgfx waits on `MultisampledArrayTexture` (shaders track); atg-simplebezierpc
   and the rig's tessellation scene on the tessellator compiling under FXC (shaders track);
   atg-simplecomputepc on the division's precision (Asked); atg-simplemsaa_pc frame 10 is 4
   pixels in 2 tiles past the rig's allowance (edges; not read further).
3. Building ahead: record the draws through a geometry or tessellation stage and the stage
   helper's dispatches too (Godot's remaining first-draw builds), and find the one pipeline per
   program still built at its first draw on a warm start (it is never recorded: a pipeline the
   front does not draw through its own draw path, perhaps the present's; not read yet).
4. Listed cases still without a passing test against real Direct3D: alpha-antialiased lines
   (shaders track's `LINE_COVERAGE`, Asked), `[earlydepthstencil]` with storage writes
   (`EARLY_TESTS`, Asked; own_cases early). Both are still unbuilt in the shaders crate at run
   21's merge.
5. Move the context fully onto the pass tracker (it begins only fitted passes since run 18; the
   usage-scope split is the front's own), so fitting, splits and occlusion share one route;
   geometry and tessellation draws do not cut their scissors to a fitted area yet, and
   occlusion queries over a fitted pass are refused.
6. Wireframe without the back end's polygon mode (the raster scene on WebGPU): no slow path is
   planned; read section 4 first (culling must still apply to the triangles).
7. The WebGL 2 cases: see Measurements for run 18's scenes through the WebGL 2 build.
8. Compute shaders sampling a clamped texture or through a biased sampler (refused by name);
   track which copy of a twin is current instead of copying at every draw (measure first).
9. `TargetsSplit` keeps queries, pixel storage views and stream output out by name (Departures,
   run 20); keep them to the first run if a back end lacking per-target blends ever has them.

## How to run

- Wine's tests (G4 run 13, until the rig's runner is fixed): scratch `rigfix/wine.sh --dll
  'd3d11.dll=%CARGO_TARGET_DIR%\debug\wcontainers_directx.dll' d3d11[:test_a/test_b]` copies the
  rig's wine-run.ps1, fixed in one line, to C:\wc\out\phase-2-graphics-direct3d-11\rigfix and
  runs it as `wine-tests run` does (logs to the same wine-logs directory). `gaps.sh <label>` lists
  the first named gap of each failing test of logs/<label>-wine.log. `run-all.sh <label>` runs the
  library tests, every scene through the whole library, Wine's d3d11 file and the WebGL 2 scenes.
  Real programs' logs are under graphics-rig/programs/phase-2-graphics-direct3d-11/<program>.

Everything of run 22 in one command: scratch `run-g4s10.sh <label> [steps]` (steps `lint front
lib wine wine10 scenes webgl programs`, default all; `wine10` is Wine's d3d10core file through
the library as d3d10.dll; `some11=test_a/test_b` and `some10=...` run some tests of either
file). Never run `tools/graphics-rig/gpu-job --help`: it takes `--help` as the command, sends
the tree and queues a job (removed by hand with the queue's `pueue remove` in run 22).

Everything of run 21 in one command: scratch `run-g4s9.sh <label> [steps]` (steps `lib wine
scenes webgl programs`; it builds the library before Wine's file, then runs run-g4s8.sh's
steps). Wine's d3d10core file: `rigfix/wine.sh --dll
'd3d10.dll=%CARGO_TARGET_DIR%\debug\wcontainers_directx.dll' d3d10core` after a `cargo build`
of the library (its devices come from `D3D10CreateDevice`, so the library goes in as d3d10.dll).
Never run tools/graphics-rig/queue-check while a job of this checkout waits: it sends the tree,
and the waiting job then stops ("holds tree ..., not the sent ...").

Everything of run 20 in one command: scratch `run-g4s8.sh <label> [steps]` (steps `lib
webgllib scenes wine webgl programs`, default all; `webgllib` runs the `own_level10` tests on
the WebGL 2 build without OES_draw_buffers_indexed); lint and front tests are resume.sh's
`lint front`.

Everything of run 19 in one command: scratch `run-g4s7.sh <label>` (resume.sh's lint, front and
lib, then the scenes through the whole library, Wine's d3d11 file through rigfix, the WebGL 2
scenes, the real programs and bgfx's examples with NVAPI kept out), logs in logs/<label>-*.log.

The scratch script `resume.sh <label> [steps]` runs
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

At G4 run 24: front.rs 47 (run 23's layered test added one; the hull and domain test now also
streams the domain stage's output), tests/cache.rs 1, unit 5, com.rs 15 (the shader refused at
creation is now a vertex shader writing `SV_CullDistance`), programs.rs 31 (new: `own_cases
stencil` against real Direct3D, passing; resinfo_empty ignored as a known failure).

At G4 run 22: front.rs 46 (the primitive index refusal's expected name changed), tests/cache.rs
1, unit 5, com.rs 15, programs.rs 30 (`own_cases early` split into `early` and
`early_writes`, both passing against real Direct3D; resinfo_empty ignored as a known failure);
all others passing on the desktop at 3c7b577d's code, lint of both builds clean with warnings
denied (logs/g4s10i-lint.log, g4s10i-front.log, g4s10i-lib.log).

At G4 run 21: front.rs 46, tests/cache.rs 1, unit 5 (unchanged), com.rs 15, programs.rs 29
(new: `own_d3d10`, `direct3d_10s_device_draws_and_keeps_its_state_as_direct3d_does`; the
two known failures ignored); all others passing on the desktop (logs/g4s9f-*, g4s9k-lib.log),
lint of both builds clean with warnings denied (logs/g4s9g-lint.log).

At G4 run 20: front.rs 46 (new: the split test), tests/cache.rs 1, unit 5, com.rs 15,
programs.rs 28 (new: `own_level10` targets, resinfo and resinfo_empty; resinfo_empty and the
early tests case ignored as known failures; `own_interfaces` also prints an over-release); all
others passing on the desktop, lint of both builds clean with warnings denied. On the WebGL 2
build the three `own_level10` tests pass, the ignored one included (logs/g4s8f-webgllib.log).

At G4 run 19: front.rs 45 (new: the YUY2 test; the depth view test now makes the stencil
view), tests/cache.rs 1, unit 5 (new: multisampled array declarations, the stencil mapping),
com.rs 15, programs.rs 25 (new: `own_yuy2`; the early tests case ignored as before); all others
passing on the desktop, lint of both builds clean with warnings denied (logs/g4s7d-*, g4s7e-*).

At G4 run 18: front.rs 44 (new: the NV12 test), tests/cache.rs 1, unit 3, com.rs 15,
programs.rs 24 (new: `own_nv12` and eight `own_cases` tests; one, the early tests case,
ignored as a known failure); all others passing on the desktop, lint of both builds clean with
warnings denied (logs/g4s6i-*). `depth_with_stencil_copies_as_direct3d_does` passes (it is no longer a known
failure; the WebGPU back end copies both aspects).

At G4 run 17: front.rs 43 (new: the tessellation and buffer target view tests), tests/cache.rs
1, unit 3, com.rs 15 (two tests renamed: the hull and domain stages are served), programs.rs
15 (`own_refused` asks for a keyed-mutex texture where it asked for a hull shader); all
passing on the desktop, lint of both builds clean with warnings denied (logs/g4s5f-*).

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

- Run 23: origin/phase-1-literals (the literal check's branch) was not merged. The brief asks
  for it once it has a passing milestone; the branch still changes its rules from run to run
  (its last commits are "the second version of the rules" and an attack's findings), it is a
  phase-one line of 344 commits and 95,000 lines, and this track's task allows merging only the
  graphics branches. The literal rule is applied by hand meanwhile (Literals).
- Run 23: a geometry shader's primitive naming a layer past the bound view's is drawn nowhere,
  where real Direct3D draws it into the view's first layer (plan 4.12 calls the layer split
  exact). The pass-through the shaders crate makes keeps exact matches of the run's layer only,
  and the pixel program's layer is the run's, so the front cannot draw such a primitive with
  the value it named; Asked of the shaders track. Viewports past the bound ones are drawn
  nowhere, which is what real Direct3D does on the RTX 3080.

- Run 22: a Direct3D 10 device made without an adapter answers its adapter's `GetParent` with a
  factory whose interface tables are copies refusing `IDXGIFactory1` and later in
  `QueryInterface` (as DXGI 1.0's factory, which Direct3D 10 makes, answers: Wine's
  test_device_interfaces passes so on real Direct3D); `CreateDXGIFactory` itself still gives the
  full factory (no measurement of real DXGI there yet).
- Run 22: `EarlyTestsPerPrimitive` is built for list topologies, one instance, direct draws only;
  strips (whose primitives drawn alone would lose their winding), instanced draws (whose
  instance order the split would change) and indirect draws are refused by name.

- Run 21: Direct3D 10's device converts each Direct3D 10 object it is handed to its Direct3D 11
  interface by `QueryInterface` (a reference taken and given back per object per call) and calls
  the context's own method; a fast path reading our object without the query is not built (no
  Direct3D 10 program measured).
- Run 21: our library exports `D3D10CreateDevice` and `D3D10CreateDevice1`, but only
  d3d10core.dll is made as a forwarder: d3d10.dll and d3d10_1.dll also export the effects
  framework, shader compiling and reflection, which this library does not serve, and a
  forwarder must not name exports that do not exist. Wine's d3d10core file runs with the library
  itself copied in as d3d10.dll (every test ran, so what the test imports from it resolves).
- Run 21: the system's d3d10.dll and d3d10_1.dll calling `D3D10CoreCreateDevice` get ours (they
  ask on the program's behalf; `factory::called_by_program_or`); every other system caller gets
  the system's, as for `D3D11CreateDevice`.
- Run 21: in Direct3D 10 mode, the final release of a bound resource does not unbind it
  (Microsoft's page on `SwapDeviceContextState` says Direct3D 10 mode does); the context keeps
  its library reference as in Direct3D 11 mode.
- Run 21: Direct3D 10's `CreateGeometryShaderWithStreamOutput` gives the one buffer its
  `OutputStreamStride`, and where entries write several buffers each buffer the components its
  entries write (Direct3D 10 has one stride argument); not compared with real Direct3D.

- Run 20: `TargetsSplit` runs each group's draw in a pass of its own and ends it after the
  last run, so a split draw costs a pass per group (counted as the slow path). Queries, pixel
  storage views and stream output during such a draw are refused by name instead of kept to the
  first run as plan 4.12 says (none is reachable on a back end without per-target blends today:
  WebGL 2 has no storage in pixel shaders and no stream output; queries are).
- Run 20: an over-release answers 0xffffffff and leaves the counts as they were (real Direct3D
  answers so; what it does with the count after is not known).

- Run 19: a multisampled texture of more than one layer is refused (`E_NOTIMPL`) instead of
  held as a texture per layer (plan 4.2); every program's multisampled array slots are converted
  as one layer.
- Run 19: stencil views always move the stencil in the program (`SwizzleInShader`): the
  interface has `Features::VIEW_SWIZZLE` but no swizzle in `TextureViewDesc`, so there is no view
  route to take.
- Run 19: programs taking `AtomicExchangeLoop` are refused at creation after one reset the
  display driver (Where things stand); a valid request refused by name, not faked.
- Run 19: YUY2 is held at half width (`Texture::mip_extent` is in texels; boxes and places are
  turned from pixels at `UpdateSubresource` and `CopySubresourceRegion`). One level only (more
  answer `E_NOTIMPL`; what Direct3D answers is not checked); its storage views (Direct3D's
  documentation also lists `R32_UINT`) are not checked against real Direct3D.

- Run 18: NV12 (plan 4.12) is two textures: the luma plane `r8unorm` (the texture itself) and
  the chroma plane `rg8unorm` at half size (`Texture::chroma`, a front texture of
  `R8G8_TYPELESS`). Views of `R8_UNORM` and `R8_UINT` name the luma, `R8G8_UNORM` and
  `R8G8_UINT` the chroma; the `_UINT` views go through their group's twin (`ReinterpretCopy`),
  as any typeless texture's do, since WebGPU's views reinterpret only sRGB. More than one level
  and misc flags other than shared answer `E_NOTIMPL` (what Direct3D answers is not checked).
  `CheckFormatSupport` answers the uses both planes' formats have (2D textures, loads,
  sampling, targets, CPU locking) and none of the video uses.
- Run 18: passes whose depth buffer is larger than their colour targets are begun through the
  pass tracker (plan 4.7's route) while the front's other passes are still its own (the plan
  has every pass go through the tracker; moving the rest is What comes next 5).
- Run 18: the examples compile their shaders with the system's d3dcompiler_47.dll at run time
  (`own_cases`), so one program can hold many small cases; both runs of a comparison take the
  same bytecode.

- Run 17: an 8- or 16-bit integer vertex element the vertex shader's input signature reads with
  the other signedness (Godot's 3D renderer through ANGLE: `R16G16B16A16_UINT` bones read as
  `int4`) is fetched in the program (`pulled`), whose fetch keeps the bits as Direct3D hands
  them over; WebGPU's formats would refuse the pipeline, and its signed formats would extend the
  top bit. Counted as `VertexFormatConverted` (the interface's name for a vertex format fetched
  from a storage buffer); a layout's `pulled_paths` names what each draw counts.
- Run 17: `IDXGISurface1::GetDC` is a memory DC on a bitmap over our own memory made as DXGI
  makes it (`D3DKMTCreateDCFromMemory`; a DIB section answered `GetObject` with an image size
  real Direct3D does not give), filled through a staging copy and `Map` (waits for the GPU)
  unless `discard`, and written back whole by `UpdateSubresource` at `ReleaseDC` (the dirty
  rectangle is not used). No slow path names it (it waits like a staging read back).
- Run 17: a render target view of a buffer is a view of a one-row texture of its elements,
  copied in from the buffer at a pass's start (unless the pass clears it) and out at its end,
  counted as `ReinterpretCopy` (the interface's name for a part a resource cannot be viewed as).
- Run 17: `own_refused` now asks for a keyed-mutex texture where it asked for a hull shader
  (hull shaders are served).

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

None in G4 run 22.

None in G4 runs 20 and 21.

None in G4 run 19 (the merge's conflict in slow.rs was resolved by keeping both sides' entries).

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

- 2026-11-18, G4 run 24, the desktop (i9-9900K, RTX 3080) through the queue, not alone (other
  tracks' hazard jobs ran one at a time before ours; the Mac's load average was 62 to 85 at
  20:15, from the other tracks), at dd123eec: Wine's d3d11 file 140 of 165 in 168 s of tests
  (20:03 to 20:06 desktop time); d3d10core 103 of 111 in 97 s; every Direct3D 11 scene through
  the whole library 135 pass, 0 fail, 4 absent; the WebGL 2 build's scenes 88 match, 27 fail,
  30 absent (67, 52, 27 at run 23; logs/g4s12e-webgl.log); real programs through the whole
  library, 120 frames each: 15 match real Direct3D on every compared frame, bgfx-24-nbody runs
  to its last frame (never compared), and atg-simplecomputepc (154,515 pixels, division
  precision, Asked), atg-simplemsaa_pc (8 pixels, 4 past the tiles' allowance) and
  bgfx-26-occlusion (about 6,500 pixels: the overlay panel without NVAPI and the 49 for 50
  count) differ as at run 23 (logs/g4s12e-programs.log, -bgfx.log). Module size and
  building-ahead hits not measured again (run 16 above).
- 2026-11-18, G4 run 21, the desktop (i9-9900K, RTX 3080) through the queue's hazard group, not
  alone (other tracks' hazard jobs one at a time beside ours; our jobs waited up to 40 minutes
  for theirs): Wine's d3d11 file 133 of 165 run, 151.6 s at d2892718 and 156.3 s at 944ff499
  (logs/g4s9f-wine.log, g4s9k-wine.log); Wine's d3d10core file 90 of 111, 96.4 s
  (logs/g4s9j-wine10.log; real Direct3D's baseline passes 111). Scenes, the WebGL 2 build and
  real programs: Where things stand (logs/g4s9f-scenes.log, -webgl.log, -programs.log,
  -bgfx.log), and again at 944ff499 (logs/g4s9l-*: the same verdicts; single runs of 120
  frames: sdk-tutorial02 to 07 0.9 s each, chocolate-doom 3.1 s, atg-simplelightingpc 3.1 s,
  atg-simpleinstancingpc 3.4 s, atg-simplebezierpc 8.0 s, godot-platformer-2d-d3d11 13.3 s,
  godot-platformer-3d-d3d11 46.2 s). Module size and
  building-ahead hits not measured again (runs 16 and 18).

- 2026-11-18, G4 run 20, the desktop (i9-9900K, RTX 3080) through the queue's hazard group, not
  alone (other tracks' hazard jobs one at a time beside ours; the WebGL 2 scenes waited 27
  minutes behind another track's job), at fbcd643's tree: Wine's d3d11 file 132 of 165 run,
  161.2 s (logs/g4s8h-wine.log). The rig's Direct3D 11 scenes through the whole library: 123
  pass, 14 fail (raster and geometry stop, views differs), 4 absent (logs/g4s8h-scenes.log). The
  WebGL 2 build: 63 match, 61 fail, 23 absent (logs/g4s8h-webgl.log; targets stops at a format
  the back end lacks, not at its blends: its per-target part needs 10_1). Real programs through
  the whole library, 120 frames each (logs/g4s8h-programs.log, g4s8h-bgfx.log): matching every
  compared frame: sdk-tutorial02, 04 to 07 (0.9 to 1.0 s), chocolate-doom 3.3 s,
  atg-simplelightingpc 3.2 s, atg-simpleinstancingpc 3.4 s, atg-simplebezierpc 9.9 s (new),
  godot-platformer-2d-d3d11 13.8 s, godot-platformer-3d-d3d11 43.7 s, bgfx-01-cubes,
  -35-dynamic, -37-gpudrivenrendering, -38-bloom; bgfx-24-nbody runs (not compared);
  atg-simplecomputepc, atg-simplemsaa_pc and bgfx-26-occlusion differ as at run 19. Module size
  and building-ahead hits not measured again (runs 16 and 18 below).

- 2026-11-18, G4 run 19, the desktop (i9-9900K, RTX 3080) through the queue's hazard group, not
  alone (other tracks' hazard jobs one at a time beside ours; Mac load average 32 at the run's
  start), at 01a6bfe's tree: Wine's d3d11 file 128 of 165 run, 154 s (logs/g4s7d-wine.log).
  The rig's Direct3D 11 scenes through the whole library: 122 pass, 16 fail, 4 absent (120, 19, 4
  at run 18; logs/g4s7d-scenes.log). The WebGL 2 build: 43, 84, 21 as at run 18 (samplers still
  stopped at the level query, fixed after: 45, 80, 23 at 3b3241c, logs/g4s7e-webgl.log). Real
  programs through the whole library, 120 frames each (logs/g4s7d-programs.log, g4s7d-bgfx.log):
  the ten of run 18 match as before; bgfx-01-cubes, -35-dynamic, -37-gpudrivenrendering and
  -38-bloom match (new), -24-nbody runs (not compared), -26-occlusion differs;
  atg-simplecomputepc and atg-simplemsaa_pc differ as before; atg-simplebezierpc exits
  0xc0000409. Module size and building-ahead hits not measured again (run 18 and 16 above).

- 2026-11-18, G4 run 18, the desktop (i9-9900K, RTX 3080) through the queue's hazard group, not
  alone (other tracks' hazard jobs one at a time beside ours; Mac load average 77 at the run's start, 47.05 at its end), at
  f5eed62's tree: Wine's d3d11 file 128 of 166, the run 152.1 s (logs/g4s6g-wine.log). Real
  programs through the whole library, 120 frames each (logs/g4s6g-programs.log): the ten as at
  run 17 match every compared frame (sdk-tutorial02, 04 to 07, chocolate-doom,
  atg-simplelightingpc, atg-simpleinstancingpc, godot-platformer-2d-d3d11,
  godot-platformer-3d-d3d11); atg-simplecomputepc and atg-simplemsaa_pc differ as before;
  atg-simplebezierpc stops (the tessellator under FXC) and bgfx's six at
  `MultisampledArrayTexture`, as before. The rig's Direct3D 11 scenes through the whole
  library: 120 pass, 19 fail, 4 absent, as at run 17 (logs/g4s6g-scenes.log).
- Rerun at 53a45b6 (the fitted passes and the staging refusal; same machine and queue, 06:20 to
  06:30 desktop time): Wine's d3d11 file 128 of 166 with the same 38 failing (151.3 s,
  logs/g4s6j-wine.log); the same programs match and fail; scenes 120, 19, 4; the WebGL 2 build
  43, 84, 21, with targets and mips now refused (`E_NOTIMPL`) where the device had been removed.
- Same run, the WebGL 2 cases (the library built on the WebGL 2 back end over ANGLE, the rig's
  Direct3D 11 scenes; logs/g4s6g-webgl.log): 43 outputs match (34 at run 13), 84 fail, 21
  absent (they need 10_1 or 11_0; the build offers 10_0). Matching: blend 4, copies 5,
  deferred, depth 9, draws, dynamic, instanced, levels 2, quads, queries 2, stencil 2,
  swapchain 5, textures 7, topology, triangle. Stopping: targets and mips ("webgl: the format
  cannot be read back here": a staging texture of a format the back end cannot read back;
  since this run such a staging texture is refused at creation, `E_NOTIMPL`), samplers (the
  back end lacks `TEXTURE_LEVEL_QUERY`, refused by name), constants (device lost in the back
  end, not read), formats (`E_FAIL` from `CheckFormatSupport`), views, raster, geometry and
  streamout (`E_NOTIMPL`: texture views, polygon mode, no geometry stage at 10_0 on WebGL 2).
- Same run, building-ahead hits on Godot's 3D platformer (scratch run-warm.sh, one cache
  directory, empty at the first run; logs/g4s6g-warm.log): first run 66 pipelines built at
  their first draw; second run 61 recorded combinations built at its start, all 61 drawn, and
  26 still built at their first draw (the draws not recorded: through the geometry stage's
  helper and others not read one by one). Built ahead from shaders' arrival: 124 prepared, none
  drawn in either run. Frames identical to real Direct3D's in both runs (42.6 s and 41.5 s
  wall time, single runs).

- 2026-11-18, G4 run 17, the desktop (i9-9900K, RTX 3080) through the queue's hazard group, not
  alone (other tracks' hazard jobs one at a time beside ours; Mac load average 54 to 71): Wine's
  d3d11 file 127 of 166, the test job 2 min 35 s (04:46:59 to 04:49:34 desktop time; 124 of
  166 at 04:26:56 to 04:29:04). Real programs through the whole library at 981ada7's tree, 120
  frames each (logs/g4s5d-programs.log): matching every compared frame with largest difference
  0: sdk-tutorial02, 04 to 07 (0.9 to 1.0 s), chocolate-doom 3.1 s, atg-simplelightingpc 3.1 s,
  atg-simpleinstancingpc 3.3 s, godot-platformer-2d-d3d11 13 s, godot-platformer-3d-d3d11
  41.9 s (new); atg-simplemsaa_pc frames 60 and 120 within the rig's allowance (largest
  differences 12 and 14), frame 10 not (4 pixels in 2 tiles past it, largest 11);
  atg-simplecomputepc differs inside the Mandelbrot set only (154515 pixels; Asked);
  atg-simplebezierpc stops (exit 0xe06d7363, 7.8 s; its hull shaders now made, the tessellator
  fails FXC); bgfx's six examples stop at `MultisampledArrayTexture` (exit 3, 1.2 s). The rig's
  Direct3D 11 scenes through the whole library: 120 pass, 19 fail, 4 absent (logs/g4s5d-scenes.log;
  the scenes failing are raster (wireframe), views (its stencil view, refused by name),
  geometry (`PrimitiveId` and layers from a geometry shader) and tessellation (the
  tessellator's FXC failure); the absent ones need B5G6R5 targets, 11_1 and min/max
  filtering). Module size and building-ahead hits: not measured again (run 16 and 15 above).

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

- Interface track (G4 run 24): a vertex format for `B8G8R8X8_UNORM`, reading blue, green and red
  with the fourth component as 1.0. The Direct3D 11.3 specification (19.1.3.3, "Defaults for
  Missing Components") reads an X component as missing, so A is 1.0 (it gives
  `R24_UNORM_X8_TYPELESS` as the example), which is also what the RTX 3080 reads. The WebGPU
  track's answer (map it to `Bgra8Unorm`, X read as written, "undefined") would read the byte
  written; so the front keeps `B8G8R8X8_UNORM` refused as a vertex format and `CheckFormatSupport`
  claims no vertex use of it (Wine's test_vertex_formats runs 98 of real Direct3D's 100 checks).

- Shaders track (G4 run 23): the pass-through after a geometry shader choosing a layer, to keep
  a primitive naming a layer at or past the view's layer count in the run of layer 0 (the count
  as a third value of `PASSTHROUGH_HELPERS`) and to pass the layer it named to the pixel program
  as a flat value instead of the run's `HelperValue::Layer`: real Direct3D draws such a
  primitive into the view's first layer and its pixel shader reads the named value (Wine's
  test_layered_rendering in both files, nine checks). Also, for test_clip_distance:
  `LinkedClipDistanceCount` (a vertex shader writing two clip distances in the program) and
  `ClipDistance` written by a geometry shader where the back end has none.
- Shaders track (G4 run 22, the output-kind item below): done in their run 33; the front takes
  `targets_in_shader` since run 23.

- WebGPU track (G4 run 22): declare `Features::PRIMITIVE_INDEX` where wgpu's adapter has the
  primitive index; the shaders crate now converts `SV_PrimitiveID` in pixel shaders natively,
  and this front refuses such shaders by name without the feature (the rig's geometry.ps_id).
- Shaders track (G4 run 22): a pixel output of another kind (float, unsigned, signed) than its
  target written as its bits (a variant naming each target's kind), as Direct3D writes it
  (Wine's d3d10core test_layered_rtv_mismatch: a float output into an `R32G32B32A32_UINT`
  target reads back as the float's bits); the front refuses such draws by name meanwhile.
- Shaders track's note of its run 31 (lift the front's refusal of `AtomicExchangeLoop` and
  rerun test_atomic_instructions): not done in run 22. The test reset the owner's display driver
  at run 19 and stays quarantined by the rig; whether to run it again is for the orchestrator
  or the owner, since the cause of the reset was not identified, only two possible causes
  removed.

- Shaders track (G4 run 19): the `AtomicExchangeLoop` program of Wine's test_atomic_instructions
  reset the display driver on the RTX 3080 through this front (07:03:58 desktop time; the test is
  quarantined); this front refuses such programs until the loop is shown to end.
- WebGPU track (G4 run 19): a stencil view read in WGSL gives 0 in its first component on wgpu's
  Direct3D 12 path (Wine's test_depth_stencil_sampling through this front: every stencil value
  reads 0); WebGPU defines the stencil there. Withdrawn in run 24: the WebGPU track found the
  back end right, and the front's own_cases `stencil` case showed the view was never bound (the
  program read the empty slot's 1 by 1 stand-in: width 1, stencil 0), because the front's
  binding check took the stencil view's depth format for "the other kind" than the unsigned
  integers the program reads. Fixed in the front (`View::read_kind`).

- Shaders track (G4 run 18): `VariantFlags::LINE_COVERAGE` (`AntialiasedLinesAsQuads`, plan
  4.12) for alpha-antialiased lines, a listed case; the front refuses such line draws until the
  variant and the stage helper's line rectangles exist. `EARLY_TESTS` (`EarlyTestsInShader`),
  which the front now refuses by name at the draw (own_cases early, Wine's
  test_early_depth_stencil).

- Shaders track (G4 run 17): the tessellator program fails FXC on wgpu's Direct3D 12 path
  ("internal error: argument pulled into unrelated predicate", in your Next list): this front
  draws hull and domain shaders through `Stages::tessellated` since run 17 and the rig's
  tessellation scene stops there (device lost at the first patch draw). Also streams other
  than 0 (`GeometryStreams`: test_so_statistics_query), geometry shaders writing no position
  (`GeometryWithoutPosition`: test_stream_output_components), stream output after a domain
  shader (test_quad_tessellation), layer and viewport outputs of geometry shaders
  (test_layered_rendering, test_multiple_viewports), and test_vs_input_relative_addressing's
  vertex shader ("shaders the converter does not yet turn into a valid module").
- Shaders or WebGPU track (G4 run 17, atg-simplecomputepc): only the pixels inside the
  Mandelbrot set differ from real Direct3D (154515, all of them; every other pixel identical).
  Its shader takes `frc(div(1200.0, 300.0))` there and samples a gradient at it: real Direct3D
  gives 0 (the first texel), ours the last texel, so the converted program's division gives a
  value just below 4 (inferred, not read out; Direct3D allows 2.5 ULP for `div`). wgpu compiles
  the HLSL with FXC and `D3DCOMPILE_ENABLE_STRICTNESS` only.

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

## Literals

The brief's rule of 2026-11-18 (no magic numbers or literals) and the audit's fix list
(orchestrator/magic-literals/fix-graphics.md; findings for this checkout in
final/lists/graphics-direct3d-11/: wcontainers-directx 311 code findings, 13 redefinitions, 45
named constants; wcontainers-direct3d-11 in final/lists/graphics/). The automatic check is not
on a branch yet (origin/phase-1-literals holds no wcontainers-magic-literals crate at run 22),
so no baseline is made.
- Run 24, format.rs: the front's own copies of seven DXGI numbers (`UNKNOWN`, `R16_UINT`,
  `R32_UINT`, `NV12`, `YUY2`, `R8G8_TYPELESS`, `R8_TYPELESS`) are now re-exports of the
  infrastructure crate's `dxgi` constants, every DXGI number in its tables is the `dxgi` name
  (storage, view, sRGB twin, typeless, target, sRGB, depth, texture, group, copy and index
  tables), and `vertex_format` uses the infrastructure crate's `dxgi_vertex_format`, the same
  table it had copied. Left: 189 to 191 in `unserved` (the infrastructure table names no
  sampler feedback or A4B4G4R4 format), and DXGI numbers in device.rs and context.rs (view
  checks and copies).
- Run 24: still no baseline. docs/literals.md on origin/phase-1-literals (tip f561838a) says a
  track adopts the check only by merging main once main has it, never the literals branch
  itself; origin/main (77e50ddc) holds no wcontainers-literals yet. Written this run: the stencil
  view formats in `View::reads_stencil` from the infrastructure crate's `dxgi` table
  (`X24_TYPELESS_G8_UINT`, `X32_TYPELESS_G8X24_UINT`), which now names every DXGI format; the
  front's other DXGI numbers (format.rs's tables, the view checks in device.rs) can take the same
  names next. No new bare literal was written.
- Run 23: origin/phase-1-literals now holds the check, but was not merged (Departures), so
  there is still no baseline. Written this run: the library's viewport limit from the windows
  crate (`D3D11_VIEWPORT_AND_SCISSORRECT_OBJECT_COUNT_PER_PIPELINE`); in the front, Direct3D's
  numbers named once in shader.rs where the other DXBC numbers are, until the shaders crate's
  `dxbc` module owns them (`RENDER_TARGET_ARRAY_INDEX`, `VIEWPORT_ARRAY_INDEX`,
  `DCL_CONSTANT_BUFFER`, `CONSTANT_BUFFER_DYNAMIC_INDEXED`); the passthrough's helper block
  sized from its module, the slot masks from `u8::BITS`/`u16::BITS`.
- Replaced in run 22 (wcontainers-directx, Windows-only, from the windows crate):
  `D3D10_SDK_VERSION`, `D3D10_DRIVER_TYPE_HARDWARE`, `D3D10_CREATE_DEVICE_DEBUG` and
  `_SINGLETHREADED` (d3d10core.rs); `D3D11_SDK_VERSION`, `D3D11_CREATE_DEVICE_DEBUG`,
  `D3D_DRIVER_TYPE_UNKNOWN`/`_HARDWARE`/`_SOFTWARE` and the eleven `D3D_FEATURE_LEVEL_*` of
  `KNOWN_LEVELS` (mod.rs, which retyped them, `CREATE_DEBUG` a second time among them);
  `MONITORINFOF_PRIMARY` (output.rs), `DXGI_PRESENT_TEST` (swapchain.rs),
  `DXGI_RESOURCE_PRIORITY_NORMAL` (objects.rs). The device's and the factory's patched table
  counts are named once (`GATED_TABLES`, `FACTORY_TABLES`). In wcontainers-direct3d-11: the
  building-ahead key's vertex strides and target formats are sized by `VERTEX_SLOTS` and
  `wcontainers_gpu::MAX_COLOR_TARGETS`.
- Kept, named once: `D3D10_1_SDK_VERSION` (d3d10_1.h; the windows crate has no d3d10_1
  module), `MAP_WITHOUT_INITIAL_DISCARD` (the windows crate lacks it).
- New literals written this run, each waiting on an owner's constant: the six faces of a cube in
  the 10_0 cube rule (`CUBE_FACES`, wcontainers-gpu, not landed; the front's other cube 6s are
  the same fact), and the vertices of one primitive of each list topology in `list_vertices`
  (`Topology::primitives`, wcontainers-gpu, not landed).
- Left: everything else in the lists. Most of the front's are Direct3D numbers (formats, enums,
  limits, DXBC tokens) whose one home is the infrastructure crate's `d3d`/`dxgi` modules (the
  Direct3D 12 track's) and the shaders crate's `dxbc` module, not on a merged branch yet; the
  interface's `Key`, `CUBE_FACES`, `Topology::primitives`, the small-float codec and format
  helpers (graphics track) likewise. The COM method labels of the generated code (hard case 8)
  belong in generate.py. The examples' and tests' sizes are test data.

## Licences

G4 run 22 added no dependency (the windows crate features used were already on) and installed
nothing on the desktop.

G4 run 21 added no dependency (the windows crate's `Win32_Graphics_Direct3D10` feature was
already on) and installed nothing on the desktop.

G4 run 18 added the windows crate's `Win32_Graphics_Direct3D_Fxc` feature to the library's
dev-dependency (same crate, MIT OR Apache-2.0; `D3DCompile` for the examples). Installed nothing
on the desktop.

G4 run 17 added the windows crate's `Wdk_Graphics_Direct3D` feature (same crate, MIT OR
Apache-2.0; `D3DKMTCreateDCFromMemory` for `GetDC`). Installed nothing on the desktop.

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
