//! The built-in programs written in both back ends' languages, as the back ends will write them.

extern crate std;

use super::*;
use naga::back::{glsl, wgsl};

/// Every built-in program a default can make.
fn all() -> Vec<Builtin> {
    let mut v = alloc::vec![
        Builtin::ClearFloat,
        Builtin::ClearUint,
        Builtin::ClearSint,
        Builtin::ClearDepth,
        Builtin::ClearStencil,
    ];
    let dims = [
        ViewDimension::D2,
        ViewDimension::D2Array,
        ViewDimension::Cube,
        ViewDimension::CubeArray,
        ViewDimension::D3,
    ];
    for dim in dims {
        v.push(Builtin::Mip(dim, MipRead::Filtered));
        v.push(Builtin::Mip(dim, MipRead::Unfiltered));
        // Integer loads read cubes through 2D arrays.
        if !matches!(dim, ViewDimension::Cube | ViewDimension::CubeArray) {
            v.push(Builtin::Mip(dim, MipRead::Unorm));
            v.push(Builtin::Mip(dim, MipRead::Snorm));
        }
    }
    v
}

fn glsl_es_300(
    module: &naga::Module,
    info: &naga::valid::ModuleInfo,
    entry: usize,
) -> Result<String, String> {
    let ep = &module.entry_points[entry];
    let options = glsl::Options {
        version: glsl::Version::Embedded {
            version: 300,
            is_webgl: true,
        },
        writer_flags: glsl::WriterFlags::empty(),
        binding_map: Default::default(),
        zero_initialize_workgroup_memory: true,
    };
    let pipeline = glsl::PipelineOptions {
        shader_stage: ep.stage,
        entry_point: ep.name.clone(),
        multiview: None,
    };
    let mut out = String::new();
    glsl::Writer::new(
        &mut out,
        module,
        info,
        &options,
        &pipeline,
        Default::default(),
    )
    .and_then(|mut w| w.write().map(|_| ()))
    .map_err(|e| format!("{e}"))?;
    Ok(out)
}

/// Writes every built-in program as WGSL, and as GLSL ES 3.00 for WebGL 2 (all but the cube-array
/// ones, which WebGL 2 never makes). With `WCONTAINERS_GPU_BUILTINS_OUT` set, also saves each file
/// there, for compiling in a browser.
#[test]
fn every_built_in_program_writes_in_each_back_end_language() {
    let out = std::env::var_os("WCONTAINERS_GPU_BUILTINS_OUT").map(std::path::PathBuf::from);
    if let Some(dir) = &out {
        std::fs::create_dir_all(dir).unwrap();
    }
    for which in all() {
        let (module, info) = module(which).unwrap_or_else(|e| panic!("{which:?}: {e:?}"));
        let name = format!("{which:?}").replace([' ', '(', ')', ','], "");
        let wgsl = wgsl::write_string(&module, &info, wgsl::WriterFlags::empty())
            .unwrap_or_else(|e| panic!("{which:?}: wgsl: {e}"));
        let webgl = !matches!(which, Builtin::Mip(ViewDimension::CubeArray, _));
        let mut files = alloc::vec![(format!("{name}.wgsl"), wgsl)];
        for (i, ep) in module.entry_points.iter().enumerate() {
            match glsl_es_300(&module, &info, i) {
                Ok(text) => {
                    let stage = if ep.stage == naga::ShaderStage::Vertex {
                        "vert"
                    } else {
                        "frag"
                    };
                    files.push((format!("{name}.{}.{stage}.glsl", ep.name), text));
                }
                Err(e) => assert!(!webgl, "{which:?} {}: glsl es 3.00: {e}", ep.name),
            }
        }
        if let Some(dir) = &out {
            for (file, text) in files {
                std::fs::write(dir.join(file), text).unwrap();
            }
        }
    }
}

/// Each module holds only what its own program uses.
#[test]
fn modules_hold_only_their_own_bindings() {
    let (m, _) = super::module(Builtin::Mip(ViewDimension::D2, MipRead::Filtered)).unwrap();
    assert_eq!(m.global_variables.len(), 2, "the sampler and the texture");
    let (m, _) = super::module(Builtin::ClearUint).unwrap();
    assert_eq!(m.global_variables.len(), 1, "the parameters");
    let (m, _) = super::module(Builtin::ClearStencil).unwrap();
    assert!(m.global_variables.is_empty());
    assert_eq!(m.entry_points.len(), 1, "the vertex stage only");
}
