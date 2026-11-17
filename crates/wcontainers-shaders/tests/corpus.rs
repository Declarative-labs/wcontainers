//! Converts every shader of the corpora and writes each result as WGSL and GLSL ES 3.00, checking the
//! WGSL by reading and validating it again and the GLSL with glslangValidator where it is installed.
//!
//! Corpora: the rig's compiled Direct3D shaders in `wcontainers-graphics-test/shaders/dxbc` (always), and
//! the directory in `WCONTAINERS_SHADER_CORPUS` (the plan's 191 DXBC blobs from Wine's tests, kept outside
//! the repository). `WCONTAINERS_SHADERS_OUT` names a directory for the written files and a verdict per
//! shader (`verdicts.txt`); `GLSLANG` names the glslangValidator to run.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use wcontainers_gpu::VariantKey;
use wcontainers_shaders::{ConvertError, Source, WEBGPU_CAPABILITIES, convert};

fn shaders(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    v.retain(|p| p.extension().is_some_and(|e| e == "dxbc"));
    v.sort();
    v
}

/// The verdict for one shader: "ok" or the first failing step and its reason.
fn verdict(path: &Path, out: Option<&Path>) -> String {
    let bytes = std::fs::read(path).unwrap();
    let converted = match convert(Source::Dxbc(&bytes), &VariantKey::default(), None) {
        Ok(c) => c,
        Err(ConvertError::Unsupported(what)) => return format!("unsupported {what}"),
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
    let again = convert(Source::Dxbc(&bytes), &VariantKey::default(), None).unwrap();
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
    let options = naga::back::glsl::Options {
        version: naga::back::glsl::Version::Embedded {
            version: 300,
            is_webgl: true,
        },
        writer_flags: naga::back::glsl::WriterFlags::empty(),
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

fn run(dir: &Path, label: &str) -> BTreeMap<String, usize> {
    let out = std::env::var_os("WCONTAINERS_SHADERS_OUT").map(|o| PathBuf::from(o).join(label));
    if let Some(out) = &out {
        std::fs::create_dir_all(out).unwrap();
    }
    let mut counts = BTreeMap::new();
    let mut lines = String::new();
    for path in shaders(dir) {
        let v = verdict(&path, out.as_deref());
        let class: String = v.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
        *counts.entry(class).or_insert(0) += 1;
        lines.push_str(&format!(
            "{}\t{v}\n",
            path.file_name().unwrap().to_string_lossy()
        ));
    }
    if let Some(out) = &out {
        std::fs::write(out.join("verdicts.txt"), &lines).unwrap();
    }
    eprintln!("{label}:");
    for (k, n) in &counts {
        eprintln!("  {n:4} {k}");
    }
    counts
}

#[test]
fn rig_shaders_convert_or_fail_with_a_named_reason() {
    let dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../wcontainers-graphics-test/shaders/dxbc");
    let counts = run(&dir, "rig");
    assert!(counts.values().sum::<usize>() > 0);
    let unnamed: usize = counts
        .iter()
        .filter(|(k, _)| k.starts_with("convert "))
        .map(|(_, n)| n)
        .sum();
    assert_eq!(
        unnamed, 0,
        "shaders failing without a named reason: {counts:?}"
    );
}

#[test]
fn corpus_converts_or_fails_with_a_named_reason() {
    let Some(dir) = std::env::var_os("WCONTAINERS_SHADER_CORPUS") else {
        eprintln!("WCONTAINERS_SHADER_CORPUS not set; skipped");
        return;
    };
    let counts = run(Path::new(&dir), "corpus");
    let unnamed: usize = counts
        .iter()
        .filter(|(k, _)| k.starts_with("convert "))
        .map(|(_, n)| n)
        .sum();
    assert_eq!(
        unnamed, 0,
        "shaders failing without a named reason: {counts:?}"
    );
}
