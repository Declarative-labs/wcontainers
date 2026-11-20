//! Both candidates end to end on programs naga writes as SPIR-V: SPIR-V to NIR, NIR's serialised
//! key, the common form's step, then Zink's SPIR-V and naga's reader (A) or NIR read straight into
//! naga (B), each validated.
#![cfg(mesa)]

use wcontainers_gpu::{ShaderStage, SlotKind};
use wcontainers_nir::bind::stage_set;
use wcontainers_nir::mesa::{Nir, Stage};
use wcontainers_shaders::ENTRY_POINT;

const COMPUTE: &str = "
@group(0) @binding(0) var<storage, read_write> data: array<u32>;
@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    data[id.x] = data[id.x] * 3u + 1u;
}
";

fn spirv(wgsl: &str) -> Vec<u32> {
    let module = naga::front::wgsl::parse_str(wgsl).expect("the test program parses");
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .expect("the test program validates");
    naga::back::spv::write_vec(&module, &info, &naga::back::spv::Options::default(), None)
        .expect("naga writes SPIR-V")
}

#[test]
fn a_compute_program_goes_through_zink_and_back() {
    let nir = Nir::from_spirv(&spirv(COMPUTE), Stage::Compute, ENTRY_POINT).unwrap();
    assert!(!nir.serialized().unwrap().is_empty());
    let words = nir.into_spirv().unwrap();
    let (module, _) = wcontainers_nir::spirv::read(&words).unwrap();
    assert_eq!(module.entry_points.len(), 1);
    assert_eq!(module.entry_points[0].stage, naga::ShaderStage::Compute);
}

const SAMPLED: &str = "
@group(2) @binding(0) var<uniform> tint: vec4<f32>;
@group(0) @binding(3) var picture: texture_2d<f32>;
@group(0) @binding(1) var nearest: sampler;
@fragment
fn main(@location(0) at: vec2<f32>) -> @location(0) vec4<f32> {
    return textureSample(picture, nearest, at) * tint;
}
";

#[test]
fn resources_are_numbered_by_kind_in_the_stage_set() {
    let mut nir = Nir::from_spirv(&spirv(SAMPLED), Stage::Fragment, ENTRY_POINT).unwrap();
    let mut bindings = nir.to_common_form(Stage::Fragment).unwrap();
    bindings.sort_by_key(|b| b.binding);
    let set = stage_set(ShaderStage::Fragment);
    let found: Vec<_> = bindings
        .iter()
        .map(|b| (b.kind, b.space, b.register, b.set, b.binding))
        .collect();
    let first = SlotKind::first_binding;
    assert_eq!(
        found,
        [
            (SlotKind::Uniform, 2, 0, set, first(SlotKind::Uniform)),
            (SlotKind::Sampler, 0, 1, set, first(SlotKind::Sampler)),
            (SlotKind::Texture, 0, 3, set, first(SlotKind::Texture)),
        ]
    );
    let (module, _) = wcontainers_nir::spirv::read(&nir.into_spirv().unwrap()).unwrap();
    let mut bound: Vec<_> = module
        .global_variables
        .iter()
        .filter_map(|(_, g)| g.binding)
        .map(|b| (b.group, b.binding))
        .collect();
    bound.sort();
    let expected: Vec<_> = bindings.iter().map(|b| (b.set, b.binding)).collect();
    assert_eq!(bound, expected);
}

#[test]
fn the_key_follows_the_nir() {
    let key = |wgsl| {
        Nir::from_spirv(&spirv(wgsl), Stage::Compute, ENTRY_POINT)
            .unwrap()
            .key()
            .unwrap()
    };
    assert_eq!(key(COMPUTE), key(COMPUTE));
    assert_ne!(key(COMPUTE), key(&COMPUTE.replace("3u", "5u")));
}

#[test]
fn both_routes_bind_a_sampling_program_alike() {
    let bound = |module: &naga::Module| {
        let mut b: Vec<_> = module
            .global_variables
            .iter()
            .filter_map(|(_, g)| g.binding)
            .map(|b| (b.group, b.binding))
            .collect();
        b.sort();
        b
    };
    let mut a = Nir::from_spirv(&spirv(SAMPLED), Stage::Fragment, ENTRY_POINT).unwrap();
    a.to_common_form(Stage::Fragment).unwrap();
    let (through_zink, _) = wcontainers_nir::spirv::read(&a.into_spirv().unwrap()).unwrap();
    let mut b = Nir::from_spirv(&spirv(SAMPLED), Stage::Fragment, ENTRY_POINT).unwrap();
    b.to_common_form(Stage::Fragment).unwrap();
    let direct = wcontainers_nir::direct::module(&mut b).unwrap();
    wcontainers_nir::spirv::validate(&direct).unwrap();
    assert_eq!(bound(&direct), bound(&through_zink));
    assert_eq!(direct.entry_points[0].stage, naga::ShaderStage::Fragment);
}

#[test]
fn candidate_b_reads_a_compute_program() {
    let mut nir = Nir::from_spirv(&spirv(COMPUTE), Stage::Compute, ENTRY_POINT).unwrap();
    nir.to_common_form(Stage::Compute).unwrap();
    let module = wcontainers_nir::direct::module(&mut nir).unwrap();
    wcontainers_nir::spirv::validate(&module).unwrap();
    assert_eq!(module.entry_points[0].workgroup_size, [1, 1, 1]);
}
