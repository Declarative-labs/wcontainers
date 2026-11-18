# Direct3D 12 and DXGI infrastructure status (G5)

Branch: phase-2-graphics-direct3d-12. Plan: docs/phase-2-graphics.md, sections 4.9, 5.2, 9, 10 and
G5 in 12. This track owns crates/wcontainers-directx-graphics-infrastructure and
crates/wcontainers-direct3d-12.

## Where things stand

G5 is not complete. What is left waits on other tracks: 21 test files on the shaders track (G6:
11 on converter features, 10 on parts of the stage helper) and 3 on a view swizzle (G1 and G6);
see "Gap list" and "Next". Real Direct3D 12 programs draw frames matching real Direct3D 12's:
Microsoft's D3D12HelloTriangle and D3D12HelloConstBuffers, bgfx's 01-cubes, 26-occlusion,
35-dynamic, 37-gpudrivenrendering and 38-bloom ("Real program"). Godot's two platformers wait
on G6 and the swizzle.

Latest results (run 27, 2026-11-18, at 058f3e3): test files 164 of 198, the same 34 failing as
runs 25 and 26; own cases 25 of 25 equal real Direct3D 12; shader runner 205 of 302; the
crates' 43 tests pass; bgfx 37 draws every frame as real Direct3D 12 does.

What each run did (runs 1 to 16 on 2026-11-17, 17 to 27 on 2026-11-18; runs 17 on are the
milestone "G5 to the pass bar"):
- 1 to 4 the first slice; 5 Wine's and vkd3d's tests through ours, root signatures; 6 compute,
  UAVs, queue waits, occlusion, predication, heaps; 7 alias copies, `ExecuteIndirect`, the pass
  bar's cases; 8 to 10 allocators, resolves, twin textures, null descriptors, custom heaps,
  bundles, newer interfaces, every `CheckFeatureSupport` feature; 11 placed resources keeping
  heaps alive, root signature checksums; 12 half and packed UAV clears, 1D UAVs, transitions
  checked at `Close`; 13 counted occlusion, draw bases, dual-source blending, depth twins; 14
  statistics queries, bases from indirect arguments, rectangle UAV clears; 15 pulled vertex
  streams, restart rewrites, render passes; 16 custom heap buffers, scaling-none swap chains.
- 17 geometry shaders through the stage helper; 18 timestamps, storage textures in their views'
  formats, view counters, stream output; 19 typed buffers through storage, helper blocks of
  pixel and compute stages, sampling in the program, storage textures through buffers; 20 root
  signatures kept alive by pipelines and lists, filtering in the program; 21 the library split
  into `wcontainers_d3d12.dll` with `d3d12.dll` and `dxgi.dll` forwarders, bgfx's first
  matching frames; 22 DXIL typed views read from the resource list, pixel shader invocations
  counted on the GPU; 23 DXIL slots by Direct3D 12's numbering, counted occlusion summed on the
  GPU, `D3D12GetInterface`; 24 hull and domain shaders through the stage helper; 25
  multisampled arrays as a texture per layer, `DiscardResource`, component mappings equal to
  the identity; 26 the shader cache, sequential blit buffers, DXGI 1.5 to 1.7's factory,
  `ID3D12Fence1`, `ID3D12GraphicsCommandList6`.
- Run 27 (the eleventh run of the milestone): merged shaders (DXIL storage textures always in
  the bound view's format, a read and written one of another format than the 32-bit
  single-channel ones refused `ReadWriteStorageFormat`; DXIL infinite and NaN constants; DXIL
  arrays of textures as a texture per element) and direct3d-11 (nothing in this track's
  crates); the shared line, webgpu, webgl and rig had nothing new. The front now answers
  `ReadWriteStorageFormat` as the shaders track asks: a storage texture the program both reads
  and writes is read through a buffer of its texels (`StorageFormatThroughBuffer`, run 19's
  route) unless its format is one of WGSL's read-write formats (`R32_FLOAT`, `R32_UINT`,
  `R32_SINT`), and a read-only one unless the back end has `STORAGE_READ` for it
  (`bind.rs`, `stores`). The shader runner's sm6-uav-rwtexture now runs 539 checks (a crash at
  its first draw before) and stops at an array of storage textures (G6). Godot's 2D platformer's
  refused pipelines fell from 72 to 9 with the merge; it still draws black (Real program).

What the front does, by subject:
- Shader cache (run 26, `src/cache.rs`): every conversion goes through the shaders crate's
  `convert_cached` into one cache the process's devices share, so a shader and variant
  converted for one pipeline is decoded, not converted again, for the next (and the trial
  conversion `convert_stage` makes is not repeated). The Windows library opens the program's
  cache file before the first device (`examples/directx/cache.rs`), in the directory
  `WC_GPU_CACHE` names, under the Direct3D 11 library's name for it (`<exe stem>-<16 hex digits
  of BLAKE3 of the lower-case full path>.wcgpu`), so a run decodes what earlier runs converted;
  without the variable conversions are kept for the process only. No state combinations are
  recorded: pipelines are built when created (`prepare`), as plan section 8 says Direct3D 12
  allows. Refused conversions are not kept and are tried again. Test tests/cache.rs (its own
  binary: the cache is the process's). Measured under "Real program".
- Sequential blit swap chains (run 26, infrastructure crate): `DXGI_SWAP_EFFECT_SEQUENTIAL`
  with several buffers gives out every buffer (`chain.buffer(i)`), as DXGI does; at each present,
  after the copy into the surface, each buffer takes the next one's contents and the last the
  first's, through a spare texture (n + 1 copies, `SequentialBuffersCopied`, counted and warned),
  which is what Wine's test_swapchain_flip checks. Discard keeps buffer 0 only. Direct3D 12
  takes flip models only, so this serves the Direct3D 11 front, which must refuse target views
  of buffers past the first (real DXGI's E_INVALIDARG). Test
  `sequential_blit_buffers_move_down_at_each_present`.
- DXGI factory (run 26): `IDXGIFactory7`. `CheckFeatureSupport(PRESENT_ALLOW_TEARING)` answers
  FALSE (the host shows whole frames), anything else or a wrong size `DXGI_ERROR_INVALID_CALL`
  (as Wine's test_factory_check_feature_support expects of real DXGI);
  `EnumAdapterByGpuPreference` gives the one adapter for every preference (E_INVALIDARG for an
  unknown one); `RegisterAdaptersChangedEvent` hands out cookies whose event is never set (the
  adapter list never changes). Microsoft's samples take the factory 6 route now and still draw
  equal frames.
- Multisampled texture arrays (run 25): the interface's multisampled textures have one layer
  (plan 4.1: a front holds such an array as a texture per layer), so `Storage::Texture` keeps
  the first layer's texture and `layers`, the others'. Views, render targets, clears, resolves,
  copies (`CopyResource`, `CopyTextureRegion` of whole subresources) and alias copies take each
  layer's own texture (`Storage::layer`). Refused by name: a view of several layers of one (a
  shader's `Texture2DMSArray`, which the converter refuses too, `MultisampledArrayTexture`), and
  one viewed in a format needing a twin.

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
  views of several layers of a multisampled texture array (run 25; run 20 refused such arrays
  at creation), arrays of resources, component mappings that read other than the identity
  does for the view's format (run 25: forcing zero or one where the format lacks the
  component, or naming one it lacks, reads the same and is taken; a refused one is printed
  with its value), min/max filters, alpha-antialiased lines,
  depth bias on lines and points, forced sample counts, conservative rasterisation, raw and
  structured views at offsets the back end cannot bind, unordered access buffers in custom
  heaps the CPU sees,
  timestamps where the back end lacks `TIMESTAMPS` (`GetTimestampFrequency` and
  `GetClockCalibration` then answer E_FAIL as for a queue without them), stream output from
  streams other than 0 and drawing a stream other than 0, rectangles in UAV clears of a 3D texture's view in another
  format than its own, an occlusion query across passes, video command lists, pipeline libraries (`DXGI_ERROR_UNSUPPORTED`), protected sessions, geometry shaders and stream output after a domain shader, state objects (ray
  tracing), depth bounds, programmable sample positions, view instancing, sampler feedback.
- Newer interfaces (run 10): the device is `ID3D12Device8`, command lists
  `ID3D12GraphicsCommandList2` (since run 15 `ID3D12GraphicsCommandList4`, since run 26
  `ID3D12GraphicsCommandList6`, whose shading rate and mesh calls fail `Close` with E_NOTIMPL:
  `CheckFeatureSupport` answers both absent), fences `ID3D12Fence1` (run 26; `GetCreationFlags`
  gives the flags made with), resources `ID3D12Resource2`; `ID3D12Device9` and later and
  `ID3D12GraphicsCommandList7` and later are not offered (E_NOINTERFACE). Render passes (run 15, `objects.rs`): `BeginRenderPass` records
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
- ms-array (run 25): a 4-sample array of three layers, layer 0 cleared by a view of it alone,
  layers 1 and 2 by one view of every layer from 1 (`ArraySize` -1), a triangle drawn into
  layer 2, the array copied whole by `CopyResource` and each layer of the copy resolved;
  colour counts of the three resolved layers.
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
  13 (15, then 16, 17, 20, 21, 22, 23, 24 in run 24, 25 in run 25: run25/m1-check.log).

## Test files (Wine's and vkd3d's, through our libraries, RTX 3080)

How a file counts: the rig compares each test's verdict with real Direct3D 12's on this GPU
(`wine-tests run`, below); a test that skips what real Direct3D 12 runs, or runs fewer checks,
fails. Since the rig's run of 2026-11-17 19:11 (its commit 4ffedc6), tests that fail some
checks on real Direct3D 12 are counted too, held to every other check, so the counted set is
198 (it was 193).

Run 27 (2026-11-18, desktop shared with other tracks; scratch run27/full1.log after the merges,
run27/full2.log at 058f3e3, the run's last code commit): 164 of 198 both times, the same 34
failing files as runs 25 and 26. d3d12 (Wine) 10 of 10, vkd3d-d3d12 142 of 176,
vkd3d-d3d12_invalid_usage 3 of 3, vkd3d-hlsl_d3d12 9 of 9. Each earlier run's logs are in
scratch run<N>/full*.log (since run 18); the counts are in the history below. Newly passing in
the latest runs: run 25 tgsm and clear_render_target_view; run 24 hull_shader_fork_phase,
nop_tessellation_shaders, tessellation_dcl_index_range, tessellation_primitive_id and
vertex_id. No run since 17 lost a file.

Files passing, by test file: d3d12 (Wine) all 10; vkd3d-d3d12_invalid_usage all 3;
vkd3d-hlsl_d3d12 all 9; vkd3d-d3d12 all but the 34 below (runs 25 to 27). The failing ones, each by
what it stops at first:
- The converter (G6), 11: `StorageOffset` (buffer_srv: a structured view at byte 240; its
  typed views pass), `[earlydepthstencil]` left out of DXBC programs (`lower.rs` sets no
  `early_depth_test`), so pixels the depth test rejects still run the shader's atomics
  (early_depth_stencil_tests, 1 check), `AtomicSignedness` (atomic_instructions), `RegisterSpace`
  2 (register_space, sampler_register_space), `DynamicOffset` 2 (gather, gather_c),
  `CullDistance` (combined_clip_and_cull_distances), `MultisampledArrayTexture`
  (multisample_array_texture), `InterpolateAt` (shader_eval_attribute), resource arrays
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
- A listed loss, 1: execute_indirect (vertices past a vertex buffer's end in an indirect draw).
- Reported absent as Direct3D 12 expects, the test then running fewer checks or stopping, 8:
  create_reserved_resource, get_resource_tiling (tiled resources),
  create_graphics_pipeline_state and output_merger_logic_op (logic operations; the first
  stops where real Direct3D 12 makes a pipeline with one and then releases the null pipeline),
  shader_instructions (float64), unbounded_resource_arrays, unbounded_samplers (binding tier 2),
  ps_layer (the layer from a vertex shader).
- Real Direct3D 12 crashes, 1: copy_block_compressed_texture. Creating a block-compressed texture
  of part of a block fails on both (this Windows 10 runtime refuses it); the test then copies
  from the null texture: real Direct3D 12 crashes there, ours fails the list's `Close` and the
  test goes on to fail later checks before crashing. Not to be matched.

History (tests passing on real Direct3D 12 that pass through ours, vkd3d-d3d12 of 172 to 174):
run 5 54, 6 66, 7 80, 8 87, 9 74 (the rig's stricter rule began), 10 87, 11 94, 12 98, 13 103,
14 104, 15 106, 16 106; run 17 108 of 176 under the newer rule, run 18 123 of 176, run 19 133,
run 20 135, runs 21 to 23 135, run 24 140, runs 25 to 27 142. Whole counts: run 12 116 of 193,
13 121, 14 122, 15 124, 16 124; run 17 129 of 198, run 18 145, run 19 155, run 20 157, runs 21
to 23 157, run 24 162, runs 25 to 27 164.

## Shader runner (vkd3d-shader_runner, 302 files)

Run with our d3d12.dll alone (the system's dxgi.dll): with our dxgi.dll too every file crashes
in its Direct3D 11 part before reaching Direct3D 12 (our dxgi.dll serves only Direct3D 12), 0 of
299 (run12/runner1.log). Command: `E='%CARGO_TARGET_DIR%\debug\examples';
tools/graphics-rig/wine-tests run --dll "wcontainers_d3d12.dll=$E\wcontainers_d3d12.dll" --dll
"d3d12.dll=$E\d3d12.dll" vkd3d-shader_runner`.

Run 27 (run27/runner1.log after the merges, runner2.log at 058f3e3): 205 of 302. hlsl
arithmetic-float newly passes (the converter's merge); the other 97 failing files are run 26's.
A focused run after the read-write commit (run27/rw1-runner.log): sm6-uav-rwtexture runs 539
checks with 2 failing (real 781, none) and stops at line 119, an array of storage textures the
converter refuses (`ResourceArray`, G6); before, its first draw removed the device
(`ReadWriteStorageFormat`). uav-rwtexture stops at its shader model 6.0 section (line 450:
naga's SPIR-V reader, "unsupported image write operand 0x100", G6), failing 12 checks where
real Direct3D 12 fails 10.

What the 97 failing files stop at (sorted in runs 12 to 23, unchanged since):
- 37 run fewer checks than real with no failure: `CheckFeatureSupport(SHADER_MODEL)` answers
  6.0 (plan section 9), so the runner compiles shader model 6.0 only where real runs 6.0 to 6.6
  ("Compiling SM6.0-SM6.5 shaders" in real's log); files needing 6.2 and later skip those
  sections.
- The converter (G6): DXIL hull, geometry and some compute shaders in naga's SPIR-V reader,
  `PackedStageValues`, `StructType`, `ConstantComponentIndexing`, `SampleClamp`,
  `InterpolateAt`, `DynamicOffset`, `CullDistance`, `ClipDistance`, `PrimitiveId`, arrays of
  storage views, and wrong values in converted shaders (arithmetic-float-uniform: min16float
  constants read as 32-bit; frexp; texture-load).
- sample-minmax: minimum and maximum filters, absent (gap list).
- uav-atomics and length-property bind views of another kind than their shaders declare
  (undefined in Direct3D 12; ours removes the device, real draws).
- hlsl mesh, uav-rwbuffer and uav-rwtexture fail some checks on real Direct3D 12 too; the rig
  holds them to the others.

History: 194 of 299 (runs 12, 13), 204 of 299 (runs 14, 15: the device now matches the
system's adapter by LUID, so the runner keeps its vendor tag), 204 of 302 (runs 17 to 26), 205
(run 27). The whole set takes 220 to 280 s.

## Real program

Every timing here: RTX 3080, desktop shared with other tracks, not `--alone`, load not taken,
debug builds, whole runs of 120 frames.

- Microsoft's D3D12HelloTriangle and D3D12HelloConstBuffers (the rig's `programs`, with the
  Agility SDK's D3D12Core.dll beside them, which ours does not load): frames 10, 60 and 120
  equal real Direct3D 12's references, 0 pixels beyond 8 (runs 22, 25, 26; 2.6 and 2.5 s in run
  25). Since run 26 they take the factory 6 route. Command: scratch `bin/programs22.sh <log>
  d3d12-hello-triangle d3d12-hello-constbuffers` (`programs run` with the three `--dll`s).
  The replacement d3d12.dll must be the forwarder, not `wcontainers_d3d12.dll` itself: two
  copies of the library under two names are two instances, and every program then fails.
- bgfx on Direct3D 12 (scratch `bin/bgfx-compare.ps1 <example> <forwarding dll> <gt-run.exe>
  [ignored rectangle]`, copied to C:\wc\out\phase-2-graphics-direct3d-12 with
  `bgfx-capture.ps1`): records twice on real Direct3D 12 behind the rig's forwarding library
  (frame clock, frames 10, 60 and 120 from the swap chain's queue) with `gt-run frames
  --record`, runs twice through ours, compares each frame. Ignored rectangle 0,208,320,64: the
  overlay's GPU time (the forwarding library does not fix timestamps by the frame clock on
  Direct3D 12) and GPU memory (ours reports none, `QueryVideoMemoryInfo` E_NOTIMPL) lines.
  Every frame of both runs passes, the differences all in the overlay's text: 01-cubes and
  38-bloom (run 21), 26-occlusion (runs 21 to 23), 35-dynamic (runs 21, 22),
  37-gpudrivenrendering (runs 25 to 27). Times against real in the same job: 37 2.5 s twice
  against 2.4 s twice (run 27: 717 to 889 pixels beyond 8, allowed 20480; run27/p1-bgfx37.log);
  26-occlusion 2.8 and 2.7 s against 2.6 and 2.5 s (run 23, after the GPU sum; 47 to 55 s
  before); 35-dynamic 3.3 s twice against 2.5 and 2.6 s (run 22); 01-cubes 2.6 s and 38-bloom
  2.7 s against 2.4 to 2.7 s (run 21). 24-nbody runs (run 22) but real Direct3D 12 draws it
  differently on every run, so no frame is compared. The rig's own bgfx-01-cubes-d3d12 and
  bgfx-38-bloom-d3d12 need new references (the rig track's). Command: `D='C:\wc\out\phase-2-graphics-direct3d-12';
  tools/graphics-rig/gpu-job --desktop --build "set CARGO_INCREMENTAL=0&& cargo build -j 3 -p
  wcontainers-direct3d-12 --examples && cargo build -j 3 -p wcontainers-graphics-test-forward -p
  wcontainers-graphics-test" -- "powershell -NoProfile -ExecutionPolicy Bypass -File
  $D\bgfx-compare.ps1 01-cubes %CARGO_TARGET_DIR%\debug\wcontainers_graphics_test_forward.dll
  %CARGO_TARGET_DIR%\debug\gt-run.exe 0,208,320,64"`.
- Godot's 2D platformer (run 27, run27/p1-*.log; scratch `bin/programs27.sh`): 120 frames in
  9.3 s, every frame black where the reference draws the level (409,222 of 518,400 pixels
  beyond 8 at frame 10, the same count since run 23; scratch run27/frames). Its refusals, all
  G6's: 2 fragment shaders `IsHelperInvocationEXT` (naga's SPIR-V reader; perhaps the canvas's
  own, which would explain the black frames, not confirmed), 3 compute shaders with
  `globallycoherent` views ("memory access 0x28 outside workgroup memory") and 1 "image load
  operand 0x200", 2 `StorageFormatKind` (the shaders track's corpus refuses the same shaders),
  1 `ResourceArray` (an array of storage views), and one pipeline drawing alpha-antialiased
  lines (`LINE_COVERAGE` and the stage helper's rectangles, G6). The merge of run 27 took away
  44 `ResourceArray`, 20 `NonFiniteConstant` and the "Bindings conflict" refusals.
  `QueryVideoMemoryInfo` answers E_NOTIMPL 426 times. With the cache file (run 26,
  `bin/warm26.sh`) its file was 334,080 bytes after the first run and the same after the
  second: nothing converted again (8.2 then 7.9 s, single runs).
- Godot's 3D platformer (runs 25 to 27): a shader resource view of `R8G8B8A8_UNORM_SRGB` with
  mapping 0x1a88 (red, green, blue, one) needs the view swizzle (G1 and G6); the device is
  removed there and Godot crashes on what follows (0xc0000005 at 6.7 s). Besides, 17 fragment
  shaders `IsHelperInvocationEXT` and the 2D platformer's refusals.
- Learnt on the way, which explains the setup: the rig's forwarding library takes the name
  d3d12.dll and loads ours behind it (`WC_D3D12`), hence `wcontainers_d3d12.dll` and two
  forwarders (run 21); it aborts a program calling an export the library behind it lacks
  (Godot's `D3D12GetInterface`, run 23); bgfx calls `IDXGIAdapter3::QueryVideoMemoryInfo` on
  the adapter `EnumAdapters` gives without asking for that interface (adapters are
  `IDXGIAdapter4` since run 18); NVIDIA's NVAPI calls into our device as if it were the
  driver's (harmless, absent in a container; `bgfx-run.ps1 <name> <seconds> ours nonvapi`
  keeps it out); Godot's textures with `ALLOW_SHADER_ATOMICS` fail E_INVALIDARG through ours
  as on this Windows 10 runtime; a Rust panic in the library shows as 0xc0000409.

## Gap list (the pass bar's written list, as of run 27; its test files unchanged since run 25)

The final list may hold only gaps that are absent features: what the back end or browser lacks,
reported absent as Direct3D 12 expects. Run 25's failing test files (34), by that rule:

Absent features (final; reported absent through Direct3D 12's own mechanism, or refused by name
where Direct3D 12 has none), 8 files:
- Tiled resources (`TiledResourcesTier` 0): create_reserved_resource, get_resource_tiling.
- Logic operations (`OutputMergerLogicOp` false): output_merger_logic_op, and
  create_graphics_pipeline_state, which makes a pipeline with one without asking and then uses
  the null pipeline.
- Float64 in shaders (`DoublePrecisionFloatShaderOps` false): shader_instructions.
- Resource binding tier 2 and descriptor arrays where the back end lacks big tables (tier 1
  answered): unbounded_resource_arrays, unbounded_samplers.
- The layer from the vertex shader
  (`VPAndRTArrayIndexFromAnyShaderFeedingRasterizerSupportedWithoutGSEmulation` false, plan
  4.12): ps_layer, whose context makes such a pipeline before asking (E_NOTIMPL) and then skips
  its checks. Listed under the converter before run 25; its pixel shader's layer input now
  converts.
- Minimum and maximum filters (WebGPU has no reduction samplers): the shader runner's
  sample-minmax. Direct3D 12 has no query for them: Microsoft's `D3D12_FILTER` page (read
  2026-11-17) sends the reader to Direct3D 11's `D3D11_FEATURE_DATA_D3D11_OPTIONS1::MinMaxFiltering`,
  so ours refuses such samplers by name at use (E_NOTIMPL, the device removed), the only report
  Direct3D 12 leaves.
- Shader models above 6.0 (plan section 9's answer): 37 shader runner files run fewer checks.
- Also reported absent with no failing test here: conservative rasterisation, raster order
  views, ray tracing, mesh shaders, variable rate shading, sampler feedback, timestamps where
  the adapter lacks `timestamp-query` (present on the RTX 3080 natively), drawing into and
  storing into `B5G6R5`, `B5G5R5A1`, `B4G4R4A4`.

A listed loss (not an absent feature; kept on the list with its reason):
- execute_indirect, 2 checks: `ExecuteIndirect` draws reading vertices past a vertex buffer's
  end, which WebGPU skips whole where Direct3D 12 reads zeros. Pulled streams would make it
  exact only by fetching every stream of every indirect draw, slowing every indirect draw for a
  case only faulty programs reach.

Not to be matched:
- copy_block_compressed_texture: real Direct3D 12 crashes in it (its texture of part of a block
  is refused on both; the test then uses the null texture).

Not absent features, so not yet allowed on the list (others' work, then this front's routing):
- G6, the converter, 11 files: `StorageOffset` (buffer_srv), `[earlydepthstencil]` in DXBC
  (early_depth_stencil_tests), `AtomicSignedness` (atomic_instructions), `RegisterSpace`
  (register_space, sampler_register_space), `DynamicOffset` (gather, gather_c), `CullDistance`
  (combined_clip_and_cull_distances), `MultisampledArrayTexture` (multisample_array_texture:
  since run 25 the front holds such arrays, a texture per layer; reading one in a shader then
  needs a binding form for several textures, to be settled with G1 and G6), `InterpolateAt`
  (shader_eval_attribute), resource arrays (resource_arrays). tgsm passes since run 25.
- G6, the stage helper after a domain stage and layer outputs, 10 files: geometry shaders and
  stream output after a domain shader (domain_shader_inputs,
  domain_shader_one_patch_constant_input, hull_shader_patch_constant_inputs,
  hull_shader_relative_addressing, line_tessellation, quad_tessellation), patches of another
  control point count than the hull shader declares (hull_shader_control_point_phase),
  `VertexOutputShape` (clip_distance), `StructType` in a hull shader (hull_shader_punned_array:
  "struct value 75 used as a value"), a geometry shader's layer output (layered_rendering,
  `LayerSplit`).
- G1 and G6, a view swizzle, 3 files: depth_stencil_sampling, srv_component_mapping,
  stencil_load; also what stops Godot's 3D platformer (run 25). Direct3D's stencil views give
  the stencil in the second component, WebGPU's stencil-aspect views in the first, and
  component mappings choose any. wgpu 30 has no view swizzle and `TextureViewDesc` none, so the
  default is a program rewrite (the converter reading each swizzled slot's mapping, as the
  Direct3D 11 track's status asks too). Since run 25 the front takes mappings equal to the
  identity for the view's format.
- The shader runner's other failures are G6's (DXIL hull, geometry and some compute shaders in
  naga's SPIR-V reader, arrays of storage views, `PackedStageValues`,
  `ConstantComponentIndexing`, `SampleClamp`, wrong values in converted shaders), plus the two
  files binding views of another kind than declared (uav-atomics, length-property), undefined
  in Direct3D 12.
- The real program: done for bgfx 01-cubes, 38-bloom, 26-occlusion and 35-dynamic (runs 21 to
  23), 37-gpudrivenrendering (run 25) and Microsoft's D3D12HelloTriangle and
  D3D12HelloConstBuffers (run 22). bgfx 24-nbody runs (run 22) but draws differently on every
  real run, so it has no frames to compare. Godot's 2D platformer runs and draws black, waiting
  on G6 (helper invocations, `globallycoherent` views, `StorageFormatKind`, arrays of storage
  views, antialiased lines; run 27); its 3D platformer on the view swizzle first.
- Front, refused by name with no failing test here: sampling in the program, typed views of other
  formats and the minimum level-of-detail clamp in vertex and geometry shaders, storage texture
  format variants with pulled vertex programs or geometry stages, stream output buffers not
  starting at a multiple of 256 bytes, unordered access buffers in CPU-visible custom heaps,
  comparison sampling of a texture whose bits match no depth format and of a multisampled one,
  views of several layers of a multisampled array.

## Next, in this order

1. At the start of each run merge the shared line, webgpu, shaders and rig. Check whether the
   rig fixed its comparison (`tools/graphics-rig/desktop/wine-run.ps1`, "How to run"); until it
   does, apply the one-line patch uncommitted for the run and never commit that file.
2. After each shaders merge rerun every test file (bin/full27.sh), Godot's two platformers
   (bin/programs27.sh) and read their refusals; the front's part where a converter item lands:
   `StorageOffset` lets `storage_range` bind any offset; a view swizzle (with G1) means converting
   programs with each swizzled slot's mapping and writing it per draw; `MultisampledArrayTexture`
   needs the binding form for a texture per layer.
3. Tessellation, waiting on the stage helper (G6): a constructor taking a geometry shader or
   stream output after the domain stage goes where `geometry_stage` refuses them now; patches
   whose control point count differs from the hull shader's declared input count, which real
   Direct3D 12 draws (hull_shader_control_point_phase: input count 1, drawn with
   3-control-point patches, `Stages::run` answers `TopologyMismatch`).
4. Godot's 2D platformer draws black (run 27). Once G6 takes `IsHelperInvocationEXT`, rerun
   it; if it is still black, find the first pipeline or draw that fails (the refusals print at
   pipeline creation; a frame capture of the rig's forwarding library shows the passes).
5. Turn this gap list into the final one as the G6 items land.
6. Later: render pass resolves of part of a subresource and of depth-stencil targets; the built-in storage clear for UAV clears of
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
- Back buffers: `chain.buffer(i)` is a `B::Texture` (`chain.buffer_desc()` describes it); the
  discard blit model gives buffer 0 only, the sequential one every buffer (since run 26: those
  past the first are for reading, so refuse target views of them as DXGI does; their contents
  move one buffer down at each present). Draw into `chain.buffer(chain.current_index())`.
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
  (43 tests: 10 infrastructure, 9 front, 11 device on the recorder, 1 cache, 1 on the GPU, 11
  unit; run 26: all pass, run26/test2.log). Put `set CARGO_INCREMENTAL=0&& ` before both commands (see the note further down).
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
- Everything at once (run 27): scratch `bin/full27.sh <n>` runs the crates' tests, every test
  file, the own program and the shader runner into run27/*<n>.log (about 40 minutes from a clean
  build); `bin/programs27.sh <n>` runs Godot's two platformers and bgfx 37 (copy them to a new
  run's folder name first, `sed s/run27/run28/`); `bin/rw27.sh <n>` lints, builds the libraries
  and runs named shader runner files; `bin/lt26.sh <name>` lints and runs the crates' tests;
  `bin/warm26.sh <log> <programs>` runs programs twice with a cache file (Real program).
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
- `SlowPath::SequentialBuffersCopied` (run 26, commit c024aa3 "interface: named the slow path
  of a sequential blit swap chain's buffers moved by copies"; plan 4.9 says it): the swap chain
  moving a sequential blit model's contents by copies at each present.
- `ShaderModule::from_wgsl(source, entry, stage)` (run 5, commit "interface: made a shader module
  from one entry point of a wgsl module..."): parse, validate and key one entry point of a WGSL
  module, as the built-ins are keyed. The infrastructure crate's present copy uses it instead of a
  builder of its own. G1's `helpers.rs` still has its own copy of the same steps (`module`/`program`);
  it could call this (G1 owns helpers.rs, so this track did not change it).

## Measurements

- Run 27 (2026-11-18, RTX 3080, desktop shared with other tracks, not `--alone`, load not
  taken; run27/check1.log after the merges, check2.log at 058f3e3): own program 25 of 25 cases
  equal both times; draw 0 of 4096 pixels beyond 1; features 253 of 872, outputs 22 of 110,
  answers 7 of 37 differ, none beyond real. Fence events, 2000 rounds of an empty queue's
  `Signal`, `SetEventOnCompletion` and `WaitForSingleObject`: check1 real first 7 µs, mean 1 µs,
  99th percentile 1 µs, largest 7 µs, ours first 77 µs, mean 17 µs, 99th percentile 21 µs,
  largest 77 µs; check2 real 7, 1, 1, 7 µs, ours 150, 17, 27, 150 µs. Test files job 05:34:17
  to 05:35:35 (78 s), shader runner 05:41:01 to 05:45:07 (246 s).
- Fence events, the same measurement in earlier runs (scratch run<N>/check*.log, same
  conditions): real mean 1 to 2 µs, 99th percentile 1 to 4 µs in every run; ours mean 17 to 24
  µs, 99th percentile 19 to 121 µs, largest 62 to 695 µs from run 7 on (runs 4 and 5: mean 33 to
  37 µs, first round 30 to 36 ms, the device's first submission). The gap is the boxed call to
  the device thread per COM call (Departures).
- Features differing from real: 493 of 872 in runs 4 and 5, 355 in run 7, 269 to 273 in runs 10
  to 17, 268 in run 18, 253 since run 19; none beyond real since run 10 (run 10 briefly answered
  `R1_UNORM`'s quality levels where real fails; fixed that run).
- Run 3: the triangle scene through `Device<WebGpu>` on wgpu's Vulkan and Direct3D 12 paths,
  largest difference 0 (desktop CPU load 14%). Run 4: the triangle and swapchain scenes and the
  own program equal real Direct3D 12 (largest difference 0, CPU load 13%).
- Line lists (measured by the independent check, its own program in scratch `check/own`, mode
  `lines`): 8 aliased `LINELIST` segments on 64x64, real against ours "59 of 4096 pixels beyond
  1, largest difference 255". A listed loss, below.
- Real programs' times: under "Real program".

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

Run 27's entry:
- Which storage formats a program may both read and write is WGSL's rule (`R32_FLOAT`,
  `R32_UINT`, `R32_SINT`), written in the front (`bind.rs`, `stores`) as the converter writes
  it (its `ReadWriteStorageFormat`): the interface's `FormatUse` has `STORAGE_READ` and
  `STORAGE_WRITE` but no read-write use. A `FormatUse::STORAGE_READ_WRITE` from the back end
  would serve both and back ends with more read-write formats; not added, since no back end
  has more today.

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
  Apache-2.0, as plan section 10 confirms); `blake3` 1 (1.8.7 in the lock; CC0-1.0 OR
  Apache-2.0 OR Apache-2.0 WITH LLVM-exception, read in its Cargo.toml; already in the
  workspace) for the cache file's name (run 26).
- Test only: `wcontainers-gpu-webgpu`, `wgpu` 30.0.2 (MIT OR Apache-2.0), `pollster` (MIT OR
  Apache-2.0), `png` (MIT OR Apache-2.0), as the WebGPU track already uses them.

## Installed on the desktop

Nothing of this track is installed on the desktop beyond the rig's setup. What stays there:
- The shaders crate's submodules, checked out in this branch's source directory
  (C:\wc\src\phase-2-graphics-direct3d-12\crates\wcontainers-shaders: dxbc-spirv and
  dxil-spirv, 18,052,177 bytes, run 4).
- In C:\wc\out\phase-2-graphics-direct3d-12: bgfx-run.ps1 (1 KB, run 16), bgfx-capture.ps1 (2
  KB) and bgfx-compare.ps1 (1 KB, run 21); bgfx's captured frames and logs under bgfx (about 12
  MB, bgfx's own art, kept as evidence); the rig's program and test outputs (programs, wine,
  check), which each run overwrites.
- The rig's build folder of this branch, C:\wc\target\phase-2-graphics-direct3d-12, is
  deleted at the end of each run (run 27: see below), so each run builds from clean (about 8
  minutes).

Run 27: nothing installed. Deleted at the end of the run: C:\wc\target\phase-2-graphics-direct3d-12.
No build folder was made on the Mac or in scratch; the logs stay in scratch run27.
