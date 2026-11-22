//! Converts every shader of the corpora and writes each result as WGSL and GLSL ES 3.00, checking the
//! WGSL by reading and validating it again and the GLSL with glslangValidator where it is installed.
//!
//! Each shader's verdict must equal the one recorded in `tests/expected/<corpus>.txt`.
//!
//! Corpora: the rig's compiled Direct3D shaders in `wcontainers-graphics-test/shaders/{dxbc,dxil}`, this
//! crate's own in `tests/own`, and (an ignored test) the directory in `WCONTAINERS_SHADER_CORPUS` (the
//! plan's 191 DXBC blobs from Wine's tests, kept outside the repository). `WCONTAINERS_SHADERS_OUT` names
//! a directory for the written files and a verdict per shader in the expected files' form
//! (`verdicts.txt`); with it, `GLSLANG` names the glslangValidator to run.
//!
//! Every converted shader also goes through the cache's record (plan section 8) under each variant
//! key a front asks for today, and must come back to the same bytes; `cache.txt` lists each key and
//! a hash of its record, so builds (native, WebAssembly) can be compared.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use wcontainers_gpu::{
    DrawBaseSource, Format, ShaderModule, SlotKind, SystemValue, TexelBufferRoute, VariantFlags,
    VariantKey, VertexFormat,
};
use wcontainers_shaders::{ConvertError, Converted, Source, cache, capabilities, convert, stage};

fn shaders(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    v.retain(|p| p.extension().is_some_and(|e| e == ext));
    v.sort();
    v
}

/// The keys a shader with storage views or typed buffers is converted with, the corpus having no
/// bound views: every storage and texture slot a view of one 32-bit format (which WebGPU reads and
/// writes) of each kind, typed buffers through storage buffers.
fn view_keys() -> [(Format, VariantKey); 3] {
    [Format::R32Float, Format::R32Uint, Format::R32Sint].map(|format| {
        let key = VariantKey {
            storage_formats: (0..64).map(|s| (s, format)).collect(),
            texel_formats: (0..128).map(|s| (s, format)).collect(),
            texel_buffers: TexelBufferRoute::Storage,
            ..VariantKey::default()
        };
        (format, key)
    })
}

/// The key a front asks for on WebGL 2 where it asks `key` on WebGPU: texture level and sample counts
/// from the helpers (no `Features::TEXTURE_LEVEL_QUERY` there).
fn webgl_key(key: &VariantKey) -> VariantKey {
    VariantKey {
        flags: key.flags | VariantFlags::TEXTURE_INFO,
        ..key.clone()
    }
}

/// One line per variant key a front asks for today under which the shader converts: the key's name,
/// the content key and the BLAKE3 hash of the cache record. Fails if a record does not decode to
/// the same record or converting again gives other bytes.
fn records(path: &Path, file: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).unwrap();
    let source = if path.extension().is_some_and(|e| e == "dxil") {
        Source::Dxil(&bytes)
    } else {
        Source::Dxbc(&bytes)
    };
    let mut keys = vec![
        ("plain".to_string(), VariantKey::default()),
        ("webgl".to_string(), webgl_key(&VariantKey::default())),
        (
            "draw-base-helpers".to_string(),
            VariantKey {
                draw_base: DrawBaseSource::Helpers,
                ..VariantKey::default()
            },
        ),
    ];
    if let Err(ConvertError::Unsupported(
        wcontainers_shaders::refusal::STORAGE_FORMAT | wcontainers_shaders::refusal::TYPED_BUFFER,
    )) = convert(source, &VariantKey::default(), None)
    {
        keys.extend(view_keys().map(|(f, k)| (format!("views-{f:?}"), k)));
    }
    let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    let mut lines = String::new();
    for (name, variant) in &keys {
        let Ok(converted) = convert(source, variant, None) else {
            continue;
        };
        let key = cache::shader_key(source, variant);
        let record = cache::encode(&converted);
        let again = cache::encode(&convert(source, variant, None).unwrap());
        if again != record {
            return Err(format!(
                "{file} {name}: converting again gave another record"
            ));
        }
        let decoded = cache::decode(key, &record).map_err(|e| format!("{file} {name}: {e}"))?;
        if cache::encode(&decoded) != record || decoded.modules[0].key != key {
            return Err(format!(
                "{file} {name}: the record did not come back the same"
            ));
        }
        lines.push_str(&format!(
            "{file}\t{name}\t{}\t{}\n",
            hex(&key),
            hex(blake3::hash(&record).as_bytes())
        ));
    }
    Ok(lines)
}

/// The verdict for one shader: "ok" or the first failing step and its reason.
fn verdict(path: &Path, out: Option<&Path>) -> String {
    let bytes = std::fs::read(path).unwrap();
    let source = |b| {
        if path.extension().is_some_and(|e| e == "dxil") {
            Source::Dxil(b)
        } else {
            Source::Dxbc(b)
        }
    };
    let named = |e: ConvertError| match e {
        ConvertError::Unsupported(what) => format!("{UNSUPPORTED}{what}"),
        ConvertError::Reader(what) => format!("spirv-reader {what}"),
        e => format!("convert {e}"),
    };
    // A shader with storage views or typed buffers converts with the bound views' formats, which
    // the corpus does not have: it passes with the first of `view_keys` that converts. Where it runs
    // atomics on storage textures, they are read through buffers, as the fronts then ask.
    let mut key = VariantKey::default();
    let mut views = None;
    let converted = match convert(source(&bytes), &key, None) {
        Ok(c) => c,
        Err(ConvertError::Unsupported(
            wcontainers_shaders::refusal::STORAGE_FORMAT
            | wcontainers_shaders::refusal::TYPED_BUFFER
            | wcontainers_shaders::refusal::STORAGE_IMAGE_ATOMICS,
        )) => {
            let mut last = None;
            for (format, mut k) in view_keys() {
                let mut made = convert(source(&bytes), &k, None);
                if let Err(ConvertError::Unsupported(
                    wcontainers_shaders::refusal::STORAGE_IMAGE_ATOMICS,
                )) = made
                {
                    k.texels_through_buffer = u64::MAX;
                    made = convert(source(&bytes), &k, None);
                }
                match made {
                    Ok(c) => {
                        last = Some(Ok(c));
                        key = k;
                        views = Some(format);
                        break;
                    }
                    Err(e) => {
                        if last.is_none() {
                            last = Some(Err(e));
                        }
                    }
                }
            }
            match last.unwrap() {
                Ok(c) => c,
                Err(e) => return format!("{} (with views)", named(e)),
            }
        }
        Err(e) => return named(e),
    };
    let ok = match (views, key.texels_through_buffer != 0) {
        (Some(f), false) => format!("ok views:{f:?}"),
        (Some(f), true) => format!("ok views:{f:?} through"),
        (None, _) => "ok".into(),
    };
    let m = &converted.modules[0];
    let name = path.file_stem().unwrap().to_string_lossy().into_owned();
    // A vertex shader writing cull distances or storage reaches WebGPU through the stage helper: the
    // WGSL written is its compute form's, converted with every input pulled.
    let drawn = |converted: &Converted| -> Result<ShaderModule, String> {
        let (route, programs): (_, fn(&Converted) -> _) = if stage::writes_storage(converted) {
            (" (storage route)", stage::stored_programs)
        } else if writes_cull_distances(converted) {
            (" (cull route)", stage::cull_programs)
        } else {
            return Ok(converted.modules[0].clone());
        };
        let k = VariantKey {
            pulled: pulled_inputs(converted),
            ..key.clone()
        };
        let pulled = convert(source(&bytes), &k, None).map_err(|e| named(e) + route)?;
        programs(&pulled)
            .map(|p| p.compute)
            .map_err(|e| named(e) + route)
    };
    let w = match drawn(&converted) {
        Ok(w) => w,
        Err(e) => return e,
    };
    let wgsl = match naga::back::wgsl::write_string(
        &w.module,
        &w.info,
        naga::back::wgsl::WriterFlags::empty(),
    ) {
        Ok(s) => s,
        Err(e) => return format!("wgsl-write {e}"),
    };
    if let Some(out) = out {
        std::fs::write(out.join(format!("{name}.wgsl")), &wgsl).unwrap();
    }
    // The converted module is a function of the source and the variant alone.
    let again = convert(source(&bytes), &key, None).unwrap();
    let w2 = drawn(&again).unwrap();
    let wgsl2 = naga::back::wgsl::write_string(
        &w2.module,
        &w2.info,
        naga::back::wgsl::WriterFlags::empty(),
    )
    .unwrap();
    if wgsl2 != wgsl || w2.key != w.key || again.modules[0].key != m.key {
        return "convert nondeterministic".into();
    }
    let reparsed = match naga::front::wgsl::parse_str(&wgsl) {
        Ok(m) => m,
        Err(e) => return format!("wgsl-parse {}", e.message()),
    };
    if let Err(e) =
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), capabilities(&w.module))
            .validate(&reparsed)
    {
        return format!("wgsl-validate {}", e.as_inner());
    }

    // GLSL ES is written from the module the WebGL 2 back end's front asks for, whose language
    // cannot ask texture level and sample counts, by that back end's own writing.
    #[cfg(not(target_arch = "wasm32"))]
    let ok = {
        let glsl = match convert(source(&bytes), &webgl_key(&key), None) {
            Ok(webgl) => glsl_verdict(&webgl.modules[0], &name, out),
            Err(e) => format!("{KEY_FAILED}{}", named(e)),
        };
        format!("{ok}{GLSL_PART}{glsl}")
    };
    ok
}

/// Where a verdict's GLSL ES part begins; a WebAssembly build checks only what comes before it.
const GLSL_PART: &str = " ; glsl ";

/// The GLSL ES outcome naming that the step was not run (no `GLSLANG`): it stands for any compile
/// outcome in a record.
const WRITTEN: &str = "written";

/// The need a cube array view names beside the back end's features.
#[cfg(not(target_arch = "wasm32"))]
const CUBE_ARRAY_VIEW: &str = "CUBE_ARRAY_VIEW";

/// The GLSL ES outcomes from glslang; a refusal is followed by glslang's first error.
const COMPILES: &str = "compiles";
const COMPILE_FAILED: &str = "compile ";

/// glslang's word before each error, and the line glslang ends a refused file with.
#[cfg(not(target_arch = "wasm32"))]
const GLSLANG_ERROR: &str = "ERROR: ";
#[cfg(not(target_arch = "wasm32"))]
const GLSLANG_TERMINATED: &str = "compilation terminated";

/// The WebGL 2 back end's verdict on one module: the features it needs beyond the core language
/// (`needs NAME|NAME`, absent from the part where none), then `absent compute`, `written`,
/// `compiles`, `compile` with glslang's first error, `write` with the back end's reason and naga's,
/// or `undeclared` with the directives the written file lacks.
#[cfg(not(target_arch = "wasm32"))]
fn glsl_verdict(m: &ShaderModule, name: &str, out: Option<&Path>) -> String {
    use wcontainers_gpu_webgl::glsl;
    if m.stage == wcontainers_gpu::ShaderStage::Compute {
        return "absent compute".into();
    }
    let needs = glsl::needs(m);
    let mut names: Vec<&str> = needs.iter_names().map(|(n, _)| n).collect();
    // A cube array view, which the interface has no feature for and WebGL 2 cannot make.
    let cube_array = m.module.types.iter().any(|(_, t)| {
        matches!(
            t.inner,
            naga::TypeInner::Image {
                dim: naga::ImageDimension::Cube,
                arrayed: true,
                ..
            }
        )
    });
    if cube_array {
        names.push(CUBE_ARRAY_VIEW);
    }
    let needed = if names.is_empty() {
        String::new()
    } else {
        format!("{NEEDS}{} ", names.join("|"))
    };
    // A context with EXT_clip_control and NV_shader_noperspective_interpolation, as Chrome's on the
    // RTX 3080.
    let settings = glsl::GlslSettings {
        flip_y: false,
        noperspective: true,
    };
    let text = match glsl::write(m, settings) {
        Ok(t) => t,
        Err(e) => return format!("{needed}{WRITE_FAILED}{e:?}{}", naga_refusal(m)),
    };
    let missing = undeclared(&names, &text.source);
    if !missing.is_empty() {
        return format!("{needed}{UNDECLARED}{}", missing.join("|"));
    }
    let (Some(out), Ok(glslang)) = (out, std::env::var("GLSLANG")) else {
        return format!("{needed}{WRITTEN}");
    };
    let ext = if m.stage == wcontainers_gpu::ShaderStage::Vertex {
        "vert"
    } else {
        "frag"
    };
    let file = out.join(format!("{name}.{ext}"));
    std::fs::write(&file, &text.source).unwrap();
    let run = std::process::Command::new(glslang)
        .arg(&file)
        .output()
        .unwrap();
    if run.status.success() {
        return format!("{needed}{COMPILES}");
    }
    let mut log = file.into_os_string();
    log.push(GLSLANG_LOG);
    std::fs::write(log, &run.stdout).unwrap();
    format!(
        "{needed}{COMPILE_FAILED}{}",
        glslang_error(&String::from_utf8_lossy(&run.stdout))
    )
}

/// What written GLSL ES `text` lacks for the needs in `names` that the WebGL 2 back end declares
/// through an extension: each directive a form of the need shows in the text and the text does not
/// carry.
#[cfg(not(target_arch = "wasm32"))]
fn undeclared(names: &[&str], text: &str) -> Vec<String> {
    let carried = |d: &str| text.contains(&format!("{EXTENSION_DIRECTIVE}{d}{EXTENSION_REQUIRED}"));
    let mut missing = Vec::new();
    for (_, forms) in DIRECTIVES.iter().filter(|(n, _)| names.contains(n)) {
        let shown = forms.iter().filter(|(form, _)| text.contains(form));
        missing.extend(
            shown
                .filter(|(_, d)| !carried(d))
                .map(|(_, d)| d.to_string()),
        );
    }
    missing.sort_unstable();
    missing.dedup();
    missing
}

/// The needs the WebGL 2 back end declares through an extension, each with the text its GLSL ES shows
/// it by and the extension that text requires: OES_sample_variables's built-ins and
/// EXT_blend_func_extended's second source, both written by naga. The `sample` qualifier's
/// directive is the back end's own constant, which it does not export, so it is not checked here.
const DIRECTIVES: [(&str, &[(&str, &str)]); 2] = [
    (
        "SAMPLE_SHADING",
        &[
            ("gl_Sample", SAMPLE_VARIABLES),
            ("gl_NumSamples", SAMPLE_VARIABLES),
        ],
    ),
    ("DUAL_SOURCE_BLEND", &[("index = 1)", BLEND_FUNC_EXTENDED)]),
];
const SAMPLE_VARIABLES: &str = "GL_OES_sample_variables";
const BLEND_FUNC_EXTENDED: &str = "GL_EXT_blend_func_extended";
/// An extension directive's line around the extension's name.
#[cfg(not(target_arch = "wasm32"))]
const EXTENSION_DIRECTIVE: &str = "#extension ";
#[cfg(not(target_arch = "wasm32"))]
const EXTENSION_REQUIRED: &str = " : require";

/// naga's own reason for refusing to write `m` as GLSL ES 3.00 for WebGL, as the back end's options
/// ask, where it refuses (the back end's refusal says only that naga refused); empty where it writes.
#[cfg(not(target_arch = "wasm32"))]
fn naga_refusal(m: &ShaderModule) -> String {
    use naga::back::glsl;
    let shader_stage = if m.stage == wcontainers_gpu::ShaderStage::Vertex {
        naga::ShaderStage::Vertex
    } else {
        naga::ShaderStage::Fragment
    };
    let options = glsl::Options {
        // GLSL ES 3.00, WebGL 2's language: the first naga writes.
        version: glsl::Version::Embedded {
            version: glsl::SUPPORTED_ES_VERSIONS[0],
            is_webgl: true,
        },
        writer_flags: glsl::WriterFlags::FORCE_POINT_SIZE,
        binding_map: Default::default(),
        zero_initialize_workgroup_memory: false,
    };
    let pipeline = glsl::PipelineOptions {
        shader_stage,
        entry_point: m.entry_point.clone(),
        multiview: None,
    };
    let mut text = String::new();
    let written = glsl::Writer::new(
        &mut text,
        &m.module,
        &m.info,
        &options,
        &pipeline,
        naga::proc::BoundsCheckPolicies::default(),
    )
    .and_then(|mut w| w.write().map(drop));
    match written {
        Ok(()) => String::new(),
        Err(e) => format!(
            "{NAGA_REFUSAL}{}",
            e.to_string()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        ),
    }
}

/// Where a write refusal's record gives naga's own reason.
#[cfg(not(target_arch = "wasm32"))]
const NAGA_REFUSAL: &str = "; naga alone: ";

/// glslang's first error without its place in the file (`ERROR: 0:12: 'x' : undeclared identifier`
/// gives `'x' : undeclared identifier`), its spaces made single.
#[cfg(not(target_arch = "wasm32"))]
fn glslang_error(output: &str) -> String {
    let first = output
        .lines()
        .filter_map(|l| l.strip_prefix(GLSLANG_ERROR))
        .find(|e| !e.contains(GLSLANG_TERMINATED))
        .unwrap_or_default();
    let place = |w: &str| {
        w.trim_end_matches(':')
            .split(':')
            .all(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    };
    let mut words = first.split_whitespace().peekable();
    if words.peek().is_some_and(|w| place(w)) {
        words.next();
    }
    words.collect::<Vec<_>>().join(" ")
}

/// Appended to a GLSL file's name for glslang's output where it refuses the file.
#[cfg(not(target_arch = "wasm32"))]
const GLSLANG_LOG: &str = ".log";

/// What a record says as far as this run checks: without glslang, a compile outcome reads as
/// written; in a WebAssembly build, the GLSL ES part is not checked at all.
fn as_checked(record: &str, glslang: bool) -> String {
    let Some((wgsl, glsl)) = record.split_once(GLSL_PART) else {
        return record.into();
    };
    if cfg!(target_arch = "wasm32") {
        return wgsl.into();
    }
    if glslang {
        return record.into();
    }
    let (needed, outcome) = split_needs(glsl);
    if outcome == COMPILES || outcome.starts_with(COMPILE_FAILED) {
        format!("{wgsl}{GLSL_PART}{needed}{WRITTEN}")
    } else {
        record.into()
    }
}

/// A verdict's class for the counts: the conversion's outcome (with a refusal's name), then the GLSL
/// ES step's outcome and whether the shader needs features beyond the core language.
fn class_of(verdict: &str) -> String {
    let (wgsl, glsl) = verdict.split_once(GLSL_PART).unwrap_or((verdict, ""));
    let mut class: Vec<&str> = wgsl.split_whitespace().take(2).collect();
    if class.first() == Some(&"ok") {
        class.truncate(1);
    }
    let (needed, outcome) = split_needs(glsl);
    if !glsl.is_empty() {
        class.push(GLSL_PART.trim());
    }
    if !needed.is_empty() {
        class.push(NEEDS.trim());
    }
    class.extend(outcome.split_whitespace().next());
    class.join(" ")
}

/// A verdict names its outcome: a conversion, or a refusal by name. A GLSL ES compile failure only
/// where the shader needs a feature the WebGL 2 back end never declares, or one it declares through
/// an extension whose directive the written file carries (glslang's error, recorded, is then the
/// feature's absence there, and Chrome's compile decides where WebGL 2 has it); a write failure
/// only where it needs a feature the WebGL 2 back end never declares, so a shader it can run is
/// written; never a file lacking a directive its needs require.
fn is_named(verdict: &str) -> bool {
    let (wgsl, glsl) = verdict.split_once(GLSL_PART).unwrap_or((verdict, ""));
    let wgsl_named = ["ok", UNSUPPORTED.trim()]
        .contains(&wgsl.split_whitespace().next().unwrap_or(""))
        || wgsl.starts_with(READER_UNSUPPORTED);
    let (needed, outcome) = split_needs(glsl);
    let names = || needed.trim().strip_prefix(NEEDS).unwrap_or("").split('|');
    let never_declared = |n: &str| !n.is_empty() && !WEBGL_DECLARABLE.contains(&n);
    let by_directive = |n: &str| DIRECTIVES.iter().any(|(d, _)| *d == n);
    // glslang's error must cite a form of a need that may be absent there.
    let cited = |n: &str| {
        CITED_FORMS
            .iter()
            .filter(|(need, _)| *need == n)
            .any(|(_, forms)| forms.iter().any(|f| outcome.contains(f)))
    };
    let compile_named = !outcome.starts_with(COMPILE_FAILED)
        || names().any(|n| (never_declared(n) || by_directive(n)) && cited(n));
    let write_named = !outcome.starts_with(WRITE_FAILED) || names().any(never_declared);
    let declared = !outcome.starts_with(UNDECLARED);
    let key_named = outcome
        .strip_prefix(KEY_FAILED)
        .is_none_or(|refusal| refusal.starts_with(UNSUPPORTED));
    wgsl_named && compile_named && write_named && declared && key_named
}

/// The naga SPIR-V reader's refusal of what it does not read, as a verdict gives it (any other
/// reader error is no named outcome).
const READER_UNSUPPORTED: &str = "spirv-reader unsupported ";

/// Each need `glsl::needs` names whose absence glslang may refuse a file for, with the forms of it
/// glslang's error cites: a refusal citing none of its record's needs is no named outcome.
const CITED_FORMS: [(&str, &[&str]); 5] = [
    (
        "SAMPLE_SHADING",
        &[
            "'gl_Sample",
            "'gl_NumSamples'",
            "'sample'",
            SAMPLE_VARIABLES,
        ],
    ),
    (
        "MULTISAMPLE_READ",
        &["'sampler2DMS", "'isampler2DMS", "'usampler2DMS"],
    ),
    (
        "CLIP_DISTANCE",
        &["GL_ANGLE_clip_cull_distance", "'gl_ClipDistance'"],
    ),
    (
        "CULL_DISTANCE",
        &["GL_ANGLE_clip_cull_distance", "'gl_CullDistance'"],
    ),
    ("PRIMITIVE_INDEX", &["'gl_PrimitiveID'"]),
];

/// The features `glsl::needs` names that the WebGL 2 back end declares where its context has the
/// extension (wcontainers-gpu-webgl's caps.rs): a shader needing only these must be written.
const WEBGL_DECLARABLE: [&str; 3] = ["DUAL_SOURCE_BLEND", "POINT_SIZE", "SAMPLE_SHADING"];

/// Shaders whose recorded verdict is not a named one, each with its gap: their verdict must still
/// equal the record.
const KNOWN_UNNAMED: [(&str, &str); 1] = [(
    "hlsl+distance-4-ps.dxil",
    "gap A1: dxc 10.0.26100 writes a PPC_FP128 type record, which dxil-spirv's reader does not know",
)];

/// A verdict's GLSL ES part as its `needs` words (with their space, empty where none) and the
/// outcome after them.
fn split_needs(glsl: &str) -> (&str, &str) {
    let Some(rest) = glsl.strip_prefix(NEEDS) else {
        return ("", glsl);
    };
    let names = rest.split_once(' ').map_or(rest.len(), |(n, _)| n.len());
    glsl.split_at((NEEDS.len() + names + 1).min(glsl.len()))
}

/// The words a verdict's GLSL ES part begins with: the features needed, the back end's refusal to
/// write, and the WebGL 2 key's refusal to convert.
const NEEDS: &str = "needs ";
const WRITE_FAILED: &str = "write ";
const KEY_FAILED: &str = "key ";
/// The written file lacks the directives named after it (or shows the need named in no form).
const UNDECLARED: &str = "undeclared ";
/// A named refusal's first word.
const UNSUPPORTED: &str = "unsupported ";

fn writes_cull_distances(converted: &Converted) -> bool {
    converted
        .reflection
        .outputs
        .iter()
        .any(|v| v.system == Some(SystemValue::CullDistance))
}

/// Every vertex input pulled, in the format its reflection gives.
fn pulled_inputs(converted: &Converted) -> Vec<(u32, VertexFormat)> {
    use wcontainers_gpu::ScalarKind as K;
    converted
        .reflection
        .inputs
        .iter()
        .filter(|v| v.system.is_none())
        .map(|v| {
            let formats = match v.scalar {
                K::Float => [
                    VertexFormat::Float32x1,
                    VertexFormat::Float32x2,
                    VertexFormat::Float32x3,
                    VertexFormat::Float32x4,
                ],
                K::Uint => [
                    VertexFormat::Uint32x1,
                    VertexFormat::Uint32x2,
                    VertexFormat::Uint32x3,
                    VertexFormat::Uint32x4,
                ],
                K::Sint => [
                    VertexFormat::Sint32x1,
                    VertexFormat::Sint32x2,
                    VertexFormat::Sint32x3,
                    VertexFormat::Sint32x4,
                ],
            };
            let components = v.mask.checked_ilog2().map_or(1, |high| high + 1) as usize;
            (v.location, formats[components - 1])
        })
        .collect()
}

fn run(dir: &Path, ext: &str, label: &str) -> BTreeMap<String, usize> {
    let (counts, failures) = verdicts(dir, ext, label);
    assert!(failures.is_empty(), "{failures}");
    counts
}

/// Each shader's verdict against its record: the counts per class, and what differs (empty when
/// every verdict is the recorded one).
fn verdicts(dir: &Path, ext: &str, label: &str) -> (BTreeMap<String, usize>, String) {
    let out = std::env::var_os("WCONTAINERS_SHADERS_OUT").map(|o| PathBuf::from(o).join(label));
    if let Some(out) = &out {
        std::fs::create_dir_all(out).unwrap();
        // Compared between builds: the content key's part that names the converter.
        std::fs::write(
            out.join("converter-version.txt"),
            wcontainers_shaders::CONVERTER_VERSION,
        )
        .unwrap();
    }
    let glslang = out.is_some() && std::env::var_os("GLSLANG").is_some();
    let expected_file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/expected")
        .join(format!("{label}.txt"));
    let expected: BTreeMap<String, String> = std::fs::read_to_string(&expected_file)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .map(|(f, v)| (f.to_string(), v.to_string()))
        .collect();
    let mut counts = BTreeMap::new();
    let mut lines = String::new();
    let mut differ = Vec::new();
    let mut cached = String::new();
    for path in shaders(dir, ext) {
        let file = path.file_name().unwrap().to_string_lossy().into_owned();
        match records(&path, &file) {
            Ok(lines) => cached.push_str(&lines),
            Err(e) => differ.push(e),
        }
        let v = verdict(&path, out.as_deref());
        // Counted by the first step's outcome and the GLSL ES part's last word before its reason.
        let class = class_of(&v);
        *counts.entry(class).or_insert(0) += 1;
        lines.push_str(&format!("{file}\t{v}\n"));
        let matches = expected
            .get(&file)
            .is_some_and(|e| as_checked(e, glslang) == v);
        // Only conversions and named failures may be recorded, besides the known ones.
        let known = KNOWN_UNNAMED.iter().find(|(f, _)| *f == file);
        if let Some((_, gap)) = known {
            eprintln!("{file}: known unnamed verdict {v:?} ({gap})");
        }
        if !matches || (!is_named(&v) && known.is_none()) {
            differ.push(format!(
                "{file}: expected {:?}, got {v:?}",
                expected.get(&file)
            ));
        }
    }
    if let Some(out) = &out {
        std::fs::write(out.join("verdicts.txt"), &lines).unwrap();
        std::fs::write(out.join("cache.txt"), &cached).unwrap();
    }
    eprintln!("{label}:");
    for (k, n) in &counts {
        eprintln!("  {n:4} {k}");
    }
    let missing: Vec<_> = expected
        .keys()
        .filter(|f| {
            !lines
                .lines()
                .any(|l| l.split('\t').next() == Some(f.as_str()))
        })
        .collect();
    let failures = if differ.is_empty() && missing.is_empty() {
        String::new()
    } else {
        format!(
            "{label}: verdicts differ from {}:\n{}\nmissing: {missing:?}",
            expected_file.display(),
            differ.join("\n")
        )
    };
    (counts, failures)
}

/// Every verdict is the one recorded for that shader in `tests/expected/<corpus>.txt` (a conversion,
/// or a named failure: `unsupported`, `spirv-reader`, or GLSL ES 3.00 lacking what the shader uses);
/// a change either way, including invalid WGSL, fails until the record is updated.
#[test]
fn rig_shaders_give_their_recorded_verdicts() {
    let dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../wcontainers-graphics-test/shaders/dxbc");
    let counts = run(&dir, "dxbc", "rig");
    assert!(counts.values().sum::<usize>() > 0);
}

#[test]
#[ignore = "needs WCONTAINERS_SHADER_CORPUS, the plan's 191 shaders, kept outside the repository"]
fn corpus_gives_its_recorded_verdicts() {
    let dir = std::env::var_os("WCONTAINERS_SHADER_CORPUS")
        .expect("WCONTAINERS_SHADER_CORPUS must name the corpus directory");
    let counts = run(Path::new(&dir), "dxbc", "corpus");
    assert_eq!(counts.values().sum::<usize>(), 191);
}

/// The shaders real programs carry (DXBC and DXIL found in the rig's real programs by
/// `examples/containers.rs`), kept outside the repository like the plan's corpus.
#[test]
#[ignore = "needs WCONTAINERS_GAME_SHADERS, the real programs' shaders, kept outside the repository"]
fn game_shaders_give_their_recorded_verdicts() {
    let dir = std::env::var_os("WCONTAINERS_GAME_SHADERS")
        .expect("WCONTAINERS_GAME_SHADERS must name the directory of the real programs' shaders");
    // Both kinds are checked before either fails, so one kind's difference never hides the other's.
    let (dxbc, dxbc_failures) = verdicts(Path::new(&dir), "dxbc", "games");
    let (dxil, dxil_failures) = verdicts(Path::new(&dir), "dxil", "games-dxil");
    assert!(
        dxbc_failures.is_empty() && dxil_failures.is_empty(),
        "{dxbc_failures}\n{dxil_failures}"
    );
    assert!(dxbc.values().sum::<usize>() > 0 && dxil.values().sum::<usize>() > 0);
}

/// The shaders of vkd3d's shader tests (the Direct3D 12 front's shader runner), compiled by fxc and
/// by dxc, kept outside the repository like the plan's corpus (LGPL test data).
#[test]
#[ignore = "needs WCONTAINERS_RUNNER_SHADERS, vkd3d's shader tests compiled, kept outside the repository"]
fn runner_shaders_give_their_recorded_verdicts() {
    let dir = std::env::var_os("WCONTAINERS_RUNNER_SHADERS")
        .expect("WCONTAINERS_RUNNER_SHADERS must name the directory of the compiled shader tests");
    let (dxbc, dxbc_failures) = verdicts(Path::new(&dir), "dxbc", "runner");
    let (dxil, dxil_failures) = verdicts(Path::new(&dir), "dxil", "runner-dxil");
    assert!(
        dxbc_failures.is_empty() && dxil_failures.is_empty(),
        "{dxbc_failures}\n{dxil_failures}"
    );
    assert!(dxbc.values().sum::<usize>() > 0 && dxil.values().sum::<usize>() > 0);
}

/// The rig's per-sample pixel shaders, whose GLSL ES needs OES_sample_variables: as written, the
/// file carries the directive; written without it, the verdict names what is missing and is not a
/// named outcome, whatever glslang says.
#[test]
#[cfg(not(target_arch = "wasm32"))]
fn a_written_file_without_its_needs_directive_is_not_a_named_outcome() {
    use wcontainers_gpu_webgl::glsl;
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../wcontainers-graphics-test/shaders");
    let directive = format!("{EXTENSION_DIRECTIVE}{SAMPLE_VARIABLES}{EXTENSION_REQUIRED}");
    for file in [
        "dxbc/msaa.ps_sample.dxbc",
        "dxbc/msaa.ps_coverage.dxbc",
        "dxil/msaa.ps_sample.dxil",
        "dxil/msaa.ps_coverage.dxil",
    ] {
        let bytes = std::fs::read(dir.join(file)).unwrap();
        let source = if file.ends_with(".dxil") {
            Source::Dxil(&bytes)
        } else {
            Source::Dxbc(&bytes)
        };
        let converted = convert(source, &webgl_key(&VariantKey::default()), None).unwrap();
        let m = &converted.modules[0];
        let names: Vec<&str> = glsl::needs(m).iter_names().map(|(n, _)| n).collect();
        assert_eq!(names, ["SAMPLE_SHADING"], "{file}");
        let settings = glsl::GlslSettings {
            flip_y: false,
            noperspective: true,
        };
        let text = glsl::write(m, settings).unwrap().source;
        assert!(text.contains(&directive), "{file}");
        assert!(undeclared(&names, &text).is_empty(), "{file}");
        let dropped: String = text
            .lines()
            .filter(|l| *l != directive)
            .map(|l| format!("{l}\n"))
            .collect();
        let missing = undeclared(&names, &dropped);
        assert_eq!(missing, [SAMPLE_VARIABLES], "{file}");
        let verdict = format!(
            "ok{GLSL_PART}{NEEDS}SAMPLE_SHADING {UNDECLARED}{}",
            missing.join("|")
        );
        assert!(!is_named(&verdict), "{file}");
        let compile = format!(
            "ok{GLSL_PART}{NEEDS}SAMPLE_SHADING {COMPILE_FAILED}'gl_SampleID' : undeclared identifier"
        );
        assert!(is_named(&compile), "{file}");
        // A refusal citing nothing of the need is another defect.
        let other = format!("ok{GLSL_PART}{NEEDS}SAMPLE_SHADING {COMPILE_FAILED}an error");
        assert!(!is_named(&other), "{file}");
        let core = format!(
            "ok{GLSL_PART}{NEEDS}POINT_SIZE {COMPILE_FAILED}'gl_SampleID' : undeclared identifier"
        );
        assert!(!is_named(&core), "{file}");
    }
}

/// The reader's refusal of what it does not read is named; any other reader error is not.
#[test]
fn only_the_readers_unsupported_refusals_are_named() {
    assert!(is_named(
        "spirv-reader unsupported instruction ImageQueryLod at Function"
    ));
    assert!(!is_named("spirv-reader invalid id 28"));
    assert!(!is_named("spirv-reader InvalidId(28)"));
}

#[test]
fn rig_dxil_shaders_give_their_recorded_verdicts() {
    let dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../wcontainers-graphics-test/shaders/dxil");
    let counts = run(&dir, "dxil", "rig-dxil");
    assert!(counts.values().sum::<usize>() > 0);
}

#[test]
fn own_shaders_give_their_recorded_verdicts() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/own");
    let (_, dxbc) = verdicts(&dir, "dxbc", "own");
    let (_, dxil) = verdicts(&dir, "dxil", "own-dxil");
    assert!(dxbc.is_empty() && dxil.is_empty(), "{dxbc}\n{dxil}");
}

/// The same HLSL compiled by fxc (DXBC) and by dxc (DXIL) gives the same values at the same locations,
/// so a front can link stages and vertex layouts by either route's reflection alike.
#[test]
fn dxil_reflection_matches_dxbc() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../wcontainers-graphics-test/shaders");
    type Key = (
        String,
        u32,
        Option<wcontainers_gpu::SystemValue>,
        Option<u32>,
        wcontainers_gpu::ScalarKind,
        wcontainers_gpu::Interpolation,
    );
    let key = |v: &wcontainers_gpu::StageValue| -> Key {
        let located = matches!(
            v.system,
            None | Some(wcontainers_gpu::SystemValue::Target(_))
        );
        (
            v.semantic.clone(),
            v.semantic_index,
            v.system,
            located.then_some(v.location),
            v.scalar,
            v.interpolation,
        )
    };
    let keys = |values: &[wcontainers_gpu::StageValue]| {
        let mut k: Vec<_> = values.iter().map(key).collect();
        k.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
        k
    };
    for name in [
        "common.vs_color",
        "common.ps_color",
        "common.vs_full",
        "triangle.vs",
        "triangle.ps",
    ] {
        let read = |ext: &str| std::fs::read(dir.join(ext).join(format!("{name}.{ext}"))).unwrap();
        let (dxbc, dxil) = (read("dxbc"), read("dxil"));
        let dxbc = convert(Source::Dxbc(&dxbc), &VariantKey::default(), None).unwrap();
        let dxil = convert(Source::Dxil(&dxil), &VariantKey::default(), None).unwrap();
        let (a, b) = (&dxbc.reflection, &dxil.reflection);
        assert!(!b.inputs.is_empty() || !b.outputs.is_empty(), "{name}");
        // Values linked by location agree exactly; built-ins the DXBC route drops when unread
        // (a pixel shader's unread position) may remain in the DXIL route's.
        for (what, a, b) in [
            ("inputs", keys(&a.inputs), keys(&b.inputs)),
            ("outputs", keys(&a.outputs), keys(&b.outputs)),
        ] {
            let located = |k: &Vec<Key>| {
                k.iter()
                    .filter(|k| k.3.is_some())
                    .cloned()
                    .collect::<Vec<_>>()
            };
            assert_eq!(located(&a), located(&b), "{name} {what}");
            assert!(
                a.iter().all(|k| b.contains(k)),
                "{name} {what}: {a:?} {b:?}"
            );
        }
    }
}

/// Values dxc packs into one register's components, and arrays and matrices taking several registers,
/// leave a DXIL program one value per register between stages, each value reflected at its register
/// and first component, with the same semantics, kinds and interpolation as the fxc build's.
#[test]
fn dxil_values_between_stages_take_whole_registers() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/own");
    for (name, between_inputs) in [
        ("vs_pack", false),
        ("ps_pack", true),
        ("vs_rows_io", false),
        ("ps_rows_io", true),
    ] {
        let read = |ext: &str| std::fs::read(dir.join(format!("{name}.{ext}"))).unwrap();
        let (dxbc, dxil) = (read("dxbc"), read("dxil"));
        let dxbc = convert(Source::Dxbc(&dxbc), &VariantKey::default(), None).unwrap();
        let dxil = convert(Source::Dxil(&dxil), &VariantKey::default(), None).unwrap();
        let module = &dxil.modules[0].module;
        let function = &module.entry_points[0].function;
        // Each register is one value of the entry point.
        let location = |b: &Option<naga::Binding>| match b {
            Some(naga::Binding::Location { location, .. }) => Some(*location),
            _ => None,
        };
        let mut locations: Vec<u32> = if between_inputs {
            function
                .arguments
                .iter()
                .filter_map(|a| location(&a.binding))
                .collect()
        } else {
            let result = function.result.as_ref().unwrap();
            let naga::TypeInner::Struct { members, .. } = &module.types[result.ty].inner else {
                panic!("{name}: outputs in a struct");
            };
            members
                .iter()
                .filter_map(|m| location(&m.binding))
                .collect()
        };
        let all = locations.len();
        locations.dedup();
        assert_eq!(all, locations.len(), "{name}");
        let values = |r: &wcontainers_gpu::Reflection| {
            let mut v: Vec<_> = r
                .inputs
                .iter()
                .chain(&r.outputs)
                .filter(|v| v.system.is_none())
                .map(|v| {
                    // A vertex program's outputs take the interpolation the variant asks for.
                    let interpolation = between_inputs.then_some(v.interpolation);
                    (
                        v.semantic.to_uppercase(),
                        v.semantic_index,
                        v.scalar,
                        interpolation,
                    )
                })
                .collect();
            v.sort_by_key(|k| format!("{k:?}"));
            v
        };
        // fxc leaves out what the program does not read; dxc keeps the whole signature.
        let (fxc, dxc) = (values(&dxbc.reflection), values(&dxil.reflection));
        assert!(
            fxc.iter().all(|v| dxc.contains(v)),
            "{name}: {fxc:?} {dxc:?}"
        );
        // Each value is placed apart from every other at its register.
        let mut taken = std::collections::BTreeSet::new();
        let reflection = &dxil.reflection;
        let between = if between_inputs {
            &reflection.inputs
        } else {
            &reflection.outputs
        };
        for v in between.iter().filter(|v| v.system.is_none()) {
            assert_eq!(v.location, v.register, "{name} {}", v.semantic);
            assert_eq!(u32::from(v.component), v.mask.trailing_zeros(), "{name}");
            for c in 0..u8::BITS {
                if v.mask & (1 << c) != 0 {
                    assert!(
                        taken.insert((v.register, c)),
                        "{name}: {} overlaps",
                        v.semantic
                    );
                }
            }
        }
    }
}

/// A DXIL vertex shader reading `SV_VertexID` reads the draw's bases from its helper block when asked,
/// as the DXBC route does.
#[test]
fn dxil_draw_bases_from_the_helpers() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../wcontainers-graphics-test/shaders/dxil/common.vs_full.dxil");
    let bytes = std::fs::read(path).unwrap();
    let variant = VariantKey {
        draw_base: wcontainers_gpu::DrawBaseSource::Helpers,
        ..VariantKey::default()
    };
    let with = convert(Source::Dxil(&bytes), &variant, None).unwrap();
    assert_eq!(with.reflection.helper_size, 16);
    assert_eq!(
        with.reflection.helpers[0].value,
        wcontainers_gpu::HelperValue::DrawBase
    );
    let m = &with.modules[0];
    let wgsl =
        naga::back::wgsl::write_string(&m.module, &m.info, naga::back::wgsl::WriterFlags::empty())
            .unwrap();
    assert!(wgsl.contains("helpers.base_vertex"), "{wgsl}");
    assert!(wgsl.contains("@group(3) @binding(0)"), "{wgsl}");
    let without = convert(Source::Dxil(&bytes), &VariantKey::default(), None).unwrap();
    assert!(without.reflection.helpers.is_empty());
}

/// Every rig shader compiled both by fxc (DXBC) and by dxc (DXIL) that converts by both routes gets the
/// same resources (kind, space, register, set), sampled pairs (by register) and stage extras from each;
/// the DXIL route numbers each kind's resources from the kind's first binding in (space, register)
/// order (Direct3D 12's rule), the DXBC route by register (Direct3D 11's).
#[test]
fn dxil_bindings_and_sampled_pairs_match_dxbc() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../wcontainers-graphics-test/shaders");
    let mut compared = 0;
    let mut sampling = 0;
    for dxil_path in shaders(&dir.join("dxil"), "dxil") {
        let name = dxil_path
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let Ok(dxbc) = std::fs::read(dir.join("dxbc").join(format!("{name}.dxbc"))) else {
            continue;
        };
        let dxil = std::fs::read(&dxil_path).unwrap();
        let key = VariantKey::default();
        let (Ok(a), Ok(b)) = (
            convert(Source::Dxbc(&dxbc), &key, None),
            convert(Source::Dxil(&dxil), &key, None),
        ) else {
            continue;
        };
        let bindings = |r: &wcontainers_gpu::Reflection| {
            let mut v: Vec<_> = r
                .bindings
                .iter()
                .map(|b| (b.set, b.kind as u32, b.space, b.register))
                .collect();
            v.sort();
            v
        };
        let (a, b) = (&a.reflection, &b.reflection);
        assert_eq!(bindings(a), bindings(b), "{name} bindings");
        for d in &a.bindings {
            assert_eq!(d.binding, d.kind.binding(d.register).unwrap(), "{name}");
        }
        for d in &b.bindings {
            let rank = b
                .bindings
                .iter()
                .filter(|e| e.kind == d.kind && (e.space, e.register) < (d.space, d.register))
                .count() as u32;
            assert_eq!(d.slot(), rank, "{name}");
        }
        let register = |r: &wcontainers_gpu::Reflection, kind: SlotKind, slot: u32| {
            r.bindings
                .iter()
                .find(|d| d.kind == kind && d.slot() == slot)
                .map(|d| d.register)
        };
        let pairs = |r: &wcontainers_gpu::Reflection| -> Vec<_> {
            r.sampled_pairs
                .iter()
                .map(|&(t, s)| {
                    (
                        register(r, SlotKind::Texture, t),
                        register(r, SlotKind::Sampler, s),
                    )
                })
                .collect()
        };
        assert_eq!(pairs(a), pairs(b), "{name} sampled pairs");
        assert_eq!(a.extras, b.extras, "{name} extras");
        compared += 1;
        sampling += usize::from(!a.sampled_pairs.is_empty());
    }
    eprintln!("compared {compared} shaders, {sampling} of them sampling");
    assert!(compared >= 18 && sampling >= 4, "{compared} {sampling}");
}

/// The size in bytes of each constant buffer a rig shader declares once converted, by register.
fn constant_sizes(dir: &str, name: &str) -> BTreeMap<u32, u32> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../wcontainers-graphics-test/shaders")
        .join(dir)
        .join(name);
    let bytes = std::fs::read(path).unwrap();
    let source = if dir == "dxil" {
        Source::Dxil(&bytes)
    } else {
        Source::Dxbc(&bytes)
    };
    let c = convert(source, &VariantKey::default(), None).unwrap();
    let m = &c.modules[0].module;
    let sizes: BTreeMap<u32, u32> = m
        .global_variables
        .iter()
        .filter(|(_, v)| v.space == naga::AddressSpace::Uniform)
        .map(|(_, v)| {
            let b = v.binding.unwrap();
            let r = c
                .reflection
                .bindings
                .iter()
                .find(|r| (r.set, r.binding) == (b.group, b.binding))
                .unwrap();
            (r.register, m.types[v.ty].inner.size(m.to_ctx()))
        })
        .collect();
    // No replaced 64 KiB type is left for a writer to declare.
    let largest = sizes.values().copied().max().unwrap_or(0);
    assert!(
        m.types
            .iter()
            .all(|(_, t)| t.inner.size(m.to_ctx()) <= largest),
        "{name}: a type larger than every constant buffer"
    );
    sizes
}

/// A dynamically indexed constant buffer is declared with the size the bytecode gives it, not the
/// converters' 64 KiB, on both routes, so a program binding just its buffer satisfies WebGPU's
/// minimum binding size.
#[test]
fn dynamically_indexed_constants_keep_their_declared_size() {
    // Instanced: 33 vectors indexed by instance.
    assert_eq!(constant_sizes("dxbc", "instanced.vs.dxbc")[&0], 33 * 16);
    for (dir, ext) in [("dxbc", "dxbc"), ("dxil", "dxil")] {
        let slots = constant_sizes(dir, &format!("constants.ps_slots.{ext}"));
        assert_eq!(
            (slots[&1], slots[&2], slots[&3]),
            (16 * 16, 256 * 16, 4096 * 16),
            "{dir}"
        );
        let window = constant_sizes(dir, &format!("constants.ps_window.{ext}"));
        assert_eq!(window[&0], 64 * 16, "{dir}");
    }
}
