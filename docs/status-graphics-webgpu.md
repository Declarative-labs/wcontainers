# Graphics: the WebGPU back end (G2, G2b, G2c)

Branch phase-2-graphics-webgpu. The plan is docs/phase-2-graphics.md (sections 4, 5.7 and 12, G2).
Crate: crates/wcontainers-gpu-webgpu, on wgpu 30.0.2.

## Where things stand

G2 passed its independent check. G2b (what the fronts need from this back end): runs 1 to 4 are
"Run 11" to "Run 15" below. G2b's independent check found nine problems; run 13 fixed them (list
in "Run 13"). Run 14 measured instance data at more streams and made binding sets made again each
frame find their bind groups. Run 15 moved copies of converted formats onto the GPU. Run 16
merged, found nothing new asked, ran both full runs and measured the timestamp workload's cause
(naga's loop bounding). Run 17 fixed the second G2b check's findings (query sets past 4096, query
results from a ring, two stale hand-over sentences, the point fill test's report). Run 18 fixed
the third check's findings (BGRX vertices answered and tested against Direct3D 11, integer formats
no longer declared resolvable, part of a depth level copied into colour, the rest of the 16-bit
group copied into depth, prepared strips built for both index formats) and answered what the
newest merges ask. G2b passed its fourth independent check. Run 19 is G2c run 1 (below): the
check's open findings, the new requests (three-channel 32-bit textures), and where real programs'
frame time goes. Run 20 is G2c run 2 (below): the interface's ruling on depth and stencil
copies within one subresource built, subgroups, and the stencil view question answered. Run 21
is G2c run 3 (below): the second G2c check's findings (the primitive index, the frame breakdown
with counters, bytes and the machine's load, Direct3D 12 texture reads at unaligned offsets,
three-channel copies at any width, Chrome's launch, subgroups in the browser, licences) and the
first part of the literal sweep. Run 22 is G2c run 4 (below): the late branches merged, fine
derivatives declared, stencil views read as Wine's test reads them, the load sampled within each
recorded program run, both full runs and web/check again, and a second part of the literal
sweep. Run 23 is G2c run 5 (below): the late branches merged, the literal check adopted, a
third part of the sweep, both full runs and web/check again. Run 24 is G2c run 6 (below): the
Direct3D 11 and literals branches merged (-direct3d-12 and -shaders not, for the literal check),
nothing new asked, a fourth part of the sweep. Run 25 is G2c run 7 (below): -direct3d-11 and
-webgl merged (-direct3d-12 and -shaders still not, for the literal check), nothing new asked,
the frame report written through serde, both full runs and web/check. Run 26 is G2c run 8
(below): every graphics branch merged (-direct3d-12 and -shaders too, now that the shaders
track adopted the check), nothing new asked, the literal check moved to an LF worktree on the
desktop, both full runs, web/check and two frame breakdowns. Run 27 is G2d run 1 (below): the
Direct3D 12 front's last blocker (sRGB views of textures that allow storage) and its two other
requests (float clears keeping their bits, buffer copies into slices lying apart), and the five
findings the G2c check left open. Every request found is built and tested or refused by name
(list below). Earlier runs follow ("Run 5" to "Run 10" are G2 runs 1 to 6; G2a runs 1 to 4 after them).

What the fronts' status files ask of this back end (read at G2b runs 1 and 2, after merging every
graphics branch), and where each stands:
- Timestamps (Direct3D 12: every real program tried stops there; Direct3D 11: so timestamps need not
  be reported disjoint): built. `TIMESTAMPS` where wgpu has `TIMESTAMP_QUERY` and resolves in
  nanoseconds; tests/timestamps.rs, tests/direct3d.rs (against Direct3D 11), and in Chrome by
  web/check. Calibration: through base primitives (see "How another track builds"). Their
  resolution differs (see "How another track builds"): natively on the RTX 3080 every timestamp
  was a multiple of 1024 ns; Chrome as users have it rounds them much more coarsely.
- `R32G32B32_*` textures (Direct3D 11, G4 run 16: test_create_texture2d test 21): built (run 19).
  `Rgb32Float`, `Rgb32Uint` and `Rgb32Sint` are held in their four-channel twins with a fourth
  channel of one (`FormatConverted`, src/packed.rs), declared sampled and copied (filtered where
  the twin is), not drawn into or stored. Writes widen on the host, read backs pack on the host,
  and copies with buffers or between the three run on the GPU through an `r32uint` texture of
  three texels a texel, every bit moved as it is. Before, the formats were declared with no use.
  examples/scenes/converted.rs `three_channels` (tests/converted.rs natively, web/check in
  Chrome): seeded bits, NaNs and denormals among them, written, read back, copied and loaded,
  every bit kept; tests/direct3d.rs `three_channel_formats_load_and_read_back_as_direct3d_does`:
  loads and read backs equal to Direct3D 11's `R32G32B32_*`, 0 of 1536 channels differing in each
  format (seed 1, 4 textures).
- Point filling under its own name: built. `POLYGON_MODE_POINT` where wgpu has it (Vulkan, not
  Direct3D 12 or browsers); wireframe `POLYGON_MODE` where wgpu has `POLYGON_MODE_LINE` (every
  native API; browsers have neither). tests/raster.rs.
- Instance step rates in the program: measured at 4, 8 and 16 streams a draw, natively and in
  Chrome (examples/scenes/pulled.rs, tests/pulled.rs, web/check; Measurements, run 14). Storage
  usage on every vertex buffer is expected to cost nothing, not measured: every buffer already
  has it (`buffer_usage`), so no run compares buffers without it. CPU a frame of 256
  draws: with binding sets kept, fetching costs no more than vertex buffers (0.28 against 0.30 ms
  at 4 streams on Vulkan, less at 8 and 16). With a set made per draw, the route the Direct3D 11
  front takes, it costs clearly more: at 4 streams 0.53 against 0.30 ms on Vulkan, 0.60 against
  0.37 on Direct3D 12, 0.6 against 0.3 in Chrome (G2b check: 0.7 against 0.3); at 8 streams 0.72
  against 0.50, 0.79 against 0.55, 0.7 against 0.3; the gap closes at 16 streams natively (0.98
  against 0.90, 1.11 against 1.09). The extra is the equal set's lookup (a set made per draw against sets kept: about 1 µs a draw
  at 4 streams, 2 at 16, on both APIs). Run
  14 removed the larger cost, bind groups made again each frame (1.25 ms at 4 streams on Vulkan
  before).
- `write_texture` of a block-compressed level smaller than a block (atg-simplemsaa_pc): built.
  Copy extents are rounded up to whole blocks. tests/blocks.rs
  `levels_smaller_than_a_block_take_their_texel_size`.
- `copy_texture` from `R16Unorm` into `Depth16Unorm` (Wine's test_format_compatibility): built,
  and from `R16Uint`. tests/copies.rs `sixteen_bit_group_copies_into_depth_keep_the_bits`.
- A 3D write whose layers are further apart than their rows lost every slice after the first on
  wgpu's Direct3D 12 (Direct3D 11 track's own_volumes): fixed by packing such layers on the host
  (`RowsRepacked`). tests/copies.rs `volume_writes_with_padded_slices_keep_every_slice` (failed on
  Direct3D 12 before).
- `OCCLUSION_COUNT` "where the adapter counts samples exactly": refused, absent. WebGPU defines an
  occlusion result only as zero or not, wgpu's Direct3D 12 asks for binary occlusion and its Vulkan
  for imprecise counts; fronts count by `OcclusionCountReplay`.
- Formats WebGPU lacks (`FormatConverted`; both Direct3D fronts ask for `A8_UNORM`, `B5G6R5`,
  `B5G5R5A1`, `B4G4R4A4`, the Direct3D 11 front also for `B8G8R8X8`, the Direct3D 12 front for
  the two-pixel formats): built (run 12, src/packed.rs). Held in an 8-bit RGBA format, widened on
  the host at writes and packed again at read backs; copies widen and pack on the GPU (run 15). `A8Unorm`, `Bgrx8Unorm` and
  `Bgrx8UnormSrgb` are sampled, filtered, drawn into, blended and resolved (only their own
  channels written); `B5g6r5Unorm`, `Bgr5a1Unorm`, `Bgra4Unorm`, `Rgbg8Unorm` and `Grgb8Unorm`
  are sampled, filtered and copied, and declared without `RENDER` (a draw would keep 8 bits a
  channel where Direct3D keeps 4, 5 or 6; the two-pixel formats are not render targets in
  Direct3D either). None has storage bits. tests/converted.rs (natively, and in Chrome through
  web/check) and tests/direct3d.rs `converted_formats_load_filter_and_draw_as_direct3d_does`:
  every load and filtered texel equal to Direct3D 11's on the RTX 3080, A8 and BGRX drawn equal
  byte for byte (BGRX's X apart: Departures).
- Asked by the Direct3D 11 front, answered by the capability list: clip distances on wgpu's
  Direct3D 12 path (wgpu has none there; `CLIP_DISTANCE` is declared where wgpu has it), logic
  operations, min/max filtering, NV12 and video formats (WebGPU has none of them; declared
  absent).
- Asked of others, not this back end: the view swizzle (interface), vertices past the end in
  `ExecuteIndirect` (a listed loss), multisampled array targets (absent in WebGPU).
- Already done before G2b: vertex formats WebGPU lacks (run 9), combined depth-stencil copies
  (run 7).
- `configure_surface` of a format the window lacks (Direct3D 12: `rgba16float`, which wgpu's Vulkan
  windows on the desktop lack, removed the device): built (run 13). Such a window is presented
  through an 8-bit format it shows as sRGB, by a draw (`PresentConverted`). tests/surface.rs (lost
  the device on Vulkan before; on Direct3D 12 the window took `rgba16float` only as linear light).
- Freeing the back end's tables when a program or block is destroyed (Direct3D 12: "its own tables
  keep their entries"): built (run 13). tests/scenes.rs
  `destroyed_programs_and_blocks_free_their_pipelines`.
- `B8G8R8X8_UNORM` vertices (Direct3D 11: test_vertex_formats runs 98 of real Direct3D's 100
  checks): served by the interface's `VertexFormat::Bgra8Unorm` (wgpu's `unorm8x4-bgra`), with X
  read as written; Direct3D leaves it undefined (the RTX 3080's Direct3D 11 reads 1.0, and Wine's
  test ignores it). No interface format is needed: the Direct3D 11 front's own table
  (`format::vertex_format`) maps DXGI 87 but not 88, so the remaining step is that front's line
  (asked of it below). tests/direct3d.rs `bgra_vertices_read_as_direct3d_reads_bgra_and_bgrx`
  (64 seeded vertices: every component equal to Direct3D 11's `B8G8R8A8_UNORM`, and to its
  `B8G8R8X8_UNORM` but for the fourth), tests/vertices.rs `bgra_vertices_read_as_they_mean`
  natively and the same in Chrome by web/check.
- Copies between `R16_UNORM` and `D16_UNORM` (Direct3D 11: test_format_compatibility, removed at
  line 992): the device was lost by this back end. The D3D11 track's newest per-test log on the
  desktop (C:\wc\out\phase-2-graphics-direct3d-11\wine\logs, 2026-11-18 01:01) ends with
  `webgpu: a copy into depth from a format outside its typeless group`: that front holds
  `R16_TYPELESS` not bound as depth as `r16float`, and Wine's last rows copy it into `D16_UNORM`.
  Built (run 18): `r16float`, `r16snorm` and `r16sint` are copied into `depth16unorm` through
  their bits (a buffer into an `r16uint` texture, then the depth draw), and part of a depth level
  is copied into a colour format of its group by drawing it into a colour texture first
  (`DepthReadByDraw`; the G2b check's `part_of_d16_into_r16` lost the device). Wine's rows
  themselves are not rerun here (the Direct3D 11 track's job). examples/scenes/depth16.rs
  `group_copies` (every colour format of the group into `depth16unorm` and back, whole and 3x2
  at an offset, bits kept), tests/copies.rs natively and web/check in Chrome; it failed before.
- Integer formats declared resolvable (Direct3D 12 worked around 34 answers; the G2b check found
  them on wgpu's Vulkan): `RESOLVE` is now declared only for float and normalised formats, as
  WebGPU and Direct3D allow. tests/scenes.rs `integer_formats_are_not_resolvable` (failed on
  Vulkan before).
- Indexed strips of a prepared combination (G2 finding): built here without an interface change.
  `prepare` of a strip builds its pipeline for 16-bit and 32-bit indices, and strips drawn
  without indices use the 32-bit one (an index format has no effect there). examples/scenes/
  prepare.rs `prepared_strips` (a prepared strip drawn without indices and with both index
  formats: 0 misses, 0 pipelines built; 2 misses before), tests/prepare.rs and web/check.
- A view swizzle for stencil views (Direct3D 11, test_depth_stencil_sampling; Direct3D 12 item 7):
  refused here, absent. WebGPU has no view swizzle and wgpu 30 has none either; the interface has
  no swizzle field. A stencil view reads stencil in its first component, so a front reading it
  elsewhere (Direct3D's `X24_TYPELESS_G8_UINT` in green) moves it in the program (the converter's
  and front's job).
- `COUNTER_BINDING` (helper set binding 15) and `SlowPath::PixelInvocationsCounted` (Direct3D 12,
  run 22): nothing to build. The binding is a storage buffer in the helper set, laid out from the
  program's reflection as every helper binding is; the Direct3D 12 front counts pixel shader
  invocations through this back end and its test_query_pipeline_statistics passes (its status,
  runs 22 and 23). The shared `OcclusionCounter`'s new GPU sum is a compute program this back end
  runs as any other.
- Copies within one subresource of depth or stencil (the interface's ruling, G1c: through a
  temporary texture of the same format): built (run 20). The region is drawn into a texture of
  its format and size, then drawn from it into its place, each aspect by the depth and stencil
  draws partial copies already use (`CopyWithinTexture`). Before, the device was marked lost by
  name. The shared suite's `a_copy_within_one_depth_or_stencil_subresource_moves_the_region`
  (`depth32float`, `depth16unorm`, `stencil8`, `depth24plus`, both combined formats) natively and
  in Chrome; tests/copies.rs `depth_copies_within_one_subresource_go_through_a_texture` (two
  layers at once, every value exact, counted once).
- Subgroups (shaders track, run 28: "ask wgpu for `SUBGROUP` where the adapter has it and
  declare `Features::SUBGROUPS`"; Godot's 3D platformer through the Direct3D 12 front stops at
  wave operations): built (run 20). The device asks for wgpu's `SUBGROUP` where the adapter has
  it and declares `SUBGROUPS` then. Where it is: wgpu's Vulkan on the RTX 3080; wgpu's Direct3D
  12 only with DXC (shader model 6), so not with FXC; not in
  Chrome until run 21, because wgpu 30's browser back end did not map WebGPU's `subgroups` feature;
  the vendored wgpu now maps it (run 21), so in Chrome it is declared where the adapter offers it.
  tests/features.rs (was tests/subgroups.rs): declared exactly where the adapter has it; a
  fragment program using `subgroupAll`, `subgroupAdd` and `subgroupBallot` draws every pixel as
  they define; a device made without the feature declares none and refuses the program by name;
  web/check runs the same draw in Chrome.
  Where the request reaches (run 27, the G2c check's finding): the Direct3D 12 library
  (wcontainers-direct3d-12 examples/directx/gpu.rs, the d3d12.dll Godot's 3D platformer loads)
  makes its wgpu instance on Vulkan, so there `SUBGROUPS` is declared and `WaveOps` answered 1
  (the Direct3D 12 status says so since its run 33's correction). The Direct3D 10 and 11 library
  (wcontainers-directx com/chosen.rs) makes its instance on Direct3D 12 with wgpu's default FXC,
  where wgpu offers no `SUBGROUP`; asked of the Direct3D 11 track under Next (give that instance
  DXC). Nothing in this back end chooses the compiler: the instance is the front's.
- The primitive index (shaders track, run 31: "ask wgpu for `PRIMITIVE_INDEX` where the adapter
  has it and declare `Features::PRIMITIVE_INDEX`", for the rig's geometry scene's ps_id; missed by
  run 20, found by the second G2c check): built (run 21). The device asks for wgpu's
  `PRIMITIVE_INDEX` where the adapter has it and declares `PRIMITIVE_INDEX` then. In the browser
  the vendored wgpu now passes WebGPU's `primitive-index` (and `subgroups`) on: its feature table
  had neither (third_party/wgpu, Departures). examples/scenes/features.rs, run natively by
  tests/features.rs (declared exactly where the adapter has it; four triangles side by side, each
  pixel reading its triangle's number; a device made without it declares none and refuses the
  program by name; failed before: declared false where the adapter had it) and in Chrome by
  web/check (`program_features`: fails if Chrome's adapter offers the feature and the device does
  not declare it).
- Stencil views reading 0 on wgpu's Direct3D 12 path (Direct3D 11 front, G4 run 19, Wine's
  test_depth_stencil_sampling): not reproduced in this back end. wgpu 30's Direct3D 12 stencil
  views use `X24_TYPELESS_G8_UINT` with a component mapping that moves the stencil (green) into
  the first component and forces the second to 0 (wgpu-hal dx12/view.rs
  `STENCIL_COMPONENT_MAPPING`), as WebGPU defines. tests/bindings.rs
  `combined_depth_stencil_views_draw_and_read_each_aspect` now also clears the stencil alone (depth
  kept, as `D3D11_CLEAR_STENCIL` does) and reads it again through the stencil view: the value is
  in the first component on Vulkan and on Direct3D 12 for `depth24plus-stencil8` and
  `depth32float-stencil8` (run 20; this test is native only). So a 0 seen through the front
  comes from the second component being read where the program's mapping was not applied, or
  from the front's path; asked back of the Direct3D 11 track under Next. Run 22 added
  tests/bindings.rs `stencil_views_bound_alone_read_each_stencil_clear`, which reads a stencil
  view as Wine's test does (bound alone, the view's size asked, each pixel loading the texel its
  position scales to, after clears of the stencil alone to several values): every value read in
  the first component on Vulkan and on Direct3D 12, for both combined formats. Wine's test itself
  through the Direct3D 11 front on this branch still reads 0 for every stencil value and passes
  every depth check (Run 22): the front's side, asked of it under Next. Settled by the Direct3D
  11 track's G4 run 24: its binding check took the stencil view's depth format for the wrong
  kind and bound the empty slot's 1 by 1 stand-in; with that fixed, test_depth_stencil_sampling
  passes through the front (its status). Nothing was needed here.
- Fine derivatives (WebGL track, its run 5, with the interface's new `Features::FINE_DERIVATIVES`:
  "the WebGPU back end should declare the bit (WGSL always has them)"): built (run 22). `FINE_DERIVATIVES` is declared on every device (WGSL's
  `dpdxFine`, `dpdyFine` and `fwidthFine` are core). examples/scenes/features.rs draws a fragment
  program differencing each pixel within its own quad row and column, natively by
  tests/features.rs `fine_derivatives_are_declared_and_difference_each_pixel_on_its_own` (Vulkan
  and Direct3D 12) and in Chrome by web/check (`program_features`). Before, the bit was not
  declared.
- sRGB views of a texture that also allows storage (Direct3D 12 front, its runs 36 to 43: the
  last item stopping Godot's 3D platformer; wgpu refused the view, "Texture view format
  `Rgba8UnormSrgb` cannot be used as a storage binding", and the device was lost): built (run
  27). A view in the texture's other sRGB form is made with the texture's usages less the
  attachment and storage usages that form lacks on the device (`convert::view_usage`, as
  WebGPU's `createView` takes them); a view in the texture's own format keeps the texture's.
  The usages go with the view, so a view made again after a rename keeps them.
  examples/scenes/views.rs `srgb_views_of_storage_textures`: an `Rgba8UnormSrgb` view of an
  `Rgba8Unorm` texture allowing storage reads every channel bit for bit as an `Rgba8UnormSrgb`
  texture of the same bytes does, before and after a `Discard` write renames it, colour channels
  decoded (below their linear value); natively tests/bindings.rs
  `srgb_views_of_storage_textures_read_as_srgb_textures`, in Chrome web/check (`srgb_views`).
  Failed before on Vulkan and Direct3D 12 with wgpu's refusal above (scratch g2d/before1.log).
- Float clears keeping their bits (Direct3D 12 front, run 37: an `R32_FLOAT` storage clear to
  the denormal 0x1 read 0 on wgpu's Direct3D 12): built (run 27). A clear of a format of 32-bit
  float channels to a value with a denormal or a NaN is made by a draw writing the bits, on every
  API (new slow path `FloatBitsClearByDraw`, an interface change; the integer clears' draw,
  `IntegerClearByDraw`, now also writes float bits). Every API needs it: measured before, wgpu's
  Direct3D 12 flushed 0x00000001 to 0, and wgpu's Vulkan kept denormals and quiet NaNs but
  quieted the signalling NaN 0x7f800001 into 0x7fc00001 (the clear value passes through an
  f64). examples/scenes/clears.rs `float_clears_keep_their_bits` (`R32Float`, `Rg32Float`,
  `Rgba32Float`, two layers, three values of four channels: both denormal signs, quiet and
  signalling NaNs with payloads, -0, normals), natively tests/writes.rs, in Chrome web/check
  (`float_clears`). Failed before on both APIs (scratch g2d/before3.log).
- Buffer copies into several slices whose rows lie further apart in the buffer than the
  region's height (Direct3D 12 front, run 37: wgpu-hal 30's Direct3D 12 sets the placed
  footprint's height to the copy's height): built (run 27). Such a copy is made one layer or
  slice at a time, each from its own rows, on every API (a few more copy commands, no slow path:
  the same bytes move). examples/scenes/gaps.rs `writes_take_each_layer_from_its_rows`: part of
  two layers of a 2D array and of two slices of a 3D texture, in `Rgba8Unorm`, `Rgba32Float`,
  `B5g6r5Unorm` and `Rgb32Float`, from two offsets and pitches, every texel exact and the rest
  kept; natively tests/copies.rs `buffer_copies_take_each_layer_from_its_rows`, in Chrome
  web/check (`copy_gaps`). Before, it failed on Direct3D 12 for 3D textures (80 bytes of
  `Rgba8Unorm` wrong; 2D arrays were right) and passed on Vulkan (scratch g2d/before2.log).

Run 27 = G2d run 1, 2026-11-19:
- Merged origin/phase-2-graphics-webgl, -direct3d-11, -direct3d-12 and -shaders (the
  Direct3D 12 branch held the others' newest work; a fast-forward); the shared line, -rig and
  phase-1-literals had nothing new. Read the four status files' new text: the Direct3D 12
  front asks three things of this back end (the three items above); nothing else new.
- Interface change: `SlowPath::FloatBitsClearByDraw` (edc6fd7f; Interface changes).
- The G2c check's open findings: the browser's per-draw cost (Measurements, run 27); the
  placement alignment defined twice (Departures; a Windows-only unit test,
  `memory::tests::placement_alignment_is_direct3d_12s`, now ties this crate's value to the
  windows crate's `D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT`, and Next asks the Direct3D 12 track
  for the one shared home); subgroups (the item above: the Direct3D 12 library is on Vulkan and
  gets them; the Direct3D 11 library's instance is asked to use DXC, Next); atg-simplecomputepc's
  division (measured, Measurements, run 27); atg-simpleinstancingpc's breakdown (repeated,
  Measurements, run 27).
- Results at 472a8daf and 34ba8f87 (2026-11-19, desktop shared with other tracks' queued jobs, not
  alone; scratch g2d/): the crate's whole suite on Vulkan and on Direct3D 12 with the interface
  crate's own tests, 311 passed, 5 ignored (the known ignores), 0 failed (full1.log, at
  472a8daf, the run's code before the literal renames); after the renames the four affected test
  files and the interface's tests again on both APIs at 34ba8f87, all passing (after2.log). web/check
  (webcheck1.log): every picture equal to Direct3D 11's at tolerance 0 natively on both APIs and
  in Chrome, every twist differing, every check passing in Chrome including the three new ones,
  and the shared suite in Chrome 44 passed, 0 failed (natively 44 on each API in full1.log).
  Lint with warnings denied, native all targets and WebAssembly examples: clean. The literal
  check at HEAD in the LF worktree: exit 0, 0 marks; this crate's baseline lines 192
  (unchanged); the check's first run found 8 new literals, named or derived before committing
  (lit1.log, lit2.log).

Run 26 = G2c run 8, 2026-11-19:
- Merged origin/phase-2-graphics-direct3d-11 (20 commits; it held -webgl's, -direct3d-12's
  per-primitive early tests and -shaders' run 37) and origin/phase-2-graphics-shaders (its run
  38, with its adoption of the literal check). -webgl and -direct3d-12 then had nothing left;
  the shared line, -rig and phase-1-literals had nothing new. So every graphics branch is merged
  again. The shaders merge conflicted only in literals.baseline, where the Direct3D 11 side had
  removed 14 of its own lines the shaders side still had: kept neither (docs/literals.md).
  Nothing in this crate, the interface or the suite crate changed; the carried naga's WGSL
  writer now writes member bindings only on entry point inputs and outputs (shaders track).
- Read the new text of the four status files. Nothing new is asked of this back end. The
  Direct3D 11 front notes it does not check the back end's combined uniform-buffer limit for a
  program's two stages (its own check); the shaders track refuses vertex programs writing
  storage views by name (`VertexStorageWrite`: WebGPU's vertex stage has none, so not this back
  end's to build) and lists cull distances and the layered item as its and the fronts' work.
- The literal check first failed on the merged tree (8,386 `+` lines): this track ran it in the
  desktop's own checkout, which is CRLF (core.autocrlf true) and has the shaders crate's nested
  submodules checked out. The shaders and Direct3D 11 tracks' baselines are made in LF trees
  with only dxbc-spirv, dxil-spirv and dxil-spirv's third_party/spirv-headers checked out (the
  shaders track's own instructions), and the merge brought those keys. The check is now run as
  the WebGL track runs it, in an LF worktree of the desktop's repository
  (C:\wc\src\phase-2-graphics-webgpu-lf, How to run), and exits 0 at bf7ddf71 with 0 marks. A
  first attempt at a union of both baselines was wrong (it re-added lines the check counts as
  growth) and was not committed. This crate's baseline lines: 192 (unchanged; the shaders init
  rekeyed packed.rs's three multi-line strings to LF). Changes that could add lines (16): run
  24's twelve, and bd53fbb4 and 949667f2 new rules, 53789ba3 the start, cd5f44a1 the init after
  the baseline arrived by a merge (the shaders track's adoption, merged in).
- Literal sweep: no part this run (no code changed, so every result below is at the merged
  tree).
- Full run at bf7ddf71 (desktop, RTX 3080, NVIDIA 621.01, shared through the GPU queue; scratch
  run26/lint2.log and full-1.log): lint of both builds clean; after `cargo clean` of this crate
  and the suite crate, both APIs: 265 passed, 5 ignored, 0 failed (the ignored tests as in run
  21), the shared suite 44 of 44 on each API. web/check at the same tree (run26/webcheck.log,
  pictures in run26/web) passed whole after cleaning the WebAssembly build: the twelve pictures
  and the viewports equal Direct3D 11's at tolerance 0, the twists differ, the shared suite 44
  of 44 in Chrome 157.0.8103.78 on the RTX 3080 (D3D12, discrete, not a fallback), subgroups,
  the primitive index and fine derivatives declared and drawn; timestamps without developer
  features in 65,536 ns steps, stretches 9.830 and 19.530 ms. Every page got an adapter at its
  first launch, so the relaunch is still not seen working.
- Frame time: bgfx-01-cubes-d3d12 (the Direct3D 12 front changed most) and chocolate-doom again
  (Measurements, run 26).
- Removed at the end: the desktop's C:\wc\target\phase-2-graphics-webgpu (11.47 GB). No build
  folder on the Mac or in scratch (scratch run26: logs and pictures). Added on the desktop: the
  LF worktree (Installed on the desktop).

Run 25 = G2c run 7, 2026-11-19:
- Merged origin/phase-2-graphics-direct3d-11 (3 commits: constant buffers past the back end's
  uniform buffers read through texture copies, more literals named) and -webgl (4: sample
  masks, the frame breakdown's timing), no conflicts; the shared line, -rig and
  phase-1-literals had nothing new. Nothing in this crate, the interface or the suite crate
  changed.
- Not merged, again: -direct3d-12 and -shaders, the same tips as in run 24 (neither has adopted
  the literal check). Both merges were made and the check run on the desktop: it still fails on
  their growth in crates this track may not edit (the shaders track's tests/compute_draw.rs
  `2048`, and as before the Direct3D 12 front's), so the two unpushed merge commits were dropped.
- Read the new text of the four status files. Nothing new is asked of this back end. The
  Direct3D 11 front reads constant buffers past `uniform_buffers_per_stage` through textures;
  this back end already declares the adapter's own limit (it asks wgpu for `adapter.limits()`),
  so there is nothing to raise. The shaders track's two items naming WebGPU (narrower DXIL pixel
  targets, a float4 written and read as float2 between stages) are the converter's, failing
  WebGPU's own pipeline checks, not this back end's code. The Direct3D 12 front's new items are
  its per-primitive early tests.
- Seen in this run's web/check: Chrome 157's adapter on the RTX 3080 now offers
  `texture-component-swizzle` (a view swizzle). wgpu 30 has none, natively or in its browser
  back end, and the interface's views carry no mapping, so programs still apply mappings
  themselves (`SwizzleInShader`). Not asked by any track; under Next.
- Literal sweep, fifth part (4 lines): the frame report wrote its counters by parsing their
  `Debug` text on `{`, `}`, `,` and `:`; it now takes them from serde (`Counters` derives
  `Serialize`, `serde_json::to_value`). serde and serde_json move from dev-dependencies (serde
  came through the interface already; both MIT or Apache-2.0; the lock file gains one line).
  This crate's baseline lines: 196 at the start, 192 now. Left as run 22 to 24 list them; the 48
  failure texts given to `fail` are the check's own "wrongly caught" case and stay.
- Full run at d2f49a3f (desktop, RTX 3080, NVIDIA 621.01, shared through the GPU queue; scratch
  run25/full-1.log): after `cargo clean` of this crate and the suite crate, lint of both builds
  clean, the literal check exits 0, then both APIs: 265 passed, 5 ignored, 0 failed (the ignored
  tests as in run 21), the shared suite 44 of 44 on each API. web/check at the same code
  (run25/webcheck.log, pictures in run25/web) passed whole after cleaning the WebAssembly build:
  the twelve pictures and the viewports equal Direct3D 11's at tolerance 0, the twists differ,
  the shared suite 44 of 44 in Chrome 157.0.8103.78 on the RTX 3080 (D3D12, discrete, not a
  fallback), subgroups, the primitive index and fine derivatives declared and drawn; Chrome's
  timestamps without developer features in 65,536 ns steps, stretches 9.830 and 19.595 ms. Every
  page got an adapter at its first launch, so the relaunch is still not seen working.
- Frame time: chocolate-doom's breakdown repeated (Measurements, run 25); the other two
  programs' breakdowns stand from run 22.
- Removed at the end: the desktop's C:\wc\target\phase-2-graphics-webgpu (11.36 GB) and the
  checkout's own target on the Mac (234 MB of `cargo check`). Scratch run25 holds logs and
  pictures (4.3 MB). Nothing new installed on the desktop.

Run 24 = G2c run 6, 2026-11-18 to 19:
- Merged origin/phase-2-graphics-direct3d-11 (G4 run 24, with its own adoption of the literal
  check) and origin/phase-1-literals (7668c010, the tip the brief names; no new version of the
  rules, so no init). The Direct3D 11 merge conflicted only in literals.baseline, where both sides
  had removed neighbouring lines: kept neither, as docs/literals.md says, and deleted the 7 lines
  the check then printed (this crate's literals fixed in run 23 that the other side's baseline
  still had). Nothing in this crate, the interface, the suite crate or the lock file changed.
- Not merged: origin/phase-2-graphics-direct3d-12 (2 commits) and -shaders (5). Neither branch
  has adopted the literal check, and each adds literals after its merge base that the check
  counts as growth (`+`) in crates this track may not edit and may not add to the baseline: the
  Direct3D 12 front's device/early.rs (three `3`s) and examples/check/cases.rs (`SIDE = 4`, a
  redefinition of this crate's blocks.rs `SIDE`), and the shaders track's tests/compute_draw.rs
  (`vec![0; 2048]`). Both merges were made, the check run (it failed on exactly these lines), and
  the two unpushed merge commits dropped. Nothing in either is needed here (Departures). Their
  status files were read from origin all the same.
- Read the new text of the four status files. Nothing new is asked of this back end: the
  Direct3D 11 front found the stencil views reading 0 were its own (list above) and asks the
  interface, not this back end, for a `B8G8R8X8_UNORM` vertex format with the fourth component
  1.0; the Direct3D 12 front's new commits are its per-primitive early tests and storage offsets;
  the shaders track lists native 16-bit floats "through a capability", which this back end
  already declares (`SHADER_F16` where wgpu has it); the WebGL track had nothing new.
- Literal sweep, fourth part (18 lines): the draw cache keeps the interface's `Viewport` and
  `ScissorRect` (was `[u32; 6]` and `[u32; 4]`), clear colours are destructured; format sample
  counts come from wgpu's `supported_sample_counts`; a sampler's three address modes are
  destructured and its largest level is WebGPU's default `lod_max_clamp`, not 32.0; a spare
  texture's key holds its `Extent3d`; the arena's growth names `ARENA_SLICES_AT_LEAST`; the
  vertex conversion's operations are a `Code` enum whose names the program's text defines (its
  `switch` became an `if` chain on those constants), a typed `Operation`, and source bytes derived
  from wgpu's single and four-component formats (`three_of`) or a word. New unit tests in
  vertex.rs: `conversion_program_is_valid` (naga parses and validates the program with its codes)
  and `lacking_formats_take_their_bytes_in_the_source`. This crate's baseline lines: 214 at the
  start, 196 now. The check's list of changes that could add lines (12): run 23's ten, and
  81489a2f the start and 5657abb6 the init after the baseline arrived by a merge (the Direct3D 11
  track's adoption, merged in).
- Full run at 3b3aa3bb (desktop, RTX 3080, NVIDIA 621.01, shared through the GPU queue; scratch
  run24/full-1.log): lint of both builds clean, the literal check exits 0, then both APIs: 265
  passed, 5 ignored, 0 failed in all (Vulkan and Direct3D 12 together; the two new unit tests on
  each, the ignored tests as in run 21), the shared suite 44 of 44 on each API. web/check at
  0f06945d (the status commit after 3b3aa3bb, same code; scratch run24/webcheck.log,
  pictures in run24/web) passed whole: the twelve pictures and the viewports equal Direct3D 11's
  at tolerance 0, the twists differ, vertex formats through shadows (seed 188, 64 vertices each)
  as they mean in Chrome, the shared suite 44 of 44 in Chrome 157.0.8103.78 on the RTX 3080
  (D3D12, discrete, not a fallback), subgroups, the primitive index and fine derivatives declared
  and drawn; Chrome's timestamps without developer features were multiples of 65,536 ns again
  (131,072 in run 23), stretches 9.830 and 19.595 ms. Every page got an adapter at its first
  launch, so the relaunch is still not seen working.
- Frame time: the breakdowns stand (Measurements, runs 22 and 23); nothing in this run changes a
  frame's path but the vertex conversion's parameters, which are the same words as before.
- Removed at the end: the desktop's C:\wc\target\phase-2-graphics-webgpu (8.99 GB) and the
  checkout's own target on the Mac (510 MB of `cargo check`). Scratch holds logs and pictures
  (401 MB). Nothing new installed on the desktop.

Run 23 = G2c run 5, 2026-11-18:
- Merged origin/phase-2-graphics-shaders, -direct3d-11, -direct3d-12 and -webgl (the moves left
  over from run 22), and origin/phase-1-literals (tip 629694af, version 7 of the rules), no
  conflicts; the shared line and -rig had nothing new. Read the five status files' new text: the
  one item naming a back end is the Direct3D 12 front's refusal of a read-only structured view at
  an offset off the storage alignment, which is the converter's (`StorageOffset` for shader
  resource slots, the shaders track's `variant/offset.rs`), not this back end's. Nothing new is
  asked of this back end.
- The literal check is adopted. -webgl had already adopted it (its baseline start 1e337706), so
  its merge brought the baseline; the literals merge was committed (e7cdf789) and its init run in
  the commit right after it (2cc8ad76), as docs/literals.md step 4 says for a merge bringing new
  rules. Init proposed no `generated` lines. This crate's baseline lines: 239 at adoption (16 of
  them new under rules 5 to 7: restated values in tests, three WGSL texts in packed.rs, four
  lines of frames/run), 214 after this run's sweep (25 fixed; a 26th line, -webgl's `WINDOW` in its tests/suite.rs, left the baseline because this crate's same-named copy was renamed). The check's list of changes that
  could add lines, at 2cc8ad76 (10): 4bcacbf5 the start (the -webgl merge), 2cc8ad76 the init
  after the baseline arrived by a merge, c33381de the start, 1e337706 the init after the baseline
  arrived by a merge, 68214b2e the start, and 89f88281, 7119e3f3, c3bf56d4, 31ec396f, 0f187563 new
  rules, for literals already in the tree (the literals branch's own).
- Literal sweep, third part: 24-bit unorm depth's largest value is public (`UNORM24_MAX`) and
  tests/copies.rs imports it instead of `0xff_ffff` and `16_777_215.0`; that test's float depth
  words are written as `f32::to_bits` of their values (its old `0x3dfc_d6ea` was not 0.1234 as its
  comment said; now it is); the query scene and tests/writes.rs take WebGPU's set size from
  `wgpu::QUERY_SET_MAX_QUERIES` and the last query from the set's size; the texture bit of a
  layout key comes from one `layout::texture_bit` (from `FIRST_TEXTURE_BINDING` and `u128::BITS`)
  in its three places; test constants that shared another crate's name and value got precise
  names (`NATIVE_WINDOW_HANDLE`, `CANVAS_WINDOW_HANDLE`, `WINDOW_SIDE`, `TEXTURE_SIDE`, `TARGET_WIDTH`,
  `INSTANCES_PER_DRAW`), and the instance scene's target side is the reference scenes' width
  (`TARGET_SIDE = scene::WIDTH`). Left in the baseline: the failure texts passed to `fail` (the check's
  doc lists messages given to a crate's own helper as wrongly caught), test sizes that happen to
  equal a constant (256, 512, 1024, 2048 as texture sides and offsets; "none" as the scenes'
  twist word, equal to `NO_FRAGMENT`), the channel and component counts of draw.rs, vertex.rs,
  fill.rs and lib.rs, packed.rs's WGSL, and the scripts.
- Full runs (desktop, RTX 3080, NVIDIA 621.01, shared through the GPU queue; scratch run23/):
  at the merged tree 2cc8ad76 (full-1.log) and again after the sweep at b3ca1b92 (full-2.log,
  with lint of both builds and the literal check, which exits 0): natively Vulkan 130 passed and
  3 ignored, Direct3D 12 131 passed and 2 ignored, 0 failed, the shared suite 44 of 44 on each
  API (the ignored tests as in run 21). web/check at b3ca1b92 (webcheck.log, pictures in
  run23/web) passed whole: the twelve pictures and the viewports equal Direct3D 11's at
  tolerance 0, the twists differ, the shared suite 44 of 44 in Chrome 157.0.8103.78 on the RTX
  3080 (D3D12, discrete, not a fallback), subgroups declared and drawn. Chrome's timestamps
  without developer features were multiples of 131,072 ns this time (65,536 ns in runs 19 to
  22), so a browser's timestamp step is not fixed; the check's 10 and 20 ms stretches still read
  9.830 and 19.530 ms. Every page got an adapter at its first launch, so Chrome's relaunch is
  still not seen working.
- Frame time: chocolate-doom's breakdown repeated after the merges (Measurements, run 23); the
  other two programs' breakdowns stand from run 22.
- After this run's tests, -direct3d-11, -direct3d-12, -shaders and -phase-1-literals moved (9, 2,
  5 and 4 commits; the literals branch changes its rules.rs, so its merge needs `init` before
  committing it or right after). Not merged: every result above is at b3ca1b92 (the later
  commits change only frames/run, the baseline and this file).
- Removed at the end: the desktop's C:\wc\target\phase-2-graphics-webgpu (11.37 GB). No build
  folder was made on the Mac or in scratch (scratch holds logs and pictures, 396 MB). Nothing new
  installed on the desktop.

Run 22 = G2c run 4, 2026-11-18:
- Merged origin/phase-2-graphics-direct3d-11, -shaders and -webgl (the commits left over from run
  21) and -direct3d-12 (run 31 of G5), no conflicts; the shared line and -rig had nothing new.
  None of them changes this crate or the interface in a way that needs work here. Read the four
  fronts' and the shared status files again: the one new ask was fine derivatives (above). The
  Direct3D 11 front still lists test_depth_stencil_sampling as this back end's; the new test
  above reads every stencil value through the view as WebGPU defines, so the remaining 0 comes
  from the front's side (Next). The Direct3D 12 front lists nothing outstanding here.
- Load within each recorded program run: frames/run now starts `typeperf` beside each recorded
  run and prints the whole machine's mean, lowest and highest load over the job's own span (from
  gpu-job's start and end times), and runs each program alone in the queue (`--alone`).
- Literal sweep, second part. Replaced: the BC decoder's 4 by 4 block and pitches (`SIDE`,
  `BLOCK_TEXELS`, `RGB`, the pitch from the decoded format's texel size); every entry point named
  as a string in the back end's own programs (fill.rs, probe.rs, memory.rs, edge.rs, vertex.rs:
  each of those programs has one entry point a stage, so the pipelines pass none and wgpu takes
  it; the empty fragment stage's name is one `NO_FRAGMENT`, used in its text and by draw.rs); the
  covering triangle's vertices (`COVER_VERTICES`); the stencil draw's eight bits and its clear
  instance (`STENCIL_BITS` from `u8::BITS`, `STENCIL_CLEAR`, also put into the program's text);
  24-bit and 16-bit unorm depth's largest values in the depth programs (`UNORM24_MAX` from
  `UNORM24_BITS`, `u16::MAX`); the depth probe's depth and tolerance (`PROBE_DEPTH`,
  `STEP_TOLERANCE`, put into its program's text); the query ring's replace flag (`REPLACE`, the
  top bit from `u32::BITS`, and its mask, put into the program's text); the uniform's four words
  (`Words`). Left: shader text in Rust strings beyond those (the WGSL of edge.rs, vertex.rs and
  packed.rs: shifts, masks and byte widths), component counts (vertex.rs `widened` and
  `source_size`, lib.rs's sample counts), the packed formats' bit table (packed.rs `fields`, kept
  as the one home of that fact), the scripts (web/*.mjs, web/check.cmd, frames/run). The
  automatic check: docs/literals.md (origin/phase-1-literals) says a track adopts it by merging
  main once main has it, never the literals branch directly; main does not have it yet, and this
  task allows merging only the graphics branches, so it is not adopted here (Departures).
- Wine's test_depth_stencil_sampling through the Direct3D 11 front on this branch (the front's
  debug wcontainers_directx.dll built from this tree, `tools/graphics-rig/wine-tests run --dll
  ...`, RTX 3080, hazard group): 95 checks, 8 failing, the same as the front reports. Every
  failure is the stencil reading 0: alone after stencil clears to 100 and 255 (0 read), and beside
  depth after clears of depth 0.3 or 1.0 with stencil 3 (0.3 and 1.0 read where 3.3 and 4.0 are
  expected); every depth check passes. The front printed "view component mapping applied in the
  program". The back end's own test of the same reads (above) passes on both APIs, so the 0 comes
  from the front's side: the converted program reading the view's second component without the
  move, or the front's stencil clear not reaching the texture (Next). The rig's runner then
  stopped with "You cannot call a method on a null-valued expression" (exit 2) after the test had
  written its log (scratch graphics-rig/wine-logs/phase-2-graphics-webgpu); the Direct3D 11
  track runs Wine's tests through its own fixed copy of the runner for that reason.
- web/check's shared suite and plain timestamp pages now launch Chrome again when it gives no
  adapter, as the drawing page did (`NoAdapter`, `exitCode` and `adapter` in web/chrome.mjs,
  `:chrome` in check.cmd). This run's first web/check failed so: the suite page and the plain
  timestamp page each got no adapter after the drawing page had drawn. The relaunch itself was
  not needed in the second web/check (every page got an adapter at its first launch), so it is
  not yet seen working.
- Full runs at 62473782 (desktop, RTX 3080, NVIDIA 621.01, shared through the GPU queue; scratch
  logs/run22-clippy2.log, run22-full2.log, run22-webcheck2.log, pictures in run22/web2): lint of
  both builds clean; natively Vulkan 130 passed and 3 ignored, Direct3D 12 131 passed and 2
  ignored (the ignored tests as in run 21), 0 failed; the shared suite 44 of 44 on each API.
  web/check passed whole: the twelve pictures and the viewports equal Direct3D 11's at tolerance
  0, the twists differ, the shared suite 44 of 44 in Chrome 157 on the RTX 3080 (D3D12, discrete,
  not a fallback), subgroups, the primitive index and fine derivatives declared and drawn in
  Chrome, timestamps without developer features in 65,536 ns steps.
- Frame time: Measurements, run 22 (chocolate-doom, atg-simpleinstancingpc and
  bgfx-01-cubes-d3d12 again at 62473782, each alone, with the load of each recorded run).
- A first try at the frame breakdown in this run failed: the web scripts were edited while it
  ran, so every program job found the desktop holding another tree and refused (logs/
  run22-frames.log). Nothing may change in the checkout while rig jobs run.
- After this run's tests, -direct3d-11, -direct3d-12, -shaders and -webgl moved (9, 6, 13 and
  364 commits). -webgl's now carry about 350 commits from outside the graphics tracks (the
  translator's and others', merged there) and one edit to this crate's examples/frames.rs
  (071cbbfc, the WebGL back end's and ANGLE's libraries in the breakdown). None is merged here:
  every result above is at 62473782, and a merge would need both full runs and web/check again.
- Removed at the end: the scratch worktree for the literal edits and its 580 MB `cargo check`
  folder, the checkout's own `target` (578 MB, left by run 22's first part), and the desktop's
  C:\wc\target\phase-2-graphics-webgpu (20.02 GB: debug, release and WebAssembly builds, the
  front's library among them). Scratch holds logs, pictures and load samples (about 390 MB).
  Nothing new installed on the desktop.

Run 21 = G2c run 3, 2026-11-18:
- Merged origin/phase-2-graphics (G1c run 9: `STORAGE_WORD`, every binding of the kind the program
  declares, buffers a storage binding reaches the end of in whole words; the newest -webgl,
  -direct3d-11 and -shaders commits with it) and origin/phase-2-graphics-shaders (run 33), no
  conflicts; -direct3d-12 and -rig had nothing new. The interface changes need nothing here (this
  back end already takes its layouts from the program); `STORAGE_WORD` is now used for the words
  of storage bindings. Read the four front and the shared status files again: the one ask of
  this back end not yet built was the shaders track's primitive index (below); the Direct3D 12
  front still lists subgroups (built in run 20, absent on its FXC instance); nothing else new.
- The second G2c check's findings:
  - Primitive index: built (list above), natively and in Chrome.
  - Frame breakdown with counters and load: the back end counts the bytes written into buffers
    and textures (`Counters::buffer_bytes`, `texture_bytes`) and prints its counters and slow
    paths at each power of two frames presented (`FRAME_REPORT`); examples/frames.rs `--log`
    reads them a frame from the program's log; the desktop's load was sampled each second by
    `typeperf` (Processor(_Total)) during the runs. Measurements, run 21.
  - bgfx's NVAPI: recorded under Measurements.
  - Direct3D 12 texture reads at offsets off its placement alignment overwrote the bytes
    between rows (wgpu-hal's own intermediate buffer, copied back whole): fixed by going through
    `RowsRepacked` there (Departures). examples/scenes/gaps.rs (tests/copies.rs
    `texture_reads_into_a_buffer_keep_the_bytes_between_rows`, and web/check in Chrome): four
    formats (two native, one converted, one three-channel), two layers, at (256, 512) and (0, 256),
    every copied byte equal to the texture's and every other byte kept. Passes on Vulkan, Direct3D
    12 and in Chrome. Not run with the fix taken out; the check's own diagnosis (own-padding.log)
    showed 1296 to 1469 bytes changed on Direct3D 12 before.
  - Three-channel textures wider than a third of the widest texture: their copies now go in runs
    (`raw_run`), each starting on a Direct3D 12 placement and kept a placement short of the widest
    texture. converted.rs `three_channels_at_widest`: each three-channel format at the device's
    widest 2D texture, buffer to texture to texture to buffer, every bit kept: 32768 texels on
    Vulkan, 16384 on Direct3D 12 and in Chrome. The first try (runs only as wide as fit) passed
    natively but lost the device in Chrome (Dawn: "D3D12 closing pending command list failed with
    E_INVALIDARG"), which starting each run on a placement and leaving a placement of room fixed
    (logs/run21-webcheck.log, then -webcheck2.log). Why Dawn failed is inferred (it moves an
    unplaced copy back to a placed offset, widening it past the widest texture), not read in
    Dawn's code.
  - web/check's first Chrome launch: draw.mjs exits with `NO_ADAPTER` when Chrome gives no
    adapter and check.cmd launches it again (up to 3). Seen working in this run's first web/check
    ("chrome: no WebGPU adapter", then the second launch drew everything).
  - Subgroups in the browser: the vendored wgpu now maps `subgroups` and `primitive-index`;
    Chrome 157 on the RTX 3080 offers both, and both are declared and drawn there.
  - pdb's licences: recorded (Licences).
  - Magic literals: the sweep's first part (below).
- Literal sweep (the brief's rule; the audit's list for this crate is in
  orchestrator/magic-literals/fix-graphics.md, 93 entries for wcontainers-gpu-webgpu in the
  categories read). Replaced: every WebGPU copy, query, immediate and vertex alignment written as
  4 or 8 (`wgpu::COPY_BUFFER_ALIGNMENT`, `QUERY_SIZE`, `IMMEDIATE_DATA_ALIGNMENT`,
  `VERTEX_ALIGNMENT`, the interface's `STORAGE_WORD` for storage words), the anisotropy limit (one
  `MAX_ANISOTROPY`), integer limits (`u8::MAX`, `i8::MAX`, `u16::MAX`, `i16::MAX`,
  `f16::ONE`, `f32::MANTISSA_DIGITS`), the compute programs' workgroup size (one `GROUP`, put into
  the programs' text), the pruning threshold, sizes of the probe's and fill's buffers (from their
  types), and the tests' duplicated `W`, `H` and `COLORED_WGSL` (one definition each). New code
  this run names its numbers. Left: BC block arithmetic in blocks.rs (4 by 4 texels and the
  decoder's pitches; `Format::block` is not a `const fn`, so a named constant there needs care),
  the packed formats' bit table (packed.rs `fields`, the one home of that fact, kept), the query
  ring's replace flag `1 << 31` (shared with its program's text), shader text in Rust strings,
  the remaining categories of the list (component counts, per-format facts, names and paths),
  and the scripts. The automatic check (branch phase-1-literals) is not on any branch this one
  merges; not merged here (the task names only the graphics branches).
- Full runs at fb32cf1f (desktop, RTX 3080, NVIDIA 621.01, shared through the GPU queue; scratch
  logs/run21-clippy2.log, run21-full2.log, run21-webcheck2.log, pictures in run21/web): lint of
  both builds clean; natively Vulkan 127 passed and 3 ignored, Direct3D 12 128 passed and 2
  ignored (the same ignored tests as run 20), 0 failed; the shared suite 44 of 44 on each API.
  web/check passed whole: the twelve pictures and the viewports equal Direct3D 11's at tolerance
  0, the twists differ, the shared suite 44 of 44 in Chrome 157 on the RTX 3080 (D3D12, discrete,
  not a fallback), subgroups and the primitive index declared and drawn in Chrome, texture reads
  into buffers and three-channel copies at 16384 texels exact in Chrome, timestamps without
  developer features in 65,536 ns steps. e54cf4cb after it changes only examples/frames.rs.
- Frame time: Measurements, run 21.
- After this run's merges, -direct3d-11, -shaders and -webgl moved again (8, 3 and 11 commits,
  among them interface and shared suite changes, and a two-line change to this crate's
  Cargo.toml from -direct3d-11): not merged in this run, so the tests above are on this run's
  merge; the next run merges them first and reruns both full runs.
- Removed at the end: the desktop's C:\wc\target\phase-2-graphics-webgpu (16.53 GB, debug and
  release) and the checkout's own target on the Mac (1.4 GB of `cargo check`); scratch holds logs,
  pictures and load samples only (383 MB). Nothing new installed on the desktop (`typeperf` is
  Windows' own).

Run 20 = G2c run 2, 2026-11-18:
- Merged origin/phase-2-graphics (G1c: the ruling on depth and stencil copies within one
  subresource, prepare naming no index format, no vertex format capability, three new shared
  suite tests); it carried the newest -rig, -shaders, -direct3d-11, -direct3d-12 and -webgl
  commits, so nothing else was left to merge. No conflicts. The three new suite tests were added
  to examples/scenes/names.rs, so Chrome runs them too.
- The G2b check's open findings were all closed in run 19 (above); the strip index field is now
  the interface's own ruling (no index format in `PrepareDesc`; strips prepared for both).
- Built: the ruling on depth and stencil copies within one subresource; subgroups. Answered with
  a test: stencil views on wgpu's Direct3D 12 path. Each is in the list above. Read again for
  new asks: the Direct3D 11 front's empty-slot `resinfo` sizes (its own stand-in texture, its
  item), its multisampled arrays as a texture per layer (refused by that front for now; plan 4.2's
  texture per layer is built in the fronts, as the Direct3D 12 front has done), the shaders track's 24-bit early
  tests (its own program), the WebGL track's replays (they replay on this back end unchanged).
  Nothing else is asked of this back end.
- Full runs (desktop, RTX 3080, NVIDIA 621.01, shared with the other tracks through the GPU
  queue, CPU load not sampled; scratch logs/run20-full.log, run20-subgroups.log,
  run20-clippy.log, run20-webcheck.log, pictures in run20/web): lint of both builds clean;
  natively after `cargo clean` of this crate and the suite crate, Vulkan 121 passed and 3
  ignored, Direct3D 12 122 passed and 2 ignored (the same ignored tests as run 19), 0 failed;
  the shared suite 42 of 42 on each API. Subgroups: the adapter has them on Vulkan (declared,
  drawn), not on Direct3D 12 (declared absent). web/check after cleaning the WebAssembly build:
  the twelve pictures and the viewports equal Direct3D 11's at tolerance 0, the twists differ,
  the shared suite 42 of 42 in Chrome 157 on the RTX 3080 (D3D12, discrete, not a fallback),
  timestamps without developer features in steps of 65,536 ns (9.830 and 19.530 ms), the other
  Chrome checks as in run 19.
- Then the shared line moved again (G1c run 7: what an indexed draw may fetch past a stream and
  a program past its binding, two new suite tests) and -direct3d-12 and -shaders with it: merged
  (one conflict, in the plan's text on fetches past a vertex buffer: both texts kept, the Direct3D
  12 track's indirect draws sentence inside the shared line's paragraph). The new suite tests went
  into examples/scenes/names.rs. Its newest asks of this back end are subgroups (built above) and
  the two suite tests, which pass. Runs after it (logs/run20-full2.log, run20-clippy2.log,
  run20-webcheck2.log, run20-plain.log, run20-webcheck3.log): lint clean; Vulkan 123 passed and
  3 ignored, Direct3D 12 124 passed and 2 ignored, 0 failed; the shared suite 44 of 44 on each
  API and 44 of 44 in Chrome. The first web/check after the merge failed at one step: Chrome
  launched without developer features got no adapter ("no adapter"; the G2b check met the same
  once). Run alone three times right after, it passed 3 of 3 (stretches 9.765 and 19.5 ms, every
  value a multiple of 65,536 ns), and web/check again passed whole (pictures at tolerance 0, the
  suite 44 of 44 in Chrome). The adapter miss is Chrome's, seen twice in about ten launches.
- Frame time: the breakdowns of run 19 stand (Measurements); nothing in this run changes a
  frame's path but the device asking for subgroups. The two large costs are now placed: the
  bgfx copy is the Direct3D 12 front's `MappedUploadWhole` (asked of it under Next), the acquire
  spin is the display wait itself (Next).
- A third merge of the shared line (G1c run 8: an interface doc change, a storage binding's size
  a multiple of four and an access past a binding may reach buffers sharing its API buffer, as
  this back end's arena does; no new suite test) took the shared line's plan text where it
  conflicted (its wording of the same indirect draws sentence). Runs after it
  (logs/run20-full3.log, run20-clippy3.log, run20-webcheck4.log): lint clean; Vulkan 123 passed
  and 3 ignored, Direct3D 12 124 passed and 2 ignored, 0 failed, the suite 44 of 44 on each;
  web/check passed whole (pictures at tolerance 0, the suite 44 of 44 in Chrome, timestamps in
  65,536 ns steps).
- Removed: the desktop's C:\wc\target\phase-2-graphics-webgpu three times (5.96, 8.53 and
  8.53 GB) and the checkout's own target on the Mac three times (505, 508 and 508 MB of
  `cargo check`); no scratch copy was made.

Run 19 = G2c run 1, 2026-11-18:
- Merged origin/phase-2-graphics-rig, -shaders, -direct3d-12 and -direct3d-11 (no conflicts; the
  shared line and -webgl had nothing new). The merged lock file lacked the Direct3D 12 manifest's
  `blake3`, so `--locked` builds failed; brought up to date in its own commit. Interface changes
  they brought (`VariantKey::texture_layers` and `swizzled`, `AddedBinding::TextureLayer`,
  `HelperValue::Layer` and `Viewport`, `Reflection::slow_paths`, three slow paths) need nothing
  here: the back end lays out added bindings from the reflection as before, and declares no
  `VIEW_SWIZZLE` (wgpu 30 has none), so programs apply mappings themselves.
- The interface track's ruling on copies within one subresource of depth or stencil has not
  landed on origin/phase-2-graphics (no new commits there); such a copy still marks the device
  lost by name ("a copy within one subresource of depth or stencil").
- Read the front, shaders and shared status files for what they ask of this back end. New: the
  Direct3D 11 front's `R32G32B32_*` textures (built, list above), and "the division's precision"
  in atg-simplecomputepc, asked of the shaders or this track: refused here, not this back end's
  code. The back end hands wgpu the converter's program; the division is the converter's
  instruction compiled by the compiler the host's wgpu instance picks (the Direct3D 11 library
  makes a Direct3D 12 instance, so FXC, whose output Direct3D 12 converts again), so which step
  rounds can be found by running the sample with an instance on Vulkan or with wgpu's DXC
  (`Dx12Compiler`), the host's choice (asked of the Direct3D 11 track under Next). Everything
  else in the lists is answered above or is others' work. The Direct3D 12 front's gap list asks
  nothing new of this back end; the shaders track's and the WebGL track's files neither.
- The G2b check's findings: Chrome's developer features flag is now described as also turning
  off timestamp rounding, and web/plain.mjs (run by web/check) times them in Chrome as users have
  it: 65,536 ns steps (recorded in the hand-over); `timestamps_are_absent_on_a_device_without_timestamp_queries`
  runs the absent branch (a device made without `TIMESTAMP_QUERY`: `TIMESTAMPS` undeclared,
  timestamp sets and `timestamps_disjoint` refused by name); the strip index field is refused
  with its reason (Departures); "storage usage costs nothing" is now said to be expected, not
  measured; the native loop bounding cost stays under Next for the owner.

- Full runs at the end (desktop, RTX 3080, shared through the GPU queue; scratch
  logs/run19-full.log and logs/run19-webcheck-final.log): lint of both builds clean; natively,
  Vulkan 116 passed and 3 ignored (the same three as run 18), Direct3D 12 117 passed and 2
  ignored, 0 failed; the shared suite 39 of 39 on each API. web/check from clean: the twelve
  pictures and the viewports equal Direct3D 11's at tolerance 0, the twists differ, the shared
  suite 39 of 39 in Chrome 157 on the RTX 3080, the three-channel formats every bit kept in
  Chrome, timestamps in Chrome without developer features in steps of 65,536 ns, the other
  Chrome checks as in run 18.
- Frame time of real programs through the fronts: Measurements, run 19.
- Removed at the end: the desktop's C:\wc\target\phase-2-graphics-webgpu (11.2 GB, debug and
  release), the program copies under C:\wc\out\phase-2-graphics-webgpu\frames (the profiles,
  3.15 GB, stay as evidence), and the checkout's own target on the Mac (833 MB of `cargo check`).

Run 18 = G2b run 8, 2026-11-18 (fixes from G2b's third independent check):
- Merged origin/phase-2-graphics-rig, -shaders, -direct3d-12, -direct3d-11 and -webgl (no
  conflicts; the shared line had nothing new). Interface changes they brought: `COUNTER_BINDING`
  and `SlowPath::PixelInvocationsCounted` (answered in the list above) and the shared
  `OcclusionCounter` summing on the GPU. Read the four front status files' new text: the new asks
  of this back end are the stencil view swizzle and the counter binding (list above); the
  Direct3D 11 gap list's other two items, BGRA vertices and R16/D16 copies, are in the list too.
- Fixes, each with its test (all listed above): BGRX vertices answered and compared with
  Direct3D 11; no integer format declared resolvable; part of a depth level copied into colour by a
  draw; `r16float`, `r16snorm` and `r16sint` copied into `depth16unorm`; prepared strips built for
  both index formats. The three back end fixes' tests were run with the source fixes taken out:
  each failed (the 16-bit copies lost the device on wgpu's validation, the strips missed, the
  integer formats were declared resolvable on Vulkan).
- Full runs at 39252f02 on the desktop (RTX 3080, shared with the other tracks through the GPU
  queue; logs in scratch logs/run18-full.log and logs/run18-webcheck.log): natively, Vulkan 112
  passed and 3 ignored (the known BC decoder difference, the wireframe on Vulkan, point filling
  absent), Direct3D 12 113 passed and 2 ignored (the BC difference, 24-bit depth bias absent), 0
  failed; the shared suite 39 of 39 on each API. web/check: the twelve pictures equal Direct3D 11's
  at tolerance 0, the twists differ, the shared suite 39 of 39 in Chrome 157 on the RTX 3080
  (Direct3D 12 underneath), and in Chrome: prepared strips 0 misses, BGRA vertices as they mean,
  the 16-bit group copies kept the bits for `r16uint`, `r16float` and `r16sint` (Chrome has no
  16-bit normalised textures), timestamps, converted formats, vertex formats, renames and query
  sets as before.
- Removed at the end: the desktop's build folder C:\wc\target\phase-2-graphics-webgpu (5.86 GB);
  no scratch copy or Mac build output was made.

Run 17 = G2b run 7, 2026-11-18 (fixes from G2b's second independent check):
- Merged origin/phase-2-graphics-rig, -shaders, -direct3d-12, -direct3d-11 and -webgl (no
  conflicts; the shared line had nothing new). The one interface change (`ReflectedBinding::slot`,
  from the shaders branch) needs nothing here. Read the four front status files' new text: nothing
  new is asked of this back end. The Direct3D 11 front lists test_early_depth_stencil as WebGPU
  having no forced early tests with storage writes, waiting on the shaders track's
  `EarlyTestsInShader`; the Direct3D 12 front's bgfx frames match, its GPU-time line differing
  because the rig's library does not fix timestamps by the frame clock on Direct3D 12 (not a gap
  here); the WebGL track's one Chrome tie stays laid on G1's mip helper.
- Query sets of more than WebGPU's 4096 queries (Direct3D 12 heaps may be larger; the check found
  a set of 5000 lost the device): paged over WebGPU sets of 4096, query i in slot i % 4096 of page
  i / 4096, the last own page going on with the occlusion spare slots and further spare pages
  after it. Timestamps write on their own page. A pass part counts on one page; an occlusion
  query begun on another page than its own takes a spare slot there (or the part moves to a page
  with room, `QueriesSegmented`), and resolving replaces its own slot's count, as for carried
  queries. Results crossing a page boundary are resolved page by page into 256-byte aligned places
  and copied out in parts. examples/scenes/queries.rs `paged_sets` (tests/queries.rs natively,
  web/check in Chrome): a set of 5000 with queries on both pages, one begun off its page, one
  carried across a split, resolved across the boundary at an offset and read; a timestamp set of
  5000 written on both pages and read in order.
- Query results come from one ring (`Counters::query_rings`), taken from its start after each
  submit, as staged bytes come from the staging ring (was: a new GPU buffer per resolve or read,
  and another for carried queries' spare slots, which now take a region of the staging ring).
  `results_ring`: 20 frames resolving and reading a query that draws on even frames only, every
  result right, one ring made.
- tests/raster.rs `point_fill_draws_the_corners` reports itself ignored with its reason where
  `POLYGON_MODE_POINT` is absent (was: passed having checked nothing there).
- Status: the instance step rate summary now says that a set made per draw (the Direct3D 11
  front's route) costs clearly more CPU than vertex buffers up to 8 streams; the hand-over no
  longer says converted-format copies wait on the device.

Run 16 = G2b run 6, 2026-11-17 to 18:
- Merged origin/phase-2-graphics-webgl, -direct3d-11, -direct3d-12 and -shaders (no conflicts;
  the shared line and -rig had nothing new). The one interface change (from the shaders branch:
  `DOMAIN_PATCHES_BINDING`, `DOMAIN_POINTS_BINDING`, a hull stage's `point_locations` and
  `constant_locations`) needs nothing here. Read the four front status files' new text: nothing
  new is asked of this back end. The Direct3D 11 front's new routes (typed views, typed buffers
  through storage) and the Direct3D 12 front's (filtering in the program where the back end cannot
  filter, multisampled arrays refused at creation) use what is declared; the WebGL track's new
  comparisons with this back end pass natively, with one tie in Chrome that it lays on G1's mip
  helper; the shaders track's tessellator program fails fxc (wgpu's Direct3D 12 compiler), its
  own known gap, and compiles in Chrome.
- The cause of the native timestamp workload's 1.3 and 1.5 times Direct3D 11's time, measured
  (Measurements, run 16): naga's loop bounding (`ShaderRuntimeChecks::force_loop_bounding`), which
  wgpu adds to every loop natively. Without it both APIs take Direct3D 11's time; without bounds
  checks nothing changes. Not changed: wgpu says not to turn it off for untrusted programs, and a
  guest's are untrusted (left to the owner). In Chrome wgpu passes WGSL on and Dawn's own handling
  costs nothing measurable (9.82 ms, Direct3D 11's time).

Run 15 = G2b run 5, 2026-11-17:
- Merged origin/phase-2-graphics-direct3d-12, -shaders and -rig (no conflicts; the shared line,
  -webgl and -direct3d-11 had nothing new). They changed nothing in the interface, this crate,
  the suite crate or the lock file. Read what is new in the Direct3D 12 and shaders status files:
  nothing new is asked of this back end. The Direct3D 12 front lists "naga's validation lacking
  `STORAGE_TEXTURE_16BIT_NORM_FORMATS` where the WebGPU back end reports those formats as stored
  into" under the converter (G6): checked here, the back end declares storage for 16-bit
  normalised formats only on a device with wgpu's `TEXTURE_FORMAT_16BIT_NORM`, and its own
  validator (`wgpu_naga_bridge::create_validator` with the device's features) then has that
  capability, so the gap is the converter's validation, not this back end's.
- Copies of converted formats run on the GPU (were: read back, waited for, and written again
  through the host, `HostCopyReadBack`, a pause for every Direct3D 12 upload of such a texture).
  A copy goes through a texture of the format's raw blocks (`r8uint`, `r16uint` or `r32uint`, one
  texel a block, `packed::raw`): from a buffer, the bytes are copied into it and a draw a slice
  widens them into the texture; into a buffer, a draw packs the texels into it and it is copied
  out; between a converted format and another of equal blocks, packed into it or copied from the
  other, then widened from it or copied into the other. The draws' programs
  (`packed::program`) do in WGSL what `widen` and `pack` do on the host (bits repeated; nearest
  value of the bits), sRGB BGRX through its linear twin, 3D textures slice by slice. Converted
  textures are made with render and binding usage for this. Counted as `FormatConverted`.
- Tests that fail without it: examples/scenes/converted.rs `copies` now fails if a copy with a
  buffer or between textures counts `HostCopyReadBack`, copies into and back from a texture of
  another format of equal blocks (`R8Uint`, `R16Uint`, `R32Uint`, the other two-pixel format),
  and `volume_copies` copies through buffers into and out of slices 1 and 2 of a 3D texture of
  three (slice 0 and the buffer outside them untouched). Run natively by tests/converted.rs and
  tests/direct3d.rs, in Chrome by web/check. The unit test `copy_programs_are_valid` parses and
  validates every program with naga.

Run 14 = G2b run 4, 2026-11-17:
- Merged origin/phase-2-graphics-webgl, -shaders and -direct3d-11 (no conflicts; the shared line,
  -direct3d-12 and -rig had nothing new). The one interface change they brought (4a72bce0, the
  scissor text of `FRACTIONAL_VIEWPORT`) needs nothing here: the bit is declared. Read the four
  front status files' new text: nothing new is asked of this back end (the WebGL track compares
  with it in Chrome; the shaders track's storage clears run on it).
- The instance data measurement moved into examples/scenes/pulled.rs (tests/pulled.rs calls it,
  web/check runs it in Chrome) and takes the streams a draw (`WEBGPU_STREAMS`, default 4, 8 and the
  device's most up to 16, WebGPU's guaranteed attributes): stream 0 the quad, 1 where, 2 how large,
  the rest colour parts. Three routes: vertex buffers, the program fetching with a binding set made
  per draw (as the Direct3D 11 front makes sets when bindings change), and with sets kept. Each
  stream's fetch is its own function reading its own helper words, as a front's variant.
- Found by it: a front's sets made again each frame made their bind groups again each frame (5120
  in 20 frames of 256 draws), since a bind group lived only while a set held it. Now the bind group
  table holds each group until a pruning finds it neither held nor bound since the pruning before
  (pruning runs at a submit once the table has doubled), so equal sets made the next frame find it.
  The scene fails unless sets made per draw make as many bind groups as kept sets (256 in 20
  frames; 5120 before).

Run 13 = G2b run 3, 2026-11-17 (fixes from G2b's independent check):
- Merged origin/phase-2-graphics, -webgl, -direct3d-12, -shaders and -rig (no conflicts;
  -direct3d-11 had nothing new). The interface changes they brought (split passes keep their
  stored aspects, format components, sampler codes, typed buffer layouts) needed nothing here. Read
  the four front status files again: new asks of this back end are the surface format and the
  table release above; the Direct3D 11 and WebGL tracks ask nothing new.
- Queries carried across splits in a set with no spare slot of its own (the Direct3D 11 front's
  sets have 4096, WebGPU's largest): a set's spare slots are now pages, page 0 being the set's
  own slots past its queries and further pages WebGPU query sets of 4096 made when page 0 is full.
  A pass part counts on one page; on a further page every query begun there takes a spare slot
  too, and resolving replaces its own slot's count instead of adding to it. Slots given back are
  reused only after the part that wrote them ends. tests/writes.rs
  `queries_carry_across_splits_in_a_set_of_the_largest_size` (a set of 4096, three splits; lost
  the device before).
- Destroying a program forgets every pipeline and layout made with it; a block (deduplicated by
  value) is forgotten once every creation of it is destroyed, a vertex layout with its shadow
  plan. `WebGpu::held()` says how many pipelines and layouts are held.
- The staging ring of 4.6: ordered writes, texture writes behind recorded work, pattern clears and
  the query sums' lists are queued into regions of one buffer (256-byte aligned) and copied from
  there, taken from its start again after each submit; a submit needing more makes one twice as
  large. `Counters::staging_rings`. tests/writes.rs `ordered_writes_are_staged_in_one_ring` (8
  writes in two frames make one ring; a 2 MiB write makes a second). Natively wgpu's
  `write_buffer` still copies the bytes once into memory of its own; in the browser Chrome stages
  them in its own ring.
- Immediates, natively: `immediate_bytes` is the device's `max_immediate_size` where wgpu has
  `IMMEDIATES` (256 on Vulkan, 128 on Direct3D 12, RTX 3080), each program's layout holds what its
  stages declare (`var<immediate>`), and a draw or dispatch sets at most that. tests/scenes.rs
  `immediates_reach_the_program`. In the browser 0 (Departures).
- A lost surface is refused by name: the device is lost with the reason that the back end holds
  no window to make a surface from (wgpu reports it only when the platform lost the swap chain).
- Texture renaming runs in Chrome: the check moved into examples/scenes/renames.rs (tests/bindings.rs
  calls it) and web/check runs it in the page.
- The converted-format test takes `WEBGPU_SEED` (default 1) and `WEBGPU_CASES` (textures of 128
  texels per format, default 4; web/check runs seed 1, 4 cases), and so does its comparison with
  Direct3D 11 in tests/direct3d.rs. With 4 cases the check without a reference failed:
  `Bgrx8UnormSrgb` filtered 1.506 steps from filtering `v / 255` decoded. Our texels equal real
  Direct3D 11's in every load and filtered value on both APIs, and Direct3D's own filtered sRGB
  texels are 1.507 steps from that rule, so the bound for sRGB filtering is now 2 steps (1.5 for
  the others; measured by Direct3D at most 0.714).
- tests/raster.rs runs under libtest-mimic: the wireframe picture on Vulkan is reported ignored as a
  known failure with the count of pixels (was: a pass comparing only the solid cell).
- examples/scenes/names.rs lists the shared line's 9 new suite tests.
- Not changed: `PrepareDesc` still has no index format (adding a field breaks the Direct3D 11 and
  12 fronts' and the suite crate's code, which this track does not touch); listed for the
  interface track.

Run 12 = G2b run 2, 2026-11-17:
- Merged origin/phase-2-graphics, -webgl, -direct3d-12 and -shaders (no conflicts; -direct3d-11
  and -rig had nothing new); the merges did not change the interface. Read the four front status
  files again:
  the new asks of this back end are `B8G8R8X8` (Direct3D 11) and the two-pixel formats (Direct3D
  12's list, also plan 5.7's `BlockDecoded` pair), built here with the formats asked before.
- Converted formats (src/packed.rs): `packed::held` names the format a texture is held in
  (`rgba8unorm`, or `bgra8unorm` and its sRGB twin for BGRX); `TextureObj::held` carries it as for
  decoded BC. Writes widen on the host (`write_packed`); a read back copies the held texels and
  packs them in `finish_read` (no wait); a copy between a buffer and such a texture reads the
  source back first and waits (`HostCopyReadBack`, as for decoded BC); a copy between a converted
  format and another of the same block size goes through the host the same way; a copy between
  two textures of one converted format is a plain GPU copy. Draws into A8 write only alpha and
  into BGRX only colour; clears and don't-care loads keep A8's colour zero and BGRX's alpha one.
  `FormatConverted` is counted when such a texture is made, written and read back.
- Widening repeats each channel's bits (3 of 31 is 24), not the nearest byte to `v / 31` (25):
  the first version took the nearest byte and the comparison with Direct3D 11 found loads one
  8-bit step off on `B5G6R5` and `B5G5R5A1`; the RTX 3080's Direct3D loads them as repeated bits
  (up to 0.72 of a step from `v / (2^n - 1)`). With repetition every load is equal.
- The vertex conversion check moved into examples/scenes/vertices.rs (tests/vertices.rs calls it)
  so that web/check runs it in Chrome too (G2 check finding: it had never run there). Texture
  renaming still runs only natively (tests/bindings.rs).
- Resolving in the other sRGB twin in the browser is now a draw loading every sample through the
  twin and averaging them (`ResolveByDraw`, interface change below; src/fill.rs
  `resolve_by_draw`), so it averages in linear light as natively, instead of the listed loss
  `ResolveInStoredFormat`. Multisampled textures of a format with a twin are made sampleable in
  the browser for it. The shared suite's `resolves_average_in_the_format_named` holds it to the
  linear-light values when the loss is not counted.

Run 11 = G2b run 1, 2026-11-17:
- Merged origin/phase-2-graphics, -webgl, -direct3d-11, -direct3d-12, -shaders and -rig (no
  conflicts). Four new shared suite tests were added to examples/scenes/names.rs.
- The open findings of G2's check:
  - A copy between regions of one subresource lost the device; it now goes through a buffer
    (`CopyWithinTexture`, interface change below); depth or stencil within one subresource fails by
    name. tests/copies.rs `copies_within_one_subresource_go_through_a_buffer`.
  - Splitting a pass while an occlusion query runs (an ordered write, a discard of the pass's
    target, a vertex shadow): the query ends, goes on in a spare slot of its set in the next part
    (occlusion sets are made with as many spare slots again, up to WebGPU's 4096), and resolving
    adds the parts' counts with a small compute program (`QueriesSegmented`). A query begun again
    gives its spare slots back. tests/writes.rs `queries_carry_across_pass_splits`.
  - `resolve_queries` past the buffer's end is `Invalid` with a reason, checked before anything is
    marked. tests/copies.rs `query_results_past_the_buffer_are_refused`.
  - `PrepareDesc` has no index format, so indexed strips still miss after `prepare`: an interface
    gap, now listed under "Gaps found for the interface track" (adding a field breaks both fronts'
    code, which this track does not touch).
  - The 5.7 items not built: see "Not carried out yet".
- Timestamps: `write_timestamp` records an empty compute pass whose beginning writes the timestamp
  (WebGPU writes timestamps only where passes begin and end; written after the work before it).
  Declared where the device has `TIMESTAMP_QUERY` and the queue's period is 1 ns, which wgpu gives
  natively only with the instance flag `AUTOMATIC_TIMESTAMP_NORMALIZATION` (`WebGpu::INSTANCE_FLAGS`;
  wgpu then converts on resolve) and in browsers always. Without the flag, on the RTX 3080 the
  period is already 1 ns on both APIs (tests/timestamps.rs prints it), so the flag changes nothing
  there.
- Wireframe and point filling: wgpu's polygon modes, declared from its features.
- In Chrome the new shared suite test `resolves_average_in_the_format_named` failed: resolving a
  four-sample `rgba8unorm` texture through `rgba8unorm-srgb` views gave the stored average (64 where
  linear light gives 137). In the browser a resolve in the other sRGB twin is now the listed loss
  `ResolveInStoredFormat` (counted, warned), which the test accepts; natively wgpu averages in the
  views' format as before. A draw resolving in linear light would make it exact (Next).

Run 10 = G2 run 6, 2026-11-17 (fixes from the independent check of run 9):
- Merged origin/phase-2-graphics (at 3e9db91, a fast-forward). It brought interface doc rules (one
  viewport a draw, least binding sizes, comparison samplers, empty pass sizes) and suite tests
  (stride zero, comparison sampling) and `SlowPath::CaptureExpanded` (WebGL 2's).
- Renamed buffers are now shown to come from the pool. `Counters::spares_reused` counts renames
  that took pooled storage instead of making a GPU buffer, `spares_dropped` pooled storage
  destroyed. Test writes.rs `renamed_buffers_reuse_the_pools_storage`: four frames, each drawing
  the triangle four times with a `Discard` write of its vertex buffer between draws: the first
  frame's three renames make buffers, each later frame's three take the pool's, the picture is
  Direct3D's triangle every frame.
- The pool is bounded: each buffer counts the GPU buffers its renames made; destroying it destroys
  that many pooled buffers of its size and usage (at once if free, else when their work is done).
  So the pool holds at most what live buffers made. The same test destroys the buffer and sees
  three dropped.
- Textures written with `Discard` while recorded work uses them are renamed from a pool as plan
  4.6 says (was: staged and copied). Views and bind groups of the texture are made again at their
  next use (views carry the texture's generation, and so does a bind group's key). A texture the
  open pass draws or resolves into is not renamed (the pass keeps drawing into its storage): that
  write is ordered in the pass, `WriteSplitPass`. Test bindings.rs
  `discarded_textures_are_renamed_from_the_pool` (four frames of four draws each after a
  `Discard` of the texture they load; three texture renames a frame, all from the pool after the
  first; then a discard of the pass's own target splits the pass; destroying drops three).
- 32-bit float depth copied in (`DepthWrittenByDraw`) keeps only what a draw can write: values
  outside 0 to 1 are clamped, denormals and NaN become zero (the independent check measured this
  on the RTX 3080 against Direct3D, which keeps every bit). It is now a listed loss in plan 4.12
  (interface change below), and copies.rs `float_depth_copies_keep_what_a_draw_can_write` checks
  exactly that, from a texture and from a buffer. The earlier claim that these copies are exact
  held only for values in 0 to 1, which the other tests use.
- `CopyWithinBuffer` is checked: writes.rs `copies_within_one_buffer_are_exact_and_counted` (three
  copies within one buffer, at places in a word alike and different, and one between two arena
  buffers: four counted), the arena test counts one, and the random byte test counts its copies
  within one buffer and requires the slow path count to equal them.
- tests/depth.rs runs under libtest-mimic, so where `DEPTH24_UNORM` is absent (wgpu on Vulkan,
  Metal, browsers) the bias picture is reported as ignored with the reason instead of passing.
- tests/blocks.rs takes `WEBGPU_CASES` (textures of seeded blocks per format, default 1) beside
  `WEBGPU_SEED`, in both random tests.

Run 9 = G2 run 5, 2026-11-17:
- Merged origin/phase-2-graphics (at 385604f). The lock file conflicted (our libtest-mimic and
  objc2 test crates against the shared line's new ones): took the shared line's and let cargo add
  ours back offline (additions only). The interface brought `INSTANCE_STEP_RATE` and
  `TEXEL_BUFFERS`; the back end has neither, so step rates other than one and typed buffer
  bindings are now `Absent` of those bits (were `Invalid`), tested in bindings.rs
  `step_rates_and_texel_buffers_are_reported_absent`.
- The present test on this Mac passes. The window was never seen as visible because the test host
  ran the run loop without handling the application's events, which is how AppKit learns a window
  is on screen (a Swift probe: occlusion state 0x2000 with the run loop alone, 0x2002, visible,
  when the events are handled). The host now calls `finishLaunching` and handles events while it
  waits. The frame is drawn through `PresentConverted` (a Metal layer takes no `rgba8unorm`),
  which the host now prints and on macOS requires; the window shows the frame's colour exactly.
- Vertex formats WebGPU lacks are built (src/vertex.rs, `VertexFormatConverted`, plan 5.7). A
  vertex layout with one gets a plan when made: each stream holding one is read from a shadow
  stream laying every attribute of that stream out again, the lacking ones widened (three 8-bit
  or 16-bit components to four, the fourth set to what a three-component format reads, one or its
  integer; `Float16x3` to `Float16x4`; `Uint10_10_10_2` to `Uint16x4`; `Float11_11_10` to
  `Float16x4`, exact, since both have a 5-bit exponent), the others copied as they are. The
  pipeline takes the shadow's layout. At a draw, a shadow kept for that buffer, offset, layout and
  stream is used while the buffer's write count is unchanged; otherwise a compute program makes it
  from every whole element from the offset to the buffer's end, splitting the open pass (counted
  once as `VertexFormatConverted`; `split_pass` now counts the slow path that needs it). Every
  buffer write, copy or clear into a buffer, query resolve and storage binding of it moves the
  buffer's write count. Destroying a buffer drops its shadows.

Run 8 = G2 run 4, 2026-11-17:
- Merged origin/phase-2-graphics (at 41816d5) and origin/phase-2-graphics-direct3d-11. The
  interface changes they brought (doc sentences, recording back end checks) needed nothing here.
  The first desktop build after them failed to link a stale `wcontainers-gpu-test` library
  (unresolved symbols); `cargo clean -p wcontainers-gpu-test` fixed it. Recorded runs now clean
  both crates.
- Presenting a format the window lacks: a Metal layer takes no `rgba8unorm`, which every back end
  presents (plan 4.9), so the device lost it there ("Validation Error" from wgpu). Now
  `configure_surface` asks the window's formats (the back end keeps its adapter for this); a
  window lacking the format is configured in the other byte order (`bgra8unorm`), frames are drawn
  into a stand-in texture of the asked format, and `present` draws it into the window's texture
  (`PresentConverted`, interface change below; src/fill.rs `present_by_draw`). Not yet run on a
  shown window (see the present test below); on Windows and in Chrome the window takes the format
  and nothing changes.
- The suite's present test on macOS: AppKit makes windows only on the main thread and the standard
  test harness runs every test on a thread of its own, so tests/present.rs runs
  `present_shows_the_frame` with libtest-mimic on the main thread (on Windows it has no tests:
  tests/suite.rs runs present there). The host (tests/host/mod.rs, shared by both files) opens a
  borderless 4 by 4 pixel floating window whose content view's own layer is the surface, and reads
  it through ScreenCaptureKit (one window, `SCScreenshotManager`) in sRGB, which undoes the
  display's colour matching exactly: a Swift probe of the same steps read 256 of 256 colours
  exactly through an external monitor's ICC profile (`CGWindowListCreateImage` gave the display's
  values, and converting them back to sRGB missed 193 of 260). The screen recording permission is
  held by the session the tests run from here (`CGPreflightScreenCaptureAccess` true).
- That test still fails here, as a known failure with its reason: macOS reports every new window
  of this session as not visible (`occlusionState` 0x2000, also for a plain Swift window, at any
  place and level), and wgpu's Metal surface hands out no texture to an occluded window, so the
  frame is drawn unshown, as the back end does for an occluded window. Why this session's windows
  are occluded is not known (display not asleep, session not locked). The test now says so instead
  of comparing the window's grey.
- The suite's list for hosts that run present apart (the browser page and macOS) is one macro,
  examples/scenes/names.rs; web/check runs the page against the whole native list on Windows, so a
  test the shared crate adds and the list lacks fails there.

Run 7 = G2 run 3, 2026-11-17:
- Merged origin/phase-2-graphics (at f107eb5), then origin/phase-2-graphics-direct3d-12 and
  -shaders (each a few commits ahead of it). The shared line brought the interface change c8a7be9:
  `copy_texture` copies a combined depth-stencil format whole, both aspects, with `stencil` false.
- Combined depth-stencil copies now follow it: equal formats and whole levels are one WebGPU copy
  of every aspect (before, the depth aspect only, which WebGPU refuses); part of a level is drawn,
  depth by `DepthWrittenByDraw` and stencil by the new `StencilWrittenByDraw` (interface change
  below): per layer one draw clears the rectangle's stencil, then one draw per bit with that bit's
  write mask and reference 0xff keeps the fragments whose source has the bit (src/fill.rs). Copies
  between a combined format and another format mark the device lost.
- Part of a stencil from a buffer (WebGPU copies depth and stencil only whole) goes into an
  `r8uint` texture and is drawn the same way. Copies into a buffer and read backs that WebGPU
  cannot make, 24-bit depth at all and part of any depth or stencil level, draw the aspect into a
  colour texture first (`DepthReadByDraw`): 24-bit depth as `round(d * 16777215)` into `r32uint`
  (exact: a unorm value stored as the nearest float is within half a step of its integer after one
  correctly rounded multiply), 32-bit float depth's bits into `r32uint`, 16-bit depth into
  `r16uint`, stencil into `r8uint`. A depth or stencil texture made with `COPY_SRC` (which read
  backs need) gets binding usage, and one made with `COPY_DST` render usage, where its format
  allows, so these draws can run; textures without copy usages get neither. Part of a 16-bit depth level from a
  buffer marks the device lost (no Direct3D call makes one).
- A buffer copy or read back of a combined format's depth aspect was sized as both aspects (8
  bytes a texel); it is now the depth format's size.
- BC1 to BC5 against Direct3D: tests/direct3d.rs loads every BC format's seeded blocks with
  Direct3D 11 on the RTX 3080 and with WARP (a pixel shader compiled at test time by
  `D3DCompile`), and through the back end natively and decoded. Natively every texel equals
  Direct3D's. Decoded, BC1 to BC5 are within 2 / 255 of WARP (BC4 and BC5 within 2.6e-4, the
  16-bit float they are stored in), while the RTX 3080's own decoder differs from WARP by up to
  13 / 255: hardware decoders interpolate BC1 to BC5 within a tolerance. So the bar is: decoded
  texels no further from the GPU's than WARP's are, plus one step of the format they are stored in.
  It passes. The old one-step comparison with the GPU's decoder in tests/blocks.rs stays ignored
  as a known failure that WARP fails too.
- `prepare` in Chrome: examples/scenes/prepare.rs (shared by tests/prepare.rs and the page, timed
  by `performance.now()` there) draws 16 new programs unprepared, then 16 others after `prepare`;
  web/check prints both lines and fails if the prepared draws built anything. In Chrome the
  `prepare` calls return at once (0.6 ms for 16) and the compiles finish in the GPU process before
  the next submit completes, so `BACKGROUND_PREPARE` is confirmed there (measurements below).
- The scene tests (scenes.rs, raster.rs, the two writes.rs tests drawing the triangle) compare with
  the rig's allowance for each picture (references/d3d11/tolerances.txt, what WARP earns: 1 for the
  triangle, quads and viewports, 0 for the instanced scene), so they hold on any GPU, and print
  how many pixels differ at all. On the RTX 3080 every pixel still equals Direct3D's (printed, and
  web/check compares the twelve pictures at 0).
- Found: incremental builds on the desktop sometimes kept a stale library (a changed string in
  src/memory.rs did not reach the test binary, twice). Runs whose results are recorded here now
  start with `cargo clean -p wcontainers-gpu-webgpu` in the job's build command. Not reported to
  the rig track yet.

Run 6 = G2 run 2, 2026-11-17:
- Merged origin/phase-2-graphics at d31a49e (every other graphics branch was already in it); the
  lock file then lacked one dependency of the Direct3D 12 crate (`naga`), added in 28a01f6.
- Waiting in the browser is built (src/wait.rs, plan 4.8): `wait_read`, `wait_fence` and the
  24-bit depth probe suspend the WebAssembly stack through JSPI (wasm-bindgen 0.2.130's
  `#[wasm_bindgen(suspending)]` import of `Promise.resolve`) until a callback of the browser's
  WebGPU (map, work done, error, loss) wakes it; `block_on` does the same for futures (adapter and
  device requests). The call that waits must run inside an export marked `#[wasm_bindgen(jspi)]`.
  The probe now runs in the browser too, so `depth24()` always answers.
- The shared suite runs in Chrome (examples/scenes/suite.rs, web/suite.html, web/suite.mjs, one
  page per test, run by web/check against the native suite's `--list`): 24 of 24 pass on the RTX
  3080. Two things had to change for it:
  - wgpu 30.0.2's browser back end dropped a render pass's `occlusionQuerySet` (Chrome: "The
    occlusionQuerySet in RenderPassDescriptor is not set"); wgpu is vendored in third_party/wgpu
    with that one fix (d65f0be, a `[patch.crates-io]` entry in the workspace manifest, as for naga).
    wgpu's trunk has the same bug. Not reported upstream.
  - In the rig's headless Chrome 157, `drawImage` and `createImageBitmap` of a WebGPU canvas give
    transparent black; `toDataURL` gives the frame. The page reads the canvas with `toDataURL` and
    the host decodes the PNG.
- Real Direct3D 11 on the same GPU (tests/direct3d.rs, Windows only, through the rig crate's device
  and shaders): `generate_mips` of `r32float` on a device without `float32-filterable` and of
  `rgba16uint` holding `rgba16unorm` values, each level made from Direct3D's level above, against
  GenerateMips of a 64 by 48 texture (7 levels, odd sizes further down), and a four-sample
  `rgba8unorm` texture made with `reinterpret` resolved as `rgba8unorm-srgb` against
  ResolveSubresource with `R8G8B8A8_UNORM_SRGB`. Tolerances: `r32float` 2^-16 relative to the value
  (at least 1), `rgba16unorm` one step, the resolve exact. The same resolve as `rgba8unorm` differs.
- BC textures are decoded where the device has no `texture-compression-bc` (src/blocks.rs,
  `BlockDecoded`, plan 4.12): BC1 to BC3 and BC7 as `rgba8unorm` (sRGB kept), BC4 and BC5 as
  `r16float` and `rg16float`, BC6H as `rgba16float`, by bcdec_rs at each write; the blocks are kept
  on the host per level and layer and answer read backs at once, copies into buffers (written in
  command order), copies between BC textures and copies into uncompressed textures of the same
  block size. Copies into such a texture from a buffer or an uncompressed texture read the source
  back and wait (`HostCopyReadBack`). Declared `SAMPLE | FILTER | COPY_OUT`, one sample.
- Natively, a copy between a BC texture and an uncompressed one of the same block size (Direct3D's
  CopySubresourceRegion) failed as "different block sizes"; it now goes through a buffer with the
  destination's extent in its own texels (`CopyThroughBuffer`).
- `FRACTIONAL_VIEWPORT` is declared in the browser too: the eight-viewport scene (now
  examples/scenes/viewports.rs, shared by tests/raster.rs and the browser page) drawn in Chrome
  equals Direct3D 11's `raster-viewports.png` in every pixel, and moved to whole pixels differs
  (web/check). The depth probe in Chrome (Dawn on Direct3D 12) answers `Float(0.10000001)`:
  `depth24plus` is 32-bit float there, so `DEPTH24_UNORM` stays clear in the browser.
- New tests: tests/direct3d.rs (above); tests/blocks.rs: every BC format's seeded blocks
  (`WEBGPU_SEED`, default 188) on a device with BC and one made without, three levels of 12 by 8
  (the last a partial block), each level read back as written on both; BC6H and BC7 sample exactly
  alike on both; copies texture to texture, into a buffer at pitch 256 and back, from and into
  `rgba32uint` texels keep the blocks on both, `HostCopyReadBack` only where decoded. Known failure
  (`#[ignore]`, `bc1_to_bc5_sample_alike_natively_and_decoded`): see "Not carried out yet".

Run 5 = G2 run 1, 2026-11-17 (fixes from the independent check of run 4, then pass bar items):
- Merged origin/phase-2-graphics at 528db9c (a fast-forward). It brought three shared suite tests,
  `multisampled_targets_cover_their_samples_and_resolve`,
  `occlusion_queries_answer_whether_samples_passed` (which also covers `resolve_queries` at the
  unaligned destination offset 8) and `programs_run_per_sample_and_read_samples`; all 24 suite tests
  pass on Vulkan and on Direct3D 12.
- `clears_fill_exactly` passes on Direct3D 12 now: an integer clear wgpu's Direct3D 12 would round
  through a float is loaded and then drawn by a full-target draw writing the value from a uniform
  (src/fill.rs, new slow path `IntegerClearByDraw`, an interface change below).
- Copies into depth are built (`DepthWrittenByDraw`, src/fill.rs): from a buffer or a write, the
  bytes go into an `r32float` texture (32-bit float depth) or an `r32uint` one (24-bit depth, drawn
  as `(v & 0xffffff) / 16777215`) and a draw writes them as depth; from a texture of the typeless
  group (`r32float`, `r32uint`, `r32sint` into 32-bit depth; any depth into the same family) by a
  draw reading it, also into part of a texture, which WebGPU copies never do for depth. Depth
  textures made with copy usages get render and binding usage for it. Stencil copies WebGPU refuses
  (part of a stencil, or between formats) and multisampled depth copies mark the device lost with
  the reason (`StencilCopiedByDraw` is not built); so do copies of 24-bit depth into a buffer.
- web/check twists the quads on wgpu's Vulkan too, and a comparison gt-compare could not make
  (exit 2) now fails the check instead of counting as "differs". It builds with `--locked`.
- The lock file: the shared line's daa1630 brought it in step; every build and test command here
  now passes `--locked`, so a stale lock file fails the build instead of being rewritten.
- A program without a fragment shader draws into colour targets through an empty fragment stage
  of the back end's own with every write mask off (was "not built yet").
- The 24-bit depth probe of 4.11 runs natively when the back end is made (src/probe.rs,
  `WebGpu::depth24()`): on the RTX 3080 wgpu's Direct3D 12 answers one 24-bit step
  (`Unorm(0.10000009)`, `DEPTH24_UNORM` set) and wgpu's Vulkan two steps (`Other(0.10000015)`:
  NVIDIA's Vulkan bias unit for D24 is 2^-23), so there the bit stays clear and fronts bias in the
  program. In the browser it does not run yet (waiting needs JSPI, 4.8), so the bit is clear there.
- The rig's depth-bias scene (constant, slope and clamped bias on `D24_UNORM_S8_UINT`) drawn
  through the back end on `depth24plus-stencil8` equals Direct3D 11's `depth-bias.png` in every
  pixel on Direct3D 12 (tests/depth.rs; on Vulkan the bit is clear, so the test says the front's
  route applies and draws nothing).
- Samplers: a linear sampler bound where the view cannot be filtered marks the device lost with
  "fronts filter in the program (FilterInShader)" instead of wgpu's validation text.
- Formats: blending is declared only where a format renders (wgpu marks `r8snorm` blendable on a
  device that cannot render it); 16-bit normalised formats are asked for natively
  (`TEXTURE_FORMAT_16BIT_NORM`).
- Programs are validated when made with naga's validator for WebGPU's core language and the
  device's features (`wgpu_naga_bridge::create_validator` with `DownlevelFlags::compliant()`), and
  the back end keeps that validation's info, not the front's.
- `prepare` builds the combination's pipeline into the cache, keyed as the draw will key it, so
  the first draw builds nothing (counted). `BACKGROUND_PREPARE` is declared only in the browser.
- `FRACTIONAL_VIEWPORT` is declared natively: the rig's eight-viewport scene (four viewports start
  half a pixel down; drawn here once per viewport with the first instance as the viewport number,
  in place of its geometry shader) equals Direct3D 11's `raster-viewports.png` in every pixel on
  both APIs, and the same viewports moved to whole pixels differ (tests/raster.rs). In the browser
  the bit stays clear until the same check runs in Chrome.
- A pass fitted by the shared pass tracker with four-sample colour targets of 2 by 2 and 3 by 3
  draws right on the GPU, sample by sample (tests/fitted.rs, ported from the shared helpers test
  that runs on the recording back end): `AttachmentsFitted` once, no `MultisampleCopyResolved`.
- New tests: tests/copies.rs (buffers at a 36-byte pitch and writes into all four depth formats;
  `r32float`, `r32uint` and `depth32float` into part of 32-bit depth and 24-bit into part of
  24-bit depth; seven typeless group copies of colour formats, through a buffer or native for sRGB
  twins, each exact and with its slow path counted); tests/depth.rs (the probe, the bias picture);
  tests/prepare.rs (prepared combinations build nothing at their first draw, timed);
  tests/bindings.rs `programs_without_a_fragment_stage_write_only_depth`,
  `helper_blocks_of_each_stage_read_their_own_values` (set 3, binding 0 read by the vertex stage
  and 12 by the fragment stage), `layouts_follow_each_views_filterability` (an `rgba8unorm` view with
  a point then a linear sampler is one pipeline; `r32float` and `depth32float` views at points on a
  device made without `float32-filterable`, where a linear sampler is refused; `r32float` filtered
  with it), `tier1_formats_are_declared_only_with_it` (on a device made without wgpu's 16-bit-norm
  and adapter-specific format features: no 16-bit normalised format and none can be made, 8-bit
  snorm without render or blend bits; with them, all six 16-bit formats sample and the unsigned
  and 8-bit snorm ones render), `programs_are_validated_with_webgpus_core_language`.

G2a (the first slice) is done, run 1, 2026-11-17:
- The back end implements the whole `Backend` trait. Blocks are made once per value; a draw's
  combination (program, four blocks, pass target formats, topology and strip index format, the
  layout key of each bound set) names a cached render pipeline, built at the first draw (counted
  `PipelineMiss`), and `setPipeline` is called only when the pipeline changes. Bind group layouts
  are derived from the program's naga modules and the sets' layout keys (4.5), shared by content;
  uniform bindings take dynamic offsets up to the device's limit; binding sets are shared by
  content and each makes one bind group per layout, made again only when a buffer it names was
  renamed (generation). Compute dispatches use the same layouts.
- Writes follow 4.6: straight to the queue when no unsubmitted command uses the resource;
  otherwise `Discard` renames the buffer (old storage pooled until its work is done), `NoOverwrite`
  writes in place, `Ordered` goes through a staging buffer and a copy, splitting an open pass
  (`WriteSplitPass`). Writes that take effect at once but are not whole words (and any later
  write of that buffer before the submit) are copies in a lead encoder submitted ahead of the
  frame's encoder (run 3, below). Read backs copy into a mapped buffer, mapped after the submit; rows arrive
  packed. Occlusion queries, `resolve_queries` by `resolveQuerySet` and a copy, resolves, surfaces
  (windows registered by the host with `WebGpu::add_window`) are in.
- The rig's three Direct3D 11 reference scenes (triangle; textured, depth-tested, blended quads;
  instanced draw with a constant buffer), drawn through the interface with WGSL programs, equal real
  Direct3D 11's pictures in every pixel on the RTX 3080: natively through wgpu on Direct3D 12 and
  on Vulkan, and built for WebAssembly in Chrome 157's WebGPU. Tolerance 0 (see Measurements).
- The quads with the colour source factor changed to one, or the depth test changed to greater,
  fail the comparison both natively and in Chrome.
- Since run 4 the quads also equal Direct3D's picture drawn on `depth24plus-stencil8` (natively and
  in Chrome) and on every other depth format of 24 bits or more (natively), and the whole shared
  suite runs here, present included (Vulkan 21 of 21; Direct3D 12 20 of 21, the known clear).
- Writes keep command order (test `writes_take_effect_in_command_order`: copies between an
  ordered write that stages, a second one, and a renaming discard each read the bytes written just
  before them), and an ordered write of a vertex buffer the open pass drew with splits the pass
  without changing the picture (`ordered_write_in_a_pass_splits_it`, equal to Direct3D's
  triangle, one `WriteSplitPass`).
- Reviewed for places doing less than the interface promises: extra viewports or scissors and
  differing per-face stencil references now fail loudly instead of being ignored, textures take
  only the copy usages their format allows (`depth24plus` has none), and anisotropy applies only to
  fully linear samplers. What is still missing is listed below.

Run 2, 2026-11-17 (fixes from the independent check of run 1, then the shared suite):
- Read backs now count as uses of their source, so a later write in the same encoder is ordered
  after them and `destroy` waits for them (they returned the new bytes, and destroying the source
  lost the device). Resolve targets of a pass count as used too.
- The device is made with `float32-blendable` where the adapter has it (the RTX 3080 natively
  and Chrome do). A blend into any non-integer target the device cannot blend now marks the device
  lost with the reason (the front blends in the program, `BlendInShader`); before, the blend was
  silently dropped.
- The uniform arena of 4.5 and 4.6 is built: uniform-only buffers with `MapIntent::Often` live in
  slices of one large buffer; a `Discard` while recorded work uses one moves it to a free slice
  (counted in `Counters::slices`), its bind group stays, and the draw passes the slice's offset as
  a dynamic offset. Bind groups are shared by what they are made of, with dynamic offsets left out:
  sets differing only in a dynamic offset, or naming different arena buffers of one size, share one
  bind group. A draw whose sets did not change and whose bind epoch (moved only by a rename or arena
  move of a buffer some set names) is the same only reads each dynamic entry's offset, with no
  allocation; `setBindGroup` is called only when the group or an offset changed.
- Clears to a non-zero pattern copy from a block of at most 64 KiB of the pattern, counted as the
  new slow path `PatternClearCopied` (an interface change, below); before, they uploaded a host
  copy of the whole range uncounted.
- Buffer writes, copies and clears take any byte offset and length (`UnalignedBufferEdge`): the
  aligned middle is copied natively and the words a range only partly covers are merged by a small
  compute program of the back end's own (src/edge.rs); where source and destination differ in their
  place within a word, the program copies every word. So every buffer gets WebGPU's storage usage.
  Buffer and texture copies at row pitches WebGPU does not take go through a buffer at its pitch,
  one buffer copy per row (`RowsRepacked`).
- Under wgpu's Direct3D 12 (native only), which clears colour targets through 32-bit floats, an
  integer clear value past 2^24 now marks the device lost with the reason instead of clearing to
  a rounded value.
- Mirror-once samplers are refused as `Absent(MIRROR_ONCE)` (was `BORDER_COLOR`); dual-source
  blend factors without the feature as `Absent(DUAL_SOURCE_BLEND)` at `create_blend`.
- Handles compare equal when they name one object, and the caching test checks that equal blocks
  made twice are one block.
- New tests: tests/bindings.rs (twenty draws each after a `Discard` of their constant buffer make
  one bind group, no rename and nineteen slice moves, every pixel its own draw's colour; two sets at
  offsets 0 and 256 make one bind group; an `rgba32float` blend equals Direct3D's rule or fails
  loudly), tests/writes.rs (read backs keep command order for buffers and textures; destroying
  after a recorded read back; a pattern clear over 200000 bytes; arena buffers through discards,
  ordered writes, copies between two of them and a read back; a seeded random test of 400 byte-
  granular writes, copies and clears against a model, `WEBGPU_SEED` and `WEBGPU_CASES`), and
  tests/suite.rs (the shared suite of `wcontainers-gpu-test`, seven tests).
- The shared suite (origin/phase-2-graphics at a58f79a, merged): 7 of 7 pass on Vulkan, one of
  which, present, checked no texels (the host gave none; the shared line has since left it out); on
  Direct3D 12, 6 of 7: `clears_fill_exactly` fails, as a known failure, at its `R32Uint` clear of
  0xdeadbeef (wgpu-hal's dx12 command.rs passes clear values to `ClearRenderTargetView` as f32,
  which gave 0xdeadbf00; the back end now fails loudly there). Vulkan clears it exactly; Chrome's
  Dawn is expected to (it clears big integers with a draw on Direct3D 12), not yet run there.

Run 3, 2026-11-17 (fixes from the independent check of run 2):
- Ordered writes no longer land out of order. Before, a write not in whole words went through a
  copy in the frame's encoder without marking the buffer used, so a later whole-word write went
  straight to the queue and landed first. Now such a write, when it takes effect at once (no
  recorded command uses the buffer, or `NoOverwrite`, or after a `Discard` renamed it), is a copy
  in a second encoder, the lead encoder, submitted just before the frame's encoder; later writes of
  that buffer before the submit go there too (`BufferObj::lead_in`), so all of them keep their
  order, and queue writes made earlier still land before. It never splits a pass, also while an
  occlusion query runs. An `Ordered` write of a buffer the frame's encoder uses still stages in
  that encoder and marks the buffer used.
- `NoOverwrite` had three readings (the test model, the interface wording, the back end). The
  interface now says the caller promises that no unfinished command and no `Ordered` write since
  it last waited reads or writes the bytes (interface change below): Direct3D 11 allows
  no-overwrite only on dynamic resources, which copies and `UpdateSubresource` cannot write, and
  OpenGL leaves unsynchronised writes over pending operations undefined. The random test now keeps
  that promise: it tracks the bytes touched since its last wait and makes a write `Ordered` where a
  no-overwrite write would break it (it prints how many of each it made).
- Byte-granular copies are merged in pieces sized from the device's limits (workgroups per
  dimension less one, and the largest storage binding less one offset alignment and a word), not
  16 MiB; a 16 MiB copy between different places in a word lost the device before.
- Clears take a pattern of any length: the copied block is a multiple of the pattern and of 4
  bytes. A 12-byte pattern wrote wrong bytes past 64 KiB before.
- Layouts follow 4.5 exactly: a texture the program only loads from is always
  `unfilterable-float`, and the pipeline and layout keys keep only the bits of textures the
  program samples through a sampler (`paired`, per shader, from naga's sampling set), so one
  program drawn with a filterable and then an unfilterable view of a load-only texture is one
  pipeline. Working out the keys per draw allocates nothing.
- Merged origin/phase-2-graphics at c15677a (a fast-forward): the suite host now leaves out
  `present_shows_the_frame`, which this host cannot check (a swap chain's texels cannot be read
  back once presented), so the suite here is 12 tests and present is not run.
- New tests: writes.rs `writes_of_any_length_keep_their_order` (every pair of modes, first or
  second write not whole words), `writes_ahead_of_a_pass_do_not_split_it` (an ordered write not
  in whole words of a buffer the pass does not use, a discard and a no-overwrite byte of the
  vertex buffer it draws with, inside a running occlusion query: no split, Direct3D's triangle),
  `large_copies_between_places_in_a_word_are_exact` (16 MiB from byte 1 to byte 2),
  `clears_take_patterns_of_any_length` (3, 12 and 16 bytes over 120000 bytes at byte 1);
  bindings.rs `load_only_textures_keep_one_pipeline_for_any_view` (an `rgba8unorm` view, then a
  `depth32float` view, whose key differs: one pipeline, both pixels right).

Run 4, 2026-11-17 (fixes from the independent check of run 3; merged origin/phase-2-graphics at
2684d97, a fast-forward):
- Views of textures with depth and stencil (`depth24plus-stencil8`, `depth32float-stencil8`) were
  invalid: a depth-only view kept the combined format, so any pass or binding with one lost the
  device. Each view now holds two wgpu views where they differ: every aspect in the texture's
  format, which passes draw into, and the aspect `stencil` names in that aspect's own format
  (`depth24plus`, `depth32float`, `stencil8`), which bindings read. A stencil view of a format
  without stencil is `Invalid`.
- The shared suite runs whole (`suite!(Native)`), so tests added to it run here at once.
  `present_shows_the_frame` runs too: the test window is a 4 by 4 popup, and `shown` reads what
  the compositor holds for it after two DWM compositions (`PrintWindow` with
  `PW_RENDERFULLCONTENT`, which includes swap chain contents also on the rig's hidden desktop,
  where reading the screen gave black; alpha set opaque, since a window holds none).
- `current_texture` no longer answers every surface state but success as `DeviceLost`. Outdated:
  configure again as last configured and try once more. Timeout, occluded, or still outdated: the
  frame is drawn into a texture of the configured size and `present` shows nothing, as a real
  swap chain does for an occluded window. Lost and validation failures mark the device lost with
  the reason, so `status()` agrees with the error.
- The device asks for `texture-adapter-specific-format-features` where the adapter has it
  (natively; browsers do not), so format caps carry the adapter's own sample counts, storage,
  filter and blend bits instead of WebGPU's guaranteed set. Copy usages still follow WebGPU's
  rules (wgpu's native table would offer copies of `depth24plus`, which WebGPU refuses).
- The binding-set table is pruned like the bind-group table: at each submit, once it has doubled
  since the last pruning, entries no set holds are dropped.
- A vertex format WebGPU lacks (three-component 8 and 16-bit, `Float16x3`, `Uint10_10_10_2`,
  `Float11_11_10`) now marks the device lost with "not built yet" instead of `Invalid` (a front
  bug): the interface and plan 5.7 give the conversion to the back end. Listed below.
- The shared test crate now brings proptest (getrandom, which needs a feature to build for
  WebAssembly), so it is a dev-dependency only off WebAssembly and tests/suite.rs is native only.
- `web/check` also draws the quads on `depth24plus-stencil8` (picture `quads-d24s8`) with wgpu on
  Direct3D 12 and Vulkan and in Chrome, compares it with Direct3D 11's quads at tolerance 0, and
  requires its twisted versions to differ.
- New tests: bindings.rs `combined_depth_stencil_views_draw_and_read_each_aspect` (both combined
  formats: cleared to depth 0.25 and stencil 7 through one view, then that view and a stencil
  view read by a program, [64, 7, 0, 255]); scenes.rs `quads_match_direct3d_on_every_depth_format`
  (`depth24plus`, `depth24plus-stencil8`, `depth32float`, `depth32float-stencil8`, stencil cleared
  where present, each equal to Direct3D's picture at tolerance 0).

## How another track builds and uses the back end

- Depend on it: `wcontainers-gpu-webgpu = { path = "../wcontainers-gpu-webgpu" }` (and `wgpu =
  "30.0.2"` to make the device). It builds natively (Windows: Direct3D 12 and Vulkan; Mac: Metal)
  and for `wasm32-unknown-unknown` (the browser's WebGPU).
- Make the wgpu instance with `desc.flags |= WebGpu::INSTANCE_FLAGS` (wgpu's
  `AUTOMATIC_TIMESTAMP_NORMALIZATION`): without it, natively on a GPU whose timestamp period is not
  1 ns, `TIMESTAMPS` is declared absent. On the RTX 3080 the period is 1 ns on both of wgpu's APIs
  with or without the flag (tests/timestamps.rs prints it), so there it changes nothing.
- `SUBGROUPS` is declared where wgpu offers `SUBGROUP`: on the RTX 3080 through wgpu's Vulkan,
  not through its Direct3D 12 with FXC (the fronts' instance; DXC would offer it), and in the
  browser where the adapter offers `subgroups`. `PRIMITIVE_INDEX` likewise where wgpu offers
  `PRIMITIVE_INDEX` (natively on both of wgpu's APIs on the RTX 3080; in the browser where the
  adapter offers `primitive-index`). A program using subgroups on a device without them is refused at
  `create_shader` with `GpuError::Invalid`, so fronts check the capability first.
- Make it from a wgpu adapter: ask for the device with `WebGpu::device_descriptor(&adapter)` (the
  features the back end uses where present, and the adapter's highest limits), then
  `WebGpu::new(&adapter, device, queue)`. `WebGpu` is the `B` of `Device<B: Backend>`. The whole
  pattern, with an adapter that must be a GPU, is `device()` in
  crates/wcontainers-gpu-webgpu/examples/scenes/draw.rs, which also shows a draw end to end
  (shaders from WGSL through naga, blocks, a binding set, a pass, a draw, a read back).
- Views of `depth24plus-stencil8` and `depth32float-stencil8` work both as depth attachments and
  as bindings; `stencil: true` reads the stencil as unsigned integers (`texture_2d<u32>`), the
  depth as `texture_2d<f32>` or `texture_depth_2d`.
- Binding numbers follow 6.4: uniform buffers from 0, samplers from 16, textures from 32, storage
  from 160; group = binding set (vertex-only set 0, fragment-only set 1, both 2, helpers 3).
- Natively `wait_read` and `wait_fence` block on the device. In the browser they suspend through
  JSPI, so the export that leads to them (and to `WebGpu::new`, which waits for the depth probe)
  must be marked `#[wasm_bindgen(jspi)]`, as `draw_scene` and `suite_test` in examples/scenes are;
  `wcontainers_gpu_webgpu::block_on` runs a future (the adapter and device requests) the same way.
- Binding sets equal to ones of the frame before (made again when bindings change) find the bind
  group made then; keeping a set still saves its lookup, about 1 µs a draw at 4 streams and 2 at 16
  (Measurements, run 14).
- Query sets may hold any number of queries: past WebGPU's 4096 a set they are paged over
  several WebGPU sets. A pass counts on one page at a time, starting on the first; an occlusion
  query whose own slot is on another page goes on in a spare slot of the pass's page, the pass
  split onto a page with room when it has none (`QueriesSegmented`). Results are resolved into
  one ring buffer, taken again from its start after each submit (`Counters::query_rings`).
- Uniform-only buffers made with `MapIntent::Often` live in the uniform arena: give constant
  buffers rewritten per draw that intent and write them with `Discard`.
- Formats WebGPU lacks are declared with what the back end does for them (`FormatConverted`,
  src/packed.rs): `A8Unorm`, `Bgrx8Unorm(Srgb)` as targets too; the 16-bit packed and two-pixel
  formats sampled and copied only. Copies between one of them and a buffer or a texture of
  another format of equal blocks run on the GPU, through a texture of its raw blocks widened or
  packed by a draw; neither copies nor read backs wait on the device.
- Timestamps (`TIMESTAMPS`): `write_timestamp` outside a pass, `resolve_queries` gives nanoseconds,
  `timestamps_disjoint` is always false. Their resolution is the GPU's or the browser's, not 1 ns
  (measured run 19, examples/scenes/timestamps.rs prints the step): natively on the RTX 3080 every
  timestamp was a multiple of 1024 ns on both of wgpu's APIs; Chrome 157 as users have it rounds
  every timestamp to a multiple of 65,536 ns (about 65.5 µs; web/plain.mjs; in run 23 of
  131,072 ns, so the step is not fixed), so a program timing GPU work shorter than that in a
  browser reads 0 or steps of 65.5 µs or more, although the fronts report
  a 1 GHz frequency. With `--enable-webgpu-developer-features` (the rig's other Chrome checks)
  Chrome does not round them. Calibration against the CPU clock (Direct3D 12's
  `GetClockCalibration`) is made from base primitives, as examples/scenes/timestamps.rs `reading`
  does: write a timestamp, resolve it, read it back, take the CPU clock just before `submit` and just
  after `wait_read`; the GPU time lies between the two. Measured: the GPU clock runs at the CPU
  clock's rate (tests/timestamps.rs).
- `WebGpu::counters()` says how many pipelines, bind groups, layouts, buffer and texture renames
  (and how many took pooled storage, how much pooled storage was destroyed) and staged writes were
  made, and how many bytes were written into buffers and textures; `slow_paths()` the slow paths
  taken. At each power of two frames presented the back end prints both on standard error as
  one line (`FRAME_REPORT`, "webgpu: frames=<n> name=count ..."), so any program's log gives a
  frame's counts between two reports (examples/frames.rs `--log` reads them).
- Anything the back end does not carry out yet (list below) marks the device lost with
  `LossReason::Error` and prints `webgpu: <reason>` on standard error, so `status()` shows it.

## How to run

All from the checkout root on the Mac; nothing is built or run on the Mac but formatting and
`cargo check -j 2`.
- Tests on the RTX 3080 (tests/bindings.rs, blocks.rs, converted.rs, copies.rs, depth.rs,
  direct3d.rs (Windows only), fitted.rs, prepare.rs, pulled.rs, queries.rs, raster.rs, scenes.rs, surface.rs
  (Windows only), features.rs, timestamps.rs, vertices.rs, writes.rs and suite.rs, the shared suite; they need a
  GPU, and the suite's present test and surface.rs a desktop, so
  `--desktop`; `WEBGPU_API=dx12|vulkan|metal` picks wgpu's API, default the platform's, Vulkan on
  the desktop), both APIs in one job:
  `tools/graphics-rig/gpu-job --desktop --build "cargo test --locked -j 3 -p wcontainers-gpu-webgpu
  --no-run" -- "set WEBGPU_API=vulkan&& cargo test --locked -j 3 -p wcontainers-gpu-webgpu
  --no-fail-fast -- --test-threads 1 & set WEBGPU_API=dx12&& cargo test --locked -j 3 -p
  wcontainers-gpu-webgpu --no-fail-fast -- --test-threads 1"`. Keep `--no-fail-fast`, so one
  failing file does not hide the others, and `--locked`, so a stale lock file fails instead of
  being rewritten (which makes the tree differ and gpu-job refuse it).
- When a result is recorded, start the build command with `cargo clean -p wcontainers-gpu-webgpu
  -p wcontainers-gpu-test &&` (incremental builds on the desktop have kept stale libraries of path
  crates changed by a merge), and before `web/check` run `tools/graphics-rig/remote "cargo clean -p
  wcontainers-gpu-webgpu -p naga -p wcontainers-gpu -p wcontainers-gpu-test --target
  wasm32-unknown-unknown & cargo clean -p wcontainers-gpu-webgpu -p naga -p wcontainers-gpu -p
  wcontainers-gpu-test"`.
- The BC tests with other blocks: `set WEBGPU_SEED=<n>&& set WEBGPU_CASES=<textures per
  format>&& cargo test --locked -j 3 -p wcontainers-gpu-webgpu --test blocks -- --nocapture`.
- The BC comparison with Direct3D and WARP with other blocks: `set WEBGPU_SEED=<n>&& set
  WEBGPU_CASES=<textures per format>&& cargo test --locked -j 3 -p wcontainers-gpu-webgpu --test
  direct3d bc_ -- --nocapture` as the job's command (prints each format's distances).
- The prepare timings: `tools/graphics-rig/gpu-job --alone --build "cargo test --locked -j 3 -p
  wcontainers-gpu-webgpu --no-run" -- "set WEBGPU_API=dx12&& cargo test --locked -j 3 -p
  wcontainers-gpu-webgpu --test prepare -- --nocapture"` (`WEBGPU_PROGRAMS=<n>` programs per half,
  default 16).
- The suite's present test on this Mac: `--test present` in the command below (the window shows
  for a moment in the middle of the main screen; the session needs the screen recording
  permission, which this one has).
- The converted formats with other texels: `set WEBGPU_SEED=<n>&& set WEBGPU_CASES=<textures per
  format>&& cargo test --locked -j 3 -p wcontainers-gpu-webgpu --test converted -- --nocapture`,
  and against Direct3D 11 the same with `--test direct3d converted` (defaults seed 1, 4 textures).
- Instance data fetched in the program against vertex buffers, timed:
  `tools/graphics-rig/gpu-job --alone --build "cargo test --release --locked -j 3 -p
  wcontainers-gpu-webgpu --test pulled --no-run" -- "set WEBGPU_API=vulkan&& cargo test --release
  --locked -j 3 -p wcontainers-gpu-webgpu --test pulled -- --nocapture"` (`WEBGPU_STREAMS=4,8,16`,
  `WEBGPU_FRAMES=20`).
- The vertex conversion with other bytes: `set WEBGPU_SEED=<n>&& set WEBGPU_CASES=<vertices per
  format>&& cargo test --locked -j 3 -p wcontainers-gpu-webgpu --test vertices -- --nocapture`.
- On this Mac's GPU, one job only (never a build loop): copy the committed tree to scratch with
  `git archive HEAD | tar -x -C <scratch>/mac-src`, and there `nix develop <checkout> -c env
  CARGO_TARGET_DIR=<scratch>/mac-target WEBGPU_API=metal cargo test --locked -j 2 -p
  wcontainers-gpu-webgpu --no-fail-fast -- --test-threads 1`, detached with its own log.
- The random byte test with other seeds and lengths: `set WEBGPU_SEED=<n>&& set WEBGPU_CASES=<n>&&
  cargo test -j 3 -p wcontainers-gpu-webgpu --test writes byte_granular -- --nocapture` as the job's
  command (it prints the seed, the count, and how many no-overwrite writes it made).
- The literal check: commit first, then run it at HEAD in the desktop's LF worktree (the
  desktop's own checkout is CRLF, which changes the keys of multi-line strings, and has nested
  submodules the other tracks' baselines do not scan): `tools/graphics-rig/remote "cd /d
  C:\wc\src\phase-2-graphics-webgpu-lf && git -c core.autocrlf=false checkout -q -f --detach
  <commit> && git -c core.autocrlf=false submodule update --init -q
  crates/wcontainers-shaders/dxbc-spirv crates/wcontainers-shaders/dxil-spirv && git -C
  crates/wcontainers-shaders/dxil-spirv -c core.autocrlf=false submodule update --init -q
  third_party/spirv-headers && set GIT_CONFIG_COUNT=1&& set GIT_CONFIG_KEY_0=core.autocrlf&& set
  GIT_CONFIG_VALUE_0=false&& set RAYON_NUM_THREADS=3&& cargo run -j 3 -q -p wcontainers-literals
  -- check"` (scratch lit-check.sh does exactly this at HEAD). It must print no `+`, `-`, `~`,
  `>` or `!` line. If the worktree is missing: `git -c core.autocrlf=false worktree add --detach
  C:\wc\src\phase-2-graphics-webgpu-lf <commit>` in C:\wc\src\phase-2-graphics-webgpu.
- Lint: `tools/graphics-rig/remote cargo clippy --locked -j 3 -p wcontainers-gpu-webgpu
  --all-targets -- -D warnings`, and the same with `--target wasm32-unknown-unknown --examples` for
  the WebAssembly build.
- Both ways against Direct3D, one command: `crates/wcontainers-gpu-webgpu/web/check [dir]`. One
  GPU job builds the scenes example natively and for WebAssembly, gt-compare and the rig's page
  driver packages, then (web/check.cmd) runs wasm-bindgen, draws the three scenes and the quads on
  `depth24plus-stencil8` with wgpu on Direct3D 12 and on Vulkan, draws them in Chrome
  (web/draw.mjs, web/scenes.html), compares all twelve with references/d3d11 at tolerance 0, and
  draws both quads pictures with each twist natively on both APIs and in Chrome and requires them
  to differ, draws the eight-viewport scene in Chrome (equal to `raster-viewports.png`, and
  differing with whole-pixel viewports), prints first-draw timings with and without `prepare` in
  Chrome (failing if the prepared draws built anything), checks timestamps, the converted formats
  (examples/scenes/converted.rs, seed 1, 4 textures each), the vertex formats read through shadows
  (examples/scenes/vertices.rs, seed 188, 64 vertices each) and texture renaming
  (examples/scenes/renames.rs), query sets past 4096 and the query ring
  (examples/scenes/queries.rs), BGRA vertices (vertices.rs), prepared strips (prepare.rs) and
  copies between `depth16unorm` and its group (depth16.rs), texture reads into buffers keeping
  the bytes between rows (gaps.rs), three-channel copies at the widest texture (converted.rs),
  subgroups and the primitive index where Chrome's adapter offers them and fine derivatives
  always (features.rs), sRGB views of storage textures (views.rs), float clears' bits
  (clears.rs) and buffer copies into layers lying apart (gaps.rs) in
  Chrome, prints the instance data routes' timings in Chrome
  (examples/scenes/pulled.rs, failing if their pictures differ), then runs the shared suite
  in Chrome, one page per test (suite-chrome.txt). Where Chrome's first launch gets no WebGPU
  adapter at all (seen about one launch in four), draw.mjs exits with check.cmd's `NO_ADAPTER`
  and is launched again, up to `LAUNCHES` (3) times. Prints
  Chrome's WebGPU adapter and fails unless it is a GPU, not a fallback. With a directory, the
  pictures and the suite's results are copied there. About 7 minutes cold, the job itself 15 s.
- Where real programs' frame time goes: `crates/wcontainers-gpu-webgpu/frames/run [--real]
  [--short n] [--long n] <program>...` (programs as the rig names them; `--real` draws through
  real Direct3D for comparison). It needs samply 0.13.1 in C:\wc\tools and an SSH session that
  may profile the system (the queue's desktop session may not: wpr "Failed to enable the policy").
  Each program is three GPU jobs, each alone in the queue, and two traces of the whole machine,
  about 1 to 10 minutes. After each recorded run it prints the whole machine's load during that
  job (`typeperf` Processor(_Total) each second over SSH, our program included; samples kept in
  scratch frames-load); at the end, per frame, the CPU by part and place and the back end's
  counts a frame from the long run's log (`FRAME_REPORT` lines, frames 128 to 256 of a 360-frame
  run).
- Where this back end's time goes in Chrome on a real program's frames: build the WebGL track's
  replay page (`crates\wcontainers-gpu-webgl\browser\build.cmd`, its How to run), then a GPU
  job `node crates\wcontainers-gpu-webgpu\web\replay-profile.mjs
  %CARGO_TARGET_DIR%\webgl-replay-pkg C:\wc\out\webgl\trace\<program>.bin.1 <out-file>`: the
  replay page's lines, then V8's samples taken inside this back end by interface call and by
  function.
- Divisions in a program against Direct3D 11's: `--test direct3d divisions -- --nocapture`
  (`WEBGPU_SEED`, `WEBGPU_CASES` draws of 64 pairs, default 1 and 4).
- Timestamps in Chrome as users have it, alone: after web/check has built the page, a GPU job
  `node crates\wcontainers-gpu-webgpu\web\plain.mjs C:\wc\out\phase-2-graphics-webgpu-webgpu\pkg`
  (web/check runs it too).
- The old one-step BC comparison (ignored): add `--ignored` to the test command (`--test blocks --
  --ignored --nocapture`); it prints each format's largest difference and the texel.
- One renderer by hand, on the desktop: `scenes <dx12|vulkan|metal> <dir> [none|blend-factor|
  depth-function]` (the example's binary).

## Measurements

Run 27 (G2d):
- In the browser, the WebGL track's figure (its status, Measurements, runs G3b-2 and G3b-6; not
  measured here): SimpleLightingPC's recorded frames (4 draws, 2 passes a frame) replayed in
  Chrome 157 on the RTX 3080 cost this back end 53.0 to 53.9 us a draw and 62.9 us a
  `begin_pass` (whole frame 0.73 to 0.78 ms), against 23 us a draw on the WebGL 2 back end. The
  native breakdowns above and below (0.01 to 0.04 ms a frame of the back end's own code) do not
  cover the browser.
- The same frames replayed here alone in the queue, under V8's sampling profiler
  (web/replay-profile.mjs at 7a6c2dde, 2026-11-19 15:53, `gpu-job --alone --desktop`, Chrome
  157.0.8103.78, Dawn on Direct3D 12, RTX 3080; scratch g2d/profile1.log; load not taken): this
  back end 31.2 us a draw, 37.7 us a `begin_pass`, 53.0 us `current_texture`, 52.1 us `submit`,
  whole frame untimed median 0.225 ms; WebGL 2 16.5 us a draw, 0.130 ms a frame. So alone the
  gap is smaller than in the WebGL track's run (53 us), still about twice. Where the time inside
  this back end's replay goes (315 ms over both replays, 50 us samples, self time): 87 ms in
  wasm-bindgen's JavaScript glue and 24 ms crossing from WebAssembly into it (Chrome's WebGPU
  calls themselves are counted there: the profiler shows no native frames), then the calls by
  name: `submit` 11.6 ms, `onSubmittedWorkDone` 10.8 (fences), `createBindGroup` 9.0,
  `createCommandEncoder` 3.5, `drawIndexed` 2.8, `draw` 1.4; `performance.now` 12.3 (the timed
  replay's own clock); allocation 9.5 (malloc and free); building JavaScript descriptor objects
  (`Object::new` 3.5, wgpu's `begin_render_pass` 2.9 and its colour attachments 2.4); this back
  end's own functions a few ms each (`begin_pass` 5.0, `draw` 2.6, `write_buffer` 2.2,
  `set_group` 2.0, hashing about 3.7). So in the browser the cost is the calls into Chrome's
  WebGPU and the objects wgpu builds for them, not the back end's own logic; cutting it means
  fewer calls (bind groups and command encoders made less often, fewer fence queries), which is
  this back end's to try next (Next 12). A second run with the profile grouped by interface call
  (the grouping's pattern fixed after the first run missed the crate hashes in the names) got no
  result from the page within 300 s (profile2.log), so that grouping is untested.
- atg-simplecomputepc's division (the G2c check's finding; tests/direct3d.rs
  `divisions_match_direct3d`, 2026-11-19 15:18, desktop shared with other tracks' queued jobs,
  not alone, load not taken; scratch g2d/after2.log): 64 draws of 64 seeded pairs (seed 1, the
  first pair the sample's 1200 and 300), each divided by its run-time divisor and by 300 written
  in the program, with fractions, through this back end on Vulkan and on Direct3D 12 against
  real Direct3D 11 (FXC) on the RTX 3080: 0 of 4096 pairs differ in any bit on either API. Real
  Direct3D 11 itself gives 1200 / 300 = 3.9999998 (fraction 0.99999976) when the divisor is a
  run-time value and 4.0 (fraction 0) when it is written in the program. So this back end, wgpu
  and the drivers divide exactly as Direct3D does; the sample's last texel comes from its
  converted program dividing at run time where its bytecode's arithmetic gives 4.0 (the
  converter's or the front's), not from this back end. Refused here by name: not this back end's.
- atg-simpleinstancingpc's breakdown repeated after the merges (the G2c check's finding;
  `frames/run atg-simpleinstancingpc` at bd160c74, Direct3D 11 front, wgpu on Direct3D 12; each
  job alone in the queue; scratch g2d/frames-instancing.log; the rig's "frames differ" mark as in
  runs 21 to 26, the frames drawn). Load: short run 20.5 % (4.2 to 74.9, over the 280 s its job
  waited and ran), long run 4.9 % (2.4 to 9.1, 7 s). Per frame over 300 frames: wall 16.451 ms
  (the display), CPU 79.527 ms: program 0.137, front 0.047, back end and naga under 0.01 (each
  -0.003, the long run less the short), wgpu 0.023, driver 0.073, system 79.253. Under the back
  end's own functions 0.133 ms (driver 0.063, system 0.077: present, submit, `memcpy` in
  `write_buffer_impl`). Not ours: 65.710 ms in the Direct3D 12 driver's threads spinning, 13.950
  in the program's. Counts a frame: 5 `MappedUploadWhole`, 935,856 bytes into buffers, 3
  pipeline binds, the same as run 22. So run 22's picture stands: nothing of ours costs more
  than 0.05 ms a frame there.

Run 26 (G2c), frame breakdowns at bf7ddf71 (every graphics branch merged; no code of this
crate changed). Desktop (i9-9900K, 16 threads, RTX 3080, driver 621.01), each job alone in the
queue, load by typeperf over each recorded job from its "alone from"; log scratch
run26/frames.log. The rig again marked bgfx FAIL for nvapi64.dll loaded from System32 and
chocolate-doom "frames differ", as in runs 21 to 25; both drew their frames.
- bgfx-01-cubes-d3d12 (Direct3D 12 front, wgpu on Vulkan): load short run 9.6 % (6.7 to 11.1,
  3 s), long run 12.3 % (5.1 to 22.4, 22 s). Per frame over 300 frames: wall 15.793 ms, CPU
  209.463 ms: program 0.287, front 0.450, back end 0.037, wgpu 0.060, naga and driver near zero
  (-0.007 and -0.040, the long run less the short), system 208.677. Under the back end 3.223 ms,
  of it 3.063 ms `memcpy` (vcruntime140) in `write_buffer_impl`: 14,680,328 bytes a frame in 4
  `MappedUploadWhole` writes, as in runs 21 and 22 (the Direct3D 12 front's whole-buffer
  uploads, Next 6). The GPU thread: 10.227 ms in ntdll under wgpu's Vulkan `acquire_texture`
  (the display wait, Next 7). Not ours: 98.887 ms in the NVIDIA Vulkan driver's threads, 66.333
  in its Direct3D 12 driver's, spinning. Counts a frame: 251 pipeline binds, 125
  `PixelInvocationsCounted`, 1 `UnalignedBufferEdge`, as in run 22.
- chocolate-doom (Direct3D 11 front, wgpu on Direct3D 12): load short run 6.9 % (5.0 to 10.4,
  4 s), long run 10.3 % (3.6 to 19.1, 34 s). Per frame over 300 frames: wall 0.803 ms, CPU
  3.747 ms: program 0.230, front 0.030, back end 0.013, wgpu 0.027, naga 0.003, driver 0.323,
  system 3.120 (2.317 of it ntdll in a thread of the NVIDIA Direct3D 12 driver). Under the back
  end's own functions 0.100 ms (back end 0.013, wgpu 0.017, driver 0.050, system 0.020). Counts a
  frame: MappedUploadWhole 3, bind_groups 1, buffer_bytes 496, pipeline_binds 3, renames 1,
  spares_reused 1, texture_bytes 256,000, the same as runs 21 to 25.
- What is ours and cheap: nothing new. The back end's own code is 0.01 to 0.04 ms a frame in
  both; atg-simpleinstancingpc's breakdown (the third program) stands from run 22 (repeated in
  run 27, the same).

Run 25 (G2c), chocolate-doom's frame breakdown at 5bdc9a93 (after this run's merges and the
frame report through serde; Direct3D 11 front, wgpu on Direct3D 12). Desktop (i9-9900K, 16
threads, RTX 3080, driver 621.01), each job alone in the queue; log scratch run25/frames-doom2.log.
The first try (frames-doom.log) stopped at `wpr -start` ("The profiles are already running"):
another session was recording the system; this run waited until `wpr -status` showed it free.
"FAIL, frames differ" is the rig's frame comparison of chocolate-doom, as in runs 22 and 23.
- Load (typeperf, whole machine, from each job's "alone from"): short run 19.7 % mean (4.9 to
  96.6) over 153 s; long run 7.5 % (3.5 to 10.7) over 7 s.
- Per frame over 300 frames: wall 0.399 ms, CPU 1.680 ms: program 0.227, front 0.073, back end
  -0.007 (the long run less the short, so near zero comes out signed), wgpu 0.007, driver 0.297,
  system 1.083 (1.263 of it ntdll in a thread of the NVIDIA Direct3D 12 driver). Under the back
  end's own functions 0.003 ms. Counts a frame (read through the new serde frame report):
  MappedUploadWhole 3, bind_groups 1, buffer_bytes 496, pipeline_binds 3, renames 1,
  spares_reused 1, texture_bytes 256,000, the same as runs 21 to 23. Our own code still costs
  hundredths of a millisecond a frame; nothing new on our side to fix.

Run 23 (G2c), chocolate-doom's frame breakdown repeated at 9d7b6b0c (after this run's merges;
Direct3D 11 front, wgpu on Direct3D 12). Desktop (i9-9900K, 16 threads, RTX 3080, driver
621.01), each job alone in the queue; log scratch run23/frames-doom3.log. The first two tries
stopped at `wpr -start` ("The profiles are already running"): another session on the desktop was
recording the system then (wpr -status showed a recording in progress that none of this run's
commands had started); it was left alone and the third try waited until wpr was free.
- Load (typeperf, whole machine): short run 73.0 % mean (3.6 to 100) over its 81 s; long run
  20.0 % (3.4 to 41.2) over 315 s, but that span is gpu-job's "started" to "ended" and includes
  about 5 minutes of the job waiting for the alone slot (the program ran from 21:50:10 to
  21:50:18), so it mostly measures the wait. frames/run now starts the span at the job's "alone from" line
  (its time patterns written with digit classes, three script lines fewer in the baseline), not
  yet run since.
- Per frame over 300 frames: wall 0.594 ms (run 22: 2.435), CPU 1.690 ms: program 0.287, front
  0.020, back end 0.007, wgpu 0.010, system 1.677 (1.767 of it ntdll in a thread of the NVIDIA
  Direct3D 12 driver, nvwgf2umx.dll), driver -0.310 (the per-frame figure is the long run less the
  short one, so a part that varies between the two runs can come out negative). Under the back
  end's own functions 0.090 ms (back end 0.007, driver 0.050, system 0.033). Counts a frame:
  MappedUploadWhole 3, bind_groups 1, buffer_bytes 496, pipeline_binds 3, renames 1,
  spares_reused 1, texture_bytes 256,000. Same picture as run 22: our own code costs hundredths
  of a millisecond a frame; nothing new on our side to fix.

Run 22 (G2c), the frame breakdown at 62473782, with the load sampled within each recorded run.
Desktop (i9-9900K, 16 threads, RTX 3080, driver 621.01), shared with the other tracks: each
program's three jobs ran alone in the queue (`--alone`), so no other GPU job ran beside them;
other tracks' builds over SSH could. Load: the whole machine's `\Processor(_Total)\% Processor
Time` each second by `typeperf`, over each recorded job's own span (start-up and our program
included). Logs run22-frames2.log (doom) and run22-frames3.log (atg, bgfx; the first try at them
ended with exit 255, the SSH client's failure code, after atg's short run, at the step stopping
the recorder; not looked into further), samples in scratch frames-load. Counts a
frame from the long run's log, frames 128 to 256.
- chocolate-doom (Direct3D 11 front, wgpu on Direct3D 12): wall 2.435 ms a frame (uncapped; run
  21 gave 0.258), CPU 10.203 ms a frame: program 0.250, front 0.020, wgpu 0.020, shaders 0.003,
  driver 0.360, system 9.550; the back end's own code was never the innermost function sampled.
  Under the back end's functions 0.097 ms (driver 0.073, system 0.020, wgpu 0.003). Not ours:
  4.437 ms in ntdll in a thread the trace does not name, 3.363 ms in threads of the NVIDIA
  Direct3D 12 driver (nvwgf2umx.dll), spinning. Counts a frame as in run 21: 3
  `MappedUploadWhole`, 1 bind group, 1 rename from the pool, 3 pipeline binds, 496 bytes into
  buffers, 256,000 into textures. Load: short run 9.0 % (5.2 to 17.1, 6 s), long run 9.5 % (0.7 to
  27.4, 179 s).
- atg-simpleinstancingpc (Direct3D 11 front): wall 16.540 ms a frame (the display), CPU 80.287
  ms: front 0.037, back end 0.010, wgpu 0.027, naga 0.003, driver 0.217, program 0.050, system
  79.943. Under the back end 0.140 ms (driver 0.107, the present and buffer creation most of it).
  Not ours: 66.163 ms in the Direct3D 12 driver's threads spinning, 13.850 ms in the program's.
  Counts a frame: 5 `MappedUploadWhole`, 3 pipeline binds, 935,856 bytes into buffers. Load:
  short 8.4 % (5.6 to 17.2, 5 s), long 17.2 % (1.5 to 25.2, 93 s).
- bgfx-01-cubes-d3d12 (Direct3D 12 front, wgpu on Vulkan): wall 17.865 ms a frame, CPU 225.460
  ms: front 0.433, back end 0.063, wgpu 0.100, naga 0.007, driver 1.443, program 0.277, system
  223.143 (two parts print as -0.003 ms: the tool takes the short run from the long). Under the
  back end 3.107 ms, of it 2.950 ms `memcpy` in `write_buffer_impl`: 14,680,328 bytes a frame in 4
  `MappedUploadWhole` writes, as in run 21. The GPU thread: 10.377 ms in ntdll under wgpu's Vulkan
  `acquire_texture` (the display wait). Not ours: 100.600 ms in the NVIDIA Vulkan driver's threads
  (nvoglv64.dll), 66.803 ms in its Direct3D 12 driver's, spinning. Also 251 pipeline binds, 125
  `PixelInvocationsCounted` and 1 `UnalignedBufferEdge` a frame. Load: short 9.4 % (5.8 to 19.0,
  21 s), long 12.3 % (3.6 to 19.9, 53 s). The rig again marked the runs FAIL for nvapi64.dll
  loaded from System32 (run 21); the frames drew.
- What is ours and cheap: nothing new. The back end's own code is 0.01 to 0.06 ms a frame; the
  one large cost under it is copying what the fronts hand it, bgfx's 14.7 MB a frame, which the
  Direct3D 12 front can cut (Next). The rig marks doom and atg FAIL "frames differ" (no reference
  frames for these captures), as in run 21; it does not bear on the timings.

Run 21 (G2c), the frame breakdown again with the back end's counters and the machine's load.
Desktop (i9-9900K, 16 threads, RTX 3080, driver 621.01), shared with the other tracks: each
program ran alone in the queue's hazard group; builds and other tracks' remote commands could run
beside. Load: the whole machine's `\Processor(_Total)\% Processor Time` sampled each second by
`typeperf` over SSH beside the runs, averaged over each recorded run's job (start to end, so it
includes start-up and our program itself). Method otherwise as run 19 (frames/run; logs
run21-frames2.log for atg and bgfx, run21-frames3.log for doom; load samples run21/load-run21*.csv).
Counts a frame are from the long run's log, frames 128 to 256 (`FRAME_REPORT`).
- chocolate-doom (Direct3D 11 front, wgpu on Direct3D 12): wall time 0.258 ms a frame (it runs
  uncapped; the first try of this run gave 2.485 ms, run 19 0.96: this number moves with the
  machine), CPU 3.553 ms a frame. Ours: front 0.037 ms, back end 0.013, wgpu 0.027; under the back
  end's functions 0.107 ms. Not ours: 3.537 ms in threads of the NVIDIA Direct3D 12 driver
  (nvwgf2umx.dll) spinning in ntdll. Counts a frame: 3 `MappedUploadWhole` (the front), 1 bind
  group, 1 buffer rename (from the pool), 3 pipeline binds, 496 bytes into buffers, 256,000 bytes
  into textures (the 320x200 frame). Load: the recorded runs fell after the sampling window
  (it covered 16:49 to 17:09, mean 25.7 %, 2.9 to 100 %; the runs were 17:11:47 and 17:14:56):
  not measured for these two runs.
- atg-simpleinstancingpc (Direct3D 11 front): wall 16.552 ms a frame (the display), CPU 79.3 ms.
  Ours: front, back end and wgpu under 0.03 ms each; under the back end 0.163 ms (the driver's
  present and buffer creation most of it). Not ours: 66.1 ms in the Direct3D 12 driver's threads
  spinning, 13.7 ms in the program's. Counts a frame: 5 `MappedUploadWhole`, 3 pipeline binds,
  935,856 bytes into buffers, no texture bytes. Load: short run mean 23.7 % (6.9 to 32.3; the
  last 80 s of its job only), long run 10.7 % (1.9 to 19.1).
- bgfx-01-cubes-d3d12 (Direct3D 12 front, wgpu on Vulkan): wall 16.841 ms a frame, CPU 218 ms.
  Ours: front 0.350 ms, back end 0.033, wgpu 0.050; under the back end 3.223 ms, of it 3.037 ms
  `memcpy` in `write_buffer_impl`. That copy is now sized: 14,680,328 bytes a frame into buffers,
  in 4 `MappedUploadWhole` writes (the front writing whole upload buffers each submit), about
  4.8 GB/s; also 251 pipeline binds, 125 `PixelInvocationsCounted` and 1 `UnalignedBufferEdge` a
  frame. The GPU thread: 10.35 ms in ntdll under wgpu's Vulkan `acquire_texture` (the display
  wait, run 20). Not ours: 100.3 ms in the NVIDIA Vulkan driver's threads, 67.0 ms in its Direct3D
  12 driver's, spinning. Load: short 19.3 % (8.2 to 30.1), long 15.2 % (6.9 to 36.8). The rig
  marked each bgfx run FAIL because nvapi64.dll was loaded from C:\Windows\System32 as well as
  the empty one beside the program (as in run 19, whose bgfx breakdown came from such runs too).
  Which module loaded the system copy was not found (expected: NVIDIA's Vulkan driver, which this
  front draws through here, not bgfx); the frames themselves drew.
- What is ours and cheap: nothing new. The back end's own functions take 0.01 to 0.03 ms a frame;
  the one large cost under them is copying the bytes the fronts hand it, 14.7 MB a frame in bgfx,
  which the Direct3D 12 front can cut by writing only what the frame's bindings and copies read
  (Next). frames.rs's first load figure (other processes' samples in the trace) counted more than
  the machine has and was dropped (e54cf4cb).

Run 19 (G2c), where a real program's frame time goes on our side. Desktop (i9-9900K, RTX 3080,
driver 621.01), shared with the other tracks: each program ran alone in the GPU queue (hazard
group), but builds and other tracks' remote commands ran beside; load not measured further.
Method (crates/wcontainers-gpu-webgpu/frames/run and examples/frames.rs): the fronts' libraries
built in release with line tables; each program run once unrecorded, then to frame 60 and to
frame 360 under Windows' recorder (wpr's CPU profile, a sample with stacks every millisecond on
every thread of the process), converted by samply; a frame's share is the difference of the two
runs over the 300 frames between, so start-up and shut-down cancel. Each sample goes to the
innermost frame of a known part; our libraries are named from their PDBs (code inlined into a
function counts as that function's), system and driver code by library, with the function of
ours that called it, or the library that started its thread. Logs: scratch
logs/run19-frames-ours.log, -ours2.log and -real.log.
- chocolate-doom (Direct3D 11 front, wgpu on Direct3D 12): 0.96 ms of wall time a frame (real
  Direct3D 11: 0.30), CPU 4.0 ms a frame (real: 0.74). Ours: front 0.010 ms, back end 0.013,
  wgpu 0.013, and under the back end's functions 0.097 ms in all (driver 0.050, system 0.030;
  the largest single place a `memcpy` in `write_texels`, 0.023 ms, the frame's texture upload).
  The rest is not ours: 2.74 ms in threads the NVIDIA Direct3D 12 driver (nvwgf2umx.dll) started,
  spinning in ntdll (real Direct3D 11's driver threads: 0.42 ms), 0.28 ms the program and SDL.
- atg-simpleinstancingpc (Direct3D 11 front): 16.8 ms of wall time a frame (paced by the
  display). Ours: front 0.027 ms, back end 0.010, wgpu 0.010, under the back end 0.130 ms (the
  driver's present and submit most of it). Not ours: 66.8 ms of CPU a frame in the Direct3D 12
  driver's threads spinning (about four threads busy all the time) and 13.8 ms in the program's.
- bgfx-01-cubes-d3d12 (Direct3D 12 front, wgpu on Vulkan): 16.6 ms of wall time a frame. Ours:
  front 0.41 ms (`Replay::draw` 0.19 the largest), back end 0.057, wgpu 0.070, and under the
  back end 3.59 ms, nearly all of it 3.40 ms of `memcpy` in `write_buffer_impl` (the bytes the
  front writes each frame, copied into the staging ring; natively wgpu copies them once more,
  see "Not carried out yet"). The GPU thread (wcontainers-gpu) was busy the whole run: 9.98 ms a
  frame spinning in ntdll under wgpu's Vulkan `acquire_texture` (the NVIDIA Vulkan driver waits
  for the display's next image by spinning) and 1.37 ms in ntdll on its own events. Not ours:
  99.5 ms of CPU a frame in the NVIDIA Vulkan driver's (nvoglv64.dll) threads and 66.5 ms in its
  Direct3D 12 driver's threads, spinning; the program's two threads busy all the time (win32u
  and the kernel).
- Not measured: godot-platformer-2d-d3d12 (its two runs' lengths differed so much that the
  per-frame CPU came out above what 16 threads can give: 141 ms a frame in 4.7 ms of wall time,
  so the difference is not one of frames; its parts of ours, front 0.87 ms, back end 0.20, wgpu
  0.18, are printed but not taken as measured), bgfx-38-bloom (stops at start through the
  Direct3D 11 front, exit 3, the front's known refusal), and bgfx-01-cubes-d3d12's first pair
  (the short run lasted longer than the long one).
- What is ours and cheap: nothing found in the back end's own functions (0.01 to 0.06 ms a
  frame). The two costs of ours that are large are not cheap: the buffer bytes copied per frame
  (3.4 ms in bgfx; the front writes the bytes the program wrote and the back end copies them
  into its staging ring: fewer bytes need the front to write only what changed, a smaller copy
  needs a mapped ring wgpu 30 does not give natively), and the GPU thread spinning in the Vulkan
  driver's acquire (a present mode or acquiring later in the frame; not tried). Both are under
  Next. The CPU the drivers' own threads burn is several times what real Direct3D 11's driver
  burns for the same program (doom: 2.74 against 0.42 ms a frame), the cost of drawing through
  Direct3D 12 and Vulkan drivers natively; in a browser the GPU process draws instead.

Run 17 (desktop: Windows 10, i9-9900K, RTX 3080, NVIDIA 621.01, shared with other tracks' jobs,
CPU load not sampled; Mac load average near its 18 cores, not sampled exactly; scratch run17):
- All tests at 44a9c8b2 after `cargo clean` of this crate and the suite crate (full-native.log):
  both APIs together 215 passed, 0 failed, 5 ignored (run 16's 212 and 4, plus the two new query
  tests on each API, and the point fill test now reported ignored on Direct3D 12, where it
  passed having checked nothing); the shared suite 39 of 39 on both. The query tests again at
  c1948d84 (queries2.log): pass on both. Clippy with warnings denied passes natively and for
  WebAssembly (clippy2.log).
- Query sets of 5000 (tests/queries.rs): occlusion 4094 to 4097 read 19738, 19738, 19738, 0 and
  the carried 4999 39476 on Vulkan (sample counts), 1, 1, 1, 0 and 2 on Direct3D 12 (binary, its
  carried query summing two parts); timestamps across the page boundary in order on both. Twenty
  frames of results from one query ring on both.
- web/check at c1948d84 (web-check2.log, pictures in run17/web; the first run, web-check.log at
  e4a9968b, passed too): twelve pictures equal to Direct3D 11's at tolerance 0, twists differ,
  viewports equal (rounded 2540 differ); Chrome 157's adapter the RTX 3080 (D3D12, discrete, not
  a fallback); timestamps 9.821 and 19.521 ms (ratio 1.99), GPU clock 303.790 ms apart with the
  page clock between 300.1 and 307.5 ms; prepared first draws built nothing; converted formats,
  vertex formats and renames pass; query sets of 5000 read 1, 1, 1, 0 and 2 with timestamps in
  order, and 20 frames of results right from the device's one query ring; the suite in Chrome 39
  passed, 0 failed.
- At the end of the run the desktop's build folder C:\wc\target\phase-2-graphics-webgpu (6.46 GB)
  and the Mac checkout's `cargo check` output (1.1 GB) were deleted; scratch holds no build
  output (logs and pictures, 344 MB).

Run 16 (desktop: Windows 10, i9-9900K, RTX 3080, NVIDIA 621.01; Mac load average near its 18
cores, not sampled exactly):
- All tests at cd3aa677 (the merges) after `cargo clean` of this crate and the suite crate
  (scratch run16/full-native.log, shared with other tracks' jobs; job 1 min 8 s): both APIs
  together 212 passed, 0 failed, 4 ignored (as run 15); the shared suite 39 of 39 on both. Clippy
  with warnings denied passes natively and for WebAssembly (run16/clippy.log).
- web/check at cd3aa677 (run16/web-check.log, pictures in run16/web): twelve pictures equal to
  Direct3D 11's at tolerance 0, twists differ, viewports equal (rounded 2540 differ); Chrome's
  adapter the RTX 3080 (D3D12, discrete, not a fallback); timestamps 9.821 and 19.551 ms (ratio
  1.99), GPU clock 302.504 ms apart with the page clock between 300.0 and 305.0 ms; prepared first
  draws built nothing; converted formats, vertex formats and renames pass; the suite in Chrome 39
  passed, 0 failed.
- The timestamp workload with wgpu's runtime checks switched off one by one (a temporary,
  uncommitted change; `gpu-job --alone`, run16/checks-exp.log), first stretch / second, Direct3D
  11's 9.43 / 18.90 ms each time: Vulkan, all checks 12.25 / 24.44 ms; no loop bounding 9.49 /
  18.94; no bounds checks 12.26 / 24.44; neither 9.49 / 18.93. Direct3D 12: all checks 14.47 /
  28.72; no loop bounding 9.51 / 18.94; no bounds checks 14.47 / 28.73; neither 9.51 / 18.95.
  So loop bounding is the whole difference on this loop of 2048 cheap rounds; this workload reads
  no buffers, so it says nothing about the bounds checks' cost elsewhere.
- At the end of the run the desktop's build folder C:\wc\target\phase-2-graphics-webgpu (6.12 GB)
  was deleted; no `cargo check` output and no scratch target folder was made on the Mac.

Run 15 (desktop: Windows 10, i9-9900K, RTX 3080, NVIDIA 621.01, shared with other tracks' jobs,
CPU load not sampled; Mac load average near its 18 cores, not sampled exactly):
- All tests at b517ab0e after `cargo clean` of this crate and the suite crate (scratch
  run15/full-native.log; job 1 min 51 s): both APIs together 212 passed, 0 failed, 4 ignored (the
  BC one-step comparison on each, 24-bit depth bias absent and the wireframe known failure on
  Vulkan); the shared suite 39 of 39 on both. That is run 14's 210 plus the new unit test on each
  API. The converted formats with 16 textures each (seed 1) pass on both APIs
  (run15/converted2.log), and against Direct3D 11 with 4. Clippy with warnings denied passes
  natively and for WebAssembly.
- web/check (run15/web-check2.log, pictures in run15/web): the first run failed in Chrome at the
  new 3D copy ("depthSlice is required for a 3D attachment": wgpu's browser back end dropped the
  depth slice; fixed in the vendored wgpu, Departures). After the fix: all twelve pictures equal
  Direct3D 11's at tolerance 0, twists differ as before, viewports equal (whole pixels 2540
  differ); Chrome's adapter the RTX 3080; timestamps 9.820 and 19.549 ms (ratio 1.99), GPU clock
  300.644 ms apart with the page clock between 300.1 and 304.3 ms; prepared first draws built
  nothing; converted formats seed 1, 4 textures each, within bounds, with every copy on the GPU;
  vertex formats and texture renames pass; the instance data routes draw alike; the suite in
  Chrome 39 passed, 0 failed.
- At the end of the run the desktop's build folder C:\wc\target\phase-2-graphics-webgpu (6.91 GB)
  and the Mac checkout's `cargo check` output (553 MB) were deleted; the next build there is cold.

Run 14 (desktop: Windows 10, i9-9900K, RTX 3080, NVIDIA 621.01, shared with other tracks' jobs
except where alone, CPU load not sampled; Mac load average near its 18 cores, not sampled exactly):
- Instance data, release build, `gpu-job --alone`, 256 draws of 16 instances, 20 frames a route,
  median of the 19 after the first, CPU to record and submit a frame / GPU time of the pass.
  Before the bind group change (scratch g2b4/pulled1.log), sets made per draw made 5120 bind
  groups: Vulkan 4 streams 1.249 ms, 8 streams 1.667 ms, 16 streams 2.526 ms CPU; Direct3D 12
  2.010, 2.082, 3.539 ms. After it (g2b4/pulled2.log), in ms CPU / GPU:
  - Vulkan, 4 streams: vertex buffers 0.304 / 0.012, a set per draw 0.533 / 0.017, sets kept
    0.282 / 0.017. 8 streams: 0.503 / 0.026, 0.721 / 0.024, 0.326 / 0.018. 16 streams: 0.898 /
    0.038, 0.980 / 0.020, 0.476 / 0.020.
  - Direct3D 12, 4 streams: 0.373 / 0.008, 0.604 / 0.007, 0.358 / 0.007. 8 streams: 0.552 / 0.011,
    0.793 / 0.007, 0.433 / 0.007. 16 streams: 1.089 / 0.016, 1.106 / 0.008, 0.582 / 0.008.
  - So fetching in the program with kept sets costs less CPU than vertex buffers at every count
    (each vertex buffer is a call per draw); GPU time is a little more at 4 streams on Vulkan
    (0.017 against 0.012 ms) and about half at 16 streams on this light scene; a set made per draw adds its lookup (0.25 ms a frame of 256 draws at 4 streams, 0.5 at 16,
    against kept sets), so on that route fetching costs more CPU than vertex buffers up to 8
    streams.
    Storage usage itself costs nothing here: every buffer already has it.
- In Chrome 157 (web/check, g2b4/webcheck1.log, not alone; `performance.now()` in 0.1 ms steps;
  the device's most is 8 streams there): 4 streams vertex buffers 0.3 / 0.016, a set per draw
  0.6 / 0.015, sets kept 0.2 / 0.015; 8 streams 0.3 / 0.029, 0.7 / 0.017, 0.2 / 0.019; 256 bind
  groups made by each pulled route.
- All tests at fded1660 after `cargo clean` of this crate and the suite crate (g2b4/full1.log):
  Vulkan 104 passed, 0 failed, 3 ignored (the BC one-step comparison, 24-bit depth bias absent,
  the wireframe known failure); Direct3D 12 106 passed, 0 failed, 1 ignored. The shared suite 39 of
  39 on both; timestamps 2 of 2 on both. Clippy with warnings denied passes natively and for
  WebAssembly.
- web/check at the status commit after fded1660, after cleaning this crate, naga, the interface
  and the suite crate on both targets (g2b4/webcheck1.log, pictures in g2b4/web): all twelve
  pictures equal Direct3D 11's at tolerance 0, the twists differ as before, viewports equal (whole
  pixels 2540 differ); Chrome's adapter the RTX 3080 (D3D12, discrete, not a fallback); timestamps
  9.820 and 19.552 ms (ratio 1.99), GPU clock 302.636 ms apart with the page clock between 300.1
  and 305.2 ms; prepared first draws built nothing; converted formats seed 1, 4 textures within
  bounds; vertex formats seed 188, 64 each; texture renames pass; the instance data routes draw
  alike; the suite in Chrome 39 passed, 0 failed.

Run 13 (desktop: Windows 10, i9-9900K, RTX 3080, NVIDIA 621.01, shared with other tracks' jobs,
CPU load not sampled; Mac load average near its 18 cores, not sampled exactly):
- Whole suite of this crate at 15512108 after `cargo clean` of it and the suite crate (scratch
  g2b3/full1.log): Vulkan 104 passed, 0 failed, 3 ignored (the BC one-step comparison, 24-bit
  depth bias absent, the wireframe known failure); Direct3D 12 106 passed, 0 failed, 1 ignored
  (the BC comparison). The shared suite 39 of 39 on both.
- web/check at the status commit after 15512108 (g2b3/webcheck3.log, pictures in g2b3/web): all
  twelve pictures equal Direct3D 11's at tolerance 0, twists differ as before, viewports equal;
  Chrome timestamps 9.816 and 19.547 ms (ratio 1.99), GPU clock 302.596 ms apart with the CPU
  between 300.0 and 303.0 ms; converted formats seed 1, 4 textures each, within bounds
  (`Bgrx8UnormSrgb` filtered 1.507 steps, as Direct3D's own); vertex formats seed 188, 64 each;
  texture renames pass; the suite in Chrome 39 passed, 0 failed. The run before it
  (webcheck2.log) failed only because Chrome gave the suite page no adapter at start (its GPU
  process); run again unchanged, it passed.
- Converted formats against Direct3D 11 (tests/direct3d.rs, seed 1, 4 textures per format, both
  APIs, g2b3/job3.log): every load and filtered texel equal to Direct3D's; Direct3D's own filtered
  texels from filtering `v / (2^n - 1)`: sRGB BGRX 1.507 steps, B5G6R5 0.714, B5G5R5A1 0.654, the
  others 0.002.
- Instance data fetched in the program, release build (`cargo test --release`), `gpu-job --alone`
  (g2b3/pulled-release.log), 256 draws of 16 instances, 4 streams a draw (one vertex, three
  instance), 20 frames: Vulkan vertex buffers 0.275 ms CPU and 0.017 ms GPU a frame (median of 19
  after the first), pulled 1.156 ms CPU and 0.020 ms GPU, first frame 7.8 against 30.2 ms CPU;
  Direct3D 12 0.341 against 1.354 ms CPU, 0.008 against 0.007 ms GPU, first frame 9.7 against
  19.6 ms. So in a release build the pulled route costs about four times the CPU time on both
  APIs, mostly its bind group per draw (5120 made in 20 frames); run 11's Direct3D 12 numbers
  (3.48 against 3.20 ms) came from a dev build of this crate (opt-level 1) and are superseded.
  More streams a draw: not measured yet (Next).
- Timestamps natively against Direct3D 11 (run 11, G2b check): our stretches 1.3 (Vulkan) and 1.5
  (Direct3D 12) times Direct3D 11's and Chrome's. The clock is not the cause: tests/timestamps.rs
  holds the GPU clock to the CPU's rate across about 300 ms on both APIs. The same loop takes
  longer through wgpu natively; expected cause, not measured: naga's loop bounding and bounds
  checks in the code it writes.

Run 12 (desktop: Windows 10, i9-9900K, RTX 3080, NVIDIA 621.01, shared with other tracks' jobs,
CPU load not sampled; Mac load average not sampled):
- web/check at 2087451 (scratch g2b2/webcheck.log, pictures and suite-chrome.txt in g2b2/web):
  Chrome's WebGPU on the RTX 3080 (D3D12 backend, discrete, not a fallback); the shared suite in
  Chrome 30 passed, 0 failed, `resolves_average_in_the_format_named` now through `ResolveByDraw`
  (no loss counted, linear-light values within one step); the twelve pictures equal to Direct3D
  11's at tolerance 0 and the twisted ones differing; converted formats in Chrome as natively (the
  same largest differences, FormatConverted 110); timestamps ordered, stretches 9.821 and 19.553
  ms (ratio 1.99), GPU clock 302.671 ms apart where the page clock had 300.1 to 306.2 ms.
- web/check again at 27411a30 (g2b2/webcheck2.log, g2b2/web2): the same, and the vertex formats read
  through shadows in Chrome (seed 188, 64 vertices each, three frames) all as they mean.
- All tests at 23d2b02's code after a clean build of this crate (scratch g2b2/native-full.log):
  Vulkan 91 passed, 2 ignored (the old BC comparison; the depth bias picture); Direct3D 12 92
  passed, 1 ignored. New: converted 1, direct3d 6 (with the converted comparison), the library's
  2 unit tests of src/packed.rs. Clippy with warnings denied passes natively for this crate and
  the interface, and for WebAssembly; the interface's 32 unit tests pass.
- All tests again at 94a9068f, the run's last code (g2b2/native-final.log): the same counts, Vulkan 91
  passed and 2 ignored, Direct3D 12 92 passed and 1 ignored, none failed.
- Converted formats against Direct3D 11 on the RTX 3080 (tests/direct3d.rs, both APIs, scratch
  g2b2/converted8.log and converted7.log): A8, BGRX, BGRX sRGB, both two-pixel formats, B5G6R5,
  B5G5R5A1, B4G4R4A4 loaded and filtered 0 steps from Direct3D's; Direct3D's own loads lie from
  `v / (2^n - 1)` by 2.8e-3 (B5G6R5), 2.66e-3 (B5G5R5A1), 3.75e-3 (BGRX sRGB, against the sRGB
  formula in f32), 0 for the rest. A8 and BGRX (both) drawn with blending: 0 bytes differ (BGRX's X
  apart, which Direct3D keeps as written: 128 of 128).

Run 11 (desktop: Windows 10, i9-9900K, RTX 3080, NVIDIA 621.01, shared with other tracks' jobs,
CPU load not sampled; Mac load average not sampled):
- All tests at 9cb45c3's code after a clean build of this crate (job 6): Vulkan 87 passed, 2
  ignored (the old BC comparison; the depth bias picture); Direct3D 12 88 passed, 1 ignored.
  bindings 12, blocks 3, copies 9, depth 2, direct3d 5, fitted 1, prepare 1, pulled 1, raster 3,
  scenes 4, suite 30, timestamps 2, vertices 1, writes 14 (Direct3D 12: point fill reported
  absent there, so its test returns early). Clippy with warnings denied passes natively for this
  crate and the interface, and for WebAssembly. The earlier job 1 found three failures, fixed
  before job 6: the Direct3D 11 reference drew nothing (its covering triangle was culled), the
  padded volume write on Direct3D 12, wireframe on Vulkan (now a known difference, Departures).
- Timestamps (tests/timestamps.rs, job 6 and the alone job): 16 then 32 draws of a 2048-round
  fragment loop over 512 by 512: Vulkan 12.25 ms and 24.62 ms, Direct3D 12 14.46 ms and 28.72 ms
  (ratio 1.99 to 2.01); the GPU clock moved 300.30 to 300.71 ms while the CPU's moved between
  300.03 and 304.61 ms (300 ms wait). Period without wgpu's conversion: 1 ns on both APIs.
- Against Direct3D 11 (tests/direct3d.rs, job 6): Direct3D 11's own timestamps (frequency 10^9)
  put the same work at 9.44 ms and 18.91 ms, in order; ours 12.26 / 24.69 ms (Vulkan) and 14.49 /
  28.77 ms (Direct3D 12): within the test's factor of two, slower by 1.3 and 1.5 times. Not looked
  into; Chrome's Dawn draws the same WGSL in 9.81 ms.
- In Chrome 157 (web/check, twice): 9.81 ms and 19.54 ms (ratio 1.99); the GPU clock moved 303.09
  ms while `performance.now()` moved between 300.0 and 306.0 ms. Adapter the RTX 3080 (D3D12,
  discrete, not a fallback).
- Query results across splits (tests/writes.rs): Vulkan [19738, 19738, 0] (counts: wgpu's Vulkan
  counts samples), Direct3D 12 [1, 1, 0] (binary occlusion).
- Wireframe: Direct3D 12 equals Direct3D 11's fill picture in every pixel; Vulkan 214 pixels differ,
  by up to 229. Point fill on Vulkan draws exactly the three corner pixels.
- Instance data fetched in the program (tests/pulled.rs, `gpu-job --alone`, 20 frames each): 256
  draws a frame, each of 16 instances with its own quad and three instance buffers, a binding set
  made per draw as a front makes it and dropped after. Vulkan: vertex buffers 0.313 ms CPU and
  0.012 ms GPU a frame (median of 19 after the first), pulled 1.366 ms CPU and 0.017 ms GPU; first
  frame 1.88 against 2.93 ms CPU. Direct3D 12: 3.48 against 3.20 ms CPU, 0.008 against 0.007 ms GPU
  (wgpu's Direct3D 12 recording dominates). The pulled route made 256 bind groups every frame
  (5120 in 20): binding sets no one holds are pruned at submit, so a front that drops its sets
  rebuilds their bind groups each frame; keeping them would avoid it (not measured). Storage usage
  itself costs nothing here: every buffer of this back end already has it. GPU time on this light
  scene is too small to show the fetch's own cost.
- `web/check` after cleaning this crate, naga, the interface and the suite crate for WebAssembly:
  all twelve pictures equal Direct3D 11's at tolerance 0, the twists differ as before (16384 pixels
  by up to 92; 25016 by up to 204), the viewports equal (whole pixels: 2540 by up to 229), `prepare`
  in Chrome 16 first draws 252.0 ms with 16 built, after prepare 3.9 ms with none built; the suite
  in Chrome 29 of 30 at 9cb45c3 (the resolve above), 30 of 30 after the fix.

Run 10 (desktop: Windows 10, i9-9900K, RTX 3080, NVIDIA 621.01, shared with other tracks' jobs,
CPU load not sampled; SSH to it refused connections from about 12:09 to 12:24 Mac time; Mac load
average 42 to 66):
- All tests at ad1a593 after a clean build of this crate and `wcontainers-gpu-test`: Vulkan 71
  passed, 2 ignored (the old BC one-step comparison; the depth bias picture, `DEPTH24_UNORM` being
  clear there); Direct3D 12 72 passed, 1 ignored (the BC one). bindings 12, blocks 2, copies 5,
  depth 2, direct3d 4, fitted 1, prepare 1, raster 1, scenes 4, suite 26 (two new from the shared
  line), vertices 1, writes 13. Clippy with warnings denied passes natively on Windows and for
  WebAssembly.
- This Mac (Apple M5 Max, Metal), single jobs from the committed tree: at c5fe867 every file
  passed but three tests (bindings 11 of 12, copies 4 of 5, writes 12 of 13; blocks 2 with one
  ignored, depth 1 with the bias picture ignored, fitted, prepare, present, raster, scenes 4,
  suite 23, vertices). The three failures were found here and fixed at ad1a593: a renamed texture
  then took the staged route's check and its next writes went to the queue ahead of the draws (each
  pixel read the frame's last write); Metal keeps a denormal copied into float depth and makes -0
  zero, the opposite of the RTX 3080; the test's arena buffers were not arena buffers. At ad1a593
  bindings 12, copies 5, writes 13 pass. Renames per frame, buffers and textures alike: (3, 0),
  (3, 3), (3, 3), (3, 3) (renames, of which from the pool). Float depth copied in, both routes:
  [3e800000, 3f800000, 0, 0, 00001234, 0, 3f800000, 3dfcd6ea] for the words 0.25, 2, -0.5, -0, a
  denormal, NaN, 1, 0.1234. The random byte test there (seed 0x5eed, 400 operations): 78 copies
  within one buffer, all counted.
- New seed on the RTX 3080: the random byte test, seed 4242 at 3000 operations on Vulkan (232
  no-overwrite writes, 93 made ordered, 514 copies within one buffer, all counted), and both BC
  tests, seed 4242 at 3 textures per format on Vulkan and on Direct3D 12: all pass.
- `web/check` after cleaning this crate, naga, `wcontainers-gpu` and `wcontainers-gpu-test` on both
  targets: the pictures, twists and viewports passed, but the suite in Chrome was 24 of 26: the two
  suite tests the shared line added were missing from examples/scenes/names.rs (the check caught
  it, as it is meant to; the Mac's suite run had silently run 23). Added at 0ab72939; then `web/check`
  passes: all twelve pictures equal Direct3D 11's at tolerance 0, the twists differ as before
  (16384 pixels by up to 92; 25016 by up to 204), the viewports equal (whole pixels: 2540 differ by
  up to 229), the suite in Chrome 26 of 26, Chrome's adapter the RTX 3080 (D3D12, discrete, not a
  fallback). `prepare` in Chrome: unprepared frame of 16 first draws 237.5 ms with 16 pipelines
  built while drawing; after 1.0 ms of `prepare` calls and 240.1 ms waiting for the compiles, the
  frame 4.5 ms with none built.
- This Mac at 0ab72939: suite 25 of 25 and present 1 of 1 (two frames through `PresentConverted`).

Run 9 (desktop: Windows 10, i9-9900K, RTX 3080, NVIDIA 621.01, shared with other tracks' jobs, CPU
load not sampled; Mac load average 29 to 75):
- All tests after the merge and a clean build of this crate and `wcontainers-gpu-test` (before
  the vertex work): Vulkan 65 of 65 and Direct3D 12 65 of 65, one ignored on each (the old BC
  one-step comparison): bindings 11, blocks 2, copies 4, depth 2, direct3d 4, fitted 1, prepare 1,
  raster 1, scenes 4, suite 24, writes 11.
- Last full run, at cf2f03c after a clean build of this crate and `wcontainers-gpu-test`: Vulkan
  66 of 66 and Direct3D 12 66 of 66, one ignored on each (as above, plus vertices 1). tests/vertices.rs: seed 188 at 64 vertices per format (both APIs), seed 7 at 1000
  (Vulkan) and seed 99 at 1000 (Direct3D 12), every one of the 11 formats, three frames each:
  every component as the format means (integers and 16-bit or smaller floats exact, normalised
  within 1e-6), the float beside it exact, one shadow made at the first frame, none at the second,
  one after the write.
- Clippy with warnings denied passes natively on Windows and macOS and for WebAssembly.
- `web/check` at e5be2a5 (after cleaning this crate, naga, `wcontainers-gpu` and
  `wcontainers-gpu-test` on both targets): all twelve pictures equal Direct3D 11's at tolerance 0,
  the twists differ as before (16384 pixels by up to 92; 25016 by up to 204), the viewports as
  before (equal; whole pixels 2540 differ by up to 229), the suite in Chrome 24 of 24, Chrome's
  adapter the RTX 3080 (D3D12, discrete, not a fallback). `prepare` in Chrome: unprepared frame of
  16 first draws 250.8 ms with 16 pipelines built while drawing; after 1.0 ms of `prepare` calls
  and 228.3 ms waiting for the compiles, the frame 1.5 ms with none built.
- This Mac (Apple M5 Max, Metal), single jobs from the committed tree (e5be2a5; build at `-j 2`
  with load average 35 to 71): every test passes, 62 of 62 with the one ignored: bindings 11, blocks
  2, copies 4, depth 2, fitted 1, prepare 1, present 1, raster 1, scenes 4, suite 23 (present runs
  apart), vertices 1, writes 11. The present test drew through `PresentConverted` (2 frames) and
  the window showed [51, 102, 153, 255] in all 16 pixels, as the suite asks. The earlier Mac run of this session (before the vertex
  work, the same pass list) printed the scene differences from the RTX 3080's Direct3D
  references: triangle 1952 pixels by 1, quads 2455 by 1 on every depth format, the eight
  viewports 662 by 1, all within the rig's allowance.

Run 8 (desktop: Windows 10, i9-9900K, RTX 3080, NVIDIA 621.01, shared with other tracks' jobs, CPU
load not sampled; Mac load average 21 to 79):
- All tests after the merges and a clean build of this crate and `wcontainers-gpu-test`, at
  8f7db1b's code plus the present change (the macOS host's later edits do not build on Windows):
  Vulkan 64 of 64 and Direct3D 12 64 of 64, one ignored on each (the old BC one-step comparison):
  bindings 10, blocks 2, copies 4, depth 2, direct3d 4, fitted 1, prepare 1, raster 1, scenes 4,
  suite 24 (present among them, so windows that take `rgba8unorm` are unchanged), writes 11;
  tests/present.rs has no tests there. Clippy with warnings denied passes natively on Windows and
  macOS and for WebAssembly.
- `web/check` at 4feccfa (after cleaning this crate, naga, `wcontainers-gpu` and
  `wcontainers-gpu-test` on both targets: a stale naga library failed the WebAssembly link before):
  all twelve pictures equal Direct3D 11's at tolerance 0, the twists differ as before, the
  viewports as before, the suite in Chrome 24 of 24 (present among them), Chrome's adapter the RTX
  3080 (D3D12, discrete, not a fallback). `prepare` in Chrome: unprepared frame of 16 first draws
  220.5 ms, 16 pipelines built while drawing; after 0.5 ms of `prepare` calls and 207.7 ms waiting
  for the compiles, the frame 3.8 ms with none built. Job 2 min 11 s.
- This Mac (Apple M5 Max, Metal), single jobs: the suite's other 23 tests pass under the new host;
  present fails as the known failure (window occluded, state 0x2000). The ScreenCaptureKit probe:
  256 of 256 colours read exactly (about 0.25 s a capture).

Run 7 (desktop: Windows 10, i9-9900K, RTX 3080, NVIDIA 621.01, shared with other tracks' jobs, CPU
load not sampled; Mac load average 18 to 64):
- Last full run, after a clean build with the final usage rule (binding and render usage for depth
  and stencil only with copy usages): Vulkan 64 of 64 and Direct3D 12 64 of 64, one ignored on
  each; clippy with warnings denied passes natively.
- All tests after a clean build of the crate, at af2d885's code: Vulkan 64 of 64 and Direct3D 12
  64 of 64, one ignored on each (the old BC one-step comparison): bindings 10, blocks 2, copies 4,
  depth 2, direct3d 4, fitted 1, prepare 1, raster 1, scenes 4, suite 24, writes 11. Every scene
  picture equal to Direct3D's in every pixel on both APIs.
- tests/direct3d.rs BC comparison, seed 188 one texture per format (Direct3D 12) and seed 7 four
  per format (Vulkan), largest channel differences over the seed-7 run: WARP from the GPU / the
  decoded from the GPU / the decoded from WARP: BC1 0.0196 / 0.0196 / 0.0078; BC1 sRGB 0.0269 /
  0.0273 / 0.0092; BC2 0.0235 / 0.0196 / 0.0078; BC2 sRGB 0.0297 / 0.0293 / 0.0095; BC3 0.0235 /
  0.0235 / 0.0078; BC3 sRGB 0.0323 / 0.0273 / 0.0095; BC4 0.0235 / 0.0236 / 0.00025; BC4 signed
  0.0504 / 0.0504 / 0.00026; BC5 0.0247 / 0.0249 / 0.00025; BC5 signed 0.0508 / 0.0508 / 0.00026;
  BC6H both 0 / 0 / 0; BC7 0 / 0 / 0; BC7 sRGB 0.0037 / 0 / 0.0037 (WARP's sRGB conversion
  differs from the GPU's and the back end's). Natively 0 texels differ for every format.
- `web/check` (clean build): all twelve pictures equal Direct3D 11's at tolerance 0, every twist
  differs as before, the viewports as before, the suite in Chrome 24 of 24, Chrome's adapter the
  RTX 3080 (D3D12, discrete, not a fallback). `prepare` in Chrome 157 (16 programs per half,
  `performance.now()` steps of 0.1 ms): unprepared, the frame of 16 first draws (draws, submit,
  wait for the GPU) took 473.7 ms, each draw call under 0.1 ms; after `prepare`, the 16 `prepare`
  calls took 0.6 ms, waiting for a submit after them 441.3 ms (the compiles), and the frame
  4.4 ms with no miss.
- This Mac (Apple M5 Max, wgpu on Metal), two single jobs from the committed tree (af2d885, then
  65e7755 for copies and the suite; Mac load average 18 to 37; build 54 s at `-j 2`): bindings 10
  of 10, blocks 2 of 2, copies 4 of 4 (after the one-step allowance for 24-bit depth stored as
  float: the probe answers `Float(0.10000001)` here, and the read of one texel was one step off a
  near tie), depth 2 of 2, fitted 1, prepare 1, raster 1, scenes 4 of 4, writes 11 of 11, suite
  23 of 24 (present, the known failure below). Differences from the RTX 3080's Direct3D
  references, all within the rig's allowance: triangle 1952 pixels by 1, quads 2455 pixels by 1 (on
  every depth format), instanced 0, the eight viewports 662 pixels by 1. `prepare` there: 16
  unprepared first draws median 0.12 ms, longest 0.42 ms, frame 4.1 ms; after 2.9 ms of prepares,
  median 0.3 us, frame 0.89 ms.
- `prepare` natively, alone in the GPU queue: Direct3D 12, unprepared draw median 9.53 ms, longest
  13.56 ms, frame 158.5 ms; after 150.4 ms of prepares, draw median 0.9 us, longest 4.4 us, frame
  1.15 ms. Vulkan: unprepared median 0.43 ms, longest 0.83 ms, frame 7.9 ms; after 6.7 ms of
  prepares, median 0.5 us, frame 0.35 ms. Both far below run 5's (Vulkan median 6.5 ms then): the
  programs are the same each run, so the driver's own shader cache likely holds them; expected,
  not measured.

Run 6 (desktop: Windows 10, i9-9900K, RTX 3080, NVIDIA 621.01, shared with other tracks' jobs, CPU
load not sampled; Mac load average 30 to 48):
- Before the BC work, all tests at e49d5f6: Vulkan 60 of 60 (bindings 10 in 1.71 s, copies 3, depth
  2, direct3d 3 in 1.29 s, fitted 1, prepare 1, raster 1, scenes 4, suite 24 in 3.92 s, writes 11
  in 1.53 s); Direct3D 12 60 of 60 (bindings 5.09 s, direct3d 2.61 s, suite 11.34 s, writes
  4.97 s). `web/check`: all twelve pictures equal Direct3D 11's at tolerance 0, the twists differ
  as before (16384 pixels by up to 92; 25016 by up to 204), the suite in Chrome 157.0.8103.78 24
  of 24; Chrome's adapter "NVIDIA GeForce RTX 3080", D3D12, discrete, not a fallback. The pictures
  in Chrome are now read back through JSPI waiting instead of polling.
- `web/check` with the viewports: Chrome's eight-viewport picture equals Direct3D 11's in all 32768
  pixels; moved to whole pixels, 2540 differ by up to 229. tests/raster.rs passes on both APIs
  through the shared scene.
- After the BC work, all tests: Vulkan 62 of 62 and Direct3D 12 62 of 62, one ignored on each (the
  known BC failure); blocks 2 in 0.53 s and 1.32 s; `web/check` passes again (suite in Chrome 24
  of 24, pictures equal, twists differ); clippy with warnings denied passes natively and for
  WebAssembly.
- tests/direct3d.rs, the same on both APIs: `generate_mips` of `r32float` filtered in the program
  (no `float32-filterable`) differs from GenerateMips by at most 7.8e-6, 1.5e-6, 1.2e-7, 0, 0, 0
  relative per level 1 to 6; filtered by the sampler (with the feature) it is equal at every level.
  `rgba16unorm` values in `rgba16uint` (program) differ by at most one step at each level; as
  `rgba16unorm` with the sampler, equal. So the sampler's filter on the RTX 3080 is what
  Direct3D's GenerateMips uses, and the program's exact average differs from it by those amounts.
  The sRGB resolve: 0 of 16384 bytes differ; resolved as `rgba8unorm`, 1269 differ by up to 56.
- tests/blocks.rs, seed 188, the same on both APIs: every level of every BC format reads back as
  written, natively and decoded; BC6H (both) and BC7 (both) decoded sample exactly as the GPU's
  decoder does; BC1 to BC5 differ by up to: BC1 2/255 (7.8e-3), BC1 sRGB 1.4e-2 linear, BC2 3/255,
  BC2 sRGB 2.0e-2, BC3 5/255, BC3 sRGB 1.8e-2, BC4 unorm 1.8e-2, BC4 snorm 4.0e-2, BC5 unorm 2.1e-2,
  BC5 snorm 3.9e-2. The GPU's BC4 values are not on an 8-bit grid (0.43237963 = 28336 / 65535).

Run 5 (desktop: Windows 10, i9-9900K, RTX 3080, NVIDIA 621.01, shared with other tracks' jobs; Mac
load average 24 to 51):
- Last full run, all tests at 72cb445 on Vulkan: 57 of 57 (bindings 10 in 2.01 s, copies 3 in
  0.49 s, depth 2 in 0.26 s, fitted 1, prepare 1, raster 1, scenes 4 in 0.69 s, suite 24 in 3.48 s,
  writes 11 in 1.55 s); on Direct3D 12: 57 of 57 (5.12 s, 1.28 s, 0.83 s, scenes 1.74 s, suite
  10.33 s, writes 5.43 s).
- This Mac (Apple M5 Max, wgpu on Metal; one job at 6471d88's code, Mac load average 42 to 50, build
  1 min 54 s at `-j 2`): bindings 10 of 10, copies 3 of 3, depth 2 of 2 (the probe answers
  `Float(0.10000001)`: `depth24plus` is 32-bit float there, `DEPTH24_UNORM` clear, the bias
  picture not drawn), prepare 1 of 1 (16 first draws unprepared median 27.6 ms, longest 80.4 ms;
  after 427 ms of prepares median 0.54 µs, longest 6.5 µs), suite 23 of 24 (present: the host has
  no macOS window), scenes 2 of 4 and writes 9 of 11: `scenes_match_direct3d` (triangle and quads),
  `quads_match_direct3d_on_every_depth_format` (2455 pixels on `depth24plus`),
  `ordered_write_in_a_pass_splits_it` and `writes_ahead_of_a_pass_do_not_split_it` compare with
  the RTX 3080's references at tolerance 0 and differ; how far was not printed. fitted.rs and
  raster.rs were not yet written then.
- `web/check` (run twice, last at a561a61): all twelve pictures equal Direct3D 11's at tolerance
  0; every twist differs natively
  on Direct3D 12 and on Vulkan and in Chrome (blend factor 16384 pixels by up to 92, depth function
  25016 by up to 204, on `quads` and `quads-d24s8` alike); Chrome's WebGPU adapter "NVIDIA GeForce
  RTX 3080", D3D12, discrete GPU, not a fallback.
- The depth probe: Direct3D 12 `Unorm(0.10000009)`, Vulkan `Other(0.10000015)`.
- `prepare` (job alone in the GPU queue, CPU load not sampled; 16 programs per half, each a
  fragment loop of 24 rounds, distinct per program): first draws without prepare, Vulkan median
  6.52 ms, longest 13.6 ms, 115 ms for 16; Direct3D 12 median 26.9 ms, longest 39.3 ms, 460 ms for
  16. After prepare (Vulkan 107 ms, Direct3D 12 410 ms for the 16 prepares): every first draw
  median 0.8 to 1 µs, longest 4.3 µs, no miss and no pipeline built while drawing. Not measured in
  Chrome yet.
- Clippy with warnings denied passes, natively and for WebAssembly.

Run 4 (desktop: Windows 10, i9-9900K, RTX 3080, NVIDIA 621.01, shared with other tracks' jobs,
CPU load 4 % sampled right after; Mac load average 27 to 50):
- All tests on Vulkan: 41 of 41 (bindings 5 in 0.58 s, scenes 4 in 0.57 s, suite 21 in 2.31 s,
  present among them, writes 11 in 1.08 s). On Direct3D 12: 40 of 41, the known
  `clears_fill_exactly` failure (bindings 2.08 s, scenes 1.80 s, suite 8.90 s, writes 4.41 s).
  The shared `stencil_operations_and_tests`, which lost the device before, passes on both.
- Before the fix of `shown`, reading the screen gave black on the rig's hidden desktop, and
  `present_shows_the_frame` failed with all texels [0, 0, 0, 255]; with `PrintWindow` it passes on
  both APIs.
- `web/check`: all twelve pictures (three scenes and `quads-d24s8`, each by wgpu on Direct3D 12,
  on Vulkan, and in Chrome 157.0.8103.78) equal Direct3D 11's at tolerance 0; the twists differ as
  before (blend factor 16384 pixels by up to 92, depth function 25016 by up to 204), on
  `quads-d24s8` too; Chrome's WebGPU adapter is "NVIDIA GeForce RTX 3080", D3D12, discrete GPU,
  not a fallback. Clippy with warnings denied passes; the WebAssembly build checks.
- The random byte test was not run again: nothing it exercises changed.

Run 3 (desktop: Windows 10, i9-9900K, RTX 3080, NVIDIA 621.01, shared with other tracks' jobs; Mac
load average 26 to 30):
- All tests on Vulkan: 30 of 30 (bindings 4 in 0.44 s, scenes 3 in 0.33 s, suite 12 in 1.12 s,
  writes 11 in 0.97 s). On Direct3D 12: 29 of 30, the suite's `clears_fill_exactly` failing loudly
  as before ("webgpu: an integer clear past 2^24, which wgpu's Direct3D 12 rounds through a float,
  is not built yet"; the suite then sees its read back never ready). Run with `--no-fail-fast`, so
  writes.rs ran on both.
- The random byte test, every run passing: on Vulkan seeds 1 to 20 at 3000 operations each and
  seed 987654321 at 6000; on Direct3D 12 seeds 1, 2, 3, 4 and 9 at 3000 and 987654321 at 6000
  (27 runs, 69000 operations). Of each seed's 3000, about 200 to 260 were no-overwrite writes and
  about 80 to 120 were turned into ordered writes to keep the promise (987654321: 457 and 188).
  Seeds 9 and 987654321 are those the independent check found failing.
- `web/check` passes again: all nine pictures equal Direct3D 11's at tolerance 0, both twists
  differ (blend factor 16384 pixels by up to 92, depth function 25016 by up to 204, natively and in
  Chrome), Chrome 157.0.8103.78's adapter is "NVIDIA GeForce RTX 3080", D3D12, discrete GPU, not a
  fallback. Clippy with warnings denied passes.

All on the desktop (Windows 10, i9-9900K, RTX 3080, driver 32.0.16.2101 / NVIDIA 621.01); desktop
CPU load 0 % when sampled after the runs; Mac load average 27 to 39 (other tracks).
- Every pixel equal to Direct3D 11's reference, all three scenes: wgpu on Vulkan (adapter "NVIDIA
  GeForce RTX 3080 (Vulkan, DiscreteGpu, NVIDIA 621.01)"), wgpu on Direct3D 12, and Chrome
  157.0.8103.78 WebGPU (GPUAdapterInfo: "NVIDIA GeForce RTX 3080", vendor nvidia, backend D3D12,
  type "discrete GPU", not a fallback). So the tolerance stated is 0: on the same GPU any allowance
  would hide a real difference. On another GPU expect the rig's WARP allowance of 1 (the rig
  measured wgpu on Metal differing by 1 on the triangle); not run here.
- The twists, natively and in Chrome alike: blend source factor one, 16384 pixels differ, by up to
  92; depth test greater, 25016 pixels differ, by up to 204.
- Caching (test `pipelines_and_bind_groups_are_cached`, Vulkan and Direct3D 12): 21 draws of two
  programs with equal blocks over two passes made 2 pipelines, 3 `setPipeline` calls, 2 bind groups
  (one per set), 1 bind group layout, 2 `PipelineMiss`.
- Run 2 (desktop CPU load 27 % sampled right after, Mac load average 24 to 26): on Vulkan all
  20 tests pass (present among them without checking texels) (bindings 3 in 0.39 s, scenes 3 in 0.39 s, suite 7 in 0.96 s, writes 7 in 0.75 s);
  on Direct3D 12, 19 of 20 (suite 6 of 7, the known failure above; writes.rs ran in a separate
  command, since without `--no-fail-fast` the failure stops cargo first). The random byte test ran
  its default 400 operations, seed 0x5eed, on both; most other seeds failed then (run 3). Constants rewritten per draw: 20 draws made 1 bind
  group, 0 renames, 19 slice moves. `web/check` passes again after these changes: all nine pictures
  equal Direct3D 11's, both twists fail natively and in Chrome with the same counts as run 1, and
  Chrome's adapter is the RTX 3080 (D3D12, discrete, not a fallback); job 6 s. The arena, edge
  program and suite have not been run in Chrome yet (the browser page draws only the scenes).
  Clippy with warnings denied passes.
- Run 1, last full run: 5 tests pass on Vulkan (scenes 0.95 s, writes 0.76 s) and on Direct3D 12 (1.10 s,
  0.69 s); `web/check` passes (job 4 s); clippy with warnings denied passes. First build of the
  crate's tests 2 min 44 s at `-j 3`.

## Not carried out yet (each marks the device lost with its reason, or is declared absent)

- 3D BC textures natively need `texture-compression-bc-sliced-3d`, which the device asks for where
  the adapter has it; decoded ones need nothing.

- Multisampled depth or stencil copies that WebGPU cannot make (between formats); part of a
  16-bit depth level written from a buffer.
- A surface wgpu reports lost marks the device lost by name (refused): only the host's hook holds
  the window a surface is made from.
- Immediates in the browser: `immediate_bytes` is 0 there (wgpu 30's browser back end has no
  `setImmediates`), so fronts spill them.
- Copies of three-channel textures with buffers or between them are made in runs of columns the
  device's widest 2D texture holds three times over (run 21); nothing is lost by width.
- A converted vertex stream reaching past the largest storage binding (128 MiB by default) or
  more than 2^22 workgroups.
- Drawing into the 16-bit packed formats: declared without `RENDER` (8-bit storage would keep
  more of a draw than Direct3D's 4, 5 or 6 bits; exact would need the program to round its output
  and could not round a blend).
- `OCCLUSION_COUNT`: absent for good (WebGPU's occlusion result is zero or not).
- Natively the staging ring's bytes are copied twice (into wgpu's own staging memory, then by
  the GPU into the ring): wgpu 30 offers no mapped ring that needs no wait.
- A buffer or texture renamed many times within one frame keeps that many spares in the pool until
  it is destroyed (bounded by what it made, not trimmed by age).
- Arena slices of buffers dropped without `destroy` are not reused until the arena is replaced.

## Next (G2d)

1. The independent check of G2d.
2. The owner's decision on loop bounding natively (run 16 measured it as the whole of the
   timestamp workload's 1.3 and 1.5 times): wgpu's `create_shader_module_trusted` without
   `force_loop_bounding` takes Direct3D's time, but lets a guest's endless loop be undefined in
   the driver's compiler, as on real Direct3D.
3. Merge the graphics branches at the start of each run and read the fronts' status files
   again; then both full runs, web/check and the literal check. Every graphics branch is merged
   as of run 26. Run the check before committing a merge that touches literals.baseline, in the
   LF worktree (How to run; never in the desktop's CRLF checkout); when a merge brings a newer
   version of its rules, run `init` there before committing that merge or in the commit right
   after it.
4. (Done by the Direct3D 11 track in its G4 run 24: the stencil views reading 0 were its
   binding check's.)
5. For the Direct3D 11 track: `B8G8R8X8_UNORM` vertices it now asks of the interface (a format
   reading X as 1.0); this back end reads BGRX as wgpu's `unorm8x4-bgra`. Its library's
   wgpu instance (wcontainers-directx com/chosen.rs) lacks `WebGpu::INSTANCE_FLAGS`, so on a GPU
   whose timestamp period is not 1 ns it would get no `TIMESTAMPS` (none on the RTX 3080). That
   instance is on Direct3D 12 with wgpu's default FXC, where wgpu offers no `SUBGROUP`: giving it
   DXC (`Dx12Compiler::DynamicDxc` with dxcompiler.dll beside the library, shader model 6) would
   let the Direct3D 11 front see `SUBGROUPS` there too. atg-simplecomputepc's division:
   Measurements, run 27.
6. For the Direct3D 12 track (not this back end's code): the 3 ms a frame of copying in
   bgfx-01-cubes-d3d12 (Measurements, runs 19 and 21) is the front's `MappedUploadWhole`
   (device/bind.rs): at each `ExecuteCommandLists` every upload buffer a binding reads is written
   to the back end whole. Writing only the ranges the recorded bindings and copies read would cut
   it to what the frame uses (run 21: 14,680,328 bytes a frame in 4 writes).
7. Frame time, what is left: the GPU thread's 10 ms a frame in the Vulkan driver's acquire is the
   wait for the display's next image under FIFO (Direct3D's `Present(1)`), which the NVIDIA Vulkan
   driver spins in rather than sleeps; acquiring later in the frame moves the wait, and another
   present mode changes what the program asked for, so neither is taken. Find why some programs'
   runs on the hidden desktop last far longer than their frames (godot-platformer-2d-d3d12's
   pair), so the breakdown can be taken for Godot.
8. The literal sweep, the parts left (Run 22's list).
9. Report wgpu-hal's Direct3D 12 gap overwrite upstream (Departures).
10. A view swizzle in the browser: Chrome 157 offers WebGPU's `texture-component-swizzle` (run
    25). It would take a mapping in the interface's view description and a capability (an
    interface change), and wgpu's browser back end passing it on; natively wgpu 30 has no
    swizzle, so `SwizzleInShader` stays there. Worth it only if a front finds the in-program
    mapping costly.
11. For the Direct3D 12 track: add `TEXTURE_DATA_PLACEMENT_ALIGNMENT` (512) to the
    infrastructure crate's `d3d` module with its Windows test; then this crate depends on that
    crate, imports it and deletes `DX12_PLACEMENT_ALIGNMENT` and its unit test, and the front
    imports it too (Departures).
12. The browser's per-call cost (Measurements, run 27): fewer calls into Chrome's WebGPU per
    frame, first `createBindGroup` (2 a frame on the replayed frames), `createCommandEncoder`
    and the fence queries (`onSubmittedWorkDone`); measure with web/replay-profile.mjs, and get
    its per-interface-call grouping to run once.

## Departures from the plan

- `prepare` of a strip builds two pipelines (16-bit and 32-bit indices), as `PrepareDesc` does not
  say which index format the draws use: one extra compile per prepared strip combination (natively
  a foreground one, since `prepare` compiles at the call there), instead of an interface field
  (run 18). The G2b check asked for the field under section 12's rule; not made (run 19): a field
  in `PrepareDesc` breaks the four places in other tracks' crates that build it
  (wcontainers-direct3d-11 cache.rs and ahead.rs, wcontainers-direct3d-12 device.rs,
  wcontainers-gpu-test fast_paths.rs), which a commit touching only crates/wcontainers-gpu cannot
  carry, and every branch merging this one would stop building. The interface track has since
  ruled (G1c, `Backend::prepare`'s doc and plan 4.3): no index format in `PrepareDesc`, and a
  back end like this one prepares strips for both, so this is now the plan, not a departure.

- Runs 24 and 25 did not merge -direct3d-12 and -shaders, though the task says to merge the
  graphics branches each run: neither had adopted the literal check, and merging either made the
  check fail on literals in their crates that this track may neither fix nor add to the baseline
  (the brief: keep the check passing; docs/literals.md step 6: never add a line). Nothing in
  their commits was needed by this back end or its tests. Both are merged since run 26.
- The literal check runs in an LF worktree of the desktop's repository, not in the checkout the
  rig's `remote` builds in (run 26): that checkout is CRLF and has the shaders crate's nested
  submodules, so it keys and scans differently from the tree every other track's baseline was
  made in. The baseline's keys are those of the committed (LF) files.

- BGRX (`Bgrx8Unorm`, `Bgrx8UnormSrgb`) is held as BGRA with alpha one, so its unused X byte reads
  back as 255. The RTX 3080's Direct3D 11 keeps the X byte as written, from the host (128 of 128
  texels in tests/direct3d.rs) and by draws (the blended alpha). Direct3D defines no value for it;
  holding it as written would make sampled alpha read it, and WebGPU has no swizzle to read one.
- The 16-bit packed formats are widened by repeating their bits, as the RTX 3080's Direct3D 11
  loads them, not to the nearest 8-bit value of `v / (2^n - 1)` (Direct3D's rule, which its own
  loads miss by up to 0.72 of an 8-bit step).

- Timestamps need the host's wgpu instance made with `AUTOMATIC_TIMESTAMP_NORMALIZATION`
  (`WebGpu::INSTANCE_FLAGS`) where the GPU's timestamp period is not 1 ns; without it the back end
  declares `TIMESTAMPS` absent rather than convert itself (wgpu has the conversion). The RTX 3080's
  period is 1 ns either way.
- Wireframe on wgpu's Vulkan follows Vulkan's line rules: on the RTX 3080, 214 pixels of the rig's
  fill picture differ from Direct3D 11's (Direct3D 12: none). Declared anyway: Vulkan's line rules
  govern line lists there too (expected, not measured), and only native testing runs on Vulkan.
  tests/raster.rs reports `wireframe_fill_matches_direct3d` there as ignored with that reason (a
  known failure; it fails if the solid cell differs too). Browsers have no polygon modes.
- Immediates are wgpu's (`var<immediate>` in the program, `set_immediates`), natively only: wgpu
  30's browser back end has no `setImmediates`, so there `immediate_bytes` is 0 and fronts spill
  them (`ImmediatesSpilled`). Plan 5.7's "else a ring buffer bound with a dynamic offset" is that
  spill, which the interface gives the fronts; the back end does not do it a second time.
- A window that does not show a format's values as sRGB is presented through an 8-bit format by
  a draw (`PresentConverted`), as the interface says frames are shown: natively that is every
  `rgba16float` window (wgpu's Direct3D 12 takes it only as linear light, its Vulkan not at all on
  the desktop); in Chrome a `rgba16float` canvas takes sRGB values and is used as it is.

- Plan 5.7 says formats WebGPU lacks are "converted on upload". For vertex formats that cannot
  be done: a buffer's bytes are written before any layout says how they are read, the same bytes
  may be read through several layouts and offsets, and GPU commands write buffers too. So a
  shadow stream is made by a compute program at the first draw that reads the buffer through the
  layout, and kept until the buffer is written (Run 9).
- On macOS the suite's present test runs in its own test file with a harness of its own (main
  thread), and tests/suite.rs there runs the list of examples/scenes/names.rs.
- The plan asks BC textures decoded to be compared with Direct3D "within the stated tolerance".
  The tolerance stated is WARP's distance from the GPU plus one step of the decoded storage
  format, since no fixed one-step tolerance is met by Direct3D's own WARP on these blocks.
- Scene pictures in this crate's tests are held to the rig's per-picture allowance (what WARP
  earns) so they hold on any GPU; the RTX 3080's exact equality is printed by them and asserted by
  web/check at tolerance 0.
- wgpu is vendored (third_party/wgpu, 30.0.2 plus two fixes in src/backend/webgpu.rs) so its
  browser back end passes a render pass's occlusion query set and a colour attachment's depth
  slice to WebGPU; without them every occlusion query in the browser failed, and so did every draw
  into a slice of a 3D texture (Chrome: "depthSlice is required for a 3D attachment"). Neither is
  reported upstream; whether wgpu's trunk has the second bug is not checked. This touches the
  workspace manifest, outside this crate.
- The `generate_mips` comparisons with Direct3D make each level from Direct3D's level above it, so
  one level's difference is not carried into the next and each tolerance is per step.

- `prepare` calls `createRenderPipeline`, not `createRenderPipelineAsync`, and keeps no cap on
  compiles in flight: wgpu 30 has no asynchronous pipeline creation. In Chrome the pipeline still
  compiles in the GPU process while the page goes on, and a draw waits only if it is not ready
  (expected, not measured in Chrome yet). Natively the compile happens at the `prepare` call, so
  `BACKGROUND_PREPARE` is declared only in the browser.
- The 24-bit depth probe accepts a third answer instead of calling it a back end bug: wgpu's
  Vulkan on the RTX 3080 moves depth by two 24-bit steps per unit of bias. The bit stays clear and
  fronts take `DepthBiasInShader`, which is right for any storage.
- `texture-formats-tier1` cannot be asked for through wgpu 30 (it has no such feature), so in the
  browser the tier 1 formats are declared without their tier 1 bits even where Chrome has them
  (fronts take `NormAsInteger`). Natively the same formats come from wgpu's 16-bit-norm and
  adapter-specific format features, and the test makes a device without those.
- Reading 24-bit depth back in a program is exact only to one 24-bit step on the RTX 3080 (a clear
  to 0.5, stored as 2^23, reads as 0.50000006), so the depth copy tests allow one step for 24-bit
  depth and none for 32-bit float depth.

- Every attachment is stored at the end of a pass, also when the interface says `Discard`: an
  ordered write may split the pass, and its first part must keep what it drew. Correct, but it
  gives up the bandwidth saving of a discard on tiled GPUs; recording a pass's draws before opening
  it would allow both.
- `DontCare` loads of colour attachments clear to zero (WebGPU has no don't-care load; a clear is
  its cheapest load); of depth and stencil they load, which is as cheap there and keeps the bytes.
- Depth writes happen only with the depth test on, as in Direct3D, OpenGL and Vulkan.
- Blend factors of min and max operations are set to one (WebGPU requires it; every API ignores
  them there). Blending is left off on integer targets, which OpenGL and Direct3D ignore and Vulkan
  and WebGPU refuse; into any other target without `BLEND` the draw fails loudly.
- The uniform "ring" of 4.5 is an arena of slices reused by size, not a ring: a buffer's latest
  bytes stay in its slice until its next `Discard`, so slices cannot be overwritten in ring order.
  It is still one large GPU buffer, replaced by one twice as large when it fills. A slice given up
  is reused after the next submit, not after the GPU finishes, because queue writes land after all
  work already submitted.
- Copies between two arena buffers go through a temporary buffer (counted `CopyWithinBuffer`),
  since both live in one GPU buffer.
- Anisotropy is set to one unless all three filters are linear, as WebGPU requires and Direct3D
  does.
- A frame the window cannot take (occluded, timed out, outdated after configuring again) is drawn
  into a texture of its own and not shown, with no error: the interface has no word for an
  unshown frame, and DXGI answers it with a success code (`DXGI_STATUS_OCCLUDED`).

- Texture reads into a buffer at an offset off Direct3D 12's placement alignment (512) with gaps
  between rows or layers go, on wgpu's Direct3D 12, through a buffer at offset 0 and one buffer
  copy per row (`RowsRepacked`, run 21): wgpu-hal's Direct3D 12 copies such reads through a buffer
  of its own and then copies that whole into place, gaps included, where the driver lacks
  unrestricted pitches (dx12/command.rs `copy_texture_to_buffer` and `buf_tex_intermediate`; the
  RTX 3080 on Windows 10 lacks them), so every byte between the rows was overwritten. Not
  reported upstream yet. Vulkan and browsers copy directly. `RowsRepacked`'s name says "a pitch
  the API does not take"; here it is a placement, the same route.
- The vendored wgpu has a third change (run 21): its browser back end's feature table maps
  WebGPU's `subgroups` and `primitive-index`, which wgpu 30 left out, so a browser device gets
  them where the adapter offers them.
- The back end prints a line of its counters at each power of two frames presented
  (`FRAME_REPORT`). The plan asks for counters and warnings; a program's frame costs can only be
  read from a log when the rig ends programs with `TerminateProcess` (no report at the end could
  be printed), and powers of two keep it to a few lines a run.
- The literal check was adopted (run 23) by merging origin/phase-1-literals, as the brief says,
  not main as docs/literals.md step 1 asks (main does not hold it yet); -webgl, merged here, had
  already done the same. So, as that document says for such a track, main is not merged here
  until main holds the check.
- Direct3D 12's texture data placement alignment (512) is defined here (`memory.rs`
  `DX12_PLACEMENT_ALIGNMENT`, needed because Direct3D 12 implements WebGPU on Windows, natively
  and in Chrome) and again in the Direct3D 12 front (`resource.rs` `PLACEMENT_ALIGNMENT`), which
  the rule of one definition forbids (the G2c check's finding). The one home the literal audit
  names for Direct3D constants of crates built for WebAssembly is the infrastructure crate's
  `d3d` module (orchestrator magic-literals README, item 13), which the Direct3D 12 track owns
  and which lacks it; this crate may not add it there, and the windows crate does not build for
  WebAssembly. Until it is there (asked under Next), this crate's value is tied to the windows
  crate's `D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT` by a Windows-only unit test.

## Interface changes

- 1b06735 `interface: named the slow path of a resolve averaged by a draw loading every sample`:
  `SlowPath::ResolveByDraw`, and the plan's 4.2 sentence on resolving in the other twin (Chrome's
  WebGPU resolve averages the stored values).

- 04bd53b `interface: named the slow path of a copy between regions of one subresource made through
  a buffer`: `SlowPath::CopyWithinTexture`, a sentence in `copy_texture`'s doc (regions of one
  subresource may be copied if they do not overlap, as Vulkan and OpenGL allow) and in the plan's
  4.2.

- 578a3fc `interface: listed as a loss float depth copied from values a draw cannot write`: a
  4.12 entry and a sentence in `copy_buffer_to_texture`'s doc. WebGPU writes depth from other
  formats only by a draw, which clamps depth to 0 to 1 and flushes denormals.

- 8f7db1b `interface: named the slow path of a surface format the window lacks converted by a draw
  at present`: `SlowPath::PresentConverted`, and the plan's 4.9 sentence on surface formats.

- 3f5519a `interface: named the slow path of stencil written by a draw per bit`:
  `SlowPath::StencilWrittenByDraw`, and the plan's 4.2 sentences on depth and stencil copies WebGPU
  makes only whole (written by draws, read by `DepthReadByDraw`). `StencilCopiedByDraw` names a
  different job (an 8-bit texture filled from the stencil for sampling, WebGL 2's).
- b999c16 `interface: named the slow path of integer colour clears made by a draw`:
  `SlowPath::IntegerClearByDraw`, and the sentence in the plan's 5.7. wgpu's Direct3D 12 clears
  colour targets through f32, which cannot hold integers past 2^24.

Gaps found for the interface track (not changed here): no capability tells a front which vertex
formats a back end converts itself. `PrepareDesc` has no index format; since run 18 this back end
builds a prepared strip for both index formats instead (Departures), so nothing misses; a field
`strip_index: Option<IndexFormat>` would save the second pipeline but breaks both fronts' code.
(Instance step rates now have `INSTANCE_STEP_RATE`, from the shared line.)

- db9e0d8 `interface: made no-overwrite writes promise that no unfinished command or ordered write
  touches their bytes`: the doc of `WriteMode::NoOverwrite` and the plan's 4.6 entry. The promise
  was only about reads; a back end writing at once cannot order itself after a pending copy into
  the same bytes, and Direct3D 11 and OpenGL do not ask it to.
- 61a266e `interface: named the slow path of buffer clears to a non-zero pattern made by copies`:
  `SlowPath::PatternClearCopied`, and the plan's sentence on `clear_buffer` (4.2) saying WebGPU
  fills non-zero patterns by copies.
- edc6fd7f `interface: a slow path for float colour clears drawn to keep their bits`:
  `SlowPath::FloatBitsClearByDraw` (appended last, so every existing reason keeps its number),
  and the plan's 5.7 sentence beside `IntegerClearByDraw`. A 32-bit float clear to a denormal or
  a NaN changes its bits through every API's clear value (run 27, measured), so this back end
  draws it; a counted reason of its own, as the integer clears have.

## Licences

Confirmed in each crate's Cargo.toml on 2026-11-17: wgpu 30.0.2 (vendored, LICENSE.MIT and
LICENSE.APACHE kept) and wgpu-naga-bridge 30.0.2 MIT OR Apache-2.0 (dependencies); bcdec_rs 0.2.1
MIT (its Cargo.toml; plan section 3 records its repository's licence); half 2.7.1 MIT OR Apache-2.0;
in the browser build js-sys 0.3.107 and wasm-bindgen 0.2.130 MIT OR Apache-2.0. Test-only, new in
run 6: web-sys 0.3.107 and getrandom 0.4.3 MIT OR Apache-2.0 (browser); windows 0.62.2 MIT OR
Apache-2.0 and wcontainers-graphics-test (ours) for tests/direct3d.rs; new in run 7, image 0.25.10
MIT OR Apache-2.0 (its Cargo.toml; already in the lock file through the rig) and the windows crate's
`Win32_Graphics_Direct3D_Fxc` feature, for tests/direct3d.rs. Test-only: windows-sys 0.61.2 MIT OR Apache-2.0 (the suite's window and reading the screen, already
in the lock file); wcontainers-gpu-test (ours); png 0.18.1, anyhow 1.0.104, wasm-bindgen 0.2.130,
js-sys 0.3.107 MIT OR Apache-2.0 (wasm-bindgen-futures is no longer used); pollster 1.0.1 Apache-2.0/MIT;
bytemuck 1.25.2 Zlib OR Apache-2.0 OR MIT; fastrand 2.5.0 Apache-2.0 OR MIT (the random test). Tool: wasm-bindgen-cli 0.2.130 MIT OR Apache-2.0.

New in run 19, test-only: serde_json 1.0.154 MIT OR Apache-2.0 (its Cargo.toml; already in the
lock file), and pdb 0.8.0 MIT OR Apache-2.0 with the crates it brings, scroll 0.11.0 MIT,
fallible-iterator 0.2.0 MIT/Apache-2.0 and uuid 1.29.3 Apache-2.0 OR MIT (their Cargo.toml files,
checked in run 21), for examples/frames.rs. Tool on the desktop: samply 0.13.1 MIT OR Apache-2.0 (its
Cargo.toml at the tag), testing only.

New in run 8, test-only: libtest-mimic 0.8.2 MIT/Apache-2.0 (its Cargo.toml); on macOS only,
block2 0.6.2 MIT, objc2 0.6.5 MIT, objc2-foundation 0.3.2 MIT, and objc2-app-kit,
objc2-core-foundation, objc2-core-graphics, objc2-quartz-core and objc2-screen-capture-kit 0.3.2,
each Zlib OR Apache-2.0 OR MIT (their Cargo.toml files; most already in the lock file through wgpu).

## Installed on the desktop

- C:\wc\src\phase-2-graphics-webgpu-lf (run 26): a git worktree of this branch's desktop
  repository with LF line ends and the shaders crate's two submodules (and dxil-spirv's
  spirv-headers), for the literal check only; 0.05 GB of source (the submodules' objects live in
  the repository's .git). Builds go to C:\wc\target\phase-2-graphics-webgpu as the rig's do.

- samply 0.13.1 (run 19, MIT OR Apache-2.0, a profiling tool for the frame breakdown only, never
  a dependency): its release zip (14.9 MB) in C:\wc\downloads\samply-0.13.1, unpacked into
  C:\wc\tools\samply-0.13.1 (samply.exe 14.9 MB and its licences). An empty
  C:\wc\out\phase-2-graphics-webgpu\nonvapi\nvapi64.dll for bgfx (as the Direct3D 11 track
  does), and the profiles in C:\wc\out\phase-2-graphics-webgpu\frames.

- wasm-bindgen-cli 0.2.130 (`cargo install --locked`): wasm-bindgen.exe, wasm2es6js.exe and
  wasm-bindgen-test-runner.exe in C:\wc\cargo\bin, 18.9 MB; its 0.53 GB build directory was
  removed.
- The rig's page-driver packages in this branch's checkout
  (C:\wc\src\phase-2-graphics-webgpu\tools\graphics-rig\browser\node_modules), by the rig's
  install.ps1.
- Build output C:\wc\target\phase-2-graphics-webgpu: removed whole at the end of run 13 (9.96 GB,
  debug and release) and again at the end of run 14 (6.68 GB), so the next build is cold (about 3 minutes for the tests, 7 for web/check);
  pictures in C:\wc\out\phase-2-graphics-webgpu-webgpu. On the Mac the checkout's `target`
  (1.2 GB of `cargo check` output) was removed at the end of run 13 and none was made in run 14;
  scratch holds no build output (logs and pictures only, 328 MB).
