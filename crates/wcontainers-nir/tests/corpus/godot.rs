//! Godot's Compatibility renderer shaders (`drivers/gles3/shaders/**/*.glsl`): each read as
//! Godot's build reads it (`gles3_builders.py`) and specialized as `ShaderGLES3` hands it to a
//! desktop OpenGL driver (`_build_variant_code`), with no material code.

use crate::godot_rd::{
    FRAGMENT_SECTION, INCLUDE_DIRECTIVE, MATERIAL_CHUNKS, SECTION_START, VERTEX_SECTION,
};
use crate::table;
use std::path::{Path, PathBuf};
use wcontainers_nir::mesa::Stage;

/// The extension of Godot's shader files, and their folder in Godot's tree.
pub const EXTENSION: &str = "glsl";
const SHADER_FOLDER: [&str; 3] = ["drivers", "gles3", "shaders"];
/// Between a shader file's path and a variant's name in a corpus item, and between the variant's
/// mode and the specialization flipped from its default.
const VARIANT_SEPARATOR: char = '#';
const FLIP_SEPARATOR: char = ':';

/// The general defines each shader is initialized with (`ShaderGLES3::initialize`), at Godot's
/// default project settings: one line a shader file, its path under the shader folder, then the
/// text, `\n` between lines; a line starting as dEQP's comments do is a comment.
const GENERAL_DEFINES: &str = include_str!("../../corpus/godot-defines");

/// The format's markers (`gles3_builders.py`).
const MODES_SECTION: &str = "#[modes]";
const SPECIALIZATIONS_SECTION: &str = "#[specializations]";
const ASSIGNMENT: char = '=';
/// A specialization's default values that mean it is set.
const SET_VALUES: [&str; 2] = ["TRUE", "1"];
/// The variant of a file without modes, and its defines.
const DEFAULT_VARIANT: &str = "DEFAULT";
const DEFAULT_VARIANT_DEFINES: &str = " ";

/// What `_build_variant_code` puts first for desktop OpenGL (`is_gles_over_gl`), with
/// `polyfill_half2float` as Godot's configuration leaves it on a desktop driver.
const DESKTOP_HEADER: &str = "#version 330\n#define USE_GLES_OVER_GL\n#define USE_HALF2FLOAT\n";
const DEFINE: &str = "#define ";
/// What it puts after the defines on a driver without external textures and without
/// `GL_OVR_multiview` (neither reported by our OpenGL).
const AFTER_DEFINES: &str = "#define samplerExternalOES sampler2D\n#ifdef USE_MULTIVIEW\n\
                             #define EMULATE_MULTIVIEW\n#define MAX_VIEWS 2\n#else\n\
                             #define ViewIndex uint(0)\n#define MAX_VIEWS 1\n#endif\n\
                             precision highp float;\nprecision highp int;\n";

#[derive(Clone, Copy, PartialEq)]
enum Reading {
    Nothing,
    Specializations,
    Stage(Stage),
}

/// One shader file as Godot's build reads it.
#[derive(Default)]
pub struct Shader {
    /// Each mode's name (upper case, as Godot's enumeration) and defines.
    modes: Vec<(String, String)>,
    /// Each specialization's name and whether it is set by default.
    specializations: Vec<(String, bool)>,
    vertex: Vec<String>,
    fragment: Vec<String>,
    vertex_included: Vec<PathBuf>,
    fragment_included: Vec<PathBuf>,
}

/// Whether a file is one of the Compatibility renderer's shader files, by its place in Godot's tree.
pub fn is_shader(path: &Path) -> bool {
    let names: Vec<_> = path.components().map(|c| c.as_os_str()).collect();
    path.extension().is_some_and(|e| e == EXTENSION)
        && names
            .windows(SHADER_FOLDER.len())
            .any(|w| w.iter().zip(SHADER_FOLDER).all(|(a, b)| *a == b))
}

/// The path of a shader file under the shader folder, as the general defines name it.
fn under_shader_folder(path: &Path) -> Option<PathBuf> {
    let names: Vec<_> = path.components().map(|c| c.as_os_str()).collect();
    let at = names
        .windows(SHADER_FOLDER.len())
        .rposition(|w| w.iter().zip(SHADER_FOLDER).all(|(a, b)| *a == b))?;
    Some(names[at + SHADER_FOLDER.len()..].iter().collect())
}

/// Reads a file into `shader` as `include_file_in_gles3_header` does, includes resolved beside it.
fn read_into(file: &Path, shader: &mut Shader, reading: &mut Reading) -> Result<(), String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    for line in text.lines() {
        let line = line.replace('\r', "");
        if *reading == Reading::Nothing
            && let Some((name, defines)) = line.split_once(ASSIGNMENT)
        {
            shader
                .modes
                .push((name.trim().to_uppercase(), defines.trim().to_owned()));
            continue;
        }
        if *reading == Reading::Specializations
            && let Some((name, value)) = line.split_once(ASSIGNMENT)
        {
            let set = SET_VALUES.contains(&value.trim().to_uppercase().as_str());
            shader.specializations.push((name.trim().to_owned(), set));
            continue;
        }
        if line.contains(MODES_SECTION) {
            continue;
        }
        if line.contains(SPECIALIZATIONS_SECTION) {
            *reading = Reading::Specializations;
            continue;
        }
        if line.contains(VERTEX_SECTION) {
            *reading = Reading::Stage(Stage::Vertex);
            continue;
        }
        if line.contains(FRAGMENT_SECTION) {
            *reading = Reading::Stage(Stage::Fragment);
            continue;
        }
        if line.trim_start().starts_with(SECTION_START) {
            return Err(format!(
                "{}: a section not read here: {}",
                file.display(),
                line.trim()
            ));
        }
        let Reading::Stage(stage) = *reading else {
            continue;
        };
        if line.contains(INCLUDE_DIRECTIVE) {
            let name = line.replace(INCLUDE_DIRECTIVE, "");
            let name = name.trim();
            let mut quoted = name.chars();
            quoted.next();
            quoted.next_back();
            let included = file
                .parent()
                .expect("a file has a folder")
                .join(quoted.as_str());
            let seen = match stage {
                Stage::Vertex => &mut shader.vertex_included,
                _ => &mut shader.fragment_included,
            };
            if !seen.contains(&included) {
                seen.push(included.clone());
                read_into(&included, shader, reading)?;
            }
            continue;
        }
        match stage {
            Stage::Vertex => shader.vertex.push(line),
            _ => shader.fragment.push(line),
        }
    }
    Ok(())
}

/// A shader file as Godot's build reads it.
pub fn read(file: &Path) -> Result<Shader, String> {
    let mut shader = Shader::default();
    read_into(file, &mut shader, &mut Reading::Nothing)?;
    Ok(shader)
}

impl Shader {
    /// The variants the corpus takes: each mode at the default specializations, and each mode with
    /// one specialization flipped from its default, named `<mode>` and `<mode>:<specialization>`.
    pub fn variants(&self) -> Vec<String> {
        let modes: Vec<&str> = if self.modes.is_empty() {
            vec![DEFAULT_VARIANT]
        } else {
            self.modes.iter().map(|(m, _)| m.as_str()).collect()
        };
        modes
            .iter()
            .flat_map(|m| {
                std::iter::once(m.to_string()).chain(
                    self.specializations
                        .iter()
                        .map(move |(s, _)| format!("{m}{FLIP_SEPARATOR}{s}")),
                )
            })
            .collect()
    }

    /// One variant's vertex and fragment programs, as `_build_variant_code` makes them with the
    /// general defines given.
    pub fn program(&self, variant: &str, general: &str) -> Result<Vec<(Stage, String)>, String> {
        let (mode, flipped) = match variant.split_once(FLIP_SEPARATOR) {
            Some((m, f)) => (m, Some(f)),
            None => (variant, None),
        };
        let mode_defines = if self.modes.is_empty() && mode == DEFAULT_VARIANT {
            DEFAULT_VARIANT_DEFINES
        } else {
            self.modes
                .iter()
                .find(|(m, _)| m == mode)
                .map(|(_, d)| d.as_str())
                .ok_or_else(|| format!("no mode {mode}"))?
        };
        if let Some(f) = flipped
            && !self.specializations.iter().any(|(s, _)| s == f)
        {
            return Err(format!("no specialization {f}"));
        }
        let mut defines = String::from(DESKTOP_HEADER);
        for (name, set) in &self.specializations {
            if *set != (flipped == Some(name.as_str())) {
                defines += &format!("{DEFINE}{name}\n");
            }
        }
        defines += "\n";
        defines += &table::unescape(general);
        defines += &table::unescape(mode_defines);
        defines += "\n\n";
        defines += AFTER_DEFINES;
        let stage = |lines: &[String]| {
            let mut text = defines.clone();
            for line in lines {
                if !MATERIAL_CHUNKS.iter().any(|c| line.starts_with(c)) {
                    text += line;
                    text += "\n";
                }
            }
            text
        };
        Ok(vec![
            (Stage::Vertex, stage(&self.vertex)),
            (Stage::Fragment, stage(&self.fragment)),
        ])
    }

    fn has_stages(&self) -> bool {
        !self.vertex.is_empty() || !self.fragment.is_empty()
    }
}

/// A corpus item's shader file and variant, when the item is Godot's.
fn split(item: &Path) -> Option<(PathBuf, String)> {
    let (file, variant) = item.to_str()?.rsplit_once(VARIANT_SEPARATOR)?;
    let file = PathBuf::from(file);
    is_shader(&file).then(|| (file, variant.to_owned()))
}

/// The corpus items of a shader file: one a variant (`Shader::variants`); none for a file of
/// included text only; the file itself when it cannot be read, so that the run names why.
pub fn items(file: &Path) -> Vec<PathBuf> {
    match read(file) {
        Ok(shader) if shader.has_stages() => shader
            .variants()
            .iter()
            .map(|v| PathBuf::from(format!("{}{VARIANT_SEPARATOR}{v}", file.display())))
            .collect(),
        Ok(_) => Vec::new(),
        Err(_) => vec![file.to_owned()],
    }
}

/// A corpus item's stages when it is Godot's: a variant's program, or why a file has none.
pub fn sources(item: &Path) -> Option<Result<Vec<(Stage, String)>, String>> {
    if let Some((file, variant)) = split(item) {
        let general = under_shader_folder(&file)
            .map(|p| table::general_defines(GENERAL_DEFINES, &p))
            .unwrap_or_default();
        return Some(read(&file).and_then(|s| s.program(&variant, &general)));
    }
    is_shader(item).then(|| {
        read(item)
            .and_then(|_| Err("a Godot shader file, whose variants are the corpus items".into()))
    })
}
