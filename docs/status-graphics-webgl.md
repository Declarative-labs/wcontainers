# Graphics WebGL 2 back end status (G3)

Track: `crates/wcontainers-gpu-webgl`, the WebGL 2 back end on glow, branch `phase-2-graphics-webgl`.
Plan: docs/phase-2-graphics.md, sections 4, 5.8, 6.5, 10 and 12 (G3).

## Where things stand

G3a (a first slice: real pictures through the interface on WebGL 2) is being built. See "Done" and
"Next".

## Done

- The crate: `WebGl` implements `wcontainers_gpu::Backend` on glow 0.18, the same code in Chrome
  (wasm32) and natively on ANGLE.
  - Buffers (one GL buffer per WebGL type: indices and other data apart, made at creation by usage),
    2D, 2D array, cube and 3D textures made with the GL target of `own_view` and `texStorage`,
    texture writes by `texSubImage` with row length and image height, samplers, binding sets
    (uniform ranges, textures, samplers), passes on cached framebuffers with `clearBuffer*` loads and
    `invalidateFramebuffer` for `DontCare` and `Discard`, direct draws (arrays and elements,
    instanced), `copy_buffer`, read backs later (pixel pack buffer and a sync object) and now
    (`readPixels`, `getBufferSubData`), submit and fences.
  - State applied only where it changed (plan 4.4): each draw compares block handles with the last
    applied ones; a changed block is converted and only the GL values that differ from what GL
    holds are set (state.rs, `GlValues`). Bindings are walked only when the program or a set
    changed, and each texture unit, sampler and uniform block point is rebound only when it differs.
    Vertex attributes are respecified only when the layout, a stream's buffer or its offset changed.
  - Shaders: naga 30 writes GLSL ES 3.00 (WebGL flavour); combined samplers get a unit each and
    uniform blocks a binding point at link time; programs are linked once per (vertex, fragment)
    pair. `instance_index` reads `gl_InstanceID + naga_vs_first_instance`, set per draw when it
    changes. Base vertex and first instance move the vertex streams (`BaseVertexRebind`, counted).
  - Window origin (plan 6.5): with EXT_clip_control the device draws upper-left and zero-to-one;
    without it the GLSL's last vertex step negates `gl_Position.y` and the front face is swapped.
  - Capability list: see "Capabilities" below.
- `platform.rs`: the context. In the browser, `WebGl::on_canvas(WebGl2RenderingContext, off)`;
  natively, `WebGl::on_angle(dir, off)` loads ANGLE's `libEGL` from `dir` with khronos-egl and makes an
  OpenGL ES 3.0 context current on a 1 by 1 pbuffer. `off` names WebGL extensions to treat as missing
  (for tests of the routes without them). The extension calls glow lacks (EXT_clip_control's
  `clipControlEXT`) are made through the extension object in the browser and `glClipControlEXT`
  natively. WebGL extension names map to ANGLE's `GL_` names in one table.

## How to run

All from the checkout root on the Mac.

- Scenes on the desktop's RTX 3080, natively through ANGLE and in Chrome's WebGL 2, each checked
  against the rig's stored Direct3D 11 pictures (references/d3d11):
  `tools/graphics-rig/gpu-job --build "crates\wcontainers-gpu-webgl\browser\build.cmd" -- "crates\wcontainers-gpu-webgl\browser\run.cmd C:\wc\out\webgl\g3a"`
  Add a second argument to run.cmd to switch extensions off (for example `EXT_clip_control`).
  - build.cmd: installs the rig's browser packages, builds the native example `webgl-scenes` and the
    WebAssembly example `browser`, and runs wasm-bindgen into `%CARGO_TARGET_DIR%\webgl-pkg`.
  - run.cmd: `webgl-scenes angle <chrome-dir> <out>\angle` draws on ANGLE from the pinned Chrome's
    own libEGL.dll and libGLESv2.dll and checks; `node crates\wcontainers-gpu-webgl\browser\run.mjs`
    draws in headless Chrome and writes `.rgba` texels and caps-chrome.txt; `webgl-scenes check`
    checks those. Each check also draws the quads with a changed blend factor and a changed depth
    function, which must fail. Exit 1 on any unexpected verdict.
- Lint and format (Mac): `nix develop -c cargo fmt -p wcontainers-gpu-webgl`,
  `nix develop -c cargo check -j 2 -p wcontainers-gpu-webgl` (also with
  `--target wasm32-unknown-unknown`). Clippy: `tools/graphics-rig/remote cargo clippy -j 3 -p
  wcontainers-gpu-webgl --all-targets -- -D warnings`.

How another track uses the back end natively (G4's Windows library on WebGL 2): depend on
`wcontainers-gpu-webgl`, and make the back end on the device thread with
`WebGl::on_angle(Path::new(r"C:\wc\chrome\chrome\win64-<version>\chrome-win64"), &[])`, the folder of
the rig's pinned Chrome (its libEGL.dll and libGLESv2.dll are ANGLE, the same build Chrome draws
WebGL 2 with). The context is current on the calling thread, which must be the device thread. In the
browser: `WebGl::on_canvas(canvas.getContext("webgl2"), &[])`.

## Capabilities

## Measurements

## Departures from the plan

## Interface changes

None.

## Installed on the desktop

- C:\wc\tools\wasm-bindgen-0.2.130-x86_64-pc-windows-msvc: the wasm-bindgen command (its release
  archive from github.com/wasm-bindgen/wasm-bindgen, MIT OR Apache-2.0).

## Licences

- glow 0.18.0: MIT OR Apache-2.0 OR Zlib (its Cargo.toml).
- khronos-egl 6.0.0 (native only, feature `dynamic`, which adds libloading): MIT/Apache-2.0 (its
  Cargo.toml); libloading: ISC.
- naga 30.0.2 (`glsl-out`; `wgsl-in` for the test programs): MIT OR Apache-2.0.
- wasm-bindgen 0.2.130, js-sys, web-sys (wasm32 only): MIT OR Apache-2.0.
- ANGLE (Chrome's libEGL.dll and libGLESv2.dll): BSD-3-Clause, a test tool, never shipped.
- anyhow (test programs): MIT OR Apache-2.0.

## Next
