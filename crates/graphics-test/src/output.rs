//! What a scene produces: named pictures and texts, saved losslessly as `<scene>[-<part>].<ext>`, or
//! the note that a part is absent because the device reports what it needs as unsupported.

use std::path::Path;

use image::DynamicImage;

pub enum Value {
    Picture(DynamicImage),
    /// Facts that must match exactly, one per line: read-back values, query results, answers.
    Text(String),
    /// Answers to capability questions, one per line, each of which may report less than the GPU
    /// but never more (see check.rs).
    Caps(String),
    /// Why the part was not drawn: what it needs, which the device reports unsupported.
    Absent(String),
}

pub struct Output {
    /// The part of the scene, empty for a scene's only output.
    pub part: String,
    pub value: Value,
}

/// The extension of an absent part's note: `<scene>[-<part>].absent`.
pub const ABSENT: &str = "absent";

impl Output {
    pub fn picture(part: &str, image: impl Into<DynamicImage>) -> Output {
        Output {
            part: part.to_owned(),
            value: Value::Picture(image.into()),
        }
    }

    pub fn text(part: &str, text: String) -> Output {
        Output {
            part: part.to_owned(),
            value: Value::Text(text),
        }
    }

    pub fn caps(part: &str, text: String) -> Output {
        Output {
            part: part.to_owned(),
            value: Value::Caps(text),
        }
    }

    pub fn absent(part: &str, why: String) -> Output {
        Output {
            part: part.to_owned(),
            value: Value::Absent(why),
        }
    }

    pub fn file_name(&self, scene: &str) -> anyhow::Result<String> {
        let stem = if self.part.is_empty() {
            scene.to_owned()
        } else {
            format!("{scene}-{}", self.part)
        };
        let extension = match &self.value {
            Value::Picture(image) => crate::extension(image)?,
            Value::Text(_) => "txt",
            Value::Caps(_) => "caps",
            Value::Absent(_) => ABSENT,
        };
        Ok(format!("{stem}.{extension}"))
    }

    /// Saves the output in `dir` and returns its file name.
    pub fn save(&self, scene: &str, dir: &Path) -> anyhow::Result<String> {
        let name = self.file_name(scene)?;
        let path = dir.join(&name);
        match &self.value {
            Value::Picture(image) => crate::save(image, &path)?,
            Value::Text(text) | Value::Caps(text) | Value::Absent(text) => {
                std::fs::create_dir_all(dir)?;
                std::fs::write(&path, text)?;
            }
        }
        Ok(name)
    }
}
