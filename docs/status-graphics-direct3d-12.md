# Direct3D 12 and DXGI infrastructure status (G5)

Branch: phase-2-graphics-direct3d-12. Plan: docs/phase-2-graphics.md, sections 4.9, 5.2, 9, 10 and
G5 in 12. This track owns crates/wcontainers-directx-graphics-infrastructure and
crates/wcontainers-direct3d-12.

## Where things stand

The first slice (G5a, runs 1 to 4) is done. Run 5 (2026-11-17) was the first run on the whole
milestone G5: the check's nine findings, Wine's and vkd3d's tests through ours for the first time,
root signatures and depth-stencil targets. Run 6 (2026-11-17, second run of G5) merged the shared
line, the webgpu and the shaders branches (no conflicts), and added compute pipelines and
`Dispatch`, unordered access views (storage buffers and textures, root UAVs and SRVs, raw and
structured buffer views), `ClearUnorderedAccessView*`, queue `Wait` with work held and released by
fences in call order, query heaps with binary occlusion, `ResolveQueryData`, `SetPredication`
(predicated draws and dispatches made indirect), root signatures carried in shaders, heaps and
placed resources that do not overlap, and fence events set before a CPU `Signal` returns. Run 7
(2026-11-17, third run of G5) merged the shared line, webgpu and shaders branches again (no
conflicts) and added alias copies for overlapping placed resources, `CreateCommandSignature` and
`ExecuteIndirect`, depth and stencil plane footprints and copies, draws with no render target, the
own program's pass-bar cases against real Direct3D 12 (`check_d3d12 cases`), and a batch of
validation answers vkd3d's tests check. Run 8 (2026-11-17, fourth run of G5) merged the shared
line (with the webgpu, webgl and direct3d-11 work in it) and the shaders branch (no conflicts),
and added: allocators used by one open list at a time (`Reset` E_FAIL while a list records from
it, a second list E_INVALIDARG), lists reset only when closed, an open list executed removing the
device with `DXGI_ERROR_INVALID_CALL`, creation after removal answering
`DXGI_ERROR_DEVICE_REMOVED`, equal root signature blobs giving one object, root signature
register checks, small (4 KiB) and multisampled (64 KiB) placement alignments and the refusal of
others, and `ResolveSubresource` (commit 3405674, tested). Run 8's last changes, written while
the desktop refused ssh, were kept as a patch. Run 9 (2026-11-17, fifth run of G5) merged the
shared line and the rig branch (no conflicts; webgpu, webgl, direct3d-11 and shaders had nothing
new), built and tested that patch and committed it (d89ad8d: twin textures for views
reinterpreting a typeless texture's bits, the frame latency waitable object, multisampled render
target views, three device-thread panics fixed, `GetClockCalibration` E_FAIL), then added null
descriptors, custom heaps, `WriteToSubresource`/`ReadFromSubresource`, Direct3D 12's creation
checks, bundles and the back buffers' shared reference on their swap chain (deee187 and after).
New ssh logins to the desktop failed all through run 9 (see Departures for how the rig was
reached). Run 10 (2026-11-17, sixth run of G5) merged the shared line (the interface's stated
contracts for samplers, binding sizes, one viewport a draw and empty passes), the shaders and the
webgl branches (no conflicts), and added: the newer interfaces (`ID3D12Device1` to
`ID3D12Device8`, `ID3D12GraphicsCommandList1` and `2`, `ID3D12Resource1` and `2`) with pipeline
state streams, waits on several fences, `WriteBufferImmediate`, `CreateCommandList1`, the `...1`
and `...2` creation calls, `GetResourceAllocationInfo1`/`2`, no meta commands, residency calls;
answers for every `CheckFeatureSupport` feature up to 34; quality levels as Direct3D 12 answers
them; typeless R16/R32 textures allowing depth stored in depth formats; default depth views of
every array slice; root signature deserializers giving null for empty arrays; and the
`ALLOW_SHADER_ATOMICS` heap flag. New ssh logins still failed (Departures). G5 is not complete:
see "Test files", "Gap list" and "Next".

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
  `INDIRECT_FIRST_INSTANCE`, and programs reading `SV_VertexID`/`SV_InstanceID` read the arguments
  back and record ordinary calls (`IndirectReadBack`). Argument buffers shorter than
  offset + max × stride fail `Close` with E_INVALIDARG, as real Direct3D 12 does (measured).
- Depth-stencil planes (run 7): D24S8 and D32S8 have two planes of subresources; footprints give
  plane 0 as `R32_TYPELESS` (4 bytes) and plane 1 as `R8_TYPELESS`, rows aligned to 512 (real
  Direct3D 12 on this desktop does so for planar formats only; single-plane depth keeps 256), and
  copies move the plane's aspect.
- Draws with no render target (run 7) open an empty pass as large as the viewports, at the
  pipeline's sample count; a viewport change ends it.
- Refused with E_NOTIMPL and the reason printed (`wcontainers: <reason>: not implemented`): arrays
  of resources, component mappings other than identity, minimum level-of-detail
  clamps, level-of-detail bias, min/max filters, border colours and mirror-once without the back
  end's feature, alpha-antialiased lines, depth bias on lines and points, forced sample counts,
  conservative rasterisation, typed buffer views, UAV counters, counted occlusion queries,
  timestamps (the WebGPU back end lacks `TIMESTAMPS`; `GetTimestampFrequency` answers E_FAIL as for
  a queue without them), pipeline and stream output statistics, stream output, rectangles in UAV
  clears, an occlusion query across passes, video command lists, buffers in custom heaps the CPU
  sees, pipeline libraries (`DXGI_ERROR_UNSUPPORTED`), protected sessions, state objects (ray
  tracing), depth bounds, programmable sample positions, view instancing, sampler feedback.
- Newer interfaces (run 10): the device is `ID3D12Device8`, command lists
  `ID3D12GraphicsCommandList2`, resources `ID3D12Resource2`; `ID3D12Device9` and later and
  `ID3D12GraphicsCommandList3` and later are not offered (E_NOINTERFACE). Pipeline state streams
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

## The pass bar's cases against real Direct3D 12 (own program, runs 7 and 10)

Run 10 (RTX 3080, commit after a0760963, scratch run10/check.log): 12 of 13 equal. depth-planes is
now equal (the webgpu branch reads 24-bit depth by draws, "depth read back through a draw"); only
indirect-vertex-id differs, as below (G6's `DrawBaseFromArguments`).

Run 7 follows.

`check_d3d12 cases` runs each case in its own process with real Direct3D 12 and through ours and
compares the lines (`examples/check/cases.rs`). Run 7, RTX 3080, commit 7f5e0ed: 11 of 13 equal.
- Equal: alias-copy (buffer written by a copy, read through an overlapping one), alias-storage
  (written by a root UAV), alias-cpu (upload heap, CPU write through one buffer, copy from the
  other), alias-texture (identical render targets at one offset, cleared through one), queues (copy
  and compute queues waiting on each other and on a CPU signal made after everything is submitted,
  with the upload written late; neither fence moved before the signal), predication (binary
  occlusion of a visible and a degenerate draw resolved into a default buffer, copied to readback,
  and four predicated draws and two predicated clears), indirect-draw, indirect-count,
  indirect-dispatch (with counts 0 and 1), indirect-short (refused at Close), indirect-vertex-buffer
  (read-back route).
- Differ: indirect-vertex-id (indexed indirect draws with base vertex 3 of a shader reading
  `SV_VertexID`): ours removes the device, "SV_VertexID or SV_InstanceID in a draw with a first
  vertex or instance"; needs the converter's `DrawBaseFromArguments` (G6). depth-planes: footprints
  equal, D32S8 and both stencil planes equal, the D24S8 depth plane copied into a buffer reads zeros
  on ours: the WebGPU back end prints "copies of 24-bit depth into a buffer (DepthReadByDraw) are not
  built yet" and writes nothing (G2's, and it reports success where it should fail).
- Not in the cases: counted occlusion and timestamps (absent, see the gap list); the plan's bar
  needs them resolved, copied and used by SetPredication against real Direct3D 12.
- Resources without overlaps take no `AliasCopied`: the warnings show it only in the alias cases,
  and the recorder test `placed_resources_lie_in_their_heap_and_overlapping_buffers_inherit_bytes`
  counts it.

## Test files (Wine's and vkd3d's, through our libraries, RTX 3080)

How a file counts: the rig compares each test's verdict with real Direct3D 12's on this GPU
(`wine-tests run`, below). "Real" counts tests that pass on real Direct3D 12. The rig's baseline now
counts 174 vkd3d-d3d12 tests passing on real Direct3D 12 (172 before).

The rig's rule is stricter since its fifth run (merged in run 9): a test that skips what real
Direct3D 12 runs, or runs fewer checks, fails. Thirteen tests counted as passing in run 8 fail by
that rule alone (they skip at an absent interface or feature: see the gap list); they are not
regressions.

Run 10 (2026-11-17, RTX 3080, desktop shared with other tracks; scratch run10/). At the start
(merged tree, run10/wine1.log): vkd3d-d3d12 75 of 174. Full run after the newer interfaces and
the window association (run10/full2.log), tests passing on real Direct3D 12 that pass through ours:
- d3d12 (Wine): 7 of 7.
- vkd3d-d3d12: 87 of 174, none lost. Newly passing: clear_depth_stencil_view,
  create_command_list, create_committed_resource, create_heap, depth_read_only_view,
  enumerate_meta_commands, get_copyable_footprints, multi_fence_event,
  multisample_quality_levels, resource_allocation_info, sample_c_lz, write_buffer_immediate.
- vkd3d-d3d12_invalid_usage: 1 of 3; vkd3d-hlsl_d3d12: 8 of 9 (test_thread_id: StorageImage).
- After that run, on named tests only (run10/named4.log, with draws into default views checked
  again: draw_instanced, multiple_render_targets, cube_maps, set_render_targets and others still
  pass): create_committed_resource passes with the protected session answer;
  clear_render_target_view fails 3 of 273 (multisampled array targets, absent in WebGPU).
What the 87 failing vkd3d-d3d12 tests stop at first (run10/fail-reasons.txt):
- The converter (G6), 34: StorageImage, TypedBuffer, Atomics, StorageCounter, RegisterSpace,
  ClipDistance and the rest of run 9's list.
- Geometry, hull and domain stages and stream output (G6's stage helper), 15.
- Counted occlusion queries (G6's counting variant), 3: query_occlusion,
  resolve_query_data_in_different/reordered_command_list.
- Timestamps (G2), 3; pipeline statistics (absent in WebGPU), 3; logic operations (absent), 1;
  dual-source blending (G6), 1.
- WebGPU lacks instance step rates and zero strides (instance_id, zero_vertex_stride): slow paths
  still to write.
- Front refusals: component mappings (srv_component_mapping), typed buffer clears of some formats
  (clear_unordered_access_view_buffer), UAV clears of a descriptor (clear_unordered_access_view_image).
- No refusal printed, still to find: execute_indirect, conditional_rendering,
  64kb_texture_alignment, check_feature_support (380 of 381 checks), clear_render_target_view,
  clock_calibration, create_placed_resource (heap corruption 0xc0000374), depth_stencil_sampling
  (28 checks), primitive_restart, render_a8, root_signature_byte_code (124: the deserializer must
  not answer `IUnknown`, and pointers to compare), root_signature_limits, sample_instructions,
  unknown_rtv_format, unknown_dsv_format, unused_interpolated_input, create_pipeline_state (no
  topology type; stream output).
- Reported absent, the test skipping (final gap list): create_reserved_resource,
  get_resource_tiling, output_merger_logic_op, shader_instructions (float64),
  unbounded_resource_arrays, unbounded_samplers.

Run 9 (2026-11-17, full run after deee187, RTX 3080; scratch run9/wine2.log), tests passing on
real Direct3D 12 that pass through ours:
- d3d12 (Wine): 7 of 7 (test_swapchain_refcount now passes).
- vkd3d-d3d12: 74 of 174 by the stricter rule. Newly passing in run 9: null_cbv, null_srv,
  bundle_state_inheritance, cube_maps, copy_texture_buffer, multisample_rendering, sample_mask, fractional_viewports, dispatch_zero_thread_groups.
- vkd3d-d3d12_invalid_usage: 1 of 3; vkd3d-hlsl_d3d12: 8 of 9 (test_thread_id: StorageImage).
- 49 tests crash, all after one of our refusals (the tests then use an object they did not get);
  none panicked in our code.
- After that run (not yet in a full run): `CheckFeatureSupport` past feature 34, more mip levels
  than the chain, and `WriteToSubresource`/`ReadFromSubresource` that do the work. Run on named
  tests only (run9/wine3.log): read_write_subresource and map_resource pass; create_heap and
  create_committed_resource fail no check but run 191 of 192 and 80 of 85; create_placed_resource,
  null_uav (StorageImage) and gather still crash; clear_render_target_view fails 7 of 273.
What the 100 failing vkd3d-d3d12 tests stop at first (one test can have several):
- The converter (G6), 30: StorageImage (10, with hlsl's thread_id), TypedBuffer (3, plus buffer_srv
  and uav_load), Atomics (4), StorageCounter (3), ClipDistance (2), SampleCount,
  MultisampledArrayTexture, InterpolateAt, RegisterSpace, dual-source blending's second output.
- Geometry, hull and domain stages and stream output (the stage helper, G6): 12, and 5 that skip
  at "stream output is not supported".
- Interfaces not offered (front): `ID3D12Device1` (multi_fence_event), `ID3D12Device2`
  (create_pipeline_state), `ID3D12Device5` (enumerate_meta_commands), `ID3D12Device8`
  (resource_allocation_info), `ID3D12GraphicsCommandList2` (write_buffer_immediate).
- Reported absent as Direct3D 12 expects, so the test skips (final gap list): tiled resources
  (create_reserved_resource, get_resource_tiling), logic operations (output_merger_logic_op),
  `SV_RenderTargetArrayIndex` from the vertex stage (ps_layer), resource binding tier 2
  (unbounded_resource_arrays, unbounded_samplers), descriptor arrays (resource_arrays), float64
  (shader_instructions).
- Absent in WebGPU and refused: timestamps (query_timestamp, resolve_non_issued_query_data), pipeline
  statistics (query_pipeline_statistics), logic operations in a pipeline
  (create_graphics_pipeline_state).
- Device removed at replay by a refusal: occlusion queries counted or across passes
  (query_occlusion, resolve_query_data_in_different/reordered_command_list), a reinterpreting
  view of a depth texture without a twin (sample_c_lz).
- Front, still to find (no refusal first): check_feature_support, clear_depth_stencil_view,
  clock_calibration, create_command_list, create_placed_resource (crashes with heap corruption,
  0xc0000374), multisample_quality_levels, render_a8, root_signature_byte_code,
  root_signature_limits, copy_descriptors, execute_indirect, primitive_restart,
  64kb_texture_alignment, gather and gather_c, conditional_rendering, bufinfo_instruction,
  clear_render_target_view (7 of 273 checks), unknown_rtv_format and
  clear_unordered_access_view_image (a list's `Close` fails), instance_id and zero_vertex_stride
  (instance step rates and zero strides on WebGPU).

Run 8 (2026-11-17, commit 3405674, the first full run of this run), tests passing on real
Direct3D 12 that pass through ours:
- d3d12 (Wine): 6 of 7 (test_swapchain_refcount).
- vkd3d-d3d12: 87 of 174 (run 7: 80, 81 expected). Newly passing: reset_command_allocator,
  device_removed_reason, create_root_signature, resource_allocation_info, map_placed_resources and
  others the refusals no longer stop.
- vkd3d-d3d12_invalid_usage: 1 of 3; vkd3d-hlsl_d3d12: 8 of 9 (test_thread_id).
- Not yet run through ours: the twin textures, the waitable object, the panic fixes and the
  multisampled texture usage made after it (see Next 1).
Why the 87 vkd3d-d3d12 tests still fail (first refusal or error in each log; one test can have
several):
- The converter (G6), 38 tests: StorageImage (11), TypedBuffer (7), Atomics (6), StorageCounter
  (2), RegisterSpace (2), DynamicOffset (2), ClipDistance (2), and single others (SampleCount,
  RenderTargetArrayIndex, InterpolateAt).
- Geometry, hull and domain stages and stream output (the stage helper, G6), 16 tests.
- Front refusals: bundles (`ExecuteBundle`), null descriptors (null_cbv, null_srv, null_uav),
  custom heaps and the sharing and display heap flags (create_heap, create_placed_resource,
  read_write_subresource), `WriteToSubresource`, logic operations without `LOGIC_OP`
  (create_graphics_pipeline_state), `GetClockCalibration`, timestamps and pipeline statistics
  queries (create_query_heap, query_timestamp, query_pipeline_statistics,
  resolve_non_issued_query_data), one-dimensional textures, dual-source blending.
- WebGPU refusing what the front passed on (each a front bug or a missing slow path, to be
  refused honestly or served): block-compressed textures whose top level is not a multiple of 4
  (create_committed_resource), vertex strides that are not a multiple of 4 (primitive_restart),
  an interpolation mismatch between stages (unused_interpolated_input), depth views of textures
  stored as `r32float` (depth_read_only_view, sample_c_lz, depth_stencil_sampling).
- Vertices past the bound vertex buffer in `ExecuteIndirect` (execute_indirect, 2 checks): WebGPU
  skips such an indirect draw; Direct3D 12 reads zeros and draws the rest.
- Panics of the device thread (fixed after this run, not yet re-run): a cube array view of -1
  cubes (cube_maps), an inverted copy box (copy_texture_buffer), a pipeline state released before
  its list ran (bufinfo_instruction).

Run 7 (2026-11-17, commit 7f5e0ed, before the empty pass), tests passing on real Direct3D 12 that
pass through ours:
- d3d12 (Wine): 6 of 7 (test_swapchain_refcount).
- vkd3d-d3d12: 80 of 174 (start of run 7: 71). Newly passing in run 7: create_device,
  check_feature_support, create_command_allocator, create_command_list, create_command_queue,
  create_descriptor_heap, object_interface, map_resource, get_copyable_footprints, and after the
  empty pass (commit dc040d9, run alone) map_placed_resources.
- vkd3d-d3d12_invalid_usage: 1 of 3; vkd3d-hlsl_d3d12: 8 of 9 (test_thread_id).
- Rig scenes through ours: 20 outputs pass, all equal to real Direct3D 12 (run 6: 16): triangle,
  swapchain (4 frames and results), targets (4), depth-bias, depth-clip (2), depth-funcs,
  depth-shader (2), heaps (2), stencil (2; the run 6 panic was a buffer of `u64::MAX` bytes made
  from a refused footprint, now E_OUTOFMEMORY, and the footprints now exist). The rest stop at:
  the converter (roots, compute, queues: StorageImage and others), bundles, copies and msaa
  (`ResolveSubresource`), queries (pipeline statistics), blend (dual-source), formats (E_FAIL),
  samplers, textures, topology, constants, views (device removed at replay by a refusal), raster
  (wireframe), geometry, tessellation, streamout (the stage helper), draws (instance step rates).
Still failing vkd3d-d3d12 tests seen in run 7 and why: execute_indirect (2 checks: a triangle
strip drawn from an upload argument buffer stays white; not found yet), reset_command_allocator
(allocator Reset must answer E_FAIL while a list records from it), device_removed_reason
(executing an open list must remove the device with DXGI_ERROR_INVALID_CALL), create_root_signature
(equal blobs must give the same object), create_heap and create_committed_resource (custom heaps
and refused heap flags), draw_uav_only (StorageImage), resource_allocation_info (25 checks) and
64kb_texture_alignment (1), clear_render_target_view (16).

Run 6 (2026-11-17, built at commit 1d5039a, before heaps), tests passing on real Direct3D 12 that
pass through ours:
- d3d12 (Wine): 6 of 7 (as run 5; test_swapchain_refcount still fails).
- vkd3d-d3d12: 65 of 172 (run 5: 54). Newly passing: create_compute_pipeline_state,
  cpu_signal_fence, queue_wait, and the compute and UAV tests whose shaders convert (the rest stop
  at the converter: see the gap list).
- vkd3d-d3d12_invalid_usage: 1 of 3; vkd3d-hlsl_d3d12: 8 of 9 (as run 5).
- Rig scenes through ours: 16 outputs pass, all equal to real Direct3D 12 (run 5's 15 and now
  depth-bias, fixed by the webgpu branch's depth views). The stencil scene still panics the device
  thread: "attempt to add with overflow" inside core's integer code (a `next_multiple_of` or the
  like; the library has no symbols in the backtrace), not found yet.

Run 6, after heaps (commit after f757cb7): vkd3d-d3d12 66 of 172 (test_suballocate_small_textures
newly passes); d3d12, invalid_usage and hlsl_d3d12 unchanged. The heap tests still fail:
test_create_heap and test_create_placed_resource crash after our E_NOTIMPL for custom heaps and the
display/sharing heap flags (the tests then use the absent object); test_map_placed_resources draws
with no render target ("drawing with no render target", a refusal to lift: an empty pass);
test_create_committed_resource fails 21 checks (23 before the clear-value check), mostly heap
flags we refuse.

Run 5 (2026-11-17), tests passing on real Direct3D 12 that pass through ours:
- d3d12 (Wine): 6 of 7. Fails: test_swapchain_refcount (10 of 39 checks: references the swap
  chain and its buffers hold). Two tests real Direct3D fails here (test_swapchain_draw,
  test_swapchain_backbuffer_index) pass through ours; they are not counted.
- vkd3d-d3d12: 54 of 172 (56 pass, two of them among the 4 real fails). At the start of run 5: 0
  (no test could start: the deserializer exports were missing), then 21, 24.
- vkd3d-d3d12_invalid_usage: 1 of 3. Fail: test_invalid_resource_barriers (2 of 29),
  test_invalid_copy_texture_region (4 of 21).
- vkd3d-hlsl_d3d12: 8 of 9. Fails: test_thread_id (962 of 4156 checks; compute).
- vkd3d-shader_runner (302 files): not run through ours yet.
The vkd3d-d3d12 tests passing: node_count, format_support, create_reserved_resource,
create_sampler, create_unordered_access_view, create_pipeline_state, create_fence,
multithread_private_data, gpu_signal_fence, multithread_fence_wait, fence_values,
set_render_targets, draw_instanced, draw_indexed_instanced, draw_no_descriptor_bindings,
multiple_render_targets, append_aligned_element, gpu_virtual_address, fragment_coords, scissor,
draw_depth_only, texture_resource_barriers, discard_instruction, shader_input_output_components,
immediate_constant_buffer, root_constants, texture_ld, resinfo, descriptor_tables,
update_descriptor_tables, update_descriptor_heap_after_closing_command_list,
copy_descriptors_range_sizes, create_null_descriptors, null_vbv, depth_clip, vertex_id,
copy_texture, copy_buffer_texture, face_culling, multithread_command_queue_exec, quad_tessellation,
line_tessellation, cpu_descriptors_lifetime, command_list_initial_pipeline_state, blend_factor,
output_merger_logic_op, shader_sample_position, vertex_shader_stream_output,
write_buffer_immediate, unbounded_resource_arrays, unbounded_samplers, readback_map_stability,
get_resource_tiling, shader_cache, multi_fence_event, enumerate_meta_commands (a pass may be a test
that checks a refusal: several of these only check that what is absent is reported absent).

The rig's Direct3D 12 scenes through ours (run 5): 17 of 124 outputs passed, all equal to real
Direct3D 12. Then blend-alpha and blend-factors were wrong at every pixel: the pipelines blend with
the second source (`SRC1` factors), which WebGPU refused because the converted shader does not mark
its second output (`Pipeline uses dual-source blending, but the shader does not support it`).
`SRC1` factors are now refused at pipeline creation (E_NOTIMPL, "dual-source blending"), so the
blend scene stops there and its four outputs are missing; last code: 15 passed (triangle;
swapchain, four frames and its results; targets rgba8, rgb10a2, rgba16f, r32ui; depth clip, funcs
and shader with their depth). Still wrong: depth-bias, every pixel (WebGPU refuses a view: "a view
of format Depth24PlusStencil8 of a texture with format Depth24PlusStencil8 ... not present in the
texture's viewFormat array", so the work is lost; look at how the typeless depth texture is made).
The stencil scene panics the device thread with an arithmetic overflow (num/mod.rs:1322, not yet
found). The others stop at a refusal: samplers, textures, topology, constants and views at
level-of-detail bias or other refusals turned into device removal at replay; formats at E_FAIL;
raster at wireframe without `POLYGON_MODE` (the Vulkan path); draws at instance step rates; heaps,
bundles, compute, queues, queries, copies (resolve) and msaa at calls not done; geometry,
tessellation and streamout at their stages (the stage helper, G6). Dual-source blending needs the
converter to mark the second output (G6).

## Real program (run 9)

`tools/graphics-rig/programs run --dll "d3d12.dll=%CARGO_TARGET_DIR%\debug\examples\d3d12.dll"
--dll "dxgi.dll=%CARGO_TARGET_DIR%\debug\examples\dxgi.dll" godot-platformer-2d-d3d12` (first try,
12:28 Pacific, build after deee187 with the subresource calls): FAIL, exit 0xc0000005 after 5.5 s,
1 frame, 0 of 3 frames saved. Godot's output.log (scratch graphics-rig/programs/<program>) shows it
made the device ("D3D12 11_0 - Forward+ - Using Device #0: NVIDIA - NVIDIA GeForce RTX 3080"), then:
- `CreateQueryHeap` of timestamps: E_NOTIMPL (absent on this back end; Godot logs it and goes on).
- `CreateResource` of a texture with a heap flag the front refuses ("heap flags for sharing,
  display, protection, write watching or atomics"; which flag is not printed yet: print it, and
  likely serve `ALLOW_SHADER_ATOMICS`, 0x400, which needs nothing of us beyond atomics in shaders).
- A compute shader with a storage image of no format: "image read/write without format is not
  currently supported" (naga's SPIR-V reader; G6), so `CreateComputePipelineState` fails.
Then it crashes, after those refusals.

## Real program (run 10)

Same command, build after a0760963 with the window association (13:18 Pacific): FAIL, exit
0xc0000005 after 7.1 s, 1 frame, 0 of 3 saved. Gone since run 9: the heap flag refusal (Godot
asks `ALLOW_SHADER_ATOMICS`), the D16 depth views WebGPU refused, `MakeWindowAssociation`.
Godot now stops at the converter only (G6): its 2D canvas vertex shaders fail with
`BindingsBeyondLimit` (7) and "unsupported image dimension %5" (buffer images, 6), so 13
`CreateGraphicsPipelineState` calls answer E_NOTIMPL; compute shaders with storage images of no
format fail as before; timestamps are absent (G2). Godot then draws with null pipelines and
crashes in its own code. Matching frames wait on those G6 items.

## Gap list (run 9; the draft of the pass bar's written list)

Each line is an absent feature behind failing tests, and who provides it. A gap belongs on the final
list only when it is the back end's or browser's to lack (reported absent as Direct3D 12 expects);
the gaps marked "front" are this milestone's work still to do, and those marked G2 or G6 wait on
those tracks.
- Absent in WebGPU, reported absent: pipeline statistics and stream output statistics queries
  (`CreateQueryHeap` answers E_NOTIMPL; Direct3D 12 has no capability bit for them); timestamps
  where the adapter lacks `timestamp-query` (`GetTimestampFrequency` and `GetClockCalibration`
  answer E_FAIL); logic operations (`D3D12_FEATURE_DATA_D3D12_OPTIONS.OutputMergerLogicOp`
  false); conservative rasterisation, raster order views, tiled resources, ray tracing, mesh
  shaders, variable rate shading, sampler feedback, float64, resource binding tier 2 and
  descriptor arrays where the back end lacks big tables, `SV_RenderTargetArrayIndex` from the vertex
  stage (all reported absent in `CheckFeatureSupport`).
- Absent in WebGPU, served by slow paths still to write in the front: one-dimensional textures (as
  2D), multisampled array textures, vertex strides that are not a multiple of 4 (repacked), zero
  strides and instance step rates, vertices past the bound buffer in indirect draws (WebGPU skips
  the draw).
- G6 (converter): StorageImage, TypedBuffer, Atomics, StorageCounter, RegisterSpace,
  DynamicOffset, ClipDistance, SampleCount, RenderTargetArrayIndex, InterpolateAt, dual-source
  blending's second output, the counting variant (counted occlusion), DrawBaseFromArguments
  (`SV_VertexID` with a base vertex in `ExecuteIndirect`), the stage helper (geometry, hull,
  domain, stream output).
- G2 (WebGPU back end): `TIMESTAMPS`.
- Front, this milestone: pipelines with no topology type, depth views of `R32_TYPELESS` textures
  made without the depth flag, the "still to find" tests under Test files, buffers in CPU-visible
  custom heaps, `ID3D12GraphicsCommandList3` and later (render passes in 4).

## Next, in this order

1. The real program: godot-platformer-2d-d3d12 waits on G6 (`BindingsBeyondLimit` in its canvas
   vertex shaders, buffer images, storage images of no format). Merge the shaders branch when it
   has them and run it again (command under "Real program"); then compare its frames with the
   rig's references. Also try godot-platformer-3d-d3d12.
2. The "still to find" tests (Test files, run 10): create_placed_resource's heap corruption
   first, then depth_stencil_sampling, execute_indirect, primitive_restart, render_a8,
   root_signature_byte_code, check_feature_support's one missing check.
3. Pipelines with no topology type (create_pipeline_state test 6): choose the topology class at
   the draw.
4. `check_d3d12 cases` again (run 10 result below if it finished) and a case for the waitable
   object against real DXGI.
5. Slow paths for what WebGPU lacks, or honest refusals: instance step rates and zero strides,
   1D textures, multisampled array textures, vertex strides not a multiple of 4, vertices past
   the bound buffer in `ExecuteIndirect`.
6. Counted occlusion and timestamps against real Direct3D 12 when G6's counting variant and G2's
   `TIMESTAMPS` land; `SV_VertexID` with a base vertex when G6's `DrawBaseFromArguments` lands.
7. `ID3D12GraphicsCommandList3` and later (render passes in 4), `ID3D12Fence1`.
8. vkd3d-shader_runner (302 files) through ours; recording without allocating per command, the
   call buffer of plan 4.8, moving `examples/directx` into `crates/wcontainers-directx`.

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
  (30 tests: 9 infrastructure, 8 front, 10 device on the recorder, 1 on the GPU, 2 unit; run 10:
  all pass). Put `set CARGO_INCREMENTAL=0&& ` before both commands (see the note further down).
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
  counts the refusals. The shader runner file (vkd3d-shader_runner, 302 files) has not been run
  through ours yet.
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

## Interface changes

- `Caps::adapter: AdapterInfo` (run 1): DXGI's adapter description is answered from `caps()`.
- `ShaderModule::from_wgsl(source, entry, stage)` (run 5, commit "interface: made a shader module
  from one entry point of a wgsl module..."): parse, validate and key one entry point of a WGSL
  module, as the built-ins are keyed. The infrastructure crate's present copy uses it instead of a
  builder of its own. G1's `helpers.rs` still has its own copy of the same steps (`module`/`program`);
  it could call this (G1 owns helpers.rs, so this track did not change it).

## Measurements

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
- The checksum of a root signature container written by our `D3D12SerializeVersionedRootSignature`
  is zero: only our runtime reads it.
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
- A pipeline state stream with no primitive topology subobject (type undefined) is refused
  (E_INVALIDARG) where real Direct3D 12 takes it: the front builds pipelines for a topology class.
  vkd3d's test_create_pipeline_state test 6 fails on it.

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
- `wcontainers-shaders` (ours) and through it dxil-spirv (MIT) and dxbc-spirv (MIT), as plan
  section 3 and the shaders track record.
- `naga` (MIT OR Apache-2.0, the workspace's copy, plan section 3): the front reads the sizes a
  program's uniform blocks declare (run 5). The infrastructure crate depends only on
  `wcontainers-gpu` (run 5; it had `naga` and `blake3`, MIT OR Apache-2.0 and CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception,
  which plan section 3 covers).
- Windows library only (dev-dependencies): `windows` and `windows-core` 0.62.2 (MIT OR
  Apache-2.0, as plan section 10 confirms).
- Test only: `wcontainers-gpu-webgpu`, `wgpu` 30.0.2 (MIT OR Apache-2.0), `pollster` (MIT OR
  Apache-2.0), `png` (MIT OR Apache-2.0), as the WebGPU track already uses them.

## Installed on the desktop

Nothing beyond the rig's setup. The shaders crate's submodules are checked out in this branch's
source directory there (C:\wc\src\phase-2-graphics-direct3d-12\crates\wcontainers-shaders: dxbc-spirv
and dxil-spirv, 18,052,177 bytes).
