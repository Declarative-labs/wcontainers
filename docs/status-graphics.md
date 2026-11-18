# Graphics track status (interface, G1c)

Plan: `docs/phase-2-graphics.md`. Brief: `pyro-parked/wcontainers-scratch/orchestrator/brief-graphics.md`.
This branch, phase-2-graphics, is the shared line: the interface track merges the other graphics
branches into it at the start and end of every run, keeps the whole workspace building, and pushes.

## Where things stand

- G0, G1a, G1 and G1b are done and passed their independent checks. Their history, run by run and
  finding by finding, is in the git log and in this file's history (G1b's last state: f639e27d).
- G1c, run 1 (2026-11-18), done and pushed:
  - Start merges: webgpu and direct3d-11 (fast-forwards: they held the others' new work), then
    direct3d-12 (a merge); webgl, shaders and rig had nothing more. No conflict. The whole workspace checks after them
    (laptop, `cargo check --workspace --all-targets`, scratch `g1c/start-check.log`); pushed.
  - The six findings of G1b's second check settled (below).
  - Every "interface:" commit the other branches made since G1b (eleven) checked and covered
    (below).
  - The WebGPU track's standing request decided (below).
  - Part 2: plan section 15, the second batch (OpenGL and Vulkan through Mesa on the interface,
    the NIR route, the call channel and stand-ins, sound and input hooks, the interface changes
    they need, milestones G1d and G7 to G12, and the owner's decisions).
  - End merges: direct3d-11 and shaders (below, "End of run"); the rest had nothing new.
- G1c, run 2 (2026-11-18): the fourteen findings of G1c's independent check settled (below, "G1c's
  check"). Start merges: webgl (a merge) and direct3d-11 (a merge); the others had nothing new; no
  conflict, no owned file changed; the whole workspace checks after them (laptop, scratch
  `g1c2/start-check.log`); pushed (33790064).
- G1c, run 3 (2026-11-18): the twelve findings of G1c's second check settled (below, "G1c's second
  check"). Start merges: webgl, direct3d-12 and shaders (merges, no conflict); shaders brought one
  "interface:" commit (b0df13af, covered below), which broke one owned unit test's struct literal
  (`ReflectedBinding` gained `stride`), fixed in 003c5ac2. Webgpu, direct3d-11 and rig had nothing
  new. End merges: direct3d-11 and shaders, no conflict (below, "End of run"). Pushed.
- G1c, run 4 (2026-11-18): the eleven findings of G1c's third check settled, all in the plan's text
  (below, "G1c's third check"). No branch had anything new at the start (fetched first); no code
  changed this run.
- G1c, run 5 (2026-11-18): the seven findings of G1c's fourth check settled (below, "G1c's fourth
  check"): plan 15.4 now names each of Vulkan 1.1's 67 features present or absent with its route or
  reason; the suite and the random sequences now copy within depth and stencil subresources of
  every format (`Depth24Plus` added). No branch had anything new at the start (fetched first).
- G1c, run 6 (2026-11-18): the four findings of G1c's fifth check settled (below, "G1c's fifth
  check"): Vulkan's `robustBufferAccess` now names a route for vertex input; the alpha lowering's
  two NIR steps are named; the random sequences now check every depth or stencil copy's moved
  texels. No branch had anything new at the start (fetched first).
- G1c, run 7 (2026-11-18): the three findings of G1c's sixth check settled (below, "G1c's sixth
  check"): Vulkan's `robustBufferAccess` now has a route for shader access too; the interface
  states what a draw may fetch and a program may access past its range; the suite copies within
  3D and compressed subresources. No branch had anything new at the start (fetched first).
- G1c, run 8 (2026-11-18): the five findings of G1c's seventh check settled (below, "G1c's seventh
  check"): Vulkan's robust shader access now binds what the interface allows (ranges smaller than
  the program's block), with a proposed `HelperValue::BufferRange`; a storage binding's size is a
  multiple of four (interface rule, refused by the recorder); the recorder holds the attachment
  rules before dropping a draw. No branch had anything new at the start (fetched first).
- G1c, run 9 (2026-11-18): the seven findings of G1c's eighth check settled (below, "G1c's eighth
  check"): the recorder holds every draw rule before it drops a draw past a stream, and refuses
  a binding of another kind than the program declares; a buffer a storage binding reaches the
  end of is a whole number of words (`STORAGE_WORD`); the random sequences drop such draws and
  count them. Start merges: webgpu and shaders; end merges: webgpu and direct3d-11 (no
  conflict, no owned file, no "interface:" commit); the others had nothing new.

## G1c's eighth check, and how each finding was settled (run 9)

- Blocking, rules skipped for a dropped draw: the check was right. Every rule the recorder held
  only while rasterising or running the program is now held for every draw before it rasterises
  or drops, in `state_rules` (vertex inputs with an attribute in a used stream with a buffer,
  read-only depth and stencil not written, per-sample programs only with `SAMPLE_SHADING`,
  blending only into formats that blend) and `program_bindings` (every binding the program
  reads present). Found while doing so, and fixed the same way: the kind of each binding was
  checked only when the program reached it, and not at all for uniform against storage buffers,
  writability, sample type and view dimension, all of which WebGPU refuses at the draw because
  the WebGPU back end takes its binding layouts from the program (`layout.rs`; WebGPU's
  "validating shader binding", checked in the check's `webgpu.txt`). The recorder now refuses
  each (`binding_kind`; `Bound::Buffer` records whether it is uniform), and the "interface:"
  commit 306377a0 states the rule in `BindingEntry`'s doc and plan 4.5. Rule tests (new):
  `an_indexed_draw_fetching_past_a_stream_still_holds_the_draw_rules` (nine rules, each broken
  with the last index inside the stream and past it: refused alike, nothing dropped) and
  `buffer_and_texture_bindings_take_the_program_declarations_kind` (six); the multisample test
  is renamed `a_texture_is_multisampled_exactly_where_the_program_reads_one` (the rule is now
  the binding's, not the load's). Faults (scratch `g1c9/logs/faults.log`): 3 of 3 caught.
- The plan credited `nir_lower_robust_access`'s comment with naming the bounded address format:
  15.4 now says the comment names none, and that `nir_defines.h` defines the format.
- gex_wgpu's facts (15.2): read again at GitHub at 21:57 UTC (scratch `g1c9/dl`): the head of
  `wink` is 5f8a8bc255 (2026-11-18), which changes `gex_wgpu`; `gex_wgpu_screen.c` is 2,659,872
  bytes and still starts `SPDX-License-Identifier: MIT`; `meson.build` still has no header; no
  repository licence; not a fork. 15.2 now says so.
- Storage ranges at a buffer's end (4.5): decided that a front whose API makes buffers of other
  sizes, and may bind them as storage to their end, makes them a whole number of words
  (`BufferDesc`'s doc, 306377a0); 4.5 names the OpenGL driver (every buffer object), the Direct3D
  fronts (unordered access and shader resource buffers) and Vulkan (its memory requirements),
  and 15.2 the OpenGL driver's buffers. No recorder rule on buffer creation (WebGPU has none, and
  a front that forgets meets the existing "a buffer binding past the buffer's end", now broken
  that way in `buffer_bindings_are_aligned_inside_the_buffer_and_the_limits`).
- `wgl.c` (15.2): the patch list now names all three places it branches per driver (checked in
  the check's `wgl.c`): `wgl_screen_create_by_name`, `wgl_screen_create`'s `drivers[]` (ours after
  `GALLIUM_DRIVER`'s choice, so `GALLIUM_DRIVER=llvmpipe` still picks the reference) and
  `wgl_present` (`pipe_screen::flush_frontbuffer`, as Zink's).
- Magic literals: the recorder's storage size check and the sequences' storage words now use
  `STORAGE_WORD` (`size_of::<u32>()`, defined once in `wcontainers-gpu`). The sweep the brief
  asks for is not done in G1c (its findings came first): the orchestrator's list
  (`magic-literals/fix-graphics.md`) holds 830 entries in the two owned crates (most in shader
  text in Rust strings 148, component counts 124, integer limits and encodings 108, per-format
  facts 97, byte widths 76, capability tables 52); all are left for that fix. This run wrote no
  new bare number in code besides test data and the sequences' option bit for the past draw,
  which decodes a random byte as its neighbours do.
- Random sequences: an indexed pass may now end with a triangle whose indices fetch past the
  vertex stream, scissored to nothing so every outcome the interface allows leaves the same
  pixels; the recorder drops it, replay drops it alike (asserted), and the summary prints how
  many were dropped. A storage binding not a whole number of words is a broken rule, which the
  sequences never make (they record valid calls only); its rule test holds it.

## G1c's seventh check, and how each finding was settled (run 8)

Sources (scratch `graphics/g1c8/dl`, Mesa's main branch through GitLab's API, all MIT:
`nir_lower_explicit_io.c`, `nir_defines.h`, `nir_lower_io_to_scalar.c`,
`nir_lower_mem_access_bit_sizes.c`; and the check's `check-g1c-7/dl` and `check-g1c-5/dl`:
the WebGPU and WGSL specifications, Vulkan's `shaders.adoc`, the CTS's
`vktRobustnessBufferAccessTests.cpp`).
- Blocking, ranges smaller than the program's block: the check was right. Plan 15.4 now gives
  the route in three NIR steps: index and offset (`nir_address_format_32bit_index_offset`), then
  32-bit scalar accesses (`nir_lower_io_to_scalar`, `nir_lower_mem_access_bit_sizes`; Vulkan
  allows wider accesses to be checked as separate 32-bit ones, checked), then
  `nir_lower_robust_access`, its sizes read from a proposed `HelperValue::BufferRange` (15.7, the
  descriptor's range as the API gave it; OpenGL's SSBO `length()` needs it too). Storage buffers
  are declared as runtime-sized arrays of words (block four bytes) and bound as the range rounded
  up to four bytes, the buffer's memory requirements so rounded (Vulkan lets out-of-range reads
  and writes reach the memory bound to the buffer, checked). Uniform buffers bind the block's size
  from the descriptor's offset where the buffer's memory holds it, else a copy grown to the block
  (`ConstantBufferGrown`, the range copied on the GPU before the pass). Accesses past the program's
  declared block are undefined in Vulkan even with the feature (checked) and not served. G8's and
  G10's pass bars now name the CTS's 1, 3, 4 and 32 byte ranges against its 8192-byte blocks, and
  G10's driver test over the recorder also sees none of those bindings refused.
- The recorder's drop of an indexed draw past a stream now comes after the attachment rules: a
  draw that also reads its own attachment is refused, not dropped. Rule test (new):
  `an_indexed_draw_fetching_past_a_stream_still_reads_no_subresource_it_writes` (the check's own
  input).
- Storage binding sizes ("interface:" commit 227c7b99): `BindingResource::StorageBuffer`'s doc now
  says the size is a multiple of four (WebGPU's bind group rule, checked); the recorder refuses
  one that is not ("a storage binding whose size is not a multiple of four"), broken in the
  existing rule test `buffer_bindings_are_aligned_inside_the_buffer_and_the_limits` (a six-byte uniform binding stays allowed:
  WebGPU's rule is storage only). Plan 4.5: a front whose API's range is not so rounds it up within
  the buffer. The Direct3D 12 front already rounds (`storage_range`), as does the Direct3D 11
  front's typed views. No suite test: nothing a back end does changes.
- Out-of-range loads reaching other buffers (same commit): `BindingResource`'s doc now says an
  access past a binding reaches anywhere in the API buffer behind it, which a back end may share
  among several of the device's buffers; plan 4.12 says the WebGPU back end's arena of small
  uniform buffers is such a buffer, written by no program, so it is a read only.
- Why `nir_lower_robust_access` and not `nir_lower_explicit_io`'s bounded format: 15.4 now quotes
  the pass's comment and says why: the bounded format makes 64-bit global addresses
  (`load_global_constant_bounded`), which the common form cannot hold. The vec4-from-a-4-byte-range
  case is closed by the 32-bit split and the four-byte minimum binding.

## Earlier checks

G1c's first to sixth checks (runs 2 to 7) and G1b's second check (run 1) were settled as this file
at 2077a64d records, finding by finding with the sources checked; all stand.

## Interface changes

Each entry: the change, whether it fits the interface's style, its rule check and test (recording
back end, `crates/wcontainers-gpu-test/tests/` unless named) and its suite test (`src/suite*`, run
by every back end through `suite!`). Changes up to G1b, with their tests, are listed in this file at
f639e27d; all are kept.

Since G1b (from the other tracks, merged at this run's start):
- 871a23bd (shaders) `HelperValue::TexelLayout` grows to five u32 (pitches, then the view's width,
  height and layers; texels outside read zero). Fits: a helper layout. Back ends do nothing new; the
  front writes it and converted programs read it, which the shaders crate's tests cover. No rule.
- 4a72bce0 (shaders) `FRACTIONAL_VIEWPORT`'s doc: the scissor of a viewport drawn in the program is
  cut to the pixels its API draws (Direct3D rounds each edge down). Fits: a doc of a front's rule;
  no interface helper computes it, so the fronts' tests against real Direct3D cover it.
- 9f79bb5a (shaders) `DOMAIN_PATCHES_BINDING` 13 and `DOMAIN_POINTS_BINDING` 14, and 3fc7fee0
  (direct3d-12) `COUNTER_BINDING` 15: helper set bindings. Fits. Rule test (new, replacing the
  narrower one): `helper_set_bindings_never_meet` (`wcontainers-gpu` unit test; every binding and
  range of the helper set against every other).
- 4a835192 (shaders) a hull shader's reflection gives its point and constant locations. Fits: data
  of the reflection. No rule; no back-end behaviour.
- 97fa85c1 (shaders) `ReflectedBinding::slot`, and per-slot fields named by it. Fits. Rule test
  (new): `a_bindings_slot_is_its_place_in_its_kinds_range` (`wcontainers-gpu` unit test).
- 3fc7fee0 `SlowPath::PixelInvocationsCounted`, c024aa3a (direct3d-12)
  `SequentialBuffersCopied`, c6e80691 (direct3d-12) `SwizzleInShader` and `AtomicExchangeLoop`:
  named slow paths. Rule: `all_lists_every_reason_in_order` (`wcontainers-gpu`).
- 07985aaa (direct3d-12) the occlusion counter sums on the GPU where the back end computes, and
  `end_into` writes a count into a front's buffer in order. Fits: a helper with a default (read
  back) and a better route. The recorder computes with no workgroup memory, so it takes the default.
  Test (new): `the_occlusion_counter_writes_its_count_into_a_buffer_in_order` (`tests/helpers.rs`:
  the words beside it stay, a later write replaces it). Suite (new):
  `the_occlusion_counter_counts_every_sample_that_passes` (37 by 21 pixels of a 40 by 24 target,
  sizes no workgroup divides, one and four samples, through `end` and `end_into`; on WebGPU this
  runs the summing program).
- 02674dd7 (shaders) `HelperValue::Layer` and `Viewport`. Fits: helper values a front writes. No
  back-end behaviour.
- 4aaf34b2 (direct3d-12) `AddedBinding::TextureLayer` and `VariantKey::texture_layers`. Fits: an
  added binding and a variant field. Back ends bind textures at added bindings as before; no new
  rule.
- c6e80691 (direct3d-12) `ViewComponent`, `VariantKey::swizzled`, `Reflection::slow_paths`, serde
  on `SlowPath`. Fits as a variant field. Gap found: `Features::VIEW_SWIZZLE` has no call that uses
  it (`TextureViewDesc` has no mapping), so fronts always apply mappings in the program. Proposed
  in plan 15.7 (G1d); not changed now, since adding a field changes every front's and back end's
  view code.

G1c's own (eb13c84b, one "interface:" commit): copies within one depth or stencil subresource
(above); `prepare` and `PrepareDesc` name no index format, by decision (below); `VertexFormat`'s
doc and plan 4.12 and 5.7: every back end takes every vertex format, so no capability lists them
(below); the two `wcontainers-gpu` rule tests above. Suite tests added for them:
`a_copy_within_one_depth_or_stencil_subresource_moves_the_region`,
`every_vertex_format_reads_its_element` (all 52 formats, one element read as a `vec4` of its number
type into an `rgba32` target, normalised values within 2^-20; the recorder now reads the four packed
formats it refused before: `Bgra8Unorm`, `Unorm10_10_10_2`, `Uint10_10_10_2`, `Float11_11_10`).

G1c run 2 (two "interface:" commits, c1324007 and 4bbfe962): `CopyWithinTexture`'s warning text;
`SlowPath`'s doc (numbers are kept, new reasons at the end) with the rule test
`reasons_keep_their_numbers`; the slot rule test widened to every kind and slot, the ranges
adjoining, and counters as many as storage slots. Suite: the copy within one depth or stencil
subresource and the vertex format test widened (above), 1cfae99c. No new "interface:" commit came from the other branches since
run 1 (the webgl and direct3d-11 merges touched no owned file).

G1c run 3:
- b0df13af (shaders) `VariantFlags::STORAGE_OFFSET`, `ReflectedBinding::stride`, and the rule that a
  front binds a storage view from the largest offset below its start that is a multiple of both
  `min_storage_offset_alignment` and the stride, the rest in `HelperValue::StorageOffset`. Fits: a
  variant flag and data of the reflection. Back ends do nothing new (the bound offset is already
  checked against the alignment by the recorder's existing rule); the program's add is the shaders
  crate's, whose test `storage_bindings_give_their_stride_and_offsets_are_refused` holds the stride
  and refuses the flag by name until it is built. No new rule or suite test: nothing a back end
  does changes. The one owned test that builds a `ReflectedBinding` gained `stride: 0`.
- G1c run 3's own "interface:" commit: `Features::VIEW_SWIZZLE` removed (no call used it, and a back
  end declaring it would have lost mappings); `VariantKey::swizzled`'s doc. No caller used the flag
  (checked across the workspace); bit 28 is left free. Plan 15.7 proposes the mapping and the flag
  together. Suite: the vertex format test's infinity and NaN elements (1 test widened).

G1c run 5 (from the shaders branch, merged at the end of the run):
- cbe7814c (shaders) `CompareFn`'s numbers fixed at 1 to 8, as `HelperValue::EarlyTests`'s codes
  (zero a test that is off), which converted programs and the cache keep; `EARLY_STENCIL_BINDING`'s
  doc: for a format without stencil any single-sampled unsigned integer texture is bound there.
  Fits: numbers of an enum and a doc of a front's rule. Rule test: the commit's own `wcontainers-gpu`
  unit test holds the numbers (as `reasons_keep_their_numbers` holds `SlowPath`'s). No suite test:
  back ends do nothing new (they match `CompareFn` by name; no use of its number was found outside
  that test and the shaders crate). Plan 4.12's early tests now say both. Its slot test change
  (`stride: 0`) was already on this line (003c5ac2); the merge met no conflict.
- 62e28dfe (webgl) serde derived on the resource, pass and clear descriptors, so the webgl track's
  recorded call streams can be saved and replayed. Fits: derives, as `SlowPath`'s serde was. No
  rule and no suite test: no back-end behaviour changes (the workspace checks with it).

G1c run 7's own (8431a623, one "interface:" commit): what a draw may fetch past a stream and a
program may access past its binding (above, "G1c's sixth check"), with its rule test and suite
test. No "interface:" commit came from the other branches.

G1c run 8's own (227c7b99, one "interface:" commit): a storage binding's size is a multiple of
four, and an access past a binding may reach other buffers sharing its API buffer (above, "G1c's
seventh check"). Fits: a rule of the base and a doc. Rule test: the widened
`buffer_bindings_are_aligned_inside_the_buffer_and_the_limits`. No suite test (no back-end
behaviour changes). The Direct3D fronts' tests over the recorder pass with the new rule
(`wcontainers-direct3d-11`: 52; `wcontainers-direct3d-12` without its GPU test: 35; laptop, scratch
`g1c8/logs/fronts2.log`).

G1c run 8, from direct3d-12 (merged at the end of the run):
- f6c1fb42 `SlowPath::IndirectFetchPastEnd`: an indirect draw without indices whose fetches may
  pass a stream's end, skipped whole by WebGPU where Direct3D reads zeros; a listed loss counted at
  each such draw. Fits: a named reason added at the end of `SlowPath`, its number held by the
  commit's own addition to `reasons_keep_their_numbers`. No rule (nothing a caller can break) and
  no suite test (back ends do nothing new; the recorder refuses indirect draws as a stated limit).
  Plan 4.12's text merged with this line's (End of run).

G1c run 9's own (306377a0, one "interface:" commit): `STORAGE_WORD` (the bytes of a storage
word, `size_of::<u32>()`); `BufferDesc`'s doc (a buffer a storage binding may reach the end of is
a whole number of words); `BindingEntry`'s doc (every binding a program reads is bound with the
kind it declares, whatever the draw covers or fetches); plan 4.5 and 15.2. Fits: a named
constant and rules of the base, stated as WebGPU holds them. Rule tests: the two new ones and the
widened storage test (above, "G1c's eighth check"). No suite test: no back end does anything new
(the WebGPU back end already takes its layouts from the program, and the rule only refuses calls
that WebGPU refuses). The fronts' tests over the recorder still pass with the new rules
(`wcontainers-direct3d-12` without its GPU test: 35; `wcontainers-directx-graphics-infrastructure`
10; `wcontainers-direct3d-11` 51 of 52, its one failure not from these rules, below).

## The WebGPU track's request, decided

- No index format in `PrepareDesc`. Direct3D 11, OpenGL and Vulkan bind index buffers at the draw,
  not with the state, so no front knows the format when it prepares (Direct3D 12's strip cut value
  says nothing when restart is off). A back end whose pipeline holds strips' index format prepares a
  strip for both, as the WebGPU back end already does; `prepare`'s doc and plan 4.3 say so.
- No capability for the vertex formats a back end converts. Every back end takes every
  `VertexFormat` and converts what its API lacks itself (WebGPU's shadow streams); fronts pull in the
  program only for their own reasons (step rates, integers read with the other signedness). Nothing
  a front would do differently was found, so no capability (rule: none without a real use).

## For the other tracks

- Every front (run 9): every binding a program reads must be bound with the kind it declares (a
  uniform buffer for a uniform block, a storage buffer writable exactly where the program's block
  is, a texture view of the program's dimension, sample type and multisampling); the WebGPU back
  end builds its layouts from the program, so WebGPU refuses any other at the draw. The recorder
  now refuses each, before it draws or drops anything, so your tests over it find them. And a
  buffer your API makes of a size that is not a whole number of words, which you may bind as
  storage up to its end, is made with its size rounded up (`BufferDesc`'s doc, plan 4.5).
- Direct3D 11 (run 9): `creation_checks_refuse_what_direct3d_refuses` (`tests/front.rs:782`) fails
  on this line: it expects `geometry.ps_id` to be refused as `NotImplemented("PrimitiveId")`, which
  the shaders branch now converts natively (its 7265cba8, merged at run 8's end). Not from the
  interface; your test needs the new expectation.

- WebGPU: copies within one depth or stencil subresource are built (your 058834fb); done.
  The new suite tests `a_copy_within_one_depth_or_stencil_subresource_moves_the_region`,
  `every_vertex_format_reads_its_element` and `the_occlusion_counter_counts_every_sample_that_passes`
  (your summing program's first shared test) come in when you merge.
- Every back end (run 7): two new suite tests come in when you merge:
  `an_indexed_draw_past_a_streams_end_covers_only_what_its_stream_allows` (an indexed draw with
  one index past its stream: you may draw it with any value WebGPU allows or drop it, but the
  device must live and the next draw land) and
  `a_copy_within_one_3d_or_compressed_subresource_moves_the_region` (WebGPU: a 3D texture's level
  is one subresource, so slice to slice goes through your copy-within route; compressed formats
  move whole block rows through the buffer). WebGL: the compressed half runs only where you
  declare `SAMPLE | COPY_OUT` for those formats.
- Direct3D 11 and 12 and shaders (run 7): a shader's access to a constant, raw or structured
  buffer past its range gets what WebGPU gives, which may be a partly discarded draw or a trap
  (plan 4.12, now a listed loss; typed buffers are exact by their route). The interface allows the
  access (`BindingResource`'s doc); the recording back end stops at it with an error naming it,
  so your tests over it find each one.
- Direct3D 11 and 12: plan 4.12 now says an indexed draw whose indices reach past a vertex
  buffer's end may be cancelled whole by WebGPU (its index resolution allows it), not only given a
  value; still a listed loss, and 15.9 asks the owner whether to take the pulled variant instead.
- WebGL: the same three suite tests; `every_vertex_format_reads_its_element` asks for `Bgra8Unorm`
  and the packed formats, which GL ES lacks or reads differently, and `rgba32float` targets (needs
  `EXT_color_buffer_float`; the test asks `RENDER | COPY_OUT` of it, so a missing extension fails it
  by name).
- Every front (run 8): a storage binding's size must be a multiple of four (WebGPU refuses the
  bind group otherwise); the recording back end now refuses one that is not, so your tests over it
  find any. Direct3D 12 already rounds up; Direct3D 11's raw and structured views bind their
  bytes as they are, so check that every such binding is a whole number of words. WebGPU: nothing
  to do (you pass the size through, which the rule now keeps valid).
- Direct3D 11: `format::components` now returns `Format::components`; it can go when convenient.
- Direct3D 11 and 12: `Features::VIEW_SWIZZLE` is gone (no call used it); keep mapping in the
  program (`SwizzleInShader`). The mapping and the flag come together at G1d at the earliest (plan
  15.7). Direct3D 12: your branch
  is contained in this line, so merging it brings this line's `SlowPath` order without a conflict;
  discard any shader cache or log your branch recorded before 2a00617e, whose numbers for
  `SequentialBuffersCopied`, `SwizzleInShader` and `AtomicExchangeLoop` were in the other order.
- Shaders: `CONVERTER_VERSION` hashes this crate's Rust and the carried naga, not
  `wcontainers-gpu`, yet the cache stores `SlowPath` and the reflection's types by number. The
  numbers of `SlowPath` are now held by a test here; the other serialised types of the interface
  (`VariantKey`, `Reflection`) are not, so consider putting the interface crate's sources into the
  hash.
- Every track: `every_vertex_format_reads_its_element` now reads at attribute offset 12 and holds
  11-11-10 denormals, infinities and NaNs (run 3); a shadow-stream converter that drops the offset,
  scales denormals wrongly or reads the all-ones exponent as a finite value now fails it.

## The rules and their tests

Every rule a caller can break is refused by the recording back end and broken once in a test:
`tests/rules.rs` (112), `tests/compute.rs`, `tests/recorder.rs`, `tests/helpers.rs` (the usage-scope
rule, the pass tracker, the occlusion counter). The list of rules is the test names. Every rule a
draw can break is checked before the draw is rasterised or dropped, as WebGPU checks it when the
draw is encoded (since run 9). An indexed draw fetching past a stream is not a broken rule: the
recorder drops it whole, after every rule holds, and counts it (`dropped_draws`).

Known unreachable guards (kept, no test): `sample_view`'s comparison without `compare`, storage
writes from vertex programs, an attachment taken twice by a draw, a colour attachment without
`target_bytes`, a bound range past its buffer at the draw, a running query without an occlusion
set. Recorder limits refused with an error naming them: points and lines drawn (captured only),
sample counts other than one and four, cube sampling, implicit level of detail over several levels,
workgroup variables, barriers and atomics, indirect draws and dispatches, alpha to coverage,
pointers to a function's locals passed to another function, a program's access to a buffer
outside its bound range (which the interface allows; stopped so tests find each one).

## The recording back end and the suite

- `Recorder` keeps every object's bytes on the CPU, runs each call as it is recorded through an
  interpreter of naga's IR, checks the rules, logs every base call (`calls`) and replays them
  (`replay`). It draws one or four samples. `Recorder::with_caps` takes capabilities away. It reads
  every vertex format (since G1c).
- Suite: 44 tests (`src/suite.rs`, `src/suite/draws.rs`, `src/suite/routes.rs`), listed in the
  `suite!` macro; `tests/recorder.rs` runs them with 6 rule tests (50). G1c added five (above).
  None has run on a GPU from this track; the back-end tracks run them when they merge.

## Next

- G1c's independent check, ninth round.
- G1d (plan 15.8) once the owner has answered plan 15.9 (now also: wvk, Vulkan's slow features
  reported present, the copy into streams, the back end as its own module and the import call per
  interface call it costs, OpenGL and Vulkan as two modules, Xwayland with a GLX stand-in).
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
    `tools/graphics-rig/linux sh -c 'WCONTAINERS_GPU_SEED=0x3c1d0e WCONTAINERS_GPU_SEQUENCES=100000 cargo test --release -p wcontainers-gpu-test --test sequences -- --nocapture'`
  - G1c's faults and long sequences: copy scratch `g1c/faults.py` and `g1c/long.sh` to
    `~/wcontainers-graphics/g1c` on the laptop (after `tools/graphics-rig/linux true` sends the
    tree) and run `nohup ./long.sh > long.out 2>&1 &` there (every fault, then 100,000 sequences at
    0x5eed and 0x3c1d0e), or `python3 -I faults.py <tree> <work dir> <target dir> [names]` (it
    copies the tree, changes one place, runs the tests named and prints CAUGHT, SURVIVED or BUILD
    FAILED). Run 2's and 3's faults: scratch `g1c2/faults.py` and `g1c3/faults.py`, the same way, each fault
    a list of changes.
    Run 5's fault: scratch `g1c5/faults.py`, the same way; run 6's two: scratch `g1c6/faults.py`;
    run 7's three: scratch `g1c7/faults.py`.
  - Plan 15.4's feature lists: `python3 -I features.py <vk.xml> docs/phase-2-graphics.md` (scratch
    `g1c6/features.py`; `vk.xml` is in scratch `check-g1c-4/dl/vk`). Earlier runs' scripts are in scratch
    `g1r2` to `g1b2`.
- A back end runs the suite with a test file holding a `Host` implementation and
  `wcontainers_gpu_test::suite!(MyHost);` (see `crates/wcontainers-gpu-test/tests/recorder.rs`).
- On the Mac only formatting: `nix develop -c cargo fmt -p wcontainers-gpu -p wcontainers-gpu-test`.

## Measurements

- G1c run 9, on the Linux laptop (i7-8750H, 12 threads; another track building, load 3.5 to 7):
  owned crates' clippy and docs with warnings denied clean; 240 tests pass (interface 36, compute
  8, fast paths 14, helpers 19, suite on the recorder 50, rules 112, sequences 1 at the default
  1,000, which dropped 87 draws; scratch `g1c9/logs/test2.log` and `lint2.log`). Fronts over the
  recorder (`g1c9/logs/fronts.log`, `test2.log`): above, under "Interface changes". Faults
  (`g1c9/logs/faults.log`, load about 5): 3 of 3 caught (the rules skipped for a dropped draw;
  binding kinds unchecked; writability unchecked). Random sequences (`g1c9/logs/seq.log`,
  `--release`, load 6 to 7), each 20,000 asked and run, recorded with no rule broken and replayed
  identically, dropped draws equal on replay: seed 0x5eed, 1,320,030 calls, 7,562 indexed draws,
  1,514 draws dropped for fetching past a stream, 24,004 copies within one subresource, 18.2 s of
  test; seed 0x3c1d0e, 1,313,015 calls, 7,433 indexed draws, 1,496 dropped, 16.1 s.

- G1c run 7, on the Linux laptop (i7-8750H, 12 threads): owned crates' clippy and docs with
  warnings denied clean; 237 tests pass (interface 36, compute 8, fast paths 14, helpers 19, suite
  on the recorder 50, rules 109, sequences 1 at the default 1,000; scratch `g1c7/test1.log`, load
  3.0 at the start, 7.3 at the end). Faults (scratch `g1c7/faults.log`, load 4.5 to 4.1): 3 of 3
  caught: every indexed draw dropped and a draw ending exactly at its stream's end dropped (each
  by the new rule test and `indexed_strips_restart_and_add_the_base_vertex`), and a copy's
  destination slice ignored in 3D (by the new copy test alone). No long random sequences: the
  generator is unchanged and never makes an indexed draw past a stream (the recorder refused one
  before, and 1,000 cases still record with no rule broken). End check after the end merge
  (scratch `g1c7/end-check.log`, load 3.3 to 7.7, another track building): the whole workspace
  checks; owned crates' clippy clean with warnings denied; the same 237 tests pass.
- G1c run 6, on the Linux laptop (i7-8750H, 12 threads): random sequences at seed 0x5eed (scratch
  `g1c6/test1.log`, load 3.3), 1,000 asked and run (debug), then 20,000 asked and run
  (`--release`, 13.3 s of test): 1,318,516 calls, 24,004 copies within one subresource, 21,071 of
  them of depth or stencil, each now checked against the expected texels; recorded with no rule
  broken and replayed identically. Faults (scratch `g1c6/faults.log`, load 3.5 to 3.8): 2 of 2
  caught by the sequences test alone. End check (scratch `g1c6/end-check.log`, load 3.0): the
  whole workspace checks; owned crates' clippy and docs clean with warnings denied; 234 tests pass
  (interface 36, compute 8, fast paths 14, helpers 19, suite on the recorder 48, rules 108,
  sequences 1 at the default 1,000).
- G1c run 5, on the Linux laptop (i7-8750H, 12 threads, load 0.2 at the start; scratch
  `g1c5/test1.log`): owned crates' clippy with warnings denied clean; 234 tests pass (interface 36,
  compute 8, fast paths 14, helpers 19, suite on the recorder 48, rules 108, sequences 1 at the
  default 1,000); random sequences at seed 0x5eed, 20,000 asked and run (`--release`, 13.1 s of
  test): 1,206,788 calls, 24,004 copies within one subresource, 21,071 of them of depth or stencil,
  2,831 multisampled passes, 3,992 depth-stencil passes, 8,031 dispatches, recorded with no rule
  broken and replayed identically. Fault (scratch `g1c5/faults.log`, load 1.1): 1 of 1 caught.
  End checks after the end merges (scratch `g1c5/end-check.log`, load 0.4, and
  `g1c5/end-check2.log` after the second round of merges, load about 5): the whole workspace
  checks; the owned crates' clippy and docs are clean with warnings denied; the same 234 tests
  pass.

- G1c run 4: no code changed and no branch merged, so no test was run; the owned crates' code is
  that of run 3's end check (`g1c3/end-check.log`).
- G1c run 3, on the Linux laptop (i7-8750H, 12 threads):
  - Owned crates' tests (scratch `g1c3/test1.log`, load 0.0 at the start and 10 at the end, another
    track building; and `g1c3/end-check.log` after the end merges, load 5.0 to 4.5): 234 pass
    (interface 36, compute 8, fast paths 14, helpers 19, suite on the recorder 48, rules 108,
    sequences 1 at the default 1,000); clippy and docs with warnings denied clean; the whole
    workspace checks.
  - Faults (scratch `g1c3/faults.log`, 2 jobs, load 8.6 to 6.8): 2 of 2 caught, each by
    `every_vertex_format_reads_its_element` only: the 11-11-10 infinity read as 65024 (the check's
    fault, which survived before) and a NaN read as infinity.
  - No random sequences: generator and recorder unchanged since run 1's 100,000 cases at each of
    two seeds.
- G1c run 2, on the Linux laptop (i7-8750H, 12 threads):
  - Owned crates' tests (scratch `g1c2/test1.log`, load 1.8, and `g1c2/end-check.log`, load 2.9):
    234 pass (interface 36, compute 8, fast paths 14, helpers 19, suite on the recorder 48, rules
    108, sequences 1 at the default 1,000); clippy and docs with warnings denied clean; the whole
    workspace checks.
  - Faults (scratch `g1c2/faults.log` and `faults2.log`, load 4 to 6): 5 of 5 caught by the test
    written for each: 11-bit denormals scaled by 2^-15 (the check's fault; the vertex format test),
    the attribute offset ignored (that test and six others), a copy within one combined depth and
    stencil subresource dropped (the depth or stencil copy test only, which before this run held
    no combined format), two reasons swapped in `SlowPath` and `ALL` together
    (`reasons_keep_their_numbers`; the older order test passes under it), the counter range one
    slot wider (the slot test, after it was widened to check the ranges adjoin: its first form let
    this fault through).
  - No random sequences this run: the generator and recorder are unchanged since run 1's 100,000
    cases at each of two seeds.
- G1c, all on the Linux laptop (i7-8750H, 12 threads):
  - Owned crates' tests after the changes (scratch `g1c/test1.log`, load 1.2): 233 pass (interface
    35, compute 8, fast paths 14, helpers 19, suite on the recorder 48, rules 108, sequences 1 at
    the default 1,000); clippy with warnings denied is clean.
  - Faults (scratch `g1c/faults.log`, load 7 to 9: another track was building): 8 of 8 caught,
    each by the test written for it: a depth copy within one texture dropped, BGRA vertices not
    swapped, the packed alpha unscaled, the 10-bit float read with a 6-bit mantissa (the vertex
    format test), `end_into` writing at offset 0 and writing a wrong count (the helpers test and the
    suite test), a helper binding clash, `slot` off by one.
  - Random sequences (scratch `g1c/seq-*.log`, `--release`, 4 jobs, on cf096f8b's recorder and
    generator), each 100,000 cases asked and run, recorded with no rule broken and replayed
    identically: seed 0x5eed, 5,240,534 calls, 16,243 copies within one subresource (new), 15,122
    multisampled passes, 21,699 depth-stencil passes, 42,596 dispatches, 114,375 timestamps; 70.2 s
    of test (33 s build before), load about 5 at the start; seed 0x3c1d0e (new), 5,219,789 calls,
    16,032 copies within one subresource, 15,206 multisampled passes, 42,923 dispatches; 69.4 s,
    load about 5.
- G1b's and G1's measurements are in this file's history (f639e27d and earlier).

## The laptop's disk

- G1c run 9: 52 GB free at the start; at the end deleted `target/phase-2-graphics` and
  `g1c9/work`, `g1c9/target` (the faults' tree and build); `g1c9/faults.py` and the logs stay
  there and in scratch `g1c9`.

- G1c run 7: 46 GB free at the start; at the end deleted `target/phase-2-graphics` (6.2 GB),
  `g1c7/work` and `g1c7/target` (1.1 GB); 51 GB free after. `g1c7/faults.py` stays there; logs in
  scratch `g1c7`.
- G1c run 6: 41 GB free before the end clean-up (another track building); deleted
  `target/phase-2-graphics` (7.0 GB), `g1c6/work` and `g1c6/target` (1 GB); 49 GB free after.
  `g1c6/faults.py` stays there; logs in scratch `g1c6`.
- G1c found 47 GB free of 122 GB at the start; `target/phase-2-graphics` was absent (deleted at
  G1b's end) and was rebuilt. At the end this run deletes its fault build and tree copy
  (`g1c/work`, `g1c/target`) and the loop's build (`target/phase-2-graphics`); logs stay in `g1c`
  there and in scratch `g1c`. Left alone: the other tracks' folders and the `check-g1*` folders of
  the checks.
- G1c run 2: 47 GB free at the start; at the end deleted `g1c2/work` (the fault tree and its build)
  and `target/phase-2-graphics` (7.2 GB); logs stay in `g1c2` there and in scratch `g1c2`. Not
  ours, left alone: `target/phase-2-graphics-` (4.2 GB, of 2026-11-17, origin unknown) and
  `target/phase-2-graphics-shaders`.
- G1c run 3: 53 GB free at the start; at the end deleted `target/phase-2-graphics` (6.2 GB) and
  `g1c3/work` (873 MB); 49 GB free after (another track's builds grew meanwhile). Logs in scratch
  `g1c3`; `g1c3/faults.py` stays on the laptop.

- G1c run 5: 46 GB free at the start; at the end deleted `target/phase-2-graphics` and
  `g1c5/work` and `g1c5/target` (the fault's tree and build); `g1c5/faults.py` stays there, logs in
  scratch `g1c5`.

## Dependencies added and their licences

- None this run. In `wcontainers-gpu`: `naga` 30.0.2, `bitflags`, `blake3` 1, `half` 2, `serde`
  1.0.229 (MIT OR Apache-2.0; blake3 CC0-1.0 OR Apache-2.0; plan 3). In `wcontainers-gpu-test`:
  `proptest` 1.12.0 (MIT OR Apache-2.0).
- Plan 15 names what the second batch would add (Mesa MIT with permissive files, bindgen, cpal,
  smithay, test suites); none is added, and each licence is confirmed at its milestone.

## Departures from the plan

- G1c run 5: plan 15.2 had Mesa vendored as files under `third_party/mesa`; it is now a submodule
  pointing at a fork of ours, because Mesa's tree holds GPL and SGI files the project may not copy.

- G1c run 4: Mesa's Windows build uses MSVC, not clang-cl (Mesa's `install.rst` names MSVC and
  Clang, not clang-cl).
- G1c run 3: plan 15 makes OpenGL and Vulkan two WebAssembly modules, each holding Mesa's shared
  code (the brief's one module per API, at a size cost measured at G9b and G10), and adds G13 for
  the stand-ins in the container in Chrome, which need the translator and kernel; G9b and G10 run
  the conformance suites in Chrome built for WebAssembly against the modules directly.
- G1c run 3: `Features::VIEW_SWIZZLE` removed from the interface (no call used it); plan 4.11 no
  longer lists it among WebGPU's declarations.
- G1c run 2: plan 15 departs from the brief in one place and from section 10 in another, each
  named there and in 15.9: consumed arguments are copied into the call streams (the brief says
  data is read in place; 15.1 gives why), and the back end becomes its own WebAssembly module that
  every front module calls (section 10 had one module per front and back end; 15.2).
- G1c: `crates/wcontainers-direct3d-11/src/format.rs` (another track's crate) changed: its
  `components` returns `Format::components`, so the two tables cannot drift (G1b's check, finding
  6). One function body; the track can drop the function.
- G1c: the plan's second batch (section 15) departs from sections 4.8 and 10 where the first batch's
  experience says so (15.1): the channel is second-batch work and the existing Windows libraries
  move onto it at G7.
- G1b: plan 4.7 said the pass tracker ends a split part with `Store`; it refuses to split a pass
  begun with `Discard`, and the explicit front begins such a pass with `Store`. Plan 4.7 says so.
- G1b: some interface changes have no back-end behaviour (helper layouts a front writes, slow path
  names, serde derives); each is listed with the test that guards it and why no suite test can.
- G1 run 10: resource tables stay absent on the recording back end; their one stated rule is tested
  (`resource_tables_are_absent`). The recorder declares `compute_workgroup_storage` 0.
- G1 run 9: only one binding number rule is checked (paired textures at 32 to 159).
- G1 run 5: a one-statement fix in `crates/wcontainers-gpu-webgl/examples/webgl-scenes.rs` (7a0feba).
- G1 run 1: a split pass names its resolve target in every part; the occlusion counter sums by
  `read_back_now` where the back end does not compute (since 07985aaa); random sequences use
  proptest with a fixed ChaCha seed.

## Known failures

- `wcontainers-direct3d-11`'s `creation_checks_refuse_what_direct3d_refuses` fails on this line
  (since the shaders branch's 7265cba8, merged at run 8's end): it expects the primitive index in
  a pixel shader to be refused, which is now converted. That track's test to update; told under
  "For the other tracks".

- None from the WebGPU back end's depth or stencil copies any more: that track built the route
  (058834fb, merged at run 8's end) and reports the suite's test passing natively and in Chrome
  (its status, "Copies within one subresource of depth or stencil"); not rerun by this track.

- The occlusion counter refuses (`GpuError::Invalid`) a single fan, strip with restart or indirect
  draw that brings a pixel's sum over its samples to its counting format's bound (2048 in
  `r16float`, only on back ends without blendable `r32float`); no test, as no route exists. The
  same for one primitive drawn as many instances where the first instance cannot be raised; tested
  (`the_occlusion_counter_splits_instances_only_where_each_reads_the_same`).

## End of run

- Run 9: webgpu (its status only) and direct3d-11 (its G4 run 21: Direct3D 10's device, with its
  merge of this line) merged without conflict; no owned file and no "interface:" commit among
  them; the rest had nothing new. End check after them (scratch `g1c9/logs/end-check.log`,
  laptop, load 5.5 to 7.2): the whole workspace checks, the owned crates' clippy and docs are
  clean with warnings denied, their 240 tests pass; `wcontainers-direct3d-11`'s `front` test
  still fails its one test named under "Known failures". The laptop's build folder deleted
  after; no folder was made on the desktop.

- Run 8: webgpu (its G2c run 2: depth or stencil copies within one subresource, subgroups,
  stencil views), direct3d-12 (its G5 run 30, with one "interface:" commit, f6c1fb42, covered under
  "Interface changes") and shaders (its run 31) merged; one conflict, in plan 4.12's vertex fetch
  item, where direct3d-12's text was on older wording: this line's text kept, with direct3d-12's
  indirect draw without indices (`IndirectFetchPastEnd`) folded in. Webgl, direct3d-11 and rig had
  nothing new. End check after them (scratch `g1c8/logs/end-check.log`, laptop, load 2.3): the
  whole workspace checks, the owned crates' clippy and docs are clean with warnings denied, and
  their 238 tests pass. The laptop's build folder of this line was deleted after; no folder was
  made on the desktop.
- Run 7: shaders brought five commits (its run 30: geometry shaders after a domain stage, layered
  geometry shaders, its status, and its merges of this line and direct3d-11); merged without
  conflict, no owned file and no "interface:" commit among them. The rest had nothing new. No
  folder was made on the desktop.
- Run 6: after the push, webgpu brought eleven commits (its G2c run 1: three-channel 32-bit
  formats in four-channel twins, timestamps in Chrome, frame profiles, its status, and its own
  merges of the other branches); merged without conflict, no owned file and no "interface:"
  commit among them. The whole workspace checks after it (scratch `g1c6/end-check2.log`, laptop,
  load 3.9); that build folder deleted after. The depth or stencil copy route is still not on that
  branch (`memory.rs` still fails it by name). The rest had nothing new. No folder was made on the
  desktop.
- Run 5: direct3d-11 (its status only) and shaders (nine commits, one "interface:", covered under
  "Interface changes") merged without conflict; then, fetched again, webgl (six commits, one
  "interface:", covered) and shaders (its status) merged without conflict; webgpu, direct3d-12 and
  rig had nothing new. End check after all of them below (`g1c5/end-check2.log`). A last fetch
  brought two more webgl commits (its status file only), merged after the push (e96876ce).

- Run 4: no branch had anything new at the end (fetched after the work); nothing to merge. No build
  folder was made on the laptop or the desktop this run; scratch `g1c4/dl` holds two downloaded
  protocol and registry files.
- Run 3: direct3d-11 (four commits, no owned file) and shaders (its status only) merged without
  conflict; the rest had nothing new. End check above (`g1c3/end-check.log`).
- Run 2: no branch had anything new at the end (fetched after the work); nothing to merge. End
  check above (`g1c2/end-check.log`).

Run 1:

- End merges: direct3d-11 conflicted in `crates/wcontainers-gpu/src/slow.rs` only: the line had
  `SequentialBuffersCopied` before `SwizzleInShader` and `AtomicExchangeLoop`, the direct3d-11 and
  shaders branches after them. `SlowPath` is `repr(u8)` and serialised in the shaders crate's cache
  (`Reflection::slow_paths`), so one order must hold everywhere: the shaders and direct3d-11
  branches' (the three last, in that order) is taken, with G1c's doc of `CopyWithinTexture` kept.
  The direct3d-12 branch's last commit is contained in this line, so it meets no conflict when it
  merges the line (corrected at run 2). Then shaders merged without conflict; the others
  had nothing new. No other owned file changed in the end merges.
- End check after them (scratch `g1c/end-check.log`, laptop, load 2.9): the whole workspace checks,
  the owned crates' clippy and docs are clean with warnings denied, their 233 tests pass, and
  `wcontainers-direct3d-11`'s 5 unit tests pass.
