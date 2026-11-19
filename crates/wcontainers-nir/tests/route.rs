//! Candidate A end to end on a program naga writes as SPIR-V: SPIR-V to NIR, NIR's serialised key,
//! Zink's SPIR-V, naga's reader and validation.
#![cfg(mesa)]

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
