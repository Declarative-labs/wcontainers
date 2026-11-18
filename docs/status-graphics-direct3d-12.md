# Direct3D 12 and DXGI infrastructure status (G5)

Branch: phase-2-graphics-direct3d-12. Plan: docs/phase-2-graphics.md, sections 4.9, 5.2, 9, 10 and
G5 in 12. This track owns crates/wcontainers-directx-graphics-infrastructure and
crates/wcontainers-direct3d-12.

## Where things stand

G5 is not complete: the real program and a few cases wait on the shaders track (G6) and the
WebGPU back end (G2); see "Gap list" and "Next". Runs, all on 2026-11-17:
- Runs 1 to 4: the first slice (G5a). Run 5: Wine's and vkd3d's tests through ours, root signatures,
  depth-stencil targets. Run 6: compute, unordered access views, queue waits, occlusion queries,
  predication, heaps. Run 7: alias copies, `ExecuteIndirect`, the pass bar's cases. Runs 8 to 10:
  allocators and lists as Direct3D 12 checks them, resolves, twin textures, null descriptors,
  custom heaps, bundles, the newer interfaces (`ID3D12Device8`, `ID3D12GraphicsCommandList2`),
  every `CheckFeatureSupport` feature. Run 11: heaps kept alive by placed resources, root
  signature checksums, unknown target formats, pending clears, 256-byte buffer blocks.
- Run 12 (eighth run of G5): merged the shared line, rig and webgl branches (no conflicts; the
  shaders and webgpu branches had nothing new). Added: typed buffer UAV clears of half, packed
  (`R11G11B10`, `R10G10B10A2`) formats with the bits Direct3D 12 writes (half crate); 1D UAVs and
  UAV clears with rectangles (2D textures); vertex shaders converted with the pixel stage's
  interpolation (`VariantKey::output_interpolation`, as the Direct3D 11 front does); per-instance
  streams of step rate 0 drawn as stride 0 at the first instance's element; single-sampled 2D
  textures of a resolvable format made drawable so resolves can write them (WebGPU needs it of a
  resolve destination; the Direct3D 11 front does the same); transitions checked at `Close`
  (write states combined, no resource, the same state before and after: all E_INVALIDARG as real
  Direct3D 12); block-compressed copies not at a block's corner refused at `Close`. New case
  `predicated-resolve` equal to real Direct3D 12. Pipeline state streams take empty
  amplification and mesh shader subobjects; the shader runner then ran through ours for the
  first time (194 of 299).
- Run 13 (ninth run of G5): merged the shared line, the shaders and webgl branches (no
  conflicts; rig and webgpu had nothing new). The shaders branch now builds the variants
  `COUNTING`, `DUAL_SOURCE`, `samples` and `DrawBaseSource::Helpers`, which the front now uses:
  counted occlusion queries through the interface's `OcclusionCounter` (`QUERY_OCCLUSION` was
  E_NOTIMPL), `SV_VertexID`/`SV_InstanceID` with a first vertex, base vertex or first instance
  (the bases in the vertex program's helper block, `DrawBaseInHelpers`), dual-source blending
  (`SRC1` factors; refused with E_INVALIDARG with more than one target or another target
  blending, as real Direct3D 12), `GetRenderTargetSampleCount`. Comparison sampling of a texture
  stored as `R32_FLOAT`/`R16_UNORM` reads a depth twin (`Depth32Float`/`Depth16Unorm`) that
  inherits its writes (`DepthWrittenByDraw`). New case `occlusion-count`; all 15 cases equal to
  real Direct3D 12.
- Run 14 (tenth run of G5): merged the shaders, webgl and rig branches (no conflicts; the shared
  line and webgpu had nothing new). Added: pipeline statistics queries made by the front (plan
  4.12's other queries) and stream output statistics (zero: no stream output pipeline is made);
  `ExecuteIndirect` draws of programs reading `SV_VertexID`/`SV_InstanceID` read the bases from
  their arguments on the GPU (`DrawBaseSource::Arguments`, `DrawBaseFromArguments`) instead of
  reading the arguments back; UAV clears with rectangles on buffer views (element ranges) and
  3D texture views (copies from a cleared scratch texture); direct draws reading past a vertex
  buffer view's end draw that stream from a copy with a zero tail (`VertexFetchZeroed`); the
  device takes the LUID of the adapter `D3D12CreateDevice` was given, or of the first adapter
  the process's DXGI lists. New cases `statistics` and `vertex-past-end`; all 17 cases equal to
  real Direct3D 12. Test files 122 of 193 (query_pipeline_statistics newly passing); shader
  runner 204 of 299 (from 194: the runner now finds our device's vendor).
- Run 15 (eleventh run of G5): merged the shaders, webgl, rig and Direct3D 11 branches (no
  conflicts; the shared line and webgpu had nothing new). The shaders branch now builds pulled
  vertex inputs (`VariantKey::pulled`), which the front now uses. Added: vertex streams whose
  stride WebGPU refuses (not a multiple of 4, or an element reaching past it) and per-instance
  streams stepping every n instances (no `INSTANCE_STEP_RATE`) are fetched in the vertex
  program (`src/device/pull.rs`; `VertexPulledInShader`, `InstanceRateInShader`); element
  offsets not a multiple of the element's size (or 4) are refused with E_INVALIDARG, as real
  Direct3D 12 refuses them (measured); strips whose indices restart at another value than
  WebGPU's (16-bit indices with the cut off or 0xFFFFFFFF, 32-bit ones cut at 0xFFFF) are drawn
  from 32-bit indices a compute program rewrites (`RestartRewrite`); command lists are
  `ID3D12GraphicsCommandList4`, with render passes recorded as the targets, clears and
  resolves they stand for. New cases `vertex-pulled`, `restart` and `render-pass`; all 20 cases
  equal to real Direct3D 12. Test files 124 of 193 (instance_id and primitive_restart newly
  passing); shader runner 204 of 299 (unchanged). Every test file still failing stops first
  at the converter (G6), timestamps or `A8` (G2), the view swizzle (interface), or a gap on the
  list; none at a front refusal of its own.

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
  end counts the kept draws (`OcclusionCountReplay`, reading back and waiting); `EndQuery` writes
  the sum into the heap's count buffer by an ordered write, from which `ResolveQueryData` copies.
- Statistics (run 14, `src/device/statistics.rs`): pipeline statistics heaps and stream output
  statistics heaps keep a buffer of values the front writes at `EndQuery` (an ordered write),
  which `ResolveQueryData` copies from, as for counted occlusion. Input assembler, vertex shader
  and clipper counts come from each draw's arguments as a GPU without vertex reuse or clipping
  splits counts them (indexed strips with a cut value read their indices back, `ReadBackWait`;
  while a query runs, `ExecuteIndirect` reads its arguments back and predicates are read back);
  compute invocations are groups times the program's group size; pixel shader invocations come
  from a second occlusion counter (`pixels`) replaying each draw of a pipeline with a pixel
  shader with the depth and stencil tests off and a WGSL program counting once a pixel (the
  first covered sample on multisampled targets) or once a sample for pixel shaders run per
  sample. Geometry, hull and domain counts are zero (those stages are refused). Stream output
  statistics are zero: stream output pipelines are refused. Heap types 4, 5 and 7 (video
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
- Restarts (run 15): WebGPU restarts strips at 0xffff for 16-bit and 0xffffffff for 32-bit
  indices, always; Direct3D 12 at the pipeline's cut value. Indexed strip draws whose cut value
  differs from WebGPU's draw from a scratch buffer of 32-bit indices of the draw's range that a
  compute program makes (`RestartRewrite`): 16-bit ones widened, 32-bit ones with 0xffff made
  0xffffffff. `ExecuteIndirect` reads such draws' arguments back; predicated ones are refused by
  name. 32-bit indices of 0xffffffff with the cut off still restart (that vertex lies past any
  buffer Direct3D 12 can bind, so it reads zeros there).
- Refused with E_NOTIMPL and the reason printed (`wcontainers: <reason>: not implemented`): arrays
  of resources, component mappings other than identity, minimum level-of-detail
  clamps, level-of-detail bias, min/max filters, border colours and mirror-once without the back
  end's feature, alpha-antialiased lines, depth bias on lines and points, forced sample counts,
  conservative rasterisation, typed buffer views, UAV counters,
  timestamps (the WebGPU back end lacks `TIMESTAMPS`; `GetTimestampFrequency` answers E_FAIL as for
  a queue without them), stream output, rectangles in UAV clears of a 3D texture's view in another
  format than its own, an occlusion query across passes, video command lists, buffers in custom heaps the CPU
  sees, pipeline libraries (`DXGI_ERROR_UNSUPPORTED`), protected sessions, state objects (ray
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
compares the lines (`examples/check/cases.rs`). Run 15 (RTX 3080, scratch run15/check5.log, built
at d7c1de0): 20 of 20 equal. New: vertex-pulled (five pipelines, of which real Direct3D 12 and
ours both refuse the one with a 4-byte element at offset 2; a stride of 6 with an element at
offset 2, offsets stepping every 2 instances from instance 1, a per-instance stream of stride 1,
and a stride of 2 in an `ExecuteIndirect` draw of a program reading `SV_VertexID` from start
vertex 1 and instance 1); restart (strips of 16-bit and 32-bit indices holding 0xffff, with each
cut value, drawn directly and through `ExecuteIndirect`); render-pass (a pass clearing and
drawing, one keeping and drawing, and one clearing a 4-sample target and resolving it at its
end). Draw 0 of 4096 pixels beyond 1; features 269 of 872, outputs 22 of 110 and answers 7 of 25
differ, none beyond real Direct3D 12.
Run 14 (RTX 3080, scratch run14/check4.log, built
at 9937b23): 17 of 17 equal. New: vertex-past-end (a triangle strip whose fifth vertex lies past
a view of four, and two instances of which the per-instance view holds one; pixels equal).
Before it (check3.log, after f2894aa's arguments route): 16 of 16 equal. New: statistics (pipeline statistics around
nothing; around two draws, an indexed draw and a dispatch with a counted occlusion query inside;
around an indirect draw and a skipped predicated draw; a stream output statistics query; the counts
Direct3D leaves to the implementation printed as bounds: vertex shader invocations at most the
vertices read, clipper primitives above zero, pixel shader invocations at least the occlusion
count). indirect-vertex-id now also has a draw from first vertex 3 and first instance 1 and a count
buffer over three indexed commands, and is drawn through the arguments route (no read back).
Run 13 (RTX 3080, built at edcbd74 plus the new
case, scratch run13/check1.log): 15 of 15 equal. New: occlusion-count (five counted queries: one
draw, two draws with a rectangle clear between them, none, a degenerate triangle, two instances;
resolved in a second list into a default buffer, read back, and two of the counts used as
predicates for clears); indirect-vertex-id now equal (bases through the helper block).
Run 12 (built after b2ce9e4, scratch run12/check2.log): 13 of 14 equal.
- Equal: alias-copy (buffer written by a copy, read through an overlapping one), alias-storage
  (written by a root UAV), alias-cpu (upload heap, CPU write through one buffer, copy from the
  other), alias-texture (identical render targets at one offset), queues (copy and compute queues
  waiting on each other and on a CPU signal made after everything is submitted), predication
  (binary occlusion resolved into a default buffer, read back, four predicated draws and two
  predicated clears), indirect-draw, indirect-count, indirect-dispatch (counts 0 and 1),
  indirect-short (refused at `Close`), indirect-vertex-buffer (read-back route), depth-planes
  (footprints and plane copies of D24S8, D32S8, D32, D16), predicated-resolve (new in run 12: a
  multisampled clear resolved under a predicate that skips and one that lets it through, and a
  same-state transition refused at `Close`).
- Run 12 differed in indirect-vertex-id (indexed indirect draws with base vertex 3 of a shader
  reading `SV_VertexID`), equal since run 13.
- Not in the cases: timestamps (absent on this back end, see the gap list).
- Run 14, same run as check1: draw 0 of 4096 pixels beyond 1 (largest difference 0); features 269
  of 872, outputs 22 of 110 and answers 7 of 25 differ, none beyond real Direct3D 12.
- Run 13, same run: draw 0 of 4096 pixels beyond 1 (largest difference 0); features 269 of 872,
  outputs 22 of 110 and answers 7 of 25 differ, none beyond real Direct3D 12 (ours answers less:
  heap flags, colour spaces, formats WebGPU lacks).
- History: run 7 11 of 13 (depth-planes then differed on G2's 24-bit depth copies), runs 10 and
  11 12 of 13.

## Test files (Wine's and vkd3d's, through our libraries, RTX 3080)

How a file counts: the rig compares each test's verdict with real Direct3D 12's on this GPU
(`wine-tests run`, below); a test that skips what real Direct3D 12 runs, or runs fewer checks,
fails. The counts are tests passing on real Direct3D 12 that pass through ours.

Run 15 (2026-11-17, desktop shared with other tracks; scratch run15/full1.log to full3.log):
- After the merge (full1 did not reach the test files: an ssh failure; full2, at 20220f7 with
  the pulled inputs): the same 122 of 193 as run 14; instance_id then stopped at an indirect
  draw of a stream stepping every 2 instances, primitive_restart at 3 restart checks.
- full3 (at d7c1de0): 124 of 193, no test lost: d3d12 (Wine) 7 of 7, vkd3d-d3d12 106 of 174,
  invalid_usage 3 of 3, hlsl_d3d12 8 of 9. Newly passing: instance_id, primitive_restart.

Run 14 (2026-11-17, desktop shared with other tracks; scratch run14/full1.log, built at f2894aa):
122 of 193, no test lost: d3d12 (Wine) 7 of 7, vkd3d-d3d12 104 of 174, invalid_usage 3 of 3,
hlsl_d3d12 8 of 9. Newly passing: query_pipeline_statistics. quad_tessellation and
line_tessellation now get past their statistics queries and stop at hull shaders (the stage
helper, G6). After the rectangle clears (run14/named2.log): clear_unordered_access_view_image no
longer crashes; it runs 35726 checks, and its 24 failures are all `CheckFeatureSupport` E_FAIL for
the 16-bit packed formats WebGPU lacks (24 checks skipped for them). Last full run at 9937b23
(run14/full2.log): the same 122 of 193, the same failing set.

Run 13 (2026-11-17, desktop shared with other tracks; scratch run13/full1.log to full4.log):
- After the merge alone (full1): the same 116 of 193 as run 12.
- Counted occlusion and draw bases (full2, at edcbd74): 119 of 193; newly passing
  query_occlusion, resolve_query_data_in_different_command_list and
  resolve_query_data_in_reordered_command_list.
- Dual-source blending and the sample count (full3, f9fdf44): 120; newly
  shader_get_render_target_sample_count; dual_source_blending then failed only at three
  pipelines real Direct3D 12 refuses (dual-source with several targets).
- With that check and the depth twin (full4): 121 of 193, no test lost: d3d12 (Wine) 7 of 7,
  vkd3d-d3d12 103 of 174, vkd3d-d3d12_invalid_usage 3 of 3, vkd3d-hlsl_d3d12 8 of 9.

Run 12 (2026-11-17, desktop shared with other tracks; built at b2ce9e4; scratch run12/full1.log,
logs in run12/logs-full1):
- d3d12 (Wine): 7 of 7.
- vkd3d-d3d12: 98 of 174, none lost. Newly passing: clear_unordered_access_view_buffer,
  conditional_rendering, unused_interpolated_input, zero_vertex_stride.
- vkd3d-d3d12_invalid_usage: 3 of 3 (newly: invalid_resource_barriers, invalid_copy_texture_region).
- vkd3d-hlsl_d3d12: 8 of 9 (test_thread_id: `StorageImage`, G6).
- Second full run after the mesh subobject and comparison refusal commits (run12/full2.log, at
  18e6979): the same 116 of 193, the same failing set.
- vkd3d-shader_runner (302 files): 194 of 299 with our d3d12.dll alone ("Shader runner" below).

What the 71 failing vkd3d-d3d12 tests stop at first (run 13, full4; run13/fail-reasons2.txt is
the list before the last two fixes). Changes since run 12: counted occlusion (3),
dual_source_blending and shader_get_render_target_sample_count now pass; vertex_id now stops at
stream output (the stage helper); gather_c now stops at `DynamicOffset` after its comparison
draws (depth twin). The run 12 breakdown, with those changes, follows.
- The converter (G6), 36: `StorageImage` 13, `TypedBuffer` 6, `Atomics` 4, `StorageCounter` 3,
  `ClipDistance` 2 (clip_distance now stops at its geometry shader; cull distances refused by
  name), `RegisterSpace` 2, `DynamicOffset` 2 (gather, gather_c), and one each of
  `MultisampledArrayTexture`, `RenderTargetArrayIndex`, `InterpolateAt`, resource arrays.
- The stage helper (G6: geometry, hull and domain stages, stream output), 15, plus
  quad_tessellation and line_tessellation at pipeline statistics, and create_pipeline_state
  (1 of 38: stream output).
- vertex_id: stream output (the stage helper, G6).
- Timestamps (G2's `TIMESTAMPS`): create_query_heap, query_timestamp,
  resolve_non_issued_query_data, clock_calibration. (query_pipeline_statistics passes since run
  14; quad_tessellation and line_tessellation now stop at hull shaders.)
- A8_UNORM (G2's `FormatConverted`): render_a8, sample_instructions.
- (Run 15: instance_id and primitive_restart pass, through pulled streams and restart rewrites.)
- View swizzles (interface and G6): depth_stencil_sampling (stencil views), srv_component_mapping.
- Formats WebGPU lacks (`B5G6R5`, `B5G5R5A1`, `B4G4R4A4`; `CheckFeatureSupport(FORMAT_SUPPORT)`
  answers E_FAIL where real answers S_OK): clear_unordered_access_view_image (since run 14 its
  only failures).
- Multisampled array targets (absent in WebGPU): clear_render_target_view (3 of 273 checks).
- Vertices past the end of the bound vertex buffer in `ExecuteIndirect` (WebGPU's indirect
  validation skips such a draw; Direct3D 12 reads zeros): execute_indirect (2 checks). Exact on
  the GPU only with vertices fetched in the program (G6's pulled streams) or by reading every
  indirect draw's arguments back; see the gap list.
- Reported absent as Direct3D 12 expects, the test then skipping or running fewer checks (final
  gap list): create_reserved_resource, get_resource_tiling (tiled resources),
  create_graphics_pipeline_state and output_merger_logic_op (logic operations),
  shader_instructions (float64), unbounded_resource_arrays, unbounded_samplers (binding tier 2).

History (tests passing on real Direct3D 12 that pass through ours, vkd3d-d3d12 of 172 or 174):
run 5 54, run 6 66, run 7 80, run 8 87, run 9 74 (the rig's stricter rule began), run 10 87,
run 11 94, run 12 98, run 13 103, run 14 104, run 15 106. Wine's d3d12: 6 of 7 until run 9, then 7 of 7. invalid_usage 1 of 3 until
run 12. hlsl_d3d12 8 of 9 throughout.

## Shader runner (vkd3d-shader_runner, 302 files)

Run with our d3d12.dll alone (the system's dxgi.dll): with our dxgi.dll too every file crashes
in its Direct3D 11 part before reaching Direct3D 12 (our dxgi.dll serves only Direct3D 12), 0 of
299 (run12/runner1.log). Command: `E='%CARGO_TARGET_DIR%\debug\examples';
tools/graphics-rig/wine-tests run --dll "d3d12.dll=$E\d3d12.dll" vkd3d-shader_runner`.

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

Command: `tools/graphics-rig/programs run --dll "d3d12.dll=%CARGO_TARGET_DIR%\debug\examples\d3d12.dll"
--dll "dxgi.dll=%CARGO_TARGET_DIR%\debug\examples\dxgi.dll" godot-platformer-2d-d3d12`.
- Run 10 (build after a0760963): FAIL, exit 0xc0000005 after 7.1 s, 1 frame, 0 of 3 frames
  saved. Godot makes the device ("D3D12 11_0 - Forward+ - Using Device #0: NVIDIA - NVIDIA
  GeForce RTX 3080") and stops at the converter only (G6): its 2D canvas vertex shaders fail with
  `BindingsBeyondLimit` (7) and "unsupported image dimension %5" (buffer images, 6), so 13
  `CreateGraphicsPipelineState` calls answer E_NOTIMPL; compute shaders with storage images of no
  format fail ("image read/write without format is not currently supported", naga's SPIR-V
  reader); timestamps are absent (G2). Godot then draws with null pipelines and crashes in its
  own code. Run 9 had stopped earlier (a heap flag and D16 depth views, both fixed in run 10).
- Runs 11 to 13: not run again. The shaders branch still has none of those converter features
  (`StorageImage`, `BindingsBeyondLimit` and `DrawBaseFromArguments` are refused by name in
  crates/wcontainers-shaders/src/lower.rs and dxil.rs at origin/phase-2-graphics-shaders as of
  run 13), so the run would end as in run 10.

## Gap list (the draft of the pass bar's written list, run 14)

Each line is an absent feature behind failing tests, and who provides it. A gap belongs on the final
list only when it is the back end's or browser's to lack (reported absent as Direct3D 12 expects).
- Absent in WebGPU, reported absent: timestamps
  where the adapter lacks `timestamp-query` (`GetTimestampFrequency` and `GetClockCalibration`
  answer E_FAIL); logic operations (`OutputMergerLogicOp` false); conservative rasterisation,
  raster order views, tiled resources, ray tracing, mesh shaders, variable rate shading, sampler
  feedback, float64, resource binding tier 2 and descriptor arrays where the back end lacks big
  tables, `SV_RenderTargetArrayIndex` from the vertex stage (all reported absent in
  `CheckFeatureSupport`); multisampled array textures; the 16-bit packed formats `B5G6R5`,
  `B5G5R5A1`, `B4G4R4A4` (format support E_FAIL until a `FormatConverted` route exists).
- Absent in WebGPU, served by slow paths still to write: one-dimensional textures in shaders
  (stored as 2D; UAV clears done, bindings refused).
- Absent in WebGPU, a listed loss: `ExecuteIndirect` draws reading vertices past the end of a
  vertex buffer (WebGPU skips the whole draw where Direct3D 12 reads zeros; execute_indirect, 2
  checks). Pulled streams (run 15) would make it exact, but only by fetching every stream of
  every indirect draw in the program, since nothing says ahead which draws reach past the end;
  that slows every indirect draw for a case only faulty programs reach, so it is not done.
- G6 (converter): `StorageImage`, `TypedBuffer`, `Atomics`, `StorageCounter`, `RegisterSpace`,
  `DynamicOffset`, `CullDistance`, `RenderTargetArrayIndex`, `InterpolateAt`,
  `MultisampledArrayTexture`, the stage helper (geometry, hull, domain, stream output), and for
  the real program
  `BindingsBeyondLimit`, buffer images and storage images of no format. (Run 13: the counting
  variant, dual source, the sample count and the draw bases landed and are used; run 15: pulled
  vertex streams.)
- G2 (WebGPU back end): `TIMESTAMPS`; `A8_UNORM` (`FormatConverted`).
- Interface and G6 (asked by the Direct3D 11 track too): a view swizzle, with a program rewrite as
  the default. Direct3D's stencil views give the stencil in the second component, WebGPU's
  stencil-aspect views in the first; the front refuses stencil views by name until then, and
  component mappings need the same.
- Front, this milestone: buffers in CPU-visible custom heaps, `ID3D12GraphicsCommandList5` and
  later (variable rate shading, absent; enhanced barriers); comparison sampling of a texture whose bits match no depth format
  (other than `R32_FLOAT`/`R16_UNORM`) and of a multisampled one (refused by name). Not tested:
  pipeline statistics of draws with no target bound (the pixel counter reads the targets).
  (Run 14: statistics queries, the arguments route of `ExecuteIndirect` and rectangle UAV clears
  of buffers and 3D textures landed. Run 15: pulled streams, restart rewrites, render passes.)

## Next, in this order

1. The real program: godot-platformer-2d-d3d12 waits on G6 (`BindingsBeyondLimit`, buffer
   images, storage images of no format). At the start of each run, merge the shaders branch and
   grep crates/wcontainers-shaders/src for `unsupported("StorageImage")` and
   `BindingsBeyondLimit`; once gone, run Godot (command under "Real program") and compare its
   frames with the rig's references; also try godot-platformer-3d-d3d12.
2. Vertices past the end in `ExecuteIndirect` (execute_indirect): a listed loss (gap list).
3. Timestamps against real Direct3D 12 (a new case) when G2's `TIMESTAMPS` lands; A8 when
   `FormatConverted` lands. A counted query whose draws write depth is not in the own cases
   (vkd3d's query_occlusion covers it).
4. The stencil view and component mapping route once the interface has a view swizzle.
5. The shader runner: its fewer-check files come from the shader model answer (6.0, plan
   section 9); its wrong values are in converted shaders (G6). Nothing left on the front's side
   that is known.
6. Turn the gap list into the pass bar's final written list once the G6 and G2 items land.
7. Later: `ID3D12GraphicsCommandList5` and later, `ID3D12Fence1`; render pass resolves of part
   of a subresource and of depth-stencil targets; recording without allocating
   per command, the call buffer of plan 4.8; moving `examples/directx` into
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
  (33 tests: 9 infrastructure, 9 front, 10 device on the recorder, 1 on the GPU, 4 unit; run 15:
  all pass at d7c1de0, run15/full3.log). Put `set CARGO_INCREMENTAL=0&& ` before both commands (see the note further down).
  `WEBGPU_API=dx12|vulkan` picks wgpu's path for the GPU test.
- Lint: `tools/graphics-rig/remote "cargo clippy -j 3 --all-targets -p
  wcontainers-directx-graphics-infrastructure -p wcontainers-direct3d-12 -- -D warnings"`.
- The desktop needs the shaders crate's submodules once (the rig's `remote` sends none):
  `tools/graphics-rig/remote "git submodule update --init crates/wcontainers-shaders/dxbc-spirv
  crates/wcontainers-shaders/dxil-spirv && git -C crates/wcontainers-shaders/dxil-spirv submodule
  update --init third_party/spirv-headers"` (done in run 4; they stay checked out).
- Scenes: `E='%CARGO_TARGET_DIR%\debug\examples'; tools/graphics-rig/scenes --build "cargo build -j 3
  -p wcontainers-direct3d-12 --examples" --dll "d3d12.dll=$E\d3d12.dll" --dll "dxgi.dll=$E\dxgi.dll"
  --api d3d12 triangle swapchain` (drop the names for every scene).
- Own program: `tools/graphics-rig/gpu-job --desktop --build "cargo build -j 3 -p
  wcontainers-direct3d-12 --examples" -- "crates\wcontainers-direct3d-12\examples\check_d3d12.cmd"`.
  It runs `draw`, `features`, `outputs`, `answers`, `cases` and `fence-wait 2000` beside real
  Direct3D 12 and beside our libraries (in C:\wc\out\phase-2-graphics-direct3d-12\check), then compares. `--desktop`
  matters: in session 0 real DXGI refuses outputs and swap chains
  (`DXGI_ERROR_NOT_CURRENTLY_AVAILABLE`).
- Wine's and vkd3d's tests through our libraries (build first; about 8 minutes):
  `tools/graphics-rig/remote "set CARGO_INCREMENTAL=0&& cargo build -j 3 -p wcontainers-direct3d-12 --examples"`, then
  `E='%CARGO_TARGET_DIR%\debug\examples'; tools/graphics-rig/wine-tests run --dll "d3d12.dll=$E\d3d12.dll"
  --dll "dxgi.dll=$E\dxgi.dll" d3d12 vkd3d-d3d12 vkd3d-d3d12_invalid_usage vkd3d-hlsl_d3d12`
  (name tests as `vkd3d-d3d12:test_a/test_b`). Logs land in
  pyro-parked/wcontainers-scratch/graphics-rig/wine-logs; `grep -o 'wcontainers: [^,(]*'` over them
  counts the refusals. `vkd3d-shader_runner` (302 files) runs the same way.
- The own program now also runs `answers` (alpha modes, a 16-bit float swap chain's presents, a
  fence waited for with no event, committed resources with each heap flag), compared like features.
- `com.rs`: `python3 examples/directx/com.py > examples/directx/com.rs`, then `cargo fmt` (it finds
  the windows crate under `$CARGO_HOME`, default `~/.cargo`).
- On the Mac only `nix develop -c cargo fmt` (the Mac is overloaded).
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
- `ShaderModule::from_wgsl(source, entry, stage)` (run 5, commit "interface: made a shader module
  from one entry point of a wgsl module..."): parse, validate and key one entry point of a WGSL
  module, as the built-ins are keyed. The infrastructure crate's present copy uses it instead of a
  builder of its own. G1's `helpers.rs` still has its own copy of the same steps (`module`/`program`);
  it could call this (G1 owns helpers.rs, so this track did not change it).

## Measurements

- Run 15 (2026-11-17, RTX 3080, desktop shared with other tracks, not `--alone`, load not taken;
  run15/check5.log): fence events, 2000 rounds: real first 8 µs, mean 1 µs, 99th percentile
  1 µs, largest 8 µs; ours first 76 µs, mean 18 µs, 99th percentile 57 µs, largest 103 µs.

- Run 14 (2026-11-17, RTX 3080, desktop shared with other tracks, not `--alone`, load not taken;
  run14/check1.log): fence events, 2000 rounds: real first 8 µs, mean 1 µs, 99th percentile
  2 µs, largest 20 µs; ours first 89 µs, mean 18 µs, 99th percentile 49 µs, largest 89 µs.

- Run 13 (2026-11-17, RTX 3080, desktop shared with other tracks, not `--alone`, load not taken;
  run13/check1.log): fence events, 2000 rounds: real first 8 µs, mean 1 µs, 99th percentile
  2 µs, largest 13 µs; ours first 45 µs, mean 19 µs, 99th percentile 60 µs, largest 162 µs.

- Run 12 (2026-11-17, RTX 3080, desktop shared with other tracks, not `--alone`, load not taken;
  run12/check2.log): fence events, 2000 rounds: real first 9 µs, mean 1 µs, 99th percentile
  2 µs, largest 9 µs; ours first 63 µs, mean 18 µs, 99th percentile 30 µs, largest 64 µs.

- Run 11 (2026-11-17, RTX 3080, desktop shared with other tracks, not `--alone`, load not taken):
  fence events, 2000 rounds: real first 8 µs, mean 1 µs, 99th percentile 2 µs, largest 29 µs;
  ours first 90 µs, mean 23 µs, 99th percentile 68 µs, largest 108 µs.

- Run 10 (2026-11-17, RTX 3080, desktop shared with other tracks, not `--alone`, load not taken):
  fence events, 2000 rounds: real first 11 µs, mean 2 µs, 99th percentile 3 µs, largest 11 µs;
  ours first 57 µs, mean 22 µs, 99th percentile 32 µs, largest 151 µs. Not timings: own program
  draw 0 of 4096 pixels beyond 1 (largest difference 0); outputs 22 of 110 and answers 7 of 25
  differ, none beyond real; features 273 of 872 differ, and 4 were beyond real (`R1_UNORM`'s
  quality levels, answered where real fails). After the fix (scratch run10/check2.log): features
  269 of 872 differ, none beyond real; fence events real mean 2 µs, 99th percentile 4 µs; ours
  mean 22 µs, 99th percentile 30 µs, largest 213 µs; the cases as before (12 of 13).

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
- Format support reports only what the back end does natively. The plan's routes for the rest
  (typed buffer views through storage, stream output, `NormAsInteger`, `FormatConverted`,
  `BlockDecoded`, `FilterInShader`, `ReinterpretCopy`) are not in the front yet, so on WebGPU those
  abilities are absent: `BUFFER`, `SO_BUFFER`, `CAST_WITHIN_BIT_LAYOUT` (without
  `FORMAT_REINTERPRET`), 3D block-compressed textures, filtering of depth views, and formats WebGPU
  lacks (16-bit normalised, `B5G6R5`, `B5G5R5A1`, `A8`, the packed pairs, the video formats).
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
  heaps hold textures stored as default ones. Their buffers are refused with E_NOTIMPL ("buffers in
  a custom heap the CPU sees": the bytes would move both ways). Such textures `Map` with no pointer.
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
- Windows library only (dev-dependencies): `windows` and `windows-core` 0.62.2 (MIT OR
  Apache-2.0, as plan section 10 confirms).
- Test only: `wcontainers-gpu-webgpu`, `wgpu` 30.0.2 (MIT OR Apache-2.0), `pollster` (MIT OR
  Apache-2.0), `png` (MIT OR Apache-2.0), as the WebGPU track already uses them.

## Installed on the desktop

Nothing beyond the rig's setup. The shaders crate's submodules are checked out in this branch's
source directory there (C:\wc\src\phase-2-graphics-direct3d-12\crates\wcontainers-shaders: dxbc-spirv
and dxil-spirv, 18,052,177 bytes).
