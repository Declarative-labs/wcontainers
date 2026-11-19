//! Built-in compute programs the fronts dispatch: the clear of a storage texture view (Direct3D's
//! `ClearUnorderedAccessViewUint` and `ClearUnorderedAccessViewFloat` on textures), one program per
//! storage format and view dimension, writing one value into a box of the view.

use alloc::format;
use alloc::string::String;
use wcontainers_gpu::{Format, FormatKind, ShaderModule, ShaderStage, ViewDimension};

use crate::ConvertError;
use crate::variant::pull::{Kind, layout};
use crate::variant::texel::vertex_format;

/// The clear program's parameters in set 0: a uniform buffer of [`clear_params`]' 48 bytes.
pub const CLEAR_PARAMS_BINDING: u32 = 0;
/// The storage view the clear program writes, in set 0, write-only, of the program's format.
pub const CLEAR_VIEW_BINDING: u32 = 1;
/// Invocations of a clear's workgroup along x and y; each workgroup covers one layer or slice.
pub const CLEAR_GROUP: [u32; 2] = [8, 8];

/// The program clearing storage views of `format` and `dimension` (`D2`, `D2Array` or `D3`; one
/// dimensional views are 2D views of one row). Refused by name: formats with no storage form
/// (`StorageFormatAbsent`) and cube dimensions (`ClearDimension`: their storage views are 2D arrays).
pub fn storage_clear(
    format: Format,
    dimension: ViewDimension,
) -> Result<ShaderModule, ConvertError> {
    let storage = crate::lower::storage_format(format).ok_or(ConvertError::Unsupported(
        crate::refusal::STORAGE_FORMAT_ABSENT,
    ))?;
    // WGSL's name of a storage format is naga's variant name in lower case.
    let name = format!("{storage:?}").to_ascii_lowercase();
    let (texture, coordinates) = match dimension {
        ViewDimension::D2 => ("texture_storage_2d", "at.xy"),
        ViewDimension::D2Array => ("texture_storage_2d_array", "at.xy, at.z"),
        ViewDimension::D3 => ("texture_storage_3d", "at"),
        ViewDimension::Cube | ViewDimension::CubeArray => {
            return Err(ConvertError::Unsupported(crate::refusal::CLEAR_DIMENSION));
        }
    };
    let value = match format.kind() {
        FormatKind::Uint => "clear.value",
        FormatKind::Sint => "bitcast<vec4<i32>>(clear.value)",
        _ => "bitcast<vec4<f32>>(clear.value)",
    };
    let [gx, gy] = CLEAR_GROUP;
    let text: String = format!(
        "struct Clear {{ value: vec4<u32>, origin: vec4<u32>, end: vec4<u32> }}\n\
         @group(0) @binding({CLEAR_PARAMS_BINDING}) var<uniform> clear: Clear;\n\
         @group(0) @binding({CLEAR_VIEW_BINDING}) var view: {texture}<{name}, write>;\n\
         @compute @workgroup_size({gx}, {gy}, 1)\n\
         fn main(@builtin(global_invocation_id) id: vec3<u32>) {{\n\
         \x20   let at = clear.origin.xyz + id;\n\
         \x20   if any(at >= clear.end.xyz) {{ return; }}\n\
         \x20   textureStore(view, {coordinates}, {value});\n\
         }}\n"
    );
    let module = naga::front::wgsl::parse_str(&text)
        .map_err(|e| ConvertError::Malformed(format!("the clear program: {e}")))?;
    Ok(ShaderModule {
        info: crate::validate(&module)?,
        module,
        entry_point: crate::ENTRY_POINT.into(),
        stage: ShaderStage::Compute,
        key: *blake3::hash(text.as_bytes()).as_bytes(),
    })
}

/// The clear program's parameters: the value as [`storage_clear_value`] gives it, and the texels from
/// `origin` to `end` (x, y, and layer or slice; `end` exclusive and inside the view).
pub fn clear_params(value: [u32; 4], origin: [u32; 3], end: [u32; 3]) -> [u8; 48] {
    let mut out = [0; 48];
    let words = value
        .into_iter()
        .chain(origin)
        .chain([0])
        .chain(end)
        .chain([0]);
    for (o, w) in out.chunks_mut(4).zip(words) {
        o.copy_from_slice(&w.to_le_bytes());
    }
    out
}

/// The workgroups a clear from `origin` to `end` dispatches.
pub fn clear_groups(origin: [u32; 3], end: [u32; 3]) -> [u32; 3] {
    let [gx, gy] = CLEAR_GROUP;
    [
        end[0].saturating_sub(origin[0]).div_ceil(gx),
        end[1].saturating_sub(origin[1]).div_ceil(gy),
        end[2].saturating_sub(origin[2]),
    ]
}

/// The words the clear program stores so that a view of `format` holds Direct3D's clear of `values`:
/// with `float`, the four floats' bits (denormals made zeros), which the store converts to the format
/// as Direct3D does; without, each value's low bits as the component's own bits (Direct3D's `Uint`
/// clear, which converts nothing; WARP does exactly this, while the RTX 3080's Direct3D 11 saturates
/// each value to the component's bits and reads the values of B8G8R8A8_UNORM, R10G10B10A2_UNORM and
/// R11G11B10_FLOAT as floats, measured). Refused by name where the store cannot give the result, so the front clears otherwise:
/// a float clear of an integer format (`FloatClearOfInteger`; Direct3D 11 drops the call), the
/// most negative code of a signed normalised component (`SnormMostNegative`: a store gives -1.0 the
/// code one above it), and formats with no storage form (`StorageFormatAbsent`).
pub fn storage_clear_value(
    format: Format,
    values: [u32; 4],
    float: bool,
) -> Result<[u32; 4], ConvertError> {
    let absent = ConvertError::Unsupported(crate::refusal::STORAGE_FORMAT_ABSENT);
    if crate::lower::storage_format(format).is_none() {
        return Err(absent);
    }
    let (kind, components, _) = layout(vertex_format(format).ok_or(absent)?);
    if float {
        return match kind {
            Kind::Uint | Kind::Sint => Err(ConvertError::Unsupported(
                crate::refusal::FLOAT_CLEAR_OF_INTEGER,
            )),
            // Denormal floats as zeros of their sign, as Direct3D's float arithmetic takes them.
            _ => Ok(values.map(|v| {
                if v & 0x7f80_0000 == 0 {
                    v & 0x8000_0000
                } else {
                    v
                }
            })),
        };
    }
    let mut out = [0; 4];
    for (o, (&v, &(_, _, bits))) in out.iter_mut().zip(values.iter().zip(&components)) {
        let low = if bits == 32 { v } else { v & ((1 << bits) - 1) };
        let signed = ((low << (32 - bits)) as i32) >> (32 - bits);
        *o = match kind {
            Kind::Uint => low,
            Kind::Sint => signed as u32,
            // The code over its largest, which the store scales back to the code.
            Kind::Unorm => ((f64::from(low) / f64::from((1u32 << bits) - 1)) as f32).to_bits(),
            Kind::Snorm => {
                let max = (1i32 << (bits - 1)) - 1;
                if signed < -max {
                    return Err(ConvertError::Unsupported(
                        crate::refusal::SNORM_MOST_NEGATIVE,
                    ));
                }
                ((f64::from(signed) / f64::from(max)) as f32).to_bits()
            }
            Kind::Float if bits == 32 => low,
            Kind::Float => small_float(low, bits).to_bits(),
        };
    }
    Ok(out)
}

/// The value of a float of five exponent bits and `bits - 5` mantissa bits (11- and 10-bit floats,
/// unsigned), or of a half float (`bits` 16, signed); infinities and NaN payloads kept.
fn small_float(raw: u32, bits: u32) -> f32 {
    let m = if bits == 16 { 10 } else { bits - 5 };
    let exponent = (raw >> m) & 0x1f;
    let mantissa = raw & ((1 << m) - 1);
    let magnitude = match exponent {
        0 => mantissa as f32 / (1u64 << (14 + m)) as f32,
        31 => f32::from_bits(0x7f80_0000 | (mantissa << (23 - m))),
        e => f32::from_bits(((e + 112) << 23) | (mantissa << (23 - m))),
    };
    if bits == 16 && raw & 0x8000 != 0 {
        -magnitude
    } else {
        magnitude
    }
}
