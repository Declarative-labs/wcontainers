//! The tests every implementation of wcontainers-shader-converters runs, one conversion of each function on shaders of
//! the shaders crate's own tests (`crates/shaders/tests/own`), written against its traits alone.

use wcontainers_shader_converters::{BYTE, Failure, Function, NO_FLAGS, ShaderConverters, WORD};

/// fxc's DXBC of `compute.hlsl`'s `cs_images`.
const DXBC_IMAGES: &[u8] = include_bytes!("shaders/cs_images.dxbc");

/// dxc's DXIL of `texels.hlsl`'s `cs_texels`: `Buffer<float4>` t0 to t3, `RWBuffer<float4>` u0, `RWBuffer<uint>` u1
/// and a structured buffer at u2.
const DXIL_TEXELS: &[u8] = include_bytes!("shaders/cs_texels.dxil");

/// The resources `DXIL_TEXELS` declares.
const TEXELS_RESOURCES: usize = 7;

/// dxc's DXIL of `shadow.hlsl`'s `ps_shadow_compared_green`, which gathers the green channel compared.
const DXIL_COMPARED_GREEN: &[u8] = include_bytes!("shaders/ps_shadow_compared_green.dxil");

/// The channel `DXIL_COMPARED_GREEN` gathers: green, the second.
const GREEN: u32 = 1;

/// The words of one resource the converters list: its class, space and register.
const RESOURCE_WORDS: usize = 3;

/// SPIR-V's magic number, its first word (https://registry.khronos.org/SPIR-V/specs/unified1/SPIRV.html#Magic).
const SPIRV_MAGIC: u32 = 0x0723_0203;

/// Bytes that are no shader.
const NOT_A_SHADER: &[u8] = b"not a shader";

/// Returns the little-endian words of `bytes`.
fn words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .as_chunks::<WORD>()
        .0
        .iter()
        .map(|word| u32::from_le_bytes(*word))
        .collect()
}

/// Checks that DXBC becomes IR, with one declared size a word.
pub fn converts_dxbc_to_ir<T: ShaderConverters>(subject: T) {
    let [ir, declared] = subject
        .call(Function::DxbcToIr, DXBC_IMAGES, NO_FLAGS)
        .unwrap();
    assert!(ir.len() > BYTE);
    assert_eq!(declared.len() % WORD, 0);
}

/// Checks that DXIL becomes SPIR-V, with one class, space and register for each resource its bindings index.
pub fn converts_dxil_to_spirv<T: ShaderConverters>(subject: T) {
    let [spirv, resources] = subject
        .call(Function::DxilToSpirv, DXIL_TEXELS, NO_FLAGS)
        .unwrap();
    assert_eq!(words(&spirv).first(), Some(&SPIRV_MAGIC));
    assert!(!resources.is_empty());
    assert_eq!(resources.len() % (RESOURCE_WORDS * WORD), 0);
}

/// Checks that the constant sizes of DXIL come as a list of words, a space, register and size each.
pub fn lists_dxil_constant_sizes<T: ShaderConverters>(subject: T) {
    let [sizes, none] = subject
        .call(Function::DxilConstantSizes, DXIL_COMPARED_GREEN, NO_FLAGS)
        .unwrap();
    assert_eq!(sizes.len() % (RESOURCE_WORDS * WORD), 0);
    assert!(none.is_empty());
}

/// Checks that the channel of a compared gather is the one the shader gathers.
pub fn lists_dxil_compared_gather_channels<T: ShaderConverters>(subject: T) {
    let [channels, none] = subject
        .call(
            Function::DxilComparedGatherChannels,
            DXIL_COMPARED_GREEN,
            NO_FLAGS,
        )
        .unwrap();
    assert_eq!(words(&channels), [GREEN]);
    assert!(none.is_empty());
}

/// Checks that every register the DXIL declares is listed.
pub fn lists_dxil_declared_registers<T: ShaderConverters>(subject: T) {
    let [registers, none] = subject
        .call(Function::DxilDeclaredRegisters, DXIL_TEXELS, NO_FLAGS)
        .unwrap();
    assert_eq!(registers.len(), TEXELS_RESOURCES * RESOURCE_WORDS * WORD);
    assert!(none.is_empty());
}

/// Checks that the views of the DXIL's resource list come as words.
pub fn lists_dxil_views<T: ShaderConverters>(subject: T) {
    let [views, none] = subject
        .call(Function::DxilViews, DXIL_TEXELS, NO_FLAGS)
        .unwrap();
    assert!(!views.is_empty());
    assert_eq!(views.len() % WORD, 0);
    assert!(none.is_empty());
}

/// Checks that bytes that are no shader are refused with a reason.
pub fn refuses_what_is_no_shader<T: ShaderConverters>(subject: T) {
    let refusal = subject.call(Function::DxbcToIr, NOT_A_SHADER, NO_FLAGS);
    assert!(matches!(refusal, Err(Failure::Refused(reason)) if !reason.is_empty()));
}

/// Runs each suite function above as its own test on a value made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn converts_dxbc_to_ir() {
            $crate::converts_dxbc_to_ir($value);
        }

        #[test]
        fn converts_dxil_to_spirv() {
            $crate::converts_dxil_to_spirv($value);
        }

        #[test]
        fn lists_dxil_constant_sizes() {
            $crate::lists_dxil_constant_sizes($value);
        }

        #[test]
        fn lists_dxil_compared_gather_channels() {
            $crate::lists_dxil_compared_gather_channels($value);
        }

        #[test]
        fn lists_dxil_declared_registers() {
            $crate::lists_dxil_declared_registers($value);
        }

        #[test]
        fn lists_dxil_views() {
            $crate::lists_dxil_views($value);
        }

        #[test]
        fn refuses_what_is_no_shader() {
            $crate::refuses_what_is_no_shader($value);
        }
    };
}
