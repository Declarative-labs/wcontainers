//! Subgroup operations: declared exactly where the device has them, run in a fragment program
//! where declared, and refused by name where not.
//!
//! Needs a GPU; `WEBGPU_API` as in tests/scenes.rs.

#[path = "../../wcontainers-graphics-test/src/scene.rs"]
#[allow(dead_code)]
mod scene;

#[allow(dead_code)]
#[path = "../examples/scenes/draw.rs"]
mod draw;

#[allow(dead_code)]
#[path = "../examples/scenes/renames.rs"]
mod renames;

use renames::{Kit, begin, draw_at, finish, kit};
use wcontainers_gpu::{
    Backend, DeviceStatus, Features, Format, GpuError, Program, ShaderModule, ShaderStage,
};
use wcontainers_gpu_webgpu::WebGpu;

fn backends() -> wgpu::Backends {
    match std::env::var("WEBGPU_API").as_deref() {
        Ok("dx12") => wgpu::Backends::DX12,
        Ok("vulkan") => wgpu::Backends::VULKAN,
        Ok("metal") => wgpu::Backends::METAL,
        _ => wgpu::Backends::PRIMARY,
    }
}

/// Every pixel: red where every invocation of its subgroup is active as seen by `subgroupAll`,
/// green where the subgroup's sum of ones equals its ballot's count.
const WAVES_WGSL: &str = r#"
enable subgroups;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn ps() -> @location(0) vec4f {
    let all = subgroupAll(true);
    let ones = subgroupAdd(1u);
    let b = subgroupBallot(true);
    let counted = countOneBits(b.x) + countOneBits(b.y) + countOneBits(b.z) + countOneBits(b.w);
    return vec4f(select(0.0, 1.0, all), select(0.0, 1.0, ones == counted && ones > 0u), 0.0, 1.0);
}
"#;

fn shader(
    b: &mut WebGpu,
    module: &naga::Module,
    entry: &str,
    stage: ShaderStage,
) -> Result<<WebGpu as Backend>::Shader, GpuError> {
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::SUBGROUP,
    )
    .subgroup_stages(naga::valid::ShaderStages::all())
    .subgroup_operations(naga::valid::SubgroupOperationSet::all())
    .validate(module)
    .unwrap();
    b.create_shader(&ShaderModule {
        module: module.clone(),
        info,
        entry_point: entry.into(),
        stage,
        key: [0; 32],
    })
}

/// Where the adapter has wgpu's `SUBGROUP`, the device declares `SUBGROUPS` and a fragment
/// program using them draws every pixel as the operations define; a device made without it
/// declares none and refuses the program.
#[test]
fn subgroup_operations_run_where_declared_and_are_refused_elsewhere() {
    let module = naga::front::wgsl::parse_str(WAVES_WGSL).unwrap();
    let instance = draw::instance(backends());
    let (mut b, name) =
        pollster::block_on(draw::device_without(&instance, wgpu::Features::empty())).unwrap();
    let has = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .map(|a| a.features().contains(wgpu::Features::SUBGROUP))
    .unwrap_or(false);
    eprintln!("{name}: adapter has subgroups: {has}");
    assert_eq!(b.caps().has(Features::SUBGROUPS), has);
    if has {
        const WIDTH: u32 = 64;
        let vs = shader(&mut b, &module, "vs", ShaderStage::Vertex).unwrap();
        let ps = shader(&mut b, &module, "ps", ShaderStage::Fragment).unwrap();
        let k = Kit {
            program: b
                .create_program(&Program::Render {
                    vertex: &vs,
                    fragment: Some(&ps),
                })
                .unwrap(),
            ..kit(&mut b, Format::Rgba8Unorm, WIDTH, &draw::opaque())
        };
        let set = b.create_binding_set(&[]).unwrap();
        begin(&mut b, &k, [0.0; 4]);
        draw_at(&mut b, &k, &set, None);
        let got = finish(&mut b, &k, 4);
        assert_eq!(b.status(), DeviceStatus::Ok);
        assert_eq!(got, [255, 255, 0, 255].repeat(WIDTH as usize));
    }
    let (mut without, _) =
        pollster::block_on(draw::device_without(&instance, wgpu::Features::SUBGROUP)).unwrap();
    assert!(!without.caps().has(Features::SUBGROUPS));
    assert!(matches!(
        shader(&mut without, &module, "ps", ShaderStage::Fragment),
        Err(GpuError::Invalid(_))
    ));
}
