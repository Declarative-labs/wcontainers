//! Godot's RenderingDevice shaders (`servers/rendering/renderer_rd/shaders/**/*.glsl`, which Forward+
//! and Mobile draw with): each read as Godot's build reads it (`include_file_in_rd_header`), and each
//! variant its C++ user initializes (`corpus/godot-rd-variants`) written stage by stage as
//! `ShaderRD::_build_variant_code` makes it for Vulkan, with no material, for glslang to compile
//! (`corpus/godot-forward-plus`).

use crate::table;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The argument that makes the corpus binary write the variants' sources: Godot's tree, then the
/// output folder.
pub const WRITE: &str = "--godot-rd";
const SHADER_FOLDER: &str = "servers/rendering/renderer_rd/shaders";
/// Each variant: a shader file under the shader folder, Godot's variant index, its defines.
const VARIANTS: &str = include_str!("../../corpus/godot-rd-variants");
/// The general defines each shader is initialized with, as `corpus/godot-defines`.
const GENERAL_DEFINES: &str = include_str!("../../corpus/godot-rd-defines");

/// The markers both of Godot's shader formats share (`glsl_builders.py`, `gles3_builders.py`).
pub const VERTEX_SECTION: &str = "#[vertex]";
pub const FRAGMENT_SECTION: &str = "#[fragment]";
pub const SECTION_START: &str = "#[";
pub const INCLUDE_DIRECTIVE: &str = "#include ";
/// Lines of a stage where a material's globals, uniforms and code go (`_add_stage`).
pub const MATERIAL_CHUNKS: [&str; 3] = ["#GLOBALS", "#MATERIAL_UNIFORMS", "#CODE"];
const COMPUTE_SECTION: &str = "#[compute]";
/// GLSL's line comment, which Godot's build cuts from each line before reading it.
const GLSL_COMMENT_START: &str = "//";
/// An include under this is found from Godot's root, any other beside the including file.
const THIRD_PARTY: &str = "thirdparty/";
/// The line `_build_variant_code` replaces with the defines.
const VERSION_DEFINES: &str = "#VERSION_DEFINES";
/// What it puts after the defines on Vulkan.
const AFTER_DEFINES: &str = "#define RENDER_DRIVER_VULKAN\n#define samplerExternalOES sampler2D\n\
                             #define textureExternalOES texture2D\n";

/// A stage, by the file extension glslang takes it from.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Stage {
    Vertex,
    Fragment,
    Compute,
}

impl Stage {
    const fn extension(self) -> &'static str {
        match self {
            Stage::Vertex => "vert",
            Stage::Fragment => "frag",
            Stage::Compute => "comp",
        }
    }
}

/// One shader file's stages as Godot's build reads them.
#[derive(Default)]
struct Shader {
    lines: HashMap<Stage, Vec<String>>,
    included: HashMap<Stage, Vec<PathBuf>>,
}

fn read_into(
    root: &Path,
    file: &Path,
    shader: &mut Shader,
    reading: &mut Option<Stage>,
) -> Result<(), String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    for line in text.lines() {
        let line = line.replace('\r', "");
        let line = line.split(GLSL_COMMENT_START).next().unwrap_or_default();
        let section = [
            (VERTEX_SECTION, Stage::Vertex),
            (FRAGMENT_SECTION, Stage::Fragment),
            (COMPUTE_SECTION, Stage::Compute),
        ]
        .into_iter()
        .find(|(marker, _)| line.contains(marker));
        if let Some((_, stage)) = section {
            *reading = Some(stage);
            continue;
        }
        if line.trim_start().starts_with(SECTION_START) {
            return Err(format!(
                "{}: a section not read here: {}",
                file.display(),
                line.trim()
            ));
        }
        let Some(stage) = *reading else {
            continue;
        };
        if line.contains(INCLUDE_DIRECTIVE) {
            let name = line.replace(INCLUDE_DIRECTIVE, "");
            let mut quoted = name.trim().chars();
            quoted.next();
            quoted.next_back();
            let name = quoted.as_str();
            let included = if name.starts_with(THIRD_PARTY) {
                root.join(name)
            } else {
                file.parent().expect("a file has a folder").join(name)
            };
            let seen = shader.included.entry(stage).or_default();
            if !seen.contains(&included) {
                seen.push(included.clone());
                read_into(root, &included, shader, reading)?;
            }
            continue;
        }
        shader.lines.entry(stage).or_default().push(line.to_owned());
    }
    Ok(())
}

impl Shader {
    /// The stages `ShaderRD::setup` takes: compute alone where there is one.
    fn stages(&self) -> Vec<(Stage, &[String])> {
        let stage = |s: Stage| self.lines.get(&s).map(|l| (s, l.as_slice()));
        match stage(Stage::Compute) {
            Some(compute) => vec![compute],
            None => [Stage::Vertex, Stage::Fragment]
                .into_iter()
                .filter_map(stage)
                .collect(),
        }
    }
}

/// One stage of a variant, as `_build_variant_code` makes it.
fn stage_source(lines: &[String], general: &str, variant: &str) -> String {
    let mut text = String::new();
    for line in lines {
        if line.starts_with(VERSION_DEFINES) {
            text += "\n";
            text += general;
            text += variant;
            text += "\n";
            text += AFTER_DEFINES;
        } else if !MATERIAL_CHUNKS.iter().any(|c| line.starts_with(c)) {
            text += line;
            text += "\n";
        }
    }
    text
}

/// One variant's stages, each by glslang's file extension for it, as `_build_variant_code` makes them
/// from the shader file `file` in Godot's tree `godot`.
pub fn variant_sources(
    godot: &Path,
    file: &Path,
    general: &str,
    variant: &str,
) -> Result<Vec<(&'static str, String)>, String> {
    let mut shader = Shader::default();
    read_into(godot, file, &mut shader, &mut None)?;
    Ok(shader
        .stages()
        .into_iter()
        .map(|(stage, lines)| (stage.extension(), stage_source(lines, general, variant)))
        .collect())
}

/// Writes every variant's stages under `out` as `<shader file>/<variant>.<stage>`; returns how many
/// files it wrote. Godot's tree is `godot`.
pub fn write(godot: &Path, out: &Path) -> Result<usize, String> {
    let mut written = 0;
    for (path, rest) in table::rows(VARIANTS) {
        let (variant, defines) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        let general = table::unescape(&table::general_defines(GENERAL_DEFINES, Path::new(path)));
        let file = godot.join(SHADER_FOLDER).join(path);
        let dir = out.join(path);
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for (extension, source) in
            variant_sources(godot, &file, &general, &table::unescape(defines.trim()))?
        {
            let stage_file = dir.join(format!("{variant}.{extension}"));
            std::fs::write(&stage_file, source)
                .map_err(|e| format!("{}: {e}", stage_file.display()))?;
            written += 1;
        }
    }
    Ok(written)
}
