# wcontainers, phase two: graphics

Status: plan written 2026-11-17 from the owner's brief of 2026-11-16 and the verified research in
`pyro-parked/wcontainers-scratch/related/`, and corrected the same day after G0's independent check. The interface crate `wcontainers-gpu` exists; nothing else
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
   memory is the host's own linear memory, so vertex and texture data are read in place.
2. Each piece is its own module, loaded when a program first asks for that API, behind one pluggable
   graphics hook on the host. A host without a screen does not provide the hook.
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
built-in programs of 4.3) adds only MIT or Apache-2.0 crates. `wgpu` and `glow` are confirmed by the
milestones that add them.

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
- `clear_texture(&view, format, ClearValue)`. Default: a pass whose load is `Clear`, ended at once, naming
  only the aspects the format has. WebGPU keeps the default; WebGL 2 overrides it with `clearBuffer*` on
  a bound framebuffer, which skips the pass bookkeeping.
- `clear_rects(&view, format, ClearValue, &[ScissorRect])`: Direct3D 11.1 `ClearView` and Direct3D 12
  clears with rectangles, outside a pass. Default: one pass, a full-screen triangle per rectangle
  scissored to it, with a built-in program writing the value (float, unsigned or signed output by format;
  depth through the fragment depth; stencil by `Replace` with the value as reference). WebGL 2 overrides
  it with scissored `clearBuffer*`.
- `generate_mips(&texture, &TextureDesc, mips, layers)`. Default: for each level and layer (each slice of
  a 3D level), a pass drawing a full-screen triangle that samples the level above through a linear
  sampler, with views of one level in the texture's own format (so sRGB is decoded and encoded). WebGL 2
  overrides it with `generateMipmap` where the format allows.
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
  alpha to coverage, and a logic operation (present only with `LOGIC_OP`).
- **Depth and stencil** (`DepthStencilDesc`): compare, write, both stencil faces, masks, depth bounds.
  The stencil reference is not in the block, because every API changes it per draw.
- **Raster** (`RasterDesc`): fill mode, cull, front face, depth bias, depth clip or clamp, scissor enable,
  line width, provoking vertex, clip depth convention, sample mask.
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

### 4.5 Bindings

A binding set is a block: a list of `BindingEntry` (binding number, stages that read it, and a resource:
uniform buffer range, storage buffer range, texture view, storage texture view, sampler, typed texel
buffer, or a resource table). `create_binding_set` may return a set equal to one the back end already
holds, so fronts call it when their slots changed and otherwise pass the handle they have.

Binding numbers come from the shaders crate's rule (section 6.4), the same for every front: one number
space per set, binding = range start + the API's slot number, with uniform buffers from 0 (16 slots),
samplers from 16 (16), sampled textures and typed buffers from 32 (128), storage buffers and textures
from 160 (64). Sets are grouped by how often fronts change them: per frame, per material, per draw, and
a fourth for helper data (the helper constants of 6.4). A draw's sets are WebGPU's bind groups 0 to 3.

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
lighting, SSAO and soft particles do) cannot be bound on WebGPU at all; the back end makes a variant of
the program that filters that pair itself from `textureLoad` reads with the sampler's state
(`FilterInShader`), and the pair's bit is in the variant key. The rewrites that depend on bound objects
(this one, `BorderInShader`, `MirrorOnceInShader`, storage formats from the bound view) are naga-to-naga
functions in a `rewrite` module of `wcontainers-gpu`, which every back end may call; G6 writes them.

- WebGPU: a set becomes a bind group, cached by content and by the generation of each buffer it names
  (renaming a buffer, 4.6, bumps its generation).
- WebGL 2: a set becomes a list of unit and uniform-block bindings. GLSL ES has combined samplers only,
  so the WebGL 2 shader writer gives each (texture, sampler) pair a unit; the back end binds the texture
  and the sampler object to that unit. More pairs than units is the slow path `SamplerPairsExpanded` where
  the shader can be split, and otherwise the shader fails to build (4.12).
- Storage buffer bindings at any offset (Direct3D's raw and structured views from any first element):
  where the offset is not a multiple of `min_storage_offset_alignment` (256 on WebGPU), the back end binds
  from the aligned offset below and passes the rest in the set's helper constants, which every converted
  program adds to its storage addresses (one add per access; there is no second program).
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
shared helper in `wcontainers-gpu` (added at G1, with the occlusion counter of 4.12) so each implicit
front uses the same one.

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
blend, multisample, resolve, storage read and write, vertex, depth-stencil, copy out). The bits are in the
crate; the values each back end declares (expected, to be confirmed by each back end's milestone against
the adapter):

- WebGPU in Chrome: `COMPUTE`, `VERTEX_STORAGE_READ`, `FRAGMENT_STORAGE_WRITE`, `PROVOKING_VERTEX_FIRST`,
  `DEPTH_CLAMP` (depth-clip-control), `CLIP_DISTANCE` and `DUAL_SOURCE_BLEND` where the adapter has them,
  `INDEPENDENT_BLEND`, `SAMPLE_SHADING`, `MULTISAMPLE_READ`, `TIMESTAMPS`, `INDIRECT`,
  `INDIRECT_FIRST_INSTANCE`, `BASE_VERTEX_INSTANCE`, `TEXTURE_VIEWS`, `VIEW_SWIZZLE` (Chrome 143),
  `SEPARATE_SAMPLERS`, `SHADER_F16`, `SUBGROUPS`, `PRIMITIVE_INDEX`, `BACKGROUND_PREPARE`,
  `CLIP_DEPTH_ZERO_ONE`. Absent: `MIRROR_ONCE`,
  `VERTEX_STORAGE_WRITE`, `TRANSFORM_FEEDBACK`, `READ_BACK_NOW`, `FAN_AND_LOOP`, `POLYGON_MODE`,
  `PROVOKING_VERTEX_LAST`, `CLIP_DEPTH_MINUS_ONE`, `CULL_DISTANCE`, `LOGIC_OP`, `WIDE_LINES`,
  `POINT_SIZE`, `OCCLUSION_COUNT`, `RESTART_SWITCH`, `FORMAT_REINTERPRET`, `SHADER_I64`, `SHADER_F64`,
  `RESOURCE_TABLES`, `PERSISTENT_MAPPING`, `BORDER_COLOR`, `SPARSE`, `DEPTH_BOUNDS`,
  `STENCIL_PER_FACE`. Sample counts 1 and 4 only, and 4 only on the formats WebGPU allows (not
  `r32uint`, `r32sint` or any two- or four-component 32-bit format). Limits are the adapter's highest,
  asked for at device creation; `color_bytes_per_sample` is 32 unless the adapter offers more,
  `buffer_copy_alignment` is 4.
- WebGL 2: `TRANSFORM_FEEDBACK`, `READ_BACK_NOW`, `FAN_AND_LOOP`, `PROVOKING_VERTEX_LAST`,
  `CLIP_DEPTH_MINUS_ONE`, `POINT_SIZE` (up to the device's range), `STENCIL_PER_FACE`, and by extension where present:
  `POLYGON_MODE` (WEBGL_polygon_mode), `PROVOKING_VERTEX_FIRST` (WEBGL_provoking_vertex),
  `CLIP_DISTANCE` and `CULL_DISTANCE` (WEBGL_clip_cull_distance), `DEPTH_CLAMP` (EXT_depth_clamp),
  `DUAL_SOURCE_BLEND` (WEBGL_blend_func_extended), `INDEPENDENT_BLEND` (OES_draw_buffers_indexed),
  `TIMESTAMPS` (EXT_disjoint_timer_query_webgl2), `CLIP_DEPTH_ZERO_ONE` (EXT_clip_control, 93.2% per
  the research), `MIRROR_ONCE` (EXT_texture_mirror_clamp_to_edge). Absent: everything that needs compute
  or storage,
  `INDIRECT`, `BASE_VERTEX_INSTANCE` (the extension is at 0.21%), `TEXTURE_VIEWS`, `SEPARATE_SAMPLERS`,
  `MULTISAMPLE_READ`, `SUBGROUPS`, `LOGIC_OP`, `WIDE_LINES`, `OCCLUSION_COUNT` (GLES 3.0 has only
  any-samples queries), `RESTART_SWITCH`, `BORDER_COLOR`, `DEPTH_BOUNDS`. Multisampling only into
  renderbuffers (4 samples where `MAX_SAMPLES` allows): no format has `MULTISAMPLE` read in programs.
  `buffer_copy_alignment` is 1.

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
     removed, or the built-in one when the draw had none. Blending happens exactly for the samples that
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
  formats. No exact route exists (8 samples from two 4-sample halves runs the pixel shader twice, which
  storage writes would see), and the next level whose tables WebGPU meets is 10_0, which would take
  compute and tessellation from every program.
- **Binding slots beyond the back end's limits.** Direct3D 11 promises 128 textures per stage; WebGPU's
  adapters give 16 or more, WebGL 2 usually 16 to 32 units. Storage bindings beyond the limit are packed
  (`StoragePacked`). Sampled textures beyond it have no exact route: creating the shader fails with
  `E_INVALIDARG` (`BindingsBeyondLimit`, a counted refusal, and a known gap). The corpus at G6 measures
  how many real shaders this hits.
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
  each; the front draws twice, culling one face each time (`StencilFacesSplit`). Exact.
- Depth bounds test (optional in Direct3D 12 and Vulkan): absent in both, reported absent.
- Logic operations: optional at Direct3D 11_0 and reported absent there. OpenGL's `glLogicOp` (core)
  runs in the program reading a copy of the target taken before the draw (`LogicOpInShader`); a draw
  whose primitives may overlap is drawn one primitive at a time with a copy between
  (`LogicOpPerPrimitive`). Exact, very slow, and rare.
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
made per vertex/pixel pair at first use. `CreateBlendState`, `CreateDepthStencilState`,
`CreateRasterizerState` call the block constructors; `CreateInputLayout` keeps the layout, and the vertex
layout block is made per (layout, strides).

A frame: `OMSetRenderTargets` marks the pass tracker; `ClearRenderTargetView` becomes the next pass's
`Clear` load, or `clear_texture` if a pass is open on other targets. `IASetVertexBuffers`,
`IASetIndexBuffer`, `*SetShaderResources`, `*SetConstantBuffers`, `*SetSamplers` update the context's
slots and mark binding sets dirty; at the draw the front makes the sets it needs (one per frequency group:
constant buffers, textures and samplers, per stage). `Map(WRITE_DISCARD)` on a constant buffer returns a
fresh shadow block; `Unmap` is `write_buffer(Discard)` (4.6). `UpdateSubresource` is
`write_buffer(Ordered)` or `write_texture` of the stand-in's copy. `ClearView` is `clear_rects`,
`GenerateMips` is `generate_mips`, `ClearUnorderedAccessView*` on buffers is `clear_buffer` with the
element's bits. `Draw*` passes the context's `DrawCall`; `DrawIndexedInstancedIndirect` uses an
indirect range. `CopyResource` and `CopySubresourceRegion` close the pass and copy. `Dispatch` closes the
pass and calls `dispatch`. `Map(READ)` on a staging texture calls `read_back_now` (or polls with
`DO_NOT_WAIT`). `GetData` on a query polls `read_state`; occlusion counts come from the counting replay
and other queries from the front's own counts (4.12). `Present` goes to the DXGI layer (4.9).

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
are finished into guest memory when their fence completes. Placed resources each get their own storage:
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
may overlap (`LogicOpPerPrimitive`). `GL_SAMPLES_PASSED`: `OcclusionCountReplay`. `glPolygonMode`: native with
WEBGL_polygon_mode, otherwise `FillModeConverted`. Clip depth from minus one: native on WebGL 2,
`ClipDepthRewrite` on WebGPU. Multisample textures (3.2): WebGPU has them; WebGL 2 does not, so Mesa
reports OpenGL 3.1 there (3.1 needs texture buffers: `TexelBufferAsTexture`). Wide lines report a range of
1 to 1, which core profile allows. Border colors: `BorderInShader` on both.

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
Secondary command buffers replay. Reported absent through Vulkan's features: `logicOp`,
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
`BorderInShader`. Depth bias on lines: `LineBiasInShader` on both. `GetRenderTargetData` is
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
- Fans and loops: default conversion. Restart off or another index: front's `RestartRewrite`. Provoking
  last: `ProvokingRewrite` (flat inputs read the last vertex through a rewritten index order). Depth bias
  on lines and points (WebGPU requires zero): `LineBiasInShader`. Polygon mode: `FillModeConverted`.
  Multiple viewports and layers (only reachable from geometry shaders): `MultiViewportSplit`,
  `LayerSplit`. Typed buffers: storage buffers with the format decoded in the shader
  (`TexelBufferAsStorage`). Sampler border colors: `BorderInShader`; mirror once: `MirrorOnceInShader`;
  linear filtering of unfilterable formats: `FilterInShader`. Unaligned buffer writes, copies and clears:
  `UnalignedBufferEdge`; storage bindings at unaligned offsets: rebased (4.5). Passes without targets:
  the back end's own attachment. Exact occlusion counts: `OcclusionCountReplay`. Instance step rates:
  `InstanceRateInShader`. Fat targets: `TargetsSplit`. Formats WebGPU lacks (three-component 8 and 16-bit vertex formats, B5G6R5 and other
  16-bit packed formats, A8): converted on upload (`VertexFormatConverted`, `FormatConverted`).
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
- Absent and reported as such: compute and storage (so Direct3D 11 reports feature level 10_0 and
  Direct3D 12 is not offered), texture views (fronts make views by copying: `ReinterpretCopy`),
  multisample textures read in shaders (reported per format through `CheckFormatSupport`), subgroups,
  logic ops, wide lines. BGRA textures are stored as RGBA, swizzled on upload (`FormatConverted`).

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

### 6.4 One binding rule

The shaders crate maps every API's resource slots onto the binding numbers of 4.5 the same way for
every front, and records the mapping in the converted shader's reflection (inputs, outputs, bindings by
register, stage extras). Binding numbers are fixed per kind (4.5): uniform buffers from 0, samplers
from 16, sampled textures and typed buffers from 32, storage from 160, in four sets by update frequency;
the fourth set holds the helper constants (storage offsets rebased by 4.5, and what a variant needs at
run time, such as texture level counts on GLSL ES 3.00). Typed UAVs whose format the shader does not
state get the format from the bound view: the format is part of the shader's variant key, as are the
sample count, dual-source blending, the clip-depth convention, and the per-binding bits of the
rewrites that depend on bound objects (filtering in the program, border colour, mirror once) and the
counting variant of 4.12.

### 6.5 Language-specific fixes, in each back end

WGSL: `isnan` and `isinf` lowered to bit tests; storage textures given their format from the variant key.
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
  with naga's `serialize` feature, plus our reflection.
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
and format support tables (direct3ddxgi/format-support-for-direct3d-*-hardware.md) on 2026-11-17. The
one shortfall against those tables, multisample counts per format, is 4.12's and the owner is told.

Direct3D 11:
- **WebGPU: feature level 11_0.** 11_1 is not reported because it requires logic operations,
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
  filtering of `R32_FLOAT` and depth (`FilterInShader`), depth bias on lines (`LineBiasInShader`),
  wireframe (`FillModeConverted`), formats WebGPU lacks (`FormatConverted`, `VertexFormatConverted`),
  instance step rates, fat targets, layered drawing, exact occlusion counts (4.12). Multisampling: 4
  samples where WebGPU allows, reported per format (4.12). Typed UAV loads of more formats follow
  `FormatCaps` storage read.
- **WebGL 2: feature level 10_0**, when the device has 8 color targets and 8192-texel textures
  (otherwise 9_3). Compute is optional at 10_x and reported absent
  (`ComputeShaders_Plus_RawAndStructuredBuffers_Via_Shader_4_x` FALSE). Multisampling is optional for
  every format at 10_0: 4 samples are reported for the formats WebGL 2 renderbuffers take, with
  multisample load reported absent for every format, so a multisampled texture cannot be made a shader
  resource; the two formats whose 10_0 table requires multisample load (BGRA sRGB, which WebGL 2 stores
  as RGBA) report no multisampling. Stream output is native transform feedback (`StreamOutRepacked`
  where the declaration needs it); geometry shaders take `GeometryAsReplay`; Direct3D clip depth through
  EXT_clip_control or `ClipDepthRewrite` (a listed loss). 10_1 is not reported because it needs cube
  map arrays and 4 samples with multisample load on 38 formats, which WebGL 2 lacks.

Direct3D 12:
- **WebGPU: feature level 11_0**, resource binding tier 1 (`RESOURCE_TABLES` absent; 11_1 requires tier
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
- **Container module.** The same crate built for `wasm32-unknown-unknown` (or `wasm32-wasip1` where it
  links the shaders module): one module with an entry per back end ("run this call buffer on WebGPU", "on
  WebGL 2"), each the family monomorphised for that back end; the host calls the one for the API the
  browser offers. In the container, the guest's stand-in libraries (built in the second batch, run by
  Wine as guest code) encode the same calls into the same buffer format. Whether shipping both back ends
  in one module costs load time worth splitting it is measured at G4.

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

- **G1, the interface complete and the recording back end.** The shared helpers for implicit fronts
  added to `wcontainers-gpu`: the pass tracker (4.7) and the occlusion counter (4.12); every fast path
  already has its default (G0); `wcontainers-gpu-test` with the recording back end and the seeded random
  call-sequence generator. Pass bar: every rule in section 4 is checked by the recording back
  end and has a test that breaks it; every fast-path default is shown to be built from the base by a
  recorded stream; 100,000 random sequences (seeded, count recorded) record and replay identically. Needs:
  nothing. Interface changes after G1 are made only here and announced in the status file, because every
  other milestone builds on it.
- **G2, the WebGPU back end.** `wcontainers-gpu-webgpu` on `wgpu`. Pass bar: the shared back-end suite
  passes natively on the Mac and the RTX 3080 and in Chrome; pipelines, bind groups and renamed buffers
  are cached as section 4 says, with counters proving it on a scripted scene; layouts derived as 4.5 says,
  with a test binding a depth view and an `r32float` view with linear and point samplers; `prepare`
  measured removing first-draw pauses. Needs: G1's suite (it may start on G0's crate and write the suite's first tests with
  G1).
- **G3, the WebGL 2 back end.** `wcontainers-gpu-webgl` on `glow`. Pass bar: the same suite passes in
  Chrome's WebGL 2, with the absent list of 4.11 declared and every slow path counted; per-draw cost
  measured against G2 on the same scene. Needs: G1's suite.
- **G4, the Direct3D 11 front.** `wcontainers-direct3d-11`, and `wcontainers-directx` with the Windows
  library and forwarders of section 10. Pass bar: Wine's
  Direct3D 11 tests give real Direct3D's verdicts on the RTX 3080 except a written gap list naming the
  absent feature behind each gap; three real programs (one per engine family, chosen at G4 from those on the desktop)
  draw frames matching real Direct3D within the stated tolerance. Needs: G1, G2 (to draw), G6's
  conversion API (it may start on G0 and the recording back end).
- **G5, the Direct3D 12 front and the shared infrastructure.**
  `wcontainers-directx-graphics-infrastructure` (adapters, outputs, swap chains, presenting, frame
  pacing) and `wcontainers-direct3d-12`. Pass bar: Wine's (vkd3d's) Direct3D 12 tests as G4's bar; a real
  Direct3D 12 program draws matching frames; `CheckFeatureSupport` answers match section 9. Needs: G1,
  G2, G6's DXIL path. G4 uses the infrastructure crate, so G5 builds it first and G4 starts with a stub
  of its two calls (create a swap chain, present) until it lands.
- **G6, shaders and the cache.** `wcontainers-shaders`: dxbc-spirv and dxil-spirv built in (natively and
  for WebAssembly, imports listed), the DXBC lowering into naga (measured first against the SPIR-V
  route, 6.3), naga's SPIR-V reader fixes for DXIL, the stage helper of section 7, the counting variant
  of 4.12, the cache of section 8, and the `rewrite` module of `wcontainers-gpu` (4.5) that back ends
  call. Pass bar: all 191 corpus shaders and a corpus
  of real game shaders (DXBC and DXIL) convert or fail with a named reason, and every converted shader
  compiles in Chrome for each back end that can run it; geometry, tessellation and stream output tests
  match real Direct3D on the RTX 3080; the cache file round-trips, merges by union and survives a cut
  last entry. Needs: G0 only for conversion; G2 for the stage helper's tests.

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
- The size and load time of one container module holding the Direct3D family for both back ends.

## 14. Where the evidence is

Research: `pyro-parked/wcontainers-scratch/related/` (graphics-design-inputs.json, webgl2-vs-webgpu.json,
vulkan-on-webgpu-inputs.json, games-prior-art.json). Shader experiments: `wcontainers-scratch/graphics/`
(sources of dxbc-spirv, dxil-spirv and Fossilize under `src/`, the corpus and every result file under
`corpus/`, the harness under `experiments/shader-corpus/`, the scripts under `tools/`). Rerun with
`tools/run-corpus.sh` inside the checkout's Nix shell (section 6.3 lists the steps it automates).
