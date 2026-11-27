//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.

use wcontainers_component_object_model as component_object_model;

/// The header's `D3D11_16BIT_INDEX_STRIP_CUT_VALUE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_16BIT_INDEX_STRIP_CUT_VALUE: u32 = 0xffff;
/// The header's `D3D11_1_UAV_SLOT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_1_UAV_SLOT_COUNT: u32 = 64;
/// The header's `D3D11_2_TILED_RESOURCE_TILE_SIZE_IN_BYTES`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_2_TILED_RESOURCE_TILE_SIZE_IN_BYTES: u32 = 0x10000;
/// The header's `D3D11_32BIT_INDEX_STRIP_CUT_VALUE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_32BIT_INDEX_STRIP_CUT_VALUE: u32 = 0xffffffff;
/// The header's `D3D11_4_VIDEO_DECODER_HISTOGRAM_OFFSET_ALIGNMENT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_4_VIDEO_DECODER_HISTOGRAM_OFFSET_ALIGNMENT: u32 = 0x100;
/// The header's `D3D11_4_VIDEO_DECODER_MAX_HISTOGRAM_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_4_VIDEO_DECODER_MAX_HISTOGRAM_COMPONENTS: u32 = 4;
/// The header's `D3D11_8BIT_INDEX_STRIP_CUT_VALUE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_8BIT_INDEX_STRIP_CUT_VALUE: u32 = 255;
/// The header's `D3D11_ANISOTROPIC_FILTERING_BIT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_ANISOTROPIC_FILTERING_BIT: u32 = 64;
/// The header's `D3D11_APPEND_ALIGNED_ELEMENT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_APPEND_ALIGNED_ELEMENT: u32 = 0xffffffff;
/// The header's `D3D11_APPNAME_STRING`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_APPNAME_STRING: &str = "Name";
/// The header's `D3D11_APPSIZE_STRING`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_APPSIZE_STRING: &str = "Size";
/// The header's `D3D11_ARRAY_AXIS_ADDRESS_RANGE_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_ARRAY_AXIS_ADDRESS_RANGE_BIT_COUNT: u32 = 9;
/// The header's `D3D11_AUTHENTICATED_CONFIGURE_CRYPTO_SESSION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_AUTHENTICATED_CONFIGURE_CRYPTO_SESSION: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x6346cc54,
        data2: 0x2cfc,
        data3: 0x4ad4,
        data4: [0x82, 0x24, 0xd1, 0x58, 0x37, 0xde, 0x77, 0x00],
    };
/// The header's `D3D11_AUTHENTICATED_CONFIGURE_ENCRYPTION_WHEN_ACCESSIBLE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_AUTHENTICATED_CONFIGURE_ENCRYPTION_WHEN_ACCESSIBLE:
    component_object_model::structures::Guid = component_object_model::structures::Guid {
    data1: 0x41fff286,
    data2: 0x6ae0,
    data3: 0x4d43,
    data4: [0x9d, 0x55, 0xa4, 0x6e, 0x9e, 0xfd, 0x15, 0x8a],
};
/// The header's `D3D11_AUTHENTICATED_CONFIGURE_INITIALIZE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_AUTHENTICATED_CONFIGURE_INITIALIZE: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x06114bdb,
        data2: 0x3523,
        data3: 0x470a,
        data4: [0x8d, 0xca, 0xfb, 0xc2, 0x84, 0x51, 0x54, 0xf0],
    };
/// The header's `D3D11_AUTHENTICATED_CONFIGURE_PROTECTION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_AUTHENTICATED_CONFIGURE_PROTECTION: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x50455658,
        data2: 0x3f47,
        data3: 0x4362,
        data4: [0xbf, 0x99, 0xbf, 0xdf, 0xcd, 0xe9, 0xed, 0x29],
    };
/// The header's `D3D11_AUTHENTICATED_CONFIGURE_SHARED_RESOURCE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_AUTHENTICATED_CONFIGURE_SHARED_RESOURCE: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x0772d047,
        data2: 0x1b40,
        data3: 0x48e8,
        data4: [0x9c, 0xa6, 0xb5, 0xf5, 0x10, 0xde, 0x9f, 0x01],
    };
/// The header's `D3D11_AUTHENTICATED_QUERY_ACCESSIBILITY_ATTRIBUTES`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_AUTHENTICATED_QUERY_ACCESSIBILITY_ATTRIBUTES:
    component_object_model::structures::Guid = component_object_model::structures::Guid {
    data1: 0x6214d9d2,
    data2: 0x432c,
    data3: 0x4abb,
    data4: [0x9f, 0xce, 0x21, 0x6e, 0xea, 0x26, 0x9e, 0x3b],
};
/// The header's `D3D11_AUTHENTICATED_QUERY_CHANNEL_TYPE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_AUTHENTICATED_QUERY_CHANNEL_TYPE: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xbc1b18a5,
        data2: 0xb1fb,
        data3: 0x42ab,
        data4: [0xbd, 0x94, 0xb5, 0x82, 0x8b, 0x4b, 0xf7, 0xbe],
    };
/// The header's `D3D11_AUTHENTICATED_QUERY_CRYPTO_SESSION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_AUTHENTICATED_QUERY_CRYPTO_SESSION: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x2634499e,
        data2: 0xd018,
        data3: 0x4d74,
        data4: [0xac, 0x17, 0x7f, 0x72, 0x40, 0x59, 0x52, 0x8d],
    };
/// The header's `D3D11_AUTHENTICATED_QUERY_CURRENT_ENCRYPTION_WHEN_ACCESSIBLE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_AUTHENTICATED_QUERY_CURRENT_ENCRYPTION_WHEN_ACCESSIBLE:
    component_object_model::structures::Guid = component_object_model::structures::Guid {
    data1: 0xec1791c7,
    data2: 0xdad3,
    data3: 0x4f15,
    data4: [0x9e, 0xc3, 0xfa, 0xa9, 0x3d, 0x60, 0xd4, 0xf0],
};
/// The header's `D3D11_AUTHENTICATED_QUERY_DEVICE_HANDLE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_AUTHENTICATED_QUERY_DEVICE_HANDLE: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xec1c539d,
        data2: 0x8cff,
        data3: 0x4e2a,
        data4: [0xbc, 0xc4, 0xf5, 0x69, 0x2f, 0x99, 0xf4, 0x80],
    };
/// The header's `D3D11_AUTHENTICATED_QUERY_ENCRYPTION_WHEN_ACCESSIBLE_GUID`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_AUTHENTICATED_QUERY_ENCRYPTION_WHEN_ACCESSIBLE_GUID:
    component_object_model::structures::Guid = component_object_model::structures::Guid {
    data1: 0xf83a5958,
    data2: 0xe986,
    data3: 0x4bda,
    data4: [0xbe, 0xb0, 0x41, 0x1f, 0x6a, 0x7a, 0x01, 0xb7],
};
/// The header's `D3D11_AUTHENTICATED_QUERY_ENCRYPTION_WHEN_ACCESSIBLE_GUID_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_AUTHENTICATED_QUERY_ENCRYPTION_WHEN_ACCESSIBLE_GUID_COUNT:
    component_object_model::structures::Guid = component_object_model::structures::Guid {
    data1: 0xb30f7066,
    data2: 0x203c,
    data3: 0x4b07,
    data4: [0x93, 0xfc, 0xce, 0xaa, 0xfd, 0x61, 0x24, 0x1e],
};
/// The header's `D3D11_AUTHENTICATED_QUERY_OUTPUT_ID`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_AUTHENTICATED_QUERY_OUTPUT_ID: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x839ddca3,
        data2: 0x9b4e,
        data3: 0x41e4,
        data4: [0xb0, 0x53, 0x89, 0x2b, 0xd2, 0xa1, 0x1e, 0xe7],
    };
/// The header's `D3D11_AUTHENTICATED_QUERY_OUTPUT_ID_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_AUTHENTICATED_QUERY_OUTPUT_ID_COUNT: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x2c042b5e,
        data2: 0x8c07,
        data3: 0x46d5,
        data4: [0xaa, 0xbe, 0x8f, 0x75, 0xcb, 0xad, 0x4c, 0x31],
    };
/// The header's `D3D11_AUTHENTICATED_QUERY_PROTECTION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_AUTHENTICATED_QUERY_PROTECTION: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xa84eb584,
        data2: 0xc495,
        data3: 0x48aa,
        data4: [0xb9, 0x4d, 0x8b, 0xd2, 0xd6, 0xfb, 0xce, 0x05],
    };
/// The header's `D3D11_AUTHENTICATED_QUERY_RESTRICTED_SHARED_RESOURCE_PROCESS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_AUTHENTICATED_QUERY_RESTRICTED_SHARED_RESOURCE_PROCESS:
    component_object_model::structures::Guid = component_object_model::structures::Guid {
    data1: 0x649bbadb,
    data2: 0xf0f4,
    data3: 0x4639,
    data4: [0xa1, 0x5b, 0x24, 0x39, 0x3f, 0xc3, 0xab, 0xac],
};
/// The header's `D3D11_AUTHENTICATED_QUERY_RESTRICTED_SHARED_RESOURCE_PROCESS_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_AUTHENTICATED_QUERY_RESTRICTED_SHARED_RESOURCE_PROCESS_COUNT:
    component_object_model::structures::Guid = component_object_model::structures::Guid {
    data1: 0x0db207b3,
    data2: 0x9450,
    data3: 0x46a6,
    data4: [0x82, 0xde, 0x1b, 0x96, 0xd4, 0x4f, 0x9c, 0xf2],
};
/// The header's `D3D11_AUTHENTICATED_QUERY_UNRESTRICTED_PROTECTED_SHARED_RESOURCE_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_AUTHENTICATED_QUERY_UNRESTRICTED_PROTECTED_SHARED_RESOURCE_COUNT:
    component_object_model::structures::Guid = component_object_model::structures::Guid {
    data1: 0x012f0bd6,
    data2: 0xe662,
    data3: 0x4474,
    data4: [0xbe, 0xfd, 0xaa, 0x53, 0xe5, 0x14, 0x3c, 0x6d],
};
/// The header's `D3D11_BREAKON_CATEGORY`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_BREAKON_CATEGORY: &str = "BreakOn_CATEGORY_%s";
/// The header's `D3D11_BREAKON_ID_DECIMAL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_BREAKON_ID_DECIMAL: &str = "BreakOn_ID_%d";
/// The header's `D3D11_BREAKON_ID_STRING`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_BREAKON_ID_STRING: &str = "BreakOn_ID_%s";
/// The header's `D3D11_BREAKON_SEVERITY`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_BREAKON_SEVERITY: &str = "BreakOn_SEVERITY_%s";
/// The header's `D3D11_CLIP_OR_CULL_DISTANCE_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CLIP_OR_CULL_DISTANCE_COUNT: u32 = 8;
/// The header's `D3D11_CLIP_OR_CULL_DISTANCE_ELEMENT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CLIP_OR_CULL_DISTANCE_ELEMENT_COUNT: u32 = 2;
/// The header's `D3D11_COMMONSHADER_CONSTANT_BUFFER_API_SLOT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_CONSTANT_BUFFER_API_SLOT_COUNT: u32 = 14;
/// The header's `D3D11_COMMONSHADER_CONSTANT_BUFFER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_CONSTANT_BUFFER_COMPONENTS: u32 = 4;
/// The header's `D3D11_COMMONSHADER_CONSTANT_BUFFER_COMPONENT_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_CONSTANT_BUFFER_COMPONENT_BIT_COUNT: u32 = 32;
/// The header's `D3D11_COMMONSHADER_CONSTANT_BUFFER_HW_SLOT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_CONSTANT_BUFFER_HW_SLOT_COUNT: u32 = 15;
/// The header's `D3D11_COMMONSHADER_CONSTANT_BUFFER_PARTIAL_UPDATE_EXTENTS_BYTE_ALIGNMENT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_CONSTANT_BUFFER_PARTIAL_UPDATE_EXTENTS_BYTE_ALIGNMENT: u32 = 16;
/// The header's `D3D11_COMMONSHADER_CONSTANT_BUFFER_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_CONSTANT_BUFFER_REGISTER_COMPONENTS: u32 = 4;
/// The header's `D3D11_COMMONSHADER_CONSTANT_BUFFER_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_CONSTANT_BUFFER_REGISTER_COUNT: u32 = 15;
/// The header's `D3D11_COMMONSHADER_CONSTANT_BUFFER_REGISTER_READS_PER_INST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_CONSTANT_BUFFER_REGISTER_READS_PER_INST: u32 = 1;
/// The header's `D3D11_COMMONSHADER_CONSTANT_BUFFER_REGISTER_READ_PORTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_CONSTANT_BUFFER_REGISTER_READ_PORTS: u32 = 1;
/// The header's `D3D11_COMMONSHADER_FLOWCONTROL_NESTING_LIMIT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_FLOWCONTROL_NESTING_LIMIT: u32 = 64;
/// The header's `D3D11_COMMONSHADER_IMMEDIATE_CONSTANT_BUFFER_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_IMMEDIATE_CONSTANT_BUFFER_REGISTER_COMPONENTS: u32 = 4;
/// The header's `D3D11_COMMONSHADER_IMMEDIATE_CONSTANT_BUFFER_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_IMMEDIATE_CONSTANT_BUFFER_REGISTER_COUNT: u32 = 1;
/// The header's `D3D11_COMMONSHADER_IMMEDIATE_CONSTANT_BUFFER_REGISTER_READS_PER_INST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_IMMEDIATE_CONSTANT_BUFFER_REGISTER_READS_PER_INST: u32 = 1;
/// The header's `D3D11_COMMONSHADER_IMMEDIATE_CONSTANT_BUFFER_REGISTER_READ_PORTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_IMMEDIATE_CONSTANT_BUFFER_REGISTER_READ_PORTS: u32 = 1;
/// The header's `D3D11_COMMONSHADER_IMMEDIATE_VALUE_COMPONENT_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_IMMEDIATE_VALUE_COMPONENT_BIT_COUNT: u32 = 32;
/// The header's `D3D11_COMMONSHADER_INPUT_RESOURCE_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_INPUT_RESOURCE_REGISTER_COMPONENTS: u32 = 1;
/// The header's `D3D11_COMMONSHADER_INPUT_RESOURCE_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_INPUT_RESOURCE_REGISTER_COUNT: u32 = 128;
/// The header's `D3D11_COMMONSHADER_INPUT_RESOURCE_REGISTER_READS_PER_INST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_INPUT_RESOURCE_REGISTER_READS_PER_INST: u32 = 1;
/// The header's `D3D11_COMMONSHADER_INPUT_RESOURCE_REGISTER_READ_PORTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_INPUT_RESOURCE_REGISTER_READ_PORTS: u32 = 1;
/// The header's `D3D11_COMMONSHADER_INPUT_RESOURCE_SLOT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_INPUT_RESOURCE_SLOT_COUNT: u32 = 128;
/// The header's `D3D11_COMMONSHADER_SAMPLER_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_SAMPLER_REGISTER_COMPONENTS: u32 = 1;
/// The header's `D3D11_COMMONSHADER_SAMPLER_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_SAMPLER_REGISTER_COUNT: u32 = 16;
/// The header's `D3D11_COMMONSHADER_SAMPLER_REGISTER_READS_PER_INST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_SAMPLER_REGISTER_READS_PER_INST: u32 = 1;
/// The header's `D3D11_COMMONSHADER_SAMPLER_REGISTER_READ_PORTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_SAMPLER_REGISTER_READ_PORTS: u32 = 1;
/// The header's `D3D11_COMMONSHADER_SAMPLER_SLOT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_SAMPLER_SLOT_COUNT: u32 = 16;
/// The header's `D3D11_COMMONSHADER_SUBROUTINE_NESTING_LIMIT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_SUBROUTINE_NESTING_LIMIT: u32 = 32;
/// The header's `D3D11_COMMONSHADER_TEMP_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_TEMP_REGISTER_COMPONENTS: u32 = 4;
/// The header's `D3D11_COMMONSHADER_TEMP_REGISTER_COMPONENT_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_TEMP_REGISTER_COMPONENT_BIT_COUNT: u32 = 32;
/// The header's `D3D11_COMMONSHADER_TEMP_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_TEMP_REGISTER_COUNT: u32 = 0x1000;
/// The header's `D3D11_COMMONSHADER_TEMP_REGISTER_READS_PER_INST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_TEMP_REGISTER_READS_PER_INST: u32 = 3;
/// The header's `D3D11_COMMONSHADER_TEMP_REGISTER_READ_PORTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_TEMP_REGISTER_READ_PORTS: u32 = 3;
/// The header's `D3D11_COMMONSHADER_TEXCOORD_RANGE_REDUCTION_MAX`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_TEXCOORD_RANGE_REDUCTION_MAX: u32 = 10;
/// The header's `D3D11_COMMONSHADER_TEXCOORD_RANGE_REDUCTION_MIN`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_TEXCOORD_RANGE_REDUCTION_MIN: i32 = -10;
/// The header's `D3D11_COMMONSHADER_TEXEL_OFFSET_MAX_NEGATIVE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_TEXEL_OFFSET_MAX_NEGATIVE: i32 = -8;
/// The header's `D3D11_COMMONSHADER_TEXEL_OFFSET_MAX_POSITIVE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMMONSHADER_TEXEL_OFFSET_MAX_POSITIVE: u32 = 7;
/// The header's `D3D11_COMPARISON_FILTERING_BIT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMPARISON_FILTERING_BIT: u32 = 128;
/// The header's `D3D11_CRYPTO_TYPE_AES128_CTR`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CRYPTO_TYPE_AES128_CTR: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x9b6bd711,
        data2: 0x4f74,
        data3: 0x41c9,
        data4: [0x9e, 0x7b, 0x0b, 0xe2, 0xd7, 0xd9, 0x3b, 0x4f],
    };
/// The header's `D3D11_CS_4_X_BUCKET00_MAX_BYTES_TGSM_WRITABLE_PER_THREAD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET00_MAX_BYTES_TGSM_WRITABLE_PER_THREAD: u32 = 0x100;
/// The header's `D3D11_CS_4_X_BUCKET00_MAX_NUM_THREADS_PER_GROUP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET00_MAX_NUM_THREADS_PER_GROUP: u32 = 64;
/// The header's `D3D11_CS_4_X_BUCKET01_MAX_BYTES_TGSM_WRITABLE_PER_THREAD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET01_MAX_BYTES_TGSM_WRITABLE_PER_THREAD: u32 = 240;
/// The header's `D3D11_CS_4_X_BUCKET01_MAX_NUM_THREADS_PER_GROUP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET01_MAX_NUM_THREADS_PER_GROUP: u32 = 68;
/// The header's `D3D11_CS_4_X_BUCKET02_MAX_BYTES_TGSM_WRITABLE_PER_THREAD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET02_MAX_BYTES_TGSM_WRITABLE_PER_THREAD: u32 = 224;
/// The header's `D3D11_CS_4_X_BUCKET02_MAX_NUM_THREADS_PER_GROUP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET02_MAX_NUM_THREADS_PER_GROUP: u32 = 72;
/// The header's `D3D11_CS_4_X_BUCKET03_MAX_BYTES_TGSM_WRITABLE_PER_THREAD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET03_MAX_BYTES_TGSM_WRITABLE_PER_THREAD: u32 = 208;
/// The header's `D3D11_CS_4_X_BUCKET03_MAX_NUM_THREADS_PER_GROUP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET03_MAX_NUM_THREADS_PER_GROUP: u32 = 76;
/// The header's `D3D11_CS_4_X_BUCKET04_MAX_BYTES_TGSM_WRITABLE_PER_THREAD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET04_MAX_BYTES_TGSM_WRITABLE_PER_THREAD: u32 = 192;
/// The header's `D3D11_CS_4_X_BUCKET04_MAX_NUM_THREADS_PER_GROUP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET04_MAX_NUM_THREADS_PER_GROUP: u32 = 84;
/// The header's `D3D11_CS_4_X_BUCKET05_MAX_BYTES_TGSM_WRITABLE_PER_THREAD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET05_MAX_BYTES_TGSM_WRITABLE_PER_THREAD: u32 = 176;
/// The header's `D3D11_CS_4_X_BUCKET05_MAX_NUM_THREADS_PER_GROUP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET05_MAX_NUM_THREADS_PER_GROUP: u32 = 92;
/// The header's `D3D11_CS_4_X_BUCKET06_MAX_BYTES_TGSM_WRITABLE_PER_THREAD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET06_MAX_BYTES_TGSM_WRITABLE_PER_THREAD: u32 = 160;
/// The header's `D3D11_CS_4_X_BUCKET06_MAX_NUM_THREADS_PER_GROUP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET06_MAX_NUM_THREADS_PER_GROUP: u32 = 100;
/// The header's `D3D11_CS_4_X_BUCKET07_MAX_BYTES_TGSM_WRITABLE_PER_THREAD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET07_MAX_BYTES_TGSM_WRITABLE_PER_THREAD: u32 = 144;
/// The header's `D3D11_CS_4_X_BUCKET07_MAX_NUM_THREADS_PER_GROUP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET07_MAX_NUM_THREADS_PER_GROUP: u32 = 112;
/// The header's `D3D11_CS_4_X_BUCKET08_MAX_BYTES_TGSM_WRITABLE_PER_THREAD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET08_MAX_BYTES_TGSM_WRITABLE_PER_THREAD: u32 = 128;
/// The header's `D3D11_CS_4_X_BUCKET08_MAX_NUM_THREADS_PER_GROUP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET08_MAX_NUM_THREADS_PER_GROUP: u32 = 128;
/// The header's `D3D11_CS_4_X_BUCKET09_MAX_BYTES_TGSM_WRITABLE_PER_THREAD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET09_MAX_BYTES_TGSM_WRITABLE_PER_THREAD: u32 = 112;
/// The header's `D3D11_CS_4_X_BUCKET09_MAX_NUM_THREADS_PER_GROUP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET09_MAX_NUM_THREADS_PER_GROUP: u32 = 144;
/// The header's `D3D11_CS_4_X_BUCKET10_MAX_BYTES_TGSM_WRITABLE_PER_THREAD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET10_MAX_BYTES_TGSM_WRITABLE_PER_THREAD: u32 = 96;
/// The header's `D3D11_CS_4_X_BUCKET10_MAX_NUM_THREADS_PER_GROUP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET10_MAX_NUM_THREADS_PER_GROUP: u32 = 168;
/// The header's `D3D11_CS_4_X_BUCKET11_MAX_BYTES_TGSM_WRITABLE_PER_THREAD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET11_MAX_BYTES_TGSM_WRITABLE_PER_THREAD: u32 = 80;
/// The header's `D3D11_CS_4_X_BUCKET11_MAX_NUM_THREADS_PER_GROUP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET11_MAX_NUM_THREADS_PER_GROUP: u32 = 204;
/// The header's `D3D11_CS_4_X_BUCKET12_MAX_BYTES_TGSM_WRITABLE_PER_THREAD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET12_MAX_BYTES_TGSM_WRITABLE_PER_THREAD: u32 = 64;
/// The header's `D3D11_CS_4_X_BUCKET12_MAX_NUM_THREADS_PER_GROUP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET12_MAX_NUM_THREADS_PER_GROUP: u32 = 0x100;
/// The header's `D3D11_CS_4_X_BUCKET13_MAX_BYTES_TGSM_WRITABLE_PER_THREAD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET13_MAX_BYTES_TGSM_WRITABLE_PER_THREAD: u32 = 48;
/// The header's `D3D11_CS_4_X_BUCKET13_MAX_NUM_THREADS_PER_GROUP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET13_MAX_NUM_THREADS_PER_GROUP: u32 = 0x154;
/// The header's `D3D11_CS_4_X_BUCKET14_MAX_BYTES_TGSM_WRITABLE_PER_THREAD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET14_MAX_BYTES_TGSM_WRITABLE_PER_THREAD: u32 = 32;
/// The header's `D3D11_CS_4_X_BUCKET14_MAX_NUM_THREADS_PER_GROUP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET14_MAX_NUM_THREADS_PER_GROUP: u32 = 0x200;
/// The header's `D3D11_CS_4_X_BUCKET15_MAX_BYTES_TGSM_WRITABLE_PER_THREAD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET15_MAX_BYTES_TGSM_WRITABLE_PER_THREAD: u32 = 16;
/// The header's `D3D11_CS_4_X_BUCKET15_MAX_NUM_THREADS_PER_GROUP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_BUCKET15_MAX_NUM_THREADS_PER_GROUP: u32 = 0x300;
/// The header's `D3D11_CS_4_X_DISPATCH_MAX_THREAD_GROUPS_IN_Z_DIMENSION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_DISPATCH_MAX_THREAD_GROUPS_IN_Z_DIMENSION: u32 = 1;
/// The header's `D3D11_CS_4_X_RAW_UAV_BYTE_ALIGNMENT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_RAW_UAV_BYTE_ALIGNMENT: u32 = 0x100;
/// The header's `D3D11_CS_4_X_THREAD_GROUP_MAX_THREADS_PER_GROUP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_THREAD_GROUP_MAX_THREADS_PER_GROUP: u32 = 0x300;
/// The header's `D3D11_CS_4_X_THREAD_GROUP_MAX_X`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_THREAD_GROUP_MAX_X: u32 = 0x300;
/// The header's `D3D11_CS_4_X_THREAD_GROUP_MAX_Y`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_THREAD_GROUP_MAX_Y: u32 = 0x300;
/// The header's `D3D11_CS_4_X_UAV_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_4_X_UAV_REGISTER_COUNT: u32 = 1;
/// The header's `D3D11_CS_DISPATCH_MAX_THREAD_GROUPS_PER_DIMENSION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_DISPATCH_MAX_THREAD_GROUPS_PER_DIMENSION: u32 = 0xffff;
/// The header's `D3D11_CS_TGSM_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_TGSM_REGISTER_COUNT: u32 = 0x2000;
/// The header's `D3D11_CS_TGSM_REGISTER_READS_PER_INST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_TGSM_REGISTER_READS_PER_INST: u32 = 1;
/// The header's `D3D11_CS_TGSM_RESOURCE_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_TGSM_RESOURCE_REGISTER_COMPONENTS: u32 = 1;
/// The header's `D3D11_CS_TGSM_RESOURCE_REGISTER_READ_PORTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_TGSM_RESOURCE_REGISTER_READ_PORTS: u32 = 1;
/// The header's `D3D11_CS_THREADGROUPID_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_THREADGROUPID_REGISTER_COMPONENTS: u32 = 3;
/// The header's `D3D11_CS_THREADGROUPID_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_THREADGROUPID_REGISTER_COUNT: u32 = 1;
/// The header's `D3D11_CS_THREADIDINGROUPFLATTENED_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_THREADIDINGROUPFLATTENED_REGISTER_COMPONENTS: u32 = 1;
/// The header's `D3D11_CS_THREADIDINGROUPFLATTENED_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_THREADIDINGROUPFLATTENED_REGISTER_COUNT: u32 = 1;
/// The header's `D3D11_CS_THREADIDINGROUP_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_THREADIDINGROUP_REGISTER_COMPONENTS: u32 = 3;
/// The header's `D3D11_CS_THREADIDINGROUP_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_THREADIDINGROUP_REGISTER_COUNT: u32 = 1;
/// The header's `D3D11_CS_THREADID_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_THREADID_REGISTER_COMPONENTS: u32 = 3;
/// The header's `D3D11_CS_THREADID_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_THREADID_REGISTER_COUNT: u32 = 1;
/// The header's `D3D11_CS_THREAD_GROUP_MAX_THREADS_PER_GROUP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_THREAD_GROUP_MAX_THREADS_PER_GROUP: u32 = 0x400;
/// The header's `D3D11_CS_THREAD_GROUP_MAX_X`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_THREAD_GROUP_MAX_X: u32 = 0x400;
/// The header's `D3D11_CS_THREAD_GROUP_MAX_Y`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_THREAD_GROUP_MAX_Y: u32 = 0x400;
/// The header's `D3D11_CS_THREAD_GROUP_MAX_Z`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_THREAD_GROUP_MAX_Z: u32 = 64;
/// The header's `D3D11_CS_THREAD_GROUP_MIN_X`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_THREAD_GROUP_MIN_X: u32 = 1;
/// The header's `D3D11_CS_THREAD_GROUP_MIN_Y`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_THREAD_GROUP_MIN_Y: u32 = 1;
/// The header's `D3D11_CS_THREAD_GROUP_MIN_Z`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_THREAD_GROUP_MIN_Z: u32 = 1;
/// The header's `D3D11_CS_THREAD_LOCAL_TEMP_REGISTER_POOL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CS_THREAD_LOCAL_TEMP_REGISTER_POOL: u32 = 0x4000;
/// The header's `D3D11_DEBUG_FEATURE_ALWAYS_DISCARD_OFFERED_RESOURCE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEBUG_FEATURE_ALWAYS_DISCARD_OFFERED_RESOURCE: u32 = 8;
/// The header's `D3D11_DEBUG_FEATURE_AVOID_BEHAVIOR_CHANGING_DEBUG_AIDS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEBUG_FEATURE_AVOID_BEHAVIOR_CHANGING_DEBUG_AIDS: u32 = 64;
/// The header's `D3D11_DEBUG_FEATURE_DISABLE_TILED_RESOURCE_MAPPING_TRACKING_AND_VALIDATION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEBUG_FEATURE_DISABLE_TILED_RESOURCE_MAPPING_TRACKING_AND_VALIDATION: u32 = 128;
/// The header's `D3D11_DEBUG_FEATURE_FINISH_PER_RENDER_OP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEBUG_FEATURE_FINISH_PER_RENDER_OP: u32 = 2;
/// The header's `D3D11_DEBUG_FEATURE_FLUSH_PER_RENDER_OP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEBUG_FEATURE_FLUSH_PER_RENDER_OP: u32 = 1;
/// The header's `D3D11_DEBUG_FEATURE_NEVER_DISCARD_OFFERED_RESOURCE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEBUG_FEATURE_NEVER_DISCARD_OFFERED_RESOURCE: u32 = 16;
/// The header's `D3D11_DEBUG_FEATURE_PRESENT_PER_RENDER_OP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEBUG_FEATURE_PRESENT_PER_RENDER_OP: u32 = 4;
/// The header's `D3D11_DECODER_BITSTREAM_ENCRYPTION_TYPE_CBCS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_BITSTREAM_ENCRYPTION_TYPE_CBCS: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x422d9319,
        data2: 0x9d21,
        data3: 0x4bb7,
        data4: [0x93, 0x71, 0xfa, 0xf5, 0xa8, 0x2c, 0x3e, 0x04],
    };
/// The header's `D3D11_DECODER_BITSTREAM_ENCRYPTION_TYPE_CENC`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_BITSTREAM_ENCRYPTION_TYPE_CENC: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xb0405235,
        data2: 0xc13d,
        data3: 0x44f2,
        data4: [0x9a, 0xe5, 0xdd, 0x48, 0xe0, 0x8e, 0x5b, 0x67],
    };
/// The header's `D3D11_DECODER_ENCRYPTION_HW_CENC`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_ENCRYPTION_HW_CENC: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x89d6ac4f,
        data2: 0x09f2,
        data3: 0x4229,
        data4: [0xb2, 0xcd, 0x37, 0x74, 0x0a, 0x6d, 0xfd, 0x81],
    };
/// The header's `D3D11_DECODER_PROFILE_AV1_VLD_12BIT_PROFILE2`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_AV1_VLD_12BIT_PROFILE2: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x17127009,
        data2: 0xa00f,
        data3: 0x4ce1,
        data4: [0x99, 0x4e, 0xbf, 0x40, 0x81, 0xf6, 0xf3, 0xf0],
    };
/// The header's `D3D11_DECODER_PROFILE_AV1_VLD_12BIT_PROFILE2_420`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_AV1_VLD_12BIT_PROFILE2_420:
    component_object_model::structures::Guid = component_object_model::structures::Guid {
    data1: 0x2d80bed6,
    data2: 0x9cac,
    data3: 0x4835,
    data4: [0x9e, 0x91, 0x32, 0x7b, 0xbc, 0x4f, 0x9e, 0xe8],
};
/// The header's `D3D11_DECODER_PROFILE_AV1_VLD_PROFILE0`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_AV1_VLD_PROFILE0: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xb8be4ccb,
        data2: 0xcf53,
        data3: 0x46ba,
        data4: [0x8d, 0x59, 0xd6, 0xb8, 0xa6, 0xda, 0x5d, 0x2a],
    };
/// The header's `D3D11_DECODER_PROFILE_AV1_VLD_PROFILE1`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_AV1_VLD_PROFILE1: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x6936ff0f,
        data2: 0x45b1,
        data3: 0x4163,
        data4: [0x9c, 0xc1, 0x64, 0x6e, 0xf6, 0x94, 0x61, 0x08],
    };
/// The header's `D3D11_DECODER_PROFILE_AV1_VLD_PROFILE2`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_AV1_VLD_PROFILE2: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x0c5f2aa1,
        data2: 0xe541,
        data3: 0x4089,
        data4: [0xbb, 0x7b, 0x98, 0x11, 0x0a, 0x19, 0xd7, 0xc8],
    };
/// The header's `D3D11_DECODER_PROFILE_H264_IDCT_FGT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_H264_IDCT_FGT: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x1b81be67,
        data2: 0xa0c7,
        data3: 0x11d3,
        data4: [0xb9, 0x84, 0x00, 0xc0, 0x4f, 0x2e, 0x73, 0xc5],
    };
/// The header's `D3D11_DECODER_PROFILE_H264_IDCT_NOFGT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_H264_IDCT_NOFGT: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x1b81be66,
        data2: 0xa0c7,
        data3: 0x11d3,
        data4: [0xb9, 0x84, 0x00, 0xc0, 0x4f, 0x2e, 0x73, 0xc5],
    };
/// The header's `D3D11_DECODER_PROFILE_H264_MOCOMP_FGT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_H264_MOCOMP_FGT: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x1b81be65,
        data2: 0xa0c7,
        data3: 0x11d3,
        data4: [0xb9, 0x84, 0x00, 0xc0, 0x4f, 0x2e, 0x73, 0xc5],
    };
/// The header's `D3D11_DECODER_PROFILE_H264_MOCOMP_NOFGT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_H264_MOCOMP_NOFGT: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x1b81be64,
        data2: 0xa0c7,
        data3: 0x11d3,
        data4: [0xb9, 0x84, 0x00, 0xc0, 0x4f, 0x2e, 0x73, 0xc5],
    };
/// The header's `D3D11_DECODER_PROFILE_H264_VLD_FGT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_H264_VLD_FGT: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x1b81be69,
        data2: 0xa0c7,
        data3: 0x11d3,
        data4: [0xb9, 0x84, 0x00, 0xc0, 0x4f, 0x2e, 0x73, 0xc5],
    };
/// The header's `D3D11_DECODER_PROFILE_H264_VLD_MULTIVIEW_NOFGT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_H264_VLD_MULTIVIEW_NOFGT: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x705b9d82,
        data2: 0x76cf,
        data3: 0x49d6,
        data4: [0xb7, 0xe6, 0xac, 0x88, 0x72, 0xdb, 0x01, 0x3c],
    };
/// The header's `D3D11_DECODER_PROFILE_H264_VLD_NOFGT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_H264_VLD_NOFGT: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x1b81be68,
        data2: 0xa0c7,
        data3: 0x11d3,
        data4: [0xb9, 0x84, 0x00, 0xc0, 0x4f, 0x2e, 0x73, 0xc5],
    };
/// The header's `D3D11_DECODER_PROFILE_H264_VLD_STEREO_NOFGT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_H264_VLD_STEREO_NOFGT: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xf9aaccbb,
        data2: 0xc2b6,
        data3: 0x4cfc,
        data4: [0x87, 0x79, 0x57, 0x07, 0xb1, 0x76, 0x05, 0x52],
    };
/// The header's `D3D11_DECODER_PROFILE_H264_VLD_STEREO_PROGRESSIVE_NOFGT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_H264_VLD_STEREO_PROGRESSIVE_NOFGT:
    component_object_model::structures::Guid = component_object_model::structures::Guid {
    data1: 0xd79be8da,
    data2: 0x0cf1,
    data3: 0x4c81,
    data4: [0xb8, 0x2a, 0x69, 0xa4, 0xe2, 0x36, 0xf4, 0x3d],
};
/// The header's `D3D11_DECODER_PROFILE_H264_VLD_WITHFMOASO_NOFGT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_H264_VLD_WITHFMOASO_NOFGT:
    component_object_model::structures::Guid = component_object_model::structures::Guid {
    data1: 0xd5f04ff9,
    data2: 0x3418,
    data3: 0x45d8,
    data4: [0x95, 0x61, 0x32, 0xa7, 0x6a, 0xae, 0x2d, 0xdd],
};
/// The header's `D3D11_DECODER_PROFILE_HEVC_VLD_MAIN`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_HEVC_VLD_MAIN: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x5b11d51b,
        data2: 0x2f4c,
        data3: 0x4452,
        data4: [0xbc, 0xc3, 0x09, 0xf2, 0xa1, 0x16, 0x0c, 0xc0],
    };
/// The header's `D3D11_DECODER_PROFILE_HEVC_VLD_MAIN10`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_HEVC_VLD_MAIN10: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x107af0e0,
        data2: 0xef1a,
        data3: 0x4d19,
        data4: [0xab, 0xa8, 0x67, 0xa1, 0x63, 0x07, 0x3d, 0x13],
    };
/// The header's `D3D11_DECODER_PROFILE_MPEG1_VLD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_MPEG1_VLD: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x6f3ec719,
        data2: 0x3735,
        data3: 0x42cc,
        data4: [0x80, 0x63, 0x65, 0xcc, 0x3c, 0xb3, 0x66, 0x16],
    };
/// The header's `D3D11_DECODER_PROFILE_MPEG2_IDCT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_MPEG2_IDCT: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xbf22ad00,
        data2: 0x03ea,
        data3: 0x4690,
        data4: [0x80, 0x77, 0x47, 0x33, 0x46, 0x20, 0x9b, 0x7e],
    };
/// The header's `D3D11_DECODER_PROFILE_MPEG2_MOCOMP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_MPEG2_MOCOMP: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xe6a9f44b,
        data2: 0x61b0,
        data3: 0x4563,
        data4: [0x9e, 0xa4, 0x63, 0xd2, 0xa3, 0xc6, 0xfe, 0x66],
    };
/// The header's `D3D11_DECODER_PROFILE_MPEG2_VLD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_MPEG2_VLD: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xee27417f,
        data2: 0x5e28,
        data3: 0x4e65,
        data4: [0xbe, 0xea, 0x1d, 0x26, 0xb5, 0x08, 0xad, 0xc9],
    };
/// The header's `D3D11_DECODER_PROFILE_MPEG2and1_VLD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_MPEG2AND1_VLD: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x86695f12,
        data2: 0x340e,
        data3: 0x4f04,
        data4: [0x9f, 0xd3, 0x92, 0x53, 0xdd, 0x32, 0x74, 0x60],
    };
/// The header's `D3D11_DECODER_PROFILE_MPEG4PT2_VLD_ADVSIMPLE_GMC`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_MPEG4PT2_VLD_ADVSIMPLE_GMC:
    component_object_model::structures::Guid = component_object_model::structures::Guid {
    data1: 0xab998b5b,
    data2: 0x4258,
    data3: 0x44a9,
    data4: [0x9f, 0xeb, 0x94, 0xe5, 0x97, 0xa6, 0xba, 0xae],
};
/// The header's `D3D11_DECODER_PROFILE_MPEG4PT2_VLD_ADVSIMPLE_NOGMC`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_MPEG4PT2_VLD_ADVSIMPLE_NOGMC:
    component_object_model::structures::Guid = component_object_model::structures::Guid {
    data1: 0xed418a9f,
    data2: 0x010d,
    data3: 0x4eda,
    data4: [0x9a, 0xe3, 0x9a, 0x65, 0x35, 0x8d, 0x8d, 0x2e],
};
/// The header's `D3D11_DECODER_PROFILE_MPEG4PT2_VLD_SIMPLE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_MPEG4PT2_VLD_SIMPLE: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xefd64d74,
        data2: 0xc9e8,
        data3: 0x41d7,
        data4: [0xa5, 0xe9, 0xe9, 0xb0, 0xe3, 0x9f, 0xa3, 0x19],
    };
/// The header's `D3D11_DECODER_PROFILE_VC1_D2010`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_VC1_D2010: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x1b81bea4,
        data2: 0xa0c7,
        data3: 0x11d3,
        data4: [0xb9, 0x84, 0x00, 0xc0, 0x4f, 0x2e, 0x73, 0xc5],
    };
/// The header's `D3D11_DECODER_PROFILE_VC1_IDCT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_VC1_IDCT: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x1b81bea2,
        data2: 0xa0c7,
        data3: 0x11d3,
        data4: [0xb9, 0x84, 0x00, 0xc0, 0x4f, 0x2e, 0x73, 0xc5],
    };
/// The header's `D3D11_DECODER_PROFILE_VC1_MOCOMP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_VC1_MOCOMP: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x1b81bea1,
        data2: 0xa0c7,
        data3: 0x11d3,
        data4: [0xb9, 0x84, 0x00, 0xc0, 0x4f, 0x2e, 0x73, 0xc5],
    };
/// The header's `D3D11_DECODER_PROFILE_VC1_POSTPROC`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_VC1_POSTPROC: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x1b81bea0,
        data2: 0xa0c7,
        data3: 0x11d3,
        data4: [0xb9, 0x84, 0x00, 0xc0, 0x4f, 0x2e, 0x73, 0xc5],
    };
/// The header's `D3D11_DECODER_PROFILE_VC1_VLD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_VC1_VLD: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x1b81bea3,
        data2: 0xa0c7,
        data3: 0x11d3,
        data4: [0xb9, 0x84, 0x00, 0xc0, 0x4f, 0x2e, 0x73, 0xc5],
    };
/// The header's `D3D11_DECODER_PROFILE_VP8_VLD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_VP8_VLD: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x90b899ea,
        data2: 0x3a62,
        data3: 0x4705,
        data4: [0x88, 0xb3, 0x8d, 0xf0, 0x4b, 0x27, 0x44, 0xe7],
    };
/// The header's `D3D11_DECODER_PROFILE_VP9_VLD_10BIT_PROFILE2`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_VP9_VLD_10BIT_PROFILE2: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xa4c749ef,
        data2: 0x6ecf,
        data3: 0x48aa,
        data4: [0x84, 0x48, 0x50, 0xa7, 0xa1, 0x16, 0x5f, 0xf7],
    };
/// The header's `D3D11_DECODER_PROFILE_VP9_VLD_PROFILE0`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_VP9_VLD_PROFILE0: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x463707f8,
        data2: 0xa1d0,
        data3: 0x4585,
        data4: [0x87, 0x6d, 0x83, 0xaa, 0x6d, 0x60, 0xb8, 0x9e],
    };
/// The header's `D3D11_DECODER_PROFILE_WMV8_MOCOMP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_WMV8_MOCOMP: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x1b81be81,
        data2: 0xa0c7,
        data3: 0x11d3,
        data4: [0xb9, 0x84, 0x00, 0xc0, 0x4f, 0x2e, 0x73, 0xc5],
    };
/// The header's `D3D11_DECODER_PROFILE_WMV8_POSTPROC`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_WMV8_POSTPROC: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x1b81be80,
        data2: 0xa0c7,
        data3: 0x11d3,
        data4: [0xb9, 0x84, 0x00, 0xc0, 0x4f, 0x2e, 0x73, 0xc5],
    };
/// The header's `D3D11_DECODER_PROFILE_WMV9_IDCT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_WMV9_IDCT: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x1b81be94,
        data2: 0xa0c7,
        data3: 0x11d3,
        data4: [0xb9, 0x84, 0x00, 0xc0, 0x4f, 0x2e, 0x73, 0xc5],
    };
/// The header's `D3D11_DECODER_PROFILE_WMV9_MOCOMP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_WMV9_MOCOMP: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x1b81be91,
        data2: 0xa0c7,
        data3: 0x11d3,
        data4: [0xb9, 0x84, 0x00, 0xc0, 0x4f, 0x2e, 0x73, 0xc5],
    };
/// The header's `D3D11_DECODER_PROFILE_WMV9_POSTPROC`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DECODER_PROFILE_WMV9_POSTPROC: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x1b81be90,
        data2: 0xa0c7,
        data3: 0x11d3,
        data4: [0xb9, 0x84, 0x00, 0xc0, 0x4f, 0x2e, 0x73, 0xc5],
    };
/// The header's `D3D11_DEFAULT_BLEND_FACTOR_ALPHA`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEFAULT_BLEND_FACTOR_ALPHA: f32 = 1.0_f32;
/// The header's `D3D11_DEFAULT_BLEND_FACTOR_BLUE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEFAULT_BLEND_FACTOR_BLUE: f32 = 1.0_f32;
/// The header's `D3D11_DEFAULT_BLEND_FACTOR_GREEN`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEFAULT_BLEND_FACTOR_GREEN: f32 = 1.0_f32;
/// The header's `D3D11_DEFAULT_BLEND_FACTOR_RED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEFAULT_BLEND_FACTOR_RED: f32 = 1.0_f32;
/// The header's `D3D11_DEFAULT_BORDER_COLOR_COMPONENT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEFAULT_BORDER_COLOR_COMPONENT: f32 = 0.0_f32;
/// The header's `D3D11_DEFAULT_DEPTH_BIAS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEFAULT_DEPTH_BIAS: u32 = 0;
/// The header's `D3D11_DEFAULT_DEPTH_BIAS_CLAMP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEFAULT_DEPTH_BIAS_CLAMP: f32 = 0.0_f32;
/// The header's `D3D11_DEFAULT_MAX_ANISOTROPY`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEFAULT_MAX_ANISOTROPY: u32 = 16;
/// The header's `D3D11_DEFAULT_MIP_LOD_BIAS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEFAULT_MIP_LOD_BIAS: f32 = 0.0_f32;
/// The header's `D3D11_DEFAULT_RENDER_TARGET_ARRAY_INDEX`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEFAULT_RENDER_TARGET_ARRAY_INDEX: u32 = 0;
/// The header's `D3D11_DEFAULT_SAMPLE_MASK`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEFAULT_SAMPLE_MASK: u32 = 0xffffffff;
/// The header's `D3D11_DEFAULT_SCISSOR_ENDX`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEFAULT_SCISSOR_ENDX: u32 = 0;
/// The header's `D3D11_DEFAULT_SCISSOR_ENDY`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEFAULT_SCISSOR_ENDY: u32 = 0;
/// The header's `D3D11_DEFAULT_SCISSOR_STARTX`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEFAULT_SCISSOR_STARTX: u32 = 0;
/// The header's `D3D11_DEFAULT_SCISSOR_STARTY`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEFAULT_SCISSOR_STARTY: u32 = 0;
/// The header's `D3D11_DEFAULT_SLOPE_SCALED_DEPTH_BIAS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEFAULT_SLOPE_SCALED_DEPTH_BIAS: f32 = 0.0_f32;
/// The header's `D3D11_DEFAULT_STENCIL_READ_MASK`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEFAULT_STENCIL_READ_MASK: u32 = 255;
/// The header's `D3D11_DEFAULT_STENCIL_REFERENCE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEFAULT_STENCIL_REFERENCE: u32 = 0;
/// The header's `D3D11_DEFAULT_STENCIL_WRITE_MASK`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEFAULT_STENCIL_WRITE_MASK: u32 = 255;
/// The header's `D3D11_DEFAULT_VIEWPORT_AND_SCISSORRECT_INDEX`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEFAULT_VIEWPORT_AND_SCISSORRECT_INDEX: u32 = 0;
/// The header's `D3D11_DEFAULT_VIEWPORT_HEIGHT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEFAULT_VIEWPORT_HEIGHT: u32 = 0;
/// The header's `D3D11_DEFAULT_VIEWPORT_MAX_DEPTH`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEFAULT_VIEWPORT_MAX_DEPTH: f32 = 0.0_f32;
/// The header's `D3D11_DEFAULT_VIEWPORT_MIN_DEPTH`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEFAULT_VIEWPORT_MIN_DEPTH: f32 = 0.0_f32;
/// The header's `D3D11_DEFAULT_VIEWPORT_TOPLEFTX`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEFAULT_VIEWPORT_TOPLEFTX: u32 = 0;
/// The header's `D3D11_DEFAULT_VIEWPORT_TOPLEFTY`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEFAULT_VIEWPORT_TOPLEFTY: u32 = 0;
/// The header's `D3D11_DEFAULT_VIEWPORT_WIDTH`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DEFAULT_VIEWPORT_WIDTH: u32 = 0;
/// The header's `D3D11_DS_INPUT_CONTROL_POINTS_MAX_TOTAL_SCALARS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DS_INPUT_CONTROL_POINTS_MAX_TOTAL_SCALARS: u32 = 0xf80;
/// The header's `D3D11_DS_INPUT_CONTROL_POINT_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DS_INPUT_CONTROL_POINT_REGISTER_COMPONENTS: u32 = 4;
/// The header's `D3D11_DS_INPUT_CONTROL_POINT_REGISTER_COMPONENT_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DS_INPUT_CONTROL_POINT_REGISTER_COMPONENT_BIT_COUNT: u32 = 32;
/// The header's `D3D11_DS_INPUT_CONTROL_POINT_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DS_INPUT_CONTROL_POINT_REGISTER_COUNT: u32 = 32;
/// The header's `D3D11_DS_INPUT_CONTROL_POINT_REGISTER_READS_PER_INST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DS_INPUT_CONTROL_POINT_REGISTER_READS_PER_INST: u32 = 2;
/// The header's `D3D11_DS_INPUT_CONTROL_POINT_REGISTER_READ_PORTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DS_INPUT_CONTROL_POINT_REGISTER_READ_PORTS: u32 = 1;
/// The header's `D3D11_DS_INPUT_DOMAIN_POINT_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DS_INPUT_DOMAIN_POINT_REGISTER_COMPONENTS: u32 = 3;
/// The header's `D3D11_DS_INPUT_DOMAIN_POINT_REGISTER_COMPONENT_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DS_INPUT_DOMAIN_POINT_REGISTER_COMPONENT_BIT_COUNT: u32 = 32;
/// The header's `D3D11_DS_INPUT_DOMAIN_POINT_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DS_INPUT_DOMAIN_POINT_REGISTER_COUNT: u32 = 1;
/// The header's `D3D11_DS_INPUT_DOMAIN_POINT_REGISTER_READS_PER_INST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DS_INPUT_DOMAIN_POINT_REGISTER_READS_PER_INST: u32 = 2;
/// The header's `D3D11_DS_INPUT_DOMAIN_POINT_REGISTER_READ_PORTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DS_INPUT_DOMAIN_POINT_REGISTER_READ_PORTS: u32 = 1;
/// The header's `D3D11_DS_INPUT_PATCH_CONSTANT_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DS_INPUT_PATCH_CONSTANT_REGISTER_COMPONENTS: u32 = 4;
/// The header's `D3D11_DS_INPUT_PATCH_CONSTANT_REGISTER_COMPONENT_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DS_INPUT_PATCH_CONSTANT_REGISTER_COMPONENT_BIT_COUNT: u32 = 32;
/// The header's `D3D11_DS_INPUT_PATCH_CONSTANT_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DS_INPUT_PATCH_CONSTANT_REGISTER_COUNT: u32 = 32;
/// The header's `D3D11_DS_INPUT_PATCH_CONSTANT_REGISTER_READS_PER_INST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DS_INPUT_PATCH_CONSTANT_REGISTER_READS_PER_INST: u32 = 2;
/// The header's `D3D11_DS_INPUT_PATCH_CONSTANT_REGISTER_READ_PORTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DS_INPUT_PATCH_CONSTANT_REGISTER_READ_PORTS: u32 = 1;
/// The header's `D3D11_DS_INPUT_PRIMITIVE_ID_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DS_INPUT_PRIMITIVE_ID_REGISTER_COMPONENTS: u32 = 1;
/// The header's `D3D11_DS_INPUT_PRIMITIVE_ID_REGISTER_COMPONENT_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DS_INPUT_PRIMITIVE_ID_REGISTER_COMPONENT_BIT_COUNT: u32 = 32;
/// The header's `D3D11_DS_INPUT_PRIMITIVE_ID_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DS_INPUT_PRIMITIVE_ID_REGISTER_COUNT: u32 = 1;
/// The header's `D3D11_DS_INPUT_PRIMITIVE_ID_REGISTER_READS_PER_INST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DS_INPUT_PRIMITIVE_ID_REGISTER_READS_PER_INST: u32 = 2;
/// The header's `D3D11_DS_INPUT_PRIMITIVE_ID_REGISTER_READ_PORTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DS_INPUT_PRIMITIVE_ID_REGISTER_READ_PORTS: u32 = 1;
/// The header's `D3D11_DS_OUTPUT_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DS_OUTPUT_REGISTER_COMPONENTS: u32 = 4;
/// The header's `D3D11_DS_OUTPUT_REGISTER_COMPONENT_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DS_OUTPUT_REGISTER_COMPONENT_BIT_COUNT: u32 = 32;
/// The header's `D3D11_DS_OUTPUT_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_DS_OUTPUT_REGISTER_COUNT: u32 = 32;
/// The header's `D3D11_ENABLE_BREAK_ON_MESSAGE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_ENABLE_BREAK_ON_MESSAGE: &str = "EnableBreakOnMessage";
/// The header's `D3D11_FILTER_REDUCTION_TYPE_MASK`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_FILTER_REDUCTION_TYPE_MASK: u32 = 3;
/// The header's `D3D11_FILTER_REDUCTION_TYPE_SHIFT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_FILTER_REDUCTION_TYPE_SHIFT: u32 = 7;
/// The header's `D3D11_FILTER_TYPE_MASK`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_FILTER_TYPE_MASK: u32 = 3;
/// The header's `D3D11_FLOAT16_FUSED_TOLERANCE_IN_ULP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_FLOAT16_FUSED_TOLERANCE_IN_ULP: f64 = 0.6;
/// The header's `D3D11_FLOAT32_MAX`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_FLOAT32_MAX: f32 = 3.4028235e38_f32;
/// The header's `D3D11_FLOAT32_TO_INTEGER_TOLERANCE_IN_ULP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_FLOAT32_TO_INTEGER_TOLERANCE_IN_ULP: f32 = 0.6_f32;
/// The header's `D3D11_FLOAT_TO_SRGB_EXPONENT_DENOMINATOR`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_FLOAT_TO_SRGB_EXPONENT_DENOMINATOR: f32 = 2.4_f32;
/// The header's `D3D11_FLOAT_TO_SRGB_EXPONENT_NUMERATOR`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_FLOAT_TO_SRGB_EXPONENT_NUMERATOR: f32 = 1.0_f32;
/// The header's `D3D11_FLOAT_TO_SRGB_OFFSET`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_FLOAT_TO_SRGB_OFFSET: f32 = 0.055_f32;
/// The header's `D3D11_FLOAT_TO_SRGB_SCALE_1`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_FLOAT_TO_SRGB_SCALE_1: f32 = 12.92_f32;
/// The header's `D3D11_FLOAT_TO_SRGB_SCALE_2`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_FLOAT_TO_SRGB_SCALE_2: f32 = 1.055_f32;
/// The header's `D3D11_FLOAT_TO_SRGB_THRESHOLD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_FLOAT_TO_SRGB_THRESHOLD: f32 = 0.0031308_f32;
/// The header's `D3D11_FORCE_DEBUGGABLE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_FORCE_DEBUGGABLE: &str = "ForceDebuggable";
/// The header's `D3D11_FORCE_SHADER_SKIP_OPTIMIZATION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_FORCE_SHADER_SKIP_OPTIMIZATION: &str = "ForceShaderSkipOptimization";
/// The header's `D3D11_FTOI_INSTRUCTION_MAX_INPUT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_FTOI_INSTRUCTION_MAX_INPUT: f32 = 2147483600.0_f32;
/// The header's `D3D11_FTOI_INSTRUCTION_MIN_INPUT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_FTOI_INSTRUCTION_MIN_INPUT: f32 = -2147483600.0_f32;
/// The header's `D3D11_FTOU_INSTRUCTION_MAX_INPUT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_FTOU_INSTRUCTION_MAX_INPUT: f32 = 4294967300.0_f32;
/// The header's `D3D11_FTOU_INSTRUCTION_MIN_INPUT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_FTOU_INSTRUCTION_MIN_INPUT: f32 = 0.0_f32;
/// The header's `D3D11_GS_INPUT_INSTANCE_ID_READS_PER_INST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_GS_INPUT_INSTANCE_ID_READS_PER_INST: u32 = 2;
/// The header's `D3D11_GS_INPUT_INSTANCE_ID_READ_PORTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_GS_INPUT_INSTANCE_ID_READ_PORTS: u32 = 1;
/// The header's `D3D11_GS_INPUT_INSTANCE_ID_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_GS_INPUT_INSTANCE_ID_REGISTER_COMPONENTS: u32 = 1;
/// The header's `D3D11_GS_INPUT_INSTANCE_ID_REGISTER_COMPONENT_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_GS_INPUT_INSTANCE_ID_REGISTER_COMPONENT_BIT_COUNT: u32 = 32;
/// The header's `D3D11_GS_INPUT_INSTANCE_ID_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_GS_INPUT_INSTANCE_ID_REGISTER_COUNT: u32 = 1;
/// The header's `D3D11_GS_INPUT_PRIM_CONST_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_GS_INPUT_PRIM_CONST_REGISTER_COMPONENTS: u32 = 1;
/// The header's `D3D11_GS_INPUT_PRIM_CONST_REGISTER_COMPONENT_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_GS_INPUT_PRIM_CONST_REGISTER_COMPONENT_BIT_COUNT: u32 = 32;
/// The header's `D3D11_GS_INPUT_PRIM_CONST_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_GS_INPUT_PRIM_CONST_REGISTER_COUNT: u32 = 1;
/// The header's `D3D11_GS_INPUT_PRIM_CONST_REGISTER_READS_PER_INST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_GS_INPUT_PRIM_CONST_REGISTER_READS_PER_INST: u32 = 2;
/// The header's `D3D11_GS_INPUT_PRIM_CONST_REGISTER_READ_PORTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_GS_INPUT_PRIM_CONST_REGISTER_READ_PORTS: u32 = 1;
/// The header's `D3D11_GS_INPUT_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_GS_INPUT_REGISTER_COMPONENTS: u32 = 4;
/// The header's `D3D11_GS_INPUT_REGISTER_COMPONENT_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_GS_INPUT_REGISTER_COMPONENT_BIT_COUNT: u32 = 32;
/// The header's `D3D11_GS_INPUT_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_GS_INPUT_REGISTER_COUNT: u32 = 32;
/// The header's `D3D11_GS_INPUT_REGISTER_READS_PER_INST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_GS_INPUT_REGISTER_READS_PER_INST: u32 = 2;
/// The header's `D3D11_GS_INPUT_REGISTER_READ_PORTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_GS_INPUT_REGISTER_READ_PORTS: u32 = 1;
/// The header's `D3D11_GS_INPUT_REGISTER_VERTICES`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_GS_INPUT_REGISTER_VERTICES: u32 = 32;
/// The header's `D3D11_GS_MAX_INSTANCE_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_GS_MAX_INSTANCE_COUNT: u32 = 32;
/// The header's `D3D11_GS_MAX_OUTPUT_VERTEX_COUNT_ACROSS_INSTANCES`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_GS_MAX_OUTPUT_VERTEX_COUNT_ACROSS_INSTANCES: u32 = 0x400;
/// The header's `D3D11_GS_OUTPUT_ELEMENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_GS_OUTPUT_ELEMENTS: u32 = 32;
/// The header's `D3D11_GS_OUTPUT_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_GS_OUTPUT_REGISTER_COMPONENTS: u32 = 4;
/// The header's `D3D11_GS_OUTPUT_REGISTER_COMPONENT_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_GS_OUTPUT_REGISTER_COMPONENT_BIT_COUNT: u32 = 32;
/// The header's `D3D11_GS_OUTPUT_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_GS_OUTPUT_REGISTER_COUNT: u32 = 32;
/// The header's `D3D11_HS_CONTROL_POINT_PHASE_INPUT_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_CONTROL_POINT_PHASE_INPUT_REGISTER_COUNT: u32 = 32;
/// The header's `D3D11_HS_CONTROL_POINT_PHASE_OUTPUT_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_CONTROL_POINT_PHASE_OUTPUT_REGISTER_COUNT: u32 = 32;
/// The header's `D3D11_HS_CONTROL_POINT_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_CONTROL_POINT_REGISTER_COMPONENTS: u32 = 4;
/// The header's `D3D11_HS_CONTROL_POINT_REGISTER_COMPONENT_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_CONTROL_POINT_REGISTER_COMPONENT_BIT_COUNT: u32 = 32;
/// The header's `D3D11_HS_CONTROL_POINT_REGISTER_READS_PER_INST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_CONTROL_POINT_REGISTER_READS_PER_INST: u32 = 2;
/// The header's `D3D11_HS_CONTROL_POINT_REGISTER_READ_PORTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_CONTROL_POINT_REGISTER_READ_PORTS: u32 = 1;
/// The header's `D3D11_HS_FORK_PHASE_INSTANCE_COUNT_UPPER_BOUND`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_FORK_PHASE_INSTANCE_COUNT_UPPER_BOUND: u32 = 0xffffffff;
/// The header's `D3D11_HS_INPUT_FORK_INSTANCE_ID_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_INPUT_FORK_INSTANCE_ID_REGISTER_COMPONENTS: u32 = 1;
/// The header's `D3D11_HS_INPUT_FORK_INSTANCE_ID_REGISTER_COMPONENT_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_INPUT_FORK_INSTANCE_ID_REGISTER_COMPONENT_BIT_COUNT: u32 = 32;
/// The header's `D3D11_HS_INPUT_FORK_INSTANCE_ID_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_INPUT_FORK_INSTANCE_ID_REGISTER_COUNT: u32 = 1;
/// The header's `D3D11_HS_INPUT_FORK_INSTANCE_ID_REGISTER_READS_PER_INST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_INPUT_FORK_INSTANCE_ID_REGISTER_READS_PER_INST: u32 = 2;
/// The header's `D3D11_HS_INPUT_FORK_INSTANCE_ID_REGISTER_READ_PORTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_INPUT_FORK_INSTANCE_ID_REGISTER_READ_PORTS: u32 = 1;
/// The header's `D3D11_HS_INPUT_JOIN_INSTANCE_ID_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_INPUT_JOIN_INSTANCE_ID_REGISTER_COMPONENTS: u32 = 1;
/// The header's `D3D11_HS_INPUT_JOIN_INSTANCE_ID_REGISTER_COMPONENT_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_INPUT_JOIN_INSTANCE_ID_REGISTER_COMPONENT_BIT_COUNT: u32 = 32;
/// The header's `D3D11_HS_INPUT_JOIN_INSTANCE_ID_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_INPUT_JOIN_INSTANCE_ID_REGISTER_COUNT: u32 = 1;
/// The header's `D3D11_HS_INPUT_JOIN_INSTANCE_ID_REGISTER_READS_PER_INST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_INPUT_JOIN_INSTANCE_ID_REGISTER_READS_PER_INST: u32 = 2;
/// The header's `D3D11_HS_INPUT_JOIN_INSTANCE_ID_REGISTER_READ_PORTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_INPUT_JOIN_INSTANCE_ID_REGISTER_READ_PORTS: u32 = 1;
/// The header's `D3D11_HS_INPUT_PRIMITIVE_ID_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_INPUT_PRIMITIVE_ID_REGISTER_COMPONENTS: u32 = 1;
/// The header's `D3D11_HS_INPUT_PRIMITIVE_ID_REGISTER_COMPONENT_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_INPUT_PRIMITIVE_ID_REGISTER_COMPONENT_BIT_COUNT: u32 = 32;
/// The header's `D3D11_HS_INPUT_PRIMITIVE_ID_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_INPUT_PRIMITIVE_ID_REGISTER_COUNT: u32 = 1;
/// The header's `D3D11_HS_INPUT_PRIMITIVE_ID_REGISTER_READS_PER_INST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_INPUT_PRIMITIVE_ID_REGISTER_READS_PER_INST: u32 = 2;
/// The header's `D3D11_HS_INPUT_PRIMITIVE_ID_REGISTER_READ_PORTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_INPUT_PRIMITIVE_ID_REGISTER_READ_PORTS: u32 = 1;
/// The header's `D3D11_HS_JOIN_PHASE_INSTANCE_COUNT_UPPER_BOUND`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_JOIN_PHASE_INSTANCE_COUNT_UPPER_BOUND: u32 = 0xffffffff;
/// The header's `D3D11_HS_MAXTESSFACTOR_LOWER_BOUND`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_MAXTESSFACTOR_LOWER_BOUND: f32 = 1.0_f32;
/// The header's `D3D11_HS_MAXTESSFACTOR_UPPER_BOUND`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_MAXTESSFACTOR_UPPER_BOUND: f32 = 64.0_f32;
/// The header's `D3D11_HS_OUTPUT_CONTROL_POINTS_MAX_TOTAL_SCALARS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_OUTPUT_CONTROL_POINTS_MAX_TOTAL_SCALARS: u32 = 0xf80;
/// The header's `D3D11_HS_OUTPUT_CONTROL_POINT_ID_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_OUTPUT_CONTROL_POINT_ID_REGISTER_COMPONENTS: u32 = 1;
/// The header's `D3D11_HS_OUTPUT_CONTROL_POINT_ID_REGISTER_COMPONENT_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_OUTPUT_CONTROL_POINT_ID_REGISTER_COMPONENT_BIT_COUNT: u32 = 32;
/// The header's `D3D11_HS_OUTPUT_CONTROL_POINT_ID_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_OUTPUT_CONTROL_POINT_ID_REGISTER_COUNT: u32 = 1;
/// The header's `D3D11_HS_OUTPUT_CONTROL_POINT_ID_REGISTER_READS_PER_INST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_OUTPUT_CONTROL_POINT_ID_REGISTER_READS_PER_INST: u32 = 2;
/// The header's `D3D11_HS_OUTPUT_CONTROL_POINT_ID_REGISTER_READ_PORTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_OUTPUT_CONTROL_POINT_ID_REGISTER_READ_PORTS: u32 = 1;
/// The header's `D3D11_HS_OUTPUT_PATCH_CONSTANT_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_OUTPUT_PATCH_CONSTANT_REGISTER_COMPONENTS: u32 = 4;
/// The header's `D3D11_HS_OUTPUT_PATCH_CONSTANT_REGISTER_COMPONENT_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_OUTPUT_PATCH_CONSTANT_REGISTER_COMPONENT_BIT_COUNT: u32 = 32;
/// The header's `D3D11_HS_OUTPUT_PATCH_CONSTANT_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_OUTPUT_PATCH_CONSTANT_REGISTER_COUNT: u32 = 32;
/// The header's `D3D11_HS_OUTPUT_PATCH_CONSTANT_REGISTER_READS_PER_INST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_OUTPUT_PATCH_CONSTANT_REGISTER_READS_PER_INST: u32 = 2;
/// The header's `D3D11_HS_OUTPUT_PATCH_CONSTANT_REGISTER_READ_PORTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_OUTPUT_PATCH_CONSTANT_REGISTER_READ_PORTS: u32 = 1;
/// The header's `D3D11_HS_OUTPUT_PATCH_CONSTANT_REGISTER_SCALAR_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_HS_OUTPUT_PATCH_CONSTANT_REGISTER_SCALAR_COMPONENTS: u32 = 128;
/// The header's `D3D11_IA_DEFAULT_INDEX_BUFFER_OFFSET_IN_BYTES`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_IA_DEFAULT_INDEX_BUFFER_OFFSET_IN_BYTES: u32 = 0;
/// The header's `D3D11_IA_DEFAULT_PRIMITIVE_TOPOLOGY`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_IA_DEFAULT_PRIMITIVE_TOPOLOGY: u32 = 0;
/// The header's `D3D11_IA_DEFAULT_VERTEX_BUFFER_OFFSET_IN_BYTES`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_IA_DEFAULT_VERTEX_BUFFER_OFFSET_IN_BYTES: u32 = 0;
/// The header's `D3D11_IA_INDEX_INPUT_RESOURCE_SLOT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_IA_INDEX_INPUT_RESOURCE_SLOT_COUNT: u32 = 1;
/// The header's `D3D11_IA_INSTANCE_ID_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_IA_INSTANCE_ID_BIT_COUNT: u32 = 32;
/// The header's `D3D11_IA_INTEGER_ARITHMETIC_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_IA_INTEGER_ARITHMETIC_BIT_COUNT: u32 = 32;
/// The header's `D3D11_IA_PATCH_MAX_CONTROL_POINT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_IA_PATCH_MAX_CONTROL_POINT_COUNT: u32 = 32;
/// The header's `D3D11_IA_PRIMITIVE_ID_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_IA_PRIMITIVE_ID_BIT_COUNT: u32 = 32;
/// The header's `D3D11_IA_VERTEX_ID_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_IA_VERTEX_ID_BIT_COUNT: u32 = 32;
/// The header's `D3D11_IA_VERTEX_INPUT_RESOURCE_SLOT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_IA_VERTEX_INPUT_RESOURCE_SLOT_COUNT: u32 = 32;
/// The header's `D3D11_IA_VERTEX_INPUT_STRUCTURE_ELEMENTS_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_IA_VERTEX_INPUT_STRUCTURE_ELEMENTS_COMPONENTS: u32 = 128;
/// The header's `D3D11_IA_VERTEX_INPUT_STRUCTURE_ELEMENT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_IA_VERTEX_INPUT_STRUCTURE_ELEMENT_COUNT: u32 = 32;
/// The header's `D3D11_INFOQUEUE_STORAGE_FILTER_OVERRIDE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_INFOQUEUE_STORAGE_FILTER_OVERRIDE: &str = "InfoQueueStorageFilterOverride";
/// The header's `D3D11_INFO_QUEUE_DEFAULT_MESSAGE_COUNT_LIMIT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_INFO_QUEUE_DEFAULT_MESSAGE_COUNT_LIMIT: u32 = 0x400;
/// The header's `D3D11_INTEGER_DIVIDE_BY_ZERO_QUOTIENT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_INTEGER_DIVIDE_BY_ZERO_QUOTIENT: u32 = 0xffffffff;
/// The header's `D3D11_INTEGER_DIVIDE_BY_ZERO_REMAINDER`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_INTEGER_DIVIDE_BY_ZERO_REMAINDER: u32 = 0xffffffff;
/// The header's `D3D11_KEEP_RENDER_TARGETS_AND_DEPTH_STENCIL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_KEEP_RENDER_TARGETS_AND_DEPTH_STENCIL: u32 = 0xffffffff;
/// The header's `D3D11_KEEP_UNORDERED_ACCESS_VIEWS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_KEEP_UNORDERED_ACCESS_VIEWS: u32 = 0xffffffff;
/// The header's `D3D11_KEY_EXCHANGE_HW_PROTECTION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_KEY_EXCHANGE_HW_PROTECTION: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xb1170d8a,
        data2: 0x628d,
        data3: 0x4da3,
        data4: [0xad, 0x3b, 0x82, 0xdd, 0xb0, 0x8b, 0x49, 0x70],
    };
/// The header's `D3D11_KEY_EXCHANGE_RSAES_OAEP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_KEY_EXCHANGE_RSAES_OAEP: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xc1949895,
        data2: 0xd72a,
        data3: 0x4a1d,
        data4: [0x8e, 0x5d, 0xed, 0x85, 0x7d, 0x17, 0x15, 0x20],
    };
/// The header's `D3D11_LINEAR_GAMMA`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_LINEAR_GAMMA: f32 = 1.0_f32;
/// The header's `D3D11_MAG_FILTER_SHIFT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_MAG_FILTER_SHIFT: u32 = 2;
/// The header's `D3D11_MAJOR_VERSION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_MAJOR_VERSION: u32 = 11;
/// The header's `D3D11_MAX_BORDER_COLOR_COMPONENT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_MAX_BORDER_COLOR_COMPONENT: f32 = 1.0_f32;
/// The header's `D3D11_MAX_DEPTH`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_MAX_DEPTH: f32 = 1.0_f32;
/// The header's `D3D11_MAX_MAXANISOTROPY`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_MAX_MAXANISOTROPY: u32 = 16;
/// The header's `D3D11_MAX_MULTISAMPLE_SAMPLE_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_MAX_MULTISAMPLE_SAMPLE_COUNT: u32 = 32;
/// The header's `D3D11_MAX_POSITION_VALUE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_MAX_POSITION_VALUE: f32 = 3.4028236e34_f32;
/// The header's `D3D11_MAX_TEXTURE_DIMENSION_2_TO_EXP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_MAX_TEXTURE_DIMENSION_2_TO_EXP: u32 = 17;
/// The header's `D3D11_MINOR_VERSION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_MINOR_VERSION: u32 = 0;
/// The header's `D3D11_MIN_BORDER_COLOR_COMPONENT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_MIN_BORDER_COLOR_COMPONENT: f32 = 0.0_f32;
/// The header's `D3D11_MIN_DEPTH`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_MIN_DEPTH: f32 = 0.0_f32;
/// The header's `D3D11_MIN_FILTER_SHIFT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_MIN_FILTER_SHIFT: u32 = 4;
/// The header's `D3D11_MIN_MAXANISOTROPY`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_MIN_MAXANISOTROPY: u32 = 0;
/// The header's `D3D11_MIP_FILTER_SHIFT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_MIP_FILTER_SHIFT: u32 = 0;
/// The header's `D3D11_MIP_LOD_BIAS_MAX`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_MIP_LOD_BIAS_MAX: f32 = 15.99_f32;
/// The header's `D3D11_MIP_LOD_BIAS_MIN`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_MIP_LOD_BIAS_MIN: f32 = -16.0_f32;
/// The header's `D3D11_MIP_LOD_FRACTIONAL_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_MIP_LOD_FRACTIONAL_BIT_COUNT: u32 = 8;
/// The header's `D3D11_MIP_LOD_RANGE_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_MIP_LOD_RANGE_BIT_COUNT: u32 = 8;
/// The header's `D3D11_MULTISAMPLE_ANTIALIAS_LINE_WIDTH`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_MULTISAMPLE_ANTIALIAS_LINE_WIDTH: f32 = 1.4_f32;
/// The header's `D3D11_MUTE_CATEGORY`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_MUTE_CATEGORY: &str = "Mute_CATEGORY_%s";
/// The header's `D3D11_MUTE_DEBUG_OUTPUT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_MUTE_DEBUG_OUTPUT: &str = "MuteDebugOutput";
/// The header's `D3D11_MUTE_ID_DECIMAL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_MUTE_ID_DECIMAL: &str = "Mute_ID_%d";
/// The header's `D3D11_MUTE_ID_STRING`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_MUTE_ID_STRING: &str = "Mute_ID_%s";
/// The header's `D3D11_MUTE_SEVERITY`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_MUTE_SEVERITY: &str = "Mute_SEVERITY_%s";
/// The header's `D3D11_NONSAMPLE_FETCH_OUT_OF_RANGE_ACCESS_RESULT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_NONSAMPLE_FETCH_OUT_OF_RANGE_ACCESS_RESULT: u32 = 0;
/// The header's `D3D11_PACKED_TILE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_PACKED_TILE: u32 = 0xffffffff;
/// The header's `D3D11_PIXEL_ADDRESS_RANGE_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_PIXEL_ADDRESS_RANGE_BIT_COUNT: u32 = 15;
/// The header's `D3D11_PRE_SCISSOR_PIXEL_ADDRESS_RANGE_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_PRE_SCISSOR_PIXEL_ADDRESS_RANGE_BIT_COUNT: u32 = 16;
/// The header's `D3D11_PS_CS_UAV_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_PS_CS_UAV_REGISTER_COMPONENTS: u32 = 1;
/// The header's `D3D11_PS_CS_UAV_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_PS_CS_UAV_REGISTER_COUNT: u32 = 8;
/// The header's `D3D11_PS_CS_UAV_REGISTER_READS_PER_INST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_PS_CS_UAV_REGISTER_READS_PER_INST: u32 = 1;
/// The header's `D3D11_PS_CS_UAV_REGISTER_READ_PORTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_PS_CS_UAV_REGISTER_READ_PORTS: u32 = 1;
/// The header's `D3D11_PS_FRONTFACING_DEFAULT_VALUE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_PS_FRONTFACING_DEFAULT_VALUE: u32 = 0xffffffff;
/// The header's `D3D11_PS_FRONTFACING_FALSE_VALUE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_PS_FRONTFACING_FALSE_VALUE: u32 = 0;
/// The header's `D3D11_PS_FRONTFACING_TRUE_VALUE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_PS_FRONTFACING_TRUE_VALUE: u32 = 0xffffffff;
/// The header's `D3D11_PS_INPUT_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_PS_INPUT_REGISTER_COMPONENTS: u32 = 4;
/// The header's `D3D11_PS_INPUT_REGISTER_COMPONENT_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_PS_INPUT_REGISTER_COMPONENT_BIT_COUNT: u32 = 32;
/// The header's `D3D11_PS_INPUT_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_PS_INPUT_REGISTER_COUNT: u32 = 32;
/// The header's `D3D11_PS_INPUT_REGISTER_READS_PER_INST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_PS_INPUT_REGISTER_READS_PER_INST: u32 = 2;
/// The header's `D3D11_PS_INPUT_REGISTER_READ_PORTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_PS_INPUT_REGISTER_READ_PORTS: u32 = 1;
/// The header's `D3D11_PS_LEGACY_PIXEL_CENTER_FRACTIONAL_COMPONENT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_PS_LEGACY_PIXEL_CENTER_FRACTIONAL_COMPONENT: f32 = 0.0_f32;
/// The header's `D3D11_PS_OUTPUT_DEPTH_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_PS_OUTPUT_DEPTH_REGISTER_COMPONENTS: u32 = 1;
/// The header's `D3D11_PS_OUTPUT_DEPTH_REGISTER_COMPONENT_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_PS_OUTPUT_DEPTH_REGISTER_COMPONENT_BIT_COUNT: u32 = 32;
/// The header's `D3D11_PS_OUTPUT_DEPTH_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_PS_OUTPUT_DEPTH_REGISTER_COUNT: u32 = 1;
/// The header's `D3D11_PS_OUTPUT_MASK_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_PS_OUTPUT_MASK_REGISTER_COMPONENTS: u32 = 1;
/// The header's `D3D11_PS_OUTPUT_MASK_REGISTER_COMPONENT_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_PS_OUTPUT_MASK_REGISTER_COMPONENT_BIT_COUNT: u32 = 32;
/// The header's `D3D11_PS_OUTPUT_MASK_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_PS_OUTPUT_MASK_REGISTER_COUNT: u32 = 1;
/// The header's `D3D11_PS_OUTPUT_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_PS_OUTPUT_REGISTER_COMPONENTS: u32 = 4;
/// The header's `D3D11_PS_OUTPUT_REGISTER_COMPONENT_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_PS_OUTPUT_REGISTER_COMPONENT_BIT_COUNT: u32 = 32;
/// The header's `D3D11_PS_OUTPUT_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_PS_OUTPUT_REGISTER_COUNT: u32 = 8;
/// The header's `D3D11_PS_PIXEL_CENTER_FRACTIONAL_COMPONENT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_PS_PIXEL_CENTER_FRACTIONAL_COMPONENT: f32 = 0.5_f32;
/// The header's `D3D11_RAW_UAV_SRV_BYTE_ALIGNMENT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_RAW_UAV_SRV_BYTE_ALIGNMENT: u32 = 16;
/// The header's `D3D11_REGKEY_PATH`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_REGKEY_PATH: &str = "Software\\Microsoft\\Direct3D";
/// The header's `D3D11_REQ_BLEND_OBJECT_COUNT_PER_DEVICE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_REQ_BLEND_OBJECT_COUNT_PER_DEVICE: u32 = 0x1000;
/// The header's `D3D11_REQ_BUFFER_RESOURCE_TEXEL_COUNT_2_TO_EXP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_REQ_BUFFER_RESOURCE_TEXEL_COUNT_2_TO_EXP: u32 = 27;
/// The header's `D3D11_REQ_CONSTANT_BUFFER_ELEMENT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_REQ_CONSTANT_BUFFER_ELEMENT_COUNT: u32 = 0x1000;
/// The header's `D3D11_REQ_DEPTH_STENCIL_OBJECT_COUNT_PER_DEVICE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_REQ_DEPTH_STENCIL_OBJECT_COUNT_PER_DEVICE: u32 = 0x1000;
/// The header's `D3D11_REQ_DRAWINDEXED_INDEX_COUNT_2_TO_EXP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_REQ_DRAWINDEXED_INDEX_COUNT_2_TO_EXP: u32 = 32;
/// The header's `D3D11_REQ_DRAW_VERTEX_COUNT_2_TO_EXP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_REQ_DRAW_VERTEX_COUNT_2_TO_EXP: u32 = 32;
/// The header's `D3D11_REQ_FILTERING_HW_ADDRESSABLE_RESOURCE_DIMENSION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_REQ_FILTERING_HW_ADDRESSABLE_RESOURCE_DIMENSION: u32 = 0x4000;
/// The header's `D3D11_REQ_GS_INVOCATION_32BIT_OUTPUT_COMPONENT_LIMIT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_REQ_GS_INVOCATION_32BIT_OUTPUT_COMPONENT_LIMIT: u32 = 0x400;
/// The header's `D3D11_REQ_IMMEDIATE_CONSTANT_BUFFER_ELEMENT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_REQ_IMMEDIATE_CONSTANT_BUFFER_ELEMENT_COUNT: u32 = 0x1000;
/// The header's `D3D11_REQ_MAXANISOTROPY`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_REQ_MAXANISOTROPY: u32 = 16;
/// The header's `D3D11_REQ_MIP_LEVELS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_REQ_MIP_LEVELS: u32 = 15;
/// The header's `D3D11_REQ_MULTI_ELEMENT_STRUCTURE_SIZE_IN_BYTES`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_REQ_MULTI_ELEMENT_STRUCTURE_SIZE_IN_BYTES: u32 = 0x800;
/// The header's `D3D11_REQ_RASTERIZER_OBJECT_COUNT_PER_DEVICE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_REQ_RASTERIZER_OBJECT_COUNT_PER_DEVICE: u32 = 0x1000;
/// The header's `D3D11_REQ_RENDER_TO_BUFFER_WINDOW_WIDTH`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_REQ_RENDER_TO_BUFFER_WINDOW_WIDTH: u32 = 0x4000;
/// The header's `D3D11_REQ_RESOURCE_SIZE_IN_MEGABYTES_EXPRESSION_A_TERM`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_REQ_RESOURCE_SIZE_IN_MEGABYTES_EXPRESSION_A_TERM: u32 = 128;
/// The header's `D3D11_REQ_RESOURCE_SIZE_IN_MEGABYTES_EXPRESSION_B_TERM`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_REQ_RESOURCE_SIZE_IN_MEGABYTES_EXPRESSION_B_TERM: f32 = 0.25_f32;
/// The header's `D3D11_REQ_RESOURCE_SIZE_IN_MEGABYTES_EXPRESSION_C_TERM`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_REQ_RESOURCE_SIZE_IN_MEGABYTES_EXPRESSION_C_TERM: u32 = 0x800;
/// The header's `D3D11_REQ_RESOURCE_VIEW_COUNT_PER_DEVICE_2_TO_EXP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_REQ_RESOURCE_VIEW_COUNT_PER_DEVICE_2_TO_EXP: u32 = 20;
/// The header's `D3D11_REQ_SAMPLER_OBJECT_COUNT_PER_DEVICE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_REQ_SAMPLER_OBJECT_COUNT_PER_DEVICE: u32 = 0x1000;
/// The header's `D3D11_REQ_TEXTURE1D_ARRAY_AXIS_DIMENSION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_REQ_TEXTURE1D_ARRAY_AXIS_DIMENSION: u32 = 0x800;
/// The header's `D3D11_REQ_TEXTURE1D_U_DIMENSION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_REQ_TEXTURE1D_U_DIMENSION: u32 = 0x4000;
/// The header's `D3D11_REQ_TEXTURE2D_ARRAY_AXIS_DIMENSION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_REQ_TEXTURE2D_ARRAY_AXIS_DIMENSION: u32 = 0x800;
/// The header's `D3D11_REQ_TEXTURE2D_U_OR_V_DIMENSION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_REQ_TEXTURE2D_U_OR_V_DIMENSION: u32 = 0x4000;
/// The header's `D3D11_REQ_TEXTURE3D_U_V_OR_W_DIMENSION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_REQ_TEXTURE3D_U_V_OR_W_DIMENSION: u32 = 0x800;
/// The header's `D3D11_REQ_TEXTURECUBE_DIMENSION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_REQ_TEXTURECUBE_DIMENSION: u32 = 0x4000;
/// The header's `D3D11_RESINFO_INSTRUCTION_MISSING_COMPONENT_RETVAL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_RESINFO_INSTRUCTION_MISSING_COMPONENT_RETVAL: u32 = 0;
/// The header's `D3D11_SDK_LAYERS_VERSION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SDK_LAYERS_VERSION: u32 = 1;
/// The header's `D3D11_SDK_VERSION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SDK_VERSION: u32 = 7;
/// The header's `D3D11_SHADER_MAJOR_VERSION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SHADER_MAJOR_VERSION: u32 = 5;
/// The header's `D3D11_SHADER_MAX_INSTANCES`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SHADER_MAX_INSTANCES: u32 = 0xffff;
/// The header's `D3D11_SHADER_MAX_INTERFACES`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SHADER_MAX_INTERFACES: u32 = 253;
/// The header's `D3D11_SHADER_MAX_INTERFACE_CALL_SITES`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SHADER_MAX_INTERFACE_CALL_SITES: u32 = 0x1000;
/// The header's `D3D11_SHADER_MAX_TYPES`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SHADER_MAX_TYPES: u32 = 0xffff;
/// The header's `D3D11_SHADER_MINOR_VERSION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SHADER_MINOR_VERSION: u32 = 0;
/// The header's `D3D11_SHADER_TRACE_FLAG_RECORD_REGISTER_READS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SHADER_TRACE_FLAG_RECORD_REGISTER_READS: u32 = 2;
/// The header's `D3D11_SHADER_TRACE_FLAG_RECORD_REGISTER_WRITES`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SHADER_TRACE_FLAG_RECORD_REGISTER_WRITES: u32 = 1;
/// The header's `D3D11_SHIFT_INSTRUCTION_PAD_VALUE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SHIFT_INSTRUCTION_PAD_VALUE: u32 = 0;
/// The header's `D3D11_SHIFT_INSTRUCTION_SHIFT_VALUE_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SHIFT_INSTRUCTION_SHIFT_VALUE_BIT_COUNT: u32 = 5;
/// The header's `D3D11_SIMULTANEOUS_RENDER_TARGET_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SIMULTANEOUS_RENDER_TARGET_COUNT: u32 = 8;
/// The header's `D3D11_SO_BUFFER_MAX_STRIDE_IN_BYTES`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SO_BUFFER_MAX_STRIDE_IN_BYTES: u32 = 0x800;
/// The header's `D3D11_SO_BUFFER_MAX_WRITE_WINDOW_IN_BYTES`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SO_BUFFER_MAX_WRITE_WINDOW_IN_BYTES: u32 = 0x200;
/// The header's `D3D11_SO_BUFFER_SLOT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SO_BUFFER_SLOT_COUNT: u32 = 4;
/// The header's `D3D11_SO_DDI_REGISTER_INDEX_DENOTING_GAP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SO_DDI_REGISTER_INDEX_DENOTING_GAP: u32 = 0xffffffff;
/// The header's `D3D11_SO_NO_RASTERIZED_STREAM`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SO_NO_RASTERIZED_STREAM: u32 = 0xffffffff;
/// The header's `D3D11_SO_OUTPUT_COMPONENT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SO_OUTPUT_COMPONENT_COUNT: u32 = 128;
/// The header's `D3D11_SO_STREAM_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SO_STREAM_COUNT: u32 = 4;
/// The header's `D3D11_SPEC_DATE_DAY`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SPEC_DATE_DAY: u32 = 16;
/// The header's `D3D11_SPEC_DATE_MONTH`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SPEC_DATE_MONTH: u32 = 5;
/// The header's `D3D11_SPEC_DATE_YEAR`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SPEC_DATE_YEAR: u32 = 0x7db;
/// The header's `D3D11_SPEC_VERSION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SPEC_VERSION: f64 = 1.07;
/// The header's `D3D11_SRGB_GAMMA`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SRGB_GAMMA: f32 = 2.2_f32;
/// The header's `D3D11_SRGB_TO_FLOAT_DENOMINATOR_1`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SRGB_TO_FLOAT_DENOMINATOR_1: f32 = 12.92_f32;
/// The header's `D3D11_SRGB_TO_FLOAT_DENOMINATOR_2`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SRGB_TO_FLOAT_DENOMINATOR_2: f32 = 1.055_f32;
/// The header's `D3D11_SRGB_TO_FLOAT_EXPONENT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SRGB_TO_FLOAT_EXPONENT: f32 = 2.4_f32;
/// The header's `D3D11_SRGB_TO_FLOAT_OFFSET`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SRGB_TO_FLOAT_OFFSET: f32 = 0.055_f32;
/// The header's `D3D11_SRGB_TO_FLOAT_THRESHOLD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SRGB_TO_FLOAT_THRESHOLD: f32 = 0.04045_f32;
/// The header's `D3D11_SRGB_TO_FLOAT_TOLERANCE_IN_ULP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SRGB_TO_FLOAT_TOLERANCE_IN_ULP: f32 = 0.5_f32;
/// The header's `D3D11_STANDARD_COMPONENT_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_STANDARD_COMPONENT_BIT_COUNT: u32 = 32;
/// The header's `D3D11_STANDARD_COMPONENT_BIT_COUNT_DOUBLED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_STANDARD_COMPONENT_BIT_COUNT_DOUBLED: u32 = 64;
/// The header's `D3D11_STANDARD_MAXIMUM_ELEMENT_ALIGNMENT_BYTE_MULTIPLE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_STANDARD_MAXIMUM_ELEMENT_ALIGNMENT_BYTE_MULTIPLE: u32 = 4;
/// The header's `D3D11_STANDARD_PIXEL_COMPONENT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_STANDARD_PIXEL_COMPONENT_COUNT: u32 = 128;
/// The header's `D3D11_STANDARD_PIXEL_ELEMENT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_STANDARD_PIXEL_ELEMENT_COUNT: u32 = 32;
/// The header's `D3D11_STANDARD_VECTOR_SIZE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_STANDARD_VECTOR_SIZE: u32 = 4;
/// The header's `D3D11_STANDARD_VERTEX_ELEMENT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_STANDARD_VERTEX_ELEMENT_COUNT: u32 = 32;
/// The header's `D3D11_STANDARD_VERTEX_TOTAL_COMPONENT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_STANDARD_VERTEX_TOTAL_COMPONENT_COUNT: u32 = 64;
/// The header's `D3D11_SUBPIXEL_FRACTIONAL_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SUBPIXEL_FRACTIONAL_BIT_COUNT: u32 = 8;
/// The header's `D3D11_SUBTEXEL_FRACTIONAL_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SUBTEXEL_FRACTIONAL_BIT_COUNT: u32 = 8;
/// The header's `D3D11_TESSELLATOR_MAX_EVEN_TESSELLATION_FACTOR`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TESSELLATOR_MAX_EVEN_TESSELLATION_FACTOR: u32 = 64;
/// The header's `D3D11_TESSELLATOR_MAX_ISOLINE_DENSITY_TESSELLATION_FACTOR`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TESSELLATOR_MAX_ISOLINE_DENSITY_TESSELLATION_FACTOR: u32 = 64;
/// The header's `D3D11_TESSELLATOR_MAX_ODD_TESSELLATION_FACTOR`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TESSELLATOR_MAX_ODD_TESSELLATION_FACTOR: u32 = 63;
/// The header's `D3D11_TESSELLATOR_MAX_TESSELLATION_FACTOR`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TESSELLATOR_MAX_TESSELLATION_FACTOR: u32 = 64;
/// The header's `D3D11_TESSELLATOR_MIN_EVEN_TESSELLATION_FACTOR`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TESSELLATOR_MIN_EVEN_TESSELLATION_FACTOR: u32 = 2;
/// The header's `D3D11_TESSELLATOR_MIN_ISOLINE_DENSITY_TESSELLATION_FACTOR`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TESSELLATOR_MIN_ISOLINE_DENSITY_TESSELLATION_FACTOR: u32 = 1;
/// The header's `D3D11_TESSELLATOR_MIN_ODD_TESSELLATION_FACTOR`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TESSELLATOR_MIN_ODD_TESSELLATION_FACTOR: u32 = 1;
/// The header's `D3D11_TEXEL_ADDRESS_RANGE_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TEXEL_ADDRESS_RANGE_BIT_COUNT: u32 = 16;
/// The header's `D3D11_TRACE_COMPONENT_W`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TRACE_COMPONENT_W: u32 = 8;
/// The header's `D3D11_TRACE_COMPONENT_X`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TRACE_COMPONENT_X: u32 = 1;
/// The header's `D3D11_TRACE_COMPONENT_Y`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TRACE_COMPONENT_Y: u32 = 2;
/// The header's `D3D11_TRACE_COMPONENT_Z`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TRACE_COMPONENT_Z: u32 = 4;
/// The header's `D3D11_TRACE_MISC_GS_CUT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TRACE_MISC_GS_CUT: u32 = 2;
/// The header's `D3D11_TRACE_MISC_GS_CUT_STREAM`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TRACE_MISC_GS_CUT_STREAM: u32 = 16;
/// The header's `D3D11_TRACE_MISC_GS_EMIT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TRACE_MISC_GS_EMIT: u32 = 1;
/// The header's `D3D11_TRACE_MISC_GS_EMIT_STREAM`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TRACE_MISC_GS_EMIT_STREAM: u32 = 8;
/// The header's `D3D11_TRACE_MISC_HALT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TRACE_MISC_HALT: u32 = 32;
/// The header's `D3D11_TRACE_MISC_MESSAGE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TRACE_MISC_MESSAGE: u32 = 64;
/// The header's `D3D11_TRACE_MISC_PS_DISCARD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TRACE_MISC_PS_DISCARD: u32 = 4;
/// The header's `D3D11_TRACE_REGISTER_FLAGS_RELATIVE_INDEXING`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TRACE_REGISTER_FLAGS_RELATIVE_INDEXING: u32 = 1;
/// The header's `D3D11_UNBOUND_MEMORY_ACCESS_RESULT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_UNBOUND_MEMORY_ACCESS_RESULT: u32 = 0;
/// The header's `D3D11_UNMUTE_SEVERITY_INFO`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_UNMUTE_SEVERITY_INFO: &str = "Unmute_SEVERITY_INFO";
/// The header's `D3D11_VIEWPORT_AND_SCISSORRECT_MAX_INDEX`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_VIEWPORT_AND_SCISSORRECT_MAX_INDEX: u32 = 15;
/// The header's `D3D11_VIEWPORT_AND_SCISSORRECT_OBJECT_COUNT_PER_PIPELINE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_VIEWPORT_AND_SCISSORRECT_OBJECT_COUNT_PER_PIPELINE: u32 = 16;
/// The header's `D3D11_VIEWPORT_BOUNDS_MAX`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_VIEWPORT_BOUNDS_MAX: u32 = 0x7fff;
/// The header's `D3D11_VIEWPORT_BOUNDS_MIN`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_VIEWPORT_BOUNDS_MIN: i32 = -32768;
/// The header's `D3D11_VS_INPUT_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_VS_INPUT_REGISTER_COMPONENTS: u32 = 4;
/// The header's `D3D11_VS_INPUT_REGISTER_COMPONENT_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_VS_INPUT_REGISTER_COMPONENT_BIT_COUNT: u32 = 32;
/// The header's `D3D11_VS_INPUT_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_VS_INPUT_REGISTER_COUNT: u32 = 32;
/// The header's `D3D11_VS_INPUT_REGISTER_READS_PER_INST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_VS_INPUT_REGISTER_READS_PER_INST: u32 = 2;
/// The header's `D3D11_VS_INPUT_REGISTER_READ_PORTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_VS_INPUT_REGISTER_READ_PORTS: u32 = 1;
/// The header's `D3D11_VS_OUTPUT_REGISTER_COMPONENTS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_VS_OUTPUT_REGISTER_COMPONENTS: u32 = 4;
/// The header's `D3D11_VS_OUTPUT_REGISTER_COMPONENT_BIT_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_VS_OUTPUT_REGISTER_COMPONENT_BIT_COUNT: u32 = 32;
/// The header's `D3D11_VS_OUTPUT_REGISTER_COUNT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_VS_OUTPUT_REGISTER_COUNT: u32 = 32;
/// The header's `D3D11_WHQL_CONTEXT_COUNT_FOR_RESOURCE_LIMIT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_WHQL_CONTEXT_COUNT_FOR_RESOURCE_LIMIT: u32 = 10;
/// The header's `D3D11_WHQL_DRAWINDEXED_INDEX_COUNT_2_TO_EXP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_WHQL_DRAWINDEXED_INDEX_COUNT_2_TO_EXP: u32 = 25;
/// The header's `D3D11_WHQL_DRAW_VERTEX_COUNT_2_TO_EXP`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_WHQL_DRAW_VERTEX_COUNT_2_TO_EXP: u32 = 25;
/// The header's `D3DCSX_DLL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DCSX_DLL: &str = "d3dcsx_47.dll";
/// The header's `D3DCSX_DLL_A`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DCSX_DLL_A: &str = "d3dcsx_47.dll";
/// The header's `D3DCSX_DLL_W`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DCSX_DLL_W: &str = "d3dcsx_47.dll";
/// The header's `D3DX11_FFT_MAX_DIMENSIONS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DX11_FFT_MAX_DIMENSIONS: u32 = 32;
/// The header's `D3DX11_FFT_MAX_PRECOMPUTE_BUFFERS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DX11_FFT_MAX_PRECOMPUTE_BUFFERS: u32 = 4;
/// The header's `D3DX11_FFT_MAX_TEMP_BUFFERS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DX11_FFT_MAX_TEMP_BUFFERS: u32 = 4;
/// The header's `D3D_RETURN_PARAMETER_INDEX`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D_RETURN_PARAMETER_INDEX: i32 = -1;
/// The header's `D3D_SHADER_REQUIRES_11_1_DOUBLE_EXTENSIONS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D_SHADER_REQUIRES_11_1_DOUBLE_EXTENSIONS: u32 = 32;
/// The header's `D3D_SHADER_REQUIRES_11_1_SHADER_EXTENSIONS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D_SHADER_REQUIRES_11_1_SHADER_EXTENSIONS: u32 = 64;
/// The header's `D3D_SHADER_REQUIRES_64_UAVS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D_SHADER_REQUIRES_64_UAVS: u32 = 8;
/// The header's `D3D_SHADER_REQUIRES_DOUBLES`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D_SHADER_REQUIRES_DOUBLES: u32 = 1;
/// The header's `D3D_SHADER_REQUIRES_EARLY_DEPTH_STENCIL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D_SHADER_REQUIRES_EARLY_DEPTH_STENCIL: u32 = 2;
/// The header's `D3D_SHADER_REQUIRES_LEVEL_9_COMPARISON_FILTERING`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D_SHADER_REQUIRES_LEVEL_9_COMPARISON_FILTERING: u32 = 128;
/// The header's `D3D_SHADER_REQUIRES_MINIMUM_PRECISION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D_SHADER_REQUIRES_MINIMUM_PRECISION: u32 = 16;
/// The header's `D3D_SHADER_REQUIRES_TILED_RESOURCES`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D_SHADER_REQUIRES_TILED_RESOURCES: u32 = 0x100;
/// The header's `D3D_SHADER_REQUIRES_UAVS_AT_EVERY_STAGE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D_SHADER_REQUIRES_UAVS_AT_EVERY_STAGE: u32 = 4;
/// The header's `DXGI_DEBUG_D3D11`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const DXGI_DEBUG_D3D11: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x4b99317b,
        data2: 0xac39,
        data3: 0x4aa6,
        data4: [0xbb, 0x0b, 0xba, 0xa0, 0x47, 0x84, 0x79, 0x8f],
    };
/// The header's `_FACD3D11`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const _FACD3D11: u32 = 0x87c;
/// The header's `IID_ID3D11Asynchronous`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11asynchronous).
pub const IID_ID3D11_ASYNCHRONOUS: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x4b35d0cd,
        data2: 0x1e15,
        data3: 0x4258,
        data4: [0x9c, 0x98, 0x1b, 0x13, 0x33, 0xf6, 0xdd, 0x3b],
    };
/// The header's `IID_ID3D11AuthenticatedChannel`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11authenticatedchannel).
pub const IID_ID3D11_AUTHENTICATED_CHANNEL: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x3015a308,
        data2: 0xdcbd,
        data3: 0x47aa,
        data4: [0xa7, 0x47, 0x19, 0x24, 0x86, 0xd1, 0x4d, 0x4a],
    };
/// The header's `IID_ID3D11BlendState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11blendstate).
pub const IID_ID3D11_BLEND_STATE: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x75b68faa,
        data2: 0x347d,
        data3: 0x4159,
        data4: [0x8f, 0x45, 0xa0, 0x64, 0x0f, 0x01, 0xcd, 0x9a],
    };
/// The header's `IID_ID3D11BlendState1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nn-d3d11_1-id3d11blendstate1).
pub const IID_ID3D11_BLEND_STATE1: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xcc86fabe,
        data2: 0xda55,
        data3: 0x401d,
        data4: [0x85, 0xe7, 0xe3, 0xc9, 0xde, 0x28, 0x77, 0xe9],
    };
/// The header's `IID_ID3D11Buffer`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11buffer).
pub const IID_ID3D11_BUFFER: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x48570b85,
        data2: 0xd1ee,
        data3: 0x4fcd,
        data4: [0xa2, 0x50, 0xeb, 0x35, 0x07, 0x22, 0xb0, 0x37],
    };
/// The header's `IID_ID3D11ClassInstance`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11classinstance).
pub const IID_ID3D11_CLASS_INSTANCE: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xa6cd7faa,
        data2: 0xb0b7,
        data3: 0x4a2f,
        data4: [0x94, 0x36, 0x86, 0x62, 0xa6, 0x57, 0x97, 0xcb],
    };
/// The header's `IID_ID3D11ClassLinkage`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11classlinkage).
pub const IID_ID3D11_CLASS_LINKAGE: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xddf57cba,
        data2: 0x9543,
        data3: 0x46e4,
        data4: [0xa1, 0x2b, 0xf2, 0x07, 0xa0, 0xfe, 0x7f, 0xed],
    };
/// The header's `IID_ID3D11CommandList`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11commandlist).
pub const IID_ID3D11_COMMAND_LIST: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xa24bc4d1,
        data2: 0x769e,
        data3: 0x43f7,
        data4: [0x80, 0x13, 0x98, 0xff, 0x56, 0x6c, 0x18, 0xe2],
    };
/// The header's `IID_ID3D11ComputeShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11computeshader).
pub const IID_ID3D11_COMPUTE_SHADER: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x4f5b196e,
        data2: 0xc2bd,
        data3: 0x495e,
        data4: [0xbd, 0x01, 0x1f, 0xde, 0xd3, 0x8e, 0x49, 0x69],
    };
/// The header's `IID_ID3D11Counter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11counter).
pub const IID_ID3D11_COUNTER: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x6e8c49fb,
        data2: 0xa371,
        data3: 0x4770,
        data4: [0xb4, 0x40, 0x29, 0x08, 0x60, 0x22, 0xb7, 0x41],
    };
/// The header's `IID_ID3D11CryptoSession`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11cryptosession).
pub const IID_ID3D11_CRYPTO_SESSION: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x9b32f9ad,
        data2: 0xbdcc,
        data3: 0x40a6,
        data4: [0xa3, 0x9d, 0xd5, 0xc8, 0x65, 0x84, 0x57, 0x20],
    };
/// The header's `IID_ID3D11Debug`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nn-d3d11sdklayers-id3d11debug).
pub const IID_ID3D11_DEBUG: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x79cf2233,
        data2: 0x7536,
        data3: 0x4948,
        data4: [0x9d, 0x36, 0x1e, 0x46, 0x92, 0xdc, 0x57, 0x60],
    };
/// The header's `IID_ID3D11DepthStencilState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11depthstencilstate).
pub const IID_ID3D11_DEPTH_STENCIL_STATE: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x03823efb,
        data2: 0x8d8f,
        data3: 0x4e1c,
        data4: [0x9a, 0xa2, 0xf6, 0x4b, 0xb2, 0xcb, 0xfd, 0xf1],
    };
/// The header's `IID_ID3D11DepthStencilView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11depthstencilview).
pub const IID_ID3D11_DEPTH_STENCIL_VIEW: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x9fdac92a,
        data2: 0x1876,
        data3: 0x48c3,
        data4: [0xaf, 0xad, 0x25, 0xb9, 0x4f, 0x84, 0xa9, 0xb6],
    };
/// The header's `IID_ID3D11Device`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11device).
pub const IID_ID3D11_DEVICE: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xdb6f6ddb,
        data2: 0xac77,
        data3: 0x4e88,
        data4: [0x82, 0x53, 0x81, 0x9d, 0xf9, 0xbb, 0xf1, 0x40],
    };
/// The header's `IID_ID3D11Device1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nn-d3d11_1-id3d11device1).
pub const IID_ID3D11_DEVICE1: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xa04bfb29,
        data2: 0x08ef,
        data3: 0x43d6,
        data4: [0xa4, 0x9c, 0xa9, 0xbd, 0xbd, 0xcb, 0xe6, 0x86],
    };
/// The header's `IID_ID3D11Device2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/nn-d3d11_2-id3d11device2).
pub const IID_ID3D11_DEVICE2: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x9d06dffa,
        data2: 0xd1e5,
        data3: 0x4d07,
        data4: [0x83, 0xa8, 0x1b, 0xb1, 0x23, 0xf2, 0xf8, 0x41],
    };
/// The header's `IID_ID3D11Device3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nn-d3d11_3-id3d11device3).
pub const IID_ID3D11_DEVICE3: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xa05c8c37,
        data2: 0xd2c6,
        data3: 0x4732,
        data4: [0xb3, 0xa0, 0x9c, 0xe0, 0xb0, 0xdc, 0x9a, 0xe6],
    };
/// The header's `IID_ID3D11Device4`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/nn-d3d11_4-id3d11device4).
pub const IID_ID3D11_DEVICE4: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x8992ab71,
        data2: 0x02e6,
        data3: 0x4b8d,
        data4: [0xba, 0x48, 0xb0, 0x56, 0xdc, 0xda, 0x42, 0xc4],
    };
/// The header's `IID_ID3D11Device5`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/nn-d3d11_4-id3d11device5).
pub const IID_ID3D11_DEVICE5: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x8ffde202,
        data2: 0xa0e7,
        data3: 0x45df,
        data4: [0x9e, 0x01, 0xe8, 0x37, 0x80, 0x1b, 0x5e, 0xa0],
    };
/// The header's `IID_ID3D11DeviceChild`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11devicechild).
pub const IID_ID3D11_DEVICE_CHILD: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x1841e5c8,
        data2: 0x16b0,
        data3: 0x489b,
        data4: [0xbc, 0xc8, 0x44, 0xcf, 0xb0, 0xd5, 0xde, 0xae],
    };
/// The header's `IID_ID3D11DeviceContext`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11devicecontext).
pub const IID_ID3D11_DEVICE_CONTEXT: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xc0bfa96c,
        data2: 0xe089,
        data3: 0x44fb,
        data4: [0x8e, 0xaf, 0x26, 0xf8, 0x79, 0x61, 0x90, 0xda],
    };
/// The header's `IID_ID3D11DeviceContext1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nn-d3d11_1-id3d11devicecontext1).
pub const IID_ID3D11_DEVICE_CONTEXT1: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xbb2c6faa,
        data2: 0xb5fb,
        data3: 0x4082,
        data4: [0x8e, 0x6b, 0x38, 0x8b, 0x8c, 0xfa, 0x90, 0xe1],
    };
/// The header's `IID_ID3D11DeviceContext2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/nn-d3d11_2-id3d11devicecontext2).
pub const IID_ID3D11_DEVICE_CONTEXT2: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x420d5b32,
        data2: 0xb90c,
        data3: 0x4da4,
        data4: [0xbe, 0xf0, 0x35, 0x9f, 0x6a, 0x24, 0xa8, 0x3a],
    };
/// The header's `IID_ID3D11DeviceContext3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nn-d3d11_3-id3d11devicecontext3).
pub const IID_ID3D11_DEVICE_CONTEXT3: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xb4e3c01d,
        data2: 0xe79e,
        data3: 0x4637,
        data4: [0x91, 0xb2, 0x51, 0x0e, 0x9f, 0x4c, 0x9b, 0x8f],
    };
/// The header's `IID_ID3D11DeviceContext4`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nn-d3d11_3-id3d11devicecontext4).
pub const IID_ID3D11_DEVICE_CONTEXT4: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x917600da,
        data2: 0xf58c,
        data3: 0x4c33,
        data4: [0x98, 0xd8, 0x3e, 0x15, 0xb3, 0x90, 0xfa, 0x24],
    };
/// The header's `IID_ID3D11DomainShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11domainshader).
pub const IID_ID3D11_DOMAIN_SHADER: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xf582c508,
        data2: 0x0f36,
        data3: 0x490c,
        data4: [0x99, 0x77, 0x31, 0xee, 0xce, 0x26, 0x8c, 0xfa],
    };
/// The header's `IID_ID3D11Fence`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nn-d3d11_3-id3d11fence).
pub const IID_ID3D11_FENCE: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xaffde9d1,
        data2: 0x1df7,
        data3: 0x4bb7,
        data4: [0x8a, 0x34, 0x0f, 0x46, 0x25, 0x1d, 0xab, 0x80],
    };
/// The header's `IID_ID3D11GeometryShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11geometryshader).
pub const IID_ID3D11_GEOMETRY_SHADER: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x38325b96,
        data2: 0xeffb,
        data3: 0x4022,
        data4: [0xba, 0x02, 0x2e, 0x79, 0x5b, 0x70, 0x27, 0x5c],
    };
/// The header's `IID_ID3D11HullShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11hullshader).
pub const IID_ID3D11_HULL_SHADER: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x8e5c6061,
        data2: 0x628a,
        data3: 0x4c8e,
        data4: [0x82, 0x64, 0xbb, 0xe4, 0x5c, 0xb3, 0xd5, 0xdd],
    };
/// The header's `IID_ID3D11InfoQueue`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nn-d3d11sdklayers-id3d11infoqueue).
pub const IID_ID3D11_INFO_QUEUE: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x6543dbb6,
        data2: 0x1b48,
        data3: 0x42f5,
        data4: [0xab, 0x82, 0xe9, 0x7e, 0xc7, 0x43, 0x26, 0xf6],
    };
/// The header's `IID_ID3D11InputLayout`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11inputlayout).
pub const IID_ID3D11_INPUT_LAYOUT: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xe4819ddc,
        data2: 0x4cf0,
        data3: 0x4025,
        data4: [0xbd, 0x26, 0x5d, 0xe8, 0x2a, 0x3e, 0x07, 0xb7],
    };
/// The header's `IID_ID3D11Multithread`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/nn-d3d11_4-id3d11multithread).
pub const IID_ID3D11_MULTITHREAD: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x9b7e4e00,
        data2: 0x342c,
        data3: 0x4106,
        data4: [0xa1, 0x9f, 0x4f, 0x27, 0x04, 0xf6, 0x89, 0xf0],
    };
/// The header's `IID_ID3D11PixelShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11pixelshader).
pub const IID_ID3D11_PIXEL_SHADER: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xea82e40d,
        data2: 0x51dc,
        data3: 0x4f33,
        data4: [0x93, 0xd4, 0xdb, 0x7c, 0x91, 0x25, 0xae, 0x8c],
    };
/// The header's `IID_ID3D11Predicate`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11predicate).
pub const IID_ID3D11_PREDICATE: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x9eb576dd,
        data2: 0x9f77,
        data3: 0x4d86,
        data4: [0x81, 0xaa, 0x8b, 0xab, 0x5f, 0xe4, 0x90, 0xe2],
    };
/// The header's `IID_ID3D11Query`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11query).
pub const IID_ID3D11_QUERY: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xd6c00747,
        data2: 0x87b7,
        data3: 0x425e,
        data4: [0xb8, 0x4d, 0x44, 0xd1, 0x08, 0x56, 0x0a, 0xfd],
    };
/// The header's `IID_ID3D11Query1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nn-d3d11_3-id3d11query1).
pub const IID_ID3D11_QUERY1: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x631b4766,
        data2: 0x36dc,
        data3: 0x461d,
        data4: [0x8d, 0xb6, 0xc4, 0x7e, 0x13, 0xe6, 0x09, 0x16],
    };
/// The header's `IID_ID3D11RasterizerState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11rasterizerstate).
pub const IID_ID3D11_RASTERIZER_STATE: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x9bb4ab81,
        data2: 0xab1a,
        data3: 0x4d8f,
        data4: [0xb5, 0x06, 0xfc, 0x04, 0x20, 0x0b, 0x6e, 0xe7],
    };
/// The header's `IID_ID3D11RasterizerState1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nn-d3d11_1-id3d11rasterizerstate1).
pub const IID_ID3D11_RASTERIZER_STATE1: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x1217d7a6,
        data2: 0x5039,
        data3: 0x418c,
        data4: [0xb0, 0x42, 0x9c, 0xbe, 0x25, 0x6a, 0xfd, 0x6e],
    };
/// The header's `IID_ID3D11RasterizerState2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nn-d3d11_3-id3d11rasterizerstate2).
pub const IID_ID3D11_RASTERIZER_STATE2: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x6fbd02fb,
        data2: 0x209f,
        data3: 0x46c4,
        data4: [0xb0, 0x59, 0x2e, 0xd1, 0x55, 0x86, 0xa6, 0xac],
    };
/// The header's `IID_ID3D11RefDefaultTrackingOptions`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nn-d3d11sdklayers-id3d11refdefaulttrackingoptions).
pub const IID_ID3D11_REF_DEFAULT_TRACKING_OPTIONS: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x03916615,
        data2: 0xc644,
        data3: 0x418c,
        data4: [0x9b, 0xf4, 0x75, 0xdb, 0x5b, 0xe6, 0x3c, 0xa0],
    };
/// The header's `IID_ID3D11RefTrackingOptions`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nn-d3d11sdklayers-id3d11reftrackingoptions).
pub const IID_ID3D11_REF_TRACKING_OPTIONS: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x193dacdf,
        data2: 0x0db2,
        data3: 0x4c05,
        data4: [0xa5, 0x5c, 0xef, 0x06, 0xca, 0xc5, 0x6f, 0xd9],
    };
/// The header's `IID_ID3D11RenderTargetView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11rendertargetview).
pub const IID_ID3D11_RENDER_TARGET_VIEW: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xdfdba067,
        data2: 0x0b8d,
        data3: 0x4865,
        data4: [0x87, 0x5b, 0xd7, 0xb4, 0x51, 0x6c, 0xc1, 0x64],
    };
/// The header's `IID_ID3D11RenderTargetView1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nn-d3d11_3-id3d11rendertargetview1).
pub const IID_ID3D11_RENDER_TARGET_VIEW1: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xffbe2e23,
        data2: 0xf011,
        data3: 0x418a,
        data4: [0xac, 0x56, 0x5c, 0xee, 0xd7, 0xc5, 0xb9, 0x4b],
    };
/// The header's `IID_ID3D11Resource`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11resource).
pub const IID_ID3D11_RESOURCE: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xdc8e63f3,
        data2: 0xd12b,
        data3: 0x4952,
        data4: [0xb4, 0x7b, 0x5e, 0x45, 0x02, 0x6a, 0x86, 0x2d],
    };
/// The header's `IID_ID3D11SamplerState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11samplerstate).
pub const IID_ID3D11_SAMPLER_STATE: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xda6fea51,
        data2: 0x564c,
        data3: 0x4487,
        data4: [0x98, 0x10, 0xf0, 0xd0, 0xf9, 0xb4, 0xe3, 0xa5],
    };
/// The header's `IID_ID3D11ShaderResourceView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11shaderresourceview).
pub const IID_ID3D11_SHADER_RESOURCE_VIEW: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xb0e06fe0,
        data2: 0x8192,
        data3: 0x4e1a,
        data4: [0xb1, 0xca, 0x36, 0xd7, 0x41, 0x47, 0x10, 0xb2],
    };
/// The header's `IID_ID3D11ShaderResourceView1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nn-d3d11_3-id3d11shaderresourceview1).
pub const IID_ID3D11_SHADER_RESOURCE_VIEW1: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x91308b87,
        data2: 0x9040,
        data3: 0x411d,
        data4: [0x8c, 0x67, 0xc3, 0x92, 0x53, 0xce, 0x38, 0x02],
    };
/// The header's `IID_ID3D11ShaderTrace`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/nn-d3d11shadertracing-id3d11shadertrace).
pub const IID_ID3D11_SHADER_TRACE: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x36b013e6,
        data2: 0x2811,
        data3: 0x4845,
        data4: [0xba, 0xa7, 0xd6, 0x23, 0xfe, 0x0d, 0xf1, 0x04],
    };
/// The header's `IID_ID3D11ShaderTraceFactory`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/nn-d3d11shadertracing-id3d11shadertracefactory).
pub const IID_ID3D11_SHADER_TRACE_FACTORY: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x1fbad429,
        data2: 0x66ab,
        data3: 0x41cc,
        data4: [0x96, 0x17, 0x66, 0x7a, 0xc1, 0x0e, 0x44, 0x59],
    };
/// The header's `IID_ID3D11SwitchToRef`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nn-d3d11sdklayers-id3d11switchtoref).
pub const IID_ID3D11_SWITCH_TO_REF: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x1ef337e3,
        data2: 0x58e7,
        data3: 0x4f83,
        data4: [0xa6, 0x92, 0xdb, 0x22, 0x1f, 0x5e, 0xd4, 0x7e],
    };
/// The header's `IID_ID3D11Texture1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11texture1d).
pub const IID_ID3D11_TEXTURE_1D: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xf8fb5c27,
        data2: 0xc6b3,
        data3: 0x4f75,
        data4: [0xa4, 0xc8, 0x43, 0x9a, 0xf2, 0xef, 0x56, 0x4c],
    };
/// The header's `IID_ID3D11Texture2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11texture2d).
pub const IID_ID3D11_TEXTURE_2D: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x6f15aaf2,
        data2: 0xd208,
        data3: 0x4e89,
        data4: [0x9a, 0xb4, 0x48, 0x95, 0x35, 0xd3, 0x4f, 0x9c],
    };
/// The header's `IID_ID3D11Texture2D1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nn-d3d11_3-id3d11texture2d1).
pub const IID_ID3D11_TEXTURE_2D1: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x51218251,
        data2: 0x1e33,
        data3: 0x4617,
        data4: [0x9c, 0xcb, 0x4d, 0x3a, 0x43, 0x67, 0xe7, 0xbb],
    };
/// The header's `IID_ID3D11Texture3D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11texture3d).
pub const IID_ID3D11_TEXTURE_3D: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x037e866e,
        data2: 0xf56d,
        data3: 0x4357,
        data4: [0xa8, 0xaf, 0x9d, 0xab, 0xbe, 0x6e, 0x25, 0x0e],
    };
/// The header's `IID_ID3D11Texture3D1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nn-d3d11_3-id3d11texture3d1).
pub const IID_ID3D11_TEXTURE_3D1: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x0c711683,
        data2: 0x2853,
        data3: 0x4846,
        data4: [0x9b, 0xb0, 0xf3, 0xe6, 0x06, 0x39, 0xe4, 0x6a],
    };
/// The header's `IID_ID3D11TracingDevice`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nn-d3d11sdklayers-id3d11tracingdevice).
pub const IID_ID3D11_TRACING_DEVICE: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x1911c771,
        data2: 0x1587,
        data3: 0x413e,
        data4: [0xa7, 0xe0, 0xfb, 0x26, 0xc3, 0xde, 0x02, 0x68],
    };
/// The header's `IID_ID3D11UnorderedAccessView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11unorderedaccessview).
pub const IID_ID3D11_UNORDERED_ACCESS_VIEW: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x28acf509,
        data2: 0x7f5c,
        data3: 0x48f6,
        data4: [0x86, 0x11, 0xf3, 0x16, 0x01, 0x0a, 0x63, 0x80],
    };
/// The header's `IID_ID3D11UnorderedAccessView1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nn-d3d11_3-id3d11unorderedaccessview1).
pub const IID_ID3D11_UNORDERED_ACCESS_VIEW1: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x7b3b6153,
        data2: 0xa886,
        data3: 0x4544,
        data4: [0xab, 0x37, 0x65, 0x37, 0xc8, 0x50, 0x04, 0x03],
    };
/// The header's `IID_ID3D11VertexShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11vertexshader).
pub const IID_ID3D11_VERTEX_SHADER: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x3b301d64,
        data2: 0xd678,
        data3: 0x4289,
        data4: [0x88, 0x97, 0x22, 0xf8, 0x92, 0x8b, 0x72, 0xf3],
    };
/// The header's `IID_ID3D11VideoContext`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11videocontext).
pub const IID_ID3D11_VIDEO_CONTEXT: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x61f21c45,
        data2: 0x3c0e,
        data3: 0x4a74,
        data4: [0x9c, 0xea, 0x67, 0x10, 0x0d, 0x9a, 0xd5, 0xe4],
    };
/// The header's `IID_ID3D11VideoContext1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nn-d3d11_1-id3d11videocontext1).
pub const IID_ID3D11_VIDEO_CONTEXT1: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xa7f026da,
        data2: 0xa5f8,
        data3: 0x4487,
        data4: [0xa5, 0x64, 0x15, 0xe3, 0x43, 0x57, 0x65, 0x1e],
    };
/// The header's `IID_ID3D11VideoContext2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/nn-d3d11_4-id3d11videocontext2).
pub const IID_ID3D11_VIDEO_CONTEXT2: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xc4e7374c,
        data2: 0x6243,
        data3: 0x4d1b,
        data4: [0xae, 0x87, 0x52, 0xb4, 0xf7, 0x40, 0xe2, 0x61],
    };
/// The header's `IID_ID3D11VideoContext3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/nn-d3d11_4-id3d11videocontext3).
pub const IID_ID3D11_VIDEO_CONTEXT3: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xa9e2faa0,
        data2: 0xcb39,
        data3: 0x418f,
        data4: [0xa0, 0xb7, 0xd8, 0xaa, 0xd4, 0xde, 0x67, 0x2e],
    };
/// The header's `IID_ID3D11VideoDecoder`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11videodecoder).
pub const IID_ID3D11_VIDEO_DECODER: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x3c9c5b51,
        data2: 0x995d,
        data3: 0x48d1,
        data4: [0x9b, 0x8d, 0xfa, 0x5c, 0xae, 0xde, 0xd6, 0x5c],
    };
/// The header's `IID_ID3D11VideoDecoderOutputView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11videodecoderoutputview).
pub const IID_ID3D11_VIDEO_DECODER_OUTPUT_VIEW: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xc2931aea,
        data2: 0x2a85,
        data3: 0x4f20,
        data4: [0x86, 0x0f, 0xfb, 0xa1, 0xfd, 0x25, 0x6e, 0x18],
    };
/// The header's `IID_ID3D11VideoDevice`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11videodevice).
pub const IID_ID3D11_VIDEO_DEVICE: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x10ec4d5b,
        data2: 0x975a,
        data3: 0x4689,
        data4: [0xb9, 0xe4, 0xd0, 0xaa, 0xc3, 0x0f, 0xe3, 0x33],
    };
/// The header's `IID_ID3D11VideoDevice1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nn-d3d11_1-id3d11videodevice1).
pub const IID_ID3D11_VIDEO_DEVICE1: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x29da1d51,
        data2: 0x1321,
        data3: 0x4454,
        data4: [0x80, 0x4b, 0xf5, 0xfc, 0x9f, 0x86, 0x1f, 0x0f],
    };
/// The header's `IID_ID3D11VideoDevice2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/nn-d3d11_4-id3d11videodevice2).
pub const IID_ID3D11_VIDEO_DEVICE2: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x59c0cb01,
        data2: 0x35f0,
        data3: 0x4a70,
        data4: [0x8f, 0x67, 0x87, 0x90, 0x5c, 0x90, 0x6a, 0x53],
    };
/// The header's `IID_ID3D11VideoProcessor`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11videoprocessor).
pub const IID_ID3D11_VIDEO_PROCESSOR: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x1d7b0652,
        data2: 0x185f,
        data3: 0x41c6,
        data4: [0x85, 0xce, 0x0c, 0x5b, 0xe3, 0xd4, 0xae, 0x6c],
    };
/// The header's `IID_ID3D11VideoProcessorEnumerator`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11videoprocessorenumerator).
pub const IID_ID3D11_VIDEO_PROCESSOR_ENUMERATOR: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x31627037,
        data2: 0x53ab,
        data3: 0x4200,
        data4: [0x90, 0x61, 0x05, 0xfa, 0xa9, 0xab, 0x45, 0xf9],
    };
/// The header's `IID_ID3D11VideoProcessorEnumerator1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nn-d3d11_1-id3d11videoprocessorenumerator1).
pub const IID_ID3D11_VIDEO_PROCESSOR_ENUMERATOR1: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x465217f2,
        data2: 0x5568,
        data3: 0x43cf,
        data4: [0xb5, 0xb9, 0xf6, 0x1d, 0x54, 0x53, 0x1c, 0xa1],
    };
/// The header's `IID_ID3D11VideoProcessorInputView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11videoprocessorinputview).
pub const IID_ID3D11_VIDEO_PROCESSOR_INPUT_VIEW: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x11ec5a5f,
        data2: 0x51dc,
        data3: 0x4945,
        data4: [0xab, 0x34, 0x6e, 0x8c, 0x21, 0x30, 0x0e, 0xa5],
    };
/// The header's `IID_ID3D11VideoProcessorOutputView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11videoprocessoroutputview).
pub const IID_ID3D11_VIDEO_PROCESSOR_OUTPUT_VIEW: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xa048285e,
        data2: 0x25a9,
        data3: 0x4527,
        data4: [0xbd, 0x93, 0xd6, 0x8b, 0x68, 0xc4, 0x42, 0x54],
    };
/// The header's `IID_ID3D11View`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11view).
pub const IID_ID3D11_VIEW: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x839d1216,
        data2: 0xbb2e,
        data3: 0x412b,
        data4: [0xb7, 0xf4, 0xa9, 0xdb, 0xeb, 0xe0, 0x8e, 0xd1],
    };
/// The header's `IID_ID3DDeviceContextState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nn-d3d11_1-id3ddevicecontextstate).
pub const IID_ID3D_DEVICE_CONTEXT_STATE: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0x5c1e0d8a,
        data2: 0x7c23,
        data3: 0x48f9,
        data4: [0x8c, 0x59, 0xa9, 0x29, 0x58, 0xce, 0xff, 0x11],
    };
/// The header's `IID_ID3DUserDefinedAnnotation`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nn-d3d11_1-id3duserdefinedannotation).
pub const IID_ID3D_USER_DEFINED_ANNOTATION: component_object_model::structures::Guid =
    component_object_model::structures::Guid {
        data1: 0xb2daad8b,
        data2: 0x03d4,
        data3: 0x4dbf,
        data4: [0x95, 0xeb, 0x32, 0xab, 0x4b, 0x63, 0xd0, 0xab],
    };
/// The header's `D3D11_BLEND_DESC1::RenderTarget`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_BLEND_DESC1_RENDERTARGET_LENGTH: usize = 8;
/// The header's `D3D11_BLEND_DESC::RenderTarget`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_BLEND_DESC_RENDERTARGET_LENGTH: usize = 8;
/// The header's `D3D11_COMPUTE_SHADER_TRACE_DESC::ThreadGroupID`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMPUTE_SHADER_TRACE_DESC_THREADGROUPID_LENGTH: usize = 3;
/// The header's `D3D11_COMPUTE_SHADER_TRACE_DESC::ThreadIDInGroup`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_COMPUTE_SHADER_TRACE_DESC_THREADIDINGROUP_LENGTH: usize = 3;
/// The header's `D3D11_KEY_EXCHANGE_HW_PROTECTION_INPUT_DATA::pbInput`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_KEY_EXCHANGE_HW_PROTECTION_INPUT_DATA_PBINPUT_LENGTH: usize = 4;
/// The header's `D3D11_KEY_EXCHANGE_HW_PROTECTION_OUTPUT_DATA::pbOutput`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_KEY_EXCHANGE_HW_PROTECTION_OUTPUT_DATA_PBOUTPUT_LENGTH: usize = 4;
/// The header's `D3D11_OMAC::Omac`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_OMAC_OMAC_LENGTH: usize = 16;
/// The header's `D3D11_SAMPLER_DESC::BorderColor`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_SAMPLER_DESC_BORDERCOLOR_LENGTH: usize = 4;
/// The header's `D3D11_TRACE_REGISTER::_Anonymous_e__Union::Index2D`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TRACE_REGISTER_ANONYMOUS_E_UNION_INDEX2D_LENGTH: usize = 2;
/// The header's `D3D11_TRACE_STATS::DSInputPatchConstantMask`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TRACE_STATS_DSINPUTPATCHCONSTANTMASK_LENGTH: usize = 32;
/// The header's `D3D11_TRACE_STATS::HSOutputPatchConstantMask`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TRACE_STATS_HSOUTPUTPATCHCONSTANTMASK_LENGTH: usize = 32;
/// The header's `D3D11_TRACE_STATS::IndexableTempSize`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TRACE_STATS_INDEXABLETEMPSIZE_LENGTH: usize = 4096;
/// The header's `D3D11_TRACE_STATS::InputMask`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TRACE_STATS_INPUTMASK_LENGTH: usize = 32;
/// The header's `D3D11_TRACE_STATS::OutputMask`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TRACE_STATS_OUTPUTMASK_LENGTH: usize = 32;
/// The header's `D3D11_TRACE_STATS::PixelCoverageMaskAfterA2CSampleMaskDepthStencil`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TRACE_STATS_PIXELCOVERAGEMASKAFTERA2CSAMPLEMASKDEPTHSTENCIL_LENGTH: usize = 4;
/// The header's `D3D11_TRACE_STATS::PixelCoverageMaskAfterA2CSampleMaskDepth`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TRACE_STATS_PIXELCOVERAGEMASKAFTERA2CSAMPLEMASKDEPTH_LENGTH: usize = 4;
/// The header's `D3D11_TRACE_STATS::PixelCoverageMaskAfterA2CSampleMask`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TRACE_STATS_PIXELCOVERAGEMASKAFTERA2CSAMPLEMASK_LENGTH: usize = 4;
/// The header's `D3D11_TRACE_STATS::PixelCoverageMaskAfterShader`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TRACE_STATS_PIXELCOVERAGEMASKAFTERSHADER_LENGTH: usize = 4;
/// The header's `D3D11_TRACE_STATS::PixelCoverageMask`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TRACE_STATS_PIXELCOVERAGEMASK_LENGTH: usize = 4;
/// The header's `D3D11_TRACE_STATS::PixelDiscardedMask`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TRACE_STATS_PIXELDISCARDEDMASK_LENGTH: usize = 4;
/// The header's `D3D11_TRACE_STATS::PixelPosition`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TRACE_STATS_PIXELPOSITION_LENGTH: usize = 8;
/// The header's `D3D11_TRACE_VALUE::Bits`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_TRACE_VALUE_BITS_LENGTH: usize = 4;
/// The header's `D3DX11_FFT_BUFFER_INFO::PrecomputeBufferFloatSizes`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DX11_FFT_BUFFER_INFO_PRECOMPUTEBUFFERFLOATSIZES_LENGTH: usize = 4;
/// The header's `D3DX11_FFT_BUFFER_INFO::TempBufferFloatSizes`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DX11_FFT_BUFFER_INFO_TEMPBUFFERFLOATSIZES_LENGTH: usize = 4;
/// The header's `D3DX11_FFT_DESC::ElementLengths`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DX11_FFT_DESC_ELEMENTLENGTHS_LENGTH: usize = 32;
/// The header's `ID3D11DeviceContext1::ClearView(Color)`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11devicecontext1-clearview).
pub const DEVICE_CONTEXT1_CLEAR_VIEW_COLOR_LENGTH: usize = 4;
/// The header's `ID3D11DeviceContext::ClearRenderTargetView(ColorRGBA)`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-clearrendertargetview).
pub const DEVICE_CONTEXT_CLEAR_RENDER_TARGET_VIEW_COLOR_RGBA_LENGTH: usize = 4;
/// The header's `ID3D11DeviceContext::ClearUnorderedAccessViewFloat(Values)`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-clearunorderedaccessviewfloat).
pub const DEVICE_CONTEXT_CLEAR_UNORDERED_ACCESS_VIEW_FLOAT_VALUES_LENGTH: usize = 4;
/// The header's `ID3D11DeviceContext::ClearUnorderedAccessViewUint(Values)`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-clearunorderedaccessviewuint).
pub const DEVICE_CONTEXT_CLEAR_UNORDERED_ACCESS_VIEW_UINT_VALUES_LENGTH: usize = 4;
/// The header's `ID3D11DeviceContext::OMGetBlendState(BlendFactor)`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-omgetblendstate).
pub const DEVICE_CONTEXT_OM_GET_BLEND_STATE_BLEND_FACTOR_LENGTH: usize = 4;
/// The header's `ID3D11DeviceContext::OMSetBlendState(BlendFactor)`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-omsetblendstate).
pub const DEVICE_CONTEXT_OM_SET_BLEND_STATE_BLEND_FACTOR_LENGTH: usize = 4;
