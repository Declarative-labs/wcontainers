# Status: graphics, NIR route (G8)

Branch phase-2-graphics-nir. Plan: docs/phase-2-graphics.md, sections 15.2, 15.3, 15.4 and 15.8 (G8).

## Where it stands

G8 run 13 (2026-11-20). Complete but for two items waiting on others, each named below; handed
to G8's independent check. Both candidates are measured on the SPIR-V corpus, piglit's, bgfx's and
dEQP's GLSL, Godot's Compatibility renderer and Godot's RenderingDevice shaders; candidate B leads
on every count and on time, and is the route (The choice, below; final). Run 13: the timed choice
over the whole corpus read and recorded; route B fixes for cubes (a cube array's size, storage
cubes); the harness's limits closed where they could be (multisampled textures filled, a reference
that does not end on the seeded inputs held on the next seeds), the rest explained (Harness
limits). Dolphin is left out (below).

Waiting on others (G8's bar, as the orchestrator ruled on 2026-11-19 night):
- The rewrites of 15.4 on the route's output (formatless storage, gathers with offsets known only
  at run time, arrays of textures, samplers and combined image samplers indexed at run time).
  Cause: the shaders crate has no public entry point for them. Owner: the shaders track (asked in
  the brief of 2026-11-19 night; its branch tip f97bec1b, unchanged at the start of run 13, has
  only `convert` for DXBC and DXIL). Nothing else stands in the way: the route's output is a naga
  module with its reflection, the form that entry point is to take.
- robustBufferAccess's three steps (15.4). Cause: `HelperValue::BufferRange` is a proposed
  interface change of 15.7 and one of the owner's open decisions of 15.9 (not on the shared line
  at 3195d728 nor on any graphics branch at the start of run 13). Owner: the owner, then the
  interface track. Step 2 (storage as runtime-sized arrays of words, 32-bit accesses) is already
  what the bridge makes of OpenGL's buffers (Done); steps 1 and 3 need the helper value.

Done:
- Mesa pinned and building on the rented x86-64 machine with llvmpipe and lavapipe (and Zink, whose
  NIR to SPIR-V compiler is candidate A's first step), by one command (How to run). Pin:
  `crates/wcontainers-nir/mesa/pin.toml` (mesa-26.3.1, commit fec8830e9c94); meson options:
  `crates/wcontainers-nir/mesa/options.ini`; patches: `crates/wcontainers-nir/mesa/patches/` (applied
  in name order, again from the pinned commit whenever they change; they are part of the converter's
  version); script: `crates/wcontainers-nir/mesa/build`. One patch:
  `zink-matrix-arrays.patch` (Zink's `get_glsl_type` decorates a member that is an array of matrices
  too, with the type's explicit stride).
- `crates/wcontainers-nir`: the crate, its C bridge (`bridge/bridge.c`, `bridge.h`) and its link to
  Mesa's compiler libraries (`build.rs`). Built when `WCONTAINERS_MESA` names the Mesa build folder;
  without it the crate builds without the NIR parts (cfg `mesa`), so the workspace builds everywhere.
  `build.rs` takes everything about Mesa from Mesa's own build (Meson's `meson-info`, the compile
  commands, the introspected dependencies and targets, among them lavapipe's file).
- The NIR the routes take is shaped by one `nir_shader_compiler_options` in the bridge (Zink's list
  plus all 64-bit integer lowering and the instance index counting from the first instance); unused
  inputs, outputs and system values are removed. The drivers (G9, G10) must hand over the same.
- `Nir::to_common_form` (`wc_nir_to_common_form`), every route's step: sampled array layers rounded
  to the nearest even integer (Vulkan's rule; the common form's layer is an integer, and naga's
  SPIR-V reader truncates the float layer); compute dispatches counted from workgroup 0 with no
  global offset (`nir_lower_compute_system_values` with no bases); the binding rule of 6.4 (each combined image sampler
  split into a texture and a sampler at the same set and binding; every resource renumbered into its
  stage's set kind by kind in the program's (set, binding) order; the reflection as
  `ReflectedBinding`, program set in `space`, program binding in `register`; unsized arrays of
  resources and overfull ranges refused by name).
- The cache key (15.3): `Nir::key`, BLAKE3 of `nir_serialize`'s bytes and `CONVERTER_VERSION` (this
  crate's version; since run 12 a BLAKE3 of the crate's `src` and `bridge` and of the carried naga,
  made by `build.rs`, so a change of the route's output changes the key, as the shaders crate's
  `CONVERTER_VERSION` does; the Mesa commit and a hash of `mesa/build`'s record of commit and
  options). Taken from the NIR as the driver hands it over. Held by `tests/route.rs`.
- Candidate A (`zink-spirv-naga`): Zink's `ntv_shader_prepare` and `nir_to_spirv` (native Vulkan,
  SPIR-V 1.3), cleaned of Zink's `Index 0` decorations and its unneeded `DrawParameters`, read by
  naga's SPIR-V reader and validated with what the common form allows.
- Candidate B (`nir-naga`, `src/direct.rs`, `bridge/flat.c`, `flat.h`): the bridge takes the
  shader out of SSA (phi webs into registers; every other value used outside its block, or by an
  `if`, into a register of its own; derefs, and buffer descriptors (`vulkan_resource_index`,
  `load_vulkan_descriptor`), made again in each block) and flattens it into plain
  arrays (types, struct fields, variables, then the instructions in order with `if`, `else`,
  `loop` and their ends as markers). Mesa's numbering stays in C: every kind and operation crosses
  as the bridge's own enums (`WC_ALU_OPS`, `WC_INTRINSICS`, mirrored in order in Rust). Rust walks
  it into naga: stage inputs and outputs as entry point arguments and a result struct through
  locals (the position's y negated on the way out: the common form's clip space is WGSL's, as naga's
  SPIR-V reader makes it for A), buffers through `vulkan_resource_index` chains, registers as u32
  or bool locals, NIR's untyped values kept with the kind their instruction gives and bitcast at
  each use, constants made at each use in the kind it needs, a texture sampled with a comparison
  made a depth texture; storage images (load, store, size; multisampled loads), atomics on buffers
  and workgroup variables (each place an atomic acts on, found by walking its deref back, gets an
  atomic type, as naga requires), `fmod` (a - b floor(a / b)), `frem`, point coordinates, storage
  buffers always readable (the common form has no write-only buffers). What it does not take is
  refused by name ("not taken: ..."). Held by
  `tests/route.rs` (both candidates bind a sampling program alike; a compute program).
- Stage values in route B are lowered IO, one form for every source: `wc_nir_lower_io` (bridge.c)
  runs Mesa's `nir_lower_io_passes` on a Vulkan program (after splitting gl_PerVertex), which the
  OpenGL linker has already run on a GLSL one. `flat.c` gathers every slot `load_input`,
  `load_interpolated_input` and `store_output` reach into one variable a slot (the widest access;
  one kind, u32 where accesses disagree; clip distances folded into one array; a built-in keeps its
  own type), with a fragment input's interpolation from its barycentric (`load_input`: flat); Rust
  reads them as `LOAD_STAGE` and `STORE_STAGE` on that variable's local. Refused by name: a slot
  indexed at run time, other than 32 bits, a second blend source, interpolation at a sample or an
  offset, a slot interpolated two ways. The corpus gives the same results as with variables (run 5).
- alphaToOne's two steps (15.4): `Nir::alpha_to_one` (`wc_nir_alpha_to_one`) lowers IO and runs
  Mesa's `nir_lower_alpha_to_coverage` and `nir_lower_alpha_to_one`. Route B turns
  `demote_samples` into a local of the samples kept (all ones at the start, ANDed with the
  complement of each demoted set) that becomes the sample mask output, ANDed with the program's
  own where it writes one; the pass's 16-bit operations (u2u16, u2u32, mov, and, or, multiply,
  shifts) are carried in 32-bit words holding the zero-extended value (multiply and shift left
  masked to 16 bits, shift counts to their low 4 bits); every other 16-bit operation is refused by
  name. Held by `tests/route.rs` (a program with and one without its own sample mask: one sample
  mask output, alpha one, validated) and `tests/narrow.rs`: a SPIR-V program with native 16-bit
  integers (written word by word in the test) run by lavapipe natively and through route B, both
  held to Rust's 16-bit arithmetic, over every 16-bit value and every shift count 0 to 31 (2,097,152
  invocations, 14,680,064 results, seed 0x5eed for the second operand and the upper bits; 0 differ).
  Checked to see differences: with the multiply's and the shift count's masks taken out (temporary),
  6,151,775 results differed.
- GLSL (`Nir::from_glsl`, `wc_glsl_to_nir` in `bridge/glsl.cpp`, a C++ file built by a second
  `cc::Build` with the flags of Mesa's `standalone.cpp`): Mesa's GLSL compiler and linker on the
  standalone scaffolding (`libglsl_standalone`, no gallium screen), one context for every source:
  OpenGL 4.6 compatibility, GLSL 4.60, the ES 3.x compatibility extensions, and the limits the
  OpenGL driver is to report (named `WC_GL_*` in `glsl.cpp`; G9 reports the same). Then, per stage,
  st/mesa's steps after a link (`st_link_glsl_to_nir`, `st_glsl_to_nir_post_opts`,
  `st_finalize_nir`, MIT) for that driver's capabilities: dead IO variables removed;
  `gl_nir_lower_buffers` (which ends in `nir_lower_explicit_io`: every buffer access is a block
  index and a byte offset); fragment coordinates and point coordinates transformed for the common
  form's origin (upper left, centres at half: `nir_lower_wpos_ytransform` and
  `nir_lower_pntc_ytransform`, their state values added to the parameters); system values; state
  references; atomic counters into storage buffers (no hardware counters); 64-bit integers lowered;
  rectangle textures and gathers with offsets lowered (`nir_lower_tex`: no rectangles, no gather
  offsets); uniform places assigned in words (packed storage, `PackedDriverUniformStorage`) and the
  default block made uniform buffer 0 (`nir_lower_uniforms_to_ubo`, every block one index up);
  samplers as derefs; `gl_FragColor` written to every draw buffer (`nir_lower_fragcolor`, the
  driver's own step, as Zink's). A built-in uniform structure of the compatibility profile (the
  lighting state) is refused by name. Compile and link errors come back with Mesa's log
  (`ConvertError::Glsl`).
- OpenGL's buffers in route B (`wc_nir_buffers_as_words`, bridge.c): each uniform block becomes a
  variable of `vec4<u32>` words (the common form's uniform arrays step by 16 bytes; its size the
  block's, the default block's from the parameters), each storage block a runtime-sized array of
  `u32`, writable only where the program writes it; uniform block i binds at i, storage block j at
  the number of uniform blocks plus j (the program's set and binding in the reflection);
  `load_ubo`, `load_ssbo`, `store_ssbo` (by its write mask), the storage atomics and
  `get_ssbo_size` become 32-bit derefs of those words at offset / 4, which route B already takes.
  A block index not constant after folding, or an access not 32 bits wide, is left as it is and
  refused by name. This is also 15.4's robustBufferAccess step 2 (storage as runtime-sized arrays
  of words, 32-bit accesses). Held by `tests/route.rs` (a vertex and fragment program with default
  uniforms, a std140 block, std430 blocks, a sampler, an atomic: both stages convert and validate,
  bindings counted by kind; a program that does not compile is refused with its log) and
  `tests/glsl_buffers.rs`: a GLSL compute program reading a std140 block (vec3, float, float array,
  mat3, ivec2) and writing a std430 block (atomic, runtime array, `.length()`) runs on lavapipe
  through route B and naga's SPIR-V writer and matches what GLSL's layout rules and arithmetic give,
  computed in the test (exact values). Checked to see differences: with the word's component within
  its vector forced to 0 (temporary), 10 of 17 values differed.
- Each GLSL stage's names (`GlslStage::names`, `wc_glsl_names` in `glsl.cpp`): every default-block
  uniform with its first word in uniform buffer 0 and its words (Mesa makes one parameter a
  column or element; the bridge joins them into one name), the state values
  (`FragCoordYTransform`, `PointCoordYTransform`, any other by Mesa's state string), each uniform
  and storage block's and each sampler's and image's program binding, and each stage input and
  output with its location in the common form, GLSL type and interpolation. Held by
  `tests/route.rs`. Stage values between two stages are what the linker leaves: varyings are packed
  (`packed:a,b`) and `nir_opt_varyings` reworks them across the stages; a vertex program linked alone
  loses its outputs. Attributes and fragment outputs keep their names.
- Route B takes `b2b1` (a 32-bit boolean word as a boolean: not zero), which OpenGL's default-block
  booleans need; casts to a boolean are made at naga's boolean width.
- The GLSL reference (`tests/corpus/llvmpipe.rs`, `glsl.rs`): llvmpipe through the pinned build's
  `libEGL` (`mesa::EGL`, from Meson's target `EGL`), on EGL's software device
  (`EGL_MESA_device_software` through `EGL_EXT_platform_device`), an OpenGL 4.5 compatibility
  context with no surface. The program is compiled and linked by llvmpipe; OpenGL's reflection
  (`glGetProgramResourceiv`, called directly: glow's asks for the count with no room and gets
  nothing) gives every uniform, block and storage-block member with its type, offset and strides.
  Default-block uniforms are filled from the seed by type, set with `glUniform*` and written into
  uniform buffer 0 at the bridge's words (booleans as the driver's true, all ones); each block is
  filled member by member by type (runtime-sized arrays 64 elements) and the same bytes are bound
  on both sides, the route's at the binding the bridge names for the block's name. Compute: the
  program dispatched twice on llvmpipe (they must agree), the route's module once on lavapipe;
  storage blocks compared word by word by the member's type (padding not compared). Held by
  `tests/glsl_reference.rs` (a compute program with float, ivec3, mat2, uint array and bool
  uniforms, a std140 block of an array of structures and a mat3, a std430 block with an atomic and
  a runtime array of structures: matched). Checked to see differences: with route B's `fmul` made
  an add (temporary, on the x86 machine's copy) 32 values differed; with `iadd` made a subtract, 49.
- piglit's shader tests in the corpus (`corpus/sources`, `glsl::shader_test_sources`): each
  `.shader_test` file's `[vertex shader]`, `[fragment shader]` and `[compute shader]` sections
  through `Nir::from_glsl` and each stage through the route; `[vertex shader passthrough]` is no
  stage of the file's; geometry, tessellation and assembly programs, two sources of one stage and a
  compute stage beside others are refused by name. A source without `#version` gets the file's
  `GLSL >= x` requirement's, as piglit's shader runner gives it (`shader_runner.c`).
- GLSL vertex and fragment programs held to llvmpipe (`glsl::drawn_program`, `compare_draw`,
  `Llvmpipe::draw`): drawn as points, one a pixel of a row, one input set each. A vertex program is
  wrapped in its own source (`#define main wc_program_main` after `#version`; a new `main` calls it,
  keeps `gl_Position` in `wc_position` and puts the point at the attribute `wc_point`); with no
  fragment program it is followed by a catcher (written from llvmpipe's reflection of the wrapped
  program linked separable) that writes every output to a storage block at the point's pixel. A
  fragment program alone is fed by a vertex program written from its inputs (llvmpipe's reflection
  of it linked separable; `gl_Color`, `gl_TexCoord` and the like fed through OpenGL's own outputs).
  Desktop feeders and catchers are `#version 450 compatibility` (no interpolation to match between
  stages from 4.40); an ES feeder is the program's own ES version. The drawn program goes through
  `Nir::from_glsl` and the same route; llvmpipe draws it (window depth 0 to 1 by `glClipControl`,
  vertex colour clamping off: both are draw-time variants of the driver's that the route does not
  take), twice (they must agree), and lavapipe runs the route's two modules. Attributes at
  llvmpipe's locations and at the bridge's; an attribute the route does not read is fed to llvmpipe
  alone. State values filled as st/mesa does for a framebuffer object. Colour targets (each
  output's components), depth and storage blocks compared. Not set up yet, by name:
  `gl_SecondaryColor` (its array has three components), structure and block stage values, an ES
  vertex program alone.
- Faults in the GLSL bridge the reference found (run 7), each fixed: blocks without a layout were
  std140 where st/mesa makes them std430 for a driver that loads its constants from a buffer
  (`UseSTD430AsDefaultPacking`; the one compute program that differed); a source without `#version`
  was taken at no version at all and accepted 1.30 and later syntax (`DefaultGLSLVersion` is now
  110, st/mesa's); `gl_FragColor` was not written to the draw buffers (`nir_lower_fragcolor` works
  on variables, and stage values are lowered by then: the bridge now does it on lowered IO);
  vertex inputs kept Mesa's slots, so `gl_Vertex` and the other attributes of OpenGL's own had no
  location: they are now numbered as st/mesa gives the driver its vertex elements (each at its rank
  among the attributes read, `nir_recompute_io_bases`), and the bridge names them so. OpenGL's own
  varyings (`gl_FrontColor`/`gl_Color`, back colours, `gl_TexCoord[]`, fog) take locations after
  the 32 generic ones, by their Mesa slot (`MAX_VARYING` + slot). A built-in still refused names
  its Mesa slot.
- Route B fixes from run 8: a depth comparison gives one value, and NIR's legacy shadow lookups
  (GLSL 1.10's `shadow2D*`, four components) have it in every component, so route B splats it, as
  llvmpipe's sampler does before the view's swizzle (OpenGL's depth texture mode is that swizzle:
  G9's view, not the program's); an infinite or NaN constant is made from its bits (`u32` literal
  bitcast to `f32`), as naga's literals must be finite; projective lookups are lowered in the
  bridge (`nir_lower_tex`'s `lower_txp`, all dimensions and arrays); a rectangle texture is 2D
  after `lower_rect` (its coordinates normalized). Refused by name where the common form has
  nothing: a comparison with a bias or gradients (WGSL compares at level 0 or the implicit level
  only), sampling an integer texture (WGSL only loads and gathers those; a rewrite to a load with
  the sampler's addressing would be a slow path G9 can add), an array of arrays of textures,
  samplers or images, a structure holding them, clip distances read by a fragment program.
- The GLSL comparison fills textures (run 8): each route texture resource finds its sampler by the
  bridge's `GlslNameKind::Sampler` name at its binding; the same seeded texels (as
  `compare::resources` makes them) become an OpenGL texture of the view's target at its own unit
  (`SampledTexture`, `Llvmpipe::sampled_texture`: nearest, clamped to the edge, one level; a depth
  texture compared less or equal with its swizzle `RED` in all four, so llvmpipe's raw comparison
  is compared); a sampler both stages read gets one set of texels. Storage images the same way
  (`GlslNameKind::Image`), bound with `glBindImageTexture` at the image unit of the same number
  and read back with `glGetTexImage` after the work, after the storage blocks (`GlStorage`); the
  route's storage image output compared word by word by its components' kind. OpenGL's own attributes
  (`gl_MultiTexCoord0..7`, `gl_Color`, `gl_Normal`, `gl_FogCoord`) are fed through OpenGL's own
  arrays (`AttributeSlot`; `glTexCoordPointer` and its kin). Held by `tests/glsl_reference.rs`
  (a compute program reading a 2D, a 3D unsigned, a cube, a 2D shadow and a 2D array signed
  texture through `textureLod`, `texelFetch`, `texelFetchOffset` and a comparison, and loading and storing an `r32ui`
  and an `rgba32f` image: matched). Checked to see differences (temporary changes on the x86
  machine's copy): with OpenGL's texels shifted by one texel, 236 values differed; with the
  images not read back, 80.
- bgfx's desktop GLSL shaders in the corpus (run 8; `tests/corpus/bgfx.rs`): its compiled shaders
  (`examples/runtime/shaders/glsl/*.bin`, shaderc's format 12) read as bgfx's OpenGL renderer
  hands them to the driver (`#version 430` before a source without one); its `bgfx_ndc` uniform is
  left a uniform (bgfx makes it a constant from its capabilities; seeded here like any other).
  A file of another format version is refused by name.
- Route B fixes from run 9: `ubitfield_extract`, `ibitfield_extract` and `bitfield_insert` (one
  `extractBits` or `insertBits` a component, as the common form's offset and count are scalars),
  `isign`, `frexp_exp` and `frexp_sig` (naga's `frexp` and its result's members), the sample mask
  input, and `ixor` of booleans (made `!=`; naga has no exclusive or of booleans). Held by
  `tests/glsl_reference.rs` (a compute program of bitfields, signs and exponents: matched; with the
  extract's offset and count swapped and the sign made an absolute value, temporarily on the x86
  machine's copy, 198 values differed).
- A vertex attribute read in part (run 9): st/mesa gives the driver a vertex element a slot read,
  so a matrix of which some columns are read has elements only for those. The bridge now names
  each run of read slots of an attribute (`GlslName::element`, the run's first slot in the value;
  `at`, its first element; `count`, its slots), and the comparison feeds each column of an
  attribute at the route's place for it, or to llvmpipe alone. Before, the unread columns moved the
  next attributes, among them the point's position. Held by `tests/route.rs` (a `mat4` read in
  columns 1 and 3: two names).
- dEQP's GLES shader library in the corpus (run 9; `tests/corpus/deqp.rs`): every case of VK-GL-CTS
  `data/gles2/shaders`, `gles3/shaders` and `gles31/shaders` (`*.test`, Apache-2.0, pinned in
  `corpus/sources`) read from dEQP's format (`gluShaderLibrary.cpp`'s grammar) and specialized as
  `glsShaderLibraryCase.cpp` does (its `${DECLARATIONS}`, `${SETUP}`, `${OUTPUT}` and the program
  cases' `${VERTEX_*}` and `${FRAGMENT_*}` values, `:single-line`, the extensions a case requires
  after the version line, a source without a version taken at the case's): a `both` case is two
  items, `<case>_vertex` (with a fragment program written here that writes what it passes on to the
  colour targets) and `<case>_fragment` (the corpus's feeder gives its inputs); a case of its own
  vertex and fragment sources is one. Where dEQP compares the outputs with its expected values in
  the shader, here every output is written to a colour target (floats four a target, from
  `dEQP_FragColor`; integers and booleans as `uint` in `uvec4` targets after them; in GLSL ES 1.00
  all as floats in `gl_FragColor`), so llvmpipe and the route are compared on the values. Refused by
  name: cases dEQP expects to fail (compile, link or validation), pipelines of separable programs,
  geometry and tessellation, outputs past the colour targets. An item is `<file>#<case>`. Held by
  `tests/glsl_reference.rs` (a library file of a `both` case, a failing case and a program case:
  each item's sources, and each compiles).
- ES programs at full precision (run 9, `glsl::at_full_precision`, every GLSL program of the
  corpus): `#define lowp highp`, `#define mediump highp` and `highp` defaults after the version and
  extension lines. llvmpipe computes `mediump` in 16 bits (its `fp16` and `int16` capabilities make
  Mesa's GLSL compiler lower precision), while the bridge lowers none, as our driver will compute;
  GLSL ES allows both. Without it 883 of dEQP's programs differed by half-precision rounding or
  16-bit integers. `#version 100` is now taken as ES (GLSL ES 1.00 has no profile word), so its
  feeders are ES too.
- The alpha_to_one programs in the corpus (run 9): `corpus/cts-programs` builds VK-GL-CTS's
  `vk-build-programs` at the commit `corpus/sources` pins and writes the programs of
  `dEQP-VK.pipeline.monolithic.multisample.alpha_to_one` (its registry: one file a distinct program;
  the group's 28 cases build 2, a vertex and a fragment program) into
  `nir-corpus/vk-gl-cts-programs/alpha_to_one`. The corpus run puts a fragment program of that
  folder through `Nir::alpha_to_one` before the routes and records it "held by G10's CTS cases"
  instead of comparing it (below). Run 9 (`results/run9/nir-results-r9-a2o.jsonl`): route B
  converts and validates the fragment program after the two steps; the vertex program matches
  lavapipe on both routes; route A fails the fragment program (Zink's compiler and lowered IO).
- Godot's Compatibility renderer in the corpus (run 10; `tests/corpus/godot.rs`, Godot pinned in
  `corpus/sources`, only `drivers/gles3/shaders` and its licence fetched): each `.glsl` file with a
  stage section read as Godot's build reads it (`gles3_builders.py`: `#[modes]` lines `name =
  defines` with `\n` escapes, `#[specializations]` `NAME = true|false`, `#[vertex]`,
  `#[fragment]`, `#include` resolved beside the including file once a stage), and each variant
  made as `ShaderGLES3::_build_variant_code` makes it for desktop OpenGL (`#version 330`,
  `USE_GLES_OVER_GL`, `USE_HALF2FLOAT`, the set specializations, the general defines, the mode's,
  external textures as `sampler2D`, multiview emulated, highp defaults; the material's
  `#GLOBALS`, `#MATERIAL_UNIFORMS` and `#CODE` lines empty, as for Godot's default version). The
  general defines are those `ShaderGLES3::initialize` gets at Godot's default project settings,
  in `corpus/godot-defines` with where each value comes from. Items: each mode at the default
  specializations and each mode with one specialization flipped (`<file>#<MODE>` and
  `<file>#<MODE>:<SPECIALIZATION>`), 241 in all. Held by `tests/glsl_reference.rs` (a file of two
  modes, two specializations, an include and material lines: its six items, two programs' text,
  both compile).
- A vertex program that writes no position (run 10; `clip_unpositioned` in `bridge.c`, part of
  `to_common_form`, so both routes): it gets the position (0, 0, 0, -1), outside every clip
  volume, as the shaders crate's `stage::clip_unpositioned` gives a Direct3D one (WGSL's vertex
  stage must write a position; OpenGL and Vulkan leave it undefined). On variables or lowered IO,
  whichever the NIR has. The comparison captures the outputs the program writes, so the added
  position is not compared. Held by `tests/route.rs` (a GLSL program writing only a varying: its
  module has a position and validates).
- Godot's RenderingDevice shaders in the corpus (run 11; `corpus/godot-forward-plus`,
  `tests/corpus/godot_rd.rs`): `servers/rendering/renderer_rd/shaders` and the AMD FSR and FSR2
  folders it includes are fetched with Godot's other paths. Each file is read as Godot's build
  reads it (`include_file_in_rd_header`: GLSL comments cut, `#[vertex]`, `#[fragment]`,
  `#[compute]`, includes beside the file or, under `thirdparty/`, from Godot's root, once a stage),
  and each variant written as `ShaderRD::_build_variant_code` makes it for Vulkan (the general
  defines, the variant's, `RENDER_DRIVER_VULKAN`, external textures as `sampler2D`, no material;
  compute alone where there is a compute stage), then compiled by glslang as Godot's
  `compile_glslang_shader` does (Vulkan rules, client Vulkan 1.1, SPIR-V 1.3) into
  `nir-corpus/godot-forward-plus`, which the corpus run takes as SPIR-V. The variants are in
  `corpus/godot-rd-variants` (387 variants of 81 files; each list copied from its `initialize`
  call, those built in loops written out as the loops make them; every variant of the list,
  those Godot enables only on some devices or settings included), the general defines in
  `corpus/godot-rd-defines` at Godot's default desktop settings, each with where it comes from.
  The tables were drafted by a scratch script reading the C++ (`scripts/rd_variants.py`,
  `rd_table.py` in scratch) and the loops written by hand; glslang compiles all 572 stages (0
  refused). `voxel_gi_sdf.glsl` and `giprobe_write.glsl` are not taken: no `initialize` call at
  the pinned commit builds them. Held by `tests/glsl_reference.rs` (a two-stage file with an
  include, a comment and material lines: both stages' text exactly as `ShaderRD` makes it).
- Route B takes every storage image format naga has (run 11; the bridge's `wc_storage_format`
  mirrors naga's `StorageFormat`, 64-bit apart, which route B refuses anyway), and NIR's `imod`
  (the remainder, plus the divisor where the signs differ and the remainder is not zero). The
  comparison makes those formats on both sides (lavapipe and llvmpipe); 32-bit float texels are
  compared as floats, every narrower format by its bits.
- A texel load's offset (run 12; `to_common_form`, both routes): Mesa's `nir_lower_tex` with
  `lower_txf_offset` adds it to the coordinate, as the common form's load takes none. Route B
  refuses a load still carrying an offset by name ("a texel load with an offset"); before, it
  dropped the offset silently, which made bgfx's two `cs_assao_prepare_depths_and_normals`
  programs differ. `nir_lower_tex` also gives every size query a level 0, which the bridge takes
  off a multisampled texture's again (the common form's size query of one takes no level). Held by
  `tests/glsl_reference.rs` (the sampling program now reads through `texelFetchOffset` too:
  matched; with the lowering taken out, temporarily on the x86 machine's copy, the route refuses
  it by name).
- Subroutine uniforms (run 12): the bridge names each as `GlslNameKind::SubroutineUniform` by its
  API name (Mesa's `__subu_<stage>_` prefix taken off), its words one an element, each the index of
  the subroutine chosen (what st/mesa writes from `glUniformSubroutinesuiv`). The comparison reads
  each stage's subroutine uniforms from llvmpipe (`GL_*_SUBROUTINE_UNIFORM`: location, elements,
  compatible subroutines), chooses one compatible subroutine an element from the seed, sets them
  with `glUniformSubroutinesuiv` after `glUseProgram`, and writes the same indices at the route's
  words. Held by `tests/route.rs` (an array of two: one name, two words) and the corpus (the 9
  piglit programs match; with every index's low bit flipped on the route's side, temporarily, 7
  of 9 differed).
- Cubes in route B (run 13): a cube's size has two components (the faces' width and height; a
  cube array's layers after them), where route B took three and failed naga's validation on a cube
  array's `textureSize` (dEQP `android_extension_pack.test#extension_directive.ext_texture_cube_map_array`).
  A storage cube or cube array is a 2D array of its faces in the common form, which has no storage
  cubes (`storage_dimension`; Vulkan and OpenGL address a storage cube's faces as layers): its
  coordinate's third component is the layer, its size the faces' size and, for a cube array,
  the layers divided by `CUBE_FACES` (now the crate's one definition, `wcontainers_nir::CUBE_FACES`).
  The driver (G9, G10) binds a 2D array view of the cube's faces, as the interface already does
  for a cube read by integer loads. The GLSL comparison makes the storage image a cube on
  llvmpipe's side (by OpenGL's reflected type) and the route's 2D array of the same faces. Both
  dEQP programs match (`implementation_limits.max_fragment_image_uniforms` was the storage cube).
- Multisampled textures in the comparison (run 13): lavapipe gets each by a draw a layer
  (`Lavapipe::fill_samples`: a covering triangle, a fragment program written in WGSL and run a
  sample at a time that reads its own sample's texel from a storage buffer), every sample of
  every texel seeded on its own, 4 samples. All 6 programs that read one match on route B (Godot's
  `resolve` 3 and `resolve_raster`, Sascha Willems' `deferredmultisampling/deferred.frag`, and its
  vertex program). Checked to see differences: with every sample read as sample 0 (temporary, on
  the x86 machine's copy) `deferred.frag` differed; with samples 0 and 1, 2 and 3 swapped none did
  (each program averages over its samples); Godot's read a seeded sample count, at most 1 on the
  seed (expected, not confirmed). The GLSL side (the one dEQP item) and multisampled depth
  textures are not filled (Harness limits).
- A reference that does not end on the seeded inputs (run 13; `corpus/run.rs`): a child writes
  `REFERENCE_ENDED` to its errors once the reference has run, so a child stopped at the time limit
  is told apart. Stopped before it: the reference did not end (a loop whose bound or step comes from
  the seeded inputs); the route is run again on the next seed, up to `REFERENCE_SEEDS` (4) seeds,
  and the seed used is recorded with the route's result (`inputs_seed`). Stopped after it: the
  route's module did not end where the reference did, a difference. The six `irradiancecube.frag`
  and Godot's `canvas_sdf.glsl#MODE_LOAD_SHRINK` now match on route B, on a later seed.
- Whether lavapipe runs `demote_samples` (checked first, as G8 says): it does not. In the pinned
  Mesa only broadcom, freedreno, asahi, KosmicKrisp and imagination take it; llvmpipe, gallivm and
  lavapipe have no code for it (searched in the source). So the fragment programs of
  `dEQP-VK.pipeline.monolithic.multisample.alpha_to_one.*` after the two passes are held by G10's
  CTS cases, not by lavapipe; they are to be listed so when they are in the corpus.
- The corpus run: every `.spv` under `WC_NIR_CORPUS`, each
  shader and route in a child process of its own; a crash or hang is named and attributed to the step
  it happened in (the route, or the comparison). One JSON line a shader. Knobs: `WC_NIR_SHADERS` (a
  seeded sample), `WC_NIR_SEED` (the sample and every shader's inputs; default 0x5eed),
  `WC_NIR_INPUT_SETS` (vertices or fragments a run, each with inputs of its own; default 64),
  `WC_NIR_JOBS`, `WC_NIR_RESULTS`, `WC_NIR_DUMP`.
- Counted apart before any route (`tests/corpus/needs.rs`): a program declaring a SPIR-V capability
  beyond Vulkan 1.1's core, or one of 1.1's whose feature our Vulkan driver reports absent (15.4's
  list of 26).
- The comparison (`tests/corpus/compare.rs`, `lavapipe.rs`, `interface.rs`): the program as given
  run by lavapipe (the reference), and the route's module written back to SPIR-V by naga's writer
  and run by lavapipe the same way, on the same seeded inputs; the reference runs twice and must
  agree with itself. Lavapipe is loaded from the pinned build by its own `vk_icdGetInstanceProcAddr`
  (no loader), every Vulkan 1.0 to 1.3 feature it has enabled, with transform feedback and
  robustness2 (accesses outside a binding read zero and write nothing, in both runs), one thread
  (`LP_NUM_THREADS=0`, so racing programs repeat). Inputs: uniform, storage and immediate data filled
  by type (floats in [-2, 2), integers in [-4, 4) or [0, 4): programs use them as indices and
  counts), runtime arrays of 64 elements, 4-texel textures of one level (2 layers for arrays, 6
  faces for cubes) with nearest clamped samplers, vertex attributes per input set. Compute: 2
  workgroups, storage compared. Vertex: transform feedback of the position and every located output
  (decorations added to both modules' words), rasteriser off. Fragment: a vertex program made with
  naga feeds one point a pixel of a row, one input set each; 32-bit colour targets cleared to a
  marker, depth where the program writes it, storage. Integers compared exactly; floats equal when
  within 1024 units in the last place or 2^-20 apart (both runs on one driver and processor: only
  the order and fusing of operations differ).
- The comparison is checked to see differences: with every float constant of the route's module
  raised by 1 (a temporary change, not kept), 37 of the 43 compared shaders of a 60-shader sample
  differed (the 6 others use no float constant on what they output).

The corpus (Sascha Willems' samples, GLSL, HLSL and Slang compiled to SPIR-V): 1005 shaders. 138
have a stage the common form lacks (ray tracing 81, tessellation 30, geometry 10, mesh and task 6); 3
crash Mesa's `spirv_to_nir` before any route (HLSL's variable rate shading); 25 need what our Vulkan
1.1 does not report (buffer device addresses 8, sparse residency 5, descriptor indexing 3, shading
rate 3, minimum LOD 3, barycentrics 2, ray query 2). 839 reach both routes.

Run 4's results, held to lavapipe (x86-64 machine, 192 children at once, load 9.2 at the start and
25.2 at the end, shared with other tracks; 293 s with the Mesa rebuild; Zink patched). Run 5, with
route B on lowered IO, gave the same counts and the same reasons (x86-64 machine, load 0.07 at the
start and 5.96 at the end, 256 s; `results/run5/nir-results-r5a.jsonl`):
- Candidate A: 728 match, 0 differ, 8 not compared, 103 fail by name.
- Candidate B: 773 match, 0 differ, 8 not compared, 58 fail by name.
- Shader by shader: 727 match on both routes, 1 on A only, 46 on B only.
- The 8 not compared, on both: 6 `irradiancecube.frag` (a loop whose step comes from the uniforms
  never ends on a negative step: the reference hangs), 1 multisampled texture (the harness does not
  fill those), 1 fragment program of the mesh shading sample with a block input.
- A's failures: 29 the draw's bases (`DrawParameters`); 40 crashes in Zink's compiler (31 in
  `emit_deref_array` on a variable mode it takes only after its own lowering of buffers and push
  constants to words, which this route does not run; 2 arrays of samplers; 7 `.length()` of runtime
  arrays); 7 naga reader `As` on HLSL vertex inputs and 1 on HLSL's row-major array of matrices in
  `shadowmappingcascade/scene.frag` (it differed before the patch; now it fails by name); 6 integer
  stage outputs without `flat`; 6 input attachments; 3 multiview; 3 vertex programs without a
  position; 3 formatless storage; 2 naga type resolution in order-independent transparency; 1 each:
  coherent image, arrays of combined image samplers indexed at run time, a uniform block's alignment.
- B's failures: 23 `load_first_vertex` and 6 `load_base_instance` (the draw's bases, as A); 11
  subpass inputs; 5 binding arrays the common form does not allow (3 textures and samplers, 2
  buffers: the descriptor heap sample); 3 `load_view_index` (multiview); 3 `image_deref_atomic`
  (order-independent transparency; A fails them too); 3 vertex programs without a position (as A);
  3 storage images without a format (15.4 rewrite); 1 uniform block alignment (HLSL's
  `texturearray/instancing.vert`, as A). The full list: `scripts/reasons.py` (in scratch) on
  `results/run4/nir-results-r4b.jsonl`.
- Zink also writes `gl_ClipDistance` and `gl_CullDistance` outputs a glslang program never writes, so
  A's modules ask a back end for clip distances they do not use; B drops them.

Run 7, with piglit's shader tests (2541 files) beside the SPIR-V corpus (`results/run7/nir-results-r7g.jsonl`;
x86-64 machine, 192 children, load 4.9 at the start and 13.2 at the end, 245 s):
- SPIR-V corpus: unchanged, A 728 match, B 773 match, 0 differ, 8 not compared each.
- piglit: 931 files reach no route (geometry 225, tessellation 147, assembly programs 91, several
  sources of a stage 122, and the rest refused by Mesa's compiler with its log: extensions our
  OpenGL does not report, and programs piglit itself expects to fail). Of the other 1610:
  route B 1152 match, 17 differ, 98 not compared, 343 fail by name; route A 3 match, 11 not
  compared, 1596 fail (Zink's compiler takes only its own lowered IO: `load_input` is not
  implemented in `nir_to_spirv` without Zink's own passes, so A fails nearly every GLSL program).
- B's 17 differences are all undefined behaviour on the seeded inputs: 14 index an array out of its
  range with a seeded uniform (`fs-vec4-const-array-indirect-access-*` 8, `glsl-array-bounds-10`
  and `-12`, `fs-mat4-out-of-bounds-1`, `glsl-fs-varying-array`, `vs-inout-index-inout-mat2-col-array`,
  `array-initializers-1`; llvmpipe reads zero, naga's SPIR-V clamps the index), 2 clamp a NaN
  (`fs-saturate-sqrt`, `vs-saturate-sqrt`: `sqrt` of a negative seeded value), 1 divides an integer
  by zero (`fs-loop-unrolled-divide-by-zero`). Left as differences (known, for this reason).
- B's 98 not compared: textures and images not filled on the GLSL side 84, subroutine uniforms 9,
  a state value with no name 2 (to look at), and 3 drawn programs llvmpipe or the bridge refused
  (an extension, too many attributes, ES versions that differ).
- B's 343 failures by name: `load_ubo` 93 (64-bit loads of `arb_gpu_shader_fp64`, bindless
  handles, uniform block arrays indexed at run time), naga validation of route B's output 126
  (type resolution in fragment programs and "expected data type": route faults, to look at first), a stage value interpolated two ways 15,
  textures of other dimensions or operands 25, `gl_PrimitiveID` and `gl_ClipVertex` (Mesa slots 21
  and 16) 17, 64-bit values 14, binding arrays the common form does not allow 21, and smaller
  groups (`scripts/glsl_reasons.py <results> nir-naga` in scratch counts them).

Run 8, with bgfx's 325 desktop GLSL shaders beside piglit and the SPIR-V corpus (3871 files;
`results/run8/nir-results-r8d.jsonl`; x86-64 machine, 192 children, load 4.5 at the start and
16.1 at the end, 250 s):
- SPIR-V corpus: unchanged, A 728 match, B 773 match, 0 differ.
- piglit: route B 1310 match (up from 1152), 17 differ (the same 17, same reasons), 17 not
  compared (subroutine uniforms 9, storage images 3, a state value with no name 2, and 3 drawn
  programs llvmpipe or the bridge refused), 266 fail by name. With storage images (the last run,
  `nir-results-r8e.jsonl`, load 7.8 at the start and 9.0 at the end, 250 s): 1313 match, 14 not
  compared, the rest unchanged. Of B's failures, 25 are naga
  validation (21 binding arrays and 1 storage binding array the common form does not allow, 3
  vertex programs without a position); every other one is a named "not taken: ..." (`load_ubo` 94:
  64-bit, bindless and uniform block arrays indexed at run time; Mesa slots of other built-ins 17;
  a stage value interpolated two ways 15; a comparison with a bias or gradients 15; a structure
  holding samplers 17; 64-bit values and constants 20; and smaller groups). Route A: 3 match.
- bgfx: 4 files refused by Mesa's compiler (3 compute programs over 256 invocations a group, 1
  needs `GL_ARB_shader_viewport_layer_array`). Route B 281 match, 3 differ, 23 not compared
  (storage images on the GLSL side 20, a storage format the harness does not make 3), 14 fail by
  name (`ubitfield_extract` 6, a storage image of another format 5, `isign` 3). Route A 9 match
  (267 of its failures are Zink's compiler aborting on GLSL's lowered stage values). With storage
  images (r8e): route B 301 match, 3 not compared (`rgba16f` storage, not made by the harness).
- bgfx's 3 differences: `fs_hdr_bright` reads seeded texels as RGBE (`exp2(w * 255 - 128)`, with
  w in [-2, 2)), which overflows to infinity; llvmpipe gives 0 where the route gives NaN, and
  GLSL leaves operations on infinities to the implementation (the class of piglit's NaN through
  clamp). `fs_shadowmaps_packdepth_vsm` and `_linear` pack a depth with `fract(255 * z)`, z from a
  division by a seeded w; the two differ by one unit in the last place of `255 * z` (2^-13 at that
  size), which `fract` makes an absolute difference past the tolerance; GLSL allows division 2.5
  units in the last place. That the division is the source is expected, not yet confirmed.

Run 9, with dEQP's GLES cases beside piglit, bgfx and the SPIR-V corpus (17,501 items;
`results/run9/nir-results-r9a.jsonl`; x86-64 machine, 192 children, load 12.2 at the start and 11.4
at the end, 248 s):
- SPIR-V corpus: unchanged, A 728 match, B 773 match, 0 differ.
- piglit: route B 1338 match (up from 1313), the same 17 differences.
- bgfx: route B 307 match (up from 301), the same 3 differences, 6 not compared (`rgba16f` storage
  images), 5 fail by name (a storage image of another format).
- dEQP (13,630 items): 4,923 reach no route (expected failures 4,396: compile 3,983, link 189,
  compile or link 144, validation 80; tessellation 280 and geometry 179;
  50 with outputs past the colour targets; 13 need `GL_ANDROID_extension_pack_es31a`; a few that
  Mesa's compiler refuses). Of the other 8,707: route B 8,574 match, 126 differ, 2 not compared, 5
  fail by name or validation; route A converts 129 and matches none (Zink's compiler takes only its
  own lowered IO, as with piglit).
- dEQP's 126 differences are all undefined behaviour on the seeded inputs: integer division by
  zero (`swizzle_math_operations.test` `vector_divide` of `ivec` and `uvec`, 68; the `div` cases of
  `implicit_conversions.test`, 42, where a float divides by a zero made from an integer and the
  sign of that zero differs: NIR's `i2f(f2i(x))` to `ftrunc(x)` keeps a negative zero), and an
  array indexed past its end with a seeded index (`large_constant_arrays.test`, 16). Left as
  differences with this reason, as piglit's are.
- dEQP's 5 route failures: a texel buffer, an image atomic, a storage image of another format (all
  named), and one cube map array program naga refuses ("Index 2 is out of bounds", to look at); 2
  not compared: a uniform llvmpipe gives no value for (`linkage.test#varying_7`).

Run 10, with Godot's Compatibility shaders beside the rest (17,650 items;
`results/run10/nir-results-r10a.jsonl`; x86-64 machine, 192 children, load 8.4 at the start and
10.8 at the end, 248 s):
- SPIR-V corpus: route B 776 match (up from 773), route A 731 (up from 728), 0 differ. The 3 new
  on each are the vertex programs without a position (HLSL's and Slang's `geometryshader/base.vert`,
  HLSL's `displacement/base.vert`).
- piglit: route B 1341 match (up from 1338: `glsl-no-glposition`, `sample-qualifier-vs-fs`,
  `pervertex-pointsize-vs-out-fs-140`, all without a position), the same 17 differences.
- bgfx and dEQP: as run 9 (307 and 8,574 match; 3 and 126 differ, same reasons).
- Godot (241 items): 7 reach no route, refused by Mesa's compiler with its log: 4 with
  `RENDER_MOTION_VECTORS` set put an attribute at location 16, past the 16 attributes our OpenGL
  reports (Godot asks for motion vectors only where the driver has more); 3 are combinations Godot
  never builds (`canvas` `USE_INSTANCING` alone, `scene` depth modes with `USE_ADDITIVE_LIGHTING`).
  Of the other 234: route B 231 match, 2 differ, 1 not compared; route A converts none (Zink's
  compiler and GLSL's lowered IO, as with piglit).
- Godot's 2 differences: `effects/cubemap_filter.glsl` (both modes) indexes a `mat3[6]` with the
  seeded `face_id`, negative on the seed: undefined. Confirmed: with the index clamped to 0..5 (a
  probe copy, not kept) both match (`results/run10/nir-results-r10-probe.jsonl`). Left as
  differences with this reason, as piglit's are. Not compared: `canvas_sdf.glsl#MODE_LOAD_SHRINK`
  loops `1 << shift` squared times with the seeded `shift` negative (undefined; the reference does
  not end), the class of `irradiancecube.frag`.

Run 11, with Godot's RenderingDevice shaders beside the rest (18,222 items;
`results/run11/nir-results-r11b.jsonl`; x86-64 machine, 192 children, load 26.7 at the end (another
run of ours had just ended), 250 s):
- In all: route B converts 11,811 of 12,234 programs that reach a route and matches 11,626 (155
  differ, 30 not compared); route A converts 1,154 and matches 993 (7 differ, 154 not compared).
- SPIR-V samples, piglit, dEQP, Godot's Compatibility shaders: as run 10.
- bgfx: route B 316 match (up from 307), 5 differ, 0 not compared; all 321 convert. The 2 new
  differences were not compared before (`r16f` and `rg16f` storage images were not made):
  `cs_assao_prepare_depths_and_normals` and its `_half` variant write storage images whose values
  differ well past rounding (e.g. 0.000039 against 0.00083). Not yet explained: to look at (Next).
- Godot RenderingDevice (572 programs): 54 need what our Vulkan 1.1 does not report (16-bit floats
  and the like, counted apart). Of the other 518: route B 387 match, 5 differ, 4 not compared, 122
  fail by name; route A 237 match, 7 differ, 274 fail (Zink's compiler aborts on `bany_inequal`,
  `ball_iequal` and on derefs of some variable modes; multiview refused by validation). Route B's
  failures: `load_view_index` 66 (multiview: needs a helper value, Requests), arrays of textures
  and samplers 21 and of storage images 3 (15.4's rewrites, blocked), `reduce` 16 (subgroups,
  which the Vulkan driver lowers to one invocation before the route, 15.4: G10), a subpass input 4,
  `image_deref_atomic` 3, `load_helper_invocation` 2, a storage image without a format 2 (15.4,
  blocked), a structure member crossing the structure's end 2 (FSR2, naga validation), a texture
  operand of another kind 2, `quad_swap_horizontal` 1. Not compared: multisampled textures 4.
  Differences: `sdfgi_debug_probes` vertex programs (4; route A differs on the same ones, so the
  harness or undefined behaviour is suspected, not yet confirmed) and `fsr2_tcr_autogen` (1).

Run 12, the same 18,222 items after run 12's fixes (`results/run12/nir-results-r12b.jsonl`;
x86-64 machine, 172 children on cores 10-95 and 106-191 (cores 0-9 and their siblings left to the
timed choice), load 6.0 at the start and 13.3 at the end, 251 s):
- In all: route B converts 11,811 and matches 11,637 (up from 11,626), 153 differ, 21 not
  compared; route A unchanged (993 match, 7 differ, 154 not compared).
- bgfx: route B 318 match (up from 316), 3 differ (run 8's three, same reasons). The two
  `cs_assao_prepare_depths_and_normals` programs were a route fault: route B dropped the offset of
  `texelFetchOffset`, so every target read the texel at the bare coordinate (the route's four
  targets held the same value). Fixed (Done); they match.
- piglit: route B 1350 match (up from 1341: the 9 subroutine programs), the same 17 differences, 4
  not compared.
- Godot's RenderingDevice programs: the 5 differences are undefined behaviour on the seeded
  inputs, each confirmed by a probe copy of the program (not kept; `probe-sdfgi`, `probe-fsr2`
  under the x86 machine's build folder): `sdfgi_debug_probes` computes `probe_index %
  params.probe_axis_size` and `occluder_index % diameter` with the seeded size 0 (an integer
  remainder by zero; naga's SPIR-V makes it defined, lavapipe does not): with the size kept at
  least 2 and `probe_voxels` at least 1 all four match on both routes.
  `fsr2_tcr_autogen` divides by the seeded `iRenderSize`, 0, and turns the infinite coordinate
  into an integer (undefined): with the render size kept at least 1 it matches. Left as
  differences with these reasons, as piglit's are.
- Godot's Compatibility shaders, dEQP and the SPIR-V samples: as run 11 (the one multisampled
  program, `deferredmultisampling/deferred.frag`, briefly failed validation after the offset
  lowering, fixed before this run: it is again not compared, as the harness does not fill
  multisampled textures).

Run 13, the same 18,222 items after run 13's changes (`results/run13/nir-results-r13a.jsonl`;
x86-64 machine, 172 children on cores 10-95 and 106-191 beside the timed choice, load 14.3 at the
start and 3.7 at the end, 732 s, most of it the references that do not end, held on further seeds):
- In all: route B converts 11,813 of 12,234 (up from 11,811) and matches 11,651 (up from 11,637),
  153 differ (the same, with the same reasons), 9 not compared (down from 21; each named in
  Harness limits, with the bindless program llvmpipe refuses); route A matches 1,000 (up from 993),
  7 differ, 147 not compared.
- The 14 new matches on route B: the 2 dEQP cube programs (route faults, fixed), the 5 programs
  reading multisampled textures, and the 7 whose reference did not end on the run's seed: the six
  `irradiancecube.frag` on seed 0x5eed + 1 or + 3, `canvas_sdf.glsl#MODE_LOAD_SHRINK` on + 3. No
  outcome changed in any other way (`scripts/outcome_changes.py`).

The choice (final; 15.3): the route is B (`nir-naga`, `src/direct.rs`). It follows from the
counts, which are far apart on every source; the times agree with them.
- Over the whole corpus (run 13's corpus run, 18,222 items, both routes, the comparison):
  B matches 11,651 programs, A 1,000. By source: SPIR-V samples B 783 and A 738; the Vulkan CTS's alpha_to_one vertex program 1 each; Godot's
  RenderingDevice SPIR-V B 391 and A 237; piglit B 1,350 and A 3; bgfx B 318 and A 21; dEQP
  B 8,576 and A 0; Godot's Compatibility shaders B 232 and A 0 (A takes almost no GLSL: Zink's
  compiler needs Zink's own lowered IO). On the 998 programs both match, at 172 at once on a busy
  machine: A median 542 us, B median 429 us.
- Timed alone (`corpus/choose`'s way: optimised, one program and route at a time, ten cores), on
  a seeded sample of 2,000 items with run 13's code (`WC_NIR_SHADERS=2000`, seed 0x5eed;
  `results/run13/nir-choice-r13.jsonl`; x86-64 machine, cores 10-19 (0-9 held run 11's choice),
  load 3.8 at the start and 2.6 at the end, 1691 s): B matches 1,285 and A 119 (of 1,346 that
  reach a route); on the 118 both match, A median 237 us, mean 325 us; B median 174 us, mean
  213 us.
- Timed alone over the whole corpus (run 11's `corpus/choose`, run 11's code, all 18,222 items,
  one program and route at a time on cores 0-9; job `graphics-nir-choice/20261120-064354-2508593`;
  `results/run13/nir-choice-r11.jsonl`; x86-64 machine, load 23.7 at the start (other runs of
  ours) and 0.6 at the end, other tracks' and this track's runs beside it on other cores, 16,673 s
  wall): B converts 11,811 of 12,234 and matches 11,626; A converts 1,154 and matches 993. On the
  991 programs both match: A median 252 us, mean 404 us; B median 193 us, mean 250 us. B is ahead
  on the count by 10,633 programs and 24 % faster at the median. Its counts are run 11's corpus
  run's exactly (run 12's and 13's fixes came after).
- Run 4's timed choice, on the SPIR-V corpus (ten cores, load 5.5 to 3.2, before the Zink patch):
  A 726, B 773 match; on the 725 both match, A median 219 us, B median 107 us.
- Dolphin is not in the corpus (Next 4); the choice does not wait on it: B leads on every source.

Harness limits left, each with why it is not closed (run 13; programs route B converts but the
comparison does not hold, of 18,222 items):
- A fragment input that is a block (1: `meshshader/meshshader.frag`, fed by a mesh shader, a stage
  the common form lacks). The feeder is written from the program's inputs and writes no block;
  splitting a block into its members' locations is the way to close it, left because its one
  program can only be fed by a stage the corpus counts as absent.
- A multisampled texture on OpenGL's side (1: dEQP `es32/android_extension_pack.test#extension_directive.oes_texture_storage_multisample_2d_array`)
  and multisampled depth textures (0 programs): OpenGL also copies no texels into one, so it needs
  a draw like lavapipe's on llvmpipe; left for its one program.
- Fixed-function state values (2 piglit: `glsl-fs-normalmatrix`, `glsl-vs-ff-frag`, which read
  `gl_NormalMatrix` and its kin): their values are what st/mesa computes from OpenGL's matrices
  (an inverse transpose), which is G9's state tracking; the bridge also names them with an empty
  name (a fault to fix in G9, which reports them). Not compared until then.
- A uniform llvmpipe's linker removes and the route's program reads (2 dEQP: `linkage.test#varying_7`,
  `varying.rules.vertex_use_fragment_use`, `u_false`): OpenGL has no value for it, and inventing one
  would compare nothing. Left not compared.
- A drawn program past llvmpipe's 128 output components once the harness's position output is
  added (1 piglit: `vs-fs-max-in-out-components`): the reference refuses it; left.
- `alpha_to_one`'s fragment program: held by G10's CTS cases (lavapipe does not run
  `demote_samples`).

Next (after G8's check):
1. G9, the OpenGL driver (15.8). It hands the route NIR made with the bridge's compiler options and
   st/mesa's variants; its own corpus run holds those variants, the fixed-function state values
   above among them.
2. When the shaders track has its entry point: the 15.4 rewrites on route B's output; Godot's
   RenderingDevice programs then give corpus programs for arrays of textures (21) and formatless
   storage (2).
3. When `HelperValue::BufferRange` is decided: robustBufferAccess's steps 1 and 3; add
   `dEQP-VK.robustness.buffer_access` to `corpus/cts-programs`' groups and lower those programs
   before both runs.
4. Dolphin (GPL: run as a test input only, never copied or paraphrased) is left out of G8's corpus:
   its shaders exist only when Dolphin runs, and no permissively licensed FIFO log or homebrew to
   boot was found. One clean way is recorded for later: a boot image written here (a few
   instructions of our own, so its licence is ours) makes Dolphin compile its ubershaders at start
   on llvmpipe with `MESA_SHADER_DUMP_PATH` set. Not done in G8.
5. Candidate A is not developed further (B is chosen); its failures stay listed as they are.

## How to run

- The corpus: `crates/wcontainers-nir/corpus/fetch` (into `$CARGO_TARGET_DIR/nir-corpus`), then with
  `WCONTAINERS_MESA` set: `WC_NIR_CORPUS=$CARGO_TARGET_DIR/nir-corpus WC_NIR_RESULTS=<file> cargo
  test -p wcontainers-nir --test corpus`. From the Mac: `orchestrator/cloud/x86-run <checkout> --name
  graphics-nir --jobs 64 -- 'crates/wcontainers-nir/corpus/fetch && export
  WCONTAINERS_MESA=$CARGO_TARGET_DIR/mesa/build && WC_NIR_CORPUS=$CARGO_TARGET_DIR/nir-corpus
  WC_NIR_RESULTS=$CARGO_TARGET_DIR/nir-results-a.jsonl cargo test -p wcontainers-nir --test corpus'`,
  then `x86-run --fetch graphics-nir target/nir-results-a.jsonl <scratch>`. 1005 shaders take about
  2 minutes with the comparison (192 at once). Run 3's results are in
  `pyro-parked/wcontainers-scratch/graphics-nir/results/run3/nir-results-a.jsonl` (run 2's, conversion
  only, one folder up); run 4's in `results/run4/` (`nir-results-r4b.jsonl` the last, with the
  patch; `nir-choice-r4.jsonl` the timed one); `scripts/reasons.py <results> [n]` counts the reasons.
- The choice between the candidates, one command: `crates/wcontainers-nir/corpus/choose [results]`
  (fetches the corpus, builds optimised, runs one shader and route at a time on ten cores; its last
  lines give each route's counts and its median and mean time on the shaders every route matches).
  From the Mac: `x86-run <checkout> --name graphics-nir -- 'WC_NIR_TIMING_CORES=0-9
  crates/wcontainers-nir/corpus/choose $CARGO_TARGET_DIR/nir-choice.jsonl'` (the whole corpus of 18,222 items took 4 h 38 min in run 11; most of it is Zink's compiler
  aborting on GLSL in route A, each abort a core dump the machine's `apport` writes).
  `WC_NIR_SHADERS=<n>` takes a seeded sample: for the independent check,
  `WC_NIR_TIMING_CORES=0-9 WC_NIR_SHADERS=2000 crates/wcontainers-nir/corpus/choose
  $CARGO_TARGET_DIR/nir-choice-check.jsonl` (run 13: 1691 s; B 1,285 match, A 119). A reference
  that does not end on the seeded inputs is held on up to four seeds, two minutes each, so a
  sample with such programs takes longer.
- The licence of every linked Mesa file: `crates/wcontainers-nir/mesa/linked-licences [file]` (after
  the Mesa build; the route test linked with a map, each kept object traced to its source).

- Build Mesa, from the Mac: `orchestrator/cloud/x86-run <checkout> --name graphics-nir --jobs 64 --
  'crates/wcontainers-nir/mesa/build path:/work/flake/graphics-nir'`. On any Linux machine with Nix:
  `crates/wcontainers-nir/mesa/build` (its argument, the flake, defaults to the checkout). The tools
  come from Nixpkgs' own `mesa` package (`nix print-dev-env --inputs-from <flake> nixpkgs#mesa`), so the
  repository's flake is unchanged and the tools are those of its locked nixpkgs. Output in
  `$CARGO_TARGET_DIR/mesa/{src,build,install}` (target/mesa without it). The script refuses a tag whose
  commit is not the pinned one, makes the build folder again when the commit or the options change
  (Meson reads a machine file only when a folder is made), and fails unless llvmpipe and lavapipe were
  built.
- GLSL tests: `cargo test -p wcontainers-nir --test route --test glsl_buffers --test
  glsl_reference` (with `WCONTAINERS_MESA`, on the x86 machine). The corpus run takes piglit's
  `.shader_test` files beside the `.spv` files, from the same `WC_NIR_CORPUS`; all 3546 take about
  4 minutes at 192 at once. `scripts/glsl_reasons.py <results> [route] [n]` (scratch) counts
  piglit's reasons; `scripts/piglit_outcomes.py <results> [route] [n] [prefix]` counts every
  outcome of the files under a prefix (`piglit/`, `bgfx/`). bgfx's shaders come with the same
  `corpus/fetch` and run in the same corpus run (any `.bin` in a folder named `glsl`).
- Test with Mesa (on the x86 machine, after the build):
  `export WCONTAINERS_MESA=$CARGO_TARGET_DIR/mesa/build; cargo test -p wcontainers-nir` and
  `cargo clippy -p wcontainers-nir --all-targets -- -D warnings`; and both again without the variable.
- dEQP's cases come with the same `corpus/fetch` (VK-GL-CTS's shader library data, sparse) and run
  in the same corpus run: 13,630 items take about 35 s alone (`WC_NIR_CORPUS` at
  `nir-corpus/vk-gl-cts`). `scripts/deqp_differences.py <results>` (scratch) groups route B's
  differences by file and message.
- The Vulkan CTS's programs: `crates/wcontainers-nir/corpus/cts-programs [flake]` (from the Mac:
  `x86-run <checkout> --name graphics-nir --jobs 96 -- 'crates/wcontainers-nir/corpus/cts-programs
  path:/work/flake/graphics-nir'`); the corpus run takes them from `nir-corpus` like the rest.
- The literal check needs the git history, which the rented machine's copy lacks. Run 6 ran it on
  the x86 machine from a git bundle of the branch (`git bundle create <scratch>/nir.bundle
  phase-2-graphics-nir`, sent with `x86-run --ssh 'cat > /work/probes/graphics-nir.bundle'`,
  cloned there, `cargo fetch`, then the check in `nix develop`; the clone and its build deleted
  after). Runs 1 to 5 ran it on the Mac (`RAYON_NUM_THREADS=3 cargo run -j 4 -q -p
  wcontainers-literals -- check`). With
  `crates/wcontainers-shaders/dxbc-spirv`'s own submodule checked out the check fails on its Python
  scripts (the baseline holds it not checked out): fetch only the shaders crate's two submodules and
  dxil-spirv's `third_party/spirv-headers`, which the shaders crate's build needs.

- A quick look at one shader: copy it alone into a folder and point `WC_NIR_CORPUS` there with
  `WC_NIR_DUMP` set; `nix shell nixpkgs#spirv-tools -c spirv-dis` reads the dumps.
- Run 8 fetched VK-GL-CTS's shader library data (sparse, at 5bb4c7e8) and a sparse bgfx on the
  x86 machine under `/work/probes` to read their formats; both deleted at the end of the run.
- Runs beside the timed choice: pin to `taskset -c 10-95,106-191` (cores 0-9 and their hyperthread
  siblings 96-105 are the choice's), and never build `--release` in `/work/target/graphics-nir`
  while it runs. Probe programs (a changed copy of one corpus program, compiled with Nixpkgs'
  glslang as `corpus/godot-forward-plus` does) go in a folder of their own under the build folder,
  pointed at by `WC_NIR_CORPUS`. `scripts/outcome_changes.py <old> <new> <route>` (scratch) lists
  the programs whose outcome changed between two results files.
- Kept between runs: a sparse clone of Dolphin's `Source/Core/VideoCommon` at 32a2b828 for reading
  how it makes its shaders (`pyro-parked/wcontainers-scratch/graphics-nir/downloads/dolphin`,
  GPL-2.0-or-later: read only, nothing of it copied or paraphrased here), and bgfx's two ASSAO
  sources and their `uniforms.sh` (`downloads/bgfx-src`, BSD-2-Clause).
- Kept between runs: a sparse clone of Godot at 73ca6be2 for reading its shader formats and their
  C++ users (`pyro-parked/wcontainers-scratch/graphics-nir/downloads/godot`, 12 MB, blobs fetched
  on demand: never `git grep` its history there, which fetches every blob; add paths with `git
  sparse-checkout add /<path>`).
- Godot's RenderingDevice shaders, one command after the fetch's sources: `crates/wcontainers-nir/corpus/godot-forward-plus [flake]`
  (from the Mac: `x86-run <checkout> --name graphics-nir --jobs 64 --
  'crates/wcontainers-nir/corpus/godot-forward-plus path:/work/flake/graphics-nir'`; 9 s). It
  fetches, writes the variants' sources (the corpus binary's `--godot-rd`, no Mesa needed) and
  compiles them with Nixpkgs' glslang into `nir-corpus/godot-forward-plus`, listing what glslang
  refuses in `glslang-refused`. Run it (and `corpus/cts-programs`) before `corpus/choose`, which
  fetches only `corpus/sources`. Alone: `WC_NIR_CORPUS=$CARGO_TARGET_DIR/nir-corpus/godot-forward-plus`.
- Godot's shaders come with the same `corpus/fetch` and run in the same corpus run (any `.glsl`
  under `drivers/gles3/shaders`); alone: `WC_NIR_CORPUS=$CARGO_TARGET_DIR/nir-corpus/godot`.
- Kept between runs: a sparse clone of VK-GL-CTS at 5bb4c7e8 for reading dEQP's formats
  (`pyro-parked/wcontainers-scratch/graphics-nir/downloads/vkglcts`, no build output); on the x86
  machine, the CTS source and build under `/work/target/graphics-nir/vk-gl-cts` (with Mesa and the
  rest, 18 GB in all after run 9).
- Kept between runs: a shallow clone of the pinned Mesa for reading on the Mac
  (`pyro-parked/wcontainers-scratch/graphics-nir/mesa-src`, 560 MB, no build output; logs beside it in
  `logs/`), and the Mesa build and cargo target on the x86 machine (`/work/target/graphics-nir`).
  No scratch copy of the checkout was made on the Mac, the laptop or the desktop. The literal check's
  own build in the checkout's `target` (282 MB) is deleted at the end of each run. On the x86
  machine `/work/target/graphics-nir` (11 GB after run 4: Mesa, the corpus, cargo's debug and release output, the last results) is kept.

## Measurements

- The whole corpus run (run 13, 18,222 items, 172 children on cores 10-95 and 106-191, x86-64
  machine, load 14.3 at the start and 3.7 at the end, the timed choice on cores 0-9 beside it):
  732 s, up from 251 s: the references that do not end are now run on up to four seeds, two
  minutes each, one after another. Time per shader to the validated common form at 172 at once on
  the 998 shaders both routes match: A median 542 us, mean 831 us; B median 429 us, mean 519 us.
- The whole suite of wcontainers-nir (run 13, x86-64 machine, load about 14): without Mesa every
  test target builds and passes (no tests run without it); with Mesa `glsl_buffers` 1,
  `glsl_reference` 7, `narrow` 1, `route` 11 passed.

- The whole corpus run (run 12, 18,222 items, 172 children on cores 10-95 and 106-191, x86-64
  machine, load 6.0 at the start and 13.3 at the end, the timed choice running on cores 0-9 and
  other tracks' jobs beside): 251 s. Time per shader to the validated common form at 172 at once
  on the 991 shaders both routes match: A median 454 us, mean 714 us; B median 358 us, mean
  440 us. The same run before the subroutine change (load 87 at the start, 27 at the end): 253 s.
- The whole suite of wcontainers-nir (run 12, x86-64 machine, load about 6): without Mesa every
  test target builds and passes (no tests run without it); with Mesa `glsl_buffers` 1, `glsl_reference` 7,
  `narrow` 1, `route` 11 passed.
- The whole corpus run (run 11, 18,222 items, 192 children, x86-64 machine, load 10.0 at the
  start and 26.7 at the end, another run of ours just before): 250 s. Time per shader to the
  validated common form at 192 at once on the 991 shaders both routes match: A median 488 us, mean
  767 us; B median 381 us, mean 481 us (busy machine; the choice's times are the ten-core ones).
  `corpus/godot-forward-plus` (fetch, the sources, 572 glslang compilations at 192 at once): 9 s,
  load 1.0. Godot's RenderingDevice programs alone: 16 s, load 0.8 to 1.7.
- The whole corpus run (run 10, 17,650 items, 192 children, x86-64 machine, load 8.4 at the start
  and 10.8 at the end): 248 s. Time per shader to the validated common form at 192 at once on the
  748 shaders both routes match: A median 421 us, mean 607 us; B median 346 us, mean 406 us (busy
  machine; the choice's times are the ten-core ones). Godot's 241 items alone: 128 to 134 s (most
  of it the one program whose reference does not end, held to the 120 s limit).

- `corpus/cts-programs` from nothing (VK-GL-CTS clone, its `fetch_sources.py`, CMake and Ninja
  for `vk-build-programs`, the alpha_to_one programs), x86-64 machine, `-j 96`, load 5.2 at the
  start and 10.4 at the end: 320 s wall, 76 min user.
- dEQP's 13,630 items alone, 192 children, x86-64 machine, load 0.1 to 32.6: about 32 s.

- Mesa 26.3.1 clean build (configure, 1239 steps, install), x86-64 machine (c7i.metal-48xl, 192
  threads), `-j 64`: 45.9 s wall, 11 min 37 s user, load average 4.96 at the start and 10.51 at the
  end (other tracks' jobs). An earlier clean build of the same options with SPIRV-Tools: 45.9 s wall
  at load 0.43 at the start.
- Corpus run with the comparison, 1005 shaders, 192 children, x86-64 machine at load 0.3 rising to
  5.3: 124 s including an incremental build (run 3).
- The corpus run with bgfx (run 8, 3871 files, x86-64 machine, 192 children, load 4.5 at the start
  and 16.1 at the end): 250 s. At 192 at once, on the 735 shaders both routes match: A median
  441 us, B median 352 us (a busy machine; the choice's times are the ten-core ones).
- The whole suite with the SPIR-V corpus and piglit compared (run 7, x86-64 machine, 192 children,
  load 4.9 at the start and 13.2 at the end): the corpus run 245 s for 3546 files. Time per shader
  to the validated common form at 192 at once, on the 728 shaders both match: A median 431 us, B
  median 341 us (busy machine; the choice's times are the ten-core ones below).
- The whole suite with the SPIR-V corpus compared (run 6, after the GLSL work; x86-64 machine,
  192 children, load 1.0 at the start and 12.5 at the end, 245 s with the build;
  `results/run6/nir-results-r6.jsonl`): the same counts as runs 4 and 5, A 728 match, B 773
  match, 0 differ, 8 not compared on each.
- `tests/glsl_buffers.rs` and `tests/route.rs` (run 6): 0.08 s and 0.01 s, x86-64 machine, load
  0.5 to 4.2.
- `tests/narrow.rs` (2,097,152 invocations, both programs on lavapipe): 0.47 s, x86-64 machine,
  load 0.08 (run 5).
- The choice's timing (run 4, `corpus/choose`, x86-64 machine, ten cores, one child at a time, load
  5.5 at the start and 3.2 at the end with another track's tests running): 1781 s in all; time per
  shader to the validated common form on the 725 shaders both match: A median 219 us, mean 261 us;
  B median 107 us, mean 125 us. The same at 192 children at once (load 9 to 25): A median 383 us, B
  median 196 us.

## Routed decisions (15.9, owner not yet answered; followed as the orchestrator set them)

- Our own OpenGL driver on the interface, as 15.2 recommends. gex_wgpu and wvk are not adopted; nothing of
  theirs is copied or paraphrased (their repository has no licence; a related project is AGPL-3.0).
- Crate and module names are the plan's suggestions: `wcontainers-nir`, `wcontainers-opengl`.
- Mesa under its MIT licence with the permissive files 15.2 lists; dEQP, VK-GL-CTS and piglit for testing only.
- No change to the interface crate: what the second batch needs from it (15.7) is written below as requests.

## Licences brought in (for the owner to confirm)

- Mesa 26.3.1 (fetched by the build script, not in this repository). Its `docs/license.rst` says each
  file's SPDX tag governs; the core is MIT. Linked into wcontainers-nir's tests today: Mesa's static
  libraries `libzink`, `libglsl_standalone`, `libglsl`, `libglcpp`, `libmesa`, `libvtn`, `libnir`,
  `libcompiler`, `libmesa_util`, `libblake3`, `libmesa_util_c11`, and since run 6 `libglsl_util`
  and `libglcpp_standalone` (the linker keeps only the objects used). Run 6, with GLSL linked
  (`results/run6/nir-linked-licences-r6.txt`): 243 files, 77 of them from the GLSL libraries
  (`libglsl` 68, `libglsl_util` 4, `libglcpp` 3, `libglcpp_standalone` 1, `libglsl_standalone`
  1), all with the MIT permission text. Four of those are generated and carry a second notice:
  `glsl_parser.cpp` and `glcpp-parse.c` are Bison's output (Bison's skeleton is GPL-3.0-or-later
  with the Bison special exception, which lets the output be distributed under any terms),
  `glsl_lexer.cpp` and `glcpp-lex.c` are flex's output (flex's own BSD-style licence). The other
  166 are run 4's files and licences, unchanged. Kept by the linker in run 4 (`mesa/linked-licences`; list in scratch
  `results/run4/nir-linked-licences.txt`): 129 files, from `libnir` 65, `libmesa_util` 37, `libvtn`
  14, `libblake3` 7, `libcompiler` 3, `libzink` 2, `libmesa_util_c11` 1. Their licences: 89 carry
  the MIT permission text, 28 the SPDX tag MIT, 2 are generated by Mesa's build from MIT scripts
  (`nir_opcodes.c`, `nir_opt_algebraic.c`), 1 BSL-1.0 (`c11/impl/threads_posix.c`), and 9 carry no
  text in the file: the 7 of `util/blake3` (BLAKE3 1.8.2 copied verbatim, its README says;
  upstream's licence is CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception),
  `util/softfloat.c` (Berkeley SoftFloat 3e, BSD-3-Clause text in the file, which the script's MIT
  match does not count) and `util/format/u_format_etc.c` (MIT text in the header it includes,
  `texcompress_etc_tmp.h`). All permissive. GLSL's libraries are not linked yet; the list is made
  again when they are.
- zlib (Zlib licence), linked as Mesa's build found it (Nixpkgs' static zlib).
- `serde_json` 1 and `serde` 1 (both MIT OR Apache-2.0), build- and dev-dependencies of
  wcontainers-nir; `cc` (MIT OR Apache-2.0), already in the workspace.
- `blake3` 1 (CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception, already in the workspace), a
  dependency and build-dependency (the cache key and the converter's version).
- `wcontainers-gpu` (ours), a dependency (`SlotKind`, `ReflectedBinding`, `ShaderStage`).
- `glow` 0.18 (MIT OR Apache-2.0) and `khronos-egl` 6 with `dynamic` (MIT OR Apache-2.0),
  dev-dependencies since run 7 (llvmpipe in the corpus run); both already in the workspace through
  the WebGL back end.
- `ash` 0.38 (MIT OR Apache-2.0) and `libloading` 0.8 (ISC), dev-dependencies (lavapipe in the
  corpus run); both already in the lock file through wgpu.
- `spirv` 0.4 (Apache-2.0; the SPIR-V grammar's names, already in the workspace through naga), a
  dependency. `rand` 0.9 and `rand_chacha` 0.9 (MIT OR Apache-2.0, already in the workspace),
  dev-dependencies.
- Corpus, test data only, fetched into the build folder and never in the repository or shipped:
  VK-GL-CTS at 5bb4c7e8 (since run 9; Apache-2.0, its `LICENSE`; only `data/gles2/shaders`,
  `data/gles3/shaders`, `data/gles31/shaders` and the licence are fetched by `corpus/fetch`;
  `corpus/cts-programs` fetches the whole tree and the dependencies its `fetch_sources.py` pins:
  glslang (BSD-3-Clause and others), SPIRV-Tools and SPIRV-Headers (Apache-2.0 and MIT), Amber
  (Apache-2.0), jsoncpp (MIT), Vulkan-Docs and the video parser (Apache-2.0 and MIT), built only to
  dump test programs, never linked into anything of ours);
  bgfx at e76ecedf (since run 8; BSD-2-Clause, its `LICENSE`; only `examples/runtime/shaders/glsl`
  and the licence are fetched);
  Godot at 73ca6be2 (since run 10; MIT, its `LICENSE.txt`; only `drivers/gles3/shaders`,
  `LICENSE.txt` and `COPYRIGHT.txt` are fetched; `COPYRIGHT.txt` names no other licence for those
  files, and `tonemap_inc.glsl` notes an ACES fit adapted from MJP's BakingLab, MIT);
  since run 11 also Godot's `servers/rendering/renderer_rd/shaders` (MIT) and the AMD FSR and FSR2
  code it includes (`thirdparty/amd-fsr`, `thirdparty/amd-fsr2`: Expat, that is MIT, by Godot's
  `COPYRIGHT.txt`; their own `license.txt` and `LICENSE.txt` fetched beside them), compiled by
  glslang (Nixpkgs' `glslang`, BSD-3-Clause and others; a tool run by `corpus/godot-forward-plus`,
  never linked);
  Sascha Willems' Vulkan samples at d2d49bf7 (MIT, `LICENSE.md`); piglit at 92510281 (since run 7;
  its `COPYING`: MIT for most of it, with some test shaders under their projects' own licences,
  e.g. GStreamer's in `glslparsertest`; piglit is for testing only, as the routed decision says,
  and never a dependency).
- Build tools only, from the repository's locked nixpkgs: Meson 1.12.1 (Apache-2.0), Ninja (Apache-2.0),
  LLVM 21.1.8 (Apache-2.0 WITH LLVM-exception; llvmpipe and lavapipe use it, wcontainers-nir does not
  link it), GCC 16.2.0 (GPL-3.0 with the GCC runtime library exception).

## Departures from the plan

- 6.4 states the binding rule for Direct3D. For Vulkan and OpenGL programs the NIR route reads it as
  Direct3D 12's: within the stage's set each kind's resources in the order of the program's own
  (set, binding), with the set in `ReflectedBinding::space` and the binding in `register`. A combined
  image sampler is a texture and a sampler reflected at the same set and binding.
- The NIR the routes take is fixed by one set of NIR compiler options in the bridge (Zink's list plus
  all 64-bit integer lowering and the instance index counting from the first instance), which 15.3
  does not name; G9 and G10 must hand over NIR made with the same options.
- The corpus is fetched by `corpus/fetch` into the build folder on the rented machine (each source
  pinned by commit), not kept in the Mac's scratch (15.8): the runs happen there, and the pins make
  it the same corpus everywhere.
- Mesa is not a git submodule at `third_party/mesa` (15.2). It is pinned by commit in
  `crates/wcontainers-nir/mesa/pin.toml` and fetched by the build script. Reasons: the literal check
  scans every Rust file and script of a submodule (Mesa has hundreds of Python scripts and Rust files),
  any new baseline or policy line is refused as growth, and an unchecked-out submodule is itself a
  finding; and where the fork 15.2 describes is kept is the owner's decision (15.9). Until then the
  patches live as files in `crates/wcontainers-nir/mesa/patches/` (one so far, Zink's), applied by
  the build script; they move into the fork when there is one.
- The Mesa build runs on the rented x86-64 machine, not the laptop (15.8 says laptop): the brief of
  2026-11-19 makes the rented machines the default. The choice's times (15.3: "measured on the
  laptop") are taken there too, alone on ten cores, as that machine's notes take timings.
- The choice was made on the SPIR-V corpus first (run 4) and made final over the whole corpus,
  GLSL and the other sources in, by `corpus/choose` over all 18,222 items (run 11's, read in run 13).
- Zink is built beside llvmpipe and lavapipe: its `nir_to_spirv` is candidate A's first step.
  SPIRV-Tools is off (Mesa takes it when present; nothing of ours needs it).
- Candidate B reads NIR through a flattened copy the bridge makes (`bridge/flat.c`), not through
  NIR's own structures from Rust: bindgen is out (below), and the copy keeps Mesa's numbering and
  layout on the C side, so a Mesa update changes C only.
- The bridge's functions are declared by hand in `src/mesa.rs` and `src/direct.rs`, not by bindgen: bindgen's output
  is included from Cargo's `OUT_DIR`, whose name the literal check refuses as a new code string here
  (its one constant, `CARGO_OUT_DIR`, is private to wcontainers-literals).

- Lowered IO keeps a stage value's interpolation only where a fragment input is loaded. Route B
  gives a vertex output Perspective (Flat for an integer one); where a fragment program reads it
  another way, the pipeline (G9, G10) must give the vertex side the fragment side's. The plan does
  not say; the comparison runs each stage alone, so it does not see this.
- The 16-bit test's reference is lavapipe running the same program with native 16-bit integers and
  Rust's 16-bit arithmetic, both, rather than a corpus mode (the program is the test's own).
- The route's module is run by lavapipe through naga's SPIR-V writer, not by the recording back
  end's interpreter (`wcontainers-gpu-test`) or the WebGPU back end on the RTX 3080 (15.3). Reasons:
  the interpreter's limits (no derivatives, implicit levels only on one-level views, no atomics or
  workgroup memory) would leave most real fragment programs to the GPU queue, while lavapipe runs
  every program the reference runs, on the same machine, with one harness for both sides. What this
  holds is the conversion (NIR to the common form); naga's SPIR-V writer is the same one wgpu's
  Vulkan back end uses. The WebGPU back end's own conversion is the shaders track's.
- The plan says "within the format's rounding" for floats; the targets are 32-bit, and two programs
  that differ only in the order of operations differ by more than one rounding. The tolerance is
  1024 units in the last place or 2^-20 absolute (`compare.rs`), stated and checked to still catch a
  change of constants (Where it stands).
- GLSL is compiled on Mesa's standalone scaffolding with st/mesa's steps after a link written out
  in `glsl.cpp` for one set of capabilities, not by st/mesa itself (which needs a gallium screen
  and context: G9's driver). The steps and their order are st/mesa's; the shader-variant steps
  st/mesa takes at draw time (clamped colours, flat shading, two-sided colour, user clip planes,
  point size), which depend on the draw's state, are not taken here: G9 hands its variants over
  and its own corpus run holds them.
- OpenGL's buffer accesses reach route B as arrays of words, not as typed blocks: after
  `gl_nir_lower_buffers` only byte offsets remain (the block types are gone from the
  instructions), so the bridge makes the words form, the one 15.4 names for robustBufferAccess.
  Program binding for OpenGL: uniform block index, storage block index after them.
- Vulkan capability classes: a program needing what our Vulkan 1.1 will not report is counted apart,
  not as a route failure (the pass bar's "named reason").

- llvmpipe is reached through EGL's software device (`EGL_EXT_platform_device`), not the surfaceless
  platform 15.2's options name: the rented machine has no render node, and surfaceless takes software
  only when told by an environment variable (`LIBGL_ALWAYS_SOFTWARE`), which a test process should
  not set for itself. The context is OpenGL 4.5 compatibility, llvmpipe's; the bridge reports 4.6
  (G9's), so a program needing 4.6 is refused by the reference and counted not compared.
- The GLSL comparison fills its inputs from OpenGL's reflection of the program on llvmpipe (names,
  types, offsets, strides) and the bridge's names for where each lands in the route's module; the
  SPIR-V comparison fills from the route's module alone.
- GLSL vertex and fragment programs are compared as a drawn program (the vertex program wrapped in
  its own source, a feeder or catcher written beside it), not each stage alone: names between two
  stages do not survive OpenGL's linker, and the draw is how llvmpipe runs a stage. In a vertex
  and fragment pair, what the vertex program computes is compared through what the fragment
  program makes of it; its position is compared only when it is alone (the catcher).
- The corpus's `sources` lines gained a paths column (`.` for the whole tree): bgfx is fetched
  sparsely (its shaders and licence only), as dEQP's data files will be.
- bgfx's 3 differences (infinities from seeded RGBE texels, and one unit in the last place made
  large by `fract`) are left as differences with their reasons, as piglit's 17 are.
- dEQP's cases are run on the corpus's seeded inputs, not on the values in each case's `values`
  block: the corpus holds the route to llvmpipe, not to dEQP's expected results, and seeded inputs
  reach more of each program. Where dEQP compares the outputs in the shader, the corpus writes them
  to colour targets (Done). Of a requirement's alternative extensions the first is taken (dEQP
  takes the first the context reports); a case whose first alternative Mesa's compiler refuses is
  counted as reaching no route with Mesa's log.
- ES programs are compared at full precision on both sides (`#define mediump highp` and the rest):
  llvmpipe lowers `mediump` to 16 bits, our driver does not; both are allowed by GLSL ES, and the
  reference must compute what the route is to compute.
- The 17 differences from undefined behaviour on seeded inputs are left as differences with their
  reason, not hidden: the seed gives indices out of range and negative square roots, which the
  programs' own tests never give.

- Godot's Compatibility shaders are made for desktop OpenGL (`#version 330`, what Godot gives a
  desktop driver; our OpenGL reports 4.6 compatibility), at Godot's default project settings, with
  each mode at the default specializations and with each one specialization flipped, not every
  combination (scene alone has 33 specializations: 2^33 programs, most never built by Godot).
  Combinations Godot never builds that Mesa refuses are counted as reaching no route with Mesa's
  log.
- A vertex program without a position gets (0, 0, 0, -1) in the bridge (NIR), not through the
  shaders crate's `stage::clip_unpositioned` (crate-private, and on naga); the same rule, the same
  value. It moves to the shaders crate's entry point when there is one (Requests).

- Godot's RenderingDevice variants are every variant each `initialize` call lists, at Godot's
  default desktop settings, with no material (as the Compatibility shaders are taken), compiled for
  a Vulkan 1.1 device (SPIR-V 1.3); Godot would compile only the groups a device and project
  enable. Texture streaming (a module the default build leaves out) adds nothing to the scene
  defines.
- The GLSL comment marker Godot's reader cuts is named `GLSL_COMMENT_START`, not imported from the
  literal check's `LINE_COMMENT` (Rust's, private there): the same two characters, another
  language's fact.
- Subgroup operations (`reduce` and the like) are refused by name by route B: 15.4 has the Vulkan
  driver (G10) lower them to their one-invocation meaning before the route, so the corpus, which
  hands over NIR as spirv_to_nir makes it, counts them as failures until G10's step exists.

- A storage cube is a 2D array of its faces in the common form (run 13): the plan does not say;
  WGSL has no storage cubes, and Vulkan and OpenGL address a storage cube's faces as layers.
- A reference that does not end on the seeded inputs is held on the next seeds, up to four (run
  13), rather than counted not compared: the program is the same and the inputs are as seeded as
  before; the seed used is in the results.
- Dolphin is left out of G8's corpus (run 13): no permissively licensed boot image was found, as
  the orchestrator's ruling of 2026-11-19 night requires; the choice is final without it.

## Interface changes

None.

## Requests to other tracks

- Interface track (15.7): everything 15.7 lists; G8 needs `HelperValue::BufferRange` and the slow path
  names `RobustAccessInShader` and `AlphaToOneInShader` before its robustness and alphaToOne parts can
  be reflected.
- Shaders track (a finding): naga's SPIR-V reader fails on HLSL's row-major array of matrices once
  it is decorated (`shadowmappingcascade/scene.frag`, "invalid access As" on candidate A); not needed
  by route B.
- Shaders track: a public entry point applying `variant::apply` and its kin to a naga module already in
  the common form (stage, reflection, variant key), so the NIR route takes the Direct3D rewrites from
  one place (15.4) instead of a copy.
- Interface track: `pub const fn stage_set(stage: ShaderStage) -> u32` in `reflect.rs` beside
  `helper_binding` (vertex and compute 0, fragment 1, plan 6.4). The shaders crate writes the rule as
  a literal in `lower.rs`; the NIR route defines `wcontainers_nir::bind::stage_set` until the
  interface has it, and will import it then.
- Shaders track: `stage::clip_unpositioned` as part of that entry point, so the NIR route gives a
  vertex program without a position the same position from one place (the bridge does it in NIR
  until then).
- Shaders track: the draw's bases rewrite (`HelperValue::DrawBase`) as part of the public entry point
  below, for Vulkan programs reading `BaseVertex` or `BaseInstance`.
- Shaders track: `wcontainers_shaders::capabilities` to add `Capabilities::IMMEDIATES` for a module
  with an `AddressSpace::Immediate` global, as it adds the other extensions a front asks for only
  where the back end has them; until then the NIR route adds it itself.
- Shaders track (a finding, not a request for this route): naga's SPIR-V reader turns a sampled
  array layer into an integer with a truncating `As` (`front/spv/image.rs`), where Vulkan and
  Direct3D round to the nearest even integer; a layer of 1.6 reads layer 1 instead of 2. The NIR
  route rounds in NIR first; dxil-spirv's and dxbc-spirv's output reaches the same reader.
- Interface or shaders track (later, when the Vulkan driver reports `vkCmdDispatchBase`): a helper
  value for the base workgroup; the routes now count dispatches from workgroup 0.
- Interface track: a helper value for the view index (e.g. `HelperValue::ViewIndex`, u32, written
  by the front for each view of a `MultiviewSerial` draw), so a program reading `gl_ViewIndex`
  (Vulkan 1.1 requires `multiview`) has it in the common form; route B refuses `load_view_index`
  by name until then (66 of Godot's RenderingDevice programs).
- Interface track: a public `CUBE_FACES` (the layers of one cube) beside `ViewDimension`;
  `resource.rs` writes it as a literal, and `wcontainers_nir::CUBE_FACES` defines it until the
  interface has it, and will import it then.
- Literals track: `CARGO_OUT_DIR` public (in a public module), so a build script can name `OUT_DIR`
  through it.

## Literal check

Run 13: exit 0 at the run's last code commit, 86cefeb2 (x86-64 machine, from a git bundle;
marks 0). wcontainers-nir still has no baseline lines.
Run 12: exit 0 at the run's last code commit (x86-64 machine, from a git bundle; marks 0).
wcontainers-nir still has no baseline lines.
Run 11: exit 0 at the run's last code commit (x86-64 machine, from a git bundle; marks 0). A first
try found the Godot reader's `LINE_COMMENT` ("//") named like the check's own (Rust's comment, private
there): renamed for what it is, GLSL's (Departures). wcontainers-nir still has no baseline lines.
Run 10: exit 0 at the run's last code commit (x86-64 machine, from a git bundle; marks 0). A
first try found the new reader's comment marker defined beside dEQP's (`COMMENT`, '#'): Godot's
reader now imports dEQP's. wcontainers-nir still has no baseline lines.
Run 9: exit 0 with the run's code (x86-64 machine, from a git bundle; marks 0). A first try
found `"in"`, `"out"` and a decimal point in `tests/corpus/deqp.rs` restating constants of the
WebGL back end and the check (private there): GLSL's qualifiers are now written through one
function of format strings, and the dEQP grammar's one word before a stage list is taken as any
word. wcontainers-nir still has no baseline lines. Run 8: exit 0 at the run's last code commit, 9e0a7c6e's parent (x86-64 machine, from a git bundle; marks 0; a first
try found the GLSL version directive named twice, in `glsl.rs` and the new `bgfx.rs`: now imported);
wcontainers-nir still has no baseline lines. Run 7: exit 0 at the run's last code commit (x86-64 machine, from a git bundle of the branch; marks
0); wcontainers-nir still has no baseline lines. Run 6: exit 0 at 4294fcf0 (x86-64 machine, from a git bundle; marks 0; the 18 changes that could
add lines are the shaders branch's, as before). The bridge's numbers are `#define`s, named once;
the Rust added has no new bare literals (test data apart). Adopted with the branch (merged from the
shaders branch, which carries it). wcontainers-nir has no
baseline lines: the check passes with none for it (run on the Mac, runs 1 to 4, exit 0; in run 4 the
two new scripts' numbers were taken out (the timing cores are passed in, the load read by `read`), and
one baseline line the Direct3D 11 merge left stale was deleted, as the check asked;
it needs the git history, which the rented machines' copies lack). The list of
changes that could add lines is the 18 the shaders branch brought; this branch added none. C files are
not scanned; the bridge names its one version by `#define`s all the same.
