//! A GLSL program held to llvmpipe (plan 15.3): the program run by llvmpipe through OpenGL, and the
//! route's modules, written to SPIR-V by naga and run by lavapipe, on the same seeded inputs. The
//! inputs follow OpenGL's own reflection of the program: each default-block uniform set with
//! `glUniform*` and written into uniform buffer 0 at the words the bridge names for it; each block
//! filled member by member by its type and bound, by its name, at the binding the bridge names.
//! Compute programs are dispatched and their storage blocks compared, each word by its member's
//! type. Vertex and fragment programs are drawn as points, one a pixel of a row: a vertex program
//! is wrapped in its own source (its `main` called by one that puts the point on its pixel and
//! keeps its position); one alone is followed by a catcher that writes every output it reaches to
//! a storage block; a fragment program alone is fed by a vertex program written from its inputs.
//! Colour targets, depth and storage blocks are compared.

use crate::compare::{
    self, COMPUTE_GROUPS, CompareOptions, NAMED_DIFFERENCES, RUNTIME_ELEMENTS, Verdict,
};
use crate::interface::{COMPONENT_BYTES, Kind, Ty};

/// The bytes of a word of a block.
const BYTES_A_WORD: usize = COMPONENT_BYTES as usize;
use crate::lavapipe::{self, Attribute, Element, Job, Lavapipe};
use crate::llvmpipe::{
    AttributeSlot, Block, DrawOptions, Llvmpipe, Reflection, SampledTexture, StageValue,
    UniformValue, Variable, VertexAttribute,
};
use ash::vk;
use naga::Module;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use wcontainers_gpu::ReflectedBinding;
use wcontainers_nir::mesa::{GlslName, GlslNameKind, Stage};

/// What OpenGL's reflection adds to an array's name.
const FIRST_ELEMENT: &str = "[0]";
/// A name inside an array of structures or of arrays: its outermost array is apart from its own.
const INSIDE_ARRAY: [&str; 2] = ["[0].", "[0]["];
/// The driver's true in a default-block boolean (the bridge's `UniformBooleanTrue`).
const UNIFORM_TRUE: u32 = u32::MAX;
/// The words a block's size is rounded up to: the common form's uniform arrays step by 16 bytes.
const BLOCK_ALIGNMENT: usize = 4 * BYTES_A_WORD;
/// The height of the row of pixels drawn into.
const ROW_HEIGHT: f32 = 1.0;
/// `STATE_FB_WPOS_Y_TRANSFORM` and `STATE_FB_PNTC_Y_TRANSFORM` as st/mesa fills them for a
/// framebuffer object, whose y is not flipped (`prog_statevars.c`), with the sprite origin upper left.
const FRAG_COORD_Y_TRANSFORM: [f32; 4] = [1.0, 0.0, -1.0, ROW_HEIGHT];
const POINT_COORD_Y_TRANSFORM: [f32; 4] = [1.0, 0.0, 0.0, 0.0];

/// A value type of OpenGL's reflection: its kind, columns and rows, and whether it is boolean.
#[derive(Clone, Copy)]
struct GlType {
    kind: Kind,
    columns: u32,
    rows: u32,
    boolean: bool,
}

fn gl_type(ty: u32) -> Option<GlType> {
    let t = |kind, columns, rows, boolean| {
        Some(GlType {
            kind,
            columns,
            rows,
            boolean,
        })
    };
    use Kind::{Float, Sint, Uint};
    match ty {
        glow::FLOAT => t(Float, 1, 1, false),
        glow::FLOAT_VEC2 => t(Float, 1, 2, false),
        glow::FLOAT_VEC3 => t(Float, 1, 3, false),
        glow::FLOAT_VEC4 => t(Float, 1, 4, false),
        glow::INT => t(Sint, 1, 1, false),
        glow::INT_VEC2 => t(Sint, 1, 2, false),
        glow::INT_VEC3 => t(Sint, 1, 3, false),
        glow::INT_VEC4 => t(Sint, 1, 4, false),
        glow::UNSIGNED_INT => t(Uint, 1, 1, false),
        glow::UNSIGNED_INT_VEC2 => t(Uint, 1, 2, false),
        glow::UNSIGNED_INT_VEC3 => t(Uint, 1, 3, false),
        glow::UNSIGNED_INT_VEC4 => t(Uint, 1, 4, false),
        glow::BOOL => t(Uint, 1, 1, true),
        glow::BOOL_VEC2 => t(Uint, 1, 2, true),
        glow::BOOL_VEC3 => t(Uint, 1, 3, true),
        glow::BOOL_VEC4 => t(Uint, 1, 4, true),
        glow::FLOAT_MAT2 => t(Float, 2, 2, false),
        glow::FLOAT_MAT3 => t(Float, 3, 3, false),
        glow::FLOAT_MAT4 => t(Float, 4, 4, false),
        glow::FLOAT_MAT2x3 => t(Float, 2, 3, false),
        glow::FLOAT_MAT2x4 => t(Float, 2, 4, false),
        glow::FLOAT_MAT3x2 => t(Float, 3, 2, false),
        glow::FLOAT_MAT3x4 => t(Float, 3, 4, false),
        glow::FLOAT_MAT4x2 => t(Float, 4, 2, false),
        glow::FLOAT_MAT4x3 => t(Float, 4, 3, false),
        _ => None,
    }
}

fn scalar(rng: &mut ChaCha8Rng, ty: GlType) -> u32 {
    if ty.boolean {
        u32::from(rng.random::<bool>())
    } else {
        compare::fill_scalar(rng, ty.kind)
    }
}

fn typed(v: &Variable) -> Result<GlType, String> {
    gl_type(v.ty).ok_or_else(|| format!("{} of type {:#x} is not filled here", v.name, v.ty))
}

/// A name of OpenGL's reflection as the bridge has it.
fn bare(name: &str) -> &str {
    name.strip_suffix(FIRST_ELEMENT).unwrap_or(name)
}

/// The bytes of one block, each member filled by its type, and each word's kind (None for
/// padding). Runtime-sized arrays get `RUNTIME_ELEMENTS` elements.
fn block_bytes(
    rng: &mut ChaCha8Rng,
    block: &Block,
    members: &[&Variable],
) -> Result<(Vec<u8>, Vec<Option<Kind>>), String> {
    let elements = |n: i32| {
        if n == 0 {
            RUNTIME_ELEMENTS as usize
        } else {
            usize::try_from(n).unwrap_or(1)
        }
    };
    let mut words: Vec<(usize, u32, Kind)> = Vec::new();
    for v in members {
        let ty = typed(v)?;
        let apart = v.top_level_stride != 0 && INSIDE_ARRAY.iter().any(|p| v.name.contains(p));
        let tops = if apart { elements(v.top_level_size) } else { 1 };
        let stride = |x: i32| usize::try_from(x).unwrap_or(0);
        for top in 0..tops {
            for element in 0..elements(v.array_size) {
                for column in 0..ty.columns as usize {
                    for row in 0..ty.rows as usize {
                        let (major, minor) = if v.row_major {
                            (row, column)
                        } else {
                            (column, row)
                        };
                        let at = stride(v.offset)
                            + top * stride(v.top_level_stride)
                            + element * stride(v.array_stride)
                            + major * stride(v.matrix_stride)
                            + minor * BYTES_A_WORD;
                        words.push((at, scalar(rng, ty), ty.kind));
                    }
                }
            }
        }
    }
    let end = words.iter().map(|w| w.0 + BYTES_A_WORD).max().unwrap_or(0);
    let size = end
        .max(block.size as usize)
        .next_multiple_of(BLOCK_ALIGNMENT);
    let mut bytes = vec![0; size];
    let mut kinds = vec![None; size / BYTES_A_WORD];
    for (at, value, kind) in words {
        bytes[at..at + BYTES_A_WORD].copy_from_slice(&value.to_le_bytes());
        kinds[at / BYTES_A_WORD] = Some(kind);
    }
    Ok((bytes, kinds))
}

fn named(names: &[GlslName], kind: GlslNameKind, at: u32) -> Option<&GlslName> {
    names.iter().find(|n| n.kind == kind && n.at == at)
}

/// The words of uniform buffer 0: each default-block uniform's values at the bridge's words.
fn default_block(
    size: usize,
    names: &[GlslName],
    values: &[UniformValue],
) -> Result<Vec<u8>, String> {
    let mut bytes = vec![0; size];
    for name in names {
        let state = match name.kind {
            GlslNameKind::Uniform => None,
            GlslNameKind::FragCoordYTransform => Some(FRAG_COORD_Y_TRANSFORM),
            GlslNameKind::PointCoordYTransform => Some(POINT_COORD_Y_TRANSFORM),
            GlslNameKind::OtherState => {
                return Err(format!("the state value {} is not filled here", name.name));
            }
            _ => continue,
        };
        if let Some(state) = state {
            for (i, value) in (0..).zip(state) {
                let at = (name.at as usize + i) * BYTES_A_WORD;
                bytes
                    .get_mut(at..at + BYTES_A_WORD)
                    .ok_or_else(|| format!("{:?} lies past uniform buffer 0", name.kind))?
                    .copy_from_slice(&value.to_bits().to_le_bytes());
            }
            continue;
        }
        let Some(value) = values.iter().find(|v| bare(&v.variable.name) == name.name) else {
            return Err(format!(
                "the uniform {} has no value from OpenGL here",
                name.name
            ));
        };
        let boolean = gl_type(value.variable.ty).is_some_and(|t| t.boolean);
        for (i, &word) in (0..).zip(&value.words) {
            let word = if boolean && word != 0 {
                UNIFORM_TRUE
            } else {
                word
            };
            let at = (name.at as usize + i) * BYTES_A_WORD;
            bytes
                .get_mut(at..at + BYTES_A_WORD)
                .ok_or_else(|| format!("{} lies past uniform buffer 0", name.name))?
                .copy_from_slice(&word.to_le_bytes());
        }
    }
    Ok(bytes)
}

/// The program's inputs on OpenGL's side, filled from the seed: each default-block uniform's
/// values, and each uniform and storage block's bytes with each word's kind.
struct GlInputs<'a> {
    values: Vec<UniformValue<'a>>,
    uniform_blocks: Vec<(Vec<u8>, Vec<Option<Kind>>)>,
    storage_blocks: Vec<(Vec<u8>, Vec<Option<Kind>>)>,
    /// Each sampler's texture, made from the first route resource that reads it.
    sampled: Vec<SampledTexture>,
}

impl GlInputs<'_> {
    fn uniform_bytes(&self) -> Vec<Vec<u8>> {
        self.uniform_blocks.iter().map(|b| b.0.clone()).collect()
    }

    fn storage_bytes(&self) -> Vec<Vec<u8>> {
        self.storage_blocks.iter().map(|b| b.0.clone()).collect()
    }
}

/// Whether a default-block uniform is a sampler or image of the route's (its texture is set up from
/// the route's resource).
fn opaque(v: &Variable, stages: &[&[GlslName]]) -> bool {
    stages.iter().flat_map(|s| s.iter()).any(|n| {
        matches!(n.kind, GlslNameKind::Sampler | GlslNameKind::Image) && n.name == bare(&v.name)
    })
}

fn gl_inputs<'a>(
    rng: &mut ChaCha8Rng,
    gl: &'a Reflection,
    stages: &[&[GlslName]],
) -> Result<GlInputs<'a>, String> {
    let mut values = Vec::new();
    for v in gl
        .uniforms
        .iter()
        .filter(|v| v.block < 0 && !opaque(v, stages))
    {
        let ty = typed(v)?;
        let count =
            usize::try_from(v.array_size.max(1)).unwrap_or(1) * (ty.columns * ty.rows) as usize;
        let words = (0..count).map(|_| scalar(rng, ty)).collect();
        values.push(UniformValue { variable: v, words });
    }
    let block_contents = |rng: &mut ChaCha8Rng, blocks: &[Block], members: &[Variable]| {
        (0..)
            .zip(blocks)
            .map(|(i, b)| {
                let mine: Vec<&Variable> = members.iter().filter(|v| v.block == i).collect();
                block_bytes(rng, b, &mine)
            })
            .collect::<Result<Vec<_>, String>>()
    };
    Ok(GlInputs {
        values,
        uniform_blocks: block_contents(rng, &gl.uniform_blocks, &gl.uniforms)?,
        storage_blocks: block_contents(rng, &gl.storage_blocks, &gl.buffer_variables)?,
        sampled: Vec::new(),
    })
}

/// Where a storage resource of the route's is on OpenGL's side: a storage block's index, or a
/// storage image's index among the textures with its components' kind.
#[derive(Clone, Copy)]
enum GlStorage {
    Block(usize),
    Image(usize, Option<Kind>),
}

/// Gives a route's texture or storage image resource the texels of the sampler or image it is,
/// making that uniform's texture on OpenGL's side from it the first time; its index among the
/// textures.
fn sampled_texture(
    elements: &mut [Element],
    binding: u32,
    names: &[GlslName],
    gl: &Reflection,
    inputs: &mut GlInputs,
    storage: bool,
) -> Result<usize, String> {
    let [Element::Image(image)] = elements else {
        return Err("an array of textures or images is not set up for GLSL here".into());
    };
    let kind = if storage {
        GlslNameKind::Image
    } else {
        GlslNameKind::Sampler
    };
    let name = named(names, kind, binding)
        .ok_or_else(|| format!("no {kind:?} of the program at binding {binding}"))?;
    let variable = gl
        .uniforms
        .iter()
        .find(|v| bare(&v.name) == name.name)
        .ok_or_else(|| format!("llvmpipe has no {kind:?} {}", name.name))?;
    match inputs
        .sampled
        .iter()
        .position(|s| s.uniform == variable.name)
    {
        Some(at) => {
            image.clone_from(&inputs.sampled[at].image);
            Ok(at)
        }
        None => {
            inputs.sampled.push(SampledTexture {
                uniform: variable.name.clone(),
                ty: variable.ty,
                image: image.clone(),
                storage,
            });
            Ok(inputs.sampled.len() - 1)
        }
    }
}

/// The route's resources of one stage, given the same bytes as the program's blocks, found by
/// their names; and each storage resource's block index.
fn route_resources(
    rng: &mut ChaCha8Rng,
    module: &Module,
    reflection: &[ReflectedBinding],
    names: &[GlslName],
    gl: &Reflection,
    inputs: &mut GlInputs,
) -> Result<(Vec<compare::Resource>, Vec<GlStorage>), String> {
    let (mut resources, immediates) = compare::resources(module, reflection, rng)?;
    if !immediates.is_empty() {
        return Err("immediate data is not set up for GLSL here".into());
    }
    let mut storage_of = Vec::new();
    for r in &mut resources {
        let binding = r.reference.1;
        let (kind, blocks, contents) = match r.kind {
            vk::DescriptorType::UNIFORM_BUFFER => (
                GlslNameKind::UniformBlock,
                &gl.uniform_blocks,
                &mut inputs.uniform_blocks,
            ),
            vk::DescriptorType::STORAGE_BUFFER => (
                GlslNameKind::StorageBlock,
                &gl.storage_blocks,
                &mut inputs.storage_blocks,
            ),
            vk::DescriptorType::SAMPLED_IMAGE => {
                sampled_texture(&mut r.elements, binding, names, gl, inputs, false)?;
                continue;
            }
            vk::DescriptorType::STORAGE_IMAGE => {
                let kind = r.image_kind;
                let at = sampled_texture(&mut r.elements, binding, names, gl, inputs, true)?;
                storage_of.push(GlStorage::Image(at, kind));
                continue;
            }
            vk::DescriptorType::SAMPLER => continue,
            other => return Err(format!("a {other:?} is not set up for GLSL here")),
        };
        let [Element::Buffer(bytes)] = r.elements.as_mut_slice() else {
            return Err("an array of blocks is not set up for GLSL here".into());
        };
        let Some(name) = named(names, kind, binding) else {
            if kind == GlslNameKind::UniformBlock {
                *bytes = default_block(bytes.len(), names, &inputs.values)?;
                continue;
            }
            return Err(format!(
                "no block of the program at storage binding {binding}"
            ));
        };
        let Some(index) = blocks.iter().position(|b| b.name == name.name) else {
            return Err(format!("llvmpipe has no block {}", name.name));
        };
        let (gl_bytes, _) = &mut contents[index];
        let size = gl_bytes.len().max(bytes.len());
        gl_bytes.resize(size, 0);
        bytes.clone_from(gl_bytes);
        if kind == GlslNameKind::StorageBlock {
            storage_of.push(GlStorage::Block(index));
        }
    }
    Ok((resources, storage_of))
}

/// The differences between llvmpipe's storage blocks and images (the images after the blocks)
/// and the route's: a block's words by its member's type, an image's by its components' kind.
fn storage_differences(
    gl: &Reflection,
    inputs: &GlInputs,
    storage_of: &[GlStorage],
    reference: &[Vec<u8>],
    routed: &[Vec<u8>],
    found: &mut Vec<String>,
) {
    for (&place, after) in storage_of.iter().zip(routed) {
        match place {
            GlStorage::Block(index) => {
                let kinds = &inputs.storage_blocks[index].1;
                compare::differences(
                    &format!("storage block {}", gl.storage_blocks[index].name),
                    &reference[index],
                    after,
                    |at| kinds.get(at).copied().flatten(),
                    found,
                );
            }
            GlStorage::Image(at, kind) => {
                let rank = inputs.sampled[..at].iter().filter(|t| t.storage).count();
                compare::differences(
                    &format!("image {}", inputs.sampled[at].uniform),
                    &reference[gl.storage_blocks.len() + rank],
                    after,
                    |_| Some(kind.unwrap_or(Kind::Uint)),
                    found,
                );
            }
        }
    }
}

fn verdict(mut found: Vec<String>) -> Verdict {
    if found.is_empty() {
        Verdict::Matched
    } else {
        let count = found.len();
        found.truncate(NAMED_DIFFERENCES);
        Verdict::Differs(format!("{count} values differ: {}", found.join("; ")))
    }
}

/// Holds the route's module of a GLSL compute program to llvmpipe running the program.
pub fn compare_compute(
    source: &str,
    names: &[GlslName],
    module: &Module,
    reflection: &[ReflectedBinding],
    options: CompareOptions,
) -> Verdict {
    let not_run = Verdict::NotRun;
    let mut rng = ChaCha8Rng::seed_from_u64(options.seed);
    let route_entry = &module.entry_points[0];
    let route = match compare::write_spirv(
        module,
        naga::ShaderStage::Compute,
        &route_entry.name,
        naga::back::spv::WriterFlags::ADJUST_COORDINATE_SPACE,
    ) {
        Ok(w) => w,
        Err(e) => return not_run(e),
    };
    let llvmpipe = match Llvmpipe::open() {
        Ok(l) => l,
        Err(e) => return not_run(e),
    };
    let program = match llvmpipe.link(&[(Stage::Compute, source)]) {
        Ok(p) => p,
        Err(e) => return not_run(e),
    };
    let gl = llvmpipe.reflect(program);
    let mut inputs = match gl_inputs(&mut rng, &gl, &[names]) {
        Ok(i) => i,
        Err(e) => return not_run(e),
    };
    let (resources, storage_of) =
        match route_resources(&mut rng, module, reflection, names, &gl, &mut inputs) {
            Ok(r) => r,
            Err(e) => return not_run(e),
        };
    let gl_run = || {
        llvmpipe.compute(
            program,
            &inputs.values,
            &inputs.uniform_bytes(),
            &inputs.storage_bytes(),
            &inputs.sampled,
            [COMPUTE_GROUPS, 1, 1],
        )
    };
    let (first, again) = match (gl_run(), gl_run()) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(e), _) | (_, Err(e)) => return not_run(e),
    };
    if first != again {
        let at = (0..)
            .zip(first.iter().zip(&again))
            .find_map(|(block, (a, b))| {
                let words = |x: &[u8]| x.as_chunks::<BYTES_A_WORD>().0.to_vec();
                let (a, b) = (words(a), words(b));
                (0..)
                    .zip(a.iter().zip(&b))
                    .find(|(_, (x, y))| x != y)
                    .map(|(w, _)| (block, w))
            });
        return not_run(format!(
            "the reference's outputs differ between two runs (storage block and word {at:?})"
        ));
    }
    let (_, route_descriptors) = compare::descriptors(&resources);
    let lavapipe = match Lavapipe::open() {
        Ok(l) => l,
        Err(e) => return not_run(e),
    };
    let job = Job {
        stage: lavapipe::Stage::Compute,
        words: &route,
        entry: &route_entry.name,
        descriptors: &route_descriptors,
        immediates: &[],
        groups: [COMPUTE_GROUPS, 1, 1],
        vertices: 0,
        attributes: &[],
        feeder: None,
        capture_stride: 0,
        targets: &[],
        depth: false,
        clear: [compare::CLEAR_BITS; 4],
    };
    let routed = match lavapipe.run(&job) {
        Ok(o) => o,
        Err(e) => return Verdict::Differs(format!("the route's module did not run: {e}")),
    };
    let mut found = Vec::new();
    storage_differences(
        &gl,
        &inputs,
        &storage_of,
        &first,
        &routed.storage,
        &mut found,
    );
    verdict(found)
}

/// The name a wrapped vertex program's own `main` takes, the attribute placing each point, and the
/// output keeping the program's position.
const PROGRAM_MAIN: &str = "wc_program_main";
const POINT_ATTRIBUTE: &str = "wc_point";
const POSITION_OUTPUT: &str = "wc_position";
/// The catcher's block, its array and its element type.
const CAUGHT_BLOCK: &str = "WcCaughtBlock";
const CAUGHT_ARRAY: &str = "wc_caught";
const CAUGHT_TYPE: &str = "WcCaught";
/// The feeder's attribute for one input element.
const FED_ATTRIBUTE: &str = "wc_fed";
/// The directive a GLSL source starts with, and the version of the feeders and catchers of
/// desktop programs: the first with no interpolation to match between stages.
pub const VERSION_DIRECTIVE: &str = "#version";
pub const ES_PROFILE: &str = "es";
/// The one GLSL ES version without the profile word: GLSL ES 1.00.
pub const ES_ONLY_VERSION: u32 = 100;
pub const EXTENSION_DIRECTIVE: &str = "#extension";
/// Every precision qualifier made `highp`, and the defaults that are not (`at_full_precision`).
const FULL_PRECISION: &str = "#define lowp highp\n#define mediump highp\nprecision highp float;\n\
                              precision highp int;\nprecision highp sampler2D;\n\
                              precision highp samplerCube;\n";
const DESKTOP_HEADER: &str = "#version 450 compatibility";
/// What OpenGL's own names start with.
const OPENGL_PREFIX: &str = "gl_";
/// Fragment outputs of OpenGL's own.
pub const FRAG_COLOR: &str = "gl_FragColor";
const FRAG_DATA: &str = "gl_FragData";
const FRAG_DEPTH: &str = "gl_FragDepth";
/// OpenGL's attribute that is generic attribute 0.
const VERTEX_ATTRIBUTE: &str = "gl_Vertex";
/// OpenGL's own attributes fed by its own arrays; a texture coordinate's name ends in its unit.
const COLOR_ATTRIBUTE: &str = "gl_Color";
const NORMAL_ATTRIBUTE: &str = "gl_Normal";
const FOG_COORD_ATTRIBUTE: &str = "gl_FogCoord";
const TEX_COORD_ATTRIBUTE: &str = "gl_MultiTexCoord";
/// Vertex inputs the draw gives, not attributes: the same on both sides (first vertex 0, one
/// instance).
const DRAW_VALUES: [&str; 3] = ["gl_VertexID", "gl_InstanceID", "gl_InstanceIDARB"];
/// OpenGL's fragment inputs and the vertex outputs that feed them.
const FED_BUILTINS: [(&str, &str); 4] = [
    ("gl_Color", "gl_FrontColor"),
    ("gl_SecondaryColor", "gl_FrontSecondaryColor"),
    ("gl_TexCoord", "gl_TexCoord"),
    ("gl_FogFragCoord", "gl_FogFragCoord"),
];
/// Keywords for stage values in every GLSL version, chosen by the preprocessor.
const STAGE_VALUE_KEYWORDS: &str = "#if __VERSION__ >= 130
#define WC_IN in
#define WC_OUT out
#define WC_FLAT flat
#else
#define WC_IN attribute
#define WC_OUT varying
#define WC_FLAT
#endif
";
/// What an ES fragment program's feeder needs and its own source has.
const ES_PRECISION: &str = "precision highp float;\nprecision highp int;\n";

/// A GLSL type's name, from OpenGL's reflection.
fn glsl_type_name(ty: u32) -> Option<&'static str> {
    Some(match ty {
        glow::FLOAT => "float",
        glow::FLOAT_VEC2 => "vec2",
        glow::FLOAT_VEC3 => "vec3",
        glow::FLOAT_VEC4 => "vec4",
        glow::INT => "int",
        glow::INT_VEC2 => "ivec2",
        glow::INT_VEC3 => "ivec3",
        glow::INT_VEC4 => "ivec4",
        glow::UNSIGNED_INT => "uint",
        glow::UNSIGNED_INT_VEC2 => "uvec2",
        glow::UNSIGNED_INT_VEC3 => "uvec3",
        glow::UNSIGNED_INT_VEC4 => "uvec4",
        glow::FLOAT_MAT2 => "mat2",
        glow::FLOAT_MAT3 => "mat3",
        glow::FLOAT_MAT4 => "mat4",
        glow::FLOAT_MAT2x3 => "mat2x3",
        glow::FLOAT_MAT2x4 => "mat2x4",
        glow::FLOAT_MAT3x2 => "mat3x2",
        glow::FLOAT_MAT3x4 => "mat3x4",
        glow::FLOAT_MAT4x2 => "mat4x2",
        glow::FLOAT_MAT4x3 => "mat4x3",
        _ => return None,
    })
}

/// A source's `#version` line, if it has one.
fn version_line(source: &str) -> Option<&str> {
    source
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with(VERSION_DIRECTIVE))
}

fn is_es(source: &str) -> bool {
    version_line(source).is_some_and(|l| {
        l.split_whitespace()
            .any(|w| w == ES_PROFILE || w.parse() == Ok(ES_ONLY_VERSION))
    })
}

/// An ES program with its precision qualifiers made `highp`, as every stage of the corpus takes
/// it: our driver computes `mediump` and `lowp` at full precision (the bridge lowers none), while
/// llvmpipe computes them in 16 bits (its `fp16` and `int16` capabilities); GLSL ES allows both,
/// so the reference is held to the precision the route gives.
pub fn at_full_precision(sources: Vec<(Stage, String)>) -> Vec<(Stage, String)> {
    sources
        .into_iter()
        .map(|(stage, source)| {
            if !is_es(&source) {
                return (stage, source);
            }
            // After the version and every extension directive, which come before declarations.
            let mut at = 0;
            let mut end = 0;
            for line in source.split_inclusive('\n') {
                end += line.len();
                let line = line.trim();
                if line.starts_with(VERSION_DIRECTIVE) || line.starts_with(EXTENSION_DIRECTIVE) {
                    at = end;
                }
            }
            let text = format!("{}\n{FULL_PRECISION}{}", &source[..at], &source[at..]);
            (stage, text)
        })
        .collect()
}

/// A vertex program wrapped in its own source: its `main` renamed by the preprocessor and called
/// by one that keeps its position and puts the point on the pixel `wc_point` names.
fn wrapped_vertex(source: &str) -> String {
    let rename = format!("#define main {PROGRAM_MAIN}\n");
    let mut text = match version_line(source) {
        Some(line) => {
            let at = source.find(line).expect("the line is in the source") + line.len();
            format!("{}\n{rename}{}", &source[..at], &source[at..])
        }
        None => format!("{rename}{source}"),
    };
    text += &format!(
        "\n#undef main\n{STAGE_VALUE_KEYWORDS}WC_IN vec4 {POINT_ATTRIBUTE};\n\
         WC_OUT vec4 {POSITION_OUTPUT};\n\
         void main()\n{{\n    {PROGRAM_MAIN}();\n    {POSITION_OUTPUT} = gl_Position;\n    \
         gl_Position = {POINT_ATTRIBUTE};\n    gl_PointSize = 1.0;\n}}\n"
    );
    text
}

/// A stage value's declaration: its type and array size, and whether it must be flat.
fn declared(v: &StageValue) -> Result<(&'static str, String, bool), String> {
    let ty = glsl_type_name(v.ty)
        .ok_or_else(|| format!("{} of type {:#x} is not set up here", v.name, v.ty))?;
    if v.name.contains('.') {
        return Err(format!(
            "{}: a structure or block stage value is not set up here",
            v.name
        ));
    }
    let array = if v.name.ends_with(FIRST_ELEMENT) {
        format!("[{}]", v.array_size)
    } else {
        String::new()
    };
    let flat = gl_type(v.ty).is_some_and(|t| t.kind != Kind::Float);
    Ok((ty, array, flat))
}

/// The catcher of a desktop vertex program's outputs: each written, as the fragment program
/// reads it, into the element of a storage block that is the point's pixel.
fn catcher(outputs: &[StageValue]) -> Result<String, String> {
    let mut declarations = String::new();
    let mut members = String::new();
    let mut stores = String::new();
    for v in outputs {
        if v.name.starts_with(OPENGL_PREFIX) {
            continue;
        }
        let (ty, array, flat) = declared(v)?;
        let name = bare(&v.name);
        let qualifier = if flat { "flat " } else { "" };
        declarations += &format!("{qualifier}in {ty} {name}{array};\n");
        members += &format!("    {ty} {name}{array};\n");
        stores += &format!("    {CAUGHT_ARRAY}[pixel].{name} = {name};\n");
    }
    Ok(format!(
        "{DESKTOP_HEADER}\n{declarations}struct {CAUGHT_TYPE} {{\n{members}}};\n\
         layout(std430, binding = 0) buffer {CAUGHT_BLOCK} {{ {CAUGHT_TYPE} {CAUGHT_ARRAY}[]; }};\n\
         void main()\n{{\n    int pixel = int(gl_FragCoord.x);\n{stores}}}\n"
    ))
}

/// The vertex program feeding a fragment program's inputs, one attribute an element, in the
/// fragment program's version family.
fn feeder_source(fragment: &str, inputs: &[StageValue]) -> Result<String, String> {
    let es = is_es(fragment);
    let header = if es {
        format!(
            "{}\n{ES_PRECISION}",
            version_line(fragment).expect("an ES source has a version")
        )
    } else {
        format!("{DESKTOP_HEADER}\n")
    };
    let mut declarations = String::new();
    let mut stores = String::new();
    for (k, v) in inputs.iter().enumerate() {
        let elements = if v.name.ends_with(FIRST_ELEMENT) {
            v.array_size
        } else {
            1
        };
        let name = bare(&v.name);
        let fed = FED_BUILTINS.iter().find(|(input, _)| *input == name);
        if name.starts_with(OPENGL_PREFIX) && fed.is_none() {
            // A value the rasteriser gives (gl_FragCoord, gl_FrontFacing, ...).
            continue;
        }
        let (ty, array, flat) = declared(v)?;
        let target = if let Some((_, output)) = fed {
            *output
        } else {
            let qualifier = if flat { "WC_FLAT " } else { "" };
            declarations += &format!("{qualifier}WC_OUT {ty} {name}{array};\n");
            name
        };
        for e in 0..elements {
            let attribute = format!("{FED_ATTRIBUTE}_{k}_{e}");
            declarations += &format!("WC_IN {ty} {attribute};\n");
            let place = if array.is_empty() {
                target.to_owned()
            } else {
                format!("{target}[{e}]")
            };
            stores += &format!("    {place} = {attribute};\n");
        }
    }
    Ok(format!(
        "{header}{STAGE_VALUE_KEYWORDS}WC_IN vec4 {POINT_ATTRIBUTE};\n{declarations}\
         void main()\n{{\n{stores}    gl_Position = {POINT_ATTRIBUTE};\n    gl_PointSize = 1.0;\n}}\n"
    ))
}

/// The program drawn for a GLSL program's vertex and fragment stages: the vertex program wrapped
/// (or written to feed the fragment program alone), and the fragment program (or a catcher of the
/// vertex program's outputs).
pub fn drawn_program(sources: &[(Stage, String)]) -> Result<Vec<(Stage, String)>, String> {
    let find = |stage| sources.iter().find(|(s, _)| *s == stage).map(|(_, t)| t);
    match (find(Stage::Vertex), find(Stage::Fragment)) {
        (Some(vertex), Some(fragment)) => Ok(vec![
            (Stage::Vertex, wrapped_vertex(vertex)),
            (Stage::Fragment, fragment.clone()),
        ]),
        (Some(vertex), None) => {
            if is_es(vertex) {
                return Err(
                    "an ES vertex program alone: its catcher would need storage blocks".into(),
                );
            }
            let wrapped = wrapped_vertex(vertex);
            let llvmpipe = Llvmpipe::open()?;
            let program = llvmpipe.link_separable(&[(Stage::Vertex, &wrapped)])?;
            let outputs = llvmpipe.reflect(program).outputs;
            Ok(vec![
                (Stage::Vertex, wrapped),
                (Stage::Fragment, catcher(&outputs)?),
            ])
        }
        (None, Some(fragment)) => {
            let llvmpipe = Llvmpipe::open()?;
            let program = llvmpipe.link_separable(&[(Stage::Fragment, fragment)])?;
            let inputs = llvmpipe.reflect(program).inputs;
            Ok(vec![
                (Stage::Vertex, feeder_source(fragment, &inputs)?),
                (Stage::Fragment, fragment.clone()),
            ])
        }
        (None, None) => Err("no vertex or fragment stage".into()),
    }
}

/// One stage of the drawn program as the route gave it.
pub struct RoutedStage<'a> {
    pub names: &'a [GlslName],
    pub module: &'a Module,
    pub reflection: &'a [ReflectedBinding],
}

fn name_at(names: &[GlslName], kind: GlslNameKind, name: &str) -> Option<u32> {
    names
        .iter()
        .find(|n| n.kind == kind && n.name == name)
        .map(|n| n.at)
}

/// Both sides' attributes: OpenGL's at its locations, the route's at the bridge's.
fn attributes(
    rng: &mut ChaCha8Rng,
    gl: &Reflection,
    names: &[GlslName],
    points: u32,
) -> Result<(Vec<VertexAttribute>, Vec<Attribute>), String> {
    let (mut ours, mut theirs) = (Vec::new(), Vec::new());
    for v in &gl.inputs {
        let name = bare(&v.name);
        if DRAW_VALUES.contains(&name) {
            continue;
        }
        let ty = gl_type(v.ty)
            .ok_or_else(|| format!("{name} of type {:#x} is not set up here", v.ty))?;
        let slot = match (v.location, name) {
            (at, _) if at >= 0 => AttributeSlot::Generic(at.cast_unsigned()),
            (_, VERTEX_ATTRIBUTE) => AttributeSlot::Generic(0),
            (_, COLOR_ATTRIBUTE) => AttributeSlot::Color,
            (_, NORMAL_ATTRIBUTE) => AttributeSlot::Normal,
            (_, FOG_COORD_ATTRIBUTE) => AttributeSlot::FogCoord,
            _ => match name
                .strip_prefix(TEX_COORD_ATTRIBUTE)
                .and_then(|unit| unit.parse().ok())
            {
                Some(unit) => AttributeSlot::TexCoord(unit),
                None => return Err(format!("OpenGL's attribute {name} is not set up here")),
            },
        };
        // A slot of an attribute the route's module does not read is fed to llvmpipe alone.
        let route = |slot: u32| {
            names
                .iter()
                .filter(|n| n.kind == GlslNameKind::Input && n.name == name)
                .find(|n| (n.element..n.element + n.count).contains(&slot))
                .map(|n| n.at + slot - n.element)
        };
        let elements = u32::try_from(v.array_size.max(1)).unwrap_or(1);
        for column in 0..elements * ty.columns {
            let words: Vec<u32> = if name == POINT_ATTRIBUTE {
                compare::positions(points)
                    .as_chunks::<{ size_of::<u32>() }>()
                    .0
                    .iter()
                    .map(|c| u32::from_le_bytes(*c))
                    .collect()
            } else {
                (0..points * ty.rows).map(|_| scalar(rng, ty)).collect()
            };
            let one = Ty {
                kind: ty.kind,
                components: ty.rows,
                count: 1,
            };
            if let Some(location) = route(column) {
                theirs.push(Attribute {
                    location,
                    format: compare::attribute_format(one),
                    stride: one.bytes(),
                    bytes: words.iter().flat_map(|w| w.to_le_bytes()).collect(),
                });
            }
            let slot = match slot {
                AttributeSlot::Generic(at) => AttributeSlot::Generic(at + column),
                conventional => conventional,
            };
            ours.push(VertexAttribute {
                slot,
                kind: ty.kind,
                components: ty.rows,
                words,
            });
        }
    }
    Ok((ours, theirs))
}

/// A colour target's kind and the components of each texel the program writes.
type Target = Option<(Kind, u32)>;

/// The colour target at each location, and whether depth is written; with the route's locations
/// checked against OpenGL's.
fn targets(gl: &Reflection, names: &[GlslName]) -> Result<(Vec<Target>, bool), String> {
    let mut at: Vec<Target> = Vec::new();
    let mut depth = false;
    let mut put = |location: u32, kind, components| {
        let i = location as usize;
        if at.len() <= i {
            at.resize(i + 1, None);
        }
        at[i] = Some((kind, components));
    };
    for v in &gl.outputs {
        let name = bare(&v.name);
        let elements = u32::try_from(v.array_size.max(1)).unwrap_or(1);
        let ty = gl_type(v.ty);
        match name {
            FRAG_DEPTH => depth = true,
            FRAG_COLOR | FRAG_DATA => {
                for e in 0..elements {
                    put(e, Kind::Float, ty.map_or(1, |t| t.rows));
                }
            }
            _ if name.starts_with(OPENGL_PREFIX) => {
                return Err(format!("OpenGL's output {name} is not set up here"));
            }
            _ => {
                let ty =
                    ty.ok_or_else(|| format!("{name} of type {:#x} is not set up here", v.ty))?;
                let location = u32::try_from(v.location)
                    .map_err(|_| format!("the output {name} has no location"))?;
                let route = name_at(names, GlslNameKind::Output, name);
                if route != Some(location) {
                    return Err(format!(
                        "the output {name} is at {location} on llvmpipe and {route:?} in the route"
                    ));
                }
                for e in 0..elements {
                    put(location + e, ty.kind, ty.rows);
                }
            }
        }
    }
    Ok((at, depth))
}

/// Holds the route's modules of a drawn GLSL program (`drawn_program`) to llvmpipe drawing it.
#[allow(clippy::too_many_lines, reason = "one comparison, step by step")]
pub fn compare_draw(
    sources: &[(Stage, String)],
    vertex: &RoutedStage,
    fragment: &RoutedStage,
    options: CompareOptions,
) -> Verdict {
    let not_run = Verdict::NotRun;
    let mut rng = ChaCha8Rng::seed_from_u64(options.seed);
    let points = options.input_sets;
    let write = |stage: &RoutedStage, naga_stage, flags| {
        let entry = &stage.module.entry_points[0].name;
        compare::write_spirv(stage.module, naga_stage, entry, flags).map(|w| (w, entry.clone()))
    };
    let spv = naga::back::spv::WriterFlags::ADJUST_COORDINATE_SPACE;
    let (vertex_words, vertex_entry) = match write(
        vertex,
        naga::ShaderStage::Vertex,
        spv | naga::back::spv::WriterFlags::FORCE_POINT_SIZE,
    ) {
        Ok(w) => w,
        Err(e) => return not_run(e),
    };
    let (fragment_words, fragment_entry) = match write(fragment, naga::ShaderStage::Fragment, spv) {
        Ok(w) => w,
        Err(e) => return not_run(e),
    };
    let llvmpipe = match Llvmpipe::open() {
        Ok(l) => l,
        Err(e) => return not_run(e),
    };
    let given: Vec<(Stage, &str)> = sources.iter().map(|(s, t)| (*s, t.as_str())).collect();
    let program = match llvmpipe.link(&given) {
        Ok(p) => p,
        Err(e) => return not_run(e),
    };
    let gl = llvmpipe.reflect(program);
    let mut inputs = match gl_inputs(&mut rng, &gl, &[vertex.names, fragment.names]) {
        Ok(i) => i,
        Err(e) => return not_run(e),
    };
    let (ours, theirs) = match attributes(&mut rng, &gl, vertex.names, points) {
        Ok(a) => a,
        Err(e) => return not_run(e),
    };
    let mut resources = Vec::new();
    let mut storage_of = Vec::new();
    for stage in [vertex, fragment] {
        match route_resources(
            &mut rng,
            stage.module,
            stage.reflection,
            stage.names,
            &gl,
            &mut inputs,
        ) {
            Ok((r, s)) => {
                resources.extend(r);
                storage_of.extend(s);
            }
            Err(e) => return not_run(e),
        }
    }
    let (target_kinds, depth) = match targets(&gl, fragment.names) {
        Ok(t) => t,
        Err(e) => return not_run(e),
    };
    let gl_targets: Vec<Option<Kind>> = target_kinds.iter().map(|t| t.map(|(k, _)| k)).collect();
    let (uniform_bytes, storage_bytes) = (inputs.uniform_bytes(), inputs.storage_bytes());
    let draw = DrawOptions {
        uniforms: &inputs.values,
        uniform_blocks: &uniform_bytes,
        storage_blocks: &storage_bytes,
        sampled: &inputs.sampled,
        attributes: &ours,
        points,
        targets: &gl_targets,
        depth,
        clear: [compare::CLEAR_BITS; 4],
    };
    let (first, again) = match (llvmpipe.draw(program, &draw), llvmpipe.draw(program, &draw)) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(e), _) | (_, Err(e)) => return not_run(e),
    };
    if first != again {
        return not_run("the reference's outputs differ between two runs".into());
    }
    let (_, route_descriptors) = compare::descriptors(&resources);
    let lavapipe = match Lavapipe::open() {
        Ok(l) => l,
        Err(e) => return not_run(e),
    };
    let route_targets: Vec<Option<vk::Format>> = target_kinds
        .iter()
        .map(|t| t.map(|(k, _)| compare::colour_format(k)))
        .collect();
    let job = Job {
        stage: lavapipe::Stage::Fragment,
        words: &fragment_words,
        entry: &fragment_entry,
        descriptors: &route_descriptors,
        immediates: &[],
        groups: [0; 3],
        vertices: points,
        attributes: &theirs,
        feeder: Some((&vertex_words, &vertex_entry)),
        capture_stride: 0,
        targets: &route_targets,
        depth,
        clear: [compare::CLEAR_BITS; 4],
    };
    let routed = match lavapipe.run(&job) {
        Ok(o) => o,
        Err(e) => return Verdict::Differs(format!("the route's modules did not run: {e}")),
    };
    let mut found = Vec::new();
    let per_texel = draw.clear.len();
    for (i, (location, (kind, components))) in target_kinds
        .iter()
        .enumerate()
        .filter_map(|(l, t)| t.map(|t| (l, t)))
        .enumerate()
    {
        compare::differences(
            &format!("target {location}"),
            &first.targets[i],
            &routed.targets[i],
            |w| (w % per_texel < components as usize).then_some(kind),
            &mut found,
        );
    }
    if depth {
        compare::differences(
            "depth",
            &first.depth,
            &routed.depth,
            |_| Some(Kind::Float),
            &mut found,
        );
    }
    storage_differences(
        &gl,
        &inputs,
        &storage_of,
        &first.storage,
        &routed.storage,
        &mut found,
    );
    verdict(found)
}

/// Piglit's shader test files, and the headers of their sections.
pub const SHADER_TEST_EXTENSION: &str = "shader_test";
const SECTION_START: char = '[';
const STAGE_SECTIONS: [(&str, Stage); 3] = [
    ("[vertex shader]", Stage::Vertex),
    ("[fragment shader]", Stage::Fragment),
    ("[compute shader]", Stage::Compute),
];
/// Piglit's own vertex program passing its position through: no stage of the file's.
const PASSTHROUGH_SECTION: &str = "[vertex shader passthrough]";
/// Stages the common form does not have, and OpenGL's assembly programs.
const ABSENT_SECTIONS: [&str; 6] = [
    "[geometry shader]",
    "[tessellation control shader]",
    "[tessellation evaluation shader]",
    "[vertex program]",
    "[fragment program]",
    "[geometry layout]",
];

/// The section of a file's requirements, its GLSL requirement (`GLSL >= 1.50`, `GLSL ES >= 3.00`)
/// and the ES version that names no profile.
const REQUIRE_SECTION: &str = "[require]";
const GLSL_REQUIREMENT: &str = "GLSL";
const ES_REQUIREMENT: &str = "ES";
const ES_WITHOUT_PROFILE: u32 = 100;

/// The `#version` line piglit's shader runner gives a source that has none: the file's GLSL
/// requirement's.
fn required_version(text: &str) -> Option<String> {
    let mut requiring = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with(SECTION_START) {
            requiring = line == REQUIRE_SECTION;
            continue;
        }
        let Some(rest) = line.strip_prefix(GLSL_REQUIREMENT).filter(|_| requiring) else {
            continue;
        };
        let words: Vec<&str> = rest.split_whitespace().collect();
        let es = words.first() == Some(&ES_REQUIREMENT);
        let number: u32 = words.last()?.replace('.', "").parse().ok()?;
        let profile = if es && number != ES_WITHOUT_PROFILE {
            format!(" {ES_PROFILE}")
        } else {
            String::new()
        };
        return Some(format!("{VERSION_DIRECTIVE} {number}{profile}\n"));
    }
    None
}

/// The GLSL stages of a piglit shader test file, by its sections; or why the routes do not take it.
/// A source without `#version` gets the file's required one, as piglit's shader runner gives it.
pub fn shader_test_sources(text: &str) -> Result<Vec<(Stage, String)>, String> {
    let mut sources: Vec<(Stage, String)> = Vec::new();
    let mut current: Option<usize> = None;
    let version = required_version(text);
    for line in text.lines() {
        let header = line.trim();
        if header.starts_with(SECTION_START) {
            current = None;
            if let Some(&(_, stage)) = STAGE_SECTIONS.iter().find(|(h, _)| *h == header) {
                if sources.iter().any(|(s, _)| *s == stage) {
                    return Err(format!("several sources for {stage:?}"));
                }
                current = Some(sources.len());
                sources.push((stage, String::new()));
            } else if ABSENT_SECTIONS.contains(&header) {
                return Err(format!("{header}: not in the common form"));
            } else if header == PASSTHROUGH_SECTION {
                current = None;
            }
            continue;
        }
        if let Some(i) = current {
            sources[i].1.push_str(line);
            sources[i].1.push('\n');
        }
    }
    if sources.is_empty() {
        return Err("no GLSL stage".into());
    }
    if sources.iter().any(|(s, _)| *s == Stage::Compute) && sources.len() > 1 {
        return Err("a compute stage beside others".into());
    }
    sources.sort_by_key(|(s, _)| Stage::ALL.iter().position(|a| a == s));
    if let Some(version) = version {
        for (_, source) in &mut sources {
            if version_line(source).is_none() {
                source.insert_str(0, &version);
            }
        }
    }
    Ok(sources)
}
