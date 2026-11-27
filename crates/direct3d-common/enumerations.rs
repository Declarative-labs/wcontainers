//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.

/// The header's `D3D_CBUFFER_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_cbuffer_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CbufferType(pub i32);

/// The header's `D3D_CT_CBUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_cbuffer_type).
pub const D3D_CT_CBUFFER: CbufferType = CbufferType(0);
/// The header's `D3D_CT_TBUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_cbuffer_type).
pub const D3D_CT_TBUFFER: CbufferType = CbufferType(1);
/// The header's `D3D_CT_INTERFACE_POINTERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_cbuffer_type).
pub const D3D_CT_INTERFACE_POINTERS: CbufferType = CbufferType(2);
/// The header's `D3D_CT_RESOURCE_BIND_INFO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_cbuffer_type).
pub const D3D_CT_RESOURCE_BIND_INFO: CbufferType = CbufferType(3);
/// The header's `D3D10_CT_CBUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_cbuffer_type).
pub const D3D10_CT_CBUFFER: CbufferType = CbufferType(0);
/// The header's `D3D10_CT_TBUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_cbuffer_type).
pub const D3D10_CT_TBUFFER: CbufferType = CbufferType(1);
/// The header's `D3D11_CT_CBUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_cbuffer_type).
pub const D3D11_CT_CBUFFER: CbufferType = CbufferType(0);
/// The header's `D3D11_CT_TBUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_cbuffer_type).
pub const D3D11_CT_TBUFFER: CbufferType = CbufferType(1);
/// The header's `D3D11_CT_INTERFACE_POINTERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_cbuffer_type).
pub const D3D11_CT_INTERFACE_POINTERS: CbufferType = CbufferType(2);
/// The header's `D3D11_CT_RESOURCE_BIND_INFO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_cbuffer_type).
pub const D3D11_CT_RESOURCE_BIND_INFO: CbufferType = CbufferType(3);

/// The header's `D3D_DRIVER_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_driver_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DriverType(pub i32);

/// The header's `D3D_DRIVER_TYPE_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_driver_type).
pub const D3D_DRIVER_TYPE_UNKNOWN: DriverType = DriverType(0);
/// The header's `D3D_DRIVER_TYPE_HARDWARE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_driver_type).
pub const D3D_DRIVER_TYPE_HARDWARE: DriverType = DriverType(1);
/// The header's `D3D_DRIVER_TYPE_REFERENCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_driver_type).
pub const D3D_DRIVER_TYPE_REFERENCE: DriverType = DriverType(2);
/// The header's `D3D_DRIVER_TYPE_NULL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_driver_type).
pub const D3D_DRIVER_TYPE_NULL: DriverType = DriverType(3);
/// The header's `D3D_DRIVER_TYPE_SOFTWARE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_driver_type).
pub const D3D_DRIVER_TYPE_SOFTWARE: DriverType = DriverType(4);
/// The header's `D3D_DRIVER_TYPE_WARP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_driver_type).
pub const D3D_DRIVER_TYPE_WARP: DriverType = DriverType(5);

/// The header's `D3D_FEATURE_LEVEL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_feature_level).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FeatureLevel(pub i32);

/// The header's `D3D_FEATURE_LEVEL_1_0_GENERIC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_feature_level).
pub const D3D_FEATURE_LEVEL_1_0_GENERIC: FeatureLevel = FeatureLevel(0x100);
/// The header's `D3D_FEATURE_LEVEL_1_0_CORE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_feature_level).
pub const D3D_FEATURE_LEVEL_1_0_CORE: FeatureLevel = FeatureLevel(0x1000);
/// The header's `D3D_FEATURE_LEVEL_9_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_feature_level).
pub const D3D_FEATURE_LEVEL_9_1: FeatureLevel = FeatureLevel(0x9100);
/// The header's `D3D_FEATURE_LEVEL_9_2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_feature_level).
pub const D3D_FEATURE_LEVEL_9_2: FeatureLevel = FeatureLevel(0x9200);
/// The header's `D3D_FEATURE_LEVEL_9_3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_feature_level).
pub const D3D_FEATURE_LEVEL_9_3: FeatureLevel = FeatureLevel(0x9300);
/// The header's `D3D_FEATURE_LEVEL_10_0`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_feature_level).
pub const D3D_FEATURE_LEVEL_10_0: FeatureLevel = FeatureLevel(0xa000);
/// The header's `D3D_FEATURE_LEVEL_10_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_feature_level).
pub const D3D_FEATURE_LEVEL_10_1: FeatureLevel = FeatureLevel(0xa100);
/// The header's `D3D_FEATURE_LEVEL_11_0`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_feature_level).
pub const D3D_FEATURE_LEVEL_11_0: FeatureLevel = FeatureLevel(0xb000);
/// The header's `D3D_FEATURE_LEVEL_11_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_feature_level).
pub const D3D_FEATURE_LEVEL_11_1: FeatureLevel = FeatureLevel(0xb100);
/// The header's `D3D_FEATURE_LEVEL_12_0`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_feature_level).
pub const D3D_FEATURE_LEVEL_12_0: FeatureLevel = FeatureLevel(0xc000);
/// The header's `D3D_FEATURE_LEVEL_12_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_feature_level).
pub const D3D_FEATURE_LEVEL_12_1: FeatureLevel = FeatureLevel(0xc100);
/// The header's `D3D_FEATURE_LEVEL_12_2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_feature_level).
pub const D3D_FEATURE_LEVEL_12_2: FeatureLevel = FeatureLevel(0xc200);

/// The header's `D3D_FORMAT_COMPONENT_INTERPRETATION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FormatComponentInterpretation(pub i32);

/// The header's `D3DFCI_TYPELESS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DFCI_TYPELESS: FormatComponentInterpretation = FormatComponentInterpretation(0);
/// The header's `D3DFCI_FLOAT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DFCI_FLOAT: FormatComponentInterpretation = FormatComponentInterpretation(-4);
/// The header's `D3DFCI_SNORM`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DFCI_SNORM: FormatComponentInterpretation = FormatComponentInterpretation(-3);
/// The header's `D3DFCI_UNORM`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DFCI_UNORM: FormatComponentInterpretation = FormatComponentInterpretation(-2);
/// The header's `D3DFCI_SINT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DFCI_SINT: FormatComponentInterpretation = FormatComponentInterpretation(-1);
/// The header's `D3DFCI_UINT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DFCI_UINT: FormatComponentInterpretation = FormatComponentInterpretation(1);
/// The header's `D3DFCI_UNORM_SRGB`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DFCI_UNORM_SRGB: FormatComponentInterpretation = FormatComponentInterpretation(2);
/// The header's `D3DFCI_BIASED_FIXED_2_8`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DFCI_BIASED_FIXED_2_8: FormatComponentInterpretation = FormatComponentInterpretation(3);

/// The header's `D3D_FORMAT_COMPONENT_NAME`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FormatComponentName(pub i32);

/// The header's `D3DFCN_R`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DFCN_R: FormatComponentName = FormatComponentName(-4);
/// The header's `D3DFCN_G`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DFCN_G: FormatComponentName = FormatComponentName(-3);
/// The header's `D3DFCN_B`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DFCN_B: FormatComponentName = FormatComponentName(-2);
/// The header's `D3DFCN_A`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DFCN_A: FormatComponentName = FormatComponentName(-1);
/// The header's `D3DFCN_D`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DFCN_D: FormatComponentName = FormatComponentName(0);
/// The header's `D3DFCN_S`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DFCN_S: FormatComponentName = FormatComponentName(1);
/// The header's `D3DFCN_X`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DFCN_X: FormatComponentName = FormatComponentName(2);

/// The header's `D3D_FORMAT_LAYOUT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FormatLayout(pub i32);

/// The header's `D3DFL_STANDARD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DFL_STANDARD: FormatLayout = FormatLayout(0);
/// The header's `D3DFL_CUSTOM`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DFL_CUSTOM: FormatLayout = FormatLayout(-1);

/// The header's `D3D_FORMAT_TYPE_LEVEL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FormatTypeLevel(pub i32);

/// The header's `D3DFTL_NO_TYPE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DFTL_NO_TYPE: FormatTypeLevel = FormatTypeLevel(0);
/// The header's `D3DFTL_PARTIAL_TYPE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DFTL_PARTIAL_TYPE: FormatTypeLevel = FormatTypeLevel(-2);
/// The header's `D3DFTL_FULL_TYPE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3DFTL_FULL_TYPE: FormatTypeLevel = FormatTypeLevel(-1);

/// The header's `D3D_INCLUDE_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_include_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct IncludeType(pub i32);

/// The header's `D3D_INCLUDE_LOCAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_include_type).
pub const D3D_INCLUDE_LOCAL: IncludeType = IncludeType(0);
/// The header's `D3D_INCLUDE_SYSTEM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_include_type).
pub const D3D_INCLUDE_SYSTEM: IncludeType = IncludeType(1);
/// The header's `D3D10_INCLUDE_LOCAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_include_type).
pub const D3D10_INCLUDE_LOCAL: IncludeType = IncludeType(0);
/// The header's `D3D10_INCLUDE_SYSTEM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_include_type).
pub const D3D10_INCLUDE_SYSTEM: IncludeType = IncludeType(1);

/// The header's `D3D_INTERPOLATION_MODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_interpolation_mode).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct InterpolationMode(pub i32);

/// The header's `D3D_INTERPOLATION_UNDEFINED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_interpolation_mode).
pub const D3D_INTERPOLATION_UNDEFINED: InterpolationMode = InterpolationMode(0);
/// The header's `D3D_INTERPOLATION_CONSTANT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_interpolation_mode).
pub const D3D_INTERPOLATION_CONSTANT: InterpolationMode = InterpolationMode(1);
/// The header's `D3D_INTERPOLATION_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_interpolation_mode).
pub const D3D_INTERPOLATION_LINEAR: InterpolationMode = InterpolationMode(2);
/// The header's `D3D_INTERPOLATION_LINEAR_CENTROID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_interpolation_mode).
pub const D3D_INTERPOLATION_LINEAR_CENTROID: InterpolationMode = InterpolationMode(3);
/// The header's `D3D_INTERPOLATION_LINEAR_NOPERSPECTIVE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_interpolation_mode).
pub const D3D_INTERPOLATION_LINEAR_NOPERSPECTIVE: InterpolationMode = InterpolationMode(4);
/// The header's `D3D_INTERPOLATION_LINEAR_NOPERSPECTIVE_CENTROID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_interpolation_mode).
pub const D3D_INTERPOLATION_LINEAR_NOPERSPECTIVE_CENTROID: InterpolationMode = InterpolationMode(5);
/// The header's `D3D_INTERPOLATION_LINEAR_SAMPLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_interpolation_mode).
pub const D3D_INTERPOLATION_LINEAR_SAMPLE: InterpolationMode = InterpolationMode(6);
/// The header's `D3D_INTERPOLATION_LINEAR_NOPERSPECTIVE_SAMPLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_interpolation_mode).
pub const D3D_INTERPOLATION_LINEAR_NOPERSPECTIVE_SAMPLE: InterpolationMode = InterpolationMode(7);

/// The header's `D3D_MIN_PRECISION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_min_precision).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MinPrecision(pub i32);

/// The header's `D3D_MIN_PRECISION_DEFAULT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_min_precision).
pub const D3D_MIN_PRECISION_DEFAULT: MinPrecision = MinPrecision(0);
/// The header's `D3D_MIN_PRECISION_FLOAT_16`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_min_precision).
pub const D3D_MIN_PRECISION_FLOAT_16: MinPrecision = MinPrecision(1);
/// The header's `D3D_MIN_PRECISION_FLOAT_2_8`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_min_precision).
pub const D3D_MIN_PRECISION_FLOAT_2_8: MinPrecision = MinPrecision(2);
/// The header's `D3D_MIN_PRECISION_RESERVED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_min_precision).
pub const D3D_MIN_PRECISION_RESERVED: MinPrecision = MinPrecision(3);
/// The header's `D3D_MIN_PRECISION_SINT_16`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_min_precision).
pub const D3D_MIN_PRECISION_SINT_16: MinPrecision = MinPrecision(4);
/// The header's `D3D_MIN_PRECISION_UINT_16`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_min_precision).
pub const D3D_MIN_PRECISION_UINT_16: MinPrecision = MinPrecision(5);
/// The header's `D3D_MIN_PRECISION_ANY_16`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_min_precision).
pub const D3D_MIN_PRECISION_ANY_16: MinPrecision = MinPrecision(240);
/// The header's `D3D_MIN_PRECISION_ANY_10`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_min_precision).
pub const D3D_MIN_PRECISION_ANY_10: MinPrecision = MinPrecision(241);

/// The header's `D3D_NAME`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Name(pub i32);

/// The header's `D3D_NAME_UNDEFINED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D_NAME_UNDEFINED: Name = Name(0);
/// The header's `D3D_NAME_POSITION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D_NAME_POSITION: Name = Name(1);
/// The header's `D3D_NAME_CLIP_DISTANCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D_NAME_CLIP_DISTANCE: Name = Name(2);
/// The header's `D3D_NAME_CULL_DISTANCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D_NAME_CULL_DISTANCE: Name = Name(3);
/// The header's `D3D_NAME_RENDER_TARGET_ARRAY_INDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D_NAME_RENDER_TARGET_ARRAY_INDEX: Name = Name(4);
/// The header's `D3D_NAME_VIEWPORT_ARRAY_INDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D_NAME_VIEWPORT_ARRAY_INDEX: Name = Name(5);
/// The header's `D3D_NAME_VERTEX_ID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D_NAME_VERTEX_ID: Name = Name(6);
/// The header's `D3D_NAME_PRIMITIVE_ID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D_NAME_PRIMITIVE_ID: Name = Name(7);
/// The header's `D3D_NAME_INSTANCE_ID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D_NAME_INSTANCE_ID: Name = Name(8);
/// The header's `D3D_NAME_IS_FRONT_FACE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D_NAME_IS_FRONT_FACE: Name = Name(9);
/// The header's `D3D_NAME_SAMPLE_INDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D_NAME_SAMPLE_INDEX: Name = Name(10);
/// The header's `D3D_NAME_FINAL_QUAD_EDGE_TESSFACTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D_NAME_FINAL_QUAD_EDGE_TESSFACTOR: Name = Name(11);
/// The header's `D3D_NAME_FINAL_QUAD_INSIDE_TESSFACTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D_NAME_FINAL_QUAD_INSIDE_TESSFACTOR: Name = Name(12);
/// The header's `D3D_NAME_FINAL_TRI_EDGE_TESSFACTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D_NAME_FINAL_TRI_EDGE_TESSFACTOR: Name = Name(13);
/// The header's `D3D_NAME_FINAL_TRI_INSIDE_TESSFACTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D_NAME_FINAL_TRI_INSIDE_TESSFACTOR: Name = Name(14);
/// The header's `D3D_NAME_FINAL_LINE_DETAIL_TESSFACTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D_NAME_FINAL_LINE_DETAIL_TESSFACTOR: Name = Name(15);
/// The header's `D3D_NAME_FINAL_LINE_DENSITY_TESSFACTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D_NAME_FINAL_LINE_DENSITY_TESSFACTOR: Name = Name(16);
/// The header's `D3D_NAME_BARYCENTRICS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D_NAME_BARYCENTRICS: Name = Name(23);
/// The header's `D3D_NAME_SHADINGRATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D_NAME_SHADINGRATE: Name = Name(24);
/// The header's `D3D_NAME_CULLPRIMITIVE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D_NAME_CULLPRIMITIVE: Name = Name(25);
/// The header's `D3D_NAME_TARGET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D_NAME_TARGET: Name = Name(64);
/// The header's `D3D_NAME_DEPTH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D_NAME_DEPTH: Name = Name(65);
/// The header's `D3D_NAME_COVERAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D_NAME_COVERAGE: Name = Name(66);
/// The header's `D3D_NAME_DEPTH_GREATER_EQUAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D_NAME_DEPTH_GREATER_EQUAL: Name = Name(67);
/// The header's `D3D_NAME_DEPTH_LESS_EQUAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D_NAME_DEPTH_LESS_EQUAL: Name = Name(68);
/// The header's `D3D_NAME_STENCIL_REF`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D_NAME_STENCIL_REF: Name = Name(69);
/// The header's `D3D_NAME_INNER_COVERAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D_NAME_INNER_COVERAGE: Name = Name(70);
/// The header's `D3D10_NAME_UNDEFINED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D10_NAME_UNDEFINED: Name = Name(0);
/// The header's `D3D10_NAME_POSITION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D10_NAME_POSITION: Name = Name(1);
/// The header's `D3D10_NAME_CLIP_DISTANCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D10_NAME_CLIP_DISTANCE: Name = Name(2);
/// The header's `D3D10_NAME_CULL_DISTANCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D10_NAME_CULL_DISTANCE: Name = Name(3);
/// The header's `D3D10_NAME_RENDER_TARGET_ARRAY_INDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D10_NAME_RENDER_TARGET_ARRAY_INDEX: Name = Name(4);
/// The header's `D3D10_NAME_VIEWPORT_ARRAY_INDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D10_NAME_VIEWPORT_ARRAY_INDEX: Name = Name(5);
/// The header's `D3D10_NAME_VERTEX_ID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D10_NAME_VERTEX_ID: Name = Name(6);
/// The header's `D3D10_NAME_PRIMITIVE_ID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D10_NAME_PRIMITIVE_ID: Name = Name(7);
/// The header's `D3D10_NAME_INSTANCE_ID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D10_NAME_INSTANCE_ID: Name = Name(8);
/// The header's `D3D10_NAME_IS_FRONT_FACE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D10_NAME_IS_FRONT_FACE: Name = Name(9);
/// The header's `D3D10_NAME_SAMPLE_INDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D10_NAME_SAMPLE_INDEX: Name = Name(10);
/// The header's `D3D10_NAME_TARGET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D10_NAME_TARGET: Name = Name(64);
/// The header's `D3D10_NAME_DEPTH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D10_NAME_DEPTH: Name = Name(65);
/// The header's `D3D10_NAME_COVERAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D10_NAME_COVERAGE: Name = Name(66);
/// The header's `D3D11_NAME_FINAL_QUAD_EDGE_TESSFACTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D11_NAME_FINAL_QUAD_EDGE_TESSFACTOR: Name = Name(11);
/// The header's `D3D11_NAME_FINAL_QUAD_INSIDE_TESSFACTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D11_NAME_FINAL_QUAD_INSIDE_TESSFACTOR: Name = Name(12);
/// The header's `D3D11_NAME_FINAL_TRI_EDGE_TESSFACTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D11_NAME_FINAL_TRI_EDGE_TESSFACTOR: Name = Name(13);
/// The header's `D3D11_NAME_FINAL_TRI_INSIDE_TESSFACTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D11_NAME_FINAL_TRI_INSIDE_TESSFACTOR: Name = Name(14);
/// The header's `D3D11_NAME_FINAL_LINE_DETAIL_TESSFACTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D11_NAME_FINAL_LINE_DETAIL_TESSFACTOR: Name = Name(15);
/// The header's `D3D11_NAME_FINAL_LINE_DENSITY_TESSFACTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D11_NAME_FINAL_LINE_DENSITY_TESSFACTOR: Name = Name(16);
/// The header's `D3D11_NAME_DEPTH_GREATER_EQUAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D11_NAME_DEPTH_GREATER_EQUAL: Name = Name(67);
/// The header's `D3D11_NAME_DEPTH_LESS_EQUAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D11_NAME_DEPTH_LESS_EQUAL: Name = Name(68);
/// The header's `D3D11_NAME_STENCIL_REF`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D11_NAME_STENCIL_REF: Name = Name(69);
/// The header's `D3D11_NAME_INNER_COVERAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D11_NAME_INNER_COVERAGE: Name = Name(70);
/// The header's `D3D12_NAME_BARYCENTRICS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D12_NAME_BARYCENTRICS: Name = Name(23);
/// The header's `D3D12_NAME_SHADINGRATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D12_NAME_SHADINGRATE: Name = Name(24);
/// The header's `D3D12_NAME_CULLPRIMITIVE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_name).
pub const D3D12_NAME_CULLPRIMITIVE: Name = Name(25);

/// The header's `D3D_PARAMETER_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_parameter_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ParameterFlags(pub i32);

/// The header's `D3D_PF_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_parameter_flags).
pub const D3D_PF_NONE: ParameterFlags = ParameterFlags(0);
/// The header's `D3D_PF_IN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_parameter_flags).
pub const D3D_PF_IN: ParameterFlags = ParameterFlags(1);
/// The header's `D3D_PF_OUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_parameter_flags).
pub const D3D_PF_OUT: ParameterFlags = ParameterFlags(2);

/// The header's `D3D_PRIMITIVE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Primitive(pub i32);

/// The header's `D3D_PRIMITIVE_UNDEFINED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_UNDEFINED: Primitive = Primitive(0);
/// The header's `D3D_PRIMITIVE_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_POINT: Primitive = Primitive(1);
/// The header's `D3D_PRIMITIVE_LINE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_LINE: Primitive = Primitive(2);
/// The header's `D3D_PRIMITIVE_TRIANGLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_TRIANGLE: Primitive = Primitive(3);
/// The header's `D3D_PRIMITIVE_LINE_ADJ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_LINE_ADJ: Primitive = Primitive(6);
/// The header's `D3D_PRIMITIVE_TRIANGLE_ADJ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_TRIANGLE_ADJ: Primitive = Primitive(7);
/// The header's `D3D_PRIMITIVE_1_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_1_CONTROL_POINT_PATCH: Primitive = Primitive(8);
/// The header's `D3D_PRIMITIVE_2_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_2_CONTROL_POINT_PATCH: Primitive = Primitive(9);
/// The header's `D3D_PRIMITIVE_3_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_3_CONTROL_POINT_PATCH: Primitive = Primitive(10);
/// The header's `D3D_PRIMITIVE_4_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_4_CONTROL_POINT_PATCH: Primitive = Primitive(11);
/// The header's `D3D_PRIMITIVE_5_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_5_CONTROL_POINT_PATCH: Primitive = Primitive(12);
/// The header's `D3D_PRIMITIVE_6_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_6_CONTROL_POINT_PATCH: Primitive = Primitive(13);
/// The header's `D3D_PRIMITIVE_7_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_7_CONTROL_POINT_PATCH: Primitive = Primitive(14);
/// The header's `D3D_PRIMITIVE_8_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_8_CONTROL_POINT_PATCH: Primitive = Primitive(15);
/// The header's `D3D_PRIMITIVE_9_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_9_CONTROL_POINT_PATCH: Primitive = Primitive(16);
/// The header's `D3D_PRIMITIVE_10_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_10_CONTROL_POINT_PATCH: Primitive = Primitive(17);
/// The header's `D3D_PRIMITIVE_11_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_11_CONTROL_POINT_PATCH: Primitive = Primitive(18);
/// The header's `D3D_PRIMITIVE_12_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_12_CONTROL_POINT_PATCH: Primitive = Primitive(19);
/// The header's `D3D_PRIMITIVE_13_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_13_CONTROL_POINT_PATCH: Primitive = Primitive(20);
/// The header's `D3D_PRIMITIVE_14_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_14_CONTROL_POINT_PATCH: Primitive = Primitive(21);
/// The header's `D3D_PRIMITIVE_15_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_15_CONTROL_POINT_PATCH: Primitive = Primitive(22);
/// The header's `D3D_PRIMITIVE_16_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_16_CONTROL_POINT_PATCH: Primitive = Primitive(23);
/// The header's `D3D_PRIMITIVE_17_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_17_CONTROL_POINT_PATCH: Primitive = Primitive(24);
/// The header's `D3D_PRIMITIVE_18_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_18_CONTROL_POINT_PATCH: Primitive = Primitive(25);
/// The header's `D3D_PRIMITIVE_19_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_19_CONTROL_POINT_PATCH: Primitive = Primitive(26);
/// The header's `D3D_PRIMITIVE_20_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_20_CONTROL_POINT_PATCH: Primitive = Primitive(27);
/// The header's `D3D_PRIMITIVE_21_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_21_CONTROL_POINT_PATCH: Primitive = Primitive(28);
/// The header's `D3D_PRIMITIVE_22_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_22_CONTROL_POINT_PATCH: Primitive = Primitive(29);
/// The header's `D3D_PRIMITIVE_23_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_23_CONTROL_POINT_PATCH: Primitive = Primitive(30);
/// The header's `D3D_PRIMITIVE_24_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_24_CONTROL_POINT_PATCH: Primitive = Primitive(31);
/// The header's `D3D_PRIMITIVE_25_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_25_CONTROL_POINT_PATCH: Primitive = Primitive(32);
/// The header's `D3D_PRIMITIVE_26_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_26_CONTROL_POINT_PATCH: Primitive = Primitive(33);
/// The header's `D3D_PRIMITIVE_27_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_27_CONTROL_POINT_PATCH: Primitive = Primitive(34);
/// The header's `D3D_PRIMITIVE_28_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_28_CONTROL_POINT_PATCH: Primitive = Primitive(35);
/// The header's `D3D_PRIMITIVE_29_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_29_CONTROL_POINT_PATCH: Primitive = Primitive(36);
/// The header's `D3D_PRIMITIVE_30_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_30_CONTROL_POINT_PATCH: Primitive = Primitive(37);
/// The header's `D3D_PRIMITIVE_31_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_31_CONTROL_POINT_PATCH: Primitive = Primitive(38);
/// The header's `D3D_PRIMITIVE_32_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D_PRIMITIVE_32_CONTROL_POINT_PATCH: Primitive = Primitive(39);
/// The header's `D3D10_PRIMITIVE_UNDEFINED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D10_PRIMITIVE_UNDEFINED: Primitive = Primitive(0);
/// The header's `D3D10_PRIMITIVE_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D10_PRIMITIVE_POINT: Primitive = Primitive(1);
/// The header's `D3D10_PRIMITIVE_LINE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D10_PRIMITIVE_LINE: Primitive = Primitive(2);
/// The header's `D3D10_PRIMITIVE_TRIANGLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D10_PRIMITIVE_TRIANGLE: Primitive = Primitive(3);
/// The header's `D3D10_PRIMITIVE_LINE_ADJ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D10_PRIMITIVE_LINE_ADJ: Primitive = Primitive(6);
/// The header's `D3D10_PRIMITIVE_TRIANGLE_ADJ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D10_PRIMITIVE_TRIANGLE_ADJ: Primitive = Primitive(7);
/// The header's `D3D11_PRIMITIVE_UNDEFINED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_UNDEFINED: Primitive = Primitive(0);
/// The header's `D3D11_PRIMITIVE_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_POINT: Primitive = Primitive(1);
/// The header's `D3D11_PRIMITIVE_LINE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_LINE: Primitive = Primitive(2);
/// The header's `D3D11_PRIMITIVE_TRIANGLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_TRIANGLE: Primitive = Primitive(3);
/// The header's `D3D11_PRIMITIVE_LINE_ADJ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_LINE_ADJ: Primitive = Primitive(6);
/// The header's `D3D11_PRIMITIVE_TRIANGLE_ADJ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_TRIANGLE_ADJ: Primitive = Primitive(7);
/// The header's `D3D11_PRIMITIVE_1_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_1_CONTROL_POINT_PATCH: Primitive = Primitive(8);
/// The header's `D3D11_PRIMITIVE_2_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_2_CONTROL_POINT_PATCH: Primitive = Primitive(9);
/// The header's `D3D11_PRIMITIVE_3_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_3_CONTROL_POINT_PATCH: Primitive = Primitive(10);
/// The header's `D3D11_PRIMITIVE_4_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_4_CONTROL_POINT_PATCH: Primitive = Primitive(11);
/// The header's `D3D11_PRIMITIVE_5_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_5_CONTROL_POINT_PATCH: Primitive = Primitive(12);
/// The header's `D3D11_PRIMITIVE_6_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_6_CONTROL_POINT_PATCH: Primitive = Primitive(13);
/// The header's `D3D11_PRIMITIVE_7_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_7_CONTROL_POINT_PATCH: Primitive = Primitive(14);
/// The header's `D3D11_PRIMITIVE_8_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_8_CONTROL_POINT_PATCH: Primitive = Primitive(15);
/// The header's `D3D11_PRIMITIVE_9_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_9_CONTROL_POINT_PATCH: Primitive = Primitive(16);
/// The header's `D3D11_PRIMITIVE_10_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_10_CONTROL_POINT_PATCH: Primitive = Primitive(17);
/// The header's `D3D11_PRIMITIVE_11_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_11_CONTROL_POINT_PATCH: Primitive = Primitive(18);
/// The header's `D3D11_PRIMITIVE_12_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_12_CONTROL_POINT_PATCH: Primitive = Primitive(19);
/// The header's `D3D11_PRIMITIVE_13_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_13_CONTROL_POINT_PATCH: Primitive = Primitive(20);
/// The header's `D3D11_PRIMITIVE_14_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_14_CONTROL_POINT_PATCH: Primitive = Primitive(21);
/// The header's `D3D11_PRIMITIVE_15_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_15_CONTROL_POINT_PATCH: Primitive = Primitive(22);
/// The header's `D3D11_PRIMITIVE_16_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_16_CONTROL_POINT_PATCH: Primitive = Primitive(23);
/// The header's `D3D11_PRIMITIVE_17_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_17_CONTROL_POINT_PATCH: Primitive = Primitive(24);
/// The header's `D3D11_PRIMITIVE_18_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_18_CONTROL_POINT_PATCH: Primitive = Primitive(25);
/// The header's `D3D11_PRIMITIVE_19_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_19_CONTROL_POINT_PATCH: Primitive = Primitive(26);
/// The header's `D3D11_PRIMITIVE_20_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_20_CONTROL_POINT_PATCH: Primitive = Primitive(27);
/// The header's `D3D11_PRIMITIVE_21_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_21_CONTROL_POINT_PATCH: Primitive = Primitive(28);
/// The header's `D3D11_PRIMITIVE_22_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_22_CONTROL_POINT_PATCH: Primitive = Primitive(29);
/// The header's `D3D11_PRIMITIVE_23_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_23_CONTROL_POINT_PATCH: Primitive = Primitive(30);
/// The header's `D3D11_PRIMITIVE_24_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_24_CONTROL_POINT_PATCH: Primitive = Primitive(31);
/// The header's `D3D11_PRIMITIVE_25_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_25_CONTROL_POINT_PATCH: Primitive = Primitive(32);
/// The header's `D3D11_PRIMITIVE_26_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_26_CONTROL_POINT_PATCH: Primitive = Primitive(33);
/// The header's `D3D11_PRIMITIVE_27_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_27_CONTROL_POINT_PATCH: Primitive = Primitive(34);
/// The header's `D3D11_PRIMITIVE_28_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_28_CONTROL_POINT_PATCH: Primitive = Primitive(35);
/// The header's `D3D11_PRIMITIVE_29_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_29_CONTROL_POINT_PATCH: Primitive = Primitive(36);
/// The header's `D3D11_PRIMITIVE_30_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_30_CONTROL_POINT_PATCH: Primitive = Primitive(37);
/// The header's `D3D11_PRIMITIVE_31_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_31_CONTROL_POINT_PATCH: Primitive = Primitive(38);
/// The header's `D3D11_PRIMITIVE_32_CONTROL_POINT_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive).
pub const D3D11_PRIMITIVE_32_CONTROL_POINT_PATCH: Primitive = Primitive(39);

/// The header's `D3D_PRIMITIVE_TOPOLOGY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PrimitiveTopology(pub i32);

/// The header's `D3D_PRIMITIVE_TOPOLOGY_UNDEFINED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_UNDEFINED: PrimitiveTopology = PrimitiveTopology(0);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_POINTLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_POINTLIST: PrimitiveTopology = PrimitiveTopology(1);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_LINELIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_LINELIST: PrimitiveTopology = PrimitiveTopology(2);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_LINESTRIP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_LINESTRIP: PrimitiveTopology = PrimitiveTopology(3);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST: PrimitiveTopology = PrimitiveTopology(4);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP: PrimitiveTopology = PrimitiveTopology(5);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_TRIANGLEFAN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_TRIANGLEFAN: PrimitiveTopology = PrimitiveTopology(6);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_LINELIST_ADJ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_LINELIST_ADJ: PrimitiveTopology = PrimitiveTopology(10);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_LINESTRIP_ADJ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_LINESTRIP_ADJ: PrimitiveTopology = PrimitiveTopology(11);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST_ADJ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST_ADJ: PrimitiveTopology = PrimitiveTopology(12);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP_ADJ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP_ADJ: PrimitiveTopology = PrimitiveTopology(13);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_1_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_1_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(33);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_2_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_2_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(34);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_3_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_3_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(35);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_4_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_4_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(36);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_5_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_5_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(37);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_6_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_6_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(38);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_7_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_7_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(39);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_8_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_8_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(40);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_9_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_9_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(41);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_10_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_10_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(42);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_11_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_11_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(43);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_12_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_12_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(44);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_13_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_13_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(45);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_14_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_14_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(46);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_15_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_15_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(47);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_16_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_16_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(48);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_17_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_17_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(49);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_18_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_18_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(50);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_19_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_19_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(51);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_20_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_20_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(52);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_21_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_21_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(53);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_22_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_22_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(54);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_23_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_23_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(55);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_24_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_24_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(56);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_25_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_25_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(57);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_26_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_26_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(58);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_27_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_27_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(59);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_28_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_28_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(60);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_29_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_29_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(61);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_30_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_30_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(62);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_31_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_31_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(63);
/// The header's `D3D_PRIMITIVE_TOPOLOGY_32_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D_PRIMITIVE_TOPOLOGY_32_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(64);
/// The header's `D3D10_PRIMITIVE_TOPOLOGY_UNDEFINED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D10_PRIMITIVE_TOPOLOGY_UNDEFINED: PrimitiveTopology = PrimitiveTopology(0);
/// The header's `D3D10_PRIMITIVE_TOPOLOGY_POINTLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D10_PRIMITIVE_TOPOLOGY_POINTLIST: PrimitiveTopology = PrimitiveTopology(1);
/// The header's `D3D10_PRIMITIVE_TOPOLOGY_LINELIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D10_PRIMITIVE_TOPOLOGY_LINELIST: PrimitiveTopology = PrimitiveTopology(2);
/// The header's `D3D10_PRIMITIVE_TOPOLOGY_LINESTRIP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D10_PRIMITIVE_TOPOLOGY_LINESTRIP: PrimitiveTopology = PrimitiveTopology(3);
/// The header's `D3D10_PRIMITIVE_TOPOLOGY_TRIANGLELIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D10_PRIMITIVE_TOPOLOGY_TRIANGLELIST: PrimitiveTopology = PrimitiveTopology(4);
/// The header's `D3D10_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D10_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP: PrimitiveTopology = PrimitiveTopology(5);
/// The header's `D3D10_PRIMITIVE_TOPOLOGY_LINELIST_ADJ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D10_PRIMITIVE_TOPOLOGY_LINELIST_ADJ: PrimitiveTopology = PrimitiveTopology(10);
/// The header's `D3D10_PRIMITIVE_TOPOLOGY_LINESTRIP_ADJ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D10_PRIMITIVE_TOPOLOGY_LINESTRIP_ADJ: PrimitiveTopology = PrimitiveTopology(11);
/// The header's `D3D10_PRIMITIVE_TOPOLOGY_TRIANGLELIST_ADJ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D10_PRIMITIVE_TOPOLOGY_TRIANGLELIST_ADJ: PrimitiveTopology = PrimitiveTopology(12);
/// The header's `D3D10_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP_ADJ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D10_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP_ADJ: PrimitiveTopology = PrimitiveTopology(13);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_UNDEFINED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_UNDEFINED: PrimitiveTopology = PrimitiveTopology(0);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_POINTLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_POINTLIST: PrimitiveTopology = PrimitiveTopology(1);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_LINELIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_LINELIST: PrimitiveTopology = PrimitiveTopology(2);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_LINESTRIP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_LINESTRIP: PrimitiveTopology = PrimitiveTopology(3);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST: PrimitiveTopology = PrimitiveTopology(4);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP: PrimitiveTopology = PrimitiveTopology(5);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_LINELIST_ADJ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_LINELIST_ADJ: PrimitiveTopology = PrimitiveTopology(10);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_LINESTRIP_ADJ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_LINESTRIP_ADJ: PrimitiveTopology = PrimitiveTopology(11);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST_ADJ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST_ADJ: PrimitiveTopology = PrimitiveTopology(12);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP_ADJ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP_ADJ: PrimitiveTopology = PrimitiveTopology(13);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_1_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_1_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(33);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_2_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_2_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(34);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_3_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_3_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(35);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_4_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_4_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(36);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_5_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_5_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(37);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_6_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_6_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(38);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_7_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_7_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(39);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_8_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_8_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(40);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_9_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_9_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(41);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_10_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_10_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(42);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_11_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_11_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(43);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_12_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_12_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(44);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_13_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_13_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(45);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_14_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_14_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(46);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_15_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_15_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(47);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_16_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_16_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(48);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_17_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_17_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(49);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_18_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_18_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(50);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_19_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_19_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(51);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_20_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_20_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(52);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_21_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_21_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(53);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_22_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_22_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(54);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_23_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_23_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(55);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_24_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_24_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(56);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_25_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_25_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(57);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_26_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_26_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(58);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_27_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_27_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(59);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_28_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_28_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(60);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_29_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_29_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(61);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_30_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_30_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(62);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_31_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_31_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(63);
/// The header's `D3D11_PRIMITIVE_TOPOLOGY_32_CONTROL_POINT_PATCHLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_primitive_topology).
pub const D3D11_PRIMITIVE_TOPOLOGY_32_CONTROL_POINT_PATCHLIST: PrimitiveTopology =
    PrimitiveTopology(64);

/// The header's `D3D_REGISTER_COMPONENT_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_register_component_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RegisterComponentType(pub i32);

/// The header's `D3D_REGISTER_COMPONENT_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_register_component_type).
pub const D3D_REGISTER_COMPONENT_UNKNOWN: RegisterComponentType = RegisterComponentType(0);
/// The header's `D3D_REGISTER_COMPONENT_UINT32`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_register_component_type).
pub const D3D_REGISTER_COMPONENT_UINT32: RegisterComponentType = RegisterComponentType(1);
/// The header's `D3D_REGISTER_COMPONENT_SINT32`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_register_component_type).
pub const D3D_REGISTER_COMPONENT_SINT32: RegisterComponentType = RegisterComponentType(2);
/// The header's `D3D_REGISTER_COMPONENT_FLOAT32`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_register_component_type).
pub const D3D_REGISTER_COMPONENT_FLOAT32: RegisterComponentType = RegisterComponentType(3);
/// The header's `D3D_REGISTER_COMPONENT_UINT16`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_register_component_type).
pub const D3D_REGISTER_COMPONENT_UINT16: RegisterComponentType = RegisterComponentType(4);
/// The header's `D3D_REGISTER_COMPONENT_SINT16`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_register_component_type).
pub const D3D_REGISTER_COMPONENT_SINT16: RegisterComponentType = RegisterComponentType(5);
/// The header's `D3D_REGISTER_COMPONENT_FLOAT16`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_register_component_type).
pub const D3D_REGISTER_COMPONENT_FLOAT16: RegisterComponentType = RegisterComponentType(6);
/// The header's `D3D_REGISTER_COMPONENT_UINT64`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_register_component_type).
pub const D3D_REGISTER_COMPONENT_UINT64: RegisterComponentType = RegisterComponentType(7);
/// The header's `D3D_REGISTER_COMPONENT_SINT64`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_register_component_type).
pub const D3D_REGISTER_COMPONENT_SINT64: RegisterComponentType = RegisterComponentType(8);
/// The header's `D3D_REGISTER_COMPONENT_FLOAT64`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_register_component_type).
pub const D3D_REGISTER_COMPONENT_FLOAT64: RegisterComponentType = RegisterComponentType(9);
/// The header's `D3D10_REGISTER_COMPONENT_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_register_component_type).
pub const D3D10_REGISTER_COMPONENT_UNKNOWN: RegisterComponentType = RegisterComponentType(0);
/// The header's `D3D10_REGISTER_COMPONENT_UINT32`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_register_component_type).
pub const D3D10_REGISTER_COMPONENT_UINT32: RegisterComponentType = RegisterComponentType(1);
/// The header's `D3D10_REGISTER_COMPONENT_SINT32`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_register_component_type).
pub const D3D10_REGISTER_COMPONENT_SINT32: RegisterComponentType = RegisterComponentType(2);
/// The header's `D3D10_REGISTER_COMPONENT_FLOAT32`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_register_component_type).
pub const D3D10_REGISTER_COMPONENT_FLOAT32: RegisterComponentType = RegisterComponentType(3);
/// The header's `D3D10_REGISTER_COMPONENT_UINT16`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_register_component_type).
pub const D3D10_REGISTER_COMPONENT_UINT16: RegisterComponentType = RegisterComponentType(4);
/// The header's `D3D10_REGISTER_COMPONENT_SINT16`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_register_component_type).
pub const D3D10_REGISTER_COMPONENT_SINT16: RegisterComponentType = RegisterComponentType(5);
/// The header's `D3D10_REGISTER_COMPONENT_FLOAT16`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_register_component_type).
pub const D3D10_REGISTER_COMPONENT_FLOAT16: RegisterComponentType = RegisterComponentType(6);
/// The header's `D3D10_REGISTER_COMPONENT_UINT64`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_register_component_type).
pub const D3D10_REGISTER_COMPONENT_UINT64: RegisterComponentType = RegisterComponentType(7);
/// The header's `D3D10_REGISTER_COMPONENT_SINT64`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_register_component_type).
pub const D3D10_REGISTER_COMPONENT_SINT64: RegisterComponentType = RegisterComponentType(8);
/// The header's `D3D10_REGISTER_COMPONENT_FLOAT64`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_register_component_type).
pub const D3D10_REGISTER_COMPONENT_FLOAT64: RegisterComponentType = RegisterComponentType(9);

/// The header's `D3D_RESOURCE_RETURN_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_resource_return_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ResourceReturnType(pub i32);

/// The header's `D3D_RETURN_TYPE_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_resource_return_type).
pub const D3D_RETURN_TYPE_UNORM: ResourceReturnType = ResourceReturnType(1);
/// The header's `D3D_RETURN_TYPE_SNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_resource_return_type).
pub const D3D_RETURN_TYPE_SNORM: ResourceReturnType = ResourceReturnType(2);
/// The header's `D3D_RETURN_TYPE_SINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_resource_return_type).
pub const D3D_RETURN_TYPE_SINT: ResourceReturnType = ResourceReturnType(3);
/// The header's `D3D_RETURN_TYPE_UINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_resource_return_type).
pub const D3D_RETURN_TYPE_UINT: ResourceReturnType = ResourceReturnType(4);
/// The header's `D3D_RETURN_TYPE_FLOAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_resource_return_type).
pub const D3D_RETURN_TYPE_FLOAT: ResourceReturnType = ResourceReturnType(5);
/// The header's `D3D_RETURN_TYPE_MIXED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_resource_return_type).
pub const D3D_RETURN_TYPE_MIXED: ResourceReturnType = ResourceReturnType(6);
/// The header's `D3D_RETURN_TYPE_DOUBLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_resource_return_type).
pub const D3D_RETURN_TYPE_DOUBLE: ResourceReturnType = ResourceReturnType(7);
/// The header's `D3D_RETURN_TYPE_CONTINUED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_resource_return_type).
pub const D3D_RETURN_TYPE_CONTINUED: ResourceReturnType = ResourceReturnType(8);
/// The header's `D3D10_RETURN_TYPE_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_resource_return_type).
pub const D3D10_RETURN_TYPE_UNORM: ResourceReturnType = ResourceReturnType(1);
/// The header's `D3D10_RETURN_TYPE_SNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_resource_return_type).
pub const D3D10_RETURN_TYPE_SNORM: ResourceReturnType = ResourceReturnType(2);
/// The header's `D3D10_RETURN_TYPE_SINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_resource_return_type).
pub const D3D10_RETURN_TYPE_SINT: ResourceReturnType = ResourceReturnType(3);
/// The header's `D3D10_RETURN_TYPE_UINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_resource_return_type).
pub const D3D10_RETURN_TYPE_UINT: ResourceReturnType = ResourceReturnType(4);
/// The header's `D3D10_RETURN_TYPE_FLOAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_resource_return_type).
pub const D3D10_RETURN_TYPE_FLOAT: ResourceReturnType = ResourceReturnType(5);
/// The header's `D3D10_RETURN_TYPE_MIXED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_resource_return_type).
pub const D3D10_RETURN_TYPE_MIXED: ResourceReturnType = ResourceReturnType(6);
/// The header's `D3D11_RETURN_TYPE_UNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_resource_return_type).
pub const D3D11_RETURN_TYPE_UNORM: ResourceReturnType = ResourceReturnType(1);
/// The header's `D3D11_RETURN_TYPE_SNORM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_resource_return_type).
pub const D3D11_RETURN_TYPE_SNORM: ResourceReturnType = ResourceReturnType(2);
/// The header's `D3D11_RETURN_TYPE_SINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_resource_return_type).
pub const D3D11_RETURN_TYPE_SINT: ResourceReturnType = ResourceReturnType(3);
/// The header's `D3D11_RETURN_TYPE_UINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_resource_return_type).
pub const D3D11_RETURN_TYPE_UINT: ResourceReturnType = ResourceReturnType(4);
/// The header's `D3D11_RETURN_TYPE_FLOAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_resource_return_type).
pub const D3D11_RETURN_TYPE_FLOAT: ResourceReturnType = ResourceReturnType(5);
/// The header's `D3D11_RETURN_TYPE_MIXED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_resource_return_type).
pub const D3D11_RETURN_TYPE_MIXED: ResourceReturnType = ResourceReturnType(6);
/// The header's `D3D11_RETURN_TYPE_DOUBLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_resource_return_type).
pub const D3D11_RETURN_TYPE_DOUBLE: ResourceReturnType = ResourceReturnType(7);
/// The header's `D3D11_RETURN_TYPE_CONTINUED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_resource_return_type).
pub const D3D11_RETURN_TYPE_CONTINUED: ResourceReturnType = ResourceReturnType(8);

/// The header's `D3D_SHADER_CBUFFER_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_cbuffer_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShaderCbufferFlags(pub i32);

/// The header's `D3D_CBF_USERPACKED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_cbuffer_flags).
pub const D3D_CBF_USERPACKED: ShaderCbufferFlags = ShaderCbufferFlags(1);
/// The header's `D3D10_CBF_USERPACKED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_cbuffer_flags).
pub const D3D10_CBF_USERPACKED: ShaderCbufferFlags = ShaderCbufferFlags(1);

/// The header's `D3D_SHADER_INPUT_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShaderInputFlags(pub i32);

/// The header's `D3D_SIF_USERPACKED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_flags).
pub const D3D_SIF_USERPACKED: ShaderInputFlags = ShaderInputFlags(1);
/// The header's `D3D_SIF_COMPARISON_SAMPLER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_flags).
pub const D3D_SIF_COMPARISON_SAMPLER: ShaderInputFlags = ShaderInputFlags(2);
/// The header's `D3D_SIF_TEXTURE_COMPONENT_0`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_flags).
pub const D3D_SIF_TEXTURE_COMPONENT_0: ShaderInputFlags = ShaderInputFlags(4);
/// The header's `D3D_SIF_TEXTURE_COMPONENT_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_flags).
pub const D3D_SIF_TEXTURE_COMPONENT_1: ShaderInputFlags = ShaderInputFlags(8);
/// The header's `D3D_SIF_TEXTURE_COMPONENTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_flags).
pub const D3D_SIF_TEXTURE_COMPONENTS: ShaderInputFlags = ShaderInputFlags(12);
/// The header's `D3D_SIF_UNUSED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_flags).
pub const D3D_SIF_UNUSED: ShaderInputFlags = ShaderInputFlags(16);
/// The header's `D3D10_SIF_USERPACKED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_flags).
pub const D3D10_SIF_USERPACKED: ShaderInputFlags = ShaderInputFlags(1);
/// The header's `D3D10_SIF_COMPARISON_SAMPLER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_flags).
pub const D3D10_SIF_COMPARISON_SAMPLER: ShaderInputFlags = ShaderInputFlags(2);
/// The header's `D3D10_SIF_TEXTURE_COMPONENT_0`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_flags).
pub const D3D10_SIF_TEXTURE_COMPONENT_0: ShaderInputFlags = ShaderInputFlags(4);
/// The header's `D3D10_SIF_TEXTURE_COMPONENT_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_flags).
pub const D3D10_SIF_TEXTURE_COMPONENT_1: ShaderInputFlags = ShaderInputFlags(8);
/// The header's `D3D10_SIF_TEXTURE_COMPONENTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_flags).
pub const D3D10_SIF_TEXTURE_COMPONENTS: ShaderInputFlags = ShaderInputFlags(12);

/// The header's `D3D_SHADER_INPUT_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShaderInputType(pub i32);

/// The header's `D3D_SIT_CBUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_type).
pub const D3D_SIT_CBUFFER: ShaderInputType = ShaderInputType(0);
/// The header's `D3D_SIT_TBUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_type).
pub const D3D_SIT_TBUFFER: ShaderInputType = ShaderInputType(1);
/// The header's `D3D_SIT_TEXTURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_type).
pub const D3D_SIT_TEXTURE: ShaderInputType = ShaderInputType(2);
/// The header's `D3D_SIT_SAMPLER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_type).
pub const D3D_SIT_SAMPLER: ShaderInputType = ShaderInputType(3);
/// The header's `D3D_SIT_UAV_RWTYPED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_type).
pub const D3D_SIT_UAV_RWTYPED: ShaderInputType = ShaderInputType(4);
/// The header's `D3D_SIT_STRUCTURED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_type).
pub const D3D_SIT_STRUCTURED: ShaderInputType = ShaderInputType(5);
/// The header's `D3D_SIT_UAV_RWSTRUCTURED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_type).
pub const D3D_SIT_UAV_RWSTRUCTURED: ShaderInputType = ShaderInputType(6);
/// The header's `D3D_SIT_BYTEADDRESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_type).
pub const D3D_SIT_BYTEADDRESS: ShaderInputType = ShaderInputType(7);
/// The header's `D3D_SIT_UAV_RWBYTEADDRESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_type).
pub const D3D_SIT_UAV_RWBYTEADDRESS: ShaderInputType = ShaderInputType(8);
/// The header's `D3D_SIT_UAV_APPEND_STRUCTURED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_type).
pub const D3D_SIT_UAV_APPEND_STRUCTURED: ShaderInputType = ShaderInputType(9);
/// The header's `D3D_SIT_UAV_CONSUME_STRUCTURED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_type).
pub const D3D_SIT_UAV_CONSUME_STRUCTURED: ShaderInputType = ShaderInputType(10);
/// The header's `D3D_SIT_UAV_RWSTRUCTURED_WITH_COUNTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_type).
pub const D3D_SIT_UAV_RWSTRUCTURED_WITH_COUNTER: ShaderInputType = ShaderInputType(11);
/// The header's `D3D_SIT_RTACCELERATIONSTRUCTURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_type).
pub const D3D_SIT_RTACCELERATIONSTRUCTURE: ShaderInputType = ShaderInputType(12);
/// The header's `D3D_SIT_UAV_FEEDBACKTEXTURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_type).
pub const D3D_SIT_UAV_FEEDBACKTEXTURE: ShaderInputType = ShaderInputType(13);
/// The header's `D3D10_SIT_CBUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_type).
pub const D3D10_SIT_CBUFFER: ShaderInputType = ShaderInputType(0);
/// The header's `D3D10_SIT_TBUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_type).
pub const D3D10_SIT_TBUFFER: ShaderInputType = ShaderInputType(1);
/// The header's `D3D10_SIT_TEXTURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_type).
pub const D3D10_SIT_TEXTURE: ShaderInputType = ShaderInputType(2);
/// The header's `D3D10_SIT_SAMPLER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_type).
pub const D3D10_SIT_SAMPLER: ShaderInputType = ShaderInputType(3);
/// The header's `D3D11_SIT_UAV_RWTYPED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_type).
pub const D3D11_SIT_UAV_RWTYPED: ShaderInputType = ShaderInputType(4);
/// The header's `D3D11_SIT_STRUCTURED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_type).
pub const D3D11_SIT_STRUCTURED: ShaderInputType = ShaderInputType(5);
/// The header's `D3D11_SIT_UAV_RWSTRUCTURED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_type).
pub const D3D11_SIT_UAV_RWSTRUCTURED: ShaderInputType = ShaderInputType(6);
/// The header's `D3D11_SIT_BYTEADDRESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_type).
pub const D3D11_SIT_BYTEADDRESS: ShaderInputType = ShaderInputType(7);
/// The header's `D3D11_SIT_UAV_RWBYTEADDRESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_type).
pub const D3D11_SIT_UAV_RWBYTEADDRESS: ShaderInputType = ShaderInputType(8);
/// The header's `D3D11_SIT_UAV_APPEND_STRUCTURED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_type).
pub const D3D11_SIT_UAV_APPEND_STRUCTURED: ShaderInputType = ShaderInputType(9);
/// The header's `D3D11_SIT_UAV_CONSUME_STRUCTURED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_type).
pub const D3D11_SIT_UAV_CONSUME_STRUCTURED: ShaderInputType = ShaderInputType(10);
/// The header's `D3D11_SIT_UAV_RWSTRUCTURED_WITH_COUNTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_input_type).
pub const D3D11_SIT_UAV_RWSTRUCTURED_WITH_COUNTER: ShaderInputType = ShaderInputType(11);

/// The header's `D3D_SHADER_VARIABLE_CLASS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_class).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShaderVariableClass(pub i32);

/// The header's `D3D_SVC_SCALAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_class).
pub const D3D_SVC_SCALAR: ShaderVariableClass = ShaderVariableClass(0);
/// The header's `D3D_SVC_VECTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_class).
pub const D3D_SVC_VECTOR: ShaderVariableClass = ShaderVariableClass(1);
/// The header's `D3D_SVC_MATRIX_ROWS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_class).
pub const D3D_SVC_MATRIX_ROWS: ShaderVariableClass = ShaderVariableClass(2);
/// The header's `D3D_SVC_MATRIX_COLUMNS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_class).
pub const D3D_SVC_MATRIX_COLUMNS: ShaderVariableClass = ShaderVariableClass(3);
/// The header's `D3D_SVC_OBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_class).
pub const D3D_SVC_OBJECT: ShaderVariableClass = ShaderVariableClass(4);
/// The header's `D3D_SVC_STRUCT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_class).
pub const D3D_SVC_STRUCT: ShaderVariableClass = ShaderVariableClass(5);
/// The header's `D3D_SVC_INTERFACE_CLASS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_class).
pub const D3D_SVC_INTERFACE_CLASS: ShaderVariableClass = ShaderVariableClass(6);
/// The header's `D3D_SVC_INTERFACE_POINTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_class).
pub const D3D_SVC_INTERFACE_POINTER: ShaderVariableClass = ShaderVariableClass(7);
/// The header's `D3D10_SVC_SCALAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_class).
pub const D3D10_SVC_SCALAR: ShaderVariableClass = ShaderVariableClass(0);
/// The header's `D3D10_SVC_VECTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_class).
pub const D3D10_SVC_VECTOR: ShaderVariableClass = ShaderVariableClass(1);
/// The header's `D3D10_SVC_MATRIX_ROWS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_class).
pub const D3D10_SVC_MATRIX_ROWS: ShaderVariableClass = ShaderVariableClass(2);
/// The header's `D3D10_SVC_MATRIX_COLUMNS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_class).
pub const D3D10_SVC_MATRIX_COLUMNS: ShaderVariableClass = ShaderVariableClass(3);
/// The header's `D3D10_SVC_OBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_class).
pub const D3D10_SVC_OBJECT: ShaderVariableClass = ShaderVariableClass(4);
/// The header's `D3D10_SVC_STRUCT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_class).
pub const D3D10_SVC_STRUCT: ShaderVariableClass = ShaderVariableClass(5);
/// The header's `D3D11_SVC_INTERFACE_CLASS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_class).
pub const D3D11_SVC_INTERFACE_CLASS: ShaderVariableClass = ShaderVariableClass(6);
/// The header's `D3D11_SVC_INTERFACE_POINTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_class).
pub const D3D11_SVC_INTERFACE_POINTER: ShaderVariableClass = ShaderVariableClass(7);

/// The header's `D3D_SHADER_VARIABLE_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShaderVariableFlags(pub i32);

/// The header's `D3D_SVF_USERPACKED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_flags).
pub const D3D_SVF_USERPACKED: ShaderVariableFlags = ShaderVariableFlags(1);
/// The header's `D3D_SVF_USED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_flags).
pub const D3D_SVF_USED: ShaderVariableFlags = ShaderVariableFlags(2);
/// The header's `D3D_SVF_INTERFACE_POINTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_flags).
pub const D3D_SVF_INTERFACE_POINTER: ShaderVariableFlags = ShaderVariableFlags(4);
/// The header's `D3D_SVF_INTERFACE_PARAMETER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_flags).
pub const D3D_SVF_INTERFACE_PARAMETER: ShaderVariableFlags = ShaderVariableFlags(8);
/// The header's `D3D10_SVF_USERPACKED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_flags).
pub const D3D10_SVF_USERPACKED: ShaderVariableFlags = ShaderVariableFlags(1);
/// The header's `D3D10_SVF_USED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_flags).
pub const D3D10_SVF_USED: ShaderVariableFlags = ShaderVariableFlags(2);
/// The header's `D3D11_SVF_INTERFACE_POINTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_flags).
pub const D3D11_SVF_INTERFACE_POINTER: ShaderVariableFlags = ShaderVariableFlags(4);
/// The header's `D3D11_SVF_INTERFACE_PARAMETER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_flags).
pub const D3D11_SVF_INTERFACE_PARAMETER: ShaderVariableFlags = ShaderVariableFlags(8);

/// The header's `D3D_SHADER_VARIABLE_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShaderVariableType(pub i32);

/// The header's `D3D_SVT_VOID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_VOID: ShaderVariableType = ShaderVariableType(0);
/// The header's `D3D_SVT_BOOL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_BOOL: ShaderVariableType = ShaderVariableType(1);
/// The header's `D3D_SVT_INT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_INT: ShaderVariableType = ShaderVariableType(2);
/// The header's `D3D_SVT_FLOAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_FLOAT: ShaderVariableType = ShaderVariableType(3);
/// The header's `D3D_SVT_STRING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_STRING: ShaderVariableType = ShaderVariableType(4);
/// The header's `D3D_SVT_TEXTURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_TEXTURE: ShaderVariableType = ShaderVariableType(5);
/// The header's `D3D_SVT_TEXTURE1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_TEXTURE1D: ShaderVariableType = ShaderVariableType(6);
/// The header's `D3D_SVT_TEXTURE2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_TEXTURE2D: ShaderVariableType = ShaderVariableType(7);
/// The header's `D3D_SVT_TEXTURE3D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_TEXTURE3D: ShaderVariableType = ShaderVariableType(8);
/// The header's `D3D_SVT_TEXTURECUBE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_TEXTURECUBE: ShaderVariableType = ShaderVariableType(9);
/// The header's `D3D_SVT_SAMPLER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_SAMPLER: ShaderVariableType = ShaderVariableType(10);
/// The header's `D3D_SVT_SAMPLER1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_SAMPLER1D: ShaderVariableType = ShaderVariableType(11);
/// The header's `D3D_SVT_SAMPLER2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_SAMPLER2D: ShaderVariableType = ShaderVariableType(12);
/// The header's `D3D_SVT_SAMPLER3D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_SAMPLER3D: ShaderVariableType = ShaderVariableType(13);
/// The header's `D3D_SVT_SAMPLERCUBE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_SAMPLERCUBE: ShaderVariableType = ShaderVariableType(14);
/// The header's `D3D_SVT_PIXELSHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_PIXELSHADER: ShaderVariableType = ShaderVariableType(15);
/// The header's `D3D_SVT_VERTEXSHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_VERTEXSHADER: ShaderVariableType = ShaderVariableType(16);
/// The header's `D3D_SVT_PIXELFRAGMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_PIXELFRAGMENT: ShaderVariableType = ShaderVariableType(17);
/// The header's `D3D_SVT_VERTEXFRAGMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_VERTEXFRAGMENT: ShaderVariableType = ShaderVariableType(18);
/// The header's `D3D_SVT_UINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_UINT: ShaderVariableType = ShaderVariableType(19);
/// The header's `D3D_SVT_UINT8`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_UINT8: ShaderVariableType = ShaderVariableType(20);
/// The header's `D3D_SVT_GEOMETRYSHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_GEOMETRYSHADER: ShaderVariableType = ShaderVariableType(21);
/// The header's `D3D_SVT_RASTERIZER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_RASTERIZER: ShaderVariableType = ShaderVariableType(22);
/// The header's `D3D_SVT_DEPTHSTENCIL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_DEPTHSTENCIL: ShaderVariableType = ShaderVariableType(23);
/// The header's `D3D_SVT_BLEND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_BLEND: ShaderVariableType = ShaderVariableType(24);
/// The header's `D3D_SVT_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_BUFFER: ShaderVariableType = ShaderVariableType(25);
/// The header's `D3D_SVT_CBUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_CBUFFER: ShaderVariableType = ShaderVariableType(26);
/// The header's `D3D_SVT_TBUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_TBUFFER: ShaderVariableType = ShaderVariableType(27);
/// The header's `D3D_SVT_TEXTURE1DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_TEXTURE1DARRAY: ShaderVariableType = ShaderVariableType(28);
/// The header's `D3D_SVT_TEXTURE2DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_TEXTURE2DARRAY: ShaderVariableType = ShaderVariableType(29);
/// The header's `D3D_SVT_RENDERTARGETVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_RENDERTARGETVIEW: ShaderVariableType = ShaderVariableType(30);
/// The header's `D3D_SVT_DEPTHSTENCILVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_DEPTHSTENCILVIEW: ShaderVariableType = ShaderVariableType(31);
/// The header's `D3D_SVT_TEXTURE2DMS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_TEXTURE2DMS: ShaderVariableType = ShaderVariableType(32);
/// The header's `D3D_SVT_TEXTURE2DMSARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_TEXTURE2DMSARRAY: ShaderVariableType = ShaderVariableType(33);
/// The header's `D3D_SVT_TEXTURECUBEARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_TEXTURECUBEARRAY: ShaderVariableType = ShaderVariableType(34);
/// The header's `D3D_SVT_HULLSHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_HULLSHADER: ShaderVariableType = ShaderVariableType(35);
/// The header's `D3D_SVT_DOMAINSHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_DOMAINSHADER: ShaderVariableType = ShaderVariableType(36);
/// The header's `D3D_SVT_INTERFACE_POINTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_INTERFACE_POINTER: ShaderVariableType = ShaderVariableType(37);
/// The header's `D3D_SVT_COMPUTESHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_COMPUTESHADER: ShaderVariableType = ShaderVariableType(38);
/// The header's `D3D_SVT_DOUBLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_DOUBLE: ShaderVariableType = ShaderVariableType(39);
/// The header's `D3D_SVT_RWTEXTURE1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_RWTEXTURE1D: ShaderVariableType = ShaderVariableType(40);
/// The header's `D3D_SVT_RWTEXTURE1DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_RWTEXTURE1DARRAY: ShaderVariableType = ShaderVariableType(41);
/// The header's `D3D_SVT_RWTEXTURE2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_RWTEXTURE2D: ShaderVariableType = ShaderVariableType(42);
/// The header's `D3D_SVT_RWTEXTURE2DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_RWTEXTURE2DARRAY: ShaderVariableType = ShaderVariableType(43);
/// The header's `D3D_SVT_RWTEXTURE3D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_RWTEXTURE3D: ShaderVariableType = ShaderVariableType(44);
/// The header's `D3D_SVT_RWBUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_RWBUFFER: ShaderVariableType = ShaderVariableType(45);
/// The header's `D3D_SVT_BYTEADDRESS_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_BYTEADDRESS_BUFFER: ShaderVariableType = ShaderVariableType(46);
/// The header's `D3D_SVT_RWBYTEADDRESS_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_RWBYTEADDRESS_BUFFER: ShaderVariableType = ShaderVariableType(47);
/// The header's `D3D_SVT_STRUCTURED_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_STRUCTURED_BUFFER: ShaderVariableType = ShaderVariableType(48);
/// The header's `D3D_SVT_RWSTRUCTURED_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_RWSTRUCTURED_BUFFER: ShaderVariableType = ShaderVariableType(49);
/// The header's `D3D_SVT_APPEND_STRUCTURED_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_APPEND_STRUCTURED_BUFFER: ShaderVariableType = ShaderVariableType(50);
/// The header's `D3D_SVT_CONSUME_STRUCTURED_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_CONSUME_STRUCTURED_BUFFER: ShaderVariableType = ShaderVariableType(51);
/// The header's `D3D_SVT_MIN8FLOAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_MIN8FLOAT: ShaderVariableType = ShaderVariableType(52);
/// The header's `D3D_SVT_MIN10FLOAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_MIN10FLOAT: ShaderVariableType = ShaderVariableType(53);
/// The header's `D3D_SVT_MIN16FLOAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_MIN16FLOAT: ShaderVariableType = ShaderVariableType(54);
/// The header's `D3D_SVT_MIN12INT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_MIN12INT: ShaderVariableType = ShaderVariableType(55);
/// The header's `D3D_SVT_MIN16INT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_MIN16INT: ShaderVariableType = ShaderVariableType(56);
/// The header's `D3D_SVT_MIN16UINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_MIN16UINT: ShaderVariableType = ShaderVariableType(57);
/// The header's `D3D_SVT_INT16`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_INT16: ShaderVariableType = ShaderVariableType(58);
/// The header's `D3D_SVT_UINT16`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_UINT16: ShaderVariableType = ShaderVariableType(59);
/// The header's `D3D_SVT_FLOAT16`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_FLOAT16: ShaderVariableType = ShaderVariableType(60);
/// The header's `D3D_SVT_INT64`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_INT64: ShaderVariableType = ShaderVariableType(61);
/// The header's `D3D_SVT_UINT64`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D_SVT_UINT64: ShaderVariableType = ShaderVariableType(62);
/// The header's `D3D10_SVT_VOID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_VOID: ShaderVariableType = ShaderVariableType(0);
/// The header's `D3D10_SVT_BOOL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_BOOL: ShaderVariableType = ShaderVariableType(1);
/// The header's `D3D10_SVT_INT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_INT: ShaderVariableType = ShaderVariableType(2);
/// The header's `D3D10_SVT_FLOAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_FLOAT: ShaderVariableType = ShaderVariableType(3);
/// The header's `D3D10_SVT_STRING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_STRING: ShaderVariableType = ShaderVariableType(4);
/// The header's `D3D10_SVT_TEXTURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_TEXTURE: ShaderVariableType = ShaderVariableType(5);
/// The header's `D3D10_SVT_TEXTURE1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_TEXTURE1D: ShaderVariableType = ShaderVariableType(6);
/// The header's `D3D10_SVT_TEXTURE2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_TEXTURE2D: ShaderVariableType = ShaderVariableType(7);
/// The header's `D3D10_SVT_TEXTURE3D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_TEXTURE3D: ShaderVariableType = ShaderVariableType(8);
/// The header's `D3D10_SVT_TEXTURECUBE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_TEXTURECUBE: ShaderVariableType = ShaderVariableType(9);
/// The header's `D3D10_SVT_SAMPLER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_SAMPLER: ShaderVariableType = ShaderVariableType(10);
/// The header's `D3D10_SVT_SAMPLER1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_SAMPLER1D: ShaderVariableType = ShaderVariableType(11);
/// The header's `D3D10_SVT_SAMPLER2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_SAMPLER2D: ShaderVariableType = ShaderVariableType(12);
/// The header's `D3D10_SVT_SAMPLER3D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_SAMPLER3D: ShaderVariableType = ShaderVariableType(13);
/// The header's `D3D10_SVT_SAMPLERCUBE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_SAMPLERCUBE: ShaderVariableType = ShaderVariableType(14);
/// The header's `D3D10_SVT_PIXELSHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_PIXELSHADER: ShaderVariableType = ShaderVariableType(15);
/// The header's `D3D10_SVT_VERTEXSHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_VERTEXSHADER: ShaderVariableType = ShaderVariableType(16);
/// The header's `D3D10_SVT_PIXELFRAGMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_PIXELFRAGMENT: ShaderVariableType = ShaderVariableType(17);
/// The header's `D3D10_SVT_VERTEXFRAGMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_VERTEXFRAGMENT: ShaderVariableType = ShaderVariableType(18);
/// The header's `D3D10_SVT_UINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_UINT: ShaderVariableType = ShaderVariableType(19);
/// The header's `D3D10_SVT_UINT8`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_UINT8: ShaderVariableType = ShaderVariableType(20);
/// The header's `D3D10_SVT_GEOMETRYSHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_GEOMETRYSHADER: ShaderVariableType = ShaderVariableType(21);
/// The header's `D3D10_SVT_RASTERIZER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_RASTERIZER: ShaderVariableType = ShaderVariableType(22);
/// The header's `D3D10_SVT_DEPTHSTENCIL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_DEPTHSTENCIL: ShaderVariableType = ShaderVariableType(23);
/// The header's `D3D10_SVT_BLEND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_BLEND: ShaderVariableType = ShaderVariableType(24);
/// The header's `D3D10_SVT_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_BUFFER: ShaderVariableType = ShaderVariableType(25);
/// The header's `D3D10_SVT_CBUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_CBUFFER: ShaderVariableType = ShaderVariableType(26);
/// The header's `D3D10_SVT_TBUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_TBUFFER: ShaderVariableType = ShaderVariableType(27);
/// The header's `D3D10_SVT_TEXTURE1DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_TEXTURE1DARRAY: ShaderVariableType = ShaderVariableType(28);
/// The header's `D3D10_SVT_TEXTURE2DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_TEXTURE2DARRAY: ShaderVariableType = ShaderVariableType(29);
/// The header's `D3D10_SVT_RENDERTARGETVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_RENDERTARGETVIEW: ShaderVariableType = ShaderVariableType(30);
/// The header's `D3D10_SVT_DEPTHSTENCILVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_DEPTHSTENCILVIEW: ShaderVariableType = ShaderVariableType(31);
/// The header's `D3D10_SVT_TEXTURE2DMS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_TEXTURE2DMS: ShaderVariableType = ShaderVariableType(32);
/// The header's `D3D10_SVT_TEXTURE2DMSARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_TEXTURE2DMSARRAY: ShaderVariableType = ShaderVariableType(33);
/// The header's `D3D10_SVT_TEXTURECUBEARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D10_SVT_TEXTURECUBEARRAY: ShaderVariableType = ShaderVariableType(34);
/// The header's `D3D11_SVT_HULLSHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D11_SVT_HULLSHADER: ShaderVariableType = ShaderVariableType(35);
/// The header's `D3D11_SVT_DOMAINSHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D11_SVT_DOMAINSHADER: ShaderVariableType = ShaderVariableType(36);
/// The header's `D3D11_SVT_INTERFACE_POINTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D11_SVT_INTERFACE_POINTER: ShaderVariableType = ShaderVariableType(37);
/// The header's `D3D11_SVT_COMPUTESHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D11_SVT_COMPUTESHADER: ShaderVariableType = ShaderVariableType(38);
/// The header's `D3D11_SVT_DOUBLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D11_SVT_DOUBLE: ShaderVariableType = ShaderVariableType(39);
/// The header's `D3D11_SVT_RWTEXTURE1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D11_SVT_RWTEXTURE1D: ShaderVariableType = ShaderVariableType(40);
/// The header's `D3D11_SVT_RWTEXTURE1DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D11_SVT_RWTEXTURE1DARRAY: ShaderVariableType = ShaderVariableType(41);
/// The header's `D3D11_SVT_RWTEXTURE2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D11_SVT_RWTEXTURE2D: ShaderVariableType = ShaderVariableType(42);
/// The header's `D3D11_SVT_RWTEXTURE2DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D11_SVT_RWTEXTURE2DARRAY: ShaderVariableType = ShaderVariableType(43);
/// The header's `D3D11_SVT_RWTEXTURE3D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D11_SVT_RWTEXTURE3D: ShaderVariableType = ShaderVariableType(44);
/// The header's `D3D11_SVT_RWBUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D11_SVT_RWBUFFER: ShaderVariableType = ShaderVariableType(45);
/// The header's `D3D11_SVT_BYTEADDRESS_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D11_SVT_BYTEADDRESS_BUFFER: ShaderVariableType = ShaderVariableType(46);
/// The header's `D3D11_SVT_RWBYTEADDRESS_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D11_SVT_RWBYTEADDRESS_BUFFER: ShaderVariableType = ShaderVariableType(47);
/// The header's `D3D11_SVT_STRUCTURED_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D11_SVT_STRUCTURED_BUFFER: ShaderVariableType = ShaderVariableType(48);
/// The header's `D3D11_SVT_RWSTRUCTURED_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D11_SVT_RWSTRUCTURED_BUFFER: ShaderVariableType = ShaderVariableType(49);
/// The header's `D3D11_SVT_APPEND_STRUCTURED_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D11_SVT_APPEND_STRUCTURED_BUFFER: ShaderVariableType = ShaderVariableType(50);
/// The header's `D3D11_SVT_CONSUME_STRUCTURED_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_shader_variable_type).
pub const D3D11_SVT_CONSUME_STRUCTURED_BUFFER: ShaderVariableType = ShaderVariableType(51);

/// The header's `D3D_SRV_DIMENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SrvDimension(pub i32);

/// The header's `D3D_SRV_DIMENSION_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D_SRV_DIMENSION_UNKNOWN: SrvDimension = SrvDimension(0);
/// The header's `D3D_SRV_DIMENSION_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D_SRV_DIMENSION_BUFFER: SrvDimension = SrvDimension(1);
/// The header's `D3D_SRV_DIMENSION_TEXTURE1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D_SRV_DIMENSION_TEXTURE1D: SrvDimension = SrvDimension(2);
/// The header's `D3D_SRV_DIMENSION_TEXTURE1DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D_SRV_DIMENSION_TEXTURE1DARRAY: SrvDimension = SrvDimension(3);
/// The header's `D3D_SRV_DIMENSION_TEXTURE2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D_SRV_DIMENSION_TEXTURE2D: SrvDimension = SrvDimension(4);
/// The header's `D3D_SRV_DIMENSION_TEXTURE2DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D_SRV_DIMENSION_TEXTURE2DARRAY: SrvDimension = SrvDimension(5);
/// The header's `D3D_SRV_DIMENSION_TEXTURE2DMS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D_SRV_DIMENSION_TEXTURE2DMS: SrvDimension = SrvDimension(6);
/// The header's `D3D_SRV_DIMENSION_TEXTURE2DMSARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D_SRV_DIMENSION_TEXTURE2DMSARRAY: SrvDimension = SrvDimension(7);
/// The header's `D3D_SRV_DIMENSION_TEXTURE3D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D_SRV_DIMENSION_TEXTURE3D: SrvDimension = SrvDimension(8);
/// The header's `D3D_SRV_DIMENSION_TEXTURECUBE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D_SRV_DIMENSION_TEXTURECUBE: SrvDimension = SrvDimension(9);
/// The header's `D3D_SRV_DIMENSION_TEXTURECUBEARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D_SRV_DIMENSION_TEXTURECUBEARRAY: SrvDimension = SrvDimension(10);
/// The header's `D3D_SRV_DIMENSION_BUFFEREX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D_SRV_DIMENSION_BUFFEREX: SrvDimension = SrvDimension(11);
/// The header's `D3D10_SRV_DIMENSION_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D10_SRV_DIMENSION_UNKNOWN: SrvDimension = SrvDimension(0);
/// The header's `D3D10_SRV_DIMENSION_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D10_SRV_DIMENSION_BUFFER: SrvDimension = SrvDimension(1);
/// The header's `D3D10_SRV_DIMENSION_TEXTURE1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D10_SRV_DIMENSION_TEXTURE1D: SrvDimension = SrvDimension(2);
/// The header's `D3D10_SRV_DIMENSION_TEXTURE1DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D10_SRV_DIMENSION_TEXTURE1DARRAY: SrvDimension = SrvDimension(3);
/// The header's `D3D10_SRV_DIMENSION_TEXTURE2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D10_SRV_DIMENSION_TEXTURE2D: SrvDimension = SrvDimension(4);
/// The header's `D3D10_SRV_DIMENSION_TEXTURE2DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D10_SRV_DIMENSION_TEXTURE2DARRAY: SrvDimension = SrvDimension(5);
/// The header's `D3D10_SRV_DIMENSION_TEXTURE2DMS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D10_SRV_DIMENSION_TEXTURE2DMS: SrvDimension = SrvDimension(6);
/// The header's `D3D10_SRV_DIMENSION_TEXTURE2DMSARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D10_SRV_DIMENSION_TEXTURE2DMSARRAY: SrvDimension = SrvDimension(7);
/// The header's `D3D10_SRV_DIMENSION_TEXTURE3D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D10_SRV_DIMENSION_TEXTURE3D: SrvDimension = SrvDimension(8);
/// The header's `D3D10_SRV_DIMENSION_TEXTURECUBE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D10_SRV_DIMENSION_TEXTURECUBE: SrvDimension = SrvDimension(9);
/// The header's `D3D10_1_SRV_DIMENSION_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D10_1_SRV_DIMENSION_UNKNOWN: SrvDimension = SrvDimension(0);
/// The header's `D3D10_1_SRV_DIMENSION_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D10_1_SRV_DIMENSION_BUFFER: SrvDimension = SrvDimension(1);
/// The header's `D3D10_1_SRV_DIMENSION_TEXTURE1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D10_1_SRV_DIMENSION_TEXTURE1D: SrvDimension = SrvDimension(2);
/// The header's `D3D10_1_SRV_DIMENSION_TEXTURE1DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D10_1_SRV_DIMENSION_TEXTURE1DARRAY: SrvDimension = SrvDimension(3);
/// The header's `D3D10_1_SRV_DIMENSION_TEXTURE2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D10_1_SRV_DIMENSION_TEXTURE2D: SrvDimension = SrvDimension(4);
/// The header's `D3D10_1_SRV_DIMENSION_TEXTURE2DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D10_1_SRV_DIMENSION_TEXTURE2DARRAY: SrvDimension = SrvDimension(5);
/// The header's `D3D10_1_SRV_DIMENSION_TEXTURE2DMS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D10_1_SRV_DIMENSION_TEXTURE2DMS: SrvDimension = SrvDimension(6);
/// The header's `D3D10_1_SRV_DIMENSION_TEXTURE2DMSARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D10_1_SRV_DIMENSION_TEXTURE2DMSARRAY: SrvDimension = SrvDimension(7);
/// The header's `D3D10_1_SRV_DIMENSION_TEXTURE3D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D10_1_SRV_DIMENSION_TEXTURE3D: SrvDimension = SrvDimension(8);
/// The header's `D3D10_1_SRV_DIMENSION_TEXTURECUBE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D10_1_SRV_DIMENSION_TEXTURECUBE: SrvDimension = SrvDimension(9);
/// The header's `D3D10_1_SRV_DIMENSION_TEXTURECUBEARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D10_1_SRV_DIMENSION_TEXTURECUBEARRAY: SrvDimension = SrvDimension(10);
/// The header's `D3D11_SRV_DIMENSION_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D11_SRV_DIMENSION_UNKNOWN: SrvDimension = SrvDimension(0);
/// The header's `D3D11_SRV_DIMENSION_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D11_SRV_DIMENSION_BUFFER: SrvDimension = SrvDimension(1);
/// The header's `D3D11_SRV_DIMENSION_TEXTURE1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D11_SRV_DIMENSION_TEXTURE1D: SrvDimension = SrvDimension(2);
/// The header's `D3D11_SRV_DIMENSION_TEXTURE1DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D11_SRV_DIMENSION_TEXTURE1DARRAY: SrvDimension = SrvDimension(3);
/// The header's `D3D11_SRV_DIMENSION_TEXTURE2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D11_SRV_DIMENSION_TEXTURE2D: SrvDimension = SrvDimension(4);
/// The header's `D3D11_SRV_DIMENSION_TEXTURE2DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D11_SRV_DIMENSION_TEXTURE2DARRAY: SrvDimension = SrvDimension(5);
/// The header's `D3D11_SRV_DIMENSION_TEXTURE2DMS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D11_SRV_DIMENSION_TEXTURE2DMS: SrvDimension = SrvDimension(6);
/// The header's `D3D11_SRV_DIMENSION_TEXTURE2DMSARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D11_SRV_DIMENSION_TEXTURE2DMSARRAY: SrvDimension = SrvDimension(7);
/// The header's `D3D11_SRV_DIMENSION_TEXTURE3D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D11_SRV_DIMENSION_TEXTURE3D: SrvDimension = SrvDimension(8);
/// The header's `D3D11_SRV_DIMENSION_TEXTURECUBE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D11_SRV_DIMENSION_TEXTURECUBE: SrvDimension = SrvDimension(9);
/// The header's `D3D11_SRV_DIMENSION_TEXTURECUBEARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D11_SRV_DIMENSION_TEXTURECUBEARRAY: SrvDimension = SrvDimension(10);
/// The header's `D3D11_SRV_DIMENSION_BUFFEREX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_srv_dimension).
pub const D3D11_SRV_DIMENSION_BUFFEREX: SrvDimension = SrvDimension(11);

/// The header's `D3D_TESSELLATOR_DOMAIN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_domain).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TessellatorDomain(pub i32);

/// The header's `D3D_TESSELLATOR_DOMAIN_UNDEFINED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_domain).
pub const D3D_TESSELLATOR_DOMAIN_UNDEFINED: TessellatorDomain = TessellatorDomain(0);
/// The header's `D3D_TESSELLATOR_DOMAIN_ISOLINE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_domain).
pub const D3D_TESSELLATOR_DOMAIN_ISOLINE: TessellatorDomain = TessellatorDomain(1);
/// The header's `D3D_TESSELLATOR_DOMAIN_TRI`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_domain).
pub const D3D_TESSELLATOR_DOMAIN_TRI: TessellatorDomain = TessellatorDomain(2);
/// The header's `D3D_TESSELLATOR_DOMAIN_QUAD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_domain).
pub const D3D_TESSELLATOR_DOMAIN_QUAD: TessellatorDomain = TessellatorDomain(3);
/// The header's `D3D11_TESSELLATOR_DOMAIN_UNDEFINED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_domain).
pub const D3D11_TESSELLATOR_DOMAIN_UNDEFINED: TessellatorDomain = TessellatorDomain(0);
/// The header's `D3D11_TESSELLATOR_DOMAIN_ISOLINE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_domain).
pub const D3D11_TESSELLATOR_DOMAIN_ISOLINE: TessellatorDomain = TessellatorDomain(1);
/// The header's `D3D11_TESSELLATOR_DOMAIN_TRI`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_domain).
pub const D3D11_TESSELLATOR_DOMAIN_TRI: TessellatorDomain = TessellatorDomain(2);
/// The header's `D3D11_TESSELLATOR_DOMAIN_QUAD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_domain).
pub const D3D11_TESSELLATOR_DOMAIN_QUAD: TessellatorDomain = TessellatorDomain(3);

/// The header's `D3D_TESSELLATOR_OUTPUT_PRIMITIVE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_output_primitive).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TessellatorOutputPrimitive(pub i32);

/// The header's `D3D_TESSELLATOR_OUTPUT_UNDEFINED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_output_primitive).
pub const D3D_TESSELLATOR_OUTPUT_UNDEFINED: TessellatorOutputPrimitive =
    TessellatorOutputPrimitive(0);
/// The header's `D3D_TESSELLATOR_OUTPUT_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_output_primitive).
pub const D3D_TESSELLATOR_OUTPUT_POINT: TessellatorOutputPrimitive = TessellatorOutputPrimitive(1);
/// The header's `D3D_TESSELLATOR_OUTPUT_LINE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_output_primitive).
pub const D3D_TESSELLATOR_OUTPUT_LINE: TessellatorOutputPrimitive = TessellatorOutputPrimitive(2);
/// The header's `D3D_TESSELLATOR_OUTPUT_TRIANGLE_CW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_output_primitive).
pub const D3D_TESSELLATOR_OUTPUT_TRIANGLE_CW: TessellatorOutputPrimitive =
    TessellatorOutputPrimitive(3);
/// The header's `D3D_TESSELLATOR_OUTPUT_TRIANGLE_CCW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_output_primitive).
pub const D3D_TESSELLATOR_OUTPUT_TRIANGLE_CCW: TessellatorOutputPrimitive =
    TessellatorOutputPrimitive(4);
/// The header's `D3D11_TESSELLATOR_OUTPUT_UNDEFINED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_output_primitive).
pub const D3D11_TESSELLATOR_OUTPUT_UNDEFINED: TessellatorOutputPrimitive =
    TessellatorOutputPrimitive(0);
/// The header's `D3D11_TESSELLATOR_OUTPUT_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_output_primitive).
pub const D3D11_TESSELLATOR_OUTPUT_POINT: TessellatorOutputPrimitive =
    TessellatorOutputPrimitive(1);
/// The header's `D3D11_TESSELLATOR_OUTPUT_LINE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_output_primitive).
pub const D3D11_TESSELLATOR_OUTPUT_LINE: TessellatorOutputPrimitive = TessellatorOutputPrimitive(2);
/// The header's `D3D11_TESSELLATOR_OUTPUT_TRIANGLE_CW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_output_primitive).
pub const D3D11_TESSELLATOR_OUTPUT_TRIANGLE_CW: TessellatorOutputPrimitive =
    TessellatorOutputPrimitive(3);
/// The header's `D3D11_TESSELLATOR_OUTPUT_TRIANGLE_CCW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_output_primitive).
pub const D3D11_TESSELLATOR_OUTPUT_TRIANGLE_CCW: TessellatorOutputPrimitive =
    TessellatorOutputPrimitive(4);

/// The header's `D3D_TESSELLATOR_PARTITIONING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_partitioning).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TessellatorPartitioning(pub i32);

/// The header's `D3D_TESSELLATOR_PARTITIONING_UNDEFINED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_partitioning).
pub const D3D_TESSELLATOR_PARTITIONING_UNDEFINED: TessellatorPartitioning =
    TessellatorPartitioning(0);
/// The header's `D3D_TESSELLATOR_PARTITIONING_INTEGER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_partitioning).
pub const D3D_TESSELLATOR_PARTITIONING_INTEGER: TessellatorPartitioning =
    TessellatorPartitioning(1);
/// The header's `D3D_TESSELLATOR_PARTITIONING_POW2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_partitioning).
pub const D3D_TESSELLATOR_PARTITIONING_POW2: TessellatorPartitioning = TessellatorPartitioning(2);
/// The header's `D3D_TESSELLATOR_PARTITIONING_FRACTIONAL_ODD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_partitioning).
pub const D3D_TESSELLATOR_PARTITIONING_FRACTIONAL_ODD: TessellatorPartitioning =
    TessellatorPartitioning(3);
/// The header's `D3D_TESSELLATOR_PARTITIONING_FRACTIONAL_EVEN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_partitioning).
pub const D3D_TESSELLATOR_PARTITIONING_FRACTIONAL_EVEN: TessellatorPartitioning =
    TessellatorPartitioning(4);
/// The header's `D3D11_TESSELLATOR_PARTITIONING_UNDEFINED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_partitioning).
pub const D3D11_TESSELLATOR_PARTITIONING_UNDEFINED: TessellatorPartitioning =
    TessellatorPartitioning(0);
/// The header's `D3D11_TESSELLATOR_PARTITIONING_INTEGER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_partitioning).
pub const D3D11_TESSELLATOR_PARTITIONING_INTEGER: TessellatorPartitioning =
    TessellatorPartitioning(1);
/// The header's `D3D11_TESSELLATOR_PARTITIONING_POW2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_partitioning).
pub const D3D11_TESSELLATOR_PARTITIONING_POW2: TessellatorPartitioning = TessellatorPartitioning(2);
/// The header's `D3D11_TESSELLATOR_PARTITIONING_FRACTIONAL_ODD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_partitioning).
pub const D3D11_TESSELLATOR_PARTITIONING_FRACTIONAL_ODD: TessellatorPartitioning =
    TessellatorPartitioning(3);
/// The header's `D3D11_TESSELLATOR_PARTITIONING_FRACTIONAL_EVEN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_tessellator_partitioning).
pub const D3D11_TESSELLATOR_PARTITIONING_FRACTIONAL_EVEN: TessellatorPartitioning =
    TessellatorPartitioning(4);
