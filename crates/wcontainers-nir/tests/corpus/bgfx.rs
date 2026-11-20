//! bgfx's compiled shaders (its `shaderc` output, `examples/runtime/shaders/<renderer>/*.bin`): a
//! desktop GLSL one read as the program bgfx's OpenGL renderer hands the driver (`renderer_gl.cpp`).

use wcontainers_nir::mesa::Stage;

/// The extension of bgfx's compiled shaders, and the folder of its desktop GLSL ones.
pub const EXTENSION: &str = "bin";
pub const DESKTOP_FOLDER: &str = "glsl";
/// The format version read here: shaderc's `BGFX_SHADER_BIN_VERSION` of the pinned files.
const FORMAT_VERSION: u8 = 12;
/// Each stage's magic letters, before the format version.
const STAGES: [(&[u8], Stage); 3] = [
    (b"VSH", Stage::Vertex),
    (b"FSH", Stage::Fragment),
    (b"CSH", Stage::Compute),
];
/// What bgfx's OpenGL renderer puts before a source without a version: its least desktop OpenGL,
/// 4.3 (`BGFX_CONFIG_RENDERER_OPENGL`).
const VERSION_DIRECTIVE: &str = "#version";
const VERSION_LINE: &str = "#version 430\n";

/// Whether a file is one of bgfx's desktop GLSL shaders, by its place in bgfx's layout.
pub fn is_desktop_shader(path: &std::path::Path) -> bool {
    path.extension().is_some_and(|e| e == EXTENSION)
        && path
            .parent()
            .and_then(|p| p.file_name())
            .is_some_and(|f| f == DESKTOP_FOLDER)
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let taken = self
            .bytes
            .get(self.at..self.at + n)
            .ok_or_else(|| format!("a bgfx shader ends at {} of {}", self.at, self.at + n))?;
        self.at += n;
        Ok(taken)
    }

    fn word<const N: usize>(&mut self) -> Result<[u8; N], String> {
        Ok(self.take(N)?.try_into().expect("N bytes"))
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(u8::from_le_bytes(self.word()?))
    }

    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.word()?))
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.word()?))
    }
}

/// The program of one compiled shader: its stage and source, with the version line bgfx adds.
pub fn sources(bytes: &[u8]) -> Result<Vec<(Stage, String)>, String> {
    let mut r = Reader { bytes, at: 0 };
    let magic = r.take(STAGES[0].0.len())?;
    let stage = STAGES
        .iter()
        .find(|(m, _)| *m == magic)
        .map(|(_, s)| *s)
        .ok_or("not a bgfx shader")?;
    let version = r.u8()?;
    if version != FORMAT_VERSION {
        return Err(format!(
            "bgfx shader format {version} is not read here ({FORMAT_VERSION} is)"
        ));
    }
    let _input_hash = r.u32()?;
    let _output_hash = r.u32()?;
    let _raw_texture_bindings = r.u32()?;
    let _raw_storage_bindings = r.u32()?;
    for _ in 0..r.u16()? {
        let name_size = r.u8()?;
        r.take(usize::from(name_size))?;
        let _type = r.u8()?;
        let _count = r.u8()?;
        let _register_index = r.u16()?;
        let _register_count = r.u16()?;
        let _texture_info = r.u16()?;
        let _texture_format = r.u16()?;
    }
    let size = r.u32()?;
    let code = r.take(usize::try_from(size).map_err(|e| e.to_string())?)?;
    let code = String::from_utf8_lossy(code);
    let source = if code.starts_with(VERSION_DIRECTIVE) {
        code.into_owned()
    } else {
        format!("{VERSION_LINE}{code}")
    };
    Ok(vec![(stage, source)])
}
