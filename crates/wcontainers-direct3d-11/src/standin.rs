//! Hand-written conversions of a few DXBC shaders, in the converter's output form (a naga module and its
//! reflection), keyed by the DXBC's content key like entries of the shader cache. They stand in for the
//! shaders crate's `convert` until it exists; any other shader is refused as not implemented.

use wcontainers_gpu::{
    Interpolation, ReflectedBinding, Reflection, ScalarKind, ShaderModule, ShaderStage, SlotKind,
    SourceStage, StageExtras, StageValue, SystemValue,
};

/// A shader in the converter's output form.
pub struct Converted {
    pub module: ShaderModule,
    pub reflection: Reflection,
}

/// A value of a stage's signature: semantic, index, register, mask, system value, location.
type Value = (&'static str, u32, u32, u8, Option<SystemValue>, u32);

struct Entry {
    /// blake3 of the DXBC bytes, as hexadecimal.
    key: &'static str,
    /// Where the DXBC comes from, for the test that checks the keys.
    source: &'static str,
    stage: SourceStage,
    wgsl: &'static str,
    inputs: &'static [Value],
    outputs: &'static [Value],
    /// Slot kind and register of each resource the shader reads.
    bindings: &'static [(SlotKind, u32)],
}

const POSITION: Option<SystemValue> = Some(SystemValue::Position);
const TARGET0: Option<SystemValue> = Some(SystemValue::Target(0));

const ENTRIES: &[Entry] = &[
    Entry {
        key: "03d2a2be466e607e28cdd479b0fb9d81e36494cba2f75071c5fd32b94b2be36d",
        source: "triangle.vs",
        stage: SourceStage::Vertex,
        wgsl: "struct Out { @builtin(position) pos: vec4f, @location(0) color: vec3f }
@vertex fn main(@location(0) pos: vec2f, @location(1) color: vec3f) -> Out {
    return Out(vec4f(pos, 0.0, 1.0), color);
}",
        inputs: &[("POSITION", 0, 0, 0x3, None, 0), ("COLOR", 0, 1, 0x7, None, 1)],
        outputs: &[("SV_Position", 0, 0, 0xf, POSITION, 0), ("COLOR", 0, 1, 0x7, None, 0)],
        bindings: &[],
    },
    Entry {
        key: "92b49f506d0e29cb83eb58700a1d6e1fa844a51d45432830bc2b892f54e2e660",
        source: "triangle.ps",
        stage: SourceStage::Pixel,
        wgsl: "@fragment fn main(@location(0) color: vec3f) -> @location(0) vec4f {
    return vec4f(color, 1.0);
}",
        inputs: &[("SV_Position", 0, 0, 0xf, POSITION, 0), ("COLOR", 0, 1, 0x7, None, 0)],
        outputs: &[("SV_Target", 0, 0, 0xf, TARGET0, 0)],
        bindings: &[],
    },
    Entry {
        key: "fee7bc6991e3552a6131d76a96eb6404efc52676da9180371cd79d5e684c1e90",
        source: "quads.vs",
        stage: SourceStage::Vertex,
        wgsl: "struct Out { @builtin(position) pos: vec4f, @location(0) uv: vec2f, @location(1) color: vec4f }
@vertex fn main(@location(0) pos: vec3f, @location(1) uv: vec2f, @location(2) color: vec4f) -> Out {
    return Out(vec4f(pos, 1.0), uv, color);
}",
        inputs: &[
            ("POSITION", 0, 0, 0x7, None, 0),
            ("TEXCOORD", 0, 1, 0x3, None, 1),
            ("COLOR", 0, 2, 0xf, None, 2),
        ],
        outputs: &[
            ("SV_Position", 0, 0, 0xf, POSITION, 0),
            ("TEXCOORD", 0, 1, 0x3, None, 0),
            ("COLOR", 0, 2, 0xf, None, 1),
        ],
        bindings: &[],
    },
    Entry {
        key: "b42083ada0ff5a0a33024109239616042619a485660901320f367361097c0e0b",
        source: "quads.ps",
        stage: SourceStage::Pixel,
        wgsl: "@group(1) @binding(32) var tex: texture_2d<f32>;
@group(1) @binding(16) var smp: sampler;
@fragment fn main(@location(0) uv: vec2f, @location(1) color: vec4f) -> @location(0) vec4f {
    return textureSample(tex, smp, uv) * color;
}",
        inputs: &[
            ("SV_Position", 0, 0, 0xf, POSITION, 0),
            ("TEXCOORD", 0, 1, 0x3, None, 0),
            ("COLOR", 0, 2, 0xf, None, 1),
        ],
        outputs: &[("SV_Target", 0, 0, 0xf, TARGET0, 0)],
        bindings: &[(SlotKind::Texture, 0), (SlotKind::Sampler, 0)],
    },
    Entry {
        key: "2c4016709d75b3997b558932a2387627167333bcb70e63cced58bba28b1b6385",
        source: "common.vs_color",
        stage: SourceStage::Vertex,
        wgsl: "struct Out { @builtin(position) pos: vec4f, @location(0) color: vec4f }
@vertex fn main(@location(0) pos: vec3f, @location(1) color: vec4f) -> Out {
    return Out(vec4f(pos, 1.0), color);
}",
        inputs: &[("POSITION", 0, 0, 0x7, None, 0), ("COLOR", 0, 1, 0xf, None, 1)],
        outputs: &[("SV_Position", 0, 0, 0xf, POSITION, 0), ("COLOR", 0, 1, 0xf, None, 0)],
        bindings: &[],
    },
    Entry {
        key: "8efc0d8fd23bc77f34f2e2b6616e03422a3cbdd9fddafe79364d963d4b6f903e",
        source: "common.ps_color",
        stage: SourceStage::Pixel,
        wgsl: "@fragment fn main(@location(0) color: vec4f) -> @location(0) vec4f {
    return color;
}",
        inputs: &[("SV_Position", 0, 0, 0xf, POSITION, 0), ("COLOR", 0, 1, 0xf, None, 0)],
        outputs: &[("SV_Target", 0, 0, 0xf, TARGET0, 0)],
        bindings: &[],
    },
];

fn hex(key: &[u8; 32]) -> String {
    key.iter().map(|b| format!("{b:02x}")).collect()
}

/// The content key of a shader's bytecode.
pub fn key(bytecode: &[u8]) -> [u8; 32] {
    *blake3::hash(bytecode).as_bytes()
}

/// The hand-written conversion of `bytecode`, if there is one.
pub fn convert(bytecode: &[u8]) -> Option<Converted> {
    let key = key(bytecode);
    let wanted = hex(&key);
    let entry = ENTRIES.iter().find(|e| e.key == wanted)?;
    let module = naga::front::wgsl::parse_str(entry.wgsl).expect("a stand-in shader is valid WGSL");
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .expect("a stand-in shader validates");
    let (stage, set) = match entry.stage {
        SourceStage::Vertex => (ShaderStage::Vertex, 0),
        _ => (ShaderStage::Fragment, 1),
    };
    let values = |list: &[Value]| -> Vec<StageValue> {
        list.iter()
            .map(
                |&(semantic, index, register, mask, system, location)| StageValue {
                    semantic: semantic.into(),
                    semantic_index: index,
                    register,
                    mask,
                    scalar: ScalarKind::Float,
                    interpolation: Interpolation::Perspective,
                    system,
                    location,
                    component: 0,
                },
            )
            .collect()
    };
    let reflection = Reflection {
        stage: entry.stage,
        inputs: values(entry.inputs),
        outputs: values(entry.outputs),
        bindings: entry
            .bindings
            .iter()
            .map(|&(kind, register)| ReflectedBinding {
                kind,
                space: 0,
                register,
                count: 1,
                set,
                binding: kind.first_binding() + register,
                added: None,
            })
            .collect(),
        helpers: Vec::new(),
        helper_size: 0,
        sampled_pairs: Vec::new(),
        extras: match entry.stage {
            SourceStage::Pixel => StageExtras::Pixel {
                early_tests: false,
                side_effects: false,
                discards: false,
                per_sample: false,
            },
            _ => StageExtras::None,
        },
    };
    Some(Converted {
        module: ShaderModule {
            module,
            info,
            entry_point: "main".into(),
            stage,
            key,
        },
        reflection,
    })
}

/// Where each stand-in's DXBC comes from and its key, for checking the table against the corpus.
pub fn sources() -> impl Iterator<Item = (&'static str, &'static str)> {
    ENTRIES.iter().map(|e| (e.source, e.key))
}
