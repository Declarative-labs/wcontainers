//! Tests that state the right value of what a draw's variant does with each of its arguments: the bridge's own
//! documented behavior (bridge.h's comments on `struct wc_variant_settings`, `struct wc_nearest_sampler`,
//! `struct wc_texture_swizzle` and the squares; OpenGL's nearest sampling and level selection; the common form's
//! binding rule of wc_nir_to_common_form) worked by hand into numbers, and the program run by `reference`, an
//! evaluator that shares no code with Mesa or the bridge. No test here only says two results differ.

use std::ffi::{CStr, CString};

use wcontainers_mesa_compiler::{Binding, Flat, MesaCompiler, NearestSampler, PositionTransform, SlotRange, TextureSwizzle, VariantForm, VariantSettings, constants};

use crate::reference::{self, Given, Outputs, Texture, Words};

/// The stages of `glsl_to_nir`'s sources.
const STAGES: usize = constants::STAGES as usize;

/// A vertex program writing one position and point size, and sampling a float and an integer texture at a place and
/// level its uniform block gives: every argument of a draw's variant reaches it (the transform, the swizzles, the
/// nearest samplers, the set and ranges), and its point size gives squares.
const SAMPLING_VERTEX_SOURCE: &str = "#version 450
layout(binding = 0) uniform sampler2D picture;
layout(binding = 1) uniform isampler2D counts;
layout(std140, binding = 0) uniform Block { vec2 at; float lod; };
layout(location = 0) out vec4 swizzled;
layout(location = 1) flat out ivec3 counted;
void main() {
    gl_Position = vec4(0.25, 0.5, 0.75, 2.0);
    gl_PointSize = 6.0;
    swizzled = textureLod(picture, at, 0.0);
    counted = textureLod(counts, at, lod).xyz;
}
";

/// The fragment program after it (the program pair must link).
const PASSING_FRAGMENT_SOURCE: &str = "#version 450
layout(location = 0) in vec4 swizzled;
layout(location = 1) flat in ivec3 counted;
layout(location = 0) out vec4 colour;
void main() {
    colour = swizzled + vec4(counted, 0.0);
}
";

/// A geometry program taking points and making two points of the input's size, each at the input's position moved
/// by (copy, copy, 0, 0).
const TWO_POINTS_GEOMETRY_SOURCE: &str = "#version 450
layout(points) in;
layout(points, max_vertices = 2) out;
layout(location = 0) in vec4 passed[];
layout(location = 0) out vec4 shaded;
void main() {
    for (int copy = 0; copy < 2; copy++) {
        gl_Position = gl_in[0].gl_Position + vec4(float(copy), float(copy), 0.0, 0.0);
        gl_PointSize = gl_in[0].gl_PointSize;
        shaded = passed[0];
        EmitVertex();
    }
}
";

/// A vertex program of the points the geometry program takes.
const POINTS_VERTEX_SOURCE: &str = "#version 450
layout(location = 0) in vec4 at;
layout(location = 0) out vec4 passed;
void main() {
    gl_Position = at;
    gl_PointSize = at.w;
    passed = at;
}
";

const SHADED_FRAGMENT_SOURCE: &str = "#version 450
layout(location = 0) in vec4 shaded;
layout(location = 0) out vec4 colour;
void main() {
    colour = shaded;
}
";

/// The position the sampling program writes.
const POSITION: [f32; 4] = [0.25, 0.5, 0.75, 2.0];
/// Its point size.
const POINT_SIZE: f32 = 6.0;

/// The float texture's one colour and the integer texture's levels (8x8 down to 1x1).
const COLOUR: [f32; 4] = [0.125, 0.25, 0.375, 0.5];
const LEVELS: [[u32; 3]; 4] = [[8, 8, 1], [4, 4, 1], [2, 2, 1], [1, 1, 1]];
/// What an integer texture a nearest sampler does not reach answers (a sample left as the program wrote it).
const SAMPLED_AS_WRITTEN: Words = [9999, 9998, 9997, 9996];

fn compiled<M: MesaCompiler>(compiler: &M, sources: [Option<&str>; STAGES]) -> wcontainers_mesa_compiler::GlslProgram<M::ReceivedVertex, M::ReceivedFragment, M::ReceivedCompute, M::ReceivedGeometry, M::ReceivedTessControl, M::ReceivedTessEvaluation> {
    let owned: Vec<Option<CString>> = sources.iter().map(|source| source.map(|text| CString::new(text).expect("no zero byte"))).collect();
    let texts: [Option<&CStr>; STAGES] = std::array::from_fn(|at| owned[at].as_deref());
    compiler.glsl_to_nir(texts).unwrap_or_else(|log| panic!("the program compiles: {}", String::from_utf8_lossy(&log)))
}

fn no_transform() -> PositionTransform {
    PositionTransform { flip_y: false, half_z: false, reverse_z: false, fixed_point_size: false }
}

/// A plain draw's settings: no transform, set 0, every kind from slot 0 with room for 64.
fn settings() -> VariantSettings {
    VariantSettings {
        transform: no_transform(),
        key_transform: no_transform(),
        fetch_unit: 0,
        fetch_multisampled: false,
        single_sampled: false,
        samples: 4,
        set: 0,
        ranges: [0, 1, 2, 3].map(|_| SlotRange { first: 0, slots: 64 }),
    }
}

/// A variant's result, bindings, key and flat form (points_in 0: no tessellation here).
struct Made {
    result: u32,
    bindings: Vec<Binding>,
    key: Vec<u8>,
    flat: Option<Flat>,
}

fn made<M: MesaCompiler>(compiler: &M, form: VariantForm<M::Variant>) -> Made {
    let flat = form.shader.and_then(|variant| compiler.variant_flatten(variant, 0));
    Made { result: form.result, bindings: form.bindings, key: form.key, flat }
}

/// The new (set, binding) of the resource of `kind` the program declared at `binding`, by the variant's bindings.
fn placed(made: &Made, set: u32, kind: u32, binding: u32) -> (u32, u32) {
    let found = made.bindings.iter().find(|b| b.kind == kind && b.binding == binding).unwrap_or_else(|| panic!("a binding of kind {kind} at {binding} in {:?}", made.bindings));
    (set, found.new_binding)
}

/// The constant buffer of a program's uniform block at GLSL binding 0: Gallium's constant buffer 0 holds the default
/// uniform block, so st/mesa binds GLSL's block b at b + 1 (the driver's numbering, which the bridge's steps keep).
const BLOCK_BUFFER: u32 = 1;

/// What the sampling program is given: its block (at, lod) and its two textures, placed where the variant's bindings
/// put them.
fn sampling_given(made: &Made, set: u32, at: [f32; 2], lod: f32) -> Given {
    let mut given = Given::default();
    given.buffers.insert(placed(made, set, constants::SLOT_UNIFORM, BLOCK_BUFFER), vec![at[0].to_bits(), at[1].to_bits(), lod.to_bits(), 0]);
    given.textures.insert(placed(made, set, constants::SLOT_TEXTURE, 0), Texture { sizes: LEVELS.to_vec(), integer: false, colour: COLOUR.map(f32::to_bits) });
    given.textures.insert(placed(made, set, constants::SLOT_TEXTURE, 1), Texture { sizes: LEVELS.to_vec(), integer: true, colour: SAMPLED_AS_WRITTEN });
    given
}

/// The location of the sampling program's generic output of `components` components (`swizzled` has four,
/// `counted` three): Mesa's linker places the outputs of a program pair itself, so the place is read from the program.
fn generic_output(flat: &Flat, components: u32) -> u32 {
    let found: Vec<u32> = flat
        .vars
        .iter()
        .filter(|var| var.mode == constants::VAR_OUTPUT && var.builtin == constants::BUILTIN_NONE)
        .filter(|var| flat.types.get(usize::try_from(var.type_).unwrap_or(usize::MAX)).is_some_and(|ty| ty.kind == constants::TYPE_VECTOR && ty.components == components))
        .map(|var| var.location)
        .collect();
    assert_eq!(found.len(), 1, "one generic output of {components} components: {found:?}");
    found[0]
}

/// The integer output's three words.
fn three(words: Option<Words>) -> Option<[u32; 3]> {
    words.map(|words| [words[0], words[1], words[2]])
}

/// A program's stage variables and stage loads, for a failing test's message.
fn stage_values(flat: &Flat) -> String {
    let vars: Vec<String> = flat.vars.iter().enumerate().filter(|(_, var)| var.mode == constants::VAR_INPUT || var.mode == constants::VAR_OUTPUT).map(|(at, var)| format!("{at}: mode {} builtin {} location {} type {:?}", var.mode, var.builtin, var.location, flat.types.get(usize::try_from(var.type_).unwrap_or(usize::MAX)).map(|ty| (ty.kind, ty.components)))).collect();
    let loads: Vec<String> = flat.instrs.iter().filter(|i| i.kind == constants::FLAT_INTRINSIC && i.name.as_deref().is_some_and(|name| name.starts_with(b"load_") && name.ends_with(b"input"))).map(|i| format!("{} var {} component {} components {}", String::from_utf8_lossy(i.name.as_deref().unwrap_or_default()), i.value[0], i.component, i.components)).collect();
    format!("vars {vars:?}; loads {loads:?}")
}

fn run(flat: &Flat, given: &Given) -> (Outputs, Vec<Outputs>) {
    reference::run(flat, given).unwrap_or_else(|refusal| panic!("the reference runs the variant: {refusal}; the flat form: {flat:#?}"))
}

/// The position after a transform, by bridge.h's meaning of each field, in the order the bridge applies them: y
/// negated; depth from -w..w to 0..w, (z + w) / 2; then depth reversed, w - z.
fn transformed(transform: &PositionTransform) -> [f32; 4] {
    let [x, mut y, mut z, w] = POSITION;
    if transform.flip_y {
        y = -y;
    }
    if transform.half_z {
        z = (z + w) / 2.0;
    }
    if transform.reverse_z {
        z = w - z;
    }
    [x, y, z, w]
}

/// Every one of the 16 transforms.
fn transforms() -> Vec<PositionTransform> {
    (0..16_u32).map(|bits| PositionTransform { flip_y: bits & 1 != 0, half_z: bits & 2 != 0, reverse_z: bits & 4 != 0, fixed_point_size: bits & 8 != 0 }).collect()
}

/// A stage's outputs after one transform: the position as `transformed` says, the point size written as the
/// program wrote it unless `fixed_point_size` removes it.
fn assert_transformed(method: &str, outputs: &Outputs, transform: &PositionTransform) {
    assert_transformed_sized(method, outputs, transform, POINT_SIZE);
}

/// `assert_transformed` for a program writing the point size `written`.
fn assert_transformed_sized(method: &str, outputs: &Outputs, transform: &PositionTransform, written: f32) {
    let position = outputs.floats(constants::BUILTIN_POSITION, 0).unwrap_or_else(|| panic!("{method}: a position written"));
    assert_eq!(position, transformed(transform), "{method}: the position under {transform:?}");
    let size = outputs.floats(constants::BUILTIN_POINT_SIZE, 0);
    if transform.fixed_point_size {
        assert_eq!(size, None, "{method}: the point size removed under {transform:?}");
    } else {
        assert_eq!(size.map(|size| size[0]), Some(written), "{method}: the point size kept under {transform:?}");
    }
}

/// The transform: every one of the 16 on the vertex program's variant, on its squares' copy (a `Made`) and on the
/// geometry and evaluation programs' variants, each position and point size the right one; and the key taken with
/// `key_transform` is the key of the program made with it (bridge.h: the key is of a copy with `key_transform`
/// applied after `transform`), while the program made has `transform` alone.
pub fn transforms_right<M: MesaCompiler>(compiler: M) {
    let compiler = &compiler;
    let program = compiled(compiler, [Some(SAMPLING_VERTEX_SOURCE), Some(PASSING_FRAGMENT_SOURCE), None, None, None, None]);
    let prepared = compiler.received_vertex_prepare(program.vertex.expect("a vertex stage")).shader.expect("prepared");
    let squares = compiler.prepared_vertex_points_as_squares(&prepared, 1, 64.0);
    let before = squares.before.expect("the program writes a point size: a copy drawn before the squares");
    let mut checked = 0;
    for transform in transforms() {
        let mut changed = settings();
        changed.transform = transform.clone();
        changed.key_transform = transform.clone();
        for (method, variant) in [("prepared_vertex_variant", made(compiler, compiler.prepared_vertex_variant(&prepared, changed.clone(), &[], &[]))), ("made_variant", made(compiler, compiler.made_variant(&before, changed.clone(), &[], &[])))] {
            assert_eq!(variant.result, constants::VARIANT_MADE, "{method}: made under {transform:?}");
            let flat = variant.flat.as_ref().expect("a flat form");
            let (outputs, _) = run(flat, &sampling_given(&variant, 0, [0.5, 0.5], 0.0));
            assert_transformed(method, &outputs, &transform);
            checked += 1;
        }
        // The key's transform alone: the program made is the plain one, its key that of the transformed program.
        let mut keyed = settings();
        keyed.key_transform = transform.clone();
        let alone = made(compiler, compiler.prepared_vertex_variant(&prepared, keyed.clone(), &[], &[]));
        let (outputs, _) = run(alone.flat.as_ref().expect("a flat form"), &sampling_given(&alone, 0, [0.5, 0.5], 0.0));
        assert_transformed("prepared_vertex_variant (key transform alone)", &outputs, &no_transform());
        let whole = made(compiler, compiler.prepared_vertex_variant(&prepared, changed.clone(), &[], &[]));
        assert!(alone.key == whole.key, "the key under key_transform {transform:?} is the key of the program made with it");
        let made_alone = made(compiler, compiler.made_variant(&before, keyed, &[], &[]));
        let (outputs, _) = run(made_alone.flat.as_ref().expect("a flat form"), &sampling_given(&made_alone, 0, [0.5, 0.5], 0.0));
        assert_transformed("made_variant (key transform alone)", &outputs, &no_transform());
        let made_whole = made(compiler, compiler.made_variant(&before, changed, &[], &[]));
        assert!(made_alone.key == made_whole.key, "made_variant: the key under key_transform {transform:?} is the key of the program made with it");
        checked += 2;
    }
    // A geometry program's variant: each vertex it emits transformed.
    let geometry_program = compiled(compiler, [Some(POINTS_VERTEX_SOURCE), Some(SHADED_FRAGMENT_SOURCE), None, Some(TWO_POINTS_GEOMETRY_SOURCE), None, None]);
    let geometry = compiler.received_geometry_prepare(geometry_program.geometry.expect("a geometry stage")).shader.expect("prepared");
    for transform in transforms() {
        let mut changed = settings();
        changed.transform = transform.clone();
        changed.key_transform = transform.clone();
        let variant = made(compiler, compiler.prepared_geometry_variant(&geometry, changed, &[], &[]));
        let input = [POSITION[0] - 1.0, POSITION[1] - 1.0, POSITION[2], POSITION[3]];
        let (_, emitted) = run(variant.flat.as_ref().expect("a flat form"), &points_given(input));
        assert_eq!(emitted.len(), 2, "prepared_geometry_variant: two points emitted");
        // The second copy is at the input moved by (1, 1): the sampling program's position; its size the input's w.
        assert_transformed_sized("prepared_geometry_variant", &emitted[1], &transform, POSITION[3]);
        checked += 1;
    }
    println!("TRANSFORMS {checked} checked");
}

/// A geometry program's inputs for one point the points vertex program drew from its input `at`: the position `at`,
/// the point size `at.w` and `at` passed on. Mesa's linker may read any of them from another (the vertex program writes
/// them all from `at`), so each is given in every place it may be read: the built-ins, a generic input of four
/// components and one of one component.
fn points_given(at: [f32; 4]) -> Given {
    let mut given = Given::default();
    given.inputs.insert((constants::BUILTIN_POSITION, 0), vec![at.map(f32::to_bits)]);
    given.inputs.insert((constants::BUILTIN_POINT_SIZE, 0), vec![[at[3].to_bits(), 0, 0, 0]]);
    given.generic.insert(4, vec![at.map(f32::to_bits)]);
    given.generic.insert(1, vec![[at[3].to_bits(), 0, 0, 0]]);
    given
}

/// What the integer texture answers at `at`, by OpenGL's nearest rules as the sampler gives them: the level of detail
/// clamped to [min_lod, max_lod], the level none above one half and else the nearest (ceil(lod + 0.5) - 1, at most
/// the last), the texel floor(at * size) wrapped on each axis by its wrap, the border where an axis that wraps to the
/// border is left; a texture of a unit no sampler names answers as the program samples it.
fn nearest(sampler: Option<&NearestSampler>, at: [f32; 2], lod: f32) -> Words {
    let Some(sampler) = sampler else {
        return SAMPLED_AS_WRITTEN;
    };
    let level = if sampler.mipmapped {
        let clamped = lod.max(sampler.min_lod).min(sampler.max_lod);
        let nearest = if clamped > 0.5 { (clamped + 0.5).ceil() as i32 - 1 } else { 0 };
        nearest.min(i32::try_from(LEVELS.len()).unwrap_or(1) - 1)
    } else {
        0
    };
    let level = u32::try_from(level.max(0)).unwrap_or(0);
    let size = LEVELS[usize::try_from(level).unwrap_or(0)];
    let mut texel = [0_u32; 2];
    for axis in 0..2 {
        let side = i32::try_from(size[axis]).unwrap_or(1);
        let i = (at[axis] * side as f32).floor() as i32;
        let wrapped = match u32::from(sampler.wrap[axis]) {
            constants::WRAP_REPEAT => i.rem_euclid(side),
            constants::WRAP_MIRRORED_REPEAT => {
                let m = i.rem_euclid(2 * side);
                if m < side { m } else { 2 * side - 1 - m }
            }
            constants::WRAP_CLAMP_TO_BORDER if i < 0 || i >= side => return sampler.border,
            _ => i.clamp(0, side - 1),
        };
        texel[axis] = u32::try_from(wrapped).unwrap_or(0);
    }
    Texture::texel(level, texel[0], texel[1])
}

/// The cases the nearest samplers are run on: (sampler, place, level of detail).
fn sampler_cases() -> Vec<(NearestSampler, [f32; 2], f32)> {
    let clamp = u8::try_from(constants::WRAP_CLAMP_TO_EDGE).unwrap_or(0);
    let repeat = u8::try_from(constants::WRAP_REPEAT).unwrap_or(0);
    let mirrored = u8::try_from(constants::WRAP_MIRRORED_REPEAT).unwrap_or(0);
    let border = u8::try_from(constants::WRAP_CLAMP_TO_BORDER).unwrap_or(0);
    let base = NearestSampler { unit: 1, wrap: [clamp; 3], mipmapped: true, min_lod: 1.0, max_lod: 2.0, border: [11, 22, 33, 44] };
    let inside = [0.3, 0.6];
    let outside = [1.3, -0.4];
    vec![
        // The level: clamped up to min_lod, down to max_lod, within; none without mipmaps.
        (base.clone(), inside, 0.0),
        (base.clone(), inside, 5.0),
        (NearestSampler { min_lod: 0.0, max_lod: 4.0, ..base.clone() }, inside, 2.6),
        (NearestSampler { min_lod: 0.0, max_lod: 4.0, ..base.clone() }, inside, 0.4),
        (NearestSampler { mipmapped: false, ..base.clone() }, inside, 5.0),
        // A level of detail below zero (level 0, none above one half) and past the last level (the last).
        (NearestSampler { min_lod: -1.0, max_lod: 4.0, ..base.clone() }, inside, -1.0),
        (NearestSampler { min_lod: 0.0, max_lod: 8.0, ..base.clone() }, inside, 6.0),
        // The wraps, one axis at a time and both, at level 1.
        (NearestSampler { wrap: [repeat, clamp, clamp], ..base.clone() }, outside, 1.0),
        (NearestSampler { wrap: [clamp, repeat, clamp], ..base.clone() }, outside, 1.0),
        (NearestSampler { wrap: [mirrored, mirrored, clamp], ..base.clone() }, outside, 1.0),
        (NearestSampler { wrap: [clamp, clamp, clamp], ..base.clone() }, outside, 1.0),
        // The border: each of its four words, where either axis leaves the texture, and not where neither does.
        (NearestSampler { wrap: [border, clamp, clamp], ..base.clone() }, [1.3, 0.6], 1.0),
        (NearestSampler { wrap: [clamp, border, clamp], ..base.clone() }, [0.3, -0.4], 1.0),
        (NearestSampler { wrap: [border, border, border], ..base.clone() }, inside, 1.0),
        // A sampler at another unit: the program's sample is left as written.
        (NearestSampler { unit: 6, ..base }, inside, 1.0),
    ]
}

/// The swizzles run: (swizzle, the swizzled colour by bridge.h's meaning: each component the one named, 0 or 1).
fn swizzle_cases() -> Vec<(TextureSwizzle, [f32; 4])> {
    let s = |value: u32| u8::try_from(value).unwrap_or(0);
    let [r, g, b, a] = COLOUR;
    vec![
        (TextureSwizzle { unit: 0, swizzle: [s(constants::SWIZZLE_Z), s(constants::SWIZZLE_Y), s(constants::SWIZZLE_X), s(constants::SWIZZLE_W)] }, [b, g, r, a]),
        (TextureSwizzle { unit: 0, swizzle: [s(constants::SWIZZLE_W), s(constants::SWIZZLE_ZERO), s(constants::SWIZZLE_ONE), s(constants::SWIZZLE_X)] }, [a, 0.0, 1.0, r]),
        (TextureSwizzle { unit: 0, swizzle: [s(constants::SWIZZLE_Y), s(constants::SWIZZLE_Y), s(constants::SWIZZLE_Y), s(constants::SWIZZLE_Z)] }, [g, g, g, b]),
        // A swizzle of another unit leaves this one as it is.
        (TextureSwizzle { unit: 7, swizzle: [s(constants::SWIZZLE_ZERO); 4] }, COLOUR),
    ]
}

/// The swizzles and the nearest samplers, on the vertex program's variant and on its squares' copy (made_variant):
/// each output the right one.
pub fn samplers_right<M: MesaCompiler>(compiler: M) {
    let compiler = &compiler;
    let program = compiled(compiler, [Some(SAMPLING_VERTEX_SOURCE), Some(PASSING_FRAGMENT_SOURCE), None, None, None, None]);
    let prepared = compiler.received_vertex_prepare(program.vertex.expect("a vertex stage")).shader.expect("prepared");
    let before = compiler.prepared_vertex_points_as_squares(&prepared, 1, 64.0).before.expect("the copy before the squares");
    let mut checked = 0;
    type Variant<'a, M> = Box<dyn Fn(&[TextureSwizzle], &[NearestSampler]) -> VariantForm<<M as MesaCompiler>::Variant> + 'a>;
    let methods: [(&str, Variant<'_, M>); 2] = [
        ("prepared_vertex_variant", Box::new(|swizzles, samplers| compiler.prepared_vertex_variant(&prepared, settings(), swizzles, samplers))),
        ("made_variant", Box::new(|swizzles, samplers| compiler.made_variant(&before, settings(), swizzles, samplers))),
    ];
    for (method, variant) in &methods {
        for (sampler, at, lod) in sampler_cases() {
            let made = made(compiler, variant(&[], std::slice::from_ref(&sampler)));
            assert_eq!(made.result, constants::VARIANT_MADE, "{method}: made with {sampler:?}");
            let (outputs, _) = run(made.flat.as_ref().expect("a flat form"), &sampling_given(&made, 0, at, lod));
            let listed = (sampler.unit == 1).then_some(&sampler);
            let counted = generic_output(made.flat.as_ref().expect("a flat form"), 3);
            assert_eq!(three(outputs.values.get(&(constants::BUILTIN_NONE, counted)).copied()), three(Some(nearest(listed, at, lod))), "{method}: the integer texture with {sampler:?} at {at:?}, lod {lod}");
            checked += 1;
        }
        for (swizzle, expected) in swizzle_cases() {
            let made = made(compiler, variant(std::slice::from_ref(&swizzle), &[]));
            let (outputs, _) = run(made.flat.as_ref().expect("a flat form"), &sampling_given(&made, 0, [0.5, 0.5], 0.0));
            let swizzled = generic_output(made.flat.as_ref().expect("a flat form"), 4);
            assert_eq!(outputs.floats(constants::BUILTIN_NONE, swizzled), Some(expected), "{method}: the float texture swizzled by {swizzle:?}");
            checked += 1;
        }
    }
    println!("SAMPLERS {checked} checked");
}

/// The common form's bindings by its rule (bridge.h, wc_nir_to_common_form): the program's resources ordered by set,
/// binding and kind (uniform, sampler, texture, storage), each kind numbered on from its range's first slot, all in
/// the settings' set; a kind past its range's slots refuses the variant. The sampling program has two constant buffers
/// (Gallium's 0, the default uniform block, and 1, its block) and two textures, each split into a texture and a
/// sampler (bindings 0 and 1).
pub fn bindings_right<M: MesaCompiler>(compiler: M) {
    let compiler = &compiler;
    let program = compiled(compiler, [Some(SAMPLING_VERTEX_SOURCE), Some(PASSING_FRAGMENT_SOURCE), None, None, None, None]);
    let prepared = compiler.received_vertex_prepare(program.vertex.expect("a vertex stage")).shader.expect("prepared");
    let before = compiler.prepared_vertex_points_as_squares(&prepared, 1, 64.0).before.expect("the copy before the squares");
    // What the program needs of each kind: two constant buffers, two samplers, two textures, no storage.
    let needed = [2_u32, 2, 2, 0];
    let firsts = [3_u32, 5, 7, 11];
    let set = 4;
    let expected = |firsts: [u32; 4]| -> Vec<(u32, u32, u32, u32, u32, u32)> {
        vec![
            (constants::SLOT_UNIFORM, 0, 0, 1, firsts[0], 0),
            (constants::SLOT_SAMPLER, 0, 0, 1, firsts[1], 0),
            (constants::SLOT_TEXTURE, 0, 0, 1, firsts[2], 0),
            (constants::SLOT_UNIFORM, 0, 1, 1, firsts[0] + 1, 0),
            (constants::SLOT_SAMPLER, 0, 1, 1, firsts[1] + 1, 0),
            (constants::SLOT_TEXTURE, 0, 1, 1, firsts[2] + 1, 0),
        ]
    };
    let rows = |made: &Made| -> Vec<(u32, u32, u32, u32, u32, u32)> { made.bindings.iter().map(|b| (b.kind, b.set, b.binding, b.count, b.new_binding, b.stride)).collect() };
    let mut checked = 0;
    type Variant<'a, M> = Box<dyn Fn(VariantSettings) -> VariantForm<<M as MesaCompiler>::Variant> + 'a>;
    let methods: [(&str, Variant<'_, M>); 2] = [
        ("prepared_vertex_variant", Box::new(|settings| compiler.prepared_vertex_variant(&prepared, settings, &[], &[]))),
        ("made_variant", Box::new(|settings| compiler.made_variant(&before, settings, &[], &[]))),
    ];
    for (method, variant) in &methods {
        // Exactly the slots needed, from the firsts above, in set 4: made, numbered by the rule, and the program
        // reads its block and textures at those places.
        let mut exact = settings();
        exact.set = set;
        exact.ranges = std::array::from_fn(|kind| SlotRange { first: firsts[kind], slots: needed[kind] });
        let made_exact = made(compiler, variant(exact.clone()));
        assert_eq!(made_exact.result, constants::VARIANT_MADE, "{method}: made with exactly the slots it needs (with room for all: {:?})", rows(&made(compiler, variant(settings()))));
        assert_eq!(rows(&made_exact), expected(firsts), "{method}: the bindings by the common form's rule");
        let (outputs, _) = run(made_exact.flat.as_ref().expect("a flat form"), &sampling_given(&made_exact, set, [0.5, 0.5], 0.0));
        let swizzled = generic_output(made_exact.flat.as_ref().expect("a flat form"), 4);
        assert_eq!(outputs.floats(constants::BUILTIN_NONE, swizzled), Some(COLOUR), "{method}: the float texture read at its new place");
        checked += 1;
        // One slot fewer of each kind it uses: refused.
        for kind in 0..3 {
            let mut short = exact.clone();
            short.ranges[kind].slots -= 1;
            let refused = made(compiler, variant(short));
            assert_eq!(refused.result, constants::VARIANT_BEYOND_LIMIT, "{method}: one slot too few of kind {kind}");
            assert!(refused.flat.is_none(), "{method}: no program when refused");
            checked += 1;
        }
    }
    println!("BINDINGS {checked} checked");
}

/// A point's square by bridge.h's documented corners: a strip of four corners (-1, -1), (1, -1), (-1, 1), (1, 1), each
/// the position moved by size * step * w in x and y (step from the squares' block: words 1 and 2), the point
/// coordinate (0.5 + 0.5 * sx, 0.5 + 0.5 * sy * t) with t the block's word 3; the size the program's, clamped to
/// [1, largest].
fn corners(position: [f32; 4], size: f32, largest: f32, block: [f32; 4]) -> Vec<([f32; 4], [f32; 2])> {
    let size = size.clamp(1.0, largest);
    let step_x = size * block[1] * position[3];
    let step_y = size * block[2] * position[3];
    [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
        .into_iter()
        .map(|(sx, sy): (f32, f32)| ([position[0] + step_x * sx, position[1] + step_y * sy, position[2], position[3]], [0.5 + 0.5 * sx, block[3] * (0.5 * sy) + 0.5]))
        .collect()
}

/// The squares' block the suite gives: a size, the steps in x and y, and t's direction.
const BLOCK: [f32; 4] = [2.0, 0.125, -0.25, -1.0];

fn assert_corners(method: &str, emitted: &[Outputs], expected: &[([f32; 4], [f32; 2])], at: u32) {
    assert_eq!(emitted.len(), expected.len(), "{method}: the corners emitted");
    for (vertex, (position, coord)) in emitted.iter().zip(expected) {
        assert_eq!(vertex.floats(constants::BUILTIN_POSITION, 0), Some(*position), "{method}: a corner's position");
        let written = vertex.floats(constants::BUILTIN_NONE, at).unwrap_or_else(|| panic!("{method}: the point coordinate at location {at}"));
        assert_eq!([written[0], written[1]], *coord, "{method}: a corner's point coordinate at location {at}");
    }
}

/// Points as squares: the vertex program's squares and the geometry program's, each corner the right one for the
/// slot (`at`), the largest size and the block the size and steps are read from.
pub fn squares_right<M: MesaCompiler>(compiler: M) {
    let compiler = &compiler;
    let mut checked = 0;
    let block_words = BLOCK.map(f32::to_bits).to_vec();
    // The vertex program's squares: the copy before them writes its point size also at `at`, which the squares read.
    let program = compiled(compiler, [Some(POINTS_VERTEX_SOURCE), Some(SHADED_FRAGMENT_SOURCE), None, None, None, None]);
    let prepared = compiler.received_vertex_prepare(program.vertex.expect("a vertex stage")).shader.expect("prepared");
    for (at, largest, size) in [(1_u32, 8.0_f32, 6.0_f32), (1, 8.0, 10.0), (2, 64.0, 0.5), (3, 4.0, 3.0)] {
        let squares = compiler.prepared_vertex_points_as_squares(&prepared, at, largest);
        assert_eq!(squares.kind, constants::SQUARES_MADE, "the vertex program's squares made");
        let geometry = squares.geometry.expect("the squares' geometry program");
        let variant = made(compiler, compiler.made_variant(&geometry, settings(), &[], &[]));
        let position = [0.25, 0.5, 0.75, size];
        let mut given = points_given(position);
        // The point size the copy before writes at `at`, and the squares' block: constant buffer 0 (bridge.h).
        given.inputs.insert((constants::BUILTIN_NONE, at), vec![[size.to_bits(), 0, 0, 0]]);
        given.buffers.insert(placed(&variant, 0, constants::SLOT_UNIFORM, 0), block_words.clone());
        let (_, emitted) = run(variant.flat.as_ref().expect("a flat form"), &given);
        assert_corners("prepared_vertex_points_as_squares", &emitted, &corners(position, size, largest, BLOCK), at);
        checked += 1;
    }
    // The geometry program's squares: each point it emits a square, the block at `block`.
    let geometry_program = compiled(compiler, [Some(POINTS_VERTEX_SOURCE), Some(SHADED_FRAGMENT_SOURCE), None, Some(TWO_POINTS_GEOMETRY_SOURCE), None, None]);
    let geometry = compiler.received_geometry_prepare(geometry_program.geometry.expect("a geometry stage")).shader.expect("prepared");
    // The block a free constant buffer (Gallium's 0 is the program's default uniform block, which every program has).
    for (at, largest, block, size) in [(1_u32, 8.0_f32, 2_u32, 6.0_f32), (1, 8.0, 3, 10.0), (2, 64.0, 5, 0.5), (3, 4.0, 1, 3.0)] {
        let squares = compiler.prepared_geometry_points_as_squares(&geometry, at, largest, block);
        assert_eq!(squares.kind, constants::SQUARES_MADE, "the geometry program's squares made");
        let made_geometry = squares.geometry.expect("the squares' geometry program");
        let variant = made(compiler, compiler.made_variant(&made_geometry, settings(), &[], &[]));
        assert_eq!(variant.bindings.iter().filter(|b| b.kind == constants::SLOT_UNIFORM).map(|b| b.binding).collect::<Vec<_>>(), vec![0, block], "the default uniform block, and the squares' block at the binding asked for");
        // The point's size is its w (the points vertex program's).
        let position = [0.25, 0.5, 0.75, size];
        let mut given = points_given(position);
        given.buffers.insert(placed(&variant, 0, constants::SLOT_UNIFORM, block), block_words.clone());
        let (_, emitted) = run(variant.flat.as_ref().expect("a flat form"), &given);
        let mut expected = corners(position, size, largest, BLOCK);
        expected.extend(corners([position[0] + 1.0, position[1] + 1.0, position[2], position[3]], size, largest, BLOCK));
        let described = stage_values(variant.flat.as_ref().expect("a flat form"));
        assert_corners(&format!("prepared_geometry_points_as_squares ({described})"), &emitted, &expected, at);
        checked += 1;
    }
    println!("SQUARES {checked} checked");
}

/// A tessellation control program of three control points, before each evaluation program of `declarations_right`.
const TRIANGLE_CONTROL_SOURCE: &str = "#version 450
layout(vertices = 3) out;
layout(location = 0) in vec4 passed[];
layout(location = 0) out vec4 handed[];
void main() {
    handed[gl_InvocationID] = passed[gl_InvocationID];
    gl_out[gl_InvocationID].gl_Position = gl_in[gl_InvocationID].gl_Position;
    gl_TessLevelOuter[0] = 2.0;
    gl_TessLevelOuter[1] = 2.0;
    gl_TessLevelOuter[2] = 2.0;
    gl_TessLevelOuter[3] = 2.0;
    gl_TessLevelInner[0] = 2.0;
    gl_TessLevelInner[1] = 2.0;
}
";

/// The control points of `TRIANGLE_CONTROL_SOURCE`'s patch.
const TRIANGLE_CONTROL_POINTS: u32 = 3;
/// The draw's patch size the control program is flattened for (not its output's, so the two are told apart).
const DRAWN_PATCH: u32 = 4;

/// A geometry program declaring its input and output primitives, its invocations and its largest output.
const DECLARING_GEOMETRY_SOURCE: &str = "#version 450
layout(INPUT, invocations = INVOCATIONS) in;
layout(OUTPUT, max_vertices = MOST) out;
layout(location = 0) in vec4 passed[];
layout(location = 0) out vec4 shaded;
void main() {
    gl_Position = gl_in[0].gl_Position;
    shaded = passed[0];
    EmitVertex();
}
";

/// spirv/streams.geom as SPIR-V for Vulkan (glslang 16.4.0, `glslangValidator -V --target-env vulkan1.1`): points in,
/// three invocations, points out (at most 2), streams 0 and 1 written. The bridge's GLSL compiler allows one vertex
/// stream (WC_GL_VERTEX_STREAMS), so a second comes only through SPIR-V.
const STREAMS_SPIRV: &[u8] = include_bytes!("spirv/streams.spv");

/// An evaluation program declaring its domain, spacing, winding and point mode (`LAYOUT`).
const DECLARING_EVALUATION_SOURCE: &str = "#version 450
layout(LAYOUT) in;
layout(location = 0) in vec4 handed[];
layout(location = 0) out vec4 shaded;
void main() {
    gl_Position = gl_TessCoord.x * gl_in[0].gl_Position + gl_TessCoord.y * gl_in[1].gl_Position + gl_TessCoord.z * gl_in[2].gl_Position;
    shaded = handed[0];
}
";

/// Every declaration of a geometry and of an evaluation program the flat form carries (bridge/flat.c's geometry_of and
/// tessellation_of), each from a program declaring it: each input and output primitive, the input's vertices, the
/// largest output, the invocations (3 where declared, else 1), the streams (2 where stream 1 is written, through
/// SPIR-V, else 1); each domain, each spacing, both windings and the point mode, with the patch's control points in and
/// out.
pub fn declarations_right<M: MesaCompiler>(compiler: M) {
    let compiler = &compiler;
    let geometries = [
        // (input, its vertices, output, largest, invocations, the flat form's input and output)
        ("points", 1, "points", 1, 1, constants::GEOMETRY_IN_POINTS, constants::GEOMETRY_OUT_POINTS),
        ("lines", 2, "line_strip", 2, 3, constants::GEOMETRY_IN_LINES, constants::GEOMETRY_OUT_LINE_STRIP),
        ("lines_adjacency", 4, "triangle_strip", 3, 1, constants::GEOMETRY_IN_LINES_ADJACENCY, constants::GEOMETRY_OUT_TRIANGLE_STRIP),
        ("triangles", 3, "points", 4, 1, constants::GEOMETRY_IN_TRIANGLES, constants::GEOMETRY_OUT_POINTS),
        ("triangles_adjacency", 6, "line_strip", 5, 2, constants::GEOMETRY_IN_TRIANGLES_ADJACENCY, constants::GEOMETRY_OUT_LINE_STRIP),
    ];
    let mut checked = 0;
    for (input, vertices_in, output, most, invocations, flat_input, flat_output) in geometries {
        let source = DECLARING_GEOMETRY_SOURCE.replace("INPUT", input).replace("INVOCATIONS", &invocations.to_string()).replace("OUTPUT", output).replace("MOST", &most.to_string());
        let program = compiled(compiler, [Some(POINTS_VERTEX_SOURCE), Some(SHADED_FRAGMENT_SOURCE), None, Some(&source), None, None]);
        let prepared = compiler.received_geometry_prepare(program.geometry.expect("a geometry stage")).shader.expect("prepared");
        let variant = made(compiler, compiler.prepared_geometry_variant(&prepared, settings(), &[], &[]));
        let flat = variant.flat.unwrap_or_else(|| panic!("{input} to {output}: a flat form (result {})", variant.result));
        let g = &flat.geometry;
        assert_eq!((g.input, g.vertices_in, g.output, g.max_vertices, g.invocations, g.streams), (flat_input, vertices_in, flat_output, most, invocations, 1), "{input} to {output}");
        checked += 1;
    }
    let words: Vec<u32> = STREAMS_SPIRV.as_chunks::<4>().0.iter().map(|word| u32::from_le_bytes(*word)).collect();
    let read = compiler.spirv_read(&words, constants::STAGE_GEOMETRY, c"main", false).expect("spirv/streams.spv read");
    let common = compiler.spirv_to_common_form(read, 0, &settings().ranges);
    let result = common.result;
    let flat = common.shader.and_then(|shader| compiler.spirv_common_flatten(shader)).unwrap_or_else(|| panic!("spirv/streams.spv: a flat form (result {result})"));
    let g = &flat.geometry;
    assert_eq!((g.input, g.vertices_in, g.output, g.max_vertices, g.invocations, g.streams), (constants::GEOMETRY_IN_POINTS, 1, constants::GEOMETRY_OUT_POINTS, 2, 3, 2), "spirv/streams.spv");
    checked += 1;
    let evaluations = [
        // (layout, domain, spacing, counter-clockwise, point mode)
        ("triangles, equal_spacing, ccw", constants::TESS_TRIANGLES, constants::TESS_EQUAL, 1, 0),
        ("quads, fractional_odd_spacing, cw", constants::TESS_QUADS, constants::TESS_FRACTIONAL_ODD, 0, 0),
        ("isolines, fractional_even_spacing, ccw, point_mode", constants::TESS_ISOLINES, constants::TESS_FRACTIONAL_EVEN, 1, 1),
    ];
    for (layout, domain, spacing, counter_clockwise, point_mode) in evaluations {
        let source = DECLARING_EVALUATION_SOURCE.replace("LAYOUT", layout);
        let program = compiled(compiler, [Some(POINTS_VERTEX_SOURCE), Some(SHADED_FRAGMENT_SOURCE), None, None, Some(TRIANGLE_CONTROL_SOURCE), Some(&source)]);
        let prepared = compiler.received_tess_evaluation_prepare(program.tess_evaluation.expect("an evaluation stage")).shader.expect("prepared");
        let form = compiler.prepared_tess_evaluation_variant(&prepared, settings(), &[], &[]);
        let result = form.result;
        let flat = form.shader.and_then(|variant| compiler.variant_flatten(variant, TRIANGLE_CONTROL_POINTS)).unwrap_or_else(|| panic!("{layout}: a flat form (result {result})"));
        let t = &flat.tessellation;
        // An evaluation program's output control points are its control program's (0 here).
        assert_eq!((t.points_in, t.points_out, t.domain, t.spacing, t.counter_clockwise, t.point_mode), (TRIANGLE_CONTROL_POINTS, 0, domain, spacing, counter_clockwise, point_mode), "{layout}");
        checked += 1;
    }
    // The control program: the draw's patch size in, its declared output control points out.
    let source = DECLARING_EVALUATION_SOURCE.replace("LAYOUT", "triangles");
    let program = compiled(compiler, [Some(POINTS_VERTEX_SOURCE), Some(SHADED_FRAGMENT_SOURCE), None, None, Some(TRIANGLE_CONTROL_SOURCE), Some(&source)]);
    let prepared = compiler.received_tess_control_prepare(program.tess_control.expect("a control stage")).shader.expect("prepared");
    let form = compiler.prepared_tess_control_variant(&prepared, settings(), &[], &[]);
    let result = form.result;
    let flat = form.shader.and_then(|variant| compiler.variant_flatten(variant, DRAWN_PATCH)).unwrap_or_else(|| panic!("the control program: a flat form (result {result})"));
    assert_eq!((flat.tessellation.points_in, flat.tessellation.points_out), (DRAWN_PATCH, TRIANGLE_CONTROL_POINTS), "the control program");
    checked += 1;
    println!("DECLARATIONS {checked} checked");
}

/// A compute program of a workgroup whose three sizes differ.
const SIZED_COMPUTE_SOURCE: &str = "#version 450
layout(local_size_x = 2, local_size_y = 3, local_size_z = 5) in;
layout(std430, binding = 0) buffer Results { uint values[]; };
void main() {
    values[gl_LocalInvocationIndex] = gl_LocalInvocationIndex;
}
";
/// Its workgroup's sizes, x, y and z.
const SIZED_WORKGROUP: [u32; 3] = [2, 3, 5];

/// `SHADED_FRAGMENT_SOURCE` with its fragment tests asked for before it runs.
const EARLY_FRAGMENT_SOURCE: &str = "#version 450
layout(early_fragment_tests) in;
layout(location = 0) in vec4 shaded;
layout(location = 0) out vec4 colour;
void main() {
    colour = shaded;
}
";

/// A control program reading control point `READ_POINT` of its input patch at a constant index.
const POINT_READING_CONTROL_SOURCE: &str = "#version 450
layout(vertices = 3) out;
layout(location = 0) out vec4 handed[];
void main() {
    gl_out[gl_InvocationID].gl_Position = gl_in[2].gl_Position;
    handed[gl_InvocationID] = vec4(1.0);
    gl_TessLevelOuter[0] = 2.0;
    gl_TessLevelOuter[1] = 2.0;
    gl_TessLevelOuter[2] = 2.0;
    gl_TessLevelOuter[3] = 2.0;
    gl_TessLevelInner[0] = 2.0;
    gl_TessLevelInner[1] = 2.0;
}
";
/// The control point `POINT_READING_CONTROL_SOURCE` reads.
const READ_POINT: u32 = 2;

/// The flat form's facts of a compute and a fragment program, and the read of a missing control point: a compute
/// program's workgroup sizes, each from its own dimension (bridge/flat.c's `workgroup_size`); a fragment program's
/// early fragment tests, 1 where declared and 0 where not; a control program's read of control point 2 kept when the
/// patch has 3 or 4 points, and made a zero (no stage read left) when it has 2 (bridge/flat.c's
/// `missing_point_reads_zero`: only a point at or past the patch's own is missing).
pub fn program_facts_right<M: MesaCompiler>(compiler: M) {
    let compiler = &compiler;
    let program = compiled(compiler, [None, None, Some(SIZED_COMPUTE_SOURCE), None, None, None]);
    let prepared = compiler.received_compute_prepare(program.compute.expect("a compute stage")).shader.expect("prepared");
    let variant = made(compiler, compiler.prepared_compute_variant(&prepared, settings(), &[], &[]));
    let flat = variant.flat.unwrap_or_else(|| panic!("the compute program: a flat form (result {})", variant.result));
    assert_eq!((flat.workgroup_size, flat.early_fragment_tests), (SIZED_WORKGROUP, 0), "the compute program");
    let mut checked = 1;
    for (source, early) in [(EARLY_FRAGMENT_SOURCE, 1), (SHADED_FRAGMENT_SOURCE, 0)] {
        let program = compiled(compiler, [Some(POINTS_VERTEX_SOURCE), Some(source), None, None, None, None]);
        let prepared = compiler.received_fragment_prepare(program.fragment.expect("a fragment stage")).shader.expect("prepared");
        let variant = made(compiler, compiler.prepared_fragment_variant(&prepared, settings(), &[], &[]));
        let flat = variant.flat.unwrap_or_else(|| panic!("the fragment program: a flat form (result {})", variant.result));
        assert_eq!(flat.early_fragment_tests, early, "the fragment program declaring early tests {early}");
        checked += 1;
    }
    let evaluation = DECLARING_EVALUATION_SOURCE.replace("LAYOUT", "triangles");
    let program = compiled(compiler, [Some(POINTS_VERTEX_SOURCE), Some(SHADED_FRAGMENT_SOURCE), None, None, Some(POINT_READING_CONTROL_SOURCE), Some(&evaluation)]);
    let prepared = compiler.received_tess_control_prepare(program.tess_control.expect("a control stage")).shader.expect("prepared");
    let stage_reads = |points_in: u32| {
        let form = compiler.prepared_tess_control_variant(&prepared, settings(), &[], &[]);
        let result = form.result;
        let flat = form.shader.and_then(|variant| compiler.variant_flatten(variant, points_in)).unwrap_or_else(|| panic!("the control program over {points_in} points: a flat form (result {result})"));
        // A stage read of a control point: an intrinsic reaching an arrayed input variable (bridge/flat.c's io_access).
        let inputs = |instr: &&wcontainers_mesa_compiler::FlatInstr| {
            usize::try_from(instr.value[0]).ok().and_then(|at| flat.vars.get(at)).is_some_and(|var| var.mode == constants::VAR_INPUT)
        };
        flat.instrs.iter().filter(|instr| instr.kind == constants::FLAT_INTRINSIC && instr.arrayed == 1).filter(inputs).count()
    };
    // The program's one read of a control point: kept over 3 and 4 points, a zero over 2.
    let reads = [READ_POINT + 1, READ_POINT + 2, READ_POINT].map(stage_reads);
    assert_eq!(reads, [1, 1, 0], "the control program's reads of control point {READ_POINT} over 3, 4 and 2 points");
    checked += 1;
    println!("PROGRAM FACTS {checked} checked");
}
