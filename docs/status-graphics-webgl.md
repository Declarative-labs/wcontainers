# Graphics WebGL 2 back end status (G3)

Track: `crates/wcontainers-gpu-webgl`, the WebGL 2 back end on glow, branch `phase-2-graphics-webgl`.
Plan: docs/phase-2-graphics.md, sections 4, 5.8, 6.5, 10 and 12 (G3).

## Where things stand

G3a is done. After run 6, G3 (section 12's pass bar) is met in Chrome and natively on ANGLE with
every extension. What remains cannot be fixed inside this crate:
- Chrome's WebGL 2 passes the shared suite (24 tests) and the back end's 16 checks, 40 of 40, with
  every extension and again without WEBGL_stencil_texturing and the four BC extensions.
- Natively on ANGLE also 40 of 40 with every extension since run 6: `present_shows_the_frame` now
  presents through an EGL window surface on a real window instead of a pbuffer.
- Against the WebGPU back end, 5 of 6 comparisons pass. The one that fails is `r32float` filtered
  in the program here against WebGPU's hardware filter (one float step off in 5 of 16 values, from
  G1's in-shader filter); the same comparison route against route is byte-equal.
- Routes without EXT_clip_control or WEBGL_provoking_vertex fail the 15 suite tests whose own raster
  blocks ask for what is absent, in Chrome and on ANGLE. G1 has to change that.
- `TIMESTAMPS` is built but cannot run here: Chrome (every ANGLE back end) and mozangle give 0
  timestamp bits on the RTX 3080.

Run 6 merged origin/phase-2-graphics (41816d5) and origin/phase-2-graphics-webgpu (eb1e292), added
the native window surface, built captures of strips, fans, loops and indexed draws (they lost the
device before, while `TRANSFORM_FEEDBACK` was declared), and made `max_line_width` 1.

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
    applied ones; for a changed block only the GL values that differ from what GL holds are set
    (state.rs, `GlValues`). Bindings are walked only when the program or a set
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
    GL's lower-left origin so rows match on both window-origin routes. The depth of depth-stencil
    formats reads the same way; their stencil with WEBGL_stencil_texturing (enabled by the back end;
    `GL_ANGLE_stencil_texturing` natively, which mozangle 0.7.1 lacks), fetched as unsigned integers
    with the texture's `DEPTH_STENCIL_TEXTURE_MODE` set to `STENCIL_INDEX` for the draw and put back.
    Views in the sRGB twin: the
    twin is a second GL texture of the same shape, made at the first such view; each side records
    when it was written (passes drawing into it, writes, copies) and is brought up to date from the
    other before it is read, sampled or written, every level and layer through a pixel pack buffer
    (`CopyThroughBuffer`, counted). Draws look at sampled textures only while a twin exists. Read backs
    waited for in the browser are ready at once (WebGL updates a sync object only between tasks).
  - Not built yet: reaching one loses the device (`LossReason::Error`, `Err(DeviceLost)` where the
    call returns a result) with the missing piece named in `WebGl::last_log()`, never a panic; a
    multisampled texture is refused as `Invalid` (the caps give one sample). Not built (queries, resolves,
    multisampled textures and transform feedback were built later): empty passes, copies from signed normalised
    formats and of formats a framebuffer cannot read (RGB9E5, BC), copies between depth and colour,
    texture-to-buffer copies of formats read wider than stored (depth included), depth read backs of
    cube textures, both aspects of one depth-stencil texture fetched by one draw, vertex formats GL
    ES cannot read as stored (BGRA8, 10-10-10-2 integer, 11-11-10), negative stream offsets after a
    base vertex, copies between an index-only and a data-only buffer, buffer-texture copies with an index-only buffer.
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
  `Discard` write of part of a buffer sees the old bytes; `NoOverwrite` writes above the part land),
  `depth_and_stencil_read_back` (every depth format with `COPY_OUT` cleared to 0.25 and stencil 0x5a,
  the middle 2 by 2 set to 0.75 and 0xc3 by `clear_rects`, read back as stored: unorm depth within
  one step, float depth and stencil exactly).
- browser/chrome.mjs: what run.mjs and suite.mjs share. Chrome is launched again (up to three times)
  when no WebGL 2 context survives half a second in ten tries, which once made the whole check fail.
- G3 run 2:
  - Stencil read backs without WEBGL_stencil_texturing (`StencilCopiedByDraw`, plan 4.12): eight
    full-screen draws into an `r8unorm` texture, each passing the stencil test only where its bit is
    set and adding 2^k/255 by blending with the constant colour; the depth fetch program then reads
    it with a scale of 255 into the `r32uint` texture read back. Stencil views: with the extension
    the depth-stencil texture itself, its `DEPTH_STENCIL_TEXTURE_MODE` set when a draw binds the view
    (a draw fetching both aspects of one texture loses the device as not built); without it an
    `r8uint` copy of the whole texture, made at the first stencil view and filled by the bit draws
    whenever the stencil changed since (tracked like the sRGB twin). Depth-stencil formats now have
    `SAMPLE` and `COPY_OUT` on every route.
  - Copies (`copy_texture`): equal renderable formats by `copyTexSubImage` as before; depth and
    depth-stencil formats to themselves by `blitFramebuffer`; formats of one typeless group through a
    pixel pack buffer where the source reads back as stored (8-bit normalised), else by a draw that
    fetches each texel, recovers its bits and writes them as the destination's value (src/convert.rs:
    8, 16 and 32-bit integers, 16 and 32-bit floats, 8-bit unorm, 10-10-10-2), both counted
    `CopyThroughBuffer`. Half floats are taken apart by hand so NaN payloads survive. Signed
    normalised sources (whose -128 samples as -127 does), sRGB destinations by draw, packed floats and
    cube sources by draw, and depth to colour lose the device as not built.
  - The 24-bit depth probe (plan 4.11) when the back end is made: a triangle at depth 0.1 into a 1 by
    1 `depth24plus` texture without and with a constant bias of one unit, each fetched as float
    bits; one 24-bit step apart sets `DEPTH24_UNORM`, one float step leaves it clear, anything else
    loses the device with the depths in `last_log`.
  - Textures read only by loads (a program's `textureLoad` with no sampler) are bound with the back
    end's nearest sampler, whose minification filter is `NEAREST_MIPMAP_NEAREST`: before, integer and
    depth textures were incomplete under the texture's default filters and read zeros, and fetches
    reached only the base level.
  - examples/common/kit.rs: small drawing helpers generic over the back end, for the checks and the
    comparisons. New checks (examples/common/checks.rs, run in Chrome and on ANGLE):
    `stencil_sampled_and_read_back` (each pixel's own stencil value, sampled through a stencil view
    and read back, twice with the stencil changed between), `copies_within_a_typeless_group` (13
    pairs, every bit pattern from a fixed xorshift sequence, two layers, a region moved),
    `loads_count_levels_from_the_view` (loads and `textureDimensions` of every level of views starting
    at levels 0, 1 and 2), `fractional_viewports_are_refused` (without `FRACTIONAL_VIEWPORT` a draw
    with a viewport at a fraction of a pixel loses the device as a broken rule, naming the viewport,
    instead of being rounded as before).
  - tests/against_webgpu.rs (Windows): the same drawings on ANGLE, with and without
    EXT_clip_control, and through the WebGPU back end on wgpu's Direct3D 12, read back and compared
    byte for byte: `both_window_origins_equal_webgpu` (a culled one-sided pair of triangles, a
    `gl_FragCoord` gradient, the gradient drawn into each level of a mip chain),
    `cube_array_and_3d_textures_equal_webgpu` (a cube sampled towards 16 directions, a 2D array and a
    3D texture loaded), `generate_mips_of_r32float_without_float_filtering_equals_webgpu` (with and
    without OES_texture_float_linear) and `generate_mips_of_an_integer_twin_equals_webgpu`
    (`rg16unorm` values stored as `rg16uint`).
- Independent check, all done on 2026-11-17: drew the three scenes and compared (exactly equal, both
  on ANGLE and in Chrome, with and without EXT_clip_control); the quads with the colour blend's
  destination factor One fails (25016 pixels differ, largest 38) and with the depth test Greater
  fails (25016 pixels, largest 204), on both; Chrome's WebGL 2 renderer (unmasked, through
  WEBGL_debug_renderer_info) is "ANGLE (NVIDIA, NVIDIA GeForce RTX 3080 (0x00002206) Direct3D11
  vs_5_0 ps_5_0, D3D11)" and chrome://gpu lists the same GL_RENDERER; the capability list was read
  against the WebGL 2 specification, GLES 3.0's format table and the extensions Chrome reports (see
  "Capabilities").

- G3 run 3:
  - Occlusion queries (`QueryKind::OcclusionAny`, `ANY_SAMPLES_PASSED`): a set holds one GL query
    per index, made at its first begin; a read back holds the query objects it reads, so a query
    begun again before the read finished gets a new one. `resolve_queries` is the interface's
    default (`QueryResolveWait`). `Occlusion` (exact counts) is `Absent(OCCLUSION_COUNT)`: GL ES
    answers only whether any sample passed. WebGL hands out query results only between tasks, so
    `wait_read` of queries suspends the device thread until the browser's next task through JSPI:
    the back end imports `next_task` from the module `wcontainers-device`, which the host supplies
    as a `WebAssembly.Suspending` function (browser/device.js), and the host calls the code that
    waits through `WebAssembly.promising` (suite.html calls the plain export `suite_run`). Natively
    the wait spins on `QUERY_RESULT_AVAILABLE`.
  - Timestamps (`write_timestamp` by `queryCounterEXT`, `timestamps_disjoint` from
    `GPU_DISJOINT_EXT`), declared only where EXT_disjoint_timer_query_webgl2 is present and
    `QUERY_COUNTER_BITS_EXT` for `TIMESTAMP_EXT` is above zero. On the RTX 3080 both Chrome and
    mozangle give zero bits (ANGLE's Direct3D 11 back end writes no timestamps; it has elapsed-time
    queries only), so `TIMESTAMPS` is absent there and the route with it is built but not run.
  - Multisampled textures are renderbuffers (WebGL 2 has no multisampled textures), made by
    `renderbufferStorageMultisample` with the counts GL reports per format (`getInternalformatParameter`
    `SAMPLES`; in WebGL that answer is the whole list). `RESOLVE` on colour formats that are not
    integers. Pass resolve targets and `resolve` by `blitFramebuffer`; a resolve in a format other
    than the source's stored one counts `ResolveInStoredFormat`. Whole copies between multisampled
    textures resolve the source into a temporary texture and draw it into every sample of the
    destination (`MultisampleCopyResolved`). `MULTISAMPLE_READ` and `SAMPLE_SHADING` are absent: a
    draw reading a multisampled texture loses the device naming it; integer samples cannot be
    copied (a loss naming `MULTISAMPLE_READ`); multisampled depth, stencil and sRGB copies lose the
    device as not built. The texture's storage is an enum (`Storage`), so every GL texture use
    handles the renderbuffer case.
  - `FRACTIONAL_DEPTH_BIAS` by a probe when the back end is made (beside the 24-bit one): a
    triangle 0.45 of a step above step 1000 of `depth16unorm`, without and with a bias of 0.1
    units; declared only when it then stores 1001. Without it a raster block with a fractional bias
    is `Absent(FRACTIONAL_DEPTH_BIAS)` (the front adds the bias in the program).
  - New checks (examples/common/checks.rs, in Chrome and on ANGLE): `timestamps_and_their_disjoint_flag`,
    `multisampled_copies_resolve_as_their_source` (a partly covered diagonal, resolved at the pass's
    end and by `resolve`, and the copy's resolve equal to the source's, `MultisampleCopyResolved`
    counted once), `fractional_depth_bias` (the refusal where the probe found none),
    `constant_alpha_blends` (`ConstantAlpha` natively at alphas 0.5 and 0.25, within one step, and
    a blend of the constant colour with the constant alpha refused as absent). The browser host runs
    the suite's three new tests (multisampled targets, per-sample programs, occlusion queries).
  - tests/against_webgpu.rs: `a_fractional_viewport_in_the_program_equals_webgpu_native` (a
    triangle in a viewport at x 2.5, y 1.25, 11 by 12.5 of 16 by 16, drawn here over the whole
    target by a WGSL variant moving the positions as the viewport would, `ViewportInShader`). It
    fails today: the WebGPU back end does not declare `FRACTIONAL_VIEWPORT` yet (see "Known
    failures").
  - examples/draw-cost.rs, browser/cost.html and cost.mjs: the per-draw cost of both back ends in one
    Chrome (see "Measurements").

- G3 run 4:
  - Transform feedback (`capture_vertices`, `TRANSFORM_FEEDBACK`, `capture_buffers` from
    `MAX_TRANSFORM_FEEDBACK_SEPARATE_ATTRIBS`). A capture links the program again from its GLSL
    with its location outputs (`_vs2fs_locationN`) as varyings, interleaved for one target and
    separate for more, with the empty fragment shader when it does not rasterise. There are four
    such programs per program at most, each made at its first capture. The draw runs between
    `beginTransformFeedback` and `endTransformFeedback`, with `RASTERIZER_DISCARD` and the default
    framebuffer outside a pass. A target that holds indices too gets its index twin marked stale, so
    the next index read refreshes it (`IndexTwinReadBack`). Generic buffer bindings are cleared
    first, because WebGL refuses a capture target bound anywhere else. GLES 3.0 captures only draws
    of lists without indices: strips, fans, loops and indexed captures lost the device as not built
    until run 6, which expands them (below).
    A target too small for the vertices, or the wrong number of targets, is `Invalid`.
  - Size queries: a program that calls `textureSize` gets a hidden uniform `<sampler>_size` per
    texture (the bound view's first level: width, height, and depth or layers). `textureSize(t, l)`
    becomes a shift of it, set when the bound view changes. ANGLE (Chrome's and mozangle alike)
    answers `textureSize` for the wrong level once a texture's base level has changed.
  - Host copies (`HostCopyKept`, src/host.rs): RGB9E5 and BC textures keep every level on the host.
    Writes go there and to GL; read backs, now and later, come from it. BC is GL's compressed
    format where its WebGL extension is present (`compressedTexSubImage`, not in 3D), else decoded
    by bcdec_rs (`BlockDecoded`): BC1 to BC3 and BC7 as RGBA8, BC4 and BC5 as R8 and RG8 (snorm for
    the signed ones), BC6H as RGBA16F. `BLOCK_COMPRESSION` is declared when all four extensions are
    present. Copies into or out of host-kept textures, and their sRGB views, lose the device as not
    built.
  - New checks (in Chrome and on ANGLE): `vertices_captured_with_and_without_rasterising`
    (interleaved, separate at an offset, while drawing, whole primitives only, and the pass rule),
    `captured_indices_drawn_both_ways` (a buffer holding indices and vertices, written by the host
    and then captured, drawn as indices and as a vertex stream, the twin refreshed once),
    `block_compressed_and_shared_exponent_textures_read_back_as_written` (RGB9E5 and all 14 BC
    formats; two layers of three levels, the last smaller than a block; one block written again;
    read back now and later; a solid BC1 block and an RGB9E5 texel loaded), and
    `fragment_depth_follows_the_clip_convention`.
  - all.cmd's Chrome run without WEBGL_stencil_texturing, and its ANGLE run without both clip
    extensions, also switch off the four BC extensions, so the decoded route runs.

- G3 run 6:
  - Natively the surface is an EGL window surface on a host window (`WebGl::add_window(name,
    hwnd)`, unsafe: the window must outlive its surfaces). `create_surface` makes it current in place
    of the 1 by 1 pbuffer, `present` blits the frame into its back buffer (stretched to the window's
    size if that differs) and calls `eglSwapBuffers`, and `vsync` is `eglSwapInterval`. Destroying the
    surface puts the pbuffer back. A window not named by `add_window` is `Invalid`. Where ANGLE has
    EGL_ANGLE_direct_composition (mozangle does), the surface is made with
    `EGL_DIRECT_COMPOSITION_ANGLE`, a flip-model swap chain shown through DirectComposition. Without
    it, ANGLE's blit-model swap chain left the test window black as `PrintWindow` read it (measured
    once, then dropped). The pbuffer resizing and `read_presented` are gone. The EGL instance is now
    EGL 1.1 (for the swap interval).
  - The native suite's host opens a 4 by 4 popup window and reads it as the compositor shows it
    (`PrintWindow` with `PW_RENDERFULLCONTENT` after two `DwmFlush`), as G2's host does. The suite
    therefore needs `gpu-job --desktop` natively.
  - Captures of strips, fans, loops and indexed draws (`CaptureExpanded`, a new slow path, see
    "Interface changes"): src/capture.rs puts the draw's vertex numbers in list order (odd strip
    triangles and fan triangles ordered to keep the raster block's provoking vertex where it was,
    as the interface's converted fans do; loops closed; the largest index value restarts, as GL's
    draws always do). Indices are read back first (the draw waits for the GPU). The capture program
    then draws points, one draw per run of consecutive vertex numbers, instance after instance, with
    `RASTERIZER_DISCARD`; when rasterising, the original draw follows for the picture. Unit tests of
    the ordering in capture.rs, and the check
    `strips_fans_loops_and_indexed_draws_captured_in_list_order` (Chrome and ANGLE). Still not built:
    a capture whose base vertex takes a vertex number below 0 (device loss naming it).
  - `max_line_width` is 1: wide lines are refused as `Absent(WIDE_LINES)`, so the limit no longer
    reports GL's range.
  - Timestamps: Chrome 157 on the RTX 3080 gives 0 timestamp bits (64 elapsed-time bits) on every
    ANGLE back end it offers: Direct3D 11, Direct3D 11 on 12, Vulkan and OpenGL (`--use-angle=`,
    measured 2026-11-17 10:15). So `TIMESTAMPS` cannot run in Chrome on this machine either.

- G3 run 5:
  - The test tool: angle/build.cmd unpacks mozangle 0.7.1 into C:\wc\tools\mozangle-0.7.1-patched
    and applies angle/d3d11-clip-control.patch. The patch holds three upstream ANGLE fixes to the
    Direct3D 11 renderer, written against mozangle's older code:
    - ba845fcf25d0: the scissor is not mirrored under the upper-left origin.
    - 6092e0b75c22: a change of the clip depth mode reaches the shaders. This change makes it do so
      by marking the viewport dirty.
    - 1dd3a4c4053a: `gl_FragCoord.z` is taken from the built-in position.
    The tool then builds ANGLE from nothing into `%CARGO_TARGET_DIR%\angle-patched`. Natively, every
    test that had failed under EXT_clip_control now passes, apart from `present_shows_the_frame`.
  - tests/against_webgpu.rs: `generate_mips_of_r32float_filtered_in_the_program_on_both_equals_webgpu`.
    It compares this back end without OES_texture_float_linear against the WebGPU back end on a
    device made without float32-filterable (and without adapter-specific format features, as G2's own
    Direct3D comparison makes it). Both must count `FilterInShader`, and every level must be
    byte-equal. The older comparison against WebGPU's hardware filter stays as it was, as a known
    failure.
  - examples/draw-cost.rs: `gpu_cost` is now a JSPI export (`#[wasm_bindgen(jspi)]`) that makes the
    adapter and device with the WebGPU back end's `block_on`. That back end now runs its depth
    probe in the browser and waits for it by suspending. Called from a plain async export, the
    page hung. wasm-bindgen-futures is no longer used. Lint the example for WebAssembly by adding
    `--example draw-cost` to the wasm32 clippy command.

## How to run

All from the checkout root on the Mac.

- Natively the suite presents into a window, so every job that runs it needs `gpu-job --desktop`
  (the queue's own session refuses swap chains); the commands below have it.
- Stale builds: in run 6, cargo still took run 4's WebAssembly build of `wcontainers-gpu` (release
  and debug) as current, and the WebGPU back end then failed to build against it. Cleaning with
  `--target wasm32-unknown-unknown`, and again with `--release`, fixed it.
- The desktop's clock: on 2026-11-17 the target directory held files written at 17:18 desktop time,
  while the clock read 09:27. The clock had gone back by about eight hours. Cargo then took every
  freshly checked-out source as older than its build, and ran stale test programs without saying so.
  The first job of run 5 ran run 4's binaries. If a build finishes at once after a change, clean
  our crates first:
  `tools/graphics-rig/remote "cargo clean -p wcontainers-gpu-webgl -p wcontainers-gpu -p wcontainers-gpu-test -p wcontainers-gpu-webgpu -p wcontainers-graphics-test"`
  (and again with `--target wasm32-unknown-unknown`).

- Everything at once (both scene routes in Chrome and on ANGLE; the suite with the checks in
  Chrome four ways (every extension, without EXT_clip_control, without it and WEBGL_provoking_vertex,
  without WEBGL_stencil_texturing and the BC extensions) and on ANGLE three ways (the third also
  without the BC extensions); the comparisons with the WebGPU back end),
  one GPU job after the build; exit 1 on any failure, so today it exits 1 for the known failures. The
  Chrome suite verdicts land in `<out>\suite-chrome*.txt`:
  `tools/graphics-rig/gpu-job --desktop --build "crates\wcontainers-gpu-webgl\browser\build.cmd" -- "crates\wcontainers-gpu-webgl\browser\all.cmd C:\wc\out\webgl\<name>"`.
  About 60 s as a job once built. Lint as part of the build:
  `tools/graphics-rig/gpu-job --desktop --build "crates\wcontainers-gpu-webgl\browser\build.cmd && cargo clippy -j 3 -p wcontainers-gpu-webgl --all-targets -- -D warnings && cargo clippy -j 3 -p wcontainers-gpu-webgl --lib --example browser --example draw-cost --target wasm32-unknown-unknown -- -D warnings" -- "cargo test -j 3 -p wcontainers-gpu-webgl --lib && crates\wcontainers-gpu-webgl\browser\all.cmd C:\wc\out\webgl\<name>"`
  (`--lib` runs the unit tests of src/capture.rs).
- The per-draw cost of both back ends in Chrome, as an exclusive job (build.cmd builds the
  example, in release, into `%CARGO_TARGET_DIR%\webgl-cost-pkg`):
  `tools/graphics-rig/gpu-job --alone --build "crates\wcontainers-gpu-webgl\browser\build.cmd" -- "node crates\wcontainers-gpu-webgl\browser\cost.mjs %CARGO_TARGET_DIR%\webgl-cost-pkg C:\wc\out\webgl\cost.txt 30 \"10000 50000\""`
  (frames timed per case, then the draw counts). About 15 s.
- The comparisons with the WebGPU back end alone (build.cmd builds them):
  `tools/graphics-rig/gpu-job -- "cargo test -j 3 -p wcontainers-gpu-webgl --test against_webgpu -- --test-threads=1"`.
- The suite in Chrome alone (after build.cmd and a `--list` into a file, as all.cmd does):
  `node crates\wcontainers-gpu-webgl\browser\suite.mjs "%CARGO_TARGET_DIR%\webgl-pkg" <out-file> <list-file> [<extensions off>]`.
- Once per desktop, and again whenever angle/ changes (done in run 5; 4 m 30 s of compiling):
  `tools/graphics-rig/remote "crates\wcontainers-gpu-webgl\angle\build.cmd"` builds ANGLE's
  libEGL.dll and libGLESv2.dll from the patched mozangle crate into C:\wc\tools\angle-mozangle-0.7.1.
  It downloads the crate from static.crates.io with curl.exe, unpacks it with tar.exe and patches it
  with `git apply`.
- The scenes, natively through ANGLE and in Chrome's WebGL 2, each checked against the rig's stored
  Direct3D 11 pictures (crates/wcontainers-graphics-test/references/d3d11, with their tolerances):
  `tools/graphics-rig/gpu-job --build "crates\wcontainers-gpu-webgl\browser\build.cmd" -- "crates\wcontainers-gpu-webgl\browser\run.cmd C:\wc\out\webgl\g3a"`
  Add a second argument to run.cmd to switch extensions off, for example
  `run.cmd C:\wc\out\webgl\g3a-noclip "EXT_clip_control WEBGL_provoking_vertex"`. Exit 1 on any unexpected verdict. About 4
  s as a job once built.
- The shared suite on ANGLE (built by build.cmd too):
  `tools/graphics-rig/gpu-job --desktop --build "cargo test -j 3 -p wcontainers-gpu-webgl --test suite --no-run" -- "cargo test -j 3 -p wcontainers-gpu-webgl --test suite -- --test-threads=1"`.
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
`WebGl::on_angle(Path::new(r"C:\wc\tools\angle-mozangle-0.7.1"), &[])` after running
angle\build.cmd (run 5's version, with the patch). To present into a window, name it first with
`unsafe { b.add_window(n, hwnd) }` and then `create_surface(n)` (since run 6). With every extension, the caps equal Chrome's on
the same GPU, as section 10 asks. Before run 5 this paragraph said to switch EXT_clip_control off,
because the unpatched ANGLE mirrored scissors under it. That is no longer needed: rebuild ANGLE
instead. The `&["EXT_clip_control"]` route still works and is the route of browsers without the
extension. The context is current on the calling thread, which must be the device thread. Natively the surface is
an EGL window surface on the named window (DirectComposition where ANGLE has it); with no surface the
context draws on a 1 by 1 pbuffer.
In the browser: `WebGl::on_canvas(canvas.getContext("webgl2"), &[])`; the canvas is the surface.
Every page that loads the back end's WebAssembly must supply the module `wcontainers-device` (an
import map to browser/device.js does) and call code that may wait through `WebAssembly.promising`.
With every extension both report the same `Caps` on the RTX 3080 (compared on 2026-11-17: identical
lists).

## Capabilities

Declared today (each one built): `READ_BACK_NOW`, `FAN_AND_LOOP` (WebGL 2 topologies),
`PROVOKING_VERTEX_LAST` (GL's rule), `PROVOKING_VERTEX_FIRST` (with WEBGL_provoking_vertex),
`CLIP_DEPTH_MINUS_ONE`, `CLIP_DEPTH_ZERO_ONE` (with EXT_clip_control), `INDEX_U8`, `DEPTH24_UNORM`
(by the probe: set in Chrome 157 and on mozangle on the RTX 3080), `FRACTIONAL_DEPTH_BIAS` (by its
probe: absent in Chrome 157 and on mozangle on the RTX 3080, where ANGLE's Direct3D 11 rounds the
units), `TIMESTAMPS` (with EXT_disjoint_timer_query_webgl2 and timestamp bits above zero: absent
on the RTX 3080, zero bits), `BLEND_CONSTANT_ALPHA` (GLES 3.0 core; mixed with the constant colour
refused, as WebGL requires), `TRANSFORM_FEEDBACK` (since run 4; GLES 3.0 core), `BLOCK_COMPRESSION`
(since run 4, with WEBGL_compressed_texture_s3tc and its sRGB twin, EXT_texture_compression_rgtc and
EXT_texture_compression_bptc: all four in Chrome 157 and mozangle on the RTX 3080). Everything else is
absent, including what WebGL 2 or Chrome's extensions have but this back end does not build yet:
`POINT_SIZE` (WGSL has no point size; a front would need a program variant writing it),
`OCCLUSION_COUNT`, `MULTISAMPLE_READ` and `SAMPLE_SHADING` (WebGL 2 has none of them), and by
extension (all reported by Chrome on the RTX 3080)
`POLYGON_MODE`, `CLIP_DISTANCE`,
`CULL_DISTANCE`, `DEPTH_CLAMP`, `DUAL_SOURCE_BLEND`, `INDEPENDENT_BLEND`, `MIRROR_ONCE`,
`DEPTH_BIAS_CLAMP`, `SAMPLE_SHADING`. The adapter is WebGL's renderer string
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
EXT_color_buffer_float); not 16-bit floats, packed formats, depth or stencil. Since run 3, samples:
1, 2, 4 and 8 for the colour formats that render and are not integers, and for depth formats (as
GL reports them on the RTX 3080); integer formats take one.
No bits yet: 16-bit normalised, BGRA and the other swizzled formats, A8, Stencil8. Since run 4:
the 14 BC formats sample, filter and copy out (read back from the host copy) on every route, and
RGB9E5 copies out too.
Since G3 run 1: `PRESENT` on `rgba8unorm` (the canvas's drawing buffer), and `COPY_OUT` on every
depth format (by `DepthReadByDraw`). Since run 2: `SAMPLE` and `COPY_OUT` on `depth24plus-stencil8`
and `depth32float-stencil8` on every route (stencil natively with WEBGL_stencil_texturing, which
Chrome has here and mozangle lacks, else by `StencilCopiedByDraw`).

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
- The final tree of run 1 (b6a0c43) was checked once more as a whole (job 15:25 to 15:26 desktop
  time): the same verdicts as the run before it, listed next.
- Later the same day (job 15:23 to 15:24 desktop time, queued, not timed), with stencil read backs
  and the third check: Chrome with every extension 23 of 23; without EXT_clip_control 10 of 23,
  without both 10 of 23 (the third check passes on all three, with all five depth formats). ANGLE
  with every extension 20 of 23 (`depth_and_stencil_read_back` fails too: `Depth16Unorm: depth at 5
  is 0x4000, expected 0xbfff`, the scissor bug below); without the extensions 9 of 23 (the check
  passes, depth-only formats only).
- 2026-11-17, G3 run 2, same desktop (RTX 3080, driver 32.0.16.2101; queued jobs, not timed, about
  55 s each between 15:31 and 16:08 desktop time), after merging origin/phase-2-graphics 2684d97,
  origin/phase-2-graphics-webgpu 2fafcb2 and origin/phase-2-graphics-direct3d-12; 21 suite tests and
  6 checks (27). Final tree (job 16:06 to 16:07):
  - Chrome 157.0.8103.78 with every extension: 26 of 27 (`loads_count_levels_from_the_view`
    fails). Without WEBGL_stencil_texturing: 26 of 27, the same one. Without EXT_clip_control: 13
    of 27; without it and WEBGL_provoking_vertex: 13 of 27 (the rest refused by the suite's raster
    blocks, and the same one).
  - ANGLE (mozangle 0.7.1) with every extension: 22 of 27 (`present_shows_the_frame`,
    `the_scissor_bounds_the_draw`, `depth_and_stencil_read_back`, `stencil_sampled_and_read_back`:
    the scissor and pbuffer bugs; and `loads_count_levels_from_the_view`). Without EXT_clip_control:
    12 of 27; without both: 12 of 27 (the stencil and copy checks pass there).
  - Against the WebGPU back end (wgpu 30.0.2, Direct3D 12, same GPU): 2 of 4 pass on both ANGLE
    routes (window origins: the target and 4 mip levels byte-equal; cube, array and 3D textures
    byte-equal); the 2 generate_mips comparisons fail as listed under "Known failures".
  - The 24-bit depth probe sets `DEPTH24_UNORM` in Chrome and on ANGLE: without the bias the depth
    read back 0.09999997, with a one-unit bias 0.10000003 (one 24-bit step).
  - Before the fixes of this run: the stencil check read zeros through stencil views on every route
    (integer textures incomplete without a sampler), and the first level check read level 0 for
    every level of a view (fetches reached only the base level).
  - Lint clean on both targets.
  - Then with `fractional_viewports_are_refused` (job 16:09 to 16:10): every count above one higher
    (Chrome 27 of 28, ANGLE 23 of 28 and 13 of 28); the new check passes on every route.
- 2026-11-17, G3 run 3, same desktop (RTX 3080, driver 32.0.16.2101; queued jobs, not timed,
  about 60 s each between 16:15 and 16:39 desktop time), after merging origin/phase-2-graphics
  528db9c and origin/phase-2-graphics-webgpu 33949c3; 24 suite tests and 11 checks (35). Final tree
  (job ending 16:39):
  - Chrome 157.0.8103.78 with every extension: 34 of 35 (`loads_count_levels_from_the_view`
    fails). Without WEBGL_stencil_texturing: 34 of 35, the same one. Without EXT_clip_control: 19
    of 35; without it and WEBGL_provoking_vertex: 19 of 35 (the rest refused by the suite's raster
    blocks, and the same one).
  - ANGLE (mozangle 0.7.1) with every extension: 30 of 35 (`present_shows_the_frame`,
    `the_scissor_bounds_the_draw`, `depth_and_stencil_read_back`, `stencil_sampled_and_read_back`,
    `loads_count_levels_from_the_view`, as before). Without EXT_clip_control: 18 of 35; without
    both: 18 of 35.
  - Against the WebGPU back end: 2 of 5 (window origins; cube, array and 3D textures); the two
    generate_mips comparisons fail as before, the fractional viewport one as listed below.
  - The occlusion query test passes in Chrome and on ANGLE (with the extensions it needs); both
    multisample tests pass in Chrome and on ANGLE with 4 samples.
  - The new fractional bias probe found the bias lost in Chrome and on ANGLE: `depth16unorm` stored
    step 1000 with and without a bias of 0.1 units (the check found it first, when the feature was
    still declared).
  - `ConstantAlpha` at 0.5 on white gave 127 where 127.5 was expected (this GPU's blend rounds it
    down), so the check allows one step.
  - Lint clean on both targets.
- Per-draw cost, 2026-11-17 16:39 desktop time, as `gpu-job --alone` (RTX 3080, driver
  32.0.16.2101, CPU load 10% just after; Chrome 157.0.8103.78 headless, whose chrome://gpu says
  WebGL and WebGPU are "hardware accelerated but at reduced performance"; both back ends built for
  WebAssembly in release, in one page). The scene: one pass into a 256 by 256 `rgba8unorm`
  target, N quads of 4 vertices from `vertex_index`, each draw naming one uniform binding set or
  each its own (1024 sets over one buffer), then a submit. Timed by the page around each frame's
  recording and submission (`performance.now()`, which Chrome rounds to 0.1 ms here), 5 frames
  untimed then 30 timed, 20 ms between frames. Median per frame, and per draw:
  - 10,000 draws, one set: WebGL 2 1.5 ms (0.150 us a draw), WebGPU 1.6 ms (0.160 us).
  - 10,000 draws, a set per draw: WebGL 2 3.9 ms (0.390 us), WebGPU 3.8 ms (0.380 us).
  - 50,000 draws, one set: WebGL 2 4.9 ms (0.098 us), WebGPU 7.4 ms (0.148 us).
  - 50,000 draws, a set per draw: WebGL 2 16.6 ms (0.332 us), WebGPU 18.1 ms (0.362 us).
  These are the device thread's costs only (both APIs hand the work to Chrome's GPU process, whose
  cost is not in them). An earlier run of the same scene was void: the WebGL device had been lost
  at creation by a bug in the new probe.
- 2026-11-17, G3 run 4, same desktop (RTX 3080, driver 32.0.16.2101; queued jobs, not timed,
  about 75 s each between 16:47 and 17:11 desktop time), after merging origin/phase-2-graphics
  d31a49e and origin/phase-2-graphics-webgpu 2fddc0e; 24 suite tests and 15 checks (39):
  - Chrome 157.0.8103.78 with every extension: 39 of 39. Without WEBGL_stencil_texturing and the
    four BC extensions (BC decoded): 39 of 39. Without EXT_clip_control: 24 of 39; without it and
    WEBGL_provoking_vertex: 24 of 39 (the rest refused by the suite's own raster blocks).
  - ANGLE (mozangle 0.7.1) with every extension: 34 of 39 (`present_shows_the_frame`,
    `the_scissor_bounds_the_draw`, `depth_and_stencil_read_back`, `stencil_sampled_and_read_back`,
    `fragment_depth_follows_the_clip_convention`: the test tool's bugs under EXT_clip_control and
    its pbuffer). Without EXT_clip_control: 23 of 39; without both and the BC extensions: 23 of 39.
  - Against the WebGPU back end (wgpu 30.0.2, Direct3D 12): 4 of 5. The fractional viewport now
    passes, because G2 declares `FRACTIONAL_VIEWPORT`. The integer twin's mips pass since the size
    fix. The r32float mips still fail.
  - At the start of the run, before any change: Chrome 34 of 35 and ANGLE 30 of 35, as in run 3.
  - The capture checks passed the first time they ran, on every route. The new BC check failed
    once, on my own expectation: RGB9E5's green of 0.5 written to `rgba8unorm` gave 127 for 127.5,
    so the check now uses values that round without a tie.
  - Both Chrome and mozangle report the four BC extensions, so `BLOCK_COMPRESSION` is declared on
    both with every extension.
  - Lint clean on both targets (final tree checked at 17:08).
- 2026-11-17, G3 run 5, same desktop (RTX 3080, driver 32.0.16.2101; desktop clock times, queued
  jobs, not timed, about 100 s each between 09:29 and 09:42). Merged origin/phase-2-graphics f107eb5
  and origin/phase-2-graphics-webgpu 5054e1a. The suite has 24 tests and there are 15 checks (39):
  - The first job of the run (before any change) ran stale binaries, because of the clock (see "How
    to run"). Its verdicts were run 4's, and they are not counted.
  - After cleaning, with ANGLE patched (job 09:40 to 09:42):
    - Chrome 157.0.8103.78: 39 of 39 with every extension, and 39 of 39 without
      WEBGL_stencil_texturing and the BC extensions. Without EXT_clip_control, 24 of 39; without
      it and WEBGL_provoking_vertex, 24 of 39.
    - ANGLE with every extension: 38 of 39 (`present_shows_the_frame`). Before the patch it was 34
      of 39. Without EXT_clip_control, 23 of 39; without both and the BC extensions, 23 of 39.
    - All 64 scene verdicts are as expected, so `triangle-clear_rects` now passes on ANGLE with
      EXT_clip_control.
    - Against the WebGPU back end: 5 of 6. The new route-against-route r32float comparison passes;
      the comparison against the hardware filter fails as before.
  - With only the scissor and depth-mode fixes (job ending 09:31), ANGLE with every extension was 37
    of 39: `fragment_depth_follows_the_clip_convention` still read 0.75. The third fix (1dd3a4c4053a)
    made it pass.
  - Lint is clean on both targets.
- 2026-11-17, G3 run 6, same desktop (RTX 3080, driver 32.0.16.2101; desktop clock times, queued
  job 10:43 to 10:50, not timed). Merged origin/phase-2-graphics 41816d5 and
  origin/phase-2-graphics-webgpu eb1e292; 24 suite tests and 16 checks (40), one job of all.cmd on
  the final tree with `--desktop`:
  - Chrome 157.0.8103.78: 40 of 40 with every extension; 40 of 40 without WEBGL_stencil_texturing
    and the BC extensions; 25 of 40 without EXT_clip_control; 25 of 40 without it and
    WEBGL_provoking_vertex.
  - ANGLE (patched mozangle 0.7.1) with every extension: 40 of 40. Without EXT_clip_control 25 of
    40; without both and the BC extensions 25 of 40 (`present_shows_the_frame` passes there too).
  - The 4 unit tests of src/capture.rs pass. All 64 scene verdicts as expected.
  - Against the WebGPU back end: 5 of 6, the r32float comparison against the hardware filter
    failing as before.
  - Lint clean on both targets.
  - Before the window surface used DirectComposition, the test window read 240 in every byte (the
    window's own background painted over ANGLE's blit-model frame) and, with the paint suppressed,
    0 (black). Both were found by `present_shows_the_frame`.
- Per-draw cost, run 5 (Chrome 157.0.8103.78 headless, RTX 3080; `gpu-job --alone`; the same scene
  and method as run 3). Since the merge, the cost page has to make the WebGPU device inside a JSPI
  export. Before that fix the page hung twice, once for 10 minutes and once for 2.
  - First measured run, ending 10:01:35, CPU load 13% just after. The WebGL 2 medians were noisy
    against their fastest frames:
    - WebGL 2, 10,000 draws: one set 6.4 ms (fastest 1.4 ms); a set per draw 10.8 ms (fastest
      3.8 ms).
    - WebGL 2, 50,000 draws: one set 9.6 ms; a set per draw 16.5 ms.
    - WebGPU: 1.6, 3.8, 7.8 and 18.0 ms.
    - Chrome's first launch in this run kept no WebGL context.
  - Second run, alone from 10:06:04, CPU load 10% just after:
    - WebGL 2, 10,000 draws: one set 1.5 ms (0.150 us a draw); a set per draw 4.3 ms (0.430 us).
    - WebGL 2, 50,000 draws: one set 5.7 ms (0.114 us); a set per draw 17.9 ms (0.358 us).
    - WebGPU, 10,000 draws: 1.6 ms (0.160 us) and 4.0 ms (0.400 us).
    - WebGPU, 50,000 draws: 7.8 ms (0.156 us) and 19.1 ms (0.382 us).
  The second run agrees with run 3 to within about 15%. Why the first run's WebGL 2 frames stalled
  was not found. Only the first run followed a Chrome launch that had failed.
- In the first full check of G3 run 1, Chrome's first launch kept no WebGL 2 context in ten tries
  and was closed; the second launch worked. In the other launches one context was lost first.
- Chrome's GPU process restarted once just after Chrome started in most runs (the first WebGL
  context lost, or none made); run.mjs waits for a context to survive half a second first and says
  how many did not.
- The headless Chrome the rig drives reports in chrome://gpu "WebGL: Hardware accelerated but at
  reduced performance" (and the same for WebGPU). Per-draw timings taken there (G3's later work)
  must say so; whether a headed Chrome differs is not measured.

## Known failures

- `generate_mips_of_r32float_without_float_filtering_equals_webgpu` without
  OES_texture_float_linear: 5 of 16 values of level 1 differ from WebGPU's by one float step. The
  built-in mip program filters in the shader (`FilterInShader`, G1's helper) while WebGPU's sampler
  filters in hardware; the order of the arithmetic differs. With the extension (hardware filtering
  on both) they are equal. This comes from the helper, so it is for G1 (helpers.rs is G1's alone).
  Matching it exactly would mean doing the helper's arithmetic in the order this GPU's filter
  does, and other GPUs need not share that order.
  Since run 5 the pass bar's own wording (exact against G2, for a format generateMipmap refuses) is
  also checked route against route. The new
  `generate_mips_of_r32float_filtered_in_the_program_on_both_equals_webgpu` passes: every level is
  byte-equal with the WebGPU back end on a device without float32-filterable. The comparison
  against the hardware filter is left failing, unchanged.
- Fixed in run 5, by patching the test tool: on native ANGLE with EXT_clip_control,
  - `fragment_depth_follows_the_clip_convention` read 0.75 for clip z 0.5. mozangle's pixel shaders
    computed `gl_FragCoord.z` from clip z by the minus-one-to-one formula.
  - `triangle-clear_rects`, the suite's `the_scissor_bounds_the_draw`, `depth_and_stencil_read_back`
    and `stencil_sampled_and_read_back` failed because mozangle mirrored scissors under the
    upper-left origin.
  Upstream ANGLE had fixed both; see run 5 under "Done".
- Fixed in run 6 by the EGL window surface: suite `present_shows_the_frame` on native ANGLE. Before,
  reading this ANGLE's pbuffer default framebuffer
  gives 72 in every byte whatever was drawn into it, even right after a `glClear` of it, with no GL
  error, a complete framebuffer, an RGBA8 configuration with no samples, and reads in RGBA and in
  the implementation's BGRA format alike. In Chrome the same test passes (the canvas read through a
  2D canvas). The pbuffer is no longer presented into.
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

- The suite crate `wcontainers-gpu-test` (G1's) takes proptest without its default features
  (cdcfbc4, a one-line Cargo.toml change): their process forking and timeouts do not build for
  WebAssembly, the suite's host in Chrome needs the crate there, and the crate uses neither. G1
  should keep it.
- Copies from signed normalised formats, and copies into them from formats read wider than stored,
  are not built (a draw cannot recover -128, which samples as -127 does); plan 4.12 has them as
  copies through a buffer.

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
- The last merge of origin/phase-2-graphics in run 1 was undone before it was pushed (see "Next"):
  its suite crate no longer builds for WebAssembly, and the Chrome host of the suite needs it.
- The native test tool is mozangle 0.7.1's ANGLE with three of upstream ANGLE's fixes applied by a
  patch in angle/ (run 5). It is still not Chrome's ANGLE. The patch is ANGLE's own code under its
  BSD-3-Clause licence, applied to a test tool that is never shipped. Before run 5, the native back
  end was to run with EXT_clip_control off; that departure is gone.
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

- The back end's WebAssembly imports `next_task` from a module `wcontainers-device` (JSPI, plan
  4.8): the plan has the device thread suspend through JSPI but names no import; the call channel
  (second batch) should supply the same one. Only query waits use it today; fence and pixel read
  waits still call `finish()`.
- `FRACTIONAL_DEPTH_BIAS` and `TIMESTAMPS` come from probes of the context rather than from the
  extension list alone: GL takes float bias units and EXT_disjoint_timer_query advertises
  timestamps, but ANGLE on Direct3D 11 rounds the one and gives zero bits for the other.
- Copies of multisampled integer, depth, stencil and sRGB textures lose the device (integers as
  needing `MULTISAMPLE_READ`, the others as not built); plan 5.8 names `MultisampleCopyResolved`
  for copies in general.

- Size queries are answered from a hidden uniform holding the bound view's size (run 4), not by
  GL's `textureSize`, because ANGLE answers for the wrong level after a base level change. It costs
  one uniform per texture a program sizes, set only when the bound view changes.
- `capture_vertices` of strips, fans, loops and indexed draws (run 6) draws the capture program
  as points, one draw per run of consecutive vertex numbers, so the vertex stage runs once per
  captured vertex rather than once per vertex. Indexed captures read their indices back (a wait for
  the GPU). With rasterising the original draw runs again for the picture, so the vertex stage runs
  twice; Direct3D runs it once, but a vertex program gives the same results each time. A capture
  past MAX_TRANSFORM_FEEDBACK_INTERLEAVED_COMPONENTS fails
  its link and loses the device with GL's log; the interface has no limit for it.
- BC4 and BC5 decoded without their extension are stored at 8 bits (bcdec_rs's byte output), which
  is coarser than a native decode. Read backs are unaffected (host copy). BC in 3D textures is
  always decoded, since GLES compresses only 2D, array and cube textures.

## Interface changes

- d37c5d9 "interface: said which vertex outputs capture_vertices writes to which target and how
  they are packed": the vertex stage's location outputs in location order as packed 32-bit
  components, no built-ins. With one target they are interleaved; with more, output n goes to
  target n, as OpenGL's two modes. Strips are written as lists, and the targets hold every vertex.
  A doc comment on `capture_vertices` and `Limits::capture_buffers`, and the plan's 4.3 text.

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

- (run 6) "interface: named the slow path of captures expanded into points in list order":
  `SlowPath::CaptureExpanded`, for draws an API's transform feedback cannot capture as they are
  (GLES 3.0 takes only lists without indices), and the plan's 5.8 text for it.

## Installed on the desktop

- C:\wc\tools\wasm-bindgen-0.2.130-x86_64-pc-windows-msvc (20 MB): the wasm-bindgen command, from its release
  archive on github.com/wasm-bindgen/wasm-bindgen (MIT OR Apache-2.0).
- C:\wc\tools\libclang-18.1.1: libclang.dll (84 MB) from the PyPI `libclang` 18.1.1 wheel (Apache-2.0
  with LLVM exception), which mozangle's bindgen needs to build ANGLE.
- C:\wc\tools\angle-mozangle-0.7.1: libEGL.dll and libGLESv2.dll (7.1 MB), built by angle\build.cmd.
- The mozangle build's objects in C:\wc\target\phase-2-graphics-webgl\release (the unpatched build of
  runs 1 to 4, now unused) and C:\wc\target\phase-2-graphics-webgl\angle-patched (run 5).
- C:\wc\tools\mozangle-0.7.1-patched (run 5): the mozangle crate's sources, unpacked and patched by
  angle\build.cmd. Its size was not measured.
- Left by the independent check of the first run (not ours to remove without asking): the pictures
  in C:\wc\out\webgl-check and an empty lock file C:\wc\locks\phase-2-graphics-webgl-check.

## Licences

- glow 0.18.0: MIT OR Apache-2.0 OR Zlib (its Cargo.toml); its slotmap 1.1.1: Zlib.
- bcdec_rs 0.2.1 (run 4, for `BlockDecoded`): MIT (its Cargo.toml; the plan's section 3 records its
  repository's LICENSE). No dependencies.
- khronos-egl 6.0.0 (native only, feature `dynamic`): MIT/Apache-2.0 (its Cargo.toml); its
  libloading 0.8.9: ISC; libc 0.2.191 and pkg-config 0.3.34: MIT OR Apache-2.0.
- naga 30.0.2 (`glsl-out`; `wgsl-in` for the test programs): MIT OR Apache-2.0.
- wasm-bindgen 0.2.130, js-sys and web-sys 0.3.107 (wasm32 only): MIT OR Apache-2.0.
- anyhow (test programs): MIT OR Apache-2.0.
- Tests only, Windows (run 6, the suite's window): windows-sys 0.61.2: MIT OR Apache-2.0 (its
  Cargo.toml; already in the lock file through the WebGPU back end's tests).
- Tests only, WebAssembly (the per-draw cost example): wasm-bindgen-futures 0.4.80, wgpu 30.0.2
  (`webgpu`) and the WebGPU back end of this workspace: MIT OR Apache-2.0 (their Cargo.toml).
- Tests only: getrandom 0.4.3 with `wasm_js` (wasm32, for proptest in the suite crate): MIT OR
  Apache-2.0; on Windows wgpu 30.0.2 (`dx12`) and pollster 1.0.1: MIT OR Apache-2.0 (their
  Cargo.toml), and the WebGPU back end of this workspace.
- mozangle 0.7.1 (the ANGLE build tool in angle/, outside the workspace, never a dependency of the
  back end): BSD-3-Clause (its Cargo.toml and LICENSE, ANGLE's). angle/d3d11-clip-control.patch
  carries upstream ANGLE's changes ba845fcf25d0, 6092e0b75c22 and 1dd3a4c4053a (BSD-3-Clause,
  github.com/google/angle), adapted to mozangle's older code.

## Next

What is left of G3's pass bar cannot be done in this crate:
- G1's suite should take clip depth and provoking vertex from the caps. Until it does, the routes
  without the extensions fail 15 suite tests.
- The r32float comparison against WebGPU's hardware filter stays one float step off (G1's helper).
- `TIMESTAMPS` cannot run on the RTX 3080, which gives zero timestamp bits; a GPU or browser that
  gives some is needed to run that route.

After that, in about this order:
1. Copies into and out of host-kept textures (RGB9E5, BC): from them as host writes (plan 5.8),
   into them by `HostCopyReadBack`.
2. BGRA (`FormatConverted`) and 16-bit normalised formats by EXT_texture_norm16.
3. The `generateMipmap` override where GLES allows it, and `clear_texture` and `clear_rects` by
   `clearBuffer*` (plan 5.8). An override that falls back to the default needs the default's body
   as a public function of `wcontainers-gpu` first (an "interface:" commit).
4. Captures whose base vertex reaches below vertex 0 (now device loss naming it).
5. Suite routes without EXT_clip_control or WEBGL_provoking_vertex stay known failures until G1's
   suite takes clip depth and provoking vertex from the caps.
