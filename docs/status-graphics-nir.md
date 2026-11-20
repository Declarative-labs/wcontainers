# Status: graphics, NIR route (G8) and OpenGL natively (G9)

Branch phase-2-graphics-nir. Plan: docs/phase-2-graphics.md, sections 15.2, 15.3, 15.4 and 15.8 (G8,
G9).

## Where it stands

G9 run 7 (2026-11-20). G9 is still far from its bar: OpenGL ES 2.0 now passes nearly all of
dEQP-GLES2 on our driver, but the driver still reports OpenGL ES 2.0 only, so seven of the nine
lists cannot start. The table of lists and shares on our driver, the WHOLE lists (desktop, RTX
3080, GPU queue, 14:57 to 15:02, 8 glcts at once, other tracks' jobs queued beside):
- dEQP-GLES2: 17,165 cases; Pass 16,656, Warn 6, Fail 165, Crash 202, Skip (NotSupported) 136:
  passed 16,662 of 17,029 run, 97.84% (run 6: 85.88%).
- KHR-GLES2: 473 cases; Pass 422, Fail 5, Crash 6, Skip 40: 422 of 433 run, 97.46% (unchanged).
- dEQP-GLES3, dEQP-GLES31, KHR-GLES3, KHR-GLES31, KHR-GLES32, KHR-GL33, KHR-GL46: still cannot
  start (no context of their version). Over the lists of 15.2 together the measure is far below
  92.9%.
- Results: scratch `results/g9r7/full3/<list>/` (`results.csv`, and `refusals.txt`, every
  refusal line of the runner's logs); the two runs before this run's last fix in `full1`, `full2`.
Fixed this run (each from the failing cases' pictures and logs):
- The cascades. A draw with the stencil test off but different front and back references (st/mesa
  passes both as set) reached the WebGPU back end, which takes one reference a draw and treats
  two as a broken rule: the device was then lost, and every later read back in the glcts process
  failed ("finish_read of a read back that is not ready or failed"), so hundreds of cases after
  it failed. The driver now gives one reference where the test is off. This alone moved
  dEQP-GLES2 from 88.81% to 97.84%: most of run 6's Fails were these cascades, not their own.
- Vertex streams wgpu cannot read (a stride not a multiple of 4, an attribute offset not aligned
  to its size or past the stride, a stream bound at an unaligned offset) and instance steps past
  one are fetched in the vertex program (`VertexPulledInShader`, `InstanceRateInShader`, counted
  each draw): the vertex program goes through the shaders crate's `rewrite` with
  `VariantKey::pulled` (route B's `direct::rewrite` now takes it and returns the reflection), the
  streams are bound as storage at the added bindings and the `PulledStream` helper constants are
  written at `HELPER_SET`; the back end's layout leaves those inputs out (one layout per set of
  fetched inputs, kept with the elements state). Every buffer now has storage usage. The screen
  reports instance divisors (ARB_instanced_arrays, which ES 3.0 needs). `vertex_arrays` alone:
  534 of 571 (run 6: 324 Fails in the group).
- A fetched input whose components would lie across words (a float at stride 17) is refused by
  name: the shaders crate's fetch reads each component within one word, which Direct3D's
  alignment rules guarantee and OpenGL's do not (asked of the shaders track, Requests). 76 logs.
- Occlusion queries (`src/query.rs`): any-samples-passed and its conservative form, from the pass
  tracker's queries (one interface query a pass, sets of 4,096 taken anew when half used), read
  back when asked (waiting only if asked to), paused around st/mesa's own blits and clears. An
  exact sample count (desktop GL's SAMPLES_PASSED) is refused by name: the WebGPU back end lacks
  `OCCLUSION_COUNT` and the counting replay is not built. Not yet exercised by any list (the
  query cases are ES 3.0's).
- Constant alpha blending: an alpha factor reading the constant's alpha is now always the constant
  factor (exact), and a colour factor reading it takes the alpha in every channel unless the
  block's colour factors also read the constant's colour; then it is refused by name, unless the
  constant's colour is its alpha. 186 logs remain (run 6: 254).
- The shared NIR step lowers projected samples, rectangles and gathers' offsets for the driver's
  programs too (st/mesa hands them over as written; only the GLSL corpus front lowered them):
  run 6's 136 "a texture operand of another kind" are gone.
- Mesa's builds (Linux and Windows) remove files an earlier patch added before applying the
  patches again (the x86 machine's build had stopped on "already exists in working directory").
- `cts/run-windows.ps1 -Include <regex,...>` runs groups alone (deqp-runner's `--include-tests`).
What is left of dEQP-GLES2 (by group, from `full3`): Crash: texture.mipmap 48 and
texture.specification 12 (most of the 120 "a program not in NIR" logs: a program Mesa hands over
as TGSI, to trace; `tgsi_to_nir` is Mesa's own converter), fragment_ops 93 (the blend refusal),
vertex_arrays.multiple_attributes 37 (components across words), 8-bit indices in a buffer 6,
route B's `fcanonicalize` 8 logs, arrays of arrays of textures 4, Mesa's slots 24 and 25 4. Fail:
texture.filtering 72, fbo.render 35, texture.vertex 28, shaders.invariance 20, texture.mipmap 6,
and 5 single ones: not yet read case by case nor compared with the RTX 3080's own OpenGL.
KHR-GLES2's: internalformat 5 Fail and 5 Crash, copy_sub_image 1 (the TGSI program, 12 logs).
Next, in order: (1) OpenGL ES 3.0, which every other list needs: transform feedback through the
shaders crate's stage helper (`Stages::new` takes a `Converted`, which `direct::rewrite` gives),
the primitives-written query, primitive restart at the fixed index (the screen's cap; lists drop
the restarted primitive), then the GLSL 3.30 and ESSL 3.00 caps and the formats st/mesa checks;
(2) the TGSI program (trace which, then `tgsi_to_nir`); (3) read the GLES2 Fail groups case by
case against the RTX 3080's OpenGL (`-Driver system`); (4) blending in the program for the
constant's colour and alpha; (5) the literal check (not run this run).
Seen, not yet acted on: the screen reports fewer vertex formats than OpenGL has (three-component
16-bit and 8-bit ones), so cso's `u_vbuf` likely translates those streams on the CPU, through the
driver's maps, uncounted. To check (whether `u_vbuf` is made for this screen) and, if so, to take
those formats by the fetch in the program instead, counted.

G9 run 6 (2026-11-20). G9 is still far from its bar, but OpenGL ES 2.0 now mostly works on our
driver. The table of lists and shares on our driver, the WHOLE lists (desktop, RTX 3080, GPU
queue, 14:00 to 14:07, 8 at once; the desktop's processors at 8% just after; other tracks' jobs
queued beside):
- dEQP-GLES2: 17,165 cases; Pass 14,619, Warn 5, Fail 2,143, Crash 262, Skip (NotSupported)
  136: passed 14,624 of 17,029 run, 85.88% (run 5, 1% shard: 23.98%).
- KHR-GLES2: 473 cases; Pass 422, Fail 5, Crash 6, Skip 40: 422 of 433 run, 97.46% (run 5: 0 of
  5).
- dEQP-GLES3, dEQP-GLES31, KHR-GLES3, KHR-GLES31, KHR-GLES32, KHR-GL33, KHR-GL46: still cannot
  start (the driver reports OpenGL ES 2.0 only; glcts makes no context). Over all the lists of
  15.2 the measure is therefore far below the 92.9% floor; no list but the two above has a case
  run.
- Results: scratch `results/g9r6/full/` (both `results.csv` and the runner's output); 1% and 10%
  shards on the way in `results/g9r6/s2` to `s10`.
Fixed this run (each found by reading the failing cases' pictures and the runner's logs):
- Uniforms: st/mesa hands every program's uniforms over as `load_ubo` (the default uniforms as
  block 0). The bridge's `wc_nir_uniform_blocks_as_words` (called once when st/mesa hands a
  program over) turns every block access into derefs of one array of words a block, the step
  the GLSL corpus already used, sized by the block variables' explicit types (the default
  block's variable is one array of vectors, not an array of blocks), and drops the default
  uniforms' own variables, which nothing reads after. 82 of run 5's 94 crash logs.
- Everything was drawn upside down: route B takes a program's clip space as Vulkan's and
  negates y itself, so the driver now negates y only for a Gallium viewport whose y scale is
  negative (window-system framebuffers), where it negated it for a positive one.
- A sampler view's target and swizzle were read wrongly on Windows: bindgen lays bit fields out
  as the Itanium ABI does, and Visual C++ starts a new unit where `pipe_sampler_view` puts a
  `bool` between its enums, so every view read target 0 (a 2D view: wgpu refused the 3D and cube
  textures, "Invalid texture view dimension D2 with texture of dimension D3", which run 5 had
  asked of the WebGPU track; it was ours). Both are now read in C by the compiler that lays them
  out (`bridge/fields.c`, built with Mesa's flags by `cc`). The other Gallium structs the driver
  reads keep one width a run of bit fields (checked: pipe_draw_info, pipe_vertex_element,
  pipe_framebuffer_state, pipe_surface, pipe_resource, the state blocks).
- Draws whose stencil faces differ in masks or reference on a back end with one of each per
  draw (WebGPU) are split by face (`StencilFacesSplit`): blocks a face, the rasterizer's blocks
  culling one face each; points and lines take the front's; a draw culling one face takes the
  other's only.
- Convergent fragment inputs: Mesa's varying optimiser made them flat, which the host takes from
  the first vertex while OpenGL's default is the last; the NIR options now keep them interpolated
  (`nir_io_always_interpolate_convergent_fs_inputs`, exact for a value that is the same at every
  vertex). Mesa then interpolates them linearly, and the host requires a vertex output and the
  fragment input it meets to be interpolated alike, so the vertex program is converted for the
  fragment program it meets: its outputs take the fragment inputs' interpolation (the variant
  holds them; OpenGL's fragment stage alone decides).
- Arrays of textures and samplers read at constant indices (OpenGL ES 2.0's only kind) are one
  variable an element (`wc_nir_split_resource_arrays`): the host has no arrays of bindings.
- Fans and loops drawn from an index buffer read the indices back (`ReadBackWait`) and convert
  them as the program's own.
- A view whose target has one layer reads one layer (st/mesa gives a cube's whole range).
- The Windows build fetches the DirectX Shader Compiler (v1.9.2609, `dxc_2026_09_29.zip`, sha256
  ad31b1fc...a7e7f1) and installs `dxcompiler.dll` and `dxil.dll` beside the driver, so wgpu's
  Direct3D 12 back end compiles with DXC, as Chrome does, and not with FXC, which failed to
  unroll loops ("X3511") and hit an internal error ("X8000: Abs modifier not allowed").
- Route B samples at level 0.0 outside comparisons (naga's HLSL writer writes `SampleLevelZero`,
  which HLSL has only for comparisons).
What is left, by what the whole run shows (grouped by the runner's results; the Fail groups are
not yet read case by case):
- Crashes (refusals by name): blending with both the blend constant's colour and its alpha in one
  block, 254 logs (BlendInShader is not built); "not taken: a texture operand of another kind",
  136 (route B); "a program not in NIR", 132 (a program st/mesa hands over as TGSI: to trace);
  8-bit indices in a buffer, 6; arrays of arrays of textures, 4; stage values at Mesa's slots 24
  and 25, 4.
- Fails: shaders 550, clipping 430, uniform_api 325, vertex_arrays 324, fbo 118, fragment_ops 111,
  texture 106, draw 72. Many are not their own: one object wgpu finds invalid makes the whole
  submit fail, and every case after it in the same glcts process reads back nothing ("finish_read
  of a read back that is not ready or failed"). The cause seen most: a vertex stream whose stride
  is not a multiple of 4 ("Vertex buffer 0 stride 2 does not respect VERTEX_ALIGNMENT"), which
  the WebGPU back end accepts and wgpu refuses (asked under Requests). The clipping cases seen
  (`clip_one.clip_neg_x_pos_y`) draw nothing in the triangle's area, likely such a cascade; to be
  rerun alone.
- NotSupported (Skip) groups, not yet listed with what each asks: dEQP-GLES2 fbo 91, the rest
  small; KHR-GLES2 ASTC 3D textures 30 and internal formats 10.
- Next, in order: (1) rerun the failing groups alone to separate cascades from real failures,
  and give each a reason; (2) the vertex stride (WebGPU track) or a refusal by name in the
  meantime; (3) BlendInShader for constant colour and alpha; route B's texture operand; the TGSI
  program; (4) OpenGL ES 3.0 (queries, transform feedback through the stage helper, instanced
  arrays, primitive restart, then the GLSL and ESSL caps), which every other list needs; (5) the
  literal check's 36 findings in wcontainers-opengl (Literal check).

G9 run 5 (2026-11-20). G9 is still far from its bar. This run found the heap corruption that
crashed llvmpipe and our driver alike, and fixed the largest groups of our driver's crashes:
- The heap corruption (exit 0xc0000374, every crash that compiled a program, on llvmpipe and on
  our driver) is explained. LLVM's own Windows release is built with LLVM's integrated allocator:
  its `LLVMSupport.lib` holds rpmalloc (checked: the library holds `rpmalloc_initialize`), which
  replaces malloc and free in whatever DLL links it, here `libgallium_wgl.dll`, while the C
  runtime's `_aligned_realloc` still grows a block in place in the Windows heap (`_expand`).
  Mesa grows a program's parameter storage with `_aligned_realloc` on Windows
  (`os_memory_stdc.h`), so the first program whose parameters outgrew their first block broke
  the heap. Found by running one crashing case (dEQP-GLES2 `shaders.conversions.matrix_combine.
  vec3_vec3_vec3_to_mat3_fragment`, which crashes alone on both drivers) under lldb with the
  build's PDB files: the heap's check stopped in `_aligned_realloc` from
  `_mesa_reserve_parameter_storage` (prog_parameter.c:237); and by two AddressSanitizer builds
  (which take over every allocation): softpipe without LLVM and llvmpipe with it both pass the
  case. Our driver uses no LLVM but crashed the same way because it shares the DLL. Fixed by
  building LLVM 21.1.8 from its source on the desktop without the integrated allocator
  (`build-windows.ps1`); llvmpipe links that. With it, 1% shards on llvmpipe (desktop, GPU queue, 12:50 to
  12:53, other tracks' jobs queued beside): dEQP-GLES2 172 of 172 (Pass 171, Warn 1), dEQP-GLES3
  447 of 447 run (Pass 444, Warn 3, Skip 1), KHR-GLES2 5 of 5, KHR-GL33 99 of 99: every case run
  passes (run 2: 64.53%, 57.05%, 100%, 75.76%). The case alone passes on llvmpipe and reaches
  our driver's next refusal (below). LLVM's build: about 22 minutes at 8 jobs (2,271 steps,
  12:20 to 12:42, desktop shared with other tracks), 0.5 GB installed.
- `set_polygon_stipple` (every KHR-GLES2 case and most dEQP-GLES2 crashes in run 4: st/mesa sets
  the pattern at every context's start) keeps nothing; a rasterizer state that stipples is
  refused by name.
- The provoking vertex: GL's default (last) was refused at the rasterizer state on the WebGPU
  back end, which has only the first. The block now takes the convention the back end has, and a
  draw is refused by name only when its fragment program reads a flat value
  (`ProvokingRewrite` is not built). OpenGL ES 2.0 has no flat values.
- Point sizes and line widths: the back end draws points and lines of one size (`max_point_size`
  and `max_line_width` 1, no `POINT_SIZE` or `WIDE_LINES`). OpenGL clamps a size to the range the
  driver reports, so the vertex program's point size output is removed in NIR
  (`PositionTransform::fixed_point_size`, which wgpu's HLSL writer refused as "Unsupported builtin
  PointSize"), and the screen reports the one line width as both ends of the range (st/mesa had
  passed widths the back end refused).
- Tools: `run-case-windows.ps1 -Debugger` (lldb with symbols), the Mesa build keeps PDB files.
- 1% shard of dEQP-GLES2 and KHR-GLES2 on our driver before the LLVM fix (desktop, GPU queue,
  12:01 to 12:04, beside other tracks' jobs), with the stipple and provoking fixes but before
  the point size, line width and LLVM fixes: dEQP-GLES2 172 cases, Pass 21, Fail 71, Crash 79,
  Skip 1: 21 of 171 run, 12.28% (run 4: 21 Pass, 150 Crash). KHR-GLES2 5 cases, all Crash (3D
  textures, below). The crashes by reason (deqp-runner's logs, retries included): heap
  corruption 84 logs; the wide-line refusal 18; wgpu's HLSL writer refusing PointSize 18; the
  2D view of a 3D texture 10. Every Fail seen follows a pipeline wgpu found invalid (the
  PointSize refusal), whose submit then failed and left the read back unfinished ("finish_read
  of a read back that is not ready or failed").
- Textures and samplers stay variables in every program (`nir_samplers_as_deref`, as Zink): the
  programs Mesa makes from TGSI (the blitter's) had lost them, and route B refused "a texture
  instruction without a texture deref".
- Constant alpha blending on a back end without `BLEND_CONSTANT_ALPHA`: a block that uses no
  constant colour takes the constant factors and the draw's blend constant is the alpha in all
  four channels (exact, as the interface says); a block with both is refused by name (blending
  in the program is not built). Mesa's own rasterizer states (clears and blits, width zero) and
  every other state take the one line width the back end draws.
- 1% shards on our driver at the end of the run (desktop, GPU queue, 12:57 to 12:58): dEQP-GLES2
  172 cases, Pass 41, Fail 62, Crash 68, Skip 1: 41 of 171 run, 23.98% (run 4: 12.28%).
  KHR-GLES2 5 cases, all Fail (3D textures). The crashes by reason (logs, retries included):
  `load_ubo` not taken by route B 82 of 94 logs (every program with uniforms: st/mesa hands
  default uniforms over as `load_ubo` of block 0, which the GLSL corpus never had: its programs
  keep uniform derefs); both the blend constant's colour and alpha 4; a cube view of 6 layers
  wgpu refused 2; "bindings: more resources of a kind than its range holds" 2. The 62 Fails (45
  shaders, 10 fragment_ops, 5 clipping, 2 buffer) have no reason recorded yet: their pictures
  are to be read next run. KHR-GLES3, dEQP-GLES3, dEQP-GLES31, KHR-GLES31, KHR-GLES32, KHR-GL33
  and KHR-GL46 cannot start on our driver: it reports OpenGL ES 2.0 and GLSL 1.20 only, so glcts
  makes no context ("Failed to create WGL context") and deqp-runner stops.
- Next, in order: (1) `load_ubo` in route B (a uniform block as an array of 16-byte words in the
  common form, each component loaded at its byte offset and bitcast; dynamic offsets as
  expressions), with a corpus test of a program st/mesa lowered; (2) the reasons of the 62
  Fails (read their pictures in the `.qpa` logs; compare with the RTX 3080's OpenGL); (3) the 2D
  view of a 3D texture and the cube view of 6 layers; (4) OpenGL ES 3.0: queries (through the
  pass tracker's occlusion segments, as the Direct3D 11 front does), transform feedback
  (`capture_vertices` is absent on WebGPU: the shaders crate's stage helper,
  `StreamOutAsCompute`), instanced arrays, primitive restart, then the GLSL and ESSL caps; (5)
  the whole lists in shards and the table; (6) 4.6; bgfx's and Godot's OpenGL renderers on the
  rig.
- Found, not yet traced: KHR-GLES2 `texture_3d` cases end on a 2D view of a 3D texture made in
  wgpu (Requests, below), then route B's refusal of "a texture instruction without a texture
  deref" (a program Mesa builds itself, likely the blitter's).

G9 run 4 (2026-11-20). G9 is still far from its bar, but our driver now has a context and passes
its first conformance cases. This run:
- The context (`crates/wcontainers-opengl/src/`, all Rust on the Gallium bindings, generic over
  the back end; the screen too, `Screen<B>`), by file:
  - `resource.rs`: buffers (every use, a whole number of storage words) and textures (1D as 2D
    one texel high, cubes, arrays, 3D) on the interface's; Gallium's references; maps as copies
    (a read map reads back now, `ReadBackWait`; a write map writes on unmap or on each flushed
    range, with the write mode Gallium's flags give: discard whole, unsynchronised as
    `NoOverwrite`, otherwise `Ordered`); `buffer_subdata` and `texture_subdata` written from the
    frontend's bytes with no copy; depth and stencil texels split into and joined from the two
    aspects the interface moves. A map of a multisampled texture is refused (null, OpenGL's out
    of memory). Objects whose owner went are given back to the back end at its next use (Mesa
    destroys resources from inside the driver's own calls).
  - `context.rs`: the context with st/mesa's upload buffers (`u_upload_create_default`), the
    framebuffer as the pass tracker's targets, clears through the tracker (full clears as the
    pass's load) and `clear_rects` (surface clears), flush and fences, texture and memory
    barriers (the open pass ends). Entry points not built end the process with their name
    (`unbuilt!`: queries, render conditions, geometry, tessellation and compute stages, stream
    output, polygon stipple), never a call that does nothing.
  - `state.rs`: blend, depth and stencil, rasterizer, sampler and vertex element states as the
    interface's blocks (blend built per framebuffer at the draw: no blending on integer
    targets); programs kept as st/mesa's NIR. What a block cannot hold is refused by name
    (different fill modes per face, a sampler's LOD bias, advanced blend equations, client-memory
    vertex streams, a vertex format the screen does not report).
  - `draw.rs`: each stage converted by route B (`direct::module`) at its first draw for its
    variant, then `create_shader` and `create_program`, cached by shader and variant. The
    variant: the vertex program's position moved to the host's clip space (`wc_nir_transform_
    position`: y negated where Gallium's viewport scale is positive, depth from -w..w to 0..w,
    `ClipDepthRewrite`, and w - z for a depth range whose near value is the larger,
    `ViewportDepthFlip`), and each bound view's swizzle applied in the program
    (`wc_nir_swizzle_textures`, `SwizzleInShader`). Uniform blocks bound by the route's
    reflection (the frontend's bytes through the ring, grown to the declared size), textures
    and samplers likewise; indices from the frontend through the ring (8-bit widened where the
    back end lacks them, `IndexWidened`), fans and loops through `draw_fan_or_loop`. Refused by
    name: indirect draws, fans and loops from an index buffer, restart at another index.
  - `view.rs`: sampler views, `resource_copy_region` (`copy_buffer`, `copy_texture`),
    `generate_mipmap` (the interface's `generate_mips`). `blit.rs`: blits through Gallium's
    blitter, which draws through the driver's own draws (the state saved as Mesa's inline
    helpers save it). `present.rs`: `flush_frontbuffer` shows the frame in the device
    context's window (a `wgpu` surface over `WindowFromDC`'s window, a copy drawn into the
    surface's texture).
- wcontainers-nir gained the driver's NIR steps in its bridge: a copy of a program, vertex
  elements by rank and gl_FragColor to every buffer (moved from `glsl.cpp`, which now calls
  them), the position transform and the texture swizzles.
- The screen now reports what st/mesa needs for OpenGL ES 2.0 and what the context serves:
  separate blend equations, independent blending where the back end has it, mixed framebuffer
  sizes (the tracker fits them), seamless cubes, LOD in fragment programs, instance ids, packed
  uniforms, the host's fragment coordinates, both clip depth spaces; and leaves to st/mesa what
  the host lacks (flat shading, alpha test, two-sided colour, user clip planes, rectangle
  textures, GL_CLAMP). Maps are never persistent. GLSL 1.20 and ESSL 1.00.
- Windows: bindgen now allows the undeclared intrinsics of Mesa's Visual C++ atomics (inside
  inline bodies the bindings never hold), and the driver compares Gallium's enums the same where
  C enums are signed (Visual C++) and where they are not.
- First case on our driver: `dEQP-GLES2.functional.color_clear.single_rgba` passes (desktop,
  through the GPU queue, 11:15, 0.7 s; frames shown in the window).

G9 run 3 (2026-11-20). G9 is still far from its bar: our driver has no context yet, so it
passes no conformance case and there is still no table of lists and shares on it. This run:
- The hang under glcts is found and fixed. glcts unloads `opengl32.dll` before it exits; Mesa's
  WGL target then destroys its screen inside `DllMain` (`stw_cleanup`, under the loader lock), and
  releasing the `wgpu` Direct3D 12 device there never returns (shown by printing each step: the
  device was made, the context refused, the screen's destroy entered, the back end's drop never
  left). The driver's module now pins itself once it holds a device (`GetModuleHandleExW` with
  `GET_MODULE_HANDLE_EX_FLAG_PIN`, `src/device.rs`), so it is only detached at the process's end,
  where Mesa releases nothing. With it the same case ends at once (glcts reports that no pixel
  format fits and falls back to EGL, which this build lacks; the honest answer while no format is
  reported). Not a slow path: the device simply lives until the process ends.
- `cts/run-case-windows.ps1` runs one case alone with a time limit and prints glcts's output, its
  exit code and the case's log (for hangs and crashes); the two run scripts share
  `cts/cts-windows.ps1`.
- The screen is now Rust (`src/screen.rs`), written against bindgen's bindings of the pinned
  Gallium headers (`bridge/gallium.h`, made by `build.rs`), as the plan says; the C screen is
  gone. It reports formats from the interface's `FormatCaps` (`src/format.rs`: every Gallium format
  the interface holds, and what each Gallium binding needs), the vertex and fragment stages' shader
  caps from the interface's limits, and wcontainers-nir's NIR options (`wc_compiler_options`) for
  every stage, so st/mesa will hand shaders over in route B's shape. Contexts are still refused
  by name.
- wcontainers-nir builds only its bridge into the driver: its new default feature `standalone`
  (the SPIR-V steps, now in `bridge/spirv.c`, the GLSL front and Mesa's libraries, for the corpus
  and tests) is off in wcontainers-opengl, so Mesa is not linked twice into `libgallium_wgl.dll`.
  `Nir::from_raw` takes the NIR a Gallium shader state hands over, and `mesa::compiler_options`
  the options (a crate the driver never names is not linked, so the bridge is reached through
  its Rust). The Windows Mesa build writes the same `wcontainers-made-from` record as the Linux
  one (the converter's version), fetches the shaders crate's submodules, and builds route B with
  the Visual C++ compiler (`flat.c` had a variable-length array, which it lacks; now allocated).
- Checked on the desktop through the GPU queue, 2026-11-20 10:11 to 10:12, beside other tracks'
  jobs: the Mesa build with the Rust screen links (42 s incremental, 8 jobs); the case alone on
  our driver now reaches the context (WGL found pixel formats) and is refused by name, then
  glcts stops at its EGL fallback, 0.5 s, exit -1; the same case on llvmpipe passes (0.4 s);
  1% shards of KHR-GLES2 and dEQP-GLES2 on our driver: every case Missing or no results, each
  process ending at once (no hang). Lint clean on the desktop (clippy with Mesa,
  `wcontainers-opengl` and `wcontainers-nir` without `standalone`) and on the x86-64 machine
  (with and without Mesa, both features of wcontainers-nir); wcontainers-nir's tests pass there
  (route 11, narrow 2, glsl_buffers 1, glsl_reference 10 and 2 ignored as known failures).

G9 run 2 (2026-11-20). G9 is far from its bar. This run: the one conformance release and its
lists chosen, glcts and deqp-runner built on the desktop, the harness proven through the GPU
queue on llvmpipe and on the RTX 3080's own OpenGL (1% of every list), the Mesa patch adding our
Gallium driver to the WGL target, and the driver's first slice (its screen, built into
`libgallium_wgl.dll`). Our driver passes no case yet: its context does not exist, and its screen
hangs under glcts (G9, below). The table of lists and shares on our driver: none yet.

G9 run 1 (2026-11-20). G9 is started, far from its bar: no OpenGL driver of ours exists yet, so no
case of 15.2's lists has run and there is no table of lists and shares to give (G9's report, below,
says what each line of it waits on). This run settled the findings of G8's third check (listed
next) and took G9's first step on the desktop (G9, below).

G8's third check, settled in G9 run 1:
- GLSL `!(x < y)` beside `!(x >= y)` gives false on a NaN where llvmpipe gives true. Explained,
  not changed, and kept as a known failure (`a_glsl_program_negating_both_comparisons_of_a_nan_
  matches_llvmpipe_on_route_b`, ignored with the reason). Mesa's GLSL front (`glsl_to_nir.cpp`,
  unchanged) gives `<` and `>=` no NaN preservation (only `==`, `!=`, min and max get it), so
  `nir_opt_algebraic`'s `inot(flt(a, b)) -> fge(a, b)` (when the `flt` is used once) is Mesa's own
  legal fold for GLSL, which leaves NaN comparisons to the implementation. In the bridge the fold
  is already in the NIR Mesa's linker hands back (its `gl_nir_opts`, run by `gl_nir_link_glsl`),
  before any step of ours: marking every operation NaN-preserving before the bridge's own loop
  (tried, then taken out) changed nothing. llvmpipe's own compile (`MESA_GLSL=dump`) keeps the
  `flt` used twice (by the `inot` and by a `bcsel` its loop made), so the rule never fires there.
  Which of the two drivers' compiler options makes that difference was not found. G9's driver runs
  st/mesa itself with this driver's options and settles it: its NIR for this program is to be
  compared with the bridge's.
- The literal check's `CUBE_FACES` finding is now listed under "Waiting on others".
- A vertex output the reference captures and the route's module lacks is now a difference when
  the program can write it (`interface::reaches`: a store or copy into it, an access chain from
  it, for a block member one whose first index is that member, or a function call given it); an
  output the program never reaches holds no defined value and is still left out.
- Godot's `scene_forward_clustered.glsl/10.vert` on seed 8675309: reproduced (route B 17 values
  differ by about 1e-4 relative, route A matches; both match on 8675310 and 24301). Fusing is ruled
  out: with route B's `ffma_weak` written as a separate multiply and add (temporary, on the x86
  copy) it still differs on 8675309 and matches on the other two. Not further explained; probably
  cancellation amplifying a difference of operation order between route B's module and lavapipe's
  own compile of the SPIR-V. Left a listed seed-dependent difference.
- A child that dies by a signal in the comparison is now a difference when the reference had
  ended (the route's module crashed the driver, as a hang there already was), and "the reference
  crashed on seed n" when it had not, which is tried on the next seeds as a reference that does
  not end is. A child ended by a panic of the harness stays not compared. Checked:
  `texturemipmapgen/texture.frag.spv` (GLSL) on seed 8675309: the reference crashes, route B is
  held on 8675310 and matches.
- The GLSL reference tests read `WC_NIR_SEED` (the first seed) and `WC_NIR_INPUT_SETS` (how many
  seeds, one input set each; default 1) and print both.
- "Licences brought in" made current (the linked list again, below).
- Route B's `umul_high` and `imul_high`: the bridge's options already ask for them lowered
  (`lower_mul_high`), but only `nir_lower_alu` lowers them, which `gl_nir_opts` runs for GLSL and
  the SPIR-V path's loop did not; it now runs in that loop. Held by `tests/narrow.rs`'s second
  test: SPIR-V's `OpUMulExtended` and `OpSMulExtended` written word by word, run natively by
  lavapipe and through route B, both held to Rust's 64-bit products (65,600 cases at the default:
  the 36 pairs of six edge values of both signs and 65,536 seeded pairs, `WC_NIR_SEED` and
  `WC_NIR_INPUT_SETS`; 0 differ, x86-64 machine).
- The other two findings were information (route B's seed-dependent outcomes on 8675309 fall in
  listed classes), recorded here.

G9 (OpenGL natively), where it stands:
- The desktop has Visual Studio Build Tools 17.14.37906.3 (`install.rst` asks for 17.9 or later)
  and the Windows 10 SDK 10.0.26100.0 (at least 20348); no Python (only the Store's stub), Meson,
  Ninja, flex, bison or LLVM for the Visual C++ runtime were there.
- `crates/wcontainers-nir/mesa/build-windows.ps1` (with `options-windows.ini`): fetches pinned
  build tools into `C:\wc\graphics-nir\tools` (Python 3.13.1 from its NuGet package with Meson
  1.12.1, Ninja, Mako, PyYAML and packaging; win_flex_bison 2.5.25; LLVM 21.1.8 built for the
  Visual C++ runtime) and builds the pinned Mesa with the same patches as the Linux build: the WGL
  frontend's `opengl32.dll` with llvmpipe, OpenGL ES through WGL. Built in G9 run 1:
  `opengl32.dll` and `libgallium_wgl.dll` in `C:\wc\graphics-nir\mesa\install\bin` (the build
  was not timed; nothing has drawn through it yet). Two fixes were needed on the way: win_flex's
  temporary files clash when two run at once (its outputs are now made first, one at a time),
  and LLVM's Windows release is built for the static runtime (`b_vscrt = 'mt'`).
  The downloads' SHA-256: python-3.13.1.zip eba9c88a...ac2df, win_flex_bison-2.5.25.zip
  8d324b62...ac135, clang+llvm-21.1.8-x86_64-pc-windows-msvc.tar.xz 749d22f5...21a47 (full in
  the log, scratch `logs/g9r1-mesa-windows.log`); to be checked by the script once the owner
  accepts the tools.
- G9 run 2: the conformance release is chosen: VK-GL-CTS `opengl-cts-4.6.8.1`, commit
  067e8832; `opengl-es-cts-3.2.14.1` names the same commit, so one release holds every list
  (`crates/wcontainers-opengl/cts/pin.toml`; the pinned Mesa's own CI uses these two tags too).
  Its lists for 15.2 (`cts/lists.txt`), with their case counts: dEQP-GLES2 17,165, dEQP-GLES3
  44,768, dEQP-GLES31 37,802 (`gles/aosp_mustpass/main/gles*-main.txt`), KHR-GLES2 473,
  KHR-GLES3 6,498, KHR-GLES31 4,101, KHR-GLES32 1,405 (`gles/khronos_mustpass/main/
  gles*-khr-main.txt`), KHR-GL33 9,886 and KHR-GL46 19,714 (`gl/khronos_mustpass/main/`). The
  seven GLES lists hold 112,212 cases, exactly the total gex_wgpu's published GLES share is over
  (15.2). Each list runs with its `mustpass.xml`'s first configuration (the CTS's own arguments,
  read from the file, not retyped). deqp-runner v0.23.3 (commit 0fc69278, the pinned Mesa's).
- `cts/build-windows.ps1` fetches the release and the sources its `fetch_sources.py` pins, builds
  `glcts` (WGL platform, `DEQP_TARGET=default`) with the Visual C++ compiler and Ninja, and
  `cargo install`s deqp-runner, all under `C:\wc\graphics-nir`. `cts/run-windows.ps1 -Driver
  <gallium driver> [-Lists <names>] [-Fraction n -FractionStart k]` copies Mesa's DLLs beside
  glcts, runs each list (or every n-th case from the k-th, a shard) through deqp-runner and prints
  each result's count and 15.8's measure. The tools both Windows scripts share are in
  `crates/wcontainers-nir/mesa/windows-tools.ps1`.
- The Mesa patch for the driver (`mesa/patches/wcontainers-gallium-driver.patch`): `wcontainers`
  in `gallium-drivers`, `with_gallium_wcontainers`, the driver's folder (its meson file only
  declares the dependency: `GALLIUM_WCONTAINERS` and the new `wcontainers-link-args` option, the
  static library cargo built and the native libraries rustc printed for it;
  `wcontainers_public.h` declares `wcontainers_screen_create`), and `wgl.c`'s three places (the
  screen by name, ours first after `GALLIUM_DRIVER`'s choice, present through
  `flush_frontbuffer`). Applied and built on Linux (x86-64 machine, driver not selected there).
- The driver's first slice, `crates/wcontainers-opengl` (as run 2 left it; run 3 moved the
  screen into Rust, above): the C bridge (`bridge/screen.c`, built
  by `cc` against the pinned headers and `u_screen.c`'s compile flags, as wcontainers-nir's
  bridge is) makes the `pipe_screen`: Gallium's default caps (`u_init_pipe_screen_caps`) with
  the interface's limits (texture sizes and levels, layers, colour targets, vertex streams and
  stride, point size) over them, the GPU's name. The device (`src/device.rs`) is the WebGPU back
  end on `wgpu`'s Direct3D 12 on Windows (Vulkan elsewhere), as the Direct3D fronts make theirs.
  Honest absence until the context exists: `is_format_supported` reports no format, and
  `context_create` refuses by name ("wcontainers: refused: an OpenGL context ..."), so WGL offers
  no pixel format and makes no context rather than one that draws nothing. Lint clean with and
  without Mesa; the static library exports `wcontainers_screen_create` and the device's entry
  points and leaves Gallium's helpers to Mesa's link (x86-64 machine, `nm`). The Windows Mesa
  script now builds it (`RUSTFLAGS=-C target-feature=+crt-static` for Mesa's static runtime, its
  own target folder `C:\wc\graphics-nir\opengl-target`), passes it and rustc's
  `native-static-libs` through `meson configure`, and links it into `libgallium_wgl.dll` with
  `gallium-drivers = ['wcontainers', 'llvmpipe']`.
- Next, in order (after run 3; the hang, the NIR options, the bridge-only build and the formats
  are done): (1) the context (`pipe_context`, a new `src/context.rs` on the bindings): the
  `stream_uploader` and `const_uploader` st/mesa needs at creation (`u_upload_create_default`,
  so buffer maps first), resources (`resource_create`, `resource_destroy` on the interface's
  buffers and textures, buffers a whole number of storage words), transfers (`buffer_map`,
  `texture_map` through the interface's write modes and read backs), surfaces and
  `set_framebuffer_state`, `clear` (`clear_texture`/`clear_rects`), `flush` and the screen's
  fences, `flush_frontbuffer` presenting through a `wgpu` surface over `WindowFromDC`'s window
  (`WebGpu::add_window`), and every state object stored as a block (`create_*_state`); an entry
  point not built yet ends the process with its name (a Crash in the results, never a silent
  no-op). Then shaders (`create_vs_state`/`create_fs_state` keep the NIR, `Nir::from_raw`; at the
  first draw route B with the variant key), `draw_vbo`, `util_blitter` blits, queries. Shader
  storage, images and atomic counters are reported absent until bound (caps 0). Then the GL
  screen caps that decide the version (`glsl_feature_level`, `essl_feature_level` and the rest
  st/mesa reads, from what the context does); (2) the lists through the queue in shards on our
  driver, the table, every failure with a reason; (3) bgfx's and Godot's OpenGL renderers on the
  rig.
- G9's report, the table of lists and shares on OUR driver: none yet. dEQP-GLES2, dEQP-GLES3,
  dEQP-GLES31, KHR-GLES and KHR-GL33 (the 92.9% floor together) and KHR-GL46 (98.6%): no case has
  run on our driver; its screen refuses contexts until the context is built.
- The harness is proven on llvmpipe (G9 run 2, the desktop through the GPU queue, `--desktop`,
  deqp-runner 8 at once, every 100th case of each list from the first, 2026-11-20 09:06 to 09:08,
  shared with other tracks' queued jobs). These are llvmpipe's numbers, not ours, and llvmpipe's
  Windows build here crashes (below), so they say nothing of the floors yet:
  - dEQP-GLES2: 172 cases, Pass 111, Crash 61: 111 of 172 run, 64.53%.
  - dEQP-GLES3: 448 cases, Pass 255, Crash 192, NotSupported 1: 255 of 447, 57.05%.
  - dEQP-GLES31: 379 cases, Pass 113, Crash 242, NotSupported 24: 113 of 355, 31.83%.
  - KHR-GLES2: 5 cases, Pass 5: 100%. (A probe of every 20th case: 24, Pass 22, Crash 2.)
  - KHR-GLES3: 65 cases, Pass 57, Crash 8: 87.69%.
  - KHR-GLES31: 42 cases, Pass 27, Crash 15: 64.29%.
  - KHR-GLES32: no results: glcts could not make an OpenGL ES 3.2 context through WGL on
    llvmpipe ("Failed to create WGL context", tcuWGL.cpp:541).
  - KHR-GL33: 99 cases, Pass 75, Crash 24: 75.76% (09:19, Mesa rebuilt with our driver beside).
  - KHR-GL46: 198 cases, Pass 116, Crash 28, NotSupported 54: 116 of 144, 80.56%.
  Every crash seen ends with exit code 0xc0000374 (heap corruption) within 0.1 to 2 s, in cases
  that compile shaders (constant expressions, indexing, conversions, draws) and in two
  renderbuffer cases. Not yet explained. Suspected: the LLVM linked into llvmpipe is LLVM's own
  Windows release, whose build settings (assertions, ABI-breaking checks, iterator debugging)
  may differ from what Mesa's build assumes; Mesa's CI builds its own LLVM. To find: a crash's
  stack (`cdb` or WER's dump of one crashing case run alone), then an LLVM built here if that is
  the cause. Our driver does not use LLVM, but llvmpipe is the reference the plan holds pictures
  to, and the share real OpenGL passes is to come from the RTX 3080's own OpenGL (not reachable
  this way: the CTS loads our `opengl32.dll` from its folder; a second glcts folder without it
  gives NVIDIA's).
- The RTX 3080's own OpenGL (NVIDIA's driver, `-Driver system`: glcts hard-linked into a folder
  without Mesa's DLLs), the same 1% shards, 09:13, 23 s in all: every case run passed. dEQP-GLES2
  172 of 172; dEQP-GLES3 446 of 446 (Pass 445, Warn 1, NotSupported 2); dEQP-GLES31 378 of 378
  (NotSupported 1); KHR-GLES2 none run (all 5 NotSupported); KHR-GLES3 65 of 65; KHR-GLES31 42
  of 42; KHR-GLES32 15 of 15; KHR-GL33 98 of 98 (NotSupported 1); KHR-GL46 175 of 175
  (NotSupported 23). So the harness is sound, llvmpipe's crashes are its own build's, and this
  is the reference for "none that real OpenGL on the same GPU passes is left without a reason".
- Our driver through the harness (`-Driver wcontainers`, Mesa rebuilt with it linked, 09:18):
  KHR-GLES2's 5 cases all Missing: each glcts process was killed at deqp-runner's 60 s limit
  with no output at all (its output is buffered and was lost when it was killed). Found and fixed
  in run 3 (above): not the screen's creation but its destruction, at `opengl32.dll`'s unload.

G8 run 15 (2026-11-20). Complete but for one item waiting on the owner's decision, named below;
handed to G8's independent check again. Run 15 fixed the second check's findings: route B's
unordered not-equal (NIR's `fneu`, which naga made ordered: a NaN compared as not unequal) is
now the not of equal, held to llvmpipe on NaNs; the fixed-function state values, the last
closable harness limit, are filled and match (their empty name was the standalone compiler's
stubbed state string); a formatless image read and written is bound as one component and held
by 273 Vulkan CTS programs; the robustness CTS programs are in the corpus; both routes are timed
without the cache key; the known failure is traced to llvmpipe's code generation. Both
candidates are measured on the SPIR-V corpus, the Vulkan CTS's programs, piglit's, bgfx's and dEQP's GLSL, Godot's Compatibility renderer and Godot's RenderingDevice
shaders; candidate B leads on every count and on time, and is the route (The choice, below;
final). Run 14 fixed the findings of G8's first check: the 15.4 rewrites are now made on route B's
output by the shaders crate's `rewrite` (`direct::rewrite`), each with corpus programs using it;
the GLSL bridge reports the atomic counter buffer size st/mesa does (48 refused programs now
compile) and route B takes GLSL's workgroup memory; the last two closable harness limits are
closed (a fragment input block, multisampled textures on llvmpipe). Dolphin is left out (below).

Waiting on others (G8's bar, as the orchestrator ruled on 2026-11-19 night):
- robustBufferAccess's three steps (15.4). Cause: `HelperValue::BufferRange` is a proposed
  interface change of 15.7 and one of the owner's open decisions of 15.9 (not on the shared line
  nor on any graphics branch at the start of run 14). Owner: the owner, then the interface track.
  Step 2 (storage as runtime-sized arrays of words, 32-bit accesses) is already what the bridge
  makes of OpenGL's buffers (Done); steps 1 and 3 need the helper value. Checked again in run 15:
  `HelperValue::BufferRange` is on no graphics branch. What does not need it is done: the
  Vulkan CTS's `dEQP-VK.robustness.buffer_access` programs are in the corpus since run 15
  (`corpus/cts-programs`; 162 programs: 52 need what our Vulkan 1.1 does not report, route B
  matches 62, 24 fail by name on 64-bit values and 24 on texel buffers; compared without the
  lowering, which they will be held with once steps 1 and 3 exist).

- The literal check's exit 0. Cause: `CUBE_FACES` (the layers of one cube) is defined in
  `wcontainers-nir/src/lib.rs` and, since the Direct3D 11 track's 7e4d8df6, crate-private in
  `wcontainers-direct3d-11/src/device.rs`; the check reports the second definition, and neither
  crate can import the other's. The one place for it is the interface (Requests). Owner: the
  interface track (or the owner, since this track may not change the interface). Still so after
  merging the Direct3D 11 branch's tip (a5834fb5) in G9 run 1.

Not a bar item, waiting on the shaders track: 20 of Godot's RenderingDevice programs with arrays
of textures (`gi.glsl` and kin) reach the shaders crate's `rewrite`, which validates without
`Capabilities::IMMEDIATES` and so refuses their push constants ("rewrite: invalid: ...
IMMEDIATES"); route B's own validation adds it. Asked under Requests to other tracks (the
`capabilities` request of run 4, now also for `rewrite`).

Done:
- Mesa pinned and building on the rented x86-64 machine with llvmpipe and lavapipe (and Zink, whose
  NIR to SPIR-V compiler is candidate A's first step), by one command (How to run). Pin:
  `crates/wcontainers-nir/mesa/pin.toml` (mesa-26.3.1, commit fec8830e9c94); meson options:
  `crates/wcontainers-nir/mesa/options.ini`; patches: `crates/wcontainers-nir/mesa/patches/` (applied
  in name order, again from the pinned commit whenever they change; they are part of the converter's
  version); script: `crates/wcontainers-nir/mesa/build`. One patch:
  `zink-matrix-arrays.patch` (Zink's `get_glsl_type` decorates a member that is an array of matrices
  too, with the type's explicit stride).
- `crates/wcontainers-nir`: the crate, its C bridge (`bridge/bridge.c`, `bridge.h`) and its link to
  Mesa's compiler libraries (`build.rs`). Built when `WCONTAINERS_MESA` names the Mesa build folder;
  without it the crate builds without the NIR parts (cfg `mesa`), so the workspace builds everywhere.
  `build.rs` takes everything about Mesa from Mesa's own build (Meson's `meson-info`, the compile
  commands, the introspected dependencies and targets, among them lavapipe's file).
- The NIR the routes take is shaped by one `nir_shader_compiler_options` in the bridge (Zink's list
  plus all 64-bit integer lowering and the instance index counting from the first instance); unused
  inputs, outputs and system values are removed. The drivers (G9, G10) must hand over the same.
- `Nir::to_common_form` (`wc_nir_to_common_form`), every route's step: sampled array layers rounded
  to the nearest even integer (Vulkan's rule; the common form's layer is an integer, and naga's
  SPIR-V reader truncates the float layer); compute dispatches counted from workgroup 0 with no
  global offset (`nir_lower_compute_system_values` with no bases); the binding rule of 6.4 (each combined image sampler
  split into a texture and a sampler at the same set and binding; every resource renumbered into its
  stage's set kind by kind in the program's (set, binding) order; the reflection as
  `ReflectedBinding`, program set in `space`, program binding in `register`; unsized arrays of
  resources and overfull ranges refused by name).
- The cache key (15.3): `Nir::key`, BLAKE3 of `nir_serialize`'s bytes and `CONVERTER_VERSION` (this
  crate's version; since run 12 a BLAKE3 of the crate's `src` and `bridge` and of the carried naga,
  made by `build.rs`, so a change of the route's output changes the key, as the shaders crate's
  `CONVERTER_VERSION` does; the Mesa commit and a hash of `mesa/build`'s record of commit and
  options). Taken from the NIR as the driver hands it over. Held by `tests/route.rs`.
- Candidate A (`zink-spirv-naga`): Zink's `ntv_shader_prepare` and `nir_to_spirv` (native Vulkan,
  SPIR-V 1.3), cleaned of Zink's `Index 0` decorations and its unneeded `DrawParameters`, read by
  naga's SPIR-V reader and validated with what the common form allows.
- Candidate B (`nir-naga`, `src/direct.rs`, `bridge/flat.c`, `flat.h`): the bridge takes the
  shader out of SSA (phi webs into registers; every other value used outside its block, or by an
  `if`, into a register of its own; derefs, and buffer descriptors (`vulkan_resource_index`,
  `load_vulkan_descriptor`), made again in each block) and flattens it into plain
  arrays (types, struct fields, variables, then the instructions in order with `if`, `else`,
  `loop` and their ends as markers). Mesa's numbering stays in C: every kind and operation crosses
  as the bridge's own enums (`WC_ALU_OPS`, `WC_INTRINSICS`, mirrored in order in Rust). Rust walks
  it into naga: stage inputs and outputs as entry point arguments and a result struct through
  locals (the position's y negated on the way out: the common form's clip space is WGSL's, as naga's
  SPIR-V reader makes it for A), buffers through `vulkan_resource_index` chains, registers as u32
  or bool locals, NIR's untyped values kept with the kind their instruction gives and bitcast at
  each use, constants made at each use in the kind it needs, a texture sampled with a comparison
  made a depth texture; storage images (load, store, size; multisampled loads), atomics on buffers
  and workgroup variables (each place an atomic acts on, found by walking its deref back, gets an
  atomic type, as naga requires), `fmod` (a - b floor(a / b)), `frem`, point coordinates, storage
  buffers always readable (the common form has no write-only buffers). What it does not take is
  refused by name ("not taken: ..."). Held by
  `tests/route.rs` (both candidates bind a sampling program alike; a compute program).
- Stage values in route B are lowered IO, one form for every source: `wc_nir_lower_io` (bridge.c)
  runs Mesa's `nir_lower_io_passes` on a Vulkan program (after splitting gl_PerVertex), which the
  OpenGL linker has already run on a GLSL one. `flat.c` gathers every slot `load_input`,
  `load_interpolated_input` and `store_output` reach into one variable a slot (the widest access;
  one kind, u32 where accesses disagree; clip distances folded into one array; a built-in keeps its
  own type), with a fragment input's interpolation from its barycentric (`load_input`: flat); Rust
  reads them as `LOAD_STAGE` and `STORE_STAGE` on that variable's local. Refused by name: a slot
  indexed at run time, other than 32 bits, a second blend source, interpolation at a sample or an
  offset, a slot interpolated two ways. The corpus gives the same results as with variables (run 5).
- alphaToOne's two steps (15.4): `Nir::alpha_to_one` (`wc_nir_alpha_to_one`) lowers IO and runs
  Mesa's `nir_lower_alpha_to_coverage` and `nir_lower_alpha_to_one`. Route B turns
  `demote_samples` into a local of the samples kept (all ones at the start, ANDed with the
  complement of each demoted set) that becomes the sample mask output, ANDed with the program's
  own where it writes one; the pass's 16-bit operations (u2u16, u2u32, mov, and, or, multiply,
  shifts) are carried in 32-bit words holding the zero-extended value (multiply and shift left
  masked to 16 bits, shift counts to their low 4 bits); every other 16-bit operation is refused by
  name. Held by `tests/route.rs` (a program with and one without its own sample mask: one sample
  mask output, alpha one, validated) and `tests/narrow.rs`: a SPIR-V program with native 16-bit
  integers (written word by word in the test) run by lavapipe natively and through route B, both
  held to Rust's 16-bit arithmetic, over every 16-bit value and every shift count 0 to 31 (2,097,152
  invocations, 14,680,064 results, seed 0x5eed for the second operand and the upper bits; 0 differ).
  Checked to see differences: with the multiply's and the shift count's masks taken out (temporary),
  6,151,775 results differed.
- GLSL (`Nir::from_glsl`, `wc_glsl_to_nir` in `bridge/glsl.cpp`, a C++ file built by a second
  `cc::Build` with the flags of Mesa's `standalone.cpp`): Mesa's GLSL compiler and linker on the
  standalone scaffolding (`libglsl_standalone`, no gallium screen), one context for every source:
  OpenGL 4.6 compatibility, GLSL 4.60, the ES 3.x compatibility extensions, and the limits the
  OpenGL driver is to report (named `WC_GL_*` in `glsl.cpp`; G9 reports the same). Then, per stage,
  st/mesa's steps after a link (`st_link_glsl_to_nir`, `st_glsl_to_nir_post_opts`,
  `st_finalize_nir`, MIT) for that driver's capabilities: dead IO variables removed;
  `gl_nir_lower_buffers` (which ends in `nir_lower_explicit_io`: every buffer access is a block
  index and a byte offset); fragment coordinates and point coordinates transformed for the common
  form's origin (upper left, centres at half: `nir_lower_wpos_ytransform` and
  `nir_lower_pntc_ytransform`, their state values added to the parameters); system values; state
  references; atomic counters into storage buffers (no hardware counters); 64-bit integers lowered;
  rectangle textures and gathers with offsets lowered (`nir_lower_tex`: no rectangles, no gather
  offsets); uniform places assigned in words (packed storage, `PackedDriverUniformStorage`) and the
  default block made uniform buffer 0 (`nir_lower_uniforms_to_ubo`, every block one index up);
  samplers as derefs; `gl_FragColor` written to every draw buffer (`nir_lower_fragcolor`, the
  driver's own step, as Zink's). A built-in uniform structure of the compatibility profile (the
  lighting state) is refused by name. Compile and link errors come back with Mesa's log
  (`ConvertError::Glsl`).
- OpenGL's buffers in route B (`wc_nir_buffers_as_words`, bridge.c): each uniform block becomes a
  variable of `vec4<u32>` words (the common form's uniform arrays step by 16 bytes; its size the
  block's, the default block's from the parameters), each storage block a runtime-sized array of
  `u32`, writable only where the program writes it; uniform block i binds at i, storage block j at
  the number of uniform blocks plus j (the program's set and binding in the reflection);
  `load_ubo`, `load_ssbo`, `store_ssbo` (by its write mask), the storage atomics and
  `get_ssbo_size` become 32-bit derefs of those words at offset / 4, which route B already takes.
  A block index not constant after folding, or an access not 32 bits wide, is left as it is and
  refused by name. This is also 15.4's robustBufferAccess step 2 (storage as runtime-sized arrays
  of words, 32-bit accesses). Held by `tests/route.rs` (a vertex and fragment program with default
  uniforms, a std140 block, std430 blocks, a sampler, an atomic: both stages convert and validate,
  bindings counted by kind; a program that does not compile is refused with its log) and
  `tests/glsl_buffers.rs`: a GLSL compute program reading a std140 block (vec3, float, float array,
  mat3, ivec2) and writing a std430 block (atomic, runtime array, `.length()`) runs on lavapipe
  through route B and naga's SPIR-V writer and matches what GLSL's layout rules and arithmetic give,
  computed in the test (exact values). Checked to see differences: with the word's component within
  its vector forced to 0 (temporary), 10 of 17 values differed.
- Each GLSL stage's names (`GlslStage::names`, `wc_glsl_names` in `glsl.cpp`): every default-block
  uniform with its first word in uniform buffer 0 and its words (Mesa makes one parameter a
  column or element; the bridge joins them into one name), the state values
  (`FragCoordYTransform`, `PointCoordYTransform`, any other by Mesa's state string), each uniform
  and storage block's and each sampler's and image's program binding, and each stage input and
  output with its location in the common form, GLSL type and interpolation. Held by
  `tests/route.rs`. Stage values between two stages are what the linker leaves: varyings are packed
  (`packed:a,b`) and `nir_opt_varyings` reworks them across the stages; a vertex program linked alone
  loses its outputs. Attributes and fragment outputs keep their names.
- Route B takes `b2b1` (a 32-bit boolean word as a boolean: not zero), which OpenGL's default-block
  booleans need; casts to a boolean are made at naga's boolean width.
- The GLSL reference (`tests/corpus/llvmpipe.rs`, `glsl.rs`): llvmpipe through the pinned build's
  `libEGL` (`mesa::EGL`, from Meson's target `EGL`), on EGL's software device
  (`EGL_MESA_device_software` through `EGL_EXT_platform_device`), an OpenGL 4.5 compatibility
  context with no surface. The program is compiled and linked by llvmpipe; OpenGL's reflection
  (`glGetProgramResourceiv`, called directly: glow's asks for the count with no room and gets
  nothing) gives every uniform, block and storage-block member with its type, offset and strides.
  Default-block uniforms are filled from the seed by type, set with `glUniform*` and written into
  uniform buffer 0 at the bridge's words (booleans as the driver's true, all ones); each block is
  filled member by member by type (runtime-sized arrays 64 elements) and the same bytes are bound
  on both sides, the route's at the binding the bridge names for the block's name. Compute: the
  program dispatched twice on llvmpipe (they must agree), the route's module once on lavapipe;
  storage blocks compared word by word by the member's type (padding not compared). Held by
  `tests/glsl_reference.rs` (a compute program with float, ivec3, mat2, uint array and bool
  uniforms, a std140 block of an array of structures and a mat3, a std430 block with an atomic and
  a runtime array of structures: matched). Checked to see differences: with route B's `fmul` made
  an add (temporary, on the x86 machine's copy) 32 values differed; with `iadd` made a subtract, 49.
- piglit's shader tests in the corpus (`corpus/sources`, `glsl::shader_test_sources`): each
  `.shader_test` file's `[vertex shader]`, `[fragment shader]` and `[compute shader]` sections
  through `Nir::from_glsl` and each stage through the route; `[vertex shader passthrough]` is no
  stage of the file's; geometry, tessellation and assembly programs, two sources of one stage and a
  compute stage beside others are refused by name. A source without `#version` gets the file's
  `GLSL >= x` requirement's, as piglit's shader runner gives it (`shader_runner.c`).
- GLSL vertex and fragment programs held to llvmpipe (`glsl::drawn_program`, `compare_draw`,
  `Llvmpipe::draw`): drawn as points, one a pixel of a row, one input set each. A vertex program is
  wrapped in its own source (`#define main wc_program_main` after `#version`; a new `main` calls it,
  keeps `gl_Position` in `wc_position` and puts the point at the attribute `wc_point`); with no
  fragment program it is followed by a catcher (written from llvmpipe's reflection of the wrapped
  program linked separable) that writes every output to a storage block at the point's pixel. A
  fragment program alone is fed by a vertex program written from its inputs (llvmpipe's reflection
  of it linked separable; `gl_Color`, `gl_TexCoord` and the like fed through OpenGL's own outputs).
  Desktop feeders and catchers are `#version 450 compatibility` (no interpolation to match between
  stages from 4.40); an ES feeder is the program's own ES version. The drawn program goes through
  `Nir::from_glsl` and the same route; llvmpipe draws it (window depth 0 to 1 by `glClipControl`,
  vertex colour clamping off: both are draw-time variants of the driver's that the route does not
  take), twice (they must agree), and lavapipe runs the route's two modules. Attributes at
  llvmpipe's locations and at the bridge's; an attribute the route does not read is fed to llvmpipe
  alone. State values filled as st/mesa does for a framebuffer object. Colour targets (each
  output's components), depth and storage blocks compared. Not set up yet, by name:
  `gl_SecondaryColor` (its array has three components), structure and block stage values, an ES
  vertex program alone.
- Faults in the GLSL bridge the reference found (run 7), each fixed: blocks without a layout were
  std140 where st/mesa makes them std430 for a driver that loads its constants from a buffer
  (`UseSTD430AsDefaultPacking`; the one compute program that differed); a source without `#version`
  was taken at no version at all and accepted 1.30 and later syntax (`DefaultGLSLVersion` is now
  110, st/mesa's); `gl_FragColor` was not written to the draw buffers (`nir_lower_fragcolor` works
  on variables, and stage values are lowered by then: the bridge now does it on lowered IO);
  vertex inputs kept Mesa's slots, so `gl_Vertex` and the other attributes of OpenGL's own had no
  location: they are now numbered as st/mesa gives the driver its vertex elements (each at its rank
  among the attributes read, `nir_recompute_io_bases`), and the bridge names them so. OpenGL's own
  varyings (`gl_FrontColor`/`gl_Color`, back colours, `gl_TexCoord[]`, fog) take locations after
  the 32 generic ones, by their Mesa slot (`MAX_VARYING` + slot). A built-in still refused names
  its Mesa slot.
- Route B fixes from run 8: a depth comparison gives one value, and NIR's legacy shadow lookups
  (GLSL 1.10's `shadow2D*`, four components) have it in every component, so route B splats it, as
  llvmpipe's sampler does before the view's swizzle (OpenGL's depth texture mode is that swizzle:
  G9's view, not the program's); an infinite or NaN constant is made from its bits (`u32` literal
  bitcast to `f32`), as naga's literals must be finite; projective lookups are lowered in the
  bridge (`nir_lower_tex`'s `lower_txp`, all dimensions and arrays); a rectangle texture is 2D
  after `lower_rect` (its coordinates normalized). Refused by name where the common form has
  nothing: a comparison with a bias or gradients (WGSL compares at level 0 or the implicit level
  only), sampling an integer texture (WGSL only loads and gathers those; a rewrite to a load with
  the sampler's addressing would be a slow path G9 can add), an array of arrays of textures,
  samplers or images, a structure holding them, clip distances read by a fragment program.
- The GLSL comparison fills textures (run 8): each route texture resource finds its sampler by the
  bridge's `GlslNameKind::Sampler` name at its binding; the same seeded texels (as
  `compare::resources` makes them) become an OpenGL texture of the view's target at its own unit
  (`SampledTexture`, `Llvmpipe::sampled_texture`: nearest, clamped to the edge, one level; a depth
  texture compared less or equal with its swizzle `RED` in all four, so llvmpipe's raw comparison
  is compared); a sampler both stages read gets one set of texels. Storage images the same way
  (`GlslNameKind::Image`), bound with `glBindImageTexture` at the image unit of the same number
  and read back with `glGetTexImage` after the work, after the storage blocks (`GlStorage`); the
  route's storage image output compared word by word by its components' kind. OpenGL's own attributes
  (`gl_MultiTexCoord0..7`, `gl_Color`, `gl_Normal`, `gl_FogCoord`) are fed through OpenGL's own
  arrays (`AttributeSlot`; `glTexCoordPointer` and its kin). Held by `tests/glsl_reference.rs`
  (a compute program reading a 2D, a 3D unsigned, a cube, a 2D shadow and a 2D array signed
  texture through `textureLod`, `texelFetch`, `texelFetchOffset` and a comparison, and loading and storing an `r32ui`
  and an `rgba32f` image: matched). Checked to see differences (temporary changes on the x86
  machine's copy): with OpenGL's texels shifted by one texel, 236 values differed; with the
  images not read back, 80.
- bgfx's desktop GLSL shaders in the corpus (run 8; `tests/corpus/bgfx.rs`): its compiled shaders
  (`examples/runtime/shaders/glsl/*.bin`, shaderc's format 12) read as bgfx's OpenGL renderer
  hands them to the driver (`#version 430` before a source without one); its `bgfx_ndc` uniform is
  left a uniform (bgfx makes it a constant from its capabilities; seeded here like any other).
  A file of another format version is refused by name.
- Route B fixes from run 9: `ubitfield_extract`, `ibitfield_extract` and `bitfield_insert` (one
  `extractBits` or `insertBits` a component, as the common form's offset and count are scalars),
  `isign`, `frexp_exp` and `frexp_sig` (naga's `frexp` and its result's members), the sample mask
  input, and `ixor` of booleans (made `!=`; naga has no exclusive or of booleans). Held by
  `tests/glsl_reference.rs` (a compute program of bitfields, signs and exponents: matched; with the
  extract's offset and count swapped and the sign made an absolute value, temporarily on the x86
  machine's copy, 198 values differed).
- A vertex attribute read in part (run 9): st/mesa gives the driver a vertex element a slot read,
  so a matrix of which some columns are read has elements only for those. The bridge now names
  each run of read slots of an attribute (`GlslName::element`, the run's first slot in the value;
  `at`, its first element; `count`, its slots), and the comparison feeds each column of an
  attribute at the route's place for it, or to llvmpipe alone. Before, the unread columns moved the
  next attributes, among them the point's position. Held by `tests/route.rs` (a `mat4` read in
  columns 1 and 3: two names).
- dEQP's GLES shader library in the corpus (run 9; `tests/corpus/deqp.rs`): every case of VK-GL-CTS
  `data/gles2/shaders`, `gles3/shaders` and `gles31/shaders` (`*.test`, Apache-2.0, pinned in
  `corpus/sources`) read from dEQP's format (`gluShaderLibrary.cpp`'s grammar) and specialized as
  `glsShaderLibraryCase.cpp` does (its `${DECLARATIONS}`, `${SETUP}`, `${OUTPUT}` and the program
  cases' `${VERTEX_*}` and `${FRAGMENT_*}` values, `:single-line`, the extensions a case requires
  after the version line, a source without a version taken at the case's): a `both` case is two
  items, `<case>_vertex` (with a fragment program written here that writes what it passes on to the
  colour targets) and `<case>_fragment` (the corpus's feeder gives its inputs); a case of its own
  vertex and fragment sources is one. Where dEQP compares the outputs with its expected values in
  the shader, here every output is written to a colour target (floats four a target, from
  `dEQP_FragColor`; integers and booleans as `uint` in `uvec4` targets after them; in GLSL ES 1.00
  all as floats in `gl_FragColor`), so llvmpipe and the route are compared on the values. Refused by
  name: cases dEQP expects to fail (compile, link or validation), pipelines of separable programs,
  geometry and tessellation, outputs past the colour targets. An item is `<file>#<case>`. Held by
  `tests/glsl_reference.rs` (a library file of a `both` case, a failing case and a program case:
  each item's sources, and each compiles).
- ES programs at full precision (run 9, `glsl::at_full_precision`, every GLSL program of the
  corpus): `#define lowp highp`, `#define mediump highp` and `highp` defaults after the version and
  extension lines. llvmpipe computes `mediump` in 16 bits (its `fp16` and `int16` capabilities make
  Mesa's GLSL compiler lower precision), while the bridge lowers none, as our driver will compute;
  GLSL ES allows both. Without it 883 of dEQP's programs differed by half-precision rounding or
  16-bit integers. `#version 100` is now taken as ES (GLSL ES 1.00 has no profile word), so its
  feeders are ES too.
- The alpha_to_one programs in the corpus (run 9): `corpus/cts-programs` builds VK-GL-CTS's
  `vk-build-programs` at the commit `corpus/sources` pins and writes the programs of
  `dEQP-VK.pipeline.monolithic.multisample.alpha_to_one` (its registry: one file a distinct program;
  the group's 28 cases build 2, a vertex and a fragment program) into
  `nir-corpus/vk-gl-cts-programs/alpha_to_one`. The corpus run puts a fragment program of that
  folder through `Nir::alpha_to_one` before the routes and records it "held by G10's CTS cases"
  instead of comparing it (below). Run 9 (`results/run9/nir-results-r9-a2o.jsonl`): route B
  converts and validates the fragment program after the two steps; the vertex program matches
  lavapipe on both routes; route A fails the fragment program (Zink's compiler and lowered IO).
- Godot's Compatibility renderer in the corpus (run 10; `tests/corpus/godot.rs`, Godot pinned in
  `corpus/sources`, only `drivers/gles3/shaders` and its licence fetched): each `.glsl` file with a
  stage section read as Godot's build reads it (`gles3_builders.py`: `#[modes]` lines `name =
  defines` with `\n` escapes, `#[specializations]` `NAME = true|false`, `#[vertex]`,
  `#[fragment]`, `#include` resolved beside the including file once a stage), and each variant
  made as `ShaderGLES3::_build_variant_code` makes it for desktop OpenGL (`#version 330`,
  `USE_GLES_OVER_GL`, `USE_HALF2FLOAT`, the set specializations, the general defines, the mode's,
  external textures as `sampler2D`, multiview emulated, highp defaults; the material's
  `#GLOBALS`, `#MATERIAL_UNIFORMS` and `#CODE` lines empty, as for Godot's default version). The
  general defines are those `ShaderGLES3::initialize` gets at Godot's default project settings,
  in `corpus/godot-defines` with where each value comes from. Items: each mode at the default
  specializations and each mode with one specialization flipped (`<file>#<MODE>` and
  `<file>#<MODE>:<SPECIALIZATION>`), 241 in all. Held by `tests/glsl_reference.rs` (a file of two
  modes, two specializations, an include and material lines: its six items, two programs' text,
  both compile).
- A vertex program that writes no position (run 10; `clip_unpositioned` in `bridge.c`, part of
  `to_common_form`, so both routes): it gets the position (0, 0, 0, -1), outside every clip
  volume, as the shaders crate's `stage::clip_unpositioned` gives a Direct3D one (WGSL's vertex
  stage must write a position; OpenGL and Vulkan leave it undefined). On variables or lowered IO,
  whichever the NIR has. The comparison captures the outputs the program writes, so the added
  position is not compared. Held by `tests/route.rs` (a GLSL program writing only a varying: its
  module has a position and validates).
- Godot's RenderingDevice shaders in the corpus (run 11; `corpus/godot-forward-plus`,
  `tests/corpus/godot_rd.rs`): `servers/rendering/renderer_rd/shaders` and the AMD FSR and FSR2
  folders it includes are fetched with Godot's other paths. Each file is read as Godot's build
  reads it (`include_file_in_rd_header`: GLSL comments cut, `#[vertex]`, `#[fragment]`,
  `#[compute]`, includes beside the file or, under `thirdparty/`, from Godot's root, once a stage),
  and each variant written as `ShaderRD::_build_variant_code` makes it for Vulkan (the general
  defines, the variant's, `RENDER_DRIVER_VULKAN`, external textures as `sampler2D`, no material;
  compute alone where there is a compute stage), then compiled by glslang as Godot's
  `compile_glslang_shader` does (Vulkan rules, client Vulkan 1.1, SPIR-V 1.3) into
  `nir-corpus/godot-forward-plus`, which the corpus run takes as SPIR-V. The variants are in
  `corpus/godot-rd-variants` (387 variants of 81 files; each list copied from its `initialize`
  call, those built in loops written out as the loops make them; every variant of the list,
  those Godot enables only on some devices or settings included), the general defines in
  `corpus/godot-rd-defines` at Godot's default desktop settings, each with where it comes from.
  The tables were drafted by a scratch script reading the C++ (`scripts/rd_variants.py`,
  `rd_table.py` in scratch) and the loops written by hand; glslang compiles all 572 stages (0
  refused). `voxel_gi_sdf.glsl` and `giprobe_write.glsl` are not taken: no `initialize` call at
  the pinned commit builds them. Held by `tests/glsl_reference.rs` (a two-stage file with an
  include, a comment and material lines: both stages' text exactly as `ShaderRD` makes it).
- Route B takes every storage image format naga has (run 11; the bridge's `wc_storage_format`
  mirrors naga's `StorageFormat`, 64-bit apart, which route B refuses anyway), and NIR's `imod`
  (the remainder, plus the divisor where the signs differ and the remainder is not zero). The
  comparison makes those formats on both sides (lavapipe and llvmpipe); 32-bit float texels are
  compared as floats, every narrower format by its bits.
- A texel load's offset (run 12; `to_common_form`, both routes): Mesa's `nir_lower_tex` with
  `lower_txf_offset` adds it to the coordinate, as the common form's load takes none. Route B
  refuses a load still carrying an offset by name ("a texel load with an offset"); before, it
  dropped the offset silently, which made bgfx's two `cs_assao_prepare_depths_and_normals`
  programs differ. `nir_lower_tex` also gives every size query a level 0, which the bridge takes
  off a multisampled texture's again (the common form's size query of one takes no level). Held by
  `tests/glsl_reference.rs` (the sampling program now reads through `texelFetchOffset` too:
  matched; with the lowering taken out, temporarily on the x86 machine's copy, the route refuses
  it by name).
- Subroutine uniforms (run 12): the bridge names each as `GlslNameKind::SubroutineUniform` by its
  API name (Mesa's `__subu_<stage>_` prefix taken off), its words one an element, each the index of
  the subroutine chosen (what st/mesa writes from `glUniformSubroutinesuiv`). The comparison reads
  each stage's subroutine uniforms from llvmpipe (`GL_*_SUBROUTINE_UNIFORM`: location, elements,
  compatible subroutines), chooses one compatible subroutine an element from the seed, sets them
  with `glUniformSubroutinesuiv` after `glUseProgram`, and writes the same indices at the route's
  words. Held by `tests/route.rs` (an array of two: one name, two words) and the corpus (the 9
  piglit programs match; with every index's low bit flipped on the route's side, temporarily, 7
  of 9 differed).
- Cubes in route B (run 13): a cube's size has two components (the faces' width and height; a
  cube array's layers after them), where route B took three and failed naga's validation on a cube
  array's `textureSize` (dEQP `android_extension_pack.test#extension_directive.ext_texture_cube_map_array`).
  A storage cube or cube array is a 2D array of its faces in the common form, which has no storage
  cubes (`storage_dimension`; Vulkan and OpenGL address a storage cube's faces as layers): its
  coordinate's third component is the layer, its size the faces' size and, for a cube array,
  the layers divided by `CUBE_FACES` (now the crate's one definition, `wcontainers_nir::CUBE_FACES`).
  The driver (G9, G10) binds a 2D array view of the cube's faces, as the interface already does
  for a cube read by integer loads. The GLSL comparison makes the storage image a cube on
  llvmpipe's side (by OpenGL's reflected type) and the route's 2D array of the same faces. Both
  dEQP programs match (`implementation_limits.max_fragment_image_uniforms` was the storage cube).
- Multisampled textures in the comparison (run 13): lavapipe gets each by a draw a layer
  (`Lavapipe::fill_samples`: a covering triangle, a fragment program written in WGSL and run a
  sample at a time that reads its own sample's texel from a storage buffer), every sample of
  every texel seeded on its own, 4 samples. All 6 programs that read one match on route B (Godot's
  `resolve` 3 and `resolve_raster`, Sascha Willems' `deferredmultisampling/deferred.frag`, and its
  vertex program). Checked to see differences: with every sample read as sample 0 (temporary, on
  the x86 machine's copy) `deferred.frag` differed; with samples 0 and 1, 2 and 3 swapped none did
  (each program averages over its samples); Godot's read a seeded sample count, at most 1 on the
  seed (expected, not confirmed). Since run 14 llvmpipe's side too (below); multisampled depth
  textures are not filled (no corpus program reads one).
- A reference that does not end on the seeded inputs (run 13; `corpus/run.rs`): a child writes
  `REFERENCE_ENDED` to its errors once the reference has run, so a child stopped at the time limit
  is told apart. Stopped before it: the reference did not end (a loop whose bound or step comes from
  the seeded inputs); the route is run again on the next seed, up to `REFERENCE_SEEDS` (4) seeds,
  and the seed used is recorded with the route's result (`inputs_seed`). Stopped after it: the
  route's module did not end where the reference did, a difference. The six `irradiancecube.frag`
  and Godot's `canvas_sdf.glsl#MODE_LOAD_SHRINK` now match on route B, on a later seed.
- The 15.4 rewrites on route B's output (run 14; `direct::route`, `direct::rewrite`): route B
  keeps an array of textures, samplers or storage images as naga's `BindingArray`, writes a
  gather's offset known only at run time as the gather's offset, and declares a storage image
  without a format as `r32float`, `r32sint` or `r32uint` by its texel's kind, listing its slots
  (`Rewrites`). `direct::rewrite(routed, bindings, &variant, &route_key)` calls
  `wcontainers_shaders::rewrite` (arrays split into a binding an element, gathers moved to a
  shifted coordinate, each named storage slot given the bound view's format) and gives the module
  with `cache::rewritten_key` and the bindings. A formatless slot the variant does not name is
  refused by name, and so is a variant asking for more than 15.4's rewrites (those read the stage
  values, which the driver's reflection holds: G9, G10). `direct::module` keeps refusing the
  three by name for callers that do not rewrite. The corpus run rewrites where a module needs it,
  each formatless slot bound as the four 32-bit components of its kind, or as one where the
  program reads and writes it (run 15: the common form reads and writes only one-component
  storage, so a four-component view was refused as `ReadWriteStorageFormat`); the comparison joins a
  split array's elements again on the reference's side (`Resource::first_element`), and on
  OpenGL's side feeds element k through the uniform `name[k]`. Corpus programs using each that
  match (run 14; one formatless program, piglit's `imageStore-array`, differs on seed 0x5eed by
  an index past its array, and matches on seed 271828): arrays of textures and samplers 19 piglit (`sampler_array_indexing` and
  kin), 4 Godot RD (`volumetric_fog_process`), 1 Sascha Willems (`texturemipmapgen`); formatless
  storage 2 Godot RD (`fsr2_accumulate_pass`, `fsr2_rcas_pass`), 3 Slang compute samples, 6
  piglit; a gather at a run-time offset, piglit's `fs-textureGatherOffset-uniform-array-offset`;
  since run 15 also 273 Vulkan CTS programs of `dEQP-VK.image.load_store.without_format`, which
  read and write formatless images. Held by `tests/glsl_reference.rs` (a program of an array read at a run-time index, a gather at a
  run-time offset and a formatless storage image: matched; with element k fed through
  `name[k+1]` on OpenGL's side, temporarily, 64 values differed); a formatless image read and
  written is held only by the CTS programs (above), as GLSL needs
  `GL_EXT_shader_image_load_formatted` for it, which our OpenGL does not report. One known failure,
  ignored with its reason, traced in run 15 to llvmpipe: a compute program reading one array at
  a run-time index and at a constant one. llvmpipe's constant read gives values no texel holds,
  the same whichever element it names (0, 1 or 2), and NaN in every fourth word once the two
  reads are swapped in the source; the textures, units and uniforms are the same in every
  variant, two run-time reads match, and llvmpipe's own final NIR indexes both reads correctly
  (`texture 0` plus the offset; `texture 2`). So the fault is in llvmpipe's code generation
  (gallivm takes a run-time index outside the fragment stage invocation by invocation,
  `lp_bld_nir_soa.c`), not in the route or the set-up; the checker's Vulkan copy of the program
  matches lavapipe on route B on three seeds. llvmpipe also aborts in LLVM ("Cannot select ... sint_to_fp") on a gather at a run-time
  offset from an array element; the test gathers from a single sampler.
- Unordered float comparisons (run 15; a fault of route B found by G8's second check): NIR's
  `fneu` (true where either side is NaN: GLSL's `isnan`, `!=`, `notEqual`; SPIR-V's `OpIsNan` and
  `OpFUnordNotEqual`) was naga's `NotEqual`, which naga's SPIR-V writer makes `OpFOrdNotEqual`,
  false on NaN. Route B now writes `fneu` as the not of `Equal`, and `bany_fnequal` as the not of
  all equal. Held by `tests/glsl_reference.rs` (NaN made from every seeded value by its exponent's
  bits, in every other invocation: `isnan`, `notEqual`, `!=`, `any(notEqual)`, and the ordered
  `==`, `<`, `>=`, `all(equal)` beside them: matched; with the old mapping, temporarily, it fails).
- Matrix state values (run 15): the bridge names each state value itself, since the standalone
  compiler's `_mesa_program_state_string` is a stub giving no name (`standalone_scaffolding.cpp`;
  this was the empty name of runs 7 to 14): a matrix as `matrix.<matrix>.<form>.<index>.<first
  row>.<last row>`, anything else as `state.` and Mesa's tokens. The GLSL comparison sets a seeded
  model-view (a scale, a turn and a move) and projection (a perspective) on llvmpipe with
  `glMatrixMode` and `glLoadMatrixf`, from a stream of the seed of its own (the program's inputs
  stay as they were), and fills each matrix state value as st/mesa's `copy_matrix` does: rows of
  the matrix or its inverse, or of their transposes (`gl_NormalMatrix` is the inverse's rows,
  `gl_ModelViewProjectionMatrix` the product's columns), inverses made in 64 bits; uniforms
  beginning `gl_` are OpenGL's state and are not set by the comparison. The texture and program
  matrices, and every other state value, are refused by name ("not filled here"). piglit's
  `glsl-fs-normalmatrix` and `glsl-vs-ff-frag` match on seeds 0x5eed, 1, 2 and 3; with the
  inverse's form and the transpose's swapped (temporary) both differ on seed 1.
- Atomic counters (run 14): the bridge reports `MaxAtomicBufferSize` (4 bytes a counter, st/mesa's
  4,096 counters without hardware counters), `MaxCombinedAtomicBuffers`,
  `MaxCombinedAtomicCounters` and `ARB_shader_atomic_counter_ops` (core in OpenGL 4.6); before,
  the size was 0 and Mesa refused every `atomic_uint` (48 corpus programs). Each atomic counter
  buffer is named (`GlslNameKind::AtomicCounterBuffer`: its storage block's program binding and
  its OpenGL binding point), and a storage index no access reaches (an atomic counter buffer's
  gap) gets no variable. The comparison binds llvmpipe's atomic counter buffers
  (`GL_ATOMIC_COUNTER_BUFFER` reflection), seeded, beside the program's storage blocks.
- Workgroup memory in GLSL (run 14): Mesa's linker makes it byte offsets (`load_shared`,
  `store_shared`, the shared atomics); the bridge makes it one workgroup array of words and each
  access 32-bit derefs of it, as for OpenGL's buffers. Held by `tests/glsl_reference.rs` (a
  program of atomic counters in two buffers, `atomicCounterAdd` and `Max`, workgroup arrays and an
  atomic on workgroup memory: matched; with the workgroup atomic made a maximum, temporarily, 16
  values differed; with the two counter buffers bound swapped on llvmpipe, 4).
- An implicit level outside the fragment stage is level 0 in route B (run 14; WGSL allows implicit
  levels only there): piglit's `border-color` and the vertex programs of `sampler_array_indexing`
  match.
- A fragment input that is a block (run 14; `interface.rs`): its members are fed at the locations
  after the block's, one after another (`meshshader/meshshader.frag` matches on seeds 1 to 4).
- Multisampled textures on llvmpipe (run 14; `Llvmpipe::fill_samples`): a draw a layer into the
  texture with a fragment program run a sample at a time reading its texel from a storage
  buffer, in the order lavapipe's fill takes (dEQP's
  `oes_texture_storage_multisample_2d_array` matches on seeds 1 to 4).
- Whether lavapipe runs `demote_samples` (checked first, as G8 says): it does not. In the pinned
  Mesa only broadcom, freedreno, asahi, KosmicKrisp and imagination take it; llvmpipe, gallivm and
  lavapipe have no code for it (searched in the source). So the fragment programs of
  `dEQP-VK.pipeline.monolithic.multisample.alpha_to_one.*` after the two passes are held by G10's
  CTS cases, not by lavapipe; they are to be listed so when they are in the corpus.
- The corpus run: every `.spv` under `WC_NIR_CORPUS`, each
  shader and route in a child process of its own; a crash or hang is named and attributed to the step
  it happened in (the route, or the comparison). One JSON line a shader. Knobs: `WC_NIR_SHADERS` (a
  seeded sample), `WC_NIR_SEED` (the sample and every shader's inputs; default 0x5eed),
  `WC_NIR_INPUT_SETS` (vertices or fragments a run, each with inputs of its own; default 64),
  `WC_NIR_JOBS`, `WC_NIR_RESULTS`, `WC_NIR_DUMP`.
- Counted apart before any route (`tests/corpus/needs.rs`): a program declaring a SPIR-V capability
  beyond Vulkan 1.1's core, or one of 1.1's whose feature our Vulkan driver reports absent (15.4's
  list of 26).
- The comparison (`tests/corpus/compare.rs`, `lavapipe.rs`, `interface.rs`): the program as given
  run by lavapipe (the reference), and the route's module written back to SPIR-V by naga's writer
  and run by lavapipe the same way, on the same seeded inputs; the reference runs twice and must
  agree with itself. Lavapipe is loaded from the pinned build by its own `vk_icdGetInstanceProcAddr`
  (no loader), every Vulkan 1.0 to 1.3 feature it has enabled, with transform feedback and
  robustness2 (accesses outside a binding read zero and write nothing, in both runs), one thread
  (`LP_NUM_THREADS=0`, so racing programs repeat). Inputs: uniform, storage and immediate data filled
  by type (floats in [-2, 2), integers in [-4, 4) or [0, 4): programs use them as indices and
  counts), runtime arrays of 64 elements, 4-texel textures of one level (2 layers for arrays, 6
  faces for cubes) with nearest clamped samplers, vertex attributes per input set. Compute: 2
  workgroups, storage compared. Vertex: transform feedback of the position and every located output
  (decorations added to both modules' words), rasteriser off. Fragment: a vertex program made with
  naga feeds one point a pixel of a row, one input set each; 32-bit colour targets cleared to a
  marker, depth where the program writes it, storage. Integers compared exactly; floats equal when
  within 1024 units in the last place or 2^-20 apart (both runs on one driver and processor: only
  the order and fusing of operations differ).
- The comparison is checked to see differences: with every float constant of the route's module
  raised by 1 (a temporary change, not kept), 37 of the 43 compared shaders of a 60-shader sample
  differed (the 6 others use no float constant on what they output).

The corpus (Sascha Willems' samples, GLSL, HLSL and Slang compiled to SPIR-V): 1005 shaders. 138
have a stage the common form lacks (ray tracing 81, tessellation 30, geometry 10, mesh and task 6); 3
crash Mesa's `spirv_to_nir` before any route (HLSL's variable rate shading); 25 need what our Vulkan
1.1 does not report (buffer device addresses 8, sparse residency 5, descriptor indexing 3, shading
rate 3, minimum LOD 3, barycentrics 2, ray query 2). 839 reach both routes.

Run 4's results, held to lavapipe (x86-64 machine, 192 children at once, load 9.2 at the start and
25.2 at the end, shared with other tracks; 293 s with the Mesa rebuild; Zink patched). Run 5, with
route B on lowered IO, gave the same counts and the same reasons (x86-64 machine, load 0.07 at the
start and 5.96 at the end, 256 s; `results/run5/nir-results-r5a.jsonl`):
- Candidate A: 728 match, 0 differ, 8 not compared, 103 fail by name.
- Candidate B: 773 match, 0 differ, 8 not compared, 58 fail by name.
- Shader by shader: 727 match on both routes, 1 on A only, 46 on B only.
- The 8 not compared, on both: 6 `irradiancecube.frag` (a loop whose step comes from the uniforms
  never ends on a negative step: the reference hangs), 1 multisampled texture (the harness does not
  fill those), 1 fragment program of the mesh shading sample with a block input.
- A's failures: 29 the draw's bases (`DrawParameters`); 40 crashes in Zink's compiler (31 in
  `emit_deref_array` on a variable mode it takes only after its own lowering of buffers and push
  constants to words, which this route does not run; 2 arrays of samplers; 7 `.length()` of runtime
  arrays); 7 naga reader `As` on HLSL vertex inputs and 1 on HLSL's row-major array of matrices in
  `shadowmappingcascade/scene.frag` (it differed before the patch; now it fails by name); 6 integer
  stage outputs without `flat`; 6 input attachments; 3 multiview; 3 vertex programs without a
  position; 3 formatless storage; 2 naga type resolution in order-independent transparency; 1 each:
  coherent image, arrays of combined image samplers indexed at run time, a uniform block's alignment.
- B's failures: 23 `load_first_vertex` and 6 `load_base_instance` (the draw's bases, as A); 11
  subpass inputs; 5 binding arrays the common form does not allow (3 textures and samplers, 2
  buffers: the descriptor heap sample); 3 `load_view_index` (multiview); 3 `image_deref_atomic`
  (order-independent transparency; A fails them too); 3 vertex programs without a position (as A);
  3 storage images without a format (15.4 rewrite); 1 uniform block alignment (HLSL's
  `texturearray/instancing.vert`, as A). The full list: `scripts/reasons.py` (in scratch) on
  `results/run4/nir-results-r4b.jsonl`.
- Zink also writes `gl_ClipDistance` and `gl_CullDistance` outputs a glslang program never writes, so
  A's modules ask a back end for clip distances they do not use; B drops them.

Run 7, with piglit's shader tests (2541 files) beside the SPIR-V corpus (`results/run7/nir-results-r7g.jsonl`;
x86-64 machine, 192 children, load 4.9 at the start and 13.2 at the end, 245 s):
- SPIR-V corpus: unchanged, A 728 match, B 773 match, 0 differ, 8 not compared each.
- piglit: 931 files reach no route (geometry 225, tessellation 147, assembly programs 91, several
  sources of a stage 122, and the rest refused by Mesa's compiler with its log: extensions our
  OpenGL does not report, and programs piglit itself expects to fail). Of the other 1610:
  route B 1152 match, 17 differ, 98 not compared, 343 fail by name; route A 3 match, 11 not
  compared, 1596 fail (Zink's compiler takes only its own lowered IO: `load_input` is not
  implemented in `nir_to_spirv` without Zink's own passes, so A fails nearly every GLSL program).
- B's 17 differences are all undefined behaviour on the seeded inputs: 14 index an array out of its
  range with a seeded uniform (`fs-vec4-const-array-indirect-access-*` 8, `glsl-array-bounds-10`
  and `-12`, `fs-mat4-out-of-bounds-1`, `glsl-fs-varying-array`, `vs-inout-index-inout-mat2-col-array`,
  `array-initializers-1`; llvmpipe reads zero, naga's SPIR-V clamps the index), 2 clamp a NaN
  (`fs-saturate-sqrt`, `vs-saturate-sqrt`: `sqrt` of a negative seeded value), 1 divides an integer
  by zero (`fs-loop-unrolled-divide-by-zero`). Left as differences (known, for this reason).
- B's 98 not compared: textures and images not filled on the GLSL side 84, subroutine uniforms 9,
  a state value with no name 2 (to look at), and 3 drawn programs llvmpipe or the bridge refused
  (an extension, too many attributes, ES versions that differ).
- B's 343 failures by name: `load_ubo` 93 (64-bit loads of `arb_gpu_shader_fp64`, bindless
  handles, uniform block arrays indexed at run time), naga validation of route B's output 126
  (type resolution in fragment programs and "expected data type": route faults, to look at first), a stage value interpolated two ways 15,
  textures of other dimensions or operands 25, `gl_PrimitiveID` and `gl_ClipVertex` (Mesa slots 21
  and 16) 17, 64-bit values 14, binding arrays the common form does not allow 21, and smaller
  groups (`scripts/glsl_reasons.py <results> nir-naga` in scratch counts them).

Run 8, with bgfx's 325 desktop GLSL shaders beside piglit and the SPIR-V corpus (3871 files;
`results/run8/nir-results-r8d.jsonl`; x86-64 machine, 192 children, load 4.5 at the start and
16.1 at the end, 250 s):
- SPIR-V corpus: unchanged, A 728 match, B 773 match, 0 differ.
- piglit: route B 1310 match (up from 1152), 17 differ (the same 17, same reasons), 17 not
  compared (subroutine uniforms 9, storage images 3, a state value with no name 2, and 3 drawn
  programs llvmpipe or the bridge refused), 266 fail by name. With storage images (the last run,
  `nir-results-r8e.jsonl`, load 7.8 at the start and 9.0 at the end, 250 s): 1313 match, 14 not
  compared, the rest unchanged. Of B's failures, 25 are naga
  validation (21 binding arrays and 1 storage binding array the common form does not allow, 3
  vertex programs without a position); every other one is a named "not taken: ..." (`load_ubo` 94:
  64-bit, bindless and uniform block arrays indexed at run time; Mesa slots of other built-ins 17;
  a stage value interpolated two ways 15; a comparison with a bias or gradients 15; a structure
  holding samplers 17; 64-bit values and constants 20; and smaller groups). Route A: 3 match.
- bgfx: 4 files refused by Mesa's compiler (3 compute programs over 256 invocations a group, 1
  needs `GL_ARB_shader_viewport_layer_array`). Route B 281 match, 3 differ, 23 not compared
  (storage images on the GLSL side 20, a storage format the harness does not make 3), 14 fail by
  name (`ubitfield_extract` 6, a storage image of another format 5, `isign` 3). Route A 9 match
  (267 of its failures are Zink's compiler aborting on GLSL's lowered stage values). With storage
  images (r8e): route B 301 match, 3 not compared (`rgba16f` storage, not made by the harness).
- bgfx's 3 differences: `fs_hdr_bright` reads seeded texels as RGBE (`exp2(w * 255 - 128)`, with
  w in [-2, 2)), which overflows to infinity; llvmpipe gives 0 where the route gives NaN, and
  GLSL leaves operations on infinities to the implementation (the class of piglit's NaN through
  clamp). `fs_shadowmaps_packdepth_vsm` and `_linear` pack a depth with `fract(255 * z)`, z from a
  division by a seeded w; the two differ by one unit in the last place of `255 * z` (2^-13 at that
  size), which `fract` makes an absolute difference past the tolerance; GLSL allows division 2.5
  units in the last place. That the division is the source is expected, not yet confirmed.

Run 9, with dEQP's GLES cases beside piglit, bgfx and the SPIR-V corpus (17,501 items;
`results/run9/nir-results-r9a.jsonl`; x86-64 machine, 192 children, load 12.2 at the start and 11.4
at the end, 248 s):
- SPIR-V corpus: unchanged, A 728 match, B 773 match, 0 differ.
- piglit: route B 1338 match (up from 1313), the same 17 differences.
- bgfx: route B 307 match (up from 301), the same 3 differences, 6 not compared (`rgba16f` storage
  images), 5 fail by name (a storage image of another format).
- dEQP (13,630 items): 4,923 reach no route (expected failures 4,396: compile 3,983, link 189,
  compile or link 144, validation 80; tessellation 280 and geometry 179;
  50 with outputs past the colour targets; 13 need `GL_ANDROID_extension_pack_es31a`; a few that
  Mesa's compiler refuses). Of the other 8,707: route B 8,574 match, 126 differ, 2 not compared, 5
  fail by name or validation; route A converts 129 and matches none (Zink's compiler takes only its
  own lowered IO, as with piglit).
- dEQP's 126 differences are all undefined behaviour on the seeded inputs: integer division by
  zero (`swizzle_math_operations.test` `vector_divide` of `ivec` and `uvec`, 68; the `div` cases of
  `implicit_conversions.test`, 42, where a float divides by a zero made from an integer and the
  sign of that zero differs: NIR's `i2f(f2i(x))` to `ftrunc(x)` keeps a negative zero), and an
  array indexed past its end with a seeded index (`large_constant_arrays.test`, 16). Left as
  differences with this reason, as piglit's are.
- dEQP's 5 route failures: a texel buffer, an image atomic, a storage image of another format (all
  named), and one cube map array program naga refuses ("Index 2 is out of bounds", to look at); 2
  not compared: a uniform llvmpipe gives no value for (`linkage.test#varying_7`).

Run 10, with Godot's Compatibility shaders beside the rest (17,650 items;
`results/run10/nir-results-r10a.jsonl`; x86-64 machine, 192 children, load 8.4 at the start and
10.8 at the end, 248 s):
- SPIR-V corpus: route B 776 match (up from 773), route A 731 (up from 728), 0 differ. The 3 new
  on each are the vertex programs without a position (HLSL's and Slang's `geometryshader/base.vert`,
  HLSL's `displacement/base.vert`).
- piglit: route B 1341 match (up from 1338: `glsl-no-glposition`, `sample-qualifier-vs-fs`,
  `pervertex-pointsize-vs-out-fs-140`, all without a position), the same 17 differences.
- bgfx and dEQP: as run 9 (307 and 8,574 match; 3 and 126 differ, same reasons).
- Godot (241 items): 7 reach no route, refused by Mesa's compiler with its log: 4 with
  `RENDER_MOTION_VECTORS` set put an attribute at location 16, past the 16 attributes our OpenGL
  reports (Godot asks for motion vectors only where the driver has more); 3 are combinations Godot
  never builds (`canvas` `USE_INSTANCING` alone, `scene` depth modes with `USE_ADDITIVE_LIGHTING`).
  Of the other 234: route B 231 match, 2 differ, 1 not compared; route A converts none (Zink's
  compiler and GLSL's lowered IO, as with piglit).
- Godot's 2 differences: `effects/cubemap_filter.glsl` (both modes) indexes a `mat3[6]` with the
  seeded `face_id`, negative on the seed: undefined. Confirmed: with the index clamped to 0..5 (a
  probe copy, not kept) both match (`results/run10/nir-results-r10-probe.jsonl`). Left as
  differences with this reason, as piglit's are. Not compared: `canvas_sdf.glsl#MODE_LOAD_SHRINK`
  loops `1 << shift` squared times with the seeded `shift` negative (undefined; the reference does
  not end), the class of `irradiancecube.frag`.

Run 11, with Godot's RenderingDevice shaders beside the rest (18,222 items;
`results/run11/nir-results-r11b.jsonl`; x86-64 machine, 192 children, load 26.7 at the end (another
run of ours had just ended), 250 s):
- In all: route B converts 11,811 of 12,234 programs that reach a route and matches 11,626 (155
  differ, 30 not compared); route A converts 1,154 and matches 993 (7 differ, 154 not compared).
- SPIR-V samples, piglit, dEQP, Godot's Compatibility shaders: as run 10.
- bgfx: route B 316 match (up from 307), 5 differ, 0 not compared; all 321 convert. The 2 new
  differences were not compared before (`r16f` and `rg16f` storage images were not made):
  `cs_assao_prepare_depths_and_normals` and its `_half` variant write storage images whose values
  differ well past rounding (e.g. 0.000039 against 0.00083). Not yet explained: to look at (Next).
- Godot RenderingDevice (572 programs): 54 need what our Vulkan 1.1 does not report (16-bit floats
  and the like, counted apart). Of the other 518: route B 387 match, 5 differ, 4 not compared, 122
  fail by name; route A 237 match, 7 differ, 274 fail (Zink's compiler aborts on `bany_inequal`,
  `ball_iequal` and on derefs of some variable modes; multiview refused by validation). Route B's
  failures: `load_view_index` 66 (multiview: needs a helper value, Requests), arrays of textures
  and samplers 21 and of storage images 3 (15.4's rewrites, blocked), `reduce` 16 (subgroups,
  which the Vulkan driver lowers to one invocation before the route, 15.4: G10), a subpass input 4,
  `image_deref_atomic` 3, `load_helper_invocation` 2, a storage image without a format 2 (15.4,
  blocked), a structure member crossing the structure's end 2 (FSR2, naga validation), a texture
  operand of another kind 2, `quad_swap_horizontal` 1. Not compared: multisampled textures 4.
  Differences: `sdfgi_debug_probes` vertex programs (4; route A differs on the same ones, so the
  harness or undefined behaviour is suspected, not yet confirmed) and `fsr2_tcr_autogen` (1).

Run 12, the same 18,222 items after run 12's fixes (`results/run12/nir-results-r12b.jsonl`;
x86-64 machine, 172 children on cores 10-95 and 106-191 (cores 0-9 and their siblings left to the
timed choice), load 6.0 at the start and 13.3 at the end, 251 s):
- In all: route B converts 11,811 and matches 11,637 (up from 11,626), 153 differ, 21 not
  compared; route A unchanged (993 match, 7 differ, 154 not compared).
- bgfx: route B 318 match (up from 316), 3 differ (run 8's three, same reasons). The two
  `cs_assao_prepare_depths_and_normals` programs were a route fault: route B dropped the offset of
  `texelFetchOffset`, so every target read the texel at the bare coordinate (the route's four
  targets held the same value). Fixed (Done); they match.
- piglit: route B 1350 match (up from 1341: the 9 subroutine programs), the same 17 differences, 4
  not compared.
- Godot's RenderingDevice programs: the 5 differences are undefined behaviour on the seeded
  inputs, each confirmed by a probe copy of the program (not kept; `probe-sdfgi`, `probe-fsr2`
  under the x86 machine's build folder): `sdfgi_debug_probes` computes `probe_index %
  params.probe_axis_size` and `occluder_index % diameter` with the seeded size 0 (an integer
  remainder by zero; naga's SPIR-V makes it defined, lavapipe does not): with the size kept at
  least 2 and `probe_voxels` at least 1 all four match on both routes.
  `fsr2_tcr_autogen` divides by the seeded `iRenderSize`, 0, and turns the infinite coordinate
  into an integer (undefined): with the render size kept at least 1 it matches. Left as
  differences with these reasons, as piglit's are.
- Godot's Compatibility shaders, dEQP and the SPIR-V samples: as run 11 (the one multisampled
  program, `deferredmultisampling/deferred.frag`, briefly failed validation after the offset
  lowering, fixed before this run: it is again not compared, as the harness does not fill
  multisampled textures).

Run 13, the same 18,222 items after run 13's changes (`results/run13/nir-results-r13a.jsonl`;
x86-64 machine, 172 children on cores 10-95 and 106-191 beside the timed choice, load 14.3 at the
start and 3.7 at the end, 732 s, most of it the references that do not end, held on further seeds):
- In all: route B converts 11,813 of 12,234 (up from 11,811) and matches 11,651 (up from 11,637),
  153 differ (the same, with the same reasons), 9 not compared (down from 21; each named in
  Harness limits, with the bindless program llvmpipe refuses); route A matches 1,000 (up from 993),
  7 differ, 147 not compared.
- The 14 new matches on route B: the 2 dEQP cube programs (route faults, fixed), the 5 programs
  reading multisampled textures, and the 7 whose reference did not end on the run's seed: the six
  `irradiancecube.frag` on seed 0x5eed + 1 or + 3, `canvas_sdf.glsl#MODE_LOAD_SHRINK` on + 3. No
  outcome changed in any other way (`scripts/outcome_changes.py`).

Run 14, the same 18,222 items after run 14's changes (`results/run14/nir-results-r14b.jsonl`;
x86-64 machine, 180 children, load 11.4 at the start and 3.3 at the end, 731 s; `r14a` is the
same run before the harness limits, the implicit level and `ARB_shader_atomic_counter_ops`):
- In all: 12,284 items reach a route (up from 12,234: the atomic counter programs). Route B
  converts 11,894 and matches 11,732 (up from 11,651), 154 differ, 8 not compared; route A
  matches 1,013, 7 differ, 147 not compared.
- By source, route B: piglit 1,417 (up from 1,350), Godot RD 397 (up from 391), Sascha Willems
  788 (up from 783), dEQP 8,579 (up from 8,576), bgfx 318, Godot's Compatibility shaders 232.
- The rewrites (the programs that failed for them in run 13): arrays of textures and samplers 19
  piglit, 4 Godot RD and 1 Sascha Willems match; 20 Godot RD wait on `rewrite`'s validation
  (IMMEDIATES, above); refused by name by the shaders crate, `ResourceArray` 6 (arrays of
  buffers, of 2D array textures, of storage images in piglit's out-of-bounds tests) and
  `ResourceArrayPair` 2 (the descriptor heap sample). Formatless storage: 2 Godot RD, 3 Slang
  samples and 6 piglit match; 1 differs (below). The run-time gather offset: 1 piglit matches.
- The 48 atomic counter programs and the workgroup programs: all compile now; most match
  (`arb_compute_shader/execution`, `arb_shader_atomic_counters`, `arb_gl_spirv` atomic uints,
  dEQP's two `max_fragment_atomic_counter*`); refused by name: signed minimum and maximum on
  storage or workgroup words (4: route B keeps words as u32, naga's signed atomics need i32),
  `load_workgroup_size` 1, `image_deref_atomic` 1, `load_helper_invocation` 1. 26 still need
  extensions OpenGL 4.6 does not have (`GL_NV_shader_atomic_int64`, float atomics).
- The one new difference: piglit's `imageStore-array` writes `tex[idx]` with `idx` a seeded
  uniform in [-4, 4), past the array's two elements on most seeds (undefined); with the index
  clamped to the array in a probe copy (not kept) it matches on seeds 1 to 4. Left as a
  difference with this reason, as piglit's other out-of-range indices are.
- Differences depend on the seed (from G8's check, seed 90210): bgfx's `cs_atmo_transmittance`
  takes `sqrt` of a seeded negative and clamps the NaN (the class of piglit's NaN through clamp)
  and differs on that seed, while the three Godot RD differences explained by a seeded zero
  (`sdfgi_debug_probes` 0 and 1, `fsr2_tcr_autogen`) match there.
- Candidate A on GLSL (from G8's check, information): A converts a GLSL compute probe of shared
  memory and gives wrong values; A is not chosen and fails most GLSL. It shows the GLSL comparison
  catches a wrong route.
- The 8 not compared: the 5 harness limits below (2 state values, 2 uniforms llvmpipe removes, 1
  past llvmpipe's output components), `alpha_to_one` (G10's CTS), and 2 programs llvmpipe refuses
  (`GL_ARB_bindless_texture`, `GL_ARB_compute_variable_group_size`).

Run 15, the corpus with the Vulkan CTS's `without_format` and `buffer_access` programs added
(18,738 items; `results/run15/nir-results-r15a.jsonl`; x86-64 machine, 172 children on cores
10-95 and 106-191 beside the timed sample, load 41.1 at the start and 100.4 at the end (other
tracks' jobs), 735 s):
- In all: 12,748 items reach a route. Route B converts 12,229 and matches 12,069, 154 differ, 6
  not compared; route A converts 1,229 and matches 1,075, 7 differ, 147 not compared.
- Against run 14, on the same 18,222 items, route B's only changed outcomes are the two
  fixed-function programs (`glsl-fs-normalmatrix`, `glsl-vs-ff-frag`): not compared, now matched
  (`scripts/outcome_changes.py`). The unordered-comparison fix changes nothing on seed 0x5eed.
- The 516 CTS programs: `without_format` 354 (route B 273 match, 81 fail by name on texel
  buffers; route A 0: naga's SPIR-V reader refuses `StorageImageReadWithoutFormat`),
  `buffer_access` 162 (above, Waiting on others).
- The NaN programs again, on seeds 0x5eed, 271828, 90210 and 1 with 256 input sets each (a probe
  folder of five, not kept): piglit's `arb_gl_spirv` `unordered-comparison` matches on all four
  (it differed on 271828 before the fix). `fs-saturate-sqrt` and `vs-saturate-sqrt` differ on
  all four: `clamp(sqrt(v), 0, 1)` of a seeded negative, llvmpipe 0 and the route 1 for the NaN;
  GLSL leaves `sqrt` of a negative undefined, so they stay in that class. bgfx's
  `cs_atmo_transmittance` (a `sqrt` of a seeded negative, clamped) differs on 90210 and 1, and
  `fs_hdr_bright` (operations on an overflowed infinity) on 271828, 90210 and 1; both match on
  0x5eed. Neither depends on an unordered comparison.
- Outcomes that depend on the seed, from G8's second check (seed 271828, 256 input sets; all of
  the classes above): route B, matched on 0x5eed and differing there, glsl-1.10's
  `fs-input-array-float-index-rd` and `fs-input-array-vec2-index-rd` (a seeded negative index),
  glsl-1.20's `vs-inout-index-inout-vec4-array-element-array` (an index from a seeded uniform),
  `glsl-array-bounds-13`; not compared there, `glsl-fs-varying-array` (the reference's outputs
  differ between its two runs); matched there and differing on 0x5eed, the three Godot RD
  programs explained by a seeded zero and `imageStore-array`. Route A only: `scene_forward_clustered`
  `10.vert` differs there by 9e-6, past the tolerance, and `descriptorheap` `cube.vert` crashed in
  the comparison (SIGSEGV), neither looked at further (A is not chosen).

The choice (final; 15.3): the route is B (`nir-naga`, `src/direct.rs`). It follows from the
counts, which are far apart on every source; the times agree with them. Since run 15 both routes
are timed on the same work: the common form's step and the conversion to a validated module (B's
time also holds the 15.4 rewrite where a module needs one, which A never reaches). Before run 15,
B's time also held the cache key (`nir_serialize` and BLAKE3), which A's did not: every B time
below from runs 4 to 14 counts that against B.
- Timed alone on a seeded sample with run 15's code (`WC_NIR_SHADERS=2000`, seed 0x5eed, the
  corpus of 18,738 items with the CTS programs; `results/run15/nir-choice-r15.jsonl`; x86-64
  machine, cores 0-9, load 98.0 at the start and 54.2 at the end (the whole corpus run and other
  tracks' jobs on the other cores), about 1,850 s): B matches 1,288 and A 119 (of 1,358 that reach
  a route); on the 118 both match, A median 375 us, mean 507 us; B median 300 us, mean 355 us.
  The machine was far busier than in run 13's sample (load 3.8), so both routes' times are higher;
  B is 20 % faster at the median.
- Run 15's corpus run: B matches 12,069 programs, A 1,075; on the 1,074 both match, at 172 at
  once on a busy machine, A median 536 us, B median 428 us.
- Run 14's corpus run (above): B matches 11,732 programs, A 1,013; on the 1,012 both match, at
  180 at once on a busy machine, A median 519 us, B median 448 us. The same order on every
  source as run 13's, below.
- Over the whole corpus (run 13's corpus run, 18,222 items, both routes, the comparison):
  B matches 11,651 programs, A 1,000. By source: SPIR-V samples B 783 and A 738; the Vulkan CTS's alpha_to_one vertex program 1 each; Godot's
  RenderingDevice SPIR-V B 391 and A 237; piglit B 1,350 and A 3; bgfx B 318 and A 21; dEQP
  B 8,576 and A 0; Godot's Compatibility shaders B 232 and A 0 (A takes almost no GLSL: Zink's
  compiler needs Zink's own lowered IO). On the 998 programs both match, at 172 at once on a busy
  machine: A median 542 us, B median 429 us.
- Timed alone (`corpus/choose`'s way: optimised, one program and route at a time, ten cores), on
  a seeded sample of 2,000 items with run 13's code (`WC_NIR_SHADERS=2000`, seed 0x5eed;
  `results/run13/nir-choice-r13.jsonl`; x86-64 machine, cores 10-19 (0-9 held run 11's choice),
  load 3.8 at the start and 2.6 at the end, 1691 s): B matches 1,285 and A 119 (of 1,346 that
  reach a route); on the 118 both match, A median 237 us, mean 325 us; B median 174 us, mean
  213 us.
- Timed alone over the whole corpus (run 11's `corpus/choose`, run 11's code, all 18,222 items,
  one program and route at a time on cores 0-9; job `graphics-nir-choice/20261120-064354-2508593`;
  `results/run13/nir-choice-r11.jsonl`; x86-64 machine, load 23.7 at the start (other runs of
  ours) and 0.6 at the end, other tracks' and this track's runs beside it on other cores, 16,673 s
  wall): B converts 11,811 of 12,234 and matches 11,626; A converts 1,154 and matches 993. On the
  991 programs both match: A median 252 us, mean 404 us; B median 193 us, mean 250 us. B is ahead
  on the count by 10,633 programs and 23 % faster at the median (193 us against 252 us). Its counts are run 11's corpus
  run's exactly (run 12's and 13's fixes came after).
- Run 4's timed choice, on the SPIR-V corpus (ten cores, load 5.5 to 3.2, before the Zink patch):
  A 726, B 773 match; on the 725 both match, A median 219 us, B median 107 us.
- Dolphin is not in the corpus (Next 4); the choice does not wait on it: B leads on every source.

Harness limits left, each with why it is not closed (run 15; programs route B converts but the
comparison does not hold, of 18,738 items; 6 in all with the two programs llvmpipe refuses):
- A uniform llvmpipe's linker removes and the route's program reads (2 dEQP: `linkage.test#varying_7`,
  `varying.rules.vertex_use_fragment_use`, `u_false`): OpenGL has no value for it, and inventing one
  would compare nothing. Left not compared.
- A drawn program past llvmpipe's 128 output components once the harness's position output is
  added (1 piglit: `vs-fs-max-in-out-components`): the reference refuses it; left.
- `alpha_to_one`'s fragment program: held by G10's CTS cases (lavapipe does not run
  `demote_samples`).

Next (after G8's check):
1. G9, the OpenGL driver (15.8). It hands the route NIR made with the bridge's compiler options and
   st/mesa's variants; its own corpus run holds those variants, the fixed-function state values
   above among them.
2. When the shaders crate's `rewrite` validates with `Capabilities::IMMEDIATES`: rerun the
   corpus; 20 Godot RD programs with arrays of textures should then reach the comparison.
3. When `HelperValue::BufferRange` is decided: robustBufferAccess's steps 1 and 3, and lower the
   `buffer_access` programs (in the corpus since run 15) before both runs.
4. Dolphin (GPL: run as a test input only, never copied or paraphrased) is left out of G8's corpus:
   its shaders exist only when Dolphin runs, and no permissively licensed FIFO log or homebrew to
   boot was found. One clean way is recorded for later: a boot image written here (a few
   instructions of our own, so its licence is ours) makes Dolphin compile its ubershaders at start
   on llvmpipe with `MESA_SHADER_DUMP_PATH` set. Not done in G8.
5. Candidate A is not developed further (B is chosen); its failures stay listed as they are.

## How to run

- The corpus: `crates/wcontainers-nir/corpus/fetch` (into `$CARGO_TARGET_DIR/nir-corpus`), then with
  `WCONTAINERS_MESA` set: `WC_NIR_CORPUS=$CARGO_TARGET_DIR/nir-corpus WC_NIR_RESULTS=<file> cargo
  test -p wcontainers-nir --test corpus`. From the Mac: `orchestrator/cloud/x86-run <checkout> --name
  graphics-nir --jobs 64 -- 'crates/wcontainers-nir/corpus/fetch && export
  WCONTAINERS_MESA=$CARGO_TARGET_DIR/mesa/build && WC_NIR_CORPUS=$CARGO_TARGET_DIR/nir-corpus
  WC_NIR_RESULTS=$CARGO_TARGET_DIR/nir-results-a.jsonl cargo test -p wcontainers-nir --test corpus'`,
  then `x86-run --fetch graphics-nir target/nir-results-a.jsonl <scratch>`. 1005 shaders take about
  2 minutes with the comparison (192 at once). Run 3's results are in
  `pyro-parked/wcontainers-scratch/graphics-nir/results/run3/nir-results-a.jsonl` (run 2's, conversion
  only, one folder up); run 4's in `results/run4/` (`nir-results-r4b.jsonl` the last, with the
  patch; `nir-choice-r4.jsonl` the timed one); `scripts/reasons.py <results> [n]` counts the reasons.
- The choice between the candidates, one command: `crates/wcontainers-nir/corpus/choose [results]`
  (fetches the corpus, builds optimised, runs one shader and route at a time on ten cores; its last
  lines give each route's counts and its median and mean time on the shaders every route matches).
  From the Mac: `x86-run <checkout> --name graphics-nir -- 'WC_NIR_TIMING_CORES=0-9
  crates/wcontainers-nir/corpus/choose $CARGO_TARGET_DIR/nir-choice.jsonl'` (the whole corpus of 18,222 items took 4 h 38 min in run 11; most of it is Zink's compiler
  aborting on GLSL in route A, each abort a core dump the machine's `apport` writes).
  `WC_NIR_SHADERS=<n>` takes a seeded sample: for the independent check,
  `WC_NIR_TIMING_CORES=0-9 WC_NIR_SHADERS=2000 crates/wcontainers-nir/corpus/choose
  $CARGO_TARGET_DIR/nir-choice-check.jsonl` (run 15: about 1,850 s at load 54 to 98; B 1,288 match,
  A 119; run `corpus/cts-programs` first, or the sample is drawn from fewer items). A reference
  that does not end on the seeded inputs is held on up to four seeds, two minutes each, so a
  sample with such programs takes longer.
- The licence of every linked Mesa file: `crates/wcontainers-nir/mesa/linked-licences [file]` (after
  the Mesa build; the route test linked with a map, each kept object traced to its source).

- One group alone (G9 run 7): add `-Include <regex>[,<regex>]` to `run-windows.ps1`, e.g.
  `-Lists gles2-main -Include dEQP-GLES2.functional.vertex_arrays.*`. The refusals of a whole
  run, from the Mac: `ssh -o BatchMode=yes tom@192.168.2.183 'findstr /C:"refused:" /C:"not
  built:" C:\wc\graphics-nir\cts\results\wcontainers\<list>\*.log'`.
- Reading a run's failures (G9 run 6): copy the list's folder back (`scp -o BatchMode=yes -r
  tom@192.168.2.183:C:/wc/graphics-nir/cts/results/wcontainers/<list> <scratch>`), then
  `scripts/qpa_images.py <case>.qpa` (scratch) writes each picture of a case's log as a PNG, and
  `scripts/crash_reasons.py <folder> <runner output>` lists each failed case with the driver's
  refusals in its process's log (a process runs many cases: a refusal is the crash's own, a
  validation error may be an earlier case's).
- Build Mesa for Windows (G9) on the desktop, from a checkout on the Mac: `tools/graphics-rig/remote
  'powershell -NoProfile -ExecutionPolicy Bypass -File crates\wcontainers-nir\mesa\build-windows.ps1'`.
  Tools, source, build and install under `C:\wc\graphics-nir` (kept between runs; the tools'
  downloads with their SHA-256 printed); `opengl32.dll` and `libgallium_wgl.dll` in
  `C:\wc\graphics-nir\mesa\install\bin`. Uses half the desktop's threads. The first build
  after G9 run 5 also builds llvmpipe's LLVM from the 21.1.8 source into
  `tools\llvm-21.1.8-mt` (X86 only, static runtime, libraries, headers and llvm-config; the
  source and build folders deleted after). A remote command waits
  while another track's hazard job holds the GPU slot. Kept between runs on the desktop for G9:
  `C:\wc\graphics-nir`, 4.8 GB at the end of G9 run 1 (the unpacked tools, LLVM most of it; the
  Mesa source, build and install); the downloaded archives were deleted (fetched again only if a
  tool is missing).
- The conformance suite on the desktop (G9): build once with `tools/graphics-rig/remote
  'powershell -NoProfile -ExecutionPolicy Bypass -File crates\wcontainers-opengl\cts\build-windows.ps1'`
  (glcts and deqp-runner; about 40 minutes the first time, at 8 jobs beside other tracks'
  builds). Run lists as a GPU job: `tools/graphics-rig/gpu-job --desktop --build 'if not exist
  "%CARGO_TARGET_DIR%" mkdir "%CARGO_TARGET_DIR%"' -- 'powershell -NoProfile -ExecutionPolicy
  Bypass -File crates\wcontainers-opengl\cts\run-windows.ps1 -Driver <wcontainers|llvmpipe|system>
  [-Lists gles2-main,gl46-main] [-Fraction 100 -FractionStart 1]'` (a list's name is its file's
  name without `.txt`). It prints each list's counts and share; deqp-runner's `results.csv`,
  `failures.csv` and logs are in `C:\wc\graphics-nir\cts\results\<driver>\<list>`. One case
  alone, for a hang or a crash: `tools/graphics-rig/gpu-job --desktop --build 'echo sent' --
  'powershell -NoProfile -ExecutionPolicy Bypass -File
  crates\wcontainers-opengl\cts\run-case-windows.ps1 -Driver wcontainers -List gles2-khr-main
  -Case <case> [-Seconds 40]'` (glcts's output, exit code and log printed; killed and reported
  as hung after the limit). Add `-Debugger` to run it under the LLVM release's lldb (with Python
  3.10 from NuGet, fetched once): every thread's stack and the stopped thread's stack words as
  symbols, read from the PDB files the build keeps (`buildtype = 'debugoptimized'`, no
  assertions). The release's folder had to be opened to the local Users group once
  (`build-windows.ps1` now does it when it unpacks LLVM), as the hidden desktop's user could
  not run lldb. Linting the driver on the desktop: `tools/graphics-rig/remote 'set
  WCONTAINERS_MESA=C:\wc\graphics-nir\mesa\build&& set RUSTFLAGS=-C target-feature=+crt-static&&
  set LIBCLANG_PATH=C:\wc\graphics-nir\tools\clang+llvm-21.1.8-x86_64-pc-windows-msvc\bin&& cargo
  clippy -q -p wcontainers-opengl --release --target-dir C:\wc\graphics-nir\opengl-target -- -D
  warnings'`; on the x86 machine with `WCONTAINERS_MESA` set and `LIBCLANG_PATH=$(nix build
  --no-link --print-out-paths --inputs-from path:/work/flake/graphics-nir
  nixpkgs#llvmPackages.libclang.lib)/lib`. Logs of
  run 2 in scratch `logs/g9r2-*.log`, its `results.csv` files in `results/g9r2/cts/<driver>/<list>`. The Mesa build (above) must have run first for
  `wcontainers` and `llvmpipe`. Kept on the desktop after run 3 (reused by every build): `cts` 3.5 GB, `mesa` 0.7 GB, `opengl-target` 2.4 GB (now with wcontainers-nir and the shaders crate's C++), `tools` 3.9 GB; the run copy's submodules are checked out by the build. After run 2: `C:\wc\graphics-nir\cts`
  3.8 GB (the suite's source and build), `tools` 3.9 GB (with deqp-runner), `mesa` 0.8 GB,
  `opengl-target` 0.8 GB (the driver's cargo output, reused by each Mesa build).
- Build Mesa, from the Mac: `orchestrator/cloud/x86-run <checkout> --name graphics-nir --jobs 64 --
  'crates/wcontainers-nir/mesa/build path:/work/flake/graphics-nir'`. On any Linux machine with Nix:
  `crates/wcontainers-nir/mesa/build` (its argument, the flake, defaults to the checkout). The tools
  come from Nixpkgs' own `mesa` package (`nix print-dev-env --inputs-from <flake> nixpkgs#mesa`), so the
  repository's flake is unchanged and the tools are those of its locked nixpkgs. Output in
  `$CARGO_TARGET_DIR/mesa/{src,build,install}` (target/mesa without it). The script refuses a tag whose
  commit is not the pinned one, makes the build folder again when the commit or the options change
  (Meson reads a machine file only when a folder is made), and fails unless llvmpipe and lavapipe were
  built.
- GLSL tests: `cargo test -p wcontainers-nir --test route --test glsl_buffers --test
  glsl_reference` (with `WCONTAINERS_MESA`, on the x86 machine). The corpus run takes piglit's
  `.shader_test` files beside the `.spv` files, from the same `WC_NIR_CORPUS`; all 3546 take about
  4 minutes at 192 at once. `scripts/glsl_reasons.py <results> [route] [n]` (scratch) counts
  piglit's reasons; `scripts/piglit_outcomes.py <results> [route] [n] [prefix]` counts every
  outcome of the files under a prefix (`piglit/`, `bgfx/`). bgfx's shaders come with the same
  `corpus/fetch` and run in the same corpus run (any `.bin` in a folder named `glsl`).
- Test with Mesa (on the x86 machine, after the build):
  `export WCONTAINERS_MESA=$CARGO_TARGET_DIR/mesa/build; cargo test -p wcontainers-nir` and
  `cargo clippy -p wcontainers-nir --all-targets -- -D warnings`; and both again without the variable.
- dEQP's cases come with the same `corpus/fetch` (VK-GL-CTS's shader library data, sparse) and run
  in the same corpus run: 13,630 items take about 35 s alone (`WC_NIR_CORPUS` at
  `nir-corpus/vk-gl-cts`). `scripts/deqp_differences.py <results>` (scratch) groups route B's
  differences by file and message.
- The Vulkan CTS's programs: `crates/wcontainers-nir/corpus/cts-programs [flake]` (from the Mac:
  `x86-run <checkout> --name graphics-nir --jobs 96 -- 'crates/wcontainers-nir/corpus/cts-programs
  path:/work/flake/graphics-nir'`); the corpus run takes them from `nir-corpus` like the rest.
- The literal check needs the git history, which the rented machine's copy lacks. Run 6 ran it on
  the x86 machine from a git bundle of the branch (`git bundle create <scratch>/nir.bundle
  phase-2-graphics-nir`, sent with `x86-run --ssh 'cat > /work/probes/graphics-nir.bundle'`,
  cloned there, `cargo fetch`, then the check in `nix develop`; the clone and its build deleted
  after). Runs 1 to 5 ran it on the Mac (`RAYON_NUM_THREADS=3 cargo run -j 4 -q -p
  wcontainers-literals -- check`). With
  `crates/wcontainers-shaders/dxbc-spirv`'s own submodule checked out the check fails on its Python
  scripts (the baseline holds it not checked out): fetch only the shaders crate's two submodules and
  dxil-spirv's `third_party/spirv-headers`, which the shaders crate's build needs.

- A quick look at one shader: copy it alone into a folder and point `WC_NIR_CORPUS` there with
  `WC_NIR_DUMP` set; `nix shell nixpkgs#spirv-tools -c spirv-dis` reads the dumps.
- Run 8 fetched VK-GL-CTS's shader library data (sparse, at 5bb4c7e8) and a sparse bgfx on the
  x86 machine under `/work/probes` to read their formats; both deleted at the end of the run.
- Runs beside the timed choice: pin to `taskset -c 10-95,106-191` (cores 0-9 and their hyperthread
  siblings 96-105 are the choice's), and never build `--release` in `/work/target/graphics-nir`
  while it runs. Probe programs (a changed copy of one corpus program, compiled with Nixpkgs'
  glslang as `corpus/godot-forward-plus` does) go in a folder of their own under the build folder,
  pointed at by `WC_NIR_CORPUS`. `scripts/outcome_changes.py <old> <new> <route>` (scratch) lists
  the programs whose outcome changed between two results files.
- Kept between runs: a sparse clone of Dolphin's `Source/Core/VideoCommon` at 32a2b828 for reading
  how it makes its shaders (`pyro-parked/wcontainers-scratch/graphics-nir/downloads/dolphin`,
  GPL-2.0-or-later: read only, nothing of it copied or paraphrased here), and bgfx's two ASSAO
  sources and their `uniforms.sh` (`downloads/bgfx-src`, BSD-2-Clause).
- Kept between runs: a sparse clone of Godot at 73ca6be2 for reading its shader formats and their
  C++ users (`pyro-parked/wcontainers-scratch/graphics-nir/downloads/godot`, 12 MB, blobs fetched
  on demand: never `git grep` its history there, which fetches every blob; add paths with `git
  sparse-checkout add /<path>`).
- Godot's RenderingDevice shaders, one command after the fetch's sources: `crates/wcontainers-nir/corpus/godot-forward-plus [flake]`
  (from the Mac: `x86-run <checkout> --name graphics-nir --jobs 64 --
  'crates/wcontainers-nir/corpus/godot-forward-plus path:/work/flake/graphics-nir'`; 9 s). It
  fetches, writes the variants' sources (the corpus binary's `--godot-rd`, no Mesa needed) and
  compiles them with Nixpkgs' glslang into `nir-corpus/godot-forward-plus`, listing what glslang
  refuses in `glslang-refused`. Run it (and `corpus/cts-programs`) before `corpus/choose`, which
  fetches only `corpus/sources`. Alone: `WC_NIR_CORPUS=$CARGO_TARGET_DIR/nir-corpus/godot-forward-plus`.
- Godot's shaders come with the same `corpus/fetch` and run in the same corpus run (any `.glsl`
  under `drivers/gles3/shaders`); alone: `WC_NIR_CORPUS=$CARGO_TARGET_DIR/nir-corpus/godot`.
- Kept between runs: a sparse clone of VK-GL-CTS at 5bb4c7e8 for reading dEQP's formats
  (`pyro-parked/wcontainers-scratch/graphics-nir/downloads/vkglcts`, no build output); on the x86
  machine, the CTS source and build under `/work/target/graphics-nir/vk-gl-cts` (with Mesa and the
  rest, 18 GB in all after run 9).
- Kept between runs: a shallow clone of the pinned Mesa for reading on the Mac
  (`pyro-parked/wcontainers-scratch/graphics-nir/mesa-src`, 560 MB, no build output; logs beside it in
  `logs/`), and the Mesa build and cargo target on the x86 machine (`/work/target/graphics-nir`).
  No scratch copy of the checkout was made on the Mac, the laptop or the desktop. The literal check's
  own build in the checkout's `target` (282 MB) is deleted at the end of each run. On the x86
  machine `/work/target/graphics-nir` (11 GB after run 4: Mesa, the corpus, cargo's debug and release output, the last results) is kept.

## Measurements

- G9 run 7, wcontainers-nir's suite (x86-64 machine, load 1 to 5): fmt clean; clippy with
  warnings denied clean with and without Mesa (wcontainers-nir and wcontainers-opengl without it);
  with Mesa `glsl_buffers` 1, `glsl_reference` 10 and 2 ignored (the known failures), `narrow` 2,
  `route` 11 passed. The driver's clippy with warnings denied clean on the desktop with Mesa. The
  corpus run was not repeated (route B's changes this run: the shared NIR step's texture
  lowerings, which the GLSL corpus front already made before it).
- G9 run 7, the whole dEQP-GLES2 and KHR-GLES2 lists on our driver (desktop, RTX 3080, one GPU
  job each, 8 glcts at once, other tracks' jobs queued beside): 14:39 to 14:45 (with the build),
  14:57:12 to 15:01:49 for the last (4 min 37 s with the incremental build). `vertex_arrays`
  alone (571 cases): 10 s.

- The whole suite of wcontainers-nir and wcontainers-opengl (G9 run 6, x86-64 machine, load 3
  to 7): fmt clean; clippy with warnings denied clean with and without Mesa; with Mesa
  `glsl_buffers` 1, `glsl_reference` 10 and 2 ignored (the known failures), `narrow` 2, `route`
  11 passed, and the driver's doc tests none (bindgen no longer copies Mesa's comments, whose C
  examples rustdoc ran as Rust and failed: 2 failures before); without Mesa every target builds,
  no tests run. The corpus run was not repeated this run (route B's one change, the sample level
  outside comparisons, is held by the conformance runs; the corpus run is the next run's).
- The whole dEQP-GLES2 and KHR-GLES2 lists on our driver (G9 run 6, desktop, RTX 3080, one GPU
  job, 8 glcts processes at once, other tracks' jobs queued beside; the desktop's processors at
  8% just after): 17,638 cases in 6 min 45 s (14:00:07 to 14:06:52, with the job's start). A 1%
  shard of both (177 cases): about 35 s; a 10% shard (1,765 cases): 71 s.
- The Windows Mesa build with the driver, incremental after a driver change (desktop, 8 jobs,
  shared): about 1 to 2 minutes, most of it cargo's release build of the driver.

- The whole corpus run (G9 run 1, 18,738 items, 96 children, x86-64 machine, load 10.9 at the
  start and 2.2 at the end; a new spot machine, Mesa and the corpus built again first): 740 s
  (12 min 20 s). Route B: 12,068 match, 156 differ, 518 failed by name, 6 not compared (run 15:
  12,069, 154, 519, 6); the three changes are piglit's `fs-shl-of-shr-int64` (was "not taken:
  umul_high", now matches) and two Slang vertex programs of Sascha Willems' samples
  (`occlusionquery/simple.vert`, `subpasses/gbuffer.vert`) whose outputs the route drops, now
  differences by the new vertex check: each stores a load of a local the program never writes
  (checked in the disassembly), an undefined value, listed with the undefined-behaviour
  differences. Route A unchanged (1,075 match, 7 differ, 147 not compared). Time per shader to
  the validated common form at 96 at once on the 1,072 shaders both routes match: A median
  448 us, mean 710 us; B median 358 us, mean 460 us. Results: scratch
  `results/g9r1/nir-results-g9r1.jsonl`.
- The whole suite of wcontainers-nir (G9 run 1, x86-64 machine, load 2 to 15): clippy with
  warnings denied clean with and without Mesa; without Mesa every target builds (no tests run);
  with Mesa `glsl_buffers` 1, `glsl_reference` 10 and 2 ignored (the known llvmpipe failure and
  the negated NaN comparisons), `narrow` 2 (2,097,152 16-bit cases and 65,600 multiplications,
  0 differ), `route` 11 passed. `corpus/cts-programs` from nothing: about 400 s at `-j 96`.

- The whole corpus run (run 15, 18,738 items, 172 children on cores 10-95 and 106-191, x86-64
  machine, load 41.1 at the start and 100.4 at the end, other tracks' jobs and the timed sample
  beside it): 735 s. Time per shader to the validated common form at 172 at once on the 1,074
  shaders both routes match: A median 536 us, mean 863 us; B median 428 us, mean 556 us (B's time
  no longer includes the cache key, which neither route's now does).
- The whole suite of wcontainers-nir (run 15, x86-64 machine, load 84 to 115): fmt clean; clippy
  with warnings denied clean with and without Mesa; without Mesa every test target builds and
  passes (no tests run without it); with Mesa `glsl_buffers` 1, `glsl_reference` 10 and 1
  ignored (the known failure, llvmpipe's), `narrow` 1, `route` 11 passed.

- The whole corpus run (run 14, 18,222 items, 180 children, x86-64 machine, load 11.4 at the
  start and 3.3 at the end): 731 s. Time per shader to the validated common form at 180 at once
  on the 1,012 shaders both routes match: A median 519 us, mean 790 us; B median 448 us, mean
  564 us.
- The whole corpus run (run 13, 18,222 items, 172 children on cores 10-95 and 106-191, x86-64
  machine, load 14.3 at the start and 3.7 at the end, the timed choice on cores 0-9 beside it):
  732 s, up from 251 s: the references that do not end are now run on up to four seeds, two
  minutes each, one after another. Time per shader to the validated common form at 172 at once on
  the 998 shaders both routes match: A median 542 us, mean 831 us; B median 429 us, mean 519 us.
- The whole suite of wcontainers-nir (run 14, x86-64 machine, load 10.3 to 11.2): without Mesa
  every test target builds and passes (no tests run without it); with Mesa `glsl_buffers` 1,
  `glsl_reference` 9 and 1 ignored (the known failure), `narrow` 1, `route` 11 passed. Clippy with
  warnings denied clean with and without Mesa; the literal check exits 0 (from a bundle of the
  branch on the x86 machine, as run 6).
- The whole suite of wcontainers-nir (run 13, x86-64 machine, load about 14): without Mesa every
  test target builds and passes (no tests run without it); with Mesa `glsl_buffers` 1,
  `glsl_reference` 7, `narrow` 1, `route` 11 passed.

- The whole corpus run (run 12, 18,222 items, 172 children on cores 10-95 and 106-191, x86-64
  machine, load 6.0 at the start and 13.3 at the end, the timed choice running on cores 0-9 and
  other tracks' jobs beside): 251 s. Time per shader to the validated common form at 172 at once
  on the 991 shaders both routes match: A median 454 us, mean 714 us; B median 358 us, mean
  440 us. The same run before the subroutine change (load 87 at the start, 27 at the end): 253 s.
- The whole suite of wcontainers-nir (run 12, x86-64 machine, load about 6): without Mesa every
  test target builds and passes (no tests run without it); with Mesa `glsl_buffers` 1, `glsl_reference` 7,
  `narrow` 1, `route` 11 passed.
- The whole corpus run (run 11, 18,222 items, 192 children, x86-64 machine, load 10.0 at the
  start and 26.7 at the end, another run of ours just before): 250 s. Time per shader to the
  validated common form at 192 at once on the 991 shaders both routes match: A median 488 us, mean
  767 us; B median 381 us, mean 481 us (busy machine; the choice's times are the ten-core ones).
  `corpus/godot-forward-plus` (fetch, the sources, 572 glslang compilations at 192 at once): 9 s,
  load 1.0. Godot's RenderingDevice programs alone: 16 s, load 0.8 to 1.7.
- The whole corpus run (run 10, 17,650 items, 192 children, x86-64 machine, load 8.4 at the start
  and 10.8 at the end): 248 s. Time per shader to the validated common form at 192 at once on the
  748 shaders both routes match: A median 421 us, mean 607 us; B median 346 us, mean 406 us (busy
  machine; the choice's times are the ten-core ones). Godot's 241 items alone: 128 to 134 s (most
  of it the one program whose reference does not end, held to the 120 s limit).

- `corpus/cts-programs` from nothing (VK-GL-CTS clone, its `fetch_sources.py`, CMake and Ninja
  for `vk-build-programs`, the alpha_to_one programs), x86-64 machine, `-j 96`, load 5.2 at the
  start and 10.4 at the end: 320 s wall, 76 min user.
- dEQP's 13,630 items alone, 192 children, x86-64 machine, load 0.1 to 32.6: about 32 s.

- Mesa 26.3.1 clean build (configure, 1239 steps, install), x86-64 machine (c7i.metal-48xl, 192
  threads), `-j 64`: 45.9 s wall, 11 min 37 s user, load average 4.96 at the start and 10.51 at the
  end (other tracks' jobs). An earlier clean build of the same options with SPIRV-Tools: 45.9 s wall
  at load 0.43 at the start.
- Corpus run with the comparison, 1005 shaders, 192 children, x86-64 machine at load 0.3 rising to
  5.3: 124 s including an incremental build (run 3).
- The corpus run with bgfx (run 8, 3871 files, x86-64 machine, 192 children, load 4.5 at the start
  and 16.1 at the end): 250 s. At 192 at once, on the 735 shaders both routes match: A median
  441 us, B median 352 us (a busy machine; the choice's times are the ten-core ones).
- The whole suite with the SPIR-V corpus and piglit compared (run 7, x86-64 machine, 192 children,
  load 4.9 at the start and 13.2 at the end): the corpus run 245 s for 3546 files. Time per shader
  to the validated common form at 192 at once, on the 728 shaders both match: A median 431 us, B
  median 341 us (busy machine; the choice's times are the ten-core ones below).
- The whole suite with the SPIR-V corpus compared (run 6, after the GLSL work; x86-64 machine,
  192 children, load 1.0 at the start and 12.5 at the end, 245 s with the build;
  `results/run6/nir-results-r6.jsonl`): the same counts as runs 4 and 5, A 728 match, B 773
  match, 0 differ, 8 not compared on each.
- `tests/glsl_buffers.rs` and `tests/route.rs` (run 6): 0.08 s and 0.01 s, x86-64 machine, load
  0.5 to 4.2.
- `tests/narrow.rs` (2,097,152 invocations, both programs on lavapipe): 0.47 s, x86-64 machine,
  load 0.08 (run 5).
- The choice's timing (run 4, `corpus/choose`, x86-64 machine, ten cores, one child at a time, load
  5.5 at the start and 3.2 at the end with another track's tests running): 1781 s in all; time per
  shader to the validated common form on the 725 shaders both match: A median 219 us, mean 261 us;
  B median 107 us, mean 125 us. The same at 192 children at once (load 9 to 25): A median 383 us, B
  median 196 us.

## Routed decisions (15.9, owner not yet answered; followed as the orchestrator set them)

- Our own OpenGL driver on the interface, as 15.2 recommends. gex_wgpu and wvk are not adopted; nothing of
  theirs is copied or paraphrased (their repository has no licence; a related project is AGPL-3.0).
- Crate and module names are the plan's suggestions: `wcontainers-nir`, `wcontainers-opengl`.
- Mesa under its MIT licence with the permissive files 15.2 lists; dEQP, VK-GL-CTS and piglit for testing only.
- No change to the interface crate: what the second batch needs from it (15.7) is written below as requests.

## Licences brought in (for the owner to confirm)

- G9 run 6: the DirectX Shader Compiler's Windows release v1.9.2609 (`dxcompiler.dll`,
  `dxil.dll`), fetched by `mesa/build-windows.ps1` onto the desktop only and placed beside the
  test driver; not in this repository. Its LICENSE.TXT is the University of Illinois/NCSA Open
  Source License (permissive) with notices for its LLVM-derived parts; to confirm from the
  archive's own files before anything ships with it. The `cc` crate (MIT or Apache-2.0, already
  a build dependency of wcontainers-nir) is now also a build dependency of wcontainers-opengl.

- Mesa 26.3.1 (fetched by the build scripts, not in this repository). Its `docs/license.rst` says
  each file's SPDX tag governs; the core is MIT. Linked into wcontainers-nir's tests: Mesa's static
  libraries `libzink`, `libglsl_standalone`, `libglsl`, `libglcpp`, `libglsl_util`,
  `libglcpp_standalone`, `libmesa`, `libvtn`, `libnir`, `libcompiler`, `libmesa_util`, `libblake3`
  and `libmesa_util_c11` (the linker keeps only the objects used). The files kept, listed again in
  G9 run 1 by `mesa/linked-licences` (scratch `results/g9r1/nir-linked-licences-g9r1.txt`): 242
  files. 196 carry the MIT permission text and 34 the SPDX tag MIT; 2 are generated by Mesa's
  build from MIT scripts (`nir_opcodes.c`, `nir_opt_algebraic.c`); 1 is BSL-1.0
  (`c11/impl/threads_posix.c`); 9 carry no licence text in the file: the 7 of `util/blake3`
  (BLAKE3 1.8.2 copied verbatim, its README says; upstream's licence is CC0-1.0 OR Apache-2.0 OR
  Apache-2.0 WITH LLVM-exception), `util/softfloat.c` (Berkeley SoftFloat 3e, BSD-3-Clause text
  in the file, which the script's MIT match does not count) and `util/format/u_format_etc.c` (MIT
  text in the header it includes, `texcompress_etc_tmp.h`). Against run 6's list (243 files) the
  one change is `nir_lower_fragcolor.c`, no longer linked (the bridge writes that step itself).
  Four of the MIT files are generated and carry a second notice: `glsl_parser.cpp` and
  `glcpp-parse.c` are Bison's output (Bison's skeleton is GPL-3.0-or-later with the Bison special
  exception, which lets the output be distributed under any terms), `glsl_lexer.cpp` and
  `glcpp-lex.c` flex's (flex's own BSD-style licence). All permissive. The Windows build
  (`mesa/build-windows.ps1`, G9) links nothing into anything of ours yet; G9 lists the driver's
  linked files when there is one.
- G9 run 2, wcontainers-opengl's dependencies: `pollster` 1 (Apache-2.0 OR MIT), `wgpu` 30.0.2
  (MIT OR Apache-2.0, the workspace's patched copy), `wcontainers-gpu` and `wcontainers-gpu-webgpu`
  (ours); build-dependencies `cc` (MIT OR Apache-2.0), `serde` and `serde_json` (MIT OR
  Apache-2.0). All already in the lock file.
- G9 run 3, wcontainers-opengl: `windows-sys` 0.61.2 (MIT OR Apache-2.0, already in the lock
  file, Windows only), `wcontainers-nir` (ours, without `standalone`: its bridge, route B and
  through it naga and wcontainers-shaders, as listed above), and the build-dependency `bindgen`
  0.71.1 (BSD-3-Clause, already in the lock file for unicorn-engine-sys) in place of `cc`.
  bindgen runs libclang at build time only (on the desktop, the LLVM 21.1.8 release the Mesa
  build already fetches, Apache-2.0 WITH LLVM-exception; on Linux Nixpkgs' clang 21.1.8);
  nothing of it is linked. The driver now links Gallium's `u_init_pipe_screen_caps`
  (`u_screen.c`, MIT) from Mesa's own library.
- G9 run 4, wcontainers-opengl: `naga` (the workspace's, MIT OR Apache-2.0, already a
  dependency of wcontainers-nir and the interface) as a direct dependency (the uniform blocks'
  declared sizes). The driver now also calls Mesa's `u_upload_mgr.c`, `u_blitter.c`,
  `u_framebuffer.c` and `util/format/u_format.c` (each MIT by its header) from Mesa's own
  library, which the WGL target links anyway; no new file of Mesa's is built.
- Testing only, on the Windows desktop (`crates/wcontainers-opengl/cts`, never linked into
  anything of ours): VK-GL-CTS `opengl-cts-4.6.8.1` (Apache-2.0, its `LICENSE`) with what its
  `fetch_sources.py` pins (glslang BSD-3-Clause and others, SPIRV-Tools and SPIRV-Headers
  Apache-2.0 and MIT, Amber Apache-2.0, jsoncpp MIT, zlib Zlib, libpng libpng-2.0, Vulkan-Docs and
  the video samples Apache-2.0 and MIT), and deqp-runner v0.23.3 (MIT, as 15.2 says; its own
  crates from crates.io, built by `cargo install --locked`).
- zlib (Zlib licence), linked as Mesa's build found it (Nixpkgs' static zlib).
- `serde_json` 1 and `serde` 1 (both MIT OR Apache-2.0), build- and dev-dependencies of
  wcontainers-nir; `cc` (MIT OR Apache-2.0), already in the workspace.
- `blake3` 1 (CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception, already in the workspace), a
  dependency and build-dependency (the cache key and the converter's version).
- `wcontainers-gpu` (ours), a dependency (`SlotKind`, `ReflectedBinding`, `ShaderStage`).
- `glow` 0.18 (MIT OR Apache-2.0) and `khronos-egl` 6 with `dynamic` (MIT OR Apache-2.0),
  dev-dependencies since run 7 (llvmpipe in the corpus run); both already in the workspace through
  the WebGL back end.
- `ash` 0.38 (MIT OR Apache-2.0) and `libloading` 0.8 (ISC), dev-dependencies (lavapipe in the
  corpus run); both already in the lock file through wgpu.
- `spirv` 0.4 (Apache-2.0; the SPIR-V grammar's names, already in the workspace through naga), a
  dependency. `rand` 0.9 and `rand_chacha` 0.9 (MIT OR Apache-2.0, already in the workspace),
  dev-dependencies.
- `glam` 0.30.10 (MIT OR Apache-2.0), a dev-dependency since run 15 (new to the lock file): the
  GLSL comparison's seeded fixed-function matrices, their product and inverses.
- Corpus, test data only, fetched into the build folder and never in the repository or shipped:
  VK-GL-CTS at 5bb4c7e8 (since run 9; Apache-2.0, its `LICENSE`; only `data/gles2/shaders`,
  `data/gles3/shaders`, `data/gles31/shaders` and the licence are fetched by `corpus/fetch`;
  `corpus/cts-programs` fetches the whole tree and the dependencies its `fetch_sources.py` pins:
  glslang (BSD-3-Clause and others), SPIRV-Tools and SPIRV-Headers (Apache-2.0 and MIT), Amber
  (Apache-2.0), jsoncpp (MIT), Vulkan-Docs and the video parser (Apache-2.0 and MIT), built only to
  dump test programs, never linked into anything of ours);
  bgfx at e76ecedf (since run 8; BSD-2-Clause, its `LICENSE`; only `examples/runtime/shaders/glsl`
  and the licence are fetched);
  Godot at 73ca6be2 (since run 10; MIT, its `LICENSE.txt`; only `drivers/gles3/shaders`,
  `LICENSE.txt` and `COPYRIGHT.txt` are fetched; `COPYRIGHT.txt` names no other licence for those
  files, and `tonemap_inc.glsl` notes an ACES fit adapted from MJP's BakingLab, MIT);
  since run 11 also Godot's `servers/rendering/renderer_rd/shaders` (MIT) and the AMD FSR and FSR2
  code it includes (`thirdparty/amd-fsr`, `thirdparty/amd-fsr2`: Expat, that is MIT, by Godot's
  `COPYRIGHT.txt`; their own `license.txt` and `LICENSE.txt` fetched beside them), compiled by
  glslang (Nixpkgs' `glslang`, BSD-3-Clause and others; a tool run by `corpus/godot-forward-plus`,
  never linked);
  Sascha Willems' Vulkan samples at d2d49bf7 (MIT, `LICENSE.md`); piglit at 92510281 (since run 7;
  its `COPYING`: MIT for most of it, with some test shaders under their projects' own licences,
  e.g. GStreamer's in `glslparsertest`; piglit is for testing only, as the routed decision says,
  and never a dependency).
- Build tools only, on the Windows desktop (G9, `mesa/build-windows.ps1`, never linked into
  anything of ours; nothing installed, all under `C:\wc\graphics-nir\tools`): Python 3.13.1 from
  its NuGet package (PSF-2.0), Meson 1.12.1 (Apache-2.0), Ninja 1.13.0 (Apache-2.0), Mako 1.3.10
  (MIT), PyYAML 6.0.2 (MIT), packaging 25.0 (Apache-2.0 OR BSD-2-Clause), win_flex_bison 2.5.25
  (flex BSD-2-Clause; bison GPL-3.0-or-later, a generator whose output carries Bison's exception,
  as GCC's Bison on Linux), LLVM 21.1.8 for the Visual C++ runtime (Apache-2.0 WITH
  LLVM-exception; since G9 run 5 its release only gives bindgen's libclang and lldb, and
  llvmpipe links the same version built there from its source, the reference only; the
  release's `LLVMSupport.lib` holds rpmalloc, public domain or MIT, never linked now), Python
  3.10.11 from its NuGet package (PSF-2.0; only for lldb, which loads it, for debugging a
  case), and Visual Studio 2022 Build Tools, already on the desktop (Microsoft's licence).
- Build tools only, from the repository's locked nixpkgs: Meson 1.12.1 (Apache-2.0), Ninja (Apache-2.0),
  LLVM 21.1.8 (Apache-2.0 WITH LLVM-exception; llvmpipe and lavapipe use it, wcontainers-nir does not
  link it), GCC 16.2.0 (GPL-3.0 with the GCC runtime library exception).

## Departures from the plan

- G9 run 5: llvmpipe's LLVM on Windows is built from LLVM's source on the desktop rather than
  taken from LLVM's release (the plan names no source for it; the release's integrated allocator
  corrupted Mesa's heap, Where it stands). The Windows Mesa build keeps debug information
  (`debugoptimized`, assertions still off as in a release build), for the stacks of crashes; its
  speed is not what G9 measures.

- G9 run 1: the Windows build of Mesa is a script beside the Linux one
  (`crates/wcontainers-nir/mesa/build-windows.ps1`), sharing its pin and patches, not a step of
  the rig's setup (the rig's scripts are another track's). It fetches its own tools into
  `C:\wc\graphics-nir\tools` rather than installing them system-wide. llvmpipe is built with
  LLVM's own Windows release (static runtime), not an LLVM built here.

- G9 run 2: `wcontainers_screen_create` is in the crate's C bridge, not in a C file of Mesa's
  driver folder (15.2): the folder holds only its meson file and the public header, so every line
  of the driver's code is in this repository. The library reaches Mesa's link through one meson
  option, `wcontainers-link-args`, set after cargo builds it (the native libraries are known only
  then), not through a path Mesa's meson file computes.
- G9 run 2: each list runs with the first configuration its `mustpass.xml` gives it (for the
  KHR lists, 64 by 64 and seed 1; the dEQP lists have one). The KHR lists' other configurations
  (other sizes and seeds, FBO surfaces) are not run: 15.8's measure names the lists, and gex_wgpu's
  112,212 GLES cases are one configuration of each.
- G9 run 2: the pinned CTS is unpatched (Mesa's own CI patches its copy); a case that fails by a
  CTS fault is listed as such with its reason, not excluded.

- 6.4 states the binding rule for Direct3D. For Vulkan and OpenGL programs the NIR route reads it as
  Direct3D 12's: within the stage's set each kind's resources in the order of the program's own
  (set, binding), with the set in `ReflectedBinding::space` and the binding in `register`. A combined
  image sampler is a texture and a sampler reflected at the same set and binding.
- The NIR the routes take is fixed by one set of NIR compiler options in the bridge (Zink's list plus
  all 64-bit integer lowering and the instance index counting from the first instance), which 15.3
  does not name; G9 and G10 must hand over NIR made with the same options.
- The corpus is fetched by `corpus/fetch` into the build folder on the rented machine (each source
  pinned by commit), not kept in the Mac's scratch (15.8): the runs happen there, and the pins make
  it the same corpus everywhere.
- Mesa is not a git submodule at `third_party/mesa` (15.2). It is pinned by commit in
  `crates/wcontainers-nir/mesa/pin.toml` and fetched by the build script. Reasons: the literal check
  scans every Rust file and script of a submodule (Mesa has hundreds of Python scripts and Rust files),
  any new baseline or policy line is refused as growth, and an unchecked-out submodule is itself a
  finding; and where the fork 15.2 describes is kept is the owner's decision (15.9). Until then the
  patches live as files in `crates/wcontainers-nir/mesa/patches/` (one so far, Zink's), applied by
  the build script; they move into the fork when there is one.
- The Mesa build runs on the rented x86-64 machine, not the laptop (15.8 says laptop): the brief of
  2026-11-19 makes the rented machines the default. The choice's times (15.3: "measured on the
  laptop") are taken there too, alone on ten cores, as that machine's notes take timings.
- The choice was made on the SPIR-V corpus first (run 4) and made final over the whole corpus,
  GLSL and the other sources in, by `corpus/choose` over all 18,222 items (run 11's, read in run 13).
- Zink is built beside llvmpipe and lavapipe: its `nir_to_spirv` is candidate A's first step.
  SPIRV-Tools is off (Mesa takes it when present; nothing of ours needs it).
- Candidate B reads NIR through a flattened copy the bridge makes (`bridge/flat.c`), not through
  NIR's own structures from Rust: bindgen is out (below), and the copy keeps Mesa's numbering and
  layout on the C side, so a Mesa update changes C only.
- wcontainers-opengl does use bindgen, for Gallium's headers (G9 run 3), as 15.2 says: Gallium's
  structs are what the driver fills, and copying them by hand would define their layout twice. The
  literal check's objection (below) is met by reading `OUT_DIR` inline in `env::var_os`, which
  the check accepts, and by handing the bindings' path to the crate through its own variable.
- wcontainers-nir's bridge-only build for the driver is the absence of a default feature,
  `standalone`, not a feature of its own: Cargo unifies features, so a `driver` feature would have
  taken the SPIR-V steps and the GLSL front away from the corpus and tests whenever both crates
  are built together; an additive `standalone` cannot. Its tests and the corpus run require it.
- The bridge's functions are declared by hand in `src/mesa.rs` and `src/direct.rs`, not by bindgen: bindgen's output
  is included from Cargo's `OUT_DIR`, whose name the literal check refuses as a new code string here
  (its one constant, `CARGO_OUT_DIR`, is private to wcontainers-literals).

- Lowered IO keeps a stage value's interpolation only where a fragment input is loaded. Route B
  gives a vertex output Perspective (Flat for an integer one); where a fragment program reads it
  another way, the pipeline (G9, G10) must give the vertex side the fragment side's. The plan does
  not say; the comparison runs each stage alone, so it does not see this.
- The 16-bit test's reference is lavapipe running the same program with native 16-bit integers and
  Rust's 16-bit arithmetic, both, rather than a corpus mode (the program is the test's own).
- The route's module is run by lavapipe through naga's SPIR-V writer, not by the recording back
  end's interpreter (`wcontainers-gpu-test`) or the WebGPU back end on the RTX 3080 (15.3). Reasons:
  the interpreter's limits (no derivatives, implicit levels only on one-level views, no atomics or
  workgroup memory) would leave most real fragment programs to the GPU queue, while lavapipe runs
  every program the reference runs, on the same machine, with one harness for both sides. What this
  holds is the conversion (NIR to the common form); naga's SPIR-V writer is the same one wgpu's
  Vulkan back end uses. The WebGPU back end's own conversion is the shaders track's.
- The plan says "within the format's rounding" for floats; the targets are 32-bit, and two programs
  that differ only in the order of operations differ by more than one rounding. The tolerance is
  1024 units in the last place or 2^-20 absolute (`compare.rs`), stated and checked to still catch a
  change of constants (Where it stands).
- GLSL is compiled on Mesa's standalone scaffolding with st/mesa's steps after a link written out
  in `glsl.cpp` for one set of capabilities, not by st/mesa itself (which needs a gallium screen
  and context: G9's driver). The steps and their order are st/mesa's; the shader-variant steps
  st/mesa takes at draw time (clamped colours, flat shading, two-sided colour, user clip planes,
  point size), which depend on the draw's state, are not taken here: G9 hands its variants over
  and its own corpus run holds them.
- OpenGL's buffer accesses reach route B as arrays of words, not as typed blocks: after
  `gl_nir_lower_buffers` only byte offsets remain (the block types are gone from the
  instructions), so the bridge makes the words form, the one 15.4 names for robustBufferAccess.
  Program binding for OpenGL: uniform block index, storage block index after them.
- Vulkan capability classes: a program needing what our Vulkan 1.1 will not report is counted apart,
  not as a route failure (the pass bar's "named reason").

- llvmpipe is reached through EGL's software device (`EGL_EXT_platform_device`), not the surfaceless
  platform 15.2's options name: the rented machine has no render node, and surfaceless takes software
  only when told by an environment variable (`LIBGL_ALWAYS_SOFTWARE`), which a test process should
  not set for itself. The context is OpenGL 4.5 compatibility, llvmpipe's; the bridge reports 4.6
  (G9's), so a program needing 4.6 is refused by the reference and counted not compared.
- The GLSL comparison fills its inputs from OpenGL's reflection of the program on llvmpipe (names,
  types, offsets, strides) and the bridge's names for where each lands in the route's module; the
  SPIR-V comparison fills from the route's module alone.
- GLSL vertex and fragment programs are compared as a drawn program (the vertex program wrapped in
  its own source, a feeder or catcher written beside it), not each stage alone: names between two
  stages do not survive OpenGL's linker, and the draw is how llvmpipe runs a stage. In a vertex
  and fragment pair, what the vertex program computes is compared through what the fragment
  program makes of it; its position is compared only when it is alone (the catcher).
- The corpus's `sources` lines gained a paths column (`.` for the whole tree): bgfx is fetched
  sparsely (its shaders and licence only), as dEQP's data files will be.
- bgfx's 3 differences (infinities from seeded RGBE texels, and one unit in the last place made
  large by `fract`) are left as differences with their reasons, as piglit's 17 are.
- dEQP's cases are run on the corpus's seeded inputs, not on the values in each case's `values`
  block: the corpus holds the route to llvmpipe, not to dEQP's expected results, and seeded inputs
  reach more of each program. Where dEQP compares the outputs in the shader, the corpus writes them
  to colour targets (Done). Of a requirement's alternative extensions the first is taken (dEQP
  takes the first the context reports); a case whose first alternative Mesa's compiler refuses is
  counted as reaching no route with Mesa's log.
- ES programs are compared at full precision on both sides (`#define mediump highp` and the rest):
  llvmpipe lowers `mediump` to 16 bits, our driver does not; both are allowed by GLSL ES, and the
  reference must compute what the route is to compute.
- The 17 differences from undefined behaviour on seeded inputs are left as differences with their
  reason, not hidden: the seed gives indices out of range and negative square roots, which the
  programs' own tests never give.

- Godot's Compatibility shaders are made for desktop OpenGL (`#version 330`, what Godot gives a
  desktop driver; our OpenGL reports 4.6 compatibility), at Godot's default project settings, with
  each mode at the default specializations and with each one specialization flipped, not every
  combination (scene alone has 33 specializations: 2^33 programs, most never built by Godot).
  Combinations Godot never builds that Mesa refuses are counted as reaching no route with Mesa's
  log.
- A vertex program without a position gets (0, 0, 0, -1) in the bridge (NIR), not through the
  shaders crate's `stage::clip_unpositioned` (crate-private, and on naga); the same rule, the same
  value. It moves to the shaders crate's entry point when there is one (Requests).

- Godot's RenderingDevice variants are every variant each `initialize` call lists, at Godot's
  default desktop settings, with no material (as the Compatibility shaders are taken), compiled for
  a Vulkan 1.1 device (SPIR-V 1.3); Godot would compile only the groups a device and project
  enable. Texture streaming (a module the default build leaves out) adds nothing to the scene
  defines.
- The GLSL comment marker Godot's reader cuts is named `GLSL_COMMENT_START`, not imported from the
  literal check's `LINE_COMMENT` (Rust's, private there): the same two characters, another
  language's fact.
- Subgroup operations (`reduce` and the like) are refused by name by route B: 15.4 has the Vulkan
  driver (G10) lower them to their one-invocation meaning before the route, so the corpus, which
  hands over NIR as spirv_to_nir makes it, counts them as failures until G10's step exists.

- A storage cube is a 2D array of its faces in the common form (run 13): the plan does not say;
  WGSL has no storage cubes, and Vulkan and OpenGL address a storage cube's faces as layers.
- A reference that does not end on the seeded inputs is held on the next seeds, up to four (run
  13), rather than counted not compared: the program is the same and the inputs are as seeded as
  before; the seed used is in the results.
- Dolphin is left out of G8's corpus (run 13): no permissively licensed boot image was found, as
  the orchestrator's ruling of 2026-11-19 night requires; the choice is final without it.
- `direct::rewrite` takes only 15.4's part of a variant key (storage formats, texels through a
  buffer) and refuses the rest by name (run 14): the other rewrites read the program's stage
  values, which the driver's reflection holds (G9, G10); it gives the bindings, not a whole
  `Reflection`, so nothing reflected is left empty and passed on as if filled.
- A storage image without a format is declared `r32float`, `r32sint` or `r32uint` by its texel's
  kind until the variant names the bound view's format (run 14): the shaders crate's contract
  asks for any format of the texel's kind.

## Interface changes

None.

## Requests to other tracks

- Shaders track (G9 run 7): the pulled-input fetch (`variant/pull.rs`, `fetch`) reads each
  component from one word at `address >> 2`, so a component that does not start at a multiple of
  its size (a float at byte 17) reads the wrong bits. Direct3D's alignment rules never make one;
  OpenGL allows any stride and offset (dEQP-GLES2's `multiple_attributes.stride.*_17`). A
  component across two words needs two loads joined by shifts. Until then the OpenGL driver
  refuses such an input by name.
- Interface track (G9 run 7): the alignment a back end needs of vertex strides, attribute offsets
  and stream offsets (WebGPU: 4, and an attribute's offset to its size up to 4, within the
  stride) as a limit or a documented rule of `VertexLayoutDesc`. The OpenGL driver takes
  `wgpu::VERTEX_ALIGNMENT` for it today, which is WebGPU's rule applied to every back end.
- WebGPU back end (G9 run 6; the driver no longer depends on it since run 7, which fetches such
  streams itself): `create_vertex_layout` accepts a stream whose stride is not a
  multiple of 4 (OpenGL allows any stride: dEQP-GLES2's `vertex_arrays` cases with strides 2 and
  17), and wgpu then refuses the pipeline ("Vertex buffer 0 stride 2 does not respect
  VERTEX_ALIGNMENT"), which fails the whole submit and every case after it in the process. The
  back end should take such a stream by a counted slow path (repacked, or fetched in the program)
  or refuse the layout so that a front can. (Run 5's request about 2D views of 3D textures is
  withdrawn: it was this driver's misread view target, fixed in run 6.)

- Literals track (G9 run 3): a lawful form for a build script's output folder. The check accepts
  `env!("OUT_DIR")` but not `env::var_os("OUT_DIR")`, the only way a build script can read it,
  and its own constant for the name (`CARGO_OUT_DIR`) is private; and `include!` takes only a
  literal, so the file a build script writes there is named twice. Either accept those two
  positions or export the constant; until then the two findings stay (Literal check).
- Interface track (15.7): everything 15.7 lists; G8 needs `HelperValue::BufferRange` and the slow path
  names `RobustAccessInShader` and `AlphaToOneInShader` before its robustness and alphaToOne parts can
  be reflected.
- Shaders track (a finding): naga's SPIR-V reader fails on HLSL's row-major array of matrices once
  it is decorated (`shadowmappingcascade/scene.frag`, "invalid access As" on candidate A); not needed
  by route B.
- Shaders track (run 14): `wcontainers_shaders::rewrite` validates without
  `Capabilities::IMMEDIATES`, so a module with push constants is refused ("rewrite: invalid: ...
  IMMEDIATES is not supported"): 20 of Godot's RenderingDevice programs with arrays of textures
  (`gi.glsl` and kin). The same as the `capabilities` request below, for `rewrite`'s validation.
- Shaders track (run 14, information): `rewrite` recomputes `sampled_pairs` only when it splits
  an array; a module with none keeps the pairs given. The NIR route gives none (its driver makes
  the reflection), so a public `sampled_pairs` of a module, or `rewrite` always recomputing them,
  would let G9 take them from one place.
- Interface track: `pub const fn stage_set(stage: ShaderStage) -> u32` in `reflect.rs` beside
  `helper_binding` (vertex and compute 0, fragment 1, plan 6.4). The shaders crate writes the rule as
  a literal in `lower.rs`; the NIR route defines `wcontainers_nir::bind::stage_set` until the
  interface has it, and will import it then.
- Shaders track: `stage::clip_unpositioned` as part of that entry point, so the NIR route gives a
  vertex program without a position the same position from one place (the bridge does it in NIR
  until then).
- Shaders track: the draw's bases rewrite (`HelperValue::DrawBase`) as part of the public entry point
  below, for Vulkan programs reading `BaseVertex` or `BaseInstance`.
- Shaders track: `wcontainers_shaders::capabilities` to add `Capabilities::IMMEDIATES` for a module
  with an `AddressSpace::Immediate` global, as it adds the other extensions a front asks for only
  where the back end has them; until then the NIR route adds it itself.
- Shaders track (a finding, not a request for this route): naga's SPIR-V reader turns a sampled
  array layer into an integer with a truncating `As` (`front/spv/image.rs`), where Vulkan and
  Direct3D round to the nearest even integer; a layer of 1.6 reads layer 1 instead of 2. The NIR
  route rounds in NIR first; dxil-spirv's and dxbc-spirv's output reaches the same reader.
- Interface or shaders track (later, when the Vulkan driver reports `vkCmdDispatchBase`): a helper
  value for the base workgroup; the routes now count dispatches from workgroup 0.
- Interface track: a helper value for the view index (e.g. `HelperValue::ViewIndex`, u32, written
  by the front for each view of a `MultiviewSerial` draw), so a program reading `gl_ViewIndex`
  (Vulkan 1.1 requires `multiview`) has it in the common form; route B refuses `load_view_index`
  by name until then (66 of Godot's RenderingDevice programs).
- Interface track: a public `CUBE_FACES` (the layers of one cube) beside `ViewDimension`;
  `resource.rs` writes it as a literal, and `wcontainers_nir::CUBE_FACES` defines it until the
  interface has it, and will import it then. Since the Direct3D 11 track's 7e4d8df6 its device
  defines one too (`pub(crate)`), and the literal check, on a tree holding both, reports the NIR
  crate's as a redefinition (run 15): neither front nor this crate can import the other's, so the
  interface's is the one place for it.
- Shaders track (a finding of run 15, from G8's second check): naga's SPIR-V reader maps
  `OpFUnordNotEqual` to `BinaryOperator::NotEqual` (`front/spv/convert.rs`), and naga's SPIR-V
  writer emits `NotEqual` on floats as `OpFOrdNotEqual` (`back/spv/block.rs`), so an unordered
  not-equal (true on NaN) becomes ordered (false on NaN) on the way through naga. Route B no longer
  uses `NotEqual` for NIR's `fneu`; dxil-spirv's and dxbc-spirv's output reaches the same reader
  (HLSL's `!=` and `isnan` on floats are unordered).
- Literals track: `CARGO_OUT_DIR` public (in a public module), so a build script can name `OUT_DIR`
  through it.

## Literal check

G9 run 7: not run (the run's budget went to the desktop). The Rust added names its numbers
(`QUERIES_PER_SET`, `VERTEX_ALIGNMENT` from wgpu's own constant, sizes from `size_of` and
Gallium's format table); its new refusal texts go through `refuse` like run 6's, which the check
reported. Run 6's 38 findings are still to be worked down; to run first next run.

G9 run 6: exit 1 at 30a4c084's successor 7ead95aa (x86-64 machine, from a git bundle; marks 0;
the clone and its build deleted after; output in scratch `logs/g9r6-literals.out`). 38
findings: `CUBE_FACES` twice (Waiting on others) and 36 in wcontainers-opengl, most from runs 3
to 5: the refusal texts given to the driver's own `refuse` and returned as `Err("...".into())`
(the check's documented false catch of a crate's own message helpers; to be made messages the
check reads, or named), `"C"` and the format string of the `unbuilt!` macro, the shader entry
names `"vs"`/`"fs"` in present.rs, `[u32; 4]` and `[f32; 4]` colour arrays, and run 3's two
`OUT_DIR` ones (build.rs's `"OUT_DIR"` and gallium.rs's `"/gallium.rs"`, still reported). Fixed
this run: the draw's binding sets (from `MAX_BINDING_SETS` by `array::from_fn`) and two zero
colours (`Default::default()`), 2062f8fe. wcontainers-nir and wcontainers-opengl have no
baseline lines.

G9 run 5: run at 99e5ff55 (x86-64 machine, from a git bundle; marks 0; the clone and
its build deleted after). Findings shown: `CUBE_FACES` twice (Waiting on others) and
`COMPONENTS` in `wcontainers-nir/src/mesa.rs` ("a second name for MAX"), now gone: the swizzle's
length is `naga::VectorSize::MAX` itself (d287ead9). Only the summary's last lines were kept, so
whether the two `OUT_DIR` findings of run 3 are still reported was not seen; the next run runs
it again whole. wcontainers-nir and wcontainers-opengl have no baseline lines.

G9 run 3: exit 1 at d6bd4c8a (x86-64 machine, from a git bundle; marks 0; the clone and its
build deleted after). The first try found 11; fixed this run: the cargo words and the Mesa
variable both build scripts defined (`MESA_VARIABLE`, `CHECK_CFG`, `CFG`,
`RERUN_IF_ENV_CHANGED`, `RUSTC_ENV`, `ASSIGN`) now live once in `mesa/meson_build.rs`, which both
include, with the defines bindgen takes (`mesa_defines`); the driver's `'\0'` (run 2's) is gone.
Left, 4 findings: `CUBE_FACES` twice (Waiting on others), and two of the driver's that no legal
form avoids today: `env::var_os("OUT_DIR")` in its build script (the check accepts `OUT_DIR` only
inside `env!`, which a build script cannot use for it, and its own name for it, `CARGO_OUT_DIR`,
is private) and `include!(concat!(env!("OUT_DIR"), "/gallium.rs"))` (a macro that takes only a
literal; handing the path through a variable of the build script's instead was found as a
restated name the same way). Asked of the literals track under Requests. wcontainers-nir and
wcontainers-opengl have no baseline lines.

G9 run 2: not run (the run's budget went to the desktop); the Rust added has no bare literal
(names, paths and flags are named constants; the build scripts share `mesa/meson_build.rs`
rather than defining its constants twice). To run first in the next run.

G9 run 1: exit 1 at 248050c5 (x86-64 machine, from a git bundle; marks 0), with the one finding
outside this track's reach, `CUBE_FACES` (Waiting on others). wcontainers-nir still has no
baseline lines; this run added no literal (the PowerShell script names its versions and paths once
at its top; the check does not scan PowerShell).

Run 15: exit 1 at 52c083b9 (x86-64 machine, from a git bundle; marks 0), for one finding outside
this track's reach: `CUBE_FACES` in `wcontainers-nir/src/lib.rs` (since run 9) is reported as a
redefinition of the Direct3D 11 device's, added on that branch at 7e4d8df6 and merged here this
run (Requests: the interface's one constant). Every other finding is the shaders crate's
submodules, not checked out in the bundle's clone (as run 6 found); this run added no literal.

Run 13: exit 0 at the run's last code commit, 86cefeb2 (x86-64 machine, from a git bundle;
marks 0). wcontainers-nir still has no baseline lines.
Run 12: exit 0 at the run's last code commit (x86-64 machine, from a git bundle; marks 0).
wcontainers-nir still has no baseline lines.
Run 11: exit 0 at the run's last code commit (x86-64 machine, from a git bundle; marks 0). A first
try found the Godot reader's `LINE_COMMENT` ("//") named like the check's own (Rust's comment, private
there): renamed for what it is, GLSL's (Departures). wcontainers-nir still has no baseline lines.
Run 10: exit 0 at the run's last code commit (x86-64 machine, from a git bundle; marks 0). A
first try found the new reader's comment marker defined beside dEQP's (`COMMENT`, '#'): Godot's
reader now imports dEQP's. wcontainers-nir still has no baseline lines.
Run 9: exit 0 with the run's code (x86-64 machine, from a git bundle; marks 0). A first try
found `"in"`, `"out"` and a decimal point in `tests/corpus/deqp.rs` restating constants of the
WebGL back end and the check (private there): GLSL's qualifiers are now written through one
function of format strings, and the dEQP grammar's one word before a stage list is taken as any
word. wcontainers-nir still has no baseline lines. Run 8: exit 0 at the run's last code commit, 9e0a7c6e's parent (x86-64 machine, from a git bundle; marks 0; a first
try found the GLSL version directive named twice, in `glsl.rs` and the new `bgfx.rs`: now imported);
wcontainers-nir still has no baseline lines. Run 7: exit 0 at the run's last code commit (x86-64 machine, from a git bundle of the branch; marks
0); wcontainers-nir still has no baseline lines. Run 6: exit 0 at 4294fcf0 (x86-64 machine, from a git bundle; marks 0; the 18 changes that could
add lines are the shaders branch's, as before). The bridge's numbers are `#define`s, named once;
the Rust added has no new bare literals (test data apart). Adopted with the branch (merged from the
shaders branch, which carries it). wcontainers-nir has no
baseline lines: the check passes with none for it (run on the Mac, runs 1 to 4, exit 0; in run 4 the
two new scripts' numbers were taken out (the timing cores are passed in, the load read by `read`), and
one baseline line the Direct3D 11 merge left stale was deleted, as the check asked;
it needs the git history, which the rented machines' copies lack). The list of
changes that could add lines is the 18 the shaders branch brought; this branch added none. C files are
not scanned; the bridge names its one version by `#define`s all the same.
