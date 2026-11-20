//! `rewrite` on programs another route made (any machine): arrays of textures, storage textures and
//! samplers become a binding per element, gathers with run-time offsets become gathers at a moved
//! coordinate, storage textures take the named view's format, and each result validates and writes
//! as WGSL that reads back.

use naga::{Expression, Module, ShaderStage as NagaStage, TypeInner};
use wcontainers_gpu::{
    Format, HELPER_SET, ReflectedBinding, Reflection, SlotKind, SourceStage, StageExtras,
    VariantKey, helper_binding,
};
use wcontainers_shaders::{ConvertError, ENTRY_POINT, cache, refusal, rewrite};

const ROUTE_KEY: [u8; cache::KEY_BYTES] = [0; cache::KEY_BYTES];

/// A compute program's reflection, its bindings left to the module.
fn compute_reflection(module: &Module) -> Reflection {
    let workgroup_size = module.entry_points[0].workgroup_size;
    Reflection {
        stage: SourceStage::Compute,
        inputs: Vec::new(),
        outputs: Vec::new(),
        bindings: Vec::new(),
        helpers: Vec::new(),
        helper_size: 0,
        sampled_pairs: Vec::new(),
        extras: StageExtras::Compute { workgroup_size },
        slow_paths: Vec::new(),
    }
}

/// `source` with its slots' first bindings substituted, parsed.
fn parse(source: &str) -> Module {
    let source = source
        .replace("UNIFORM", &SlotKind::Uniform.first_binding().to_string())
        .replace("SAMPLER", &SlotKind::Sampler.first_binding().to_string())
        .replace("TEXTURE", &SlotKind::Texture.first_binding().to_string())
        .replace("STORAGE", &SlotKind::Storage.first_binding().to_string())
        .replace("ENTRY", ENTRY_POINT);
    naga::front::wgsl::parse_str(&source).expect("the test's WGSL parses")
}

/// The module written as WGSL, which must parse again.
fn written(module: &Module, info: &naga::valid::ModuleInfo) -> String {
    let wgsl = naga::back::wgsl::write_string(module, info, naga::back::wgsl::WriterFlags::empty())
        .expect("the WGSL writer takes the module");
    naga::front::wgsl::parse_str(&wgsl).expect("the written WGSL parses");
    wgsl
}

fn bindings(module: &Module) -> Vec<u32> {
    let mut b: Vec<u32> = module
        .global_variables
        .iter()
        .filter_map(|(_, g)| g.binding.map(|b| b.binding))
        .collect();
    b.sort();
    b
}

const ARRAYS: &str = "
enable wgpu_binding_array;
struct U { i: u32, j: u32 }
@group(0) @binding(UNIFORM) var<uniform> u: U;
@group(0) @binding(SAMPLER) var s: binding_array<sampler, 4>;
@group(0) @binding(TEXTURE) var t: binding_array<texture_2d<f32>, 4>;
@group(0) @binding(STORAGE) var<storage, read_write> out: array<vec4<f32>>;
@compute @workgroup_size(1) fn ENTRY() {
    let i = u.i;
    out[0] = textureSampleLevel(t[i], s[i], vec2(0.5), 0.0);
    out[1] = textureLoad(t[2], vec2(0), 0);
    out[2] = textureSampleLevel(t[1], s[i], vec2(0.25), 0.0);
}
";

#[test]
fn arrays_of_textures_and_samplers_split_into_a_binding_per_element() {
    let module = parse(ARRAYS);
    let reflection = compute_reflection(&module);
    let converted =
        rewrite(module, reflection, &VariantKey::default(), &ROUTE_KEY).expect("the arrays split");
    let m = &converted.modules[0];
    let arrays = m
        .module
        .global_variables
        .iter()
        .any(|(_, g)| matches!(m.module.types[g.ty].inner, TypeInner::BindingArray { .. }));
    assert!(!arrays, "no binding array is left");
    let sampler = SlotKind::Sampler.first_binding();
    let texture = SlotKind::Texture.first_binding();
    let mut expected = vec![SlotKind::Uniform.first_binding()];
    expected.extend(sampler..sampler + 4);
    expected.extend(texture..texture + 4);
    expected.push(SlotKind::Storage.first_binding());
    assert_eq!(bindings(&m.module), expected);
    // The pick of both at one index pairs element k with element k; the constant texture 1 is
    // sampled with each sampler.
    let pairs = &converted.reflection.sampled_pairs;
    for k in 0..4 {
        assert!(pairs.contains(&(k, k)), "pair {k} in {pairs:?}");
        assert!(
            pairs.contains(&(1, k)),
            "texture 1 with sampler {k} in {pairs:?}"
        );
    }
    written(&m.module, &m.info);
}

#[test]
fn a_sample_picking_its_texture_and_sampler_at_two_indices_is_refused_by_name() {
    let source = ARRAYS.replace(
        "textureSampleLevel(t[i], s[i]",
        "textureSampleLevel(t[i], s[u.j]",
    );
    let module = parse(&source);
    let reflection = compute_reflection(&module);
    let refused = rewrite(module, reflection, &VariantKey::default(), &ROUTE_KEY)
        .expect_err("two indices are refused");
    assert_eq!(
        refused,
        ConvertError::Unsupported(refusal::RESOURCE_ARRAY_PAIR)
    );
}

#[test]
fn an_array_of_buffers_is_refused_by_name() {
    let source = "
enable wgpu_binding_array;
@group(0) @binding(STORAGE) var<storage, read_write> out: binding_array<array<u32>, 2>;
@compute @workgroup_size(1) fn ENTRY() { out[1][0] = 1u; }
";
    let module = parse(source);
    let reflection = compute_reflection(&module);
    let refused = rewrite(module, reflection, &VariantKey::default(), &ROUTE_KEY)
        .expect_err("buffer arrays are refused");
    assert_eq!(refused, ConvertError::Unsupported(refusal::RESOURCE_ARRAY));
}

#[test]
fn a_vertex_program_writing_storage_is_refused_by_name_as_convert_refuses_it() {
    let source = "
@group(0) @binding(STORAGE) var<storage, read_write> out: array<u32>;
@vertex fn ENTRY(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    out[v] = v;
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}
";
    let module = parse(source);
    let reflection = Reflection {
        stage: SourceStage::Vertex,
        extras: StageExtras::None,
        ..compute_reflection(&module)
    };
    let refused = rewrite(module, reflection, &VariantKey::default(), &ROUTE_KEY)
        .expect_err("no back end runs it");
    assert_eq!(
        refused,
        ConvertError::Unsupported(refusal::VERTEX_STORAGE_WRITE)
    );
}

/// A gather whose offset the test then makes the run-time value `u.o`.
const GATHER: &str = "
struct U { o: vec2<i32> }
@group(0) @binding(UNIFORM) var<uniform> u: U;
@group(0) @binding(SAMPLER) var s: sampler;
@group(0) @binding(TEXTURE) var t: texture_2d<f32>;
@group(0) @binding(STORAGE) var<storage, read_write> out: array<vec4<f32>>;
@compute @workgroup_size(1) fn ENTRY() {
    let o = u.o;
    out[1] = vec4<f32>(vec2<f32>(o), 0.0, 0.0);
    out[0] = textureGather(1, t, s, vec2(0.5), vec2(1, -2));
}
";

/// Makes every gather's offset the run-time load of `u.o`.
fn offsets_at_run_time(module: &mut Module) {
    let f = &mut module.entry_points[0].function;
    let load = f
        .expressions
        .iter()
        .find(|(_, e)| matches!(e, Expression::Load { .. }))
        .map(|(h, _)| h)
        .expect("the offset's load");
    for (_, e) in f.expressions.iter_mut() {
        if let Expression::ImageSample {
            gather: Some(_),
            offset,
            ..
        } = e
        {
            *offset = Some(load);
        }
    }
}

#[test]
fn gathers_with_run_time_offsets_gather_at_a_moved_coordinate() {
    let mut module = parse(GATHER);
    offsets_at_run_time(&mut module);
    let reflection = compute_reflection(&module);
    let converted = rewrite(module, reflection, &VariantKey::default(), &ROUTE_KEY)
        .expect("the gather is moved");
    let m = &converted.modules[0];
    let functions = m.module.functions.iter().map(|(_, f)| f);
    let mut gathers = 0;
    for f in functions.chain(m.module.entry_points.iter().map(|ep| &ep.function)) {
        for (_, e) in f.expressions.iter() {
            if let Expression::ImageSample {
                gather: Some(_),
                offset,
                ..
            } = e
            {
                assert_eq!(*offset, None, "the offset is in the coordinate");
                gathers += 1;
            }
        }
    }
    assert_eq!(gathers, 1);
    assert_eq!(m.module.entry_points[0].stage, NagaStage::Compute);
    written(&m.module, &m.info);
}

const STORAGE_TEXTURE: &str = "
@group(0) @binding(STORAGE) var w: texture_storage_2d<r32float, write>;
@compute @workgroup_size(1) fn ENTRY() { textureStore(w, vec2(0), vec4(1.0)); }
";

fn storage_format(module: &Module) -> naga::StorageFormat {
    module
        .global_variables
        .iter()
        .find_map(|(_, g)| match m_inner(module, g.ty) {
            TypeInner::Image {
                class: naga::ImageClass::Storage { format, .. },
                ..
            } => Some(*format),
            _ => None,
        })
        .expect("a storage texture")
}

fn m_inner(module: &Module, ty: naga::Handle<naga::Type>) -> &TypeInner {
    &module.types[ty].inner
}

#[test]
fn storage_textures_take_the_format_the_key_names_and_keep_their_own_otherwise() {
    let module = parse(STORAGE_TEXTURE);
    let reflection = compute_reflection(&module);
    let named = VariantKey {
        storage_formats: vec![(0, Format::Rgba8Unorm)],
        ..VariantKey::default()
    };
    let converted = rewrite(module.clone(), reflection.clone(), &named, &ROUTE_KEY)
        .expect("the format is given");
    assert_eq!(
        storage_format(&converted.modules[0].module),
        naga::StorageFormat::Rgba8Unorm
    );
    let kept = rewrite(module, reflection, &VariantKey::default(), &ROUTE_KEY)
        .expect("the module's format is kept");
    assert_eq!(
        storage_format(&kept.modules[0].module),
        naga::StorageFormat::R32Float
    );
}

#[test]
fn the_key_holds_the_routes_key_and_the_variant() {
    let module = parse(STORAGE_TEXTURE);
    let reflection = compute_reflection(&module);
    let plain = VariantKey::default();
    let named = VariantKey {
        storage_formats: vec![(0, Format::Rgba8Unorm)],
        ..VariantKey::default()
    };
    let other_route = [1; cache::KEY_BYTES];
    let key = |variant: &VariantKey, route: &[u8; cache::KEY_BYTES]| {
        rewrite(module.clone(), reflection.clone(), variant, route)
            .expect("rewritten")
            .modules[0]
            .key
    };
    assert_eq!(
        key(&plain, &ROUTE_KEY),
        cache::rewritten_key(&ROUTE_KEY, &plain)
    );
    assert_ne!(key(&plain, &ROUTE_KEY), key(&named, &ROUTE_KEY));
    assert_ne!(key(&plain, &ROUTE_KEY), key(&plain, &other_route));
    assert_ne!(key(&plain, &ROUTE_KEY), ROUTE_KEY);
}

#[test]
fn the_variant_rewrites_apply_as_on_the_direct3d_routes() {
    // A helper value the variant adds lands in the helper block, as for a converted program.
    let source = "
@group(1) @binding(TEXTURE) var t: texture_2d<f32>;
@fragment fn ENTRY(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(t, vec2<i32>(p.xy), 0);
}
";
    let module = parse(source);
    let texture = ReflectedBinding {
        kind: SlotKind::Texture,
        space: 0,
        register: 0,
        count: 1,
        set: 1,
        binding: SlotKind::Texture.first_binding(),
        added: None,
        stride: 0,
    };
    let reflection = Reflection {
        stage: SourceStage::Pixel,
        bindings: vec![texture],
        extras: StageExtras::Pixel {
            early_tests: false,
            side_effects: false,
            discards: false,
            per_sample: false,
        },
        ..compute_reflection(&module)
    };
    let variant = VariantKey {
        min_lod_clamped: 1,
        ..VariantKey::default()
    };
    let converted = rewrite(module, reflection, &variant, &ROUTE_KEY).expect("rewritten");
    let helper = naga::ResourceBinding {
        group: HELPER_SET,
        binding: helper_binding(wcontainers_gpu::ShaderStage::Fragment),
    };
    assert!(
        converted.modules[0]
            .module
            .global_variables
            .iter()
            .any(|(_, g)| g.binding == Some(helper)),
        "the helper block is bound"
    );
    assert!(!converted.reflection.helpers.is_empty());
}
