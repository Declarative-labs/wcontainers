//! Program features the device declares, each drawn where declared: subgroup operations and the
//! primitive index where the adapter has them, fine derivatives always (WGSL's core). Shared by
//! tests/features.rs and the browser page.

use crate::draw;
use crate::renames::{Kit, begin, finish, kit};
use wcontainers_gpu::{
    Backend, DeviceStatus, DrawCall, DrawRange, Features, Format, GpuError, Program, ShaderModule,
    ShaderStage, Topology, Viewport,
};
use wcontainers_gpu_webgpu::WebGpu;

/// Bytes of an `Rgba8Unorm` texel.
const TEXEL: u32 = size_of::<[u8; 4]>() as u32;
/// Pixels of the one-row target each check draws into.
const WIDTH: u32 = 64;
/// The largest value of an 8-bit normalised channel.
const UNORM8_MAX: u8 = u8::MAX;

/// Every pixel: red where every invocation of its subgroup is active as seen by `subgroupAll`,
/// green where the subgroup's sum of ones equals its ballot's count.
pub const WAVES_WGSL: &str = r#"
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

/// Every pixel: red where `dpdxFine` of x times y is the pixel's own y, green where `dpdyFine` of it
/// is the pixel's own x, blue where `fwidthFine` is their sum. Coarse derivatives give one pixel of
/// each 2 by 2 quad's row or column the other's value, which differs by one.
pub const FINE_WGSL: &str = r#"
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn ps(@builtin(position) p: vec4f) -> @location(0) vec4f {
    let f = p.x * p.y;
    let across = dpdxFine(f) == p.y;
    let down = dpdyFine(f) == p.x;
    let width = fwidthFine(f) == p.x + p.y;
    return vec4f(select(0.0, 1.0, across), select(0.0, 1.0, down), select(0.0, 1.0, width), 1.0);
}
"#;

/// Vertices of one triangle of a list.
const PER_TRIANGLE: u32 = 3;

/// Triangles drawn by the primitive index check: each covers its own equal run of the row.
pub const TRIANGLES: u32 = 4;

/// `TRIANGLES` triangles side by side over a one-row target, triangle `t` covering columns
/// `t * w` to `(t + 1) * w` at the row's centre; each pixel's red is its primitive index in 8-bit
/// steps.
pub fn primitives_wgsl() -> String {
    format!(
        r#"
enable primitive_index;
const TRIANGLES: u32 = {TRIANGLES}u;
const PER_TRIANGLE: u32 = {PER_TRIANGLE}u;
const UNORM8_MAX: f32 = {UNORM8_MAX}.0;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {{
    let w = 2.0 / f32(TRIANGLES);
    let x0 = f32(i / PER_TRIANGLE) * w - 1.0;
    let corner = i % PER_TRIANGLE;
    let x = select(x0, x0 + 2.0 * w, corner == 2u);
    let y = select(-1.0, 1.0, corner == 1u);
    return vec4f(x, y, 0.0, 1.0);
}}
@fragment fn ps(@builtin(primitive_index) p: u32) -> @location(0) vec4f {{
    return vec4f(f32(p) / UNORM8_MAX, 0.0, 0.0, 1.0);
}}
"#
    )
}

/// A program feature this module checks.
#[derive(Clone, Copy, Debug)]
pub enum Feature {
    Subgroups,
    PrimitiveIndex,
    FineDerivatives,
}

impl Feature {
    pub const ALL: [Feature; 3] = [
        Feature::Subgroups,
        Feature::PrimitiveIndex,
        Feature::FineDerivatives,
    ];

    /// What the interface calls it.
    pub fn declared(self) -> Features {
        match self {
            Feature::Subgroups => Features::SUBGROUPS,
            Feature::PrimitiveIndex => Features::PRIMITIVE_INDEX,
            Feature::FineDerivatives => Features::FINE_DERIVATIVES,
        }
    }

    /// What wgpu calls it; none where WebGPU's core has it.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub fn wgpu(self) -> Option<wgpu::Features> {
        match self {
            Feature::Subgroups => Some(wgpu::Features::SUBGROUP),
            Feature::PrimitiveIndex => Some(wgpu::Features::PRIMITIVE_INDEX),
            Feature::FineDerivatives => None,
        }
    }

    /// What WebGPU calls it (`GPUFeatureName`); none where its core has it.
    pub fn web_name(self) -> Option<&'static str> {
        match self {
            Feature::Subgroups => Some("subgroups"),
            Feature::PrimitiveIndex => Some("primitive-index"),
            Feature::FineDerivatives => None,
        }
    }

    /// What the checks' lines call it.
    pub fn name(self) -> &'static str {
        match self {
            Feature::FineDerivatives => "fine derivatives",
            _ => self
                .web_name()
                .expect("an optional feature has its WebGPU name"),
        }
    }

    fn source(self) -> String {
        match self {
            Feature::Subgroups => WAVES_WGSL.into(),
            Feature::PrimitiveIndex => primitives_wgsl(),
            Feature::FineDerivatives => FINE_WGSL.into(),
        }
    }

    /// The pixels the program draws into the row, as `Rgba8Unorm` bytes.
    fn expected(self) -> Vec<u8> {
        match self {
            Feature::Subgroups => [UNORM8_MAX, UNORM8_MAX, 0, UNORM8_MAX].repeat(WIDTH as usize),
            Feature::FineDerivatives => [UNORM8_MAX; 4].repeat(WIDTH as usize),
            Feature::PrimitiveIndex => (0..WIDTH)
                .flat_map(|x| {
                    let primitive = (x * TRIANGLES / WIDTH) as u8;
                    [primitive, 0, 0, UNORM8_MAX]
                })
                .collect(),
        }
    }

    fn vertices(self) -> u32 {
        match self {
            Feature::Subgroups | Feature::FineDerivatives => PER_TRIANGLE,
            Feature::PrimitiveIndex => PER_TRIANGLE * TRIANGLES,
        }
    }
}

/// The feature's program for `stage`, validated as the shader converter does for a device with
/// every program feature; the back end checks it against its own device.
pub fn shader(
    b: &mut WebGpu,
    feature: Feature,
    stage: ShaderStage,
) -> Result<<WebGpu as Backend>::Shader, GpuError> {
    let module = naga::front::wgsl::parse_str(&feature.source()).expect("the program parses");
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::SUBGROUP | naga::valid::Capabilities::PRIMITIVE_INDEX,
    )
    .subgroup_stages(naga::valid::ShaderStages::all())
    .subgroup_operations(naga::valid::SubgroupOperationSet::all())
    .validate(&module)
    .expect("the program is valid with every program feature");
    let entry = match stage {
        ShaderStage::Vertex => "vs",
        _ => "ps",
    };
    b.create_shader(&ShaderModule {
        module,
        info,
        entry_point: entry.into(),
        stage,
        key: [0; 32],
    })
}

/// Where the device declares `feature`, draws its program and requires every pixel as the
/// feature defines it; one line saying what was found, or the failed check.
pub fn check(b: &mut WebGpu, feature: Feature) -> Result<String, String> {
    if !b.caps().has(feature.declared()) {
        return Ok(format!("{}: absent", feature.name()));
    }
    let vs = shader(b, feature, ShaderStage::Vertex).map_err(|e| format!("{e:?}"))?;
    let ps = shader(b, feature, ShaderStage::Fragment).map_err(|e| format!("{e:?}"))?;
    let k = Kit {
        program: b
            .create_program(&Program::Render {
                vertex: &vs,
                fragment: Some(&ps),
            })
            .map_err(|e| format!("{e:?}"))?,
        ..kit(b, Format::Rgba8Unorm, WIDTH, &draw::opaque())
    };
    let set = b.create_binding_set(&[]).map_err(|e| format!("{e:?}"))?;
    begin(b, &k, [0.0; 4]);
    draw_vertices(b, &k, &set, feature.vertices());
    let got = finish(b, &k, TEXEL);
    if b.status() != DeviceStatus::Ok {
        return Err(format!("{}: device {:?}", feature.name(), b.status()));
    }
    if got != feature.expected() {
        return Err(format!(
            "{}: drawn {:?}, not {:?}",
            feature.name(),
            got,
            feature.expected()
        ));
    }
    Ok(format!(
        "{}: declared, {WIDTH} pixels drawn as it defines",
        feature.name()
    ))
}

/// Draws `count` vertices of the kit's program over the whole row.
fn draw_vertices(b: &mut WebGpu, k: &Kit, set: &<WebGpu as Backend>::BindingSet, count: u32) {
    let viewport = [Viewport {
        x: 0,
        y: 0,
        width: (k.width as f32).to_bits(),
        height: 1.0f32.to_bits(),
        min_depth: 0,
        max_depth: 1.0f32.to_bits(),
    }];
    b.draw(&DrawCall {
        program: &k.program,
        blend: &k.blend,
        depth_stencil: &k.depth_stencil,
        raster: &k.raster,
        vertex_layout: &k.vertex_layout,
        bindings: [None, Some(set), None, None],
        vertex_buffers: &[],
        index: None,
        topology: Topology::TriangleList,
        restart: false,
        viewports: &viewport,
        scissors: &[],
        blend_constant: [0; 4],
        stencil_reference: [0; 2],
        immediates: &[],
        range: DrawRange::Vertices {
            first: 0,
            count,
            first_instance: 0,
            instances: 1,
        },
    });
}
