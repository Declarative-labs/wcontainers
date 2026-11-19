//! Converts every shader of the corpora and writes each result as WGSL and GLSL ES 3.00, checking the
//! WGSL by reading and validating it again and the GLSL with glslangValidator where it is installed.
//!
//! Each shader's verdict must equal the one recorded in `tests/expected/<corpus>.txt`.
//!
//! Corpora: the rig's compiled Direct3D shaders in `wcontainers-graphics-test/shaders/{dxbc,dxil}`, this
//! crate's own in `tests/own`, and (an ignored test) the directory in `WCONTAINERS_SHADER_CORPUS` (the
//! plan's 191 DXBC blobs from Wine's tests, kept outside the repository). `WCONTAINERS_SHADERS_OUT` names
//! a directory for the written files, a verdict per shader (`verdicts.txt`) and the classes in the
//! expected files' form (`classes.txt`); with it, `GLSLANG` names the glslangValidator to run.
//!
//! Every converted shader also goes through the cache's record (plan section 8) under each variant
//! key a front asks for today, and must come back to the same bytes; `cache.txt` lists each key and
//! a hash of its record, so builds (native, WebAssembly) can be compared.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use wcontainers_gpu::{
    DrawBaseSource, Format, ShaderModule, SlotKind, SystemValue, TexelBufferRoute, VariantKey,
    VertexFormat,
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
        (
            "draw-base-helpers".to_string(),
            VariantKey {
                draw_base: DrawBaseSource::Helpers,
                ..VariantKey::default()
            },
        ),
    ];
    if let Err(ConvertError::Unsupported("StorageFormat" | "TypedBuffer")) =
        convert(source, &VariantKey::default(), None)
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
        ConvertError::Unsupported(what) => format!("unsupported {what}"),
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
        Err(ConvertError::Unsupported("StorageFormat" | "TypedBuffer" | "StorageImageAtomics")) => {
            let mut last = None;
            for (format, mut k) in view_keys() {
                let mut made = convert(source(&bytes), &k, None);
                if let Err(ConvertError::Unsupported("StorageImageAtomics")) = made {
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
                    // A view of the other kind tells nothing of the shader: any other refusal is
                    // the one reported.
                    Err(e) => {
                        let kind = |e: &ConvertError| {
                            matches!(
                                e,
                                ConvertError::Unsupported(
                                    "StorageFormatKind" | "TypedBufferFormatKind"
                                )
                            )
                        };
                        if !matches!(&last, Some(Err(l)) if !kind(l)) {
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
    // A vertex shader writing cull distances reaches WebGPU through the stage helper: the WGSL
    // written is its compute form's, converted with every input pulled.
    let drawn = |converted: &Converted| -> Result<ShaderModule, String> {
        if !writes_cull_distances(converted) {
            return Ok(converted.modules[0].clone());
        }
        let k = VariantKey {
            pulled: pulled_inputs(converted),
            ..key.clone()
        };
        let pulled = convert(source(&bytes), &k, None).map_err(|e| named(e) + " (cull route)")?;
        stage::cull_programs(&pulled)
            .map(|p| p.compute)
            .map_err(|e| named(e) + " (cull route)")
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

    let stage = m.module.entry_points[0].stage;
    // The options the WebGL 2 back end writes with (`wcontainers-gpu-webgl`'s `write_glsl`, on a
    // context with EXT_clip_control, so without its y flip).
    let options = naga::back::glsl::Options {
        version: naga::back::glsl::Version::Embedded {
            version: 300,
            is_webgl: true,
        },
        writer_flags: naga::back::glsl::WriterFlags::FORCE_POINT_SIZE,
        ..Default::default()
    };
    let pipeline = naga::back::glsl::PipelineOptions {
        shader_stage: stage,
        entry_point: wcontainers_shaders::ENTRY_POINT.into(),
        multiview: None,
    };
    let mut glsl = String::new();
    let written = naga::back::glsl::Writer::new(
        &mut glsl,
        &m.module,
        &m.info,
        &options,
        &pipeline,
        naga::proc::BoundsCheckPolicies::default(),
    )
    .and_then(|mut w| w.write().map(|_| ()));
    if let Err(e) = written {
        return format!("ok-wgsl glsl-write {e}");
    }
    let ext = if stage == naga::ShaderStage::Vertex {
        "vert"
    } else {
        "frag"
    };
    if let Some(out) = out {
        let file = out.join(format!("{name}.{ext}"));
        std::fs::write(&file, &glsl).unwrap();
        if let Ok(glslang) = std::env::var("GLSLANG") {
            let run = std::process::Command::new(glslang)
                .arg(&file)
                .output()
                .unwrap();
            if !run.status.success() {
                let text = String::from_utf8_lossy(&run.stdout);
                let first = text
                    .lines()
                    .find(|l| l.contains("ERROR"))
                    .unwrap_or("")
                    .to_string();
                return format!("ok-wgsl glsl-compile {first}");
            }
        }
    }
    ok
}

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
    let mut classes = String::new();
    let mut differ = Vec::new();
    let mut cached = String::new();
    for path in shaders(dir, ext) {
        let file = path.file_name().unwrap().to_string_lossy().into_owned();
        match records(&path, &file) {
            Ok(lines) => cached.push_str(&lines),
            Err(e) => differ.push(e),
        }
        let v = verdict(&path, out.as_deref());
        let class: String = v.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
        *counts.entry(class.clone()).or_insert(0) += 1;
        lines.push_str(&format!("{file}\t{v}\n"));
        classes.push_str(&format!("{file}\t{class}\n"));
        // Without glslang, a shader glslang refuses is written and reads as "ok".
        let matches = expected.get(&file).is_some_and(|e| {
            *e == class || (!glslang && e == "ok-wgsl glsl-compile" && class == "ok")
        });
        // Only conversions and named failures may be recorded.
        let named = ["ok", "ok-wgsl", "unsupported", "spirv-reader"]
            .contains(&v.split_whitespace().next().unwrap_or(""));
        if !matches || !named {
            differ.push(format!(
                "{file}: expected {:?}, got {v:?}",
                expected.get(&file)
            ));
        }
    }
    if let Some(out) = &out {
        std::fs::write(out.join("verdicts.txt"), &lines).unwrap();
        std::fs::write(out.join("classes.txt"), &classes).unwrap();
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
    let dxbc = run(Path::new(&dir), "dxbc", "games");
    let dxil = run(Path::new(&dir), "dxil", "games-dxil");
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
