# Direct3D 11 front status

Branch: agents, in the agents' shared worktree /Users/thomaseaton/Documents/GitHub/wcontainers-agents
(since the evening of 2026-11-21). Plan: docs/phase-2-graphics.md, milestone G4 (section 12).
This file keeps the current state only; each earlier run's record (what it did, its measurements,
its literal counts) is in this file's git history (`git log -p docs/status-graphics-direct3d-11.md`).
Runs so far: six on G4a, 25 on G4 (2026-11-17 and 18), round 2 runs 1 to 12 (2026-11-19 and 20),
round 3 run 1 and round 4 runs 2 to 6 (2026-11-20), main runs 1 to 3 and agents runs 1 to 3
(2026-11-21), G4h run 1 (2026-11-22).

## Where things stand

G4 passed its check. G4h (feature level 11_0 reported truthfully, the owner's ruling of
2026-11-22) is in progress: run 1 built one missing ability and the means to know them all; the
rest is listed under What comes next, items 1 to 4, of which 2 and 3 wait on other tracks' crates.

G4h run 1 (2026-11-22, from about 08:00 Pacific):
- `A8_UNORM` typed storage views are served: the WebGPU back end declares storage use of
  `A8Unorm` through the `rgba8unorm` it holds it in (packed.rs `stored`), and the front binds such
  views with `A8Unorm` in the shaders crate's `storage_formats` (its run 81 makes every store keep
  alpha alone). `CheckFormatSupport` now claims `TYPED_UNORDERED_ACCESS_VIEW`, `UAV_TYPED_STORE`
  and `UAV_TYPED_LOAD` for it as the RTX 3080 does (its row left `support_differences`). Test:
  programs.rs `a8_storage_views_store_as_direct3d_does` (stores, then sampled), against the RTX
  3080: passed.
- `required::LEVEL_REQUIRED`: Microsoft's 10_0, 10_1 and 11_0 format tables, generated (scratch
  g4h/required_rows.py from the pages' markdown in downloads/msdocs), every required bit of both
  words, the 4- and 8-sample counts, and whether the format itself is optional. programs.rs
  `every_format_requirement_of_levels_10_0_to_11_0_met_where_direct3d_meets_it` compares ours with
  it wherever the RTX 3080 meets it (the format_support case now also prints 8-sample quality
  levels). The tables agree with the RTX 3080 everywhere but P016 and NV11, optional formats it
  lacks. Ours lacks, at 11_0: the B5G6R5 target (with blending, mip generation, 4 and 8 samples,
  load and resolve), the resolves of B5G5R5A1 and B4G4R4A4, `B8G8R8X8_UNORM` vertex and typed
  buffers, the extended-range display and back buffer cast, and the `R32G32B32` formats'
  multisample loads and resolve (new: the comparison test had masked multisample bits); 10_0 and
  10_1 lack the same less what those levels do not require. Known failure (ignored) until built.
- The device's level is not yet taken from the table (Gap list); the owner's ruling is that it
  must be, once the abilities exist.
- Programs under the orchestrator's rule of 2026-11-22 (a result passes when no further from a
  real implementation than two real implementations are from each other): atg-simplecomputepc is a
  real difference (every pixel inside the Mandelbrot set has another colour; the shaders track's
  division); atg-simplemsaa_pc's frame 10 (8 pixels beyond the channel tolerance, largest 11, on
  edges; the RTX 3080's own two runs differ by 0 and 1 such pixels) is not re-judged, as WARP was
  not run on it; bgfx-26-occlusion's missing memory line is an absent feature reported absent.
  Not re-measured this run.

Agents run 3 (2026-11-21, from about 21:10 Pacific):
- The task carried the fifth check's findings once more; each is closed as this file says (the
  WebGL 2 build at 10_0 with its listed cases passing; the family library the joint step after G4
  and G5, its two modules measured apart; the 11_0 shortfalls listed as failures of the reported
  level, each waiting on a named track, with 11_0 left to the owner; `CheckFormatSupport` and
  `FORMAT_SUPPORT2` compared bit by bit with the RTX 3080 for every format at every level; 16-bit
  normalised targets held as integer twins with their test running; the minor findings as listed
  under the Gap list and Differences). The bar was run again at this run's last code commit
  (Measurements).
- The owner's interface changes merged at a00ab267 (readiness, binding settings, the negative
  route crate, the DNS answer) touch nothing this track builds or calls; both crates build and
  their tests pass after the merge.
- Literals: 66 lines of this track cut from the baseline (Literals).

Agents run 2 (2026-11-21, from about 20:00 Pacific), the eighth check's findings:
- Compute programs reading textures held as integer twins (blocking): a compute program's variant
  now takes the decoded slots (`StageVariant::decoded`) as a draw's does, so a compute shader that
  loads or samples, 2D or 3D, a normalised texture held as its integer twin (`NormAsInteger`)
  reads it decoded instead of removing the device (`COMPUTE_VARIANT_OF_DRAWS`). The same applied
  to stencil views and to a compute program asking an empty slot's size, which now take their
  draws' routes (`SwizzleInShader`, `TextureInfoInHelpers`). So `SHADER_LOAD` and `SHADER_SAMPLE`
  of these formats are served in every stage, as claimed. Tests: programs.rs
  `normalised_targets_drawn_read_and_their_mips_generated_as_direct3d_does` (each of the nine
  formats drawn into, then read by a compute program; a 2x2x2 texture of each read by one; native,
  without texture-formats-tier1 and without 16-bit norm) and `stencil` (the stencil, depth and
  empty-slot reads by a compute program), against the RTX 3080.
- Minor: the 32-bit float 3D textures filtered in the program are compared with the RTX 3080 within
  the error the specification's 8-bit filter weights allow (`float32_reads`' 3D part, clamp, wrap
  and mirror). Linear samples of the normalised formats are compared the same way, as the RTX 3080
  is not exact even at texel centres (0.44880 against 0.44882 for one `R8G8B8A8_SNORM` value).
- Minor: the WebGL 2 build refuses a swap chain on another process's window with
  `DXGI_ERROR_INVALID_CALL`, warned, as the WebGPU build does. Before, ANGLE failed to make one on the
  desktop window and the device was removed (`DXGI_ERROR_DEVICE_REMOVED`). Wine's d3d10core tests
  that crash after a refusal removed the device are named under the WebGL 2 item of the Gap list.
- The others: the 11_0 shortfalls stay with the owner; the constant buffers' counts wait on the
  WebGPU track; building ahead and B8G8R8X8's X byte are as listed; the literal check's findings
  are none of this track's.

## Wine's Direct3D 11 tests through ours

Files passing (the bar: real Direct3D's verdict for every test, except the gap list), on the RTX
3080 through the queue, not alone, rigfix/wine.sh, agents run 3 at 3b1cd4f3 (logs/a3b-wine*.log):
- d3d11: 151 pass of 166, the same 15 failing as since round 4 run 3; the failing tests are in the
  Gap list. test_format_support's 7 checks are the B5 targets (6) and XR_BIAS's display (1).
- d3d10core: 110 of 111: test_desktop_window (Gap list).
- d3d10core through the WebGL 2 build (10_0): 96 of 111 (logs/a3b-wine10gl.log): the 13 failing
  since round 2 run 12 and, since main run 3 reports no multisampling there,
  test_alpha_to_coverage and test_check_multisample_quality_levels; all are under the WebGL 2
  item of the Gap list.
- dxgi (not on G4's bar): 4 of the 17 tests without a Direct3D 12 part at round 2 run 8.

## Gap list (the pass bar's written list)

Everything on G4's pass bar that does not give real Direct3D's verdict on the RTX 3080, each item
once, with its cause and whose work it is. A gap stands behind a named absent feature; an ability
Microsoft's 11_0 table requires is not an absence, so those shortfalls are listed as failures of
the reported level, with the track whose part they wait on. Nothing is excluded for convenience.
The rig's exclusion of d3d11 test_create_device applies to real Direct3D's run too: its child
process makes a swap chain with `Windowed = FALSE`, which would take the owner's screen
(tools/graphics-rig/wine-excluded.txt).

Waiting on the joint step after G4 and G5 (orchestrator's ruling, 2026-11-20):
- Section 10's one family library (one DXGI shell, `D3D12CreateDevice` exported, a Direct3D 11
  program loading Direct3D 12's code). Today there are two modules, measured apart (Measurements):
  ours, `wcontainers_directx.dll` (Direct3D 10 and 11 and a DXGI), and the Direct3D 12 track's
  `wcontainers_d3d12.dll` (its own DXGI). Nothing else of section 10 is missing: the forwarders
  (d3d11.dll, dxgi.dll, d3d10core.dll), the exports of Direct3D 10 and 11, and the WebGL 2 build of
  the same source are built and tested.

Failures of the reported 11_0 against Microsoft's format table, waiting on other tracks (each
compared in the format support tests and in Wine's test_format_support; each needs code in a
crate this track does not own before this front can serve it):
- `B5G6R5_UNORM` and `B5G5R5A1_UNORM` as render targets, blended (and so mip generation): the
  WebGPU back end holds them as `rgba8unorm` for sampling only and declares no render use;
  drawing into them exactly needs the output quantised to the format's bits and blending at that
  precision, which neither the back end nor the shaders crate's `targets_in_shader` (no layout
  for packed formats) does. Wine d3d11 and d3d10core test_format_support ("Format 0x55 -
  blendable not supported", 6 checks). The 10Level9 tables ask the same of `B5G6R5_UNORM` at 9_1
  to 9_3, and 10_1 and 11_0 ask 4 and 8 samples of both. Since G4h this track builds it in the
  WebGPU back end (What comes next, 1); WebGL 2 draws into and blends `RGB565` and `RGB5_A1`
  natively.
- `R10G10B10_XR_BIAS_A2_UNORM`'s display and back buffer casts: everything else of its 11_0 row
  is served; a swap chain of the format, and presenting it with the extended-range conversion,
  are the infrastructure crate's (`wcontainers-directx-graphics-infrastructure`, swapchain.rs,
  built by the Direct3D 12 track, G5; Asked, with the route below). test_format_support ("Format
  0x59 - display not supported", 1 check), test_swapchain_formats (1 check). It can be built: the
  back buffers are held in `Rgb10a2Unorm`'s bits (as textures of the format already are), and the
  present copy decodes each channel, `(c * 1023 - 384) / 510` (the functional specification's
  XR_BIAS mapping, -0.7529 to 1.2529), into an `rgba16float` surface. In a browser that is a canvas
  configured `rgba16float` with `toneMapping: { mode: "extended" }`, which shows values outside 0
  to 1 in extended sRGB; natively wgpu's `Rgba16Float` surface on Direct3D 12 is linear scRGB, so
  the copy also linearises (sRGB's curve mirrored about 0). Where a canvas lacks extended tone
  mapping the values are clamped to 0 to 1, which is what a display without extended range shows.
- `B8G8R8X8_UNORM` vertex elements and typed buffers (typed buffer views of it are refused by
  name, as they would read the X byte): the RTX 3080 reads blue, green and red with 1 in the
  fourth component (`b8g8r8x8_elements_read_as_direct3d_reads_them`, ignored). No `VertexFormat`
  reads so. The route: a variant `VertexFormat::Bgrx8Unorm` in the interface, which the WebGPU
  back end converts as it does `Float11_11_10` (vertex.rs, into `Unorm8x4` with the fourth byte
  255) and the WebGL 2 back end reads as `BGRA` with the fourth component given 1; typed buffers
  of it then decode through the same format (`TexelBufferAsStorage`). Since G4h this track may
  change the interface and both back ends, but the variant breaks the exhaustive matches of the
  shaders crate (variant/pull.rs `layout`) and of wcontainers-gpu-test (draw.rs, suite/routes.rs)
  the moment it lands in the shared worktree, so it waits on one step with those arms (Asked of
  the shaders track; the Direct3D 12 track asked the same). Wine test_vertex_formats runs 98
  checks of real Direct3D's 100 and fails none.
- Found by the requirement test (G4h run 1), multisampling the tables require and the RTX 3080
  reports, which the format comparison had masked as section 9's per-format shortfall:
  `R32G32B32_FLOAT`'s multisample load and resolve and `R32G32B32_UINT` and `_SINT`'s multisample
  load (10_1 and up; the resolve from 10_0): the front holds these formats in their four-channel
  twins and makes no multisampled twin; WebGPU multisamples `rgba32float`, `rgba32uint` and
  `rgba32sint` where the adapter does (natively on the RTX 3080), so this is the front's to build
  (multisampled twins, resolved through the twin). And the multisample resolve of
  `B5G5R5A1_UNORM` and `B4G4R4A4_UNORM` (10_0 and up), which comes with the packed targets' route
  (What comes next, 1, for all three 16-bit packed formats).
- Blending into the 8-bit signed and 16-bit normalised formats where the back end does not draw
  into them (a browser without texture-formats-tier1; natively
  `WEBGPU_OFF=wgpu-texture-adapter-specific-format-features`, or
  `wgpu-texture-format-16-bit-norm` for the 16-bit ones): textures of them drawn into are then
  held as their integer twins (`NormAsInteger`), which no pass blends; blending into one is
  refused by name (`BLEND_INTO_INTEGER_TWIN`) and `CheckFormatSupport` withdraws `BLENDABLE` and
  the multisample bits from the nine such formats (`R8G8B8A8_SNORM`, `R8G8_SNORM`, `R8_SNORM`
  and the six 16-bit ones). Plan section 9 requires them blended at every
  level, by `BlendInShader`: the shaders crate's `TargetInShader::combine`, refused there
  (`BLEND_IN_SHADER`). The shaders track's (Asked). With the features present (the RTX 3080
  natively, browsers with tier1) nothing is withdrawn.

The owner ruled on 2026-11-22 that no level is reported whose requirements are not met, and
that the missing abilities are built (milestone G4h). `required::LEVEL_REQUIRED` holds Microsoft's
10_0, 10_1 and 11_0 format tables (every required bit of both words and the 4- and 8-sample
counts), and programs.rs `every_format_requirement_of_levels_10_0_to_11_0_met_where_direct3d_meets_it`
compares ours against them wherever the RTX 3080 meets them (ignored as a known failure until the
three items above are built). The device's level is not yet chosen by that table: under it, the
missing `B8G8R8X8_UNORM` vertex buffers (required from 10_0) would take every build to 9_3. Once
the items are built, `Device::new` takes the highest level with nothing unmet, which also keeps
browsers lacking abilities from claiming a level (What comes next).

Absent features (each reported absent through its own query, so Wine's verdicts there differ):
every failing test of Wine's d3d11 file but test_format_support and test_swapchain_formats (the
11_0 shortfalls above) and test_vertex_formats (fewer checks, the B8G8R8X8 shortfall) sits
behind one of these:
- The 11_1 device and Wine's WARP fallback: `D3D11CreateDevice` with 11_1 alone fails, logic
  operations have no route (plan section 9), and this library offers no WARP adapter:
  test_state_refcounting, test_logic_op, test_create_shader, test_shared_resource (its 11_1 part).
- Debug devices (`D3D11_CREATE_DEVICE_DEBUG` fails) and the 11_1 device: test_device_interfaces.
- Min/max filtering (`D3D11_OPTIONS1.MinMaxFiltering` FALSE): test_filter_minmax.
- Video (`QueryInterface` for `ID3D11VideoDevice` and `ID3D11VideoContext` answers
  `E_NOINTERFACE`, and `D3D11_CREATE_DEVICE_VIDEO_SUPPORT` is refused with
  `DXGI_ERROR_UNSUPPORTED`): test_immediate_context (1 check, the video context),
  test_video_support (1 check of 49: the hardware device with video support, which real Direct3D
  makes; real Direct3D runs 51 there; no crash since main run 2), test_h264_decoder.
- Mapping default buffers (`D3D11_OPTIONS1.MapOnDefaultBuffers` FALSE, plan section 9):
  test_resource_access (6 checks at 11_0) and the 11_1 device.
- A vertex shader's layer and viewport (`D3D11_OPTIONS3.VPAndRTArrayIndexFromAnyShaderFeedingRasterizer`
  FALSE): test_layered_rendering skips that part (2 checks); its other 72 pass.
- Presenting to another process's window (the swap chain refusal, Departures): test_desktop_window
  runs 23 checks of real Direct3D's 28 and fails none; Wine's test accepts the refusal as `broken`.
Wine's d3d10core file: test_desktop_window alone, as above.

Not on Wine's verdicts, against the RTX 3080:
- `ClearView` of float depth (`D32_FLOAT`, `D32_FLOAT_S8X24_UINT`) to a value outside 0 to 1
  (NaN among them): the RTX 3080 stores the value as given; WebGPU's depth clears refuse such
  values and its draws clamp depth to 0 to 1, and float depth textures cannot be copied or stored
  into, so no route can write it. Refused by name, which removes the device (a `ClearView` has no
  result). Needs a route the interface does not have; no program measured reaches it.

The WebGL 2 build (10_0; the bar's WebGL 2 cases match real Direct3D through it): what it does
not give real Direct3D's verdict on (agents run 1: Wine's d3d10core 96 of 111; the rig's scenes
95 pass, 17 fail, 30 absent), each with its owner:
- Multisampling: plan section 9 makes it optional at 10_0, and without a route for a partial
  sample mask (WebGL 2's `gl_SampleMask` needs OES_sample_variables and `SampleMaskInShader`,
  which is not built) none is reported, so multisampled textures are refused as Direct3D refuses
  a count it does not report (main run 3; before, 4 samples were reported and a partial mask
  removed the device). Wine test_sample_mask and test_multisample_resolve, and the skipped
  multisampled parts of test_alpha_to_coverage (51 checks of 67) and
  test_check_multisample_quality_levels (21 of 43); the rig's msaa scene.
- Multisampled textures read by shaders (absent on WebGL 2, plan section 6):
  test_unbound_multisample_texture.
- A depth bias clamp on float depth, refused by name (Direct3D's float step follows each
  primitive's largest depth exponent, which a pixel program does not see): test_depth_bias.
- Wine's test crashes (exit 0xc0000005) after a refusal removed the device, in test_depth_bias
  (the clamp above), test_texture and test_render_a8 (`A8_UNORM`, below): a later creation answers
  `DXGI_ERROR_DEVICE_REMOVED` and Wine's test uses the object it was not given. The crash is the
  test's; the gap is the refusal.
- Cull distances (`CullDistanceAsGeometry` needs compute): test_combined_clip_and_cull_distances.
- Views in another format of the group (`Features::TEXTURE_VIEWS`, the WebGL 2 back end's):
  test_create_shader_resource_view, the rig's views scene.
- `A8_UNORM`, which the WebGL 2 back end lacks: test_create_texture2d, test_texture,
  test_render_a8, test_clear_render_target_view_2d.
- Abilities Microsoft's 10_0 table requires that the WebGL 2 build reports absent (the fifth
  check's comparison with a real 10_0 device): render targets of the six 8- and 16-bit signed
  normalised formats (WebGL 2 draws into none without `EXT_render_snorm`, which the back end does
  not use); textures of `R8G8_B8G8_UNORM`, `G8R8_G8B8_UNORM`, `B5G5R5A1_UNORM`, `B4G4R4A4_UNORM`
  and `A8_UNORM`. The WebGL 2 back end's format table (Asked). A probe lost the WebGL 2 device at
  `R16G16B16A16_FLOAT`'s staging read back ("texture copy made through a buffer", then GL error
  0x502); not traced.
- test_format_compatibility (16 checks): a 16-bit depth texture copied into an `R16_TYPELESS`
  colour texture held as `R16Float`, copied by a draw that makes half-float NaNs one NaN (0x7fff).
  The WebGL 2 track's copy route (Asked).
- test_generate_mips (56 checks, all on 3D textures' second level): the WebGL 2 back end's. The
  front makes the same `generate_mips` call on both builds; the WebGPU back end takes the
  interface's default (`generate_mips_by_draws`) and passes all 410 checks, as real Direct3D's
  run counts them, while the WebGL 2 back end takes its own `glGenerateMipmap` route for a whole
  3D view (wcontainers-gpu-webgl lib.rs, `generate_mips`). Read from the code (Asked of the WebGL
  2 track).
- Wireframe without WEBGL_polygon_mode (`FillModeConverted`, not built for any back end); pixel
  shaders reading `SV_PrimitiveID` without a geometry shader (needs compute): the rig's raster and
  geometry scenes.
- Comparisons through colour views (new this run) were not run through the WebGL 2 build.
- Wine's d3d11 file through the WebGL 2 build (not on the bar): 78 of 166 at round 2 run 12.

Real programs: the three of the pass bar (godot-platformer-3d-d3d11, chocolate-doom,
atg-simplelightingpc) match real Direct3D on every compared frame (logs/a3b-programs.log, agents run 3), and so do
atg-simplebezierpc, atg-simpleinstancingpc, godot-platformer-2d-d3d11 and sdk-tutorial02 and 04
to 07; of bgfx's six (logs/q2f-bgfx.log, round 4 run 2) five pass or run to their end. Outside
the bar's three, three differ:
- bgfx-26-occlusion: 6,458 pixels beyond the tolerance in each saved frame, all in one line of
  the overlay: real Direct3D's frames print "GPU mem: 341 MiB / 10 GiB", ours leave that line out
  (scratch m2bgfx, crops of both frames; every other pixel within the tolerance). bgfx prints it
  only when DXGI gives a memory budget (`IDXGIAdapter3::QueryVideoMemoryInfo`); our adapter is
  `IDXGIAdapter1` alone and reports memory sizes of zero, as no browser API reports memory (the
  infrastructure crate's rule). An absent feature, reported absent through `QueryInterface`; the
  occlusion queries themselves match. Found in main run 1; earlier runs thought it the counts.
- atg-simplecomputepc: every pixel inside the Mandelbrot set differs; the converted compute
  program's division gives just below 4 where real Direct3D gives 4 (inferred). The shaders
  track's (Asked).
- atg-simplemsaa_pc: frame 10 has 8 pixels beyond the channel tolerance (largest 11), 4 of them
  beyond their tiles' allowance; frames 60 and 120 pass. Real Direct3D's own two runs had 0 and 1
  such pixels in frame 10 (logs/q3j-sysruns.log); the cause is not found.

Listed cases of section 12, each with its test against real Direct3D:
- Every listed case passes against real Direct3D on WebGPU, the WebGL 2 cases through the WebGL 2
  build, and the cases without texture-formats-tier1 or 16-bit norm.
- Failing: constant buffers updated per draw, counted on the WebGPU back end itself (com.rs
  `constants_rewritten_between_draws_make_no_bind_group_per_draw_on_webgpu`, ignored as a known
  failure): after each buffer's first draw, 2 more bind groups for 2 more draws, for
  `Map(WRITE_DISCARD)` and for `UpdateSubresource` alike. Cause: the WebGPU back end puts a buffer
  in its uniform arena only when `desc.usage == BufferUsage::UNIFORM` and the intent is `Often`
  (wcontainers-gpu-webgpu src/lib.rs, `create_buffer`), while the interface makes `COPY_DST` the
  condition of `write_buffer`; no buffer a front may write is uniform alone. This front declares
  constant buffers `UNIFORM | STORAGE | COPY_SRC | COPY_DST`. The WebGPU track's (Asked). The
  drawing itself matches real Direct3D (`own_handled`).
- Failing: aliased lines (`MultisampleEnable` FALSE) into a multisampled target
  (`aliased_lines_into_a_multisampled_target_hit_every_sample_as_direct3d_does`, ignored): real
  Direct3D covers every sample of the pixels the diamond rule picks; WebGPU draws lines into a
  multisampled target by its own rule. Needs an aliased-line route (the shaders track and the
  interface).

Differences from the RTX 3080 that Wine's verdicts do not show:
- `GetDimensions` of a view of a YUY2 texture: the RTX 3080 reports the texture's width in pixels,
  ours the view's texels (pixel pairs). The interface has no helper value giving a program another
  width than its texture's (interface and shaders tracks; Asked). Known failure
  `yuy2_view_sizes_read_as_direct3d_reads_them` (ignored).
- An NT handle of a shared resource is an event object made by this library, where Windows makes
  a `DxgkSharedResource`: duplicating, closing and opening it work; its object type differs.
- Shared resources are shared between the devices of one process; a handle from another process
  is not found (`E_INVALIDARG`), where Windows opens it.
- A swap chain's back buffer used after the program released the swap chain: real DXGI keeps it
  working; ours destroyed its texture and the next view lost the device (not fixed).
- `ClearView` of an integer storage view with negative values: the RTX 3080 writes -5 into an
  8-bit unsigned channel as 0xfb and any negative value into a 16-bit signed channel as 0x7fff,
  and its whole and rectangle clears differ from each other; ours clamps to the channel as the
  functional specification (5.2.3.3) says. The listed case leaves negative storage values out.
- Target clears of integer formats convert as the RTX 3080 does (`target_integer`), which is not
  the specification's clamp for values past 32 bits (Departures).
- `ClearView` of a buffer view's rectangles, and of rectangles of a storage view held as an
  integer twin or stored through a buffer, of a 3D texture or one no pass draws into, are refused
  by name. No program measured reaches them.
- `GenerateMips` of an integer view (no `MIP_AUTOGEN` claimed by either side): ours leaves the
  levels and warns once; the RTX 3080's driver fills them by its own rule (Where things stand).
- Creation refusals answered `E_NOTIMPL` where the RTX 3080 answers `E_INVALIDARG` (both fail;
  only the code and the warning differ): render target views and mip generation of 1D, 3D and
  array textures of the R32G32B32 formats; textures of `B4G4R4A4_UNORM` bound as render targets
  of every dimension, plain 2D included (fifth check's probe12 b5); 1D, 3D, array and cube-array
  textures of `R1_UNORM` and of the video formats ours holds no texture of; shared textures of
  P016 and NV11; typed buffer views of `R1_UNORM`, the extended-range format and the video
  formats. 22 formats differ so in the eighth check's probe (probe8/out). Needs Microsoft's
  per-level table at 10_0 and up in the front; not built.
- d3d11 test_clear_buffer_unordered_access_view passes through ours, where real Direct3D fails 223
  of its 934 checks: ours clears by the documented rule; NVIDIA's driver does not.
- `IDXGISwapChain::GetContainingOutput` answers `E_NOTIMPL` (the infrastructure crate's).
- `B8G8R8X8_UNORM` and its sRGB twin: the X byte reads 0xff after a clear, a draw, a blend, a
  multisample resolve and initial data, where the RTX 3080 writes 0x00 for clears, draws, blends
  and resolves and keeps the bytes it was given (the
  WebGPU back end's `bgra8unorm`, src/packed.rs; Asked).
- Values just past a half between two codes drawn into a 16-bit normalised texture held as its
  integer twin: 6553.5006 is written 6553 by the RTX 3080 and 6554 by ours; both within the
  specification's tolerance. Filtered samples and mips of twins differ by 1 ULP at most.
- `GenerateMips` of `R8G8B8A8_SNORM`: the RTX 3080 averages the bytes as unsigned (levels of
  -0.4 and 0 give 0x66, +0.8; measured in main run 3, logs/m3e-lib.log), where `R8G8_SNORM` and
  `R8_SNORM` average the signed values; ours averages the signed values for every format, as the
  functional specification filters. The normalised targets case varies positive channels only.
- `ID3D11Device3::ReadFromSubresource` and `WriteToSubresource` are not served: they warn and
  remove the device. No valid call reaches them (`D3D11_OPTIONS2.MapOnDefaultTextures` FALSE).
- A swap chain on another process's window is refused with `DXGI_ERROR_INVALID_CALL` (Departures).
- Drawing a geometry stream past 0 is refused by name (`GEOMETRY_STREAM_DRAWN_PAST_ZERO`).
- Building ahead at a shader's arrival (plan 4.4) has never been hit. Traced in Godot 3D on main
  (a build with a print per shader made, pair tried and combination first drawn, not committed;
  scratch programs/m1dbg, main/ahead-analyse.py): it makes 98 vertex and 99 pixel shaders, mostly
  a pixel shader then its vertex shader, and draws 36 programs in 62 first-drawn combinations. The
  pairing finds 34 of the 36 drawn programs; the misses are the tuples. Of the 32 drawn pairs that
  were prepared, every one was drawn with another input layout and other strides, targets or
  binding layouts than the tuple picked, and 28 also with other blend, depth or raster objects: the
  layout Godot (through ANGLE) makes for a program is made at or after its first draw, and its
  draws do not use the most drawn tuple. Pairing each new shader with the newest unpaired one of
  the other stage would halve the pairs tried (98 for 186) with the same 34 drawn, but it never
  prepares a second pixel shader for a vertex shader already paired (front test
  `a_new_pair_is_built_ahead_with_the_states_its_vertex_shader_was_drawn_with`), so it was not
  made. The cost is about 110 to 124 background pipeline builds per Godot 3D run that are never
  used; the recorded combinations built at the next start are what serves these programs (all 61
  of Godot 3D's drawn, Measurements).

## Format support against the RTX 3080

Every bit of both words of every format, and the second query's result, compared with real
Direct3D on devices of 11_0 (`format_support_as_direct3d_reports_it_but_the_named_shortfalls`),
10_1, 10_0, 9_1, 9_2 and 9_3, and at 11_0 without the 16-bit normalised formats; the creations of
9_x line by line (`level9_make`). The differences, by cause:
- Absent features, never claimed: video (the first word's bits from `DECODER_OUTPUT` up), logic
  operations, tiled resources and multiplane overlays. Where these were a format's only
  second-word bits, ours fails the second query as Direct3D fails a word without bits.
- Multisampling per format (plan section 9's listed shortfall): claimed only where the back end
  draws a count of the format, a subset of real Direct3D's.
- Optional and served: `R32G32B32_FLOAT` is sampled (and at 11_0 gathered) by ours from 10_0.
- The 8-bit signed and 16-bit normalised formats held as their integer twins (without
  texture-formats-tier1 or 16-bit norm) claim `SHADER_LOAD` and `SHADER_SAMPLE` as the RTX 3080
  does and are read decoded in every stage, compute included (agents run 2).
- Optional video formats absent altogether: AYUV, Y410, Y416, P010, P016, Y210, Y216, NV11, AI44,
  IA44, P8, A8P8.
- Shortfalls of the reported level, in the Gap list: the B5 targets (at every level),
  `R10G10B10_XR_BIAS_A2_UNORM`'s display and back buffer casts and `B8G8R8X8_UNORM` elements and
  typed buffers (10_0 and up). `A8_UNORM` storage views are served since G4h run 1.
- Below 10_0, as the RTX 3080 answers: only the 10Level9 table's bits; no buffers, comparisons,
  gathers or storage; `SHAREABLE` for the 17 formats the RTX 3080 reports it for, and shared
  textures made of them but `R8G8B8A8_UNORM` and its sRGB twin (both refused there, measured in
  check-r15's probe11 share9). This copies the RTX 3080's own contradiction at 9_x: those two
  are reported shareable and then refused, and `R32_FLOAT` is made though not reported. Ours
  follows the measured answers rather than its own rule; for the owner (Departures).
- Below 11_0: sharing is claimed from 10_0 and the second query answers it alone; gathers at 10_1
  only for formats of a red channel alone, uncompressed, one aspect of depth, NV12 and YUY2;
  multisampled depth is not read at 10_0.
- `D3D11_OPTIONS2.TypedUAVLoadAdditionalFormats` TRUE at 11_0 where the set claims typed loads;
  at 10_x as the RTX 3080 answers (compared at each level).
- Every creation ours claims succeeds and every one it serves is claimed, but those where the RTX
  3080 itself claims and refuses alike (cubes of the depth formats, NV12 and YUY2 views without a
  description; the seventh check's probe). `CheckMultisampleQualityLevels` matches the RTX 3080
  for every value 0 to 200 at counts 1 to 33 at 11_0 but the B5 formats' counts 2 to 8 and the
  absent video formats' single sample; at every level its 4-sample answer is compared per format
  (the same code, levels only where the RTX 3080 has them).

## Known failures

- `aliased_lines_into_a_multisampled_target_hit_every_sample_as_direct3d_does`,
  `b8g8r8x8_elements_read_as_direct3d_reads_them`, `yuy2_view_sizes_read_as_direct3d_reads_them`
  and `every_format_requirement_of_levels_10_0_to_11_0_met_where_direct3d_meets_it`
  (programs.rs) and `constants_rewritten_between_draws_make_no_bind_group_per_draw_on_webgpu`
  (com.rs) fail on every build and are marked `#[ignore]` with the reason (Gap list); run them
  with `--include-ignored`.
- On the WebGL 2 build only: the library tests that ask for 11_0 or 10_1 fail, as the build
  reports 10_0, or reach the absences of the Gap list's WebGL 2 item. The bar's WebGL 2 run takes
  the 10_0 cases alone; of those, `direct3d_10s_device_draws_and_keeps_its_state_as_direct3d_does`
  (an 11_0 device with a 10_1 state) is marked ignored on that build with that reason.

## What comes next

In order (G4h, 11_0 reported truthfully):
1. B5 targets, blended, in the WebGPU back end (this track's since G4h): declare `RENDER`,
   `BLEND`, `RESOLVE` and the multisample counts of `rgba8unorm` for `B5g6r5Unorm`, `Bgr5a1Unorm`
   and `Bgra4Unorm` (lib.rs, converted formats; the tables require the B5G6R5 target with 4 and 8
   samples and the other two's resolves), write mask RGB for `B5G6R5` (alpha stays one), and
   at `end_pass` quantise every such attachment drawn into: copy the attached level and layer to
   a scratch `rgba8unorm` texture and draw it back through a program writing each channel's code
   `round(v * (2^n - 1))` widened by repeating its bits (packed.rs `widen`'s rule), per sample with
   `@builtin(sample_index)` for multisampled targets, and the same after a resolve into one.
   Named slow path (a new `SlowPath::PackedTargetQuantised`, interface commit), counter, warning.
   Within the specification: the output's two roundings stay under its 0.6 ULP (0.56 at most) and
   blending at 8 bits is "equal or more precision" (17.5); a pass blending twice into one texel
   rounds once, at its end. Test: own_cases drawing, blending and resolving into both formats
   against the RTX 3080, and Wine's test_format_support.
2. `VertexFormat::Bgrx8Unorm` in one step with the shaders crate's and wcontainers-gpu-test's
   arms (Gap list); then un-ignore `b8g8r8x8_elements_read_as_direct3d_reads_them` and serve typed
   buffers of the format.
3. The extended-range display, in the infrastructure crate (Asked of the Direct3D 12 track; the
   route is in the Gap list).
3b. Multisampled four-channel twins of the `R32G32B32` formats, loaded and resolved (Gap list).
4. With 1 to 3 built: `Device::new` takes the highest level with nothing unmet in
   `required::LEVEL_REQUIRED` (the table needs no correction: the only required bits the RTX 3080
   does not report are those of P016 and NV11, formats the tables leave optional and it lacks),
   un-ignore the requirement test, and remove the three rows from `support_differences`. Run the requirement test without texture-formats-tier1,
   16-bit norm and float32-filterable too: browsers lacking them must then report a lower level
   unless the abilities are built another way (the nine integer twins' blending through the
   shaders crate's `combine`, built for single-sample targets in its run 80).
5. The YUY2 view's size, once the interface and shaders tracks give the helper value (Asked).
2. Constant buffers rewritten per draw: once the WebGPU back end's arena admits uniform buffers
   with other usages (Asked), un-ignore the counters test and run it.
3. Building ahead at a shader's arrival: the measured miss is the tuple (Differences from the RTX
   3080). What could hit: build again when an input layout arrives (it names its vertex shader's
   signature), and pick tuples by the targets and binding layouts last drawn rather than the most
   drawn tuple; measure with the same trace.
4. A back buffer kept by the program past its swap chain's release: keep the chain's textures
   alive while a buffer is held.
5. Blending into integer twins (`BlendInShader`) once the shaders crate builds `combine`.
6. Creation refusals of optional abilities ours does not claim as `E_INVALIDARG` at 10_0 and up:
   needs the per-level requirement table (9_x has it since round 4 run 6).
7. Run the comparison case and the 9_x tests through the WebGL 2 build (its 9_x fallback of plan
   section 9); prove whose the WebGL 2 build's 3D second-level failure of test_generate_mips is.
8. Replace the cube array probe (`makes_cube_arrays`) with a `Caps` feature once one exists.
9. Wine's dxgi file: the null-out-pointer crashes, `CreateSurface`, `CreateSubresourceSurface`.
10. Section 10's joint step with the Direct3D 12 track, after both milestones pass.
11. The literal fix list, a part per run.

## How to run

Everything builds and runs on the desktop through the rig (docs/status-graphics-rig.md, "How to
run"); the Mac runs only formatting, git, `cargo check -j 2` of the front, and the front's tests
on the rented x86 machine. Scripts named below are in scratch (pyro-parked/wcontainers-scratch/
graphics-direct3d-11); logs go to its logs/. Since every track is on one branch (agents), the rig's own commands
would give all tracks one desktop folder (they name it by the branch), so this track runs the copies
in scratch main/ (the scripts below, run from the checkout after `source main/env.sh`), which
use scratch rigmain/: the rig with the desktop folder named `phase-2-graphics-direct3d-11`
(`WC_RIG_BRANCH`) and only committed trees sent, so other agents' unfinished edits are never built.
Commit before running anything. Do not send this checkout's tree (remote, gpu-job,
scenes) while another of its jobs waits in the queue: the waiting job then stops ("holds tree
..., not the sent ..."). Never run `tools/graphics-rig/gpu-job --help` (it queues `--help` as a
command). The desktop's checkout needs the shaders crate's submodules, so commands start with
`git submodule update --init --recursive -q`.

- The whole bar in one command: `q5-final.sh <label> [steps]` (the literal check, lint of both
  builds, the library suite, the WebGL 2 build's 10_0 cases, Wine's two files, scenes, programs,
  sizes and load times, building ahead, Wine's d3d10core through the WebGL 2 build). Steps:
  literals lint lib webgllib bar sizes warm wine10gl.
- Library tests matching a filter: `run-q1.sh <label> "lib=--test programs -- <filters>
  --test-threads 1"`; `r6/r6b.sh <label> "<filters>"` runs filters and then real Direct3D's own
  output of some cases.
- The literal check at HEAD (commit first): `lit-check.sh <label>`; a `-` line it prints is a
  baseline line to delete in the same commit.
- Lint: `tools/graphics-rig/remote "git submodule update --init --recursive -q && cargo clippy -q
  -j 3 -p wcontainers-direct3d-11 -p wcontainers-directx --all-targets -- -D warnings"`, and the
  same with `--features wcontainers-directx/webgl --target-dir %CARGO_TARGET_DIR%\webgl`.
- Front tests (recording back end, no GPU): `orchestrator/cloud/x86-run <clone> --name g-d3d11
  --jobs 32 -- "cargo test -q -p wcontainers-direct3d-11"`, where `<clone>` is a scratch
  `git clone --shared` of the commit to test with the submodules' files copied in (x86-run wants a
  git checkout; the worktree itself holds other agents' unfinished edits).
- Library tests (real devices and a window, so a GPU job on the hidden desktop):
  `tools/graphics-rig/gpu-job --desktop --build "<sub> && cargo test -q -j 3 -p wcontainers-directx
  --no-run" -- "cargo test -q -j 3 -p wcontainers-directx -- --test-threads 1"`; the WebGL 2 build
  with `--features webgl --target-dir %CARGO_TARGET_DIR%\webgl`. The random lifetimes comparison
  takes `WC_LIFETIMES=<first seed>,<seeds>,<steps>` (default 1,8,300); a failure names the seed.
- Wine's files through the library (build it first with `cargo build -q -j 3 -p
  wcontainers-directx`): `rigfix/wine.sh --dll 'd3d11.dll=%CARGO_TARGET_DIR%\debug\wcontainers_directx.dll'
  d3d11[:test_a/test_b]`, and `--dll 'd3d10.dll=...'` with `d3d10core`. rigfix/wine.sh is the rig's
  wine-tests with its runner fixed in one line. Wine's dxgi file: `run-dxgi.sh <label>
  [test_a/test_b]` (needs the test-only d3d10_1.dll forwarder in
  C:\wc\out\phase-2-graphics-direct3d-11\d3d10_1fwd; remake it with link.exe from its .def there).
- Scenes and programs through the whole library (our DXGI too): `--dll
  'd3d11.dll=%CARGO_TARGET_DIR%\debug\d3d11.dll' --dll 'dxgi.dll=%CARGO_TARGET_DIR%\debug\dxgi.dll'
  --dll 'wcontainers_directx.dll=%CARGO_TARGET_DIR%\debug\wcontainers_directx.dll'` on
  `tools/graphics-rig/scenes` or `tools/graphics-rig/programs run`; always the three together.
  bgfx's examples also take `--dll 'nvapi64.dll=C:\wc\out\phase-2-graphics-direct3d-11\nonvapi\nvapi64.dll'`.
- The WebGL 2 build loads ANGLE from `WEBGL_ANGLE_DIR` (default C:\wc\tools\angle-mozangle-0.7.1)
  and takes the WebGL extensions `WEBGL_OFF` lists as missing; `WEBGPU_OFF` does the same for
  WebGPU features on the WebGPU build.
- Load time and release sizes of both builds and of the Direct3D 12 track's module:
  `run-r8-sizes.sh <label>` (example `load_time`).
- src/com/generated.rs: `python3 crates/wcontainers-directx/generate.py <windows-0.62.2 crate dir>`,
  then `cargo fmt -p wcontainers-directx`. A method named in its `hand` table calls the hand-written
  one.
- If a run on the desktop does not reflect a change (its clock once moved back), run
  `tools/graphics-rig/remote "cargo clean -q -p wcontainers-directx -p wcontainers-direct3d-11"`.

## How it is built

- `Device<B: Backend>` holds the back end, the immediate context's state and caches of programs (per
  vertex and pixel shader pair), vertex layout blocks (per input layout and strides) and vertex
  shaders' input signatures (by blake3 of the bytecode). Front objects hold back-end handles in
  `Kept`, which on drop hands the handle to a graveyard emptied at the next call.
- Shaders (src/shader.rs): the stage is read from the DXBC container's version token, so bytecode of
  the wrong stage is `E_INVALIDARG`; then `convert` with the default `VariantKey`. Bytecode the
  converter rejects is `E_INVALIDARG`; what it cannot convert yet is `E_NOTIMPL`, warned with the
  converter's reason.
- Binding sets: set 0 vertex, set 1 pixel, made from the reflection when a slot or shader changed.
- Empty slots read what Direct3D defines (`Zeros` in device.rs, each made at its first use).
- Views a resource cannot be seen as natively are made on a twin of the view's format
  (`View::twin`), copied in before each use and out after each write; colour views a program
  compares through read a depth copy (`View::compared`); both are `ReinterpretCopy`.
- Format support (`Device::format_support`): the infrastructure crate's answer from the back end's
  uses, with what the front serves by its own routes, what the feature level allows, and below
  10_0 the 10Level9 table (`format::LEVEL9_SUPPORT`), which creation checks too.
- Creation checks run in the front before any back-end call and answer `E_INVALIDARG` as Direct3D
  does; what is valid but not served answers `E_NOTIMPL` with a warning. A draw that cannot be
  served removes the device; COM methods not served answer `E_NOTIMPL`, and those without a result
  remove the device, each warned once.
- The Windows library: `wcontainers_directx.dll` exports `D3D11CreateDevice`,
  `D3D11CreateDeviceAndSwapChain`, `D3D11On12CreateDevice` and Direct3D 10's entry points; the
  device, context and adapter are made with the `windows` crate's `implement`, the device children
  with our `counted!` (two counts); src/com/generated.rs is generated, each method taking the
  library lock and calling the hand-written method.
- The back end is wgpu on Direct3D 12 (`com::backend`): what browsers' WebGPU uses on Windows.
- build.rs makes d3d11.dll, dxgi.dll and d3d10core.dll with the MSVC linker from generated `.def`
  files: no code, each export forwarding to `wcontainers_directx`.

## Tests

At G4h run 1: front.rs 60, tests/cache.rs 1, unit 6 (shader.rs 5, stream.rs 1;
wcontainers-direct3d-11, recording back end); com.rs 16 (1 ignored) and programs.rs 64 (4 ignored)
(wcontainers-directx, real devices). New in G4h run 1: programs.rs
`a8_storage_views_store_as_direct3d_does` (own_cases `a8_storage`) and the requirement test
(ignored, known failure). New in round 4 run 6: programs.rs `comparisons_through_colour_views_as_direct3d_does`
(own_cases `compare_colour`, 16-bit norm on and off), `depth_clears_outside_the_range_as_direct3d_does`
(`depth_clears`) and `format_support_and_creations_below_10_0_as_direct3d_reports_them` (9_1, 9_2
and 9_3, `format_support` and `level9_make`); front.rs
`devices_below_10_0_refuse_what_the_10level9_tables_do_not_give`. programs.rs runs each example
once beside the system's d3d11.dll and once beside ours and compares what each printed; the random
lifetimes comparison is seeded (`WC_LIFETIMES`, default seeds 1 to 8 at 300 steps each). Every
listed case of section 12 has its test against real Direct3D.

## Departures from the plan

- Integer target clears follow the RTX 3080 where it departs from the functional specification's
  clamp for values past 32 bits (`target_integer`); storage views keep the clamp
  (`storage_integer`), which the RTX 3080's storage rectangles follow.
- Below 10_0, format support and creation follow the runtime's 10Level9 answers measured on the
  RTX 3080 (`LEVEL9_SUPPORT`): Microsoft's 9_1 and 9_3 tables with the driver's optional rows, and
  for 9_2, for which Microsoft has no table, the measured answer alone.
- Shared resources are shared between the devices of one process only, each device holding its own
  copy, copied through the host when a device flushes or releases the keyed mutex
  (`SharedThroughHost`); a copy is made only where the bytes differ from the last hand-over. NT
  handles are event objects (`OpenSharedResource1` finds a share by `CompareObjectHandles`).
  Multisampled shared textures are refused by name.
- 420_OPAQUE is held as NV12's two planes and refused with bind flags, CPU access or another usage
  than default, as the RTX 3080 refuses them. NV12 is two textures (luma `r8unorm`, chroma
  `rg8unorm` at half size); YUY2 is held at half width; one level each (more answer `E_NOTIMPL`).
- `R10G10B10_XR_BIAS_A2_UNORM` textures are held in `Rgb10a2Unorm`'s bits; no view of the format is
  made, as on the RTX 3080.
- Normalised clear values of integer twins round halves toward zero, as the RTX 3080 clears.
- 16-bit normalised textures held as integer twins are blended nowhere (refused by name) where plan
  4.12 blends them in the program (`BlendInShader`, refused by the shaders crate).
- 11_0 is reported with the shortfalls of the Gap list, against section 9's rule (Gap list).
- A swap chain on another process's window is refused with `DXGI_ERROR_INVALID_CALL` (warned): the
  WebGPU back end presents through a flip-model swap chain, which presents only to the process's
  own windows.
- The names of Windows' runtime libraries and entry points live in the front crate
  (`wcontainers_direct3d_11::runtime`), as the example own_handled must name them and cannot link
  the library crate.
- Wine's dxgi file runs with a test-only d3d10_1.dll forwarder made by hand on the desktop;
  d3d10.dll and d3d10_1.dll are not built as forwarders (they also export the effects framework,
  shader compiling and reflection, which this library does not serve).
- The library reads `WEBGPU_OFF` beside `WEBGL_OFF`, so routes browsers without a feature take are
  tested natively against real Direct3D.
- The feature level check probes the back end for cube arrays at device creation (no `Caps` feature
  names them); a refusal stops the level at 10_0, which is how the WebGL 2 build reports 10_0.
- Typed buffers read through texture copies copy the whole buffer into a texture of each view
  format the first time a draw reads it after a write.
- Aliased lines drawn into multisampled targets by the back end's rule are not compared with
  Microsoft's conformance tolerance (no such comparison exists; Gap list).
- Depth bias applies by the primitives rasterised: lines and points are not biased, as on the RTX
  3080 (plan 5.1 says `DepthBiasInShader`); a geometry stage's biased lines and points are refused.
- A stage reading more constant buffers than the back end's uniform buffers keeps one uniform buffer
  for the helper constants whenever its variant may have them.
- A geometry shader's primitive naming a layer past the bound view's is drawn nowhere, where real
  Direct3D draws it into the first layer (Asked of the shaders track).
- A Direct3D 10 device made without an adapter answers its adapter's `GetParent` with DXGI 1.0's
  factory; `CreateDXGIFactory` gives DXGI 1.0's factory only when asked for the 1.0 interfaces.
- `EarlyTestsPerPrimitive` is built for list topologies, one instance, direct draws only; the rest
  is refused by name.
- Direct3D 10's device converts each object it is handed by `QueryInterface`; the system's d3d10.dll
  and d3d10_1.dll calling `D3D10CoreCreateDevice` get ours; in Direct3D 10 mode the final release of
  a bound resource does not unbind it; `CreateGeometryShaderWithStreamOutput` gives the one buffer
  its `OutputStreamStride` (not compared with real Direct3D).
- `TargetsSplit` runs each group's draw in a pass of its own; queries, pixel storage views and stream
  output during such a draw are refused by name.
- An over-release answers 0xffffffff and leaves the counts as they were.
- A multisampled texture of more than one layer is refused (`E_NOTIMPL`) instead of held as a
  texture per layer (plan 4.2).
- Stencil views always move the stencil in the program (`SwizzleInShader`): `TextureViewDesc` has no
  swizzle.
- Programs taking `AtomicExchangeLoop` are refused at creation after one reset the display driver.
- Passes whose depth buffer is larger than their colour targets are begun through the pass tracker;
  the front's other passes are still its own (section 4.7 has every pass on the tracker).
- The examples compile their shaders with the system's d3dcompiler_47.dll at run time.
- An 8- or 16-bit integer vertex element read with the other signedness is fetched in the program
  (`pulled`, `VertexFormatConverted`).
- `IDXGISurface1::GetDC` is a memory DC made as DXGI makes it (`D3DKMTCreateDCFromMemory`), filled
  through a staging copy and written back whole at `ReleaseDC`; no slow path names it.
- A render target view of a buffer is a view of a one-row texture of its elements, copied in at a
  pass's start and out at its end (`ReinterpretCopy`).
- A uint clear of a view of floats is written as the texels' bytes from the host.
- A storage view of some slices of a 3D level is made on a texture of those slices (`View::window`).
- Direct3D 11.3's interfaces are served; tiled resources and conservative rasterization are not
  (not reported); `Flush1` with an event waits on the calling thread.
- bgfx's examples are run with an empty nvapi64.dll beside them, so NVIDIA's NVAPI does not load and
  take our device for its driver's (the Direct3D 12 track does the same).
- The native cache file lives in the directory `WC_GPU_CACHE` names, one per program; without it
  every run is a first run. Building the record happens at the process's first device only.
- Views on twins and storage textures read through buffers are copied at every use, without
  tracking which copy is newest (correct, slow, counted).
- A stream output target's filled size lives in a 4-byte GPU buffer; `DrawAuto` reads it back.
- The stage helper's buffers and programs are kept per (vertex shader, input layout, geometry
  shader, pixel shader) for the device's life.
- A texture view whose kind differs from its slot's declaration reads as an empty slot, zeros, as
  real Direct3D answers in Wine's test_generate_mips.
- A cube texture whose layers are not whole cubes is a 2D array in the back end.
- A constant buffer window's copy is counted as `ConstantBufferGrown`.
- `EnumAdapterByLuid` with another identifier answers `DXGI_ERROR_NOT_FOUND`;
  `IDXGIFactory3::GetCreationFlags` answers 0.
- With only d3d11.dll replaced, the device made on a system adapter reports our own adapter.
- Depth bias in the program is refused on float depth formats where the back end lacks the clamp.
- Building ahead's counts are printed at each present where they changed.
- `D3D11CreateDevice` takes only `D3D11_SDK_VERSION`.
- Vertex buffers get storage usage on back ends without `INSTANCE_STEP_RATE`; every default-usage
  2D texture of a format that multisamples and resolves gets render usage (a resolve's destination).
- `LodBiasInShader` differs by one unit at 4 of 256 texels (measured); biased rows are compared
  within one unit.
- Pipeline statistics' vertex shader invocations equal the vertices fetched; stream output
  statistics answer zero.
- A sampler whose `MinLOD` is above its `MaxLOD` is made with the maximum raised to the minimum.
- Timestamps are answered with `Disjoint` TRUE and CPU times (no back end writes GPU timestamps).
- `CheckMultisampleQualityLevels` and `CheckFormatSupport` are served through a copy of the
  device's function table with those two entries replaced.
- An indirect draw whose vertex program reads `SV_VertexID` or `SV_InstanceID` has its arguments
  read back (`IndirectReadBack`); the shaders crate refuses `DrawBaseFromArguments`.
- The draw bases' helper block is kept between draws with equal bases, the front being the only
  writer of the back end's data ring.
- Building ahead pairs shaders and fits tuples by location and scalar type.
- DXGI's factory entry points and `D3D11CreateDevice` decide by the caller's module whose library a
  call wants (no device thread exists to mark).
- Comparing through a sampler that does not compare (and the reverse) binds a twin sampler.
- The WebGL 2 build is a cargo feature (`webgl`) choosing the back end type.
- No call buffer or device thread: COM calls run on the calling thread under one re-entrant lock.
  Plan 15.1 moves the channel, its encoder and this library's device thread to the second batch
  (G7), where the three fronts' shells move onto it together. So section 13's cost of the creation
  checks per shader is not measured apart: here a check and its shader's conversion are one call
  on the program's thread, with nothing to separate them from.
- Device children are made by our `counted!` macro, not `implement`, for Direct3D's two counts;
  windows-core is pinned to exactly 0.62.2 for its object layout.
- The recording back end is in the front crate (`record.rs`), not G1's wcontainers-gpu-test.
- State objects are kept by the device for its life once made.
- `CheckFeatureSupport` answers `D3D11_FEATURE_THREADING` with both FALSE.
- The feature level is 11_0 at most.
- The adapter reports zero for its memory sizes (the infrastructure crate's rule).
- `D3D11CreateDevice` with an adapter uses ours whatever is passed; the debug flag answers
  `DXGI_ERROR_SDK_COMPONENT_MISSING`; software, WARP, reference and null drivers, and
  `D3D11_CREATE_DEVICE_VIDEO_SUPPORT` (no video device is served), `DXGI_ERROR_UNSUPPORTED`.
- `D3D11_OPTIONS.ClearView` is TRUE: target, depth and storage views, whole or by rectangles;
  storage rectangles through the shaders crate's storage clear program; 3D target rectangles copied
  into each slice from a cleared slice.
- The library draws through wgpu on Direct3D 12 only.

## For the owner

- Sharing at 9_x copies the RTX 3080's own contradiction: `R8G8B8A8_UNORM` and its sRGB twin are
  reported shareable and refused when made, and `R32_FLOAT` is made though not reported
  (`format::level9_shared`, measured). The other choice is to report only what ours makes, which
  would then differ from the reference. Yours to decide.

## Interface changes

- 116c369d and ff482b63 "interface: named the slow path of a pass split where its query set has no
  index left" (and its wording): `SlowPath::QuerySetSplitPass`, and one clause in plan 4.7.
- 2f3b3263 "interface: named the faces of a cube once as CUBE_FACES": `wcontainers_gpu::CUBE_FACES`,
  which this front and the NIR track's crate had each defined (the literal check's redefinition
  once both branches were on main); the front now imports it.

- 978d1661 "interface: named the slow path of a resource shared between devices copied through the
  host": `SlowPath::SharedThroughHost`, and one sentence in plan section 9.
- 8b556a05 "interface: named each normalised format's integer twin as Format::integer_twin and read
  generate_mips' twins from it".
- c8a7be9 "interface: copied combined depth-stencil textures whole in copy_texture".
- 7f73347 "interface: named the slow path of a constant buffer grown to its shader's declared size,
  and said a depth attachment holds both aspects".

## Measurements

G4h run 1 (2026-11-22, Pacific). Logs in scratch logs/h1-* to h3-*; the rig copy as in How to run.
- Lint of the front, the library and the WebGPU back end, warnings denied, on the desktop at
  3825e2ab: clean (h2-check). Front 60, unit 6, cache 1 and clippy of the front and the WebGPU back
  end on the rented x86 machine (96 cores, load 0 to 8): pass (h1-front).
- The literal check at 3825e2ab on the rented x86 machine: no finding in this track's crates or
  the WebGPU back end (h3-lit; its lines are other crates' and the scratch clone's submodules).
- Desktop, through the queue, not alone (a display-reset hazard job of another track held it
  before): `a8_storage_views_store_as_direct3d_does` and the seven format support comparisons
  pass (h1-lib); the requirement test fails as listed under Where things stand (h2-lib, 3 s).
- The whole library suite at 3825e2ab (h3-lib, 08:52 to 08:54): com 15 pass, 1 ignored; programs
  60 pass, 4 ignored. Lint of the WebGL 2 build, warnings denied: clean (h3-webgl-check). Wine's
  files, scenes, programs, sizes and load times were not run again (the change touches only
  `A8_UNORM` storage and what the examples print).
Earlier runs' measurements follow (agents run 3).

Agents run 3 (2026-11-21, Pacific). Machine: the desktop (Windows 10, RTX 3080, i9-9900K), through
the queue, not alone except the load times; the shaders track's display-reset hazard jobs held the
queue about 21:21 to 21:34 and 21:59 to 22:24. Code: agents at 3b1cd4f3, which holds this run's
literal commits (scratch main/q5-final.sh a3b, logs/a3b-*; the library suite again as a3c, as its
first try met a run left by the stopped agents run 2 sending another tree to the same desktop
folder, which this run stopped).
- The literal check at 3b1cd4f3: 107 lines to act on, none this track's. Lint of both builds clean
  (21:35 to 21:37).
- Library (a3c-lib, 23:03 to 23:20): com 15 pass, 1 ignored; programs 59 pass, 3 ignored. The
  WebGL 2 build's 10_0 cases: 6 pass, 1 ignored (a3b-webgllib). Front 60, unit 6, cache 1 and
  clippy clean on the rented x86 machine (96 cores, load 0 to 9, a3-front, a3-lit4).
- Wine (21:44 to 21:48): d3d11 151 of 166, the same 15 failing tests as since round 4 run 3;
  d3d10core 110 of 111 (test_desktop_window). Through the WebGL 2 build d3d10core 96 of 111
  (a3b-wine10gl, 22:53 to 23:03), as at agents run 1.
- Scenes through the library 135 pass, 0 fail, 4 absent; the WebGL 2 build's 95 pass, 17 fail,
  30 absent, as before.
- Programs (a3b-programs, 22:24 to 22:26): the bar's three (godot-platformer-3d-d3d11,
  chocolate-doom, atg-simplelightingpc) match real Direct3D on every compared frame, and so do
  atg-simplebezierpc, atg-simpleinstancingpc, godot-platformer-2d-d3d11 and sdk-tutorial02 and 04
  to 07; atg-simplecomputepc (154,515 pixels) and atg-simplemsaa_pc (frame 10: 8 pixels, 4 beyond
  their tiles' allowance) differ as before (Gap list).
- Module sizes (section 10), release builds at 22:30 to 22:32 (a3b-sizes): `wcontainers_directx.dll`
  14,359,552 bytes on WebGPU and 9,930,240 bytes on WebGL 2; the Direct3D 12 track's
  `wcontainers_d3d12.dll` 13,767,680 bytes. The family library itself is the joint step.
- Load time (section 10), alone, `LoadLibraryW` in 20 fresh processes each, medians (a3b-load,
  22:32 to 22:35): the system's d3d11.dll 1,200 us; ours on WebGPU 1,674 us; ours on WebGL 2
  1,150 us; `wcontainers_d3d12.dll` 1,684 us.
- Building ahead (a3b-warm, 22:35 to 22:49): Godot 3D's first run built 54 to 115 ahead and drew
  none; its warm starts built their 61 recorded combinations at start and drew 60 and 61, built
  115 and 124 ahead and drew none, 25 and 26 at their first draw. sdk-tutorial07 and the others
  warm: their recorded combinations built at start and all drawn.
Earlier runs' measurements are in this file's git history.

## Known issues

- Threads: one lock serialises all calls; no parallelism between a program's threads.
- No front (recording back end) tests for 1D and 3D textures or queries; they are checked against
  real Direct3D instead (`own_volumes`, `own_queries`, the rig's textures scene).
- The rig's replacement check cannot see wcontainers_directx.dll behind the forwarder.

## Asked of other tracks

- Shaders track (G4h, blocking 11_0): the arm of `VertexFormat::Bgrx8Unorm` in variant/pull.rs
  `layout` (`Unorm`, bytes 2, 1 and 0 of one word, three components, so the fourth reads 1), in
  the same commit as the variant, which this track would add to crates/wcontainers-gpu with the
  WebGPU, WebGL 2 and wcontainers-gpu-test arms. Either the shaders track makes that one commit
  (this track's arms are given in the Gap list) or it allows this track to add its one arm; the
  orchestrator's call.
- Interface track: a `Caps` feature for cube arrays, declared by the WebGPU back end at the same
  time.
- Infrastructure crate (the Direct3D 12 track, G5; G4h, blocking 11_0 for both fronts): swap
  chains of `R10G10B10_XR_BIAS_A2_UNORM` presented through a decoding copy into an `rgba16float`
  surface (route in the Gap list), and back buffer views of it as `R10G10B10A2_UNORM`;
  `D3D11_FORMAT_SUPPORT2_SHAREABLE` in its `support2` list.
- WebGPU track: `FormatCaps` from the device's format features rather than the adapter's
  (`FILTER` of the 32-bit floats is claimed without float32-filterable while the adapter's own
  format features are on; wgpu-core takes it away on the device);
  `B5G6R5_UNORM` and `B5G5R5A1_UNORM` as render targets, blended at the format's
  precision; the uniform arena admitting uniform buffers whose other usages lie within the arena's
  own; `B8G8R8X8_UNORM`'s X byte kept as given and written 0 by clears and draws.
- WebGL 2 track: the 10_0 format abilities of the Gap list's WebGL 2 item; copies from a depth
  texture into a colour texture stored as a half float keeping every bit pattern; mips of whole
  3D views as Direct3D generates them.
- Interface and shaders tracks: a helper value giving a program a view's width other than its
  texture's (YUY2 views).
- Shaders track: blending into integer twins (`TargetInShader::combine`, built in its run 80 for
  single-sample targets; not yet used here); atg-simplecomputepc's division; the geometry replay's streams past
  the first; an aliased-line route for multisampled targets (with the interface track).
- NIR track: import `wcontainers_gpu::CUBE_FACES` in crates/wcontainers-nir/src/lib.rs in place of
  its own copy; until then the literal check reports that copy as a redefinition (its one finding
  outside the shaders and OpenGL crates' new code that touches this track).
- Rig track: the rig names the desktop's folder by the branch, which is `main` for every track now
  (this track runs its scratch copy, How to run).
- Rig track: a way to run Wine's dxgi file with its Direct3D 12 tests kept off the GPU; the scene
  programs reporting every `--dll` library they loaded.

## Literals

- Agents run 3 (2026-11-21): 66 lines of this track cut: the stage helper's refusals behind
  geometry, hull and domain shaders, its draw range (`Direct`) and word sizes
  (`wcontainers_gpu::STORAGE_WORD`), the patch list count, stream output declarations' buffer and
  stream counts (the shaders crate's `STREAM_BUFFERS` and `STREAMS`), the counting programs'
  names, sizes and refusals (the shaders crate's `ENTRY_POINT`), split draws' write mask and
  refusals, and the combination cache's key type (`blake3::OUT_LEN`). Left:
  wcontainers-direct3d-11 338 lines (404), wcontainers-directx 200. Checked on the rented x86
  machine (logs/a3-lit1 to a3-lit4): no new finding of this track's.
- Agents run 1 (2026-11-21): 36 lines of this track cut: the helper block takes each value's
  length from the words it writes (`size_of`), and colours, blend factors, border colours and
  scissor rectangles use the front's `Color`, `Rect` and `COLOR_COMPONENTS` in both crates. Left:
  wcontainers-direct3d-11 404 lines (430), wcontainers-directx 200 (210). The check at
  05a2cff4 exits 1 with 20 lines to act on, none of this track's (the OpenGL crate, the kernel
  interface's poll events, the NIR crate's cube faces, two socket suites).
- Main run 2 (2026-11-21): 19 lines of this track cut from the baseline, the library's messages of
  what it does not serve named once (`CLASS_LINKAGE`, `NO_COMPUTE`, `NO_TESSELLATION`,
  `LOGIC_OPERATIONS`, `FULLSCREEN` and others in device.rs and swapchain.rs, and the device
  makers' `NOT_HARDWARE`, `NOT_HARDWARE_10`, `NO_VIDEO`); the new video refusal adds no line.
  Left in the baseline: wcontainers-direct3d-11 432 lines (499 at main run 1; the rest went with
  the shared container reader, 7090247d), wcontainers-directx 210 (229).
- Main run 1 (2026-11-21): the check at main 10d3fd72 exits 1 with 50 lines to act on, none this
  track's but the cube faces' redefinition, which came from two branches meeting on main: the
  front now imports `wcontainers_gpu::CUBE_FACES` (Interface changes); the NIR crate's copy is
  that track's (Asked). The rest are new code of the shaders, OpenGL and interface crates.
- Round 4 run 6: the new refusals' messages are named once (`COMPARED_FORMAT_UNCLAIMED`,
  `COMPARED_VIEW_INVALID`, `LEVEL9_BUFFER_VIEWED`, `LEVEL9_USE_ABSENT`); new code adds no line and
  makes no mark. wcontainers-direct3d-11 499 lines, wcontainers-directx 229 (728), as at round 4
  run 5 (no line cut this run: the run went to the findings). The check exits 0 at 318f6a9e
  (logs/r6x-literals.log).
Left: the export names in build.rs (a build script cannot import the crate it builds), and the rest
of the fix list (orchestrator magic-literals/fix-graphics.md), a part per run.

## Licences

G4h run 1 added no dependency. Downloaded to scratch downloads/msdocs: Microsoft's 10_0, 10_1 and
11_0 format support pages (markdown from github.com/MicrosoftDocs/win32, CC-BY-4.0 text; the
table in required.rs is facts generated from them, not their text).

Dependencies of what this track owns, each read in its Cargo.toml when added: `windows` and
`windows-core` 0.62.2, `wgpu` and `naga` 30.0.2 (MIT OR Apache-2.0), `pollster` 1.0.1
(Apache-2.0/MIT), `blake3` 1.8.7 (CC0-1.0 OR Apache-2.0), `cc` 1.11.2 (build), `parking_lot`
0.12.5, `serde` 1, `rand` 0.9.5 and `rand_chacha` 0.9.0 (dev) (MIT OR Apache-2.0); through the
`webgl` feature our `wcontainers-gpu-webgl` with `glow` 0.18.0 (MIT OR Apache-2.0 OR Zlib),
`khronos-egl` 6.0.0 (MIT/Apache-2.0), `libloading` (ISC) and `bcdec_rs` 0.2.1 (MIT); our
`wcontainers-shaders` (which builds dxbc-spirv, MIT, in) and
`wcontainers-directx-graphics-infrastructure`. Test tools never shipped: Wine's tests (LGPL),
ANGLE (BSD-3-Clause), the system's d3dcompiler_47.dll. Installed on the desktop: build output in
C:\wc\target\phase-2-graphics-direct3d-11, pictures and logs in C:\wc\out, the 15 KB runner copy
in C:\wc\out\phase-2-graphics-direct3d-11\rigfix, an empty nvapi64.dll there (bgfx's examples),
the programs' cache files under C:\wc\out\phase-2-graphics-direct3d-11\gpu-cache, the test-only
d3d10_1.dll forwarder in C:\wc\out\phase-2-graphics-direct3d-11\d3d10_1fwd, and the LF worktree
C:\wc\src\phase-2-graphics-direct3d-11-lf for the literal check. On the Mac (not the desktop),
rustup's x86_64-pc-windows-msvc standard library for 1.99.0.
