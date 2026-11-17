//! The reference scenes: the geometry, textures, constants and shaders every renderer of them shares.

use std::f32::consts::TAU;

pub const WIDTH: u32 = 256;
pub const HEIGHT: u32 = 256;

/// Background of every scene. Each channel is an exact 8-bit value, so no renderer has to round a tie.
pub const CLEAR: [f32; 4] = [26.0 / 255.0, 26.0 / 255.0, 38.0 / 255.0, 1.0];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    /// One triangle with a colour at each corner.
    Triangle,
    /// Two textured quads: the near one blended, the far one partly hidden by the depth test.
    Quads,
    /// One small triangle drawn sixteen times, placed and coloured from a constant buffer.
    Instanced,
}

impl Scene {
    pub const ALL: [Scene; 3] = [Scene::Triangle, Scene::Quads, Scene::Instanced];

    pub fn name(self) -> &'static str {
        match self {
            Scene::Triangle => "triangle",
            Scene::Quads => "quads",
            Scene::Instanced => "instanced",
        }
    }

    pub fn from_name(name: &str) -> Option<Scene> {
        Scene::ALL.into_iter().find(|s| s.name() == name)
    }

    /// The scenes a command line names: one by name, or all of them.
    pub fn parse_list(arg: &str) -> anyhow::Result<Vec<Scene>> {
        if arg == "all" {
            return Ok(Scene::ALL.to_vec());
        }
        Scene::from_name(arg).map(|s| vec![s]).ok_or_else(|| {
            let names: Vec<_> = Scene::ALL.iter().map(|s| s.name()).collect();
            anyhow::anyhow!(
                "unknown scene {arg}; scenes are {} or all",
                names.join(", ")
            )
        })
    }
}

/// Triangle vertices: position x, y and colour r, g, b.
pub const TRIANGLE: [[f32; 5]; 3] = [
    [0.0, 0.8, 1.0, 0.0, 0.0],
    [0.8, -0.7, 0.0, 1.0, 0.0],
    [-0.8, -0.7, 0.0, 0.0, 1.0],
];

pub const TRIANGLE_HLSL: &str = r"
struct VsIn { float2 pos : POSITION; float3 color : COLOR; };
struct VsOut { float4 pos : SV_Position; float3 color : COLOR; };
VsOut vs(VsIn i) { VsOut o; o.pos = float4(i.pos, 0.0, 1.0); o.color = i.color; return o; }
float4 ps(VsOut i) : SV_Target { return float4(i.color, 1.0); }
";

pub const TRIANGLE_WGSL: &str = include_str!("triangle.wgsl");

/// Quad vertices: position x, y, z, texture u, v and colour r, g, b, a. The first quad is near
/// and half transparent, the second far and opaque; they overlap, and the near one is drawn first.
pub const QUADS: [[f32; 9]; 8] = [
    [-0.7, 0.4, 0.25, 0.0, 0.0, 1.0, 1.0, 1.0, 0.6],
    [0.3, 0.4, 0.25, 2.0, 0.0, 1.0, 1.0, 1.0, 0.6],
    [0.3, -0.6, 0.25, 2.0, 2.0, 1.0, 1.0, 1.0, 0.6],
    [-0.7, -0.6, 0.25, 0.0, 2.0, 1.0, 1.0, 1.0, 0.6],
    [-0.3, 0.6, 0.75, 0.0, 0.0, 1.0, 0.8, 0.6, 1.0],
    [0.7, 0.6, 0.75, 1.0, 0.0, 1.0, 0.8, 0.6, 1.0],
    [0.7, -0.4, 0.75, 1.0, 1.0, 1.0, 0.8, 0.6, 1.0],
    [-0.3, -0.4, 0.75, 0.0, 1.0, 1.0, 0.8, 0.6, 1.0],
];

pub const QUAD_INDICES: [u16; 12] = [0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7];

pub const QUADS_HLSL: &str = r"
Texture2D tex : register(t0);
SamplerState smp : register(s0);
struct VsIn { float3 pos : POSITION; float2 uv : TEXCOORD; float4 color : COLOR; };
struct VsOut { float4 pos : SV_Position; float2 uv : TEXCOORD; float4 color : COLOR; };
VsOut vs(VsIn i) { VsOut o; o.pos = float4(i.pos, 1.0); o.uv = i.uv; o.color = i.color; return o; }
float4 ps(VsOut i) : SV_Target { return tex.Sample(smp, i.uv) * i.color; }
";

pub const TEXTURE_SIZE: u32 = 16;

/// The quads' texture, RGBA8: a checker of four-texel squares in two colours.
pub fn texture() -> Vec<u8> {
    let mut texels = Vec::with_capacity((TEXTURE_SIZE * TEXTURE_SIZE * 4) as usize);
    for y in 0..TEXTURE_SIZE {
        for x in 0..TEXTURE_SIZE {
            let texel = if (x / 4 + y / 4) % 2 == 0 {
                [230, 80, 40, 255]
            } else {
                [40, 120, 220, 255]
            };
            texels.extend_from_slice(&texel);
        }
    }
    texels
}

/// The instanced scene's triangle: position x, y.
pub const INSTANCE_SHAPE: [[f32; 2]; 3] = [[0.0, 0.1], [0.09, -0.07], [-0.09, -0.07]];

pub const INSTANCES: u32 = 16;

pub const INSTANCED_HLSL: &str = r"
cbuffer Scene : register(b0) {
    float4 placement[16]; // offset x, y; cosine and sine of the turn
    float4 colors[16];
    float4 scale;
};
struct VsIn { float2 pos : POSITION; uint id : SV_InstanceID; };
struct VsOut { float4 pos : SV_Position; float4 color : COLOR; };
VsOut vs(VsIn i) {
    float4 p = placement[i.id];
    float2 turned = float2(i.pos.x * p.z - i.pos.y * p.w, i.pos.x * p.w + i.pos.y * p.z);
    VsOut o;
    o.pos = float4(turned * scale.xy + p.xy, 0.0, 1.0);
    o.color = colors[i.id];
    return o;
}
float4 ps(VsOut i) : SV_Target { return i.color; }
";

/// The instanced scene's constant buffer, laid out as `INSTANCED_HLSL`'s `Scene`: sixteen
/// placements, sixteen colours and a scale, each a float4.
pub fn instance_constants() -> Vec<[f32; 4]> {
    let mut placement = Vec::new();
    let mut colors = Vec::new();
    for i in 0..INSTANCES {
        let (col, row) = ((i % 4) as f32, (i / 4) as f32);
        let turn = i as f32 * TAU / INSTANCES as f32;
        placement.push([-0.75 + 0.5 * col, 0.75 - 0.5 * row, turn.cos(), turn.sin()]);
        colors.push([col / 3.0, row / 3.0, 1.0 - col / 3.0, 1.0]);
    }
    let mut constants = placement;
    constants.extend(colors);
    constants.push([1.8, 1.8, 0.0, 0.0]);
    constants
}
