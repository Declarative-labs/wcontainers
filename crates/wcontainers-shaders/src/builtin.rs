//! Built-in programs the fronts use: the clear of a storage texture view (Direct3D's
//! `ClearUnorderedAccessViewUint` and `ClearUnorderedAccessViewFloat` on textures), one program per
//! storage format and view dimension, writing one value into a box of the view; and the pixel
//! program of a pipeline without a pixel shader whose variant needs one.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use wcontainers_gpu::{
    Format, FormatKind, Reflection, ShaderModule, ShaderStage, SourceStage, StageExtras,
    VariantKey, ViewDimension,
};

use crate::{ConvertError, Converted};

/// The pixel program [`pixel_program_for_variant`] rewrites: one writing nothing.
const EMPTY_PIXEL_PROGRAM: &str = "@fragment fn main() {}\n";

/// The pixel program a pipeline without a pixel shader takes where its variant needs one (depth
/// clamped or biased, clip distances, counting, the sample mask): a program writing nothing, with
/// `variant`'s rewrites, as [`crate::convert`] gives them for a pixel shader.
pub fn pixel_program_for_variant(variant: &VariantKey) -> Result<Converted, ConvertError> {
    let mut module = naga::front::wgsl::parse_str(EMPTY_PIXEL_PROGRAM)
        .map_err(|e| ConvertError::Malformed(format!("the empty pixel program: {e}")))?;
    let mut reflection = Reflection {
        stage: SourceStage::Pixel,
        inputs: Vec::new(),
        outputs: Vec::new(),
        bindings: Vec::new(),
        helpers: Vec::new(),
        helper_size: 0,
        sampled_pairs: Vec::new(),
        extras: StageExtras::Pixel {
            early_tests: false,
            side_effects: false,
            discards: false,
            per_sample: false,
        },
        slow_paths: Vec::new(),
    };
    crate::unbuilt(variant, None)?;
    crate::variant::apply(&mut module, ShaderStage::Fragment, variant, &mut reflection)?;
    let mut key = blake3::Hasher::new();
    key.update(EMPTY_PIXEL_PROGRAM.as_bytes())
        .update(crate::CONVERTER_VERSION.as_bytes())
        .update(&postcard::to_allocvec(variant).expect("a variant key serialises"));
    Ok(Converted {
        modules: alloc::vec![ShaderModule {
            info: crate::validate(&module)?,
            module,
            entry_point: crate::ENTRY_POINT.into(),
            stage: ShaderStage::Fragment,
            key: *key.finalize().as_bytes(),
        }],
        reflection,
    })
}
use crate::variant::pull::{Kind, layout};
use crate::variant::texel::vertex_format;

/// The clear program's parameters in set 0: a uniform buffer of [`CLEAR_PARAMS_BYTES`], which
/// [`clear_params`] gives.
pub const CLEAR_PARAMS_BINDING: u32 = 0;
/// The storage view the clear program writes, in set 0, write-only, of the program's format.
pub const CLEAR_VIEW_BINDING: u32 = 1;
/// Invocations of a clear's workgroup along x and y; each workgroup covers one layer or slice.
pub const CLEAR_GROUP: [u32; 2] = [8, 8];
/// The axes a clear's texels span: x, y, and layer or slice.
pub const CLEAR_AXES: usize = 3;
/// The components of a clear value.
const CLEAR_COMPONENTS: usize = crate::build::VEC4 as usize;

/// The clear program's parameters as its `Clear` block lays them out: the value, then the first
/// texel and the end, each padded to four words.
#[repr(C)]
struct ClearBlock {
    value: [u32; CLEAR_COMPONENTS],
    origin: [u32; CLEAR_COMPONENTS],
    end: [u32; CLEAR_COMPONENTS],
}

/// Bytes of the clear program's parameters.
pub const CLEAR_PARAMS_BYTES: usize = core::mem::size_of::<ClearBlock>();

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
pub fn clear_params(
    value: [u32; CLEAR_COMPONENTS],
    origin: [u32; CLEAR_AXES],
    end: [u32; CLEAR_AXES],
) -> [u8; CLEAR_PARAMS_BYTES] {
    let mut out = [0; CLEAR_PARAMS_BYTES];
    let padded = |p: [u32; CLEAR_AXES]| core::array::from_fn(|i| p.get(i).copied().unwrap_or(0));
    let block = ClearBlock {
        value,
        origin: padded(origin),
        end: padded(end),
    };
    let words = block.value.into_iter().chain(block.origin).chain(block.end);
    for (o, w) in out.chunks_mut(crate::build::WORD as usize).zip(words) {
        o.copy_from_slice(&w.to_le_bytes());
    }
    out
}

/// The workgroups a clear from `origin` to `end` dispatches.
pub fn clear_groups(origin: [u32; CLEAR_AXES], end: [u32; CLEAR_AXES]) -> [u32; CLEAR_AXES] {
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
    values: [u32; CLEAR_COMPONENTS],
    float: bool,
) -> Result<[u32; CLEAR_COMPONENTS], ConvertError> {
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
                if v & crate::build::F32_EXPONENT_MASK == 0 {
                    v & crate::build::F32_SIGN_MASK
                } else {
                    v
                }
            })),
        };
    }
    let mut out = [0; CLEAR_COMPONENTS];
    for (o, (&v, &(_, _, bits))) in out.iter_mut().zip(values.iter().zip(&components)) {
        let low = if bits == u32::BITS {
            v
        } else {
            v & ((1 << bits) - 1)
        };
        let signed = ((low << (u32::BITS - bits)) as i32) >> (u32::BITS - bits);
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
            Kind::Float if bits == u32::BITS => low,
            Kind::Float => small_float(low, bits).to_bits(),
        };
    }
    Ok(out)
}

/// The value of a float of five exponent bits and `bits - 5` mantissa bits (11- and 10-bit floats,
/// unsigned), or of a half float (`bits` 16, signed); infinities and NaN payloads kept.
fn small_float(raw: u32, bits: u32) -> f32 {
    use crate::build::{
        F32_EXPONENT_BIAS, F32_EXPONENT_MASK, F32_FRACTION_BITS, HALF_BITS,
        SMALL_FLOAT_EXPONENT_BIAS, SMALL_FLOAT_EXPONENT_BITS, SMALL_FLOAT_EXPONENT_MASK,
    };
    let signed = bits == HALF_BITS;
    let m = bits - SMALL_FLOAT_EXPONENT_BITS - u32::from(signed);
    let exponent = (raw >> m) & SMALL_FLOAT_EXPONENT_MASK;
    let mantissa = raw & ((1 << m) - 1);
    let fraction = mantissa << (F32_FRACTION_BITS - m);
    let magnitude = match exponent {
        0 => mantissa as f32 / (1u64 << (SMALL_FLOAT_EXPONENT_BIAS - 1 + m)) as f32,
        SMALL_FLOAT_EXPONENT_MASK => f32::from_bits(F32_EXPONENT_MASK | fraction),
        e => f32::from_bits(
            ((e + F32_EXPONENT_BIAS - SMALL_FLOAT_EXPONENT_BIAS) << F32_FRACTION_BITS) | fraction,
        ),
    };
    if signed && raw & (1 << (HALF_BITS - 1)) != 0 {
        -magnitude
    } else {
        magnitude
    }
}
