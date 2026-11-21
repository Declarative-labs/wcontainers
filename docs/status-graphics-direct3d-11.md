# Direct3D 11 front status

Branch: phase-2-graphics-direct3d-11. Plan: docs/phase-2-graphics.md, milestone G4 (section 12).
Runs so far: six on G4a; runs 1 to 25 of G4 (2026-11-17 and 18); round 2, runs 1 to 12 (2026-11-19
and 20); round 3 run 1 and round 4 runs 2 to 4 (2026-11-20; the orchestrator counts all four as
round 4); round 4 run 5 (2026-11-20, the ninth check's findings). This file keeps the current state only; each earlier run's record is in its git history
(`git log -p docs/status-graphics-direct3d-11.md`).

## Where things stand

G4 is reported complete for its next independent check, with the items below that wait on other
tracks (round 4's task: do not spend further runs waiting on them). Nothing of the 11_0 format
table is this front's own any more; what is left waits on other tracks (Gap list).

Round 4, run 5 (2026-11-20, about 15:00 to 17:00 Pacific), the ninth check's findings:
- Merged origin/phase-2-graphics-shaders (its g6b runs 2 and 3); no conflict. No other track's
  branch had news.
- Format support below 11_0 (blocking): measured first on the RTX 3080 at 10_1 and 10_0 (the
  ninth check's probe9 and `own_cases format_support <level>`, scratch m5/out). Sharing is
  claimed from 10_0, where ours serves it, and the second query answers it; gathers at 10_1 only
  for formats of a red channel alone, uncompressed, and the depth views of one aspect (NV12 and
  YUY2 keep theirs); multisampled depth is not read at 10_0 (no `MULTISAMPLE_LOAD` on the depth
  views; four-sample depth textures made for shaders are refused there, as on the RTX 3080).
  New tests compare every bit at 10_1 and 10_0 as at 11_0 (Format support against the RTX
  3080); all four pass. The front's own format test asked for the old answers and is corrected.
- Integer target clears (minor): converted as the RTX 3080 converts them, measured first with
  values past 32 and 64 bits and the infinities (Differences from the RTX 3080); storage views
  keep the clamp, as the RTX 3080's storage rectangles clamp. The listed case `clear_view` now
  covers five integer formats with three such sets, whole and by rectangles.
- `ClearView` with a rectangle whose right edge is `i32::MIN` no longer overflows (it clears
  nothing, as on the RTX 3080); in the listed case.
- `ClearView` of a depth view clears its depth alone to the first value, whole or by rectangles
  (measured: the RTX 3080 keeps the stencil); in the listed case. The warning for other views
  no longer calls every one a video view.
- The B5 targets: the Gap list now states the conflict between Microsoft's per-format tables and
  its feature-level overview instead of one reading.
- 16-bit normalised twins: the ninth check's measurement (1 code off just past a half, within
  tolerance) is recorded under Differences; item 3 of the task was already done.
- `BLEND_INTO_INTEGER_TWIN` has its doc comment back; `RGB10A2_BITS` its own.
- `direct3d_10s_device_draws_and_keeps_its_state_as_direct3d_does` is marked ignored on the
  WebGL 2 build with its reason (it needs an 11_0 device and a 10_1 state).
- WebGL 2 item: test_generate_mips is named the WebGL 2 back end's (its `glGenerateMipmap`
  route for 3D). Wine's d3d10core through the WebGL 2 build rerun with the integer twins: 98 of
  111, the same 13 tests as at round 2 run 12 (logs/r5z-wine10gl.log). The device lost at
  `R16G16B16A16_FLOAT`'s staging read back is still not traced (no WebGL 2 probe run this run).
- Not done: building ahead still gives no hits (What comes next 3; this run went to the
  findings above), and the remaining creation codes.
- Literals: the sampler's filter bits, address modes and largest anisotropy named once in
  `desc`; wcontainers-direct3d-11's baseline 512 to 499 lines; the check exits 0 at d9c373b9
  (logs/r5x-literals.log).
- The bar rerun (Measurements): lint of both builds clean; library com 15 (1 ignored), programs
  52 (3 ignored); the WebGL 2 build's 10_0 cases 5 pass, 1 ignored; front 59, unit 5, cache 1;
  Wine d3d11 151 of 166 and d3d10core 110 of 111, the same tests as before; scenes 135 pass;
  the bar's three programs match.

Round 4, run 4 (2026-11-20, about 12:50 to 14:40 Pacific), the seventh check's findings:
- Merged origin/phase-2-graphics-shaders (its g6b run 1: normalised codes encoded with halves
  toward zero, DXIL geometry shaders); no conflict. No other branch had news.
- `ClearView` (blocking): rectangles of storage views are served through the shaders crate's
  storage clear program (`builtin::storage_clear`, `clear_params`, `clear_groups`), one dispatch
  per rectangle over every layer or slice the view sees; whole storage views of textures no pass
  draws into take it too (refused before). Rectangles of 3D target views are copied into each
  slice from a texture of one slice cleared whole (`ReinterpretCopy`), as the interface's
  `clear_rects` draws into one slice of a view. Integer clear values are clamped to the channel
  (the specification, 5.2.3.3; the RTX 3080's targets clamp so); a decoded twin's storage
  `ClearView` took its floats as integers before and now encodes them. Target rectangles on a
  view made on a twin copy the twin in first, so the copy out keeps what lies outside them.
  `D3D11_OPTIONS.ClearView` answers TRUE. The listed case (`clear_view`, measured first on the
  RTX 3080) now covers rectangles of storage views of float, unorm, snorm, 8-, 16- and 32-bit
  integer formats and a two-layer array, targets of an integer format, whole views of a buffer
  and of 1D and 3D textures, and 1D and 3D rectangles; it and a new run without
  texture-formats-tier1 (where `R8G8B8A8_SNORM` is stored but not drawn into) match real
  Direct3D line by line.
- Typed storage buffers of formats Direct3D does not store (the R32G32B32 formats among them,
  which ours made though unclaimed) and typed buffer views of typeless formats answer
  `E_INVALIDARG` as on the RTX 3080; typed storage buffers are served exactly where
  `CheckFormatSupport` claims typed storage views. The eighth check's probe (scratch probe8):
  no creation the RTX 3080 refuses succeeds (the formats whose success differs fell from 16 to
  13: the A8 storage, B5 target and absent video formats' sharing shortfalls); the formats with
  code-only differences fell from 48 to 22 (Differences from the RTX 3080 lists what is left).
- Status corrections: the 9_x tables require `B5G6R5_UNORM` alone as a target (not
  `B5G5R5A1_UNORM`); the integer twins' missing blending is now in the Gap list's failures of
  the reported 11_0; the release module size is measured at this run's code (Measurements).
- Not done: building ahead's pairing (0 hits; What comes next 3); the remaining creation codes.
- The bar rerun at 8ec7532e (Measurements): lint of both builds clean; library com 15 (1
  ignored), programs 49 (3 ignored); front 59, unit 5, cache 1; Wine d3d11 151 of 166 and
  d3d10core 110 of 111, the same tests as before; scenes 135 pass; the three bar programs match.
- Literal baseline: wcontainers-direct3d-11 527 to 512, wcontainers-directx 229; the check
  exits 0 at 8ec7532e.


Round 4, run 3 (2026-11-20, about 09:45 to 11:45 Pacific), the sixth check's findings:
- Merged origin/phase-2-graphics-shaders (its g6a run 5); no conflict. No other branch had news.
- Sharing (blocking 1): shared resources are served. `MISC_SHARED`, `MISC_SHARED_KEYEDMUTEX` and
  `MISC_SHARED_NTHANDLE` are taken on buffers and textures; `GetSharedHandle` answers a KMT handle,
  `CreateSharedHandle` an NT handle (a real kernel object, an event, Departures);
  `OpenSharedResource`, `OpenSharedResource1`, `OpenSharedResourceByName` and Direct3D 10's
  `OpenSharedResource` open them on any device of the process; `IDXGIKeyedMutex` is served on
  keyed-mutex resources (keys, waits with time-outs, abandonment when the holder's device goes).
  Each device holds its own copy (no two back ends share memory); a copy written on one device
  reaches the others through the host at that device's `Flush` and at `ReleaseSync`, a new slow
  path (`SharedThroughHost`, an interface change). `CheckFormatSupport`'s second word claims
  `SHAREABLE` for exactly the formats the RTX 3080 claims it for (the 11_0 table's Shared Resource
  row). New example own_shared (two devices: a KMT texture written on each and read on the other,
  a keyed-mutex texture handed between them by key, a keyed-mutex buffer, an NT-handle texture),
  compared line by line with real Direct3D: identical.
- This front's own 11_0 shortfalls (blocking 2), all built: NV12's and YUY2's storage views (each
  plane's format, natively or `StorageFormatThroughBuffer`), gathers, and NV12's blending; 420_OPAQUE
  textures (held as NV12's two planes, made bound to nothing and of default usage as the RTX 3080
  takes them, copied and shared); render target views of a swap chain's buffer in the other formats
  of its group (`R10G10B10A2_UNORM` as `_UINT`, `R8G8B8A8_UNORM` as `_SRGB`, `_UINT`, `_SNORM`,
  `_SINT`, and so on), measured first on the RTX 3080 (its shader resource views of other formats
  are refused, as ours). Swap chains with `DXGI_USAGE_SHADER_INPUT` now give their buffers sampled
  usage (a shader resource view of one removed the device before). The own_nv12, own_yuy2 and
  own_swap_chain examples cover each, compared line by line with real Direct3D: identical but the
  YUY2 view's `GetDimensions` (Gap list). Format support differs from the RTX 3080 now only by the
  absent features and the other tracks' items.
- The pass bar rerun (Measurements), for the check that could not finish: library suite, Wine's
  d3d11 151 of 166 (was 146) and d3d10core 110 of 111, scenes, the bar's three programs, and the
  seventh check's own probe beside the system's library and ours. The probe found three things,
  fixed here: `A8_UNORM` storage views lost the device (now refused by name and not claimed; the
  shaders crate has no storage form of A8, Asked); typed buffer views of `B8G8R8X8_UNORM` were
  served (reading the X byte) though not claimed (typed buffer views are now served exactly where
  `CheckFormatSupport` claims them); `CheckMultisampleQualityLevels` accepted 189 to 191, which
  the RTX 3080 refuses, and answered 0 for one sample of `UNKNOWN`, where it answers 1. After the
  fixes every creation ours claims succeeds and every one it serves is claimed, but where the RTX
  3080 itself claims and refuses alike.
- Minor findings: the constant-buffer listed case still waits on the WebGPU back end (Gap list);
  the B5 targets, the extended-range display and `B8G8R8X8_UNORM` elements still wait on the
  WebGPU, infrastructure, interface and shaders tracks; building ahead's pairing is not redesigned
  this run (What comes next 3; this run went to the blocking findings);
  `CheckMultisampleQualityLevels` of 189 to 191 is fixed (above); the two real programs outside
  the bar still differ, and real Direct3D's own runs pass their references (Gap list).
- Literal baseline: wcontainers-directx 239 to 229 lines (the open calls' names, the resource
  interfaces' places named once); wcontainers-direct3d-11 527; the check exits 0 (Literals).
- The fifth check's chain (scratch check-r11) kept running on this checkout while this run worked,
  and some of its steps sent this run's unfinished tree (its Wine step built a tree that did not
  compile). Its logs after 09:46 are not evidence of anything.

- Section 10's family library: by the orchestrator's ruling of 2026-11-20 it is the joint step
  after G4 and G5 pass, not part of this bar; listed as waiting, with both modules' sizes
  measured today (Measurements).

Round 4 run 2 and round 3 run 1 (2026-11-20; their full record is in git): the extended-range
format's textures, the creation refusals, clear rounding, integer twins of 16-bit normalised
formats (interface: `Format::integer_twin`), the format support comparison bit by bit.

## Wine's Direct3D 11 tests through ours

Files passing (the bar: real Direct3D's verdict for every test, except the gap list), round 4
run 5, on the RTX 3080 through the queue, not alone, rigfix/wine.sh, at dbf1d5cc's code:
- d3d11: 151 pass of 166 (logs/r5z-wine.log), the same 15 failing as in round 4 runs 3 and 4;
  they are in the Gap list. Round 4 run 3's sharing took test_keyed_mutex,
  test_create_buffer and the three texture interface tests off the list; test_shared_resource now
  differs only by the 11_1 device it skips. test_format_support's 7 checks are the B5 targets (6)
  and XR_BIAS's display (1).
- d3d10core: 110 of 111 (logs/r5z-wine10.log): test_desktop_window (Gap list).
- d3d10core through the WebGL 2 build (10_0): 98 of 111 at round 4 run 5
  (logs/r5z-wine10gl.log), the same 13 failing as at round 2 run 12; they are under the WebGL 2
  item of the Gap list.
- dxgi (not on G4's bar): 4 of the 17 tests without a Direct3D 12 part at round 2 run 8.

## Gap list (the pass bar's written list, round 4 run 3)

Everything on G4's pass bar that does not give real Direct3D's verdict on the RTX 3080, each item
once, with its cause and whose work it is. A gap stands behind a named absent feature; an ability
Microsoft's 11_0 table requires is not an absence, so those shortfalls are listed as failures of
the reported level, with the track whose part they wait on. Nothing is excluded for convenience.
The rig's exclusion of d3d11 test_create_device applies to real Direct3D's run too: its child
process makes a swap chain with `Windowed = FALSE`, which would take the owner's screen
(tools/graphics-rig/wine-excluded.txt).

Waiting on the joint step after G4 and G5 (orchestrator's ruling, 2026-11-20):
- Section 10's one family library (one DXGI shell, `D3D12CreateDevice` exported, a Direct3D 11
  program loading Direct3D 12's code). Today there are two modules: ours,
  `wcontainers_directx.dll` (Direct3D 10 and 11 and a DXGI), 14,275,072 bytes on WebGPU and
  9,858,560 bytes on WebGL 2; and the Direct3D 12 track's `wcontainers_d3d12.dll` (its own DXGI),
  13,732,352 bytes (release builds of dbf1d5cc's code, round 4 run 5, Measurements). Nothing else of section 10 is missing: the forwarders
  (d3d11.dll, dxgi.dll, d3d10core.dll), the exports of Direct3D 10 and 11, and the WebGL 2 build
  of the same source are built and tested.

Failures of the reported 11_0 against Microsoft's format table, waiting on other tracks (each
compared in the format support tests and in Wine's test_format_support; each needs code in a
crate this track does not own before this front can serve it):
- `B5G6R5_UNORM` and `B5G5R5A1_UNORM` as render targets, blended (and so mip generation): the
  WebGPU back end holds them as `rgba8unorm` for sampling only and declares no render use;
  drawing into them exactly needs the output quantised to the format's bits and blending at that
  precision, which neither the back end nor the shaders crate's `targets_in_shader` (no layout
  for packed formats) does. The WebGPU track's, with the shaders track (Asked). Wine d3d11 and
  d3d10core test_format_support ("Format 0x55 - blendable not supported", 6 checks).
- `R10G10B10_XR_BIAS_A2_UNORM`'s display and back buffer casts: everything else of its 11_0 row
  is served since round 4 run 2 (above); a swap chain of the format, and presenting it with the
  extended-range conversion, are the infrastructure crate's (`wcontainers-directx-graphics-
  infrastructure`, built by the Direct3D 12 track, G5; Asked). test_format_support ("Format
  0x59 - display not supported", 1 check), test_swapchain_formats (1 check).
- `A8_UNORM` storage views (typed loads and stores): the front holds A8 as `rgba8unorm` with its
  value in alpha, and the shaders crate has no storage form of `A8Unorm` (its storage formats are
  WGSL's), so a storage view of it would lose the device (the seventh check's probe showed it).
  Since round 4 run 3 they are refused by name (`E_NOTIMPL`) and not claimed. The shaders
  track's (Asked). No Wine test reaches it.
- `B8G8R8X8_UNORM` vertex elements and typed buffers (typed buffer views of it are refused by name
  since round 4 run 3, as they read the X byte): the RTX 3080 reads blue, green and red with
  1 in the fourth component (measured, `b8g8r8x8_elements_read_as_direct3d_reads_them`, ignored).
  No `VertexFormat` reads so (`Bgra8Unorm` gives the X byte). Section 12 lets this track make
  the interface change, but a new `VertexFormat` variant does not build alone: the WebGPU back
  end (convert.rs, its vertex format match), the WebGL 2 back end (state.rs, its attribute
  match) and the shaders crate (variant/pull.rs and variant/texel.rs, its fetches) match the
  enum exhaustively, so the variant must land with their arms in the same step, in three crates
  this track may not touch, and this library depends on all of them. Asked of the interface
  track with those three; the Direct3D 12 track asked the same in its run 51. Wine
  test_vertex_formats runs 98 checks of real Direct3D's 100 and fails none.
- Blending into the 16-bit normalised formats where the back end lacks them (a browser without
  texture-formats-tier1; natively `WEBGPU_OFF` with the 16-bit-norm feature): they are then held
  as their integer twins (`NormAsInteger`), which no pass blends; blending into one is refused by
  name (`BLEND_INTO_INTEGER_TWIN`) and `CheckFormatSupport` withdraws `BLENDABLE` (and the
  multisample bits, which plan 4.12 names) from `R16G16B16A16_UNORM`/`_SNORM`, `R16G16_UNORM`/
  `_SNORM` and `R16_UNORM`/`_SNORM`. Plan section 9 requires them blended at every level, by
  `BlendInShader` (plan 4.12): the shaders crate's `TargetInShader::combine`, refused by name
  there (`BLEND_IN_SHADER`, variant/targets.rs, still at this run's merge). The shaders track's
  (Asked). With the features present (the RTX 3080 natively, browsers with tier1) nothing is
  withdrawn. Compared in `format_support_without_16_bit_norm_as_direct3d_reports_it_but_the_named_shortfalls`.
Why 11_0 is still reported. Microsoft's sources disagree on the B5 targets. The per-format
tables (DXGI_FORMAT support for 11_0 hardware) and the 10Level9 table (archived ff471324:
`B5G6R5_UNORM` a render target and blendable at '9_1 and higher', 'Requires DXGI 1.2'; it
leaves `B5G5R5A1_UNORM`'s target rows '-') require them; the feature-level overview
(overviews-direct3d-11-devices-downlevel-intro, 'Direct3D 11 feature support') marks '16 bits
per pixel (bpp) formats' Optional at 9_1 to 11_0 (footnote: needs the 11.1 runtime) and
required only from 11_1. On the overview's reading, 11_0 without B5 targets is a truthful
optional absence, which `CheckFormatSupport` reports (no render target or blend bits); on the
per-format tables' reading it is a shortfall of 11_0 and of every lower level alike. This list
keeps the stricter reading and names the wait; either way no other level would be more truthful:
the 10Level9 table asks the same of 9_1 and up, the RTX 3080 claims the B5 targets at 10_1 and
10_0 too (measured, the format support tests at each level), and the integer twins' blending is
required at every level, so a lower level would not remove these and would refuse every program
of the bar.

Absent features (each reported absent through its own query, so Wine's verdicts there differ):
Wine's d3d11 file: 15 of 166 failing (logs/q3l-wine.log).
Every failing test but test_format_support and test_swapchain_formats (the 11_0 shortfalls
above) and test_vertex_formats (fewer checks, the B8G8R8X8 shortfall) sits behind an absence:
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
Wine's d3d10core file: 1 of 111 failing (logs/q2f-wine10.log), test_desktop_window, as above.

The WebGL 2 build (10_0; the bar's WebGL 2 cases match real Direct3D through it): what it does
not give real Direct3D's verdict on, measured at round 2 run 12 (Wine's d3d10core 98 of 111
through it, logs/r12d-wine10gl.log; the rig's scenes 95 pass, 17 fail, 30 absent), each with its
owner:
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
- Formats the WebGL 2 back end lacks: the 16-bit normalised ones are held as integer twins since
  round 3 run 1 (not yet rerun through the WebGL 2 build's Wine files); `A8_UNORM`:
  test_create_texture2d, test_texture, test_render_a8, test_clear_render_target_view_2d.
- Abilities Microsoft's 10_0 table requires that the WebGL 2 build reports absent (the fifth
  check compared its format support with a real 10_0 device, check-r10/logs/claims-gl10.txt):
  render targets of the six 8- and 16-bit signed normalised formats (`R8G8B8A8_SNORM`,
  `R8G8_SNORM`, `R8_SNORM`, `R16G16B16A16_SNORM`, `R16G16_SNORM`, `R16_SNORM`; WebGL 2 draws into
  no signed normalised format without `EXT_render_snorm`, which the back end does not use);
  textures of `R8G8_B8G8_UNORM`, `G8R8_G8B8_UNORM`, `B5G5R5A1_UNORM`, `B4G4R4A4_UNORM` and
  `A8_UNORM`. Each is the WebGL 2 back end's format table (Asked). BC6H and BC7, which a real
  10_0 device refuses, are refused and unclaimed below 11_0 since round 4 run 2. The same probe
  lost the WebGL 2 device at `R16G16B16A16_FLOAT`'s staging read back ("texture copy made through
  a buffer", then GL error 0x502); not traced in this run.
- test_format_compatibility (16 checks): a 16-bit depth texture copied into an `R16_TYPELESS`
  colour texture held as `R16Float`, copied by a draw that makes half-float NaNs one NaN (0x7fff).
  The WebGL 2 track's copy route (Asked).
- test_generate_mips (56 checks, all on 3D textures' second level, the first "Resource type 3,
  test 0: pixel (5, 6) has color 48484848, expected 00000000"): the WebGL 2 back end's. The
  front makes the same `generate_mips` call on both builds (the view's levels and every slice
  of a whole 3D view); the WebGPU back end takes the interface's default
  (`generate_mips_by_draws`) and passes all 413 checks, while the WebGL 2 back end takes its own
  route for a whole view of a format it draws and filters, `glGenerateMipmap` on the 3D texture
  (wcontainers-gpu-webgl lib.rs, `generate_mips`). Read from the code, not run through the
  default on WebGL 2 (Asked of the WebGL 2 track).
- Wireframe without WEBGL_polygon_mode (`FillModeConverted`, not built for any back end); pixel
  shaders reading `SV_PrimitiveID` without a geometry shader (needs compute): the rig's raster and
  geometry scenes.
- Wine's d3d11 file through the WebGL 2 build (not on the bar): 78 of 166 at round 2 run 12.

Real programs: the three of the pass bar (godot-platformer-3d-d3d11, chocolate-doom,
atg-simplelightingpc) match real Direct3D on every compared frame (logs/q2f-programs.log, round 4 run 2), and so
do atg-simplebezierpc, atg-simpleinstancingpc, godot-platformer-2d-d3d11 and sdk-tutorial02 and 04
to 07; of bgfx's six (logs/q2f-bgfx.log) five pass or run to their end. Outside the bar's three,
three differ:
- bgfx-26-occlusion: 6,458 pixels beyond the tolerance in each saved frame, in the overlay's text
  (the passing pixel count it prints from its occlusion queries). Real Direct3D 11's own two runs
  this run passed its references (169 pixels beyond, within the allowance, logs/q3j-sysruns.log),
  so run-to-run variation on real Direct3D 11 does not explain ours; the cause is not found.
- atg-simplecomputepc (outside the bar's three): every pixel inside the Mandelbrot set differs; the
  converted compute program's division gives just below 4 where real Direct3D gives 4 (inferred).
  The shaders track's (Asked).
- atg-simplemsaa_pc (outside the bar's three): frame 10 has 8 pixels beyond the channel tolerance
  (largest 11), 4 of them beyond their tiles' allowance; frames 60 and 120 pass. Real Direct3D's
  own two runs this run had 0 and 1 such pixels in frame 10 (logs/q3j-sysruns.log), within the
  allowance; the cause is not found.

Listed cases of section 12, each with its test against real Direct3D:
- Every listed case passes against real Direct3D on WebGPU, the WebGL 2 cases through the WebGL 2
  build, and the case without texture-formats-tier1 or 16-bit norm since this run.
- Failing: constant buffers updated per draw, counted on the WebGPU back end itself (com.rs
  `constants_rewritten_between_draws_make_no_bind_group_per_draw_on_webgpu`, ignored as a known
  failure): after each buffer's first draw, 2 more bind groups for 2 more draws, for
  `Map(WRITE_DISCARD)` and for `UpdateSubresource` alike (round 4 run 2, logs/q2a-lib.log).
  Cause: the WebGPU back end puts a buffer in its uniform arena (whose slices move on a discard
  under the same bind group) only when `desc.usage == BufferUsage::UNIFORM` and the intent is
  `Often` (wcontainers-gpu-webgpu src/lib.rs, `create_buffer`), while the interface makes
  `COPY_DST` the condition of `write_buffer` (wcontainers-gpu src/resource.rs, `BufferUsage`).
  No buffer a front may write is uniform alone, so the arena is reached by no front; its own GPU
  buffer already has `UNIFORM | COPY_DST | COPY_SRC | STORAGE` (`memory::arena_usage`), so
  admitting any uniform buffer whose usages lie within those would serve this case. This front
  declares constant buffers `UNIFORM | STORAGE | COPY_SRC | COPY_DST` (storage for
  `ConstantsAsStorage`, copies for `CopyResource` and `UpdateSubresource`) and, since round 4 run
  2, default ones written `Often` as dynamic ones are. The WebGPU track's (Asked). The drawing
  itself matches real Direct3D (`own_handled`); no pass split is counted on the recording back
  end, as the WebGPU back end counts no passes.
- Failing: aliased lines (`MultisampleEnable` FALSE) into a multisampled target
  (`aliased_lines_into_a_multisampled_target_hit_every_sample_as_direct3d_does`, ignored as a
  known failure): real Direct3D covers every sample of the pixels the diamond rule picks; WebGPU
  draws lines into a multisampled target by its own rule. Needs an aliased-line route (the
  shaders track and the interface).

Differences from the RTX 3080 that Wine's verdicts do not show:
- `GetDimensions` of a view of a YUY2 texture: the RTX 3080 reports the texture's width in pixels,
  ours the view's texels (pixel pairs), which is what the back end's texture is. The interface has
  no helper value giving a program another width than its texture's (interface and shaders
  tracks; Asked). Known failure `yuy2_view_sizes_read_as_direct3d_reads_them` (ignored).
- An NT handle of a shared resource is an event object made by this library, where Windows makes
  a `DxgkSharedResource` (a kernel object only the display driver's kernel side makes):
  duplicating, closing and opening it work; its object type differs. Wine's test_shared_resource
  fails no check at 11_0.
- Shared resources are shared between the devices of one process; a KMT or NT handle from another
  process is not found (`E_INVALIDARG`), where Windows opens it. The container has no shared
  driver memory between processes to serve it with.
- A swap chain's back buffer used after the program released the swap chain: real DXGI keeps it
  working, ours destroyed its texture and the next view lost the device (seen in round 4 run 3
  while writing own_swap_chain; not fixed).
- `ClearView` of an integer storage view with negative values: the RTX 3080 writes -5 into an
  8-bit unsigned channel as 0xfb and any negative value into a 16-bit signed channel as 0x7fff
  (measured, round 4 run 4, logs/r4a-cv-sys.log); the functional specification (5.2.3.3) clamps
  out-of-range values to the channel and ours does so (0 and the value itself,
  `storage_integer`). On the RTX 3080 a storage view cleared whole and the same view cleared by
  rectangles also differ from each other for such values (the ninth check's probe9). The listed
  case (`clear_view`) leaves negative storage values out for this reason.
- Target clears of integer formats (`ClearRenderTargetView`, `ClearView`) convert as the RTX 3080
  does since round 4 run 5 (`target_integer`), which is not the specification's clamp for values
  past 32 bits: fractions dropped, NaN 0, the infinities the channel's limits; a signed value
  outside 32 bits becomes the least 32-bit integer, an unsigned one below zero or past 64 bits
  0 and any other its low 32 bits, each then limited to the channel. Measured on the RTX 3080
  (scratch m5/out and the ninth check's probe9 cleari) and in the listed case `clear_view`
  (five formats, three sets of such values, whole and by rectangles), which matches line by
  line.
- `ClearView` of a buffer view's rectangles is refused by name (`CLEAR_VIEW_BUFFER_RECTS`; the
  specification defines rectangles on 2D surfaces only); of rectangles of a storage view held
  as an integer twin or stored through a buffer, of a 3D texture or one no pass draws into,
  refused by name too. No program measured reaches either.
- Creation refusals answered `E_NOTIMPL` where the RTX 3080 answers `E_INVALIDARG` (both fail;
  only the code and the warning differ; the eighth check's probe): render target views and
  mip generation of 1D, 3D and array textures of the R32G32B32 formats and `B4G4R4A4_UNORM`;
  1D, 3D, array and cube-array textures of `R1_UNORM` and of the video formats ours holds no
  texture of; shared textures of P016 and NV11. Telling an optional ability ours does not claim
  (which the runtime refuses as invalid) from a shortfall of the reported level (unserved) needs
  Microsoft's per-level table in the front; not built. Typed buffer views of typeless formats
  and typed storage buffers of formats Direct3D does not store (the R32G32B32 formats among
  them, which ours made before) are `E_INVALIDARG` since round 4 run 4; typed buffer views of
  the formats ours holds no texture of (`R1_UNORM`, the extended-range format, the video
  formats) are still `E_NOTIMPL` there. 22 formats differ so in the probe (probe8/out).
- d3d11 test_clear_buffer_unordered_access_view passes through ours, where real Direct3D fails 223
  of its 934 checks: ours clears by the documented rule; NVIDIA's driver does not.
- `IDXGISwapChain::GetContainingOutput` answers `E_NOTIMPL` (the infrastructure crate's).
- `B8G8R8X8_UNORM` and its sRGB twin: the X byte reads 0xff after a clear, a draw and initial
  data, where the RTX 3080 writes 0x00 for clears and draws and keeps the bytes it was given; the
  WebGPU back end holds the format as `bgra8unorm`, clears alpha to 1 and widens initial data
  with 0xff (src/packed.rs). The WebGPU track's (Asked).
- Values just past a half between two codes drawn into a 16-bit normalised texture held as its
  integer twin (natively `WEBGPU_OFF=wgpu-texture-format-16-bit-norm`): measured by the ninth
  check (probe9 twinsx), 0.1000000089 * 65535 = 6553.5006 is written 6553 by the RTX 3080 and
  6554 (the nearer code) by ours, in the drawn texture and in every view and copy of it; both
  are within the specification's conversion tolerance. The shaders crate encodes the program's
  output; exact halves go toward zero as on the RTX 3080 since its 5873eeaa. Blending into a twin
  is refused by name (Gap list). Filtered samples and mips of twins
  differ by 1 ULP at most (the shaders crate's decoding, the interface's default
  `generate_mips`), within the specification's tolerance.
- `ID3D11Device3::ReadFromSubresource` and `WriteToSubresource` are not served: they warn and
  remove the device. No valid call reaches them, as `D3D11_OPTIONS2.MapOnDefaultTextures` is FALSE.
- A swap chain on another process's window (the desktop window among them) is refused with
  `DXGI_ERROR_INVALID_CALL`, warned (Departures); real DXGI makes one.
- Drawing a geometry stream past 0 is refused by name (`GEOMETRY_STREAM_DRAWN_PAST_ZERO`).
- Building ahead at a shader's arrival (plan 4.4) has never been hit (Measurements). Measured
  cause (round 4 run 2): 122 of Godot 3D's 124 pairs built ahead are pairs it never draws (a new
  shader is paired with the latest shader of the other stage), and its 2 drawn pairs draw with
  other state than the single most drawn tuple picked for a vertex shader not drawn before. The
  cost is background pipeline builds that are never used (124 in Godot 3D's warm run).

## Format support against the RTX 3080

Every bit of both words of every format, and the second query's result, compared in
`format_support_as_direct3d_reports_it_but_the_named_shortfalls` (11_0), in
`format_support_at_level_10_1_...` and `format_support_at_level_10_0_...` (devices of those
levels, since round 4 run 5), and without the 16-bit normalised formats in
`format_support_without_16_bit_norm_as_direct3d_reports_it_but_the_named_shortfalls` (all four
passing, round 4 run 5; the last allows only the twins' blending, named). Below 11_0, as the
RTX 3080 answers and since round 4 run 5 ours: sharing (`SHAREABLE`) is claimed from 10_0 (where
ours serves it) and the second query answers it; no storage view, typed store or atomic is
claimed; gathers are claimed at 10_1 only for formats of a red channel alone, uncompressed, and
the depth views of one aspect (shader model 4.1's gather), and for NV12 and YUY2; multisampled
depth is not read at 10_0 (no `MULTISAMPLE_LOAD` on the depth views, and four-sample depth
textures made for shaders are refused, `multisampled_depth_read_by_shaders_as_direct3d_takes_it_at_each_level`).
The WebGL 2 build's own 10_0 table is not in these tests (its differences are the Gap list's
WebGL 2 item). The differences at every level, by cause:
- Absent features, never claimed by ours: video (the first word's bits from `DECODER_OUTPUT` up;
  no `ID3D11VideoDevice`), logic operations (`LOGIC_OP`), tiled resources (`TILED`) and
  multiplane overlays. Sharing (`SHAREABLE`) is served and compared bit for bit since round 4
  run 3. Where these were a format's only second-word bits, ours
  fails the second query as Direct3D fails a word without bits (formats 1, 7, 9, 15, 23 and
  others).
- Multisampling per format (plan section 9's listed shortfall): ours claims the multisampling bits
  only where the back end draws a count of the format, a subset of real Direct3D's (R32G32B32
  formats, B5G6R5, B5G5R5A1, B4G4R4A4 and others lack some).
- Optional and served: `R32G32B32_FLOAT` is sampled (and at 11_0 gathered) by ours, not by the
  RTX 3080.
- Optional video formats absent altogether: AYUV, Y410, Y416, P010, P016, Y210, Y216, NV11, AI44,
  IA44, P8, A8P8.
- Shortfalls of 11_0, listed in the Gap list, each waiting on another track: the B5 targets,
  `R10G10B10_XR_BIAS_A2_UNORM`'s display and back buffer casts, `B8G8R8X8_UNORM` elements and
  typed buffers. NV12's and YUY2's storage views, stores and gathers, NV12's blending,
  `420_OPAQUE` textures and `R10G10B10A2_UNORM`'s back buffer casts are served since round 4 run 3.
- Typed storage loads (`UAV_TYPED_LOAD`) and every atomic bit agree with the RTX 3080 for every
  format but `A8_UNORM`, whose storage views are refused (Gap list).
- The seventh check's probe (scratch probe3, round 4 run 3, logs/q3k-probe.log): every creation
  ours claims succeeds and every one it serves is claimed, but those where the RTX 3080 itself
  claims and refuses alike (cubes of the depth formats, NV12 and YUY2 views without a
  description). `CheckMultisampleQualityLevels` now matches the RTX 3080 for every value 0 to 200
  at counts 1 to 33 but the B5 formats' counts 2 to 8 (the per-format multisample shortfall) and
  the absent video formats' single sample.

## Known failures

- `aliased_lines_into_a_multisampled_target_hit_every_sample_as_direct3d_does`,
  `b8g8r8x8_elements_read_as_direct3d_reads_them` and `yuy2_view_sizes_read_as_direct3d_reads_them`
  (library, programs.rs) and
  `constants_rewritten_between_draws_make_no_bind_group_per_draw_on_webgpu` (library, com.rs) fail
  on every build and are marked `#[ignore]` with the reason (Gap list); run them with
  `--include-ignored`.
- On the WebGL 2 build only: the library tests that ask for 11_0 or 10_1 fail, as the build
  reports 10_0, or reach the absences of the Gap list's WebGL 2 item (round 2 run 12: com 14 of
  15, programs 12 pass, 29 fail). The bar's WebGL 2 run takes the 10_0 cases alone; of the
  tests it selects, `direct3d_10s_device_draws_and_keeps_its_state_as_direct3d_does` (an 11_0
  device with a 10_1 state) is marked ignored on that build with that reason since round 5
  run 1. The others are not marked: the same tests pass on WebGPU.

## What comes next

In order:
1. The B5 targets, XR_BIAS's swap chain, the B8G8R8X8 vertex format, A8 storage views and the
   YUY2 view's size, once the WebGPU, infrastructure, interface and shaders tracks give their
   parts (Asked); then remove each from `support_differences` and un-ignore
   `b8g8r8x8_elements_read_as_direct3d_reads_them` and `yuy2_view_sizes_read_as_direct3d_reads_them`.
2. Constant buffers rewritten per draw: once the WebGPU back end's arena admits uniform buffers
   with other usages (Asked), un-ignore the counters test and run it.
3. Building ahead at a shader's arrival: pair shaders by what programs draw together rather
   than by creation order (for example only from the vertex shader's arrival, or from pairs
   recorded in the cache), and pick tuples by the pixel shader's targets; measure again with the
   diagnostic described in Where things stand.
4. A back buffer kept by the program past its swap chain's release (Gap list): keep the chain's
   textures alive while a buffer is held.
5. Blending into integer twins (`BlendInShader`) once the shaders crate builds `combine`.
6. Creation refusals of optional abilities ours does not claim as `E_INVALIDARG` (Differences
   from the RTX 3080): needs the per-level requirement table.
7. Prove whose the WebGL 2 build's 3D second-level failure of test_generate_mips is; rerun the
   WebGL 2 build's Wine files with the integer twins.
8. Replace the cube array probe (`makes_cube_arrays`) with a `Caps` feature once one exists.
9. Wine's dxgi file: the null-out-pointer crashes, `CreateSurface`, `CreateSubresourceSurface`.
10. Section 10's joint step with the Direct3D 12 track, after both milestones pass.
11. The literal fix list, a part per run.

## How to run

Everything builds and runs on the desktop through the rig (docs/status-graphics-rig.md, "How to
run"); the Mac runs only formatting, git, and checks of the front crate on the rented x86
machine. Scripts named below are in scratch (pyro-parked/wcontainers-scratch/graphics-direct3d-11);
logs go to its logs/. Do not send this checkout's tree (remote, gpu-job, scenes) while another of
its jobs waits in the queue: the waiting job then stops ("holds tree ..., not the sent ..."). Never
run `tools/graphics-rig/gpu-job --help` (it queues `--help` as a command). The desktop's checkout
needs the shaders crate's submodules, so commands start with `git submodule update --init
--recursive -q`.

- Library tests matching a filter, and lint of the changed crates: `run-q1.sh <label>
  "lib=--test programs -- <filters> --test-threads 1"` and `run-q1.sh <label> check`.
- The whole bar, then release module sizes and building-ahead hits: `run-g4t1.sh <label>` (library
  tests, Wine's d3d11 and d3d10core files, every scene through the library, the WebGL 2 build's
  scenes, the real programs and bgfx's, sizes, then run-warm.sh). Its steps one by one:
  `run-g4s13.sh <label> [literals lint front lib wine wine10 scenes webgl programs]`.
- This run's bar in one command: scratch q3-final.sh <label> (lint, library, the WebGL 2 build's
  10_0 cases, Wine's two files, scenes, programs, the seventh check's probe beside the system's
  and ours, the eighth check's (`probe8`, analysed by `cd probe8/out && python3 ../dims.py
  -codes`), sizes, load times, building ahead, and real Direct3D's own runs of the two programs
  that differ); probe3/claims.py compares each library's claims with its creations.
- The literal check at HEAD (commit first): `lit-check.sh <label>`; a `-` line it prints is a
  baseline line to delete in the same commit.
- Lint: `tools/graphics-rig/remote "git submodule update --init --recursive -q && cargo clippy -q
  -j 3 -p wcontainers-direct3d-11 -p wcontainers-directx --all-targets -- -D warnings"`, and the
  same with `--features wcontainers-directx/webgl --target-dir %CARGO_TARGET_DIR%\webgl`.
- Front tests (recording back end, no GPU): on the rented x86 machine,
  `orchestrator/cloud/x86-run <checkout> --name g-d3d11 --jobs 32 -- "cargo test -q -p
  wcontainers-direct3d-11"` (or the same through `tools/graphics-rig/remote`).
- Library tests (real devices and a window, so a GPU job on the hidden desktop):
  `tools/graphics-rig/gpu-job --desktop --build "<sub> && cargo test -q -j 3 -p wcontainers-directx
  --no-run" -- "cargo test -q -j 3 -p wcontainers-directx -- --test-threads 1"`; the WebGL 2 build
  with `--features webgl --target-dir %CARGO_TARGET_DIR%\webgl`. The random lifetimes comparison
  takes `WC_LIFETIMES=<first seed>,<seeds>,<steps>` (default 1,8,300); a failure names the seed.
- Wine's files through the library (build it first with `cargo build -q -j 3 -p
  wcontainers-directx`): `rigfix/wine.sh --dll 'd3d11.dll=%CARGO_TARGET_DIR%\debug\wcontainers_directx.dll'
  d3d11[:test_a/test_b]`, and `--dll 'd3d10.dll=...'` with `d3d10core`. rigfix/wine.sh is the rig's
  wine-tests with its runner fixed in one line. Per-test logs land in
  graphics-rig/wine-logs/phase-2-graphics-direct3d-11/. Wine's dxgi file: `run-dxgi.sh <label>
  [test_a/test_b]` (needs the test-only d3d10_1.dll forwarder in
  C:\wc\out\phase-2-graphics-direct3d-11\d3d10_1fwd; remake it with link.exe from its .def there).
- Scenes and programs through the whole library (our DXGI too): `--dll
  'd3d11.dll=%CARGO_TARGET_DIR%\debug\d3d11.dll' --dll 'dxgi.dll=%CARGO_TARGET_DIR%\debug\dxgi.dll'
  --dll 'wcontainers_directx.dll=%CARGO_TARGET_DIR%\debug\wcontainers_directx.dll'` on
  `tools/graphics-rig/scenes` or `tools/graphics-rig/programs run`; always the three together.
  bgfx's examples also take `--dll 'nvapi64.dll=C:\wc\out\phase-2-graphics-direct3d-11\nonvapi\nvapi64.dll'`
  (an empty file, so NVIDIA's NVAPI does not load; Departures).
- The WebGL 2 build loads ANGLE from `WEBGL_ANGLE_DIR` (default C:\wc\tools\angle-mozangle-0.7.1)
  and takes the WebGL extensions `WEBGL_OFF` lists as missing; `WEBGPU_OFF` does the same for
  WebGPU features on the WebGPU build.
- Load time of a module: example `load_time` (scratch run-r8-load.sh); release sizes of both of
  this library's builds and of the Direct3D 12 track's module, with each one's load time alone:
  run-r8-sizes.sh <label>.
- The fifth check's format claims probe, beside the system's d3d11.dll and ours (16-bit norm on
  and off): scratch probe2 (built on the desktop in C:\wc\out\phase-2-graphics-direct3d-11\probe2
  with `cargo build`), run by `tools/graphics-rig/gpu-job -- "C:\wc\out\phase-2-graphics-direct3d-11\probe2\run.cmd"`
  after a debug build of the library; outputs copied to scratch probe2/out.
- This run's final steps in one command: scratch q2-final.sh (lint of both builds, the whole
  library suite, the WebGL 2 build's 10_0 cases, the probe, sizes and load times).
- src/com/generated.rs: `python3 crates/wcontainers-directx/generate.py <windows-0.62.2 crate dir>`,
  then `cargo fmt -p wcontainers-directx`. A method named in its `hand` table calls the hand-written
  one.
- If a run on the desktop does not reflect a change (its clock once moved back), run
  `tools/graphics-rig/remote "cargo clean -q -p wcontainers-directx -p wcontainers-direct3d-11"`.

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

At round 4 run 4: front.rs 59, tests/cache.rs 1, unit 5 (wcontainers-direct3d-11, recording
back end); com.rs 16 (1 ignored) and programs.rs 52 (3 ignored) (wcontainers-directx, real
devices), of which 4 are ignored as known failures (Known failures). New in round 4 run 4:
programs.rs `clear_view_without_texture_formats_tier1_as_direct3d_does`; own_cases `clear_view`
covers storage rectangles of seven formats, an array, integer targets, buffers, 1D and 3D
views; front.rs `creation_refuses_what_direct3d_refuses` covers typed buffer views of typeless
formats and typed storage buffers of the R32G32B32 formats. New in round 4 run 3:
front.rs `formats_direct3d_shares_claim_sharing` and 420_OPAQUE's refusals; programs.rs
`resources_shared_between_devices_answer_and_read_as_direct3d_does` (example own_shared) and
`yuy2_view_sizes_read_as_direct3d_reads_them` (ignored); own_nv12 and own_yuy2 gather and store
through storage views, own_nv12 makes 420_OPAQUE textures, own_swap_chain takes views of its
buffers in the other formats of their group; own_refused's cases are now all served. New in
round 4 run 2:
front.rs `creation_refuses_what_direct3d_refuses`; programs.rs
`xr_bias_textures_as_direct3d_does` (own_cases `xr`) and
`format_support_without_16_bit_norm_as_direct3d_reports_it_but_the_named_shortfalls`; the
counters test measures both halves; the encoding test asserts the RTX 3080's halves. New in
round 3 run 1: front.rs
`a_normalised_format_the_back_end_lacks_is_held_as_its_integer_twin` and
`clear_values_of_integer_twins_are_encoded_as_direct3d_converts_them`; com.rs
`constants_rewritten_between_draws_make_no_bind_group_per_draw_on_webgpu`; programs.rs
`b8g8r8x8_elements_read_as_direct3d_reads_them` (ignored); the format support test compares every
bit; the 16-bit-norm case runs. Every listed case of section 12 has its test against real
Direct3D. programs.rs runs each example once beside the system's d3d11.dll and once beside ours
and compares what each printed; the random lifetimes comparison is seeded (`WC_LIFETIMES`,
default seeds 1 to 8 at 300 steps each).

## Departures from the plan

- Round 4 run 5: integer target clears follow the RTX 3080 (the reference) where it departs
  from the functional specification's clamp (5.2.3.3) for values past 32 bits: a signed value
  outside 32 bits becomes the least 32-bit integer, an unsigned one keeps its low 32 bits
  (`target_integer`). Storage views keep the specification's clamp (`storage_integer`), which
  the RTX 3080's storage rectangles follow and its whole storage clears do not (Differences from
  the RTX 3080). Two conversions, because the reference converts the two kinds of view
  differently and the listed case compares each.
- Round 4 run 3: shared resources are shared between the devices of one process only, each
  device holding its own copy, copied through the host when a device flushes or releases the
  keyed mutex (`SharedThroughHost`, plan section 9's text added with the interface change). A
  copy is made only where the bytes read back differ from the last ones handed over; a program
  that writes a shared resource on two devices between synchronisations (undefined in Direct3D)
  sees the last hand-over win. Multisampled textures shared across devices are refused by name.
- Round 4 run 3: an NT handle of a shared resource is an unnamed (or named, with the name given)
  event object; `OpenSharedResource1` finds its share by `CompareObjectHandles`, and
  `OpenSharedResourceByName` by the name given to `CreateSharedHandle`. The object keeps nothing
  alive: the share lives while a resource of it does.
- Round 4 run 3: 420_OPAQUE is held as NV12 (a luma plane and a chroma plane at half size) and
  refused with bind flags, CPU access or another usage than default, as the RTX 3080 refuses
  them (its claims and the probe's creations); the layout is the driver's in Direct3D, and no
  program can see it.
- Round 4 run 3: render target views of a swap chain's buffer in another format of its group go
  through the typeless route (`ReinterpretCopy` where the bits are stored as another format).

- Round 4 run 2: `R10G10B10_XR_BIAS_A2_UNORM` textures are held in `Rgb10a2Unorm`'s bits and
  take every bind flag the RTX 3080 takes (shader resource, target, unordered access), but no
  view of the format is made (`E_INVALIDARG`, as on the RTX 3080, which claims no shader or
  target use of it at 11_0); copies between it and the R10G10B10A2 formats copy bits, as
  measured there. Its display waits on the infrastructure crate (Gap list).
- Round 4 run 2: normalised clear values of integer twins round halves toward zero, as the RTX
  3080 clears (the functional specification allows either within 0.6 ULP).
- Round 4 run 2: input layouts whose elements, program-fetched elements and instance steps are
  equal share one id, so the vertex layout cache and building ahead's tuples treat them as one
  (programs through ANGLE make one per vertex shader).
- Round 4 run 2: default constant buffers are declared written often (`MapIntent::Often`), as
  dynamic ones are: programs rewrite them by `UpdateSubresource` between draws.
- Round 3 run 1: 16-bit normalised textures held as integer twins are blended nowhere: blending
  into one is refused by name (`BLEND_INTO_INTEGER_TWIN`), where plan 4.12 blends them in the
  program from a copy of the target (`BlendInShader`), which the shaders crate refuses today
  (`TargetInShader::combine`). `CheckFormatSupport` reports such formats without `BLENDABLE` and
  without multisampling (plan 4.12 names the latter). The twin is taken only where the back end
  has no use of the format at all and samples its twin; 8-bit signed normalised formats the back
  end samples but cannot draw into are not held as twins.
- Round 2 run 9: plan section 9 says every format ability Microsoft's tables require at the
  reported level is served, but for two named shortfalls (multisample counts per format; the
  video formats' video abilities). The device reports 11_0 on WebGPU with these further
  shortfalls against the 11_0 table (format-support-for-direct3d-11-0-feature-level-hardware.md,
  saved in scratch graphics/downloads/msdocs), each compared with the RTX 3080 in
  `format_support_as_direct3d_reports_it_but_the_named_shortfalls`:
  - `B5G6R5_UNORM` and `B5G5R5A1_UNORM` as render targets and blended targets (and so no mip
    generation): the WebGPU back end has neither as a target (the WebGPU track's);
  - `R10G10B10_XR_BIAS_A2_UNORM`'s display and back buffer casts: no extended-range swap chain
    (the infrastructure crate's; the rest of its row is served since round 4 run 2);
  - `B8G8R8X8_UNORM` as a vertex element and typed buffer: no vertex format giving X as 1 (the
    interface track's with both back ends and the shaders crate, Asked);
  - back buffer views of `R10G10B10A2_UNORM` in other formats of its group, NV12's and YUY2's
    storage views and gathers, NV12's blending, and `420_OPAQUE` textures: served since round 4
    run 3.
  Why 11_0 is still reported: the next level down (10_1) shares most of them (its table requires
  the B5 targets and BC formats too), and reporting below 10_0 would refuse every program of the
  bar; the plan's rule (section 9) is to report the highest level served, which these shortfalls
  break, so they are departures to be closed, not absences.
- Round 2 run 9: a swap chain on another process's window is refused with
  `DXGI_ERROR_INVALID_CALL` (warned), where real DXGI on the RTX 3080 makes one: the WebGPU back
  end presents through a flip-model swap chain, which presents only to the process's own windows.
  Wine's test_desktop_window accepts the refusal (`broken`).
- Round 2 run 7: the names of Windows' Direct3D and DXGI runtime libraries and entry points live
  in the front crate (`wcontainers_direct3d_11::runtime`), not in the library that serves them:
  the example own_handled, which must name them, cannot link the library crate (its exported
  entry points clash with d3d11.lib's), and a fact is defined once.
- Round 2 run 7: Wine's dxgi file runs with a test-only d3d10_1.dll forwarding its one import
  (`D3D10CreateDevice1`) to the library, made by hand on the desktop, not a forwarder the build
  makes: d3d10_1.dll also exports the effects framework and reflection, which this library does
  not serve, so a shipped forwarder would break programs importing those (as for d3d10.dll, run
  21 below).
- Round 2 run 6: the library reads `WEBGPU_OFF` (WebGPU features by their kebab-case names,
  taken as missing), beside the existing `WEBGL_OFF`, so that the routes browsers without a
  feature take are tested natively against real Direct3D (the listed case without
  texture-formats-tier1). Natively wgpu has no texture-formats-tier1 feature of its own; taking
  `wgpu-texture-adapter-specific-format-features` away is the nearest: it leaves WebGPU's
  guaranteed format uses, under which `R11G11B10_FLOAT` cannot be stored, but `R16G16_UNORM`
  still can (wgpu's 16-bit-norm feature guarantees it).
- Round 2 run 12: the feature level check needs cube arrays from 10_1 (Microsoft's feature level
  table), and `Caps` names no such feature, so the device asks the back end at its creation for
  a texture of two cubes of one texel and destroys it (`makes_cube_arrays`); a refusal stops the
  level at 10_0. This is how the WebGL 2 build reports 10_0, as plan section 9 fixes it. The
  cost is one texture made and destroyed per device. A `Caps` feature is the plan's form;
  adding one now would take 10_1 and 11_0 from the WebGPU back end until its track declares it
  (Asked of the interface track).
- Round 2 run 10: typed buffers read through texture copies (`TexelBufferAsTexture`) copy the
  whole buffer into a texture of each view format the first time a draw reads it after a write,
  not only the view's range: a view's first element is then at its own byte offset over the
  format's size, as the shaders crate's route reads it. Typed unordered access buffers stay
  refused there (the route reads texture slots only; they are 11_0's).
- Round 2 run 4: plan 4.12 lists aliased lines drawn into multisampled targets by the back
  end's rule as a loss at the line's ends and edges, to be compared with Microsoft's
  conformance tolerance for lines. No such comparison exists yet (Gap list): the loss stands
  unmeasured against that tolerance.
- Round 2 run 4: the round 2 run 3 entry below is corrected: whether the bias applies follows
  the primitives rasterised (a geometry stage's output, else the tessellator's), so triangles
  made by those stages are biased and their lines and points are not, as on the reference
  (`own_cases staged_bias`); the refusal of a geometry stage's biased lines and points is gone.

- Round 2 run 3: plan section 5.1 says depth bias on line and point draws is `DepthBiasInShader`;
  real Direct3D on the RTX 3080 does not bias lines and points at all (`own_cases
  biased_lines`: a `DepthBias` of 1000 left 0.25 exact in D16 and D24 depth), so this front draws
  them without it, as the reference does. The functional specification's text on it was not
  read; the reference was. A geometry stage's lines and points under a bias stay refused by name
  (`draw_output`), as before.
- Run 25: origin/phase-1-literals was merged into this branch directly. docs/literals.md says a
  track adopts the check through main; the brief (2026-11-18, late) says to merge
  origin/phase-1-literals and adopt it now, and the WebGL 2 track's branch, which this branch
  merges, had already done so, bringing the check and its baseline here anyway. So this branch
  must not merge a main that lacks the check (docs/literals.md step 1).
- Run 25: a stage reading more constant buffers than the back end's uniform buffers keeps one
  uniform buffer for the helper constants whenever its variant may have them, so a program using
  exactly the back end's count with no helpers reads one slot through a texture it did not need
  to (slower, exact). The back end's combined limit for a program's two stages is not checked.
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
- d3d11.dll, dxgi.dll and d3d10core.dll are forwarders; d3d12.dll is not (section 10 names it):
  the library exports no Direct3D 12 entry point, and the Direct3D 12 track's
  `wcontainers_d3d12.dll` has a DXGI of its own. Joining them is the joint step after G4 and G5
  pass (the orchestrator's ruling of 2026-11-20); until then the two modules are measured apart.
- `D3D11_OPTIONS` answers `ClearView` TRUE since round 4 run 4: target and storage views, whole
  or their rectangles (no video view can be made). Rectangles of a storage view go through the
  shaders crate's storage clear program (`builtin::storage_clear`), one dispatch per rectangle
  over every layer or slice the view sees; a storage view held as an integer twin or stored
  through a buffer is cleared as a target where its texture is drawn into. Rectangles of a 3D
  target view are copied into each slice from a texture of one slice cleared whole
  (`ReinterpretCopy`): the interface's `clear_rects` draws into one slice of a view. Whole
  storage views of textures no pass draws into take the storage clear program too (they were
  refused before). Integer clear values are clamped to the channel, as the specification says.
  `DiscardAPIsSeenByDriver` is FALSE: discards keep the contents,
  so no driver sees them. `ConstantBufferPartialUpdate`, `ConstantBufferOffsetting` and both
  `MapNoOverwriteOnDynamic...` answers are TRUE since run 8.
- `CreateDXGIFactory` gives DXGI 1.0's factory (no `IDXGIFactory1`) only when asked for
  `IUnknown`, `IDXGIObject` or `IDXGIFactory`, refuses `IDXGIFactory1`, and gives the whole
  factory when asked for a later interface (run 8). Microsoft's page says only that 1.0 and 1.1
  are not mixed; Wine's test_create_factory shows Windows answering `IDXGIFactory2` from
  `CreateDXGIFactory`, and this follows it.
- The library draws through wgpu on Direct3D 12 only (section 10 allows Direct3D 12 or Vulkan): a
  machine without Direct3D 12 gets `DXGI_ERROR_UNSUPPORTED` from `D3D11CreateDevice`.


## Interface changes

- Round 4 run 3, 978d1661 "interface: named the slow path of a resource shared between devices
  copied through the host": `SlowPath::SharedThroughHost` (added at the end, its number kept),
  and one sentence in plan section 9 on shared resources.

- Round 3 run 1, 8b556a05 "interface: named each normalised format's integer twin as
  Format::integer_twin and read generate_mips' twins from it": `Format::integer_twin` (the
  integer format of the same bits a front holds a normalised format as, `NormAsInteger`); the
  default `generate_mips` reads its twins from it instead of its own list. No behaviour changes.
- c8a7be9 "interface: copied combined depth-stencil textures whole in copy_texture": the
  documentation of `copy_texture` and `TextureRegion::stencil`, and the plan's sentence in 4.2: a copy
  between two textures of a combined depth-stencil format copies both aspects in one call, naming
  none; copies to and from buffers name one aspect.
- 7f73347 "interface: named the slow path of a constant buffer grown to its shader's declared size,
  and said a depth attachment holds both aspects": `SlowPath::ConstantBufferGrown`, and one sentence
  on `TextureViewDesc::stencil` (a combined-format view a pass draws into holds both aspects), with
  the plan's text in section 7 (Direct3D 11's constant buffers).

## Measurements

- Round 4 run 5. Machine: the desktop (Windows 10, RTX 3080, i9-9900K), through the queue, not
  alone except the load times (other tracks' jobs sharing it; the NIR track's OpenGL CTS run,
  a display-reset hazard job, held the queue 15:30 to 15:56 and 16:00 to 16:31). Times Pacific,
  2026-11-20. Scratch q5-final.sh r5z, logs/r5z-*; code of 099a8a07 for the library suite and
  of dbf1d5cc for the rest, the suite rerun at d9c373b9's code (logs/r5w-lib.log, 17:13 to
  17:21).
  - Lint of both crates and of the WebGL 2 build clean (r5w-lint at HEAD). Library: com 15
    pass, 1 ignored; programs 52 pass, 3 ignored. The WebGL 2 build's 10_0 cases: 5 pass, 1
    ignored (the Direct3D 10 device case, marked). Front 59, unit 5, cache 1 on the rented x86
    machine (96 processors, load about 1).
  - Wine's files and programs: as in Wine's section and the Gap list. Scenes through the
    library 135 pass, 0 fail, 4 absent; the WebGL 2 build's scenes 95 pass, 17 fail, 30 absent.
    Programs: the bar's three match real Direct3D, and atg-simplebezierpc,
    atg-simpleinstancingpc, godot-platformer-2d-d3d11 and sdk-tutorial02 and 04 to 07;
    atg-simplecomputepc and atg-simplemsaa_pc differ as before (frame 10: 8 pixels beyond, 4
    beyond their tiles' allowance). bgfx's were not run this run.
  - Module sizes (section 10), release builds 16:59 to 17:03 (r5z-sizes):
    `wcontainers_directx.dll` 14,275,072 bytes on WebGPU and 9,858,560 bytes on WebGL 2;
    `wcontainers_d3d12.dll` (the Direct3D 12 track's code as merged here) 13,732,352 bytes; the
    forwarders 1,024 bytes each.
  - Load time (section 10), alone, `LoadLibraryW` in 20 fresh processes each, medians
    (r5z-load, 17:03): the system's d3d11.dll 1,141 us; ours on WebGPU 1,622 us; ours on WebGL 2
    1,096 us; `wcontainers_d3d12.dll` 1,627 us.
  - Building ahead (plan 4.4; run-warm.sh, five programs run twice from one cache directory,
    17:03 to 17:07): Godot 3D's cold run built 124 ahead and drew none, 66 at their first draw;
    its warm start built its 61 recorded combinations at start and drew all 61, built 124 ahead
    and drew none, and 26 at their first draw. Unchanged.
  - Before building: `own_cases` beside the system's library and ours (scratch m5/out, 15:14):
    format support at 10_1, 10_0 and 11_0, multisampled depth reads at each level, the new
    clear values and depth `ClearView`.

- Round 4 run 4. Machine: the desktop (Windows 10, RTX 3080, i9-9900K), through the queue, not
  alone except the load times (other tracks' jobs sharing it; a display-reset hazard job of
  another track held the queue before the lint). Times Pacific, 2026-11-20. Code of 8ec7532e
  (scratch q3-final.sh r4h, logs/r4h-*, 13:32 to 14:24).
  - Lint of both crates and of the WebGL 2 build clean. Library: com 15 pass, 1 ignored;
    programs 49 pass, 3 ignored (13:33 to 13:42). The WebGL 2 build's 10_0 cases: 5 pass, the
    Direct3D 10 device case failing as before. Front 59, unit 5, cache 1 on the rented x86
    machine (96 processors, load about 33), at b592a3a6.
  - Wine's files and programs: as in Wine's section and the Gap list (the same tests and
    programs pass and fail as in round 4 run 3). Scenes through the library 135 pass, 0 fail, 4
    absent; the WebGL 2 build's scenes 95 pass, 17 fail, 30 absent.
  - Module sizes (section 10), release builds 14:08 to 14:14 (r4h-sizes):
    `wcontainers_directx.dll` 14,263,296 bytes on WebGPU and 9,846,784 bytes on WebGL 2;
    `wcontainers_d3d12.dll` (the Direct3D 12 track's code as merged here) 13,689,344 bytes; the
    forwarders 1,024 bytes each.
  - Load time (section 10), alone, `LoadLibraryW` in 20 fresh processes each, medians
    (r4h-load): the system's d3d11.dll 1,151 us; ours on WebGPU 1,637 us; ours on WebGL 2
    1,101 us; `wcontainers_d3d12.dll` 1,632 us.
  - Building ahead (plan 4.4; run-warm.sh, five programs run twice from one cache directory,
    14:15 to 14:24): Godot 3D's cold run built 125 ahead and drew none, 66 at their first draw;
    its warm start built its 61 recorded combinations at start and drew all 61, built 124 ahead
    and drew none, and 26 at their first draw. Unchanged.
  - The eighth check's probe beside the system's library and ours (probe8/out, 14:07) and the
    seventh check's (probe3/out): see Where things stand.

- Round 4 run 3. Machine: the desktop (Windows 10, RTX 3080, i9-9900K), through the queue, not
  alone except the load times (other tracks' jobs sharing it; display-reset hazard jobs of other
  tracks held the queue before some steps). Times Pacific, 2026-11-20. Code of 1aee4aa1 unless
  said (scratch q3-final.sh, logs/q3j-*).
  - Lint of both crates and of the WebGL 2 build clean (q3j, and q3k at e925f8f2's code).
  - Library: com 15 pass, 1 ignored; programs 48 pass, 3 ignored (q3j 10:27 to 10:33, q3k at
    e925f8f2's code 11:17 to 11:18). The WebGL 2 build's 10_0 cases: 5 pass, the Direct3D 10
    device case failing as before (q3k). Front 59, unit 5, cache 1 on the rented x86 machine (96
    processors, load about 6).
  - Wine's files: see Wine's section. Scenes through the library 135 pass, 0 fail, 4 absent; the
    WebGL 2 build's scenes 95 pass, 17 fail, 30 absent (q3j-scenes, q3j-webgl). Programs: the
    bar's three match real Direct3D, and atg-simplebezierpc, atg-simpleinstancingpc,
    godot-platformer-2d-d3d11 and sdk-tutorial02 and 04 to 07; bgfx's 01, 24 (run to its end),
    35, 37 and 38 pass (q3j-programs, q3j-bgfx); the three that differ are in the Gap list.
  - Module sizes (section 10), release builds 11:03 to 11:08 (q3j-sizes): `wcontainers_directx.dll`
    14,157,312 bytes on WebGPU and 9,747,968 bytes on WebGL 2; `wcontainers_d3d12.dll` (the
    Direct3D 12 track's code as merged here) 13,649,408 bytes; the forwarders 1,024 bytes each.
  - Load time (section 10), alone, `LoadLibraryW` in 20 fresh processes each, medians
    (q3j-load): the system's d3d11.dll 1,147 us; ours on WebGPU 1,635 us; ours on WebGL 2
    1,104 us; `wcontainers_d3d12.dll` 1,634 us.
  - Building ahead (plan 4.4; run-warm.sh, five programs run twice from one cache directory,
    q3j-warm, 11:09 to 11:14): Godot 3D's warm start built its 61 recorded combinations at start
    and drew all 61, built 124 ahead at shaders' arrival and drew none, and built 26 at their
    first draw. Godot 2D built 1 at start and drew it. Unchanged.
  - Real Direct3D 11 alone (no library of ours), bgfx-26-occlusion and atg-simplemsaa_pc twice each
    against their references (q3j-sysruns, 11:14): both pass both times (Gap list).

- Round 4 run 2. Machine: the desktop (Windows 10, RTX 3080, i9-9900K), through the queue, not
  alone except the load times (other tracks' jobs sharing it; display-reset hazard jobs of other
  tracks held the queue before some steps). Times Pacific, 2026-11-20.
  - Module sizes (section 10; the brief's 2026-11-20 ruling: the two separate modules measured
    today), release builds of 72d27585's code plus the run's uncommitted 1D refusal fix, 09:13
    to 09:16 (logs/q2h-sizes.log): `wcontainers_directx.dll` 14,103,552 bytes on WebGPU and
    9,697,280 bytes on WebGL 2; the Direct3D 12 track's `wcontainers_d3d12.dll`, built from its
    run 52 code as merged here, 13,648,384 bytes; the forwarders 1,024 bytes each. The builds of
    the bar's step at 08:25 to 08:29 (logs/q2f-sizes.log) gave 14,104,064 and 9,697,280.
  - Load time (section 10), alone, 09:16 to 09:18 (logs/q2h-load.log), `LoadLibraryW` in 20 fresh
    processes each, medians: the system's d3d11.dll 1,169 us (19 samples read); ours on WebGPU
    1,650 us; ours on WebGL 2 1,110 us; `wcontainers_d3d12.dll` 1,637 us.
  - Building ahead (plan 4.4; run-warm.sh, five programs run twice from one cache directory,
    08:29 to 08:36, logs/q2f-warm.log): Godot 3D's warm start built its 61 recorded combinations
    at start and drew all 61, built 124 ahead at shaders' arrival and drew none, and built 26 at
    their first draw. Godot 2D built 3 at start, drew 3, and 10 at their first draw. Unchanged
    by the input layout ids. The diagnostic run (08:40 to 09:05, Godot 3D alone,
    logs/q2diag-warm.log): of 124 pairs built ahead, 122 never drawn, 2 drawn with other state.
  - The whole bar (logs/q2f-*, 07:50 to 08:36): library com 14 pass and 1 fail (the 1D refusal
    of two-pixel formats, fixed after it; programs.rs did not run in that step); Wine's d3d11
    146 of 166 (120.2 s), d3d10core 110 of 111; scenes 135 pass, 0 fail, 4 absent; the WebGL 2
    build's scenes 95 pass, 17 fail, 30 absent; the programs and bgfx's as in the Gap list.
    After the fix (logs/q2h-*, 09:07 to 09:13): lint of both crates and of the WebGL 2 build
    clean; library com 15 pass, 1 ignored; programs 47 pass, 2 ignored; the WebGL 2 build's 10_0
    cases 5 pass, the Direct3D 10 device case failing as before (Known failures). Front 58,
    unit 5, cache 1 on the rented x86 machine (96 processors, load about 7).
  - The fifth check's claims probe (09:13, scratch probe2/out): compared with its own script,
    every creation the RTX 3080 refuses is refused, no device is lost, and no format claims what
    creation then refuses but the cubes of depth formats and the NV12 and YUY2 views without a
    description, which the RTX 3080 claims and refuses alike; extended-range cubes, the one new
    difference it showed, are refused since 626dde80.
- Round 3 run 1. Machine: the desktop (Windows 10, RTX 3080, i9-9900K), through the queue, not
  alone (other tracks' jobs sharing it; display-reset hazard jobs of other tracks held the queue
  before some steps). Times Pacific, 2026-11-20.
  - Module sizes (section 10), release builds of 853c98a2's code, 05:10 to 05:27
    (logs/q1k-sizes.log): wcontainers_directx.dll 14,097,408 bytes on WebGPU (8,192 more than
    round 2 run 12) and 9,679,872 bytes on WebGL 2 (9,216 more); the forwarders 1,024 bytes each.
  - Building ahead (plan 4.4; run-warm.sh, the five programs run twice from one cache directory,
    05:27 to 05:34, logs/q1k-warm.log): Godot 3D's warm start built its 61 recorded combinations
    at start and drew all 61, built 124 ahead at shaders' arrival and drew none of them, and built
    26 at their first draw. Godot 2D built 3 at start, drew 3, and 10 at their first draw.
    Unchanged since round 2 run 9.
  - The whole bar (logs/q1k-*, 04:23 to 05:34): library com 15, programs 45 with 2 ignored
    (57.5 s); Wine's d3d11 146 of 166 (126.3 s), d3d10core 110 of 111 (73.7 s); scenes 135 pass,
    0 fail, 4 absent; the WebGL 2 build's scenes 95 pass, 17 fail, 30 absent; programs and bgfx's
    as in the Gap list.
  - Front 57, unit 5, cache 1 and the interface's 36 on the rented x86 machine (192 processors,
    load 6.5 when it ended), 04:15.
- Load time (section 10), round 2 run 8, alone (logs/r8g-load.log), 20 fresh processes each,
  `LoadLibraryW` medians: the system's d3d11.dll 1,166 us; ours on WebGPU 1,675 us; ours on WebGL 2
  1,111 us; the Direct3D 12 track's `wcontainers_d3d12.dll` 1,677 us. Nothing of loading changed
  since; the joined module of section 10 is the joint step (Gap list).

Earlier runs' measurements are in this file's git history.

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

Open asks only; the answered ones are in this file's git history.
- Interface track, with the WebGPU, WebGL 2 and shaders tracks in the same step: a
  `VertexFormat` for `B8G8R8X8_UNORM` reading blue, green and red with 1 in the fourth component
  (measured on the RTX 3080 in round 3 run 1; the Direct3D 12 track asks the same), whose arms
  the back ends' and the shaders crate's exhaustive matches need; a `Caps` feature for cube
  arrays, declared by the WebGPU back end at the same time (this front probes for them
  meanwhile, Departures round 2 run 12). The `R10G10B10_XR_BIAS_A2_UNORM` format is no longer
  asked: this front holds it in `Rgb10a2Unorm`'s bits.
- Infrastructure crate (the Direct3D 12 track, G5): swap chains of `R10G10B10_XR_BIAS_A2_UNORM`,
  presented with the extended-range conversion, and back buffer views of it as
  `R10G10B10A2_UNORM`.
- WebGPU track: `B5G6R5_UNORM` and `B5G5R5A1_UNORM` as render targets, blended at the format's
  precision, through the back end's converted formats; the uniform arena admitting uniform
  buffers whose other usages lie within the arena's own (`create_buffer` admits only usage
  exactly `UNIFORM`, which writing through the interface never is; both halves of the listed
  case make a bind group per draw, round 4 run 2); `B8G8R8X8_UNORM`'s X byte kept as given and
  written 0 by clears and draws, as the RTX 3080 does.
- WebGL 2 track: the 10_0 format abilities listed in the Gap list's WebGL 2 item (signed
  normalised targets, the two-pixel, B5G5R5A1, B4G4R4A4 and A8 textures).
- Infrastructure crate (the Direct3D 12 track): `D3D11_FORMAT_SUPPORT2_SHAREABLE` in its
  `support2` list (this front names it once in `desc::SUPPORT2_SHAREABLE` meanwhile).
- Interface and shaders tracks: a helper value giving a program a view's width other than its
  texture's (YUY2 views, whose `GetDimensions` reports pixels on the RTX 3080).
- Shaders track: a storage form of `A8Unorm` (alpha in a wider format's fourth component), so
  `A8_UNORM` storage views can be served.
- Shaders track: blending into integer twins (`TargetInShader::combine`, refused by name
  today);
  atg-simplecomputepc's division (every pixel inside the Mandelbrot set just below 4 where
  Direct3D gives 4, inferred in run 17); the geometry replay's streams past the first and drawing
  a stream past 0; an aliased-line route for multisampled targets (with the interface track); .
- WebGL 2 track: copies from a depth texture into a colour texture of its group stored as a half
  float (`R16_TYPELESS` as `R16Float`) keep every bit pattern (Wine's test_format_compatibility).
- WebGL 2 track: mips of whole 3D views as Direct3D generates them (its `glGenerateMipmap` route
  for 3D textures differs from the interface's default, which the WebGPU build passes Wine's
  test_generate_mips with; round 4 run 5).
- Rig track: a way to run Wine's dxgi file with its Direct3D 12 tests kept off the GPU; the scene
  programs reporting every `--dll` library they loaded.

## Literals

- Round 4 run 5: 13 lines of wcontainers-direct3d-11 left the baseline (the sampler's filter
  bits, address modes and Direct3D's anisotropy limit named once in `desc`, the empty slot's
  sampler from them); new code adds none and makes no mark. wcontainers-direct3d-11 499 lines,
  wcontainers-directx 229 (728). The check exits 0 at HEAD (logs/r5x-literals.log).

- Round 4 run 4: 15 lines of wcontainers-direct3d-11 left the baseline (the clear's channel
  table named once as `RGB10A2_BITS` and `RG11B10_BITS` and its sizes derived, the clear's
  messages named once); new code adds none and makes no mark. wcontainers-direct3d-11 512 lines,
  wcontainers-directx 229 (741). The check exits 0 at 8ec7532e (logs/r4g-literals.log).
- Round 4 run 3: 10 lines of wcontainers-directx left the baseline (the open calls' names now
  served, the resource interfaces' vtable places named once); new code adds none and makes no
  mark. wcontainers-direct3d-11 527 lines, wcontainers-directx 229 (756). The check exits 0 at
  1aee4aa1 (logs/q3i-literals.log) and at the run's last commit (logs/q3m-literals.log).
- Round 4 run 2: 15 lines of wcontainers-direct3d-11 left the baseline (the slot and element
  counts of input layouts named once, the GDI formats and feature levels by name, the blend
  targets from `MAX_COLOR_TARGETS`, the mip count from `u32::BITS`); new code adds none and makes
  no mark. wcontainers-direct3d-11 527 lines, wcontainers-directx 239 (766). The check exits 0
  at 72d27585 (logs/q2e-literals.log).
- Round 3 run 1: the integer twins' maxima in the interface's `norm_twin` are derived
  (`u16::MAX`, `i16::MAX`, `i8::MAX`), so three baseline lines left; the new code and tests add
  none and make no mark. wcontainers-direct3d-11 542 lines, wcontainers-directx 239 (781). The
  check exits 0 at ef6afe7d (logs/q1n-literals.log).
- Left, as recorded before (git history of this section): the export names in build.rs (a build
  script cannot import the crate it builds), and the rest of the fix list (orchestrator
  magic-literals/fix-graphics.md), a part per run.

## Licences

Round 4 runs 3 and 4 added no dependency. Run 4 added on the desktop the eighth check's probe,
built in C:\wc\out\phase-2-graphics-direct3d-11\probe8 (its build folder deleted at the end of
the run). On the desktop it added the seventh check's probe, copied from
scratch probe3 and built in C:\wc\out\phase-2-graphics-direct3d-11\probe3 (its build folder is
deleted at the end of the run).

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
rustup's x86_64-pc-windows-msvc standard library for 1.99.0 (G4 run 4).
