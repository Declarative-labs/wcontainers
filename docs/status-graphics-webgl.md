# Graphics WebGL 2 back end status (G3)

Track: `crates/wcontainers-gpu-webgl`, the WebGL 2 back end on glow, branch `phase-2-graphics-webgl`.
Plan: docs/phase-2-graphics.md, sections 4, 5.8, 6.5, 10 and 12 (G3).

## Where things stand

G3a is done. G3 (section 12's pass bar) is under way: run 1 of G3 fixed every finding of the last
check, ran the shared suite in Chrome as well as on ANGLE, and built surfaces and present, depth read
backs by draws and views in the sRGB twin. The shared suite (20 tests, origin/phase-2-graphics
975c55c) passes 20 of 20 in Chrome's WebGL 2 with every extension, and 18 of 20 natively on ANGLE (the
two failures are bugs of the ANGLE test build; see "Known failures"). Two checks of the back end's own
(vertex and instance numbers under bases with 8, 16 and 32-bit indices; Discard then NoOverwrite
writes) pass on every route. Without EXT_clip_control or WEBGL_provoking_vertex the suite's own raster
blocks ask for what the back end refuses (known failures, G1's suite). Most of the pass bar is still
"Next".

## Done

- `WebGl` implements `wcontainers_gpu::Backend` on glow 0.18, the same code in Chrome (wasm32) and
  natively on ANGLE:
  - Buffers (one GL buffer per WebGL type, indices and other data apart, made at creation by
    usage), 2D, 2D array, cube and 3D textures made with the GL target of `own_view` by
    `texStorage`, texture writes by `texSubImage` with row length and image height, samplers,
    binding sets (uniform ranges, textures, samplers), passes on cached framebuffers with
    `clearBuffer*` loads and `invalidateFramebuffer` for `DontCare` and `Discard`, direct draws
    (arrays and elements, instanced, 8, 16 and 32-bit indices), `copy_buffer` between GL buffers of
    one type, read backs later (pixel pack buffer and a sync object) and now (`readPixels`,
    `getBufferSubData`), submit (with a `getError` check that turns a GL error into device loss) and
    fences.
  - State applied only where it changed (plan 4.4): each draw compares block handles with the last
    applied ones; for a changed block only the GL values that differ from what GL holds are set (state.rs, `GlValues`). Bindings are walked only when the program or a set
    changed, and each texture unit, sampler and uniform block point is rebound only when it differs.
    Vertex attributes are respecified only when the layout, a stream's buffer or its offset changed.
    Clears open the masks and scissor through the same record, and mark the blocks for reapplying.
    Creating a texture or writing one binds it on unit 0, so the next draw walks its bindings again
    (fixed in the second run: before, a draw with the same program and set after an upload sampled
    the uploaded texture). No allocation per draw or per pass.
  - Shaders: naga 30 writes GLSL ES 3.00 (WebGL flavour, `FORCE_POINT_SIZE`); each combined sampler
    gets a unit and each uniform block a binding point when the link is read; programs are linked
    once per (vertex, fragment) pair and counted per handle. `instance_index` reads
    `gl_InstanceID + naga_vs_first_instance`, set per draw only when it changes. Base vertex and
    first instance move the vertex streams (`BaseVertexRebind`, counted).
  - Window origin (plan 6.5): with EXT_clip_control the device draws upper-left and zero-to-one
    (minus-one-to-one per raster block that asks for it); without it the GLSL's last vertex step
    negates `gl_Position.y` and the front face is swapped, and only minus-one-to-one is taken. Both
    routes draw every picture exactly; on the second the test programs rewrite clip z as 2z - w, as
    a front's `ClipDepthRewrite` does.
  - Refused as absent at creation: logic ops, per-target blends, second-source factors, constant
    colour with constant alpha in one blend, depth bounds, per-face stencil masks, fill modes, first
    provoking vertex, depth clamp, bias clamp, wide lines, partial sample masks, storage bindings,
    border and mirror-once samplers, compute, texture views beyond the base ones.
  - Refused as `Absent(CLIP_DEPTH_ZERO_ONE)` without EXT_clip_control: a raster block with
    `depth_minus_one` false (the interface's rule; the front rewrites clip depth instead).
  - Second run: texture copies by `copyTexSubImage2D/3D` from a read framebuffer (colour formats a
    framebuffer can read); buffer-to-texture copies through the pixel unpack buffer; texture-to-
    buffer copies by `readPixels` into the pixel pack buffer, with `PACK_ROW_LENGTH` for the pitch;
    buffer clears as host writes of the pattern into every GL buffer of the buffer; read backs of
    every format `readPixels` reads exactly (format.rs `read_as`: 8-bit normalised read as RGBA
    bytes, integers as RGBA 32-bit integers, 32-bit floats as RGBA floats), kept narrow on the host.
  - Index twins (plan 4.6): a GPU write that reaches only one of a buffer's two GL buffers marks the
    other stale for that range; a draw reading it as indices (or other data), a copy from it and a
    read back first refresh it with a read back (`IndexTwinReadBack`, counted). Draws look only
    when some buffer is stale (a device count). Every GPU write refreshes its destination first, so
    at most one side is stale.
  - Programs: `linkProgram` is issued at creation and read at the first draw (plan 5.8); that draw
    counts `PipelineMiss` when it must wait (always without KHR_parallel_shader_compile, otherwise
    when `COMPLETION_STATUS_KHR` is still false). A failed link loses the device and leaves GL's log
    in `last_log`.
  - `vertex_index` with a base vertex: naga's `naga_vs_base_vertex` is set per draw, only when it
    changes, as `naga_vs_first_instance` is (built; no test reads `vertex_index` under a base vertex
    yet).
  - Run 3: triangle fans and line loops drawn as `TRIANGLE_FAN` and `LINE_LOOP` (`draw_fan_or_loop`
    overridden, `FAN_AND_LOOP` declared); guest indices go once into the index ring. A vertex stream
    of stride 0, and one stepping per instance at rate 0, reads one element for every vertex and
    instance (instance divisor 2^31; GL's stride 0 means tightly packed). `PROVOKING_VERTEX_FIRST`
    with WEBGL_provoking_vertex (`GL_ANGLE_provoking_vertex` natively), set per raster block only
    when it changes. Buffer clears write one 256-byte block of the pattern from the host into every
    GL buffer of the buffer and double it by copies within each (`PatternClearCopied`, counted; no
    host copy of the whole range). A native buffer read whose map fails, a WebGL read that raises an
    error, and a read back whose sync object could not be made lose the device instead of answering
    with stale bytes. `max_point_size` is 1 (programs write `gl_PointSize = 1`; `POINT_SIZE` absent).
  - G3 run 1: each block is converted to the GL values it stands for when it is made (state.rs
    `GlBlend`, `GlDepthStencil`, `GlRaster`, `GlLayout`, plan 4.4), so a draw that applies a changed
    block only compares and sets values. A draw whose binding sets lack a resource its program reads
    (or hold the wrong kind) loses the device with the slot named in `last_log` instead of reading what
    the unit held before. Surfaces: one per back end (a WebGL context draws into its one canvas;
    natively the EGL surface); `configure_surface` sizes the canvas's drawing buffer (natively a new
    pbuffer) and makes an `rgba8unorm` texture that `current_texture` hands out; `present` copies it
    into the drawing buffer by `blitFramebuffer`, upside down (GL's row 0 is the bottom). `vsync` changes
    nothing: the browser shows the canvas at the display's rate and presenting never waits. Depth read
    backs (`DepthReadByDraw`, counted): a full-screen triangle fetches each depth texel of a 2D or 2D
    array texture and writes its stored value (16-bit unorm, 24-bit unorm in a 32-bit word, or a
    float's bits) into an `r32uint` texture, which reads back like any colour texture; it draws with
    GL's lower-left origin so rows match on both window-origin routes. Views in the sRGB twin: the
    twin is a second GL texture of the same shape, made at the first such view; each side records
    when it was written (passes drawing into it, writes, copies) and is brought up to date from the
    other before it is read, sampled or written, every level and layer through a pixel pack buffer
    (`CopyThroughBuffer`, counted). Draws look at sampled textures only while a twin exists. Read backs
    waited for in the browser are ready at once (WebGL updates a sync object only between tasks).
  - Not built yet: reaching one loses the device (`LossReason::Error`, `Err(DeviceLost)` where the
    call returns a result) with the missing piece named in `WebGl::last_log()`, never a panic; a
    multisampled texture is refused as `Invalid` (the caps give one sample). Not built: queries,
    surfaces and present, resolves, multisampled textures, sRGB-twin and stencil views, empty
    passes, copies of depth and of formats a framebuffer cannot read, texture-to-buffer copies of
    formats read wider than stored (depth included), read backs of stencil and of depth-stencil
    formats, depth read backs of cube textures, stencil views, vertex formats GL ES cannot read as stored (BGRA8, 10-10-10-2
    integer, 11-11-10), negative stream offsets after a base vertex, copies between an index-only
    and a data-only buffer, buffer-texture copies with an index-only buffer.
- platform.rs: the context. In the browser `WebGl::on_canvas(WebGl2RenderingContext, off)`;
  natively `WebGl::on_angle(dir, off)` loads `libEGL` from `dir` with khronos-egl (EGL 1.0 calls only)
  and makes an OpenGL ES 3.0 context current on a 1 by 1 pbuffer. `off` names WebGL extensions to
  treat as missing, so tests take the routes of browsers without them. `clipControlEXT` (which glow
  lacks) is called through the extension object in the browser and `glClipControlEXT` natively.
  WebGL extension names map to ANGLE's `GL_` names in one table. GLES has no `glGetBufferSubData`,
  so natively buffers are read by `glMapBufferRange` for reading (`platform::get_buffer_sub_data`);
  in the browser by WebGL's `getBufferSubData`.
- Test programs (examples/): `common/scenes.rs` draws each scene through the interface, generic over
  `Backend`, with the rig's own scene data (scene.rs included by path) and WGSL programs
  (triangle.wgsl from the rig, quads.wgsl and instanced.wgsl here, as the rig's HLSL);
  `webgl-scenes` draws natively on ANGLE and checks pictures; `browser` is the same for WebAssembly,
  driven by browser/run.mjs and page.html. The list of pictures (`Change::PICTURES`) is shared by
  both: the three scenes; two altered quads that must fail (blend factor One, depth test Greater);
  and four routes that must still equal the reference: `stale_unit` (the quads drawn into another
  target, then a new texture made and written magenta, then the same draw with the same program and
  set), `twin_copy` (the index buffer made INDEX|VERTEX and given its indices by `copy_buffer` from a
  data buffer), `base_vertex` (the quads after four other vertices, drawn with base vertex 4) and
  `clear_near` (the triangle, clip z 0, over depth cleared to 0.25 and tested Less). Run 3 added
  (`Expected` says what each must equal): `quads-fan` (two fans from guest 16-bit indices with a
  restart between) and `quads-fan_sequential`, equal to Direct3D's quads; `triangle-line_loop`, equal
  exactly to `triangle-line_strip` (indices 0, 1, 2, 0) drawn in the same run; `triangle-stride_zero`
  (colour from a second stream of stride 0 holding red) and `triangle-flat_red` (colour flat from
  the first vertex, red, where the first may provoke, else from the last with the vertices rotated),
  each equal to Direct3D's triangle with every covered pixel red; `triangle-clear_rects` (the
  triangle, then a 64-pixel square cleared red by `clear_rects`, which draws with the interface's
  built-in programs), equal to Direct3D's triangle with that square red. Every fan or loop picture
  also fails if `FanConverted` or `LoopConverted` was counted while `FAN_AND_LOOP` is present.
- tests/suite.rs: the shared suite (`wcontainers_gpu_test::suite!`) and the checks of
  examples/common/checks.rs on ANGLE, Windows only. examples/browser.rs is the suite's host in
  Chrome: browser/suite.mjs opens browser/suite.html once per test (a failing test panics and the
  module traps), each device on a new canvas, `Host::shown` reading the canvas through a 2D canvas.
  The list of tests comes from the native suite's `--list`, so a test the browser host lacks fails.
- examples/common/checks.rs: `vertex_and_instance_numbers_include_the_bases` (points writing
  `vertex_index`, `instance_index` and the elements a per-vertex and a per-instance stream read into
  an `rgba32uint` target; non-indexed and 8, 16 and 32-bit indexed instanced draws with first vertex,
  base vertex and first instance), `discard_then_no_overwrite_writes` (a read back recorded before a
  `Discard` write of part of a buffer sees the old bytes; `NoOverwrite` writes above the part land).
- browser/chrome.mjs: what run.mjs and suite.mjs share. Chrome is launched again (up to three times)
  when no WebGL 2 context survives half a second in ten tries, which once made the whole check fail.
- Independent check, all done on 2026-11-17: drew the three scenes and compared (exactly equal, both
  on ANGLE and in Chrome, with and without EXT_clip_control); the quads with the colour blend's
  destination factor One fails (25016 pixels differ, largest 38) and with the depth test Greater
  fails (25016 pixels, largest 204), on both; Chrome's WebGL 2 renderer (unmasked, through
  WEBGL_debug_renderer_info) is "ANGLE (NVIDIA, NVIDIA GeForce RTX 3080 (0x00002206) Direct3D11
  vs_5_0 ps_5_0, D3D11)" and chrome://gpu lists the same GL_RENDERER; the capability list was read
  against the WebGL 2 specification, GLES 3.0's format table and the extensions Chrome reports (see
  "Capabilities").

## How to run

All from the checkout root on the Mac.

- Everything at once (both scene routes in Chrome and on ANGLE, and the suite with the checks in
  Chrome and on ANGLE three ways each: every extension, without EXT_clip_control, without both), one
  GPU job after the build; exit 1 on any failure, so today it exits 1 for the known failures. The
  Chrome suite verdicts land in `<out>\suite-chrome*.txt`:
  `tools/graphics-rig/gpu-job --build "crates\wcontainers-gpu-webgl\browser\build.cmd" -- "crates\wcontainers-gpu-webgl\browser\all.cmd C:\wc\out\webgl\<name>"`.
  About 40 s as a job once built.
- The suite in Chrome alone (after build.cmd and a `--list` into a file, as all.cmd does):
  `node crates\wcontainers-gpu-webgl\browser\suite.mjs "%CARGO_TARGET_DIR%\webgl-pkg" <out-file> <list-file> [<extensions off>]`.
- Once per desktop (done; about 4 minutes):
  `tools/graphics-rig/remote "crates\wcontainers-gpu-webgl\angle\build.cmd"` builds ANGLE's
  libEGL.dll and libGLESv2.dll from the mozangle crate into C:\wc\tools\angle-mozangle-0.7.1.
- The scenes, natively through ANGLE and in Chrome's WebGL 2, each checked against the rig's stored
  Direct3D 11 pictures (crates/wcontainers-graphics-test/references/d3d11, with their tolerances):
  `tools/graphics-rig/gpu-job --build "crates\wcontainers-gpu-webgl\browser\build.cmd" -- "crates\wcontainers-gpu-webgl\browser\run.cmd C:\wc\out\webgl\g3a"`
  Add a second argument to run.cmd to switch extensions off, for example
  `run.cmd C:\wc\out\webgl\g3a-noclip "EXT_clip_control WEBGL_provoking_vertex"`. Exit 1 on any unexpected verdict. About 4
  s as a job once built.
- The shared suite on ANGLE (built by build.cmd too):
  `tools/graphics-rig/gpu-job --build "cargo test -j 3 -p wcontainers-gpu-webgl --test suite --no-run" -- "cargo test -j 3 -p wcontainers-gpu-webgl --test suite -- --test-threads=1"`.
  `WEBGL_ANGLE_DIR` overrides the ANGLE directory; `WEBGL_OFF=EXT_clip_control` takes the route
  without clip control. About 1 to 4 s.
  - build.cmd: installs the rig's browser packages, builds the native example `webgl-scenes` and the
    WebAssembly example `browser`, and runs wasm-bindgen into `%CARGO_TARGET_DIR%\webgl-pkg`.
  - run.cmd: `webgl-scenes angle <dir> <out>\angle` draws on ANGLE and checks (pictures and
    caps-angle.txt there); `node crates\wcontainers-gpu-webgl\browser\run.mjs` draws in the rig's
    headless Chrome and writes `.rgba` texels and caps-chrome.txt; `webgl-scenes check` checks those.
- Lint and format: `nix develop -c cargo fmt -p wcontainers-gpu-webgl` and
  `nix develop -c cargo check -j 2 -p wcontainers-gpu-webgl [--target wasm32-unknown-unknown]` on the
  Mac; clippy on the desktop:
  `tools/graphics-rig/remote "cargo clippy -j 3 -p wcontainers-gpu-webgl --all-targets -- -D warnings && cargo clippy -j 3 -p wcontainers-gpu-webgl --lib --example browser --target wasm32-unknown-unknown -- -D warnings"`.
  The crate's tests are the scene runs and the suite above.

How another track uses the back end natively (G4's Windows library on WebGL 2): depend on
`wcontainers-gpu-webgl`, and make the back end on the device thread with
`WebGl::on_angle(Path::new(r"C:\wc\tools\angle-mozangle-0.7.1"), &["EXT_clip_control"])` after
running angle\build.cmd once. Switch EXT_clip_control off natively: this ANGLE mirrors scissor
rectangles vertically under it (see "Known failures"), which breaks every scissored draw; without it
the front rewrites clip depth (`ClipDepthRewrite`), as it must for browsers without the extension.
The caps then lack `CLIP_DEPTH_ZERO_ONE`, which Chrome on the same GPU has (see "Departures"). The
context is current on the calling thread, which must be the device thread. Natively the surface is
an off-screen pbuffer; a real window needs an EGL window surface, not built yet.
In the browser: `WebGl::on_canvas(canvas.getContext("webgl2"), &[])`; the canvas is the surface.
With every extension both report the same `Caps` on the RTX 3080 (compared on 2026-11-17: identical
lists).

## Capabilities

Declared today (each one built): `READ_BACK_NOW`, `FAN_AND_LOOP` (WebGL 2 topologies),
`PROVOKING_VERTEX_LAST` (GL's rule), `PROVOKING_VERTEX_FIRST` (with WEBGL_provoking_vertex),
`CLIP_DEPTH_MINUS_ONE`, `CLIP_DEPTH_ZERO_ONE` (with EXT_clip_control), `INDEX_U8`,
`FRACTIONAL_DEPTH_BIAS` (`polygonOffset` units are floats), `BLEND_CONSTANT_ALPHA` (GLES 3.0 core;
mixed with the constant colour refused, as WebGL requires). Everything else is absent, including
what WebGL 2 or Chrome's extensions have but this back end does not build yet: `TRANSFORM_FEEDBACK`,
`POINT_SIZE` (WGSL has no point size; a front would need a program variant writing it),
`TIMESTAMPS`, `DEPTH24_UNORM` (the probe), and by extension (all reported by Chrome on the RTX 3080)
`POLYGON_MODE`, `CLIP_DISTANCE`,
`CULL_DISTANCE`, `DEPTH_CLAMP`, `DUAL_SOURCE_BLEND`, `INDEPENDENT_BLEND`, `MIRROR_ONCE`,
`DEPTH_BIAS_CLAMP`, `BLOCK_COMPRESSION`, `SAMPLE_SHADING`. The adapter is WebGL's renderer string
(unmasked in Chrome), PCI numbers 0 (WebGL gives none), software when it names SwiftShader,
llvmpipe or Microsoft Basic Render Driver.

Limits on the RTX 3080, Chrome 157 and ANGLE alike: 16384 2D and cube, 2048 3D and layers, 12
uniform blocks per stage of 65536 bytes (the plan's 16384 was the Mac), 16 texture units per stage,
16 attributes, stride 255 (WebGL's rule), 8 targets, 30 varying vectors, uniform offsets aligned to
256, points 1 (the device's range is 1024, unused while `POINT_SIZE` is absent), lines 1 wide,
anisotropy 16, immediates 0 (plain uniforms not built yet).

Formats, from GLES 3.0's table with EXT_color_buffer_float, OES_texture_float_linear and
EXT_float_blend (all present here): every sized colour format GLES 3.0 holds as stored samples;
normalised ones filter, and render and blend unless signed or RGB9E5; 16-bit float and
R11G11B10 filter, render and blend; 32-bit float filter, render and blend (RGB32F does not render);
integers render (not RGB32), never filter or blend; depth formats are depth targets, and those
without stencil also sample. Copy out (read back): R8, RG8 and RGBA8 normalised and RGBA8 sRGB,
every colour-renderable 8, 16 and 32-bit integer format, and R32F, RG32F and RGBA32F (with
EXT_color_buffer_float); not 16-bit floats, packed formats, depth or stencil. One sample only.
No bits yet: 16-bit normalised, BGRA and the other swizzled formats, A8, Stencil8, block-compressed.
Since G3 run 1: `PRESENT` on `rgba8unorm` (the canvas's drawing buffer), and `COPY_OUT` on
`depth16unorm`, `depth24plus` and `depth32float` (by `DepthReadByDraw`); not yet on the depth-stencil
formats, whose stencil cannot be read back yet.

## Measurements

- 2026-11-17, desktop (RTX 3080, driver 32.0.16.2101; other tracks' jobs queued, not timed): the
  three scenes equal real Direct3D 11's pictures exactly (0 of 65536 pixels differ, largest
  difference 0) through Chrome 157's WebGL 2 and through ANGLE (mozangle 0.7.1, Direct3D 11), with
  and without EXT_clip_control. Tolerance needed: none; the rig's recorded allowance (1, 1 and 0
  steps, from WARP) is not used up. Exactness is expected here because ANGLE draws on the same GPU
  through Direct3D 11 and the programs do the same float operations; other GPUs and browsers may
  need the recorded allowance.
- 2026-11-17, second run, same desktop (queued jobs, not timed; jobs near 14:00 desktop time): all 9 pictures as
  expected in all four runs (ANGLE and Chrome, with and without EXT_clip_control): the 3 scenes and
  the 4 same-picture routes exactly equal (0 pixels), the 2 altered quads failing (25016 pixels,
  largest differences 38 and 204). Before the fixes the check found `stale_unit`, `twin_copy` and
  (without clip control) `clear_near` failing (25016, 25016 and 19738 pixels).
- Shared suite on ANGLE, second run: 4 of 7 pass, on both routes (buffer writes and read backs,
  texture writes and read backs, copies, clears). Once, with the suite's raster block changed
  locally (not committed) to take provoking vertex Last and clip depth by the caps, 6 of 7 passed
  on both routes; only `present_shows_the_frame` failed.
- 2026-11-17, third run, same desktop (queued jobs, not timed): Chrome 157 and ANGLE report the same
  features with every extension: READ_BACK_NOW, FAN_AND_LOOP, PROVOKING_VERTEX_LAST and FIRST,
  CLIP_DEPTH_MINUS_ONE and ZERO_ONE, INDEX_U8, FRACTIONAL_DEPTH_BIAS, BLEND_CONSTANT_ALPHA. All 16
  pictures as expected in Chrome with every extension and with EXT_clip_control and
  WEBGL_provoking_vertex off, and on ANGLE with both off; on ANGLE with every extension 15 of 16
  (`triangle-clear_rects` fails, 4096 pixels: the known ANGLE failure). The three scenes and every
  same-picture route exactly equal (0 pixels); the two altered quads fail (25016 pixels each, largest
  38 and 204). Renderer in Chrome "ANGLE (NVIDIA, NVIDIA GeForce RTX 3080 (0x00002206) Direct3D11
  vs_5_0 ps_5_0, D3D11)". Lint clean on both targets.
- Shared suite on ANGLE, third run (13 tests, origin/phase-2-graphics c15677a): 11 pass with every
  extension (all but `generate_mips_averages_in_the_values_format` and `present_shows_the_frame`); 5
  without EXT_clip_control, and 5 without it and WEBGL_provoking_vertex (the others are refused by
  the suite's own raster block; see "Known failures").
- 2026-11-17, G3 run 1, same desktop (RTX 3080, driver 32.0.16.2101; jobs from the queue, not timed,
  about 40 s each between 15:02 and 15:19 desktop time; CPU load 17% just after): the shared suite at
  origin/phase-2-graphics 975c55c, 20 tests, plus the 2 checks:
  - Chrome 157.0.8103.78 with every extension: 22 of 22 pass. Without EXT_clip_control: 9 of 22;
    without it and WEBGL_provoking_vertex: 9 of 22 (the rest refused by the suite's raster blocks).
  - ANGLE (mozangle 0.7.1) with every extension: 20 of 22 (`present_shows_the_frame` and
    `the_scissor_bounds_the_draw` fail). Without EXT_clip_control: 8 of 22; without both: 8 of 22.
  - Before the fixes of this run the suite in Chrome failed 18 of 20 (read backs never ready after
    `wait_read`), and `generate_mips_averages_in_the_values_format` failed everywhere (no sRGB twin).
  - The 16 scene pictures: as in run 3 (all as expected in Chrome on both routes and on ANGLE without
    the extensions; `triangle-clear_rects` fails on ANGLE with EXT_clip_control).
  - Lint clean on both targets.
- In the first full check of this run, Chrome's first launch kept no WebGL 2 context in ten tries
  and was closed; the second launch worked. In the other launches one context was lost first.
- Chrome's GPU process restarted once just after Chrome started in most runs (the first WebGL
  context lost, or none made); run.mjs waits for a context to survive half a second first and says
  how many did not.
- The headless Chrome the rig drives reports in chrome://gpu "WebGL: Hardware accelerated but at
  reduced performance" (and the same for WebGPU). Per-draw timings taken there (G3's later work)
  must say so; whether a headed Chrome differs is not measured.

## Known failures

- `triangle-clear_rects` and the suite's `the_scissor_bounds_the_draw` on native ANGLE with
  EXT_clip_control. mozangle 0.7.1's ANGLE mirrors the scissor rectangle vertically when the clip
  origin is upper-left, though EXT_clip_control leaves scissors in window coordinates (in its
  source, StateManager11.cpp turns on `presentPathFastActive` for an upper-left origin, lines 871 to
  874, and then sets `scissorY = height - scissor.height - scissor.y`, lines 1415 to 1418). In the
  suite, the scissor x 1, y 2, w 2, h 1 on a 4 by 4 target drew row 1 instead of row 2. Chrome 157's
  ANGLE draws both right, and so does this ANGLE without EXT_clip_control. A bug of the test tool;
  left failing.
- Suite `present_shows_the_frame` on native ANGLE: reading this ANGLE's pbuffer default framebuffer
  gives 72 in every byte whatever was drawn into it, even right after a `glClear` of it, with no GL
  error, a complete framebuffer, an RGBA8 configuration with no samples, and reads in RGBA and in
  the implementation's BGRA format alike. In Chrome the same test passes (the canvas read through a
  2D canvas). A bug of the test tool, or of using an ANGLE pbuffer this way; left failing.
- Suite, without EXT_clip_control or WEBGL_provoking_vertex, in Chrome and on ANGLE: 13 tests
  (`triangle_covers_...`, `textured_draw_...`, `edges_through_pixel_centres_...`,
  `strips_take_flat_values_...`, `depth_writes_off_...`, `srgb_targets_...`,
  `values_are_interpolated_...`, `the_scissor_bounds_...`, `stencil_operations_...`,
  `near_clipping_...`, `linear_filtering_...`, `instances_start_...`, `indexed_strips_...`; on ANGLE
  also `present_shows_the_frame`): the suite's own raster blocks ask for clip depth from zero
  (`depth_minus_one` false) and provoking vertex First, which the back end lacks on those routes and
  refuses as the interface says (`Absent(CLIP_DEPTH_ZERO_ONE)`, `Absent(PROVOKING_VERTEX_FIRST)`).
  The suite (G1's crate) should take both from the caps, as the built-in programs do.

## Departures from the plan

- ANGLE natively is Mozilla's ANGLE (mozangle 0.7.1: Firefox 153 ESR, ANGLE commit 7b0bc3d196d4 of
  2026-06-09), built from source, not Chrome's: Chrome 157's libEGL.dll and libGLESv2.dll are 453 KB
  stubs with no exports (ANGLE is inside chrome.dll), and Electron 45 ships none. It is the same
  ANGLE on Direct3D 11, but a different version from Chrome's, and its extension list differs (no
  EXT_polygon_offset_clamp or EXT_depth_clamp on an ES 3.0 context; provoking vertex as
  GL_ANGLE_provoking_vertex); the table in platform.rs must map those names when they are built.
- GL buffers for indices and other data are made at creation from the usage, not at first binding
  (plan 4.6), so a buffer with both usages used one way pays a second upload. Simpler and never
  stale; revisit with `IndexTwinReadBack`.
- `wait_fence` in the browser calls `finish()`, because WebGL's longest wait on a sync object is zero;
  the plan's JSPI suspension belongs to the device thread's event loop, not built yet.
- Programs are linked at creation, not at the first draw, even without KHR_parallel_shader_compile;
  only the reading of the link waits for the first draw. Issuing the link early can only help, and
  the first draw still counts `PipelineMiss` as the plan says. A link failure is device loss at that
  draw rather than an error from `create_program`: the modules are validated, so a failure is a bug
  of ours.
- Natively the read of a buffer maps it (GLES 3.0 has no `glGetBufferSubData`); WebGL reads it with
  `getBufferSubData`. Same bytes, same wait.
- Buffer clears: WebGL 2 has no buffer clear, so every clear (zero included) takes
  `PatternClearCopied`, which the interface names for non-zero patterns only: a 256-byte host block,
  doubled by copies inside each GL buffer, so both twins stay whole and nothing large crosses from
  the host.
- The interface change ab4b7e9 edits `helpers.rs`, which section 12 gives to G1 alone. The fault was
  in a helper's block (the built-ins' raster asked for a provoking vertex WebGL 2 lacks without an
  extension), the fix is two lines in the style of the clip depth one beside it, and no other route
  made the fast-path defaults usable here; G1 should take it over.
- Natively the back end is to be made with EXT_clip_control switched off (above), so its caps lack
  `CLIP_DEPTH_ZERO_ONE`, which Chrome on the same GPU reports; section 10 says the native route
  reports the Caps the browser would. The reason: this ANGLE mirrors every scissored draw under the
  extension (checked in its source), and the browser route without the extension is one the front
  must serve anyway. With every extension both report identical caps.
- Presenting copies the frame once more than WebGPU does: WebGL's drawing buffer is not a texture,
  so `current_texture` is a texture of the back end's and `present` blits it into the drawing buffer
  (one full-size copy per frame, expected cheap, not measured). Not counted as a slow path: it is
  the only route WebGL 2 has.
- A back end has one surface (`create_surface` a second time is `Invalid`): a WebGL context draws
  into one canvas only. `vsync` is not honoured (the browser decides).
- Views in the sRGB twin are a second texture kept in step inside the back end, since the interface
  allows such views on every texture; plan 4.12 has fronts copy only for other formats
  (`ReinterpretCopy`). Counted as `CopyThroughBuffer`, the route plan 4.12 names for WebGL 2's copies
  within a typeless group.
- Draws with a provoking vertex the raster block does not name: `draw_fan_or_loop` follows the raster
  block, as the default's converted list also assumes; its `provoking` argument is not used.

## Interface changes

- db2c406 "interface: said that a webgl back end refuses a blend block using the constant colour and
  constant alpha together": a doc comment on `Features::BLEND_CONSTANT_ALPHA` and the plan's WebGL 2
  list. WebGL forbids those two factors together in one colour blend (WebGL specification, "Blending
  With Constant Color"); the back end answers `Absent(BLEND_CONSTANT_ALPHA)` for such a block and the
  front blends it in the program.

- 4b71023 "interface: drew the built-in programs in the clip depth convention the back end has":
  the built-in raster block takes `depth_minus_one` from `CLIP_DEPTH_ZERO_ONE`. The built-ins write
  clip z 0, which both conventions hold. On its own it was not enough: the block still asked for
  provoking vertex First, which this back end then always refused (found by the second check).
- ab4b7e9 "interface: drew the built-in programs with the provoking vertex the back end has": the
  built-in raster block asks for First only where `PROVOKING_VERTEX_FIRST` is present, else Last; the
  built-ins pass no flat values, so either gives the same picture. Shown by `triangle-clear_rects`
  in Chrome with and without WEBGL_provoking_vertex and EXT_clip_control, and on ANGLE without them.

## Installed on the desktop

- C:\wc\tools\wasm-bindgen-0.2.130-x86_64-pc-windows-msvc (20 MB): the wasm-bindgen command, from its release
  archive on github.com/wasm-bindgen/wasm-bindgen (MIT OR Apache-2.0).
- C:\wc\tools\libclang-18.1.1: libclang.dll (84 MB) from the PyPI `libclang` 18.1.1 wheel (Apache-2.0
  with LLVM exception), which mozangle's bindgen needs to build ANGLE.
- C:\wc\tools\angle-mozangle-0.7.1: libEGL.dll and libGLESv2.dll (7.1 MB), built by angle\build.cmd.
- The mozangle build's objects in C:\wc\target\phase-2-graphics-webgl\release.
- Left by the independent check of the first run (not ours to remove without asking): the pictures
  in C:\wc\out\webgl-check and an empty lock file C:\wc\locks\phase-2-graphics-webgl-check.

## Licences

- glow 0.18.0: MIT OR Apache-2.0 OR Zlib (its Cargo.toml); its slotmap 1.1.1: Zlib.
- khronos-egl 6.0.0 (native only, feature `dynamic`): MIT/Apache-2.0 (its Cargo.toml); its
  libloading 0.8.9: ISC; libc 0.2.191 and pkg-config 0.3.34: MIT OR Apache-2.0.
- naga 30.0.2 (`glsl-out`; `wgsl-in` for the test programs): MIT OR Apache-2.0.
- wasm-bindgen 0.2.130, js-sys and web-sys 0.3.107 (wasm32 only): MIT OR Apache-2.0.
- anyhow (test programs): MIT OR Apache-2.0.
- mozangle 0.7.1 (the ANGLE build tool in angle/, outside the workspace, never a dependency of the
  back end): BSD-3-Clause (its Cargo.toml and LICENSE, ANGLE's).

## Next

G3's pass bar (plan section 12), in about this order:
1. Per-draw cost against G2 on the same scene, in Chrome (a scene of many small draws changing a
   uniform range and a texture per draw, timed in both back ends as `gpu-job --alone`).
2. Stencil: read backs of stencil and depth-stencil formats (`DepthReadByDraw` with
   WEBGL_stencil_texturing, `GL_ANGLE_stencil_texturing` natively; without it the eight-draw bit
   route of plan 4.12), stencil views (`StencilCopiedByDraw`), then `COPY_OUT` and `SAMPLE` on the
   depth-stencil formats; a test reading both ways with equal answers.
3. Copies within a typeless group (`CopyThroughBuffer`; by a draw where the widths differ, since
   `readPixels` reads integers and floats four components wide), and depth copies by `blitFramebuffer`.
4. Queries: occlusion (`OcclusionAny`), timestamps and `timestamps_disjoint` (EXT_disjoint_timer_query
   in Chrome where present); WebGL gives query results only between tasks, so a wait needs the device
   thread's event loop (plan 4.8's JSPI).
5. Transform feedback (`capture_vertices` with and without rasterising), and the index twin written
   by it (`IndexBufferTwin`, `IndexTwinReadBack`).
6. Multisampled textures, resolves, `MultisampleCopyResolved`; the 24-bit depth probe.
7. `generate_mips` against G2 for `r32float` without OES_texture_float_linear and an integer twin of
   a 16-bit normalised format (the default's `FilterInShader`); the `generateMipmap` override;
   `clear_texture` and `clear_rects` by `clearBuffer*`.
8. BC and RGB9E5 read back as written (`HostCopyKept`, `BlockDecoded`), BGRA by `FormatConverted`,
   16-bit normalised by EXT_texture_norm16.
9. Both window-origin routes against G2 (a culled one-sided triangle, a `gl_FragCoord` gradient, a mip
   chain and a read back); a fractional viewport through `ViewportInShader` against G2; a fractional
   `depth_bias` and `ConstantAlpha` natively; cube, array and 3D textures made with the GL target of
   `own_view`, with a test.
10. An EGL window surface natively, for G4's Windows library.
