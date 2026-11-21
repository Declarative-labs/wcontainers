//! What a SPIR-V program needs that our Vulkan 1.1 driver does not report (plan 15.4): a capability
//! beyond Vulkan 1.1's core, or one of 1.1's whose feature the driver reports absent. Such programs
//! are counted apart; no route is held to them.

use spirv::Capability as C;

/// The capabilities Vulkan 1.1's core allows a program (its SPIR-V environment appendix), the stages
/// the common form lacks left to the stage check.
const VULKAN_1_1: &[C] = &[
    C::Matrix,
    C::Shader,
    C::InputAttachment,
    C::Sampled1D,
    C::Image1D,
    C::SampledBuffer,
    C::ImageBuffer,
    C::ImageQuery,
    C::DerivativeControl,
    C::Geometry,
    C::Tessellation,
    C::Float64,
    C::Int64,
    C::Int16,
    C::TessellationPointSize,
    C::GeometryPointSize,
    C::ImageGatherExtended,
    C::StorageImageMultisample,
    C::UniformBufferArrayDynamicIndexing,
    C::SampledImageArrayDynamicIndexing,
    C::StorageBufferArrayDynamicIndexing,
    C::StorageImageArrayDynamicIndexing,
    C::ClipDistance,
    C::CullDistance,
    C::ImageCubeArray,
    C::SampleRateShading,
    C::SparseResidency,
    C::MinLod,
    C::SampledCubeArray,
    C::ImageMSArray,
    C::StorageImageExtendedFormats,
    C::InterpolationFunction,
    C::StorageImageReadWithoutFormat,
    C::StorageImageWriteWithoutFormat,
    C::MultiViewport,
    C::DrawParameters,
    C::MultiView,
    C::DeviceGroup,
    C::VariablePointersStorageBuffer,
    C::VariablePointers,
    C::StorageBuffer16BitAccess,
    C::UniformAndStorageBuffer16BitAccess,
    C::StoragePushConstant16,
    C::StorageInputOutput16,
    C::GroupNonUniform,
    C::GroupNonUniformVote,
    C::GroupNonUniformArithmetic,
    C::GroupNonUniformBallot,
    C::GroupNonUniformShuffle,
    C::GroupNonUniformShuffleRelative,
    C::GroupNonUniformClustered,
    C::GroupNonUniformQuad,
];

/// Vulkan 1.1's capabilities whose feature our driver reports absent, with the feature (15.4).
const ABSENT: &[(C, &str)] = &[
    (C::Int16, "shaderInt16"),
    (C::StorageImageMultisample, "shaderStorageImageMultisample"),
    (
        C::UniformBufferArrayDynamicIndexing,
        "shaderUniformBufferArrayDynamicIndexing",
    ),
    (
        C::StorageBufferArrayDynamicIndexing,
        "shaderStorageBufferArrayDynamicIndexing",
    ),
    (
        C::StorageImageArrayDynamicIndexing,
        "shaderStorageImageArrayDynamicIndexing",
    ),
    (C::StorageBuffer16BitAccess, "storageBuffer16BitAccess"),
    (
        C::UniformAndStorageBuffer16BitAccess,
        "uniformAndStorageBuffer16BitAccess",
    ),
    (C::StoragePushConstant16, "storagePushConstant16"),
    (C::StorageInputOutput16, "storageInputOutput16"),
    (C::MinLod, "shaderResourceMinLod"),
    (C::SparseResidency, "shaderResourceResidency"),
    (C::VariablePointers, "variablePointers"),
    (
        C::VariablePointersStorageBuffer,
        "variablePointersStorageBuffer",
    ),
];

/// Why our Vulkan 1.1 driver could not take the program, where it could not.
pub fn unreported(words: &[u32]) -> Option<String> {
    let mut reasons: Vec<String> = wcontainers_nir::spirv::instructions(words)
        .filter(|&(opcode, _)| opcode == spirv::Op::Capability as u32)
        .filter_map(|(_, inst)| {
            let raw = *inst.last()?;
            let Some(capability) = C::from_u32(raw) else {
                return Some(format!("capability {raw}, beyond Vulkan 1.1"));
            };
            if let Some((_, feature)) = ABSENT.iter().find(|(c, _)| *c == capability) {
                return Some(format!("{capability:?}: {feature} is reported absent"));
            }
            (!VULKAN_1_1.contains(&capability))
                .then(|| format!("{capability:?}, beyond Vulkan 1.1"))
        })
        .collect();
    reasons.sort();
    reasons.dedup();
    (!reasons.is_empty()).then(|| reasons.join(", "))
}
