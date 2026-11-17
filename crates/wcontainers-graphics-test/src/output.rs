//! What a scene produces: named pictures and texts, saved losslessly as `<scene>[-<part>].<ext>`.

use std::path::Path;

use image::DynamicImage;

pub enum Value {
    Picture(DynamicImage),
    /// Facts that must match exactly, one per line: read-back values, query results, answers.
    Text(String),
}

pub struct Output {
    /// The part of the scene, empty for a scene's only output.
    pub part: String,
    pub value: Value,
}

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

    pub fn file_name(&self, scene: &str) -> anyhow::Result<String> {
        let stem = if self.part.is_empty() {
            scene.to_owned()
        } else {
            format!("{scene}-{}", self.part)
        };
        let extension = match &self.value {
            Value::Picture(image) => crate::extension(image)?,
            Value::Text(_) => "txt",
        };
        Ok(format!("{stem}.{extension}"))
    }

    /// Saves the output in `dir` and returns its file name.
    pub fn save(&self, scene: &str, dir: &Path) -> anyhow::Result<String> {
        let name = self.file_name(scene)?;
        let path = dir.join(&name);
        match &self.value {
            Value::Picture(image) => crate::save(image, &path)?,
            Value::Text(text) => {
                std::fs::create_dir_all(dir)?;
                std::fs::write(&path, text)?;
            }
        }
        Ok(name)
    }
}
