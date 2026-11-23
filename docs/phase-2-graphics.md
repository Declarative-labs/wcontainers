# wcontainers, phase two: graphics

Status: plan written 2026-11-17 from the owner's brief of 2026-11-16 and the verified research in
`pyro-parked/wcontainers-scratch/related/`, and corrected the same day after each of G0's independent
checks. The first batch (sections 1 to 12) is being built by six tracks; section 15, added 2026-11-18,
plans the second batch.

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

- `wcontainers-gpu`: the interface (section 4). Depends on `naga` (the common form), `bitflags`,
  `blake3` (content keys of the built-in programs, as of every key, section 8), `half` (the
  occlusion counter's `r16float` sums) and `serde` (derives on the variant key, the reflection and the
  state blocks, which the cache records, section 8) only.
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
glslang's SPIR-V builder (`third_party/glslang-spirv`, BSD-3-Clause, LunarG and Google), the LLVM
bitcode reader (`bc`, MIT, Hans-Kristian Arntzen for Valve, on RenderDoc's `third_party/bc-decoder`, MIT,
Baldur Karlsson) and SPIRV-Headers. Its own
`dxbc-spirv` submodule is used only by a sandbox tool and is not built. Nothing in either is copyleft.
Fossilize MIT (evaluated in section 8, not used). `naga`'s WGSL reader (feature `wgsl-in`, for the
built-in programs of 4.3) adds only MIT or Apache-2.0 crates. `bcdec_rs` 0.2.1, the BC decoder G2 and
G3 add for `BlockDecoded`: MIT (its `Cargo.toml`, and the LICENSE of its repository
ScanMountGoat/image_dds, "Copyright (c) 2023 SMG"), a port of iOrange's bcdec (MIT or Unlicense, its
LICENSE), `no_std`, read on 2026-11-17. `blake3` 1.8.7: CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH
LLVM-exception (its repository's `Cargo.toml` and `LICENSE_CC0`), with `arrayvec`, `cfg-if`, `cc`
(MIT OR Apache-2.0) and `constant_time_eq` (CC0-1.0 OR MIT-0 OR Apache-2.0), read on 2026-11-17.
`serde` 1.0.229 with `serde_core` and `serde_derive` (and their `proc-macro2`, `quote`, `syn`): MIT OR
Apache-2.0 (each crate's `Cargo.toml`, read on 2026-11-17).
`glow` 0.18: MIT OR Apache-2.0 OR Zlib (its repository's `Cargo.toml`; the brief says MIT, which is
one of the three); `khronos-egl` 6.0: MIT or Apache-2.0 (its repository); ANGLE, a desktop test tool
only (section 10): BSD-3-Clause (its `LICENSE`). `wgpu` is confirmed by the milestone that adds it.

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
  A multisampled texture is 2D with one level and one layer, never a storage texture, and its bytes
  move only by draws, resolves and whole-subresource copies between equal sample counts (WebGPU's
  rules; a front holds a multisampled array, such as Direct3D's `Texture2DMSArray`, as one texture per
  layer, which converted programs read through `VariantKey::texture_layers`). A cube texture is 2D and square, its layers a multiple of six; a texture has from one level to
  its full chain, `floor(log2(largest side)) + 1` (a 3D texture's depth counts). A view's dimension fits
  its texture and layers: `D2` one layer and `D2Array` any of a 2D texture, `Cube` six and `CubeArray`
  a multiple of six layers of a cube texture, `D3` a 3D texture (WebGPU's view rules, and WebGL 2's and
  Direct3D's, whose cube kind is fixed when the texture is made). Samplers have no level-of-detail bias,
  because neither browser API has one (4.12).
- `create_shader(&ShaderModule)`: a validated `naga::Module` with its entry point, stage and content key.
- `create_program(&Program)`: vertex plus optional fragment shader, or one compute shader. Every
  location the fragment shader reads, the vertex shader writes, with the same type, interpolation and
  sampling (WebGPU's and GLSL ES's linking rule, unstated ones taken as WGSL's defaults: a float
  perspective at the centre, a flat value from the first vertex); it may write more.
- The state blocks: `create_blend`, `create_depth_stencil`, `create_raster`, `create_vertex_layout`.
- `create_binding_set(&[BindingEntry])`: the resources a draw binds, as one block (4.5).
- `create_query_set(kind, count)`: occlusion queries that say "any" (every back end), exact occlusion
  counts (`OCCLUSION_COUNT`), or timestamps (`TIMESTAMPS`).
- Buffer usages combine freely, as Direct3D 11's bind flags and the untyped buffers of Direct3D 12,
  OpenGL and Vulkan allow. WebGL 2 forbids one buffer holding both indices and other data (its
  specification's "Buffer Object Binding" and "Copying Buffers"); its back end handles that itself (4.6),
  and the fast-path defaults keep indices in a ring of their own.
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
  `Discard`), resolve targets, and the occlusion query set. Every attachment has the same size (4.7) and sample
  count; a resolve target is single-sampled, in its attachment's format.
- `draw(&DrawCall)`: inside a pass. Section 4.4.
- `begin_query(index)`, `end_query()`: occlusion, inside a pass.
- `copy_buffer`, `copy_texture`, `copy_buffer_to_texture`, `copy_texture_to_buffer`, `clear_buffer`
  (a pattern of 1, 2, 4, 8 or 16 bytes repeated over a range of whole patterns: zero, or an element's
  bits for Direct3D's storage view clears;
  WebGPU clears only to zero, so its back end fills with any other pattern by copies from a block of
  the pattern, `PatternClearCopied`), `resolve`: outside a pass. A buffer copy or read back of a format
  with depth and stencil moves the aspect its region names (the stencil byte, or depth in four bytes);
  `copy_texture` between two such textures moves both aspects in one call, as Direct3D copies it and
  as WebGPU requires, and a front whose API copies one plane (Direct3D 12's plane slices, Vulkan's
  aspects) goes through a buffer. `resolve` names the format the samples are averaged in, the source's
  stored format or its sRGB twin, because Direct3D 11's and 12's `ResolveSubresource` take one: a
  multisampled `R8G8B8A8_TYPELESS` target (one texture seen through sRGB and linear views) resolves in
  linear light as `UNORM_SRGB` and on the stored values as `UNORM`. WebGPU resolves natively through
  views of both textures in that format (the twin is a view every back end makes), and where its
  resolve averages the stored values instead (Chrome's), by a draw loading every sample through the
  twin (`ResolveByDraw`); WebGL 2's
  `blitFramebuffer` decodes and encodes by the renderbuffer's own format, and its multisampled
  renderbuffers can be read no other way, so a resolve in the other twin averages the stored values
  there (`ResolveInStoredFormat`, a listed loss, 4.12). A copy between a buffer and a texture takes
  any row pitch that is a whole number of blocks (Vulkan's tightly packed `bufferRowLength` 0, OpenGL's
  pixel buffer uploads); WebGPU needs multiples of 256, so its back end copies such rows through a
  staging buffer at its own pitch, one buffer copy per row (`RowsRepacked`); Direct3D 12's own 256-byte
  pitch rule never meets it. The buffer offset is a whole number of blocks too, as Vulkan and WebGPU
  require (a block of the aspect moved: one byte of stencil, four of depth); WebGPU's multiple of 4
  for depth and stencil goes the same `RowsRepacked` way. WebGPU cannot copy a buffer into `depth24plus` or `depth32float`: its back
  end copies the buffer into a colour texture and draws the depth from it (`DepthWrittenByDraw`): for
  `depth32float` an `r32float` texture whose texels are the depth; for `depth24plus` an `r32uint`
  texture, since the bytes a front hands over for it are 24-bit unorm depth in the low bits of a 32-bit
  word (Direct3D's `D24_UNORM_S8_UINT` depth, Vulkan's D24 depth aspect, GL's `UNSIGNED_INT_24_8`), and
  the draw writes `(v & 0xffffff) / 16777215`.
  WebGPU copies depth and stencil only as whole subresources, so its back end writes part of a stencil
  (from a buffer through an `r8uint` texture, or from a texture) by one draw per bit with that bit's
  write mask (`StencilWrittenByDraw`), and reads part of either aspect, or 24-bit depth at all, into a
  buffer by drawing it into a colour texture first (`DepthReadByDraw`).
  Buffer ranges are byte-granular in every call; a back end whose API copies
  in larger granules (`Limits::buffer_copy_alignment`, 4 on WebGPU for `writeBuffer`,
  `copyBufferToBuffer` and `clearBuffer`) does the aligned middle natively and merges the edge words with
  a small program (`UnalignedBufferEdge`). `copy_buffer` may copy between non-overlapping ranges of one
  buffer, as OpenGL's `glCopyBufferSubData`, Vulkan's `vkCmdCopyBuffer` and Direct3D 12's
  `CopyBufferRegion` allow (Direct3D 12's buffers are untyped ranges that engines sub-allocate, and its
  documentation sets no rule against one buffer; Direct3D 11's `CopySubresourceRegion` and Direct3D 12's
  `CopyTextureRegion` need different subresources); WebGPU forbids a buffer as both source and
  destination, so its back end copies through a temporary buffer (`CopyWithinBuffer`). WebGL 2 copies
  within one buffer natively. Likewise `copy_texture` may copy between non-overlapping regions of one
  subresource (`vkCmdCopyImage`, `glCopyImageSubData`), in every format, depth and stencil too; WebGPU
  refuses one subresource as both, so its back end copies through a buffer, and a region of a depth
  or stencil format it cannot copy into from a buffer (the depth of `depth32float`, `depth24plus` and
  the combined formats; `depth16unorm` and stencil it can) through a temporary texture of the same
  format by the routes above, the copy in and the copy out (`CopyWithinTexture`). Direct3D's copies of depth
  and stencil take whole subresources, so only the OpenGL and Vulkan fronts will ask for this.
- `submit() -> Fence`, `fence_done(&fence)`, `wait_fence(&fence)`.

Bytes out:
- `read_back(&ReadSource) -> Readback` (a buffer range, or a texture region whose rows arrive tightly
  packed), `read_queries(&set, first, count) -> Readback`, `read_state(&readback)`, `wait_read(&readback)`,
  `finish_read(readback, &mut [u8])` which copies the answer into guest memory. Query results written
  into a buffer on the GPU are the fast path `resolve_queries` (4.3).

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
  triangle list (fan) or a line list (loop) in the ring of `write_buffer_ring` (one write, no buffer
  made per draw), honouring primitive restart in guest indices, and draw it with restart off (counted
  `FanConverted` or `LoopConverted`).
  Each fan triangle keeps the fan's winding and is ordered so the vertex the API's convention names comes
  first for `First` (vertex i + 1, OpenGL's first-vertex rule) and last for `Last` (vertex i + 2); a loop's
  segments are (v_i, v_i+1) and the closing (v_n, v_0), which gives both conventions. Direct3D 9's fan
  rule is checked against the reference at its milestone. WebGL 2 overrides it with `TRIANGLE_FAN` and
  `LINE_LOOP`. Index values come from guest memory or are sequential, 8, 16 or 32 bits wide; a front
  whose index data lives only on the GPU keeps a copy in host memory (the OpenGL front does, as Mesa's
  index rewriting does). The list is built in a buffer the default keeps (`Helpers`), so no draw allocates
  once it has grown.
- `prepare(&PrepareDesc)`: a combination of blocks, target formats and topology to build ahead of its
  first draw. Default: nothing (the first draw builds it). WebGPU overrides it with
  `createRenderPipelineAsync`. It names no index format: Direct3D 11, OpenGL and Vulkan bind the index
  buffer at the draw, not with the state, so no front knows it when it prepares. A back end whose
  pipeline holds the index format of strips (WebGPU's `stripIndexFormat`) prepares a strip topology for
  both formats.
- `clear_texture(&texture, &TextureDesc, format, mips, layers, ClearValue)`: levels and layers (depth
  slices of a 3D texture) seen in the texture's format or its sRGB twin. Default: for each level and
  layer or slice, a view to draw into of that level and layer (which every back end makes, 4.2) and a
  pass whose load is `Clear`, ended at once, naming only the aspects the format has. WebGPU keeps the
  default; WebGL 2 overrides it with `clearBuffer*` on each attachment, which skips the pass
  bookkeeping.
- `clear_rects(&view, format, ClearValue, &[ScissorRect])`: Direct3D 11.1 `ClearView` and Direct3D 12
  clears with rectangles, outside a pass; rectangles are clipped to the view (the default's scissors are
  clipped as every draw's is, 4.4, and WebGL 2's scissored clears clip). Default: one pass, a
  full-screen triangle per rectangle scissored to it, with a built-in program writing the value (float,
  unsigned or signed output by format;
  depth through the fragment depth; stencil by `Replace` with the value as reference). WebGL 2 overrides
  it with scissored `clearBuffer*`.
- `generate_mips(&texture, &TextureDesc, values, mips, layers)`. `values` is the format whose values the
  texels hold: the texture's own, or for a texture a front stores by `NormAsInteger` (4.12) the
  normalised format it stands for, or the sRGB twin of the texture's format (Direct3D's `GenerateMips`
  through an sRGB view of a typeless texture, which its mipmap generation requirement covers for
  `R8G8B8A8_UNORM_SRGB` and `B8G8R8A8_UNORM_SRGB`; the default then reads and writes through views in
  the twin, so filtering is in linear light). Default: for each level and layer (each slice of a 3D level), a pass
  drawing a full-screen triangle that reads the level above in the texture's own format (so sRGB is
  decoded and encoded), by one of three built-in programs:
  - the format has `FILTER`: one sample through the linear sampler;
  - a float format without `FILTER` (`r32float`, `rg32float`, `rgba32float` without `float32-filterable`,
    about 9% of WebGPU adapters per the research; the same on WebGL 2 without
    OES_texture_float_linear): four samples (eight in 3D) at the texel centres through the nearest
    sampler, which every back end allows on unfilterable formats, blended in the program with the
    weights a linear filter gives at the centre of the texel written (`FilterInShader`);
  - an integer twin of a normalised format: four (eight) integer loads, decoded by Direct3D's rules
    (value over 65535, 32767 or 127, signed values clamped at -1), blended the same way, encoded back by
    rounding to nearest even, and written through an integer output (`FilterInShader`).
  It reads the level above through a view of all layers in the texture's own dimension (2D array, cube,
  cube array), the layer or cube face chosen in the program, and draws into a view of one level and one
  layer, so it needs no `TEXTURE_VIEWS`. One exception: neither WGSL nor GLSL ES loads from cubes, so
  an integer cube is read through a 2D array view, or without `TEXTURE_VIEWS` (WebGL 2) from a copy of
  the level above into a 2D array texture (`ReinterpretCopy`). In-program filtering clamps at cube face
  edges where the hardware's seamless filter reads the next face; this differs only for cubes of odd
  size, and Direct3D does not define GenerateMips' filter. WebGL 2 overrides it with `generateMipmap`
  where the format is colour-renderable and filterable and `values` is the texture's own format, and
  keeps the default otherwise, calling `generate_mips_by_draws`, the default's body as a public
  function.
- `write_buffer_ring(RingKind, &[&[u8]], align, &mut offsets) -> Buffer`: a place for the data one draw
  needs (Direct3D 9 user pointers, OpenGL client arrays, converted fans, widened 8-bit indices, spilled
  immediates, helper constants), written in one call and valid for commands recorded before the next
  call into the same ring. Two rings: `Indices` (usage `INDEX`) and `Data` (`VERTEX` and `UNIFORM`), so
  no ring buffer ever holds both indices and other data, which WebGL 2 forbids; `DrawIndexedPrimitiveUP`
  is two calls, one per ring. Default: a ring buffer per kind (4 MiB, grown to fit) written with
  `NoOverwrite`, and with `Discard` when it wraps, so nothing waits. No back end overrides it today; a
  native back end would map the rings.
- `resolve_queries(&set, first, count, &dst, dst_offset)`: query results written into a buffer on the
  GPU, one u64 each at an offset that is a multiple of 8 (refused otherwise, `GpuError::Invalid`),
  ordered with the work around it (Direct3D 12's `ResolveQueryData`, the only way it
  gives query results; Vulkan's `vkCmdCopyQueryPoolResults`; OpenGL's query buffer objects). Default:
  `read_queries`, a wait on the device thread (counted `QueryResolveWait`), then an `Ordered`
  `write_buffer` of the results. WebGPU overrides it with `resolveQuerySet` into a buffer of its own
  (`QUERY_RESOLVE` usage, 256-byte aligned offsets) and a `copyBufferToBuffer` into `dst`, so nothing
  waits. WebGL 2 keeps the default: GLES 3.0 gives query results only to the host.

The built-in programs are WGSL read through naga's WGSL reader (`wcontainers-gpu/src/helpers.rs`), so
every back end gets them in the common form like any other shader. Each program is its own module,
holding only its own bindings and types and validated with only the abilities it needs, and is made when
a default first draws with it. That matters because naga's GLSL writer asks for every type a module
declares, not only those its entry point uses: one module holding a cube-array texture could not be
written for WebGL 2 at all. WebGL 2 never makes the cube-array programs (it has no cube arrays). The
crate's test writes every built-in program as WGSL and, all but the two cube-array ones, as GLSL ES
3.00 for WebGL 2; Chrome compiled all of them (6.3).

Present or absent, with no cheap default:
- `dispatch(&DispatchCall)`: compute (`COMPUTE`), outside a pass. WebGPU has it; WebGL 2 does not. Each
  dispatch is one usage scope (a buffer it binds as writable storage it binds no other way), and its
  group counts, workgroup size and workgroup variables stay within the compute limits.
- `capture_vertices(&call, targets, rasterize)`: transform feedback (`TRANSFORM_FEEDBACK`). With
  `rasterize` false it runs outside a pass and draws nothing (rasterizer discard); with it true it runs
  inside a pass and also draws the primitives in the same run of the vertex work, as Direct3D's
  `RasterizedStream` naming the captured stream and OpenGL's transform feedback without discard do.
  It writes the vertex stage's location outputs in location order as packed 32-bit components (no
  built-ins; the program has at least one, all of 32-bit components): with one target interleaved,
  vertex after vertex, at most `Limits::capture_components` components a vertex (WebGL 2's
  `MAX_TRANSFORM_FEEDBACK_INTERLEAVED_COMPONENTS`, 64 or more); with more, output n into target n
  (OpenGL's interleaved and separate modes), once per vertex of each primitive, strips as the lists
  they draw (odd triangles `(i, i + 2, i + 1)`, or `(i + 1, i, i + 2)` with the last vertex
  provoking). At most `Limits::capture_buffers` targets, each a range with `BufferUsage::CAPTURE`
  that holds every vertex the draw makes and that the call uses no other way (WebGL 2 refuses a buffer
  bound for capture and elsewhere at once).
  WebGL 2 has both natively; WebGPU has neither, and the shaders crate's stage helper builds stream
  output from compute instead (section 7), whose draw of the captured vertices is the rasterised part.
- `create_resource_table`, `write_resource_table`: big resource tables (`RESOURCE_TABLES`). Absent in both
  browsers today (Chrome has a sampling table only behind its unsafe flag).
- `write_timestamp`: GPU timestamps (`TIMESTAMPS`), outside a pass, into a timestamp query set. WebGPU writes timestamps only where
  passes begin and end, so its back end attaches the write to the next pass (or an empty one); a front
  asked for a timestamp between draws ends its pass first (`TimestampSplitPass`).
- `timestamps_disjoint() -> bool` (`TIMESTAMPS`): whether timestamps written since the last call are
  meaningless, clearing the condition. WebGL 2 answers from `GPU_DISJOINT_EXT`, which has exactly that
  meaning; WebGPU's API has no such signal and its back end answers false. Direct3D's
  `TIMESTAMP_DISJOINT` query asks it at its end, and OpenGL's `GL_GPU_DISJOINT` asks it directly.

Why fans have a fast path but geometry shaders do not: fans are one draw turned into another draw, so a
default made from the base exists and is cheap. Geometry, hull and domain stages need converted shaders
and several dispatches; that conversion is shader work, so it lives in the shaders crate and reaches
back ends only as ordinary `dispatch` and `draw` calls. A native back end with real geometry shaders
would add a fast path then, not before.

### 4.4 Drawing state as immutable blocks

A draw names (`DrawCall`): a program, a blend block, a depth-stencil block, a raster block, a vertex
layout block, up to four binding sets, the vertex streams and index buffer (buffer plus offset, plain data),
the topology, whether strips restart, one viewport and at most one scissor (one where the raster block
scissors, clipped to the pass's attachments, which a back end whose API refuses a scissor past them
cuts; several viewports are split into one draw each, `MultiViewportSplit`, 4.12; the viewport within
WebGPU's bounds, width and height at most `Limits::max_texture_2d` and edges within twice it, depths 0
to 1, a front whose API allows a larger one taking `ViewportInShader`, 4.12), blend constant,
stencil reference, small immediate constants, and the range (direct, indexed, or indirect from a
buffer). In every back end the
program's `vertex_index` and `instance_index` include the range's first or base vertex and first
instance, as WebGPU and Vulkan count them; APIs that count otherwise are served by a program variant
(4.12, vertex and instance numbers).

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
  A stride of zero means every vertex and instance reads the same element (Direct3D 11 allows it, and
  engines bind default-attribute streams so): WebGPU takes it natively; on WebGL 2, where
  `vertexAttribPointer`'s zero means tightly packed, the back end draws the stream with an instance
  divisor of 2^31, so every vertex of every instance reads element zero. Exact, no cost. Unused streams
  are `None`.
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
vertex shader and the counts (with the cache's record taken in, and given back as the record to
write, `totals`), and picks the tuples; it counts how many prepared combinations were
later drawn and how many never were, which G4 reports on real programs. A combination still missing at
its first draw is built then and the frame pauses (`PipelineMiss`), never skipping the draw.

### 4.5 Bindings

A binding set is a block: a list of `BindingEntry` (binding number, stages that read it, naming every
stage whose program reads it, since back ends binding per stage and the limits per stage count by it,
and a resource:
uniform buffer range, storage buffer range, texture view, storage texture view, sampler, typed texel
buffer, or a resource table). `create_binding_set` may return a set equal to one the back end already
holds, so fronts call it when their slots changed and otherwise pass the handle they have.

Binding numbers come from the shaders crate's rule (section 6.4), the same for every front: one number
space per set, binding = range start + the API's slot number, with uniform buffers from 0 (16 slots),
samplers from 16 (16), sampled textures and typed buffers from 32 (128), storage buffers and textures
from 160 (64) (`SlotKind` in the crate). Back ends rely on one part of this numbering only, the range
of textures paired with samplers (below); the helper set's bindings lie outside the kinds' ranges.
Which set a slot lands in is the API's rule of 6.4; set 3
always holds the program's helper constants (`HELPER_SET`). A draw's sets are WebGPU's bind groups 0
to 3.

How layouts are made. A WebGPU pipeline needs explicit bind group layouts; WebGL 2 needs none. Most of a
layout comes from the program: the WebGPU back end reads, from the naga module of each stage, every
global the entry point uses (naga's per-entry-point usage), and takes from it the binding type (uniform,
read-only or read-write storage buffer, texture, storage texture, sampler), the texture's dimension,
whether it is multisampled, its sample kind (float, signed, unsigned, depth), a storage texture's format
and access, and whether a sampler compares. One thing a program does not say, and the bound views do:
whether a float texture can be filtered on this back end (`FormatCaps` `FILTER`: `r32float` only with
`float32-filterable`, depth formats never), which picks `"float"` or `"unfilterable-float"`. A texture
the program only loads from, never pairing it with a sampler, is `"unfilterable-float"` whatever is
bound (every float view binds to that), so only paired textures need to be told apart; these must all
lie in the API's sampled-texture range, 32 to 159, while the helper set's load-only textures below 32
(the target copies of `BlendInShader` and `LogicOpInShader`, the early-test depth) need no bit. So each
binding set has a layout key (`BindingLayoutKey`, from `binding_layout_key`): one bit per texture binding
whose view cannot be filtered. Samplers need no bit: by WebGPU's rules a `"filtering"` sampler layout
takes every sampler that does not compare, and `"non-filtering"` is needed only where the program
pairs the sampler with an `"unfilterable-float"` texture, which the texture bits and the program
already say. So a sampler binding is `"comparison"` where the program compares with it,
`"non-filtering"` where the program pairs it with a texture whose bit is set, and `"filtering"`
otherwise; a game binding one program with a point sampler and then a linear one keeps one pipeline. The sampler
bound follows from this: one that compares exactly where the program compares through that binding,
and none with a linear filter (min, mag or mip) where the program pairs it with a texture whose bit is
set. A uniform or storage binding is at least the size of the block the program declares there (a
runtime-sized array counted as one element), as WebGPU's `minBindingSize` and WebGL 2's
`UNIFORM_BLOCK_DATA_SIZE` require (`ConstantBufferGrown`, 5.1). A storage binding's size is a
whole number of words (`STORAGE_WORD`, four bytes), as WebGPU's bind group rule requires; a front
whose API's view or range is not rounds its bytes up (the API's out-of-range rules then cover the
bytes added), as the Vulkan driver does with a descriptor's range (15.4). So that a range reaching
the end of its buffer stays inside it rounded, a front whose API makes buffers of other sizes and
may bind them as storage makes them with their size rounded up to a whole number of words
(`BufferDesc`'s doc): the OpenGL driver for every buffer object (`glBufferData` of six bytes bound
whole as a shader storage buffer), the Direct3D fronts for buffers with unordered access or
shader resource bindings, and the Vulkan driver by its memory requirements (15.4). Every binding
the program reads is bound, whatever the draw then covers or fetches, with the kind the program
declares there (`BindingEntry`'s doc): the WebGPU back end takes its binding layouts from the
program, so a uniform buffer where the program declares storage, a read-only storage binding of a
block the program writes, or a texture view of another dimension, sample type or multisampling
is refused by WebGPU at the draw. The recording back end refuses each before it rasterises or
drops anything.
`prepare` takes the layout keys too (`PrepareDesc::layouts`; the zero key, every texture filterable, is
what a Direct3D 12 pipeline state is prepared with before any descriptor is known), and the cache
records them, so a combination first seen with other keys is prepared at the next start. The pipeline
key includes the layout keys of the sets a draw binds, so the same program with a filterable and an
unfilterable view is two pipelines.

A texture this back end cannot filter, paired in the program with a sampler that filters (any of its
filters linear: Direct3D 11 allows linear filtering of `R32_FLOAT` and of depth read as `R24_UNORM_X8`
or `R32_FLOAT`, which deferred lighting, SSAO and soft particles do), cannot be bound on WebGPU as it
is: the texture is `"unfilterable-float"`, so the sampler would have to be `"non-filtering"`, which a
filtering sampler cannot be bound to. The front sees this from `FormatCaps` (`FILTER` missing for the
view's format) and its own sampler, and binds the variant of the program that filters that texture
itself from `textureLoad` reads with the sampler's state (`FilterInShader`, bit in
`VariantKey::filter_in_shader`), reading the sampler's filter, address modes and level range from the
helper constants (`HelperValue::SamplerState`), so sampler changes build nothing. In that variant the
texture is no longer paired with the sampler, so the sampler binding is `"filtering"` and the front binds
the program's real sampler there: every other texture the program samples through the same sampler
(deferred lighting reads an `R32_FLOAT` depth and its colour G-buffer through one linear-clamp `s0`)
is filtered natively, exactly as before. A point sampler paired with an unfilterable texture needs no
variant: the sampler binding is `"non-filtering"` and the point sampler binds there. Anisotropic filtering, whose footprint Direct3D leaves to the
implementation, is done in the program along the major axis with up to the sampler's anisotropy in
taps.

Variants are chosen by fronts and made by the shaders crate. Every rewrite that depends on bound objects
or on the back end's abilities (filtering in the program, border colours, mirror once, level-of-detail
bias, storage formats from the bound view, constant buffers past the limits, vertex streams fetched in
the program, how typed buffers are read (`TexelBufferRoute`: native where the back end has
`TEXEL_BUFFERS`, which neither browser API has, storage on WebGPU, a texture on WebGL 2 whose element e
is texel (e % 2048, e / 2048), `TEXEL_BUFFER_TEXTURE_WIDTH`; by either, the format of each view at a
texture slot is `texel_formats`, which also gives the view of a texture of another component kind than
the program declares, read by its bits as Direct3D reads it), storage slots
packed into one buffer, storage textures used with atomics read through a buffer, textures and targets stored as integers (`decoded`, `targets_in_shader`), blending or logic
operations done in the program, the sample mask written by the program, clip depth, flipped depth
ranges, depth bias, the counting variant, the interpolation the pixel program reads the vertex
program's outputs with, which WebGPU and GLSL ES require the two stages to agree on) is a field of
`VariantKey` in `wcontainers-gpu`; the front
computes the key from `Caps` and what it binds, asks the shaders crate for that variant (cached by
key, section 8), and counts the slow path. `convert` (6.4) reads nothing but the source, the key and
the root signature (whose hash is in the key), never `Caps`: so a key names one module on every back
end, which the cache's merge relies on. Back ends never rewrite programs: they write their language
from what they are given (6.5).

- WebGPU: a set becomes a bind group, cached by content and by the generation of each buffer it names
  (renaming a buffer, 4.6, bumps its generation). Per-draw constants must not make a bind group per
  draw (the research's femtovg case: about 0.5 ms of GPU-process time per drawn element while each draw
  made a bind group and other objects, about 1.4 µs once it stopped). So uniform bindings use WebGPU's
  dynamic offsets: the layout makes the program's uniform bindings dynamic, in set and binding order, up
  to the adapter's `maxDynamicUniformBuffersPerPipelineLayout` (8 by default, which covers what a stage
  typically binds); a dynamic entry is cached without its offset, and the draw passes the offset to
  `setBindGroup`. Buffers with uniform usage and `MapIntent::Often` are not renamed into a separate
  spare (4.6): each `Discard` write places their bytes at a new offset of a uniform ring (one large GPU
  buffer, replaced by a larger one only when it fills), so their bind groups keep naming the same ring
  buffer and the draw's dynamic offset is the buffer's current ring offset plus the binding's offset
  (one load per dynamic entry per draw). Direct3D 11.1's constant buffer offsets
  (`*SetConstantBuffers1`, multiples of 256 bytes, which WebGPU's 256-byte alignment takes) are
  dynamic offsets too. The uniform bindings past the dynamic limit stay static, their sets cached by
  generation.
- WebGL 2: a set becomes a list of unit and uniform-block bindings. GLSL ES has combined samplers only,
  so the WebGL 2 shader writer gives each (texture, sampler) pair a unit; the back end binds the texture
  and the sampler object to that unit. More pairs than units is the slow path `SamplerPairsExpanded` where
  the shader can be split, and otherwise the shader fails to build (4.12).
- Storage buffer bindings at any offset (Direct3D's raw and structured views from any first element):
  where the offset is not a multiple of `min_storage_offset_alignment` (256 on WebGPU), the front binds
  from the largest offset below that is a multiple of both the alignment and the program's element
  stride (`ReflectedBinding::stride`) and writes the rest into the helper constants
  (`HelperValue::StorageOffset`), which a program made with `VariantFlags::STORAGE_OFFSET` adds to its
  storage addresses (one add per access; the slow path `StorageOffsetInShader`, counted at each binding
  bound from below its start). A front that cannot know ahead of the draw (Direct3D 12's
  pipelines) asks for the flag on every program with storage buffers, so there is no second program.
- Direct3D 12 descriptor tables live in heaps the program writes with the CPU at any time, and root
  signature 1.0 (and `DESCRIPTORS_VOLATILE` ranges in 1.1) let it change descriptors after recording a
  command list, up to that list's execution. Without `RESOURCE_TABLES` the Direct3D 12 front therefore
  reads a table's descriptors when it replays the list at `ExecuteCommandLists` (never when the guest
  records the draw) and makes a binding set from them, cached by content as any set is; with it, the
  front binds the heap as a table.

### 4.6 Memory: bytes in, mapping, bytes out

Nothing is mapped across the boundary. Every API's mapping rule becomes a write or a read back:

- **WriteMode::Ordered**: the write takes effect between the commands before and after it. The general
  case: Direct3D 11 `UpdateSubresource`, Direct3D 9 `Lock` without flags, OpenGL `glBufferSubData`. A
  front passes a write that covers the whole buffer as `Discard` instead, which means the same thing
  (Direct3D 11.0's `UpdateSubresource` of a constant buffer always does). WebGPU: outside a pass, a `writeBuffer` into a staging buffer and a copy in command order; inside a
  pass, the back end ends the pass, copies, and resumes with `Load` (counted `WriteSplitPass`), unless no
  earlier command in the frame uses the buffer, when it writes directly. WebGL 2: `bufferSubData` is
  ordered by GL itself.
- **WriteMode::Discard**: the whole old contents go; earlier commands still see them. Direct3D 11
  `Map(WRITE_DISCARD)`, Direct3D 9 `D3DLOCK_DISCARD`, OpenGL `GL_MAP_INVALIDATE_BUFFER_BIT` and
  `glBufferData` with the same size. WebGPU: the back end renames the buffer to a spare from a pool (the
  same idea DXVK uses) and writes the spare, or for a frequently written uniform buffer moves it to a
  new offset of the uniform ring (4.5); no pass is split. WebGL 2: `bufferData(size, null)` at the
  buffer's own size (orphaning, which drivers rename), then `bufferSubData` of the part. A `Discard` may
  write only part of the buffer, as the ring default does when it wraps (its first part is `Discard`,
  later parts `NoOverwrite` at higher offsets), so `bufferData` with only the part's bytes, which would
  shrink the GL buffer and make those later writes fail, is never used.
- **WriteMode::NoOverwrite**: the caller promises no unfinished command, and no `Ordered` write made
  since it last waited, reads or writes the bytes. Direct3D 11 `Map(WRITE_NO_OVERWRITE)` (on dynamic
  resources, which copies and `UpdateSubresource` cannot write), OpenGL `GL_MAP_UNSYNCHRONIZED_BIT`
  (whose writes over bytes pending operations touch are undefined). Both back ends write at once,
  ahead of the commands recorded before it; writes of any mode but `Ordered` keep their order among
  themselves.

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
has it, the whole mapped range is written (`MappedUploadWhole`). That is exact only while the shadow
equals what the GPU holds, so the front keeps that true: dynamic resources are never written by the
GPU (Direct3D forbids it), and readable maps fill the block from a read back before returning the
pointer. That read back starts when the front records a copy into a CPU-readable staging resource,
right after the copy, not at the map: the common pattern copies into one of several staging resources
and maps it frames later (auto exposure, GPU picking, read back rings), and by then the read back is
ready, so `Map(READ)` finds it `Ready` and never waits (DXVK starts its staging read backs at the copy
too). A later copy into the same resource starts a new read back and drops the older one; a map that
finds its read back still `Pending` waits on it (`ReadBackWait`), or with `DO_NOT_WAIT` answers
`DXGI_ERROR_WAS_STILL_DRAWING`. A staging resource that is copied into and never mapped costs one read
back per copy, which is what staging resources are made for. A staging resource the GPU may have written since its shadow was last filled (a copy into it),
mapped for writing only (`Map(WRITE)` on a staging resource), is read back into the shadow first
(`StagingRefreshed`), so the bytes the program does not write keep the GPU's values. The front tracks
"written by the GPU since the last fill" per staging resource from the copies it records.

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

Dynamic textures: `write_texture` with `Discard` drops the whole texture's old contents (every level
and layer; texels not written are undefined), so the WebGPU back end renames the texture from a pool,
as for buffers, instead of splitting a pass for an ordered write. `NoOverwrite` promises of the texels
written (both aspects of depth and stencil) what it promises of a buffer's bytes.

Buffers that hold both indices and other data on WebGL 2. A WebGL buffer takes a type when it is first
bound (element array for `ELEMENT_ARRAY_BUFFER`, other data for every other target but the two copy
targets) and can never be bound the other way, and `copyBufferSubData` between the two types fails. The
fronts' APIs allow both in one buffer: Direct3D 11 with both bind flags, and every buffer of Direct3D 12,
OpenGL and Vulkan. The WebGL 2 back end therefore holds an interface buffer as up to two GL buffers, each
made when the buffer is first bound its way, so a buffer only ever used one way stays one GL buffer:
- Host writes (`write_buffer` in every mode, `Discard` renaming both) go to every GL buffer that exists.
  Two GL buffers mean two uploads of the same bytes (`IndexBufferTwin`, counted when the second is made).
- GPU writes into the buffer (`copy_buffer`, `copy_texture_to_buffer`, `clear_buffer`, `resolve_queries`,
  transform feedback) are made in the GL buffer they can reach: copies from a buffer go between GL
  buffers of the same type, so a copy whose source has both types reaches both; everything else lands
  in the other-data buffer. A GL buffer a GPU write could not reach is marked stale for that range.
- Before a draw binds a stale range as indices, the back end reads the other-data buffer's range back
  with `getBufferSubData` (which waits for the GPU) and writes it into the element-array buffer
  (`IndexTwinReadBack`). Before a stale range is used as other data, the same the other way. Indices the
  GPU writes are rare (stream output or compute writing an index buffer, and WebGL 2 has no compute);
  WebGL's own validation of indices on the host is why the rule exists.
Exact. WebGPU, and the fast-path defaults on every back end (two rings, 4.3), never meet it.

What the back ends do with `MapIntent::Often`: WebGPU keeps such buffers in the rename pool; WebGL 2 uses
`DYNAMIC_DRAW`. Guest memory is the source of every write, so there is no second CPU copy of a buffer
anywhere unless a front needs one (the OpenGL front's index copies, 4.3).

### 4.7 Passes

Draws and queries happen inside a pass; copies, whole-texture clears, dispatches, vertex capture and read
backs outside. Explicit fronts map their own passes (Direct3D 12 render passes or its target changes,
Vulkan render passes; subpasses become one pass each). Implicit fronts (Direct3D 11, OpenGL, Direct3D 9)
open a pass at the first draw after their targets change and close it before a copy, a dispatch, a
target change or a draw that would break the pass's usage scope (below); a clear that comes first in a
pass becomes the pass's `Clear` load. This pass tracker is a
shared helper in `wcontainers-gpu` (added at G1, with the occlusion counter of 4.12 and the combination
tracker of 4.4) so each implicit front uses the same one.

Rules a pass description follows:
- A depth-stencil attachment names only the aspects its format has (`AspectOps` for depth and for
  stencil, each with its load, store and read-only flag), so no back end sees an operation on a missing
  aspect. No draw writes an aspect marked read-only (WebGPU's `depthReadOnly` and `stencilReadOnly`).
- No subresource or 3D slice is attached, or resolved into, twice in one pass. Colour attachments stay
  within `color_targets` and `color_bytes_per_sample`.
- A color attachment of a 3D view names the slice it draws into (`depth_slice`, WebGPU's `depthSlice`).
- Every attachment and resolve target has the same width and height. WebGPU requires it (its
  `GPURenderPassDescriptor` validation: every view's render extent must match), and so does WebGL 2 (its
  specification keeps WebGL 1's `FRAMEBUFFER_INCOMPLETE_DIMENSIONS`). The fronts' APIs do not: Direct3D 9
  to 11 require equal sizes among render targets but let the depth buffer be larger (a screen-sized
  depth buffer bound with a smaller colour target), and OpenGL and Vulkan draw into attachments of any
  sizes, limited to the area common to all. The pass tracker, which every front uses for this (explicit
  fronts too), fits them (`AttachmentsFitted`): the pass takes the size of its depth-stencil attachment,
  or of colour target 0 when there is none, and each other attachment of a different size is replaced by
  a temporary of the pass's size, format and sample count (pooled by those, so a frame makes none after
  the first). Depth is never the one replaced, because WebGPU copies depth only as whole subresources.
  At the pass's start a replaced attachment whose load is `Load` has the common area copied into its
  temporary (`copy_texture` for single-sampled colour; for multisampled colour on WebGPU, a draw reading
  each sample with `MULTISAMPLE_READ` and writing it per sample); `Clear` clears the temporary. At the
  end, with `Store`, the common area is copied back the same way, and a resolve target goes through a
  temporary of the pass's size as well. Every draw's scissor is cut to the common area, so the larger
  attachments are not touched outside it. Exact on WebGPU. On WebGL 2, multisampled targets are
  renderbuffers that no program can read and `blitFramebuffer` cannot copy into, so a replaced
  multisampled attachment is carried through its resolved value (resolved into a single-sampled
  texture, then drawn into every sample, `MultisampleCopyResolved`): a listed loss (4.12), only for
  multisampled attachments of different sizes whose contents are loaded or stored. The tracker takes
  the route from `MULTISAMPLE_READ` and `SAMPLE_SHADING`; integer samples, which no resolve averages,
  have only the first, and without it their fitting is reported absent (WebGL 2 multisamples no
  integer format).
- A pass with no attachment at all (Direct3D 11's drawing with only pixel shader storage writes bound)
  gives its size and sample count in `empty`. The WebGPU back end adds an attachment of its own of that
  size (`r8unorm`, writes masked off, store `Discard`), because a WebGPU pass needs one; its pipelines are
  built for it. So the size is 1 to `max_texture_2d` each way and the count one `r8unorm` takes. Direct3D 11.1's forced sample counts above one are reported absent (section 9).
- One pass is one usage scope, as WebGPU defines it (its specification: "one render pass is one usage
  scope", covering every resource any `setBindGroup`, `setVertexBuffer` or `setIndexBuffer` of the pass
  references; binding a buffer as storage and as input in one pass is "a non-compatible usage list"). A
  whole buffer, or a texture subresource, that any draw of the pass binds as writable storage is used
  no other way by any draw of the pass: not as vertices, indices, indirect arguments, a uniform, a
  sampled texture or read-only storage. Direct3D 11 and 12, OpenGL and Vulkan allow a pixel shader's
  storage write followed, in a later draw with the same targets, by a read of the same resource another
  way (per-pixel linked lists for order-independent transparency, GPU histograms, Direct3D 12's
  unordered-access-to-shader-resource barrier between draws). The pass tracker keeps, per open pass,
  the buffers and subresources its draws named as writable storage and those they named any other way
  (kept sorted; while neither the pass nor the draw writes storage, the common case, the check is a
  test of one flag, and recording a draw's uses one search per use); before a draw that would
  put a resource in both, it ends the pass with `Store` and begins the next on the same targets with
  `Load` (`UsageScopeSplit`, counted), keeping the pass's read-only aspects; each part names the
  resolve target, whose last resolve is the one that stays (a part cannot be known to be the last when
  it begins; a split pass costs one more resolve). For the same reason an explicit front, which knows
  its whole pass before it replays it, begins a pass whose draws write storage with `Store` where its
  API discards (a split pass keeps those attachments), and the tracker refuses to split a pass that
  discards. Running
queries carry over as at any split (below). Exact: the
  later draw sees the storage writes, as the APIs promise after their barrier or hazard rule. Within one
  draw, Direct3D forbids the same resource as a storage view and any other binding (Direct3D 11's
  runtime unbinds the other; Direct3D 12's resource states exclude it); OpenGL and Vulkan allow one
  buffer as a storage buffer and, at another range, a uniform or vertex buffer in the same draw. For
  those the front copies the other use's range into a pooled temporary before the pass and binds the
  copy (`UsageScopeCopied`): exact, because a draw reading a range it also writes has no defined order
  in those APIs either. WebGL 2 has no storage in render passes, so it never meets the rule; a native
  back end would split a few passes it need not, a cost only where storage writes are read back in one
  pass.
- Occlusion queries live inside one pass. A guest query that spans a pass split (a target change or a
  copy in the middle, which the pass tracker turns into several passes) is several interface queries,
  one per pass; the front adds their counts or combines their "any" answers.
- One interface query runs at a time (WebGPU allows one active occlusion query per pass), and each
  index of the pass's set is begun at most once in that pass (WebGPU refuses a second), so a front
  takes a fresh index for each stretch; a pass whose set has no index left ends, and the next pass
  takes a fresh set (`QuerySetSplitPass`). Guest queries
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

Object creation never waits for the device thread: the stand-in allocates object names on the guest
side, so `CreateBuffer` returns at once and the creation travels in the buffer like any call. What the
API reports at creation is therefore decided before the call returns, on the calling thread, by checks
that need no device: each front has pure check functions over the call's description or bytes and a
copy of the back end's `Caps` (plain data, copied to the calling side when the device is made). In the
container the stand-in reaches them through the graphics hook as a host call that runs on the guest's
own thread, in host code; in the Windows-library build the entry point calls them directly. They
decide everything the APIs refuse at creation: invalid descriptions (`E_INVALIDARG`), sizes beyond the
reported limits, formats and uses `FormatCaps` lacks, and for shaders the two counted refusals of 4.12
(`BindingsBeyondLimit`, `VaryingsBeyondLimit`), both functions of the shader's declarations and
signatures and of `Caps` (the declared size of each constant buffer says whether `ConstantsAsTexture`
takes a unit, so binding a larger buffer later changes nothing). The check reads only the declarations
(its cost is unmeasured; G4 measures it); conversion runs later on the device
thread or a worker. What only the device can learn comes after the guest was told the object exists:
memory exhausted (WebGPU reports it through an error scope, WebGL through `getError`, both after the
call) becomes device loss with `LossReason::OutOfMemory` (4.10); a shader the checks passed that the
shaders crate then fails to convert is a bug of ours and becomes device loss with `LossReason::Error`,
never a skipped draw.

A back end has one submission order. A front whose API has several queues that wait on each other
(Direct3D 12 command queues, Vulkan queues) orders their work into that one order itself, by the fence
values they signal and wait for (5.2); work of different guest queues then runs one after the other on
the GPU, never at once (a performance cost, not a correctness one).

### 4.9 Presenting frames

A surface comes from the host's graphics hook (a canvas, transferred to the device worker, in the
browser; a window natively). The DXGI layer gives the program its own back buffer textures and, at
`Present`, copies the current back buffer into `current_texture(surface)` (converting format or sRGB on
the way, as one draw) and calls `present`. One copy per frame keeps the program's back buffers ordinary
textures it can read, copy and keep across frames, which swap chains allow. A blit swap chain with the
sequential effect gives out every buffer, as DXGI does (those past the first for reading); at each
present DXGI moves their contents one place down, the first's to the last, which the layer does by
copies (`SequentialBuffersCopied`). Frame pacing: `Present` blocks
the guest when more than the swap chain's maximum frame latency (three by default) are queued; the device
thread learns a frame was shown from the host hook (requestAnimationFrame in the browser, which a worker
also has). The device worker returns to its event loop after every present, because a transferred canvas
updates only then.

Surface formats: `FormatUse::PRESENT` names the formats `configure_surface` takes (WebGPU: `bgra8unorm`,
`rgba8unorm`, `rgba16float`, the canvas formats; WebGL 2: `rgba8unorm`; every back end takes
`rgba8unorm`; a back end whose window lacks a format it declares, as a Metal layer lacks
`rgba8unorm`, draws the frame into the window's format at present, `PresentConverted`). The DXGI layer configures the back buffer's own format where it has `PRESENT`, otherwise
`rgba16float` for back buffers of more than 8 bits per channel where present, otherwise `rgba8unorm`;
the present copy converts. The host's window scales whatever it shows, so a frame of another size
than its window, from a swap chain asking for no stretching (`DXGI_SCALING_NONE`,
`ASPECT_RATIO_STRETCH`), is shown stretched (a listed loss, `FrameStretched`). Colour space: SDR only. Colours are shown as sRGB and values beyond one are
clipped, as on an SDR display. DXGI reports the output's colour space as
`DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709` (`IDXGIOutput6::GetDesc1`, with the SDR luminance values),
`CheckColorSpaceSupport` answers present for that space and, on 16-bit float back buffers, for
`RGB_FULL_G10_NONE_P709`, and absent for every HDR10 space. That is what Windows answers on an SDR
display (expected; G5 compares it with the development desktop's own answers, dumped read only). HDR
output (the canvas's extended tone mapping) is not offered.

### 4.10 Errors and device loss

Calls that make objects return `Result<_, GpuError>`: `Absent(features)`, `OutOfMemory` (where the
back end knows at once, as a native one may), `DeviceLost`, or `Invalid(rule)` (a bug in a front). The
guest has already had its answer from the creation checks (4.8), so a front receiving an error for an
object it reported made reports the device lost: `OutOfMemory` as `LossReason::OutOfMemory`, which the
Direct3D fronts answer as `DXGI_ERROR_DEVICE_REMOVED` with `GetDeviceRemovedReason` giving
`DXGI_ERROR_DRIVER_INTERNAL_ERROR` (Microsoft lists no memory reason for Direct3D 11; saved in scratch,
`downloads/msdocs/nf-d3d11-id3d11device-getdeviceremovedreason.md`), Vulkan as `VK_ERROR_DEVICE_LOST`,
OpenGL as `GL_CONTEXT_LOST`. Recording calls do not return
errors; a back end that hits one records it and reports the device lost with `LossReason::Error`.
`status()` reports `Ok` or `Lost(reason)`; fronts poll it when they present and when they wait, and
answer with their API's own words (`DXGI_ERROR_DEVICE_REMOVED` with `GetDeviceRemovedReason`,
`VK_ERROR_DEVICE_LOST`, `GL_CONTEXT_LOST`). WebGPU's `device.lost` promise and WebGL's
`webglcontextlost` event feed it.

### 4.11 The capability list

`Caps` is `AdapterInfo` (the GPU's name, PCI vendor and device numbers where the host gives them,
and whether it draws on the CPU, which DXGI's adapter description and the other APIs' device names
answer from), `Features` (bits), `Limits` (numbers) and `FormatCaps` (per format: sample, filter, render,
blend, resolve, storage read and write, storage atomics, vertex, depth-stencil, copy out, present, and
the set of sample counts a texture of that format may have, so a native back end can say a format takes
4 samples but not 8 where others take 8, as Direct3D's `CheckMultisampleQualityLevels` and Vulkan's
sample count flags ask per format and count). A format with any use is served in every texture
dimension it can have (1D where its blocks are one texel high, 3D but for depth and stencil), held
decoded or converted where the back end's API lacks that dimension for it, as Direct3D's format tables
require. A format a back end does not hold has no bits, and the front takes the route 4.12 names for
it. The bits are in the
crate; the values each back end declares (expected, to be confirmed by each back end's milestone against
the adapter):

- WebGPU in Chrome: `COMPUTE`, `VERTEX_STORAGE_READ`, `FRAGMENT_STORAGE_WRITE`, `PROVOKING_VERTEX_FIRST`,
  `DEPTH_CLAMP` (depth-clip-control, 98.1% of adapters per the research), `CLIP_DISTANCE`
  (clip-distances, 82.2%) and `DUAL_SOURCE_BLEND` (dual-source-blending, 82.2%) where the adapter has
  them (each has a route without, 4.12),
  `INDEPENDENT_BLEND`, `SAMPLE_SHADING`, `SAMPLE_MASK`, `MULTISAMPLE_READ`, `TIMESTAMPS`, `INDIRECT`,
  `INDIRECT_FIRST_INSTANCE` where the adapter has indirect-first-instance (99.97%; without it, indirect
  draws with arguments a front cannot vouch for read them back, `IndirectReadBack`),
  `BASE_VERTEX_INSTANCE`, `TEXTURE_VIEWS`,
  `SEPARATE_SAMPLERS`, `SHADER_F16`, `SUBGROUPS`, `PRIMITIVE_INDEX` where the adapter has
  primitive-index (85.1%), `BACKGROUND_PREPARE`,
  `CLIP_DEPTH_ZERO_ONE`, `DEPTH_BIAS_CLAMP`, `TEXTURE_LEVEL_QUERY` (`textureNumLevels`,
  `textureNumSamples`), `FINE_DERIVATIVES` (WGSL's `dpdxFine` and the like), `DEPTH24_UNORM` where the probe below finds it, `FRACTIONAL_VIEWPORT` (WebGPU lets an implementation round
  viewports, at least to whole pixels; Chrome passes them to Metal, Direct3D 12 and Vulkan, which keep
  sub-pixel precision; G2 confirms by drawing with a viewport at a half-pixel offset against the
  reference, and if Chrome rounds, the bit goes and fronts take `ViewportInShader`), and
  `BLOCK_COMPRESSION` where the adapter has
  `texture-compression-bc` (86.8% of adapters per the research) and, for 3D textures,
  `texture-compression-bc-sliced-3d`. Absent: `MIRROR_ONCE`,
  `VERTEX_STORAGE_WRITE`, `TRANSFORM_FEEDBACK`, `READ_BACK_NOW`, `FAN_AND_LOOP`, `POLYGON_MODE`,
  `POLYGON_MODE_POINT`,
  `PROVOKING_VERTEX_LAST`, `CLIP_DEPTH_MINUS_ONE`, `CULL_DISTANCE`, `LOGIC_OP`, `WIDE_LINES`,
  `POINT_SIZE`, `OCCLUSION_COUNT`, `RESTART_SWITCH`, `FORMAT_REINTERPRET`, `SHADER_I64`, `SHADER_F64`,
  `INDEX_U8` (WebGPU indices are 16 or 32 bits), `FRACTIONAL_DEPTH_BIAS` (`depthBias` is an integer),
  `BLEND_CONSTANT_ALPHA` (WebGPU's blend factors have only the constant colour),
  `RESOURCE_TABLES`, `PERSISTENT_MAPPING`, `BORDER_COLOR`, `SPARSE`, `DEPTH_BOUNDS`,
  `STENCIL_PER_FACE`. `Depth32FloatStencil8` has its bits only with depth32float-stencil8 (Chrome since
  113, Firefox and Safari expose it; its share of adapters is unmeasured, section 13). Sample counts
  (per format) 1 and 4 only, and 4 only on the formats WebGPU allows (not
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
  `CLIP_DEPTH_MINUS_ONE`, `POINT_SIZE` (up to the device's range), `INDEX_U8` (`UNSIGNED_BYTE`
  indices), `FRACTIONAL_DEPTH_BIAS` (`polygonOffset` takes float units), `BLEND_CONSTANT_ALPHA` (but WebGL forbids the constant colour
  and constant alpha as one colour blend's two factors, so such a block is refused as absent and blended
  in the program), `DEPTH24_UNORM` where the probe below finds it, and by extension where present:
  `POLYGON_MODE` (WEBGL_polygon_mode, lines only: `POLYGON_MODE_POINT` is absent), `PROVOKING_VERTEX_FIRST` (WEBGL_provoking_vertex),
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
  `TEXTURE_LEVEL_QUERY` (GLSL ES 3.00 has `textureSize` but no `textureQueryLevels`, and no multisampled
  samplers),
  `MULTISAMPLE_READ`, `SUBGROUPS`, `FINE_DERIVATIVES` (GLSL ES 3.00 has only `dFdx` and `dFdy`, which
  may be coarse), `LOGIC_OP`, `WIDE_LINES`, `FRACTIONAL_VIEWPORT` (`viewport` takes
  integers), `OCCLUSION_COUNT` (GLES 3.0 has only
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

How 24-bit depth is stored (`DEPTH24_UNORM`). Direct3D's most common depth format, `D24_UNORM_S8_UINT`,
maps to `depth24plus-stencil8` (WebGPU) or `DEPTH24_STENCIL8` (WebGL 2), and neither API says how it is
stored: on GPUs with 24-bit depth it is 24-bit unsigned normalised, and on Apple silicon (Metal has no
24-bit depth there) it is 32-bit float. That changes the constant depth bias: WebGPU's "biased fragment
depth" applies `depthBias * 2^-24` for a unorm format but `depthBias * 2^(exponent of the primitive's
largest depth - 23)` for a float one, and GLES's polygon offset unit follows the stored format the same
way; Direct3D defines the unorm rule for its 24-bit formats. Equal for depths in [0.5, 1), up to 2^7
times smaller near 0.1, smaller still towards 0, which shadow maps reach. So each back end probes when its
device is made: one triangle at a constant depth of 0.1 drawn with a constant bias of one unit into a 1
by 1 `Depth24Plus` target, the depth then sampled into an `r32float` target and read back. A result one
24-bit step above the 24-bit value of 0.1 sets `DEPTH24_UNORM`; a result 2^-27 above 0.1 means float
storage, and the bit stays clear; anything else is a back end bug that fails its tests. Without the bit
a front drawing into a 24-bit depth target with a non-zero constant bias binds `DepthBiasInShader` with
the 24-bit step in `HelperValue::DepthBias`, so the bias is Direct3D's. The depth values themselves keep
float precision there, finer than Direct3D's 24 bits: a listed loss (4.12). G2 and G3 write the probe
and record what this Mac and the desktop report.

Neither list is reduced to the other: each back end declares what it has and runs at its best.

### 4.12 What the interface cannot serve exactly, and what fronts do

These were looked for on purpose. Each is done exactly by a named slow path, reported absent through
the API's own mechanism, or (in the few places that say so) listed as a loss.

- **Occlusion query sample counts.** Neither browser API counts samples: WebGPU promises only zero or
  non-zero (its spec's `endOcclusionQuery` steps), WebGL 2 has only any-samples queries. Every back end
  has `OcclusionAny` natively; `Occlusion` (exact counts) needs `OCCLUSION_COUNT`, absent in both. Where
  the API asks only "any" (Direct3D's occlusion predicate and `SetPredication`, OpenGL's
  `ANY_SAMPLES_PASSED`, Vulkan's queries without `VK_QUERY_CONTROL_PRECISE_BIT`) the native query
  answers. Where it asks for a count (Direct3D 9, 11 and 12 occlusion queries, OpenGL's
  `SAMPLES_PASSED`, Vulkan's precise queries where `occlusionQueryPrecise` is reported, 15.4) the
  front counts exactly by replay (`OcclusionCountReplay`), with a shared helper in `wcontainers-gpu`
  (G1):
  1. At the guest's query begin, the pass tracker ends the pass; before the first draw since the last
     count that writes depth or stencil, the depth-stencil target is copied.
  2. The query's draws run normally (with an `OcclusionAny` query, which the front can answer early
     when it is zero), and the front keeps their draw calls. The kept draws name their buffers and
     targets, not their contents, so before anything else the front records while the query runs
     that changes what they read or drew on (a buffer or texture write, a copy, a clear, a resolve, a
     change of targets) it has them counted (step 3) and keeps the later ones afresh; the helper
     refuses a draw or a query end on changed targets that were not counted first.
  3. At query end, and at each such change, a counting pass: one counting target of the pass's size and sample count (`r32float`
     where `float32-blendable` or EXT_float_blend is present, otherwise `r16float`; WebGL 2 renders float
     with EXT_color_buffer_float, 99.94% per the research), cleared to zero, with the depth-stencil
     target (read-only when no draw writes it, as with the usual bounding-box queries; otherwise the
     copy, which the counting draws write as the draws wrote the target). The draws are replayed with their own depth-stencil and raster blocks, alpha to coverage
     kept, blend One plus One, and the counting variant of each program: the fragment shader with its
     outputs replaced by 1.0 at location 0 (alpha kept for alpha to coverage) and its storage writes
     removed, or the built-in one when the draw had none. The helper takes each draw's counting program
     from the front, which gets it from the shaders crate (`VariantFlags::COUNTING`); so the helper
     (G1) needs nothing from G6, and G1's tests give it counting programs written in WGSL. Blending happens exactly for the samples that
     pass every per-fragment test, so each sample holds how many fragments passed there.
  4. A reduction sums the target into the query's result: where the back end computes (with 8 bytes
     of workgroup memory, and reading multisampled textures for a multisampled target) and the draws'
     primitives times the sample count stay below the counting format's exact bound, a dispatch adds
     every sample into a u64 buffer of the helper's, so nothing waits; `end_into` copies it into a
     buffer of the front's in order (Direct3D 12's query heap), and `end` reads it back. Otherwise
     (the default; WebGL 2) the target is read back and summed on the device thread, a multisampled
     target resolved first and the average multiplied by the sample count (`ReadBackWait` where it
     waits). The query's count is the sum over its counting passes. If any pixel's sum over
     its samples reached the counting format's exact bound (2^24 for `r32float`, 2048 for `r16float`;
     below it the resolved average keeps every count exact), the replay runs
     again with the draws split into halves (of the draws; of one draw's instances, by raising the
     first instance, where the back end has `BASE_VERTEX_INSTANCE` and the front says every
     per-instance stream steps by one, so each instance reads what it read; otherwise of one list's
     or strip's primitives, each drawn for every instance, a triangle strip at an even triangle), the
     second half after the first and
     both from the depth the whole began with (kept by one more copy, made only where the draws'
     primitives times the sample count reach the bound), and the halves added, so the answer stays
     exact. A single fan, strip with restart or indirect draw reaching the bound cannot be split, nor
     can one primitive's instances where the first instance cannot be raised: the
     helper refuses it (`GpuError::Invalid`), a known gap that needs, from one such draw on a back end
     without blendable `r32float`, a pixel whose samples together pass 2048 fragments (2048 at its one
     sample on a single-sampled target, 512 a sample on average on four samples). A program reading the primitive index sees it
     start again in a split's second half.
  The cost is one pass and one reduction per counted query and per change inside it, and a depth copy
  only for those whose draws write depth or stencil (unmeasured; G4 measures it on real programs).
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
  with `E_INVALIDARG`, decided by the creation check on the calling thread (4.8) from the shader's
  declarations (`BindingsBeyondLimit`, a counted refusal, and a known gap). The corpus at G6
  measures how many real shaders this hits.
- **Vertex input slots and elements.** Direct3D 11 has 32 input slots and 32 elements (10_0: 16 and
  16); WebGPU's adapters give 8 vertex buffers and 16 to 30 attributes. Streams and attributes beyond the
  back end's limits are fetched by the vertex program from read-only storage buffers by vertex and
  instance index, with the format decoded in the program and bounds checked, so reads past the end give
  zero as Direct3D says (`VertexPulledInShader`, `VariantKey::pulled`; each pulled input has its own
  storage binding and its offset, stride and step in the helper constants, so the program does not
  depend on how the inputs share streams). Exact. WebGL 2 binds a buffer per attribute and GLES 3.0 guarantees 16 attributes, which
  is 10_0's number, so it never needs this.
- **Constant buffer slots and sizes.** Direct3D has 14 constant buffers per stage of up to 65536 bytes;
  the helper constants take one more uniform binding. WebGPU gives 12 uniform buffers per stage by
  default (12 on this Mac) of 65536 bytes: the slots beyond the limit are read as read-only storage
  buffers, which hold the same 16-byte rows (`ConstantsAsStorage`). Exact. WebGL 2 gives at least 12
  uniform blocks (16 here) of at least 16384 bytes (16384 here): a constant buffer beyond the block
  count, or bound with more bytes than a block holds, is copied on the GPU into a 32-bit RGBA integer
  texture (`copy_buffer_to_texture`) when it changed since the last copy, row r at texel (r % 256,
  r / 256) (`CONSTANTS_TEXTURE_WIDTH`), and read with `texelFetch` (`ConstantsAsTexture`). Exact. The texture takes a unit; a shader that then needs more units than the
  back end has is refused (`BindingsBeyondLimit`).
- **Values between stages.** Direct3D 11 passes 32 four-component registers from the vertex stage to
  the pixel stage (10_0: 16). WebGPU gives 16 by default and 28 here; WebGL 2 at least 15 vectors plus
  position (30 here). The shaders crate packs values by component where their interpolation agrees,
  which costs nothing at run time; a shader that still needs more than the back end has is refused at
  creation by the calling thread's check (4.8) from its signature, as `E_INVALIDARG`
  (`VaryingsBeyondLimit`, a counted refusal and a known gap, like `BindingsBeyondLimit`). The
  corpus at G6 measures how many real shaders this hits.
- **Texture sizes.** Not emulated: a front reports the feature level whose sizes the back end meets
  (section 9), and refuses larger textures as its API does.
- **Storage textures through a buffer: atomics, and formats without storage.** Direct3D 11_0 requires
  every atomic operation on typed storage views of `R32_UINT` and `R32_SINT`, and exchange on
  `R32_FLOAT` (tiled light culling and depth min-max passes use them); no browser format has
  `STORAGE_ATOMIC` (4.11). Its format tables also require typed storage stores (UAV typed store) on
  `R10G10B10A2_*`, `R11G11B10_FLOAT`, `R16G16_*`, `R8G8_*`, `R16_*`, `R8_*` and `A8_UNORM` (compute
  post-processing writes `R11G11B10` and `R16G16` this way); WebGPU stores into these only with
  `texture-formats-tier1` (its spec's feature list: `r8*`, `rg8*`, `r16*`, `rg16*`, `rgb10a2*`,
  `rg11b10ufloat`), which 6.9% of adapters lack. `A8_UNORM` is stored as `rgba8unorm`
  (`FormatConverted`), which core WebGPU stores into, the program writing (0, 0, 0, a). One route
  serves both: the front gives each texture used through such a view a buffer holding the view's texels
  in the layout `copy_texture_to_buffer` writes (rows padded to the copy alignment), and binds the
  variant that reads, writes and runs atomics on that storage slot through the buffer at the same
  binding (`VariantKey::texels_through_buffer`, the view's format from `VariantKey::storage_formats`;
  the row pitch, rows per layer and the view's width, height and layers in
  `HelperValue::TexelLayout`, for its bounds and size queries). The program encodes and decodes the
  format as Direct3D's conversion rules say; float exchange is u32 exchange of the bits; a texel
  narrower than 32 bits is written by a compare-exchange loop on its 32-bit word, so neighbouring texels
  written at the same time by other invocations are kept and one of two racing writes to the same texel
  wins, as on hardware. The front tracks which of texture and buffer is newer: before a draw or dispatch
  with the variant, a newer texture is copied into the buffer; before any other use of the texture
  (sampling, target, copy, read back, a storage view without the variant), a newer buffer is copied
  back (`AtomicsThroughBuffer` for atomics, `StorageFormatThroughBuffer` for formats). Exact: the same
  bits, and storage buffer atomics are atomic. Typed storage buffers (`RWBuffer<uint>`) are storage
  buffers already (`TexelBufferRoute::Storage`), so their atomics and formats are native. WebGL 2 has no
  storage views (10_0).
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
- **Clip and cull distances, dual-source blending, depth clamping and the primitive index.** Direct3D
  10 and 11 require all five in core, with no capability a program could read: `SV_ClipDistance`,
  `SV_CullDistance` (functional specification 15.4), dual-source blending (17.6), `DepthClipEnable`
  false in the rasterizer state, and `SV_PrimitiveID` in pixel shaders. Each is optional in both
  browsers: WebGPU has clip-distances (82.2% of adapters per the research), dual-source-blending
  (82.2%), depth-clip-control (98.1%) and primitive-index (85.1%), and no cull distances at all; WebGL 2
  has WEBGL_clip_cull_distance (81.9%, Firefox 1%), WEBGL_blend_func_extended (its share is not
  published) and EXT_depth_clamp (90.8%, Android 25%), and no primitive index (GLSL ES 3.00 has no
  `gl_PrimitiveID`). Native where the bit is set (`CLIP_DISTANCE`, `CULL_DISTANCE`,
  `DUAL_SOURCE_BLEND`, `DEPTH_CLAMP`, `PRIMITIVE_INDEX`); otherwise:
  - Clip distances: the variant `VariantFlags::CLIP_DISTANCE` passes each distance as an ordinary
    perspective-correct value, and the fragment program discards where one is negative. A clip
    distance is linear in clip space, so perspective-correct interpolation gives its exact value at
    the pixel's point of the primitive, whose sign is the clip test. That is exactly the set of pixels
    clipping leaves, and the attributes are interpolated as clipping leaves them. GLSL ES 3.00 has no
    `noperspective` (naga's GLSL writer refuses linear interpolation for ES), so no route here needs
    a value interpolated linearly in screen space: where one is needed, the program multiplies the
    perspective-correct value by the fragment position's w (`gl_FragCoord.w`, one over the
    interpolated clip w), which gives the value over w interpolated affinely on the screen. On
    multisampled targets the program takes each distance over w that way, evaluates it at each sample
    position from its screen derivatives (exact for an affine value) and clears those samples from its
    output sample mask (`@builtin(sample_mask)` on WebGPU; `gl_SampleMask` on WebGL 2 with
    OES_sample_variables, without which multisampling is reported absent anyway, above). The same
    program serves both back ends. `ClipDistanceInShader`. Exact up to float rounding.
  - Cull distances: a primitive is dropped only when all its vertices are negative on one plane, which
    no fragment can tell (an interpolated value can be negative where a vertex is not), so a discard
    cannot make them. The front hands such draws to the stage helper (section 7) as if they had a
    pass-through geometry shader that drops those primitives (`CullDistanceAsGeometry`: compute on
    WebGPU, transform feedback replay on WebGL 2 without the extension). Exact, slow; cull distances
    are rare in games.
  - Dual-source blending: blending in the program from a copy of the target (`BlendInShader`, with
    `TargetReadPerPrimitive` where primitives overlap). Exact within Direct3D's blending tolerance, very
    slow; dual-source blending is used by some engines' subpixel text and by OpenGL 3.3, so G4 counts
    it on real programs.
  - Depth clamping: the variant `VariantFlags::DEPTH_CLAMP` keeps clip z inside the clip volume in the
    last vertex stage, passes the unclamped clip z as a perspective-correct value, and writes the
    fragment's depth from it times the fragment position's w (z over w, affine on the screen, as
    above; no `noperspective` is needed), with the raster block's bias added (as `DepthBiasInShader` does, since a written depth is
    not biased by the fixed function) and clamped to the viewport's depth range
    (`HelperValue::DepthRange`). `DepthClampInShader`. Exact; writing depth turns off early depth tests
    for those draws only.
  - The vertex and pixel programs of these two variants are converted apart, so both keys carry
    `VariantKey::linked`: the first location past every location either program's reflection lists,
    and the vertex program's clip distance count. The added values are four-component values from that
    location, the distances first, then the unclamped z.
  - The primitive index in pixel shaders: the stage helper's route again, as for a geometry shader
    that passes the primitive's number on, with the pixel shader made as `PRIMITIVE_ID_FROM_STAGE`
    (it reads the number from a flat value the helper's draw passes) (`PrimitiveIdAsGeometry`). Exact,
    slow; visibility-buffer renderers use it, and they are 11_0 programs on WebGPU, where 85.1% of
    adapters have it natively.
  `D32_FLOAT_S8X24_UINT` as a depth-stencil target is required from 10_0 too: WebGL 2 has
  `DEPTH32F_STENCIL8` in core; WebGPU only with depth32float-stencil8. No exact route exists without
  it (a pass has one depth-stencil attachment, so depth and stencil cannot live in two textures, and
  24-bit storage loses bits), so it is a condition of 10_0 and every level above (section 9).
  Indirect draws with a first instance need `INDIRECT_FIRST_INSTANCE` (99.97% of WebGPU adapters);
  without it the front reads the arguments back and draws directly (`IndirectReadBack`).
- **Resolving in the other sRGB twin on WebGL 2** (a listed loss). `resolve` in the sRGB twin of the
  stored format (4.2) averages the stored values on WebGL 2 (`ResolveInStoredFormat`): only a
  multisampled typeless target resolved with the other sRGB-ness than the format it is stored in,
  whose averaged pixels differ where samples differ (polygon edges). WebGPU is exact.
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
- Format reinterpretation through views (`FORMAT_REINTERPRET`: views of a texture made with
  `reinterpret` in another colour format of the same block size; depth and stencil formats never) is
  absent in both; fronts copy (`ReinterpretCopy`). Exact, slow.
- **Predicated drawing.** Direct3D 11's `SetPredication` names a query. Direct3D 12's names a buffer and
  an offset holding a 64-bit value (usually written there by `ResolveQueryData`), and predicates draws,
  dispatches, `ExecuteIndirect`, clears, copies and resolves; Vulkan's conditional rendering also reads
  a buffer. Neither browser API has a GPU-side form. With Direct3D 11's hint flag
  (`D3D11_QUERY_MISC_PREDICATEHINT`) the operations are kept, as the API allows. Otherwise:
  - Recorded command lists (Direct3D 12, Vulkan) on a back end with `COMPUTE` and `INDIRECT` (WebGPU):
    the front replays a pass only after the program executes its list, so it knows the pass's predicated
    draws and dispatches first. It writes their arguments into an argument buffer and, before the pass,
    dispatches a small compute program of its own (WGSL, like the count-buffer program of 5.2) that
    reads the predicate value and zeroes the vertex, index or group counts of those it skips; they are
    then recorded as indirect draws and dispatches (`PredicationIndirect`; indirect draws keep their
    first instance with `INDIRECT_FIRST_INSTANCE`). `ExecuteIndirect` under predication has its commands
    zeroed the same way. Exact, and nothing waits.
  - Predicated clears, copies and resolves, Direct3D 11's predicates (its draws are recorded as they
    come), and every predicate on WebGL 2: the front reads the predicate value on the device thread
    (`read_back_now` of the buffer, or the query's answer), which waits, and skips or keeps the
    operations (`PredicationWait`). Exact, slow.
- Depth and stencil read back (a staging copy of a depth texture, then `Map(READ)`; OpenGL's
  `glReadPixels` of depth): WebGPU cannot copy `depth24plus` out, and WebGL 2's `readPixels` takes only
  colour formats (GLES 3.0.6 4.3.2). Both back ends draw the depth, sampled as a texture, into an
  `r32float` colour texture and read that (`DepthReadByDraw`; WebGL 2 renders `r32float` with
  EXT_color_buffer_float, a condition of its 10_0); exact for 16 and 32-bit depth, and for 24-bit depth
  as stored. Stencil: WebGPU copies the stencil aspect out natively; WebGL 2 samples it with
  WEBGL_stencil_texturing and draws it into an `r8ui` texture read with `RGBA_INTEGER`, or without the
  extension fills that texture by `StencilCopiedByDraw` first. A depth-stencil texture is a texture on
  WebGL 2 whenever it is single-sampled, which every staging and read-back source is.
- Different stencil reference or masks per face (Vulkan, OpenGL, newer Direct3D 12): WebGPU has one of
  each, and WebGL 2 forbids drawing with them different (4.11); the front draws twice, culling one face
  each time (`StencilFacesSplit`). Exact.
- Depth bounds test (optional in Direct3D 12 and Vulkan): absent in both, reported absent.
- **24-bit depth stored as float** (a listed loss, where `DEPTH24_UNORM` is absent, 4.11). The constant
  depth bias is made exact by `DepthBiasInShader` with the 24-bit step. The stored depth keeps 32-bit
  float precision, finer than Direct3D's 24 bits, so two depths Direct3D rounds to one value may compare
  unequal. Rounding every fragment's depth to 24 bits in the program would make it exact, but writing
  depth in the program turns off early depth testing for every draw, which costs more than the loss is
  worth; pictures are compared with tolerance. Read backs return the depth as stored (4.12, depth read
  back).
- **32-bit float depth copied from values a draw cannot write** (a listed loss, WebGPU). Direct3D and
  Vulkan copy any bits into `D32_FLOAT`; WebGPU writes depth from a buffer or another format only by a
  draw (`DepthWrittenByDraw`), whose depth is clamped to the viewport's range, at most 0 to 1, and whose
  denormals may be flushed. Copied values in that range are exact; values above 1 or below 0 are
  clamped and NaN becomes zero; denormals and -0 are kept or become zero by the GPU (the RTX 3080
  through Direct3D 12 and Vulkan keeps -0 and flushes denormals, Metal on Apple M5 the opposite).
  Depth that draws write is clamped by every API, so only depth copied in differs.
- **Attachments of different sizes** (4.7): exact on WebGPU (`AttachmentsFitted`). On WebGL 2, a
  multisampled attachment replaced by a temporary keeps only its resolved value across the copy: a listed
  loss, for multisampled attachments of different sizes whose contents are loaded or stored.
- **8-bit indices** (OpenGL's `GL_UNSIGNED_BYTE`): native where `INDEX_U8` (WebGL 2); on WebGPU the
  front widens them to 16 bits in the index ring, with 0xff becoming 0xffff where restart is on
  (`IndexWidened`). Exact.
- **Adjacency topologies** (Direct3D 10 and 11's `*_ADJ`, OpenGL's `GL_*_ADJACENCY`): with a geometry
  shader the stage helper reads them (section 7). Without one, which OpenGL allows and where the adjacent
  vertices are ignored, the front writes into the index ring the indices of the list or strip without
  the adjacent vertices (vertices 1 and 2 of each four for lines, 0, 2 and 4 of each six for triangles,
  and the strip forms as the OpenGL specification's primitives with adjacency define them) and draws
  that (`AdjacencyDropped`). Exact. The same route serves Direct3D if a program draws them without a
  geometry shader.
- **Texture level and sample counts** (Direct3D's `resinfo` and `sampleinfo`, required at 10_0;
  OpenGL's `textureQueryLevels`): asked of the texture where `TEXTURE_LEVEL_QUERY`. GLSL ES 3.00 cannot
  ask (naga refused 11 corpus shaders for it, 6.3), so on WebGL 2 the front binds the variant
  `VariantFlags::TEXTURE_INFO`, which the shaders crate makes by reading `HelperValue::TextureLevels`
  and `TextureSamples` for each texture slot the shader asks about; the front writes them from the bound
  views (the view's level count, zero for an empty slot; one sample, as WebGL 2 textures are never
  multisampled) when the bindings change (`TextureInfoInHelpers`). Under the same flag a size asked at
  a level at or past that count reads zero, as Direct3D answers for an empty slot or a level past the
  view's, so a front on any back end binds it where a slot asked its size is empty. Exact.
- Logic operations: optional at Direct3D 11_0 and reported absent there. OpenGL's `glLogicOp` (core)
  runs in the program reading a copy of the target taken before the draw (`LogicOpInShader`; the copy
  is bound in the helper set at `TARGET_COPY_BINDING` plus the target, the operation is in the helper
  constants, `HelperValue::TargetCombine`); a draw whose primitives may overlap is drawn one primitive
  at a time with a copy between (`TargetReadPerPrimitive`). Exact, very slow, and rare.
- Mirror-once addressing (required by Direct3D at every level): native where `MIRROR_ONCE`, otherwise
  the program samples the absolute coordinate with clamping and the original gradients
  (`MirrorOnceInShader`), which gives the same footprint and level of detail. Exact.
- Filtering a texture the back end cannot filter: `FilterInShader` (4.5). Generating mipmaps of one, or
  of a texture stored by `NormAsInteger`, is filtered by the default's own programs (4.3).
- **Early depth and stencil tests** (Direct3D's `[earlydepthstencil]`, required at 11_0 with shader
  model 5; OpenGL 4.2's `early_fragment_tests`; Vulkan's `EarlyFragmentTests`). The API runs the depth
  and stencil tests, and their writes, before the pixel shader: a fragment that fails never runs its
  storage writes, and one that passes keeps its depth and stencil writes even if the shader discards.
  WebGPU runs the fragment shader before both tests (its spec's render stages), lets storage writes land
  in any order, and WGSL has no attribute that forces early tests. The reflection says when it matters
  (`StageExtras::Pixel`: `early_tests`, `side_effects`, `discards`): a program with early tests and
  either storage or counter writes, or a discard while the draw writes depth or stencil, in a draw whose
  depth or stencil test is on. Then:
  - Depth and stencil not written by the draw (the usual case: order-independent transparency and
    light lists against an opaque depth buffer): the pass's depth-stencil attachment is read-only,
    which WebGPU lets a draw also bind as a texture. The front binds the variant
    `VariantFlags::EARLY_TESTS`: it reads depth at `EARLY_DEPTH_BINDING` and stencil at
    `EARLY_STENCIL_BINDING` in `HELPER_SET` at the fragment's pixel (each covered sample, from the
    input sample mask, on multisampled targets; for a format without stencil, any single-sampled
    unsigned integer texture is bound there, read only where the stencil test is on), applies the
    draw's compare functions (as `CompareFn`'s numbers, 1 to 8, zero for a test that is off; the
    cache keeps them, and a `wcontainers-gpu` unit test holds them), reference and
    read mask from `HelperValue::EarlyTests` to the fragment's depth with the raster block's bias added
    (the fixed bias applies after the shader, so the program adds it as `DepthBiasInShader` does), and
    discards before any side effect if no covered sample passes. The fixed tests still run after it on
    the unchanged buffer, so colour writes are those of the API (`EarlyTestsInShader`). Exact.
  - The draw writes depth or stencil: primitives drawn earlier in the draw change the buffer later ones
    are tested against, so each primitive is drawn twice, as in `TargetReadPerPrimitive`: first in a
    pass whose attachment is read-only, with the variant above and the fixed tests (writes off), then
    in a pass with the attachment writable, without a fragment stage and with the draw's own
    depth-stencil block, which makes exactly the API's depth and stencil writes, discards or not
    (`EarlyTestsPerPrimitive`). Exact, very slow, and expected to be rare (G4 counts it on real
    programs).
  WebGL 2 never meets it: shader model 5 needs 11_0, and it has no storage writes. A native back end
  whose API forces early tests would add a feature bit then.
- **Per-resource minimum level of detail** (Direct3D 11's `SetResourceMinLOD` with
  `D3D11_RESOURCE_MISC_RESOURCE_CLAMP`, Direct3D 12's `ResourceMinLODClamp` in every texture view, used by
  texture streaming). Microsoft's functional specification (5.8, read in
  `downloads/d3d11spec/` in scratch) defines it: the clamp is fractional and counts levels from the
  resource's most detailed one; sampling clamps the level of detail to it as the sampler's own minimum
  does (both apply); loads (`ld*`) of a level below the clamp's whole part, and gathers when the view's
  first level is below it, read the out-of-bounds result (zero); when the clamp passes the view's last
  level every access reads zero; `resinfo` (`GetDimensions`) is unaffected; copies, targets and
  `GenerateMips` are unaffected. Neither browser API has it. The front binds the variant
  `VariantKey::min_lod_clamped` for the clamped texture slots: each (texture, sampler) pair the program
  samples with (`Reflection::sampled_pairs`) samples through a merged sampler at
  `CLAMPED_SAMPLER_BINDING` plus the pair's index in `HELPER_SET`, whose minimum level is the larger of
  the sampler's and the clamp less the view's first level; loads and gathers compare their level with
  `HelperValue::MinLod` (the clamp in the view's numbering and the view's level count) and return zero
  below it, and every access returns zero past the last level (`MinLodClamp`). The clamp's value is not
  in the key: changing it makes a new merged sampler, never a new program. Exact. A merged sampler counts
  against the stage's samplers; a program that then needs more than the back end has is refused
  (`BindingsBeyondLimit`).
- **Fractional viewports** (Direct3D 10 to 12 and Vulkan give viewports as floats; a game may use a
  width of w/2 for an odd w). Native with `FRACTIONAL_VIEWPORT` (WebGPU, 4.11). WebGL 2's `viewport`
  takes integers: the front sets the enclosing whole-pixel viewport, binds the variant
  `VariantFlags::VIEWPORT_ADJUST` that scales and offsets the last vertex stage's clip x and y
  (`HelperValue::ViewportAdjust`, `x' = x * s + o * w`) so positions land where the fractional viewport
  puts them, and cuts the scissor to the pixels its API draws (`ViewportInShader`). Direct3D scissors
  to the viewport implicitly, with each edge rounded towards negative infinity (D3D11.3 functional
  specification, 15.7; wgpu on the RTX 3080's Direct3D 12 leaves the last row of a viewport ending at 13.75
  undrawn although that row's centre is inside), so its front cuts the scissor that way: exact, the
  half pixel at the left and top edges being a zone Direct3D leaves undefined. Vulkan clips to the
  viewport only, so its front cuts the scissor to the pixels whose centres the viewport covers: exact
  for single-sampled targets; on multisampled targets, samples of the edge pixels that lie outside the
  fractional viewport while their pixel's centre is inside are covered where the API would not cover
  them: a listed loss, at most part of one pixel row or column on each edge: edge samples on the other
  side of the fractional edge from their pixel's centre are drawn or left as the centre is, not as the
  sample is. A viewport past
  `Viewport`'s bounds (Direct3D bounds both edges by -32768 and 32767, so its widths reach past
  `max_texture_2d`) takes the same route: the whole-pixel viewport is one within the bounds that covers the
  attachments, and the adjustment puts positions where the API's viewport does. Exact, as the
  viewport clips nothing the attachments hold.
- Targets of more bytes per sample than `color_bytes_per_sample` (every back end counts them as WebGPU
  does, by `color_target_bytes`: each target from a multiple of its alignment, `rgba8unorm` as 8 bytes,
  so a front sizes the groups once for all back ends): each draw runs once per group of
  targets that fits, each run starting from a saved copy of depth and stencil, storage writes and
  queries kept only in the first run (`TargetsSplit`). Exact.
- Drawing with no targets (Direct3D 11's storage-only drawing): the back end's own attachment (4.7).
  Direct3D 11.1's forced sample counts are reported absent (section 9).
- Layered drawing (`SV_RenderTargetArrayIndex` from a geometry shader, for one-pass cube shadows and
  volume tables): the stage helper's output carries each primitive's layer, and the draw runs once per
  layer used, into a pass on that layer's view, keeping only that layer's primitives (`LayerSplit`).
  Exact. A pixel shader reading its layer or viewport index reads the run's (`HelperValue::Layer`,
  `HelperValue::Viewport`; zero where no earlier stage writes one). A primitive naming a layer at or
  past the views' layer count is drawn into their first layer and its pixel shader reads the layer it
  named, as Direct3D draws it: the geometry shader's pass-through and the pixel shader made with
  `LAYER_FROM_STAGE` keep it in layer 0's run (`HelperValue::LayerCount`) and pass the named layer
  as a flat value. The layer or viewport from the vertex shader (an optional Direct3D 11.3 capability) is
  reported absent. Several viewports: `MultiViewportSplit`.
- Vertex input: every format can be named (`VertexFormat` has every component count of every type),
  and every back end takes every one, converting what its API lacks itself (WebGPU: a shadow stream a
  compute program makes at the first draw reading the buffer through that layout, kept until the
  buffer is written; `VertexFormatConverted`). So no capability lists vertex formats, and a front
  fetches in the program only for its own reasons (step rates, below; integers read with the other
  signedness). Step rates
  other than one instance: natively where the back end has `INSTANCE_STEP_RATE` (WebGL 2's divisor),
  otherwise `InstanceRateInShader`. The interface (`DrawRange`'s doc) refuses a direct draw whose
  indices, instances, or vertices without indices reach past their buffers, and allows an indexed
  or indirect draw to fetch a vertex past a stream's end, giving a value from anywhere in the
  buffer, zero, (0, 0, 0, x), or the whole draw dropped, which WebGPU's vertex processing allows
  (checked: "Optionally (implementation-defined), empty vertexIndexList and return, cancelling the
  draw call"); the recording back end drops such a draw, the harshest of these, and counts it.
  Direct3D reads zero past the end of a vertex buffer: for draws without indices the front draws
  the part past the end with a zero buffer bound (`VertexFetchZeroed`), exact, except an indirect
  one, whose count only the GPU knows: WebGPU may skip it whole when it reads past the end, a listed
  loss (`IndirectFetchPastEnd`, counted at each indirect draw without indices fetching per-vertex
  streams); for indexed draws, indices past the end get what the back end gives; a listed loss. The exact route is the pulled
  variant for the draw's streams (`VariantKey::pulled`), which the Vulkan driver takes when its
  program enables robust access (15.4); here every indexed draw would pay for it, since Direct3D
  has no such switch.
- A shader's access to a buffer outside its bound range (Direct3D reads zero and drops the write
  for constant, raw and structured buffers) gets what the back end gives (`BindingResource`'s doc):
  on WebGPU a value from anywhere in the WebGPU buffer behind the binding, zero or (0, 0, 0, x), a
  store landing anywhere in that buffer, or the draw or dispatch partly dropped (checked: the WebGPU
  specification's out-of-bounds access in shaders; WGSL's out-of-bounds access: "any memory
  location(s) of the WebGPU GPUBuffer bound"), and WGSL may end the invocation with its outputs
  zero; a listed loss. The WebGPU back end holds small uniform buffers the program writes often in
  one shared WebGPU buffer, so such a load may read another of the device's buffers (a read only:
  that shared buffer holds no buffer a program writes).
  Typed buffers are exact: their route checks each element against the view's length
  (`HelperValue::TexelBufferView`). The exact route for the rest is the check in the program that
  the Vulkan driver makes (`RobustAccessInShader`, 15.4), which every program would pay for. The
  recording back end stops at such an access with an error naming it (a call it cannot carry out),
  so a front's tests over it find every one.
- Stream output that transform feedback cannot write as declared (gaps that must leave the buffer's
  bytes alone, several elements per buffer in separate mode) is captured whole into a scratch buffer and
  repacked by a second capture that also reads the destination's old bytes (`StreamOutRepacked`).
  Exact.
- **Vertex and instance numbers.** The common form's `vertex_index` and `instance_index` count as
  WebGPU's and Vulkan's do: they include the draw's first vertex (draws without indices) or base vertex
  (indexed draws) and its first instance (`DrawRange`'s doc, 4.4). Every back end gives exactly that:
  WebGPU natively; WebGL 2 natively for `drawArrays`' `first`, and for the bases it applies by binding
  streams at an offset (`BaseVertexRebind`) through two hidden uniforms its GLSL adds back: naga's GLSL
  writer already writes `instance_index` as `gl_InstanceID + naga_vs_first_instance` (a uniform naga
  declares, which the back end sets to each draw's first instance), and our naga fork gives
  `vertex_index` the same treatment with `naga_vs_base_vertex`, set to each indexed draw's base vertex
  (G6 makes the writer change and sends it upstream; G3 sets both uniforms). WGSL has no base vertex or
  base instance built-in (naga's WGSL writer refuses them), so the common form never holds them: the
  shaders crate turns them into a read of the draw's bases, as it does for the API values that leave
  the bases out. Direct3D's `SV_VertexID` is the vertex's place (draws without indices) or the index
  value (indexed draws), and `SV_InstanceID` starts at zero (functional specification 8.16 and 8.18,
  `downloads/d3d11spec/` in scratch; dxbc-spirv's own SPIR-V subtracts `BaseVertex` and `BaseInstance`
  for the same reason). OpenGL's `gl_InstanceID` leaves out the base instance too, and OpenGL 4.6's and
  Vulkan's `gl_BaseVertex`, `gl_BaseInstance` and `gl_DrawID` name the bases. The reflection lists
  these as `SystemValue::VertexId`, `InstanceId`, `BaseVertex`, `BaseInstance` and `DrawIndex`. A
  program reading any of them that needs a base gets the variant `VariantKey::draw_base`:
  - `DrawBaseSource::Helpers` for direct draws, on every back end: the program reads base vertex, first
    instance and draw number from `HelperValue::DrawBase`, which the front writes before each draw whose
    bases differ from the last (`DrawBaseInHelpers`, counted per helper write). The values are those of
    the API's draw, also when a route splits it (`VertexFetchZeroed`, `TargetsSplit`) or turns it into an
    indexed draw (`draw_fan_or_loop`, which keeps `vertex_index`), and a draw the front itself made
    indirect (`PredicationIndirect`) keeps this source, since its bases are known.
  - `DrawBaseSource::Arguments` for indirect draws, whose arguments live on the GPU (WebGPU, with
    `INDIRECT`): the front binds the argument buffer read-only at `DRAW_ARGUMENTS_BINDING` (11) in
    `HELPER_SET`, beside its indirect use (both are read-only uses, which WebGPU allows in one pass), and
    the program reads the base vertex and first instance there, at the word offset and indexed-or-not
    that `HelperValue::DrawArguments` gives (`DrawBaseFromArguments`). The front makes argument buffers
    with `BufferUsage::STORAGE` as well as `INDIRECT` (Direct3D 11's `DRAWINDIRECT_ARGS` buffers; every
    Direct3D 12 and Vulkan buffer). The binding counts against the stage's storage buffers, so a program
    then beyond the limit is packed (`StoragePacked`). WebGL 2 has no indirect draws (`IndirectReadBack`
    makes them direct), so it only meets the first source.
  Exact. A program that reads no number, the common case, has `DrawBaseSource::None` and costs nothing.
- **Video and packed-pair formats.** Microsoft's 10_0, 10_1 and 11_0 tables require `NV12`, `YUY2`,
  `420_OPAQUE`, `R8G8_B8G8_UNORM` and `G8R8_G8B8_UNORM`. No browser API has them; the interface has the
  two packed-pair formats and the front builds the others:
  - `R8G8_B8G8_UNORM` and `G8R8_G8B8_UNORM` (`Format::Rgbg8Unorm`, `Grgb8Unorm`; two pixels in four
    bytes sharing red and blue): sampled, loaded, mipmapped and CPU-locked, never targets. Back ends
    store them decoded as `rgba8unorm` on every write, with the bytes kept on the host for copies and
    read backs, as for BC textures (`BlockDecoded`, `HostCopyReadBack`; block 2 by 1). Exact: the
    decode is a byte copy per pixel, and filtering of the decoded texels is what Direct3D defines.
  - `YUY2` (two pixels in four bytes, Y0 U Y1 V): Direct3D views it only as `R8G8B8A8_UNORM` or `_UINT`
    of half the width, so the front makes an `rgba8unorm` texture of half the width, and its views,
    copies, maps and storage stores are those of that format. Exact, with no slow path.
  - `NV12` (an 8-bit luma plane, then a half-size plane of interleaved chroma pairs): Direct3D views it
    only through its planes, as `R8_*` (plane 0) and `R8G8_*` (plane 1), for sampling, targets and
    storage stores. The front makes two textures, `r8unorm` and a half-size `rg8unorm`, and each view
    names one; `Map`, `UpdateSubresource` and copies of the whole resource split and join the planes in
    the layout Direct3D gives (plane 1's rows after plane 0's, at the same row pitch). Storage stores
    into the planes as for the formats above. Exact, with no slow path.
  - `420_OPAQUE`: usable only by the video decoder and video processor; the front stores it as `NV12`.
  - The video abilities those tables also require (Video Decoder Support, Video Processor Input and
    Output) have no route: the Direct3D 11 front does not offer `ID3D11VideoDevice` (`QueryInterface`
    fails with `E_NOINTERFACE`, as with a driver without video acceleration) and `CheckFormatSupport`
    reports no decoder or video processor bits. This is a shortfall against the tables, listed in
    section 9 beside multisample counts; games play video through Media Foundation, which falls back to
    software decoding and uploads frames as ordinary textures.
- **Copies between multisampled textures on WebGL 2.** Direct3D 11 copies a multisampled subresource
  whole into another of the same sample count (`CopyResource`, `CopySubresourceRegion` without a box).
  WebGL 2 keeps multisampled textures as renderbuffers, which no program reads, `blitFramebuffer` cannot
  write into, and `readPixels` cannot read, so no exact route exists. The back end resolves the source
  into a pooled single-sampled texture with `blitFramebuffer`, then draws it into every sample of the
  destination (colour as sampled; depth by `gl_FragDepth` from the resolved depth, stencil by eight
  stencil-reference draws as in `StencilCopiedByDraw`) (`MultisampleCopyResolved`): a listed loss, the
  same as 4.7's for fitted attachments. A resolve or present of the copy right away is exact (the
  resolved value of equal samples is that value); drawing more into the copy where a primitive covers
  part of a pixel can differ there, and at 10_0 nothing else can see samples (multisample load is
  reported absent). WebGPU copies multisampled textures natively (`copyTextureToTexture` with equal
  sample counts).
- **Alpha-antialiased lines** (Direct3D 10 and 11's `AntialiasedLineEnable` with `MultisampleEnable`
  off; functional specification 3.4.4): the coverage of a one-pixel-wide rectangle around each line is
  multiplied into the pixel shader's `o0.a`, with an implementation's own filter kernel and "reasonable"
  tolerances. Neither browser API draws such lines, and no fixed state expresses it, so the raster block
  has no field for it. The front hands such line draws to the stage helper (section 7), which runs the
  vertex stage as for geometry shaders and draws each line as a rectangle widened by the filter kernel
  (the reference's 3 by 3 pixels), passing each fragment's distances to the line's axis and ends; the
  pixel program variant `VariantFlags::LINE_COVERAGE` multiplies target 0's alpha by the coverage
  computed from them (`AntialiasedLinesAsQuads`). Exact within the specification's tolerance, compared
  with the reference at G4. Direct3D 9 and OpenGL take the same
  route when they come (`GL_LINE_SMOOTH`, Direct3D 9's `D3DRS_ANTIALIASEDLINEENABLE`).
- **Which line rasterisation rule.** Direct3D 10 and 11 draw lines as quadrilaterals with
  `MultisampleEnable` on and by the aliased diamond-exit rule with it off. WebGPU leaves the algorithm
  undefined (its specification, "Line Rasterization": a rectangle around the segment or Bresenham's),
  and GLES likewise; neither lets a program choose, so the raster block has no field for it. Lines are
  drawn by the back end's rule: a listed loss, at most the pixels at a line's ends and along its edges
  (compared with the reference at G4 with the tolerance Microsoft's own conformance allows lines).
  Aliased lines into a multisampled target are the exception: Direct3D covers every sample of each
  pixel the rule picks, where the back ends cover the samples their own multisampled rule picks, a
  difference along the whole line. The front hands such draws to the stage helper, which draws each
  line as a rectangle covering every pixel the rule may pick and passes the line's window-space ends;
  the pixel program variant `VariantFlags::LINE_DIAMOND` keeps the pixels the diamond-exit rule picks
  and discards the rest (`AliasedLinesAsQuads`), exact.
- **Fractional constant depth bias.** OpenGL's `glPolygonOffset` units, Vulkan's
  `depthBiasConstantFactor` and Direct3D 12's `D3D12_RASTERIZER_DESC1::DepthBias` are floats;
  Direct3D 9 to 11's are integers. `RasterDesc::depth_bias` is a float; WebGPU's `depthBias` is an
  integer, so a bias with a fraction is native only with `FRACTIONAL_DEPTH_BIAS` (WebGL 2's
  `polygonOffset` takes float units) and otherwise `DepthBiasInShader`, exact.
- **Constant-alpha blend factors.** OpenGL's and Vulkan's `CONSTANT_ALPHA` and
  `ONE_MINUS_CONSTANT_ALPHA` are in the blend block; WebGPU has only the constant colour factors, so they
  need `BLEND_CONSTANT_ALPHA` (WebGL 2). Without it a front whose block uses no constant-colour factor
  sets the blend constant to the alpha in all four channels and uses `Constant` (exact, no cost); a block
  using both blends in the program (`BlendInShader`).

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
answer from the creation check on the calling thread (4.8), then, on the device thread, convert DXBC
through the shaders crate (or take it from the cache) and call `create_shader`; the program is
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
slots and mark binding sets dirty; at the draw the front makes a set for each stage whose slots changed (set 0
vertex, set 1 pixel, set 3 the helper constants; 6.4). `Map(WRITE_DISCARD)` on a constant buffer returns a
fresh shadow block; `Unmap` is `write_buffer(Discard)` (4.6). `UpdateSubresource` is
`write_buffer(Ordered)` or `write_texture` of the stand-in's copy, except that a write covering the
whole buffer, which is every `UpdateSubresource` of a constant buffer in Direct3D 11.0 (partial updates
come only with `UpdateSubresource1`), is `write_buffer(Discard)`: the same meaning, so older engines
that update constants this way per draw never split a pass (4.6). A constant buffer smaller than the size
its shader declares (dxbc-spirv declares a dynamically indexed one whole, 65536 bytes, and WebGPU binds no
smaller buffer) is moved into a buffer of that size whose bytes past its own are zero, as Direct3D reads
them, and its whole writes are padded with zeros (`ConstantBufferGrown`). `ClearView` is `clear_rects`,
`GenerateMips` is `generate_mips`, `ClearUnorderedAccessView*` on buffers is `clear_buffer` with the
element's bits. On textures it is a `dispatch` of a built-in compute program (WGSL in the shaders crate,
one per format class) writing the value through a storage view of the cleared range, where the view's
format has `STORAGE_WRITE`; `Uint` clears of a normalised or float format write the low bits of each
value as Direct3D defines. A view served through a buffer of its texels (`StorageFormatThroughBuffer`
or `AtomicsThroughBuffer`, 4.12) is cleared in that buffer instead: `clear_buffer` of the view's
range with the value encoded as one texel of the view's format (1, 2, 4, 8 or 16 bytes, which the
pattern takes; the row padding is cleared too, which nothing reads), and the buffer marked newer than
the texture. `ResolveSubresource` is `resolve` with its format argument (4.2). `Draw*` passes the context's `DrawCall`; `DrawIndexedInstancedIndirect` uses an
indirect range. `CopyResource` and `CopySubresourceRegion` close the pass and copy (`copy_texture` takes formats of
one typeless group, 4.12). `Dispatch` closes the
pass and calls `dispatch`. `Map(READ)` on a staging resource takes the read back started when the
copy into it was recorded (4.6), waiting only if it is still pending (or polling with `DO_NOT_WAIT`); `Map(WRITE)` on a staging resource the GPU wrote since its shadow was filled reads back
first (`StagingRefreshed`, 4.6). `SetResourceMinLOD` sets the texture's clamp and marks its binding sets
dirty (`MinLodClamp`, 4.12). A pixel shader with `[earlydepthstencil]` and storage writes:
`EarlyTestsInShader` or `EarlyTestsPerPrimitive` (4.12). Viewports are floats and pass as they are
where `FRACTIONAL_VIEWPORT`, otherwise `ViewportInShader` when one is fractional. `D3D11_QUERY_EVENT` (frame pacing and CPU-GPU sync): `End` records a `submit()` of the work so far and
keeps its fence; `GetData` answers from `fence_done` (`S_FALSE` until then), and a stand-in that is told
to wait spins on it as the program does, each poll one flush and reply. `GetData` on other queries polls `read_state`; occlusion counts come from the counting replay
and other queries from the front's own counts (4.12). `Present` goes to the DXGI layer (4.9).

`OMSetBlendState`'s sample mask goes into the raster block where `SAMPLE_MASK`, otherwise 4.12's route.
Overlapping queries: `QueriesSegmented` (4.7). A depth-stencil view larger than the render targets (which
Direct3D 11 allows; only the render targets must agree): the pass tracker fits them (`AttachmentsFitted`,
4.7). Buffers with both index and vertex bind flags are ordinary buffers (WebGL 2 holds them twice, 4.6).
`SetPredication`: kept with the hint flag, otherwise `PredicationWait` (4.12). `D3D11_QUERY_TIMESTAMP_DISJOINT`
answers its `Disjoint` member from `timestamps_disjoint` and its `Frequency` as 10^9 (timestamps are
nanoseconds). A depth bias on a 24-bit depth target where `DEPTH24_UNORM` is absent: `DepthBiasInShader`
(4.11). `resinfo` and `sampleinfo` on WebGL 2: `TextureInfoInHelpers` (4.12). Stream output with
`RasterizedStream` naming the captured stream: `capture_vertices` with `rasterize` on WebGL 2, one run of
the vertex work.

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

Vertex shaders reading `SV_VertexID` or `SV_InstanceID` with a non-zero `StartVertexLocation`,
`BaseVertexLocation` or `StartInstanceLocation`: `DrawBaseInHelpers`, or for
`Draw*InstancedIndirect` on WebGPU `DrawBaseFromArguments` (4.12). Typed storage views of
`R11G11B10_FLOAT`, `R16G16_*` and the other formats WebGPU stores into only with
`texture-formats-tier1`, on adapters without it: `StorageFormatThroughBuffer` (4.12). `NV12`, `YUY2`
and `420_OPAQUE` textures are built from ordinary ones, `R8G8_B8G8_UNORM` and `G8R8_G8B8_UNORM` are
`BlockDecoded`; `ID3D11VideoDevice` is not offered (4.12). `CopyResource` between multisampled
textures: native on WebGPU, `MultisampleCopyResolved` on WebGL 2 (a listed loss, 4.12). A rasterizer
state with `AntialiasedLineEnable` and `MultisampleEnable` off, on line draws:
`AntialiasedLinesAsQuads` (4.12). Line draws with both off into a multisampled
target: `AliasedLinesAsQuads` (4.12). `MultisampleEnable`'s choice between quadrilateral and aliased line
rasterisation for other line draws is a listed loss (4.12). A pixel shader's storage view read by a
later draw of the same targets as a shader resource, vertex or constant buffer: `UsageScopeSplit` (4.7). A staging copy of a depth texture read back: `DepthReadByDraw` on both
back ends (4.12).

### 5.2 Direct3D 12 (explicit)

`CreateGraphicsPipelineState` creates the blocks, the program and calls `prepare` with the PSO's target
formats and topology class, so WebGPU compiles in the background long before the draw; no first-draw
pause. Root signatures decide the binding sets: root constants become immediates (up to the back end's
`immediate_bytes`, beyond that `ImmediatesSpilled`); root descriptors (GPU virtual addresses) are
resolved through the front's address map to buffer ranges; descriptor tables become binding sets made
when the list is replayed at `ExecuteCommandLists`, from the descriptors as they are then (4.5). `ResourceBarrier` does nothing on either back end (both synchronise for
us); where passes end is the pass tracker's rule (4.7): a target change, a copy or dispatch, or a draw
that would mix a storage write with another use in one pass (`UsageScopeSplit`), which covers an
unordered-access-to-shader-resource barrier between two draws on the same targets. Command lists record into a
front-side list; `ExecuteCommandLists` replays it into the back end and submits. `Signal` on a fence
records `submit()` and its fence; `SetEventOnCompletion` and `GetCompletedValue` use `fence_done` and
`wait_fence`. Several queues (direct, compute and copy queues, which engines use for streaming and
async compute) share the back end's one submission order (4.8). The front keeps, per guest queue, the
work submitted to it in order. `ExecuteCommandLists` and `Signal` on a queue run at once unless an
earlier `Wait` on that queue is unmet; `ID3D12CommandQueue::Wait(fence, value)` is met when the fence's
value has been reached by work already replayed or by the CPU. While a queue's head waits, its later
work is held. When a `Signal` replays (another queue's work reaching it) or the program calls
`ID3D12Fence::Signal` from the CPU (typically after writing upload memory), the front releases every
queue whose wait is now met, in the order the program made the calls; so a command list is replayed only
after the work and the upload bytes it depends on exist, and nothing is reordered across a wait. Upload
memory written before a CPU signal is written to the GPU at the release (4.6). A wait that is never met
holds that queue's work for good, as the GPU would; `GetCompletedValue` keeps reporting the value
reached. `ResourceMinLODClamp` in a texture view is `MinLodClamp` with the view's fixed value (4.12). Upload heaps are persistently mapped guest memory written at submit (4.6); readback heaps
are finished into guest memory when their fence completes. `ExecuteIndirect`: a command signature of
only draw, indexed draw or dispatch arguments becomes one indirect call per command at the signature's
stride (`INDIRECT`); with a count buffer, the largest count is drawn after a small compute pass zeroes
the commands past the count (`IndirectCountExpanded`, no read back). A signature that also changes
vertex or index buffers, root constants or root descriptors per command has no GPU-side form in either
API: the argument and count buffers are read back to the device thread, which waits, decodes each
command (root descriptors through the address map) and records ordinary draws (`IndirectReadBack`).
Exact, slow. Bundles replay their recorded calls.

Placed resources and aliasing. Microsoft's "Memory Aliasing and Data Inheritance" (fetched in scratch,
`check6/spec/aliasing.md`) defines when an aliased resource keeps the bytes another one wrote: buffers
always inherit from overlapping buffers, in any state; two textures with identical descriptions
(including the optimised clear value and the creation method) at the same offset of a heap inherit;
buffers and textures inherit from each other only through standard-swizzle or row-major layouts. Every
other overlap leaves the new resource's contents undefined until a full write, which a copy or clear of
the whole subresource may be. The front gives each placed resource its own storage, as WebGPU must
(one buffer used for a placed vertex buffer and, through another placed buffer, as a writable storage
view in the same pass would break WebGPU's usage rules), and makes the defined cases exact by copying:
when a placed resource is created, the front lists the live placed resources it inherits from (the
overlapping buffers; for a texture, the identically described textures at the same offset). Each
placed resource carries a write stamp the front bumps whenever it records a GPU write into it (storage
view, target, copy or clear destination, resolve, query resolve, stream output). Before the front
records any use of a resource with such a list, it finds the listed resources written after this one
was last written or brought up to date, and copies from each of them, oldest write first, the bytes
they share (`AliasCopied`; for buffers `copy_buffer` of the overlap, for textures `copy_texture` of
each subresource); the resource is then up to date. Resources without
overlaps, the common case, cost nothing; `CopyBufferRegion` into one alias to initialise another
(which Microsoft's page for it mentions) works the same way. The check is the front's own and does not
depend on the program's aliasing barriers, so programs that omit them still get the bytes hardware
would give. Upload and readback heaps are guest memory (4.6), so their placed buffers share bytes
directly. Reported: `StandardSwizzle64KBSupported` FALSE (so no texture-buffer inheritance arises),
resource heap tier 2. `CopyBufferRegion` within one buffer: `CopyWithinBuffer` on WebGPU.

Vertex shaders reading `SV_VertexID` or `SV_InstanceID`: as Direct3D 11's (5.1); `ExecuteIndirect`'s
draws and the count-buffer route read their bases from the argument buffer (`DrawBaseFromArguments`).
Typed storage views of formats WebGPU cannot store into without `texture-formats-tier1`:
`StorageFormatThroughBuffer`.

Queries in Direct3D 12 live in query heaps and leave them only through `ResolveQueryData` into a buffer,
which the program then copies into a readback heap or hands to `SetPredication`. The front makes each
query heap an interface query set (occlusion heaps of `OcclusionAny` for `BINARY_OCCLUSION`, timestamps
for `TIMESTAMP`) and `ResolveQueryData` is `resolve_queries` into the destination buffer at its offset:
on WebGPU `resolveQuerySet` and a copy, nothing waits; elsewhere the default's wait
(`QueryResolveWait`). Binary occlusion must be exactly 0 or 1, and WebGPU promises only zero or non-zero,
so for those heaps a small compute program of the front's turns each resolved value into 0 or 1 in place.
Counted occlusion (`OCCLUSION`) comes from the counting replay (4.12), whose reduction writes the count
into a buffer of the front's (a dispatch on WebGPU), copied into the destination with `copy_buffer`.
Pipeline and stream output statistics are the front's own counts (4.12), written with an `Ordered`
`write_buffer`. `SetPredication(buffer, offset, op)` reads that buffer: `PredicationIndirect` on WebGPU,
`PredicationWait` for predicated clears, copies and resolves (4.12). Timestamp frequency is reported
as 10^9 by `GetTimestampFrequency`. Render targets and a depth buffer of different sizes in
`OMSetRenderTargets` or a render pass: `AttachmentsFitted` (4.7). A pipeline state's
`D24_UNORM_S8_UINT` depth with a constant depth bias where `DEPTH24_UNORM` is absent:
`DepthBiasInShader` (4.11); so is `D3D12_RASTERIZER_DESC1`'s float `DepthBias` with a fraction on
WebGPU (native on WebGL 2 would need Direct3D 12 there, which is not offered). Reported absent through `CheckFeatureSupport`: ray
tracing, mesh shaders, variable rate shading, tiled resources, rasterizer ordered views, conservative
rasterization, 64-bit shader integers; resource binding tier 1 without `RESOURCE_TABLES`, tier 3 with it.

### 5.3 OpenGL 3.3 (Mesa, second batch)

Mesa's state tracker calls a Gallium driver we write on the interface. Its `create_*_state` calls map to
our blocks; `set_*` calls mark dirty and the driver looks blocks up by value at the draw (4.4).
`glBufferSubData` is `write_buffer(Ordered)`; `glCopyBufferSubData` within one buffer is `copy_buffer`
(`CopyWithinBuffer` on WebGPU); `glMapBufferRange` with invalidate is `Discard`, with
unsynchronised `NoOverwrite`, with read `read_back_now`. `glReadPixels` is `read_back_now` (native on
WebGL 2). Fans and loops use `draw_fan_or_loop`. Transform feedback (core since 3.0) is
`capture_vertices` on WebGL 2 (with `rasterize` on when `GL_RASTERIZER_DISCARD` is off) and the stage
helper on WebGPU (`StreamOutAsCompute`). `GL_UNSIGNED_BYTE` indices: native on WebGL 2, `IndexWidened`
on WebGPU. Adjacency topologies without a geometry shader: `AdjacencyDropped`. Buffers are untyped, so a
buffer used as indices and as vertices is held twice on WebGL 2 (`IndexBufferTwin`, 4.6). Framebuffer
attachments of different sizes: `AttachmentsFitted` (4.7). `GL_TIME_ELAPSED` and `GL_TIMESTAMP` with
`GL_GPU_DISJOINT` from `timestamps_disjoint`. Geometry shaders
(3.2) go through the stage helper. Provoking vertex: native last on WebGL 2; on WebGPU `ProvokingRewrite`.
`glPrimitiveRestartIndex` with any index, or restart off: `RestartRewrite` (WebGPU and WebGL 2 restart only
at the largest value, always). `glLogicOp`: `LogicOpInShader`, one primitive at a time where primitives
may overlap (`TargetReadPerPrimitive`). `GL_SAMPLES_PASSED`: `OcclusionCountReplay`. `glPolygonMode`: native with
WEBGL_polygon_mode (lines; points need `POLYGON_MODE_POINT`), otherwise `FillModeConverted`. Clip depth from minus one: native on WebGL 2,
`ClipDepthRewrite` on WebGPU. Multisample textures (3.2): WebGPU has them; WebGL 2 does not, so Mesa
reports OpenGL 3.1 there (3.1 needs texture buffers: `TexelBufferAsTexture`). Wide lines report a range of
1 to 1, which core profile allows. Border colors: `BorderInShader` on both. `GL_TEXTURE_LOD_BIAS`:
`LodBiasInShader`. `glDepthRange` with near beyond far: `ViewportDepthFlip`. Cube map and array targets
set `TextureDesc::cube` and the layers as the target says; 1D textures are one texel high.
`gl_VertexID` and `gl_InstanceID` are the common form's numbers as they are: OpenGL's `gl_VertexID`
includes `first` and the base vertex as WebGPU's does, and 3.3 has no base instance. `GL_LINE_SMOOTH`
(still core): `AntialiasedLinesAsQuads` (4.12). `glPolygonOffset` units with a fraction: native on WebGL
2 (`FRACTIONAL_DEPTH_BIAS`), `DepthBiasInShader` on WebGPU. `GL_CONSTANT_ALPHA` and
`GL_ONE_MINUS_CONSTANT_ALPHA`: native on WebGL 2 (`BLEND_CONSTANT_ALPHA`); on WebGPU the constant
colour factors with the alpha in all four channels, or `BlendInShader` when the block also uses the
constant colour (4.12).

### 5.4 OpenGL 4.6 (Mesa, second batch, WebGPU only)

Adds over 3.3: tessellation (stage helper, `TessellationAsCompute`); 64-bit floats (4.0), which neither
browser has: `F64Emulated` through Mesa's soft-float lowering, slow and counted; image load and store and
atomic counters (storage textures and buffers); compute (`dispatch`); multi-draw indirect (one indirect
draw each; with a count read from a buffer (4.6), `IndirectCountExpanded`); query buffer objects (4.4)
by `resolve_queries`; SPIR-V shaders (into the
common form through naga's SPIR-V reader, section 6); clip control (`ClipDepthRewrite`); anisotropic
filtering (native). `layout(early_fragment_tests)` with image stores: `EarlyTestsInShader` or
`EarlyTestsPerPrimitive` (4.12). An image or storage buffer written by one draw and read another way
by a later one after `glMemoryBarrier`: `UsageScopeSplit`; one buffer bound as a storage buffer and, at
another range, a uniform or vertex buffer of the same draw: `UsageScopeCopied` (4.7). Cull distances are absent on WebGPU, and a discard cannot make them (they drop a primitive only when
all its vertices are outside one plane, which no fragment can tell), so they take the stage helper's
route of 4.12 (`CullDistanceAsGeometry`).
`gl_InstanceID` with a base instance (4.2), and `gl_BaseVertex`, `gl_BaseInstance` and `gl_DrawID`
(4.6): `DrawBaseInHelpers`, or `DrawBaseFromArguments` for indirect draws (4.12). On WebGL 2 the version
stays at 3.1.

### 5.5 Vulkan 1.1 (Mesa Vulkan runtime, second batch, WebGPU only)

Pipelines are complete block sets plus `prepare`: no stutter. Descriptor sets become binding sets (made
when the set is updated, not per draw). Push constants are immediates (Vulkan's minimum is 128 bytes;
WebGPU's default immediate size is 64, the rest is `ImmediatesSpilled`). Render passes map to passes,
subpasses to one pass each, input attachments to sampled textures in the next pass. Barriers and events
do nothing; the pass tracker splits a pass where a storage write and another use would meet in one
(`UsageScopeSplit`), and a draw using one buffer both ways reads a copy (`UsageScopeCopied`, 4.7).
`depthBiasConstantFactor` with a fraction: `DepthBiasInShader`; `VK_BLEND_FACTOR_CONSTANT_ALPHA`: as
OpenGL's (5.3). Host-visible coherent memory, which Vulkan requires, is guest memory written at submit
(4.6). The other direction, which is Vulkan's ordinary read back (`vkCmdCopyImageToBuffer` or a shader
writing into a buffer bound to `HOST_VISIBLE` memory, then `vkWaitForFences`, then reading the mapped
pointer), works as Direct3D 12's readback heaps: at submit, the front lists every range of host-visible
memory the submitted commands may write (copy and clear destinations, query results, and storage buffers
the bound programs write, from their reflection), and records `read_back` of each after the work.
When the program learns the work is done (`vkWaitForFences`, `vkGetFenceStatus` or
`vkQueueWaitIdle` answering done, `vkDeviceWaitIdle`, a timeline semaphore wait), the front first
finishes those read backs into guest memory, so the bytes are there when the program looks. The memory
types offered keep this to memory the program chose for it: one `DEVICE_LOCAL` type that is not
host-visible, one `HOST_VISIBLE | HOST_COHERENT` for uploads and one with `HOST_CACHED` added for read
backs; no type is both device-local and host-visible, so a program that renders into memory it maps
picks that on purpose.
Secondary command buffers replay. Several queues are ordered by their semaphores as Direct3D 12's by
fences (5.2). `vkCmdCopyQueryPoolResults` is `resolve_queries` (with `VK_QUERY_RESULT_64_BIT`; 32-bit
results and availability words are written by a small compute program of the front's from the resolved
values); `vkGetQueryPoolResults` is `read_queries`. Conditional rendering
(`VK_EXT_conditional_rendering`, an extension 1.1 does not require) reads a buffer:
`PredicationIndirect` (4.12). A framebuffer whose attachments are larger than it, or of different sizes:
`AttachmentsFitted` (4.7). A `D24_UNORM_S8_UINT`-style 24-bit depth format: `DEPTH24_UNORM` decides
whether Vulkan reports `VK_FORMAT_X8_D24_UNORM_PACK32` and `VK_FORMAT_D24_UNORM_S8_UINT` (with it) or only
`D32_SFLOAT` and `D32_SFLOAT_S8_UINT` (without it), so programs pick a format whose bias is exact; Vulkan
lets an implementation omit the 24-bit ones. `vkCmdCopyBuffer` within one buffer: `CopyWithinBuffer`.
`VertexIndex` and `InstanceIndex` are the common form's numbers as they are; `shaderDrawParameters`
(`BaseVertex`, `BaseInstance`, `DrawIndex`) is reported present, served by `DrawBaseInHelpers` and
`DrawBaseFromArguments` (4.12). `EarlyFragmentTests`: as
Direct3D's (4.12). Fractional viewports are native. A viewport whose `minDepth` exceeds `maxDepth` (allowed by Vulkan
and used for reversed depth): `ViewportDepthFlip`. A sampler's `mipLodBias`: `LodBiasInShader`; the
`maxSamplerLodBias` limit is reported as 15.99, the range Direct3D also allows. `VK_IMAGE_CREATE_CUBE_COMPATIBLE_BIT`
sets `cube`. Vulkan's features and subgroup properties are reported as 15.4 lists them: each of
1.1's 67 features is named there, present (natively, by a program rewrite, or through a counted slow
path; engines ask for them at start-up, research vulkan-on-webgpu-inputs Q5a) or absent with its
reason. WebGL 2 cannot host Vulkan (Vulkan requires compute).

### 5.6 Direct3D 9 (later, with 32-bit x86)

Fixed function uses one general program whose behaviour comes from immediates and a uniform block, so
state changes never make new programs (brief). Shaders 1 to 3 convert through dxbc-spirv's SM3 path.
`DrawPrimitiveUP` writes its vertices into the `Data` ring; `DrawIndexedPrimitiveUP` writes its vertices
there and its indices into the `Indices` ring, two calls (`write_buffer_ring`). A depth surface larger
than the render target, which Direct3D 9 allows: `AttachmentsFitted` (4.7). `Lock` flags
map to write modes as in 4.6. Fans are native in Direct3D 9: `draw_fan_or_loop`. Point sprites and point
size: native on WebGL 2 within its range, `PointsAsQuads` on WebGPU. Border address mode:
`BorderInShader`. Depth bias on lines: `DepthBiasInShader` on both. `D3DSAMP_MIPMAPLODBIAS`:
`LodBiasInShader`. `GetRenderTargetData` is
`read_back_now`. Clip planes and fog are program features. The half-pixel offset is applied in the
converted vertex shader.

### 5.7 The WebGPU back end carrying these out

- Blocks to pipelines through the hash map of 4.4; `prepare` calls `createRenderPipelineAsync` with at
  most as many compiles in flight as the host's logical processors (research: more stalls the page).
- Binding sets to bind groups, cached (4.5), uniform bindings with dynamic offsets up to the adapter's
  limit, so per-draw constants make no bind group. Each draw's helper constants are two entries of
  bind group 3, one per stage (6.4). Immediates by `setImmediates` where present (Chrome 149+),
  else a ring buffer bound with a dynamic offset. Naga modules are validated with WebGPU's core
  capabilities (6.3).
- Writes as in 4.6: renaming pool for `Discard` (the uniform ring for frequently written uniform
  buffers), staging and copy for `Ordered`, direct for `NoOverwrite`. One `submit` per frame or per wait (submit costs about 13 µs per call, research).
- Read backs: copy into a `MAP_READ` buffer, then `mapAsync`; `wait_read` suspends through JSPI; the
  answer is copied from the mapped range into guest memory by `finish_read`.
- In the back end itself: fans and loops by the default conversion; unaligned buffer writes, copies and
  clears (`UnalignedBufferEdge`); passes without targets on its own attachment; formats WebGPU lacks
  (three-component 8 and 16-bit vertex formats, read through shadow streams; B5G6R5 and other 16-bit
  packed formats, A8, converted on upload) (`VertexFormatConverted`, `FormatConverted`); BC textures decoded where `BLOCK_COMPRESSION` is
  absent, and the packed-pair formats `Rgbg8Unorm` and `Grgb8Unorm` always, with host blocks for copies
  and read backs (`BlockDecoded`, `HostCopyReadBack`); copies
  between a BC and an uncompressed texture of the same block size, and between formats of one typeless
  group, through a buffer (`CopyThroughBuffer`), into `depth32float` by a draw (`DepthWrittenByDraw`);
  copies within one buffer through a temporary one (`CopyWithinBuffer`); integer colour clears past
  2^24 where the API clears through a float (wgpu's native Direct3D 12), by a draw writing the value
  (`IntegerClearByDraw`), and 32-bit float colour clears of denormals or NaNs, whose bits a clear
  value would change, by a draw writing the bits (`FloatBitsClearByDraw`); the 24-bit depth probe
  (4.11).
  The fast-path defaults it keeps: `clear_texture`, `clear_rects`, `generate_mips` (filtering in the
  program for `r32float` and the like without `float32-filterable`, and for `NormAsInteger` textures),
  `write_buffer_ring`, `draw_fan_or_loop`. Its override: `resolve_queries` by `resolveQuerySet` and a
  copy (4.3), and `prepare`.
- What fronts do on WebGPU, from its `Caps` (all program variants of 4.5, made by the shaders crate):
  restart off or another index (`RestartRewrite`); provoking last (`ProvokingRewrite`); depth bias on
  lines and points (`DepthBiasInShader`); polygon mode (`FillModeConverted`); multiple viewports and
  layers, only reachable from geometry shaders (`MultiViewportSplit`, `LayerSplit`); typed buffers
  (`TexelBufferAsStorage`); border colours, mirror once, filtering of unfilterable formats and
  level-of-detail bias (`BorderInShader`, `MirrorOnceInShader`, `FilterInShader`, `LodBiasInShader`);
  storage bindings at unaligned offsets (rebased, 4.5); a storage write and another use of one resource
  in one pass (`UsageScopeSplit`, `UsageScopeCopied`, 4.7); a fractional constant depth bias
  (`DepthBiasInShader`); constant-alpha blend factors (the constant colour, or `BlendInShader`);
  exact occlusion counts (`OcclusionCountReplay`);
  instance step rates and vertex streams past the limits (`InstanceRateInShader`,
  `VertexPulledInShader`); constant buffers past the limit (`ConstantsAsStorage`); fat targets
  (`TargetsSplit`); clip depth from minus one and swapped depth ranges (`ClipDepthRewrite`,
  `ViewportDepthFlip`); atomics on storage textures (`AtomicsThroughBuffer`); 16-bit normalised formats
  and rendered snorm ones without `texture-formats-tier1` (`NormAsInteger`, `BlendInShader`);
  overlapping queries (`QueriesSegmented`); stencil values per face (`StencilFacesSplit`); early depth
  and stencil tests with side effects (`EarlyTestsInShader`, `EarlyTestsPerPrimitive`); per-resource
  minimum level of detail (`MinLodClamp`); staging maps for writing after GPU writes
  (`StagingRefreshed`); attachments of different sizes (`AttachmentsFitted`); 8-bit indices
  (`IndexWidened`); adjacency without a geometry shader (`AdjacencyDropped`); predicates of recorded
  command lists (`PredicationIndirect`), other predicates (`PredicationWait`); constant depth bias into
  24-bit depth stored as float (`DepthBiasInShader`, where `DEPTH24_UNORM` is absent); the API's vertex
  and instance numbers and draw bases (`DrawBaseInHelpers`, `DrawBaseFromArguments`); typed storage
  stores of formats without `texture-formats-tier1` (`StorageFormatThroughBuffer`); alpha-antialiased
  lines (`AntialiasedLinesAsQuads`); Direct3D 12's aliased placed resources (`AliasCopied`).
- Absent and reported as such in the back end's own `Caps`: transform feedback (the stage helper
  builds stream output instead), logic ops, wide lines, sample counts other than 1 and 4, 64-bit
  shader types, resource tables, persistent mapping, sparse resources. These are the back end's
  native abilities, not what a front reports to its program: a front may still serve a feature of
  its API through a counted slow path, as Vulkan's logic ops, wide lines and 64-bit types are
  (`LogicOpInShader`, `WideLinesAsQuads`, `Int64Emulated`, `F64Emulated`, 15.4).

### 5.8 The WebGL 2 back end carrying these out

- Blocks applied by difference (4.4); programs linked per pair, in parallel where
  KHR_parallel_shader_compile is present, otherwise at first draw (`PipelineMiss`).
- Window origin: the interface's is top-left (row 0 of a texture is its top, as in WebGPU, Direct3D and
  Vulkan); GL's is bottom-left. The back end keeps every texture, renderbuffer and read back in the
  interface's order and draws upside down in GL's terms, so 6.5 decides it once for everything.
- Binding sets to units and uniform-block bindings; combined samplers per pair (4.5). Uniform buffers
  are std140 in GLSL ES; Direct3D's constant buffer packing fits std140 for the layouts dxbc-spirv
  produces (expected; checked by the corpus at G6). Immediates are plain uniforms set with `uniform4uiv`
  only when changed.
- Buffers holding both indices and other data as two GL buffers (`IndexBufferTwin`,
  `IndexTwinReadBack`, 4.6); the fast-path defaults never make one (two rings).
- Writes: `bufferSubData` (ordered by GL); for `Discard`, `bufferData(size, null)` at the buffer's size
  (orphaning), then `bufferSubData` of the part (4.6). Chrome charges a
  `bufferSubData` into a buffer a queued draw uses in proportion to the whole buffer (research: 0.9 ms at
  16 MB), so `MapIntent::Often` buffers are kept small and renamed.
- Read back now: `readPixels` and `getBufferSubData`, inside the call. Depth and stencil, which
  `readPixels` cannot take, are drawn into a colour texture first (`DepthReadByDraw`, 4.12).
- Fans and loops native; 8-bit indices native; transform feedback native, with or without
  rasterising, for lists without indices (GLES 3.0 captures nothing else); strips, fans, loops and
  indexed draws are captured as points in list order, one draw per run of consecutive vertices, with
  indices read back first, and drawn again for the picture when rasterising (`CaptureExpanded`); `resolve_queries` by its default (`QueryResolveWait`), since GLES 3.0 gives query results
  only to the host; `timestamps_disjoint` from `GPU_DISJOINT_EXT`; the 24-bit depth probe (4.11); base vertex and first instance by binding the vertex
  streams at an offset (`BaseVertexRebind`), with the hidden uniforms `naga_vs_base_vertex` and
  `naga_vs_first_instance` set per draw so `vertex_index` and `instance_index` still include them
  (4.12); indirect draws read their arguments back first
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
  formats of one typeless group through a pixel pack buffer (`CopyThroughBuffer`), multisampled
  textures by resolving and drawing into every sample (`MultisampleCopyResolved`, a listed loss, 4.12),
  formats it cannot
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
  (`NormAsInteger`, `BlendInShader`); fractional viewports (`ViewportInShader`); per-resource minimum
  level of detail (`MinLodClamp`); staging maps for writing after GPU writes (`StagingRefreshed`);
  texture level and sample counts (`TextureInfoInHelpers`); attachments of different sizes
  (`AttachmentsFitted`, with the multisample loss of 4.12); adjacency without a geometry shader
  (`AdjacencyDropped`); predicates (`PredicationWait`); constant depth bias into 24-bit depth stored as
  float (`DepthBiasInShader`, where `DEPTH24_UNORM` is absent); the API's vertex and instance numbers
  (`DrawBaseInHelpers`); alpha-antialiased lines (`AntialiasedLinesAsQuads`).
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
  writer refused 11 that use `isnan`, which WGSL lacks. The harness's own verdict file counts 107 of
  the 112 as passing, because it revalidated the written WGSL with naga and no optional capabilities at
  all (`Capabilities::empty()`), stricter than WebGPU: three shaders reading `sample_index` need naga's
  `MULTISAMPLED_SHADING` and two using `pack2x16float` and `unpack2x16float` need
  `SHADER_FLOAT16_IN_FLOAT32`, both of which WebGPU's core language has (rerun with the error text on
  2026-11-17 in run 8, load average 31.03, 63.57, 50.76: `corpus/revalidate-run8.txt`; the three pass
  with `MULTISAMPLED_SHADING`). So the WebGPU back end and the shaders crate validate with the
  capabilities WebGPU's core gives (G2 and G6 list them from wgpu's WebGPU set), never with none.
- GLSL ES 3.00: naga wrote 95; Chrome's WebGL 2 compiled 80, with every extension the context offers
  enabled first (as the back end does; `chrome-compile.mjs` did not enable them before run 6, which made
  the two conservative depth shaders and the sample-variables one fail, rerun on 2026-11-17, load average
  27.88, 27.60, 28.43). The 15 failures: features GLSL ES 3.00 lacks, bitfield insert, extract and
  reverse (7), `textureGather` (6) and multisampled samplers (1); and one naga GLSL writer bug, which
  assigns `gl_SampleID` (an `int`) to a `uint` without a conversion (1, `corpus/dxbc-via-spv-chrome-run6.txt`;
  G6 fixes it in naga's writer). naga refused 28 more: storage buffers (14, absent on WebGL 2),
  `textureQueryLevels` (11), sample counts (2), overrides (1).

DXIL: 8 shaders written for this test (vertex transform, lit pixel shader with `clip`, multiple targets
with depth output, compute reduction with group shared memory, wave operations, storage image, dynamic
resources through `ResourceDescriptorHeap` at SM 6.6, typed buffer with comparison sampling), compiled by
DXC to DXIL. dxil-spirv converted 8 of 8 to SPIR-V; naga read 1 of 8 (the multiple-target shader failed
validation, six failed in the reader for the gaps listed in 6.2); that one compiled in Chrome as WGSL and
GLSL ES 3.00.

The interface's own built-in programs (4.3): `WCONTAINERS_GPU_BUILTINS_OUT=<dir> cargo test -p
wcontainers-gpu` writes each of the 21 as WGSL and the 19 WebGL 2 makes as GLSL ES 3.00 (one file per
entry point), and `tools/chrome-compile.mjs <dir>` compiles them in Chrome: on 2026-11-17, on this Mac
(load average 25.87, 28.86, 35.02), 21 WGSL modules and 37 GLSL ES files compiled, none failed
(`g0-run5/builtins/` in scratch). Before each program had its own module, naga refused every entry
point for GLSL ES 3.00 because the shared module declared a cube-array texture.

What follows for the plan:
- The chosen route, lowering dxbc-spirv's own form straight into naga, was not tried in G0. It is
  expected to remove the reader's failures (they are failures of naga's SPIR-V reader, a step the direct
  route does not have) and to let us emit what WGSL and GLSL ES 3.00 lack in their own terms (6.5); G6
  measures it on the same corpus first, and falls back to the measured SPIR-V route if it does worse.
- DXIL depends on naga's SPIR-V reader growing; that work is G6's first task, measured on the same
  corpus plus real DXIL from games.
- Vertex shaders reading `SV_VertexID` or `SV_InstanceID`: dxbc-spirv's SPIR-V subtracts the
  `BaseVertex` and `BaseInstance` built-ins from `VertexIndex` and `InstanceIndex`, and naga's reader
  keeps them as entry arguments, which WGSL cannot express (naga's WGSL writer has no form for them).
  G0's sixth independent check reran `corpus/dxbc/047_vs_code_4.dxbc` and got exactly that, and then a
  validation failure for integer stage values without flat interpolation. The direct lowering never
  emits the base built-ins: it reads the bases as 4.12 says (`VariantKey::draw_base`), and it marks
  integer stage values flat. dxil-spirv does the same subtraction for DXIL, so the reader path gets the
  same rewrite in the shaders crate.

### 6.4 One binding rule, the reflection and the conversion API

The shaders crate maps every API's resource slots onto binding numbers the same way for every front,
and records the mapping in the converted shader's `Reflection`. The types are in `wcontainers-gpu`
(`reflect.rs`: `SlotKind`, `Reflection`, `ReflectedBinding`, `StageValue`, `StageExtras`, `HelperValue`,
`HelperSlot`, `VariantKey`, `VariantFlags`), so the shaders crate (G6) and the fronts (G4, G5) build
against the same Rust from the start; back ends never read them.

- Binding numbers within a set: range start of the slot's kind plus the API's slot (`SlotKind`:
  uniform 0 to 15, sampler 16 to 31, texture and typed buffer 32 to 159, storage 160 to 223, the hidden
  counters of storage slots 224 to 287).
- Sets, Direct3D 9 to 11 (slots per stage): set 0 the vertex stage's slots (or a compute program's),
  set 1 the pixel stage's (with Direct3D 11's pixel-stage storage views), set 2 empty, set 3 the helper
  constants (`HELPER_SET`). Geometry, hull and domain shaders run through the stage helper, which binds
  them the same way in its own programs. These are the only sets: the front makes one set per stage
  whose slots changed, never one per kind of slot.
- Sets, Direct3D 12: the stage sets as for Direct3D 11. Within a set, each kind's slots are the
  program's own resources of that kind in (register space, register) order, from the kind's first
  binding, so any space and register fits while a program uses no more resources of a kind than its
  range holds (`BindingsBeyondLimit` past it); kinds follow Direct3D's register classes (shader resource
  views are textures, unordered access views storage). The numbering is the program's alone (the root
  signature, `VariantKey::layout`, enters the key only for root constants read as immediates); the
  front finds each binding's descriptor in the bound root signature
  by its `ReflectedBinding::space` and `register`. A binding's slot (`ReflectedBinding::slot`, its place
  in its kind's range) is what `VariantKey`'s per-slot fields and the sampled pairs name; under Direct3D
  11's numbering it is the register. An array of resources is a binding per element at its own
  register; an array declared without a length inside a descriptor range of finite length is the
  rest of that range, which the front gives as `VariantKey::register_ranges` (the root signature's
  ranges by kind and space). Unbounded ranges (bindless) need `RESOURCE_TABLES`.
- Bindings a variant adds, for every API: pulled vertex streams, constant buffers read as storage or
  through a texture, the layers after the first of a multisampled array texture, and (Direct3D 12) root constants spilled past `immediate_bytes` as one uniform
  buffer (`ImmediatesSpilled`). Each lives in the set of the one stage that reads it (set 0 for vertex
  and compute programs, set 1 for pixel), numbered in its kind's range from the first binding past the
  highest one of that kind the stage's program already uses, and the reflection names it
  (`ReflectedBinding::added`, `AddedBinding`). Root constants every stage reads are bound in both
  stages' sets, the same buffer range twice. Because each stage's additions live in its own set, two
  stages converted apart never number into one space.
- Helper constants: one uniform block per program stage, laid out by the shaders crate for that stage
  alone (`Reflection::helpers` gives each `HelperValue` its byte offset in its own stage's block). Each
  stage's block has its own binding in set 3 (`helper_binding(stage)`: 0 for vertex and compute, 12 for
  fragment), so on WebGPU they are two entries of bind group 3 and on WebGL 2 two uniform blocks with
  their own binding points, and the vertex stage's base vertex and the pixel stage's level-of-detail
  bias, both at offset 0 of their blocks, never meet. The front writes each block with
  `write_buffer_ring` when one of its values changed and binds both in set 3; a value both stages need
  (a storage offset of a Direct3D 12 range visible to all stages) is written into each. The values: storage offset remainders, samplers' level-of-detail bias,
  border colour and state (filters and address modes by the fixed numbers of `Filter` and
  `AddressMode`, which converted programs and so the cache keep) and the components of a texture's
  format (`Format::components`) (for filtering and addressing in the program), texture level and sample
  counts where the back end's language cannot ask (`TEXTURE_INFO`), depth bias, pulled streams' offset, stride and step,
  the sample mask, packed storage offsets, the texel layout of storage views read through a buffer (pitches
  and the view's size), where a typed buffer view read through a storage buffer or a texture starts and how
  many elements it has, the layer and viewport of the draw's run,
  how each target combined in the program blends, the early tests a program runs itself, a texture's
  minimum level-of-detail clamp, the viewport adjustment, and the draw's bases or where an indirect
  draw's arguments are (`DrawBase`, `DrawArguments`) (`HelperValue`). Set 3 also holds the
  copies of targets such programs read, from `TARGET_COPY_BINDING` (1 to 8); the read-only depth and
  stencil of early tests in the program, at `EARLY_DEPTH_BINDING` (9) and `EARLY_STENCIL_BINDING` (10);
  the argument buffer of an indirect draw whose program reads the bases, at `DRAW_ARGUMENTS_BINDING` (11);
the hull stage's patches and the tessellator's points a domain program reads, at `DOMAIN_PATCHES_BINDING`
(13) and `DOMAIN_POINTS_BINDING` (14), buffers the stage helper owns;
  a storage buffer a front's own counting program adds to, at `COUNTER_BINDING` (15);
  and the merged samplers of clamped textures, from `CLAMPED_SAMPLER_BINDING` (16) plus the index of
  the pair in `Reflection::sampled_pairs`.
- Variants: `VariantKey` (4.5) is part of the converted shader's cache key. Its per-slot fields carry a
  bit per API slot; the typed buffer route is one field; the sample count, dual-source blending, the
  clip-depth rewrite, the depth flip, depth bias, the provoking rewrite, the sample mask, the counting
  variant, early tests in the program, the viewport adjustment, texture level and sample counts from
  the helpers (`TEXTURE_INFO`), line coverage (`LINE_COVERAGE`), clip distances and depth clamping in
  the program (`CLIP_DISTANCE`, `DEPTH_CLAMP`) and the primitive index from the stage helper
  (`PRIMITIVE_ID_FROM_STAGE`) and a geometry shader's replay form (`GEOMETRY_REPLAY`, section 7)
  are its flags; the clamped texture
  slots are `min_lod_clamped`; storage slots run through a buffer are `texels_through_buffer`; where
  the draw's bases come from is `draw_base`; a view's component mapping, applied by the program
  (`SwizzleInShader`; views carry no mapping, 15.7), is `swizzled`.
- Slow paths a program takes by itself, whatever its variant (an atomic minimum or maximum of the
  other signedness than its memory's, made by a compare-exchange loop: `AtomicExchangeLoop`), are
  listed in `Reflection::slow_paths`; the front counts and warns of them when it uses the program.
  The cache stores each by its number, so a reason keeps its number: new ones go at the end of
  `SlowPath`, and a unit test holds every number given so far.

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
`texelFetch` reads at the gather footprint; conservative depth hints kept with EXT_conservative_depth and
dropped without it (they only allow optimisation); `gl_SampleID` converted where it meets unsigned values;
`instance_index` written as `gl_InstanceID + naga_vs_first_instance` (naga's own) and `vertex_index` as
`gl_VertexID + naga_vs_base_vertex` (our naga fork, 4.12), uniforms the back end sets per draw.
Window origin on WebGL 2, decided here once for the back end. The interface's origin is top-left: a
texture's row 0 is its top row and the first row of bytes written or read, viewports, scissors and
copies count y downward, and the fragment position's y grows downward, as in WebGPU, Direct3D and
Vulkan. GL's window origin is bottom-left, but GL itself only ever stores row 0 first: `texImage2D`
takes the first row of bytes as row 0, `readPixels` returns row 0 first, and `glViewport`, `glScissor`,
`blitFramebuffer` and `copyTexSubImage2D` count from row 0. So the back end keeps every interface y as
GL's y unchanged and makes the rasteriser put the image's top on row 0:
- With EXT_clip_control (93.2%), the device sets `clipControlEXT(UPPER_LEFT_EXT, ZERO_TO_ONE_EXT)` once.
  Clip y = +1 then lands on row 0, `gl_FragCoord.y` grows downward as Direct3D's `SV_Position` does,
  derivatives in y keep Direct3D's sign, and the extension keeps front-facing as clip space orients it,
  so cull and front-face state pass unchanged.
- Without it, the GLSL writer's last step in the last vertex stage is `gl_Position.y =
  -gl_Position.y` (the back end's own fix; naga's `ADJUST_COORDINATE_SPACE` is not used, because it also
  rewrites z as `2z - w`, which the fronts' `ClipDepthRewrite` variant already decides), and the back
  end swaps `frontFace` (clockwise for counter-clockwise), since flipping y reverses every triangle's
  winding in window space; `gl_FrontFacing` follows the swapped setting and stays right. Row 0 is again
  the top, so `gl_FragCoord`, derivatives, viewports, scissors, copies and read backs need nothing more.
- Presenting: the drawing buffer shows row 0 at the bottom of the canvas, so the present copy (4.9),
  already one draw, samples with v flipped. No other step changes.
The built-in programs of `wcontainers-gpu` (whose full-screen triangle maps uv (0, 0) to clip (-1, 1))
therefore write level row 0 from source row 0 on both back ends; G3's tests check a mip chain and a
clear rectangle by read back against the WebGPU back end's, and both origin routes with a culled,
one-sided triangle and a `gl_FragCoord` gradient (the facing rule under EXT_clip_control is read from
the extension's text, not yet tested). The GLSL the crate's own test writes with
`WriterFlags::empty()` is the module before this fix, which G3 adds.

Each of these needs nothing beyond the module, which is why a back end may do it while writing its
language. Texture level and sample counts are not among them: their values come from the bound views, so
the shaders crate reads them from the helper constants when the front asks for
`VariantFlags::TEXTURE_INFO` (4.12), and the back end never sees the question. Absent on WebGL 2 and
reported so by the feature level: storage, compute, multisampled reads.

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
  only its own emitted vertex and collapsing unused slots (`GeometryAsReplay`). Stream output after
  it is written on the host from a read back of the replay's capture, which holds the collapsed
  slots too; dropping them needs a count across invocations (`StreamOutAfterReplay`). Stream
  output from a vertex shader alone is written the same way from the vertex stage's capture, which
  the pass-through draws. Hull and domain shaders
  are not offered on WebGL 2 (feature level 10_0 does not have them).

Alpha-antialiased lines (4.12) take the same first step: the vertex stage runs as compute (WebGPU) or is
captured (WebGL 2), then a draw expands each line into a rectangle around it, carrying each fragment's
distances to the line's axis and ends to the pixel program's `LINE_COVERAGE` variant
(`AntialiasedLinesAsQuads`). Vertex and instance numbers reach the vertex stage's compute form as they
do a draw (4.12): the stage helper passes the API draw's bases in the helper constants.

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
and format support tables (direct3ddxgi/format-support-for-direct3d-*-hardware.md, saved in
`downloads/msdocs/` in scratch) on 2026-11-17. Every format ability those tables require is served
natively or by an exact route of 4.12, except the two shortfalls below: atomics on storage textures and
typed storage stores of the formats WebGPU stores into only with `texture-formats-tier1` (through a
buffer of texels), 16-bit normalised and rendered signed normalised formats, stencil loads, copies within
typeless groups, read backs of every format (depth and stencil by a draw), `D32_FLOAT_S8X24_UINT`
targets (a condition on WebGPU, below), mipmap generation of every
format the tables require it on (including the 32-bit float formats where the adapter cannot filter
them), the packed-pair formats `R8G8_B8G8_UNORM` and `G8R8_G8B8_UNORM` (decoded), and `NV12`, `YUY2` and
`420_OPAQUE` built from ordinary textures, and shared resources (the tables' Shared Resource row): a
resource opened on another device of the process is a copy on that device, copied through a read back
at the API's synchronisation points (`SharedThroughHost`). Shader model 5's early depth and stencil tests, required at
11_0, are served by `EarlyTestsInShader` and `EarlyTestsPerPrimitive`. The shortfalls against the
tables, which the owner is told: multisample counts per format (4.12); and the video abilities of the
video formats (Video Decoder Support, Video Processor Input and Output), since the Direct3D 11 front
offers no video device (4.12). On WebGL 2 a multisampled copy keeps only the resolved value
(`MultisampleCopyResolved`, a listed loss, 4.12).

What each level needs of a back end's numbers, from Microsoft's table and the SDK headers
(`d3d10.h`, `d3d10_1.h`, `d3d11.h` of SDK 10.0.26100, read on the development desktop), and how each is
met:
- Every level from 10_0: `D32_FLOAT_S8X24_UINT` as a depth-stencil target, a condition on WebGPU
  (depth32float-stencil8; core on WebGL 2); clip and cull distances, dual-source blending, depth
  clamping and `SV_PrimitiveID` in pixel shaders natively or by the routes of 4.12
  (`ClipDistanceInShader`, `CullDistanceAsGeometry`, `BlendInShader`, `DepthClampInShader`,
  `PrimitiveIdAsGeometry`).
- 11_0: 16384 2D and cube textures, 2048 3D extent and array layers: met only by the adapter's limits,
  so they are conditions. 32 input slots and 32 elements: by `VertexPulledInShader` past the limits. 14
  constant buffers of 65536 bytes per stage: by `ConstantsAsStorage` past the limit. 32 registers from
  the vertex to the pixel stage: by packing, past that refused (`VaryingsBeyondLimit`). BC1 to BC7: by
  `BlockDecoded` where not native. 128 textures per stage: refused past the limit
  (`BindingsBeyondLimit`). 8 targets: WebGPU's minimum. Every atomic operation on storage views of
  `R32_UINT` and `R32_SINT`, and exchange on `R32_FLOAT`: by `AtomicsThroughBuffer`. Mipmap
  generation on `R32G32B32A32_FLOAT`, `R32G32_FLOAT` and `R32_FLOAT`: natively with
  `float32-filterable` (91.2% of adapters per the research), otherwise by the default's in-program
  filtering (4.3). `[earlydepthstencil]`: 4.12. Typed storage stores on `R10G10B10A2_*`,
  `R11G11B10_FLOAT`, `R16G16_*`, `R8G8_*`, `R16_*`, `R8_*` (and `A8_UNORM`, stored as `rgba8unorm`):
  natively with `texture-formats-tier1` (93.1%), otherwise `StorageFormatThroughBuffer`.
- 10_1: 8192 2D and cube, 2048 3D, 512 layers: conditions. 32 input slots and elements, 32 registers:
  as 11_0. BC1 to BC5.
- 10_0: 8192 2D and cube, 2048 3D, 512 layers: conditions. 16 input slots and elements, 16 registers:
  GLES 3.0's own minimum (16 attributes, 15 varying vectors plus position). 14 constant buffers of 65536
  bytes: by `ConstantsAsTexture` past WebGL 2's blocks or block size. BC1 to BC5: by `BlockDecoded` where
  the extensions are missing. 8 targets: a condition. Requirements every level shares, and how each is
  met: the 16-bit normalised formats sampled, rendered and blended, and the 8-bit and 16-bit signed
  normalised ones rendered and blended, natively or by `NormAsInteger`, with their mipmaps generated by
  the default's decoding program where they are stored as integers (4.3); stencil loads natively or by
  `StencilCopiedByDraw`; a blend and write mask per target natively or by `TargetsSplit`; the sample
  mask natively, by `SampleMaskInShader`, or with multisampling reported absent; rendering of the float
  formats, native on WebGPU and a condition on WebGL 2 (EXT_color_buffer_float, 99.94%); blending of
  the 32-bit float formats (required from 10_0) natively with `float32-blendable` (WebGPU) or
  EXT_float_blend (WebGL 2), both present in this Mac's Chrome and of unchecked share, otherwise in the
  program from a copy of the target (`BlendInShader`, exact within Direct3D's blending tolerance, G2
  and G3 compare); read backs of every format natively or from the host copy (`HostCopyKept`).

Direct3D 11:
- **WebGPU: feature level 11_0, when the adapter has 16384-texel 2D textures, 2048 3D extent and 2048
  array layers, and depth32float-stencil8** (this Mac's Chrome has; WebGPU's defaults do not). An adapter
  without depth32float-stencil8 reports 9_3 whatever its sizes (9_3 has no `D32_FLOAT_S8X24_UINT`
  target). An adapter short of the sizes reports
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
  filtering of `R32_FLOAT` and depth (`FilterInShader`), depth bias on lines, and constant depth bias
  into `D24_UNORM_S8_UINT` where the adapter stores it as float (`DepthBiasInShader`, 4.11), a
  depth buffer larger than the targets (`AttachmentsFitted`),
  sampler bias (`LodBiasInShader`), input slots and constant buffers past the limits
  (`VertexPulledInShader`, `ConstantsAsStorage`), BC formats without the adapter feature
  (`BlockDecoded`),
  wireframe (`FillModeConverted`), formats WebGPU lacks (`FormatConverted`, `VertexFormatConverted`),
  instance step rates, fat targets, layered drawing, exact occlusion counts (4.12), typed storage
  stores without `texture-formats-tier1` (`StorageFormatThroughBuffer`), `SV_VertexID` and
  `SV_InstanceID` with non-zero bases (`DrawBaseInHelpers`, `DrawBaseFromArguments`), alpha-antialiased
  lines (`AntialiasedLinesAsQuads`). Multisampling: 4 samples where WebGPU allows, reported per format
  (4.12). Typed UAV loads of more formats follow `FormatCaps` storage read.
- **WebGL 2: feature level 10_0**, when the device has 8 color targets, 8192-texel 2D and cube
  textures, 2048 3D extent and 512 array layers, and EXT_color_buffer_float (GLES 3.0 guarantees only
  2048, 2048, 256 and 256, and no float targets;
  this Mac's Chrome gives 16384, 16384, 2048 and 2048); otherwise 9_3 (4096 2D and cube, 256 3D, 4
  targets), otherwise 9_1. BC1 to BC5, 16 input slots and 14 constant buffers as above. `resinfo` and
  `sampleinfo` (shader model 4.0) by `TextureInfoInHelpers` (4.12). Compute is optional at 10_x and reported absent
  (`ComputeShaders_Plus_RawAndStructuredBuffers_Via_Shader_4_x` FALSE). Copies between multisampled
  textures: `MultisampleCopyResolved` (a listed loss, 4.12). Multisampling is optional for
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
- **WebGPU: feature level 11_0** under the same conditions as Direct3D 11 (sizes and
  depth32float-stencil8; otherwise
  `D3D12CreateDevice` fails as on hardware below 11_0), resource binding tier 1 (`RESOURCE_TABLES` absent;
  11_1 is not reported: Direct3D 12's table asks logic operations and target-independent
  rasterisation of it), shader model 6.0 (6.6 needs tier 3), wave operations where `SUBGROUPS`.
  Absent: ray tracing, mesh shaders, variable rate shading, sampler feedback, 64-bit integers, tiled
  resources, depth bounds, rasterizer ordered views, conservative rasterisation. Multisampling as for
  Direct3D 11. When a browser offers resource tables, the front reports tier 3.
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
- **Windows library on WebGL 2.** The brief's second desktop back end: the same library built a second
  time with `B` = the WebGL 2 back end, which draws through the `glow` crate (MIT OR Apache-2.0 OR Zlib,
  read in its `Cargo.toml`) on a native OpenGL ES 3.0 context from ANGLE, the layer Chrome itself
  implements WebGL 2 with on Windows (BSD-3-Clause, its `LICENSE` read in the ANGLE repository;
  `libEGL.dll` and `libGLESv2.dll` are test tools on the desktop, never shipped, set up through the
  graphics-rig loop at G3). The context is made through EGL with the `khronos-egl` crate (MIT or
  Apache-2.0, read in its repository). The back end's code is the same as in the browser: glow's
  calls are the same on both, the back end follows WebGL 2's own rules (the buffer type rule of 4.6,
  the stencil rule of 4.11) on a context that would not enforce them, and it reads ANGLE's `GL_`
  extension names for the WebGL ones it would read in Chrome (`GL_EXT_clip_control` for
  `EXT_clip_control`, and so on, one table in the back end). It reports the same `Caps` the browser
  would on that GPU, so the Direct3D 11 front reports 10_0 as it does in Chrome. This is how G3 and G4
  run their Direct3D 11 tests on WebGL 2 against real Direct3D on the same GPU; what WebGL adds over
  GLES 3.0 (its validation) is then covered by the same back end's tests in Chrome (G3).
- **Container modules.** The same crate built for `wasm32-unknown-unknown` once per back end:
  `directx-webgpu.wasm` (DXGI, Direct3D 10, 11 and 12 on WebGPU) and `directx-webgl.wasm` (DXGI,
  Direct3D 10 and 11 on WebGL 2; Direct3D 12 is not offered there). The host loads only the one for the
  API the browser offers, when a program first calls `CreateDXGIFactory*`, `D3D10CreateDevice*`,
  `D3D11CreateDevice*` or `D3D12CreateDevice`. The shaders crate, with its C++, is a module of its own
  (`wasm32-wasip1`), loaded at the first shader; how bytes cross between it, the Direct3D module and
  guest memory, each in its own linear memory, is settled with the second batch's modules (15.2,
  G9b's first step). OpenGL, Vulkan and Direct3D 9 will be modules of their
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
  tracker (4.7, with the fitting of attachments of different sizes), the occlusion counter (4.12; it
  takes each draw's counting program from its caller) and the combination tracker (4.4). `wcontainers-gpu-test`: the recording back end, the seeded random
  call-sequence generator, and the first tests of the shared back-end suite. Pass bar: every rule in
  section 4 is checked by the recording back end and has a test that breaks it; every fast-path default
  is shown to be built from the base by a recorded stream; the occlusion counter gives exact counts on
  the recording back end for scripted draws (with counting programs written in WGSL); the combination
  tracker picks the tuples 4.4 says on scripted sequences; the pass tracker fits a larger depth buffer
  and colour targets of different sizes as 4.7 says (temporaries pooled, the common area copied in and
  out only for `Load` and `Store`, scissors cut), checked on the recording back end; the pass tracker
  splits a pass at a storage write followed by another use of the resource (`UsageScopeSplit`, with a
  running query carried across) and the recording back end rejects a pass that breaks the usage-scope
  rule of `PassDesc`; 100,000 random sequences (seeded, count
  recorded) record and replay identically. Needs: nothing.
- **G2, the WebGPU back end.** `wcontainers-gpu-webgpu` on `wgpu`, with `bcdec_rs` for `BlockDecoded`.
  Pass bar: the shared back-end suite passes natively on the Mac and the RTX 3080 and in Chrome;
  pipelines, bind groups and renamed buffers are cached as section 4 says, with counters proving it on a
  scripted scene; layouts derived as 4.5 says, with a test binding a depth view and an `r32float` view
  with point samplers (unfilterable layouts) and, where `float32-filterable` is present, with linear
  ones; BC textures sampled natively and decoded (the back end is made from the `wgpu` device it drives,
  so a test makes one without `texture-compression-bc`), both read back as the blocks written; copies
  between formats of one typeless group exact, including `r32float` into `depth32float`; the formats of
  `texture-formats-tier1` declared only with it, tested on a device made without it; `generate_mips`
  of `r32float` on a device made without `float32-filterable` and of an `rgba16uint` texture holding
  `rgba16unorm` values, both against real Direct3D's GenerateMips within the stated tolerance;
  `FRACTIONAL_VIEWPORT` confirmed by a draw with a viewport at a half-pixel offset against the
  reference (if Chrome rounds, the bit is dropped and the status file says so); a copy within one buffer
  (`CopyWithinBuffer`); `resolve_queries` by `resolveQuerySet` at unaligned destination offsets; a
  multisampled `Rgba8Unorm` texture made with `reinterpret` resolved in `Rgba8UnormSrgb` against real
  Direct3D's `ResolveSubresource` with `R8G8B8A8_UNORM_SRGB`; a buffer copied into a texture at a row
  pitch that is not a multiple of 256 (`RowsRepacked`) and into `depth32float`; the
  24-bit depth probe of 4.11, with what it finds on the Mac and the RTX 3080 recorded, and a constant
  bias on `Depth24PlusStencil8` matching real Direct3D (natively or through `DepthBiasInShader`); a pass
  fitted by `AttachmentsFitted` with multisampled attachments; uniform bindings with dynamic offsets
  and frequently written uniform buffers in the uniform ring (4.5), with a counter showing no bind group
  made per draw when constants change every draw; one program bound with a point sampler and then a
  linear one keeps one pipeline; a vertex and a fragment helper block of one draw read their own values;
  naga validation with WebGPU's core capabilities (6.3); `prepare` measured removing first-draw pauses. Needs: G1's
  suite (it may start on G0's crate and write the suite's first tests with G1). Test programs are WGSL.
- **G3, the WebGL 2 back end.** `wcontainers-gpu-webgl` on `glow`. Pass bar: the same suite passes in
  Chrome's WebGL 2, with the absent list of 4.11 declared and every slow path counted; cube, array and
  3D textures made with the GL target of `own_view`; BC and `RGB9E5` textures read back as written;
  stencil sampled with and without WEBGL_stencil_texturing (the back end can be made with an extension
  switched off, for tests), with equal answers; copies between formats of one typeless group exact;
  `generate_mips` exact against G2 for a format `generateMipmap` refuses (`r32float` without
  OES_texture_float_linear, and an integer twin of a 16-bit normalised format); a fractional viewport
  through `ViewportInShader` against G2's native one on single-sampled targets; a buffer used as indices
  and as vertices, written by the host and by a copy and by transform feedback, drawn both ways
  (`IndexBufferTwin`, `IndexTwinReadBack`); 8-bit indices; `capture_vertices` with and without
  rasterising; `timestamps_disjoint`; the 24-bit depth probe; `vertex_index` and `instance_index`
  including the bases under `BaseVertexRebind` (the two hidden uniforms of 4.12), in indexed and
  non-indexed instanced draws; a `Discard` write of part of a buffer followed by `NoOverwrite` writes
  above it; depth and stencil read back by `DepthReadByDraw` (with and without
  WEBGL_stencil_texturing); a copy between two multisampled textures (`MultisampleCopyResolved`) whose
  resolve equals the source's; both window-origin routes of 6.5 (with and without EXT_clip_control:
  a culled one-sided triangle, a `gl_FragCoord` gradient, a mip chain and a read back, each equal to
  G2's); a fractional `depth_bias` and `ConstantAlpha` natively; the same suite also passes natively
  on the desktop through ANGLE (section 10), which G4 draws through; per-draw cost measured against
  G2 on the same scene. Needs: G1's suite; G6's `naga_vs_base_vertex` change to naga's GLSL writer (until
  it lands, base-vertex draws whose program reads `vertex_index` are a known failure). Test programs are
  WGSL, written to GLSL ES by naga.
- **G4, the Direct3D 11 front.** `wcontainers-direct3d-11`, and `wcontainers-directx` with the Windows
  library and forwarders of section 10. Pass bar: Wine's Direct3D 11 tests give real Direct3D's verdicts
  on the RTX 3080 except a written gap list naming the absent feature behind each gap; three real
  programs (one per engine family, chosen at G4 from those on the desktop) draw frames matching real
  Direct3D within the stated tolerance; the building-ahead hit counts of 4.4 and the module size of
  section 10 reported; append buffers, `CopyStructureCount`, overlapping queries, per-target blends on
  WebGL 2 without OES_draw_buffers_indexed, texture atomics, `[earlydepthstencil]` with storage writes
  (both with depth writes off and on), `SetResourceMinLOD` with a fractional clamp (sampling, loads,
  gathers and a clamp past the last level), `Map(WRITE)` on a staging texture after a copy into it, a
  depth buffer larger than its render target, `resinfo` on WebGL 2, a rasterised stream output, the
  timestamp disjoint query, `SV_VertexID` and `SV_InstanceID` with non-zero start and base locations in
  direct and indirect draws, a compute shader storing to `R11G11B10_FLOAT` and `R16G16_UNORM` typed views
  on a device made without `texture-formats-tier1`, `NV12` and `YUY2` textures written, sampled through
  their views and mapped, `R8G8_B8G8_UNORM` sampled, a copy between multisampled textures, and
  alpha-antialiased lines (within the specification's tolerance), a pixel shader's storage view read by
  the next draw as a shader resource on the same targets (`UsageScopeSplit`), a vertex shader using a
  base vertex with a pixel shader using a sampler's `MipLODBias` (both helper blocks), deferred lighting
  sampling an `R32_FLOAT` depth and a colour texture through one linear sampler (`FilterInShader` on the
  depth only), constant buffers updated per draw by `Map(WRITE_DISCARD)` and by `UpdateSubresource`
  (no bind group made per draw and no pass split, counted by the back end's test counters), an event
  query polled to completion, and a shader refused at creation with `E_INVALIDARG` each have a test
  against real Direct3D. The WebGL 2 cases run through the Windows library built on WebGL 2 (section
  10).
  Needs: G1, G2 (to draw), G3 (for the WebGL 2 cases), G6's `convert` (6.4) for real shaders. Before G6 lands it
  works on the recording back end and G2 with WGSL programs and hand-written reflections.
- **G5, the Direct3D 12 front and the shared infrastructure.**
  `wcontainers-directx-graphics-infrastructure` (adapters, outputs, swap chains, presenting, frame
  pacing) and `wcontainers-direct3d-12`. Pass bar: Wine's (vkd3d's) Direct3D 12 tests as G4's bar; a real
  Direct3D 12 program draws matching frames; `CheckFeatureSupport` answers match section 9; work on a
  copy queue and a compute queue that wait on each other's fences and on CPU signals replays in the
  order of 5.2, tested with fences signalled late from the CPU; `ResolveQueryData` of binary occlusion,
  counted occlusion and timestamps into a default-heap buffer, copied to a readback heap and used by
  `SetPredication`, matches real Direct3D, with the predicated draws made indirect on WebGPU; placed
  buffers overlapping in one heap inherit each other's bytes (written by a copy, a storage view and the
  CPU through an upload heap), and identically described textures at one offset inherit, both against
  real Direct3D, while resources without overlaps take no `AliasCopied`; `ExecuteIndirect` draws whose
  vertex shader reads `SV_VertexID` with a base vertex. Needs: G1,
  G2, G6's `convert` for DXIL (as G4, it starts with WGSL programs). G4 uses the infrastructure crate, so
  G5 builds it first and G4 starts with a stub of its two calls (create a swap chain, present) until it
  lands.
- **G6, shaders and the cache.** `wcontainers-shaders`: dxbc-spirv and dxil-spirv built in (natively and
  for WebAssembly, imports listed), the DXBC lowering into naga (measured first against the SPIR-V
  route, 6.3), naga's SPIR-V reader fixes for DXIL, `convert` with every `VariantKey` field and flag of
  6.4 (including the counting variant, texel buffer routes, storage packing, atomics through a buffer,
  decoded textures, targets written in the program, the sample mask, early tests in the program, the
  minimum level-of-detail clamp, the viewport adjustment, texture level and sample counts from the
  helpers, the draw's bases from the helpers or the argument buffer, storage formats through a buffer
  with compare-exchange for narrow texels, line coverage, clip distances and depth clamping in the
  program, and the primitive index from the stage helper), the `gl_SampleID` fix and the
  `naga_vs_base_vertex` uniform in naga's GLSL writer (6.3, 4.12; G3 needs the second early, so it comes
  first), integer stage values marked flat, `side_effects` and `discards` and the
  sampled pairs in the reflection, the counter operations of 5.1,
  the binding rule and reflection of 6.4, the built-in compute
  programs fronts use (texture clears of 5.1), the stage helper of section 7, and the cache of section 8.
  Pass bar: all 191 corpus shaders and a corpus of real game shaders (DXBC and DXIL) convert or fail with
  a named reason, and every converted shader compiles in Chrome for each back end that can run it; each
  variant field has a test showing the rewritten program's result on G2 or G3 equals the unrewritten
  program's with the native feature (or a reference picture); geometry, tessellation and stream output
  tests match real Direct3D on the RTX 3080; the cache file round-trips, merges by union and survives a
  cut last entry; converting under each back end's keys gives the same bytes on both (section 8). Needs: G0 for conversion and variants; G2 (and G3) to run their tests and the stage
  helper's.

Second batch, after these: the call channel and guest stand-ins, OpenGL through Mesa on our interface,
Vulkan 1.1 through Mesa's runtime, sound and input; section 15 plans them, with milestones G1d and G7 to
G13.

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
- The share of WebGPU adapters with depth32float-stencil8 (a condition of 10_0 and above, section 9)
  and of WebGL 2 browsers with WEBGL_blend_func_extended (the survey's page for it is missing); G2 and
  G3 record what their machines report.
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
- Whether Chrome's WebGPU keeps fractional viewports to sub-pixel precision (expected; G2 checks).
- How each browser and GPU stores `depth24plus` (the probe of 4.11; G2 and G3 record it), and how often
  games draw with a constant depth bias into 24-bit depth.
- What `AttachmentsFitted`, `IndexBufferTwin`, `QueryResolveWait` and `PredicationIndirect` cost, and
  how often real programs reach them.
- How often real programs read `SV_VertexID` or `SV_InstanceID` with non-zero bases, and what
  `DrawBaseInHelpers` costs per draw where the bases change every draw (G4).
- What `StorageFormatThroughBuffer`, `AliasCopied` and `AntialiasedLinesAsQuads` cost, and how often real
  programs reach them; how close `MultisampleCopyResolved` comes to real Direct3D in pictures (G3, G4,
  G5).
- How close the mipmap default's in-program filtering comes to the RTX 3080's GenerateMips (G2), and
  what `EarlyTestsInShader`, `EarlyTestsPerPrimitive`, `MinLodClamp` and `StagingRefreshed` cost, and how
  often real programs reach them (G4).
- How often `UsageScopeSplit` splits passes in real programs and what it costs (G4, G5); what the
  creation checks of 4.8 cost per shader (G4).
- Whether every real game's constant buffers fit `maxDynamicUniformBuffersPerPipelineLayout` (8 by
  default) per pipeline, and what the uniform ring saves per draw against renamed spares (G2, G4).
- Whether EXT_clip_control's upper-left origin keeps facing as clip space orients it, as read in the
  extension (G3 tests both origin routes).
- How far the browser back ends' line rasterisation differs from Direct3D's quadrilateral and aliased
  rules in pixels (G4).

## 14. Where the evidence is

Research: `pyro-parked/wcontainers-scratch/related/` (graphics-design-inputs.json, webgl2-vs-webgpu.json,
vulkan-on-webgpu-inputs.json, games-prior-art.json). Shader experiments: `wcontainers-scratch/graphics/`
(sources of dxbc-spirv, dxil-spirv and Fossilize under `src/`, the corpus and every result file under
`corpus/`, the harness under `experiments/shader-corpus/`, the scripts under `tools/`). Rerun with
`tools/run-corpus.sh` inside the checkout's Nix shell (section 6.3 lists the steps it automates).

## 15. The second batch

Written 2026-11-18 (G1c) from the brief's "Second batch" paragraph, the research named in section 14,
and what the first batch's six tracks found (their status files). It plans; nothing of it is built.
Where the first batch's experience contradicts what sections 1 to 12 assumed, 15.1 says so and the
rest follows the experience. Claims checked at their source in this run are marked "checked"; the
rest come from the research files, which name their sources.

### 15.1 What the first batch changed

- **The call buffer of 4.8 was not built in the first batch.** Section 10 had the Windows shell
  encode every call into the call buffer in-process. The Direct3D 12 library instead sends every COM
  call to the device thread as a boxed closure and waits for the answer, creation included (its
  status, Departures). The Direct3D 11 library has neither a call buffer nor a device thread: its
  COM calls run the front on the calling thread under one re-entrant lock (its status). So the
  channel, the encoder generation, the device thread of Direct3D 11 and the "creation never waits"
  rule are all second-batch work, and the three existing fronts' shells move onto the channel (G7).
- **Consumed arguments are copied once, not read in place** (a narrowing of the brief's "vertex and
  texture data are read in place", 15.5). An argument the API consumes at the call
  (`UpdateSubresource`, `glBufferSubData`, `glTexSubImage*`) may be overwritten by the program as
  soon as the call returns, while the stream crosses once a frame, so the stand-in copies it into the
  stream (as a native driver copies it into staging memory); reading it in place would need a
  crossing per upload. What the API reads later (maps, Direct3D 12 upload heaps, Vulkan host
  memory) is read in place as the brief says. G7 measures the copy's cost.
- **A test process must never draw through the API it stands in for.** With our `d3d12.dll` loaded,
  wgpu's Direct3D 12 path loads ours; the Direct3D 12 library draws through wgpu's Vulkan path, whose
  line rules differ from Direct3D's on the RTX 3080 (a listed loss). The same holds for the new
  fronts: the OpenGL test process may use wgpu's Direct3D 12 or Vulkan path (never its GL path), and
  the Vulkan test process must use wgpu's Direct3D 12 path, since our ICD is the process's Vulkan.
- **The real driver differs from the specification.** The RTX 3080's Direct3D rounds some normalised
  stores 0.51 to 0.55 of a code down, keeps BGRX's X byte, and optimises trilinear filtering, so the
  shaders track holds results to two references (the GPU and WARP), equal to either. The OpenGL and
  Vulkan fronts take the same rule with Mesa's software drivers as the second reference: llvmpipe
  for OpenGL, lavapipe for Vulkan (both in Mesa's tree, checked: `src/gallium/targets` holds
  `libgl-gdi` and `lavapipe`).
- **Fronts cannot know the index format when they prepare** (G1c: `prepare` names none; WebGPU
  prepares a strip for both). OpenGL and Vulkan bind index buffers at the draw too, so nothing
  changes for them.
- **Vertex formats are converted by the back end, not on upload** (WebGPU's shadow streams, 4.12):
  the OpenGL and Vulkan fronts hand every `VertexFormat` as it is.
- **Program-side rewrites are the norm, not the exception.** Most of what WebGPU lacks became helper
  values and variant fields (6.4: 22 helper values, 23 variant fields and 15 variant flags at G1c,
  counted in `reflect.rs`). Mesa's st/mesa does its
  own OpenGL-specific lowering (alpha test, clip planes, two-sided colour, flat shading, point size,
  research `graphics-design-inputs` 2c) when a driver leaves the cap off. Rule: each rewrite happens
  in one place. The OpenGL driver leaves off the caps of OpenGL-only features so st/mesa lowers them,
  and takes the interface's variant fields for what the back end lacks.
- **Direct3D 12's numbering became the program's own** (the shaders track, run 22), not the root
  signature's. Vulkan's (set, binding) numbering is likewise the program's own; that meets the
  helper set (15.4).
- **The display driver can reset under our jobs** (the rig's status): risky jobs run alone and a
  reset quarantines the job. Conformance suites are run in short shards through the queue, never as
  one long job.

### 15.2 OpenGL through Mesa, on our interface

Built on: Mesa's OpenGL state tracker (st/mesa, with its GLSL compiler producing NIR) over a Gallium
driver of ours, `wcontainers-opengl`, which is a front onto `wcontainers-gpu` like the Direct3D ones
(generic over `B`, resolved at compile time). Mesa enters the repository as a git submodule at
`third_party/mesa`, as `dxbc-spirv` and `dxil-spirv` do (6.2), not as copied files: Mesa's tree
holds GPL-2.0 and GPL-1.0-or-later files (the Linux uapi headers in `include/drm-uapi`, the svga
driver's headers) and SGI-B-2.0 ones (`src/glx`), and the project copies no GPL file (its licences
directory lists Apache-2.0, BSL-1.0, GPL-1.0-or-later, GPL-2.0-only, MIT and SGI-B-2.0, research 1
and Mesa's `licenses/`). The submodule points at a fork of ours whose branch is the pinned release
plus the patch set, one commit per change, so this repository holds none of Mesa's files and its
build links none of those (below). New directories in the fork:
`src/gallium/drivers/wcontainers` (the screen's create function in C and its meson file, which links
a Rust static library built by cargo), the Vulkan driver's directory (15.4) likewise, and the
container's frontend (below). Changes to Mesa's own files, because Mesa names its drivers in fixed
lists (checked in Mesa's main branch, 2026-11-18): `meson.options` (a new choice in the fixed
`gallium-drivers` and `vulkan-drivers` lists), the top-level `meson.build` (the
`with_gallium_wcontainers` and Vulkan flags, as `with_gallium_zink` and `with_microsoft_vk` are made
there), `src/gallium/meson.build` (the driver's `subdir` and its `driver_*` dependency),
`src/meson.build` (the Vulkan driver's `subdir`), and the WGL target, whose `wgl.c` branches per
driver in three places (checked in Mesa's main branch, 2026-11-18): `wgl_screen_create_by_name`
picks a screen by `#ifdef GALLIUM_*` blocks, one per driver, so ours gets one (setting a
`use_wcontainers` flag, as `use_zink` is set); `wgl_screen_create`'s `drivers[]` list (the
`GALLIUM_DRIVER` variable's choice first, then `d3d12`, `zink`, `llvmpipe`, `softpipe`) gets ours
after the variable's choice, so the test build draws through it by default and
`GALLIUM_DRIVER=llvmpipe` still picks the software reference; and `wgl_present` gets a branch on
that flag calling `pipe_screen::flush_frontbuffer`, as Zink's does, which our driver serves by
presenting the interface's surface (`wgl_create_framebuffer` returns nothing for drivers other
than d3d12, and stw then keeps its own framebuffer, so it is left as it is). `libgl-gdi`'s meson
file links the driver. The Rust side fills `pipe_screen` and
`pipe_context` itself from bindings that `bindgen` (BSD-3-Clause, to confirm at G9) makes of the
pinned headers. So the driver is Rust, as the project's rules ask, and Mesa's build links it as a
prebuilt library.

What the driver does, by the plan's earlier sections: `create_*_state` makes blocks (4.4), `set_*`
marks dirty, `draw_vbo` names blocks and binding sets, `pipe_context::blit` and clears go through
Gallium's `util_blitter` drawing with our own blocks, queries through the interface's queries and the
occlusion counter, `resource_copy_region` through `copy_texture` (copies within one subresource of
every format, G1c), maps through the interface's write modes (5.3), buffer objects made a whole
number of storage words in size so that one bound whole as a shader storage buffer stays inside it
(4.5). Shaders arrive as NIR and go
through the NIR route (15.3) into the common form, then the shaders crate's `convert` with the
variant key the draw needs. Versions, as 5.3 and 5.4: 4.6 core on WebGPU (64-bit floats by Mesa's
soft-float lowering, `F64Emulated`), 3.1 on WebGL 2 (no multisampled textures there). Caps report
what the back end has; st/mesa then computes the version from them (research 2b, `version.c`).

The frontend, the code that makes contexts and surfaces: on Windows, Mesa's own WGL frontend and its
`libgl-gdi` target (`opengl32.dll`; checked: both are in Mesa's tree, and Mesa's `docs/install.rst`
lists the Microsoft Visual C++ compiler, with Visual Studio 2022 17.9 or later and a Windows SDK of
at least 20348, and Clang 5.0 or later among the compilers known to work; it names no clang-cl),
with a `stw_winsys` of ours whose present calls the
interface's `present` on a surface over the window. In the container: a small frontend of ours on
st/mesa's frontend interface, as Haiku's `hgl` frontend is, making contexts on the interface's
surfaces (4.9). The guest never runs Mesa: it runs the stand-in (15.5), and Mesa runs on the device
thread.

How Mesa is built:
- **For Windows testing**: Meson and Ninja with the Microsoft Visual C++ compiler (the one
  `install.rst` names for Windows; the desktop's Visual Studio version is checked against its 17.9
  at G9's first step), on the desktop under `C:\wc`, from the pinned tree, with `-Dgallium-drivers=wcontainers,llvmpipe` (llvmpipe as the deterministic reference),
  `-Dvulkan-drivers=wcontainers,swrast` (lavapipe likewise), LLVM for llvmpipe and lavapipe only,
  and the Rust static library built natively first. Programs load our `opengl32.dll` from their own
  folder (the rig's existing replacement mechanism); the system's OpenGL is untouched.
- **For WebAssembly**: the same pinned tree cross-built for `wasm32-wasip1` with wasi-sdk, the
  toolchain the shaders crate already uses for its C++ (6.2), as one module per API, each loaded
  when a program first asks for its API (the brief): `opengl.wasm` (st/mesa and our Gallium driver,
  when a program first makes a GL context) and `vulkan.wasm` (Mesa's Vulkan runtime and our Vulkan
  driver, when a program first creates a Vulkan instance), each for both back ends. Mesa's shared
  code (NIR, its compiler passes, `util`) is linked into both, a size cost measured at G9b and G10
  against one module holding both. Off: LLVM, GLX, EGL, GBM, the
  DRI frontend, the disk shader cache (ours is section 8), zstd and expat; glthread off (the stream
  already batches). Not yet known and settled at G9's first step: whether Mesa builds without
  pthreads (a wasi-sdk `-threads` target otherwise), how `setjmp` (which `spirv_to_nir` uses, research
  4b) is served, and the module's size (gex reports about 20 MB before compression for its whole Mesa,
  research 4a; to be measured, and split as 10 says if Chrome's synchronous limit bites).
- **One device across modules.** Section 10 builds each front with its back end inside one
  `wasm32-unknown-unknown` module, where wgpu reaches WebGPU through `wasm-bindgen`. A WASI C module
  cannot hold that back end as it is, and a program using two APIs (Direct3D through Wine beside
  OpenGL, say) would get two back-end instances and two devices, whose objects cannot meet. So the
  back end becomes its own module per page (`gpu-webgpu.wasm`, `gpu-webgl.wasm`), built as now and
  owning the one device on the device thread; every front module (the Direct3D one, and
  `opengl.wasm` and `vulkan.wasm`, built for `wasm32-wasip1`) implements `Backend` by a type whose
  methods call the back-end module's exported functions, one wasm import call per interface call,
  objects named by handles.
  Bytes are the open part: each module has its own linear memory, and the calls that take bytes
  (`write_buffer`, `write_texture`, read backs' destinations, Mesa's upload buffers) and the guest's
  memory, which the brief has read in place, are in memories the back-end module cannot address.
  Three ways: every module imports one shared memory, which needs the modules linked against one
  allocator (the WebAssembly tool conventions' dynamic linking, as Emscripten's side modules are),
  and reads in place; the back-end module imports the front's memory as a second memory
  (multi-memory, which Chrome runs; whether Rust and clang emit accesses to a second memory is not
  known); or the adapter copies each byte argument across, one copy of every byte, the cost the brief's
  "read in place" avoids. The question is not new to the second batch: section 10 already makes the
  shaders crate a module of its own that the Direct3D module hands shader bytes to, and in the
  container the Direct3D module reads guest memory, which lives in another module's memory. So the
  answer below settles it for the first batch's modules as well, and section 10's modules follow it.
  The boundary also changes rule 4 of section 2 (the brief's "one ordinary call per operation"): an
  import call into another module cannot be inlined, so each interface call costs one call across
  modules. Settled at G9b's first step with the questions above: which of the three
  the toolchains allow, whether the Rust driver links into the wasi-sdk module as a static library,
  and the boundary's cost per call and per byte, measured on a Direct3D 11 rig program in Chrome
  against the one-module build (expected small for the first two, not measured). If no way is cheap,
  the Direct3D module keeps its back end inside and a program using two APIs is refused the second
  device by name.

Licences: Mesa's core is MIT, files carry SPDX tags (research 1): what a WebGPU driver build links is
MIT plus BSL-1.0 (`src/c11` threads), BSD-3-Clause (soft-float, used by `F64Emulated`), BSD-2-Clause
(xxhash) and the Khronos headers' Apache-2.0; no GPL or SGI file is built (GLX and the svga driver are
off). G9 lists every linked file's SPDX tag from the build's own dependency list, not from memory.
The GPL and SGI files stay in the submodule's tree, never copied into this repository (above).

**The MIT-headed WebGPU driver for Mesa.** It is `gex_wgpu` in `github.com/MercuryWorkshop/mesa-wink`
(branch `wink`). Checked at the source on 2026-11-18 (and by G1c's independent check, every file of
the directory; the head of `wink` read again at 21:57 UTC that day): its C sources,
`gex_wgpu_screen.c` (2,659,872 bytes at that head, one file) among them, start
`SPDX-License-Identifier: MIT`; its build and test files do not: `meson.build` (which begins
`gex_emdawn_dir = get_option(...)`, no header), the `native/` build scripts, its README, CI lists
and log carry no licence. The repository declares no licence (GitHub's licence field is empty; it
carries Mesa's own `docs/license.rst` and `licenses/`, which say each file's SPDX tag governs, and
these files have none), so those files are of unknown licence, and building the driver needs its
`meson.build`. It is not a GitHub fork. It is still being worked on, daily: the head of `wink`
(5f8a8bc255, 2026-11-18, fence syncs on a remote context) changes `gex_wgpu` itself, as did an
earlier commit on another branch (2026-11-13, `fr/tbo-align`, 4a3419242a, buffer texture ranges
at a 16-byte alignment); the repository was pushed to again later that day. Its
meson file requires Tint and emdawnwebgpu (Dawn's `webgpu.h` for Emscripten), and it also builds
natively against Dawn (`gex_wgpu_native.c`). Its shader compiler (NIR to SPIR-V by Zink's
`nir_to_spirv`, then Tint to WGSL) is `src/wink/compiler`, shared with the same repository's Vulkan
driver, wvk (15.4). Recommendation: do not adopt it. It draws
through `webgpu.h` directly, so it would bypass the interface: no WebGL 2 back end, no shared slow
path counters, capabilities or pass tracker, a second WebGPU device owner beside wgpu, and a second
shader path (NIR to SPIR-V to Tint) beside naga, against rule 3 and the one-device rule. It is one
file of over two and a half megabytes maintained by another project's fork, it was written for
gex (AGPL), so its provenance is the owner's call even with MIT headers, and adopting it would need
its authors to license the build files or a build of ours written apart from them. Its published conformance
figures (research 7, and its README checked: full runs of 2026-09-28 on an Apple M4 with native
Dawn, "pass counts include warnings and flakes") are, for dEQP's GLES 2, 3 and 3.1 and KHR-GLES,
92.9% of the 111,069 cases run of 112,212, and for KHR-GL46, 98.6% of the 20,298 cases run of
31,062 (its own failure column, 295, gives 98.5%; the published share is taken). They are over other
case lists on another GPU and count flakes as passes, so G9 takes them as floors of its own,
stricter measure, not as a comparison (15.8). Until the owner decides, it is treated as gex is:
ideas only.

How it is proven (G9, G9b): from the mustpass of one pinned VK-GL-CTS release (Apache-2.0, testing
only), the GLES lists (dEQP-GLES2, -GLES3, -GLES31 and KHR-GLES, the families gex_wgpu reports;
OpenGL ES contexts come from Mesa's WGL frontend, which offers `WGL_EXT_create_context_es2_profile`,
checked) and the KHR-GL lists for 3.3 and 4.6, run by deqp-runner (MIT) with a fails list per back end, in
shards through the rig's queue on the RTX 3080 natively, and in Chrome; piglit's quick profile
(testing only). Pictures of real programs (bgfx's OpenGL renderer, Godot's Compatibility renderer,
both already among the rig's programs for Direct3D) against the RTX 3080's own OpenGL and llvmpipe,
held to the rig's per-picture allowance, equal to either reference.

### 15.3 One NIR route into the common form

Both Mesa drivers hand shaders over as NIR, so one library, `wcontainers-nir`, turns NIR into the
common form (a naga module) with the reflection of 6.4, for both. Two candidate routes, chosen by
measurement on a corpus at G8, as 6.3 chose the DXBC route:
- NIR to SPIR-V by Zink's `nir_to_spirv` (MIT), then naga's SPIR-V reader, which the shaders track
  has already hardened on dxil-spirv's output (6.3). Expected cheap to build; Zink's SPIR-V shape
  differs from dxil-spirv's, so new reader fixes are likely.
- A direct NIR to naga writer in Rust, modelled on KosmicKrisp's `nir_to_msl` (about 2,600 lines,
  research Q2a). More work; it keeps NIR's structure and needs no SPIR-V round trip.
The corpus: piglit's and dEQP's GLSL, the GLSL of real programs (Godot's Compatibility renderer,
bgfx's shaders, Dolphin's), and SPIR-V from Vulkan programs (Godot Forward+, Sascha Willems' samples).
A converted shader counts only if it computes what the NIR does. Each corpus shader is run on
seeded inputs (its uniforms, vertex attributes and textures filled from the seed; the seed and the
number of input sets recorded, a knob) twice: by Mesa's own software driver from the same NIR
(llvmpipe for GLSL, lavapipe for SPIR-V, built from the pinned tree on the laptop), the reference;
and by the route's common form, run by the recording back end's interpreter (`wcontainers-gpu-test`, section 11), or, for a
shader past the interpreter's stated limits, by the WebGPU back end on the RTX 3080 through the
rig's queue. Outputs are compared exactly for integers and within the format's rounding for
floats; a shader whose outputs differ counts as failed on that route, listed by name.
Pass bar of the choice: every corpus shader converts and matches the reference, or fails with a
named reason, on both routes; the route with more matching shaders is chosen, and on equal counts
the one of less time per shader (both measured on the laptop and recorded with its load, with each
route's count and time).

The cache (section 8) keys these by the NIR's serialised bytes (Mesa's `nir_serialize`) plus the
converter's version, so OpenGL and Vulkan programs keep their combinations across runs as Direct3D's
do.

### 15.4 Vulkan 1.1, a new driver on Mesa's Vulkan runtime

Built on: Mesa's common Vulkan runtime (`src/vulkan/runtime`, checked: it holds `vk_cmd_enqueue.c`,
`vk_graphics_state.c`, `vk_meta*.c` and the pipeline and shader code the research names), with a
driver of ours, `wcontainers-vulkan`, onto `wcontainers-gpu`. As research 1 recommends: the driver
implements dynamic rendering only and gets render passes from the runtime, supplies
`vk_device_shader_ops` (its `compile` is the NIR route of 15.3 plus `convert`) and gets pipelines,
shader objects, pipeline libraries and the pipeline cache from common code, records command buffers
with `vk_cmd_enqueue` (as lavapipe does) and replays them onto the interface at submit, where the pass
tracker (4.7) opens, splits and reopens passes with load operations as KosmicKrisp does at barriers.
`vk_meta` gives blits, resolves, clears and copies made of draws and dispatches through the driver's
own pipelines. Window surfaces are a small WSI of ours over the interface's surfaces (4.9), not Mesa's
`wsi_common`, which presents through DRM or CPU images. The Rust and C split is that of 15.2.

Prior art: wvk, the Vulkan driver in the same `mesa-wink` repository as `gex_wgpu` (checked at the
source on 2026-11-18, and by G1c's independent check, every file: `src/wink/vulkan`, whose C sources
and meson files start `SPDX-License-Identifier: MIT` while its README, CI lists, test scripts and
test shaders carry no licence, in the same repository without a licence of its own; directory last
changed 2026-10-28). It is Mesa's
Vulkan runtime over `webgpu.h` (natively an ICD over Dawn, in a browser over emdawnwebgpu) and its
design is close to this one: commands recorded into `vk_cmd_queue` and replayed at submit,
pipelines made at the draw, shaders compiled for some of the state they are drawn with, the
driver's own bindings at the top of group 3. It differs where `gex_wgpu` does (15.2): it draws
through `webgpu.h` beside wgpu, and its shaders go NIR to SPIR-V to Tint, not into our common form,
so it bypasses the interface and serves no WebGL 2 fallback, counters or cache of ours. By default
it reports Vulkan 1.3 without `bufferDeviceAddress`, so not a conformant 1.3, and 1.2 only when
`WINK_VK_API_VERSION=1.2` is set (checked: `wvk_private.h`'s `WVK_API_VERSION` is 1.3, and
`api_version()` in `wvk_physical_device.c` returns it unless that variable says 1.2; its README's
gaps list agrees, "Vulkan 1.3 is reported, but for `bufferDeviceAddress`", while the README's first
line still says 1.2); it exposes `VK_KHR_portability_subset` unless `WINK_VK_NO_PORTABILITY` is set,
reports `timelineSemaphore`, serves through emulations most of the optional features listed
below, and gives its full dEQP-VK run of 2026-09-28 on an Apple M4: 2,443,884 cases, 313,069 run,
98.5% of those passed. The brief asks for a new driver, so wvk is ideas only, as `gex_wgpu` is,
unless the owner decides otherwise (15.9); its figure is the floor of G10's measure (15.8), with
the same caveat as `gex_wgpu`'s: other GPU, other version reported.

Back ends: only those that compute (`Features::COMPUTE`, storage in every stage the stage helper
needs). On WebGL 2 the ICD enumerates no physical device, which is how Vulkan says "no Vulkan here";
programs fall back to their OpenGL path. So in the browser it is WebGPU only, as the brief says.

What is reported (5.5 points here), by research Q1 and Q3:
- Version 1.1. 1.2 needs timeline semaphores, `hostQueryReset`, imageless framebuffers and
  `uniformBufferStandardLayout` among others (research Q1a), emulable on the CPU; 1.3 needs buffer
  device addresses, which WGSL lacks, and `maxBufferSize` of 2^30 against WebGPU's 256 MiB default.
  wvk, on the same runtime over WebGPU, reports 1.3 by default without buffer device addresses (not
  a conformant 1.3) and 1.2 under `WINK_VK_API_VERSION=1.2`, with timeline semaphores, so the step
  to 1.2 is expected to be small; it stays the owner's decision (15.9), not planned here.
- Host-visible coherent memory, which 1.1 requires: guest memory written at submit, ranges the GPU
  wrote read back before a fence reports done (5.5), counted.
- Subgroups, which are properties, not features (1.1 requires the basic operations in the compute
  stage, checked in the specification's limits chapter): `subgroupSize` 1, every invocation a
  subgroup of its own, in every stage, with the operations exact at that size (basic, vote,
  arithmetic, ballot, shuffle, shuffle relative, clustered) and no quad operations; the driver
  lowers them in NIR to their one-invocation meaning before the common form, so no back end is
  asked for anything. wvk reports the same (checked: `wvk_physical_device.c`, `subgroupSize = 1`).
  Larger subgroups through `Features::SUBGROUPS` are a later step: WebGPU's subgroup size varies
  between the adapter's minimum and maximum, and 1.1 reports one size.
- Features. Vulkan 1.1 has 67: the 55 of `VkPhysicalDeviceFeatures` and the 12 of 1.1's six
  feature structures (`VkPhysicalDevice16BitStorageFeatures`, `...MultiviewFeatures`,
  `...VariablePointersFeatures`, `...ProtectedMemoryFeatures`,
  `...SamplerYcbcrConversionFeatures`, `...ShaderDrawParametersFeatures`), counted in `vk.xml`
  (checked 2026-11-18). Each is named once in the five lists below: 41 present (three of them only
  where the adapter has what they need) and 26 absent. Nothing else decides a feature; G10 checks
  the driver's answer against these lists.

Present natively (11):
- `fullDrawIndexUint32`: WebGPU splits only strips at the restart value; a list takes every 32-bit
  value as an index (checked: the WebGPU specification's primitive assembly, 23.2.3), and a strip
  drawn with restart off is `RestartRewrite` (5.7).
- `imageCubeArray`, `independentBlend`, `sampleRateShading` and `depthBiasClamp`: WebGPU's core
  (`DEPTH_BIAS_CLAMP`).
- `samplerAnisotropy`: the sampler's anisotropy, up to 16. WebGPU takes it only with every filter
  linear, so a sampler with anisotropy and a nearest filter is made with anisotropy 1; Vulkan leaves
  the scheme to the implementation, which "should consider" the filters (checked: the
  specification's texel anisotropic filtering).
- `fragmentStoresAndAtomics`: `FRAGMENT_STORAGE_WRITE`; atomics on a storage texture whose format
  has none, `AtomicsThroughBuffer`.
- `multiDrawIndirect`: one interface indirect draw per draw of the call, as OpenGL 4.6's (5.4); the
  draw count is a host value, so nothing is read back, and each draw costs what a separate indirect
  draw costs.
- `inheritedQueries`: secondary command buffers replay inside the primary's pass (5.5), so a query
  the primary began counts their draws.
- `textureCompressionETC2` and `textureCompressionASTC_LDR`: where the adapter has WebGPU's feature
  for each; absent otherwise.

Present natively where the adapter has WebGPU's feature, otherwise by the route the Direct3D fronts
take on the same back end, counted (6):
- `dualSrcBlend` (`BlendInShader`), `shaderClipDistance` (`ClipDistanceInShader`), `depthClamp`
  (`DepthClampInShader`).
- `textureCompressionBC`: `texture-compression-bc`, otherwise held decoded (`BlockDecoded`, 5.7).
  Vulkan requires one compression family; with BC always present, no adapter loses Vulkan for want
  of one.
- `drawIndirectFirstInstance`: `INDIRECT_FIRST_INSTANCE` (99.97% of adapters), otherwise the
  arguments read back and drawn directly (`IndirectReadBack`), as Direct3D's.
- `shaderStorageImageExtendedFormats`: `texture-formats-tier1`, otherwise
  `StorageFormatThroughBuffer`.

Present through program rewrites the shaders crate already makes for Direct3D, made on the NIR
route's output (15.3) and, as for Direct3D, not slow paths (4):
- `shaderStorageImageReadWithoutFormat` and `shaderStorageImageWriteWithoutFormat`: the variant
  named by the bound view's format (4.5), as Direct3D's typed storage views; read-write formats
  WebGPU lacks through `StorageFormatThroughBuffer`.
- `shaderImageGatherExtended`: a gather's offset given at run time moves the coordinate by the
  offset in texels of level 0, which picks the same four texels (the shaders crate's route for
  Direct3D's `gather4_po`); `textureGatherOffsets` is four gathers, each giving the one texel the
  specification takes from it.
- `shaderSampledImageArrayDynamicIndexing`: each element of an array its own binding, an element
  picked at run time by a function switching on the index (the shaders crate's route for Direct3D
  12's texture arrays, `dxil/arrays.rs`); G8 widens it from textures to arrays of samplers and of
  combined image samplers. Arrays of any kind indexed by constants are an element's own binding on
  every route.

Present through counted slow paths, by the routes the Direct3D and OpenGL fronts take on the same
back end (20). wvk serves most of these on WebGPU too, some only where the adapter has a feature: its
`occlusionQueryPrecise` only with `float32-blendable` or native Vulkan, its `shaderCullDistance`
only with `clip-distances` (checked in `wvk_physical_device.c`).
- `robustBufferAccess` (required). Vulkan bounds every access through a descriptor by the
  descriptor's range and every vertex fetch by the bound vertex buffer range: a read past it gives
  a value from the memory bound to the buffer, zero, or (0,0,0,x) with x 0, 1 or the largest value;
  a write past it is dropped or lands within that memory; an atomic past it acts as such a write
  and returns an undefined value; nothing else (checked: the specification's robust buffer access,
  `shaders.adoc`). WebGPU gives more: an out-of-range load or store in a shader may partly discard
  the draw or dispatch, and a load may give (0,0,0,X) with any X (checked: the WebGPU
  specification, "Out-of-bounds access in shaders"); WGSL makes such an access a dynamic error
  whose outcomes include a trap that ends the invocation with its outputs zero (checked: WGSL,
  "Out-of-Bounds Access"); and the index resolution may cancel a whole draw any of whose vertex
  fetches is out of range (checked: the WebGPU specification's vertex processing, "Optionally
  (implementation-defined), empty vertexIndexList and return, cancelling the draw call"). wgpu's
  naga and Chrome's Tint are expected to bound such accesses within the binding (not checked here),
  but neither specification requires it, so the driver relies on neither: on a device made with the feature enabled it hands the
  interface no access out of range, and the interface's back ends never meet one (the interface
  allows them, with WebGPU's outcomes: `BindingResource`'s and `DrawRange`'s docs, 4.12).
  Shader access, in every program of a device made with the feature, in three NIR steps (each
  pass MIT, checked at its source on Mesa's main branch, `src/compiler/nir/`). First, buffer
  access is lowered to a binding index and a byte offset (`nir_lower_explicit_io` with
  `nir_address_format_32bit_index_offset`). Second, it is split into 32-bit scalar accesses
  (`nir_lower_io_to_scalar` over uniform and storage buffers, which takes `load_ubo`, `load_ssbo`
  and `store_ssbo`; `nir_lower_mem_access_bit_sizes` for the 64-bit ones `shaderInt64` brings;
  16-bit buffer access is absent, below), as Vulkan allows: "Any non-atomic access ... wider than
  32-bits may be treated as multiple 32-bit accesses that are separately bounds checked" (checked:
  `shaders.adoc`). Third, `nir_lower_robust_access` runs, its filter taking uniform and storage
  buffer loads, stores and atomics: a load whose last byte passes the size reads at offset 0
  instead, and a store or atomic past it is not executed (the atomic gives zero, an undefined
  value being allowed). That pass's own comment says only that Vulkan drivers get buffer
  robustness lowered through `nir_lower_explicit_io`, naming no address format; the bounded one
  that pass offers, `nir_address_format_64bit_bounded_global` (defined in `nir_defines.h`), makes
  64-bit global addresses (`load_global_constant_bounded` and its kin, checked:
  `nir_defines.h`, `nir_lower_explicit_io.c`), which the common form cannot hold since WebGPU has
  no buffer addresses, so this route keeps the index and offset form, which `nir_lower_explicit_io`
  does not bound, and bounds it with this pass. The sizes the pass reads (`get_ubo_size`,
  `get_ssbo_size`, which also give a runtime-sized array's length) are lowered to the
  descriptor's range in bytes, which the driver writes into the program's helper constants; no
  helper value carries it today, so 15.7 proposes `HelperValue::BufferRange`. The check runs on
  offsets within the descriptor's range, before the `StorageOffset` remainder is added.
  Every access the check lets through then lies inside the range, and so inside the binding the
  interface sees, if that binding meets the interface's rules (at least the program's block, a
  storage binding's size a multiple of four), which a descriptor's range need not: the CTS binds
  ranges of 1, 3, 4 and 32 bytes of buffers of that size to blocks of 8192 bytes (checked:
  `vktRobustnessBufferAccessTests.cpp`, `s_testArraySize` 128 `mat4`s, `range_1_byte` to
  `range_32_bytes`, both buffers created with the range's size). So:
  - Storage buffers: the converted program declares each as a runtime-sized array of 32-bit
    words, so its block is four bytes, and the driver binds the descriptor's range rounded up to a
    multiple of four. The driver reports every buffer's memory requirements as its size so
    rounded, so the bytes the rounding adds are memory bound to the buffer, which Vulkan lets an
    out-of-range read return ("may include bytes beyond the size of the buffer itself") and an
    out-of-range write modify ("within the memory range(s) bound to the underlying buffer"),
    checked: `shaders.adoc`. A word whose last byte passes the range is out of range; a load of
    it reads the word at offset 0, inside the binding, which is at least four bytes.
  - Uniform buffers: a WGSL uniform block has a fixed size, so the binding is at least the block
    the program declares. Where the memory bound to the buffer holds the block's size from the
    descriptor's offset, the driver binds that size (the check sends every read past the range to
    offset 0, so the bytes past it are never read). Where it does not, as in those CTS cases, it
    binds a copy grown to the block's size (`ConstantBufferGrown`, 5.1): the words holding the
    range copied on the GPU into a zeroed buffer of the block's size, before the pass or dispatch
    that reads it; the driver counts the source as read by that draw in the pass tracker, so a
    storage write to it earlier in the same pass splits the pass first (`UsageScopeSplit`). An
    access past the block the program declares is not served: Vulkan leaves it undefined even
    with the feature ("whether the underlying buffer is bigger than that or not", checked:
    `shaders.adoc`).
  Texel buffers take the typed-buffer route, which already reads zero past the view's
  length and drops writes there (`HelperValue::TexelBufferView`). Proposed name
  `RobustAccessInShader` (15.7), counted per program so made. Vertex input: draws whose ranges the
  driver sees on the CPU (draws without indices, and the instance streams of every direct draw;
  WebGPU's validation refuses those past the end): the part past a stream's end drawn with a zero
  buffer bound (`VertexFetchZeroed`), exact, as the Direct3D fronts'. Indexed and indirect draws,
  whose vertices it cannot see: the program's variant that fetches every vertex stream from storage
  with bounds checks against the bound range, reads past it giving zero (`VertexPulledInShader`,
  `VariantKey::pulled`, 4.12). On a device made without the feature, Vulkan leaves such accesses
  undefined: programs are converted without the checks and streams bound natively. A direct indexed
  draw whose indices reach past the bound index buffer breaks Vulkan's valid usage without
  `robustBufferAccess2`, which is not reported; the driver drops it with a warning rather than hand
  WebGPU a draw its validation refuses. Both routes' cost against the unchecked program is measured
  at G10 on the programs that enable the feature.
- `geometryShader`, `tessellationShader` (the stage helper, which matched Direct3D 11 on the RTX
  3080 in the shaders track) and `shaderTessellationAndGeometryPointSize` (the stage helper's points
  carry their size and are drawn as quads, `PointsAsQuads`, as OpenGL 4.6's).
- `multiview` (required; `MultiviewSerial`), and `multiviewGeometryShader` and
  `multiviewTessellationShader` (each serial draw runs the stage helper).
- `shaderDrawParameters` (`DrawBaseInHelpers`, `DrawBaseFromArguments`), `logicOp`
  (`LogicOpInShader` with `TargetReadPerPrimitive`, as `glLogicOp`), `fillModeNonSolid`
  (`FillModeConverted`, as `glPolygonMode`), `largePoints` (`PointsAsQuads`), `multiViewport`
  (`MultiViewportSplit`), `shaderFloat64` (`F64Emulated`, as OpenGL 4.6), `shaderCullDistance`
  (`CullDistanceAsGeometry`, as OpenGL 4.6), `vertexPipelineStoresAndAtomics`
  (`VertexStorageAsCompute`).
- `occlusionQueryPrecise`, only where the back end has blendable `r32float`: the occlusion counter
  (4.12), exact where WebGPU's query promises only non-zero. Without it the counter's `r16float`
  refuses some draws (a known failure), so the feature is false and the native query answers, as
  Vulkan allows.
- `pipelineStatisticsQuery`: the Direct3D 12 front's counts, the host's from each draw's arguments
  and `PixelInvocationsCounted` for fragment invocations.
- Three need a slow path the interface does not name yet (15.7): `wideLines`, drawn as quads as
  `AntialiasedLinesAsQuads` draws Direct3D's lines (`WideLinesAsQuads`); `shaderInt64`, by Mesa's
  `nir_lower_int64` into pairs of words (`Int64Emulated`); `alphaToOne` (`AlphaToOneInShader`):
  Mesa's `nir_lower_alpha_to_coverage` turns alpha into the sample mask in the program, then
  `nir_lower_alpha_to_one` writes alpha one (both in `src/compiler/nir/nir_lower_alpha.c`, MIT,
  checked); Vulkan specifies no algorithm from alpha to coverage beyond a count of ones
  proportional to alpha (checked: the specification's multisample coverage), so the program's
  pattern is one an implementation may use. Two steps of the NIR route (15.3) serve that pass's
  output, which WGSL cannot say as it stands (checked in the source): its `demote_samples` of the
  uncovered samples becomes the program's sample mask output, ANDed with any mask the program
  writes itself (all ones where it writes none), which removes the same samples; and its 16-bit
  integer arithmetic (`nir_u2u16`, 16-bit multiplies and a 16-bit rotate) is carried in 32-bit
  words holding the zero-extended value, for the operations that pass uses only: conversions,
  and, or, multiply and shift left masked to the low 16 bits after; shift counts masked to their
  low 4 bits, as NIR's 16-bit shifts take them; shift right on the zero-extended value. These give
  the same bits as 16-bit operations (held at G8 by a test over every 16-bit value and count).
  Programs' 16-bit integers stay absent (below): they would bring every other 16-bit operation. The
  OpenGL driver takes the same route for `GL_SAMPLE_ALPHA_TO_ONE`.

Absent, each with its reason (26):
- `depthBounds`: WebGPU has no depth bounds test, and a program cannot read the stored depth of an
  attachment its pass writes (one usage scope, 4.7); the Direct3D fronts report it absent too
  (4.12).
- `shaderStorageImageMultisample`: a multisampled texture is never a storage texture (4.2, WebGPU's
  rule).
- `shaderUniformBufferArrayDynamicIndexing`, `shaderStorageBufferArrayDynamicIndexing` and
  `shaderStorageImageArrayDynamicIndexing`: WebGPU has no binding arrays (`RESOURCE_TABLES` absent),
  and the shaders crate's switch serves sampled textures only (it refuses arrays of buffers and
  storage views by name, `ResourceArray`). Widening the switch to every load, store and atomic of a
  buffer or storage view is possible and not planned. Arrays indexed by constants still work.
- `shaderInt16`: WGSL has no 16-bit integers. Mesa can widen 16-bit arithmetic to 32 bits
  (`nir_lower_bit_size`), but that its results equal 16-bit ones at every operation (shifts by 16
  or more among them) is not checked, so it is not planned.
- `storageBuffer16BitAccess`, `uniformAndStorageBuffer16BitAccess`, `storagePushConstant16` and
  `storageInputOutput16`: each covers 16-bit integers and floats together. Without 16-bit integers
  in WGSL, a 16-bit integer in a buffer would be half of a 32-bit word written by reading and
  writing the word, which races with an invocation writing the other half. WebGPU's `shader-f16`
  gives floats only.
- `shaderResourceMinLod`: WGSL's sampling takes no clamp per call and cannot ask which level it
  would choose, so a clamp in the program would replace the GPU's choice of level with its own (not
  the same picture). `MinLodClamp` differs: it clamps a whole view through its sampler.
- `shaderResourceResidency` and the nine sparse features (`sparseBinding`, `sparseResidencyBuffer`,
  `sparseResidencyImage2D`, `sparseResidencyImage3D`, `sparseResidency2Samples`,
  `sparseResidency4Samples`, `sparseResidency8Samples`, `sparseResidency16Samples`,
  `sparseResidencyAliased`): WebGPU binds no memory to part of a resource; binding pages of one
  memory to several resources would need `AliasCopied`'s copies at every bind, and none of the
  programs 15.8 tests uses it. A scope choice, not a missing route; a later decision.
- `variableMultisampleRate`: a pass without attachments draws into the back end's own attachment
  of one sample count (4.7, 4.12). Reported false, Vulkan requires one sample count per such subpass,
  which is what that serves.
- `variablePointersStorageBuffer` and `variablePointers`: a WGSL pointer cannot be chosen at run
  time between variables (no `select` or merge of pointers), and the common form has no such
  pointer; rewriting each use into code per choice is not planned.
- `protectedMemory`: neither browser API nor the interface has protected memory.
- `samplerYcbcrConversion`: a scope choice, as sparse binding is. It serves video frames, none of
  15.8's programs uses it, and a route exists (each plane a texture of the interface's formats, the
  conversion in the program by Mesa's runtime, `nir_vk_lower_ycbcr_tex` in
  `src/vulkan/runtime/vk_nir_convert_ycbcr.c`, MIT, checked); a later decision.

- Reporting the slow features present follows the brief (a counted, warned slow path, not absence)
  and the Direct3D fronts' practice. A program that asks for one and leans on it runs slowly and
  says so through the counter; Blender's Vulkan path, which the research found needs several of
  them, is one to watch at G10.
- Limits at WebGPU's values where below Vulkan's minimum would be required, emulated where the minimum
  is required: 16 vertex bindings (WebGPU's 8; streams past the eighth fetched in the program,
  `VertexPulledInShader`), 128 bytes of push constants (`ImmediatesSpilled`).

What the interface lacks for it (proposed in 15.7): four descriptor sets with the helper set beside
them, view component mappings, and the slow path names `WideLinesAsQuads`, `Int64Emulated` and
`AlphaToOneInShader`.

How it is proven (G10): VK-GL-CTS's `vk-default` list through deqp-runner with a fails list,
NotSupported counted as passing (the CTS's own rule), in shards through the queue, natively on the
RTX 3080 with wgpu's Direct3D 12 path (15.1) and in Chrome; the driver is chosen for a job only through
the Vulkan loader's `VK_DRIVER_FILES` in that job's environment, so nothing is installed on the
desktop. Pictures of real programs (vkcube, Sascha Willems' samples (MIT), bgfx's Vulkan renderer,
Godot's Forward+ and Mobile renderers) against the RTX 3080's own Vulkan and lavapipe.

### 15.5 Stand-ins and the call channel

The guest side, one thin library per API, written in Rust as `no_std` libraries and generated from
each API's registry: Microsoft's Windows metadata (MIT) for DXGI, Direct3D 10, 11 and 12 (as 10
says), Khronos' `gl.xml`/`egl.xml` (Apache-2.0) with Mesa's `glapi` marshalling annotations (MIT; they
say which calls need an answer and how many bytes each pointer carries; to confirm at G7), and `vk.xml` (Apache-2.0).
Built per guest: x86-64 and arm64 Linux libraries (an EGL vendor library for glvnd with its manifest,
and a Vulkan ICD with its manifest, loaded by the image's own libglvnd and Vulkan loader), and x86-64
Windows libraries run by Wine (`d3d11.dll`, `d3d12.dll`, `dxgi.dll`, chosen with Wine's DLL overrides;
their Unix-side halves make the crossing, as Wine's own drivers do). The runtime places them and their
manifests in the container's file system. The EGL library and the ICD make Wayland surfaces (15.6).
Programs that draw through GLX (X11 only: SDL2 programs on X11, many older Linux games) need more,
and only if the owner takes Xwayland (15.6, 15.9): a GLX vendor library for glvnd of ours, written
from `glx.xml` (Apache-2.0) as the EGL one is, and the ICD's `VK_KHR_xlib_surface` and
`VK_KHR_xcb_surface`. Mesa's own GLX (`src/glx`, SGI-B-2.0) is never built. Without Xwayland, GLX
and those surfaces are absent: glvnd finds no GLX vendor and the ICD lists neither extension.

The channel, one primitive for all five fronts:
- **Streams in guest memory.** Each object that orders work has a stream, a ring of records (call
  number, length, arguments) in guest memory: a Direct3D 11 immediate or deferred context, a Direct3D
  12 command list or queue, an OpenGL context, a Vulkan command buffer or queue, and one per guest
  thread for device-level calls (creation, destruction, queries of the device). Recording into a
  command list, a deferred context or a command buffer touches only its own stream, so many guest
  threads record at once without meeting, and the device thread reads the stream in place when the
  list is executed or submitted (Venus encodes command buffers locally the same way, research Q4).
- **Order between streams.** A call that publishes work to another stream or another thread
  (creation and destruction, `ExecuteCommandLists`, `vkQueueSubmit`, `Present`, `glFlush`,
  `glFenceSync`, fence signals and waits) takes a ticket from one counter in shared memory; the
  device thread replays streams up to each ticket in ticket order. Calls inside one stream take
  none.
- **What is copied.** The stand-in copies into the stream every argument the API consumes at the
  call, deep (the registries say the lengths: Vulkan's create infos and their `pNext` chains,
  `glShaderSource` strings, `UpdateSubresource` bytes), so the program may reuse its memory when the
  call returns (a narrowing of the brief's "read in place", 15.1). Data the API reads later is never copied: mapped buffers and textures, Vulkan
  host-visible memory, Direct3D 12 upload heaps; the device thread reads those in place when the API
  says they are read (unmap, submit). A consumed argument larger than the stream's free space
  flushes the stream and is run at once, the device thread reading it in place while the guest waits;
  the stream's size is the only number involved.
- **Answers without crossing.** Object names are chosen by the stand-in (4.8). Query results, fence
  values and frame statistics land in result words in guest memory that the device thread writes, so
  `GetData`, `GetCompletedValue`, `vkGetFenceStatus` and `vkGetQueryPoolResults` without `WAIT` read
  memory (Chromium's command buffer keeps query results the same way, research 5). A `Discard` map
  takes a fresh block from a pool of guest blocks per dynamic resource, and a block returns to the
  pool when the frame that used it is done (a result word), so renaming never waits. Creation checks
  run in host code on the guest's thread (4.8).
- **Crossing, defined.** A crossing is the guest thread handing streams to the device thread, and
  waiting where it needs an answer: in the container one `ioctl` on the graphics device, in the
  Windows-library build one signal to the device thread. A host call that runs in host code on the
  guest's own thread without the device thread (creation checks, 4.8; in the container it too
  passes through the kernel) is not a crossing; it is counted apart, as host calls per call kind.
- **Crossing.** A stream crosses when a frame ends (`Present`, `eglSwapBuffers`, `vkQueuePresentKHR`),
  when a call needs an answer the result words cannot give (a read map, `glGetError` after a failing
  call, `glReadPixels`, `glGet*` of state Mesa holds, a wait), or when it is full. The guest crosses
  by an `ioctl` on a character device the kernel provides for the graphics hook; the kernel parks the
  thread until the hook answers, as for any blocking call. In the Windows-library build the shell
  writes the same streams in-process and the device thread is a thread of the process.
- **OpenGL's sync calls.** Mesa holds OpenGL's state on the device thread, so `glGet*` and
  `glGetError` cross. Mesa's glthread answers some of them from state kept on the calling side; the
  stand-in starts with none and G7 counts sync crossings per frame in real programs (a counter and a
  warning per call name); a call that real programs make every frame gets a guest-side answer only
  with that measurement.

How it is proven (G7): the three existing fronts' Windows libraries run on the channel and the rig's
scenes and programs draw as before (same pictures, same counters); each API's encoder and decoder are
generated from one description and a seeded random round trip of every call (count recorded) decodes
to the same calls; the ordering rule is checked by seeded random multi-threaded call sequences
replayed on the recording back end, whose recorded calls must equal a single-threaded run in ticket
order; creation never crosses (counters of crossings and of host calls per call kind: zero
crossings for creations, at most one host call each). The Direct3D 11 library gains its device
thread here (15.1). The copy of consumed arguments (15.1) is measured on the rig's programs: bytes
copied into streams per frame and the stand-ins' time copying, beside the frame time, with the
machine and its load.

The Direct3D stand-ins for Wine are proven before the container exists on the Linux laptop: a
pinned Wine release runs the rig's Direct3D 11 and 12 scene programs with our stand-ins chosen by
DLL overrides; their Unix-side halves run the device thread in the process (as the Windows-library
build does) over the WebGPU back end on lavapipe (Mesa's software Vulkan, from the pinned tree of
15.2, built on the laptop); the pictures match the rig's reference pictures within the rig's
allowance, equal to the GPU's or WARP's. Wine is LGPL-2.1 and only runs the programs, as the brief
has it.

### 15.6 Sound and input hooks

Like the graphics hook: optional host hooks beside the kernel's `Host` trait (`wcontainers-abi`), each
a host may lack. Their guest side is the kernel's, not an API stand-in: every Linux sound stack ends
at ALSA's kernel interface and every input stack at evdev, so serving those two covers programs,
sound servers and Wine alike. Sound is a few hundred kilobytes a second and input a few events a
frame, so the kernel-level cut costs nothing measurable (expected; measured at G11 and G12).

- **Sound hook.** `open_playback(rate, channels, format)`, `start(stream, ring, frames, period)`,
  `stop`, `close`; the hook reports the frames played as `Event::SoundPeriod` each period and writes
  the position into the stream's status page. The ring is the ALSA buffer the program maps, in guest
  memory, read in place: in the browser by an AudioWorklet over the shared memory, natively by the
  `cpal` crate (Apache-2.0, to confirm). The kernel serves `/dev/snd/pcmC0D0p` and `controlC0` (the
  hardware-parameter, prepare, start, drop, drain, sync-pointer and status calls, and the mapped
  status, control and data pages). Formats and rates offered are exactly what the hook plays
  (32-bit float or 16-bit integer at the audio context's rate); others are refused by ALSA's own
  hardware-parameter refinement, and alsa-lib's plug layer converts in the guest. Capture
  (microphone) is not planned (an owner decision).
- **Input hook.** Events: keys (as evdev codes), pointer (absolute in a surface, relative while the
  pointer is locked, buttons, wheel), gamepads (connect, buttons and axes by the browser's standard
  mapping), focus. The kernel serves them as evdev devices (`/dev/input/event*`: a keyboard, a mouse,
  one per gamepad, with the identification and capability queries) and as `Event::Input` to whatever
  shows windows.
- **Windows.** The brief does not say how a program gets a window, and input to a window goes
  through the window system. The fitting choice is a Wayland compositor in the host (the `smithay`
  crate, MIT, to confirm), one canvas surface per top-level window: Wine (`winewayland.drv`), SDL,
  GLFW, GTK and Qt speak Wayland; the stand-ins' EGL and WSI make Wayland surfaces on the interface's
  surfaces; input reaches windows as Wayland events and gamepads as evdev. X11-only programs would
  need Xwayland (a guest program, run rootless, each X top-level window a Wayland surface of the
  compositor), and, to draw, the GLX stand-in and the ICD's X11 surfaces of 15.5, which present on
  the interface surface behind the X window's Wayland surface. Xwayland ties the two by its
  `xwayland_shell_v1` protocol (checked 2026-11-18: in wayland-protocols' staging set, and smithay
  serves it in `smithay::wayland::xwayland_shell`). An owner decision (15.9).
  The guest reaches the compositor through a Unix socket (`$XDG_RUNTIME_DIR/wayland-0`) that the
  kernel connects to the host's compositor, passing file descriptors (`SCM_RIGHTS`) whose shared
  memory (`wl_shm`'s `memfd`s) is guest memory the compositor reads in place.

How they are proven. Guest programs run in the container on the native host (wasmtime) on the
laptop, x86-64 Linux programs run by the translator over the kernel; the same programs in Chrome
are G13's. Every sequence is seeded with its count recorded.
- **Sound (G11).** The guest side: `aplay` (alsa-utils, a guest program for testing only), an SDL
  program of ours playing a WAV through SDL's ALSA driver, and a Windows program of ours playing it
  through winmm's `PlaySound` under Wine (`winealsa.drv`) each play a known file in a format the
  hook offers; the samples a recording hook receives equal the file's exactly, and one file at a
  rate not offered plays through alsa-lib's plug layer with its frame count right. The native host:
  `cpal` plays a seeded signal written into a ring into the laptop's `snd-aloop` loopback device,
  and the samples captured from its other side equal the ring's. The browser host: the AudioWorklet
  node run in an `OfflineAudioContext` in Chrome renders exactly the ring's samples (float as is,
  16-bit integers over 32768), and in a running `AudioContext` the reported position advances at
  the context's rate within one period over ten seconds.
- **Input (G12).** Seeded random event sequences injected at the hook are read back in order and
  equal by an evdev reader of ours in the guest, by an SDL program of ours printing the game
  controller events it reads (SDL's evdev joystick driver), and by a Windows program of ours under
  Wine printing what `XInputGetState` returns (Wine's `winebus` reading the evdev device; which of
  its settings that needs is found at G12). The browser host's translation (DOM keyboard and pointer
  events, Pointer Lock, the Gamepad API's standard mapping into evdev codes) is a pure function,
  tested on recorded browser events, and keys and pointer moves sent to a rig page in Chrome through
  the DevTools protocol's `Input.dispatch*` arrive at the hook in order.
- **Windows (G12).** With Weston's demo clients (MIT, a pinned release, testing only):
  `weston-simple-shm`'s window appears as one interface surface whose pixels, read back, equal its
  buffer's; a seeded sequence of keyboard and pointer events at the hook reaches `weston-eventdemo`,
  which prints each event, in order with surface-local coordinates; two windows are two surfaces,
  and closing one destroys its surface. EGL and Vulkan surfaces on Wayland need the stand-ins, so
  they are proven at G13.

### 15.7 What the interface needs (proposed, not made)

Each in the interface's style; made at G1d with a rule test in the recorder and a suite test.
- **Four descriptor sets beside the helper set.** Vulkan requires `maxBoundDescriptorSets` of at least
  4 and `MAX_BINDING_SETS` is 4 with the helper set at 3, so a Vulkan program using set 3 meets the
  helpers. Proposed: a capability limit `binding_sets` (WebGPU's `maxBindGroups`, which adapters may
  raise past 4); where it is 5 or more the helper set moves past the program's sets; where it is 4,
  the conversion numbers the helper bindings past the program's own in its last set (a variant field
  naming that set) and the front adds them to that set's binding set (`HelpersInProgramSet`, a
  counted slow path: the front makes that set's binding set again whenever the helpers change). Which
  of the two Chrome allows is measured first.
- **View component mappings.** `TextureViewDesc` gains a component mapping (`ViewComponent`, which
  c6e80691 added), with a capability `VIEW_SWIZZLE` made in the same change and declared by back ends
  that take it, so WebGPU's `texture-component-swizzle` feature (Chrome 143, research
  `webgl2-vs-webgpu`; listed among WebGPU's optional features in the research, part 2, question
  2c) serves Vulkan's
  `VkComponentMapping`, OpenGL's texture swizzle (core in 3.3) and Direct3D 12's mappings natively;
  `VariantKey::swizzled` is then filled only where the back end lacks it. Today views carry no mapping
  and no capability names one (G1c removed a `VIEW_SWIZZLE` flag that no call used), so every front
  applies mappings in the program (`SwizzleInShader`).
- **Buffers whose uses are unknown at creation.** OpenGL buffers bind as anything; the OpenGL front
  creates them with every use but mapping. No change if every back end accepts that (WebGPU does;
  WebGL 2's `IndexBufferTwin` already covers indices); G9 checks it with a suite test.
- **Five slow path names**: `WideLinesAsQuads` (lines wider than one pixel drawn as quads, the
  route of `AntialiasedLinesAsQuads`), `Int64Emulated` (64-bit integers as pairs of 32-bit words)
  and `AlphaToOneInShader` (alpha to coverage made in the program as the sample mask, then alpha
  one; Vulkan's `alphaToOne` and OpenGL's `GL_SAMPLE_ALPHA_TO_ONE`) for the fronts' optional
  features (15.4), `RobustAccessInShader` (buffer accesses checked against their range in the
  program, Vulkan's `robustBufferAccess`, 15.4), and `HelpersInProgramSet` (above). Each a named
  reason added at the end of `SlowPath`.
- **A buffer binding's range as the API gave it.** `HelperValue::BufferRange { set, binding }`: the
  byte length (u32) of the range a front's API binds at a uniform or storage binding, which may be
  shorter than the binding the interface sees (rounded up to a multiple of four, or grown to the
  program's block). Vulkan's robust shader access checks against it and a runtime-sized array's
  length is computed from it (15.4); OpenGL's `length()` on an SSBO's last member, whose range
  `glBindBufferRange` sets, takes it the same way. A base primitive: a helper value the front writes
  as it writes the others, with nothing for a back end to do.
- **Nothing else is expected**: 5.3 to 5.5 found routes for the rest in the existing interface.

### 15.8 Milestones of the second batch

Each is checked by an independent agent against its pass bar. Measurements state the machine and
its load, as everywhere.

- **G1d, the interface for the second batch.** The changes of 15.7, each with a rule test and a
  suite test; the shared line kept as in the first batch. Pass bar: each change is in commits that
  touch only `crates/wcontainers-gpu` (and this plan's text), whose messages start "interface:",
  in the interface's style (section 12's rule: a base primitive, a fast path with a default, or a
  capability); every rule a caller can break with it is refused by the recording back end and
  broken once in a test; every new behaviour has a suite test, listed in `suite!`, that passes on
  the recording back end; the interface crates' tests pass and their lint and docs are clean with
  warnings denied; the whole workspace builds on the shared line after the other tracks' branches
  are merged; and the status file lists each change with its tests under "Interface changes".
  Needs: the owner's answers on 15.9.
- **G7, the call channel and the stand-ins.** `wcontainers-gpu-channel` (stream format, tickets,
  result words, the guest block pools), the encoder and decoder generators for all five APIs, the
  Direct3D stand-ins for Wine, and the three existing Windows libraries moved onto the channel
  (Direct3D 11 gaining its device thread). Pass bar: 15.5's proofs, including the Wine stand-ins on
  the laptop and the copy's measured cost; the rig's scenes and programs draw the same pictures as
  before through the channel; a frame of each rig program crosses once (15.5's definition) when
  nothing needs an answer (counted). Needs:
  the kernel track's character device and parking (the `ioctl` of 15.5), or a stand-in for it in the
  Windows-library build, which needs nothing.
- **G8, the NIR route.** `wcontainers-nir`, both candidates of 15.3 measured, one chosen, the cache
  keyed as 15.3 says, and the program rewrites 15.4 takes from the shaders crate made on the
  route's output (formatless storage, gathers with offsets at run time, arrays of textures,
  samplers and combined image samplers indexed at run time), each with a corpus shader using it;
  and the two steps 15.4's `alphaToOne` needs (`demote_samples` as the sample mask output, the
  pass's 16-bit integer operations in 32-bit words), with the fragment shaders of
  `dEQP-VK.pipeline.monolithic.multisample.alpha_to_one.*` after `nir_lower_alpha_to_coverage` and
  `nir_lower_alpha_to_one` in the corpus, and the widened operations held to 16-bit ones by a test
  over every 16-bit value and shift count; and the program of 15.4's `robustBufferAccess` (its
  three steps: index and offset, 32-bit scalar accesses, `nir_lower_robust_access`, with the sizes
  read from `HelperValue::BufferRange` and storage buffers declared as runtime-sized arrays of
  words), with the shaders of `dEQP-VK.robustness.buffer_access.*` in the corpus, lowered before
  both runs (so the reference runs the same checks), on seeded ranges that include the CTS's 1, 3,
  4 and 32 bytes against its 8192-byte blocks and accesses past the range, bound as 15.4 binds
  them (storage rounded up to four bytes, uniform grown to the block), and none of the converted
  program's accesses outside its binding (the recording back end stops at one). Whether lavapipe runs the lowered NIR
  (`demote_samples`) is checked first; where it does not, those shaders are held by G10's CTS
  cases, listed so. Pass bar: 15.3's, the outputs of every converted shader held to llvmpipe's or
  lavapipe's. Needs:
  Mesa pinned and building on the laptop (with llvmpipe and lavapipe), and the rig's queue for
  shaders past the recorder's limits.
- **G9, OpenGL natively.** `wcontainers-opengl` and Mesa built for Windows with it, llvmpipe beside.
  Pass bar, on the RTX 3080 through deqp-runner, over 15.2's lists from the pinned mustpass: the
  measure is passed cases over cases run, where a case run is any result but NotSupported, a pass
  is deqp-runner's Pass or Warn (the CTS's QualityWarning and CompatibilityWarning, which Khronos'
  conformance accepts), a Flake (failed, then passed when run again) is a failure and is listed, and
  every NotSupported case asks for an extension or limit the driver does not report (checked by
  listing the groups with what each asks);
  the measure is at least 92.9% over the GLES lists and the KHR-GL33 list together (gex_wgpu's
  published GLES share, taken as a floor, not a like-for-like comparison: 15.2); every failure is
  listed with a reason, and none that real OpenGL on the same GPU passes is left without a named
  one; bgfx's and Godot's OpenGL renderers match the RTX 3080's OpenGL or llvmpipe within the rig's
  allowance; every slow path taken is counted. G9's second part: 4.6 reported, and the same measure
  over KHR-GL46 at least 98.6% (gex_wgpu's published 4.6 share, a floor likewise). Needs: G8.
- **G9b, OpenGL in the browser.** The same driver built for `wasm32-wasip1`, loaded as a module on
  first use, on both back ends (3.1 on WebGL 2). First step: 15.2's open questions (pthreads,
  `setjmp`, size, the Rust driver in the wasi-sdk module, the back end as its own module, how bytes
  cross between modules, and the boundary's cost per call and per byte), and how the conformance
  suites run in Chrome, each answered with what was built and measured. The suites run in Chrome
  without the stand-in, which needs the translator and kernel (G13): dEQP and the KHR lists built for
  WebAssembly with a platform layer of ours that makes contexts on `opengl.wasm` and calls its GL
  entry points directly, run in shards from a page of the rig. Not yet known: whether they build with
  wasi-sdk (dEQP throws C++ exceptions); if not, with Emscripten (MIT and NCSA), for the test build
  only, linked to the same modules. Pass bar: G9's measure on WebGPU in Chrome over G9's lists (on
  WebGL 2, the 3.1 lists, every failure listed with a reason); one device per page when a program
  uses Direct3D and OpenGL together (a test program doing both); the module's size and load time
  measured. Needs: G9.
- **G10, Vulkan 1.1.** `wcontainers-vulkan` on Mesa's runtime, natively and in Chrome. Pass bar:
  `vk-default` through deqp-runner with every failure listed with a reason, and of the cases run
  (any result but NotSupported) at least 98.5% passed (wvk's published share, a floor, not a
  like-for-like comparison: 15.4); vkcube, the Sascha Willems
  samples that need only what 15.4 reports, and Godot's Forward+ and Mobile renderers match the RTX
  3080's Vulkan or lavapipe within the rig's allowance; the reported features are exactly 15.4's
  (the 67 features as `vulkaninfo` lists them on the RTX 3080, against 15.4's five lists; and a
  test of the driver over the recording back end, `Recorder::with_caps` taking away in turn each
  capability a feature depends on, giving each adapter-dependent feature's answer both ways);
  `robustBufferAccess` (15.4) held by `dEQP-VK.robustness.vertex_access.*` (draws with and
  without indices) and `dEQP-VK.robustness.buffer_access.*` (uniform and storage reads and storage
  writes past the range from vertex, fragment and compute programs, texel buffers among them;
  checked: `vktRobustnessBufferAccessTests.cpp`) among `vk-default`, and by a test of the driver
  over the recording back end, which drops and counts any draw fetching past a stream's end and
  stops at any program access outside its bound range: on a device with the feature enabled, the
  same cases' draws and dispatches (indices past a stream, uniform and storage accesses past their
  descriptor's range, with ranges of 1, 3, 4 and 32 bytes of buffers of that size against blocks
  larger than them, texel buffer elements past the view) run with none dropped or stopped and
  none of their bindings refused, and give zero, a value from the memory bound to the buffer, or
  a dropped write, as 15.4 says; with it disabled, the converted programs hold no check.
  The browser half: `vulkan.wasm` in Chrome on WebGPU, `vk-default` built for WebAssembly as G9b
  builds dEQP (the CTS calling the driver's entry points without a loader), with the same measure.
  Needs: G1d, G8 (G9b's way of running the suites in Chrome for the browser half).
- **G11, sound.** The sound hook, its native host (`cpal`) and browser host (the AudioWorklet), and
  the ALSA device. Pass bar: 15.6's sound proofs, all three parts (guest programs through a
  recording hook, the native host through the loopback, the browser host in Chrome). Needs, from
  the kernel track: the ALSA character devices with their `ioctl`s dispatched to the hook and their
  status, control and data pages mapped as guest memory the hook reads in place; from the
  translator track: `aplay`, an SDL program and Wine running as x86-64 Linux guests in the
  container on the native host. Without them, only the two hosts' parts can be checked.
- **G12, input and windows.** The input hook, its browser and native hosts, the evdev devices, and
  the window system the owner chooses (15.6's Wayland compositor on smithay unless the owner says
  otherwise). Pass bar: 15.6's input and windows proofs. Needs, from the kernel track: evdev
  character devices (read, `poll`, the `EVIOCG*` queries), a Unix socket at
  `$XDG_RUNTIME_DIR/wayland-0` connected to the host's compositor with `SCM_RIGHTS` and `memfd`s
  whose pages are guest memory the host can address; from the translator track: the evdev reader,
  SDL, Wine and Weston's clients running as x86-64 Linux guests on the native host.
- **G13, the second batch in the container.** The stand-ins of 15.5 as guest libraries (the EGL
  vendor library, the Vulkan ICD, and the Wine stand-ins) in a container in Chrome, crossing by the
  kernel's character device. Pass bar: the rig's Direct3D programs under Wine, bgfx's and Godot's
  OpenGL and Vulkan renderers built for Linux, and shards of G9's and G10's lists run as guest
  programs, draw in Chrome as they do natively (the same pictures within the rig's allowance, the
  same measure); they draw into windows of G12's compositor through the EGL library's and the ICD's
  Wayland surfaces (each window one interface surface, resized and closed with it); a frame crosses
  once when nothing needs an answer (counted); G11's and G12's guest programs pass their proofs in
  Chrome. If the owner takes Xwayland: the GLX stand-in and the ICD's X11 surfaces, with a GLX
  program of ours and an SDL2 program forced onto X11 (`SDL_VIDEODRIVER=x11`) drawing as they do
  natively. Needs: G7, G9b, G10, G11,
  G12's window system (the programs' windows), and the translator and kernel tracks running guest
  Linux programs in Chrome.

### 15.9 Decisions that are the owner's

- Whether to adopt `gex_wgpu` (MIT on its C sources, no licence on its build files or in the
  repository, written for gex) as the OpenGL
  driver, or write ours on the interface as 15.2 recommends; and the same for wvk, the same
  repository's Vulkan driver (15.4), against the brief's new driver. Neither is planned; a
  like-for-like comparison would run them natively over Dawn on the rig's lists and GPU.
- Vulkan's version: 1.1 as the brief says, with 1.2 left for later (wvk, on the same runtime over
  WebGPU, reports 1.3 without buffer device addresses by default, which is not a conformant 1.3,
  and 1.2 under an environment variable; 15.4).
- Vulkan's optional features reported present through counted slow paths (15.4) rather than absent,
  and the 26 absent ones of 15.4, two of them scope choices with a route (sparse binding and
  residency, and Y'CbCr conversion).
- Consumed arguments copied into the call streams rather than read in place (15.1, 15.5), a
  narrowing of the brief.
- The back end as its own WebAssembly module per page that every front module calls (15.2), a
  change to section 10's one module per front and back end, and to the brief's "one ordinary call
  per operation": each interface call becomes an import call across modules that cannot be
  inlined (its cost measured at G9b; if no way is cheap, the Direct3D module keeps its back end
  inside and a second API is refused its own device by name). How bytes cross between modules
  (15.2) applies to the first batch's shaders module and guest memory too. OpenGL and Vulkan as two
  modules, each holding Mesa's shared code.
- The window system: a Wayland compositor in the host (15.6), and whether X11-only programs
  (Xwayland) are in scope; taking them also takes a GLX stand-in of ours and the Vulkan ICD's X11
  surfaces (15.5), proven at G13.
- Sound capture (the microphone): in or out.
- Direct3D's indexed draws whose indices reach past a vertex buffer's end, and its shaders'
  accesses past a constant, raw or structured buffer's range: kept listed losses (WebGPU may cancel
  or partly discard the draw, 4.12), or made exact as Vulkan's robust devices are (15.4: the pulled
  variant, and the check in the program), which every indexed draw and every program would pay
  for.
- The interface changes of 15.7, which change the fronts' code.
- Names: `wcontainers-opengl`, `wcontainers-vulkan`, `wcontainers-nir`, `wcontainers-gpu-channel`,
  and the modules `gpu-webgpu.wasm`, `gpu-webgl.wasm`, `opengl.wasm`, `vulkan.wasm`; and where our
  fork of Mesa, the submodule of 15.2, is kept.
- Licences of what is new to the project: Mesa (MIT with the permissive files of 15.2), bindgen,
  cpal and smithay (to confirm at their milestones), and VK-GL-CTS, dEQP and piglit for testing only
  (with Emscripten for their browser build only, if wasi-sdk cannot build them, G9b); Weston's demo
  clients (MIT) and alsa-utils' `aplay` (GPL-2.0) as guest programs for testing only (15.6).
