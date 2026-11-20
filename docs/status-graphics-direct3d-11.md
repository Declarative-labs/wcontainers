# Direct3D 11 front status

Branch: phase-2-graphics-direct3d-11. Plan: docs/phase-2-graphics.md, milestone G4 (section 12).
Runs so far: six on G4a, the first slice; runs 1 to 25 of G4 (2026-11-17 and 18); round 2, runs
1 to 12 (2026-11-19 and 20). This file keeps the current state only; each earlier run's own record is in
its git history (`git log -p docs/status-graphics-direct3d-11.md`).

## Where things stand

G4 is not complete. Of the independent check's two blocking findings left after run 9, the
WebGL 2 one is closed in round 2 run 12: the WebGL 2 build reports 10_0 and the bar's WebGL 2
cases match real Direct3D through it. The other, section 10's one family library, waits on the
Direct3D 12 track moving its COM objects out of its examples (its Next 4; it has not answered on
the order). This front's own open work on the bar: the format shortfalls named as departures,
and `NormAsInteger` (16-bit normalised formats where the back end lacks them).

Round 2, run 12 (2026-11-20, about 02:00 to 04:00 Pacific):
- Merged origin/phase-2-graphics-shaders (its run 58: stream output of a vertex shader alone on
  the replay route, `GeometryReplay::streamed`, with the interface commit widening
  `StreamOutAfterReplay`) and origin/phase-2-graphics-direct3d-12 (nothing past the shaders
  merge); no conflict.
- A vertex shader's stream output on a back end without compute programs now takes
  `GeometryReplay::streamed` (`make_replayed`); the refusal `STREAM_OUT_WITHOUT_COMPUTE` is gone.
  The feature level check lets transform feedback stand for compute below 11_0 and needs cube
  arrays from 10_1, which the device learns by making a texture of two cubes (Departures), so
  the WebGL 2 build reports 10_0, as plan section 9 fixes it. New tests: front.rs's level test
  (transform feedback without compute gives 10_1, without cube arrays 10_0; the recorder's
  `cube_arrays`), and programs.rs
  `stream_output_of_a_vertex_shader_alone_at_10_0_as_direct3d_does` (example own_level10
  `stream`), which matches real Direct3D on both builds.
- The cube-face count is the named `CUBE_FACES` in device.rs; ten literal lines left the
  baseline.
- The whole bar at 32c76f17 (logs/r12e-*, 02:50 to 03:40 Pacific) and the WebGL 2 build's
  (logs/r12b-*, r12c-*, r12d-*, 02:09 to 02:41): see Wine's files, the Gap list and
  Measurements.

## Wine's Direct3D 11 tests through ours

Files passing (the bar: real Direct3D's verdict for every test, except the gap list), round 2
run 12, on the RTX 3080 through the queue, not alone, rigfix/wine.sh:
- d3d11: 146 pass of 166 (logs/r12e-wine.log, 124.4 s). The 20 failing are in the Gap list.
- d3d10core: 110 of 111 (logs/r12e-wine10.log, 76 s): test_desktop_window (Gap list).
- d3d10core through the WebGL 2 build (10_0): 98 of 111 (logs/r12d-wine10gl.log, 146.8 s); the
  13 failing are under the WebGL 2 item of the Gap list. test_stream_output now passes there.
- d3d11 through the WebGL 2 build: 78 of 166 (logs/r12d-wine11gl.log, 180.7 s), not on the bar.
- dxgi (not on G4's bar): 4 of the 17 tests without a Direct3D 12 part at run 8 (Gap list).
Each earlier run's counts are in this file's git history.

## Gap list (the pass bar's written list, round 2 run 12)

Everything on G4's pass bar that does not give real Direct3D's verdict on the RTX 3080, each item
once, with its cause and whose work it is. The bar allows a gap only where a named absent feature
stands behind it; an ability Microsoft's tables require at the reported feature level is not an
absence, so such shortfalls are named as departures (Departures, round 2 run 9) and keep the bar
from being met. Nothing is excluded for convenience. The rig's exclusion of d3d11
test_create_device applies to real Direct3D's run too: its child process makes a swap chain with
`Windowed = FALSE`, which would take the owner's screen (tools/graphics-rig/wine-excluded.txt).

Not met, this track's own work and others':
- The WebGL 2 cases are met since round 2 run 12: the WebGL 2 build reports 10_0, and the four
  `own_level10` cases (per-target blends without OES_draw_buffers_indexed, `resinfo`, an empty
  slot's `resinfo`, and a vertex shader's stream output without a geometry shader, streamed and
  drawn over two draws) match real Direct3D through it (logs/r12c-webgllib.log,
  r12d-webgllib.log). Stream output from a vertex shader alone takes the shaders crate's
  `GeometryReplay::streamed` (`StreamOutAfterReplay`); geometry shaders take `GeometryAsReplay`,
  typed buffers `TexelBufferAsTexture`. What the WebGL 2 build still does not give real
  Direct3D's verdict on, measured at round 2 run 12 (Wine's d3d10core 98 of 111 through it,
  logs/r12d-wine10gl.log; the rig's scenes 95 pass, 17 fail, 30 absent, logs/r12d-webgl.log,
  the 17 being the missing pictures of the three scenes named below), each with its owner:
  - A sample mask leaving out samples (WebGL 2 has no `gl_SampleMask`; the shaders crate's
    `SAMPLE_MASK` cannot serve it): refused by name, removing the device. Wine
    test_sample_mask, test_multisample_resolve; the rig's msaa scene. No route exists; the plan
    (section 9) fixes 10_0 on WebGL 2 without naming this loss.
  - Multisampled textures read by shaders (absent on WebGL 2, plan section 6): Wine
    test_unbound_multisample_texture.
  - Depth clamping, clip distances and a depth bias in the program without a pixel shader are
    served since round 2 run 11 (the shaders crate's `builtin::pixel_program_for_variant`;
    Wine test_depth_clip passes through the WebGL 2 build). Left: a depth bias clamp on float depth,
    refused by name: Direct3D's float step follows each primitive's largest depth exponent,
    which a pixel program does not see (the shaders crate's `DEPTH_BIAS` takes one fixed
    step). Wine test_depth_bias.
  - Cull distances (`CullDistanceAsGeometry` needs compute; WEBGL_clip_cull_distance): Wine
    test_combined_clip_and_cull_distances.
  - Views in another format of the group (`Features::TEXTURE_VIEWS`, the WebGL 2 back end's):
    Wine test_create_shader_resource_view, the rig's views scene.
  - Formats the WebGL 2 back end lacks: `R16G16_SNORM` and the other 16-bit normalised formats
    (this front's `NormAsInteger` item), `A8_UNORM`: Wine test_create_texture2d, test_texture,
    test_render_a8, test_clear_render_target_view_2d (48 fewer checks).
  - test_format_compatibility (16 checks, all of its test 27: a 16-bit depth texture copied
    whole into an `R16_TYPELESS` colour texture). Cause, read from the log in round 2 run 11:
    every 16-bit word at or above 0x8000 arrives as 0x7fff, the rest exact. The front stores
    `R16_TYPELESS` colour textures as `R16Float`, and the WebGL 2 back end copies depth into
    colour by a draw (`DepthWrittenByDraw`) writing the bits as a half float, whose NaN
    patterns (0xff00, 0xffff) the GPU makes its one NaN, 0x7fff. The WebGL 2 track's copy
    route (the WebGPU build copies through a buffer and passes).
  - test_generate_mips (56 checks): all on 3D textures (resource type 3), on tests 0 to 3 that
    expect no mips as well as 6 to 8 that do: level 1 reads 0x48484848 after it was written
    with zeros, so the write or the read back of a 3D texture's second level goes wrong, not
    the mip generation. 2D arrays pass, and the same calls pass through the WebGPU build. Not
    yet proven whose: the WebGL 2 back end's 3D writes (`tex_sub_image_3d` with the unpack
    image height set) look right when read.
  - Wireframe without WEBGL_polygon_mode (`FillModeConverted`, not built for any back end): the
    rig's raster scene. Pixel shaders reading `SV_PrimitiveID` without a geometry shader
    (`PrimitiveIdAsGeometry` needs compute): the rig's geometry scene.
  - Wine's d3d11 file through the WebGL 2 build (not on the bar, whose d3d11 file is the WebGPU
    build's): 78 of 166 (logs/r12d-wine11gl.log); 57 of the failing run fewer checks than real
    Direct3D because they skip below 11_0, the rest are the absences above.
- Section 10's one family library. Ours (`wcontainers_directx.dll`) carries Direct3D 10 and 11
  and DXGI; the Direct3D 12 track's `wcontainers_d3d12.dll` (crates/wcontainers-direct3d-12/
  examples/directx) carries Direct3D 12 and a DXGI of its own. Section 10 has one library, built
  by this crate, depending on both fronts, with one DXGI. Takes: this crate depending on the
  Direct3D 12 front, exporting `D3D12CreateDevice`, and the two DXGI shells joined into this
  crate's, agreed with that track first; then its size and load time measured. Until then the two
  modules are measured each alone (Measurements), which is not the measurement the plan asks.
  Waits on the Direct3D 12 track: its COM objects (about 9,400 lines) are in its crate's
  examples, which this crate cannot depend on, and its status plans to move them into
  `wcontainers-directx` itself (its Next 4); asked since round 2 run 9; at round 2 run 12 its status still lists the move as its own Next 4.

Shortfalls of the reported 11_0 against Microsoft's format tables (Departures, round 2 run 9),
with the test that shows each:
- `B5G6R5_UNORM` and `B5G5R5A1_UNORM` targets (render target, blending, mip generation): the
  WebGPU back end has neither format as a target. Wine d3d11 and d3d10core test_format_support
  (`Format 0x55 - blendable not supported` at each level, 6 checks).
- `R10G10B10_XR_BIAS_A2_UNORM` (textures, staging, display): the interface has no such format.
  test_format_support (1 check at 11_0), test_swapchain_formats (1 check).
- `B8G8R8X8_UNORM` vertex elements and typed buffers: the interface has no vertex format giving
  the X byte as 1 (Asked of the interface track). test_vertex_formats runs 98 checks of real
  Direct3D's 100 (it skips the format's two where the support bit is off) and fails none.
- Back buffer views of `R10G10B10A2_UNORM` in other formats of its group: this front's. (3D
  textures of block-compressed formats and 1D and 3D textures of the packed pairs are reported
  and made since run 10.)
- NV12's and YUY2's storage views and gathers, NV12's blending, and `420_OPAQUE` textures: this
  front's (plan section 9 says `420_OPAQUE` is built from ordinary textures; it is not).
- Multisample counts per format and the video formats' video abilities: the plan's two named
  shortfalls (section 9).
All of these are compared in `format_support_as_direct3d_reports_it_but_the_named_shortfalls`.

Wine's d3d11 file (20 of 166 failing at round 2 run 12, logs/r12e-wine.log; the same 20 since run 8). Every failing test but test_format_support and
test_swapchain_formats (shortfalls above) sits behind an absence:
- The 11_1 device and Wine's WARP fallback: `D3D11CreateDevice` with 11_1 alone fails, logic
  operations have no route (plan section 9), and this library offers no WARP adapter:
  test_state_refcounting, test_logic_op, test_create_shader.
- Debug devices (`D3D11_CREATE_DEVICE_DEBUG` fails) and the 11_1 device: test_device_interfaces.
- Min/max filtering (`D3D11_OPTIONS1.MinMaxFiltering` FALSE): test_filter_minmax.
- Video (`QueryInterface` for `ID3D11VideoDevice` and `ID3D11VideoContext` answers
  `E_NOINTERFACE`): test_immediate_context, test_video_support, test_h264_decoder.
- Shared resources (keyed mutexes and NT handles refused at creation, a shared resource's
  `GetSharedHandle` not served): test_keyed_mutex, test_create_buffer (test 36),
  test_texture1d/2d/3d_interfaces (their keyed-mutex entries), test_shared_resource (12 checks).
- Mapping default buffers (`D3D11_OPTIONS1.MapOnDefaultBuffers` FALSE, plan section 9):
  test_resource_access (6 checks at 11_0) and the 11_1 device.
- A vertex shader's layer and viewport (`D3D11_OPTIONS3.VPAndRTArrayIndexFromAnyShaderFeedingRasterizer`
  FALSE): test_layered_rendering skips that part (2 checks); its other 72 pass.
- Presenting to another process's window (the swap chain refusal above): test_desktop_window
  runs 23 checks of real Direct3D's 28 and fails none. Real DXGI on the RTX 3080 makes that swap
  chain; Wine's test accepts the refusal as `broken`.

Wine's d3d10core file (1 of 111 failing, logs/r12e-wine10.log): test_desktop_window, as above.

Wine's dxgi file through our dxgi.dll (not on G4's bar; run since run 7 because the check on run 5
asked; scratch run-dxgi.sh, the 17 tests without a Direct3D 12 part). Passing at run 8:
test_create_factory, test_private_data, test_object_wrapping,
test_factory_check_feature_support. The others, all this library's DXGI shell unless named:
absent, a second adapter and `IDXGIAdapter3` (test_adapter_luid, test_output_desc,
test_query_video_memory_info, test_video_memory_budget_notification: fewer checks, none
failing); absent, WARP (test_enum_warp_adapter, which then crashes on the null adapter); not
served, `IDXGIDevice::CreateSurface` (test_create_surface and test_parents crash on the null
surface after `E_NOTIMPL`) and `IDXGIResource1::CreateSubresourceSurface`
(test_subresource_surface); not served, `CheckInterfaceSupport` (test_check_interface_support, 6
checks); crashes on a null out pointer through the windows crate's form of a method returning a
value (`GetDesc`, `EnumOutputs`, `GetMaximumFrameLatency`: test_adapter_desc, test_output,
test_maximum_frame_latency); fewer display modes (the infrastructure crate's `OutputDesc`:
test_find_closest_matching_mode runs 67 checks of 2055 and fails none).

Real programs (logs/r12e-programs.log and r12e-bgfx.log, round 2 run 12: the three programs of the pass bar, godot-platformer-3d-d3d11, chocolate-doom and atg-simplelightingpc, match real Direct3D on every compared frame, and so do atg-simplebezierpc, atg-simpleinstancingpc, godot-platformer-2d-d3d11 and sdk-tutorial02 and 04 to 07; bgfx's six as before, five passing or running to their end and bgfx-26-occlusion differing in its overlay (Departures, run 16)). The two that differ, outside the bar's three:
- atg-simplecomputepc: every pixel inside the Mandelbrot set differs (154,515 pixels); the
  converted compute program's division gives just below 4 where real Direct3D gives 4 (inferred,
  run 17). The shaders track's (Asked).
- atg-simplemsaa_pc: frame 10 has 8 pixels beyond the channel tolerance (largest 11); frames 60
  and 120 pass. 6 of the 8 are lighter than every one of 10 real runs, which looks like one sample
  of four covered differently at an edge; the cause is not proven (round 2 run 5).

Listed cases of section 12, each with its test against real Direct3D (Tests):
- The WebGL 2 cases: met since round 2 run 12 (above).
- Not met in the browser's configuration: a compute shader storing to `R16G16_UNORM` typed views
  on a device without texture-formats-tier1 and without wgpu's 16-bit-norm feature.
  `typed_storage_stores_without_texture_formats_tier1_or_16_bit_norm_as_direct3d_does` fails
  (the texture is refused, E_NOTIMPL) and is ignored as a known failure. Takes: this front
  storing 16-bit normalised textures as their integer twins (`NormAsInteger`) and reading them
  through the shaders crate's `decoded` variant, which that track built in its run 54 (merged this
  run, not yet used here). With only the adapter-specific format features off, the tier1 test
  (`R11G11B10_FLOAT` through `StorageFormatThroughBuffer`) matches real Direct3D.
- Failing: aliased lines (`MultisampleEnable` FALSE) into a multisampled target.
  `aliased_lines_into_a_multisampled_target_hit_every_sample_as_direct3d_does` is ignored as a
  known failure: real Direct3D covers every sample of the pixels the diamond rule picks
  (specification 15.14); WebGPU draws lines into a multisampled target by its own rule. The
  interface has no aliased-line route; the fix is a stage helper route drawing each line as the
  whole pixels the rule picks (the shaders track and the interface), which this front then takes.
- Constant buffers updated per draw: the drawing matches real Direct3D (`own_handled`), and the
  counts (no binding set made per draw, no pass split) are tested on the recording back end only
  (front.rs `constants_rewritten_between_draws_make_no_binding_set_and_split_no_pass`), not
  through the WebGPU back end's counters as the bar says.
- Every other listed case passes against real Direct3D on WebGPU, among them alpha-antialiased
  lines (within the allowance of 64, largest 43), `ClearView`, `SetResourceMinLOD` read by
  compute shaders and frame latency waitable objects.

Differences from the RTX 3080 that Wine's verdicts do not show:
- `ClearView` of rectangles of a storage view is refused and removes the device (warned), and
  `D3D11_OPTIONS.ClearView` answers FALSE; real Direct3D clears them and answers TRUE (the check's
  probe `clearview_uav`). Takes: the shaders crate's storage clear program (`clear_draw`), which
  the fronts do not dispatch yet.
- d3d11 test_clear_buffer_unordered_access_view passes through ours, where real Direct3D fails 223
  of its 934 checks: ours clears by the documented rule of `ClearUnorderedAccessViewUint`;
  NVIDIA's driver does not. Kept, because the documented rule is what programs are written for.
- `IDXGISwapChain::GetContainingOutput` answers `E_NOTIMPL` (the infrastructure crate's swap
  chain). atg-simplelightingpc copes.
- `ID3D11Device3::ReadFromSubresource` and `WriteToSubresource` are not served: they warn and
  remove the device. No valid call reaches them, as `D3D11_OPTIONS2.MapOnDefaultTextures` is
  FALSE.
- Drawing a geometry stream past 0 is refused by name (`GEOMETRY_STREAM_DRAWN_PAST_ZERO`; the
  stage helper draws stream 0 alone, the shaders track's). No program or Wine test seen does it.
- Building ahead at a shader's arrival (plan 4.4) has never been hit: on Godot 3D every run
  builds 54 to 124 combinations ahead and draws none of them, so it costs background builds with
  no measured benefit. The recorded combinations built at start are all drawn.

## Known failures

- `aliased_lines_into_a_multisampled_target_hit_every_sample_as_direct3d_does` and
  `typed_storage_stores_without_texture_formats_tier1_or_16_bit_norm_as_direct3d_does` (library,
  programs.rs) fail on every build and are marked `#[ignore]` with the reason (Gap list); run
  them with `--include-ignored`.
- On the WebGL 2 build only: the library tests that ask for 11_0 or 10_1 (shader model 5
  bytecode, compute, tessellation, or the default levels compared with real Direct3D's 11_x) fail,
  as the build reports 10_0, or reach the absences of the Gap list's WebGL 2 item: com.rs
  `unserved_shaders_are_refused_not_faked` and 29 of programs.rs at round 2 run 12
  (logs/r12b-webgllib.log, r12c-webgllib.log; 12 pass, 1 ignored). They are not marked in the
  code: the same tests pass on WebGPU.

## What comes next

In order:
1. 16-bit normalised textures as integer twins (`NormAsInteger`, plan 4.12: `decoded` for
   reads, `targets_in_shader` for writes, `BlendInShader` for blends, clears and copies by the
   bits), then un-ignore the 16-bit-norm test; it also serves WebGL 2's 16-bit formats (Wine
   test_create_texture2d, test_texture, test_clear_render_target_view_2d on that build).
2. Section 10's family library, once the Direct3D 12 track moves its COM objects into
   `wcontainers-directx` (its Next 4) or agrees that this track does: one DXGI shell, export
   `D3D12CreateDevice`, measure the joined module's size and load time.
3. Replace the cube array probe (`makes_cube_arrays`, Departures) with a `Caps` feature once the
   interface names one and both back ends declare it (Asked).
4. Prove whose the 3D second-level failure of test_generate_mips on the WebGL 2 build is (a
   small write and read back of a 3D texture's level 1 on the WebGL 2 back end alone).
5. The format shortfalls that are this front's own: `420_OPAQUE` (the Direct3D 12 track stored
   it as ordinary textures in its round's run 50; read how), NV12's and YUY2's storage views and
   gathers, back buffer views in the group; remove each from the test's named differences as it
   is served.
6. Constant buffer counts through the WebGPU back end's counters (its frame report).
7. `ClearView` of storage views' rectangles through the shaders crate's storage clear program.
8. Building ahead at a shader's arrival: read why the guessed states differ from the drawn ones,
   or stop building ahead at arrival.
9. Wine's dxgi file: the null-out-pointer crashes, `CreateSurface`, `CreateSubresourceSurface`.
10. The literal fix list, a part per run.

## How to run

Everything builds and runs on the desktop through the rig (docs/status-graphics-rig.md, "How to
run"); the Mac runs only formatting, git, and checks of the front crate on the rented x86
machine. Scripts named below are in scratch (pyro-parked/wcontainers-scratch/graphics-direct3d-11);
logs go to its logs/. Do not send this checkout's tree (remote, gpu-job, scenes) while another of
its jobs waits in the queue: the waiting job then stops ("holds tree ..., not the sent ..."). Never
run `tools/graphics-rig/gpu-job --help` (it queues `--help` as a command). The desktop's checkout
needs the shaders crate's submodules, so commands start with `git submodule update --init
--recursive -q`.

- The whole bar, then release module sizes and building-ahead hits: `run-g4t1.sh <label>` (library
  tests, Wine's d3d11 and d3d10core files, every scene through the library, the WebGL 2 build's
  scenes, the real programs and bgfx's, sizes, then run-warm.sh). Its steps one by one:
  `run-g4s13.sh <label> [literals lint front lib wine wine10 scenes webgl programs]`.
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
- Load time of a module: example `load_time` (scratch run-r8-load.sh).
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

At round 2 run 12: front.rs 55, tests/cache.rs 1, unit 5 (wcontainers-direct3d-11, recording
back end); com.rs 15 and programs.rs 46 (wcontainers-directx, real devices), of which 2 are
ignored as known failures (Known failures). New this run: programs.rs
`stream_output_of_a_vertex_shader_alone_at_10_0_as_direct3d_does`; front.rs's level test takes
transform feedback without compute and a back end without cube arrays. Every listed case of section 12 has its test against real
Direct3D on WebGPU; the constant buffer counts are tested on the recording back end only, and
the WebGL 2 cases fail on the WebGL 2 build (Gap list). programs.rs runs each example once beside
the system's d3d11.dll and once beside ours and compares what each printed; the random lifetimes
comparison is seeded (`WC_LIFETIMES`, default seeds 1 to 8 at 300 steps each).

## Departures from the plan

- Round 2 run 9: plan section 9 says every format ability Microsoft's tables require at the
  reported level is served, but for two named shortfalls (multisample counts per format; the
  video formats' video abilities). The device reports 11_0 on WebGPU with these further
  shortfalls against the 11_0 table (format-support-for-direct3d-11-0-feature-level-hardware.md,
  saved in scratch graphics/downloads/msdocs), each compared with the RTX 3080 in
  `format_support_as_direct3d_reports_it_but_the_named_shortfalls`:
  - `B5G6R5_UNORM` and `B5G5R5A1_UNORM` as render targets and blended targets (and so no mip
    generation): the WebGPU back end has neither as a target (the WebGPU track's);
  - `R10G10B10_XR_BIAS_A2_UNORM` altogether (Texture2D, CPU lockable, display, casting): the
    interface has no such format (the interface track's);
  - `B8G8R8X8_UNORM` as a vertex element and typed buffer: the interface has no vertex format
    giving X as 1 (the interface track's, Asked);
  - back buffer views of `R10G10B10A2_UNORM` in other formats of its group, NV12's and YUY2's storage views and
    gathers, NV12's blending, and `420_OPAQUE` textures: this front's.
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
- Round 2 run 4, superseded in round 2 run 12: the WebGL 2 build reported 9_3 while stream output
  without a geometry shader had no route without compute; the shaders crate's replay serves it
  since its run 58, and the build reports 10_0.
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
- Round 2 run 3: G4 is reported complete again for the independent check, with the items the
  Gap list gives to other tracks still failing (the round's task: do not spend further runs
  waiting on other tracks).

- Round 2 run 1: G4 is reported complete with items of this front still on the gap list (named
  there as this front's, not as gaps), and with items waiting on the shaders, interface and
  infrastructure tracks, as this round's task asked, so that the independent check runs on the
  front as it stands. The plan's bar counts only absent features as gaps; those items are work.

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
- d3d11.dll, dxgi.dll and d3d10core.dll are forwarders; d3d12.dll is not (section 10 names it):
  the library exports no Direct3D 12 entry point. The Direct3D 12 track has had its own Windows
  library of DXGI and Direct3D 12 since 2026-11-17 (crates/wcontainers-direct3d-12/examples/
  directx, `wcontainers_d3d12.dll`), so there are two DXGI shells where section 10 has one family
  library. Joining them here is not done (round 2 run 2 wrote that no such shell existed, which
  was wrong by run 7). Run 8 measured both modules' size and load time instead (Measurements).
- `D3D11_OPTIONS` answers `ClearView` FALSE, where section 9 says the front reports it: `ClearView`
  serves render targets with rectangles and whole storage views (run 8), not rectangles of
  storage views or video views. `DiscardAPIsSeenByDriver` is FALSE: discards keep the contents,
  so no driver sees them. `ConstantBufferPartialUpdate`, `ConstantBufferOffsetting` and both
  `MapNoOverwriteOnDynamic...` answers are TRUE since run 8.
- `CreateDXGIFactory` gives DXGI 1.0's factory (no `IDXGIFactory1`) only when asked for
  `IUnknown`, `IDXGIObject` or `IDXGIFactory`, refuses `IDXGIFactory1`, and gives the whole
  factory when asked for a later interface (run 8). Microsoft's page says only that 1.0 and 1.1
  are not mixed; Wine's test_create_factory shows Windows answering `IDXGIFactory2` from
  `CreateDXGIFactory`, and this follows it.
- The library draws through wgpu on Direct3D 12 only (section 10 allows Direct3D 12 or Vulkan): a
  machine without Direct3D 12 gets `DXGI_ERROR_UNSUPPORTED` from `D3D11CreateDevice`.
- (Round 2 run 2: superseded.) Lines under `MultisampleEnable` are drawn by the back end's rule
  as plan 4.12 says; `AntialiasedLineEnable` applies only without `MultisampleEnable`, as
  `D3D11_RASTERIZER_DESC` documents. Earlier text: lines under either were refused at the draw on
  any target. Microsoft's pages disagree on whether these apply to single-sample targets; this follows the
  reading that they always pick the quadrilateral and alpha rules (as DXVK does). Not yet measured
  against real Direct3D.


## Interface changes

None in round 2 runs 11 and 12 (the interface commits merged in them are the shaders and
Direct3D 12 tracks').

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

- Round 2 run 12. Machine: the desktop (Windows 10, RTX 3080, i9-9900K), through the queue, not
  alone (other tracks' jobs sharing it; a display-reset hazard job of another track held the
  queue before some steps). Times Pacific, 2026-11-20.
  - Module sizes (section 10), release builds of 32c76f17, 03:13 to 03:19 (logs/r12e-sizes.log):
    wcontainers_directx.dll 14,089,216 bytes on WebGPU (7,680 more than run 11) and 9,670,656
    bytes on WebGL 2 (6,144 more); the forwarders 1,024 bytes each. Section 10's joined module
    does not exist (Gap list); run 8's load times stand.
  - Building ahead (plan 4.4; run-warm.sh, the five programs run twice from one cache
    directory, 03:19 to 03:30, logs/r12e-warm.log): Godot 3D's warm start built its 61 recorded
    combinations at start and drew all 61, built 124 ahead at shaders' arrival and drew none of
    them, and built 26 at their first draw. Godot 2D built 3 at start, drew 3, 10 at their first
    draw. Unchanged from runs 9 to 11.
  - The whole bar at 32c76f17 (logs/r12e-*, 02:50 to 03:30): literal check exit 0 (after the
    eight baseline lines it named were deleted); lint of both builds clean with warnings denied;
    library com 15, programs 44 with 2 ignored (55.2 s); Wine's d3d11 146 of 166 (124.4 s),
    d3d10core 110 of 111 (76 s); scenes 135 pass, 0 fail, 4 absent; the WebGL 2 build's scenes
    95 pass, 17 fail, 30 absent; programs and bgfx's as in the Gap list.
  - The WebGL 2 build at 29d3fb69 and 32c76f17 (logs/r12b-*, r12c-*, r12d-*, 02:09 to 02:41):
    com 14 of 15, programs 12 pass, 29 fail, 1 ignored (30.8 s; Known failures); the new stream
    case passes; Wine's d3d10core 98 of 111 (146.8 s), d3d11 78 of 166 (180.7 s).
  - Front 55, unit 5, cache 1 on the rented x86 machine (192 processors, load 1.25 when the run
    began), 02:08.

- Round 2 run 10. Machine: the desktop (Windows 10, RTX 3080, i9-9900K), through the queue, not
  alone (other tracks' jobs sharing it). Times Pacific, 2026-11-19.
  - Module sizes (section 10), release builds of a46cf8d1, 23:43 to 23:48 (logs/r10c-sizes.log):
    wcontainers_directx.dll 14,065,152 bytes on WebGPU (86,016 more than run 9, the replay and
    texture copy routes and the merged shaders crate) and 9,650,688 bytes on WebGL 2 (112,640
    more); the forwarders 1,024 bytes each. Section 10's joined module does not exist (Gap list);
    run 8's load times stand.
  - Building ahead (plan 4.4; run-warm.sh, the five programs run twice from one cache
    directory, 23:48 to 23:56, logs/r10c-warm.log): Godot 3D's warm start built its 61 recorded
    combinations at start and drew all 61, built 124 ahead at shaders' arrival and drew none of
    them, and built 26 at their first draw. Godot 2D built 3 at start, drew 3, 10 at their first
    draw. Unchanged from run 9.
  - The WebGL 2 experiment at 10_0 (uncommitted level check; logs/r10x-*, 23:00 to 23:21): Wine's
    d3d10core 95 of 111 (146.9 s); the rig's scenes 94 pass, 36 fail (31 of them pictures of
    scenes that stopped), 14 absent.
  - Front 55, unit 5, cache 1 on the rented x86 machine (192 processors, load 14 to 25).

- Round 2 run 9. Machine: the desktop (Windows 10, RTX 3080, i9-9900K), through the queue, not
  alone (other tracks' jobs sharing it; a display-reset hazard job of another track held the
  queue for some minutes before two of the steps). Times Pacific.
  - Module sizes (section 10), release builds of 5d14a977's tree with the depth-view change,
    22:13 to 22:18 (logs/r9i-sizes.log): wcontainers_directx.dll 13,978,112 bytes on WebGPU and
    9,538,048 bytes on WebGL 2; the forwarders 1,024 bytes each. Section 10's joined module does
    not exist (Gap list); run 8's load times stand (below), as nothing of loading changed.
  - Building ahead (plan 4.4; run-warm.sh, the five programs run twice from one cache directory,
    22:18 to 22:35, logs/r9i-warm.log): Godot 3D on its cold start built 124 combinations ahead
    at shaders' arrival and drew none of them, and built 66 at their first draw; on its warm start
    it built its 61 recorded combinations at start and drew all 61, built 124 ahead (none drawn)
    and 26 at their first draw. Godot 2D built 3 at start, drew 3, and 10 at their first draw.
  - The whole bar: library step at 5d14a977 (logs/r9i-lib.log) stopped at com.rs's
    `capability_queries_claim_only_what_creation_serves` (the depth-view textures, fixed after);
    Wine's d3d11 146 of 166 and d3d10core 110 of 111, scenes 135/0/4, WebGL 2 scenes 2/0/137, the
    13 programs and bgfx's 6 as in Where things stand (logs/r9i-*, 22:04 to 22:13). At the final
    code (06e6c258, logs/r9j-lib.log, r9k-*, 22:25 to 22:52): lint of both builds
    clean with warnings denied, library com 15 and programs 41 with 2 ignored (50.3 s), Wine's
    d3d11 146 of 166 (127 s) and d3d10core 110 of 111 (76.2 s), the same failing tests; front 55,
    unit 5, cache 1 on the rented x86 machine (192 processors, load 55 when it ended).
  - The WebGL 2 experiment at 10_0 (uncommitted level check, logs/r9x-webgllib.log, 22:02):
    programs.rs 11 pass, 28 fail, 1 ignored; the three `own_level10` cases pass; the 28 failing
    ask for 11_0 devices (22 of them) or reach routes the WebGL 2 back end lacks.

- Round 2 run 8. Machine: the desktop (RTX 3080, i9-9900K), through the queue. Timings marked
  alone ran as `gpu-job --alone`; the rest ran beside other tracks' jobs.
  - Module sizes (section 10), release builds of b43be1e7's tree, desktop 19:41 to 19:46
    Pacific (logs/r8g-sizes.log): wcontainers_directx.dll 13,984,256 bytes on WebGPU and
    9,545,216 bytes on WebGL 2. The Direct3D 12 track's wcontainers_d3d12.dll is 13,562,368
    bytes. The forwarders are 1,024 bytes each. 23c9b34e changed only the factory's entry point
    after that build; its size is not measured.
  - Load time (section 10), alone, 19:46:56 to 19:47:08 Pacific (logs/r8g-load.log). Each
    sample loads one library with `LoadLibraryW` in a fresh process (example `load_time`), 20
    samples per library. Medians, with the lowest and highest:
    - the system's d3d11.dll: 1,166 us (1,137 to 1,406)
    - ours on WebGPU: 1,675 us (1,604 to 2,692)
    - ours on WebGL 2: 1,111 us (1,072 to 2,215)
    - wcontainers_d3d12.dll: 1,677 us (1,614 to 2,750)
  - The two parts of section 10's sum, each measured alone: ours (13.98 MB, 1.67 ms) and the
    Direct3D 12 library (13.56 MB, 1.68 ms). The joined module is not built; its cost is not
    the sum, since both carry wgpu, and it is not measured.
  - Building ahead: not measured again; no change of this run touches it (run 7's numbers stand).
  - At 23c9b34e (logs/r8h-*, 19:46 to 19:50): lint of both builds clean with warnings denied.
    Wine's dxgi file, the factory's tests: test_create_factory, test_private_data,
    test_object_wrapping and test_factory_check_feature_support pass; test_parents crashes as
    before (`CreateSurface`). The programs chocolate-doom, godot-platformer-3d-d3d11,
    atg-simplelightingpc, sdk-tutorial02 and godot-platformer-2d-d3d11 match real Direct3D on
    every compared frame.
  - The whole bar at 23c9b34e (logs/r8i-*, 19:50 to 20:12 Pacific). Literal check exit 0. Lint of both builds clean with warnings
    denied. Front 54, plus 5 unit and 1 cache. Library com 15 and programs 40, with 2 ignored
    (Known failures). Wine's d3d11 file 146 of 166 (158.9 s) and d3d10core 110 of 111 (99.3 s).
    Scenes 135 pass, 0 fail, 4 absent. WebGL 2 scenes 2, 0 and 137. The 13 programs: 11 match,
    and atg-simplecomputepc and atg-simplemsaa_pc differ as before (Gap list).
  - Before that, at b43be1e7's code (logs/r8e-*, 19:25 to 19:35): Wine's d3d11 file 146 of 166
    (157.6 s), d3d10core 110 of 111 (96.3 s). At d943e6bd (logs/r8d-libsome.log), the library
    tests this run touched, ignored ones included: 7 of 8 pass; the 16-bit-norm one fails as
    recorded.

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
- Interface track: a vertex format for `B8G8R8X8_UNORM` reading blue, green and red with the
  fourth component 1 (WebGPU's `unorm8x4-bgra` reads the X byte); an `R10G10B10_XR_BIAS_A2_UNORM`
  format; a `Caps` feature for cube arrays, declared by the WebGPU back end at the same time
  (this front probes for them meanwhile, Departures round 2 run 12).
- WebGPU track: `B5G6R5_UNORM` and `B5G5R5A1_UNORM` as render targets (blended), through the back
  end's converted formats.
- Shaders track: atg-simplecomputepc's division (every pixel inside the Mandelbrot set just below
  4 where Direct3D gives 4, inferred in run 17); the geometry replay's streams past the first
  (`GeometryReplayStreams`) and drawing a stream past 0; an aliased-line route for multisampled
  targets (with the interface track); the pass-through's clip distances at the linked location
  behind a geometry stage (round 2 run 3).
- Direct3D 12 track: the order in which this crate takes over its Windows library with one DXGI
  shell (section 10); not answered by round 2 run 12 (its Next 4 plans to move its COM objects
  into `wcontainers-directx` itself).
- WebGL 2 track: copies from a depth texture into a colour texture of its group stored as a half
  float (`R16_TYPELESS` as `R16Float`) keep every bit pattern, NaNs included (the draw-made copy
  gives 0x7fff for 0xff00 and 0xffff; Wine's test_format_compatibility, round 2 run 11).
- Rig track: a way to run Wine's dxgi file with its Direct3D 12 tests kept off the GPU; the scene
  programs reporting every `--dll` library they loaded, so the forwarder's placement can be
  checked by the rig (the library tests check it today).

## Literals

- Round 2 run 12: the cube-face count is `CUBE_FACES` (device.rs); the ten lines of `6` it
  replaced left the baseline; no line added, no mark made. wcontainers-direct3d-11 542 lines,
  wcontainers-directx 239 (781). The check exits 0 at the run's last commit (logs/r12f-literals.log).

- Round 2 run 11: the two refusals the pixel program for a variant replaced left the baseline
  (two lines); no line added, no mark made. wcontainers-direct3d-11 552 lines,
  wcontainers-directx 239 (791). The check exits 0 at 4430d1cb (logs/r11d-literals.log).
- Round 2 run 10: the four refusals the routes replaced left the baseline (four lines); the new
  refusals are named constants (`VARIANT_BEHIND_REPLAY`, `THROUGH_BEHIND_GEOMETRY`,
  `STREAM_OUT_WITHOUT_COMPUTE`, `TYPED_BUFFER_FORMAT_UNHELD`, `TYPED_STORAGE_BUFFERS_UNSERVED`,
  `GEOMETRY_STAGE_UNSERVED`); no mark made. The check exits 0 at 455f67d7
  (logs/r10e-literals.log).
- Round 2 run 9: wcontainers-direct3d-11 558, wcontainers-directx 239 (797, from 801). The
  rewritten format support answer and the window refusal name their facts (the infrastructure
  crate's support words, `WARNING_PREFIX`, a named refusal); four lines the changes removed left
  the baseline; no line added, no mark made. The check exits 0 at 06e6c258
  (logs/r9l-literals.log).
- Left, as recorded before (git history of this section): the export names in build.rs (a build
  script cannot import the crate it builds), and the rest of the fix list (orchestrator
  magic-literals/fix-graphics.md), a part per run.

## Licences

Round 2 runs 10 to 12 added no dependency and installed nothing on the desktop.

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
