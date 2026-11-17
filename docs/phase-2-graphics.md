# wcontainers, phase two: graphics

Status: plan written 2026-11-17 from the owner's brief of 2026-11-16 and the verified research in
`pyro-parked/wcontainers-scratch/related/`, and corrected the same day after each of G0's independent
checks. The interface crate `wcontainers-gpu` exists; nothing else
is built.

Words used here:
- **guest**: the program in the container (a Linux program, or a Windows program running on Wine).
- **front**: our implementation of an API a guest calls: Direct3D 11 and 12 now; OpenGL, Vulkan and
  Direct3D 9 later. A front speaks the API's rules and answers the guest.
- **back end**: what draws through a GPU API the host offers: WebGPU and WebGL 2 now; perhaps a native
  one later.
- **the interface**: the crate `wcontainers-gpu`, where fronts and back ends meet. They never meet
  each other.
- **base**: what every back end provides. **Fast path**: an operation with a default made from the base,
  which a back end overrides where it has something better. **Present or absent**: an operation with no
  cheap default; a back end either has it or reports it absent.
- **block**: an immutable piece of drawing state (blend, depth and stencil, rasterising, vertex layout,
  program, bindings), made once, named by draws.
- **common form**: the one shader form every front hands to the interface and every back end writes
  its own language from.
- **device thread**: the one thread that owns a back end.

Every figure below says whether it was measured or is only expected.

## 1. What this phase delivers

Programs in a container draw through a browser GPU API at the best speed the browser allows. Target:
indie and older big games first.

Done means:
- Real Windows programs draw through our Direct3D 11 and Direct3D 12 fronts on the WebGPU back end,
  first as ordinary Windows libraries on the development desktop (section 10), then as modules in the
  container in Chrome. Pictures match real Direct3D on the same GPU within stated tolerances (section 11).
- The same fronts on the WebGL 2 back end, for browsers without WebGPU, at the lower level of section 9.
- Wine's Direct3D test suites give the same verdicts through our fronts as through real Direct3D, minus a
  written gap list where each gap is a capability the back end reports absent.
- Every slow path is named, counted and warned; every absent feature is reported through the API's own
  capability mechanism.

Out of the first batch, but the interface must already fit each (section 5 walks through them):
OpenGL 3.3 and 4.6 through Mesa with a driver on our interface; Vulkan 1.1 through Mesa's Vulkan
runtime (WebGPU only); Direct3D 9 (with 32-bit x86); our own OpenGL in Rust beside Mesa's; a native
back end; the guest stand-in libraries and the call channel; sound and input.

## 2. Rules already decided

1. Graphics calls leave the guest at the API the program calls. A thin stand-in library in the guest
   writes each call into a buffer; the buffer crosses to the host once a frame. Everything below runs as
   code compiled straight to WebAssembly (or natively for tests), never as translated guest code. Guest
   memory is the host's own linear memory, so vertex and texture data are read in place. One exception,
   a departure from the brief (4.6): bytes the API lets the program reuse as soon as the call returns
   (`UpdateSubresource`, initial data, Direct3D 9's `*UP` draws, OpenGL's `glBufferSubData` and client
   arrays) are copied once by the stand-in at the call, because the device thread reads them only when
   the frame's call buffer arrives, after the program may have changed them.
2. Each piece is its own module, loaded when a program first asks for that API, behind one pluggable
   graphics hook on the host. A host without a screen does not provide the hook. The Direct3D family
   (DXGI, Direct3D 10, 11 and 12) is one module per back end, loaded when a program first asks for any of
   them; this is a departure from the brief, whose reasons are in section 10.
3. Fronts and back ends meet only at our interface. It is our own design: not WebGPU's shape, not Mesa's
   Gallium. It is never reduced to what both back ends share.
4. Resolved at compile time: generics, no run-time dispatch, one ordinary call per operation.
5. Honest reporting: what a back end cannot do is reported absent through the API's own capability
   mechanism, or done by a named, counted, warned slow path. Nothing pretends.
6. We write no shader compiler front end. DXBC goes through the MIT `dxbc-spirv`; DXIL through an
   existing open-source compiler (section 6).
7. One thread owns the GPU device; the guest waits on it when it needs an answer.
8. Proven against the real thing: real Direct3D on the same GPU, and Microsoft's software rasteriser
   (WARP) as a deterministic second reference.
9. Rust. Established crates where the job is the same input for the same output; our own code where a
   purpose-built fit is what makes it fast.

## 3. Crates and which way they depend

- `wcontainers-gpu`: the interface (section 4). Depends on `naga` (the common form) and `bitflags` only.
- `wcontainers-gpu-webgpu`: the WebGPU back end, written on the `wgpu` crate, which runs natively (Metal,
  Direct3D 12, Vulkan) for tests and on the browser's WebGPU when built for WebAssembly. Depends on the
  interface only (plus `wgpu`).
- `wcontainers-gpu-webgl`: the WebGL 2 back end, written on the `glow` crate. Depends on the interface
  only (plus `glow`).
- `wcontainers-shaders`: turns DXBC and DXIL (later SPIR-V) into the common form, lowers geometry, hull
  and domain stages and stream output into what back ends run (section 7), and holds the cache (section
  8). Depends on the interface only (plus `dxbc-spirv` and `dxil-spirv`, built from C++).
- `wcontainers-directx-graphics-infrastructure`: what Direct3D 10 to 12 share: adapter and output
  discovery, swap chains, presenting, frame pacing. Depends on the interface.
- `wcontainers-direct3d-11`: our Direct3D 11 front, also serving Direct3D 10. Depends on the interface,
  the shaders crate and the infrastructure crate.
- `wcontainers-direct3d-12`: our Direct3D 12 front. Same dependencies.
- `wcontainers-gpu-test`: the recording back end, the back-end test suite every back end runs, and the
  picture comparison glue (the comparison tools themselves come from the graphics-rig track).

Directions: back ends depend on the interface and nothing else of ours. Fronts depend on the interface,
the shaders crate and the infrastructure crate, never on a back end. The shaders crate depends on the
interface only. The back end is chosen where a front is built into a library or module (section 10),
as a type parameter. This is how "fronts and back ends depend only on the interface crate" is read: they
never depend on each other; fronts also use the two shared helper crates, which themselves sit on the
interface.

Licences, each confirmed in its repository on 2026-11-17: `naga` 30.0.2 and its dependency tree
MIT or Apache-2.0 (one Zlib, one BSD-2, `unicode-ident` with Unicode-3.0; `cargo tree` listed);
`dxbc-spirv` MIT (LICENSE, "Copyright (c) 2025 Philip Rebohle"), with SPIRV-Headers (Khronos, MIT-style
licence) as its one submodule; `dxil-spirv` MIT (LICENSE.MIT, SPDX MIT, Hans-Kristian Arntzen for Valve).
The parts of dxil-spirv that ship with its converter library, each read in the source on 2026-11-17:
glslang's SPIR-V builder (`third_party/glslang-spirv`, BSD-3-Clause, LunarG and Google), RenderDoc's LLVM
bitcode reader (`bc` and `third_party/bc-decoder`, MIT, Baldur Karlsson) and SPIRV-Headers. Its own
`dxbc-spirv` submodule is used only by a sandbox tool and is not built. Nothing in either is copyleft.
Fossilize MIT (evaluated in section 8, not used). `naga`'s WGSL reader (feature `wgsl-in`, for the
built-in programs of 4.3) adds only MIT or Apache-2.0 crates. `bcdec_rs` 0.2.1, the BC decoder G2 and
G3 add for `BlockDecoded`: MIT (its `Cargo.toml`, and the LICENSE of its repository
ScanMountGoat/image_dds, "Copyright (c) 2023 SMG"), a port of iOrange's bcdec (MIT or Unlicense, its
LICENSE), `no_std`, read on 2026-11-17. `wgpu` and `glow` are confirmed by the milestones that add
them.

## 4. The interface

The real Rust is `crates/wcontainers-gpu`. This section gives its shape and the reasons; the crate's doc
comments give each item's purpose.

### 4.1 Three kinds of operation

`trait Backend` holds everything. Each method is one of three kinds:

- **Base**: no default; every back end implements it.
- **Fast path**: a default built from base calls, in the trait. A back end overrides it only where it has
  something better. The counted slow path, if the default is slower than what an API expects, is bumped
  inside the default, so a back end that overrides it never counts it.
- **Present or absent**: the default returns `GpuError::Absent(feature)`. A back end that has the feature
  overrides the method and sets the matching `Features` bit. Fronts read the bit before choosing a route.

A front is written as `struct Device<B: Backend>`; every call it makes is a direct call on `B`, inlined
or not as the compiler chooses. No trait objects, no function tables.

### 4.2 The base

Objects, each made from plain data and named by an associated type of the back end:

- `create_buffer(&BufferDesc)`: size, usage, and `MapIntent` (rarely written, or rewritten often).
- `create_texture(&TextureDesc)`, `create_view(&texture, &TextureViewDesc)`, `create_sampler(&SamplerDesc)`.
  A texture is 2D or 3D, with layers, and says whether its layers are cube faces (`cube`), so a back end
  that must fix a texture's kind when it makes it (WebGL 2: `TEXTURE_2D`, `TEXTURE_2D_ARRAY`,
  `TEXTURE_CUBE_MAP`, `TEXTURE_3D`) knows it then; `TextureDesc::own_view` is that kind. There is no 1D
  texture: an API's 1D textures are 2D textures one texel high, read as such by converted programs, which
  every back end serves with mipmaps, arrays and rendering (WebGPU's 1D textures allow none of these).
  Samplers have no level-of-detail bias, because neither browser API has one (4.12).
- `create_shader(&ShaderModule)`: a validated `naga::Module` with its entry point, stage and content key.
- `create_program(&Program)`: vertex plus optional fragment shader, or one compute shader.
- The state blocks: `create_blend`, `create_depth_stencil`, `create_raster`, `create_vertex_layout`.
- `create_binding_set(&[BindingEntry])`: the resources a draw binds, as one block (4.5).
- `create_query_set(kind, count)`: occlusion queries that say "any" (every back end), exact occlusion
  counts (`OCCLUSION_COUNT`), or timestamps (`TIMESTAMPS`).
- `destroy(Object)`: one call for every kind of object. Work already recorded still sees the object; the
  back end frees it when that work is done, so no front ever waits to destroy. Handles are cheap to
  clone; a clone names the same object (a `wgpu` handle, a GL name).

Bytes in:
- `write_buffer(&buffer, offset, &[u8], WriteMode)` and `write_texture(&region, extent, &[u8], row_pitch,
  rows_per_layer, WriteMode)`, at any byte offset and length. The slice is read during the call and not
  after; the back end hands it to its API without an intermediate copy where its API allows (WebGPU
  `writeBuffer` and `writeTexture`, WebGL `bufferSubData` and `texSubImage`, both take a view of
  WebAssembly memory). Which memory the slice may point into, and how long the guest must leave it alone,
  is the rule of 4.6.

Recording:
- `begin_pass(&PassDesc)` and `end_pass()`. A pass names up to eight color targets and one depth-stencil
  target, each with what happens at the start (`Load`, `Clear(value)`, `DontCare`) and the end (`Store`,
  `Discard`), resolve targets, and the occlusion query set.
- `draw(&DrawCall)`: inside a pass. Section 4.4.
- `begin_query(index)`, `end_query()`: occlusion, inside a pass.
- `copy_buffer`, `copy_texture`, `copy_buffer_to_texture`, `copy_texture_to_buffer`, `clear_buffer`
  (a pattern of 1 to 16 bytes repeated: zero, or an element's bits for Direct3D's storage view clears),
  `resolve`: outside a pass. Buffer ranges are byte-granular in every call; a back end whose API copies
  in larger granules (`Limits::buffer_copy_alignment`, 4 on WebGPU for `writeBuffer`,
  `copyBufferToBuffer` and `clearBuffer`) does the aligned middle natively and merges the edge words with
  a small program (`UnalignedBufferEdge`).
- `submit() -> Fence`, `fence_done(&fence)`, `wait_fence(&fence)`.

Bytes out:
- `read_back(&ReadSource) -> Readback` (a buffer range, or a texture region whose rows arrive tightly
  packed), `read_queries(&set, first, count) -> Readback`, `read_state(&readback)`, `wait_read(&readback)`,
  `finish_read(readback, &mut [u8])` which copies the answer into guest memory.

Frames:
- `create_surface(window)`, `configure_surface(&surface, format, width, height, vsync)`,
  `current_texture(&surface) -> TextureView`, `present(&surface)`.

What the back end is:
- `caps() -> &Caps`, `slow_paths() -> &mut SlowPaths`, `status() -> DeviceStatus`, and
  `helpers() -> &mut Helpers`, where the fast-path defaults keep what they made (built-in programs and
  blocks, the ring). A back end holds a `Helpers::default()` and never looks inside.

### 4.3 Fast paths and present-or-absent operations

Fast paths, all in the crate with their defaults:
- `read_back_now(&src, &mut dst)`. Default: `read_back`, then `wait_read` if not ready (counted
  `ReadBackWait`), then `finish_read`. WebGL 2 overrides it with its blocking `readPixels` and
  `getBufferSubData`, which answer inside the call.
- `draw_fan_or_loop(&call, kind, IndexSource, provoking, first_instance, instances)`. Default: build a
  triangle list (fan) or a line list (loop) in a fresh index buffer, honouring primitive restart in guest
  indices, draw it with restart off, destroy the buffer (counted `FanConverted` or `LoopConverted`).
  Each fan triangle keeps the fan's winding and is ordered so the vertex the API's convention names comes
  first for `First` (vertex i + 1, OpenGL's first-vertex rule) and last for `Last` (vertex i + 2); a loop's
  segments are (v_i, v_i+1) and the closing (v_n, v_0), which gives both conventions. Direct3D 9's fan
  rule is checked against the reference at its milestone. WebGL 2 overrides it with `TRIANGLE_FAN` and
  `LINE_LOOP`. Index values come from guest memory or are sequential; a front whose index data lives only
  on the GPU keeps a copy in host memory (the OpenGL front does, as Mesa's index rewriting does).
- `prepare(&PrepareDesc)`: a combination of blocks, target formats and topology to build ahead of its
  first draw. Default: nothing (the first draw builds it). WebGPU overrides it with
  `createRenderPipelineAsync`.
- `clear_texture(&texture, &TextureDesc, format, mips, layers, ClearValue)`: levels and layers (depth
  slices of a 3D texture) seen in the texture's format or its sRGB twin. Default: for each level and
  layer or slice, a view to draw into of that level and layer (which every back end makes, 4.2) and a
  pass whose load is `Clear`, ended at once, naming only the aspects the format has. WebGPU keeps the
  default; WebGL 2 overrides it with `clearBuffer*` on each attachment, which skips the pass
  bookkeeping.
- `clear_rects(&view, format, ClearValue, &[ScissorRect])`: Direct3D 11.1 `ClearView` and Direct3D 12
  clears with rectangles, outside a pass. Default: one pass, a full-screen triangle per rectangle
  scissored to it, with a built-in program writing the value (float, unsigned or signed output by format;
  depth through the fragment depth; stencil by `Replace` with the value as reference). WebGL 2 overrides
  it with scissored `clearBuffer*`.
- `generate_mips(&texture, &TextureDesc, mips, layers)`. Default: for each level and layer (each slice of
  a 3D level), a pass drawing a full-screen triangle that samples the level above through a linear
  sampler, in the texture's own format (so sRGB is decoded and encoded). It reads the level above
  through a view of all layers in the texture's own dimension (2D array, cube, cube array), the layer
  or cube face chosen in the program, and draws into a view of one level and one layer, so it needs no
  `TEXTURE_VIEWS`. WebGL 2 overrides it with `generateMipmap` where the format allows and keeps the
  default otherwise.
- `write_buffer_ring(&[&[u8]], align, &mut offsets) -> Buffer`: a place for the data one draw needs
  (Direct3D 9 user pointers, OpenGL client arrays, spilled immediates), written in one call and valid for
  commands recorded before the next call. Default: a ring buffer (4 MiB, grown to fit) written with
  `NoOverwrite`, and with `Discard` when it wraps, so nothing waits. No back end overrides it today; a
  native back end would map the ring.

The built-in programs are WGSL read once through naga's WGSL reader (`wcontainers-gpu/src/helpers.rs`),
so every back end gets them in the common form like any other shader.

Present or absent, with no cheap default:
- `dispatch(&DispatchCall)`: compute (`COMPUTE`). WebGPU has it; WebGL 2 does not.
- `capture_vertices(&call, targets)`: transform feedback (`TRANSFORM_FEEDBACK`). WebGL 2 has it; WebGPU
  does not, and the shaders crate's stage helper builds stream output from compute instead (section 7).
- `create_resource_table`, `write_resource_table`: big resource tables (`RESOURCE_TABLES`). Absent in both
  browsers today (Chrome has a sampling table only behind its unsafe flag).
- `write_timestamp`: GPU timestamps (`TIMESTAMPS`), outside a pass. WebGPU writes timestamps only where
  passes begin and end, so its back end attaches the write to the next pass (or an empty one); a front
  asked for a timestamp between draws ends its pass first (`TimestampSplitPass`).

Why fans have a fast path but geometry shaders do not: fans are one draw turned into another draw, so a
default made from the base exists and is cheap. Geometry, hull and domain stages need converted shaders
and several dispatches; that conversion is shader work, so it lives in the shaders crate and reaches
back ends only as ordinary `dispatch` and `draw` calls. A native back end with real geometry shaders
would add a fast path then, not before.

### 4.4 Drawing state as immutable blocks

A draw names (`DrawCall`): a program, a blend block, a depth-stencil block, a raster block, a vertex
layout block, up to four binding sets, the vertex streams and index buffer (buffer plus offset, plain data),
the topology, whether strips restart, viewports and scissors, blend constant, stencil reference, small
immediate constants, and the range (direct, indexed, or indirect from a buffer).

The blocks:
- **Blend** (`BlendDesc`): per target blend and write mask, whether all targets share target 0,
  alpha to coverage, and a logic operation (present only with `LOGIC_OP`). A back end without
  `INDEPENDENT_BLEND` gets only blocks whose targets share target 0: a front whose targets differ in
  blend or write mask (Direct3D 10 and 11 allow it at every level) draws once per group of targets that
  agree, through the `TargetsSplit` route of 4.12.
- **Depth and stencil** (`DepthStencilDesc`): compare, write, both stencil faces, masks, depth bounds.
  The stencil reference is not in the block, because every API changes it per draw.
- **Raster** (`RasterDesc`): fill mode, cull, front face, depth bias, depth clip or clamp, scissor enable,
  line width, provoking vertex, clip depth convention, sample mask (a partial mask only with
  `SAMPLE_MASK`; otherwise 4.12's route).
- **Vertex layout** (`VertexLayoutDesc`): streams with stride and step (per vertex, or per N instances)
  and attributes. Direct3D 11 keeps strides out of its input layout, so its front makes the block from the
  layout and the bound strides together; the strides of a running game take few values, so few blocks.
- **Program**: the shaders.
- **Bindings**: see 4.5.

Target formats are not a block a draw names, unlike the brief's list: the pass already says what its
targets are, and the back end reads them from there, so a seventh handle per draw would only repeat the
pass. `prepare` names them (`TargetLayout`), because it runs before any pass.

How each back end consumes them:
- WebGPU: blocks are plain data held in a table. The back end compares the draw's block handles with the
  previous draw's; when any differ, it looks up the pipeline for (program, blend, depth-stencil, raster,
  vertex layout, pass target formats, topology class, the layout key of each bound binding set, 4.5) in a
  hash map and calls `setPipeline` only when the pipeline changed. A missing pipeline is built at once (counted `PipelineMiss`): the frame pauses, nothing
  is skipped. Viewport, scissor, blend constant and stencil reference are dynamic in WebGPU, so they never
  add pipelines.
- WebGL 2: each block handle carries the GL state it stands for. The back end compares handles with the
  last applied ones and, for a changed block, sets only the GL values that differ from the current GL
  state. Programs are linked once per (vertex, fragment) pair. No pipeline exists.

Who makes blocks: Direct3D 11 already has them (`CreateBlendState` and the others map one to one).
Direct3D 12 and Vulkan pipelines are a full set of blocks plus `prepare`, so they never stutter. OpenGL
and Direct3D 9 change state call by call; their fronts keep a dirty flag per block and look the block up
by value in a hash map at the next draw, which makes a handful of distinct blocks per frame in practice
(expected; measured at the OpenGL milestone).

Building ahead, for the implicit fronts (the brief's three steps against stutter):
1. Leave adjustable what WebGPU allows: viewports, scissors, blend constant and stencil reference are
   dynamic and never in a pipeline key.
2. When a shader arrives, build it in the background with the most common states. The front pairs the
   new shader with the most recently created shader of the other stage whose signature matches it
   (vertex outputs covering the pixel shader's inputs, by `Reflection`; engines create a material's
   shaders together), and calls `prepare` for that pair with every state tuple (blend, depth-stencil,
   raster, vertex layout, target layout, topology class, binding layout keys) the vertex shader of the
   pair has already been drawn with in this run or in the cache's record. A vertex shader not yet drawn
   takes the one tuple drawn most often so far whose vertex layout fits its inputs; at the very start of
   a first run there is none, and nothing is built ahead.
3. Record every combination drawn into the cache (section 8) and `prepare` them all at the next start.
These `prepare` calls queue behind Direct3D 12's and the cache's, and never block. A shared helper in
`wcontainers-gpu` (the combination tracker, written at G1 with the pass tracker) keeps the tuples per
vertex shader and the counts, and picks the tuples; it counts how many prepared combinations were
later drawn and how many never were, which G4 reports on real programs. A combination still missing at
its first draw is built then and the frame pauses (`PipelineMiss`), never skipping the draw.

### 4.5 Bindings

A binding set is a block: a list of `BindingEntry` (binding number, stages that read it, and a resource:
uniform buffer range, storage buffer range, texture view, storage texture view, sampler, typed texel
buffer, or a resource table). `create_binding_set` may return a set equal to one the back end already
holds, so fronts call it when their slots changed and otherwise pass the handle they have.

Binding numbers come from the shaders crate's rule (section 6.4), the same for every front: one number
space per set, binding = range start + the API's slot number, with uniform buffers from 0 (16 slots),
samplers from 16 (16), sampled textures and typed buffers from 32 (128), storage buffers and textures
from 160 (64) (`SlotKind` in the crate). Which set a slot lands in is the API's rule of 6.4; set 3
always holds the program's helper constants (`HELPER_SET`). A draw's sets are WebGPU's bind groups 0
to 3.

How layouts are made. A WebGPU pipeline needs explicit bind group layouts; WebGL 2 needs none. Most of a
layout comes from the program: the WebGPU back end reads, from the naga module of each stage, every
global the entry point uses (naga's per-entry-point usage), and takes from it the binding type (uniform,
read-only or read-write storage buffer, texture, storage texture, sampler), the texture's dimension,
whether it is multisampled, its sample kind (float, signed, unsigned, depth), a storage texture's format
and access, and whether a sampler compares. Two things a program does not say, and the bound objects do:
- whether a float texture can be filtered on this back end (`FormatCaps` `FILTER`: `r32float` only with
  `float32-filterable`, depth formats never), which picks `"float"` or `"unfilterable-float"`;
- whether each sampler the program pairs with such a texture filters, which picks `"filtering"` or
  `"non-filtering"`.
So each binding set has a layout key (`BindingLayoutKey`, from `binding_layout_key`): one bit per
texture binding whose view cannot be filtered, and one bit per sampler binding that filters. `prepare`
takes the layout keys too (`PrepareDesc::layouts`; the zero key, filterable textures and filtering
samplers, is what a Direct3D 12 pipeline state is prepared with before any descriptor is known), and the
cache records them, so a combination first seen with other keys is prepared at the next start. The pipeline key includes the layout keys of the
sets a draw binds, so the same program with a filterable and an unfilterable view is two pipelines. A
pair that is both (a texture this back end cannot filter, sampled with a filtering sampler: Direct3D 11
allows linear filtering of `R32_FLOAT` and of depth read as `R24_UNORM_X8` or `R32_FLOAT`, which deferred
lighting, SSAO and soft particles do) cannot be bound on WebGPU at all. The front sees this from
`FormatCaps` (`FILTER` missing for the view's format) and its own sampler, and binds the variant of the
program that filters that texture itself from `textureLoad` reads with the sampler's state
(`FilterInShader`, bit in `VariantKey::filter_in_shader`), reading the sampler's filter, address modes
and level range from the helper constants (`HelperValue::SamplerState`), so sampler changes build
nothing; the sampler slot is bound with a non-filtering sampler of the same addressing, for any other
texture the program samples through it. Anisotropic filtering, whose footprint Direct3D leaves to the
implementation, is done in the program along the major axis with up to the sampler's anisotropy in
taps.

Variants are chosen by fronts and made by the shaders crate. Every rewrite that depends on bound objects
or on the back end's abilities (filtering in the program, border colours, mirror once, level-of-detail
bias, storage formats from the bound view, constant buffers past the limits, vertex streams fetched in
the program, how typed buffers are read (`TexelBufferRoute`: native, storage on WebGPU, a texture on
WebGL 2), storage slots packed into one buffer, storage textures used with atomics read through a
buffer, textures and targets stored as integers (`decoded`, `targets_in_shader`), blending or logic
operations done in the program, the sample mask written by the program, clip depth, flipped depth
ranges, depth bias, the counting variant) is a field of `VariantKey` in `wcontainers-gpu`; the front
computes the key from `Caps` and what it binds, asks the shaders crate for that variant (cached by
key, section 8), and counts the slow path. `convert` (6.4) reads nothing but the source, the key and
the root signature (whose hash is in the key), never `Caps`: so a key names one module on every back
end, which the cache's merge relies on. Back ends never rewrite programs: they write their language
from what they are given (6.5).

- WebGPU: a set becomes a bind group, cached by content and by the generation of each buffer it names
  (renaming a buffer, 4.6, bumps its generation).
- WebGL 2: a set becomes a list of unit and uniform-block bindings. GLSL ES has combined samplers only,
  so the WebGL 2 shader writer gives each (texture, sampler) pair a unit; the back end binds the texture
  and the sampler object to that unit. More pairs than units is the slow path `SamplerPairsExpanded` where
  the shader can be split, and otherwise the shader fails to build (4.12).
- Storage buffer bindings at any offset (Direct3D's raw and structured views from any first element):
  where the offset is not a multiple of `min_storage_offset_alignment` (256 on WebGPU), the front binds
  from the aligned offset below and writes the rest into the helper constants
  (`HelperValue::StorageOffset`), which every converted program adds to its storage addresses (one add
  per access; there is no second program).
- Direct3D 12 descriptor tables live in heaps the program writes with the CPU at any time. Without
  `RESOURCE_TABLES` the Direct3D 12 front reads the table's descriptors when a draw is recorded and makes a
  binding set from them; with it, the front binds the heap as a table.

### 4.6 Memory: bytes in, mapping, bytes out

Nothing is mapped across the boundary. Every API's mapping rule becomes a write or a read back:

- **WriteMode::Ordered**: the write takes effect between the commands before and after it. The general
  case: Direct3D 11 `UpdateSubresource`, Direct3D 9 `Lock` without flags, OpenGL `glBufferSubData`.
  WebGPU: outside a pass, a `writeBuffer` into a staging buffer and a copy in command order; inside a
  pass, the back end ends the pass, copies, and resumes with `Load` (counted `WriteSplitPass`), unless no
  earlier command in the frame uses the buffer, when it writes directly. WebGL 2: `bufferSubData` is
  ordered by GL itself.
- **WriteMode::Discard**: the whole old contents go; earlier commands still see them. Direct3D 11
  `Map(WRITE_DISCARD)`, Direct3D 9 `D3DLOCK_DISCARD`, OpenGL `GL_MAP_INVALIDATE_BUFFER_BIT` and
  `glBufferData` with the same size. WebGPU: the back end renames the buffer to a spare from a pool (the
  same idea DXVK uses) and writes the spare; no pass is split. WebGL 2: `bufferData` with the new bytes
  (orphaning), which drivers rename.
- **WriteMode::NoOverwrite**: the caller promises no unfinished command reads the bytes. Direct3D 11
  `Map(WRITE_NO_OVERWRITE)`, OpenGL `GL_MAP_UNSYNCHRONIZED_BIT`. Both back ends write at once.

Which memory a write may read, and how long. Calls run on the device thread after the guest's stand-in
has returned (the call buffer crosses once a frame, 4.8), so a slice given to `write_buffer` or
`write_texture` must point into a block the guest leaves alone until the device thread has run the call.
The rule, the same for the container and for the Windows-library build:
- Bytes the API lets the program reuse as soon as the call returns (Direct3D `UpdateSubresource` and
  `UpdateSubresource1`, the initial data of `CreateBuffer` and `CreateTexture*`, Direct3D 9 `*UP` draws,
  OpenGL `glBufferSubData`, `glTexSubImage*` and client arrays) are copied once by the stand-in, at the
  call, into the call buffer (small) or into a staging block it allocates from a ring in shared memory
  (large). The device thread reads that copy in place and reports how far it has consumed the ring, so
  the stand-in reuses staging only after that. DXVK makes the same one copy for `UpdateSubresource`.
- Mapped memory is a block the front owns in the guest's address space (below), handed out by the
  stand-in and released only after the device thread consumed the write that reads it.

How a mapped pointer works in each front. A dynamic resource has a shadow block in guest memory with its
latest contents. `Map(WRITE_DISCARD)` hands out a fresh shadow from a small per-resource chain (the old
one stays until the device thread has consumed its write); `Map(WRITE_NO_OVERWRITE)` hands out the current
shadow; `Unmap` records `write_buffer` (or `write_texture`) of the bytes written, in the mode the flags
imply. Which bytes were written comes from the host's first-write tracking of pages (below); until a host
has it, the whole mapped range is written (`MappedUploadWhole`), which is exact because the unwritten
bytes equal what the GPU holds. Readable maps call `read_back_now` into the block before returning the
pointer.

Persistent mapping (Direct3D 12 upload and readback heaps; later Vulkan's host-visible coherent memory,
which Vulkan requires): `PERSISTENT_MAPPING` is absent in both browser back ends (neither API lets a
buffer stay mapped while the GPU uses it). The fronts keep the mapped memory in guest memory and, when the
program submits work (`ExecuteCommandLists`, `vkQueueSubmit`), write the ranges the guest changed. Which
bytes changed comes from first-write tracking of pages: the kernel of phase one already starts shared
pages read-only in its permission table so the first write to each is noticed (docs/phase-1-kernel.md,
shared blocks and private file mappings); the graphics hook will ask the host "which pages of this range
were written since I last asked", using the same mechanism on the mapped blocks. Until that hook answer
exists for a host, the whole mapped range is written (counted `MappedUploadWhole`).
Readback heaps: when a fence the program waits on completes, the front finishes the read backs recorded
before it into guest memory first, so the program sees the bytes when it sees the fence.

Read back later and now:
- Later is base: `read_back`, then poll with `read_state` or wait with `wait_read`. Direct3D 11
  `Map(READ, DO_NOT_WAIT)` polls and answers `DXGI_ERROR_WAS_STILL_DRAWING`; `GetData` on queries polls.
- Now is a fast path (4.3): WebGL 2 answers inside the call. On WebGPU the device thread waits (section
  4.8), so the guest thread is paused, counted.

Dynamic textures: `write_texture` with `Discard` drops the whole texture's old contents, so the WebGPU
back end renames the texture from a pool, as for buffers, instead of splitting a pass for an ordered
write.

What the back ends do with `MapIntent::Often`: WebGPU keeps such buffers in the rename pool; WebGL 2 uses
`DYNAMIC_DRAW`. Guest memory is the source of every write, so there is no second CPU copy of a buffer
anywhere unless a front needs one (the OpenGL front's index copies, 4.3).

### 4.7 Passes

Draws and queries happen inside a pass; copies, whole-texture clears, dispatches, vertex capture and read
backs outside. Explicit fronts map their own passes (Direct3D 12 render passes or its target changes,
Vulkan render passes; subpasses become one pass each). Implicit fronts (Direct3D 11, OpenGL, Direct3D 9)
open a pass at the first draw after their targets change and close it before a copy, a dispatch or a
target change; a clear that comes first in a pass becomes the pass's `Clear` load. This pass tracker is a
shared helper in `wcontainers-gpu` (added at G1, with the occlusion counter of 4.12 and the combination
tracker of 4.4) so each implicit front uses the same one.

Rules a pass description follows:
- A depth-stencil attachment names only the aspects its format has (`AspectOps` for depth and for
  stencil, each with its load, store and read-only flag), so no back end sees an operation on a missing
  aspect.
- A color attachment of a 3D view names the slice it draws into (`depth_slice`, WebGPU's `depthSlice`).
- A pass with no attachment at all (Direct3D 11's drawing with only pixel shader storage writes bound)
  gives its size and sample count in `empty`. The WebGPU back end adds an attachment of its own of that
  size (`r8unorm`, writes masked off, store `Discard`), because a WebGPU pass needs one; its pipelines are
  built for it. Direct3D 11.1's forced sample counts above one are reported absent (section 9).
- Occlusion queries live inside one pass. A guest query that spans a pass split (a target change or a
  copy in the middle, which the pass tracker turns into several passes) is several interface queries,
  one per pass; the front adds their counts or combines their "any" answers.
- One interface query runs at a time (WebGPU allows one active occlusion query per pass). Guest queries
  may overlap (Direct3D 11 and 12 and OpenGL allow several active at once, for example a predicate
  around per-object queries). The front ends the running interface query at every guest begin and end
  and starts a new one, and adds each stretch's result to every guest query open during it, as DXVK
  does (`QueriesSegmented`). For counted queries the counting replay of 4.12 runs once per guest query
  over that query's own draws.

Both back ends gain from knowing passes: WebGPU needs them; WebGL 2 binds the framebuffer once per pass
and calls `invalidateFramebuffer` for `Discard` and `DontCare`, which tiled mobile GPUs use.

### 4.8 Threads and waiting

One thread owns a back end (WebGPU objects cannot be shared or moved between browser threads), so
nothing in the interface is `Send`. The front runs on that same device thread. Guest threads never call
the interface: the guest stand-in writes calls into a buffer that crosses to the device thread once a
frame or when the guest needs an answer.

When the guest needs an answer (a readable map, `GetData`, a fence wait, `glGetError` after a failing
call), its stand-in flushes the buffer and waits on a reply word: in the browser with `Atomics.wait` on
shared memory (the guest runs in a worker), natively on a futex. The device thread runs the calls, and
when one needs the GPU (`wait_read`, `wait_fence`) it waits itself: in the browser by suspending through
JSPI so its event loop can deliver `mapAsync` and presentation (JSPI is Baseline since 14 September 2026,
per the research); natively by polling the device. Then it writes the reply and wakes the guest.

The Windows-library build (section 10) runs the same split in one process: the library's entry points
write the call buffer, and a device thread owns the back end. The development desktop therefore tests the
same channel the container uses.

Object creation never waits: the stand-in allocates object names on the guest side, so `CreateBuffer`
returns at once and the creation travels in the buffer like any call.

### 4.9 Presenting frames

A surface comes from the host's graphics hook (a canvas, transferred to the device worker, in the
browser; a window natively). The DXGI layer gives the program its own back buffer textures and, at
`Present`, copies the current back buffer into `current_texture(surface)` (converting format or sRGB on
the way, as one draw) and calls `present`. One copy per frame keeps the program's back buffers ordinary
textures it can read, copy and keep across frames, which swap chains allow. Frame pacing: `Present` blocks
the guest when more than the swap chain's maximum frame latency (three by default) are queued; the device
thread learns a frame was shown from the host hook (requestAnimationFrame in the browser, which a worker
also has). The device worker returns to its event loop after every present, because a transferred canvas
updates only then.

Surface formats: `FormatUse::PRESENT` names the formats `configure_surface` takes (WebGPU: `bgra8unorm`,
`rgba8unorm`, `rgba16float`, the canvas formats; WebGL 2: `rgba8unorm`; every back end takes
`rgba8unorm`). The DXGI layer configures the back buffer's own format where it has `PRESENT`, otherwise
`rgba16float` for back buffers of more than 8 bits per channel where present, otherwise `rgba8unorm`;
the present copy converts. Colour space: SDR only. Colours are shown as sRGB and values beyond one are
clipped, as on an SDR display. DXGI reports the output's colour space as
`DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709` (`IDXGIOutput6::GetDesc1`, with the SDR luminance values),
`CheckColorSpaceSupport` answers present for that space and, on 16-bit float back buffers, for
`RGB_FULL_G10_NONE_P709`, and absent for every HDR10 space. That is what Windows answers on an SDR
display (expected; G5 compares it with the development desktop's own answers, dumped read only). HDR
output (the canvas's extended tone mapping) is not offered.

### 4.10 Errors and device loss

Calls that make objects return `Result<_, GpuError>`: `Absent(features)`, `OutOfMemory`, `DeviceLost`,
or `Invalid(rule)` (a bug in a front, never shown to a guest as success). Recording calls do not return
errors; a back end that hits one records it and reports the device lost with `LossReason::Error`.
`status()` reports `Ok` or `Lost(reason)`; fronts poll it when they present and when they wait, and
answer with their API's own words (`DXGI_ERROR_DEVICE_REMOVED` with `GetDeviceRemovedReason`,
`VK_ERROR_DEVICE_LOST`, `GL_CONTEXT_LOST`). WebGPU's `device.lost` promise and WebGL's
`webglcontextlost` event feed it.

### 4.11 The capability list

`Caps` is `Features` (bits), `Limits` (numbers) and `FormatCaps` (per format: sample, filter, render,
blend, multisample, resolve, storage read and write, storage atomics, vertex, depth-stencil, copy out,
present). A format a back end does not hold has no bits, and the front takes the route 4.12 names for
it. The bits are in the
crate; the values each back end declares (expected, to be confirmed by each back end's milestone against
the adapter):

- WebGPU in Chrome: `COMPUTE`, `VERTEX_STORAGE_READ`, `FRAGMENT_STORAGE_WRITE`, `PROVOKING_VERTEX_FIRST`,
  `DEPTH_CLAMP` (depth-clip-control), `CLIP_DISTANCE` and `DUAL_SOURCE_BLEND` where the adapter has them,
  `INDEPENDENT_BLEND`, `SAMPLE_SHADING`, `SAMPLE_MASK`, `MULTISAMPLE_READ`, `TIMESTAMPS`, `INDIRECT`,
  `INDIRECT_FIRST_INSTANCE`, `BASE_VERTEX_INSTANCE`, `TEXTURE_VIEWS`, `VIEW_SWIZZLE` (Chrome 143),
  `SEPARATE_SAMPLERS`, `SHADER_F16`, `SUBGROUPS`, `PRIMITIVE_INDEX`, `BACKGROUND_PREPARE`,
  `CLIP_DEPTH_ZERO_ONE`, `DEPTH_BIAS_CLAMP`, and `BLOCK_COMPRESSION` where the adapter has
  `texture-compression-bc` (86.8% of adapters per the research) and, for 3D textures,
  `texture-compression-bc-sliced-3d`. Absent: `MIRROR_ONCE`,
  `VERTEX_STORAGE_WRITE`, `TRANSFORM_FEEDBACK`, `READ_BACK_NOW`, `FAN_AND_LOOP`, `POLYGON_MODE`,
  `PROVOKING_VERTEX_LAST`, `CLIP_DEPTH_MINUS_ONE`, `CULL_DISTANCE`, `LOGIC_OP`, `WIDE_LINES`,
  `POINT_SIZE`, `OCCLUSION_COUNT`, `RESTART_SWITCH`, `FORMAT_REINTERPRET`, `SHADER_I64`, `SHADER_F64`,
  `RESOURCE_TABLES`, `PERSISTENT_MAPPING`, `BORDER_COLOR`, `SPARSE`, `DEPTH_BOUNDS`,
  `STENCIL_PER_FACE`. Sample counts 1 and 4 only, and 4 only on the formats WebGPU allows (not
  `r32uint`, `r32sint` or any two- or four-component 32-bit format). Limits are the adapter's highest,
  asked for at device creation; `color_bytes_per_sample` is 32 unless the adapter offers more,
  `buffer_copy_alignment` is 4. Measured on this Mac's Chrome on 2026-11-17 (`corpus/chrome-limits-run3.json`
  in scratch): 16384 2D and cube, 2048 3D, 2048 layers, 8 vertex buffers, 30 vertex attributes, 12
  uniform buffers, 10 storage buffers, 48 sampled textures and 16 samplers per stage, 28 inter-stage
  slots, 128 colour bytes per sample, 64 immediate bytes, `texture-compression-bc` and its sliced-3d
  feature present. WebGPU's defaults, which a weaker adapter may give, are lower (8192 2D, 256 layers,
  16 inter-stage slots). Formats: the 16-bit normalised formats (`r16unorm` to `rgba16snorm`) exist,
  and the 8-bit signed normalised ones render and blend, only with `texture-formats-tier1` (93.1% of
  adapters per the research, Chrome 142 and later, present here; not confirmed in Firefox); without it
  they have no bits, or no render bits, and fronts take `NormAsInteger` (4.12). No format has
  `STORAGE_ATOMIC`: WGSL has atomics only in storage buffers and workgroup memory (WGSL atomic types),
  and the texture atomics proposal (gpuweb issue 4329) is open.
- WebGL 2: `TRANSFORM_FEEDBACK`, `READ_BACK_NOW`, `FAN_AND_LOOP`, `PROVOKING_VERTEX_LAST`,
  `CLIP_DEPTH_MINUS_ONE`, `POINT_SIZE` (up to the device's range), and by extension where present:
  `POLYGON_MODE` (WEBGL_polygon_mode), `PROVOKING_VERTEX_FIRST` (WEBGL_provoking_vertex),
  `CLIP_DISTANCE` and `CULL_DISTANCE` (WEBGL_clip_cull_distance), `DEPTH_CLAMP` (EXT_depth_clamp),
  `DUAL_SOURCE_BLEND` (WEBGL_blend_func_extended), `INDEPENDENT_BLEND` (OES_draw_buffers_indexed),
  `TIMESTAMPS` (EXT_disjoint_timer_query_webgl2), `CLIP_DEPTH_ZERO_ONE` (EXT_clip_control, 93.2% per
  the research), `MIRROR_ONCE` (EXT_texture_mirror_clamp_to_edge), `DEPTH_BIAS_CLAMP`
  (EXT_polygon_offset_clamp), `BLOCK_COMPRESSION` when WEBGL_compressed_texture_s3tc (85.6%; 31.5% on
  Android), its sRGB twin, EXT_texture_compression_rgtc and EXT_texture_compression_bptc are all present,
  `SAMPLE_SHADING` when OES_sample_variables (89.72%; Firefox 0.98%) and
  OES_shader_multisample_interpolation are present. Formats by extension: the 16-bit normalised ones
  with EXT_texture_norm16 (91%; it makes the unsigned ones renderable), the signed normalised ones
  renderable with EXT_render_snorm; without them, `NormAsInteger` (4.12). The stencil aspect samples
  natively with WEBGL_stencil_texturing (89.9%), otherwise through `StencilCopiedByDraw` (4.12).
  Absent: everything that needs compute or storage,
  `INDIRECT`, `BASE_VERTEX_INSTANCE` (the extension is at 0.21%), `TEXTURE_VIEWS`, `SEPARATE_SAMPLERS`,
  `MULTISAMPLE_READ`, `SUBGROUPS`, `LOGIC_OP`, `WIDE_LINES`, `OCCLUSION_COUNT` (GLES 3.0 has only
  any-samples queries), `RESTART_SWITCH`, `BORDER_COLOR`, `DEPTH_BOUNDS`, `SAMPLE_MASK` (GLES 3.0 has
  no `glSampleMaski`; the mask is written by the program with OES_sample_variables), and
  `STENCIL_PER_FACE`: WebGL forbids drawing with different front and back stencil reference, value
  masks or write masks (the WebGL specification's "Stencil Separate Mask and Reference Value",
  `INVALID_OPERATION`), though compare and operations may differ per face. Multisampling only into
  renderbuffers (4 samples where `MAX_SAMPLES` allows): no format has `MULTISAMPLE` read in programs.
  `buffer_copy_alignment` is 1. Measured on this Mac's Chrome (same file): 16 vertex attributes, 2048 3D,
  2048 layers, 16384 2D and cube, 16 uniform blocks per stage of at most 16384 bytes (less than a
  Direct3D constant buffer's 65536), 16 texture units per stage, 30 varying vectors, 8 targets, and every
  extension named above.

Neither list is reduced to the other: each back end declares what it has and runs at its best.

### 4.12 What the interface cannot serve exactly, and what fronts do

These were looked for on purpose. Each is done exactly by a named slow path, reported absent through
the API's own mechanism, or (in two places, said so) listed as a loss.

- **Occlusion query sample counts.** Neither browser API counts samples: WebGPU promises only zero or
  non-zero (its spec's `endOcclusionQuery` steps), WebGL 2 has only any-samples queries. Every back end
  has `OcclusionAny` natively; `Occlusion` (exact counts) needs `OCCLUSION_COUNT`, absent in both. Where
  the API asks only "any" (Direct3D's occlusion predicate and `SetPredication`, OpenGL's
  `ANY_SAMPLES_PASSED`, Vulkan, which reports `occlusionQueryPrecise` false) the native query answers.
  Where it asks for a count (Direct3D 9, 11 and 12 occlusion queries, OpenGL's `SAMPLES_PASSED`) the
  front counts exactly by replay (`OcclusionCountReplay`), with a shared helper in `wcontainers-gpu`
  (G1):
  1. At the guest's query begin, the pass tracker ends the pass; if any draw recorded inside the query
     writes depth or stencil, the depth-stencil target is copied first.
  2. The query's draws run normally (with an `OcclusionAny` query, which the front can answer early
     when it is zero), and the front keeps their draw calls.
  3. At query end, a counting pass: one counting target of the pass's size and sample count (`r32float`
     where `float32-blendable` or EXT_float_blend is present, otherwise `r16float`; WebGL 2 renders float
     with EXT_color_buffer_float, 99.94% per the research), cleared to zero, with the depth-stencil
     target (read-only when no draw writes it, as with the usual bounding-box queries; otherwise the
     copy). The draws are replayed with their own depth-stencil and raster blocks, alpha to coverage
     kept, blend One plus One, and the counting variant of each program: the fragment shader with its
     outputs replaced by 1.0 at location 0 (alpha kept for alpha to coverage) and its storage writes
     removed, or the built-in one when the draw had none. The helper takes each draw's counting program
     from the front, which gets it from the shaders crate (`VariantFlags::COUNTING`); so the helper
     (G1) needs nothing from G6, and G1's tests give it counting programs written in WGSL. Blending happens exactly for the samples that
     pass every per-fragment test, so each sample holds how many fragments passed there.
  4. A reduction sums the target into the query's result (WebGPU: a dispatch; WebGL 2: a read back
     summed on the device thread, a multisampled target resolved first and the average multiplied by
     the sample count). If any sample reached the counting format's exact bound (2^24 for `r32float`,
     2048 for `r16float`), the replay runs again with the draws split into halves and the halves added,
     so the answer stays exact.
  The cost is one pass and one reduction per counted query, and a depth copy only for queries whose
  draws write depth or stencil (unmeasured; G4 measures it on real programs).
- **Other queries.** Direct3D's pipeline statistics, stream output statistics and overflow predicate,
  and Direct3D 10's `DrawAuto`, are made by the front and the stage helper from the work they issued,
  not by a back end: input assembler counts from draw arguments (indirect arguments read back,
  `IndirectReadBack`), shader invocation counts from the stage helper's dispatches; the counts Direct3D
  leaves to the implementation take the value of a GPU without vertex reuse or clipping splits
  (vertex shader invocations equal vertices fetched; primitives out of the clipper equal primitives in)
  and pixel shader invocations come from the counting replay with the depth and stencil tests off.
  Stream output counts are written by the stage helper (or read from transform feedback's primitives
  written query on WebGL 2), and `DrawAuto` draws from them.
- **Multisample counts and formats.** WebGPU has 1 and 4 samples only, and 4 not on 32-bit integer or
  wide 32-bit formats; WebGL 2 multisamples renderbuffers only. Fronts report this through the API's own
  per-format mechanism (Direct3D's `CheckMultisampleQualityLevels` returning 0 and `CheckFormatSupport`,
  OpenGL's internal format queries, Vulkan's sample count flags), which programs must call anyway to
  learn quality levels. This falls short of Microsoft's format tables in one way, which the owner is
  told: 10_1 and 11_0 require 4 samples on `R32_UINT` and others, and 11_0 requires 8 samples on 47
  formats; on adapters without 16-bit normalised formats, those formats have no multisampling either. No exact route exists (8 samples from two 4-sample halves runs the pixel shader twice, which
  storage writes would see), and the next level whose tables WebGPU meets is 10_0, which would take
  compute and tessellation from every program.
- **Binding slots beyond the back end's limits.** Direct3D 11 promises 128 textures per stage; WebGPU's
  adapters give 16 or more (48 on this Mac), WebGL 2 usually 16 units. Storage bindings beyond the limit
  are packed (`StoragePacked`). Sampled textures beyond it have no exact route: creating the shader fails
  with `E_INVALIDARG` (`BindingsBeyondLimit`, a counted refusal, and a known gap). The corpus at G6
  measures how many real shaders this hits.
- **Vertex input slots and elements.** Direct3D 11 has 32 input slots and 32 elements (10_0: 16 and
  16); WebGPU's adapters give 8 vertex buffers and 16 to 30 attributes. Streams and attributes beyond the
  back end's limits are fetched by the vertex program from read-only storage buffers by vertex and
  instance index, with the format decoded in the program and bounds checked, so reads past the end give
  zero as Direct3D says (`VertexPulledInShader`, `VariantKey::pulled`, offset and stride in the helper
  constants). Exact. WebGL 2 binds a buffer per attribute and GLES 3.0 guarantees 16 attributes, which
  is 10_0's number, so it never needs this.
- **Constant buffer slots and sizes.** Direct3D has 14 constant buffers per stage of up to 65536 bytes;
  the helper constants take one more uniform binding. WebGPU gives 12 uniform buffers per stage by
  default (12 on this Mac) of 65536 bytes: the slots beyond the limit are read as read-only storage
  buffers, which hold the same 16-byte rows (`ConstantsAsStorage`). Exact. WebGL 2 gives at least 12
  uniform blocks (16 here) of at least 16384 bytes (16384 here): a constant buffer beyond the block
  count, or bound with more bytes than a block holds, is copied on the GPU into a 32-bit RGBA integer
  texture (`copy_buffer_to_texture`) when it changed since the last copy, and read with `texelFetch`
  (`ConstantsAsTexture`). Exact. The texture takes a unit; a shader that then needs more units than the
  back end has is refused (`BindingsBeyondLimit`).
- **Values between stages.** Direct3D 11 passes 32 four-component registers from the vertex stage to
  the pixel stage (10_0: 16). WebGPU gives 16 by default and 28 here; WebGL 2 at least 15 vectors plus
  position (30 here). The shaders crate packs values by component where their interpolation agrees,
  which costs nothing at run time; a program that still needs more than the back end has is refused at
  creation (`VaryingsBeyondLimit`, a counted refusal and a known gap, like `BindingsBeyondLimit`). The
  corpus at G6 measures how many real shaders this hits.
- **Texture sizes.** Not emulated: a front reports the feature level whose sizes the back end meets
  (section 9), and refuses larger textures as its API does.
- **Atomics on storage textures.** Direct3D 11_0 requires every atomic operation on typed storage views
  of `R32_UINT` and `R32_SINT`, and exchange on `R32_FLOAT` (tiled light culling and depth min-max
  passes use them). No browser format has `STORAGE_ATOMIC` (4.11). The front gives each texture used
  through such a view a buffer holding the view's texels in the layout `copy_texture_to_buffer` writes
  (rows padded to the copy alignment), and binds the variant that reads, writes and runs atomics on that
  storage slot through the buffer at the same binding (`VariantKey::atomics_through_buffer`; the row
  pitch and rows per layer in `HelperValue::TexelLayout`; float exchange as u32 exchange of the bits).
  The front tracks which of texture and buffer is newer: before a draw or dispatch with the variant, a
  newer texture is copied into the buffer; before any other use of the texture (sampling, target, copy,
  read back, a storage view without atomics), a newer buffer is copied back
  (`AtomicsThroughBuffer`). Exact: the same 32-bit values, and storage buffer atomics are atomic. Typed
  storage buffers (`RWBuffer<uint>`) are storage buffers already (`TexelBufferRoute::Storage`), so their
  atomics are native.
- **16-bit normalised formats and rendered signed normalised ones.** Required from 10_0 (sampled,
  loaded, rendered and blended `R16_UNORM` to `R16G16B16A16_SNORM`, rendered and blended `R8_SNORM` to
  `R8G8B8A8_SNORM`). Native where the back end has them (4.11). Otherwise the front stores the texture
  as the integer format of the same bits (`r16uint` for `R16_UNORM`, `rgba8sint` for a rendered
  `R8G8B8A8_SNORM`; an 8-bit snorm texture made without a render target bind flag stays native, since
  sampling it needs nothing). Programs reading it take the variant that loads the integers, converts
  them as Direct3D's conversion rules say and filters with the sampler's state from the helper
  constants, as `FilterInShader` does (`VariantKey::decoded`); programs writing it take the variant that
  converts the output to the integer (`VariantKey::targets_in_shader`); blending into it is done in the
  program from a copy of the target (`BlendInShader`, drawn one primitive at a time where primitives may
  overlap, `TargetReadPerPrimitive`). Copies, clears with the converted value, and read backs see the
  same bits as the guest's format (`NormAsInteger`). Exact within Direct3D's filtering tolerance, as for
  `FilterInShader`. Multisampling of these formats is reported absent on such adapters, which adds to
  the multisample shortfall below.
- **Sample mask.** Direct3D 10 and 11's `OMSetBlendState` mask, Direct3D 9's `D3DRS_MULTISAMPLEMASK`,
  OpenGL's `glSampleMaski`. Native with `SAMPLE_MASK` (WebGPU's `multisample.mask`). WebGL 2 has no
  mask in its pipeline: with `SAMPLE_SHADING` (OES_sample_variables) the front binds the variant whose
  fragment program writes `gl_SampleMask` from the helper constants (`VariantFlags::SAMPLE_MASK`,
  `HelperValue::SampleMask`; with the built-in fragment program for draws without one), which ANDs with
  coverage for every write as the fixed mask does (`SampleMaskInShader`). Exact. Without the extension
  (10.3% of WebGL 2 browsers, nearly all Firefox) no route exists for a partial mask on a multisampled
  target, so the Direct3D fronts report no multisampling for any format there (allowed at 10_0, where it
  is optional per format); with one sample, a draw whose mask clears bit 0 writes nothing and the front
  drops it (expected to match Direct3D, including for occlusion counts; G3 compares).
- **Stencil sampled in programs.** Required at 10_0 (`X24_TYPELESS_G8_UINT` loads). WebGPU samples the
  stencil aspect natively; WebGL 2 with WEBGL_stencil_texturing. Without it, the WebGL 2 back end fills
  an `r8unorm` texture from the stencil when the stencil changed since the last fill: eight full-screen
  draws, each testing one stencil bit with the stencil test (no writes) and adding 2^k/255 with
  additive blending, which unorm8 blending sums exactly; then one draw converts it to `r8ui` for the
  program's integer loads (`StencilCopiedByDraw`). Exact.
- **Copies between formats of one typeless group.** Direct3D's `CopyResource` and
  `CopySubresourceRegion` copy between formats of the same group (`R8G8B8A8_UNORM` to `_UINT`,
  `R32_FLOAT` to `_UINT` or `D32_FLOAT`), which `copy_texture` accepts. WebGPU copies texture to texture
  only between equal formats or sRGB twins, so the back end copies through a buffer (`CopyThroughBuffer`,
  two copies); into `depth32float`, which WebGPU cannot write by a copy, by a draw writing depth from the
  source read as `r32float` (`DepthWrittenByDraw`). `depth24plus` has no other member a texture can be
  made in (the group's other formats are view formats, served by views), so it is only copied to itself,
  which WebGPU does whole. WebGL 2 copies colour formats through a pixel pack buffer: `readPixels` from the
  source attached to a framebuffer, then `texSubImage` from the buffer into the destination, on the GPU
  (`CopyThroughBuffer`); equal renderable formats with `blitFramebuffer` or `copyTexSubImage`.
- **Copies and read backs of formats WebGL 2 cannot attach.** GLES 3.0 reads and copies textures only
  through a framebuffer, and some formats cannot be attached: `RGB9E5` (never a target in Direct3D
  either) and, without EXT_render_snorm, 8 and 16-bit signed normalised textures made without a render
  target bind flag (those made with one are integers, above). Direct3D requires read backs of them
  (10_0 table: CPU lockable). Such textures can only change by writes and copies, never by drawing, so
  the back end keeps
  their bytes on the host as well, updated by every write (`HostCopyKept`), answers read backs and copies
  out of them from that copy, and updates both from the source on a copy into them (reading a GPU-side
  source back first, `HostCopyReadBack`). This is the same mechanism as for BC textures. Exact.
- **Block-compressed (BC) textures.** Required from 10_0 (BC1 to BC5) and 11_0 (BC6H, BC7). Where the
  back end samples them natively (`BLOCK_COMPRESSION`), they are native. Otherwise the back end stores
  each BC texture decoded (BC1 to BC3 and BC7 as RGBA8, sRGB kept; BC4 and BC5 as R16 and RG16 float
  from the exact interpolated values; BC6H as RGBA16 float, which is what BC6H decodes to), decoding on
  the device thread at each write with the `bcdec_rs` crate (MIT; a port of bcdec, MIT or Unlicense)
  (`BlockDecoded`). The format still reports sampling and filtering; storage use is reported absent, as
  for every BC format. To answer copies and read backs in blocks, the back end keeps each such texture's
  blocks in host memory, updated by every write: a read back of it, or a copy into a buffer, answers from
  those blocks, taken when the read is recorded; a copy between two BC textures copies both forms; a copy
  from an uncompressed texture of the same block size (which Direct3D allows, for textures compressed on
  the GPU) reads the source back to the device thread, which waits, decodes it and writes both forms
  (`HostCopyReadBack`). Exact for BC6H and BC7, whose decoding Microsoft specifies to the bit; for BC1 to
  BC5 within the interpolation tolerance Direct3D allows, compared with the reference at G2. WebGL 2
  cannot read compressed textures back at all (they cannot be framebuffer attachments), so its back end
  keeps the host blocks for every BC texture, native or not.
- **Sampler level-of-detail bias.** Neither WebGPU's nor WebGL 2's samplers have one. The front puts the
  sampler's bias in the helper constants and binds the variant that adds it to every sampling through
  that sampler (`LodBiasInShader`, `VariantKey::lod_bias`): implicit-level sampling becomes sampling with
  a bias (WGSL `textureSampleBias`, GLSL ES `texture` with bias; the shader's own bias is added), an
  explicit level gets the bias added (as Vulkan defines `mipLodBias`, which DXVK relies on for
  Direct3D), gradients are scaled by 2 to the bias, and comparison sampling, which WGSL cannot bias, is
  filtered in the program from loads at the biased level (`FilterInShader`). The bias value is not in
  the key, so changing it builds nothing. Exact up to the level computation of the hardware for gradient
  and comparison sampling, compared at G4.
- **Depth range with near beyond far.** Vulkan's viewports and OpenGL's `glDepthRange` allow it (some
  engines use it for reversed depth); WebGPU and WebGL 2 require near at most far. The front passes the
  range swapped and binds the variant writing clip depth as `w - z`, which gives the same depth value
  (`ViewportDepthFlip`); the result differs from the hardware's by rounding only (unmeasured; the Vulkan
  milestone compares it).
- **Cube textures and other view kinds on WebGL 2.** A WebGL 2 texture's kind is fixed when it is made,
  and it has no views. `TextureDesc::cube` and the layer count fix the kind (4.2). A view of another kind
  (one layer of an array, a cube seen as a 2D array, a 2D texture seen as a one-layer array) comes from a
  second texture of that kind, which the front keeps and copies into whenever the first changed since the
  last copy (`ReinterpretCopy`; a depth or stencil texture is copied with `blitFramebuffer`). Mip ranges
  are set per bind with the texture's base and max level; two bindings of one texture with different mip
  ranges in one draw take the second texture too. Cube arrays are absent on WebGL 2 (GLES 3.0), which is
  why it reports 10_0.
- **Floating point modes** (a listed loss). Direct3D 11 requires 32-bit denormals flushed in shaders;
  WGSL and GLSL ES leave denormal handling to the implementation. Pictures are compared with tolerance.
- **Clip depth from zero on WebGL 2 without EXT_clip_control** (6.8% of WebGL 2 browsers; a listed loss).
  The vertex program writes z as 2z - w (`ClipDepthRewrite`), which loses low bits of depth and the
  precision gain of reversed depth. Measured against the reference at G3.
- Format reinterpretation through views is absent in both; fronts copy (`ReinterpretCopy`). Exact, slow.
- Predicated drawing (Direct3D `SetPredication` without the hint flag, Vulkan conditional rendering)
  has no GPU-side form in either API. The front waits for the query's answer on the device thread and
  skips or keeps the draws (`PredicationWait`). Exact, slow; with the hint flag the draws are kept.
- Depth and stencil read back: WebGPU cannot copy `depth24plus` out. The back end draws the depth into a
  color texture and reads that (`DepthReadByDraw`); exact for 16 and 32-bit depth, and for 24-bit depth
  as stored.
- Different stencil reference or masks per face (Vulkan, OpenGL, newer Direct3D 12): WebGPU has one of
  each, and WebGL 2 forbids drawing with them different (4.11); the front draws twice, culling one face
  each time (`StencilFacesSplit`). Exact.
- Depth bounds test (optional in Direct3D 12 and Vulkan): absent in both, reported absent.
- Logic operations: optional at Direct3D 11_0 and reported absent there. OpenGL's `glLogicOp` (core)
  runs in the program reading a copy of the target taken before the draw (`LogicOpInShader`; the copy
  is bound in the helper set at `TARGET_COPY_BINDING` plus the target, the operation is in the helper
  constants, `HelperValue::TargetCombine`); a draw whose primitives may overlap is drawn one primitive
  at a time with a copy between (`TargetReadPerPrimitive`). Exact, very slow, and rare.
- Mirror-once addressing (required by Direct3D at every level): native where `MIRROR_ONCE`, otherwise
  the program samples the absolute coordinate with clamping and the original gradients
  (`MirrorOnceInShader`), which gives the same footprint and level of detail. Exact.
- Filtering a texture the back end cannot filter: `FilterInShader` (4.5).
- Targets of more bytes per sample than `color_bytes_per_sample`: each draw runs once per group of
  targets that fits, each run starting from a saved copy of depth and stencil, storage writes and
  queries kept only in the first run (`TargetsSplit`). Exact.
- Drawing with no targets (Direct3D 11's storage-only drawing): the back end's own attachment (4.7).
  Direct3D 11.1's forced sample counts are reported absent (section 9).
- Layered drawing (`SV_RenderTargetArrayIndex` from a geometry shader, for one-pass cube shadows and
  volume tables): the stage helper's output carries each primitive's layer, and the draw runs once per
  layer used, into a pass on that layer's view, keeping only that layer's primitives (`LayerSplit`).
  Exact. The layer or viewport from the vertex shader (an optional Direct3D 11.3 capability) is
  reported absent. Several viewports: `MultiViewportSplit`.
- Vertex input: every format can be named (`VertexFormat` has every component count of every type);
  WebGPU's missing ones are fetched in the program or converted (`VertexFormatConverted`). Step rates
  other than one instance: `InstanceRateInShader`. Direct3D reads zero past the end of a vertex buffer:
  for draws without indices the front draws the part past the end with a zero buffer bound
  (`VertexFetchZeroed`), exact; for indexed draws, indices past the end get what WebGPU's robust access
  gives (zero or a value from inside the buffer), a listed loss, as DXVK has on Vulkan without
  `robustBufferAccess2`.
- Stream output that transform feedback cannot write as declared (gaps that must leave the buffer's
  bytes alone, several elements per buffer in separate mode) is captured whole into a scratch buffer and
  repacked by a second capture that also reads the destination's old bytes (`StreamOutRepacked`).
  Exact.

### 4.13 Compile-time resolution

`Backend` has associated types for every object, so a front stores `B::Buffer` directly (a `wgpu::Buffer`
or a GL name), never an index into a table of trait objects. Every interface call is a generic method
call resolved when the front is compiled with its back end: `Device<WebGpu>` and `Device<WebGl>` are
two separate compiled fronts. How the Direct3D family shares one back end and one device thread across
its libraries is section 10.

## 5. Proof on paper

For each front, a frame's calls and where they land. Then, for each back end, how it carries them out.
Slow paths and absent features are named as they appear.

### 5.1 Direct3D 11 (implicit)

Device creation: the DXGI layer enumerates one adapter from `caps()`; `D3D11CreateDevice` picks the
feature level of section 9. `CreateBuffer`, `CreateTexture2D`, views and samplers map to `create_*`;
initial data is a `write_buffer`/`write_texture` from the stand-in's copy (4.6). `CreateVertexShader` and the others
convert DXBC through the shaders crate (or take it from the cache) and call `create_shader`; the program is
made per vertex/pixel pair, built ahead in the background as 4.4's step 2 says and otherwise at first
use. `CreateBlendState`, `CreateDepthStencilState`, `CreateRasterizerState` call the block constructors;
`CreateInputLayout` keeps the layout, and the vertex layout block is made per (layout, strides).
`Texture1D` is a 2D texture one texel high; `D3D11_RESOURCE_MISC_TEXTURECUBE` sets `cube`. A sampler's
`MipLODBias` goes to the helper constants (`LodBiasInShader` where non-zero); a rasterizer state's
`DepthBiasClamp` is native with `DEPTH_BIAS_CLAMP`, and depth bias on line and point draws is
`DepthBiasInShader` (4.12).

A frame: `OMSetRenderTargets` marks the pass tracker; `ClearRenderTargetView` becomes the next pass's
`Clear` load, or `clear_texture` (the view's texture, its format, its mip and its layers or 3D slices)
if a pass is open on other targets; `ClearDepthStencilView` likewise. `IASetVertexBuffers`,
`IASetIndexBuffer`, `*SetShaderResources`, `*SetConstantBuffers`, `*SetSamplers` update the context's
slots and mark binding sets dirty; at the draw the front makes the sets it needs (one per frequency group:
constant buffers, textures and samplers, per stage). `Map(WRITE_DISCARD)` on a constant buffer returns a
fresh shadow block; `Unmap` is `write_buffer(Discard)` (4.6). `UpdateSubresource` is
`write_buffer(Ordered)` or `write_texture` of the stand-in's copy. `ClearView` is `clear_rects`,
`GenerateMips` is `generate_mips`, `ClearUnorderedAccessView*` on buffers is `clear_buffer` with the
element's bits. On textures it is a `dispatch` of a built-in compute program (WGSL in the shaders crate,
one per format class) writing the value through a storage view of the cleared range: every format a
typed storage view can have is one the back end stores into (`FormatCaps` `STORAGE_WRITE`, which
`CheckFormatSupport` reports), so no other route is needed; `Uint` clears of a normalised or float
format write the low bits of each value as Direct3D defines. `Draw*` passes the context's `DrawCall`; `DrawIndexedInstancedIndirect` uses an
indirect range. `CopyResource` and `CopySubresourceRegion` close the pass and copy (`copy_texture` takes formats of
one typeless group, 4.12). `Dispatch` closes the
pass and calls `dispatch`. `Map(READ)` on a staging texture calls `read_back_now` (or polls with
`DO_NOT_WAIT`). `GetData` on a query polls `read_state`; occlusion counts come from the counting replay
and other queries from the front's own counts (4.12). `Present` goes to the DXGI layer (4.9).

`OMSetBlendState`'s sample mask goes into the raster block where `SAMPLE_MASK`, otherwise 4.12's route.
Overlapping queries: `QueriesSegmented` (4.7).

Append, consume and counter views (`D3D11_BUFFER_UAV_FLAG_APPEND` and `_COUNTER`, compute particle
systems): the front gives each such view a hidden counter, one u32 in a small storage buffer of its
own, bound at the `SlotKind::Counter` binding of the view's storage slot (224 plus the slot). The
shaders crate converts the shader's counter operations (`IMM_ATOMIC_ALLOC`, `IMM_ATOMIC_CONSUME`, the
`IncrementCounter` and `DecrementCounter` intrinsics) into atomic add and subtract on that binding, and
the reflection lists it. The initial counts of `CSSetUnorderedAccessViews` and
`OMSetRenderTargetsAndUnorderedAccessViews` (other than -1, which keeps the count) are an ordered
`write_buffer` of the counter; `CopyStructureCount` is `copy_buffer` of its 4 bytes; an indirect draw
of particles then reads them in place. Exact, and native on WebGPU. On WebGL 2 there are no storage
views at 10_0. Direct3D 12 gives counters its own resource and offset (`CounterResource`), which the
front binds at the same binding.

Geometry, hull and domain shaders and stream output: the front hands the stages to the shaders crate's
stage helper (section 7), which issues `dispatch` and `draw` (WebGPU) or `capture_vertices` and `draw`
(WebGL 2). Deferred contexts record into a front-side list replayed on the immediate context at
`ExecuteCommandList`.

### 5.2 Direct3D 12 (explicit)

`CreateGraphicsPipelineState` creates the blocks, the program and calls `prepare` with the PSO's target
formats and topology class, so WebGPU compiles in the background long before the draw; no first-draw
pause. Root signatures decide the binding sets: root constants become immediates (up to the back end's
`immediate_bytes`, beyond that `ImmediatesSpilled`); root descriptors (GPU virtual addresses) are
resolved through the front's address map to buffer ranges; descriptor tables become binding sets made
when a draw is recorded (4.5). `ResourceBarrier` does nothing on either back end (both synchronise for
us) except closing the pass when a target becomes a shader resource. Command lists record into a
front-side list; `ExecuteCommandLists` replays it into the back end and submits. `Signal` on a fence
records `submit()` and its fence; `SetEventOnCompletion` and `GetCompletedValue` use `fence_done` and
`wait_fence`. Upload heaps are persistently mapped guest memory written at submit (4.6); readback heaps
are finished into guest memory when their fence completes. `ExecuteIndirect`: a command signature of
only draw, indexed draw or dispatch arguments becomes one indirect call per command at the signature's
stride (`INDIRECT`); with a count buffer, the largest count is drawn after a small compute pass zeroes
the commands past the count (`IndirectCountExpanded`, no read back). A signature that also changes
vertex or index buffers, root constants or root descriptors per command has no GPU-side form in either
API: the argument and count buffers are read back to the device thread, which waits, decodes each
command (root descriptors through the address map) and records ordinary draws (`IndirectReadBack`).
Exact, slow. Placed resources each get their own storage:
aliased resources' contents are undefined in Direct3D 12 after aliasing, so separate storage is exact,
only larger. Bundles replay their recorded calls. Reported absent through `CheckFeatureSupport`: ray
tracing, mesh shaders, variable rate shading, tiled resources, rasterizer ordered views, conservative
rasterization, 64-bit shader integers; resource binding tier 1 without `RESOURCE_TABLES`, tier 3 with it.

### 5.3 OpenGL 3.3 (Mesa, second batch)

Mesa's state tracker calls a Gallium driver we write on the interface. Its `create_*_state` calls map to
our blocks; `set_*` calls mark dirty and the driver looks blocks up by value at the draw (4.4).
`glBufferSubData` is `write_buffer(Ordered)`; `glMapBufferRange` with invalidate is `Discard`, with
unsynchronised `NoOverwrite`, with read `read_back_now`. `glReadPixels` is `read_back_now` (native on
WebGL 2). Fans and loops use `draw_fan_or_loop`. Transform feedback (core since 3.0) is
`capture_vertices` on WebGL 2 and the stage helper on WebGPU (`StreamOutAsCompute`). Geometry shaders
(3.2) go through the stage helper. Provoking vertex: native last on WebGL 2; on WebGPU `ProvokingRewrite`.
`glPrimitiveRestartIndex` with any index, or restart off: `RestartRewrite` (WebGPU and WebGL 2 restart only
at the largest value, always). `glLogicOp`: `LogicOpInShader`, one primitive at a time where primitives
may overlap (`TargetReadPerPrimitive`). `GL_SAMPLES_PASSED`: `OcclusionCountReplay`. `glPolygonMode`: native with
WEBGL_polygon_mode, otherwise `FillModeConverted`. Clip depth from minus one: native on WebGL 2,
`ClipDepthRewrite` on WebGPU. Multisample textures (3.2): WebGPU has them; WebGL 2 does not, so Mesa
reports OpenGL 3.1 there (3.1 needs texture buffers: `TexelBufferAsTexture`). Wide lines report a range of
1 to 1, which core profile allows. Border colors: `BorderInShader` on both. `GL_TEXTURE_LOD_BIAS`:
`LodBiasInShader`. `glDepthRange` with near beyond far: `ViewportDepthFlip`. Cube map and array targets
set `TextureDesc::cube` and the layers as the target says; 1D textures are one texel high.

### 5.4 OpenGL 4.6 (Mesa, second batch, WebGPU only)

Adds over 3.3: tessellation (stage helper, `TessellationAsCompute`); 64-bit floats (4.0), which neither
browser has: `F64Emulated` through Mesa's soft-float lowering, slow and counted; image load and store and
atomic counters (storage textures and buffers); compute (`dispatch`); multi-draw indirect (one indirect
draw each; with a count read from a buffer (4.6), `IndirectCountExpanded`); SPIR-V shaders (into the
common form through naga's SPIR-V reader, section 6); clip control (`ClipDepthRewrite`); anisotropic
filtering (native). Cull distances are absent on WebGPU: Mesa lowers them to discards in the shader. On
WebGL 2 the version stays at 3.1.

### 5.5 Vulkan 1.1 (Mesa Vulkan runtime, second batch, WebGPU only)

Pipelines are complete block sets plus `prepare`: no stutter. Descriptor sets become binding sets (made
when the set is updated, not per draw). Push constants are immediates (Vulkan's minimum is 128 bytes;
WebGPU's default immediate size is 64, the rest is `ImmediatesSpilled`). Render passes map to passes,
subpasses to one pass each, input attachments to sampled textures in the next pass. Barriers and events
do nothing. Host-visible coherent memory, which Vulkan requires, is guest memory written at submit (4.6).
Secondary command buffers replay. A viewport whose `minDepth` exceeds `maxDepth` (allowed by Vulkan
and used for reversed depth): `ViewportDepthFlip`. A sampler's `mipLodBias`: `LodBiasInShader`; the
`maxSamplerLodBias` limit is reported as 15.99, the range Direct3D also allows. `VK_IMAGE_CREATE_CUBE_COMPATIBLE_BIT`
sets `cube`. Reported absent through Vulkan's features: `logicOp`,
`fillModeNonSolid`, `wideLines`, `largePoints`, `occlusionQueryPrecise`, `pipelineStatisticsQuery`,
`shaderFloat64`, `shaderInt64`, `sparseBinding`, `multiViewport`. Reported present through slow paths,
because engines ask for them at start-up (research, vulkan-on-webgpu-inputs Q5a): `geometryShader` and
`tessellationShader` (stage helper), `multiview` (required by 1.1: `MultiviewSerial`),
`vertexPipelineStoresAndAtomics` false. WebGL 2 cannot host Vulkan (Vulkan requires compute).

### 5.6 Direct3D 9 (later, with 32-bit x86)

Fixed function uses one general program whose behaviour comes from immediates and a uniform block, so
state changes never make new programs (brief). Shaders 1 to 3 convert through dxbc-spirv's SM3 path.
`DrawPrimitiveUP` and `DrawIndexedPrimitiveUP` write vertex data with `write_buffer_ring`. `Lock` flags
map to write modes as in 4.6. Fans are native in Direct3D 9: `draw_fan_or_loop`. Point sprites and point
size: native on WebGL 2 within its range, `PointsAsQuads` on WebGPU. Border address mode:
`BorderInShader`. Depth bias on lines: `DepthBiasInShader` on both. `D3DSAMP_MIPMAPLODBIAS`:
`LodBiasInShader`. `GetRenderTargetData` is
`read_back_now`. Clip planes and fog are program features. The half-pixel offset is applied in the
converted vertex shader.

### 5.7 The WebGPU back end carrying these out

- Blocks to pipelines through the hash map of 4.4; `prepare` calls `createRenderPipelineAsync` with at
  most as many compiles in flight as the host's logical processors (research: more stalls the page).
- Binding sets to bind groups, cached (4.5). Immediates by `setImmediates` where present (Chrome 149+),
  else a ring buffer bound with a dynamic offset.
- Writes as in 4.6: renaming pool for `Discard`, staging and copy for `Ordered`, direct for
  `NoOverwrite`. One `submit` per frame or per wait (submit costs about 13 µs per call, research).
- Read backs: copy into a `MAP_READ` buffer, then `mapAsync`; `wait_read` suspends through JSPI; the
  answer is copied from the mapped range into guest memory by `finish_read`.
- In the back end itself: fans and loops by the default conversion; unaligned buffer writes, copies and
  clears (`UnalignedBufferEdge`); passes without targets on its own attachment; formats WebGPU lacks
  (three-component 8 and 16-bit vertex formats, B5G6R5 and other 16-bit packed formats, A8) converted on
  upload (`VertexFormatConverted`, `FormatConverted`); BC textures decoded where `BLOCK_COMPRESSION` is
  absent, with host blocks for copies and read backs (`BlockDecoded`, `HostCopyReadBack`); copies
  between a BC and an uncompressed texture of the same block size, and between formats of one typeless
  group, through a buffer (`CopyThroughBuffer`), into `depth32float` by a draw (`DepthWrittenByDraw`).
  The fast-path defaults it keeps: `clear_texture`, `clear_rects`, `generate_mips`, `write_buffer_ring`,
  `draw_fan_or_loop`.
- What fronts do on WebGPU, from its `Caps` (all program variants of 4.5, made by the shaders crate):
  restart off or another index (`RestartRewrite`); provoking last (`ProvokingRewrite`); depth bias on
  lines and points (`DepthBiasInShader`); polygon mode (`FillModeConverted`); multiple viewports and
  layers, only reachable from geometry shaders (`MultiViewportSplit`, `LayerSplit`); typed buffers
  (`TexelBufferAsStorage`); border colours, mirror once, filtering of unfilterable formats and
  level-of-detail bias (`BorderInShader`, `MirrorOnceInShader`, `FilterInShader`, `LodBiasInShader`);
  storage bindings at unaligned offsets (rebased, 4.5); exact occlusion counts (`OcclusionCountReplay`);
  instance step rates and vertex streams past the limits (`InstanceRateInShader`,
  `VertexPulledInShader`); constant buffers past the limit (`ConstantsAsStorage`); fat targets
  (`TargetsSplit`); clip depth from minus one and swapped depth ranges (`ClipDepthRewrite`,
  `ViewportDepthFlip`); atomics on storage textures (`AtomicsThroughBuffer`); 16-bit normalised formats
  and rendered snorm ones without `texture-formats-tier1` (`NormAsInteger`, `BlendInShader`);
  overlapping queries (`QueriesSegmented`); stencil values per face (`StencilFacesSplit`).
- Absent and reported as such: transform feedback (the stage helper builds stream output instead), logic
  ops, wide lines, sample counts other than 1 and 4, 64-bit shader types, resource tables, persistent
  mapping, sparse resources.

### 5.8 The WebGL 2 back end carrying these out

- Blocks applied by difference (4.4); programs linked per pair, in parallel where
  KHR_parallel_shader_compile is present, otherwise at first draw (`PipelineMiss`).
- Binding sets to units and uniform-block bindings; combined samplers per pair (4.5). Uniform buffers
  are std140 in GLSL ES; Direct3D's constant buffer packing fits std140 for the layouts dxbc-spirv
  produces (expected; checked by the corpus at G6). Immediates are plain uniforms set with `uniform4uiv`
  only when changed.
- Writes: `bufferSubData` (ordered by GL) and `bufferData` orphaning for `Discard`. Chrome charges a
  `bufferSubData` into a buffer a queued draw uses in proportion to the whole buffer (research: 0.9 ms at
  16 MB), so `MapIntent::Often` buffers are kept small and renamed.
- Read back now: `readPixels` and `getBufferSubData`, inside the call.
- Fans and loops native; transform feedback native; base vertex and first instance by binding the vertex
  streams at an offset (`BaseVertexRebind`); indirect draws read their arguments back first
  (`IndirectReadBack`, with `INDIRECT` absent).
- Stream output declarations transform feedback cannot write: `StreamOutRepacked`. Exact occlusion
  counts: `OcclusionCountReplay`. Direct3D's clip depth: native with EXT_clip_control, otherwise
  `ClipDepthRewrite` (a listed loss, 4.12).
- Textures: the GL target from `TextureDesc::own_view` (4.2); mip ranges by base and max level at bind
  time; other view kinds from the front's second texture (`ReinterpretCopy`, 4.12). Views to draw into
  (one level and one layer or slice) are framebuffer attachments by `framebufferTextureLayer`, which
  need no views. Its own overrides: `clear_texture` and `clear_rects` by `clearBuffer*` on each
  attachment (scissored for rectangles), `generate_mips` by `generateMipmap` where the format is
  colour-renderable and filterable (GLES's rule) and otherwise the default (the level read is set as
  the texture's base and max level, so the level drawn is outside it and no feedback loop arises),
  `draw_fan_or_loop`,
  `read_back_now`. Stencil views: native with WEBGL_stencil_texturing, else `StencilCopiedByDraw`.
  Copies: equal renderable formats by `blitFramebuffer` (depth and stencil too) or `copyTexSubImage`,
  formats of one typeless group through a pixel pack buffer (`CopyThroughBuffer`), formats it cannot
  attach through the host copy (`HostCopyKept`, `HostCopyReadBack`; 4.12). BC textures native
  with the extensions of 4.11, else decoded (`BlockDecoded`); either way their blocks are kept on the
  host for read backs and copies, since GLES cannot read compressed textures back; copies into them from
  uncompressed textures read the source back (`HostCopyReadBack`).
- What fronts do on WebGL 2, from its `Caps`: constant buffers beyond the blocks or the block size
  (`ConstantsAsTexture`); depth bias clamp without EXT_polygon_offset_clamp, and bias on lines and points
  (`DepthBiasInShader`); level-of-detail bias (`LodBiasInShader`); border colours (`BorderInShader`);
  swapped depth ranges (`ViewportDepthFlip`); typed buffers (`TexelBufferAsTexture`); stencil values
  per face (`StencilFacesSplit`); targets with different blends without OES_draw_buffers_indexed
  (`TargetsSplit`); a partial sample mask (`SampleMaskInShader`, or no multisampling without
  OES_sample_variables); 16-bit normalised and rendered snorm formats without their extensions
  (`NormAsInteger`, `BlendInShader`).
- Absent and reported as such: compute and storage (so Direct3D 11 reports feature level 10_0 and
  Direct3D 12 is not offered), texture views (fronts make views by copying: `ReinterpretCopy`),
  multisample textures read in shaders (reported per format through `CheckFormatSupport`), subgroups,
  logic ops, wide lines, stencil values per face and the sample mask in the pipeline. BGRA textures are stored as RGBA, swizzled on upload (`FormatConverted`).

## 6. Shaders

### 6.1 The common form: a naga module

Decided: the common form is `naga::Module` (naga 30.0.2), validated, with its entry point. Back ends write
their language from it with naga's own writers: WGSL for WebGPU, GLSL ES 3.00 for WebGL 2, each with
the language-specific fixes that back end needs (6.5). The choice rests on what was tried (6.3): naga
reads and writes a module directly, is MIT or Apache-2.0, already ships both writers, builds for
WebAssembly, and can be serialised for the cache (its `serialize` feature). SPIR-V as the common form
would put a SPIR-V reader between every front and every back end; naga's reader rejected 36% of our
real DXBC corpus and 7 of 8 DXIL shaders as SPIR-V (6.3).

### 6.2 Getting shaders into the common form

- **DXBC (Direct3D 10 and 11, later 9):** `dxbc-spirv` (MIT) parses DXBC and runs its passes (SSA,
  structured control flow, type propagation). Its intermediate form is designed to be translated onward
  and has a documented serialised token form (ir/ir.md, "Serialization"). We take that serialised form
  and lower it straight to a naga module with our own Rust code in `wcontainers-shaders`. This is the
  branch point the brief names. Our lowering also decides what dxbc-spirv's SPIR-V lowering decides for
  Vulkan (resource bindings, inputs and outputs, system values) in the shape our back ends need, so no
  SPIR-V is made.
- **DXIL (Direct3D 12):** `dxil-spirv` (MIT; the converter vkd3d-proton uses; it has its own small LLVM
  bitcode reader, so no LLVM) turns DXIL into SPIR-V, which naga's SPIR-V reader turns into the common
  form. The reader's gaps found in 6.3 (demote to helper, image buffers, storage images without a format,
  subgroup elect, memory operands on stores, runtime arrays of resources) are fixed in naga's SPIR-V
  reader and sent upstream; until merged we depend on our fork by git revision. The same reader serves
  Vulkan's SPIR-V and Mesa's (NIR to SPIR-V) later, so the work is shared by three fronts.
  `vkd3d-shader` also reads DXIL but is LGPL and was not chosen.
- **How the C++ is built into our tree:** `dxbc-spirv` and `dxil-spirv` are git submodules of
  `wcontainers-shaders`, compiled by its build script with the `cc` crate (no meson or CMake at build
  time; the file lists are read from their build files once and kept in the build script). `dxbc-spirv`
  is C++17 (`cpp_std=c++17` in its meson.build); `dxil-spirv` is C++14 (`CMAKE_CXX_STANDARD 14`) and its
  converter library links glslang's SPIR-V builder, RenderDoc's bitcode reader and SPIRV-Headers, all
  built from its own tree (licences in section 3). dxil-spirv's own `dxbc-spirv` submodule serves only
  its sandbox tool, so it is not fetched or built and our tree has one copy of dxbc-spirv. Natively the system C++ compiler
  is used. For WebAssembly, clang targets `wasm32-wasip1` with wasi-sdk's libc and libc++, exceptions off
  (dxbc-spirv already builds with `cpp_eh=none`); the shaders module is built for `wasm32-wasip1` and the
  host supplies the few WASI imports it has (listed with `wasm-tools` as part of G6's pass bar; expected:
  `proc_exit` and `fd_write` only). A thin C function per converter is the only boundary: bytes in,
  serialised IR or SPIR-V out. wasi-sdk is added to the flake at G6.

### 6.3 What was tried (scratch: `wcontainers-scratch/graphics`)

Measured on 2026-11-17 on the Mac (load average 12.5 to 21 at the start of the runs; no timings were
taken, only verdicts). Tools: dxbc-spirv at 6196cc6, dxil-spirv at 41815bf, naga 30.0.2, DXC 1.10.2605.37
(nixpkgs), Chrome's WebGPU (Tint) and WebGL 2 for the final verdict. Chrome's verdict is shader
module compilation (`getCompilationInfo` and `COMPILE_STATUS`), not pipeline creation, which needs the
bindings and state a real draw has; G2, G3 and G6 build pipelines. Chrome ran headless with
`--enable-unsafe-webgpu` (`tools/chrome-compile.mjs`); G0's independent check ran the same files without
that flag and got the same counts (111 and 78).

DXBC corpus: 191 real DXBC blobs compiled by Microsoft's compiler, pulled out of Wine's Direct3D 11 test
source (LGPL, used as test data only, kept in scratch). By stage: 137 pixel (4_0, 4_1, 5_0), 19 vertex, 19
compute, 7 geometry, 5 hull, 4 domain.

Route tried: DXBC, dxbc-spirv's own SPIR-V lowering, naga's SPIR-V reader, naga's writers, then Chrome.
This is the route section 6.2 replaces with a direct lowering, so its losses show what the direct
lowering must cover and what the SPIR-V route would cost.
- dxbc-spirv converted 191 of 191 to SPIR-V.
- With dxbc-spirv's default float-control modes, naga read and validated 20 of 191 (132 failed on
  the rounding-mode execution mode). With those modes off (WebGPU has no way to honour them anyway), naga read
  and validated 123 of 191. The 68 failures: 16 geometry, hull and domain (naga has no such stages; we
  lower them, section 7), 19 compute shaders (all of them: their group size comes as
  `ExecutionModeId`), 5 storage images without a format, 4 typed buffers, 2 demote to helper, 1 stencil
  export, 4 other reader gaps (two built-ins, two storage classes), and 17 validation failures.
- WGSL: naga wrote 112; Chrome's WebGPU compiled 111 of them (one constant `f32tof16` overflow). naga's
  writer refused 11 that use `isnan`, which WGSL lacks.
- GLSL ES 3.00: naga wrote 95; Chrome's WebGL 2 compiled 78. The 17 failures are features GLSL ES 3.00
  lacks: bitfield insert, extract and reverse (7), `textureGather` (6), conservative depth (2), sample
  variables (1), multisampled samplers (1). naga refused 28 more: storage buffers (14, absent on WebGL 2),
  `textureQueryLevels` (11), sample counts (2), overrides (1).

DXIL: 8 shaders written for this test (vertex transform, lit pixel shader with `clip`, multiple targets
with depth output, compute reduction with group shared memory, wave operations, storage image, dynamic
resources through `ResourceDescriptorHeap` at SM 6.6, typed buffer with comparison sampling), compiled by
DXC to DXIL. dxil-spirv converted 8 of 8 to SPIR-V; naga read 1 of 8 (the multiple-target shader failed
validation, six failed in the reader for the gaps listed in 6.2); that one compiled in Chrome as WGSL and
GLSL ES 3.00.

What follows for the plan:
- The chosen route, lowering dxbc-spirv's own form straight into naga, was not tried in G0. It is
  expected to remove the reader's failures (they are failures of naga's SPIR-V reader, a step the direct
  route does not have) and to let us emit what WGSL and GLSL ES 3.00 lack in their own terms (6.5); G6
  measures it on the same corpus first, and falls back to the measured SPIR-V route if it does worse.
- DXIL depends on naga's SPIR-V reader growing; that work is G6's first task, measured on the same
  corpus plus real DXIL from games.

### 6.4 One binding rule, the reflection and the conversion API

The shaders crate maps every API's resource slots onto binding numbers the same way for every front,
and records the mapping in the converted shader's `Reflection`. The types are in `wcontainers-gpu`
(`reflect.rs`: `SlotKind`, `Reflection`, `ReflectedBinding`, `StageValue`, `StageExtras`, `HelperValue`,
`HelperSlot`, `VariantKey`, `VariantFlags`), so the shaders crate (G6) and the fronts (G4, G5) build
against the same Rust from the start; back ends never read them.

- Binding numbers within a set: range start of the slot's kind plus the API's slot (`SlotKind`:
  uniform 0 to 15, sampler 16 to 31, texture and typed buffer 32 to 159, storage 160 to 223, the hidden
  counters of storage slots 224 to 287).
- Sets, Direct3D 9 to 11 (slots per stage): set 0 the vertex stage's slots, set 1 the pixel stage's
  (with Direct3D 11's pixel-stage storage views), set 0 for a compute program, set 2 the streams and
  copies the front adds (pulled vertex streams, constant buffers as storage or textures, numbered as
  storage or texture slots past the API's own), set 3 the helper constants (`HELPER_SET`, binding 0).
  Geometry, hull and domain shaders run through the stage helper, which binds them the same way in its
  own programs.
- Sets, Direct3D 12 (slots shared across stages by the root signature): set 0 ranges visible to the
  vertex stage only, set 1 to the pixel stage only, set 2 to all stages, set 3 helpers. Within a set the
  ranges are numbered in root-signature order inside their kind's range; register spaces are kept in
  `ReflectedBinding::space`. The numbering depends on the root signature, so its hash is
  `VariantKey::layout`. Unbounded ranges (bindless) need `RESOURCE_TABLES`; without it the range takes
  the slots left in its kind, and a program needing more is refused (`BindingsBeyondLimit`).
- Helper constants: one uniform block per program, laid out by the shaders crate (`Reflection::helpers`
  gives each `HelperValue` its byte offset); the front writes it with `write_buffer_ring` when a value
  changed and binds it in set 3. The values: storage offset remainders, samplers' level-of-detail bias,
  border colour and state (for filtering and addressing in the program), texture level and sample
  counts where the back end's language cannot ask, depth bias, pulled streams' offset, stride and step,
  the sample mask, packed storage offsets, the texel layout of storage views read through a buffer,
  and how each target combined in the program blends (`HelperValue`). Set 3 also holds the copies of
  targets such programs read, from `TARGET_COPY_BINDING`.
- Variants: `VariantKey` (4.5) is part of the converted shader's cache key. Its per-slot fields carry a
  bit per API slot; the typed buffer route is one field; the sample count, dual-source blending, the
  clip-depth rewrite, the depth flip, depth bias, the provoking rewrite, the sample mask and the
  counting variant are its flags.

The conversion API G6 provides in `wcontainers-shaders`, which G4 and G5 call:

```rust
pub enum Source<'a> { Dxbc(&'a [u8]), Dxil(&'a [u8]), SpirV(&'a [u32]) }
pub struct Converted {
    /// One module per program stage the shader becomes: one for vertex, pixel and compute shaders;
    /// the stage helper's compute and vertex programs for geometry, hull and domain shaders.
    pub modules: Vec<wcontainers_gpu::ShaderModule>,
    pub reflection: wcontainers_gpu::Reflection,
}
pub fn convert(source: Source<'_>, variant: &wcontainers_gpu::VariantKey, root_signature: Option<&[u8]>)
    -> Result<Converted, ConvertError>;  // ConvertError names the reason (6.3's categories)
pub struct Stages<B: Backend> { /* converted vertex, hull, domain, geometry shaders and stream output */ }
impl<B: Backend> Stages<B> {
    /// Draws `call` through the lowered stages (section 7), recording dispatches and draws on `backend`.
    pub fn draw(&mut self, backend: &mut B, call: &DrawCall<'_, B>, outputs: &[BufferRange<&B::Buffer>]);
}
```

Before G6 lands, G4 and G5 test with programs written in WGSL and reflections written by hand, through
the same types.

### 6.5 Language-specific fixes, in each back end

WGSL: `isnan` and `isinf` lowered to bit tests. (Storage formats from the bound view are a variant the
shaders crate makes, 4.5.)
GLSL ES 3.00: bitfield operations lowered to shifts and masks; `textureGather` built from four
`texelFetch` reads at the gather footprint; `textureQueryLevels` and sample counts passed as uniforms;
conservative depth hints dropped (they only allow optimisation). Absent on WebGL 2 and reported so by the
feature level: storage, compute, multisampled reads.

## 7. Geometry, hull and domain stages, and stream output

They never reach a back end. The shaders crate lowers them, and its stage helper issues base calls:

- WebGPU (`COMPUTE`): the vertex shader runs as compute and writes its outputs to a storage buffer; the
  geometry shader runs as compute, one invocation per input primitive, appending vertices and indices;
  a pass-through vertex shader draws them (`GeometryAsCompute`). Hull shader as compute, a tessellator as
  compute (ported from Mesa's MIT code at G6, not a CPU tessellator, so no read back per draw), domain
  shader as the vertex shader of the final draw (`TessellationAsCompute`). Stream output: the vertex or
  geometry compute pass also writes the stream output buffers in Direct3D's layout
  (`StreamOutAsCompute`). Vertex-stage storage writes (Direct3D 11.1 UAVs in every stage) take the same
  route (`VertexStorageAsCompute`).
- WebGL 2 (no compute, `TRANSFORM_FEEDBACK`): stream output is native transform feedback. Geometry
  shaders run by replay: the vertex outputs are captured by transform feedback into a buffer read as a
  texture, then one draw runs the geometry shader once per output vertex slot, each invocation keeping
  only its own emitted vertex and collapsing unused slots (`GeometryAsReplay`). Hull and domain shaders
  are not offered on WebGL 2 (feature level 10_0 does not have them).

The ideas follow Asahi's and the MIT-headed mesa-wink driver's published descriptions; nothing is copied
from gex (AGPL).

## 8. The cache

Decided: our own store; Fossilize is not used.

Fossilize (MIT, read in its repository at 556da9f) records Vulkan create-info structures as JSON in its
`.foz` container and replays them through a C++ interface. Its value to us would be the container alone:
our keys are our own blocks and shader keys, not Vulkan objects, and its entries are keyed by a 64-bit
hash written as ASCII. A container of the same idea is a few hundred lines of Rust; taking C++ into the
browser for it is the larger cost. We keep its good properties: append-only, every entry checked, a cut-off
last entry ignored.

Keys, all 32-byte BLAKE3 hashes of content:
- **Converted shader**: source bytes (DXBC, DXIL, SPIR-V), converter version (our crate version plus the
  dxbc-spirv or dxil-spirv revision), and the variant key (6.4). Value: the common-form module serialised
  with naga's `serialize` feature, plus our reflection. The variant key holds every back-end-dependent
  rewrite (4.5) and `convert` reads nothing else, so the value is the same whichever back end or
  adapter made it; G6 tests this by converting each corpus shader under each back end's keys on both
  and comparing the bytes.
- **State combination**: key: the BLAKE3 hash of the payload. Payload: the block descriptions (blend,
  depth-stencil, raster, vertex layout), the program's shader keys with their variant keys, the target
  layout, the topology class and the layout keys of the bound binding sets (4.5), serialised with
  `postcard` from `serde` derives on the interface's plain-data types (both added at G6, licences
  MIT or Apache-2.0, to be confirmed then). At start-up each recorded payload is decoded, its blocks
  made, and the combination given to `prepare`. The combination is in interface terms, so the record
  works for any front and any back end.

The file: a 16-byte header (magic `WCGPUCACHE`, version), then entries of kind (1 byte), key (32), payload
length (4), CRC-32 (4), payload. Loading builds a hash map from key to offset (indexable). Writing
appends. Merging two files is the union of their entries; equal keys have equal payloads by construction,
and a disagreement is counted and the first kept. Exporting is copying the file. In the browser it lives
in the origin private file system, one file per game, after asking for persistent storage; natively it is
a file. The browser offers no way to keep compiled pipelines, so this is the only route (research:
gpuweb #11052 closed).

Crates for the cache, licences to be confirmed at G6: `blake3` (CC0-1.0 or Apache-2.0), `crc32fast`
(MIT or Apache-2.0).

## 9. Feature levels and what becomes a slow path

The rule: a front reports the highest level whose required features it serves natively or by an exact
slow path, and reports each optional capability it lacks through the API's own query. Requirements were
read in Microsoft's feature level tables (direct3d11/overviews-direct3d-11-devices-downlevel-intro.md)
and format support tables (direct3ddxgi/format-support-for-direct3d-*-hardware.md) on 2026-11-17. Every
format ability those tables require is served natively or by an exact route of 4.12, named below (atomics
on storage textures, 16-bit normalised and rendered signed normalised formats, stencil loads, copies
within typeless groups, read backs of every format). The one shortfall against the tables is
multisample counts per format (4.12), and the owner is told.

What each level needs of a back end's numbers, from Microsoft's table and the SDK headers
(`d3d10.h`, `d3d10_1.h`, `d3d11.h` of SDK 10.0.26100, read on the development desktop), and how each is
met:
- 11_0: 16384 2D and cube textures, 2048 3D extent and array layers: met only by the adapter's limits,
  so they are conditions. 32 input slots and 32 elements: by `VertexPulledInShader` past the limits. 14
  constant buffers of 65536 bytes per stage: by `ConstantsAsStorage` past the limit. 32 registers from
  the vertex to the pixel stage: by packing, past that refused (`VaryingsBeyondLimit`). BC1 to BC7: by
  `BlockDecoded` where not native. 128 textures per stage: refused past the limit
  (`BindingsBeyondLimit`). 8 targets: WebGPU's minimum. Every atomic operation on storage views of
  `R32_UINT` and `R32_SINT`, and exchange on `R32_FLOAT`: by `AtomicsThroughBuffer`.
- 10_1: 8192 2D and cube, 2048 3D, 512 layers: conditions. 32 input slots and elements, 32 registers:
  as 11_0. BC1 to BC5.
- 10_0: 8192 2D and cube, 2048 3D, 512 layers: conditions. 16 input slots and elements, 16 registers:
  GLES 3.0's own minimum (16 attributes, 15 varying vectors plus position). 14 constant buffers of 65536
  bytes: by `ConstantsAsTexture` past WebGL 2's blocks or block size. BC1 to BC5: by `BlockDecoded` where
  the extensions are missing. 8 targets: a condition. Requirements every level shares, and how each is
  met: the 16-bit normalised formats sampled, rendered and blended, and the 8-bit and 16-bit signed
  normalised ones rendered and blended, natively or by `NormAsInteger`; stencil loads natively or by
  `StencilCopiedByDraw`; a blend and write mask per target natively or by `TargetsSplit`; the sample
  mask natively, by `SampleMaskInShader`, or with multisampling reported absent; rendering of the float
  formats, native on WebGPU and a condition on WebGL 2 (EXT_color_buffer_float, 99.94%); blending of
  the 32-bit float formats (required from 10_0) natively with `float32-blendable` (WebGPU) or
  EXT_float_blend (WebGL 2), both present in this Mac's Chrome and of unchecked share, otherwise in the
  program from a copy of the target (`BlendInShader`, exact within Direct3D's blending tolerance, G2
  and G3 compare); read backs of every format natively or from the host copy (`HostCopyKept`).

Direct3D 11:
- **WebGPU: feature level 11_0, when the adapter has 16384-texel 2D textures, 2048 3D extent and 2048
  array layers** (this Mac's Chrome has; WebGPU's defaults do not). An adapter short of these reports
  10_1 if it meets 10_1's sizes, with compute reported present at 10_1
  (`ComputeShaders_Plus_RawAndStructuredBuffers_Via_Shader_4_x` TRUE), otherwise 10_0 by the same
  rule. 11_1 is not reported because it requires logic operations,
  target-independent rasterisation and storage-only drawing with forced sample counts up to 16, none of
  which has an exact route on WebGPU (no logic operations, no sample counts other than 1 and 4, no
  raster sample count apart from the target's). 12_0 and 12_1 need tiled resources and more.
  Reported FALSE through `CheckFeatureSupport` (all optional at 11_0): `OutputMergerLogicOp`,
  `UAVOnlyRenderingForcedSampleCount`, `MultisampleRTVWithForcedSampleCountOne`, double-precision
  shaders, `MapDefaultBuffer`; tiled resources, conservative rasterisation, rasterizer ordered views and
  minimum-maximum filtering are absent at 11_0 anyway. The 11.1 runtime's optional abilities the front
  can serve exactly are reported TRUE (`ClearView` through `clear_rects`, constant buffer offsetting and
  partial updates, discard calls). Counted slow paths: geometry shaders, hull and domain shaders, stream
  output (all section 7), more storage bindings than slots (`StoragePacked`), typed buffers
  (`TexelBufferAsStorage`), border colors (`BorderInShader`), mirror once (`MirrorOnceInShader`), linear
  filtering of `R32_FLOAT` and depth (`FilterInShader`), depth bias on lines (`DepthBiasInShader`),
  sampler bias (`LodBiasInShader`), input slots and constant buffers past the limits
  (`VertexPulledInShader`, `ConstantsAsStorage`), BC formats without the adapter feature
  (`BlockDecoded`),
  wireframe (`FillModeConverted`), formats WebGPU lacks (`FormatConverted`, `VertexFormatConverted`),
  instance step rates, fat targets, layered drawing, exact occlusion counts (4.12). Multisampling: 4
  samples where WebGPU allows, reported per format (4.12). Typed UAV loads of more formats follow
  `FormatCaps` storage read.
- **WebGL 2: feature level 10_0**, when the device has 8 color targets, 8192-texel 2D and cube
  textures, 2048 3D extent and 512 array layers, and EXT_color_buffer_float (GLES 3.0 guarantees only
  2048, 2048, 256 and 256, and no float targets;
  this Mac's Chrome gives 16384, 16384, 2048 and 2048); otherwise 9_3 (4096 2D and cube, 256 3D, 4
  targets), otherwise 9_1. BC1 to BC5, 16 input slots and 14 constant buffers as above. Compute is optional at 10_x and reported absent
  (`ComputeShaders_Plus_RawAndStructuredBuffers_Via_Shader_4_x` FALSE). Multisampling is optional for
  every format at 10_0. Without OES_sample_variables none is reported, because a partial sample mask
  would have no route (4.12). With it, 4 samples are reported for the formats WebGL 2 renderbuffers take,
  with
  multisample load reported absent for every format, so a multisampled texture cannot be made a shader
  resource; the two formats whose 10_0 table requires multisample load (BGRA sRGB, which WebGL 2 stores
  as RGBA) report no multisampling. Stream output is native transform feedback (`StreamOutRepacked`
  where the declaration needs it); geometry shaders take `GeometryAsReplay`; Direct3D clip depth through
  EXT_clip_control or `ClipDepthRewrite` (a listed loss). 10_1 is not reported because it needs cube
  map arrays and 4 samples with multisample load on 38 formats, which WebGL 2 lacks.

Direct3D 12:
- **WebGPU: feature level 11_0** under the same size conditions as Direct3D 11 (otherwise
  `D3D12CreateDevice` fails as on hardware below 11_0), resource binding tier 1 (`RESOURCE_TABLES` absent; 11_1 requires tier
  3 in the Direct3D 12 table), shader model 6.0 (6.6 needs tier 3), wave operations where `SUBGROUPS`.
  Absent: ray tracing, mesh shaders, variable rate shading, sampler feedback, 64-bit integers, tiled
  resources, depth bounds, rasterizer ordered views, conservative rasterisation. Multisampling as for
  Direct3D 11. When a browser offers resource tables, the front reports tier 3 and 11_1.
- **WebGL 2:** not offered; `D3D12CreateDevice` fails as on hardware below 11_0.

## 10. One source, two builds

Each front crate has the front itself (`Device<B>`, `Context<B>` and the objects, plain Rust methods
named after the API's methods) and two thin shells around it:

One library holds the whole Direct3D family, so a process has one copy of the fronts, one back end and
one device thread, and DXGI's swap chain works on the same `B::Texture`s as Direct3D 11's and 12's
devices. That library is built by a crate `wcontainers-directx` (the shells; it depends on both fronts,
the infrastructure crate and both back ends, and is where `B` is chosen):

- **Windows library.** `wcontainers_directx.dll`, built for `x86_64-pc-windows-msvc` as `cdylib`,
  exporting the real entry points (`D3D11CreateDevice`, `D3D12CreateDevice`, `CreateDXGIFactory2`, ...).
  `d3d11.dll`, `d3d12.dll` and `dxgi.dll` are export-forwarding libraries made by the linker from `.def`
  files (no code), each export forwarding to `wcontainers_directx.dll`, so whichever a program loads
  first, all reach the one library. COM objects are made with the `windows` crate's `implement` macro
  (MIT or Apache-2.0, confirmed at G4); each COM method encodes its call into the call buffer (4.8), and
  the one device thread decodes it into the front with `B` = the WebGPU back end on `wgpu`'s native
  Direct3D 12 or Vulkan path. Real games load them from their own folder. Built and run on the
  development desktop through the graphics-rig track's loop.
- **Container modules.** The same crate built for `wasm32-unknown-unknown` once per back end:
  `directx-webgpu.wasm` (DXGI, Direct3D 10, 11 and 12 on WebGPU) and `directx-webgl.wasm` (DXGI,
  Direct3D 10 and 11 on WebGL 2; Direct3D 12 is not offered there). The host loads only the one for the
  API the browser offers, when a program first calls `CreateDXGIFactory*`, `D3D10CreateDevice*`,
  `D3D11CreateDevice*` or `D3D12CreateDevice`. The shaders crate, with its C++, is a module of its own
  (`wasm32-wasip1`), loaded at the first shader. OpenGL, Vulkan and Direct3D 9 will be modules of their
  own. In the container, the guest's stand-in libraries (built in the second batch, run by Wine as guest
  code) encode the same calls into the same buffer format.

Why the family is one module, a departure from the brief's "each piece its own module, loaded when a
program first asks for that API": DXGI hands the same adapter, swap chain and back buffer objects to
Direct3D 10, 11 and 12, which must share one device and its device thread (Direct3D 11 on 12 and shared
resources depend on it). Separate modules for them would have to call each other's back end through
run-time tables, against rule 4, or hold separate devices that cannot share objects. So the unit
loaded on first use is the family, and the unused back end is never loaded. A Direct3D 11 program
therefore loads Direct3D 12's code too; its size and load time are measured at G4 (section 13) and
reported, without changing the rule.

The encoder and decoder of the call buffer are generated from one description of each API's methods,
taken from Microsoft's Windows metadata (MIT), so the Windows shell, the guest stand-in and the decoder
cannot disagree. The channel and the stand-ins are second-batch work; the first batch builds the Windows
shell with the channel in-process, which exercises the same decoder.

## 11. Testing

- **Interface rules:** the recording back end in `wcontainers-gpu-test` checks every rule of section 4
  (pass rules, ordering of writes, destroy deferral, binding numbers) and records a call stream that
  replays identically. A seeded generator makes valid random call sequences (proptest); the case count is
  a knob, and the seed and count of every run are recorded.
- **Back ends:** one shared suite of drawing tests through the interface, run by every back end: exact
  answers where the API promises them (clears, copies, read backs, integer formats, query results),
  pictures within a stated tolerance where it does not, and both back ends compared with each other.
  Natively on the Mac (Metal) and the desktop (RTX 3080), and in Chrome for WebAssembly.
- **Fronts:** Wine's Direct3D 10, 11 and 12 test suites (LGPL, test-only, never a dependency of anything
  shipped) run through our libraries on the desktop and through real Direct3D, verdict for verdict; and
  captured frames of real programs compared pixel by pixel with real Direct3D and with WARP, using the
  graphics-rig track's comparison tools and its GPU job queue.
- **Shaders:** every corpus shader either converts or fails with a named reason; converted shaders are
  run on both back ends and compared with real Direct3D.
- **Speed:** frame time and CPU time per draw on the same scenes through real Direct3D, our fronts on the
  native back end, and Chrome; recorded with the machine and its load.

## 12. Milestones

G0 is this plan and the interface crate. The rest fan out to separate agents in separate checkouts.
Each says what it needs from the others; nothing waits on a later milestone to start.

How the interface changes after G0: `wcontainers-gpu` holds everything the milestones share (the
`Backend` trait, the blocks, `Caps`, the slow paths, and the reflection and variant types of 6.4). A
milestone that needs a change makes it in a commit that touches only `wcontainers-gpu`, with its
message starting "interface:", and lists it in its status file under "Interface changes", so parallel
checkouts merge it without conflict. G1 owns the crate's shared helpers; no other milestone writes
helpers there.

- **G1, the interface's helpers and the recording back end.** Added to `wcontainers-gpu`: the pass
  tracker (4.7), the occlusion counter (4.12; it takes each draw's counting program from its caller) and
  the combination tracker (4.4). `wcontainers-gpu-test`: the recording back end, the seeded random
  call-sequence generator, and the first tests of the shared back-end suite. Pass bar: every rule in
  section 4 is checked by the recording back end and has a test that breaks it; every fast-path default
  is shown to be built from the base by a recorded stream; the occlusion counter gives exact counts on
  the recording back end for scripted draws (with counting programs written in WGSL); the combination
  tracker picks the tuples 4.4 says on scripted sequences; 100,000 random sequences (seeded, count
  recorded) record and replay identically. Needs: nothing.
- **G2, the WebGPU back end.** `wcontainers-gpu-webgpu` on `wgpu`, with `bcdec_rs` for `BlockDecoded`.
  Pass bar: the shared back-end suite passes natively on the Mac and the RTX 3080 and in Chrome;
  pipelines, bind groups and renamed buffers are cached as section 4 says, with counters proving it on a
  scripted scene; layouts derived as 4.5 says, with a test binding a depth view and an `r32float` view
  with point samplers (unfilterable layouts) and, where `float32-filterable` is present, with linear
  ones; BC textures sampled natively and decoded (the back end is made from the `wgpu` device it drives,
  so a test makes one without `texture-compression-bc`), both read back as the blocks written; copies
  between formats of one typeless group exact, including `r32float` into `depth32float`; the formats of
  `texture-formats-tier1` declared only with it, tested on a device made without it; `prepare` measured
  removing first-draw pauses. Needs: G1's
  suite (it may start on G0's crate and write the suite's first tests with G1). Test programs are WGSL.
- **G3, the WebGL 2 back end.** `wcontainers-gpu-webgl` on `glow`. Pass bar: the same suite passes in
  Chrome's WebGL 2, with the absent list of 4.11 declared and every slow path counted; cube, array and
  3D textures made with the GL target of `own_view`; BC and `RGB9E5` textures read back as written;
  stencil sampled with and without WEBGL_stencil_texturing (the back end can be made with an extension
  switched off, for tests), with equal answers; copies between formats of one typeless group exact;
  `generate_mips` exact against G2 for a format `generateMipmap` refuses; per-draw cost measured against
  G2 on the same scene. Needs: G1's suite. Test programs are WGSL, written to GLSL ES by naga.
- **G4, the Direct3D 11 front.** `wcontainers-direct3d-11`, and `wcontainers-directx` with the Windows
  library and forwarders of section 10. Pass bar: Wine's Direct3D 11 tests give real Direct3D's verdicts
  on the RTX 3080 except a written gap list naming the absent feature behind each gap; three real
  programs (one per engine family, chosen at G4 from those on the desktop) draw frames matching real
  Direct3D within the stated tolerance; the building-ahead hit counts of 4.4 and the module size of
  section 10 reported; append buffers, `CopyStructureCount`, overlapping queries, per-target blends on
  WebGL 2 without OES_draw_buffers_indexed and texture atomics each have a test against real Direct3D.
  Needs: G1, G2 (to draw), G6's `convert` (6.4) for real shaders. Before G6 lands it
  works on the recording back end and G2 with WGSL programs and hand-written reflections.
- **G5, the Direct3D 12 front and the shared infrastructure.**
  `wcontainers-directx-graphics-infrastructure` (adapters, outputs, swap chains, presenting, frame
  pacing) and `wcontainers-direct3d-12`. Pass bar: Wine's (vkd3d's) Direct3D 12 tests as G4's bar; a real
  Direct3D 12 program draws matching frames; `CheckFeatureSupport` answers match section 9. Needs: G1,
  G2, G6's `convert` for DXIL (as G4, it starts with WGSL programs). G4 uses the infrastructure crate, so
  G5 builds it first and G4 starts with a stub of its two calls (create a swap chain, present) until it
  lands.
- **G6, shaders and the cache.** `wcontainers-shaders`: dxbc-spirv and dxil-spirv built in (natively and
  for WebAssembly, imports listed), the DXBC lowering into naga (measured first against the SPIR-V
  route, 6.3), naga's SPIR-V reader fixes for DXIL, `convert` with every `VariantKey` field and flag of
  6.4 (including the counting variant, texel buffer routes, storage packing, atomics through a buffer,
  decoded textures, targets written in the program and the sample mask), the counter operations of 5.1,
  the binding rule and reflection of 6.4, the built-in compute
  programs fronts use (texture clears of 5.1), the stage helper of section 7, and the cache of section 8.
  Pass bar: all 191 corpus shaders and a corpus of real game shaders (DXBC and DXIL) convert or fail with
  a named reason, and every converted shader compiles in Chrome for each back end that can run it; each
  variant field has a test showing the rewritten program's result on G2 or G3 equals the unrewritten
  program's with the native feature (or a reference picture); geometry, tessellation and stream output
  tests match real Direct3D on the RTX 3080; the cache file round-trips, merges by union and survives a
  cut last entry; converting under each back end's keys gives the same bytes on both (section 8). Needs: G0 for conversion and variants; G2 (and G3) to run their tests and the stage
  helper's.

Second batch, after these: the call channel and guest stand-ins, OpenGL through Mesa on our interface
(and whether to adopt the MIT-headed mesa-wink driver once its licence is confirmed), Vulkan 1.1 through
Mesa's runtime, sound and input.

## 13. Not yet measured

- Per-draw cost of each back end from WebAssembly, and whether WebGL 2 beats WebGPU on many small draws
  as the JavaScript benchmarks in the research found.
- How many distinct blocks and pipelines real games make per frame and in total.
- How long `prepare` takes per pipeline in Chrome, and how much of first-frame stutter it removes.
- What JSPI suspension costs per read back.
- How often real games use geometry shaders, tessellation, stream output, logic ops, more than 16
  textures per stage, and occlusion counts rather than "any".
- The cost of each slow path in section 5.
- Size of each module (front, back end, shaders with its C++), and its compile time in Chrome.
- Whether dxbc-spirv and dxil-spirv build for WebAssembly without changes.
- How much real DXIL naga's SPIR-V reader accepts once fixed.
- Whether lowering dxbc-spirv's own form straight into naga converts more of the corpus than the
  measured SPIR-V route (expected, 6.3; G6 measures it first).
- What the occlusion counting replay costs per query, and how often a query's draws write depth or
  stencil (which adds a depth copy).
- How many real shaders need `FilterInShader`, `MirrorOnceInShader` or more sampled textures than a back
  end has (`BindingsBeyondLimit`).
- How much `ClipDepthRewrite` changes depth on WebGL 2 without EXT_clip_control.
- The size and load time of each Direct3D family module (section 10), and what carrying Direct3D 12's
  code costs a Direct3D 11 program.
- How many of a Direct3D 11 program's combinations building ahead (4.4, step 2) prepares before their
  first draw, and how many it prepares that are never drawn.
- What `BlockDecoded`, `ConstantsAsTexture`, `ConstantsAsStorage` and `VertexPulledInShader` cost per
  frame, and how often real games bind more constant buffers or vertex streams than WebGPU's limits.
- How many real shaders need more values between stages than WebGPU's adapters give
  (`VaryingsBeyondLimit`), and how many games set a sampler level-of-detail bias.
- How the decoded BC1 to BC5 values compare with the RTX 3080's own decoding.
- What `AtomicsThroughBuffer`, `NormAsInteger`, `BlendInShader` and `StencilCopiedByDraw` cost per frame,
  and how often real games use texture atomics, 16-bit normalised targets and stencil reads.
- How many browsers lack `float32-blendable` and EXT_float_blend (their share was not in the research).
- Whether Windows on an SDR display answers `CheckColorSpaceSupport` as 4.9 expects (G5 dumps it).
- Whether a draw whose sample mask clears bit 0 on a single-sample target leaves Direct3D's occlusion
  counts unchanged (4.12 expects so; G3 compares).

## 14. Where the evidence is

Research: `pyro-parked/wcontainers-scratch/related/` (graphics-design-inputs.json, webgl2-vs-webgpu.json,
vulkan-on-webgpu-inputs.json, games-prior-art.json). Shader experiments: `wcontainers-scratch/graphics/`
(sources of dxbc-spirv, dxil-spirv and Fossilize under `src/`, the corpus and every result file under
`corpus/`, the harness under `experiments/shader-corpus/`, the scripts under `tools/`). Rerun with
`tools/run-corpus.sh` inside the checkout's Nix shell (section 6.3 lists the steps it automates).
