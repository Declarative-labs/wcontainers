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
validation answers vkd3d's tests check. G5 is not complete: see "Test files", "Gap list" and "Next".

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
- Refused with E_NOTIMPL and the reason printed (`wcontainers: <reason>: not implemented`): null
  descriptors, arrays of resources, component mappings other than identity, minimum level-of-detail
  clamps, level-of-detail bias, min/max filters, border colours and mirror-once without the back
  end's feature, alpha-antialiased lines, depth bias on lines and points, forced sample counts,
  conservative rasterisation, typed buffer views, UAV counters, counted occlusion queries,
  timestamps (the WebGPU back end lacks `TIMESTAMPS`; `GetTimestampFrequency` answers E_FAIL as for
  a queue without them), pipeline and stream output statistics, bundles, stream output,
  `ResolveSubresource`, rectangles in UAV clears, an occlusion query across passes, video command
  lists, custom heaps.
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

## The pass bar's cases against real Direct3D 12 (own program, run 7)

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

Gap list (run 6, still current in run 7 less what run 7 added: the absent feature behind failing
vkd3d-d3d12 tests, from the logs, most common first). The converter (G6): "StorageImage" (11 shaders, DXBC typed UAV textures), "TypedBuffer" (6),
"Atomics" (4), "StorageCounter" (3), "RegisterSpace" (3, DXIL spaces other than 0), "DynamicOffset",
"ClipDistance", "SampleCount", "RenderTargetArrayIndex", "MultisampledArrayTexture", "InterpolateAt".
The front: geometry, hull and domain stages and stream output (12; the stage helper, G6),
`ResolveSubresource` (6), counted occlusion queries
(the counting replay of 4.12, which needs the converter's counting variant), timestamps (the WebGPU
back end lacks `TIMESTAMPS`), pipeline statistics, bundles, null
descriptors, custom heaps, `WriteToSubresource`/`ReadFromSubresource`, 1D and other render target
view dimensions, dual-source blending (G6), logic operations without `LOGIC_OP`. Not yet the pass
bar's final written list.
Wine's d3d12 `test_draw` fails converting its DXIL (dxil-spirv's output has
`SPV_KHR_float_controls`, which the shaders crate's SPIR-V reader refuses: G6). This list is not yet
the pass bar's final written gap list: most of these gaps are work of this milestone.

## Next, in this order

1. vkd3d-d3d12's cheap verdicts: allocator `Reset` E_FAIL while a list records from it; executing
   an open list removes the device (DXGI_ERROR_INVALID_CALL); equal root signature blobs give the
   same object; resource_allocation_info; clear_render_target_view; the execute_indirect strip.
2. Counted occlusion queries through the occlusion counter (needs G6's counting variant, the
   converter refuses every `VariantFlags` today); timestamps need G2's `TIMESTAMPS`. Then add
   counted occlusion and timestamps to `check_d3d12 cases` (pass bar).
3. `SV_VertexID` with a base vertex in `ExecuteIndirect` needs G6's `DrawBaseFromArguments`; the
   case indirect-vertex-id is ready for it. The D24S8 depth plane copy needs G2's DepthReadByDraw.
4. `ResolveSubresource`, bundles, null descriptors, custom heaps, typed buffer views (G6's
   `TypedBuffer`), UAV counters (G6's `StorageCounter`); keep raising vkd3d-d3d12 from 81 of 174;
   run vkd3d-shader_runner (302 files) through ours; test_swapchain_refcount, test_thread_id.
5. The real Direct3D 12 program (the rig's godot-platformer-*-d3d12), the frame latency waitable
   object, recording without allocating per command, the call buffer of plan 4.8, and moving
   `examples/directx` into `crates/wcontainers-directx` with the Direct3D 11 track.
6. The final written gap list for the pass bar.

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
  (28 tests: 9 infrastructure, 7 front, 9 device on the recorder, 1 on the GPU, 2 unit; all pass
  in run 6).
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
  `tools/graphics-rig/remote "cargo build -j 3 -p wcontainers-direct3d-12 --examples"`, then
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
- Seen in run 7: after `remote` sent edited sources, cargo on the desktop twice called the crate
  `Fresh` and kept the old library (clippy then checked nothing new). When a result looks stale,
  `tools/graphics-rig/remote "cargo clean -p wcontainers-direct3d-12 && ..."`.
- One case of the own program: `check_d3d12 case <name>` (names in `examples/check/cases.rs`).

## Interface changes

- `Caps::adapter: AdapterInfo` (run 1): DXGI's adapter description is answered from `caps()`.
- `ShaderModule::from_wgsl(source, entry, stage)` (run 5, commit "interface: made a shader module
  from one entry point of a wgsl module..."): parse, validate and key one entry point of a WGSL
  module, as the built-ins are keyed. The infrastructure crate's present copy uses it instead of a
  builder of its own. G1's `helpers.rs` still has its own copy of the same steps (`module`/`program`);
  it could call this (G1 owns helpers.rs, so this track did not change it).

## Measurements

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
