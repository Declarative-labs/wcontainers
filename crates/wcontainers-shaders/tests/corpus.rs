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

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use wcontainers_gpu::VariantKey;
use wcontainers_shaders::{ConvertError, Source, WEBGPU_CAPABILITIES, convert};

fn shaders(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    v.retain(|p| p.extension().is_some_and(|e| e == ext));
    v.sort();
    v
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
    let converted = match convert(source(&bytes), &VariantKey::default(), None) {
        Ok(c) => c,
        Err(ConvertError::Unsupported(what)) => return format!("unsupported {what}"),
        Err(ConvertError::Reader(what)) => return format!("spirv-reader {what}"),
        Err(e) => return format!("convert {e}"),
    };
    let m = &converted.modules[0];
    let name = path.file_stem().unwrap().to_string_lossy().into_owned();

    let wgsl = match naga::back::wgsl::write_string(
        &m.module,
        &m.info,
        naga::back::wgsl::WriterFlags::empty(),
    ) {
        Ok(s) => s,
        Err(e) => return format!("wgsl-write {e}"),
    };
    if let Some(out) = out {
        std::fs::write(out.join(format!("{name}.wgsl")), &wgsl).unwrap();
    }
    // The converted module is a function of the source and the variant alone.
    let again = convert(source(&bytes), &VariantKey::default(), None).unwrap();
    let m2 = &again.modules[0];
    let wgsl2 = naga::back::wgsl::write_string(
        &m2.module,
        &m2.info,
        naga::back::wgsl::WriterFlags::empty(),
    )
    .unwrap();
    if wgsl2 != wgsl || m2.key != m.key {
        return "convert nondeterministic".into();
    }
    let reparsed = match naga::front::wgsl::parse_str(&wgsl) {
        Ok(m) => m,
        Err(e) => return format!("wgsl-parse {}", e.message()),
    };
    if let Err(e) =
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), WEBGPU_CAPABILITIES)
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
        entry_point: "main".into(),
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
    "ok".into()
}

fn run(dir: &Path, ext: &str, label: &str) -> BTreeMap<String, usize> {
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
    for path in shaders(dir, ext) {
        let file = path.file_name().unwrap().to_string_lossy().into_owned();
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
    assert!(
        differ.is_empty() && missing.is_empty(),
        "{label}: verdicts differ from {}:\n{}\nmissing: {missing:?}",
        expected_file.display(),
        differ.join("\n")
    );
    counts
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
    run(&dir, "dxbc", "own");
    run(&dir, "dxil", "own-dxil");
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
/// same bindings, sampled pairs and stage extras from each.
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
                .map(|b| (b.set, b.binding, b.register, format!("{:?}", b.kind)))
                .collect();
            v.sort();
            v
        };
        let (a, b) = (&a.reflection, &b.reflection);
        assert_eq!(bindings(a), bindings(b), "{name} bindings");
        assert_eq!(a.sampled_pairs, b.sampled_pairs, "{name} sampled pairs");
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
