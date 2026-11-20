//! A GLSL program held to llvmpipe (plan 15.3): the program run by llvmpipe through OpenGL, and the
//! route's module, written to SPIR-V by naga and run by lavapipe, on the same seeded inputs. The
//! inputs follow OpenGL's own reflection of the program: each default-block uniform set with
//! `glUniform*` and written into uniform buffer 0 at the words the bridge names for it; each block
//! filled member by member by its type and bound, by its name, at the binding the bridge names.
//! Compute programs; their storage blocks compared, each word by its member's type.

use crate::compare::{
    self, COMPUTE_GROUPS, CompareOptions, NAMED_DIFFERENCES, RUNTIME_ELEMENTS, Verdict,
};
use crate::interface::{COMPONENT_BYTES, Kind};

/// The bytes of a word of a block.
const BYTES_A_WORD: usize = COMPONENT_BYTES as usize;
use crate::lavapipe::{self, Element, Job, Lavapipe};
use crate::llvmpipe::{Block, Llvmpipe, UniformValue, Variable};
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
        match name.kind {
            GlslNameKind::Uniform => {}
            GlslNameKind::FragCoordYTransform
            | GlslNameKind::PointCoordYTransform
            | GlslNameKind::OtherState => {
                return Err(format!(
                    "the state value {:?} is not filled here",
                    name.kind
                ));
            }
            _ => continue,
        }
        let Some(value) = values.iter().find(|v| bare(&v.variable.name) == name.name) else {
            continue;
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

/// Holds the route's module of a GLSL compute program to llvmpipe running the program.
#[allow(clippy::too_many_lines, reason = "one comparison, step by step")]
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

    let mut values = Vec::new();
    for v in gl.uniforms.iter().filter(|v| v.block < 0) {
        let ty = match typed(v) {
            Ok(t) => t,
            Err(e) => return not_run(e),
        };
        let count =
            usize::try_from(v.array_size.max(1)).unwrap_or(1) * (ty.columns * ty.rows) as usize;
        let words = (0..count).map(|_| scalar(&mut rng, ty)).collect();
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
    let (mut uniform_blocks, mut storage_blocks) = match (
        block_contents(&mut rng, &gl.uniform_blocks, &gl.uniforms),
        block_contents(&mut rng, &gl.storage_blocks, &gl.buffer_variables),
    ) {
        (Ok(u), Ok(s)) => (u, s),
        (Err(e), _) | (_, Err(e)) => return not_run(e),
    };

    // The route's resources, given the same bytes as the program's blocks, found by their names.
    let (mut resources, immediates) = match compare::resources(module, reflection, &mut rng) {
        Ok(r) => r,
        Err(e) => return not_run(e),
    };
    // Each storage resource's block index.
    let mut storage_of = Vec::new();
    for r in &mut resources {
        let binding = r.reference.1;
        let (kind, blocks, contents) = match r.kind {
            vk::DescriptorType::UNIFORM_BUFFER => (
                GlslNameKind::UniformBlock,
                &gl.uniform_blocks,
                &mut uniform_blocks,
            ),
            vk::DescriptorType::STORAGE_BUFFER => (
                GlslNameKind::StorageBlock,
                &gl.storage_blocks,
                &mut storage_blocks,
            ),
            other => return not_run(format!("a {other:?} is not set up for GLSL here")),
        };
        let [Element::Buffer(bytes)] = r.elements.as_mut_slice() else {
            return not_run("an array of blocks is not set up for GLSL here".into());
        };
        let Some(name) = named(names, kind, binding) else {
            if kind == GlslNameKind::UniformBlock {
                match default_block(bytes.len(), names, &values) {
                    Ok(b) => *bytes = b,
                    Err(e) => return not_run(e),
                }
                continue;
            }
            return not_run(format!(
                "no block of the program at storage binding {binding}"
            ));
        };
        let Some(index) = blocks.iter().position(|b| b.name == name.name) else {
            return not_run(format!("llvmpipe has no block {}", name.name));
        };
        let (gl_bytes, _) = &mut contents[index];
        let size = gl_bytes.len().max(bytes.len());
        gl_bytes.resize(size, 0);
        bytes.clone_from(gl_bytes);
        if kind == GlslNameKind::StorageBlock {
            storage_of.push(index);
        }
    }

    let gl_run = || {
        let uniforms: Vec<Vec<u8>> = uniform_blocks.iter().map(|b| b.0.clone()).collect();
        let storage: Vec<Vec<u8>> = storage_blocks.iter().map(|b| b.0.clone()).collect();
        llvmpipe.compute(
            program,
            &values,
            &uniforms,
            &storage,
            [COMPUTE_GROUPS, 1, 1],
        )
    };
    let (first, again) = match (gl_run(), gl_run()) {
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
    let job = Job {
        stage: lavapipe::Stage::Compute,
        words: &route,
        entry: &route_entry.name,
        descriptors: &route_descriptors,
        immediates: &immediates,
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
    for (&index, after) in storage_of.iter().zip(&routed.storage) {
        let kinds = &storage_blocks[index].1;
        compare::differences(
            &format!("storage block {}", gl.storage_blocks[index].name),
            &first[index],
            after,
            |at| kinds.get(at).copied().flatten(),
            &mut found,
        );
    }
    if found.is_empty() {
        Verdict::Matched
    } else {
        let count = found.len();
        found.truncate(NAMED_DIFFERENCES);
        Verdict::Differs(format!("{count} values differ: {}", found.join("; ")))
    }
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

/// The GLSL stages of a piglit shader test file, by its sections; or why the routes do not take it.
pub fn shader_test_sources(text: &str) -> Result<Vec<(Stage, String)>, String> {
    let mut sources: Vec<(Stage, String)> = Vec::new();
    let mut current: Option<usize> = None;
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
    Ok(sources)
}
