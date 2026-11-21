# Direct3D 11 front status

Branch: main, in the one shared checkout (since 2026-11-21; the old phase-2-graphics-direct3d-11
branch is merged into it). Plan: docs/phase-2-graphics.md, milestone G4 (section 12).
This file keeps the current state only; each earlier run's record (what it did, its measurements,
its literal counts) is in this file's git history (`git log -p docs/status-graphics-direct3d-11.md`).
Runs so far: six on G4a, 25 on G4 (2026-11-17 and 18), round 2 runs 1 to 12 (2026-11-19 and 20),
round 3 run 1 and round 4 runs 2 to 6 (2026-11-20).

## Where things stand

G4 is reported complete for its sixth independent check. What is left on the pass bar waits on
other tracks (Gap list), or is the joint step with the Direct3D 12 track after both milestones.

Round 4, run 6 (2026-11-20, about 18:30 to 21:30 Pacific), the tenth check's findings:
- Merged origin/phase-2-graphics-shaders (its g6b run 4); no conflict. No other branch had news.
- Comparisons through colour views (blocking): `SampleCmp`, `SampleCmpLevelZero` and `GatherCmp`
  through views of `R32_FLOAT` and `R16_UNORM` colour textures, which Microsoft's 11_0 table
  requires and which removed the device (WebGPU compares only depth views), are served: a
  program's slot read as a depth texture with a colour view bound reads a depth texture of the same
  bits (`Depth32Float`, `Depth16Unorm`) in the view's shape, made at the first such draw and copied
  from the view's texels before each use (`View::compared`, `ReinterpretCopy`; into float depth the
  back end copies by a draw, `DepthWrittenByDraw`). A comparison through a colour view of any
  other format (which Direct3D claims for none) is refused by name. New listed case
  `compare_colour` (compute and pixel shaders, point and linear comparison filters, gathers with
  comparison, texels changed between uses), with 16-bit norm on and off: both match real Direct3D line by line (real Direct3D's results mix
  passes and failures, logs/r6c.log).
- Feature levels 9_1 to 9_3 (blocking): `CheckFormatSupport` reports only what the 10Level9
  tables give (`format::LEVEL9_SUPPORT`, the runtime's answers on the RTX 3080, which agree with
  Microsoft's 9_1 and 9_3 tables but for video formats; Microsoft has no 9_2 table), intersected
  with what ours serves. Creation follows the same table (`check_level9_texture`: dimension, each
  binding, mips, mip generation, samples, CPU access; multisampled textures are never read by
  shaders there) and refuses viewed and stream output buffers, all as `E_INVALIDARG`. The old
  three partial 9_x lists are gone. New tests compare every bit at 9_1, 9_2 and 9_3 and a list of
  creations line by line: all three levels match (logs/r6g.log). Sharing below 10_0 is served
  as the RTX 3080 serves it (measured, logs/r6e.log): `SHAREABLE` for the 17 formats it reports,
  and shared textures of those it makes (`format::level9_shared`; `R32_FLOAT` made though not
  reported, `R8G8B8A8_UNORM` refused though reported).
- Depth clears outside 0 to 1 (minor): `ClearDepthStencilView` drops the whole call for such a
  depth (NaN and the infinities among them), as the RTX 3080 does; `ClearView` of a depth view
  limits normalised depth to 0 to 1 (NaN as 0) and refuses by name float depth outside that range
  (`CLEAR_VIEW_FLOAT_DEPTH_OUTSIDE_RANGE`), which the RTX 3080 stores as given and WebGPU's depth
  clears and draws cannot write (Gap list). New listed case `depth_clears`: matches real Direct3D (logs/r6g.log).
- `GenerateMips` of an integer view (minor): neither side claims `MIP_AUTOGEN` there and Direct3D
  defines no result; the RTX 3080 fills the levels by a rule of its driver's (measured in the
  check's probe: 8-bit formats as their unorm twins' truncated box average, 32-bit components
  averaged as floats, 16-bit ones neither). Ours now warns once by name and leaves the levels
  (Differences from the RTX 3080); not copied from one driver's behaviour.
- The extended-range display (minor): the Gap list now says that it is a shortfall of 11_0 under
  both readings of Microsoft's pages (the overview marks it required at 11_0, optional at 10_x),
  and why 11_0 is still reported (for the owner).
- Building ahead (minor): unchanged, still no hit (What comes next 3).
- test_generate_mips' count corrected to 410 checks, as real Direct3D's; this file cut from 1,296
  lines to about 680 lines.
- The first format query now fails where the first word has no uses though the second has
  (sharing at 9_x; Wine's test_format_support found it at 9_1 to 9_3, 8 checks), and the format
  support tests compare the first query's result too.
- The bar rerun (Measurements): lint of both builds clean; library com 15 (1 ignored), programs 55
  (3 ignored); the WebGL 2 build's 10_0 cases 5 pass, 1 ignored; front 60, unit 5, cache 1; Wine
  d3d11 151 of 166 and d3d10core 110 of 111, the same tests as before; scenes 135 pass; the bar's
  three programs match; building ahead unchanged; sizes and load times measured.

## Wine's Direct3D 11 tests through ours

Files passing (the bar: real Direct3D's verdict for every test, except the gap list), on the RTX
3080 through the queue, not alone, rigfix/wine.sh, round 4 run 6 at the code of the run's last commit (logs/r6y-wine*.log):
- d3d11: 151 pass of 166, the same 15 failing as in round 4 runs 3 to 5; the failing tests are in the Gap list. test_format_support's 7
  checks are the B5 targets (6) and XR_BIAS's display (1).
- d3d10core: 110 of 111: test_desktop_window (Gap list).
- d3d10core through the WebGL 2 build (10_0): 98 of 111 at round 4 run 5 (logs/r5z-wine10gl.log),
  the same 13 failing as at round 2 run 12; they are under the WebGL 2 item of the Gap list.
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
  for packed formats) does. The WebGPU track's, with the shaders track (Asked). Wine d3d11 and
  d3d10core test_format_support ("Format 0x55 - blendable not supported", 6 checks). The 10Level9
  tables ask the same of `B5G6R5_UNORM` at 9_1 to 9_3.
- `R10G10B10_XR_BIAS_A2_UNORM`'s display and back buffer casts: everything else of its 11_0 row
  is served; a swap chain of the format, and presenting it with the extended-range conversion,
  are the infrastructure crate's (`wcontainers-directx-graphics-infrastructure`, swapchain.rs,
  built by the Direct3D 12 track, G5; Asked). test_format_support ("Format 0x59 - display not
  supported", 1 check), test_swapchain_formats (1 check).
- `A8_UNORM` storage views (typed loads and stores): the front holds A8 as `rgba8unorm` with its
  value in alpha, and the shaders crate has no storage form of `A8Unorm`; refused by name
  (`E_NOTIMPL`) and not claimed. The shaders track's (Asked). No Wine test reaches it.
- `B8G8R8X8_UNORM` vertex elements and typed buffers (typed buffer views of it are refused by
  name, as they would read the X byte): the RTX 3080 reads blue, green and red with 1 in the
  fourth component (`b8g8r8x8_elements_read_as_direct3d_reads_them`, ignored). No `VertexFormat`
  reads so, and a new variant must land with the arms of the WebGPU back end, the WebGL 2 back end
  and the shaders crate in one step (exhaustive matches in three crates this track may not
  touch). Asked of the interface track with those three; the Direct3D 12 track asked the same.
  Wine test_vertex_formats runs 98 checks of real Direct3D's 100 and fails none.
- Blending into the 16-bit normalised formats where the back end lacks them (a browser without
  texture-formats-tier1; natively `WEBGPU_OFF=wgpu-texture-format-16-bit-norm`): they are then held
  as their integer twins (`NormAsInteger`), which no pass blends; blending into one is refused by
  name (`BLEND_INTO_INTEGER_TWIN`) and `CheckFormatSupport` withdraws `BLENDABLE` and the
  multisample bits from the six such formats. Plan section 9 requires them blended at every
  level, by `BlendInShader`: the shaders crate's `TargetInShader::combine`, refused there
  (`BLEND_IN_SHADER`). The shaders track's (Asked). With the features present (the RTX 3080
  natively, browsers with tier1) nothing is withdrawn.

Why 11_0 is still reported (for the owner). Plan section 9 reports the highest level served, and
these shortfalls break that rule; `CheckFormatSupport` reports each of them absent, which is the
answer programs ask for a format's abilities. Microsoft's pages disagree on the B5 targets: the
per-format tables require them at 11_0, 10_x and (`B5G6R5_UNORM`) 9_x; the feature-level
overview (overviews-direct3d-11-devices-downlevel-intro) marks '16 bits per pixel (bpp) formats'
optional below 11_1. So no lower level removes them under the strict reading. The extended-range
display is different: the overview marks '10-bit XR High Color Format' required at 11_0 and
optional at 10_1 and 10_0, so under either reading the truthful level without it is 10_1. 10_1 is
not reported because it takes away shader model 5 (compute, tessellation, storage views), which
the bar's programs and most of Wine's d3d11 file use, for one swap-chain format no measured
program asks for. That is a departure from section 9 left to the owner.

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
  `E_NOINTERFACE`): test_immediate_context, test_video_support, test_h264_decoder.
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
not give real Direct3D's verdict on, measured at round 2 run 12 (Wine's d3d10core 98 of 111,
rerun at round 4 run 5 with the integer twins with the same 13; the rig's scenes 95 pass, 17 fail,
30 absent), each with its owner:
- A sample mask leaving out samples (WebGL 2 has no `gl_SampleMask`; the shaders crate's
  `SAMPLE_MASK` cannot serve it): refused by name. Wine test_sample_mask,
  test_multisample_resolve; the rig's msaa scene. No route exists; plan section 9 fixes 10_0 on
  WebGL 2 without naming this loss.
- Multisampled textures read by shaders (absent on WebGL 2, plan section 6):
  test_unbound_multisample_texture.
- A depth bias clamp on float depth, refused by name (Direct3D's float step follows each
  primitive's largest depth exponent, which a pixel program does not see): test_depth_bias.
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
atg-simplelightingpc) match real Direct3D on every compared frame (logs/r6z-programs.log, round 4 run 6), and so do
atg-simplebezierpc, atg-simpleinstancingpc, godot-platformer-2d-d3d11 and sdk-tutorial02 and 04
to 07; of bgfx's six (logs/q2f-bgfx.log, round 4 run 2) five pass or run to their end. Outside
the bar's three, three differ:
- bgfx-26-occlusion: 6,458 pixels beyond the tolerance in each saved frame, in the overlay's text
  (the passing pixel count it prints from its occlusion queries). Real Direct3D 11's own two runs
  passed its references (169 pixels beyond, within the allowance, logs/q3j-sysruns.log), so
  run-to-run variation does not explain ours; the cause is not found.
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
  array textures of the R32G32B32 formats and `B4G4R4A4_UNORM`; 1D, 3D, array and cube-array
  textures of `R1_UNORM` and of the video formats ours holds no texture of; shared textures of
  P016 and NV11; typed buffer views of `R1_UNORM`, the extended-range format and the video
  formats. 22 formats differ so in the eighth check's probe (probe8/out). Needs Microsoft's
  per-level table at 10_0 and up in the front; not built.
- d3d11 test_clear_buffer_unordered_access_view passes through ours, where real Direct3D fails 223
  of its 934 checks: ours clears by the documented rule; NVIDIA's driver does not.
- `IDXGISwapChain::GetContainingOutput` answers `E_NOTIMPL` (the infrastructure crate's).
- `B8G8R8X8_UNORM` and its sRGB twin: the X byte reads 0xff after a clear, a draw and initial
  data, where the RTX 3080 writes 0x00 for clears and draws and keeps the bytes it was given (the
  WebGPU back end's `bgra8unorm`, src/packed.rs; Asked).
- Values just past a half between two codes drawn into a 16-bit normalised texture held as its
  integer twin: 6553.5006 is written 6553 by the RTX 3080 and 6554 by ours; both within the
  specification's tolerance. Filtered samples and mips of twins differ by 1 ULP at most.
- `ID3D11Device3::ReadFromSubresource` and `WriteToSubresource` are not served: they warn and
  remove the device. No valid call reaches them (`D3D11_OPTIONS2.MapOnDefaultTextures` FALSE).
- A swap chain on another process's window is refused with `DXGI_ERROR_INVALID_CALL` (Departures).
- Drawing a geometry stream past 0 is refused by name (`GEOMETRY_STREAM_DRAWN_PAST_ZERO`).
- Building ahead at a shader's arrival (plan 4.4) has never been hit: 122 of Godot 3D's 124 pairs
  built ahead are pairs it never draws (a new shader is paired with the latest shader of the other
  stage), and its 2 drawn pairs draw with other state than the single most drawn tuple. The cost is
  about 110 to 124 background pipeline builds per Godot 3D run that are never used.

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
- Optional video formats absent altogether: AYUV, Y410, Y416, P010, P016, Y210, Y216, NV11, AI44,
  IA44, P8, A8P8.
- Shortfalls of the reported level, in the Gap list: the B5 targets (at every level),
  `R10G10B10_XR_BIAS_A2_UNORM`'s display and back buffer casts and `B8G8R8X8_UNORM` elements and
  typed buffers (10_0 and up), `A8_UNORM` storage views (11_0).
- Below 10_0, as the RTX 3080 answers: only the 10Level9 table's bits; no buffers, comparisons,
  gathers or storage; `SHAREABLE` for the 17 formats the RTX 3080 reports it for, and shared
  textures made of them but `R8G8B8A8_UNORM` (refused there, measured) and its sRGB twin
  (inferred, not measured).
- Below 11_0: sharing is claimed from 10_0 and the second query answers it alone; gathers at 10_1
  only for formats of a red channel alone, uncompressed, one aspect of depth, NV12 and YUY2;
  multisampled depth is not read at 10_0.
- Every creation ours claims succeeds and every one it serves is claimed, but those where the RTX
  3080 itself claims and refuses alike (cubes of the depth formats, NV12 and YUY2 views without a
  description; the seventh check's probe). `CheckMultisampleQualityLevels` matches the RTX 3080
  for every value 0 to 200 at counts 1 to 33 but the B5 formats' counts 2 to 8 and the absent
  video formats' single sample.

## Known failures

- `aliased_lines_into_a_multisampled_target_hit_every_sample_as_direct3d_does`,
  `b8g8r8x8_elements_read_as_direct3d_reads_them` and `yuy2_view_sizes_read_as_direct3d_reads_them`
  (programs.rs) and `constants_rewritten_between_draws_make_no_bind_group_per_draw_on_webgpu`
  (com.rs) fail on every build and are marked `#[ignore]` with the reason (Gap list); run them
  with `--include-ignored`.
- On the WebGL 2 build only: the library tests that ask for 11_0 or 10_1 fail, as the build
  reports 10_0, or reach the absences of the Gap list's WebGL 2 item. The bar's WebGL 2 run takes
  the 10_0 cases alone; of those, `direct3d_10s_device_draws_and_keeps_its_state_as_direct3d_does`
  (an 11_0 device with a 10_1 state) is marked ignored on that build with that reason.

## What comes next

In order:
1. The B5 targets, XR_BIAS's swap chain, the B8G8R8X8 vertex format, A8 storage views and the
   YUY2 view's size, once the WebGPU, infrastructure, interface and shaders tracks give their
   parts (Asked); then remove each from `support_differences` and un-ignore their tests.
2. Constant buffers rewritten per draw: once the WebGPU back end's arena admits uniform buffers
   with other usages (Asked), un-ignore the counters test and run it.
3. Building ahead at a shader's arrival: pair shaders by what programs draw together rather than
   by creation order (for example only from the vertex shader's arrival, or from pairs recorded in
   the cache), and pick tuples by the pixel shader's targets; measure again.
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
graphics-direct3d-11); logs go to its logs/. Since every track is on main, the rig's own commands
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
- Front tests (recording back end, no GPU): `orchestrator/cloud/x86-run <checkout> --name g-d3d11
  --jobs 32 -- "cargo test -q -p wcontainers-direct3d-11"`.
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

At round 4 run 6: front.rs 60, tests/cache.rs 1, unit 5 (wcontainers-direct3d-11, recording back
end); com.rs 16 (1 ignored) and programs.rs 55 (3 ignored) (wcontainers-directx, real
devices). New in round 4 run 6: programs.rs `comparisons_through_colour_views_as_direct3d_does`
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
- Device children are made by our `counted!` macro, not `implement`, for Direct3D's two counts;
  windows-core is pinned to exactly 0.62.2 for its object layout.
- The recording back end is in the front crate (`record.rs`), not G1's wcontainers-gpu-test.
- State objects are kept by the device for its life once made.
- `CheckFeatureSupport` answers `D3D11_FEATURE_THREADING` with both FALSE.
- The feature level is 11_0 at most.
- The adapter reports zero for its memory sizes (the infrastructure crate's rule).
- `D3D11CreateDevice` with an adapter uses ours whatever is passed; the debug flag answers
  `DXGI_ERROR_SDK_COMPONENT_MISSING`; software, WARP, reference and null drivers
  `DXGI_ERROR_UNSUPPORTED`.
- `D3D11_OPTIONS.ClearView` is TRUE: target, depth and storage views, whole or by rectangles;
  storage rectangles through the shaders crate's storage clear program; 3D target rectangles copied
  into each slice from a cleared slice.
- The library draws through wgpu on Direct3D 12 only.

## Interface changes

- 978d1661 "interface: named the slow path of a resource shared between devices copied through the
  host": `SlowPath::SharedThroughHost`, and one sentence in plan section 9.
- 8b556a05 "interface: named each normalised format's integer twin as Format::integer_twin and read
  generate_mips' twins from it".
- c8a7be9 "interface: copied combined depth-stencil textures whole in copy_texture".
- 7f73347 "interface: named the slow path of a constant buffer grown to its shader's declared size,
  and said a depth attachment holds both aspects".

## Measurements

Round 4 run 6. Machine: the desktop (Windows 10, RTX 3080, i9-9900K), through the queue, not
alone except the load times (other tracks' jobs sharing it; the NIR track's OpenGL CTS runs,
display-reset hazard jobs, held the queue for about 35 minutes before several steps). Times
Pacific, 2026-11-20. The bar (scratch q5-final.sh r6z, logs/r6z-*) ran at 6b3ea847's code but the
first query's failure on an empty first word (committed after it, then rerun: Wine, scenes, the
format support tests, lint; logs/r6y-*).
- Library (r6z-lib, 20:05 to 20:09): com 15 pass, 1 ignored; programs 55 pass, 3 ignored. The
  WebGL 2 build's 10_0 cases: 5 pass, 1 ignored. Front 60, unit 5, cache 1 on the rented x86
  machine (96 processors, load about 8 to 25 from other jobs).
- Scenes through the library: 135 pass, 0 fail, 4 absent (r6y-scenes, 21:15; the bar's own
  scenes step lost its connection, exit 255). The WebGL 2 build's scenes 95 pass, 17 fail, 30 absent
  (r6z-webgl), as before.
- Programs (r6z-programs, 20:54 to 20:56): the bar's three (godot-platformer-3d-d3d11,
  chocolate-doom, atg-simplelightingpc) match real Direct3D on every compared frame, and so do
  atg-simplebezierpc, atg-simpleinstancingpc, godot-platformer-2d-d3d11 and sdk-tutorial02 and 04
  to 07; atg-simplecomputepc and atg-simplemsaa_pc differ as before (Gap list). bgfx's were not
  run this run.
- Module sizes (section 10), release builds 20:56 to 20:58 (r6z-sizes): `wcontainers_directx.dll`
  14,304,256 bytes on WebGPU and 9,881,600 bytes on WebGL 2; the forwarders 1,024 bytes each;
  `wcontainers_d3d12.dll` (the Direct3D 12 track's code as merged here)
  13,733,376 bytes.
- Load time (section 10), alone, `LoadLibraryW` in 20 fresh processes each, medians (r6z-load,
  20:58 to 21:02): the system's d3d11.dll 1,147 us; ours on WebGPU 1,637 us; ours on WebGL 2
  1,110 us; `wcontainers_d3d12.dll` 1,643 us.
- Building ahead (plan 4.4; run-warm.sh, five programs run twice from one cache directory, 21:02
  to 21:06, r6z-warm): Godot 3D's cold run built 124 ahead and drew none, 66 at their first draw;
  its warm start built its 61 recorded combinations at start and drew all 61, built 124 ahead and
  drew none, and 26 at their first draw. Godot 2D's warm start built 3 at start and drew 3.
  Unchanged: building ahead has never hit.
- Real Direct3D's own outputs of the new cases (logs/r6c.log, r6e.log): comparisons through
  colour views give a mix of 0 and 1 by texel; at 9_x the RTX 3080 makes shared textures of
  `B8G8R8A8_UNORM`, `BC1_UNORM`, `R16G16B16A16_FLOAT` and (from 9_2) `R32_FLOAT`, and refuses
  `R8G8B8A8_UNORM` ones whatever the bindings.

Earlier runs' measurements are in this file's git history.

## Known issues

- Threads: one lock serialises all calls; no parallelism between a program's threads.
- An occlusion set running out of slots inside a pass ends the pass and opens the next on a fresh
  set; that split is not counted as a slow path.
- No front (recording back end) tests for 1D and 3D textures or queries; they are checked against
  real Direct3D instead (`own_volumes`, `own_queries`, the rig's textures scene).
- The rig's replacement check cannot see wcontainers_directx.dll behind the forwarder.

## Asked of other tracks

- Interface track, with the WebGPU, WebGL 2 and shaders tracks in the same step: a `VertexFormat`
  for `B8G8R8X8_UNORM` reading blue, green and red with 1 in the fourth component; a `Caps` feature
  for cube arrays, declared by the WebGPU back end at the same time.
- Infrastructure crate (the Direct3D 12 track, G5): swap chains of `R10G10B10_XR_BIAS_A2_UNORM`,
  presented with the extended-range conversion, and back buffer views of it as
  `R10G10B10A2_UNORM`; `D3D11_FORMAT_SUPPORT2_SHAREABLE` in its `support2` list.
- WebGPU track: `B5G6R5_UNORM` and `B5G5R5A1_UNORM` as render targets, blended at the format's
  precision; the uniform arena admitting uniform buffers whose other usages lie within the arena's
  own; `B8G8R8X8_UNORM`'s X byte kept as given and written 0 by clears and draws.
- WebGL 2 track: the 10_0 format abilities of the Gap list's WebGL 2 item; copies from a depth
  texture into a colour texture stored as a half float keeping every bit pattern; mips of whole
  3D views as Direct3D generates them.
- Interface and shaders tracks: a helper value giving a program a view's width other than its
  texture's (YUY2 views).
- Shaders track: a storage form of `A8Unorm`; blending into integer twins
  (`TargetInShader::combine`); atg-simplecomputepc's division; the geometry replay's streams past
  the first; an aliased-line route for multisampled targets (with the interface track).
- Rig track: a way to run Wine's dxgi file with its Direct3D 12 tests kept off the GPU; the scene
  programs reporting every `--dll` library they loaded.

## Literals

- Round 4 run 6: the new refusals' messages are named once (`COMPARED_FORMAT_UNCLAIMED`,
  `COMPARED_VIEW_INVALID`, `LEVEL9_BUFFER_VIEWED`, `LEVEL9_USE_ABSENT`); new code adds no line and
  makes no mark. wcontainers-direct3d-11 499 lines, wcontainers-directx 229 (728), as at round 4
  run 5 (no line cut this run: the run went to the findings). The check exits 0 at 318f6a9e
  (logs/r6x-literals.log).
Left: the export names in build.rs (a build script cannot import the crate it builds), and the rest
of the fix list (orchestrator magic-literals/fix-graphics.md), a part per run.

## Licences

Round 4 run 6 added no dependency. Downloaded to scratch (downloads/msdocs92) nothing: Microsoft
has no 9_2 format table.

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
