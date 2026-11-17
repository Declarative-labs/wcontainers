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
  - The direct3d-12 branch merged at the start (the others had nothing new); all six again at the end.

## G1's pass bar, item by item

- Every rule of section 4 checked by the recording back end, with a test that breaks it (the list
  under "The rules and their tests").
- Every fast-path default shown built from the base by a recorded stream: `tests/fast_paths.rs`, one
  test each for `read_back_now`, `prepare`, `clear_texture`, `clear_rects`, `generate_mips`,
  `draw_fan_or_loop` (fans; the recorder draws no lines, so loops are not run), `write_buffer_ring`
  and `resolve_queries`; each asserts the calls recorded, by name, and the answer.
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
  answer, same bytes in every buffer and texture, same frames, no rule broken): run for seeds 0x5eed
  and 0xc0ffee, 100,000 each (Measurements).

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
- multisampled textures 2D, one level and layer, no storage
  (`a_multisampled_texture_is_2d_with_one_level_and_layer_and_no_storage`); their bytes moved only by
  draws, resolves and whole copies between equal sample counts
  (`a_multisampled_textures_bytes_move_by_draws_resolves_and_whole_copies`); attachments of one
  sample count (`attachments_share_one_sample_count`); resolves from many samples to one, same size,
  format (or twin for `resolve`), averaging formats only
  (`resolves_go_from_many_samples_to_one_of_an_averaging_format`); a load names a sample exactly
  when its texture is multisampled (an error of the draw).

## Interface changes

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
  it declares `FRAGMENT_STORAGE_WRITE`. Since run 2 it draws four samples (above). No format it holds
  has storage uses, so storage textures are refused. Still not drawn, each refused with an error
  naming it: points and lines, sample counts other than one and four, cube sampling, implicit level of detail over several levels, compute, indirect draws, alpha to
  coverage, storage writes from vertex programs.
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

- The independent check of G1.
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
  - Rust there: rustup with the 1.99.0 toolchain the repository names, installed by another track;
    this run installed nothing.
- A back end runs the suite with a test file holding a `Host` implementation and
  `wcontainers_gpu_test::suite!(MyHost);` (see `crates/wcontainers-gpu-test/tests/recorder.rs`).
- On the Mac only formatting: `nix develop -c cargo fmt -p wcontainers-gpu -p wcontainers-gpu-test`.

## Measurements

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

- None.
