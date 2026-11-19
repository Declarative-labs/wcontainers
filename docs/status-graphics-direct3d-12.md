# Direct3D 12 and DXGI infrastructure status (G5)

Branch: phase-2-graphics-direct3d-12. Plan: docs/phase-2-graphics.md, sections 4.9, 5.2, 9, 10 and
G5 in 12. This track owns crates/wcontainers-directx-graphics-infrastructure and
crates/wcontainers-direct3d-12.

## Where things stand

Everything on G5's pass bar is done or on the written gap list (runs 35 to 41), ready for the
independent check; the literal check passes (runs 36 to 43). Test files 186 of 198. Every failing file is on the gap list
with its cause: an absent feature reported as Direct3D 12 expects (8), a listed loss naming what
the back end lacks (1), not to be matched (1), or a named item of the shaders track (2: one
refused by name, one a value the converted program gives). None is the front's own unfinished
part. Wine's dxgi tests that make no Direct3D 10 or 11 device are now counted too (3 of 5 pass,
2 fail on the absent software adapter). Real Direct3D 12 programs draw frames matching real
Direct3D 12's: Microsoft's D3D12HelloTriangle and D3D12HelloConstBuffers, bgfx's 01-cubes,
26-occlusion, 35-dynamic, 37-gpudrivenrendering and 38-bloom, and Godot's 2D platformer ("Real
program"). Godot's 3D platformer converts every shader since the shaders track's run 42 and, with
run 38's filtering fix, stops only at the WebGPU back end's view usage (an sRGB view of a texture
that also allows storage; "Asked of other tracks"): with that back end change applied for a run
and not committed (scratch run38/srgb-view-usage.patch), it drew 1 of 518,400 pixels beyond 8 in
each of frames 10, 60 and 120 (run38/g3d3.log, and the same in run 41, run41/g3d1.log, with its
one pipeline drawing alpha-antialiased lines now made, through the stage helper since run 41,
and drawing in none of those frames). The rig's Direct3D 12 scenes ("Scenes"): 95 outputs pass, 22
fail, 3 absent, each failure named (the textures scene passes since run 38, with DXIL 1D
textures). On wgpu's Direct3D 12 path (run 36's experiment, not kept) the three differences the
Vulkan path makes were measured gone; what that path still lacks is other tracks' ("Next" 3).

Latest results (run 43, 2026-11-19, desktop shared with other tracks; scratch run43/). At
4518fd0e, the run's last code (the run's literal sweep, tested before it was committed; logs
*1.log, `bin/all43.sh 1`): the literal check passes (lit1.log; lit5.log after the baseline was
pruned); lint clean (lint1.log); the crates' 50 tests pass (test1.log); test files 186 of 198, the
same 12 failing as runs 39 to 42 (full1.log, fail1.txt); own cases 30 of 30 equal, draw 0 of 4096
pixels beyond 1, `antialiased-lines` 0 of 4096 pixels beyond 63, largest difference 38
(check1.log); shader runner 214 of 302, the same 88 failing files (runner1.log, rfail1.txt);
scenes 95 pass, 22 fail, 3 absent, the same outputs (scenes1.log, sfail1.txt); Wine's five dxgi
tests 3 pass, the same 2 fail (dxgi1.log); Microsoft's two samples (2.6 and 2.5 s) and Godot's 2D
platformer (4.3 s) equal real (p1-*.log); Godot's 3D platformer still stops at the back end's
sRGB view (p1-godot-platformer-3d-d3d12-output.log); bgfx 37 727 to 791 pixels beyond 8 of
20,480 allowed, the overlay's text (p1-bgfx37.log); fence round trip `--alone` under
"Measurements". Run 42 (cut off before it wrote anything here) had run the same set at 236fd376,
the merge of the shaders track's run 46, with the same results (scratch run42/*1.log).

Results of run 41 (2026-11-19, desktop shared with other tracks; scratch run41/). At
731791fe, the run's last code (logs *1.log, `bin/all41.sh 1`): the literal check passes
(lit1.log); lint clean (lint1.log); the crates' 50 tests pass (test1.log); test files 186 of
198, the same 12 failing as run 39 (full1.log, fail1.txt); own cases 30 of 30 equal (the new
`antialiased-lines` among them; its picture 0 of 4096 pixels beyond 63, largest difference 38),
draw 0 of 4096 pixels beyond 1 (check1.log); shader runner 214 of 302, the same 88 failing files
(runner1.log, rfail1.txt); scenes 95 pass, 22 fail, 3 absent, the same outputs (scenes1.log);
Wine's five dxgi tests 3 pass, the same 2 fail (dxgi1.log); Microsoft's two samples (2.9 and 2.6
s) and Godot's 2D platformer (4 s) equal real (p1-*.log); Godot's 3D platformer still stops at
the back end's sRGB view, and with the view patch for one run as run 38 (g3d1.log, above); bgfx
37 747 to 883 pixels beyond 8 of 20,480 allowed, the overlay's text (p1-bgfx37.log); fence round
trip `--alone` under "Measurements".

Results of run 39 (2026-11-19, desktop shared with other tracks; scratch run39/). At
8ffdc980, the run's last code (logs *2.log): the literal check passes (lit2.log); lint clean
(lint2.log); the crates' 50 tests pass (test2.log); test files 186 of 198, the same 12 failing as
run 38 (full2.log); own cases 29 of 29 equal (the new `layers` among them), draw 0 of 4096 pixels
beyond 1 (check2.log); shader runner 214 of 302, the same 88 failing files (runner2.log); scenes
95 pass, 22 fail, 3 absent, the same outputs (scenes2.log); Wine's five dxgi tests 3 pass, the
same 2 fail (dxgi2.log); Microsoft's two samples and Godot's 2D platformer equal real (p2-*.log);
Godot's 3D platformer still stops at the back end's sRGB view; bgfx 37 826 to 980 pixels beyond 8
of 20,480 allowed, the overlay's text (p2-bgfx37.log); fence round trip `--alone` under
"Measurements". The same at d0b08b61 (the merge and the layer change; logs *1.log), but for the
literal check, which failed there on one moved line of the own program's readback pitch, named
in 8ffdc980.

Results of run 38 (2026-11-19, desktop shared with other tracks; scratch run38/). At
d5881c24's code (logs *2.log, the run's last code but the literal sweep below): the literal check
passes (lit2.log); lint clean (lint2.log); the crates' 50 tests pass (test2.log: 48 of the two
crates, 2 of the interface's slow paths); test files 186 of 198, the same 12 failing as run 37
(full2.log); own cases 28 of 28 equal, draw 0 of 4096 pixels beyond 1 (check2.log); shader runner
214 of 302, the same 88 failing files (runner2.log); scenes 95 pass, 22 fail, 3 absent
(scenes2.log); Wine's five dxgi tests 3 pass, the same 2 fail (dxgi2.log); Microsoft's two samples
and Godot's 2D platformer equal real (p2-*.log); bgfx 37 817 to 920 pixels beyond 8 of 20,480
allowed, the overlay's text (p2-bgfx37.log); fence round trip `--alone` under "Measurements". The
same at 436a4509 (the merges and the 1D binding; logs *1.log) but for the filtering fix's Godot
3D lines. The literal sweep's commit deee93a2: the literal check passes (lit6.log), lint clean and the crates' 50 tests pass (lint3.log, test3.log), and the six clear and unordered access test files pass (files4.log).

Results of run 37, 2026-11-19, desktop shared with other tracks; scratch run37/). At
0b231aa9 (the merges and the array slots; logs *1.log): the literal check passes (lit1.log);
lint clean (lint1.log); the crates' 50 tests pass (test1.log: 48 of the two crates, 2 of the interface's slow paths); test files 186 of 198, the same
12 failing as run 36 (full1.log, fail1.txt); own cases 28 of 28 equal, draw 0 of 4096 pixels
beyond 1 (check1.log); scenes 86 pass, 32 fail, 3 absent, as run 36 (scenes1.log); Wine's five
dxgi tests 3 pass, the same 2 fail (dxgi1.log); Microsoft's two samples and Godot's 2D platformer
equal real (p1-*.log); fence round trip `--alone` under "Measurements". The shader runner and
bgfx 37 were cut off there by a dropped connection (runner1.log "scp: Connection closed") and
were run again at e83ac416, the run's last code (the clip route, which changes nothing where the
back end has clip distances), with lint, the crates' tests and every test file (logs *9.log):
lint clean (lint9.log); the crates' 50 tests pass
(test9.log); test files 186 of 198, the same 12 (full9.log); own cases 28 of 28 equal
(check9.log); shader runner 214 of 302, the same 88 failing files as run 36 (runner9.log), with
sm6-uav-rwtexture now failing no check (565 checks run against real's 781, a crash at its first
array of storage textures before; the checks it does not reach were not traced this run) and
object-references running 1048 of real's 1312; Godot's 2D platformer equal, its 3D platformer
as at 0b231aa9 (p9-*.log); bgfx 37 702 to 852 pixels beyond 8 of 20,480 allowed, the overlay's
text (p9-bgfx37.log).

Results of run 36 (2026-11-19, desktop shared with other tracks; scratch run36/; at
0b7b2f36, the run's last code, logs *3.log): the literal check passes (lit3.log); lint clean
(lint3.log); the crates' 49 tests pass (test3.log); test files 186 of 198, the same 12 failing
(full3.log); own cases 28 of 28 equal, draw 0 of 4096 pixels beyond 1 (check3.log); shader
runner 214 of 302, the same 88 failing as at 42a2c38a (runner1.log, after the merge) and run 35
(runner3.log); scenes 86 pass, 32 fail, 3 absent, as run 35 (scenes3.log); Wine's five dxgi
tests 3 pass, the same 2 fail (dxgi3.log). Programs (p3-*.log): Microsoft's two samples 0 pixels
beyond 8 (2.6 s each), Godot's 2D platformer 0 of 518,400 pixels beyond 8 (3.8 s), bgfx 37 789
to 861 pixels beyond 8 of 20,480 allowed (the overlay's text). Fence round trip `--alone`:
"Measurements".

Latest results (run 35, 2026-11-19, desktop shared with other tracks; scratch run35/; at
ebc55743, the run's last code, logs *2.log; the same at 881224b4 in *1.log but for
combined_clip_and_cull_distances): test files 186 of 198 (full2.log); own cases 28 of 28 equal
(check2.log); shader runner 214 of 302, as at run 34 (runner2.log); the crates' 49 tests pass
(test2.log: 47 of the two crates, 2 of the interface's slow paths); lint clean (lint2.log); the
literal check fails on one line only, the shaders track's (No magic numbers, lit2.log). Programs
(p2-*.log): Microsoft's two samples 0 pixels beyond 8 (2.6 and 2.5 s), Godot's 2D platformer 0 of
518,400 pixels beyond 8 (4.1 s), bgfx 37 732 to 867 pixels beyond 8 of 20,480 allowed (the
overlay's text). Wine's five dxgi tests: dxgi2.log. Scenes: scenes2.log. Fence round trip
`--alone`: "Measurements".

Latest results (run 34, 2026-11-18 and 19, desktop shared with other tracks, load not taken;
scratch run34/): test files 183 of 198 at 91b4f491 (run34/full1.log; buffer_srv newly); own
cases 28 of 28 equal real Direct3D 12 (run34/check1.log); shader runner 214 of 302
(run34/runner1.log; expr-indexing and hard-copy-prop newly, from the shaders merge); the crates'
46 tests and the interface's 2 slow path tests pass; lint clean (run34/lint1.log, test1.log).
Programs at 91b4f491 (run34/p1-*.log): Microsoft's two samples (0 pixels beyond 8; 4.2 and 2.9
s), Godot's 2D platformer (0 of 518,400 pixels beyond 8; 4.5 s) and bgfx 37 (738 to 901 pixels
beyond 8 of 20,480 allowed, the overlay's text; 2.6 and 2.5 s against real's 2.4 s twice) draw
every compared frame as real Direct3D 12 does. The run's later code was run again in full
twice: at e2af024f (DXBC 5.1 slots, views of another dimension refused by name; run34/*2.log:
183 of 198, own cases 28 of 28, runner 214, the same programs equal) and with typed views read
as raw or structured buffers (run34/*3.log, the run's last code): test files 184 of 198
(register_space newly), and the rest below ("Test files", "Shader runner", "Real program").

Results of run 33 (2026-11-18, desktop shared with other tracks, load not taken; scratch
run33/, libraries at 0c33f3b4, the run's last code; the same at b436401c in full1.log and
runner1.log): test files 182 of 198 (run33/full2.log; early_depth_stencil_tests and
layered_rendering newly; buffer_srv now stops at a named refusal); own cases 28 of 28 equal real
Direct3D 12 (run33/check2.log, with the new early-per-primitive; the feature answers as in run
31); shader runner 212 of 302 (run33/runner2.log, below); the crates' 46 tests and the
interface's 2 slow path tests pass; lint clean (run33/lint2.log, test2.log). Programs
(run33/p1-*.log at b436401c, p2-*.log at 0c33f3b4): Microsoft's two samples (0
pixels beyond 8; 3.0 and 2.6 s), Godot's 2D platformer (0 of 518,400 pixels beyond 8; 4.6 s) and
bgfx 37 (735 to 848 pixels beyond 8 of 20,480 allowed, the overlay's text) draw every compared
frame as real Direct3D 12 does, and again at 0c33f3b4 (2.8 and 2.5 s, 5.1 s, 772 to 880 pixels); Godot's 3D platformer still draws white (its 22 fragment shaders
refused at naga's validation, G6).

Earlier results (run 31, 2026-11-18; scratch run31/): test files 180 of 198 (run31/full1.log;
9 newly: the six tessellation files and hull_shader_control_point_phase through the stage
helper after a domain stage, sampler_register_space and hull_shader_punned_array from the
shaders merge); own cases 27 of 27 equal real Direct3D 12 (run31/final1-check.log); shader
runner 205 of 302, the same 97 failing files as run 30 (run31/runner1.log); the crates' 46
tests and the interface's 2 slow path tests pass; lint clean. Programs (run31/p1-*.log, at
9467a66f): Microsoft's two samples, Godot's 2D platformer and bgfx 37 draw every compared frame
as real Direct3D 12 does; bgfx 26 too (run31/bgfx26-2.log, at 12966001).

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

- Run 28 (the twelfth run of the milestone): merged shaders (views' component mappings applied
  in the program, `VariantKey::swizzled`; multisampled arrays read as a texture per layer,
  `VariantKey::texture_layers`; atomic minimums and maximums of the other signedness by
  compare-exchange loops; `Reflection::slow_paths`); the shared line, webgpu, webgl, rig and
  direct3d-11 had nothing new. The merge's one conflict was `SlowPath::ALL` (this track's
  `SequentialBuffersCopied` beside the shaders track's two, 105 in all). The front now routes
  all three (below): five test files pass newly, and Godot's 3D platformer no longer stops at
  its first swizzled view. `ALLOW_SHADER_ATOMICS` is now taken on committed resources allowing
  unordered access, as measured on this desktop's runtime (new `answers` lines
  `heap_flag_atomics_*`; it was refused for every resource since run 26, from a buffer
  without unordered access): Godot's textures were refused there, and with them taken its 2D
  platformer draws every compared frame exactly as real Direct3D 12 does.

- Run 43 (the thirteenth run of the milestone's current form): run 42 had merged the shaders
  track's run 46 (236fd376: its tests' corpus and the Direct3D 11 front's alpha-antialiased
  lines, nothing of this track's crates or of the interface) and rerun everything, then stopped
  before committing or pushing anything else. This run merged direct3d-11 (c261e247: its own
  crate, the shared programs' tests and the baseline, nothing of this track's), checked the
  literal check there, pushed both merges, and took the next part of the literal sweep (4518fd0e,
  "No magic numbers"). The shared line, webgpu, webgl and rig had nothing new. At the end the
  desktop's build folder of this branch (8,394,851,094 bytes) was deleted; no build output was
  made in scratch or on the Mac.
- Run 41 (the eleventh run of the milestone's current form; run 40 stopped before committing
  anything but the merge 2e1dc463 of the shaders track's run 45, and left the line route below
  uncommitted, which this run checked, finished and committed): no branch had anything newer
  (direct3d-11's ten commits touch nothing of this track's). Alpha-antialiased lines (8ba6aae0):
  a pipeline with `AntialiasedLineEnable` and multisampling off, of line topology, with a pixel
  shader and no geometry, tessellation or stream output stage, is drawn through the stage
  helper's line route (`Stages::antialiased_lines`, `run_antialiased_lines` with viewport 0's
  size per draw): the pixel shader converted with `VariantFlags::LINE_COVERAGE` and `linked`
  past both programs' values, the vertex stage alone as compute, the final draw a triangle list
  with culling off and solid fill (`AntialiasedLinesAsQuads`, counted and warned). Other
  alpha-antialiased lines (after those stages, wireframe triangles) stay refused by name. New
  own case `antialiased-lines`, whose picture is compared within 63 of 255 (`cases::PICTURES`,
  `compare-case-pictures`; Departures): 0 of 4096 pixels beyond, largest difference 38
  (run41/case1.log, check1.log, alone1.log). The primitive index (731791fe): a pixel shader
  reading `SV_PrimitiveID` where the back end lacks `Features::PRIMITIVE_INDEX` is refused by name
  (`PrimitiveIdAsGeometry`), as the Direct3D 11 front does; before, the back end refused its
  module without a name. Not reached on this desktop. The topology types and fill modes are
  named (`state::TOPOLOGY_TYPE_*`, `FILL_MODE_*`); 4 baseline lines deleted.
- Run 39 (the tenth run of the milestone's current form): merged shaders (its runs 43 and 44:
  the interface change "a layer named past the views drawn into the first", `LAYER_FROM_STAGE`
  and `HelperValue::LayerCount`; the float layouts named; `stage::STREAM_BUFFERS` public); the
  shared line, webgpu, webgl, direct3d-11, rig and phase-1-literals had nothing new. No conflicts.
  Adopted the interface change (d0b08b61): a geometry shader writing `SV_RenderTargetArrayIndex`
  is converted with `LAYER_FROM_STAGE` and the pixel shader with the same flag and `linked`
  (`device::Link`, which now carries the flags with the linked values for clip distances too);
  the pass-through's block gets the target views' layer count (`PASSTHROUGH_LAYER_COUNT`). So a
  primitive naming a layer at or past the views' layers is drawn into the first layer and its
  pixel shader reads the layer it named, as Direct3D draws it; before, it was not drawn. New own
  case `layers` (the shaders crate's `gs_layers` naming four layers through two viewports into a
  two-layer target, `ps_layer` writing the layer it reads): equal to real Direct3D 12 in both
  layers (run39/case1.log, check1.log, check2.log). That it differed before the change is
  expected from the code, not measured. Literal sweep (8ffdc980): the infrastructure crate has a
  `d3d` module (fix-graphics.md items 3, 8 and 9): `PipelineStatistics` and
  `StreamOutputStatistics` as `repr(C)` mirrors of Direct3D's query data, `APPEND_ALIGNED_ELEMENT`,
  `SO_NO_RASTERIZED_STREAM` and the three placement alignments; the front's query values take
  their sizes and field offsets from the structures (written with `bytemuck::bytes_of`), and the
  own program's readback pitch is `D3D12_TEXTURE_DATA_PITCH_ALIGNMENT`; 31 baseline lines
  deleted. At the end the desktop's build folder of this branch (8,976,912,215 bytes) was
  deleted; no build output was made in scratch or on the Mac.
- Run 38 (the ninth run of the milestone's current form): merged shaders (its run 42: DXIL slots
  numbered over every register the resource list declares, used or not; DXIL 1D textures and 1D
  arrays made 2D textures one texel high; `TEXTURE_INFO` sizes past the level count zero) and
  direct3d-11 (nothing of this track's; the interface's `reflect.rs` doc of `TEXTURE_INFO`); the
  shared line, webgpu, webgl, rig and phase-1-literals had nothing new. Two baseline conflicts,
  neighbouring lines of the shaders crate both sides removed, resolved by keeping neither as
  docs/literals.md says; the check then passed. The front's own numbering already counted every
  declared register, an element per register (run 37), so nothing changed there. The front no
  longer refuses a texture the converted program declares one-dimensional
  (`ONE_DIMENSIONAL_DECLARATION` deleted): 1D views were already bound as 2D views of one row; the
  rig's textures scene now passes all 9 outputs (refused at its first, `textures-1d`, before).
  Godot's 3D platformer: no shader refused for `StorageFormatKind` or `ResourceArray` any more;
  with the WebGPU back end's view usage patched for the run (scratch run38/srgb-view-usage.patch,
  the ask below, not committed) it stopped at 5,198 bind groups the back end refused ("a filtering
  sampler with a texture this device cannot filter"). Traced with a print in the back end (not
  committed): a pixel shader samples a texture its converted program declares as depth (a
  `Depth16Unorm` texture compared through one sampler and sampled plainly through another,
  linear one); the front skipped such textures when noting what the back end cannot filter, so
  the program was not converted with `filter_in_shader` for it. Fixed (d5881c24, bind.rs
  `note_filtering`): a texture the program declares as depth is noted whatever its format, as
  the back end takes no filtering sampler with a depth texture, and so is a null one there; the
  program then filters it where a linear non-comparison sampler samples it (`FilterInShader`; the
  converter leaves the comparisons native). With both, Godot's 3D platformer drew its frames
  10, 60 and 120 with 1 of 518,400 pixels beyond 8 each (largest difference 48, at 921,317;
  run38/g3d3.log, 44 s for 120 frames; picture run38/g3d3-frames). Without the back end patch it
  stops at the sRGB view as before (run38/p2-*.log). Next 3b reconsidered (Departures): the
  interface's `ClearValue::Color` is the clear's raw bits, so a back end whose API clears through
  a float that does not keep them (denormals on wgpu's Direct3D 12 path) is to clear another way,
  as the WebGPU back end already does for integers (`IntegerClearByDraw`); asked of it, not done
  in the front. Traced the runner's two files that ran fewer checks with none failing: the runner
  runs a file once per shader model in its range (shader_runner.c, the loop over models in
  `run_shader_tests`), SM6.0 to SM6.5 on real Direct3D 12 and SM6.0 alone through ours (section 9
  answers 6.0), so sm6-uav-rwtexture's 565 of 781 checks and object-references' 1048 of 1312 are
  the models above 6.0. Literal sweep: the storage clears' component bit widths named by their types (`u8::BITS`, `u16::BITS`, `u32::BITS`, `HALF_BITS` from `half::f16`), 20 baseline lines deleted. At the end the desktop's build folder of this
  branch (8,415,375,384 bytes) was deleted; no build output was made in scratch or on the Mac.
- Run 37 (the eighth run of the milestone's current form): merged shaders (its run 41: DXIL
  arrays of storage textures split into a view per element, the refusal names in a `refusal`
  module) and direct3d-11 (nothing of this track's or of the interface); the shared line, webgpu,
  webgl, rig and phase-1-literals had nothing new. No conflicts. The DXIL reader now gives every
  element of an array of views its own slot (`dxil.rs`, `Reader::declared`: a resource's range
  size from the resource list, an element per register, slots counted over the elements), as the
  converter now declares a view per element; before, an array took one slot and the views after
  it were numbered one too low. Godot's 3D platformer's `ResourceArray` compute shaders now
  convert or stop at `StorageFormatKind` (3, below). Next 3a done: a vertex, geometry or domain
  shader writing clip distances on a back end without `Features::CLIP_DISTANCE` is converted to
  pass them past both programs' values (`VariantKey::linked`, `VariantFlags::CLIP_DISTANCE`), the
  pixel program applies them (`ClipDistanceInShader`, counted per draw with the pipeline's slow
  paths), and every vertex program made later from the pipeline's key (indirect arguments,
  pulled streams, bound views) and the counting program carry the same values; the culling
  stage's pass-through (cull distances with clip distances) is refused by name, a shaders item.
  Measured on wgpu's Direct3D 12 path (run 36's patch applied for the run, not committed;
  run37/files7.log): clip_distance passes (264 checks, before refused at wgpu's validation),
  test_geometry_shader and tessellation_dcl_index_range pass, combined_clip_and_cull_distances
  stops at the named refusal. The literal check's 8 lines of the old reader were deleted.
  At the end the desktop's build folder of this branch (8,386,073,614 bytes) and the scratch
  worktree dev37 (no build output) were deleted; no build output was made on the Mac.
- Run 36 (the seventh run of the milestone's current form): merged shaders (its run 40:
  `SwizzledDepth` lifted for mappings keeping red first, wave minimums and maximums of the other
  signedness, the vertex counts made public) and direct3d-11 (nothing of this track's or of the
  interface); the shared line, webgpu, webgl, rig and phase-1-literals had nothing new. The
  front imports `wcontainers_shaders::stage::{TRIANGLE_VERTICES, LINE_ADJACENCY_VERTICES,
  TRIANGLE_ADJACENCY_VERTICES}` and its own `state::TRIANGLE_VERTICES` is gone, so the literal
  check passes. An unordered access view of a one-layer 2D array bound where the program declares
  a 2D storage texture is bound as a 2D view of that layer, as shader resource views have been
  since run 35, and a storage view of another dimension is refused by name (`declared_dimension`
  in bind.rs; Godot's 3D platformer reached wgpu's validation with one at binding 160, storage
  slot 0). Next 3 tried and measured, not kept (below and "Next" 3): with the device thread on
  wgpu's Direct3D 12 path the scenes' anisotropy, alpha-to-coverage and line differences are
  gone, but four test files and every real program behind the rig's forwarding library fail.
- Run 35 (the sixth run of the milestone's current form): merged shaders (its runs 38 and 39:
  clip distances from geometry shaders, vertex cull distances through the stage helper), webgpu,
  webgl and direct3d-11; the shared line, rig and phase-1-literals had nothing new. Settled the
  independent check of run 34: setting the root signature already set keeps its bindings, so a
  bundle setting the caller's root signature inherits them (`RootState::set_root`; the rig's
  bundles scene now passes); 1D texture views and unordered access views are read as 2D ones of
  one row (DXBC; DXIL keeps `Texture1D` one-dimensional, refused by name, a shaders item); a
  constant buffer view shorter than the program's block, or past its buffer's end, is read from
  the program's bytes with a zero tail for upload buffers (`ConstantBufferGrown`; the rig's
  constants scene now passes), refused by name for other heaps; adjacency topologies without a
  geometry shader draw the list or strip of their own vertices from generated indices
  (`AdjacencyDropped`; indexed and indirect ones refused by name); Wine's dxgi tests without
  Direct3D 10 or 11 devices are counted, and their three failures fixed or named (adapters hold
  their factory, outputs their adapter, `CreateDXGIFactory`'s factory refuses `IDXGIFactory1`,
  `EnumWarpAdapter` with a null pointer gives DXGI_ERROR_INVALID_CALL; no software adapter is
  the absent feature behind the other two); vertex shaders writing cull distances are drawn
  through `Stages::culled` and the pass-through (combined_clip_and_cull_distances passes;
  clip_distance passed with the merge alone); draws and dispatches with no pipeline state warn;
  the own program deletes its previous results first; the fence round trip measured `--alone`
  with the load. A one-layer 2D array view bound where the program declares a 2D texture is
  bound as a 2D view of that layer (Godot's 3D platformer), and the dimension refusal names
  both dimensions. The desktop's copy now has `core.autocrlf` false and only the submodules the
  build needs (with Windows line ends the literal check read multi-line strings differently).
  At the end the desktop's build folder of this branch (8,520,160,351 bytes) was deleted; no
  build output was made in scratch or on the Mac.
- Run 34 (the fifth run of the milestone's current form): merged shaders (its runs 35 to 37:
  storage offsets of shader resource slots, DXIL targets of four components, 16-bit integers,
  constant components picked at run time, Godot's 3D fragment shaders), webgpu, webgl,
  direct3d-11 and origin/phase-1-literals (the literal check adopted, "No magic numbers"); the
  shared line and the rig had nothing new. Settled the independent check of run 33: the gap
  list and the shader runner's classification redone from this run's logs (shader_eval_attribute
  and eval-attrib fail by values and by DXIL `InterpolateAt`, not DXBC refusals; wave-ops-int and
  wave-ops-float fail by converter refusals, wave-ops-uint runs fewer checks only for int64);
  buffer_srv passes with the front's refusal dropped; views bound below their start count the new
  slow path `StorageOffsetInShader` (interface change); Wine's dxgi tests written outside the
  counted set with the reason; the format answers' text corrected (one beyond real, the
  departure's routes); the call buffer of plan 4.8 written as a departure with its measured cost.
  Found and fixed: the front numbered shader model 5.1 DXBC typed views by register, where the
  converter numbers them by their place in (space, register) order (`dxbc.rs`, `slots`), so a
  typed buffer in a space or past another view got no format (`TypedBufferFormat`, which run 31
  to 33 had written as a shaders item: register_space, resource_arrays, runner
  unbounded-array-5.1). Shader resource views of another dimension than the program declares
  are refused by name (Godot's 3D platformer reached wgpu's validation with one). A typed view
  bound where the program declares a raw or structured buffer is read as its elements' bytes
  (`buffer_view`), as real Direct3D 12 does on this GPU (register_space binds `R32_UINT` views to
  `ByteAddressBuffer` and `StructuredBuffer<uint>` declarations), instead of refused. Test files
  184 of 198. At the end the desktop's build folder of this branch (8,228,781,297 bytes) was
  deleted; no build output was made in scratch or on the Mac.
- Run 33 (the fourth run of the milestone's current form; run 32 was cut off before it
  committed, and its uncommitted work was finished here): no graphics branch had anything new
  but direct3d-11, which changed nothing of this track's or of the interface (not merged). The
  front's own three files: forced early depth and stencil tests run in the program
  (`early.rs`, below); geometry shaders choosing a layer or viewport draw once per layer and
  viewport (`draw_runs`, below); buffer_srv's cause found (a read-only structured view at byte
  240 was bound from byte 224, where the variant did not move it; the converter's
  `STORAGE_OFFSET` covers unordered access slots only), now refused by name and asked of the
  shaders track. Pixel outputs of another type than their target are written as the target's
  type (`retyped_targets`, `VariantKey::targets_in_shader`, the shaders track's
  `TargetInShader`) instead of refused. Draws writing depth or stencil whose pixel shader forces
  early tests draw each primitive twice (`EarlyTestsPerPrimitive`, below; new own case
  early-per-primitive, equal to real Direct3D 12). Programs converted with `STORAGE_OFFSET` were
  refused at conversion since run 31 ("shaders reading helper constants the front does not
  write": the front's list of the helper constants it writes lacked `StorageOffset`), so no
  view at an unaligned offset was ever read through it; the list now has it, and refusals of
  that kind name the constant. More literals named (below). An orphan of run 32's
  last `remote` (a `cargo clippy` blocked for 17 minutes at 1 s of CPU) held the desktop's GPU
  slot shared, so every track's queued jobs and remote commands waited; it was this branch's
  own, and was ended (`taskkill` of its process tree). At the end the desktop's build folder of
  this branch, C:\wc\target\phase-2-graphics-direct3d-12, and run 32's scratch worktree dev32
  (31 MB, no build output) were deleted; no build output was made in scratch or on the Mac.
- Run 31 (the third run of the milestone's current form): merged the shared line, shaders and
  webgpu (webgl and direct3d-11 had nothing new); the shaders branch's run 29 to 32 items had
  landed. Settled the independent check of run 30: stages after a domain shader (geometry
  shaders and stream output) run through the stage helper (`Stages::tessellated_with`,
  `run_after_domain`); geometry shaders choosing a layer or viewport are refused by name until
  the front draws per layer (`Stages::selects`); raw and structured views at offsets the back
  end cannot bind are bound from below with the rest in the program (`STORAGE_OFFSET`); the
  format answers give sampling and gathering of depth views and other formats the back end
  cannot filter (`FilterInShader`), casts within typeless groups (twins), and stream output into
  the formats of 32-bit components; bgfx 26-occlusion rechecked (equal); the first part of the
  literal sweep (below). At the end the desktop's build folder of this branch,
  C:\wc\target\phase-2-graphics-direct3d-12 (7.5 GB), was deleted; no build output was made in
  scratch or on the Mac (a `cargo check` on the Mac stopped at the shaders crate's missing
  submodules; its 223 MB `target` was deleted).
- Run 30 (the second run of the milestone's current form): merged the shared line (which held
  the shaders branch: gathers with run-time offsets, the helper lane query, wave operations);
  no other branch had anything this track needed. Settled the independent check of run 29: early
  depth tests before storage writes, pixel outputs of another type than their target (after the
  description's own checks), textures read as another type than their view, and patches of
  another control point count are refused by name; `ExecuteIndirect` draws without indices are
  counted as a listed loss (`IndirectFetchPastEnd`, an interface change); lists that never set
  scissors draw unscissored, as real Direct3D 12 on the RTX 3080 does; `Int64ShaderOps` and
  `DepthBoundsTestSupported` are answered false whatever the back end declares (section 9);
  releases no longer wait on the device thread (a release during process exit hung). The gap
  list and the shader runner's 97 files were sorted file by file by cause.
- Run 29 (the first run of the milestone's current form): nothing new on the shared line,
  shaders, webgpu, webgl or rig; direct3d-11 changed nothing of this track's (not merged).
  Vertex shaders drawn directly are now converted for what is bound to them, as pixel shaders
  are (below), which lifts the refusals of typed buffers and read-only storage textures in other
  formats than `R32_*`, component mappings, stencil views, level-of-detail bias, border and
  mirror-once sampling, and filtering in the program, in vertex shaders; and pixel shaders
  converted for their bound views now draw with pulled or arguments vertex programs too. New own
  case vertex-views. The gap list was set out as the pass bar asks (absent features, listed
  losses, not to be matched, named G6 items).

What the front does, by subject:
- Early tests in the program (run 33, `device/early.rs`): a pipeline whose pixel shader forces
  early depth and stencil tests that change its results (storage writes, or a discard while
  depth or stencil is written) keeps its depth-stencil state. Its draws while a depth-stencil
  view is bound end the pass and draw in one whose attachment is read only (a clear waiting
  for the view is done first), the depth aspect bound at `EARLY_DEPTH_BINDING` and the stencil
  aspect at `EARLY_STENCIL_BINDING` (for a format without stencil, a zeroed `RGBA8_UINT` null
  texture of the view's sample count), `HelperValue::EarlyTests` written (compares as
  `CompareFn` codes, zero for a test that is off or an aspect the view lacks, reference, read
  mask) and the pixel stage converted with `VariantFlags::EARLY_TESTS` through the formats
  record (`Bound::early`); counted `EarlyTestsInShader`. A draw that also writes depth or
  stencil (not read-only in the view, writes on, stencil operations other than keep) draws each
  primitive alone twice (`early::draw_per_primitive`, counted `EarlyTestsPerPrimitive`): first
  tested in the program as above with a twin of the pipeline's depth-stencil block whose writes
  are off, then the pipeline's vertex program alone, with its own depth-stencil block, in a pass
  with no colour targets, which makes Direct3D's depth and stencil writes. A strip's odd
  triangles are drawn alone with a twin raster block of the other front face, so culling and
  `SV_IsFrontFace` are as in the strip (the own case early-per-primitive shows real Direct3D 12
  counting the second triangle of its strip as back-facing, and ours the same). Refused by name:
  such a draw behind a geometry or tessellation stage, on a depth buffer denying shader
  resources, and per primitive: a pixel shader reading its primitive index, instanced,
  indirect or predicated draws, draws while a query runs, indexed strips, and vertex programs
  converted for the draw.
- Layered geometry shaders (run 33, `geometry.rs`, `draw_runs`): where the geometry stage
  writes `SV_RenderTargetArrayIndex` or `SV_ViewportArrayIndex`, the draw after the stage
  helper is drawn once per layer of the bound target views (`LayerSplit`) and per viewport
  (`MultiViewportSplit`), each run a pass of its own into views of that layer alone, through
  that viewport and scissor, with the pass-through program's helper block naming the run (its
  size read from the pass-through module's uniform type) and the pixel stage's
  `HelperValue::Layer` and `Viewport` the run's. Refused by name: such a draw while a query
  runs, views of different layer counts, and a 3D texture's target view.
- Vertex stages for what is bound (run 29, `formats.rs`): a pipeline whose vertex stage the back
  end's vertex stage runs (no geometry, hull or domain stage or stream output after it) keeps a
  `Formats` record of it as for its pixel stage. At each draw `bound_program` works out what the
  bound views and samplers ask of each stage (`stage_bound`); where the vertex stage's differs
  from its own, or the pixel stage's does and the draw starts from another vertex program
  (pulled streams, bases from indirect arguments: `VertexKind`), both stages are converted
  together, the vertex one from that vertex program's key, and kept per pipeline by
  `Combination`. The vertex program's own helper block and pulled streams then come from it.
  Refused by name: such a draw inside a counted occlusion or statistics query, a minimum
  level-of-detail clamp and multisampled arrays of several layers in vertex shaders, and any
  of these in a vertex shader before a geometry or domain stage (its program runs as compute).
- View component mappings and multisampled arrays (run 28, `bind.rs`, `formats.rs`, `dxbc.rs`,
  `dxil.rs`): a shader resource view whose `Shader4ComponentMapping` reads other than the
  identity does for its format, and every stencil view (`X24_TYPELESS_G8_UINT`,
  `X32_TYPELESS_G8X24_UINT`: Direct3D's stencil is in green, the interface's stencil view's in
  the first component, so the mapping is composed onto green), is noted at binding; the pixel
  or compute program is converted with `VariantKey::swizzled` for that slot and kept per
  pipeline and bound state, as typed view formats are (`SwizzleInShader`, counted at each such
  binding). Multisampled arrays (`Texture2DMSArray`): the shader's declarations (DXBC
  `dcl_resource_texture2dmsarray`, DXIL resource kind 8) give the slots, converted at creation
  with `texture_layers` of one layer each; a bound view of several layers converts the stage
  again with its layer count, and its layers after the first are bound at the bindings the
  variant's reflection adds (`AddedBinding::TextureLayer`, a texture per layer, run 25's
  storage). So `binding_sets` now makes the bound program between resolving the bindings and
  making the sets. Refused by name: either in vertex, geometry, hull and domain shaders (they are
  not converted again for bound views). The programs' own slow paths
  (`Reflection::slow_paths`, such as `AtomicExchangeLoop`) are kept per pipeline and counted at
  each draw or dispatch.
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
  component mappings, stencil views and multisampled arrays of several layers read in vertex,
  geometry, hull or domain shaders (run 28), arrays of resources, min/max filters,
  alpha-antialiased lines after a geometry or tessellation stage or stream output, or of
  wireframe triangles (run 41; line lists and strips straight from the vertex shader are drawn
  through the stage helper), a pixel shader reading its primitive index where the back end
  lacks it (run 41, `PrimitiveIdAsGeometry`),
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

## CheckFeatureSupport and the front's refusals

Run 31 (run31/final1-check.log: 192 of 872 feature answers differ, one beyond real; an earlier build's answers compared field by field in run31/features/): every
format ability section 9 requires at 11_0 is answered where the front serves it, natively or by
its route: `SHADER_SAMPLE` and `SHADER_GATHER` of depth views (`R24_UNORM_X8_TYPELESS`,
`R32_FLOAT_X8X24_TYPELESS`) and of formats the back end cannot filter (`FilterInShader`; but for
`R32G32B32_FLOAT`, whose sampling Direct3D's tables leave optional: only where the back end
filters it), `CAST_WITHIN_BIT_LAYOUT` for every member of a typeless group (twin textures where
the back end cannot reinterpret, `ReinterpretCopy`), and `SO_BUFFER` for the formats of 32-bit
components (`StreamOutAsCompute`). `R32_FLOAT`, `R24_UNORM_X8_TYPELESS` and
`R32_FLOAT_X8X24_TYPELESS` now equal real Direct3D 12's answers. One answer is beyond real
Direct3D 12 on the RTX 3080: `R32G32B32_FLOAT` sampled and gathered, which the WebGPU back end
now declares (it stores the format as `rgba32float` and filters it); the format tables leave it
optional, and it is served, so it stays.

The answers are plan section 9's (tests `webgpu_reports_11_0_and_tier_1`,
`resource_tables_give_tier_3_but_not_11_1`, `no_device_without_compute_11_0_sizes_or_d32_s8_targets`;
the own program's `features` against real Direct3D 12: the one answer beyond it above, none
other). The front still refuses
by name a few things feature level 11_0 allows; every one is in the gap list's "Refused by name
in the front" (the independent check of run 34 found three missing there: views of 1D textures,
constant buffer views past their buffer's end, adjacency without a geometry shader; run 35
serves all three, below, and lists what is still refused of them). A program meeting a refusal
gets E_NOTIMPL or a removed device, never a fake success.

## The pass bar's cases against real Direct3D 12 (own program)

`check_d3d12 cases` runs each case in its own process with real Direct3D 12 and through ours and
compares the lines (`examples/check/cases.rs`). Run 41: 30 of 30 equal and the one picture
within its tolerance (run41/check1.log). Run 39: 29 of 29 equal (run39/check2.log, the
new `layers` among them; features, outputs and answers as run 34). Run 34: 28 of 28 equal (run34/check1.log;
features 192 of 872 differ, one beyond real (above), outputs 22 of 110 and answers 7 of 40, none
beyond). Run 29: 26 of 26 equal (run29/check1.log;
features 253 of 872, answers 7 of 40 differ, none beyond real Direct3D 12). Run 20: 24 of 24 equal; runs 18 and 19: 23 of
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
- layers (run 39): a geometry shader naming one of four layers per instance through viewport 0
  or 1 into a target of two layers; the primitives naming layers 2 and 3 land in layer 0 with
  the pixel shader reading 2 and 3. Both layers compared pixel by pixel and by colour counts.
- antialiased-lines (run 41): alpha-antialiased lines (`AntialiasedLineEnable`, multisampling
  off, back faces culled, which lines ignore) blended over the target: a list of twelve spokes at
  every 30 degrees, ends on and between pixel centres, and a strip. The case writes its picture
  (`antialiased-lines.rgba`), which `compare-case-pictures` holds to real Direct3D 12's within 63
  of 255 per channel (`cases::PICTURES`): Direct3D leaves the line's filter to the
  implementation; the shaders track measured the RTX 3080 and WARP 128 apart and its own box
  filter within 52 of the RTX 3080 (its status, run 45). Its line in `cases.txt` says only that
  something was drawn.
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
- vertex-views (run 29): a vertex shader reading typed buffers viewed as
  `R32G32B32A32_FLOAT` and `R8G8B8A8_UNORM`, a texture through a view mapping blue, red, one,
  green, and a texture through a sampler with `MipLODBias` 1 (which real Direct3D 12 applies to
  `SampleLevel`), drawn directly from first vertex 7; and a second pipeline sampling the
  textures alone, drawn by `ExecuteIndirect` of two instances from first vertex 5 (its program
  reads the base from the arguments). The colours of 4 by 13 pixels.
- History: run 7 11 of 13, runs 10 and 11 12 of 13, run 12 13 of 14, every case equal since run
  13 (15, then 16, 17, 20, 21, 22, 23, 24 in run 24, 25 in run 25: run25/m1-check.log).

## Test files (Wine's and vkd3d's, through our libraries, RTX 3080)

How a file counts: the rig compares each test's verdict with real Direct3D 12's on this GPU
(`wine-tests run`, below); a test that skips what real Direct3D 12 runs, or runs fewer checks,
fails. Since the rig's run of 2026-11-17 19:11 (its commit 4ffedc6), tests that fail some
checks on real Direct3D 12 are counted too, held to every other check, so the counted set is
198 (it was 193).

Run 36 (2026-11-19, desktop shared with other tracks): 186 of 198 after the merges (scratch
run36/full1.log, at 42a2c38a), with the storage view fix (files2.log) and at the run's last code
(full3.log, 0b7b2f36), the same 12 failing as run 35. On wgpu's Direct3D 12 path (run 36's
experiment, not kept, full2.log) 182: clip_distance, combined_clip_and_cull_distances,
clear_unordered_access_view_image and copy_buffer_texture failed too ("Next" 3).

Run 35 (2026-11-19, desktop shared with other tracks): at the run's last code (scratch
run35/full2.log) 186 of 198: d3d12 (Wine) 10 of 10, vkd3d-d3d12 164 of 176,
vkd3d-d3d12_invalid_usage 3 of 3, vkd3d-hlsl_d3d12 9 of 9. Newly passing: clip_distance (the
shaders merge) and combined_clip_and_cull_distances (the cull route). No file lost.

Run 34 (2026-11-18 and 19, desktop shared with other tracks, load not taken): at the run's
last code (scratch run34/full3.log) 184 of 198: d3d12 (Wine) 10 of 10, vkd3d-d3d12 162 of 176,
vkd3d-d3d12_invalid_usage 3 of 3, vkd3d-hlsl_d3d12 9 of 9. Newly passing: buffer_srv (the
converter's storage offsets of shader resource slots, and the front's refusal dropped; 183 at
91b4f491, run34/full1.log, and at e2af024f, full2.log) and register_space (DXBC 5.1 slots and
typed views read as raw or structured buffers). No file lost.

Run 33 (2026-11-18, desktop shared with other tracks; scratch run33/full1.log, libraries at
b436401c): 182 of 198. d3d12 (Wine) 10 of 10, vkd3d-d3d12 160 of 176, vkd3d-d3d12_invalid_usage 3
of 3, vkd3d-hlsl_d3d12 9 of 9. Newly passing: early_depth_stencil_tests (`EarlyTestsInShader`) and
layered_rendering (`LayerSplit`). No file lost.

Run 31 (2026-11-18, desktop shared with other tracks; scratch run31/full1.log, libraries at
12966001; again at 9467a66f, the run's last code, run31/full2.log: the same 18 failing): 180 of
198. d3d12 (Wine) 10 of 10,
vkd3d-d3d12 158 of 176, vkd3d-d3d12_invalid_usage 3 of 3, vkd3d-hlsl_d3d12 9 of 9. Newly passing:
domain_shader_inputs, domain_shader_one_patch_constant_input, hull_shader_patch_constant_inputs,
hull_shader_relative_addressing, line_tessellation, quad_tessellation (the stage helper after a
domain stage), hull_shader_control_point_phase (the helper's patches of more control points),
sampler_register_space (shader model 5.1 numbering) and hull_shader_punned_array (struct values
used whole), the last three from the shaders merge alone. buffer_srv went from a removed device
to one failing check.

Run 30 (2026-11-18, desktop shared with other tracks; scratch run30/full1.log, libraries at
418cba9 after merging the shared line with the shaders branch): 171 of 198. d3d12 (Wine) 10 of
10, vkd3d-d3d12 149 of 176, vkd3d-d3d12_invalid_usage 3 of 3, vkd3d-hlsl_d3d12 9 of 9. Newly
passing: gather and gather_c (the shaders track's gathers with run-time offsets, no change in the
front). No run since 17 lost a file. Earlier: run 28 169 (stencil_load, srv_component_mapping,
depth_stencil_sampling, multisample_array_texture, atomic_instructions newly), run 25 tgsm and
clear_render_target_view, run 24 hull_shader_fork_phase, nop_tessellation_shaders,
tessellation_dcl_index_range, tessellation_primitive_id and vertex_id. Each run's logs are in
scratch run<N>/full*.log and logs-full*/ (since run 18).

Files passing: d3d12 (Wine) all 10; vkd3d-d3d12_invalid_usage all 3; vkd3d-hlsl_d3d12 all 9;
vkd3d-d3d12 all but the 12 the gap list names, each with what it stops at (run 36's logs).

History (tests passing on real Direct3D 12 that pass through ours, vkd3d-d3d12 of 172 to 174):
run 5 54, 6 66, 7 80, 8 87, 9 74 (the rig's stricter rule began), 10 87, 11 94, 12 98, 13 103,
14 104, 15 106, 16 106; run 17 108 of 176 under the newer rule, run 18 123 of 176, run 19 133,
run 20 135, runs 21 to 23 135, run 24 140, runs 25 to 27 142, runs 28 and 29 147, run 30 149,
run 31 158, run 33 160, run 34 162, runs 35 and 36 164.
Whole counts: run 12 116 of 193, 13 121, 14 122, 15 124, 16 124; run 17 129 of 198, run 18 145,
run 19 155, run 20 157, runs 21 to 23 157, run 24 162, runs 25 to 27 164, runs 28 and 29 169,
run 30 171, run 31 180, run 33 182, run 34 184, runs 35 and 36 186.

## Shader runner (vkd3d-shader_runner, 302 files)

Run with our d3d12.dll alone (the system's dxgi.dll): with our dxgi.dll too every file crashes
in its Direct3D 11 part before reaching Direct3D 12 (our dxgi.dll serves only Direct3D 12), 0 of
299 (run12/runner1.log). Command: `E='%CARGO_TARGET_DIR%\debug\examples';
tools/graphics-rig/wine-tests run --dll "wcontainers_d3d12.dll=$E\wcontainers_d3d12.dll" --dll
"d3d12.dll=$E\d3d12.dll" vkd3d-shader_runner`.

Run 30 (run30/runner1.log, libraries at 418cba9, 10:22 to 10:37): 205
of 302, the same 97 failing files as runs 27 to 29. Sorted file by file by scratch
`bin/classify30.py <runner log> <per-file logs> <vkd3d tests> <real logs>` (run30/classify-run30.txt):
it reads each file's "Missing capabilities/tags" skips against the file's `[require]` blocks and
real Direct3D 12's capability line (the rig's real logs, copied to run30/real-logs), the
sections needing shader models above 6.0, and the front's refusals. Each file, by cause:

Run 34 (run34/runner1.log, libraries at 91b4f491, 23:48 to 23:55, desktop shared, load not
taken): 214 of 302, 88 failing; the same 88 files fail at e2af024f (runner2.log) and at the
run's last code (runner3.log), unbounded-array-5.1 now stopping at `ResourceArray`. Newly passing: expr-indexing and hard-copy-prop (the shaders
track's `ConstantComponentIndexing`, merged this run). No file lost. Sorted file by file by
scratch `bin/classify30.py` (run34/classify-run34.txt), each "Missing capabilities/tags" skip
read against the file's `[require]` block by hand where the script found none. Each file, by
cause:

Run 36 (run36/runner1.log after the merge, runner3.log at the run's last code): 214 of 302, the
same 88 files. wave-ops-int moved from the converter refusals below to the 64-bit integers: with
the shaders track's run 40 it runs 413 of real's 668 checks, none failing, its 3 skipped
sections needing int64 (run36/logs-runner3). So "Run fewer" holds 60 files, "Fail some
checks" 28, and the converter refusals 20.

Run 37 and 38 (run37/runner9.log, run38/runner1.log and runner2.log): 214 of 302, the same 88
files. sm6-uav-rwtexture moved from the converter refusals to the shader models above 6.0: since
the shaders track's run 41 it runs 565 of real's 781 checks, none failing. The runner runs each
file once per shader model in its range (shader_runner.c, `run_shader_tests`' loop over models,
read in run 38), SM6.0 to SM6.5 on real Direct3D 12 and SM6.0 alone through ours, so the checks
it does not run are those of models 6.1 to 6.5; object-references (1048 of 1312) likewise. So
"Run fewer" holds 61 files, "Fail some checks" 27, and the converter refusals 19.

Run fewer checks than real, nothing failing (61):
- Shader models above 6.0 (section 9 answers 6.0; real's runner says "Compiling SM6.0-SM6.5",
  ours "SM6.0-SM6.0"), an absent feature, 50: sm6-uav-rwtexture (since run 37), barriers, barycentrics (6.1, and barycentrics
  absent), cast-to-half, cast-to-int, cast-to-uint, cbuffer, compute, conditional, coverage,
  descriptors (6.6), discard, early-depth-stencil (357 checks of real's 390), gather,
  gather-offset, getdimensions, half (6.2 and 16-bit types), initializer-objects, load-level,
  matrix-semantics, max-min, mesh (6.5 and mesh shaders absent), multiple-rt, numeric-types,
  numthreads, object-parameters, object-references, register-reservations-numeric, -profile,
  -resources and -space, rt-format-mismatch, rt-get-sample-info, sample-bias, sample-cmp,
  sample-grad, sample-level, sampler-offset, semantic-cascading, shader-interstage-interface,
  srv-byteaddressbuffer, srv-structuredbuffer, static-initializer, swizzle-constant-prop,
  texture-load-offset, texture-load-typed, uav-counter, uav-load, uav-out-param,
  wave-reconvergence. This cause is read from the runner's own lines (its model range against
  real's), not checked check by check.
- 64-bit integers absent (section 9; `[require] int64`): arithmetic-int-uniform,
  arithmetic-uint, bitwise, wave-ops-uint (its 14 skipped sections need int64; every other
  check passes), wave-ops-int (since run 36, above); with float64 absent too: cast-64-bit.
- Stencil export absent (`PSSpecifiedStencilRefSupported` false; WebGPU has no stencil
  export): stencil. Depth bounds absent: depth-bounds. Raster order views absent:
  rasteriser-ordered-views. The layer and viewport index from any stage absent: rt-array-index,
  vp-array-index.

Fail some checks (27):
- Absent features, 2: sample-minmax (minimum and maximum filters, refused by name); switch
  (its shader model 6.0 section switches on a `uint64_t` without requiring int64, which the
  device answers absent; refused by name, `Int64`).
- Not to be matched, 2: uav-atomics and length-property bind views of another kind than their
  shaders declare (a buffer for a `RWTexture2D`, which vkd3d's file itself notes; a structured
  view for a `RWBuffer`); Direct3D 12 leaves that undefined; ours removes the device, real draws.
- Refused by the front by name, waiting on G6, 1: texture-load (lines 235 to 267: a texture read
  as another type than its view; needs the converter to read the slot in the view's type; and
  line 145, a value, below).
- G6, a converter feature refused by name ("a ... shader did not convert: unsupported: <name>"),
  19: calculate-lod (`CalculateLevelOfDetail`), clip-cull-distance-1 (`CullDistance`,
  `ClipDistance` in geometry shaders, `DxilGeometryShader`), clip-cull-distance-2
  (`ClipDistance` read by a pixel shader), eval-attrib (DXIL `InterpolateAt`, "Draw failed" at
  lines 378 and 419; and line 105, a value, below), gather-cmp (`DepthGatherComponent`,
  `SwizzledDepth`), geometry (`DxilGeometryShader`), primitive-id (`PrimitiveId`), sampler
  (`SampleClamp`), unbounded-array-5.1 (`ResourceArray`, shader model 5.1 DXBC; it
  stopped at `TypedBufferFormat`, the front's own numbering of shader model 5.1 slots, until run
  34's second run), srv-buffers, tgsm, uav-rwbuffer and wave-ops-float
  (`TypedBufferFormatKind`, a typed view of another kind than the declaration, such as an
  `R8G8B8A8_UINT` view read as `Buffer<snorm float4>`, which Direct3D gives as the view's bits;
  the list's device is removed and the test then crashes),
  tessellation and tessellation-patch-vars (`DxilHullShader`), tiled-status (`SparseFeedback`),
  uav-rwbyteaddressbuffer, uav-rwstructuredbuffer and uav-rwtexture (memory operands wider than
  the workgroup, `globallycoherent`); wave-ops-int until run 36 (wave minimum and maximum of
  integers of the other signedness, landed in the shaders track's run 40).
- G6, a converted program giving other values, no refusal possible in the front (each asked of
  the shaders track by name, "Asked of other tracks"), 3 files and the value lines of two
  above: arithmetic-float-uniform (lines 359, 381, 408 and 430) and minimum-precision (line
  216), every shader model 6 program reading `min16float` constants: the test writes them as
  16-bit values and the converted program reads 32-bit ones; frexp (line 57 in shader model
  4.0, a subnormal input, 0x0007ffff, which Direct3D flushes to zero and the converted program
  does not, giving (-126, 0.53) for (0, 0); line 52 in shader model 6.0, a NaN input,
  0x7fc90fdb, for which Direct3D gives the exponent 129 and the NaN's mantissa and the converted
  program zeros); eval-attrib lines 105 to 108 (shader models 5.0 and 6.0): a centroid
  `SV_Position` in a pixel shader running per sample reads another position than Direct3D's
  (the file's resolved output (500.25, 875.25, 0.25, 0.25) where Direct3D gives (0.25, 0.25,
  0.25, 0.25)); texture-load line 145 (the shader model
  4.0 multisampled load in a loop reads texel (0,1) at (1,0); the 4.1 programs over the same
  texture read right).

Earlier classifications (run 33's, the independent check of run 33 corrected): wave-ops-int,
wave-ops-uint and wave-ops-float were written as running fewer checks because `WaveOps` followed
a `SUBGROUPS` the WebGPU back end did not declare; that was wrong: the back end has declared
`SUBGROUPS` since its run of g2c 2, `WaveOps` is answered 1, and run 33's own log had all three
failing checks (the converter items above). eval-attrib was written as waiting on
`InterpolateAt` and `PackedStageValues` for DXBC; both were built and merged by run 33, and its
first failure was already line 105.

Run 33 (run33/runner1.log, libraries at b436401c): 212 of 302, 90 failing. The 7 files passing
since run 31 (color-clamping, entry-point-semantics, instance-id, interface-packing, loop-sm3,
many-semantics, return-semantics) came with run 32's shaders merge (run32/base1-runner.log, before
run 33's changes, has the same 90 failing; run33/fail32base.txt and fail33.txt). early-depth-stencil
runs every check through ours but those of shader models above 6.0 (357 of 390, nothing failing;
with `EarlyTestsPerPrimitive`, run33/runner2.log), rt-format-mismatch stops at line 53 (above).

Run 31 (run31/runner1.log, libraries at 12966001): 205 of 302, the same 97 failing files as run
30 (the sets compared file by file). The wave operation files still run fewer checks: the
WebGPU back end now asks for subgroups (its run of g2c 2), but this desktop's run gave the same
verdicts; not looked into further this run.

History: 194 of 299 (runs 12, 13), 204 of 299 (runs 14, 15: the device now matches the
system's adapter by LUID, so the runner keeps its vendor tag), 204 of 302 (runs 17 to 26), 205
(runs 27 to 31), 212 (run 33), 214 (run 34). Run 27 (run27/runner*.log): arithmetic-float newly passing. The whole set's job took 246 s in run 27,
4.2 min in run 28, 6.5 min in run 29 and 14.5 min in run 30, its files' printed times alike
(the desktop's load not taken; the cause of the longer jobs not found).

## Scenes (the rig's Direct3D 12 scenes through our libraries)

Run 38 (scratch run38/scenes1.log and scenes2.log; `bin/scenes38.sh <n> [scenes]`): 95 outputs
pass, 22 fail, 3 absent; the 9 textures outputs pass newly, every other output as in run 35.
Run 35 (scratch run35/scenes2.log, logs in scenes-logs1, pictures in scenes-out1; `bin/scenes35.sh
<n> [scenes]`): 86 outputs pass, 32 fail, 3 absent. Bundles and constants pass since run 35 (root
bindings kept, short constant buffer views). Each failure:
- textures: passes since run 38 (run38/scenes1.log; its first output, a DXIL `Texture1D`, was
  refused by name until the shaders track's run 42 made DXIL 1D textures 2D ones of one row).
- raster (4 missing): a pixel shader the converter refuses ("spirv-reader: unsupported builtin");
  geometry (7), streamout (3), tessellation (1): `DxilGeometryShader` and `DxilHullShader`, the
  shaders track's (all of this rig's shaders are DXIL).
- topology: 43 of 65,536 pixels beyond 1; every one in a cell of lines (lists, strips, their
  adjacency forms, indexed strips; counted per cell from the pictures), none in the triangle and
  triangle adjacency cells: the aliased line rasterisation listed loss (Departures, plan 5.2).
- samplers-filters (7,680 pixels beyond 8 of 6,004 allowed, in the cells of `MaxAnisotropy` 2 and
  16) and msaa-alpha_to_coverage (3,222 of 2,580): the front hands both to the back end as
  Direct3D gives them (`state::sampler`, the blend block's alpha-to-coverage). Measured in run 36
  (scratch run36/scenes1.log, the same front with the device thread on wgpu's Direct3D 12 path,
  run36/dx12-path.patch): samplers-filters, msaa-alpha_to_coverage and topology then draw with
  largest difference 0, every other scene output as on the Vulkan path (scenes2.log). The three
  differences are the NVIDIA driver's Vulkan path, which this library draws on ("Next" 3).
- Absent (3): `B5G6R5` targets and the format support line (absent formats), minimum and maximum
  filtering.

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
  37-gpudrivenrendering (runs 25 to 28). Times against real in the same job: 37 2.6 and 2.5 s
  against 2.4 s twice (run 28: 748 to 801 pixels beyond 8, allowed 20480; run28/p2-bgfx37.log),
  2.5 s twice against 2.4 s twice (run 27);
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
- Run 34 (at 91b4f491, run34/p1-*.log, scratch `bin/real34.sh 1`): Microsoft's two samples
  equal (0 pixels beyond 8; 4.2 and 2.9 s); Godot's 2D platformer equal (0 of 518,400 pixels
  beyond 8 in frames 10, 60 and 120; 4.5 s); bgfx 37 every frame within its allowance in both
  runs (738 to 901 pixels beyond 8 of 20,480 allowed; 2.6 and 2.5 s against real's 2.4 s
  twice). Godot's 3D platformer now gets further and crashes (0xc0000005 after 16.5 s, no frame):
  with the shaders merge its fragment shaders convert, and then (1) bind groups were refused by
  wgpu's validation ("Texture binding 32 expects dimension = D2, but given a view with
  dimension = D2Array"), a shader resource view of another dimension than the program declares,
  which Direct3D leaves undefined and the front now refuses by name; (2) a fragment shader
  sampling a depth texture through a view with a component mapping is refused by the converter
  (`SwizzledDepth`, G6), the device is removed, Godot's buffer creations then fail
  (DXGI_ERROR_DEVICE_REMOVED) and Godot crashes in its own code. Its other refusals: 3
  `ResourceArray` and 2 `StorageFormatKind` compute shaders and one alpha-antialiased line
  pipeline, and `QueryVideoMemoryInfo` E_NOTIMPL 663 times.
  At e2af024f (run34/p2-*.log, the view dimension refused by name): Microsoft's two samples
  equal (2.9 and 2.5 s), Godot's 2D platformer equal (4.5 s), bgfx 37 every frame within its
  allowance twice (788 to 841 pixels beyond 8; 2.7 and 2.5 s against real's 2.4 s twice). Godot's
  3D platformer now stops at that refusal (twice; "a shader resource view of another dimension
  than the program declares"), the device is removed and it crashes after 7.6 s. Which view
  and declaration these are is not yet found: whether Godot binds a 2D array view where its
  shader declares a 2D texture (as real Direct3D 12 tolerates on this GPU) or the converted
  program declares another dimension than the DXIL does ("Next").
  At the run's last code (run34/p3-*.log, typed views read as raw or structured buffers):
  Microsoft's two samples equal (2.6 and 2.5 s), Godot's 2D platformer equal (4.1 s), bgfx 37
  every frame within its allowance twice (871 to 908 pixels beyond 8; 2.5 s twice against
  real's 2.4 s twice); Godot's 3D platformer as at e2af024f (7.4 s, the same refusal).
- Run 31 (desktop shared, not `--alone`, load not taken): bgfx 26-occlusion, which the
  independent check of run 30 found differing in frame 10 (91 passing pixels against real's
  92, and a cube of the inset), drew every frame within its allowance in both of our runs
  (run31/bgfx26-2.log: 1069 and 1073 pixels beyond 8 in frame 10, allowed 21760, none beyond
  their tiles' allowance), and this time real Direct3D 12 itself drew frame 10 differently from
  run to run (85 tiles, 87 pixels outside the ignored ones): what frame 10 shows depends on
  which frame's occlusion results bgfx has read back by then, on real Direct3D 12 as on ours.
  Frames 60 and 120 equal.
  At 9467a66f (run31/p1-*.log, scratch `bin/final31.sh 1`): Microsoft's D3D12HelloTriangle and
  D3D12HelloConstBuffers equal (0 pixels beyond 8); Godot's 2D platformer equal (0 of 518,400
  pixels beyond 8 in frames 10, 60 and 120; 4.1 s); bgfx 37-gpudrivenrendering every frame
  within its allowance in both runs (831 to 887 pixels beyond 8 of 20,480 allowed, the
  overlay's text). Godot's 3D platformer runs 120 frames (7.1 s) and still draws white: with
  the shaders track's run 32 its 22 fragment shaders no longer stop at "inconsistent comparison
  sampling" but at naga's validation of the converted program ("malformed: Invalid access into
  expression [N], indexed: true", refused by name as a shader the converter got wrong; G6,
  "Asked of other tracks"); its other refusals are 3 `ResourceArray` and 2 `StorageFormatKind`
  compute shaders, and `QueryVideoMemoryInfo` answers E_NOTIMPL 890 times.
- Run 30 (libraries at 2c38f81, desktop shared, not `--alone`, load not taken; run30/p1-*.log,
  scratch `bin/programs30.sh 1`): Godot's 2D platformer equal (0 of 518,400 pixels beyond 8 in
  frames 10, 60 and 120; 7.1 s). Its 3D platformer runs 120 frames (6.5 s) and draws white
  (518,400 pixels beyond 8): with the shaders merge `IsHelperInvocationEXT` converts, and 22
  fragment shaders now stop at naga's SPIR-V reader's "inconsistent comparison sampling" (one
  texture both compared and sampled plainly; G6, "Asked of other tracks"); its other refusals
  are 3 `ResourceArray` and 2 `StorageFormatKind` compute shaders and one alpha-antialiased line
  pipeline, and `QueryVideoMemoryInfo` answers E_NOTIMPL 892 times. None of run 30's new
  refusals appears in either platformer.
  bgfx 37-gpudrivenrendering every frame within its allowance in both runs (728 to 904 pixels
  beyond 8 of 20,480 allowed, the overlay's text; 2.6 and 2.5 s against real's 2.6 and 2.4 s).
- Run 29 (at 83d4085, desktop shared, not `--alone`; run29/p1-*.log): Microsoft's two samples
  equal (2.6 and 2.5 s), bgfx 37 every frame within its allowance twice (803 to 852 pixels
  beyond 8 of 20480 allowed; 2.5 s twice against real's 2.4 s twice), Godot's 2D platformer
  equal (0 pixels beyond 8, 4.7 s), its 3D platformer still white (38 fragment shaders
  `IsHelperInvocationEXT`, G6). Scratch `bin/programs29.sh 1` and `bin/programs22.sh`.
- Godot's 2D platformer (run 28, run28/p3-godot.log, at 0e3cd52; scratch `bin/programs28.sh`):
  frames 10, 60 and 120 equal the references, 0 of 518,400 pixels beyond 8, 120 frames in
  4.2 s (one run, desktop shared). Before, every frame was black (runs 23 to 28, 8.7 to 9.3 s):
  its textures allowing unordered access were made with `ALLOW_SHADER_ATOMICS`, which the
  front refused (E_INVALIDARG, 795 times in the 3D platformer's run), and Godot drew without
  them. It still has refusals, all G6's, that do not reach these frames: 2 fragment shaders
  `IsHelperInvocationEXT` (naga's SPIR-V reader), 3 compute shaders with `globallycoherent`
  views ("memory access 0x28 outside workgroup memory") and 1 "image load operand 0x200", 2
  `StorageFormatKind`, 1 `ResourceArray` (an array of storage views), and one pipeline
  drawing alpha-antialiased lines (`LINE_COVERAGE` and the stage helper's rectangles).
  `QueryVideoMemoryInfo` answers E_NOTIMPL 426 times. With the cache file (run 26,
  `bin/warm26.sh`) its file was 334,080 bytes after the first run and the same after the
  second: nothing converted again (8.2 then 7.9 s, single runs, before the fix).
- Godot's 3D platformer (run 28, run28/p3-*.log at 0e3cd52): with mappings applied in the
  program it no longer stops at its `R8G8B8A8_UNORM_SRGB` view of mapping 0x1a88 (runs 25 to
  27: the device was removed there and Godot crashed at 6.7 s); it runs 120 frames in 6.1 s,
  every frame white (518,400 pixels beyond 8). 39 graphics pipelines are refused, 38 of them
  for fragment shaders using `IsHelperInvocationEXT` (G6); Godot then draws with no pipeline
  (339 "No render pipeline was set"). Its other refusals are the 2D platformer's.
- Learnt on the way, which explains the setup: the rig's forwarding library takes the name
  d3d12.dll and loads ours behind it (`WC_D3D12`), hence `wcontainers_d3d12.dll` and two
  forwarders (run 21); it aborts a program calling an export the library behind it lacks
  (Godot's `D3D12GetInterface`, run 23); bgfx calls `IDXGIAdapter3::QueryVideoMemoryInfo` on
  the adapter `EnumAdapters` gives without asking for that interface (adapters are
  `IDXGIAdapter4` since run 18); NVIDIA's NVAPI calls into our device as if it were the
  driver's (harmless, absent in a container; `bgfx-run.ps1 <name> <seconds> ours nonvapi`
  keeps it out); a Rust panic in the library shows as 0xc0000409.

## Gap list (the pass bar's written list, as of run 39)

Every failing test file, shader runner file and real-program difference is one of: an absent
feature (reported absent as Direct3D 12 expects, or refused by name where Direct3D 12 has no
query), a listed loss naming what the back end lacks (counted and warned), not to be matched
(real Direct3D 12 crashes, or the behaviour is undefined), a named item of another track that
the front reports by a named refusal until it lands or that gives another value in a converted
program, or the front's own unfinished part. Every refusal below prints its name in the test's
log ("wcontainers: ..."); none passes as success. Run 36's test files (12 failing at the run's
last code, run36/full3.log and logs-full3/, the same 12 as run 35; and the same 12 at run 37's
code, run37/full1.log and full9.log, at run 38's, run38/full1.log and full2.log, and at run 39's,
run39/full1.log and full2.log):

Absent features, 8 files (final):
- Tiled resources (`TiledResourcesTier` 0): create_reserved_resource, get_resource_tiling.
- Logic operations (`OutputMergerLogicOp` false): output_merger_logic_op, and
  create_graphics_pipeline_state, which makes a pipeline with one without asking and then uses
  the null pipeline.
- Float64 in shaders (`DoublePrecisionFloatShaderOps` false): shader_instructions.
- Resource binding tier 2 and descriptor arrays where the back end lacks big tables (tier 1
  answered): unbounded_resource_arrays, unbounded_samplers.
- The layer from the vertex shader
  (`VPAndRTArrayIndexFromAnyShaderFeedingRasterizerSupportedWithoutGSEmulation` false, plan
  4.12): ps_layer (its context makes such a pipeline before asking; refused
  `RenderTargetArrayIndex`).
- Also reported absent with no failing test file: conservative rasterisation, raster order
  views, ray tracing, mesh shaders, variable rate shading, sampler feedback, 64-bit integers and
  depth bounds (answered absent whatever the back end has, section 9), timestamps where the
  adapter lacks `timestamp-query`, drawing into and storing into `B5G6R5`, `B5G5R5A1`,
  `B4G4R4A4`; minimum and maximum filters (no query in Direct3D 12: Microsoft's `D3D12_FILTER`
  page sends the reader to Direct3D 11's `MinMaxFiltering`), refused by name at use.

A listed loss, 1 file:
- execute_indirect, 2 checks: WebGPU skips an indirect draw that fetches past a vertex buffer's
  end whole, where Direct3D 12 reads zeros (Vulkan's `robustBufferAccess2`; WebGPU has no such
  fetch). Direct draws are exact (`VertexFetchZeroed`). Every `ExecuteIndirect` draw without
  indices fetching per-vertex streams is counted and warned (`IndirectFetchPastEnd`; the test's
  log: "indirect draw skipped whole if it fetches past a vertex buffer's end"). Departures.

Not to be matched, 1 file:
- copy_block_compressed_texture: real Direct3D 12 crashes in it (its texture of part of a block
  is refused on both; the test then uses the null texture).

Named items of the shaders track (G6), 2 files (each item is in the shaders track's own status
or in "Asked of other tracks"). One is refused by name until the item lands:
- `ResourceArray` (arrays of views in shader model 5.1 DXBC, "a Compute shader did not convert:
  unsupported: ResourceArray"): resource_arrays.
One is a converted program giving another value, with no refusal possible in the front:
- shader_eval_attribute, 1 check (d3d12.c:34067, the `ps_eval_sample_index` program, DXBC
  `eval_sample_index`): `EvaluateAttributeAtSample` gives -1.16e-10 where real Direct3D 12 gives
  exactly 0 at pixel (15, 0) (runs 35 and 36, run36/logs-full3). The value is the converter's (its sample
  position arithmetic; "Asked of other tracks").

Passing since run 35: clip_distance (the shaders track's run 38, clip distances through the
pass-through, with the merge alone) and combined_clip_and_cull_distances (its run 39's
`Stages::culled`, which the front now routes vertex shaders writing cull distances through).

Passing since run 34: register_space (a typed view bound where the program declares a raw or
structured buffer is now read as its elements' bytes, as the hardware addresses them, instead
of refused; with the 5.1 numbering above). buffer_srv. The shaders track's run 37 gave raw and structured views of
shader resource slots their stride and `STORAGE_OFFSET`, and the front dropped its refusal
(views bound from below their start now count `StorageOffsetInShader`).

Wine's dxgi tests (dxgi_test.exe; the rig's hardware baseline has 23 tests, 3 failing on real),
since run 35 (the independent check of run 34 showed that five of them make no Direct3D 10 or 11
device and give verdicts through this track's dxgi.dll alone):
- Counted, run 35 (run35/dxgi1.log, logs-dxgi1): test_create_factory (52 checks; it failed
  before run 35: a factory from `CreateDXGIFactory` offered `IDXGIFactory1`),
  test_factory_check_feature_support (23) and test_colour_space_support (89) pass.
  test_enum_warp_adapter and test_output_desc fail on one absent feature: no software adapter.
  Real DXGI enumerates Microsoft's Basic Render Driver as a second adapter and gives it from
  `EnumWarpAdapter`; the back end offers no software adapter, so ours enumerates one adapter
  (test_output_desc runs 38 of real's 45 checks and fails none; the 7 missing are, as read from
  the test's source, not measured, those of the loop over a second adapter) and `EnumWarpAdapter` answers E_NOTIMPL (a null out pointer gives
  DXGI_ERROR_INVALID_CALL as real, since run 35; the test then crashes using the adapter it did
  not get). Fixed in run 35 from the check: adapters hold their factory and outputs their adapter
  (`GetParent` gives them back, the reference counts the test reads), and `CreateDXGIFactory`'s
  factory refuses `IDXGIFactory1` as Windows 10's does.
- Outside the counted set, with the reason: the other 18 tests make Direct3D 10.1 or 11 devices
  through the system's d3d10_1.dll and d3d11.dll, which load this dxgi.dll and get adapters they
  cannot make devices on (this dxgi.dll serves Direct3D 12 only; the Direct3D 11 track ships its
  own). Their verdicts become comparable when one dxgi.dll serves both fronts (plan section 10's
  one library, `crates/wcontainers-directx`, "Next"). Their Direct3D 12 runs on real Direct3D 12
  also hung this desktop's GPU six times on 2026-11-17 (the rig's status, "display resets").

The shader runner (vkd3d-shader_runner, 302 files): see "Shader runner" for every failing file
and its cause.

The real programs: Microsoft's D3D12HelloTriangle and D3D12HelloConstBuffers, bgfx 01-cubes,
26-occlusion, 35-dynamic, 37-gpudrivenrendering and 38-bloom, and Godot's 2D platformer draw as
real Direct3D 12 does. bgfx 24-nbody draws differently on every real run, so it has no frames to
compare. Godot's 3D platformer (run 38) converts every shader and stops at one item of the WebGPU
back end: Godot reads an `R8G8B8A8_UNORM_SRGB` shader resource view of a texture that also allows
unordered access, and the back end makes every view with the texture's usages, which wgpu refuses
for a format that cannot be stored ("Texture view format `Rgba8UnormSrgb` cannot be used as a
storage binding", run38/p2-godot-platformer-3d-d3d12-output.log; every bind group holding that
view is then invalid). Direct3D 12 allows it; the back end would make such a view with the
texture's usages less storage ("Asked of other tracks"). With that change applied for one run and
not committed (scratch run38/srgb-view-usage.patch), it drew frames 10, 60 and 120 with 1 of
518,400 pixels beyond 8 each, largest difference 48 (run38/g3d3.log). Its one pipeline drawing
alpha-antialiased lines (`AntialiasedLineEnable` without multisampling), refused by name until
run 41, is now drawn through the stage helper's covered rectangles (`AntialiasedLinesAsQuads`,
the shaders track's `LINE_COVERAGE`; own case `antialiased-lines`). With the back end patch
applied again for one run (run 41, run41/g3d1.log), frames 10, 60 and 120 were as in run 38: 1
of 518,400 pixels beyond 8 each, largest difference 48 at 921,317, and nothing of ours refused
but `QueryVideoMemoryInfo`; the line pipeline is now made but draws in none of the 120 frames
(its warning, "antialiased lines drawn as covered rectangles", is not printed), so it is not
that pixel. The pixel is not traced.

The rig's Direct3D 12 scenes: see "Scenes" for every failing output and its cause (shaders
track items, and three differences measured in run 36 to be the driver's Vulkan path: the
aliased line listed loss, anisotropic filtering and alpha-to-coverage; the front's own open
item, "Next" 3).

Refused by name in the front with no failing test here (run 35 adds the first four, of the
routes it built for what the independent check of run 34 found refused and unlisted; the
refusal of a texture the converted program declares one-dimensional was deleted in run 38, as the
converter now makes DXIL 1D textures 2D ones of one row; alpha-antialiased lines straight from
the vertex shader are drawn since run 41): alpha-antialiased lines after a geometry or
tessellation stage or stream output, or of wireframe triangles (the stage helper's line route
takes the vertex stage's lines alone); a pixel shader reading its primitive index where the back
end lacks `Features::PRIMITIVE_INDEX` (since run 41, as the Direct3D 11 front refuses it; not
reached on this desktop, whose back end has it; the stage helper's `primitive_numbered` route is
"Next"); a constant buffer
view shorter than the program's block, or passing its buffer's end, of a buffer outside an upload
heap (the GPU may write it, and its zero tail would need a copy before the pass; upload buffers
are served from the program's bytes, `ConstantBufferGrown`); indexed or indirect adjacency draws
without a geometry shader (direct ones drop the adjacent vertices, `AdjacencyDropped`); early
tests in the program per primitive of such a draw; sampling in the program, typed views of
other formats, component mappings and stencil views in geometry, hull and domain shaders and
vertex shaders before them; multisampled arrays of several layers and the minimum
level-of-detail clamp in those and in vertex shaders; storage texture format variants with
pulled vertex programs or geometry stages; stream output buffers not starting at a multiple of
256 bytes; unordered access buffers in CPU-visible custom heaps; comparison sampling of a texture
whose bits match no depth format and of a multisampled one; a buffer view starting inside one of
the program's elements; textures read as another type than their view's format (Direct3D
returns the view format's values and the shader takes their bits; needs a variant reading the
slot in the view's type, such as `decoded`); early tests in the program of a draw writing depth
or stencil (`EarlyTestsPerPrimitive`), behind a geometry or tessellation stage, or on a depth
buffer denying shader resources, and per primitive those listed under "Early tests in the
program"; draws split per layer or viewport while a query runs, into target views of different
layer counts or into a 3D texture's view; shader resource and (since run 36) unordered access
views of another dimension than the program declares, but one-layer 2D arrays bound to 2D
declarations, which are bound as 2D views.

## Asked of other tracks

Run 38: the shaders track's run 42 settled run 37's Godot `StorageFormatKind` item (views the
program never uses kept their slots; every Godot shader now converts with the front's keys) and
DXIL one-dimensional textures (2D textures one texel high; the front's refusal is deleted). Asked
newly or again, with run 38's evidence:
- WebGPU back end (G2), Godot's 3D platformer, again and now its only blocker: make each view with
  the texture's usages less those the view's format lacks (storage for sRGB formats), as
  WebGPU's `createView` takes them. Measured with exactly that change (scratch
  run38/srgb-view-usage.patch, `raw_views` in lib.rs, applied for one run, never committed):
  Godot's 3D platformer then drew frames 10, 60 and 120 with 1 of 518,400 pixels beyond 8 each
  (run38/g3d3.log).
- WebGPU back end (G2), on wgpu's Direct3D 12 path (clear_unordered_access_view_image there,
  run 37's trace): `ClearValue::Color` is the clear's raw bits, and that path's clear flushes an
  `R32_FLOAT` denormal (bits 0x1) to 0. Asked: where the API clears through a float that does
  not keep the bits (denormals, NaN payloads), clear another way (a copy from a buffer filled
  with the texel, or a draw writing the bits, as `IntegerClearByDraw` does for integers).
- Shaders (G6), clip distances through the culling stage: `Stages::culled`'s pass-through writes
  the vertex program's clip distances as a built-in, and takes no variant; on a back end without
  `Features::CLIP_DISTANCE` (wgpu's Direct3D 12 path, WebGL 2) the front refuses such pipelines
  by name. Asked: apply `VariantFlags::CLIP_DISTANCE` with `linked` to that pass-through as the
  geometry stage's pass-through takes it (combined_clip_and_cull_distances on that path).

This track cannot change another's branch or status; these are the items its gap list waits on,
read against origin/phase-2-graphics-shaders at its run 37 (aa4a1f46, merged in run 34), each
with its evidence in scratch run34/ (logs-full1, logs-runner1). Landed since: `SwizzledDepth` for
mappings keeping red first and the wave signedness item (shaders run 40, merged in run 36; the
runner's wave-ops-int still fails, now past the converter), the public vertex counts (imported in
run 36).
- Shaders (G6), refused by name at conversion: `ResourceArray` in shader model 5.1 DXBC
  (resource_arrays, runner unbounded-array-5.1; DXIL arrays are built); `TypedBufferFormatKind`
  (runner srv-buffers, tgsm, uav-rwbuffer, wave-ops-float); `ClipDistance` read by a pixel
  shader (runner clip-cull-distance-2); DXIL `InterpolateAt` (runner
  eval-attrib lines 378 and 419); `DxilGeometryShader`, `DxilHullShader` (runner geometry,
  tessellation, tessellation-patch-vars, clip-cull-distance-1); `CalculateLevelOfDetail`;
  `DepthGatherComponent` and `SwizzledDepth` of a mapping moving red (gather-cmp); `PrimitiveId`; `SampleClamp`;
  `SparseFeedback` (tiled-status); memory operands wider
  than the workgroup (`globallycoherent`: uav-rwbyteaddressbuffer, uav-rwstructuredbuffer,
  uav-rwtexture). Most are in the shaders track's own list ("The Direct3D 12 track's converter
  items still open"); the two typed buffer kinds were named here first in run 34.
- Shaders (G6), values a converted program gives where Direct3D gives others (no refusal is
  possible): DXBC `eval_sample_index` gives -1.16e-10 for 0 (shader_eval_attribute,
  d3d12.c:34067); a centroid `SV_Position` in a per-sample pixel shader (runner eval-attrib lines
  105 to 108, shader models 5.0 and 6.0); `min16float` constants of shader model 6 programs held
  as 16-bit values (runner arithmetic-float-uniform lines 359, 381, 408 and 430,
  minimum-precision line 216); 32-bit float subnormals flushed to zero as shader models 4 and 5
  do (runner frexp line 57); DXIL `frexp` of a NaN (runner frexp line 52); the shader model 4.0
  multisampled load in a loop (runner texture-load line 145; the shaders track's run 32 found
  fxc's bytecode converts right, so the runner's bytecode should be dumped).
- Shaders (G6), needing the interface: a texture slot read in its view's type and handed to a
  program declaring another (runner texture-load lines 235 to 267; the front refuses it by name,
  `read_as` in bind.rs).
- WebGPU back end (G2), on wgpu's Direct3D 12 path (run 37): buffer-to-texture copies of
  several slices or layers whose `rows_per_image` exceeds the copy's height read the slices at
  the wrong pitch (wgpu-hal 30 sets the placed footprint's `Height` to the copy's height); copy
  slice by slice there (copy_buffer_texture, "Next" 3b).
- WebGPU back end (G2), Godot's 3D platformer (run 36; run 38 above): `raw_views` makes every
  view with `usage: None`, which wgpu takes as the texture's usages, and wgpu refuses a view whose
  format cannot be stored on a texture allowing storage ("Texture view format `Rgba8UnormSrgb`
  cannot be used as a storage binding"). Direct3D 12 reads an sRGB shader resource view of a
  texture that allows unordered access in its UNORM twin.

## Next, in this order

1. At the start of each run merge the shared line and every graphics branch that has something
   new, and origin/phase-1-literals; keep `cargo run -q -p wcontainers-literals -- check`
   passing for this track's crates (run through `remote`); after a merge that brings a new
   version of its rules, run `init` before committing the merge or in the commit right after.
   Check whether the rig fixed its comparison (`tools/graphics-rig/desktop/wine-run.ps1`, "How
   to run"); until it does, apply the one-line patch uncommitted for the run and never commit
   that file.
2. The independent check of G5 (everything on the pass bar is done or on the gap list). Then,
   after each shaders merge, rerun everything (scratch bin/all39.sh <n>: the literal check,
   lint, the crates' tests, every test file, the own program, the shader runner, the scenes,
   the five dxgi tests, the real programs and the `--alone` fence timing), reclassify the runner
   (bin/classify30.py) and read the refusals. The front's part as other tracks' items land: a
   texture of another type than its view: replace the refusal in `bind.rs` (`read_as`) by the
   variant reading the slot in the view's type; a pixel shader reading its primitive index where
   the back end lacks `Features::PRIMITIVE_INDEX` (browsers' WebGPU without `primitive-index`):
   route it through `Stages::primitive_numbered` (the shaders status, "for the fronts") as the
   line route is routed, in place of run 41's refusal, once there is a way to compare it with
   real Direct3D 12 on a back end without the feature (this desktop's has it). When the WebGPU
   back end's view usage lands, rerun Godot's 3D platformer (with run
   38's patch it drew 1 pixel beyond 8 per frame; trace that pixel then).
3. Draw on wgpu's Direct3D 12 path, as the Direct3D 11 track does and as browsers draw on
   Windows. Run 36 built it (scratch run36/dx12-path.patch: `system_on_device_thread!` sends the
   device thread's `CreateDXGIFactory2`, `D3D12CreateDevice`, `D3D12GetDebugInterface`,
   `D3D12GetInterface` and both `D3D12Serialize*RootSignature` to the system's libraries;
   `Backends::DX12`; a warning where wgpu finds no dxcompiler.dll and compiles with FXC; the own
   program's scripts copy the Windows SDK's dxcompiler.dll and dxil.dll beside it) and measured
   it (run36/*2.log): scenes samplers-filters, msaa-alpha_to_coverage and topology largest
   difference 0; own cases 28 of 28, shader runner 214 (the same files), dxgi tests the same. Not
   kept, for what it broke, each of which is the next step:
   a. Clip distances: done in run 37 (clip_distance passes on that path); the culling stage's
      pass-through waits on the shaders track ("Asked of other tracks"), so
      combined_clip_and_cull_distances stops at a named refusal there.
   b. Traced in run 37 (run37/files5.log, logs-files5); both are the WebGPU back end's (run 38,
      "Asked of other tracks"):
      - copy_buffer_texture (2 checks, d3d12.c:27057 and 27083): a 4 by 4 by 4 box copied from a
        buffer footprint 100 rows high into a 3D texture reads its second slice from row 4 of
        the first (got 0x00010501 for 0x00020101 at (2, 2, 3)). wgpu-hal 30's Direct3D 12 path
        makes the copy's placed footprint `Height` the copy's height
        (`to_subresource_footprint` in wgpu-hal's dx12/command.rs), so `rows_per_image` is not
        the slice pitch there; the front passes the footprint's height as `rows_per_layer`. An
        item of the WebGPU back end (copy slice by slice where `rows_per_image` exceeds the
        copy's height on that path) or of wgpu upstream; "Asked of other tracks".
      - clear_unordered_access_view_image (44 checks): every failing check is a
        `ClearUnorderedAccessViewUint` of an `R32_FLOAT` view with the bits 0x1, a denormal
        (tests 0, 1 and 4 of its table; float-valued clears pass). The front clears textures
        with the bits as a colour (`ClearValue::Color`), and the clear on the Direct3D 12 path
        flushes the denormal to 0, where Direct3D 12 writes the bits. The interface's
        `ClearValue::Color` is the clear's raw bits, so keeping them is the back end's part, as
        it already clears integers its API would round by a draw (`IntegerClearByDraw`); run 37
        had written it as the front's, with a new slow path (Departures, run 38).
   c. The real programs run behind the rig's forwarding library, which is the program's
      d3d12.dll; on the device thread wgpu then loads it too, so it wraps wgpu's own device
      (Microsoft's samples: "census has no command-list SetGraphicsRootSignature") and, asked by
      Direct3D 12's runtime for `GetBehaviorValue`, aborts (Godot and bgfx exit 0xc0000409). The
      rig's forwarder would need to pass the device thread's calls to the system untouched (an
      ask of the rig track), or the library would load the system's d3d12.dll for wgpu by path.
4. Plan 4.8's call buffer, with plan section 10's one library (`crates/wcontainers-directx`,
   the Direct3D 11 track's shells): move this track's COM objects there when it carries both
   fronts, which also lets the other 18 dxgi tests be counted (Departures).
5. For G6 (seen in run 29's vertex-views case): a DXBC vertex shader reading a `Buffer<float4>`
   converted with its slot's placeholder `R32_FLOAT` texel format and `DrawBaseSource::Arguments`
   fails naga's validation ("'texels_0_32_load' ... Index 3 is out of bounds"), where the same key
   with `DrawBaseSource::Helpers` converts; such pipelines' `ExecuteIndirect` draws then read their
   arguments back (`IndirectReadBack`, exact, a wait).
6. Later: render pass resolves of part of a subresource and of depth-stencil targets; the
   built-in storage clear for UAV clears of formats that do not render
   (`builtin::storage_clear`); constant buffer views shorter than the program's block in
   default heaps (a zero-tailed copy before the pass).
7. The literal sweep, a part per run ("No magic numbers" below).

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
- Direct3D's shared values (`d3d` module, run 39): `PipelineStatistics` and
  `StreamOutputStatistics` (the query data, `repr(C)`, `bytemuck::Pod`), `APPEND_ALIGNED_ELEMENT`,
  `SO_NO_RASTERIZED_STREAM`, the placement alignments.
- Indirect arguments (run 43, `d3d` module): `DrawArguments`, `DrawIndexedArguments`,
  `DispatchArguments` (`repr(C)`, `bytemuck::Pod`), for argument sizes and reading arguments back.
  The depth-stencil view's read-only flags (`D3D11_DSV_READ_ONLY_DEPTH`, `_STENCIL`, the same
  values as Direct3D 12's) are not there yet: adding them makes the literal check flag your
  copies, so add them with your import in one commit; this front's `list.rs` will then import them.
- Format queries: `format_support(caps, DXGI_FORMAT)` gives `support1` and `support2`; empty means
  the API's E_FAIL. `takes_samples(caps, format, count)`; `dxgi_vertex_format(DXGI_FORMAT)`.

## How to run

- Tests (one draws on the GPU): `tools/graphics-rig/gpu-job --build "cargo test -j 3 -p
  wcontainers-directx-graphics-infrastructure -p wcontainers-direct3d-12 --no-run" -- "cargo test
  -j 3 --no-fail-fast -p wcontainers-directx-graphics-infrastructure -p wcontainers-direct3d-12"`
  (46 tests: 10 infrastructure, 9 front, 11 device on the recorder, 1 cache, 1 on the GPU, 14
  unit; run 28: all pass, run28/test1.log). Put `set CARGO_INCREMENTAL=0&& ` before both commands (see the note further down).
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
- One case of the own program: `check_d3d12 case <name>` (names in `examples/check/cases.rs`);
  both ways on the desktop: `tools/graphics-rig/gpu-job --desktop --build "set
  CARGO_INCREMENTAL=0&& cargo build -j 3 -p wcontainers-direct3d-12 --examples" --
  "crates\wcontainers-direct3d-12\examples\check_case.cmd <name>"` (scratch `bin/case29.sh <log>
  <name>`). Do not change the working tree while a rig job of it waits: the job refuses a tree
  other than the one sent. Cases compile their HLSL with the system's `D3DCompile`
  (d3dcompiler_47.dll, part of Windows) where no compiled shader fits.
- Run 33's scripts (scratch bin/, logs in run33/): `full33.sh <n>` everything at once as
  `full30.sh` below; `real33.sh <n>` the real programs (Microsoft's samples, Godot's two
  platformers, bgfx 37); `lt33.sh <n>` lint and the crates' tests; `files33.sh <n> <files>` builds
  and runs named test files; `runfiles33.sh <n> <files>` named shader runner files
  (`vkd3d-shader_runner:hlsl+early-depth-stencil.shader_test/...`).
- Everything at once (run 30): scratch `bin/full30.sh <n>` lints (with the interface crate),
  runs the crates' tests and the interface's slow path tests, every test file, the own program
  and the shader runner into run30/*<n>.log; `bin/programs30.sh <n>` the real programs;
  `bin/classify30.py` sorts the runner's failing files (Shader runner); run30/every.cmd (copied
  to C:\wc\out\phase-2-graphics-direct3d-12, run by `gpu-job`) runs named runner files on real
  Direct3D 12 and through ours with every check reported (`VKD3D_TEST_DEBUG=2`), from the
  libraries the last `wine-tests run` put in its bin folder. Copy them under a new run's name
  for the next run (`sed s/run30/run31/`).
- Everything at once (run 28): scratch `bin/full28.sh <n>` runs the crates' tests, every test
  file, the own program and the shader runner into run28/*<n>.log (about 15 minutes with the
  build warm, 40 from clean); `bin/programs28.sh <n>` runs Godot's two platformers and bgfx 37
  (copy them to a new run's folder name first, `sed s/run28/run29/`); `bin/named28.sh <n>
  <files>` runs the crates' tests and the named test files; `bin/rw27.sh <n>` lints, builds the libraries
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
- The format answers (`format_support`, shared with Direct3D 11) give `SHADER_LOAD` for the
  stencil view formats, which the front reads since run 28 (pixel and compute shaders).

## No magic numbers (the owner's rule of 2026-11-18)

Run 43: the check passes at 4518fd0e (run43/lit5.log, lit1.log). Named this run (fix-graphics.md
items 11, 17 and the shader program text): the indirect argument layouts as `repr(C)` mirrors in
the infrastructure crate's `d3d` module (`DrawArguments`, `DrawIndexedArguments`,
`DispatchArguments`), with `list::VertexBufferView` and `IndexBufferView` made `repr(C)` and
`Pod`; `IndirectArgument::size` and `read_back_indirect` take sizes and fields from them
(`bytemuck::pod_read_unaligned`), and a predicated draw's record is `DrawIndexedArguments`'s size;
`address::GpuVirtualAddress` (root views) and `address.rs`'s alignment from
`DEFAULT_RESOURCE_PLACEMENT_ALIGNMENT`; the own compute programs' bindings from the interface's
numbering (`OWN_SOURCE`, `OWN_RESULT`, `OWN_PARAMETERS` from `SlotKind::first_binding`), their
group size (`OWN_GROUP`) and entry (`wcontainers_shaders::ENTRY_POINT`, also for the counting
programs), their WGSL written with `format!` from those names and from `IndexFormat`'s sizes and
restart values; the index formats by `device::index_format` (the DXGI names) and the restart
decision once (`restart_rewritten`, `STRIP_CUT_0XFFFF`), used by `draw`, `ExecuteIndirect` and the
statistics' strip count; the query answer, predicate and command count sizes from their types;
`D3D12_QUERY_HEAP_TYPE` 4 and 7 named; the uniform block rounded to `CONSTANT_REGISTER`. 58 lines
deleted. Not done: the depth-stencil view's read-only flags in the `d3d` module (both fronts
define them with the same values): adding them there makes the check fail on the Direct3D 11
front's copies (they become redefinitions of the new constant, `+` lines this branch may not add
or fix), so they wait for the Direct3D 11 track, which can add them to the `d3d` module and import
them in one commit ("For the Direct3D 11 track"). Baseline lines at the end of run 43:
wcontainers-direct3d-12 471, the infrastructure crate 63. Changes that could add lines: the same
18 as run 35.

Run 41: the check passes at 731791fe (run41/lit1.log). The front's topology types and fill modes
are named once in `state.rs` (`TOPOLOGY_TYPE_UNDEFINED` to `_PATCH`, `FILL_MODE_WIREFRAME`,
`_SOLID`, after Direct3D 12's names; a module `topology_type` with `UNDEFINED` was caught as a
redefinition of the Direct3D 11 crate's `topology::UNDEFINED`, `D3D_PRIMITIVE_TOPOLOGY`'s, a
different fact). 4 lines deleted. Baseline lines at the end of run 41: wcontainers-direct3d-12
529, the infrastructure crate 63. Changes that could add lines: the same 18 as run 35.

Run 39: the check passes at 8ffdc980 (run39/lit2.log, exit 0). Named this run, from
fix-graphics.md items 3, 8 and 9 (whose owner is the infrastructure crate's `d3d` module): the
pipeline and stream output statistics' layouts (`PipelineStatistics`, `StreamOutputStatistics`;
the front's field positions, value sizes 8, 88 and 16 and the field size are gone),
`APPEND_ALIGNED_ELEMENT`, `SO_NO_RASTERIZED_STREAM`, `SMALL_`, `DEFAULT_` and
`DEFAULT_MSAA_RESOURCE_PLACEMENT_ALIGNMENT` (resource.rs, the device test; the Windows library and
the own program use the windows crate's `D3D12_DEFAULT_RESOURCE_PLACEMENT_ALIGNMENT`), and the
own program's readback pitch (`D3D12_TEXTURE_DATA_PITCH_ALIGNMENT`). 31 lines deleted. The Direct3D
11 front still defines its own copies of these; they are its to replace by the `d3d` module.
Baseline lines at the end of run 39: wcontainers-direct3d-12 533, the infrastructure crate 63.
Changes that could add lines to the baseline: the same 18 as run 35, none this branch's own.

Run 38: the check passes (run38/lit1.log after the merges, lit6.log at deee93a2). The merges'
two baseline conflicts (neighbouring shaders crate lines both sides removed) were resolved by
keeping neither. Named this run: the storage clears' component bit widths (`components`, `mask`,
`small_float`, `small_float_value`, `storage_clear_value` in device.rs) by `u8::BITS`,
`u16::BITS`, `u32::BITS` and `HALF_BITS` (from `half::f16`'s size), and the small floats' shift by
`HALF_MAGNITUDE_BITS`; 20 lines deleted. The packed widths of `R10G10B10A2` and `R11G11B10`
(10, 11 and 2) stay: the interface's `Format` names no component widths. Baseline lines at the
end of run 38: wcontainers-direct3d-12 564, wcontainers-directx-graphics-infrastructure 63.
Changes that could add lines to the baseline: the same 18 as run 35, none this branch's own.

Run 31, the first part, following orchestrator/magic-literals/fix-graphics.md (item 1, the
DXGI format table, whose owner is the infrastructure crate): a `dxgi` module in the
infrastructure crate names every `DXGI_FORMAT` value once (dxgiformat.h's names), and
`dxgi_format`, `dxgi_vertex_format`, `BACK_BUFFER_FORMATS`, the format answers, `plane_count`
and the new `typeless_group` use it; the front imports it where it named formats itself
(`resource.rs`'s plane formats, `alias.rs`'s depth formats, the index formats in `device.rs` and
`geometry.rs`, `R1_UNORM` in `quality_levels`). Also named: feature level 11_0's sizes
(`LEVEL_11_0_TEXTURE_*`), `SUPPORT1_TYPED_BUFFER` from the support bits, the sample counts
Direct3D asks about (`SAMPLE_COUNTS`), the formats whose sampling is optional
(`OPTIONAL_SAMPLE`). The test of the format answers imports the names.
Left: a grep of numeric literals other than 0, 1 and 2 (comments left out; named constants'
own definitions counted) gives examples/check/cases.rs 488 (mostly test data), format.rs 147
(the `dxgi` definitions and vertex sizes, which `VertexFormat` has no size method to derive),
device.rs 117, examples/check_d3d12.rs 111, state.rs 102, dxbc.rs 98, root.rs 87, bind.rs 84,
resource.rs 64, dxil.rs 42, query.rs 41, support.rs 37 (mostly the support bits' definitions),
objects.rs 27, indirect.rs 24, geometry.rs 23, pull.rs 22, swapchain.rs 21, the rest under 20.
Run 33, the second part: `D3D12_DSV_FLAG_READ_ONLY_DEPTH` and `_STENCIL` (`list.rs`, used
where the replay read 1 and 2), `D3D12_CONSTANT_BUFFER_DATA_PLACEMENT_ALIGNMENT`
(`resource::CONSTANT_BUFFER_ALIGNMENT`, in `stored_size`), the interface's `STORAGE_WORD` for
storage ranges and scratch sizes rounded to words (bind.rs, pull.rs, query.rs, geometry.rs,
device.rs), the pixel counter's size from `u64` (`statistics::COUNTER_BYTES`), and the
pass-through block's size read from its module. Copy sizes rounded to 4 (WebGPU's copy
alignment) and the bytes-per-row 256 stay: the interface names neither yet.
Run 36: the check passes (exit 0, run36/lit1.log after the merges and lit3.log at the run's last
code): the front imports the shaders crate's now public `TRIANGLE_VERTICES`,
`LINE_ADJACENCY_VERTICES` and `TRIANGLE_ADJACENCY_VERTICES` (the adjacency route's own `line * 2`
and `TRIANGLE_VERTICES * 2` are those counts) and its `state::TRIANGLE_VERTICES` is deleted. The
merge's one baseline conflict (a shaders crate line both sides changed) was taken as the shaders
branch has it. Baseline lines at the end of run 36: wcontainers-direct3d-12 592,
wcontainers-directx-graphics-infrastructure 63 (unchanged; the deleted constant was a `+` line,
never in the baseline). Changes that could add lines to the baseline: the same 18 as run 35,
none this branch's own. No sweep step this run.
Run 35: the check prints one line not in the baseline (`+`): `state::TRIANGLE_VERTICES` (named in
run 33 in `early.rs`, moved to `state.rs` in run 35 to serve the adjacency route too) is now a
redefinition of the shaders crate's crate-private `TRIANGLE_VERTICES` (its run 39), which this
crate cannot import; asked of the shaders track ("Asked of other tracks"). Nothing else: the run
named the view dimensions (`list::SRV_DIMENSION_*`, `UAV_DIMENSION_*`), the adjacency topologies
(`state::TOPOLOGY_*_ADJ`), the constant register size (`CONSTANT_REGISTER`), and deleted 36
lines the merges no longer have and 30 of this track's. Baseline lines at the end of run 35:
wcontainers-direct3d-12 592, wcontainers-directx-graphics-infrastructure 63. Changes that could
add lines to the baseline (18, lit2.log): run 34's 14 plus four the merges brought, none this
branch's own: 53789ba39b78 (start) and cd5f44a16180 (init) of the shaders branch's adoption,
bd53fbb448e2 and 949667f217bd (new rules, a merge into webgl and the init after the seventh
rules). The desktop's
copy C:\wc\src\phase-2-graphics-direct3d-12 now has `core.autocrlf` false and its nested
submodules (`dxbc-spirv/submodules/spirv_headers`, `dxil-spirv/subprojects/dxbc-spirv`,
`dxil-spirv/third_party/SPIRV-Cross`, `SPIRV-Tools`) deinitialised, as the baseline expects.

Run 34, the automatic check adopted (docs/literals.md): merged origin/phase-1-literals at
7668c010 (through the webgpu, webgl and direct3d-11 branches first; their baselines conflicted
and both sides' lines were kept, as the document says). `init` was then refused ("init writes
the baseline once"): the baseline had arrived here by those merges, whose own inits had already
scanned this track's crates, so this branch has no init of its own. The check then printed 13
lines not in the baseline (`+`): 4 in this track's crates, all from run 33's code (`3` three times
in `early.rs`, now `TRIANGLE_VERTICES`; the own case's `SIDE = 4`, named like the WebGPU back
end's private `SIDE`, now `DEPTH_SIDE`), and 9 of the shaders track's (its `build.rs`, `dxil.rs`,
a test, and 4 nested submodules the desktop's copy had not checked out; the submodules were
checked out recursively there, C:\wc\src\phase-2-graphics-direct3d-12). Baseline lines of this
track's crates at adoption: wcontainers-direct3d-12 643 (517 numbers and 33 strings in code, 66
restated, 23 redefinitions, the rest roles and a script), wcontainers-directx-graphics-infrastructure
63; at the end of run 34, 622 and 63 (21 lines deleted: the DXBC token fields and the
multisampled array dimension named); at the end of run 37, 584 and 63 (run 36's end 592: the 8
lines of the DXIL reader's resource fields, now named `SPACE`, `LOWER_BOUND`, `RANGE_SIZE`,
`KIND`, `SRV_EXTENDED`, `UAV_EXTENDED`). Changes that could add lines to the baseline, as the check printed them: 55435e4096b9,
4bcacbf5c5b9, c33381def450, 68214b2eec10 and 81489a2f8fab (starts), ca2eb3e82b59, 2cc8ad761bf6,
1e3377067526 and 5657abb6ba15 (inits after the baseline arrived by a merge), 89f882810517,
7119e3f3a527, c3bf56d4ed9f, 31ec396f9a4a and 0f1875633508 (new rules); none is this branch's own
commit. Run: `tools/graphics-rig/remote "set CARGO_INCREMENTAL=0&& cargo run -j 3 -q -p
wcontainers-literals -- check"`.
The check then fails on this branch with the shaders track's new literals alone (its
build.rs `"."`, dxil.rs `3` in `Resource`, compute_draw.rs `512` twice and `2048`; "Asked of
other tracks"): this track may neither name them nor add their lines. Lines no longer found (37,
all of other tracks' crates, whose code the merges brought already fixed) were deleted. Named
this run in this track's crates: the per-primitive triangle size (`TRIANGLE_VERTICES`), the own
case's `DEPTH_SIDE`, the DXBC token fields `dxbc.rs` reads (`OPCODE_TYPE_MASK`, the resource
dimension and operand index fields, the extended bit, the return types, the three view
declarations it newly reads), `SRV_DIMENSION_2D_MS_ARRAY`, and the new slow path's reason (a
constant in the interface). Not done from the independent check's list: `features.rs`'s wave
lane counts (4 and 128) and `resource_heap_tier: 2`, and 40 there; they are baseline lines
(Direct3D's own values, which no crate this branch uses names) and stay for the sweep.
Next, in the fix list's order: Direct3D 12 placement alignments, WebGPU alignments and limits,
byte widths, then the Direct3D enum values; facts another crate owns stay until its constant is
on a branch this one merges.

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
- `SlowPath::IndirectFetchPastEnd` (run 30, commit f6c1fb4 "interface: named the loss of an
  indirect draw without indices skipped whole when it fetches past a vertex buffer's end"; plan
  4.12's vertex input paragraph says it): the listed loss of an `ExecuteIndirect` draw without
  indices reading past a vertex buffer's end, counted at each such draw that fetches per-vertex
  streams (only the GPU knows its count). The interface's own test of the reasons' numbers holds
  it (`cargo test -p wcontainers-gpu --lib slow`).
- `SlowPath::StorageOffsetInShader` (run 34, commit 86f21b4f "interface: named the slow path of
  a storage view bound from below its start with the rest added in the shader"; plan 4.5's
  storage buffer paragraph says it): a raw or structured view bound below its start, with the
  rest added to every address in the program (`VariantFlags::STORAGE_OFFSET`), counted at each
  such binding. Before, that route had no slow path.
- `ShaderModule::from_wgsl(source, entry, stage)` (run 5, commit "interface: made a shader module
  from one entry point of a wgsl module..."): parse, validate and key one entry point of a WGSL
  module, as the built-ins are keyed. The infrastructure crate's present copy uses it instead of a
  builder of its own. G1's `helpers.rs` still has its own copy of the same steps (`module`/`program`);
  it could call this (G1 owns helpers.rs, so this track did not change it).
  That commit (4f1894c) also deleted this track's `wgsl.rs` in the infrastructure crate, which
  section 12 keeps out of interface commits; history is not rewritten, so it stays as made.

## Measurements

- Run 43, fence-wait (own program, 2000 rounds, RTX 3080) as `gpu-job --alone` at 4518fd0e's
  code (run43/alone1.log, 14:17; desktop CPU load 1 % before and 0 % after, Win32_Processor
  LoadPercentage): real first 7 µs, mean 1 µs, 99th percentile 2 µs, largest 14 µs; ours first
  108 µs, mean 20 µs, 99th percentile 59 µs, largest 117 µs. Run 42 at 236fd376
  (run42/alone1.log, 13:50; load 1 % before, 2 % after): real mean 1 µs, 99th percentile 1 µs;
  ours first 42 µs, mean 17 µs, 99th percentile 26 µs, largest 61 µs. Programs (not alone,
  shared desktop, load not taken): Microsoft's samples 2.6 and 2.5 s, Godot's 2D platformer 4.3 s
  (run43/p1-*.log).

- Run 41, fence-wait (own program, 2000 rounds, RTX 3080) as `gpu-job --alone` at 731791fe
  (run41/alone1.log, 13:02; desktop CPU load 4 % before, Win32_Processor LoadPercentage; the load
  after not printed): real first 6 µs, mean 1 µs, 99th percentile 1 µs, largest 6 µs; ours first
  45 µs, mean 19 µs, 99th percentile 62 µs, largest 113 µs. Programs (not alone, shared desktop,
  load not taken): Microsoft's samples 2.9 and 2.6 s, Godot's 2D platformer 4 s (p1-*.log).

- Run 39, fence-wait (own program, 2000 rounds, RTX 3080) as `gpu-job --alone` at 8ffdc980
  (run39/alone2.log, 11:50; desktop CPU load 1 % before, Win32_Processor LoadPercentage; the
  load after not printed): real first 7 µs, mean 1 µs, 99th percentile 1 µs, largest 7 µs; ours
  first 103 µs, mean 17 µs, 99th percentile 19 µs, largest 103 µs. At d0b08b61
  (run39/alone1.log, 11:17; load 2 % before and after): ours first 65 µs, mean 19 µs, 99th
  percentile 51 µs, largest 81 µs; real mean 1 µs. Programs (not alone, shared desktop, load not
  taken): Microsoft's samples 2.6 s each, Godot's 2D platformer 4.1 s (p2-*.log).

- Run 38, fence-wait (own program, 2000 rounds, RTX 3080) as `gpu-job --alone` at d5881c24
  (run38/alone2.log, 10:13; desktop CPU load 1 % before and 1 % after, Win32_Processor
  LoadPercentage): real first 8 µs, mean 1 µs, 99th percentile 2 µs, largest 26 µs; ours first
  69 µs, mean 17 µs, 99th percentile 19 µs, largest 72 µs. At 436a4509 (run38/alone1.log, 09:34;
  load 2 % after, the load before not printed): real mean 1 µs, ours mean 17 µs, 99th percentile
  19 µs. Godot's 3D platformer with run 38's two changes (one of them the WebGPU back end's,
  applied for the run): 120 frames in 44 s (run38/g3d3.log, not alone, load not taken; the 2D
  platformer takes 3.8 s); the time is not traced.

- Run 37, fence-wait (own program, 2000 rounds, RTX 3080) as `gpu-job --alone` at 0b231aa9
  (run37/alone1.log, 08:13; desktop CPU load 1 % before and 2 % after, Win32_Processor
  LoadPercentage): real first 7 µs, mean 1 µs, 99th percentile 1 µs, largest 67 µs; ours first
  43 µs, mean 17 µs, 99th percentile 19 µs, largest 69 µs. Test files 4.7 min as one queued job
  (07:38:07 to 07:42:47, not alone).

- Run 36, fence-wait (own program, 2000 rounds, RTX 3080) as `gpu-job --alone` at 0b7b2f36
  (run36/alone3.log, 07:05; desktop CPU load 2 % before, Win32_Processor LoadPercentage; the
  load after was not printed, the chain stopping at the script's exit code): real first 8 µs,
  mean 1 µs, 99th percentile 1 µs, largest 8 µs; ours first 42 µs, mean 17 µs, 99th percentile
  19 µs, largest 76 µs. The same `--alone` on wgpu's Direct3D 12 path (run36/alone2.log, 06:35,
  load 2 % before): ours first 35 µs, mean 14 µs, 99th percentile 16 µs, largest 103 µs; real
  7, 1, 1, 7 µs. Programs (not alone, shared desktop, load not taken): Microsoft's samples 2.6 s
  each, Godot's 2D platformer 3.8 s (p3-*.log).

- Run 35, fence-wait (own program, 2000 rounds, RTX 3080) as `gpu-job --alone` at ebc55743
  (run35/alone2.log, 05:14; desktop CPU load 1 % before and 2 % after, Win32_Processor
  LoadPercentage): real first 7 µs, mean 1 µs, 99th percentile 1 µs, largest 7 µs; ours first
  40 µs, mean 17 µs, 99th percentile 19 µs, largest 40 µs. Not alone, the same code
  (check2.log, desktop shared, load not taken): ours mean 18 µs, 99th percentile 58 µs. The cost
  is the boxed call per COM call (Departures). The programs' times under "Latest results" are
  not alone (shared desktop, load not taken).
- Run 34, fence-wait (own program, 2000 rounds, RTX 3080, desktop shared with other tracks, not
  `--alone`, load not taken; run34/check1.log at 91b4f491): real first 9 µs, mean 1 µs, 99th
  percentile 1 µs, largest 20 µs; ours first 68 µs, mean 40 µs, 99th percentile 97 µs, largest
  147 µs. The independent check of run 33 measured ours at mean 17 µs and 99th percentile 19 µs
  against real's 1 and 2 µs (its check.log), on the same desktop at another load; the spread is
  the load, the cost the boxed call per COM call (Departures). Test files 7.7 min as one queued
  job (23:24:08 to 23:31:50), shader runner 6.7 min (23:48:40 to 23:55:22). At e2af024f
  (check2.log): ours first 44 µs, mean 17 µs, 99th percentile 25 µs, largest 73 µs; at the
  run's last code (check3.log): ours first 102 µs, mean 18 µs, 99th percentile 49 µs, largest
  155 µs; real mean 1 µs both times. Own cases 28 of 28 equal at all three code points.

- Run 31, fence-wait (own program, 2000 rounds, desktop shared with other tracks, not
  `--alone`, load not taken; run31/final1-check.log): real first 8 µs, mean 1 µs, 99th
  percentile 1 µs, largest 8 µs; ours first 98 µs, mean 25 µs, 99th percentile 30 µs, largest
  98 µs (an earlier run the same hour, run31/check2.log: ours mean 18 µs)
  (the call per COM call to the device thread, Departures).

- Run 30 (2026-11-18, RTX 3080, desktop shared with other tracks, not `--alone`, load not
  taken; run30/check1.log at 418cba9): own program 26 of 26 cases equal; draw 0 of 4096 pixels
  beyond 1; features 253 of 872, outputs 22 of 110, answers 7 of 40 differ, none beyond real.
  Fence events (2000 rounds, as below): real first 7 µs, mean 1 µs, 99th percentile 1 µs,
  largest 7 µs; ours first 45 µs, mean 17 µs, 99th percentile 28 µs, largest 100 µs. The
  independent check of run 29 measured ours at mean 39 µs, 99th percentile 60 µs, largest 169
  µs on the same desktop at another load (its check.log), about twice; the spread is the
  desktop's load, the cost the boxed call per COM call (Departures). Test files 80 s (10:20:24
  to 10:21:44), shader runner 14.5 min (10:22:31 to 10:37:03; 6.5 min in run 29, 4.2 in run 28,
  the files' printed times alike; cause not found). At c9d8c7f (run30/check2.log, 12:05): 27 of
  27 cases equal; fence events real 7, 1, 1, 7 µs, ours first 88 µs, mean 18 µs, 99th
  percentile 48 µs, largest 88 µs. Microsoft's samples (run30/samples2.log) 2.7 and 2.5 s, equal.
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
  to 17, 268 in run 18, 253 in runs 19 to 30, 192 since run 31, with one beyond real since run 31
  (`R32G32B32_FLOAT` sampled and gathered, "CheckFeatureSupport"); run 10 briefly answered
  `R1_UNORM`'s quality levels where real fails, fixed that run.
- Run 3: the triangle scene through `Device<WebGpu>` on wgpu's Vulkan and Direct3D 12 paths,
  largest difference 0 (desktop CPU load 14%). Run 4: the triangle and swapchain scenes and the
  own program equal real Direct3D 12 (largest difference 0, CPU load 13%).
- Line lists (measured by the independent check, its own program in scratch `check/own`, mode
  `lines`): 8 aliased `LINELIST` segments on 64x64, real against ours "59 of 4096 pixels beyond
  1, largest difference 255". A listed loss, below.
- Real programs' times: under "Real program".

## Departures from the plan

- Run 41: the own program's cases are compared line for line, exactly; `antialiased-lines` is
  compared as a picture within 63 of 255 per channel instead. Direct3D 12 leaves the alpha-based
  line filter to the implementation (the RTX 3080 and WARP differ by up to 128, measured by the
  shaders track), so exact equality with one GPU is not Direct3D's verdict; the tolerance is the
  shaders track's for the same route, and an aliased line (rows of 255 and 0 where the RTX 3080
  gives 115 and 140) would exceed it (expected from those values, not measured).

- Run 38: run 37 had planned a uint clear of a float format whose bits a colour clear does not
  keep (the denormal on wgpu's Direct3D 12 path) as the front's, by a copy from a filled buffer
  with a new slow path in the interface. Not done: the interface's `ClearValue::Color` already
  promises the clear's raw bits, so a back end whose API loses them breaks its own contract, and
  a fix in each front would be parallel code for one back end's gap. Asked of the WebGPU back
  end instead, where `IntegerClearByDraw` is the same kind of fix for integers.
- Run 38: a texture the converted program declares as depth (compared somewhere in it) is filtered
  in the program wherever a linear non-comparison sampler also samples it, whatever its format:
  WebGPU's depth textures take no filtering sampler, while Direct3D 12 filters a depth view
  plainly sampled. The comparisons stay native.

- Run 35: a 2D array view of one layer bound where the program declares a 2D texture is bound as
  a 2D view of that layer. Direct3D leaves a view of another dimension than the declaration
  undefined; a one-layer array view holds exactly the texels of the 2D view, so nothing is
  guessed (Godot's 3D platformer binds such views). Views of several layers stay refused.

- Run 34: the literal check was adopted by merging origin/phase-1-literals itself (as the brief
  says), not through main as docs/literals.md's step 1 asks (main does not hold it); the webgpu
  and webgl branches did the same. So main is not merged here until it holds the check.
- Every COM call crosses to the device thread as a boxed closure and waits for its answer;
  plan 4.8's call buffer (calls written into a buffer, crossing once a frame) is not built. It
  costs on every call that waits (a fence round trip, Measurements). Why not here: the call
  buffer is the channel of plan section 10's one library of the Direct3D family
  (`crates/wcontainers-directx`), whose COM shells the Direct3D 11 track (G4) has built and
  whose call channel is to come with them (its crate's own words); building a second channel in this track's Windows library would be
  the parallel path the rules forbid. This track's part is to move its COM objects into that
  library when it carries both fronts ("Next").

- Indirect draws reading past a vertex buffer's end (since run 7): plan 4.12 made draws without
  indices exact there (`VertexFetchZeroed`), which the front does for direct draws. For
  `ExecuteIndirect` it does not: the draw's range is only in GPU memory, and WebGPU skips such an
  indirect draw whole. Exactness would need reading every command's arguments back (a wait per
  `ExecuteIndirect`) or fetching every stream of every indirect draw in the program; both slow
  every indirect draw of every program for a case only a program reading past its views reaches.
  Kept as a listed loss naming the back end's lack (gap list); since run 30 the plan says so and
  every such draw is counted and warned (`IndirectFetchPastEnd`, Interface changes).

- The Windows library is built as cdylib examples of `wcontainers-direct3d-12` (examples may use
  dev-dependencies, so the front never depends on a back end), not in `wcontainers-directx`, which
  the Direct3D 11 track now has on the shared line (Next 5).
- Calls do not go through the call buffer of plan 4.8, although section 10 has the first batch build
  the Windows shell with the channel in-process. Every COM call, `ExecuteCommandLists` with its whole
  replay, every `Create*` and every `Release` included, crosses to the device thread as a boxed
  closure and waits for the answer, so object creation waits for the device thread, against 4.8.
  Command list recording runs on the program's thread. A cost of run speed, not of correctness;
  Next 5. Since run 30 releases (`Release` of queues, fences, resources, heaps, query heaps,
  pipelines and descriptor heaps) are sent without waiting (`gpu::post`): faster, and a release
  during process exit, after Windows has ended the device thread, no longer waits for good (it
  hung a program freeing resources from a thread-local destructor; real Direct3D 12 exits).
  Swap chains still wait (the driver may message their window).
- The library draws on wgpu's Vulkan path: with our d3d12.dll loaded, wgpu's Direct3D 12 path would
  load it as the system's. A listed loss follows: Vulkan rasterises aliased lines differently from
  Direct3D 12 on this GPU (Measurements); the Direct3D 11 track moved to wgpu's Direct3D 12 path for
  this. Points, strips and triangles were equal in the same check. Run 36 built and measured
  that path for this library (the device thread's calls sent to the system's libraries): the line,
  anisotropy and alpha-to-coverage differences are gone there, but it was not kept for what it
  broke ("Next" 3).
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
- Format support reports what the back end does (its converted formats included, run 19),
  typed buffer views through storage (`BUFFER`, `SHADER_LOAD`, since run 19), and since run 31
  the front's own routes: `SO_BUFFER` for the formats of 32-bit components
  (`StreamOutAsCompute`), `CAST_WITHIN_BIT_LAYOUT` for every member of a typeless group (twins,
  `ReinterpretCopy`), and sampling and gathering of depth views and of formats the back end
  cannot filter (`FilterInShader`). Still absent on WebGPU: 3D block-compressed textures and
  formats WebGPU lacks (the packed pairs, the video formats); `NormAsInteger` and
  `BlockDecoded` are not in the answers.
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
- The format answers follow the front's routes since run 31 ("CheckFeatureSupport"). Only this
  front calls the infrastructure crate's `format_support`; the Direct3D 11 front answers from
  its own.

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
- `ALLOW_SHADER_ATOMICS` (heap flag 0x400) is taken on committed resources allowing unordered
  access and refused (E_INVALIDARG) on other committed resources and on heaps, as this
  desktop's Windows 10 runtime does for committed resources (measured in run 28; heaps not
  measured, kept refused).

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
- `bytemuck` 1.25.2 (MIT OR Apache-2.0 OR Zlib, read in its Cargo.toml; already in the workspace
  through the WebGPU back end and the test crate), with its `derive` feature in the
  infrastructure crate (`bytemuck_derive` 1.12.1, MIT OR Apache-2.0 OR Zlib, already in the lock),
  for the query data structures' bytes (run 39).
- Test only: `wcontainers-gpu-webgpu`, `wgpu` 30.0.2 (MIT OR Apache-2.0), `pollster` (MIT OR
  Apache-2.0), `png` (MIT OR Apache-2.0), as the WebGPU track already uses them.

## Installed on the desktop

Nothing of this track is installed on the desktop beyond the rig's setup. What stays there:
- The shaders crate's submodules, checked out in this branch's source directory
  (C:\wc\src\phase-2-graphics-direct3d-12\crates\wcontainers-shaders: dxbc-spirv and
  dxil-spirv, 18,052,177 bytes, run 4; with their nested submodules, which the literal check
  reads, checked out recursively in run 34, 68,417,586 bytes including git's own files).
- In C:\wc\out\phase-2-graphics-direct3d-12: bgfx-run.ps1 (1 KB, run 16), bgfx-capture.ps1 (2
  KB) and bgfx-compare.ps1 (1 KB, run 21); bgfx's captured frames and logs under bgfx (about 12
  MB, bgfx's own art, kept as evidence); the rig's program and test outputs (programs, wine,
  check), which each run overwrites.
- The rig's build folder of this branch, C:\wc\target\phase-2-graphics-direct3d-12, is
  deleted at the end of each run (run 27: see below), so each run builds from clean (about 8
  minutes).

Run 39: nothing installed. Deleted at the end of the run: C:\wc\target\phase-2-graphics-direct3d-12
(8,976,912,215 bytes). A scratch git worktree (dev39, sources only, no build) was
made and removed again; no build output on the Mac.
Run 36: for the Direct3D 12 path's experiment the Windows SDK's dxcompiler.dll and dxil.dll
(14,261,232 and 1,503,656 bytes, copies of the SDK's own) were put in
C:\wc\out\phase-2-graphics-direct3d-12\dxc and beside the own program and bgfx; all were deleted
again the same run, and bgfx-capture.ps1 put back as it was. Deleted at the end of the run:
C:\wc\target\phase-2-graphics-direct3d-12 (8,405,526,107 bytes). No build output was made in scratch or on the Mac.
Run 29: nothing installed. Deleted at the end of the run: C:\wc\target\phase-2-graphics-direct3d-12,
and the checkout's own `target` (219 MB) that a `cargo check` on the Mac left (the shaders
crate's build script fails there; check on the desktop). Logs stay in scratch run29.
Run 28: nothing installed. Deleted at the end of the run: C:\wc\target\phase-2-graphics-direct3d-12.
No build folder was made on the Mac or in scratch; the logs stay in scratch run28, and Godot
4.7.2's `rendering_device_driver_d3d12.cpp` (MIT, read only, to find its heap flags) in
scratch godot-src.
Run 30: nothing installed. Deleted at the end of the run: C:\wc\target\phase-2-graphics-direct3d-12
(7.4 GB; the next build there starts from clean) and the run's temporary files under
C:\wc\out\phase-2-graphics-direct3d-12 (every\, two zip files). No build folder in scratch or
the checkout; the logs stay in scratch run30, with vkd3d's shader test files and real Direct3D
12's runner logs (LGPL test data, for testing only).
