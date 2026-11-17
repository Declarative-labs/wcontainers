# Graphics track status (interface, G1)

Plan: `docs/phase-2-graphics.md`. Brief: `pyro-parked/wcontainers-scratch/orchestrator/brief-graphics.md`.
This branch, phase-2-graphics, is the shared line: the interface track merges the other graphics
branches into it at the start and end of every run.

## Where things stand

- G0 is done (eight runs and eight independent checks on 2026-11-17; detail in the git log and the
  plan): the plan, and `crates/wcontainers-gpu` with the interface as real Rust.
- G1a (two runs, 2026-11-17) is done: the recording back end and the first 13 suite tests.
- G1, run 1 (2026-11-17), done and pushed, every item of section 12's pass bar met on the recording
  back end (item by item below), with two parts the pass bar does not reach left for later (Next):
  - the five findings of G1a's second check fixed (below);
  - in `wcontainers-gpu`: the pass tracker (`passes.rs`), the occlusion counter (`occlusion.rs`) and
    the combination tracker (`combinations.rs`);
  - in `wcontainers-gpu-test`: the call stream and its replay (`stream.rs`), the seeded random
    call-sequence generator (`sequence.rs`), storage buffer writes from fragments, the rule checks
    G1a left (usage flags, typeless groups, the no-overwrite promise, usage scope, attachment
    slices), and the tests: `tests/rules.rs`, `tests/fast_paths.rs`, `tests/helpers.rs`,
    `tests/sequences.rs`, and 8 new suite tests (21 now);
  - the six track branches merged at the start and again at the end.
- G1, run 2 (2026-11-17), done and pushed: the two parts run 1 left inside G1's scope.
  - The recording back end draws four samples (standard positions, per-sample depth, stencil,
    blending and occlusion counts, programs run per sample where they read the sample index or a
    value at the sample), resolves (at a pass's end and by `resolve`), copies multisampled textures
    whole, loads samples in programs, and checks the multisampled rules; it now declares
    `MULTISAMPLE_READ` and `SAMPLE_MASK`, and four samples on every format it draws into.
  - The pass tracker fits multisampled attachments of different sizes (plan 4.7): by a draw reading
    each sample where the back end has `MULTISAMPLE_READ` and `SAMPLE_SHADING`, otherwise through the
    resolved value (`MultisampleCopyResolved`); integer formats without the first are reported
    `Absent(MULTISAMPLE_READ)`. A new built-in program (`Builtin::SampleCopy`) does both.
  - The occlusion counter's count in halves past its format's bound is tested (on a recording back
    end without blendable `r32float`, so it counts in `r16float`), and so is its count of samples on
    a four-sample target.
  - New suite tests: `multisampled_targets_cover_their_samples_and_resolve`,
    `programs_run_per_sample_and_read_samples`, `occlusion_queries_answer_whether_samples_passed`
    (24 now). The random sequences now include multisampled passes resolved into the texture named.
  - `Recorder::with_caps` makes a recording back end with capabilities taken away, for the routes
    of helpers on back ends without them.
  - The direct3d-12 branch merged at the start (the others had nothing new); at the end webgpu,
    webgl and direct3d-12 had new work and were merged, with no conflict.

- G1, run 3 (2026-11-17), done and pushed: the seven findings of G1's independent check fixed
  (below). The start merges brought webgl, direct3d-11, direct3d-12 and shaders in without
  conflict; the shaders track added `VariantKey::output_interpolation` (kept: it serves every
  front). The end merges brought webgpu, webgl, direct3d-11 and direct3d-12 in without conflict; the
  webgpu track added `SlowPath::IntegerClearByDraw` (kept: a named slow path).

- G1, run 4 (2026-11-17): the five findings of G1's second independent check fixed (below). The
  start merges brought webgpu, webgl, direct3d-11, direct3d-12 and shaders in; one conflict, in the
  text of `TextureRegion::stencil` and plan 4.2, where the direct3d-11 track stated the same rule
  as run 3 (copies between textures move both aspects): kept one statement with both reasons.

## G1's second check, and how each finding was settled (run 4)

- Rule checks no test broke (blocking): every rule check of the recording back end is now broken by
  a test, shown by a sweep that turns each check off in turn and runs the crate's tests
  (scratch `g1r4/sweep.py`; Measurements). Run 4's first sweep, on the code the check saw, left 47 of
  101 checks (and parts of checks) unbroken, the 13 the check named among them; 4 more are match
  guards the sweep cannot turn off, each now asserted by message. New tests in
  `tests/rules.rs` for each: attachment views of one level and of one layer, a 3D slice past its
  level's depth, colour formats as the depth attachment and depth formats as colour, `empty` with
  attachments and neither, a pass ended with its query running, `end_pass` and `end_query` without
  their begin, present inside a pass, surfaces in a format without `PRESENT` and `current_texture`
  before configuring, regions past the last level, layer and slice and outside their level, pitches
  of texture writes and buffer-to-texture copies (both aspects), data shorter than the region,
  buffer copies, buffer-texture copies and clears past their buffers, clear patterns of 3 bytes,
  formats not held, empty textures, cube layers not whole cubes, view ranges past the layers or of
  no level or layer, loads naming a sample from a single-sampled texture and none from a
  multisampled one, bindings of the wrong kind of resource, vertices and indices past their
  buffers, draws with no viewport, scissor on with no scissor, streams without buffers or
  descriptions, fans without `FAN_AND_LOOP`, indirect draws, query sets of absent kinds,
  `read_queries` outside its set or inside a pass, depth bounds, border and mirror-once samplers,
  the raster block's absent features (polygon mode, depth clamp, clip depth from -1), storage
  bindings of buffers without `STORAGE`, `resolve` from one sample or into many, past its levels
  or layers or from another level or layer.
  - The multisampled no-storage case could not fail as written: the multisampled-shape check now
    comes before the format-use check, and the test asserts each refusal's message.
  - New checks with their tests: a draw writing stencil into a read-only stencil aspect (it was
    skipped silently); one subresource attached or resolved into twice (refused at `begin_pass`;
    the draw's own check is now unreachable); and the limits the recording back end declared but
    did not check: texture sizes (2D, cube, 3D, layers), vertex streams, attributes and stride,
    sampler anisotropy, the vertex program's outputs against `inter_stage_slots`, colour targets
    and their bytes a sample (counted by block bytes, the recorder's own measure), and uniform
    buffers, textures, samplers and storage buffers per stage across a draw's sets.
- Four-sample bound (minor): the comparison stays on a pixel's sum over its samples (it keeps the
  resolved average exact); the text now says so (plan 4.12 step 4, the counter's docs, Known
  failures). The same reasoning found a real fault: the depth copy was saved for halves only where
  the draws' primitives reached the bound, not primitives times samples, so on four samples the
  halves could start from the depth the whole left. Fixed, and tested by
  `the_occlusion_counter_keeps_the_starting_depth_for_halves_on_four_samples` (9,616; 0 before).
- `clear_rects` depth and stencil (minor): `clear_rects_writes_depth_and_stencil_by_draws`
  (`depth32float` rectangle at 0.25, the rest 0; `depth24plus-stencil8` stencil 7 in column 0, its
  depth word unchanged).
- Prepared combinations drawn before (minor): a combination remembers its draws when first
  prepared and counts as used only for draws after; tested by the check's own case.
- 3D attachment slice (minor): `a_3d_attachment_slice_lies_inside_its_levels_depth` (level 1 of a
  4-deep texture takes slice 1, refuses slice 2).

## G1's check, and how each finding was settled (run 3)

- Rules stated but not checked (blocking): the recording back end now reads its own `Features`,
  `Limits` and format uses and refuses what they leave out. New checks, each broken by a test in
  `tests/rules.rs`: blocks with `same_for_all` false without `INDEPENDENT_BLEND`; the constant alpha
  factors without `BLEND_CONSTANT_ALPHA`; per-face stencil masks (at creation) and references (at the
  draw) without `STENCIL_PER_FACE`; raster blocks needing `PROVOKING_VERTEX_FIRST` or `_LAST`,
  `DEPTH_BIAS_CLAMP`, `FRACTIONAL_DEPTH_BIAS`, `SAMPLE_MASK`, `CLIP_DEPTH_ZERO_ONE`, `WIDE_LINES`;
  uniform and storage bindings off their alignment, past their buffer or over their limit; writable
  fragment storage without `FRAGMENT_STORAGE_WRITE`, vertex storage reads without
  `VERTEX_STORAGE_READ`, multisampled textures bound without `MULTISAMPLE_READ`; texture usages the
  format does not take; viewports with `min_depth` above `max_depth`, fractional viewports without
  `FRACTIONAL_VIEWPORT`; bases without `BASE_VERTEX_INSTANCE`; 8-bit indices without `INDEX_U8`;
  immediates past the limit; blending into a format without `BLEND`, filtering one without `FILTER`;
  programs run per sample without `SAMPLE_SHADING`; resolves of formats without `RESOLVE` (now
  declared where it resolves) and copies out of formats without `COPY_OUT`; views of other shapes,
  and bound views not of all layers in the texture's own dimension, without `TEXTURE_VIEWS`.
- Occlusion counter not exact across buffer writes and target changes (blocking): it now counts the
  draws kept so far before each change. `OcclusionCounter::flush` is called by the front before any
  write, copy, clear, resolve or target change inside a counted query; the counter refuses a draw or
  an end on targets changed without it (`PassTracker::generation`). The depth copy is stored by the
  count, so it follows the target. Tests: `the_occlusion_counter_counts_across_writes_and_target_changes`
  (the check's two cases, and the refusal).
- Halves from discarded depth (minor): the copy is stored, and where the draws have as many
  primitives as the bound it is saved first and restored before the halves, which run one after the
  other. Strips without restart are split too (triangle strips at an even triangle). Test:
  `the_occlusion_counter_counts_halves_from_the_depth_they_began_with` (12,004, the restore broken
  gives 0; a 3,000-triangle strip counted as two). A single fan, strip with restart or indirect draw
  past the bound is still refused (Known failures).
- Function calls in programs (minor): the interpreter runs `Statement::Call`. Test:
  `generate_mips_averages_the_four_texels_below_on_every_route` (rgba8unorm through the sampler,
  r32float without `FILTER` filtered in the program, rgba16uint holding rgba16unorm values), each
  against the exact averages. The line loop default is shown by its recorded stream
  (`loops_become_line_lists_in_the_ring`); the recorder still draws no lines and says so.
- Stencil aspect ignored (minor): buffer copies and read backs of a format with depth and stencil now
  move the aspect named (stencil one byte, depth four); copies between textures move both
  (`copies_of_depth_and_stencil_move_the_aspect_named`). Stated in the interface (below).
- Features never read (minor): see the first item; `with_caps` now takes features away for real.
- Narrow random sequences (minor): ten formats (unorm, sRGB, BGRA, 8 to 32-bit float, unsigned and
  signed integers), passes with depth and stencil (both aspects read back), uniform binding sets at
  either aligned offset, indexed draws (16 and 32-bit, instanced from a first instance), scissors and
  occlusion queries read back; no-overwrite writes wait only where they would touch a range in use,
  so they land beside ranges unfinished commands use (a generator broken to never wait is caught).
  The test prints how many of each it made. Passes destroy their views, buffers and query sets
  right after recording, before any submit: work recorded before a destroy still uses them.
- Hot paths (minor): `CombinationTracker::combination` looks a combination up when the front's
  state changes and `drawn(Combination)` is one indexed add per draw; the pass tracker skips its
  usage-scope check while neither the pass nor the draw writes storage and keeps its lists sorted.

## G1's pass bar, item by item

- Every rule of section 4 checked by the recording back end, with a test that breaks it (the list
  under "The rules and their tests"), shown since run 4 by the sweep that turns each check off in
  turn (Measurements).
- Every fast-path default shown built from the base by a recorded stream: `tests/fast_paths.rs`, one
  test each for `read_back_now`, `prepare`, `clear_texture`, `clear_rects`, `generate_mips` (and
  its three routes by their averages), `draw_fan_or_loop` (fans drawn; loops by their stream, the
  recorder draws no lines), `write_buffer_ring` and `resolve_queries`; each asserts the calls
  recorded, by name, and the answer.
- The occlusion counter's exact counts on scripted draws with a counting program in WGSL:
  `the_occlusion_counter_counts_exactly` (draws that write depth counted against the depth copied at
  the query's begin; draws that do not, against the target read-only).
- The combination tracker's tuples (4.4) on a scripted sequence:
  `the_combination_tracker_picks_the_tuples_drawn` (a drawn vertex shader's tuples, the most drawn
  fitting tuple for a new one, ties to the smaller, the cache's record, prepared-then-drawn counts).
- The pass tracker fits multisampled colour targets of different sizes on both routes:
  `multisampled_targets_of_different_sizes_are_fitted_sample_by_sample` and
  `..._through_the_resolved_value` (each sample of the larger target read back by a program), and
  `integer_multisampled_targets_need_multisample_reads_to_be_fitted`.
- The occlusion counter past its format's bound: `the_occlusion_counter_counts_past_its_formats_bound_in_halves`
  (one draw's instances, one list's primitives, three draws, and below the bound counted once); on
  four samples: `the_occlusion_counter_counts_the_samples_of_multisampled_targets`.
- The pass tracker fits a larger depth buffer and colour targets of different sizes:
  `a_larger_depth_buffer_is_fitted` (temporary made once and pooled, copied in only for `Load` and
  out only for `Store`, scissors cut, depth outside the common area untouched) and
  `colour_targets_of_different_sizes_are_fitted`; and makes a clear the pass's load:
  `an_implicit_fronts_clear_becomes_the_passs_load`.
- The pass tracker splits a pass at a storage write followed by another use (`UsageScopeSplit`, the
  running query carried across as two interface queries), and the recording back end rejects the
  same draws in one pass: `a_pass_is_split_where_a_draw_reads_what_one_wrote_as_storage`.
- 100,000 random sequences, seeded, recorded and replayed identically (same stream with every
  answer, same bytes in every buffer and texture, same frames, no rule broken): in run 3, on the
  widened generator, seeds 0x5eed, 0xc0ffee and 0x7a11ed, 100,000 each (Measurements).

## G1a's second check, and how each finding was settled

- Texture-to-buffer copies at an offset over layers: `copies_move_bytes_exactly` now copies two
  layers at offset 12, pitch 20, three rows per layer, and checks the bytes between rows untouched.
- Truncating float-to-unorm conversions: `clears_fill_exactly` clears 0.25, 0.85, 0.55 (63.75,
  216.75, 140.25 steps) exactly; `the_scissor_bounds_the_draw` and `stencil_operations_and_tests`
  draw those values exactly.
- Perspective: `values_are_interpolated_with_perspective` (w from 1 to 4, one step of tolerance;
  screen-linear is up to 64 steps off).
- Untested draw behaviours: `the_scissor_bounds_the_draw`, `stencil_operations_and_tests` (replace,
  invert, increment, equal and not-equal), `near_clipping_and_the_viewport_depth_range`,
  `linear_filtering_and_address_modes` (clamp and repeat, weights, two steps of tolerance),
  `instances_start_at_the_first_instance`, `indexed_strips_restart_and_add_the_base_vertex` (16 and
  32-bit, index offset, `first`, restart, base vertex; first instance and base vertex only where
  `BASE_VERTEX_INSTANCE` is declared).
- `create_blend` refuses second-source factors with `Absent(DUAL_SOURCE_BLEND)` at creation
  (`blocks_needing_absent_features_are_refused_at_creation`).
- All 13 faults of the check's script (`g1a-check2/breaks.py`; scratch copy `g1/breaks.py`, results
  `g1/breaks-run1.log`) now fail the suite on the recording back end; before, 9 passed it.

## The rules and their tests

Each rule a caller can break is refused (a violation that loses the device, or `GpuError` at
creation) and broken once in a test (`tests/rules.rs` unless named):
- draws inside a pass (`draws_happen_inside_a_pass`); passes do not nest; submits outside a pass;
  copies, clears and read backs outside a pass (also `copies_and_read_backs_inside_a_pass_are_refused`
  in `tests/recorder.rs`);
- attachments of one size and depth aspects of the format (`tests/recorder.rs`); one level and layer
  with render usage; a 3D slice inside its level;
- one usage scope per pass (`tests/helpers.rs`); a draw reads no subresource it writes;
- queries: inside a pass with a set, one at a time, an index inside the set;
- restart on strips only; every binding a program reads bound; each binding once in a set;
- usage flags for every use (the interface now states them, `BufferUsage` and `TextureUsage` docs);
- texture copies within one typeless group; views in the texture's format or its twin, ranges
  inside the texture; writes, copies and clears inside their buffers, no copy between overlapping
  ranges of one buffer, clear patterns of 1, 2, 4, 8 or 16 bytes; buffer-texture pitches covering
  the region;
- read backs finished when ready (`tests/recorder.rs`) and into bytes of their size;
- no-overwrite writes touch nothing an unfinished command reads or writes, nor an ordered write not
  waited for (the rule as the webgpu track widened it, below);
- objects not used after destroy, not destroyed twice (`tests/recorder.rs`); and the promise the
  other way, work recorded before a destroy still sees the object;
- blocks needing absent features refused at creation (`tests/recorder.rs`); storage writes only
  from fragments; present needs the current texture;
- every feature, limit and format use the back end declares, as listed under G1's check above
  (`tests/rules.rs`, one test per rule or per block kind);
- buffer copies and read backs of depth with stencil move the aspect named, texture copies both,
  and only such formats name stencil;
- since run 4 (each in `tests/rules.rs`): attachment views of one level and one layer, 3D slices
  inside their level's depth, depth formats only as the depth attachment, attachments or `empty`
  but not both, nothing attached or resolved into twice, no draw writing a read-only aspect, a pass
  ending with no query running, `end_pass` and `end_query` after their begins, present outside a
  pass, surfaces of presentable formats configured before `current_texture`, regions inside their
  level, layers and slices, pitches and data covering the region, buffer ranges of every copy and
  clear inside their buffers, bindings of the resource kind the program reads, vertices and indices
  inside their buffers, a viewport, a scissor where the raster block scissors, a buffer and a
  description for every stream an attribute names, query sets and `read_queries` ranges, and every
  limit the back end declares that a call can pass (buffer and texture sizes, vertex streams,
  attributes and stride, anisotropy, inter-stage slots, colour targets and their bytes, bindings
  per stage, immediates, binding sizes and alignments);
- multisampled textures 2D, one level and layer, no storage
  (`a_multisampled_texture_is_2d_with_one_level_and_layer_and_no_storage`); their bytes moved only by
  draws, resolves and whole copies between equal sample counts
  (`a_multisampled_textures_bytes_move_by_draws_resolves_and_whole_copies`); attachments of one
  sample count (`attachments_share_one_sample_count`); resolves from many samples to one, same size,
  format (or twin for `resolve`), averaging formats only
  (`resolves_go_from_many_samples_to_one_of_an_averaging_format`); a load names a sample exactly
  when its texture is multisampled (an error of the draw).

## Interface changes

- G1 run 4:
  - c39aeac ("interface: kept the starting depth for halves ..."): helpers, G1's own, no public item
    changed. The occlusion counter saves the depth copy wherever the draws' primitives times the
    sample count reach the bound; `CombinationTracker::prepared_use` counts only draws after a
    combination was prepared. Plan 4.12 step 4 now says the bound is on a pixel's sum over its
    samples.
  - 9aa5da6 ("interface: stated that no draw writes a read-only aspect ..."): documentation of
    `AspectOps::read_only` and `PassDesc`, and plan 4.7: no draw writes an aspect marked read-only
    (WebGPU's `depthReadOnly`, `stencilReadOnly`), nothing is attached or resolved into twice,
    colour attachments within `color_targets` and `color_bytes_per_sample`.
  - From the direct3d-11 track (c8a7be9, merged at the start): `copy_texture` of a combined
    depth-stencil format copies both aspects in one call; the same rule as run 3's c743db5, so the
    two texts were made one.

- G1 run 3:
  - c743db5 ("interface: stated the alignment and limits of buffer bindings and which aspect a copy
    of depth and stencil moves"): `BindingResource` and `TextureRegion::stencil` docs, plan 4.2. A
    uniform binding's offset is a multiple of `min_uniform_offset_alignment` (the limit existed, the
    rule was unstated). Depth of a combined format moves as four bytes (as plan 4.2 already said for
    buffers written into `depth24plus`). Direct3D 12's per-plane texture copies go through a buffer.
  - 9a2d9d3 ("interface: counted occlusion before every change ..."): helpers, G1's own.
    `OcclusionCounter::flush` (new; fronts call it before changes inside a counted query),
    `PassTracker::generation` (new), `Combination` and `CombinationTracker::combination` (new;
    `drawn` now takes a `Combination`, `record` is for the cache's record only). No other track uses
    the helpers yet. Plan 4.7 and 4.12 say the same.
  - From the shaders track (1ffcab5, merged at the start): `VariantKey::output_interpolation`. From
    the webgpu track (b999c16, merged at the end): `SlowPath::IntegerClearByDraw`. Both kept as
    they are; neither conflicts with anything here.

- G1 run 2:
  - 54ff74f ("interface: stated the rules for multisampled textures, attachments and resolve
    targets"): documentation of `TextureDesc::samples` and `PassDesc`, and the plan's 4.2; the rules
    are WebGPU's. A front holds a multisampled array (Direct3D's `Texture2DMSArray`) as one texture
    per layer; written into the plan for the Direct3D tracks.
  - 4bd2adb ("interface: stated that a resolve needs render usage on both sides"): `TextureUsage`
    documentation.
  - d7347d3 (helpers, G1's own): the pass tracker's multisampled fitting and `Builtin::SampleCopy`;
    a multisampled attachment the tracker fits needs `SAMPLE` usage too (`Target` docs). No public
    item added.
  - From the direct3d-12 track (4f1894c, merged at the end): `ShaderModule::from_wgsl`, one entry
    point of a WGSL module keyed as the built-ins are. Kept: it serves every front's WGSL programs.
    It validates with no extra naga capabilities, so the built-ins needing some (cube arrays, sample
    index) keep their own path.
- G1 run 1:
  - Commit 3fcb44f ("added the pass tracker, the occlusion counter and the combination tracker to
    the interface's helpers"): new items `PassTracker`, `PassTargets`, `ColorTarget`, `Target`,
    `PassOps`, `ScopeUse`, `cut_scissors`, `OcclusionCounter`, `CombinationTracker`; the crate now
    depends on `half` too. Additions only; G1 owns the helpers (plan 12).
  - Commit d3b5eb0 ("interface: stated which buffer and texture usage each call needs"):
    documentation of `BufferUsage` and `TextureUsage`. The recording back end enforces it.
  - From the webgpu track (db9e0d8, merged at the end): `WriteMode::NoOverwrite` now also promises
    that no ordered write made since the caller last waited touches the bytes, and that a no-overwrite
    write may land ahead of earlier commands. Kept as it is: it serves both back ends and the
    recording back end now checks exactly that.
  - From the webgl track (4b71023 at the start, ab4b7e9 at the end): the built-in programs drawn in
    the clip depth convention and with the provoking vertex the back end has. Kept.
  - Start-of-run merge conflict in the webgl crate (adapter info in `caps`): took the webgl track's
    form, which passes the renderer string into `caps`.
- Earlier runs, all kept: `Backend::resolve` takes a `Format` (c60a829); `Caps::adapter`
  (`AdapterInfo`, from the direct3d-12 track); clip distances and unclamped depth as
  perspective-correct values (275c370); `BLEND_CONSTANT_ALPHA` doc (webgl); `PatternClearCopied`
  (webgpu). Their detail is in this file's history.

## The recording back end and the suite

- `Recorder` keeps every object's bytes on the CPU, runs each call when it is recorded, checks the
  rules above, and logs every base call in `calls` as a `Call` (`stream.rs`) with its answer;
  `replay(&calls)` runs them again on a new one. Calls a base call makes inside itself are not logged
  (the outer one is). `contents()` gives every buffer's and texture's bytes and the frames shown.
- It draws through an interpreter of naga's IR (`program.rs`); since this run fragment programs
  write storage buffers (applied when the draw ends; an invocation sees its own earlier writes), and
  it declares `FRAGMENT_STORAGE_WRITE`. Since run 2 it draws four samples (above); since run 3 its
  programs call functions, and it refuses everything its own capabilities leave out; since run 4 it
also checks every limit it declares that a call can pass. No format it
  holds has storage uses, so storage textures are refused. Still not drawn, each refused with an
  error naming it: points and lines, sample counts other than one and four, cube sampling, implicit
  level of detail over several levels, compute, indirect draws, alpha to coverage, storage writes
  from vertex programs, pointers to a function's locals passed to another function.
  `texture_bytes` gives a texture's bytes as held (samples as planes), for tests on back ends that
  cannot read samples in programs.
- Suite tests (24, `src/suite.rs` and `src/suite/draws.rs`): the 13 of G1a plus
  `values_are_interpolated_with_perspective`, `the_scissor_bounds_the_draw`,
  `stencil_operations_and_tests`, `near_clipping_and_the_viewport_depth_range`,
  `linear_filtering_and_address_modes`, `instances_start_at_the_first_instance`,
  `indexed_strips_restart_and_add_the_base_vertex`, `fragments_write_storage_buffers` (where
  `FRAGMENT_STORAGE_WRITE` is declared), and from run 2
  `multisampled_targets_cover_their_samples_and_resolve` (where `rgba8unorm` takes four samples),
  `programs_run_per_sample_and_read_samples` (and `SAMPLE_SHADING` and `MULTISAMPLE_READ`),
  `occlusion_queries_answer_whether_samples_passed`. Suite programs are validated with naga's
  `MULTISAMPLED_SHADING`. The webgpu track's host lists its tests by name
  (`suite!(@each ...)`), so it runs the new ones once it adds them; the webgl host runs them all.

## Next

- The independent check of G1 (run 4's fixes).
- The measure of `color_bytes_per_sample` per format is not stated by the interface: WebGPU counts
  each format by its own render-target byte cost and alignment (8 for `rgba8unorm`), and the webgpu
  back end passes WebGPU's limit straight; the recording back end counts block bytes. To settle with
  the webgpu and Direct3D tracks when `TargetsSplit` is built (no front draws it yet).
- Recording back end limitations, each refused with an error naming it and so left without a
  breaking test: cube sampling, alpha to coverage, implicit level of detail over several levels.
  Two checks are unreachable through the interface and stay as guards: storage writes from vertex
  programs (refused when the binding set is made) and an attachment taken twice by a draw (refused
  at `begin_pass`).
- Taken away with `with_caps`, `TEXTURE_VIEWS` is checked for view shapes and bound views
  (`views_of_other_shapes_need_texture_views`) but not yet for its one mip range per texture per
  draw; a view in a format other than the texture's or its twin is refused as invalid rather than
  as absent `FORMAT_REINTERPRET`; `SEPARATE_SAMPLERS` is neither declared nor checked.
- Limits the recording back end does not check because nothing they bound is drawn: point size
  and line width (lines and points are not drawn; `WIDE_LINES` is checked), compute and transform
  feedback (absent).
- Not G1's: the occlusion counter's sum by a dispatch on WebGPU is G2's (plan 4.12 step 4); until
  then it reads back (`ReadBackWait`).
- More suite tests, beyond G1's pass bar: every format's clear and read back (needs reference bytes
  from real Direct3D through the rig, not the recorder's own encoder), mip levels, layered and 3D
  targets, fans and loops through `draw_fan_or_loop` on back ends that draw lines.
- The webgpu track's host lists suite tests by name: it runs the three new ones once it adds them.

## How to run

- Linux loop (CPU only): `tools/graphics-rig/linux <command>` sends the working tree to
  `~/wcontainers-graphics/src/phase-2-graphics` on tom@192.168.2.240 and runs the command there with
  `CARGO_TARGET_DIR=~/wcontainers-graphics/target/phase-2-graphics`, 5 cargo jobs (2 when that
  machine's load average is 8 or more), at nice 10. Examples:
  - `tools/graphics-rig/linux cargo test -p wcontainers-gpu -p wcontainers-gpu-test`
  - `tools/graphics-rig/linux sh -c 'cargo clippy -p wcontainers-gpu -p wcontainers-gpu-test --all-targets -- -D warnings'`
  - `tools/graphics-rig/linux sh -c 'RUSTDOCFLAGS="-D warnings" cargo doc -p wcontainers-gpu -p wcontainers-gpu-test --no-deps'`
  - random sequences, seed and count as knobs (defaults 0x5eed and 1000; the test prints both and
    how many ran):
    `tools/graphics-rig/linux sh -c 'WCONTAINERS_GPU_SEED=0x5eed WCONTAINERS_GPU_SEQUENCES=100000 cargo test -p wcontainers-gpu-test --test sequences -- --nocapture'`.
    A failing sequence is shrunk by proptest and printed as its steps.
  - the check's faults against the suite: copy `g1r2/breaks.py` from scratch to the laptop and run
    `python3 -I breaks.py <tree> <work dir>` there (it copies the tree, changes one line, runs
    `tests/recorder.rs` and `tests/helpers.rs`); 18 faults, five of them added in run 2 for
    multisampling and the counter's halving.
  - the rule sweep: copy `g1r4/sweep.py` from scratch to the laptop, copy the tree beside it, and
    run `python3 -I sweep.py <tree> <target dir>` there (`list` instead of a target prints the
    checks); it turns each `if` that refuses something in `src/recorder.rs` and `src/draw.rs` off
    (whole, and each line of a condition over several lines) and runs `cargo test -p
    wcontainers-gpu-test`, printing CAUGHT or SURVIVED. About 40 s a check at 4 jobs.
  - Rust there: rustup with the 1.99.0 toolchain the repository names, installed by another track;
    this run installed nothing.
- A back end runs the suite with a test file holding a `Host` implementation and
  `wcontainers_gpu_test::suite!(MyHost);` (see `crates/wcontainers-gpu-test/tests/recorder.rs`).
- On the Mac only formatting: `nix develop -c cargo fmt -p wcontainers-gpu -p wcontainers-gpu-test`.

## Measurements

- G1 run 3, all on the Linux laptop (i7-8750H), on the final code after the end merges (b9a51ce):
  - Random sequences (scratch `g1r3/sequences-100k-final.log`, `--release`), each 100,000 asked and
    run, recorded with no rule broken and replayed identically, 35.0 to 35.1 s each, load 0.7 to
    0.9: seed 0x5eed, 4,556,023 calls (19,994 multisampled passes, 28,344 depth-stencil passes,
    42,632 indexed draws, 28,105 queries, 21,258 scissored draws, 7,507 tinted draws, 10,085
    no-overwrite writes without a wait); seed 0xc0ffee, 4,555,910 calls; seed 0x7a11ed (new),
    4,567,456 calls. The same three on the code before the view checks gave the same counts
    (`g1r3/sequences-100k.log`).
  - Suite on the recording back end 24 of 24; rules 45 of 45; helpers 13 of 13; fast paths 10 of
    10; `tests/recorder.rs` 30 of 30 (its 6 rule tests and the suite); `wcontainers-gpu` 32 of 32.
  - Faults checked by hand this run: the counter's restore of the saved depth removed (the halves
    test gives 0, not 12,004); the generator never waiting before a no-overwrite write (the
    recorder refuses it in the first sequences run).
  - The whole workspace checks (`cargo check --workspace --all-targets`) after the end merges.

- G1 run 2, all on the Linux laptop (i7-8750H):
  - Random sequences on the final recorder (scratch `g1r2/sequences-100k.log`, `--release`):
    seed 0x5eed, 100,000 asked and run, 3,327,668 calls, 19,128 multisampled passes, 19.8 s; seed
    0xc0ffee, 100,000, 3,332,160 calls, 18,957 multisampled passes, 21.4 s; seed 0x6b1d (new),
    100,000, 3,339,740 calls, 19,318 multisampled passes, 19.9 s. Each recorded with no rule broken
    and replayed identically. Load 0.4 to 2.3.
  - Suite on the recording back end 24 of 24; rules 26 of 26; helpers 11 of 11; fast paths 8 of 8;
    `tests/recorder.rs` rule tests 6 of 6; `wcontainers-gpu` 32 of 32.
  - The whole workspace checks (`cargo check --workspace --all-targets`) after the merges.
  - The 18 faults of `g1r2/breaks.py` (the 13 of G1a's check, and sample positions swapped, a
    per-sample run writing every sample, a resolve taking sample 0, the tracker skipping the sample
    load, the counter never halving): each fails at least one test (`g1r2/breaks-run2.log`; load
    1.6 to 3.3).
- G1 run 1, all on the Linux laptop (i7-8750H):
  - Random sequences on the final code (scratch `g1/end-checks.log`): seed 0x5eed, 100,000 asked,
    100,000 run, 3,249,453 calls, 22.5 s of test time; seed 0xc0ffee, 100,000 run, 3,263,974 calls,
    22.6 s; every one recorded without a broken rule and replayed identically. Load 0.75 to 0.98.
    Earlier the same day on the code before the widened no-overwrite rule: seeds 0x5eed and 0xc0ffee,
    100,000 each, all passing (`g1/sequences-100k-final.log`, load 0.2 to 0.9).
  - The whole workspace checks (`cargo check --workspace --all-targets`) after the end-of-run merges.
  - Suite on the recording back end 21 of 21; rules 22 of 22; fast paths 8 of 8; helpers 6 of 6;
    `tests/recorder.rs` rule tests 6 of 6; `wcontainers-gpu` 32 of 32.
  - The check's 13 faults: each fails the suite (`g1/breaks-run1.log`; load 0.3 to 1.6).
- Earlier runs' measurements are in this file's history.

## Dependencies added and their licences

- This run: `proptest` 1.12.0 in `wcontainers-gpu-test` (MIT OR Apache-2.0, its `Cargo.toml`;
  already in the workspace's lock through the kernel and translator test crates); `half` 2 in
  `wcontainers-gpu` (MIT OR Apache-2.0; already in the lock through `wcontainers-gpu-test`).
- Earlier: `naga` 30.0.2, `blake3` 1, `half` 2 (MIT OR Apache-2.0, CC0-1.0 OR Apache-2.0); plan 3.

## Departures from the plan

- From G0 and G1a: written in the plan and in this file's history.
- G1 run 1 (each now written in the plan where it changes it):
  - A pass split by the pass tracker names its resolve target in every part, not only the last,
    since a part cannot be known to be the last when it begins (plan 4.7): one more resolve per split.
  - The occlusion counter sums its target by `read_back_now` on every back end (plan 4.12 step 4);
    the WebGPU dispatch is left (Next).
  - The recording back end checks `NoOverwrite` writes against every range a command since the
    caller last waited read or wrote, and against ordered writes, which needs fence waits in the
    call stream (`FenceDone`, `WaitFence`).
  - Random sequences: proptest, as the plan says, with a fixed ChaCha seed; the generator waits for
    all work before each no-overwrite write, so it never breaks that promise.

## Known failures

- The occlusion counter refuses (`GpuError::Invalid`) a single fan, strip with restart or indirect
  draw that brings a pixel's sum over its samples to its counting format's bound (2048 in
  `r16float`, only on back ends without blendable `r32float`): 2048 fragments at the one sample of a
  single-sampled target, 512 a sample on average on four samples; no test, as no route exists. A counting program
  reading the primitive index sees it start again in the second half of a split draw.
