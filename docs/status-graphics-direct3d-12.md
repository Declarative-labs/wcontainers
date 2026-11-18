# Direct3D 12 and DXGI infrastructure status (G5)

Branch: phase-2-graphics-direct3d-12. Plan: docs/phase-2-graphics.md, sections 4.9, 5.2, 9, 10 and
G5 in 12. This track owns crates/wcontainers-directx-graphics-infrastructure and
crates/wcontainers-direct3d-12.

## Where things stand

G5 is not complete: 23 test files wait on the shaders track (G6) and 3 on a view swizzle (G1
and G6); see "Gap list" and "Next". A real Direct3D 12 program now draws matching frames (run
21: four of bgfx's examples, "Real program"). Run history (runs 1 to 20 on 2026-11-17, runs 21
to 24 on 2026-11-18): runs 1 to 4 the
first slice; 5 Wine's and vkd3d's tests through ours, root signatures; 6 compute, UAVs, queue
waits, occlusion, predication, heaps; 7 alias copies, `ExecuteIndirect`, the pass bar's cases;
8 to 10 allocators, resolves, twin textures, null descriptors, custom heaps, bundles, newer
interfaces, every `CheckFeatureSupport` feature; 11 placed resources keeping heaps alive, root
signature checksums; 12 half and packed UAV clears, 1D UAVs, transitions checked at `Close`;
13 counted occlusion, draw bases, dual-source blending, depth twins; 14 statistics queries,
bases from indirect arguments, rectangle UAV clears; 15 pulled vertex streams, restart
rewrites, render passes; 16 custom heap buffers, scaling-none swap chains, bgfx tried.
- Run 17 (first run of the milestone "G5 to the pass bar"; the thirteenth of G5): merged the
  shaders, rig and Direct3D 11 branches (no conflicts; the shared line, webgpu and webgl had
  nothing new). The shaders branch now has the stage helper for DXBC geometry shaders, which the
  front now uses (`src/device/geometry.rs`, below): vkd3d's test_geometry_shader passes, and a
  new own case `geometry` (five geometry shaders: points to quads with a root constant buffer
  seen by the geometry stage alone and instances from 1, a cut triangle strip, three geometry
  instances of indexed triangles with a base vertex, triangles with adjacency, points to a
  varying number of points) equals real Direct3D 12 pixel for pixel. A draw or dispatch reading
  a root argument not set since its root signature was set is skipped with a warning instead of
  removing the device (Direct3D 12 leaves it undefined; real Direct3D 12 draws garbage). The
  rig now holds tests that fail some checks on real Direct3D 12 to every other check, so the
  counted set grew from 193 to 198: 129 of 198 pass (see "Test files"). All 22 own cases equal.
- Run 18 (second run of the milestone): merged the shared line, webgpu (timestamps) and shaders
  (storage textures, atomics, view counters, stream output through the stage helper). The
  front now has timestamps where the back end has them (the Windows library makes its wgpu
  instance with `WebGpu::INSTANCE_FLAGS`, so they count nanoseconds), copy queue timestamp
  heaps, `GetClockCalibration`, storage textures in their views' formats (`formats.rs`, below),
  view counters, and stream output (`geometry.rs`, below). bgfx's Direct3D 12 examples now get
  past the timestamps and stop at a crash not yet understood (Real program).
- Run 19 (third run of the milestone): merged the shared line, webgpu (A8 and the 16-bit packed
  formats held converted) and shaders (typed buffers through storage, storage textures through
  buffers). The front now reads typed buffers through storage buffers, writes helper blocks for
  pixel and compute stages, applies level-of-detail bias, border colours, mirror-once
  addressing and the minimum level-of-detail clamp in the program, and reads storage textures
  through buffers of their texels for atomics and for formats the back end cannot store (all
  below). 155 of 198 test files pass (145 in run 18), none lost.
- Run 20 (fourth run of the milestone): merged webgpu (query pages, freed tables, one ring of
  ordered writes), shaders (the tessellator alone, not yet hull and domain stages; naga's 16-bit
  normalised storage capability) and rig (failing tests held to exact evaluations; its new
  comparison stops at the first failing test, see "How to run"). The front now keeps root
  signatures alive while pipelines and recorded lists name them (vkd3d's shader runner releases
  one before its list runs), filters in the program textures the back end cannot filter where a
  filtering sampler samples them (`formats.rs`, below), and refuses multisampled texture arrays
  at creation. 157 of 198 test files pass (depth_load and typed_uav_store now pass with the
  capability fix), none lost; 24 of 24 own cases equal real Direct3D 12.
- Run 21 (fifth run of the milestone): merged webgpu, shaders (hull shaders to compute and
  domain shaders to vertex programs, not yet run through `Stages`) and rig (its forwarding
  library now stands in for d3d12.dll with a frame clock and Direct3D 12 frame capture). The
  Windows library is now `wcontainers_d3d12.dll` with two forwarders, `d3d12.dll` and
  `dxgi.dll` (examples/d3d12.rs, examples/dxgi.rs): the rig's forwarding library takes the name
  d3d12.dll and loads ours behind it (WC_D3D12), and ours had been named d3d12.dll too, so its
  dxgi.dll forwarded back into the rig's library and bgfx crashed. With it, bgfx's 01-cubes,
  38-bloom, 26-occlusion and 35-dynamic on Direct3D 12 draw frames 10, 60 and 120 as real
  Direct3D 12 does ("Real program"). Test files, cases and the shader runner unchanged: 157 of
  198, 24 of 24, 204 of 302.
- Run 22 (sixth run of the milestone): merged shaders (DXIL typed buffers and storage images
  without a format, converted with the bound views' formats), webgpu and rig. The front now
  reads a DXIL shader's typed views from its resource list (`src/dxil.rs`, below), so those
  convert: 8 shader runner files now pass their shader model 6.0 compute and pixel sections or
  get further (one of them, sm6-uav-rwtexture, now draws a wrong value where it was refused,
  "Shader runner"), and bgfx's 24-nbody runs its 120 frames through ours. Pixel shader
  invocations of pipeline statistics queries are now counted on the GPU with no wait
  (`statistics.rs`, below; interface: `COUNTER_BINDING`, `SlowPath::PixelInvocationsCounted`).
  Test files 157 of 198 (the same 41 failing), 24 of 24 own cases, shader runner 204 of 302.
- Run 23 (seventh run of the milestone): merged shaders (DXIL resources numbered by Direct3D
  12's rule, each kind's slots the program's own resources in (space, register) order; interface
  `ReflectedBinding::slot`) and rig. The front now names every per-slot thing (typed views'
  formats, sampler and texture helpers, clamps, texels read through buffers) by
  `ReflectedBinding::slot()` instead of the register, and `src/dxil.rs` gives a DXIL typed
  view its slot by ranking its class's (space, register) places. Counted occlusion is now
  summed on the GPU (interface: `OcclusionCounter::end_into` and a summing dispatch, below):
  bgfx 26-occlusion runs its 120 frames in 2.8 and 2.7 s (run 22: 47 to 55 s; real 2.5 and
  2.6 s), every compared frame equal. `D3D12GetInterface` and
  `D3D12EnableExperimentalFeatures` are now exported (no class and no experimental feature
  exists): the rig's forwarding library aborts a program calling an export the library behind
  it lacks, which was Godot's crash at start; Godot now runs its 120 frames ("Real program").
  Test files 157 of 198 (the same 41), 24 of 24 own cases, shader runner 204 of 302 (the same
  98), the crates' 40 tests pass.
- Run 24 (eighth run of the milestone): merged webgpu and shaders (vertex shaders without a
  position; hull and domain shaders through `Stages::tessellated`; `Stages::destroy`); the rig
  and the shared line had nothing new. The front now routes hull and domain shaders through the
  stage helper (below, "Hull and domain shaders") and reads back `ExecuteIndirect` draws of
  pipelines through the helper. Test files 162 of 198 (run24/full1.log): newly passing
  hull_shader_fork_phase, nop_tessellation_shaders, tessellation_dcl_index_range,
  tessellation_primitive_id and vertex_id, none lost. 24 of 24 own cases (run24/check2.log),
  shader runner 204 of 302 (the same 98; hlsl tessellation-patch-vars now passes its shader
  model 5 section and stops at DXIL hull shaders, G6), the crates' 40 tests pass.

- `wcontainers-directx-graphics-infrastructure`, the portable core: the adapter description from the
  back end's `Caps`; output descriptions, their modes, and the colour spaces swap chains present in
  (`presents_in`, `sdr`); DXGI format numbers to interface formats; swap chains (flip sequential,
  flip discard, blit discard, blit sequential) with presenting by one copy into the host surface (sRGB
  and float back buffers encoded on the way), resizing, and frame pacing by the maximum frame latency;
  what each DXGI format can be used for. It depends only on `wcontainers-gpu` now (run 5: the present
  copy's shaders come from the interface's `ShaderModule::from_wgsl`). 9 tests on a recorder whose GPU
  finishes work only when the test says so.
- `wcontainers-direct3d-12`, the front (plain Rust, generic over the back end): serialized root
  signatures and their conversion between versions 1.0 and 1.1 (`RootSignatureDesc::at_version`),
  `D3D12CreateDevice` and `CheckFeatureSupport` (plan section 9), GPU virtual addresses, descriptor
  heaps owned by the front (CPU handles point into its slots; GPU handles are numbers it maps back),
  committed resources, `GetCopyableFootprints`, graphics pipeline states from DXIL or DXBC (through
  `wcontainers_shaders::convert`), command lists recorded front-side and replayed at
  `ExecuteCommandLists` (each list with its own state), fences, swap chains.
- Bindings (run 5, `src/device/bind.rs`): at pipeline creation every binding the stages' reflections
  name must be found in the root signature (else `E_INVALIDARG`, as Direct3D 12); at each draw the
  bound root signature is searched for each binding by kind, register space and register, the
  descriptor is read from the heap as it is at replay, and one binding set per stage set is made
  (the back end caches equal sets). Root constants are written into the data ring and bound as a
  uniform buffer of the size the program declares (`ImmediatesSpilled`, counted and warned: the
  converter does not read root constants as immediates yet). Constant buffer views bind the size the
  program declares. Texture views are cached per resource and description; samplers per description.
  Upload buffers a binding reads are written to the GPU once per `ExecuteCommandLists`
  (`MappedUploadWhole`).
- Compute and storage (run 6, `src/device/bind.rs`, `src/device.rs`): `CreateComputePipelineState`,
  `Dispatch` (zero groups do nothing), the compute root signature and arguments. A binding's form
  is read from the converted program: a storage-space buffer binds a raw or structured view (SRV
  read-only, UAV writable) or a root SRV/UAV (the buffer from the address to its end); a storage
  image binds a 2D, 2D array or whole 3D UAV. Offsets the back end cannot bind (WebGPU: multiples
  of 256) are refused until the converter's `StorageOffset` variant exists. Upload buffers can be
  read as storage. `ClearUnorderedAccessViewUint` writes the bits with no conversion,
  `...Float` converts to the format (unorm, snorm, 32-bit float, integers): buffers by
  `clear_buffer` with one texel's bytes (raw and structured: the first value; formats with 8, 16 or
  32-bit components), textures by `clear_texture` (UAV textures also get `RENDER` where the format
  renders, for that pass).
- Queues (run 6): each guest queue holds its work behind a `Wait` not yet met; a wait is met by a
  value reached by work already replayed or by the CPU. A queue `Signal` or a CPU `Signal` releases
  held work, earliest call first. A wait never met holds its queue for good. Test
  `queues_wait_for_fences_signalled_late` (recorder); vkd3d's `test_queue_wait` passes.
- Counted occlusion (run 13): `BeginQuery(OCCLUSION)` ends the pass and begins the interface's
  `OcclusionCounter`; each draw is kept with the pipeline's counting program (its pixel shader
  converted with `VariantFlags::COUNTING`, made at the first counted draw; a WGSL one writing 1.0
  for pipelines without a pixel shader), and the bound targets are given to a `PassTracker` the
  counter reads (the front's own passes stay its own, as the Direct3D 11 front does). Every pass
  end counts the kept draws (`OcclusionCountReplay`). Since run 23 the counter sums its counting
  target on the GPU into a buffer of its own where the back end computes and no pixel can reach
  the counting format's bound, and `EndQuery` has it copy the count into the heap's count
  buffer (`end_into`), so nothing waits; otherwise it reads back, as before. `ResolveQueryData`
  copies from the count buffer.
- DXIL typed views (run 22, `src/dxil.rs`): a DXIL shader's typed unordered access views (of
  textures and buffers) and typed buffers are read from its module's resource list
  (`!dx.resources` in the DXIL chunk's LLVM bitcode, read with the `llvm-bitcode` crate:
  module constants, metadata nodes and the named node; component types as `DXIL.rst`'s
  `ComponentType`), as `dxbc.rs` reads DXBC's declarations; `formats::typed_key` takes either.
  So DXIL storage images and typed buffers are converted with each slot's 32-bit format of
  its kind first and with the bound views' formats at the draw, as DXBC's are. Since run 23 a
  view's slot is its place among every view of its class in the list in (space, lower bound)
  order, the converter's Direct3D 12 numbering (DXBC keeps the register). The converter does
  not read DXIL storage images through buffers yet, so DXIL atomics on images stay refused
  (G6).
- Statistics (run 14, `src/device/statistics.rs`): pipeline statistics heaps and stream output
  statistics heaps keep a buffer of values the front writes at `EndQuery` (an ordered write),
  which `ResolveQueryData` copies from, as for counted occlusion. Input assembler, vertex shader
  and clipper counts come from each draw's arguments as a GPU without vertex reuse or clipping
  splits counts them (indexed strips with a cut value read their indices back, `ReadBackWait`;
  while a query runs, `ExecuteIndirect` reads its arguments back and predicates are read back);
  compute invocations are groups times the program's group size. Pixel shader invocations
  (run 22): each draw of a pipeline with a pixel shader is drawn again right after it in the
  same pass with the depth and stencil tests off, no colour written and a WGSL program adding
  one to a 64-bit counter in a storage buffer (at `COUNTER_BINDING` in the helper set) once a
  pixel, or once a sample for pixel shaders run per sample (`PixelInvocationsCounted`);
  `BeginQuery` clears the counter and `EndQuery` copies it into the value after the ordered
  write of the other counts. Nothing waits (before run 22 an occlusion counter replayed the
  draws and read its count back at every pass end). Refused by name: a counted draw fetching
  its streams in the program, and one inside a running binary occlusion query (the counting
  draw would be counted by it). `D3D12CreateDevice` now also requires the back end's
  `FRAGMENT_STORAGE_WRITE` (feature level 11_0 has unordered access views in pixel shaders). Geometry, hull and domain counts are zero: draws through those stages are refused while a
  statistics query runs. Heap types 4, 5 and 7 (video
  decode statistics, copy queue timestamps, mesh shader statistics) answer E_NOTIMPL.
- Queries and predication (run 6, `src/device/query.rs`): occlusion heaps are `OcclusionAny` query
  sets; a pass opens with the set of a begun query, and a query begun with no draw writes its zero
  by an empty pass. `ResolveQueryData` resolves into a buffer of the front's, a compute program
  turns binary answers into exactly 0 or 1, and a copy puts them at the destination (readback
  heaps then read back). `SetPredication`: the draws and dispatches up to the next `SetPredication`
  get argument records, a compute program zeroes the counts the predicate skips, and they are drawn
  indirect (`PredicationIndirect`, nothing waits); clears and copies under a predicate read its
  value back once (`PredicationWait`).
- Heaps (runs 6, 7): `CreateHeap`, `CreatePlacedResource` (offset alignment and bounds checked, as
  Direct3D 12). In default heaps each placed resource has storage of its own and inherits by copies
  (`src/device/alias.rs`, plan 5.2): at placement it lists the live resources it inherits from
  (overlapping buffers; identically described textures at the same offset), every recorded GPU
  write stamps the resource, and before any use (copy, clear, draw target, vertex/index buffer,
  binding, resolve, predicate, indirect arguments) the bytes of listed resources written since are
  copied in, oldest first (`AliasCopied`; copies before a pass opens, ending it if needed). While no
  resource has such a list nothing is noted. Upload and readback heaps have one guest memory their
  placed buffers map (`Host`), so the CPU's bytes are shared directly.
- `ExecuteIndirect` (run 7, `src/device/indirect.rs`): a signature of one draw, indexed draw or
  dispatch becomes one indirect call per command at the stride; with a count buffer the commands
  are copied and a compute program zeroes those past the count (`IndirectCountExpanded`). Other
  signatures (vertex/index buffers, constants, root views), draws on back ends without
  `INDIRECT_FIRST_INSTANCE`, draws with per-instance streams of step rate 0 (bound at the first
  instance's element), draws while a counted occlusion or statistics query runs, and programs
  reading `SV_VertexID`/`SV_InstanceID` whose arguments variant did not convert read the
  arguments back and record ordinary calls (`IndirectReadBack`). Run 14: pipelines whose vertex
  shader reads the bases also get a program converted with `DrawBaseSource::Arguments` at
  creation; their `ExecuteIndirect` draws bind the argument buffer at `DRAW_ARGUMENTS_BINDING`
  and a helper block with the command's word offset (`DrawBaseFromArguments`), nothing waits. Argument buffers shorter than
  offset + max × stride fail `Close` with E_INVALIDARG, as real Direct3D 12 does (measured).
- Depth-stencil planes (run 7): D24S8 and D32S8 have two planes of subresources; footprints give
  plane 0 as `R32_TYPELESS` (4 bytes) and plane 1 as `R8_TYPELESS`, rows aligned to 512 (real
  Direct3D 12 on this desktop does so for planar formats only; single-plane depth keeps 256), and
  copies move the plane's aspect.
- Draws with no render target (run 7) open an empty pass as large as the viewports, at the
  pipeline's sample count; a viewport change ends it.
- Buffers in custom heaps the CPU sees (run 16, `HeapType::Shared`): write-combine or write-back
  heaps in system memory. Their buffers have program memory as upload and readback buffers do
  (`Map` gives it; placed buffers share their heap's) and a GPU buffer. Before work reading one,
  its memory is written to the GPU once per `ExecuteCommandLists` (`MappedUploadWhole`), leaving
  out the bytes whose GPU writes have not reached the program's memory yet (the GPU's copy is
  newer); bytes another buffer over the same memory got are waited for first (`ReadBackWait`).
  What the GPU writes into one (copies, query resolves, immediate writes) is read back into the
  memory as for readback buffers. Refused by name: such buffers allowing unordered access (their
  storage writes would need reading back after every draw), and reading one after an overlapping
  one was written in the same submission. Textures in such heaps are stored as default ones, as
  before.
- Restarts (run 15): WebGPU restarts strips at 0xffff for 16-bit and 0xffffffff for 32-bit
  indices, always; Direct3D 12 at the pipeline's cut value. Indexed strip draws whose cut value
  differs from WebGPU's draw from a scratch buffer of 32-bit indices of the draw's range that a
  compute program makes (`RestartRewrite`): 16-bit ones widened, 32-bit ones with 0xffff made
  0xffffffff. `ExecuteIndirect` reads such draws' arguments back; predicated ones are refused by
  name. 32-bit indices of 0xffffffff with the cut off still restart (that vertex lies past any
  buffer Direct3D 12 can bind, so it reads zeros there).
- Geometry shaders (run 17, `src/device/geometry.rs`, plan section 7): a pipeline with a
  geometry shader converts its vertex shader again with every input fetched in the program
  (`VariantKey::pulled`, the bases in its helper block), the geometry shader with the pixel
  stage's interpolation, and makes the shaders crate's `Stages` from both; the pipeline's program
  is the helper's pass-through vertex program with the pixel program. Each draw ends the open
  pass, binds the vertex stage's bindings and pulled streams for compute (root visibility
  vertex), the geometry stage's (visibility geometry), runs `Stages::run` (the two dispatches,
  `GeometryAsCompute`) and draws its output in a new pass with the pixel stage's bindings.
  Adjacency topologies are taken by `IASetPrimitiveTopology` and drawn only through a geometry
  shader (without one: E_NOTIMPL at the draw, as before at the call). Refused by name: geometry
  shaders on back ends without compute, indirect and predicated draws through them, draws
  through them while a counted occlusion or statistics query runs, and what the helper refuses
  (indexed strips, triangle strips with adjacency, streams other than 0, layer and viewport
  outputs, DXIL geometry shaders, which naga's SPIR-V reader refuses). A draw whose topology
  does not give the shader's input draws nothing. Each binding knows its API stage (its
  `D3D12_SHADER_VISIBILITY`) apart from the back end's stages, so root parameters are matched by
  the stage the program was written for.
- Timestamps (run 18): timestamp heaps are `Timestamp` query sets where the back end has
  `TIMESTAMPS`; copy queue timestamp heaps are the same (every queue's work runs in one
  submission order), and `D3D12_OPTIONS3` says so. `GetClockCalibration` lets the submitted work
  finish, reads `QueryPerformanceCounter` just before submitting a lone timestamp and reads that
  back (`ReadBackWait`): the GPU value is later by the submission's latency, not measured.
- Storage textures (run 18, `src/device/formats.rs`, `src/dxbc.rs`): WebGPU's storage textures
  name their format in the program, Direct3D's only in the bound view. A DXBC pixel or compute
  shader's typed texture views are read from its declarations (`dcl_uav_typed`, register and
  declared kind); the pipeline's program is converted with each as the 32-bit single-channel
  format of its kind (`VariantKey::storage_formats`; the only formats Direct3D guarantees typed
  loads of). Each draw or dispatch notes the formats of the views it binds; for other formats
  the stage is converted again and the program kept per pipeline and formats (`PipelineMiss`
  counts it). A null view binds the program's own format. Refused by name: such variants with a
  pulled or arguments vertex program or a geometry stage, and typed texture views in vertex and
  geometry shaders (the converter's `StorageFormat`). Unordered access views of a texture array
  with more layers than are left take the layers left, as shader resource views do.
- View counters (run 18): `CreateUnorderedAccessView`'s counter resource and offset are kept in
  the descriptor as a GPU virtual address (in the two words shader resource views use for the
  mapping and the clamp, so descriptors stay 64 bytes); a program's counter binding
  (`SlotKind::Counter`) is found where its view is and binds those 4 bytes as writable storage;
  a view made without a counter binds zeros whose writes are dropped.
- Stream output (run 18, `src/device/geometry.rs`): a pipeline with a stream output
  declaration makes the stage helper with it, with or without a geometry shader. `SOSetTargets`
  binds views; at each draw the filled sizes are copied from where the program keeps them into
  the helper's block, the helper writes the buffers, and the sizes are copied back (4 bytes,
  the high word left as it is). Without a geometry shader the draw is then drawn as usual (its
  vertex bindings are made visible to compute and vertex), unless `RasterizedStream` is
  `D3D12_SO_NO_RASTERIZED_STREAM`. Stream output statistics queries of stream 0 add the helper's
  primitives written and needed after each such draw (read back, `ReadBackWait`); other streams
  stay zero (the helper writes stream 0 only). Refused by name: buffers whose start the back end
  cannot bind as storage (WebGPU: multiples of 256 bytes), other rasterized streams, and what
  the helper refuses (streams other than 0, system values other than the position, adjacency).
- Hull and domain shaders (run 24, `src/device/geometry.rs`): a pipeline with both (and the
  patch topology type; one without the other, or patches without them, is E_INVALIDARG)
  converts the vertex shader with every input pulled, the hull shader plain and the domain
  shader with the pixel stage's interpolation, and makes `Stages::tessellated`; its program is
  the domain program with the pixel program. Each draw runs the vertex and hull stages and the
  tessellator (`TessellationAsCompute`) with the vertex bindings (visibility vertex) and the
  hull's (visibility hull, its helper block in set 3), then draws in the pass with the domain
  and pixel stages' bindings: one indexed indirect draw of the tessellator's indices, the
  helper set holding the domain block, the pixel block and the helper's patches and points
  (`DOMAIN_PATCHES_BINDING`, `DOMAIN_POINTS_BINDING`). `IASetPrimitiveTopology` takes patch lists;
  without a hull shader they are E_NOTIMPL at the draw, as adjacency is. The geometry stage's
  helper block is now bound the same way (before, none was). `ExecuteIndirect` draws of any
  pipeline through the stage helper read their arguments back (`IndirectReadBack`; the helper
  takes direct draws only), which made vertex_id pass. Pipelines' `Stages` are released with
  them (`Stages::destroy`). Refused by name: geometry shaders and stream output after a domain
  shader (the helper has no constructor for them yet, G6), and what the helper refuses.
- Helper blocks of pixel and compute stages (run 19, `src/device/pull.rs`): while the binding
  sets are made, each binding notes the helper values it gives (`Device::bound_helpers`, by API
  stage); the vertex block, the pixel block and the merged samplers go into one set at
  `HELPER_SET`, a compute program's block into its own. Each value is written at its own length
  (up to nine words, `SamplerState`).
- Typed buffers (run 19, `src/device/bind.rs`, `src/dxbc.rs`): on a back end without
  `TEXEL_BUFFERS` a DXBC shader's typed buffers (`dcl_resource` of a buffer, `dcl_uav_typed` of
  a buffer) are converted with `TexelBufferRoute::Storage`, first as 32-bit single-channel
  formats of their declared kind. A view binds its buffer as storage from its first element
  rounded down to the storage alignment, with the rest and the element count in
  `TexelBufferView`; its format is noted, and for other formats the program is converted again
  (`formats.rs`). Null views bind zeros with a count of zero. `CheckFeatureSupport` answers
  `BUFFER` and `SHADER_LOAD` for formats with a vertex fetch form (`features::typed_buffer`).
  Refused by name: typed views of other formats in a vertex shader (its program is made once),
  and what the converter refuses (geometry shaders' typed buffers, DXIL typed buffers).
- Sampling in the program (run 19, `formats.rs`, `state.rs`): a sampler with a level-of-detail
  bias, or with border or mirror-once addressing where the back end lacks `BORDER_COLOR` or
  `MIRROR_ONCE` (WebGPU lacks both), is made clamping and without the bias, and the pixel or
  compute program is converted with `lod_bias` and `border_in_shader` for its register
  (`LodBias`, `BorderColor`, `SamplerState`, `TextureComponents` written per draw). A shader
  resource view's `ResourceMinLODClamp` past its first level converts the program with
  `min_lod_clamped` (`MinLod`) and binds merged samplers at `CLAMPED_SAMPLER_BINDING`, each
  pair's sampler with its minimum level raised to the clamp. Programs are kept per pipeline and
  bound state, as typed view formats are. Refused by name: these in vertex and geometry shaders,
  sampler registers past 15 and texture registers past 127.
- Storage textures through buffers (run 19, `src/device/through.rs`): where the converter
  refuses atomics on storage textures, the pixel or compute shader is converted at pipeline
  creation with its typed texture views read through buffers (`texels_through_buffer`,
  `AtomicsThroughBuffer`); a view of a format the back end cannot store into goes the same way
  at the draw (`StorageFormatThroughBuffer`). The view's level and layers (or depth slices) are
  copied into a scratch buffer at rows of 256 bytes before the draw or dispatch, with the open
  pass ended, and copied back after it where the view is writable (the pass ended again); the
  layout goes in `TexelLayout`. Refused by name: these behind a geometry shader, in vertex
  shaders, multisampled, and while a counted occlusion or statistics query runs.
- Filtering in the program (run 20, `formats.rs`): a shader resource view whose texture the back
  end cannot filter (a depth texture read as its depth, or a float format without `FILTER`) is
  noted at binding; where the pixel or compute program samples it through a non-comparison
  sampler filtering linearly, the program is converted with `filter_in_shader` for that texture
  and kept per pipeline and bound state, and the sampler's `SamplerState` and `BorderColor` are
  written to its helper block (`FilterInShader`). Before, such a draw failed WebGPU's validation
  (a filtering sampler in a non-filtering slot). Own case `filter-depth`.
- Refused with E_NOTIMPL and the reason printed (`wcontainers: <reason>: not implemented`):
  multisampled texture arrays (run 20; at creation: the interface's multisampled textures have
  one layer, and before, the texture was made invalid on wgpu and the call answered S_OK), arrays
  of resources, component mappings other than identity, min/max filters, alpha-antialiased lines,
  depth bias on lines and points, forced sample counts, conservative rasterisation, raw and
  structured views at offsets the back end cannot bind, unordered access buffers in custom
  heaps the CPU sees,
  timestamps where the back end lacks `TIMESTAMPS` (`GetTimestampFrequency` and
  `GetClockCalibration` then answer E_FAIL as for a queue without them), stream output from
  streams other than 0 and drawing a stream other than 0, rectangles in UAV clears of a 3D texture's view in another
  format than its own, an occlusion query across passes, video command lists, pipeline libraries (`DXGI_ERROR_UNSUPPORTED`), protected sessions, geometry shaders and stream output after a domain shader, state objects (ray
  tracing), depth bounds, programmable sample positions, view instancing, sampler feedback.
- Newer interfaces (run 10): the device is `ID3D12Device8`, command lists
  `ID3D12GraphicsCommandList2` (since run 15 `ID3D12GraphicsCommandList4`), resources
  `ID3D12Resource2`; `ID3D12Device9` and later and `ID3D12GraphicsCommandList5` and later are not
  offered (E_NOINTERFACE). Render passes (run 15, `objects.rs`): `BeginRenderPass` records
  the targets and the clears of `CLEAR` beginning accesses; `EndRenderPass` records the resolves
  of `RESOLVE` ending accesses (whole subresources, averaged; part of one, other modes and
  depth-stencil resolves fail `Close` with E_NOTIMPL); discard and preserve accesses keep the
  contents, which Direct3D 12 allows. Suspending and resuming flags change nothing.
  `SetProtectedResourceSession` takes only null. The ray tracing and meta command methods
  fail `Close` (absent in `CheckFeatureSupport`).
- Pulled vertex inputs (run 15, `src/device/pull.rs`): at each draw, a stream whose bound
  stride WebGPU refuses, or that steps every n instances without `INSTANCE_STEP_RATE`, is left
  out of the vertex layout and its inputs are fetched by a variant of the vertex program
  (`VariantKey::pulled`, made at the stream's first such draw and kept per pipeline, with the
  arguments route's base source where the draw is indirect). Each input's stream is bound as
  read-only storage from its view's offset rounded down to the storage alignment, as long as
  the view rounded up to a word, and the helper block gives the rest of the offset, the stride
  and the step. Indirect draws of streams stepping other than once an instance fetched so
  read their arguments back (the first instance places them). Counted draws of such streams are
  refused by name. Pipeline state streams
  are read in `examples/directx/stream.rs` (absent subobjects take Direct3D 12's documented
  defaults). abi.rs patches the newest interfaces' vtables, base entries included; `objects::ours`
  reaches our resource behind any `ID3D12Resource`.
- The Windows library (`examples/directx`, `d3d12.dll`, with `dxgi.dll` forwarding to it): as in run
  4, plus (run 5) `D3D12CreateRootSignatureDeserializer` and `D3D12CreateVersionedRootSignatureDeserializer`,
  the system library's export ordinals (101 for `D3D12CreateDevice`, and so on), one device per
  process while it lives (as Direct3D 12 hands out one per adapter), every Direct3D 12 object's
  private data, name and device (`base.rs`; children hold a reference to the device), structure
  returns as C callers read them (`abi.rs`, below), depth-stencil, constant buffer, shader resource,
  unordered access and sampler descriptors, descriptor copies, the root argument calls,
  `GetHeapProperties`, `CopyResource`, `GetParent` of adapters, outputs and swap chains,
  `GetContainingOutput`, swap chains refused on the desktop window (`E_ACCESSDENIED`) and on
  queues other than direct ones (`DXGI_ERROR_INVALID_CALL`).
- Structure returns (`abi.rs`): COM methods returning a structure (`GetDesc`, `GetAdapterLuid`,
  `GetCPUDescriptorHandleForHeapStart` and the like) take a pointer for it and return that pointer,
  which C callers dereference; the windows crate's shims write the structure and return nothing.
  Each such object gets a copy of its vtable with those entries replaced by shims that also return
  the pointer. Without this every C program read garbage (Wine's tests crashed on it).

## CheckFeatureSupport describes the planned front

The answers (plan section 9) are honest about the back end but run ahead of the front as it is
today: feature level 11_0, resource binding tier 1, resource heap tier 2, depth formats with
`DEPTH_STENCIL`, `SHADER_SAMPLE`/`SHADER_LOAD` and the rest describe the front this milestone builds.
Today the refusals listed above answer E_NOTIMPL when used, so a program trusting the answers can
fail at those calls.
Nothing answers success without the work.

## The pass bar's cases against real Direct3D 12 (own program)

`check_d3d12 cases` runs each case in its own process with real Direct3D 12 and through ours and
compares the lines (`examples/check/cases.rs`). Run 20: 24 of 24 equal; runs 18 and 19: 23 of
23 (Measurements). Run 17 (RTX 3080, desktop shared with other
tracks, scratch run17/check1.log, built at d9a661c's sources): 22 of 22 equal; draw 0 of 4096
pixels beyond 1; features 269 of 872, outputs 22 of 110 and answers 7 of 37 differ, none beyond
real Direct3D 12 (ours answers less: heap flags, colour spaces, formats WebGPU lacks).

The cases and what each covers:
- alias-copy, alias-storage, alias-cpu: placed buffers overlapping in one heap, written by a copy,
  a root UAV and the CPU through an upload heap, read through the other. alias-texture: identical
  render targets at one offset.
- queues: copy and compute queues waiting on each other and on a CPU signal made after
  everything is submitted.
- predication: binary occlusion resolved into a default buffer, read back, four predicated draws
  and two predicated clears. predicated-resolve: a multisampled clear resolved under a predicate
  that skips and one that lets it through, and a same-state transition refused at `Close`.
- occlusion-count: five counted queries (one draw, two draws around a rectangle clear, none, a
  degenerate triangle, two instances), resolved in a second list, two used as predicates.
- statistics: pipeline statistics around nothing, draws, an indexed draw, a dispatch, an
  indirect draw and a skipped predicated draw; a stream output statistics query; counts left to
  the implementation printed as bounds.
- indirect-draw, indirect-count, indirect-dispatch (counts 0 and 1), indirect-short (refused at
  `Close`), indirect-vertex-buffer (read-back route), indirect-vertex-id (`SV_VertexID` with base
  vertex 3, first vertex 3, first instance 1, a count buffer; the arguments route).
- depth-planes: footprints and plane copies of D24S8, D32S8, D32, D16.
- vertex-past-end, vertex-pulled, restart: zero tails past a view's end, streams WebGPU cannot
  fetch, strip cut values with 16-bit and 32-bit indices, direct and indirect.
- render-pass: clearing, keeping and resolving passes. custom-heap: CPU-visible custom heap
  buffers written and read both ways, two overlapping.
- geometry (run 17): five geometry shaders (see "Where things stand"), compared pixel by pixel
  (a hash of the picture) and by colour counts.
- timestamps (run 18): timestamps around a dispatch on the direct queue and around a copy on a
  copy queue timestamp heap, both queues' frequencies, and two clock calibrations 20 ms apart
  around the work; printed as relations (in order, inside the calibrations, the GPU and CPU
  clocks agreeing within 10 %), since the values differ between runs. Since run 20 the direct
  queue's timestamps are also resolved into a default-heap buffer and copied to a readback one
  (equal to those resolved straight into readback).
- filter-depth (run 20): a 4 by 4 `R32_TYPELESS` depth texture of three depths (cleared by
  rectangles) sampled as `R32_FLOAT` through a linear static sampler over the 64 by 64 target,
  each pixel checked against bilinear filtering worked out on the CPU (within 2 steps of 8 bits)
  and the picture having more than 16 shades. Real Direct3D 12 filters in hardware, ours in the
  program.
- History: run 7 11 of 13, runs 10 and 11 12 of 13, run 12 13 of 14, every case equal since run
  13 (15, then 16, 17, 20, 21, 22, 23).

## Test files (Wine's and vkd3d's, through our libraries, RTX 3080)

How a file counts: the rig compares each test's verdict with real Direct3D 12's on this GPU
(`wine-tests run`, below); a test that skips what real Direct3D 12 runs, or runs fewer checks,
fails. Since the rig's run of 2026-11-17 19:11 (its commit 4ffedc6), tests that fail some
checks on real Direct3D 12 are counted too, held to every other check, so the counted set is
198 (it was 193).

Run 24 (2026-11-18, desktop shared with other tracks; scratch run24/full1.log, built at
78b8525, the tessellation commit): 162 of 198. d3d12 (Wine) 10 of 10, vkd3d-d3d12 140 of 176,
vkd3d-d3d12_invalid_usage 3 of 3, vkd3d-hlsl_d3d12 9 of 9. Newly passing (5):
hull_shader_fork_phase, nop_tessellation_shaders, tessellation_dcl_index_range,
tessellation_primitive_id (hull and domain shaders through the stage helper), vertex_id (a
vertex shader without a position, now converted, streamed out by `ExecuteIndirect` draws read
back). No test lost. The other 9 tessellation files stop at what the helper or converter lacks
(below).

Run 23 (2026-11-18, desktop shared with other tracks; scratch run23/full1.log, built at e148d28,
the slots commit, with the two new exports uncommitted then, and full2.log at e2d1b4e, after
the occlusion commits): 157 of 198 both times, the same 41 failing files as runs 20 to 22. After the occlusion commits (run23/o1-wine.log): the query files
test_query_occlusion, test_query_pipeline_statistics, test_resolve_non_issued_query_data,
test_resolve_query_data_in_different_command_list,
test_resolve_query_data_in_reordered_command_list and test_create_query_heap pass, as before.

Run 22 (2026-11-18, desktop shared with other tracks; scratch run22/full1.log after the merges
and the DXIL typed views commit, run22/s3-full.log after the statistics commit, efc9461): 157 of
198 both times, the same 41 failing files as runs 20 and 21 (listed below).

Run 20 (2026-11-17, desktop shared with other tracks; scratch run20/full3.log, built at
c682b6b with the multisampled array refusal of 04f7d23, the rig's comparison patched locally as
"How to run" says): 157 of 198. d3d12 (Wine) 10 of 10, vkd3d-d3d12 135 of 176,
vkd3d-d3d12_invalid_usage 3 of 3, vkd3d-hlsl_d3d12 9 of 9. Newly passing (2): depth_load and
typed_uav_store (the shaders branch now declares naga's 16-bit normalised storage capability).
No test lost. clear_render_target_view now stops at the multisampled array's creation (E_NOTIMPL)
instead of failing 3 checks on an invalid texture. Run 20's first full run (run20/full1.log) ran
only 33 tests: the rig's comparison stopped at the first failing one.

Run 19 (2026-11-17, desktop shared with other tracks; scratch run19/full1.log, built at
f74262c, the storage textures through buffers commit): 155 of 198. d3d12 (Wine) 10 of 10,
vkd3d-d3d12 133 of 176, vkd3d-d3d12_invalid_usage 3 of 3, vkd3d-hlsl_d3d12 9 of 9. Newly
passing (10): with typed buffers bufinfo_instruction, copy_descriptors, cs_constant_buffer,
typed_buffer_uav, update_compute_descriptor_tables; with the WebGPU back end's converted
formats render_a8 and clear_unordered_access_view_image; with sampling in the program
sample_instructions; with storage textures through buffers coverage and draw_uav_only. No test
lost. Run 18: 145 of 198 (scratch run18/full1.log).

Files passing, by test file: d3d12 (Wine) all 10; vkd3d-d3d12_invalid_usage all 3;
vkd3d-hlsl_d3d12 all 9; vkd3d-d3d12 all but the 36 below. The failing ones, each by what it
stops at first:
- The converter (G6), 13: `StorageOffset` (buffer_srv: a structured view at byte 240; its
  typed views now pass), `StructType` (tgsm: structured group shared memory),
  `[earlydepthstencil]` left out of DXBC programs (`lower.rs` sets no `early_depth_test`), so
  pixels the depth test rejects still run the shader's atomics (early_depth_stencil_tests, 1
  check), `AtomicSignedness` (atomic_instructions), `RegisterSpace` 2 (register_space,
  sampler_register_space), `DynamicOffset` 2 (gather, gather_c), `CullDistance`
  (combined_clip_and_cull_distances), `MultisampledArrayTexture` (multisample_array_texture),
  `RenderTargetArrayIndex` (ps_layer), `InterpolateAt` (shader_eval_attribute), resource arrays
  (resource_arrays).
- The stage helper's other parts (G6), 10: geometry shaders and stream output after a domain
  shader (domain_shader_inputs, domain_shader_one_patch_constant_input,
  hull_shader_patch_constant_inputs, hull_shader_relative_addressing, line_tessellation,
  quad_tessellation), patches of another control point count than the hull shader declares,
  drawn by real Direct3D 12 and not by the helper (hull_shader_control_point_phase),
  `VertexOutputShape` (clip_distance), `StructType` in a hull shader (hull_shader_punned_array),
  a geometry shader writing `SV_RenderTargetArrayIndex` (layered_rendering).
- A view swizzle (interface and G6), 3: depth_stencil_sampling (stencil views),
  srv_component_mapping, stencil_load (at its stencil view).
- Multisampled texture arrays (absent in WebGPU, refused at creation since run 20), 1:
  clear_render_target_view (it then uses the null texture and crashes).
- A listed loss, 1: execute_indirect (vertices past a vertex buffer's end in an indirect draw).
- Reported absent as Direct3D 12 expects, the test then running fewer checks or stopping, 7:
  create_reserved_resource, get_resource_tiling (tiled resources),
  create_graphics_pipeline_state and output_merger_logic_op (logic operations; the first
  stops where real Direct3D 12 makes a pipeline with one and then releases the null pipeline),
  shader_instructions (float64), unbounded_resource_arrays, unbounded_samplers (binding tier 2).
- Real Direct3D 12 crashes, 1: copy_block_compressed_texture. Creating a block-compressed texture
  of part of a block fails on both (this Windows 10 runtime refuses it); the test then copies
  from the null texture: real Direct3D 12 crashes there, ours fails the list's `Close` and the
  test goes on to fail later checks before crashing. Not to be matched.

History (tests passing on real Direct3D 12 that pass through ours, vkd3d-d3d12 of 172 to 174):
run 5 54, 6 66, 7 80, 8 87, 9 74 (the rig's stricter rule began), 10 87, 11 94, 12 98, 13 103,
14 104, 15 106, 16 106; run 17 108 of 176 under the newer rule, run 18 123 of 176, run 19 133,
run 20 135, runs 21 to 23 135, run 24 140. Whole counts: run 12 116 of 193, 13 121, 14 122, 15
124, 16 124; run 17 129 of 198, run 18 145, run 19 155, run 20 157, runs 21 to 23 157, run 24
162.

## Shader runner (vkd3d-shader_runner, 302 files)

Run with our d3d12.dll alone (the system's dxgi.dll): with our dxgi.dll too every file crashes
in its Direct3D 11 part before reaching Direct3D 12 (our dxgi.dll serves only Direct3D 12), 0 of
299 (run12/runner1.log). Command: `E='%CARGO_TARGET_DIR%\debug\examples';
tools/graphics-rig/wine-tests run --dll "wcontainers_d3d12.dll=$E\wcontainers_d3d12.dll" --dll
"d3d12.dll=$E\d3d12.dll" vkd3d-shader_runner`.

Run 24 (run24/runner1.log, built at 78b8525, the tessellation commit, 234 s): 204 of 302, the
same 98 failing files. The two tessellation files now get past their shader model 5 pipelines:
hlsl tessellation-patch-vars passes its 5.0 section and stops at 6.0 (a DXIL hull shader,
"spirv-reader: unsupported execution model", G6); hlsl tessellation stops at the converter's
`PackedStageValues` (pixel shaders) and `StructType` (hull shaders), G6.

Run 23 (run23/runner1.log, built at e148d28, the slots commit, 234 s; runner2.log at
e2d1b4e, after the occlusion commits, 244 s): 204 of 302 both times, the same 98 failing
files. sm6-uav-rwtexture now binds its one view (u1) at slot 0, the converter's new
numbering, and still fails as in run 22: the draw's bind group is made for a program whose
storage texture is `R32Float` while the bound view is `R32G32B32A32_FLOAT` (WebGPU validation
error; nothing written). With a temporary trace (not committed, run23/d1-sm6.log): the front
asks for the program variant with slot 0 as `Rgba32Float` (made with `R32Float`, bound
`Rgba32Float`), the variant converts with no refusal, and the bind group is still checked
against an `R32Float` layout. The file's second pipeline (u0 `R32_FLOAT`, u1
`R32G32B32A32_FLOAT` written whole) takes the same route and passes. Its first shader writes
`.xy` and then `.zw` of one texel, so the texel is read as well. Likely cause, read in the
shaders crate and not confirmed by a run: dxil-spirv declares a storage image the program
reads with a 32-bit format of its own (`R32f`), and `src/dxil/images.rs` `prepare` rewrites
only storage images declared with no format, so the variant's `storage_formats` never reach
that image (G6). A read and written `R32G32B32A32_FLOAT` view must then go through a buffer
there, as on the DXBC route.

Run 22 (run22/runner1.log, built at 81ca7a1, the DXIL typed views commit, 279 s): 204 of 302,
the same 98 failing files, but 8 get further with DXIL storage images and typed buffers: hlsl
compute, descriptors, discard, numthreads, uav-load and uav-out-param now fail only by running
fewer checks than real (shader model 6.0 alone, plan section 9); barriers and uav-rwtexture
stop later, at other DXIL refusals. sm6-uav-rwtexture was refused at its first pipeline and now
draws, but its first probe reads the texel unchanged (`RWTexture2D<float4>` u1 of
`R32G32B32A32_FLOAT`, written by two partial stores, `.xy` then `.zw`): no refusal was printed
and nothing was written; not looked into yet (Next). A read and written storage texture of a
format other than the three 32-bit single-channel ones can only go through a buffer on WebGPU,
which the converter does not do for DXIL yet.

Run 20 (run20/runner3.log, built as the run's last full run, 231 s): 204 of 302, the same 98
failing files. Nine compute files (barriers, compute, numthreads, tgsm, uav-atomics, uav-counter,
uav-load, uav-out-param, uav-rwstructuredbuffer) had removed the device at their first dispatch
("no such root signature": the runner releases the root signature after recording, before
the list runs); they now pass their shader model 4.0 to 5.1 sections and stop at 6.0 (DXIL
compute shaders the converter refuses, G6), except tgsm (`TypedBufferFormatKind`, G6) and
uav-atomics, which binds a typed buffer view where its shader declares `RWTexture2D<int>`
(undefined in Direct3D 12; the vkd3d file itself says the type does not match; ours removes the
device, "not a texture"). length-property binds a structured view where its shader declares a
typed `RWBuffer` (undefined too; ours removes the device). hlsl sampler-offset and sample-bias
run fewer checks than real only because real runs shader models 6.0 to 6.5 and ours 6.0 (real's
log: "Compiling SM6.0-SM6.5 shaders"; 30 checks is 5 models of sampler-offset's 6), as plan
section 9's answer says. sample-minmax stops at minimum and maximum filters (absent in WebGPU;
the runner assumes them on every Direct3D 12 device, and Direct3D 12 has no query for them, see
"Gap list").

Run 19 (run19/runner1.log, built at f74262c, 252 s): 204 of 302, the same 98 failing files as
run 18. After the clamp commit (1cb2548, run by hand, not the whole set): hlsl sampler runs
3681 checks and fails 42, at the converter's `SampleClamp` (the `Sample` overload taking a
clamp) and, in its shader model 6.0 sections, at DXIL programs whose array layer is not
rounded and which do not apply `min_lod_clamped` (G6); its 4.0 and 5.0 clamp sections pass.
hlsl sample-bias and sampler-offset run fewer checks than real (1136 of 1244, 388 of 418),
not looked into; hlsl sampler-state and combined-samplers pass.

Run 17 (run17/runner1.log, built at d9a661c's sources, 248 s): 204 of 302. The rig now counts
three files that fail some checks on real Direct3D 12 (hlsl mesh, uav-rwbuffer, uav-rwtexture),
which fail here too; the other 95 failing files are run 15's. hlsl geometry now passes its
shader model 4.0 to 5.1 sections through the stage helper and fails only its 6.0 ones (DXIL
geometry shaders, which naga's SPIR-V reader refuses, G6); hlsl primitive-id stops at the
converter's `PrimitiveId` (a pixel shader reading `SV_PrimitiveID` with no geometry shader, G6).

Run 15 (run15/runner1.log, built at d7c1de0): 204 of 299, the same 95 failing files as run 14.

Run 14 (run14/runner2.log, built at 9937b23): 204 of 299. The ten newly passing files
(d3dbc modifiers and sub; hlsl bool-semantics, depth-out, f32tof16, get-sample-pos,
is-front-face, nointerpolation, pixel-position, struct-semantics) failed only because the
runner, matching our device to the system's adapters by LUID, found no vendor and dropped its
"nvidia" tag (run14/runner1.log, before that fix: 194). The 95 failing, by first cause:
- 37 run fewer checks than real with no failure: the runner compiles shader model 6.0 only
  (real: 6.0 to 6.6), since `CheckFeatureSupport(SHADER_MODEL)` answers 6.0 as plan section 9
  says; files requiring 6.2 and later (16-bit types, int64 and the like) skip those sections.
- The converter (G6), 46: refused by name (`StorageImage` 8, `TypedBuffer` 6,
  `PackedStageValues` 4, `StructType`, `NonFiniteConstant`, `ConstantComponentIndexing` 2 each,
  `StorageCounter`, `SparseFeedback`, `InterpolateAt`, `DynamicOffset`, `CullDistance`,
  `ClipDistance`, `CalculateLevelOfDetail` 1 each, int64 in naga 1), "a shader feature the
  converter lacks" 9, "a shader the converter got wrong" 5, state creation E_NOTIMPL 4.
- Device removed after a refusal, 5.
- Wrong values, 3: arithmetic-float-uniform (min16float constants read as 32-bit, converter),
  frexp (converter), texture-load (an SM 4.0 `Texture2DMS` load in an unrolled loop reads at
  `pos.yx` instead of `pos.xy`, converter; and unorm and float views read through a
  `Texture2D<uint4>`, undefined in Direct3D, where real converts and ours gives the raw bits,
  then the device is removed). Not looked into further: G6's to check.

Run 13 (built at the depth twin commit; run13/runner1.log): 194 of 299 again, the same 105
failing files; 242 of 302 report no failed check (239 in run 12). sample-cmp no longer stops at
its comparisons: it runs 747 checks against real Direct3D 12's 784, all passing; the remaining
differences are as below.

Run 12 (built after the mesh subobject fix; run12/runner2.log, logs in run12/logs-runner2,
first refusal per file in run12/runner2-reasons.txt): 194 of 299 files that pass on real
Direct3D 12 pass through ours (by the rig's rule; 239 of 302 report no failed check). Before
the fix every pipeline failed: the runner's pipeline state streams carry empty amplification
and mesh shader subobjects, which the stream reader refused. The 105 failing:
- The converter (G6), 58: `StorageImage` 8, `TypedBuffer` 6, `PackedStageValues` 3,
  `StructType` 2, `NonFiniteConstant` 2, `ConstantComponentIndexing` 2, `StorageCounter` 1,
  naga's SPIR-V reader or validator 16 (buffer images, 64-bit and 16-bit types, structs as stage
  values), the stage helper 4, device removed after a refusal 7, others.
- Fewer checks than real, 43: the runner skips sections needing what `CheckFeatureSupport`
  reports absent (float64, int64, 16-bit types, wave operations, depth bounds, barycentrics,
  `SV_RenderTargetArrayIndex`/`SV_ViewportArrayIndex` from the vertex stage, and smaller gaps
  of 1 to 100 checks not sorted yet).
- With the system's dxgi.dll the runner finds no vendor for our device (its LUID is ours, not
  the system adapter's), so it drops its "nvidia" tag; some of the small check-count gaps may
  come from that and not from us.
- Wrong values, 4: arithmetic-float-uniform (4 checks), arithmetic-float (2), frexp (2) not
  looked into yet; sample-cmp (20) compared an `R32_TYPELESS` texture stored as `r32float`, which
  WebGPU's bind group refused, so the draws were dropped silently. Such comparison sampling is
  now refused by name ("comparison sampling of a texture not stored in a depth format"), so the
  file stops there (crash after the device is removed).

## Real program

Run 23 (2026-11-18, desktop shared with other tracks, not `--alone`, debug build at e2d1b4e's
sources; scratch run23/o1-bgfx26.log, o1-godot.log, godot1-output.log):
- 26-occlusion with counts summed on the GPU: every frame of both runs through ours passes
  (1058 to 1199 pixels beyond 8, all in the overlay's text, allowed 20480 to 22016); 120
  frames in 2.8 and 2.7 s (real 2.6 and 2.5 s in the same job; run 22: 55.4 and 47.3 s).
- Godot's 2D platformer: its crash at start was the rig's forwarding library aborting
  ("the system library lacks this export") when Godot called `D3D12GetInterface`, which ours
  did not export (the forwarder mirrors the system library's exports and sends each to the
  library behind it). Ours now exports it (no debug layer, tools, extended data, SDK
  configuration or device factory: E_NOINTERFACE) and `D3D12EnableExperimentalFeatures`
  (none: E_NOINTERFACE, S_OK for none asked), the two documented exports Windows 10's
  d3d12.dll has that ours lacked (dumpbin of the system library). Godot then runs 120 frames
  in 10.5 s and saves 3, all differing from the reference (409,222 of 518,400 pixels beyond 8
  at frame 10). Refused, each G6's: 44 compute shaders `ResourceArray`, 27 `TypedBuffer` (the
  route left native: the front found no typed view in those shaders' resource lists, being
  looked into), 20 `NonFiniteConstant`, 5 "unsupported storage class" (atomics on typed
  buffers or images), 4 graphics pipelines; 100 compute pipelines answered E_NOTIMPL; 6
  textures E_INVALIDARG (`ALLOW_SHADER_ATOMICS`, as this Windows 10 runtime). Godot then draws
  with what failed and its own errors follow (draw lists, framebuffers). `BindingsBeyondLimit`
  no longer appears. A second run with a temporary dump (not committed; run23/d1-godot.log,
  the 69 refused shaders' bytecode in scratch run23/dump, Godot's own shaders, compiled at run
  time by its NIR to DXIL): the shaders refused `TypedBuffer` declare no typed buffer at all
  (their PSV0 lists only raw buffers, kind 11, beside typed textures; tools23/psv.py prints
  it), so the converter itself makes a typed buffer of a raw one there (G6). The
  `ResourceArray` ones bind ranges of 8 typed textures (G6's resource arrays).

Run 22 (2026-11-18, desktop shared with other tracks, not `--alone`, debug build at efc9461's
sources; scratch run22/s3-bgfx-*.log and bgfx-24-nbody-1.log; bgfx-compare.ps1 with the
ignored rectangle 0,208,320,64, as run 21):
- 35-dynamic: every frame of both runs through ours passes (1132 to 1217 pixels beyond 8, all
  in the overlay's text, allowed 20480). 120 frames in 3.3 s and 3.3 s (run 21: 72 and 75 s;
  real 2.5 and 2.6 s): the pixel shader invocation counts no longer wait.
- 26-occlusion: every frame of both runs passes. 55.4 and 47.3 s (run 21: 114 s and past the
  120 s limit; real 2.5 s). What is left is its counted occlusion queries: the interface's
  occlusion counter reads its count back and waits at each pass end with counted draws (Next).
- 24-nbody (after the DXIL typed views commit, before the statistics one): runs its 120 frames
  through ours (2.8 and 2.6 s; real 2.5 s) where run 21 stopped at its first compute pipeline.
  Not compared: real Direct3D 12 itself draws it differently on every run (80, 292 and 667 of
  3600 tiles changed between the two real runs at frames 10, 60 and 120, up to 35557 pixels
  outside the ignored tiles), so the rig's rule compares no frame.
- Microsoft's samples through the rig (`programs run`, run22/programs2.log, job 01:07 to
  01:07:42): d3d12-hello-triangle and d3d12-hello-constbuffers (D3D12HelloTriangle and
  D3D12HelloConstBuffers, with the Agility SDK's D3D12Core.dll beside them, which ours does not
  load) run their 120 frames through ours (2.7 s each) and frames 10, 60 and 120 of both equal
  real Direct3D 12's references: 0 pixels beyond 8, largest difference 0. Command: `E='%CARGO_TARGET_DIR%\debug\examples';
  tools/graphics-rig/programs run --dll "d3d12.dll=$E\d3d12.dll" --dll
  "wcontainers_d3d12.dll=$E\wcontainers_d3d12.dll" --dll "dxgi.dll=$E\dxgi.dll" <programs>`
  (scratch bin/programs22.sh). The replacement d3d12.dll must be the forwarder, not
  wcontainers_d3d12.dll itself: two copies of the library under two names are two instances,
  and every program then failed (run22/programs1.log).
- The same run: bgfx-01-cubes-d3d12 and bgfx-38-bloom-d3d12 ran their 120 frames (2.8 and 2.9
  s) but were not compared: the rig's references on the desktop are not the ones its
  frames.sha256 names (the rig's to record again). godot-platformer-2d-d3d12 now fast-fails
  (0xc0000409) 1.4 s after start, before Godot prints its renderer line, with our dxgi.dll and
  with the system's (run22/godot2.log); run 20 ran 120 frames with ours as d3d12.dll and no
  forwarding library in front. A Rust panic in either library (panic=abort shows as
  0xc0000409) is the first thing to look for (Next).
- 37-gpudrivenrendering: stops at its first compute pipeline (naga's SPIR-V reader:
  "unsupported storage class %11", atomics on typed buffers or images, `OpImageTexelPointer`,
  G6's next item); bgfx then ends itself (exit 3).

Run 21 (2026-11-18; scratch run21/bgfx5.log; bgfx built by the rig at 18:11 on 2026-11-17):
bgfx's Direct3D 12 examples behind the rig's forwarding library (frame clock, frames 10, 60 and
120 saved from the swap chain's queue), recorded twice on real Direct3D 12 with the rig's
`gt-run frames --record` (tolerances from the two runs) and run twice through ours, each frame
compared by `gt-run frames`. Pass, every frame of both runs: 01-cubes, 38-bloom, 26-occlusion
(one of its two runs through ours hit the 120 s limit before frame 120; its frames 10 and 60
passed), 35-dynamic. Between 720 and 1226 of 921,600 pixels differ by more than 8 in each, all
in the left overlay's text (largest difference 217), inside the allowance. The ignored rectangle
is 0,208,320,64: the overlay's lines of GPU time and GPU memory. The rig ignores 0,236,320,36
(the memory line) for Direct3D 11; on Direct3D 12 its library does not fix timestamps by the
frame clock, so the measured GPU time differs too (real "GPU 0.073", ours "GPU 4.904", frame
60), and ours reports no memory (`QueryVideoMemoryInfo` E_NOTIMPL: "8 B / 0 B" against real "57
MiB / 9 GiB"). With the rig's rectangle every 01-cubes and 38-bloom frame failed only on those
two lines (run21/bgfx3.log). Not compared: 24-nbody and 37-gpudrivenrendering stop at their
first compute pipeline (DXIL compute shaders with buffer images, naga's SPIR-V reader:
"unsupported image dimension %5", G6); bgfx then ends itself.
Speed (desktop shared, not `--alone`, debug build, whole runs of 120 frames with the frame
clock): real 2.4 to 2.7 s each; ours 01-cubes 2.6 s, 38-bloom 2.7 s, 35-dynamic 72 and 75 s,
26-occlusion 114 s and more than 120 s. bgfx wraps every command list in a pipeline statistics
query; ours counts its pixel shader invocations with the interface's occlusion counter, which
reads its count texture back and waits at every pass end with counted draws (expected to be
the cost, not measured; see "Next").
Commands: `scratch bin/bgfx-compare.ps1 <example> <forwarding dll> <gt-run.exe> [ignored
rectangle]` (copied to C:\wc\out\phase-2-graphics-direct3d-12, with `bgfx-capture.ps1`, which
it calls). Run as `D='C:\wc\out\phase-2-graphics-direct3d-12'; tools/graphics-rig/gpu-job
--desktop --build "set CARGO_INCREMENTAL=0&& cargo build -j 3 -p wcontainers-direct3d-12
--examples && cargo build -j 3 -p wcontainers-graphics-test-forward -p wcontainers-graphics-test"
-- "powershell -NoProfile -ExecutionPolicy Bypass -File $D\bgfx-compare.ps1 01-cubes
%CARGO_TARGET_DIR%\debug\wcontainers_graphics_test_forward.dll
%CARGO_TARGET_DIR%\debug\gt-run.exe 0,208,320,64"`. The rig's `programs` list does not yet run
bgfx on Direct3D 12 (rig track's).

Run 16: bgfx's examples on Direct3D 12 (`example-<name>Release.exe --d3d12`, built by the rig
in C:\wc\programs\src\bgfx; run by hand with scratch bin/bgfx-run.ps1, copied to
C:\wc\out\phase-2-graphics-direct3d-12, since the rig's `programs` list runs bgfx on Direct3D 11
only and has no frame capture for Direct3D 12). 01-cubes, 24-nbody and 38-bloom run on real
Direct3D 12. Through ours (scratch run16/bgfx5.log) each makes the device and the swap chain and
then stops: bgfx makes a timestamp query heap (`E_NOTIMPL`, timestamps absent on this back end),
ignores the failure and records `EndQuery` with no heap, so its first list fails `Close` and the
device is removed. Timestamps are G2's `TIMESTAMPS` (the WebGPU back end answers
`Absent(TIMESTAMPS)`); Direct3D 12 has no way to report them absent on a direct queue, and every
Direct3D 12 program tried (bgfx, Godot) uses them. Seen on the way: NVIDIA's NVAPI (bgfx's
Reflex setup) calls into our device as if it were the driver's (`CreateReservedResource`,
answered `E_NOTIMPL`); harmless, and absent in a container. Frames are not compared yet: that
needs the rig to capture Direct3D 12 frames (or bgfx's own screenshots) and a frame clock.

Run 18 (scratch run18/bgfx1.log to bgfx6.log; bgfx built by the rig at 18:11 that day): with
timestamps, bgfx got past its first list and then crashed in its own code. The cause: bgfx
calls `IDXGIAdapter3::QueryVideoMemoryInfo` on the adapter `EnumAdapters` gives without asking
for that interface (Windows 10's adapters are all DXGI 1.6 ones), and ours was an
`IDXGIAdapter1`, so the call went past the end of its vtable. Found by a temporary trace of
every call (not committed); NVAPI was ruled out first (`bgfx-run.ps1 <name> <seconds> ours
nonvapi` puts an empty nvapi64.dll beside the program so NVIDIA's library does not load, as in
a container). Adapters are now `IDXGIAdapter4` (`GetDesc2`, `GetDesc3`; video memory budgets and
reservations answer E_NOTIMPL, no browser reports memory). Now 01-cubes and 38-bloom run through
ours until stopped (10 s each, with and without NVAPI, run18/bgfx6.log), as on real Direct3D 12.
Their frames are not compared: the rig cannot capture a Direct3D 12 program's frames yet (its
status: a forwarding dxgi.dll with capture and a frame clock is its next step).

Godot, run 18 (run18/godot1.log to godot4.log): godot-platformer-2d-d3d12 now runs 120 frames
through ours and saves 3 (run 10 crashed after 1), but every frame differs from the reference
(1,535,974 of 1,536,000 pixels beyond 8 at frame 10): its compute shaders do not convert
(naga's SPIR-V reader: DXIL storage images without a format 132 times, buffer images 54, an
image load operand 3) and 8 graphics shaders hit `BindingsBeyondLimit`, all G6's. Its 6
committed textures with `ALLOW_SHADER_ATOMICS` fail with E_INVALIDARG through ours as on this
desktop's Windows 10 runtime (the Godot build carries no Agility SDK: its exe has no
`D3D12SDKVersion`).

Godot, run 20 (run20/godot1.log): as run 18: 120 frames, 3 saved, every frame differs
(1,535,974 of 1,536,000 pixels beyond 8 at frame 10). Refusals: 132 compute shaders with DXIL
storage images of no format and 55 with buffer images (naga's SPIR-V reader), 8 graphics shaders
`BindingsBeyondLimit`, all G6's; `QueryVideoMemoryInfo` 671 times (E_NOTIMPL, no browser reports
memory).

Godot:
Command: `tools/graphics-rig/programs run --dll "wcontainers_d3d12.dll=%CARGO_TARGET_DIR%\debug\examples\wcontainers_d3d12.dll" --dll "d3d12.dll=%CARGO_TARGET_DIR%\debug\examples\d3d12.dll"
--dll "dxgi.dll=%CARGO_TARGET_DIR%\debug\examples\dxgi.dll" godot-platformer-2d-d3d12`.
- Run 10 (build after a0760963): FAIL, exit 0xc0000005 after 7.1 s, 1 frame, 0 of 3 frames
  saved. Godot makes the device ("D3D12 11_0 - Forward+ - Using Device #0: NVIDIA - NVIDIA
  GeForce RTX 3080") and stops at the converter only (G6): its 2D canvas vertex shaders fail with
  `BindingsBeyondLimit` (7) and "unsupported image dimension %5" (buffer images, 6), so 13
  `CreateGraphicsPipelineState` calls answer E_NOTIMPL; compute shaders with storage images of no
  format fail ("image read/write without format is not currently supported", naga's SPIR-V
  reader); timestamps are absent (G2). Godot then draws with null pipelines and crashes in its
  own code. Run 9 had stopped earlier (a heap flag and D16 depth views, both fixed in run 10).
- Runs 11 to 16: not run again (run 18 above). The shaders branch then had none of those converter features
  (`StorageImage`, `BindingsBeyondLimit` and `DrawBaseFromArguments` are refused by name in
  crates/wcontainers-shaders/src/lower.rs and dxil.rs at origin/phase-2-graphics-shaders as of
  run 13), so the run would end as in run 10.

## Gap list (the pass bar's written list, as of run 24)

The final list may hold only gaps that are absent features: what the back end or browser lacks,
reported absent as Direct3D 12 expects. Run 24's failing test files, by that rule:

Absent features (final; reported absent through Direct3D 12's own mechanism, or refused by name
where Direct3D 12 has none):
- Tiled resources (`TiledResourcesTier` 0): create_reserved_resource, get_resource_tiling.
- Logic operations (`OutputMergerLogicOp` false): output_merger_logic_op, and
  create_graphics_pipeline_state, which makes a pipeline with one without asking and then uses
  the null pipeline.
- Float64 in shaders (`DoublePrecisionFloatShaderOps` false): shader_instructions.
- Resource binding tier 2 and descriptor arrays where the back end lacks big tables (tier 1
  answered): unbounded_resource_arrays, unbounded_samplers.
- Multisampled texture arrays (WebGPU's multisampled textures have one layer; Direct3D 12 has no
  capability for them, so creation answers E_NOTIMPL): clear_render_target_view.
- Minimum and maximum filters (WebGPU has no reduction samplers): the shader runner's
  sample-minmax. Direct3D 12 has no query for them: Microsoft's `D3D12_FILTER` page (read
  2026-11-17) sends the reader to Direct3D 11's `D3D11_FEATURE_DATA_D3D11_OPTIONS1::MinMaxFiltering`,
  so ours refuses such samplers by name at use (E_NOTIMPL, the device removed), the only report
  Direct3D 12 leaves.
- Shader models above 6.0 (plan section 9's answer): 37 shader runner files run fewer checks.
- Also reported absent with no failing test here: conservative rasterisation, raster order
  views, ray tracing, mesh shaders, variable rate shading, sampler feedback,
  `SV_RenderTargetArrayIndex` from the vertex stage, timestamps where the adapter lacks
  `timestamp-query` (present on the RTX 3080 natively), drawing into and storing into
  `B5G6R5`, `B5G5R5A1`, `B4G4R4A4`.

A listed loss (not an absent feature; kept on the list with its reason):
- execute_indirect, 2 checks: `ExecuteIndirect` draws reading vertices past a vertex buffer's
  end, which WebGPU skips whole where Direct3D 12 reads zeros. Pulled streams would make it
  exact only by fetching every stream of every indirect draw, slowing every indirect draw for a
  case only faulty programs reach.

Not to be matched:
- copy_block_compressed_texture: real Direct3D 12 crashes in it (its texture of part of a block
  is refused on both; the test then uses the null texture).

Not absent features, so not yet allowed on the list (others' work, then this front's routing):
- G6, the converter, 13 files: `StorageOffset` (buffer_srv), `StructType` (tgsm),
  `[earlydepthstencil]` in DXBC (early_depth_stencil_tests), `AtomicSignedness`
  (atomic_instructions), `RegisterSpace` (register_space, sampler_register_space),
  `DynamicOffset` (gather, gather_c), `CullDistance` (combined_clip_and_cull_distances),
  `MultisampledArrayTexture` (multisample_array_texture), `RenderTargetArrayIndex` (ps_layer),
  `InterpolateAt` (shader_eval_attribute), resource arrays (resource_arrays). vertex_id passes
  since run 24.
- G6, the stage helper after a domain stage and layer outputs, 10 files (run 24; hull and
  domain shaders are routed since then, and hull_shader_fork_phase, nop_tessellation_shaders,
  tessellation_dcl_index_range and tessellation_primitive_id pass): geometry shaders and stream
  output after a domain shader (domain_shader_inputs, domain_shader_one_patch_constant_input,
  hull_shader_patch_constant_inputs, hull_shader_relative_addressing, line_tessellation,
  quad_tessellation), patches of another control point count than the hull shader declares
  (hull_shader_control_point_phase), `VertexOutputShape` (clip_distance), `StructType` in a hull
  shader (hull_shader_punned_array), layer outputs (layered_rendering).
- G1 and G6, a view swizzle (wgpu 30 has none, so a program rewrite), 3 files:
  depth_stencil_sampling, srv_component_mapping, stencil_load. Direct3D's stencil views give
  the stencil in the second component, WebGPU's stencil-aspect views in the first.
- The shader runner's other failures are G6's (DXIL storage images, typed buffers and compute
  shaders in naga's SPIR-V reader, `PackedStageValues`, `NonFiniteConstant`,
  `ConstantComponentIndexing`, `SampleClamp`, wrong values in converted shaders), plus the two
  files binding views of another kind than declared (uav-atomics, length-property), undefined
  in Direct3D 12.
- The real program: done for bgfx 01-cubes, 38-bloom, 26-occlusion and 35-dynamic (runs 21 to
  23) and Microsoft's D3D12HelloTriangle and D3D12HelloConstBuffers (run 22). bgfx 24-nbody runs
  (run 22) but draws differently on every real run, so it has no frames to compare;
  37-gpudrivenrendering waits on G6 (DXIL atomics on typed buffers and images) and Godot, which
  runs since run 23, on G6 (raw buffers made typed, resource arrays, `NonFiniteConstant`,
  atomics).
- Front, refused by name with no failing test here: sampling in the program, typed views of other
  formats and the minimum level-of-detail clamp in vertex and geometry shaders, storage texture
  format variants with pulled vertex programs or geometry stages, stream output buffers not
  starting at a multiple of 256 bytes, unordered access buffers in CPU-visible custom heaps,
  comparison sampling of a texture whose bits match no depth format and of a multisampled one.
  `Stages` has no way to release its programs and buffers (asked of G6).

## Next, in this order

1. At the start of each run merge the shared line, webgpu, shaders and rig. Check whether the
   rig fixed its comparison (`tools/graphics-rig/desktop/wine-run.ps1`, "How to run"); until it
   does, apply the one-line patch uncommitted for the run and never commit that file.
2. hlsl sm6-uav-rwtexture (shader runner, "Shader runner" run 23): the variant with slot 0 as
   `Rgba32Float` is asked for and converts, yet the bind group is checked against an
   `R32Float` layout, so nothing is written and nothing refused. Likely the converter's (G6):
   a DXIL storage image the program reads keeps dxil-spirv's own `R32f` format. Ask G6 (its
   status) and rerun that one file after the fix (`wine-tests run --dll ...
   "vkd3d-shader_runner:hlsl+sm6-uav-rwtexture.shader_test"`, as bin/debug23.sh).
3. The real program: Godot now runs (run 23) and waits on G6: raw buffers the converter turns
   into typed buffers (`TypedBuffer` with no typed buffer in the shader; bytecode in scratch
   run23/dump), resource arrays, `NonFiniteConstant`, atomics on typed buffers and images
   (`OpImageTexelPointer`, also bgfx 37-gpudrivenrendering). After each shaders merge, run
   Godot (bin programs22.sh) and bgfx 37 (bgfx-compare.ps1) and read the refusals.
4. Tessellation, waiting on the stage helper (G6), then rerun the files: geometry shaders and
   stream output after a domain shader (6 files make such a pipeline, refused by name:
   domain_shader_inputs, domain_shader_one_patch_constant_input,
   hull_shader_patch_constant_inputs, hull_shader_relative_addressing, line_tessellation,
   quad_tessellation); `Stages::run` refusing (`TopologyMismatch`, so nothing is drawn) patches whose
   control point count differs from the hull shader's declared input count, which real
   Direct3D 12 draws (hull_shader_control_point_phase: input count 1, drawn with
   3-control-point patches); `VertexOutputShape` (clip_distance), `StructType`
   (hull_shader_punned_array). Front side then: a constructor taking a geometry shader or
   stream output after the domain stage goes where `geometry_stage` refuses them now. Layer
   outputs (layered_rendering) as before.
5. The converter items above as they land (`StorageOffset` would let `storage_range` bind any
   offset).
6. Turn this gap list into the final one as the G6 items land.
7. The view swizzle once the interface and converter have one.
8. Later: `ID3D12GraphicsCommandList5` and later, `ID3D12Fence1`; render pass resolves of part
   of a subresource and of depth-stencil targets; the built-in storage clear for UAV clears of
   formats that do not render (`builtin::storage_clear`); recording without allocating per
   command, the call buffer of plan 4.8; moving `examples/directx` into
   `crates/wcontainers-directx`.

## For the Direct3D 11 track: using the infrastructure crate

Depend on `wcontainers-directx-graphics-infrastructure` (path `../wcontainers-directx-graphics-infrastructure`).
A working example of all of it is this track's Windows library: `examples/directx/present.rs`
(outputs, swap chains, presenting) and `examples/directx/gpu.rs` (the window's surface).
- Adapter: `adapters(backend.caps())` gives the one `AdapterDesc` (`description_utf16()`,
  `vendor_id`, `device_id`, `luid` = `ADAPTER_LUID`, `software`). Memory sizes are zero: no browser
  reports them.
- Outputs: `OutputDesc` (name, desktop rectangle, refresh rate) comes from the host; natively,
  `present.rs`'s `outputs()` makes them from GDI. `modes(format)` and `closest_mode` answer the mode
  calls; `GetDesc1` takes `sdr::*` and `COLOR_SPACE_SRGB`.
- Colour spaces: `presents_in(DXGI_FORMAT, DXGI_COLOR_SPACE_TYPE)` is `CheckColorSpaceSupport`'s
  present bit and the check of `SetColorSpace1`.
- Formats: `dxgi_format(DXGI_FORMAT value) -> Option<(Format, typeless)>`.
- Swap chain: `SwapChain::new(&mut backend, window, &SwapChainDesc { width, height, format (DXGI
  value), buffers, model: Model::from_dxgi(SwapEffect)?, usage })`. `Err(SwapChainError::InvalidCall)`
  is `DXGI_ERROR_INVALID_CALL`. Fill a width or height of zero from the window's client area first.
  Natively, `window` is a number given to `WebGpu::add_window` with a `wgpu::Surface` made from the
  HWND (`gpu.rs`, `State::add_window`: the Win32 handle needs `hinstance`, falling back to the
  program's module, and a Windows display handle).
- Back buffers: `chain.buffer(i)` is a `B::Texture` (`chain.buffer_desc()` describes it); blit models
  give buffer 0 only. Draw into `chain.buffer(chain.current_index())`.
- `chain.present(&mut backend, sync_interval)`: end your open pass first. It records the copy,
  submits (your recorded work goes with it), shows the surface, rotates flip models, and waits while
  more than `max_latency()` frames are queued (`set_max_latency` for `SetMaximumFrameLatency`).
  `would_wait` answers `DXGI_PRESENT_DO_NOT_WAIT`.
- `chain.resize(&mut backend, buffers, width, height, format)` for `ResizeBuffers` (zeros keep the
  value); release your views of the old buffers first, and refuse while the program holds one.
  `chain.destroy(&mut backend)` at the end.
- On Windows, two things the real driver does (see Where things stand): answer sent messages while a
  swap chain call waits (`gpu::call_windowed`), and send the device thread's own `CreateDXGIFactory*`
  to the system's dxgi.dll. Without them making a swap chain hangs.
- Format queries: `format_support(caps, DXGI_FORMAT)` gives `support1` and `support2`; empty means
  the API's E_FAIL. `takes_samples(caps, format, count)`; `dxgi_vertex_format(DXGI_FORMAT)`.

## How to run

- Tests (one draws on the GPU): `tools/graphics-rig/gpu-job --build "cargo test -j 3 -p
  wcontainers-directx-graphics-infrastructure -p wcontainers-direct3d-12 --no-run" -- "cargo test
  -j 3 --no-fail-fast -p wcontainers-directx-graphics-infrastructure -p wcontainers-direct3d-12"`
  (40 tests: 9 infrastructure, 9 front, 11 device on the recorder, 1 on the GPU, 10 unit; run 24:
  all pass, run24/test1.log and test2.log). Put `set CARGO_INCREMENTAL=0&& ` before both commands (see the note further down).
  `WEBGPU_API=dx12|vulkan` picks wgpu's path for the GPU test.
- Lint: `tools/graphics-rig/remote "cargo clippy -j 3 --all-targets -p
  wcontainers-directx-graphics-infrastructure -p wcontainers-direct3d-12 -- -D warnings"`.
- The desktop needs the shaders crate's submodules once (the rig's `remote` sends none):
  `tools/graphics-rig/remote "git submodule update --init crates/wcontainers-shaders/dxbc-spirv
  crates/wcontainers-shaders/dxil-spirv && git -C crates/wcontainers-shaders/dxil-spirv submodule
  update --init third_party/spirv-headers"` (done in run 4; they stay checked out).
- The libraries (run 21): `wcontainers_d3d12.dll` holds everything; `d3d12.dll` and `dxgi.dll`
  forward to it by that name. All three go beside a program (every `--dll` list below names
  all three, the shader runner's two).
- Scenes: `E='%CARGO_TARGET_DIR%\debug\examples'; tools/graphics-rig/scenes --build "cargo build -j 3
  -p wcontainers-direct3d-12 --examples" --dll "wcontainers_d3d12.dll=$E\wcontainers_d3d12.dll" --dll
  "d3d12.dll=$E\d3d12.dll" --dll "dxgi.dll=$E\dxgi.dll"
  --api d3d12 triangle swapchain` (drop the names for every scene).
- Own program: `tools/graphics-rig/gpu-job --desktop --build "cargo build -j 3 -p
  wcontainers-direct3d-12 --examples" -- "crates\wcontainers-direct3d-12\examples\check_d3d12.cmd"`.
  It runs `draw`, `features`, `outputs`, `answers`, `cases` and `fence-wait 2000` beside real
  Direct3D 12 and beside our libraries (in C:\wc\out\phase-2-graphics-direct3d-12\check), then compares. `--desktop`
  matters: in session 0 real DXGI refuses outputs and swap chains
  (`DXGI_ERROR_NOT_CURRENTLY_AVAILABLE`).
- Wine's and vkd3d's tests through our libraries (build first; about 8 minutes):
  `tools/graphics-rig/remote "set CARGO_INCREMENTAL=0&& cargo build -j 3 -p wcontainers-direct3d-12 --examples"`, then
  `E='%CARGO_TARGET_DIR%\debug\examples'; tools/graphics-rig/wine-tests run --dll
  "wcontainers_d3d12.dll=$E\wcontainers_d3d12.dll" --dll "d3d12.dll=$E\d3d12.dll"
  --dll "dxgi.dll=$E\dxgi.dll" d3d12 vkd3d-d3d12 vkd3d-d3d12_invalid_usage vkd3d-hlsl_d3d12`
  (name tests as `vkd3d-d3d12:test_a/test_b`). Logs land in
  pyro-parked/wcontainers-scratch/graphics-rig/wine-logs; `grep -o 'wcontainers: [^,(]*'` over them
  counts the refusals. `vkd3d-shader_runner` (302 files) runs the same way.
- The own program now also runs `answers` (alpha modes, a 16-bit float swap chain's presents, a
  fence waited for with no event, committed resources with each heap flag), compared like features.
- `com.rs`: `python3 examples/directx/com.py > examples/directx/com.rs`, then `cargo fmt` (it finds
  the windows crate under `$CARGO_HOME`, default `~/.cargo`).
- On the Mac only `nix develop -c cargo fmt` (the Mac is overloaded).
- The rig's comparison (its commit fa1a165, merged in run 20) stops at the first test that
  fails a check real Direct3D passes ("wine-run: You cannot call a method on a null-valued
  expression"): `$allowed = if (...) { ... } else { @() }` gives `$null` for a test with no
  known failures (PowerShell unrolls the empty array), and `$allowed.Contains` then throws. Until
  the rig track fixes it, apply this change after merging (`git apply` of scratch
  run20/rig-wine-run-fix.patch; the rig's `remote` sends the working tree, so the desktop runs
  it), never commit that file, and undo it (`git checkout` of the file) before the next merge
  and at the end of a run:
  in tools/graphics-rig/desktop/wine-run.ps1 replace the `$allowed = ...` line and the
  `$worse = ...` line after the comment with `$worse = @($run.Failed.Keys | Where-Object { -not
  ($known.ContainsKey($name) -and $known[$name].Contains($_)) })` (scratch
  run20/rig-wine-run-fix.patch).
- Seen in run 8: builds on the desktop failed to link with unresolved `anon.*.llvm.*` symbols
  (stale incremental objects of workspace crates). Every build command of this track now starts
  with `set CARGO_INCREMENTAL=0&& ` (cmd syntax), which fixed it; `bin/wine-all.sh <log> <files>`
  in this track's scratch directory builds that way and runs the named test files.
- Seen in run 7: after `remote` sent edited sources, cargo on the desktop twice called the crate
  `Fresh` and kept the old library (clippy then checked nothing new). When a result looks stale,
  `tools/graphics-rig/remote "cargo clean -p wcontainers-direct3d-12 && ..."`.
- One case of the own program: `check_d3d12 case <name>` (names in `examples/check/cases.rs`).
- Seen in run 10: running a `remote` command (clippy) while a queued test build used the same
  source directory left the shaders library half-built (unresolved `wcs_dxil_constant_sizes` and
  `anon.*` symbols). Run one rig command of this branch at a time; the test command needs
  `set CARGO_INCREMENTAL=0&& ` in both its build and run parts; after such a failure,
  `tools/graphics-rig/remote "cargo clean -p wcontainers-shaders -p wcontainers-direct3d-12"`.
  `bin/test-and-named.sh <log> <files...>` in this track's scratch directory runs the crates'
  tests and then the named test files that way.

Run 15's entries:
- A pulled stream is bound up to its view's end rounded up to a word (WebGPU binds storage in
  words), so an element ending in the last two bytes past a view whose size is no multiple of 4
  reads the buffer's bytes there, not zeros. Views ending inside an element are the only case.
- Real Direct3D 12 refuses an input element whose offset is not a multiple of its size, or of 4
  where the size is larger (an `R8G8B8A8` element at offset 2 refused, an `R8G8` one taken,
  measured by `check_d3d12 case vertex-pulled`); ours now does too. The rule for formats of 8 and
  more bytes is assumed to be 4 from that.
- Render passes are the calls they stand for (targets, clears, resolves), not passes of their
  own: the replay already joins draws into passes, and `CheckFeatureSupport` answers render
  passes tier 0 (no hardware passes). Bound targets stay bound after `EndRenderPass`.

Run 14's entries:
- Pipeline statistics: the counts Direct3D leaves to the implementation take the values of a
  GPU without vertex reuse or clipping splits (vertex shader invocations equal vertices read,
  clipper primitives equal primitives in), and pixel shader invocations count covered pixels
  with the depth and stencil tests off (plan 4.12). Real NVIDIA counts differ (the own case
  compares them as bounds); vkd3d's test checks the same bounds.
- `D3D12CreateDevice` given an adapter (ours or the system's) makes a device whose
  `GetAdapterLuid` is that adapter's; with none, the first adapter of the dxgi.dll the process
  loads (ours where it lies beside the program, else the system's), as Direct3D 12 takes the
  first adapter. Before, always ours, so a program
  matching the device to the system's adapters by LUID (vkd3d's shader runner, for its vendor)
  found none.
- Direct draws reading past a vertex buffer view's end (per-vertex streams without indices,
  per-instance streams with a step rate) draw that stream from a scratch copy of the view's bytes
  with a zero tail, released at the list's end (`VertexFetchZeroed`, plan 5.2). Indexed draws
  keep WebGPU's robust access for indices past the end (the plan's listed loss).
- 3D texture UAV clears with rectangles copy from a scratch 2D texture of the level's size
  cleared to the value, one copy per rectangle and depth slice; a view whose format differs
  from the texture's (not its sRGB twin) is refused by name.

Run 13's entries:
- The replay keeps its own passes; the interface's `PassTracker` only holds the bound targets
  for the `OcclusionCounter` while a counted query runs (as the Direct3D 11 front does), not the
  plan 4.7 route of every pass through the tracker. Moving the replay onto the tracker would
  also give fitted attachments and binary queries across passes (still refused by name).
- Each occlusion heap has a count buffer (8 bytes a query) beside its query set; counted results
  are written there by an ordered write when the query ends, which waits for the count's read
  back (`OcclusionCountReplay`).
- Vertex shaders are always converted with `DrawBaseSource::Helpers`; only those reading
  `SV_VertexID` or `SV_InstanceID` get a helper block, written per draw. Pixel shaders always get
  the pipeline's sample count in their variant key, so pipelines of different sample counts do
  not share a converted pixel shader.
- A texture compared by a sampler but stored in a colour format gets a depth twin at the first
  such binding; the bytes it held before then are copied in as the twin's first inherited write.

Run 12's entries:
- Single-sampled 2D textures whose format resolves are made with the back end's `RENDER` usage
  whatever their flags, since WebGPU resolves only into textures drawn into and Direct3D 12 lets
  `ResolveSubresource` write any such texture (the Direct3D 11 front does the same). A cost in
  memory layout on some GPUs, not measured.
- Per-instance streams of step rate 0 on back ends without `INSTANCE_STEP_RATE` are drawn as
  stride-0 streams bound at the first instance's element; in an `ExecuteIndirect` draw read
  directly from the GPU (first instance unknown) they are refused by name. Other step rates still
  wait on the converter's pulled streams (G6).
- UAV clears with rectangles are done by `clear_rects` on each slice's view; on 3D textures they
  are refused by name ("clearing rectangles of a 3D texture's unordered access view"): the
  interface's `clear_rects` takes one layer of a view, not a depth slice. On buffer views they
  stay refused.
- `ResourceBarrier` checks transitions and records nothing else (the device runs work in
  submission order).

Run 11's entries:
- A heap's program reference count is kept apart from its placed resources' references
  (`abi.rs` replaces the heap vtable's `IUnknown` entries): the program's last `Release` leaves
  the heap alive while resources lie in it, and it may still place resources there, as real
  Direct3D 12 does (vkd3d's test_create_placed_resource).
- `CheckFeatureSupport(SHADER_MODEL)` refuses 6.7 and later (E_INVALIDARG), as this desktop's
  Windows 10 runtime does; before, ours answered them.
- A pipeline made with no depth format draws with its depth state while a depth view is bound
  (real Direct3D 12 tests and writes it: test_unknown_dsv_format).
- Clears waiting for views a pass does not draw into are done before the pass opens (one
  `clear_texture` each), so its draws read them; before, a sampled depth texture read the value
  of the clear before.
- The format answers (`format_support`, shared with Direct3D 11) still give `SHADER_LOAD` for
  the stencil view formats that the front now refuses at use; to change once the view swizzle
  exists or the Direct3D 11 track agrees.

## Interface changes

- `Caps::adapter: AdapterInfo` (run 1): DXGI's adapter description is answered from `caps()`.
- `OcclusionCounter::end_into` and the counter's GPU sum (run 23, commit 07985aa "interface:
  summed the occlusion counter's targets on the gpu..."; plan 4.12 step 4 rewritten, which had
  this dispatch "to come with G2"): where the back end computes with 8 bytes of workgroup
  memory and 16 by 16 workgroups (and reads multisampled textures, for a multisampled
  target), and the kept draws' primitives times the sample count stay below the counting
  format's bound, each count is a dispatch adding every sample of the counting target into
  a u64 buffer of the counter's (workgroup sums, then one 64-bit add with carry a
  workgroup). `end_into(dst, offset)` copies that into a front's buffer in order; `end` reads
  it back. Otherwise the default: read back and summed on the device thread, as before. The
  recorder of G1's tests has no workgroup memory, so its counter tests keep the read back
  (run: `cargo test -p wcontainers-gpu-test --test helpers occlusion`, 7 pass, o1-test.log).
  The Direct3D 11 front keeps calling `end`, which now waits once at the query's end instead
  of at every pass end; `end_into` would let it not wait.
- `COUNTER_BINDING` (15, in `HELPER_SET`) and `SlowPath::PixelInvocationsCounted` (run 22,
  commit 3fc7fee "interface: reserved a helper set binding for a front's own counter..."): the binding of a storage buffer a front's own counting program adds
  to, and the slow path of counting pixel shader invocations by drawing again. The plan's list
  of helper set bindings names it.
- `ShaderModule::from_wgsl(source, entry, stage)` (run 5, commit "interface: made a shader module
  from one entry point of a wgsl module..."): parse, validate and key one entry point of a WGSL
  module, as the built-ins are keyed. The infrastructure crate's present copy uses it instead of a
  builder of its own. G1's `helpers.rs` still has its own copy of the same steps (`module`/`program`);
  it could call this (G1 owns helpers.rs, so this track did not change it).

## Measurements

- Run 23 (2026-11-18, RTX 3080, desktop shared with other tracks, not `--alone`, load not
  taken; run23/check1.log at e148d28 and o1-check.log at e2d1b4e): own program 24 of 24 cases
  equal both times (occlusion-count and statistics among them, after the GPU sum); draw 0 of
  4096 pixels beyond 1; features 253 of 872, outputs 22 of 110, answers 7 of 37 differ, none
  beyond real. Test files 157 of 198, shader runner 204 of 302 (234 s). bgfx 26-occlusion
  2.8 and 2.7 s for 120 frames against real's 2.6 and 2.5 s (o1-bgfx26.log).

- Run 22 (2026-11-18, RTX 3080, desktop shared with other tracks, not `--alone`, load not
  taken; run22/s3-check.log, built at efc9461's sources): fence events, 2000 rounds: real first
  7 µs, mean 1 µs, 99th percentile 1 µs, largest 7 µs; ours first 42 µs, mean 24 µs, 99th
  percentile 121 µs, largest 695 µs (run22/check1.log, before the statistics change: ours mean
  21 µs, 99th percentile 35 µs, largest 89 µs). Own program: 24 of 24 cases equal; draw 0 of
  4096 pixels beyond 1; features 253 of 872, outputs 22 of 110, answers 7 of 37 differ, none
  beyond real. Test files 157 of 198 with the same 41 failing as run 21 (run22/full1.log after
  the merges and the DXIL commit, s3-full.log after the statistics commit); shader runner 204
  of 302 (runner1.log, 279 s). bgfx's run times under "Real program".

- Run 21 (2026-11-18, RTX 3080, desktop shared with other tracks, not `--alone`, load not
  taken; run21/check2.log, built at the library split): fence events, 2000 rounds: real first
  9 µs, mean 1 µs, 99th percentile 2 µs, largest 9 µs; ours first 56 µs, mean 22 µs, 99th
  percentile 55 µs, largest 83 µs. Own program: 24 of 24 cases equal; draw 0 of 4096 pixels
  beyond 1; features 253 of 872 differ, none beyond real. Test files 157 of 198 with the same
  41 failing as run 20 (run21/full1.log after the merges, full2.log after the split); shader
  runner 204 of 302 (runner1.log, runner2.log, 227 s). bgfx's run times under "Real program".

- Run 20 (2026-11-17, RTX 3080, desktop shared with other tracks, not `--alone`, load not taken;
  run20/check3.log, built at c682b6b with 04f7d23's change): fence events, 2000 rounds: real
  first 8 µs, mean 1 µs, 99th percentile 2 µs, largest 15 µs; ours first 53 µs, mean 21 µs,
  99th percentile 52 µs, largest 83 µs. Own program: 24 of 24 cases equal; draw 0 of 4096 pixels
  beyond 1; features 253 of 872, outputs 22 of 110, answers 7 of 37 differ, none beyond real.
  Godot's 2D demo through ours: 18.2 s for 120 frames (run20/godot1.log), not a timing that
  matters (it draws wrong frames).
- Run 19 (2026-11-17, RTX 3080, desktop shared with other tracks, not `--alone`, load not taken;
  run19/check3.log, built at f136240): fence events, 2000 rounds: real first 10 µs, mean 1 µs,
  99th percentile 1 µs, largest 10 µs; ours first 93 µs, mean 23 µs, 99th percentile 59 µs,
  largest 155 µs. Own program: 23 of 23 cases equal; draw 0 of 4096 pixels beyond 1; features
  253 of 872 (268 in run 18: the typed buffer answers), outputs 22 of 110, answers 7 of 37
  differ, none beyond real.
- Run 18 (2026-11-17, RTX 3080, desktop shared with other tracks, not `--alone`, load not taken;
  run18/check3.log): fence events, 2000 rounds: real first 8 µs, mean 1 µs, 99th percentile 2 µs,
  largest 8 µs; ours first 53 µs, mean 17 µs, 99th percentile 24 µs, largest 62 µs. Own program
  (same log): 23 of 23 cases equal (the new `timestamps` case too); draw 0 of 4096 pixels beyond
  1; features 268 of 872, outputs 22 of 110, answers 7 of 37 differ, none beyond real.
- Run 18, shader runner (run18/runner1.log, built at 9c6d863, 232 s): 204 of 302, as run 17.

- Run 17 (2026-11-17, RTX 3080, desktop shared with other tracks, not `--alone`, load not taken;
  run17/check1.log): fence events, 2000 rounds of an empty queue's `Signal`,
  `SetEventOnCompletion` and `WaitForSingleObject`: real first 9 µs, mean 1 µs, 99th percentile
  2 µs, largest 17 µs; ours first 41 µs, mean 21 µs, 99th percentile 50 µs, largest 100 µs.
- Runs 10 to 16, the same measurement and conditions (scratch runN/check*.log): real mean 1 to
  2 µs, 99th percentile 1 to 4 µs; ours mean 18 to 23 µs, 99th percentile 30 to 68 µs, largest
  64 to 213 µs, first round 44 to 90 µs.
- Run 10, not timings: features 273 of 872 differed and 4 were beyond real (`R1_UNORM`'s quality
  levels, answered where real fails); fixed the same run (269 of 872, none beyond).

- Run 7 (2026-11-17, RTX 3080, desktop shared with other tracks, not `--alone`, load not taken):
  fence events, 2000 rounds: real mean 1 µs, 99th percentile 1 µs, largest 7 µs; ours mean 17 µs,
  99th percentile 48 µs, largest 71 µs, first round 41 µs. Not timings: own program draw 0 of 4096
  pixels beyond 1; features 355 of 872 differ, none beyond real (after the multisample fix below;
  before it, 34 format answers promised MULTISAMPLE_RESOLVE on integer formats and MULTISAMPLE_LOAD
  on formats that are no targets, from the WebGPU back end's adapter-specific format flags); outputs
  22 of 110 and answers 7 of 25 differ, none beyond.

- Run 5 (2026-11-17, RTX 3080, desktop shared with other tracks, not `--alone`): fence events, 2000
  rounds: real mean 1 µs, 99th percentile 2 µs; ours mean 35 µs, 99th percentile 60 µs, first round
  32.2 ms. Not timings: own program draw 0 of 4096 pixels beyond 1; features 493 of 872 differ, none
  beyond real; outputs 22 of 110 differ, none beyond; answers 7 of 25 differ, none beyond (ours
  refuses sharing, display and the other heap flags it lacks; everything else equal, including the
  alpha modes, the float swap chain's three presents and the wait with no event).

- Run 4 (2026-11-17, RTX 3080, desktop CPU load 13%, shared, not `--alone`): fence events, 2000
  rounds of an empty queue's `Signal`, `SetEventOnCompletion` and `WaitForSingleObject`: real
  Direct3D 12 mean 1 µs, 99th percentile 4 µs, largest 39 µs; ours mean 33 µs, 99th percentile
  58 µs, the first round 30.5 ms (the device's first submission). A second run: real mean 1 µs,
  99th percentile 2 µs; ours mean 37 µs, 99th percentile 39 µs, first round 35.8 ms. Before the event and timer (a
  250 µs `recv_timeout`), ours was mean 23 µs, largest 26 ms (also the first round), 99th percentile
  not taken.
- Run 4, not timings: the triangle and swapchain scenes and the own program equal to real Direct3D 12
  (largest difference 0); features 493 of 872 differ, none beyond; outputs 22 of 110 differ, none
  beyond.
- Run 3: triangle scene through `Device<WebGpu>` on wgpu's Vulkan and Direct3D 12 paths, largest
  difference 0 (desktop CPU load 14%).
- Line lists (measured by the independent check, its own program in scratch
  `check/own`, mode `lines`): 8 aliased `LINELIST` segments on 64x64, real against ours "59 of 4096
  pixels beyond 1, largest difference 255". A listed loss, below.

## Departures from the plan

- The Windows library is built as cdylib examples of `wcontainers-direct3d-12` (examples may use
  dev-dependencies, so the front never depends on a back end), not in `wcontainers-directx`, which
  the Direct3D 11 track now has on the shared line (Next 5).
- Calls do not go through the call buffer of plan 4.8, although section 10 has the first batch build
  the Windows shell with the channel in-process. Every COM call, `ExecuteCommandLists` with its whole
  replay, every `Create*` and every `Release` included, crosses to the device thread as a boxed
  closure and waits for the answer, so object creation waits for the device thread, against 4.8.
  Command list recording runs on the program's thread. A cost of run speed, not of correctness; Next 4.
- The library draws on wgpu's Vulkan path: with our d3d12.dll loaded, wgpu's Direct3D 12 path would
  load it as the system's. A listed loss follows: Vulkan rasterises aliased lines differently from
  Direct3D 12 on this GPU (Measurements); the Direct3D 11 track moved to wgpu's Direct3D 12 path for
  this, which this library cannot. Points, strips and triangles were equal in the same check.
- `CheckColorSpaceSupport` answers as Windows does on this SDR desktop for the spaces we present
  (sRGB from 8- and 10-bit buffers, linear light only from 16-bit float buffers: real Windows refuses
  sRGB for those, which plan 4.9 had as present), and leaves out the studio-range and HDR10 spaces
  Windows also presents, since we convert none of them. `GetDesc1`'s luminance is Windows' answer
  for an SDR display here (0.5, 270, 270 cd/m²); the primaries are sRGB's.
- A 16-bit float back buffer is presented encoded into an `rgb10a2unorm` surface where presented,
  else `rgba8unorm`, never into an `rgba16float` one (run 5): swap chains present in SDR sRGB only,
  so a float surface adds nothing. Before, the swap chain configured `rgba16float`, which `Caps`
  lists but wgpu's Vulkan surface on this desktop lacks, and every `Present` then removed the device
  (`DXGI_ERROR_DEVICE_REMOVED`). For G2 still: `configure_surface` should fail for a format the
  surface lacks.
- Sync intervals 2 to 4 show each frame for at least one refresh, not exactly that many: the host
  surface has only "wait for the display" or not. A listed loss.
- DXGI's memory sizes in the adapter description are zero (no browser API reports memory).
- `GetResourceAllocationInfo` answers this front's layout (copyable footprints, 64 KiB pages, 4 MiB
  for multisampled textures); `GetCustomHeapProperties` answers a discrete adapter's heaps, as
  `ARCHITECTURE` does.
- Format support reports what the back end does (its converted formats included, run 19) and,
  since run 19, typed buffer views through storage (`BUFFER`, `SHADER_LOAD`). The plan's other
  routes (stream output formats, `NormAsInteger`, `BlockDecoded`, `FilterInShader`,
  `ReinterpretCopy`) are not in the front's answers, so on WebGPU those abilities are absent:
  `SO_BUFFER`, `CAST_WITHIN_BIT_LAYOUT` (without `FORMAT_REINTERPRET`), 3D block-compressed
  textures, filtering of depth views, and formats WebGPU lacks (the packed pairs, the video
  formats).
- Feature level 11_1 is never reported (run 4): Direct3D 12's table asks logic operations and
  target-independent rasterisation of it (and binding tier 1, not 3), and no `Caps` capability
  promises the latter. The plan's sentence in section 9 is corrected to say so.
- `ARCHITECTURE` answers no UMA, no cache-coherent UMA, no tile-based renderer and no isolated MMU.
- check_d3d12 does not ask shader model 6.7 or root signature 1.2: this desktop's Direct3D 12
  runtime refuses both whatever the GPU (ours answers 6.0 and 1.1, tested in tests/front.rs).
- Wave lane counts, where a back end has subgroups, are WebGPU's bounds (4 to 128).
- The infrastructure crate's tests keep their own recorder (tests/support/record.rs, 358 lines) rather
  than G1's (crates/wcontainers-gpu-test): G1's finishes every fence at a submit, so frame pacing
  cannot be tested on it. A parallel code path; the fix is a mode of G1's recorder whose fences
  finish only when the test says (asked of the interface track here; this track does not write in
  G1's crate). Kept until then.
- `SetEventOnCompletion` with no event (run 5) makes an event of the calling thread's own, registers
  it like any event and waits on it, so the device thread stays free for the signal (a CPU signal
  from another thread included); before, it answered E_NOTIMPL unless submitted work reached the
  value.
- A draw with no pipeline state set draws nothing (vkd3d's tests rely on the hardware ignoring it).
- After the device is removed every fence reports `u64::MAX` reached, as Direct3D 12's do.
- `GetParent` of an adapter or a swap chain gives a new factory, not the one that made it.
- Descriptors are 64 bytes (`GetDescriptorHandleIncrementSize`); real NVIDIA's are 32.
- The infrastructure crate answers `MULTISAMPLE_RESOLVE` only for float and normalised formats that
  render multisampled, and `MULTISAMPLE_LOAD` only for formats that are targets, whatever the back
  end's format flags say (run 7): Direct3D's own rules, which the WebGPU back end's adapter flags
  exceeded.
- Planar depth-stencil footprints align rows to 512 bytes, as real Direct3D 12 answers on this
  desktop; the documented 256 holds for the others (run 7, measured by `check_d3d12 case
  depth-planes`).
- `CheckFeatureSupport(SHADER_MODEL)` refuses values naming no shader model (0x51 and 0x60 to 0x69
  are taken), as real Direct3D 12 does for 0, 0x52 and 0x89.
- `ExecuteIndirect` leaves the vertex, index and root state the last read-back command set, where
  Direct3D 12 leaves it undefined.
- `ResolveQueryData` always resolves into a buffer of the front's and copies to the destination
  (run 6), so binary answers can be made 0 or 1 by a storage write whatever heap the destination is
  in (readback buffers cannot be storage). One extra copy per resolve.
- A predicate covers the draws and dispatches up to the next `SetPredication` in the same list; its
  argument records are written at `SetPredication`'s replay, so the program reading the predicate
  runs there, after the work before it (plan 4.12's "before the pass").
- An occlusion query whose draws fall in more than one pass is refused ("an occlusion query across
  passes") rather than summed over passes; a query begun where the open pass counts in another heap
  ends that pass.

- Null descriptors (run 9, `src/device/null.rs`): a view made with no resource, or a constant
  buffer view at address zero, binds a front-owned resource of zeros of the shape the program reads
  (a 64 KiB buffer; 1x1 textures of the program's dimension, sample type and storage format, made
  at pipeline creation). Writable ones are separate and zeroed again before the submission that
  bound them ends, so a write through a null view is dropped for later submissions but visible to
  reads through a null view later in the same submission (Direct3D 12 drops it at once). A null
  view's `GetDimensions` answers 1 (or the 64 KiB buffer's length) where Direct3D 12 answers 0. An
  unwritten (empty) descriptor slot is read the same way: Direct3D 12 leaves that undefined.
- Custom heaps (run 9): properties are checked as a discrete adapter's (`ARCHITECTURE` says so);
  memory the CPU does not see is a default heap; CPU-visible (L0, write-combine or write-back)
  heaps hold textures stored as default ones, and (run 16) buffers kept in program memory and
  moved both ways (see Where things stand). Such textures `Map` with no pointer.
  `WriteToSubresource` writes the box on the device thread (an ordered texture write) and
  `ReadFromSubresource` reads it back at once (`ReadBackWait`); both check the box and subresource
  and do nothing for an empty box. Neither asks for a `Map` first: Microsoft's documentation does,
  but real Direct3D 12 on the RTX 3080 does not (vkd3d's test_read_write_subresource, 68 checks,
  passes there without one).
- `CheckFeatureSupport` answers E_INVALIDARG for features past 34
  (`PROTECTED_RESOURCE_SESSION_TYPES`), which this desktop's Windows 10 runtime does not know and
  refuses so (vkd3d's tests ask `D3D12_OPTIONS8` and `D3D12_OPTIONS16` and pass on it).
- A texture made with more mip levels than its chain has (Direct3D 12 takes that and reports the
  count asked for) is stored with the full chain only.
- Bundles (run 9): `ExecuteBundle` records the closed bundle's commands into the direct list, and
  they replay in its state; what they set stays set. Commands a bundle may not hold are not
  refused at the bundle's `Close`.
- Back buffers (run 9) share one reference on their swap chain while the program holds any of
  them (DXGI's rule, measured by Wine's test_swapchain_refcount); the swap chain holds them weakly.
- Creation checks (run 9, `resource::check_creation`): write states combined with others, buffer
  clear values, flags and layouts, the states of upload and readback buffers, textures in upload
  and readback heaps, zero sample counts, multisampled textures that are no target, target states
  without the target flag, and block-compressed textures of part of a block (refused, as
  Direct3D 12 on this Windows 10 desktop reports no unaligned block textures); heap alignments
  other than 0, 64 KiB and 4 MiB, and heaps allowing display. Failed creation calls set the out
  pointer to null.
- The desktop refused new ssh logins all through run 9 (reset after key exchange, from before
  11:45 to past 12:08 Pacific). Run 9 sent its rig commands over another checkout's shared
  connection that was still open (a symlink at this checkout's control socket path to it,
  ~/.ssh/wc-rig-78928cb75b0c7924, one session at a time), nothing in that checkout touched. The
  link was removed at the end of run 9; if new logins still fail, the rig waits up to 20 minutes.

Run 10's entries:
- The desktop refused new ssh logins all through run 10 too (reset right after key exchange, from
  12:30 Pacific on; the desktop's OpenSSH log shows nothing after 11:45, so the resets never reach
  sshd's logging). As in run 9, rig commands went over another checkout's shared connection that
  was still open (a symlink at ~/.ssh/wc-rig-78928cb75b0c7924 to ~/.ssh/wc-rig-2c84cba95f04e27c),
  one session at a time, nothing in that checkout touched; the link is removed at the end of the
  run (done). Nothing on the desktop was changed to look into it.
- `ALLOW_SHADER_ATOMICS` (0x400) is taken: atomics on one adapter need nothing of the heap. The
  refused heap flags now each print their own reason.
- `MakeResident`, `Evict`, `SetResidencyPriority` and `SetBackgroundProcessingMode` succeed and do
  nothing (resources are always resident; the front has no background work);
  `EnqueueMakeResident` signals its fence at once.
- `CheckFeatureSupport` answers every feature up to 34: no protected sessions, no shader cache
  abilities (so no `GetCachedBlob` or pipeline libraries), no existing heaps, heap serialization,
  cross-node sharing or displayable textures, no meta commands (E_INVALIDARG), and queue priorities
  normal and high supported (one submission order serves all queues, so priority changes
  nothing); values naming no feature answer E_INVALIDARG.
- Quality levels: one sample gives one level for any format but `R1_UNORM`, more samples a level
  only where the format takes them, no samples and `R1_UNORM` E_FAIL (vkd3d's
  test_multisample_quality_levels and check_d3d12 features against real).
- Typeless `R32` and `R16` textures allowing depth-stencil views are stored as `Depth32Float` and
  `Depth16Unorm` (as `R24G8` and `R32G8X24` already were); `R32_FLOAT` and `R16_UNORM` views of
  them read the depth. Before, such depth views failed on WebGPU (Godot's `D16` targets).
- A draw writes a target view's first array slice only, and gets the first viewport and scissor
  only: no stage can choose another (`SV_RenderTargetArrayIndex` and `SV_ViewportArrayIndex` from
  the vertex stage are reported absent, geometry shaders are refused). Clears, with or without
  rectangles, cover every slice of the view. A depth view made with no description covers every
  slice.
- A pipeline with no primitive topology type (run 11: taken, as real Direct3D 12 takes it) builds
  nothing ahead; its draws' topologies decide. Depth bias with no type is refused as for lines.

The next entries came with run 8's patch, committed in run 9 (d89ad8d).
- Views reinterpreting a typeless texture's bits (run 8) get a twin texture in the view's format,
  made at the first such descriptor (`Device::view_holder`), and the descriptor names the twin.
  The twin and its texture inherit each other's writes by the alias copies of plan 5.2 (counted
  as `ReinterpretCopy`). A twin does not inherit from the placed resources its texture overlaps;
  multisampled twins are refused (E_NOTIMPL at use), as WebGPU cannot copy them.
- The frame latency waitable object (run 8) is a semaphore of the swap chain's, made with a count
  of one (the latency such swap chains start with), released by the device thread once for each
  presented frame found finished, and once more per frame `SetMaximumFrameLatency` adds.
  `Present` still waits at the latency as without the flag. Each `GetFrameLatencyWaitableObject`
  gives a duplicate handle, which the program closes.
- `GetClockCalibration` answers E_FAIL, as `GetTimestampFrequency` does, while the back end has no
  timestamps.
- Root signatures are checked at `CreateRootSignature` only (registers bound twice for one stage,
  empty and overflowing ranges, samplers beside views, ranges appended after an unbounded one);
  `D3D12SerializeRootSignature` serializes whatever it is given.

Run 21's entries:
- The Windows library is `wcontainers_d3d12.dll` with `d3d12.dll` and `dxgi.dll` forwarding to
  it by that name (before: one `d3d12.dll` and a `dxgi.dll` forwarding to `d3d12`). A capture
  or counting layer named d3d12.dll (the rig's) loads ours behind it by path, and the program's
  dxgi.dll then still reaches ours, not the layer. One more library file to place.
- bgfx's frames are compared with the overlay's GPU time and GPU memory lines ignored (a
  rectangle of 0,208,320,64 against the rig's 0,236,320,36 for Direct3D 11): both lines print
  values that differ by nature (a measured GPU time the forwarding library does not fix on
  Direct3D 12; memory no browser reports).

Run 20's entries:
- Root signatures are shared objects (`root::RootSignature`, an `Arc`), held by the program's
  object, the pipelines made with them and the lists that set them; the device keeps no table
  of them. A root signature released before a list setting it runs still binds there, as on
  real Direct3D 12 (vkd3d's shader runner does this; Direct3D 12 asks programs to keep objects
  alive until their work ends).
- Filtering in the program takes the sampler's state from its descriptor at the draw, so a
  texture the back end cannot filter costs a program per pipeline and bound state, made at the
  first such draw (`PipelineMiss`), as typed view formats do. Not measured.
- The format answers (`format_support`, shared with Direct3D 11) are unchanged: they still
  report `SHADER_SAMPLE` by the back end's `FILTER`, although the program now filters formats
  without it. To change with the Direct3D 11 track, whose front filters in the program too.

Run 19's entries:
- Mirror-once addressing where the back end lacks it is applied through `border_in_shader`
  (the program filters and addresses itself), not `mirror_once_in_shader` for plain samples as
  the Direct3D 11 front does: one route for every sampling through such a sampler, slower for
  plain samples. Mirror-once is rare; not measured.
- A storage texture read through a buffer ends the open pass before and after its draw, and
  costs two copies of the view per draw (one when read-only); consecutive such draws are not
  joined. Not measured.
- Where the converter refuses atomics on storage textures, the shader is converted once to
  learn it and again with every typed texture view through buffers, so all such views of that
  shader take the buffer route, not only those used atomically.
- `CheckFeatureSupport` answers `BUFFER` and `SHADER_LOAD` for typed buffer formats the
  storage route decodes (those with a vertex fetch form) whatever the texture answers say,
  since Direct3D's `SHADER_LOAD` covers buffer loads; vkd3d's test_buffer_srv asks it so.
- The texel buffer route is `Storage` wherever the back end lacks `TEXEL_BUFFERS`; the
  `Texture` route (WebGL 2) is not used: this front needs compute, which WebGL 2 lacks.

Run 18's entries:
- Storage textures are first converted as 32-bit single-channel formats of their declared
  kind and converted again for other bound formats at the draw (a program made at a draw,
  counted as `PipelineMiss`), since Direct3D 12 names their formats only in the views. Their
  declarations are read from the DXBC program chunk in the front (`src/dxbc.rs`, about 60
  lines), because the converter reports only that a format is missing, not which register.
- A view counter is kept as a GPU virtual address in the two words of the descriptor that
  shader resource views use for the mapping and the clamp, so descriptors stay 64 bytes.
- `GetClockCalibration` pairs a lone timestamp with `QueryPerformanceCounter` read just before
  its submission, after the queue's work finished; the error is the submission's latency (not
  measured). Every queue answers the same timestamp frequency (10^9).
- `QueryVideoMemoryInfo`, `SetVideoMemoryReservation` and the budget and protection
  notifications of the DXGI 1.4 to 1.6 adapter answer E_NOTIMPL (no browser reports memory);
  `GetDesc3` reports monitored fences and no preemption finer than a submission.
- Stream output filled sizes are written back as their low 4 bytes; the high word stays as
  the program left it.

Run 17's entries:
- Every draw through a geometry shader ends the open pass and opens another: the stage helper's
  dispatches must run outside a pass (the interface allows no dispatch inside one) and its
  buffers are reused by the next draw. A cost per geometry draw, not measured; consecutive
  geometry draws could share one pass if the helper kept each draw's output apart.
- A draw or dispatch reading a root descriptor or table not set since the root signature was
  set is skipped with a warning, not refused: Direct3D 12 leaves it undefined, real Direct3D 12
  on this desktop draws with what the hardware still holds and keeps the device, and vkd3d's
  test_update_descriptor_tables_after_root_signature_change expects the device to survive.
  Before, such a draw removed the device.
- A draw whose topology does not give the geometry shader's input primitive draws nothing
  (the helper's `TopologyMismatch`); Direct3D 12's debug layer calls it an error and the
  hardware's result is undefined.

Run 16's entries:
- `DXGI_SCALING_NONE` swap chains are taken, but the host shows every frame stretched to its
  window: a window whose client area differs from the buffers shows the frame stretched, where
  DXGI shows it unscaled at the top left. A listed loss, warned once per swap chain at the first
  such `Present` ("a frame stretched to its window against the swap chain's scaling").
- `SetHDRMetaData` is checked (type, size, pointer) and changes nothing: swap chains present in
  SDR sRGB only, where the content's mastering description has no use.
- `ALLOW_SHADER_ATOMICS` (heap flag 0x400) is refused as undefined (E_INVALIDARG), as this
  desktop's Windows 10 runtime refuses it; newer runtimes take it.

## Licences

- `slab` 0.4 (MIT, read in its Cargo.toml): the device's object tables.
- `half` 2.7.1 (MIT OR Apache-2.0, read in its Cargo.toml; default features off; already in the
  workspace through the interface and the WebGPU back end) for half floats in UAV clears (run 12).
- `wcontainers-shaders` (ours) and through it dxil-spirv (MIT) and dxbc-spirv (MIT), as plan
  section 3 and the shaders track record.
- `naga` (MIT OR Apache-2.0, the workspace's copy, plan section 3): the front reads the sizes a
  program's uniform blocks declare (run 5). The infrastructure crate depends only on
  `wcontainers-gpu` (run 5; it had `naga` and `blake3`, MIT OR Apache-2.0 and CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception,
  which plan section 3 covers).
- `md-5` 0.11 (MIT OR Apache-2.0, read in its Cargo.toml; default features off) for MD5's block
  function in the DXBC checksum, with `digest` 0.11, `block-buffer` 0.12, `crypto-common` 0.2 and
  `hybrid-array` 0.4 (each MIT OR Apache-2.0, read in their Cargo.toml) (run 11).
- `llvm-bitcode` 0.4 (0.4.2 in the lock; MIT AND Apache-2.0 WITH LLVM-exception, read in its
  Cargo.toml) to read DXIL's LLVM bitcode for its resource list (run 22), with `num_enum` and
  `num_enum_derive` 0.7.6 (BSD-3-Clause OR MIT OR Apache-2.0, read in their Cargo.toml);
  `bitflags` and `proc-macro-crate` were already in the workspace.
- Windows library only (dev-dependencies): `windows` and `windows-core` 0.62.2 (MIT OR
  Apache-2.0, as plan section 10 confirms).
- Test only: `wcontainers-gpu-webgpu`, `wgpu` 30.0.2 (MIT OR Apache-2.0), `pollster` (MIT OR
  Apache-2.0), `png` (MIT OR Apache-2.0), as the WebGPU track already uses them.

## Installed on the desktop

Run 24: nothing installed. The rig's build folder of this branch, C:\wc\target\phase-2-graphics-direct3d-12
(20.0 GB), was deleted at the end of the run, so the next run builds from clean (about 8 minutes
more). No build folder was made on the Mac.
Run 23: nothing installed; no build folder made besides the rig's (kept, the rig reuses it). A
temporary folder C:\wc\out\phase-2-graphics-direct3d-12\dump (Godot's refused shaders, 684 KB)
was made and deleted in the run; its copy is in scratch run23/dump. On the Mac two `cargo
check` runs made target/ in this checkout (218 MB, then the interface crate's); deleted.
Run 22: nothing installed; no build folder made besides the rig's (C:\wc\target\phase-2-graphics-direct3d-12, kept, the rig reuses it). bgfx's captured frames and logs under C:\wc\out\phase-2-graphics-direct3d-12\bgfx as in run 21. On the Mac a `cargo check` of the merged tree made target/ in this checkout (219 MB before it failed at the shaders crate's missing submodules); deleted at the end of the run.
Run 21: nothing installed; no build folder made besides the rig's. In
C:\wc\out\phase-2-graphics-direct3d-12: bgfx-capture.ps1 (2 KB) and bgfx-compare.ps1 (1 KB), and
bgfx's captured frames and logs under bgfx (11.6 MB, kept as evidence; bgfx's own art). Run 20: nothing installed; no build folder made besides the rig's. Run 19: nothing installed; no build folder made besides the rig's. Run 18: nothing installed. Scratch copies of bgfx examples under
C:\wc\out\phase-2-graphics-direct3d-12\bgfx as in run 16 (with an empty nvapi64.dll in the
nonvapi runs), and Godot's output under C:\wc\out\phase-2-graphics-direct3d-12\programs (the
rig's). Run 17: nothing installed. The rig's build folder of this branch,
C:\wc\target\phase-2-graphics-direct3d-12, is kept for the next run of this track (the rig
builds there every run); this run made no other build folder there or on the Mac's scratch.

Nothing beyond the rig's setup. Run 16: C:\wc\out\phase-2-graphics-direct3d-12\bgfx-run.ps1 (a
1 KB script) and its copies of bgfx examples and our libraries in
C:\wc\out\phase-2-graphics-direct3d-12\bgfx (about 3.9 MB a program run, 13 MB more for ours). The shaders crate's submodules are checked out in this branch's
source directory there (C:\wc\src\phase-2-graphics-direct3d-12\crates\wcontainers-shaders: dxbc-spirv
and dxil-spirv, 18,052,177 bytes).
