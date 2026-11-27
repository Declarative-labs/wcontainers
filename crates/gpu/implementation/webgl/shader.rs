//! Shaders written as GLSL ES 3.00 by naga, and programs linked once per pair of them.

use crate::platform;
use naga::back::glsl;
use std::rc::Rc;
use wcontainers_gpu::{caps::Features, error::GpuError, shader::ShaderModule, shader::ShaderStage};
use wcontainers_opengl_es::OpenGlEs;
use wcontainers_opengl_es::constants as gles;

/// A (set, binding) pair: where a program reads a resource.
pub(crate) type Slot = (u32, u32);

/// A part of the immediates a program reads as a plain uniform (WebGL 2 has no immediates): its
/// GLSL name, its offset in the immediates and its type.
pub(crate) type Immediate = (String, u32, glsl::GlslUniformType);

/// A compiled shader. Shaders of equal text share one, so their programs share one GL program.
#[derive(Debug)]
pub struct ShaderObj {
    pub id: u64,
    pub raw: u32,
    /// How many handles name it.
    pub handles: std::cell::Cell<u32>,
    pub text: Rc<ShaderText>,
}

impl core::ops::Deref for ShaderObj {
    type Target = ShaderText;

    fn deref(&self) -> &ShaderText {
        &self.text
    }
}

/// A shader's GLSL and what its program must bind.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ShaderText {
    pub stage: ShaderStage,
    /// Combined samplers by GLSL name.
    pub textures: Vec<Combined>,
    /// Uniform blocks by GLSL name.
    pub blocks: Vec<(String, Slot)>,
    /// The GLSL, for the log of a program that fails to build and for capture programs.
    pub source: String,
    /// A vertex shader's location outputs in location order: GLSL name and bytes.
    pub outputs: Vec<(String, u32)>,
    pub immediates: Vec<Immediate>,
    /// How many clip distances a vertex shader writes.
    pub clip_distances: u8,
}

/// A combined sampler of a shader: its GLSL name, the texture's slot and its sampler's, and
/// whether gathers read it, so its unit takes the sampler's twin whose minimum level of detail is
/// 0 (see [`gathers_at_level_zero`]).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Combined {
    pub name: String,
    pub texture: Slot,
    pub sampler: Option<Slot>,
    pub level_zero: bool,
}

/// A vertex shader and an optional fragment shader.
pub(crate) type Pair = (Rc<ShaderObj>, Option<Rc<ShaderObj>>);

/// A program and how its resources reach GL. Linking runs from creation; what the link decides is
/// read at the first draw, so links overlap other work.
#[derive(Debug)]
pub struct ProgramObj {
    pub id: u64,
    pub raw: u32,
    /// The shaders it links, until the link is read.
    pub shaders: std::cell::RefCell<Option<Pair>>,
    /// The shaders it was made from, whose sources capture programs compile again.
    pub stages: Pair,
    /// Its capture programs, made at the first capture of each kind (see [`Capture::slot`]).
    pub captures: [std::cell::OnceCell<Rc<ProgramObj>>; 4],
    pub linked: std::cell::OnceCell<Linked>,
    /// The (vertex, fragment) pair it was linked from, and how many handles name it.
    pub pair: (u64, u64),
    pub handles: std::cell::Cell<u32>,
    /// How it captures, for a capture program.
    pub capture: Option<Capture>,
    /// Its variants for vertex layouts GL ES cannot read as stored, by [`GlLayout::converted`],
    /// made at the first draw with each.
    ///
    /// [`GlLayout::converted`]: crate::state::GlLayout::converted
    pub variants: std::cell::RefCell<Vec<(u32, Rc<ProgramObj>)>>,
}

/// What a finished link decided.
#[derive(Debug)]
pub struct Linked {
    /// Texture unit n samples the texture in slot `.0` through the sampler in slot `.1`, or its
    /// twin whose minimum level of detail is 0 where `.2` is set.
    pub units: Vec<(Slot, Option<Slot>, bool)>,
    /// Unit n's hidden size uniform (see [`sizes_from_uniforms`]), with the value it holds.
    pub sizes: Vec<Option<(i32, std::cell::Cell<[i32; 3]>)>>,
    /// Uniform block binding point n reads the uniform buffer in this slot.
    pub blocks: Vec<Slot>,
    /// naga's `naga_vs_first_instance` (added to `gl_InstanceID`) and `naga_vs_base_vertex` (added to
    /// `gl_VertexID`), each with the value it holds.
    pub first_instance: Option<(i32, std::cell::Cell<u32>)>,
    pub base_vertex: Option<(i32, std::cell::Cell<u32>)>,
    /// Each part of the immediates the program reads: its uniform, offset and type.
    pub immediates: Vec<(i32, u32, glsl::GlslUniformType)>,
    /// The immediates its uniforms hold.
    pub held: std::cell::RefCell<Vec<u8>>,
}

/// What the context's GLSL ES 3.00 takes beyond the core language, and how the picture is turned.
#[derive(Clone, Copy, Debug, Default)]
pub struct GlslSettings {
    /// Without EXT_clip_control the last vertex stage negates y as its last step, so row 0 is the
    /// top of the picture on both origin routes.
    pub flip_y: bool,
    /// NV_shader_noperspective_interpolation's `noperspective`, which linear values take where the
    /// context has it; elsewhere they are carried times w ([`linear_by_w`]).
    pub noperspective: bool,
}

/// Clip and cull distances together: what WEBGL_clip_cull_distance guarantees, Direct3D's limit.
pub(crate) const CLIP_AND_CULL_DISTANCES: u32 = 8;

/// The directive of NV_shader_noperspective_interpolation.
pub(crate) const NOPERSPECTIVE_EXTENSION: &str = "GL_NV_shader_noperspective_interpolation";

/// The directive of OES_shader_multisample_interpolation, which gives the `sample` qualifier.
pub(crate) const SAMPLE_QUALIFIER_EXTENSION: &str = "GL_OES_shader_multisample_interpolation";

/// The combined samplers whose units take a sampler's level-zero twin, each with the name whose
/// slots it binds (see [`gathers_at_level_zero`]).
pub(crate) type LevelZero = Vec<(String, String)>;

/// Writes `module` as GLSL ES 3.00.
pub(crate) fn write_glsl(
    module: &ShaderModule,
    settings: GlslSettings,
) -> Result<(String, glsl::ReflectionInfo, LevelZero), String> {
    let stage = match module.stage {
        ShaderStage::Vertex => naga::ShaderStage::Vertex,
        ShaderStage::Fragment => naga::ShaderStage::Fragment,
        ShaderStage::Compute => return Err("compute shaders are absent on WebGL 2".into()),
    };
    let options = glsl::Options {
        version: glsl::Version::Embedded {
            version: 300,
            is_webgl: true,
        },
        // GLES leaves the size of points a vertex program does not write undefined.
        writer_flags: glsl::WriterFlags::FORCE_POINT_SIZE,
        binding_map: Default::default(),
        zero_initialize_workgroup_memory: false,
    };
    let pipeline = glsl::PipelineOptions {
        shader_stage: stage,
        entry_point: module.entry_point.clone(),
        multiview: None,
    };
    let qualified = qualified_values(module);
    let plain;
    let module = if qualified.is_empty() {
        module
    } else {
        plain = qualifiers_removed(module)?;
        &plain
    };
    let mut out = String::new();
    let mut writer = glsl::Writer::new(
        &mut out,
        &module.module,
        &module.info,
        &options,
        &pipeline,
        naga::proc::BoundsCheckPolicies::default(),
    )
    .map_err(|e| e.to_string())?;
    let reflection = writer.write().map_err(|e| e.to_string())?;
    if settings.flip_y && stage == naga::ShaderStage::Vertex {
        // naga writes each `gl_Position = ...;` on a line of its own, just before the entry returns.
        let mut flipped = String::with_capacity(out.len() + 64);
        for line in out.lines() {
            flipped.push_str(line);
            flipped.push('\n');
            let trimmed = line.trim_start();
            if trimmed.starts_with("gl_Position = ") {
                flipped.push_str(&line[..line.len() - trimmed.len()]);
                flipped.push_str("gl_Position.y = -gl_Position.y;\n");
            }
        }
        out = flipped;
    }
    let out = qualifiers_written(out, &qualified, settings.noperspective);
    let carried: Vec<Qualified> = if settings.noperspective {
        Vec::new()
    } else {
        qualified.into_iter().filter(|q| q.linear).collect()
    };
    let out = opaque_float_bits(linear_by_w(out, stage, &carried));
    let out = single_sample_masks(out);
    let (out, level_zero) = gathers_at_level_zero(gathers_by_samples(bit_functions(out)));
    Ok((out, reflection, level_zero))
}

/// How naga's GLSL writer starts the line writing the sample mask.
const SAMPLE_MASK_WRITE: &str = "gl_SampleMask[0] = ";

/// GL ignores the sample mask a program writes into a single-sample target, where Direct3D and
/// WebGPU drop a fragment whose mask clears sample 0: so after the mask is written, a fragment
/// drawn into one sample (`gl_NumSamples`, OES_sample_variables) whose mask clears it is discarded.
fn single_sample_masks(source: String) -> String {
    if !source.contains(SAMPLE_MASK_WRITE) {
        return source;
    }
    let mut out = String::with_capacity(source.len() + SAMPLE_MASK_WRITE.len());
    for line in source.lines() {
        out.push_str(line);
        out.push(LINE_END);
        let trimmed = line.trim_start();
        if let Some(mask) = trimmed
            .strip_prefix(SAMPLE_MASK_WRITE)
            .and_then(|m| m.strip_suffix(STATEMENT_END))
        {
            let indent = &line[..line.len() - trimmed.len()];
            out.push_str(&format!(
                "{indent}if (gl_NumSamples == 1 && (({mask}) & 1) == 0) {{ discard; }}\n"
            ));
        }
    }
    out
}

/// What [`gathers_by_samples`] makes of each gather's first argument, before its name.
const GATHER_CALLS: &[&str] = &["_wc_gather(", "_wc_gather_offset("];

/// The name of the second sampler uniform of a texture both gathered and read otherwise, after the
/// first's.
const GATHERED: &str = "_wc_gathered";

/// Gathers sample level 0 of the view by `textureLod`, which GL clamps to the sampler's minimum
/// level of detail, while Direct3D and WebGPU gather from level 0 whatever it is. So each combined
/// sampler gathers read is bound with the sampler's twin whose minimum is 0: the uniform itself
/// where only gathers read it, else a second uniform (and unit) of its own that the gathers read.
/// Returns the names whose units take the twin, each with the name whose slots it binds.
fn gathers_at_level_zero(source: String) -> (String, LevelZero) {
    let ident = |c: char| c.is_ascii_alphanumeric() || c == NAME_PUNCTUATION;
    let mut names: Vec<String> = Vec::new();
    for call in GATHER_CALLS {
        for (at, _) in source.match_indices(call) {
            let rest = &source[at + call.len()..];
            let name = &rest[..rest.find(ARGUMENT_GAP).unwrap_or(0)];
            // The calls gathers_by_samples makes size their first argument right after it.
            if rest[name.len()..].starts_with(&format!(", textureSize({name}, 0)"))
                && !names.iter().any(|n| n == name)
            {
                names.push(name.to_string());
            }
        }
    }
    let mut out = source;
    let mut level_zero = Vec::new();
    for name in names {
        let declaration = |l: &str| l.starts_with(UNIFORM) && l.ends_with(&format!(" {name};"));
        // Whether anything but its declaration and the gathers names it.
        let mut rest = out.clone();
        for call in GATHER_CALLS {
            rest = rest.replace(&format!("{call}{name}, textureSize({name}, 0)"), "");
        }
        let elsewhere = rest.lines().filter(|l| !declaration(l)).any(|l| {
            l.match_indices(name.as_str()).any(|(at, _)| {
                !l[..at].ends_with(ident) && !l[at + name.len()..].starts_with(ident)
            })
        });
        if !elsewhere {
            level_zero.push((name.clone(), name));
            continue;
        }
        let alias = format!("{name}{GATHERED}");
        for call in GATHER_CALLS {
            out = out.replace(&format!("{call}{name}, "), &format!("{call}{alias}, "));
        }
        let mut declared = String::with_capacity(out.len() + alias.len());
        for line in out.lines() {
            declared.push_str(line);
            declared.push(LINE_END);
            if declaration(line) {
                declared.push_str(&line.replacen(&format!(" {name};"), &format!(" {alias};"), 1));
                declared.push(LINE_END);
            }
        }
        out = declared;
        level_zero.push((alias, name));
    }
    (out, level_zero)
}

/// The uniform [`opaque_float_bits`] reads its bits through: never set, so 0 as GL leaves every
/// uniform after a link.
const ZERO: &str = "_wc_zero";

/// Floats made from bits and negative zero literals, each given its bits through an exclusive or
/// with a uniform zero, outside global constants (whose initialisers must be constant). ANGLE folds
/// such constants and writes them as decimal literals into HLSL, where negative zero loses its
/// sign and, in mozangle's build, a vector of them can turn into infinities; values it cannot fold
/// keep their bits. Costs one integer operation per such call.
fn opaque_float_bits(source: String) -> String {
    const FROM_UINT: &str = "uintBitsToFloat(";
    const FROM_INT: &str = "intBitsToFloat(";
    const NEGATIVE_ZERO: &str = "-0.0";
    const OPAQUE_UINT: &str = "_wc_from_bits(";
    const OPAQUE_INT: &str = "_wc_from_int_bits(";
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let number = |c: char| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E');
    let mut out = String::with_capacity(source.len());
    let mut changed = false;
    for line in source.lines() {
        if line.starts_with("const ") {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        let mut rest = line;
        while let Some(c) = rest.chars().next() {
            let free = !out.ends_with(ident);
            let swap = if !free {
                None
            } else if let Some(after) = rest.strip_prefix(FROM_UINT) {
                Some((OPAQUE_UINT.to_string(), after))
            } else if let Some(after) = rest.strip_prefix(FROM_INT) {
                Some((OPAQUE_INT.to_string(), after))
            } else if let Some(after) = rest.strip_prefix(NEGATIVE_ZERO)
                && !after.starts_with(number)
                && !out.ends_with(number)
            {
                Some((format!("{OPAQUE_UINT}{:#x}u)", (-0f32).to_bits()), after))
            } else {
                None
            };
            match swap {
                Some((with, after)) => {
                    out.push_str(&with);
                    rest = after;
                    changed = true;
                }
                None => {
                    out.push(c);
                    rest = &rest[c.len_utf8()..];
                }
            }
        }
        out.push('\n');
    }
    if !changed {
        return source;
    }
    let mut helpers = format!("uniform highp uint {ZERO};\n");
    for (float, uint, int) in [
        ("float", "uint", "int"),
        ("vec2", "uvec2", "ivec2"),
        ("vec3", "uvec3", "ivec3"),
        ("vec4", "uvec4", "ivec4"),
    ] {
        helpers.push_str(&format!(
            "{float} {OPAQUE_UINT}{uint} v) {{ return {FROM_UINT}v ^ {ZERO}); }}\n\
             {float} {OPAQUE_INT}{int} v) {{ return {FROM_INT}v ^ int({ZERO})); }}\n"
        ));
    }
    after_header(&out, &helpers)
}

/// The name naga's GLSL writer gives a value from the vertex to the fragment stage, before its
/// location.
const VARYING: &str = "_vs2fs_location";

/// The line naga's GLSL writer opens the entry point with.
const MAIN: &str = "void main() {";

/// How naga's GLSL writer ends each return from the entry point, on a line of its own.
const RETURN: &str = "return;";

/// The varying carrying a linear value at `location` perspective-correct (see [`linear_by_w`]).
const CARRIED: &str = "_wc_linear";

/// The varying carrying the vertex's w, before the qualifier it is sampled with, for linear values
/// sampled at the centroid or per sample.
const CARRIED_W: &str = "_wc_linear_w_";

/// GLSL's qualifiers of values between the stages.
const SMOOTH: &str = "smooth";
const NOPERSPECTIVE: &str = "noperspective";
const IN: &str = "in";
const OUT: &str = "out";

/// GLSL's auxiliary qualifier of each place a value is sampled other than the centre.
const AUXILIARY: [(naga::Sampling, &str); 2] = [
    (naga::Sampling::Centroid, "centroid"),
    (naga::Sampling::Sample, "sample"),
];

/// What separates the words of a declaration naga writes.
const WORD_GAP: &str = " ";

/// What ends each line of GLSL.
const LINE_END: char = '\n';

/// What ends a GLSL statement.
const STATEMENT_END: char = ';';

/// What separates the arguments of a call.
const ARGUMENT_GAP: char = ',';

/// What GLSL takes in names besides letters and digits.
const NAME_PUNCTUATION: char = '_';

/// How a sampler uniform's declaration starts.
const UNIFORM: &str = "uniform ";

/// Why a module's entry point cannot be rewritten: it is not there.
const NO_ENTRY_POINT: &str = "no such entry point";

/// Why a module's linear or per-sample values cannot be made plain: their struct exists plainly
/// too, and naga's arena keeps one of each type.
const PLAIN_STRUCT_EXISTS: &str = "its struct of linear or per-sample values exists plainly too";

/// A value between the stages that naga cannot qualify in GLSL ES 3.00 as the module does: linear
/// (GLSL ES 3.00 has no `noperspective`), or sampled per sample (`sample` is GLSL ES 3.20's).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Qualified {
    location: u32,
    linear: bool,
    sampling: Option<naga::Sampling>,
}

impl Qualified {
    /// The auxiliary qualifier it is sampled with.
    fn auxiliary(self) -> Option<&'static str> {
        AUXILIARY
            .iter()
            .find(|(s, _)| Some(*s) == self.sampling)
            .map(|(_, q)| *q)
    }

    fn per_sample(self) -> bool {
        self.sampling == Some(naga::Sampling::Sample)
    }
}

/// The values the stage writes (vertex) or reads (fragment) that are linear or sampled per sample.
fn qualified_values(module: &ShaderModule) -> Vec<Qualified> {
    let bindings: Vec<naga::Binding> = match module.stage {
        ShaderStage::Vertex => results(module).map(|(b, _)| b).collect(),
        _ => arguments(module).collect(),
    };
    bindings
        .into_iter()
        .filter_map(|b| match b {
            naga::Binding::Location {
                location,
                interpolation,
                sampling,
                ..
            } => Some(Qualified {
                location,
                linear: interpolation == Some(naga::Interpolation::Linear),
                sampling,
            })
            .filter(|q| q.linear || q.per_sample()),
            _ => None,
        })
        .collect()
}

/// `module` with its entry point's linear values made perspective-correct and its values sampled
/// per sample sampled at the centre, so naga writes them; [`qualifiers_written`] and
/// [`linear_by_w`] then qualify them in GLSL.
fn qualifiers_removed(module: &ShaderModule) -> Result<ShaderModule, String> {
    let plain = |b: &mut Option<naga::Binding>| {
        if let Some(naga::Binding::Location {
            interpolation,
            sampling,
            ..
        }) = b
        {
            if *interpolation == Some(naga::Interpolation::Linear) {
                *interpolation = Some(naga::Interpolation::Perspective);
            }
            if *sampling == Some(naga::Sampling::Sample) {
                *sampling = Some(naga::Sampling::Center);
            }
        }
    };
    let mut out = module.module.clone();
    let Some(entry) = out
        .entry_points
        .iter_mut()
        .find(|e| e.name == module.entry_point)
    else {
        return Err(NO_ENTRY_POINT.into());
    };
    let mut structs = Vec::new();
    let mut values: Vec<(&mut Option<naga::Binding>, naga::Handle<naga::Type>)> = entry
        .function
        .arguments
        .iter_mut()
        .map(|a| (&mut a.binding, a.ty))
        .collect();
    values.extend(
        entry
            .function
            .result
            .as_mut()
            .map(|r| (&mut r.binding, r.ty)),
    );
    for (binding, ty) in values {
        match binding {
            Some(_) => plain(binding),
            None => structs.push(ty),
        }
    }
    for ty in structs {
        let mut t = out.types[ty].clone();
        if let naga::TypeInner::Struct { members, .. } = &mut t.inner {
            for m in members {
                plain(&mut m.binding);
            }
        }
        if t != out.types[ty] {
            if out.types.get(&t).is_some() {
                return Err(PLAIN_STRUCT_EXISTS.into());
            }
            out.types.replace(ty, t);
        }
    }
    Ok(ShaderModule {
        module: out,
        info: module.info.clone(),
        entry_point: module.entry_point.clone(),
        stage: module.stage,
        key: module.key,
    })
}

/// The declarations of `qualified` values given back what [`qualifiers_removed`] took: `sample`
/// under OES_shader_multisample_interpolation, and with `noperspective` (where the context has
/// NV_shader_noperspective_interpolation) `noperspective` in place of `smooth`.
fn qualifiers_written(source: String, qualified: &[Qualified], noperspective: bool) -> String {
    let per_sample = qualified.iter().any(|q| q.per_sample());
    let linear = noperspective && qualified.iter().any(|q| q.linear);
    if !per_sample && !linear {
        return source;
    }
    let mut out = String::with_capacity(source.len() * 2);
    let mut in_main = false;
    for (i, line) in source.lines().enumerate() {
        in_main |= line == MAIN;
        let declared = qualified
            .iter()
            .find(|q| line.ends_with(&format!(" {VARYING}{};", q.location)))
            .filter(|_| !in_main);
        match declared {
            Some(q) => {
                let mut words: Vec<&str> = line.split(WORD_GAP).collect();
                if let Some(at) = words.iter().position(|w| *w == IN || *w == OUT) {
                    if q.per_sample() {
                        words.extend(q.auxiliary());
                        words[at..].rotate_right(1);
                    }
                    if q.linear && noperspective {
                        for w in &mut words[..at] {
                            if *w == SMOOTH {
                                *w = NOPERSPECTIVE;
                            }
                        }
                    }
                }
                out.push_str(&words.join(WORD_GAP));
            }
            None => out.push_str(line),
        }
        out.push(LINE_END);
        // The directives come right after the version.
        if i == 0 {
            for (wanted, extension) in [
                (per_sample, SAMPLE_QUALIFIER_EXTENSION),
                (linear, NOPERSPECTIVE_EXTENSION),
            ] {
                if wanted {
                    out.push_str(&format!("#extension {extension} : require\n"));
                }
            }
        }
    }
    out
}

/// Carries the linear values `linear` through GLSL ES 3.00 without `noperspective`: the vertex
/// stage writes each one times its w into a perspective-correct varying of its own (keeping the
/// value itself for transform feedback), and the fragment stage divides by the w interpolated at
/// the same place: `gl_FragCoord.w` (1 / w) at the centre, the vertex's w carried beside it at the
/// centroid or per sample. A perspective-correct a·w over w is Σbᵢaᵢ, the value interpolated in
/// window space, exactly; the same holds where clipping made the vertex.
fn linear_by_w(source: String, stage: naga::ShaderStage, linear: &[Qualified]) -> String {
    if linear.is_empty() {
        return source;
    }
    let vertex = stage == naga::ShaderStage::Vertex;
    let mut auxiliaries: Vec<&str> = linear.iter().filter_map(|q| q.auxiliary()).collect();
    auxiliaries.sort();
    auxiliaries.dedup();
    let names: Vec<(String, String, Option<&str>)> = linear
        .iter()
        .map(|q| {
            (
                format!("{VARYING}{}", q.location),
                format!("{CARRIED}{}", q.location),
                q.auxiliary(),
            )
        })
        .collect();
    let direction = if vertex { OUT } else { IN };
    let mut out = String::with_capacity(source.len() * 2);
    let mut in_main = false;
    let mut declared_w = false;
    for line in source.lines() {
        if !in_main {
            in_main = line == MAIN;
            out.push_str(line);
            out.push('\n');
            // Each declaration of a linear value is followed by its carrier's.
            if let Some((name, carried, _)) = names
                .iter()
                .find(|(n, _, _)| line.ends_with(&format!(" {n};")))
            {
                let carrier = line.replacen(name.as_str(), carried, 1);
                if vertex {
                    out.push_str(&carrier);
                    out.push('\n');
                } else {
                    // The fragment stage reads only the carrier.
                    out.truncate(out.len() - line.len() - 1);
                    out.push_str(&carrier);
                    out.push('\n');
                }
                if !declared_w {
                    for a in &auxiliaries {
                        out.push_str(&format!("{a} {direction} float {CARRIED_W}{a};\n"));
                    }
                    declared_w = true;
                }
            }
            continue;
        }
        if vertex {
            if line.trim_start() == RETURN {
                let indent = &line[..line.len() - RETURN.len()];
                for (name, carried, _) in &names {
                    out.push_str(&format!("{indent}{carried} = {name} * gl_Position.w;\n"));
                }
                for a in &auxiliaries {
                    out.push_str(&format!("{indent}{CARRIED_W}{a} = gl_Position.w;\n"));
                }
            }
            out.push_str(line);
        } else {
            let ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
            let mut rest = line;
            'scan: while !rest.is_empty() {
                for (name, carried, auxiliary) in &names {
                    if let Some(after) = rest.strip_prefix(name.as_str())
                        && !after.starts_with(ident)
                        && !out.ends_with(ident)
                    {
                        out.push_str(&match auxiliary {
                            Some(a) => format!("({carried} / {CARRIED_W}{a})"),
                            None => format!("({carried} * gl_FragCoord.w)"),
                        });
                        rest = after;
                        continue 'scan;
                    }
                }
                let next = rest.chars().next().map_or(0, char::len_utf8);
                out.push_str(&rest[..next]);
                rest = &rest[next..];
            }
        }
        out.push('\n');
    }
    out
}

/// GLSL ES 3.10's bit functions as naga writes them, defined for GLSL ES 3.00, which lacks them
/// (and so leaves their names free): for `int` and `uint` and their vectors, with offsets and
/// counts already cut to the width. Added after the header where the source calls any of them.
fn bit_functions(source: String) -> String {
    const NAMES: [&str; 6] = [
        "bitfieldExtract(",
        "bitfieldInsert(",
        "bitCount(",
        "bitfieldReverse(",
        "findLSB(",
        "findMSB(",
    ];
    if !NAMES.iter().any(|n| source.contains(n)) {
        return source;
    }
    let mut f = String::from(
        "highp uint _wc_ones(int n) { return n >= 32 ? 0xffffffffu : (1u << uint(n)) - 1u; }
         highp uint bitfieldExtract(highp uint v, int o, int n) { return n == 0 ? 0u : (v >> uint(o)) & _wc_ones(n); }
         highp int bitfieldExtract(highp int v, int o, int n) { return n == 0 ? 0 : (v << (32 - o - n)) >> (32 - n); }
         highp uint bitfieldInsert(highp uint b, highp uint i, int o, int n) { highp uint m = n == 0 ? 0u : _wc_ones(n) << uint(o); return (b & ~m) | ((i << uint(min(o, 31))) & m); }
         highp int bitfieldInsert(highp int b, highp int i, int o, int n) { return int(bitfieldInsert(uint(b), uint(i), o, n)); }
         highp int bitCount(highp uint v) { v = v - ((v >> 1u) & 0x55555555u); v = (v & 0x33333333u) + ((v >> 2u) & 0x33333333u); return int((((v + (v >> 4u)) & 0x0f0f0f0fu) * 0x01010101u) >> 24u); }
         highp int bitCount(highp int v) { return bitCount(uint(v)); }
         highp uint bitfieldReverse(highp uint v) { v = ((v >> 1u) & 0x55555555u) | ((v & 0x55555555u) << 1u); v = ((v >> 2u) & 0x33333333u) | ((v & 0x33333333u) << 2u); v = ((v >> 4u) & 0x0f0f0f0fu) | ((v & 0x0f0f0f0fu) << 4u); v = ((v >> 8u) & 0x00ff00ffu) | ((v & 0x00ff00ffu) << 8u); return (v >> 16u) | (v << 16u); }
         highp int bitfieldReverse(highp int v) { return int(bitfieldReverse(uint(v))); }
         highp int findLSB(highp uint v) { return v == 0u ? -1 : bitCount((v & (0u - v)) - 1u); }
         highp int findLSB(highp int v) { return findLSB(uint(v)); }
         highp int findMSB(highp uint v) { v |= v >> 1u; v |= v >> 2u; v |= v >> 4u; v |= v >> 8u; v |= v >> 16u; return bitCount(v) - 1; }
         highp int findMSB(highp int v) { return findMSB(uint(v < 0 ? ~v : v)); }
",
    );
    // Vectors, component by component.
    for n in 2..=4 {
        let c = ["x", "y", "z", "w"][..n].to_vec();
        let each = |call: &dyn Fn(&str) -> String| {
            c.iter().map(|c| call(c)).collect::<Vec<_>>().join(", ")
        };
        for v in ["ivec", "uvec"] {
            let t = format!("{v}{n}");
            f.push_str(&format!(
                "highp {t} bitfieldExtract(highp {t} v, int o, int n) {{ return {t}({}); }}
",
                each(&|c| format!("bitfieldExtract(v.{c}, o, n)"))
            ));
            f.push_str(&format!(
                "highp {t} bitfieldInsert(highp {t} b, highp {t} i, int o, int n) {{ return {t}({}); }}
",
                each(&|c| format!("bitfieldInsert(b.{c}, i.{c}, o, n)"))
            ));
            f.push_str(&format!(
                "highp {t} bitfieldReverse(highp {t} v) {{ return {t}({}); }}
",
                each(&|c| format!("bitfieldReverse(v.{c})"))
            ));
            for name in ["bitCount", "findLSB", "findMSB"] {
                f.push_str(&format!(
                    "highp ivec{n} {name}(highp {t} v) {{ return ivec{n}({}); }}
",
                    each(&|c| format!("{name}(v.{c})"))
                ));
            }
        }
    }
    after_header(&source, &f)
}

/// `source` with `text` added after the version, extensions and default precisions, which come
/// first.
fn after_header(source: &str, text: &str) -> String {
    let mut out = String::with_capacity(source.len() + text.len());
    let mut added = false;
    for line in source.lines() {
        let header = line.starts_with('#') || line.starts_with("precision ") || line.is_empty();
        if !added && !header {
            out.push_str(text);
            added = true;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// GLSL ES 3.10's `textureGather` and `textureGatherOffset`, which GLSL ES 3.00 lacks, as four
/// samples at the centres of the texels of the gather's footprint, in its order, at level 0: each
/// call is rewritten to a helper also given `textureSize(texture, 0)`, which the back end answers
/// from the bound view. At a texel's centre a sample through the bound sampler is that texel's
/// value (or comparison) whatever its filter, and its address mode places footprints past an edge
/// as gathers do; cube footprints are sampled by direction, so they cross faces as seamless
/// sampling does.
fn gathers_by_samples(source: String) -> String {
    const GATHER: &str = "textureGather";
    if !source.contains(GATHER) {
        return source;
    }
    let mut out = String::with_capacity(source.len() + 64);
    let mut rest = &source[..];
    while let Some(at) = rest.find(GATHER) {
        out.push_str(&rest[..at]);
        let after = &rest[at + GATHER.len()..];
        let (helper, args) = if let Some(a) = after.strip_prefix("Offset(") {
            ("_wc_gather_offset", a)
        } else if let Some(a) = after.strip_prefix('(') {
            ("_wc_gather", a)
        } else {
            out.push_str(GATHER);
            rest = after;
            continue;
        };
        // The first argument, up to its comma outside brackets.
        let mut depth = 0i32;
        let end = args
            .find(|c| {
                match c {
                    '(' | '[' => depth += 1,
                    ')' | ']' => depth -= 1,
                    _ => {}
                }
                c == ',' && depth == 0
            })
            .unwrap_or(0);
        let name = args[..end].trim();
        out.push_str(&format!("{helper}({name}, textureSize({name}, 0)"));
        rest = &args[end..];
    }
    out.push_str(rest);
    let mut f = String::new();
    for g in ["", "i", "u"] {
        f.push_str(&format!(
            "highp {g}vec4 _wc_gather_offset(highp {g}sampler2D s, ivec2 n, highp vec2 p, ivec2 o, int c) {{\n\
             highp vec2 size = vec2(n); highp vec2 b = floor(p * size - 0.5) + vec2(o);\n\
             highp vec2 l = (b + 0.5) / size; highp vec2 h = (b + 1.5) / size;\n\
             return {g}vec4(textureLod(s, vec2(l.x, h.y), 0.0)[c], textureLod(s, h, 0.0)[c], \
             textureLod(s, vec2(h.x, l.y), 0.0)[c], textureLod(s, l, 0.0)[c]); }}\n\
             highp {g}vec4 _wc_gather(highp {g}sampler2D s, ivec2 n, highp vec2 p, int c) {{ \
             return _wc_gather_offset(s, n, p, ivec2(0), c); }}\n\
             highp {g}vec4 _wc_gather_offset(highp {g}sampler2DArray s, ivec3 n, highp vec3 p, ivec2 o, int c) {{\n\
             highp vec2 size = vec2(n.xy); highp vec2 b = floor(p.xy * size - 0.5) + vec2(o);\n\
             highp vec2 l = (b + 0.5) / size; highp vec2 h = (b + 1.5) / size;\n\
             return {g}vec4(textureLod(s, vec3(l.x, h.y, p.z), 0.0)[c], textureLod(s, vec3(h, p.z), 0.0)[c], \
             textureLod(s, vec3(h.x, l.y, p.z), 0.0)[c], textureLod(s, vec3(l, p.z), 0.0)[c]); }}\n\
             highp {g}vec4 _wc_gather(highp {g}sampler2DArray s, ivec3 n, highp vec3 p, int c) {{ \
             return _wc_gather_offset(s, n, p, ivec2(0), c); }}\n\
             highp {g}vec4 _wc_gather(highp {g}samplerCube s, ivec2 n, highp vec3 d, int c) {{\n\
             highp vec3 m; highp vec3 u; highp vec3 v; _wc_face(d, m, u, v);\n\
             highp vec2 size = vec2(n); highp vec2 b = floor(_wc_face_st(d, m, u, v) * size - 0.5);\n\
             highp vec2 l = (b + 0.5) / size * 2.0 - 1.0; highp vec2 h = (b + 1.5) / size * 2.0 - 1.0;\n\
             return {g}vec4(textureLod(s, m + l.x * u + h.y * v, 0.0)[c], textureLod(s, m + h.x * u + h.y * v, 0.0)[c], \
             textureLod(s, m + h.x * u + l.y * v, 0.0)[c], textureLod(s, m + l.x * u + l.y * v, 0.0)[c]); }}\n"
        ));
    }
    let face = "void _wc_face(highp vec3 d, out highp vec3 m, out highp vec3 u, out highp vec3 v) {\n\
        highp vec3 a = abs(d);\n\
        if (a.x >= a.y && a.x >= a.z) { m = vec3(sign(d.x), 0.0, 0.0); u = vec3(0.0, 0.0, -sign(d.x)); v = vec3(0.0, -1.0, 0.0); }\n\
        else if (a.y >= a.z) { m = vec3(0.0, sign(d.y), 0.0); u = vec3(1.0, 0.0, 0.0); v = vec3(0.0, 0.0, sign(d.y)); }\n\
        else { m = vec3(0.0, 0.0, sign(d.z)); u = vec3(sign(d.z), 0.0, 0.0); v = vec3(0.0, -1.0, 0.0); } }\n\
        highp vec2 _wc_face_st(highp vec3 d, highp vec3 m, highp vec3 u, highp vec3 v) {\n\
        return (vec2(dot(d, u), dot(d, v)) / dot(d, m) + 1.0) * 0.5; }\n";
    // Comparisons; GLSL ES 3.00 has `textureLod` only for 2D shadow samplers.
    let shadow = "highp vec4 _wc_gather_offset(highp sampler2DShadow s, ivec2 n, highp vec2 p, highp float r, ivec2 o) {\n\
        highp vec2 size = vec2(n); highp vec2 b = floor(p * size - 0.5) + vec2(o);\n\
        highp vec2 l = (b + 0.5) / size; highp vec2 h = (b + 1.5) / size;\n\
        return vec4(textureLod(s, vec3(l.x, h.y, r), 0.0), textureLod(s, vec3(h, r), 0.0), \
        textureLod(s, vec3(h.x, l.y, r), 0.0), textureLod(s, vec3(l, r), 0.0)); }\n\
        highp vec4 _wc_gather(highp sampler2DShadow s, ivec2 n, highp vec2 p, highp float r) { \
        return _wc_gather_offset(s, n, p, r, ivec2(0)); }\n\
        highp vec4 _wc_gather_offset(highp sampler2DArrayShadow s, ivec3 n, highp vec3 p, highp float r, ivec2 o) {\n\
        highp vec2 size = vec2(n.xy); highp vec2 b = floor(p.xy * size - 0.5) + vec2(o);\n\
        highp vec2 l = (b + 0.5) / size; highp vec2 h = (b + 1.5) / size; highp vec2 z = vec2(0.0);\n\
        return vec4(textureGrad(s, vec4(l.x, h.y, p.z, r), z, z), textureGrad(s, vec4(h, p.z, r), z, z), \
        textureGrad(s, vec4(h.x, l.y, p.z, r), z, z), textureGrad(s, vec4(l, p.z, r), z, z)); }\n\
        highp vec4 _wc_gather(highp sampler2DArrayShadow s, ivec3 n, highp vec3 p, highp float r) { \
        return _wc_gather_offset(s, n, p, r, ivec2(0)); }\n\
        highp vec4 _wc_gather(highp samplerCubeShadow s, ivec2 n, highp vec3 d, highp float r) {\n\
        highp vec3 m; highp vec3 u; highp vec3 v; _wc_face(d, m, u, v); highp vec3 z = vec3(0.0);\n\
        highp vec2 size = vec2(n); highp vec2 b = floor(_wc_face_st(d, m, u, v) * size - 0.5);\n\
        highp vec2 l = (b + 0.5) / size * 2.0 - 1.0; highp vec2 h = (b + 1.5) / size * 2.0 - 1.0;\n\
        return vec4(textureGrad(s, vec4(m + l.x * u + h.y * v, r), z, z), textureGrad(s, vec4(m + h.x * u + h.y * v, r), z, z), \
        textureGrad(s, vec4(m + h.x * u + l.y * v, r), z, z), textureGrad(s, vec4(m + l.x * u + l.y * v, r), z, z)); }\n";
    after_header(&out, &format!("{face}{f}{shadow}"))
}

/// A module rewritten by [`split_samplers`], and each copy's slot with the slot it stands for.
type Split = (ShaderModule, Vec<(Slot, Slot)>);

/// The binding of the first copy [`split_samplers`] makes, past every binding a front gives.
const COPY_BINDINGS: u32 = 1 << 20;

/// GLSL combines each texture with one sampler, and naga's writer refuses a texture sampled through
/// two. Where a program does that, each texture read through a further sampler is read through a copy
/// of its global variable, at a binding of its own; the copies and the slots they stand for are
/// returned with the rewritten module (None where no texture has two samplers).
fn split_samplers(module: &ShaderModule) -> Result<Option<Split>, &'static str> {
    use naga::{Expression, GlobalVariable, Handle};
    use std::collections::{BTreeMap, btree_map::Entry};
    type Var = Handle<GlobalVariable>;
    let m = &module.module;
    // The sampler each texture keeps: its first by handle.
    let mut kept: BTreeMap<Var, Var> = BTreeMap::new();
    for (i, _) in m.entry_points.iter().enumerate() {
        for s in module.info.get_entry_point(i).sampling_set.iter() {
            let first = kept.entry(s.image).or_insert(s.sampler);
            *first = (*first).min(s.sampler);
        }
    }
    // Functions by handle, then entry points by index.
    let functions = m.functions.iter().map(|(h, f)| (Ok(h), f));
    let entries = m.entry_points.iter().enumerate();
    let mut moves = Vec::new();
    for (index, f) in functions.chain(entries.map(|(i, e)| (Err(i), &e.function))) {
        // The sampler each expression naming a texture is sampled through.
        let mut through: BTreeMap<Handle<Expression>, (Var, Var)> = BTreeMap::new();
        for (_, e) in f.expressions.iter() {
            let Expression::ImageSample { image, sampler, .. } = *e else {
                continue;
            };
            let (Expression::GlobalVariable(t), Expression::GlobalVariable(s)) =
                (&f.expressions[image], &f.expressions[sampler])
            else {
                continue;
            };
            match through.entry(image) {
                Entry::Vacant(v) => {
                    v.insert((*t, *s));
                }
                Entry::Occupied(o) if o.get().1 != *s => {
                    return Err("webgl: one expression of a texture is sampled through two \
                                samplers (not built)");
                }
                Entry::Occupied(_) => {}
            }
        }
        for (expression, (t, s)) in through {
            if kept.get(&t) != Some(&s) {
                moves.push((index, expression, t, s));
            }
        }
    }
    if moves.is_empty() {
        return Ok(None);
    }
    let mut out = m.clone();
    let mut copies: BTreeMap<(Var, Var), Var> = BTreeMap::new();
    let mut own = Vec::new();
    for (index, expression, t, s) in moves {
        let copy = match copies.get(&(t, s)) {
            Some(c) => *c,
            None => {
                let original = &out.global_variables[t];
                let Some(binding) = original.binding else {
                    return Err("webgl: a sampled texture without a binding");
                };
                let at = naga::ResourceBinding {
                    group: binding.group,
                    binding: COPY_BINDINGS + own.len() as u32,
                };
                own.push(((at.group, at.binding), (binding.group, binding.binding)));
                let var = GlobalVariable {
                    name: original.name.as_ref().map(|n| format!("{n}_{}", own.len())),
                    binding: Some(at),
                    ..original.clone()
                };
                let c = out.global_variables.append(var, Default::default());
                copies.insert((t, s), c);
                c
            }
        };
        let f = match index {
            Ok(h) => &mut out.functions[h],
            Err(i) => &mut out.entry_points[i].function,
        };
        f.expressions[expression] = Expression::GlobalVariable(copy);
    }
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&out)
    .map_err(|_| "webgl: a texture's copy for its second sampler does not validate")?;
    Ok(Some((
        ShaderModule {
            module: out,
            info,
            entry_point: module.entry_point.clone(),
            stage: module.stage,
            key: module.key,
        },
        own,
    )))
}

/// The binding of the first copy [`plain_depth_reads`] makes, past those of [`split_samplers`].
const PLAIN_DEPTH_BINDINGS: u32 = COPY_BINDINGS << 1;

/// GLSL declares a depth texture as a shadow sampler, which only compares: naga's writer refuses a
/// load from one and samples it only with a reference. Where a program also reads a depth texture
/// plainly (sampled without a reference, loaded or gathered), each such read becomes a call to a
/// function of its own that reads a copy of the texture's variable declared as a float texture, at a
/// binding of its own (a depth texture sampled with comparison off gives its depth in red). The copies
/// and the slots they stand for are returned with the rewritten module (None where nothing reads a
/// depth texture plainly).
fn plain_depth_reads(module: &ShaderModule) -> Result<Option<Split>, &'static str> {
    use naga::{
        Arena, Expression, Function, FunctionArgument, FunctionResult, GlobalVariable, Handle,
        ImageClass, Range, SampleLevel, Scalar, Span, Statement, Type, TypeInner,
    };
    use std::collections::BTreeMap;
    let m = &module.module;
    let depth = |var: Handle<GlobalVariable>| {
        matches!(
            m.types[m.global_variables[var].ty].inner,
            TypeInner::Image {
                class: ImageClass::Depth { .. },
                ..
            }
        )
    };
    // The plain reads: the function (by handle, or entry point by index) and the expression.
    let functions = m.functions.iter().map(|(h, f)| (Ok(h), f));
    let entries = m.entry_points.iter().enumerate();
    let mut sites = Vec::new();
    for (index, f) in functions.chain(entries.map(|(i, e)| (Err(i), &e.function))) {
        for (h, e) in f.expressions.iter() {
            let image = match *e {
                Expression::ImageSample {
                    image,
                    depth_ref: None,
                    ..
                }
                | Expression::ImageLoad { image, .. } => image,
                _ => continue,
            };
            let info = match index {
                Ok(f) => &module.info[f],
                Err(i) => module.info.get_entry_point(i),
            };
            let TypeInner::Image {
                class: ImageClass::Depth { .. },
                ..
            } = *info[image].ty.inner_with(&m.types)
            else {
                continue;
            };
            match f.expressions[image] {
                Expression::GlobalVariable(var) if depth(var) => sites.push((index, h, var)),
                _ => {
                    return Err("webgl: a depth texture read plainly through a function \
                                argument (not built)");
                }
            }
        }
    }
    if sites.is_empty() {
        return Ok(None);
    }
    let mut out = m.clone();
    let mut copies: BTreeMap<Handle<GlobalVariable>, Handle<GlobalVariable>> = BTreeMap::new();
    let mut own = Vec::new();
    for &(_, _, var) in &sites {
        if copies.contains_key(&var) {
            continue;
        }
        let original = &m.global_variables[var];
        let (
            Some(binding),
            TypeInner::Image {
                dim,
                arrayed,
                class,
            },
        ) = (original.binding, &m.types[original.ty].inner)
        else {
            return Err("webgl: a depth texture without a binding");
        };
        let ImageClass::Depth { multi } = *class else {
            unreachable!("a depth texture's class")
        };
        let inner = TypeInner::Image {
            dim: *dim,
            arrayed: *arrayed,
            class: ImageClass::Sampled {
                kind: naga::ScalarKind::Float,
                multi,
            },
        };
        let ty = out
            .types
            .insert(Type { name: None, inner }, Span::UNDEFINED);
        let at = naga::ResourceBinding {
            group: binding.group,
            binding: PLAIN_DEPTH_BINDINGS + own.len() as u32,
        };
        own.push(((at.group, at.binding), (binding.group, binding.binding)));
        let copy = GlobalVariable {
            name: original.name.as_ref().map(|n| format!("{n}_plain")),
            binding: Some(at),
            ty,
            ..original.clone()
        };
        copies.insert(var, out.global_variables.append(copy, Span::UNDEFINED));
    }
    // One function per read, made first, as a function calls only functions made before it.
    let mut helpers = Arena::new();
    let mut calls = Vec::new();
    for &(index, h, var) in &sites {
        let (f, info) = match index {
            Ok(f) => (&m.functions[f], &module.info[f]),
            Err(i) => (&m.entry_points[i].function, module.info.get_entry_point(i)),
        };
        let mut type_of = |e: Handle<Expression>| match &info[e].ty {
            naga::proc::TypeResolution::Handle(t) => *t,
            naga::proc::TypeResolution::Value(inner) => out.types.insert(
                Type {
                    name: None,
                    inner: inner.clone(),
                },
                Span::UNDEFINED,
            ),
        };
        let mut helper = Function {
            name: Some(format!("_wc_plain_depth_{}", calls.len())),
            ..Function::default()
        };
        let mut arguments = Vec::new();
        // Each operand the read computes becomes an argument; textures, samplers and constant
        // offsets are read where they are.
        let mut operand = |e: Handle<Expression>, helper: &mut Function| -> Handle<Expression> {
            match f.expressions[e] {
                Expression::GlobalVariable(_)
                | Expression::Constant(_)
                | Expression::Literal(_)
                | Expression::ZeroValue(_) => {
                    return helper
                        .expressions
                        .append(f.expressions[e].clone(), Span::UNDEFINED);
                }
                _ => {}
            }
            helper.arguments.push(FunctionArgument {
                name: None,
                ty: type_of(e),
                binding: None,
            });
            arguments.push(e);
            let at = arguments.len() as u32 - 1;
            helper
                .expressions
                .append(Expression::FunctionArgument(at), Span::UNDEFINED)
        };
        let image = helper
            .expressions
            .append(Expression::GlobalVariable(copies[&var]), Span::UNDEFINED);
        // The first expression the helper emits: what precedes it is never emitted.
        let mut emitted = None;
        let read = match f.expressions[h].clone() {
            Expression::ImageSample {
                sampler,
                gather,
                coordinate,
                array_index,
                offset,
                level,
                clamp_to_edge,
                ..
            } => {
                let sampler = operand(sampler, &mut helper);
                let coordinate = operand(coordinate, &mut helper);
                let array_index = array_index.map(|a| operand(a, &mut helper));
                let integer_level = match level {
                    SampleLevel::Exact(l) => info[l]
                        .ty
                        .inner_with(&m.types)
                        .scalar_kind()
                        .filter(|k| *k != naga::ScalarKind::Float),
                    _ => None,
                };
                let level = match level {
                    SampleLevel::Exact(l) => SampleLevel::Exact(operand(l, &mut helper)),
                    SampleLevel::Bias(b) => SampleLevel::Bias(operand(b, &mut helper)),
                    SampleLevel::Gradient { x, y } => SampleLevel::Gradient {
                        x: operand(x, &mut helper),
                        y: operand(y, &mut helper),
                    },
                    l => l,
                };
                let offset = match offset {
                    None => None,
                    Some(o) => {
                        let (c, built) = constant(&f.expressions, o, &mut helper.expressions)?;
                        emitted = built.then_some(c);
                        Some(c)
                    }
                };
                // A depth texture's level may be an integer, a float texture's is a float.
                let level = match (level, integer_level) {
                    (SampleLevel::Exact(l), Some(_)) => {
                        let float = helper.expressions.append(
                            Expression::As {
                                expr: l,
                                kind: naga::ScalarKind::Float,
                                convert: Some(Scalar::F32.width),
                            },
                            Span::UNDEFINED,
                        );
                        emitted = emitted.or(Some(float));
                        SampleLevel::Exact(float)
                    }
                    (l, _) => l,
                };
                Expression::ImageSample {
                    image,
                    sampler,
                    gather,
                    coordinate,
                    array_index,
                    offset,
                    level,
                    depth_ref: None,
                    clamp_to_edge,
                }
            }
            Expression::ImageLoad {
                coordinate,
                array_index,
                sample,
                level,
                ..
            } => Expression::ImageLoad {
                image,
                coordinate: operand(coordinate, &mut helper),
                array_index: array_index.map(|a| operand(a, &mut helper)),
                sample: sample.map(|s| operand(s, &mut helper)),
                level: level.map(|l| operand(l, &mut helper)),
            },
            _ => unreachable!("a plain read"),
        };
        let gathers = matches!(
            read,
            Expression::ImageSample {
                gather: Some(_),
                ..
            }
        );
        let first = helper.expressions.append(read, Span::UNDEFINED);
        // The copy reads four components where the depth texture read one, its red.
        let last = if gathers {
            first
        } else {
            helper.expressions.append(
                Expression::AccessIndex {
                    base: first,
                    index: 0,
                },
                Span::UNDEFINED,
            )
        };
        let result = if gathers {
            type_of(h)
        } else {
            out.types.insert(
                Type {
                    name: None,
                    inner: TypeInner::Scalar(Scalar::F32),
                },
                Span::UNDEFINED,
            )
        };
        helper.result = Some(FunctionResult {
            ty: result,
            binding: None,
        });
        helper.body.push(
            Statement::Emit(Range::new_from_bounds(emitted.unwrap_or(first), last)),
            Span::UNDEFINED,
        );
        helper
            .body
            .push(Statement::Return { value: Some(last) }, Span::UNDEFINED);
        calls.push((index, h, helpers.append(helper, Span::UNDEFINED), arguments));
    }
    // The program's own functions after the helpers, every call moved to the new handles.
    let mut moved = BTreeMap::new();
    let old = std::mem::replace(&mut out.functions, helpers);
    for (h, f) in old.iter() {
        moved.insert(h, out.functions.append(f.clone(), old.get_span(h)));
    }
    let renumber = |f: &mut Function| {
        for (_, e) in f.expressions.iter_mut() {
            if let Expression::CallResult(c) = e {
                *c = moved[c];
            }
        }
        each_statement(&mut f.body, &mut |s| {
            if let Statement::Call { function, .. } = s {
                *function = moved[function];
            }
        });
    };
    for (_, f) in out.functions.iter_mut().skip(calls.len()) {
        renumber(f);
    }
    for e in &mut out.entry_points {
        renumber(&mut e.function);
    }
    for (index, h, helper, arguments) in calls {
        let f = match index {
            Ok(f) => out.functions.get_mut(moved[&f]),
            Err(i) => &mut out.entry_points[i].function,
        };
        f.expressions[h] = Expression::CallResult(helper);
        let handles: Vec<_> = f.expressions.iter().map(|(e, _)| e).collect();
        let call = Statement::Call {
            function: helper,
            arguments,
            result: Some(h),
        };
        if !call_in_place(&mut f.body, h, &handles, call) {
            return Err("webgl: a plain depth read outside an emitted range");
        }
    }
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&out)
    .map_err(|_| "webgl: a depth texture's plain copy does not validate")?;
    Ok(Some((
        ShaderModule {
            module: out,
            info,
            entry_point: module.entry_point.clone(),
            stage: module.stage,
            key: module.key,
        },
        own,
    )))
}

/// Copies the constant expression `e` of `from` into `to`, its leaves before what is built from them
/// (leaves are never emitted), for an offset, which must be constant. True where the copy is built
/// from leaves, and so must be emitted.
fn constant(
    from: &naga::Arena<naga::Expression>,
    e: naga::Handle<naga::Expression>,
    to: &mut naga::Arena<naga::Expression>,
) -> Result<(naga::Handle<naga::Expression>, bool), &'static str> {
    use naga::Expression;
    let leaf = |e: &Expression| {
        matches!(
            e,
            Expression::Literal(_) | Expression::Constant(_) | Expression::ZeroValue(_)
        )
    };
    match &from[e] {
        l if leaf(l) => Ok((to.append(l.clone(), naga::Span::UNDEFINED), false)),
        Expression::Compose { ty, components } if components.iter().all(|&c| leaf(&from[c])) => {
            let components = components
                .iter()
                .map(|&c| to.append(from[c].clone(), naga::Span::UNDEFINED))
                .collect();
            let ty = *ty;
            Ok((
                to.append(
                    Expression::Compose { ty, components },
                    naga::Span::UNDEFINED,
                ),
                true,
            ))
        }
        _ => Err("webgl: a plain depth read's offset is not a vector of constants"),
    }
}

/// Calls `f` on every statement of `block` and of the blocks inside it.
fn each_statement(block: &mut naga::Block, f: &mut impl FnMut(&mut naga::Statement)) {
    use naga::Statement;
    for (s, _) in block.span_iter_mut() {
        f(s);
        match s {
            Statement::Block(b) => each_statement(b, f),
            Statement::If { accept, reject, .. } => {
                each_statement(accept, f);
                each_statement(reject, f);
            }
            Statement::Switch { cases, .. } => {
                for c in cases {
                    each_statement(&mut c.body, f);
                }
            }
            Statement::Loop {
                body, continuing, ..
            } => {
                each_statement(body, f);
                each_statement(continuing, f);
            }
            _ => {}
        }
    }
}

/// Puts `call`, whose result is `at`, where `at` was emitted in `block` (or a block inside it): the
/// emitted range is split around it. `handles` are the function's expressions by index. False where
/// no range emits `at`.
fn call_in_place(
    block: &mut naga::Block,
    at: naga::Handle<naga::Expression>,
    handles: &[naga::Handle<naga::Expression>],
    call: naga::Statement,
) -> bool {
    use naga::{Block, Range, Span, Statement};
    let found = block.span_iter_mut().position(|(s, _)| match s {
        Statement::Emit(r) => r.index_range().contains(&(at.index() as u32)),
        _ => false,
    });
    if let Some(k) = found {
        let Some((Statement::Emit(r), span)) = block.span_iter_mut().nth(k) else {
            unreachable!("the emit found")
        };
        let span = span.copied().unwrap_or(Span::UNDEFINED);
        let range = r.index_range();
        let mut split = Vec::new();
        let range_of =
            |a: u32, b: u32| Range::new_from_bounds(handles[a as usize], handles[b as usize]);
        if range.start < at.index() as u32 {
            split.push(Statement::Emit(range_of(
                range.start,
                at.index() as u32 - 1,
            )));
        }
        split.push(call);
        if at.index() as u32 + 1 < range.end {
            split.push(Statement::Emit(range_of(
                at.index() as u32 + 1,
                range.end - 1,
            )));
        }
        let mut replaced = Block::with_capacity(split.len());
        for s in split {
            replaced.push(s, span);
        }
        block.splice(k..=k, replaced);
        return true;
    }
    let mut call = Some(call);
    for (s, _) in block.span_iter_mut() {
        let inner: Vec<&mut Block> = match s {
            Statement::Block(b) => vec![b],
            Statement::If { accept, reject, .. } => vec![accept, reject],
            Statement::Switch { cases, .. } => cases.iter_mut().map(|c| &mut c.body).collect(),
            Statement::Loop {
                body, continuing, ..
            } => vec![body, continuing],
            _ => continue,
        };
        for b in inner {
            if contains_emit(b, at) {
                return call_in_place(b, at, handles, call.take().expect("one call"));
            }
        }
    }
    false
}

/// Whether `block` or a block inside it emits `at`.
fn contains_emit(block: &naga::Block, at: naga::Handle<naga::Expression>) -> bool {
    use naga::Statement;
    block.iter().any(|s| match s {
        Statement::Emit(r) => r.index_range().contains(&(at.index() as u32)),
        Statement::Block(b) => contains_emit(b, at),
        Statement::If { accept, reject, .. } => {
            contains_emit(accept, at) || contains_emit(reject, at)
        }
        Statement::Switch { cases, .. } => cases.iter().any(|c| contains_emit(&c.body, at)),
        Statement::Loop {
            body, continuing, ..
        } => contains_emit(body, at) || contains_emit(continuing, at),
        _ => false,
    })
}

/// The slot a global variable of `module` is bound at.
fn slot(module: &naga::Module, var: naga::Handle<naga::GlobalVariable>) -> Option<Slot> {
    module.global_variables[var]
        .binding
        .as_ref()
        .map(|b| (b.group, b.binding))
}

/// Writes a shader's GLSL and what its program must bind.
pub fn write(module: &ShaderModule, settings: GlslSettings) -> Result<ShaderText, GpuError> {
    let plain = plain_depth_reads(module).map_err(GpuError::Invalid)?;
    let module = plain.as_ref().map_or(module, |p| &p.0);
    let split = split_samplers(module).map_err(GpuError::Invalid)?;
    // Every copy's slot with its texture's: a sampler's copy of a plain copy stands for the texture.
    let mut own: Vec<(Slot, Slot)> = plain.as_ref().map_or(Vec::new(), |p| p.1.clone());
    if let Some((_, copies)) = &split {
        for &(copy, of) in copies {
            let texture = own.iter().find(|(c, _)| *c == of).map_or(of, |o| o.1);
            own.push((copy, texture));
        }
    }
    let module = split.as_ref().map_or(module, |s| &s.0);
    let own = &own[..];
    // What the back end lacks is refused by `needs` before this, so a refusal here is the front's.
    let (source, reflection, level_zero) = write_glsl(module, settings).map_err(|e| {
        crate::platform::report(&format!(
            "wcontainers webgl: naga could not write the shader as GLSL ES 3.00: {e}"
        ));
        GpuError::Invalid("webgl: naga could not write the shader as GLSL ES 3.00")
    })?;
    // A copy's slot is its texture's.
    let slot = |m: &naga::Module, var| {
        let at = slot(m, var)?;
        Some(own.iter().find(|(copy, _)| *copy == at).map_or(at, |o| o.1))
    };
    let names: Vec<&str> = reflection
        .texture_mapping
        .keys()
        .map(String::as_str)
        .collect();
    let source = sizes_from_uniforms(source, &names);
    let m = &module.module;
    let mut textures: Vec<Combined> = reflection
        .texture_mapping
        .iter()
        .filter_map(|(name, map)| {
            Some(Combined {
                name: name.clone(),
                texture: slot(m, map.texture)?,
                sampler: map.sampler.and_then(|s| slot(m, s)),
                level_zero: false,
            })
        })
        .collect();
    for (unit, of) in level_zero {
        let Some(c) = textures.iter_mut().find(|c| c.name == of) else {
            continue;
        };
        if unit == of {
            c.level_zero = true;
        } else {
            let twin = Combined {
                name: unit,
                level_zero: true,
                ..c.clone()
            };
            textures.push(twin);
        }
    }
    textures.sort();
    let mut blocks: Vec<_> = reflection
        .uniforms
        .iter()
        .filter_map(|(var, name)| Some((name.clone(), slot(m, *var)?)))
        .collect();
    blocks.sort();
    let outputs = match module.stage {
        ShaderStage::Vertex => outputs(module),
        _ => Vec::new(),
    };
    let immediates = reflection
        .immediates_items
        .iter()
        .map(|i| (i.access_path.clone(), i.offset, i.ty))
        .collect();
    let clip_distances = results(module)
        .find(|(b, _)| *b == naga::Binding::BuiltIn(naga::BuiltIn::ClipDistances))
        .map_or(0, |(_, n)| n);
    let cull_distances = results(module)
        .find(|(b, _)| *b == naga::Binding::BuiltIn(naga::BuiltIn::CullDistance))
        .map_or(0, |(_, n)| n);
    if clip_distances + cull_distances > CLIP_AND_CULL_DISTANCES {
        return Err(GpuError::Invalid(
            "webgl: more than eight clip and cull distances",
        ));
    }
    Ok(ShaderText {
        stage: module.stage,
        textures,
        blocks,
        source,
        outputs,
        immediates,
        clip_distances: clip_distances as u8,
    })
}

/// The entry point's output bindings, each with its array length (1 for what is not an array).
fn results(module: &ShaderModule) -> impl Iterator<Item = (naga::Binding, u32)> + '_ {
    let m = &module.module;
    let result = m
        .entry_points
        .iter()
        .find(|e| e.name == module.entry_point)
        .and_then(|e| e.function.result.as_ref());
    let length = |ty: naga::Handle<naga::Type>| match m.types[ty].inner {
        naga::TypeInner::Array {
            size: naga::ArraySize::Constant(n),
            ..
        } => n.get(),
        _ => 1,
    };
    result.into_iter().flat_map(move |r| {
        let single = r.binding.clone().map(|b| (b, length(r.ty)));
        let fields = match &m.types[r.ty].inner {
            naga::TypeInner::Struct { members, .. } if r.binding.is_none() => &members[..],
            _ => &[],
        };
        single.into_iter().chain(
            fields
                .iter()
                .filter_map(move |f| Some((f.binding.clone()?, length(f.ty)))),
        )
    })
}

/// The entry point's input bindings.
fn arguments(module: &ShaderModule) -> impl Iterator<Item = naga::Binding> + '_ {
    let m = &module.module;
    m.entry_points
        .iter()
        .filter(|e| e.name == module.entry_point)
        .flat_map(|e| &e.function.arguments)
        .flat_map(move |a| {
            let fields = match &m.types[a.ty].inner {
                naga::TypeInner::Struct { members, .. } if a.binding.is_none() => &members[..],
                _ => &[],
            };
            a.binding
                .clone()
                .into_iter()
                .chain(fields.iter().filter_map(|f| f.binding.clone()))
        })
}

/// How many uniform blocks the GLSL of `module`'s entry point declares: one per uniform buffer it
/// reads.
pub(crate) fn uniform_blocks(module: &ShaderModule) -> u32 {
    let m = &module.module;
    let Some(i) = m
        .entry_points
        .iter()
        .position(|e| e.name == module.entry_point)
    else {
        return 0;
    };
    let used = module.info.get_entry_point(i);
    m.global_variables
        .iter()
        .filter(|(h, v)| v.space == naga::AddressSpace::Uniform && !used[*h].is_empty())
        .count() as u32
}

/// The features a shader needs that a back end may lack, read from its inputs and outputs, the
/// globals its entry point uses, its types and its expressions. Storage that a fragment program only
/// reads is named by `FRAGMENT_STORAGE_WRITE`, as binding sets name it: the interface has no bit of
/// its own for it.
pub fn needs(module: &ShaderModule) -> Features {
    use naga::{
        AddressSpace, Binding, BuiltIn, Expression, ImageClass, ImageQuery, Sampling, ScalarKind,
        StorageAccess, TypeInner,
    };
    let m = &module.module;
    let vertex = module.stage == ShaderStage::Vertex;
    let storage = |access: StorageAccess| match (vertex, access.contains(StorageAccess::STORE)) {
        (true, false) => Features::VERTEX_STORAGE_READ,
        (true, true) => Features::VERTEX_STORAGE_WRITE,
        (false, _) => Features::FRAGMENT_STORAGE_WRITE,
    };
    let mut f = Features::empty();
    for b in results(module).map(|(b, _)| b).chain(arguments(module)) {
        f |= match b {
            Binding::BuiltIn(BuiltIn::ClipDistances) => Features::CLIP_DISTANCE,
            Binding::BuiltIn(BuiltIn::CullDistance) => Features::CULL_DISTANCE,
            Binding::BuiltIn(BuiltIn::PointSize) => Features::POINT_SIZE,
            Binding::BuiltIn(BuiltIn::SampleIndex | BuiltIn::SampleMask) => {
                Features::SAMPLE_SHADING
            }
            Binding::BuiltIn(BuiltIn::PrimitiveIndex) => Features::PRIMITIVE_INDEX,
            Binding::BuiltIn(
                BuiltIn::NumSubgroups
                | BuiltIn::SubgroupId
                | BuiltIn::SubgroupSize
                | BuiltIn::SubgroupInvocationId,
            ) => Features::SUBGROUPS,
            Binding::Location {
                blend_src: Some(1), ..
            } => Features::DUAL_SOURCE_BLEND,
            Binding::Location {
                sampling: Some(Sampling::Sample),
                ..
            } => Features::SAMPLE_SHADING,
            _ => Features::empty(),
        };
    }
    let entry = m
        .entry_points
        .iter()
        .position(|e| e.name == module.entry_point);
    if let Some(i) = entry {
        let used = module.info.get_entry_point(i);
        for (h, var) in m.global_variables.iter() {
            if used[h].is_empty() {
                continue;
            }
            if let AddressSpace::Storage { access } = var.space {
                f |= storage(access);
            }
            f |= match &m.types[var.ty].inner {
                TypeInner::Image { class, .. } => match *class {
                    ImageClass::Sampled { multi: true, .. } | ImageClass::Depth { multi: true } => {
                        Features::MULTISAMPLE_READ
                    }
                    ImageClass::Storage { access, .. } => storage(access),
                    _ => Features::empty(),
                },
                TypeInner::BindingArray { .. } => Features::RESOURCE_TABLES,
                _ => Features::empty(),
            };
        }
    }
    // Scalar widths, from the declared types and from each expression's type (naga keeps the
    // types of most expressions out of the module's arena).
    let width = |inner: &TypeInner| match *inner {
        TypeInner::Scalar(s)
        | TypeInner::Vector { scalar: s, .. }
        | TypeInner::Matrix { scalar: s, .. }
        | TypeInner::Atomic(s) => match (s.kind, s.width) {
            (ScalarKind::Float, 2) => Features::SHADER_F16,
            (ScalarKind::Float, 8) => Features::SHADER_F64,
            (ScalarKind::Sint | ScalarKind::Uint, 8) => Features::SHADER_I64,
            _ => Features::empty(),
        },
        _ => Features::empty(),
    };
    for (_, t) in m.types.iter() {
        f |= width(&t.inner);
    }
    let functions = m.functions.iter().map(|(h, func)| (func, &module.info[h]));
    let entry = entry.map(|i| (&m.entry_points[i].function, module.info.get_entry_point(i)));
    for (func, info) in functions.chain(entry) {
        for (h, e) in func.expressions.iter() {
            f |= width(info[h].ty.inner_with(&m.types));
            f |= match e {
                Expression::ImageQuery {
                    query: ImageQuery::NumLevels | ImageQuery::NumSamples,
                    ..
                } => Features::TEXTURE_LEVEL_QUERY,
                Expression::SubgroupBallotResult | Expression::SubgroupOperationResult { .. } => {
                    Features::SUBGROUPS
                }
                Expression::Derivative {
                    ctrl: naga::DerivativeControl::Fine,
                    ..
                } => Features::FINE_DERIVATIVES,
                _ => Features::empty(),
            };
        }
    }
    f
}

/// Turns each `textureSize(t, l)` into a shift of the hidden uniform `t_size`, the size of the bound
/// view's first level, which the back end sets with the view. ANGLE on Direct3D 11 answers
/// `textureSize` for the wrong level once a texture's base level has changed.
fn sizes_from_uniforms(source: String, names: &[&str]) -> String {
    if !source.contains("textureSize(") {
        return source;
    }
    const HELPERS: &str = "ivec2 _wc_size_2d(ivec3 s, int l) { return max(s.xy >> l, ivec2(1)); }\n\
        ivec3 _wc_size_array(ivec3 s, int l) { return ivec3(max(s.xy >> l, ivec2(1)), s.z); }\n\
        ivec3 _wc_size_3d(ivec3 s, int l) { return max(s >> l, ivec3(1)); }\n";
    let mut out = String::with_capacity(source.len() + 512);
    let mut helpers = false;
    for line in source.lines() {
        out.push_str(line);
        out.push('\n');
        let declared = names.iter().find(|n| {
            line.starts_with("uniform ")
                && line.contains("sampler")
                && line
                    .strip_suffix(';')
                    .is_some_and(|l| l.ends_with(&format!(" {n}")))
        });
        if let Some(name) = declared {
            out.push_str(&format!("uniform highp ivec3 {name}_size;\n"));
            if !helpers {
                out.push_str(HELPERS);
                helpers = true;
            }
        }
    }
    for name in names {
        let helper = match source.lines().find(|l| {
            l.starts_with("uniform ")
                && l.strip_suffix(';')
                    .is_some_and(|l| l.ends_with(&format!(" {name}")))
        }) {
            Some(l) if l.contains("sampler3D") => "_wc_size_3d",
            Some(l) if l.contains("Array") => "_wc_size_array",
            _ => "_wc_size_2d",
        };
        out = out.replace(
            &format!("textureSize({name}, "),
            &format!("{helper}({name}_size, "),
        );
    }
    out
}

/// Starts compiling GLSL of `stage`. The compile status is read when its program links, so compiles
/// can overlap.
pub(crate) fn start_compile(
    gl: &impl OpenGlEs,
    stage: ShaderStage,
    source: &str,
) -> Result<u32, GpuError> {
    let kind = match stage {
        ShaderStage::Vertex => gles::VERTEX_SHADER,
        _ => gles::FRAGMENT_SHADER,
    };
    let raw = gl.create_shader(kind);
    if raw == 0 {
        return Err(GpuError::OutOfMemory);
    }
    gl.shader_source(raw, &[source.as_bytes()]);
    gl.compile_shader(raw);
    Ok(raw)
}

/// The entry point's location outputs in location order, named as naga's GLSL writer names vertex
/// outputs, with their size in bytes (0 for types transform feedback cannot write).
fn outputs(module: &ShaderModule) -> Vec<(String, u32)> {
    let m = &module.module;
    let Some(entry) = m.entry_points.iter().find(|e| e.name == module.entry_point) else {
        return Vec::new();
    };
    let Some(result) = &entry.function.result else {
        return Vec::new();
    };
    let size = |ty: naga::Handle<naga::Type>| match m.types[ty].inner {
        naga::TypeInner::Scalar(s) if s.width == 4 => 4,
        naga::TypeInner::Vector { size, scalar } if scalar.width == 4 => 4 * size as u32,
        _ => 0,
    };
    let mut found: Vec<(u32, u32)> = match (&result.binding, &m.types[result.ty].inner) {
        (Some(naga::Binding::Location { location, .. }), _) => vec![(*location, size(result.ty))],
        (None, naga::TypeInner::Struct { members, .. }) => members
            .iter()
            .filter_map(|member| match member.binding {
                Some(naga::Binding::Location { location, .. }) => Some((location, size(member.ty))),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    found.sort();
    found
        .into_iter()
        .map(|(location, bytes)| (format!("{VARYING}{location}"), bytes))
        .collect()
}

/// What a capture program writes: interleaved into one target or separate, and whether it draws.
#[derive(Clone, Copy, Debug)]
pub struct Capture {
    pub separate: bool,
    pub rasterize: bool,
}

impl Capture {
    /// Its place in [`ProgramObj::captures`].
    pub fn slot(self) -> usize {
        usize::from(self.separate) * 2 + usize::from(self.rasterize)
    }
}

/// Starts linking `program`'s shaders again from their sources, as a capture program whose
/// location outputs are transform feedback varyings (one that does not draw links the empty fragment
/// shader, so no fragment output meets the framebuffer), or for a vertex layout whose attributes in
/// `convert` GL ES cannot read as stored (see [`convert_inputs`]). The new program captures as
/// `program` does unless `capture` names how.
pub(crate) fn relink(
    gl: &impl OpenGlEs,
    id: u64,
    program: &ProgramObj,
    capture: Option<Capture>,
    convert: u32,
    empty_fragment: u32,
) -> Result<ProgramObj, GpuError> {
    let capture = capture.or(program.capture);
    let (vertex, fragment) = &program.stages;
    let fresh = |s: &ShaderObj, source: String| -> Result<Rc<ShaderObj>, GpuError> {
        Ok(Rc::new(ShaderObj {
            id: s.id,
            raw: start_compile(gl, s.stage, &source)?,
            handles: std::cell::Cell::new(1),
            text: Rc::new(ShaderText {
                source,
                ..(*s.text).clone()
            }),
        }))
    };
    let source = match convert {
        0 => vertex.source.clone(),
        _ => convert_inputs(&vertex.source, convert).ok_or(GpuError::Invalid(
            "webgl: a converted vertex attribute the program does not read as naga writes it",
        ))?,
    };
    let v = fresh(vertex, source)?;
    let f = match fragment {
        Some(f) if capture.is_none_or(|c| c.rasterize) => Some(fresh(f, f.source.clone())?),
        _ => None,
    };
    let names: Vec<std::ffi::CString> = v
        .outputs
        .iter()
        .map(|(n, _)| std::ffi::CString::new(n.as_str()))
        .collect::<Result<_, _>>()
        .map_err(|_| GpuError::Invalid("webgl: an output name holding a zero byte"))?;
    let names: Vec<&core::ffi::CStr> = names.iter().map(|n| n.as_c_str()).collect();
    let raw = {
        let raw = gl.create_program();
        if raw == 0 {
            return Err(GpuError::OutOfMemory);
        }
        gl.attach_shader(raw, v.raw);
        gl.attach_shader(raw, f.as_ref().map_or(empty_fragment, |f| f.raw));
        if let Some(c) = capture {
            let mode = if c.separate {
                gles::SEPARATE_ATTRIBS
            } else {
                gles::INTERLEAVED_ATTRIBS
            };
            gl.transform_feedback_varyings(raw, &names, mode);
        }
        gl.link_program(raw);
        // Flagged only: GL deletes them with the program.
        gl.delete_shader(v.raw);
        if let Some(f) = &f {
            gl.delete_shader(f.raw);
        }
        raw
    };
    let mut p = started(id, raw, (v, f), program.pair);
    p.capture = capture;
    Ok(p)
}

/// A program whose link has started.
fn started(id: u64, raw: u32, stages: Pair, pair: (u64, u64)) -> ProgramObj {
    ProgramObj {
        id,
        raw,
        shaders: std::cell::RefCell::new(Some(stages.clone())),
        stages,
        captures: Default::default(),
        linked: std::cell::OnceCell::new(),
        pair,
        handles: std::cell::Cell::new(1),
        capture: None,
        variants: Default::default(),
    }
}

/// Starts linking a vertex and an optional fragment shader. A fragment-less program gets an empty
/// fragment shader, since GLES links none without one.
pub(crate) fn link(
    gl: &impl OpenGlEs,
    id: u64,
    vertex: &Rc<ShaderObj>,
    fragment: Option<&Rc<ShaderObj>>,
    empty_fragment: u32,
) -> Result<ProgramObj, GpuError> {
    let raw = {
        let raw = gl.create_program();
        if raw == 0 {
            return Err(GpuError::OutOfMemory);
        }
        gl.attach_shader(raw, vertex.raw);
        gl.attach_shader(raw, fragment.map_or(empty_fragment, |f| f.raw));
        gl.link_program(raw);
        raw
    };
    Ok(started(
        id,
        raw,
        (vertex.clone(), fragment.cloned()),
        (vertex.id, fragment.map_or(0, |f| f.id)),
    ))
}

/// Reads a started link, waiting for it, then points each combined sampler at a unit and each uniform
/// block at a binding point. Leaves the program in use. On failure the error carries GL's logs.
pub(crate) fn finish(gl: &impl OpenGlEs, program: &ProgramObj) -> Result<Linked, String> {
    let Some((vertex, fragment)) = program.shaders.borrow_mut().take() else {
        return Err("the link already failed".into());
    };
    let raw = program.raw;
    if !platform::linked(gl, raw) {
        let mut log = platform::program_log(gl, raw);
        for s in core::iter::once(&vertex).chain(&fragment) {
            if !platform::compiled(gl, s.raw) {
                log.push_str(&platform::shader_log(gl, s.raw));
                log.push_str(&s.source);
            }
        }
        return Err(log);
    }
    gl.use_program(raw);
    let mut units = Vec::new();
    let mut sizes = Vec::new();
    let mut blocks: Vec<Slot> = Vec::new();
    let mut immediates = Vec::new();
    for s in core::iter::once(&vertex).chain(&fragment) {
        for (name, offset, ty) in &s.immediates {
            if let Some(location) = platform::location(gl, raw, name) {
                immediates.push((location, *offset, *ty));
            }
        }
        for Combined {
            name,
            texture,
            sampler,
            level_zero,
        } in &s.textures
        {
            let location = platform::location(gl, raw, name);
            let size = platform::location(gl, raw, &format!("{name}_size"))
                .map(|l| (l, std::cell::Cell::new([-1; 3])));
            // A texture only sized has no sampler uniform left, but still takes a unit so its
            // size uniform follows the bound view.
            if location.is_none() && size.is_none() {
                continue;
            }
            if let Some(location) = location {
                gl.uniform1i(location, units.len() as i32);
            }
            units.push((*texture, *sampler, *level_zero));
            sizes.push(size);
        }
        for (name, slot) in &s.blocks {
            if let Some(index) = platform::block_index(gl, raw, name) {
                let point = match blocks.iter().position(|b| b == slot) {
                    Some(p) => p,
                    None => {
                        blocks.push(*slot);
                        blocks.len() - 1
                    }
                };
                gl.uniform_block_binding(raw, index, point as u32);
            }
        }
    }
    let uniform = |name| platform::location(gl, raw, name).map(|l| (l, std::cell::Cell::new(0)));
    Ok(Linked {
        units,
        sizes,
        blocks,
        first_instance: uniform(glsl::FIRST_INSTANCE_BINDING),
        base_vertex: uniform(glsl::BASE_VERTEX_BINDING),
        immediates,
        held: Default::default(),
    })
}

/// Sets the uniforms standing for the program's immediates from `bytes`, unless they hold them
/// already. Bytes past the end of `bytes` read as zero, as GL's uniforms start. The program is in use.
pub(crate) fn set_immediates(gl: &impl OpenGlEs, linked: &Linked, bytes: &[u8]) {
    use glsl::{GlslScalarKind as Kind, GlslUniformType as Ty};
    let mut held = linked.held.borrow_mut();
    if linked.immediates.is_empty() || *held == bytes {
        return;
    }
    held.clear();
    held.extend_from_slice(bytes);
    let word = |at: usize| {
        bytes
            .get(at..at + 4)
            .map_or([0; 4], |w| [w[0], w[1], w[2], w[3]])
    };
    for (location, offset, ty) in &linked.immediates {
        let at = *offset as usize;
        let l = *location;
        match *ty {
            Ty::Scalar(s) | Ty::Vector { scalar: s, .. } => {
                let n = match *ty {
                    Ty::Vector { size, .. } => size as usize,
                    _ => 1,
                };
                let words: [[u8; 4]; 4] = core::array::from_fn(|i| word(at + 4 * i));
                match s.kind {
                    Kind::Float => {
                        let v = words.map(f32::from_le_bytes);
                        let v = &v[..n];
                        match n {
                            1 => gl.uniform1fv(l, v),
                            2 => gl.uniform2fv(l, v),
                            3 => gl.uniform3fv(l, v),
                            _ => gl.uniform4fv(l, v),
                        }
                    }
                    Kind::Sint => {
                        let v = words.map(i32::from_le_bytes);
                        let v = &v[..n];
                        match n {
                            1 => gl.uniform1iv(l, v),
                            2 => gl.uniform2iv(l, v),
                            3 => gl.uniform3iv(l, v),
                            _ => gl.uniform4iv(l, v),
                        }
                    }
                    Kind::Uint => {
                        let v = words.map(u32::from_le_bytes);
                        let v = &v[..n];
                        match n {
                            1 => gl.uniform1uiv(l, v),
                            2 => gl.uniform2uiv(l, v),
                            3 => gl.uniform3uiv(l, v),
                            _ => gl.uniform4uiv(l, v),
                        }
                    }
                }
            }
            Ty::Matrix { columns, rows, .. } => {
                // Columns are vectors of `rows`, each aligned as such a vector; GL takes them packed.
                let (c, r) = (columns as usize, rows as usize);
                let stride = if r == 2 { 8 } else { 16 };
                let mut v = [0f32; 16];
                for col in 0..c {
                    for row in 0..r {
                        v[col * r + row] = f32::from_le_bytes(word(at + col * stride + 4 * row));
                    }
                }
                let v = &v[..c * r];
                match (c, r) {
                    (2, 2) => gl.uniform_matrix2fv(l, false, v),
                    (3, 3) => gl.uniform_matrix3fv(l, false, v),
                    (4, 4) => gl.uniform_matrix4fv(l, false, v),
                    (2, 3) => gl.uniform_matrix2x3fv(l, false, v),
                    (3, 2) => gl.uniform_matrix3x2fv(l, false, v),
                    (2, 4) => gl.uniform_matrix2x4fv(l, false, v),
                    (4, 2) => gl.uniform_matrix4x2fv(l, false, v),
                    (3, 4) => gl.uniform_matrix3x4fv(l, false, v),
                    _ => gl.uniform_matrix4x3fv(l, false, v),
                }
            }
        }
    }
}

/// Attributes GL ES cannot read as stored are read as something it can, and turned into what the
/// program reads at the start of `main`: per location, two bits of `convert` (see
/// [`crate::state::conversion`]). Attributes the program does not read stay as they are. None when
/// the source has no `main` as naga writes it.
pub(crate) fn convert_inputs(source: &str, convert: u32) -> Option<String> {
    const SMALL_FLOAT: &str = "float _wc_small_float(uint b, uint m) {
    uint e = b >> m;
    uint f = b & ((1u << m) - 1u);
    float scale = float(1u << m);
    if (e == 0u) { return float(f) / scale * exp2(-14.0); }
    if (e == 31u) { return uintBitsToFloat(f == 0u ? 0x7f800000u : 0x7fc00000u); }
    return exp2(float(e) - 15.0) * (1.0 + float(f) / scale);
}
";
    let mut out = String::with_capacity(source.len() + 1024);
    let mut starts = String::new();
    for line in source.lines() {
        let declared = (0..16u32).find(|l| {
            convert >> (2 * l) & 3 != 0
                && line.starts_with(&format!("layout(location = {l}) in "))
                && line.ends_with(&format!(" _p2vs_location{l};"))
        });
        let Some(l) = declared else {
            out.push_str(line);
            out.push('\n');
            if line == "void main() {" {
                out.push_str(&starts);
            }
            continue;
        };
        let ty = line
            .strip_prefix(&format!("layout(location = {l}) in "))?
            .strip_suffix(&format!(" _p2vs_location{l};"))?;
        let (raw, value) = match convert >> (2 * l) & 3 {
            1 => ("vec4", format!("_wc_raw{l}.zyxw")),
            2 => (
                "uint",
                format!(
                    "uvec4(_wc_raw{l} & 1023u, (_wc_raw{l} >> 10) & 1023u, \
                     (_wc_raw{l} >> 20) & 1023u, _wc_raw{l} >> 30)"
                ),
            ),
            _ => (
                "uint",
                format!(
                    "vec4(_wc_small_float(_wc_raw{l} & 2047u, 6u), \
                     _wc_small_float((_wc_raw{l} >> 11) & 2047u, 6u), \
                     _wc_small_float(_wc_raw{l} >> 22, 5u), 1.0)"
                ),
            ),
        };
        out.push_str(&format!(
            "layout(location = {l}) in {raw} _wc_raw{l};\n{ty} _p2vs_location{l};\n"
        ));
        if convert >> (2 * l) & 3 == 3 && !out.contains("_wc_small_float(uint") {
            out.push_str(SMALL_FLOAT);
        }
        starts.push_str(&format!("    _p2vs_location{l} = {ty}({value});\n"));
    }
    out.contains("void main() {").then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A vertex program's clip distances are counted, and written as `gl_ClipDistance` under
    /// WEBGL_clip_cull_distance's directive.
    #[test]
    fn clip_distances_are_written_and_counted() {
        let wgsl = "
            enable clip_distances;
            struct Out {
                @builtin(position) p: vec4<f32>,
                @builtin(clip_distances) d: array<f32, 2>,
            }
            @vertex fn vs(@location(0) p: vec4<f32>) -> Out {
                return Out(p, array<f32, 2>(p.x, p.y));
            }";
        let module = naga::front::wgsl::parse_str(wgsl).expect("parses");
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("validates");
        let module = ShaderModule {
            module,
            info,
            entry_point: "vs".into(),
            stage: ShaderStage::Vertex,
            key: [0; 32],
        };
        assert_eq!(needs(&module), Features::CLIP_DISTANCE);
        let distances: Vec<_> = results(&module)
            .filter(|(b, _)| *b == naga::Binding::BuiltIn(naga::BuiltIn::ClipDistances))
            .collect();
        assert_eq!(distances.len(), 1);
        assert_eq!(distances[0].1, 2);
        let (glsl, _, _) = write_glsl(&module, GlslSettings::default()).expect("writes");
        assert!(
            glsl.contains("#extension GL_ANGLE_clip_cull_distance"),
            "{glsl}"
        );
        assert!(glsl.contains("gl_ClipDistance = _tmp_return.d;"), "{glsl}");
    }

    /// A depth texture compared against and read plainly, in the entry point, a function and a
    /// loop, writes as GLSL ES 3.00: the plain reads go through a float copy at a binding of its own
    /// standing for the texture's, the comparison stays on the shadow sampler.
    #[test]
    fn plain_depth_reads_write_through_a_float_copy() {
        let wgsl = "
            @group(0) @binding(16) var s: sampler;
            @group(0) @binding(17) var c: sampler_comparison;
            @group(0) @binding(32) var t: texture_depth_2d;
            fn plain(uv: vec2<f32>) -> f32 { return textureSampleLevel(t, s, uv, 0); }
            @fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
                var sum = textureSampleCompare(t, c, p.xy, 0.5);
                for (var i = 0; i < 2; i++) {
                    sum += textureLoad(t, vec2<i32>(p.xy) + i, 0);
                }
                let g = textureGather(t, s, p.xy, vec2<i32>(1, -1));
                return vec4<f32>(sum, plain(p.xy), textureSample(t, s, p.zw), g.x);
            }";
        let module = ShaderModule::from_wgsl(wgsl, "fs", ShaderStage::Fragment).expect("validates");
        assert!(
            write_glsl(&module, GlslSettings::default()).is_err(),
            "naga writes it as it is"
        );
        let (plain, own) = plain_depth_reads(&module)
            .expect("rewrites")
            .expect("reads plainly");
        assert_eq!(own, [((0, PLAIN_DEPTH_BINDINGS), (0, 32))]);
        let (glsl, _, _) = write_glsl(&plain, GlslSettings::default()).expect("writes");
        assert!(glsl.contains("sampler2DShadow"), "{glsl}");
        assert!(glsl.contains("texelFetch("), "{glsl}");
        assert_eq!(glsl.matches("_wc_plain_depth_").count(), 8, "{glsl}");
    }

    /// The attributes a layout converts are declared as GL reads them and turned into what the
    /// program reads at the start of `main`; others stay.
    #[test]
    fn converted_inputs_are_turned_into_their_values() {
        let wgsl = "
            @vertex fn vs(@location(0) p: vec4<f32>, @location(1) c: vec4<f32>,
                          @location(2) u: vec4<u32>, @location(3) f: vec3<f32>)
                -> @builtin(position) vec4<f32> {
                return p + c + vec4<f32>(u) + vec4<f32>(f, 0.0);
            }";
        let module = ShaderModule::from_wgsl(wgsl, "vs", ShaderStage::Vertex).expect("validates");
        let (glsl, _, _) = write_glsl(&module, GlslSettings::default()).expect("writes");
        let convert = 1 << 2 | 2 << 4 | 3 << 6;
        let out = convert_inputs(&glsl, convert).expect("converts");
        for line in [
            "layout(location = 0) in vec4 _p2vs_location0;",
            "layout(location = 1) in vec4 _wc_raw1;",
            "layout(location = 2) in uint _wc_raw2;",
            "layout(location = 3) in uint _wc_raw3;",
            "uvec4 _p2vs_location2;",
            "    _p2vs_location1 = vec4(_wc_raw1.zyxw);",
        ] {
            assert!(out.lines().any(|l| l == line), "{line} in\n{out}");
        }
        assert_eq!(out.matches("float _wc_small_float(").count(), 1);
        let main = out.find("void main() {").expect("main");
        assert!(out[main..].contains("_p2vs_location3 = vec3(vec4(_wc_small_float("));
    }

    /// Linear values are carried times w beside the values themselves and divided by the w at
    /// the same place; locations sharing a prefix stay apart.
    #[test]
    fn linear_values_are_carried_times_w() {
        let wgsl = "
            struct V {
                @builtin(position) p: vec4<f32>,
                @location(1) @interpolate(linear) a: vec2<f32>,
                @location(10) @interpolate(linear, centroid) b: f32,
                @location(2) c: f32,
            }
            @vertex fn vs(@location(0) p: vec4<f32>) -> V { return V(p, p.xy, p.z, p.w); }
            @fragment fn fs(v: V) -> @location(0) vec4<f32> { return vec4<f32>(v.a, v.b, v.c); }";
        let write = |entry: &str, stage| {
            let module = ShaderModule::from_wgsl(wgsl, entry, stage).expect("validates");
            write_glsl(&module, GlslSettings::default())
                .expect("writes")
                .0
        };
        let vs = write("vs", ShaderStage::Vertex);
        for line in [
            "smooth out vec2 _vs2fs_location1;",
            "smooth out vec2 _wc_linear1;",
            "smooth centroid out float _wc_linear10;",
            "centroid out float _wc_linear_w_centroid;",
            "    _wc_linear1 = _vs2fs_location1 * gl_Position.w;",
            "    _wc_linear10 = _vs2fs_location10 * gl_Position.w;",
            "    _wc_linear_w_centroid = gl_Position.w;",
        ] {
            assert!(vs.lines().any(|l| l == line), "{line} in\n{vs}");
        }
        assert!(!vs.contains("_wc_linear2"), "{vs}");
        let fs = write("fs", ShaderStage::Fragment);
        assert!(!fs.contains("_vs2fs_location1;"), "{fs}");
        assert!(!fs.contains("_vs2fs_location10"), "{fs}");
        for text in [
            "smooth in vec2 _wc_linear1;",
            "smooth centroid in float _wc_linear10;",
            "centroid in float _wc_linear_w_centroid;",
            "(_wc_linear1 * gl_FragCoord.w)",
            "(_wc_linear10 / _wc_linear_w_centroid)",
            "_vs2fs_location2",
        ] {
            assert!(fs.contains(text), "{text} in\n{fs}");
        }
    }

    /// With NV_shader_noperspective_interpolation linear values are declared `noperspective` and
    /// read as they are; values sampled per sample are declared `sample` on both sides, under
    /// their directives, carried times w beside a per-sample w where they are linear too.
    #[test]
    fn linear_and_per_sample_values_take_their_qualifiers() {
        let wgsl = "
            struct V {
                @builtin(position) p: vec4<f32>,
                @location(1) @interpolate(linear) a: vec2<f32>,
                @location(3) @interpolate(perspective, sample) b: f32,
                @location(4) @interpolate(linear, sample) c: f32,
            }
            @vertex fn vs(@location(0) p: vec4<f32>) -> V { return V(p, p.xy, p.z, p.w); }
            @fragment fn fs(v: V) -> @location(0) vec4<f32> { return vec4<f32>(v.a, v.b, v.c); }";
        let parsed = naga::front::wgsl::parse_str(wgsl).expect("parses");
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&parsed)
        .expect("validates");
        let write = |entry: &str, stage, noperspective| {
            let module = ShaderModule {
                module: parsed.clone(),
                info: info.clone(),
                entry_point: entry.into(),
                stage,
                key: [0; 32],
            };
            let settings = GlslSettings {
                flip_y: false,
                noperspective,
            };
            write_glsl(&module, settings).expect("writes").0
        };
        let sample = format!("#extension {SAMPLE_QUALIFIER_EXTENSION} : require");
        let linear = format!("#extension {NOPERSPECTIVE_EXTENSION} : require");
        let vs = write("vs", ShaderStage::Vertex, true);
        for line in [
            sample.as_str(),
            linear.as_str(),
            "noperspective out vec2 _vs2fs_location1;",
            "smooth sample out float _vs2fs_location3;",
            "noperspective sample out float _vs2fs_location4;",
        ] {
            assert!(vs.lines().any(|l| l == line), "{line} in\n{vs}");
        }
        assert!(!vs.contains(CARRIED), "{vs}");
        let fs = write("fs", ShaderStage::Fragment, true);
        for line in [
            "noperspective in vec2 _vs2fs_location1;",
            "smooth sample in float _vs2fs_location3;",
            "noperspective sample in float _vs2fs_location4;",
        ] {
            assert!(fs.lines().any(|l| l == line), "{line} in\n{fs}");
        }
        let fs = write("fs", ShaderStage::Fragment, false);
        assert!(!fs.contains(&linear), "{fs}");
        for text in [
            "smooth sample in float _wc_linear4;",
            "sample in float _wc_linear_w_sample;",
            "(_wc_linear4 / _wc_linear_w_sample)",
            "(_wc_linear1 * gl_FragCoord.w)",
        ] {
            assert!(fs.contains(text), "{text} in\n{fs}");
        }
    }

    /// Floats from bits and negative zero literals read their bits through the uniform zero;
    /// global constants and other numbers stay.
    #[test]
    fn float_bits_are_kept_from_folding() {
        let source = "#version 300 es\nprecision highp float;\nconst float k = -0.0;\n\
                      void main() { a = uintBitsToFloat(uvec4(1u)); b = intBitsToFloat(i); \
                      c = vec2(-0.0, -0.05); d = myuintBitsToFloat(x); e = 1e-0; }\n";
        let out = opaque_float_bits(source.into());
        let main = &out[out.find("void main").expect("main")..];
        assert_eq!(
            main,
            "void main() { a = _wc_from_bits(uvec4(1u)); b = _wc_from_int_bits(i); \
             c = vec2(_wc_from_bits(0x80000000u), -0.05); d = myuintBitsToFloat(x); e = 1e-0; }\n"
        );
        assert!(out.contains("const float k = -0.0;"), "{out}");
        assert!(out.contains("uniform highp uint _wc_zero;"), "{out}");
        assert!(
            out.contains("vec4 _wc_from_bits(uvec4 v) { return uintBitsToFloat(v ^ _wc_zero); }"),
            "{out}"
        );
        let plain = "#version 300 es\nvoid main() { a = 1.0 - 0.0; }\n";
        assert_eq!(opaque_float_bits(plain.into()), plain);
    }

    #[test]
    fn gathers_name_their_whole_first_argument() {
        let source = "#version 300 es\nvoid main() { v = textureGather(s[f(1, 2)], p, 0); \
                      w = textureGatherOffset(t, q, ivec2(1, 0), 2); }\n";
        let out = gathers_by_samples(source.into());
        assert!(
            out.contains("_wc_gather(s[f(1, 2)], textureSize(s[f(1, 2)], 0), p, 0)"),
            "{out}"
        );
        assert!(
            out.contains("_wc_gather_offset(t, textureSize(t, 0), q, ivec2(1, 0), 2)"),
            "{out}"
        );
    }
}
