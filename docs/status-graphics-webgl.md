# Graphics WebGL 2 back end status (G3)

Track: `crates/wcontainers-gpu-webgl`, the WebGL 2 back end on glow, branch `phase-2-graphics-webgl`.
Plan: docs/phase-2-graphics.md, sections 4, 5.8, 6.5, 10 and 12 (G3).

## Where things stand

G3a (a first slice: real pictures through the interface on WebGL 2) is done. The rig's three
Direct3D 11 reference scenes (triangle; textured, depth-tested, blended quads; an instanced draw
reading a constant buffer) are drawn by small programs that call the interface directly, with WGSL
programs written to GLSL ES 3.00 by naga, in Chrome's WebGL 2 on the RTX 3080 and natively through
ANGLE, and every picture equals real Direct3D 11's exactly. The rest of G3 (section 12's pass bar) is
not started; see "Next".

The shared back-end suite (G1, `wcontainers-gpu-test`) does not exist yet on origin/phase-2-graphics
(checked 2026-11-17: the branch had nothing past the rig merge), so it was not run.

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
    applied ones; a changed block is converted and only the GL values that differ from what GL
    holds are set (state.rs, `GlValues`). Bindings are walked only when the program or a set
    changed, and each texture unit, sampler and uniform block point is rebound only when it differs.
    Vertex attributes are respecified only when the layout, a stream's buffer or its offset changed.
    Clears open the masks and scissor through the same record, and mark the blocks for reapplying.
  - Shaders: naga 30 writes GLSL ES 3.00 (WebGL flavour, `FORCE_POINT_SIZE`); each combined sampler
    gets a unit and each uniform block a binding point at link time; programs are linked once per
    (vertex, fragment) pair and counted per handle. `instance_index` reads
    `gl_InstanceID + naga_vs_first_instance`, set per draw only when it changes. Base vertex and
    first instance move the vertex streams (`BaseVertexRebind`, counted).
  - Window origin (plan 6.5): with EXT_clip_control the device draws upper-left and zero-to-one
    (minus-one-to-one per raster block that asks for it); without it the GLSL's last vertex step
    negates `gl_Position.y` and the front face is swapped. Both routes draw the three scenes exactly.
  - Refused as absent at creation: logic ops, per-target blends, second-source factors, constant
    colour with constant alpha in one blend, depth bounds, per-face stencil masks, fill modes, first
    provoking vertex, depth clamp, bias clamp, wide lines, partial sample masks, storage bindings,
    border and mirror-once samplers, compute, texture views beyond the base ones.
  - Not built yet, and loud when reached (`unimplemented!` naming the missing piece): queries,
    surfaces and present, texture copies and buffer-texture copies, buffer clears, resolves,
    multisampled textures, sRGB-twin and stencil views, empty passes, read backs of formats other
    than RGBA8, vertex formats GL ES cannot read as stored (BGRA8, 10-10-10-2 integer, 11-11-10),
    negative stream offsets after a base vertex, copies between an index-only and another buffer.
- platform.rs: the context. In the browser `WebGl::on_canvas(WebGl2RenderingContext, off)`;
  natively `WebGl::on_angle(dir, off)` loads `libEGL` from `dir` with khronos-egl (EGL 1.0 calls only)
  and makes an OpenGL ES 3.0 context current on a 1 by 1 pbuffer. `off` names WebGL extensions to
  treat as missing, so tests take the routes of browsers without them. `clipControlEXT` (which glow
  lacks) is called through the extension object in the browser and `glClipControlEXT` natively.
  WebGL extension names map to ANGLE's `GL_` names in one table.
- Test programs (examples/): `common/scenes.rs` draws each scene through the interface, generic over
  `Backend`, with the rig's own scene data (scene.rs included by path) and WGSL programs
  (triangle.wgsl from the rig, quads.wgsl and instanced.wgsl here, as the rig's HLSL);
  `webgl-scenes` draws natively on ANGLE and checks pictures; `browser` is the same for WebAssembly,
  driven by browser/run.mjs and page.html.
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

- Once per desktop (done; about 4 minutes):
  `tools/graphics-rig/remote "crates\wcontainers-gpu-webgl\angle\build.cmd"` builds ANGLE's
  libEGL.dll and libGLESv2.dll from the mozangle crate into C:\wc\tools\angle-mozangle-0.7.1.
- The scenes, natively through ANGLE and in Chrome's WebGL 2, each checked against the rig's stored
  Direct3D 11 pictures (crates/wcontainers-graphics-test/references/d3d11, with their tolerances):
  `tools/graphics-rig/gpu-job --build "crates\wcontainers-gpu-webgl\browser\build.cmd" -- "crates\wcontainers-gpu-webgl\browser\run.cmd C:\wc\out\webgl\g3a"`
  Add a second argument to run.cmd to switch extensions off, for example
  `run.cmd C:\wc\out\webgl\g3a-noclip EXT_clip_control`. Exit 1 on any unexpected verdict. About 4
  s as a job once built.
  - build.cmd: installs the rig's browser packages, builds the native example `webgl-scenes` and the
    WebAssembly example `browser`, and runs wasm-bindgen into `%CARGO_TARGET_DIR%\webgl-pkg`.
  - run.cmd: `webgl-scenes angle <dir> <out>\angle` draws on ANGLE and checks (pictures and
    caps-angle.txt there); `node crates\wcontainers-gpu-webgl\browser\run.mjs` draws in the rig's
    headless Chrome and writes `.rgba` texels and caps-chrome.txt; `webgl-scenes check` checks those.
- Lint and format: `nix develop -c cargo fmt -p wcontainers-gpu-webgl` and
  `nix develop -c cargo check -j 2 -p wcontainers-gpu-webgl [--target wasm32-unknown-unknown]` on the
  Mac; clippy on the desktop:
  `tools/graphics-rig/remote "cargo clippy -j 3 -p wcontainers-gpu-webgl --all-targets -- -D warnings && cargo clippy -j 3 -p wcontainers-gpu-webgl --lib --example browser --target wasm32-unknown-unknown -- -D warnings"`.
  The crate has no unit tests yet; its tests are the scene runs above.

How another track uses the back end natively (G4's Windows library on WebGL 2): depend on
`wcontainers-gpu-webgl`, and make the back end on the device thread with
`WebGl::on_angle(Path::new(r"C:\wc\tools\angle-mozangle-0.7.1"), &[])` after running
angle\build.cmd once. The context is current on the calling thread, which must be the device thread.
In the browser: `WebGl::on_canvas(canvas.getContext("webgl2"), &[])`. Either reports the same `Caps`
on the RTX 3080 (compared on 2026-11-17: identical lists).

## Capabilities

Declared today (each one built): `READ_BACK_NOW`, `PROVOKING_VERTEX_LAST` (GL's rule),
`CLIP_DEPTH_MINUS_ONE`, `CLIP_DEPTH_ZERO_ONE` (with EXT_clip_control), `INDEX_U8`,
`FRACTIONAL_DEPTH_BIAS` (`polygonOffset` units are floats), `BLEND_CONSTANT_ALPHA` (GLES 3.0 core;
mixed with the constant colour refused, as WebGL requires). Everything else is absent, including
what WebGL 2 or Chrome's extensions have but this back end does not build yet: `TRANSFORM_FEEDBACK`,
`FAN_AND_LOOP`, `POINT_SIZE`, `TIMESTAMPS`, `DEPTH24_UNORM` (the probe), and by extension (all
reported by Chrome on the RTX 3080) `POLYGON_MODE`, `PROVOKING_VERTEX_FIRST`, `CLIP_DISTANCE`,
`CULL_DISTANCE`, `DEPTH_CLAMP`, `DUAL_SOURCE_BLEND`, `INDEPENDENT_BLEND`, `MIRROR_ONCE`,
`DEPTH_BIAS_CLAMP`, `BLOCK_COMPRESSION`, `SAMPLE_SHADING`.

Limits on the RTX 3080, Chrome 157 and ANGLE alike: 16384 2D and cube, 2048 3D and layers, 12
uniform blocks per stage of 65536 bytes (the plan's 16384 was the Mac), 16 texture units per stage,
16 attributes, stride 255 (WebGL's rule), 8 targets, 30 varying vectors, uniform offsets aligned to
256, points to 1024, lines 1 wide, anisotropy 16, immediates 0 (plain uniforms not built yet).

Formats, from GLES 3.0's table with EXT_color_buffer_float, OES_texture_float_linear and
EXT_float_blend (all present here): every sized colour format GLES 3.0 holds as stored samples;
normalised ones filter, and render and blend unless signed or RGB9E5; 16-bit float and
R11G11B10 filter, render and blend; 32-bit float filter, render and blend (RGB32F does not render);
integers render (not RGB32), never filter or blend; depth formats are depth targets, and those
without stencil also sample. Copy out (read back) only RGBA8 and its sRGB twin. One sample only.
No bits yet: 16-bit normalised, BGRA and the other swizzled formats, A8, Stencil8, block-compressed.

## Measurements

- 2026-11-17, desktop (RTX 3080, driver 32.0.16.2101; other tracks' jobs queued, not timed): the
  three scenes equal real Direct3D 11's pictures exactly (0 of 65536 pixels differ, largest
  difference 0) through Chrome 157's WebGL 2 and through ANGLE (mozangle 0.7.1, Direct3D 11), with
  and without EXT_clip_control. Tolerance needed: none; the rig's recorded allowance (1, 1 and 0
  steps, from WARP) is not used up. Exactness is expected here because ANGLE draws on the same GPU
  through Direct3D 11 and the programs do the same float operations; other GPUs and browsers may
  need the recorded allowance.
- Chrome's GPU process restarted once just after Chrome started in every run (the first WebGL
  context lost); run.mjs waits for a context to survive half a second first and says how many did
  not.

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
- A raster block with `depth_minus_one` false on a device without EXT_clip_control is accepted as it
  is: the interface does not say whether a front that rewrote clip depth (`ClipDepthRewrite`) sets
  the flag, so the back end does nothing with it there.

## Interface changes

- db2c406 "interface: said that a webgl back end refuses a blend block using the constant colour and
  constant alpha together": a doc comment on `Features::BLEND_CONSTANT_ALPHA` and the plan's WebGL 2
  list. WebGL forbids those two factors together in one colour blend (WebGL specification, "Blending
  With Constant Color"); the back end answers `Absent(BLEND_CONSTANT_ALPHA)` for such a block and the
  front blends it in the program.

## Installed on the desktop

- C:\wc\tools\wasm-bindgen-0.2.130-x86_64-pc-windows-msvc (20 MB): the wasm-bindgen command, from its release
  archive on github.com/wasm-bindgen/wasm-bindgen (MIT OR Apache-2.0).
- C:\wc\tools\libclang-18.1.1: libclang.dll (84 MB) from the PyPI `libclang` 18.1.1 wheel (Apache-2.0
  with LLVM exception), which mozangle's bindgen needs to build ANGLE.
- C:\wc\tools\angle-mozangle-0.7.1: libEGL.dll and libGLESv2.dll (7.1 MB), built by angle\build.cmd.
- The mozangle build's objects in C:\wc\target\phase-2-graphics-webgl\release.

## Licences

- glow 0.18.0: MIT OR Apache-2.0 OR Zlib (its Cargo.toml).
- khronos-egl 6.0.0 (native only, feature `dynamic`): MIT/Apache-2.0 (its Cargo.toml); its
  libloading 0.8: ISC.
- naga 30.0.2 (`glsl-out`; `wgsl-in` for the test programs): MIT OR Apache-2.0.
- wasm-bindgen 0.2.130, js-sys and web-sys 0.3.107 (wasm32 only): MIT OR Apache-2.0.
- anyhow (test programs): MIT OR Apache-2.0.
- mozangle 0.7.1 (the ANGLE build tool in angle/, outside the workspace, never a dependency of the
  back end): BSD-3-Clause (its Cargo.toml and LICENSE, ANGLE's).

## Next

G3's pass bar (plan section 12), in about this order:
1. The shared suite from G1, once origin/phase-2-graphics has it.
2. Read backs of every `COPY_OUT` format (float and integer `readPixels` with repacking), then
   texture copies (`blitFramebuffer`, `copyTexSubImage`, `CopyThroughBuffer`), buffer-texture
   copies, buffer clears, resolves and multisampled renderbuffers.
3. Queries (`OcclusionAny`, timestamps with `timestamps_disjoint`), surfaces and present.
4. The extension features of "Capabilities", each with its route and a test; the fast paths
   `draw_fan_or_loop`, `clear_texture`, `clear_rects`, `generate_mips`; transform feedback.
5. BC formats (native or `BlockDecoded`), BGRA by `FormatConverted`, 16-bit normalised by
   EXT_texture_norm16, stencil views (WEBGL_stencil_texturing and `StencilCopiedByDraw`),
   `DepthReadByDraw`, the 24-bit depth probe.
6. The rest of section 12's G3 list, and per-draw cost against G2.
