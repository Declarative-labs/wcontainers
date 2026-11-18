# Graphics track status (interface, G1b)

Plan: `docs/phase-2-graphics.md`. Brief: `pyro-parked/wcontainers-scratch/orchestrator/brief-graphics.md`.
This branch, phase-2-graphics, is the shared line: the interface track merges the other graphics
branches into it at the start and end of every run, keeps the whole workspace building, and pushes.

## Where things stand

- G0 (the plan and the interface crate), G1a (the recording back end and the first suite tests) and
  G1 (the pass tracker, the occlusion counter, the combination tracker, the call stream and its
  replay, the random sequences, every rule of section 4 checked and broken by a test) are done; G1
  passed its tenth independent check. Their history, run by run and finding by finding, is in the
  git log (and in this file's history up to f7e5dfe).
- G1b, run 1 (2026-11-17), done and pushed:
  - The four open findings of G1's tenth check settled (below).
  - Every interface change the other tracks made since G1 began, and every entry under "Interface
    changes" in their status files from then on, checked for the interface's style, given a rule
    check and a test that breaks it where the change states a rule a back end or a caller can
    break, and a suite test where it states something a back end does (list below).
  - Start merges: webgl and direct3d-11 (fast-forwards: direct3d-11 already held webgl and this
    line), then direct3d-12, shaders and rig; no conflict. The whole workspace checks after them.
  - End merges: webgpu (with one interface commit, 04bd53bf, covered below), shaders and then
    webgl, no conflict. The whole workspace checks after them. The direct3d-12 branch's newest work
    (6 commits, up to 1345db29) was merged too, but `wcontainers-direct3d-12` then did not build:
    its new `device/geometry.rs` is written against an older stage helper than the shaders branch
    this line already holds (`StageDraw::stream_output`, `StageError::Declaration`, a fourth
    argument, `vertex_layout()` now an `Option`). That merge was not pushed and was taken off
    again, so the line builds; the direct3d-12 track should merge this line (or the shaders
    branch), fix its geometry route, and it comes in at the next merge.

## G1's tenth check, and how each finding was settled

- A split of an explicit pass that discards: the tracker cannot know a part is the last when it
  begins it, so it now refuses (`GpuError::Invalid`) to split a pass with a `Discard` store, and
  `begin`'s docs and plan 4.7 say an explicit front (which knows its whole pass before it replays
  it) begins a pass whose draws write storage with `Store` where its API discards. The next part
  now keeps the pass's store ops and read-only aspects (it took `LOAD_STORE` before, losing a
  read-only depth). Tests: `a_split_explicit_pass_keeps_its_depth_and_ops_between_parts` (the
  check's picture, and the second part's logged ops) and `a_split_of_a_pass_that_discards_is_refused`
  (`tests/helpers.rs`). Departure noted below.
- Unchecked u32 sums: texture regions, layers, slices and view ranges are compared in u64; a draw
  whose `first + count` or `first_instance + instances` passes a u32 is refused (the rule is now in
  `DrawRange`'s docs); `read_queries`' range likewise. Tests: `regions_lie_inside_their_texture`,
  `views_take_the_textures_format_or_its_twin` (extended) and `draw_ranges_end_within_a_u32`.
- The suite's size in this file: rewritten (below).
- The combination tracker gives its record out: `CombinationTracker::totals` (this run's draws per
  vertex shader and tuple, with the cache's record taken in); plan 4.4 says so. Test: the end of
  `the_combination_tracker_picks_the_tuples_drawn`.

## Interface changes

Each entry: the change, whether it fits the interface's style, its rule check and test (recording
back end, `crates/wcontainers-gpu-test/tests/` unless named) and its suite test (`src/suite*`, run
by every back end through `suite!`).

Since G1 passed (from the other tracks, merged at this run's start, and 04bd53bf at its end):
- 04bd53bf (webgpu) `copy_texture` between non-overlapping regions of one subresource, and
  `SlowPath::CopyWithinTexture` where the API refuses it. Fits: a base rule stated, with a named
  slow path. The recorder took any copy within a texture, overlapping ones too; it now refuses an
  overlap. Rule test: `texture_copies_within_one_texture_do_not_overlap`. Suite:
  `a_copy_within_one_subresource_moves_the_region`.
- 72dbc184 (webgl) `Features::POLYGON_MODE_POINT`, `POLYGON_MODE` narrowed to lines. Fits: a
  capability. The recorder refused point filling as `POLYGON_MODE`; it now answers
  `Absent(POLYGON_MODE_POINT)`. Rule test: `raster_blocks_need_the_features_they_use` (a case per
  fill mode). Suite: `fill_modes_draw_edges_or_corners` (edges or corners where declared, the
  right `Absent` where not). For the webgpu track: its `create_raster` still names `POLYGON_MODE`
  for points, so this suite test fails there until it names `POLYGON_MODE_POINT`.
- 360ee4b7 (shaders) `VariantKey::linked` (`LinkedValues`). Fits: a variant field, as 4.4's
  rewrites are. It rests on a linking rule the interface never stated; now stated
  (`Program::Render`, plan 4.2): every location the fragment shader reads, the vertex shader
  writes, with the same type, interpolation and sampling (WebGPU's and GLSL ES's rule); it may
  write more. Rule test: `fragment_inputs_are_vertex_outputs_of_their_type_and_interpolation`
  (`tests/rules.rs`). Suite: `values_pass_between_programs_by_location` (outputs 0, 2 and 4, inputs
  0 and 4: a back end linking by rank reads the wrong value).
- b157c760 (shaders) `HelperValue::SamplerState` codes in the order of `Filter` and `AddressMode`.
  Fits (a documented layout), but the order was implicit, and converted programs, and so the
  cache, keep the codes: `Filter` and `AddressMode` now carry explicit numbers. Rule test:
  `sampler_state_codes_keep_their_numbers` (`wcontainers-gpu` unit test). Suite: none; the codes
  pass between a front and its converted program through a uniform buffer, and no back end sees
  them.
- dd6d9b29 (shaders) `HelperValue::PulledStream { input }`, four u32, and one binding per pulled
  input. Fits: a helper layout. Back-end rules it leans on (storage binding offsets aligned, inside
  the buffer) were already checked: `buffer_bindings_are_aligned_inside_the_buffer_and_the_limits`.
  Suite: `vertices_pulled_from_storage_match_the_fetched_ones` (fetched and pulled pictures equal,
  at step rates one and two from first instance 1, a binding from an unaligned stream offset
  rounded down); runs where `VERTEX_STORAGE_READ`, which the recorder now declares (its programs
  always could read storage).
- c00421e4 (direct3d-12) `CONSTANTS_TEXTURE_WIDTH` and `ConstantsAsTexture`'s texture. Fits: a
  constant both sides share. The copy's rules were already checked
  (`buffer_texture_copies_take_pitches_covering_the_region`). Suite:
  `constants_read_through_a_texture_lie_256_rows_to_a_texture_row` (300 rows, read at rows 0, 1,
  255, 256 and 299).
- ba3c5737 (shaders) `HelperValue::TextureComponents`. Fits, but each front was to work the bits
  out itself (direct3d-11 has its own `format::components`): the measure is now
  `Format::components` in the interface, so fronts share one. Suite:
  `missing_components_read_their_defaults` (a view of each format reads the components
  `components` names and defaults for the rest: r8, rg8, rgba8, r16float, rg16float, and
  rg11b10ufloat where sampled). For the direct3d-11 track: `format::components` can become
  `Format::components`.
- This run's own (each an "interface:" commit): 017b511 (split ops kept, a discarding split
  refused, `CombinationTracker::totals`, `DrawRange`'s u32 bound), 8423348 (the linking rule),
  4ef381f (`Format::components`, the fixed sampler codes). Tests named above.

Earlier since G1 began (merged in G1's runs, all kept; checked again here):
- c8a7be9 (direct3d-11) `copy_texture` moves both aspects of a depth-stencil format. Rule:
  `copies_of_depth_and_stencil_move_the_aspect_named`. Suite:
  `copies_between_depth_stencil_textures_move_both_aspects`.
- d37c5d9 (webgl) what `capture_vertices` writes. Rule: `captures_keep_to_their_rules`
  (`tests/compute.rs`). Suite (new): `captures_write_location_outputs_packed_in_draw_order` (where
  `TRANSFORM_FEEDBACK`).
- 4b71023, ab4b7e9 (webgl) built-ins drawn in the back end's clip depth and provoking vertex. Rule
  test (new): `built_ins_take_the_provoking_vertex_the_back_end_has` (`tests/fast_paths.rs`; the
  minus-one depth route cannot run on the recorder, which lacks `CLIP_DEPTH_MINUS_ONE`). Suite: the
  draw-made `clear_rects` and `generate_mips` tests run the built-ins on every back end.
- 4e98cf5 (webgl) `generate_mips_by_draws`, the default made callable. Rule: none (no new rule).
  Tests: `generate_mips_is_a_pass_per_level_and_layer`; suite
  `generate_mips_averages_in_the_values_format`.
- 1ffcab5 (shaders) `VariantKey::output_interpolation`. Rule: the linking rule above. Suite:
  `values_are_interpolated_with_perspective`, `strips_take_flat_values_from_the_provoking_vertex`.
- 3428d7c, a17eca9 (shaders) serde derives. No behaviour; round trips are the shaders crate's
  cache tests.
- 4f1894c (direct3d-12) `ShaderModule::from_wgsl`. A constructor; the new linking rule test builds
  its shaders with it.
- Named slow paths, each a listed name in the interface's style: `IntegerClearByDraw` (b999c16,
  webgpu), `StencilWrittenByDraw` (3f5519a), `PresentConverted` (8f7db1b), `CaptureExpanded`
  (3dbc7d5, webgl). Rule: `all_lists_every_reason_in_order` (`wcontainers-gpu`). Suites covering
  what they serve: `clears_fill_exactly` (an `r32uint` clear past 2^24),
  `copies_between_depth_stencil_textures_move_both_aspects`, `present_shows_the_frame`, the new
  capture test.
- 578a3fc (webgpu) a listed loss (float depth copied by a draw is clamped and flushed). No rule
  (a loss); the suite copies only values every back end keeps.
- G1's own changes in its runs (helpers, documentation of rules): each has the rule test named in
  "The rules and their tests".

## The rules and their tests

Every rule a caller can break is refused by the recording back end (a violation that loses the
device, or `GpuError` at creation) and broken once in a test; `tests/rules.rs` holds most, one or
more tests per rule (108 now), `tests/compute.rs` the dispatch, capture and timestamp rules,
`tests/recorder.rs` six more, `tests/helpers.rs` the usage-scope rule and the pass tracker. The
list of rules and their tests is the test names themselves; this run added the linking rule, the
u32 bounds, the point-fill capability, the discarding split and overlapping copies within a
texture.

Known unreachable guards (kept, no test): `sample_view`'s comparison without `compare`, storage
writes from vertex programs, an attachment taken twice by a draw, a colour attachment without
`target_bytes`, a bound range past its buffer at the draw, a running query without an occlusion
set. Recorder limits refused with an error naming them: points and lines drawn (captured only),
sample counts other than one and four, cube sampling, implicit level of detail over several levels,
workgroup variables, barriers and atomics, indirect draws and dispatches, alpha to coverage,
pointers to a function's locals passed to another function.

## The recording back end and the suite

- `Recorder` keeps every object's bytes on the CPU, runs each call as it is recorded through an
  interpreter of naga's IR, checks the rules, logs every base call (`calls`) and replays them
  (`replay`). It draws one or four samples. `Recorder::with_caps` takes capabilities away.
- Suite: 37 tests (`src/suite.rs`, `src/suite/draws.rs`, `src/suite/routes.rs`), listed in the
  `suite!` macro; `tests/recorder.rs` runs them with 6 rule tests (43). This run added
  `a_copy_within_one_subresource_moves_the_region`,
  `fill_modes_draw_edges_or_corners`, `values_pass_between_programs_by_location`,
  `vertices_pulled_from_storage_match_the_fetched_ones`,
  `constants_read_through_a_texture_lie_256_rows_to_a_texture_row`,
  `missing_components_read_their_defaults` and
  `captures_write_location_outputs_packed_in_draw_order`. None has run on a GPU yet.

## Next

- G1b's independent check.
- The webgpu and webgl hosts take the seven new suite tests when they merge; expected there: webgpu
  must answer `Absent(POLYGON_MODE_POINT)` for point filling; webgl draws lines natively where
  WEBGL_polygon_mode is present and captures through transform feedback.
- Not G1b's: the occlusion counter's sum by a dispatch on WebGPU (G2; until then `ReadBackWait`).
- More suite tests beyond the pass bars: every format's clear and read back against real
  Direct3D's bytes, mip levels, layered and 3D targets, fans and loops on back ends that draw lines.

## How to run

- Linux loop (CPU only): `tools/graphics-rig/linux <command>` sends the working tree to
  `~/wcontainers-graphics/src/phase-2-graphics` on tom@192.168.2.240 and runs the command there with
  `CARGO_TARGET_DIR=~/wcontainers-graphics/target/phase-2-graphics`, 5 cargo jobs (2 at load 8 or
  more), at nice 10. Rust there: rustup with the 1.99.0 toolchain the repository names (installed
  by another track; this run installed nothing).
  - `tools/graphics-rig/linux cargo test -p wcontainers-gpu -p wcontainers-gpu-test`
  - `tools/graphics-rig/linux sh -c 'cargo clippy -p wcontainers-gpu -p wcontainers-gpu-test --all-targets -- -D warnings'`
  - `tools/graphics-rig/linux sh -c 'RUSTDOCFLAGS="-D warnings" cargo doc -p wcontainers-gpu -p wcontainers-gpu-test --no-deps'`
  - `tools/graphics-rig/linux sh -c 'cargo check --workspace --all-targets'`
  - Random sequences, seed and count as knobs (defaults 0x5eed and 1000; the test prints both, the
    count run and what was drawn); a failing sequence is shrunk by proptest and printed:
    `tools/graphics-rig/linux sh -c 'WCONTAINERS_GPU_SEED=0x61b0b1 WCONTAINERS_GPU_SEQUENCES=100000 cargo test --release -p wcontainers-gpu-test --test sequences -- --nocapture'`
  - This run's faults: copy scratch `g1b/faults.py` and `g1b/long.sh` to `~/wcontainers-graphics/g1b`
    on the laptop and run `nohup ./long.sh > long.out 2>&1 &` there (100,000 sequences at 0x5eed
    and 0x61b0b1, then every fault), or `python3 -I faults.py <tree> <work dir> <target dir>
    [names]` (it copies the tree with fresh file times, changes one place, runs the tests named
    with `--no-fail-fast` and prints CAUGHT, SURVIVED or BUILD FAILED). Earlier runs' fault and
    sweep scripts are in scratch `g1r2` to `g1r11`; started detached they need
    `export PATH=$HOME/.cargo/bin:$PATH` first.
- A back end runs the suite with a test file holding a `Host` implementation and
  `wcontainers_gpu_test::suite!(MyHost);` (see `crates/wcontainers-gpu-test/tests/recorder.rs`).
- On the Mac only formatting: `nix develop -c cargo fmt -p wcontainers-gpu -p wcontainers-gpu-test`.

## Measurements

- G1b run 1, all on the Linux laptop (i7-8750H, 12 threads), nothing else of ours running:
  - Random sequences on c4a5d61 (scratch `g1b/sequences-100k.log`, `--release`, 4 jobs). Each seed
    had 100,000 cases asked and run, recorded with no rule broken and replayed identically:
    - seed 0x5eed: 5,223,851 calls (the generator did not change, so run 11's count), 79.4 s
      with the build, load 0.2 at the start;
    - seed 0x61b0b1 (new): 5,216,633 calls, 15,168 multisampled passes, 21,676 depth-stencil
      passes, 32,341 indexed draws, 21,439 queries, 15,970 scissored draws, 7,499 no-overwrite
      buffer writes and 8,919 no-overwrite texture writes without a wait, 14,291 discarding
      texture writes, 42,941 dispatches, 35,156 vertex captures, 114,269 timestamps; 62.9 s,
      load 1.0.
  - Faults (scratch `g1b/faults.log`, load about 2): 15 of 15 caught, each by the test written
    for it: a discarding split allowed, a split losing the pass's ops, `totals` dropping the
    record, a region's x end, layer end and a view's level range wrapping, draw and instance
    ranges unchecked, point filling refused under `POLYGON_MODE` (caught by the rule test and the
    suite test), the linking check off or taking another location, the interpreter linking by
    rank, `instance_index` without the first instance (caught by the pulled-vertices suite test and
    `instances_start_at_the_first_instance`), `Format::components` wrong for `r8unorm`, and the
    filter codes swapped. After the end merge, two more (`g1b/faults2.log`, load 1.0): an
    overlapping copy within a texture allowed, and a copy within one texture dropping its texels;
    2 of 2 caught, by `texture_copies_within_one_texture_do_not_overlap` and
    `a_copy_within_one_subresource_moves_the_region`.
  - End check after the end merges and the overlap rule (scratch `g1b/end-check.log`): the whole
    workspace checks (`cargo check --workspace --all-targets`), the owned crates' docs build with
    warnings denied, and 100,000 sequences again replay identically with no rule broken: seed
    0x61b0b1, 5,216,633 calls, 114.5 s with the build, load 3.3 at the start (another track
    building beside it); seed 0x5eed, 5,223,851 calls, 71.7 s, load 8.4. The owned crates' whole
    test set on the final tree: 225 pass (interface 33, compute 8, fast paths 14, helpers 18,
    suite on the recorder 43, rules 108, sequences 1 at the default 1,000 cases).
- Earlier runs' measurements (G1 runs 1 to 11) are in this file's history up to f7e5dfe: the
  last, run 11, ran 100,000 sequences at 0x5eed, 0xc0ffee and 0x7e11c5 (about 64 s each at 4 jobs)
  and caught 7 of 7 faults.

## The laptop's disk

- This run found 7.6 GB free of 122 GB at the start. It deleted run 10's sweep build
  (`g1r10/target`, 1.3 GB) at the start and its own fault build (`g1b/target`) and tree copy
  (`g1b/work/tree`) at the end. Left alone: `target/phase-2-graphics` (our shared build, kept for
  the next run), `target/phase-2-graphics-shaders` and the `check-g6a*` folders (the shaders
  track's), `target/phase-2-graphics-` (owner unknown), and the `check-g1-*` folders of G1's checks.

## Dependencies added and their licences

- None this run. In `wcontainers-gpu`: `naga` 30.0.2, `bitflags`, `blake3` 1, `half` 2, `serde`
  1.0.229 (MIT OR Apache-2.0; blake3 CC0-1.0 OR Apache-2.0; plan 3). In `wcontainers-gpu-test`:
  `proptest` 1.12.0 (MIT OR Apache-2.0).

## Departures from the plan

- G1b: plan 4.7 said the pass tracker ends a split part with `Store`. A part is begun before the
  tracker knows it will be split, so a pass begun with `Discard` cannot keep its first part. The
  tracker refuses such a split, and the explicit front (which knows its whole pass) begins a pass
  whose draws write storage with `Store`. Plan 4.7 now says so. Direct3D 12 does not use the
  tracker yet, so nothing changes for the other tracks today.
- G1b: the pass bar asks for a rule test and a suite test per interface change. Some changes have
  no back-end behaviour (the sampler codes, serde derives, `from_wgsl`, listed losses, slow path
  names); each is listed above with the test that does guard it and the reason no suite test can.
- G1 run 10: resource tables stay absent on the recording back end; the interface does not yet say
  how a program reads a table. Their one stated rule is tested (`resource_tables_are_absent`).
- G1 run 10: the recorder declares `compute_workgroup_storage` 0 (one invocation at a time).
- G1 run 9: only one binding number rule is checked (paired textures at 32 to 159); the other
  kinds' ranges are the fronts' numbering (plan 4.5).
- G1 run 5: a one-statement fix in `crates/wcontainers-gpu-webgl/examples/webgl-scenes.rs` (7a0feba)
  to keep the shared line building after the rig's and webgl's work met.
- G1 run 1: a split pass names its resolve target in every part; the occlusion counter sums by
  `read_back_now` on every back end; random sequences use proptest with a fixed ChaCha seed.

## Known failures

- The occlusion counter refuses (`GpuError::Invalid`) a single fan, strip with restart or indirect
  draw that brings a pixel's sum over its samples to its counting format's bound (2048 in
  `r16float`, only on back ends without blendable `r32float`); no test, as no route exists. The
  same for one primitive drawn as many instances where the first instance cannot be raised; tested
  (`the_occlusion_counter_splits_instances_only_where_each_reads_the_same`).
