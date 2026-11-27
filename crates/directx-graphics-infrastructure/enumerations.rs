//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.

/// The header's `DXGI_ADAPTER_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_adapter_flag).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct AdapterFlag(pub i32);

/// The header's `DXGI_ADAPTER_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_adapter_flag).
pub const DXGI_ADAPTER_FLAG_NONE: AdapterFlag = AdapterFlag(0);
/// The header's `DXGI_ADAPTER_FLAG_REMOTE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_adapter_flag).
pub const DXGI_ADAPTER_FLAG_REMOTE: AdapterFlag = AdapterFlag(1);
/// The header's `DXGI_ADAPTER_FLAG_SOFTWARE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_adapter_flag).
pub const DXGI_ADAPTER_FLAG_SOFTWARE: AdapterFlag = AdapterFlag(2);

/// The header's `DXGI_ADAPTER_FLAG3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/ne-dxgi1_6-dxgi_adapter_flag3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct AdapterFlag3(pub i32);

/// The header's `DXGI_ADAPTER_FLAG3_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/ne-dxgi1_6-dxgi_adapter_flag3).
pub const DXGI_ADAPTER_FLAG3_NONE: AdapterFlag3 = AdapterFlag3(0);
/// The header's `DXGI_ADAPTER_FLAG3_REMOTE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/ne-dxgi1_6-dxgi_adapter_flag3).
pub const DXGI_ADAPTER_FLAG3_REMOTE: AdapterFlag3 = AdapterFlag3(1);
/// The header's `DXGI_ADAPTER_FLAG3_SOFTWARE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/ne-dxgi1_6-dxgi_adapter_flag3).
pub const DXGI_ADAPTER_FLAG3_SOFTWARE: AdapterFlag3 = AdapterFlag3(2);
/// The header's `DXGI_ADAPTER_FLAG3_ACG_COMPATIBLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/ne-dxgi1_6-dxgi_adapter_flag3).
pub const DXGI_ADAPTER_FLAG3_ACG_COMPATIBLE: AdapterFlag3 = AdapterFlag3(4);
/// The header's `DXGI_ADAPTER_FLAG3_SUPPORT_MONITORED_FENCES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/ne-dxgi1_6-dxgi_adapter_flag3).
pub const DXGI_ADAPTER_FLAG3_SUPPORT_MONITORED_FENCES: AdapterFlag3 = AdapterFlag3(8);
/// The header's `DXGI_ADAPTER_FLAG3_SUPPORT_NON_MONITORED_FENCES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/ne-dxgi1_6-dxgi_adapter_flag3).
pub const DXGI_ADAPTER_FLAG3_SUPPORT_NON_MONITORED_FENCES: AdapterFlag3 = AdapterFlag3(16);
/// The header's `DXGI_ADAPTER_FLAG3_KEYED_MUTEX_CONFORMANCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/ne-dxgi1_6-dxgi_adapter_flag3).
pub const DXGI_ADAPTER_FLAG3_KEYED_MUTEX_CONFORMANCE: AdapterFlag3 = AdapterFlag3(32);

/// The header's `DXGI_COMPUTE_PREEMPTION_GRANULARITY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_compute_preemption_granularity).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ComputePreemptionGranularity(pub i32);

/// The header's `DXGI_COMPUTE_PREEMPTION_DMA_BUFFER_BOUNDARY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_compute_preemption_granularity).
pub const DXGI_COMPUTE_PREEMPTION_DMA_BUFFER_BOUNDARY: ComputePreemptionGranularity =
    ComputePreemptionGranularity(0);
/// The header's `DXGI_COMPUTE_PREEMPTION_DISPATCH_BOUNDARY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_compute_preemption_granularity).
pub const DXGI_COMPUTE_PREEMPTION_DISPATCH_BOUNDARY: ComputePreemptionGranularity =
    ComputePreemptionGranularity(1);
/// The header's `DXGI_COMPUTE_PREEMPTION_THREAD_GROUP_BOUNDARY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_compute_preemption_granularity).
pub const DXGI_COMPUTE_PREEMPTION_THREAD_GROUP_BOUNDARY: ComputePreemptionGranularity =
    ComputePreemptionGranularity(2);
/// The header's `DXGI_COMPUTE_PREEMPTION_THREAD_BOUNDARY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_compute_preemption_granularity).
pub const DXGI_COMPUTE_PREEMPTION_THREAD_BOUNDARY: ComputePreemptionGranularity =
    ComputePreemptionGranularity(3);
/// The header's `DXGI_COMPUTE_PREEMPTION_INSTRUCTION_BOUNDARY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_compute_preemption_granularity).
pub const DXGI_COMPUTE_PREEMPTION_INSTRUCTION_BOUNDARY: ComputePreemptionGranularity =
    ComputePreemptionGranularity(4);

/// The header's `DXGI_CREATE_FACTORY_FLAGS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CreateFactoryFlags(pub u32);

/// The header's `DXGI_CREATE_FACTORY_DEBUG`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_CREATE_FACTORY_DEBUG: CreateFactoryFlags = CreateFactoryFlags(1);

/// The header's `DXGI_DEBUG_RLO_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/ne-dxgidebug-dxgi_debug_rlo_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DebugRloFlags(pub i32);

/// The header's `DXGI_DEBUG_RLO_SUMMARY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/ne-dxgidebug-dxgi_debug_rlo_flags).
pub const DXGI_DEBUG_RLO_SUMMARY: DebugRloFlags = DebugRloFlags(1);
/// The header's `DXGI_DEBUG_RLO_DETAIL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/ne-dxgidebug-dxgi_debug_rlo_flags).
pub const DXGI_DEBUG_RLO_DETAIL: DebugRloFlags = DebugRloFlags(2);
/// The header's `DXGI_DEBUG_RLO_IGNORE_INTERNAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/ne-dxgidebug-dxgi_debug_rlo_flags).
pub const DXGI_DEBUG_RLO_IGNORE_INTERNAL: DebugRloFlags = DebugRloFlags(4);
/// The header's `DXGI_DEBUG_RLO_ALL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/ne-dxgidebug-dxgi_debug_rlo_flags).
pub const DXGI_DEBUG_RLO_ALL: DebugRloFlags = DebugRloFlags(7);

/// The header's `DXGI_ENUM_MODES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-enum-modes).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct EnumModes(pub u32);

/// The header's `DXGI_ENUM_MODES_INTERLACED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-enum-modes).
pub const DXGI_ENUM_MODES_INTERLACED: EnumModes = EnumModes(1);
/// The header's `DXGI_ENUM_MODES_SCALING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-enum-modes).
pub const DXGI_ENUM_MODES_SCALING: EnumModes = EnumModes(2);
/// The header's `DXGI_ENUM_MODES_STEREO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-enum-modes).
pub const DXGI_ENUM_MODES_STEREO: EnumModes = EnumModes(4);
/// The header's `DXGI_ENUM_MODES_DISABLED_STEREO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-enum-modes).
pub const DXGI_ENUM_MODES_DISABLED_STEREO: EnumModes = EnumModes(8);

/// The header's `DXGI_FEATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_5/ne-dxgi1_5-dxgi_feature).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Feature(pub i32);

/// The header's `DXGI_FEATURE_PRESENT_ALLOW_TEARING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_5/ne-dxgi1_5-dxgi_feature).
pub const DXGI_FEATURE_PRESENT_ALLOW_TEARING: Feature = Feature(0);

/// The header's `DXGI_FRAME_PRESENTATION_MODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/ne-dxgi1_3-dxgi_frame_presentation_mode).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FramePresentationMode(pub i32);

/// The header's `DXGI_FRAME_PRESENTATION_MODE_COMPOSED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/ne-dxgi1_3-dxgi_frame_presentation_mode).
pub const DXGI_FRAME_PRESENTATION_MODE_COMPOSED: FramePresentationMode = FramePresentationMode(0);
/// The header's `DXGI_FRAME_PRESENTATION_MODE_OVERLAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/ne-dxgi1_3-dxgi_frame_presentation_mode).
pub const DXGI_FRAME_PRESENTATION_MODE_OVERLAY: FramePresentationMode = FramePresentationMode(1);
/// The header's `DXGI_FRAME_PRESENTATION_MODE_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/ne-dxgi1_3-dxgi_frame_presentation_mode).
pub const DXGI_FRAME_PRESENTATION_MODE_NONE: FramePresentationMode = FramePresentationMode(2);
/// The header's `DXGI_FRAME_PRESENTATION_MODE_COMPOSITION_FAILURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/ne-dxgi1_3-dxgi_frame_presentation_mode).
pub const DXGI_FRAME_PRESENTATION_MODE_COMPOSITION_FAILURE: FramePresentationMode =
    FramePresentationMode(3);

/// The header's `DXGI_GPU_PREFERENCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/ne-dxgi1_6-dxgi_gpu_preference).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct GpuPreference(pub i32);

/// The header's `DXGI_GPU_PREFERENCE_UNSPECIFIED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/ne-dxgi1_6-dxgi_gpu_preference).
pub const DXGI_GPU_PREFERENCE_UNSPECIFIED: GpuPreference = GpuPreference(0);
/// The header's `DXGI_GPU_PREFERENCE_MINIMUM_POWER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/ne-dxgi1_6-dxgi_gpu_preference).
pub const DXGI_GPU_PREFERENCE_MINIMUM_POWER: GpuPreference = GpuPreference(1);
/// The header's `DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/ne-dxgi1_6-dxgi_gpu_preference).
pub const DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE: GpuPreference = GpuPreference(2);

/// The header's `DXGI_GRAPHICS_PREEMPTION_GRANULARITY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_graphics_preemption_granularity).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct GraphicsPreemptionGranularity(pub i32);

/// The header's `DXGI_GRAPHICS_PREEMPTION_DMA_BUFFER_BOUNDARY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_graphics_preemption_granularity).
pub const DXGI_GRAPHICS_PREEMPTION_DMA_BUFFER_BOUNDARY: GraphicsPreemptionGranularity =
    GraphicsPreemptionGranularity(0);
/// The header's `DXGI_GRAPHICS_PREEMPTION_PRIMITIVE_BOUNDARY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_graphics_preemption_granularity).
pub const DXGI_GRAPHICS_PREEMPTION_PRIMITIVE_BOUNDARY: GraphicsPreemptionGranularity =
    GraphicsPreemptionGranularity(1);
/// The header's `DXGI_GRAPHICS_PREEMPTION_TRIANGLE_BOUNDARY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_graphics_preemption_granularity).
pub const DXGI_GRAPHICS_PREEMPTION_TRIANGLE_BOUNDARY: GraphicsPreemptionGranularity =
    GraphicsPreemptionGranularity(2);
/// The header's `DXGI_GRAPHICS_PREEMPTION_PIXEL_BOUNDARY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_graphics_preemption_granularity).
pub const DXGI_GRAPHICS_PREEMPTION_PIXEL_BOUNDARY: GraphicsPreemptionGranularity =
    GraphicsPreemptionGranularity(3);
/// The header's `DXGI_GRAPHICS_PREEMPTION_INSTRUCTION_BOUNDARY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_graphics_preemption_granularity).
pub const DXGI_GRAPHICS_PREEMPTION_INSTRUCTION_BOUNDARY: GraphicsPreemptionGranularity =
    GraphicsPreemptionGranularity(4);

/// The header's `DXGI_HARDWARE_COMPOSITION_SUPPORT_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/ne-dxgi1_6-dxgi_hardware_composition_support_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HardwareCompositionSupportFlags(pub i32);

/// The header's `DXGI_HARDWARE_COMPOSITION_SUPPORT_FLAG_FULLSCREEN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/ne-dxgi1_6-dxgi_hardware_composition_support_flags).
pub const DXGI_HARDWARE_COMPOSITION_SUPPORT_FLAG_FULLSCREEN: HardwareCompositionSupportFlags =
    HardwareCompositionSupportFlags(1);
/// The header's `DXGI_HARDWARE_COMPOSITION_SUPPORT_FLAG_WINDOWED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/ne-dxgi1_6-dxgi_hardware_composition_support_flags).
pub const DXGI_HARDWARE_COMPOSITION_SUPPORT_FLAG_WINDOWED: HardwareCompositionSupportFlags =
    HardwareCompositionSupportFlags(2);
/// The header's `DXGI_HARDWARE_COMPOSITION_SUPPORT_FLAG_CURSOR_STRETCHED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/ne-dxgi1_6-dxgi_hardware_composition_support_flags).
pub const DXGI_HARDWARE_COMPOSITION_SUPPORT_FLAG_CURSOR_STRETCHED: HardwareCompositionSupportFlags =
    HardwareCompositionSupportFlags(4);

/// The header's `DXGI_HDR_METADATA_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_5/ne-dxgi1_5-dxgi_hdr_metadata_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HdrMetadataType(pub i32);

/// The header's `DXGI_HDR_METADATA_TYPE_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_5/ne-dxgi1_5-dxgi_hdr_metadata_type).
pub const DXGI_HDR_METADATA_TYPE_NONE: HdrMetadataType = HdrMetadataType(0);
/// The header's `DXGI_HDR_METADATA_TYPE_HDR10`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_5/ne-dxgi1_5-dxgi_hdr_metadata_type).
pub const DXGI_HDR_METADATA_TYPE_HDR10: HdrMetadataType = HdrMetadataType(1);
/// The header's `DXGI_HDR_METADATA_TYPE_HDR10PLUS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_5/ne-dxgi1_5-dxgi_hdr_metadata_type).
pub const DXGI_HDR_METADATA_TYPE_HDR10PLUS: HdrMetadataType = HdrMetadataType(2);

/// The header's `DXGI_INFO_QUEUE_MESSAGE_CATEGORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/ne-dxgidebug-dxgi_info_queue_message_category).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct InfoQueueMessageCategory(pub i32);

/// The header's `DXGI_INFO_QUEUE_MESSAGE_CATEGORY_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/ne-dxgidebug-dxgi_info_queue_message_category).
pub const DXGI_INFO_QUEUE_MESSAGE_CATEGORY_UNKNOWN: InfoQueueMessageCategory =
    InfoQueueMessageCategory(0);
/// The header's `DXGI_INFO_QUEUE_MESSAGE_CATEGORY_MISCELLANEOUS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/ne-dxgidebug-dxgi_info_queue_message_category).
pub const DXGI_INFO_QUEUE_MESSAGE_CATEGORY_MISCELLANEOUS: InfoQueueMessageCategory =
    InfoQueueMessageCategory(1);
/// The header's `DXGI_INFO_QUEUE_MESSAGE_CATEGORY_INITIALIZATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/ne-dxgidebug-dxgi_info_queue_message_category).
pub const DXGI_INFO_QUEUE_MESSAGE_CATEGORY_INITIALIZATION: InfoQueueMessageCategory =
    InfoQueueMessageCategory(2);
/// The header's `DXGI_INFO_QUEUE_MESSAGE_CATEGORY_CLEANUP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/ne-dxgidebug-dxgi_info_queue_message_category).
pub const DXGI_INFO_QUEUE_MESSAGE_CATEGORY_CLEANUP: InfoQueueMessageCategory =
    InfoQueueMessageCategory(3);
/// The header's `DXGI_INFO_QUEUE_MESSAGE_CATEGORY_COMPILATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/ne-dxgidebug-dxgi_info_queue_message_category).
pub const DXGI_INFO_QUEUE_MESSAGE_CATEGORY_COMPILATION: InfoQueueMessageCategory =
    InfoQueueMessageCategory(4);
/// The header's `DXGI_INFO_QUEUE_MESSAGE_CATEGORY_STATE_CREATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/ne-dxgidebug-dxgi_info_queue_message_category).
pub const DXGI_INFO_QUEUE_MESSAGE_CATEGORY_STATE_CREATION: InfoQueueMessageCategory =
    InfoQueueMessageCategory(5);
/// The header's `DXGI_INFO_QUEUE_MESSAGE_CATEGORY_STATE_SETTING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/ne-dxgidebug-dxgi_info_queue_message_category).
pub const DXGI_INFO_QUEUE_MESSAGE_CATEGORY_STATE_SETTING: InfoQueueMessageCategory =
    InfoQueueMessageCategory(6);
/// The header's `DXGI_INFO_QUEUE_MESSAGE_CATEGORY_STATE_GETTING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/ne-dxgidebug-dxgi_info_queue_message_category).
pub const DXGI_INFO_QUEUE_MESSAGE_CATEGORY_STATE_GETTING: InfoQueueMessageCategory =
    InfoQueueMessageCategory(7);
/// The header's `DXGI_INFO_QUEUE_MESSAGE_CATEGORY_RESOURCE_MANIPULATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/ne-dxgidebug-dxgi_info_queue_message_category).
pub const DXGI_INFO_QUEUE_MESSAGE_CATEGORY_RESOURCE_MANIPULATION: InfoQueueMessageCategory =
    InfoQueueMessageCategory(8);
/// The header's `DXGI_INFO_QUEUE_MESSAGE_CATEGORY_EXECUTION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/ne-dxgidebug-dxgi_info_queue_message_category).
pub const DXGI_INFO_QUEUE_MESSAGE_CATEGORY_EXECUTION: InfoQueueMessageCategory =
    InfoQueueMessageCategory(9);
/// The header's `DXGI_INFO_QUEUE_MESSAGE_CATEGORY_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/ne-dxgidebug-dxgi_info_queue_message_category).
pub const DXGI_INFO_QUEUE_MESSAGE_CATEGORY_SHADER: InfoQueueMessageCategory =
    InfoQueueMessageCategory(10);

/// The header's `DXGI_INFO_QUEUE_MESSAGE_SEVERITY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/ne-dxgidebug-dxgi_info_queue_message_severity).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct InfoQueueMessageSeverity(pub i32);

/// The header's `DXGI_INFO_QUEUE_MESSAGE_SEVERITY_CORRUPTION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/ne-dxgidebug-dxgi_info_queue_message_severity).
pub const DXGI_INFO_QUEUE_MESSAGE_SEVERITY_CORRUPTION: InfoQueueMessageSeverity =
    InfoQueueMessageSeverity(0);
/// The header's `DXGI_INFO_QUEUE_MESSAGE_SEVERITY_ERROR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/ne-dxgidebug-dxgi_info_queue_message_severity).
pub const DXGI_INFO_QUEUE_MESSAGE_SEVERITY_ERROR: InfoQueueMessageSeverity =
    InfoQueueMessageSeverity(1);
/// The header's `DXGI_INFO_QUEUE_MESSAGE_SEVERITY_WARNING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/ne-dxgidebug-dxgi_info_queue_message_severity).
pub const DXGI_INFO_QUEUE_MESSAGE_SEVERITY_WARNING: InfoQueueMessageSeverity =
    InfoQueueMessageSeverity(2);
/// The header's `DXGI_INFO_QUEUE_MESSAGE_SEVERITY_INFO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/ne-dxgidebug-dxgi_info_queue_message_severity).
pub const DXGI_INFO_QUEUE_MESSAGE_SEVERITY_INFO: InfoQueueMessageSeverity =
    InfoQueueMessageSeverity(3);
/// The header's `DXGI_INFO_QUEUE_MESSAGE_SEVERITY_MESSAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/ne-dxgidebug-dxgi_info_queue_message_severity).
pub const DXGI_INFO_QUEUE_MESSAGE_SEVERITY_MESSAGE: InfoQueueMessageSeverity =
    InfoQueueMessageSeverity(4);

/// The header's `DXGI_MAP_FLAGS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MapFlags(pub u32);

/// The header's `DXGI_MAP_READ`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MAP_READ: MapFlags = MapFlags(1);
/// The header's `DXGI_MAP_WRITE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MAP_WRITE: MapFlags = MapFlags(2);
/// The header's `DXGI_MAP_DISCARD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MAP_DISCARD: MapFlags = MapFlags(4);

/// The header's `DXGI_MEMORY_SEGMENT_GROUP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/ne-dxgi1_4-dxgi_memory_segment_group).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MemorySegmentGroup(pub i32);

/// The header's `DXGI_MEMORY_SEGMENT_GROUP_LOCAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/ne-dxgi1_4-dxgi_memory_segment_group).
pub const DXGI_MEMORY_SEGMENT_GROUP_LOCAL: MemorySegmentGroup = MemorySegmentGroup(0);
/// The header's `DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/ne-dxgi1_4-dxgi_memory_segment_group).
pub const DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL: MemorySegmentGroup = MemorySegmentGroup(1);

/// The header's `DXGI_MULTIPLANE_OVERLAY_YCbCr_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/ne-dxgi1_3-dxgi_multiplane_overlay_ycbcr_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MultiplaneOverlayYcbcrFlags(pub i32);

/// The header's `DXGI_MULTIPLANE_OVERLAY_YCbCr_FLAG_NOMINAL_RANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/ne-dxgi1_3-dxgi_multiplane_overlay_ycbcr_flags).
pub const DXGI_MULTIPLANE_OVERLAY_YCBCR_FLAG_NOMINAL_RANGE: MultiplaneOverlayYcbcrFlags =
    MultiplaneOverlayYcbcrFlags(1);
/// The header's `DXGI_MULTIPLANE_OVERLAY_YCbCr_FLAG_BT709`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/ne-dxgi1_3-dxgi_multiplane_overlay_ycbcr_flags).
pub const DXGI_MULTIPLANE_OVERLAY_YCBCR_FLAG_BT709: MultiplaneOverlayYcbcrFlags =
    MultiplaneOverlayYcbcrFlags(2);
/// The header's `DXGI_MULTIPLANE_OVERLAY_YCbCr_FLAG_xvYCC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/ne-dxgi1_3-dxgi_multiplane_overlay_ycbcr_flags).
pub const DXGI_MULTIPLANE_OVERLAY_YCBCR_FLAG_XVYCC: MultiplaneOverlayYcbcrFlags =
    MultiplaneOverlayYcbcrFlags(4);

/// The header's `DXGI_MWA_FLAGS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MwaFlags(pub u32);

/// The header's `DXGI_MWA_NO_WINDOW_CHANGES`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MWA_NO_WINDOW_CHANGES: MwaFlags = MwaFlags(1);
/// The header's `DXGI_MWA_NO_ALT_ENTER`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MWA_NO_ALT_ENTER: MwaFlags = MwaFlags(2);
/// The header's `DXGI_MWA_NO_PRINT_SCREEN`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MWA_NO_PRINT_SCREEN: MwaFlags = MwaFlags(4);
/// The header's `DXGI_MWA_VALID`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MWA_VALID: MwaFlags = MwaFlags(7);

/// The header's `DXGI_Message_Id`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MessageId(pub i32);

/// The header's `DXGI_MSG_IDXGISwapChain_CreationOrResizeBuffers_InvalidOutputWindow`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_CREATIONORRESIZEBUFFERS_INVALIDOUTPUTWINDOW: MessageId =
    MessageId(0);
/// The header's `DXGI_MSG_IDXGISwapChain_CreationOrResizeBuffers_BufferWidthInferred`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_CREATIONORRESIZEBUFFERS_BUFFERWIDTHINFERRED: MessageId =
    MessageId(1);
/// The header's `DXGI_MSG_IDXGISwapChain_CreationOrResizeBuffers_BufferHeightInferred`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_CREATIONORRESIZEBUFFERS_BUFFERHEIGHTINFERRED: MessageId =
    MessageId(2);
/// The header's `DXGI_MSG_IDXGISwapChain_CreationOrResizeBuffers_NoScanoutFlagChanged`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_CREATIONORRESIZEBUFFERS_NOSCANOUTFLAGCHANGED: MessageId =
    MessageId(3);
/// The header's `DXGI_MSG_IDXGISwapChain_Creation_MaxBufferCountExceeded`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_CREATION_MAXBUFFERCOUNTEXCEEDED: MessageId = MessageId(4);
/// The header's `DXGI_MSG_IDXGISwapChain_Creation_TooFewBuffers`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_CREATION_TOOFEWBUFFERS: MessageId = MessageId(5);
/// The header's `DXGI_MSG_IDXGISwapChain_Creation_NoOutputWindow`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_CREATION_NOOUTPUTWINDOW: MessageId = MessageId(6);
/// The header's `DXGI_MSG_IDXGISwapChain_Destruction_OtherMethodsCalled`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_DESTRUCTION_OTHERMETHODSCALLED: MessageId = MessageId(7);
/// The header's `DXGI_MSG_IDXGISwapChain_GetDesc_pDescIsNULL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_GETDESC_PDESCISNULL: MessageId = MessageId(8);
/// The header's `DXGI_MSG_IDXGISwapChain_GetBuffer_ppSurfaceIsNULL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_GETBUFFER_PPSURFACEISNULL: MessageId = MessageId(9);
/// The header's `DXGI_MSG_IDXGISwapChain_GetBuffer_NoAllocatedBuffers`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_GETBUFFER_NOALLOCATEDBUFFERS: MessageId = MessageId(10);
/// The header's `DXGI_MSG_IDXGISwapChain_GetBuffer_iBufferMustBeZero`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_GETBUFFER_IBUFFERMUSTBEZERO: MessageId = MessageId(11);
/// The header's `DXGI_MSG_IDXGISwapChain_GetBuffer_iBufferOOB`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_GETBUFFER_IBUFFEROOB: MessageId = MessageId(12);
/// The header's `DXGI_MSG_IDXGISwapChain_GetContainingOutput_ppOutputIsNULL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_GETCONTAININGOUTPUT_PPOUTPUTISNULL: MessageId = MessageId(13);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_SyncIntervalOOB`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_SYNCINTERVALOOB: MessageId = MessageId(14);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_InvalidNonPreRotatedFlag`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_INVALIDNONPREROTATEDFLAG: MessageId = MessageId(15);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_NoAllocatedBuffers`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_NOALLOCATEDBUFFERS: MessageId = MessageId(16);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_GetDXGIAdapterFailed`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_GETDXGIADAPTERFAILED: MessageId = MessageId(17);
/// The header's `DXGI_MSG_IDXGISwapChain_ResizeBuffers_BufferCountOOB`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RESIZEBUFFERS_BUFFERCOUNTOOB: MessageId = MessageId(18);
/// The header's `DXGI_MSG_IDXGISwapChain_ResizeBuffers_UnreleasedReferences`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RESIZEBUFFERS_UNRELEASEDREFERENCES: MessageId = MessageId(19);
/// The header's `DXGI_MSG_IDXGISwapChain_ResizeBuffers_InvalidSwapChainFlag`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RESIZEBUFFERS_INVALIDSWAPCHAINFLAG: MessageId = MessageId(20);
/// The header's `DXGI_MSG_IDXGISwapChain_ResizeBuffers_InvalidNonPreRotatedFlag`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RESIZEBUFFERS_INVALIDNONPREROTATEDFLAG: MessageId = MessageId(21);
/// The header's `DXGI_MSG_IDXGISwapChain_ResizeTarget_RefreshRateDivideByZero`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RESIZETARGET_REFRESHRATEDIVIDEBYZERO: MessageId = MessageId(22);
/// The header's `DXGI_MSG_IDXGISwapChain_SetFullscreenState_InvalidTarget`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_SETFULLSCREENSTATE_INVALIDTARGET: MessageId = MessageId(23);
/// The header's `DXGI_MSG_IDXGISwapChain_GetFrameStatistics_pStatsIsNULL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_GETFRAMESTATISTICS_PSTATSISNULL: MessageId = MessageId(24);
/// The header's `DXGI_MSG_IDXGISwapChain_GetLastPresentCount_pLastPresentCountIsNULL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_GETLASTPRESENTCOUNT_PLASTPRESENTCOUNTISNULL: MessageId =
    MessageId(25);
/// The header's `DXGI_MSG_IDXGISwapChain_SetFullscreenState_RemoteNotSupported`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_SETFULLSCREENSTATE_REMOTENOTSUPPORTED: MessageId = MessageId(26);
/// The header's `DXGI_MSG_IDXGIOutput_TakeOwnership_FailedToAcquireFullscreenMutex`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_TAKEOWNERSHIP_FAILEDTOACQUIREFULLSCREENMUTEX: MessageId =
    MessageId(27);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSoftwareAdapter_ppAdapterInterfaceIsNULL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESOFTWAREADAPTER_PPADAPTERINTERFACEISNULL: MessageId =
    MessageId(28);
/// The header's `DXGI_MSG_IDXGIFactory_EnumAdapters_ppAdapterInterfaceIsNULL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_ENUMADAPTERS_PPADAPTERINTERFACEISNULL: MessageId = MessageId(29);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_ppSwapChainIsNULL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_PPSWAPCHAINISNULL: MessageId = MessageId(30);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_pDescIsNULL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_PDESCISNULL: MessageId = MessageId(31);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_UnknownSwapEffect`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_UNKNOWNSWAPEFFECT: MessageId = MessageId(32);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_InvalidFlags`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_INVALIDFLAGS: MessageId = MessageId(33);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_NonPreRotatedFlagAndWindowed`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_NONPREROTATEDFLAGANDWINDOWED: MessageId =
    MessageId(34);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_NullDeviceInterface`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_NULLDEVICEINTERFACE: MessageId = MessageId(35);
/// The header's `DXGI_MSG_IDXGIFactory_GetWindowAssociation_phWndIsNULL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_GETWINDOWASSOCIATION_PHWNDISNULL: MessageId = MessageId(36);
/// The header's `DXGI_MSG_IDXGIFactory_MakeWindowAssociation_InvalidFlags`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_MAKEWINDOWASSOCIATION_INVALIDFLAGS: MessageId = MessageId(37);
/// The header's `DXGI_MSG_IDXGISurface_Map_InvalidSurface`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISURFACE_MAP_INVALIDSURFACE: MessageId = MessageId(38);
/// The header's `DXGI_MSG_IDXGISurface_Map_FlagsSetToZero`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISURFACE_MAP_FLAGSSETTOZERO: MessageId = MessageId(39);
/// The header's `DXGI_MSG_IDXGISurface_Map_DiscardAndReadFlagSet`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISURFACE_MAP_DISCARDANDREADFLAGSET: MessageId = MessageId(40);
/// The header's `DXGI_MSG_IDXGISurface_Map_DiscardButNotWriteFlagSet`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISURFACE_MAP_DISCARDBUTNOTWRITEFLAGSET: MessageId = MessageId(41);
/// The header's `DXGI_MSG_IDXGISurface_Map_NoCPUAccess`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISURFACE_MAP_NOCPUACCESS: MessageId = MessageId(42);
/// The header's `DXGI_MSG_IDXGISurface_Map_ReadFlagSetButCPUAccessIsDynamic`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISURFACE_MAP_READFLAGSETBUTCPUACCESSISDYNAMIC: MessageId = MessageId(43);
/// The header's `DXGI_MSG_IDXGISurface_Map_DiscardFlagSetButCPUAccessIsNotDynamic`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISURFACE_MAP_DISCARDFLAGSETBUTCPUACCESSISNOTDYNAMIC: MessageId =
    MessageId(44);
/// The header's `DXGI_MSG_IDXGIOutput_GetDisplayModeList_pNumModesIsNULL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_GETDISPLAYMODELIST_PNUMMODESISNULL: MessageId = MessageId(45);
/// The header's `DXGI_MSG_IDXGIOutput_FindClosestMatchingMode_ModeHasInvalidWidthOrHeight`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_FINDCLOSESTMATCHINGMODE_MODEHASINVALIDWIDTHORHEIGHT: MessageId =
    MessageId(46);
/// The header's `DXGI_MSG_IDXGIOutput_GetCammaControlCapabilities_NoOwnerDevice`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_GETCAMMACONTROLCAPABILITIES_NOOWNERDEVICE: MessageId = MessageId(47);
/// The header's `DXGI_MSG_IDXGIOutput_TakeOwnership_pDeviceIsNULL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_TAKEOWNERSHIP_PDEVICEISNULL: MessageId = MessageId(48);
/// The header's `DXGI_MSG_IDXGIOutput_GetDisplaySurfaceData_NoOwnerDevice`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_GETDISPLAYSURFACEDATA_NOOWNERDEVICE: MessageId = MessageId(49);
/// The header's `DXGI_MSG_IDXGIOutput_GetDisplaySurfaceData_pDestinationIsNULL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_GETDISPLAYSURFACEDATA_PDESTINATIONISNULL: MessageId = MessageId(50);
/// The header's `DXGI_MSG_IDXGIOutput_GetDisplaySurfaceData_MapOfDestinationFailed`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_GETDISPLAYSURFACEDATA_MAPOFDESTINATIONFAILED: MessageId =
    MessageId(51);
/// The header's `DXGI_MSG_IDXGIOutput_GetFrameStatistics_NoOwnerDevice`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_GETFRAMESTATISTICS_NOOWNERDEVICE: MessageId = MessageId(52);
/// The header's `DXGI_MSG_IDXGIOutput_GetFrameStatistics_pStatsIsNULL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_GETFRAMESTATISTICS_PSTATSISNULL: MessageId = MessageId(53);
/// The header's `DXGI_MSG_IDXGIOutput_SetGammaControl_NoOwnerDevice`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_SETGAMMACONTROL_NOOWNERDEVICE: MessageId = MessageId(54);
/// The header's `DXGI_MSG_IDXGIOutput_GetGammaControl_NoOwnerDevice`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_GETGAMMACONTROL_NOOWNERDEVICE: MessageId = MessageId(55);
/// The header's `DXGI_MSG_IDXGIOutput_GetGammaControl_NoGammaControls`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_GETGAMMACONTROL_NOGAMMACONTROLS: MessageId = MessageId(56);
/// The header's `DXGI_MSG_IDXGIOutput_SetDisplaySurface_IDXGIResourceNotSupportedBypPrimary`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_SETDISPLAYSURFACE_IDXGIRESOURCENOTSUPPORTEDBYPPRIMARY: MessageId =
    MessageId(57);
/// The header's `DXGI_MSG_IDXGIOutput_SetDisplaySurface_pPrimaryIsInvalid`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_SETDISPLAYSURFACE_PPRIMARYISINVALID: MessageId = MessageId(58);
/// The header's `DXGI_MSG_IDXGIOutput_SetDisplaySurface_NoOwnerDevice`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_SETDISPLAYSURFACE_NOOWNERDEVICE: MessageId = MessageId(59);
/// The header's `DXGI_MSG_IDXGIOutput_TakeOwnership_RemoteDeviceNotSupported`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_TAKEOWNERSHIP_REMOTEDEVICENOTSUPPORTED: MessageId = MessageId(60);
/// The header's `DXGI_MSG_IDXGIOutput_GetDisplayModeList_RemoteDeviceNotSupported`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_GETDISPLAYMODELIST_REMOTEDEVICENOTSUPPORTED: MessageId =
    MessageId(61);
/// The header's `DXGI_MSG_IDXGIOutput_FindClosestMatchingMode_RemoteDeviceNotSupported`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_FINDCLOSESTMATCHINGMODE_REMOTEDEVICENOTSUPPORTED: MessageId =
    MessageId(62);
/// The header's `DXGI_MSG_IDXGIDevice_CreateSurface_InvalidParametersWithpSharedResource`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIDEVICE_CREATESURFACE_INVALIDPARAMETERSWITHPSHAREDRESOURCE: MessageId =
    MessageId(63);
/// The header's `DXGI_MSG_IDXGIObject_GetPrivateData_puiDataSizeIsNULL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOBJECT_GETPRIVATEDATA_PUIDATASIZEISNULL: MessageId = MessageId(64);
/// The header's `DXGI_MSG_IDXGISwapChain_Creation_InvalidOutputWindow`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_CREATION_INVALIDOUTPUTWINDOW: MessageId = MessageId(65);
/// The header's `DXGI_MSG_IDXGISwapChain_Release_SwapChainIsFullscreen`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RELEASE_SWAPCHAINISFULLSCREEN: MessageId = MessageId(66);
/// The header's `DXGI_MSG_IDXGIOutput_GetDisplaySurfaceData_InvalidTargetSurfaceFormat`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_GETDISPLAYSURFACEDATA_INVALIDTARGETSURFACEFORMAT: MessageId =
    MessageId(67);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSoftwareAdapter_ModuleIsNULL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESOFTWAREADAPTER_MODULEISNULL: MessageId = MessageId(68);
/// The header's `DXGI_MSG_IDXGIOutput_FindClosestMatchingMode_IDXGIDeviceNotSupportedBypConcernedDevice`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_FINDCLOSESTMATCHINGMODE_IDXGIDEVICENOTSUPPORTEDBYPCONCERNEDDEVICE:
    MessageId = MessageId(69);
/// The header's `DXGI_MSG_IDXGIOutput_FindClosestMatchingMode_pModeToMatchOrpClosestMatchIsNULL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_FINDCLOSESTMATCHINGMODE_PMODETOMATCHORPCLOSESTMATCHISNULL:
    MessageId = MessageId(70);
/// The header's `DXGI_MSG_IDXGIOutput_FindClosestMatchingMode_ModeHasRefreshRateDenominatorZero`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_FINDCLOSESTMATCHINGMODE_MODEHASREFRESHRATEDENOMINATORZERO:
    MessageId = MessageId(71);
/// The header's `DXGI_MSG_IDXGIOutput_FindClosestMatchingMode_UnknownFormatIsInvalidForConfiguration`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_FINDCLOSESTMATCHINGMODE_UNKNOWNFORMATISINVALIDFORCONFIGURATION:
    MessageId = MessageId(72);
/// The header's `DXGI_MSG_IDXGIOutput_FindClosestMatchingMode_InvalidDisplayModeScanlineOrdering`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_FINDCLOSESTMATCHINGMODE_INVALIDDISPLAYMODESCANLINEORDERING:
    MessageId = MessageId(73);
/// The header's `DXGI_MSG_IDXGIOutput_FindClosestMatchingMode_InvalidDisplayModeScaling`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_FINDCLOSESTMATCHINGMODE_INVALIDDISPLAYMODESCALING: MessageId =
    MessageId(74);
/// The header's `DXGI_MSG_IDXGIOutput_FindClosestMatchingMode_InvalidDisplayModeFormatAndDeviceCombination`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_FINDCLOSESTMATCHINGMODE_INVALIDDISPLAYMODEFORMATANDDEVICECOMBINATION: MessageId = MessageId(75);
/// The header's `DXGI_MSG_IDXGIFactory_Creation_CalledFromDllMain`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATION_CALLEDFROMDLLMAIN: MessageId = MessageId(76);
/// The header's `DXGI_MSG_IDXGISwapChain_SetFullscreenState_OutputNotOwnedBySwapChainDevice`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_SETFULLSCREENSTATE_OUTPUTNOTOWNEDBYSWAPCHAINDEVICE: MessageId =
    MessageId(77);
/// The header's `DXGI_MSG_IDXGISwapChain_Creation_InvalidWindowStyle`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_CREATION_INVALIDWINDOWSTYLE: MessageId = MessageId(78);
/// The header's `DXGI_MSG_IDXGISwapChain_GetFrameStatistics_UnsupportedStatistics`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_GETFRAMESTATISTICS_UNSUPPORTEDSTATISTICS: MessageId =
    MessageId(79);
/// The header's `DXGI_MSG_IDXGISwapChain_GetContainingOutput_SwapchainAdapterDoesNotControlOutput`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_GETCONTAININGOUTPUT_SWAPCHAINADAPTERDOESNOTCONTROLOUTPUT:
    MessageId = MessageId(80);
/// The header's `DXGI_MSG_IDXGIOutput_SetOrGetGammaControl_pArrayIsNULL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_SETORGETGAMMACONTROL_PARRAYISNULL: MessageId = MessageId(81);
/// The header's `DXGI_MSG_IDXGISwapChain_SetFullscreenState_FullscreenInvalidForChildWindows`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_SETFULLSCREENSTATE_FULLSCREENINVALIDFORCHILDWINDOWS: MessageId =
    MessageId(82);
/// The header's `DXGI_MSG_IDXGIFactory_Release_CalledFromDllMain`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_RELEASE_CALLEDFROMDLLMAIN: MessageId = MessageId(83);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_UnreleasedHDC`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_UNRELEASEDHDC: MessageId = MessageId(84);
/// The header's `DXGI_MSG_IDXGISwapChain_ResizeBuffers_NonPreRotatedAndGDICompatibleFlags`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RESIZEBUFFERS_NONPREROTATEDANDGDICOMPATIBLEFLAGS: MessageId =
    MessageId(85);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_NonPreRotatedAndGDICompatibleFlags`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_NONPREROTATEDANDGDICOMPATIBLEFLAGS: MessageId =
    MessageId(86);
/// The header's `DXGI_MSG_IDXGISurface1_GetDC_pHdcIsNULL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISURFACE1_GETDC_PHDCISNULL: MessageId = MessageId(87);
/// The header's `DXGI_MSG_IDXGISurface1_GetDC_SurfaceNotTexture2D`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISURFACE1_GETDC_SURFACENOTTEXTURE2D: MessageId = MessageId(88);
/// The header's `DXGI_MSG_IDXGISurface1_GetDC_GDICompatibleFlagNotSet`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISURFACE1_GETDC_GDICOMPATIBLEFLAGNOTSET: MessageId = MessageId(89);
/// The header's `DXGI_MSG_IDXGISurface1_GetDC_UnreleasedHDC`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISURFACE1_GETDC_UNRELEASEDHDC: MessageId = MessageId(90);
/// The header's `DXGI_MSG_IDXGISurface_Map_NoCPUAccess2`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISURFACE_MAP_NOCPUACCESS2: MessageId = MessageId(91);
/// The header's `DXGI_MSG_IDXGISurface1_ReleaseDC_GetDCNotCalled`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISURFACE1_RELEASEDC_GETDCNOTCALLED: MessageId = MessageId(92);
/// The header's `DXGI_MSG_IDXGISurface1_ReleaseDC_InvalidRectangleDimensions`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISURFACE1_RELEASEDC_INVALIDRECTANGLEDIMENSIONS: MessageId = MessageId(93);
/// The header's `DXGI_MSG_IDXGIOutput_TakeOwnership_RemoteOutputNotSupported`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_TAKEOWNERSHIP_REMOTEOUTPUTNOTSUPPORTED: MessageId = MessageId(94);
/// The header's `DXGI_MSG_IDXGIOutput_FindClosestMatchingMode_RemoteOutputNotSupported`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_FINDCLOSESTMATCHINGMODE_REMOTEOUTPUTNOTSUPPORTED: MessageId =
    MessageId(95);
/// The header's `DXGI_MSG_IDXGIOutput_GetDisplayModeList_RemoteOutputNotSupported`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_GETDISPLAYMODELIST_REMOTEOUTPUTNOTSUPPORTED: MessageId =
    MessageId(96);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_pDeviceHasMismatchedDXGIFactory`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_PDEVICEHASMISMATCHEDDXGIFACTORY: MessageId =
    MessageId(97);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_NonOptimalFSConfiguration`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_NONOPTIMALFSCONFIGURATION: MessageId = MessageId(98);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_FlipSequentialNotSupportedOnD3D10`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_FLIPSEQUENTIALNOTSUPPORTEDOND3D10: MessageId =
    MessageId(99);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_BufferCountOOBForFlipSequential`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_BUFFERCOUNTOOBFORFLIPSEQUENTIAL: MessageId =
    MessageId(100);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_InvalidFormatForFlipSequential`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_INVALIDFORMATFORFLIPSEQUENTIAL: MessageId =
    MessageId(101);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_MultiSamplingNotSupportedForFlipSequential`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_MULTISAMPLINGNOTSUPPORTEDFORFLIPSEQUENTIAL:
    MessageId = MessageId(102);
/// The header's `DXGI_MSG_IDXGISwapChain_ResizeBuffers_BufferCountOOBForFlipSequential`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RESIZEBUFFERS_BUFFERCOUNTOOBFORFLIPSEQUENTIAL: MessageId =
    MessageId(103);
/// The header's `DXGI_MSG_IDXGISwapChain_ResizeBuffers_InvalidFormatForFlipSequential`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RESIZEBUFFERS_INVALIDFORMATFORFLIPSEQUENTIAL: MessageId =
    MessageId(104);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_PartialPresentationBeforeStandardPresentation`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_PARTIALPRESENTATIONBEFORESTANDARDPRESENTATION: MessageId =
    MessageId(105);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_FullscreenPartialPresentIsInvalid`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_FULLSCREENPARTIALPRESENTISINVALID: MessageId =
    MessageId(106);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_InvalidPresentTestOrDoNotSequenceFlag`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_INVALIDPRESENTTESTORDONOTSEQUENCEFLAG: MessageId =
    MessageId(107);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_ScrollInfoWithNoDirtyRectsSpecified`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_SCROLLINFOWITHNODIRTYRECTSSPECIFIED: MessageId =
    MessageId(108);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_EmptyScrollRect`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_EMPTYSCROLLRECT: MessageId = MessageId(109);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_ScrollRectOutOfBackbufferBounds`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_SCROLLRECTOUTOFBACKBUFFERBOUNDS: MessageId =
    MessageId(110);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_ScrollRectOutOfBackbufferBoundsWithOffset`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_SCROLLRECTOUTOFBACKBUFFERBOUNDSWITHOFFSET: MessageId =
    MessageId(111);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_EmptyDirtyRect`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_EMPTYDIRTYRECT: MessageId = MessageId(112);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_DirtyRectOutOfBackbufferBounds`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_DIRTYRECTOUTOFBACKBUFFERBOUNDS: MessageId =
    MessageId(113);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_UnsupportedBufferUsageFlags`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_UNSUPPORTEDBUFFERUSAGEFLAGS: MessageId =
    MessageId(114);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_DoNotSequenceFlagSetButPreviousBufferIsUndefined`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_DONOTSEQUENCEFLAGSETBUTPREVIOUSBUFFERISUNDEFINED:
    MessageId = MessageId(115);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_UnsupportedFlags`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_UNSUPPORTEDFLAGS: MessageId = MessageId(116);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_FlipModelChainMustResizeOrCreateOnFSTransition`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_FLIPMODELCHAINMUSTRESIZEORCREATEONFSTRANSITION:
    MessageId = MessageId(117);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_pRestrictToOutputFromOtherIDXGIFactory`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_PRESTRICTTOOUTPUTFROMOTHERIDXGIFACTORY: MessageId =
    MessageId(118);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_RestrictOutputNotSupportedOnAdapter`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_RESTRICTOUTPUTNOTSUPPORTEDONADAPTER: MessageId =
    MessageId(119);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_RestrictToOutputFlagSetButInvalidpRestrictToOutput`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_RESTRICTTOOUTPUTFLAGSETBUTINVALIDPRESTRICTTOOUTPUT:
    MessageId = MessageId(120);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_RestrictToOutputFlagdWithFullscreen`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_RESTRICTTOOUTPUTFLAGDWITHFULLSCREEN: MessageId =
    MessageId(121);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_RestrictOutputFlagWithStaleSwapChain`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_RESTRICTOUTPUTFLAGWITHSTALESWAPCHAIN: MessageId =
    MessageId(122);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_OtherFlagsCausingInvalidPresentTestFlag`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_OTHERFLAGSCAUSINGINVALIDPRESENTTESTFLAG: MessageId =
    MessageId(123);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_UnavailableInSession0`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_UNAVAILABLEINSESSION0: MessageId = MessageId(124);
/// The header's `DXGI_MSG_IDXGIFactory_MakeWindowAssociation_UnavailableInSession0`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_MAKEWINDOWASSOCIATION_UNAVAILABLEINSESSION0: MessageId =
    MessageId(125);
/// The header's `DXGI_MSG_IDXGIFactory_GetWindowAssociation_UnavailableInSession0`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_GETWINDOWASSOCIATION_UNAVAILABLEINSESSION0: MessageId =
    MessageId(126);
/// The header's `DXGI_MSG_IDXGIAdapter_EnumOutputs_UnavailableInSession0`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIADAPTER_ENUMOUTPUTS_UNAVAILABLEINSESSION0: MessageId = MessageId(127);
/// The header's `DXGI_MSG_IDXGISwapChain_CreationOrSetFullscreenState_StereoDisabled`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_CREATIONORSETFULLSCREENSTATE_STEREODISABLED: MessageId =
    MessageId(128);
/// The header's `DXGI_MSG_IDXGIFactory2_UnregisterStatus_CookieNotFound`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY2_UNREGISTERSTATUS_COOKIENOTFOUND: MessageId = MessageId(129);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_ProtectedContentInWindowedModeWithoutFSOrOverlay`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_PROTECTEDCONTENTINWINDOWEDMODEWITHOUTFSOROVERLAY:
    MessageId = MessageId(130);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_ProtectedContentInWindowedModeWithoutFlipSequential`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_PROTECTEDCONTENTINWINDOWEDMODEWITHOUTFLIPSEQUENTIAL:
    MessageId = MessageId(131);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_ProtectedContentWithRDPDriver`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_PROTECTEDCONTENTWITHRDPDRIVER: MessageId = MessageId(132);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_ProtectedContentInWindowedModeWithDWMOffOrInvalidDisplayAffinity`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_PROTECTEDCONTENTINWINDOWEDMODEWITHDWMOFFORINVALIDDISPLAYAFFINITY: MessageId = MessageId(133);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChainForComposition_WidthOrHeightIsZero`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAINFORCOMPOSITION_WIDTHORHEIGHTISZERO: MessageId =
    MessageId(134);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChainForComposition_OnlyFlipSequentialSupported`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAINFORCOMPOSITION_ONLYFLIPSEQUENTIALSUPPORTED:
    MessageId = MessageId(135);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChainForComposition_UnsupportedOnAdapter`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAINFORCOMPOSITION_UNSUPPORTEDONADAPTER: MessageId =
    MessageId(136);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChainForComposition_UnsupportedOnWindows7`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAINFORCOMPOSITION_UNSUPPORTEDONWINDOWS7: MessageId =
    MessageId(137);
/// The header's `DXGI_MSG_IDXGISwapChain_SetFullscreenState_FSTransitionWithCompositionSwapChain`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_SETFULLSCREENSTATE_FSTRANSITIONWITHCOMPOSITIONSWAPCHAIN:
    MessageId = MessageId(138);
/// The header's `DXGI_MSG_IDXGISwapChain_ResizeTarget_InvalidWithCompositionSwapChain`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RESIZETARGET_INVALIDWITHCOMPOSITIONSWAPCHAIN: MessageId =
    MessageId(139);
/// The header's `DXGI_MSG_IDXGISwapChain_ResizeBuffers_WidthOrHeightIsZero`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RESIZEBUFFERS_WIDTHORHEIGHTISZERO: MessageId = MessageId(140);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_ScalingNoneIsFlipModelOnly`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_SCALINGNONEISFLIPMODELONLY: MessageId =
    MessageId(141);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_ScalingUnrecognized`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_SCALINGUNRECOGNIZED: MessageId = MessageId(142);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_DisplayOnlyFullscreenUnsupported`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_DISPLAYONLYFULLSCREENUNSUPPORTED: MessageId =
    MessageId(143);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_DisplayOnlyUnsupported`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_DISPLAYONLYUNSUPPORTED: MessageId = MessageId(144);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_RestartIsFullscreenOnly`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_RESTARTISFULLSCREENONLY: MessageId = MessageId(145);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_ProtectedWindowlessPresentationRequiresDisplayOnly`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_PROTECTEDWINDOWLESSPRESENTATIONREQUIRESDISPLAYONLY:
    MessageId = MessageId(146);
/// The header's `DXGI_MSG_IDXGISwapChain_SetFullscreenState_DisplayOnlyUnsupported`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_SETFULLSCREENSTATE_DISPLAYONLYUNSUPPORTED: MessageId =
    MessageId(147);
/// The header's `DXGI_MSG_IDXGISwapChain1_SetBackgroundColor_OutOfRange`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN1_SETBACKGROUNDCOLOR_OUTOFRANGE: MessageId = MessageId(148);
/// The header's `DXGI_MSG_IDXGISwapChain_ResizeBuffers_DisplayOnlyFullscreenUnsupported`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RESIZEBUFFERS_DISPLAYONLYFULLSCREENUNSUPPORTED: MessageId =
    MessageId(149);
/// The header's `DXGI_MSG_IDXGISwapChain_ResizeBuffers_DisplayOnlyUnsupported`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RESIZEBUFFERS_DISPLAYONLYUNSUPPORTED: MessageId = MessageId(150);
/// The header's `DXGI_MSG_IDXGISwapchain_Present_ScrollUnsupported`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_SCROLLUNSUPPORTED: MessageId = MessageId(151);
/// The header's `DXGI_MSG_IDXGISwapChain1_SetRotation_UnsupportedOS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN1_SETROTATION_UNSUPPORTEDOS: MessageId = MessageId(152);
/// The header's `DXGI_MSG_IDXGISwapChain1_GetRotation_UnsupportedOS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN1_GETROTATION_UNSUPPORTEDOS: MessageId = MessageId(153);
/// The header's `DXGI_MSG_IDXGISwapchain_Present_FullscreenRotation`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_FULLSCREENROTATION: MessageId = MessageId(154);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_PartialPresentationWithMSAABuffers`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_PARTIALPRESENTATIONWITHMSAABUFFERS: MessageId =
    MessageId(155);
/// The header's `DXGI_MSG_IDXGISwapChain1_SetRotation_FlipSequentialRequired`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN1_SETROTATION_FLIPSEQUENTIALREQUIRED: MessageId = MessageId(156);
/// The header's `DXGI_MSG_IDXGISwapChain1_SetRotation_InvalidRotation`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN1_SETROTATION_INVALIDROTATION: MessageId = MessageId(157);
/// The header's `DXGI_MSG_IDXGISwapChain1_GetRotation_FlipSequentialRequired`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN1_GETROTATION_FLIPSEQUENTIALREQUIRED: MessageId = MessageId(158);
/// The header's `DXGI_MSG_IDXGISwapChain_GetHwnd_WrongType`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_GETHWND_WRONGTYPE: MessageId = MessageId(159);
/// The header's `DXGI_MSG_IDXGISwapChain_GetCompositionSurface_WrongType`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_GETCOMPOSITIONSURFACE_WRONGTYPE: MessageId = MessageId(160);
/// The header's `DXGI_MSG_IDXGISwapChain_GetCoreWindow_WrongType`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_GETCOREWINDOW_WRONGTYPE: MessageId = MessageId(161);
/// The header's `DXGI_MSG_IDXGISwapChain_GetFullscreenDesc_NonHwnd`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_GETFULLSCREENDESC_NONHWND: MessageId = MessageId(162);
/// The header's `DXGI_MSG_IDXGISwapChain_SetFullscreenState_CoreWindow`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_SETFULLSCREENSTATE_COREWINDOW: MessageId = MessageId(163);
/// The header's `DXGI_MSG_IDXGIFactory2_CreateSwapChainForCoreWindow_UnsupportedOnWindows7`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY2_CREATESWAPCHAINFORCOREWINDOW_UNSUPPORTEDONWINDOWS7: MessageId =
    MessageId(164);
/// The header's `DXGI_MSG_IDXGIFactory2_CreateSwapChainForCoreWindow_pWindowIsNULL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY2_CREATESWAPCHAINFORCOREWINDOW_PWINDOWISNULL: MessageId =
    MessageId(165);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_FSUnsupportedForModernApps`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_FSUNSUPPORTEDFORMODERNAPPS: MessageId =
    MessageId(166);
/// The header's `DXGI_MSG_IDXGIFactory_MakeWindowAssociation_ModernApp`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_MAKEWINDOWASSOCIATION_MODERNAPP: MessageId = MessageId(167);
/// The header's `DXGI_MSG_IDXGISwapChain_ResizeTarget_ModernApp`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RESIZETARGET_MODERNAPP: MessageId = MessageId(168);
/// The header's `DXGI_MSG_IDXGISwapChain_ResizeTarget_pNewTargetParametersIsNULL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RESIZETARGET_PNEWTARGETPARAMETERSISNULL: MessageId =
    MessageId(169);
/// The header's `DXGI_MSG_IDXGIOutput_SetDisplaySurface_ModernApp`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_SETDISPLAYSURFACE_MODERNAPP: MessageId = MessageId(170);
/// The header's `DXGI_MSG_IDXGIOutput_TakeOwnership_ModernApp`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_TAKEOWNERSHIP_MODERNAPP: MessageId = MessageId(171);
/// The header's `DXGI_MSG_IDXGIFactory2_CreateSwapChainForCoreWindow_pWindowIsInvalid`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY2_CREATESWAPCHAINFORCOREWINDOW_PWINDOWISINVALID: MessageId =
    MessageId(172);
/// The header's `DXGI_MSG_IDXGIFactory2_CreateSwapChainForCompositionSurface_InvalidHandle`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY2_CREATESWAPCHAINFORCOMPOSITIONSURFACE_INVALIDHANDLE: MessageId =
    MessageId(173);
/// The header's `DXGI_MSG_IDXGISurface1_GetDC_ModernApp`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISURFACE1_GETDC_MODERNAPP: MessageId = MessageId(174);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_ScalingNoneRequiresWindows8OrNewer`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_SCALINGNONEREQUIRESWINDOWS8ORNEWER: MessageId =
    MessageId(175);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_TemporaryMonoAndPreferRight`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_TEMPORARYMONOANDPREFERRIGHT: MessageId = MessageId(176);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_TemporaryMonoOrPreferRightWithDoNotSequence`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_TEMPORARYMONOORPREFERRIGHTWITHDONOTSEQUENCE: MessageId =
    MessageId(177);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_TemporaryMonoOrPreferRightWithoutStereo`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_TEMPORARYMONOORPREFERRIGHTWITHOUTSTEREO: MessageId =
    MessageId(178);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_TemporaryMonoUnsupported`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_TEMPORARYMONOUNSUPPORTED: MessageId = MessageId(179);
/// The header's `DXGI_MSG_IDXGIOutput_GetDisplaySurfaceData_ArraySizeMismatch`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_GETDISPLAYSURFACEDATA_ARRAYSIZEMISMATCH: MessageId = MessageId(180);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_PartialPresentationWithSwapEffectDiscard`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_PARTIALPRESENTATIONWITHSWAPEFFECTDISCARD: MessageId =
    MessageId(181);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_AlphaUnrecognized`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_ALPHAUNRECOGNIZED: MessageId = MessageId(182);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_AlphaIsWindowlessOnly`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_ALPHAISWINDOWLESSONLY: MessageId = MessageId(183);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_AlphaIsFlipModelOnly`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_ALPHAISFLIPMODELONLY: MessageId = MessageId(184);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_RestrictToOutputAdapterMismatch`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_RESTRICTTOOUTPUTADAPTERMISMATCH: MessageId =
    MessageId(185);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_DisplayOnlyOnLegacy`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_DISPLAYONLYONLEGACY: MessageId = MessageId(186);
/// The header's `DXGI_MSG_IDXGISwapChain_ResizeBuffers_DisplayOnlyOnLegacy`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RESIZEBUFFERS_DISPLAYONLYONLEGACY: MessageId = MessageId(187);
/// The header's `DXGI_MSG_IDXGIResource1_CreateSubresourceSurface_InvalidIndex`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIRESOURCE1_CREATESUBRESOURCESURFACE_INVALIDINDEX: MessageId = MessageId(188);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChainForComposition_InvalidScaling`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAINFORCOMPOSITION_INVALIDSCALING: MessageId =
    MessageId(189);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChainForCoreWindow_InvalidSwapEffect`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAINFORCOREWINDOW_INVALIDSWAPEFFECT: MessageId =
    MessageId(190);
/// The header's `DXGI_MSG_IDXGIResource1_CreateSharedHandle_UnsupportedOS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIRESOURCE1_CREATESHAREDHANDLE_UNSUPPORTEDOS: MessageId = MessageId(191);
/// The header's `DXGI_MSG_IDXGIFactory2_RegisterOcclusionStatusWindow_UnsupportedOS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY2_REGISTEROCCLUSIONSTATUSWINDOW_UNSUPPORTEDOS: MessageId =
    MessageId(192);
/// The header's `DXGI_MSG_IDXGIFactory2_RegisterOcclusionStatusEvent_UnsupportedOS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY2_REGISTEROCCLUSIONSTATUSEVENT_UNSUPPORTEDOS: MessageId =
    MessageId(193);
/// The header's `DXGI_MSG_IDXGIOutput1_DuplicateOutput_UnsupportedOS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT1_DUPLICATEOUTPUT_UNSUPPORTEDOS: MessageId = MessageId(194);
/// The header's `DXGI_MSG_IDXGIDisplayControl_IsStereoEnabled_UnsupportedOS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIDISPLAYCONTROL_ISSTEREOENABLED_UNSUPPORTEDOS: MessageId = MessageId(195);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChainForComposition_InvalidAlphaMode`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAINFORCOMPOSITION_INVALIDALPHAMODE: MessageId =
    MessageId(196);
/// The header's `DXGI_MSG_IDXGIFactory_GetSharedResourceAdapterLuid_InvalidResource`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_GETSHAREDRESOURCEADAPTERLUID_INVALIDRESOURCE: MessageId =
    MessageId(197);
/// The header's `DXGI_MSG_IDXGIFactory_GetSharedResourceAdapterLuid_InvalidLUID`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_GETSHAREDRESOURCEADAPTERLUID_INVALIDLUID: MessageId =
    MessageId(198);
/// The header's `DXGI_MSG_IDXGIFactory_GetSharedResourceAdapterLuid_UnsupportedOS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_GETSHAREDRESOURCEADAPTERLUID_UNSUPPORTEDOS: MessageId =
    MessageId(199);
/// The header's `DXGI_MSG_IDXGIOutput1_GetDisplaySurfaceData1_2DOnly`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT1_GETDISPLAYSURFACEDATA1_2DONLY: MessageId = MessageId(200);
/// The header's `DXGI_MSG_IDXGIOutput1_GetDisplaySurfaceData1_StagingOnly`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT1_GETDISPLAYSURFACEDATA1_STAGINGONLY: MessageId = MessageId(201);
/// The header's `DXGI_MSG_IDXGIOutput1_GetDisplaySurfaceData1_NeedCPUAccessWrite`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT1_GETDISPLAYSURFACEDATA1_NEEDCPUACCESSWRITE: MessageId =
    MessageId(202);
/// The header's `DXGI_MSG_IDXGIOutput1_GetDisplaySurfaceData1_NoShared`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT1_GETDISPLAYSURFACEDATA1_NOSHARED: MessageId = MessageId(203);
/// The header's `DXGI_MSG_IDXGIOutput1_GetDisplaySurfaceData1_OnlyMipLevels1`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT1_GETDISPLAYSURFACEDATA1_ONLYMIPLEVELS1: MessageId = MessageId(204);
/// The header's `DXGI_MSG_IDXGIOutput1_GetDisplaySurfaceData1_MappedOrOfferedResource`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT1_GETDISPLAYSURFACEDATA1_MAPPEDOROFFEREDRESOURCE: MessageId =
    MessageId(205);
/// The header's `DXGI_MSG_IDXGISwapChain_SetFullscreenState_FSUnsupportedForModernApps`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_SETFULLSCREENSTATE_FSUNSUPPORTEDFORMODERNAPPS: MessageId =
    MessageId(206);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_FailedToGoFSButNonPreRotated`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_FAILEDTOGOFSBUTNONPREROTATED: MessageId =
    MessageId(207);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChainOrRegisterOcclusionStatus_BlitModelUsedWhileRegisteredForOcclusionStatusEvents`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAINORREGISTEROCCLUSIONSTATUS_BLITMODELUSEDWHILEREGISTEREDFOROCCLUSIONSTATUSEVENTS: MessageId = MessageId(208);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_BlitModelUsedWhileRegisteredForOcclusionStatusEvents`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_BLITMODELUSEDWHILEREGISTEREDFOROCCLUSIONSTATUSEVENTS:
    MessageId = MessageId(209);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_WaitableSwapChainsAreFlipModelOnly`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_WAITABLESWAPCHAINSAREFLIPMODELONLY: MessageId =
    MessageId(210);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_WaitableSwapChainsAreNotFullscreen`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_WAITABLESWAPCHAINSARENOTFULLSCREEN: MessageId =
    MessageId(211);
/// The header's `DXGI_MSG_IDXGISwapChain_SetFullscreenState_Waitable`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_SETFULLSCREENSTATE_WAITABLE: MessageId = MessageId(212);
/// The header's `DXGI_MSG_IDXGISwapChain_ResizeBuffers_CannotAddOrRemoveWaitableFlag`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RESIZEBUFFERS_CANNOTADDORREMOVEWAITABLEFLAG: MessageId =
    MessageId(213);
/// The header's `DXGI_MSG_IDXGISwapChain_GetFrameLatencyWaitableObject_OnlyWaitable`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_GETFRAMELATENCYWAITABLEOBJECT_ONLYWAITABLE: MessageId =
    MessageId(214);
/// The header's `DXGI_MSG_IDXGISwapChain_GetMaximumFrameLatency_OnlyWaitable`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_GETMAXIMUMFRAMELATENCY_ONLYWAITABLE: MessageId = MessageId(215);
/// The header's `DXGI_MSG_IDXGISwapChain_GetMaximumFrameLatency_pMaxLatencyIsNULL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_GETMAXIMUMFRAMELATENCY_PMAXLATENCYISNULL: MessageId =
    MessageId(216);
/// The header's `DXGI_MSG_IDXGISwapChain_SetMaximumFrameLatency_OnlyWaitable`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_SETMAXIMUMFRAMELATENCY_ONLYWAITABLE: MessageId = MessageId(217);
/// The header's `DXGI_MSG_IDXGISwapChain_SetMaximumFrameLatency_MaxLatencyIsOutOfBounds`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_SETMAXIMUMFRAMELATENCY_MAXLATENCYISOUTOFBOUNDS: MessageId =
    MessageId(218);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_ForegroundIsCoreWindowOnly`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_FOREGROUNDISCOREWINDOWONLY: MessageId =
    MessageId(219);
/// The header's `DXGI_MSG_IDXGIFactory2_CreateSwapChainForCoreWindow_ForegroundUnsupportedOnAdapter`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY2_CREATESWAPCHAINFORCOREWINDOW_FOREGROUNDUNSUPPORTEDONADAPTER:
    MessageId = MessageId(220);
/// The header's `DXGI_MSG_IDXGIFactory2_CreateSwapChainForCoreWindow_InvalidScaling`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY2_CREATESWAPCHAINFORCOREWINDOW_INVALIDSCALING: MessageId =
    MessageId(221);
/// The header's `DXGI_MSG_IDXGIFactory2_CreateSwapChainForCoreWindow_InvalidAlphaMode`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY2_CREATESWAPCHAINFORCOREWINDOW_INVALIDALPHAMODE: MessageId =
    MessageId(222);
/// The header's `DXGI_MSG_IDXGISwapChain_ResizeBuffers_CannotAddOrRemoveForegroundFlag`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RESIZEBUFFERS_CANNOTADDORREMOVEFOREGROUNDFLAG: MessageId =
    MessageId(223);
/// The header's `DXGI_MSG_IDXGISwapChain_SetMatrixTransform_MatrixPointerCannotBeNull`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_SETMATRIXTRANSFORM_MATRIXPOINTERCANNOTBENULL: MessageId =
    MessageId(224);
/// The header's `DXGI_MSG_IDXGISwapChain_SetMatrixTransform_RequiresCompositionSwapChain`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_SETMATRIXTRANSFORM_REQUIRESCOMPOSITIONSWAPCHAIN: MessageId =
    MessageId(225);
/// The header's `DXGI_MSG_IDXGISwapChain_SetMatrixTransform_MatrixMustBeFinite`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_SETMATRIXTRANSFORM_MATRIXMUSTBEFINITE: MessageId = MessageId(226);
/// The header's `DXGI_MSG_IDXGISwapChain_SetMatrixTransform_MatrixMustBeTranslateAndOrScale`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_SETMATRIXTRANSFORM_MATRIXMUSTBETRANSLATEANDORSCALE: MessageId =
    MessageId(227);
/// The header's `DXGI_MSG_IDXGISwapChain_GetMatrixTransform_MatrixPointerCannotBeNull`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_GETMATRIXTRANSFORM_MATRIXPOINTERCANNOTBENULL: MessageId =
    MessageId(228);
/// The header's `DXGI_MSG_IDXGISwapChain_GetMatrixTransform_RequiresCompositionSwapChain`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_GETMATRIXTRANSFORM_REQUIRESCOMPOSITIONSWAPCHAIN: MessageId =
    MessageId(229);
/// The header's `DXGI_MSG_DXGIGetDebugInterface1_NULL_ppDebug`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_DXGIGETDEBUGINTERFACE1_NULL_PPDEBUG: MessageId = MessageId(230);
/// The header's `DXGI_MSG_DXGIGetDebugInterface1_InvalidFlags`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_DXGIGETDEBUGINTERFACE1_INVALIDFLAGS: MessageId = MessageId(231);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_Decode`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_DECODE: MessageId = MessageId(232);
/// The header's `DXGI_MSG_IDXGISwapChain_ResizeBuffers_Decode`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RESIZEBUFFERS_DECODE: MessageId = MessageId(233);
/// The header's `DXGI_MSG_IDXGISwapChain_SetSourceSize_FlipModel`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_SETSOURCESIZE_FLIPMODEL: MessageId = MessageId(234);
/// The header's `DXGI_MSG_IDXGISwapChain_SetSourceSize_Decode`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_SETSOURCESIZE_DECODE: MessageId = MessageId(235);
/// The header's `DXGI_MSG_IDXGISwapChain_SetSourceSize_WidthHeight`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_SETSOURCESIZE_WIDTHHEIGHT: MessageId = MessageId(236);
/// The header's `DXGI_MSG_IDXGISwapChain_GetSourceSize_NullPointers`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_GETSOURCESIZE_NULLPOINTERS: MessageId = MessageId(237);
/// The header's `DXGI_MSG_IDXGISwapChain_GetSourceSize_Decode`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_GETSOURCESIZE_DECODE: MessageId = MessageId(238);
/// The header's `DXGI_MSG_IDXGIDecodeSwapChain_SetColorSpace_InvalidFlags`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIDECODESWAPCHAIN_SETCOLORSPACE_INVALIDFLAGS: MessageId = MessageId(239);
/// The header's `DXGI_MSG_IDXGIDecodeSwapChain_SetSourceRect_InvalidRect`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIDECODESWAPCHAIN_SETSOURCERECT_INVALIDRECT: MessageId = MessageId(240);
/// The header's `DXGI_MSG_IDXGIDecodeSwapChain_SetTargetRect_InvalidRect`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIDECODESWAPCHAIN_SETTARGETRECT_INVALIDRECT: MessageId = MessageId(241);
/// The header's `DXGI_MSG_IDXGIDecodeSwapChain_SetDestSize_InvalidSize`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIDECODESWAPCHAIN_SETDESTSIZE_INVALIDSIZE: MessageId = MessageId(242);
/// The header's `DXGI_MSG_IDXGIDecodeSwapChain_GetSourceRect_InvalidPointer`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIDECODESWAPCHAIN_GETSOURCERECT_INVALIDPOINTER: MessageId = MessageId(243);
/// The header's `DXGI_MSG_IDXGIDecodeSwapChain_GetTargetRect_InvalidPointer`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIDECODESWAPCHAIN_GETTARGETRECT_INVALIDPOINTER: MessageId = MessageId(244);
/// The header's `DXGI_MSG_IDXGIDecodeSwapChain_GetDestSize_InvalidPointer`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIDECODESWAPCHAIN_GETDESTSIZE_INVALIDPOINTER: MessageId = MessageId(245);
/// The header's `DXGI_MSG_IDXGISwapChain_PresentBuffer_YUV`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENTBUFFER_YUV: MessageId = MessageId(246);
/// The header's `DXGI_MSG_IDXGISwapChain_SetSourceSize_YUV`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_SETSOURCESIZE_YUV: MessageId = MessageId(247);
/// The header's `DXGI_MSG_IDXGISwapChain_GetSourceSize_YUV`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_GETSOURCESIZE_YUV: MessageId = MessageId(248);
/// The header's `DXGI_MSG_IDXGISwapChain_SetMatrixTransform_YUV`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_SETMATRIXTRANSFORM_YUV: MessageId = MessageId(249);
/// The header's `DXGI_MSG_IDXGISwapChain_GetMatrixTransform_YUV`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_GETMATRIXTRANSFORM_YUV: MessageId = MessageId(250);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_PartialPresentation_YUV`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_PARTIALPRESENTATION_YUV: MessageId = MessageId(251);
/// The header's `DXGI_MSG_IDXGISwapChain_ResizeBuffers_CannotAddOrRemoveFlag_YUV`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RESIZEBUFFERS_CANNOTADDORREMOVEFLAG_YUV: MessageId =
    MessageId(252);
/// The header's `DXGI_MSG_IDXGISwapChain_ResizeBuffers_Alignment_YUV`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RESIZEBUFFERS_ALIGNMENT_YUV: MessageId = MessageId(253);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_ShaderInputUnsupported_YUV`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_SHADERINPUTUNSUPPORTED_YUV: MessageId =
    MessageId(254);
/// The header's `DXGI_MSG_IDXGIOutput3_CheckOverlaySupport_NullPointers`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT3_CHECKOVERLAYSUPPORT_NULLPOINTERS: MessageId = MessageId(255);
/// The header's `DXGI_MSG_IDXGIOutput3_CheckOverlaySupport_IDXGIDeviceNotSupportedBypConcernedDevice`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT3_CHECKOVERLAYSUPPORT_IDXGIDEVICENOTSUPPORTEDBYPCONCERNEDDEVICE:
    MessageId = MessageId(0x100);
/// The header's `DXGI_MSG_IDXGIAdapter_EnumOutputs2_InvalidEnumOutputs2Flag`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIADAPTER_ENUMOUTPUTS2_INVALIDENUMOUTPUTS2FLAG: MessageId = MessageId(0x101);
/// The header's `DXGI_MSG_IDXGISwapChain_CreationOrSetFullscreenState_FSUnsupportedForFlipDiscard`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_CREATIONORSETFULLSCREENSTATE_FSUNSUPPORTEDFORFLIPDISCARD:
    MessageId = MessageId(0x102);
/// The header's `DXGI_MSG_IDXGIOutput4_CheckOverlayColorSpaceSupport_NullPointers`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT4_CHECKOVERLAYCOLORSPACESUPPORT_NULLPOINTERS: MessageId =
    MessageId(0x103);
/// The header's `DXGI_MSG_IDXGIOutput4_CheckOverlayColorSpaceSupport_IDXGIDeviceNotSupportedBypConcernedDevice`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT4_CHECKOVERLAYCOLORSPACESUPPORT_IDXGIDEVICENOTSUPPORTEDBYPCONCERNEDDEVICE: MessageId = MessageId(0x104);
/// The header's `DXGI_MSG_IDXGISwapChain3_CheckColorSpaceSupport_NullPointers`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN3_CHECKCOLORSPACESUPPORT_NULLPOINTERS: MessageId =
    MessageId(0x105);
/// The header's `DXGI_MSG_IDXGISwapChain3_SetColorSpace1_InvalidColorSpace`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN3_SETCOLORSPACE1_INVALIDCOLORSPACE: MessageId = MessageId(0x106);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_InvalidHwProtect`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_INVALIDHWPROTECT: MessageId = MessageId(0x107);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_HwProtectUnsupported`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_HWPROTECTUNSUPPORTED: MessageId = MessageId(0x108);
/// The header's `DXGI_MSG_IDXGISwapChain_ResizeBuffers_InvalidHwProtect`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RESIZEBUFFERS_INVALIDHWPROTECT: MessageId = MessageId(0x109);
/// The header's `DXGI_MSG_IDXGISwapChain_ResizeBuffers_HwProtectUnsupported`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RESIZEBUFFERS_HWPROTECTUNSUPPORTED: MessageId = MessageId(0x10a);
/// The header's `DXGI_MSG_IDXGISwapChain_ResizeBuffers1_D3D12Only`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RESIZEBUFFERS1_D3D12ONLY: MessageId = MessageId(0x10b);
/// The header's `DXGI_MSG_IDXGISwapChain_ResizeBuffers1_FlipModel`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RESIZEBUFFERS1_FLIPMODEL: MessageId = MessageId(0x10c);
/// The header's `DXGI_MSG_IDXGISwapChain_ResizeBuffers1_NodeMaskAndQueueRequired`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RESIZEBUFFERS1_NODEMASKANDQUEUEREQUIRED: MessageId =
    MessageId(0x10d);
/// The header's `DXGI_MSG_IDXGISwapChain_CreateSwapChain_InvalidHwProtectGdiFlag`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_CREATESWAPCHAIN_INVALIDHWPROTECTGDIFLAG: MessageId =
    MessageId(0x10e);
/// The header's `DXGI_MSG_IDXGISwapChain_ResizeBuffers_InvalidHwProtectGdiFlag`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RESIZEBUFFERS_INVALIDHWPROTECTGDIFLAG: MessageId =
    MessageId(0x10f);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_10BitFormatNotSupported`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_10BITFORMATNOTSUPPORTED: MessageId =
    MessageId(0x110);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_FlipSwapEffectRequired`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_FLIPSWAPEFFECTREQUIRED: MessageId =
    MessageId(0x111);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_InvalidDevice`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_INVALIDDEVICE: MessageId = MessageId(0x112);
/// The header's `DXGI_MSG_IDXGIOutput_TakeOwnership_Unsupported`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_TAKEOWNERSHIP_UNSUPPORTED: MessageId = MessageId(0x113);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_InvalidQueue`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_INVALIDQUEUE: MessageId = MessageId(0x114);
/// The header's `DXGI_MSG_IDXGISwapChain3_ResizeBuffers1_InvalidQueue`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN3_RESIZEBUFFERS1_INVALIDQUEUE: MessageId = MessageId(0x115);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChainForHwnd_InvalidScaling`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAINFORHWND_INVALIDSCALING: MessageId = MessageId(0x116);
/// The header's `DXGI_MSG_IDXGISwapChain3_SetHDRMetaData_InvalidSize`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN3_SETHDRMETADATA_INVALIDSIZE: MessageId = MessageId(0x117);
/// The header's `DXGI_MSG_IDXGISwapChain3_SetHDRMetaData_InvalidPointer`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN3_SETHDRMETADATA_INVALIDPOINTER: MessageId = MessageId(0x118);
/// The header's `DXGI_MSG_IDXGISwapChain3_SetHDRMetaData_InvalidType`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN3_SETHDRMETADATA_INVALIDTYPE: MessageId = MessageId(0x119);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_FullscreenAllowTearingIsInvalid`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_FULLSCREENALLOWTEARINGISINVALID: MessageId =
    MessageId(0x11a);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_AllowTearingRequiresPresentIntervalZero`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_ALLOWTEARINGREQUIRESPRESENTINTERVALZERO: MessageId =
    MessageId(0x11b);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_AllowTearingRequiresCreationFlag`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_ALLOWTEARINGREQUIRESCREATIONFLAG: MessageId =
    MessageId(0x11c);
/// The header's `DXGI_MSG_IDXGISwapChain_ResizeBuffers_CannotAddOrRemoveAllowTearingFlag`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_RESIZEBUFFERS_CANNOTADDORREMOVEALLOWTEARINGFLAG: MessageId =
    MessageId(0x11d);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_AllowTearingFlagIsFlipModelOnly`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_ALLOWTEARINGFLAGISFLIPMODELONLY: MessageId =
    MessageId(0x11e);
/// The header's `DXGI_MSG_IDXGIFactory_CheckFeatureSupport_InvalidFeature`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CHECKFEATURESUPPORT_INVALIDFEATURE: MessageId = MessageId(0x11f);
/// The header's `DXGI_MSG_IDXGIFactory_CheckFeatureSupport_InvalidSize`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CHECKFEATURESUPPORT_INVALIDSIZE: MessageId = MessageId(0x120);
/// The header's `DXGI_MSG_IDXGIOutput6_CheckHardwareCompositionSupport_NullPointer`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT6_CHECKHARDWARECOMPOSITIONSUPPORT_NULLPOINTER: MessageId =
    MessageId(0x121);
/// The header's `DXGI_MSG_IDXGISwapChain_SetFullscreenState_PerMonitorDpiShimApplied`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_SETFULLSCREENSTATE_PERMONITORDPISHIMAPPLIED: MessageId =
    MessageId(0x122);
/// The header's `DXGI_MSG_IDXGIOutput_DuplicateOutput_PerMonitorDpiShimApplied`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_DUPLICATEOUTPUT_PERMONITORDPISHIMAPPLIED: MessageId =
    MessageId(0x123);
/// The header's `DXGI_MSG_IDXGIOutput_DuplicateOutput1_PerMonitorDpiRequired`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIOUTPUT_DUPLICATEOUTPUT1_PERMONITORDPIREQUIRED: MessageId = MessageId(0x124);
/// The header's `DXGI_MSG_IDXGIFactory7_UnregisterAdaptersChangedEvent_CookieNotFound`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY7_UNREGISTERADAPTERSCHANGEDEVENT_COOKIENOTFOUND: MessageId =
    MessageId(0x125);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_LegacyBltModelSwapEffect`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_LEGACYBLTMODELSWAPEFFECT: MessageId =
    MessageId(0x126);
/// The header's `DXGI_MSG_IDXGISwapChain4_SetHDRMetaData_MetadataUnchanged`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN4_SETHDRMETADATA_METADATAUNCHANGED: MessageId = MessageId(0x127);
/// The header's `DXGI_MSG_IDXGISwapChain_Present_11On12_Released_Resource`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGISWAPCHAIN_PRESENT_11ON12_RELEASED_RESOURCE: MessageId = MessageId(0x128);
/// The header's `DXGI_MSG_IDXGIFactory_CreateSwapChain_MultipleSwapchainRefToSurface_DeferredDtr`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_CREATESWAPCHAIN_MULTIPLESWAPCHAINREFTOSURFACE_DEFERREDDTR:
    MessageId = MessageId(0x129);
/// The header's `DXGI_MSG_IDXGIFactory_MakeWindowAssociation_NoOpBehavior`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_IDXGIFACTORY_MAKEWINDOWASSOCIATION_NOOPBEHAVIOR: MessageId = MessageId(0x12a);
/// The header's `DXGI_MSG_Phone_IDXGIFactory_CreateSwapChain_NotForegroundWindow`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGIFACTORY_CREATESWAPCHAIN_NOTFOREGROUNDWINDOW: MessageId =
    MessageId(0x3e8);
/// The header's `DXGI_MSG_Phone_IDXGIFactory_CreateSwapChain_DISCARD_BufferCount`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGIFACTORY_CREATESWAPCHAIN_DISCARD_BUFFERCOUNT: MessageId =
    MessageId(0x3e9);
/// The header's `DXGI_MSG_Phone_IDXGISwapChain_SetFullscreenState_NotAvailable`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGISWAPCHAIN_SETFULLSCREENSTATE_NOTAVAILABLE: MessageId =
    MessageId(0x3ea);
/// The header's `DXGI_MSG_Phone_IDXGISwapChain_ResizeBuffers_NotAvailable`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGISWAPCHAIN_RESIZEBUFFERS_NOTAVAILABLE: MessageId = MessageId(0x3eb);
/// The header's `DXGI_MSG_Phone_IDXGISwapChain_ResizeTarget_NotAvailable`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGISWAPCHAIN_RESIZETARGET_NOTAVAILABLE: MessageId = MessageId(0x3ec);
/// The header's `DXGI_MSG_Phone_IDXGISwapChain_Present_InvalidLayerIndex`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGISWAPCHAIN_PRESENT_INVALIDLAYERINDEX: MessageId = MessageId(0x3ed);
/// The header's `DXGI_MSG_Phone_IDXGISwapChain_Present_MultipleLayerIndex`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGISWAPCHAIN_PRESENT_MULTIPLELAYERINDEX: MessageId = MessageId(0x3ee);
/// The header's `DXGI_MSG_Phone_IDXGISwapChain_Present_InvalidLayerFlag`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGISWAPCHAIN_PRESENT_INVALIDLAYERFLAG: MessageId = MessageId(0x3ef);
/// The header's `DXGI_MSG_Phone_IDXGISwapChain_Present_InvalidRotation`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGISWAPCHAIN_PRESENT_INVALIDROTATION: MessageId = MessageId(0x3f0);
/// The header's `DXGI_MSG_Phone_IDXGISwapChain_Present_InvalidBlend`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGISWAPCHAIN_PRESENT_INVALIDBLEND: MessageId = MessageId(0x3f1);
/// The header's `DXGI_MSG_Phone_IDXGISwapChain_Present_InvalidResource`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGISWAPCHAIN_PRESENT_INVALIDRESOURCE: MessageId = MessageId(0x3f2);
/// The header's `DXGI_MSG_Phone_IDXGISwapChain_Present_InvalidMultiPlaneOverlayResource`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGISWAPCHAIN_PRESENT_INVALIDMULTIPLANEOVERLAYRESOURCE: MessageId =
    MessageId(0x3f3);
/// The header's `DXGI_MSG_Phone_IDXGISwapChain_Present_InvalidIndexForPrimary`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGISWAPCHAIN_PRESENT_INVALIDINDEXFORPRIMARY: MessageId =
    MessageId(0x3f4);
/// The header's `DXGI_MSG_Phone_IDXGISwapChain_Present_InvalidIndexForOverlay`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGISWAPCHAIN_PRESENT_INVALIDINDEXFOROVERLAY: MessageId =
    MessageId(0x3f5);
/// The header's `DXGI_MSG_Phone_IDXGISwapChain_Present_InvalidSubResourceIndex`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGISWAPCHAIN_PRESENT_INVALIDSUBRESOURCEINDEX: MessageId =
    MessageId(0x3f6);
/// The header's `DXGI_MSG_Phone_IDXGISwapChain_Present_InvalidSourceRect`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGISWAPCHAIN_PRESENT_INVALIDSOURCERECT: MessageId = MessageId(0x3f7);
/// The header's `DXGI_MSG_Phone_IDXGISwapChain_Present_InvalidDestinationRect`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGISWAPCHAIN_PRESENT_INVALIDDESTINATIONRECT: MessageId =
    MessageId(0x3f8);
/// The header's `DXGI_MSG_Phone_IDXGISwapChain_Present_MultipleResource`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGISWAPCHAIN_PRESENT_MULTIPLERESOURCE: MessageId = MessageId(0x3f9);
/// The header's `DXGI_MSG_Phone_IDXGISwapChain_Present_NotSharedResource`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGISWAPCHAIN_PRESENT_NOTSHAREDRESOURCE: MessageId = MessageId(0x3fa);
/// The header's `DXGI_MSG_Phone_IDXGISwapChain_Present_InvalidFlag`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGISWAPCHAIN_PRESENT_INVALIDFLAG: MessageId = MessageId(0x3fb);
/// The header's `DXGI_MSG_Phone_IDXGISwapChain_Present_InvalidInterval`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGISWAPCHAIN_PRESENT_INVALIDINTERVAL: MessageId = MessageId(0x3fc);
/// The header's `DXGI_MSG_Phone_IDXGIFactory_CreateSwapChain_MSAA_NotSupported`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGIFACTORY_CREATESWAPCHAIN_MSAA_NOTSUPPORTED: MessageId =
    MessageId(0x3fd);
/// The header's `DXGI_MSG_Phone_IDXGIFactory_CreateSwapChain_ScalingAspectRatioStretch_Supported_ModernApp`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGIFACTORY_CREATESWAPCHAIN_SCALINGASPECTRATIOSTRETCH_SUPPORTED_MODERNAPP: MessageId = MessageId(0x3fe);
/// The header's `DXGI_MSG_Phone_IDXGISwapChain_GetFrameStatistics_NotAvailable_ModernApp`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGISWAPCHAIN_GETFRAMESTATISTICS_NOTAVAILABLE_MODERNAPP: MessageId =
    MessageId(0x3ff);
/// The header's `DXGI_MSG_Phone_IDXGISwapChain_Present_ReplaceInterval0With1`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGISWAPCHAIN_PRESENT_REPLACEINTERVAL0WITH1: MessageId = MessageId(0x400);
/// The header's `DXGI_MSG_Phone_IDXGIFactory_CreateSwapChain_FailedRegisterWithCompositor`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGIFACTORY_CREATESWAPCHAIN_FAILEDREGISTERWITHCOMPOSITOR: MessageId =
    MessageId(0x401);
/// The header's `DXGI_MSG_Phone_IDXGIFactory_CreateSwapChain_NotForegroundWindow_AtRendering`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGIFACTORY_CREATESWAPCHAIN_NOTFOREGROUNDWINDOW_ATRENDERING: MessageId =
    MessageId(0x402);
/// The header's `DXGI_MSG_Phone_IDXGIFactory_CreateSwapChain_FLIP_SEQUENTIAL_BufferCount`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGIFACTORY_CREATESWAPCHAIN_FLIP_SEQUENTIAL_BUFFERCOUNT: MessageId =
    MessageId(0x403);
/// The header's `DXGI_MSG_Phone_IDXGIFactory_CreateSwapChain_FLIP_Modern_CoreWindow_Only`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGIFACTORY_CREATESWAPCHAIN_FLIP_MODERN_COREWINDOW_ONLY: MessageId =
    MessageId(0x404);
/// The header's `DXGI_MSG_Phone_IDXGISwapChain_Present1_RequiresOverlays`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGISWAPCHAIN_PRESENT1_REQUIRESOVERLAYS: MessageId = MessageId(0x405);
/// The header's `DXGI_MSG_Phone_IDXGISwapChain_SetBackgroundColor_FlipSequentialRequired`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGISWAPCHAIN_SETBACKGROUNDCOLOR_FLIPSEQUENTIALREQUIRED: MessageId =
    MessageId(0x406);
/// The header's `DXGI_MSG_Phone_IDXGISwapChain_GetBackgroundColor_FlipSequentialRequired`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MSG_PHONE_IDXGISWAPCHAIN_GETBACKGROUNDCOLOR_FLIPSEQUENTIALREQUIRED: MessageId =
    MessageId(0x407);

/// The header's `DXGI_OFFER_RESOURCE_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_5/ne-dxgi1_5-dxgi_offer_resource_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct OfferResourceFlags(pub i32);

/// The header's `DXGI_OFFER_RESOURCE_FLAG_ALLOW_DECOMMIT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_5/ne-dxgi1_5-dxgi_offer_resource_flags).
pub const DXGI_OFFER_RESOURCE_FLAG_ALLOW_DECOMMIT: OfferResourceFlags = OfferResourceFlags(1);

/// The header's `DXGI_OFFER_RESOURCE_PRIORITY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_offer_resource_priority).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct OfferResourcePriority(pub i32);

/// The header's `DXGI_OFFER_RESOURCE_PRIORITY_LOW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_offer_resource_priority).
pub const DXGI_OFFER_RESOURCE_PRIORITY_LOW: OfferResourcePriority = OfferResourcePriority(1);
/// The header's `DXGI_OFFER_RESOURCE_PRIORITY_NORMAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_offer_resource_priority).
pub const DXGI_OFFER_RESOURCE_PRIORITY_NORMAL: OfferResourcePriority = OfferResourcePriority(2);
/// The header's `DXGI_OFFER_RESOURCE_PRIORITY_HIGH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_offer_resource_priority).
pub const DXGI_OFFER_RESOURCE_PRIORITY_HIGH: OfferResourcePriority = OfferResourcePriority(3);

/// The header's `DXGI_OUTDUPL_FLAG`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct OutduplFlag(pub i32);

/// The header's `DXGI_OUTDUPL_COMPOSITED_UI_CAPTURE_ONLY`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_OUTDUPL_COMPOSITED_UI_CAPTURE_ONLY: OutduplFlag = OutduplFlag(1);

/// The header's `DXGI_OUTDUPL_POINTER_SHAPE_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_outdupl_pointer_shape_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct OutduplPointerShapeType(pub i32);

/// The header's `DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MONOCHROME`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_outdupl_pointer_shape_type).
pub const DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MONOCHROME: OutduplPointerShapeType =
    OutduplPointerShapeType(1);
/// The header's `DXGI_OUTDUPL_POINTER_SHAPE_TYPE_COLOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_outdupl_pointer_shape_type).
pub const DXGI_OUTDUPL_POINTER_SHAPE_TYPE_COLOR: OutduplPointerShapeType =
    OutduplPointerShapeType(2);
/// The header's `DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MASKED_COLOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_outdupl_pointer_shape_type).
pub const DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MASKED_COLOR: OutduplPointerShapeType =
    OutduplPointerShapeType(4);

/// The header's `DXGI_OVERLAY_COLOR_SPACE_SUPPORT_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/ne-dxgi1_4-dxgi_overlay_color_space_support_flag).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct OverlayColorSpaceSupportFlag(pub i32);

/// The header's `DXGI_OVERLAY_COLOR_SPACE_SUPPORT_FLAG_PRESENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/ne-dxgi1_4-dxgi_overlay_color_space_support_flag).
pub const DXGI_OVERLAY_COLOR_SPACE_SUPPORT_FLAG_PRESENT: OverlayColorSpaceSupportFlag =
    OverlayColorSpaceSupportFlag(1);

/// The header's `DXGI_OVERLAY_SUPPORT_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/ne-dxgi1_3-dxgi_overlay_support_flag).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct OverlaySupportFlag(pub i32);

/// The header's `DXGI_OVERLAY_SUPPORT_FLAG_DIRECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/ne-dxgi1_3-dxgi_overlay_support_flag).
pub const DXGI_OVERLAY_SUPPORT_FLAG_DIRECT: OverlaySupportFlag = OverlaySupportFlag(1);
/// The header's `DXGI_OVERLAY_SUPPORT_FLAG_SCALING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/ne-dxgi1_3-dxgi_overlay_support_flag).
pub const DXGI_OVERLAY_SUPPORT_FLAG_SCALING: OverlaySupportFlag = OverlaySupportFlag(2);

/// The header's `DXGI_PRESENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-present).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Present(pub u32);

/// The header's `DXGI_PRESENT_TEST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-present).
pub const DXGI_PRESENT_TEST: Present = Present(1);
/// The header's `DXGI_PRESENT_DO_NOT_SEQUENCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-present).
pub const DXGI_PRESENT_DO_NOT_SEQUENCE: Present = Present(2);
/// The header's `DXGI_PRESENT_RESTART`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-present).
pub const DXGI_PRESENT_RESTART: Present = Present(4);
/// The header's `DXGI_PRESENT_DO_NOT_WAIT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-present).
pub const DXGI_PRESENT_DO_NOT_WAIT: Present = Present(8);
/// The header's `DXGI_PRESENT_STEREO_PREFER_RIGHT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-present).
pub const DXGI_PRESENT_STEREO_PREFER_RIGHT: Present = Present(16);
/// The header's `DXGI_PRESENT_STEREO_TEMPORARY_MONO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-present).
pub const DXGI_PRESENT_STEREO_TEMPORARY_MONO: Present = Present(32);
/// The header's `DXGI_PRESENT_RESTRICT_TO_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-present).
pub const DXGI_PRESENT_RESTRICT_TO_OUTPUT: Present = Present(64);
/// The header's `DXGI_PRESENT_USE_DURATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-present).
pub const DXGI_PRESENT_USE_DURATION: Present = Present(0x100);
/// The header's `DXGI_PRESENT_ALLOW_TEARING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-present).
pub const DXGI_PRESENT_ALLOW_TEARING: Present = Present(0x200);

/// The header's `DXGI_RECLAIM_RESOURCE_RESULTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_5/ne-dxgi1_5-dxgi_reclaim_resource_results).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ReclaimResourceResults(pub i32);

/// The header's `DXGI_RECLAIM_RESOURCE_RESULT_OK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_5/ne-dxgi1_5-dxgi_reclaim_resource_results).
pub const DXGI_RECLAIM_RESOURCE_RESULT_OK: ReclaimResourceResults = ReclaimResourceResults(0);
/// The header's `DXGI_RECLAIM_RESOURCE_RESULT_DISCARDED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_5/ne-dxgi1_5-dxgi_reclaim_resource_results).
pub const DXGI_RECLAIM_RESOURCE_RESULT_DISCARDED: ReclaimResourceResults =
    ReclaimResourceResults(1);
/// The header's `DXGI_RECLAIM_RESOURCE_RESULT_NOT_COMMITTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_5/ne-dxgi1_5-dxgi_reclaim_resource_results).
pub const DXGI_RECLAIM_RESOURCE_RESULT_NOT_COMMITTED: ReclaimResourceResults =
    ReclaimResourceResults(2);

/// The header's `DXGI_RESIDENCY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_residency).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Residency(pub i32);

/// The header's `DXGI_RESIDENCY_FULLY_RESIDENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_residency).
pub const DXGI_RESIDENCY_FULLY_RESIDENT: Residency = Residency(1);
/// The header's `DXGI_RESIDENCY_RESIDENT_IN_SHARED_MEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_residency).
pub const DXGI_RESIDENCY_RESIDENT_IN_SHARED_MEMORY: Residency = Residency(2);
/// The header's `DXGI_RESIDENCY_EVICTED_TO_DISK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_residency).
pub const DXGI_RESIDENCY_EVICTED_TO_DISK: Residency = Residency(3);

/// The header's `DXGI_RESOURCE_PRIORITY`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ResourcePriority(pub u32);

/// The header's `DXGI_RESOURCE_PRIORITY_MINIMUM`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_RESOURCE_PRIORITY_MINIMUM: ResourcePriority = ResourcePriority(0x28000000);
/// The header's `DXGI_RESOURCE_PRIORITY_LOW`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_RESOURCE_PRIORITY_LOW: ResourcePriority = ResourcePriority(0x50000000);
/// The header's `DXGI_RESOURCE_PRIORITY_NORMAL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_RESOURCE_PRIORITY_NORMAL: ResourcePriority = ResourcePriority(0x78000000);
/// The header's `DXGI_RESOURCE_PRIORITY_HIGH`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_RESOURCE_PRIORITY_HIGH: ResourcePriority = ResourcePriority(0xa0000000);
/// The header's `DXGI_RESOURCE_PRIORITY_MAXIMUM`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_RESOURCE_PRIORITY_MAXIMUM: ResourcePriority = ResourcePriority(0xc8000000);

/// The header's `DXGI_SCALING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_scaling).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Scaling(pub i32);

/// The header's `DXGI_SCALING_STRETCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_scaling).
pub const DXGI_SCALING_STRETCH: Scaling = Scaling(0);
/// The header's `DXGI_SCALING_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_scaling).
pub const DXGI_SCALING_NONE: Scaling = Scaling(1);
/// The header's `DXGI_SCALING_ASPECT_RATIO_STRETCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_scaling).
pub const DXGI_SCALING_ASPECT_RATIO_STRETCH: Scaling = Scaling(2);

/// The header's `DXGI_SHARED_RESOURCE_RW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-shared-resource-rw).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SharedResourceRw(pub u32);

/// The header's `DXGI_SHARED_RESOURCE_READ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-shared-resource-rw).
pub const DXGI_SHARED_RESOURCE_READ: SharedResourceRw = SharedResourceRw(0x80000000);
/// The header's `DXGI_SHARED_RESOURCE_WRITE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-shared-resource-rw).
pub const DXGI_SHARED_RESOURCE_WRITE: SharedResourceRw = SharedResourceRw(1);

/// The header's `DXGI_SWAP_CHAIN_COLOR_SPACE_SUPPORT_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/ne-dxgi1_4-dxgi_swap_chain_color_space_support_flag).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SwapChainColorSpaceSupportFlag(pub i32);

/// The header's `DXGI_SWAP_CHAIN_COLOR_SPACE_SUPPORT_FLAG_PRESENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/ne-dxgi1_4-dxgi_swap_chain_color_space_support_flag).
pub const DXGI_SWAP_CHAIN_COLOR_SPACE_SUPPORT_FLAG_PRESENT: SwapChainColorSpaceSupportFlag =
    SwapChainColorSpaceSupportFlag(1);
/// The header's `DXGI_SWAP_CHAIN_COLOR_SPACE_SUPPORT_FLAG_OVERLAY_PRESENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/ne-dxgi1_4-dxgi_swap_chain_color_space_support_flag).
pub const DXGI_SWAP_CHAIN_COLOR_SPACE_SUPPORT_FLAG_OVERLAY_PRESENT: SwapChainColorSpaceSupportFlag =
    SwapChainColorSpaceSupportFlag(2);

/// The header's `DXGI_SWAP_CHAIN_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_swap_chain_flag).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SwapChainFlag(pub i32);

/// The header's `DXGI_SWAP_CHAIN_FLAG_NONPREROTATED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_swap_chain_flag).
pub const DXGI_SWAP_CHAIN_FLAG_NONPREROTATED: SwapChainFlag = SwapChainFlag(1);
/// The header's `DXGI_SWAP_CHAIN_FLAG_ALLOW_MODE_SWITCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_swap_chain_flag).
pub const DXGI_SWAP_CHAIN_FLAG_ALLOW_MODE_SWITCH: SwapChainFlag = SwapChainFlag(2);
/// The header's `DXGI_SWAP_CHAIN_FLAG_GDI_COMPATIBLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_swap_chain_flag).
pub const DXGI_SWAP_CHAIN_FLAG_GDI_COMPATIBLE: SwapChainFlag = SwapChainFlag(4);
/// The header's `DXGI_SWAP_CHAIN_FLAG_RESTRICTED_CONTENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_swap_chain_flag).
pub const DXGI_SWAP_CHAIN_FLAG_RESTRICTED_CONTENT: SwapChainFlag = SwapChainFlag(8);
/// The header's `DXGI_SWAP_CHAIN_FLAG_RESTRICT_SHARED_RESOURCE_DRIVER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_swap_chain_flag).
pub const DXGI_SWAP_CHAIN_FLAG_RESTRICT_SHARED_RESOURCE_DRIVER: SwapChainFlag = SwapChainFlag(16);
/// The header's `DXGI_SWAP_CHAIN_FLAG_DISPLAY_ONLY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_swap_chain_flag).
pub const DXGI_SWAP_CHAIN_FLAG_DISPLAY_ONLY: SwapChainFlag = SwapChainFlag(32);
/// The header's `DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_swap_chain_flag).
pub const DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT: SwapChainFlag = SwapChainFlag(64);
/// The header's `DXGI_SWAP_CHAIN_FLAG_FOREGROUND_LAYER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_swap_chain_flag).
pub const DXGI_SWAP_CHAIN_FLAG_FOREGROUND_LAYER: SwapChainFlag = SwapChainFlag(128);
/// The header's `DXGI_SWAP_CHAIN_FLAG_FULLSCREEN_VIDEO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_swap_chain_flag).
pub const DXGI_SWAP_CHAIN_FLAG_FULLSCREEN_VIDEO: SwapChainFlag = SwapChainFlag(0x100);
/// The header's `DXGI_SWAP_CHAIN_FLAG_YUV_VIDEO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_swap_chain_flag).
pub const DXGI_SWAP_CHAIN_FLAG_YUV_VIDEO: SwapChainFlag = SwapChainFlag(0x200);
/// The header's `DXGI_SWAP_CHAIN_FLAG_HW_PROTECTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_swap_chain_flag).
pub const DXGI_SWAP_CHAIN_FLAG_HW_PROTECTED: SwapChainFlag = SwapChainFlag(0x400);
/// The header's `DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_swap_chain_flag).
pub const DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING: SwapChainFlag = SwapChainFlag(0x800);
/// The header's `DXGI_SWAP_CHAIN_FLAG_RESTRICTED_TO_ALL_HOLOGRAPHIC_DISPLAYS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_swap_chain_flag).
pub const DXGI_SWAP_CHAIN_FLAG_RESTRICTED_TO_ALL_HOLOGRAPHIC_DISPLAYS: SwapChainFlag =
    SwapChainFlag(0x1000);

/// The header's `DXGI_SWAP_EFFECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_swap_effect).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SwapEffect(pub i32);

/// The header's `DXGI_SWAP_EFFECT_DISCARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_swap_effect).
pub const DXGI_SWAP_EFFECT_DISCARD: SwapEffect = SwapEffect(0);
/// The header's `DXGI_SWAP_EFFECT_SEQUENTIAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_swap_effect).
pub const DXGI_SWAP_EFFECT_SEQUENTIAL: SwapEffect = SwapEffect(1);
/// The header's `DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_swap_effect).
pub const DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL: SwapEffect = SwapEffect(3);
/// The header's `DXGI_SWAP_EFFECT_FLIP_DISCARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_swap_effect).
pub const DXGI_SWAP_EFFECT_FLIP_DISCARD: SwapEffect = SwapEffect(4);

/// The header's `DXGI_USAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-usage).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Usage(pub u32);

/// The header's `DXGI_USAGE_SHADER_INPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-usage).
pub const DXGI_USAGE_SHADER_INPUT: Usage = Usage(16);
/// The header's `DXGI_USAGE_RENDER_TARGET_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-usage).
pub const DXGI_USAGE_RENDER_TARGET_OUTPUT: Usage = Usage(32);
/// The header's `DXGI_USAGE_BACK_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-usage).
pub const DXGI_USAGE_BACK_BUFFER: Usage = Usage(64);
/// The header's `DXGI_USAGE_SHARED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-usage).
pub const DXGI_USAGE_SHARED: Usage = Usage(128);
/// The header's `DXGI_USAGE_READ_ONLY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-usage).
pub const DXGI_USAGE_READ_ONLY: Usage = Usage(0x100);
/// The header's `DXGI_USAGE_DISCARD_ON_PRESENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-usage).
pub const DXGI_USAGE_DISCARD_ON_PRESENT: Usage = Usage(0x200);
/// The header's `DXGI_USAGE_UNORDERED_ACCESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-usage).
pub const DXGI_USAGE_UNORDERED_ACCESS: Usage = Usage(0x400);

/// The header's `DXGI_ALPHA_MODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_alpha_mode).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct AlphaMode(pub i32);

/// The header's `DXGI_ALPHA_MODE_UNSPECIFIED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_alpha_mode).
pub const DXGI_ALPHA_MODE_UNSPECIFIED: AlphaMode = AlphaMode(0);
/// The header's `DXGI_ALPHA_MODE_PREMULTIPLIED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_alpha_mode).
pub const DXGI_ALPHA_MODE_PREMULTIPLIED: AlphaMode = AlphaMode(1);
/// The header's `DXGI_ALPHA_MODE_STRAIGHT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_alpha_mode).
pub const DXGI_ALPHA_MODE_STRAIGHT: AlphaMode = AlphaMode(2);
/// The header's `DXGI_ALPHA_MODE_IGNORE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_alpha_mode).
pub const DXGI_ALPHA_MODE_IGNORE: AlphaMode = AlphaMode(3);

/// The header's `DXGI_COLOR_SPACE_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ne-dxgicommon-dxgi_color_space_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ColorSpaceType(pub i32);

/// The header's `DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ne-dxgicommon-dxgi_color_space_type).
pub const DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709: ColorSpaceType = ColorSpaceType(0);
/// The header's `DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ne-dxgicommon-dxgi_color_space_type).
pub const DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709: ColorSpaceType = ColorSpaceType(1);
/// The header's `DXGI_COLOR_SPACE_RGB_STUDIO_G22_NONE_P709`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ne-dxgicommon-dxgi_color_space_type).
pub const DXGI_COLOR_SPACE_RGB_STUDIO_G22_NONE_P709: ColorSpaceType = ColorSpaceType(2);
/// The header's `DXGI_COLOR_SPACE_RGB_STUDIO_G22_NONE_P2020`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ne-dxgicommon-dxgi_color_space_type).
pub const DXGI_COLOR_SPACE_RGB_STUDIO_G22_NONE_P2020: ColorSpaceType = ColorSpaceType(3);
/// The header's `DXGI_COLOR_SPACE_RESERVED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ne-dxgicommon-dxgi_color_space_type).
pub const DXGI_COLOR_SPACE_RESERVED: ColorSpaceType = ColorSpaceType(4);
/// The header's `DXGI_COLOR_SPACE_YCBCR_FULL_G22_NONE_P709_X601`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ne-dxgicommon-dxgi_color_space_type).
pub const DXGI_COLOR_SPACE_YCBCR_FULL_G22_NONE_P709_X601: ColorSpaceType = ColorSpaceType(5);
/// The header's `DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P601`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ne-dxgicommon-dxgi_color_space_type).
pub const DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P601: ColorSpaceType = ColorSpaceType(6);
/// The header's `DXGI_COLOR_SPACE_YCBCR_FULL_G22_LEFT_P601`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ne-dxgicommon-dxgi_color_space_type).
pub const DXGI_COLOR_SPACE_YCBCR_FULL_G22_LEFT_P601: ColorSpaceType = ColorSpaceType(7);
/// The header's `DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P709`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ne-dxgicommon-dxgi_color_space_type).
pub const DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P709: ColorSpaceType = ColorSpaceType(8);
/// The header's `DXGI_COLOR_SPACE_YCBCR_FULL_G22_LEFT_P709`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ne-dxgicommon-dxgi_color_space_type).
pub const DXGI_COLOR_SPACE_YCBCR_FULL_G22_LEFT_P709: ColorSpaceType = ColorSpaceType(9);
/// The header's `DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P2020`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ne-dxgicommon-dxgi_color_space_type).
pub const DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P2020: ColorSpaceType = ColorSpaceType(10);
/// The header's `DXGI_COLOR_SPACE_YCBCR_FULL_G22_LEFT_P2020`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ne-dxgicommon-dxgi_color_space_type).
pub const DXGI_COLOR_SPACE_YCBCR_FULL_G22_LEFT_P2020: ColorSpaceType = ColorSpaceType(11);
/// The header's `DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ne-dxgicommon-dxgi_color_space_type).
pub const DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020: ColorSpaceType = ColorSpaceType(12);
/// The header's `DXGI_COLOR_SPACE_YCBCR_STUDIO_G2084_LEFT_P2020`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ne-dxgicommon-dxgi_color_space_type).
pub const DXGI_COLOR_SPACE_YCBCR_STUDIO_G2084_LEFT_P2020: ColorSpaceType = ColorSpaceType(13);
/// The header's `DXGI_COLOR_SPACE_RGB_STUDIO_G2084_NONE_P2020`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ne-dxgicommon-dxgi_color_space_type).
pub const DXGI_COLOR_SPACE_RGB_STUDIO_G2084_NONE_P2020: ColorSpaceType = ColorSpaceType(14);
/// The header's `DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_TOPLEFT_P2020`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ne-dxgicommon-dxgi_color_space_type).
pub const DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_TOPLEFT_P2020: ColorSpaceType = ColorSpaceType(15);
/// The header's `DXGI_COLOR_SPACE_YCBCR_STUDIO_G2084_TOPLEFT_P2020`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ne-dxgicommon-dxgi_color_space_type).
pub const DXGI_COLOR_SPACE_YCBCR_STUDIO_G2084_TOPLEFT_P2020: ColorSpaceType = ColorSpaceType(16);
/// The header's `DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P2020`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ne-dxgicommon-dxgi_color_space_type).
pub const DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P2020: ColorSpaceType = ColorSpaceType(17);
/// The header's `DXGI_COLOR_SPACE_YCBCR_STUDIO_GHLG_TOPLEFT_P2020`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ne-dxgicommon-dxgi_color_space_type).
pub const DXGI_COLOR_SPACE_YCBCR_STUDIO_GHLG_TOPLEFT_P2020: ColorSpaceType = ColorSpaceType(18);
/// The header's `DXGI_COLOR_SPACE_YCBCR_FULL_GHLG_TOPLEFT_P2020`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ne-dxgicommon-dxgi_color_space_type).
pub const DXGI_COLOR_SPACE_YCBCR_FULL_GHLG_TOPLEFT_P2020: ColorSpaceType = ColorSpaceType(19);
/// The header's `DXGI_COLOR_SPACE_RGB_STUDIO_G24_NONE_P709`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ne-dxgicommon-dxgi_color_space_type).
pub const DXGI_COLOR_SPACE_RGB_STUDIO_G24_NONE_P709: ColorSpaceType = ColorSpaceType(20);
/// The header's `DXGI_COLOR_SPACE_RGB_STUDIO_G24_NONE_P2020`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ne-dxgicommon-dxgi_color_space_type).
pub const DXGI_COLOR_SPACE_RGB_STUDIO_G24_NONE_P2020: ColorSpaceType = ColorSpaceType(21);
/// The header's `DXGI_COLOR_SPACE_YCBCR_STUDIO_G24_LEFT_P709`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ne-dxgicommon-dxgi_color_space_type).
pub const DXGI_COLOR_SPACE_YCBCR_STUDIO_G24_LEFT_P709: ColorSpaceType = ColorSpaceType(22);
/// The header's `DXGI_COLOR_SPACE_YCBCR_STUDIO_G24_LEFT_P2020`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ne-dxgicommon-dxgi_color_space_type).
pub const DXGI_COLOR_SPACE_YCBCR_STUDIO_G24_LEFT_P2020: ColorSpaceType = ColorSpaceType(23);
/// The header's `DXGI_COLOR_SPACE_YCBCR_STUDIO_G24_TOPLEFT_P2020`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ne-dxgicommon-dxgi_color_space_type).
pub const DXGI_COLOR_SPACE_YCBCR_STUDIO_G24_TOPLEFT_P2020: ColorSpaceType = ColorSpaceType(24);
/// The header's `DXGI_COLOR_SPACE_CUSTOM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ne-dxgicommon-dxgi_color_space_type).
pub const DXGI_COLOR_SPACE_CUSTOM: ColorSpaceType = ColorSpaceType(-1);

/// The header's `DXGI_FORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Format(pub i32);

/// The header's `DXGI_FORMAT_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_UNKNOWN: Format = Format(0);
/// The header's `DXGI_FORMAT_R32G32B32A32_TYPELESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R32G32B32A32_TYPELESS: Format = Format(1);
/// The header's `DXGI_FORMAT_R32G32B32A32_FLOAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R32G32B32A32_FLOAT: Format = Format(2);
/// The header's `DXGI_FORMAT_R32G32B32A32_UINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R32G32B32A32_UINT: Format = Format(3);
/// The header's `DXGI_FORMAT_R32G32B32A32_SINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R32G32B32A32_SINT: Format = Format(4);
/// The header's `DXGI_FORMAT_R32G32B32_TYPELESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R32G32B32_TYPELESS: Format = Format(5);
/// The header's `DXGI_FORMAT_R32G32B32_FLOAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R32G32B32_FLOAT: Format = Format(6);
/// The header's `DXGI_FORMAT_R32G32B32_UINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R32G32B32_UINT: Format = Format(7);
/// The header's `DXGI_FORMAT_R32G32B32_SINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R32G32B32_SINT: Format = Format(8);
/// The header's `DXGI_FORMAT_R16G16B16A16_TYPELESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R16G16B16A16_TYPELESS: Format = Format(9);
/// The header's `DXGI_FORMAT_R16G16B16A16_FLOAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R16G16B16A16_FLOAT: Format = Format(10);
/// The header's `DXGI_FORMAT_R16G16B16A16_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R16G16B16A16_UNORM: Format = Format(11);
/// The header's `DXGI_FORMAT_R16G16B16A16_UINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R16G16B16A16_UINT: Format = Format(12);
/// The header's `DXGI_FORMAT_R16G16B16A16_SNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R16G16B16A16_SNORM: Format = Format(13);
/// The header's `DXGI_FORMAT_R16G16B16A16_SINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R16G16B16A16_SINT: Format = Format(14);
/// The header's `DXGI_FORMAT_R32G32_TYPELESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R32G32_TYPELESS: Format = Format(15);
/// The header's `DXGI_FORMAT_R32G32_FLOAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R32G32_FLOAT: Format = Format(16);
/// The header's `DXGI_FORMAT_R32G32_UINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R32G32_UINT: Format = Format(17);
/// The header's `DXGI_FORMAT_R32G32_SINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R32G32_SINT: Format = Format(18);
/// The header's `DXGI_FORMAT_R32G8X24_TYPELESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R32G8X24_TYPELESS: Format = Format(19);
/// The header's `DXGI_FORMAT_D32_FLOAT_S8X24_UINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_D32_FLOAT_S8X24_UINT: Format = Format(20);
/// The header's `DXGI_FORMAT_R32_FLOAT_X8X24_TYPELESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R32_FLOAT_X8X24_TYPELESS: Format = Format(21);
/// The header's `DXGI_FORMAT_X32_TYPELESS_G8X24_UINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_X32_TYPELESS_G8X24_UINT: Format = Format(22);
/// The header's `DXGI_FORMAT_R10G10B10A2_TYPELESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R10G10B10A2_TYPELESS: Format = Format(23);
/// The header's `DXGI_FORMAT_R10G10B10A2_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R10G10B10A2_UNORM: Format = Format(24);
/// The header's `DXGI_FORMAT_R10G10B10A2_UINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R10G10B10A2_UINT: Format = Format(25);
/// The header's `DXGI_FORMAT_R11G11B10_FLOAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R11G11B10_FLOAT: Format = Format(26);
/// The header's `DXGI_FORMAT_R8G8B8A8_TYPELESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R8G8B8A8_TYPELESS: Format = Format(27);
/// The header's `DXGI_FORMAT_R8G8B8A8_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R8G8B8A8_UNORM: Format = Format(28);
/// The header's `DXGI_FORMAT_R8G8B8A8_UNORM_SRGB`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R8G8B8A8_UNORM_SRGB: Format = Format(29);
/// The header's `DXGI_FORMAT_R8G8B8A8_UINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R8G8B8A8_UINT: Format = Format(30);
/// The header's `DXGI_FORMAT_R8G8B8A8_SNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R8G8B8A8_SNORM: Format = Format(31);
/// The header's `DXGI_FORMAT_R8G8B8A8_SINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R8G8B8A8_SINT: Format = Format(32);
/// The header's `DXGI_FORMAT_R16G16_TYPELESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R16G16_TYPELESS: Format = Format(33);
/// The header's `DXGI_FORMAT_R16G16_FLOAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R16G16_FLOAT: Format = Format(34);
/// The header's `DXGI_FORMAT_R16G16_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R16G16_UNORM: Format = Format(35);
/// The header's `DXGI_FORMAT_R16G16_UINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R16G16_UINT: Format = Format(36);
/// The header's `DXGI_FORMAT_R16G16_SNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R16G16_SNORM: Format = Format(37);
/// The header's `DXGI_FORMAT_R16G16_SINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R16G16_SINT: Format = Format(38);
/// The header's `DXGI_FORMAT_R32_TYPELESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R32_TYPELESS: Format = Format(39);
/// The header's `DXGI_FORMAT_D32_FLOAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_D32_FLOAT: Format = Format(40);
/// The header's `DXGI_FORMAT_R32_FLOAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R32_FLOAT: Format = Format(41);
/// The header's `DXGI_FORMAT_R32_UINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R32_UINT: Format = Format(42);
/// The header's `DXGI_FORMAT_R32_SINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R32_SINT: Format = Format(43);
/// The header's `DXGI_FORMAT_R24G8_TYPELESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R24G8_TYPELESS: Format = Format(44);
/// The header's `DXGI_FORMAT_D24_UNORM_S8_UINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_D24_UNORM_S8_UINT: Format = Format(45);
/// The header's `DXGI_FORMAT_R24_UNORM_X8_TYPELESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R24_UNORM_X8_TYPELESS: Format = Format(46);
/// The header's `DXGI_FORMAT_X24_TYPELESS_G8_UINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_X24_TYPELESS_G8_UINT: Format = Format(47);
/// The header's `DXGI_FORMAT_R8G8_TYPELESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R8G8_TYPELESS: Format = Format(48);
/// The header's `DXGI_FORMAT_R8G8_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R8G8_UNORM: Format = Format(49);
/// The header's `DXGI_FORMAT_R8G8_UINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R8G8_UINT: Format = Format(50);
/// The header's `DXGI_FORMAT_R8G8_SNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R8G8_SNORM: Format = Format(51);
/// The header's `DXGI_FORMAT_R8G8_SINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R8G8_SINT: Format = Format(52);
/// The header's `DXGI_FORMAT_R16_TYPELESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R16_TYPELESS: Format = Format(53);
/// The header's `DXGI_FORMAT_R16_FLOAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R16_FLOAT: Format = Format(54);
/// The header's `DXGI_FORMAT_D16_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_D16_UNORM: Format = Format(55);
/// The header's `DXGI_FORMAT_R16_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R16_UNORM: Format = Format(56);
/// The header's `DXGI_FORMAT_R16_UINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R16_UINT: Format = Format(57);
/// The header's `DXGI_FORMAT_R16_SNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R16_SNORM: Format = Format(58);
/// The header's `DXGI_FORMAT_R16_SINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R16_SINT: Format = Format(59);
/// The header's `DXGI_FORMAT_R8_TYPELESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R8_TYPELESS: Format = Format(60);
/// The header's `DXGI_FORMAT_R8_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R8_UNORM: Format = Format(61);
/// The header's `DXGI_FORMAT_R8_UINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R8_UINT: Format = Format(62);
/// The header's `DXGI_FORMAT_R8_SNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R8_SNORM: Format = Format(63);
/// The header's `DXGI_FORMAT_R8_SINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R8_SINT: Format = Format(64);
/// The header's `DXGI_FORMAT_A8_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_A8_UNORM: Format = Format(65);
/// The header's `DXGI_FORMAT_R1_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R1_UNORM: Format = Format(66);
/// The header's `DXGI_FORMAT_R9G9B9E5_SHAREDEXP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R9G9B9E5_SHAREDEXP: Format = Format(67);
/// The header's `DXGI_FORMAT_R8G8_B8G8_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R8G8_B8G8_UNORM: Format = Format(68);
/// The header's `DXGI_FORMAT_G8R8_G8B8_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_G8R8_G8B8_UNORM: Format = Format(69);
/// The header's `DXGI_FORMAT_BC1_TYPELESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_BC1_TYPELESS: Format = Format(70);
/// The header's `DXGI_FORMAT_BC1_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_BC1_UNORM: Format = Format(71);
/// The header's `DXGI_FORMAT_BC1_UNORM_SRGB`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_BC1_UNORM_SRGB: Format = Format(72);
/// The header's `DXGI_FORMAT_BC2_TYPELESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_BC2_TYPELESS: Format = Format(73);
/// The header's `DXGI_FORMAT_BC2_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_BC2_UNORM: Format = Format(74);
/// The header's `DXGI_FORMAT_BC2_UNORM_SRGB`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_BC2_UNORM_SRGB: Format = Format(75);
/// The header's `DXGI_FORMAT_BC3_TYPELESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_BC3_TYPELESS: Format = Format(76);
/// The header's `DXGI_FORMAT_BC3_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_BC3_UNORM: Format = Format(77);
/// The header's `DXGI_FORMAT_BC3_UNORM_SRGB`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_BC3_UNORM_SRGB: Format = Format(78);
/// The header's `DXGI_FORMAT_BC4_TYPELESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_BC4_TYPELESS: Format = Format(79);
/// The header's `DXGI_FORMAT_BC4_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_BC4_UNORM: Format = Format(80);
/// The header's `DXGI_FORMAT_BC4_SNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_BC4_SNORM: Format = Format(81);
/// The header's `DXGI_FORMAT_BC5_TYPELESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_BC5_TYPELESS: Format = Format(82);
/// The header's `DXGI_FORMAT_BC5_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_BC5_UNORM: Format = Format(83);
/// The header's `DXGI_FORMAT_BC5_SNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_BC5_SNORM: Format = Format(84);
/// The header's `DXGI_FORMAT_B5G6R5_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_B5G6R5_UNORM: Format = Format(85);
/// The header's `DXGI_FORMAT_B5G5R5A1_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_B5G5R5A1_UNORM: Format = Format(86);
/// The header's `DXGI_FORMAT_B8G8R8A8_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_B8G8R8A8_UNORM: Format = Format(87);
/// The header's `DXGI_FORMAT_B8G8R8X8_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_B8G8R8X8_UNORM: Format = Format(88);
/// The header's `DXGI_FORMAT_R10G10B10_XR_BIAS_A2_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_R10G10B10_XR_BIAS_A2_UNORM: Format = Format(89);
/// The header's `DXGI_FORMAT_B8G8R8A8_TYPELESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_B8G8R8A8_TYPELESS: Format = Format(90);
/// The header's `DXGI_FORMAT_B8G8R8A8_UNORM_SRGB`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_B8G8R8A8_UNORM_SRGB: Format = Format(91);
/// The header's `DXGI_FORMAT_B8G8R8X8_TYPELESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_B8G8R8X8_TYPELESS: Format = Format(92);
/// The header's `DXGI_FORMAT_B8G8R8X8_UNORM_SRGB`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_B8G8R8X8_UNORM_SRGB: Format = Format(93);
/// The header's `DXGI_FORMAT_BC6H_TYPELESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_BC6H_TYPELESS: Format = Format(94);
/// The header's `DXGI_FORMAT_BC6H_UF16`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_BC6H_UF16: Format = Format(95);
/// The header's `DXGI_FORMAT_BC6H_SF16`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_BC6H_SF16: Format = Format(96);
/// The header's `DXGI_FORMAT_BC7_TYPELESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_BC7_TYPELESS: Format = Format(97);
/// The header's `DXGI_FORMAT_BC7_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_BC7_UNORM: Format = Format(98);
/// The header's `DXGI_FORMAT_BC7_UNORM_SRGB`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_BC7_UNORM_SRGB: Format = Format(99);
/// The header's `DXGI_FORMAT_AYUV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_AYUV: Format = Format(100);
/// The header's `DXGI_FORMAT_Y410`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_Y410: Format = Format(101);
/// The header's `DXGI_FORMAT_Y416`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_Y416: Format = Format(102);
/// The header's `DXGI_FORMAT_NV12`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_NV12: Format = Format(103);
/// The header's `DXGI_FORMAT_P010`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_P010: Format = Format(104);
/// The header's `DXGI_FORMAT_P016`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_P016: Format = Format(105);
/// The header's `DXGI_FORMAT_420_OPAQUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_420_OPAQUE: Format = Format(106);
/// The header's `DXGI_FORMAT_YUY2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_YUY2: Format = Format(107);
/// The header's `DXGI_FORMAT_Y210`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_Y210: Format = Format(108);
/// The header's `DXGI_FORMAT_Y216`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_Y216: Format = Format(109);
/// The header's `DXGI_FORMAT_NV11`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_NV11: Format = Format(110);
/// The header's `DXGI_FORMAT_AI44`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_AI44: Format = Format(111);
/// The header's `DXGI_FORMAT_IA44`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_IA44: Format = Format(112);
/// The header's `DXGI_FORMAT_P8`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_P8: Format = Format(113);
/// The header's `DXGI_FORMAT_A8P8`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_A8P8: Format = Format(114);
/// The header's `DXGI_FORMAT_B4G4R4A4_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_B4G4R4A4_UNORM: Format = Format(115);
/// The header's `DXGI_FORMAT_P208`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_P208: Format = Format(130);
/// The header's `DXGI_FORMAT_V208`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_V208: Format = Format(131);
/// The header's `DXGI_FORMAT_V408`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_V408: Format = Format(132);
/// The header's `DXGI_FORMAT_SAMPLER_FEEDBACK_MIN_MIP_OPAQUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_SAMPLER_FEEDBACK_MIN_MIP_OPAQUE: Format = Format(189);
/// The header's `DXGI_FORMAT_SAMPLER_FEEDBACK_MIP_REGION_USED_OPAQUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_SAMPLER_FEEDBACK_MIP_REGION_USED_OPAQUE: Format = Format(190);
/// The header's `DXGI_FORMAT_A4B4G4R4_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format).
pub const DXGI_FORMAT_A4B4G4R4_UNORM: Format = Format(191);

/// The header's `DXGI_MODE_ROTATION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ModeRotation(pub i32);

/// The header's `DXGI_MODE_ROTATION_UNSPECIFIED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MODE_ROTATION_UNSPECIFIED: ModeRotation = ModeRotation(0);
/// The header's `DXGI_MODE_ROTATION_IDENTITY`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MODE_ROTATION_IDENTITY: ModeRotation = ModeRotation(1);
/// The header's `DXGI_MODE_ROTATION_ROTATE90`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MODE_ROTATION_ROTATE90: ModeRotation = ModeRotation(2);
/// The header's `DXGI_MODE_ROTATION_ROTATE180`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MODE_ROTATION_ROTATE180: ModeRotation = ModeRotation(3);
/// The header's `DXGI_MODE_ROTATION_ROTATE270`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MODE_ROTATION_ROTATE270: ModeRotation = ModeRotation(4);

/// The header's `DXGI_MODE_SCALING`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ModeScaling(pub i32);

/// The header's `DXGI_MODE_SCALING_UNSPECIFIED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MODE_SCALING_UNSPECIFIED: ModeScaling = ModeScaling(0);
/// The header's `DXGI_MODE_SCALING_CENTERED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MODE_SCALING_CENTERED: ModeScaling = ModeScaling(1);
/// The header's `DXGI_MODE_SCALING_STRETCHED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MODE_SCALING_STRETCHED: ModeScaling = ModeScaling(2);

/// The header's `DXGI_MODE_SCANLINE_ORDER`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ModeScanlineOrder(pub i32);

/// The header's `DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED: ModeScanlineOrder = ModeScanlineOrder(0);
/// The header's `DXGI_MODE_SCANLINE_ORDER_PROGRESSIVE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MODE_SCANLINE_ORDER_PROGRESSIVE: ModeScanlineOrder = ModeScanlineOrder(1);
/// The header's `DXGI_MODE_SCANLINE_ORDER_UPPER_FIELD_FIRST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MODE_SCANLINE_ORDER_UPPER_FIELD_FIRST: ModeScanlineOrder = ModeScanlineOrder(2);
/// The header's `DXGI_MODE_SCANLINE_ORDER_LOWER_FIELD_FIRST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_MODE_SCANLINE_ORDER_LOWER_FIELD_FIRST: ModeScanlineOrder = ModeScanlineOrder(3);
